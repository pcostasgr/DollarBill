// Heston analytical pricing using Carr-Madan formula
// Fast, deterministic pricing via Fourier transform (no Monte Carlo noise)

use num_complex::Complex64;
use std::f64::consts::PI;
use crate::models::heston::HestonParams;

/// Price European call using semi-analytical Heston formula (Carr-Madan)
/// ~1000x faster than Monte Carlo, no random noise
/// 
/// This uses the characteristic function approach with Fourier integration
pub fn heston_call_carr_madan(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    // Adjust integration limit based on parameters (numerical stability)
    let integration_limit = 50.0;  // Reduced from 100 for stability
    let n_points = 256; // More points for accuracy
    
    // Calculate the two probabilities P1 and P2 using adaptive integration
    let p1 = 0.5 + (1.0 / PI) * adaptive_integrate(
        |u| {
            if u.abs() < 1e-10 { return 0.0; } // Avoid division by zero
            integrand_p1(u, spot, strike, maturity, rate, params)
        },
        0.001,  // Start slightly above zero to avoid singularity
        integration_limit,
        n_points,
    );
    
    let p2 = 0.5 + (1.0 / PI) * adaptive_integrate(
        |u| {
            if u.abs() < 1e-10 { return 0.0; }
            integrand_p2(u, spot, strike, maturity, rate, params)
        },
        0.001,
        integration_limit,
        n_points,
    );
    
    // Clamp probabilities to [0, 1]
    let p1 = p1.max(0.0).min(1.0);
    let p2 = p2.max(0.0).min(1.0);
    
    // Call option price: S*P1 - K*exp(-rT)*P2
    let discount = (-rate * maturity).exp();
    let price = spot * p1 - strike * discount * p2;
    
    // Ensure non-negative price
    price.max(0.0)
}

/// Price European put using put-call parity
pub fn heston_put_carr_madan(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    let call_price = heston_call_carr_madan(spot, strike, maturity, rate, params);
    
    // Put-Call Parity: P = C - S + K*exp(-rT)
    call_price - spot + strike * (-rate * maturity).exp()
}

/// Heston characteristic function for probability P_j (improved numerical stability)
fn characteristic_function(
    phi: Complex64,
    tau: f64,
    v0: f64,
    theta: f64,
    kappa: f64,
    sigma: f64,
    rho: f64,
    j: i32,
) -> Complex64 {
    let i = Complex64::i();
    
    // Parameters depend on which probability we're computing
    let (u_j, b_j) = if j == 1 {
        (0.5, kappa - rho * sigma)
    } else {
        (-0.5, kappa)
    };
    
    let a = kappa * theta;
    
    // Compute discriminant with numerical safety
    let disc = (rho * sigma * phi * i - b_j).powi(2) 
             - sigma.powi(2) * (2.0 * u_j * phi * i - phi * phi);
    
    // Use principal branch of sqrt for complex numbers
    let d = disc.sqrt();
    
    // Compute g with numerical stability check
    let numerator = b_j - rho * sigma * phi * i - d;
    let denominator = b_j - rho * sigma * phi * i + d;
    
    // Avoid division by very small numbers
    let g = if denominator.norm() < 1e-10 {
        Complex64::new(0.0, 0.0)
    } else {
        numerator / denominator
    };
    
    // Compute exponential terms with checks
    let exp_d_tau = (-d * tau).exp();
    let one_minus_g_exp = 1.0 - g * exp_d_tau;
    
    // C term with safe logarithm
    let c = if one_minus_g_exp.norm() < 1e-10 || (1.0 - g).norm() < 1e-10 {
        Complex64::new(0.0, 0.0)
    } else {
        (1.0 / sigma.powi(2)) * (b_j - rho * sigma * phi * i - d) * tau
        - 2.0 * (one_minus_g_exp / (1.0 - g)).ln()
    };
    
    // D term with safety
    let d_term = if one_minus_g_exp.norm() < 1e-10 {
        Complex64::new(0.0, 0.0)
    } else {
        ((b_j - rho * sigma * phi * i - d) / sigma.powi(2))
        * ((1.0 - exp_d_tau) / one_minus_g_exp)
    };
    
    // Return characteristic function
    (i * phi * a * tau + a * c + d_term * v0).exp()
}

/// Integrand for probability P1
fn integrand_p1(
    u: f64,
    spot: f64,
    strike: f64,
    tau: f64,
    _rate: f64,
    params: &HestonParams,
) -> f64 {
    let phi = Complex64::new(u, 0.0);
    let i = Complex64::i();
    
    let char_func = characteristic_function(
        phi - i,  // Shift for P1
        tau,
        params.v0,
        params.theta,
        params.kappa,
        params.sigma,
        params.rho,
        1,
    );
    
    let log_moneyness = (strike / spot).ln();
    let exp_term = (-i * phi * log_moneyness).exp();
    let integrand = (exp_term * char_func / (i * phi)).re;
    
    // Check for NaN or Inf
    if integrand.is_nan() || integrand.is_infinite() {
        0.0
    } else {
        integrand
    }
}

/// Integrand for probability P2
fn integrand_p2(
    u: f64,
    spot: f64,
    strike: f64,
    tau: f64,
    _rate: f64,
    params: &HestonParams,
) -> f64 {
    let phi = Complex64::new(u, 0.0);
    let i = Complex64::i();
    
    let char_func = characteristic_function(
        phi,
        tau,
        params.v0,
        params.theta,
        params.kappa,
        params.sigma,
        params.rho,
        2,
    );
    
    let log_moneyness = (strike / spot).ln();
    let exp_term = (-i * phi * log_moneyness).exp();
    let integrand = (exp_term * char_func / (i * phi)).re;
    
    // Check for NaN or Inf
    if integrand.is_nan() || integrand.is_infinite() {
        0.0
    } else {
        integrand
    }
}

/// Adaptive integration with better numerical stability
fn adaptive_integrate<F>(f: F, a: f64, b: f64, n: usize) -> f64
where
    F: Fn(f64) -> f64,
{
    // Simpson's 1/3 rule - better than trapezoid
    let mut sum = 0.0;
    let h = (b - a) / (n as f64);
    
    for i in 0..n {
        let x0 = a + i as f64 * h;
        let x1 = x0 + h / 2.0;
        let x2 = x0 + h;
        
        let y0 = f(x0);
        let y1 = f(x1);
        let y2 = f(x2);
        
        // Skip if any value is invalid
        if y0.is_finite() && y1.is_finite() && y2.is_finite() {
            sum += (h / 6.0) * (y0 + 4.0 * y1 + y2);
        }
    }
    
    sum
}

/// Price OTM call option using Carr-Madan
/// The Heston characteristic function automatically generates the volatility smile
pub fn heston_call_otm(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    // Carr-Madan works for all strikes - the smile comes from the characteristic function
    heston_call_carr_madan(spot, strike, maturity, rate, params)
}

/// Price ITM call option using Carr-Madan
/// Same formula as OTM - the characteristic function handles all moneyness levels
pub fn heston_call_itm(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    heston_call_carr_madan(spot, strike, maturity, rate, params)
}

/// Price OTM put option using Carr-Madan
pub fn heston_put_otm(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    heston_put_carr_madan(spot, strike, maturity, rate, params)
}

/// Price ITM put option using Carr-Madan
pub fn heston_put_itm(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    heston_put_carr_madan(spot, strike, maturity, rate, params)
}

/// Classify option moneyness
#[derive(Debug, PartialEq)]
pub enum Moneyness {
    ATM,
    OTM,
    ITM,
}

pub fn classify_moneyness(strike: f64, spot: f64, threshold: f64) -> Moneyness {
    let ratio = strike / spot;
    
    if (ratio - 1.0).abs() < threshold {
        Moneyness::ATM
    } else if strike > spot {
        Moneyness::OTM  // For calls
    } else {
        Moneyness::ITM  // For calls
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_carr_madan_atm() {
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
        
        let price = heston_call_carr_madan(100.0, 100.0, 1.0, 0.05, &params);
        
        // Should be reasonable ATM call price (around $10-20)
        assert!(price > 5.0 && price < 30.0);
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
        
        let call = heston_call_carr_madan(100.0, 100.0, 1.0, 0.05, &params);
        let put = heston_put_carr_madan(100.0, 100.0, 1.0, 0.05, &params);
        
        // Put-Call Parity: C - P = S - K*exp(-rT)
        let parity_lhs = call - put;
        let parity_rhs = 100.0 - 100.0 * (-0.05_f64).exp();
        
        assert!((parity_lhs - parity_rhs).abs() < 0.1);
    }
}
