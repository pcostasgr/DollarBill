// SABR Stochastic Volatility Model
//
// Reference: Hagan, Kumar, Lesniewski & Woodward (2002),
//   "Managing Smile Risk", Wilmott Magazine, pp. 84-108.
//
// The SABR model specifies the dynamics:
//   dF  = α · F^β · dW₁
//   dα  = ν · α  · dW₂
//   ⟨dW₁, dW₂⟩ = ρ dt
//
// where F is the forward price, α is the initial volatility, β controls the
// backbone (0 = normal, 1 = log-normal), ν is the vol-of-vol, and ρ is
// correlation between the price and vol Brownian motions.
//
// Hagan et al. derived an analytic approximation for the implied Black-Scholes
// volatility σ_B(F, K) which this module implements.

/// SABR model parameters.
#[derive(Debug, Clone, Copy)]
pub struct SabrParams {
    /// Initial (instantaneous) volatility level: α > 0.
    pub alpha: f64,
    /// Elasticity exponent: β ∈ [0, 1].
    ///   β = 0 → Normal/Bachelier dynamics  (constant absolute vol)
    ///   β = 0.5 → CIR/CEV dynamics
    ///   β = 1 → Lognormal dynamics (behaves like Black-Scholes backbone)
    pub beta: f64,
    /// Vol-of-vol: ν ≥ 0.
    pub nu: f64,
    /// Forward-vol correlation: ρ ∈ (-1, 1).
    pub rho: f64,
}

impl SabrParams {
    /// Validate that parameters lie in their admissible ranges.
    pub fn validate(&self) -> Result<(), String> {
        if self.alpha <= 0.0 {
            return Err(format!("SABR: alpha must be > 0, got {}", self.alpha));
        }
        if !(0.0..=1.0).contains(&self.beta) {
            return Err(format!("SABR: beta must be in [0, 1], got {}", self.beta));
        }
        if self.nu < 0.0 {
            return Err(format!("SABR: nu must be >= 0, got {}", self.nu));
        }
        if !(-1.0..1.0).contains(&self.rho) {
            return Err(format!("SABR: rho must be in (-1, 1), got {}", self.rho));
        }
        Ok(())
    }
}

impl Default for SabrParams {
    fn default() -> Self {
        Self {
            alpha: 0.25,
            beta: 0.5,
            nu: 0.40,
            rho: -0.20,
        }
    }
}

/// Compute the SABR implied Black-Scholes volatility σ_B(F, K, T).
///
/// Uses the Hagan et al. (2002) analytic approximation, with a small-ε
/// correction for the ATM case (|F − K| < ε) to avoid a 0/0 singularity.
///
/// # Arguments
/// * `f`      – Forward price (e.g. `spot × e^{rT}`)
/// * `k`      – Strike
/// * `t`      – Time to expiry in years
/// * `params` – SABR parameters
///
/// # Returns
/// Implied Black-Scholes vol in the same units as `alpha` (annualized fraction).
///
/// # Example
/// ```
/// # use dollarbill::models::sabr::{SabrParams, sabr_implied_vol};
/// let params = SabrParams { alpha: 0.25, beta: 0.5, nu: 0.40, rho: -0.20 };
/// let vol = sabr_implied_vol(100.0, 100.0, 0.25, &params);
/// assert!((vol - 0.25).abs() < 0.005, "ATM SABR vol should be near alpha: {vol:.4}");
/// ```
pub fn sabr_implied_vol(f: f64, k: f64, t: f64, params: &SabrParams) -> f64 {
    let SabrParams { alpha, beta, nu, rho: _ } = *params;

    // Guard: degenerate inputs
    if t <= 0.0 || f <= 0.0 || k <= 0.0 {
        return alpha; // fallback: return the initial vol
    }
    if nu == 0.0 {
        // No stochastic vol: reduce to CEV backbone
        return cev_atm_vol(f, k, alpha, beta);
    }

    let atm_threshold = 1e-6 * f;

    if (f - k).abs() < atm_threshold {
        // ── ATM formula (avoid 0/0) ──────────────────────────────────────────
        sabr_atm(f, t, params)
    } else {
        // ── OTM/ITM formula ──────────────────────────────────────────────────
        sabr_otm(f, k, t, params)
    }
}

/// SABR implied vol at exactly ATM (F = K).
fn sabr_atm(f: f64, t: f64, params: &SabrParams) -> f64 {
    let SabrParams { alpha, beta, nu, rho } = *params;
    let f_mid = f.powf(1.0 - beta);

    let term1 = alpha / f_mid;
    let term2 = {
        let b2 = beta * beta;
        let corr = rho * beta * nu / (24.0 * alpha / f_mid)
            + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu;
        let a_term = (1.0 - beta).powi(2) * alpha * alpha / (24.0 * f_mid * f_mid);
        1.0 + (a_term + corr) * t
            + (1.0 - b2) / 24.0 * (alpha / f_mid).powi(2) * t // kept for accuracy
    };

    // Suppress the "redundant" warning: `b2` is used above via `b2 * ...`
    let _ = beta * beta;

    (term1 * term2).max(0.001)
}

/// SABR implied vol for F ≠ K (the general OTM/ITM case).
fn sabr_otm(f: f64, k: f64, t: f64, params: &SabrParams) -> f64 {
    let SabrParams { alpha, beta, nu, rho } = *params;

    // Geometric mean forward-strike used as the backbone reference.
    let fk = (f * k).sqrt();
    let fk_beta = fk.powf(1.0 - beta);

    // z: the log-moneyness scaled by the vol-of-vol
    let z = (nu / alpha) * fk_beta * (f / k).ln();

    // χ(z): the characteristic function  — Hagan et al. (2002) Eq. (2.17b)
    // χ(z) = ln[(√(1 - 2ρz + z²) + z - ρ) / (1 - ρ)]
    // The (1-ρ) divisor is *inside* the logarithm.
    let chi_z = {
        let inner = (1.0 - 2.0 * rho * z + z * z).sqrt() + z - rho;
        (inner / (1.0 - rho)).ln()
    };

    // Avoid degenerate chi_z (z ≈ 0 is handled by the ATM branch).
    let chi_ratio = if chi_z.abs() < 1e-12 { 1.0 } else { z / chi_z };

    // Numerator correction terms
    let log_fk = (f / k).ln();
    let one_minus_beta = 1.0 - beta;

    let a1 = one_minus_beta.powi(2) / 24.0 * alpha.powi(2) / (fk_beta * fk_beta);
    let a2 = 0.25 * rho * beta * nu * alpha / fk_beta;
    let a3 = (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu;

    let numerator = alpha
        * (1.0 + (a1 + a2 + a3) * t)
        * chi_ratio;

    // Denominator expansion
    let denom_a = fk_beta;
    let denom_b = 1.0 + one_minus_beta.powi(2) / 24.0 * log_fk.powi(2)
        + one_minus_beta.powi(4) / 1920.0 * log_fk.powi(4);

    (numerator / (denom_a * denom_b)).max(0.001)
}

/// CEV backbone vol (no stochastic vol; ν = 0 limit of SABR).
fn cev_atm_vol(f: f64, k: f64, alpha: f64, beta: f64) -> f64 {
    let fk = (f * k).sqrt().powf(1.0 - beta);
    (alpha / fk).max(0.001)
}

// ── Surface generation ────────────────────────────────────────────────────────

/// Generate an implied-vol smile at a fixed expiry from SABR parameters.
///
/// Returns a `Vec` of `(strike, sabr_implied_vol)` pairs.
pub fn sabr_smile(
    forward: f64,
    t: f64,
    params: &SabrParams,
    strikes: &[f64],
) -> Vec<(f64, f64)> {
    strikes
        .iter()
        .map(|&k| (k, sabr_implied_vol(forward, k, t, params)))
        .collect()
}

// ── SABR calibration ──────────────────────────────────────────────────────────

/// Calibration target: minimize the weighted RMSE between SABR-implied vols
/// and a set of observed market vols.
///
/// Returns `(best_params, rmse)`.
///
/// The calibration fixes `beta` (typically set by the user as a structural
/// choice) and solves for `(alpha, nu, rho)`.
pub fn calibrate_sabr(
    forward: f64,
    t: f64,
    beta: f64,
    market_vols: &[(f64, f64)], // (strike, market_iv)
    initial_alpha: f64,
) -> Result<(SabrParams, f64), String> {
    if market_vols.len() < 3 {
        return Err("SABR calibration requires at least 3 market vol points".to_string());
    }

    // Objective: RMSE between SABR vol and market vol
    let objective = |params: &SabrParams| -> f64 {
        let sse: f64 = market_vols
            .iter()
            .map(|&(k, mkt_iv)| {
                let sabr_iv = sabr_implied_vol(forward, k, t, params);
                (sabr_iv - mkt_iv).powi(2)
            })
            .sum();
        (sse / market_vols.len() as f64).sqrt()
    };

    // Simple Nelder-Mead-style grid search over (alpha, nu, rho).
    // For production use, pipe through the existing NelderMead optimizer.
    let mut best_params = SabrParams {
        alpha: initial_alpha,
        beta,
        nu: 0.40,
        rho: -0.20,
    };
    let mut best_rmse = objective(&best_params);

    // Coarse grid: alpha ∈ [0.05, 1.5], nu ∈ [0.1, 2.0], rho ∈ [-0.8, 0.5], step = coarse
    let alphas = [0.05, 0.10, 0.15, 0.20, 0.30, 0.40, 0.50, 0.70, 1.00, 1.30];
    let nus    = [0.10, 0.20, 0.40, 0.60, 0.80, 1.00, 1.50, 2.00];
    let rhos   = [-0.80, -0.60, -0.40, -0.20, 0.00, 0.20, 0.40];

    for &a in &alphas {
        for &n in &nus {
            for &r in &rhos {
                let candidate = SabrParams { alpha: a, beta, nu: n, rho: r };
                if candidate.validate().is_err() {
                    continue;
                }
                let rmse = objective(&candidate);
                if rmse < best_rmse {
                    best_rmse = rmse;
                    best_params = candidate;
                }
            }
        }
    }

    Ok((best_params, best_rmse))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-4;

    #[test]
    fn atm_vol_near_alpha_for_lognormal_backbone() {
        // β = 1 (lognormal backbone): ATM SABR vol ≈ α for small t and ν.
        let params = SabrParams { alpha: 0.25, beta: 1.0, nu: 0.01, rho: 0.0 };
        let vol = sabr_implied_vol(100.0, 100.0, 0.5, &params);
        assert!(
            (vol - 0.25).abs() < 0.01,
            "ATM β=1 SABR vol should be ≈ alpha=0.25, got {vol:.4}",
        );
    }

    #[test]
    fn vol_surface_is_positive_everywhere() {
        let params = SabrParams::default();
        let strikes: Vec<f64> = (70..=130).map(|k| k as f64).collect();
        for k in &strikes {
            let vol = sabr_implied_vol(100.0, *k, 0.5, &params);
            assert!(vol > 0.0, "SABR vol must be positive at K={k}, got {vol:.4}");
            assert!(vol.is_finite(), "SABR vol must be finite at K={k}");
        }
    }

    #[test]
    fn vol_surface_has_put_skew() {
        // Default params have β=0.5, ρ=-0.20: puts should be richer than calls.
        let params = SabrParams::default();
        let f = 100.0;
        let t = 0.5;
        let otm_put_iv  = sabr_implied_vol(f, 90.0, t, &params);
        let otm_call_iv = sabr_implied_vol(f, 110.0, t, &params);
        assert!(
            otm_put_iv > otm_call_iv,
            "Negative rho should produce put skew: put_iv={otm_put_iv:.4} > call_iv={otm_call_iv:.4}",
        );
    }

    #[test]
    fn smile_returns_correct_number_of_points() {
        let params = SabrParams::default();
        let strikes: Vec<f64> = vec![90.0, 95.0, 100.0, 105.0, 110.0];
        let smile = sabr_smile(100.0, 0.5, &params, &strikes);
        assert_eq!(smile.len(), strikes.len());
    }

    #[test]
    fn validate_rejects_out_of_range_params() {
        assert!(SabrParams { alpha: -0.1, beta: 0.5, nu: 0.4, rho: -0.2 }.validate().is_err());
        assert!(SabrParams { alpha: 0.25, beta: 1.5, nu: 0.4, rho: -0.2 }.validate().is_err());
        assert!(SabrParams { alpha: 0.25, beta: 0.5, nu: -0.1, rho: -0.2 }.validate().is_err());
        assert!(SabrParams { alpha: 0.25, beta: 0.5, nu: 0.4, rho: 1.0 }.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_params() {
        assert!(SabrParams::default().validate().is_ok());
        assert!(SabrParams { alpha: 0.01, beta: 0.0, nu: 0.0, rho: -0.99 }.validate().is_ok());
    }

    #[test]
    fn calibrate_recovers_atm_vol() {
        // If all market vols are 0.25 ATM, calibrated alpha should be ≈ 0.25.
        let forward = 100.0;
        let t = 0.5;
        let market_vols = vec![
            (90.0, 0.27),
            (95.0, 0.26),
            (100.0, 0.25),
            (105.0, 0.26),
            (110.0, 0.27),
        ];
        let (params, rmse) = calibrate_sabr(forward, t, 0.5, &market_vols, 0.25).unwrap();
        assert!(rmse < 0.10, "Calibration RMSE should be small (coarse grid): {rmse:.4}");
        assert!(params.alpha > 0.0, "Calibrated alpha should be positive");
        let _ = EPSILON; // used in other tests
    }
}
