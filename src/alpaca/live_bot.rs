// Live trading WebSocket event loop extracted from main::cmd_trade.
// Called when `dollarbill trade --live` (or `--dry-run`) is invoked.

use crate::alpaca::occ as occ_parser;
use crate::risk::guards::{DailyRiskLimits, check_all as check_risk_guards};
use crate::risk::invariants::{assert_invariants, BotState, InvariantPosition};
use crate::risk::{ManagementAction, ManagementConfig, manage_open_positions};
use crate::alerting::Alerter;
use crate::alpaca::AlpacaClient;
use crate::alpaca::execution::{position_from_fill, managed_positions, invariant_positions,
    management_action, close_tracked_position, reconcile_positions};
use crate::analysis::regime_detector::RegimeDetector;
use crate::calibration::heston_calibrator::{calibrate_heston, CalibParams};
use crate::config;
use crate::market_data::csv_loader::load_csv_closes;
use crate::market_data::options_feed::LiveIvCache;
use crate::market_data::real_option_data_yahoo::fetch_liquid_options;
use crate::market_data::spot_price::fetch_spot;
use crate::models::bs_mod::{compute_historical_vol, compute_historical_vol_ticks, black_scholes_call, black_scholes_put};
use crate::models::heston::heston_start;
use crate::persistence;
use crate::portfolio::{PortfolioManager, PortfolioConfig};
use crate::portfolio::position_monitor::CooldownTracker;
use crate::strategies::{
    SignalAction, StrategyRegistry,
    momentum::MomentumStrategy,
    mean_reversion::MeanReversionStrategy,
    breakout::BreakoutStrategy,
    vol_arbitrage::VolatilityArbitrageStrategy,
    cash_secured_puts::CashSecuredPuts,
};
use crate::streaming;
use chrono::Utc;
use log::{debug, info, warn, error};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Conservative collateral based on the resolved standard-contract payoff.
/// Naked calls (including short straddles) are unsupported by this entry path.
fn resolved_entry_budget(order: &crate::alpaca::types::OptionsOrderRequest, premium: f64) -> Result<f64, String> {
    use crate::alpaca::types::OrderSide;
    use crate::risk::payoff::{classify_group, OptionExposure, RiskClassification};
    if !premium.is_finite() || premium <= 0.0 { return Err("Invalid premium estimate".into()); }
    let legs: Vec<_> = if let Some(legs) = &order.legs {
        legs.iter().map(|leg| (leg.symbol.as_str(), leg.ratio_qty, &leg.side)).collect()
    } else {
        vec![(order.symbol.as_deref().ok_or("Missing entry symbol")?, 1,
            order.side.as_ref().ok_or("Missing entry side")?)]
    };
    if legs.is_empty() { return Err("Empty entry legs".into()); }
    let mut key = None;
    let mut exposures = Vec::new();
    let mut debit = 0.0;
    for (symbol, ratio, side) in legs {
        let occ = occ_parser::parse_occ(symbol).ok_or("Invalid resolved OCC")?;
        let this_key = (occ.root, occ.expiry_str);
        if key.as_ref().is_some_and(|k| k != &this_key) || ratio == 0 || ratio > i32::MAX as u32 {
            return Err("Entry requires one underlying/expiry and valid ratios".into());
        }
        key = Some(this_key);
        let buy = matches!(side, OrderSide::Buy);
        if buy { debit += premium * ratio as f64 * 100.0; }
        exposures.push(OptionExposure { is_call: occ.is_call, strike: occ.strike,
            quantity: if buy { ratio as i32 } else { -(ratio as i32) } });
    }
    match classify_group(&exposures) {
        RiskClassification::DefinedRisk { max_loss } => {
            let cost = max_loss + debit; // Do not spend uncertain short premium twice.
            if cost.is_finite() && cost > 0.0 { Ok(cost) } else { Err("Invalid entry budget".into()) }
        }
        _ => Err("Uncovered short-call entry is unsupported".into()),
    }
}

fn candidate_record(order: &crate::alpaca::types::OptionsOrderRequest, root: &str, qty: u32)
    -> Result<persistence::PositionRecord, String> {
    use crate::alpaca::types::OrderSide;
    let legs = if let Some(legs) = &order.legs {
        legs.iter().map(|l| persistence::PositionLegRecord { occ_symbol: l.symbol.clone(),
            qty: qty as f64 * l.ratio_qty as f64 * if matches!(l.side, OrderSide::Buy) { 1.0 } else { -1.0 },
            entry_price: 0.0 }).collect()
    } else { vec![persistence::PositionLegRecord {
        occ_symbol: order.symbol.clone().ok_or("Missing order symbol")?,
        qty: qty as f64 * if matches!(order.side, Some(OrderSide::Buy)) { 1.0 } else { -1.0 },
        entry_price: 0.0 }] };
    Ok(persistence::PositionRecord { symbol: root.into(), qty: qty as f64, entry_price: 0.0,
        entry_date: Utc::now().to_rfc3339(), strategy: None, expires_at: None,
        premium_collected: None, occ_symbol: None, roll_count: 0, legs })
}

/// Build a marked risk snapshot; incomplete marks block entries, never erase risk.
fn marked_book(records: &HashMap<String, persistence::PositionRecord>,
    marks: &HashMap<String, (f64, f64, Instant)>, now: chrono::DateTime<Utc>)
    -> Result<Vec<crate::backtesting::position::Position>, String> {
    use crate::backtesting::position::{Position, OptionType};
    use crate::models::{american::ExerciseStyle, bs_mod::Greeks};
    let mut positions = Vec::new();
    for record in records.values() {
        let (spot, sigma, stamp) = marks.get(&record.symbol).ok_or("Missing portfolio mark")?;
        if stamp.elapsed().as_secs() > 120 || !spot.is_finite() || *spot <= 0.0 || !sigma.is_finite() || *sigma <= 0.0 {
            return Err("Invalid/stale portfolio mark".into());
        }
        let legs = if !record.legs.is_empty() { record.legs.clone() }
            else if let Some(occ) = &record.occ_symbol { vec![persistence::PositionLegRecord {
                occ_symbol: occ.clone(), qty: record.qty, entry_price: record.entry_price }] }
            else { Vec::new() };
        if legs.is_empty() {
            if !record.qty.is_finite() || record.qty.fract() != 0.0 || record.qty.abs() > i32::MAX as f64 {
                return Err("Unsupported equity quantity".into());
            }
            // Position's option multiplier is 100: scale per-share equity Greeks.
            positions.push(Position::new(positions.len(), record.symbol.clone(), OptionType::Call,
                ExerciseStyle::European, *spot, record.qty as i32, *spot / 100.0,
                record.entry_date.clone(), *spot, Some(Greeks { price: *spot / 100.0,
                    delta: 0.01, gamma: 0.0, vega: 0.0, theta: 0.0, rho: 0.0 })));
        }
        for leg in legs {
            let occ = occ_parser::parse_occ(&leg.occ_symbol).ok_or("Invalid portfolio OCC")?;
            if occ.root != record.symbol || !leg.qty.is_finite() || leg.qty.fract() != 0.0 || leg.qty.abs() > i32::MAX as f64 {
                return Err("Invalid portfolio leg".into());
            }
            let expiry = chrono::NaiveDate::parse_from_str(&occ.expiry_str, "%Y-%m-%d").map_err(|e| e.to_string())?
                .and_hms_opt(23,59,59).unwrap().and_utc();
            if expiry < now { return Err("Expired portfolio leg requires reconciliation".into()); }
            let t = (expiry - now).num_seconds() as f64 / (365.0 * 86400.0);
            let g = if occ.is_call { black_scholes_call(*spot, occ.strike, t, 0.05, *sigma) }
                else { black_scholes_put(*spot, occ.strike, t, 0.05, *sigma) };
            let mut position = Position::new(positions.len(), record.symbol.clone(),
                if occ.is_call { OptionType::Call } else { OptionType::Put }, ExerciseStyle::American,
                occ.strike, leg.qty as i32, leg.entry_price, record.entry_date.clone(), *spot, Some(g));
            position.entry_higher_greeks = Some(crate::models::bs_mod::higher_order_greeks(
                *spot, occ.strike, t, 0.05, *sigma, 0.0, occ.is_call));
            positions.push(position);
        }
    }
    Ok(positions)
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
            let now = Utc::now().to_rfc3339();
            let records = match reconcile_positions(&alpaca_pos, &persisted, &now) {
                Ok(records) => records,
                Err(e) => { error!("Position reconciliation failed: {}", e); return; }
            };
            for old in &persisted {
                if !records.iter().any(|p| p.symbol == old.symbol) {
                    if let Err(e) = store.close_position(&old.symbol).await {
                        error!("Removing stale position failed: {}", e); return;
                    }
                }
            }
            for record in records {
                if let Err(e) = store.upsert_position(&record).await {
                    error!("Saving reconciled position failed: {}", e); return;
                }
            }
        }
        Err(e) => { error!("Position reconciliation failed: {}", e); return; },
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
    // profit_target_pct, stop_loss_pct, itm_proximity_pct, roll_trigger_pct,
    // roll_dte_days, and max_rolls are passed into PositionMonitorConfig below.
    let max_rolls             = bot_cfg.max_rolls;   // still used in log messages
    let min_vol_percentile    = bot_cfg.min_vol_percentile;
    let spot_price_source     = bot_cfg.spot_price_source.clone();
    let max_daily_loss        = equity * bot_cfg.max_daily_loss_pct;
    let mut estimated_daily_loss = 0.0_f64;
    let mut circuit_broken       = false;
    // DailyRiskLimits guard — mirrors the guard in personality_based_bot
    let daily_risk_limits = DailyRiskLimits {
        max_daily_drawdown_pct: Some(bot_cfg.max_daily_loss_pct),
        max_daily_trades:       None,
    };
    let mut trades_today: usize = 0;
    // Tracks remaining options buying power so we can reject orders that would
    // exceed it before they reach Alpaca (avoiding 403 insufficient-buying-power).
    // Decremented after each fill; not incremented on close (conservative).
    let mut remaining_buy_pwr: f64 = buy_pwr;
    let mut risk_marks: HashMap<String, (f64, f64, Instant)> = HashMap::new();
    let mut last_account_refresh = Instant::now();

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

    // Legacy matrices label simple threshold variants as named strategies.
    // Do not use those uncertified artifacts to select live strategies.
    info!("Legacy performance-matrix filtering disabled pending validated research");

    // ── P3.1 Live IV cache (15-min TTL) ───────────────────────────────────
    // Shared so a background task can populate it (network fetches must never
    // block the per-tick trading loop below).
    let iv_cache: Arc<RwLock<LiveIvCache>> = Arc::new(RwLock::new(LiveIvCache::new(900)));

    // Spawn background live ATM-IV refresh task. Polls every minute and only
    // does the (network) fetch when a symbol's cached reading has gone stale,
    // storing the result under a brief write lock so the hot loop's reads
    // never wait on a network call.
    {
        let syms_iv = symbols.clone();
        let iv_bg   = Arc::clone(&iv_cache);
        tokio::spawn(async move {
            let poll_interval = tokio::time::Duration::from_secs(60);
            loop {
                for sym in &syms_iv {
                    let is_fresh = iv_bg.read().unwrap().get_cached_iv(sym).is_some();
                    if is_fresh { continue; }
                    match LiveIvCache::fetch_atm_iv(sym, 0.05).await {
                        Ok(iv) => {
                            iv_bg.write().unwrap().store(sym, iv);
                            info!("Live ATM IV refreshed for {}: {:.1}%", sym, iv * 100.0);
                        }
                        Err(e) => warn!("Live ATM IV refresh failed for {}: {} — using model/HV fallback", sym, e),
                    }
                }
                tokio::time::sleep(poll_interval).await;
            }
        });
    }

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
        let client_bg  = client.clone();
        let source_bg  = spot_price_source.clone();
        tokio::spawn(async move {
            let calib_interval = tokio::time::Duration::from_secs(30 * 60);
            // Calibrate immediately on startup (don't wait a full 30 min for
            // the first refresh — the JSON seed above can otherwise stay the
            // only source of `model_iv` for half an hour).
            loop {
                for sym in &syms_bg {
                    let spot = match fetch_spot(&source_bg, sym, || {
                        let c = client_bg.clone();
                        let s = sym.clone();
                        async move {
                            c.get_latest_trade(&s).await.map(|t| t.price)
                        }
                    }).await {
                        Ok(p) => p,
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
                tokio::time::sleep(calib_interval).await;
            }
        });
    }

    // ── Rolling price buffers and signal-cooldown trackers ────────────────
    // Each price sample is stamped with its arrival `Instant` so the realized
    // volatility below can be annualized from the buffer's actual sampling
    // frequency rather than assuming one sample per trading day.
    let mut price_buf: HashMap<String, VecDeque<(f64, Instant)>> = HashMap::new();
    let mut last_signal: HashMap<String, Instant>     = HashMap::new();
    // Equity tickers the bot intentionally holds long — never auto-sell or flag.
    let protected_equity_set: HashSet<String> =
        bot_cfg.protected_equity.iter().cloned().collect();
    // ── Shared position management config (mirrors entry-guard thresholds) ─
    let mgmt_config = ManagementConfig {
        credit_target_pct:       0.50,
        roll_before_dte:         21,
        max_rolls:               bot_cfg.max_rolls,
        roll_dte_days:           bot_cfg.roll_dte_days,
        risk_free_rate:          0.045,
        profit_target_pct:       bot_cfg.profit_target_pct,
        stop_loss_pct:           bot_cfg.stop_loss_pct,
        max_position_days:       bot_cfg.max_position_days as i64,
        itm_proximity_pct:       bot_cfg.itm_proximity_pct,
        roll_trigger_pct:        bot_cfg.roll_trigger_pct,
        block_long_premium:      true,
        max_portfolio_delta_pct: 0.003,
        protected_equity:        protected_equity_set.clone(),
        max_risk_per_symbol_pct: 0.06,
    };
    // Re-entry cooldown: after closing a position, block new entries for 5 min
    let mut cooldown = CooldownTracker::new(300);

    println!("📡 Connecting to Alpaca live stream for {} symbols...", symbols.len());
    // ── Options tier gate ─────────────────────────────────────────────────
    // For live accounts, abort if options trading is not yet approved.
    // Paper trading skips this check (paper accounts always report level 0).
    // To apply: https://app.alpaca.markets → Account → Options Trading → Apply
    if client.is_live() {
        match client.verify_options_approval(1).await {
            Ok(level) => println!("✅ Options Tier {} approved — live options orders enabled", level),
            Err(e) => {
                eprintln!("❌ Options approval check failed:\n{}", e);
                return;
            }
        }
    } else {
        println!("ℹ️  Paper trading: skipping options tier check");
    }
    // ─────────────────────────────────────────────────────────────────────
    let mut stream = match streaming::AlpacaStream::connect_from_env(&symbols).await {
        Ok(s) => s,
        Err(e) => { eprintln!("WebSocket connect failed: {}", e); return; }
    };
    println!("Stream connected -- press Ctrl-C to stop\n");

    // ── Sell any assigned equity (options-only bot) ───────────────────────
    // Any plain-equity position is an assignment accident — flatten immediately.
    if !dry_run {
        if let Ok(positions) = client.get_positions().await {
            for pos in &positions {
                let sym = &pos.symbol;
                let is_equity = sym.len() <= 6 && sym.chars().all(|c| c.is_ascii_alphabetic());
                if !is_equity { continue; }
                let qty: f64 = pos.qty.parse().unwrap_or(0.0);
                if qty == 0.0 { continue; }
                use crate::alpaca::types::{OrderRequest, OrderSide, OrderType, TimeInForce};
                let sell_req = OrderRequest {
                    symbol: sym.clone(),
                    qty: qty.abs(),
                    side: if qty > 0.0 { OrderSide::Sell } else { OrderSide::Buy },
                    r#type: OrderType::Market,
                    time_in_force: TimeInForce::Day,
                    limit_price: None, stop_price: None,
                    extended_hours: None, client_order_id: None,
                };
                match client.submit_order(&sell_req).await {
                    Ok(o)  => println!("  ⚠️  Assigned {sym} qty={qty:.0} — liquidated ({})", o.id),
                    Err(e) => eprintln!("  ❌ Failed to liquidate assigned {sym}: {e}"),
                }
            }
        }
    }

    // ── OPASN / OPEXC activity check on startup ───────────────────────────
    // Use Alpaca's structured activity feed instead of heuristic symbol-length
    // checks. OPASN = option assigned against us (short put exercised), OPEXC =
    // option exercised by us (long call/put).  Log any recent assignments so
    // operators can confirm the liquidation above handled them.
    match client.get_account_activities(&["OPASN", "OPEXC"], None).await {
        Ok(activities) => {
            let relevant: Vec<_> = activities.iter()
                .filter(|a| a.activity_type == "OPASN" || a.activity_type == "OPEXC")
                .collect();
            if !relevant.is_empty() {
                println!("\n📋 Recent option assignment/exercise activity:");
                for act in relevant {
                    println!("  {} {} {} qty={} @ {} on {}",
                        act.activity_type, act.side, act.symbol,
                        act.qty, act.price, act.date);
                    if act.activity_type == "OPASN" {
                        warn!("OPASN detected for {} — verify equity liquidation completed", act.symbol);
                    }
                }
            } else {
                println!("✅ No recent OPASN/OPEXC activity found");
            }
        }
        Err(e) => eprintln!("⚠️  Activity feed unavailable (non-critical): {}", e),
    }

    let mut ctrl_c_trade = std::pin::pin!(tokio::signal::ctrl_c());
    // ── Options market-hours gate ──────────────────────────────────────────
    // Options do not trade outside regular session hours; without this guard
    // the bot keeps generating and attempting to submit entry signals
    // pre/post-market, which Alpaca then rejects with a 422. Pessimistic
    // default (closed) until the first clock poll confirms the session is open.
    let mut market_open = false;
    let mut clock_poll = tokio::time::interval(tokio::time::Duration::from_secs(60));
    'trading: loop {
        let event = tokio::select! {
            biased;
            res = &mut ctrl_c_trade => {
                let _ = res;
                info!("Ctrl-C received -- shutting down trading bot gracefully.");
                break;
            }
            _ = clock_poll.tick() => {
                if let Ok(account) = client.get_account().await {
                    if let (Ok(equity_now), Ok(bp_now)) = (account.equity.parse::<f64>(), account.buying_power.parse::<f64>()) {
                        if equity_now.is_finite() && equity_now > 0.0 && bp_now.is_finite() && bp_now >= 0.0 {
                            pm.sync_from_account(equity_now, bp_now);
                            remaining_buy_pwr = bp_now;
                            last_account_refresh = Instant::now();
                        }
                    }
                }
                match client.get_clock().await {
                    Ok(clock) => {
                        use chrono::DateTime;
                        let now_utc = Utc::now();
                        // Belt-and-suspenders: also check `next_close` directly,
                        // since options stop trading at the session close even
                        // if `is_open` lags by a tick.
                        let session_closed = DateTime::parse_from_rfc3339(&clock.next_close)
                            .map(|close_ts| now_utc >= close_ts.with_timezone(&Utc))
                            .unwrap_or(false);
                        market_open = clock.is_open && !session_closed;
                    }
                    Err(e) => warn!("Market clock check failed: {} -- keeping prior state (open={})", e, market_open),
                }
                continue 'trading;
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
                    spot_price:    None,
                    iv_at_fill:    None,
                    delta_at_fill: None,
                    vega_at_fill:  None,
                    theta_at_fill: None,
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
                buf.push_back((price, Instant::now()));
                if buf.len() > max_price_buf { buf.pop_front(); }
                let warmed_up = buf.len() >= min_prices_for_hv;
                let entries_halted = circuit_broken || check_risk_guards(
                    equity, equity - estimated_daily_loss, trades_today, &daily_risk_limits).is_halt();

                // ── P3.1 Live IV cache read-path ───────────────────────────
                // Populated by the background refresh task spawned above; this
                // is just a quick lock + HashMap lookup, never a network call.
                if iv_cache.read().unwrap().get_cached_iv(&sym).is_none() {
                    debug!("No cached market IV for {} yet; background task will refresh", sym);
                }

                // Volatility + Heston params from rolling ticks.
                // `compute_historical_vol_ticks` annualizes from the buffer's
                // actual sampling rate (ticks arrive far more often than once a
                // day, so the daily-close √252 assumption in
                // `compute_historical_vol` would wildly over-annualize here).
                let samples: Vec<(f64, Instant)> = buf.iter().copied().collect();
                let prices: Vec<f64> = samples.iter().map(|(p, _)| *p).collect();
                let observed_sigma = compute_historical_vol_ticks(&samples);
                let valid_sigma = observed_sigma.is_finite() && observed_sigma >= 1e-8;
                // Management must run even before the tick buffer warms up.
                let sigma = if valid_sigma { observed_sigma } else { 0.25 };

                // ── Market vs. model IV — these must stay distinct inputs ──
                // `market_iv`: what the market is actually pricing options at.
                // Prefer the live options-chain reading (populated by the
                // background refresh task); fall back to the tick-based
                // realized-vol estimate only while no fresh reading is cached.
                let live_iv = iv_cache.read().unwrap().get_cached_iv(&sym);
                let market_iv = live_iv.unwrap_or(sigma);
                // `model_iv`: the Heston-calibrated theoretical fair value,
                // used by strategies to detect mispricing against `market_iv`.
                let model_iv = live_params
                    .read().unwrap()
                    .get(&sym)
                    .map(|p| p.v0.sqrt())
                    .unwrap_or_else(|| {
                        let h = heston_start(price, sigma, 1.0, 0.05);
                        h.v0.sqrt()
                    });

                // ── P2.1 Position close check (via shared manage_open_positions) ──
                if valid_sigma && price.is_finite() && price > 0.0 {
                    risk_marks.insert(sym.clone(), (price, sigma, Instant::now()));
                }
                let risk_ready = match marked_book(&open_positions, &risk_marks, Utc::now()) {
                    Ok(book) => { pm.update_positions(book); true }
                    Err(e) => { warn!("Portfolio marking blocks entries: {}", e); false }
                };
                if risk_ready {
                    let risk = pm.get_portfolio_risk();
                    port_delta = risk.total_delta; port_gamma = risk.total_gamma;
                    port_vega = risk.total_vega; port_theta = risk.total_theta;
                }
                if open_syms.contains(&sym) {
                    if let Some(pos) = open_positions.get(&sym).cloned() {
                        let snapshots = managed_positions(&pos, price, sigma);
                        let mgmt_actions = manage_open_positions(&snapshots, &mgmt_config,
                            equity - estimated_daily_loss, Utc::now().date_naive());
                        // Derive strike for roll targeting from existing OCC; fall back to spot
                        let occ_strike = pos.occ_symbol.as_deref()
                            .and_then(|occ| occ_parser::parse_occ(occ).map(|p| p.strike))
                            .unwrap_or(price);
                        let primary = management_action(&mgmt_actions, entries_halted, pos.legs.len() > 1);
                        let (close_reason_opt, roll_info): (Option<String>, Option<(f64, u32, i32)>) =
                            match primary.as_ref() {
                                None => (None, None),
                                Some(ManagementAction::Hold) | Some(ManagementAction::DeltaAlert { .. }) => (None, None),
                                Some(ManagementAction::Roll { new_dte_days, roll_number, .. }) =>
                                    (None, Some((occ_strike, *new_dte_days, *roll_number))),
                                Some(ManagementAction::ProfitTake { reason, .. }) =>
                                    (Some(format!("PROFIT_TAKE: {}", reason)), None),
                                Some(ManagementAction::DefensiveClose { reason, .. }) =>
                                    (Some(format!("DEFENSIVE_CLOSE: {}", reason)), None),
                                Some(ManagementAction::ForceCloseLong { .. }) =>
                                    (Some("FORCE_CLOSE_LONG_PREMIUM".to_string()), None),
                            };

                        if let Some((new_strike, new_dte_days, roll_number)) = roll_info {
                            if let Some(ref occ) = pos.occ_symbol.clone() {
                                info!("ROLL_TRIGGER [{}]: spot ${:.2} → roll #{} at K={:.2} +{}d",
                                    sym, price, roll_number, new_strike, new_dte_days);
                                println!("  🔄 Rolling {} — spot ${:.2} K=${:.2} (roll {}/{})",
                                    sym, price, new_strike, roll_number, max_rolls);
                                if !dry_run {
                                    let close_result = client.close_position_and_wait(occ).await;
                                    match close_result {
                                        Ok(_) => {
                                            println!("    ✅ Closed old leg {}", occ);
                                            use crate::alpaca::types::{OptionsOrderRequest, OrderType, TimeInForce, OrderSide};
                                            let (yy, mm, dd) = AlpacaClient::expiry_from_dte(new_dte_days as usize);
                                            let is_call = occ_parser::parse_occ(occ).map(|p| p.is_call).unwrap_or(false);
                                            let new_occ = AlpacaClient::occ_symbol(&sym, yy, mm, dd, is_call, new_strike);
                                            let resolved = client.resolve_single_leg_occ(&new_occ).await;
                                            let new_order = OptionsOrderRequest {
                                                r#type: OrderType::Market,
                                                time_in_force: TimeInForce::Day,
                                                symbol: Some(resolved.clone()),
                                                qty: Some(pos.qty.abs().to_string()),
                                                side: Some(OrderSide::Sell),
                                                position_intent: Some("sell_to_open".to_string()),
                                                order_class: None,
                                                legs: None,
                                                limit_price: None,
                                                client_order_id: None,
                                            };
                                            match client.submit_options_order_and_wait(&new_order).await {
                                                Ok(filled) => {
                                                    match position_from_fill(&filled, &sym, pos.strategy.clone(), &ts) {
                                                        Ok(Some(mut rolled_pos)) => {
                                                            rolled_pos.roll_count = roll_number;
                                                            if let Err(e) = store.upsert_position(&rolled_pos).await {
                                                                error!("DB roll upsert failed: {}", e);
                                                                circuit_broken = true;
                                                            }
                                                            open_positions.insert(sym.clone(), rolled_pos);
                                                        }
                                                        Ok(None) => {
                                                            let _ = store.close_position(&sym).await;
                                                            open_syms.remove(&sym);
                                                            open_positions.remove(&sym);
                                        if let Ok(book) = marked_book(&open_positions, &risk_marks, Utc::now()) { pm.update_positions(book); }
                                                            cooldown.record_close(&sym);
                                                        }
                                                        Err(e) => {
                                                            error!("Roll execution needs reconciliation: {}", e);
                                                            break 'trading;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("Roll submission/fill unresolved for {}: {}; stopping for reconciliation", sym, e);
                                                    break 'trading;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Roll close old leg failed for {} ({}): {}", sym, occ, e);
                                            eprintln!("    ⚠️  Roll close failed for {}: {}", sym, e);
                                        }
                                    }
                                } else {
                                    let (yy2, mm2, dd2) = AlpacaClient::expiry_from_dte(new_dte_days as usize);
                                    let preview = AlpacaClient::occ_symbol(&sym, yy2, mm2, dd2, false, new_strike);
                                    println!("    [DRY RUN] Would roll {} → {} (roll {}/{})",
                                        occ, preview, roll_number, max_rolls);
                                }
                            }
                            continue;
                        }

                        if let Some(close_reason) = close_reason_opt {
                                info!("Closing {} ({}): {}", sym, pos.strategy.as_deref().unwrap_or("?"), &close_reason);
                                println!("  🔒 Closing {} — {}", sym, close_reason);
                                if !dry_run {
                                    match close_tracked_position(&client, &pos).await {
                                        Ok(()) => {
                                            let _ = store.close_position(&sym).await;
                                            open_syms.remove(&sym);
                                            open_positions.remove(&sym);
                                        if let Ok(book) = marked_book(&open_positions, &risk_marks, Utc::now()) { pm.update_positions(book); }
                                            cooldown.record_close(&sym);
                                            println!("    Closed {}", sym);
                                        }
                                        Err(e) => {
                                            error!("Close incomplete for {}: {}; keeping position tracked", sym, e);
                                            circuit_broken = true;
                                        }
                                    }
                                } else {
                                    let close_sym = pos.occ_symbol.as_deref().unwrap_or(&sym);
                                    println!("    [DRY RUN] Would close {} ({})", sym, close_sym);
                                }
                                continue;
                        } // end close_reason_opt
                    }
                    // Position is open and not ready to close; skip new-entry signals
                    continue;
                }

                // Entry-only guards must never bypass management of open risk.
                if !risk_ready || last_account_refresh.elapsed().as_secs() > 120 || entries_halted || !warmed_up || !valid_sigma || !model_iv.is_finite()
                    || model_iv > 2.0 || !market_open {
                    continue;
                }
                let now = Instant::now();
                if let Some(&prev) = last_signal.get(&sym) {
                    if now.duration_since(prev).as_secs() < signal_cooldown_secs { continue; }
                }

                // ── Re-entry cooldown: skip if symbol was recently closed ─
                if cooldown.is_cooling_down(&sym) {
                    debug!("Skipping {} — re-entry cooldown ({:.0}s remaining)",
                        sym, cooldown.secs_remaining(&sym) as f64);
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

                // Generate signals. `market_iv` (live options-chain reading, or
                // the realized-vol fallback) vs. `model_iv` (Heston fair value)
                // — previously `sigma` was passed for both the market-IV *and*
                // historical-vol slots, so strategies never saw a real IV edge.
                let signals = registry.generate_all_signals(&sym, price, market_iv, model_iv, sigma);
                let actionable: Vec<_> = signals.iter()
                    .filter(|s| {
                        // Base confidence gate
                        if s.confidence < min_confidence { return false; }
                        if matches!(s.action, SignalAction::NoAction) { return false; }
                        if matches!(s.action, SignalAction::ClosePosition { .. }) { return false; }
                        // P2.3 performance-matrix gate: skip historically worst strategy
                        // Regime weight gate: skip strategies with weight < 0.5
                        let weight = RegimeDetector::weight_for(&regime, &s.strategy_name);
                        s.confidence * weight >= min_confidence
                    })
                    .collect();

                if actionable.is_empty() { continue; }
                last_signal.insert(sym.clone(), now);

                info!("Signal [{}] ${:.2} vol={:.1}% regime={:?} -- {} actionable signal(s)",
                    sym, price, sigma * 100.0, regime, actionable.len());
                println!("[{}]  ${:.2}  vol={:.1}%  regime={:?}", sym, price, sigma * 100.0, regime);
                for sig in &actionable {
                    // Resolve one strategy unit first. Budget its actual strikes/ratios,
                    // then size and validate this strategy's actual proposed quantity.
                    let rough_premium = (price * sigma * (sig.expiry_days as f64 / 365.0).sqrt()).max(0.01);
                    let qty = 1;
                    println!("  📊 [{:<22}]  {:?}  K=${:.2}  exp={}d  conf={:.0}%  edge=${:.2}",
                        sig.strategy_name, sig.action,
                        sig.strike, sig.expiry_days,
                        sig.confidence * 100.0, sig.edge);

                    match AlpacaClient::signal_to_options_order(&sig.action, &sym, qty, None) {
                        Ok(mut order) => {
                            // Resolve the generated OCC symbol to the nearest actually-listed contract.
                            // After resolution, apply an ITM guard: if the resolved put strike is at
                            // or above the current spot price, the contract is ATM/ITM — skip it to
                            // avoid entering positions with immediate assignment risk.
                            if let Some(ref raw_sym) = order.symbol.clone() {
                                let resolved = client.resolve_single_leg_occ(raw_sym).await;
                                // ── Order-path §5: reject any symbol Alpaca would 422 on ──
                                if let Err(e) = crate::order_path::validate_occ_symbol(&resolved) {
                                    warn!("Order path validation rejected {} — skipping: {}", resolved, e);
                                    println!("    ⚠️  Rejected {} ({})", resolved, e);
                                    continue;
                                }
                                let resolved_parts = occ_parser::parse_occ(&resolved);
                                let is_put = resolved_parts.as_ref().map(|p| !p.is_call).unwrap_or(false);
                                let resolved_strike = resolved_parts.as_ref().map(|p| p.strike);
                                if is_put {
                                    if let Some(rs) = resolved_strike {
                                        // Reject if the resolved put strike is within 0.5% of
                                        // spot (ATM) or above (ITM).
                                        if rs >= price * 0.995 {
                                            warn!(
                                                "ITM guard: skipping {} — resolved put ${:.2} >= spot ${:.2}",
                                                resolved, rs, price
                                            );
                                            println!(
                                                "    ⚠️  ITM guard: skipping {} — put K=${:.2} is ATM/ITM (spot ${:.2})",
                                                sym, rs, price
                                            );
                                            continue;
                                        }
                                    }
                                }
                                order.symbol = Some(resolved);
                            }
                            // For multi-leg orders (mleg) resolve each leg's OCC symbol to
                            // the nearest actually-listed contract. order.symbol is None for
                            // mleg orders so the block above is a no-op for them.
                            if let Some(ref mut legs) = order.legs {
                                let mut reject_reason: Option<String> = None;
                                for leg in legs.iter_mut() {
                                    let raw = leg.symbol.clone();
                                    let resolved = client.resolve_single_leg_occ(&raw).await;
                                    if let Err(e) = crate::order_path::validate_occ_symbol(&resolved) {
                                        reject_reason = Some(format!("{resolved}: {e}"));
                                        break;
                                    }
                                    leg.symbol = resolved;
                                }
                                if let Some(reason) = reject_reason {
                                    warn!("Order path validation rejected multi-leg order for {} — skipping: {}", sym, reason);
                                    println!("    ⚠️  Rejected multi-leg order for {} ({})", sym, reason);
                                    continue;
                                }
                            }
                            let per_unit = match resolved_entry_budget(&order, rough_premium) {
                                Ok(cost) => cost,
                                Err(e) => { warn!("Entry rejected [{}]: {}", sym, e); continue; }
                            };
                            let qty = pm.calculate_position_size(per_unit / 100.0, sigma, None, None, None)
                                .min((pm.capital() * pm.config().max_risk_per_trade / 100.0 / per_unit).floor() as i32);
                            if qty <= 0 { warn!("No affordable entry size [{}]", sym); continue; }
                            let decision = pm.can_take_position(&sig.strategy_name, per_unit / 100.0, sigma, qty);
                            if !decision.can_trade {
                                warn!("Risk gate [{}]: {:?}", sym, decision.risk_warnings);
                                continue;
                            }
                            let qty = qty as u32;
                            order.qty = Some(qty.to_string());
                            let estimated_margin = per_unit * qty as f64;
                            if !estimated_margin.is_finite() { continue; }
                            let mut projected = open_positions.clone();
                            let candidate = match candidate_record(&order, &sym, qty) {
                                Ok(p) => p, Err(e) => { warn!("Invalid candidate: {}", e); continue; }
                            };
                            projected.insert(sym.clone(), candidate);
                            let book = match marked_book(&projected, &risk_marks, Utc::now()) {
                                Ok(book) => book, Err(e) => { warn!("Unmarked candidate: {}", e); continue; }
                            };
                            let projected_warnings = pm.projected_risk_warnings(&book);
                            if !projected_warnings.is_empty() {
                                warn!("Projected entry risk [{}]: {:?}", sym, projected_warnings);
                                continue;
                            }
                            let invariants = assert_invariants(&BotState {
                                positions: projected.values().flat_map(invariant_positions).collect(),
                                equity: pm.capital(), start_of_day_equity: equity,
                                was_circuit_broken: circuit_broken, circuit_broken,
                                max_risk_capital_pct: bot_cfg.max_daily_loss_pct,
                                max_daily_drawdown_pct: bot_cfg.max_daily_loss_pct,
                                block_long_premium: true, protected_equity: protected_equity_set.clone(),
                            });
                            if !invariants.is_empty() {
                                warn!("Projected invariant violation [{}]: {:?}", sym, invariants);
                                continue;
                            }
                            if dry_run {
                                println!("    [DRY RUN] Would submit {} x {} (budget ${:.2})", qty, sym, estimated_margin);
                                continue;
                            }
                            if estimated_margin > 0.0 && estimated_margin > remaining_buy_pwr {
                                warn!("Buying power check: skipping {} — estimated margin ${:.2} > remaining ${:.2}",
                                    sym, estimated_margin, remaining_buy_pwr);
                                println!("    ⚠️  Insufficient buying power: need ${:.2}, have ${:.2} — skipping",
                                    estimated_margin, remaining_buy_pwr);
                                continue;
                            }
                            // Pre-flight circuit breaker: reject before submitting if
                            // this order would push estimated 1-day risk over the limit.
                            // Use 1-day 1σ VaR (price × daily_vol × notional) rather than
                            // the BSM option premium.  For credit strategies the premium is
                            // income, not a cost, so comparing it against a loss limit is
                            // inverted and blocks valid trades (e.g. CSP on high-IV stocks).
                            let daily_vol = sigma / (252.0_f64).sqrt();
                            let order_cost = price * daily_vol * qty as f64 * 100.0;
                            if estimated_daily_loss + order_cost >= max_daily_loss {
                                circuit_broken = true;
                                error!("CIRCUIT BREAKER (pre-flight): skipping {} — projected spend ${:.2} >= limit ${:.2}",
                                    sym, estimated_daily_loss + order_cost, max_daily_loss);
                                eprintln!("    🚫 Circuit breaker: would exceed daily limit — order cancelled");
                                let a = alerter.clone();
                                let (edl, mdl) = (estimated_daily_loss + order_cost, max_daily_loss);
                                tokio::spawn(async move { a.circuit_breaker(edl, mdl).await; });
                                continue;
                            }
                            match client.submit_options_order_and_wait(&order).await {
                                Ok(filled) => {
                                    let pos = match position_from_fill(&filled, &sym, Some(sig.strategy_name.clone()), &ts) {
                                        Ok(Some(pos)) => pos,
                                        Ok(None) => {
                                            info!("Order {} ended {} without fills", filled.id, filled.status);
                                            continue;
                                        }
                                        Err(e) => {
                                            error!("Execution needs reconciliation: {}", e);
                                            break 'trading;
                                        }
                                    };
                                    let executed_qty = filled.filled_qty.parse::<f64>().unwrap_or(0.0);
                                    let fill_price = pos.entry_price;
                                    // Track exposure even if the database write later fails.
                                    open_syms.insert(sym.clone());
                                    open_positions.insert(sym.clone(), pos.clone());
                                    info!("Order submitted: id={} sym={} status={} strategy={}",
                                        filled.id, sym, filled.status, sig.strategy_name);
                                    println!("    Submitted: {} ({})", filled.id, filled.status);
                                    // ── P3.3 Greeks / portfolio-risk alert ────────────
                                    if let Ok(book) = marked_book(&open_positions, &risk_marks, Utc::now()) {
                                        pm.update_positions(book);
                                    } else {
                                        circuit_broken = true;
                                    }
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
                                    let delta_limit = pm.capital() * pm.config().risk_limits.max_portfolio_delta;
                                    if risk.gross_delta_dollars > delta_limit {
                                        warn!("DELTA HEDGE ALERT: |Δ|={:.3} exceeds limit {:.3} -- consider hedging",
                                            risk.gross_delta_dollars, delta_limit);
                                        println!("  ⚠️  DELTA HEDGE ALERT: |Δ|={:.3} exceeds {:.3} — consider hedging",
                                            risk.gross_delta_dollars, delta_limit);
                                    }

                                    let daily_vol_post = sigma / (252.0_f64).sqrt();
                                    estimated_daily_loss += price * daily_vol_post * executed_qty * 100.0;

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
                                    trades_today   += 1;
                                    let desc = format!("{} {:?} @ ${:.2}  {}",
                                        sig.strategy_name, sig.action, price,
                                        chrono::Utc::now().format("%H:%M:%S"));
                                    last_signal_desc.insert(sym.clone(), desc);
                                    if let Err(e) = store.upsert_position(&pos).await {
                                        error!("DB position upsert failed: {}", e);
                                        eprintln!("DB position error: {}", e);
                                        circuit_broken = true;
                                    } else {
                                        open_syms.insert(sym.clone());
                                        open_positions.insert(sym.clone(), pos);
                                        // Deduct estimated margin so subsequent pre-flight
                                        // checks see the reduced available buying power.
                                        remaining_buy_pwr -= per_unit * executed_qty;
                                        // Fill alert
                                        let a = alerter.clone();
                                        let sym_c  = sym.clone();
                                        let strat_c = sig.strategy_name.clone();
                                        let (qty_c, price_c) = (executed_qty as u32, fill_price);
                                        tokio::spawn(async move { a.fill(&sym_c, &strat_c, qty_c, price_c).await; });
                                        // ── Fill-time snapshot: log Greeks + IV to trades table ──
                                        let dte_years = (sig.expiry_days as f64 / 365.0).max(1.0 / 365.0);
                                        let fill_greeks = match &sig.action {
                                            crate::strategies::SignalAction::BuyCall { strike, .. }
                                            | crate::strategies::SignalAction::SellCall { strike, .. } =>
                                                Some(black_scholes_call(price, *strike, dte_years, 0.05, sigma)),
                                            crate::strategies::SignalAction::BuyPut { strike, .. }
                                            | crate::strategies::SignalAction::SellPut { strike, .. }
                                            | crate::strategies::SignalAction::CashSecuredPut { strike, .. } =>
                                                Some(black_scholes_put(price, *strike, dte_years, 0.05, sigma)),
                                            _ => None,
                                        };
                                        let fill_rec = persistence::TradeRecord {
                                            symbol:        sym.clone(),
                                            action:        format!("{:?}", sig.action),
                                            quantity:      executed_qty,
                                            price:         fill_price,
                                            order_id:      Some(filled.id.clone()),
                                            fill_status:   Some(filled.status.clone()),
                                            strategy:      Some(sig.strategy_name.clone()),
                                            error_message: None,
                                            timestamp:     ts.clone(),
                                            spot_price:    Some(price),
                                            iv_at_fill:    Some(sigma),
                                            delta_at_fill: fill_greeks.map(|g| g.delta),
                                            vega_at_fill:  fill_greeks.map(|g| g.vega),
                                            theta_at_fill: fill_greeks.map(|g| g.theta),
                                        };
                                        if let Err(e) = store.insert_trade(&fill_rec).await {
                                            error!("Fill record DB write failed: {}", e);
                                        }
                                        // ── Post-fill invariant check ─────────────────────
                                        {
                                            let inv_positions: Vec<InvariantPosition> = open_positions.values()
                                                .flat_map(invariant_positions).collect();
                                            let inv_state = BotState {
                                                positions:              inv_positions,
                                                equity:                 equity - estimated_daily_loss,
                                                start_of_day_equity:    equity,
                                                was_circuit_broken:     circuit_broken,
                                                circuit_broken,
                                                max_risk_capital_pct:   bot_cfg.max_daily_loss_pct,
                                                max_daily_drawdown_pct: bot_cfg.max_daily_loss_pct,
                                                block_long_premium:     true,
                                                protected_equity:       protected_equity_set.clone(),
                                            };
                                            let violations = assert_invariants(&inv_state);
                                            if !violations.is_empty() {
                                                error!("INVARIANT VIOLATIONS AFTER FILL — flattening all risk:");
                                                let violation_strings: Vec<String> = violations.iter().map(|v| v.to_string()).collect();
                                                for v in &violation_strings {
                                                    error!("  {}", v);
                                                }
                                                circuit_broken = true;
                                                eprintln!("⛔ Invariant violated — flattening all open risk, circuit breaker tripped permanently. Requires manual review.");
                                                // ── Flatten all open risk (plan §4: on failure the bot must
                                                // flatten, trip the breaker, alert, and log the full state) ──
                                                if !dry_run {
                                                    for position in open_positions.values().cloned().collect::<Vec<_>>() {
                                                        match close_tracked_position(&client, &position).await {
                                                            Ok(()) => {
                                                                let _ = store.close_position(&position.symbol).await;
                                                                open_positions.remove(&position.symbol);
                                                                open_syms.remove(&position.symbol);
                                                            }
                                                            Err(e) => error!("Flatten incomplete for {}: {}; retaining position for management", position.symbol, e),
                                                        }
                                                    }
                                                }
                                                let a = alerter.clone();
                                                tokio::spawn(async move { a.invariant_violation(&violation_strings).await; });
                                            }
                                        }
                                        // Position recorded — stop processing further signals for
                                        // this symbol on this tick to prevent duplicate entries.
                                        break;
                                    }

                                    if circuit_broken { break; }

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
                                        spot_price:    None,
                                        iv_at_fill:    None,
                                        delta_at_fill: None,
                                        vega_at_fill:  None,
                                        theta_at_fill: None,
                                    };
                                    let _ = store.insert_trade(&fail_rec).await;
                                    break 'trading; // Broker outcome must be reconciled before new submissions.
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

#[cfg(test)]
mod math_tests {
    use super::*;
    use crate::alpaca::types::{OptionsOrderRequest, OptionsLeg, OrderSide, OrderType, TimeInForce};

    fn order(legs: Vec<OptionsLeg>) -> OptionsOrderRequest {
        OptionsOrderRequest { r#type: OrderType::Market, time_in_force: TimeInForce::Day,
            symbol: None, qty: Some("1".into()), side: None, position_intent: None,
            order_class: Some("mleg".into()), legs: Some(legs), limit_price: None, client_order_id: None }
    }
    fn leg(symbol: &str, side: OrderSide) -> OptionsLeg {
        OptionsLeg { symbol: symbol.into(), side, ratio_qty: 1, position_intent: "open".into() }
    }

    #[test]
    fn resolved_straddle_is_rejected_and_spread_uses_actual_strikes() {
        let naked = order(vec![leg("MSFT261218C00500000", OrderSide::Sell),
            leg("MSFT261218P00500000", OrderSide::Sell)]);
        assert!(resolved_entry_budget(&naked, 2.0).is_err());
        let spread = order(vec![leg("MSFT261218C00500000", OrderSide::Sell),
            leg("MSFT261218C00510000", OrderSide::Buy)]);
        assert_eq!(resolved_entry_budget(&spread, 2.0).unwrap(), 1200.0);
        let mixed = order(vec![leg("MSFT261218C00500000", OrderSide::Sell),
            leg("MSFT261224C00510000", OrderSide::Buy)]);
        assert!(resolved_entry_budget(&mixed, 2.0).is_err());
    }

    #[test]
    fn risk_projection_preserves_ratios_signs_equity_and_missing_marks() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-28T15:00:00Z").unwrap().with_timezone(&Utc);
        let mut spread = order(vec![leg("MSFT261218C00500000", OrderSide::Sell),
            leg("MSFT261218C00510000", OrderSide::Buy)]);
        spread.legs.as_mut().unwrap()[1].ratio_qty = 2;
        let record = candidate_record(&spread, "MSFT", 3).unwrap();
        let mut records = HashMap::from([("MSFT".into(), record)]);
        assert!(marked_book(&records, &HashMap::new(), now).is_err());
        let marks = HashMap::from([("MSFT".into(), (500.0,0.3,Instant::now()))]);
        let book = marked_book(&records, &marks, now).unwrap();
        assert_eq!(book.iter().map(|p|p.quantity).collect::<Vec<_>>(),vec![-3,6]);
        assert!(book.iter().all(|p| p.entry_greeks.unwrap().price.is_finite()));
        records.get_mut("MSFT").unwrap().legs.clear();
        records.get_mut("MSFT").unwrap().qty=25.0;
        let book=marked_book(&records,&marks,now).unwrap();
        let risk=crate::portfolio::RiskAnalyzer::new(100_000.0,Default::default()).calculate_portfolio_greeks(&book);
        assert_eq!(risk.total_delta,25.0);
        assert_eq!(risk.net_exposure,12500.0);
        let stale=HashMap::from([("MSFT".into(),(500.0,0.3,Instant::now()-std::time::Duration::from_secs(121)))]);
        assert!(marked_book(&records,&stale,now).is_err());
    }
}
