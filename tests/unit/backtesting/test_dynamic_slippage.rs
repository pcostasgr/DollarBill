//! Tests for dynamic bid-ask widening, partial fills, dividend-yield pricing,
//! and regime-shift Kelly blowup scenarios.
//!
//! Proposal 3: "Dynamic bid-ask + partial fills — static slippage is cute;
//! real markets widen in panic."
//! Proposal 4: "Regime-shift stress — feed 2020-03 data; watch your Kelly blow up."

use crate::helpers::generate_synthetic_stock_data;
use dollarbill::backtesting::{BacktestEngine, BacktestConfig, TradingCosts, SlippageModel, PartialFillModel, SignalAction};
use dollarbill::market_data::csv_loader::HistoricalDay;

// ─── PartialFillModel unit tests ─────────────────────────────────────────────

#[test]
fn partial_fill_always_full_rate_is_one() {
    let model = PartialFillModel::AlwaysFull;
    for vol in [0.0, 0.10, 0.25, 0.80, 2.0] {
        assert!(
            (model.fill_rate(vol) - 1.0).abs() < 1e-12,
            "AlwaysFull should return 1.0 at vol={}, got {}",
            vol, model.fill_rate(vol)
        );
    }
}

#[test]
fn partial_fill_vol_scaled_full_at_or_below_normal_vol() {
    let model = PartialFillModel::VolScaled { normal_vol: 0.30, min_fill_rate: 0.25 };
    for vol in [0.05, 0.15, 0.25, 0.30] {
        let rate = model.fill_rate(vol);
        assert!(
            (rate - 1.0).abs() < 1e-12,
            "VolScaled should be 1.0 at vol={} <= normal_vol=0.30, got {}",
            vol, rate
        );
    }
}

#[test]
fn partial_fill_vol_scaled_decreases_above_normal_vol() {
    let model = PartialFillModel::VolScaled { normal_vol: 0.25, min_fill_rate: 0.20 };
    let rate_normal = model.fill_rate(0.25);
    let rate_high   = model.fill_rate(0.50);
    let rate_panic  = model.fill_rate(1.00);

    assert!((rate_normal - 1.0).abs() < 1e-12, "rate at normal vol must be 1.0");
    assert!(
        rate_high < rate_normal,
        "fill rate at vol=0.50 ({}) should be < at vol=0.25 ({})",
        rate_high, rate_normal
    );
    assert!(
        rate_panic < rate_high,
        "fill rate at vol=1.00 ({}) should be < at vol=0.50 ({})",
        rate_panic, rate_high
    );
}

#[test]
fn partial_fill_vol_scaled_clamped_at_min_fill_rate() {
    let min = 0.15;
    let model = PartialFillModel::VolScaled { normal_vol: 0.20, min_fill_rate: min };
    // At extremely high vol the formula normal_vol/vol approaches 0 → must clamp to min
    let rate = model.fill_rate(100.0);
    assert!(
        (rate - min).abs() < 1e-9,
        "VolScaled floor should clamp to min_fill_rate={}, got {}",
        min, rate
    );
}

#[test]
fn apply_partial_fill_reduces_contracts_at_panic_vol() {
    let costs = TradingCosts {
        partial_fill_model: PartialFillModel::VolScaled {
            normal_vol: 0.25,
            min_fill_rate: 0.50,
        },
        ..TradingCosts::default()
    };
    let normal_fill = costs.apply_partial_fill(10, 0.20);
    let panic_fill  = costs.apply_partial_fill(10, 0.80);

    assert_eq!(normal_fill, 10, "At normal vol, full fill expected");
    assert!(
        panic_fill < normal_fill,
        "At panic vol=0.80, partial fill ({}) should be < full fill ({})",
        panic_fill, normal_fill
    );
    assert!(panic_fill >= 5, "50% floor should give at least 5 contracts from 10, got {}", panic_fill);
}

#[test]
fn apply_partial_fill_never_negative() {
    let costs = TradingCosts {
        partial_fill_model: PartialFillModel::VolScaled { normal_vol: 0.20, min_fill_rate: 0.0 },
        ..TradingCosts::default()
    };
    // min_fill_rate=0 means at extreme vol we could get 0 contracts — but never negative
    let result = costs.apply_partial_fill(5, 999.0);
    assert!(result >= 0, "apply_partial_fill must never return negative, got {}", result);
}

// ─── PanicWidening unit tests ─────────────────────────────────────────────────

#[test]
fn panic_widening_equals_base_at_normal_vol() {
    let normal_spread = TradingCosts {
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::Fixed,
        ..TradingCosts::default()
    };
    let panic_model = TradingCosts {
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::PanicWidening { normal_vol: 0.25, panic_exponent: 2.0 },
        ..TradingCosts::default()
    };

    // At vol == normal_vol the panic model must give the same spread as Fixed
    let fixed_spread  = normal_spread.half_spread(0.25, 1);
    let panic_spread  = panic_model.half_spread(0.25, 1);
    assert!(
        (fixed_spread - panic_spread).abs() < 1e-9,
        "PanicWidening at normal vol should equal Fixed: {} vs {}",
        fixed_spread, panic_spread
    );
}

#[test]
fn panic_widening_widens_above_normal_vol() {
    let costs = TradingCosts {
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::PanicWidening { normal_vol: 0.20, panic_exponent: 2.0 },
        ..TradingCosts::default()
    };

    let spread_normal = costs.half_spread(0.20, 1);
    let spread_panic  = costs.half_spread(0.80, 1); // 4× normal

    // With exponent=2, spread should be 4²=16× wider at 4× vol
    assert!(
        spread_panic > spread_normal * 10.0,
        "Panic spread ({}) should be >> normal spread ({}) at 4× vol",
        spread_panic, spread_normal
    );
}

#[test]
fn panic_widening_strictly_monotone_in_vol() {
    let costs = TradingCosts {
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::PanicWidening { normal_vol: 0.20, panic_exponent: 1.5 },
        ..TradingCosts::default()
    };
    let vols = [0.20, 0.30, 0.50, 0.80, 1.50];
    let spreads: Vec<f64> = vols.iter().map(|&v| costs.half_spread(v, 1)).collect();

    for i in 1..spreads.len() {
        assert!(
            spreads[i] >= spreads[i - 1],
            "PanicWidening spread should be non-decreasing: at vol={} got {}, at vol={} got {}",
            vols[i - 1], spreads[i - 1], vols[i], spreads[i]
        );
    }
}

// ─── Engine integration tests ─────────────────────────────────────────────────

/// Engine with PanicWidening slippage must survive a panic-like data sequence.
#[test]
fn engine_panic_widening_survives_crash() {
    let config = BacktestConfig {
        initial_capital: 50_000.0,
        trading_costs: TradingCosts {
            bid_ask_spread_percent: 0.5,
            slippage_model: SlippageModel::PanicWidening {
                normal_vol: 0.20,
                panic_exponent: 2.0,
            },
            ..TradingCosts::default()
        },
        max_positions: 3,
        position_size_pct: 15.0,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);

    // Crash-like data: flat then -2%/day for 30 days
    let mut data: Vec<HistoricalDay> = Vec::new();
    let mut price = 200.0_f64;
    for i in 0..60_usize {
        data.push(HistoricalDay { date: format!("2020-{:03}", i + 1), close: price });
        price *= if i < 20 { 1.001 } else { 0.980 };
    }

    let result = engine.run_with_signals(
        "SPY",
        data,
        |_sym, spot, idx, hist_vols| {
            let vol = if idx < 20 { 0.15 } else { 0.85 };
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: vol,
            }]
        },
    );

    assert!(!result.metrics.total_return_pct.is_infinite(), "return must be finite");
    assert!(!result.metrics.max_drawdown.is_infinite(), "drawdown must be finite");
}

/// Engine with VolScaled partial fills must survive panic sequence.
#[test]
fn engine_partial_fill_vol_scaled_survives_panic() {
    let config = BacktestConfig {
        initial_capital: 50_000.0,
        trading_costs: TradingCosts {
            partial_fill_model: PartialFillModel::VolScaled {
                normal_vol: 0.25,
                min_fill_rate: 0.40,
            },
            ..TradingCosts::default()
        },
        position_size_pct: 20.0,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(150.0, 80, 0.0, 0.60);

    let result = engine.run_with_signals(
        "SPY",
        data,
        |_sym, spot, _idx, hist_vols| {
            let vol = hist_vols.last().copied().unwrap_or(0.60);
            vec![SignalAction::BuyCall { strike: spot, days_to_expiry: 30, volatility: vol }]
        },
    );

    assert!(!result.metrics.total_return_pct.is_infinite());
    assert!(!result.metrics.max_drawdown.is_infinite());
}

/// PanicWidening fills are more expensive than Fixed at the same high vol.
/// This verifies that the model actually changes the economics vs Fixed.
#[test]
fn panic_widening_more_expensive_fill_than_fixed_at_high_vol() {
    let mid  = 5.00;
    let vol  = 0.80;

    let fixed = TradingCosts {
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::Fixed,
        ..TradingCosts::default()
    };
    let panic = TradingCosts {
        bid_ask_spread_percent: 1.0,
        slippage_model: SlippageModel::PanicWidening { normal_vol: 0.20, panic_exponent: 2.0 },
        ..TradingCosts::default()
    };

    let fixed_cost = fixed.fill_price(mid, true, vol, 1);
    let panic_cost = panic.fill_price(mid, true, vol, 1);

    assert!(
        panic_cost > fixed_cost,
        "PanicWidening buy cost ({}) should exceed Fixed ({}) at high vol",
        panic_cost, fixed_cost
    );
}

// ─── Regime-shift Kelly blowup ────────────────────────────────────────────────

/// Proposal 4: "Feed 2020-03 data; watch your Kelly blow up."
///
/// A Kelly-oversized strategy (50% position size with no stop-loss) selling
/// naked calls through a simulated 2020 crash with:
///   • PanicWidening spreads (fills get 50× wider during crash)
///   • VolScaled partial fills (only 40% of orders execute in panic)
///
/// The combined effect must not panic the engine and P&L must remain finite.
/// If Kelly sizing is too aggressive the account can approach 0, but it must
/// never go below -100% return (account can't go negative past zero).
#[test]
fn kelly_blowup_2020_panic_with_realistic_costs() {
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        trading_costs: TradingCosts {
            commission_per_contract: 1.50,
            bid_ask_spread_percent: 0.5,
            slippage_model: SlippageModel::PanicWidening {
                normal_vol: 0.20,
                panic_exponent: 2.0,
            },
            partial_fill_model: PartialFillModel::VolScaled {
                normal_vol: 0.25,
                min_fill_rate: 0.40,
            },
        },
        max_positions: 1,
        position_size_pct: 50.0,  // Kelly-aggressive: 50% of capital per trade
        stop_loss_pct: None,       // No stop — maximum pain
        take_profit_pct: None,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);

    // Three phases replicating 2020-02 → 2020-05:
    let mut data: Vec<HistoricalDay> = Vec::new();
    let mut price = 340.0_f64;

    // Phase 1: 60 days calm (February baseline)
    for i in 0..60_usize {
        data.push(HistoricalDay { date: format!("2020-{:03}", i + 1), close: price });
        price *= 1.0003;
    }
    // Phase 2: 30 days crash (−1.5%/day; mirrors Feb 20 – Mar 23 2020)
    for i in 0..30_usize {
        data.push(HistoricalDay { date: format!("2020-{:03}", 60 + i + 1), close: price });
        price *= 0.985;
    }
    // Phase 3: 30 days whipsaw recovery (+0.8%/day with extreme vol)
    for i in 0..30_usize {
        data.push(HistoricalDay { date: format!("2020-{:03}", 90 + i + 1), close: price });
        price *= 1.008;
    }

    let result = engine.run_with_signals(
        "SPY",
        data,
        |_sym, spot, idx, hist_vols| {
            // Vol signal: 15% in calm, 85% during crash, 60% in recovery
            let vol = if idx < 60 {
                0.15
            } else if idx < 90 {
                0.85
            } else {
                0.60
            };
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: vol,
            }]
        },
    );

    // ── Correctness assertions ─────────────────────────────────────────────
    assert!(!result.metrics.total_return_pct.is_infinite(),
            "Kelly blowup: return must not be Inf");
    assert!(result.metrics.max_drawdown.is_finite(),
            "Kelly blowup: drawdown must be finite");
    // Return must be >= -100% (account cannot go below zero)
    if result.metrics.total_trades > 0 {
        assert!(
            result.metrics.total_return_pct >= -100.0,
            "Return should be >= -100%, got {:.1}%",
            result.metrics.total_return_pct
        );
    }
}
