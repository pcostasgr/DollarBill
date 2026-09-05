# Order Path Contract

Per Adversarial Hardening Plan §3 (`TestFiles/DollarBill_Adversarial_Hardening_Plan.md`).
Pure logic lives in [`src/order_path.rs`](src/order_path.rs); network I/O
(`resolve_single_leg_occ`, `submit_options_order`, `post_order_safe`) stays in
[`src/alpaca/client.rs`](src/alpaca/client.rs) as a thin async façade.

```
Signal
  -> validate_signal        (step 1, pure)
  -> check_risk_guards      (step 2, pure)
  -> size_position          (step 3, pure)
  -> build_order            (step 4, pure)
  -> resolve_single_leg_occ (step 4b, network — AlpacaClient)
  -> validate_occ_symbol    (step 5, pure)
  -> generate_client_order_id (step 6, pure)
  -> submit / post_order_safe (step 7, network — AlpacaClient)
```

Every pure step returns `Result<_, OrderPathError>` with an explicit variant.
No step uses `unwrap_or`/silent `continue` to mask a failure.

## Step 1 — `validate_signal`

- **Pre-conditions:** `confidence` is the strategy's reported confidence in `[0,1]`.
- **Post-conditions:** `Ok(())` iff `action` is actionable (not `NoAction`/`ClosePosition`)
  and `confidence >= min_confidence`.
- **Errors:** `SignalRejected(reason)`.
- **Invariant:** never returns `Ok` for `NoAction`. `ClosePosition` always routes
  through `manage_open_positions`, never through the entry order path.

## Step 2 — `check_risk_guards`

- **Pre-conditions:** none.
- **Post-conditions:** `Ok(())` iff `risk::guards::check_all` returns `Allow`.
- **Errors:** `RiskHalted(reason)` — carries the guard's own message verbatim.
- **Invariant:** this function can never diverge from `check_all`, the same
  guard used by backtesting — no separate/duplicated threshold logic.

## Step 3 — `size_position`

- **Pre-conditions:** `suggested_qty` from the portfolio sizer.
- **Post-conditions:** `Ok(qty)` with `qty >= 1`.
- **Errors:** `InvalidSizing(reason)` for zero/negative sizer output. A bad
  sizer output is never silently clamped to a default quantity.

## Step 4 — `build_order`

- **Pre-conditions:** `qty >= 1`.
- **Post-conditions:** an `OptionsOrderRequest` with one leg (single-leg
  strategies) or N legs (multi-leg), every leg symbol produced by
  `AlpacaClient::occ_symbol`.
- **Errors:** `BuildOrderFailed(reason)` for signals with no options
  representation (currently only `NoAction`/`ClosePosition`, already filtered
  by step 1, but this stays independently defensive).

## Step 4b — `resolve_single_leg_occ` (network, `AlpacaClient`)

- **Pre-conditions:** an OCC symbol built by step 4.
- **Post-conditions:** the nearest actually-listed contract symbol, or the
  original symbol unchanged if parsing fails or the API is unavailable.
- **Error cases:** network failure / empty contract list — both fall back to
  the original symbol rather than failing the whole order; the caller must
  re-validate with step 5 after resolution since the resolved symbol may
  differ from the generated one.

## Step 5 — `validate_occ_symbol`

- **Pre-conditions:** `occ` as produced by step 4, ideally after step 4b.
- **Post-conditions:** `Ok(OccParts)` only for symbols that are both
  structurally parseable *and* pass the extra checks Alpaca enforces
  server-side: root is 1–6 alphabetic characters, strike > 0, expiry not in
  the past.
- **Contract:** `validate_occ_symbol` must **never** return `Ok` for a symbol
  that Alpaca would reject with 422 (asset not found / malformed symbol).
  Covered by the `proptest_pipeline::pipeline_never_panics` fuzz test in
  `src/order_path.rs`, which asserts rejection for adversarial roots/strikes.
- **Errors:** `OccRejected(reason)`.

## Step 6 — `generate_client_order_id`

- **Pre-conditions:** none.
- **Post-conditions:** a string unique per call (monotonic counter + ms
  timestamp), stable and safe to log verbatim for replay/audit.
- **Contract:** never reused across submission attempts.

## Step 7 — `submit` / `post_order_safe` (network, `AlpacaClient`)

- **Pre-conditions:** a fully built, validated order with a `client_order_id`
  already set.
- **Post-conditions:** `Ok(Order)` on any HTTP success status.
- **Error cases:**
  - 429/502/503/504 → retried with exponential backoff (up to `MAX_RETRIES`).
  - Any other non-2xx (403 insufficient buying power, 422 asset not found,
    etc.) → `Err` immediately, no retry, response body included in the error.
  - Network timeout / connect error → `Err` immediately, **never retried**,
    because the order's fate is ambiguous (Alpaca may have already received
    it). The error message tells the caller to check `/v2/orders` by
    `client_order_id` before resubmitting.
- **Invariant:** the same `client_order_id`, generated once per attempt in
  step 6, is never regenerated for a retried attempt — this is what makes
  Alpaca's server-side dedup effective and prevents a duplicate position from
  a retried submission.

## Independent Review Checklist (Adversarial Hardening Plan §3.3)

1. **Alpaca error codes** — 429/5xx retried with backoff; 403/422/other →
   immediate `Err` with body text (`AlpacaClient::request_with_retry`,
   `post_order_safe`). Covered by mock-HTTP tests in `src/alpaca/client.rs`
   (`mock_http_tests` module).
2. **Partial multi-leg fills** — not detected by the order path itself;
   `manage_open_positions`'s roll-imbalance guard in `live_bot.rs` closes any
   leg whose filled qty doesn't match the expected qty. Covered by
   `tests/integration/test_kill_switches.rs` (invariant layer) — no HTTP-level
   partial-fill simulation yet.
3. **Duplicate `client_order_id`** — never regenerated on retry (see Step 7
   invariant); network-ambiguous failures are never retried, so the bot never
   blindly resubmits with a fresh id. Covered by
   `no_retry_on_ambiguous_connect_error` in `src/alpaca/client.rs`.
4. **Assignment race (risk check vs. submit)** — not yet closed; there is a
   window between `check_risk_guards`/`manage_open_positions` and the actual
   submit where an assignment could land. Mitigated after the fact by the
   post-fill `assert_invariants` check in `live_bot.rs`, which flattens all
   risk and alerts on violation, but this is reactive, not preventive.
5. **New position after breaker tripped** — `check_risk_guards`/`check_all` is
   called before every entry; `circuit_broken` is also checked directly in
   the live-bot tick loop before signal generation. Covered by
   `scenario1_equity_drop_exceeds_daily_limit_halts_entries` and
   `scenario6_circuit_broken_and_long_present_is_double_violation`.
6. **Long-premium position while `block_long_premium=true`** — enforced by
   `manage_open_positions`'s force-close gate and by `assert_invariants`'s
   `NoNakedLongPremium` invariant (defense in depth: two independent checks).
7. **Max-loss under-estimation** — `assert_invariants`'s `MaxLossWithinLimit`
   and `position_management`'s concentration check both use
   `strike × qty × 100`, never raw premium.
8. **Reachability from both bots** — `manage_open_positions` and
   `assert_invariants` are called identically from `live_bot.rs`;
   `examples/personality_based_bot.rs` uses the same `risk::guards` module
   but does not yet call `manage_open_positions` (still open — see repo
   memory `dollarbill-roadmap-status.md`).

**Status:** items 1, 3, 5, 6, 7, 8 (partial) covered by tests referenced above.
Items 2 and 4 remain open — no HTTP-level partial-fill simulation, and the
assignment-race window is only closed reactively, not preventively.
