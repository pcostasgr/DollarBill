#![allow(dead_code)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
