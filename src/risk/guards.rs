/// Shared intra-day risk guards used by both the backtesting engine and the live bot.
///
/// Extracting them here ensures that any change to a limit automatically applies to
/// both execution paths — preventing silent drift between backtest and live behaviour.

/// Outcome returned by every guard check.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardAction {
    /// New entries are allowed.
    Allow,
    /// New entries must be blocked; the reason string describes why.
    Halt { reason: String },
}

impl GuardAction {
    pub fn is_halt(&self) -> bool {
        matches!(self, GuardAction::Halt { .. })
    }
    /// Convenience: true when new entries are permitted.
    pub fn allows_entry(&self) -> bool {
        !self.is_halt()
    }
}

/// Limits that govern intra-day entry behaviour.
///
/// Both `BacktestConfig` and the live-bot config should be mapped into this struct
/// before calling any guard function.
#[derive(Debug, Clone)]
pub struct DailyRiskLimits {
    /// Halt new entries when today's equity drawdown from yesterday's close
    /// exceeds this fraction (e.g. `0.05` = 5%).  `None` = no breaker.
    pub max_daily_drawdown_pct: Option<f64>,
    /// Halt new entries once this many trades have been submitted today.
    /// `None` = no cap.
    pub max_daily_trades: Option<usize>,
}

impl Default for DailyRiskLimits {
    fn default() -> Self {
        Self {
            max_daily_drawdown_pct: Some(0.05),
            max_daily_trades: None,
        }
    }
}

/// Check whether the daily drawdown circuit breaker should halt new entries.
///
/// # Arguments
/// * `start_of_day_equity` – equity at market open (or start of backtest day).
/// * `current_equity`      – current mark-to-market equity.
/// * `limits`              – the configured risk limits.
pub fn check_daily_drawdown(
    start_of_day_equity: f64,
    current_equity: f64,
    limits: &DailyRiskLimits,
) -> GuardAction {
    let Some(max_dd) = limits.max_daily_drawdown_pct else {
        return GuardAction::Allow;
    };
    if start_of_day_equity <= 0.0 {
        return GuardAction::Allow;
    }
    let drawdown = (start_of_day_equity - current_equity) / start_of_day_equity;
    if drawdown >= max_dd {
        GuardAction::Halt {
            reason: format!(
                "daily drawdown {:.2}% >= limit {:.2}%",
                drawdown * 100.0,
                max_dd * 100.0,
            ),
        }
    } else {
        GuardAction::Allow
    }
}

/// Check whether the max-daily-trades cap should halt new entries.
///
/// # Arguments
/// * `trades_today` – number of trades already submitted today.
/// * `limits`       – the configured risk limits.
pub fn check_daily_trade_cap(trades_today: usize, limits: &DailyRiskLimits) -> GuardAction {
    let Some(max_trades) = limits.max_daily_trades else {
        return GuardAction::Allow;
    };
    if trades_today >= max_trades {
        GuardAction::Halt {
            reason: format!(
                "daily trade cap reached ({}/{})",
                trades_today, max_trades,
            ),
        }
    } else {
        GuardAction::Allow
    }
}

/// Convenience: run all daily guards and return the first `Halt`, or `Allow`.
pub fn check_all(
    start_of_day_equity: f64,
    current_equity: f64,
    trades_today: usize,
    limits: &DailyRiskLimits,
) -> GuardAction {
    let dd = check_daily_drawdown(start_of_day_equity, current_equity, limits);
    if dd.is_halt() {
        return dd;
    }
    check_daily_trade_cap(trades_today, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits_5pct_10trades() -> DailyRiskLimits {
        DailyRiskLimits {
            max_daily_drawdown_pct: Some(0.05),
            max_daily_trades: Some(10),
        }
    }

    #[test]
    fn allow_within_limits() {
        let lim = limits_5pct_10trades();
        assert!(check_all(100_000.0, 96_000.0, 5, &lim).allows_entry());
    }

    #[test]
    fn halt_on_drawdown() {
        let lim = limits_5pct_10trades();
        let r = check_all(100_000.0, 94_000.0, 5, &lim);
        assert!(r.is_halt());
        if let GuardAction::Halt { reason } = r {
            assert!(reason.contains("drawdown"));
        }
    }

    #[test]
    fn halt_on_trade_cap() {
        let lim = limits_5pct_10trades();
        let r = check_all(100_000.0, 99_000.0, 10, &lim);
        assert!(r.is_halt());
        if let GuardAction::Halt { reason } = r {
            assert!(reason.contains("trade cap"));
        }
    }

    #[test]
    fn no_drawdown_limit_when_none() {
        let lim = DailyRiskLimits { max_daily_drawdown_pct: None, max_daily_trades: None };
        assert!(check_all(100_000.0, 1.0, 999, &lim).allows_entry());
    }
}
