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
        let poly = t * (0.319381530 +
                        t * (-0.356563782 +
                             t * (1.781477937 +
                                  t * (-1.821255978 +
                                       t * 1.330274429))));
        let pdf_part = norm_pdf(x);
        1.0 - pdf_part * poly * t
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
#[allow(dead_code)]
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
                + q * s * e_qt * nd1_neg
                - r * k * e_rt * nd2_neg;
    let rho = -k * t * e_rt * nd2_neg;

    Greeks { price, delta, gamma, theta, vega, rho }
}

// Wrapper for backward compatibility
#[allow(dead_code)]
pub fn black_scholes_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> Greeks {
    black_scholes_merton_call(s, k, t, r, sigma, 0.0)
}

#[allow(dead_code)]
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

// ... keep your other functions: implied_vol_call, compute_mock_vol_smile, etc.