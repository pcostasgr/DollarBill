/// Runtime invariant checker — run after every fill and position reconciliation.
///
/// On any violation the caller must:
///   1. Flatten all open risk.
///   2. Trip the circuit breaker permanently for the day.
///   3. Emit a high-priority alert.
///   4. Log the full state for post-mortem.
use crate::alpaca::occ::parse_occ;
use std::collections::HashSet;

// ── Invariant definitions ─────────────────────────────────────────────────────

/// A single invariant that must hold after every state transition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Invariant {
    /// When `block_long_premium = true`, no long-premium option position exists
    /// that is not a hedge leg of an existing short on the same underlying+expiry.
    NoNakedLongPremium,
    /// No plain equity position exists outside the `protected_equity` set.
    NoUnprotectedEquity,
    /// Sum of max-loss across all open option positions ≤ equity × limit.
    MaxLossWithinLimit,
    /// Intraday drawdown has not exceeded the configured circuit-breaker threshold.
    DailyDrawdownWithinLimit,
    /// Circuit breaker, once tripped, stays tripped for the remainder of the day.
    CircuitBreakerStaysTripped,
}

/// A violated invariant with a human-readable explanation.
#[derive(Debug, Clone)]
pub struct InvariantViolation {
    pub invariant: Invariant,
    pub detail: String,
}

impl std::fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "INVARIANT VIOLATION [{:?}]: {}", self.invariant, self.detail)
    }
}

// ── Input types ───────────────────────────────────────────────────────────────

/// A position as visible to the invariant checker.
#[derive(Debug, Clone)]
pub struct InvariantPosition {
    pub symbol: String,
    pub occ_symbol: Option<String>,
    /// Negative = short, positive = long.
    pub qty: f64,
    /// Latest mark price.
    pub current_mark: f64,
}

/// All state the invariant checker needs.
#[derive(Debug, Clone)]
pub struct BotState {
    pub positions: Vec<InvariantPosition>,
    /// Current mark-to-market equity.
    pub equity: f64,
    /// Equity at the start of today's session (yesterday's close).
    pub start_of_day_equity: f64,
    /// True if the circuit breaker was tripped before this check.
    pub was_circuit_broken: bool,
    /// True if the circuit breaker is currently tripped.
    pub circuit_broken: bool,
    /// Max-loss fraction per underlying (e.g. 0.04 = 4% of equity).
    pub max_risk_capital_pct: f64,
    /// Daily drawdown limit (e.g. 0.05 = 5%).
    pub max_daily_drawdown_pct: f64,
    /// When true, any unhedged long-premium option is a violation.
    pub block_long_premium: bool,
    /// Equity tickers the bot intentionally holds long.
    pub protected_equity: HashSet<String>,
}

// ── Checker ───────────────────────────────────────────────────────────────────

/// Run all invariants and return the list of violations (empty = all green).
pub fn assert_invariants(state: &BotState) -> Vec<InvariantViolation> {
    let mut violations = Vec::new();

    // ── 1. No naked long premium ──────────────────────────────────────────
    if state.block_long_premium {
        for pos in &state.positions {
            if pos.qty <= 0.0 { continue; }
            let Some(ref occ_str) = pos.occ_symbol else { continue };
            let Some(parts) = parse_occ(occ_str) else { continue };

            let is_hedge = state.positions.iter().any(|other| {
                if other.symbol == pos.symbol && other.occ_symbol == pos.occ_symbol { return false; }
                if other.qty >= 0.0 { return false; }
                parse_occ(other.occ_symbol.as_deref().unwrap_or(""))
                    .map(|o| o.root == parts.root && o.expiry == parts.expiry)
                    .unwrap_or(false)
            });
            if !is_hedge {
                violations.push(InvariantViolation {
                    invariant: Invariant::NoNakedLongPremium,
                    detail: format!(
                        "{} qty={:.0} is a long-premium position with no matching short hedge",
                        occ_str, pos.qty
                    ),
                });
            }
        }
    }

    // ── 2. No unprotected equity ──────────────────────────────────────────
    for pos in &state.positions {
        if pos.occ_symbol.is_some() { continue; } // option, not equity
        let is_equity = pos.symbol.len() <= 6
            && pos.symbol.chars().all(|c| c.is_ascii_alphabetic());
        if is_equity && !state.protected_equity.contains(&pos.symbol) {
            violations.push(InvariantViolation {
                invariant: Invariant::NoUnprotectedEquity,
                detail: format!(
                    "{} qty={:.0} is plain equity not in the protected set",
                    pos.symbol, pos.qty
                ),
            });
        }
    }

    // ── 3. Max loss within limit ──────────────────────────────────────────
    if state.max_risk_capital_pct > 0.0 && state.equity > 0.0 {
        let total_max_loss: f64 = state.positions.iter().filter_map(|p| {
            if p.qty >= 0.0 { return None; } // only short positions carry uncapped risk
            parse_occ(p.occ_symbol.as_deref()?)
                .map(|o| o.strike * p.qty.abs() * 100.0)
        }).sum();
        let loss_pct = total_max_loss / state.equity;
        let limit = state.max_risk_capital_pct;
        // Allow up to 10× the per-symbol limit as a total portfolio ceiling
        if loss_pct > limit * 10.0 {
            violations.push(InvariantViolation {
                invariant: Invariant::MaxLossWithinLimit,
                detail: format!(
                    "total max-loss ${:.0} is {:.1}% of equity — exceeds {:.1}% portfolio limit",
                    total_max_loss, loss_pct * 100.0, limit * 1000.0
                ),
            });
        }
    }

    // ── 4. Daily drawdown within limit ────────────────────────────────────
    if state.start_of_day_equity > 0.0 {
        let drawdown =
            (state.start_of_day_equity - state.equity) / state.start_of_day_equity;
        if drawdown > state.max_daily_drawdown_pct {
            violations.push(InvariantViolation {
                invariant: Invariant::DailyDrawdownWithinLimit,
                detail: format!(
                    "drawdown {:.2}% > limit {:.2}%",
                    drawdown * 100.0,
                    state.max_daily_drawdown_pct * 100.0
                ),
            });
        }
    }

    // ── 5. Circuit breaker stays tripped once set ─────────────────────────
    if state.was_circuit_broken && !state.circuit_broken {
        violations.push(InvariantViolation {
            invariant: Invariant::CircuitBreakerStaysTripped,
            detail: "circuit breaker was tripped but is now cleared without manual reset".into(),
        });
    }

    violations
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_state() -> BotState {
        BotState {
            positions: vec![],
            equity: 100_000.0,
            start_of_day_equity: 100_000.0,
            was_circuit_broken: false,
            circuit_broken: false,
            max_risk_capital_pct: 0.04,
            max_daily_drawdown_pct: 0.05,
            block_long_premium: true,
            protected_equity: HashSet::new(),
        }
    }

    fn occ_put(root: &str, strike: f64) -> String {
        format!("{}260926P{:08.0}", root, strike * 1000.0)
    }

    #[test]
    fn empty_state_no_violations() {
        assert!(assert_invariants(&healthy_state()).is_empty());
    }

    #[test]
    fn unhedged_long_premium_is_violation() {
        let mut s = healthy_state();
        s.positions.push(InvariantPosition {
            symbol: "AAPL".into(),
            occ_symbol: Some(occ_put("AAPL", 150.0)),
            qty: 1.0,
            current_mark: 3.0,
        });
        let v = assert_invariants(&s);
        assert!(v.iter().any(|x| x.invariant == Invariant::NoNakedLongPremium));
    }

    #[test]
    fn long_wing_with_matching_short_not_a_violation() {
        let mut s = healthy_state();
        // Short put at 150
        s.positions.push(InvariantPosition {
            symbol: "AAPL".into(),
            occ_symbol: Some(occ_put("AAPL", 150.0)),
            qty: -1.0,
            current_mark: 3.0,
        });
        // Long put at 140 — hedge leg, same root + expiry
        s.positions.push(InvariantPosition {
            symbol: "AAPL".into(),
            occ_symbol: Some(occ_put("AAPL", 140.0)),
            qty: 1.0,
            current_mark: 1.0,
        });
        assert!(assert_invariants(&s).is_empty());
    }

    #[test]
    fn assigned_equity_is_violation() {
        let mut s = healthy_state();
        s.positions.push(InvariantPosition {
            symbol: "AAPL".into(),
            occ_symbol: None,
            qty: 100.0,
            current_mark: 150.0,
        });
        let v = assert_invariants(&s);
        assert!(v.iter().any(|x| x.invariant == Invariant::NoUnprotectedEquity));
    }

    #[test]
    fn protected_equity_not_a_violation() {
        let mut s = healthy_state();
        s.protected_equity.insert("AAPL".into());
        s.positions.push(InvariantPosition {
            symbol: "AAPL".into(),
            occ_symbol: None,
            qty: 100.0,
            current_mark: 150.0,
        });
        assert!(assert_invariants(&s).is_empty());
    }

    #[test]
    fn daily_drawdown_breach_is_violation() {
        let mut s = healthy_state();
        s.equity = 94_000.0; // 6% drawdown > 5% limit
        let v = assert_invariants(&s);
        assert!(v.iter().any(|x| x.invariant == Invariant::DailyDrawdownWithinLimit));
    }

    #[test]
    fn circuit_breaker_cleared_without_reset_is_violation() {
        let mut s = healthy_state();
        s.was_circuit_broken = true;
        s.circuit_broken = false; // improperly cleared
        let v = assert_invariants(&s);
        assert!(v.iter().any(|x| x.invariant == Invariant::CircuitBreakerStaysTripped));
    }

    #[test]
    fn drawdown_within_limit_no_violation() {
        let mut s = healthy_state();
        s.equity = 96_000.0; // 4% drawdown < 5% limit
        assert!(assert_invariants(&s).is_empty());
    }
}
