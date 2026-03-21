// Reg T margin requirements for short option positions.
//
// Rules implemented (CBOE / Reg T):
//
//   Naked Call:
//     max(20% × spot − OTM_amount + premium,  10% × spot + premium)
//     where OTM_amount = max(0, strike − spot)
//
//   Naked Put:
//     max(20% × spot − OTM_amount + premium,  10% × strike + premium)
//     where OTM_amount = max(0, spot − strike)
//
//   Vertical spread (credit call/put spread):
//     (buy_strike − sell_strike).abs() × 100  per contract  (max_loss)
//
//   Iron condor (two vertical spreads):
//     max(call_spread_width, put_spread_width) × 100  per condor
//
//   Cash-secured put:
//     strike × 100  per contract  (full cash secures assignment)
//
// All amounts are in dollars **per contract** (1 contract = 100 shares).
// Multiply by number of contracts to get total requirement.

/// Margin requirement details for a single short option position or spread.
#[derive(Debug, Clone, PartialEq)]
pub struct MarginRequirement {
    /// Total dollar margin required per contract.
    pub per_contract: f64,
    /// The rule or formula that produced this figure.
    pub rule: MarginRule,
}

/// Which Reg T formula applied.
#[derive(Debug, Clone, PartialEq)]
pub enum MarginRule {
    /// Naked short call — 20%/10% CBOE formula.
    NakedCall,
    /// Naked short put — 20%/10% CBOE formula.
    NakedPut,
    /// Vertical credit spread — max loss is the spread width.
    CreditSpread,
    /// Iron condor — max loss of the wider wing only.
    IronCondor,
    /// Cash-secured put — full assignment cost held in cash.
    CashSecuredPut,
}

/// Compute Reg T margin for a **naked short call** per contract.
///
/// # Arguments
/// * `spot`    — current underlying price
/// * `strike`  — option strike price
/// * `premium` — current option mid-price (per share, so ×100 for contract value)
pub fn naked_call_margin(spot: f64, strike: f64, premium: f64) -> MarginRequirement {
    debug_assert!(spot > 0.0, "spot must be positive");
    debug_assert!(premium >= 0.0, "premium must be non-negative");

    let otm_amount = (strike - spot).max(0.0);
    let premium_dollars = premium * 100.0;

    let twenty_pct_rule = 0.20 * spot * 100.0 - otm_amount * 100.0 + premium_dollars;
    let ten_pct_rule    = 0.10 * spot * 100.0 + premium_dollars;

    MarginRequirement {
        per_contract: twenty_pct_rule.max(ten_pct_rule).max(0.0),
        rule: MarginRule::NakedCall,
    }
}

/// Compute Reg T margin for a **naked short put** per contract.
///
/// # Arguments
/// * `spot`    — current underlying price
/// * `strike`  — option strike price (put strike)
/// * `premium` — current option mid-price (per share)
pub fn naked_put_margin(spot: f64, strike: f64, premium: f64) -> MarginRequirement {
    debug_assert!(spot > 0.0, "spot must be positive");
    debug_assert!(premium >= 0.0, "premium must be non-negative");

    let otm_amount = (spot - strike).max(0.0);
    let premium_dollars = premium * 100.0;

    let twenty_pct_rule = 0.20 * spot * 100.0 - otm_amount * 100.0 + premium_dollars;
    let ten_pct_rule    = 0.10 * strike * 100.0 + premium_dollars;

    MarginRequirement {
        per_contract: twenty_pct_rule.max(ten_pct_rule).max(0.0),
        rule: MarginRule::NakedPut,
    }
}

/// Compute Reg T margin for a **vertical credit spread** (call or put) per contract.
///
/// Max loss = spread width (the most you can lose, net of premium collected).
/// `sell_strike` and `buy_strike` should be the naked leg and the protective leg;
/// the width is taken as an absolute value so caller order doesn't matter.
pub fn credit_spread_margin(sell_strike: f64, buy_strike: f64) -> MarginRequirement {
    let width = (buy_strike - sell_strike).abs();
    MarginRequirement {
        per_contract: width * 100.0,
        rule: MarginRule::CreditSpread,
    }
}

/// Compute Reg T margin for an **iron condor** per condor (one set of 4 legs).
///
/// Reg T requires margin only on the wider wing (the other wing is "free" because
/// both wings cannot both expire at max loss simultaneously).
pub fn iron_condor_margin(
    sell_call_strike: f64,
    buy_call_strike: f64,
    sell_put_strike: f64,
    buy_put_strike: f64,
) -> MarginRequirement {
    let call_width = (buy_call_strike - sell_call_strike).abs();
    let put_width  = (sell_put_strike - buy_put_strike).abs();
    MarginRequirement {
        per_contract: call_width.max(put_width) * 100.0,
        rule: MarginRule::IronCondor,
    }
}

/// Compute margin for a **cash-secured put** per contract.
///
/// The broker holds `strike × 100` in cash to cover assignment.  No additional
/// margin formula applies — this is purely the cash set-aside.
pub fn cash_secured_put_margin(strike: f64) -> MarginRequirement {
    MarginRequirement {
        per_contract: strike * 100.0,
        rule: MarginRule::CashSecuredPut,
    }
}

/// Check whether opening a short position is feasible given available capital.
///
/// Returns `true` if `available_capital` covers the total margin for `contracts`
/// lots at the computed per-contract rate (plus a 10% buffer for maintenance
/// margin fluctuations).
pub fn has_sufficient_margin(margin: &MarginRequirement, contracts: i32, available_capital: f64) -> bool {
    const SAFETY_BUFFER: f64 = 1.10; // 10% above initial margin
    let required = margin.per_contract * contracts.abs() as f64 * SAFETY_BUFFER;
    available_capital >= required
}

// ─── Max-loss / max-profit helpers ────────────────────────────────────────────

/// Maximum possible **loss** for a **credit call or put spread**, per contract.
///
/// `max_loss = (spread_width − net_premium_received) × 100`
///
/// This is also the Reg T margin requirement for credit spreads.
///
/// # Arguments
/// * `sell_strike`         — strike of the short leg
/// * `buy_strike`          — strike of the long (protective) leg
/// * `net_premium_per_share` — net credit received per share (positive number)
pub fn max_loss_credit_spread(sell_strike: f64, buy_strike: f64, net_premium_per_share: f64) -> f64 {
    let width = (buy_strike - sell_strike).abs();
    (width - net_premium_per_share).max(0.0) * 100.0
}

/// Maximum possible **loss** for an **iron condor**, per condor.
///
/// Only one wing can expire at max loss, so max loss is:
/// `max(call_spread_width, put_spread_width) × 100 − net_premium × 100`
///
/// # Arguments
/// * `sell_call_strike` / `buy_call_strike` — strikes of the call wing
/// * `sell_put_strike`  / `buy_put_strike`  — strikes of the put wing
/// * `net_premium_per_share` — total net credit collected per share
pub fn max_loss_iron_condor(
    sell_call_strike: f64,
    buy_call_strike: f64,
    sell_put_strike: f64,
    buy_put_strike: f64,
    net_premium_per_share: f64,
) -> f64 {
    let call_width = (buy_call_strike - sell_call_strike).abs();
    let put_width  = (sell_put_strike - buy_put_strike).abs();
    let max_width  = call_width.max(put_width);
    (max_width - net_premium_per_share).max(0.0) * 100.0
}

/// Maximum **profit** on any short option position = premium received.
///
/// # Arguments
/// * `premium_per_share` — per-share premium received at entry
/// * `contracts`         — number of contracts (1 contract = 100 shares)
pub fn max_profit_short(premium_per_share: f64, contracts: u32) -> f64 {
    premium_per_share * 100.0 * contracts as f64
}

/// Maximum possible **loss** for a **naked short put**, per contract.
///
/// Occurs if the underlying goes to $0: `(strike − premium) × 100`.
/// A naked short call has theoretically unlimited loss.
///
/// # Arguments
/// * `strike`            — put strike price
/// * `premium_per_share` — premium received per share at entry
pub fn max_loss_naked_put(strike: f64, premium_per_share: f64) -> f64 {
    (strike - premium_per_share).max(0.0) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Naked Call ────────────────────────────────────────────────────────────

    #[test]
    fn naked_call_atm_uses_twenty_pct_formula() {
        // ATM call on $100 stock with $3 premium
        // 20% rule: 0.20×100×100 - 0 + 300 = 2300
        // 10% rule: 0.10×100×100 + 300 = 1300
        // => 2300
        let m = naked_call_margin(100.0, 100.0, 3.0);
        assert!((m.per_contract - 2300.0).abs() < 0.01, "expected 2300, got {}", m.per_contract);
        assert_eq!(m.rule, MarginRule::NakedCall);
    }

    #[test]
    fn naked_call_deep_otm_may_hit_ten_pct_floor() {
        // $100 stock, $150 call (deep OTM), $0.10 premium
        // 20% rule: 2000 - 5000 + 10 = negative → floored at 0 by 10% rule
        // 10% rule: 1000 + 10 = 1010
        // => 1010
        let m = naked_call_margin(100.0, 150.0, 0.10);
        assert!((m.per_contract - 1010.0).abs() < 0.01, "expected 1010, got {}", m.per_contract);
    }

    #[test]
    fn naked_call_margin_never_negative() {
        // astronomically deep OTM should still produce ≥ 0
        let m = naked_call_margin(50.0, 999.0, 0.01);
        assert!(m.per_contract >= 0.0);
    }

    // ── Naked Put ─────────────────────────────────────────────────────────────

    #[test]
    fn naked_put_atm_uses_twenty_pct_formula() {
        // ATM put on $100 stock, $3 premium
        // 20% rule: 2000 - 0 + 300 = 2300
        // 10% rule: 0.10×100×100 + 300 = 1300
        // => 2300
        let m = naked_put_margin(100.0, 100.0, 3.0);
        assert!((m.per_contract - 2300.0).abs() < 0.01, "expected 2300, got {}", m.per_contract);
        assert_eq!(m.rule, MarginRule::NakedPut);
    }

    #[test]
    fn naked_put_itm_strike_higher_margin() {
        // $100 stock, $110 strike ITM put — OTM amount = max(0, 100-110)= 0
        // 20% rule: 2000 + premium
        // Should exceed ATM margin when ITM
        let itm = naked_put_margin(100.0, 110.0, 12.0);
        let atm = naked_put_margin(100.0, 100.0, 3.0);
        assert!(itm.per_contract > atm.per_contract,
            "ITM put {:.0} should require more margin than ATM put {:.0}",
            itm.per_contract, atm.per_contract);
    }

    #[test]
    fn naked_put_otm_reduces_twenty_pct() {
        // $100 stock, $80 put (OTM by $20), $1 premium
        // 20% rule: 2000 - 2000 + 100 = 100
        // 10% rule: 0.10×80×100 + 100 = 900
        // => 900 (10% floor applies)
        let m = naked_put_margin(100.0, 80.0, 1.0);
        assert!((m.per_contract - 900.0).abs() < 0.01, "expected 900, got {}", m.per_contract);
    }

    // ── Credit Spread ─────────────────────────────────────────────────────────

    #[test]
    fn credit_spread_margin_is_width_times_100() {
        // 5-point spread → $500 per contract
        let m = credit_spread_margin(100.0, 105.0);
        assert!((m.per_contract - 500.0).abs() < 0.01);
        assert_eq!(m.rule, MarginRule::CreditSpread);
    }

    #[test]
    fn credit_spread_margin_order_independent() {
        let m1 = credit_spread_margin(105.0, 100.0);
        let m2 = credit_spread_margin(100.0, 105.0);
        assert!((m1.per_contract - m2.per_contract).abs() < 0.01);
    }

    // ── Iron Condor ───────────────────────────────────────────────────────────

    #[test]
    fn iron_condor_margin_uses_wider_wing() {
        // Call wing: 5 points, put wing: 3 points → margin = 5×100 = 500
        let m = iron_condor_margin(110.0, 115.0, 90.0, 87.0);
        assert!((m.per_contract - 500.0).abs() < 0.01);
        assert_eq!(m.rule, MarginRule::IronCondor);
    }

    #[test]
    fn iron_condor_equal_wings() {
        let m = iron_condor_margin(110.0, 115.0, 90.0, 85.0);
        assert!((m.per_contract - 500.0).abs() < 0.01);
    }

    // ── Cash-Secured Put ──────────────────────────────────────────────────────

    #[test]
    fn cash_secured_put_margin_is_strike_times_100() {
        let m = cash_secured_put_margin(95.0);
        assert!((m.per_contract - 9500.0).abs() < 0.01);
        assert_eq!(m.rule, MarginRule::CashSecuredPut);
    }

    // ── has_sufficient_margin ─────────────────────────────────────────────────

    #[test]
    fn sufficient_margin_passes_when_capital_covers_requirement() {
        let m = naked_call_margin(100.0, 100.0, 3.0); // $2300 per contract
        // 5 contracts × $2300 × 1.10 buffer = $12650
        assert!(has_sufficient_margin(&m, 5, 15_000.0));
    }

    #[test]
    fn sufficient_margin_fails_when_capital_insufficient() {
        let m = naked_call_margin(100.0, 100.0, 3.0); // $2300 per contract
        assert!(!has_sufficient_margin(&m, 5, 5_000.0));
    }

    #[test]
    fn sufficient_margin_respects_safety_buffer() {
        let m = naked_call_margin(100.0, 100.0, 3.0); // $2300 per contract
        // Exactly 1×$2300 without safety buffer would pass, but with 10% buffer it fails
        assert!(!has_sufficient_margin(&m, 1, 2_301.0)); // < 2300 × 1.10 = 2530
        assert!(has_sufficient_margin(&m, 1, 2_530.0));
    }

    // ── Max-loss / max-profit helpers ─────────────────────────────────────────

    #[test]
    fn max_loss_credit_spread_five_point_two_credit() {
        // 5-point spread, $2 premium → max loss = (5-2)×100 = $300
        let loss = max_loss_credit_spread(100.0, 105.0, 2.0);
        assert!((loss - 300.0).abs() < 0.01, "expected 300, got {loss}");
    }

    #[test]
    fn max_loss_credit_spread_premium_equals_width_is_zero() {
        // If premium == spread width, max loss = 0 (perfect credit collected)
        let loss = max_loss_credit_spread(100.0, 105.0, 5.0);
        assert!(loss.abs() < 0.01, "expected 0, got {loss}");
    }

    #[test]
    fn max_loss_credit_spread_never_negative() {
        // Premium > width is not practical but result must not go negative
        let loss = max_loss_credit_spread(100.0, 105.0, 10.0);
        assert!(loss >= 0.0);
    }

    #[test]
    fn max_loss_iron_condor_uses_wider_wing() {
        // Call wing 5pts, put wing 3pts, $3 net premium
        // max_loss = (5 - 3) × 100 = $200
        let loss = max_loss_iron_condor(110.0, 115.0, 90.0, 87.0, 3.0);
        assert!((loss - 200.0).abs() < 0.01, "expected 200, got {loss}");
    }

    #[test]
    fn max_profit_short_is_premium_times_contracts() {
        // Sold 3 contracts at $5.00 → max profit = $1500
        let profit = max_profit_short(5.0, 3);
        assert!((profit - 1500.0).abs() < 0.01, "expected 1500, got {profit}");
    }

    #[test]
    fn max_loss_naked_put_underlying_goes_to_zero() {
        // $100 strike, $5 premium → max loss = (100-5)×100 = $9500
        let loss = max_loss_naked_put(100.0, 5.0);
        assert!((loss - 9500.0).abs() < 0.01, "expected 9500, got {loss}");
    }
}
