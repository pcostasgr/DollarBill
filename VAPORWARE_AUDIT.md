# Vaporware Audit - Files and Code to Delete

## 🗑️ Configuration Files (Non-functional)

### Delete These Configs:
- `config/ml_config.json` - No ML integration exists, just a config file
- `config/personality_bot_config.json` - Bot uses hardcoded logic, not this config
- `config/signals_config.json` - Not used by signal generation
- `config/strategy_deployment.json` - Strategy matching uses hardcoded data

### Keep These Configs:
- `config/stocks.json` - **USED** - Central stock configuration
- `config/trading_bot_config.json` - **USED** - Alpaca API settings
- `config/paper_trading_config.json` - **USED** - Paper trading settings
- `config/strategy_config.json` - **USED** - Basic strategy parameters
- `config/vol_surface_config.json` - **USED** - Volatility surface settings
- `config/personality_config.json` - **PARTIALLY USED** - Basic threshold settings only

## 📝 Source Code Files

### Barely Used (~1500 lines of code)
- `src/analysis/advanced_classifier.rs` (791 lines)
  - **Status:** Exists, compiles, but compiler warns "never read"
  - **Used in:** 2 optional examples only (enhanced_personality_analysis.rs, personality_driven_pipeline.rs)
  - **NOT used in:** Main workflow (multi_symbol_signals.rs)
  - **Recommendation:** Keep but document as experimental/optional

- `src/analysis/performance_matrix.rs` (unknown lines)
  - **Status:** Hardcoded performance data, not data-driven
  - **Lines 174-220:** Placeholder lookup table from ONE backtest
  - **Recommendation:** Keep but mark as prototype/placeholder

### Strategy Files (Check Usage)
- `src/strategies/matching.rs` - Uses hardcoded data, not dynamic
- `src/strategies/vol_arbitrage.rs` - Basic single-asset only (NOT cross-asset despite claims)

## 🔬 Example Files (Educational/Experimental)

### Rarely Run Examples:
- `examples/ml_enhanced_signals.rs` - **ML vaporware** - No actual ML integration
- `examples/cali_enhanced_signals.rs` - **California-specific?** - Unclear purpose
- `examples/enhanced_personality_analysis.rs` - Uses advanced_classifier (works but rarely used)
- `examples/personality_driven_pipeline.rs` - Uses advanced_classifier
- `examples/personality_based_bot.rs` - Uses personality features
- `examples/strategy_deployment.rs` - Strategy deployment patterns demo

### Keep Main Examples:
- `examples/multi_symbol_signals.rs` - **MAIN WORKFLOW** - Actually used
- `examples/backtest_strategy.rs` - **CORE** - Backtesting
- `examples/backtest_heston.rs` - Heston backtesting
- `examples/vol_surface_analysis.rs` - Vol surface extraction
- `examples/calibrate_live_options.rs` - Heston calibration
- `examples/paper_trading.rs` - Paper trading
- `examples/trading_bot.rs` - Bot execution

## 📚 Documentation Files

### Over-Hyped Docs:
- `docs/enhanced-personality-implementation.md` - Describes advanced_classifier (barely used)
- `docs/personality-guide.md` - Oversells personality features
- `docs/competitive-analysis.md` - Compares to enterprise platforms (misleading)
- `docs/parameter_atlas.md` - Documents vaporware config parameters

### Useful Docs:
- `docs/getting-started.md` - Actual quick start
- `docs/alpaca-guide.md` - Real Alpaca integration
- `docs/backtesting-guide.md` - Real backtesting
- `docs/trading-guide.md` - Real trading workflows
- `docs/implementation-summary.md` - Technical details
- `docs/advanced-features.md` - Mixed (some real, some vaporware)

## 🐍 Python Scripts

### All Python Scripts Are Functional:
- `py/fetch_multi_stocks.py` - **WORKS** - Fetches stock data
- `py/fetch_multi_options.py` - **WORKS** - Fetches options chains
- `py/plot_vol_surface.py` - **WORKS** - 3D visualization
- Keep all Python scripts - they deliver real value

## 🛠️ Scripts

### Batch/PowerShell Scripts:
- All scripts in `scripts/` are functional
- Keep all - they work and provide automation

## 📊 Vaporware Features (Zero Implementation)

### Portfolio Features (0 lines of code):
1. Multi-Asset Portfolio Construction - **ZERO CODE**
2. Sector Rotation - **ZERO CODE**
3. Cross-Asset Arbitrage - **ZERO CODE** (only single-asset vol_arbitrage exists)
4. Currency Hedging - **ZERO CODE**
5. Event-Driven Trading - **ZERO CODE**
6. Tail Risk Management - **ZERO CODE**
7. Correlation Trading - **ZERO CODE**
8. Regime-Based Allocation - **ZERO CODE**

### ML Features (Config files only, no integration):
- ML config exists but no Rust-Python bridge (PyO3 not implemented)
- No trained models in repo
- No model loading code
- Just config files suggesting it exists

## 🎯 Deletion Recommendations

### Phase 1: Safe Deletes (Zero Impact)
```bash
# Delete vaporware configs
rm config/ml_config.json
rm config/personality_bot_config.json
rm config/signals_config.json
rm config/strategy_deployment.json

# Delete misleading docs
rm docs/competitive-analysis.md
rm docs/parameter_atlas.md
```

### Phase 2: Consider Deleting (Low Value)
```bash
# Experimental examples rarely used
rm examples/ml_enhanced_signals.rs
rm examples/cali_enhanced_signals.rs
rm examples/strategy_deployment.rs

# Over-hyped documentation
rm docs/enhanced-personality-implementation.md
mv docs/personality-guide.md docs/personality-guide-experimental.md
```

### Phase 3: Mark as Experimental (Keep but Warn)
- Keep `src/analysis/advanced_classifier.rs` but add warning comment
- Keep personality examples but mark as experimental
- Keep performance_matrix.rs but document hardcoded data

## 📈 Code Cleanup (Non-Deletes)

### Add Honest Comments:
```rust
// src/analysis/advanced_classifier.rs
// WARNING: This classifier is experimental and barely used in production.
// Only called in optional examples, not in main signal generation workflow.
// Compiler warnings indicate most fields are never read.

// src/strategies/matching.rs (line 174)
// TODO: Replace hardcoded performance data with actual historical backtests
// Current implementation uses placeholder data from ONE sample backtest
```

### Update README Warnings:
- Already done in previous edits
- Portfolio features marked as "planned" not "implemented"
- ML integration marked as non-existent

## 📋 Summary

**Safe to Delete:** ~4 config files, 2-3 docs
**Consider Deleting:** 3 experimental examples, 2 over-hyped docs
**Keep but Mark Experimental:** advanced_classifier, personality examples
**Total Vaporware Code:** ~1500 lines (mostly in advanced_classifier.rs)
**Total Fake Features:** 8 portfolio features (0 lines of code)

**Net Effect:** Removing ~2000 lines of misleading code/configs, keeping ~500 lines of experimental features clearly marked as such.
