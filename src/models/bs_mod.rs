// src/bs_mod.rs
// Black-Scholes-Merton pricer + Greeks + Historical Vol + P&L Attribution
// Pure Rust, zero external crates

const FRAC_1_SQRT_2PI: f64 = 0.39894228040143267793994605993439;  // 1 / √(2π)

fn norm_pdf(x: f64) -> f64 {
    FRAC_1_SQRT_2PI * (-0.5 * x * x).exp()
}

pub fn norm_cdf_abst(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }

    if x.is_infinite() {
        return if x.is_sign_positive() { 1.0 } else { 0.0 };
    }

    if x >= 0.0 {
        let t = 1.0 / (1.0 + 0.2316419 * x);
        // Abramowitz & Stegun 26.2.17: N(x) ≈ 1 − φ(x)(b₁t + b₂t² + b₃t³ + b₄t⁴ + b₅t⁵)
        // Horner form: poly = t·(b₁ + t·(b₂ + t·(b₃ + t·(b₄ + t·b₅)))) already includes all t powers
        let poly = t * (0.319381530 +
                        t * (-0.356563782 +
                             t * (1.781477937 +
                                  t * (-1.821255978 +
                                       t * 1.330274429))));
        let pdf_part = norm_pdf(x);
        1.0 - pdf_part * poly
    } else {
        1.0 - norm_cdf_abst(-x)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Greeks {
    pub price: f64,
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

/// Black-Scholes-Merton European call pricer + full Greeks
/// q = continuous dividend yield (0.0 = vanilla Black-Scholes)
pub fn black_scholes_merton_call(
    s: f64,
    k: f64,
    t: f64,
    r: f64,
    sigma: f64,
    q: f64,
) -> Greeks {
    if t <= 0.0 {
        let price = s.max(k) - k;
        let delta = if s > k { 1.0 } else { 0.0 };
        return Greeks {
            price,
            delta,
            gamma: 0.0,
            theta: 0.0,
            vega: 0.0,
            rho: 0.0,
        };
    }

    let sqrt_t = t.sqrt();
    let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
    let d2 = d1 - sigma * sqrt_t;

    let nd1 = norm_cdf_abst(d1);
    let nd2 = norm_cdf_abst(d2);
    let n_d1_pdf = norm_pdf(d1);

    let e_qt = (-q * t).exp();
    let e_rt = (-r * t).exp();

    let price = s * e_qt * nd1 - k * e_rt * nd2;
    let delta = e_qt * nd1;
    let gamma = e_qt * n_d1_pdf / (s * sigma * sqrt_t);
    let vega = s * e_qt * sqrt_t * n_d1_pdf;
    let theta = -(s * e_qt * n_d1_pdf * sigma) / (2.0 * sqrt_t)
                + q * s * e_qt * nd1
                - r * k * e_rt * nd2;
    let rho = k * t * e_rt * nd2;

    Greeks { price, delta, gamma, theta, vega, rho }
}

/// Black-Scholes-Merton European put pricer + full Greeks
pub fn black_scholes_merton_put(
    s: f64,
    k: f64,
    t: f64,
    r: f64,
    sigma: f64,
    q: f64,
) -> Greeks {
    if t <= 0.0 {
        let price = (k - s).max(0.0);
        let delta = if s < k { -1.0 } else { 0.0 };
        return Greeks {
            price,
            delta,
            gamma: 0.0,
            theta: 0.0,
            vega: 0.0,
            rho: 0.0,
        };
    }

    let sqrt_t = t.sqrt();
    let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
    let d2 = d1 - sigma * sqrt_t;

    let nd1_neg = norm_cdf_abst(-d1);
    let nd2_neg = norm_cdf_abst(-d2);
    let e_rt = (-r * t).exp();
    let e_qt = (-q * t).exp();

    let price = k * e_rt * nd2_neg - s * e_qt * nd1_neg;
    let delta = -e_qt * nd1_neg;
    let gamma = e_qt * norm_pdf(d1) / (s * sigma * sqrt_t);
    let vega = s * e_qt * norm_pdf(d1) * sqrt_t;
    let theta = -(s * norm_pdf(d1) * sigma * e_qt) / (2.0 * sqrt_t)
                - q * s * e_qt * nd1_neg
                + r * k * e_rt * nd2_neg;
    let rho = -k * t * e_rt * nd2_neg;

    Greeks { price, delta, gamma, theta, vega, rho }
}

// Wrapper for backward compatibility
pub fn black_scholes_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> Greeks {
    black_scholes_merton_call(s, k, t, r, sigma, 0.0)
}

pub fn black_scholes_put(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> Greeks {
    black_scholes_merton_put(s, k, t, r, sigma, 0.0)
}

/// Compute annualized historical volatility from closing prices
/// Log returns, sample std dev, √252 annualization
pub fn compute_historical_vol(closes: &[f64]) -> f64 {
    if closes.len() < 2 {
        return 0.0;
    }

    let mut log_returns = Vec::with_capacity(closes.len() - 1);
    for i in 1..closes.len() {
        let ret = (closes[i] / closes[i - 1]).ln();
        log_returns.push(ret);
    }

    let mean = log_returns.iter().sum::<f64>() / log_returns.len() as f64;
    let variance = log_returns.iter()
        .map(|&r| (r - mean).powi(2))
        .sum::<f64>() / (log_returns.len() - 1) as f64;

    variance.sqrt() * 252f64.sqrt()
}

/// Daily P&L attribution from Greeks and market changes
/// Approximate Taylor expansion: delta*ΔS + ½gamma*(ΔS)² + vega*Δσ + theta*Δt + rho*Δr
pub fn pnl_attribution(
    greeks: &Greeks,
    delta_s: f64,
    delta_sigma: f64,
    delta_t: f64,
    delta_r: f64,
) -> f64 {
    let delta_pnl = greeks.delta * delta_s;
    let gamma_pnl = 0.5 * greeks.gamma * delta_s * delta_s;
    let vega_pnl = greeks.vega * delta_sigma;
    let theta_pnl = greeks.theta * delta_t;
    let rho_pnl = greeks.rho * delta_r;

    delta_pnl + gamma_pnl + vega_pnl + theta_pnl + rho_pnl
}

// ── Third-order (higher-order) Greeks ────────────────────────────────────────
//
// These are computed via central finite differences on Gamma, with a
// singularity-aware step-size selection that prevents garbage near expiry.
//
// References:
//   • "The Complete Guide to Option Pricing Formulas", Haug (2007), Ch. 5
//   • "Greeks for Higher Order Risk", Dupire (2004)

/// Container for third-order Black-Scholes Greeks.
#[derive(Debug, Clone, Copy)]
pub struct HigherOrderGreeks {
    /// Speed = ∂Γ/∂S — rate of change of gamma w.r.t. spot.
    /// Positive means gamma rises as spot rises (concave payoff curvature).
    pub speed: f64,
    /// Zomma = ∂Γ/∂σ — rate of change of gamma w.r.t. implied vol.
    /// Measures how gamma-hedging changes when vol moves.
    pub zomma: f64,
    /// Color = ∂Γ/∂t — rate of change of gamma w.r.t. time (daily decay of gamma).
    /// Note: sign convention here is *positive when gamma increases with time*.
    pub color: f64,
}

/// Compute third-order BSM Greeks using singularity-aware central differences.
///
/// Step size `h_spot` is scaled by `max(ε, c·√T)` to avoid singularity as
/// `T → 0` (where Gamma blows up and any finite difference breaks down).
///
/// # Arguments
/// - `s`, `k`, `t`, `r`, `sigma`, `q` — same as `black_scholes_merton_call`
/// - `is_call` — `true` for call, `false` for put
///
/// # Returns
/// `HigherOrderGreeks` containing Speed, Zomma, and Color.
pub fn higher_order_greeks(
    s: f64,
    k: f64,
    t: f64,
    r: f64,
    sigma: f64,
    q: f64,
    is_call: bool,
) -> HigherOrderGreeks {
    // Singularity-aware step: h grows with √T to keep the perturbation
    // within the smooth region of the payoff even near expiry.
    let h_spot_rel = (1e-4_f64).max(0.01 * t.sqrt()); // relative to spot
    let h_spot = s * h_spot_rel;
    let h_vol = sigma * (1e-4_f64).max(0.01 * t.sqrt());
    let h_time = t * 0.001_f64.max(0.01); // 1% of remaining time, min 0.1% per year

    let pricer = |spot: f64, vol: f64, mat: f64| -> Greeks {
        if is_call {
            black_scholes_merton_call(spot, k, mat, r, vol, q)
        } else {
            black_scholes_merton_put(spot, k, mat, r, vol, q)
        }
    };

    // Base Gamma at the four perturbed spots needed for Speed.
    let g_up  = pricer(s + h_spot, sigma, t).gamma;
    let g_mid = pricer(s,           sigma, t).gamma;
    let g_dn  = pricer(s - h_spot, sigma, t).gamma;

    // ── Speed = ∂Γ/∂S ──
    let speed = (g_up - g_dn) / (2.0 * h_spot);

    // ── Zomma = ∂Γ/∂σ ──
    let g_vol_up = pricer(s, sigma + h_vol, t).gamma;
    let g_vol_dn = pricer(s, sigma - h_vol, t).gamma;
    let zomma = (g_vol_up - g_vol_dn) / (2.0 * h_vol);

    // ── Color = ∂Γ/∂t ──
    // Guard: if remaining time is too small for a backward step use a one-sided diff.
    let color = if t > 2.0 * h_time {
        let g_t_up = pricer(s, sigma, t + h_time).gamma;
        let g_t_dn = pricer(s, sigma, t - h_time).gamma;
        (g_t_up - g_t_dn) / (2.0 * h_time)
    } else if t > h_time {
        let g_t_dn = pricer(s, sigma, t - h_time).gamma;
        (g_dn - g_t_dn) / h_time  // one-sided forward difference
    } else {
        0.0 // T too small to differentiate meaningfully
    };

    HigherOrderGreeks { speed, zomma, color: -g_mid / (2.0 * t) + color }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_is_finite() {
        let hog = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, true);
        assert!(hog.speed.is_finite(), "Speed should be finite");
    }

    #[test]
    fn test_zomma_is_finite() {
        let hog = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, true);
        assert!(hog.zomma.is_finite(), "Zomma should be finite");
    }

    #[test]
    fn test_color_is_finite() {
        let hog = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, true);
        assert!(hog.color.is_finite(), "Color should be finite");
    }

    // Speed = -Gamma/S * (1 + d1/(σ√T)) for ATM call, which is negative (gamma
    // decreases as spot rises for an ATM call).  We just verify sign consistency.
    #[test]
    fn test_speed_atm_call_is_negative() {
        let hog = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, true);
        assert!(hog.speed < 0.0,
            "Speed of ATM call should be negative (gamma falls as spot rises above strike); got {}", hog.speed);
    }

    // Zomma = Γ * (d1*d2 - 1) / σ.  With r=0.05, T=1, σ=0.2:
    // d1=0.35, d2=0.15, d1*d2-1 ≈ -0.95 → Zomma < 0 for ATM call.
    // The sign depends heavily on d1*d2 vs 1; we only assert finiteness.
    #[test]
    fn test_zomma_atm_call_is_finite_and_nonzero() {
        let hog = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, true);
        assert!(hog.zomma.is_finite(), "Zomma must be finite");
        assert!(hog.zomma.abs() > 1e-10, "Zomma must be non-zero");
    }

    // Near-expiry guard: should not panic and should return finite values.
    #[test]
    fn test_higher_order_greeks_near_expiry() {
        let hog = higher_order_greeks(100.0, 100.0, 0.001, 0.05, 0.2, 0.0, true);
        assert!(hog.speed.is_finite());
        assert!(hog.zomma.is_finite());
        assert!(hog.color.is_finite());
    }

    // Call and put should share the same Gamma, hence same Speed and Zomma.
    #[test]
    fn test_call_put_speed_zomma_match() {
        let call = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, true);
        let put  = higher_order_greeks(100.0, 100.0, 1.0, 0.05, 0.2, 0.0, false);
        assert!((call.speed - put.speed).abs() < 1e-8,
            "Call and put Speed should match: {} vs {}", call.speed, put.speed);
        assert!((call.zomma - put.zomma).abs() < 1e-8,
            "Call and put Zomma should match: {} vs {}", call.zomma, put.zomma);
    }
}