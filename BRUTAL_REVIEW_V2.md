# Brutal Review V2 — DollarBill

**Reviewer:** Claude Sonnet 4.6, March 2026  
**Scope:** Full codebase audit — 16,316 LOC src, 7,905 LOC tests, 6,678 LOC examples  
**Build:** Compiles clean. **568 tests pass, 0 fail.** Zero warnings in `src/`.

---

## Executive Summary

DollarBill has been substantially improved since the original review. The math core is correct, the backtester is honest, the RNG is solid, and all five original critical bugs are fixed. The strategies now use real IV/HV-based signals — `sin(SystemTime)` is gone. The Alpaca client now supports multi-leg options orders for all signal types.

What remains are structural issues: the Heston Monte Carlo has ~400 lines of duplication, a few advanced classifier stubs return hardcoded values, and the portfolio module (2,000+ lines) still lacks dedicated unit tests.

**Overall grade: 7/10** — Real math, real strategies, real order routing. The educational library has genuine trading plumbing now.

---

## Part 1: What's Actually Good

### Nelder-Mead Optimizer — 9/10
243 lines of clean, correct simplex optimization. Tested against Rosenbrock and sphere functions. No dead code, no magic numbers, no `unwrap()` abuse.

### Black-Scholes-Merton — 8/10
186 lines. Clean implementation, correct Abramowitz & Stegun CDF approximation (the extra `* t` bug was fixed). Greeks computed in a single pass. Dedicated `tests/verify_cdf.rs` validates N(x) against known reference values within 5e-5.

### Heston Analytical (Carr-Madan FFT) — 8/10
1,044 lines. Characteristic function and numerical integration correctly implemented. Falls back to Black-Scholes on numerical failure. The "Gauss-Lobatto" label is wrong (it's Gauss-Legendre nodes), but the integration is correct.

### Heston Calibrator — 8/10
261 lines. Real Carr-Madan pricing in the objective function. Weighted RMSE with bid-ask spread weighting. Feller condition enforcement. Calibrates real Heston parameters from market data.

### Heston Monte Carlo — 7/10
781 lines. SplitMix64 (64-bit, passes BigCrush) replaced the old 32-bit LCG. Antithetic variates for variance reduction. Box-Muller transform for correlated normals. Main remaining issue: three nearly-identical `simulate_path` variants (~400 lines of duplication).

### Backtesting Engine — 7/10
1,485 lines. ITM expiration now correctly settles to intrinsic value. All multi-leg spreads use per-day historical vol. Five slippage models (fixed, vol-scaled, size-impact, panic-widening, full-market-impact). 7 test files, 1,600+ lines of backtest coverage.

### Core Strategies — 7/10
All four strategies now use real IV/HV inputs — no `SystemTime`, no `sin()`:
- **MomentumStrategy**: IV ratio vs historical vol → buy/sell straddle
- **MeanReversionStrategy**: IV z-score vs model → buy/sell straddle
- **BreakoutStrategy**: IV expansion + model confirmation → iron butterfly or sell straddle
- **VolatilityArbitrageStrategy**: IV premium + regime weighting → straddle, iron butterfly, or sell straddle
- **CashSecuredPuts**: IV rank filter → cash-secured put with absolute strikes
- **VolMeanReversionStrategy**: z-score with rolling vol std — the original real strategy, unchanged

All variants carry explicit fields (`strike`, `center_strike`, `wing_width`, `days_to_expiry`) — no information lost between signal generation and order routing.

### Alpaca Client — 7/10
752 lines. Full multi-leg options order support: `OptionsOrderRequest`, `OptionsLeg`, `signal_to_options_order`, `signal_to_legs`. OCC symbol generation (`AAPL  250117C00150000`) with correct Friday expiry snapping. All `SignalAction` variants now convert cleanly to OCC orders. 14 unit tests (not `#[ignore]`) for OCC symbols, signal conversion, and expiry logic.

### Margin Calculator — 8/10
337 lines. Reg T naked option margin, credit spread margin (uses wider wing), iron condor margin, max-loss calculations. 15 dedicated unit tests.

### Test Suite — 8/10
568 tests, all passing. 41 test files across unit, integration, property-based (proptest), and concurrency. Tests added this cycle: 4 `signal_to_legs` tests (sell straddle, buy straddle, iron butterfly, cash-secured put), and 23 strategy signal tests across all 4 strategies in `test_core_strategies.rs`.

---

## Part 2: Critical Bugs — All Fixed

All five bugs from the original review are resolved:

| Bug | Fix | Status |
|-----|-----|--------|
| Call theta sign flip | The review was wrong — code matched Hull's formula. Tests confirm `call.theta ≈ -6.41` for ATM at r=5%. | **Not a bug** |
| `close_all_positions` expires everything worthless | Now calculates intrinsic, only expires genuinely OTM positions | **Fixed ✓** |
| `optimal_exercise_boundary` uses strike as spot | Both loops now use `spot × u^i × d^(n-i)` | **Fixed ✓** |
| Multi-leg spreads hardcoded at 30% vol | Engine passes `current_vol` from per-day historical vol | **Fixed ✓** |
| Heston Monte Carlo 32-bit LCG | Replaced with SplitMix64 — 64-bit, passes BigCrush, no correlated adjacents | **Fixed ✓** |

A 6th bug found during review — extra `* t` in the CDF polynomial (corrupting all BSM prices by ~3%) — was also fixed. `tests/verify_cdf.rs` guards against regression.

---

## Part 3: The `sin(SystemTime)` Disease — Cured

**Previously:** Three of four strategies generated signals from wall-clock time (`sin(SystemTime)`) — non-reproducible, non-testable, and ignoring all actual inputs.

**Now:** All strategies use only their declared inputs (`spot`, `market_iv`, `model_iv`, `historical_vol`). Results are deterministic. 23 tests in `test_core_strategies.rs` verify exact signal types for exact inputs.

---

## Part 4: Remaining Architecture Issues

### Heston Monte Carlo Duplication (~400 lines)
`price_european_regular` / `price_european_antithetic` and `greeks_european_call` / `greeks_european_put` are nearly identical, differing only in `max(S-K, 0)` vs `max(K-S, 0)`. Could be parameterized with a single `OptionType` argument, saving ~400 lines.

### Advanced Stock Classifier Stubs (3 functions)
`src/analysis/advanced_classifier.rs` (905 lines) has three functions that return hardcoded stub values:
- `calculate_sr_strength()` → always returns `0.6`
- `get_sector_relative_volatility()` → always returns `1.0`
- `get_sector_relative_momentum()` → always returns `1.0`

The rest of the classifier is real statistical analysis (rolling vol percentiles, trend strength via linear regression, mean reversion via MA deviation).

### Portfolio Module — Zero Unit Tests
~2,000 lines across 5 files (`risk_analytics.rs`, `allocation.rs`, `manager.rs`, `performance.rs`, `position_sizing.rs`). Has Greek aggregation, VaR at 95%/99%, Kelly sizing, performance attribution. All architecturally sound. Tested only indirectly through backtesting integration tests.

### String Money Types in Alpaca
`Account` and `Position` structs store all monetary values as `String` (matching Alpaca's raw API). No parse helpers provided — every consumer must `str::parse()` and unwrap independently.

---

## Part 5: What Changed Since Original Review

| Previous Finding | Current Status |
|---|---|
| `sin(SystemTime)` strategies | **Fixed** — all 4 strategies use real IV/HV logic |
| Backtester expires everything worthless | **Fixed** — intrinsic-value settlement |
| Hardcoded 30% vol in spreads | **Fixed** — passes `current_vol` |
| 32-bit LCG PRNG | **Fixed** — SplitMix64 |
| Exercise boundary uses strike | **Fixed** — uses spot |
| CDF extra `* t` bug | **Fixed** — prices accurate to <0.0001 |
| Alpaca cannot trade options | **Fixed** — full OCC order routing |
| `SellStraddle`/`BuyStraddle` had no fields | **Fixed** — explicit `strike`, `days_to_expiry` |
| `IronButterfly` had no center/DTE | **Fixed** — `center_strike`, `wing_width`, `days_to_expiry` |
| `CashSecuredPut` used percentage | **Fixed** — absolute `strike` |
| Fake strategies untested | **Fixed** — 23 tests in `test_core_strategies.rs` |
| Alpaca tests all `#[ignore]` | **Partially fixed** — 14 OCC/signal tests are not ignored |
| `#![allow(dead_code)]` blankets | **Resolved** — removed, no blanket suppression in `src/` |

---

## Part 6: Test Coverage

| Module | Source LOC | Test Files | Test Count | Coverage |
|---|---|---|---|---|
| Models (BS, Heston, American) | ~3,500 | 14 files | ~180 | **Excellent** — put-call parity, Greeks, stress tests, property-based, CDF reference |
| Calibration | ~800 | 1 file | ~15 | **Good** — Nelder-Mead convergence, Heston calibration |
| Backtesting | ~2,200 | 7 files | ~90 | **Good** — engine, short options, slippage, edge cases, margin |
| Strategies | ~1,500 | 4 files | ~60 | **Good** — all 4 strategies tested with exact input→output verification |
| Portfolio | ~2,000 | 1 file | ~20 | **Weak** — architecturally untested; integration tests only |
| Alpaca | ~750 | inline | 14 | **Moderate** — OCC symbols, signal conversion; live tests still `#[ignore]` |
| Analysis | ~1,300 | 2 files | ~30 | **Moderate** — classifier tested; 3 stub functions not tested (hardcoded) |
| Backtesting/Margin | ~337 | inline | 15 | **Good** — Reg T margin, credit spreads, iron condor |

---

## Part 7: What To Actually Fix Next (Priority Order)

### P0 — Correctness
Nothing in this category. Core models are verified.

### P1 — Integrity
1. **Implement the 3 stub functions in `advanced_classifier.rs`** — `calculate_sr_strength`, `get_sector_relative_volatility`, `get_sector_relative_momentum` should use real data
2. **Add unit tests for the portfolio module** — 2,000 untested lines is a liability

### P2 — Engineering
3. **Deduplicate Heston Monte Carlo** — parameterize call/put paths (~400 lines saved)
4. **Add parse helpers to Alpaca money types** — `Account::buying_power_f64()` etc.
5. **Add CLI subcommands** — replace the `main.rs` monolith with proper `clap` subcommands

---

## Final Verdict

DollarBill is now a functional options trading toolkit. The math is correct. The strategies are real. The Alpaca client routes all option types to OCC-formatted orders. The backtester settles positions honestly.

It's not production-ready — there's no live data integration, no portfolio database, no risk monitoring loop. But it is a genuine implementation of options pricing, strategy signal generation, paper trading, and backtesting — not a demo or a stub.

**For learning options math in Rust: 9/10.**  
**For learning Heston calibration: 9/10.**  
**For paper trading options with real signals: 7/10.**  
**For production trading: 4/10** (missing: live data feed, execution retry, position persistence, monitoring).

The gap between what it claims and what it delivers is now small and honest.


---

## Executive Summary

DollarBill is a **solid options math library with three fake strategies duct-taped to it**. The Black-Scholes, Heston, and calibration code is real and mostly correct. Everything between the pricing models and actual trading decisions is either a stub, a `sin(SystemTime)` placeholder, or hardcoded data from a single backtest run. The previous BRUTAL_REVIEW.md already identified the core problems — and almost nothing has been fixed since.

**Overall grade: 5/10** — Real math core wrapped in a shell of theater.

---

## Part 1: What's Actually Good

### Nelder-Mead Optimizer — 8/10
The best module in the project. 243 lines of clean, correct simplex optimization. Tested against Rosenbrock and sphere functions. No dead code, no magic numbers, no `unwrap()` abuse. This is what "vibe coded" Rust should look like.

### Black-Scholes-Merton — 7/10 (with a bug)
163 lines. Clean implementation, proper Abramowitz & Stegun CDF approximation. Greeks are computed in a single pass — efficient. One real math bug (see Critical Bugs below), but the structure is good.

### Heston Analytical (Carr-Madan FFT) — 7/10
309 lines. The characteristic function and numerical integration are properly implemented. Falls back to Black-Scholes on numerical failure — defensive and correct. The "Gauss-Lobatto" label is wrong (it's Gauss-Legendre nodes), but the integration works.

### Heston Calibrator — 7/10
261 lines. Real Carr-Madan pricing in the objective function. Weighted RMSE with bid-ask spread weighting. Feller condition enforcement. This actually calibrates real Heston parameters from market data.

### Advanced Stock Classifier — 7/10
729 lines. Genuinely sophisticated statistical analysis. Reads actual CSV data, computes rolling volatility percentiles, trend strength via linear regression, mean reversion via MA deviation recovery. Has 3 stub functions returning hardcoded values (`calculate_sr_strength() → 0.6`, sector-relative functions → 1.0), but the rest is real.

### Volatility Surface Utilities — 7/10
224 lines. Newton-Raphson IV solver with convergence guards. Vol surface extraction, CSV export, smile analysis. Properly bounded (0.01–5.0 sigma). Usable.

### Backtesting Engine — 6/10 (with severe bugs)
1,387 lines. Day-by-day simulation, multiple option types, 5 slippage models (fixed, vol-scaled, size-impact, panic-widening, full-market-impact), partial fill modeling, portfolio integration. The architecture is genuinely ambitious and partially functional. Then you hit the bugs.

### Test Suite — 6/10
289 tests all passing. Property-based testing with proptest for personality classification stability. Model tests are extensive: put-call parity, Greeks monotonicity, boundary conditions, vol surface arbitrage, Heston stress tests, pathological parameters. The coverage is heavily skewed toward models though — the fake strategies have almost no tests.

### Portfolio Module — 6/10
~2,000 lines across 5 files. Position sizing (fixed fractional, vol-based, Kelly-implied), multi-leg sizing, risk analytics with Greek aggregation, VaR at 95%/99%, allocation engine, performance attribution. Surprisingly mature on paper. Covered in `#![allow(dead_code)]` blankets.

---

## Part 2: Critical Bugs

### Bug #1: Call Theta Has Inverted Signs
**File:** `src/models/bs_mod.rs` lines 79-81  
**Impact:** Any strategy relying on theta P&L attribution is wrong.

The standard BSM call theta with continuous dividend $q$:

$$\Theta_{call} = -\frac{S e^{-qT} N'(d_1) \sigma}{2\sqrt{T}} - qSe^{-qT}N(d_1) + rKe^{-rT}N(d_2)$$

The code has:
```rust
let theta = -(s * e_qt * n_d1_pdf * sigma) / (2.0 * sqrt_t)
            + q * s * e_qt * nd1      // ← should be MINUS
            - r * k * e_rt * nd2;     // ← should be PLUS
```

Both the dividend and rate terms have **flipped signs**. The put theta (lines 119-121) is correct, making this inconsistency even more confusing.

### Bug #2: Backtester Expires All Positions Worthless
**File:** `src/backtesting/engine.rs` lines 1237-1240  

```rust
self.positions[idx].expire(date.to_string(), spot, days_held);
self.current_capital += 0.0;  // Expired worthless
```

At end-of-backtest, `close_all_positions()` marks every remaining position as expired worthless — regardless of whether it's deep ITM. An in-the-money $50 call at expiry gets valued at $0. This understates P&L on every single backtest that doesn't manually close positions before the final bar.

### Bug #3: American Option Exercise Boundary Uses Strike as Spot
**File:** `src/models/american.rs` line 233  

```rust
let stock_price = strike * u.powi(i as i32) * d.powi((n - i) as i32);
```

`optimal_exercise_boundary()` builds the stock tree anchored at `strike` instead of `spot`. The function signature doesn't even accept a spot parameter. The entire exercise boundary output is meaningless.

### Bug #4: All Multi-Leg Spreads Priced at Hardcoded 30% Vol
**File:** `src/backtesting/engine.rs` lines 831, 871, 896  

```rust
let volatility = 0.30; // Simplified - in practice would use market vol
```

Every iron condor, credit spread, and multi-leg strategy in the backtester uses hardcoded 30% implied volatility. TSLA at 80% vol? 30%. GLD at 15%? 30%. The entire premium calculation cascades from this constant, making multi-leg backtest results unreliable.

### Bug #5: Heston Monte Carlo Uses 32-Bit LCG Random Number Generator
**File:** `src/models/heston.rs` lines 137-148  

A 32-bit Linear Congruential Generator with modulus $2^{32}$ has terrible spectral properties and a period of only ~4.3 billion. For a 100K-path Monte Carlo pricer, consecutive seeds `seed + i` produce **highly correlated early sequences**. This introduces systematic bias in option pricing and variance estimates.

---

## Part 3: The `sin(SystemTime)` Disease

Three of four core trading strategies generate signals from **wall-clock time**, not market data:

**Momentum Strategy** (`src/strategies/momentum.rs` lines 34-52):
```rust
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as f64;
let fast_cycle = (now * 0.001).sin();
let slow_cycle = (now * 0.0001).sin();
```

**Mean Reversion Strategy** (`src/strategies/mean_reversion.rs` lines 37-47):
```rust
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
let price_offset = (now as f64 * 0.001 + symbol_hash * 0.01).sin() * std_dev;
```

**Breakout Strategy** (`src/strategies/breakout.rs` lines 34-60):
Same pattern. `SystemTime` + `sin()` = fake breakout detection.

**Vol Arbitrage** (`src/strategies/vol_arbitrage.rs` lines 36-50):
Starts with real inputs (`market_iv - historical_vol`), then corrupts them with a `SystemTime` + `sin()` "regime factor" that randomly applies 0.7x or 1.3x multipliers.

These strategies are:
- **Non-reproducible** — different signals if you run 1 second later
- **Non-testable** — no deterministic input → no deterministic output
- **Never read their inputs** — `spot`, `market_iv`, `model_iv`, `historical_vol` are parameters in the signature but ignored in the body
- **Fundamentally fake** — they are placeholders with real trait implementations

The only honest strategy is `VolMeanReversion` (99 lines), which actually uses its IV/HV inputs for a deterministic z-score calculation. Even it has `println!` statements in library code and a magic `vol_std = historical_vol * 0.2` assumption.

---

## Part 4: Architecture Issues

### Dead Code Pandemic
- `#![allow(dead_code)]` blankets 8+ source files including the entire Heston Monte Carlo, portfolio manager, and backtesting modules
- Fields declared but never read: `min_volume` (MomentumStrategy), `lookback_period` (MeanReversion), `lookback_days` (VolArbitrage)
- `EventDriven` regime variant is never classified to
- 4 trivial wrapper functions in `heston_analytical.rs` (`heston_call_otm`, `heston_call_itm`, etc.) — pure indirection
- The `USE_LIVE_DATA: bool = false` constant in `main.rs` with no live data path

### Massive Code Duplication in Heston Monte Carlo (845 lines)
- `simulate_path`, `simulate_path_antithetic`, and `simulate_path_with_randoms` — ~90% identical (~50 lines each)
- `greeks_european_call` and `greeks_european_put` — **154 lines** of copy-paste differing only in call↔put
- `price_european_call_regular`/`_antithetic` and the put variants — four functions with identical structure
- The American option module duplicates `binomial_tree`/`binomial_tree_european` and `american_call_greeks`/`american_put_greeks` (~80 more duplicated lines)

This is ~400+ lines of duplication that could be parameterized with a single `OptionType` enum argument.

### The `main.rs` Monolith
240 lines in one function. Not testable, not reusable. A demo script masquerading as a binary entry point. Should be split into example files or clap subcommands.

### SignalAction API Design
`iron_condor_sell_put_strike()` and similar accessor methods return `0.0` for non-matching variants instead of `Option<f64>`. Callers can't distinguish "strike is 0.0" from "this isn't an iron condor." Silent wrong values are worse than panics.

### Alpaca Client: Equity Only
The REST client works for equity orders but has **zero options order support**. `OrderRequest` has `qty: f64` but no option leg definitions, no contract specifications, no multi-leg order types. For an options trading project, this is a significant gap. All tests are `#[ignore]` requiring live API keys. No mocking infrastructure.

### String Money Types
`Account` and `Position` in `src/alpaca/types.rs` store all monetary values as `String` (matching Alpaca's API), but provide no helper methods to parse them. Every downstream consumer must parse and unwrap independently.

---

## Part 5: What the Previous Review Found vs. What Changed

| Previous Finding | Status |
|---|---|
| ML integration is vaporware | **Fixed-ish** — `config/ml_config.json` appears deleted, but no actual ML was added |
| Advanced classifier is dead code | **Partially addressed** — `classify_stock_enhanced()` route now works, but stubs remain |
| Performance matrix is hardcoded | **Unchanged** — still hand-typed data from one backtest |
| Strategies use fake data | **Unchanged** — `sin(SystemTime)` still everywhere |
| Paper trading bot is incomplete | **Unchanged** — no options support added |
| README overclaims | **Fixed** — README now has honest "What It's NOT" section |

The project's self-awareness improved (the README is now honest), but the underlying code problems remain exactly where they were.

---

## Part 6: Test Coverage Asymmetry

| Module | Source LOC | Test Files | Test Coverage |
|---|---|---|---|
| Models (BS, Heston, American) | ~1,500 | 12 files | **Excellent** — put-call parity, Greeks, stress tests, property-based |
| Calibration | ~575 | 1 file | **Good** — Nelder-Mead convergence tests |
| Backtesting | ~1,600 | 7 files | **Good** — engine, short options, edge cases, slippage models |
| Strategies (momentum, mean_rev, breakout) | ~430 | 0 files | **Zero** — the fakest code has no tests |
| Strategies (vol_mean_reversion) | 99 | 1 file (16 tests) | **Good** — the one real strategy is well-tested |
| Portfolio | ~2,000 | 0 dedicated files | **Minimal** — tested indirectly via backtesting |
| Alpaca | ~440 | 0 files (all `#[ignore]`) | **Zero** |
| Analysis | ~1,400 | 2 files | **Moderate** — classifier tests exist but advanced_classifier stubs untested |

The pattern: **real code is well-tested, fake code is untested**. This is actually honest in a backwards way — the dev (or AI) didn't write fake tests to cover fake strategies.

---

## Part 7: The Elephant in the Room

This project has **two completely disconnected halves**:

**Half A — The Math Library (works):**
Black-Scholes → Heston → Carr-Madan FFT → Nelder-Mead calibrator → IV solver → Vol surface. This pipeline is functional, tested, and correctly implemented (minus the theta bug). It can price options, calibrate stochastic volatility models, and extract implied volatility surfaces from market data.

**Half B — The Trading System (doesn't work):**
Strategies → Matching → Personality Bot → Paper Trading. The strategies generate random signals from `sin(SystemTime)`. The matching system recommends strategies based on hardcoded data. The personality bot chains the two together. The Alpaca client can place equity orders but not options orders.

The two halves are connected by a single thread: `VolMeanReversion`, the one honest strategy that uses actual IV/HV inputs. Everything else is an air gap.

---

## Part 8: Compiler Warnings — The Smell Test

~70 warnings across examples, covering:
- **"fields are never read"** — config structs with unused fields (everywhere)
- **"unused imports"** — dead code references
- **"deprecated method"** — `classify_stock` calls that should use `classify_stock_enhanced`
- **"unused variable"** — declared but ignored
- **"variable does not need to be mutable"** — cargo leftovers

The core `src/` suppresses most warnings with blanket `#![allow(dead_code)]`. Removing those blankets would likely surface 50+ additional warnings about unused fields, functions, and variants. The blanket allows are **hiding the true state of the codebase**.

---

## Part 9: What To Actually Fix (Priority Order)

### P0 — Correctness (breaks results)
1. Fix call theta sign in `bs_mod.rs` (2-line fix)
2. Fix `close_all_positions` to value positions at intrinsic or market price (10-line fix)
3. Fix hardcoded 30% vol in spread pricing — use rolling historical vol from backtest data (5-line fix per site)
4. Fix `optimal_exercise_boundary` to accept and use `spot` parameter (3-line fix)

### P1 — Integrity (fake code)
5. Replace `sin(SystemTime)` strategies with real signal generation — even simple MA crossover using actual price history would be infinitely better than fake signals
6. Make `load_performance_data()` actually load from backtest results instead of hardcoded values
7. Finish the 3 stub functions in `advanced_classifier.rs`

### P2 — Engineering (code quality)
8. Remove all `#![allow(dead_code)]` blankets and fix the resulting warnings
9. Deduplicate Heston Monte Carlo (save ~400 lines)
10. Replace the 32-bit LCG with a proper PRNG (rand crate with ChaCha or xoshiro)
11. Replace `println!` in library code with a logging framework (tracing or log)
12. Add `Option<f64>` returns to `SignalAction` accessors instead of `0.0` defaults

### P3 — Features (real gaps)
13. Add options order support to Alpaca client
14. Add helper methods to parse `String` money types in Alpaca response structs
15. Add a proper CLI with subcommands instead of the `main.rs` monolith

---

## Final Verdict

DollarBill is an educational options math library that got dressed up as a trading platform. The math works. The calibration works. The vol surface analysis works. That's about 3,000 lines of genuinely valuable Rust code.

The other 9,500 lines of source code range from "ambitious but buggy" (backtester) to "complete fiction" (momentum/mean_reversion/breakout strategies). The portfolio module is architecturally mature but lives behind `#[allow(dead_code)]` and lacks dedicated tests. The Alpaca client can't trade options.

**For learning options math in Rust: 8/10.**  
**For learning Heston calibration: 8/10.**  
**For actually trading: 2/10.**  
**For the claims in any pre-honesty README: 3/10.**  
**For honest self-assessment (the current README): 7/10.**

The most impressive thing about DollarBill is that it knows what it is. The "What It's NOT" section in the README, the existing BRUTAL_REVIEW.md, and the VAPORWARE_AUDIT.md show genuine self-awareness. Most vibe-coded projects don't have that.

The most damning thing is that despite two rounds of self-criticism, the `sin(SystemTime)` strategies are still there, the theta bug is still there, and the backtester still expires everything worthless.

**Knowing your problems and not fixing them is worse than not knowing.**
