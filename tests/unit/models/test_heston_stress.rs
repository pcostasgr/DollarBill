//! Heston model stress tests — reflection boundary + Carr-Madan price floors.
//!
//! Proposal 1: "Feller condition & negative variance in Heston paths still can
//! produce garbage prices/Greeks.  Nuke with absorption/reflection + tests."
//! Proposal 3: "FFT Carr-Madan stability — negative prices/oscillations on
//! extremes still likely lurking."
//!
//! These tests catch:
//!   • Any variance that goes negative in a simulated path (regression for the
//!     `simulate_path_with_randoms` reflection fix)
//!   • Antithetic-variate prices that cannot be negative or wildly divergent
//!   • Carr-Madan prices that must satisfy the European call lower bound
//!     C >= max(0, S − K·e^{−rT}) on deep-ITM and extreme inputs

use dollarbill::models::heston::{HestonMonteCarlo, HestonParams, MonteCarloConfig};
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};

// ─── Shared fixtures ─────────────────────────────────────────────────────────

/// Borderline-Feller params: 2κθ = 0.08 > σ² = 0.0784 (just barely satisfied).
/// With a coarse time grid this is the most likely configuration to expose a
/// missing reflection because the Euler–Maruyama scheme can step the variance
/// negative in a single large noise draw.
fn borderline_feller_params() -> HestonParams {
    HestonParams {
        s0: 100.0,
        v0: 0.04,
        kappa: 1.0,
        theta: 0.04,
        sigma: 0.28,   // σ² = 0.0784, 2κθ = 0.08 — barely above Feller threshold
        rho: -0.5,
        r: 0.05,
        t: 1.0,
    }
}

/// ATM params with moderate vol-of-vol — used for Carr-Madan numerical tests.
fn atm_params() -> HestonParams {
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

// ─── 1. Reflection boundary in all path-simulation code paths ────────────────

/// ALL variance values in every simulated path must be ≥ 0 after the fix.
/// Before: `simulate_path` had reflection; `simulate_path_with_randoms` did not.
/// This test exercises the `simulate_paths()` code path (uses `simulate_path`).
#[test]
fn heston_mc_all_variances_nonneg_simulate_paths() {
    let params = borderline_feller_params();
    let config = MonteCarloConfig {
        n_paths: 500,
        n_steps: 20,   // coarse grid to stress the Euler scheme
        seed: 42,
        use_antithetic: false,
    };
    let mc = HestonMonteCarlo::new(params, config).expect("valid Heston params");
    let paths = mc.simulate_paths();

    for (path_idx, path) in paths.iter().enumerate() {
        for (step, &v) in path.variances.iter().enumerate() {
            assert!(
                v >= 0.0,
                "Path {} step {}: variance went negative ({:.6}) — reflection missing",
                path_idx, step, v
            );
        }
    }
}

/// Price a call with antithetic variates; result must be finite and positive.
/// Before the fix `simulate_path_with_randoms` could produce negative variances,
/// freezing the vol at 0 and systematically underpricing the option.
#[test]
fn heston_antithetic_call_price_positive_and_finite() {
    let params = borderline_feller_params();
    let config = MonteCarloConfig {
        n_paths: 1_000,
        n_steps: 20,
        seed: 123,
        use_antithetic: true,
    };
    let mc = HestonMonteCarlo::new(params, config).expect("valid Heston params");
    let price = mc.price_european_call(100.0);

    assert!(price.is_finite(), "Antithetic call price is not finite: {}", price);
    assert!(price > 0.0,       "Antithetic call price is non-positive: {}", price);
}

/// Antithetic and standard estimators must converge to the same value.
/// If `simulate_path_with_randoms` was still missing reflection, the antithetic
/// estimator would be biased low — causing a divergence > 30%.
#[test]
fn heston_antithetic_and_standard_prices_agree_within_30pct() {
    let params = borderline_feller_params();

    let mc_reg = HestonMonteCarlo::new(
        params.clone(),
        MonteCarloConfig { n_paths: 2_000, n_steps: 20, seed: 7, use_antithetic: false },
    ).expect("valid params");

    let mc_anti = HestonMonteCarlo::new(
        params,
        MonteCarloConfig { n_paths: 2_000, n_steps: 20, seed: 7, use_antithetic: true },
    ).expect("valid params");

    let price_reg  = mc_reg.price_european_call(100.0);
    let price_anti = mc_anti.price_european_call(100.0);

    let rel_diff = (price_reg - price_anti).abs() / price_reg.max(1e-8);
    assert!(
        rel_diff < 0.30,
        "Antithetic ({:.4}) and standard ({:.4}) MC prices diverge by {:.1}% — \
         variance reflection may still be broken",
        price_anti, price_reg, rel_diff * 100.0
    );
}

// ─── 2. Carr-Madan price lower bounds ─────────────────────────────────────────

/// Deep-ITM call: C must be >= intrinsic S − K·e^{−rT}.
/// Before the fix, high-frequency Fourier oscillations could return a value
/// below intrinsic for extreme deep-ITM inputs.
#[test]
fn carr_madan_deep_itm_call_geq_intrinsic() {
    let spot   = 180.0;
    let strike = 100.0;
    let rate   = 0.05;
    let t      = 1.0;

    let params = HestonParams { s0: spot, ..atm_params() };
    let price    = heston_call_carr_madan(spot, strike, t, rate, &params);
    let intrinsic = (spot - strike * (-rate * t).exp()).max(0.0);

    assert!(
        price >= intrinsic - 1e-6,  // 1 µ$ tolerance for float rounding
        "Deep-ITM Carr-Madan call {:.4} < intrinsic {:.4}",
        price, intrinsic
    );
    assert!(price.is_finite(), "Deep-ITM Carr-Madan price is not finite: {}", price);
}

/// Deep-OTM call: K = 5×S — price must be ≥ 0 and finite.
#[test]
fn carr_madan_deep_otm_call_nonneg_and_finite() {
    let spot   = 100.0;
    let strike = 500.0;
    let rate   = 0.05;
    let t      = 0.5;

    let params = HestonParams { s0: spot, ..atm_params() };
    let price = heston_call_carr_madan(spot, strike, t, rate, &params);

    assert!(price.is_finite(), "Deep-OTM Carr-Madan price is not finite: {}", price);
    assert!(price >= 0.0,      "Deep-OTM Carr-Madan price is negative: {:.6}", price);
}

/// Put-call parity: |C − P − S + K·e^{−rT}| < 1 cent for ATM standard params.
#[test]
fn carr_madan_put_call_parity_atm() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let t      = 1.0;
    let params = HestonParams { s0: spot, ..atm_params() };

    let call = heston_call_carr_madan(spot, strike, t, rate, &params);
    let put  = heston_put_carr_madan (spot, strike, t, rate, &params);
    let pcp_rhs = spot - strike * (-rate * t).exp();
    let parity_error = (call - put - pcp_rhs).abs();

    assert!(
        parity_error < 0.01,
        "Put-call parity violated: C={:.4} P={:.4} error={:.6}",
        call, put, parity_error
    );
}

/// High vol-of-vol stress: σ = 0.40 with kappa raised to maintain Feller
/// (2·4·0.04 = 0.32 > 0.16 = 0.40²).  Price must be finite and positive.
#[test]
fn carr_madan_high_vol_of_vol_remains_stable() {
    let params = HestonParams {
        s0: 100.0,
        v0: 0.04,
        kappa: 4.0,   // 2*4.0*0.04 = 0.32 > 0.40² = 0.16 ✓ Feller satisfied
        theta: 0.04,
        sigma: 0.40,
        rho: -0.7,
        r: 0.05,
        t: 1.0,
    };
    let price = heston_call_carr_madan(100.0, 100.0, 1.0, 0.05, &params);

    assert!(price.is_finite(), "High-σ Carr-Madan price is not finite: {}", price);
    assert!(price > 0.0,       "High-σ Carr-Madan price is non-positive: {}", price);
}

/// Short-maturity (T = 0.02 ≈ 1 week) must not produce negative prices for any
/// strike — FFT oscillations are most severe at very short maturities.
#[test]
fn carr_madan_short_maturity_all_strikes_nonneg() {
    let params = HestonParams { s0: 100.0, t: 0.02, ..atm_params() };
    for &k in &[80.0_f64, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0] {
        let price = heston_call_carr_madan(100.0, k, 0.02, 0.05, &params);
        assert!(
            price >= 0.0 && price.is_finite(),
            "Short-maturity Carr-Madan gave bad price {:.6} for K={}", price, k
        );
        let intrinsic = (100.0_f64 - k * (-0.05_f64 * 0.02).exp()).max(0.0);
        assert!(
            price >= intrinsic - 1e-6,
            "Short-maturity Carr-Madan {:.4} < intrinsic {:.4} at K={}", price, intrinsic, k
        );
    }
}
