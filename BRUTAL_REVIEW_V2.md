# Brutal Review V2 — DollarBill

**Reviewer:** Claude Opus 4.6, Feb 2026  
**Scope:** Full codebase audit — 12,574 LOC src, 6,788 LOC tests, 5,758 LOC examples  
**Build:** Compiles. **289 tests pass, 0 fail.** ~70 compiler warnings across examples.

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
