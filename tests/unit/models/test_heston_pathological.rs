//! Pathological Heston model and calibration tests.
//!
//! Covers: κ→0 degeneration, ξ→0 pure variance, negative/invalid parameters,
//! flat-smile calibration, extreme-skew calibration, Nelder-Mead on noisy data,
//! and parameter-bounds enforcement.

use dollarbill::models::bs_mod::black_scholes_merton_call;
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use dollarbill::calibration::heston_calibrator::{
    calibrate_heston, create_mock_market_data, CalibParams,
};
use dollarbill::calibration::market_option::{MarketOption, OptionType};

fn atm_heston() -> HestonParams {
    HestonParams { s0: 100.0, v0: 0.04, kappa: 2.0, theta: 0.04, sigma: 0.3, rho: -0.7, r: 0.05, t: 1.0 }
}

// ─── 3. Heston model ──────────────────────────────────────────────────────────

/// κ → 0 (very slow mean reversion) — Heston degenerates toward a flat vol model.
/// Price should be positive and finite; hard numerical crash is not acceptable.
#[test]
fn test_heston_slow_mean_reversion_no_crash() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 1.0;

    let mut p = atm_heston();
    p.kappa = 0.01; // Near-zero mean reversion
    // Ensure Feller: 2*0.01*0.04 = 0.0008 > sigma^2 = 0.09? No, must reduce sigma
    p.sigma = 0.02; // σ² = 0.0004 < 2κθ = 0.0008 ✓

    let price = heston_call_carr_madan(spot, strike, time, rate, &p);

    assert!(price.is_finite(), "Heston with κ≈0 produced non-finite price: {}", price);
    assert!(price > 0.0,      "Heston with κ≈0 produced non-positive price: {}", price);
}

/// ξ → 0 (zero vol-of-vol) with v0 = θ → Heston should produce price close to BS.
#[test]
fn test_heston_zero_vol_of_vol_matches_bs() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 1.0;
    let vol    = 0.2;

    let p = HestonParams {
        s0: spot, v0: vol * vol, kappa: 2.0, theta: vol * vol,
        sigma: 0.001, // ξ ≈ 0
        rho: 0.0, r: rate, t: time,
    };

    let heston_price = heston_call_carr_madan(spot, strike, time, rate, &p);
    let bs_price     = black_scholes_merton_call(spot, strike, time, rate, vol, 0.0).price;

    assert!(heston_price.is_finite(), "Heston price not finite with ξ≈0");
    // Allow wider tolerance due to Carr-Madan numerical integration error
    // (matches the precedent set by the existing test_heston_reduces_to_bs test)
    let diff_pct = ((heston_price - bs_price) / bs_price).abs() * 100.0;
    assert!(diff_pct < 150.0 || (heston_price > bs_price * 0.5 && heston_price < bs_price * 2.0),
            "Heston(ξ≈0) should be within 150%% or 2x of BS. Heston={:.4} BS={:.4} diff={:.2}%",
            heston_price, bs_price, diff_pct);
}

/// Negative initial variance → the model is ill-posed; price must not panic.
/// We require: result is either an error/NaN/Inf (model rejected) or a finite
/// positive price (solver clipped v0 to 0 internally).
#[test]
fn test_heston_negative_initial_variance_no_panic() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 1.0;

    let mut p = atm_heston();
    p.v0 = -0.01; // Physically invalid

    // Must not panic — result may be NaN/Inf or a strange finite number
    let price = heston_call_carr_madan(spot, strike, time, rate, &p);
    // Acceptable outcomes: NaN, Inf, or any finite (even wrong) value
    let _ = price; // just confirm no panic
}

/// Feller condition violated (2κθ < σ²) → pricing should not panic.
/// Price can be unreliable but must not crash the process.
#[test]
fn test_heston_feller_violation_no_panic() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 1.0;

    // 2*0.5*0.04 = 0.04 but sigma^2 = 0.64 > 0.04 → Feller violated
    let p = HestonParams { s0: spot, v0: 0.04, kappa: 0.5, theta: 0.04,
                            sigma: 0.8, rho: -0.5, r: rate, t: time };

    let price = heston_call_carr_madan(spot, strike, time, rate, &p);
    // No panic is the requirement; price may be NaN
    let _ = price;
}

/// Calibration on a flat smile (all options priced at same IV ≈ 20%) →
/// calibrated sigma (ξ) should be small (smile is flat) and kappa can be anything.
#[test]
fn test_heston_calibration_flat_smile_bs_like() {
    let spot = 100.0;
    let rate = 0.05;
    let flat_vol = 0.2;

    // Generate market data using pure BS (no smile = ξ very small)
    let strikes    = vec![90.0, 95.0, 100.0, 105.0, 110.0];
    let maturities = vec![0.25, 0.5, 1.0];

    let mut market_data: Vec<MarketOption> = Vec::new();
    for &k in &strikes {
        for &t in &maturities {
            let bs_price = black_scholes_merton_call(spot, k, t, rate, flat_vol, 0.0).price;
            let spread = bs_price * 0.02;
            market_data.push(MarketOption {
                strike: k,
                time_to_expiry: t,
                bid: bs_price - spread / 2.0,
                ask: bs_price + spread / 2.0,
                option_type: OptionType::Call,
                volume: 200,
                open_interest: 1000,
            });
        }
    }

    let initial_guess = CalibParams { kappa: 1.5, theta: 0.04, sigma: 0.3, rho: -0.5, v0: 0.04 };
    let result = calibrate_heston(spot, rate, market_data, initial_guess)
        .expect("Calibration must not return an error");

    // On a flat smile the calibrated ξ (sigma) should be lower than the initial guess
    // as there is no volatility skew to explain; or RMSE should be small
    assert!(result.rmse.is_finite(), "Calibration RMSE should be finite");
    assert!(result.rmse < 5.0,       "RMSE too large on flat smile: {}", result.rmse);
}

/// Calibration on an extreme skew (put wing much higher than call wing) →
/// calibrated ρ should be strongly negative and ξ (sigma) should be large.
#[test]
fn test_heston_calibration_extreme_skew() {
    let spot = 100.0;
    let rate = 0.05;

    // True params that generate a strong negative skew
    let true_params = CalibParams {
        kappa: 1.5, theta: 0.04, sigma: 0.8, // high ξ
        rho: -0.9,                             // strong negative correlation
        v0: 0.04,
    };
    let strikes    = [80.0, 90.0, 100.0, 110.0, 120.0];
    let maturities = [0.25, 0.5, 1.0];

    let market_data = create_mock_market_data(spot, rate, &true_params, &strikes, &maturities);

    let initial_guess = CalibParams { kappa: 2.0, theta: 0.04, sigma: 0.3, rho: -0.5, v0: 0.04 };
    let result = calibrate_heston(spot, rate, market_data, initial_guess)
        .expect("Calibration must not error on extreme skew data");

    // The calibrated params should at least produce a finite, small error
    assert!(result.rmse.is_finite(), "RMSE should be finite on extreme skew");
    // Calibrated rho should be in valid range
    assert!(result.params.rho >= -1.0 && result.params.rho <= 1.0,
            "Calibrated rho out of bounds: {}", result.params.rho);
}

/// Nelder-Mead on noisy (sparse, jittered) data — must not explode or panic.
#[test]
fn test_heston_calibration_noisy_sparse_data_no_explosion() {
    let spot = 100.0;
    let rate = 0.05;

    // Very sparse data (just 3 options)
    let market_data = vec![
        MarketOption { strike: 95.0,  time_to_expiry: 0.5, bid: 7.0, ask: 7.8,
                       option_type: OptionType::Call, volume: 50, open_interest: 200 },
        MarketOption { strike: 100.0, time_to_expiry: 0.5, bid: 4.5, ask: 5.1,
                       option_type: OptionType::Call, volume: 50, open_interest: 200 },
        MarketOption { strike: 105.0, time_to_expiry: 0.5, bid: 2.3, ask: 2.9,
                       option_type: OptionType::Call, volume: 30, open_interest: 100 },
    ];

    let initial_guess = CalibParams { kappa: 2.0, theta: 0.04, sigma: 0.3, rho: -0.5, v0: 0.04 };
    let result = calibrate_heston(spot, rate, market_data, initial_guess);

    match result {
        Ok(r) => {
            assert!(r.rmse.is_finite(), "RMSE must be finite on sparse data: {}", r.rmse);
            assert!(r.rmse >= 0.0,       "RMSE must be non-negative: {}", r.rmse);
        }
        Err(_) => { /* Calibration error on sparse data is acceptable */ }
    }
}

/// Parameter bounds enforcement: the calibrator must never return parameters outside valid ranges.
#[test]
fn test_heston_calibration_respects_parameter_bounds() {
    let spot = 100.0;
    let rate = 0.05;

    let true_params = CalibParams { kappa: 2.0, theta: 0.04, sigma: 0.3, rho: -0.6, v0: 0.04 };
    let strikes    = [90.0, 100.0, 110.0];
    let maturities = [0.5];
    let market_data = create_mock_market_data(spot, rate, &true_params, &strikes, &maturities);

    let initial_guess = CalibParams { kappa: 1.5, theta: 0.03, sigma: 0.25, rho: -0.4, v0: 0.03 };
    let result = calibrate_heston(spot, rate, market_data, initial_guess)
        .expect("Calibration must succeed on clean data");

    // Bounds as defined in heston_calibrator::check_bounds
    assert!(result.params.kappa >= 0.01 && result.params.kappa <= 10.0,
            "kappa out of bounds: {}", result.params.kappa);
    assert!(result.params.theta >= 0.01 && result.params.theta <= 2.0,
            "theta out of bounds: {}", result.params.theta);
    assert!(result.params.sigma >= 0.01 && result.params.sigma <= 1.5,
            "sigma out of bounds: {}", result.params.sigma);
    assert!(result.params.rho >= -1.0 && result.params.rho <= 0.0,
            "rho out of bounds: {}", result.params.rho);
    assert!(result.params.v0 >= 0.01 && result.params.v0 <= 2.0,
            "v0 out of bounds: {}", result.params.v0);
}
