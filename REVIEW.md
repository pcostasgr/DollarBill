# DollarBill — Cumulative Project Review

**Last Updated:** September 5, 2026  
**Reviewer:** Claude Sonnet 4.6  
**Scope:** Full codebase audit — ~18,000 LOC src, 7,905 LOC tests, 6,678+ LOC examples  
**Build:** Compiles clean. **748 tests pass, 0 fail, 16 ignored.** Zero warnings in `src/`.

This document merges all review rounds — original Brutal Review, V2, Reevaluation, the
July 2026 update, and the September 2026 Adversarial Hardening Plan update — into a single
reference. The current-state sections reflect September 2026. The original pre-fix audits
are preserved at the bottom for historical context.

---

## September 2026 Update — Adversarial Hardening Plan

A dedicated adversarial-hardening pass (`TestFiles/DollarBill_Adversarial_Hardening_Plan.md`,
local working doc) closed the remaining gaps between the risk layer's design and its runtime
behavior, informed by a replay of the actual July 2026 incident activities ledger.

**OCC Parsing Rewrite**
`src/alpaca/occ.rs` replaces ad-hoc `nth()`/slice-index parsing (which broke on
compact/padded root symbols like GLD, QCOM, GOOGL) with a single `parse_occ()` used
everywhere — `client.rs`, `live_bot.rs`, `position_monitor.rs`. Proptest fuzzing covers
no-panic-on-arbitrary-input, round-trip (compact + padded), and malformed input.

**Shared Position Management**
`src/risk/position_management.rs::manage_open_positions()` is now the single decision point
for profit-take, defensive-close, roll, and force-close-long-premium actions, called
identically by `live_bot.rs` and the backtesting engine. Added a per-symbol concentration cap
(`max_risk_per_symbol_pct`, default 6%) and a roll-imbalance guard that flattens a rolled leg
if the new fill quantity doesn't match the old leg's quantity.

**Runtime Invariant Enforcement**
`src/risk/invariants.rs::assert_invariants()` runs post-fill and checks NoNakedLongPremium,
NoUnprotectedEquity, MaxLossWithinLimit, DailyDrawdownWithinLimit, and
CircuitBreakerStaysTripped. On violation the bot now actually flattens all open risk
(`close_all_positions` + local/store state cleanup) and sends a dedicated
`Alerter::invariant_violation` email — previously it only set the circuit-breaker flag,
leaving existing risk in place.

**Order-Path Contract**
`src/order_path.rs` is a new pure module implementing an explicit
validate_signal → check_risk_guards → size_position → build_order → validate_occ_symbol →
generate_client_order_id pipeline, with a distinct `OrderPathError` variant per failure mode
(no silent-continue / `unwrap_or` masking). Documented step-by-step in `ORDER_PATH.md`,
including an independent-review checklist answered against the current codebase.
`validate_occ_symbol` is now called in `live_bot.rs` as an extra pre-submit gate for both
single-leg and multi-leg orders.

**`protected_equity` Wiring**
`BotRuntimeConfig.protected_equity` is now threaded into both `live_bot`'s `ManagementConfig`
and the invariant checker's `BotState`. Previously this field was always empty, so
intentionally-held long stock (e.g. from an assignment) could never be marked protected and
was vulnerable to being force-closed by the long-premium guard.

**Assignment Detection**
`get_account_activities()` on `AlpacaClient` queries Alpaca's OPASN/OPEXC activity feed; the
bot warns on startup if there are recent assignments/exercises it doesn't yet account for.

**New Test Coverage**
- `src/alpaca/client.rs` `mock_http_tests`: a local hand-rolled HTTP server (no external
  mocking crate) proving 422/403 responses surface as `Err` without retry, 429 is retried to
  success, and ambiguous connect/timeout errors are never retried — the basis of the
  `client_order_id` idempotency contract.
- `src/streaming/mod.rs`: a local mock WebSocket server test proving `next_event()` recovers
  from an abrupt connection drop via `reconnect_with_backoff` and resumes delivering data.
- `tests/integration/test_kill_switches.rs` (10 tests): the §1.3 kill-switch scenario table
  plus 3 bonus tests, exercised purely at the risk/invariant layer (no real Alpaca calls).
- `tests/integration/test_july_replay.rs`: deterministic replay of the saved July 2026
  incident's Alpaca activities ledger through the shared risk layer, asserting the post-fix
  guards prevent the original loss class (naked long premium, missed assignment liquidation,
  runaway concentration, drawdown breaker).
- `.github/workflows/ci.yml`: build + test + `clippy -D warnings` gate on every push/PR.

**Still Open**
- The assignment-race window (gap between a risk check and order submission) is closed
  reactively (via invariants/assignment detection) but not preventively.
- Partial multi-leg fill has no HTTP-level mock test — only invariant-layer coverage.
- `examples/personality_based_bot.rs` still uses its own inline close logic instead of the
  shared `manage_open_positions()`.

---

## July 2026 Update — Post-April Changes

### What Was Shipped (April 12 – July 26, 2026)

**Iron Condor Backtest — Variants A–G (Kill 17)**
A seven-variant diagnostic backtest was run on the regime-aware iron condor strategy.
Key finding: `should_enter()` suppression of `HighVol`/`Trending` regime entries (Variant G)
suppresses 188 entry slots and raises max DD from 18.79% (Variant F) to 20.95%. The delta
between F and G comes from mid-trade regime changes on condors entered in `LowVol`. Variant F
(P&L-aware dynamic stop with realistic slippage and entry quality filter at 0.35) is the
recommended configuration — it balances drawdown protection with fill quality without the
additional DD cost of full HighVol suppression.

**Portfolio Greeks Engine (Phase 2 Part 1)**
`src/analysis/portfolio_greeks.rs`: `OptionLeg`, `PortfolioGreeks`, `ExposureVector`,
`PortfolioLimits`. Vanna/volga/charm computed via closed-form BSM; speed/zomma/color via
finite difference. 20-leg book reprices in ~90 µs (22× under the 2 ms budget). Kill tests
12–14 pass: regime-aware sizing by vega/delta/theta concentration.

**RegimePipeline + AuditLog (Phase 2 Parts 2–3)**
`RegimePipeline` wired into the order pipeline. All orders flow through the pipeline before
submission. `AuditLog` records regime at signal time and at fill time so regime drift between
signal and execution is detectable. Kill tests 15–16 pass.

**Regime-Adaptive Heston Calibrator**
ε-insensitive Feller condition enforcement prevents degenerate paths near the boundary.
Nelder-Mead polish applied after CMA-ES convergence for local refinement. Regime stability
test added: calibrated params are checked for consistency across consecutive regime windows.

**Backtest-vs-Live Gap Fixes (2.1–2.6)**
- *2.1*: `backtest_strategy.rs` adds Strategy 4 (Personality-Matched) using
  `StrategyMatcher::build_from_backtests()` + `get_optimal_strategy()` — same call chain as
  the live `personality_based_bot`.
- *2.2*: `src/risk/guards.rs` (new): shared `DailyRiskLimits`, `check_daily_drawdown()`,
  `check_daily_trade_cap()`, `check_all()`. Both `engine.rs` and the personality bot call
  the same functions; no drift possible.
- *2.4*: `backtest_heston.rs` replaces full-window `load_heston_params()` with
  `estimate_rolling_heston_params()` which re-estimates at each trade date from trailing
  price history only. **Eliminates look-ahead bias in Heston Sharpe numbers.**
- *2.5*: `personality_based_bot.rs` adds `EXPIRY_CLOSE` handler that closes positions ≤1 day
  to expiry at market — mirrors the backtest engine's intrinsic-value settlement.
- *2.6*: `examples/reconcile_backtest_vs_live.rs` (new): reads `trade_audit.csv`, re-prices
  each fill with BSM rolling vol, diffs expected vs realised P&L, flags rows >20% divergence.

**Live Bot Safety Guards (13 guards across 2 commits)**

From the Alpaca activities audit (`5d0afb5`):
1. OCC fingerprint dedup — `submitted_this_iteration` set prevents double-submitting legs.
2. DTE=0 gate — positions never opened on expiry day (prevents zero-value GLD-style fills).
3. Explicit ET window check (09:30–16:00 ET) after Alpaca clock — no extended-hours options.
4. `block_long_premium` flag (default `true`) — drops `BuyCall`/`BuyPut`/`BuyStraddle`.
5. `min_momentum_short_put` gate (default −5%) — blocks short puts when 20d momentum negative.
6. `sell_assigned_stock` flag (default `true`) — market-sells equity from option assignment.
7. `max_positions_per_symbol` cap (default 2) — prevents QCOM-style concentration.

From the consolidated library audit (`a7e1f1d`):
8. `post_order_safe()` — idempotent order submission with auto-generated `client_order_id`;
   does **not** retry on timeout (ambiguous outcome), retries Alpaca 429/50x only.
9. Force-close long-premium positions — each iteration when `block_long_premium=true`, scans
   open OCC positions with `qty>0` and submits market sell-to-close.
10. Call-side roll/ITM defense in `position_monitor.rs` — `evaluate_roll_or_itm()` now applies
    mirrored roll/ITM thresholds as spot rises toward a short-call strike.
11. BSM-based P&L for SL/TP — `PositionSnapshot` gains `strike`/`is_call` fields; `evaluate()`
    uses `black_scholes_call/put` when populated; ATM proxy retained as fallback.
12. 50%-credit target and 21-DTE rules wired into `PositionMonitorConfig`.
13. `credit_target_pct` (default 0.50) config field for position closing threshold.

All new config fields use `serde` defaults — existing `trading_bot_config.json` files
continue to work without modification.

**Other Fixes and Features**
- Finnhub added as a third spot-price connector (`src/market_data/`); connector is
  configurable via `trading_bot_config.json` (`spot_provider`: `"alpaca"` | `"yfinance"` | `"finnhub"`).
- Ubuntu/Linux server compatibility: Alpaca price feed fix + SQLite read-only mode.
- Cross-platform scripts and docs added (`scripts/`, `docs/`).
- Reporting: fill-time Greeks/IV snapshot, JSON/CSV export, paper trading flag.
- Yahoo Finance crumb-based auth fixed for options fetch.
- `serde rename` attrs added to `Trade`/`Quote`/`Snapshot` for Alpaca's abbreviated field names.
- Stale position cleanup: positions no longer present in Alpaca are removed from local state.
- Multi-leg OCC order resolution: `ratio_qty=1` per leg (GCD coprime requirement enforced).
- Circuit breaker pre-flight check fixed; NO SIGNAL log spam reduced.
- Duplicate position entries and false circuit-breaker trips fixed.
- Backtest date display fixed; Sharpe ratio overflow on zero-variance returns fixed.

**Test Count (July 2026)**
| Harness | Passing | Ignored |
|---|---|---|
| lib (unit) | 279 | 7 |
| integration | 379 | 0 |
| pricing validation | 14 | 7 |
| portfolio | 64 | 0 |
| doctests | 11 | 2 |
| verify_cdf | 1 | 0 |
| **Total** | **748** | **16** |

---

## Overall Grade: 7.5 / 10

DollarBill is a functional options trading toolkit. The math is correct, the strategies are
real, the order routing works end-to-end, and the trading infrastructure (WebSocket streaming,
SQLite persistence, portfolio risk monitoring, proper CLI) is all in place.

It is not production-grade — no execution retry, no institutional risk monitoring, no
live options approval from Alpaca. But it is a genuine implementation of options pricing,
strategy signal generation, paper trading, and backtesting. Not a demo, not a stub.

**For learning options math in Rust: 9/10.**  
**For paper trading with real signals: 7.5/10.**  
**For production: 4/10** — needs execution retry, deeper monitoring, broker approval for live
options orders.

---

## Part 1: What Works — Module by Module

### CMA-ES Optimizer — 9/10
Replaced the original Nelder-Mead simplex with CMA-ES (`src/calibration/cmaes.rs`).
Covariance Matrix Adaptation Evolution Strategy with σ₀=0.15, 10k function-evaluation
budget, and Feller-condition enforcement. Calibrates to the TSLA crash-period surface
with mean |ΔIV| < 0.8% — verified by `heston_on_tesla_crash_period` kill-criterion test.

### Black-Scholes-Merton — 8/10 (was 7/10 → fixed)
186 lines. Correct Abramowitz & Stegun CDF approximation — the extra `* t` bug was fixed.
Greeks computed in a single pass. `tests/verify_cdf.rs` validates N(x) against 6 known
reference values within 5e-5. ATM call theta verifies to ≈ −6.41/year at r=5% (Hull §18).

### Heston Analytical (Carr-Madan FFT) — 9/10
1,044 lines. Characteristic function and numerical integration correctly implemented.
Falls back to Black-Scholes on numerical failure. The "Gauss-Lobatto" label is wrong
(it's Gauss-Legendre nodes) but the integration is correct.

**Phase 1 validation complete (April 11, 2026):** all 4 kill-criterion tests pass in release:
- BSM PCP < $0.001 on 10k random options ✅
- BSM delta < 0.5% vs finite-difference ✅
- 50×10 Heston batch < 1.5 ms in release (`HestonCfCache` + GL-32) ✅
- Heston CMA-ES calibration MAE < 0.8% on TSLA crash surface ✅

`py/validate_pricing.py --rust` confirms QuantLib parity across BSM, Heston, and American pricing.

### Heston Calibrator — 9/10
261 lines + CMA-ES backend. Real Carr-Madan pricing in the objective function. Weighted
RMSE with bid-ask spread weighting. Feller condition enforcement. CMA-ES optimizer
(replaced Nelder-Mead) calibrates real Heston parameters from market data. Proven on
the TSLA Feb–Mar 2025 crash surface.

### Heston Monte Carlo — 7/10 (was 6/10 → fixed)
781 lines. SplitMix64 (64-bit, passes BigCrush) replaced the 32-bit LCG. Antithetic
variates for variance reduction. Path simulation deduplicated with `is_call: bool` —
~400 lines of call/put duplication removed.

### Backtesting Engine — 7/10 (was 6/10 → fixed)
1,485 lines. ITM expiration now settles at intrinsic value. All multi-leg spreads use
per-day historical vol (was hardcoded at 30%). Five slippage models (fixed, vol-scaled,
size-impact, panic-widening, full-market-impact). Reg T margin. 7 test files.

### Core Strategies — 7/10 (was 3/10 → rewritten)
All 6 strategies use real IV/HV inputs — no `SystemTime`, no `sin()`:

| Strategy | Signal Logic | Default DTE |
|----------|-------------|-------------|
| `MomentumStrategy` | IV ratio vs historical vol → buy/sell straddle | 30d |
| `MeanReversionStrategy` | IV z-score vs model → buy/sell straddle | 21d |
| `BreakoutStrategy` | IV expansion + model confirmation → iron butterfly or sell straddle | 14–30d |
| `VolatilityArbitrageStrategy` | IV premium + regime-weighted edge → straddle or iron butterfly | 14–30d |
| `CashSecuredPuts` | IV rank filter → cash-secured put with absolute strike | 30d |
| `VolMeanReversionStrategy` | z-score with rolling vol std — original real strategy, unchanged | 30d |

All `SignalAction` variants carry explicit fields — no information lost between signal and order:
- `SellStraddle { strike, days_to_expiry }`
- `BuyStraddle { strike, days_to_expiry }`
- `IronButterfly { center_strike, wing_width, days_to_expiry }`
- `CashSecuredPut { strike, days_to_expiry }`

23 tests in `test_core_strategies.rs` verify exact signal types for exact inputs.

### Alpaca Client — 8/10 (was 4/10 → rebuilt)
752 lines. Full multi-leg options order support: `OptionsOrderRequest`, `OptionsLeg`,
`signal_to_options_order`, `signal_to_legs`. OCC symbol generation
(`AAPL  250117C00150000`) with correct Friday expiry snapping. All `SignalAction` variants
convert to OCC orders. Parse helpers: `buying_power_f64()`, `equity_f64()`,
`portfolio_value_f64()`, `last_equity_f64()`, `day_pnl_f64()`. 14 unit tests.

### Alpaca Options Order Routing

| SignalAction | Legs | Side |
|---|---|---|
| `SellStraddle` | 2 (call + put, same strike) | sell-to-open |
| `BuyStraddle` | 2 (call + put, same strike) | buy-to-open |
| `IronButterfly` | 4 (sell ATM call/put, buy OTM call/put at ±wing) | mixed |
| `CashSecuredPut` | 1 (put at strike) | sell-to-open |

OCC format: `"AAPL  250117C00150000"` — 6-char symbol, 6-char YYMMDD, C/P, 8-char strike × 1000.
Expiry snaps to the nearest Friday at or after `today + days_to_expiry`.

### Margin Calculator — 8/10
337 lines. Reg T naked option margin, credit spread margin (wider wing), iron condor
margin, max-loss calculations. 15 dedicated unit tests.

### Advanced Stock Classifier — 8/10 (was 7/10 → stubs fixed)
905 lines. All 3 previously-stubbed functions now fully implemented:
- `calculate_sr_strength()` — local extrema (5-bar window), 1% clustering into S/R zones,
  bounce/break scoring over 252 trading days. Returns empirical bounce rate.
- `calculate_sector_relative_vol()` — reads peer CSVs, computes 21-day HV median,
  returns `own_vol / sector_median` clamped to [0.2, 5.0].
- `calculate_sector_relative_momentum()` — 21-day log-return z-score vs sector peers,
  clamped to [−1, 1].

### Portfolio Module — 7/10 (was 6/10 → tests added)
~2,400 lines. Greek aggregation, VaR at 95%/99%, CVaR, Kelly sizing, performance
attribution, allocation engine (equal-weight, risk-parity, performance-weighted).
30+ direct unit tests covering VaR bounds, Greek aggregation, concentration risk,
CVaR ≥ VaR, allocation methods, rebalancing, Sortino, Omega ratio, drawdown.

### WebSocket Streaming — 8/10 (new)
`src/streaming/mod.rs`. `AlpacaStream` connects to Alpaca's IEX/SIP data feed.
Trade ticks, best-bid/ask quotes, auto-reconnect with `MarketEvent::Reconnected`.
Active in `dollarbill signals --live` and `dollarbill trade --live`.

### SQLite Persistence — 7/10 (new)
`src/persistence/mod.rs`. `TradeStore` via `sqlx`. `TradeRecord`, `PositionRecord`,
`BotStatus` (written atomically to `data/bot_status.json` after every tick). Live bot
reads back open positions at startup. Dashboard binary reads `bot_status.json`.

### CLI — 8/10 (was 3/10 → rebuilt)
Full `clap` subcommand tree replacing the 240-line `main()` monolith:

| Command | Purpose |
|---|---|
| `dollarbill demo [--symbol TSLA]` | Interactive pricing demo |
| `dollarbill price <SYMBOL> <STRIKE> [--dte 0.25] [--rate 0.05]` | Price a single option |
| `dollarbill backtest [--symbol TSLA] [--save]` | Run backtests, optionally persist results |
| `dollarbill signals [--symbol TSLA] [--live]` | Print or stream trading signals |
| `dollarbill calibrate <SYMBOL>` | Calibrate Heston parameters |
| `dollarbill trade [--live] [--dry-run]` | Start the paper-trading bot |

---

## Part 2: All Bugs Fixed (Cumulative)

| # | Bug | File | Was | Fixed |
|---|---|---|---|---|
| 1 | Call theta sign (original claim — not a real bug) | `bs_mod.rs` | Code matched Hull's formula | ✓ verified |
| 2 | CDF extra `* t` (~3% error on all BSM prices) | `bs_mod.rs` | Spurious `* t` in polynomial | ✓ |
| 3 | `close_all_positions` expires ITM at $0 | `engine.rs` | No intrinsic calc | ✓ |
| 4 | `optimal_exercise_boundary` anchored at strike | `american.rs` | Used strike instead of spot lattice | ✓ |
| 5 | Multi-leg spreads hardcoded at 30% vol | `engine.rs` | Ignored hist vol input | ✓ |
| 6 | Heston Monte Carlo 32-bit LCG RNG | `heston.rs` | Period 2³², fails BigCrush | ✓ SplitMix64 |

---

## Part 3: Complete Fix Changelog

| Previous Finding | Current Status |
|---|---|
| `sin(SystemTime)` strategies | **Fixed** — all strategies use real IV/HV logic |
| Backtester expires everything worthless | **Fixed** — intrinsic-value settlement |
| Hardcoded 30% vol in spreads | **Fixed** — passes `current_vol` |
| 32-bit LCG PRNG | **Fixed** — SplitMix64 |
| Exercise boundary uses strike | **Fixed** — uses spot lattice |
| CDF extra `* t` bug | **Fixed** — prices accurate to <0.0001 |
| Alpaca cannot trade options | **Fixed** — full OCC order routing |
| `SellStraddle`/`BuyStraddle` had no fields | **Fixed** — explicit `strike`, `days_to_expiry` |
| `IronButterfly` had no center/DTE | **Fixed** — `center_strike`, `wing_width`, `days_to_expiry` |
| `CashSecuredPut` used percentage | **Fixed** — absolute `strike` |
| Fake strategies untested | **Fixed** — 23 tests in `test_core_strategies.rs` |
| Alpaca tests all `#[ignore]` | **Partially fixed** — 14 OCC/signal tests are live |
| `#![allow(dead_code)]` blankets | **Resolved** — no blanket suppression in `src/` |
| Advanced classifier stubs (3 fns) | **Fixed** — all 3 implemented with real algorithms |
| Portfolio module zero unit tests | **Fixed** — 30+ direct unit tests |
| String money types — no parse helpers | **Fixed** — `buying_power_f64()`, `equity_f64()`, etc. |
| Heston MC ~400-line duplication | **Fixed** — parameterized `is_call: bool` |
| No CLI subcommands | **Fixed** — full `clap` tree |
| No live WebSocket feed | **Fixed** — `AlpacaStream` with reconnect logic |
| No position persistence | **Fixed** — SQLite via sqlx (`TradeStore`) |
| No portfolio risk monitoring loop | **Fixed** — live bot tracks Δ/Γ/Vega/Θ, emits hedge alerts |

---

## Part 4: Module Quality Ratings (July 2026)

| Module | LOC | Rating | Notes |
|---|---|---|---|
| `models/bs_mod.rs` | 186 | **9/10** | CDF correct, Greeks correct, reference-tested |
| `models/heston_analytical.rs` | 1,044 | **9/10** | Carr-Madan FFT + GL batch cache; Phase 1 kill-criteria all pass |
| `models/heston.rs` | 781 | **7/10** | SplitMix64; path sim deduplicated (`is_call: bool`) |
| `models/american.rs` | 460 | **7/10** | Tree math correct |
| `backtesting/engine.rs` | ~1,600 | **8/10** | ITM settlement; real vol; DailyRiskLimits guards; look-ahead-free Heston |
| `backtesting/margin.rs` | 337 | **8/10** | Reg T rules; 15 tests |
| `strategies/` (all) | ~1,500 | **7/10** | Real signals; all variants tested |
| `alpaca/client.rs` | ~800 | **8/10** | Full OCC routing; idempotent post_order_safe(); parse helpers |
| `calibration/` | ~800 | **9/10** | CMA-ES + Heston; ε-insensitive Feller; NM polish; regime stability test |
| `analysis/advanced_classifier.rs` | 905 | **8/10** | Real S/R strength + sector relative vol/momentum |
| `analysis/portfolio_greeks.rs` | ~350 | **8/10** | vanna/volga/charm closed-form; 20-leg book ~90 µs; kill tests 12–14 pass |
| `portfolio/` | ~2,400 | **7/10** | 30+ direct unit tests; complete architecture |
| `streaming/mod.rs` | ~400 | **8/10** | Alpaca WebSocket; trade + quote events; auto-reconnect |
| `persistence/mod.rs` | ~350 | **7/10** | SQLite via sqlx; TradeRecord, PositionRecord, BotStatus |
| `alpaca/live_bot.rs` | ~1,200 | **8/10** | 13 safety guards; idempotent orders; position monitor with BSM SL/TP |
| `risk/guards.rs` | ~160 | **8/10** | Shared DailyRiskLimits; drawdown/trade-cap checks used by both engine and bot |
| `market_data/` | ~400 | **7/10** | Configurable spot provider: Alpaca / Yahoo Finance / Finnhub |

---

## Part 5: Test Coverage (September 2026)

**748 passing tests across 45+ test files — 0 failed, 16 ignored**

| Module | Source LOC | Test Files | Count | Coverage |
|---|---|---|---|---|
| Models (BS, Heston, American) | ~3,500 | 14 files | ~180 | **Excellent** — put-call parity, Greeks, stress, proptest, CDF reference |
| Pricing Validation (Phase 1) | — | `pricing_validation.rs` | 14 | **Kill-criteria** — BSM PCP 10k, delta FD, Heston batch 1.5ms, CMA-ES crash, portfolio Greeks 20-leg |
| Calibration | ~800 | 1 file | ~15 | **Good** — CMA-ES convergence, Heston calibration, regime stability |
| Backtesting | ~2,200 | 7 files | ~90 | **Good** — engine, short options, slippage, edge cases, margin |
| Strategies | ~1,500 | 4 files | ~60 | **Good** — exact input→output for all 6 strategies |
| Portfolio | ~2,000 | 1 file | 64 | **Good** — VaR, Greeks, Kelly, allocation, performance, CVaR |
| Alpaca / Live Bot | ~2,400 | inline + unit | ~90 | **Good** — OCC parser proptest, mock-HTTP retry/idempotency, signal conversion, safety guards |
| Risk (guards/invariants/position_management) | ~900 | inline + unit | ~30 | **Good** — proptest guards, invariant violation paths, concentration/roll checks |
| Order Path | ~350 | inline | ~10 | **Good** — full-pipeline proptest fuzzing across adversarial roots/strikes/expiries |
| Analysis | ~1,300 | 2 files | ~30 | **Good** — classifier; portfolio Greeks; all stub functions covered |
| Backtesting/Margin | ~337 | inline | 15 | **Good** — Reg T margin, credit spreads, iron condor |
| Integration | — | `tests/` | 379 | **Good** — end-to-end flows, kill-switch scenarios, July incident replay |

---

## Part 6: What Still Needs Work

1. **`examples/personality_based_bot.rs` still inline** — doesn't yet call the shared
   `manage_open_positions()` used by `live_bot.rs` and backtesting; still uses older inline
   close logic, so it can drift from the shared guards.
2. **Assignment-race window** — the gap between a risk check and order submission is closed
   reactively (invariants + OPASN/OPEXC detection) but not preventively.
3. **Partial multi-leg fill** — no HTTP-level mock test, only invariant-layer coverage.
4. **Iron condor Variant G DD regression** — Variant G's 20.95% max DD (vs 18.79% for F)
   is caused by mid-trade regime changes on condors entered in `LowVol`. The fix is entry-time
   regime pinning (store regime at entry; close if current regime differs by >1 class).
5. **Live options approval** — Alpaca paper approval does not carry over to live. The bot
   will not submit live options orders without a separate live-trading options approval.

---

## Part 7: Final Component Scores (September 2026)

| Component | Rating | Assessment |
|---|---|---|
| Options Pricing | 9/10 | Correct, tested, QuantLib-parity verified (Phase 1 complete) |
| Greeks | 9/10 | All signs correct; higher-order (vanna/volga/charm) closed-form |
| Heston MC | 7/10 | SplitMix64 RNG; duplication removed |
| Heston Calibrator | 9/10 | CMA-ES + NM polish; ε-insensitive Feller; regime stability test |
| Backtesting | 8/10 | Honest P&L; real vol; DailyRiskLimits; look-ahead-free Heston params |
| Strategies | 7/10 | Real signals, all variants tested |
| Alpaca / Order Routing | 8/10 | Full OCC via central `occ.rs` parser; idempotent submission; mock-HTTP retry tests |
| Advanced Classifier | 8/10 | Real S/R + sector relative vol/momentum |
| Portfolio Greeks | 8/10 | vanna/volga/charm; 20-leg ~90 µs; kill tests 12–16 pass |
| Strategy Matching | 8/10 | `performance_matrix.json` populated from real backtest data |
| Portfolio Module | 7/10 | Complete architecture; 64 direct tests |
| Streaming | 8/10 | AlpacaStream; trades, quotes, reconnect; mock server test proves recovery |
| Persistence | 7/10 | SQLite; fills, positions, bot status |
| CLI | 8/10 | Full clap subcommand tree |
| Live Bot | 8.5/10 | Shared `manage_open_positions()`; runtime invariant enforcement with active flatten+alert; order-path contract |
| Order Path | 8/10 | Pure pipeline, explicit error variants, documented + proptest-fuzzed |
| **OVERALL** | **8/10** | Production-approaching options trading toolkit |

---

---

> # Historical — Review V2 (Pre-Fix Audit)
>
> _Written before the fixes described above. Retained for historical context.
> Everything marked as a bug, stub, or missing in this section has since been resolved._

## Original V2 Executive Summary

DollarBill is a **solid options math library with three fake strategies duct-taped to it**.
The Black-Scholes, Heston, and calibration code is real and mostly correct. Everything between
the pricing models and actual trading decisions is either a stub, a `sin(SystemTime)` placeholder,
or hardcoded data from a single backtest run.

**Overall grade: 5/10** — Real math core wrapped in a shell of theater.

---

## V2 Part 1: What Was Good

### Nelder-Mead Optimizer — 8/10
The best module in the project. 243 lines of clean, correct simplex optimization.
Tested against Rosenbrock and sphere functions. No dead code, no magic numbers, no `unwrap()` abuse.

### Black-Scholes-Merton — 7/10 (with a bug)
163 lines. Clean implementation, proper Abramowitz & Stegun CDF approximation. Greeks in a
single pass — efficient. One real math bug (CDF extra `* t`), but the structure is good.

### Heston Analytical (Carr-Madan FFT) — 7/10
309 lines. Characteristic function and numerical integration properly implemented.
Falls back to Black-Scholes on numerical failure. The "Gauss-Lobatto" label is wrong
(it's Gauss-Legendre nodes), but the integration works.

### Heston Calibrator — 7/10
261 lines. Real Carr-Madan pricing in the objective function. Weighted RMSE with bid-ask
spread weighting. Feller condition enforcement. Calibrates real Heston parameters.

### Advanced Stock Classifier — 7/10
729 lines. Rolling volatility percentiles, trend strength via linear regression, mean
reversion via MA deviation recovery. Had 3 stub functions returning hardcoded values
(`calculate_sr_strength() → 0.6`, sector-relative → 1.0). All 3 have since been
implemented with real algorithms.

### Volatility Surface Utilities — 7/10
224 lines. Newton-Raphson IV solver with convergence guards. Vol surface extraction,
CSV export, smile analysis. Bounded (0.01–5.0 sigma).

### Backtesting Engine — 6/10 (with severe bugs)
1,387 lines. Architecturally ambitious, partially functional. Multiple option types,
5 slippage models, partial fill modeling. All four critical bugs (below) have since
been fixed.

### Test Suite — 6/10
289 tests passing. Heavily skewed toward models — the fake strategies had almost no tests.
Fake strategies were untested because there was nothing deterministic to test.

### Portfolio Module — 6/10
~2,000 lines. Position sizing, VaR, Kelly, allocation engine. Architecturally sound.
Covered in `#![allow(dead_code)]` blankets. No dedicated unit tests. Both issues fixed.

---

## V2 Part 2: Critical Bugs (All Since Fixed)

### Bug 1 — CDF Extra `* t` (~3% error)
**`src/models/bs_mod.rs`** — Spurious `* t` in the CDF polynomial corrupted every BSM
price by ~3%. Fixed; `tests/verify_cdf.rs` guards against regression.

### Bug 2 — Backtester Expires All Positions Worthless
**`src/backtesting/engine.rs`** — `close_all_positions()` marked every remaining position
as expired worthless regardless of moneyness. Deep ITM calls were valued at $0. Fixed
to calculate and credit intrinsic value at expiry.

### Bug 3 — American Exercise Boundary Uses Strike as Spot
**`src/models/american.rs`** — `optimal_exercise_boundary()` anchored the stock tree at
`strike` instead of `spot`. The entire output was meaningless. Fixed: both loops now use
`spot × u^i × d^(n-i)`.

### Bug 4 — All Multi-Leg Spreads at Hardcoded 30% Vol
**`src/backtesting/engine.rs`** — Every iron condor and credit spread used `volatility = 0.30`
regardless of the actual symbol. Fixed: engine passes `current_vol` from per-day historical vol.

### Bug 5 — Heston Monte Carlo 32-Bit LCG
**`src/models/heston.rs`** — 32-bit LCG with period 2³². For 100K-path MC, consecutive
seeds `seed + i` produced highly correlated sequences. Fixed: replaced with SplitMix64
(64-bit, passes BigCrush).

---

## V2 Part 3: The `sin(SystemTime)` Disease

Three of four core strategies generated signals from wall-clock time, not market data:

```rust
// Momentum (src/strategies/momentum.rs)
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as f64;
let fast_cycle = (now * 0.001).sin();

// Mean Reversion (src/strategies/mean_reversion.rs)
let price_offset = (now as f64 * 0.001 + symbol_hash * 0.01).sin() * std_dev;

// Breakout — same pattern
// Vol Arbitrage — started with real inputs, then multiplied by sin() "regime factor"
```

These strategies were: non-reproducible, non-testable, and ignored all declared inputs.
`spot`, `market_iv`, `model_iv`, `historical_vol` were parameters in the signature but
ignored in the body.

The only honest strategy was `VolMeanReversion` (99 lines), which used IV/HV inputs for
a deterministic z-score calculation.

All four have since been rewritten. 23 tests in `test_core_strategies.rs` now verify
exact signal types for exact inputs.

---

## V2 Part 4: Architecture Debt (All Resolved)

| Issue | Status |
|---|---|
| `#![allow(dead_code)]` blankets in `src/` | Removed — zero blanket suppression |
| Heston MC ~400-line call/put duplication | Parameterized `is_call: bool` |
| `main.rs` 240-line monolith | Replaced with full `clap` subcommand tree |
| Alpaca equity-only — no options support | Full OCC multi-leg order routing |
| String money types — no parse helpers | `buying_power_f64()`, `equity_f64()`, etc. |
| 3 classifier stubs returning hardcoded values | All 3 implemented with real algorithms |
| Portfolio module — zero dedicated tests | 30+ direct unit tests |
| No WebSocket live data | `AlpacaStream` — trades, quotes, auto-reconnect |
| No position persistence | SQLite via sqlx (`TradeStore`) |
| No portfolio risk monitoring | `live_bot` tracks Δ/Γ/Vega/Θ, emits hedge alerts |

---

## V2 Original Verdict

> DollarBill is an educational options math library that got dressed up as a trading platform.
> The math works. The calibration works. The vol surface analysis works. That's about 3,000
> lines of genuinely valuable Rust code.
>
> The other 9,500 lines range from "ambitious but buggy" (backtester) to "complete fiction"
> (momentum/mean_reversion/breakout strategies). The Alpaca client can't trade options.
>
> **Knowing your problems and not fixing them is worse than not knowing.**

That was March 2026. The problems have since been fixed.
