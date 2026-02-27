//! Unit tests for `LiquidityTier` and `MidPriceImpact`.
//!
//! Verifies the mathematical invariants of the √-participation impact model:
//!   1. Monotonicity of tier parameters (MegaCap cheapest → MicroCap most expensive)
//!   2. Impact grows with order size (√-rule)
//!   3. Permanent + temporary = total  (decomposition identity)
//!   4. MicroCap orders cost dramatically more than MegaCap at the same size
//!   5. `for_symbol` maps known tickers to correct tiers
//!   6. `cap_multiplier()` agrees with `base_half_spread_bps()` ratios
//!   7. Adjusted mid-price direction is correct

use dollarbill::backtesting::{LiquidityTier, MidPriceImpact};

// ─── 1. Tier parameter monotonicity ──────────────────────────────────────────

/// Base half-spread must strictly increase from MegaCap → MicroCap.
#[test]
fn liquidity_tier_base_spread_strictly_increasing() {
    let spreads: Vec<f64> = LiquidityTier::all_tiers()
        .iter()
        .map(|t| t.base_half_spread_bps())
        .collect();

    for i in 0..spreads.len() - 1 {
        assert!(
            spreads[i] < spreads[i + 1],
            "base_half_spread_bps must be strictly increasing: tier[{}]={} >= tier[{}]={}",
            i, spreads[i], i + 1, spreads[i + 1]
        );
    }
}

/// Impact coefficient must strictly increase from MegaCap → MicroCap.
#[test]
fn liquidity_tier_impact_coefficient_strictly_increasing() {
    let lambdas: Vec<f64> = LiquidityTier::all_tiers()
        .iter()
        .map(|t| t.impact_coefficient())
        .collect();

    for i in 0..lambdas.len() - 1 {
        assert!(
            lambdas[i] < lambdas[i + 1],
            "impact_coefficient must be strictly increasing: tier[{}]={} >= tier[{}]={}",
            i, lambdas[i], i + 1, lambdas[i + 1]
        );
    }
}

/// Permanent fraction must be non-decreasing from MegaCap → MicroCap.
#[test]
fn liquidity_tier_permanent_fraction_nondecreasing() {
    let fracs: Vec<f64> = LiquidityTier::all_tiers()
        .iter()
        .map(|t| t.permanent_fraction())
        .collect();

    for i in 0..fracs.len() - 1 {
        assert!(
            fracs[i] <= fracs[i + 1],
            "permanent_fraction must be non-decreasing: tier[{}]={} > tier[{}]={}",
            i, fracs[i], i + 1, fracs[i + 1]
        );
    }
}

/// All permanent fractions must be in (0, 1).
#[test]
fn liquidity_tier_permanent_fraction_in_unit_interval() {
    for tier in LiquidityTier::all_tiers() {
        let f = tier.permanent_fraction();
        assert!(
            f > 0.0 && f < 1.0,
            "{}: permanent_fraction={} not in (0,1)", tier.label(), f
        );
    }
}

// ─── 2. cap_multiplier agrees with base_half_spread_bps ratios ────────────────

/// cap_multiplier() must equal base_half_spread_bps / MegaCap.base_half_spread_bps.
#[test]
fn cap_multiplier_equals_spread_ratio_vs_mega_cap() {
    let mega_spread = LiquidityTier::MegaCap.base_half_spread_bps();
    for tier in LiquidityTier::all_tiers() {
        let expected = tier.base_half_spread_bps() / mega_spread;
        let got = tier.cap_multiplier();
        assert!(
            (got - expected).abs() < 1e-12,
            "{}: cap_multiplier={} expected={}",
            tier.label(), got, expected
        );
    }
}

/// MegaCap cap_multiplier must be 1.0 (the reference point).
#[test]
fn mega_cap_multiplier_is_one() {
    assert!(
        (LiquidityTier::MegaCap.cap_multiplier() - 1.0).abs() < 1e-12,
        "MegaCap cap_multiplier must be 1.0, got {}",
        LiquidityTier::MegaCap.cap_multiplier()
    );
}

// ─── 3. for_symbol maps known tickers correctly ───────────────────────────────

#[test]
fn for_symbol_spy_is_mega_cap() {
    let imp = MidPriceImpact::for_symbol("SPY").unwrap();
    assert_eq!(imp.tier, LiquidityTier::MegaCap, "SPY must be MegaCap");
    assert!(imp.avg_daily_value >= 1_000_000_000.0, "SPY ADV must be ≥ $1 B");
}

#[test]
fn for_symbol_tsla_is_large_cap() {
    let imp = MidPriceImpact::for_symbol("TSLA").unwrap();
    assert_eq!(imp.tier, LiquidityTier::LargeCap, "TSLA must be LargeCap");
}

#[test]
fn for_symbol_pltr_is_large_cap() {
    let imp = MidPriceImpact::for_symbol("PLTR").unwrap();
    assert_eq!(imp.tier, LiquidityTier::LargeCap, "PLTR must be LargeCap");
}

#[test]
fn for_symbol_case_insensitive() {
    let lower = MidPriceImpact::for_symbol("aapl").map(|i| i.tier);
    let upper = MidPriceImpact::for_symbol("AAPL").map(|i| i.tier);
    assert_eq!(lower, upper, "Symbol lookup must be case-insensitive");
}

#[test]
fn for_symbol_unknown_returns_none() {
    assert!(
        MidPriceImpact::for_symbol("UNKNOWN_TICKER_XYZ").is_none(),
        "Unknown ticker must return None"
    );
}

// ─── 4. Mid-price impact mathematical properties ─────────────────────────────

/// total_impact must be strictly positive for any non-zero order.
#[test]
fn mid_price_impact_total_always_positive() {
    for tier in LiquidityTier::all_tiers() {
        let imp = MidPriceImpact::new(tier, 1_000_000_000.0);
        let total = imp.total_impact(10.0, 10_000.0);
        assert!(total > 0.0, "{}: total_impact must be > 0, got {}", tier.label(), total);
        assert!(total.is_finite(), "{}: total_impact must be finite, got {}", tier.label(), total);
    }
}

/// Decomposition identity: permanent + temporary = total  (within float tolerance).
#[test]
fn mid_price_impact_permanent_plus_temporary_equals_total() {
    for tier in LiquidityTier::all_tiers() {
        let imp = MidPriceImpact::new(tier, 500_000_000.0);
        let mid = 8.50_f64;
        let order_val = 85_000.0;  // 100-lot × 100 × $8.50

        let total = imp.total_impact(mid, order_val);
        let perm  = imp.permanent_impact(mid, order_val);
        let temp  = imp.temporary_impact(mid, order_val);

        assert!(
            (perm + temp - total).abs() < 1e-10,
            "{}: permanent ({:.8}) + temporary ({:.8}) ≠ total ({:.8})",
            tier.label(), perm, temp, total
        );
    }
}

/// Impact must grow with order size following the √-rule:
/// 4× the order value → 2× the impact.
#[test]
fn mid_price_impact_grows_with_sqrt_of_order_size() {
    let imp = MidPriceImpact::new(LiquidityTier::MidCap, 100_000_000.0);
    let mid = 5.0_f64;

    let small_order = 10_000.0;
    let large_order = 40_000.0;   // 4× larger

    let small_impact = imp.total_impact(mid, small_order);
    let large_impact = imp.total_impact(mid, large_order);

    // √4 = 2; check ratio is exactly 2.0 within float precision
    let ratio = large_impact / small_impact;
    assert!(
        (ratio - 2.0).abs() < 1e-9,
        "4× order size must double impact (√-rule): ratio={:.6} expected=2.0",
        ratio
    );
}

/// MicroCap impact must be far larger than MegaCap for the same order.
/// λ ratio = 1.20 / 0.03 = 40×; we assert > 10× for a comfortable bound.
#[test]
fn micro_cap_impact_far_exceeds_mega_cap() {
    let mid = 10.0_f64;
    let order_val = 100_000.0;

    let mega  = MidPriceImpact::new(LiquidityTier::MegaCap,  5_000_000_000.0);
    let micro = MidPriceImpact::new(LiquidityTier::MicroCap,    50_000_000.0);

    let mega_impact  = mega.total_impact(mid, order_val);
    let micro_impact = micro.total_impact(mid, order_val);

    assert!(
        micro_impact > mega_impact * 10.0,
        "MicroCap impact ({:.4}) must be > 10× MegaCap impact ({:.4}); ratio={:.1}",
        micro_impact, mega_impact, micro_impact / mega_impact
    );
}

// ─── 5. adjusted_mid direction ────────────────────────────────────────────────

/// Buying: adjusted mid must be above the pre-trade mid.
#[test]
fn adjusted_mid_is_above_pre_trade_for_buy() {
    let imp = MidPriceImpact::new(LiquidityTier::MidCap, 200_000_000.0);
    let mid = 12.0_f64;
    let order_val = 50_000.0;

    let adj = imp.adjusted_mid(mid, order_val, true);
    assert!(
        adj > mid,
        "Buy: adjusted mid ({:.4}) must be above pre-trade mid ({:.4})",
        adj, mid
    );
}

/// Selling: adjusted mid must be below the pre-trade mid.
#[test]
fn adjusted_mid_is_below_pre_trade_for_sell() {
    let imp = MidPriceImpact::new(LiquidityTier::MidCap, 200_000_000.0);
    let mid = 12.0_f64;
    let order_val = 50_000.0;

    let adj = imp.adjusted_mid(mid, order_val, false);
    assert!(
        adj < mid,
        "Sell: adjusted mid ({:.4}) must be below pre-trade mid ({:.4})",
        adj, mid
    );
}

// ─── 6. impact_cost_bps units sanity check ────────────────────────────────────

/// MegaCap 1-lot ($1k order vs $30 B ADV) impact cost must be < 1 bps.
#[test]
fn mega_cap_small_order_impact_bps_under_one() {
    let spy = MidPriceImpact::for_symbol("SPY").unwrap();
    let order_val = 1_000.0;  // 1 contract at ~$10 mid
    let bps = spy.impact_cost_bps(order_val);
    assert!(
        bps < 1.0,
        "SPY 1-lot impact must be < 1 bps, got {:.4} bps", bps
    );
}

/// SmallCap 100-lot ($100k order vs $5 M ADV) impact cost must exceed 50 bps.
#[test]
fn small_cap_large_order_impact_bps_substantial() {
    let imp = MidPriceImpact::new(LiquidityTier::SmallCap, 5_000_000.0);
    let order_val = 100_000.0;
    let bps = imp.impact_cost_bps(order_val);
    assert!(
        bps > 50.0,
        "SmallCap 100-lot impact must be > 50 bps, got {:.2} bps", bps
    );
}

// ─── 7. total_impact sub-linearity (price displacement per dollar of order) ───

/// mid-price displacement per dollar of order value decreases as order size
/// grows — this is the defining property of the √-participation model.
///
/// `total_impact(mid, order) / order_value = λ × √(order/ADV) × mid / order`
///   = `λ × mid / √(order × ADV)`  which is O(1/√order) → decreasing.
///
/// A 100× larger order should have < 100× the price impact per dollar.
#[test]
fn impact_price_per_dollar_sublinear_in_order_size() {
    let imp = MidPriceImpact::new(LiquidityTier::LargeCap, 1_000_000_000.0);
    let mid = 10.0_f64;

    let small_order = 10_000.0;
    let large_order = 1_000_000.0;   // 100× larger

    let impact_per_dollar_small = imp.total_impact(mid, small_order) / small_order;
    let impact_per_dollar_large = imp.total_impact(mid, large_order) / large_order;

    assert!(
        impact_per_dollar_large < impact_per_dollar_small,
        "Price impact per dollar must decrease with order size (sub-linear): \
         small={:.8} large={:.8}",
        impact_per_dollar_small, impact_per_dollar_large
    );
}
