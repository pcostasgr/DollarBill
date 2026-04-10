# Short Options Guide

Guide to selling options with DollarBill: signals, margin, backtesting, and paper trading.

---

## Table of Contents
1. [Concepts](#concepts)
2. [Signal Types](#signal-types)
3. [Margin Requirements](#margin-requirements)
4. [IV Rank Filter](#iv-rank-filter)
5. [Backtesting Short Options](#backtesting-short-options)
6. [Profit Targets and Stop-Losses](#profit-targets-and-stop-losses)
7. [Expiry and Assignment Simulation](#expiry-and-assignment-simulation)
8. [Alpaca Paper Trading](#alpaca-paper-trading)
9. [Risk Management Checklist](#risk-management-checklist)

---

## Concepts

When you **sell** an option you collect premium upfront.  Your maximum profit is
that premium; your maximum loss depends on the structure:

| Strategy        | Max Profit        | Max Loss                      |
|-----------------|-------------------|-------------------------------|
| Naked call      | premium received  | **unlimited** (underlying ↑)  |
| Naked put       | premium received  | `(strike − premium) × 100`   |
| Credit call spread | premium received | `(spread_width − premium) × 100` |
| Credit put spread  | premium received | `(spread_width − premium) × 100` |
| Iron condor     | premium received  | `(wider_wing − premium) × 100` |
| Covered call    | premium + upside to strike | stock loss below entry |
| Cash-secured put | premium received | `(strike − premium) × 100`  |

The key edge in selling options comes from **IV overstatement**: implied
volatility tends to be higher than subsequent realised volatility on most
underlyings, so option sellers collect more premium than the eventual move
justifies.

---

## Signal Types

```rust
use dollarbill::strategies::SignalAction;

// Single-leg shorts
SignalAction::SellCall { strike: 155.0, days_to_expiry: 30, volatility: 0.28 }
SignalAction::SellPut  { strike: 145.0, days_to_expiry: 30, volatility: 0.28 }

// Two-leg spreads
SignalAction::CreditCallSpread { sell_strike: 155.0, buy_strike: 160.0, days_to_expiry: 30 }
SignalAction::CreditPutSpread  { sell_strike: 145.0, buy_strike: 140.0, days_to_expiry: 30 }

// Four-leg iron condor
SignalAction::IronCondor {
    sell_call_strike: 155.0, buy_call_strike: 160.0,
    sell_put_strike:  145.0, buy_put_strike:  140.0,
    days_to_expiry: 30,
}

// Other
SignalAction::CoveredCall  { sell_strike: 155.0, days_to_expiry: 30 }
SignalAction::SellStraddle                    // ATM call + ATM put (use add a concrete strike)
SignalAction::CashSecuredPut { strike_pct: 0.97 }  // sell put at 97% of spot
```

---

## Margin Requirements

Reg T (CBOE) margin is automatically enforced by the backtest engine before any
short position is opened.

```rust
use dollarbill::backtesting::{
    naked_call_margin, naked_put_margin,
    credit_spread_margin, iron_condor_margin, cash_secured_put_margin,
    has_sufficient_margin,
    max_loss_credit_spread, max_loss_iron_condor,
    max_profit_short, max_loss_naked_put,
};

// Margin requirement per contract
let m = naked_call_margin(spot, strike, premium);
println!("Required margin: ${:.0}/contract", m.per_contract);

// Check before opening
if has_sufficient_margin(&m, contracts, available_capital) {
    // safe to open
}

// Max-loss / max-profit helpers
let max_loss   = max_loss_credit_spread(sell_strike, buy_strike, net_premium);
let max_profit = max_profit_short(premium_per_share, contracts);
let put_loss   = max_loss_naked_put(strike, premium_per_share);
println!("Max loss on spread: ${:.0}", max_loss);   // e.g. $300
println!("Max profit:         ${:.0}", max_profit); // e.g. $200
```

### Reg T formulas implemented

**Naked call** (per-contract):
```
max(20% × spot × 100 − OTM_amount × 100 + premium × 100,
    10% × spot × 100 + premium × 100)
```

**Naked put** (per-contract):
```
max(20% × spot × 100 − OTM_amount × 100 + premium × 100,
    10% × strike × 100 + premium × 100)
```

**Credit spread**: `|buy_strike − sell_strike| × 100`  
**Iron condor**: `max(call_width, put_width) × 100`  
**Cash-secured put**: `strike × 100`

---

## IV Rank Filter

Before generating spread signals, filter on historical volatility rank to
avoid selling options when premiums are thin:

```rust
use dollarbill::strategies::spreads::{SpreadConfig, generate_spread_signals, iv_rank, rolling_hv21};
use dollarbill::market_data::csv_loader::load_csv_closes;

// Load historical closes
let closes: Vec<f64> = load_csv_closes("data/aapl_five_year.csv")?
    .iter().map(|d| d.close).collect();

// Compute rolling 21-day HV and current IV rank
let hv_series   = rolling_hv21(&closes);
let current_hv  = *hv_series.last().unwrap_or(&0.25);
let rank        = iv_rank(&hv_series, current_hv);   // 0.0 = lowest, 1.0 = highest
println!("HV rank: {:.0}%", rank * 100.0);

// SpreadConfig with IV rank gate
let config = SpreadConfig {
    symbol: "AAPL".into(),
    spot: 192.0,
    volatility: 0.28,
    risk_free_rate: 0.05,
    days_to_expiry: 30,
    wing_width: 5.0,
    min_premium: 0.50,
    max_delta: 0.30,
    iv_rank_threshold: 0.50,  // Only sell when HV in top 50% of its history
};

// Signals will be empty if current iv_rank < iv_rank_threshold
let signals = generate_spread_signals(&config);
```

Rule of thumb: set `iv_rank_threshold` to **0.50** (top half) for iron condors
and **0.40** for individual short options.

---

## Backtesting Short Options

```rust
use dollarbill::backtesting::{BacktestConfig, BacktestEngine};
use dollarbill::strategies::SignalAction;
use dollarbill::market_data::csv_loader::load_csv_closes;

let config = BacktestConfig {
    initial_capital: 100_000.0,
    days_to_expiry: 30,
    max_days_hold:  21,          // Close at 70% of expiry
    // Short-specific exit rules (see next section)
    short_take_profit_pct: Some(50.0),
    short_stop_loss_pct:   Some(200.0),
    // Long positions still use these
    stop_loss_pct:   Some(50.0),
    take_profit_pct: Some(100.0),
    ..BacktestConfig::default()
};

let mut engine = BacktestEngine::new(config);
let data = load_csv_closes("data/spy_five_year.csv")?;

let result = engine.run_with_signals("SPY", data, |symbol, spot, day_idx, hist_vols| {
    let vol = hist_vols.get(day_idx).copied().unwrap_or(0.25);
    // Only sell when vol is elevated
    if vol > 0.25 {
        vec![SignalAction::IronCondor {
            sell_call_strike: spot * 1.05,
            buy_call_strike:  spot * 1.10,
            sell_put_strike:  spot * 0.95,
            buy_put_strike:   spot * 0.90,
            days_to_expiry: 30,
        }]
    } else {
        vec![SignalAction::NoAction]
    }
});

println!("Total return: {:.1}%", result.metrics.total_return_pct);
println!("Win rate:     {:.1}%", result.metrics.win_rate * 100.0);
println!("Sharpe:       {:.2}",  result.metrics.sharpe_ratio);
```

---

## Profit Targets and Stop-Losses

Short options use **different** conventions from long options.  Percentages are
measured against the **premium received**, not the current option price.

| Config field            | Default   | Meaning                                              |
|-------------------------|-----------|------------------------------------------------------|
| `short_take_profit_pct` | `50.0`    | Close when option drops to 50% of entry premium      |
| `short_stop_loss_pct`   | `200.0`   | Close when option rises to 300% of entry premium     |
| `stop_loss_pct`         | `50.0`    | Long positions: close when down 50% from entry       |
| `take_profit_pct`       | `100.0`   | Long positions: close when up 100% from entry        |

**Example** — sold a call at $5.00:
- 50% profit target fires when option is at **$2.50** (`5 × 0.50`)
- 200% stop fires when option reaches **$15.00** (`5 × 3.0`)

These match the standard TastyTrade / CBOE management rules that statistically
outperform holding to expiry on most underlyings.

Set both to `None` to use the generic long-style checks (not recommended for
credit positions):

```rust
BacktestConfig {
    short_take_profit_pct: None,  // disable short-specific logic
    short_stop_loss_pct:   None,
    // falls through to generic stop_loss_pct / take_profit_pct
    ..BacktestConfig::default()
}
```

---

## Expiry and Assignment Simulation

At the end of a backtest, the engine simulates expiry for every open position:

- **ITM long call / ITM long put** — exercised at intrinsic value; brokerage
  commission is charged on the settlement leg.
- **ITM short call (assignment)** — you are assigned; the loss `(spot − strike) × qty × 100`
  is deducted from capital, commission charged.
- **ITM short put (assignment)** — you are assigned at the strike; loss
  `(strike − spot) × qty × 100` deducted, commission charged.
- **OTM options (both long and short)** — expire worthless, no commission
  charged (standard brokerage practice).

Exit Trades are recorded for all ITM settlements so they appear in trade history
and are included in P&L attribution.

---

## Alpaca Paper Trading

```rust
use dollarbill::alpaca::AlpacaClient;
use dollarbill::strategies::SignalAction;

let client = AlpacaClient::from_env()?;

let signal = SignalAction::IronCondor { ... };

// Convert strategy signal → Alpaca multi-leg order
let order = AlpacaClient::signal_to_options_order(
    &signal,
    "SPY",
    1,           // 1 contract per leg
    Some(-2.50), // net credit (negative = you receive)
)?;

// Submit to paper account
let response = client.submit_options_order(order).await?;
println!("Order ID: {}", response.id);
```

Supported signals: `BuyCall`, `BuyPut`, `SellCall`, `SellPut`,
`CreditCallSpread`, `CreditPutSpread`, `IronCondor`, `CoveredCall`,
`CashSecuredPut`, `SellStraddle`, `BuyStraddle`, `IronButterfly`.

### OCC symbol format

```
AAPL  260117C00150000
^---^ ^^^^^^ ^ ^^^^^^^^
ticker (6-char padded) YYMMDD C/P strike×1000 (8 digits)
```

Built automatically by `AlpacaClient::occ_symbol(ticker, yy, mm, dd, is_call, strike)`.

---

## Risk Management Checklist

Before going live with short options, verify:

- [ ] IV rank filter enabled (`iv_rank_threshold ≥ 0.40`)
- [ ] Margin check passes (`has_sufficient_margin`)
- [ ] Position sizing ≤ 2% max risk per trade
- [ ] Short take-profit set (50% recommended)
- [ ] Short stop-loss set (200% recommended)
- [ ] Max positions limit enforced (`max_positions` in `BacktestConfig`)
- [ ] Portfolio-level delta within tolerance (use `RiskAnalytics::portfolio_greeks`)
- [ ] Never sell naked calls on high-beta names without a hedge

The backtest does **not** model early assignment risk on dividends or deep-ITM
scenarios.  Always add a conservative buffer when sizing real positions.

---

## Live Position Management

The live bot (`dollarbill trade --live`) enforces a four-tier automated exit
ladder for every open short-put position.

### Decision Ladder

Evaluated on every price tick for each symbol with an open position:

```
spot > strike × (1 + roll_trigger_pct)   → Hold (no action)
strike × (1 + itm_proximity_pct) < spot
  ≤ strike × (1 + roll_trigger_pct)      → Roll down/out
spot ≤ strike × (1 + itm_proximity_pct) → ITM-proximity close
0–1 DTE                                  → Expiry close
premium ≤ entry × profit_target_pct      → Profit-target close
premium ≥ entry × stop_loss_pct          → Stop-loss close
```

### Roll Down / Out

A roll is a two-step atomic sequence:
1. **Buy to close** the current OCC contract at market
2. **Sell to open** a new put at the *same strike*, expiring `roll_dte_days`
   calendar days from today (default 30)

The replacement contract is resolved via `resolve_single_leg_occ` (queries the
live Alpaca options chain for the nearest listed strike/expiry) before
submission.

`roll_count` is stored in SQLite and incremented on every successful roll.
When `roll_count ≥ max_rolls` (default 2), the roll zone is skipped entirely
and the position falls through to the ITM-proximity close on the next trigger.

### Configuration Reference

| Field | Default | Description |
|---|---|---|
| `profit_target_pct` | `0.50` | Close when option value = 50% of entry premium |
| `stop_loss_pct` | `2.00` | Close when option value = 2× entry premium |
| `max_position_days` | `21` | Force-close after N calendar days |
| `itm_proximity_pct` | `0.03` | Close if spot ≤ strike × 1.03 |
| `roll_trigger_pct` | `0.05` | Roll if spot ≤ strike × 1.05 |
| `roll_dte_days` | `30` | DTE target for the new leg |
| `max_rolls` | `2` | Maximum rolls per position before hard close |

Set either `itm_proximity_pct` or `roll_trigger_pct` to `0.0` to disable that tier.

### Re-entry Cooldown

After any position close (automatic or manual), the bot blocks new entry
signals for that symbol for 5 minutes to prevent immediately reversing the
close on the next tick.

### ITM Guard (Entry)

Before submitting a new cash-secured put, the resolved OCC strike is compared
to the current spot price.  If the resolved put strike is ≥ 99.5% of spot
(ATM or ITM), the order is skipped — preventing the `resolve_occ` nearest-strike
logic from accidentally placing an ATM put when a 5%-OTM target was intended.

