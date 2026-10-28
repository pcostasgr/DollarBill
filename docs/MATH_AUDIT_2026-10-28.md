# Mathematical audit — October 28, 2026

The findings below describe the pre-fix checkout. Subsequent fixes and
validation are tracked in [math-fixes.md](math-fixes.md).

Reviewed checkout `1e378b6`. This is a targeted source audit and numerical
counterexample exercise, not certification of every pricing model or live path.
No broker calls were made and no trading parameters or implementation were
changed. Library compilation passed with `cargo build --locked --offline --lib`.
Local numerical probe output is in `target/math-audit-results.txt`; source is
`target/math_audit_probe.rs`. Those scratch artifacts are not tracked by Git.

## Priority findings

### 1. P1 — Short-straddle buying-power estimates default to zero

`src/alpaca/live_bot.rs:46–71` handles selected structures in `estimate_margin`
but omits `SellStraddle`. It reaches `_ => 0.0`, even though
strategies generate this action. The check at line 855 only blocks positive
estimates exceeding remaining buying power, and the post-fill decrement uses
the same estimator. These structures therefore bypass this local reservation
check. Other broker checks and post-fill invariants still exist; this is not a
claim that Alpaca will accept unlimited orders.

Remedy: make supported entry actions exhaustive, reject unsupported actions,
and compute collateral from actual resolved legs, quantity and portfolio
coverage before submission. A post-fill unbounded-risk check is too late to
serve as the entry check.

### 2. P1 — The live portfolio manager never receives the open book

`src/alpaca/live_bot.rs:84–88` creates the manager and syncs equity/buying power
once. There is no `pm.update_positions(...)` call in this function. Consequently
the manager's current-position Greek checks and `pm.get_portfolio_risk()` operate
on its empty book, despite the separate tracked positions used by management
and invariants. The displayed manager-derived Greeks cannot establish actual
portfolio risk. Separate post-fill invariants do not repair this calculation.

Remedy: construct a current, signed, leg-level portfolio projection after
startup reconciliation and every fill/close, mark it using current inputs, and
refresh account state with an explicit policy.

### 3. P1 — Delta exposure and VaR have incompatible units

`src/portfolio/risk_analytics.rs:127` converts share delta to dollar exposure by
multiplying by a hard-coded 100 instead of each underlying's spot. The risk
limit at line 198 then divides share delta by dollar equity. Gamma likewise
needs a stated spot shock before normalization into account-loss units.

Numerical probe: a one-contract option with delta 0.5 reports exposure $5,000
at both spot $100 and spot $1,000. The respective delta-dollar exposures are
$5,000 and $50,000. Positions contain `entry_spot`, but a current-risk projection
needs current spots, not just entry prices.

At lines 180–191, VaR uses `vega * option market value / 100`, then sums squared
contributions independently. It does not specify an IV shock distribution,
horizon or covariance. This is not a dimensionally justified dollar volatility
estimator and discards offsetting exposure. Identical +1/-1 contracts with
identical Greeks produced delta 0 but VaR95 $46.527626; their net payoff and
first-order sensitivities cancel exactly.

Remedy: normalize Greeks through explicit spot/IV/time shocks and calculate
P&L scenarios or covariance-aware signed factor exposure. Label approximations
with their horizon and assumptions. Recompute Greeks rather than reusing only
entry Greeks for ongoing risk.

### 4. P1 — Regime adjustment can exceed the position cap

`src/portfolio/position_sizing.rs:86–98` first applies the cap in `calculate_size`,
then multiplies by the regime factor with no final cap. `LowVol` multiplies by
1.8. The backtest regime pipeline uses this method.

Probe: account $100,000, max position 10%, option price $1, fixed fraction 10%:
base size 100 contracts; LowVol size 180 contracts, costing $18,000. The same
sizer's `validate_position(180, 1.0)` returns false.

Remedy: apply all ceilings after regime adjustment; retain zero when there is
no affordable size. Also reject non-finite/non-positive price and account
inputs: `validate_position(1, -1.0)` currently returns true.

### 5. P1 — Pricing boundaries can erase intrinsic value or return NaN

`src/models/american.rs:80–103` calculates binomial transition probabilities
before handling expiry and zero volatility. At either boundary `u == d` and
the probability is non-finite; the routine returns zero as if it were a price.

Probes with S=120, K=100:

| Case | Returned | Required property |
| --- | ---: | --- |
| American call at expiry | 0 | Intrinsic value = 20 |
| American call, one year, zero volatility | 0 | Price >= immediate exercise value 20 |

`src/models/bs_mod.rs:70,115` also divides by volatility without a zero-volatility
branch. S=K=100, T=1, r=q=sigma=0 produces NaN price and gamma rather than a
finite zero price (the Greek at the kink needs a documented convention).

Remedy: handle expiry and deterministic zero-volatility limits explicitly;
validate model domains and tree configuration. Report invalid parameters as
errors rather than a tradable zero valuation. Add intrinsic, parity, finiteness,
and monotonicity boundary tests.

### 6. P1 — Historical strategy results have invalid identity and chronology

`src/main.rs:207–238` labels five threshold variants of `run_simple_strategy`
as five different named strategies. It passes the CSV loader's newest-first
output directly to a forward-iterating engine. These are not executions of
the five strategy implementations. Historical direction is reversed and
"past" windows can contain observations from the actual future.

This was already documented in `FUTURE_MAP_REVIEW.md` and remains present.
The live bot loads recommendations derived from the performance matrix, so
mislabelled research results can influence strategy filtering too.

Remedy: normalize chronological order at the harness boundary, validate strict
dates, execute actual strategy implementations with past-only inputs, and
regenerate provenance-tagged results. Do not certify existing saved rows.

## Additional confirmed defects

### 7. P2 — Sample volatility divides by zero for two prices

Both daily and tick volatility routines in `src/models/bs_mod.rs:146–198` allow
two samples, yielding one return. Sample variance divides by `n_returns - 1`,
which is zero. `compute_historical_vol(&[100.0, 101.0])` returns NaN. Invalid
prices and timestamp order also need validation. The current live path has
additional sample-count/finite gates, so this API defect alone does not prove
that two-price NaN reaches an order.

Remedy: require at least two valid returns, define insufficient-history behavior,
and test non-positive prices, non-finite inputs and timestamp order.

### 8. P2 — Strategy performance has incorrect edge cases and denominators

`src/portfolio/performance.rs:121` leaves profit factor at zero for a profitable
book with no losses. Two closed trades earning $100 and $50 produced profit
factor 0; it should be infinite or explicitly undefined, not zero.

The same function builds "equity" from cumulative P&L starting at zero (line
80), then computes percentage drawdown against that P&L peak. A $100 gain
followed by a $50 loss on $100,000 capital is a $50 / $100,100 drawdown, about
0.04995%, not 50%. ROI also silently assumes capital of $100,000 (line 140).
Its per-trade Sharpe/Sortino subtract a fixed daily risk-free rate despite
variable holding periods. This differs from the daily-equity metrics
implementation in `backtesting/metrics.rs`.

Remedy: accept actual capital and dated equity, use consistent return frequency,
and represent zero-loss/zero-variance ratios explicitly.

### 9. P2 — Live pre-trade validation checks a different quantity and strategy

`src/alpaca/live_bot.rs:768–784` checks one contract for `actionable[0]` using a
rough 30-day premium, then submits `decision.suggested_size.max(1)` for every
actionable strategy. Allocation validation therefore does not assess the actual
proposed quantity/structure, and different strategies share the first strategy's
decision. Zero suggested size is promoted to one. Some independent margin and
post-fill controls remain, but they do not establish this gate's correctness.

Remedy: resolve each candidate's actual legs, compute its own feasible quantity,
and validate incremental portfolio exposure and allocation before submission.

## Follow-up order

1. Repair live action coverage, portfolio synchronization and per-order checks.
2. Define risk units and replace the VaR proxy with a specified loss model.
3. Enforce final sizing ceilings and pricing boundary contracts.
4. Repair historical identity/chronology and regenerate research artifacts.
5. Consolidate performance metrics and validate numeric input domains.

Existing passing tests do not negate these counterexamples. This audit did not
complete independent numerical validation of Heston, SABR, calibration or the
new execution reducer. No claim of mathematical correctness is made for
unreviewed paths, and no risk limits were relaxed.
