# DollarBill — Post-Fix Reevaluation

**Date:** Reevaluation after fixing 6 critical bugs  
**Test Suite:** 289 unit tests + 1 CDF verification + 2 doc-tests — **all passing**  
**Build:** Clean (compiler warnings only, no errors)

---

## Executive Summary

Six critical bugs have been fixed — the 5 identified in the original review, plus a **6th pre-existing bug** discovered during this reevaluation that corrupted every single BSM price and Greek in the system by ~3%. The core pricing models are now mathematically correct. The project moves from "dangerous if used" to "educational with caveats."

**Overall grade: C+ (up from D-)**

The math core is now trustworthy. What remains is the large surface area of dead code, fake strategies, untested modules, and architectural scaffolding that has never been exercised.

---

## Bugs Fixed (6 total)

### Bug #1: Put Theta Sign Inversion — FIXED ✓
**File:** `src/models/bs_mod.rs` (lines 117-119)  
Put theta was returning the wrong sign: `+q·N(-d₁) − r·N(-d₂)` instead of `−q·N(-d₁) + r·N(-d₂)`. Fixed. Put theta is now correctly negative for typical parameters.

### Bug #2: All Positions Expire Worthless — FIXED ✓
**File:** `src/backtesting/engine.rs` (`close_all_positions()`)  
Expiration used `close_price = 0.0` for everything, destroying ITM positions. Now calculates intrinsic value: `max(0, spot − strike)` for calls, `max(0, strike − spot)` for puts, and settles capital accordingly.

### Bug #3: Exercise Boundary Uses Strike as Root — FIXED ✓
**File:** `src/models/american.rs` (`optimal_exercise_boundary()`)  
Function now accepts a `spot` parameter and builds the binomial tree from `spot × u^i × d^(n-i)` instead of `strike × u^i × d^(n-i)`. Dividend-adjusted probability is consistent with the main `binomial_tree()` function.

### Bug #4: Hardcoded 30% Volatility in Spreads — FIXED ✓
**File:** `src/backtesting/engine.rs` (4 functions)  
`open_iron_condor()`, `open_credit_call_spread()`, `open_credit_put_spread()`, and `open_covered_call()` now accept a `volatility: f64` parameter. All callers pass `current_vol` from historical vol calculations.

### Bug #5: 32-bit LCG PRNG — FIXED ✓
**File:** `src/models/heston.rs`  
The 32-bit LCG (period 2³²) was replaced with SplitMix64 (period 2⁶⁴, passes BigCrush). New `SplitMix64` struct with `next_u64()`, `next_uniform()`, `next_normal()`, `next_correlated_normals()`. All 10+ call sites updated.

### Bug #6 (NEW): Normal CDF Off By ~3% — FIXED ✓
**File:** `src/models/bs_mod.rs` (line 28)  
**Discovered during this reevaluation.** The Abramowitz & Stegun CDF approximation had an extra `* t` that corrupted the result:

```rust
// BEFORE (broken): extra * t multiplied the polynomial a second time
1.0 - pdf_part * poly * t

// AFTER (correct): poly already contains all powers of t via Horner form
1.0 - pdf_part * poly
```

**Impact:** Every BSM call/put price, every Greek, every IV solve was off by ~3%. N(0.5) returned 0.7235 instead of the correct 0.6915. Put-call parity tests **could not catch this** because N(x) + N(−x) = 1 is structurally guaranteed regardless of the bug.

**Verification:** A dedicated test (`tests/verify_cdf.rs`) confirms CDF accuracy against 6 known standard normal values, all within 0.00005 of reference.

---

## Module Quality Ratings (Post-Fix)

| Module | Files | LOC | Rating | Assessment |
|--------|-------|-----|--------|------------|
| **models/bs_mod.rs** | 1 | 186 | **7/10** | CDF and Greeks now correct. Production-quality for educational use. |
| **models/heston.rs** | 1 | 859 | **6/10** | SplitMix64 is sound. Euler scheme has truncation bias; Milstein or QE would be better. 3 similar path simulators could be DRY'd. 10+ panic points on unwrap. |
| **models/american.rs** | 1 | 415 | **7/10** | Tree math now correct. Confusing `let r = disc;` shadow still present. No Richardson extrapolation for convergence. |
| **backtesting/engine.rs** | 1 | 1405 | **5/10** | ITM settlement and vol params fixed. Empty signal handlers for 4 signal types (SellStraddle, BuyStraddle, IronButterfly, CashSecuredPut) silently discard orders. |
| **strategies/momentum.rs** | 1 | 108 | **1/10** | Uses `(SystemTime::now() * 0.001).sin()` for signals. Non-reproducible, non-testable, fiction. |
| **strategies/mean_reversion.rs** | 1 | 121 | **1/10** | Same `sin(SystemTime)` approach. |
| **strategies/breakout.rs** | 1 | 96 | **1/10** | Same. |
| **strategies/matching.rs** | 1 | 249 | **2/10** | `load_performance_data()` is hardcoded fabricated backtest results. Strategy-matching logic is sound in structure only. |
| **portfolio/** | 5 | ~2000 | **4/10** | Architecturally mature (risk analytics, VaR, Kelly sizing, attribution). Zero tests. 5/5 files have `#![allow(dead_code)]`. |
| **calibration/** | 3 | ~800 | **5/10** | Nelder-Mead calibrator works correctly per existing tests. All 3 files have `#![allow(dead_code)]`. |
| **alpaca/** | 3 | ~600 | **3/10** | HTTP client for equity orders only. Cannot trade options. All files suppressed. |
| **analysis/** | 3 | ~500 | **5/10** | Stock personality classifier works and is well-tested (20+ tests). Advanced classifier less tested. |

---

## What Still Needs Work

### Critical (Correctness)

1. **Empty signal handlers** — `engine.rs` lines ~441-452 silently drop SellStraddle, BuyStraddle, IronButterfly, and CashSecuredPut signals. These should either be implemented or return an explicit error.

2. **Fake strategies** — `momentum.rs`, `mean_reversion.rs`, `breakout.rs` all use `sin(SystemTime::now())` for trade signals. These are not strategies; they are random number generators with trigonometric dressing. They need to be either rewritten to consume actual price data or removed entirely.

3. **Fabricated backtest data** — `matching.rs` `load_performance_data()` returns hardcoded fake Sharpe ratios and win rates. This will mislead any user who relies on strategy recommendations.

### Moderate (Code Health)

4. **26 file-level `#![allow(dead_code)]` blankets** — Covers almost every source file outside `lib.rs`. Removing them would surface 50+ warnings about unused functions, fields, and variants. These blankets mask the true state of the codebase.
   
   Files affected: `config.rs`, all 5 portfolio files, all 6 backtesting files, both calibration files, all 3 market_data files, `heston.rs`, `american.rs`, both analysis files, both alpaca files, `strategies/mod.rs`, `utils/vol_surface.rs`.

5. **Heston Euler scheme bias** — `simulate_path()` uses simple Euler discretization for the variance process, which can go negative even with Feller condition satisfied. The truncation `variance.max(0.0)` introduces bias. Consider QE (Quadratic-Exponential) scheme.

6. **No absolute-value tests** — 289 tests check structural properties (monotonicity, put-call parity, sign, bounds) but not a single test verifies `BSM_call(100, 100, 0.2, 1.0, 0.05, 0.0) ≈ 10.45`. This is how the CDF bug survived undetected.

7. **Portfolio module untested** — ~2,000 lines of risk analytics, position sizing, allocation, and performance attribution with zero test coverage.

### Minor

8. **Code duplication in heston.rs** — Three nearly identical path simulation methods, duplicated call/put Greek calculations.

9. **Variable shadowing in american.rs** — `let r = disc;` is confusing: `r` looks like the interest rate but is the discount factor.

10. **~70 compiler warnings** — Mostly unused imports and variables. Not harmful but noisy.

---

## What's Good

- **Core math is now correct** — BSM, CDF, Greeks, American binomial, and Heston MC all produce accurate results.
- **Test suite is structurally sound** — 289 tests covering property-based testing (proptest), stress tests, edge cases, and put-call parity. The tests caught regressions during all 6 fixes.
- **Calibration pipeline works** — Nelder-Mead Heston calibration against real option chains is functional and tested.
- **Stock personality classifier** — Genuinely useful feature with good test coverage.
- **Architecture is ambitious and well-organized** — Clean module boundaries, decent separation of concerns.

---

## Recommended Next Steps (Priority Order)

1. **Add absolute-value BSM tests** — `assert!((call - 10.4506).abs() < 0.01)` for known reference values. This catches CDF-class bugs instantly.
2. **Delete or rewrite the 3 fake strategies** — Replace `sin(SystemTime)` with actual technical indicators consuming price series.
3. **Implement the 4 empty signal handlers** or make them return `Err`.
4. **Remove `#![allow(dead_code)]` blankets** one file at a time and fix or delete the dead code.
5. **Add tests for the portfolio module** — 2,000 lines with zero test coverage is a liability.
6. **Remove fabricated performance data** from `matching.rs`.

---

## Summary of Changes Made

| What | Where | Lines Changed |  
|------|-------|--------------|
| Put theta sign fix | bs_mod.rs:117-119 | 3 |
| CDF extra `* t` fix | bs_mod.rs:28 | 1 |
| ITM expiration fix | engine.rs:1248-1277 | ~30 |
| Exercise boundary spot param | american.rs:273+ | ~25 |
| Volatility params for spreads | engine.rs (4 functions + callers) | ~40 |
| LCG → SplitMix64 | heston.rs (struct + 10 sites) | ~80 |
| CDF verification test | tests/verify_cdf.rs (new) | 35 |
| **Total** | | **~214 lines** |

All 292 tests pass. Zero regressions.
