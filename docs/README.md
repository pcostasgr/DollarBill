# Documentation Index

Welcome to the DollarBill documentation! This folder contains comprehensive guides for using and understanding the platform.

## 📖 User Guides

### Getting Started
- **[Getting Started Guide](getting-started.md)** - Quick setup for personality-driven trading ⭐ NEW
- **[Main README](../README.md)** - Project overview, installation, and quick start guide

### Trading & Strategies
- **[Personality Guide](personality-guide.md)** - Personality-driven trading system and live bot ⭐ NEW
- **[Trading Guide](trading-guide.md)** - Live trading examples, paper trading setup, and strategy workflows
- **[Strategy Deployment](trading-guide.md#strategy-deployment-system)** - Modular strategy architecture and deployment patterns
- **[Alpaca Integration](alpaca-guide.md)** - Complete guide for Alpaca API integration and paper trading
- **[Backtesting Guide](backtesting-guide.md)** - Methodology for testing trading strategies on historical data

### Features & Usage
- **[Advanced Features](advanced-features.md)** - Detailed guides for platform features, Greeks calculation, and advanced functionality

## 🔧 Technical Documentation

### Development
- **[Implementation Summary](implementation-summary.md)** - Technical details, architecture, and implementation notes
- **[Parameter Atlas](parameter_atlas.md)** - Complete reference for all configuration parameters
- **[Testing Strategies](testing-strategies.md)** - Comprehensive test plan and test categories
- **[Test Implementation Summary](test-implementation-summary.md)** - Test results and coverage (421+ tests, 100% passing) ⭐ UPDATED
- **[Failed Tests Analysis](failed-tests-analysis.md)** - Resolved test issues and mathematical explanations
- **[Benchmark Summary](benchmarks/SUMMARY.md)** - Criterion benchmarks with QuantLib cross-validation ⭐ UPDATED

## 📂 Project Structure

```
DollarBill/
├── config/                  # JSON configuration files
├── docs/                    # Documentation (this folder)
│   └── benchmarks/          # Criterion HTML reports & QuantLib comparison
├── src/                     # Rust source code
│   └── models/              # BS, Heston, Gauss-Laguerre quadrature
├── benches/                 # Criterion benchmark harnesses
├── tests/                   # Integration & unit test suite (307 tests)
├── examples/                # Rust example programs
├── py/                      # Python utilities & QuantLib reference scripts
├── scripts/                 # Shell/batch scripts
├── data/                    # CSV/JSON data files
├── images/                  # Generated charts and visualizations
└── README.md                # Main project documentation
```

## 🚀 Quick Links

- [Run Multi-Symbol Signals](../scripts/run_multi_signals.ps1)
- [Fetch Market Data](../py/fetch_multi_stocks.py)
- [View Backtest Results](../scripts/run_backtest.ps1)
- [Run Heston Backtesting](../scripts/run_heston_backtest.ps1)
- [Generate Volatility Surfaces](../scripts/run_vol_surface.ps1)
- [View Generated Charts](../images/)

## 📞 Getting Started

### Heston Pricing
- **Gauss-Laguerre (recommended)**: `IntegrationMethod::GaussLaguerre(64)` — 33 µs/call, matches QuantLib to 6 sig figs
- **Batch pricing (CF cache)**: `HestonCfCache::new()` + `price_calls()` — **2.3 µs/opt** amortized across strikes (10× faster) 🆕
- **Carr-Madan (legacy)**: `IntegrationMethod::CarrMadan` — 474 µs/call, uses original characteristic function
- Configure in `config/vol_surface_config.json` (`integration_method` + `gauss_laguerre_nodes`)

For questions or issues, check the inline code comments or create an issue in the repository.