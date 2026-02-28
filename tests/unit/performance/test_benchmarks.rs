//! Performance smoke tests (lightweight to keep CI fast)

use std::time::Instant;
use dollarbill::calibration::nelder_mead::{NelderMead, NelderMeadConfig};
use dollarbill::models::bs_mod::{black_scholes_call, Greeks};
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::heston_call_carr_madan;

fn bs_call(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_call(s, k, t, r, sigma)
}

#[test]
fn bench_black_scholes_pricing_speed() {
    let iterations = 1000;
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = bs_call(spot, strike, rate, time, vol).price;
    }
    let duration = start.elapsed();
    let avg_time_us = duration.as_micros() / iterations;
    assert!(avg_time_us < 500, "BS pricing too slow: {} μs", avg_time_us);
}

#[test]
fn bench_heston_pricing_speed() {
    let iterations = 20;
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let params = HestonParams { s0: spot, v0: 0.04, theta: 0.04, kappa: 2.0, sigma: 0.3, rho: -0.5, r: rate, t: time };

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = heston_call_carr_madan(spot, strike, time, rate, &params);
    }
    let duration = start.elapsed();
    let avg_time_ms = duration.as_millis() / iterations as u128;
    assert!(avg_time_ms < 200, "Heston pricing too slow: {} ms", avg_time_ms);
}

#[test]
fn bench_nelder_mead_optimization_speed() {
    let iterations = 5;
    let quadratic = |x: &[f64]| (x[0] - 2.0).powi(2) + (x[1] - 3.0).powi(2);
    let optimizer = NelderMead::new(NelderMeadConfig::default());

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = optimizer.minimize(&quadratic, vec![0.0, 0.0]);
    }
    let duration = start.elapsed();
    let avg_time_ms = duration.as_millis() / iterations as u128;
    assert!(avg_time_ms < 200, "Nelder-Mead too slow: {} ms", avg_time_ms);
}
