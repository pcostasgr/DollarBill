// Backtesting framework for historical strategy evaluation
// Simulates option trading on historical data with full P&L and performance metrics
#![allow(unused_imports)]

pub mod position;
pub mod trade;
pub mod engine;
pub mod metrics;
pub mod liquidity;
pub mod ledger;
pub mod margin;
pub mod audit_log;
pub mod regime_pipeline;

pub use position::{Position, PositionStatus};
pub use trade::{Trade, TradeType};
pub use engine::{BacktestEngine, BacktestConfig, TradingCosts, SlippageModel, PartialFillModel};
pub use crate::strategies::SignalAction;
pub use metrics::{BacktestResult, PerformanceMetrics, EquityCurve};
pub use liquidity::{LiquidityTier, MidPriceImpact};
pub use ledger::Ledger;
pub use margin::{
    naked_call_margin, naked_put_margin,
    credit_spread_margin, iron_condor_margin, cash_secured_put_margin,
    has_sufficient_margin, MarginRequirement, MarginRule,
    max_loss_credit_spread, max_loss_iron_condor,
    max_profit_short, max_loss_naked_put,
};
pub use audit_log::{AuditLog, RegimeSizingAuditEntry};
pub use regime_pipeline::{RegimePipeline, PreTradeDecision};
