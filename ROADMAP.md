# DollarBill — Roadmap

**Written:** March 21, 2026 · **Updated:** July 26, 2026  
**Baseline:** 682 tests passing · clean build · `a7e1f1d`  
**Grade at baseline:** 8/10

---

## Where We Are (July 2026)

The original 30-day sprint and all follow-on phases are complete. Phases 2–4 have been
shipped. The live bot now opens **and** closes positions, uses regime-aware sizing, filters
signals through a `RegimePipeline + AuditLog`, and has 13 safety guards from the Alpaca
activities audit.

**March 2026 sprint shipped:**
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

**April – July 2026 additions:**
- ✅ Position close logic: profit target (50% credit), stop loss (21-DTE rule), ITM defense
- ✅ IV rank gate: HV percentile via 1-yr CSV; `min_iv_rank` config key
- ✅ RegimeDetector + StrategyMatcher wired into live bot order pipeline
- ✅ RegimePipeline + AuditLog (kill tests 15–16)
- ✅ Portfolio Greeks Engine: vanna/volga/charm; 20-leg book ~90 µs (kill tests 12–14)
- ✅ Regime-adaptive Heston calibrator: ε-insensitive Feller, NM polish
- ✅ Backtest-vs-live gap fixes (2.1–2.6): shared guards, rolling Heston, expiry-close
- ✅ 13 safety guards from Alpaca activities audit (idempotent orders, DTE=0 gate, ET window, etc.)
- ✅ Finnhub as third spot-price connector; configurable via `spot_provider`
- ✅ Ubuntu/Linux server compatibility; cross-platform scripts
- ✅ Reporting: fill-time Greeks/IV snapshot, JSON/CSV export, paper flag
- ✅ Iron condor variants A–G: Variant F recommended (P&L-stop + slippage + 0.35 filter)
- ✅ 682 tests, zero failures

**What still has gaps:**

| Gap | Impact | Effort |
|-----|--------|--------|
| `performance_matrix.json` not populated from real backtest results | Medium | Low |
| Iron condor Variant G: regime pinning at entry would fix 20.95% DD regression | Medium | Low |
| Live options approval required for Alpaca live (separate from paper) | HIGH | External |

---

## Phase 2: Close the Loop ✅ COMPLETE

All P2.x items are done.

### P2.1 — Position Close Logic ✅
`position_monitor.rs`: BSM-based P&L for SL/TP, 50%-credit target, 21-DTE rule,
call-side roll/ITM defense. Config: `credit_target_pct`, `max_position_days`.

### P2.2 — IV Rank in Live Bot ✅
IV rank computed from vol surface CSV. `min_iv_rank` config key gates short-vol signals.

### P2.3 — RegimeDetector + StrategyMatcher ✅
Both wired into the live bot order pipeline via `RegimePipeline`. `AuditLog` records
regime at signal time and fill time.

### P2.4 — performance_matrix.json
Run `dollarbill backtest --save` to populate. StrategyMatcher falls back to defaults until then.

**Config keys (all live):**
```json
"profit_target_pct": 0.50,
"stop_loss_pct": 2.00,
"max_position_days": 21,
"credit_target_pct": 0.50,
"block_long_premium": true,
"min_momentum_short_put": -0.05,
"sell_assigned_stock": true,
"max_positions_per_symbol": 2,
"spot_provider": "alpaca"
```

### P2.5 — advanced_classifier.rs stubs ✅
All 3 stubs implemented: `calculate_sr_strength()`, `calculate_sector_relative_vol()`,
`calculate_sector_relative_momentum()` with real algorithms.

---

## Phase 3: Data & Pricing Quality ✅ COMPLETE

### P3.1 — Live Options Chain Feed ✅
`src/market_data/options_feed.rs` + `LiveIvCache`. Refreshes every 15 min during session.
Yahoo Finance crumb-based auth fixed.

### P3.2 — Vol Surface Calibration Loop ✅
Background Heston recalibration every 30 min via `tokio::spawn`. Configurable via
`spot_provider` (`alpaca` / `yfinance` / `finnhub`). Params written to
`data/{symbol}_heston_params.json`.

### P3.3 — Greeks Hedging ✅
Portfolio delta/vega/gamma logged after each order. Hedge alerts emitted when delta
exceeds `max_portfolio_delta` threshold.

---

## Phase 4: Deployment & Observability ✅ COMPLETE

### P4.1 — Dashboard ✅
`src/bin/dashboard.rs` (ratatui): live P&L, Greeks, circuit breaker state, signals.

### P4.2 — Alerting ✅
Email via `lettre` in `src/alerting/mod.rs`. Triggers on circuit breaker, fills, errors,
daily loss > 3%.

### P4.3 — Docker / systemd ✅
`Dockerfile` + `deploy/dollarbill.service`. Ubuntu server compatible.
Cross-platform scripts in `scripts/`.

---

## Success Metrics (Updated July 2026)

- [x] Live bot opens **and closes** positions automatically
- [x] Zero positions held past `max_position_days` without close (21-DTE rule)
- [x] IV rank filter reduces false signals in flat-IV periods
- [ ] `performance_matrix.json` populated from real backtest run
- [x] `StrategyMatcher` produces non-default recommendations (wired via RegimePipeline)
- [x] 682 tests passing
- [x] Paper trading session: bot runs for a full market day without crash

---

## What NOT to Build Next

- ❌ More example programs (already 30+)
- ❌ More documentation pages (existing docs are comprehensive)
- ❌ REST API / web UI (out of scope)
- ❌ Iron condor Variant G suppression of HighVol entries (raises DD; not worth it)

---

### P4.2 — Alerting ✅
Email via `lettre` in `src/alerting/mod.rs`. Triggers on circuit breaker, fills, errors,
daily loss > 3%.

### P4.3 — Docker / systemd ✅
`Dockerfile` + `deploy/dollarbill.service`. Ubuntu server compatible.
Cross-platform scripts in `scripts/`.

---

## Phase 5: ML Integration (Optional)

The `config/trading_bot_config.json` has ML sections that currently do nothing.
Real ML integration requires PyO3 to call into Python.

**Scope (if pursued):**
- `src/ml/` module with PyO3 bridge
- Train price direction model on 5-year closes (random forest or LSTM)
- Use model confidence as additional signal gate in live bot
- A/B test: strategy-only vs strategy+ML over paper trading period

**Honest assessment:** High effort. Defer until live trading approval is obtained and the
paper trading session demonstrates stability over multiple market weeks.

---

## Priority Ranking (July 2026)

| Priority | Item | Why |
|----------|------|-----|
| 🔴 1 | Run `backtest --save` → populate `performance_matrix.json` | 10-minute task; unlocks StrategyMatcher priors |
| 🟠 2 | Entry-time regime pinning in iron condor | Fixes Variant G DD regression (20.95% → ~18%) |
| 🟡 3 | Live options approval (Alpaca) | Required before any live trading |
| ⚪ 4 | Phase 5 ML | Defer until live trading is stable |

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

### V1 — Pricing Engine Validation ✅ COMPLETE (April 11, 2026)

> Implemented in `tests/pricing_validation.rs` (commit `4f29352`, batch fix `HestonCfCache`).
> All 4 kill-criterion tests pass in release. `py/validate_pricing.py --rust` green.

**Files changed:** `tests/pricing_validation.rs` (created), `py/validate_pricing.py` (existing, verified)

#### V1a — 10k random BSM batch test ✅  
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

#### V1b — Heston batch speed bench ✅

`tests/pricing_validation.rs` enforces this as a kill-criterion test (`heston_batch_50x10_under_1500us`).
Uses `HestonCfCache::new()` + `GaussLaguerreRule` 32-node: CF computed once per maturity,
50 strikes priced via phase multiplication — passes 1.5ms in release. Original approach
(500 individual `heston_call_carr_madan` calls) was 495ms — 330× too slow.

`benches/heston_pricing.rs` already exists. A `50_strikes × 10_expiries` criterion group can be added:

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

#### V1c — Greeks relative error vs QuantLib ✅
Implemented as `bsm_delta_vs_finite_difference` in `tests/pricing_validation.rs`.
Also covered by `tests/unit/models/test_quantlib_reference.rs` and `test_greeks.rs`.
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

## Phase V4: Live Paper + Statistical Rigor

> Origin: Phase 4 audit, March 22, 2026.  
> Three of five originally-proposed items are valid and buildable.  
> Two items (OOS Sharpe > 2.0, max DD < 15%) had unrealistic thresholds —  
> corrected below based on actual TSLA 2025 data (-46% underlying crash).

---

### V4.1 — Bootstrap Sharpe Confidence Interval  
**Effort:** ~2 hours. No new crates required.

Add to `src/backtesting/metrics.rs`:

```rust
/// Bootstraps the Sharpe ratio CI at `confidence` level (e.g. 0.95).
/// Uses n_bootstrap resamples. Returns (lower, upper) percentile bounds.
pub fn bootstrap_sharpe_ci(
    returns: &[f64],
    n_bootstrap: usize,
    confidence: f64,
    seed: u64,
) -> (f64, f64) {
    let n = returns.len();
    let mut bootstrap_sharpes: Vec<f64> = Vec::with_capacity(n_bootstrap);
    let mut rng = SplitMix64::new(seed);
    for _ in 0..n_bootstrap {
        let sample: Vec<f64> = (0..n)
            .map(|_| returns[rng.next_u64() as usize % n])
            .collect();
        bootstrap_sharpes.push(compute_sharpe(&sample));
    }
    bootstrap_sharpes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let alpha = 1.0 - confidence;
    let lo = bootstrap_sharpes[(alpha / 2.0 * n_bootstrap as f64) as usize];
    let hi = bootstrap_sharpes[((1.0 - alpha / 2.0) * n_bootstrap as f64) as usize];
    (lo, hi)
}
```

**Usage in tests:** CI straddling zero is a *warning* (insufficient data), not a
hard failure. For a 30-day paper period the CI will be wide by design.

**Test to add:**
```rust
#[test]
fn bootstrap_ci_positive_returns_above_zero() {
    let returns: Vec<f64> = vec![0.002; 250]; // flat +0.2% per day
    let (lo, _hi) = bootstrap_sharpe_ci(&returns, 1000, 0.95, 42);
    assert!(lo > 0.0, "CI lower bound should be positive for strongly positive returns");
}
```

---

### V4.2 — Walk-Forward Optimization  
**Effort:** 1–2 days. Requires V2b `run_date_range()` first.

The originally proposed "auto-kill if params drift >10%" is **wrong as written**.
Parameter magnitudes naturally shift with market regime — a 10% drift in a
`min_confidence` param from 0.60 → 0.54 is expected and healthy.

**Correct gate:** Walk-forward *out-of-sample Sharpe < 0.0* for two consecutive
30-day windows → pause live bot and emit alert. This tests whether the model
is still predictive, not whether params changed arbitrarily.

Add to `src/backtesting/`:

```rust
pub struct WalkForwardConfig {
    pub in_sample_days: usize,   // e.g. 180
    pub oos_days: usize,         // e.g. 30
    pub param_grid: Vec<BotParams>,
    pub kill_threshold_sharpe: f64,  // e.g. 0.0
}

pub struct WalkForwardResult {
    pub windows: Vec<WalkForwardWindow>,
    pub mean_oos_sharpe: f64,
    pub degraded_windows: usize,
}

pub fn walk_forward_test(
    engine: &mut BacktestEngine,
    symbol: &str,
    history: Vec<HistoricalDay>,
    config: WalkForwardConfig,
) -> WalkForwardResult
```

**Integration with live bot:** On each 30-day boundary, re-run walk-forward
on the preceding 6-month window. If `mean_oos_sharpe < kill_threshold_sharpe`,
log a critical alert and optionally halt new position-opening.

---

### V4.3 — Diebold-Mariano vs Benchmark   ⚠️ Deprioritised

**Why deprioritised:** DM tests forecast accuracy, not risk-adjusted returns.
It's well-suited for prediction models; less meaningful for a short-vol strategy
with no directional forecast. The correct benchmark test is the bootstrap
Sharpe CI above (does the CI exclude zero?).

**If you want it anyway:** Implement in Python against the 30-day equity curve.
```python
# py/dm_test.py
from scipy.stats import norm
import numpy as np

def dm_test(e1, e2, h=1):
    """Diebold-Mariano test. e1=strategy errors, e2=benchmark errors."""
    d = e1**2 - e2**2
    n = len(d)
    dm_stat = np.mean(d) / (np.std(d, ddof=1) / np.sqrt(n))
    p_value = 2 * norm.cdf(-abs(dm_stat))
    return dm_stat, p_value
```
Use daily `equity_curve_pct_change - benchmark_pct_change` as the loss series.

**No Rust implementation needed.** Do not add `statrs` crate for this.

---

### V4.4 — Prometheus Metrics (optional, operational quality)  
**Effort:** 1 day. Requires adding a new crate.

Add to `Cargo.toml`:
```toml
prometheus-client = "0.22"
```

Expose a `/metrics` HTTP endpoint in `src/metrics/mod.rs` with:
- `dollarbill_trades_total{symbol, side}` — counter
- `dollarbill_position_pnl_pct{symbol}` — gauge
- `dollarbill_circuit_breaker_trips_total` — counter
- `dollarbill_daily_loss_pct` — gauge

**Honest assessment:** This is a quality-of-life feature for production
monitoring. It is **not an acceptance gate** for the bot's validity.
SQLite already provides a full audit log via `TradeRecord` and `PositionRecord`.
Add Prometheus *after* Phase 2 (close logic) and Phase V validation are done.

---

### V4 Corrected Final Acceptance Thresholds

The originally proposed thresholds have been corrected against TSLA 2025 reality:

| Criterion | Proposed | **Corrected** | Rationale |
|-----------|----------|---------------|-----------|
| OOS Sharpe (2025-H2) | > 2.0 | **> 0.5** | Sharpe 2.0 is top-decile hedge-fund; TSLA vol-selling in crash year. Sharpe 1.0 is excellent. |
| Max drawdown (paper 30d) | < 15% | **< 30%** | TSLA had -46% underlying in Feb-Mar; 15% is impossible without multi-leg hedging |
| Live profit factor (30d) | > 1.8 | **> 1.5** | Realistic for short-vol in cooperative regime; 1.8 is achievable but weather-dependent |
| Bootstrap Sharpe CI | — | **CI lower > -0.5** | Not overlapping deeply negative is the gate; zero-crossing in 30d is expected |
| Walk-forward OOS Sharpe | — | **> 0.0 each window** | Two consecutive negative windows → pause bot |
| Audit trail | SQLite | **SQLite sufficient** | `TradeRecord`/`PositionRecord` already cover this; Prometheus is optional |
| All Phase V validations | — | **must pass** | V0–V3b thresholds as specified in Phase V above |

---

## Updated Success Metrics (all phases)

- [ ] `py/validate_data.py` passes with TSLA ann_vol in [50%, 150%]
- [ ] BSM batch: 0/10000 options exceed $0.001 vs QuantLib
- [ ] Heston surface bench: 500 prices in < 1.5 ms
- [ ] Greeks relative error: Δ and ν both < 0.5% vs QuantLib reference
- [ ] `calmar_ratio` computed and > 1.0 on in-sample 2025-H1
- [ ] Out-of-sample backtest (2025-H2) Sharpe > -1.0 (hard pass), > 0.5 (target)
- [ ] Feb-Mar 2025 stress: max drawdown < 35% (realistic, not fantasy)
- [ ] Apr-9 2025 stress: max drawdown < 15% (IV crush is recoverable)
- [ ] Per-regime metrics visible in backtest output
- [ ] 1-day 99% CVaR < 8% of equity (Heston TSLA-like params)
- [ ] 1-year 99% CVaR < 18% (Heston standard params)
- [ ] Bootstrap Sharpe CI lower bound > -0.5 after 30-day paper run
- [ ] Walk-forward OOS Sharpe > 0.0 for at least 2 of 3 windows
- [ ] Live profit factor > 1.5 over first 500 trades
- [ ] No uncharted crash: max daily loss < 5% for any single session
- [ ] Total tests: 660+ after all new validation tests

---

## April 2026 Audit — Priority Fix List

> Source: external code review, April 10, 2026.
> Six claimed flaws were checked against the actual codebase.
> Two were wrong, two were partially wrong, two were correct.

### Confirmed Problems (implement in order)

| Priority | Item | Status | Details |
|----------|------|--------|---------|
| 🔴 1 | **Replace Nelder-Mead with CMA-ES for Heston calibration** | ❌ Not done | `calibrate_heston()` in `src/calibration/heston_calibrator.rs` uses 500-iteration Nelder-Mead. On noisy real chains (TSLA vol regime switching), simplex stagnates at local minima. CMA-ES or differential evolution required. Zero-dependency pure Rust CMA-ES is ~300 lines. |
| 🔴 2 | **Add SVI per-expiry smile fit with butterfly arbitrage check** | ❌ Not done | Current smile fitting: cubic spline (can produce negative butterfly density) or SABR (breaks at extreme strikes). No no-arb enforcement at the surface level. Edge signals derived from an arbitrageable surface are unreliable. SVI parameterization: σ²(k) = a + b(ρ(k−m) + √((k−m)² + σ²)); butterfly check: d²C/dK² ≥ 0. |
| 🟠 3 | **Halve position size in HighVol regime** | ❌ Not done | `RegimeDetector` classifies correctly and gates which strategies fire, but `suggested_size` in `PositionDecision` is NOT reduced when `regime == HighVol`. When realized vol > 40% (TSLA Feb-Mar 2025 reality), the bot was still sizing at normal contract counts. Fix: in `live_bot.rs`, after `pm.can_take_position()`, multiply `qty` by 0.5 (round up) when `regime == MarketRegime::HighVol`. |
| 🟡 4 | **Add vanna, volga, charm to Greeks struct and portfolio aggregation** | ❌ Not done | `Greeks` struct has Δ, Γ, θ, vega, ρ. `HigherOrderGreeks` has speed, zomma, color. Missing: **vanna** = ∂Δ/∂σ = ∂²V/∂S∂σ, **volga** = ∂²V/∂σ² (vega convexity), **charm** = ∂Δ/∂t (delta decay). These matter for multi-leg hedging. All three are closed-form in BSM. Add to `bs_mod.rs` and aggregate in `risk_analytics.rs`. |
| 🟡 5 | **Calibrate slippage params against actual TSLA Feb-Mar 2025 spread behavior** | ⚠️ Structure OK, params not validated | `PanicWidening` and `FullMarketImpact` models exist in `engine.rs`. But `normal_vol`, `panic_exponent`, `cap_multiplier` values are chosen by convention, not fitted to real spread data. Run the stress scenario with `SlippageModel::PanicWidening { normal_vol: 0.25, panic_exponent: 2.0 }` on the TSLA Feb-Mar window and verify the slippage-adjusted P&L is materially worse than the flat model. |

### Claimed Problems That Were Wrong

| Claim | Verdict | Reality |
|-------|---------|---------|
| "Options trading is paper-only simulation" | ❌ False | Live bot submits real Alpaca API options orders. QCOM position is live proof. |
| "No regime detection" | ❌ False | `RegimeDetector` in `src/analysis/regime_detector.rs` classifies 5 regimes per bar; live bot gates strategy weights. Gap is position sizing, not detection. |

### Claimed Problems That Were Partially Wrong

| Claim | Verdict | Reality |
|-------|---------|---------|
| "No portfolio-level Greeks (vanna, volga, charm)" | ⚠️ Partial | First-order Greeks (Δ,Γ,θ,vega) ARE aggregated. Higher-order (speed, zomma, color) exist. Vanna/volga/charm genuinely missing. |
| "Backtester is optimistic (no real bid-ask, weak slippage)" | ⚠️ Partial | Five slippage models including `PanicWidening` exist. Problem is params not calibrated to actual spread observations, not missing architecture. |
