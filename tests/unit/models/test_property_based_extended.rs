//! Enhanced property-based tests for financial mathematics invariants.
//!
//! These complement the existing proptest suite with table-driven properties that
//! exercise monotonicity in spot, extended put-call parity with dividends, and
//! other invariants that should hold across a wide parameter grid.

use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};

// ─── 8. Property-Based Tests ─────────────────────────────────────────────────

/// Call price is monotonically non-decreasing in spot price.
/// c(0.99S) ≤ c(S) ≤ c(1.01S) for a grid of (S, K) pairs.
#[test]
fn test_call_price_monotonic_in_spot() {
    let rate = 0.05;
    let time = 0.5;
    let vol  = 0.25;
    let div  = 0.0;

    // Sample a grid of (spot, strike) pairs in [50, 500]
    let spots   = [50.0, 80.0, 100.0, 150.0, 200.0, 300.0, 500.0];
    let strikes = [50.0, 80.0, 100.0, 150.0, 200.0, 300.0, 500.0];

    for &s in &spots {
        for &k in &strikes {
            let p_down = black_scholes_merton_call(s * 0.99, k, time, rate, vol, div).price;
            let p_mid  = black_scholes_merton_call(s,        k, time, rate, vol, div).price;
            let p_up   = black_scholes_merton_call(s * 1.01, k, time, rate, vol, div).price;

            assert!(p_down <= p_mid + 1e-10,
                    "Call not monotone in S: p(0.99S)={:.6} > p(S)={:.6} for S={}, K={}",
                    p_down, p_mid, s, k);
            assert!(p_mid <= p_up + 1e-10,
                    "Call not monotone in S: p(S)={:.6} > p(1.01S)={:.6} for S={}, K={}",
                    p_mid, p_up, s, k);
        }
    }
}

/// Put price is monotonically non-increasing in spot price (inverse of call).
#[test]
fn test_put_price_monotone_decreasing_in_spot() {
    let rate = 0.05;
    let time = 0.5;
    let vol  = 0.25;
    let div  = 0.0;

    let spots   = [50.0, 80.0, 100.0, 150.0, 200.0];
    let strikes = [80.0, 100.0, 120.0, 150.0];

    for &s in &spots {
        for &k in &strikes {
            let p_down = black_scholes_merton_put(s * 0.99, k, time, rate, vol, div).price;
            let p_up   = black_scholes_merton_put(s * 1.01, k, time, rate, vol, div).price;

            assert!(p_down >= p_up - 1e-10,
                    "Put not monotone decreasing in S: p(0.99S)={:.6} < p(1.01S)={:.6} for S={}, K={}",
                    p_down, p_up, s, k);
        }
    }
}

/// Generalised put-call parity with continuous dividends:
/// C - P = S * e^(-qT) - K * e^(-rT)
/// This must hold to machine precision across a wide parameter grid.
#[test]
fn test_put_call_parity_with_dividends() {
    let test_cases = vec![
        // (spot, strike, rate, time, vol, div)
        (100.0, 100.0, 0.05, 0.25, 0.20, 0.0),
        (100.0, 100.0, 0.05, 0.25, 0.20, 0.02),
        (100.0, 100.0, 0.05, 0.25, 0.20, 0.05),
        (100.0,  90.0, 0.05, 0.50, 0.30, 0.01),
        (150.0, 120.0, 0.03, 1.00, 0.25, 0.03),
        (50.0,   55.0, 0.08, 0.10, 0.40, 0.0),
        (200.0, 200.0, -0.01, 0.50, 0.20, 0.0), // negative rate
        (100.0, 100.0, 0.02,  1.00, 0.20, 0.08), // div > rate
    ];

    for (spot, strike, rate, time, vol, div) in test_cases {
        let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
        let put  = black_scholes_merton_put (spot, strike, time, rate, vol, div);

        let pcp_lhs = call.price - put.price;
        let pcp_rhs = spot * (-div * time).exp() - strike * (-rate * time).exp();

        let diff = (pcp_lhs - pcp_rhs).abs();
        assert!(diff < 1e-9,
                "Put-call parity (with div) violated: diff={:.2e} for S={} K={} r={} t={} σ={} q={}",
                diff, spot, strike, rate, time, vol, div);
    }
}

/// Call is always ≥ 0 and put is always ≥ 0 for any valid inputs.
#[test]
fn test_option_prices_always_non_negative() {
    let combos = vec![
        (100.0, 80.0,  0.05,  1.0, 0.1, 0.0),
        (100.0, 120.0, 0.05,  1.0, 0.1, 0.0),
        (100.0, 100.0, -0.02, 1.0, 0.3, 0.0),
        (100.0, 100.0, 0.05,  0.001, 0.5, 0.0),
        (100.0, 100.0, 0.05,  1.0, 3.0, 0.0),  // σ=300%
        (100.0, 100.0, 0.05,  1.0, 0.2, 0.1),  // high div
    ];

    for (s, k, r, t, sigma, q) in combos {
        let call = black_scholes_merton_call(s, k, t, r, sigma, q);
        let put  = black_scholes_merton_put (s, k, t, r, sigma, q);

        assert!(call.price >= -1e-12,
                "Call price negative: {} for s={} k={} r={} t={} σ={} q={}", call.price, s, k, r, t, sigma, q);
        assert!(put.price  >= -1e-12,
                "Put  price negative: {} for s={} k={} r={} t={} σ={} q={}", put.price,  s, k, r, t, sigma, q);
    }
}

/// Delta-call - delta-put = e^(-qT) (generalised relation with dividend yield).
#[test]
fn test_delta_put_call_relation_with_dividends() {
    let test_cases = vec![
        (100.0, 100.0, 0.05, 1.0, 0.2, 0.0),
        (100.0, 100.0, 0.05, 1.0, 0.2, 0.02),
        (100.0, 100.0, 0.05, 1.0, 0.2, 0.05),
        (120.0,  90.0, 0.03, 0.5, 0.3, 0.01),
    ];

    for (s, k, r, t, sigma, q) in test_cases {
        let call = black_scholes_merton_call(s, k, t, r, sigma, q);
        let put  = black_scholes_merton_put (s, k, t, r, sigma, q);

        let delta_diff = call.delta - put.delta;
        let expected   = (-q * t).exp();

        assert!((delta_diff - expected).abs() < 1e-9,
                "Delta relation failed: Δ_call - Δ_put = {:.6} ≠ e^(-qT) = {:.6} for q={}",
                delta_diff, expected, q);
    }
}
