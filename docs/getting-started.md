# 🚀 DollarBill Getting Started Guide

**Get trading with personality-driven strategies in under 15 minutes**

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

## 🎯 Fast Track: Personality Trading (10 minutes)

### Step 1: Configure Stocks (1 minute)
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

### Step 2: Fetch Market Data (2 minutes)
```bash
# Get historical stock data
python py/fetch_multi_stocks.py

# Get live options data
python py/fetch_multi_options.py
```

### Step 3: Train Personality Models (3 minutes)
```bash
# Run the complete personality pipeline
cargo run --example personality_driven_pipeline
```
This analyzes stock behaviors, matches optimal strategies, and saves trained models.

### Step 4: Test Live Trading (2 minutes)
```bash
# Test without real trades
cargo run --example personality_based_bot -- --dry-run
```

### Step 5: Go Live (2 minutes)
Set up Alpaca paper trading:
```bash
# Set your Alpaca credentials
$env:ALPACA_API_KEY="your-paper-api-key"
$env:ALPACA_API_SECRET="your-paper-api-secret"

# Start automated trading
cargo run --example personality_based_bot -- --continuous 5
```

## 📊 What Just Happened

✅ **Personality Analysis**: Stocks classified as MomentumLeader, MeanReverting, etc.
✅ **Strategy Matching**: Each stock got its optimal trading strategy
✅ **Backtesting**: Strategies validated with historical performance
✅ **Live Trading**: Automated execution with risk management

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
🎭 Personality-Based Trading Bot
================================================================================

💰 Account: $98543.67 cash | $142456.33 portfolio value

🧠 Analyzing with Personality-Driven Strategies...

   TSLA $247.89 | Strategy: Momentum | Conf: 0.78% | 🟢 BUY → 5 shares... ✅
   AAPL $192.45 | Strategy: Vol Mean Reversion | Conf: 0.82% | ⏸️ HOLD
   NVDA $875.30 | Strategy: Iron Condor | Conf: 0.71% | 🔴 SELL → Closing... ✅
```

### Key Metrics to Watch
- **Account Balance**: Should grow steadily
- **Win Rate**: Look for >60% winning trades
- **Confidence Scores**: Higher is better
- **Position Count**: Stay within your limits

## 🚨 Safety First

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
# Update models with new data
cargo run --example personality_driven_pipeline

# Test updated strategies
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
- **Low confidence signals**: Normal - bot only trades high-confidence opportunities

### Get Support
- Check the [Personality Guide](docs/personality-guide.md) for detailed documentation
- Review [Alpaca Setup](docs/alpaca-guide.md) for API configuration
- Monitor logs for error messages and troubleshooting

---

**🎉 You're now running an AI-powered trading system that adapts strategies to each stock's personality. Welcome to the future of automated trading!**