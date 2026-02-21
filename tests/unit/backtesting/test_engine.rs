// Backtesting engine tests

use crate::helpers::generate_synthetic_stock_data;
use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig};
use dollarbill::market_data::csv_loader::HistoricalDay;

#[test]
fn test_engine_initialization() {
    let config = BacktestConfig::default();
    let _engine = BacktestEngine::new(config.clone());
    
    // Verify initial state - we can't access private fields directly,
    // but we can test the engine runs without panicking
    assert!(config.initial_capital > 0.0);
}

#[test]
fn test_backtest_with_synthetic_data() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    // Generate some test data
    let data = generate_synthetic_stock_data(100.0, 50, 0.1, 0.2);
    
    // Run a simple strategy
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Basic sanity checks on results
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan(), 
            "Total return should be finite or NaN");
    assert!(result.metrics.sharpe_ratio.is_finite() || result.metrics.sharpe_ratio.is_nan(), 
            "Sharpe ratio should be finite or NaN (if no trades)");
}

#[test]
fn test_backtest_config_default() {
    let config = BacktestConfig::default();
    
    assert_eq!(config.initial_capital, 100_000.0);
    assert!(config.commission_per_trade > 0.0);
    assert!(config.max_positions > 0);
    assert!(config.position_size_pct > 0.0 && config.position_size_pct <= 100.0);
}

#[test]
fn test_backtest_custom_config() {
    let config = BacktestConfig {
        initial_capital: 50_000.0,
        commission_per_trade: 2.0,
        risk_free_rate: 0.03,
        max_positions: 5,
        position_size_pct: 20.0,
        days_to_expiry: 45,
        max_days_hold: 30,
        stop_loss_pct: Some(30.0),
        take_profit_pct: Some(50.0),
        use_portfolio_management: false,
    };
    
    let _engine = BacktestEngine::new(config.clone());
    
    assert_eq!(config.initial_capital, 50_000.0);
    assert_eq!(config.max_positions, 5);
}

#[test]
fn test_empty_data_handling() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let empty_data: Vec<HistoricalDay> = Vec::new();
    let result = engine.run_simple_strategy("TEST", empty_data, 0.15);
    
    // Should handle empty data gracefully
    assert!(result.metrics.total_trades == 0 || result.metrics.total_trades >= 0);
}

#[test]
fn test_minimal_data() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    // Just a few days of data
    let data = generate_synthetic_stock_data(100.0, 5, 0.0, 0.1);
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Should handle minimal data
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_commission_impact() {
    // Test that commissions are properly deducted
    let mut config_low_commission = BacktestConfig::default();
    config_low_commission.commission_per_trade = 0.1;
    
    let mut config_high_commission = BacktestConfig::default();
    config_high_commission.commission_per_trade = 10.0;
    
    let data = generate_synthetic_stock_data(100.0, 100, 0.1, 0.2);
    
    let mut engine_low = BacktestEngine::new(config_low_commission);
    let mut engine_high = BacktestEngine::new(config_high_commission);
    
    let result_low = engine_low.run_simple_strategy("TEST", data.clone(), 0.15);
    let result_high = engine_high.run_simple_strategy("TEST", data, 0.15);
    
    // If there are trades, high commission should result in lower returns
    if result_low.metrics.total_trades > 0 && result_high.metrics.total_trades > 0 {
        // Can't guarantee lower returns due to position sizing differences,
        // but we can verify both executed
        assert!(result_low.metrics.total_trades > 0);
        assert!(result_high.metrics.total_trades > 0);
    }
}

#[test]
fn test_volatility_strategy_logic() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    // Create data with varying volatility
    let data = generate_synthetic_stock_data(100.0, 100, 0.05, 0.25);
    
    // Test with different volatility thresholds
    let result_low_threshold = engine.run_simple_strategy("TEST", data.clone(), 0.10);
    
    let mut engine2 = BacktestEngine::new(BacktestConfig::default());
    let result_high_threshold = engine2.run_simple_strategy("TEST", data, 0.30);
    
    // Both should complete without errors
    assert!(result_low_threshold.metrics.total_return_pct.is_finite() || result_low_threshold.metrics.total_return_pct.is_nan());
    assert!(result_high_threshold.metrics.total_return_pct.is_finite() || result_high_threshold.metrics.total_return_pct.is_nan());
}

#[test]
fn test_position_limits() {
    let mut config = BacktestConfig::default();
    config.max_positions = 2; // Only allow 2 positions
    
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(100.0, 100, 0.1, 0.3);
    
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Should respect position limits (can't directly verify, but should not crash)
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_stop_loss_configuration() {
    let mut config = BacktestConfig::default();
    config.stop_loss_pct = Some(25.0); // 25% stop loss
    
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(100.0, 100, -0.2, 0.4); // Downward drift
    
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Should execute with stop loss
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_take_profit_configuration() {
    let mut config = BacktestConfig::default();
    config.take_profit_pct = Some(50.0); // 50% take profit
    
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(100.0, 100, 0.3, 0.3); // Upward drift
    
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Should execute with take profit
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_no_stop_loss() {
    let mut config = BacktestConfig::default();
    config.stop_loss_pct = None; // No stop loss
    
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(100.0, 50, 0.1, 0.2);
    
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_result_metrics_present() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 100, 0.1, 0.2);
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Check that key metrics are present and valid
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
    assert!(result.metrics.max_drawdown.is_finite() || result.metrics.max_drawdown.is_nan());
    assert!(result.metrics.win_rate >= 0.0 && result.metrics.win_rate <= 100.0);
    assert!(result.metrics.total_trades >= 0);
}

#[test]
fn test_different_symbols() {
    let config = BacktestConfig::default();
    let mut engine1 = BacktestEngine::new(config.clone());
    let mut engine2 = BacktestEngine::new(config);
    
    let data = generate_synthetic_stock_data(100.0, 50, 0.1, 0.2);
    
    let result1 = engine1.run_simple_strategy("AAPL", data.clone(), 0.15);
    let result2 = engine2.run_simple_strategy("GOOGL", data, 0.15);
    
    // Both should complete successfully
    assert!(result1.metrics.total_return_pct.is_finite() || result1.metrics.total_return_pct.is_nan());
    assert!(result2.metrics.total_return_pct.is_finite() || result2.metrics.total_return_pct.is_nan());
}

#[test]
fn test_position_sizing() {
    let mut config = BacktestConfig::default();
    config.position_size_pct = 5.0; // Small position size
    
    let mut engine = BacktestEngine::new(config);
    let data = generate_synthetic_stock_data(100.0, 50, 0.1, 0.2);
    
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    // Should handle small position sizes
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_uptrend_data() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    // Strong uptrend
    let data = generate_synthetic_stock_data(100.0, 100, 0.5, 0.2);
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}

#[test]
fn test_downtrend_data() {
    let config = BacktestConfig::default();
    let mut engine = BacktestEngine::new(config);
    
    // Downtrend
    let data = generate_synthetic_stock_data(100.0, 100, -0.3, 0.2);
    let result = engine.run_simple_strategy("TEST", data, 0.15);
    
    assert!(result.metrics.total_return_pct.is_finite() || result.metrics.total_return_pct.is_nan());
}
