#![allow(dead_code)]
// Position tracking for open options positions

use crate::models::american::ExerciseStyle;
use crate::models::bs_mod::Greeks;

#[derive(Debug, Clone)]
pub enum PositionStatus {
    Open,
    Closed,
    Expired,
}

#[derive(Debug, Clone)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub id: usize,
    pub symbol: String,
    pub option_type: OptionType,
    pub exercise_style: ExerciseStyle,
    pub strike: f64,
    pub quantity: i32,  // Positive for long, negative for short
    pub entry_price: f64,
    pub entry_date: String,
    pub entry_spot: f64,
    pub exit_price: Option<f64>,
    pub exit_date: Option<String>,
    pub exit_spot: Option<f64>,
    pub status: PositionStatus,
    pub days_held: usize,
    
    // Greeks at entry
    pub entry_greeks: Option<Greeks>,
    
    // P&L tracking
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
}

impl Position {
    pub fn new(
        id: usize,
        symbol: String,
        option_type: OptionType,
        exercise_style: ExerciseStyle,
        strike: f64,
        quantity: i32,
        entry_price: f64,
        entry_date: String,
        entry_spot: f64,
        entry_greeks: Option<Greeks>,
    ) -> Self {
        Self {
            id,
            symbol,
            option_type,
            exercise_style,
            strike,
            quantity,
            entry_price,
            entry_date,
            entry_spot,
            exit_price: None,
            exit_date: None,
            exit_spot: None,
            status: PositionStatus::Open,
            days_held: 0,
            entry_greeks,
            realized_pnl: 0.0,
            unrealized_pnl: 0.0,
        }
    }
    
    /// Close position at market price
    pub fn close(&mut self, exit_price: f64, exit_date: String, exit_spot: f64, days_held: usize) {
        self.exit_price = Some(exit_price);
        self.exit_date = Some(exit_date);
        self.exit_spot = Some(exit_spot);
        self.status = PositionStatus::Closed;
        self.days_held = days_held;
        
        // Calculate realized P&L
        // Long: (exit - entry) * quantity
        // Short: (entry - exit) * quantity
        let price_diff = exit_price - self.entry_price;
        self.realized_pnl = price_diff * self.quantity as f64 * 100.0; // Options are per 100 shares
        self.unrealized_pnl = 0.0;
    }
    
    /// Mark position as expired (worthless)
    pub fn expire(&mut self, exit_date: String, exit_spot: f64, days_held: usize) {
        self.close(0.0, exit_date, exit_spot, days_held);
        self.status = PositionStatus::Expired;
    }
    
    /// Update unrealized P&L for open position
    pub fn update_unrealized_pnl(&mut self, current_price: f64) {
        if matches!(self.status, PositionStatus::Open) {
            let price_diff = current_price - self.entry_price;
            self.unrealized_pnl = price_diff * self.quantity as f64 * 100.0;
        }
    }
    
    /// Total P&L (realized + unrealized)
    pub fn total_pnl(&self) -> f64 {
        self.realized_pnl + self.unrealized_pnl
    }
    
    /// Return on investment (percentage)
    pub fn roi(&self) -> f64 {
        let cost_basis = self.entry_price * self.quantity.abs() as f64 * 100.0;
        if cost_basis > 0.0 {
            (self.realized_pnl / cost_basis) * 100.0
        } else {
            0.0
        }
    }
    
    /// Is this a winning trade?
    pub fn is_winner(&self) -> bool {
        self.realized_pnl > 0.0
    }
    
    /// Position direction (long/short)
    pub fn direction(&self) -> &str {
        if self.quantity > 0 { "LONG" } else { "SHORT" }
    }
}
