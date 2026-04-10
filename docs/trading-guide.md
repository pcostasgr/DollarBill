# Paper Trading Strategies - Quick Start

## ✅ What's Ready

### Strategy Deployment System ⭐ NEW
Test and deploy multiple trading strategies with flexible configuration.

```bash
# Test all deployment patterns
cargo run --example strategy_deployment
```

**What it demonstrates:**
- **Manual Strategy Registration** - Direct strategy instantiation
- **Configuration-Driven Deployment** - JSON-based strategy loading
- **Strategy Performance Comparison** - Side-by-side evaluation across market conditions
- **Ensemble Strategies** - Weighted combination of multiple approaches

**Available Strategies:**
- **Vol Mean Reversion** - Statistical arbitrage on volatility mispricings
- **Momentum** - Trend-following based on volatility momentum
- **Ensemble** - Combines multiple strategies with configurable weights

**Output Example:**
```
🎭 Example 4: Ensemble Strategy
Ensemble strategy combines:
  - Vol Mean Reversion (60% weight)
  - Momentum (40% weight)

🌍 High Vol Spike:
  Ensemble: IronButterfly { wing_width: 50.0 }, Confidence: 83.3%, Edge: $6.00
```

### Live Paper Trading Examples

### 1. **paper_trading.rs** - Single Scan
Run once to check signals and execute trades.

```powershell
# Set your Alpaca API keys
$env:ALPACA_API_KEY   = "your-key"
$env:ALPACA_API_SECRET = "your-secret"

# Dry-run: prints orders but submits nothing
.\target\release\dollarbill.exe trade --dry-run
```

**What it does:**
- Scans all **enabled stocks from `config/stocks.json`**
- Uses same momentum + RSI strategy from backtesting
- Volatility-adaptive thresholds (aggressive for high-vol stocks, moderate for others)
- Buys 5 shares on BUY signals (if no position)
- Sells entire position on SELL signals
- Shows account balance and current P&L

**Output Example:**
```
=== ALPACA PAPER TRADING - LIVE STRATEGY TEST ===

📊 Account Status:
   Cash: $95,234.50
   Buying Power: $190,469.00
   Portfolio Value: $104,765.50

📈 Current Positions:
   TSLA: 5 shares @ $345.20 | P&L: $127.50

🔍 Analyzing Signals...

--- TSLA ---
   Volatility: 60.7%
   Current Price: $350.70
   Signal: Hold
   💼 Already have position - holding

--- NVDA ---
   Volatility: 52.4%
   Current Price: $875.30
   Signal: Buy
   🟢 BUY SIGNAL - Submitting market order...
   ✅ Order submitted! ID: abc-123-def

=== SUMMARY ===
Signals Generated: 1
```

### 2. **trading_bot.rs** - Continuous Trading
Runs continuously, scanning every N minutes.

```powershell
# Dry-run first — prints orders, submits nothing
.\target\release\dollarbill.exe trade --dry-run

# Live mode — Alpaca WebSocket stream + continuous trading
$env:ALPACA_API_KEY   = "your-key"
$env:ALPACA_API_SECRET = "your-secret"
.\target\release\dollarbill.exe trade --live
```

**What it does:**
- Everything from paper_trading.rs PLUS:
- Runs in a loop (checks market every N minutes)
- Max 3 concurrent positions (risk management)
- Shows timestamp and P&L% for each position
- Scans all **enabled stocks from `config/stocks.json`**
- Press Ctrl+C to stop

**Output Example:**
```
🤖 Trading Bot Iteration - 2026-01-05 14:32:15
============================================================

💰 Account: $95,234.50 cash | $104,765.50 portfolio value

📊 Positions (2):
   TSLA | 5 @ $345.20 | P&L: $127.50 (+7.4%)
   NVDA | 5 @ $870.00 | P&L: $26.50 (+0.6%)

🔍 Scanning for signals...

   NVDA $875.30 | Vol: 52.4% | 🟢 BUY → Holding (already owned)
   AAPL $425.80 | Vol: 42.1% | ⏸️  HOLD

💤 Sleeping for 5 minutes...
```

### 3. **Heston-Based Options Trading** ⭐

Advanced options strategies using stochastic volatility pricing.

**Backtesting First:**
```powershell
# Calibrate Heston parameters to live market
.\target\release\dollarbill.exe calibrate TSLA

# Backtest Heston strategies and save the performance matrix
.\target\release\dollarbill.exe backtest --save
```

**What makes Heston special:**
- **Realistic pricing**: Accounts for volatility smiles and skews
- **Better edge detection**: Finds true mispricings vs Black-Scholes
- **Professional-grade**: Used by hedge funds and market makers
- **NVDA Results**: +270% backtested returns vs +150% Black-Scholes

---

### 4. **Live IV Feed, Background Recalibration & Greeks Alerts** ⭐ NEW

Three Phase 3 enhancements baked into `.\target\release\dollarbill.exe trade`:

**Live ATM IV Feed (`LiveIvCache` — 15-min TTL)**
The bot maintains a TTL-cached ATM implied-vol feed sourced from live Yahoo options chains. Newton-Raphson IV solves are done on near-ATM strikes (|K/S − 1| ≤ 5%); the median IV is stored and returned for subsequent ticks without extra network calls. Falls back to the 30-min recalibration value, then to the boot-time Heston JSON if cache is empty.

**Background 30-Min Heston Recalibration**
At startup, the bot seeds a shared `Arc<RwLock<HashMap<String, CalibParams>>>` from `data/{symbol}_heston_params.json`, then spawns an async background task that re-runs full Nelder-Mead Heston calibration (fetch price + liquid options → optimise) for every configured symbol every 30 minutes. The tick loop always reads the freshest available parameters.

**Greeks Logging & Delta Hedge Alert**
After every filled order the bot logs aggregate portfolio Greeks and fires a `⚠️ DELTA HEDGE ALERT` when net delta exceeds the 30%-of-equity threshold:

```
📊 Portfolio Greeks — Δ: 4.82 | Γ: 0.0412 | Vega: $512.40 | Θ: -$91.30/day
⚠️  DELTA HEDGE ALERT: |Δ| = 4.82 exceeds 30% threshold — consider hedging
```

No extra CLI flags are needed — all three features activate automatically when you run:
```powershell
.\target\release\dollarbill.exe trade --live
```

---

### 5. **Live Dashboard (`dashboard.exe`)** ⭐ NEW

A separate `ratatui` terminal UI that monitors the bot in real time.

**Start in a second terminal:**
```powershell
.\target\release\dashboard.exe
```

**Panel layout:**

| Panel | Content |
|---|---|
| Header | Mode (LIVE/DRY-RUN), circuit-breaker state, daily loss vs limit, equity, order count |
| Open Positions | Symbol, qty, entry price, strategy, expiry |
| Last Signals | Per-symbol last strategy name + action + time |
| Greeks | Portfolio Δ / Γ / Vega / Θ (colour-coded: yellow when delta is large) |
| Recent Orders | Last 20 non-tick trades from SQLite with fill status |
| Footer | Keybindings + age of last bot write |

**How it works:**
- `dollarbill.exe trade --live` writes `data/bot_status.json` atomically (tmp→rename) after every price tick
- `dashboard.exe` reads that file + `data/trades.db` every second
- No network, no shared memory — just files

**Keybindings:** `q` / `Esc` quit · `r` force-refresh

---

### 7. **Automated Position Management** ⭐ NEW

The live bot manages open options positions autonomously through a four-tier
decision ladder evaluated on every price tick.

#### Trigger Ladder (short put example — strike $120, spot falling from $126)

| Zone | Spot threshold | Action |
|---|---|---|
| Normal | spot > strike × 1.05 | Hold — no action |
| **Roll zone** | strike × 1.03 < spot ≤ strike × 1.05 | Auto-roll (close + reopen 30 DTE out) |
| **Close zone** | spot ≤ strike × 1.03 | Auto-close (ITM proximity guard) |
| Expiry / P&L | 0–1 DTE, or 50% profit / 2× stop | Standard exit |

#### Roll Down / Out

When spot enters the *roll zone*, the bot:
1. Buys to close the current OCC contract (market order)
2. Resolves the nearest listed contract at the same strike ~30 DTE out
3. Sells to open the new contract (market order)
4. Updates SQLite: new `occ_symbol`, new `expires_at`, new `premium_collected`, `roll_count += 1`

Rolling caps out at `max_rolls` (default 2). On the third approach toward the
strike the bot falls through to the ITM-proximity close instead.

#### Configuration (`config/trading_bot_config.json`)

```json
"bot_runtime": {
  "profit_target_pct":  0.50,   // Close when 50% of premium has decayed
  "stop_loss_pct":      2.00,   // Close when option doubles in price
  "max_position_days":  21,     // Force-close after 21 calendar days
  "itm_proximity_pct":  0.03,   // Close if spot ≤ strike × 1.03 (3% buffer)
  "roll_trigger_pct":   0.05,   // Roll if spot ≤ strike × 1.05 (5% buffer)
  "roll_dte_days":      30,     // Target DTE for rolled leg
  "max_rolls":          2       // Max roll attempts per position
}
```

Set `"itm_proximity_pct": 0.0` or `"roll_trigger_pct": 0.0` to disable that
tier individually.

#### Re-entry Cooldown

After any close (automatic or manual reconciliation), the bot suppresses new
entry signals for that symbol for 5 minutes (`REENTRY_COOLDOWN_SECS = 300`).
This prevents immediately reversing the close on the next tick.

#### ITM Guard (Order Submission)

Before submitting a new cash-secured put, the bot calls
`resolve_single_leg_occ` and checks the resolved strike against the current
spot price.  If the resolved put strike is ≥ 99.5% of spot (ATM or ITM), the
order is skipped with a warning — avoiding the scenario where `resolve_occ`
snaps a 5%-OTM target to a nearby ATM contract.



---

### 6. **Email Alerts (`lettre` SMTP)** ⭐ NEW

The bot sends email notifications for critical events — no polling required.

**Enable in `config/trading_bot_config.json`:**
```json
"alerts": {
  "enabled": true,
  "smtp_host": "smtp.gmail.com",
  "smtp_port": 587,
  "smtp_user": "you@gmail.com",
  "smtp_password": "",
  "from": "DollarBill Bot <you@gmail.com>",
  "to": "you@gmail.com",
  "use_smtps": false,
  "on_circuit_breaker": true,
  "on_fill": false,
  "on_daily_loss": true,
  "on_disconnect": true,
  "daily_loss_alert_pct": 0.80
}
```

**Supply the password via env var (never hard-code it):**
```powershell
$env:DOLLARBILL_SMTP_PASSWORD = "your-gmail-app-password"
```

> **Gmail**: Google Account → Security → 2-Step Verification → App passwords.

| Event | Default | Triggered when |
|---|---|---|
| `on_circuit_breaker` | **on** | Daily spend hits 100% of `max_daily_loss_pct` |
| `on_daily_loss` | **on** | Daily spend hits 80% of limit (`daily_loss_alert_pct`) |
| `on_disconnect` | **on** | Alpaca WebSocket permanently disconnects |
| `on_fill` | off | Every filled order (disable to avoid noise) |

All alerts fire via `tokio::spawn` (non-blocking) except disconnect, which awaits before the bot exits. The alerter logs a `warn!` on SMTP failure and never panics.

## 🎯 Strategy Details

### From Your Backtesting Results:

**High-Vol Strategy** (TSLA 60.7%, NVDA 52.4%):
- RSI: Buy < 60, Sell > 40
- Momentum: ±3% threshold
- **Backtest**: TSLA +718%, NVDA +403%

**Medium-Vol Strategy** (META 42.1%, AMZN 37.8%, GOOGL 35.2%):
- RSI: Buy < 65, Sell > 35
- Momentum: ±2% threshold
- **Backtest**: META +96%, GOOGL +83%

**Low-Vol Filter** (AAPL 27.8%, MSFT 29.1%):
- Automatically **HOLD** (no trades)
- **Backtest**: Learned these lose money

### Risk Management:

**paper_trading.rs**:
- No position limit (can buy all symbols if signals align)
- 5 shares per trade = ~$500-$4,000 per position
- Suitable for testing signal quality

**trading_bot.rs**:
- Max 3 concurrent positions
- Prevents over-concentration
- Better for real money later

## 📊 Monitoring Your Trades

**Live Dashboard:**
https://app.alpaca.markets/paper/dashboard/overview

**Activity Tab:**
See all orders, fills, and executions

**Positions Tab:**
Real-time P&L tracking

**Account Tab:**
Cash, buying power, portfolio value

## 🔧 Customization

Edit the examples to adjust:

```rust
// Change position size (shares per trade)
let position_size = 5.0;  // Default: 5 shares

// Change symbols by editing config/stocks.json
// Enable/disable stocks without code changes:
// {
//   "symbol": "TSLA",
//   "enabled": true,  // Set to false to exclude
// }

// Adjust max positions (trading_bot only)
max_positions: 3,  // Increase for more diversification

// Change thresholds
let (rsi_oversold, rsi_overbought, momentum_threshold) =
    (35.0, 65.0, 0.025);  // Tweak sensitivity
```

## ⚠️ Important Notes

### Market Hours
- Pre-market: 4:00 AM - 9:30 AM ET
- Regular: 9:30 AM - 4:00 PM ET (best liquidity)
- After-hours: 4:00 PM - 8:00 PM ET

**Recommendation**: Only trade during regular hours for best fills.

### Data Delays
- Free tier: 15-minute delayed quotes
- For real-time: Upgrade to Alpaca Data Feed Pro

### Order Types
- **Market orders** (current): Execute immediately at current price
- **Limit orders** (better): Set max/min price, may not fill

To use limit orders, edit OrderRequest:
```rust
r#type: OrderType::Limit,
limit_price: Some(current_price * 0.99),  // Buy 1% below market
```

## 🚀 Recommended Workflow

### 1. Test with dry-run (Day 1-3)
```powershell
# Run a few times during market hours
.\target\release\dollarbill.exe trade --dry-run
```
- Verify signals make sense
- Check order execution
- Monitor fills on Alpaca dashboard

### 2. Run live bot during market hours (Week 1-2)
```powershell
# 9:30 AM - 4:00 PM ET
$env:ALPACA_API_KEY            = "your-key"
$env:ALPACA_API_SECRET         = "your-secret"
$env:DOLLARBILL_SMTP_PASSWORD  = "your-app-password"   # omit if alerts disabled
.\target\release\dollarbill.exe trade --live
```
- Let it trade for a week
- Track performance daily
- Compare to backtest expectations

### 3. Analyze results
- TSLA/NVDA should be profitable (historically +400-700%)
- META/AMZN/GOOGL should be positive (historically +80-200%)
- If losing money, check:
  * Are signals firing correctly?
  * Is volatility still high? (calc changes over time)
  * Market regime change? (2026 vs 2021-2025 data)

### 4. Optimize
- Adjust thresholds based on paper results
- Try different position sizes
- Add stop-loss orders
- Implement take-profit levels

### 5. Run unattended (Week 3+)

Once the bot is stable, move to automated deployment so it survives reboots and restarts on crash.

**Windows — `start_bot.ps1` (simplest):**
```powershell
# Copy credentials template
Copy-Item .env.example .env   # then edit with real keys

# Dry-run to confirm everything loads
.\scripts\start_bot.ps1 -DryRun

# Live mode — logs written to data\logs\
.\scripts\start_bot.ps1
```

**Windows — Task Scheduler (auto-start on logon):**
```powershell
# Edit deploy\dollarbill-task.xml: set your username in <UserId>
# Then import:
schtasks /Create /XML deploy\dollarbill-task.xml /TN "DollarBill\TradingBot"

# Verify
schtasks /Query /TN "DollarBill\TradingBot"
```

**Linux — Docker (recommended for servers):**
```bash
cp .env.example .env    # fill in keys
docker compose up -d bot
docker compose logs -f bot
docker compose down     # stop; data persists in ./data
```

**Linux — systemd:**
```bash
sudo cp target/release/dollarbill /usr/local/bin/
sudo cp deploy/dollarbill.service /etc/systemd/system/

# Create secrets file (chmod 600 — never world-readable)
sudo mkdir -p /etc/dollarbill
sudo tee /etc/dollarbill/secrets.env <<'EOF'
ALPACA_API_KEY=your-key
ALPACA_API_SECRET=your-secret
DOLLARBILL_SMTP_PASSWORD=your-app-password
EOF
sudo chmod 600 /etc/dollarbill/secrets.env

sudo systemctl daemon-reload
sudo systemctl enable --now dollarbill
journalctl -u dollarbill -f
```
The service restarts automatically on crash (up to 5 times per 5-minute window) and sends SIGTERM on stop, which triggers the bot’s graceful-shutdown handler (cancels open orders).  

## 💡 Next Steps

- [ ] Get Alpaca API keys and test paper_trading
- [ ] Configure email alerts in `config/trading_bot_config.json` (set `enabled: true`, add Gmail App Password)
- [ ] Run trading_bot for 1-2 weeks
- [ ] Compare paper P&L to backtest expectations
- [ ] Fine-tune strategies based on live results
- [ ] Deploy unattended: `scripts/start_bot.ps1` (Windows) or Docker / systemd (Linux)
- [ ] Explore options trading (original goal!)

**Options Trading**: Once stock strategies are proven, apply same logic to options using Alpaca's options API (similar to fetch_options.py but with live trading).
