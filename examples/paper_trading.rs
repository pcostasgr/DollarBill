use dollarbill::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
use dollarbill::market_data::symbols::load_enabled_stocks;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PaperTradingConfig {
    trading: TradingConfig,
    signals: SignalsConfig,
    paper_trading: PaperTradingSettings,
}

#[derive(Debug, Deserialize)]
struct TradingConfig {
    position_size_shares: f64,
    max_positions: usize,
    risk_management: RiskManagementConfig,
}

#[derive(Debug, Deserialize)]
struct RiskManagementConfig {
    stop_loss_pct: f64,
    take_profit_pct: f64,
}

#[derive(Debug, Deserialize)]
struct SignalsConfig {
    rsi_period: usize,
    momentum_period: usize,
    volatility_thresholds: VolatilityThresholds,
    thresholds: Thresholds,
}

#[derive(Debug, Deserialize)]
struct VolatilityThresholds {
    high_vol_threshold: f64,
    medium_vol_threshold: f64,
}

#[derive(Debug, Deserialize)]
struct Thresholds {
    high_vol: VolThresholds,
    medium_vol: VolThresholds,
}

#[derive(Debug, Deserialize)]
struct VolThresholds {
    rsi_oversold: f64,
    rsi_overbought: f64,
    momentum_threshold: f64,
}

#[derive(Debug, Deserialize)]
struct PaperTradingSettings {
    initial_balance: f64,
    commission_per_trade: f64,
    data_lookback_days: i64,
    simulation_days: usize,
}

/// Calculate RSI from price history
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

/// Calculate volatility from daily returns
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

/// Signal types
#[derive(Debug, Clone, PartialEq)]
enum Signal {
    Buy,
    Sell,
    Hold,
}

/// Generate trading signal based on momentum + RSI strategy
fn generate_signal(
    prices: &[f64],
    current_price: f64,
    annual_vol: f64,
    config: &SignalsConfig,
) -> Signal {
    if prices.len() < 30 {
        return Signal::Hold;
    }

    // Adjust thresholds based on volatility from configuration
    let (rsi_oversold, rsi_overbought, momentum_threshold) = if annual_vol > config.volatility_thresholds.high_vol_threshold {
        // High volatility: aggressive thresholds
        (config.thresholds.high_vol.rsi_oversold,
         config.thresholds.high_vol.rsi_overbought,
         config.thresholds.high_vol.momentum_threshold)
    } else if annual_vol > config.volatility_thresholds.medium_vol_threshold {
        // Medium volatility: moderate thresholds
        (config.thresholds.medium_vol.rsi_oversold,
         config.thresholds.medium_vol.rsi_overbought,
         config.thresholds.medium_vol.momentum_threshold)
    } else {
        // Low volatility: conservative (mostly hold)
        return Signal::Hold;
    };

    // Calculate RSI
    let rsi = match calculate_rsi(prices, config.rsi_period) {
        Some(r) => r,
        None => return Signal::Hold,
    };

    // Calculate momentum (5-day change)
    let momentum = if prices.len() >= config.momentum_period {
        (current_price - prices[prices.len() - config.momentum_period]) / prices[prices.len() - config.momentum_period]
    } else {
        0.0
    };

    // BUY: Strong upward momentum + not overbought
    if momentum > momentum_threshold && rsi < rsi_overbought {
        return Signal::Buy;
    }

    // SELL: Downward momentum + not oversold (to exit positions)
    if momentum < -momentum_threshold && rsi > rsi_oversold {
        return Signal::Sell;
    }

    Signal::Hold
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== ALPACA PAPER TRADING - LIVE STRATEGY TEST ===\n");

    // Load configuration
    let config_content = fs::read_to_string("config/paper_trading_config.json")
        .map_err(|e| format!("Failed to read paper trading config file: {}", e))?;
    let config: PaperTradingConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse paper trading config file: {}", e))?;

    println!("📋 Loaded paper trading configuration from config/paper_trading_config.json");

    // Debug: Show what keys we're using
    let api_key = std::env::var("ALPACA_API_KEY")
        .map_err(|_| "ALPACA_API_KEY not set")?;
    let api_secret = std::env::var("ALPACA_API_SECRET")
        .map_err(|_| "ALPACA_API_SECRET not set")?;

    println!("🔑 Using API Key: {}", api_key);

    // Initialize Alpaca client - create directly instead of from_env()
    let client = AlpacaClient::new(api_key, api_secret);

    // Load enabled symbols from stocks.json
    let symbols = load_enabled_stocks().expect("Failed to load stocks from config/stocks.json");

    // Get account info
    let account = client.get_account().await?;
    println!("📊 Account Status:");
    println!("   Account Number: {}", account.account_number);
    println!("   Cash: ${}", account.cash);
    println!("   Buying Power: ${}", account.buying_power);
    println!("   Portfolio Value: ${}", account.portfolio_value);
    println!();

    // Get current positions
    let positions = client.get_positions().await?;
    let mut position_map: HashMap<String, f64> = HashMap::new();
    
    if !positions.is_empty() {
        println!("📈 Current Positions:");
        for pos in &positions {
            println!(
                "   {}: {:.0} shares @ ${:.2} | P&L: ${:.2}",
                pos.symbol, pos.qty, pos.avg_entry_price, pos.unrealized_pl
            );
            position_map.insert(pos.symbol.clone(), pos.qty.parse().unwrap_or(0.0));
        }
        println!();
    }

    // Fetch historical data and generate signals
    println!("🔍 Analyzing Signals...\n");

    let mut signals_generated = 0;
    let position_size = config.trading.position_size_shares;

    for symbol in symbols {
        println!("--- {} ---", symbol);

        // Get 60 days of daily bars to ensure we have at least 30 trading days
        let end_time = chrono::Utc::now();
        let start_time = end_time - chrono::Duration::days(60);
        
        let start_str = start_time.format("%Y-%m-%d").to_string();
        let end_str = end_time.format("%Y-%m-%d").to_string();
        
        let bars = match client
            .get_bars(
                &symbol,
                "1Day",
                &start_str,
                Some(&end_str),
                Some(60),
            )
            .await
        {
            Ok(b) => b,
            Err(e) => {
                println!("   ⚠️  Failed to get historical data: {}", e);
                continue;
            }
        };

        if bars.is_empty() {
            println!("   ⚠️  No historical data available");
            continue;
        }

        println!("   Bars received: {}", bars.len());

        // Extract closing prices
        let prices: Vec<f64> = bars.iter().map(|b| b.c).collect();

        // Get current price from latest snapshot
        let snapshot = match client.get_snapshot(&symbol).await {
            Ok(s) => s,
            Err(e) => {
                println!("   ⚠️  Failed to get current price: {}", e);
                continue;
            }
        };

        let current_price = snapshot
            .latest_trade
            .as_ref()
            .map(|t| t.price)
            .unwrap_or_else(|| *prices.last().unwrap());

        // Calculate volatility
        let annual_vol = calculate_volatility(&prices).unwrap_or(0.0);
        println!("   Volatility: {:.1}%", annual_vol * 100.0);
        println!("   Current Price: ${:.2}", current_price);

        // Generate signal
        let signal = generate_signal(&prices, current_price, annual_vol, &config.signals);
        println!("   Signal: {:?}", signal);

        // Check if we have a position
        let has_position = position_map.contains_key(&symbol);

        // Execute trade based on signal
        match signal {
            Signal::Buy if !has_position => {
                println!("   🟢 BUY SIGNAL - Submitting market order...");
                
                let order = OrderRequest {
                    symbol: symbol.to_string(),
                    qty: position_size,
                    side: OrderSide::Buy,
                    r#type: OrderType::Market,
                    time_in_force: TimeInForce::Day,
                    limit_price: None,
                    stop_price: None,
                    extended_hours: None,
                    client_order_id: None,
                };

                match client.submit_order(&order).await {
                    Ok(result) => {
                        println!("   ✅ Order submitted! ID: {}", result.id);
                        signals_generated += 1;
                    }
                    Err(e) => println!("   ❌ Order failed: {}", e),
                }
            }
            Signal::Sell if has_position => {
                println!("   🔴 SELL SIGNAL - Closing position...");
                
                match client.close_position(&symbol).await {
                    Ok(result) => {
                        println!("   ✅ Position closed! Order ID: {}", result.id);
                        signals_generated += 1;
                    }
                    Err(e) => println!("   ❌ Close failed: {}", e),
                }
            }
            Signal::Buy => {
                println!("   💼 Already have position - holding");
            }
            Signal::Sell => {
                println!("   ⚪ No position to sell");
            }
            Signal::Hold => {
                println!("   ⏸️  HOLD - No action");
            }
        }

        println!();
    }

    // Summary
    println!("=== SUMMARY ===");
    println!("Signals Generated: {}", signals_generated);
    println!("View live results: https://app.alpaca.markets/paper/dashboard");
    println!();
    println!("💡 Tip: Run this script periodically (every 5-30 min) to trade on live signals");
    println!("💡 Or modify to run continuously with a loop + sleep");

    Ok(())
}
