/// Shared risk enforcement logic used by both the backtesting engine and the live bot.
pub mod guards;

pub use guards::{DailyRiskLimits, GuardAction, check_all, check_daily_drawdown, check_daily_trade_cap};
