# DollarBill Unit Test Suggestions

## 1. Pricing Models Tests

### `tests/unit/models/test_black_scholes.rs`
- **test_call_option_atm()** - Verify at-the-money call option pricing accuracy
- **test_put_option_atm()** - Verify at-the-money put option pricing accuracy
- **test_put_call_parity()** - Ensure C - P = S - K*e^(-r*T) holds
- **test_deep_itm_call()** - Deep in-the-money call approaches S - K*e^(-r*T)
- **test_deep_otm_put()** - Deep out-of-the-money put approaches zero
- **test_zero_volatility()** - Option value equals intrinsic value when σ = 0
- **test_zero_time_to_expiry()** - Returns intrinsic value when T = 0
- **test_negative_time_handling()** - Proper error handling for invalid time
- **test_negative_volatility_handling()** - Proper error handling for invalid volatility
- **test_extreme_strike_prices()** - Handles very high and very low strikes
- **test_dividend_yield_impact()** - Verify dividend yield reduces call prices
- **test_interest_rate_impact()** - Verify rate changes affect option prices correctly

### `tests/unit/models/test_greeks.rs`
- **test_call_delta_range()** - Call delta between 0 and 1
- **test_put_delta_range()** - Put delta between -1 and 0
- **test_gamma_symmetry()** - Gamma same for calls and puts
- **test_gamma_positive()** - Gamma always positive for long options
- **test_vega_positive()** - Vega always positive
- **test_vega_symmetry()** - Vega same for calls and puts
- **test_theta_negative_long()** - Theta negative for long options
- **test_rho_sign()** - Rho positive for calls, negative for puts
- **test_atm_gamma_maximum()** - Gamma highest at-the-money
- **test_delta_put_call_relationship()** - Call delta - Put delta = 1
- **test_greeks_numerical_stability()** - No NaN or Inf values
- **test_greeks_with_zero_expiry()** - Proper handling at expiration

### `tests/unit/models/test_heston.rs`
- **test_heston_reduces_to_bs()** - With σ_vol = 0, approximates Black-Scholes
- **test_heston_call_put_parity()** - Satisfies put-call parity
- **test_feller_condition()** - Validates 2κθ > σ² for stability
- **test_heston_parameter_bounds()** - All parameters within valid ranges
- **test_heston_with_zero_correlation()** - ρ = 0 case
- **test_heston_with_perfect_correlation()** - ρ = ±1 boundary cases
- **test_carr_madan_integration()** - FFT integration converges
- **test_characteristic_function()** - Complex characteristic function accuracy
- **test_heston_vs_monte_carlo()** - Compare with MC simulation (within tolerance)
- **test_heston_smile_generation()** - Generates proper volatility smile
- **test_heston_numerical_stability()** - No overflow in complex exponentials
- **test_heston_extreme_parameters()** - Handles edge cases gracefully

## 2. Calibration Tests

### `tests/unit/calibration/test_nelder_mead.rs`
- **test_optimize_rosenbrock()** - Rosenbrock function (minimum at [1,1])
- **test_optimize_sphere()** - Sphere function x² + y² (minimum at [0,0])
- **test_optimize_beale()** - Beale function test
- **test_convergence_criteria()** - Stops when tolerance reached
- **test_max_iterations()** - Respects iteration limit
- **test_initial_simplex_formation()** - Proper simplex initialization
- **test_reflection_step()** - Reflection coefficient works correctly
- **test_expansion_step()** - Expansion when finding better point
- **test_contraction_step()** - Contraction when no improvement
- **test_shrinkage_step()** - Shrink toward best point
- **test_parameter_bounds_enforcement()** - Parameters stay within bounds
- **test_optimizer_with_noisy_function()** - Handles noise in objective

### `tests/unit/calibration/test_heston_calibration.rs`
- **test_calibrate_to_synthetic_data()** - Recover known Heston parameters
- **test_calibration_convergence()** - RMSE improves over iterations
- **test_calibration_with_sparse_data()** - Works with few data points
- **test_calibration_with_dense_data()** - Handles large option chains
- **test_parameter_stability()** - Consistent results on similar data
- **test_outlier_rejection()** - Handles extreme market prices
- **test_weighted_calibration()** - Weight by bid-ask spread or volume
- **test_multi_expiry_calibration()** - Calibrate across multiple maturities
- **test_calibration_speed()** - Completes within reasonable time
- **test_parallel_calibration()** - Multi-symbol parallel processing
- **test_calibration_error_metrics()** - RMSE, MAE, max error calculation
- **test_initial_guess_impact()** - Different starting points converge

### `tests/unit/calibration/test_market_option.rs`
- **test_moneyness_calculation()** - strike / spot ratio
- **test_time_to_expiry()** - Correct business days calculation
- **test_bid_ask_spread()** - Spread calculation
- **test_mid_price()** - (bid + ask) / 2
- **test_liquidity_filter()** - Filter by volume and open interest
- **test_option_chain_parsing()** - Load from JSON correctly
- **test_expired_option_handling()** - Skip expired options
- **test_invalid_strike_handling()** - Handle negative or zero strikes

## 3. Market Data Tests

### `tests/unit/market_data/test_csv_loader.rs`
- **test_load_valid_csv()** - Successfully load well-formed CSV
- **test_load_missing_file()** - Return error for non-existent file
- **test_load_empty_csv()** - Handle empty file gracefully
- **test_load_invalid_format()** - Detect malformed CSV
- **test_missing_columns()** - Error on missing required columns
- **test_extra_columns()** - Ignore extra columns
- **test_date_parsing()** - Parse various date formats
- **test_price_data_validation()** - Reject negative prices
- **test_volume_data_validation()** - Handle zero or missing volume
- **test_corporate_actions()** - Handle splits and dividends

### `tests/unit/market_data/test_options_json_loader.rs`
- **test_load_options_chain()** - Load JSON options data
- **test_parse_call_options()** - Extract call options correctly
- **test_parse_put_options()** - Extract put options correctly
- **test_missing_fields()** - Handle incomplete JSON
- **test_invalid_json()** - Detect malformed JSON
- **test_filter_by_expiry()** - Select specific expiration dates
- **test_filter_by_strike_range()** - Filter strikes by range
- **test_liquidity_filtering()** - Apply volume/OI filters
- **test_sort_by_strike()** - Options sorted by strike price

### `tests/unit/market_data/test_real_market_data.rs`
- **test_fetch_stock_quote()** - Get current stock price
- **test_fetch_options_chain()** - Download live options
- **test_api_rate_limiting()** - Respect API limits
- **test_network_error_handling()** - Graceful failure on timeout
- **test_invalid_symbol()** - Handle unknown tickers
- **test_market_hours_check()** - Detect if market is open
- **test_data_staleness()** - Flag outdated data
- **test_cache_mechanism()** - Use cached data when appropriate

### `tests/unit/market_data/test_symbols.rs`
- **test_symbol_validation()** - Valid ticker format
- **test_symbol_normalization()** - Convert to uppercase, trim
- **test_multi_symbol_processing()** - Batch symbol handling
- **test_symbol_metadata()** - Sector, market classification

## 4. Personality System Tests

### `tests/unit/personality/test_stock_classifier.rs`
- **test_classify_momentum_leader()** - Identify trending stocks
- **test_classify_mean_reverting()** - Identify oscillating stocks
- **test_classify_high_volatility()** - Detect high σ stocks
- **test_classify_low_volatility()** - Detect low σ stocks
- **test_classify_balanced()** - Identify neutral behavior
- **test_insufficient_data()** - Handle short histories
- **test_volatility_calculation()** - Rolling volatility accuracy
- **test_trend_detection()** - Moving average crossovers
- **test_mean_reversion_detection()** - Oscillation around mean
- **test_personality_confidence_score()** - Classification certainty
- **test_personality_transitions()** - Detect behavior changes
- **test_multi_timeframe_analysis()** - Consistent across periods

### `tests/unit/personality/test_performance_matrix.rs`
- **test_record_trade()** - Add trade to history
- **test_calculate_strategy_performance()** - Aggregate P&L by strategy
- **test_best_strategy_for_personality()** - Recommend optimal strategy
- **test_learning_over_time()** - Performance improves with data
- **test_strategy_comparison()** - Compare multiple strategies
- **test_confidence_intervals()** - Statistical significance
- **test_regime_change_detection()** - Adapt to market shifts
- **test_export_performance_report()** - Generate analytics

### `tests/unit/personality/test_matching.rs`
- **test_match_momentum_to_trend_strategy()** - Correct pairing
- **test_match_mean_revert_to_fade_strategy()** - Correct pairing
- **test_match_high_vol_to_vol_strategy()** - Correct pairing
- **test_strategy_override()** - Manual strategy selection
- **test_ensemble_strategy()** - Weighted combination
- **test_strategy_rotation()** - Switch based on performance
- **test_confidence_threshold()** - Only trade high-confidence signals

## 5. Backtesting Tests

### `tests/unit/backtesting/test_engine.rs`
- **test_engine_initialization()** - Set starting capital
- **test_open_position()** - Create new position
- **test_close_position()** - Exit existing position
- **test_update_position()** - Modify position size
- **test_mark_to_market()** - Update position values
- **test_commission_calculation()** - Apply trading fees
- **test_slippage_modeling()** - Simulate execution slippage
- **test_margin_requirements()** - Enforce margin rules
- **test_position_sizing()** - Respect max position size
- **test_portfolio_value()** - Calculate total equity
- **test_available_capital()** - Cash available for trading
- **test_leverage_calculation()** - Total exposure / capital

### `tests/unit/backtesting/test_metrics.rs`
- **test_total_return()** - (Final - Initial) / Initial
- **test_sharpe_ratio()** - Risk-adjusted return
- **test_sortino_ratio()** - Downside deviation
- **test_max_drawdown()** - Peak to trough decline
- **test_calmar_ratio()** - Return / max drawdown
- **test_win_rate()** - Winning trades / total trades
- **test_profit_factor()** - Gross profit / gross loss
- **test_average_win_loss()** - Mean P&L per trade
- **test_expectancy()** - Expected value per trade
- **test_recovery_factor()** - Net profit / max drawdown
- **test_equity_curve()** - Portfolio value over time
- **test_underwater_curve()** - Drawdown over time

### `tests/unit/backtesting/test_position.rs`
- **test_position_creation()** - Initialize new position
- **test_position_pnl()** - Calculate unrealized P&L
- **test_position_greeks()** - Aggregate Greeks
- **test_position_expiry()** - Handle option expiration
- **test_position_rollover()** - Roll to next expiry
- **test_assignment_risk()** - Deep ITM assignment
- **test_early_exercise()** - American option exercise

### `tests/unit/backtesting/test_trade.rs`
- **test_trade_entry()** - Record entry price and time
- **test_trade_exit()** - Record exit price and time
- **test_holding_period()** - Calculate days held
- **test_trade_return()** - (Exit - Entry) / Entry
- **test_trade_greeks()** - Greeks at entry and exit
- **test_trade_commission()** - Apply fees to P&L
- **test_trade_tags()** - Categorize trades (strategy, setup)

## 6. Strategy Tests

### `tests/unit/strategies/test_vol_mean_reversion.rs`
- **test_identify_high_iv()** - Detect elevated IV
- **test_identify_low_iv()** - Detect depressed IV
- **test_mean_calculation()** - Rolling IV average
- **test_standard_deviation()** - IV volatility
- **test_z_score_calculation()** - (IV - Mean) / StdDev
- **test_entry_signal_threshold()** - Signal when |Z| > threshold
- **test_exit_signal()** - Close when IV reverts
- **test_position_sizing()** - Size based on confidence
- **test_stop_loss()** - Exit on adverse moves
- **test_take_profit()** - Exit on target reached

### `tests/unit/strategies/test_momentum.rs`
- **test_uptrend_detection()** - Price > MA
- **test_downtrend_detection()** - Price < MA
- **test_momentum_calculation()** - Rate of change
- **test_strength_indicator()** - RSI or similar
- **test_entry_signal()** - Buy on breakout
- **test_exit_signal()** - Sell on reversal
- **test_trend_following()** - Stay with trend
- **test_whipsaw_filter()** - Avoid false signals

### `tests/unit/strategies/test_strategy_registry.rs`
- **test_register_strategy()** - Add strategy to registry
- **test_get_strategy_by_name()** - Retrieve by identifier
- **test_list_available_strategies()** - Enumerate all
- **test_strategy_factory()** - Create from JSON config
- **test_ensemble_strategy()** - Combine multiple strategies
- **test_strategy_weights()** - Weight contributions
- **test_strategy_performance_tracking()** - Record results

## 7. Portfolio Risk Tests

### `tests/unit/portfolio/test_risk_metrics.rs`
- **test_portfolio_delta()** - Aggregate delta across positions
- **test_portfolio_gamma()** - Aggregate gamma
- **test_portfolio_vega()** - Aggregate vega
- **test_portfolio_theta()** - Aggregate theta decay
- **test_delta_neutral_check()** - |Delta| < threshold
- **test_gamma_scalping()** - Positive gamma benefits
- **test_vega_hedging()** - Offset volatility risk
- **test_theta_decay_management()** - Time decay impact
- **test_correlation_effects()** - Position correlations
- **test_concentration_risk()** - Single position limits
- **test_sector_exposure()** - Diversification by sector
- **test_var_calculation()** - Value at risk (95%, 99%)

### `tests/unit/portfolio/test_hedging.rs`
- **test_delta_hedge_recommendation()** - Shares to hedge
- **test_vega_hedge_recommendation()** - Options to hedge
- **test_gamma_hedge_recommendation()** - Convexity hedge
- **test_hedge_ratio_calculation()** - Optimal hedge size
- **test_dynamic_hedging()** - Adjust hedges as Greeks change
- **test_cross_hedge()** - Hedge with correlated instruments

## 8. Configuration Tests

### `tests/unit/config/test_stocks_config.rs`
- **test_load_config()** - Parse stocks.json
- **test_filter_enabled_stocks()** - Return only enabled
- **test_get_stock_by_symbol()** - Find by ticker
- **test_validate_stock_data()** - Check required fields
- **test_market_classification()** - US vs EU markets
- **test_sector_classification()** - Tech, Finance, etc.
- **test_config_modification()** - Enable/disable stocks
- **test_invalid_config()** - Handle malformed JSON

### `tests/unit/config/test_personality_config.rs`
- **test_load_personality_config()** - Parse config file
- **test_classification_thresholds()** - Vol, trend thresholds
- **test_lookback_periods()** - Analysis windows
- **test_confidence_thresholds()** - Minimum confidence
- **test_strategy_mapping()** - Personality → Strategy

### `tests/unit/config/test_ml_config.rs`
- **test_load_ml_config()** - Parse ML settings
- **test_model_paths()** - Verify model file locations
- **test_feature_engineering()** - Define features
- **test_training_parameters()** - Epochs, batch size, etc.
- **test_evaluation_metrics()** - Accuracy, precision, recall

## 9. Utility Tests

### `tests/unit/utils/test_vol_surface.rs`
- **test_implied_volatility_solver()** - Newton-Raphson IV
- **test_iv_convergence()** - Solver converges quickly
- **test_iv_initial_guess()** - Good starting point
- **test_smile_interpolation()** - Interpolate between strikes
- **test_term_structure()** - IV across maturities
- **test_surface_smoothing()** - Remove noise
- **test_arbitrage_detection()** - No calendar/butterfly arb
- **test_export_surface_data()** - Save to CSV

### `tests/unit/utils/test_action_table_out.rs`
- **test_format_trade_signal()** - Pretty print signal
- **test_format_greeks()** - Display Greeks table
- **test_format_portfolio_risk()** - Risk metrics table
- **test_color_coding()** - Highlight important values
- **test_decimal_precision()** - Appropriate rounding

### `tests/unit/utils/test_pnl_output.rs`
- **test_calculate_trade_pnl()** - Single trade P&L
- **test_calculate_position_pnl()** - Position P&L
- **test_calculate_portfolio_pnl()** - Total P&L
- **test_pnl_attribution()** - P&L by strategy, symbol
- **test_cumulative_pnl()** - Running total over time
- **test_pnl_charts()** - Generate equity curve

## 10. Integration Tests

### `tests/integration/test_full_pipeline.rs`
- **test_data_fetch_to_signals()** - End-to-end pipeline
- **test_multi_symbol_calibration()** - Parallel processing
- **test_signal_generation_with_greeks()** - Complete workflow
- **test_portfolio_risk_analysis()** - Aggregate risk
- **test_backtest_full_strategy()** - Historical simulation
- **test_paper_trading_execution()** - Live paper trades
- **test_personality_pipeline()** - Classify → Match → Trade

### `tests/integration/test_alpaca_integration.rs`
- **test_alpaca_authentication()** - API key validation
- **test_fetch_account_info()** - Get account details
- **test_fetch_positions()** - Current positions
- **test_submit_order()** - Place paper trade
- **test_cancel_order()** - Cancel pending order
- **test_get_order_status()** - Check order fill
- **test_market_hours_check()** - Verify trading hours
- **test_rate_limiting()** - Respect API limits

### `tests/integration/test_personality_bot.rs`
- **test_bot_initialization()** - Start trading bot
- **test_continuous_trading()** - Run multiple iterations
- **test_signal_generation()** - Generate live signals
- **test_strategy_selection()** - Choose by personality
- **test_risk_management()** - Apply position limits
- **test_dry_run_mode()** - Test without trading
- **test_error_recovery()** - Handle exceptions gracefully

## 11. Performance Tests

### `tests/performance/test_pricing_speed.rs`
- **bench_black_scholes()** - BS pricing benchmark
- **bench_heston_pricing()** - Heston pricing benchmark
- **bench_greeks_calculation()** - Greeks computation
- **bench_parallel_calibration()** - Multi-symbol speed
- **bench_iv_solver()** - Implied volatility solver
- **bench_memory_usage()** - Memory profiling

### `tests/performance/test_data_processing.rs`
- **bench_csv_loading()** - CSV parsing speed
- **bench_json_parsing()** - JSON parsing speed
- **bench_options_chain_processing()** - Chain processing
- **bench_vol_surface_calculation()** - Surface generation

## 12. Error Handling Tests

### `tests/unit/errors/test_error_types.rs`
- **test_data_loading_error()** - File not found
- **test_network_error()** - API timeout
- **test_calibration_error()** - Failed to converge
- **test_invalid_parameter_error()** - Out of bounds
- **test_insufficient_data_error()** - Too few points
- **test_numerical_error()** - NaN or Inf
- **test_error_propagation()** - Errors bubble up
- **test_error_messages()** - Helpful error text

## 13. Edge Case Tests

### `tests/unit/edge_cases/test_boundary_conditions.rs`
- **test_zero_stock_price()** - S = 0
- **test_zero_strike()** - K = 0
- **test_zero_rate()** - r = 0
- **test_zero_volatility()** - σ = 0
- **test_zero_time()** - T = 0
- **test_negative_inputs()** - Reject invalid data
- **test_very_large_numbers()** - No overflow
- **test_very_small_numbers()** - No underflow
- **test_extreme_greeks()** - Handle edge cases

## 14. ML Integration Tests (if implementing ML features)

### `tests/unit/ml/test_volatility_prediction.rs`
- **test_lstm_model_loading()** - Load trained model
- **test_feature_preparation()** - Prepare input features
- **test_prediction_generation()** - Generate forecasts
- **test_prediction_accuracy()** - Compare to actual
- **test_model_confidence()** - Output confidence scores
- **test_model_retraining()** - Update with new data

### `tests/unit/ml/test_signal_classification.rs`
- **test_signal_quality_scoring()** - ML score quality
- **test_classification_accuracy()** - Win/loss prediction
- **test_feature_importance()** - Identify key features
- **test_model_calibration()** - Probability calibration

## Test Infrastructure

### `tests/helpers/mod.rs`
- **generate_synthetic_stock_data()** - Create test price series
- **generate_synthetic_options_chain()** - Create test option data
- **create_test_config()** - Mock configuration
- **mock_market_data_api()** - Mock external APIs
- **assert_greeks_valid()** - Custom assertion for Greeks
- **assert_price_reasonable()** - Sanity check prices
- **setup_test_environment()** - Initialize test fixtures
- **teardown_test_environment()** - Cleanup after tests

### `tests/fixtures/`
- **test_stock_data.csv** - Sample historical prices
- **test_options_chain.json** - Sample options data
- **test_config.json** - Test configuration
- **test_heston_params.json** - Known calibration results
- **test_backtest_data.csv** - Backtest input data

## Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests only
cargo test --test integration

# Run specific test file
cargo test test_black_scholes

# Run with output
cargo test -- --nocapture

# Run with specific test
cargo test test_call_option_atm

# Run benchmarks
cargo bench

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage
```

## Coverage Goals

- **Pricing Models**: 95%+ coverage (critical path)
- **Calibration**: 90%+ coverage (core functionality)
- **Market Data**: 85%+ coverage (I/O heavy)
- **Backtesting**: 90%+ coverage (validation critical)
- **Strategies**: 85%+ coverage (business logic)
- **Utilities**: 80%+ coverage (helpers)
- **Overall Target**: 85%+ coverage

## Priority Testing Order

1. **Critical Path** (P0):
   - Black-Scholes pricing and Greeks
   - Heston pricing
   - Calibration (Nelder-Mead, Heston)
   - Backtest engine

2. **Core Features** (P1):
   - Market data loading
   - Signal generation
   - Portfolio risk metrics
   - Personality classification

3. **Supporting Features** (P2):
   - Configuration management
   - Volatility surface
   - Strategy registry
   - Output formatting

4. **Integration** (P3):
   - Full pipeline tests
   - Alpaca integration
   - Personality-based bot
   - Performance benchmarks
