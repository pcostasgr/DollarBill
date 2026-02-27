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
    /// Effective fill price after bid-ask spread applied
    pub price: f64,
    /// Theoretical mid-market price (before spread); used for slippage reporting
    pub mid_price: f64,
    pub quantity: i32,
    pub spot_price: f64,
    pub greeks: Option<Greeks>,
    /// Total commission for this trade (commission_per_contract × |quantity|)
    pub commission: f64,
    /// One-way bid-ask slippage cost in dollars (already baked into `price`)
    /// Informational: shows how much the spread cost on this leg
    pub slippage: f64,
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
            mid_price: price,  // default: mid == fill (zero spread)
            price,
            quantity,
            spot_price,
            greeks,
            commission,
            slippage: 0.0,
        }
    }

    /// Total value of the trade at fill price (excluding commission and slippage)
    pub fn value(&self) -> f64 {
        self.price * self.quantity.abs() as f64 * 100.0
    }

    /// Cost to BUY this many contracts at the fill price plus commissions.
    /// Use for debit (long-entry / short-exit) capital updates.
    pub fn total_cost(&self) -> f64 {
        self.value() + self.commission
    }

    /// Net proceeds from SELLING (short-entry / long-exit): fill value minus commissions.
    /// Use for credit capital updates.
    pub fn proceeds(&self) -> f64 {
        (self.value() - self.commission).max(0.0)
    }
}
