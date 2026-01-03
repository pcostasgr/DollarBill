// Heston parameter calibration - SIMPLIFIED VERSION (no argmin for now)
// Using basic Nelder-Mead implementation

use crate::models::heston::HestonParams;
use crate::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use crate::calibration::market_option::{MarketOption, OptionType};

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

/// Calibrate Heston parameters (simplified - just returns initial guess for now)
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
    
    let initial_error = calculate_error(&initial_guess, spot, rate, &market_data);
    
    // For now, just return the initial guess
    // TODO: Implement actual optimization (Nelder-Mead or similar)
    let final_params = initial_guess.clone();
    let final_error = initial_error;
    
    println!("⚠ NOTE: Calibration not yet implemented - returning initial guess");
    
    Ok(CalibrationResult {
        params: final_params,
        rmse: final_error,
        iterations: 0,
        success: false,
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

