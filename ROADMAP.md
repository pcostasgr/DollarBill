# DollarBill — Roadmap

**Written:** March 21, 2026 · **Updated:** March 22, 2026  
**Baseline:** 637 tests passing · clean build · `a06231b`  
**Grade at baseline:** 7.5/10

---

## Where We Are

The 30-day sprint (Feb 20 – Mar 19) is complete. Every original critical bug is
fixed, all strategies generate real signals, the Alpaca client routes every
`SignalAction` variant to OCC orders, and the live bot has a WebSocket event
loop with circuit breaker, SQLite persistence, and graceful shutdown.

**What was shipped:**
- ✅ Real IV/HV-based signals (sin(SystemTime) eliminated)
- ✅ Full OCC options order routing for all 5 strategies
- ✅ Margin calculator (Reg T, spreads, iron condor)
- ✅ Live WebSocket bot (reconnect, circuit breaker, position deduplication)
- ✅ SQLite persistence with startup reconciliation
- ✅ Portfolio manager (64 unit tests)
- ✅ Heston MC deduplicated (greeks_european shared)
- ✅ main.rs refactored into utils/demo.rs + alpaca/live_bot.rs
- ✅ docs/getting-started.md with full CLI reference
- ✅ 637 tests, zero failures

**What still has gaps:**

| Gap | Impact | Effort |
|-----|--------|--------|
| Live bot only opens positions — no close logic | HIGH | Medium |
| Live bot uses raw HV from ticks — not IV rank | HIGH | Low |
| Live bot ignores RegimeDetector and StrategyMatcher | HIGH | Medium |
| `advanced_classifier.rs` has 2 hardcoded stubs (RV/IV=1.0, sector_corr=0.7) | Medium | Medium |
| `performance_matrix.json` never populated with real backtest results | Medium | Low |
| 27 examples — some may be stale; none verified post-refactor | Low | Low |

---

## Phase 2: Close the Loop (Est. 2–3 weeks)

**Goal:** The bot can now open positions. Phase 2 makes it complete — it must also
close them, use real market context, and survive real market sessions.

---

### P2.1 — Position Close Logic in Live Bot  ⭐ highest priority

**Problem:** `alpaca/live_bot.rs` only opens positions. There is zero close logic.
Positions accumulate indefinitely.

**What to build:**
- On each tick, check open positions in `open_syms` against current price and
  entry price stored in SQLite
- Emit close signals when: profit target hit (e.g. 50% of premium collected) or
  stop loss hit (e.g. 200% of premium collected)
- Submit `sell-to-close` order via `client.submit_options_order()`
- Remove from `open_syms` and call `store.close_position()` on fill

**Config keys to add to `trading_bot_config.json`:**
```json
"profit_target_pct": 0.50,
"stop_loss_pct": 2.00,
"max_position_days": 21
```

**Tests to add:** 5–8 tests in `tests/unit/` covering profit target trigger,
stop loss trigger, max days expiry.

---

### P2.2 — IV Rank in Live Bot signal filter  ⭐

**Problem:** The live bot computes `sigma` from a 22-tick rolling window. This is
intraday HV, not IV rank. Most short-options strategies should only fire when
IV rank > 50th percentile.

**What to build:**
- Load persisted vol surface from `data/{symbol}_vol_surface.csv` at startup
- Compute IV rank: `(current_iv - 52w_low_iv) / (52w_high_iv - 52w_low_iv)`
- Add `min_iv_rank` to `TradingBotConfigFile` (default `0.40`)
- Gate short-sell signals behind IV rank check in `live_bot.rs`

**Files touched:** `src/alpaca/live_bot.rs`, `src/config.rs`,
`src/utils/vol_surface.rs` (already has the cubic spline)

---

### P2.3 — Wire RegimeDetector + StrategyMatcher into Live Bot

**Problem:** `src/analysis/regime_detector.rs` and `src/strategies/matching.rs`
exist but are not called anywhere in the live trading path.

**What to build:**
- On startup, load `StrategyMatcher` from `models/performance_matrix.json`
- Run `RegimeDetector` on each symbol's rolling price buf every N ticks
- In the signal loop: only forward signals whose strategy is recommended by
  `matcher.get_recommendations(sym)`
- Skip strategies with confidence below threshold in current regime

**Benefit:** Strategies are selected based on actual historical performance per
symbol per regime — not just "run all five every time".

---

### P2.4 — Populate performance_matrix.json  (quick win)

**Problem:** `models/performance_matrix.json` is empty or missing. The
`StrategyMatcher` falls back to defaults for all symbols.

**What to do:** This is a one-liner once P2.3 is wired:
```powershell
.\target\release\dollarbill.exe backtest --save
```
Run it, commit the resulting JSON. The live bot then has real signal priors.

---

### P2.5 — Fix advanced_classifier.rs stubs

**Problem:** `realized_vs_implied` always returns `1.0`; `sector_correlation`
always returns `0.7`. These fields are used in `StockPersonality` scoring.

**What to build:**
- `realized_vs_implied`: compute as `historical_vol / model_iv` using the same
  HV and Heston v0 we already calculate elsewhere
- `sector_correlation`: load sector ETF prices from `data/` (SPY for broad
  market), compute rolling 21-day correlation of returns

**Tests to add:** 4 tests verifying non-stub return paths.

---

## Phase 3: Data & Pricing Quality (2–3 weeks after Phase 2)

### P3.1 — Live Options Chain Feed

`examples/live_pricer.rs` already fetches Yahoo Finance options chains but it's
an example, not wired into the live bot.

**What to build:**
- Extract Yahoo options fetch into `src/market_data/options_feed.rs`
- On startup (and every 15 min during session), refresh IV surface per symbol
- Use live IV for signal generation instead of HV from ticks
- Cache to `data/{symbol}_options_live.json` (already have these files)

**Files created:** `src/market_data/options_feed.rs`  
**Integration point:** called from `alpaca/live_bot.rs` startup block

---

### P3.2 — Vol Surface Calibration Loop

Heston params are currently static (loaded from `data/{symbol}_heston_params.json`
last calibrated against historical data). In live trading, these drift.

**What to build:**
- Background task: re-calibrate Heston every 30 min using live options chain
- Write updated params to `data/{symbol}_heston_params.json`
- Use updated params for same-session signal generation

**Risk:** calibration is slow (~2s per symbol). Run in a `tokio::spawn` task,
avoid blocking the event loop.

---

### P3.3 — Greeks Hedging in Portfolio Manager

`portfolio/risk_analytics.rs` calculates portfolio-level Greeks but `live_bot.rs`
never queries them. 

**What to build:**
- After each order, call `pm.get_portfolio_risk()` for aggregate delta/gamma/vega
- If portfolio delta exceeds threshold (e.g. `|Δ| > 0.30 × equity / 100`), emit
  a delta-hedge signal (buy/sell underlying or futures)
- Log risk state after each order so the user can see aggregate exposure

---

## Phase 4: Deployment & Observability (1–2 weeks)

### P4.1 — Metrics / Dashboard

A terminal dashboard (using `ratatui`) showing:
- Live P&L per position
- Portfolio delta/gamma/vega
- Circuit breaker state
- Last signal per symbol
- Daily spend vs limit

### P4.2 — Alerting

- Email (via `lettre` crate) or webhook (Discord/Slack) on:
  - Circuit breaker trip
  - Position opened/closed
  - Error connecting to Alpaca
  - Daily loss > 3%

### P4.3 — Docker / systemd packaging

Package the live bot so it can run unattended:
- `Dockerfile` for containerized deployment
- `dollarbill.service` systemd unit (Linux) or Windows Task Scheduler script
- Startup log rotation

---

## Phase 5: ML Integration (Month 3, optional)

The `config/trading_bot_config.json` has ML sections that currently do nothing.
Real ML integration requires PyO3 to call into Python.

**Scope (if pursued):**
- `src/ml/` module with PyO3 bridge
- Train price direction model on 5-year closes (random forest or LSTM)
- Use model confidence as additional signal gate in live bot
- A/B test: strategy-only vs strategy+ML over paper trading period

**Honest assessment:** This is high effort. Defer until Phase 2/3 are solid.
ML features on top of a shaky foundation help nothing.

---

## Priority Ranking

| Priority | Item | Why |
|----------|------|-----|
| 🔴 1 | P2.1 Position close logic | Bot never exits. This is blocking. |
| 🔴 2 | P2.4 Run backtest --save | 10-minute task. Unlocks P2.3. |
| 🟠 3 | P2.2 IV rank gate | Prevents signals during low-IV noise. |
| 🟠 4 | P2.3 RegimeDetector + StrategyMatcher | Use the analysis code that already exists. |
| 🟡 5 | P2.5 advanced_classifier stubs | Easy fix, improves personality matching. |
| 🟡 6 | P3.1 Live options feed | Data quality upgrade. |
| 🟢 7 | P3.2 Vol surface calibration loop | Pricing accuracy in live session. |
| 🟢 8 | P3.3 Greeks hedging | Risk management completeness. |
| ⚪ 9 | P4.x Observability | Quality of life — do after bot is stable. |
| ⚪ 10 | Phase 5 ML | Defer until trading loop is reliable. |

---

## Success Metrics for Phase 2

- [ ] Live bot opens **and closes** positions automatically
- [ ] Zero positions held past `max_position_days` without close
- [ ] IV rank filter reduces false signals in flat-IV periods
- [ ] `performance_matrix.json` populated from real backtest run
- [ ] `StrategyMatcher` produces non-default recommendations for all 10+ symbols
- [ ] 650+ tests passing (add ~15 new tests)
- [ ] Paper trading session: bot runs for a full market day without crash

---

## What NOT to Build Next

- ❌ More example programs (27 is already too many)
- ❌ More documentation pages (14 docs pages is sufficient)
- ❌ PyO3 / ML integration (premature — live bot is incomplete)
- ❌ Spread strategy in live bot (phase 2 should consolidate, not expand)
- ❌ REST API / web UI (out of scope, no demand signal)

---

## Phase V: Validation & Backtesting Realism (1–2 days)

> Origin: Validation Plan audit, March 22, 2026.  
> Separates "pricing calculator" from "production-grade system."  
> All items below are buildable against existing data. Fabricated items
> (CMA-ES, SVI surface from OHLCV, historical Alpaca chains) are excluded.

---

### V0 — Data Integrity (do first; everything downstream depends on it)

**Files to create/change:** `py/validate_data.py`

The full 2025 TSLA dataset is in `data/tsla_one_year.csv` (251 rows, Jan 2–Jan 2, 2026).  
`data/tesla_one_year.csv` has a different header format (row 2 is ticker labels) — the script must handle both.

```python
# py/validate_data.py
import pandas as pd, numpy as np, sys

for path in ["data/tsla_one_year.csv", "data/tesla_one_year.csv"]:
    df = pd.read_csv(path, parse_dates=["Date"], index_col="Date",
                     comment="#", skip_blank_lines=True)
    # Drop ticker-label rows (non-numeric Date index)
    df = df[pd.to_numeric(df["Close"], errors="coerce").notna()]
    df["Close"] = df["Close"].astype(float)
    assert df.index.is_monotonic_increasing, f"{path}: dates not sorted"
    assert (df["Close"] > 0).all(), f"{path}: non-positive closes"
    ann_vol = df["Close"].pct_change().std() * np.sqrt(252)
    max_dd  = (df["Close"] / df["Close"].cummax() - 1).min()
    assert 0.50 < ann_vol < 1.50, f"{path}: ann vol {ann_vol:.1%} outside sane range"
    print(f"{path}: {len(df)} rows | ann_vol={ann_vol:.1%} | max_dd={max_dd:.1%}")
print("Phase 0 PASSED")
```

**Expected output:**
- Annualized realized vol ≈ 85–110% (TSLA 2025 reality)
- Max drawdown ≈ -40% to -50% (Feb–Mar 2025 crash from ~410 → ~222)

**Also add to `tests/verify_data.rs`:**
```rust
#[test]
fn tsla_csv_integrity() {
    let h = load_csv_closes("data/tsla_one_year.csv").unwrap();
    assert!(h.len() >= 240, "expected ~251 trading days");
    assert!(h.iter().all(|d| d.close > 0.0), "non-positive close");
}
```

---

### V1 — Pricing Engine Validation (extend existing; don't duplicate)

**Files to change:** `py/validate_pricing.py`, `benches/heston_pricing.rs`

#### V1a — 10k random BSM batch test  
The existing `validate_pricing.py` tests ≈20 specific points. Add:

```python
# at bottom of validate_pricing.py  -- section: "Batch BSM tolerance"
import random
random.seed(42)
failures = 0
for _ in range(10_000):
    S = random.uniform(50, 500)
    K = S * random.uniform(0.7, 1.3)
    T = random.uniform(0.05, 2.0)
    r = random.uniform(0.01, 0.08)
    sig = random.uniform(0.10, 1.20)
    ql_price = ql_bsm_call(S, K, T, r, sig)   # existing helper
    rs_price = rust_bsm_call(S, K, T, r, sig)  # existing --rust subprocess
    if abs(ql_price - rs_price) > 0.001:
        failures += 1
assert failures == 0, f"BSM batch: {failures}/10000 options exceeded $0.001 threshold"
print(f"BSM batch 10k: PASSED (all within $0.001 of QuantLib)")
```

_Note: skip `--rust` integration if subprocess is slow; run in-process via `cffi` or just
against internal scipy reference. The threshold of `< 0.001 USD` is already met per
existing tests — this hardens it statistically._

#### V1b — Heston batch speed bench

`benches/heston_pricing.rs` already exists. Add a `50_strikes × 10_expiries` criterion group:

```rust
// benches/heston_pricing.rs  -- add this bench group
fn bench_heston_surface(c: &mut Criterion) {
    let params = HestonParams { s0: 250.0, v0: 0.04, kappa: 2.0, theta: 0.04,
                                sigma: 0.3, rho: -0.7, r: 0.05, t: 1.0 };
    let strikes: Vec<f64> = (0..50).map(|i| 150.0 + i as f64 * 4.0).collect();
    let expiries: Vec<f64> = (1..=10).map(|i| i as f64 * 0.1).collect();
    c.bench_function("heston_surface_500_prices", |b| {
        b.iter(|| {
            for &t in &expiries {
                for &k in &strikes {
                    let mut p = params.clone();
                    p.t = t;
                    heston_call_carr_madan(p.s0, k, t, p.r, &p);
                }
            }
        })
    });
}
```

**Pass threshold:** 500 prices < 1.5 ms on a single core  
(`cargo bench -- heston_surface_500` — expected ~0.3–0.8 ms based on existing timing)

#### V1c — Greeks relative error vs QuantLib  
Already covered by `tests/unit/models/test_quantlib_reference.rs` and `test_greeks.rs`.
Add a single assertion to `test_quantlib_reference.rs` to guard the `< 0.5%` bar explicitly:

```rust
#[test]
fn greeks_relative_error_vs_quantlib() {
    // QuantLib AnalyticHestonEngine finite-difference references (precomputed)
    let ql_delta = 0.6323;
    let ql_vega  = 37.82;
    let params = /* classic params */;
    let result = black_scholes_call(100.0, 100.0, 1.0, 0.05, 0.2);
    assert!((result.delta - ql_delta).abs() / ql_delta < 0.005,
        "delta relative error > 0.5%");
    assert!((result.vega  - ql_vega ).abs() / ql_vega  < 0.005,
        "vega relative error > 0.5%");
}
```

**What NOT to build (yet):**
- ❌ CMA-ES calibrator — only Nelder-Mead exists; CMA-ES is new work; defer to Phase 3
- ❌ SVI surface fit from OHLCV — the CSV has no implied vols; not possible without options chain
- ❌ Historical Alpaca options snapshots — paper accounts don't have historical chains

---

### V2 — Backtesting Realism

**Files to change:** `src/backtesting/metrics.rs`, `src/backtesting/engine.rs`

#### V2a — Add missing metrics to `BacktestMetrics`

`metrics.rs` already has Sharpe, Sortino, max drawdown, win rate, profit factor.  
**Add:**

```rust
// src/backtesting/metrics.rs
pub calmar_ratio: f64,   // annual_return / max_drawdown_pct.abs()
pub expectancy: f64,     // avg_win * win_rate - avg_loss * (1 - win_rate)
```

Both are trivial to compute from data already in `BacktestMetrics`. Calmar requires
annualizing the total return (divide by `trading_days / 252`).

**Calmar threshold:** The plan demands Calmar > 3.0. This is **unrealistic for TSLA
short strangles in a year with a -46% drawdown regime.** Use Calmar > 1.0 as the
pass threshold for a single-stock vol-selling strategy. Calmar > 3.0 is only
achievable on cherry-picked periods.

#### V2b — In-sample / out-of-sample date-range split

Add to `BacktestEngine`:

```rust
// src/backtesting/engine.rs
pub fn run_date_range(
    &mut self,
    symbol: &str,
    history: Vec<HistoricalDay>,
    vol_threshold: f64,
    start: &str,   // "2025-01-01"
    end: &str,     // "2025-06-30"
) -> BacktestResult
```

Filter `history` to `[start, end]` by date string prefix match. No new dependencies.

**Usage in tests:**
```rust
// in-sample: Jan–Jun 2025
let is_result  = engine.run_date_range("TSLA", h.clone(), 0.35, "2025-01", "2025-06");
// out-of-sample: Jul–Dec 2025
let oos_result = engine.run_date_range("TSLA", h.clone(), 0.35, "2025-07", "2025-12");
assert!(oos_result.metrics.sharpe_ratio > -1.0, "OOS Sharpe below -1");
```

#### V2c — Named stress scenario replay

Add to `BacktestEngine`:

```rust
pub struct StressScenario {
    pub label: &'static str,
    pub start: &'static str,
    pub end:   &'static str,
    pub max_loss_threshold: f64,  // fraction of equity, e.g. 0.25
}

pub const TSLA_2025_SCENARIOS: &[StressScenario] = &[
    StressScenario { label: "Feb-Mar crash",  start: "2025-02-25", end: "2025-03-10", max_loss_threshold: 0.25 },
    StressScenario { label: "Apr 9 IV crush", start: "2025-04-07", end: "2025-04-11", max_loss_threshold: 0.15 },
];

pub fn run_stress(&mut self, symbol: &str, history: Vec<HistoricalDay>,
                  vol_threshold: f64, scenario: &StressScenario) -> BacktestResult
```

**Add smoke test:**
```rust
#[test]
fn stress_feb_mar_crash_survivable() {
    let h = load_csv_closes("data/tsla_one_year.csv").unwrap();
    let result = engine.run_stress("TSLA", h, 0.35,
        &TSLA_2025_SCENARIOS[0]);
    // A 25% max loss in a -46% underlying crash is realistic for a
    // hedged strategy; increase to 0.40 for naked short strangles
    assert!(result.metrics.max_drawdown_pct < 25.0,
        "Feb-Mar crash max drawdown {:.1}% exceeded 25%",
        result.metrics.max_drawdown_pct);
}
```

---

### V3 — Regime & Tail Risk Validation

**Files to change:** `src/analysis/regime_detector.rs`, `src/backtesting/engine.rs`,  
`src/portfolio/risk_analytics.rs`  
**Files to create:** `tests/unit/models/test_monte_carlo_cvar.rs`

#### V3a — Regime-tagged backtest output

`RegimeDetector::detect(closes)` already exists and produces `MarketRegime` per bar.  
Wire it into backtest reporting:

```rust
// src/backtesting/engine.rs
pub struct RegimeMetrics {
    pub regime: MarketRegime,
    pub sharpe: f64,
    pub max_dd: f64,
    pub trade_count: usize,
}

// BacktestResult gains:
pub regime_breakdown: Vec<RegimeMetrics>,
```

Tag each bar's P&L with the regime at that bar. Aggregate Sharpe and drawdown
per regime bucket.

**What NOT to build:** HMM regime detection. The existing 21-day rolling vol
clustering in `RegimeDetector` is equivalent and already passes its own 17 tests.
HMM adds weeks of work for no practical improvement on monthly timeframes.

#### V3b — Monte Carlo CVaR test

`HestonMonteCarlo` already supports 100k paths and 500 steps.  
`portfolio/risk_analytics.rs` already has `calculate_cvar()`.

**New test file** `tests/unit/models/test_monte_carlo_cvar.rs`:

```rust
#[test]
fn heston_monte_carlo_cvar_within_bounds() {
    // TSLA-like params: high vol-of-vol regime
    let params = HestonParams {
        s0: 250.0, v0: 0.09, kappa: 1.5, theta: 0.09,
        sigma: 1.0, rho: -0.9, r: 0.05, t: 1.0 / 252.0,
    };
    let config = MonteCarloConfig { n_paths: 10_000, n_steps: 1, seed: 42,
                                    use_antithetic: true };
    let mc = HestonMonteCarlo::new(params.clone(), config).unwrap();
    let daily_returns = mc.simulate_daily_returns();  // new helper — see below
    let var_99  = percentile(&daily_returns, 0.01);   // 1st percentile = worst 1%
    let cvar_99 = daily_returns.iter()
        .filter(|&&r| r < var_99)
        .sum::<f64>() / daily_returns.iter().filter(|&&r| r < var_99).count() as f64;

    assert!(cvar_99.abs() < 0.08,
        "1-day 99% CVaR {:.2}% exceeds 8% equity limit", cvar_99 * 100.0);
}

#[test]
fn heston_1yr_cvar_99_below_18pct() {
    // 1-year horizon, 10k paths: 99th percentile tail loss < 18%
    let params = HestonParams { s0: 100.0, v0: 0.04, kappa: 2.0, theta: 0.04,
                                sigma: 0.3, rho: -0.7, r: 0.05, t: 1.0 };
    let config = MonteCarloConfig { n_paths: 10_000, n_steps: 252, seed: 42,
                                    use_antithetic: true };
    let mc  = HestonMonteCarlo::new(params, config).unwrap();
    let annual_pnl = mc.simulate_annual_pnl_pct();  // new helper
    let cvar = compute_cvar_99(&annual_pnl);
    assert!(cvar < 0.18, "1-year 99% CVaR {:.1}% > 18%", cvar * 100.0);
}
```

**New helpers needed in `src/models/heston.rs`:**
- `simulate_daily_returns(&self) -> Vec<f64>` — run 1-step MC, return `(S_1 - S_0) / S_0` per path
- `simulate_annual_pnl_pct(&self) -> Vec<f64>` — run 252-step MC, return terminal `(S_T - S_0) / S_0` per path

Both are 10-line additions using the existing `simulate_path` infrastructure.

**Threshold adjustments from the original plan:**
- 1-day worst-case `< 8%` ✅ keep as-is (reasonable for non-leveraged position)
- 1-year 99% CVaR `< 18%` ✅ keep as-is (applies to a single ATM option, not equity)
- Regime max DD `< 12%` ❌ change to `< 35%` — 12% is impossible for short strangles
  on TSLA in a 120% realized-vol regime (Feb–Mar 2025 saw underlying lose 46%)

---

## Validation Phase Priority Table

| ID | Item | Effort | Blocks | Do First? |
|----|------|--------|--------|-----------|
| V0 | `py/validate_data.py` data integrity script | 30 min | nothing | **Yes** |
| V0 | `tests/verify_data.rs` CSV Rust smoke test | 15 min | nothing | **Yes** |
| V1a | 10k random BSM batch in `validate_pricing.py` | 1 hr | nothing | Yes |
| V1b | Heston surface 500-price bench in `benches/` | 1 hr | nothing | Yes |
| V1c | Greeks `< 0.5%` explicit assertion | 30 min | nothing | Yes |
| V2a | `calmar_ratio` + `expectancy` in `metrics.rs` | 1 hr | V2b,V2c | Yes |
| V2b | `run_date_range()` in `engine.rs` | 2 hr | V2c | Yes |
| V2c | Named stress scenarios + Feb-Mar/ Apr-9 test | 2 hr | V2b | Yes |
| V3a | Regime-tagged `BacktestResult` breakdown | 3 hr | V3b | Yes |
| V3b | MC `simulate_daily_returns` + CVaR test | 2 hr | — | Yes |
| — | CMA-ES calibrator | 2 days | SVI surface | **Defer** |
| — | SVI surface from OHLCV | 2 days | options data | **Skip** — no IV in CSV |
| — | Historical Alpaca IV chains | N/A | — | **Skip** — not available |
| — | HMM regime detection | 3 days | — | **Skip** — existing clustering sufficient |

**Total for "Yes" items: ~13 hours across 2 days**

---

## Updated Success Metrics (all phases)

- [ ] `py/validate_data.py` passes with TSLA ann_vol in [50%, 150%]
- [ ] BSM batch: 0/10000 options exceed $0.001 vs QuantLib
- [ ] Heston surface bench: 500 prices in < 1.5 ms
- [ ] Greeks relative error: Δ and ν both < 0.5% vs QuantLib reference
- [ ] `calmar_ratio` computed and > 1.0 on in-sample 2025-H1
- [ ] Out-of-sample backtest (2025-H2) Sharpe > -1.0
- [ ] Feb-Mar 2025 stress: max drawdown < 35% (realistic, not fantasy)
- [ ] Apr-9 2025 stress: max drawdown < 15% (IV crush is recoverable)
- [ ] Per-regime metrics visible in backtest output
- [ ] 1-day 99% CVaR < 8% of equity (Heston TSLA-like params)
- [ ] 1-year 99% CVaR < 18% (Heston standard params)
- [ ] Total tests: 660+ after all new validation tests
