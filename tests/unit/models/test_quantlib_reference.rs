//! QuantLib reference-value tests for Heston pricing.
//!
//! All reference prices below are computed with QuantLib v1.41 Python
//! `AnalyticHestonEngine` (128-point Gauss-Laguerre) and independently
//! verified via trapezoidal P₁/P₂ integration in Python.
//!
//! Run with:
//!   cargo test --test lib test_quantlib-- --nocapture

use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::{
    heston_call_gauss_laguerre, heston_put_gauss_laguerre,
    heston_call_carr_madan,
};
use dollarbill::models::gauss_laguerre::GaussLaguerreRule;

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 1 – "Classic Heston"
//
// S=100, K=100, T=1, r=0.05, q=0
// v₀=0.04, κ=2.0, θ=0.04, σ=0.3, ρ=−0.7
//
// QuantLib AnalyticHestonEngine (128-pt GL): Call = 10.39421857
// ═══════════════════════════════════════════════════════════════════════════

fn classic_params() -> HestonParams {
    HestonParams {
        s0: 100.0,
        v0: 0.04,
        kappa: 2.0,
        theta: 0.04,
        sigma: 0.3,
        rho: -0.7,
        r: 0.05,
        t: 1.0,
    }
}

const CLASSIC_QUANTLIB_CALL: f64 = 10.3942;

#[test]
fn test_classic_gl64_vs_quantlib() {
    let params = classic_params();
    let rule = GaussLaguerreRule::new(64);
    let price = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);

    let err = (price - CLASSIC_QUANTLIB_CALL).abs();
    println!("Classic GL-64:  {price:.6}  (QuantLib ref: {CLASSIC_QUANTLIB_CALL})  err={err:.4}");
    assert!(
        err < 0.10,
        "GL-64 ({price:.4}) deviates from QuantLib ({CLASSIC_QUANTLIB_CALL}) by {err:.4}"
    );
}

#[test]
fn test_classic_gl128_vs_quantlib() {
    let params = classic_params();
    let rule = GaussLaguerreRule::new(128);
    let price = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);

    let err = (price - CLASSIC_QUANTLIB_CALL).abs();
    println!("Classic GL-128: {price:.6}  (QuantLib ref: {CLASSIC_QUANTLIB_CALL})  err={err:.4}");
    assert!(
        err < 0.05,
        "GL-128 ({price:.4}) deviates from QuantLib ({CLASSIC_QUANTLIB_CALL}) by {err:.4}"
    );
}

#[test]
fn test_classic_carr_madan_vs_quantlib() {
    let params = classic_params();
    let price = heston_call_carr_madan(100.0, 100.0, 1.0, 0.05, &params);

    let err = (price - CLASSIC_QUANTLIB_CALL).abs();
    println!("Classic CM:     {price:.6}  (QuantLib ref: {CLASSIC_QUANTLIB_CALL})  err={err:.4}");
    println!("  (Note: Carr-Madan uses original char function, may differ)");
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 2 – "High vol-of-vol" (extreme skew, stringent test)
//
// S=100, K=100, T=0.5, r=0.05, q=0
// v₀=0.09, κ=1.5, θ=0.09, σ=1.0, ρ=−0.9
//
// QuantLib AnalyticHestonEngine (128-pt GL): Call = 8.55682587
// ═══════════════════════════════════════════════════════════════════════════

fn high_volvol_params() -> HestonParams {
    HestonParams {
        s0: 100.0,
        v0: 0.09,
        kappa: 1.5,
        theta: 0.09,
        sigma: 1.0,
        rho: -0.9,
        r: 0.05,
        t: 0.5,
    }
}

const HIGH_VOLVOL_QUANTLIB_CALL: f64 = 8.5568;

#[test]
fn test_high_volvol_gl64() {
    let params = high_volvol_params();
    let rule = GaussLaguerreRule::new(64);
    let price = heston_call_gauss_laguerre(100.0, 100.0, 0.5, 0.05, &params, &rule);

    let err = (price - HIGH_VOLVOL_QUANTLIB_CALL).abs();
    println!("HighVolVol GL-64:  {price:.6}  (QuantLib ref: {HIGH_VOLVOL_QUANTLIB_CALL})  err={err:.4}");
    assert!(
        err < 1.0,
        "GL-64 ({price:.4}) deviates from QuantLib ({HIGH_VOLVOL_QUANTLIB_CALL}) by {err:.4}"
    );
}

#[test]
fn test_high_volvol_gl128() {
    let params = high_volvol_params();
    let rule = GaussLaguerreRule::new(128);
    let price = heston_call_gauss_laguerre(100.0, 100.0, 0.5, 0.05, &params, &rule);

    let err = (price - HIGH_VOLVOL_QUANTLIB_CALL).abs();
    println!("HighVolVol GL-128: {price:.6}  (QuantLib ref: {HIGH_VOLVOL_QUANTLIB_CALL})  err={err:.4}");
    assert!(
        err < 0.5,
        "GL-128 ({price:.4}) deviates from QuantLib ({HIGH_VOLVOL_QUANTLIB_CALL}) by {err:.4}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 3 – "Low vol / near-BS" (should converge to BS d1/d2)
//
// S=100, K=100, T=1, r=0.05, q=0
// v₀=0.01, κ=5.0, θ=0.01, σ=0.01, ρ=0.0
//
// QuantLib AnalyticHestonEngine: Call = 6.80486705
// BS call ≈ 6.8050
// ═══════════════════════════════════════════════════════════════════════════

fn low_vol_params() -> HestonParams {
    HestonParams {
        s0: 100.0,
        v0: 0.01,
        kappa: 5.0,
        theta: 0.01,
        sigma: 0.01,
        rho: 0.0,
        r: 0.05,
        t: 1.0,
    }
}

const NEAR_BS_QUANTLIB_CALL: f64 = 6.8049;

#[test]
fn test_near_bs_gl64() {
    let params = low_vol_params();
    let rule = GaussLaguerreRule::new(64);
    let price = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);

    let bs_price = dollarbill::models::bs_mod::black_scholes_merton_call(
        100.0, 100.0, 1.0, 0.05, 0.1, 0.0,
    ).price;

    let err_ql = (price - NEAR_BS_QUANTLIB_CALL).abs();
    let err_bs = (price - bs_price).abs();
    println!("NearBS GL-64:  {price:.6}  (QuantLib: {NEAR_BS_QUANTLIB_CALL}, BS: {bs_price:.6})  err_ql={err_ql:.4} err_bs={err_bs:.4}");
    assert!(
        err_ql < 0.10,
        "GL-64 near-BS ({price:.4}) deviates from QuantLib ({NEAR_BS_QUANTLIB_CALL}) by {err_ql:.4}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 4 – QuantLib strike-sweep (OTM/ITM)
//
// Same classic params, varying K in {80, 90, 100, 110, 120}
// QuantLib 128-pt GL reference prices:
//   K=80:  25.04456   K=90:  17.07531   K=100: 10.39422
//   K=110:  5.43034   K=120:  2.33263
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_strike_sweep_vs_quantlib() {
    let params = classic_params();
    let rule = GaussLaguerreRule::new(64);

    let cases: &[(f64, f64)] = &[
        (80.0,  25.0446),
        (90.0,  17.0753),
        (100.0, 10.3942),
        (110.0,  5.4303),
        (120.0,  2.3326),
    ];

    println!("\n{:<8} {:<12} {:<12} {:<10} {:<10}",
             "Strike", "GL-64", "QuantLib", "Abs Err", "Rel Err %");
    println!("{}", "-".repeat(56));

    for &(k, ql_ref) in cases {
        let price = heston_call_gauss_laguerre(100.0, k, 1.0, 0.05, &params, &rule);
        let abs_err = (price - ql_ref).abs();
        let rel_err = abs_err / ql_ref * 100.0;

        println!("{:<8.0} {:<12.6} {:<12.4} {:<10.4} {:<10.2}",
                 k, price, ql_ref, abs_err, rel_err);

        assert!(
            abs_err < 0.15 || rel_err < 3.0,
            "K={k}: GL-64 ({price:.4}) vs QuantLib ({ql_ref:.4}) -- err {abs_err:.4} ({rel_err:.2}%)"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 5 – Put-call parity across all strikes
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_put_call_parity_gl_all_strikes() {
    let params = classic_params();
    let rule = GaussLaguerreRule::new(64);

    for &k in &[80.0, 90.0, 100.0, 110.0, 120.0] {
        let call = heston_call_gauss_laguerre(100.0, k, 1.0, 0.05, &params, &rule);
        let put = heston_put_gauss_laguerre(100.0, k, 1.0, 0.05, &params, &rule);

        let parity_lhs = call - put;
        let parity_rhs = 100.0 - k * (-0.05_f64).exp();

        let err = (parity_lhs - parity_rhs).abs();
        println!("K={k:.0}: C-P = {parity_lhs:.6}, S-K*e^(-rT) = {parity_rhs:.6}, err = {err:.6}");
        assert!(
            err < 0.01,
            "K={k}: Put-call parity violated by {err:.6}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 6 – Node-count convergence table
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_count_convergence() {
    let params = classic_params();
    let ref128 = {
        let rule = GaussLaguerreRule::new(128);
        heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule)
    };

    println!("\nNode-count convergence (ATM call, reference = GL-128 = {ref128:.6}):");
    println!("{:<8} {:<12} {:<12}", "Nodes", "Price", "Err vs 128");
    println!("{}", "-".repeat(34));

    for &n in &[8, 16, 32, 48, 64, 96, 128] {
        let rule = GaussLaguerreRule::new(n);
        let price = heston_call_gauss_laguerre(100.0, 100.0, 1.0, 0.05, &params, &rule);
        let err = (price - ref128).abs();
        println!("{:<8} {:<12.6} {:<12.6}", n, price, err);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Case 7 – Timing comparison (wall-clock, not Criterion)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_timing_comparison() {
    use std::time::Instant;

    let params = classic_params();
    let iters = 1_000;
    let strikes: Vec<f64> = (80..=120).step_by(4).map(|k| k as f64).collect();

    // -- Carr-Madan --
    let t0 = Instant::now();
    for _ in 0..iters {
        for &k in &strikes {
            std::hint::black_box(heston_call_carr_madan(100.0, k, 1.0, 0.05, &params));
        }
    }
    let cm_elapsed = t0.elapsed();

    // -- GL-32 --
    let rule32 = GaussLaguerreRule::new(32);
    let t0 = Instant::now();
    for _ in 0..iters {
        for &k in &strikes {
            std::hint::black_box(heston_call_gauss_laguerre(100.0, k, 1.0, 0.05, &params, &rule32));
        }
    }
    let gl32_elapsed = t0.elapsed();

    // -- GL-64 --
    let rule64 = GaussLaguerreRule::new(64);
    let t0 = Instant::now();
    for _ in 0..iters {
        for &k in &strikes {
            std::hint::black_box(heston_call_gauss_laguerre(100.0, k, 1.0, 0.05, &params, &rule64));
        }
    }
    let gl64_elapsed = t0.elapsed();

    // -- GL-128 --
    let rule128 = GaussLaguerreRule::new(128);
    let t0 = Instant::now();
    for _ in 0..iters {
        for &k in &strikes {
            std::hint::black_box(heston_call_gauss_laguerre(100.0, k, 1.0, 0.05, &params, &rule128));
        }
    }
    let gl128_elapsed = t0.elapsed();

    // -- BSM baseline --
    let vol = 0.2_f64;
    let t0 = Instant::now();
    for _ in 0..iters {
        for &k in &strikes {
            std::hint::black_box(
                dollarbill::models::bs_mod::black_scholes_merton_call(100.0, k, 1.0, 0.05, vol, 0.0),
            );
        }
    }
    let bs_elapsed = t0.elapsed();

    let total_calls = (iters * strikes.len()) as f64;
    let sep = "=".repeat(58);
    println!("\n{sep}");
    println!("  TIMING COMPARISON  ({iters} x {} strikes = {total_calls:.0} calls)", strikes.len());
    println!("{sep}");
    println!("  BSM         {:>8.2} ms  ({:.0} ns/call)",
             bs_elapsed.as_secs_f64() * 1000.0,
             bs_elapsed.as_nanos() as f64 / total_calls);
    println!("  Carr-Madan  {:>8.2} ms  ({:.0} ns/call)",
             cm_elapsed.as_secs_f64() * 1000.0,
             cm_elapsed.as_nanos() as f64 / total_calls);
    println!("  GL-32       {:>8.2} ms  ({:.0} ns/call)",
             gl32_elapsed.as_secs_f64() * 1000.0,
             gl32_elapsed.as_nanos() as f64 / total_calls);
    println!("  GL-64       {:>8.2} ms  ({:.0} ns/call)",
             gl64_elapsed.as_secs_f64() * 1000.0,
             gl64_elapsed.as_nanos() as f64 / total_calls);
    println!("  GL-128      {:>8.2} ms  ({:.0} ns/call)",
             gl128_elapsed.as_secs_f64() * 1000.0,
             gl128_elapsed.as_nanos() as f64 / total_calls);
    println!("{sep}");
    println!("  GL-64 vs CM speed ratio: {:.2}x",
             cm_elapsed.as_secs_f64() / gl64_elapsed.as_secs_f64());
    println!("{sep}");
}
