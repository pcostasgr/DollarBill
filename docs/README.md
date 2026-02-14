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
- **[Test Implementation Summary](test-implementation-summary.md)** - Test results and coverage (97 tests, 100% passing) ⭐ NEW
- **[Failed Tests Analysis](failed-tests-analysis.md)** - Resolved test issues and mathematical explanations ⭐ NEW

## 📂 Project Structure

```
DollarBill/
├── config/                  # JSON configuration files
├── docs/                    # Documentation (this folder)
├── src/                     # Rust source code
├── examples/               # Rust example programs
├── py/                     # Python utilities
├── scripts/                # Shell/batch scripts
├── data/                   # CSV/JSON data files
├── images/                 # Generated charts and visualizations
└── README.md              # Main project documentation
```

## 🚀 Quick Links

- [Run Multi-Symbol Signals](../scripts/run_multi_signals.ps1)
- [Fetch Market Data](../py/fetch_multi_stocks.py)
- [View Backtest Results](../scripts/run_backtest.ps1)
- [Run Heston Backtesting](../scripts/run_heston_backtest.ps1)
- [Generate Volatility Surfaces](../scripts/run_vol_surface.ps1)
- [View Generated Charts](../images/)

## 📞 Support

For questions or issues, check the inline code comments or create an issue in the repository.