# Portfolio Management Integration Guide

This guide explains how the portfolio management system integrates with backtesting and live trading components.

## Overview

The portfolio management system (Option C) is now integrated into:
1. **Backtesting Engine** - Intelligent position sizing and risk limits during historical simulations
2. **Personality-Based Trading Bot** - Smart position sizing and pre-trade approval for live trading

## Integration Architecture

```
┌─────────────────────────────────────────────────────────┐
│                Portfolio Manager                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │ Position     │  │ Risk         │  │ Performance   │ │
│  │ Sizing       │  │ Analytics    │  │ Attribution   │ │
│  └──────────────┘  └──────────────┘  └───────────────┘ │
└────────────┬────────────────────┬────────────────────────┘
             │                    │
    ┌────────▼────────┐   ┌───────▼──────────┐
    │  Backtesting    │   │  Trading Bot     │
    │  Engine         │   │  (Live Trading)  │
    └─────────────────┘   └──────────────────┘
```

## Backtesting Integration

### How It Works

The backtesting engine now optionally uses the portfolio manager for:
- **Volatility-based position sizing** instead of fixed percentage
- **Portfolio-level risk checks** before opening positions
- **Greek-aware risk management** (delta, gamma, vega limits)

### Enabling Portfolio Management in Backtests

```rust
use dollarbill::backtesting::{BacktestEngine, BacktestConfig};

// Create config with portfolio management enabled
let mut config = BacktestConfig::default();
config.use_portfolio_management = true;  // Enable intelligent sizing

// Create engine (will auto-create portfolio manager)
let mut engine = BacktestEngine::new(config);

// Run backtest - position sizing is now volatility-adjusted
let result = engine.run_simple_strategy(&symbol, historical_data, 0.25);
```

### Custom Portfolio Configuration

```rust
use dollarbill::portfolio::{PortfolioConfig, SizingMethod, RiskLimits};

// Create custom portfolio config
let portfolio_config = PortfolioConfig {
    initial_capital: 100_000.0,
    max_risk_per_trade: 2.0,      // 2% max risk per trade
    max_position_pct: 10.0,       // 10% max single position
    sizing_method: SizingMethod::VolatilityBased,
    risk_limits: RiskLimits {
        max_portfolio_delta: 0.30,     // 30% portfolio delta max
        max_concentration_pct: 20.0,   // 20% max per position
        ..Default::default()
    },
    ..Default::default()
};

// Create backtest engine with custom portfolio settings
let mut engine = BacktestEngine::new_with_portfolio(
    BacktestConfig::default(),
    portfolio_config
);
```

### Impact on Backtests

**Before (Simple Sizing):**
```
Position Size = (Capital × 10%) / (Option Price × 100)
```
- Fixed 10% of capital per trade
- No volatility adjustment
- Could oversize in high-IV environments

**After (Portfolio-Aware Sizing):**
```
Position Size = Portfolio Manager Calculation
  - Base size from capital × risk percentage
  - Adjusted for volatility (high IV = smaller size)
  - Checked against portfolio delta limits
  - Validated against concentration limits
```
- Volatility-adjusted sizing
- Portfolio-level risk awareness
- Prevents dangerous positions automatically

## Trading Bot Integration

### Configuration

Add portfolio settings to `config/personality_bot_config.json`:

```json
{
  "trading": {
    "position_size_shares": 10,  // Fallback if portfolio mgmt disabled
    "max_positions": 5,
    "risk_management": {
      "stop_loss_pct": 0.15,
      "take_profit_pct": 0.30,
      "max_daily_trades": 10
    },
    "min_confidence": 0.30
  },
  "portfolio": {
    "enabled": true,                   // Enable portfolio management
    "initial_capital": 100000.0,       // Starting capital
    "max_risk_per_trade": 2.0,         // 2% risk per trade
    "max_position_pct": 10.0,          // 10% max position size
    "sizing_method": "VolatilityBased",  // Options: VolatilityBased, KellyCriterion, RiskParity
    "max_portfolio_delta": 0.30,       // 30% max portfolio delta
    "max_concentration_pct": 20.0      // 20% max single position
  }
}
```

### Behavior

**Portfolio Management Disabled (`enabled: false`):**
- Uses fixed `position_size_shares` (e.g., always 10 shares)
- No pre-trade risk checks
- Position-level stop loss/take profit only

**Portfolio Management Enabled (`enabled: true`):**
- **Intelligent Sizing**: Position size calculated based on volatility
  - AAPL (30% IV): 12 shares
  - NVDA (80% IV): 4 shares
- **Pre-Trade Approval**: Checks portfolio risk before placing order
  - Blocks trades if delta > 30%
  - Blocks trades if concentration > 20%
  - Warns if approaching limits
- **Dynamic Risk Management**: Adjusts to market conditions

### Example Output

```
🎭 Personality-Based Trading Bot - 2026-02-21 14:30:00

💰 Account: $95,234.50 cash | $104,765.50 portfolio value

📊 Analyzing with Personality-Driven Strategies...

   AAPL | 🔍 SIGNAL: Volatility Mean Reversion - Confidence: 75.2% (min: 30.0%)
   AAPL $185.50 | Strategy: Volatility Mean Reversion | Conf: 75.2% | 🟢 BUY → 8 shares... ✅
   
   NVDA | 🔍 SIGNAL: Momentum - Confidence: 82.1% (min: 30.0%)
   ❌ REJECTED by portfolio manager:
      ⚠️  Portfolio delta (32.5%) exceeds limit (30.0%)
      ⚠️  Strategy Momentum at capacity
   
   TSLA | 🔍 SIGNAL: Iron Condor - Confidence: 68.4% (min: 30.0%)
   TSLA $195.20 | Strategy: Iron Condor | Conf: 68.4% | 🟢 BUY → 5 shares... ✅
```

## Benefits of Integration

### For Backtesting

1. **More Realistic Simulations**
   - Volatility-adjusted sizing matches real-world behavior
   - Portfolio-level risk limits prevent unrealistic leverage

2. **Better Strategy Evaluation**
   - Backtest results closer to live trading performance
   - Risk-adjusted metrics more meaningful

3. **Risk Discovery**
   - Identifies when strategies would violate portfolio limits
   - Prevents implementing unsafe strategies

### For Live Trading

1. **Automatic Risk Management**
   - Prevents over-leveraging in high-volatility markets
   - Blocks trades that would exceed portfolio risk limits

2. **Intelligent Position Sizing**
   - Smaller positions in high-IV stocks (NVDA, TSLA)
   - Larger positions in low-IV stocks (GLD, TLT)

3. **Pre-Trade Approval**
   - Catches dangerous trades before execution
   - Warns when approaching limits
   - Provides clear rejection reasons

4. **Multi-Strategy Coordination**
   - Tracks allocation across all strategies
   - Prevents single strategy from dominating portfolio
   - Optimizes capital deployment

## Sizing Method Comparison

| Method | Best For | Characteristics |
|--------|----------|----------------|
| **VolatilityBased** | General trading | Adjusts for IV, conservative in high-vol |
| **KellyCriterion** | High-confidence setups | Maximizes geometric growth, requires win stats |
| **RiskParity** | Multi-strategy | Equal risk contribution across strategies |
| **FixedFractional** | Simple strategies | Fixed % of capital, predictable sizing |
| **FixedDollar** | Specific risk targets | Fixed dollar amount per trade |

## Migration Guide

### Existing Backtests

**Simple Migration:**
```rust
// Old code:
let mut engine = BacktestEngine::new(BacktestConfig::default());

// New code (minimal change):
let mut config = BacktestConfig::default();
config.use_portfolio_management = true;
let mut engine = BacktestEngine::new(config);
```

**Advanced Migration:**
```rust
// Configure portfolio manager explicitly
let portfolio_config = PortfolioConfig {
    sizing_method: SizingMethod::VolatilityBased,
    max_risk_per_trade: 1.5,  // More conservative
    // ... other settings
    ..Default::default()
};

let engine = BacktestEngine::new_with_portfolio(
    BacktestConfig::default(),
    portfolio_config
);
```

### Existing Trading Bots

1. Add portfolio section to config file (see example above)
2. Set `enabled: false` initially to maintain current behavior
3. Test with `enabled: true` in paper trading
4. Monitor rejection messages to tune limits
5. Enable for live trading once validated

## Best Practices

### Backtesting

1. **Enable portfolio management for production-like backtests**
   - More accurate position sizing
   - Realistic risk constraints

2. **Use default settings initially**
   - Adjust limits based on strategy characteristics
   - Start conservative, relax if needed

3. **Compare results with/without**
   - Run backtests both ways
   - Understand impact on performance metrics

### Live Trading

1. **Start with strict limits**
   - `max_risk_per_trade: 1.0` (1%)
   - `max_portfolio_delta: 0.20` (20%)
   - Relax gradually as confidence grows

2. **Monitor rejection reasons**
   - Log all trade rejections
   - Adjust limits if too many false rejections
   - Tighten if taking too much risk

3. **Use VolatilityBased sizing initially**
   - Most robust for diverse market conditions
   - Switch to KellyCriterion after tracking win stats

4. **Set realistic initial_capital**
   - Match your actual account size
   - Position sizing scales with capital

## Troubleshooting

### "All trades rejected"
- **Cause**: Limits too strict for your strategy
- **Fix**: Gradually increase `max_risk_per_trade` or `max_portfolio_delta`

### "Position sizes too small"
- **Cause**: High volatility or conservative settings
- **Fix**: Increase `max_position_pct` or adjust sizing method

### "Still using fixed sizing"
- **Cause**: Portfolio management not enabled
- **Check**: `portfolio.enabled: true` in config
- **Check**: `use_portfolio_management: true` in BacktestConfig

### "Position sizes too large"
- **Cause**: Volatility too low or limits too loose
- **Fix**: Decrease `max_position_pct` or use stricter sizing method

## Example: Complete Integration

```rust
use dollarbill::portfolio::{PortfolioManager, PortfolioConfig, SizingMethod};
use dollarbill::backtesting::{BacktestEngine, BacktestConfig};

// Create portfolio manager
let portfolio_config = PortfolioConfig {
    initial_capital: 100_000.0,
    max_risk_per_trade: 2.0,
    max_position_pct: 10.0,
    sizing_method: SizingMethod::VolatilityBased,
    ..Default::default()
};

// Create backtest engine with portfolio management
let mut engine = BacktestEngine::new_with_portfolio(
    BacktestConfig::default(),
    portfolio_config
);

// Run backtest
let result = engine.run_simple_strategy("AAPL", historical_data, 0.25);

println!("Total Return: {:.2}%", result.metrics.total_return * 100.0);
println!("Max Drawdown: {:.2}%", result.metrics.max_drawdown * 100.0);
println!("Sharpe Ratio: {:.2}", result.metrics.sharpe_ratio);
```

## Next Steps

1. Enable portfolio management in one strategy first
2. Monitor results in paper trading
3. Compare performance vs fixed sizing
4. Tune limits based on actual metrics
5. Roll out to all strategies once validated

For detailed API reference, see [Portfolio Management Guide](portfolio-guide.md).
