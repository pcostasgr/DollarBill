# Advanced Features Guide

## 🎯 Recently Added Features

### 1. JSON Configuration System ⭐ ENHANCED

**What it does:** Centralizes all stock configuration in a single `config/stocks.json` file that controls the entire pipeline. Individual config files now contain only algorithm parameters, not symbol lists.

**Configuration file structure:**
```json
// config/stocks.json - Central symbol management
{
  "stocks": [
    {
      "symbol": "TSLA",
      "market": "US",
      "sector": "Technology",
      "enabled": true,
      "notes": "High volatility, good for options"
    },
    {
      "symbol": "AAPL",
      "market": "US",
      "sector": "Technology",
      "enabled": true
    }
  ]
}

// Individual configs - Parameters only
{
  "trading": {
    "position_size_shares": 100,
    "max_positions": 3
    // No symbols array - loaded from stocks.json
  }
}
```

**How it works:**
- **All examples** automatically read enabled stocks from `config/stocks.json`
- **Individual configs** contain only algorithm parameters (thresholds, sizes, limits)
- **Enable/disable stocks** by editing `stocks.json` - no code changes needed
- **Add new stocks** by editing JSON only

**Benefits:**
- Single source of truth for all symbols across the entire platform
- Clean separation between symbol management and algorithm parameters
- Enable/disable stocks globally without touching individual configs
- Consistent symbol universe across all examples and scripts

### 2. Greeks Output for Each Signal ✅

**What it does:** Calculates and displays Delta, Gamma, Vega, and Theta for every trade signal.

**Example output:**
```
Symbol Type   Strike   Bid      Ask      Model Val  Edge %   Delta    Gamma    Vega     Theta
-----------------------------------------------------------------------------------------------------------
TSLA   Call   $440.00  $12.50   $13.00   $15.20      17.5%   0.625   0.0035   85.20    -12.50
AAPL   Put    $270.00  $8.20    $8.50    $10.10      22.0%  -0.350   0.0042   45.30    -8.75
```

**How to use:**
```bash
cargo run --release --example multi_symbol_signals
```

### 3. Portfolio Risk Metrics ✅

**What it does:** Aggregates Greeks across your top positions to show portfolio-level risk exposure.

**Example output:**
```
📊 PORTFOLIO RISK METRICS

Top 10 Positions (1 contract each):
  Portfolio Delta:      2.450  (directional exposure)
  Portfolio Gamma:    0.0320  (convexity)
  Portfolio Vega:    427.50  (vol sensitivity)
  Portfolio Theta:   -85.30  (daily decay)
  Combined Edge:   $145.25  (per contract)

📈 Risk Analysis:
  ✓ Delta-neutral: Low directional risk (2.45)
  ⚠ High vega: $428 exposure to 1% IV change
    Portfolio benefits if implied volatility rises
  ⚠ High theta decay: $-85.30/day time decay
    Position loses value each day - consider shorter holding period
```

**Interpretation:**
- **Delta < ±5**: Portfolio is direction-neutral (good!)
- **High Vega**: You profit if volatility increases
- **Negative Theta**: You lose money each day from time decay

### 5. Heston Stochastic Volatility Backtesting ⭐ NEW

**What it does:** Advanced options strategy backtesting using the Heston stochastic volatility model instead of constant volatility Black-Scholes.

**Key Advantages:**
- **Realistic pricing**: Captures volatility smiles, skews, and term structure
- **Professional-grade**: Used by hedge funds and market makers worldwide
- **Better edge detection**: Finds true mispricings that Black-Scholes misses
- **Live calibration**: Parameters fitted to current market options data

**How to use:**
```bash
# 1. Calibrate Heston parameters to live market data
cargo run --example calibrate_live_options

# 2. Run Heston backtesting
cargo run --example backtest_heston
```

**Example Results:**
```
NVDA Short-Term Strategy (Heston vs Black-Scholes):

Heston Results:
- Total P&L: +270.12%
- Sharpe Ratio: 2.67
- 385 trades, 47.5% win rate

Black-Scholes Results:
- Total P&L: +150%
- Sharpe Ratio: 1.8
- Same strategy, same signals

Improvement: +80% better returns with Heston pricing!
```

**What makes it special:**
- **Carr-Madan analytical pricing**: 4161x faster than Monte Carlo
- **Multi-timeframe testing**: Short-term (14-day), medium-term (30-day), long-term (60-day)
- **Proper position sizing**: Accounts for option contracts (100 shares each)
- **Realistic P&L**: Includes commissions, proper contract sizing, time decay

**Pipeline:**
```bash
# Step 1: Extract IV from options data → CSV files
cargo run --release --example vol_surface_analysis

# Step 2: Create interactive 3D plots → HTML files
python plot_vol_surface.py
```

**Output files:**
- `data/{symbol}_vol_surface.csv` - Raw volatility data
- `images/{symbol}_vol_surface_3d.html` - 3D interactive surface
- `images/{symbol}_vol_smile.html` - 2D smile (IV vs Strike)
- `images/{symbol}_term_structure.html` - IV vs Time to Expiry

**Example vol_surface_analysis output:**
```
📈 VOLATILITY SMILE - TSLA

CALLS:
Strike     Moneyness    IV %       Volume
---------------------------------------------
420.00     0.9589       42.30      2500
430.00     0.9817       41.80      3200
440.00     1.0046       40.50      4100  ← ATM
450.00     1.0274       41.20      2800
460.00     1.0503       42.80      1500

PUTS:
Strike     Moneyness    IV %       Volume
---------------------------------------------
420.00     0.9589       45.20      1800
430.00     0.9817       43.50      2400
440.00     1.0046       42.10      3500  ← ATM
450.00     1.0274       41.50      1900
460.00     1.0503       40.80      1200

📊 ATM Volatility Analysis:
  ATM Call IV:  40.5%
  ATM Put IV:   42.1%
  ⚠ Put skew detected: Puts trading at 1.6% premium
    Market pricing in downside protection
```

**Volatility Smile Patterns:**
- **Flat smile**: Market is calm, no fear/greed
- **Put skew** (higher IV on puts): Fear of crash
- **Call skew** (higher IV on calls): Speculation/FOMO
- **Smile** (both wings high): Uncertainty in both directions

## 📊 Complete Workflow Example

### Full Analysis Pipeline

```bash
# 0. Configure stocks (edit config/stocks.json to enable/disable symbols)
# All components automatically use enabled stocks from config

# 1. Fetch market data for enabled stocks
python py/fetch_multi_stocks.py    # Historical stock prices
python py/fetch_multi_options.py   # Live options chains

# 2. Generate trade signals with Greeks
cargo run --release --example multi_symbol_signals

# 3. Analyze volatility surfaces
cargo run --release --example vol_surface_analysis

# 4. Visualize volatility (requires: pip install pandas plotly)
python py/plot_vol_surface.py
```

### Quick Start Scripts

**Windows PowerShell:**
```powershell
.\scripts\run_multi_signals.ps1    # Signals with Greeks & portfolio risk
.\scripts\run_vol_surface.ps1      # Full vol surface pipeline
```

## 🔬 Technical Details

### Greeks Calculation
- Uses **Black-Scholes-Merton** for analytical Greeks
- Implied vol from **Heston model calibration**
- Includes dividend yield support (q parameter)

### Volatility Surface
- **Newton-Raphson** method for implied vol extraction
- Handles both calls and puts separately
- Filters out illiquid options (zero bids)
- Exports to CSV for external analysis

### Risk Metrics
- Portfolio delta: Sum of deltas (accounting for buy/sell)
- Portfolio gamma, vega, theta: Direct sum
- Automatic delta-neutral detection (< ±5)
- Vega exposure warnings (> ±$100)

## 📈 Python Visualization Requirements

```bash
pip install pandas plotly
```

**Alternative visualization (if plotly not available):**
```python
import pandas as pd
import matplotlib.pyplot as plt
from mpl_toolkits.mplot3d import Axes3D

df = pd.read_csv('data/tsla_vol_surface.csv')
fig = plt.figure()
ax = fig.add_subplot(111, projection='3d')
ax.scatter(df['Strike'], df['TimeToExpiry'], df['ImpliedVol']*100)
ax.set_xlabel('Strike')
ax.set_ylabel('Time to Expiry')
ax.set_zlabel('Implied Vol %')
plt.show()
```

## 🎓 Understanding the Output

### When to Trade
- **High positive edge + delta-neutral**: Good risk/reward
- **Volatility skew**: Trade against the skew (sell high IV, buy low IV)
- **Theta decay**: Short-dated options lose value fast - only for quick trades

### Red Flags
- **High portfolio delta**: Not direction-neutral, risky
- **Excessive theta decay**: Position bleeding money daily
- **Low vega**: Can't profit from vol changes
- **Extreme IV skew**: Market expects large move

## 📁 File Reference

**Configuration:**
- `config/stocks.json` - Central stock configuration ⭐ NEW
- `src/config.rs` - JSON configuration loader ⭐ NEW

**New Examples:**
- `examples/multi_symbol_signals.rs` - Greeks + portfolio risk (config-driven)
- `examples/vol_surface_analysis.rs` - IV extraction

**New Modules:**
- `src/utils/vol_surface.rs` - Volatility surface tools

**Python Scripts (Config-Driven):**
- `py/plot_vol_surface.py` - 3D visualization
- `py/fetch_multi_stocks.py` - Multi-symbol stock data
- `py/fetch_multi_options.py` - Multi-symbol options data

**Run Scripts:**
- `scripts/run_multi_signals.ps1` - Full signal analysis
- `scripts/run_vol_surface.ps1` - Volatility pipeline
