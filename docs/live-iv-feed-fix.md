# Live IV feed repair — October 29, 2026

The server export showed successful option-chain requests followed by failures
from a separate Yahoo closing-price request. Live IV now uses the underlying
spot and `regularMarketTime` from the dated chain response. The obsolete
closing-price request is removed from this path and the live-pricer example.
This does not change the separately configured Heston recalibration spot source.

## Eligibility and freshness

`bot_runtime.live_iv_policy` in `config/trading_bot_config.json` defaults to:

```json
{
  "min_remaining_secs": 86400,
  "max_spot_age_secs": 120
}
```

Expiry selection sorts/deduplicates dates and indexes only eligible future
expiries. Past dates and the current UTC date are excluded. Yahoo expiration
date markers are conservatively treated as UTC midnight, not as exact exchange
settlement instants. Same-day contracts are not supported by this path. A
minimum 24-hour remaining horizon can also exclude the next calendar date.
The horizon is checked again after fetching; maturity is never clamped positive.
Returned chain and individual contract expirations must match the requested date.

Missing, nonpositive, stale or more than 30-second future-dated spot timestamps
are rejected. Invalid/crossed quotes and unsupported price/IV bounds are rejected
before an observation can populate the live cache. Error counts distinguish
illiquid contracts, missing ATM candidates and failed IV inversions. The quote
and expiry timestamps are logged with the source on successful IV refresh.
Yahoo does not provide bid/ask update timestamps in this path: fresh underlying
spot does not certify option-quote freshness, synchronization or execution prices.

## Failure behavior

HTTP requests have a 20-second timeout. HTTP failures, API errors, invalid JSON
and empty results try the second Yahoo host; diagnostics identify the failing
stage without exposing the authentication crumb. Failed IV refreshes back off
per symbol for 60, 120, 240, 480 and then at most 900 seconds (with the existing
60-second background poll determining the actual retry time).

Fresh IV expires after the existing 15-minute cache TTL. Stale observations are
retained for diagnostics, but are never returned as live IV on refresh failure.
All current registry entry strategies consume market IV, so new entries pause
when it is unavailable. Realized volatility remains separate and position
management executes before the entry gate. No historical/model value is silently
substituted for market IV. Closing/rolling risk follows existing management rules.

Regression tests cover the server's expired/short-date pattern, mismatched
expirations, stale/future/missing spot data, quote bounds, HTTP fallback, error
redaction, retry scheduling, and separation of stale from fresh cache data.

## Verification

- `cargo test --locked --offline -- --test-threads=1`: 1151 test executions passed,
  zero failures, 23 ignored, including doctests. Counts include duplicated
  library modules in binary targets. All calibration stress cases completed.
- `cargo check --locked --offline --all-targets --all-features`: passed with existing warnings.
- Focused market-data tests: 17 passed, 4 existing network tests ignored.
- `git diff --check`: passed.

Full local output: `target/feed-full-suite.log`; focused checks:
`target/feed-regression-tests.log` and `target/feed-check.log`. Generated logs are
not source artifacts. Live Yahoo availability on the server has not been tested
by this local run; the external network tests remain ignored.
