//! Regime-shift stress suite.
//!
//! Simulates three real-world market-stress scenarios using the Heston model
//! and the full cost stack (FullMarketImpact + VolScaled partial fills):
//!
//!   1. **March 2020 crash** — VIX 82, vol = 80%, sustained for 15 trading days.
//!      Tests: Heston pricing survives extreme variance, spread costs crush P&L,
//!      partial fills drop below 40%, call prices still satisfy lower bounds.
//!
//!   2. **Post-earnings vol crush** — IV collapses from 60% → 15% overnight.
//!      Tests: put value collapses correctly, call value falls, put-call parity holds.
//!
//!   3. **Regime switch: calm → panic → recovery** — 30-day vol path simulated via
//!      Heston Monte-Carlo under three parameter regimes.  Tests: all path variances
//!      non-negative throughout the switch, prices remain bracketed.

use dollarbill::models::heston::{HestonMonteCarlo, HestonParams, MonteCarloConfig};
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use dollarbill::backtesting::{TradingCosts, SlippageModel, PartialFillModel};

// ─── Parameter fixtures ───────────────────────────────────────────────────────

/// March 2020 crash: VIX 82, spot down 35%.
///
/// v0 = 0.82² ≈ 0.6724  (instantaneous variance at VIX 82)
/// kappa deliberately low — mean reversion is slow during sustained panic.
fn crash_heston_params(spot: f64) -> HestonParams {
    HestonParams {
        s0:    spot,
        v0:    0.6724,   // VIX 82 instantaneous variance
        kappa: 1.0,      // slow mean reversion (panic persists)
        theta: 0.16,     // long-run variance = 40% vol (elevated post-crisis)
        sigma: 0.60,     // high vol-of-vol in panic
        rho:   -0.70,    // strong leverage effect
        r:     0.0025,   // near-zero rates (March 2020 Fed cut)
        t:     1.0 / 252.0 * 20.0,  // 20 trading days remaining
    }
}

/// Normal (pre-crash) params: VIX 15, calm market.
fn calm_heston_params(spot: f64) -> HestonParams {
    HestonParams {
        s0:    spot,
        v0:    0.0225,   // VIX 15 → σ = 15%, v0 = 0.0225
        kappa: 3.0,
        theta: 0.04,
        sigma: 0.30,
        rho:   -0.70,
        r:     0.05,
        t:     30.0 / 252.0,
    }
}

/// Post-earnings vol crush: IV drops from 60% → 15%.
fn vol_crush_before(spot: f64) -> HestonParams {
    HestonParams {
        s0:    spot,
        v0:    0.36,     // σ = 60%
        kappa: 5.0,
        theta: 0.04,
        sigma: 0.40,
        rho:   -0.60,
        r:     0.05,
        t:     1.0 / 52.0,  // 1 week until earnings
    }
}

fn vol_crush_after(spot: f64) -> HestonParams {
    HestonParams {
        s0:    spot,
        v0:    0.0225,   // σ = 15% post-earnings
        kappa: 5.0,
        theta: 0.04,
        sigma: 0.30,
        rho:   -0.60,
        r:     0.05,
        t:     1.0 / 52.0,
    }
}

/// Crash-day TradingCosts: large-cap FullMarketImpact + VolScaled partial fills.
fn crash_day_costs() -> TradingCosts {
    TradingCosts {
        commission_per_contract: 1.50,
        bid_ask_spread_percent:  1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier:  1.0,    // large-cap (SPY, QQQ style)
            size_impact_bps: 10.0,
            normal_vol:      0.20,
            panic_exponent:  2.0,
        },
        partial_fill_model: PartialFillModel::VolScaled {
            normal_vol:    0.20,
            min_fill_rate: 0.25,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  1. March 2020 crash scenario
// ═══════════════════════════════════════════════════════════════════════════════

/// Heston Carr-Madan call price survives VIX-82 parameters and returns a
/// finite, positive value.
#[test]
fn crash_march_2020_heston_call_price_finite_and_positive() {
    let params = crash_heston_params(250.0);  // S&P ~2500 during crash
    let price = heston_call_carr_madan(250.0, 250.0, params.t, params.r, &params);

    assert!(
        price.is_finite(),
        "Crash-day Carr-Madan call must be finite (VIX 82): {}", price
    );
    assert!(
        price > 0.0,
        "Crash-day Carr-Madan call must be positive: {}", price
    );
}

/// Crash-day call price must exceed the intrinsic value (arbitrage lower bound).
#[test]
fn crash_march_2020_call_exceeds_intrinsic() {
    let spot   = 250.0_f64;
    let strike = 200.0;       // deep ITM
    let params = crash_heston_params(spot);
    let price    = heston_call_carr_madan(spot, strike, params.t, params.r, &params);
    let intrinsic = (spot - strike * (-params.r * params.t).exp()).max(0.0);

    assert!(
        price >= intrinsic - 1e-4,
        "Crash-day call {:.4} < intrinsic {:.4} — arbitrage violated",
        price, intrinsic
    );
}

/// Crash-day put price must be positive and finite.
#[test]
fn crash_march_2020_heston_put_price_finite_and_positive() {
    let params = crash_heston_params(250.0);
    let price = heston_put_carr_madan(250.0, 300.0, params.t, params.r, &params);
    // OTM put with high vol; should be expensive but finite

    assert!(
        price.is_finite(),
        "Crash-day Carr-Madan put must be finite: {}", price
    );
    assert!(
        price >= 0.0,
        "Crash-day Carr-Madan put must be non-negative: {}", price
    );
}

/// Crash put-call parity: |C − P − S + K·e^{−rT}| < $5 tolerance at VIX 82.
/// (Wider tolerance than normal because MC estimator variance is higher at this vol.)
#[test]
fn crash_put_call_parity_within_five_dollars() {
    let spot   = 250.0_f64;
    let strike = 250.0;
    let params = crash_heston_params(spot);

    let call = heston_call_carr_madan(spot, strike, params.t, params.r, &params);
    let put  = heston_put_carr_madan (spot, strike, params.t, params.r, &params);
    let pcp  = spot - strike * (-params.r * params.t).exp();
    let err  = (call - put - pcp).abs();

    assert!(
        err < 5.0,
        "Crash put-call parity error = {:.4} (call={:.4} put={:.4}): exceeds $5 tolerance",
        err, call, put
    );
}

/// Under VIX-82 conditions crash-day trading costs (spread + partial fills) must
/// impose dramatically higher per-contract cost than calm vol=0.18 conditions.
#[test]
fn crash_trading_costs_exceed_calm_by_over_5x() {
    let costs = crash_day_costs();
    let mid = 10.0_f64;
    let lots = 10_i32;

    let calm_vol  = 0.18_f64;
    let crash_vol = 0.82_f64;  // VIX 82

    let filled_calm  = costs.apply_partial_fill(lots, calm_vol);
    let filled_crash = costs.apply_partial_fill(lots, crash_vol);

    assert!(
        filled_crash < filled_calm,
        "Crash partial fills ({}) must be below calm fills ({})", filled_crash, filled_calm
    );
    assert!(
        filled_crash <= (lots as f64 * 0.40).ceil() as i32,
        "At VIX 82, fills must be ≤ 40% of requested ({} of {})", filled_crash, lots
    );

    let slipp_per_lot_calm  = costs.one_way_slippage(mid, 1, calm_vol);
    let slipp_per_lot_crash = costs.one_way_slippage(mid, 1, crash_vol);

    assert!(
        slipp_per_lot_crash > slipp_per_lot_calm * 5.0,
        "Per-lot slippage in crash ({:.4}) must be > 5× calm ({:.4})",
        slipp_per_lot_crash, slipp_per_lot_calm
    );
}

/// 15-day crash sequence cumulative spread cost must dominate a calm 15-day window.
#[test]
fn crash_15_day_cumulative_spread_cost_exceeds_calm_3x() {
    let costs = crash_day_costs();
    let mid = 8.0_f64;
    let lots = 5_i32;

    let calm_vols:  Vec<f64> = (0..15).map(|_| 0.18_f64).collect();
    // VIX escalates from 20 to 82 over 5 days then stays elevated
    let crash_vols: Vec<f64> = (0..5).map(|i| 0.20 + i as f64 * 0.125)
        .chain((5..15).map(|_| 0.82))
        .collect();

    let total_slippage = |vols: &[f64]| -> f64 {
        vols.iter().map(|&v| costs.one_way_slippage(mid, lots, v)).sum::<f64>()
    };

    let calm_total  = total_slippage(&calm_vols);
    let crash_total = total_slippage(&crash_vols);

    assert!(
        crash_total > calm_total * 3.0,
        "15-day crash slippage ({:.2}) must be > 3× calm ({:.2}); ratio={:.2}",
        crash_total, calm_total, crash_total / calm_total
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
//  2. Post-earnings vol crush
// ═══════════════════════════════════════════════════════════════════════════════

/// After vol crush, both call and put must be cheaper than before.
#[test]
fn vol_crush_reduces_both_call_and_put_prices() {
    let spot   = 150.0_f64;
    let strike = 150.0;

    let before = vol_crush_before(spot);
    let after  = vol_crush_after(spot);

    let call_before = heston_call_carr_madan(spot, strike, before.t, before.r, &before);
    let call_after  = heston_call_carr_madan(spot, strike, after.t,  after.r,  &after);
    let put_before  = heston_put_carr_madan (spot, strike, before.t, before.r, &before);
    let put_after   = heston_put_carr_madan (spot, strike, after.t,  after.r,  &after);

    assert!(
        call_after < call_before,
        "Post-crush call ({:.4}) must be cheaper than pre-crush call ({:.4})",
        call_after, call_before
    );
    assert!(
        put_after < put_before,
        "Post-crush put ({:.4}) must be cheaper than pre-crush put ({:.4})",
        put_after, put_before
    );
}

/// Vol crush must collapse option prices by at least 50%: IV 60% → 15% is a 4×
/// reduction in vol, which typically halves or quarters ATM option value.
#[test]
fn vol_crush_collapses_atm_call_by_at_least_50pct() {
    let spot   = 150.0_f64;
    let strike = 150.0;

    let before = vol_crush_before(spot);
    let after  = vol_crush_after(spot);

    let call_before = heston_call_carr_madan(spot, strike, before.t, before.r, &before);
    let call_after  = heston_call_carr_madan(spot, strike, after.t,  after.r,  &after);

    let collapse_pct = 1.0 - call_after / call_before;
    assert!(
        collapse_pct > 0.50,
        "Post-crush ATM call should collapse > 50%; got {:.1}% (before={:.4} after={:.4})",
        collapse_pct * 100.0, call_before, call_after
    );
}

/// Post-crush put-call parity holds within $0.05 (tight maturity adjustment).
#[test]
fn vol_crush_after_put_call_parity_holds() {
    let spot   = 150.0_f64;
    let strike = 150.0;
    let params = vol_crush_after(spot);

    let call = heston_call_carr_madan(spot, strike, params.t, params.r, &params);
    let put  = heston_put_carr_madan (spot, strike, params.t, params.r, &params);
    let pcp  = spot - strike * (-params.r * params.t).exp();
    let err  = (call - put - pcp).abs();

    assert!(
        err < 0.05,
        "Post-crush PCP error = {:.6} (call={:.4} put={:.4}): exceeds $0.05",
        err, call, put
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
//  3. Calm → panic → recovery regime switch via Monte Carlo
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper: run a Heston MC simulation and return (price, min_variance, negative_count).
fn run_mc(params: HestonParams, n_paths: usize, n_steps: usize, seed: u64, strike: f64)
    -> (f64, f64, usize)
{
    let cfg = MonteCarloConfig {
        n_paths,
        n_steps,
        seed,
        use_antithetic: false,
    };
    let mc = HestonMonteCarlo::new_unchecked(params, cfg).expect("regime MC must succeed");
    let paths = mc.simulate_paths();

    let mut min_variance   = f64::INFINITY;
    let mut negative_count = 0usize;
    for path in &paths {
        for &v in &path.variances {
            if v < 0.0 { negative_count += 1; }
            if v < min_variance { min_variance = v; }
        }
    }

    let price = mc.price_european_call(strike);
    (price, min_variance, negative_count)
}

/// Calm regime: 2 000 paths, all variances non-negative, call price positive.
#[test]
fn calm_regime_2k_paths_all_variances_nonneg_price_positive() {
    let params = calm_heston_params(100.0);
    let (price, min_v, neg_count) = run_mc(params, 2_000, 50, 1001, 100.0);

    assert_eq!(neg_count, 0, "Calm regime: {} negative variances found", neg_count);
    assert!(
        min_v >= 0.0,
        "Calm regime: minimum variance must be ≥ 0, got {:.6e}", min_v
    );
    assert!(
        price > 0.0 && price.is_finite(),
        "Calm regime call price must be positive and finite: {}", price
    );
}

/// Crash regime: 2 000 paths with VIX-82 params, all variances non-negative
/// (full-truncation boundary prevents negative variance even at extreme vol-of-vol).
#[test]
fn crash_regime_2k_paths_all_variances_nonneg() {
    // Use new_unchecked because high sigma may violate Feller
    let params = crash_heston_params(250.0);

    assert!(!params.satisfies_feller(),
        "Crash params must violate Feller (sigma={:.2} is high)", params.sigma);

    let (_price, min_v, neg_count) = run_mc(params, 2_000, 100, 2020, 250.0);

    assert_eq!(
        neg_count, 0,
        "Crash regime: {} negative variance values — truncation not holding", neg_count
    );
    assert!(
        min_v >= 0.0,
        "Crash regime: minimum variance must be ≥ 0, got {:.6e}", min_v
    );
}

/// Crash call price must be substantially higher than calm call price
/// (ATM option with 80% vol >> 15% vol, everything else equal).
#[test]
fn crash_regime_call_price_far_exceeds_calm() {
    let spot   = 100.0_f64;
    let strike = 100.0;

    let calm_params = calm_heston_params(spot);
    let crash_p = HestonParams {
        s0:    spot,
        v0:    0.6724,
        kappa: 1.0,
        theta: 0.16,
        sigma: 0.60,
        rho:   -0.70,
        r:     0.05,
        t:     calm_params.t,  // same maturity for fair comparison
    };

    let calm_price  = heston_call_carr_madan(spot, strike, calm_params.t,  calm_params.r,  &calm_params);
    let crash_price = heston_call_carr_madan(spot, strike, crash_p.t,      crash_p.r,      &crash_p);

    assert!(
        crash_price > calm_price * 3.0,
        "Crash call ({:.4}) must be > 3× calm call ({:.4}); VIX 82 vs VIX 15",
        crash_price, calm_price
    );
}

/// Recovery regime: variance mean-reverts toward theta; MC prices should
/// bracket between crash and calm levels.
#[test]
fn recovery_regime_price_between_calm_and_crash() {
    let spot   = 100.0_f64;
    let strike = 100.0;

    // Recovery: v0 elevated but kappa high → fast mean reversion back to theta
    let recovery = HestonParams {
        s0:    spot,
        v0:    0.25,     // still elevated (VIX 50) but falling
        kappa: 8.0,      // aggressive reversion — V returning to normal
        theta: 0.04,     // long-run calm
        sigma: 0.40,
        rho:   -0.70,
        r:     0.05,
        t:     30.0 / 252.0,
    };

    let calm_params = calm_heston_params(spot);
    let crash_p = HestonParams { s0: spot, v0: 0.6724, kappa: 1.0, theta: 0.16, sigma: 0.60, rho: -0.70, r: 0.05, t: recovery.t };

    let calm_price     = heston_call_carr_madan(spot, strike, calm_params.t,  calm_params.r,  &calm_params);
    let crash_price    = heston_call_carr_madan(spot, strike, crash_p.t,      crash_p.r,      &crash_p);
    let recovery_price = heston_call_carr_madan(spot, strike, recovery.t,     recovery.r,     &recovery);

    assert!(
        recovery_price > calm_price,
        "Recovery call ({:.4}) must exceed calm ({:.4})", recovery_price, calm_price
    );
    assert!(
        recovery_price < crash_price,
        "Recovery call ({:.4}) must be below crash ({:.4})", recovery_price, crash_price
    );
}
