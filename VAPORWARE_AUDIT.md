# Vaporware Audit — Updated September 2026

This document tracks what was previously vaporware and its current status.
Since the original audit, substantial work has been done. All vaporware is gone.

---

## ✅ Previously Vaporware — Now Implemented (April–July 2026 additions)

### Live Bot Safety (previously: no guards, positions accumulated indefinitely)
- No close logic → **`position_monitor.rs`: BSM-based P&L, 50%-credit target, 21-DTE rule, call-side ITM defense**
- Open positions never closed → **force-close long-premium legs each iteration**
- Orders could be double-submitted → **`post_order_safe()` with auto-generated `client_order_id`**
- Options submitted in extended hours → **explicit 09:30–16:00 ET window check**
- Positions opened on expiry day → **DTE=0 gate**
- Concentration per symbol → **`max_positions_per_symbol` cap (default 2)**
- Assignment equity held indefinitely → **`sell_assigned_stock` flag (default true)**
- Negative-momentum short puts → **`min_momentum_short_put` gate (−5%)**
- Debit strategies approved without buying-power check → **pre-flight check for all leg types**

### Backtest / Live Gap (previously: backtest and live used different logic)
- Different risk guards → **shared `src/risk/guards.rs` (`DailyRiskLimits`) used by both**
- Look-ahead bias in Heston Sharpe → **`estimate_rolling_heston_params()` from trailing data only**
- No expiry-close in personality bot → **`EXPIRY_CLOSE` handler closes ≤1-DTE positions at market**
- No way to audit fill vs expected P&L → **`examples/reconcile_backtest_vs_live.rs`**

### Spot Price / Data (previously: one hardcoded provider)
- Only Alpaca prices for recalibration → **configurable `spot_provider`: Alpaca / Yahoo Finance / Finnhub**
- Yahoo Finance options fetch broken → **crumb-based auth fixed**
- Alpaca API field name mismatches → **`serde rename` attrs on Trade/Quote/Snapshot**

### Regime Pipeline (previously: RegimeDetector and StrategyMatcher existed but were never called)
- Regime detection unused → **`RegimePipeline` wired into order pipeline; kill tests 15–16 pass**
- No audit trail for regime at fill time → **`AuditLog` records regime at signal and fill**
- No regime-aware sizing → **vega/delta/theta concentration limits; kill tests 12–14 pass**

### Calibrator (previously: Feller condition could be violated near boundary)
- Feller condition enforced with hard clamp only → **ε-insensitive enforcement; NM polish after CMA-ES**
- No cross-regime stability check → **regime stability test added to kill criteria**

---

## ⚠️ Still Partially Stubbed
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
- Hardcoded data was removed; function loads real data via `PerformanceMatrix::load_from_file()`
- **Status:** ✅ Resolved (Aug 28, 2026) — `models/performance_matrix.json` populated from a real
  `dollarbill backtest --save` run across all 15 symbols; results in `BACKTEST_REPORT.md`.
  Re-run the same command to refresh after any strategy/config change.

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
- `src/alpaca/` — Full options order routing. Idempotent `post_order_safe()`. 13 safety guards.
  Central OCC parser (`occ.rs`) with proptest fuzzing. Mock-HTTP tests for retry/idempotency behavior.
- `src/backtesting/` — Honest P&L. Reg T margin. Shared DailyRiskLimits guards.
- `src/models/` — BSM, Heston, American all correct and well-tested.
- `src/calibration/` — CMA-ES + Heston calibration; ε-insensitive Feller; NM polish; regime stability.
- `src/analysis/portfolio_greeks.rs` — vanna/volga/charm closed-form; kill tests 12–16.
- `src/risk/guards.rs` — shared daily drawdown/trade-cap guards; proptest invariants.
- `src/risk/position_management.rs` — shared `manage_open_positions()` (live bot + backtest); per-symbol concentration cap.
- `src/risk/invariants.rs` — post-fill runtime invariant checker; flattens risk and alerts on violation.
- `src/order_path.rs` — pure order-path pipeline with explicit error variants; documented in `ORDER_PATH.md`.
- `src/market_data/` — configurable spot provider; live options feed; 30-min recalibration loop.
- `src/strategies/matching.rs` — `performance_matrix.json` populated from real backtest output.

### Needs targeted fixes:
- `examples/personality_based_bot.rs` — still uses older inline close logic instead of the shared
  `manage_open_positions()` function used by `live_bot.rs` and backtesting.

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

| Category | Original Audit | April 2026 | July 2026 | September 2026 |
|----------|---------------|------------|-----------|-----------------|
| Fake strategies (sin/random) | 4 | 0 | 0 | 0 |
| Signal variants missing fields | 4 | 0 | 0 | 0 |
| Signal variants returning Err in order routing | 4 | 0 | 0 | 0 |
| Math bugs | 6 | 0 | 0 | 0 |
| Stubbed functions (hardcoded returns) | 5+ | 0 | 0 | 0 |
| Modules with zero dedicated tests | 6 | 0 | 0 | 0 |
| Vaporware config files | 4+ | 0 | 0 | 0 |
| Live bot with no close logic | 1 | 0 | 0 | 0 |
| No regime pipeline in live path | 1 | 0 | 0 | 0 |
| Missing safety guards | ~10 | 0 | 0 | 0 |
| Look-ahead bias in Heston backtest | 1 | 0 | 0 | 0 |
| `performance_matrix.json` unpopulated | — | — | 1 | 0 |

**Net:** All vaporware is gone. `performance_matrix.json` was populated Aug 28, 2026. The
remaining item is bringing `examples/personality_based_bot.rs` onto the shared
`manage_open_positions()` path (see [ROADMAP.md](ROADMAP.md) priority ranking).
