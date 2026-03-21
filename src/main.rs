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
use market_data::csv_loader::load_csv_closes;
use models::bs_mod::{compute_historical_vol, black_scholes_call};
use models::heston::{heston_start, HestonParams};
use models::heston_analytical::{heston_call_carr_madan, classify_moneyness, Moneyness};
use strategies::{StrategyRegistry, vol_mean_reversion::VolMeanReversion};
use std::time::Instant;

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
    use utils::demo;

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

    demo::demo_market_analysis(&history, n_days, sigma, load_time);
    let (_, carr_madan_time) =
        demo::demo_analytical_pricing(current_price, time_to_maturity, rate, sigma, &heston_params);
    let speedup = demo::demo_monte_carlo(heston_params.clone(), current_price, carr_madan_time);
    demo::demo_strategy_signals(symbol, current_price, sigma, &heston_params);
    demo::demo_calibration(current_price, rate);
    demo::demo_summary(speedup);
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
    use utils::demo;
    let csv_path = find_csv(symbol);
    let history = match load_csv_closes(&csv_path) {
        Ok(h) => h,
        Err(e) => { eprintln!("Failed to load {}: {}", csv_path, e); return; }
    };
    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let spot = *closes.last().unwrap();
    let rate = 0.05;
    demo::demo_calibration(spot, rate);
}

// ─── Subcommand: trade ─────────────────────────────────────────────────────

async fn cmd_trade(dry_run: bool, live: bool) {
    use alpaca::{AlpacaClient, live_bot};
    use market_data::symbols::load_enabled_stocks;

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

        let store = match persistence::TradeStore::new("data/trades.db").await {
            Ok(s) => s,
            Err(e) => { eprintln!("Failed to open trade database: {}", e); return; }
        };
        let persisted = store.get_open_positions().await.unwrap_or_default();

        live_bot::run_live_bot(client, symbols, equity, buy_pwr, dry_run, store, persisted).await;
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
