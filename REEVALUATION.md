# DollarBill — Reevaluation (March 2026)

**Test Suite:** 568 tests — 206 lib + 355 integration + 1 CDF + 6 doc — **all passing, 0 failed**
**Build:** Clean — zero errors, zero warnings in `src/`
**Source:** 16,316 LOC | Tests: 7,905 LOC | Examples: 6,678 LOC

---

## Executive Summary

This reevaluation covers all changes since the original Brutal Review. The project has been
transformed from a pricing library with fake strategies into a functional options trading toolkit.
All six critical math bugs are fixed, all strategy signal types carry real information, and the
Alpaca client routes every signal type to correctly formatted OCC options orders.

**Overall grade: 7.5/10** (up from 5/10 at original review, C+ at intermediate reevaluation)

---

## All Bugs Fixed (Cumulative)

| # | Bug | File | Was | Fixed |
|---|-----|------|-----|-------|
| 1 | Put theta wrong sign | `bs_mod.rs` | θ_put sign flipped | ✓ |
| 2 | CDF extra `* t` (~3% error on all prices) | `bs_mod.rs` | Added spurious `* t` | ✓ |
| 3 | `close_all_positions` expires ITM at $0 | `engine.rs` | No intrinsic calc | ✓ |
| 4 | `optimal_exercise_boundary` anchored at strike | `american.rs` | Should use spot lattice | ✓ |
| 5 | Multi-leg spreads hardcoded at 30% vol | `engine.rs` | Ignored hist vol input | ✓ |
| 6 | Heston Monte Carlo 32-bit LCG RNG | `heston.rs` | Period 2³², fails BigCrush | ✓ |

> Note: The original review's "call theta sign bug" was incorrect — the code matched Hull's
> formula correctly. Tests confirm `call.theta ≈ -6.41/year` for ATM at r=5%.

---

## Strategy Rewrites

All four strategies previously used `(SystemTime::now() * k).sin()` for signal generation.
Every signal was non-reproducible, non-testable, and ignored all market inputs. All four
have been rewritten with real IV/HV logic:

| Strategy | Old Logic | New Logic | Default DTE |
|----------|-----------|-----------|-------------|
| `MomentumStrategy` | `sin(SystemTime)` | `market_iv / historical_vol` ratio vs threshold | 30d |
| `MeanReversionStrategy` | `sin(SystemTime)` | IV z-score = `(market_iv - model_iv) / (hist_vol × 0.25)` | 21d |
| `BreakoutStrategy` | `sin(SystemTime)` | IV expansion + model confirmation ratio | 14–30d |
| `VolatilityArbitrageStrategy` | `sin(SystemTime)` + regime factor | IV premium + regime-weighted edge | 14–30d |
| `CashSecuredPuts` | `strike_pct` (percentage) | Absolute `strike = spot × (1 - otm_pct)` | 30d |

All signal variants now carry explicit fields — no data lost between signal and order:
- `SellStraddle { strike: f64, days_to_expiry: usize }`
- `BuyStraddle { strike: f64, days_to_expiry: usize }`
- `IronButterfly { center_strike: f64, wing_width: f64, days_to_expiry: usize }`
- `CashSecuredPut { strike: f64, days_to_expiry: usize }`

---

## Alpaca Options Order Routing

Previously: `SellStraddle`, `BuyStraddle`, `IronButterfly`, `CashSecuredPut` all returned
`Err("unimplemented")` from `signal_to_legs`.

Now: all convert to proper OCC-formatted `OptionsOrderRequest`:

| SignalAction | Legs | Side |
|-------------|------|------|
| `SellStraddle` | 2 (call + put, same strike) | sell-to-open |
| `BuyStraddle` | 2 (call + put, same strike) | buy-to-open |
| `IronButterfly` | 4 (sell ATM call/put, buy OTM call/put at ±wing) | mixed |
| `CashSecuredPut` | 1 (put at strike) | sell-to-open |

- Expiry snaps to the nearest Friday at or after `today + days_to_expiry`
- OCC format: `"AAPL  250117C00150000"` (6-char symbol, 6-char YYMMDD, C/P, 8-char strike × 1000)
- 14 unit tests verify OCC encoding, signal conversion, and expiry snapping

---

## Module Quality Ratings (March 2026)

| Module | LOC | Rating | Notes |
|--------|-----|--------|-------|
| `models/bs_mod.rs` | 186 | **9/10** | CDF correct, Greeks correct, reference-tested |
| `models/heston_analytical.rs` | 1,044 | **8/10** | Carr-Madan FFT correct |
| `models/heston.rs` | 781 | **7/10** | SplitMix64 sound; 3 complementary path sim fns (antithetic design) |
| `models/american.rs` | 460 | **7/10** | Tree math correct |
| `backtesting/engine.rs` | 1,485 | **7/10** | ITM settlement correct; real vol params |
| `backtesting/margin.rs` | 337 | **8/10** | Reg T rules; 15 tests |
| `strategies/` (all) | ~1,500 | **7/10** | Real signals; all variants tested |
| `alpaca/client.rs` | 752 | **8/10** | Full OCC routing; 14 tests; complete parse helpers |
| `calibration/` | ~800 | **7/10** | Nelder-Mead + Heston calibration tested |
| `analysis/advanced_classifier.rs` | 905 | **8/10** | Real S/R strength + sector relative vol/momentum |
| `portfolio/` | ~2,400 | **7/10** | 38+ unit tests; complete architecture |

---

## Test Coverage (March 2026)

**Total: 568 passing tests across 41+ test files**

| Area | Tests | Coverage Quality |
|------|-------|-----------------|
| BSM / Greeks / CDF | ~60 | Excellent — reference values, property-based, stress |
| Heston (analytical + MC) | ~50 | Good — convergence, stress, pathological params |
| American binomial | ~25 | Good — dividends, Greeks, early exercise |
| Backtesting engine | ~90 | Good — short options, slippage, edge cases, margin |
| Strategies | ~60 | Good — exact input→output for all 4 strategies |
| Alpaca / order routing | 14 | Moderate — OCC encoding, signals, expiry |
| Calibration | ~15 | Good — Nelder-Mead convergence |
| Analysis / classifier | ~30 | Moderate — 3 stub functions not covered |
| Portfolio | ~20 | Weak — indirect / integration paths only |

---

## What Still Needs Work

**Priority 1 (data-driven, needs backtest runs):**
- Populate `matching.rs` with real backtest results via `PerformanceMatrix::add_result()`

**Priority 2 (quality / cleanup):**
- Refactor `main.rs` into `clap` subcommands (currently 240-line monolith)
- Remove examples dead code suppression by actually wiring unused config fields

**Priority 3 (architecture):**
- Live WebSocket data feed (Tokio + Alpaca streaming)
- Position persistence (SQLite via sqlx)
- CLI refactor (`clap` subcommands)

---

## What's Solidly Good

- **Core math is correct and verified.** BSM, Heston, American binomial all produce accurate
  results, validated against independent reference values in dedicated test files.
- **All strategies are real.** No wall-clock time, no random seeds. Every signal is a
  deterministic function of IV/HV inputs.
- **Full options order routing.** Every `SignalAction` variant converts to an OCC options order.
  No more returning `Err` for half the signal types.
- **Backtesting is honest.** ITM expiration settles at intrinsic value. Vol params are
  per-symbol, per-day from real historical data.
- **Test suite is substantial.** 568 tests, property-based coverage (proptest), stress tests,
  put-call parity enforcement, numerical stability.
- **Zero build warnings** in `src/`. Compiler is clean.
