# Phase 3: execution simulator and shared routing

The simulator and `AlpacaVenue` implement `ExecutionVenue`. Existing
`TradingStrategy::generate_signals` implementations feed `signal_intent`, then
`route_intent` runs the same pre-submit checks for either venue. The integration
test uses the existing Momentum strategy unchanged, submits identical intents,
books simulator quotes and Alpaca-shaped incremental multi-leg executions, and
checks identical orders and portfolio accounting.

## Execution behavior

The simulator supports market and per-leg limit orders, bid/ask, adverse fixed
slippage, configurable latency, finite shared snapshot liquidity, partial fills,
rejections, cancellations and independent execution of up to four legs. Matching
is FIFO with client ID as a tie-break, not price priority. Duplicate or
out-of-order instrument timestamps cannot replenish consumed liquidity. The
model requires distinct millisecond snapshot timestamps. Cash is checked again
at execution, including fees on sells. The independent-leg model deliberately
exercises legging risk; it does not claim to reproduce Alpaca's native complex
order matching.

`timeout_after_acceptance` lists client order IDs for deterministic ambiguous
acknowledgements. The first submission records acceptance and an unknown-outcome
event, then returns an error. Polling finds the accepted order; retrying the same
intent cannot create a second order. An altered intent under that ID fails.

`settle` injects explicit early assignment or exercise, including fees, while
unrelated pending orders remain intact. `expire` accepts an explicit settlement
spot and positive exercise threshold. It cancels orders touching the expired
contract, closes out-of-the-money inventory at zero, and delivers 100 shares per
in-the-money contract through assignment/exercise. Processing requires the UTC
day after expiry; this is a scenario boundary, not an exchange calendar or cutoff
model. Duplicate expiry fails. Intraday exercise cutoffs, contrary exercise
instructions, adjusted contracts and corporate actions are outside this model.

The CLI supports `submit_timeout`, `settle` and `expire` actions alongside the
existing submit/quote/cancel/breaker actions. The
`assignment_timeout_expiry.json` fixture replays early assignment with a pending
equity order, an accepted timeout and idempotent retry, then contract expiry.

## Shared risk path

Always use `route_intent` for strategy submissions; direct venue methods are
execution primitives, not a replacement for pre-trade risk. The router records
signal and risk decisions, rechecks quote age at submission, applies existing
daily drawdown/trade-cap guards, checks reduce-only capacity, and reserves
remaining quantities on pending orders against supplied buying power.

Reservation uses long premium/notional, 150% notional for short equities, and
strike notional for short options without spread offsets. This is a conservative
stress budget, not broker margin replication; short calls retain unbounded risk.
The caller supplies current account buying power before these pending-order
reservations and current equity. Fees, execution gaps, correlated positions and
broker-specific margin require additional account/risk controls. Fresh quotes
are required for pending instruments too; missing information fails closed.

Risk checks and venue calls assume one serialized owner per account/run.
Reduce-only orders bypass entry breakers but still require fresh quotes and
unreserved inventory. Strategy closes require an explicit position-derived
reduce-only intent; a strategy's numeric position ID alone is insufficient.

## Alpaca boundary

`AlpacaVenue::paper` and `AlpacaVenue::live` explicitly select the endpoint and
credentials; no environment flag activates either. Both use the same adapter
implementation. `AlpacaTransport` permits offline contract tests. No test here
contacts Alpaca, and the existing production bot is not switched to this path.

Intents and submission events are durable before POST. An HTTP failure records
an unknown outcome. Retrying an existing ID performs lookup only, even after a
restart. Missing lookup results require reconciliation; the adapter never guesses
that it is safe to submit again. Single-leg market/limit and native multi-leg
market requests preserve quantities and exact OCC strikes. Per-leg limits on a
multi-leg order are rejected because they cannot faithfully become a net limit.

Acknowledgements and cancellation requests do not book fills. `poll` returns the
last journal-confirmed state; `reconcile_ack` refreshes acceptance, while
`on_trade_update` applies incremental executions and final statuses. Supply the
actual trade update and an explicit execution-ID-to-fee map. Unknown fees fail
instead of silently becoming zero. Cumulative average prices are never used as
incremental execution prices. Unsupported statuses require reconciliation.

The request mapping follows Alpaca's [order documentation](https://docs.alpaca.markets/us/docs/working-with-orders).
Single-leg and per-leg execution normalization follow its
[trade update examples](https://docs.alpaca.markets/us/docs/websocket-streaming).
Stream subscription/reconnection, historical gap recovery, fees reconciliation,
broker account seeding and deployment into the live bot remain integration work.
The shared strategy/risk gate is verified offline against the transport contract;
real-account paper acceptance and operational live readiness are not claimed.

## Verification

```text
cargo check --locked --offline --all-targets --all-features
cargo build --locked --offline --release --all-targets --all-features
cargo test --locked --offline --test execution_replay
python scripts/baseline.py --offline --output target/baseline-phase3
```

The focused suite includes partial four-leg execution, accepted timeout,
duplicate IDs, assignment with pending orders, stale-at-submission quotes,
breaker during partial execution, pending buying-power reservations, all four
stock-delivery directions at expiry, worthless expiry, and a fill arriving while
cancellation is pending. The baseline checks repeated fixture artifacts and
SQLite/JSONL replay equality. Strict Clippy remains a separate repository-wide
gate with existing failures; do not interpret build success as lint success.

Verification completed on 2026-10-04: all-target/all-feature debug checking and
release building passed. The offline baseline passed 1,097 unit/integration test
executions (including duplicated library modules across targets), plus 11
doctests. There were 21 ignored unit/integration executions and two ignored
doctests; the inventory records their exceptions. All 21 focused execution
tests passed. Each of the three fixtures produced identical repeated artifacts
and matching JSONL/SQLite replay state. Evidence is stored in
`target/baseline-phase3/baseline.json` and its adjacent logs and artifacts.
Strict all-target/all-feature Clippy failed on existing module diagnostics;
none were reported in the new domain/execution modules, replay binary, build
script or execution integration tests. Broker paper acceptance and live
integration remain unverified.
