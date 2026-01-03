// Heston Stochastic Volatility Model - Monte Carlo Implementation
// This implementation uses no external libraries except std

use std::f64::consts::PI;

/// Heston model parameters
#[derive(Debug, Clone)]
pub struct HestonParams {
    pub s0: f64,        // Initial stock price
    pub v0: f64,        // Initial variance
    pub kappa: f64,     // Mean reversion rate
    pub theta: f64,     // Long-term variance
    pub sigma: f64,     // Volatility of volatility (vol of vol)
    pub rho: f64,       // Correlation between asset and variance
    pub r: f64,         // Risk-free rate
    pub t: f64,         // Time to maturity
}

/// Monte Carlo simulation configuration
#[derive(Debug, Clone)]
pub struct MonteCarloConfig {
    pub n_paths: usize,     // Number of simulation paths
    pub n_steps: usize,     // Number of time steps
    pub seed: u64,          // Random seed for reproducibility
}

/// Simple Linear Congruential Generator for random numbers
struct LCG {
    state: u64,
}

impl LCG {
    fn new(seed: u64) -> Self {
        LCG { state: seed }
    }

    // Generate uniform random number in [0, 1)
    fn next_uniform(&mut self) -> f64 {
        // Using parameters from Numerical Recipes
        const A: u64 = 1664525;
        const C: u64 = 1013904223;
        const M: u64 = 4294967296; // 2^32
        
        self.state = (A.wrapping_mul(self.state).wrapping_add(C)) % M;
        self.state as f64 / M as f64
    }

    // Box-Muller transform to generate standard normal random variable
    fn next_normal(&mut self) -> f64 {
        let u1 = self.next_uniform();
        let u2 = self.next_uniform();
        
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }

    // Generate correlated normal random variables
    fn next_correlated_normals(&mut self, rho: f64) -> (f64, f64) {
        let z1 = self.next_normal();
        let z2 = self.next_normal();
        
        let w1 = z1;
        let w2 = rho * z1 + (1.0 - rho * rho).sqrt() * z2;
        
        (w1, w2)
    }
}

/// A single simulated path
pub struct HestonPath {
    pub stock_prices: Vec<f64>,
    pub variances: Vec<f64>,
}

/// Heston Monte Carlo simulator
pub struct HestonMonteCarlo {
    params: HestonParams,
    config: MonteCarloConfig,
}

/// Create Heston parameters from historical data
/// Uses reasonable defaults for parameters that can't be estimated from price history
pub fn heston_start(
    current_price: f64,
    historical_vol: f64,
    time_to_maturity: f64,
    risk_free_rate: f64,
) -> HestonParams {
    let variance = historical_vol * historical_vol;
    
    HestonParams {
        s0: current_price,
        v0: variance,               // Current variance
        kappa: 2.0,                 // Mean reversion speed (moderate)
        theta: variance,            // Long-term variance = current variance
        sigma: 0.3,                 // Vol-of-vol (30% - typical for equities)
        rho: -0.7,                  // Correlation (negative for stocks)
        r: risk_free_rate,
        t: time_to_maturity,
    }
}

impl HestonMonteCarlo {
    pub fn new(params: HestonParams, config: MonteCarloConfig) -> Self {
        HestonMonteCarlo { params, config }
    }

    /// Simulate a single path using Euler discretization with full truncation scheme
    fn simulate_path(&self, rng: &mut LCG) -> HestonPath {
        let dt = self.params.t / self.config.n_steps as f64;
        let sqrt_dt = dt.sqrt();
        
        let mut stock_prices = Vec::with_capacity(self.config.n_steps + 1);
        let mut variances = Vec::with_capacity(self.config.n_steps + 1);
        
        stock_prices.push(self.params.s0);
        variances.push(self.params.v0);
        
        for _ in 0..self.config.n_steps {
            let s = *stock_prices.last().unwrap();
            let v = *variances.last().unwrap();
            
            // Generate correlated normal random variables
            let (dw_s, dw_v) = rng.next_correlated_normals(self.params.rho);
            
            // Full truncation scheme: use max(v, 0) to prevent negative variance
            let v_pos = v.max(0.0);
            let sqrt_v = v_pos.sqrt();
            
            // Update stock price
            let s_new = s * (1.0 + self.params.r * dt + sqrt_v * sqrt_dt * dw_s);
            
            // Update variance using Euler scheme
            let v_new = v + self.params.kappa * (self.params.theta - v_pos) * dt 
                        + self.params.sigma * sqrt_v * sqrt_dt * dw_v;
            
            stock_prices.push(s_new.max(0.0)); // Ensure non-negative stock price
            variances.push(v_new);
        }
        
        HestonPath {
            stock_prices,
            variances,
        }
    }

    /// Run Monte Carlo simulation and return all paths
    pub fn simulate_paths(&self) -> Vec<HestonPath> {
        let mut rng = LCG::new(self.config.seed);
        let mut paths = Vec::with_capacity(self.config.n_paths);
        
        for _ in 0..self.config.n_paths {
            paths.push(self.simulate_path(&mut rng));
        }
        
        paths
    }

    /// Price a European call option
    pub fn price_european_call(&self, strike: f64) -> f64 {
        let mut rng = LCG::new(self.config.seed);
        let mut payoff_sum = 0.0;
        
        for _ in 0..self.config.n_paths {
            let path = self.simulate_path(&mut rng);
            let final_price = *path.stock_prices.last().unwrap();
            let payoff = (final_price - strike).max(0.0);
            payoff_sum += payoff;
        }
        
        let discount_factor = (-self.params.r * self.params.t).exp();
        discount_factor * payoff_sum / self.config.n_paths as f64
    }

    /// Price a European put option
    pub fn price_european_put(&self, strike: f64) -> f64 {
        let mut rng = LCG::new(self.config.seed);
        let mut payoff_sum = 0.0;
        
        for _ in 0..self.config.n_paths {
            let path = self.simulate_path(&mut rng);
            let final_price = *path.stock_prices.last().unwrap();
            let payoff = (strike - final_price).max(0.0);
            payoff_sum += payoff;
        }
        
        let discount_factor = (-self.params.r * self.params.t).exp();
        discount_factor * payoff_sum / self.config.n_paths as f64
    }

    /// Calculate the average final stock price across all paths
    pub fn average_final_price(&self) -> f64 {
        let mut rng = LCG::new(self.config.seed);
        let mut sum = 0.0;
        
        for _ in 0..self.config.n_paths {
            let path = self.simulate_path(&mut rng);
            sum += *path.stock_prices.last().unwrap();
        }
        
        sum / self.config.n_paths as f64
    }

    /// Calculate the average final variance across all paths
    pub fn average_final_variance(&self) -> f64 {
        let mut rng = LCG::new(self.config.seed);
        let mut sum = 0.0;
        
        for _ in 0..self.config.n_paths {
            let path = self.simulate_path(&mut rng);
            sum += *path.variances.last().unwrap();
        }
        
        sum / self.config.n_paths as f64
    }

    /// Get model parameters
    pub fn params(&self) -> &HestonParams {
        &self.params
    }

    /// Get simulation configuration
    pub fn config(&self) -> &MonteCarloConfig {
        &self.config
    }

    /// Display terminal price distribution as ASCII histogram
    pub fn show_terminal_distribution(&self, num_bins: usize) {
        let mut rng = LCG::new(self.config.seed);
        let mut final_prices = Vec::with_capacity(self.config.n_paths);
        
        // Collect all final prices
        for _ in 0..self.config.n_paths {
            let path = self.simulate_path(&mut rng);
            final_prices.push(*path.stock_prices.last().unwrap());
        }
        
        // Calculate statistics
        final_prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let min_price = *final_prices.first().unwrap();
        let max_price = *final_prices.last().unwrap();
        let mean_price = final_prices.iter().sum::<f64>() / final_prices.len() as f64;
        
        let median_price = if final_prices.len() % 2 == 0 {
            (final_prices[final_prices.len() / 2 - 1] + final_prices[final_prices.len() / 2]) / 2.0
        } else {
            final_prices[final_prices.len() / 2]
        };
        
        // Create histogram bins
        let bin_width = (max_price - min_price) / num_bins as f64;
        let mut bins = vec![0; num_bins];
        
        for &price in &final_prices {
            let bin_index = ((price - min_price) / bin_width).floor() as usize;
            let bin_index = bin_index.min(num_bins - 1);
            bins[bin_index] += 1;
        }
        
        let max_count = *bins.iter().max().unwrap();
        let scale = 50.0 / max_count as f64; // Scale to 50 chars max
        
        println!("\n=== Terminal Price Distribution (T={:.2}y) ===", self.params.t);
        println!("Statistics:");
        println!("  Mean:   ${:.2}", mean_price);
        println!("  Median: ${:.2}", median_price);
        println!("  Min:    ${:.2}", min_price);
        println!("  Max:    ${:.2}", max_price);
        println!("  StdDev: ${:.2}", 
                 (final_prices.iter().map(|x| (x - mean_price).powi(2)).sum::<f64>() 
                  / final_prices.len() as f64).sqrt());
        println!("\nHistogram ({} bins, {} paths):", num_bins, self.config.n_paths);
        
        for i in 0..num_bins {
            let bin_start = min_price + i as f64 * bin_width;
            let bin_end = bin_start + bin_width;
            let bar_length = (bins[i] as f64 * scale) as usize;
            let bar = "█".repeat(bar_length);
            let pct = bins[i] as f64 / final_prices.len() as f64 * 100.0;
            
            println!("${:6.2}-{:6.2} | {:50} {:5} ({:4.1}%)", 
                     bin_start, bin_end, bar, bins[i], pct);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heston_simulation() {
        let params = HestonParams {
            s0: 100.0,
            v0: 0.04,
            kappa: 2.0,
            theta: 0.04,
            sigma: 0.3,
            rho: -0.7,
            r: 0.05,
            t: 1.0,
        };

        let config = MonteCarloConfig {
            n_paths: 1000,
            n_steps: 252,
            seed: 42,
        };

        let mc = HestonMonteCarlo::new(params, config);
        let call_price = mc.price_european_call(100.0);
        
        // Call price should be positive and reasonable
        assert!(call_price > 0.0);
        assert!(call_price < 100.0);
    }

    #[test]
    fn test_put_call_parity() {
        let params = HestonParams {
            s0: 100.0,
            v0: 0.04,
            kappa: 2.0,
            theta: 0.04,
            sigma: 0.3,
            rho: -0.7,
            r: 0.05,
            t: 1.0,
        };

        let config = MonteCarloConfig {
            n_paths: 10000,
            n_steps: 252,
            seed: 42,
        };

        let strike = 100.0;
        let mc = HestonMonteCarlo::new(params.clone(), config);
        
        let call_price = mc.price_european_call(strike);
        let put_price = mc.price_european_put(strike);
        
        // Put-Call parity: C - P = S0 * exp(-q*T) - K * exp(-r*T)
        // With q = 0: C - P = S0 - K * exp(-r*T)
        let parity_lhs = call_price - put_price;
        let parity_rhs = params.s0 - strike * (-params.r * params.t).exp();
        
        // Allow for Monte Carlo error
        let error = (parity_lhs - parity_rhs).abs();
        assert!(error < 1.0, "Put-Call parity violated by {}", error);
    }

    #[test]
    fn test_variance_mean_reversion() {
        let params = HestonParams {
            s0: 100.0,
            v0: 0.09,      // Start above long-term variance
            kappa: 3.0,     // Strong mean reversion
            theta: 0.04,    // Long-term variance
            sigma: 0.2,
            rho: -0.5,
            r: 0.05,
            t: 5.0,         // Long time horizon
        };

        let config = MonteCarloConfig {
            n_paths: 5000,
            n_steps: 1000,
            seed: 123,
        };

        let mc = HestonMonteCarlo::new(params.clone(), config);
        let avg_final_var = mc.average_final_variance();
        
        // With strong mean reversion, final variance should be close to theta
        let error = (avg_final_var - params.theta).abs();
        assert!(error < 0.02, "Variance didn't converge to theta. Error: {}", error);
    }

    #[test]
    fn test_lcg_uniform_distribution() {
        let mut rng = LCG::new(42);
        let n_samples = 10000;
        let mut sum = 0.0;
        
        for _ in 0..n_samples {
            let u = rng.next_uniform();
            assert!(u >= 0.0 && u < 1.0);
            sum += u;
        }
        
        let mean = sum / n_samples as f64;
        // Mean should be close to 0.5
        assert!((mean - 0.5).abs() < 0.02);
    }
}
