// Trade record for entry/exit transactions

use crate::models::bs_mod::Greeks;

#[derive(Debug, Clone)]
pub enum TradeType {
    Entry,
    Exit,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub position_id: usize,
    pub trade_type: TradeType,
    pub date: String,
    pub symbol: String,
    pub price: f64,
    pub quantity: i32,
    pub spot_price: f64,
    pub greeks: Option<Greeks>,
    pub commission: f64,
}

impl Trade {
    pub fn new(
        position_id: usize,
        trade_type: TradeType,
        date: String,
        symbol: String,
        price: f64,
        quantity: i32,
        spot_price: f64,
        greeks: Option<Greeks>,
        commission: f64,
    ) -> Self {
        Self {
            position_id,
            trade_type,
            date,
            symbol,
            price,
            quantity,
            spot_price,
            greeks,
            commission,
        }
    }
    
    /// Total value of the trade (excluding commission)
    pub fn value(&self) -> f64 {
        self.price * self.quantity.abs() as f64 * 100.0
    }
    
    /// Total cost including commission (per contract)
    pub fn total_cost(&self) -> f64 {
        self.value() + (self.commission * self.quantity.abs() as f64)
    }
}
