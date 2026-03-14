// Personality-Based Trading Bot
// Uses trained personality models to select optimal strategies for each stock

use dollarbill::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
use dollarbill::market_data::symbols::load_enabled_stocks;
use dollarbill::strategies::matching::StrategyMatcher;
use dollarbill::strategies::SignalAction;
use dollarbill::portfolio::{PortfolioManager, PortfolioConfig, SizingMethod, AllocationMethod, RiskLimits};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::Write;
use tokio::time::{sleep, Duration};
use tokio::select;
use serde::Deserialize;

/// Append a single trade-decision line to the audit log CSV.
/// Format: timestamp,symbol,action,shares,price,order_id,fill_status,reason
fn audit_log(
    symbol: &str,
    action: &str,
    shares: f64,
    price: f64,
    order_id: &str,
    fill_status: &str,
    reason: &str,
) {
    let path = "trade_audit.csv";
    let needs_header = !std::path::Path::new(path).exists();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if needs_header {
            let _ = writeln!(f, "timestamp,symbol,action,shares,price,order_id,fill_status,reason");
        }
        let ts = chrono::Utc::now().to_rfc3339();
        let _ = writeln!(f, "{ts},{symbol},{action},{shares:.4},{price:.4},{order_id},{fill_status},{reason}");
    }
}

#[derive(Debug, Deserialize)]
struct PersonalityBotConfig {
    trading: TradingConfig,
    execution: ExecutionConfig,
    portfolio: Option<PortfolioSettings>,  // Optional portfolio management settings
}

#[derive(Debug, Deserialize, Clone)]
struct PortfolioSettings {
    enabled: bool,
    initial_capital: f64,
    max_risk_per_trade: f64,
    max_position_pct: f64,
    sizing_method: String,  // "VolatilityBased", "FixedFractional", "KellyCriterion", etc.
    max_portfolio_delta: f64,
    max_concentration_pct: f64,
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
    #[serde(default = "default_max_daily_drawdown_pct")]
    max_daily_drawdown_pct: f64,
}

fn default_max_daily_drawdown_pct() -> f64 {
    0.05 // 5% default
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
    portfolio_manager: Option<PortfolioManager>,
    /// Trades submitted this calendar day (reset at the top of each new day).
    daily_trades_taken: usize,
    /// The calendar date when `daily_trades_taken` was last reset (YYYY-MM-DD).
    daily_trades_date: String,
}

impl PersonalityBasedBot {
    fn new(
        client: Option<AlpacaClient>,
        config: PersonalityBotConfig,
        symbols: Vec<String>,
        matcher: StrategyMatcher,
    ) -> Self {
        // Initialize portfolio manager if enabled in config
        let portfolio_manager = if let Some(ref portfolio_settings) = config.portfolio {
            if portfolio_settings.enabled {
                let sizing_method = match portfolio_settings.sizing_method.as_str() {
                    "VolatilityBased" => SizingMethod::VolatilityBased,
                    "KellyCriterion" => SizingMethod::KellyCriterion,
                    "RiskParity" => SizingMethod::RiskParity,
                    "FixedDollar" => SizingMethod::FixedDollar(5000.0),
                    _ => SizingMethod::FixedFractional(5.0),  // Default
                };
                
                Some(PortfolioManager::new(PortfolioConfig {
                    initial_capital: portfolio_settings.initial_capital,
                    max_risk_per_trade: portfolio_settings.max_risk_per_trade,
                    max_position_pct: portfolio_settings.max_position_pct,
                    sizing_method,
                    allocation_method: AllocationMethod::RiskParity,
                    risk_limits: RiskLimits {
                        max_portfolio_delta: portfolio_settings.max_portfolio_delta,
                        max_concentration_pct: portfolio_settings.max_concentration_pct,
                        ..Default::default()
                    },
                }))
            } else {
                None
            }
        } else {
            None
        };
        
        Self {
            client,
            config,
            symbols,
            matcher,
            portfolio_manager,
            daily_trades_taken: 0,
            daily_trades_date: String::new(),
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
        let equity: f64 = account.equity.parse().unwrap_or(0.0);
        let last_equity: f64 = account.last_equity.parse().unwrap_or(equity);
        let buying_power: f64 = account.buying_power.parse().unwrap_or(0.0);
        println!("💰 Account: ${:.2} cash | ${:.2} equity | ${:.2} buying power",
            account.cash, equity, buying_power);

        // Sanity-check: if equity is exactly 0.0 something went wrong with the API
        // response (NaN, empty field, etc.) — halt to avoid trading on garbage data.
        if equity == 0.0 {
            eprintln!("🛑 SAFETY HALT: account equity parsed to $0.00 — possible API data error. Aborting iteration.");
            audit_log("SYSTEM", "SAFETY_HALT", 0.0, 0.0, "", "halted", "equity=0 parse anomaly");
            return Ok(());
        }

        // ── Market-hours check ──
        let clock = client.get_clock().await?;
        if !clock.is_open {
            println!("🕐 Market is closed. Next open: {}  Skipping iteration.", clock.next_open);
            return Ok(());
        }

        // ── PDT protection ──
        let account_pdt = account.pattern_day_trader;
        let daytrade_count = account.daytrade_count;
        if account_pdt {
            println!("⚠️  Account is flagged as Pattern Day Trader (PDT). Day-trade count: {}", daytrade_count);
        }
        // Block all trading if PDT-flagged AND equity < $25,000
        if account_pdt && equity < 25_000.0 {
            println!("🛑 PDT BLOCK: PDT account with equity ${:.2} < $25,000 — halting trades to avoid lockout", equity);
            return Ok(());
        }

        // ── Circuit Breaker: daily drawdown kill switch ──
        let daily_drawdown = if last_equity > 0.0 {
            (last_equity - equity) / last_equity
        } else {
            0.0
        };
        let max_dd = self.config.trading.risk_management.max_daily_drawdown_pct;
        if daily_drawdown >= max_dd {
            println!("🛑 CIRCUIT BREAKER: Daily drawdown {:.2}% exceeds limit {:.2}% — halting trades",
                daily_drawdown * 100.0, max_dd * 100.0);
            return Ok(());
        }

        // ── Sync portfolio manager with live account ──
        if let Some(ref mut manager) = self.portfolio_manager {
            manager.sync_from_account(equity, buying_power);
        }

        // ── Daily trade counter: reset on a new calendar day ──
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if self.daily_trades_date != today {
            self.daily_trades_date = today;
            self.daily_trades_taken = 0;
        }
        let max_daily = self.config.trading.risk_management.max_daily_trades;
        if self.daily_trades_taken >= max_daily {
            println!("🛑 DAILY TRADE LIMIT: {}/{} trades used today — skipping new entries",
                self.daily_trades_taken, max_daily);
            // Still run the SL/TP checks below — just no new entries
        }

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

        // 🔥 ACTIVE POSITION MANAGEMENT - Check stop-losses and take-profits
        println!("\n🔍 Checking Position Management (Stop-Loss/Take-Profit)...");
        for pos in &positions {
            let symbol = &pos.symbol;
            let entry_price: f64 = pos.avg_entry_price.parse().unwrap_or(0.0);

            // Use current_price from position data; if missing or zero, skip this
            // position's SL/TP check rather than silently using a stale price.
            let current_price: f64 = match pos.current_price.parse::<f64>() {
                Ok(p) if p > 0.0 => p,
                _ => {
                    println!("   ⚠️  {} | Skipping SL/TP — current price unavailable or zero", symbol);
                    continue;
                }
            };

            let _unrealized_pl: f64 = pos.unrealized_pl.parse().unwrap_or(0.0);
            let pl_pct = if entry_price > 0.0 { 
                (current_price - entry_price) / entry_price 
            } else { 
                0.0 
            };

            let stop_loss_threshold = -self.config.trading.risk_management.stop_loss_pct;
            let take_profit_threshold = self.config.trading.risk_management.take_profit_pct;

            if pl_pct <= stop_loss_threshold {
                // STOP LOSS TRIGGERED
                print!("   🛑 STOP LOSS: {} | Entry: ${:.2} → Current: ${:.2} | Loss: {:.1}% | ",
                    symbol, entry_price, current_price, pl_pct * 100.0);

                // Retry the close once — a single transient failure must not leave
                // a losing position open.
                let close_result = match client.close_position(symbol).await {
                    Ok(ord) => Ok(ord),
                    Err(_) => client.close_position(symbol).await,
                };
                match close_result {
                    Ok(ord) => {
                        println!("✅ Position closed ({})", ord.id);
                        audit_log(symbol, "STOP_LOSS", 0.0, current_price, &ord.id, &ord.status, "stop-loss triggered");
                    }
                    Err(e) => {
                        eprintln!("🚨 CRITICAL: Failed to close stop-loss position {}: {}", symbol, e);
                        audit_log(symbol, "STOP_LOSS_FAILED", 0.0, current_price, "", "error", &e.to_string());
                    }
                }
                continue; // Skip further analysis for this symbol
            } else if pl_pct >= take_profit_threshold {
                // TAKE PROFIT TRIGGERED
                print!("   💰 TAKE PROFIT: {} | Entry: ${:.2} → Current: ${:.2} | Gain: {:.1}% | ",
                    symbol, entry_price, current_price, pl_pct * 100.0);

                match client.close_position(symbol).await {
                    Ok(ord) => {
                        println!("✅ Position closed ({})", ord.id);
                        audit_log(symbol, "TAKE_PROFIT", 0.0, current_price, &ord.id, &ord.status, "take-profit triggered");
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to close take-profit position {}: {}", symbol, e);
                        audit_log(symbol, "TAKE_PROFIT_FAILED", 0.0, current_price, "", "error", &e.to_string());
                    }
                }
                continue; // Skip further analysis for this symbol
            } else {
                // Position within acceptable range
                println!("   ✅ {} | P&L: {:.1}% (Target: +{:.0}% / Stop: -{:.0}%)", 
                    symbol, pl_pct * 100.0, 
                    take_profit_threshold * 100.0,
                    stop_loss_threshold.abs() * 100.0);
            }
        }

        println!("\n🧠 Analyzing with Personality-Driven Strategies...\n");

        // Fetch open orders to avoid duplicate submissions
        let open_orders = client.get_orders(Some("open")).await.unwrap_or_default();
        let open_order_symbols: std::collections::HashSet<String> = open_orders.iter()
            .map(|o| o.symbol.clone())
            .collect();
        if !open_orders.is_empty() {
            println!("📋 Open orders ({}): {:?}", open_orders.len(),
                open_order_symbols.iter().collect::<Vec<_>>());
        }

        // Analyze each symbol using personality-matched strategies
        for symbol in &self.symbols {
            // Skip if we already have max positions and don't own this one
            if positions.len() >= self.config.trading.max_positions && !position_map.contains_key(symbol) {
                continue;
            }

            // Skip new entries if daily trade limit is reached
            if self.daily_trades_taken >= max_daily && !position_map.contains_key(symbol) {
                continue;
            }

            // Skip symbols that already have an open (pending) order
            if open_order_symbols.contains(symbol) {
                println!("   {} | ⏭️  Skipping — already has an open order", symbol);
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

            // Process signals - convert options signals to stock actions.
            // `acted` ensures at most one order is submitted per symbol per iteration,
            // preventing double-buys when multiple signals fire simultaneously.
            let mut acted = false;
            for signal in signals {
                if acted { break; }
                println!("   {} | 🔍 SIGNAL: {} - Confidence: {:.1}% (min: {:.1}%)", 
                    symbol, signal.strategy_name, signal.confidence * 100.0, self.config.trading.min_confidence * 100.0);
                
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
                        if signal.confidence >= self.config.trading.min_confidence {
                            println!("💰 EXECUTING CASH-SECURED PUT: {} - Strike: {:.1}% OTM",
                                symbol, 5.0);
                            Some("CASH_PUT")
                        } else {
                            None
                        }
                    }
                    SignalAction::NoAction => None,
                    _ => None, // Other signals not handled by this bot
                };

                if let Some(action) = stock_action {
                    let has_position = position_map.contains_key(symbol);
                    acted = true;

                    print!("   {} ${:.2} | Strategy: {} | Conf: {:.1}% | ",
                        symbol, current_price, signal.strategy_name, signal.confidence * 100.0);

                    match (action, has_position) {
                        ("BUY", false) => {
                            // Calculate position size using portfolio manager if enabled
                            let position_size = if let Some(ref manager) = self.portfolio_manager {
                                // Use intelligent sizing based on volatility and risk
                                let contracts = manager.calculate_position_size(
                                    current_price,
                                    hist_vol,
                                    None,  // Could track win_rate from historical trades
                                    None,  // avg_win
                                    None,  // avg_loss
                                );
                                
                                // Check if we can take this position (portfolio risk limits)
                                let decision = manager.can_take_position(
                                    &signal.strategy_name,
                                    current_price,
                                    hist_vol,
                                    contracts,
                                );
                                
                                if !decision.can_trade {
                                    println!("❌ REJECTED by portfolio manager:");
                                    for warning in &decision.risk_warnings {
                                        println!("   ⚠️  {}", warning);
                                    }
                                    continue;  // Skip this trade
                                }
                                
                                contracts
                            } else {
                                // Fallback to fixed size if portfolio management disabled
                                self.config.trading.position_size_shares
                            };
                            
                            // Zero/negative size guard: portfolio sizer can return 0
                            // on small accounts or when limits are too tight.
                            if position_size <= 0 {
                                println!("⏭️  SKIP — position sizer returned {} shares (too small)", position_size);
                                audit_log(symbol, "BUY_SKIP", 0.0, current_price, "", "skipped", "position_size <= 0");
                                acted = false; // don't block later signals
                                continue;
                            }

                            print!("🟢 BUY → {} shares...", position_size);

                            // Buying power guard
                            let order_cost = position_size as f64 * current_price;
                            if order_cost > buying_power {
                                println!("❌ Insufficient buying power: need ${:.2}, have ${:.2}",
                                    order_cost, buying_power);
                                continue;
                            }

                            let order = OrderRequest {
                                symbol: symbol.clone(),
                                qty: position_size as f64,
                                side: OrderSide::Buy,
                                r#type: OrderType::Market,
                                time_in_force: TimeInForce::Day,
                                limit_price: None,
                                stop_price: None,
                                extended_hours: None,
                                client_order_id: None,
                            };

                            match client.submit_order(&order).await {
                                Ok(submitted) => {
                                    println!(" ✅ submitted ({})", submitted.id);
                                    print!("   ⏳ Waiting for fill...");
                                    match client.await_order_fill(&submitted.id).await {
                                        Ok(filled) => {
                                            let fill_px = filled.filled_avg_price.as_deref().unwrap_or("?");
                                            println!(" {} @ ${} ({} shares filled)",
                                                filled.status, fill_px, filled.filled_qty);
                                            audit_log(symbol, "BUY", position_size as f64,
                                                current_price, &filled.id, &filled.status, &signal.strategy_name);
                                            self.daily_trades_taken += 1;
                                        }
                                        Err(e) => println!(" ⚠️  Fill poll error: {}", e),
                                    }
                                }
                                Err(e) => {
                                    println!(" ❌ {}", e);
                                    audit_log(symbol, "BUY_FAILED", position_size as f64,
                                        current_price, "", "error", &e.to_string());
                                }
                            }
                        }
                        ("CASH_PUT", false) => {
                            // Real cash-secured puts require Alpaca's options API and a
                            // separate options-trading approval. Skipping until options
                            // support is implemented.
                            println!("⏭️  CASH-SECURED PUT signal skipped — options API not yet implemented");
                            audit_log(symbol, "CSP_SKIPPED", 0.0, current_price,
                                "", "skipped", "options API not implemented");
                        }
                        ("SELL", true) => {
                            print!("🔴 SELL → Closing position...");
                            match client.close_position(symbol).await {
                                Ok(closed) => {
                                    println!(" ✅ ({})", closed.id);
                                    audit_log(symbol, "SELL", 0.0, current_price,
                                        &closed.id, &closed.status, &signal.strategy_name);
                                }
                                Err(e) => {
                                    println!(" ❌ {}", e);
                                    audit_log(symbol, "SELL_FAILED", 0.0, current_price,
                                        "", "error", &e.to_string());
                                }
                            }
                        }
                        ("SELL", false) => {
                            println!("⚪ SELL SIGNAL but no position to close");
                        }
                        ("BUY", true) => {
                            // Add to existing position if signal is strong enough (>20% confidence)
                            if signal.confidence > 0.20 && self.daily_trades_taken < max_daily {
                                // Use portfolio manager sizing for adds too; fall back to config value
                                let add_size = if let Some(ref manager) = self.portfolio_manager {
                                    let s = manager.calculate_position_size(current_price, hist_vol, None, None, None);
                                    if s > 0 { s } else { self.config.trading.position_size_shares }
                                } else {
                                    self.config.trading.position_size_shares
                                };

                                print!("🟢 ADD TO POSITION → +{} shares...", add_size);

                                // Buying power guard
                                let order_cost = add_size as f64 * current_price;
                                if order_cost > buying_power {
                                    println!(" ❌ Insufficient buying power: need ${:.2}, have ${:.2}",
                                        order_cost, buying_power);
                                    continue;
                                }

                                let order = OrderRequest {
                                    symbol: symbol.clone(),
                                    qty: add_size as f64,
                                    side: OrderSide::Buy,
                                    r#type: OrderType::Market,
                                    time_in_force: TimeInForce::Day,
                                    limit_price: None,
                                    stop_price: None,
                                    extended_hours: None,
                                    client_order_id: None,
                                };

                                match client.submit_order(&order).await {
                                    Ok(submitted) => {
                                        println!(" ✅ submitted ({})", submitted.id);
                                        print!("   ⏳ Waiting for fill...");
                                        match client.await_order_fill(&submitted.id).await {
                                            Ok(filled) => {
                                                let fill_px = filled.filled_avg_price.as_deref().unwrap_or("?");
                                                println!(" {} @ ${} ({} shares filled)",
                                                    filled.status, fill_px, filled.filled_qty);
                                                audit_log(symbol, "ADD",
                                                    add_size as f64,
                                                    current_price, &filled.id, &filled.status,
                                                    &signal.strategy_name);
                                            self.daily_trades_taken += 1;
                                            }
                                            Err(e) => println!(" ⚠️  Fill poll error: {}", e),
                                        }
                                    }
                                    Err(e) => {
                                        println!(" ❌ {}", e);
                                        audit_log(symbol, "ADD_FAILED",
                                            self.config.trading.position_size_shares as f64,
                                            current_price, "", "error", &e.to_string());
                                    }
                                }
                            } else {
                                println!("⏭️  SKIP (have position) - Buy signal not strong enough ({:.1}% < 20%)", signal.confidence * 100.0);
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
        println!("   Max Daily Trades: {}", self.config.trading.risk_management.max_daily_trades);
        println!("\n   Press Ctrl+C to stop gracefully\n");

        let shutdown = tokio::signal::ctrl_c();
        tokio::pin!(shutdown);

        loop {
            if let Err(e) = self.run_iteration().await {
                eprintln!("❌ Error in iteration: {}", e);
            }

            println!("\n💤 Sleeping for {} minutes...", interval_minutes);
            select! {
                _ = sleep(Duration::from_secs(interval_minutes * 60)) => {}
                _ = &mut shutdown => {
                    println!("\n🛑 Shutdown signal received — closing all open orders and exiting cleanly.");
                    if let Some(ref client) = self.client {
                        match client.cancel_all_orders().await {
                            Ok(cancelled) => println!("   Cancelled {} open orders.", cancelled.len()),
                            Err(e) => eprintln!("   ⚠️  Cancel-all failed: {}", e),
                        }
                    }
                    return Ok(());
                }
            }
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
        // Prefer interval from CLI arg; fall back to config value
        let interval = if args.len() > 2 {
            args[2].parse().unwrap_or(bot.config.execution.continuous_mode_interval_minutes)
        } else {
            bot.config.execution.continuous_mode_interval_minutes
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