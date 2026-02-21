# Options Trading Strategies Guide

A comprehensive guide to the multi-leg options strategies available in DollarBill, including educational content, risk profiles, and implementation details.

## Table of Contents

1. [Strategy Overview](#strategy-overview)
2. [Credit Spreads](#credit-spreads)
3. [Iron Condor](#iron-condor)
4. [Straddles & Strangles](#straddles--strangles)
5. [Income Strategies](#income-strategies)
6. [Strategy Templates System](#strategy-templates-system)
7. [Risk Management](#risk-management)
8. [Best Practices](#best-practices)

---

## Strategy Overview

DollarBill supports both simple and complex multi-leg options strategies. All strategies can be backtested and customized using our template system.

### Strategy Classification

| Strategy Type | Market Outlook | Risk Profile | Complexity |
|--------------|----------------|--------------|------------|
| Covered Call | Neutral/Bullish | Limited | Simple |
| Cash-Secured Put | Neutral/Bullish | Limited | Simple |
| Bull Put Spread | Bullish | Defined | Medium |
| Bear Call Spread | Bearish | Defined | Medium |
| Iron Condor | Neutral | Defined | Advanced |
| Short Straddle | Neutral | Undefined | Advanced |
| Short Strangle | Neutral | Undefined | Advanced |

---

## Credit Spreads

Credit spreads are vertical spreads where you **sell** an option closer to the money and **buy** an option further from the money. You collect a net credit.

### Bull Put Spread

**Market Outlook:** Bullish to neutral  
**Risk:** Defined (spread width - credit received)  
**Reward:** Limited (credit received)

#### Structure
```
Sell Put (higher strike)  ← Collect premium
Buy Put (lower strike)    ← Limit downside
```

#### Example
- Stock at $100
- Sell $97 Put (collect $200)
- Buy $92 Put (pay $50)
- Net Credit: $150
- Max Profit: $150 (if stock stays above $97)
- Max Loss: $350 (if stock falls below $92)
  - Spread width: ($97 - $92) × 100 = $500
  - Max loss: $500 - $150 = $350

#### When to Use
- You're bullish but want defined risk
- Stock is in an uptrend
- Implied volatility is high (better premium)
- You want to generate income with downside protection

#### Implementation
```rust
use dollarbill::strategies::templates::BullPutSpreadConfig;

let config = BullPutSpreadConfig {
    days_to_expiry: 30,
    sell_put_pct: 0.97,  // Sell 3% below spot
    buy_put_pct: 0.92,   // Buy 8% below spot
};

let signals = config.generate_signals(spot, volatility);
```

#### Exit Strategy
- Close at 50-75% of max profit
- Use stop loss at 200% of credit received
- Exit before earnings or major events
- Consider rolling if untested with time value remaining

---

### Bear Call Spread

**Market Outlook:** Bearish to neutral  
**Risk:** Defined (spread width - credit received)  
**Reward:** Limited (credit received)

#### Structure
```
Sell Call (lower strike)   ← Collect premium
Buy Call (higher strike)   ← Limit upside
```

#### Example
- Stock at $100
- Sell $103 Call (collect $180)
- Buy $108 Call (pay $40)
- Net Credit: $140
- Max Profit: $140 (if stock stays below $103)
- Max Loss: $360 (if stock rises above $108)

#### When to Use
- You're bearish or expect consolidation
- Stock hit resistance level
- Implied volatility is elevated
- After a significant run-up

#### Implementation
```rust
use dollarbill::strategies::templates::BearCallSpreadConfig;

let config = BearCallSpreadConfig {
    days_to_expiry: 30,
    sell_call_pct: 1.03,  // Sell 3% above spot
    buy_call_pct: 1.08,   // Buy 8% above spot
};

let signals = config.generate_signals(spot, volatility);
```

---

## Iron Condor

The iron condor is one of the most popular income strategies. It combines a bull put spread and a bear call spread.

**Market Outlook:** Neutral (expect stock to stay in range)  
**Risk:** Defined (spread width - net credit)  
**Reward:** Limited (net credit from both spreads)

### Structure
```
Buy Put  ← Sell Put | Current Price | Sell Call → Buy Call
  $90      $95             $100          $105       $110
```

### The Four Legs

1. **Buy Put** (far OTM) - Protects against large downside move
2. **Sell Put** (near OTM) - Collects premium on put side
3. **Sell Call** (near OTM) - Collects premium on call side
4. **Buy Call** (far OTM) - Protects against large upside move

### Profit/Loss Profile

- **Max Profit:** Net credit received (if stock stays between short strikes)
- **Max Loss:** Width of spread - net credit (if stock moves past long strikes)
- **Breakeven Points:** 
  - Lower: Short put strike - net credit
  - Upper: Short call strike + net credit

### Example Trade

Stock at $100 with 30 DTE:
- Buy $90 Put @ $0.30 (pay $30)
- Sell $95 Put @ $1.20 (collect $120)
- Sell $105 Call @ $1.10 (collect $110)
- Buy $110 Call @ $0.25 (pay $25)

**Net Credit:** $120 + $110 - $30 - $25 = **$175**

**Max Profit:** $175 (79% return on risk if stock stays $95-$105)  
**Max Loss:** $500 - $175 = **$325** (spread width minus credit)  
**Risk/Reward Ratio:** 1.86:1 (not the best, but high probability)

### When to Use Iron Condors

✅ **Good Conditions:**
- Low volatility environment (VIX < 20)
- Stock trading in defined range
- After volatility spike (collect higher premium)
- 45-60 days to expiration
- Major earnings/events are past

❌ **Avoid When:**
- High volatility (VIX > 30)
- Upcoming earnings
- Strong trending market
- Major economic events pending

### Implementation

```rust
use dollarbill::strategies::templates::IronCondorConfig;

// Conservative iron condor (wider wings)
let conservative = IronCondorConfig {
    days_to_expiry: 45,
    sell_put_pct: 0.93,    // 7% below spot
    buy_put_pct: 0.88,     // 12% below spot
    sell_call_pct: 1.07,   // 7% above spot
    buy_call_pct: 1.12,    // 12% above spot
};

// Aggressive iron condor (tighter wings)
let aggressive = IronCondorConfig {
    days_to_expiry: 30,
    sell_put_pct: 0.97,    // 3% below spot
    buy_put_pct: 0.94,     // 6% below spot
    sell_call_pct: 1.03,   // 3% above spot
    buy_call_pct: 1.06,    // 6% above spot
};

let signals = conservative.generate_signals(spot, volatility);
```

### Management Rules

1. **Entry:** Place when 45-60 DTE
2. **Profit Target:** Close at 50-75% of max profit
3. **Loss Limit:** Exit if loss reaches 2x or 3x max profit
4. **Time-based:** Close 7-10 days before expiration
5. **Adjustment:** Consider rolling untested side if one side is challenged

### Best Underlyings for Iron Condors

- **SPY** - S&P 500 ETF (low volatility, liquid)
- **QQQ** - Nasdaq ETF (medium volatility, tech exposure)
- **IWM** - Russell 2000 ETF (small caps)
- **Large caps with low volatility:** AAPL, MSFT, GOOGL

Avoid: High volatility stocks (TSLA, meme stocks), stocks with upcoming earnings

---

## Straddles & Strangles

These are **undefined risk** strategies - use with caution and proper risk management.

### Short Straddle

**Market Outlook:** Neutral (expect stock to stay near current price)  
**Risk:** Undefined (can lose significantly if stock moves)  
**Reward:** Limited (premium from both options)

#### Structure
```
Sell ATM Call
Sell ATM Put
(Both at same strike, typically at-the-money)
```

#### Example
- Stock at $100
- Sell $100 Call @ $3.50
- Sell $100 Put @ $3.40
- Net Credit: $690

#### Risk
This strategy has **unlimited upside risk** and **substantial downside risk**. Only for experienced traders with proper capital.

#### Implementation
```rust
use dollarbill::strategies::templates::ShortStraddleConfig;

let config = ShortStraddleConfig {
    days_to_expiry: 30,
    strike_pct: 1.00,  // At-the-money
};

let signals = config.generate_signals(spot, volatility);
```

---

### Short Strangle

**Market Outlook:** Neutral (expect stock to stay in range)  
**Risk:** Undefined (but less than straddle due to OTM strikes)  
**Reward:** Limited (less premium than straddle)

#### Structure
```
Sell OTM Put (below current price)
Sell OTM Call (above current price)
```

#### Example
- Stock at $100
- Sell $95 Put @ $1.80
- Sell $105 Call @ $1.70
- Net Credit: $350

#### Advantages over Straddle
- Wider breakeven range
- Lower margin requirement
- More forgiving to small moves

#### Implementation
```rust
use dollarbill::strategies::templates::ShortStrangleConfig;

let config = ShortStrangleConfig {
    days_to_expiry: 30,
    put_strike_pct: 0.95,   // 5% below
    call_strike_pct: 1.05,  // 5% above
};

let signals = config.generate_signals(spot, volatility);
```

---

## Income Strategies

### Covered Call

**Market Outlook:** Neutral to slightly bullish  
**Risk:** Downside risk in stock (partially offset by premium)  
**Reward:** Premium + potential capital gains to strike

#### When to Use
- Own 100 shares of stock
- Willing to sell at strike price
- Generate income while holding
- Stock in consolidation phase

#### Structure
- Long 100 shares
- Sell 1 call (typically 5-10% OTM)

#### Example
- Own 100 shares of AAPL at $150
- Sell $157.50 Call (5% OTM) @ $2.50
- Collect $250 premium
- If stock stays below $157.50: Keep stock + premium
- If stock goes above $157.50: Sell stock at $157.50 (profit: $750 + $250 = $1,000)

#### Implementation
```rust
use dollarbill::strategies::templates::CoveredCallConfig;

let config = CoveredCallConfig {
    days_to_expiry: 30,
    call_strike_pct: 1.05,  // 5% above current price
};

let signals = config.generate_signals(spot, volatility);
```

---

### Cash-Secured Put

**Market Outlook:** Bullish (willing to own stock at strike price)  
**Risk:** Downside risk if stock falls significantly  
**Reward:** Premium collected

#### When to Use
- Want to buy stock at lower price
- Generate income while waiting
- Bullish long-term on stock

#### Structure
- Set aside cash to buy 100 shares at strike
- Sell 1 put (typically 3-5% OTM)

#### Example
- AAPL at $150, you want to buy at $145
- Sell $145 Put @ $2.00
- Collect $200 premium
- Keep cash secured: $14,500
- If stock stays above $145: Keep premium, repeat next month
- If stock drops below $145: Buy 100 shares at $145 (effective price: $143)

#### Implementation
```rust
use dollarbill::strategies::templates::CashSecuredPutConfig;

let config = CashSecuredPutConfig {
    days_to_expiry: 30,
    put_strike_pct: 0.95,  // 5% below current price
};

let signals = config.generate_signals(spot, volatility);
```

---

## Strategy Templates System

DollarBill provides a flexible template system that allows you to quickly configure and backtest strategies with custom parameters.

### Using Templates

```rust
use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig};
use dollarbill::strategies::templates::IronCondorConfig;
use dollarbill::market_data::csv_loader::load_csv_closes;

// Create custom configuration
let strategy_config = IronCondorConfig {
    days_to_expiry: 45,
    sell_put_pct: 0.95,
    buy_put_pct: 0.90,
    sell_call_pct: 1.05,
    buy_call_pct: 1.10,
};

// Load historical data
let data = load_csv_closes("data/spy_five_year.csv")?;

// Create backtest engine
let mut engine = BacktestEngine::new(BacktestConfig {
    initial_capital: 100_000.0,
    position_size_pct: 20.0,
    max_positions: 2,
    days_to_expiry: 45,
    risk_free_rate: 0.045,
    commission_per_trade: 2.0,
    max_days_hold: 40,
    stop_loss_pct: Some(2.0),
    take_profit_pct: Some(0.50),
});

// Run backtest with template
let result = engine.run_with_signals(
    "SPY",
    data,
    move |_symbol, spot, _day_idx, hist_vols| {
        let vol = hist_vols.last().copied().unwrap_or(0.25);
        strategy_config.generate_signals(spot, vol)
    },
);

result.print_summary();
```

### Available Templates

All templates implement a `generate_signals()` method and have sensible defaults:

- `IronCondorConfig` - Four-leg neutral strategy
- `BullPutSpreadConfig` - Bullish credit spread
- `BearCallSpreadConfig` - Bearish credit spread
- `ShortStraddleConfig` - ATM premium collection
- `ShortStrangleConfig` - OTM premium collection
- `CoveredCallConfig` - Stock + short call
- `CashSecuredPutConfig` - Short put with cash backing

### Customization

Every template allows full customization:

```rust
// Create your own configuration
let my_iron_condor = IronCondorConfig {
    days_to_expiry: 60,        // Your timeframe
    sell_put_pct: 0.92,        // Your risk tolerance
    buy_put_pct: 0.87,         // Your spread width
    sell_call_pct: 1.08,       // Your profit target
    buy_call_pct: 1.13,        // Your max loss point
};
```

---

## Risk Management

### Position Sizing

**Golden Rule:** Never risk more than 2-5% of your account on any single trade.

For iron condors with $325 max loss:
- $10,000 account: 1 contract max (3.25% risk)
- $50,000 account: 3-5 contracts (2-3% risk)
- $100,000 account: 6-10 contracts (2-3% risk)

### Stop Loss Strategies

1. **Percentage-based:** Exit if loss reaches 2x or 3x the credit received
2. **Time-based:** Exit 7-10 days before expiration
3. **Profit target:** Close at 50-75% of max profit
4. **Technical:** Exit if stock breaks key support/resistance

### Diversification

- Don't put all capital in one strategy
- Trade different underlyings (SPY, QQQ, IWM)
- Vary expiration dates (don't all expire same day)
- Mix neutral and directional strategies

### Managing Winners

Exit early to lock in profits:
- **50% of max profit:** Conservative approach, high win rate
- **65% of max profit:** Balanced approach
- **75% of max profit:** Aggressive, but reduces risk significantly

Holding to expiration maximizes profit but increases risk.

---

## Best Practices

### 1. Start with Defined Risk Strategies
- Begin with credit spreads and iron condors
- Avoid undefined risk until experienced
- Paper trade before using real money

### 2. Choose the Right Underlyings
- **For iron condors:** SPY, QQQ, IWM (liquid, lower volatility)
- **For credit spreads:** Any stock with good liquidity
- **Avoid:** Low volume options, penny stocks, recent IPOs

### 3. Timing is Everything
- Enter iron condors after volatility spikes
- Enter credit spreads in trending markets
- Avoid earnings unless that's your strategy
- Check economic calendar for major events

### 4. Use Backtesting
```bash
# Test your strategy before trading live
cargo run --example iron_condor
cargo run --example credit_spreads
cargo run --example strategy_templates
```

### 5. Keep a Trading Journal
Track:
- Entry/exit dates and prices
- P&L per trade
- Why you entered
- What you learned
- Adjustments made

### 6. Continuous Learning
- Review past trades monthly
- Analyze wins and losses
- Adjust parameters based on data
- Stay updated on market conditions

---

## Quick Reference

### Strategy Selection Matrix

| Market Condition | Recommended Strategy |
|-----------------|---------------------|
| Strong uptrend | Bull put spread |
| Strong downtrend | Bear call spread |
| Range-bound | Iron condor |
| Low volatility | Iron condor, short strangle |
| High volatility | Credit spreads (wider), avoid iron condors |
| Neutral | Any income strategy |

### Running Examples

```bash
# Iron Condor (4-leg neutral income)
cargo run --example iron_condor

# Credit Spreads (bull put & bear call)
cargo run --example credit_spreads

# Strategy Template System
cargo run --example strategy_templates

# Basic Short Options
cargo run --example backtest_short_options
```

### Key Metrics to Track

- **Win Rate:** Percentage of profitable trades
- **Profit Factor:** Gross profit ÷ gross loss
- **Average Days Held:** Holding period
- **Max Drawdown:** Largest peak-to-trough decline
- **Return on Risk:** Return per dollar of max risk

---

## Additional Resources

- **Backtesting Guide:** [docs/backtesting-guide.md](docs/backtesting-guide.md)
- **Trading Guide:** [docs/trading-guide.md](docs/trading-guide.md)
- **Getting Started:** [docs/getting-started.md](docs/getting-started.md)

## Disclaimer

This guide is for educational purposes only. Options trading involves substantial risk of loss. Past performance does not guarantee future results. Always paper trade strategies before using real capital.

---

**Last Updated:** February 2026  
**DollarBill Version:** 0.1.0
