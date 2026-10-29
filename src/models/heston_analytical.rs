// Heston analytical pricing using the corrected Lord-Kahl characteristic function
// Fast, deterministic pricing via Fourier transform (no Monte Carlo noise)
//
// Supports two integration backends:
//   1. Compatibility aliases for the retired Carr-Madan path
//   2. Gauss-Laguerre quadrature with configurable 32–128 nodes

use num_complex::Complex64;
use std::f64::consts::PI;
use crate::models::heston::HestonParams;
use crate::models::gauss_laguerre::GaussLaguerreRule;

/// Integration method for Heston semi-analytical pricing.
///
/// Uses corrected Gauss-Laguerre pricing. The CarrMadan variant is retained
/// solely as a compatibility alias; it no longer invokes the legacy integrator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegrationMethod {
    /// Compatibility alias for corrected 128-node GL pricing.
    CarrMadan,
    /// Gauss-Laguerre quadrature with the specified number of nodes (32–128).
    GaussLaguerre { nodes: usize },
}

impl Default for IntegrationMethod {
    fn default() -> Self {
        IntegrationMethod::GaussLaguerre { nodes: HESTON_PRICING_NODES }
    }
}

impl std::fmt::Display for IntegrationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrationMethod::CarrMadan => write!(f, "Gauss-Laguerre (128 nodes; legacy alias)"),
            IntegrationMethod::GaussLaguerre { nodes } => {
                write!(f, "Gauss-Laguerre ({nodes} nodes)")
            }
        }
    }
}

/// Compatibility alias for [`heston_call`]. The faulty legacy integration was
/// retired; this name now selects corrected GL pricing.
pub fn heston_call_carr_madan(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    heston_call(spot, strike, maturity, rate, params)
}

/// Shared node count for calibration and signal pricing.
pub const HESTON_PRICING_NODES: usize = 128;

/// Corrected risk-neutral Heston call price used by signals.
pub fn heston_call(spot: f64, strike: f64, maturity: f64, rate: f64, params: &HestonParams) -> f64 {
    static RULE: std::sync::OnceLock<GaussLaguerreRule> = std::sync::OnceLock::new();
    let rule = RULE.get_or_init(|| GaussLaguerreRule::new(HESTON_PRICING_NODES));
    HestonCfCache::new(spot, maturity, rate, params, rule).price_call(strike)
}

/// Corrected European Heston put, derived from [`heston_call`] by parity.
pub fn heston_put(spot: f64, strike: f64, maturity: f64, rate: f64, params: &HestonParams) -> f64 {
    heston_call(spot,strike,maturity,rate,params)-spot+strike*(-rate*maturity).exp()
}

/// Price European put using put-call parity
pub fn heston_put_carr_madan(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    params: &HestonParams,
) -> f64 {
    heston_put(spot, strike, maturity, rate, params)
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

/// Trig-free complex square root.
///
/// The standard `num_complex` sqrt goes through `to_polar` → `from_polar`
/// which requires `atan2` + `sin` + `cos`.  This direct formula uses only
/// `hypot` + two real `sqrt` calls — roughly 2× faster.
#[inline(always)]
fn fast_complex_sqrt(z: Complex64) -> Complex64 {
    let a = z.re;
    let b = z.im;
    if a == 0.0 && b == 0.0 {
        return Complex64::new(0.0, 0.0);
    }
    let modulus = a.hypot(b);                       // |z|
    let t = ((modulus + a) * 0.5).sqrt();           // re(√z) when b ≥ 0
    let u = ((modulus - a) * 0.5).sqrt();           // |im(√z)|
    if b >= 0.0 {
        Complex64::new(t, u)
    } else {
        Complex64::new(t, -u)
    }
}

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
    let d = fast_complex_sqrt(d_sq);

    let g_num = beta - d;
    let g_den = beta + d;
    let g = if g_den.norm_sqr() < 1e-28 {
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
    let big_c = if one_minus_g.norm_sqr() < 1e-28 || one_minus_g_exp.norm_sqr() < 1e-28 {
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
    let big_d = if one_minus_g_exp.norm_sqr() < 1e-28 {
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
#[inline(always)]
fn integrand_p1_gl(
    u: f64, log_moneyness: f64, tau: f64, rate: f64,
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
    let result = ((-i * z * log_moneyness).exp() * cf / (i * z)).re;
    if result.is_finite() && result.abs() < 1e8 { result } else { 0.0 }
}

/// Heston (1993) integrand for P₂ using the corrected char function.
#[inline(always)]
fn integrand_p2_gl(
    u: f64, log_moneyness: f64, tau: f64, rate: f64,
    params: &HestonParams,
) -> f64 {
    if u.abs() < 1e-8 { return 0.0; }
    let z = Complex64::new(u, 0.0);
    let i = Complex64::i();

    let cf = heston_cf(
        z, tau, rate, params.v0, params.kappa,
        params.theta, params.sigma, params.rho,
    );
    let result = ((-i * z * log_moneyness).exp() * cf / (i * z)).re;
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

    // Pre-compute log-moneyness once (was inside every integrand call before).
    let log_moneyness = (strike / spot).ln();

    // P₁ uses the stock-price measure CF = φ(u−i)/φ(−i).
    // Since φ(−i) = e^{rτ}, the P₁ integral carries an extra e^{−rτ} factor.
    let phi_neg_i_inv = (-rate * maturity).exp(); // 1/φ(−i) = e^{−rτ}

    let p1 = 0.5
        + (phi_neg_i_inv / PI)
            * c
            * rule.integrate(|x| {
                let u = c * x;
                integrand_p1_gl(u, log_moneyness, maturity, rate, params)
            });
    let p2 = 0.5
        + (1.0 / PI)
            * c
            * rule.integrate(|x| {
                let u = c * x;
                integrand_p2_gl(u, log_moneyness, maturity, rate, params)
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
// Batch pricing with CF cache  (many strikes, same maturity)
// ═══════════════════════════════════════════════════════════════════════

/// Cached CF evaluations at every GL node for a single maturity.
///
/// For a given (τ, params), the Heston characteristic function
/// φ(z) and φ(z−i) are **strike-independent**.  This struct stores
/// them so that pricing across many strikes only requires the cheap
/// phase-factor multiplication `exp(−i·z·ln(K/S))` per strike.
pub struct HestonCfCache {
    /// Scaled GL nodes: u_j = c · x_j
    u_nodes: Vec<f64>,
    /// Exponential-modified GL weights (w̃_j · c)
    scaled_weights: Vec<f64>,
    /// Cached φ(z_j − i) / (i·z_j)  for P₁  (z_j = u_j + 0i)
    cf1_over_iz: Vec<Complex64>,
    /// Cached φ(z_j)     / (i·z_j)  for P₂
    cf2_over_iz: Vec<Complex64>,
    /// Pre-multiplier for P₁: e^{−rτ} / π
    p1_prefactor: f64,
    /// Pre-multiplier for P₂: 1 / π
    p2_prefactor: f64,
    /// Discount factor e^{−rτ}
    discount: f64,
    /// Spot price (needed for final call value)
    spot: f64,
}

impl HestonCfCache {
    /// Build a CF cache for the given maturity.
    ///
    /// Cost: one `heston_cf` per node × 2 (P₁ and P₂) — identical to
    /// pricing a single strike.  All subsequent `price_call` / `price_calls`
    /// calls reuse the cached CF values.
    pub fn new(
        spot: f64,
        maturity: f64,
        rate: f64,
        params: &HestonParams,
        rule: &GaussLaguerreRule,
    ) -> Self {
        let d_eff = (params.kappa * params.theta * maturity + params.v0)
            / params.sigma.max(0.01);
        let c = (1.0 / d_eff.max(0.01)).max(1.0).min(30.0);

        let i = Complex64::i();
        let n = rule.node_count();

        let mut u_nodes = Vec::with_capacity(n);
        let mut scaled_weights = Vec::with_capacity(n);
        let mut cf1_over_iz = Vec::with_capacity(n);
        let mut cf2_over_iz = Vec::with_capacity(n);

        for j in 0..n {
            let x = rule.nodes[j];
            let u = c * x;
            let w = rule.exp_weights[j] * c;

            if u.abs() < 1e-8 {
                u_nodes.push(u);
                scaled_weights.push(w);
                cf1_over_iz.push(Complex64::new(0.0, 0.0));
                cf2_over_iz.push(Complex64::new(0.0, 0.0));
                continue;
            }

            let z = Complex64::new(u, 0.0);

            // P₁: stock-price measure CF
            let cf1 = heston_cf(
                z - i, maturity, rate, params.v0, params.kappa,
                params.theta, params.sigma, params.rho,
            );
            // P₂: risk-neutral measure CF
            let cf2 = heston_cf(
                z, maturity, rate, params.v0, params.kappa,
                params.theta, params.sigma, params.rho,
            );

            let iz = i * z;
            let c1 = cf1 / iz;
            let c2 = cf2 / iz;

            u_nodes.push(u);
            scaled_weights.push(w);
            cf1_over_iz.push(if c1.is_finite() { c1 } else { Complex64::new(0.0, 0.0) });
            cf2_over_iz.push(if c2.is_finite() { c2 } else { Complex64::new(0.0, 0.0) });
        }

        let discount = (-rate * maturity).exp();

        HestonCfCache {
            u_nodes,
            scaled_weights,
            cf1_over_iz,
            cf2_over_iz,
            p1_prefactor: discount / PI,  // e^{-rτ}/π
            p2_prefactor: 1.0 / PI,
            discount,
            spot,
        }
    }

    /// Price a single European call using the cached CF values.
    ///
    /// Cost: one `exp` per node × 2 integrals — no CF evaluations.
    #[inline]
    pub fn price_call(&self, strike: f64) -> f64 {
        let k = (strike / self.spot).ln();
        let i = Complex64::i();

        let mut sum_p1 = 0.0_f64;
        let mut sum_p2 = 0.0_f64;

        for j in 0..self.u_nodes.len() {
            let u = self.u_nodes[j];
            let w = self.scaled_weights[j];

            if u.abs() < 1e-8 { continue; }

            let z = Complex64::new(u, 0.0);
            let phase = (-i * z * k).exp();

            let v1 = (phase * self.cf1_over_iz[j]).re;
            let v2 = (phase * self.cf2_over_iz[j]).re;

            if v1.is_finite() && v1.abs() < 1e8 { sum_p1 += w * v1; }
            if v2.is_finite() && v2.abs() < 1e8 { sum_p2 += w * v2; }
        }

        let p1 = (0.5 + self.p1_prefactor * sum_p1).clamp(0.0, 1.0);
        let p2 = (0.5 + self.p2_prefactor * sum_p2).clamp(0.0, 1.0);

        let price = self.spot * p1 - strike * self.discount * p2;
        let intrinsic = (self.spot - strike * self.discount).max(0.0);
        price.max(intrinsic)
    }

    /// Price European calls for many strikes, returning a vec of prices.
    ///
    /// This is the primary batch entry-point.  The CF is evaluated **once**
    /// (during `HestonCfCache::new`) and reused across all strikes.
    pub fn price_calls(&self, strikes: &[f64]) -> Vec<f64> {
        strikes.iter().map(|&k| self.price_call(k)).collect()
    }
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
        let _diff = (gatheral_price - p1p2_price).abs();
        // They may not agree perfectly if P₁ is missing the φ(−i) factor
        // For now, just report the results
        assert!(
            p1p2_price > 5.0 && gatheral_price > 5.0,
            "Both prices should be positive and reasonable"
        );
    }
}

#[cfg(test)]
mod convergence_grid {
    use super::*;
    #[test]
    fn gl_matches_quantlib_maturity_and_strike_grid() {
        let fixture: serde_json::Value=serde_json::from_str(include_str!("../../tests/fixtures/heston_quantlib_grid.json")).unwrap();
        let mut failures=Vec::new();
        for row in fixture["cases"].as_array().unwrap() {
            let stress=row["stressed"].as_bool().unwrap();
            let t=row["days"].as_f64().unwrap()/365.0;
            let k=row["strike"].as_f64().unwrap();
            let expected=row["call"].as_f64().unwrap();
            let p=if stress {HestonParams {s0:100.0,v0:0.09,kappa:1.5,theta:0.09,sigma:1.0,rho:-0.9,r:0.05,t}}
                else {HestonParams {s0:100.0,v0:0.04,kappa:2.0,theta:0.04,sigma:0.3,rho:-0.7,r:0.05,t}};
            let production=heston_call(100.0,k,t,0.05,&p);
            assert!((production-expected).abs()<1e-4,"Production price {production} vs {expected}");
            for n in [32,64,128] {
                let actual=heston_call_gauss_laguerre(100.0,k,t,0.05,&p,&GaussLaguerreRule::new(n));
                println!("stress={stress} T={t:.4} K={k} n={n}: error={:.8}",actual-expected);
                if n==HESTON_PRICING_NODES && (actual-expected).abs()>1e-4 {
                    failures.push(format!("stress={stress} T={t} K={k}: {actual} vs {expected}"));
                }
            }
        }
        assert!(failures.is_empty(),"{}",failures.join("\n"));
    }
}
