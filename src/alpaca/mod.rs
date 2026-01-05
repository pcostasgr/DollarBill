// Alpaca API integration for paper trading
// Provides live market data and simulated order execution

pub mod client;
pub mod types;

pub use client::AlpacaClient;
pub use types::*;
