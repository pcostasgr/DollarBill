# Parameter Atlas

Complete reference guide for all configuration parameters in the DollarBill trading platform.

## stocks.json - Symbol Configuration

Central repository for all tradable symbols:

- **symbol**: Stock ticker symbol (e.g., "TSLA", "AAPL")
- **market**: Market identifier ("US", "EU", etc.)
- **sector**: Industry sector classification
- **enabled**: Boolean flag to include/exclude symbol from trading
- **notes**: Optional descriptive notes about the symbol

## trading_bot_config.json - Live Trading Bot Configuration

### Trading Section
- **position_size_shares**: Number of shares per position (default: 100)
- **max_positions**: Maximum concurrent positions allowed (default: 3)

### Risk Management Sub-section
- **stop_loss_pct**: Stop loss percentage threshold (default: 20.0%)
- **take_profit_pct**: Take profit percentage threshold (default: 50.0%)
- **max_daily_trades**: Maximum trades allowed per day (default: 5)

### Signals Section
- **rsi_period**: Period for RSI calculation (default: 14)
- **momentum_period**: Period for momentum calculation (default: 5)

#### Volatility Thresholds Sub-section
- **high_vol_threshold**: Threshold for high volatility regime (default: 0.50 or 50%)
- **medium_vol_threshold**: Threshold for medium volatility regime (default: 0.35 or 35%)

#### Thresholds by Volatility Regime
**High Volatility:**
- **rsi_oversold**: RSI level for oversold signals (default: 40.0)
- **rsi_overbought**: RSI level for overbought signals (default: 60.0)
- **momentum_threshold**: Momentum change threshold (default: 0.03 or 3%)

**Medium Volatility:**
- **rsi_oversold**: RSI level for oversold signals (default: 35.0)
- **rsi_overbought**: RSI level for overbought signals (default: 65.0)
- **momentum_threshold**: Momentum change threshold (default: 0.02 or 2%)

### Execution Section
- **continuous_mode_interval_minutes**: Minutes between trading cycles (default: 5)
- **data_lookback_days**: Days of historical data to analyze (default: 60)

## paper_trading_config.json - Paper Trading Simulation Configuration

### Trading Section
- **position_size_shares**: Number of shares per position (default: 5.0)
- **max_positions**: Maximum concurrent positions allowed (default: 3)

### Risk Management Sub-section
- **stop_loss_pct**: Stop loss percentage threshold (default: 15.0%)
- **take_profit_pct**: Take profit percentage threshold (default: 40.0%)

### Signals Section
- **rsi_period**: Period for RSI calculation (default: 14)
- **momentum_period**: Period for momentum calculation (default: 5)

#### Volatility Thresholds Sub-section
- **high_vol_threshold**: Threshold for high volatility regime (default: 0.50 or 50%)
- **medium_vol_threshold**: Threshold for medium volatility regime (default: 0.35 or 35%)

#### Thresholds by Volatility Regime
**High Volatility:**
- **rsi_oversold**: RSI level for oversold signals (default: 40.0)
- **rsi_overbought**: RSI level for overbought signals (default: 60.0)
- **momentum_threshold**: Momentum change threshold (default: 0.03 or 3%)

**Medium Volatility:**
- **rsi_oversold**: RSI level for oversold signals (default: 35.0)
- **rsi_overbought**: RSI level for overbought signals (default: 65.0)
- **momentum_threshold**: Momentum change threshold (default: 0.02 or 2%)

### Paper Trading Section
- **initial_balance**: Starting account balance in dollars (default: 10000.0)
- **commission_per_trade**: Commission cost per trade (default: 0.0 for paper trading)
- **data_lookback_days**: Days of historical data to use (default: 60)
- **simulation_days**: Number of days to simulate (default: 30)

## signals_config.json - Options Signals Analysis Configuration

### Analysis Section
- **risk_free_rate**: Risk-free interest rate for pricing (default: 0.05 or 5%)

#### Liquidity Filters Sub-section
- **min_volume**: Minimum option volume required (default: 50)
- **max_spread_pct**: Maximum bid-ask spread percentage (default: 10.0%)

#### Edge Thresholds Sub-section
- **min_edge_dollars**: Minimum edge in dollars for signals (default: 0.10)
- **min_delta**: Minimum delta exposure required (default: 0.05)

### Calibration Section
- **tolerance**: Convergence tolerance for optimization (default: 1e-6)
- **max_iterations**: Maximum iterations for calibration (default: 100)

### Options Section
- **default_time_to_expiry_days**: Default option expiry in days (default: 30)
- **min_time_to_expiry_days**: Minimum acceptable expiry (default: 7)
- **max_time_to_expiry_days**: Maximum acceptable expiry (default: 90)

## vol_surface_config.json - Volatility Surface Analysis Configuration

### Volatility Surface Section
- **risk_free_rate**: Risk-free interest rate for pricing (default: 0.05 or 5%)

#### Analysis Sub-section
- **min_strikes_around_atm**: Minimum strikes around at-the-money (default: 3)
- **max_strikes_around_atm**: Maximum strikes around at-the-money (default: 10)
- **moneyness_tolerance**: Tolerance for moneyness matching (default: 0.05)

#### Calibration Sub-section
- **tolerance**: Convergence tolerance for optimization (default: 1e-6)
- **max_iterations**: Maximum iterations for calibration (default: 100)
- **initial_vol_guess**: Initial volatility guess for optimization (default: 0.3 or 30%)

## strategy_config.json - Strategy Parameters Configuration

### Backtest Section
- **commission_per_trade**: Commission cost per trade in dollars (default: 1.0)
- **risk_free_rate**: Risk-free interest rate for discounting (default: 0.05 or 5%)
- **max_positions**: Maximum number of concurrent positions allowed (default: 5)
- **position_size_pct**: Percentage of capital to allocate per position (default: 20.0%)
- **stop_loss_pct**: Stop loss percentage threshold (default: 50.0%)
- **take_profit_pct**: Take profit percentage threshold (default: 100.0%)

### Strategies Section

#### Short-Term Strategy (14-day expiry)
- **initial_capital**: Starting capital in dollars (default: 100000.0)
- **days_to_expiry**: Target days until option expiry (default: 14)
- **max_days_hold**: Maximum days to hold position (default: 10)
- **vol_threshold_high_vol**: High volatility threshold (default: 0.35 or 35%)
- **vol_threshold_medium_vol**: Medium volatility threshold (default: 0.30 or 30%)
- **vol_threshold_low_vol**: Low volatility threshold (default: 0.25 or 25%)

#### Medium-Term Strategy (30-day expiry)
- **initial_capital**: Starting capital in dollars (default: 100000.0)
- **days_to_expiry**: Target days until option expiry (default: 30)
- **max_days_hold**: Maximum days to hold position (default: 21)
- **rsi_oversold**: RSI oversold level (default: 35.0)
- **rsi_overbought**: RSI overbought level (default: 65.0)
- **momentum_threshold**: Momentum change threshold (default: 0.01 or 1%)
- **vol_zscore_lookback**: Lookback period for volatility z-score (default: 20)

##### Strike Offsets Sub-section
- **call_otm_pct**: Call option out-of-the-money percentage (default: 1.03 or 3% OTM)
- **put_otm_pct**: Put option out-of-the-money percentage (default: 0.97 or 3% OTM)

##### Momentum Breakout Sub-Strategy
- **momentum_threshold**: Momentum threshold for breakout (default: 0.025 or 2.5%)
- **vol_zscore_threshold**: Volatility z-score threshold (default: 0.5)
- **call_otm_pct**: Call strike offset (default: 1.02 or 2% OTM)
- **put_otm_pct**: Put strike offset (default: 0.98 or 2% OTM)

#### Long-Term Strategy (60-day expiry)
- **initial_capital**: Starting capital in dollars (default: 100000.0)
- **days_to_expiry**: Target days until option expiry (default: 60)
- **max_days_hold**: Maximum days to hold position (default: 45)
- **rsi_period**: Period for RSI calculation (default: 14)
- **momentum_period**: Period for momentum calculation (default: 10)
- **vol_zscore_lookback**: Lookback period for volatility z-score (default: 20)

##### Volatility Thresholds Sub-section
- **high_vol_threshold**: High volatility threshold (default: 50.0)
- **medium_vol_threshold**: Medium volatility threshold (default: 35.0)

##### RSI Momentum Thresholds by Volatility Regime
**High Volatility:**
- **rsi_oversold**: RSI oversold level (default: 45.0)
- **rsi_overbought**: RSI overbought level (default: 55.0)
- **momentum_threshold**: Momentum threshold (default: 0.003 or 0.3%)

**Medium Volatility:**
- **rsi_oversold**: RSI oversold level (default: 40.0)
- **rsi_overbought**: RSI overbought level (default: 60.0)
- **momentum_threshold**: Momentum threshold (default: 0.007 or 0.7%)

**Low Volatility:**
- **rsi_oversold**: RSI oversold level (default: 35.0)
- **rsi_overbought**: RSI overbought level (default: 65.0)
- **momentum_threshold**: Momentum threshold (default: 0.01 or 1%)

##### Strike Offsets Sub-section
- **call_otm_pct**: Call strike offset (default: 1.02 or 2% OTM)
- **put_otm_pct**: Put strike offset (default: 0.98 or 2% OTM)

##### Momentum Breakout Sub-Strategy
- **momentum_threshold**: Momentum threshold (default: 0.02 or 2%)
- **vol_zscore_threshold**: Volatility z-score threshold (default: 0.1)
- **call_otm_pct**: Call strike offset (default: 1.01 or 1% OTM)
- **put_otm_pct**: Put strike offset (default: 0.99 or 1% OTM)

##### RSI Divergence Sub-Strategy
- **rsi_oversold**: RSI oversold level for divergence (default: 45.0)
- **rsi_overbought**: RSI overbought level for divergence (default: 55.0)
- **momentum_threshold**: Momentum threshold for divergence (default: 0.005 or 0.5%)
- **call_otm_pct**: Call strike offset for divergence (default: 1.015 or 1.5% OTM)
- **put_otm_pct**: Put strike offset for divergence (default: 0.985 or 1.5% OTM)

## Implemented Strategy Parameters

### Volatility Mean Reversion Strategy
- **zscore_threshold**: Minimum z-score deviation from mean volatility for signals (default: 1.5)
- **edge_threshold**: Minimum edge (market IV - model IV) for signals (default: 0.05 or 5%)

## Notes

- All percentage values are expressed as decimals (e.g., 0.35 = 35%)
- Volatility thresholds adjust strategy behavior based on market conditions
- Strike offsets determine how far out-of-the-money options are selected
- RSI (Relative Strength Index) identifies momentum and overbought/oversold conditions
- Z-score calculations identify statistically significant deviations from normal levels
- Configuration files are designed to separate concerns: symbols, trading parameters, signals, and strategies