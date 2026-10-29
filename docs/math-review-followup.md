# Math review follow-up — October 29, 2026

The faulty legacy Heston integration has been removed. Its public Carr-Madan
names remain compatibility aliases to corrected Lord-Kahl GL-128 pricing.
Calibration, synthetic calibration data and signal pricing share the same node
count. This changes prices and calibration results; historical benchmarks and
backtests using the retired implementation are not recertified.

The independent QuantLib 1.41 fixture uses dated European options, Actual/365
Fixed and adaptive integration (1e-10 requested tolerance). It covers 7, 30 and
365 days, strikes 70, 85, 100, 115 and 130, with classic and stressed parameters.
Tests show the 32/64/128 node errors and require the production rule to agree
within $0.0001 per share. This is a bounded regression grid, not a proof for
arbitrary parameters. Regenerate with `python py/generate_heston_grid.py`.
The old high-vol-of-vol reference was inconsistent with its stated T=0.5;
QuantLib at exact half-year maturity gives 8.593019474854358, not 8.5568.

Calibration and surface extraction now share a bracketed Newton solver with
bisection fallback, finite-input checks, European price bounds, and explicit
rejection when a root lies outside the supported volatility bracket [0,10].
At the intrinsic lower bound, zero volatility is the return convention.

Portfolio analysis keeps share delta and separately carries marked dollar
delta for equity-fraction limits. Position-management alerts also use dollar
delta, include equity positions and positions queued for exit, and compare the
actual exposure against equity before emitting an alert. Position management
uses the supplied simulation date for option expiry calculations.

Fitted-chain residuals remain research signals. Out-of-sample predictive value
has not been established. BSM Greeks, the approximate normal CDF and the
calibrator's soft Feller preference remain explicit model limitations. No
live orders, deployments or historical performance certification are included.

## Verification

The complete serial suite finished successfully on October 29, 2026:

```text
cargo test --locked --offline -- --test-threads=1
```

- 1120 unit/integration test executions passed; 21 remain ignored.
- 11 doctests passed; 2 remain ignored.
- No test failures. Counts include duplicated modules across binary targets.
- All four lengthy calibration cases completed; the integration binary took about seven minutes.
- A dashboard cleanup race was fixed by explicitly awaiting TradeStore connection closure
  after refreshes and before deleting the scratch database. The full dashboard tests passed.
- `cargo check --locked --offline --all-targets --all-features` passed earlier with existing warnings.
- The independent fixture was regenerated with locally installed QuantLib 1.41.

The complete output is in `target/math-review-full-suite.log` (generated local evidence).
GL-128 increases calibration work relative to GL-32. Ignored tests were not executed,
and this verification does not establish out-of-sample trading performance.
