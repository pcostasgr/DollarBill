# Mathematical fixes — October 28, 2026

This change addresses the nine findings in the [audit](MATH_AUDIT_2026-10-28.md).
It does not certify every model or deploy the Linux service.

## Pricing and sizing

Black–Scholes handles expiry and deterministic zero-volatility limits, including
discounted put–call parity. At the deterministic ATM kink, midpoint delta and
zero curvature are explicit numerical conventions. Invalid domains return NaN.
The binomial pricers preserve intrinsic value at expiry, optimize discounted
deterministic exercise at zero volatility, and return NaN for invalid tree
parameters rather than a false zero price.

Sample volatility requires at least two returns; insufficient history returns
zero, while non-positive/non-finite prices and unordered tick times return NaN.
Regime multiplication is capped again after scaling. Invalid sizing inputs and
unaffordable spreads return zero contracts, including the former minimum-one
multi-leg sizing path.

## Live entry checks

The bot no longer uses uncertified legacy performance matrices for filtering.
Each candidate is resolved as one unit, budgeted from its actual OCC strikes,
expiry and leg ratios, and sized for that strategy. The real proposed quantity
is checked against position/allocation limits, a per-trade risk budget, buying
power, the projected marked portfolio and existing invariants before submission.
Zero size stays zero. Uncovered short-call structures, including naked short
straddles, are rejected. Mixed-expiry or malformed structures are rejected by
this conservative entry path. It does not infer equity cover for a new call.

The collateral estimate includes worst expiration intrinsic loss plus estimated
long-leg debit, without crediting uncertain short premium. It is a stress budget,
not broker margin replication or a guaranteed market fill price. Debit estimates
still depend on the supplied volatility; actual fees, slippage and broker margin
remain distinct constraints. Broker order-state reconciliation remains required
after ambiguous outcomes.

Signed option legs and equity shares are projected into a current risk snapshot.
Current marks replace entry Greeks for this projection, and the book is refreshed
before entry decisions and after fills/closes. Missing/stale marks block entries;
marks expire after 120 seconds. Broker account capital/buying power is refreshed
on the clock polling cycle; account data older than 120 seconds blocks entry.
Symbols held outside the subscribed universe therefore require reconciliation
and usable marks before new risk can be added. Equity quantities that this
integer projection cannot represent are rejected.

## Risk units and model boundaries

Delta dollars use each underlying's marked spot. Limits use fractions of account
equity: gross delta dollars, gamma P&L for a 1% spot move, vega/volga P&L for a
0.01 absolute IV move, and one-day charm delta drift under a 1% spot move.
Signed exposures net within each underlying before absolute exposure is summed
across underlyings. Invalid or unmarked books block entry.

The old dimensionally inconsistent VaR proxy is replaced by an explicitly
specified one-day **linear normal stress proxy**. Assumed spot sigma is
20%/sqrt(252), assumed absolute IV sigma is 0.01; spot and IV factors are
independent, while adverse factor exposures align across underlyings. Identical
opposite legs cancel exactly. The existing `var_95`/`var_99` and CVaR fields retain
their API names, but these outputs are conditional stress estimates, not
historically calibrated forecasts or promises of tail coverage. Curvature is
limited separately. Normal tails, fixed shocks, European BSM risk marks and the
UTC expiry-day approximation do not model American assignment or exchange
calendars. Operational validation remains necessary before deployment.

## Performance and historical research

Attribution accepts explicit initial capital (`with_initial_capital`); its
legacy `new()` default is documented as $100,000. Realized-equity drawdown starts
from capital and closes are ordered by exit date. ROI uses that same capital.
Positive profit with no losses gives infinite profit factor. Per-trade Sharpe
and Sortino use a zero hurdle without a daily rate or annualization; zero
denominators have explicit infinity/zero conventions. These remain premium-based
trade-return statistics, not daily mark-to-market NAV metrics. The audit's
original wording about annualization was corrected: the actual defect was the
fixed daily risk-free subtraction.

Both engine entry points normalize historical dates ascending. The CLI labels
its five simple-call threshold variants accurately. `backtest --save` writes a
fresh `models/simple_volatility_matrix.json` plus provenance containing input
hashes, thresholds, default configuration and build identity. It neither imports
legacy rows nor replaces `models/performance_matrix.json`. Running the five
actual named strategies and certifying their performance remains separate
research; old performance reports are not recertified.

## Verification

```text
cargo test --locked --offline --test math_regressions
cargo test --locked --offline --lib alpaca::live_bot::math_tests
cargo check --locked --offline --all-targets --all-features
```

Regression cases cover pricing parity/intrinsic boundaries, insufficient samples,
invalid units, post-regime caps, zero affordable size, signed portfolio risk,
capital-based drawdown, profit-factor edge cases, chronological callbacks,
resolved leg collateral and marked-book projection. Full-suite and release
verification evidence is recorded alongside the completion report.

Completed verification on October 28:

- All-target/all-feature debug check and release build passed.
- The full baseline passed 1,110 unit/integration test executions and 11 doctests;
  21 unit/integration executions and two doctests remain ignored with documented
  exceptions. Counts include duplicated modules across binary targets.
- All three execution scenarios matched repeated artifacts and JSONL/SQLite state.
- The release CLI smoke test produced five correctly named variants and build/input
  provenance while preserving a sentinel legacy matrix in an isolated directory.

The successful baseline used `RUST_TEST_THREADS=1` to avoid local HTTP-mock
timeouts observed in the first parallel run. The 11 affected binary HTTP-mock
tests also passed independently when run serially. Existing strict-Clippy debt
was not part of this mathematical repair and no clean-lint claim is made.

Local evidence: `target/baseline-math-fixes-final-20261028/baseline.json`,
`target/math-final-check.log`, `target/math-final-release.log`,
`target/math-http-recheck.log`, and `target/math-cli-smoke-final/`.
These generated artifacts are excluded from the source commit.
