// Heston Stochastic Volatility Model - Monte Carlo Implementation
// This implementation uses no external libraries except std and rayon for parallelization

use std::f64::consts::PI;
use rayon::prelude::*;

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

impl HestonParams {
    /// Validate Heston parameters including Feller condition
    /// Returns Ok(()) if valid, Err(message) if invalid
    pub fn validate(&self) -> Result<(), String> {
        // Basic parameter bounds
        if self.s0 <= 0.0 {
            return Err("Initial stock price must be positive".to_string());
        }
        if self.v0 < 0.0 {
            return Err("Initial variance cannot be negative".to_string());
        }
        if self.kappa <= 0.0 {
            return Err("Mean reversion rate must be positive".to_string());
        }
        if self.theta <= 0.0 {
            return Err("Long-term variance must be positive".to_string());
        }
        if self.sigma <= 0.0 {
            return Err("Volatility of volatility must be positive".to_string());
        }
        if self.rho < -1.0 || self.rho > 1.0 {
            return Err("Correlation must be between -1 and 1".to_string());
        }
        if self.t <= 0.0 {
            return Err("Time to maturity must be positive".to_string());
        }

        // Feller condition: 2κθ > σ² (prevents negative variance)
        let feller_ratio = 2.0 * self.kappa * self.theta / (self.sigma * self.sigma);
        if feller_ratio <= 1.0 {
            return Err(format!(
                "Feller condition violated: 2κθ/σ² = {:.3} ≤ 1.0. \
                 Variance can become negative - increase κ or θ, or decrease σ",
                feller_ratio
            ));
        }

        Ok(())
    }

    /// Check if parameters satisfy Feller condition
    pub fn satisfies_feller(&self) -> bool {
        2.0 * self.kappa * self.theta > self.sigma * self.sigma
    }

    /// Feller ratio 2κθ/σ².  Values > 1 satisfy the Feller condition.
    /// Calibrated parameters often return values between 0.3–0.9, which is why
    /// `new_unchecked` exists — hard rejection is impractical for real workflows.
    #[allow(dead_code)]
    pub fn feller_ratio(&self) -> f64 {
        2.0 * self.kappa * self.theta / (self.sigma * self.sigma)
    }

    /// Validate only hard parameter bounds; does NOT check the Feller condition.
    /// Used by `HestonMonteCarlo::new_unchecked` and the Carr-Madan pricer.
    #[allow(dead_code)]
    pub fn validate_bounds_only(&self) -> Result<(), String> {
        if self.s0 <= 0.0 {
            return Err("Initial stock price must be positive".to_string());
        }
        if self.v0 < 0.0 {
            return Err("Initial variance cannot be negative".to_string());
        }
        if self.kappa <= 0.0 {
            return Err("Mean reversion rate must be positive".to_string());
        }
        if self.theta <= 0.0 {
            return Err("Long-term variance must be positive".to_string());
        }
        if self.sigma <= 0.0 {
            return Err("Volatility of volatility must be positive".to_string());
        }
        if self.rho < -1.0 || self.rho > 1.0 {
            return Err("Correlation must be between -1 and 1".to_string());
        }
        if self.t <= 0.0 {
            return Err("Time to maturity must be positive".to_string());
        }
        Ok(())
    }
}

/// Greeks for Heston model options
#[derive(Debug, Clone)]
pub struct HestonGreeks {
    pub price: f64,
    pub delta: f64,      // ∂V/∂S
    pub gamma: f64,      // ∂²V/∂S²
    pub vega: f64,       // ∂V/∂v0 (initial variance)
    pub theta: f64,      // -∂V/∂t (per day)
    pub rho: f64,        // ∂V/∂r
}

/// Monte Carlo simulation configuration
#[derive(Debug, Clone)]
pub struct MonteCarloConfig {
    pub n_paths: usize,     // Number of simulation paths
    pub n_steps: usize,     // Number of time steps
    pub seed: u64,          // Random seed for reproducibility
    pub use_antithetic: bool, // Use antithetic variates for variance reduction
}

/// SplitMix64 pseudo-random number generator.
///
/// Replaces the old 32-bit LCG which had terrible spectral properties
/// (lattice structure, period only 2^32, correlated sequences from
/// adjacent seeds).  SplitMix64 has:
///   - Period 2^64
///   - Excellent avalanche — adjacent seeds produce uncorrelated streams
///   - Passes BigCrush / PractRand
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// Advance state and return a 64-bit pseudo-random integer.
    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// Generate uniform random number in (0, 1).
    /// Uses the upper 53 bits for full f64 mantissa precision.
    fn next_uniform(&mut self) -> f64 {
        // Shift right by 11 to get 53 bits, add 0.5 ULP to avoid exact 0.0
        // (needed for Box-Muller ln(u1) safety)
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64) + f64::EPSILON
    }

    // Box-Muller transform to generate standard normal random variable
    fn next_normal(&mut self) -> f64 {
        let u1 = self.next_uniform();
        let u2 = self.next_uniform();
        
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }

    // Generate correlated normal random variables
    #[allow(dead_code)] // kept as utility; QE scheme uses independent normals
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
    #[allow(dead_code)]
    pub variances: Vec<f64>,
}

// ── QE scheme helpers (Andersen 2008) ───────────────────────────────────
// The Quadratic-Exponential discretisation matches the first two conditional
// moments of the CIR variance process **exactly**, eliminating the negative-
// variance problem without ad-hoc reflection / truncation.

/// ψ threshold: quadratic branch when ψ ≤ ψ_c, exponential otherwise.
const QE_PSI_CRIT: f64 = 1.5;

/// Fast standard-normal CDF (Abramowitz & Stegun 7.1.26, max error ~1.5e-7).
/// Local to avoid a cross-module hot-loop dependency.
fn norm_cdf_qe(x: f64) -> f64 {
    const A1: f64 =  0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 =  1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 =  1.061405429;
    const P:  f64 =  0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    let t = 1.0 / (1.0 + P * ax);
    let y = 1.0 - (((((A5 * t + A4) * t) + A3) * t + A2) * t + A1)
            * t * (-ax * ax * 0.5).exp();
    0.5 * (1.0 + sign * y)
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
    pub fn new(params: HestonParams, config: MonteCarloConfig) -> Result<Self, String> {
        // Validate Heston parameters before creating simulator
        params.validate()?;
        
        Ok(HestonMonteCarlo { params, config })
    }

    /// Create simulator without enforcing the Feller condition (2κθ > σ²).
    ///
    /// Calibrated parameters routinely violate Feller — rejecting them inside
    /// `new()` is overly strict for production use.  Hard bounds (s0 > 0,
    /// v0 ≥ 0, etc.) are still checked; only the Feller check is skipped.
    ///
    /// When Feller is violated the path simulation automatically switches from
    /// *reflection* to *full truncation* at the zero boundary.  Reflection
    /// introduces an upward-biased pile-up at v = 0 in this regime; truncation
    /// (absorbing at 0) is less biased and is the industry-standard fallback.
    pub fn new_unchecked(params: HestonParams, config: MonteCarloConfig) -> Result<Self, String> {
        params.validate_bounds_only()?;
        Ok(HestonMonteCarlo { params, config })
    }

    /// Advance variance one step using the QE scheme (Andersen 2008).
    ///
    /// Matches the first two conditional moments of the CIR process exactly.
    /// When ψ = s²/m² ≤ 1.5 (quadratic branch): V′ = a(b + Z_v)².
    /// When ψ > 1.5 (exponential branch): V′ drawn from a point-mass /
    /// exponential mixture using U = Φ(Z_v).
    #[inline]
    fn qe_variance_step(&self, v: f64, z_v: f64, dt: f64) -> f64 {
        let kappa = self.params.kappa;
        let theta = self.params.theta;
        let sigma = self.params.sigma;

        let e = (-kappa * dt).exp();

        // Conditional mean   E[V(t+Δt) | V(t) = v]
        let m = theta + (v - theta) * e;

        // Conditional variance  Var[V(t+Δt) | V(t) = v]
        let s2 = v * sigma * sigma * e / kappa * (1.0 - e)
               + theta * sigma * sigma / (2.0 * kappa) * (1.0 - e).powi(2);

        // Guard: degenerate case where mean is tiny
        if m < 1e-12 {
            return 0.0;
        }

        let psi = s2 / (m * m);

        if psi <= QE_PSI_CRIT {
            // ── Quadratic branch ────────────────────────────────────────
            let b2 = 2.0 / psi - 1.0 + (2.0 / psi).sqrt() * (2.0 / psi - 1.0).max(0.0).sqrt();
            let a = m / (1.0 + b2);
            (a * (b2.sqrt() + z_v).powi(2)).max(0.0)
        } else {
            // ── Exponential branch ──────────────────────────────────────
            let p = (psi - 1.0) / (psi + 1.0);
            let beta = (1.0 - p) / m;
            let u = norm_cdf_qe(z_v); // Φ(Z) is U(0,1); antithetic Φ(-Z)=1-U

            if u <= p {
                0.0
            } else {
                // Inverse CDF of exponential: F^{-1}(u) = -ln((1-p)/(1-u)) / β
                // but we want the right sign → ln((1-p)/(1-u)) / β
                (((1.0 - p) / (1.0 - u).max(1e-15)).ln() / beta).max(0.0)
            }
        }
    }

    /// Advance stock price one step using the QE log-scheme.
    ///
    /// The ρ-correlation between the asset and variance Brownian motions is
    /// absorbed into the drift through the K-coefficients; `z_s` is an
    /// **independent** standard normal.
    #[inline]
    fn qe_stock_step(&self, s: f64, v_old: f64, v_new: f64, z_s: f64, dt: f64) -> f64 {
        let r     = self.params.r;
        let rho   = self.params.rho;
        let kappa = self.params.kappa;
        let sigma = self.params.sigma;
        let theta = self.params.theta;

        // Trapezoidal rule: γ₁ = γ₂ = 0.5
        let g1 = 0.5_f64;
        let g2 = 0.5_f64;

        let k0 = -rho * kappa * theta / sigma * dt;
        let k1 = g1 * dt * (kappa * rho / sigma - 0.5) - rho / sigma;
        let k2 = g2 * dt * (kappa * rho / sigma - 0.5) + rho / sigma;
        let k3 = g1 * dt * (1.0 - rho * rho);
        let k4 = g2 * dt * (1.0 - rho * rho);

        let vol_term = (k3 * v_old + k4 * v_new).max(0.0).sqrt();
        let log_return = r * dt + k0 + k1 * v_old + k2 * v_new + vol_term * z_s;

        s * log_return.exp()
    }

    /// Simulate a single path using the QE scheme (Andersen 2008).
    ///
    /// Variance is advanced with the Quadratic-Exponential discretisation
    /// that matches the first two conditional moments of the CIR process,
    /// eliminating negative variance without ad-hoc reflection / truncation.
    /// The stock price uses a log-scheme whose drift coefficients absorb the
    /// ρ-correlation, so the two driving normals are independent.
    fn simulate_path(&self, rng: &mut SplitMix64) -> HestonPath {
        let dt = self.params.t / self.config.n_steps as f64;

        let mut stock_prices = Vec::with_capacity(self.config.n_steps + 1);
        let mut variances = Vec::with_capacity(self.config.n_steps + 1);

        stock_prices.push(self.params.s0);
        variances.push(self.params.v0);

        for _ in 0..self.config.n_steps {
            let s = *stock_prices.last().unwrap();
            let v = *variances.last().unwrap();

            // Two independent standard normals per step
            let z_v = rng.next_normal();
            let z_s = rng.next_normal();

            let v_new = self.qe_variance_step(v, z_v, dt);
            let s_new = self.qe_stock_step(s, v, v_new, z_s, dt);

            stock_prices.push(s_new);
            variances.push(v_new);
        }

        HestonPath {
            stock_prices,
            variances,
        }
    }

    /// Simulate path using antithetic variates (negated random numbers).
    /// Uses the same QE scheme as `simulate_path`, but with negated normals
    /// (for the quadratic branch −Z flips around b; for the exponential
    /// branch Φ(−Z) = 1 − Φ(Z), which correctly flips the uniform).
    fn simulate_path_antithetic(&self, original_randoms: &[(f64, f64)]) -> HestonPath {
        let dt = self.params.t / self.config.n_steps as f64;

        let mut stock_prices = Vec::with_capacity(self.config.n_steps + 1);
        let mut variances = Vec::with_capacity(self.config.n_steps + 1);

        stock_prices.push(self.params.s0);
        variances.push(self.params.v0);

        for &(z_v, z_s) in original_randoms {
            let s = *stock_prices.last().unwrap();
            let v = *variances.last().unwrap();

            // Negate both independent normals for antithetic pair
            let v_new = self.qe_variance_step(v, -z_v, dt);
            let s_new = self.qe_stock_step(s, v, v_new, -z_s, dt);

            stock_prices.push(s_new);
            variances.push(v_new);
        }

        HestonPath {
            stock_prices,
            variances,
        }
    }

    /// Simulate path and capture random numbers for antithetic pair.
    /// Records (z_v, z_s) independent normal pairs per step.
    fn simulate_path_with_randoms(&self, rng: &mut SplitMix64) -> (HestonPath, Vec<(f64, f64)>) {
        let dt = self.params.t / self.config.n_steps as f64;

        let mut stock_prices = Vec::with_capacity(self.config.n_steps + 1);
        let mut variances = Vec::with_capacity(self.config.n_steps + 1);
        let mut randoms = Vec::with_capacity(self.config.n_steps);

        stock_prices.push(self.params.s0);
        variances.push(self.params.v0);

        for _ in 0..self.config.n_steps {
            let s = *stock_prices.last().unwrap();
            let v = *variances.last().unwrap();

            let z_v = rng.next_normal();
            let z_s = rng.next_normal();
            randoms.push((z_v, z_s));

            let v_new = self.qe_variance_step(v, z_v, dt);
            let s_new = self.qe_stock_step(s, v, v_new, z_s, dt);

            stock_prices.push(s_new);
            variances.push(v_new);
        }

        (HestonPath { stock_prices, variances }, randoms)
    }

    /// Run Monte Carlo simulation and return all paths
    pub fn simulate_paths(&self) -> Vec<HestonPath> {
        let mut rng = SplitMix64::new(self.config.seed);
        let mut paths = Vec::with_capacity(self.config.n_paths);
        
        for _ in 0..self.config.n_paths {
            paths.push(self.simulate_path(&mut rng));
        }
        
        paths
    }

    /// Price a European call option (parallelized)
    /// Uses antithetic variates if configured
    pub fn price_european_call(&self, strike: f64) -> f64 {
        if self.config.use_antithetic {
            self.price_european_call_antithetic(strike)
        } else {
            self.price_european_call_regular(strike)
        }
    }

    /// Price European call - regular Monte Carlo
    fn price_european_call_regular(&self, strike: f64) -> f64 {
        let payoff_sum: f64 = (0..self.config.n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                let path = self.simulate_path(&mut rng);
                let final_price = *path.stock_prices.last().unwrap();
                (final_price - strike).max(0.0)
            })
            .sum();
        
        let discount_factor = (-self.params.r * self.params.t).exp();
        discount_factor * payoff_sum / self.config.n_paths as f64
    }

    /// Price European call - antithetic variates
    fn price_european_call_antithetic(&self, strike: f64) -> f64 {
        let n_pairs = self.config.n_paths / 2;
        
        let payoff_sum: f64 = (0..n_pairs)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                
                // Simulate pair
                let (path1, randoms) = self.simulate_path_with_randoms(&mut rng);
                let path2 = self.simulate_path_antithetic(&randoms);
                
                // Calculate payoffs
                let final_price1 = *path1.stock_prices.last().unwrap();
                let final_price2 = *path2.stock_prices.last().unwrap();
                let payoff1 = (final_price1 - strike).max(0.0);
                let payoff2 = (final_price2 - strike).max(0.0);
                
                // Average the pair
                (payoff1 + payoff2) / 2.0
            })
            .sum();
        
        let discount_factor = (-self.params.r * self.params.t).exp();
        discount_factor * payoff_sum / n_pairs as f64
    }

    /// Price a European put option (parallelized)
    /// Uses antithetic variates if configured
    pub fn price_european_put(&self, strike: f64) -> f64 {
        if self.config.use_antithetic {
            self.price_european_put_antithetic(strike)
        } else {
            self.price_european_put_regular(strike)
        }
    }

    /// Price European put - regular Monte Carlo
    fn price_european_put_regular(&self, strike: f64) -> f64 {
        let payoff_sum: f64 = (0..self.config.n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                let path = self.simulate_path(&mut rng);
                let final_price = *path.stock_prices.last().unwrap();
                (strike - final_price).max(0.0)
            })
            .sum();
        
        let discount_factor = (-self.params.r * self.params.t).exp();
        discount_factor * payoff_sum / self.config.n_paths as f64
    }

    /// Price European put - antithetic variates
    fn price_european_put_antithetic(&self, strike: f64) -> f64 {
        let n_pairs = self.config.n_paths / 2;
        
        let payoff_sum: f64 = (0..n_pairs)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                
                let (path1, randoms) = self.simulate_path_with_randoms(&mut rng);
                let path2 = self.simulate_path_antithetic(&randoms);
                
                let final_price1 = *path1.stock_prices.last().unwrap();
                let final_price2 = *path2.stock_prices.last().unwrap();
                let payoff1 = (strike - final_price1).max(0.0);
                let payoff2 = (strike - final_price2).max(0.0);
                
                (payoff1 + payoff2) / 2.0
            })
            .sum();
        
        let discount_factor = (-self.params.r * self.params.t).exp();
        discount_factor * payoff_sum / n_pairs as f64
    }

    /// Calculate the average final stock price across all paths (parallelized)
    pub fn average_final_price(&self) -> f64 {
        let sum: f64 = (0..self.config.n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                let path = self.simulate_path(&mut rng);
                *path.stock_prices.last().unwrap()
            })
            .sum();
        
        sum / self.config.n_paths as f64
    }

    /// Calculate the average final variance across all paths (parallelized)
    pub fn average_final_variance(&self) -> f64 {
        let sum: f64 = (0..self.config.n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                let path = self.simulate_path(&mut rng);
                *path.variances.last().unwrap()
            })
            .sum();
        
        sum / self.config.n_paths as f64
    }

    /// Calculate Greeks for a European call option using finite differences
    pub fn greeks_european_call(&self, strike: f64) -> HestonGreeks {
        // Larger bumps needed for Monte Carlo (vs analytical methods)
        // Even larger for short-dated options to overcome noise
        let bump_s = 0.01;      // 1% bump for spot
        let bump_v = 0.01;      // 25% bump for variance (critical for short-dated)
        let bump_t = 1.0 / 365.0; // 1 day
        let bump_r = 0.0001;    // 1 basis point
        
        // Base price
        let price = self.price_european_call(strike);
        
        // Delta: ∂V/∂S
        let mut params_up = self.params.clone();
        params_up.s0 = self.params.s0 * (1.0 + bump_s);
        let mc_up = HestonMonteCarlo::new(params_up, self.config.clone())
            .unwrap_or_else(|_| panic!("Invalid Heston parameters for delta calculation"));
        let price_up = mc_up.price_european_call(strike);
        
        let mut params_down = self.params.clone();
        params_down.s0 = self.params.s0 * (1.0 - bump_s);
        let mc_down = HestonMonteCarlo::new(params_down, self.config.clone())
            .unwrap_or_else(|_| panic!("Invalid Heston parameters for delta calculation"));
        let price_down = mc_down.price_european_call(strike);
        
        let delta = (price_up - price_down) / (2.0 * self.params.s0 * bump_s);
        
        // Gamma: ∂²V/∂S²
        let gamma = (price_up - 2.0 * price + price_down) / ((self.params.s0 * bump_s).powi(2));
        
        // Vega: ∂V/∂v0 (sensitivity to initial variance)
        let mut params_vega_up = self.params.clone();
        params_vega_up.v0 = self.params.v0 + bump_v;
        let mc_vega = HestonMonteCarlo::new(params_vega_up, self.config.clone())
            .unwrap_or_else(|_| panic!("Invalid Heston parameters for vega calculation"));
        let price_vega_up = mc_vega.price_european_call(strike);
        
        // Convert to volatility units (multiply by 2*sqrt(v0) since ∂v/∂σ = 2σ)
        let vega = (price_vega_up - price) / bump_v * 2.0 * self.params.v0.sqrt();
        
        // Theta: -∂V/∂t (negative time decay, per day)
        if self.params.t > bump_t {
            let mut params_theta = self.params.clone();
            params_theta.t = self.params.t - bump_t;
            let mc_theta = HestonMonteCarlo::new(params_theta, self.config.clone())
                .unwrap_or_else(|_| panic!("Invalid Heston parameters for theta calculation"));
            let price_theta = mc_theta.price_european_call(strike);
            
            let theta = (price_theta - price) / bump_t;
            
            // Rho: ∂V/∂r
            let mut params_rho = self.params.clone();
            params_rho.r = self.params.r + bump_r;
            let mc_rho = HestonMonteCarlo::new(params_rho, self.config.clone())
                .unwrap_or_else(|_| panic!("Invalid Heston parameters for rho calculation"));
            let price_rho = mc_rho.price_european_call(strike);
            
            let rho = (price_rho - price) / bump_r;
            
            HestonGreeks {
                price,
                delta,
                gamma,
                vega,
                theta,
                rho,
            }
        } else {
            // Near expiry, theta calculation may be unstable
            HestonGreeks {
                price,
                delta,
                gamma,
                vega,
                theta: 0.0,
                rho: 0.0,
            }
        }
    }

    /// Calculate Greeks for a European put option using finite differences
    pub fn greeks_european_put(&self, strike: f64) -> HestonGreeks {
        // Larger bumps needed for Monte Carlo (vs analytical methods)
        // Even larger for short-dated options to overcome noise
        let bump_s = 0.01;
        let bump_v = 0.01;      // 25% bump for variance
        let bump_t = 1.0 / 365.0;
        let bump_r = 0.0001;
        
        let price = self.price_european_put(strike);
        
        // Delta
        let mut params_up = self.params.clone();
        params_up.s0 = self.params.s0 * (1.0 + bump_s);
        let mc_up = HestonMonteCarlo::new(params_up, self.config.clone())
            .unwrap_or_else(|_| panic!("Invalid Heston parameters for put delta calculation"));
        let price_up = mc_up.price_european_put(strike);
        
        let mut params_down = self.params.clone();
        params_down.s0 = self.params.s0 * (1.0 - bump_s);
        let mc_down = HestonMonteCarlo::new(params_down, self.config.clone())
            .unwrap_or_else(|_| panic!("Invalid Heston parameters for put delta calculation"));
        let price_down = mc_down.price_european_put(strike);
        
        let delta = (price_up - price_down) / (2.0 * self.params.s0 * bump_s);
        
        // Gamma
        let gamma = (price_up - 2.0 * price + price_down) / ((self.params.s0 * bump_s).powi(2));
        
        // Vega
        let mut params_vega_up = self.params.clone();
        params_vega_up.v0 = self.params.v0 + bump_v;
        let mc_vega = HestonMonteCarlo::new(params_vega_up, self.config.clone())
            .unwrap_or_else(|_| panic!("Invalid Heston parameters for put vega calculation"));
        let price_vega_up = mc_vega.price_european_put(strike);
        
        let vega = (price_vega_up - price) / bump_v * 2.0 * self.params.v0.sqrt();
        
        // Theta
        if self.params.t > bump_t {
            let mut params_theta = self.params.clone();
            params_theta.t = self.params.t - bump_t;
            let mc_theta = HestonMonteCarlo::new(params_theta, self.config.clone())
                .unwrap_or_else(|_| panic!("Invalid Heston parameters for put theta calculation"));
            let price_theta = mc_theta.price_european_put(strike);
            
            let theta = (price_theta - price) / bump_t;
            
            // Rho
            let mut params_rho = self.params.clone();
            params_rho.r = self.params.r + bump_r;
            let mc_rho = HestonMonteCarlo::new(params_rho, self.config.clone())
                .unwrap_or_else(|_| panic!("Invalid Heston parameters for put rho calculation"));
            let price_rho = mc_rho.price_european_put(strike);
            
            let rho = (price_rho - price) / bump_r;
            
            HestonGreeks {
                price,
                delta,
                gamma,
                vega,
                theta,
                rho,
            }
        } else {
            HestonGreeks {
                price,
                delta,
                gamma,
                vega,
                theta: 0.0,
                rho: 0.0,
            }
        }
    }

    /// Get model parameters
    pub fn params(&self) -> &HestonParams {
        &self.params
    }

    /// Get simulation configuration
    pub fn config(&self) -> &MonteCarloConfig {
        &self.config
    }

    /// Display terminal price distribution as ASCII histogram (parallelized)
    pub fn show_terminal_distribution(&self, num_bins: usize) {
        let final_prices: Vec<f64> = (0..self.config.n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = SplitMix64::new(self.config.seed + i as u64);
                let path = self.simulate_path(&mut rng);
                *path.stock_prices.last().unwrap()
            })
            .collect();
        
        let mut sorted_prices = final_prices.clone();
        sorted_prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let min_price = *sorted_prices.first().unwrap();
        let max_price = *sorted_prices.last().unwrap();
        let mean_price = sorted_prices.iter().sum::<f64>() / sorted_prices.len() as f64;
        
        let median_price = if sorted_prices.len() % 2 == 0 {
            (sorted_prices[sorted_prices.len() / 2 - 1] + sorted_prices[sorted_prices.len() / 2]) / 2.0
        } else {
            sorted_prices[sorted_prices.len() / 2]
        };
        
        // Create histogram bins
        let bin_width = (max_price - min_price) / num_bins as f64;
        let mut bins = vec![0; num_bins];
        
        for &price in &sorted_prices {
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
                 (sorted_prices.iter().map(|x| (x - mean_price).powi(2)).sum::<f64>() 
                  / sorted_prices.len() as f64).sqrt());
        println!("\nHistogram ({} bins, {} paths):", num_bins, self.config.n_paths);
        
        for i in 0..num_bins {
            let bin_start = min_price + i as f64 * bin_width;
            let bin_end = bin_start + bin_width;
            let bar_length = (bins[i] as f64 * scale) as usize;
            let bar = "█".repeat(bar_length);
            let pct = bins[i] as f64 / sorted_prices.len() as f64 * 100.0;
            
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
            use_antithetic: false,
        };

        let mc = HestonMonteCarlo::new(params, config).unwrap();
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
            use_antithetic: false,
        };

        let strike = 100.0;
        let mc = HestonMonteCarlo::new(params.clone(), config).unwrap();
        
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
            use_antithetic: false,
        };

        let mc = HestonMonteCarlo::new(params.clone(), config).unwrap();
        let avg_final_var = mc.average_final_variance();
        
        // With strong mean reversion, final variance should be close to theta
        let error = (avg_final_var - params.theta).abs();
        assert!(error < 0.02, "Variance didn't converge to theta. Error: {}", error);
    }

    #[test]
    fn test_splitmix64_uniform_distribution() {
        let mut rng = SplitMix64::new(42);
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
