# 🔥 Brutal Honest Review of DollarBill

**Reviewed:** March 2026 — reflects current codebase state after all major bug fixes and strategy rewrites.

**TL;DR:** Solid options trading toolkit. The math is correct, strategies use real signals,
and the Alpaca client routes all option types to OCC-formatted orders. What remains is the
portfolio module needing unit tests and the advanced_classifier having 3 stubbed functions.

---

## ✅ What Actually Works

### 1. Options Pricing Models — 9/10

- **Black-Scholes-Merton**: Correct CDF (fixed extra `* t`), correct call/put Greeks, verified
  against 6 reference values in `tests/verify_cdf.rs`.
- **Heston Model**: Carr-Madan FFT analytical pricer + Monte Carlo with SplitMix64 (64-bit RNG,
  passes BigCrush). Antithetic variates for variance reduction.
- **American Binomial**: Correct binomial tree using `spot` as root (not `strike`).
  ITM exercise boundary correct.
- **Greeks**: All computed correctly. ATM call theta verifies to ≈ -6.41/year at r=5%.
- **Tests**: 180+ model tests — put-call parity, Greeks monotonicity, stress tests,
  property-based (proptest), numerical stability, CDF reference values.

### 2. Trading Strategies — 7/10

- All 6 strategies use real IV/HV inputs — no `SystemTime`, no `sin()`.
- `MomentumStrategy`, `MeanReversionStrategy`, `BreakoutStrategy`,
  `VolatilityArbitrageStrategy`, `CashSecuredPuts`, `VolMeanReversionStrategy`
  all produce deterministic signals.
- All `SignalAction` variants carry explicit fields (`strike`, `days_to_expiry`,
  `center_strike`, `wing_width`) — no information lost between signal and order.
- 23 tests in `test_core_strategies.rs` verify exact signal types for exact inputs.

### 3. Backtesting Engine — 7/10

- 1,485 lines. ITM expiration settles to intrinsic value correctly (was broken).
- Multi-leg spreads use per-day historical vol (was hardcoded at 30%).
- Five slippage models. Reg T margin calculations. Ledger accounting. 7 test files.

### 4. Alpaca Client — 7/10

- Full multi-leg options order support: OCC symbol generation, all `SignalAction` →
  `OptionsOrderRequest` conversions.
- Correctly snaps expiry to nearest Friday.
- 14 unit tests for OCC encoding, signal conversion, and expiry math.

### 5. Data Pipeline — 7/10

- CSV parsing, Yahoo Finance Python scripts, Heston calibration from real option chains.
- Calibrator uses weighted RMSE with bid-ask spread weighting and Feller condition enforcement.

---

## ⚠️ What Was Oversold (Previous State) — Now Fixed

| Claim | Old Reality | Current Reality |
|-------|-------------|-----------------|
| Real strategies | `sin(SystemTime)` fiction | Real IV/HV signals ✓ |
| Multi-leg orders | Not supported | OCC order routing ✓ |
| Backtester is accurate | Expired everything at $0 | Intrinsic settlement ✓ |
| Heston RNG is sound | 32-bit LCG with period 2³² | SplitMix64, BigCrush ✓ |
| BSM prices are correct | CDF off by ~3% | Verified to 5e-5 ✓ |
| American tree uses spot | Was using strike | Fixed ✓ |

---

## ⚠️ What's Still Oversold

### 1. Strategy Matching — 6/10

`matching.rs` `load_performance_data()` is a no-op that correctly documents its own state: populate
via `PerformanceMatrix::load_from_file()` after running real backtests. Structure is solid;
the match (symbol, regime) → recommended strategy routing works. Waiting on data.

### 2. Portfolio Module — 6/10

~2,400 lines of architecture: Greek aggregation, VaR, Kelly sizing, performance attribution,
allocation engine. **38+ unit tests exist** covering sizing, VaR, allocation, performance, and
`PortfolioManager` directly. The gap is no concurrency or edge-case property-based tests.

---

## 🚫 What's Completely Missing (Real Gaps)

1. **Live data feed** — No WebSocket market data. Python scripts fetch static snapshots.
2. **Position persistence** — No database. State lives in-memory per run.
3. **Portfolio-level risk monitoring** — No real-time margin/delta/gamma exposure loop.
4. **CLI** — `main.rs` is a 240-line monolith. No subcommands.
5. **Alpaca money type helpers** — `Account` stores `buying_power` as `String`; no parse methods.

These are gaps, not lies. The README accurately describes what this is and isn't.

---

## 📊 Final Scores (March 2026)

| Component | Rating | Assessment |
|-----------|--------|------------|
| Options Pricing | 9/10 | Correct, tested, production-quality math |
| Greeks | 9/10 | All signs correct, theta verified |
| Heston MC | 7/10 | Good RNG; ~400 lines of path simulator duplication |
| Backtesting | 7/10 | Honest P&L, real vol params |
| Strategies | 7/10 | Real signals, all variants tested |
| Alpaca / Order Routing | 8/10 | Full OCC options support, 14 tests, parse helpers |
| Advanced Classifier | 8/10 | Real S/R strength, real sector relative vol/momentum |
| Strategy Matching | 6/10 | Correct structure, waiting on real backtest data |
| Portfolio Module | 7/10 | 38+ unit tests, complete architecture |
| **OVERALL** | **7.5/10** | Solid working toolkit |

---

## 🎯 The Hard Truth (Updated)

DollarBill is no longer a pricing library wrapped in theater. It's a functional
(if incomplete) options trading toolkit. The math is right. The strategies are deterministic.
The order routing works end-to-end.

The remaining 30% of gaps — portfolio tests, advanced_classifier stubs, and strategy matching
data — are real, isolated, and fixable in 1-2 weeks. They are not rewrites.

**For learning options math in Rust: 9/10.**
**For paper trading options with real signals: 7.5/10.**
**For production: 4/10** — needs live data, persistence, monitoring.
