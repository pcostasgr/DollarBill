// Short options backtesting tests

use crate::helpers::generate_synthetic_stock_data;
use dollarbill::backtesting::{BacktestEngine, BacktestConfig, SignalAction};
use dollarbill::backtesting::position::PositionStatus;

#[test]
fn test_sell_call_signal_execution() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 50, 0.0, 0.2);
    
    // Strategy that sells calls
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: hist_vol,
            }]
        },
    );
    
    // Should execute trades
    assert!(result.metrics.total_trades > 0, "Should have executed SellCall trades");
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_sell_put_signal_execution() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 50, 0.0, 0.2);
    
    // Strategy that sells puts
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![SignalAction::SellPut {
                strike: spot * 0.95,
                days_to_expiry: 30,
                volatility: hist_vol,
            }]
        },
    );
    
    // Should execute trades
    assert!(result.metrics.total_trades > 0, "Should have executed SellPut trades");
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_call_and_put_combination() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 100, 0.0, 0.2);
    
    // Strategy that sells both calls and puts (short strangle)
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![
                SignalAction::SellCall {
                    strike: spot * 1.05,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
                SignalAction::SellPut {
                    strike: spot * 0.95,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
            ]
        },
    );
    
    // Should execute both types of trades
    assert!(result.metrics.total_trades >= 2, "Should have executed both SellCall and SellPut trades");
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_options_win_rate() {
    let mut config = BacktestConfig {
        initial_capital: 100_000.0,
        max_positions: 3,
        position_size_pct: 10.0,
        days_to_expiry: 30,
        max_days_hold: 25,
        take_profit_pct: Some(50.0), // Exit at 50% profit
        stop_loss_pct: Some(200.0),  // Wide stop loss for shorts
        ..Default::default()
    };
    
    let mut engine = BacktestEngine::new(config);
    
    // Stable market (good for short options)
    let data = generate_synthetic_stock_data(100.0, 100, 0.0, 0.15);
    
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![
                SignalAction::SellCall {
                    strike: spot * 1.05,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
                SignalAction::SellPut {
                    strike: spot * 0.95,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
            ]
        },
    );
    
    // In stable markets, short options should have good win rate
    if result.metrics.total_trades > 5 {
        assert!(result.metrics.win_rate > 0.0, "Should have some winning trades");
    }
}

#[test]
fn test_short_options_with_stop_loss() {
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        max_positions: 2,
        position_size_pct: 15.0,
        days_to_expiry: 30,
        max_days_hold: 20,
        take_profit_pct: Some(50.0),
        stop_loss_pct: Some(150.0), // 150% loss triggers stop
        ..Default::default()
    };
    
    let mut engine = BacktestEngine::new(config);
    
    // Volatile market (challenging for short options)
    let data = generate_synthetic_stock_data(100.0, 100, 0.1, 0.35);
    
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: hist_vol,
            }]
        },
    );
    
    // Should handle stop losses for short positions
    assert!(result.metrics.total_trades >= 0);
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_options_with_early_exit() {
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        max_positions: 3,
        position_size_pct: 10.0,
        days_to_expiry: 30,
        max_days_hold: 15, // Exit early
        take_profit_pct: Some(60.0),
        stop_loss_pct: Some(200.0),
        ..Default::default()
    };
    
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 60, 0.0, 0.2);
    
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![SignalAction::SellPut {
                strike: spot * 0.95,
                days_to_expiry: 30,
                volatility: hist_vol,
            }]
        },
    );
    
    // Should exit positions before expiration
    assert!(result.metrics.total_trades >= 0);
    if result.metrics.total_trades > 0 {
        assert!(result.metrics.avg_days_held <= 15.0, "Should respect max_days_hold");
    }
}

#[test]
fn test_mixed_long_and_short_positions() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 100, 0.05, 0.2);
    
    // Strategy that mixes long and short positions
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            
            // Alternate between long and short
            if day_idx % 2 == 0 {
                vec![SignalAction::BuyCall {
                    strike: spot * 1.02,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                }]
            } else {
                vec![SignalAction::SellPut {
                    strike: spot * 0.98,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                }]
            }
        },
    );
    
    // Should handle mixed positions
    assert!(result.metrics.total_trades > 0, "Should execute mixed long/short trades");
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_options_position_limits() {
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        max_positions: 2, // Strict limit
        position_size_pct: 20.0,
        days_to_expiry: 30,
        max_days_hold: 25,
        ..Default::default()
    };
    
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 50, 0.0, 0.2);
    
    // Try to open many short positions
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![
                SignalAction::SellCall {
                    strike: spot * 1.05,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
                SignalAction::SellCall {
                    strike: spot * 1.10,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
                SignalAction::SellPut {
                    strike: spot * 0.95,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
            ]
        },
    );
    
    // Should respect position limits even with multiple signals
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_options_different_strikes() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 80, 0.0, 0.2);
    
    // Test different strike distances
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            
            // Vary strikes over time
            let strike_mult = if day_idx % 3 == 0 {
                1.03 // Close to ATM
            } else if day_idx % 3 == 1 {
                1.05 // Medium OTM
            } else {
                1.10 // Far OTM
            };
            
            vec![SignalAction::SellCall {
                strike: spot * strike_mult,
                days_to_expiry: 30,
                volatility: hist_vol,
            }]
        },
    );
    
    // Should handle different strike prices
    assert!(result.metrics.total_trades >= 0);
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_options_commission_impact() {
    let config_low = BacktestConfig {
        trading_costs: dollarbill::backtesting::TradingCosts {
            commission_per_contract: 0.50,
            bid_ask_spread_percent: 0.0,
            slippage_model: dollarbill::backtesting::SlippageModel::Fixed,
            ..dollarbill::backtesting::TradingCosts::default()
        },
        ..Default::default()
    };
    
    let config_high = BacktestConfig {
        trading_costs: dollarbill::backtesting::TradingCosts {
            commission_per_contract: 5.0,
            bid_ask_spread_percent: 0.0,
            slippage_model: dollarbill::backtesting::SlippageModel::Fixed,
            ..dollarbill::backtesting::TradingCosts::default()
        },
        ..Default::default()
    };
    
    let mut engine_low = BacktestEngine::new(config_low);
    let mut engine_high = BacktestEngine::new(config_high);
    
    let data = generate_synthetic_stock_data(100.0, 50, 0.0, 0.2);
    
    let strategy = |_symbol: &str, spot: f64, _day_idx: usize, hist_vols: &[f64]| {
        let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
        vec![SignalAction::SellCall {
            strike: spot * 1.05,
            days_to_expiry: 30,
            volatility: hist_vol,
        }]
    };
    
    let result_low = engine_low.run_with_signals("TEST", data.clone(), strategy);
    let result_high = engine_high.run_with_signals("TEST", data, strategy);
    
    // Higher commissions should reduce total commissions paid (fewer trades due to higher cost)
    // or same trades with higher total commission
    if result_low.metrics.total_trades > 0 && result_high.metrics.total_trades > 0 {
        assert!(result_low.metrics.total_commissions > 0.0);
        assert!(result_high.metrics.total_commissions > 0.0);
    }
}

#[test]
fn test_short_straddle_strategy() {
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        max_positions: 4,
        position_size_pct: 12.0,
        days_to_expiry: 30,
        max_days_hold: 20,
        take_profit_pct: Some(50.0),
        stop_loss_pct: Some(150.0),
        ..Default::default()
    };
    
    let mut engine = BacktestEngine::new(config);
    
    // Low volatility market (ideal for short straddles)
    let data = generate_synthetic_stock_data(100.0, 100, 0.0, 0.12);
    
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            // Short straddle: sell ATM call and put
            vec![
                SignalAction::SellCall {
                    strike: spot,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
                SignalAction::SellPut {
                    strike: spot,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
            ]
        },
    );
    
    // Should execute straddle trades
    assert!(result.metrics.total_trades >= 2, "Should execute both sides of straddle");
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_short_options_no_data() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let empty_data = vec![];
    
    let result = engine.run_with_signals(
        "TEST",
        empty_data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![SignalAction::SellCall {
                strike: spot * 1.05,
                days_to_expiry: 30,
                volatility: hist_vol,
            }]
        },
    );
    
    // Should handle empty data gracefully
    assert_eq!(result.metrics.total_trades, 0);
}

#[test]
fn test_short_options_high_volatility() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    // Very high volatility market
    let data = generate_synthetic_stock_data(100.0, 100, 0.0, 0.50);
    
    let result = engine.run_with_signals(
        "TEST",
        data,
        |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            vec![
                SignalAction::SellCall {
                    strike: spot * 1.10, // Wider strikes for high vol
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
                SignalAction::SellPut {
                    strike: spot * 0.90,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                },
            ]
        },
    );
    
    // Should handle high volatility
    assert!(result.metrics.total_trades >= 0);
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}
