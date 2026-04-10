// Live trading WebSocket event loop extracted from main::cmd_trade.
// Called when `dollarbill trade --live` (or `--dry-run`) is invoked.

use crate::alerting::Alerter;
use crate::alpaca::AlpacaClient;
use crate::analysis::regime_detector::RegimeDetector;
use crate::calibration::heston_calibrator::{calibrate_heston, CalibParams};
use crate::config;
use crate::market_data::csv_loader::load_csv_closes;
use crate::market_data::options_feed::LiveIvCache;
use crate::market_data::real_option_data_yahoo::fetch_liquid_options;
use crate::market_data::real_market_data::fetch_latest_price;
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
use log::{debug, info, warn, error};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Parse expiry date from an OCC symbol string.
/// OCC format: ROOT(6) YYMMDD(6) C|P(1) STRIKE(8)  — e.g. "QCOM260508P00120000"
/// Returns `Some("2026-05-08")` or `None` if parsing fails.
fn parse_occ_expiry(occ: &str) -> Option<String> {
    if occ.len() < 15 { return None; }
    let yy: u32 = occ[6..8].parse().ok()?;
    let mm: u32 = occ[8..10].parse().ok()?;
    let dd: u32 = occ[10..12].parse().ok()?;
    if mm < 1 || mm > 12 || dd < 1 || dd > 31 { return None; }
    Some(format!("20{:02}-{:02}-{:02}", yy, mm, dd))
}

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
                // For options (OCC symbols like "QCOM260508P00120000") use the
                // underlying root as the key so WebSocket tick matching works.
                // Store the full OCC symbol in occ_symbol for accurate closing.
                let (rec_symbol, rec_occ, rec_expires_at) =
                    if ap.asset_class == "us_option" && ap.symbol.len() >= 18 {
                        let root = ap.symbol[..6].trim().to_string();
                        // OCC: ROOT(6) YYMMDD(6) C|P(1) STRIKE(8)
                        // e.g. QCOM260508P00120000 → 2026-05-08
                        let exp = parse_occ_expiry(&ap.symbol);
                        (root, Some(ap.symbol.clone()), exp)
                    } else {
                        (ap.symbol.clone(), None, None)
                    };
                if !sqlite_syms.contains(&rec_symbol) {
                    println!("  📥 Importing {} (Alpaca: {}) from Alpaca into SQLite", rec_symbol, ap.symbol);
                    let premium = ap.avg_entry_price.parse::<f64>().ok()
                        .filter(|&p| p > 0.0);
                    let rec = persistence::PositionRecord {
                        symbol:            rec_symbol,
                        qty:               ap.qty.parse::<f64>().unwrap_or(0.0),
                        entry_price:       ap.avg_entry_price.parse::<f64>().unwrap_or(0.0),
                        entry_date:        Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                        strategy:          Some("reconciled".to_string()),
                        expires_at:        rec_expires_at,
                        premium_collected: premium,
                        occ_symbol:        rec_occ,
                    };
                    let _ = store.upsert_position(&rec).await;
                } else {
                    // Position already in SQLite — patch expires_at and premium_collected
                    // if they are missing (e.g. previous reconciliation didn't fill them).
                    if let Some(existing) = persisted.iter().find(|p| p.symbol == rec_symbol) {
                        if existing.expires_at.is_none() || existing.premium_collected.is_none() {
                            let exp = if existing.expires_at.is_none() {
                                if ap.asset_class == "us_option" && ap.symbol.len() >= 18 {
                                    parse_occ_expiry(&ap.symbol)
                                } else { None }
                            } else {
                                existing.expires_at.clone()
                            };
                            let premium = if existing.premium_collected.is_none() {
                                ap.avg_entry_price.parse::<f64>().ok().filter(|&p| p > 0.0)
                            } else {
                                existing.premium_collected
                            };
                            let patched = persistence::PositionRecord {
                                expires_at:        exp,
                                premium_collected: premium,
                                occ_symbol:        existing.occ_symbol.clone().or_else(|| {
                                    if ap.asset_class == "us_option" { Some(ap.symbol.clone()) } else { None }
                                }),
                                ..existing.clone()
                            };
                            let _ = store.upsert_position(&patched).await;
                            println!("  🔧 Patched {} — expires_at={:?} premium={:?}",
                                rec_symbol, patched.expires_at, patched.premium_collected);
                        }
                    }
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

    // ── Email alerting (P4.2) ─────────────────────────────────────────────
    let alert_cfg = config::TradingBotConfigFile::load_alerts();
    let alerter = Alerter::new(alert_cfg.clone());
    if alerter.is_active() {
        info!("Email alerting enabled → {}", alert_cfg.to);
    }
    let mut daily_loss_warned = false;
    // ── Dashboard status state ────────────────────────────────────────────
    let mut last_signal_desc: HashMap<String, String> = HashMap::new();
    let mut session_orders:   usize = 0;
    let mut port_delta:       f64   = 0.0;
    let mut port_gamma:       f64   = 0.0;
    let mut port_vega:        f64   = 0.0;
    let mut port_theta:       f64   = 0.0;

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

    // ── P3.1 Live IV cache (15-min TTL) ───────────────────────────────────
    let iv_cache = LiveIvCache::new(900);

    // ── P3.2 Shared calibrated Heston params (updated by background task) ─
    let live_params: Arc<RwLock<HashMap<String, CalibParams>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // Seed from existing JSON files so first ticks use real params immediately
    for sym in &symbols {
        let path = format!("data/{}_heston_params.json", sym.to_ascii_lowercase());
        if let Ok(contents) = std::fs::read_to_string(&path) {
            #[derive(serde::Deserialize)]
            struct HJson { kappa: f64, theta: f64, sigma: f64, rho: f64, v0: f64 }
            #[derive(serde::Deserialize)]
            struct CalibJson { heston_params: HJson }
            if let Ok(c) = serde_json::from_str::<CalibJson>(&contents) {
                let p = c.heston_params;
                live_params.write().unwrap().insert(sym.clone(), CalibParams {
                    kappa: p.kappa, theta: p.theta,
                    sigma: p.sigma, rho: p.rho, v0: p.v0,
                });
                info!("Seeded Heston params for {} from cached JSON (v0={:.4})", sym, p.v0);
            }
        }
    }

    // Spawn background recalibration task (every 30 minutes during session)
    {
        let syms_bg    = symbols.clone();
        let params_bg  = Arc::clone(&live_params);
        tokio::spawn(async move {
            let calib_interval = tokio::time::Duration::from_secs(30 * 60);
            loop {
                tokio::time::sleep(calib_interval).await;
                for sym in &syms_bg {
                    let spot = match fetch_latest_price(sym).await {
                        Ok(s) => s,
                        Err(e) => { warn!("BG calib price fetch failed for {}: {}", sym, e); continue; }
                    };
                    let opts = match fetch_liquid_options(sym, 0, 10, 25.0).await {
                        Ok(o) if !o.is_empty() => o,
                        Ok(_) => { warn!("BG calib: no liquid options for {}", sym); continue; }
                        Err(e) => { warn!("BG calib options fetch failed for {}: {}", sym, e); continue; }
                    };
                    let initial = CalibParams {
                        kappa: 2.0, theta: 0.25,
                        sigma: 0.30, rho: -0.60, v0: 0.25,
                    };
                    match calibrate_heston(spot, 0.05, opts, initial) {
                        Ok(res) => {
                            info!("BG calib {} rmse={:.4} v0={:.4}",
                                sym, res.rmse, res.params.v0);
                            params_bg.write().unwrap()
                                .insert(sym.clone(), res.params);
                        }
                        Err(e) => warn!("BG calib failed for {}: {}", sym, e),
                    }
                }
            }
        });
    }

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
        // Normalize Trade and Quote events into a shared (sym, price, ts)
        let (sym, price, ts) = match event {
            Some(streaming::MarketEvent::Trade(t)) => {
                let sym   = t.symbol.clone();
                let price = t.price;
                let ts    = t.timestamp.clone();
                // Persist raw tick (trades only)
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
                (sym, price, ts)
            }
            Some(streaming::MarketEvent::Quote(q)) => {
                let mid = (q.bid_price + q.ask_price) / 2.0;
                (q.symbol.clone(), mid, q.timestamp.clone())
            }
            Some(streaming::MarketEvent::Reconnected) => {
                info!("Stream reconnected.");
                println!("Stream reconnected -- resuming trading loop.");
                continue;
            }
            Some(streaming::MarketEvent::Disconnected) | None => {
                error!("Stream permanently disconnected -- stopping bot.");
                println!("Stream permanently disconnected -- stopping bot.");
                alerter.disconnect().await;
                break;
            }
        };
        {
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

                // ── P3.1 Non-blocking IV cache refresh ────────────────────
                // The main cache is not sent into the task; instead we log that
                // a refresh is warranted.  The live_params map (shared via Arc)
                // is updated by the 30-min background task; the IV cache is only
                // used as a quick per-tick read-path.
                if iv_cache.get_cached_iv(&sym).is_none() {
                    debug!("No cached IV for {} yet; background task will refresh", sym);
                }

                // Volatility + Heston params from rolling ticks
                let prices: Vec<f64> = buf.iter().copied().collect();
                let sigma = compute_historical_vol(&prices);
                if sigma < 1e-8 { continue; }

                // ── P3.1 Use live IV from cache if available, else HV ─────
                let live_iv = iv_cache.get_cached_iv(&sym);
                let model_iv = live_iv.unwrap_or_else(|| {
                    // Kick off an async refresh but don't block the tick loop.
                    // The refresh will populate the cache for the next tick.
                    // We use the shared calibrated v0 as the best available IV proxy.
                    live_params
                        .read().unwrap()
                        .get(&sym)
                        .map(|p| p.v0.sqrt())
                        .unwrap_or_else(|| {
                            let h = heston_start(price, sigma, 1.0, 0.05);
                            h.v0.sqrt()
                        })
                });

                // ── IV sanity guard: stale after-hours quotes produce absurd IV ──
                // >200% IV is physically implausible for equities during live hours;
                // skip this tick rather than fire garbage signals.
                if model_iv > 2.0 {
                    debug!("Skipping {} — IV {:.1}% looks like stale/after-hours data", sym, model_iv * 100.0);
                    continue;
                }

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
                            // No expiry stored — use calendar days from entry.
                            // If entry_date can't be parsed (e.g. old "reconciled" records
                            // written before this fix), close immediately to free buying power.
                            let raw = if pos.entry_date.len() >= 10 {
                                &pos.entry_date[..10]
                            } else {
                                ""
                            };
                            match NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
                                Ok(entry_date) => {
                                    let age = (today - entry_date).num_days();
                                    if age >= max_position_days {
                                        should_close = true;
                                        close_reason = format!("max {} days elapsed", age);
                                    }
                                }
                                Err(_) => {
                                    // Unparseable entry date (stale reconciled record) — close now.
                                    should_close = true;
                                    close_reason = "stale reconciled position (no valid entry date)".to_string();
                                }
                            }
                        }

                        if should_close {
                            info!("Closing {} ({}): {}", sym, pos.strategy.as_deref().unwrap_or("?"), close_reason);
                            println!("  🔒 Closing {} — {}", sym, close_reason);
                            if !dry_run {
                                // Use the stored OCC symbol for options, underlying for equity.
                                // For multi-leg positions (occ_symbol = None, no equity ticker),
                                // fall back to querying live positions and closing each leg.
                                let occ_opt = pos.occ_symbol.clone();
                                let close_sym = occ_opt.as_deref().unwrap_or(&sym);
                                let close_result = client.close_position(close_sym).await;
                                match close_result {
                                    Ok(_) => {
                                        let _ = store.close_position(&sym).await;
                                        open_syms.remove(&sym);
                                        open_positions.remove(&sym);
                                        println!("    ✅ {} closed ({})", sym, close_sym);
                                    }
                                    Err(e) => {
                                        // If direct close failed and we used the underlying,
                                        // the position may be an untracked multi-leg options
                                        // order.  Try closing all legs via positions lookup.
                                        if occ_opt.is_none() {
                                            warn!("Direct close failed for {} — trying leg lookup: {}", sym, e);
                                            let results = client.close_positions_for_underlying(&sym).await;
                                            let any_ok = results.iter().any(|r| r.is_ok());
                                            if any_ok {
                                                let _ = store.close_position(&sym).await;
                                                open_syms.remove(&sym);
                                                open_positions.remove(&sym);
                                                println!("    ✅ {} legs closed via lookup", sym);
                                            } else {
                                                error!("close leg lookup also failed for {}: {:?}", sym,
                                                    results.iter().map(|r| r.as_ref().err().map(|e| e.to_string())).collect::<Vec<_>>());
                                                eprintln!("    ⚠️  All close attempts failed for {}", sym);
                                            }
                                        } else {
                                            error!("close_position failed for {} ({}): {}", sym, close_sym, e);
                                            eprintln!("    ⚠️  Close failed for {} ({}): {}", sym, close_sym, e);
                                        }
                                    }
                                }
                            } else {
                                let close_sym = pos.occ_symbol.as_deref().unwrap_or(&sym);
                                println!("    [DRY RUN] Would close {} ({})", sym, close_sym);
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

                    match AlpacaClient::signal_to_options_order(&sig.action, &sym, qty, None) {
                        Ok(mut order) => {
                            // Resolve the generated OCC symbol to the nearest actually-listed contract
                            if let Some(ref raw_sym) = order.symbol.clone() {
                                order.symbol = Some(client.resolve_single_leg_occ(raw_sym).await);
                            }
                            match client.submit_options_order(&order).await {
                                Ok(filled) => {
                                    info!("Order submitted: id={} sym={} status={} strategy={}",
                                        filled.id, sym, filled.status, sig.strategy_name);
                                    println!("    Submitted: {} ({})", filled.id, filled.status);
                                    estimated_daily_loss += rough_premium * qty as f64 * 100.0;

                                    // Daily-loss warning at configurable threshold (default 80%)
                                    if !daily_loss_warned
                                        && estimated_daily_loss >= alert_cfg.daily_loss_alert_pct * max_daily_loss
                                    {
                                        daily_loss_warned = true;
                                        let a = alerter.clone();
                                        let (edl, mdl) = (estimated_daily_loss, max_daily_loss);
                                        tokio::spawn(async move { a.daily_loss_warning(edl, mdl).await; });
                                    }

                                    if estimated_daily_loss >= max_daily_loss && !circuit_broken {
                                        circuit_broken = true;
                                        error!("CIRCUIT BREAKER: daily spend ${:.2} >= limit ${:.2} -- halting new orders",
                                            estimated_daily_loss, max_daily_loss);
                                        let a = alerter.clone();
                                        let (edl, mdl) = (estimated_daily_loss, max_daily_loss);
                                        tokio::spawn(async move { a.circuit_breaker(edl, mdl).await; });
                                    }
                                    // dashboard tracking
                                    session_orders += 1;
                                    let desc = format!("{} {:?} @ ${:.2}  {}",
                                        sig.strategy_name, sig.action, price,
                                        chrono::Utc::now().format("%H:%M:%S"));
                                    last_signal_desc.insert(sym.clone(), desc);
                                    let expiry_date = Utc::now().date_naive()
                                        + Duration::days(sig.expiry_days as i64);
                                    // Store the OCC symbol from the confirmed fill so
                                    // close logic can target the exact contract.
                                    // Single-leg: filled.symbol is the OCC (>10 chars).
                                    // Multi-leg:  filled.symbol may be empty/short; use None.
                                    let occ = if filled.symbol.len() > 10 {
                                        Some(filled.symbol.clone())
                                    } else {
                                        None
                                    };
                                    let pos = persistence::PositionRecord {
                                        symbol:            sym.clone(),
                                        qty:               qty as f64,
                                        entry_price:       price,
                                        entry_date:        ts.clone(),
                                        strategy:          Some(sig.strategy_name.clone()),
                                        expires_at:        Some(expiry_date.format("%Y-%m-%d").to_string()),
                                        premium_collected: Some(rough_premium),
                                        occ_symbol:        occ,
                                    };
                                    if let Err(e) = store.upsert_position(&pos).await {
                                        error!("DB position upsert failed: {}", e);
                                        eprintln!("DB position error: {}", e);
                                    } else {
                                        open_syms.insert(sym.clone());
                                        open_positions.insert(sym.clone(), pos);
                                        // Fill alert
                                        let a = alerter.clone();
                                        let sym_c  = sym.clone();
                                        let strat_c = sig.strategy_name.clone();
                                        let (qty_c, price_c) = (qty, price);
                                        tokio::spawn(async move { a.fill(&sym_c, &strat_c, qty_c, price_c).await; });
                                    }

                                    // ── P3.3 Greeks / portfolio-risk alert ────────────
                                    let risk = pm.get_portfolio_risk();
                                    port_delta = risk.total_delta;
                                    port_gamma = risk.total_gamma;
                                    port_vega  = risk.total_vega;
                                    port_theta = risk.total_theta;
                                    info!("Portfolio risk after order: Δ={:.3} Γ={:.4} Vega={:.2} Theta={:.2}",
                                        risk.total_delta, risk.total_gamma,
                                        risk.total_vega, risk.total_theta);
                                    println!("  📐 Portfolio risk:  Δ={:.3}  Γ={:.4}  Vega={:.2}  Theta={:.2}  NetExp=${:.0}",
                                        risk.total_delta, risk.total_gamma,
                                        risk.total_vega, risk.total_theta,
                                        risk.net_exposure);
                                    // Issue a hedge alert if portfolio delta is too large
                                    let delta_limit = equity * 0.30 / 100.0; // 30% of equity per $100
                                    if risk.total_delta.abs() > delta_limit {
                                        warn!("DELTA HEDGE ALERT: |Δ|={:.3} exceeds limit {:.3} -- consider hedging",
                                            risk.total_delta, delta_limit);
                                        println!("  ⚠️  DELTA HEDGE ALERT: |Δ|={:.3} exceeds {:.3} — consider hedging",
                                            risk.total_delta, delta_limit);
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
                // ── Write dashboard status file ───────────────────────────
                persistence::BotStatus {
                    updated_at:           chrono::Utc::now().to_rfc3339(),
                    dry_run,
                    circuit_broken,
                    estimated_daily_loss,
                    max_daily_loss,
                    equity,
                    open_position_count:  open_syms.len(),
                    session_orders,
                    last_signals:         last_signal_desc.clone(),
                    portfolio_delta:      port_delta,
                    portfolio_gamma:      port_gamma,
                    portfolio_vega:       port_vega,
                    portfolio_theta:      port_theta,
                }.write();
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
