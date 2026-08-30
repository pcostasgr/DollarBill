/// Shared risk enforcement logic used by both the backtesting engine and the live bot.
pub mod guards;
pub mod invariants;
pub mod position_management;

pub use guards::{DailyRiskLimits, GuardAction, check_all, check_daily_drawdown, check_daily_trade_cap};
pub use invariants::{BotState, InvariantPosition, InvariantViolation, Invariant, assert_invariants};
pub use position_management::{
    ManagedPosition, ManagementAction, ManagementConfig, manage_open_positions,
};
