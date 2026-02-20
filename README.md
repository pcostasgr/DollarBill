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

# Backtest strategies
cargo run --example backtest_strategy

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

## 🔮 Potential Improvements

**Realistic Enhancements:**
- [ ] More sophisticated stock classification (currently basic)
- [ ] Additional strategy types beyond the current 6
- [ ] Better Greeks hedging recommendations  
- [ ] WebSocket real-time data feeds
- [ ] SQLite persistence for historical analysis
- [ ] Unit tests for mathematical functions

**Ambitious Goals:**
- [ ] Actual machine learning integration (not just config files)
- [ ] Real-time portfolio optimization
- [ ] Advanced volatility forecasting models

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

# Python (optional, for data fetching, 3D visualization, and ML integration)
pip install pandas plotly yfinance scikit-learn tensorflow
```

### Installation

```bash
git clone https://github.com/yourusername/DollarBill.git
cd DollarBill
cargo build --release
```

### Configure Diversified Portfolio

Edit `config/stocks.json` to build a comprehensive options portfolio across sectors:

```json
{
  "stocks": [
    {
      "symbol": "SPY",
      "market": "US",
      "sector": "ETF",
      "enabled": true,
      "weight": 0.15,
      "notes": "S&P 500 ETF - Core holding, highest liquidity"
    },
    {
      "symbol": "QQQ",
      "market": "US",
      "sector": "ETF",
      "enabled": true,
      "weight": 0.12,
      "notes": "Tech ETF - Growth exposure, momentum strategies"
    },
    {
      "symbol": "TSLA",
      "market": "US",
      "sector": "Automotive",
      "enabled": true,
      "weight": 0.08,
      "notes": "High volatility leader - Premium selling opportunities"
    },
    {
      "symbol": "AAPL",
      "market": "US",
      "sector": "Technology",
      "enabled": true,
      "weight": 0.10,
      "notes": "Large-cap stability - Covered calls, defensive"
    },
    {
      "symbol": "AMD",
      "market": "US",
      "sector": "Technology",
      "enabled": true,
      "weight": 0.07,
      "notes": "High-beta semiconductor - Trend following"
    },
    {
      "symbol": "JPM",
      "market": "US",
      "sector": "Financials",
      "enabled": true,
      "weight": 0.08,
      "notes": "Banking sector - Rate sensitivity, earnings plays"
    },
    {
      "symbol": "JNJ",
      "market": "US",
      "sector": "Healthcare",
      "enabled": true,
      "weight": 0.06,
      "notes": "Defensive healthcare - Low volatility, steady income"
    },
    {
      "symbol": "GLD",
      "market": "US",
      "sector": "Commodities",
      "enabled": true,
      "weight": 0.05,
      "notes": "Gold ETF - Inflation hedge, portfolio diversifier"
    }
  ],
  "portfolio_settings": {
    "max_sector_concentration": 0.40,
    "min_options_volume": 1000,
    "target_portfolio_beta": 1.0,
    "max_single_position": 0.15,
    "correlation_limit": 0.70
  }
}
```

**Strategic Portfolio Allocation:**
- **40% Core Markets** (SPY, QQQ) - Liquidity and market exposure
- **30% Growth Tech** (AAPL, TSLA, AMD) - Momentum and volatility capture  
- **20% Diversification** (JPM, JNJ) - Sector balance and defense
- **10% Alternatives** (GLD) - Hedging and uncorrelated returns

- Set `"enabled": true` to include a stock in the pipeline
- Configure `"weight"` for target portfolio allocation  
- Add/remove stocks based on market conditions
- The pipeline automatically uses enabled stocks for analysis
- Portfolio rebalancing alerts when weights drift beyond thresholds

### 🚀 Quick Expansion Guide

**Next Priority Adds:**
```bash
# Essential ETFs for any serious options portfolio
SPY, QQQ, IWM  # The "big three" for liquidity
GLD, TLT       # Diversification and hedging

# High-beta momentum plays  
AMD, COIN      # Semiconductor and crypto exposure
PLTR, ARKK     # Meme stocks and innovation
```

**Sector Diversification:**
```bash
JPM, JNJ, XOM  # Finance, Healthcare, Energy
DIS, WMT, UNH  # Entertainment, Retail, Healthcare
```

### Fetch Market Data

**Option 1: Automated Pipeline (Recommended)**
```bash
# Complete data collection and analysis pipeline
cmd /c ".\scripts\collect_data_fixed.bat"

# Test Python environment first (if issues)
cmd /c ".\scripts\test_python.bat"

# Setup Python environment from scratch (if needed)
cmd /c ".\scripts\setup_python.bat"
```

**Option 2: Manual Python Scripts**
The Python scripts automatically read enabled stocks from `config/stocks.json`:

```bash
# Fetch historical stock data for enabled stocks
python py/fetch_multi_stocks.py

# Fetch live options chains for enabled stocks
python py/fetch_multi_options.py
```

**✅ Python Environment Fixed**: All environment issues resolved with automated setup scripts.

### Run Analysis

```bash
# Test advanced multi-dimensional personality analysis ⭐ ENHANCED
cargo run --release --example enhanced_personality_analysis

# Test strategy deployment patterns (manual, config-driven, ensemble)
cargo run --release --example strategy_deployment

# Generate trade signals with Greeks and portfolio risk
cargo run --release --example multi_symbol_signals

# Analyze volatility surfaces
cargo run --release --example vol_surface_analysis

# Backtest strategies on historical data
cargo run --release --example backtest_strategy

# Advanced Heston stochastic volatility backtesting ⭐ NEW
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
# Python Environment Management ⭐ NEW
cmd /c ".\scripts\setup_python.bat"        # Setup Python environment from scratch
cmd /c ".\scripts\test_python.bat"         # Test and diagnose Python issues
cmd /c ".\scripts\collect_data_fixed.bat"  # Complete data collection pipeline

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

### Enhanced Personality Analysis Output

```
🚀 DollarBill Enhanced Stock Personality Analysis
===============================================

🧠 Advanced Classification for TSLA:
   📊 Personality: VolatileBreaker (confidence: 30.0%)
   📈 Vol Percentile: 91.7% | Trend: 45.2% | Reversion: 62.1%
   🎯 Market Regime: HighVol | Beta: 1.23 | Sector: Automotive
   🎯 Best strategies: ["Iron Butterfly", "Volatility Harvesting", "Short Straddles"]
   ❌ Avoid strategies: ["Directional Bets", "Long Options", "Momentum Strategies"]

🧠 Advanced Classification for PLTR:
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
│   ├── stocks.json                    # Central stock configuration
│   ├── personality_config.json        # Personality analysis settings ⭐ NEW
│   ├── ml_config.json                 # ML model configuration ⭐ NEW
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
   ├── analysis/                       # Advanced analytics system ⭐ ENHANCED
   │   ├── stock_classifier.rs         # Enhanced personality analysis with legacy compatibility
   │   ├── advanced_classifier.rs      # Multi-dimensional feature analysis engine ⭐ NEW
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
│   ├── backtest_heston.rs              # Heston stochastic volatility backtesting ⭐ NEW
│   ├── calibrate_live_options.rs       # Heston calibration demo
│   ├── trade_signals.rs                # Basic signal generation
│   ├── alpaca_demo.rs                  # Alpaca API demo
│   ├── paper_trading.rs                # Paper trading with momentum
│   ├── trading_bot.rs                  # Continuous trading bot
│   ├── test_keys.rs                    # Alpaca API key testing
│   ├── personality_driven_pipeline.rs  # Personality-optimized trading ⭐ NEW
│   ├── personality_based_bot.rs        # Personality-based live trading ⭐ NEW
│   ├── enhanced_personality_analysis.rs # Advanced multi-dimensional personality analysis ⭐ ENHANCED
│   ├── ml_enhanced_signals.rs          # ML-enhanced signal generation ⭐ NEW
│   └── cali_enhanced_signals.rs        # California-specific signals ⭐ NEW
├── py/
│   ├── fetch_multi_stocks.py           # Stock data fetcher (config-driven)
│   ├── fetch_multi_options.py          # Options chain fetcher (config-driven)
│   ├── plot_vol_surface.py             # 3D volatility visualization
│   ├── fetch_options.py                # Single symbol options fetcher
│   ├── get_tesla_quotes.py             # Tesla quotes fetcher
│   └── get_tesla_stock_csv.py          # Tesla CSV downloader
├── scripts/
│   ├── setup_python.bat                # Batch: Python environment setup ⭐ NEW
│   ├── test_python.bat                 # Batch: Python environment testing ⭐ NEW
│   ├── collect_data_fixed.bat          # Batch: Complete data collection pipeline ⭐ NEW
│   ├── run_enhanced_personality.bat    # Batch: Enhanced personality analysis ⭐ NEW
│   ├── run_multi_signals.ps1           # PowerShell: Run signals
│   ├── run_vol_surface.ps1             # PowerShell: Vol pipeline
│   ├── run_signals.ps1                 # PowerShell: Single symbol signals
│   ├── run_backtest.ps1                # PowerShell: Black-Scholes backtesting
│   ├── run_heston_backtest.ps1         # PowerShell: Heston backtesting ⭐ NEW
│   ├── run_paper_trading.ps1           # PowerShell: Paper trading
│   ├── run_full_pipeline.ps1           # PowerShell: Complete pipeline ⭐ NEW
│   ├── run_multi_signals.bat           # Batch: Run signals
│   ├── run_signals.bat                 # Batch: Single symbol signals
│   ├── run_paper_trading.sh            # Shell: Paper trading
│   └── run_signals.sh                  # Shell: Single symbol signals
├── docs/
│   ├── advanced-features.md            # Advanced features guide
│   ├── alpaca-guide.md                 # Alpaca API integration
│   ├── backtesting-guide.md            # Backtesting methodology
│   ├── enhanced-personality-implementation.md # Enhanced personality system implementation ⭐ NEW
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
- **[Advanced Features](docs/advanced-features.md)** - Detailed feature guides and examples
- **[Alpaca Integration](docs/alpaca-guide.md)** - Paper trading setup and API usage
- **[Backtesting Guide](docs/backtesting-guide.md)** - Strategy testing methodology
- **[Trading Strategies](docs/trading-guide.md)** - Live trading examples and workflows
- **[Implementation Details](docs/implementation-summary.md)** - Technical documentation
- **[Parameter Atlas](docs/parameter_atlas.md)** - Complete configuration reference
- **Inline comments** - Throughout source code
- **Example programs** - Demonstrative usage in `examples/`

## 🎯 Use Cases

### Core Trading Applications
✅ **Options Trading** - Identify mispriced options across diverse asset classes  
✅ **Risk Management** - Monitor portfolio Greeks with sector diversification  
✅ **Volatility Analysis** - Study IV surfaces and skew patterns across markets  
✅ **Strategy Backtesting** - Evaluate historical performance with realistic P&L  
✅ **Market Making** - Fair value pricing with correlation adjustments  
✅ **Research** - Model calibration and cross-asset comparison  

### Advanced Portfolio Applications ⭐ NEW
✅ **Multi-Asset Portfolio Construction** - Build diversified options portfolios across 8+ sectors  
✅ **Sector Rotation Strategies** - Identify cyclical opportunities and defensive positioning  
✅ **Cross-Asset Volatility Arbitrage** - Exploit IV discrepancies between correlated assets  
✅ **Currency Hedging** - Manage international exposure with FX-sensitive positions  
✅ **Event-Driven Trading** - Capitalize on earnings, splits, and corporate actions  
✅ **Tail Risk Management** - VIX-based hedging strategies for black swan protection  
✅ **Correlation Trading** - Exploit mean reversion in asset correlations  
✅ **Regime-Based Allocation** - Adapt portfolio weights to volatility regimes  

## 🚦 Current Status

**Production Ready:**
- ✅ Options pricing (BS-M and Heston)
- ✅ Full Greeks calculation
- ✅ Heston calibration
- ✅ Multi-symbol signal generation
- ✅ Portfolio risk analytics
- ✅ Volatility surface extraction
- ✅ Real-time market data integration
- ✅ **Backtesting framework** - Historical strategy performance analysis
- ✅ **JSON Configuration System** - Centralized stock management
- ✅ **Paper Trading Integration** - Alpaca API client
- ✅ **Parallel Processing** - Multi-symbol pipeline
- ✅ **Python Environment Automation** - Automated setup, testing, and data collection scripts ⭐ FIXED
- ✅ **Complete Data Pipeline** - Fresh market data with 653+ live options and 10+ stock analysis ⭐ NEW
- ✅ **Advanced Personality-Driven Trading** - Multi-dimensional stock behavior analysis with market regime detection and sector normalization ⭐ ENHANCED
- ✅ **Intelligent Strategy Matching** - Confidence-based strategy selection with 20-70% confidence scoring ⭐ ENHANCED
- ✅ **PersonalityBasedBot** - Live trading with advanced personality-optimized strategies ⭐ NEW

**Compilation:** ✅ Clean build (minor warnings only)  
**Performance:** ✅ Optimized with `--release` builds  
**Documentation:** ✅ Comprehensive guides and examples

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

## 📊 Data Coverage & Portfolio Expansion

### 📈 Current Core Holdings
**Symbols with Live Options Data:**
- TSLA, AAPL, NVDA, MSFT (JSON options chains available)

**Symbols with Historical Data:**
- TSLA, AAPL, GOOGL, NVDA, MSFT, AMZN, META (CSV files)

### 🎯 Recommended Portfolio Expansion

#### **Tier 1: High-Volume ETF Leaders** 🔥
*Essential building blocks for any options portfolio*
- **SPY** - S&P 500 ETF (Highest options volume globally, tight spreads)
- **QQQ** - Nasdaq 100 ETF (Tech concentration, high volatility)
- **IWM** - Russell 2000 ETF (Small-cap exposure, higher premiums)
- **GLD** - Gold ETF (Safe haven, inflation hedge, negative correlation)
- **TLT** - 20+ Year Treasury ETF (Interest rate sensitivity, recession hedge)

#### **Tier 2: High-Beta Momentum Plays** ⚡
*Perfect for volatility strategies and breakout trading*
- **AMD** - Advanced Micro Devices (High-beta semiconductor leader)
- **COIN** - Coinbase (Crypto proxy, extreme volatility)
- **PLTR** - Palantir (Meme stock favorite, retail sentiment)
- **ARKK** - ARK Innovation ETF (Disruptive tech, high growth)
- **RBLX** - Roblox (Gaming, metaverse exposure)

#### **Tier 3: Sector Diversification** 🏭
*Essential for balanced portfolio exposure*
- **JPM** - JPMorgan Chase (Banking leader, rate sensitivity)
- **JNJ** - Johnson & Johnson (Defensive healthcare, dividend yield)
- **XOM** - ExxonMobil (Energy giant, commodity exposure)
- **DIS** - Disney (Entertainment, reopening beneficiary)
- **WMT** - Walmart (Consumer staples, recession-resistant)
- **UNH** - UnitedHealth (Healthcare services, aging demographics)

#### **Tier 4: Specialized Strategies** 🎯
*Advanced trading opportunities and hedging*
- **VIX** - Volatility Index (Pure volatility play, tail risk hedging)
- **UVXY** - VIX Short-Term Futures ETN (Leveraged volatility)
- **SQQQ** - ProShares UltraPro Short QQQ (3x inverse, market hedging)
- **FXI** - China Large-Cap ETF (Emerging market exposure)
- **EWZ** - Brazil ETF (Latin America, commodities)

### 📊 Strategic Portfolio Matrix

| Tier | Allocation | Purpose | Vol Level | Liquidity | Strategy Focus |
|------|------------|---------|-----------|-----------|---------------|
| **Core ETFs** | 40% | Market exposure | Medium | Highest | Spreads, covered calls |
| **Tech Growth** | 30% | Momentum capture | High | High | Breakouts, straddles |
| **Diversification** | 20% | Risk reduction | Low-Med | Medium | Income, defense |
| **Specialized** | 10% | Alpha/hedging | Extreme | Variable | Vol arb, tail risk |

### 💡 Implementation Roadmap

**Phase 1: Core Foundation (Week 1)**
```
Immediate Adds: SPY, QQQ, GLD
Focus: High liquidity, diversification
Strategies: Market neutral spreads, covered calls
```

**Phase 2: Growth Enhancement (Week 2)**
```
Growth Adds: AMD, COIN, JPM
Focus: Volatility capture, sector exposure
Strategies: Momentum plays, earnings straddles
```

**Phase 3: Advanced Strategies (Week 3)**
```
Advanced Adds: VIX, UVXY, FXI
Focus: Hedging, international exposure
Strategies: Volatility arbitrage, tail risk management
```

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
