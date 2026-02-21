# Portfolio Management Module

Advanced portfolio management system providing position sizing, risk analytics, multi-strategy allocation, and performance attribution.

## Overview

The portfolio management module helps you:
- **Size positions** intelligently based on risk and volatility
- **Monitor portfolio risk** with Greeks aggregation and VaR
- **Allocate capital** across multiple strategies optimally
- **Track performance** by strategy with risk-adjusted metrics
- **Enforce risk limits** to prevent excessive exposure

## Quick Start

```rust
use dollarbill::portfolio::{Port folioManager, PortfolioConfig, SizingMethod, AllocationMethod};

// Create portfolio manager
let config = PortfolioConfig {
    initial_capital: 100_000.0,
    max_risk_per_trade: 2.0,    // 2% per trade
    max_position_pct: 10.0,      // 10% max position
    sizing_method: SizingMethod::VolatilityBased,
    allocation_method: AllocationMethod::RiskParity,
    ..Default::default()
};

let mut manager = PortfolioManager::new(config);

// Add strategies
manager.add_strategy("IronCondor".to_string(), 40_000.0, 15.0, 35.0);
manager.add_strategy("CreditSpreads".to_string(), 40_000.0, 15.0, 35.0);

// Calculate position size
let size = manager.calculate_position_size(2.50, 0.30, Some(0.60), Some(300.0), Some(200.0));
println!("Suggested position size: {} contracts", size);
```

## Core Components

### 1. Position Sizing (`position_sizing.rs`)

Calculates optimal position sizes using various methods:

#### Sizing Methods

- **FixedFractional**: Allocate fixed % of capital
- **KellyCriterion**: Optimal growth rate based on win/loss stats
- **VolatilityBased**: Adjust for asset volatility
- **RiskParity**: Equal risk contribution
- **FixedDollar**: Fixed dollar amount

#### Example: Volatility-Based Sizing

```rust
use dollarbill::portfolio::{PositionSizer, SizingMethod};

let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);

// Low volatility stock - larger position
let size1 = sizer.calculate_size(
    SizingMethod::VolatilityBased,
    2.50,  // option price
    0.20,  // 20% volatility
    None, None, None
);

// High volatility stock - smaller position
let size2 = sizer.calculate_size(
    SizingMethod::VolatilityBased,
    2.50,
    0.60,  // 60% volatility
    None, None, None
);

assert!(size1 > size2);  // Low vol gets more allocation
```

#### Multi-Leg Sizing

```rust
use dollarbill::portfolio::MultiLegSizer;

let sizer = MultiLegSizer::new(100_000.0, 2.0, 10.0);

// Iron condor: size by max loss
let ic_size = sizer.iron_condor_size(
    SizingMethod::FixedFractional(2.0),
    500.0,   // max loss per spread
    1.50,    // net credit
    0.30     // IV
);

// Credit spread: size by spread width
let cs_size = sizer.credit_spread_size(
    SizingMethod::FixedFractional(2.0),
    5.0,     // spread width
    1.25,    // net credit
    0.28     // IV
);
```

### 2. Risk Analytics (`risk_analytics.rs`)

Portfolio-level risk monitoring and limits:

#### Risk Metrics

- **Portfolio Greeks**: Aggregated delta, gamma, theta, vega
- **Exposure**: Net and gross exposure in dollars
- **VaR**: Value at Risk (95% and 99% confidence)
- **Concentration**: Maximum single position %
- **Diversification**: Portfolio diversity score

#### Example: Risk Monitoring

```rust
use dollarbill::portfolio::{RiskAnalyzer, RiskLimits};

let limits = RiskLimits {
    max_portfolio_delta: 0.30,       // 30% max delta
    max_portfolio_gamma: 0.10,       // 10% max gamma
    max_concentration_pct: 20.0,     // 20% max per position
    max_var_pct: 10.0,               // 10% VaR limit
    ..Default::default()
};

let analyzer = RiskAnalyzer::new(100_000.0, limits);

// Analyze current positions
let risk = analyzer.calculate_portfolio_greeks(&positions);

println!("Portfolio Delta: {:.2}", risk.total_delta);
println!("Portfolio VaR (95%): ${:.2}", risk.var_95);
println!("Concentration: {:.1}%", risk.concentration_risk);

// Check violations
let violations = analyzer.check_risk_limits(&risk);
if !violations.is_empty() {
    for warning in violations {
        println!("⚠️  {}", warning);
    }
}
```

### 3. Strategy Allocation (`allocation.rs`)

Optimal capital allocation across strategies:

#### Allocation Methods

- **EqualWeight**: Equal allocation to all strategies
- **RiskParity**: Equal risk contribution (inverse volatility)
- **PerformanceWeighted**: Based on Sharpe ratios
- **VolatilityWeighted**: Inverse volatility weighting
- **Custom**: Manually set allocations

#### Example: Risk Parity Allocation

```rust
use dollarbill::portfolio::{PortfolioAllocator, AllocationMethod, StrategyStats};
use std::collections::HashMap;

let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::RiskParity);

// Add strategies with capacity and constraints
allocator.add_strategy("LowVolStrategy".to_string(), 50_000.0, 10.0, 50.0);
allocator.add_strategy("HighVolStrategy".to_string(), 50_000.0, 10.0, 50.0);

// Provide strategy statistics
let mut stats = HashMap::new();
stats.insert("LowVolStrategy".to_string(), StrategyStats {
    volatility: 0.15,   // Low vol
    sharpe_ratio: 1.5,
    ..Default::default()
});
stats.insert("HighVolStrategy".to_string(), StrategyStats {
    volatility: 0.45,   // High vol
    sharpe_ratio: 1.2,
    ..Default::default()
});

// Calculate optimal allocations
allocator.calculate_allocations(&stats);

// Low vol strategy gets more allocation (risk parity)
let low_vol = allocator.get_allocation("LowVolStrategy").unwrap();
let high_vol = allocator.get_allocation("HighVolStrategy").unwrap();

println!("Low Vol:  {:.1}%", low_vol.target_pct);
println!("High Vol: {:.1}%", high_vol.target_pct);
```

#### Rebalancing

```rust
// Get rebalancing recommendations
let trades = allocator.get_rebalancing_trades();

for trade in trades {
    println!("{} {} by ${:.2}",
        trade.action,
        trade.strategy,
        trade.dollar_change.abs()
    );
}
```

### 4. Performance Attribution (`performance.rs`)

Track and analyze strategy performance:

#### Performance Metrics

- Win rate, profit factor
- Average win/loss
- Sharpe and Sortino ratios
- Maximum drawdown
- ROI and returns

#### Example: Performance Tracking

```rust
use dollarbill::portfolio::PerformanceAttribution;

let mut attribution = PerformanceAttribution::new();

// Calculate performance for a strategy
let perf = attribution.calculate_strategy_performance("IronCondor", &positions);

println!("Win Rate: {:.1}%", perf.win_rate);
println!("Profit Factor: {:.2}", perf.profit_factor);
println!("Sharpe Ratio: {:.2}", perf.sharpe_ratio);
println!("Max Drawdown: {:.1}%", perf.max_drawdown_pct);

// Compare strategies
let comparisons = attribution.compare_strategies(&["IronCondor", "CreditSpreads", "Straddles"]);

for comp in comparisons {
    println!("{}: Sharpe {:.2}, Win Rate {:.1}%",
        comp.strategy,
        comp.sharpe_ratio,
        comp.win_rate
    );
}

// Find best strategy
if let Some(best) = attribution.best_strategy() {
    println!("Best performer: {}", best);
}
```

### 5. Portfolio Manager (`manager.rs`)

Orchestrates all portfolio management components:

#### Complete Workflow

```rust
use dollarbill::portfolio::{PortfolioManager, PortfolioConfig};

let mut manager = PortfolioManager::new(PortfolioConfig::default());

// 1. Add strategies
manager.add_strategy("IronCondor".to_string(), 40_000.0, 15.0, 35.0);

// 2. Update positions
manager.update_positions(current_positions);

// 3. Check if new trade allowed
let decision = manager.can_take_position("IronCondor", 2.0, 0.30, 20);

if decision.can_trade {
    println!("✅ Trade approved: {} contracts", decision.suggested_size);
} else {
    for warning in decision.risk_warnings {
        println!("⚠️  {}", warning);
    }
}

// 4. Get portfolio risk
let risk = manager.get_portfolio_risk();
println!("Portfolio Delta: {:.2}", risk.total_delta);

// 5. Print summary
manager.print_summary();
```

## Configuration

### Portfolio Config

```rust
use dollarbill::portfolio::{PortfolioConfig, SizingMethod, AllocationMethod, RiskLimits};

let config = PortfolioConfig {
    initial_capital: 100_000.0,
    max_risk_per_trade: 2.0,         // 2% per trade
    max_position_pct: 10.0,          // 10% max position
    
    sizing_method: SizingMethod::VolatilityBased,
    allocation_method: AllocationMethod::RiskParity,
    
    risk_limits: RiskLimits {
        max_portfolio_delta: 0.30,    // 30% max delta
        max_portfolio_gamma: 0.10,    // 10% max gamma
        max_portfolio_vega: 0.15,     // 15% max vega
        max_concentration_pct: 20.0,  // 20% per position
        max_var_pct: 10.0,            // 10% VaR limit
        max_sector_exposure_pct: 40.0, // 40% per sector
    },
};
```

### Risk Limits

| Limit | Description | Example |
|-------|-------------|---------|
| `max_portfolio_delta` | Max net delta as % of portfolio | 0.30 (30%) |
| `max_portfolio_gamma` | Max gamma exposure | 0.10 (10%) |
| `max_portfolio_vega` | Max vega exposure | 0.15 (15%) |
| `max_concentration_pct` | Max single position % | 20.0 (20%) |
| `max_var_pct` | Max VaR as % of portfolio | 10.0 (10%) |
| `max_sector_exposure_pct` | Max sector concentration | 40.0 (40%) |

## Best Practices

### 1. Position Sizing

✅ **DO:**
- Use volatility-based sizing for different asset classes
- Respect maximum position limits
- Account for multi-leg max loss (not just credit)

❌ **DON'T:**
- Use fixed sizes for all positions
- Ignore volatility differences
- Size by premium received alone

### 2. Risk Management

✅ **DO:**
- Monitor portfolio Greeks daily
- Set realistic VaR limits
- Diversify across uncorrelated strategies
- Rebalance when allocations drift >5%

❌ **DON'T:**
- Exceed concentration limits
- Ignore risk violations
- Put all capital in one strategy

### 3. Performance Tracking

✅ **DO:**
- Track risk-adjusted returns (Sharpe ratio)
- Analyze drawdowns and recovery time
- Compare strategy performance regularly
- Use performance to adjust allocations

❌ **DON'T:**
- Focus only on total P&L
- Ignore losing strategies too long
- Overweight recent performance

## Integration with Trading Bot

```rust
use dollarbill::portfolio::PortfolioManager;

// Initialize
let mut manager = PortfolioManager::new(config);

// Before each trade
let decision = manager.can_take_position(strategy_name, option_price, volatility, contracts);

if decision.can_trade {
    // Execute trade with suggested size
    execute_trade(decision.suggested_size);
    
    // Update positions
    manager.update_positions(get_current_positions());
} else {
    // Log risk warnings
    for warning in decision.risk_warnings {
        log_warning(&warning);
    }
}

// End of day
manager.print_summary();

// Weekly
let rebalances = manager.get_rebalancing_recommendations();
process_rebalancing(rebalances);
```

## Examples

Run the comprehensive example:

```bash
cargo run --example portfolio_management
```

This demonstrates:
- Multi-strategy allocation
- Position sizing for options
- Risk monitoring and limits
- Performance tracking
- Rebalancing recommendations

## Testing

All components have comprehensive unit tests (41 tests, 100% passing):

```bash
# Test all portfolio components
cargo test portfolio

# Test specific component
cargo test portfolio::position_sizing
cargo test portfolio::risk_analytics
cargo test portfolio::allocation
cargo test portfolio::performance
cargo test portfolio::manager
```

## API Reference

### Position Sizing

- `PositionSizer::new(account, max_risk, max_position)` - Create sizer
- `calculate_size(method, price, vol, ...)` - Calculate contracts
- `validate_position(contracts, price)` - Check if valid

### Risk Analytics

- `RiskAnalyzer::new(portfolio_value, limits)` - Create analyzer
- `calculate_portfolio_greeks(&positions)` - Get portfolio risk
- `check_risk_limits(&risk)` - Check violations
- `diversification_score(&positions)` - Diversity metric

### Allocation

- `PortfolioAllocator::new(capital, method)` - Create allocator
- `add_strategy(name, capacity, min, max)` - Add strategy
- `calculate_allocations(&stats)` - Optimize allocations
- `get_rebalancing_trades()` - Get rebalancing actions

### Performance

- `PerformanceAttribution::new()` - Create tracker
- `calculate_strategy_performance(name, positions)` - Calculate metrics
- `compare_strategies(&names)` - Side-by-side comparison
- `best_strategy()` - Find top performer

### Portfolio Manager

- `PortfolioManager::new(config)` - Initialize manager
- `add_strategy(...)` - Register strategy
- `can_take_position(...)` - Pre-trade check
- `calculate_position_size(...)` - Size calculation
- `get_portfolio_risk()` - Current risk metrics
- `print_summary()` - Display overview

## Advanced Topics

### Custom Sizing Algorithms

Extend `SizingMethod` enum for custom algorithms.

### Dynamic Risk Limits

Adjust limits based on market regime using `RiskLimits`.

### Strategy Capacity Models

Use `CapacityAnalyzer` to estimate strategy capacity based on liquidity.

## Troubleshooting

**Q: Position size is 0?**
A: Check that account value > 0 and price > 0. May be hitting max position limit.

**Q: Risk violations but trade approved?**
A: `can_trade` is false if violations exist. Check `risk_warnings` field.

**Q: Allocations don't sum to 100%?**
A: When constraints prevent reaching 100%, allocations respect limits. This is correct behavior.

**Q: Sharpe ratio is 0?**
A: Need at least one trade to calculate. Returns 0.0 for empty performance.

---

**Next:** See [examples/portfolio_management.rs](../examples/portfolio_management.rs) for complete working example.
