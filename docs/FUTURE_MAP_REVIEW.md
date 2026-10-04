# Future map review and first milestone

Reviewed 2026-10-04 against commit `58391ea` and the first-milestone changes
in this working tree. Source proposal: `DollarBill_FUTURE_MAP.md` supplied
by the user. This document records an assessment and selected implementation;
the proposal is not a release commitment.

## Assessment

The proposed direction is sound: unify execution and accounting before expanding
the model catalogue. The existing pricing, strategies, risk guards, streaming,
SQLite and CLI/TUI modules support the proposal's capability inventory. Several
claims need tighter qualification:

1. **Move reconciliation and recovery forward.** Phase 7 should be a dependency
   of the first broker adapter, not something introduced after Backtester V2.
   `src/alpaca/execution.rs` now rebuilds signed legs from broker positions at
   startup, but there is no durable local order lifecycle reconciled against
   open orders, cash and assignments. A successful position refresh does not
   establish complete broker/local agreement.
2. **Do not certify the saved performance matrix yet.** In `src/main.rs`,
   `cmd_backtest` runs `run_simple_strategy` with five volatility thresholds and
   labels the results Momentum, Mean Reversion, Breakout, Vol Arbitrage and
   Cash-Secured Puts. Those are variants of the same call-buying routine, not
   executions of the five named strategy implementations. `load_csv_closes`
   returns newest-first data, while this caller passes it directly to an engine
   expecting chronological observations. The October fixes also invalidate
   earlier equity/volatility results. Regenerating the same matrix would
   reproduce misleading labels and chronology. Correct the harness first.
3. **An audit log is not yet an execution journal.** `backtesting/audit_log.rs`
   records regime/sizing snapshots; `persistence/mod.rs` stores mutable positions
   and trade records. Neither reconstructs a complete portfolio from an ordered
   stream of immutable fills and lifecycle events. The new journal below fills
   that gap for the offline foundation, not the existing live bot.
4. **Use decimal cash from the start.** `backtesting/ledger.rs` already uses
   `rust_decimal`, alongside a separate `f64` engine cash balance. A new
   `Money(f64)` would preserve the unit and precision problems. The new domain
   uses decimal USD money and prices, positive quantities and explicit share
   multipliers. Multi-currency and adjusted option contracts remain separate work.
5. **The state machine is a graph.** Filled, cancelled and rejected are alternative
   terminal outcomes, not a chain `Filled -> Cancelled -> Rejected`. A cancelled
   order may retain earlier partial fills. Duplicate execution reports must not
   change cash twice. A broker adapter must normalize cumulative fills and race
   conditions before appending canonical incremental events.
6. **Determinism requires explicit time, ordering and identity.** Existing paths
   still use wall-clock time (for example OCC validation and some Greeks/DTE
   calculations). Seeded numerical routines alone do not establish deterministic
   trading replay. The foundation takes timestamps explicitly, validates ordered
   sequences, uses stable iteration and records build/input hashes.
7. **Replace numeric completion claims with evidence.** The map's 765-test figure
   is historical; `src/main.rs` recompiles modules and duplicates many library
   tests. Count results per target and document ignored cases. Cargo version is
   currently `0.1.0`; `v0.8` through `v1.0` are proposed milestones, not existing
   releases or promises of elapsed time.

## Priorities and acceptance gates

| Priority | Work | Acceptance evidence |
| --- | --- | --- |
| P0 / this milestone | Reproducible offline baseline, source identity, ignored-test inventory | One script archives checks, test logs, input hashes and repeatable scenario artifacts |
| P0 / this milestone | Canonical orders/fills, exact accounting, event replay | Simulation, JSONL replay and reopened SQLite reproduce the same state; invalid/duplicate transitions are tested |
| P0 / next | Repair historical harness identity and chronology; regenerate matrices and reports | Named strategy implementations actually execute; inputs/config/code recorded; no legacy rows silently retained |
| P0 / next | Persist intent before broker submission; reconcile pending/unknown orders and positions | Crash at each lifecycle boundary; restart accounts for every fill and prevents duplicate submission |
| P0 / next | Config validation and schema migrations | Malformed risk config fails startup; upgrades are versioned and tested on old databases |
| P1 | Alpaca adapter and unified risk decisions on the new domain | Existing strategy/risk code runs through both adapters; cumulative fills normalized idempotently |
| P1 | Simulator margin, assignment, expiration and broker-race scenarios | Explicit `assignment_race`, expiry settlement and pending-order recovery tests |
| P1 | Backtester V2, validated quotes, artifacts and statistical research | Historical and paper sessions replay through the same accounting/lifecycle; divergence explained |
| P2 | Portfolio scenarios and operations views | Persisted pre-trade decisions and post-fill invariants, actionable health/status displays |
| Later | Crate splitting, API/web UI, new quant models and live promotion | Stable dependency boundaries and measured needs; sustained paper evidence before live deployment |

The proposal's phase targets should be estimates with gates. Phases 1–3 and the
recovery portion of Phase 7 should progress together in small vertical slices.
Keep SQLite and a single crate until the interfaces have users and stable contracts.

## Implemented first milestone

- `build.rs` and `src/build_info.rs`: machine-readable package, Git revision,
  source SHA-256, compiler and target identity. Source hashing covers code,
  tests, Cargo files and build script, including untracked source files; text
  line endings are normalized for cross-platform checkouts.
- `src/domain/`: provider-independent USD money, prices, quantities,
  equity/standard option instruments, order intents, orders and incremental fills.
- `src/execution/`: one atomic event reducer for lifecycle and fill accounting;
  rejected events leave the prior state intact. Replay rejects schema drift,
  sequence gaps, backwards event time, duplicate execution IDs and overfills.
- `ExecutionVenue` and `SimulatedVenue`: deterministic independent-leg execution
  with bid/ask, per-leg limits, fixed slippage/latency, shared snapshot liquidity,
  partial fills, cancellation, cash checks and a circuit breaker that permits
  reduce-only exits. Matching does not consult wall-clock time or randomness.
- `EventStore`: transactional SQLite append/replay; exact retry is idempotent,
  conflicting retry fails, and SQL triggers reject updates/deletes. The versioned
  `trading_events_v1` table is separate from the existing trading database schema.
- `dollarbill-replay`: offline simulation, machine-readable build identity and
  JSONL replay. It exports scenario/config/run snapshots, events, orders, fills,
  positions, state and accounting metrics; it reopens the database and verifies
  replay before writing its completion manifest.
- `tests/execution_replay.rs` and scenario fixtures: cash conservation, short/long
  reversals, exact fees, partial fills, cancelled inventory, invalid transitions,
  duplicate IDs, stale/crossed quotes, latency/limits, liquidity conservation,
  circuit-breaker exits and SQLite recovery. A four-leg partial execution is
  cancelled and flattened, with repeatable JSON artifacts checked end-to-end.
- `scripts/baseline.py`, `docs/ignored-tests.json` and `docs/baseline.md`:
  reproducible checks and documented exceptions, without requiring credentials.

## Boundaries still open

Phase 3 now extends this foundation with simulator settlement and shared
strategy/risk routing through offline-tested Alpaca adapters. See
[execution-phase3.md](execution-phase3.md) for the current scope and validation
boundary; the original milestone assessment below remains historical context.

This is a tested foundation, not completion of every Phase 0–3 exit gate. The
existing live bot and historical engine have not migrated to the new journal.
At that milestone, no broker adapter, market calendar, adjusted contracts,
margin reservation, automatic assignment/exercise simulation, mark-to-market
performance reporting or live promotion workflow was claimed. Phase 3 adds
the adapter boundary, conservative pending-order reservations and explicit
settlement scenarios described above. Reduce-only/cash checks are not a complete options risk
engine. Per-leg limit prices are not net spread limits. Prices and liquidity are
scenario assumptions, not a validated execution forecast.

The first store validates the full stream during each append and the simulator
copies state for atomic changes; they favor inspectable correctness over high
throughput. Report batches now commit atomically; later work should introduce
validated checkpoints while preserving the same invariants.

## Phase 2: durable journal and recovery

`ExecutionJournal` persists an intent before the caller performs broker I/O,
resumes exclusively from committed events, and ingests normalized broker reports
as atomic batches. Stable report and execution IDs provide idempotent retries;
conflicting identities, unknown orders and cumulative quantities unsupported by
individual executions fail without publishing state. This is a library boundary;
the existing Alpaca bot is not yet its caller.

Events carry validated configuration, strategy, instrument and correlation
context where applicable. Signal generation/rejection, risk decisions and
invariant violations are replayable. Authoritative standard option assignment
and exercise events close option inventory and deliver stock at the strike,
including fees, without modifying unrelated pending orders. Generating these
events from broker activity remains adapter work.

Recovery tests cover a persisted intent followed by restart, a second restart
after partial execution, duplicate reports/fills, cancellation, transactional
rollback, tampered provenance and settlement with an order still pending.

Historical performance matrices, benchmarks and QuantLib outputs remain legacy
artifacts until regenerated by corrected, versioned harnesses. The baseline
manifest records these gates as not completed. No stable-release tag is created
and no change enables live trading.
