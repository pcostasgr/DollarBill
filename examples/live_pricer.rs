// Live options pricer — wires Yahoo options feed → Heston calibration (TTL-cached) → edge signal loop
//
// Usage:
//   cargo run --example live_pricer                              # poll every 60s, recalibrate every 15min
//   cargo run --example live_pricer -- --once                    # single pass then exit
//   cargo run --example live_pricer -- --interval 30             # poll every 30s
//   cargo run --example live_pricer -- --calibrate-ttl 300       # recalibrate every 5min
//   cargo run --example live_pricer -- --expiry 1 --once         # second-nearest expiry, single pass
//   cargo run --example live_pricer -- --min-edge-pct 3.0        # 3% edge threshold (default 5%)

use dollarbill::market_data::real_option_data_yahoo::fetch_liquid_options;
use dollarbill::market_data::real_market_data::fetch_latest_price;
use dollarbill::calibration::heston_calibrator::{calibrate_heston, CalibParams};
use dollarbill::calibration::market_option::OptionType;
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use dollarbill::market_data::symbols::load_enabled_stocks;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

// ── Config (mirrors signals_config.json schema) ──────────────────────────────

#[derive(Debug, Deserialize)]
struct SignalsConfig {
    analysis: AnalysisConfig,
    #[allow(dead_code)]
    calibration: CalibrationConfig,
    #[allow(dead_code)]
    options: OptionsConfig,
}

#[derive(Debug, Deserialize)]
struct AnalysisConfig {
    risk_free_rate: f64,
    liquidity_filters: LiquidityFilters,
    edge_thresholds: EdgeThresholds,
}

#[derive(Debug, Deserialize)]
struct LiquidityFilters {
    min_volume: i32,
    max_spread_pct: f64,
}

#[derive(Debug, Deserialize)]
struct EdgeThresholds {
    min_edge_dollars: f64,
    min_delta: f64,
}

#[derive(Debug, Deserialize)]
struct CalibrationConfig {
    #[allow(dead_code)]
    tolerance: f64,
    #[allow(dead_code)]
    max_iterations: usize,
}

#[derive(Debug, Deserialize)]
struct OptionsConfig {
    #[allow(dead_code)]
    default_time_to_expiry_days: usize,
    #[allow(dead_code)]
    min_time_to_expiry_days: usize,
    #[allow(dead_code)]
    max_time_to_expiry_days: usize,
}

// ── Calibration cache ─────────────────────────────────────────────────────────

struct CacheEntry {
    params: CalibParams,
    rmse: f64,
    cached_at: Instant,
}

// ── Edge signal ───────────────────────────────────────────────────────────────

struct EdgeSignal {
    symbol: String,
    option_type: &'static str,
    strike: f64,
    moneyness: f64,
    market_bid: f64,
    market_ask: f64,
    model_price: f64,
    edge_pct: f64,
    edge_dollars: f64,
    action: &'static str,
    delta: f64,
    vega: f64,
}

// ── Per-symbol async processing ───────────────────────────────────────────────

async fn process_symbol(
    symbol: &str,
    rate: f64,
    min_volume: i32,
    max_spread_pct: f64,
    expiry_index: usize,
    min_edge_pct: f64,
    min_edge_dollars: f64,
    min_delta: f64,
    cache: &mut HashMap<String, CacheEntry>,
    calibrate_ttl: Duration,
) -> Result<(f64, Vec<EdgeSignal>, bool), Box<dyn std::error::Error>> {
    // Fetch live spot price and options chain concurrently
    let (spot_res, options_res) = tokio::join!(
        fetch_latest_price(symbol),
        fetch_liquid_options(symbol, expiry_index, min_volume, max_spread_pct),
    );
    let spot = spot_res?;
    let liquid_options = options_res?;

    if liquid_options.is_empty() {
        return Ok((spot, vec![], false));
    }

    // Recalibrate Heston when cache is missing or TTL has expired
    let recalibrated = match cache.get(symbol) {
        Some(entry) if entry.cached_at.elapsed() < calibrate_ttl => false,
        _ => {
            let initial = CalibParams {
                kappa: 2.0,
                theta: 0.25,
                sigma: 0.30,
                rho: -0.60,
                v0: 0.30,
            };
            let result = calibrate_heston(spot, rate, liquid_options.clone(), initial)?;
            cache.insert(
                symbol.to_string(),
                CacheEntry {
                    params: result.params,
                    rmse: result.rmse,
                    cached_at: Instant::now(),
                },
            );
            true
        }
    };

    let entry = cache.get(symbol).expect("inserted or existed above");
    let time_to_expiry = liquid_options[0].time_to_expiry;
    let heston_params = entry.params.to_heston(spot, rate, time_to_expiry);
    let iv = entry.params.v0.sqrt();
    let q = 0.0; // no continuous dividend yield

    // Price each liquid option and collect edge signals
    let mut signals = Vec::new();
    for opt in &liquid_options {
        let market_mid = opt.mid_price();

        let model_price = match opt.option_type {
            OptionType::Call => {
                heston_call_carr_madan(spot, opt.strike, opt.time_to_expiry, rate, &heston_params)
            }
            OptionType::Put => {
                heston_put_carr_madan(spot, opt.strike, opt.time_to_expiry, rate, &heston_params)
            }
        };

        let edge_dollars = model_price - market_mid;
        let edge_pct = (edge_dollars / market_mid) * 100.0;

        // Apply both dollar and percent edge filters
        if edge_pct.abs() < min_edge_pct || edge_dollars.abs() < min_edge_dollars {
            continue;
        }

        let greeks = match opt.option_type {
            OptionType::Call => {
                black_scholes_merton_call(spot, opt.strike, opt.time_to_expiry, rate, iv, q)
            }
            OptionType::Put => {
                black_scholes_merton_put(spot, opt.strike, opt.time_to_expiry, rate, iv, q)
            }
        };

        if greeks.delta.abs() < min_delta {
            continue;
        }

        let action: &'static str = if edge_pct > 0.0 { "BUY" } else { "SELL" };
        let option_type_str: &'static str = match opt.option_type {
            OptionType::Call => "Call",
            OptionType::Put => "Put",
        };

        signals.push(EdgeSignal {
            symbol: symbol.to_string(),
            option_type: option_type_str,
            strike: opt.strike,
            moneyness: opt.strike / spot,
            market_bid: opt.bid,
            market_ask: opt.ask,
            model_price,
            edge_pct,
            edge_dollars,
            action,
            delta: greeks.delta,
            vega: greeks.vega,
        });
    }

    // Sort by absolute edge descending so best opportunities appear first
    signals.sort_by(|a, b| {
        b.edge_pct
            .abs()
            .partial_cmp(&a.edge_pct.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok((spot, signals, recalibrated))
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ANSI color codes (work in Windows Terminal / PowerShell 7)
    const GREEN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const BLUE: &str = "\x1b[34m";
    const YELLOW: &str = "\x1b[33m";
    const CYAN: &str = "\x1b[36m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";
    const DIM: &str = "\x1b[2m";
    let _ = BLUE; // suppress unused-variable if --once path never hits blue

    // ── Argument parsing ─────────────────────────────────────────────────────
    let args: Vec<String> = std::env::args().collect();
    let mut interval_secs: u64 = 60;
    let mut calibrate_ttl_secs: u64 = 900; // 15 min default
    let mut expiry_index: usize = 1; // 0 = nearest (often very short-dated), 1 = next
    let mut min_edge_pct_override: Option<f64> = None;
    let mut once = false;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--once" => once = true,
            "--interval" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    interval_secs = v.parse().unwrap_or(60);
                }
            }
            "--calibrate-ttl" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    calibrate_ttl_secs = v.parse().unwrap_or(900);
                }
            }
            "--expiry" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    expiry_index = v.parse().unwrap_or(1);
                }
            }
            "--min-edge-pct" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    min_edge_pct_override = v.parse().ok();
                }
            }
            _ => {}
        }
        i += 1;
    }

    let calibrate_ttl = Duration::from_secs(calibrate_ttl_secs);
    let poll_interval = Duration::from_secs(interval_secs);

    // ── Load config ──────────────────────────────────────────────────────────
    let config_content = fs::read_to_string("config/signals_config.json")
        .map_err(|e| format!("Failed to read config/signals_config.json: {}", e))?;
    let config: SignalsConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse signals config: {}", e))?;

    let symbols = load_enabled_stocks()
        .map_err(|e| format!("Failed to load config/stocks.json: {}", e))?;

    if symbols.is_empty() {
        eprintln!("No enabled symbols in config/stocks.json");
        return Ok(());
    }

    let rate = config.analysis.risk_free_rate;
    let min_volume = config.analysis.liquidity_filters.min_volume;
    let max_spread_pct = config.analysis.liquidity_filters.max_spread_pct;
    let min_edge_dollars = config.analysis.edge_thresholds.min_edge_dollars;
    let min_edge_pct = min_edge_pct_override.unwrap_or(5.0);
    let min_delta = config.analysis.edge_thresholds.min_delta;

    // ── Header ───────────────────────────────────────────────────────────────
    println!(
        "{}{}╔═══════════════════════════════════════════════════╗{}",
        BOLD, CYAN, RESET
    );
    println!(
        "{}{}║       D O L L A R B I L L   L I V E  P R I C E R  ║{}",
        BOLD, CYAN, RESET
    );
    println!(
        "{}{}║  Heston-calibrated edge signals on live data       ║{}",
        BOLD, CYAN, RESET
    );
    println!(
        "{}{}╚═══════════════════════════════════════════════════╝{}",
        BOLD, CYAN, RESET
    );
    println!();
    println!("  Symbols       : {}", symbols.join(", "));
    println!("  Risk-free rate: {:.2}%", rate * 100.0);
    println!("  Poll interval : {}s", interval_secs);
    println!(
        "  Calibrate TTL : {}s  ({:.0} min)",
        calibrate_ttl_secs,
        calibrate_ttl_secs as f64 / 60.0
    );
    println!("  Expiry slot   : {} (0=nearest)", expiry_index);
    println!("  Min edge      : {:.1}% and ${:.2}", min_edge_pct, min_edge_dollars);
    println!("  Min |delta|   : {:.2}", min_delta);
    println!(
        "  Mode          : {}",
        if once {
            "one-shot"
        } else {
            "loop (Ctrl+C to quit)"
        }
    );
    println!();

    // ── Calibration cache (persists across poll cycles) ───────────────────────
    let mut cache: HashMap<String, CacheEntry> = HashMap::new();

    // ── Poll loop ────────────────────────────────────────────────────────────
    loop {
        let cycle_start = Instant::now();
        let now = chrono::Local::now();

        println!(
            "{}━━━  {}  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
            CYAN,
            now.format("%Y-%m-%d  %H:%M:%S"),
            RESET
        );

        let mut total_signals = 0usize;
        let mut symbols_with_errors = 0usize;

        for symbol in &symbols {
            print!("  {}{:<6}{} ", BOLD, symbol, RESET);
            let t0 = Instant::now();

            match process_symbol(
                symbol,
                rate,
                min_volume,
                max_spread_pct,
                expiry_index,
                min_edge_pct,
                min_edge_dollars,
                min_delta,
                &mut cache,
                calibrate_ttl,
            )
            .await
            {
                Err(e) => {
                    println!("{}ERROR{} — {}", RED, RESET, e);
                    symbols_with_errors += 1;
                }
                Ok((spot, signals, recalibrated)) => {
                    let elapsed_ms = t0.elapsed().as_millis();
                    let rmse = cache.get(symbol.as_str()).map(|e| e.rmse).unwrap_or(0.0);
                    let calib_tag = if recalibrated {
                        format!(" {}[calibrated rmse={:.4}]{}", YELLOW, rmse, RESET)
                    } else {
                        format!(" {}[cached]{}", DIM, RESET)
                    };
                    println!(
                        "spot=${:.2}  {} signal{}  {:.0}ms{}",
                        spot,
                        signals.len(),
                        if signals.len() == 1 { "" } else { "s" },
                        elapsed_ms,
                        calib_tag
                    );

                    total_signals += signals.len();

                    if !signals.is_empty() {
                        println!(
                            "  {}  {:<10}  {:>8}  {:>7}  {:>7}  {:>8}  {:>8}  {:>7}  {:>6}  {:>6}  ACTION{}",
                            DIM, "TYPE", "STRIKE", "BID", "ASK", "MODEL", "EDGE $", "EDGE%", "DELTA", "VEGA", RESET
                        );
                        for s in &signals {
                            let color = if s.action == "BUY" { GREEN } else { RED };
                            let atm = if (s.moneyness - 1.0).abs() < 0.03 {
                                format!(" {}[ATM]{}", BOLD, RESET)
                            } else {
                                String::new()
                            };
                            println!(
                                "  {}{:<10}  {:>8.2}  {:>7.4}  {:>7.4}  {:>8.4}  {:>+8.4}  {:>+7.2}%  {:>6.3}  {:>6.4}  {}{}{}",
                                color,
                                format!("{} {}", s.symbol, s.option_type),
                                s.strike,
                                s.market_bid,
                                s.market_ask,
                                s.model_price,
                                s.edge_dollars,
                                s.edge_pct,
                                s.delta,
                                s.vega,
                                s.action,
                                RESET,
                                atm
                            );
                        }
                    }
                }
            }
        }

        // Cycle summary
        let cycle_secs = cycle_start.elapsed().as_secs_f64();
        println!();
        if total_signals > 0 {
            println!(
                "  {}Summary:{} {} edge signal{} found across {} symbol{}  ({:.1}s)",
                BOLD,
                RESET,
                total_signals,
                if total_signals == 1 { "" } else { "s" },
                symbols.len() - symbols_with_errors,
                if symbols.len() - symbols_with_errors == 1 { "" } else { "s" },
                cycle_secs
            );
        } else {
            println!(
                "  {}Summary:{} No edge signals — market is fairly priced  ({:.1}s)",
                BOLD, RESET, cycle_secs
            );
        }
        if symbols_with_errors > 0 {
            println!(
                "  {}Warning:{} {} symbol{} failed data fetch",
                YELLOW,
                RESET,
                symbols_with_errors,
                if symbols_with_errors == 1 { "" } else { "s" }
            );
        }

        if once {
            break;
        }

        // Sleep until the next poll, with graceful Ctrl+C exit
        let elapsed = cycle_start.elapsed();
        if elapsed < poll_interval {
            let sleep_dur = poll_interval - elapsed;
            println!(
                "\n  {}Next cycle in {:.0}s  (Ctrl+C to stop){}",
                DIM,
                sleep_dur.as_secs_f64(),
                RESET
            );
            tokio::select! {
                _ = tokio::time::sleep(sleep_dur) => {}
                _ = tokio::signal::ctrl_c() => {
                    println!("\n{}Stopped.{}", CYAN, RESET);
                    break;
                }
            }
        }
    }

    Ok(())
}
