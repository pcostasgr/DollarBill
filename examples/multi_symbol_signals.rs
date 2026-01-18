 // Multi-symbol trade signal generator with parallel calibration
use dollarbill::market_data::options_json_loader::{load_options_from_json, filter_liquid_options};
use dollarbill::calibration::heston_calibrator::{calibrate_heston, CalibParams};
use dollarbill::calibration::market_option::OptionType;
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use dollarbill::market_data::symbols::load_enabled_stocks;
use rayon::prelude::*;
use std::time::Instant;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct SignalsConfig {
    analysis: AnalysisConfig,
    calibration: CalibrationConfig,
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
    tolerance: f64,
    max_iterations: usize,
}

#[derive(Debug, Deserialize)]
struct OptionsConfig {
    default_time_to_expiry_days: usize,
    min_time_to_expiry_days: usize,
    max_time_to_expiry_days: usize,
}

#[derive(Debug, Clone)]
struct TradeSignal {
    symbol: String,
    option_type: String,
    strike: f64,
    moneyness: f64,  // strike / spot
    market_bid: f64,
    market_ask: f64,
    model_price: f64,
    edge_pct: f64,
    edge_dollars: f64,
    action: String,
    // Greeks
    delta: f64,
    gamma: f64,
    vega: f64,
    theta: f64,
    implied_vol: f64,
}

impl TradeSignal {
    fn is_atm(&self) -> bool {
        // Consider ATM if within ±3% of spot
        (self.moneyness - 1.0).abs() < 0.03
    }
    
    fn atm_indicator(&self) -> &str {
        if self.is_atm() { "ATM" } else { "" }
    }
}

#[derive(Debug)]
struct SymbolResult {
    symbol: String,
    spot: f64,
    signals: Vec<TradeSignal>,
    calibration_time_ms: u128,
    rmse: f64,
    iterations: u64,
}

const SIGNAL_THRESHOLD: f64 = 5.0;  // 5% edge required for signal

fn process_symbol(symbol: &str, config: &SignalsConfig) -> Result<SymbolResult, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();
    
    // Load options for this symbol
    let json_file = format!("data/{}_options_live.json", symbol.to_lowercase());
    let (spot, all_options) = load_options_from_json(&json_file)
        .map_err(|e| format!("Failed to load {}: {}", json_file, e))?;
    
    let liquid_options = filter_liquid_options(all_options, config.analysis.liquidity_filters.min_volume, config.analysis.liquidity_filters.max_spread_pct);
    
    if liquid_options.is_empty() {
        return Ok(SymbolResult {
            symbol: symbol.to_string(),
            spot,
            signals: vec![],
            calibration_time_ms: start.elapsed().as_millis(),
            rmse: 0.0,
            iterations: 0,
        });
    }
    
    // Calibrate Heston model
    let initial_guess = CalibParams {
        kappa: 2.0,
        theta: 0.25,
        sigma: 0.30,
        rho: -0.60,
        v0: 0.30,
    };
    
    let rate = config.analysis.risk_free_rate;
    let result = calibrate_heston(spot, rate, liquid_options.clone(), initial_guess)
        .map_err(|e| format!("Calibration failed for {}: {}", symbol, e))?;
    
    // Generate trade signals
    let time_to_expiry = liquid_options[0].time_to_expiry;
    let heston_params = result.params.to_heston(spot, rate, time_to_expiry);
    
    let mut signals = Vec::new();
    
    for option in &liquid_options {
        let market_mid = option.mid_price();
        
        let model_price = match option.option_type {
            OptionType::Call => heston_call_carr_madan(spot, option.strike, time_to_expiry, rate, &heston_params),
            OptionType::Put => heston_put_carr_madan(spot, option.strike, time_to_expiry, rate, &heston_params),
        };
        
        let edge_pct = ((model_price - market_mid) / market_mid) * 100.0;
        let edge_dollars = model_price - market_mid;
        
        // Calculate Greeks using implied vol from Heston model
        let implied_vol = heston_params.v0.sqrt();
        let q = 0.0; // dividend yield
        
        let greeks = match option.option_type {
            OptionType::Call => black_scholes_merton_call(spot, option.strike, time_to_expiry, rate, implied_vol, q),
            OptionType::Put => black_scholes_merton_put(spot, option.strike, time_to_expiry, rate, implied_vol, q),
        };
        
        let action = if edge_pct > SIGNAL_THRESHOLD {
            "BUY".to_string()
        } else if edge_pct < -SIGNAL_THRESHOLD {
            "SELL".to_string()
        } else {
            "HOLD".to_string()
        };
        
        // Only include options with meaningful delta (not deep OTM)
        // This filters out illiquid far OTM options with near-zero Greeks
        let min_delta = 0.05;  // At least 5% delta
        let has_meaningful_delta = greeks.delta.abs() >= min_delta;
        
        if action != "HOLD" && has_meaningful_delta {
            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                option_type: match option.option_type {
                    OptionType::Call => "Call".to_string(),
                    OptionType::Put => "Put".to_string(),
                },
                strike: option.strike,
                moneyness: option.strike / spot,
                market_bid: option.bid,
                market_ask: option.ask,
                model_price,
                edge_pct,
                edge_dollars,
                action,
                delta: greeks.delta,
                gamma: greeks.gamma,
                vega: greeks.vega,
                theta: greeks.theta,
                implied_vol,
            });
        }
    }
    
    Ok(SymbolResult {
        symbol: symbol.to_string(),
        spot,
        signals,
        calibration_time_ms: start.elapsed().as_millis(),
        rmse: result.final_error,
        iterations: result.iterations,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ANSI color codes for Windows PowerShell
    const GREEN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const BLUE: &str = "\x1b[34m";
    const YELLOW: &str = "\x1b[33m";
    const CYAN: &str = "\x1b[36m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";
    
    println!("===============================================================");
    println!("{}{}MULTI-SYMBOL TRADE SIGNAL GENERATOR{}", BOLD, CYAN, RESET);
    println!("Parallel Heston Calibration & Options Mispricing Detection");
    println!("===============================================================\n");

    // Load signals configuration
    let config_content = fs::read_to_string("config/signals_config.json")
        .map_err(|e| format!("Failed to read signals config file: {}", e))?;
    let config: SignalsConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse signals config file: {}", e))?;

    println!("📋 Loaded signals configuration from config/signals_config.json");

    // Load enabled symbols from stocks.json
    let symbols = load_enabled_stocks().expect("Failed to load stocks from config/stocks.json");
    
    if symbols.is_empty() {
        eprintln!("No enabled stocks found in config/stocks.json");
        return Ok(());
    }
    
    println!("📊 Processing {} symbols in parallel...\n", symbols.len());
    let total_start = Instant::now();
    
    // Convert to &str for par_iter
    let symbols_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
    
    // Parallel calibration and signal generation
    let results: Vec<_> = symbols_refs
        .par_iter()
        .map(|symbol| {
            print!("  ⏳ {}...", symbol);
            match process_symbol(symbol, &config) {
                Ok(result) => {
                    println!(" ✓ ({} ms, {} signals)", result.calibration_time_ms, result.signals.len());
                    Ok(result)
                }
                Err(e) => {
                    println!(" ✗ {}", e);
                    Err(e)
                }
            }
        })
        .collect();
    
    let total_time = total_start.elapsed();
    
    // Separate successes from failures
    let successful: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();
    
    if successful.is_empty() {
        println!("\n❌ All symbols failed to process");
        return Ok(());
    }
    
    println!("\n⚡ Total processing time: {} ms (parallel)", total_time.as_millis());
    println!("   Average per symbol: {} ms", total_time.as_millis() / symbols.len() as u128);
    
    // Display calibration summary
    println!("\n===============================================================");
    println!("{}{}📈 CALIBRATION SUMMARY{}", BOLD, CYAN, RESET);
    println!("===============================================================");
    println!("{:<8} {:<12} {:<10} {:<12} {:<10}", "Symbol", "Spot Price", "RMSE", "Iterations", "Signals");
    println!("{:-<60}", "");
    
    for result in &successful {
        println!("{:<8} ${:<11.2} ${:<9.2} {:<12} {:<10}",
            result.symbol,
            result.spot,
            result.rmse,
            result.iterations,
            result.signals.len()
        );
    }
    
    // Combine all signals
    let mut all_signals: Vec<TradeSignal> = successful
        .iter()
        .flat_map(|r| r.signals.clone())
        .collect();
    
    if all_signals.is_empty() {
        println!("\n⚪ No signals - all markets are efficiently priced (no edge > {}%)", SIGNAL_THRESHOLD);
        return Ok(());
    }
    
    // Sort by absolute edge (best opportunities first)
    all_signals.sort_by(|a, b| b.edge_pct.abs().partial_cmp(&a.edge_pct.abs()).unwrap());
    
    let buy_signals: Vec<_> = all_signals.iter().filter(|s| s.action == "BUY").collect();
    let sell_signals: Vec<_> = all_signals.iter().filter(|s| s.action == "SELL").collect();
    
    // Display LONG (BUY) signals
    println!("\n===============================================================");
    println!("{}{}🟢 LONG (BUY) SIGNALS ({} opportunities across all symbols){}", BOLD, GREEN, buy_signals.len(), RESET);
    println!("{}Market is underpricing these - Model says they're worth MORE{}", GREEN, RESET);
    println!("===============================================================");
    
    if !buy_signals.is_empty() {
        println!("{:<6} {:<6} {:<8} {:<9} {:<5} {:<10} {:<10} {:<12} {:<8} {:<8} {:<8} {:<8} {:<8}",
            "Symbol", "Type", "Strike", "Money%", "ATM", "Bid", "Ask", "Model Val", "Edge %", "Delta", "Gamma", "Vega", "Theta");
        println!("{:-<120}", "");
        
        for signal in buy_signals.iter().take(15) {
            println!("{:<6} {:<6} ${:<7.2} {:>7.1}% {:<5} ${:<9.2} ${:<9.2} ${:<11.2} {:>7.1}% {:>7.3} {:>7.4} {:>7.2} {:>7.2}",
                signal.symbol,
                signal.option_type,
                signal.strike,
                (signal.moneyness - 1.0) * 100.0,  // % from ATM
                signal.atm_indicator(),
                signal.market_bid,
                signal.market_ask,
                signal.model_price,
                signal.edge_pct,
                signal.delta,
                signal.gamma,
                signal.vega,
                signal.theta
            );
        }
        
        if buy_signals.len() > 15 {
            println!("... and {} more opportunities", buy_signals.len() - 15);
        }
    } else {
        println!("None found");
    }
    
    // Display SHORT (SELL) signals
    println!("\n===============================================================");
    println!("{}{}🔴 SHORT (SELL) SIGNALS ({} opportunities across all symbols){}", BOLD, RED, sell_signals.len(), RESET);
    println!("{}Market is overpricing these - Model says they're worth LESS{}", RED, RESET);
    println!("===============================================================");
    
    if !sell_signals.is_empty() {
        println!("{:<6} {:<6} {:<8} {:<9} {:<5} {:<10} {:<10} {:<12} {:<8} {:<8} {:<8} {:<8} {:<8}",
            "Symbol", "Type", "Strike", "Money%", "ATM", "Bid", "Ask", "Model Val", "Edge %", "Delta", "Gamma", "Vega", "Theta");
        println!("{:-<120}", "");
        
        for signal in sell_signals.iter().take(15) {
            println!("{:<6} {:<6} ${:<7.2} {:>7.1}% {:<5} ${:<9.2} ${:<9.2} ${:<11.2} {:>7.1}% {:>7.3} {:>7.4} {:>7.2} {:>7.2}",
                signal.symbol,
                signal.option_type,
                signal.strike,
                (signal.moneyness - 1.0) * 100.0,  // % from ATM
                signal.atm_indicator(),
                signal.market_bid,
                signal.market_ask,
                signal.model_price,
                signal.edge_pct,
                signal.delta,
                signal.gamma,
                signal.vega,
                signal.theta
            );
        }
        
        if sell_signals.len() > 15 {
            println!("... and {} more opportunities", sell_signals.len() - 15);
        }
    } else {
        println!("None found");
    }
    
    // Top opportunities by symbol
    println!("\n===============================================================");
    println!("{}{}🎯 BEST OPPORTUNITY PER SYMBOL{}", BOLD, YELLOW, RESET);
    println!("===============================================================");
    
    for result in &successful {
        if !result.signals.is_empty() {
            let best = result.signals.iter().max_by(|a, b| 
                a.edge_pct.abs().partial_cmp(&b.edge_pct.abs()).unwrap()
            ).unwrap();
            
            println!("\n{} (spot: ${:.2})", result.symbol, result.spot);
            let color = if best.action == "BUY" { GREEN } else { RED };
            println!("  {}{} {} ${:.2} {} → {} at ${:.2} (model: ${:.2}){}",
                color,
                if best.action == "BUY" { "🟢 LONG (Buy)" } else { "🔴 SHORT (Sell)" },
                best.option_type,
                best.strike,
                best.atm_indicator(),
                if best.action == "BUY" { "ask" } else { "bid" },
                if best.action == "BUY" { best.market_ask } else { best.market_bid },
                best.model_price,
                RESET
            );
            println!("  Edge: {:.1}% (${:.2}), Delta: {:.3}", best.edge_pct.abs(), best.edge_dollars.abs(), best.delta);
        }
    }
    
    // ATM Options Summary
    let atm_signals: Vec<_> = all_signals.iter().filter(|s| s.is_atm()).collect();
    
    if !atm_signals.is_empty() {
        println!("\n===============================================================");
        println!("{}{}🎯 AT-THE-MONEY OPTIONS (within ±3% of spot){}", BOLD, CYAN, RESET);
        println!("===============================================================");
        println!("Total ATM opportunities: {}", atm_signals.len());
        
        let atm_buys: Vec<_> = atm_signals.iter().filter(|s| s.action == "BUY").collect();
        let atm_sells: Vec<_> = atm_signals.iter().filter(|s| s.action == "SELL").collect();
        
        if !atm_buys.is_empty() {
            println!("\n{}🟢 ATM LONG (Buy) Signals ({}):{}", GREEN, atm_buys.len(), RESET);
            for signal in atm_buys.iter().take(5) {
                println!("  {} {} ${:.2} ({:+.1}% from spot) | Edge: {:.1}% | Delta: {:.3}",
                    signal.symbol,
                    signal.option_type,
                    signal.strike,
                    (signal.moneyness - 1.0) * 100.0,
                    signal.edge_pct,
                    signal.delta
                );
            }
        }
        
        if !atm_sells.is_empty() {
            println!("\n{}🔴 ATM SHORT (Sell) Signals ({}):{}", RED, atm_sells.len(), RESET);
            for signal in atm_sells.iter().take(5) {
                println!("  {} {} ${:.2} ({:+.1}% from spot) | Edge: {:.1}% | Delta: {:.3}",
                    signal.symbol,
                    signal.option_type,
                    signal.strike,
                    (signal.moneyness - 1.0) * 100.0,
                    signal.edge_pct.abs(),
                    signal.delta
                );
            }
        }
        
        println!("\n💡 Why ATM options?");
        println!("  - Highest liquidity and tightest spreads");
        println!("  - Maximum gamma (fastest delta changes)");
        println!("  - Delta ≈ 0.5 for calls/puts (balanced risk)");
        println!("  - Best for delta-neutral strategies");
    }
    
    // Portfolio recommendation
    println!("\n===============================================================");
    println!("{}{}📊 PORTFOLIO RISK METRICS{}", BOLD, BLUE, RESET);
    println!("===============================================================");
    
    let top_signals: Vec<_> = all_signals.iter().take(10).collect();
    
    // Calculate portfolio-level Greeks
    let portfolio_delta: f64 = top_signals.iter().map(|s| {
        if s.action == "BUY" { s.delta } else { -s.delta }
    }).sum();
    
    let portfolio_gamma: f64 = top_signals.iter().map(|s| s.gamma).sum();
    let portfolio_vega: f64 = top_signals.iter().map(|s| s.vega).sum();
    let portfolio_theta: f64 = top_signals.iter().map(|s| s.theta).sum();
    let total_edge: f64 = top_signals.iter().map(|s| s.edge_dollars.abs()).sum();
    
    println!("\nTop 10 Positions (1 contract each):");
    println!("  Portfolio Delta:  {:>8.3}  (directional exposure)", portfolio_delta);
    println!("  Portfolio Gamma:  {:>8.4}  (convexity)", portfolio_gamma);
    println!("  Portfolio Vega:   {:>8.2}  (vol sensitivity)", portfolio_vega);
    println!("  Portfolio Theta:  {:>8.2}  (daily decay)", portfolio_theta);
    println!("  Combined Edge:    ${:>7.2}  (per contract)", total_edge);
    
    println!("\n📈 Risk Analysis:");
    if portfolio_delta.abs() < 5.0 {
        println!("  ✓ Delta-neutral: Low directional risk ({:.2})", portfolio_delta);
    } else {
        println!("  ⚠ Directional bias: Portfolio has {:.2} delta", portfolio_delta);
        println!("    Consider hedging with {} shares of underlying", -portfolio_delta as i32);
    }
    
    if portfolio_vega > 100.0 {
        println!("  ⚠ High vega: ${:.0} exposure to 1% IV change", portfolio_vega);
        println!("    Portfolio benefits if implied volatility rises");
    } else if portfolio_vega < -100.0 {
        println!("  ⚠ Negative vega: ${:.0} exposure to 1% IV change", portfolio_vega.abs());
        println!("    Portfolio benefits if implied volatility falls");
    }
    
    if portfolio_theta < -50.0 {
        println!("  ⚠ High theta decay: ${:.2}/day time decay", portfolio_theta);
        println!("    Position loses value each day - consider shorter holding period");
    }
    
    println!("\n📊 Diversification:");
    println!("  Symbols with opportunities: {}", successful.iter().filter(|r| !r.signals.is_empty()).count());
    println!("  Position size: 1-3 contracts per signal");
    println!("  Risk management: Max 2% of portfolio per symbol");
    
    if !buy_signals.is_empty() && !sell_signals.is_empty() {
        println!("\n💡 Strategy: Multi-symbol delta-neutral portfolio");
        println!("   - Diversified across {} symbols", successful.len());
        println!("   - Buy underpriced, sell overpriced options");
        println!("   - Target: Delta < ±5, positive edge");
    }
    
    println!("\n⚠️  RISK WARNINGS:");
    println!("   • Model risk: Heston assumptions may not hold");
    println!("   • Execution risk: Bid-ask spreads reduce profits");
    println!("   • Correlation risk: Symbols may move together");
    println!("   • Liquidity risk: May not get fills at quoted prices");
    println!("   • Time decay: Short-dated options lose value quickly");
    
    Ok(())
}
