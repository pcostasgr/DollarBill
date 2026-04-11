//! Pricing engine kill-criterion validation suite.
//!
//! Every assertion here must pass before the repo can claim QuantLib parity.
//! py/validate_pricing.py cross-checks the same numbers against QuantLib Python.
//!
//! | Kill criterion | Bound                        | Test                                          |
//! |----------------|------------------------------|-----------------------------------------------|
//! | BSM accuracy   | PCP error < $0.001 on 10 000 random options | `bsm_put_call_parity_10k_random`  |
//! | Greeks         | delta rel. error < 0.5 % vs FD | `bsm_delta_vs_finite_difference`            |
//! | Batch speed    | 50 × 10 Heston prices < 1.5 ms (release) | `heston_batch_50x10_under_1500us`  |
//! | Heston fit     | mean |ΔIV| < 0.8 % on TSLA crash surface | `heston_on_tesla_crash_period`   |
//!
//! **Note on Nelder-Mead**: the original custom simplex was deleted and replaced
//! with CMA-ES (`src/calibration/cmaes.rs`). The assert message in
//! `heston_on_tesla_crash_period` records what Nelder-Mead used to produce so
//! future regressions are immediately obvious.

use dollarbill::calibration::heston_calibrator::{calibrate_heston, CalibParams, create_mock_market_data};
use dollarbill::calibration::market_option::OptionType;
use dollarbill::market_data::csv_loader::load_csv_closes;
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
#[cfg(not(debug_assertions))]
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
#[cfg(not(debug_assertions))]
use std::time::Instant;

// ─── Constants ────────────────────────────────────────────────────────────────

const TSLA_CSV:   &str = "data/tesla_one_year.csv";
const CRASH_START: &str = "2025-02-01";
const CRASH_END:   &str = "2025-03-31";
/// Approximate 2025 US risk-free rate.
const R: f64 = 0.045;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Invert BSM price to an implied vol via 120-step bisection.
///
/// Returns `None` when the price is below intrinsic (arbitrage-free lower
/// bound) or any input is degenerate.
fn bsm_iv(market_price: f64, s: f64, k: f64, t: f64, r: f64, is_call: bool) -> Option<f64> {
    if market_price <= 0.0 || t <= 0.0 || s <= 0.0 || k <= 0.0 {
        return None;
    }
    let discount = (-r * t).exp();
    let intrinsic = if is_call {
        (s - k * discount).max(0.0)
    } else {
        (k * discount - s).max(0.0)
    };
    // Allow a tiny tolerance below intrinsic for numerical noise in the pricer
    if market_price < intrinsic - 1e-6 {
        return None;
    }
    let mut lo = 1e-6_f64;
    let mut hi = 10.0_f64;
    for _ in 0..120 {
        let mid = (lo + hi) * 0.5;
        let price = if is_call {
            black_scholes_merton_call(s, k, t, r, mid, 0.0).price
        } else {
            black_scholes_merton_put(s, k, t, r, mid, 0.0).price
        };
        if price < market_price {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let iv = (lo + hi) * 0.5;
    if iv.is_finite() { Some(iv) } else { None }
}

/// Compute mean |IV_model – IV_market| across a synthetic options surface.
///
/// Each option's "market" IV is inverted from its mid price; the Heston model
/// with `params` is used to compute the model price, which is then also inverted
/// to an IV.  Only entries where both inversions succeed are counted.
fn mean_abs_iv_error(surface: &[dollarbill::calibration::market_option::MarketOption],
                     spot: f64,
                     params: &CalibParams) -> f64 {
    let mut sum   = 0.0_f64;
    let mut count = 0_usize;
    for opt in surface {
        let is_call = opt.option_type == OptionType::Call;
        let h = params.to_heston(spot, R, opt.time_to_expiry);
        let model_price = if is_call {
            heston_call_carr_madan(spot, opt.strike, opt.time_to_expiry, R, &h)
        } else {
            heston_put_carr_madan(spot, opt.strike, opt.time_to_expiry, R, &h)
        };
        let iv_mkt = bsm_iv(opt.mid_price(), spot, opt.strike, opt.time_to_expiry, R, is_call);
        let iv_mod = bsm_iv(model_price,     spot, opt.strike, opt.time_to_expiry, R, is_call);
        if let (Some(vm), Some(vd)) = (iv_mkt, iv_mod) {
            if vm.is_finite() && vd.is_finite() && vm > 0.001 {
                sum   += (vm - vd).abs();
                count += 1;
            }
        }
    }
    if count == 0 { f64::INFINITY } else { sum / count as f64 }
}

/// Minimal LCG pseudo-RNG — deterministic, no external deps, reproducible.
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn f64(&mut self) -> f64 { (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 }
    fn range(&mut self, lo: f64, hi: f64) -> f64 { lo + self.f64() * (hi - lo) }
}

// ─── Kill criterion 1 ────────────────────────────────────────────────────────

/// BSM: put-call parity error < $0.001 on 10 000 pseudo-random options.
///
/// Uses:  C - P  =  S·e^{-qT} - K·e^{-rT}
///
/// A violation here means our BSM call or put pricer has a systematic bug
/// that would make every downstream delta-hedge, spread pricing, and risk
/// estimate wrong.
#[test]
fn bsm_put_call_parity_10k_random() {
    let mut rng = Lcg::new(0xdead_beef_cafe_babe);
    let mut max_err = 0.0_f64;
    let mut worst   = (0.0_f64, 0.0, 0.0, 0.0, 0.0);

    for _ in 0..10_000 {
        let s     = rng.range(10.0,  1_000.0);
        let k     = s * rng.range(0.70, 1.30);
        let t     = rng.range(0.02,  2.0);      // 1 week … 2 years
        let r     = rng.range(0.001, 0.10);
        let sigma = rng.range(0.05,  1.50);
        let q     = 0.0;

        let call = black_scholes_merton_call(s, k, t, r, sigma, q).price;
        let put  = black_scholes_merton_put( s, k, t, r, sigma, q).price;
        // PCP: C - P = S·e^{-qT} - K·e^{-rT}
        let err  = ((call - put) - (s * (-q * t).exp() - k * (-r * t).exp())).abs();
        if err > max_err {
            max_err = err;
            worst   = (s, k, t, r, sigma);
        }
    }

    assert!(
        max_err < 0.001,
        "BSM put-call parity violated: max error ${:.2e} ≥ $0.001  \
         (worst case S={:.1} K={:.1} T={:.3} r={:.3} σ={:.3})",
        max_err, worst.0, worst.1, worst.2, worst.3, worst.4
    );
}

// ─── Kill criterion 2 ────────────────────────────────────────────────────────

/// Greeks: BSM analytical delta must match central-finite-difference approximation
/// to within 0.5 % relative error across a representative set of inputs.
///
/// This serves as a proxy for "QuantLib FD" agreement since the BSM delta is
/// analytically tractable; any implementation bug shows up here.
#[test]
fn bsm_delta_vs_finite_difference() {
    // (S, K, T, r, sigma)
    let cases: &[(f64, f64, f64, f64, f64)] = &[
        (100.0, 100.0, 0.25, 0.05, 0.20),  // ATM standard
        (100.0, 110.0, 0.50, 0.05, 0.25),  // OTM
        (100.0,  90.0, 0.10, 0.03, 0.30),  // ITM short-dated
        (500.0, 520.0, 1.00, 0.045, 0.35), // high-price OTM
        (250.0, 240.0, 0.05, 0.04, 0.60),  // short-dated high-vol (crash-like)
        ( 50.0,  45.0, 0.25, 0.02, 1.00),  // extreme vol
        (200.0, 200.0, 2.00, 0.06, 0.15),  // long-dated ATM
    ];

    for &(s, k, t, r, sigma) in cases {
        let eps   = s * 1e-5;
        let fd_d  = (black_scholes_merton_call(s + eps, k, t, r, sigma, 0.0).price
                   - black_scholes_merton_call(s - eps, k, t, r, sigma, 0.0).price)
                  / (2.0 * eps);
        let anal_d = black_scholes_merton_call(s, k, t, r, sigma, 0.0).delta;

        let rel_err = if anal_d.abs() > 1e-6 {
            ((anal_d - fd_d) / anal_d).abs()
        } else {
            (anal_d - fd_d).abs()
        };

        assert!(
            rel_err < 0.005,
            "BSM delta rel. error {:.4}% ≥ 0.5% for \
             S={s:.1} K={k:.1} T={t:.2} r={r:.3} σ={sigma:.2}  \
             (analytical={anal_d:.6}, FD={fd_d:.6})",
            rel_err * 100.0
        );
    }
}

// ─── Kill criterion 3 ────────────────────────────────────────────────────────

/// Heston analytical: 50 strikes × 10 expiries must price in < 1.5 ms (release).
///
/// This test is **release-only** (`--release` flag required).  Debug builds
/// produce code that is ~100× slower; 500 Heston prices take > 1 s in debug
/// whereas they comfortably clear 1.5 ms in a release build on a modern CPU.
///
/// Run with: `cargo test --release --test pricing_validation heston_batch`
#[test]
#[cfg(not(debug_assertions))]
fn heston_batch_50x10_under_1500us() {
    let spot = 250.0;
    // Reasonable US large-cap params
    let base = HestonParams {
        s0:    spot,
        v0:    0.09,
        kappa: 2.0,
        theta: 0.09,
        sigma: 0.30,
        rho:   -0.70,
        r:     0.045,
        t:     0.5,
    };

    // 50 strikes: 0.70*S … 1.30*S
    let strikes: Vec<f64>    = (0..50).map(|i| spot * (0.70 + i as f64 * 0.012)).collect();
    // 10 maturities: 1 month … 10 months
    let maturities: Vec<f64> = (1..=10).map(|m| m as f64 / 12.0).collect();

    // Warm-up: single call so the instruction cache is hot before timing.
    let _ = heston_call_carr_madan(spot, spot, 0.5, 0.045, &base);

    let t0 = Instant::now();
    let mut checksum = 0.0_f64; // prevents dead-code elimination
    for &mat in &maturities {
        let mut p = base.clone();
        p.t = mat;
        for &k in &strikes {
            checksum += heston_call_carr_madan(spot, k, mat, 0.045, &p);
        }
    }
    let elapsed_us = t0.elapsed().as_micros();
    let _ = checksum;

    // 1.5 ms in release; 200 ms in debug
    assert!(
        elapsed_us < 1_500,
        "Heston 50×10 batch: {}µs ≥ 1500µs kill criterion  \
         (release build required)",
        elapsed_us
    );
}

// ─── Kill criterion 4 ────────────────────────────────────────────────────────

/// Heston calibration on a TSLA crash-period surface: mean |ΔIV| < 0.8 %.
///
/// **Why this test exists**
/// The original Nelder-Mead produced mean IV errors of 3–8 % on high-vol
/// crash-regime surfaces — well above the 0.8 % kill criterion.  This test
/// documents the regression boundary.  CMA-ES (the current optimizer) is
/// expected to pass it.
///
/// **Surface construction** (no live options data required)
/// `tesla_one_year.csv` contains only OHLCV.  We:
///   1. Extract the Feb–Mar 2025 crash window closes.
///   2. Compute the realized vol for that window (should be >> 40 %).
///   3. Build a crash-regime Heston surface via `create_mock_market_data`
///      using v₀ = RV² and structurally plausible crash params.
///   4. Calibrate Heston (CMA-ES) with an intentionally poor initial guess.
///   5. Assert mean |IV_model – IV_market| < 0.8 % across 28 cells.
///
/// **Runtime note**: CMA-ES with 10 000 fevals × 28 options is slow in debug
/// builds (~4 min).  This test is `#[ignore]` by default.  Run explicitly:
///   `cargo test --test pricing_validation -- --ignored heston_on_tesla`
/// For the full kill-criterion validation in reasonable time:
///   `cargo test --release --test pricing_validation -- --ignored heston_on_tesla`
#[test]
#[ignore]
fn heston_on_tesla_crash_period() {
    // ── 1. Load TSLA CSV, extract crash window ────────────────────────────
    // csv_loader returns newest-first; reverse to get chronological order.
    let all_days = load_csv_closes(TSLA_CSV)
        .expect("data/tesla_one_year.csv is required for pricing_validation tests");
    let chron: Vec<_> = all_days.iter().rev().collect();

    let crash_closes: Vec<f64> = chron.iter()
        .filter(|d| d.date.as_str() >= CRASH_START && d.date.as_str() <= CRASH_END)
        .map(|d| d.close)
        .collect();

    assert!(
        crash_closes.len() >= 20,
        "Expected ≥20 trading days in {CRASH_START}…{CRASH_END}, got {}",
        crash_closes.len()
    );

    // ── 2. Realized vol for the crash window ─────────────────────────────
    let log_returns: Vec<f64> = crash_closes.windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .collect();
    let n  = log_returns.len() as f64;
    let mu = log_returns.iter().sum::<f64>() / n;
    let crash_rv = ((log_returns.iter()
        .map(|r| (r - mu).powi(2))
        .sum::<f64>() / (n - 1.0)) * 252.0).sqrt();

    assert!(
        crash_rv > 0.40,
        "Feb-Mar 2025 TSLA realized vol should exceed 40 %, got {:.1} %",
        crash_rv * 100.0
    );

    // ── 3. Crash-regime Heston surface ────────────────────────────────────
    let spot = *crash_closes.last().unwrap();
    let v0   = (crash_rv * crash_rv).min(1.5); // instantaneous variance = crash RV²

    let true_params = CalibParams {
        v0,
        kappa: 1.5,           // slow mean-reversion (panic persists)
        theta: (v0 * 0.80).max(0.04), // long-run var below instantaneous
        sigma: 0.60,          // high vol-of-vol during panic
        rho:   -0.75,         // strong leverage effect
    };

    // 7 strikes (0.75–1.25 moneyness) × 4 maturities (1w, 1m, 3m, 6m) = 28 options
    let strikes: Vec<f64>    = (0..7)
        .map(|i| {
            let frac = 0.75 + i as f64 * (0.50 / 6.0); // 0.75, 0.833, ..., 1.25
            let raw  = spot * frac;
            // Round to the nearest $5 as a real exchange would list
            (raw / 5.0).round() * 5.0
        })
        .collect();
    let maturities: Vec<f64> = vec![7.0 / 365.0, 30.0 / 365.0, 90.0 / 365.0, 180.0 / 365.0];

    let surface = create_mock_market_data(spot, R, &true_params, &strikes, &maturities);
    assert_eq!(surface.len(), 7 * 4, "Expected 28 surface options");

    // ── 4. Calibrate with a deliberately poor initial guess ───────────────
    // The perturbation is large enough that a simplex solver (Nelder-Mead)
    // would frequently stall in a local minimum on this high-vol surface.
    let initial_guess = CalibParams {
        kappa: 3.0,  // 2× too high
        theta: 0.09, // ~3× too low (long-run vol = 30 % vs true ≈ 80 %)
        sigma: 0.35, // too low
        rho:   -0.40,
        v0:    0.15,
    };

    let result = calibrate_heston(spot, R, surface.clone(), initial_guess)
        .expect("calibrate_heston must not return Err");

    // ── 5. Kill criterion: mean |ΔIV| < 0.8 % ────────────────────────────
    let mae = mean_abs_iv_error(&surface, spot, &result.params);

    assert!(
        mae < 0.008,
        "Heston calibration on TSLA crash surface (Feb-Mar 2025): \
         mean |ΔIV| = {:.3} % ≥ 0.8 %.  \
         CMA-ES should pass; Nelder-Mead historically failed (3–8 % error) on \
         high-vol regimes.  Check cmaes.rs convergence or surface construction.",
        mae * 100.0
    );
}
