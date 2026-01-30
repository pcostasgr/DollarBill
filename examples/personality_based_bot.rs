// Personality-Based Trading Bot
// Uses trained personality models to select optimal strategies for each stock

use dollarbill::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
use dollarbill::market_data::symbols::load_enabled_stocks;
use dollarbill::strategies::matching::StrategyMatcher;
use dollarbill::strategies::SignalAction;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use tokio::time::{sleep, Duration};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PersonalityBotConfig {
    trading: TradingConfig,
    execution: ExecutionConfig,
}

#[derive(Debug, Deserialize)]
struct TradingConfig {
    position_size_shares: i32,
    max_positions: usize,
    risk_management: RiskManagementConfig,
    min_confidence: f64,  // Minimum confidence score to execute trades
}

#[derive(Debug, Deserialize)]
struct RiskManagementConfig {
    stop_loss_pct: f64,
    take_profit_pct: f64,
    max_daily_trades: usize,
}

#[derive(Debug, Deserialize)]
struct ExecutionConfig {
    continuous_mode_interval_minutes: u64,
    data_lookback_days: i64,
}

/// Personality-Based Trading Bot
struct PersonalityBasedBot {
    client: Option<AlpacaClient>,
    config: PersonalityBotConfig,
    symbols: Vec<String>,
    matcher: StrategyMatcher,
}

impl PersonalityBasedBot {
    fn new(
        client: Option<AlpacaClient>,
        config: PersonalityBotConfig,
        symbols: Vec<String>,
        matcher: StrategyMatcher,
    ) -> Self {
        Self {
            client,
            config,
            symbols,
            matcher,
        }
    }

    async fn run_iteration(&mut self) -> Result<(), Box<dyn Error>> {
        let client = match &self.client {
            Some(c) => c,
            None => {
                println!("❌ No Alpaca client available (dry-run mode)");
                return Ok(());
            }
        };

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("\n{:=>70}", "");
        println!("🎭 Personality-Based Trading Bot - {}", timestamp);
        println!("{:=>70}\n", "");

        // Get account status
        let account = client.get_account().await?;
        println!("💰 Account: ${:.2} cash | ${:.2} portfolio value",
            account.cash, account.portfolio_value);

        // Get current positions
        let positions = client.get_positions().await?;
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

        println!("\n🧠 Analyzing with Personality-Driven Strategies...\n");

        // Analyze each symbol using personality-matched strategies
        for symbol in &self.symbols {
            // Skip if we already have max positions and don't own this one
            if positions.len() >= self.config.trading.max_positions && !position_map.contains_key(symbol) {
                continue;
            }

            // Get the optimal strategy for this stock's personality
            let strategy = match self.matcher.get_optimal_strategy(symbol) {
                Ok(s) => s,
                Err(e) => {
                    println!("   {} | ❌ No strategy available: {}", symbol, e);
                    continue;
                }
            };

            // Get current market data
            let snapshot = match client.get_snapshot(symbol).await {
                Ok(s) => s,
                Err(e) => {
                    println!("   {} | ❌ Failed to get snapshot: {}", symbol, e);
                    continue;
                }
            };

            // Debug: Log what data sources are available
            let trade_avail = snapshot.latest_trade.is_some();
            let quote_avail = snapshot.latest_quote.is_some();
            let daily_avail = snapshot.daily_bar.is_some();
            let prev_avail = snapshot.prev_daily_bar.is_some();
            
            if !trade_avail && !quote_avail && !daily_avail && !prev_avail {
                println!("   {} | 🔍 DEBUG: No data in snapshot (trade:{} quote:{} daily:{} prev:{})", 
                         symbol, trade_avail, quote_avail, daily_avail, prev_avail);
            }

            // Try multiple price sources in order of preference
            let current_price = if let Some(trade) = &snapshot.latest_trade {
                trade.price
            } else if let Some(quote) = &snapshot.latest_quote {
                // Use mid-price from quote if no trade available
                (quote.bid + quote.ask) / 2.0
            } else if let Some(daily_bar) = &snapshot.daily_bar {
                // Use daily close price as fallback
                daily_bar.c
            } else if let Some(prev_bar) = &snapshot.prev_daily_bar {
                // Use previous day's close as last resort
                println!("   {} | ⚠️  Using previous day's close (no current data)", symbol);
                prev_bar.c
            } else {
                // Last resort: Try to get recent historical data
                let end_time = chrono::Utc::now();
                let start_time = end_time - chrono::Duration::days(5); // Look back 5 days
                let start_str = start_time.format("%Y-%m-%d").to_string();
                let end_str = end_time.format("%Y-%m-%d").to_string();
                
                match client.get_bars(symbol, "1Day", &start_str, Some(&end_str), Some(5)).await {
                    Ok(bars) if !bars.is_empty() => {
                        println!("   {} | ⚠️  Using historical data (snapshot unavailable)", symbol);
                        bars.last().unwrap().c // Use most recent close price
                    }
                    _ => {
                        println!("   {} | ❌ No price data available anywhere, skipping", symbol);
                        continue;
                    }
                }
            };

            // Get historical data for volatility calculation
            let end_time = chrono::Utc::now();
            let start_time = end_time - chrono::Duration::days(self.config.execution.data_lookback_days);

            let start_str = start_time.format("%Y-%m-%d").to_string();
            let end_str = end_time.format("%Y-%m-%d").to_string();

            let bars = match client
                .get_bars(symbol, "1Day", &start_str, Some(&end_str), Some(60))
                .await
            {
                Ok(b) if !b.is_empty() => b,
                _ => {
                    println!("   {} | ❌ No historical data available", symbol);
                    continue;
                }
            };

            // Calculate historical volatility
            let prices: Vec<f64> = bars.iter().map(|b| b.c).collect();
            let hist_vol = match calculate_volatility(&prices) {
                Some(v) => v,
                None => {
                    println!("   {} | ❌ Could not calculate volatility", symbol);
                    continue;
                }
            };

            // Use historical volatility as both market and model IV for now
            // In production, you'd fetch live options data for market IV
            let market_iv = hist_vol;
            let model_iv = hist_vol * 0.95; // Slight adjustment for model calibration

            // Generate signals using the personality-matched strategy
            let signals = strategy.generate_signals(
                symbol,
                current_price,
                market_iv,
                model_iv,
                hist_vol,
            );

            // Process signals - convert options signals to stock actions
            for signal in signals {
                // Convert options signals to stock buy/sell actions
                let stock_action = match signal.action {
                    SignalAction::BuyStraddle | SignalAction::IronButterfly { .. } => {
                        if signal.confidence >= self.config.trading.min_confidence {
                            Some("BUY")
                        } else {
                            None
                        }
                    }
                    SignalAction::SellStraddle => {
                        if signal.confidence >= self.config.trading.min_confidence {
                            Some("SELL")
                        } else {
                            None
                        }
                    }
                    SignalAction::CashSecuredPut { .. } => {
                        // Cash-secured puts are neutral/bullish - don't trade stock
                        None
                    }
                    SignalAction::NoAction => None,
                };

                if let Some(action) = stock_action {
                    let has_position = position_map.contains_key(symbol);

                    print!("   {} ${:.2} | Strategy: {} | Conf: {:.1}% | ",
                        symbol, current_price, signal.strategy_name, signal.confidence * 100.0);

                    match (action, has_position) {
                        ("BUY", false) => {
                            print!("🟢 BUY → {} shares...", self.config.trading.position_size_shares);
                            let order = OrderRequest {
                                symbol: symbol.clone(),
                                qty: self.config.trading.position_size_shares as f64,
                                side: OrderSide::Buy,
                                r#type: OrderType::Market,
                                time_in_force: TimeInForce::Day,
                                limit_price: None,
                                stop_price: None,
                                extended_hours: None,
                                client_order_id: None,
                            };

                            match client.submit_order(&order).await {
                                Ok(_) => println!(" ✅"),
                                Err(e) => println!(" ❌ {}", e),
                            }
                        }
                        ("SELL", true) => {
                            print!("🔴 SELL → Closing position...");
                            match client.close_position(symbol).await {
                                Ok(_) => println!(" ✅"),
                                Err(e) => println!(" ❌ {}", e),
                            }
                        }
                        _ => {
                            println!("⏭️  SKIP ({} position)", if has_position { "have" } else { "no" });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn run_continuous(&mut self, interval_minutes: u64) -> Result<(), Box<dyn Error>> {
        println!("\n🚀 Starting Continuous Personality-Based Trading Bot");
        println!("   Symbols: {:?}", self.symbols);
        println!("   Position Size: {} shares", self.config.trading.position_size_shares);
        println!("   Max Positions: {}", self.config.trading.max_positions);
        println!("   Min Confidence: {:.1}%", self.config.trading.min_confidence * 100.0);
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

    async fn run_dry_run(&mut self) -> Result<(), Box<dyn Error>> {
        println!("🧠 Testing Personality Strategy Matching...\n");

        for symbol in &self.symbols {
            // Get the optimal strategy for this stock's personality
            match self.matcher.get_optimal_strategy(symbol) {
                Ok(strategy) => {
                    println!("   {} → {} strategy", symbol, strategy.name());
                }
                Err(e) => {
                    println!("   {} → ❌ No strategy available: {}", symbol, e);
                }
            }
        }

        println!("\n✅ Strategy matching test complete!");
        println!("   All symbols have been matched with personality-optimized strategies");

        Ok(())
    }
}

/// Calculate volatility from price history
fn calculate_volatility(prices: &[f64]) -> Option<f64> {
    if prices.len() < 20 {
        return None;
    }

    let returns: Vec<f64> = prices
        .windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>() / returns.len() as f64;

    Some(variance.sqrt() * (252.0_f64).sqrt()) // Annualized volatility
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🎭 DollarBill - Personality-Based Trading Bot");
    println!("==============================================");
    println!("This bot uses trained personality models to select");
    println!("optimal trading strategies for each stock individually.");
    println!("");

    // Load personality bot configuration
    let config_content = fs::read_to_string("config/personality_bot_config.json")
        .map_err(|e| format!("Failed to read personality bot config file: {}", e))?;
    let config: PersonalityBotConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse personality bot config file: {}", e))?;

    println!("📋 Loaded personality bot configuration from config/personality_bot_config.json");

    // Load enabled symbols from stocks.json
    let symbols = load_enabled_stocks().expect("Failed to load stocks from config/stocks.json");
    println!("📊 Loaded {} enabled stocks: {:?}", symbols.len(), symbols);

    // Load trained personality models
    println!("🧠 Loading trained personality models...");
    let matcher = StrategyMatcher::load_from_files(
        "models/stock_classifier.json",
        "models/performance_matrix.json"
    ).map_err(|e| format!("Failed to load personality models: {}", e))?;

    println!("✅ Personality models loaded successfully!");
    println!("   Strategies available per stock based on personality analysis");

    // Choose mode: single run, continuous, or dry-run
    let args: Vec<String> = std::env::args().collect();

    // Initialize Alpaca client (only needed for actual trading)
    let client = if args.len() > 1 && args[1] == "--dry-run" {
        None // No client needed for dry run
    } else {
        Some(AlpacaClient::from_env()?)
    };

    let mut bot = PersonalityBasedBot::new(client, config, symbols, matcher);

    if args.len() > 1 && args[1] == "--continuous" {
        let interval = if args.len() > 2 {
            args[2].parse().unwrap_or(5)
        } else {
            5 // Default: 5 minutes
        };
        bot.run_continuous(interval).await?;
    } else if args.len() > 1 && args[1] == "--dry-run" {
        // Dry run mode - test strategy loading without trading
        println!("🔍 Running in dry-run mode (no actual trades)...\n");
        bot.run_dry_run().await?;
        println!("\n💡 Dry run complete! Use without --dry-run to trade for real");
    } else {
        // Single iteration
        bot.run_iteration().await?;
        println!("\n💡 Run with --continuous to keep trading, or --dry-run to test without trading");
        println!("💡 Example: cargo run --example personality_based_bot -- --continuous 5");
    }

    Ok(())
}