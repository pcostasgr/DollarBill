// Backtesting framework for historical strategy evaluation
// Simulates option trading on historical data with full P&L and performance metrics

pub mod position;
pub mod trade;
pub mod engine;
pub mod metrics;

pub use position::{Position, PositionStatus};
pub use trade::{Trade, TradeType};
pub use engine::{BacktestEngine, BacktestConfig, TradingCosts, SlippageModel};
pub use crate::strategies::SignalAction;
pub use metrics::{BacktestResult, PerformanceMetrics, EquityCurve};
