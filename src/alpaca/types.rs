// Type definitions for Alpaca API

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub account_number: String,
    pub status: String,
    pub currency: String,
    pub buying_power: String,
    pub cash: String,
    pub portfolio_value: String,
    pub equity: String,
    pub last_equity: String,
    pub long_market_value: String,
    pub short_market_value: String,
    pub initial_margin: String,
    pub maintenance_margin: String,
    pub daytrade_count: i32,
    pub pattern_day_trader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub asset_id: String,
    pub symbol: String,
    pub exchange: String,
    pub asset_class: String,
    pub avg_entry_price: String,
    pub qty: String,
    pub side: String,
    pub market_value: String,
    pub cost_basis: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
    pub unrealized_intraday_pl: String,
    pub unrealized_intraday_plpc: String,
    pub current_price: String,
    pub lastday_price: String,
    pub change_today: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub client_order_id: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub submitted_at: Option<String>,
    pub filled_at: Option<String>,
    pub expired_at: Option<String>,
    pub canceled_at: Option<String>,
    pub failed_at: Option<String>,
    pub replaced_at: Option<String>,
    pub asset_id: String,
    pub symbol: String,
    pub asset_class: String,
    pub qty: String,
    pub filled_qty: String,
    pub order_type: String,
    pub side: String,
    pub time_in_force: String,
    pub limit_price: Option<String>,
    pub stop_price: Option<String>,
    pub filled_avg_price: Option<String>,
    pub status: String,
    pub extended_hours: bool,
    pub legs: Option<Vec<Order>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub qty: f64,
    pub side: OrderSide,
    pub r#type: OrderType,
    pub time_in_force: TimeInForce,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extended_hours: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeInForce {
    Day,
    Gtc,  // Good til canceled
    Opg,  // Market on open
    Cls,  // Market on close
    Ioc,  // Immediate or cancel
    Fok,  // Fill or kill
}

/// Alpaca market clock — is the market currently open?
#[derive(Debug, Clone, Deserialize)]
pub struct Clock {
    pub timestamp: String,
    pub is_open: bool,
    pub next_open: String,
    pub next_close: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bar {
    pub t: String,  // Timestamp
    pub o: f64,     // Open
    pub h: f64,     // High
    pub l: f64,     // Low
    pub c: f64,     // Close
    pub v: u64,     // Volume
}

#[derive(Debug, Clone, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
    pub bid_size: i32,
    pub ask_size: i32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub size: i32,
    pub timestamp: String,
    pub exchange: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Snapshot {
    pub symbol: String,
    pub latest_trade: Option<Trade>,
    pub latest_quote: Option<Quote>,
    pub minute_bar: Option<Bar>,
    pub daily_bar: Option<Bar>,
    pub prev_daily_bar: Option<Bar>,
}

impl Account {
    /// Parse buying_power as f64; returns None if missing or non-numeric.
    pub fn buying_power_f64(&self) -> Option<f64> { self.buying_power.parse().ok() }
    /// Parse cash as f64.
    pub fn cash_f64(&self) -> Option<f64> { self.cash.parse().ok() }
    /// Parse portfolio_value as f64.
    pub fn portfolio_value_f64(&self) -> Option<f64> { self.portfolio_value.parse().ok() }
    /// Parse equity as f64.
    pub fn equity_f64(&self) -> Option<f64> { self.equity.parse().ok() }
    /// Parse last_equity as f64.
    pub fn last_equity_f64(&self) -> Option<f64> { self.last_equity.parse().ok() }
    /// Parse long_market_value as f64.
    pub fn long_market_value_f64(&self) -> Option<f64> { self.long_market_value.parse().ok() }
    /// Parse short_market_value as f64.
    pub fn short_market_value_f64(&self) -> Option<f64> { self.short_market_value.parse().ok() }
    /// Parse initial_margin as f64.
    pub fn initial_margin_f64(&self) -> Option<f64> { self.initial_margin.parse().ok() }
    /// Parse maintenance_margin as f64.
    pub fn maintenance_margin_f64(&self) -> Option<f64> { self.maintenance_margin.parse().ok() }
    /// Equity change since last close (equity - last_equity); None if either field is non-numeric.
    pub fn daily_pnl_f64(&self) -> Option<f64> {
        Some(self.equity_f64()? - self.last_equity_f64()?)
    }
}

impl Position {
    /// Parse avg_entry_price as f64.
    pub fn avg_entry_price_f64(&self) -> Option<f64> { self.avg_entry_price.parse().ok() }
    /// Parse qty as f64 (negative for short positions).
    pub fn qty_f64(&self) -> Option<f64> { self.qty.parse().ok() }
    /// Parse market_value as f64.
    pub fn market_value_f64(&self) -> Option<f64> { self.market_value.parse().ok() }
    /// Parse cost_basis as f64.
    pub fn cost_basis_f64(&self) -> Option<f64> { self.cost_basis.parse().ok() }
    /// Parse unrealized_pl as f64.
    pub fn unrealized_pl_f64(&self) -> Option<f64> { self.unrealized_pl.parse().ok() }
    /// Parse unrealized_plpc (percent) as f64.
    pub fn unrealized_plpc_f64(&self) -> Option<f64> { self.unrealized_plpc.parse().ok() }
    /// Parse unrealized_intraday_pl as f64.
    pub fn unrealized_intraday_pl_f64(&self) -> Option<f64> { self.unrealized_intraday_pl.parse().ok() }
    /// Parse unrealized_intraday_plpc (percent) as f64.
    pub fn unrealized_intraday_plpc_f64(&self) -> Option<f64> { self.unrealized_intraday_plpc.parse().ok() }
    /// Parse current_price as f64.
    pub fn current_price_f64(&self) -> Option<f64> { self.current_price.parse().ok() }
    /// Parse lastday_price as f64.
    pub fn lastday_price_f64(&self) -> Option<f64> { self.lastday_price.parse().ok() }
    /// Parse change_today (percent) as f64.
    pub fn change_today_f64(&self) -> Option<f64> { self.change_today.parse().ok() }
}

/// A single leg of a multi-leg options order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsLeg {
    /// OCC option symbol, e.g. "AAPL250117C00150000"
    pub symbol: String,
    /// Number of contracts for this leg (always positive).
    pub ratio_qty: u32,
    pub side: OrderSide,
    /// "buy_to_open" | "buy_to_close" | "sell_to_open" | "sell_to_close"
    pub position_intent: String,
}

/// Request body for a multi-leg options order (iron condor, vertical spread, straddle, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsOrderRequest {
    pub r#type: OrderType,
    pub time_in_force: TimeInForce,
    /// Must be "mleg" for multi-leg orders.
    pub order_class: String,
    pub legs: Vec<OptionsLeg>,
    /// Net debit (positive) or credit (negative) limit price; None for market orders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}
