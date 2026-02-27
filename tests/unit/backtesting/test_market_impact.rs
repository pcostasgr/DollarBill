//! Tests for `SlippageModel::FullMarketImpact` and the regime-shift stress suite.
//!
//! Proposal 1: "Dynamic liquidity + market impact — size-based impact (big orders
//! move the market) + small-cap wider spreads"
//!
//! Proposal 2: "Regime-shift stress suite — 2020-03-16 crash data; watch partials
//! + panic spreads + Kelly implode your account"

use dollarbill::backtesting::{TradingCosts, SlippageModel, PartialFillModel};

// ─── Shared fixtures ──────────────────────────────────────────────────────────

/// Large-cap `FullMarketImpact`: same parameters as `PanicWidening` when
/// cap_multiplier=1.0 and size_impact_bps=0.0.
fn large_cap_impact(normal_vol: f64, panic_exponent: f64) -> TradingCosts {
    TradingCosts {
        commission_per_contract: 1.0,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 1.0,
            size_impact_bps: 0.0,
            normal_vol,
            panic_exponent,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    }
}

/// Small-cap `FullMarketImpact`: 3× illiquidity multiplier on base spread.
fn small_cap_impact() -> TradingCosts {
    TradingCosts {
        commission_per_contract: 1.0,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 3.0,
            size_impact_bps: 0.0,
            normal_vol: 0.20,
            panic_exponent: 2.0,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    }
}

// ─── 1. Unit tests: FullMarketImpact behaviour ────────────────────────────────

/// With cap_multiplier=1 and size_impact_bps=0 the result must equal PanicWidening.
#[test]
fn full_market_impact_large_cap_no_size_equals_panic_widening() {
    let normal_vol = 0.20;
    let panic_exp  = 2.0;

    let fmi   = large_cap_impact(normal_vol, panic_exp);
    let panic = TradingCosts {
        commission_per_contract: 1.0,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::PanicWidening { normal_vol, panic_exponent: panic_exp },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    // Test at five vol levels: below normal, at normal, 2×, 4×, 10×-cap
    for &vol in &[0.10_f64, 0.20, 0.40, 0.80, 2.00] {
        let s_fmi   = fmi.half_spread(vol, 1);
        let s_panic = panic.half_spread(vol, 1);
        assert!(
            (s_fmi - s_panic).abs() < 1e-12,
            "vol={}: FullMarketImpact ({:.8}) != PanicWidening ({:.8})",
            vol, s_fmi, s_panic
        );
    }
}

/// Small-cap (cap_multiplier=3) must have 3× the base spread at calm vol.
#[test]
fn full_market_impact_small_cap_3x_base_spread_at_calm_vol() {
    let sc = small_cap_impact();
    let lc = large_cap_impact(0.20, 2.0);  // cap_multiplier=1, but let's use calm vol

    let calm_vol = 0.10;  // well below normal_vol=0.20 → no panic widening
    let s_sc = sc.half_spread(calm_vol, 1);
    let s_lc = lc.half_spread(calm_vol, 1);

    assert!(
        (s_sc / s_lc - 3.0).abs() < 1e-9,
        "Small-cap spread ({:.6}) should be 3× large-cap ({:.6}), ratio={:.4}",
        s_sc, s_lc, s_sc / s_lc
    );
}

/// Size impact must be monotonically non-decreasing in contract count.
#[test]
fn full_market_impact_size_effect_monotone_in_contracts() {
    let costs = TradingCosts {
        commission_per_contract: 0.0,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 1.0,
            size_impact_bps: 20.0,  // 20 bps per √contract
            normal_vol: 0.20,
            panic_exponent: 1.0,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    let vol = 0.10;  // calm, no panic widening
    let mut prev = costs.half_spread(vol, 1);
    for contracts in [5, 10, 25, 50, 100, 500] {
        let current = costs.half_spread(vol, contracts);
        assert!(
            current >= prev,
            "half_spread should be non-decreasing in contracts: {} contracts ({}) < {} contracts ({})",
            contracts, current, contracts / 5, prev
        );
        prev = current;
    }
}

/// All three effects combine additively (then scaled by panic).
/// Manual calculation must match `half_spread()`.
#[test]
fn full_market_impact_all_effects_match_manual_formula() {
    let bid_ask_pct       = 2.0_f64;  // 2%
    let cap_multiplier    = 2.5_f64;
    let size_impact_bps   = 30.0_f64;
    let normal_vol        = 0.25_f64;
    let panic_exponent    = 1.5_f64;
    let contracts         = 16_i32;   // √16 = 4
    let vol               = 0.50_f64; // 2× normal_vol

    let costs = TradingCosts {
        commission_per_contract: 0.0,
        bid_ask_spread_percent: bid_ask_pct,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier,
            size_impact_bps,
            normal_vol,
            panic_exponent,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    // Manual formula
    let base        = (bid_ask_pct / 200.0) * cap_multiplier;        // = 0.025
    let size        = (size_impact_bps / 10_000.0) * (contracts as f64).sqrt();  // = 0.012
    let panic_mult  = (vol / normal_vol).min(10.0).powf(panic_exponent); // = 2^1.5 = 2.828...
    let expected    = (base + size) * panic_mult;

    let got = costs.half_spread(vol, contracts);
    assert!(
        (got - expected).abs() < 1e-12,
        "FullMarketImpact formula mismatch: expected {:.8} got {:.8}",
        expected, got
    );
}

/// Half-spread must be strictly positive even at zero vol and 1 contract.
#[test]
fn full_market_impact_always_positive() {
    let costs = TradingCosts {
        commission_per_contract: 0.0,
        bid_ask_spread_percent: 0.5,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 1.0,
            size_impact_bps: 0.0,
            normal_vol: 0.20,
            panic_exponent: 2.0,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    let h = costs.half_spread(0.0, 1);
    assert!(h > 0.0, "Half-spread must be > 0, got {}", h);
    assert!(h.is_finite(), "Half-spread must be finite, got {}", h);
}

/// At 10× normal_vol (panic cap), the spread must not overflow to infinity.
#[test]
fn full_market_impact_panic_cap_at_10x_vol_finite() {
    let normal_vol = 0.20;
    let costs = TradingCosts {
        commission_per_contract: 0.0,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 3.0,
            size_impact_bps: 50.0,
            normal_vol,
            panic_exponent: 3.0,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    // vol = 100× normal_vol — should be capped at 10× by the `.min(10.0)` guard
    let h = costs.half_spread(normal_vol * 100.0, 100);
    assert!(h.is_finite(), "10×-panic-capped spread must be finite, got {}", h);
    assert!(h > 0.0, "10×-panic-capped spread must be positive, got {}", h);
}

// ─── 2. Regime-shift stress: 2020-03-16 crash conditions ─────────────────────
//
// On 2020-03-16 the S&P 500 dropped ~12% in a single session — the largest
// single-day fall since 1987.  The VIX closed at 82.69.  We synthesise 30
// crash-day sequences and verify that the combined cost model (FullMarketImpact
// + VolScaled partial fills) imposes proportionally crushing costs relative to
// calm conditions, matching the intuition that large vol-sellers were wiped out
// by spread blow-out and partial fills simultaneously.

/// Steady-state (vol=0.18) and crash (vol=0.80) total costs for a 10-contract
/// trade must diverge substantially under FullMarketImpact+VolScaled.
#[test]
fn crash_2020_total_costs_far_exceed_calm_conditions() {
    let mid       = 10.0_f64;  // $10 option mid price
    let contracts = 10_i32;

    let crash_costs = TradingCosts {
        commission_per_contract: 1.50,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 2.0,       // small-cap flavour
            size_impact_bps: 15.0,
            normal_vol: 0.20,
            panic_exponent: 2.0,
        },
        partial_fill_model: PartialFillModel::VolScaled {
            normal_vol: 0.20,
            min_fill_rate: 0.25,
        },
    };

    let calm_vol  = 0.18;
    let crash_vol = 0.80;

    // Contracts actually filled under each vol regime
    let filled_calm  = crash_costs.apply_partial_fill(contracts, calm_vol);
    let filled_crash = crash_costs.apply_partial_fill(contracts, crash_vol);

    // One-way slippage cost
    let slippage_calm  = crash_costs.one_way_slippage(mid, filled_calm,  calm_vol);
    let slippage_crash = crash_costs.one_way_slippage(mid, filled_crash, crash_vol);

    // Commission
    let comm_calm  = crash_costs.commission_for(filled_calm);
    let comm_crash = crash_costs.commission_for(filled_crash);

    let total_calm  = slippage_calm  + comm_calm;
    let total_crash = slippage_crash + comm_crash;

    // In a crash the spread is 16× wider: the cost-per-dollar-traded should be
    // dramatically higher despite fewer contracts filling.
    let slippage_per_contract_calm  = if filled_calm  > 0 { slippage_calm  / filled_calm  as f64 } else { 0.0 };
    let slippage_per_contract_crash = if filled_crash > 0 { slippage_crash / filled_crash as f64 } else { 0.0 };

    assert!(
        filled_crash < filled_calm,
        "Partial fills should be lower in crash ({}) than calm ({})",
        filled_crash, filled_calm
    );
    assert!(
        slippage_per_contract_crash > slippage_per_contract_calm * 5.0,
        "Per-contract slippage in crash ({:.4}) should be > 5× calm ({:.4})",
        slippage_per_contract_crash, slippage_per_contract_calm
    );
    // Total cost must still be positive and finite
    assert!(total_calm.is_finite() && total_calm > 0.0,
            "Calm total cost must be positive and finite: {}", total_calm);
    assert!(total_crash.is_finite() && total_crash > 0.0,
            "Crash total cost must be positive and finite: {}", total_crash);
}

/// 30-day crash sequence: cumulative spread cost must exceed calm-market equivalent
/// by more than 5× when using FullMarketImpact at crash vol.
/// Uses `AlwaysFull` fills to isolate the pure spread-widening effect; a companion
/// test (`crash_2020_total_costs_far_exceed_calm_conditions`) covers the combined
/// partial-fill + spread interaction.
#[test]
fn crash_sequence_30_days_spread_cost_exceeds_5x_calm() {
    // Simulated 30-day vol series: first 5 calm, then escalating crash
    let calm_vols:  Vec<f64> = (0..30).map(|_| 0.18).collect();
    let crash_vols: Vec<f64> = (0..5).map(|_| 0.18_f64)
        .chain((5..12).map(|i| 0.18_f64 + (i as f64 - 4.0) * 0.09)) // ramp to 0.81
        .chain((12..30).map(|_| 0.80_f64))                            // sustained crash
        .collect();

    // AlwaysFull fills so we measure spread widening alone (partial fills are a
    // separate effect tested in crash_2020_total_costs_far_exceed_calm_conditions).
    let costs = TradingCosts {
        commission_per_contract: 0.0,   // zero commission to isolate slippage
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 1.5,
            size_impact_bps: 10.0,
            normal_vol: 0.25,
            panic_exponent: 2.0,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    let mid       = 5.0_f64;
    let requested = 20_i32;

    let cumulative_slippage = |vols: &[f64]| -> f64 {
        vols.iter().map(|&vol| {
            costs.one_way_slippage(mid, requested, vol)
        }).sum::<f64>()
    };

    let calm_total  = cumulative_slippage(&calm_vols);
    let crash_total = cumulative_slippage(&crash_vols);

    assert!(
        crash_total > calm_total * 5.0,
        "30-day crash cumulative spread cost ({:.2}) should be > 5× calm ({:.2}); ratio={:.2}",
        crash_total, calm_total, crash_total / calm_total
    );
}

/// fill_price must be higher when buying and lower when selling — direction
/// sign is preserved under FullMarketImpact at crash vol.
#[test]
fn full_market_impact_fill_price_direction_correct_at_crash_vol() {
    let mid = 8.0_f64;
    let vol = 0.80;

    let costs = TradingCosts {
        commission_per_contract: 0.0,
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::FullMarketImpact {
            cap_multiplier: 2.0,
            size_impact_bps: 20.0,
            normal_vol: 0.20,
            panic_exponent: 2.0,
        },
        partial_fill_model: PartialFillModel::AlwaysFull,
    };

    let buy_price  = costs.fill_price(mid, true,  vol, 10);
    let sell_price = costs.fill_price(mid, false, vol, 10);

    assert!(
        buy_price > mid,
        "Buy fill price ({:.4}) must exceed mid ({}) at crash vol",
        buy_price, mid
    );
    assert!(
        sell_price < mid,
        "Sell fill price ({:.4}) must be below mid ({}) at crash vol",
        sell_price, mid
    );
}
