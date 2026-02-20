//! Edge case tests for extreme parameter values and boundary conditions

use dollarbill::calibration::nelder_mead::{NelderMead, NelderMeadConfig};
use dollarbill::models::bs_mod::{black_scholes_call, black_scholes_put, Greeks};
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::heston_call_carr_madan;

// Helper to keep argument order consistent with current API (s, k, t, r, sigma)
fn bs_call(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_call(s, k, t, r, sigma)
}

fn bs_put(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_put(s, k, t, r, sigma)
}

#[test]
fn test_zero_time_to_expiry() {
    // Options expiring immediately should equal intrinsic value
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1e-10; // Essentially zero

    let test_strikes = vec![80.0, 100.0, 120.0];

    for strike in test_strikes {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        let call_intrinsic = (spot - strike).max(0.0);
        let put_intrinsic = (strike - spot).max(0.0);

        assert!(
            (call.price - call_intrinsic).abs() < 1e-3,
            "Call price doesn't approach intrinsic at expiry: price={:.6}, intrinsic={:.6}",
            call.price, call_intrinsic
        );

        assert!(
            (put.price - put_intrinsic).abs() < 1e-3,
            "Put price doesn't approach intrinsic at expiry: price={:.6}, intrinsic={:.6}",
            put.price, put_intrinsic
        );
    }
}

#[test]
fn test_zero_volatility() {
    // Zero volatility should give pure intrinsic + time value
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 1e-10; // Essentially zero

    let call = bs_call(spot, strike, rate, time, vol);
    let put = bs_put(spot, strike, rate, time, vol);

    // With zero vol, ATM options should have minimal time value
    let expected_call = (spot - strike * (-rate * time).exp()).max(0.0);
    let expected_put = (strike * (-rate * time).exp() - spot).max(0.0);

    assert!(
        (call.price - expected_call).abs() < 1e-6,
        "Zero vol call price incorrect: got {:.6}, expected {:.6}",
        call.price, expected_call
    );

    assert!(
        (put.price - expected_put).abs() < 1e-6,
        "Zero vol put price incorrect: got {:.6}, expected {:.6}",
        put.price, expected_put
    );
}

#[test]
fn test_extreme_strike_prices() {
    let spot = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;

    // Deep ITM call (strike near zero)
    let deep_itm_strike = 0.01;
    let deep_itm_call = bs_call(spot, deep_itm_strike, rate, time, vol).price;
    let expected_deep_itm = spot - deep_itm_strike * (-rate * time).exp();

    assert!(
        (deep_itm_call - expected_deep_itm).abs() < 0.01,
        "Deep ITM call pricing error: got {:.4}, expected ~{:.4}",
        deep_itm_call, expected_deep_itm
    );

    // Deep OTM call (strike very high)
    let deep_otm_strike = 10000.0;
    let deep_otm_call = bs_call(spot, deep_otm_strike, rate, time, vol).price;

    assert!(
        deep_otm_call < 0.01,
        "Deep OTM call should be near zero: got {:.6}",
        deep_otm_call
    );

    // Deep ITM put
    let deep_itm_put = bs_put(spot, deep_otm_strike, rate, time, vol).price;
    let expected_deep_itm_put = deep_otm_strike * (-rate * time).exp() - spot;

    assert!(
        (deep_itm_put - expected_deep_itm_put).abs() < 1.0,
        "Deep ITM put pricing error: got {:.4}, expected ~{:.4}",
        deep_itm_put, expected_deep_itm_put
    );
}

#[test]
fn test_extreme_volatility() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;

    // Very high volatility
    let high_vol = 5.0; // 500% volatility
    let high_vol_call = bs_call(spot, strike, rate, time, high_vol);
    let high_vol_put = bs_put(spot, strike, rate, time, high_vol);

    // With extreme volatility, options should have significant time value
    assert!(
        high_vol_call.price > spot * 0.3,
        "High vol call too cheap: {:.4} with σ={}",
        high_vol_call.price, high_vol
    );

    assert!(
        high_vol_put.price > spot * 0.3,
        "High vol put too cheap: {:.4} with σ={}",
        high_vol_put.price, high_vol
    );

    // Greeks should be well-behaved
    assert!(high_vol_call.delta >= 0.0 && high_vol_call.delta <= 1.0, "Delta out of bounds at high vol");
    assert!(high_vol_call.gamma >= 0.0, "Negative gamma at high vol");
    assert!(high_vol_call.vega >= 0.0, "Negative vega at high vol");
}

#[test]
fn test_negative_interest_rates() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = -0.02; // Negative 2%
    let time = 0.25;
    let vol = 0.2;

    let call = bs_call(spot, strike, rate, time, vol);
    let put = bs_put(spot, strike, rate, time, vol);

    // Prices should still be positive
    assert!(call.price > 0.0, "Negative call price with negative rates");
    assert!(put.price > 0.0, "Negative put price with negative rates");

    // Put-call parity should still hold
    let pcp_left = call.price - put.price;
    let pcp_right = spot - strike * (-rate * time).exp();

    assert!(
        (pcp_left - pcp_right).abs() < 1e-10,
        "Put-call parity fails with negative rates"
    );
}

#[test]
fn test_very_long_time_to_expiry() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 10.0; // 10 years
    let vol = 0.2;

    let call = bs_call(spot, strike, rate, time, vol);
    let _put = bs_put(spot, strike, rate, time, vol);

    // Very long-dated options should have significant time value
    assert!(
        call.price > spot * 0.4,
        "Long-dated call too cheap: {:.4}",
        call.price
    );

    // Call should approach spot price for very long times with positive rates
    assert!(
        call.price < spot * 1.1,
        "Long-dated call too expensive: {:.4}",
        call.price
    );

    // Greeks should be reasonable
    assert!(call.delta > 0.0 && call.delta < 1.0, "Long-dated ATM delta should be a valid probability weight");
    assert!(call.theta < 0.0, "Long-dated options should still have time decay");
}

#[test]
fn test_heston_extreme_parameters() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    // High vol-of-vol scenario
    let high_volvol_params = HestonParams {
        s0: spot,
        v0: 0.04,
        theta: 0.04,
        kappa: 2.0,
        sigma: 2.0,
        rho: -0.7,
        r: rate,
        t: time,
    };

    let heston_price = heston_call_carr_madan(spot, strike, time, rate, &high_volvol_params);

    assert!(heston_price.is_finite(), "Heston pricing failed with extreme vol-of-vol");
    assert!(heston_price > 0.0, "Heston price should be positive");
    assert!(heston_price < spot * 2.0, "Heston price seems too high: {:.4}", heston_price);
}

#[test]
fn test_optimizer_extreme_functions() {
    // Test Nelder-Mead with pathological functions

    let cfg = NelderMeadConfig::default();
    let optimizer = NelderMead::new(cfg.clone());

    // Rosenbrock function (banana function) - notoriously difficult
    let rosenbrock = |x: &[f64]| {
        let a = 1.0;
        let b = 100.0;
        b * (x[1] - x[0] * x[0]).powi(2) + (a - x[0]).powi(2)
    };

    let result = optimizer.minimize(&rosenbrock, vec![-1.0, 1.0]);

    assert!(result.converged, "Optimizer should handle Rosenbrock function");
    assert!(result.best_value.is_finite());

    // Very flat function (numerically challenging)
    let flat_function = |x: &[f64]| x[0] * x[0] * 1e-10 + x[1] * x[1] * 1e-10;

    let flat_optimizer = NelderMead::new(cfg);
    let flat_result = flat_optimizer.minimize(&flat_function, vec![100.0, 200.0]);

    // Should converge even with very flat functions
    assert!(flat_result.converged, "Optimizer should handle flat functions");
}

#[test]
fn test_numerical_precision_limits() {
    // Test behavior near machine precision limits
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;

    // Very small time to expiry (but not zero)
    let tiny_time = 1e-8;
    let tiny_call = bs_call(spot, strike, rate, tiny_time, vol).price;

    // Should not panic or return NaN
    assert!(tiny_call.is_finite(), "Tiny time call should be finite");
    assert!(tiny_call >= 0.0, "Tiny time call should be non-negative");

    // Very large spot price
    let huge_spot = 1e6;
    let huge_call = bs_call(huge_spot, strike, rate, 0.25, vol).price;

    assert!(huge_call.is_finite(), "Huge spot call should be finite");
    assert!(huge_call > huge_spot - strike, "Huge spot call should be approximately intrinsic");
}

#[test] 
fn test_missing_data_handling() {
    // This would test CSV loading with missing/corrupted files
    // For now, just test that our functions handle edge cases gracefully
    
    use std::f64;
    
    // Test with NaN inputs (should not panic)
    let nan_result = bs_call(f64::NAN, 100.0, 0.05, 0.25, 0.2).price;
    assert!(nan_result.is_nan() || nan_result.is_infinite(), "NaN input should produce NaN/inf");

    // Test with infinite inputs
    let inf_result = bs_call(f64::INFINITY, 100.0, 0.05, 0.25, 0.2).price;
    assert!(inf_result.is_infinite() || inf_result.is_nan(), "Inf input should produce inf/NaN");
}
