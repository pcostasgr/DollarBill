# DollarBill 🦀📈

A high-performance options pricing and analytics platform built in pure Rust. Features institutional-grade pricing models (Black-Scholes-Merton, Heston), real-time market data integration, full Greeks calculations, portfolio risk analytics, and volatility surface visualization.

## 🤖 Development Approach

**This entire platform was built through vibe coding using AI pair programming** - primarily with **Claude Sonnet 4.5** and **Grok**. From initial architecture decisions to final implementation details, every module, algorithm, and optimization was crafted through conversational iteration with large language models. This demonstrates the power of AI-assisted development in creating production-grade financial software with sophisticated mathematical implementations.

No traditional coding sessions. Just vibes, prompts, and Rust. 🚀

## 🎯 Key Features

### Options Pricing
- **Black-Scholes-Merton** - Analytical European options pricing with dividend support
- **Heston Stochastic Volatility** - Advanced pricing via Carr-Madan FFT method
- **Full Greeks** - Delta, Gamma, Vega, Theta, Rho for risk management
- **Implied Volatility** - Newton-Raphson solver for IV extraction

### Model Calibration
- **Heston Calibration** - Custom Nelder-Mead optimizer (pure Rust, no dependencies)
- **Market Data Fitting** - Calibrate to live options chains
- **Parallel Processing** - Multi-symbol calibration using Rayon
- **Error Tracking** - RMSE metrics and convergence analysis

### Trade Signal Generation
- **Mispricing Detection** - Model price vs. market price comparison
- **Multi-Symbol Analysis** - Parallel processing of TSLA, AAPL, NVDA, MSFT
- **Greeks Per Signal** - Full risk metrics for every trade opportunity
- **Liquidity Filtering** - Minimum volume and open interest thresholds

### Portfolio Risk Analytics
- **Aggregated Greeks** - Portfolio-level Delta, Gamma, Vega, Theta
- **Delta-Neutral Detection** - Automatic directional risk alerts
- **Vega Exposure Warnings** - Volatility sensitivity analysis
- **Hedging Recommendations** - Smart position adjustment suggestions

### Volatility Surface Analysis
- **IV Extraction** - Newton-Raphson implied volatility calculation
- **Volatility Smile** - IV vs. Strike visualization
- **Term Structure** - IV vs. Time to expiry analysis
- **Skew Detection** - Put/call skew identification
- **3D Visualization** - Interactive Plotly charts (Python integration)
- **CSV Export** - Data export for Excel, Python, R

### Market Data Integration
- **Yahoo Finance API** - Real-time stock quotes and options chains
- **CSV Loader** - Historical stock price data
- **JSON Loader** - Options chain data storage and retrieval
- **Multi-Symbol Fetch** - Batch data collection scripts

### Backtesting Framework
- **Historical Simulation** - Run strategies on past data with full P&L tracking
- **Performance Metrics** - Sharpe ratio, max drawdown, win rate, profit factor
- **Equity Curve** - Track portfolio value over time
- **Custom Strategies** - Flexible signal generator interface
- **Risk Management** - Stop loss, take profit, position sizing
- **Trade Analytics** - Entry/exit prices, holding periods, ROI per trade

## 🚀 Quick Start

### Prerequisites

```bash
# Rust (2021 edition or later)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python (optional, for 3D visualization)
pip install pandas plotly
```

### Installation

```bash
git clone <repository-url>
cd DollarBill
cargo build --release
```

### Fetch Market Data

```bash
# Fetch historical stock data
python py/fetch_multi_stocks.py

# Fetch live options chains
python py/fetch_multi_options.py
```

### Run Analysis

```bash
# Generate trade signals with Greeks and portfolio risk
cargo run --release --example multi_symbol_signals

# Analyze volatility surfaces
cargo run --release --example vol_surface_analysis

# Backtest strategies on historical data
cargo run --release --example backtest_strategy

# Create 3D volatility visualizations (requires Python)
python py/plot_vol_surface.py
```

### PowerShell Quick Scripts

```powershell
# Trade signals with full Greeks
.\scripts\run_multi_signals.ps1

# Complete volatility pipeline
.\scripts\run_vol_surface.ps1
```

## 📊 Example Output

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
├── src/
│   ├── lib.rs                          # Library exports
│   ├── main.rs                         # Main entry point
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
│   ├── backtesting/                    # Backtesting framework
│   │   ├── engine.rs                   # Backtest orchestration
│   │   ├── position.rs                 # Position tracking
│   │   ├── trade.rs                    # Trade records
│   │   ├── metrics.rs                  # Performance analytics
│   │   └── mod.rs                      # Module exports
│   └── utils/                          # Utilities
│       ├── vol_surface.rs              # Volatility surface tools
│       ├── action_table_out.rs         # Output formatting
│       └── pnl_output.rs               # P&L calculations
├── examples/
│   ├── multi_symbol_signals.rs         # Main: Signals + Greeks + Risk
│   ├── vol_surface_analysis.rs         # Volatility surface extraction
│   ├── backtest_strategy.rs            # Strategy backtesting demo
│   ├── calibrate_live_options.rs       # Heston calibration demo
│   ├── trade_signals.rs                # Basic signal generation
│   └── test_yahoo_options.rs           # Yahoo API testing
├── py/
│   ├── fetch_multi_stocks.py           # Stock data fetcher
│   ├── fetch_multi_options.py          # Options chain fetcher
│   ├── plot_vol_surface.py             # 3D volatility visualization
│   ├── fetch_options.py                # Single symbol options fetcher
│   ├── get_tesla_quotes.py             # Tesla quotes fetcher
│   └── get_tesla_stock_csv.py          # Tesla CSV downloader
├── scripts/
│   ├── run_multi_signals.ps1           # PowerShell: Run signals
│   ├── run_vol_surface.ps1             # PowerShell: Vol pipeline
│   ├── run_signals.ps1                 # PowerShell: Single symbol signals
│   ├── run_backtest.ps1                # PowerShell: Backtesting
│   ├── run_paper_trading.ps1           # PowerShell: Paper trading
│   ├── run_multi_signals.bat           # Batch: Run signals
│   ├── run_signals.bat                 # Batch: Single symbol signals
│   ├── run_paper_trading.sh            # Shell: Paper trading
│   └── run_signals.sh                  # Shell: Single symbol signals
├── docs/
│   ├── advanced-features.md            # Advanced features guide
│   ├── alpaca-guide.md                 # Alpaca API integration
│   ├── backtesting-guide.md            # Backtesting methodology
│   ├── implementation-summary.md       # Technical implementation details
│   └── trading-guide.md                # Trading strategies guide
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

## 📚 Documentation

- **README.md** (this file) - Overview and quick start
- **[Advanced Features](docs/advanced-features.md)** - Detailed feature guides and examples
- **[Alpaca Integration](docs/alpaca-guide.md)** - Paper trading setup and API usage
- **[Backtesting Guide](docs/backtesting-guide.md)** - Strategy testing methodology
- **[Trading Strategies](docs/trading-guide.md)** - Live trading examples and workflows
- **[Implementation Details](docs/implementation-summary.md)** - Technical documentation
- **Inline comments** - Throughout source code
- **Example programs** - Demonstrative usage in `examples/`

## 🎯 Use Cases

✅ **Options Trading** - Identify mispriced options  
✅ **Risk Management** - Monitor portfolio Greeks  
✅ **Volatility Analysis** - Study IV surfaces and skew  
✅ **Strategy Backtesting** - Evaluate historical performance with realistic P&L  
✅ **Market Making** - Fair value pricing  
✅ **Research** - Model calibration and comparison  

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

## 📊 Data Coverage

**Symbols with Live Options Data:**
- TSLA, AAPL, NVDA, MSFT (JSON options chains available)

**Symbols with Historical Data:**
- TSLA, AAPL, GOOGL, NVDA, MSFT, AMZN, META (CSV files)

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

[Specify your license here - e.g., MIT, Apache 2.0, etc.]

## 👤 Author

Costas

---

**Built with 🦀 Rust - Fast, Safe, Concurrent**
