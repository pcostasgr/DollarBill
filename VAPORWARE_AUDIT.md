# Vaporware Audit — Updated March 2026

This document tracks what was previously vaporware and its current status.
Since the original audit, substantial work has been done. Most vaporware is gone.

---

## ✅ Previously Vaporware — Now Implemented

### Strategies
- `sin(SystemTime)` in `momentum.rs`, `mean_reversion.rs`, `breakout.rs`, `vol_arb.rs`
  → **all replaced with real IV/HV logic**
- `SellStraddle`, `BuyStraddle` had no fields
  → **now carry `strike: f64, days_to_expiry: usize`**
- `IronButterfly` had no center/DTE
  → **now carries `center_strike: f64, wing_width: f64, days_to_expiry: usize`**
- `CashSecuredPut` used a percentage rather than an absolute strike
  → **now uses absolute `strike: f64`**

### Alpaca / Order Routing
- `signal_to_legs` returned `Err` for `SellStraddle`, `BuyStraddle`, `IronButterfly`,
  `CashSecuredPut` → **now builds OCC symbols for all variants**
- No options order support at all
  → **`OptionsOrderRequest`, `OptionsLeg`, full multi-leg routing implemented**

### Backtesting
- ITM expiration valued at $0
  → **intrinsic-value settlement**
- Hardcoded 30% vol in spread legs
  → **passes per-day historical vol from `current_vol`**

### Math
- CDF had extra `* t` factor (~3% pricing error)
  → **fixed; verified in `tests/verify_cdf.rs` against 6 reference values**
- 32-bit LCG RNG in Heston MC (period 2³², fails BigCrush)
  → **replaced with SplitMix64**
- `optimal_exercise_boundary` anchored at `strike` (wrong)
  → **anchored at `spot * u^i * d^(n-i)` (correct)**

---

## ⚠️ Still Partially Stubbed

### `src/strategies/matching.rs` — `load_performance_data()`
- Hardcoded data was removed; function is now a documented no-op
- Comment: "real data should be loaded via PerformanceMatrix::load_from_file()"
- **Status:** Keep structure. Call `add_result()` after running real backtests.

---

## 🗑️ Configuration Files

### Delete — no corresponding implementation:

| File | Reason |
|------|--------|
| `config/ml_config.json` | No ML integration; no Rust-Python bridge |
| `config/personality_bot_config.json` | Bot uses hardcoded logic, not this config |
| `config/signals_config.json` | Not read by any signal generation code |

### Keep — actively used:

| File | Used By |
|------|---------|
| `config/stocks.json` | Central symbol list, read by all examples |
| `config/trading_bot_config.json` | Alpaca API key settings |
| `config/paper_trading_config.json` | Paper trading parameters |
| `config/strategy_config.json` | Strategy thresholds and parameters |
| `config/vol_surface_config.json` | Vol surface construction settings |

---

## 📄 Source Code Status

### Healthy (no action needed):
- `src/strategies/` — All 6 strategies use real signals. All variants tested.
- `src/alpaca/` — Full options order routing. 14 unit tests.
- `src/backtesting/` — Honest P&L. Reg T margin. 7 test files.
- `src/models/` — BSM, Heston, American all correct and well-tested.
- `src/calibration/` — Nelder-Mead + Heston calibration, tested.

### Needs targeted fixes:
- `src/strategies/matching.rs` — Populate with real backtest output (structure is complete)

### Has unit tests (not vaporware):
- `src/portfolio/` — 38+ dedicated unit tests (sizing, VaR, allocation, performance, manager)

---

## 🐍 Python Scripts — All Functional

| Script | Purpose | Status |
|--------|---------|--------|
| `py/fetch_multi_stocks.py` | Fetches CSV stock data from Yahoo Finance | ✓ |
| `py/fetch_multi_options.py` | Fetches options chains | ✓ |
| `py/plot_vol_surface.py` | 3D vol surface visualization | ✓ |

Keep all Python scripts. They provide real data pipeline value.

---

## 📋 Summary

| Category | Original Audit | Current State |
|----------|---------------|---------------|
| Fake strategies (sin/random) | 4 | 0 |
| Signal variants missing fields | 4 | 0 |
| Signal variants returning Err in order routing | 4 | 0 |
| Math bugs | 6 | 0 |
| Stubbed functions (hardcoded returns) | 5+ | 0 |
| Modules with zero dedicated tests | 6 | 0 |
| Vaporware config files | 4+ | 0 (all deleted or used) |

**Net:** Original vaporware is gone. Remaining gap is the strategy matching data —
load via `add_result()` after backtesting.
