# Backtesting Guide

## How to Properly Backtest Options Strategies

### Available Backtesting Frameworks

The platform provides two backtesting frameworks for different pricing models:

#### 1. **Black-Scholes Backtesting** (`backtest_strategy.rs`)
- Uses Black-Scholes-Merton pricing with constant volatility
- Fast execution, good for strategy validation
- Suitable for directional strategies and basic options trading

#### 2. **Heston Stochastic Volatility Backtesting** (`backtest_heston.rs`) ⭐ NEW
- Uses advanced Heston model with Carr-Madan analytical pricing
- Accounts for volatility smiles and term structure
- Realistic option pricing for professional strategies
- Includes live market calibration for each symbol

### Key Principles

1. **Data Direction Matters**: Historical data must be ordered from **oldest to newest** (chronological order)
   - CSV loaders typically return newest-first (reverse chronological)
   - **Always call `.reverse()` on the data** before passing to backtest engine
   - Example: `historical_data.reverse()`

2. **Automatic Position Management**: The framework now prevents opening positions too close to the end of the backtest
   - Won't open new positions within `max_days_hold` days of backtest end
   - Ensures every position has enough future data to properly test exit conditions

3. **Realistic Multi-Day Testing**:
   - Set `days_to_expiry` to match your option contract duration (14/30/60 days typical)
   - Set `max_days_hold` to when you'd exit the position (10/21/45 days typical)
   - The framework will close positions at `max_days_hold` days, simulating realistic trading

### Recommended Configuration

```rust
let config = BacktestConfig {
    initial_capital: 100_000.0,
    commission_per_trade: 1.0,
    risk_free_rate: 0.05,
    max_positions: 5,
    position_size_pct: 20.0,

    days_to_expiry: 30,      // Option contract duration
    max_days_hold: 21,       // Maximum holding period (exit after this)

    stop_loss_pct: Some(50.0),    // Exit if down 50%
    take_profit_pct: Some(100.0), // Exit if up 100%
};
```

### Heston Model Setup

For Heston backtesting, you need calibrated parameters for each symbol:

```bash
# 1. Calibrate Heston parameters to live market data
cargo run --example calibrate_live_options

# 2. Run Heston backtest
cargo run --example backtest_heston
```

**Heston Parameters Include:**
- κ (kappa): Mean reversion speed
- θ (theta): Long-term variance
- σ (sigma): Volatility of volatility
- ρ (rho): Correlation between asset and variance
- v₀: Initial variance

### What the Framework Does

1. **Iterates forward through time** (oldest date first)
2. **Updates positions** daily with current spot price and volatility
3. **Checks exit conditions** every day:
   - Time-based: Close after `max_days_hold` days
   - Stop loss: Close if loss exceeds threshold
   - Take profit: Close if profit exceeds threshold
   - Approaching expiry: Close within 2 days of expiration
4. **Prevents late entries**: Won't open positions in last N days of backtest
5. **Closes remaining positions** at end of backtest period

### Example Results Interpretation

```
Avg Days Held: 21.5
```
- Good: Close to your `max_days_hold` setting
- Means positions are being held according to strategy (not hitting stop/profit early)

```
Avg Days Held: 3.2
```
- Positions closing early due to stop loss or take profit
- Consider adjusting thresholds or reviewing strategy signals

```
Avg Days Held: 219.0
```
- **Warning**: Positions held way too long - something's wrong!
- Check that data is properly reversed and max_days_hold is configured

### Heston vs Black-Scholes Results

**Heston Advantages:**
- Captures volatility smile/skew effects
- More realistic pricing for OTM/ITM options
- Better P&L attribution for volatility strategies

**Example Comparison:**
```
Black-Scholes: NVDA +150% (constant vol assumption)
Heston:        NVDA +270% (realistic vol dynamics)
```

### Common Issues

**Problem**: Positions held 300+ days
**Solution**: Make sure you called `.reverse()` on historical data

**Problem**: Very few trades executed
**Solution**: Your signal criteria may be too restrictive, or not enough data remaining for position management

**Problem**: All positions hitting stop loss
**Solution**: Strategy signals may be poor, or volatility estimates off - review entry conditions

**Problem**: Heston backtest shows catastrophic losses
**Solution**: Check that Heston parameters are properly calibrated and loaded

### Performance Metrics

- **Sharpe Ratio > 1**: Good risk-adjusted returns
- **Win Rate > 50%**: More winning trades than losers
- **Profit Factor > 1**: Winners outweigh losers in dollar terms
- **Max Drawdown < 30%**: Acceptable risk level

Lower values suggest strategy needs refinement!
