# DollarBill 🦀📈

![DollarBill](DollarBill.png)

**An educational options pricing and trading analysis tool built in Rust through AI pair programming.**

DollarBill demonstrates options mathematics, Greeks calculations, and basic trading strategies through a clean Rust implementation. Features Black-Scholes and Heston pricing models, volatility surface analysis, backtesting, and paper trading integration.

## 🤖 Built Entirely with AI

**This project was created through conversational AI development** - every line of code emerged from natural language descriptions with **Claude Sonnet 4.6** and **Grok**. From the Heston FFT implementation to the Nelder-Mead optimizer, it showcases how AI can build sophisticated mathematical software through "vibe coding."

No traditional programming sessions. Just prompts, iterations, and Rust. 🚀

## 🎯 What DollarBill Actually Is

### ✅ **Real Capabilities**
- **Options Pricing**: Black-Scholes-Merton, Heston stochastic volatility, and SABR models
- **SABR Model**: Hagan et al. (2002) analytic approximation — ATM/OTM branches, CEV backbone, smile generation, and calibration
- **Gauss-Laguerre Quadrature**: Pure Rust GL engine (2–128 nodes) — matches QuantLib to 6 significant figures
- **Batch Pricing with CF Cache**: 50 strikes × 10 maturities in 1.16 ms (2.3 µs/opt amortized, 10× faster)
- **Greeks Calculation**: Delta, Gamma, Vega, Theta, Rho for risk analysis
- **Model Calibration**: Heston parameter fitting using custom Nelder-Mead optimizer
- **Volatility Analysis**: IV extraction, volatility surfaces, and smile analysis
- **Alpaca Trading Integration**: Paper *and* live trading via Alpaca API (`APCA_LIVE=1`)
- **Production-Hardened Trading Bot**: Fully safety-gated personality bot with market-hours enforcement, PDT protection, circuit breakers, fill confirmation, audit logging, and crash recovery
- **Live Options Pricer**: `live_pricer` wires Yahoo live feed → TTL-cached Heston calibration → per-option edge signals with Greeks in a configurable polling loop
- **Backtesting**: Historical strategy evaluation with P&L tracking and annualised Sharpe and Sortino ratios
- **QuantLib Validator**: `py/validate_pricing.py` cross-validates BSM, Heston GL-64/128, and American binomial pricing against QuantLib v1.41; `--speed` flag benchmarks Rust vs Python timings
- **Stock Classification**: Basic personality-driven strategy selection (3 types)
- **Short Options**: SellCall and SellPut support for premium collection strategies
- **Multi-Leg Strategies**: Iron condors, credit spreads, straddles, strangles with customizable templates
- **Strategy Templates**: Configurable strategy builders for quick backtesting
- **Portfolio Management**: Position sizing, risk analytics, multi-strategy allocation, performance attribution

### ❌ **What It's NOT**
- Institutional-grade platform  
- Competitor to professional platforms
- Enterprise solution
- Real options API support (live options *orders* via Alpaca require a separate approval tier; the bot trades underlying equities only)

### 🎓 **Perfect For**
- Learning options pricing mathematics
- Understanding Rust in quantitative finance
- Experimenting with basic trading strategies
- Educational backtesting and paper trading
- Seeing AI-assisted development in action

## 🚀 Quick Start

### Prerequisites
```bash
# Rust (2021 edition or later)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python (optional, for data fetching and visualization)
pip install pandas plotly yfinance
```

### Installation
```bash
git clone https://github.com/yourusername/DollarBill.git
cd DollarBill
cargo build --release
```

### Basic Usage

**1. Configure Stocks** (edit `config/stocks.json`):
```json
{
  "stocks": [
    { "symbol": "TSLA", "market": "US", "sector": "Automotive", "enabled": true },
    { "symbol": "AAPL", "market": "US", "sector": "Technology",  "enabled": true },
    { "symbol": "NVDA", "market": "US", "sector": "Technology",  "enabled": true }
  ]
}
```

**2. Fetch Market Data**:
```bash
python py/fetch_multi_stocks.py    # historical stock data
python py/fetch_multi_options.py   # live options chains
```

**3. Use the CLI**:

After `cargo build --release`, the `dollarbill` binary exposes six subcommands:

```powershell
# Interactive pricing demo (default symbol: TSLA)
.\target\release\dollarbill.exe demo --symbol TSLA

# Price a single option: AAPL $200 strike, 3-month expiry, 5% rate
.\target\release\dollarbill.exe price AAPL 200 --dte 0.25 --rate 0.05

# Backtest all configured symbols and save performance matrix
.\target\release\dollarbill.exe backtest --save

# Backtest a single symbol
.\target\release\dollarbill.exe backtest --symbol NVDA

# Print trading signals for all configured symbols
.\target\release\dollarbill.exe signals

# Print signals and stream live prices via Alpaca WebSocket
.\target\release\dollarbill.exe signals --live

# Calibrate Heston model parameters for a symbol
.\target\release\dollarbill.exe calibrate TSLA

# Paper-trade bot (dry-run: print orders, don't submit)
.\target\release\dollarbill.exe trade --dry-run

# Paper-trade bot with live Alpaca streaming + SQLite persistence
.\target\release\dollarbill.exe trade --live

# Full help
.\target\release\dollarbill.exe --help
```

**4. Run Examples**:
```bash
# Generate trading signals with Greeks
cargo run --example multi_symbol_signals

# Analyze stock personalities
cargo run --example enhanced_personality_analysis

# Analyze volatility surfaces
cargo run --example vol_surface_analysis

# Backtest long options strategies
cargo run --example backtest_strategy

# Backtest with Heston model
cargo run --example backtest_heston

# Backtest short options (covered calls, cash-secured puts)
cargo run --example backtest_short_options

# Multi-leg strategies — Iron condor (neutral income strategy)
cargo run --example iron_condor

# Credit spreads (bull put spread, bear call spread)
cargo run --example credit_spreads

# Strategy templates (customizable parameters)
cargo run --example strategy_templates

# Portfolio management (position sizing, risk analytics, allocation)
cargo run --example portfolio_management

# Paper trade (requires Alpaca API keys — paper endpoint)
cargo run --example personality_based_bot

# Paper trade — continuous mode (runs on schedule, graceful Ctrl+C shutdown)
cargo run --example personality_based_bot -- --continuous

# Live trade — real money (set APCA_LIVE=1 + live Alpaca keys)
APCA_LIVE=1 ALPACA_API_KEY=<key> ALPACA_API_SECRET=<secret> cargo run --release --example personality_based_bot -- --continuous

# Live options pricer — Yahoo live feed → Heston calibration → edge signals (loop mode)
cargo run --example live_pricer

# Live options pricer — single pass then exit (useful for testing)
cargo run --example live_pricer -- --once

# Live options pricer — custom poll interval and edge threshold
cargo run --example live_pricer -- --interval 30 --min-edge-pct 3.0

# 3D volatility visualizations (requires Python)
python py/plot_vol_surface.py
```

### Release Build Workflow (Recommended)

For faster execution, pre-build the release binaries once, then run the pipeline quickly:

```powershell
# Step 1: Build release binaries (do this once)
.\scripts\build_release.ps1

# Step 2: Run the complete pipeline quickly (no compilation time)
.\scripts\run_release_pipeline.ps1
```

This saves significant time compared to `cargo run --release` which compiles each time.

### PowerShell Quick Scripts

```powershell
# Python Environment Management
cmd /c ".\scripts\setup_python.bat"        # Setup Python environment
cmd /c ".\scripts\test_python.bat"         # Test Python setup
cmd /c ".\scripts\collect_data_fixed.bat"  # Fetch market data

# Build release binaries once for fast execution
.\scripts\build_release.ps1

# Complete pipeline: Data fetch -> Calibration -> Signals -> Paper trading (fast)
.\scripts\run_release_pipeline.ps1

# Complete pipeline: Data fetch -> Calibration -> Signals -> Paper trading (with compilation)
.\scripts\run_full_pipeline.ps1

# Personality-driven pipeline: Stock analysis -> Strategy matching -> Optimized trading
cargo run --example personality_driven_pipeline

# Personality-based live trading bot
cargo run --example personality_based_bot -- --dry-run   # Test without trading
cargo run --example personality_based_bot                # Single live iteration
cargo run --example personality_based_bot -- --continuous 5  # Continuous trading

# Trade signals with full Greeks
.\scripts\run_multi_signals.ps1

# Complete volatility pipeline
.\scripts\run_vol_surface.ps1
```

## 📊 Example Output

### Personality Analysis Output

```
🚀 DollarBill Stock Personality Analysis
===============================================

🧠 Classification for TSLA:
   📊 Personality: VolatileBreaker (confidence: 30.0%)
   📈 Vol Percentile: 91.7% | Trend: 45.2% | Reversion: 62.1%
   🎯 Market Regime: HighVol | Beta: 1.23 | Sector: Automotive
   🎯 Best strategies: ["Iron Butterfly", "Volatility Harvesting", "Short Straddles"]
   ❌ Avoid strategies: ["Directional Bets", "Long Options", "Momentum Strategies"]

🧠 Classification for PLTR:
   📊 Personality: MomentumLeader (confidence: 50.0%)
   📈 Vol Percentile: 97.2% | Trend: 98.5% | Reversion: 23.4%
   🎯 Market Regime: HighVol | Beta: 2.14 | Sector: Software
   🎯 Best strategies: ["Short-Term Momentum", "Breakout Trading", "Trend Following"]
   ❌ Avoid strategies: ["Long-Term Holding", "Mean Reversion", "Iron Butterflies"]
```

### Trade Signals with Greeks

```
===============================================================
MULTI-SYMBOL TRADE SIGNAL GENERATOR
Parallel Heston Calibration & Options Mispricing Detection
===============================================================

Symbol Type   Strike   Bid      Ask      Model Val  Edge %   Delta    Gamma    Vega     Theta
-----------------------------------------------------------------------------------------------------------
TSLA   Call   $440.00  $12.50   $13.00   $15.20      17.5%   0.625   0.0035   85.20    -12.50
AAPL   Put    $270.00  $8.20    $8.50    $10.10      22.0%  -0.350   0.0042   45.30    -8.75
NVDA   Call   $850.00  $25.00   $26.00   $29.50      13.5%   0.540   0.0028   95.40    -15.20
```

### Portfolio Risk Metrics

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
  ⚠ High theta decay: $-85.30/day time decay
```

### Volatility Smile

```
📈 VOLATILITY SMILE - TSLA

Strike     Moneyness    IV %       Volume
420.00     0.9589       42.30      2500
430.00     0.9817       41.80      3200
440.00     1.0046       40.50      4100  ← ATM
450.00     1.0274       41.20      2800
460.00     1.0503       42.80      1500

ATM Call IV: 40.5% | ATM Put IV: 42.1%
⚠ Put skew detected: Puts trading at 1.6% premium
```

### Backtest Results

```
================================================================================
BACKTEST RESULTS - TSLA
================================================================================
Period: 2025-01-03 to 2026-01-02
Initial Capital: $100000.00 | Final Capital: $146402.25

Total P&L:   $46406.25 (46.41%)  |  Sharpe Ratio: 1.22  |  Max Drawdown: 0.00%
Trades: 2 | Wins: 2 (100%) | Avg Win: $23,203 | Profit Factor: inf
================================================================================
```

## 🔧 Architecture

### Core Models
- **Black-Scholes-Merton**: Analytical European pricing with dividends
- **Heston (Gauss-Laguerre)**: Lord-Kahl CF with pure Rust GL quadrature — **33 µs/call** single, **2.3 µs/opt** batch with CF cache
- **Heston (Carr-Madan)**: Legacy adaptive Simpson integration path
- **SABR**: Hagan et al. (2002) analytic approximation — ATM/OTM branches, β backbone, smile + calibration
- **Greeks**: All first-order sensitivities
- **Implied Volatility**: Newton-Raphson solver

### Data Pipeline
- **Market Data**: Yahoo Finance API integration
- **Storage**: CSV (historical) + JSON (options chains)
- **Configuration**: Central JSON-based stock management

### Trading Features
- **Strategy Classification**: 3 basic stock personality types
- **Signal Generation**: Model vs market price comparison
- **Risk Management**: Portfolio Greeks aggregation, daily drawdown circuit breaker, buying-power validation
- **Backtest Analytics**: Sharpe ratio (excess return / std dev × √252) and Sortino ratio (downside deviation below risk-free rate) in every `BacktestResult`
- **Alpaca API Integration**: Paper and live trading with HTTP retries, exponential backoff, and fill confirmation
- **Persistent State**: `bot_state.json` survives crashes — daily trade counter is restored across restarts
- **Audit Log**: Every trade decision written to `trade_audit.csv` (append-only, atomic header check)

## 🛡️ Production Safety Features (Trading Bot)

`examples/personality_based_bot.rs` is fully hardened for real-money operation:

| Guard | Detail |
|---|---|
| **Market hours** | Checks Alpaca `/v2/clock`; skips iteration if market is closed |
| **PDT protection** | Blocks trades if account is flagged as pattern-day-trader with equity < $25 000 |
| **Daily drawdown circuit breaker** | Halts if daily loss exceeds `max_daily_drawdown_pct` (config-driven) |
| **Max daily trades** | Hard cap via `max_daily_trades`; counter persists across crashes in `bot_state.json` |
| **Buying-power validation** | Checks live buying power from Alpaca before every BUY/ADD |
| **Zero-size guard** | Skips orders where computed quantity ≤ 0 |
| **Single action per symbol** | `acted` flag ensures at most one order per symbol per iteration |
| **Duplicate order dedup** | Checks for existing open orders before submitting a new one |
| **Fill confirmation** | Polls `await_order_fill()` (up to 30 s) after every submission |
| **Stop-loss retry** | One additional close attempt on failure before logging `STOP_LOSS_FAILED` |
| **Stale-price guard** | Skips SL/TP evaluation if `current_price` is 0.0 |
| **Equity sanity check** | Halts iteration immediately if account equity parses to 0.0 |
| **HTTP retries** | 3 retries with 500 ms / 1 s / 2 s exponential backoff on every API call |
| **Graceful shutdown** | `Ctrl+C` cancels all open orders before exit |
| **Audit log** | Every decision (BUY, SELL, SKIP, errors) appended to `trade_audit.csv` |
| **Crash recovery** | `bot_state.json` written atomically after every confirmed fill |

### Running in Live Mode
```bash
# 1. Paper trading (default — uses paper-api.alpaca.markets)
cargo run --release --example personality_based_bot -- --continuous

# 2. Live trading (CAUTION: real money)
APCA_LIVE=1 \
  ALPACA_API_KEY=<your_live_key> \
  ALPACA_API_SECRET=<your_live_secret> \
  cargo run --release --example personality_based_bot -- --continuous
```

## 📡 Live Options Pricer

`examples/live_pricer.rs` connects all the pieces for real-time options analysis without placing orders:

| Feature | Detail |
|---|---|
| **Live spot price** | Yahoo Finance via `yahoo_finance_api` crate |
| **Live options chain** | Yahoo `/v7/finance/options/{symbol}` — bid/ask/IV per expiry |
| **Liquidity filter** | Configurable min volume and max spread % (from `signals_config.json`) |
| **Heston calibration** | Nelder-Mead fit with configurable TTL cache (default 15 min) |
| **Model pricing** | Carr-Madan Heston call/put per live option |
| **Edge signals** | Reports BUY/SELL where `|model − market| > min_edge_pct%` *and* `> min_edge_$` |
| **Greeks** | BSM Delta and Vega per signal for position sizing |
| **Poll loop** | Configurable interval with `tokio::select!` Ctrl+C exit |

```bash
cargo run --example live_pricer                             # 60s loop, 15min recalibrate
cargo run --example live_pricer -- --once                   # single pass
cargo run --example live_pricer -- --interval 30            # 30s poll
cargo run --example live_pricer -- --calibrate-ttl 300      # recalibrate every 5min
cargo run --example live_pricer -- --min-edge-pct 3.0       # 3% edge threshold
cargo run --example live_pricer -- --expiry 0               # nearest expiry
```

Output columns per signal: `TYPE | STRIKE | BID | ASK | MODEL | EDGE$ | EDGE% | DELTA | VEGA | ACTION [ATM]`

## 🔬 Technical Details

### Pricing Models

**Black-Scholes-Merton:**
- Analytical solution for European options with dividend yield support
- All Greeks: Δ, Γ, ν, Θ, ρ computed in a single pass
- Abramowitz & Stegun CDF approximation, cross-validated to 5 decimal places

**Heston Stochastic Volatility:**
- **Gauss-Laguerre quadrature** (primary): Lord-Kahl Formulation 2 CF, 2–128 GL nodes
- **Carr-Madan / adaptive Simpson** (legacy): original characteristic function path
- QuantLib-validated: matches `AnalyticHestonEngine` to 6 significant figures
- Put-call parity to machine precision; configurable via `IntegrationMethod` enum

**SABR Stochastic Volatility:**
- Hagan, Kumar, Lesniewski & Woodward (2002) analytic approximation
- Separate ATM and OTM/ITM formulas to avoid 0/0 singularities
- β ∈ [0,1]: Normal (β=0), CEV (β=0.5), Log-normal (β=1) backbone
- `sabr_smile()` and `calibrate_sabr()` included; zero-ν fallback to CEV

**Nelder-Mead Optimizer:**
- Pure Rust simplex implementation; configurable reflection/expansion/contraction
- Used for Heston calibration and SABR fitting; parameter bounds enforcement

**Greeks struct:**
```rust
Greeks {
    price: f64,   // Option price
    delta: f64,   // ∂V/∂S — directional exposure
    gamma: f64,   // ∂²V/∂S² — convexity
    theta: f64,   // ∂V/∂t — time decay
    vega:  f64,   // ∂V/∂σ — vol sensitivity
    rho:   f64,   // ∂V/∂r — rate sensitivity
}
```

## 📂 Project Structure

```
DollarBill/
├── config/
│   └── stocks.json                    # Central stock configuration
├── src/
│   ├── lib.rs                          # Library exports
│   ├── main.rs                         # Main entry point
│   ├── config.rs                       # JSON configuration loader
│   ├── models/                         # Pricing models
│   │   ├── bs_mod.rs                   # Black-Scholes-Merton + Greeks
│   │   ├── heston.rs                   # Heston model structures
│   │   ├── heston_analytical.rs        # Heston semi-analytical (GL + Carr-Madan)
│   │   ├── gauss_laguerre.rs           # Pure Rust GL quadrature engine
│   │   ├── sabr.rs                     # SABR stochastic volatility (Hagan et al. 2002)
│   │   └── american.rs                 # American options (binomial tree)
│   ├── calibration/                    # Model calibration
│   │   ├── heston_calibrator.rs        # Heston parameter fitting
│   │   ├── nelder_mead.rs              # Custom optimizer
│   │   └── market_option.rs            # Market data structures
│   ├── market_data/                    # Data loaders
│   │   ├── csv_loader.rs               # CSV parsing
│   │   ├── options_json_loader.rs      # JSON options chains
│   │   ├── real_market_data.rs         # Yahoo Finance integration
│   │   └── symbols.rs                  # Symbol definitions
│   ├── strategies/                     # Trading strategies
│   │   ├── factory.rs                  # Strategy factory
│   │   ├── matching.rs                 # Signal-to-strategy matching
│   │   ├── momentum.rs                 # Momentum strategy
│   │   ├── mean_reversion.rs           # Mean reversion strategy
│   │   ├── breakout.rs                 # Breakout strategy
│   │   ├── cash_secured_puts.rs        # Cash-secured put selling
│   │   ├── short_strangle.rs           # Short strangle strategy
│   │   ├── spreads.rs                  # Iron condors and credit spreads
│   │   ├── templates.rs                # Configurable strategy templates
│   │   ├── ensemble.rs                 # Ensemble strategy combiner
│   │   ├── mispricing.rs               # Model vs market mispricing detection
│   │   ├── vol_arbitrage.rs            # Volatility arbitrage strategy
│   │   ├── vol_mean_reversion.rs       # Vol trading strategy
│   │   └── mod.rs                      # Strategy trait
│   ├── analysis/                       # Stock analysis system
│   │   ├── stock_classifier.rs         # Personality classification
│   │   ├── advanced_classifier.rs      # Multi-dimensional feature analysis
│   │   ├── performance_matrix.rs       # Strategy performance tracking
│   │   └── mod.rs                      # Analysis exports
│   ├── backtesting/                    # Backtesting framework
│   │   ├── engine.rs                   # Backtest orchestration
│   │   ├── position.rs                 # Position tracking
│   │   ├── trade.rs                    # Trade records
│   │   ├── metrics.rs                  # Performance analytics
│   │   └── mod.rs                      # Module exports
│   ├── alpaca/                         # Paper trading integration
│   │   ├── client.rs                   # Alpaca API client
│   │   ├── types.rs                    # API data structures
│   │   └── mod.rs                      # Module exports
│   └── utils/                          # Utilities
│       ├── vol_surface.rs              # Volatility surface tools
│       ├── action_table_out.rs         # Output formatting
│       └── pnl_output.rs               # P&L calculations
├── examples/
│   ├── multi_symbol_signals.rs         # Main: Signals + Greeks + Risk
│   ├── live_pricer.rs                  # Live Yahoo feed → Heston cache → edge signal loop
│   ├── vol_surface_analysis.rs         # Volatility surface extraction
│   ├── backtest_strategy.rs            # Black-Scholes strategy backtesting
│   ├── backtest_heston.rs              # Heston model backtesting
│   ├── backtest_short_options.rs       # Short options backtesting
│   ├── backtest_short_strangles.rs     # Short strangle backtesting
│   ├── calibrate_live_options.rs       # Heston calibration demo
│   ├── credit_spreads.rs               # Bull put / bear call spread demo
│   ├── iron_condor.rs                  # Iron condor neutral income demo
│   ├── iron_condor_strategy.rs         # Iron condor strategy (extended)
│   ├── mispricing_detection.rs         # Model vs market mispricing scanner
│   ├── performance_benchmarks.rs       # Pricing engine benchmarks
│   ├── portfolio_management.rs         # Portfolio management demo
│   ├── strategy_matching.rs            # Strategy matching demo
│   ├── strategy_showcase.rs            # Strategy showcase
│   ├── strategy_templates.rs           # Customizable strategy templates
│   ├── trade_signals.rs                # Basic signal generation
│   ├── alpaca_demo.rs                  # Alpaca API demo
│   ├── paper_trading.rs                # Paper trading with momentum
│   ├── trading_bot.rs                  # Continuous trading bot
│   ├── test_keys.rs                    # Alpaca API key testing
│   ├── test_yahoo_options.rs           # Yahoo Finance options validation
│   ├── personality_driven_pipeline.rs  # Personality-optimized trading
│   ├── personality_based_bot.rs        # Production-hardened trading bot (paper + live)
│   └── enhanced_personality_analysis.rs
├── py/
│   ├── validate_pricing.py             # QuantLib v1.41 cross-validation + speed bench
│   ├── fetch_multi_stocks.py           # Stock data fetcher (config-driven)
│   ├── fetch_multi_options.py          # Options chain fetcher (config-driven)
│   ├── plot_vol_surface.py             # 3D volatility visualization
│   ├── fetch_options.py                # Single symbol options fetcher
│   └── get_tesla_stock_csv.py          # Tesla CSV downloader
├── scripts/
│   ├── build_release.ps1               # Build all release binaries
│   ├── run_release_pipeline.ps1        # Fast pipeline (pre-built binaries)
│   ├── run_full_pipeline.ps1           # Full pipeline with compilation
│   ├── run_multi_signals.ps1           # Run signals
│   ├── run_vol_surface.ps1             # Vol pipeline
│   ├── setup_python.bat                # Python environment setup
│   ├── test_python.bat                 # Python environment testing
│   └── collect_data_fixed.bat          # Data collection pipeline
├── tests/                              # Integration + unit test suite
├── benches/                            # Criterion.rs benchmarks
├── data/                               # Market data storage (CSV + JSON)
├── docs/                               # Guides and documentation
├── images/                             # Generated charts and visualizations
bot_state.json                          # Runtime: daily trade counter (crash recovery)
trade_audit.csv                         # Runtime: append-only audit log
└── Cargo.toml                          # Rust dependencies
```

## 🎓 Understanding the Output

### Trade Signals
- **Edge %** — Model price premium over market (buy signal if > threshold)
- **Delta** — Position direction (+call / −put exposure)
- **Gamma** — Price acceleration (convexity)
- **Vega** — Profit from volatility increase
- **Theta** — Daily time decay (always negative for long options)

### Portfolio Risk
- **Delta < ±5** — Direction-neutral (market-neutral strategy)
- **High Vega** — Profits from vol expansion (long gamma/vega)
- **Negative Theta** — Loses value daily (needs quick moves to profit)

### Volatility Patterns
- **Flat Smile** — Market is calm, no fear/greed premium
- **Put Skew** — Higher IV on puts = crash protection being bought
- **Call Skew** — Higher IV on calls = speculation/FOMO
- **Smile** — Both wings elevated = uncertainty

## 📈 Technology Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust 2021 Edition |
| Async Runtime | Tokio |
| HTTP Client | Reqwest |
| Serialization | Serde, Serde JSON |
| CSV Parsing | CSV crate |
| Market Data | yahoo_finance_api |
| Parallelism | Rayon |
| Complex Math | num-complex |
| Quadrature | Pure Rust Gauss-Laguerre (2–128 nodes) |
| Vol Model | SABR (Hagan et al. 2002 analytic approx) |
| Time/Date | Chrono, Time |

## 🗂 Data Coverage

**Configurable Stocks (via `config/stocks.json`):**
- **Enabled by Default:** TSLA, AAPL, NVDA, MSFT
- **Easy to Add:** Any Yahoo Finance–supported symbol (SPY, QQQ, GOOGL, AMZN, META, etc.)
- Options chains work best for high-liquidity symbols

**Data Types:**
- Historical stock prices — 5+ years of daily closes (CSV)
- Options chains — real-time bid/ask per expiry (JSON)
- Volatility surfaces — IV extracted and analysed per strike/maturity

**Pipeline Integration:**
- All components automatically pick up enabled stocks from `config/stocks.json`
- No code changes needed to add or remove symbols

## 📚 Documentation

- **[README.md](README.md)** — This file: overview and quick start
- **[Getting Started Guide](docs/getting-started.md)** — Setup for personality-driven trading
- **[Options Strategies Guide](docs/strategies-guide.md)** — Multi-leg strategies, credit spreads, iron condors
- **[Advanced Features](docs/advanced-features.md)** — Detailed feature guides and examples
- **[Alpaca Integration](docs/alpaca-guide.md)** — Paper trading setup and API usage
- **[Backtesting Guide](docs/backtesting-guide.md)** — Strategy testing methodology
- **[Implementation Details](docs/implementation-summary.md)** — Technical documentation
- **Inline comments** — Throughout source code
- **Example programs** — Demonstrative usage in `examples/`

## 🎓 Educational Value

### Mathematical Concepts Demonstrated
- **Stochastic Calculus**: Heston model implementation
- **Numerical Methods**: Gauss-Laguerre quadrature, FFT, Newton-Raphson, Nelder-Mead
- **Financial Mathematics**: Options pricing, Greeks, volatility
- **Risk Management**: Portfolio analytics, hedging, Sharpe and Sortino ratios
- **QuantLib Cross-Validation**: Verified against QuantLib v1.41 analytical engine (`py/validate_pricing.py`)

### Programming Techniques Showcased
- **Rust Best Practices**: Zero-cost abstractions, ownership
- **Parallel Processing**: Rayon for multi-symbol analysis
- **API Integration**: REST clients and JSON handling
- **Error Handling**: Result types and graceful failures

### AI Development Insights
- **Conversational Coding**: How AI translates math to code
- **Iterative Refinement**: Building complex systems through dialog
- **Domain Translation**: Financial concepts → Rust implementation

## ✅ Testing

**Comprehensive Test Suite: 487+ tests, 100% passing**

### Test Coverage by Category

| Category | Tests | Description |
|----------|------:|-------------|
| **Models — Black-Scholes** | 21 | Pricing, put-call parity, dividends, numerical stability |
| **Models — Black-Scholes Pathological** | 12 | Edge cases, Hull textbook references, 8 absolute-value tests |
| **Models — Greeks** | 16 | All first-order sensitivities: Δ, Γ, ν, Θ, ρ |
| **Models — Heston Analytical** | 15 | GL P₁/P₂, Carr-Madan FFT, put-call parity, unified dispatch |
| **Models — Heston Stress & Pathological** | 24 | QE scheme, variance non-negativity, Feller violation, extreme params |
| **Models — American** | 7 | Binomial tree pricing, convergence, early exercise with dividends |
| **Models — Gauss-Laguerre** | 14 | Node/weight accuracy, convergence, exp-modified weights, edge cases |
| **Models — QuantLib Reference** | 10 | Cross-validated vs QuantLib v1.41 with tolerances as tight as 0.05 |
| **Models — Property-Based** | 13 | Proptest: delta bounds, gamma positivity, vega symmetry, monotonicity |
| **Models — Numerical Stability** | 8 | Convergence, precision, degenerate inputs |
| **Models — Vol Surface** | 5 | Arbitrage-free surface constraints |
| **Models — Portfolio Risk** | 5 | Delta-neutral portfolios, gamma scalping, vega/rho sign correctness |
| **Models — Edge Cases** | 10 | Pathological inputs, boundary conditions |
| **Backtesting — Engine** | 17 | Config, execution, stop-loss, take-profit, position limits, commissions |
| **Backtesting — Short Options** | 13 | SellCall, SellPut, straddles, IV-based sizing, early exit, mixed |
| **Backtesting — Trading Costs** | 14 | Round-trip costs, bid-ask spread, commissions, no-free-lunch invariants |
| **Backtesting — Liquidity** | 20 | Tier-based spread models, impact coefficients |
| **Backtesting — Slippage** | 13 | Panic widening, partial fills, vol-scaled fill rates |
| **Backtesting — Market Impact** | 9 | Full market impact model, crash vs calm, size monotonicity |
| **Backtesting — Edge Cases** | 7 | COVID vol explosion, regime change, naked call risk, iron condor Greeks |
| **Strategies — Core** | 18 | Strategy factory, signal generation, 6 strategy types |
| **Strategies — Stock Classifier** | 5 | Personality classification, confidence scoring |
| **Strategies — Vol Mean Reversion** | 16 | Vol mean reversion, z-score, edge thresholds |
| **Strategies — Personality Props** | 15 | Proptest classifier stability under noise |
| **Portfolio** | 14 | Position sizing, risk analytics, VaR, Greeks aggregation, attribution |
| **Calibration** | 2 | Nelder-Mead optimizer (Rosenbrock, sphere functions) |
| **Market Data** | 8 | CSV loader validation, date handling, missing file handling |
| **Concurrency** | 3 | Thread-safe pricing, deadlock prevention, parallel calibration |
| **Integration** | 17 | End-to-end pipeline, multi-model consistency, regime stress |
| **Performance** | 3 | BSM, Heston, Nelder-Mead speed benchmarks |
| **CDF Verification** | 1 | Normal CDF accuracy against 6 reference values |
| **Doc-tests** | 5 | SABR, GL, vol surface, Alpaca client examples |

### Running Tests
```bash
cargo test                                     # Run all tests
cargo test --lib                               # Library unit tests only
cargo test --test lib                          # Integration tests only
cargo test test_black_scholes                  # Black-Scholes tests
cargo test test_property_based                 # Property-based tests
cargo test test_numerical_stability            # Stability tests
cargo test test_thread_safety                  # Concurrency tests
cargo test test_personality_props              # Personality classifier
cargo test quantlib                            # QuantLib reference tests
cargo test -- --nocapture                      # See detailed output
```

See [tests/README.md](tests/README.md) for full test documentation.

### Benchmarks

[Criterion.rs](https://github.com/bheisler/criterion.rs) benchmarks (200 samples, 10s window) cross-validated against QuantLib v1.41. Heston params: S=K=100, T=1y, r=5%, v₀=0.04, κ=2, θ=0.04, σ=0.3, ρ=−0.7.

| Engine | Method | Price | Latency | Throughput |
|--------|--------|------:|--------:|-----------:|
| **DollarBill** | BSM call + full Greeks | 10.4506 | **79 ns** | 12.7M ops/s |
| **DollarBill** | Heston GL-32 (precomputed) | 10.3942 | **15 µs** | 66,700 ops/s |
| **DollarBill** | Heston GL-64 (precomputed) | 10.3942 | **33 µs** | 30,300 ops/s |
| **DollarBill** | Heston GL-128 (precomputed) | 10.3942 | **69 µs** | 14,500 ops/s |
| **QuantLib** | Heston GL (AnalyticHestonEngine) | 10.3942 | 39.25 µs | 25,480 ops/s |
| **DollarBill** | Heston Carr-Madan (adaptive Simpson) | 10.4506* | 474 µs | 2,040 ops/s |
| **DollarBill** | 11-strike sweep (GL-64) | — | **398 µs** | — |
| **DollarBill** | 11-strike sweep (Carr-Madan) | — | 5.43 ms | — |
| **QuantLib** | 11-strike sweep | — | 531 µs | — |

\* Carr-Madan uses the legacy characteristic function; GL uses the corrected Lord-Kahl CF.

**Key results:**
- GL-64 matches QuantLib to **6 significant figures** (10.394219 vs 10.394218)
- GL-32 is **31.6× faster** than Carr-Madan and already fully converged
- GL-64 11-strike sweep is **13.6× faster** than Carr-Madan sweep

```bash
cargo bench                                    # Run all benchmarks
cargo bench -- "Gauss-Laguerre"                # GL benchmarks only
python py/bench_quantlib_heston.py             # QuantLib comparison
```

<p align="center">
  <img src="images/heston_fft_violin.svg" alt="Heston Carr-Madan FFT — Criterion violin plot" width="700">
  <br>
  <em>Criterion violin plot: Heston Carr-Madan FFT latency distribution</em>
</p>

## 🎯 Use Cases

### Core Trading Applications (Implemented)
✅ **Options Pricing** — Black-Scholes, Heston, and SABR models for fair value calculation  
✅ **Greeks Analysis** — Delta, Gamma, Vega, Theta, Rho risk metrics  
✅ **Volatility Analysis** — IV extraction and volatility surface visualization  
✅ **Strategy Backtesting** — Historical P&L evaluation for long and short options strategies  
✅ **Multi-Leg Strategies** — Iron condors, credit spreads, straddles, strangles  
✅ **Short Options Trading** — Sell calls and puts for premium collection (covered calls, cash-secured puts)  
✅ **Mispricing Detection** — Model vs market comparison for edge identification  
✅ **Model Calibration** — Heston parameter fitting to market options data  

### Future Enhancements (Not Yet Implemented)
⚠️ **Multi-Asset Portfolio Construction** — Diversified portfolio optimization (planned)  
⚠️ **Event-Driven Trading** — Earnings/corporate action strategies (planned)  
⚠️ **Tail Risk Management** — VIX-based hedging (planned)  
⚠️ **Regime-Based Allocation** — Dynamic portfolio weighting (planned)  

## 🚦 Current Status

**Working Features:**
- ✅ Options pricing (Black-Scholes, Heston, SABR models)
- ✅ Greeks calculation (Delta, Gamma, Vega, Theta, Rho)
- ✅ Heston parameter calibration
- ✅ Multi-symbol signal generation
- ✅ Portfolio risk analytics
- ✅ Volatility surface analysis
- ✅ Market data integration (Yahoo Finance)
- ✅ Backtesting framework with P&L tracking
- ✅ Paper trading integration (Alpaca API)
- ✅ Short options (SellCall/SellPut) with multi-leg strategies
- ✅ JSON configuration system

**Build Status:**
- ✅ Compiles successfully (compiler warnings only)
- ✅ **487+ comprehensive tests** (100% passing)
- ✅ **QuantLib-validated**: Heston GL prices match QuantLib v1.41 to 6 significant figures

## 🔮 Potential Enhancements

**Realistic:**
- [ ] More sophisticated stock classification beyond 3 types
- [ ] Better Greeks hedging recommendations
- [ ] WebSocket real-time data feeds
- [ ] Real options order support via Alpaca (requires options-approved account)
- [ ] GARCH volatility forecasting
- [ ] Automatic position sizing with risk limits
- [ ] REST API for web integration
- [ ] Database persistence (PostgreSQL/SQLite)

**Ambitious:**
- [ ] Real-time portfolio optimization
- [ ] Advanced volatility forecasting models
- [ ] Regime-based allocation and factor models

**Already Implemented:**
- [x] ~~SQLite persistence~~ — lightweight `bot_state.json` crash recovery
- [x] ~~Graceful shutdown~~ — `Ctrl+C` cancels all open orders cleanly
- [x] ~~Live options data feed~~ — `live_pricer` polls Yahoo + Heston + edge signals
- [x] ~~Iron Condor / multi-leg strategies~~ — fully implemented with templates
- [x] ~~Short options~~ — SellCall/SellPut with Greeks and backtesting

## ⚠️ Important Disclaimers

1. **Educational Purpose**: This project was built as a learning exercise through AI pair programming
2. **No Financial Advice**: All analysis is for educational use only — nothing here is investment advice
3. **Options Risk**: Options trading involves substantial risk of loss
4. **Live Trading Risk**: The bot can connect to Alpaca live markets via `APCA_LIVE=1`. All production safety guards are implemented, but **use real money at your own risk**. Start with paper trading
5. **Mathematical Accuracy**: Core pricing models are cross-validated against QuantLib v1.41 (see `py/validate_pricing.py`). Backtest metrics (Sharpe, Sortino) use standard annualised formulas with configurable risk-free rate. The live pricer uses Yahoo Finance's unofficial options endpoint — production use requires a reliable data vendor
6. **Survivorship Bias**: Backtests use historical data from currently-active symbols. Stocks that were delisted or went bankrupt are absent from the dataset, meaning backtest results may overstate real-world performance

## 🤝 Contributing

This is a personal/educational project demonstrating:
- Advanced Rust programming patterns
- Financial mathematics implementation
- Real-time data processing
- Numerical optimization techniques
- **AI-assisted development** — the power of vibe coding with Claude Sonnet 4.6 and Grok

Feel free to use as reference or learning material.

### Development Philosophy

This project proves that complex quantitative finance software can be built entirely through **conversational AI pair programming**. Every line of code, from the Nelder-Mead optimizer to the Carr-Madan FFT implementation, emerged from natural language descriptions transformed into working Rust by AI coding assistants.

## 📄 License

MIT License — See [LICENSE](LICENSE) for details

## 👤 Author

Constantinos 'Costas' Papadopoulos — 720° Software  
(Built with AI assistance from Claude Sonnet 4.6 and Grok)

---

**Built with 🦀 Rust — Fast, Safe, Concurrent**
