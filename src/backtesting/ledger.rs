// Exact-arithmetic accounting ledger using `rust_decimal`.
//
// The backtest engine uses `f64` for all pricing math (BSM, Heston, binomial)
// because transcendental functions require floating-point.  However, the
// *bookkeeping* side — tracking cash balances, commissions, and realized P&L
// across hundreds of trades — accumulates floating-point error that misleads
// performance reporting.
//
// This module maintains a parallel exact ledger.  The engine keeps its
// existing `current_capital: f64` for gate-checks and equity-curve math; it
// additionally calls `Ledger::debit` / `Ledger::credit` so the final reports
// can use the exact figures.

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

/// Exact-precision bookkeeper for a single strategy run.
#[derive(Debug, Clone)]
pub struct Ledger {
    starting_balance: Decimal,
    balance: Decimal,
    total_commissions: Decimal,
    total_realized_pnl: Decimal,
    total_debit_trades: u64,
    total_credit_trades: u64,
}

impl Ledger {
    /// Create a new ledger with the given starting balance (in dollars).
    pub fn new(starting_balance: f64) -> Self {
        let start = Decimal::from_f64(starting_balance)
            .unwrap_or(Decimal::ZERO);
        Self {
            starting_balance: start,
            balance: start,
            total_commissions: Decimal::ZERO,
            total_realized_pnl: Decimal::ZERO,
            total_debit_trades: 0,
            total_credit_trades: 0,
        }
    }

    /// Debit the ledger (cost to open a position or pay commissions).
    ///
    /// `cost` is the total dollar outflow (positive number).
    /// `commission` is the brokerage fee component of `cost`.
    pub fn debit(&mut self, cost: f64, commission: f64) {
        let d_cost = Decimal::from_f64(cost).unwrap_or(Decimal::ZERO);
        let d_comm = Decimal::from_f64(commission).unwrap_or(Decimal::ZERO);
        self.balance -= d_cost;
        self.total_commissions += d_comm;
        self.total_debit_trades += 1;
    }

    /// Credit the ledger (proceeds from closing a position).
    ///
    /// `proceeds` is the total dollar inflow (positive number).
    /// `commission` is the brokerage fee component already deducted from `proceeds`.
    /// `realized_pnl` is the net profit/loss on the closed position.
    pub fn credit(&mut self, proceeds: f64, commission: f64, realized_pnl: f64) {
        let d_proc = Decimal::from_f64(proceeds).unwrap_or(Decimal::ZERO);
        let d_comm = Decimal::from_f64(commission).unwrap_or(Decimal::ZERO);
        let d_pnl  = Decimal::from_f64(realized_pnl).unwrap_or(Decimal::ZERO);
        self.balance += d_proc;
        self.total_commissions += d_comm;
        self.total_realized_pnl += d_pnl;
        self.total_credit_trades += 1;
    }

    // ── Read-back accessors ──────────────────────────────────────────────────

    /// Current exact cash balance.
    pub fn balance(&self) -> Decimal { self.balance }

    /// Current exact cash balance as `f64` (for interop with pricing math).
    pub fn balance_f64(&self) -> f64 {
        self.balance.to_string().parse::<f64>().unwrap_or(0.0)
    }

    /// Total brokerage commissions paid, exact.
    pub fn total_commissions(&self) -> Decimal { self.total_commissions }

    /// Total realized P&L, exact.
    pub fn total_realized_pnl(&self) -> Decimal { self.total_realized_pnl }

    /// Net return on starting capital, as a fraction (e.g. 0.12 = 12%).
    pub fn net_return(&self) -> Option<Decimal> {
        if self.starting_balance.is_zero() {
            return None;
        }
        Some((self.balance - self.starting_balance) / self.starting_balance)
    }

    /// Number of opening trades debited.
    pub fn debit_count(&self) -> u64 { self.total_debit_trades }

    /// Number of closing trades credited.
    pub fn credit_count(&self) -> u64 { self.total_credit_trades }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commission_accumulation_is_exact() {
        let mut ledger = Ledger::new(100_000.0);
        // Credit $1.00 commission 100 times — f64 would accumulate rounding error.
        for _ in 0..100 {
            ledger.credit(0.0, 1.00, 0.0);
        }
        assert_eq!(
            ledger.total_commissions(),
            Decimal::from(100),
            "commission accumulation should be exact"
        );
    }

    #[test]
    fn test_pnl_accumulation_is_exact() {
        let mut ledger = Ledger::new(100_000.0);
        // Accumulate $0.01 P&L 1000 times.
        for _ in 0..1000 {
            ledger.credit(0.0, 0.0, 0.01);
        }
        assert_eq!(
            ledger.total_realized_pnl(),
            Decimal::new(10, 0), // Exactly $10
            "P&L accumulation should be exact"
        );
    }

    #[test]
    fn test_net_return() {
        let mut ledger = Ledger::new(100_000.0);
        // Earn $10,000
        ledger.credit(10_000.0, 0.0, 10_000.0);
        let ret = ledger.net_return().unwrap();
        // Should be exactly 0.10 (10%)
        let expected = Decimal::new(1, 1); // 0.1
        assert_eq!(ret, expected);
    }

    #[test]
    fn test_balance_tracks_debits_and_credits() {
        let mut ledger = Ledger::new(50_000.0);
        ledger.debit(5_000.0, 5.0);  // Buy position
        ledger.credit(6_000.0, 5.0, 995.0); // Close for profit
        // balance = 50000 - 5000 + 6000 = 51000
        assert_eq!(ledger.balance_f64(), 51_000.0);
    }
}
