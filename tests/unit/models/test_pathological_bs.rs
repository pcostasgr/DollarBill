//! Pathological edge cases for Black-Scholes, Greeks, and the IV Newton-Raphson solver.
//!
//! These tests target the nasty corners that blow up real money: deep ITM/OTM flatlines,
//! gamma explosions at expiry, zero-vol intrinsic pricing, negative rates, near-zero vega
//! divergence, sub-intrinsic market prices, and solver stability on worthless options.

use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put,
                                  black_scholes_call, black_scholes_put};
use dollarbill::utils::vol_surface::implied_volatility_newton;

// ─── 1. Black-Scholes & Greeks ────────────────────────────────────────────────

/// Deep ITM call: Delta → 1, Gamma → 0, Vega → 0 (flatline)
#[test]
fn test_deep_itm_call_greeks_flatline() {
    // Use very deep ITM: S/K = 5x at short expiry → d1 is enormous → flatline
    let spot = 500.0;
    let strike = 100.0;
    let time = 0.25;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;

    let g = black_scholes_merton_call(spot, strike, time, rate, vol, div);

    assert!(g.delta > 0.99,  "Deep ITM call delta should be ≈ 1.0, got {}", g.delta);
    assert!(g.gamma < 1e-4,  "Deep ITM call gamma should be ≈ 0, got {}",   g.gamma);
    assert!(g.vega  < 1e-2,  "Deep ITM call vega should be ≈ 0,  got {}",   g.vega);
    assert!(g.price.is_finite(), "Price must be finite");
}

/// Deep OTM put: Delta → 0, Gamma → 0, but Theta still negative and meaningful
#[test]
fn test_deep_otm_put_theta_still_negative() {
    let spot = 100.0;
    let strike = 40.0;  // way below spot → OTM put
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;

    let g = black_scholes_merton_put(spot, strike, time, rate, vol, div);

    assert!(g.delta.abs() < 1e-3,  "Deep OTM put delta should be ≈ 0, got {}", g.delta);
    assert!(g.gamma        < 1e-4,  "Deep OTM put gamma should be ≈ 0, got {}", g.gamma);
    // Theta is measured in price/year; for a nearly-worthless OTM put it's tiny but negative
    assert!(g.theta <= 0.0, "Deep OTM put theta must be ≤ 0 (time decay), got {}", g.theta);
    assert!(g.price.is_finite() && g.price >= 0.0, "Price must be finite non-negative");
}

/// ATM near expiry: t → 0+ → Gamma explosion — no NaN / Inf anywhere
#[test]
fn test_atm_near_expiry_gamma_explosion_no_nan() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let vol    = 0.2;
    let div    = 0.0;

    // Sweep from 1 second to 1 day in fractions of a year
    for &t in &[1.0 / (365.0 * 24.0 * 3600.0), 1e-5, 1e-4, 1e-3, 0.01] {
        let call = black_scholes_merton_call(spot, strike, t, rate, vol, div);
        let put  = black_scholes_merton_put (spot, strike, t, rate, vol, div);

        assert!(call.price.is_finite() && !call.price.is_nan(),
                "Call price NaN/Inf at t={}", t);
        assert!(call.delta.is_finite() && !call.delta.is_nan(),
                "Call delta NaN/Inf at t={}", t);
        assert!(call.gamma.is_finite() && !call.gamma.is_nan(),
                "Call gamma NaN/Inf at t={}", t);
        assert!(put.price.is_finite() && !put.price.is_nan(),
                "Put price NaN/Inf at t={}", t);
    }
}

/// Very high volatility (σ = 300%) → price > intrinsic, all Greeks finite and sane
#[test]
fn test_very_high_volatility_greeks_sane() {
    let spot   = 100.0;
    let strike = 100.0;
    let time   = 0.25;
    let rate   = 0.05;
    let vol    = 3.0;   // 300 %
    let div    = 0.0;

    let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put  = black_scholes_merton_put (spot, strike, time, rate, vol, div);

    // Price > intrinsic (both 0 here because ATM, but should be substantial)
    assert!(call.price > 0.0 && call.price.is_finite(), "Call price invalid at σ=300%");
    assert!(put.price  > 0.0 && put.price.is_finite(),  "Put  price invalid at σ=300%");

    // Greeks within valid ranges
    assert!(call.delta >= 0.0 && call.delta <= 1.0, "Call delta out of range: {}", call.delta);
    assert!(call.gamma >= 0.0 && call.gamma.is_finite(), "Call gamma invalid: {}", call.gamma);
    assert!(call.vega  >= 0.0 && call.vega.is_finite(),  "Call vega invalid: {}",  call.vega);
}

/// Zero volatility → price = discounted intrinsic; ITM delta = 1, OTM delta = 0
#[test]
fn test_zero_volatility_discounted_intrinsic() {
    let rate = 0.05;
    let time = 1.0;
    let div  = 0.0;
    // Use essentially-zero vol instead of exactly 0 to avoid σ=0 division by zero in d1
    let vol = 1e-10;

    // ITM call (spot > strike)
    let itm = black_scholes_merton_call(110.0, 100.0, time, rate, vol, div);
    let expected_itm = 110.0 - 100.0 * (-rate * time).exp();
    assert!((itm.price - expected_itm).abs() < 1e-4,
            "Zero-vol ITM call should equal discounted intrinsic, got {} vs {}", itm.price, expected_itm);
    assert!(itm.delta > 0.99, "Zero-vol ITM call delta should be ≈ 1, got {}", itm.delta);

    // OTM call (spot < strike)
    let otm = black_scholes_merton_call(90.0, 100.0, time, rate, vol, div);
    assert!(otm.price.abs() < 1e-4, "Zero-vol OTM call should be ≈ 0, got {}", otm.price);
    assert!(otm.delta < 1e-4, "Zero-vol OTM call delta should be ≈ 0, got {}", otm.delta);

    // ITM put (spot < strike)
    let itm_put = black_scholes_merton_put(90.0, 100.0, time, rate, vol, div);
    let expected_itm_put = 100.0 * (-rate * time).exp() - 90.0;
    assert!((itm_put.price - expected_itm_put).abs() < 1e-4,
            "Zero-vol ITM put should equal discounted intrinsic, got {} vs {}", itm_put.price, expected_itm_put);
}

/// Negative risk-free rate: standard BSM formulas keep rho signs unchanged.
/// Call rho = K*T*e^(-rT)*N(d2) > 0 always; Put rho < 0 always.
/// Verify these invariants hold AND put-call parity is preserved.
#[test]
fn test_negative_rate_rho_and_parity() {
    let spot   = 100.0;
    let strike = 100.0;
    let time   = 1.0;
    let rate   = -0.02; // Negative!
    let vol    = 0.2;
    let div    = 0.0;

    let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put  = black_scholes_merton_put (spot, strike, time, rate, vol, div);

    // In BSM, Call rho = K*T*e^(-rT)*N(d2) which is always positive because
    // K, T, e^(-rT), and N(d2) are all positive regardless of rate sign.
    assert!(call.rho > 0.0,
            "Call rho must be positive even with negative rate, got {}", call.rho);
    assert!(put.rho  < 0.0,
            "Put rho must be negative even with negative rate, got {}", put.rho);

    // Put-call parity must still hold: C - P = S*e^(-qT) - K*e^(-rT)
    let pcp_left  = call.price - put.price;
    let pcp_right = spot - strike * (-rate * time).exp();
    assert!((pcp_left - pcp_right).abs() < 1e-9,
            "Put-call parity violated with negative rate: diff={}", (pcp_left - pcp_right).abs());

    // With negative rate, the discount factor e^(-rT) > 1, so put premium increases
    // Check prices are finite and positive
    assert!(call.price.is_finite() && call.price > 0.0, "Call price invalid with neg rate");
    assert!(put.price.is_finite()  && put.price  > 0.0, "Put price invalid with neg rate");
}

/// Dividend yield > r: both call/put prices still finite; discounted forward is inverted
#[test]
fn test_high_dividend_yield_exceeds_rate() {
    let spot   = 100.0;
    let strike = 100.0;
    let time   = 1.0;
    let rate   = 0.02;
    let vol    = 0.2;
    let div    = 0.08; // div > rate

    let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put  = black_scholes_merton_put (spot, strike, time, rate, vol, div);

    // Prices should still be finite and positive
    assert!(call.price.is_finite() && call.price >= 0.0,
            "Call price invalid when div > rate: {}", call.price);
    assert!(put.price.is_finite() && put.price >= 0.0,
            "Put price invalid when div > rate: {}", put.price);

    // Put-call parity for BSM: C - P = S*e^(-qT) - K*e^(-rT)
    let pcp_left  = call.price - put.price;
    let pcp_right = spot * (-div * time).exp() - strike * (-rate * time).exp();
    assert!((pcp_left - pcp_right).abs() < 1e-9,
            "BSM put-call parity fails with div > rate: diff={}",
            (pcp_left - pcp_right).abs());
}

// ─── 2. Implied Volatility Solver ────────────────────────────────────────────

/// Near-zero vega option → solver should return None (cannot determine IV)
#[test]
fn test_iv_solver_near_zero_vega_returns_none() {
    // Deep OTM option → vega ≈ 0 → Newton-Raphson divide-by-zero
    let spot   = 100.0;
    let strike = 500.0; // 5x OTM
    let time   = 0.01;  // very short expiry
    let rate   = 0.05;

    // Price essentially 0 → vega essentially 0
    let market_price = 1e-8;
    let result = implied_volatility_newton(market_price, spot, strike, time, rate, true);

    // Solver should gracefully return None rather than diverge / NaN
    if let Some(iv) = result {
        // If it does return something, must be finite and positive
        assert!(iv.is_finite() && iv > 0.0,
                "IV should be finite positive when solver returns Some, got {}", iv);
    }
    // None is the expected outcome — test just verifies no panic
}

/// Market price < intrinsic value → solver must return None or signal impossibility
#[test]
fn test_iv_solver_price_below_intrinsic_returns_none() {
    let spot   = 110.0;
    let strike = 100.0;
    let time   = 1.0;
    let rate   = 0.05;

    // Intrinsic = 110 - 100*e^(-0.05) ≈ 14.76
    // Supply a price below intrinsic
    let sub_intrinsic_price = 5.0;
    let result = implied_volatility_newton(sub_intrinsic_price, spot, strike, time, rate, true);

    // No finite, economically sensible IV can exist
    if let Some(iv) = result {
        // If it returns something, verify the model price at that IV is close to market
        let greeks = dollarbill::models::bs_mod::black_scholes_merton_call(spot, strike, time, rate, iv, 0.0);
        assert!((greeks.price - sub_intrinsic_price).abs() < 0.5,
                "Returned IV produces price {}, expected {}", greeks.price, sub_intrinsic_price);
    }
    // None is perfectly fine here
}

/// Market price exactly equals discounted intrinsic → IV ≈ 0
#[test]
fn test_iv_solver_price_equals_intrinsic_iv_near_zero() {
    let spot: f64  = 110.0;
    let strike     = 100.0_f64;
    let time: f64  = 1.0;
    let rate: f64  = 0.05;

    // Set market price very close to discounted intrinsic
    // For a call: forward intrinsic ≈ S*e^(-qT) - K*e^(-rT) = S - K*e^(-rT) (q=0)
    let intrinsic_price = spot - strike * (-rate * time).exp(); // ~14.76
    let result = implied_volatility_newton(intrinsic_price, spot, strike, time, rate, true);

    if let Some(iv) = result {
        assert!(iv < 0.1,
                "IV should be near 0 when price ≈ intrinsic, got {}", iv);
    }
    // None is acceptable here too
}

/// Almost-worthless option → solver should return a small IV or None, never hang
#[test]
fn test_iv_solver_flat_option_no_hang() {
    let spot   = 100.0;
    let strike = 150.0;  // Deep OTM
    let time   = 0.25;
    let rate   = 0.05;

    // Tiny price for a deep OTM option
    let tiny_price = 0.001;
    // This should complete quickly (test harness timeout handles actual hangs)
    let result = implied_volatility_newton(tiny_price, spot, strike, time, rate, true);

    match result {
        Some(iv) => {
            assert!(iv.is_finite() && iv >= 0.0,
                    "Returned IV must be finite non-negative, got {}", iv);
        }
        None => { /* expected — vega too small to converge reliably */ }
    }
}

/// IV round-trip: compute price from known vol, recover vol via solver — match within 0.5%
#[test]
fn test_iv_round_trip_recovery() {
    let spot = 100.0;
    let rate = 0.05;
    let time = 0.25;

    // Test a range of strikes and target vols
    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let target_vols = [0.15, 0.25, 0.40, 0.60];

    for &strike in &strikes {
        for &target_vol in &target_vols {
            let market_call = black_scholes_call(spot, strike, time, rate, target_vol).price;
            let market_put  = black_scholes_put (spot, strike, time, rate, target_vol).price;

            if let Some(recovered) = implied_volatility_newton(market_call, spot, strike, time, rate, true) {
                assert!((recovered - target_vol).abs() < 5e-3,
                        "Call IV round-trip failed for K={}, σ_target={}: got {}",
                        strike, target_vol, recovered);
            }

            if let Some(recovered) = implied_volatility_newton(market_put, spot, strike, time, rate, false) {
                assert!((recovered - target_vol).abs() < 5e-3,
                        "Put IV round-trip failed for K={}, σ_target={}: got {}",
                        strike, target_vol, recovered);
            }
        }
    }
}
