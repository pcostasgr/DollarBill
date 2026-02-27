// American options pricing using binomial trees (Cox-Ross-Rubinstein model)
// Handles early exercise optimally for American calls and puts

use crate::models::bs_mod::Greeks;

/// American option exercise style
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExerciseStyle {
    European,  // Cannot exercise early
    American,  // Can exercise early
}

/// Binomial tree configuration
#[derive(Debug, Clone)]
pub struct BinomialConfig {
    pub n_steps: usize,       // Number of time steps
    pub use_dividends: bool,  // Whether to include dividends
    pub dividend_yield: f64,  // Continuous annual dividend yield (0.0 = none)
}

impl Default for BinomialConfig {
    fn default() -> Self {
        Self {
            n_steps: 100,
            use_dividends: false,
            dividend_yield: 0.0,
        }
    }
}

/// Price American call option using binomial tree
pub fn american_call_binomial(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
) -> f64 {
    binomial_tree(spot, strike, maturity, rate, volatility, config, true)
}

/// Price American put option using binomial tree
pub fn american_put_binomial(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
) -> f64 {
    binomial_tree(spot, strike, maturity, rate, volatility, config, false)
}

/// Price European call option using binomial tree (for comparison/validation)
pub fn european_call_binomial(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
) -> f64 {
    binomial_tree_european(spot, strike, maturity, rate, volatility, config, true)
}

/// Price European put option using binomial tree (for comparison/validation)
pub fn european_put_binomial(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
) -> f64 {
    binomial_tree_european(spot, strike, maturity, rate, volatility, config, false)
}

/// Core binomial tree implementation for American options
fn binomial_tree(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
    is_call: bool,
) -> f64 {
    let n = config.n_steps;
    let dt = maturity / n as f64;
    let u = (volatility * dt.sqrt()).exp();  // Up factor
    let d = 1.0 / u;                         // Down factor
    let disc = (rate * dt).exp();            // Discount factor per step
    // Continuous dividend yield shifts risk-neutral drift: forward = S*exp((r-q)*dt)
    let q = if config.use_dividends { config.dividend_yield } else { 0.0 };
    let fwd_factor = ((rate - q) * dt).exp();
    let p = (fwd_factor - d) / (u - d);     // Risk-neutral up probability
    let r = disc;                            // alias kept for backwards-compat

    // Validate parameters
    if !p.is_finite() || p < 0.0 || p > 1.0 {
        return 0.0;  // Invalid parameters
    }

    // Build the binomial tree backwards
    let mut option_values = vec![0.0; n + 1];

    // Terminal node values (at maturity)
    for i in 0..=n {
        let stock_price = spot * u.powi(i as i32) * d.powi((n - i) as i32);
        option_values[i] = if is_call {
            (stock_price - strike).max(0.0)
        } else {
            (strike - stock_price).max(0.0)
        };
    }

    // Work backwards through the tree
    for step in (0..n).rev() {
        for i in 0..=step {
            let stock_price = spot * u.powi(i as i32) * d.powi((step - i) as i32);

            // Risk-neutral expectation
            let expected_value = (p * option_values[i + 1] + (1.0 - p) * option_values[i]) / r;

            // Early exercise value
            let exercise_value = if is_call {
                (stock_price - strike).max(0.0)
            } else {
                (strike - stock_price).max(0.0)
            };

            // For American options, take maximum of exercise and continuation
            option_values[i] = expected_value.max(exercise_value);
        }
    }

    option_values[0]
}

/// Binomial tree for European options (no early exercise)
fn binomial_tree_european(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
    is_call: bool,
) -> f64 {
    let n = config.n_steps;
    let dt = maturity / n as f64;
    let u = (volatility * dt.sqrt()).exp();
    let d = 1.0 / u;
    let disc = (rate * dt).exp();
    let q = if config.use_dividends { config.dividend_yield } else { 0.0 };
    let fwd_factor = ((rate - q) * dt).exp();
    let p = (fwd_factor - d) / (u - d);
    let r = disc;

    if !p.is_finite() || p < 0.0 || p > 1.0 {
        return 0.0;
    }

    // Build the binomial tree backwards (European - no early exercise)
    let mut option_values = vec![0.0; n + 1];

    // Terminal node values
    for i in 0..=n {
        let stock_price = spot * u.powi(i as i32) * d.powi((n - i) as i32);
        option_values[i] = if is_call {
            (stock_price - strike).max(0.0)
        } else {
            (strike - stock_price).max(0.0)
        };
    }

    // Work backwards (European - only risk-neutral expectation)
    for step in (0..n).rev() {
        for i in 0..=step {
            option_values[i] = (p * option_values[i + 1] + (1.0 - p) * option_values[i]) / r;
        }
    }

    option_values[0]
}

/// Calculate Greeks for American options using finite differences
pub fn american_call_greeks(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
) -> Greeks {
    let eps = 1e-4;  // Small perturbation for finite differences

    // Base price
    let price = american_call_binomial(spot, strike, maturity, rate, volatility, config);

    // Delta: dPrice/dSpot
    let price_up = american_call_binomial(spot * (1.0 + eps), strike, maturity, rate, volatility, config);
    let price_down = american_call_binomial(spot * (1.0 - eps), strike, maturity, rate, volatility, config);
    let delta = (price_up - price_down) / (2.0 * spot * eps);

    // Gamma: d²Price/dSpot²
    let gamma = (price_up - 2.0 * price + price_down) / (spot * spot * eps * eps);

    // Theta: -dPrice/dTime
    let price_theta = american_call_binomial(spot, strike, maturity * (1.0 - eps), rate, volatility, config);
    let theta = -(price_theta - price) / (maturity * eps);

    // Vega: dPrice/dVol
    let price_vega = american_call_binomial(spot, strike, maturity, rate, volatility * (1.0 + eps), config);
    let vega = (price_vega - price) / (volatility * eps);

    // Rho: dPrice/dRate
    let price_rho = american_call_binomial(spot, strike, maturity, rate + eps, volatility, config);
    let rho = (price_rho - price) / eps;

    Greeks {
        price,
        delta,
        gamma,
        theta,
        vega,
        rho,
    }
}

/// Calculate Greeks for American puts
pub fn american_put_greeks(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
) -> Greeks {
    let eps = 1e-4;

    let price = american_put_binomial(spot, strike, maturity, rate, volatility, config);

    let price_up = american_put_binomial(spot * (1.0 + eps), strike, maturity, rate, volatility, config);
    let price_down = american_put_binomial(spot * (1.0 - eps), strike, maturity, rate, volatility, config);
    let delta = (price_up - price_down) / (2.0 * spot * eps);

    let gamma = (price_up - 2.0 * price + price_down) / (spot * spot * eps * eps);

    let price_theta = american_put_binomial(spot, strike, maturity * (1.0 - eps), rate, volatility, config);
    let theta = -(price_theta - price) / (maturity * eps);

    let price_vega = american_put_binomial(spot, strike, maturity, rate, volatility * (1.0 + eps), config);
    let vega = (price_vega - price) / (volatility * eps);

    let price_rho = american_put_binomial(spot, strike, maturity, rate + eps, volatility, config);
    let rho = (price_rho - price) / eps;

    Greeks {
        price,
        delta,
        gamma,
        theta,
        vega,
        rho,
    }
}

/// Optimal exercise boundary for American options
/// Returns the stock price at which early exercise becomes optimal
pub fn optimal_exercise_boundary(
    strike: f64,
    maturity: f64,
    rate: f64,
    volatility: f64,
    config: &BinomialConfig,
    is_call: bool,
) -> Vec<f64> {
    let n = config.n_steps;
    let dt = maturity / n as f64;
    let u = (volatility * dt.sqrt()).exp();
    let d = 1.0 / u;
    let r = (rate * dt).exp();
    let p = (r - d) / (u - d);

    let mut boundaries = Vec::with_capacity(n);

    // Build the tree and track exercise boundaries
    let mut option_values = vec![0.0; n + 1];
    let mut exercise_points = vec![false; n + 1];

    // Terminal values
    for i in 0..=n {
        let stock_price = strike * u.powi(i as i32) * d.powi((n - i) as i32);
        option_values[i] = if is_call {
            (stock_price - strike).max(0.0)
        } else {
            (strike - stock_price).max(0.0)
        };
    }

    // Work backwards, tracking where early exercise occurs
    for step in (0..n).rev() {
        for i in 0..=step {
            let stock_price = strike * u.powi(i as i32) * d.powi((step - i) as i32);
            let expected_value = (p * option_values[i + 1] + (1.0 - p) * option_values[i]) / r;

            let exercise_value = if is_call {
                (stock_price - strike).max(0.0)
            } else {
                (strike - stock_price).max(0.0)
            };

            if exercise_value > expected_value {
                exercise_points[i] = true;
                option_values[i] = exercise_value;
            } else {
                exercise_points[i] = false;
                option_values[i] = expected_value;
            }
        }

        // Record the exercise boundary at this time step
        let mut boundary_price = 0.0_f64;
        for i in 0..=step {
            if exercise_points[i] {
                let stock_price = strike * u.powi(i as i32) * d.powi((step - i) as i32);
                boundary_price = boundary_price.max(stock_price);
            }
        }
        boundaries.push(boundary_price);
    }

    boundaries.reverse();
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_american_call_atm() {
        let config = BinomialConfig::default();
        let price = american_call_binomial(100.0, 100.0, 1.0, 0.05, 0.2, &config);

        // ATM American call should be worth more than European call due to early exercise
        let euro_price = european_call_binomial(100.0, 100.0, 1.0, 0.05, 0.2, &config);

        assert!(price.is_finite());
        assert!(price > 0.0);
        assert!(price >= euro_price);  // American >= European
    }

    #[test]
    fn test_american_put_itm() {
        let config = BinomialConfig::default();
        let price = american_put_binomial(90.0, 100.0, 1.0, 0.05, 0.2, &config);

        // ITM American put should be worth approximately strike - spot
        assert!(price.is_finite());
        assert!(price > 8.0);  // Should be close to intrinsic value
        assert!(price < 12.0);
    }

    #[test]
    fn test_put_call_parity_approximate() {
        let config = BinomialConfig::default();
        let call = american_call_binomial(100.0, 100.0, 1.0, 0.05, 0.2, &config);
        let put = american_put_binomial(100.0, 100.0, 1.0, 0.05, 0.2, &config);

        // Put-call parity: C - P ≈ S - K*e^(-rT)
        let parity_lhs = call - put;
        let parity_rhs = 100.0 - 100.0 * (-0.05_f64).exp();

        // Allow some tolerance due to early exercise premium
        assert!((parity_lhs - parity_rhs).abs() < 2.0);
    }

    #[test]
    fn test_convergence_with_steps() {
        let spot = 100.0;
        let strike = 100.0;
        let maturity = 1.0;
        let rate = 0.05;
        let vol = 0.2;

        let config_50 = BinomialConfig { n_steps: 50, use_dividends: false, dividend_yield: 0.0 };
        let config_100 = BinomialConfig { n_steps: 100, use_dividends: false, dividend_yield: 0.0 };
        let config_200 = BinomialConfig { n_steps: 200, use_dividends: false, dividend_yield: 0.0 };

        let price_50 = american_call_binomial(spot, strike, maturity, rate, vol, &config_50);
        let price_100 = american_call_binomial(spot, strike, maturity, rate, vol, &config_100);
        let price_200 = american_call_binomial(spot, strike, maturity, rate, vol, &config_200);

        // Should converge as number of steps increases
        assert!((price_100 - price_50).abs() < (price_200 - price_100).abs() * 2.0);
    }

    #[test]
    fn test_greeks_calculation() {
        let config = BinomialConfig::default();
        let greeks = american_call_greeks(100.0, 100.0, 1.0, 0.05, 0.2, &config);

        assert!(greeks.price > 0.0);
        assert!(greeks.delta >= 0.0 && greeks.delta <= 1.0);
        assert!(greeks.gamma >= 0.0);
        assert!(greeks.vega > 0.0);
        assert!(greeks.rho > 0.0);
        // Theta can be positive or negative for American options
        assert!(greeks.theta.is_finite());
    }
}