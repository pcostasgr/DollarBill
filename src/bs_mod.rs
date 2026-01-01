// src/bs_mod.rs
// Black-Scholes analytical pricer + Greeks + Implied Vol + Mock Chain
// Pure Rust, zero external crates
// Uses Abramowitz & Stegun approximation for norm CDF (error < 7.5e-8)

const FRAC_1_SQRT_2PI: f64 = 0.39894228040143267793994605993439;  // 1 / √(2π) — the real constant we need

/// Standard normal probability density function φ(x) = (1/√(2π)) * exp(-x²/2)
fn norm_pdf(x: f64) -> f64 {
    FRAC_1_SQRT_2PI * (-0.5 * x * x).exp()
}

/// Standard normal cumulative distribution function N(x)
/// Abramowitz & Stegun rational approximation (7.1.26)
/// Fast, fixed ops, no loops — production-grade for pricing
pub fn norm_cdf_abst(x: f64) -> f64 {
    if x >= 0.0 {
        let t = 1.0 / (1.0 + 0.2316419 * x);
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

/// All Black-Scholes Greeks in one struct
#[derive(Debug, Clone, Copy)]
pub struct Greeks {
    pub price: f64,
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

/// Black-Scholes European call pricer + full Greeks
/// Black-Scholes-Merton European call pricer + Greeks (with dividend yield q)
/// Set q=0.0 for vanilla BS
pub fn black_scholes_merton_call(
    s: f64,
    k: f64,
    t: f64,
    r: f64,
    sigma: f64,
    q: f64,  // Dividend yield (continuous, default 0.0)
) -> Greeks {
    if t <= 0.0 {
        // Same as before
        let price = s.max(k) - k;
        let delta = if s > k { 1.0 } else { 0.0 };
        return Greeks { price, delta, gamma: 0.0, theta: 0.0, vega: 0.0, rho: 0.0 };
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
    let theta = - (s * e_qt * n_d1_pdf * sigma) / (2.0 * sqrt_t) + q * s * e_qt * nd1 - r * k * e_rt * nd2;
    let rho = k * t * e_rt * nd2;

    Greeks { price, delta, gamma, theta, vega, rho }
}

/// Newton-Raphson implied volatility solver for calls
pub fn implied_vol_call(
    market_price: f64,
    s: f64,
    k: f64,
    t: f64,
    r: f64,
    initial_guess: f64,
    tolerance: f64,
    max_iter: usize,
) -> Result<f64, &'static str> {
    if market_price <= 0.0 {
        return Err("Market price must be positive");
    }    if t <= 0.0 {
        let intrinsic = (s - k).max(0.0);
        if market_price < intrinsic {
            return Err("Market price below intrinsic — arbitrage!");
        }
        return Ok(0.0);
    }

    let mut sigma = initial_guess.max(1e-10);
    for _ in 0..max_iter {
        let greeks = black_scholes_merton_call(s, k, t, r, sigma,0.0);
        let price_diff = greeks.price - market_price;

        if price_diff.abs() < tolerance {
            return Ok(sigma);
        }

        let vega = greeks.vega;
        if vega < 1e-10 {
            return Err("Vega too small — no convergence");
        }

        sigma -= price_diff / vega;

        if sigma <= 0.0 {
            return Err("Implied vol went negative");
        }
        if sigma > 10.0 {
            return Err("Implied vol exploded");
        }
    }
    Err("Failed to converge")
}

/// Mock call option from chain
#[derive(Debug)]
pub struct MockCall {
    pub strike: f64,
    pub market_price: f64,
}

// Constants for implied volatility solver
const DEFAULT_IV_GUESS: f64 = 0.3;
const IV_TOLERANCE: f64 = 1e-8;
const IV_MAX_ITERATIONS: usize = 100;

/// Compute implied vol smile from a vector of mock calls
pub fn compute_mock_vol_smile(
    s: f64,
    t: f64,
    r: f64,
    calls: Vec<MockCall>,
) -> Vec<(f64, f64, Option<f64>)> {
    calls.into_iter()
        .map(|call| {
            let iv_result = implied_vol_call(
                call.market_price,
                s,
                call.strike,
                t,
                r,
                DEFAULT_IV_GUESS,
                IV_TOLERANCE,
                IV_MAX_ITERATIONS,
            );
            let iv = iv_result.ok();
            (call.strike, call.market_price, iv)
        })
        .collect()
}