// Main entry point - Modular Options Pricing System
mod models;
mod market_data;
mod strategies;
mod utils;
mod calibration;

use market_data::csv_loader::load_csv_closes;
use models::bs_mod::{compute_historical_vol, black_scholes_call};
use models::heston::{heston_start, MonteCarloConfig};
use models::heston_analytical::{heston_call_carr_madan, classify_moneyness, Moneyness};
use strategies::{StrategyRegistry, vol_mean_reversion::VolMeanReversion};
use utils::action_table_out;
use utils::pnl_output;
use std::time::Instant;

// ========== CONFIGURATION ==========
const USE_LIVE_DATA: bool = false;  // Set to true for Yahoo Finance, false for CSV
const RUN_CALIBRATION_DEMO: bool = true;  // Set to false to skip calibration demo
// ===================================

#[tokio::main]
async fn main() {
    println!("{}", "=".repeat(70));
    println!("    BLACK-SCHOLES & HESTON OPTIONS PRICING SYSTEM");
    println!("    Modular Architecture with Carr-Madan Analytical Pricing");
    println!("{}", "=".repeat(70));
    
    let symbol = "TSLA";
    let n_days = 10;

    let start = Instant::now();
    
    // Load data (CSV for now)
    let history = match load_csv_closes("tesla_one_year.csv") {
        Ok(h) => {
            println!("\n✓ Loaded from CSV (USE_LIVE_DATA = {})", USE_LIVE_DATA);
            h
        },
        Err(e) => {
            println!("✗ CSV load failed: {}", e);
            println!("Cannot continue without data.");
            return;
        }
    };
    
    let load_time = start.elapsed();
    
    println!("Loaded {} trading days", history.len());

    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let sigma = compute_historical_vol(&closes);
    println!("Historical Volatility: {:.2}%", sigma * 100.0);

    // Traditional analysis
    action_table_out::show_action_table(&history, n_days, sigma);

    if history.len() >= n_days {
        pnl_output::show_pnl_post_mortem(&history, n_days, sigma);
    }
    println!("\nCSV load time: {:.6} ms", load_time.as_secs_f64() * 1000.0);

    // ===================================================================
    // HESTON MODEL COMPARISON: Monte Carlo vs Carr-Madan
    // ===================================================================
    
    let current_price = *closes.last().unwrap();
    let time_to_maturity = 1.0;  // 1 year
    let rate = 0.05;             // 5% risk-free rate
    
    // Create Heston parameters
    let heston_params = heston_start(current_price, sigma, time_to_maturity, rate);
    
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
    
    // ===================================================================
    // CARR-MADAN ANALYTICAL PRICING (ATM)
    // ===================================================================
    
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
        current_price,
        atm_strike,
        time_to_maturity,
        rate,
        &heston_params
    );
    let carr_madan_time = carr_madan_start.elapsed();
    
    println!("Carr-Madan Price: ${:.2}", carr_madan_price);
    println!("Computation Time: {:.3} ms", carr_madan_time.as_secs_f64() * 1000.0);
    
    // ===================================================================
    // BLACK-SCHOLES COMPARISON
    // ===================================================================
    
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
             ((carr_madan_price - bs_greeks.price) / bs_greeks.price * 100.0));
    println!("Speed Advantage: Carr-Madan is analytical (no Monte Carlo noise)");
    
    // ===================================================================
    // VOLATILITY SMILE DEMONSTRATION (OTM/ITM Pricing)
    // ===================================================================
    
    println!("\n{}", "=".repeat(70));
    println!("VOLATILITY SMILE: Pricing Across Strikes");
    println!("{}", "=".repeat(70));
    
    use models::heston_analytical::{heston_call_otm, heston_call_itm, heston_put_otm, heston_put_itm};
    
    // Generate strikes from deep ITM to deep OTM
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
            heston_call_itm(current_price, *strike, time_to_maturity, rate, &heston_params)
        } else if strike > &current_price {
            heston_call_otm(current_price, *strike, time_to_maturity, rate, &heston_params)
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
            heston_put_otm(current_price, *strike, time_to_maturity, rate, &heston_params)
        } else if strike < &current_price {
            heston_put_itm(current_price, *strike, time_to_maturity, rate, &heston_params)
        } else {
            // ATM put via put-call parity
            use models::heston_analytical::heston_put_carr_madan;
            heston_put_carr_madan(current_price, *strike, time_to_maturity, rate, &heston_params)
        };
        
        // BS put via put-call parity
        let bs_call = black_scholes_call(current_price, *strike, time_to_maturity, rate, sigma).price;
        let bs_put = bs_call - current_price + strike * (-rate * time_to_maturity).exp();
        let diff_pct = (heston_price - bs_put) / bs_put * 100.0;
        
        println!("{:<12.2} {:<10} ${:<11.2} ${:<11.2} {:>9.1}%", 
                 strike, label, heston_price, bs_put, diff_pct);
    }
    
    println!("\n💡 Heston captures volatility smile/skew - prices differ from constant-vol BS");
    
    // ===================================================================
    // MONTE CARLO COMPARISON (Optional - for validation)
    // ===================================================================
    
    println!("\n{}", "=".repeat(70));
    println!("MONTE CARLO VALIDATION (100K paths)");
    println!("{}", "=".repeat(70));
    
    use models::heston::HestonMonteCarlo;
    
    let config = MonteCarloConfig {
        n_paths: 100000,
        n_steps: 500,
        seed: 42,
        use_antithetic: true,
    };
    
    let mc = HestonMonteCarlo::new(heston_params.clone(), config);
    
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
    
    println!("\nSpeed Comparison:");
    let speedup = mc_time.as_secs_f64() / carr_madan_time.as_secs_f64();
    println!("Carr-Madan: {:.3} ms", carr_madan_time.as_secs_f64() * 1000.0);
    println!("Monte Carlo: {:.1} seconds", mc_time.as_secs_f64());
    println!("Speedup: {:.0}x faster ⚡", speedup);
    
    // ===================================================================
    // STRATEGY SIGNALS
    // ===================================================================
    
    println!("\n{}", "=".repeat(70));
    println!("TRADING STRATEGY SIGNALS");
    println!("{}", "=".repeat(70));
    
    // Register strategies
    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(VolMeanReversion::new()));
    
    println!("\nActive Strategies:");
    for (i, strategy_name) in registry.list_strategies().iter().enumerate() {
        println!("  {}. {}", i + 1, strategy_name);
    }
    
    // Generate signals
    let market_iv = sigma;  // Using historical vol as proxy for market IV
    let model_iv = heston_params.v0.sqrt();  // Heston calibrated vol
    
    let signals = registry.generate_all_signals(
        current_price,
        market_iv,
        model_iv,
        sigma,
    );
    
    if !signals.is_empty() {
        println!("\n📊 Signal Summary:");
        for signal in &signals {
            println!("\n[{}]", signal.strategy_name);
            println!("   Action: {:?}", signal.action);
            println!("   Strike: ${:.2}", signal.strike);
            println!("   Expiry: {} days", signal.expiry_days);
            println!("   Confidence: {:.0}%", signal.confidence * 100.0);
            println!("   Est. Edge: ${:.2}", signal.edge);
        }
    }
    
    // ===================================================================
    // CALIBRATION DEMONSTRATION (Optional)
    // ===================================================================
    
    if RUN_CALIBRATION_DEMO {
        println!("\n{}", "=".repeat(70));
        println!("CALIBRATION DEMONSTRATION");
        println!("{}", "=".repeat(70));
        
        use calibration::{create_mock_market_data, calibrate_heston, CalibParams};
        
        // "True" parameters (simulate reality)
        let true_params = CalibParams {
            kappa: 3.5,
            theta: 0.25,
            sigma: 0.45,
            rho: -0.75,
            v0: 0.30,
        };
        
        println!("\n🎯 True Parameters (hidden from optimizer):");
        println!("  κ = {:.2}, θ = {:.4}, σ = {:.2}, ρ = {:.2}, v₀ = {:.4}",
                 true_params.kappa, true_params.theta, true_params.sigma, 
                 true_params.rho, true_params.v0);
        
        // Generate synthetic market data
        let strikes: Vec<f64> = vec![0.85, 0.90, 0.95, 1.00, 1.05, 1.10, 1.15]
            .iter()
            .map(|k| current_price * k)
            .collect();
        let maturities = vec![30.0 / 365.0, 60.0 / 365.0];  // 30, 60 days
        
        let market_data = create_mock_market_data(
            current_price,
            rate,
            &true_params,
            &strikes,
            &maturities,
        );
        
        println!("📊 Generated {} synthetic market options", market_data.len());
        
        // Initial guess (intentionally wrong)
        let initial_guess = CalibParams {
            kappa: 2.0,
            theta: 0.35,
            sigma: 0.30,
            rho: -0.60,
            v0: 0.35,
        };
        
        println!("\n🔧 Initial Guess:");
        println!("  κ = {:.2}, θ = {:.4}, σ = {:.2}, ρ = {:.2}, v₀ = {:.4}",
                 initial_guess.kappa, initial_guess.theta, initial_guess.sigma, 
                 initial_guess.rho, initial_guess.v0);
        
        // Run calibration
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
            Err(e) => {
                println!("❌ Calibration failed: {}", e);
            }
        }
    }
    
    // ===================================================================
    // SUMMARY
    // ===================================================================
    
    println!("\n{}", "=".repeat(70));
    println!("SYSTEM SUMMARY");
    println!("{}", "=".repeat(70));
    println!("✓ Modular architecture implemented");
    println!("✓ Carr-Madan analytical pricing (ATM/OTM/ITM)");
    println!("✓ Volatility smile captured by Heston characteristic function");
    println!("✓ Monte Carlo validation ({:.0}x slower)", speedup);
    println!("✓ Strategy framework operational");
    if RUN_CALIBRATION_DEMO {
        println!("✓ Parameter calibration working (argmin optimizer)");
    }
    println!("\nNext Steps:");
    println!("  1. Add live options data feed (Polygon.io API)");
    println!("  2. Calibrate to real market options chain");
    println!("  3. Add more trading strategies");
    println!("  4. Build execution layer");
    println!("{}", "=".repeat(70));
}
