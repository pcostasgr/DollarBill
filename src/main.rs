// src/main.rs
mod bs_mod;
mod csv_loader;
mod action_table_out;
mod pnl_output;
mod heston;
mod real_market_data;

use csv_loader::load_csv_closes;
use bs_mod::{compute_historical_vol, black_scholes_call};
use heston::{heston_start, MonteCarloConfig, HestonMonteCarlo};
use real_market_data::{fetch_market_data, display_summary};
use std::time::Instant;

#[tokio::main]
async fn main() {
    let symbol = "TSLA";  // Stock symbol to fetch
    let days_back = 365;
    let n_days = 10;

    let start = Instant::now();
    
    // Fetch live market data with proper error handling
    let history = match fetch_market_data(symbol, days_back).await {
        Ok(h) => {
            println!("✓ Successfully fetched {} data", symbol);
            h
        },
        Err(e) => {
            println!("✗ Failed to fetch market data: {}", e);
            println!("Falling back to CSV...");
            
            // Fallback to CSV if API fails
            match load_csv_closes("tesla_one_year.csv") {
                Ok(h) => {
                    println!("✓ Loaded from CSV backup");
                    h
                },
                Err(csv_err) => {
                    println!("✗ CSV load also failed: {}", csv_err);
                    println!("Cannot continue without data.");
                    return;
                }
            }
        }
    };
    
    let load_time = start.elapsed();
    
    // Display market data summary
    display_summary(&history);
    
    println!("Loaded {} trading days", history.len());

    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let sigma = compute_historical_vol(&closes);
    println!("Historical Volatility: {:.2}%", sigma * 100.0);

    // 1. Action table
    action_table_out::show_action_table(&history, n_days, sigma);

    // 2. P&L post-mortem
    if history.len() >= n_days {
        pnl_output::show_pnl_post_mortem(&history, n_days, sigma);
    }
    println!("CSV load time: {:.6} ms", load_time.as_secs_f64() * 1000.0);

    //Heston Model Implementation
    //-------------------------------------------------------------
    let heston_start_time = Instant::now();
    
    let current_price = *closes.last().unwrap();
    
    // Create Heston parameters easily:
    let heston_params = heston_start(
        current_price,
        sigma,
        1.0/52.0,    // 1 year to maturity
        0.05    // 5% risk-free rate
    );

    // Run simulation:
    let config = MonteCarloConfig {
        n_paths: 100000,
        n_steps: 500,
        seed: 42,
        use_antithetic: false,  // Set to true for variance reduction
    };
     // Compare with Black-Scholes
    let bs_greeks = black_scholes_call(current_price, current_price, 1.0, 0.05, sigma);
    

    let mc = HestonMonteCarlo::new(heston_params, config);
    
    // Get Heston Greeks (warning: this takes time as it re-runs simulations)
    let heston_greeks = mc.greeks_european_call(current_price);
    
    let heston_time = heston_start_time.elapsed();
    
   
    println!("\n=== Option Pricing Comparison (ATM, 1Y) ===");
    println!("Black-Scholes call: ${:.2}", bs_greeks.price);
    println!("Heston Monte Carlo:  ${:.2}", heston_greeks.price);
    println!("Difference:          ${:.2} ({:.1}%)", 
             heston_greeks.price - bs_greeks.price,
             ((heston_greeks.price - bs_greeks.price) / bs_greeks.price * 100.0));
    
    println!("\n=== Greeks Comparison ===");
    println!("             Black-Scholes    Heston MC");
    println!("Delta:       {:8.4}         {:8.4}", bs_greeks.delta, heston_greeks.delta);
    println!("Gamma:       {:8.4}         {:8.4}", bs_greeks.gamma, heston_greeks.gamma);
    println!("Vega:        {:8.2}         {:8.2}", bs_greeks.vega, heston_greeks.vega);
    println!("Theta:       {:8.2}         {:8.2}", bs_greeks.theta, heston_greeks.theta);
    println!("Rho:         {:8.2}         {:8.2}", bs_greeks.rho, heston_greeks.rho);
    
    println!("\nHeston computation time: {:.6} ms", heston_time.as_secs_f64() * 1000.0);

    // Show terminal price distribution
    mc.show_terminal_distribution(20);
    
}