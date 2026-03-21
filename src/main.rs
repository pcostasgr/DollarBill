// Main entry point - Modular Options Pricing System
// The binary re-compiles all modules directly (mod declarations below),
// so many pub library items appear "unused" from main's perspective.
#![allow(dead_code, unused_imports)]

mod models;
mod market_data;
mod strategies;
mod utils;
mod calibration;
mod backtesting;
mod alpaca;
mod config;
mod analysis;
mod portfolio;
mod streaming;
mod persistence;

use clap::{Parser, Subcommand};
use log::{info, warn, error};
use market_data::csv_loader::{load_csv_closes, HistoricalDay};
use models::bs_mod::{compute_historical_vol, black_scholes_call};
use models::heston::{heston_start, MonteCarloConfig, HestonParams, HestonMonteCarlo};
use models::heston_analytical::{
    heston_call_carr_madan, classify_moneyness, Moneyness,
    heston_call_otm, heston_call_itm, heston_put_otm, heston_put_itm,
    heston_put_carr_madan,
};
use strategies::{StrategyRegistry, vol_mean_reversion::VolMeanReversion};
use calibration::{create_mock_market_data, calibrate_heston, CalibParams};
use utils::action_table_out;
use utils::pnl_output;
use std::time::{Duration, Instant};

// ─── CLI definition ────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "dollarbill", version, about = "Options pricing & algorithmic trading toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the interactive demo (default behaviour)
    Demo {
        /// Underlying symbol (CSV must exist in data/)
        #[arg(long, default_value = "TSLA")]
        symbol: String,
    },

    /// Price an ATM option using Heston/BSM models
    Price {
        /// Underlying symbol — reads data/{symbol}_*.csv for implied vol
        symbol: String,
        /// Strike price in dollars
        strike: f64,
        /// Time to expiry in years (0.25 = ~3 months)
        #[arg(long, default_value_t = 1.0)]
        dte: f64,
        /// Risk-free rate (0.05 = 5 %)
        #[arg(long, default_value_t = 0.05)]
        rate: f64,
    },

    /// Run backtests across configured symbols and optionally update the
    /// performance matrix used by strategy matching.
    Backtest {
        /// Limit to a single symbol — runs all configured symbols if absent
        #[arg(long)]
        symbol: Option<String>,
        /// Persist results to models/performance_matrix.json
        #[arg(long)]
        save: bool,
    },

    /// Print trading signals for configured symbols
    Signals {
        /// Limit to a single symbol
        #[arg(long)]
        symbol: Option<String>,
        /// Subscribe to the Alpaca live stream and generate signals in real time
        #[arg(long)]
        live: bool,
    },

    /// Calibrate the Heston model against synthetic market data for a symbol
    Calibrate {
        /// Symbol name (e.g. TSLA)
        symbol: String,
    },

    /// Start the paper-trading bot
    /// Requires ALPACA_API_KEY and ALPACA_API_SECRET environment variables.
    Trade {
        /// Print orders but do not submit them to Alpaca
        #[arg(long)]
        dry_run: bool,
        /// Stream live prices via Alpaca WebSocket and persist fills to SQLite
        #[arg(long)]
        live: bool,
    },
}

// ─── Entry point ───────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_secs()
        .init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Demo { symbol } => cmd_demo(&symbol),
        Commands::Price { symbol, strike, dte, rate } => cmd_price(&symbol, strike, dte, rate),
        Commands::Backtest { symbol, save } => cmd_backtest(symbol.as_deref(), save),
        Commands::Signals { symbol, live } => cmd_signals(symbol.as_deref(), live).await,
        Commands::Calibrate { symbol } => cmd_calibrate(&symbol),
        Commands::Trade { dry_run, live } => cmd_trade(dry_run, live).await,
    }
}

// ─── Subcommand: demo ──────────────────────────────────────────────────────

fn cmd_demo(symbol: &str) {
    println!("{}", "=".repeat(70));
    println!("    BLACK-SCHOLES & HESTON OPTIONS PRICING SYSTEM");
    println!("    Modular Architecture with Carr-Madan Analytical Pricing");
    println!("{}", "=".repeat(70));

    let n_days = 10;
    let rate = 0.05;
    let time_to_maturity = 1.0;

    let csv_path = find_csv(symbol);
    let start = Instant::now();
    let history = match load_csv_closes(&csv_path) {
        Ok(h) => { println!("\n✓ Loaded {} from {}", symbol, csv_path); h }
        Err(e) => { println!("✗ CSV load failed: {}", e); return; }
    };
    let load_time = start.elapsed();

    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let sigma = compute_historical_vol(&closes);
    let current_price = *closes.last().unwrap();
    let heston_params = heston_start(current_price, sigma, time_to_maturity, rate);

    demo_market_analysis(&history, n_days, sigma, load_time);
    let (_, carr_madan_time) =
        demo_analytical_pricing(current_price, time_to_maturity, rate, sigma, &heston_params);
    let speedup = demo_monte_carlo(heston_params.clone(), current_price, carr_madan_time);
    demo_strategy_signals(symbol, current_price, sigma, &heston_params);
    demo_calibration(current_price, rate);
    demo_summary(speedup);
}

// ─── Subcommand: price ─────────────────────────────────────────────────────

fn cmd_price(symbol: &str, strike: f64, dte: f64, rate: f64) {
    let csv_path = find_csv(symbol);
    let history = match load_csv_closes(&csv_path) {
        Ok(h) => h,
        Err(e) => { eprintln!("Failed to load {}: {}", csv_path, e); return; }
    };
    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let sigma = compute_historical_vol(&closes);
    let spot = *closes.last().unwrap();
    let heston_params = heston_start(spot, sigma, dte, rate);

    let moneyness = classify_moneyness(strike, spot, 0.05);
    let heston_price = heston_call_carr_madan(spot, strike, dte, rate, &heston_params);
    let bs = black_scholes_call(spot, strike, dte, rate, sigma);

    println!("\n{}", "=".repeat(60));
    println!("OPTION PRICING  —  {} @ ${:.2}", symbol, spot);
    println!("{}", "=".repeat(60));
    println!("Strike:       ${:.2}  ({})", strike, format!("{:?}", moneyness));
    println!("DTE (years):   {:.4}", dte);
    println!("Rate:          {:.2}%", rate * 100.0);
    println!("Hist. Vol:     {:.2}%", sigma * 100.0);
    println!();
    println!("Heston (Carr-Madan):  ${:.4}", heston_price);
    println!("Black-Scholes:        ${:.4}", bs.price);
    println!("  Delta {:+.4}  Gamma {:.4}  Vega {:.4}  Theta {:.4}",
             bs.delta, bs.gamma, bs.vega, bs.theta);
}

// ─── Subcommand: backtest ──────────────────────────────────────────────────

fn cmd_backtest(symbol: Option<&str>, save: bool) {
    use backtesting::engine::{BacktestEngine, BacktestConfig};
    use analysis::performance_matrix::{PerformanceMatrix,
        PerformanceMetrics as PerfMetrics};
    use market_data::symbols::load_stocks_with_sectors;

    let symbols: Vec<String> = match symbol {
        Some(s) => vec![s.to_string()],
        None => match load_stocks_with_sectors() {
            Ok(v) => v.into_iter().map(|(sym, _)| sym).collect(),
            Err(e) => { eprintln!("Failed to load symbols: {}", e); return; }
        },
    };

    // Load or start fresh performance matrix
    let mut matrix = PerformanceMatrix::load_from_file("models/performance_matrix.json")
        .unwrap_or_else(|_| PerformanceMatrix::new());

    let strategies: &[(&str, f64)] = &[
        ("Momentum",         0.20),
        ("Mean Reversion",   0.35),
        ("Breakout",         0.15),
        ("Vol Arbitrage",    0.30),
        ("Cash-Secured Puts",0.25),
    ];

    println!("{}", "=".repeat(70));
    println!("RUNNING BACKTESTS");
    println!("{}", "=".repeat(70));

    let mut ran = 0usize;
    for sym in &symbols {
        let csv_path = find_csv(sym);
        let history = match load_csv_closes(&csv_path) {
            Ok(h) => h,
            Err(_) => {
                println!("  ⚠  {} — no CSV data, skipping", sym);
                continue;
            }
        };

        println!("\n📊 {}  ({} trading days)", sym, history.len());

        for &(strategy_name, vol_threshold) in strategies {
            let config = BacktestConfig {
                initial_capital: 100_000.0,
                ..BacktestConfig::default()
            };
            let mut engine = BacktestEngine::new(config);
            let result = engine.run_simple_strategy(sym, history.clone(), vol_threshold);
            let m = &result.metrics;

            // Convert backtesting::PerformanceMetrics → analysis::PerformanceMetrics
            let pm = PerfMetrics {
                total_return: m.total_return_pct / 100.0 + 1.0,
                sharpe_ratio: m.sharpe_ratio,
                max_drawdown: m.max_drawdown_pct / 100.0,
                win_rate: m.win_rate,
                profit_factor: m.profit_factor,
                total_trades: m.total_trades,
                avg_holding_period: m.avg_days_held,
            };

            println!("  [{:>22}]  Sharpe {:+.2}  Return {:+.1}%  MaxDD {:.1}%  Trades {}",
                     strategy_name,
                     pm.sharpe_ratio,
                     (pm.total_return - 1.0) * 100.0,
                     pm.max_drawdown * 100.0,
                     pm.total_trades);

            matrix.add_result(sym, strategy_name, pm);
            ran += 1;
        }
    }

    println!("\n✅ Ran {} backtests across {} symbols", ran, symbols.len());

    if save {
        std::fs::create_dir_all("models").ok();
        match matrix.save_to_file("models/performance_matrix.json") {
            Ok(_) => println!("💾 Saved  →  models/performance_matrix.json"),
            Err(e) => eprintln!("❌ Save failed: {}", e),
        }
    } else {
        println!("(Pass --save to persist results to models/performance_matrix.json)");
    }
}

// ─── Subcommand: signals ───────────────────────────────────────────────────

async fn cmd_signals(symbol: Option<&str>, live: bool) {
    if live {
        cmd_signals_live(symbol).await;
        return;
    }

    use market_data::symbols::load_stocks_with_sectors;
    use strategies::matching::StrategyMatcher;

    let symbols: Vec<String> = match symbol {
        Some(s) => vec![s.to_string()],
        None => match load_stocks_with_sectors() {
            Ok(v) => v.into_iter().map(|(s, _)| s).collect(),
            Err(e) => { eprintln!("Failed to load stocks: {}", e); return; }
        },
    };

    println!("{}", "=".repeat(70));
    println!("TRADING SIGNALS");
    println!("{}", "=".repeat(70));

    // Load the strategy matcher — use saved models if available
    let matcher = StrategyMatcher::load_from_files(
        "models/stock_classifier.json",
        "models/performance_matrix.json",
    ).unwrap_or_else(|_| StrategyMatcher::new());

    for sym in &symbols {
        let csv_path = find_csv(sym);
        let history = match load_csv_closes(&csv_path) {
            Ok(h) => h,
            Err(_) => {
                println!("  ⚠  {} — no data", sym);
                continue;
            }
        };
        let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
        let sigma = compute_historical_vol(&closes);
        let spot = *closes.last().unwrap();
        let rate = 0.05;
        let heston_params = heston_start(spot, sigma, 1.0, rate);

        let recs = matcher.get_recommendations(sym);
        println!("\n[{}]  spot=${:.2}  vol={:.1}%", sym, spot, sigma * 100.0);
        if recs.confidence_score > 0.0 {
            println!("  Recommended: {}  (confidence {:.0}%)",
                     recs.recommended_strategy, recs.confidence_score * 100.0);
        }

        let mut reg = StrategyRegistry::new();
        reg.register(Box::new(VolMeanReversion::new()));
        let signals = reg.generate_all_signals(sym, spot, sigma, heston_params.v0.sqrt(), sigma);
        for sig in &signals {
            println!("  {:?}  strike=${:.2}  exp={}d  conf={:.0}%  edge=${:.2}",
                     sig.action, sig.strike, sig.expiry_days,
                     sig.confidence * 100.0, sig.edge);
        }
    }
}

/// Subscribe to the Alpaca live stream and print signals as prices arrive.
async fn cmd_signals_live(symbol: Option<&str>) {
    use market_data::symbols::load_stocks_with_sectors;
    use strategies::{
        SignalAction, StrategyRegistry,
        momentum::MomentumStrategy,
        mean_reversion::MeanReversionStrategy,
        breakout::BreakoutStrategy,
        vol_arbitrage::VolatilityArbitrageStrategy,
        cash_secured_puts::CashSecuredPuts,
    };

    let symbols: Vec<String> = match symbol {
        Some(s) => vec![s.to_string()],
        None => match load_stocks_with_sectors() {
            Ok(v) => v.into_iter().map(|(s, _)| s).collect(),
            Err(e) => { eprintln!("Failed to load symbols: {}", e); return; }
        },
    };

    println!("{}", "=".repeat(70));
    println!("LIVE SIGNALS  ({} symbols)", symbols.len());
    println!("{}", "=".repeat(70));

    let mut stream = match streaming::AlpacaStream::connect_from_env(&symbols).await {
        Ok(s) => s,
        Err(e) => { eprintln!("WebSocket connect failed: {}", e); return; }
    };
    println!("✅ Stream connected — press Ctrl-C to stop\n");

    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(MomentumStrategy::new()));
    registry.register(Box::new(MeanReversionStrategy::new()));
    registry.register(Box::new(BreakoutStrategy::new()));
    registry.register(Box::new(VolatilityArbitrageStrategy::new()));
    registry.register(Box::new(CashSecuredPuts::new()));

    let mut price_buf: std::collections::HashMap<String, std::collections::VecDeque<f64>>
        = std::collections::HashMap::new();
    let mut last_signal: std::collections::HashMap<String, Instant>
        = std::collections::HashMap::new();
    const SIGNAL_COOLDOWN_SECS: u64   = 300;
    const MIN_PRICES_FOR_HV:    usize = 22;
    const MAX_PRICE_BUF:        usize = 50;
    const MIN_CONFIDENCE:       f64   = 0.60;

    // Graceful Ctrl-C shutdown
    let mut ctrl_c_sig = std::pin::pin!(tokio::signal::ctrl_c());
    loop {
        let event = tokio::select! {
            biased;
            res = &mut ctrl_c_sig => {
                let _ = res;
                println!("[STOP] Signal stream shutting down gracefully.");
                break;
            }
            ev = stream.next_event() => ev,
        };
        match event {
            Some(streaming::MarketEvent::Trade(t)) => {
                let sym = t.symbol.clone();
                let buf = price_buf.entry(sym.clone())
                    .or_insert_with(std::collections::VecDeque::new);
                buf.push_back(t.price);
                if buf.len() > MAX_PRICE_BUF { buf.pop_front(); }
                if buf.len() < MIN_PRICES_FOR_HV { continue; }

                let now = Instant::now();
                if let Some(&prev) = last_signal.get(&sym) {
                    if now.duration_since(prev).as_secs() < SIGNAL_COOLDOWN_SECS { continue; }
                }

                let prices: Vec<f64> = buf.iter().copied().collect();
                let sigma = compute_historical_vol(&prices);
                if sigma < 1e-8 { continue; }
                let spot = t.price;
                let heston = heston_start(spot, sigma, 1.0, 0.05);
                let model_iv = heston.v0.sqrt();

                let signals = registry.generate_all_signals(&sym, spot, sigma, model_iv, sigma);
                let actionable: Vec<_> = signals.iter()
                    .filter(|s| {
                        s.confidence >= MIN_CONFIDENCE
                            && !matches!(s.action, SignalAction::NoAction)
                            && !matches!(s.action, SignalAction::ClosePosition { .. })
                    })
                    .collect();

                if actionable.is_empty() { continue; }
                last_signal.insert(sym.clone(), now);

                println!("\n[{}]  ${:.2}  vol={:.1}%", sym, spot, sigma * 100.0);
                for sig in &actionable {
                    println!("  📊 [{:<22}]  {:?}  K=${:.2}  exp={}d  conf={:.0}%  edge=${:.2}",
                        sig.strategy_name, sig.action,
                        sig.strike, sig.expiry_days,
                        sig.confidence * 100.0, sig.edge);
                }
            }
            // Suppress quote noise in signals-only mode
            Some(streaming::MarketEvent::Quote(_)) => {}
            Some(streaming::MarketEvent::Reconnected) => {
                println!("🔄 Stream reconnected — resuming signals.");
            }
            Some(streaming::MarketEvent::Disconnected) | None => {
                println!("❌ Stream permanently disconnected.");
                break;
            }
        }
    }
}

// ─── Subcommand: calibrate ─────────────────────────────────────────────────

fn cmd_calibrate(symbol: &str) {
    let csv_path = find_csv(symbol);
    let history = match load_csv_closes(&csv_path) {
        Ok(h) => h,
        Err(e) => { eprintln!("Failed to load {}: {}", csv_path, e); return; }
    };
    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let spot = *closes.last().unwrap();
    let rate = 0.05;
    demo_calibration(spot, rate);
}

// ─── Subcommand: trade ─────────────────────────────────────────────────────

async fn cmd_trade(dry_run: bool, live: bool) {
    use alpaca::AlpacaClient;
    use market_data::symbols::load_enabled_stocks;
    use portfolio::{PortfolioManager, PortfolioConfig};
    use strategies::{
        SignalAction, StrategyRegistry,
        momentum::MomentumStrategy,
        mean_reversion::MeanReversionStrategy,
        breakout::BreakoutStrategy,
        vol_arbitrage::VolatilityArbitrageStrategy,
        cash_secured_puts::CashSecuredPuts,
    };

    let client = match AlpacaClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Alpaca credentials not found: {}", e);
            eprintln!("   Set ALPACA_API_KEY and ALPACA_API_SECRET env vars.");
            return;
        }
    };

    let symbols = match load_enabled_stocks() {
        Ok(s) => s,
        Err(e) => { eprintln!("Failed to load symbols: {}", e); return; }
    };

    println!("{}", "=".repeat(70));
    println!("PAPER TRADING BOT{}{}",
             if dry_run { "  [DRY RUN]" } else { "" },
             if live    { "  [LIVE STREAM]" } else { "" });
    println!("{}", "=".repeat(70));

    // Fetch and display account status
    let account = match client.get_account().await {
        Ok(a) => a,
        Err(e) => { eprintln!("Failed to fetch account: {}", e); return; }
    };
    println!("Account: {}  equity=${:.2}  buying_power=${:.2}",
             account.status,
             account.equity_f64().unwrap_or(0.0),
             account.buying_power_f64().unwrap_or(0.0));

    if live {
        let equity  = account.equity_f64().unwrap_or(100_000.0);
        let buy_pwr = account.buying_power_f64().unwrap_or(equity);

        // ── Portfolio manager — shared risk gate for all orders ───────────
        let mut pm = PortfolioManager::new(PortfolioConfig {
            initial_capital: equity,
            ..PortfolioConfig::default()
        });
        pm.sync_from_account(equity, buy_pwr);

        // ── Persistent trade store ────────────────────────────────────────
        let store = match persistence::TradeStore::new("data/trades.db").await {
            Ok(s) => s,
            Err(e) => { eprintln!("Failed to open trade database: {}", e); return; }
        };

        // ── Restore open positions from previous sessions ─────────────────
        let persisted = store.get_open_positions().await.unwrap_or_default();
        if !persisted.is_empty() {
            println!("\n📂  {} persisted position(s) restored:", persisted.len());
            for p in &persisted {
                println!("     {} qty={:.0} @ ${:.2}  [{}]",
                    p.symbol, p.qty, p.entry_price,
                    p.strategy.as_deref().unwrap_or("—"));
            }
        }

        // ── Reconcile SQLite state against live Alpaca positions ──────────
        println!("\n🔄 Reconciling positions with Alpaca…");
        match client.get_positions().await {
            Ok(alpaca_pos) => {
                let alpaca_syms: std::collections::HashSet<String> =
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
                let sqlite_syms: std::collections::HashSet<String> =
                    persisted.iter().map(|p| p.symbol.clone()).collect();
                for ap in &alpaca_pos {
                    if !sqlite_syms.contains(&ap.symbol) {
                        println!("  📥 Importing {} from Alpaca into SQLite", ap.symbol);
                        let rec = persistence::PositionRecord {
                            symbol:      ap.symbol.clone(),
                            qty:         ap.qty.parse::<f64>().unwrap_or(0.0),
                            entry_price: ap.avg_entry_price.parse::<f64>().unwrap_or(0.0),
                            entry_date:  "reconciled".to_string(),
                            strategy:    Some("reconciled".to_string()),
                            expires_at:  None,
                        };
                        let _ = store.upsert_position(&rec).await;
                    }
                }
            }
            Err(e) => eprintln!("⚠️  Position reconciliation skipped: {}", e),
        }

        // In-memory open-position guard (rebuilt from reconciled SQLite state).
        // Prevents the bot from opening a second position in a symbol it already holds.
        let reconciled = store.get_open_positions().await.unwrap_or_default();
        let mut open_syms: std::collections::HashSet<String> =
            reconciled.iter().map(|p| p.symbol.clone()).collect();
        if !open_syms.is_empty() {
            println!("  [LOCK] {} open position(s) -- skipping duplicate entries",
                open_syms.len());
        }

        // ── Load runtime config from trading_bot_config.json ─────────────
        let bot_cfg = config::TradingBotConfigFile::load();
        info!("Bot config: min_confidence={:.2} max_daily_loss={:.1}% cooldown={}s",
            bot_cfg.min_confidence,
            bot_cfg.max_daily_loss_pct * 100.0,
            bot_cfg.signal_cooldown_secs);

        // ── Strategy registry ─────────────────────────────────────────────
        let mut registry = StrategyRegistry::new();
        registry.register(Box::new(MomentumStrategy::new()));
        registry.register(Box::new(MeanReversionStrategy::new()));
        registry.register(Box::new(BreakoutStrategy::new()));
        registry.register(Box::new(VolatilityArbitrageStrategy::new()));
        registry.register(Box::new(CashSecuredPuts::new()));

        // ── Rolling price buffers and signal-cooldown trackers ────────────
        let mut price_buf: std::collections::HashMap<String, std::collections::VecDeque<f64>>
            = std::collections::HashMap::new();
        let mut last_signal: std::collections::HashMap<String, Instant>
            = std::collections::HashMap::new();
        const SIGNAL_COOLDOWN_SECS: u64   = 300; // 5-minute cooldown per symbol
        const MIN_PRICES_FOR_HV:    usize = 22;  // need 22 ticks for HV-21
        const MAX_PRICE_BUF:        usize = 50;  // rolling window size
        const MIN_CONFIDENCE:       f64   = 0.60;
        let signal_cooldown_secs = bot_cfg.signal_cooldown_secs;
        let min_prices_for_hv    = bot_cfg.min_prices_for_hv;
        let max_price_buf        = bot_cfg.max_price_buf;
        let min_confidence       = bot_cfg.min_confidence;
        /// Halt new orders once estimated daily spend reaches this fraction of equity.
        const MAX_DAILY_LOSS_PCT:   f64   = 0.05; // 5 %
        let max_daily_loss           = equity * bot_cfg.max_daily_loss_pct;
        let mut estimated_daily_loss = 0.0_f64;
        let mut circuit_broken       = false;

        println!("📡 Connecting to Alpaca live stream for {} symbols...", symbols.len());
        let mut stream = match streaming::AlpacaStream::connect_from_env(&symbols).await {
            Ok(s) => s,
            Err(e) => { eprintln!("WebSocket connect failed: {}", e); return; }
        };
        println!("Stream connected -- press Ctrl-C to stop\n");

        // Graceful Ctrl-C shutdown
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
                        .or_insert_with(std::collections::VecDeque::new);
                    buf.push_back(price);
                    if buf.len() > max_price_buf { buf.pop_front(); }
                    if buf.len() < min_prices_for_hv { continue; }

                    // Circuit breaker — stop new signals if daily loss limit is hit
                    if circuit_broken {
                        continue;
                    }

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

                    // Generate signals
                    let signals = registry.generate_all_signals(&sym, price, sigma, model_iv, sigma);
                    let actionable: Vec<_> = signals.iter()
                        .filter(|s| {
                            s.confidence >= min_confidence
                                && !matches!(s.action, SignalAction::NoAction)
                                && !matches!(s.action, SignalAction::ClosePosition { .. })
                        })
                        .collect();

                    if actionable.is_empty() { continue; }
                    last_signal.insert(sym.clone(), now);

                    // Portfolio risk gate
                    // Rough ATM premium ≈ spot × vol × √(30/365)
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

                    info!("Signal [{}] ${:.2} vol={:.1}% -- {} actionable signal(s)",
                        sym, price, sigma * 100.0, actionable.len());
                    println!("[{}]  ${:.2}  vol={:.1}%", sym, price, sigma * 100.0);
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
                        // Deduplication guard -- skip if we already hold an open position
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
                                        // Update circuit-breaker spend tracker
                                        estimated_daily_loss += rough_premium * qty as f64 * 100.0;
                                        if estimated_daily_loss >= max_daily_loss && !circuit_broken {
                                            circuit_broken = true;
                                            error!("CIRCUIT BREAKER: daily spend ${:.2} >= limit ${:.2} -- halting new orders",
                                                estimated_daily_loss, max_daily_loss);
                                        }
                                        let pos = persistence::PositionRecord {
                                            symbol:      sym.clone(),
                                            qty:         qty as f64,
                                            entry_price: price,
                                            entry_date:  ts.clone(),
                                            strategy:    Some(sig.strategy_name.clone()),
                                            expires_at:  None,
                                        };
                                        if let Err(e) = store.upsert_position(&pos).await {
                                            error!("DB position upsert failed: {}", e);
                                            eprintln!("DB position error: {}", e);
                                        } else {
                                            open_syms.insert(sym.clone());
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

        // Graceful shutdown: close stream, cancel open orders, log session summary
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
            Ok(_)   => info!("No open orders to cancel."),
            Err(e)  => warn!("cancel_all_orders failed: {}", e),
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
    } else {
        println!("Symbols: {:?}", symbols);
        if dry_run {
            println!("[DRY RUN] Would evaluate signals and submit paper orders.");
        } else {
            println!("Run with --live to start the WebSocket stream, or --dry-run for simulation.");
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Return the most suitable CSV path for a symbol.
fn find_csv(symbol: &str) -> String {
    let lower = symbol.to_lowercase();
    let candidates = [
        format!("data/{}_five_year.csv", lower),
        format!("data/{}_one_year.csv", lower),
        // Legacy naming used for TSLA before consistent naming was adopted
        "data/tesla_one_year.csv".to_string(),
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.clone();
        }
    }
    // Default fallback that will produce a clear error message
    candidates[0].clone()
}

// ─── Demo helper functions (used by `demo` subcommand) ────────────────────

fn demo_market_analysis(history: &[HistoricalDay], n_days: usize, sigma: f64, load_time: Duration) {
    println!("Loaded {} trading days", history.len());
    println!("Historical Volatility: {:.2}%", sigma * 100.0);
    action_table_out::show_action_table(history, n_days, sigma);
    if history.len() >= n_days {
        pnl_output::show_pnl_post_mortem(history, n_days, sigma);
    }
    println!("\nCSV load time: {:.6} ms", load_time.as_secs_f64() * 1000.0);
}

/// Displays Heston params, Carr-Madan pricing, BS comparison, and vol smile.
/// Returns the ATM Carr-Madan price and the time taken to compute it.
fn demo_analytical_pricing(
    current_price: f64,
    time_to_maturity: f64,
    rate: f64,
    sigma: f64,
    heston_params: &HestonParams,
) -> (f64, Duration) {
    // ── Heston parameters ────────────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("HESTON MODEL PARAMETERS");
    println!("{}", "=".repeat(70));
    println!("Spot Price (S0):        ${:.2}", heston_params.s0);
    println!("Initial Variance (v0):  {:.4} (vol: {:.2}%)",
             heston_params.v0, heston_params.v0.sqrt() * 100.0);
    println!("Long-term Var (θ):      {:.4}", heston_params.theta);
    println!("Mean Reversion (κ):     {:.2}", heston_params.kappa);
    println!("Vol-of-Vol (σ):         {:.2}", heston_params.sigma);
    println!("Correlation (ρ):        {:.2}", heston_params.rho);
    println!("Risk-free Rate (r):     {:.2}%", heston_params.r * 100.0);
    println!("Time to Maturity (T):   {:.2} years", heston_params.t);

    // ── Carr-Madan analytical pricing ────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("CARR-MADAN ANALYTICAL PRICING (Fast & Deterministic)");
    println!("{}", "=".repeat(70));
    let atm_strike = current_price;
    let moneyness = classify_moneyness(atm_strike, current_price, 0.05);
    println!("\nPricing ATM Call Option:");
    println!("Strike: ${:.2} ({})", atm_strike,
             if moneyness == Moneyness::ATM { "ATM ✓" } else { "NOT ATM" });
    let carr_madan_start = Instant::now();
    let carr_madan_price = heston_call_carr_madan(
        current_price, atm_strike, time_to_maturity, rate, heston_params,
    );
    let carr_madan_time = carr_madan_start.elapsed();
    println!("Carr-Madan Price: ${:.2}", carr_madan_price);
    println!("Computation Time: {:.3} ms", carr_madan_time.as_secs_f64() * 1000.0);

    // ── Black-Scholes comparison ─────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("BLACK-SCHOLES COMPARISON");
    println!("{}", "=".repeat(70));
    let bs_greeks = black_scholes_call(current_price, atm_strike, time_to_maturity, rate, sigma);
    println!("\nBlack-Scholes ATM Call:");
    println!("Price:  ${:.2}", bs_greeks.price);
    println!("Delta:  {:.4}", bs_greeks.delta);
    println!("Gamma:  {:.4}", bs_greeks.gamma);
    println!("Vega:   {:.2}", bs_greeks.vega);
    println!("Theta:  {:.2}", bs_greeks.theta);
    println!("Rho:    {:.2}", bs_greeks.rho);
    println!("\nCarr-Madan vs Black-Scholes:");
    println!("Price Difference: ${:.2} ({:.1}%)",
             carr_madan_price - bs_greeks.price,
             (carr_madan_price - bs_greeks.price) / bs_greeks.price * 100.0);
    println!("Speed Advantage: Carr-Madan is analytical (no Monte Carlo noise)");

    // ── Volatility smile across strikes ──────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("VOLATILITY SMILE: Pricing Across Strikes");
    println!("{}", "=".repeat(70));
    let strikes = vec![
        (current_price * 0.80, "Deep ITM"),
        (current_price * 0.90, "ITM"),
        (current_price * 1.00, "ATM"),
        (current_price * 1.10, "OTM"),
        (current_price * 1.20, "Deep OTM"),
    ];
    println!("\nCALL OPTIONS:");
    println!("{:<12} {:<10} {:<12} {:<12} {:<10}", "Strike", "Moneyness", "Heston", "Black-Scholes", "Diff %");
    println!("{}", "-".repeat(70));
    for (strike, label) in &strikes {
        let heston_price = if strike < &current_price {
            heston_call_itm(current_price, *strike, time_to_maturity, rate, heston_params)
        } else if strike > &current_price {
            heston_call_otm(current_price, *strike, time_to_maturity, rate, heston_params)
        } else {
            carr_madan_price
        };
        let bs_price = black_scholes_call(current_price, *strike, time_to_maturity, rate, sigma).price;
        let diff_pct = (heston_price - bs_price) / bs_price * 100.0;
        println!("{:<12.2} {:<10} ${:<11.2} ${:<11.2} {:>9.1}%",
                 strike, label, heston_price, bs_price, diff_pct);
    }
    println!("\nPUT OPTIONS:");
    println!("{:<12} {:<10} {:<12} {:<12} {:<10}", "Strike", "Moneyness", "Heston", "Black-Scholes", "Diff %");
    println!("{}", "-".repeat(70));
    for (strike, label) in &strikes {
        let heston_price = if strike > &current_price {
            heston_put_otm(current_price, *strike, time_to_maturity, rate, heston_params)
        } else if strike < &current_price {
            heston_put_itm(current_price, *strike, time_to_maturity, rate, heston_params)
        } else {
            heston_put_carr_madan(current_price, *strike, time_to_maturity, rate, heston_params)
        };
        let bs_call = black_scholes_call(current_price, *strike, time_to_maturity, rate, sigma).price;
        let bs_put = bs_call - current_price + strike * (-rate * time_to_maturity).exp();
        let diff_pct = (heston_price - bs_put) / bs_put * 100.0;
        println!("{:<12.2} {:<10} ${:<11.2} ${:<11.2} {:>9.1}%",
                 strike, label, heston_price, bs_put, diff_pct);
    }
    println!("\n💡 Heston captures volatility smile/skew - prices differ from constant-vol BS");

    (carr_madan_price, carr_madan_time)
}

/// Runs 100K-path Monte Carlo validation. Returns the Carr-Madan speedup factor.
fn demo_monte_carlo(heston_params: HestonParams, atm_strike: f64, carr_madan_time: Duration) -> f64 {
    println!("\n{}", "=".repeat(70));
    println!("MONTE CARLO VALIDATION (100K paths)");
    println!("{}", "=".repeat(70));
    let config = MonteCarloConfig { n_paths: 100_000, n_steps: 500, seed: 42, use_antithetic: true };
    let mc = match HestonMonteCarlo::new(heston_params, config) {
        Ok(mc) => mc,
        Err(e) => { eprintln!("Invalid Heston parameters: {}", e); return 1.0; }
    };
    let mc_start = Instant::now();
    let mc_greeks = mc.greeks_european_call(atm_strike);
    let mc_time = mc_start.elapsed();
    println!("\nMonte Carlo Results:");
    println!("Price:  ${:.2}", mc_greeks.price);
    println!("Delta:  {:.4}", mc_greeks.delta);
    println!("Gamma:  {:.4}", mc_greeks.gamma);
    println!("Vega:   {:.2}", mc_greeks.vega);
    println!("Theta:  {:.2}", mc_greeks.theta);
    println!("Rho:    {:.2}", mc_greeks.rho);
    println!("Time:   {:.1} seconds", mc_time.as_secs_f64());
    let speedup = mc_time.as_secs_f64() / carr_madan_time.as_secs_f64();
    println!("\nSpeed Comparison:");
    println!("Carr-Madan: {:.3} ms", carr_madan_time.as_secs_f64() * 1000.0);
    println!("Monte Carlo: {:.1} seconds", mc_time.as_secs_f64());
    println!("Speedup: {:.0}x faster ⚡", speedup);
    speedup
}

fn demo_strategy_signals(symbol: &str, current_price: f64, sigma: f64, heston_params: &HestonParams) {
    println!("\n{}", "=".repeat(70));
    println!("TRADING STRATEGY SIGNALS");
    println!("{}", "=".repeat(70));
    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(VolMeanReversion::new()));
    println!("\nActive Strategies:");
    for (i, name) in registry.list_strategies().iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }
    let signals = registry.generate_all_signals(
        symbol, current_price, sigma, heston_params.v0.sqrt(), sigma,
    );
    if !signals.is_empty() {
        println!("\n📊 Signal Summary:");
        for signal in &signals {
            println!("\n[{} - {}]", signal.symbol, signal.strategy_name);
            println!("   Action: {:?}", signal.action);
            println!("   Strike: ${:.2}", signal.strike);
            println!("   Expiry: {} days", signal.expiry_days);
            println!("   Confidence: {:.0}%", signal.confidence * 100.0);
            println!("   Est. Edge: ${:.2}", signal.edge);
        }
    }
}

fn demo_calibration(current_price: f64, rate: f64) {
    println!("\n{}", "=".repeat(70));
    println!("CALIBRATION DEMONSTRATION");
    println!("{}", "=".repeat(70));
    let true_params = CalibParams { kappa: 3.5, theta: 0.25, sigma: 0.45, rho: -0.75, v0: 0.30 };
    println!("\n🎯 True Parameters (hidden from optimizer):");
    println!("  κ = {:.2}, θ = {:.4}, σ = {:.2}, ρ = {:.2}, v₀ = {:.4}",
             true_params.kappa, true_params.theta, true_params.sigma,
             true_params.rho, true_params.v0);
    let strikes: Vec<f64> = [0.85, 0.90, 0.95, 1.00, 1.05, 1.10, 1.15]
        .iter().map(|k| current_price * k).collect();
    let maturities = vec![30.0 / 365.0, 60.0 / 365.0];
    let market_data = create_mock_market_data(current_price, rate, &true_params, &strikes, &maturities);
    println!("📊 Generated {} synthetic market options", market_data.len());
    let initial_guess = CalibParams { kappa: 2.0, theta: 0.35, sigma: 0.30, rho: -0.60, v0: 0.35 };
    println!("\n🔧 Initial Guess:");
    println!("  κ = {:.2}, θ = {:.4}, σ = {:.2}, ρ = {:.2}, v₀ = {:.4}",
             initial_guess.kappa, initial_guess.theta, initial_guess.sigma,
             initial_guess.rho, initial_guess.v0);
    let calib_start = Instant::now();
    match calibrate_heston(current_price, rate, market_data, initial_guess) {
        Ok(result) => {
            let calib_time = calib_start.elapsed();
            result.print_summary();
            println!("\n⏱️  Calibration Time: {:.2} seconds", calib_time.as_secs_f64());
            println!("\n📈 Recovery Accuracy:");
            println!("  κ error: {:.2}%", (result.params.kappa - true_params.kappa).abs() / true_params.kappa * 100.0);
            println!("  θ error: {:.2}%", (result.params.theta - true_params.theta).abs() / true_params.theta * 100.0);
            println!("  σ error: {:.2}%", (result.params.sigma - true_params.sigma).abs() / true_params.sigma * 100.0);
            println!("  ρ error: {:.2}%", (result.params.rho - true_params.rho).abs() / true_params.rho.abs() * 100.0);
            println!("  v₀ error: {:.2}%", (result.params.v0 - true_params.v0).abs() / true_params.v0 * 100.0);
        }
        Err(e) => println!("❌ Calibration failed: {}", e),
    }
}

fn demo_summary(speedup: f64) {
    println!("\n{}", "=".repeat(70));
    println!("SYSTEM SUMMARY");
    println!("{}", "=".repeat(70));
    println!("✓ Modular architecture implemented");
    println!("✓ Carr-Madan analytical pricing (ATM/OTM/ITM)");
    println!("✓ Volatility smile captured by Heston characteristic function");
    println!("✓ Monte Carlo validation ({:.0}x slower)", speedup);
    println!("✓ Strategy framework operational");
    println!("✓ Parameter calibration working (argmin optimizer)");
    println!("\nRun `dollarbill --help` to explore all CLI subcommands.");
    println!("{}", "=".repeat(70));
}
