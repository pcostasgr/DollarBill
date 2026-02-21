# DollarBill 🦀📈

![DollarBill](DollarBill.png)

**An educational options pricing and trading analysis tool built in Rust through AI pair programming.**

DollarBill demonstrates options mathematics, Greeks calculations, and basic trading strategies through a clean Rust implementation. Features Black-Scholes and Heston pricing models, volatility surface analysis, backtesting, and paper trading integration.

## 🤖 Built Entirely with AI

**This project was created through conversational AI development** - every line of code emerged from natural language descriptions with **Claude Sonnet 4.5** and **Grok**. From the Heston FFT implementation to the Nelder-Mead optimizer, it showcases how AI can build sophisticated mathematical software through "vibe coding."

No traditional programming sessions. Just prompts, iterations, and Rust. 🚀

## 🎯 What DollarBill Actually Is

### ✅ **Real Capabilities**
- **Options Pricing**: Black-Scholes-Merton and Heston stochastic volatility models
- **Greeks Calculation**: Delta, Gamma, Vega, Theta, Rho for risk analysis
- **Model Calibration**: Heston parameter fitting using custom Nelder-Mead optimizer
- **Volatility Analysis**: IV extraction, volatility surfaces, and smile analysis
- **Paper Trading**: Live integration with Alpaca API for risk-free testing
- **Backtesting**: Historical strategy evaluation with P&L tracking
- **Stock Classification**: Basic personality-driven strategy selection (3 types)
- **Short Options**: SellCall and SellPut support for premium collection strategies
- **Multi-Leg Strategies**: Iron condors, credit spreads, straddles, strangles with customizable templates
- **Strategy Templates**: Configurable strategy builders for quick backtesting
- **Portfolio Management**: Position sizing, risk analytics, multi-strategy allocation, performance attribution 🆕 NEW

### ❌ **What It's NOT**
- Production trading system
- Institutional-grade platform  
- Machine learning enhanced (despite config files suggesting it)
- Competitor to professional platforms
- Enterprise solution

### 🎓 **Perfect For**
- Learning options pricing mathematics
- Understanding Rust in quantitative finance
- Experimenting with basic trading strategies
- Educational backtesting and paper trading
- Seeing AI-assisted development in action

## 🚀 Quick Start

### Prerequisites
```bash
# Rust (required)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python (optional, for data fetching)
pip install pandas yfinance
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
    {
      "symbol": "TSLA",
      "market": "US", 
      "sector": "Automotive",
      "enabled": true
    },
    {
      "symbol": "AAPL",
      "market": "US",
      "sector": "Technology", 
      "enabled": true
    }
  ]
}
```

**2. Fetch Market Data**:
```bash
# Get historical stock data
python py/fetch_multi_stocks.py

# Get options chains  
python py/fetch_multi_options.py
```

**3. Run Analysis**:
```bash
# Generate trading signals with Greeks
cargo run --example multi_symbol_signals

# Analyze stock personalities  
cargo run --example enhanced_personality_analysis

# Portfolio management demonstration
cargo run --example portfolio_management

# Backtest long options strategies
cargo run --example backtest_strategy

# Backtest short options (covered calls, cash-secured puts)
cargo run --example backtest_short_options

# Multi-leg strategies - Iron condor (neutral income strategy)
cargo run --example iron_condor

# Credit spreads (bull put spread, bear call spread)
cargo run --example credit_spreads

# Strategy templates (customizable parameters)
cargo run --example strategy_templates

# Portfolio management (position sizing, risk analytics, allocation)
cargo run --example portfolio_management

# Paper trade (requires Alpaca API keys)
cargo run --example personality_based_bot
```

## 📊 Example Output

### Options Pricing with Greeks
```
Symbol Type   Strike   Market   Model    Edge %   Delta    Gamma    Vega     Theta
TSLA   Call   $440.00  $12.75   $15.20   19.2%   0.625   0.0035   85.20    -12.50
AAPL   Put    $270.00  $8.35    $10.10   21.0%  -0.350   0.0042   45.30    -8.75
```

### Volatility Smile Analysis  
```
TSLA Volatility Smile:
Strike     IV %       Volume
430.00     41.8%      3200
440.00     40.5%      4100  ← ATM  
450.00     41.2%      2800

ATM IV: 40.5% | Put Skew: 1.6% premium
```

### Stock Personality Classification
```
🧠 TSLA Classification:
   Personality: VolatileBreaker (confidence: 30%)
   Volatility: 91.7% percentile | Trend: 45.2% | Reversion: 62.1%
   Best strategies: ["Iron Butterfly", "Short Straddles"]
```

## 🔧 Architecture

### Core Models
- **Black-Scholes-Merton**: Analytical European pricing with dividends
- **Heston**: Carr-Madan FFT method (no Monte Carlo)
- **Greeks**: All first-order sensitivities
- **Implied Volatility**: Newton-Raphson solver

### Data Pipeline  
- **Market Data**: Yahoo Finance API integration
- **Storage**: CSV (historical) + JSON (options chains)
- **Configuration**: Central JSON-based stock management

### Trading Features
- **Strategy Classification**: 3 basic stock personality types
- **Signal Generation**: Model vs market price comparison  
- **Risk Management**: Portfolio Greeks aggregation
- **Paper Trading**: Alpaca API integration with position tracking

## 📂 Project Structure

```
DollarBill/
├── config/
│   └── stocks.json              # Stock configuration
├── src/
│   ├── models/                  # Pricing models (BS, Heston)
│   ├── calibration/             # Parameter fitting
│   ├── market_data/             # Data loading
│   ├── analysis/                # Stock classification
│   ├── backtesting/             # Strategy testing
│   ├── alpaca/                  # Paper trading
│   └── utils/                   # Utilities
├── examples/
│   ├── multi_symbol_signals.rs  # Main analysis
│   ├── enhanced_personality_analysis.rs
│   ├── backtest_strategy.rs
│   ├── personality_based_bot.rs # Paper trading bot
│   └── ...                      # More examples
├── py/                          # Python data fetchers
├── scripts/                     # Automation scripts  
└── data/                        # Market data storage
```

## 🎓 Educational Value

### Mathematical Concepts Demonstrated
- **Stochastic Calculus**: Heston model implementation
- **Numerical Methods**: FFT, Newton-Raphson, Nelder-Mead
- **Financial Mathematics**: Options pricing, Greeks, volatility
- **Risk Management**: Portfolio analytics and hedging

### Programming Techniques Showcased
- **Rust Best Practices**: Zero-cost abstractions, ownership
- **Parallel Processing**: Rayon for multi-symbol analysis  
- **API Integration**: REST clients and JSON handling
- **Error Handling**: Result types and graceful failures

### AI Development Insights
- **Conversational Coding**: How AI translates math to code
- **Iterative Refinement**: Building complex systems through dialog
- **Domain Translation**: Financial concepts → Rust implementation

## 📈 Performance Notes

- **Heston Calibration**: ~2-3 seconds per symbol
- **Multi-symbol Analysis**: Parallel processing with Rayon
- **Memory Usage**: Efficient with zero-copy parsing
- **Build Time**: Use `--release` for mathematical optimizations

## ✅ Testing

**Comprehensive Test Suite: 254 tests, 100% passing**

### Test Coverage
- **Unit Tests (26)**: Core library functionality in `src/`
- **Integration Tests (118)**: Comprehensive test suite in `tests/`
  - Black-Scholes Pricing: 15 tests
  - Greeks Calculations: 19 tests
  - Heston Model: 22 tests
  - Property-Based Tests: 7 tests (mathematical invariants)
  - Numerical Stability: 8 tests (convergence & precision)
  - Edge Cases: Multiple tests (boundary conditions)
  - Nelder-Mead Optimization: 14 tests
  - Backtest Engine: 17 tests
  - Short Options: 13 tests (premium collection strategies)
  - Market Data Loading: 8 tests
  - Volatility Mean Reversion Strategy: 17 tests
  - Thread Safety: 3 tests (concurrent calculations)
  - Performance Benchmarks: 3 tests (speed validation)
- **Doc Tests (2)**: API documentation examples

### Running Tests
```bash
# Run all tests
cargo test

# Run specific test categories
cargo test --lib                    # Library tests only
cargo test test_black_scholes       # Black-Scholes tests
cargo test test_property_based      # Property-based tests
cargo test test_numerical_stability # Stability tests
cargo test test_thread_safety       # Concurrency tests

# See detailed output
cargo test -- --nocapture
```

See [tests/README.md](tests/README.md) for detailed test documentation.

## 🔮 Potential Improvements

**Realistic Enhancements:**
- [ ] More sophisticated stock classification (currently very basic)
- [ ] Additional options strategies beyond current types
- [ ] Better Greeks hedging recommendations  
- [ ] WebSocket real-time data feeds
- [ ] SQLite persistence for historical analysis

**Ambitious Goals:**
- [ ] Actual machine learning integration (not just config files)
- [ ] Real-time portfolio optimization
- [ ] Advanced volatility forecasting models
- [ ] REST API for web integration

## ⚠️ Important Disclaimers

1. **Educational Purpose**: This is a learning project, not production software
2. **No Financial Advice**: All analysis is for educational use only
3. **Options Risk**: Options trading involves substantial risk of loss
4. **Paper Trading Only**: Live trading integration not recommended
5. **Mathematical Accuracy**: Models are simplified for educational clarity

## 🤝 Contributing

This project demonstrates AI-assisted development in quantitative finance. Feel free to:
- Use as reference for Rust financial programming
- Extend with additional pricing models or strategies  
- Improve the mathematical implementations
- Add proper unit tests and documentation

### Development Philosophy

DollarBill proves that complex mathematical software can emerge from conversational AI programming. Every algorithm, from FFT pricing to optimization routines, was developed through natural language descriptions transformed into working Rust code.

## 📄 License

MIT License - See [LICENSE](LICENSE) for details

## 👤 Author

Constantinos 'Costas' Papadopoulos - 720° Software  
Built through AI pair programming with Claude Sonnet 4.5

---

**Educational Rust Financial Programming - Powered by AI** 🦀

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

### Configure Stocks

Edit `config/stocks.json` to select symbols for analysis:

```json
{
  "stocks": [
    {
      "symbol": "TSLA",
      "market": "US",
      "sector": "Automotive",
      "enabled": true
    },
    {
      "symbol": "AAPL",
      "market": "US",
      "sector": "Technology",
      "enabled": true
    },
    {
      "symbol": "NVDA",
      "market": "US",
      "sector": "Technology",
      "enabled": true
    }
  ]
}
```

- Set `"enabled": true` to include a stock in analysis
- Add/remove stocks as needed
- The pipeline automatically processes enabled stocks

### Fetch Market Data

```bash
# Fetch historical stock data for enabled stocks
python py/fetch_multi_stocks.py

# Fetch live options chains for enabled stocks
python py/fetch_multi_options.py
```

### Run Analysis

```bash
# Analyze stock personalities
cargo run --release --example enhanced_personality_analysis

# Generate trade signals with Greeks and portfolio risk
cargo run --release --example multi_symbol_signals

# Analyze volatility surfaces
cargo run --release --example vol_surface_analysis

# Backtest strategies on historical data
cargo run --release --example backtest_strategy

# Backtest with Heston model
cargo run --release --example backtest_heston

# Create 3D volatility visualizations (requires Python)
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

# Complete pipeline: Data fetch -> Calibration -> Signals -> Paper trading (fast execution)
.\scripts\run_release_pipeline.ps1

# Complete pipeline: Data fetch -> Calibration -> Signals -> Paper trading (with compilation)
.\scripts\run_full_pipeline.ps1

# Personality-driven pipeline: Stock analysis -> Strategy matching -> Optimized trading
cargo run --example personality_driven_pipeline

# Personality-based live trading bot: Uses trained models for real-time strategy selection
cargo run --example personality_based_bot -- --dry-run  # Test without trading
cargo run --example personality_based_bot               # Single live iteration
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
    Portfolio benefits if implied volatility rises
  ⚠ High theta decay: $-85.30/day time decay
    Position loses value each day - consider shorter holding period
```

### Volatility Smile

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

📊 ATM Volatility Analysis:
  ATM Call IV:  40.5%
  ATM Put IV:   42.1%
  ⚠ Put skew detected: Puts trading at 1.6% premium
    Market pricing in downside protection
```

### Backtest Results

```
================================================================================
BACKTEST RESULTS - TSLA
================================================================================
Period: 2025-01-03 to 2026-01-02
Initial Capital: $100000.00
Final Capital: $146402.25

📊 PERFORMANCE METRICS
--------------------------------------------------------------------------------
Total P&L:        $    46406.25  ( 46.41%)
Sharpe Ratio:             1.22
Max Drawdown:     $        0.00  (  0.00%)

📈 TRADE STATISTICS
--------------------------------------------------------------------------------
Total Trades:                2
Winning Trades:              2  (100.00%)
Losing Trades:               0

Average Win:      $    23203.13
Average Loss:     $        0.00
Largest Win:      $    25390.73
Largest Loss:     $        0.00
Profit Factor:             inf

Avg Days Held:             1.0
Total Commissions:$        4.00
================================================================================
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
│   │   └── heston_analytical.rs        # Carr-Madan FFT pricing
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
│   │   ├── vol_mean_reversion.rs       # Vol trading strategy
│   │   └── mod.rs                      # Strategy trait
   ├── analysis/                       # Stock analysis system
   │   ├── stock_classifier.rs         # Personality classification
   │   ├── advanced_classifier.rs      # Multi-dimensional feature analysis (rarely used)
   │   ├── performance_matrix.rs       # Strategy performance tracking
   │   └── mod.rs                      # Analysis exports
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
│   ├── vol_surface_analysis.rs         # Volatility surface extraction
│   ├── backtest_strategy.rs            # Black-Scholes strategy backtesting
│   ├── backtest_heston.rs              # Heston model backtesting
│   ├── calibrate_live_options.rs       # Heston calibration demo
│   ├── trade_signals.rs                # Basic signal generation
│   ├── alpaca_demo.rs                  # Alpaca API demo
│   ├── paper_trading.rs                # Paper trading with momentum
│   ├── trading_bot.rs                  # Continuous trading bot
│   ├── test_keys.rs                    # Alpaca API key testing
│   ├── personality_driven_pipeline.rs  # Personality-optimized trading
│   ├── personality_based_bot.rs        # Personality-based live trading
│   └── enhanced_personality_analysis.rs # Multi-dimensional personality analysis
├── py/
│   ├── fetch_multi_stocks.py           # Stock data fetcher (config-driven)
│   ├── fetch_multi_options.py          # Options chain fetcher (config-driven)
│   ├── plot_vol_surface.py             # 3D volatility visualization
│   ├── fetch_options.py                # Single symbol options fetcher
│   ├── get_tesla_quotes.py             # Tesla quotes fetcher
│   └── get_tesla_stock_csv.py          # Tesla CSV downloader
├── scripts/
│   ├── setup_python.bat                # Batch: Python environment setup
│   ├── test_python.bat                 # Batch: Python environment testing
│   ├── collect_data_fixed.bat          # Batch: Data collection pipeline
│   ├── run_enhanced_personality.bat    # Batch: Personality analysis
│   ├── run_multi_signals.ps1           # PowerShell: Run signals
│   ├── run_vol_surface.ps1             # PowerShell: Vol pipeline
│   ├── run_signals.ps1                 # PowerShell: Single symbol signals
│   ├── run_backtest.ps1                # PowerShell: Black-Scholes backtesting
│   ├── run_heston_backtest.ps1         # PowerShell: Heston backtesting
│   ├── run_paper_trading.ps1           # PowerShell: Paper trading
│   ├── run_full_pipeline.ps1           # PowerShell: Complete pipeline
│   ├── run_multi_signals.bat           # Batch: Run signals
│   ├── run_signals.bat                 # Batch: Single symbol signals
│   ├── run_paper_trading.sh            # Shell: Paper trading
│   └── run_signals.sh                  # Shell: Single symbol signals
├── docs/
│   ├── advanced-features.md            # Advanced features guide
│   ├── alpaca-guide.md                 # Alpaca API integration
│   ├── backtesting-guide.md            # Backtesting methodology
│   ├── personality-guide-experimental.md # Personality system (experimental)
│   ├── implementation-summary.md       # Technical implementation details
│   └── trading-guide.md                # Trading strategies guide
├── images/                             # Generated charts and visualizations
├── data/                               # Market data storage
└── Cargo.toml                          # Rust dependencies
```

## 🔧 Technical Details

### Pricing Models

**Black-Scholes-Merton:**
- Analytical solution for European options
- Dividend yield support (q parameter)
- All Greeks: Δ, Γ, ν, Θ, ρ
- Zero-expiry handling

**Heston Stochastic Volatility:**
- Carr-Madan FFT method (analytical, no Monte Carlo)
- Complex characteristic function
- Adaptive integration
- ITM/OTM handling for numerical stability

### Optimization

**Nelder-Mead Simplex:**
- Pure Rust implementation
- Configurable reflection/expansion/contraction coefficients
- Convergence tolerance and max iterations
- Parameter bounds enforcement

### Greeks Calculation

```rust
Greeks {
    price: f64,   // Option price
    delta: f64,   // ∂V/∂S - directional exposure
    gamma: f64,   // ∂²V/∂S² - convexity
    theta: f64,   // ∂V/∂t - time decay
    vega: f64,    // ∂V/∂σ - vol sensitivity
    rho: f64,     // ∂V/∂r - rate sensitivity
}
```

### Performance

- **Parallel calibration** - Rayon for multi-symbol processing
- **Zero-copy parsing** - CSV crate optimizations
- **Analytical pricing** - No Monte Carlo overhead
- **Typical runtime** - 500-1000ms for full multi-symbol analysis
- **Release builds** - LLVM optimizations enabled

## 🎓 Understanding the Output

### Trade Signals

- **Edge %** - Model price premium over market (buy if > 5%)
- **Delta** - Position direction (+call/-put exposure)
- **Gamma** - Price acceleration (convexity)
- **Vega** - Profit from volatility increase
- **Theta** - Daily time decay (always negative for longs)

### Portfolio Risk

- **Delta < ±5** - Direction-neutral (market-neutral strategy)
- **High Vega** - Profits from vol expansion (long gamma/vega)
- **Negative Theta** - Loses value daily (needs quick moves)

### Volatility Patterns

- **Flat Smile** - Market is calm, no fear/greed
- **Put Skew** - Higher IV on puts = crash protection
- **Call Skew** - Higher IV on calls = speculation/FOMO
- **Smile** - Both wings high = uncertainty

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
| Time/Date | Chrono, Time |

## � Data Coverage

**Configurable Stocks (via config/stocks.json):**
- **Enabled by Default:** TSLA, AAPL, NVDA, MSFT (US Technology)
- **Available for Enable:** SAP.DE (EU Technology example)
- **Easy to Add:** Any Yahoo Finance supported symbol

**Data Types Available:**
- **Historical Stock Data:** 5+ years of daily prices (CSV format)
- **Live Options Chains:** Real-time bid/ask for all strikes (JSON format)
- **Volatility Surfaces:** Implied volatility extraction and analysis

**Pipeline Integration:**
- All components automatically use enabled stocks from config
- No code changes needed to add/remove symbols
- Consistent symbol handling across Python fetchers and Rust examples

## �📚 Documentation

- **[Getting Started Guide](docs/getting-started.md)** - Quick setup for personality trading ⭐ NEW
- **README.md** (this file) - Overview and quick start
- **[Personality Guide](docs/personality-guide.md)** - Personality-driven trading system ⭐ NEW
- **[Enhanced Personality Implementation](docs/enhanced-personality-implementation.md)** - Advanced multi-dimensional personality system ⭐ ENHANCED
- **[Options Strategies Guide](docs/strategies-guide.md)** - Multi-leg strategies, credit spreads, iron condors 🆕 NEW
- **[Advanced Features](docs/advanced-features.md)** - Detailed feature guides and examples
- **[Alpaca Integration](docs/alpaca-guide.md)** - Paper trading setup and API usage
- **[Backtesting Guide](docs/backtesting-guide.md)** - Strategy testing methodology
- **[Trading Strategies](docs/trading-guide.md)** - Live trading examples and workflows
- **[Implementation Details](docs/implementation-summary.md)** - Technical documentation
- **[Parameter Atlas](docs/parameter_atlas.md)** - Complete configuration reference
- **Inline comments** - Throughout source code
- **Example programs** - Demonstrative usage in `examples/`

## 🎯 Use Cases

### Core Trading Applications (Implemented)
✅ **Options Pricing** - Black-Scholes and Heston models for fair value calculation
✅ **Greeks Analysis** - Delta, Gamma, Vega, Theta, Rho risk metrics  
✅ **Volatility Analysis** - IV extraction and volatility surface visualization
✅ **Strategy Backtesting** - Historical P&L evaluation for long and short options strategies
✅ **Multi-Leg Strategies** - Iron condors, credit spreads, straddles, strangles 🆕 NEW
✅ **Short Options Trading** - Sell calls and puts for premium collection 🆕 NEW
✅ **Mispricing Detection** - Model vs market comparison for edge identification
✅ **Model Calibration** - Heston parameter fitting to market options data
✅ **Short Options Trading** - Sell calls and puts for premium collection (covered calls, cash-secured puts)

### Future Enhancements (Not Yet Implemented)
⚠️ **Multi-Asset Portfolio Construction** - Diversified portfolio optimization (planned)
⚠️ **Sector Rotation** - Cyclical opportunity identification (planned)
⚠️ **Cross-Asset Arbitrage** - Exploit discrepancies between correlated assets (planned)
⚠️ **Currency Hedging** - International exposure management (planned)
⚠️ **Event-Driven Trading** - Earnings/corporate action strategies (planned)
⚠️ **Tail Risk Management** - VIX-based hedging (planned)
⚠️ **Correlation Trading** - Mean reversion strategies (planned)
⚠️ **Regime-Based Allocation** - Dynamic portfolio weighting (planned)  

## 🚦 Current Status

**Working Features:**
- ✅ Options pricing (Black-Scholes and Heston models)
- ✅ Greeks calculation (Delta, Gamma, Vega, Theta, Rho)
- ✅ Heston parameter calibration
- ✅ Multi-symbol signal generation 
- ✅ Portfolio risk analytics
- ✅ Volatility surface analysis
- ✅ Market data integration (Yahoo Finance)
- ✅ Backtesting framework with P&L tracking
- ✅ Paper trading integration (Alpaca API)
- ✅ Basic stock personality classification (3 types)
- ✅ JSON configuration system

**Build Status:**
- ✅ Compiles successfully (with warnings)
- ✅ Optimized `--release` builds available  
- ✅ **146 comprehensive tests** (100% passing)
  - 118 integration tests
  - 26 unit tests
  - 2 doc tests
- ✅ **Mathematical accuracy verified** across all core models

## 🔮 Potential Enhancements

- [ ] Real-time Greeks updates via WebSocket
- [ ] Position optimizer with Kelly criterion
- [ ] Additional strategies (Iron Condor, Calendar spreads)
- [ ] Greeks hedging calculator
- [ ] GARCH volatility forecasting
- [ ] Automatic position sizing with risk limits
- [ ] REST API for web integration
- [ ] Database persistence (PostgreSQL/SQLite)
- [ ] Unit and integration tests

## 📊 Data Coverage

**Supported Symbols:**
- Any stock or ETF available on Yahoo Finance can be added to `config/stocks.json`
- Examples included: TSLA, AAPL, NVDA, MSFT, GOOGL, AMZN, META
- Options chains work best for high-liquidity symbols (SPY, QQQ, etc.)

**Data Types:**
- Historical stock prices (CSV format)
- Options chains (JSON format)
- Volatility surfaces extracted from options data

## 🤝 Contributing

This is a personal/educational project demonstrating:
- Advanced Rust programming patterns
- Financial mathematics implementation
- Real-time data processing
- Numerical optimization techniques
- **AI-assisted development** - The power of vibe coding with Claude Sonnet 4.5 and Grok

Feel free to use as reference or learning material.

### Development Philosophy

This project proves that complex quantitative finance software can be built entirely through **conversational AI pair programming**. Every line of code, from the Nelder-Mead optimizer to the Carr-Madan FFT implementation, emerged from natural language descriptions transformed into working Rust by AI coding assistants. It's a testament to how AI is democratizing access to sophisticated software engineering.

## ⚠️ Disclaimer

This software is for **educational and research purposes only**. It is not financial advice. Options trading involves substantial risk of loss. Always conduct your own research and consult with licensed financial professionals before trading.

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details

## 👤 Author

Constantinos 'Costas' Papadopoulos - 720° Software

(Built with AI assistance from Claude Sonnet 4.5 and Grok)

---

**Built with 🦀 Rust - Fast, Safe, Concurrent**
