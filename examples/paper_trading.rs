use black_scholes_rust::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
use std::collections::HashMap;
use std::error::Error;

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
) -> Signal {
    if prices.len() < 30 {
        return Signal::Hold;
    }

    // Adjust thresholds based on volatility
    let (rsi_oversold, rsi_overbought, momentum_threshold) = if annual_vol > 0.50 {
        // High volatility: aggressive thresholds
        (40.0, 60.0, 0.03)
    } else if annual_vol > 0.35 {
        // Medium volatility: moderate thresholds
        (35.0, 65.0, 0.02)
    } else {
        // Low volatility: conservative (mostly hold)
        (30.0, 70.0, 0.015)
    };

    // Calculate RSI
    let rsi = match calculate_rsi(prices, 14) {
        Some(r) => r,
        None => return Signal::Hold,
    };

    // Calculate momentum (5-day change)
    let momentum = if prices.len() >= 5 {
        (current_price - prices[prices.len() - 5]) / prices[prices.len() - 5]
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

    // Initialize Alpaca client
    let client = AlpacaClient::from_env()?;

    // Test symbols (your proven performers)
    let symbols = vec![
        "TSLA",  // High vol: 60.7%
        "NVDA",  // High vol: 52.4%
        "META",  // Medium vol: 42.1%
        "AMZN",  // Medium vol: 37.8%
        "GOOGL", // Medium vol: 35.2%
        // Skip AAPL and MSFT (low vol, poor backtest results)
    ];

    // Get account info
    let account = client.get_account().await?;
    println!("📊 Account Status:");
    println!("   Cash: ${:.2}", account.cash);
    println!("   Buying Power: ${:.2}", account.buying_power);
    println!("   Portfolio Value: ${:.2}", account.portfolio_value);
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
    let position_size = 5.0; // Number of shares per trade

    for symbol in symbols {
        println!("--- {} ---", symbol);

        // Get 30 days of daily bars for historical context
        let end_time = chrono::Utc::now();
        let start_time = end_time - chrono::Duration::days(30);
        
        let bars = match client
            .get_bars(
                symbol,
                "1Day",
                &start_time.to_rfc3339(),
                Some(&end_time.to_rfc3339()),
                Some(30),
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

        // Extract closing prices
        let prices: Vec<f64> = bars.iter().map(|b| b.c).collect();

        // Get current price from latest snapshot
        let snapshot = match client.get_snapshot(symbol).await {
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
        let signal = generate_signal(&prices, current_price, annual_vol);
        println!("   Signal: {:?}", signal);

        // Check if we have a position
        let has_position = position_map.contains_key(symbol);

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
                
                match client.close_position(symbol).await {
                    Ok(result) => {
                        println!("   ✅ Position closed! Order ID: {}", result.id);
                        signals_generated += 1;
                    }
                    Err(e) => println!("   ❌ Close failed: {}", e),
                }
            }
            Signal::Buy if has_position => {
                println!("   💼 Already have position - holding");
            }
            Signal::Sell if !has_position => {
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
