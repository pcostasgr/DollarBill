// Generate trade signals from model-vs-market mispricings
use dollarbill::market_data::options_json_loader::{load_options_from_json, filter_liquid_options};
use dollarbill::calibration::heston_calibrator::{calibrate_heston, CalibParams};
use dollarbill::calibration::market_option::OptionType;
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
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

#[derive(Debug)]
struct TradeSignal {
    symbol: String,
    option_type: String,
    strike: f64,
    market_bid: f64,
    market_ask: f64,
    market_mid: f64,
    model_price: f64,
    edge_pct: f64,
    edge_dollars: f64,
    action: String,
    volume: i32,
}

const SIGNAL_THRESHOLD: f64 = 5.0;  // 5% edge required for signal

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===============================================================");
    println!("TRADE SIGNAL GENERATOR - Live Options Mispricing Detection");
    println!("===============================================================\n");

    // Load configuration
    let config_content = fs::read_to_string("config/signals_config.json")
        .map_err(|e| format!("Failed to read signals config file: {}", e))?;
    let config: SignalsConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse signals config file: {}", e))?;

    println!("📋 Loaded signals configuration from config/signals_config.json");

    // 1. Load live options
    let json_file = "data/tsla_options_live.json";
    let symbol = json_file.split('/').last().unwrap_or("tsla_options_live.json").split('_').next().unwrap_or("UNKNOWN").to_uppercase();
    let (spot, all_options) = load_options_from_json(json_file)?;
    let liquid_options = filter_liquid_options(all_options, config.analysis.liquidity_filters.min_volume, config.analysis.liquidity_filters.max_spread_pct);
    println!();

    // 2. Calibrate Heston model
    println!("Calibrating Heston model...");
    let initial_guess = CalibParams {
        kappa: 2.0,
        theta: 0.25,
        sigma: 0.30,
        rho: -0.60,
        v0: 0.30,
    };

    let rate = config.analysis.risk_free_rate;
    let result = calibrate_heston(spot, rate, liquid_options.clone(), initial_guess)?;
    
    println!("✓ Calibration complete (RMSE: ${:.2}, {} iterations)\n", result.final_error, result.iterations);
    
    // 3. Generate trade signals
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
        
        // Generate signal if edge > threshold
        let action = if edge_pct > SIGNAL_THRESHOLD {
            "BUY".to_string()  // Model says it's worth more than market
        } else if edge_pct < -SIGNAL_THRESHOLD {
            "SELL".to_string()  // Model says it's worth less than market
        } else {
            "HOLD".to_string()
        };
        
        if action != "HOLD" {
            signals.push(TradeSignal {
                symbol: symbol.clone(),
                option_type: match option.option_type {
                    OptionType::Call => "Call".to_string(),
                    OptionType::Put => "Put".to_string(),
                },
                strike: option.strike,
                market_bid: option.bid,
                market_ask: option.ask,
                market_mid,
                model_price,
                edge_pct,
                edge_dollars,
                action,
                volume: option.volume,
            });
        }
    }
    
    // 4. Display trade signals
    if signals.is_empty() {
        println!("⚪ No signals - market is efficiently priced (no edge > {}%)", SIGNAL_THRESHOLD);
        return Ok(());
    }
    
    // Sort by edge (best opportunities first)
    signals.sort_by(|a, b| {
        b.edge_pct.abs().partial_cmp(&a.edge_pct.abs()).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    let buy_signals: Vec<_> = signals.iter().filter(|s| s.action == "BUY").collect();
    let sell_signals: Vec<_> = signals.iter().filter(|s| s.action == "SELL").collect();
    
    println!("===============================================================");
    println!("🟢 BUY SIGNALS ({} opportunities)", buy_signals.len());
    println!("Market is underpricing these - Model says they're worth MORE");
    println!("===============================================================");
    
    if !buy_signals.is_empty() {
        println!("{:<6} {:<6} {:<8} {:<10} {:<10} {:<12} {:<10} {:<10}",
            "Symbol", "Type", "Strike", "Bid", "Ask", "Model Val", "Edge %", "Edge $");
        println!("{:-<81}", "");
        
        for signal in buy_signals.iter().take(10) {
            println!("{:<6} {:<6} ${:<7.2} ${:<9.2} ${:<9.2} ${:<11.2} {:>9.2}% ${:>9.2}",
                signal.symbol,
                signal.option_type,
                signal.strike,
                signal.market_bid,
                signal.market_ask,
                signal.model_price,
                signal.edge_pct,
                signal.edge_dollars
            );
        }
        
        println!("\n💡 Execution: BUY at ask price (pay slightly more but get immediate fill)");
        let best_buy = &buy_signals[0];
        println!("   Best opportunity: {} ${:.2} at ${:.2} (model: ${:.2}, edge: {:.1}%)",
            best_buy.option_type, best_buy.strike, best_buy.market_ask, 
            best_buy.model_price, best_buy.edge_pct);
    } else {
        println!("None found");
    }
    
    println!("\n===============================================================");
    println!("🔴 SELL SIGNALS ({} opportunities)", sell_signals.len());
    println!("Market is overpricing these - Model says they're worth LESS");
    println!("===============================================================");
    
    if !sell_signals.is_empty() {
        println!("{:<6} {:<6} {:<8} {:<10} {:<10} {:<12} {:<10} {:<10}",
            "Symbol", "Type", "Strike", "Bid", "Ask", "Model Val", "Edge %", "Edge $");
        println!("{:-<81}", "");
        
        for signal in sell_signals.iter().take(10) {
            println!("{:<6} {:<6} ${:<7.2} ${:<9.2} ${:<9.2} ${:<11.2} {:>9.2}% ${:>9.2}",
                signal.symbol,
                signal.option_type,
                signal.strike,
                signal.market_bid,
                signal.market_ask,
                signal.model_price,
                signal.edge_pct,
                signal.edge_dollars
            );
        }
        
        println!("\n💡 Execution: SELL at bid price (receive slightly less but get immediate fill)");
        let best_sell = &sell_signals[0];
        println!("   Best opportunity: {} ${:.2} at ${:.2} (model: ${:.2}, edge: {:.1}%)",
            best_sell.option_type, best_sell.strike, best_sell.market_bid,
            best_sell.model_price, best_sell.edge_pct.abs());
    } else {
        println!("None found");
    }
    
    // 5. Portfolio suggestion
    println!("\n===============================================================");
    println!("📊 PORTFOLIO RECOMMENDATION");
    println!("===============================================================");
    
    let total_edge: f64 = signals.iter().take(5).map(|s| s.edge_dollars.abs()).sum();
    println!("Top 5 signals combined edge: ${:.2} per contract", total_edge);
    println!("Position size: Start with 1-5 contracts per signal");
    println!("Risk management: Use 2-3% of portfolio per trade");
    
    if !buy_signals.is_empty() && !sell_signals.is_empty() {
        println!("\n💡 Strategy: Delta-neutral portfolio");
        println!("   - Buy underpriced options");
        println!("   - Sell overpriced options");
        println!("   - Net delta close to zero → profit from mispricing, not direction");
    }
    
    println!("\n⚠️  RISK WARNINGS:");
    println!("   • These are 5-day expiration options (high theta decay)");
    println!("   • Model assumes continuous hedging (not realistic for retail)");
    println!("   • Bid-ask spread eats into profits");
    println!("   • Liquidity risk: may not get fills at quoted prices");
    println!("   • Model risk: Heston may not capture all market dynamics");
    
    println!("\n✅ Next steps:");
    println!("   1. Review signals manually (don't blindly follow)");
    println!("   2. Check option Greeks (delta, gamma, vega)");
    println!("   3. Size positions appropriately");
    println!("   4. Set stop losses");
    println!("   5. Monitor P&L daily");
    
    Ok(())
}
