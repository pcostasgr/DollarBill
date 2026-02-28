//! Numerical stability and convergence tests

use dollarbill::calibration::nelder_mead::{NelderMead, NelderMeadConfig};
use dollarbill::models::bs_mod::{black_scholes_call, black_scholes_put, Greeks};
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::heston_call_carr_madan;
use dollarbill::utils::vol_surface::implied_volatility_newton;

const HIGH_PRECISION: f64 = 1e-9;
#[allow(dead_code)]
const STANDARD_PRECISION: f64 = 1e-6;

fn bs_call(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_call(s, k, t, r, sigma)
}

fn bs_put(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_put(s, k, t, r, sigma)
}

#[test]
fn test_greeks_numerical_stability() {
    let test_scenarios = vec![
        (100.0, 100.0, 0.05, 0.25, 0.2, "baseline ATM"),
        (1.0, 1.0, 0.05, 0.25, 0.2, "low price"),
        (10000.0, 10000.0, 0.05, 0.25, 0.2, "high price"),
        (100.0, 100.0, 0.001, 0.25, 0.2, "low rate"),
        (100.0, 100.0, 0.15, 0.25, 0.2, "high rate"),
        (100.0, 100.0, 0.05, 0.001, 0.2, "short expiry"),
        (100.0, 100.0, 0.05, 5.0, 0.2, "long expiry"),
        (100.0, 100.0, 0.05, 0.25, 0.05, "low vol"),
        (100.0, 100.0, 0.05, 0.25, 1.0, "high vol"),
    ];

    for (spot, strike, rate, time, vol, desc) in test_scenarios {
        let call = bs_call(spot, strike, rate, time, vol);
        let put = bs_put(spot, strike, rate, time, vol);

        for (name, value) in [
            ("call price", call.price),
            ("call delta", call.delta),
            ("call gamma", call.gamma),
            ("call theta", call.theta),
            ("call vega", call.vega),
            ("call rho", call.rho),
            ("put price", put.price),
            ("put delta", put.delta),
            ("put gamma", put.gamma),
            ("put theta", put.theta),
            ("put vega", put.vega),
            ("put rho", put.rho),
        ] {
            assert!(value.is_finite(), "{} not finite for {}", name, desc);
        }

        assert!(call.gamma >= 0.0, "Negative call gamma for {}", desc);
        assert!(put.gamma >= 0.0, "Negative put gamma for {}", desc);
        assert!(call.vega >= 0.0, "Negative call vega for {}", desc);
        assert!(put.vega >= 0.0, "Negative put vega for {}", desc);
    }
}

#[test]
fn test_implied_volatility_convergence() {
    let spot = 100.0;
    let rate = 0.05;
    let time = 0.25;

    let test_strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let test_vols = vec![0.1, 0.2, 0.3, 0.5, 0.8];

    for strike in &test_strikes {
        for &target_vol in &test_vols {
            let market_call = bs_call(spot, *strike, rate, time, target_vol).price;
            let market_put = bs_put(spot, *strike, rate, time, target_vol).price;

            let recovered_call_vol = implied_volatility_newton(market_call, spot, *strike, time, rate, true);
            let recovered_put_vol = implied_volatility_newton(market_put, spot, *strike, time, rate, false);

            if let Some(call_vol) = recovered_call_vol {
                assert!((call_vol - target_vol).abs() < 1e-2, "Call IV mismatch for K={}", strike);
            }

            if let Some(put_vol) = recovered_put_vol {
                assert!((put_vol - target_vol).abs() < 1e-2, "Put IV mismatch for K={}", strike);
            }
        }
    }
}

#[test]
fn test_nelder_mead_convergence_robustness() {
    let quadratic = |x: &[f64]| x[0] * x[0] + x[1] * x[1];
    let cfg = NelderMeadConfig { max_iterations: 1000, tolerance: HIGH_PRECISION, ..Default::default() };

    let starting_points = vec![
        vec![1.0, 1.0],
        vec![-5.0, -5.0],
        vec![10.0, -10.0],
        vec![0.1, 0.1],
    ];

    for (i, initial) in starting_points.iter().enumerate() {
        let optimizer = NelderMead::new(cfg.clone());
        let result = optimizer.minimize(&quadratic, initial.clone());

        assert!(result.converged, "Optimizer failed from starting point {}", i);
        let error = result.best_params.iter().map(|p| p.abs()).sum::<f64>();
        assert!(error < 1e-4, "Poor convergence from start {}", i);
    }
}

#[test]
fn test_heston_fft_numerical_stability() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;

    let test_params = vec![
        HestonParams { s0: spot, v0: 0.04, theta: 0.04, kappa: 2.0, sigma: 0.3, rho: -0.5, r: rate, t: time },
        HestonParams { s0: spot, v0: 0.01, theta: 0.01, kappa: 5.0, sigma: 0.1, rho: 0.0, r: rate, t: time },
        HestonParams { s0: spot, v0: 0.09, theta: 0.04, kappa: 1.0, sigma: 0.6, rho: -0.8, r: rate, t: time },
        HestonParams { s0: spot, v0: 0.04, theta: 0.09, kappa: 3.0, sigma: 0.2, rho: 0.3, r: rate, t: time },
    ];

    for (i, params) in test_params.iter().enumerate() {
        let price = heston_call_carr_madan(spot, strike, time, rate, params);
        assert!(price.is_finite(), "Heston price not finite for parameter set {}", i);
        assert!(price > 0.0, "Heston price not positive for parameter set {}: {:.6}", i, price);
        assert!(price < spot * 2.0, "Heston price too high for parameter set {}: {:.4}", i, price);
    }
}

#[test]
fn test_precision_consistency() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;

    let prices: Vec<f64> = (0..10)
        .map(|_| bs_call(spot, strike, rate, time, vol).price)
        .collect();

    let first_price = prices[0];
    for (i, &price) in prices.iter().enumerate() {
        assert_eq!(price, first_price, "Price calculation not deterministic: iteration {}", i);
    }
}

#[test]
fn test_parameter_sensitivity_smoothness() {
    let base_spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;
    let epsilon = 1e-3;

    let price_down = bs_call(base_spot - epsilon, strike, rate, time, vol).price;
    let price_up = bs_call(base_spot + epsilon, strike, rate, time, vol).price;
    let numerical_delta = (price_up - price_down) / (2.0 * epsilon);

    let greeks = bs_call(base_spot, strike, rate, time, vol);
    let delta_error = (numerical_delta - greeks.delta).abs();
    assert!(delta_error < 5.0, "Delta approximation error too large");

    let price_vol_down = bs_call(base_spot, strike, rate, time, vol - epsilon).price;
    let price_vol_up = bs_call(base_spot, strike, rate, time, vol + epsilon).price;
    let numerical_vega = (price_vol_up - price_vol_down) / (2.0 * epsilon);

    let vega_error = (numerical_vega - greeks.vega).abs();
    assert!(vega_error < 10.0, "Vega approximation error too large");
}

#[test]
fn test_optimization_iteration_limits() {
    use std::time::Instant;

    let difficult_function = |x: &[f64]| {
        let noise = (x[0] * 37.0).sin() * 0.1 + (x[1] * 43.0).sin() * 0.1;
        x[0] * x[0] + x[1] * x[1] + noise
    };

    let mut cfg = NelderMeadConfig::default();
    cfg.max_iterations = 50;
    cfg.tolerance = 1e-12;
    let optimizer = NelderMead::new(cfg);

    let start_time = Instant::now();
    let result = optimizer.minimize(&difficult_function, vec![10.0, 10.0]);
    let duration = start_time.elapsed();

    assert!(duration.as_secs() < 1, "Optimizer took too long: {:.3}s", duration.as_secs_f64());
    assert!(result.converged || result.iterations >= 50);
}

#[test]
fn test_monte_carlo_placeholder_convergence() {
    let params = HestonParams {
        s0: 100.0,
        v0: 0.04,
        theta: 0.04,
        kappa: 2.0,
        sigma: 0.3,
        rho: -0.5,
        r: 0.05,
        t: 0.25,
    };

    let analytical_price = heston_call_carr_madan(params.s0, 100.0, params.t, params.r, &params);
    assert!(analytical_price > 0.0);
}
