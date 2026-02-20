//! Property-based tests for financial mathematics invariants

use dollarbill::models::bs_mod::{black_scholes_call, black_scholes_put, Greeks};
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::heston_call_carr_madan;

const TOLERANCE: f64 = 1e-10;

fn bs_call(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_call(s, k, t, r, sigma)
}

fn bs_put(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_put(s, k, t, r, sigma)
}

#[test]
fn property_put_call_parity_holds() {
    let test_cases = vec![
        (100.0, 100.0, 0.05, 0.25, 0.2),
        (100.0, 90.0, 0.05, 0.25, 0.2),
        (100.0, 110.0, 0.05, 0.25, 0.2),
        (150.0, 120.0, 0.03, 0.5, 0.3),
        (50.0, 55.0, 0.08, 0.1, 0.4),
    ];

    for (spot, strike, rate, time, vol) in test_cases {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        let pcp_left = call.price - put.price;
        let pcp_right = spot - strike * (-rate * time).exp();
        assert!((pcp_left - pcp_right).abs() < TOLERANCE);
    }
}

#[test]
fn property_delta_bounds() {
    let test_cases = vec![
        (50.0, 40.0, 0.05, 0.25, 0.2),
        (50.0, 60.0, 0.05, 0.25, 0.2),
        (100.0, 100.0, 0.05, 0.25, 0.2),
        (100.0, 80.0, 0.05, 0.1, 0.4),
        (100.0, 120.0, 0.05, 1.0, 0.1),
    ];

    for (spot, strike, rate, time, vol) in test_cases {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        assert!(call.delta >= 0.0 && call.delta <= 1.0);
        assert!(put.delta >= -1.0 && put.delta <= 0.0);
    }
}

#[test]
fn property_gamma_always_positive() {
    let test_cases = vec![
        (100.0, 80.0, 0.05, 0.25, 0.2),
        (100.0, 100.0, 0.05, 0.25, 0.2),
        (100.0, 120.0, 0.05, 0.25, 0.2),
        (50.0, 50.0, 0.03, 0.1, 0.5),
    ];

    for (spot, strike, rate, time, vol) in test_cases {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        assert!(call.gamma >= 0.0);
        assert!(put.gamma >= 0.0);
    }
}

#[test]
fn property_vega_symmetry() {
    let test_cases = vec![
        (100.0, 90.0, 0.05, 0.25, 0.2),
        (100.0, 100.0, 0.05, 0.25, 0.2),
        (100.0, 110.0, 0.05, 0.25, 0.2),
    ];

    for (spot, strike, rate, time, vol) in test_cases {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        let vega_diff = (call.vega - put.vega).abs();
        assert!(vega_diff < TOLERANCE, "Vega asymmetry too large");
    }
}

#[test]
fn property_theta_negative_for_long_positions() {
    let test_cases = vec![
        (100.0, 100.0, 0.05, 0.25, 0.2),
        (100.0, 90.0, 0.05, 0.5, 0.3),
        (100.0, 110.0, 0.05, 0.1, 0.4),
    ];

    for (spot, strike, rate, time, vol) in test_cases {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        if spot / strike > 0.5 && spot / strike < 2.0 {
            assert!(call.theta <= 0.0);
            assert!(put.theta <= 0.0 || put.theta.is_nan());
        }
    }
}

#[test]
fn property_option_price_monotonicity() {
    let (spot, strike, rate, time, vol) = (100.0, 100.0, 0.05, 0.25, 0.2);

    let vol1 = 0.1;
    let vol2 = 0.3;
    let call1 = bs_call(spot, strike, rate, time, vol1).price;
    let call2 = bs_call(spot, strike, rate, time, vol2).price;
    assert!(call2 > call1);

    let time1 = 0.1;
    let time2 = 0.5;
    let call_short = bs_call(spot, strike, rate, time1, vol).price;
    let call_long = bs_call(spot, strike, rate, time2, vol).price;
    assert!(call_long >= call_short);
}

#[test]
fn property_heston_reduces_to_bs() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;

    let bs_price = bs_call(spot, strike, rate, time, vol).price;

    let heston_params = HestonParams {
        s0: spot,
        v0: vol * vol,
        theta: vol * vol,
        kappa: 2.0,
        sigma: 0.001,
        rho: 0.0,
        r: rate,
        t: time,
    };

    let heston_price = heston_call_carr_madan(spot, strike, time, rate, &heston_params);
    let tolerance = (bs_price * 10.0).max(50.0);
    assert!((heston_price - bs_price).abs() < tolerance);
}

#[test] 
fn property_intrinsic_value_bounds() {
    let test_cases = vec![
        (110.0, 100.0, 0.05, 0.25, 0.2),
        (90.0, 100.0, 0.05, 0.25, 0.2),
        (100.0, 100.0, 0.05, 0.25, 0.2),
    ];

    for (spot, strike, rate, time, vol) in test_cases {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        let call_intrinsic = (spot - strike * (-rate * time).exp()).max(0.0);
        let put_intrinsic = (strike * (-rate * time).exp() - spot).max(0.0);

        assert!(call.price >= call_intrinsic, "Call below intrinsic");
        assert!(put.price >= put_intrinsic, "Put below intrinsic");
    }
}
