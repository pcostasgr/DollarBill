// Live trading WebSocket event loop extracted from main::cmd_trade.
// Called when `dollarbill trade --live` (or `--dry-run`) is invoked.

use crate::alpaca::AlpacaClient;
use crate::analysis::regime_detector::RegimeDetector;
use crate::config;
use crate::market_data::csv_loader::load_csv_closes;
use crate::models::bs_mod::compute_historical_vol;
use crate::models::heston::heston_start;
use crate::persistence;
use crate::portfolio::{PortfolioManager, PortfolioConfig};
use crate::strategies::{
    SignalAction, StrategyRegistry,
    momentum::MomentumStrategy,
    mean_reversion::MeanReversionStrategy,
    breakout::BreakoutStrategy,
    vol_arbitrage::VolatilityArbitrageStrategy,
    cash_secured_puts::CashSecuredPuts,
};
use crate::streaming;
use chrono::{Duration, NaiveDate, Utc};
use log::{info, warn, error};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

pub async fn run_live_bot(
    client: AlpacaClient,
    symbols: Vec<String>,
    equity: f64,
    buy_pwr: f64,
    dry_run: bool,
    store: persistence::TradeStore,
    persisted: Vec<persistence::PositionRecord>,
) {
    // ── Portfolio manager — shared risk gate for all orders ───────────────
    let mut pm = PortfolioManager::new(PortfolioConfig {
        initial_capital: equity,
        ..PortfolioConfig::default()
    });
    pm.sync_from_account(equity, buy_pwr);

    // ── Restore open positions from previous sessions ─────────────────────
    if !persisted.is_empty() {
        println!("\n📂  {} persisted position(s) restored:", persisted.len());
        for p in &persisted {
            println!("     {} qty={:.0} @ ${:.2}  [{}]",
                p.symbol, p.qty, p.entry_price,
                p.strategy.as_deref().unwrap_or("—"));
        }
    }

    // ── Reconcile SQLite state against live Alpaca positions ──────────────
    println!("\n🔄 Reconciling positions with Alpaca…");
    match client.get_positions().await {
        Ok(alpaca_pos) => {
            let alpaca_syms: HashSet<String> =
                alpaca_pos.iter().map(|p| p.symbol.clone()).collect();
            // Remove SQLite records absent from Alpaca (must have closed/expired)
            for p in &persisted {
                if !alpaca_syms.contains(&p.symbol) {
                    eprintln!("  ⚠  {} in SQLite but absent from Alpaca — removing stale record",
                        p.symbol);
                    let _ = store.close_position(&p.symbol).await;
                } else {
                    println!("  ✅ {} confirmed open in Alpaca", p.symbol);
                }
            }
            // Import Alpaca positions not yet tracked in SQLite
            let sqlite_syms: HashSet<String> =
                persisted.iter().map(|p| p.symbol.clone()).collect();
            for ap in &alpaca_pos {
                if !sqlite_syms.contains(&ap.symbol) {
                    println!("  📥 Importing {} from Alpaca into SQLite", ap.symbol);
                    let rec = persistence::PositionRecord {
                        symbol:            ap.symbol.clone(),
                        qty:               ap.qty.parse::<f64>().unwrap_or(0.0),
                        entry_price:       ap.avg_entry_price.parse::<f64>().unwrap_or(0.0),
                        entry_date:        "reconciled".to_string(),
                        strategy:          Some("reconciled".to_string()),
                        expires_at:        None,
                        premium_collected: None,
                    };
                    let _ = store.upsert_position(&rec).await;
                }
            }
        }
        Err(e) => eprintln!("⚠️  Position reconciliation skipped: {}", e),
    }

    // In-memory open-position guard (rebuilt from reconciled SQLite state).
    let reconciled = store.get_open_positions().await.unwrap_or_default();
    let mut open_syms: HashSet<String> =
        reconciled.iter().map(|p| p.symbol.clone()).collect();
    // Full position records in memory for close-logic lookups.
    let mut open_positions: HashMap<String, persistence::PositionRecord> =
        reconciled.into_iter().map(|p| (p.symbol.clone(), p)).collect();
    if !open_syms.is_empty() {
        println!("  [LOCK] {} open position(s) -- skipping duplicate entries",
            open_syms.len());
    }

    // ── Load runtime config ───────────────────────────────────────────────
    let bot_cfg = config::TradingBotConfigFile::load();
    info!("Bot config: min_confidence={:.2} max_daily_loss={:.1}% cooldown={}s \
profit_target={:.0}% stop_loss={:.0}% max_days={} vol_pct={:.0}%",
        bot_cfg.min_confidence,
        bot_cfg.max_daily_loss_pct * 100.0,
        bot_cfg.signal_cooldown_secs,
        bot_cfg.profit_target_pct  * 100.0,
        bot_cfg.stop_loss_pct      * 100.0,
        bot_cfg.max_position_days,
        bot_cfg.min_vol_percentile * 100.0);

    let signal_cooldown_secs  = bot_cfg.signal_cooldown_secs;
    let min_prices_for_hv     = bot_cfg.min_prices_for_hv;
    let max_price_buf         = bot_cfg.max_price_buf;
    let min_confidence        = bot_cfg.min_confidence;
    let profit_target_pct     = bot_cfg.profit_target_pct;
    let stop_loss_pct         = bot_cfg.stop_loss_pct;
    let max_position_days     = bot_cfg.max_position_days;
    let min_vol_percentile    = bot_cfg.min_vol_percentile;
    let max_daily_loss        = equity * bot_cfg.max_daily_loss_pct;
    let mut estimated_daily_loss = 0.0_f64;
    let mut circuit_broken       = false;

    // ── Load per-symbol vol history for HV-rank gate (P2.2) ──────────────
    // For each symbol, compute the 40th-percentile of rolling-22-day HV from
    // the one-year CSV.  Signals are skipped if current HV is below this floor.
    let mut vol_p40_threshold: HashMap<String, f64> = HashMap::new();
    for sym in &symbols {
        let csv = format!("data/{}_one_year.csv", sym.to_lowercase());
        let alt = format!("data/{}_five_year.csv", sym.to_lowercase());
        let path = if std::path::Path::new(&csv).exists() { csv } else { alt };
        match load_csv_closes(&path) {
            Ok(history) if history.len() >= 23 => {
                let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
                let mut hvs: Vec<f64> = closes
                    .windows(22)
                    .map(|w| compute_historical_vol(w))
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .collect();
                if !hvs.is_empty() {
                    hvs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let idx = ((hvs.len() as f64) * min_vol_percentile) as usize;
                    let threshold = hvs[idx.min(hvs.len() - 1)];
                    println!("  📈 {} HV p{:.0} threshold: {:.1}%",
                        sym, min_vol_percentile * 100.0, threshold * 100.0);
                    vol_p40_threshold.insert(sym.clone(), threshold);
                }
            }
            Ok(_)  => warn!("Not enough history for {} vol rank — gate disabled", sym),
            Err(e) => warn!("Could not load vol history for {}: {} — gate disabled", sym, e),
        }
    }

    // ── Strategy registry ─────────────────────────────────────────────────
    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(MomentumStrategy::new()));
    registry.register(Box::new(MeanReversionStrategy::new()));
    registry.register(Box::new(BreakoutStrategy::new()));
    registry.register(Box::new(VolatilityArbitrageStrategy::new()));
    registry.register(Box::new(CashSecuredPuts::new()));

    // ── Rolling price buffers and signal-cooldown trackers ────────────────
    let mut price_buf: HashMap<String, VecDeque<f64>> = HashMap::new();
    let mut last_signal: HashMap<String, Instant>     = HashMap::new();

    println!("📡 Connecting to Alpaca live stream for {} symbols...", symbols.len());
    let mut stream = match streaming::AlpacaStream::connect_from_env(&symbols).await {
        Ok(s) => s,
        Err(e) => { eprintln!("WebSocket connect failed: {}", e); return; }
    };
    println!("Stream connected -- press Ctrl-C to stop\n");

    // ── Main event loop ───────────────────────────────────────────────────
    let mut ctrl_c_trade = std::pin::pin!(tokio::signal::ctrl_c());
    loop {
        let event = tokio::select! {
            biased;
            res = &mut ctrl_c_trade => {
                let _ = res;
                info!("Ctrl-C received -- shutting down trading bot gracefully.");
                break;
            }
            ev = stream.next_event() => ev,
        };
        match event {
            Some(streaming::MarketEvent::Trade(t)) => {
                let sym   = t.symbol.clone();
                let price = t.price;
                let ts    = t.timestamp.clone();

                // Persist raw tick
                let rec = persistence::TradeRecord {
                    symbol:        sym.clone(),
                    action:        "tick".to_string(),
                    quantity:      t.size as f64,
                    price,
                    order_id:      None,
                    fill_status:   Some("tick".to_string()),
                    strategy:      None,
                    error_message: None,
                    timestamp:     ts.clone(),
                };
                if let Err(e) = store.insert_trade(&rec).await {
                    eprintln!("DB write error: {}", e);
                }

                // Update rolling price buffer
                let buf = price_buf.entry(sym.clone())
                    .or_insert_with(VecDeque::new);
                buf.push_back(price);
                if buf.len() > max_price_buf { buf.pop_front(); }
                if buf.len() < min_prices_for_hv { continue; }

                // Circuit breaker — stop new signals if daily loss limit is hit
                if circuit_broken { continue; }

                // Signal cooldown check
                let now = Instant::now();
                if let Some(&prev) = last_signal.get(&sym) {
                    if now.duration_since(prev).as_secs() < signal_cooldown_secs { continue; }
                }

                // Volatility + Heston params from rolling ticks
                let prices: Vec<f64> = buf.iter().copied().collect();
                let sigma = compute_historical_vol(&prices);
                if sigma < 1e-8 { continue; }
                let heston = heston_start(price, sigma, 1.0, 0.05);
                let model_iv = heston.v0.sqrt();

                // ── P2.1 Position close check ─────────────────────────────
                if open_syms.contains(&sym) {
                    if let Some(pos) = open_positions.get(&sym) {
                        let today = Utc::now().date_naive();
                        let mut should_close = false;
                        let mut close_reason = String::new();

                        // Days-based close: force exit after max_position_days
                        if let Some(exp_str) = &pos.expires_at {
                            if let Ok(exp_date) = NaiveDate::parse_from_str(exp_str, "%Y-%m-%d") {
                                let remaining = (exp_date - today).num_days();
                                if remaining <= 0 {
                                    should_close = true;
                                    close_reason = "expired / max days reached".to_string();
                                } else if remaining <= 1 {
                                    // 1 DTE — exit day before expiry to avoid pin risk
                                    should_close = true;
                                    close_reason = format!("1 DTE early exit (exp {})", exp_str);
                                } else {
                                    // P&L-based close using ATM option repricing
                                    if let Some(entry_premium) = pos.premium_collected {
                                        if entry_premium > 0.0 {
                                            let remaining_t = (remaining as f64 / 365.0).max(1.0 / 365.0);
                                            let current_val = price * sigma * remaining_t.sqrt();
                                            let pct = current_val / entry_premium;
                                            if pct <= profit_target_pct {
                                                should_close = true;
                                                close_reason = format!(
                                                    "profit target hit ({:.0}% of premium remaining)",
                                                    pct * 100.0);
                                            } else if pct >= stop_loss_pct {
                                                should_close = true;
                                                close_reason = format!(
                                                    "stop loss hit ({:.0}% of premium = {:.0}% loss)",
                                                    pct * 100.0,
                                                    (pct - 1.0) * 100.0);
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // No expiry stored — use calendar days from entry
                            if let Ok(entry_date) = NaiveDate::parse_from_str(
                                    &pos.entry_date[..10], "%Y-%m-%d") {
                                let age = (today - entry_date).num_days();
                                if age >= max_position_days {
                                    should_close = true;
                                    close_reason = format!("max {} days elapsed", age);
                                }
                            }
                        }

                        if should_close {
                            info!("Closing {} ({}): {}", sym, pos.strategy.as_deref().unwrap_or("?"), close_reason);
                            println!("  🔒 Closing {} — {}", sym, close_reason);
                            if !dry_run {
                                match client.close_position(&sym).await {
                                    Ok(_) => {
                                        let _ = store.close_position(&sym).await;
                                        open_syms.remove(&sym);
                                        open_positions.remove(&sym);
                                        println!("    ✅ {} closed", sym);
                                    }
                                    Err(e) => {
                                        error!("close_position failed for {}: {}", sym, e);
                                        eprintln!("    ⚠️  Close failed for {}: {}", sym, e);
                                    }
                                }
                            } else {
                                println!("    [DRY RUN] Would close {}", sym);
                            }
                            continue; // Skip signal generation this tick — position is being closed
                        }
                    }
                    // Position is open and not ready to close; skip new-entry signals
                    continue;
                }

                // ── P2.2 HV-rank gate: skip low-IV environments ───────────
                if let Some(&hv_floor) = vol_p40_threshold.get(&sym) {
                    if sigma < hv_floor {
                        // Current vol is below the 40th percentile — premium too thin
                        continue;
                    }
                }

                // ── P2.3 Regime detection ─────────────────────────────────
                let trend_strength = {
                    let n = prices.len();
                    if n >= 2 {
                        (prices[n - 1] / prices[0] - 1.0) / 0.10
                    } else {
                        0.0
                    }
                };
                let regime = RegimeDetector::detect_from_scalars(sigma, trend_strength);

                // Generate signals
                let signals = registry.generate_all_signals(&sym, price, sigma, model_iv, sigma);
                let actionable: Vec<_> = signals.iter()
                    .filter(|s| {
                        // Base confidence gate
                        if s.confidence < min_confidence { return false; }
                        if matches!(s.action, SignalAction::NoAction) { return false; }
                        if matches!(s.action, SignalAction::ClosePosition { .. }) { return false; }
                        // Regime weight gate: skip strategies with weight < 0.5
                        let weight = RegimeDetector::weight_for(&regime, &s.strategy_name);
                        s.confidence * weight >= min_confidence
                    })
                    .collect();

                if actionable.is_empty() { continue; }
                last_signal.insert(sym.clone(), now);

                // Portfolio risk gate
                let rough_premium = (price * sigma * (30.0_f64 / 365.0).sqrt()).max(0.01);
                let decision = pm.can_take_position(
                    &actionable[0].strategy_name,
                    rough_premium,
                    sigma,
                    1,
                );
                for w in &decision.risk_warnings {
                    warn!("Risk gate [{}]: {}", sym, w);
                    println!("  Risk [{}]: {}", sym, w);
                }

                info!("Signal [{}] ${:.2} vol={:.1}% regime={:?} -- {} actionable signal(s)",
                    sym, price, sigma * 100.0, regime, actionable.len());
                println!("[{}]  ${:.2}  vol={:.1}%  regime={:?}", sym, price, sigma * 100.0, regime);
                for sig in &actionable {
                    let qty = decision.suggested_size.max(1) as u32;
                    println!("  📊 [{:<22}]  {:?}  K=${:.2}  exp={}d  conf={:.0}%  edge=${:.2}",
                        sig.strategy_name, sig.action,
                        sig.strike, sig.expiry_days,
                        sig.confidence * 100.0, sig.edge);

                    if dry_run {
                        println!("    [DRY RUN] Would submit {} x {}", qty, sym);
                        continue;
                    }
                    if !decision.can_trade { continue; }
                    // Deduplication guard
                    if open_syms.contains(&sym) {
                        println!("    [SKIP] {} -- open position already tracked", sym);
                        continue;
                    }

                    match AlpacaClient::signal_to_options_order(&sig.action, &sym, qty, None) {
                        Ok(order) => {
                            match client.submit_options_order(&order).await {
                                Ok(filled) => {
                                    info!("Order submitted: id={} sym={} status={} strategy={}",
                                        filled.id, sym, filled.status, sig.strategy_name);
                                    println!("    Submitted: {} ({})", filled.id, filled.status);
                                    estimated_daily_loss += rough_premium * qty as f64 * 100.0;
                                    if estimated_daily_loss >= max_daily_loss && !circuit_broken {
                                        circuit_broken = true;
                                        error!("CIRCUIT BREAKER: daily spend ${:.2} >= limit ${:.2} -- halting new orders",
                                            estimated_daily_loss, max_daily_loss);
                                    }
                                    let expiry_date = Utc::now().date_naive()
                                        + Duration::days(sig.expiry_days as i64);
                                    let pos = persistence::PositionRecord {
                                        symbol:            sym.clone(),
                                        qty:               qty as f64,
                                        entry_price:       price,
                                        entry_date:        ts.clone(),
                                        strategy:          Some(sig.strategy_name.clone()),
                                        expires_at:        Some(expiry_date.format("%Y-%m-%d").to_string()),
                                        premium_collected: Some(rough_premium),
                                    };
                                    if let Err(e) = store.upsert_position(&pos).await {
                                        error!("DB position upsert failed: {}", e);
                                        eprintln!("DB position error: {}", e);
                                    } else {
                                        open_syms.insert(sym.clone());
                                        open_positions.insert(sym.clone(), pos);
                                    }
                                }
                                Err(e) => {
                                    error!("Order failed: sym={} error={}", sym, e);
                                    eprintln!("    Order failed: {}", e);
                                    let fail_rec = persistence::TradeRecord {
                                        symbol:        sym.clone(),
                                        action:        format!("{:?}", sig.action),
                                        quantity:      qty as f64,
                                        price,
                                        order_id:      None,
                                        fill_status:   Some("error".to_string()),
                                        strategy:      Some(sig.strategy_name.clone()),
                                        error_message: Some(e.to_string()),
                                        timestamp:     ts.clone(),
                                    };
                                    let _ = store.insert_trade(&fail_rec).await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Cannot build order for {:?}: {}", sig.action, e);
                            eprintln!("    Cannot build order for {:?}: {}", sig.action, e);
                        }
                    }
                }
            }
            Some(streaming::MarketEvent::Quote(q)) => {
                println!("[QUOTE] {}  bid=${:.4} ask=${:.4}", q.symbol, q.bid_price, q.ask_price);
            }
            Some(streaming::MarketEvent::Reconnected) => {
                info!("Stream reconnected.");
                println!("Stream reconnected -- resuming trading loop.");
            }
            Some(streaming::MarketEvent::Disconnected) | None => {
                error!("Stream permanently disconnected -- stopping bot.");
                println!("Stream permanently disconnected -- stopping bot.");
                break;
            }
        }
    }

    // ── Graceful shutdown ─────────────────────────────────────────────────
    info!("Shutting down -- closing WebSocket...");
    let _ = stream.close().await;

    info!("Cancelling any open orders...");
    match client.cancel_all_orders().await {
        Ok(cancelled) if !cancelled.is_empty() => {
            warn!("{} order(s) cancelled on shutdown", cancelled.len());
            for o in &cancelled {
                warn!("  Cancelled: id={} status={}", o.id, o.status);
            }
        }
        Ok(_)  => info!("No open orders to cancel."),
        Err(e) => warn!("cancel_all_orders failed: {}", e),
    }

    let final_positions = store.get_open_positions().await.unwrap_or_default();
    let session_trades  = store.get_trade_history(200).await.unwrap_or_default();
    let session_orders  = session_trades.iter().filter(|t| t.action != "tick").count();
    info!("Session summary: {} open position(s) | {} orders | ${:.2} estimated daily spend",
        final_positions.len(), session_orders, estimated_daily_loss);
    println!("\n--- Session summary ---");
    println!("  Open positions : {}", final_positions.len());
    println!("  Orders this run: {}", session_orders);
    println!("  Est. daily spend: ${:.2}", estimated_daily_loss);
}
