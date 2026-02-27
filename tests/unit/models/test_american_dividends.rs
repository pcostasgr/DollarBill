//! American option early-exercise tests with continuous dividend yield.
//!
//! Proposal 2: "American early exercise → most options are American; European
//! BS underprices deep ITM calls with dividends."
//!
//! The `BinomialConfig` struct now holds `dividend_yield: f64`, and both
//! `binomial_tree` (American) and `binomial_tree_european` (European) use it
//! to adjust the risk-neutral drift: p = (e^{(r−q)·dt} − d) / (u − d).
//!
//! Tests verify:
//!   • High dividend yield produces a positive, finite American call price
//!   • American call is always ≥ European call for the same params (no free lunch)
//!   • Early-exercise premium is larger with dividends than without
//!   • Dividend yield reduces the European call price (lower forward price)
//!   • With q = 0 the American equals the European (Merton 1973)

use dollarbill::models::american::{
    american_call_binomial, european_call_binomial, BinomialConfig,
};
use dollarbill::models::bs_mod::black_scholes_merton_call;

// ─── Shared fixtures ─────────────────────────────────────────────────────────

fn config(q: f64) -> BinomialConfig {
    BinomialConfig {
        n_steps: 200,
        use_dividends: q > 0.0,
        dividend_yield: q,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// Smoke test: American call with a 10 % continuous dividend yield on a deep-ITM
/// stock returns a positive, finite price.
#[test]
fn american_call_positive_with_high_dividend() {
    let price = american_call_binomial(200.0, 100.0, 1.0, 0.05, 0.20, &config(0.10));
    assert!(price.is_finite(), "American call price is not finite: {}", price);
    assert!(price > 0.0,       "American call price is non-positive: {}", price);
}

/// American call >= European call always — regardless of dividend yield.
/// This must hold because American ⊇ European (extra right of early exercise).
#[test]
fn american_call_geq_european_call_any_dividend() {
    for &q in &[0.0_f64, 0.02, 0.05, 0.08, 0.12] {
        let american = american_call_binomial(100.0, 100.0, 1.0, 0.05, 0.25, &config(q));
        let european = european_call_binomial(100.0, 100.0, 1.0, 0.05, 0.25, &config(q));
        assert!(
            american >= european - 1e-6,
            "American ({:.4}) < European ({:.4}) at q={} — early exercise cannot have negative value",
            american, european, q
        );
    }
}

/// Key early-exercise test: for a deep-ITM call on a high-dividend stock the
/// early-exercise premium (American − European) must be larger than the premium
/// on an otherwise identical zero-dividend stock.
///
/// Intuition: with q = 10 % per year the stock is expected to lose dividend
/// value, so the option holder prefers to exercise today and pocket $100 rather
/// than wait and see the intrinsic erode.
#[test]
fn early_exercise_premium_larger_with_high_dividends() {
    let spot   = 200.0;
    let strike = 100.0;
    let t      = 1.0;
    let r      = 0.05;
    let vol    = 0.20;

    let american_div    = american_call_binomial(spot, strike, t, r, vol, &config(0.10));
    let european_div    = european_call_binomial(spot, strike, t, r, vol, &config(0.10));
    let american_no_div = american_call_binomial(spot, strike, t, r, vol, &config(0.00));
    let european_no_div = european_call_binomial(spot, strike, t, r, vol, &config(0.00));

    let ee_premium_div    = american_div    - european_div;
    let ee_premium_no_div = american_no_div - european_no_div;

    assert!(
        ee_premium_div > ee_premium_no_div,
        "Early-exercise premium with dividends ({:.4}) should be > without ({:.4})",
        ee_premium_div, ee_premium_no_div
    );

    // With a 10 % yield and 2× ITM, the premium must be meaningfully positive
    assert!(
        ee_premium_div > 0.50,
        "Early-exercise premium with 10% dividend yield should be > $0.50, got {:.4}",
        ee_premium_div
    );
}

/// Dividend yield reduces the European call price.
/// The forward price is S·e^{(r−q)T}: higher q → lower forward → lower call.
#[test]
fn higher_dividend_yield_reduces_european_call() {
    let spot = 100.0; let strike = 100.0; let t = 1.0; let r = 0.05; let vol = 0.25;

    let euro_0  = european_call_binomial(spot, strike, t, r, vol, &config(0.00));
    let euro_5  = european_call_binomial(spot, strike, t, r, vol, &config(0.05));
    let euro_10 = european_call_binomial(spot, strike, t, r, vol, &config(0.10));

    assert!(
        euro_0 > euro_5 && euro_5 > euro_10,
        "European call prices should decrease with dividend yield: \
         q=0%: {:.4}, q=5%: {:.4}, q=10%: {:.4}",
        euro_0, euro_5, euro_10
    );
}

/// European BSM (q = 0) overprices relative to European binomial with correct
/// dividend yield — quantifying the mispricing that motivated Proposal 2.
/// (Equivalently: ignoring dividends inflates the theoretical call price.)
#[test]
fn bsm_no_dividend_overprices_vs_binomial_with_dividend() {
    let spot = 150.0; let strike = 100.0; let t = 1.0; let r = 0.03; let vol = 0.20;
    let q    = 0.08;  // 8 % continuous dividend yield

    // BSM without dividends — what the engine uses if dividends are ignored
    let bsm_no_div = black_scholes_merton_call(spot, strike, t, r, vol, 0.0).price;

    // European binomial with the correct dividend yield
    let euro_with_div = european_call_binomial(spot, strike, t, r, vol, &config(q));

    assert!(
        bsm_no_div > euro_with_div,
        "BSM(q=0)={:.4} should be > European binomial(q=8%)={:.4}: \
         ignoring dividends inflates the call",
        bsm_no_div, euro_with_div
    );
}

/// American call with q = 0 must equal the European call to within binomial
/// discretisation error (Merton 1973: early exercise is never optimal on a
/// non-dividend-paying stock because holding the cash-equivalent dominates).
#[test]
fn no_dividend_american_call_equals_european() {
    let spot = 100.0; let strike = 80.0; let t = 0.5; let r = 0.05; let vol = 0.30;
    let cfg  = config(0.0);

    let american = american_call_binomial(spot, strike, t, r, vol, &cfg);
    let european = european_call_binomial(spot, strike, t, r, vol, &cfg);

    let abs_diff = (american - european).abs();
    let rel_diff = abs_diff / european.max(1e-8);

    assert!(
        rel_diff < 0.01,   // within 1 % — binomial with 200 steps is accurate
        "No-dividend American call ({:.4}) should equal European ({:.4}) within 1%, \
         got {:.2}% difference",
        american, european, rel_diff * 100.0
    );
}

/// American call is >= intrinsic value: can always exercise immediately and
/// realise S − K.  This is the most basic put-call lower bound.
#[test]
fn american_call_geq_intrinsic_value() {
    for &(spot, strike, q) in &[
        (120.0_f64, 100.0, 0.00),
        (150.0,     100.0, 0.05),
        (200.0,     100.0, 0.10),
        (300.0,     100.0, 0.12),
    ] {
        let price    = american_call_binomial(spot, strike, 1.0, 0.05, 0.20, &config(q));
        let intrinsic = (spot - strike).max(0.0);
        assert!(
            price >= intrinsic - 1e-6,
            "American call {:.4} < intrinsic {:.4} at spot={} strike={} q={}",
            price, intrinsic, spot, strike, q
        );
    }
}
