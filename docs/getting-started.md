# 🚀 DollarBill Getting Started Guide

**Get trading with enhanced multi-dimensional personality-driven strategies in under 7 minutes (fast track) or 15 minutes (step-by-step)**

**⭐ NEW: Advanced Personality System** - Now featuring 15+ sophisticated features with percentile-based volatility analysis, market regime detection, and intelligent confidence scoring!

## ⚡ Quick Prerequisites (2 minutes)

### 1. Install Rust
```bash
# Download and install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 2. Clone and Build
```bash
git clone https://github.com/yourusername/DollarBill.git
cd DollarBill
cargo build --release
```

### 3. Install Python (Optional, for data fetching)
```bash
pip install pandas plotly yfinance
```

### Fast Track: Personality Trading (7 minutes)

**Quick setup combining steps 2-4:**
```powershell
# One-command preparation (PowerShell)
.\scripts\heston_preparation.ps1
```
```batch
# Or use batch version
.\scripts\heston_preparation.bat
```

This script automatically:
- **Step 2**: Fetches market data (historical stocks + live options)
- **Step 3**: Runs Heston backtesting with live market calibration
- **Step 4**: Trains personality models and matches optimal strategies

**Then proceed to steps 5-6 for testing and live trading!**

#### Step 1: Configure Stocks (1 minute)
Edit `config/stocks.json` to choose your stocks:
```json
{
  "stocks": [
    {"symbol": "TSLA", "market": "US", "enabled": true},
    {"symbol": "AAPL", "market": "US", "enabled": true},
    {"symbol": "NVDA", "market": "US", "enabled": true}
  ]
}
```

#### Step 2: Fetch Market Data (2 minutes)
```bash
# Get historical stock data
python py/fetch_multi_stocks.py

# Get live options data
python py/fetch_multi_options.py
```

#### Step 3: Run Heston Backtesting (2 minutes)
```powershell
# CRITICAL: Build accurate performance data for live trading
.\scripts\run_heston_backtest.ps1
```
This calibrates Heston parameters to live market data and builds the performance matrix that the bot uses for trading decisions. **Essential for realistic results!**

#### Step 4: Train Enhanced Personality Models (3 minutes)
```bash
# See the new enhanced personality system in action
cargo run --example enhanced_personality_analysis

# Run the complete personality pipeline with advanced features
cargo run --example personality_driven_pipeline
```
This analyzes stock behaviors using advanced multi-dimensional features, detects market regimes, and matches optimal strategies with confidence scoring.

#### Step 5: Test Live Trading (2 minutes)
```bash
# Test without real trades
cargo run --example personality_based_bot -- --dry-run
```

#### Step 6: Go Live (2 minutes)
Set up Alpaca paper trading:
```bash
# Set your Alpaca credentials
$env:ALPACA_API_KEY="your-paper-api-key"
$env:ALPACA_API_SECRET="your-paper-api-secret"

# Start automated trading
cargo run --example personality_based_bot -- --continuous 5
```

## 📊 What Just Happened

✅ **Market Data**: Historical prices and live options fetched
✅ **Heston Calibration**: Parameters fitted to current market conditions
✅ **Enhanced Personality Analysis**: Stocks classified using 15+ features: volatility percentiles, market regime detection, trend persistence
✅ **Intelligent Strategy Matching**: Each stock got optimal strategy with 20-70% confidence scoring
✅ **Market Regime Awareness**: LowVol/HighVol/Trending/MeanReverting classification for context-aware trading
✅ **Backtesting**: Strategies validated with realistic Heston pricing
✅ **Live Testing**: Dry-run confirmed advanced signal generation works
✅ **Live Trading**: Automated execution with confidence-based risk management

## 🎛️ Quick Configuration

### Personality Bot Settings
Edit `config/personality_bot_config.json`:
```json
{
  "trading": {
    "position_size_shares": 5,    // Small position size to start
    "max_positions": 3,           // Limit concurrent positions
    "min_confidence": 0.6         // Only high-confidence trades
  }
}
```

### Risk Controls
- **Position Size**: Start small (5 shares)
- **Max Positions**: Limit to 3-5 stocks
- **Confidence**: Only trade signals >60% confidence
- **Paper Trading**: Use Alpaca paper account first

## 📈 Monitor Your Bot

### Live Output Example
```
🎭 Personality-Based Trading Bot with Enhanced Analytics
================================================================================

💰 Account: $98543.67 cash | $142456.33 portfolio value

🧠 Enhanced Personality Analysis Results:
   TSLA: VolatileBreaker (confidence: 30.0%) | Vol: 91.7% | Regime: HighVol
   PLTR: MomentumLeader (confidence: 50.0%) | Vol: 97.2% | Trend: 98.5%
   AAPL: TrendFollower (confidence: 20.0%) | Vol: 45.2% | Regime: Trending

🚀 Trading Decisions:
   TSLA $247.89 | Strategy: Iron Butterfly | Conf: 30.0% | ⏸️ HOLD (low confidence)
   PLTR $185.23 | Strategy: Short-Term Momentum | Conf: 50.0% | 🟢 BUY → 5 shares... ✅
   AAPL $192.45 | Strategy: Medium-Term RSI | Conf: 20.0% | ⏸️ HOLD (low confidence)
```

### Key Metrics to Watch
- **Account Balance**: Should grow steadily
- **Win Rate**: Look for >60% winning trades
- **Confidence Scores**: 20-70% range (higher is better, <40% = hold)
- **Market Regime**: Adaptive strategy selection based on HighVol/LowVol/Trending/MeanReverting
- **Position Count**: Stay within your limits
- **Volatility Percentiles**: Track relative volatility rankings

## 🚨 Safety First

### ⚠️ Critical: Heston Backtesting Required
**Before live trading, always run Heston backtesting first:**
```powershell
.\scripts\run_heston_backtest.ps1
```
This builds accurate performance data. **Skipping this step means trading with potentially unreliable strategy performance data!**

### Start Small
- Use **paper trading** only initially
- **Small position sizes** (5-10 shares)
- **Limit max positions** (3-5 stocks)
- **Monitor daily** for the first week

### Emergency Stops
```bash
# Stop the bot (Ctrl+C in terminal)
# Or close all positions manually in Alpaca app
```

### Risk Limits
- **Never risk more than 1-2%** of account per trade
- **Set stop-loss orders** in Alpaca if needed
- **Take profits regularly** to lock in gains

## 🔄 Daily Workflow

### Morning (5 minutes)
```bash
# Check account status
cargo run --example personality_based_bot -- --dry-run

# Start trading
cargo run --example personality_based_bot -- --continuous 15
```

### Evening (2 minutes)
- Review daily P&L
- Check position performance
- Adjust position sizes if needed

### Weekly (10 minutes)
```bash
# Review enhanced personality analysis with latest data
cargo run --example enhanced_personality_analysis

# Update models with new market data
cargo run --example personality_driven_pipeline

# Refresh Heston calibration for current market conditions
.\scripts\run_heston_backtest.ps1

# Test updated strategies with confidence scoring
cargo run --example personality_based_bot -- --dry-run
```

## 🎯 Next Steps

### Level Up Your Trading
1. **Add More Stocks**: Enable additional symbols in `config/stocks.json`
2. **Increase Position Sizes**: Gradually increase from 5 to 10-20 shares
3. **Customize Strategies**: Modify confidence thresholds and risk settings
4. **Add Stop Losses**: Implement additional risk management in Alpaca

### Advanced Features
- **Real-time Alerts**: Monitor bot performance
- **Performance Analytics**: Track detailed metrics
- **Strategy Optimization**: Fine-tune personality models
- **Portfolio Rebalancing**: Adjust allocations automatically

## 🆘 Need Help?

### Common Issues
- **"No Alpaca credentials"**: Set environment variables correctly
- **"No historical data"**: Run data fetching scripts first
- **"Strategy not found"**: Re-run personality pipeline to train models
- **"Poor performance/unrealistic results"**: Run Heston backtesting first - `.\scripts\run_heston_backtest.ps1`
- **Low confidence signals**: Normal with enhanced system - bot only trades high-confidence opportunities (>40%)
- **"File structure mismatch"**: Enhanced personality system requires latest code structure
- **"Classification errors"**: Check that all stocks have sufficient historical data (2+ years)

### Get Support
- Check the [Personality Guide](docs/personality-guide.md) for detailed documentation
- Review [Alpaca Setup](docs/alpaca-guide.md) for API configuration
- Monitor logs for error messages and troubleshooting

---

**🎉 You're now running an AI-powered trading system with advanced multi-dimensional personality analysis that adapts strategies to each stock's behavior with sophisticated confidence scoring. Welcome to the future of automated trading!**

> **Enhanced Personality System**: The new multi-dimensional analysis provides significantly more accurate classification than the legacy fixed-threshold system. Confidence scores of 20-70% are normal and provide intelligent risk management.