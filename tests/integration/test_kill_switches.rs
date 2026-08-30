/// Kill-switch scenario tests (§1.3 from the Adversarial Hardening Plan).
///
/// Each test exercises one specific failure mode identified from the July trading
/// incident post-mortem.  No actual Alpaca API calls are made; all tests use the
/// pure-Rust risk/invariant layer with constructed state.
///
/// Scenarios covered:
///   1.  Equity drops ≥ daily drawdown limit → circuit breaker must trip.
///   2.  Alpaca returns incomplete Account JSON → zero-default must NOT
///       bypass position limits (invariant fires on artificial state).
///   3.  Partial multi-leg fill → imbalanced state is an invariant violation.
///   4.  Same `client_order_id` submitted twice → should not create a duplicate
///       unhedged long position (checked via invariant layer).
///   5.  DTE=0 short put with matching assignment activity → must remain closeable.
///   6.  Simultaneous circuit-breaker + new long position → invariant fires.
///   7.  Circuit breaker once tripped stays tripped unless manually reset.
use dollarbill::risk::invariants::{assert_invariants, BotState, InvariantPosition, Invariant};
use dollarbill::risk::guards::{DailyRiskLimits, check_all};
use dollarbill::alpaca::occ::parse_occ;
use std::collections::HashSet;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn clean_state() -> BotState {
    BotState {
        positions:              vec![],
        equity:                 100_000.0,
        start_of_day_equity:    100_000.0,
        was_circuit_broken:     false,
        circuit_broken:         false,
        max_risk_capital_pct:   0.04,
        max_daily_drawdown_pct: 0.05,
        block_long_premium:     true,
        protected_equity:       HashSet::new(),
    }
}

fn occ_put(root: &str, yymmdd: &str, strike: f64) -> String {
    format!("{}{}P{:08.0}", root, yymmdd, strike * 1000.0)
}

fn short_put_pos(symbol: &str, occ: &str, strike: f64) -> InvariantPosition {
    InvariantPosition {
        symbol:       symbol.to_string(),
        occ_symbol:   Some(occ.to_string()),
        qty:          -1.0,
        current_mark: strike * 0.02, // 2% of strike ATM proxy
    }
}

fn long_put_pos(symbol: &str, occ: &str, mark: f64) -> InvariantPosition {
    InvariantPosition {
        symbol:       symbol.to_string(),
        occ_symbol:   Some(occ.to_string()),
        qty:          1.0,
        current_mark: mark,
    }
}

// ── Scenario 1: Equity drawdown trips circuit breaker ─────────────────────────

#[test]
fn scenario1_equity_drop_exceeds_daily_limit_halts_entries() {
    // Simulate equity falling from $100k to $94k (6% drawdown, limit is 5%)
    let limits = DailyRiskLimits {
        max_daily_drawdown_pct: Some(0.05),
        max_daily_trades:       None,
    };
    let start_equity   = 100_000.0;
    let current_equity =  94_000.0; // 6% down
    let action = check_all(start_equity, current_equity, 0, &limits);
    assert!(
        action.is_halt(),
        "circuit breaker should halt new entries when drawdown ({:.1}%) > limit (5%)",
        (start_equity - current_equity) / start_equity * 100.0
    );

    // Symmetrically: 4% drawdown should still allow entries
    let mild_equity = 96_000.0;
    let mild_action = check_all(start_equity, mild_equity, 0, &limits);
    assert!(
        mild_action.allows_entry(),
        "4% drawdown should NOT trip the circuit breaker"
    );
}

// ── Scenario 2: Zero-default account fields must not bypass risk checks ────────

#[test]
fn scenario2_zero_equity_from_bad_json_does_not_suppress_invariants() {
    // Simulate a partially deserialized Account where equity parsed as 0.0.
    // The bot must NOT skip risk checks when equity is zero.
    let limits = DailyRiskLimits {
        max_daily_drawdown_pct: Some(0.05),
        max_daily_trades: None,
    };
    // If start_equity > 0 but current_equity is 0, that is a 100% drawdown.
    let action = check_all(100_000.0, 0.0, 0, &limits);
    assert!(
        action.is_halt(),
        "zero equity must trip the circuit breaker, not allow new entries"
    );
}

// ── Scenario 3: Partial multi-leg fill leaves unhedged long ───────────────────

#[test]
fn scenario3_partial_fill_unhedged_long_is_invariant_violation() {
    // A credit put spread: sell 150P, buy 140P.
    // Suppose only the long 140P leg filled, creating a naked long position.
    let mut state = clean_state();
    let long_occ = occ_put("AAPL", "260917", 140.0);
    state.positions.push(long_put_pos("AAPL", &long_occ, 1.5));
    // No matching short — this is the imbalanced-fill scenario
    let violations = assert_invariants(&state);
    assert!(
        violations.iter().any(|v| v.invariant == Invariant::NoNakedLongPremium),
        "partial fill leaving only the long leg must be a NoNakedLongPremium violation; got {:?}",
        violations
    );
}

// ── Scenario 4: Duplicate order-ID creates duplicate long ─────────────────────

#[test]
fn scenario4_duplicate_order_creates_two_naked_longs_is_violation() {
    // Two fills with the same client_order_id would create two identical long
    // positions.  State has 2 unhedged longs from the same symbol.
    let mut state = clean_state();
    let long_occ = occ_put("MSFT", "260917", 380.0);
    state.positions.push(long_put_pos("MSFT", &long_occ, 4.0));
    state.positions.push(long_put_pos("MSFT", &long_occ, 4.0)); // duplicate
    let violations = assert_invariants(&state);
    assert!(
        violations.iter().any(|v| v.invariant == Invariant::NoNakedLongPremium),
        "duplicate long positions must trigger a violation"
    );
}

// ── Scenario 5: DTE=0 put with symbol that parses correctly ───────────────────

#[test]
fn scenario5_expired_occ_still_parseable() {
    // A short put on expiry day — OCC symbol still parses to a valid structure.
    // The bot must be able to identify and close it (not skip due to parse error).
    let occ = occ_put("GLD", "250620", 395.0);
    let parts = parse_occ(&occ).expect("OCC should parse even on DTE=0 expiry");
    assert_eq!(parts.root, "GLD");
    assert!(!parts.is_call);
    assert_eq!(parts.strike, 395.0);

    // Adding an expired short to state: the invariant should NOT fire just because
    // the position is expired — that's handled by the position monitor, not invariants.
    let mut state = clean_state();
    state.positions.push(short_put_pos("GLD", &occ, 395.0));
    let violations = assert_invariants(&state);
    assert!(
        violations.is_empty(),
        "expired short put alone should not trigger any invariant; got {:?}",
        violations
    );
}

// ── Scenario 6: Circuit breaker tripped but new long present ──────────────────

#[test]
fn scenario6_circuit_broken_and_long_present_is_double_violation() {
    // Circuit breaker should have prevented entry; if a long slipped through
    // anyway, invariants must catch both issues.
    let mut state = clean_state();
    state.circuit_broken = true;
    // Equity also in drawdown (which should have tripped the breaker)
    state.equity = 93_000.0; // 7% drawdown > 5% limit → drawdown violation
    // An unhedged long snuck in while the breaker was supposedly tripped
    let long_occ = occ_put("SPY", "260620", 480.0);
    state.positions.push(long_put_pos("SPY", &long_occ, 5.0));
    let violations = assert_invariants(&state);
    assert!(
        violations.iter().any(|v| v.invariant == Invariant::NoNakedLongPremium),
        "unhedged long should fire NoNakedLongPremium"
    );
    assert!(
        violations.iter().any(|v| v.invariant == Invariant::DailyDrawdownWithinLimit),
        "7% drawdown should fire DailyDrawdownWithinLimit"
    );
}

// ── Scenario 7: Circuit breaker stays tripped without manual reset ────────────

#[test]
fn scenario7_circuit_breaker_cannot_be_silently_cleared() {
    // Once tripped, circuit_broken=true must remain true until manual operator
    // intervention.  Setting it back to false without going through a proper reset
    // procedure is a violation.
    let mut state = clean_state();
    state.was_circuit_broken = true; // it was tripped in the previous tick
    state.circuit_broken = false;    // someone accidentally cleared it
    let violations = assert_invariants(&state);
    assert!(
        violations.iter().any(|v| v.invariant == Invariant::CircuitBreakerStaysTripped),
        "clearing the circuit breaker without a reset should be a violation"
    );
}

// ── Scenario 7b: Proper re-enable (was=false, now=false) is fine ─────────────

#[test]
fn scenario7b_circuit_never_tripped_then_not_tripped_is_fine() {
    let state = clean_state();
    // was_circuit_broken=false, circuit_broken=false → normal healthy state
    assert!(assert_invariants(&state).is_empty());
}

// ── Scenario 7c: Circuit breaker still tripped is fine ───────────────────────

#[test]
fn scenario7c_circuit_still_tripped_is_fine() {
    let mut state = clean_state();
    state.was_circuit_broken = true;
    state.circuit_broken = true; // still tripped — correct
    assert!(
        assert_invariants(&state).is_empty(),
        "circuit still tripped is a valid (safe) state"
    );
}

// ── Bonus: spread (short+long on same root+expiry) passes all invariants ──────

#[test]
fn credit_put_spread_passes_all_invariants() {
    let mut state = clean_state();
    // Use realistic strike: $300 × 1 contract × 100 = $30k max-loss on $100k equity = 30%
    // The portfolio ceiling is max_risk_capital_pct (4%) × 10 = 40%, so $30k < $40k → ok.
    let short_occ = occ_put("NVDA", "260620", 300.0);
    let long_occ  = occ_put("NVDA", "260620", 280.0);
    state.positions.push(short_put_pos("NVDA", &short_occ, 300.0));
    state.positions.push(long_put_pos("NVDA", &long_occ, 2.0));
    let violations = assert_invariants(&state);
    assert!(
        violations.is_empty(),
        "a properly hedged credit spread should have no invariant violations; got {:?}",
        violations
    );
}
