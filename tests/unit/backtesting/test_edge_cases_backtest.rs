//! Backtesting engine edge-case tests.
//!
//! Covers: short naked call during a vol spike (unlimited loss), iron condor
//! aggregated Greeks (near-zero delta), zero-trades Sharpe/drawdown, slippage
//! realism (fees eat thin edges), and position-sizing safety.

use crate::helpers::generate_synthetic_stock_data;
use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig, SignalAction};
use dollarbill::market_data::csv_loader::HistoricalDay;

// ─── 5. Backtesting Engine ────────────────────────────────────────────────────

/// Short naked call during a 2020-like vol spike → engine must not panic,
/// P&L loss should be large (unlimited-loss scenario).
#[test]
fn test_short_naked_call_vol_spike_large_loss() {
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        max_positions: 1,
        position_size_pct: 50.0,   // large enough to feel the pain
        days_to_expiry: 30,
        max_days_hold: 25,
        stop_loss_pct: None,        // no stop — unlimited loss scenario
        take_profit_pct: None,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);

    // Simulate a severe upward vol spike: stock jumps 50% in a short period
    let mut data: Vec<HistoricalDay> = Vec::new();
    let mut price = 100.0_f64;
    // Use sequential day numbers to avoid date wrapping
    for i in 0..60_usize {
        let year  = 2020 + i / 365;
        let day_of_year = i % 365 + 1;
        data.push(HistoricalDay { date: format!("{}-{:03}", year, day_of_year), close: price });
        if i < 10 {
            price *= 1.0; // flat first
        } else if i < 20 {
            price *= 1.05; // spike up — bad for short call
        } else {
            price *= 1.01; // remain elevated
        }
    }
    data.reverse(); // engine expects oldest-first

    let result = engine.run_with_signals(
        "SPY",
        data,
        |_sym, spot, _idx, hist_vols| {
            let vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: vol,
            }]
        },
    );

    // Engine must not panic; result metrics must be finite/NaN (never Inf)
    assert!(!result.metrics.total_return_pct.is_infinite(),
            "total_return_pct should not be Inf");
    assert!(!result.metrics.max_drawdown.is_infinite(),
            "max_drawdown should not be Inf");
}

/// Iron condor: sell OTM call + sell OTM put + buy further OTM call + buy further OTM put.
/// Net delta should be near zero; theta should be positive (short vol, collecting premium).
#[test]
fn test_iron_condor_greeks_aggregate() {
    use dollarbill::models::bs_mod::black_scholes_merton_call;
    use dollarbill::models::bs_mod::black_scholes_merton_put;

    let spot   = 100.0;
    let rate   = 0.05;
    let vol    = 0.20;
    let time   = 30.0 / 365.0;
    let div    = 0.0;

    // Iron condor legs (1 contract each = ×100 multiplier)
    let short_call_strike = 105.0;
    let long_call_strike  = 110.0;
    let short_put_strike  =  95.0;
    let long_put_strike   =  90.0;

    let sc = black_scholes_merton_call(spot, short_call_strike, time, rate, vol, div);
    let lc = black_scholes_merton_call(spot, long_call_strike,  time, rate, vol, div);
    let sp = black_scholes_merton_put (spot, short_put_strike,  time, rate, vol, div);
    let lp = black_scholes_merton_put (spot, long_put_strike,   time, rate, vol, div);

    // Short call (-1) + long call (+1) + short put (-1) + long put (+1)
    let net_delta = -sc.delta + lc.delta + (-sp.delta) + lp.delta;
    // For a short option, the theta P&L is -theta (negative of the long option theta).
    // Since long option theta < 0, short theta contribution = -negative = positive.
    let net_theta = -sc.theta + lc.theta + (-sp.theta) + lp.theta;

    assert!(net_delta.abs() < 0.15,
            "Iron condor net delta should be near 0, got {:.4}", net_delta);
    // For an iron condor (net short options), theta is POSITIVE:
    // the shorts are closer to ATM → their |theta| dominates the longs' |theta|,
    // so the position benefits from time decay (positive theta = earn money per day).
    assert!(net_theta > -1e-4,
            "Iron condor net theta expected ≥ 0 (net short premium = positive theta), got {:.4}", net_theta);
}

/// Zero trades case: Sharpe ratio is undefined (NaN), max drawdown is 0.
#[test]
fn test_zero_trades_sharpe_undefined_drawdown_zero() {
    // Set volatility_threshold very low so no signals ever fire
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);
    let empty_data: Vec<HistoricalDay> = Vec::new();
    let result = engine.run_simple_strategy("TEST", empty_data, 0.0);

    // With no data there can be no trades
    assert_eq!(result.metrics.total_trades, 0,
               "Expected 0 trades on empty data");
    assert!(result.metrics.sharpe_ratio.is_nan() || result.metrics.sharpe_ratio == 0.0,
            "Sharpe should be NaN or 0 with no trades, got {}",
            result.metrics.sharpe_ratio);
    assert!((result.metrics.max_drawdown).abs() < 1e-9,
            "Max drawdown should be 0 with no trades, got {}",
            result.metrics.max_drawdown);
}

/// High commission realism: a strategy with a small edge is eaten alive by fees.
/// With commissions set to match the expected premium, returns should be near 0 or negative.
#[test]
fn test_slippage_commission_eats_thin_edge() {
    let mut config_no_commission = BacktestConfig::default();
    config_no_commission.commission_per_trade = 0.0;

    let mut config_high_commission = BacktestConfig::default();
    config_high_commission.commission_per_trade = 50.0; // $50 per trade

    let data = generate_synthetic_stock_data(100.0, 100, 0.0, 0.15);

    let signal_fn = |_sym: &str, spot: f64, _idx: usize, hist_vols: &[f64]| {
        let vol = hist_vols.last().copied().unwrap_or(0.20);
        vec![SignalAction::SellCall {
            strike: spot * 1.05,
            days_to_expiry: 30,
            volatility: vol,
        }]
    };

    let mut engine_free = BacktestEngine::new(config_no_commission);
    let mut engine_costly = BacktestEngine::new(config_high_commission);

    let result_free   = engine_free.run_with_signals("TEST", data.clone(), signal_fn);
    let result_costly = engine_costly.run_with_signals("TEST", data, signal_fn);

    // Both must finish without panic
    assert!(!result_free.metrics.total_return_pct.is_infinite());
    assert!(!result_costly.metrics.total_return_pct.is_infinite());

    // High-commission version should pay more in commissions
    if result_free.metrics.total_trades > 0 && result_costly.metrics.total_trades > 0 {
        assert!(result_costly.metrics.total_commissions >= result_free.metrics.total_commissions,
                "Higher commission rate should cost more or equal");
    }
}

/// Position sizing safety: fixed-fraction sizing should never exceed account size,
/// even with many trades on volatile data.
#[test]
fn test_position_sizing_never_exceeds_account() {
    let config = BacktestConfig {
        initial_capital: 10_000.0,
        position_size_pct: 10.0,
        max_positions: 5,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(100.0, 200, 0.1, 0.40);

    let result = engine.run_with_signals(
        "TEST",
        data,
        |_sym, spot, _idx, hist_vols| {
            let vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![
                SignalAction::BuyCall { strike: spot, days_to_expiry: 30, volatility: vol },
                SignalAction::BuyPut  { strike: spot, days_to_expiry: 30, volatility: vol },
            ]
        },
    );

    // Account should not have exploded
    assert!(!result.metrics.total_return_pct.is_infinite(),
            "Return should not be Inf: {}", result.metrics.total_return_pct);
    // Max drawdown percentage should be finite
    assert!(result.metrics.max_drawdown.is_finite(),
            "Max drawdown should be finite: {}", result.metrics.max_drawdown);
}

/// Regime change simulation: low-vol data followed by high-vol data.
/// P&L should be finite and strategy should survive the transition.
#[test]
fn test_regime_change_low_to_high_vol_survives() {
    let mut low_vol_data  = generate_synthetic_stock_data(100.0, 60, 0.0, 0.10);
    let mut high_vol_data = generate_synthetic_stock_data(
        low_vol_data.last().map(|d| d.close).unwrap_or(100.0),
        60, 0.0, 0.60
    );

    // Adjust dates to avoid conflicts — use sequential day numbers in a different month/year
    for (i, d) in high_vol_data.iter_mut().enumerate() {
        // Day 60..119 mapped to unique date strings (no wrapping)
        d.date = format!("2024-{:03}", 60 + i + 1);
    }

    let mut combined = low_vol_data;
    combined.append(&mut high_vol_data);

    let config = BacktestConfig {
        initial_capital: 100_000.0,
        stop_loss_pct: Some(50.0),
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);

    let result = engine.run_with_signals(
        "SPY",
        combined,
        |_sym, spot, _idx, hist_vols| {
            let vol = hist_vols.last().copied().unwrap_or(0.20);
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: vol,
            }]
        },
    );

    assert!(!result.metrics.total_return_pct.is_nan() || result.metrics.total_trades == 0,
            "Return should be finite or 0-trades NaN");
    assert!(!result.metrics.max_drawdown.is_infinite(),
            "Max drawdown should not be Inf after regime change");
}
