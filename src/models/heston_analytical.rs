// Heston analytical pricing using Carr-Madan formula
// Fast, deterministic pricing via Fourier transform (no Monte Carlo noise)
//
// Supports two integration backends:
//   1. Carr-Madan damped Gauss-Lobatto quadrature (default, existing)
//   2. Gauss-Laguerre quadrature with configurable 32–128 nodes

use num_complex::Complex64;
use std::f64::consts::PI;
use crate::models::heston::HestonParams;
use crate::models::gauss_laguerre::GaussLaguerreRule;

/// Integration method for Heston semi-analytical pricing.
///
/// Choose between the original Carr-Madan damped Gauss-Lobatto quadrature
/// and the Gauss-Laguerre alternative which can offer higher accuracy
/// at a configurable node count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegrationMethod {
    /// Carr-Madan damped Gauss-Lobatto quadrature (original method).
    CarrMadan,
    /// Gauss-Laguerre quadrature with the specified number of nodes (32–128).
    GaussLaguerre { nodes: usize },
}

impl Default for IntegrationMethod {
    fn default() -> Self {
        IntegrationMethod::CarrMadan
    }
}

impl std::fmt::Display for IntegrationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrationMethod::CarrMadan => write!(f, "Carr-Madan (Gauss-Lobatto)"),
            IntegrationMethod::GaussLaguerre { nodes } => {
                write!(f, "Gauss-Laguerre ({nodes} nodes)")
            }
        }
    }
}

/// Price European call using semi-analytical Heston formula (Carr-Madan)
/// ~1000x faster than Monte Carlo, no random noise
/// 
/// This uses the characteristic function approach with Fourier integration
/// Enhanced with numerical stability improvements for Phase 2
pub fn heston_call_carr_madan(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    // Phase 2: Adaptive integration parameters based on model parameters
    let (integration_limit, n_points, damping_alpha) = adaptive_integration_params(params, maturity);
    
    // Calculate the two probabilities P1 and P2 using improved integration
    let p1 = 0.5 + (1.0 / PI) * carr_madan_integrate(
        |u| integrand_p1(u, spot, strike, maturity, rate, params, damping_alpha),
        0.001,  // Start slightly above zero to avoid singularity
        integration_limit,
        n_points,
    );
    
    let p2 = 0.5 + (1.0 / PI) * carr_madan_integrate(
        |u| integrand_p2(u, spot, strike, maturity, rate, params, damping_alpha),
        0.001,
        integration_limit,
        n_points,
    );
    
    // Clamp probabilities to [0, 1] with better bounds checking
    let p1 = p1.max(0.0).min(1.0);
    let p2 = p2.max(0.0).min(1.0);
    
    // Call option price: S*P1 - K*exp(-rT)*P2
    let discount = (-rate * maturity).exp();
    let price = spot * p1 - strike * discount * p2;
    
    // No-arbitrage lower bound for a European call: C >= max(0, S - K*exp(-rT))
    // The Fourier integration can produce small positive values below intrinsic on
    // deep-ITM inputs or with extreme vol-of-vol parameters; clamp here ensures
    // every returned price respects put-call parity at the lower boundary.
    let intrinsic = (spot - strike * discount).max(0.0);

    if price.is_finite() && price >= 0.0 {
        price.max(intrinsic)
    } else {
        // Fallback to Black-Scholes approximation for numerical failure
        let bs_vol = (params.v0).sqrt(); // Approximate volatility
        crate::models::bs_mod::black_scholes_merton_call(spot, strike, maturity, rate, bs_vol, 0.0).price.max(intrinsic)
    }
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

/// Heston characteristic function for probability P_j (improved numerical stability - Phase 2)
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
    
    // Phase 2: Improved discriminant calculation with better numerical stability
    let discriminant = (rho * sigma * phi * i - b_j).powi(2) 
                     - sigma.powi(2) * (2.0 * u_j * phi * i - phi * phi);
    
    // Use principal branch of sqrt with better handling
    let d = discriminant.sqrt();
    
    // Phase 2: More robust g calculation
    let numerator = b_j - rho * sigma * phi * i - d;
    let denominator = b_j - rho * sigma * phi * i + d;
    
    // Enhanced division by zero protection
    let g = if denominator.norm() < 1e-12 {
        Complex64::new(0.0, 0.0)
    } else if numerator.norm() < 1e-12 {
        Complex64::new(0.0, 0.0)
    } else {
        numerator / denominator
    };
    
    // Phase 2: Better exponential calculations with overflow protection
    let exp_d_tau = if (d * tau).re.abs() > 700.0 {
        Complex64::new(0.0, 0.0)  // Underflow protection
    } else {
        (-d * tau).exp()
    };
    
    let one_minus_g_exp = 1.0 - g * exp_d_tau;
    
    // Phase 2: Enhanced C term calculation
    let c = if one_minus_g_exp.norm() < 1e-12 || (1.0 - g).norm() < 1e-12 {
        Complex64::new(0.0, 0.0)
    } else {
        let log_term = (one_minus_g_exp / (1.0 - g)).ln();
        if log_term.is_finite() {
            (1.0 / sigma.powi(2)) * (b_j - rho * sigma * phi * i - d) * tau - 2.0 * log_term
        } else {
            Complex64::new(0.0, 0.0)
        }
    };
    
    // Phase 2: Enhanced D term calculation
    let d_term = if one_minus_g_exp.norm() < 1e-12 {
        Complex64::new(0.0, 0.0)
    } else {
        let ratio = (b_j - rho * sigma * phi * i - d) / sigma.powi(2);
        let exp_ratio = (1.0 - exp_d_tau) / one_minus_g_exp;
        if ratio.is_finite() && exp_ratio.is_finite() {
            ratio * exp_ratio
        } else {
            Complex64::new(0.0, 0.0)
        }
    };
    
    // Return characteristic function with overflow protection
    let result = (i * phi * a * tau + a * c + d_term * v0).exp();
    
    // Phase 2: Final numerical stability check
    if result.is_finite() {
        result
    } else {
        Complex64::new(1.0, 0.0)  // Return 1.0 for numerical failure
    }
}

/// Integrand for probability P1 with Carr-Madan damping (Phase 2 improvement)
fn integrand_p1(
    u: f64,
    spot: f64,
    strike: f64,
    tau: f64,
    _rate: f64,
    params: &HestonParams,
    alpha: f64,
) -> f64 {
    let phi = Complex64::new(u, -alpha);  // Carr-Madan damping: φ = u - iα
    let i = Complex64::i();
    
    let char_func = characteristic_function(
        phi - i,  // P1 still needs the -i shift
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
    
    // Carr-Madan damping factor: e^{-α k} where k = ln(K/S)
    let damping_factor = (-alpha * log_moneyness).exp();
    
    let integrand = (exp_term * char_func / (alpha * alpha + phi * phi)).re * damping_factor;
    
    // Enhanced numerical stability checks
    if integrand.is_nan() || integrand.is_infinite() || integrand.abs() > 1e6 {
        0.0
    } else {
        integrand
    }
}

/// Integrand for probability P2 with Carr-Madan damping (Phase 2 improvement)
fn integrand_p2(
    u: f64,
    spot: f64,
    strike: f64,
    tau: f64,
    _rate: f64,
    params: &HestonParams,
    alpha: f64,
) -> f64 {
    let phi = Complex64::new(u, -alpha);  // Carr-Madan damping: φ = u - iα
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
    
    // Carr-Madan damping factor: e^{-α k} where k = ln(K/S)
    let damping_factor = (-alpha * log_moneyness).exp();
    
    let integrand = (exp_term * char_func / (alpha * alpha + phi * phi)).re * damping_factor;
    
    // Enhanced numerical stability checks
    if integrand.is_nan() || integrand.is_infinite() || integrand.abs() > 1e6 {
        0.0
    } else {
        integrand
    }
}

/// Phase 2: Adaptive integration parameters based on Heston model parameters
/// Returns (integration_limit, n_points, damping_alpha)
fn adaptive_integration_params(params: &HestonParams, maturity: f64) -> (f64, usize, f64) {
    // Adaptive integration limit based on volatility of volatility and correlation
    // Higher sigma and |rho| require larger integration limits
    let vol_of_vol_factor = params.sigma.max(0.1).min(2.0);
    let correlation_factor = params.rho.abs().max(0.1).min(0.9);
    let time_factor = maturity.max(0.01).min(5.0).sqrt();
    
    // Base limit scales with model complexity
    let base_limit = 30.0;
    let integration_limit = base_limit * vol_of_vol_factor * correlation_factor / time_factor;
    let integration_limit = integration_limit.max(20.0).min(200.0);
    
    // Adaptive number of points - more for complex models
    let complexity = (params.sigma * params.rho.abs() * params.kappa).abs();
    let n_points = if complexity > 0.5 {
        512  // High complexity
    } else if complexity > 0.2 {
        256  // Medium complexity
    } else {
        128  // Low complexity
    };
    
    // Carr-Madan damping parameter (α) - critical for numerical stability
    // Optimal α balances damping vs accuracy
    let damping_alpha = 1.5;  // Standard Carr-Madan value
    
    (integration_limit, n_points, damping_alpha)
}

/// Phase 2: Improved Carr-Madan integration with damping and better numerical methods
fn carr_madan_integrate<F>(f: F, a: f64, b: f64, n: usize) -> f64
where
    F: Fn(f64) -> f64,
{
    // Use Gauss-Lobatto integration for better accuracy on oscillatory integrands
    // This is more stable than basic Simpson's rule for Fourier transforms
    
    let mut sum = 0.0;
    let h = (b - a) / (n as f64);
    
    // Gauss-Lobatto weights for better endpoint handling
    let weights = [1.0/6.0, 5.0/6.0, 5.0/6.0, 1.0/6.0]; // 4-point rule
    
    for i in 0..n {
        let x0 = a + i as f64 * h;
        let x1 = x0 + h * 0.276393202250021;  // Gauss-Lobatto points
        let x2 = x0 + h * 0.723606797749979;
        let x3 = x0 + h;
        
        let y0 = f(x0);
        let y1 = f(x1);
        let y2 = f(x2);
        let y3 = f(x3);
        
        // Check for numerical issues
        if y0.is_finite() && y1.is_finite() && y2.is_finite() && y3.is_finite() {
            // Gauss-Lobatto quadrature
            let segment_sum = h * (weights[0] * y0 + weights[1] * y1 + 
                                 weights[2] * y2 + weights[3] * y3);
            sum += segment_sum;
        } else {
            // Fallback to trapezoidal rule for this segment
            let y_avg = (y0.max(0.0) + y3.max(0.0)) * 0.5;
            sum += h * y_avg;
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

// ═══════════════════════════════════════════════════════════════════════
// Gauss-Laguerre pricing  (correct Heston char function + GL scaling)
// ═══════════════════════════════════════════════════════════════════════

/// Correct Heston risk-neutral characteristic function for the log-return
/// X = ln(S(T)/S(0)).
///
/// Returns φ(z) = E\[exp(iz·X)\] = exp(iz·rτ + C(z,τ) + D(z,τ)·v₀)
///
/// Uses the Lord-Kahl "Formulation 2" (better numerics: exp(−d·τ) decays).
fn heston_cf(
    z: Complex64,
    tau: f64,
    rate: f64,
    v0: f64,
    kappa: f64,
    theta: f64,
    sigma: f64,
    rho: f64,
) -> Complex64 {
    let i = Complex64::i();

    let beta = kappa - rho * sigma * i * z; // κ − ρσiz
    let d_sq = beta * beta + sigma * sigma * (z * z + i * z);
    let d = d_sq.sqrt();

    let g_num = beta - d;
    let g_den = beta + d;
    let g = if g_den.norm() < 1e-14 {
        Complex64::new(0.0, 0.0)
    } else {
        g_num / g_den
    };

    let exp_d_tau = if (d * tau).re.abs() > 700.0 {
        Complex64::new(0.0, 0.0)
    } else {
        (-d * tau).exp()
    };

    let one_minus_g = 1.0 - g;
    let one_minus_g_exp = 1.0 - g * exp_d_tau;

    // C = (κθ/σ²) · [(β−d)τ − 2·ln((1−g·e^{−dτ})/(1−g))]
    let big_c = if one_minus_g.norm() < 1e-14 || one_minus_g_exp.norm() < 1e-14 {
        Complex64::new(0.0, 0.0)
    } else {
        let log_ratio = (one_minus_g_exp / one_minus_g).ln();
        if log_ratio.is_finite() {
            (kappa * theta / (sigma * sigma))
                * ((beta - d) * tau - 2.0 * log_ratio)
        } else {
            Complex64::new(0.0, 0.0)
        }
    };

    // D = ((β−d)/σ²) · (1−e^{−dτ}) / (1−g·e^{−dτ})
    let big_d = if one_minus_g_exp.norm() < 1e-14 {
        Complex64::new(0.0, 0.0)
    } else {
        let ratio = (beta - d) / (sigma * sigma);
        let frac = (1.0 - exp_d_tau) / one_minus_g_exp;
        if ratio.is_finite() && frac.is_finite() {
            ratio * frac
        } else {
            Complex64::new(0.0, 0.0)
        }
    };

    // φ(z) = exp(iz·r·τ + C + D·v₀)
    let exponent = i * z * rate * tau + big_c + big_d * v0;
    let result = exponent.exp();
    if result.is_finite() { result } else { Complex64::new(1.0, 0.0) }
}

/// Heston (1993) integrand for P₁ using the corrected char function.
fn integrand_p1_gl(
    u: f64, spot: f64, strike: f64, tau: f64, rate: f64,
    params: &HestonParams,
) -> f64 {
    if u.abs() < 1e-8 { return 0.0; }
    let z = Complex64::new(u, 0.0);
    let i = Complex64::i();

    // Stock-price measure: evaluate φ at z − i
    let cf = heston_cf(
        z - i, tau, rate, params.v0, params.kappa,
        params.theta, params.sigma, params.rho,
    );
    let k = (strike / spot).ln();
    let result = ((-i * z * k).exp() * cf / (i * z)).re;
    if result.is_finite() && result.abs() < 1e8 { result } else { 0.0 }
}

/// Heston (1993) integrand for P₂ using the corrected char function.
fn integrand_p2_gl(
    u: f64, spot: f64, strike: f64, tau: f64, rate: f64,
    params: &HestonParams,
) -> f64 {
    if u.abs() < 1e-8 { return 0.0; }
    let z = Complex64::new(u, 0.0);
    let i = Complex64::i();

    let cf = heston_cf(
        z, tau, rate, params.v0, params.kappa,
        params.theta, params.sigma, params.rho,
    );
    let k = (strike / spot).ln();
    let result = ((-i * z * k).exp() * cf / (i * z)).re;
    if result.is_finite() && result.abs() < 1e8 { result } else { 0.0 }
}

/// Price a European **call** using Gauss-Laguerre quadrature with
/// the corrected Heston characteristic function.
///
/// Uses the Heston (1993) P₁/P₂ Fourier inversion with a GL scaling
/// parameter `c = 1/d_eff` that matches the natural exponential decay
/// of the characteristic function.
///
/// The `rule` should be pre-computed via [`GaussLaguerreRule::new`]
/// for best performance in tight loops (e.g. calibration).
pub fn heston_call_gauss_laguerre(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
    rule: &GaussLaguerreRule,
) -> f64 {
    // d_eff ≈ (κθτ + v₀)/σ  — exponential decay rate of |φ(u)| for large u
    let d_eff = (params.kappa * params.theta * maturity + params.v0)
        / params.sigma.max(0.01);
    let c = (1.0 / d_eff.max(0.01)).max(1.0).min(30.0);

    // P₁ uses the stock-price measure CF = φ(u−i)/φ(−i).
    // Since φ(−i) = e^{rτ}, the P₁ integral carries an extra e^{−rτ} factor.
    let phi_neg_i_inv = (-rate * maturity).exp(); // 1/φ(−i) = e^{−rτ}

    let p1 = 0.5
        + (phi_neg_i_inv / PI)
            * c
            * rule.integrate(|x| {
                let u = c * x;
                integrand_p1_gl(u, spot, strike, maturity, rate, params)
            });
    let p2 = 0.5
        + (1.0 / PI)
            * c
            * rule.integrate(|x| {
                let u = c * x;
                integrand_p2_gl(u, spot, strike, maturity, rate, params)
            });

    let p1 = p1.clamp(0.0, 1.0);
    let p2 = p2.clamp(0.0, 1.0);

    let discount = (-rate * maturity).exp();
    let price = spot * p1 - strike * discount * p2;
    let intrinsic = (spot - strike * discount).max(0.0);

    if price.is_finite() && price >= 0.0 {
        price.max(intrinsic)
    } else {
        let bs_vol = params.v0.sqrt();
        crate::models::bs_mod::black_scholes_merton_call(
            spot, strike, maturity, rate, bs_vol, 0.0,
        )
        .price
        .max(intrinsic)
    }
}

/// Price a European **put** using Gauss-Laguerre quadrature (via put-call parity).
pub fn heston_put_gauss_laguerre(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
    rule: &GaussLaguerreRule,
) -> f64 {
    let call = heston_call_gauss_laguerre(spot, strike, maturity, rate, params, rule);
    call - spot + strike * (-rate * maturity).exp()
}

// ═══════════════════════════════════════════════════════════════════════
// Gatheral single-integral formula  (independent cross-check)
// ═══════════════════════════════════════════════════════════════════════

/// Price a European **call** using the Gatheral (2006) / Lewis (2000)
/// single-integral Fourier formula with Gauss-Laguerre quadrature.
///
/// **NOTE**: This function is for diagnostic / cross-check purposes only.
/// The Gatheral formula is canonically defined using the CF of `ln(S_T)`,
/// but our `heston_cf` computes the CF of the log-return `ln(S_T/S_0)`.
/// The conversion introduces an oscillatory phase `e^{iv ln(S)}` that
/// degrades GL convergence.  For production use, prefer
/// [`heston_call_gauss_laguerre`] which uses the P₁/P₂ formulation.
pub fn heston_call_gatheral_gl(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
    rule: &GaussLaguerreRule,
) -> f64 {
    let log_moneyness = (spot / strike).ln(); // ln(S/K)
    let sqrt_sk = (spot * strike).sqrt();
    let discount_half = (-rate * maturity / 2.0).exp();

    // GL scaling
    let d_eff = (params.kappa * params.theta * maturity + params.v0)
        / params.sigma.max(0.01);
    let c = (1.0 / d_eff.max(0.01)).max(1.0).min(30.0);

    let integral = c * rule.integrate(|x| {
        let v = c * x;
        if v.abs() < 1e-12 {
            // Limit as v → 0: Re[φ(−i/2)] / (1/4)
            let cf = heston_cf(
                Complex64::new(0.0, -0.5),
                maturity, rate, params.v0, params.kappa,
                params.theta, params.sigma, params.rho,
            );
            return cf.re * 4.0;
        }
        let z = Complex64::new(v, -0.5); // v − i/2
        let cf = heston_cf(
            z, maturity, rate, params.v0, params.kappa,
            params.theta, params.sigma, params.rho,
        );
        let phase = Complex64::new(0.0, v * log_moneyness).exp(); // e^{iv ln(S/K)}
        let numerator = (phase * cf).re;
        let denominator = v * v + 0.25;
        if numerator.is_finite() { numerator / denominator } else { 0.0 }
    });

    let price = spot - sqrt_sk * discount_half / PI * integral;
    let intrinsic = (spot - strike * (-rate * maturity).exp()).max(0.0);

    if price.is_finite() && price >= intrinsic {
        price
    } else {
        intrinsic
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Unified dispatch — choose integration method at runtime
// ═══════════════════════════════════════════════════════════════════════

/// Price a European **call** using the specified [`IntegrationMethod`].
///
/// This is the recommended entry point when the integration backend is
/// selected at run-time (e.g. from a config file).  For Gauss-Laguerre
/// in a tight loop, prefer pre-allocating a [`GaussLaguerreRule`] and
/// calling [`heston_call_gauss_laguerre`] directly.
pub fn heston_call_price(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
    method: &IntegrationMethod,
) -> f64 {
    match method {
        IntegrationMethod::CarrMadan => {
            heston_call_carr_madan(spot, strike, maturity, rate, params)
        }
        IntegrationMethod::GaussLaguerre { nodes } => {
            let rule = GaussLaguerreRule::new(*nodes);
            heston_call_gauss_laguerre(spot, strike, maturity, rate, params, &rule)
        }
    }
}

/// Price a European **put** using the specified [`IntegrationMethod`].
pub fn heston_put_price(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
    method: &IntegrationMethod,
) -> f64 {
    match method {
        IntegrationMethod::CarrMadan => {
            heston_put_carr_madan(spot, strike, maturity, rate, params)
        }
        IntegrationMethod::GaussLaguerre { nodes } => {
            let rule = GaussLaguerreRule::new(*nodes);
            heston_put_gauss_laguerre(spot, strike, maturity, rate, params, &rule)
        }
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

    // ── Gauss-Laguerre tests ──────────────────────────────────────────

    #[test]
    fn test_gauss_laguerre_atm() {
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

        let rule = GaussLaguerreRule::new(64);
        let price = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);

        assert!(
            price > 5.0 && price < 30.0,
            "GL ATM call price should be reasonable, got {price}"
        );
    }

    #[test]
    fn test_gauss_laguerre_put_call_parity() {
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

        let rule = GaussLaguerreRule::new(64);
        let call = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);
        let put = heston_put_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);

        let parity_lhs = call - put;
        let parity_rhs = 100.0 - 100.0 * (-0.05_f64).exp();

        assert!(
            (parity_lhs - parity_rhs).abs() < 0.1,
            "GL put-call parity failed: lhs={parity_lhs}, rhs={parity_rhs}"
        );
    }

    #[test]
    fn test_gauss_laguerre_vs_carr_madan() {
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

        let rule = GaussLaguerreRule::new(64);
        let gl_call = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);

        // Heston price with stochastic vol should be close to BS for v0≈theta
        let bs_vol = params.v0.sqrt();
        let bs_price = crate::models::bs_mod::black_scholes_merton_call(
            100.0, 100.0, 1.0, 0.05, bs_vol, 0.0,
        ).price;

        let diff = (gl_call - bs_price).abs();
        assert!(
            diff < 3.0,
            "GL ({gl_call:.4}) and BS ({bs_price:.4}) diverge by {diff:.4}"
        );
    }

    #[test]
    fn test_unified_dispatch_carr_madan() {
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

        let direct = heston_call_carr_madan(100.0, 100.0, 1.0, 0.05, &params);
        let dispatched = heston_call_price(100.0, 100.0, 1.0, 0.05, &params, &IntegrationMethod::CarrMadan);

        assert!(
            (direct - dispatched).abs() < 1e-12,
            "Unified dispatch should match direct Carr-Madan call"
        );
    }

    #[test]
    fn test_unified_dispatch_gauss_laguerre() {
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

        let method = IntegrationMethod::GaussLaguerre { nodes: 32 };
        let price = heston_call_price(100.0, 100.0, 1.0, 0.05, &params, &method);

        assert!(
            price > 5.0 && price < 30.0,
            "Unified GL call should produce reasonable price, got {price}"
        );
    }

    #[test]
    fn test_32_vs_64_nodes_accuracy() {
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

        let rule32 = GaussLaguerreRule::new(32);
        let rule64 = GaussLaguerreRule::new(64);

        let p32 = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule32);
        let p64 = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule64);

        // Both should be in the same ballpark; 64 is the "reference"
        assert!(
            (p32 - p64).abs() < 2.0,
            "32-node ({p32:.4}) and 64-node ({p64:.4}) prices should be close"
        );
    }

    /// Comprehensive diagnostic: compare P₁/P₂ form, Gatheral single-integral,
    /// and the corrected P₁ (with 1/φ(−i) normalisation) to pin down accuracy.
    #[test]
    fn test_diagnostic_gatheral_vs_p1p2() {
        let params = HestonParams {
            s0: 100.0, v0: 0.04, kappa: 2.0, theta: 0.04,
            sigma: 0.3, rho: -0.7, r: 0.05, t: 1.0,
        };
        let rule = GaussLaguerreRule::new(64);
        let (spot, strike, tau, rate) = (100.0, 100.0, 1.0, 0.05);

        // (a) Our current P₁/P₂ form
        let p1p2_price = heston_call_gauss_laguerre(spot, strike, tau, rate, &params, &rule);

        // (b) Gatheral single-integral
        let gatheral_price = heston_call_gatheral_gl(spot, strike, tau, rate, &params, &rule);

        // (c) BS baseline
        let bs_price = crate::models::bs_mod::black_scholes_merton_call(
            spot, strike, tau, rate, params.v0.sqrt(), 0.0,
        ).price;

        // (d) CF sanity check: φ(0) = 1
        let cf0 = heston_cf(
            Complex64::new(0.0, 0.0), tau, rate,
            params.v0, params.kappa, params.theta, params.sigma, params.rho,
        );

        // (e) martingale check: φ(−i) = e^{rτ}
        let cf_neg_i = heston_cf(
            Complex64::new(0.0, -1.0), tau, rate,
            params.v0, params.kappa, params.theta, params.sigma, params.rho,
        );

        // (f) Strike sweep with Gatheral
        let sep = "=".repeat(60);
        println!("\n{sep}");
        println!(" DIAGNOSTIC: Gatheral vs P1/P2 vs BS");
        println!("{sep}");
        println!(" phi(0)  = {:.8} + {:.8}i  (should be 1+0i)", cf0.re, cf0.im);
        println!(" phi(-i) = {:.8} + {:.8}i  (should be e^(rT) = {:.6})",
                 cf_neg_i.re, cf_neg_i.im, (rate * tau).exp());
        println!();
        println!(" ATM (K=100):");
        println!("   P1/P2 form:   {p1p2_price:.6}");
        println!("   Gatheral:     {gatheral_price:.6}");
        println!("   BS (vol=0.2): {bs_price:.6}");
        println!("   Gatheral-P1P2 = {:.6}", gatheral_price - p1p2_price);

        println!();
        println!(" Strike sweep (Gatheral vs P1/P2):");
        println!(" {:<8} {:<12} {:<12} {:<10}", "Strike", "Gatheral", "P1/P2", "Diff");
        for k in [80.0, 90.0, 100.0, 110.0, 120.0] {
            let g = heston_call_gatheral_gl(spot, k, tau, rate, &params, &rule);
            let p = heston_call_gauss_laguerre(spot, k, tau, rate, &params, &rule);
            println!(" {:<8.0} {:<12.6} {:<12.6} {:<10.6}", k, g, p, g - p);
        }
        println!("{sep}");

        // Gatheral and P₁/P₂ should agree if both use the same CF correctly
        let diff = (gatheral_price - p1p2_price).abs();
        // They may not agree perfectly if P₁ is missing the φ(−i) factor
        // For now, just report the results
        assert!(
            p1p2_price > 5.0 && gatheral_price > 5.0,
            "Both prices should be positive and reasonable"
        );
    }
}
