use black_scholes_rust::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
use std::collections::HashMap;
use std::error::Error;
use tokio::time::{sleep, Duration};

/// Calculate RSI
fn calculate_rsi(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period + 1 {
        return None;
    }

    let mut gains = 0.0;
    let mut losses = 0.0;

    for i in (prices.len() - period)..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > 0.0 {
            gains += change;
        } else {
            losses -= change;
        }
    }

    let avg_gain = gains / period as f64;
    let avg_loss = losses / period as f64;

    if avg_loss == 0.0 {
        return Some(100.0);
    }

    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

/// Calculate volatility
fn calculate_volatility(prices: &[f64]) -> Option<f64> {
    if prices.len() < 20 {
        return None;
    }

    let returns: Vec<f64> = prices
        .windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / returns.len() as f64;

    Some(variance.sqrt() * (252.0_f64).sqrt())
}

#[derive(Debug, Clone, PartialEq)]
enum Signal {
    Buy,
    Sell,
    Hold,
}

/// Generate signal based on proven backtested strategy
fn generate_signal(prices: &[f64], current_price: f64, annual_vol: f64) -> Signal {
    if prices.len() < 30 {
        return Signal::Hold;
    }

    // Volatility-adaptive thresholds (from backtesting)
    let (rsi_oversold, rsi_overbought, momentum_threshold) = if annual_vol > 0.50 {
        (40.0, 60.0, 0.03) // High vol: aggressive
    } else if annual_vol > 0.35 {
        (35.0, 65.0, 0.02) // Medium vol: moderate
    } else {
        return Signal::Hold; // Low vol: skip (learned from AAPL)
    };

    let rsi = match calculate_rsi(prices, 14) {
        Some(r) => r,
        None => return Signal::Hold,
    };

    let momentum = if prices.len() >= 5 {
        (current_price - prices[prices.len() - 5]) / prices[prices.len() - 5]
    } else {
        0.0
    };

    if momentum > momentum_threshold && rsi < rsi_overbought {
        return Signal::Buy;
    }

    if momentum < -momentum_threshold && rsi > rsi_oversold {
        return Signal::Sell;
    }

    Signal::Hold
}

/// Trading bot state
struct TradingBot {
    client: AlpacaClient,
    symbols: Vec<String>,
    position_size: f64,
    max_positions: usize,
}

impl TradingBot {
    fn new(client: AlpacaClient) -> Self {
        Self {
            client,
            symbols: vec![
                "TSLA".to_string(),
                "NVDA".to_string(),
                "META".to_string(),
                "AMZN".to_string(),
                "GOOGL".to_string(),
            ],
            position_size: 5.0, // Shares per trade
            max_positions: 3,   // Max concurrent positions
        }
    }

    async fn run_iteration(&self) -> Result<(), Box<dyn Error>> {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("\n{:=>60}", "");
        println!("🤖 Trading Bot Iteration - {}", timestamp);
        println!("{:=>60}\n", "");

        // Get account status
        let account = self.client.get_account().await?;
        println!("💰 Account: ${:.2} cash | ${:.2} portfolio value",
            account.cash, account.portfolio_value);

        // Get current positions
        let positions = self.client.get_positions().await?;
        let position_map: HashMap<String, f64> = positions
            .iter()
            .map(|p| (p.symbol.clone(), p.qty.parse().unwrap_or(0.0)))
            .collect();

        if !positions.is_empty() {
            println!("\n📊 Positions ({}):", positions.len());
            for pos in &positions {
                let pl_pct = (pos.unrealized_pl.parse::<f64>().unwrap_or(0.0) 
                    / (pos.avg_entry_price.parse::<f64>().unwrap_or(1.0) 
                    * pos.qty.parse::<f64>().unwrap_or(1.0))) * 100.0;
                println!("   {} | {:.0} @ ${:.2} | P&L: ${:.2} ({:+.1}%)",
                    pos.symbol,
                    pos.qty.parse::<f64>().unwrap_or(0.0),
                    pos.avg_entry_price.parse::<f64>().unwrap_or(0.0),
                    pos.unrealized_pl.parse::<f64>().unwrap_or(0.0),
                    pl_pct
                );
            }
        }

        println!("\n🔍 Scanning for signals...\n");

        // Analyze each symbol
        for symbol in &self.symbols {
            // Skip if we already have max positions and don't own this one
            if positions.len() >= self.max_positions && !position_map.contains_key(symbol) {
                continue;
            }

            // Get 60 days of bars to ensure we have at least 30 trading days
            let end_time = chrono::Utc::now();
            let start_time = end_time - chrono::Duration::days(60);
            
            let start_str = start_time.format("%Y-%m-%d").to_string();
            let end_str = end_time.format("%Y-%m-%d").to_string();

            let bars = match self.client
                .get_bars(
                    symbol,
                    "1Day",
                    &start_str,
                    Some(&end_str),
                    Some(60),
                )
                .await
            {
                Ok(b) if !b.is_empty() => b,
                _ => continue,
            };

            let prices: Vec<f64> = bars.iter().map(|b| b.c).collect();

            // Get current price
            let snapshot = match self.client.get_snapshot(symbol).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            let current_price = snapshot
                .latest_trade
                .as_ref()
                .map(|t| t.price)
                .unwrap_or_else(|| *prices.last().unwrap());

            // Calculate metrics
            let annual_vol = calculate_volatility(&prices).unwrap_or(0.0);
            let signal = generate_signal(&prices, current_price, annual_vol);

            // Only show interesting signals
            if signal != Signal::Hold || position_map.contains_key(symbol) {
                print!("   {} ${:.2} | Vol: {:.1}% | ", 
                    symbol, current_price, annual_vol * 100.0);

                match signal {
                    Signal::Buy => print!("🟢 BUY"),
                    Signal::Sell => print!("🔴 SELL"),
                    Signal::Hold => print!("⏸️  HOLD"),
                }

                let has_position = position_map.contains_key(symbol);

                // Execute trades
                match (signal, has_position) {
                    (Signal::Buy, false) => {
                        print!(" → Buying {} shares...", self.position_size);
                        let order = OrderRequest {
                            symbol: symbol.clone(),
                            qty: self.position_size,
                            side: OrderSide::Buy,
                            r#type: OrderType::Market,
                            time_in_force: TimeInForce::Day,
                            limit_price: None,
                            stop_price: None,
                            extended_hours: None,
                            client_order_id: None,
                        };

                        match self.client.submit_order(&order).await {
                            Ok(_) => println!(" ✅"),
                            Err(e) => println!(" ❌ {}", e),
                        }
                    }
                    (Signal::Sell, true) => {
                        print!(" → Closing position...");
                        match self.client.close_position(symbol).await {
                            Ok(_) => println!(" ✅"),
                            Err(e) => println!(" ❌ {}", e),
                        }
                    }
                    _ => println!(),
                }
            }
        }

        Ok(())
    }

    async fn run_continuous(&self, interval_minutes: u64) -> Result<(), Box<dyn Error>> {
        println!("\n🚀 Starting Continuous Trading Bot");
        println!("   Symbols: {:?}", self.symbols);
        println!("   Position Size: {} shares", self.position_size);
        println!("   Max Positions: {}", self.max_positions);
        println!("   Check Interval: {} minutes", interval_minutes);
        println!("\n   Press Ctrl+C to stop\n");

        loop {
            if let Err(e) = self.run_iteration().await {
                eprintln!("❌ Error in iteration: {}", e);
            }

            println!("\n💤 Sleeping for {} minutes...", interval_minutes);
            sleep(Duration::from_secs(interval_minutes * 60)).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = AlpacaClient::from_env()?;
    let bot = TradingBot::new(client);

    // Choose mode: single run or continuous
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && args[1] == "--continuous" {
        let interval = if args.len() > 2 {
            args[2].parse().unwrap_or(5)
        } else {
            5 // Default: 5 minutes
        };
        bot.run_continuous(interval).await?;
    } else {
        // Single iteration
        bot.run_iteration().await?;
        println!("\n💡 Run with --continuous to keep trading");
        println!("💡 Example: cargo run --example trading_bot --continuous 5");
    }

    Ok(())
}
