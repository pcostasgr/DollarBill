# Reproducible execution baseline

This is the first offline engineering milestone described in
[the future-map review](FUTURE_MAP_REVIEW.md). It verifies the new execution
foundation and existing default tests. It does not certify historical strategy
performance, the ignored calibration gates, or live trading.

## Latest verified results — October 4, 2026

`target/baseline-phase3/baseline.json` records a passed run of 15 checks against
three scenario fixtures. All 1,097 unit/integration test executions and 11
doctests passed; 21 unit/integration executions and two doctests were ignored
with documented exceptions. Test executions include repeated library modules
in the main binary. The focused execution suite passed all 21 tests.

The fixtures cover equity round trips, partial multi-leg execution, and
assignment with accepted timeout recovery and expiry. Repeated JSON/JSONL
artifacts matched byte for byte; JSONL replay and reopened SQLite reconstructed
the same state. Separate all-target/all-feature debug checks and release builds
passed. Local evidence lives under `target/` and is not committed; CI archives
its own baseline output.

### Strict Clippy backlog

`cargo clippy --locked --offline --all-targets --all-features -- -D warnings`
failed on 94 distinct diagnostics after deduplicating repeated target output.
These are warnings promoted to errors by `-D warnings`, rather than failed tests
or ordinary compiler errors. The largest groups were:

| Diagnostic | Count | Typical issue |
| --- | ---: | --- |
| `manual_clamp` | 22 | Chained `.max(...).min(...)` expressions |
| `new_without_default` | 14 | Constructors without a `Default` implementation |
| `too_many_arguments` | 13 | Functions exceeding Clippy's argument threshold |
| `needless_range_loop` | 8 | Index loops where iterators are suggested |
| Other diagnostics | 37 | Casts, clones, documentation formatting and redundant expressions |

Examples include `src/backtesting/engine.rs:1182` (`manual_clamp`) and
`src/backtesting/engine.rs:1583` (`cast_abs_to_unsigned`). Float clamp changes
must preserve intended NaN behavior; lint cleanup should be reviewed rather
than applied indiscriminately. No error diagnostics were reported in
`src/domain/`, `src/execution/`, `build.rs`, the replay binary or its integration
tests. Full structured output is in `target/phase3-final-clippy.jsonl`, with
command output in `target/phase3-final-clippy.log`. The CI lint gate remains
open until the repository-wide backlog is resolved.

## Reproduce from a checkout

Requirements: Rust/Cargo, Python 3 (standard library only), and the normal
platform dependencies already needed to build DollarBill. Cargo.lock is used
for every command. No broker credentials are required.

```text
python scripts/baseline.py --output target/baseline
```

With dependencies cached, add `--offline`. Use a fresh output directory on each
run: the script refuses to overwrite prior evidence. The run includes:

1. `cargo check --locked --all-targets`.
2. Library, binary, integration and documentation tests.
3. Ignored-test listing and validation against `docs/ignored-tests.json`.
4. A compiled `dollarbill-replay` build identity.
5. Two executions of each tracked execution scenario, with byte-identical
   JSON/JSONL artifacts and equal replayed state.
6. SQLite close/reopen and event-replay equality, verified by the CLI itself.

`baseline.json` records command exit codes, compiler/target/source identity,
Cargo.lock and input SHA-256 hashes, artifact hashes and deferred gates. The
manifest is marked failed if a check fails; logs are retained. Wall time and
SQLite storage bytes are not promised to be deterministic. Core scenario
events, positions, accounting and run/config snapshots are deterministic for
the same binary and inputs.

The source ID hashes normalized contents of `src/`, `tests/`, Cargo.toml,
Cargo.lock and build.rs, including newly added source files. Git commit alone
is not the identity of an uncommitted checkout. Compiler and target are recorded
separately. Scenario bytes and configuration have their own hashes. Baseline
script and exception-inventory contents are archived too.

## Run a scenario or replay it

```text
cargo run --locked --bin dollarbill-replay -- build-info
cargo run --locked --bin dollarbill-replay -- simulate --scenario tests/fixtures/execution/partial_multileg_fill.json --output target/partial-fill
cargo run --locked --bin dollarbill-replay -- replay --events target/partial-fill/events.jsonl
```

The simulation command exports:

| Artifact | Meaning |
| --- | --- |
| `run.json` | Completion manifest: code/toolchain/input identity, initial USD cash and execution model |
| `scenario.json`, `config_snapshot.json` | Exact scenario input and parsed execution assumptions |
| `events.jsonl`, `events.sqlite` | Versioned append-only execution events |
| `state.json` | Entire replayed state, including pending/terminal orders and execution IDs |
| `orders.json`, `fills.json`, `positions.json` | Canonical order, incremental fill and position data |
| `metrics.json` | Cash, realized P&L, fees, counts and replay equality |

`run.json` is written last. Its absence means export did not complete. These
outputs contain no synthetic Sharpe or NAV claim: equity-curve/mark-to-market
research reporting belongs to the historical-harness milestone. Monetary JSON
values are decimal strings; quantities use shares for equity and contracts for
standard options. Option cash flows use a multiplier of 100.

The four-leg fixture deliberately fills only `[1, 1, 2, 0]` of `[2, 2, 2, 2]`
contracts, trips the breaker, cancels remaining entry quantities and permits
reduce-only exits. Its final positions are empty and cash is `$100,249.20`
from `$100,000.00`, including `$0.80` in fees. The equity fixture ends at
`$1,019.80` from `$1,000.00`, including `$0.20` in fees. These are accounting
fixtures, not trading performance evidence. Their clock starts at a synthetic
Unix epoch offset; no market-hours claim is made.

## Lifecycle and recovery contract

Created orders are submitted and then accepted or rejected. Accepted orders
may partially fill, fill completely, or cancel. Cancelling retains executed
inventory. A terminal order cannot accept another canonical fill; the future
broker adapter must normalize delayed/cumulative broker reports into their
actual execution order before publication.

Events require consecutive per-run sequences, nondecreasing explicit timestamps,
known schemas and stable execution IDs. Applying an invalid event changes
nothing. The journal accepts an exact repeated append idempotently, rejects a
conflicting payload, and persists each valid append in a transaction. SQL
triggers reject row updates/deletes; this is an integrity contract, not a
tamper-proof security boundary against a database administrator.

The simulator models independent legs with per-leg limits. It shares each
snapshot's available quantity across orders and matches deterministically by
creation time then order ID. It models fixed latency, fixed adverse slippage,
fees, stale/crossed quote rejection and cash availability. It does not model
broker margin, all-or-none net spread orders, corporate actions, an exchange
calendar or probabilistic queue position. Phase 3 adds explicit assignment,
exercise and expiry scenarios; see [execution-phase3.md](execution-phase3.md).
The production bot has not migrated to the shared venue path yet.

## Test inventory and exceptions

The baseline exports an annotated-source inventory with mathematical, strategy,
risk, execution, persistence, broker-contract, backtesting, portfolio,
market-data, integration, incident-regression and supporting categories.
Mixed integration files can exercise several categories; the file-level label
is organizational, not a coverage metric. Test-run logs are authoritative for
executed cases. Property tests generate many cases beyond source declarations.

There are 14 explicitly ignored source tests and two illustrative ignored
doctests at this milestone. Seven source cases need network/account access;
seven are opt-in numerical/historical acceptance gates. Their precise reasons
are in [ignored-tests.json](ignored-tests.json). These are exceptions, not
passing evidence. `main.rs` still recompiles library modules, so its unit-test
run duplicates cases and ignored counts; do not sum those as unique coverage.

To evaluate an ignored numerical case deliberately:

```text
cargo test --locked --release --test pricing_validation heston_on_tesla_crash_period -- --ignored --exact
```

No blanket `--ignored` execution is part of the baseline: that would mix broker
access with numerical checks. Benchmarks and QuantLib validation need their own
archived release-build/environment evidence and remain deferred gates.

## Phase 2 journal API

For the subsequent simulator and Alpaca adapter work, see
[Phase 3 execution](execution-phase3.md).

Use `ExecutionJournal::start(store, run_id, manifest, source)` for a new run,
or `resume(store, run_id, source)` after interruption. Await `prepare_order`
before sending the identical client order ID to a broker. Pass normalized
`BrokerReport` values to `ingest_report`; await success before publishing the
new portfolio. Reports must contain actual incremental executions with stable
IDs. Cumulative average prices cannot reconstruct individual execution fees
or prices and must not be substituted. Unknown orders, out-of-order terminal
updates and contradictory reports require external reconciliation; the journal
fails closed instead of guessing. It does not itself send or retry broker orders.

Each report's lifecycle events, new fills and receipt share one SQLite
transaction. Exact reports and repeated executions are idempotent across
restart. Conflicting identities or cumulative totals fail atomically. One
writer should own a run; concurrent stale writers must reload on conflict.

`record` accepts signal, risk, invariant and authoritative settlement events.
Every event has validated configuration and applicable strategy/instrument/
correlation context. Portfolio-wide events have no single strategy or instrument;
multi-leg orders carry their instruments in the payload. Assignment/exercise
supports standard 100-share contracts only. Option premium is realized when
the option closes and delivered stock takes strike-price basis; jurisdictional
tax-basis adjustments and corporate actions are not modeled. Phase 3 adds
explicit simulator expiry processing; see [execution-phase3.md](execution-phase3.md)
for its scenario timing and settlement limits.
Authoritative settlement may produce negative cash; recording an actual broker
fact is separate from pre-trade buying-power enforcement.

Run the recovery and accounting checks with:

```text
cargo test --locked --offline --test execution_replay
```

The production live bot has not yet migrated to this API.

Strict linting is a separate gate: `cargo clippy --all -- -D warnings`
currently fails on existing warnings across pricing, calibration, portfolio
and risk modules. A passing baseline manifest certifies its listed commands,
not a clean Clippy run or every CI job.

## Historical artifacts awaiting regeneration

`models/performance_matrix.json`, `BACKTEST_REPORT.md` and historical benchmark
claims predate the corrected equity/volatility path. In addition, the current
`backtest --save` command labels threshold variants as separate strategies and
does not normalize its newest-first CSV input. Do not use that command as the
certification step for this baseline. Correct those harness issues, attach
input/config/code metadata, then regenerate named strategy results in a separate
milestone. No existing matrix is silently overwritten by the baseline script.
