#![allow(dead_code)]
// Heston parameter calibration using custom Nelder-Mead optimizer

use crate::models::heston::HestonParams;
use crate::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use crate::calibration::market_option::{MarketOption, OptionType};
use crate::calibration::nelder_mead::{NelderMead, NelderMeadConfig};

/// Compact calibration parameters (just the 5 we're fitting)
#[derive(Debug, Clone)]
pub struct CalibParams {
    pub kappa: f64,
    pub theta: f64,
    pub sigma: f64,
    pub rho: f64,
    pub v0: f64,
}

impl CalibParams {
    pub fn to_heston(&self, spot: f64, rate: f64, time: f64) -> HestonParams {
        HestonParams {
            s0: spot,
            v0: self.v0,
            kappa: self.kappa,
            theta: self.theta,
            sigma: self.sigma,
            rho: self.rho,
            r: rate,
            t: time,
        }
    }
}

/// Result of Heston calibration
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub params: CalibParams,
    pub rmse: f64,
    pub iterations: u64,
    pub success: bool,
    pub initial_error: f64,
    pub final_error: f64,
}

impl CalibrationResult {
    pub fn to_heston(&self, spot: f64, rate: f64, time: f64) -> HestonParams {
        self.params.to_heston(spot, rate, time)
    }
    
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(70));
        println!("HESTON CALIBRATION RESULTS");
        println!("{}", "=".repeat(70));
        
        if self.success {
            println!("✓ Calibration succeeded");
        } else {
            println!("⚠ Calibration may not have converged");
        }
        
        println!("\nCalibrated Parameters:");
        println!("  κ (kappa):     {:.4}  (mean reversion speed)", self.params.kappa);
        println!("  θ (theta):     {:.4}  (long-term variance, vol={:.2}%)", 
                 self.params.theta, self.params.theta.sqrt() * 100.0);
        println!("  σ (sigma):     {:.4}  (vol-of-vol)", self.params.sigma);
        println!("  ρ (rho):       {:.4}  (correlation)", self.params.rho);
        println!("  v₀ (v0):       {:.4}  (initial variance, vol={:.2}%)", 
                 self.params.v0, self.params.v0.sqrt() * 100.0);
        
        println!("\nFeller Condition: 2κθ = {:.4} {} σ² = {:.4}",
                 2.0 * self.params.kappa * self.params.theta,
                 if 2.0 * self.params.kappa * self.params.theta > self.params.sigma.powi(2) { ">" } else { "≤" },
                 self.params.sigma.powi(2));
        
        println!("\nFit Quality:");
        println!("  Initial RMSE:  ${:.4}", self.initial_error);
        println!("  Final RMSE:    ${:.4}", self.final_error);
        println!("  Improvement:   {:.1}%", 
                 (self.initial_error - self.final_error) / self.initial_error * 100.0);
        println!("  Iterations:    {}", self.iterations);
        println!("{}", "=".repeat(70));
    }
}

/// Calculate calibration error for given parameters
fn calculate_error(
    params: &CalibParams,
    spot: f64,
    rate: f64,
    market_data: &[MarketOption],
) -> f64 {
    let mut total_error = 0.0;
    let mut total_weight = 0.0;
    
    for option in market_data {
        let heston = params.to_heston(spot, rate, option.time_to_expiry);
        
        let model_price = match option.option_type {
            OptionType::Call => heston_call_carr_madan(
                spot,
                option.strike,
                option.time_to_expiry,
                rate,
                &heston,
            ),
            OptionType::Put => heston_put_carr_madan(
                spot,
                option.strike,
                option.time_to_expiry,
                rate,
                &heston,
            ),
        };
        
        let market_price = option.mid_price();
        let weight = 1.0 / (option.spread() + 0.01);
        
        let error = (model_price - market_price).powi(2) * weight;
        total_error += error;
        total_weight += weight;
    }
    
    (total_error / total_weight).sqrt()
}

/// Check if parameters satisfy bounds
fn check_bounds(params: &[f64]) -> bool {
    let kappa = params[0];
    let theta = params[1];
    let sigma = params[2];
    let rho = params[3];
    let v0 = params[4];
    
    kappa >= 0.01 && kappa <= 10.0
        && theta >= 0.01 && theta <= 2.0
        && sigma >= 0.01 && sigma <= 1.5
        && rho >= -1.0 && rho <= 0.0
        && v0 >= 0.01 && v0 <= 2.0
}

/// Check Feller condition
fn check_feller(params: &[f64]) -> bool {
    let kappa = params[0];
    let theta = params[1];
    let sigma = params[2];
    2.0 * kappa * theta > sigma.powi(2)
}

/// Calibrate Heston parameters using Nelder-Mead optimization
pub fn calibrate_heston(
    spot: f64,
    rate: f64,
    market_data: Vec<MarketOption>,
    initial_guess: CalibParams,
) -> Result<CalibrationResult, String> {
    if market_data.is_empty() {
        return Err("No market data provided for calibration".to_string());
    }
    
    println!("Starting Heston calibration with {} options...", market_data.len());
    
    // Convert initial guess to vector
    let initial_params = vec![
        initial_guess.kappa,
        initial_guess.theta,
        initial_guess.sigma,
        initial_guess.rho,
        initial_guess.v0,
    ];
    
    let initial_error = {
        let calib_params = CalibParams {
            kappa: initial_params[0],
            theta: initial_params[1],
            sigma: initial_params[2],
            rho: initial_params[3],
            v0: initial_params[4],
        };
        calculate_error(&calib_params, spot, rate, &market_data)
    };
    
    // Define objective function with penalties for constraint violations
    let objective = |params: &[f64]| {
        // Check bounds
        if !check_bounds(params) {
            return 1e10;  // Penalty for out of bounds
        }
        
        // Check Feller condition
        if !check_feller(params) {
            return 1e10;  // Penalty for violating Feller
        }
        
        let calib_params = CalibParams {
            kappa: params[0],
            theta: params[1],
            sigma: params[2],
            rho: params[3],
            v0: params[4],
        };
        
        calculate_error(&calib_params, spot, rate, &market_data)
    };
    
    // Run Nelder-Mead optimization
    let config = NelderMeadConfig {
        max_iterations: 500,
        tolerance: 1e-6,
        ..Default::default()
    };
    
    let optimizer = NelderMead::new(config);
    let result = optimizer.minimize(objective, initial_params);
    
    let final_params = CalibParams {
        kappa: result.best_params[0],
        theta: result.best_params[1],
        sigma: result.best_params[2],
        rho: result.best_params[3],
        v0: result.best_params[4],
    };
    
    let final_error = result.best_value;
    let success = result.converged && final_error < initial_error * 0.5;
    
    Ok(CalibrationResult {
        params: final_params,
        rmse: final_error,
        iterations: result.iterations as u64,
        success,
        initial_error,
        final_error,
    })
}

/// Create synthetic market data for testing
pub fn create_mock_market_data(
    spot: f64,
    rate: f64,
    true_params: &CalibParams,
    strikes: &[f64],
    maturities: &[f64],
) -> Vec<MarketOption> {
    let mut market_data = Vec::new();
    
    for &strike in strikes {
        for &maturity in maturities {
            let heston = true_params.to_heston(spot, rate, maturity);
            
            let true_price = heston_call_carr_madan(
                spot, strike, maturity, rate, &heston
            );
            
            let spread = true_price * 0.03;
            let bid = true_price - spread / 2.0;
            let ask = true_price + spread / 2.0;
            
            market_data.push(MarketOption {
                strike,
                time_to_expiry: maturity,
                bid,
                ask,
                option_type: OptionType::Call,
                volume: 100,
                open_interest: 500,
            });
        }
    }
    
    market_data
}

