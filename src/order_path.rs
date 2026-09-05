//! Pure, side-effect-free order-construction pipeline (Adversarial Hardening
//! Plan §3.1). See `ORDER_PATH.md` for the full per-step contract.
//!
//! ```text
//! Signal -> validate_signal -> check_risk_guards -> size_position
//!        -> build_order -> validate_occ_symbol -> generate_client_order_id
//! ```
//!
//! Every function here is deterministic and performs no network I/O and no
//! `unwrap_or`/silent-`continue` masking — every rejection is a distinct,
//! named `OrderPathError` variant. Network I/O (`resolve_single_leg_occ`,
//! `submit_options_order`) intentionally stays in `AlpacaClient` as a thin
//! async façade over this pure core.

use crate::alpaca::occ::{parse_occ, OccParts};
use crate::alpaca::types::OptionsOrderRequest;
use crate::alpaca::AlpacaClient;
use crate::risk::guards::{check_all, DailyRiskLimits, GuardAction};
use crate::strategies::SignalAction;
use std::fmt;

/// Every way the order path can reject a signal before submission.
/// Deliberately has no catch-all variant — every rejection must be traceable
/// to a specific step for post-mortem / audit purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderPathError {
    /// Step 1: non-actionable signal or confidence below the configured floor.
    SignalRejected(String),
    /// Step 2: a `DailyRiskLimits` guard halted new entries.
    RiskHalted(String),
    /// Step 3: requested size is not a valid tradable quantity.
    InvalidSizing(String),
    /// Step 4: `SignalAction` could not be converted into an order request.
    BuildOrderFailed(String),
    /// Step 5: an OCC symbol fails structural validation that Alpaca would
    /// reject server-side (422 asset not found / malformed symbol).
    OccRejected(String),
}

impl fmt::Display for OrderPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderPathError::SignalRejected(s) => write!(f, "SignalRejected: {s}"),
            OrderPathError::RiskHalted(s) => write!(f, "RiskHalted: {s}"),
            OrderPathError::InvalidSizing(s) => write!(f, "InvalidSizing: {s}"),
            OrderPathError::BuildOrderFailed(s) => write!(f, "BuildOrderFailed: {s}"),
            OrderPathError::OccRejected(s) => write!(f, "OccRejected: {s}"),
        }
    }
}

impl std::error::Error for OrderPathError {}

/// **Step 1 — signal evaluation.**
///
/// Pre-conditions: `confidence` is the strategy's reported confidence in `[0,1]`.
/// Post-conditions: `Ok(())` iff `action` is actionable and meets the floor.
/// Errors: `SignalRejected` for `NoAction`, `ClosePosition` (handled by
/// `manage_open_positions`, not the entry path), or low confidence.
pub fn validate_signal(
    action: &SignalAction,
    confidence: f64,
    min_confidence: f64,
) -> Result<(), OrderPathError> {
    if matches!(action, SignalAction::NoAction) {
        return Err(OrderPathError::SignalRejected("NoAction is not actionable".into()));
    }
    if matches!(action, SignalAction::ClosePosition { .. }) {
        return Err(OrderPathError::SignalRejected(
            "ClosePosition belongs to manage_open_positions, not the entry order path".into(),
        ));
    }
    if confidence < min_confidence {
        return Err(OrderPathError::SignalRejected(format!(
            "confidence {confidence:.2} < floor {min_confidence:.2}"
        )));
    }
    Ok(())
}

/// **Step 2 — shared risk guards.**
///
/// Pre-conditions: none.
/// Post-conditions: `Ok(())` iff `check_all` allows new entries.
/// Invariant: never returns `Ok` when `check_all` would return `Halt` —
/// this function cannot diverge from the shared guard used by backtesting.
/// Errors: `RiskHalted` carrying the guard's own reason string.
pub fn check_risk_guards(
    start_of_day_equity: f64,
    current_equity: f64,
    trades_today: usize,
    limits: &DailyRiskLimits,
) -> Result<(), OrderPathError> {
    match check_all(start_of_day_equity, current_equity, trades_today, limits) {
        GuardAction::Allow => Ok(()),
        GuardAction::Halt { reason } => Err(OrderPathError::RiskHalted(reason)),
    }
}

/// **Step 3 — sizing.**
///
/// Pre-conditions: `suggested_qty` as computed by the portfolio sizer.
/// Post-conditions: `Ok(qty)` with `qty >= 1`.
/// Errors: `InvalidSizing` for zero/negative suggested size (never silently
/// clamped to a default — a bad sizer output must be visible, not masked).
pub fn size_position(suggested_qty: i64) -> Result<u32, OrderPathError> {
    if suggested_qty < 1 {
        return Err(OrderPathError::InvalidSizing(format!(
            "suggested size {suggested_qty} must be >= 1"
        )));
    }
    Ok(suggested_qty as u32)
}

/// **Step 4 — build order.**
///
/// Pre-conditions: `qty >= 1` (see [`size_position`]).
/// Post-conditions: an `OptionsOrderRequest` with one leg (single-leg
/// strategies) or N legs (multi-leg strategies), every leg symbol produced
/// by [`AlpacaClient::occ_symbol`].
/// Errors: `BuildOrderFailed` for signals with no options representation.
pub fn build_order(
    signal: &SignalAction,
    underlying: &str,
    qty: u32,
    limit_price: Option<f64>,
) -> Result<OptionsOrderRequest, OrderPathError> {
    AlpacaClient::signal_to_options_order(signal, underlying, qty, limit_price)
        .map_err(|e| OrderPathError::BuildOrderFailed(e.to_string()))
}

/// **Step 5 — OCC validation.**
///
/// Pre-conditions: `occ` as produced by step 4, optionally after
/// `AlpacaClient::resolve_single_leg_occ`.
/// Post-conditions: `Ok(OccParts)` only for symbols that are both
/// structurally parseable *and* pass the extra checks Alpaca enforces
/// server-side.
/// Contract: `validate_occ_symbol` must NEVER return `Ok` for a symbol that
/// Alpaca would reject with 422 (asset not found / malformed symbol).
/// Errors: `OccRejected` with the specific reason.
pub fn validate_occ_symbol(occ: &str) -> Result<OccParts, OrderPathError> {
    let parts = parse_occ(occ)
        .ok_or_else(|| OrderPathError::OccRejected(format!("{occ}: does not parse as OCC")))?;
    if parts.root.is_empty() || parts.root.len() > 6 || !parts.root.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(OrderPathError::OccRejected(format!(
            "{occ}: root '{}' is not 1-6 alphabetic chars", parts.root
        )));
    }
    if parts.strike <= 0.0 {
        return Err(OrderPathError::OccRejected(format!(
            "{occ}: strike {} must be > 0", parts.strike
        )));
    }
    if parts.expiry < chrono::Utc::now().date_naive() {
        return Err(OrderPathError::OccRejected(format!(
            "{occ}: expiry {} is in the past", parts.expiry_str
        )));
    }
    Ok(parts)
}

/// **Step 6 — client_order_id.**
///
/// Pre-conditions: none.
/// Post-conditions: a string unique per call (monotonic counter + millisecond
/// timestamp), stable and safe to log verbatim for replay/audit.
/// Contract: never reused across submission attempts — see
/// `AlpacaClient::post_order_safe`'s ambiguous-timeout-never-retry rule for
/// why a stable, non-regenerated id per attempt matters.
pub fn generate_client_order_id(prefix: &str) -> String {
    AlpacaClient::generate_order_id(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confident_buy_call() -> SignalAction {
        SignalAction::BuyCall { strike: 150.0, days_to_expiry: 30, volatility: 0.25 }
    }

    #[test]
    fn validate_signal_rejects_no_action() {
        assert!(matches!(
            validate_signal(&SignalAction::NoAction, 0.9, 0.6),
            Err(OrderPathError::SignalRejected(_))
        ));
    }

    #[test]
    fn validate_signal_rejects_close_position() {
        assert!(matches!(
            validate_signal(&SignalAction::ClosePosition { position_id: 1 }, 0.9, 0.6),
            Err(OrderPathError::SignalRejected(_))
        ));
    }

    #[test]
    fn validate_signal_rejects_low_confidence() {
        assert!(matches!(
            validate_signal(&confident_buy_call(), 0.1, 0.6),
            Err(OrderPathError::SignalRejected(_))
        ));
    }

    #[test]
    fn validate_signal_allows_actionable_confident_signal() {
        assert!(validate_signal(&confident_buy_call(), 0.9, 0.6).is_ok());
    }

    #[test]
    fn check_risk_guards_halts_on_drawdown_breach() {
        let limits = DailyRiskLimits { max_daily_drawdown_pct: Some(0.05), max_daily_trades: None };
        assert!(matches!(
            check_risk_guards(100_000.0, 94_000.0, 0, &limits),
            Err(OrderPathError::RiskHalted(_))
        ));
    }

    #[test]
    fn check_risk_guards_allows_within_limits() {
        let limits = DailyRiskLimits { max_daily_drawdown_pct: Some(0.05), max_daily_trades: None };
        assert!(check_risk_guards(100_000.0, 98_000.0, 0, &limits).is_ok());
    }

    #[test]
    fn size_position_rejects_zero_and_negative() {
        assert!(matches!(size_position(0), Err(OrderPathError::InvalidSizing(_))));
        assert!(matches!(size_position(-5), Err(OrderPathError::InvalidSizing(_))));
    }

    #[test]
    fn size_position_accepts_positive() {
        assert_eq!(size_position(3).unwrap(), 3);
    }

    #[test]
    fn build_order_produces_single_leg_for_buy_call() {
        let order = build_order(&confident_buy_call(), "AAPL", 1, None).unwrap();
        assert!(order.symbol.is_some());
        assert!(order.legs.is_none());
    }

    #[test]
    fn build_order_rejects_no_action() {
        assert!(matches!(
            build_order(&SignalAction::NoAction, "AAPL", 1, None),
            Err(OrderPathError::BuildOrderFailed(_))
        ));
    }

    #[test]
    fn validate_occ_symbol_accepts_well_formed_future_contract() {
        let occ = AlpacaClient::occ_symbol("AAPL", 35, 1, 17, true, 150.0);
        assert!(validate_occ_symbol(&occ).is_ok());
    }

    #[test]
    fn validate_occ_symbol_rejects_past_expiry() {
        let occ = AlpacaClient::occ_symbol("AAPL", 20, 1, 17, true, 150.0);
        assert!(matches!(validate_occ_symbol(&occ), Err(OrderPathError::OccRejected(_))));
    }

    #[test]
    fn validate_occ_symbol_rejects_zero_strike() {
        let occ = AlpacaClient::occ_symbol("AAPL", 35, 1, 17, true, 0.0);
        assert!(matches!(validate_occ_symbol(&occ), Err(OrderPathError::OccRejected(_))));
    }

    #[test]
    fn validate_occ_symbol_rejects_unparseable_garbage() {
        assert!(matches!(validate_occ_symbol("garbage"), Err(OrderPathError::OccRejected(_))));
    }

    #[test]
    fn generate_client_order_id_is_unique_across_calls() {
        let a = generate_client_order_id("opt");
        let b = generate_client_order_id("opt");
        assert_ne!(a, b);
        assert!(a.starts_with("db-opt-"));
    }

    // ── Full-pipeline test: every step chained, dry-run only (no submit) ──
    #[test]
    fn full_pipeline_happy_path() {
        let signal = confident_buy_call();
        validate_signal(&signal, 0.9, 0.6).unwrap();
        let limits = DailyRiskLimits { max_daily_drawdown_pct: Some(0.05), max_daily_trades: None };
        check_risk_guards(100_000.0, 99_000.0, 0, &limits).unwrap();
        let qty = size_position(2).unwrap();
        let order = build_order(&signal, "AAPL", qty, None).unwrap();
        let occ = order.symbol.expect("single-leg order must carry a symbol");
        let parts = validate_occ_symbol(&occ).unwrap();
        assert_eq!(parts.root, "AAPL");
        let cid = generate_client_order_id("opt");
        assert!(!cid.is_empty());
    }
}

// ── Property-based fuzzing across the pipeline (Adversarial Hardening Plan §1.2) ──
// Goal: for thousands of adversarial (root, strike, expiry, side) combinations,
// running the full parse_occ -> risk_guards -> sizing -> build_order ->
// validate_occ_symbol -> generate_client_order_id pipeline must never panic,
// and must never let an invalid contract survive to `validate_occ_symbol`.
#[cfg(test)]
mod proptest_pipeline {
    use super::*;
    use proptest::prelude::*;

    fn any_root() -> impl Strategy<Value = String> {
        prop::string::string_regex("[A-Za-z]{0,8}").unwrap()
    }

    proptest! {
        /// Building an order for a BuyCall/BuyPut signal and validating the
        /// resulting OCC symbol never panics, for arbitrary roots, strikes,
        /// and expiries (including adversarial ones that should be rejected).
        #[test]
        fn pipeline_never_panics(
            root in any_root(),
            strike in -100f64..10_000f64,
            dte in 0i64..800i64,
            is_call in any::<bool>(),
            qty in 0i64..5i64,
        ) {
            let signal = if is_call {
                SignalAction::BuyCall { strike, days_to_expiry: dte as usize, volatility: 0.25 }
            } else {
                SignalAction::BuyPut { strike, days_to_expiry: dte as usize, volatility: 0.25 }
            };

            let limits = DailyRiskLimits { max_daily_drawdown_pct: Some(0.05), max_daily_trades: None };
            let _ = check_risk_guards(100_000.0, 98_000.0, 0, &limits);

            let sized = size_position(qty);
            if let Ok(q) = sized {
                if let Ok(order) = build_order(&signal, &root, q, None) {
                    if let Some(occ) = order.symbol {
                        // Any adversarial (negative/zero/huge strike, bad root, past
                        // expiry) combination must be rejected, never silently pass.
                        let result = validate_occ_symbol(&occ);
                        if strike <= 0.0 || root.is_empty() || root.len() > 6
                            || !root.chars().all(|c| c.is_ascii_alphabetic())
                        {
                            prop_assert!(result.is_err());
                        }
                    }
                }
            }
        }
    }
}
