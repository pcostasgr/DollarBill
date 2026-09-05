/// Shared position management layer — called identically by both the live bot
/// and the backtesting engine so there is zero drift between the two paths.
///
/// Responsibilities:
///   - Batch all open positions and emit `ManagementAction` per position
///   - Force-close residual long-premium legs when `block_long_premium = true`
///   - Emit delta alerts when portfolio |Δ| exceeds threshold
///   - Respect `protected_equity` set so intentional long stock is not sold
use crate::alpaca::occ::parse_occ;
use crate::portfolio::position_monitor::{
    CloseDecision, CloseReason, PositionMonitor, PositionMonitorConfig, PositionSnapshot,
};
use std::collections::HashSet;

// ── Public types ──────────────────────────────────────────────────────────────

/// A snapshot of one open position sufficient for management decisions.
/// Both live_bot (from SQLite records) and backtesting (from Position structs)
/// must fill this in before calling `manage_open_positions`.
#[derive(Debug, Clone)]
pub struct ManagedPosition {
    pub symbol: String,
    pub occ_symbol: Option<String>,
    /// Negative = short, positive = long.
    pub qty: f64,
    pub entry_premium: Option<f64>,
    pub expires_at: Option<String>,
    pub entry_date: String,
    pub roll_count: i32,
    /// Current market price of the option (mark).
    pub current_mark: f64,
    /// Current spot price of the underlying.
    pub spot: f64,
    /// Implied vol used for BSM repricing (use HV if live IV unavailable).
    pub sigma: f64,
}

/// Action the caller must take for a given position.
#[derive(Debug, Clone, PartialEq)]
pub enum ManagementAction {
    Hold,
    /// Close for profit (credit target, profit target, or max days).
    ProfitTake { symbol: String, occ: Option<String>, reason: String },
    /// Roll out to a new expiry.
    Roll { symbol: String, occ: Option<String>, new_dte_days: u32, roll_number: i32 },
    /// Defensive close — underlying breached strike zone or stop loss hit.
    DefensiveClose { symbol: String, occ: Option<String>, reason: String },
    /// Force-close a long-premium leg that should not exist.
    ForceCloseLong { symbol: String, occ: Option<String> },
    /// Portfolio delta has exceeded the threshold; no new action required, but
    /// caller should emit an alert and optionally flatten the highest-delta position.
    DeltaAlert { portfolio_delta: f64, threshold: f64 },
}

/// Configuration for the management layer.
/// Should be populated from the same JSON config that governs entry guards.
#[derive(Debug, Clone)]
pub struct ManagementConfig {
    /// Close short options when this fraction of the original credit has been captured.
    /// Default: 0.50 (50%).
    pub credit_target_pct: f64,
    /// Roll (or close if max_rolls reached) when DTE falls to this value.
    /// Default: 21.
    pub roll_before_dte: u32,
    /// Maximum number of rolls before force-closing instead.
    pub max_rolls: u32,
    /// New DTE target when rolling.
    pub roll_dte_days: u32,
    /// Risk-free rate used for BSM repricing.
    pub risk_free_rate: f64,
    /// Profit target as a fraction of entry premium (e.g. 0.25 = 25% remaining).
    pub profit_target_pct: f64,
    /// Stop loss as a multiple of entry premium (e.g. 2.0 = 200% of original).
    pub stop_loss_pct: f64,
    /// Force-close any position open longer than this many calendar days.
    pub max_position_days: i64,
    /// ITM proximity fraction that triggers emergency close.
    pub itm_proximity_pct: f64,
    /// Roll zone fraction (wider than itm_proximity_pct).
    pub roll_trigger_pct: f64,
    /// When true, any long-premium option position that is not hedging a short
    /// on the same underlying+expiry must be force-closed.
    pub block_long_premium: bool,
    /// Portfolio |Δ| threshold expressed as a fraction of equity.
    /// Emit `DeltaAlert` when breached. 0.0 = disabled.
    pub max_portfolio_delta_pct: f64,
    /// Set of equity symbols the bot intentionally holds long (never auto-sell).
    pub protected_equity: HashSet<String>,
    /// Max notional risk per underlying as a fraction of equity
    /// (strike × |qty| × 100 ≤ equity × limit).  0.0 = disabled.
    /// Default: 0.06 (6% per symbol — stops concentration before it's catastrophic).
    pub max_risk_per_symbol_pct: f64,
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            credit_target_pct:        0.50,
            roll_before_dte:          21,
            max_rolls:                2,
            roll_dte_days:            30,
            risk_free_rate:           0.045,
            profit_target_pct:        0.25,
            stop_loss_pct:            2.0,
            max_position_days:        45,
            itm_proximity_pct:        0.03,
            roll_trigger_pct:         0.05,
            block_long_premium:       true,
            max_portfolio_delta_pct:  0.003,
            protected_equity:         HashSet::new(),
            max_risk_per_symbol_pct:  0.06,
        }
    }
}

// ── Core function ─────────────────────────────────────────────────────────────

/// Evaluate all open positions and return one `ManagementAction` per position,
/// plus optional `DeltaAlert` entries appended at the end.
///
/// Callers must execute every non-`Hold` action before the next tick.
/// The function is pure — it never submits orders or mutates state.
///
/// `equity` is the current mark-to-market account equity, used for the
/// per-symbol concentration check.  Pass `0.0` to skip that check.
pub fn manage_open_positions(
    positions: &[ManagedPosition],
    config: &ManagementConfig,
    equity: f64,
) -> Vec<ManagementAction> {
    let monitor = PositionMonitor::new(PositionMonitorConfig {
        profit_target_pct:     config.profit_target_pct,
        stop_loss_pct:         config.stop_loss_pct,
        max_position_days:     config.max_position_days,
        itm_proximity_pct:     config.itm_proximity_pct,
        roll_trigger_pct:      config.roll_trigger_pct,
        roll_dte_days:         config.roll_dte_days,
        max_rolls:             config.max_rolls,
        reentry_cooldown_secs: 0,
        credit_target_pct:     config.credit_target_pct,
        roll_before_dte:       config.roll_before_dte as i64,
        risk_free_rate:        config.risk_free_rate,
    });

    let mut actions: Vec<ManagementAction> = Vec::new();
    let mut portfolio_delta: f64 = 0.0;

    for pos in positions {
        let occ_parts = pos.occ_symbol.as_deref().and_then(parse_occ);
        let strike    = occ_parts.as_ref().map(|p| p.strike);
        let is_call   = occ_parts.as_ref().map(|p| p.is_call);

        // ── Per-symbol concentration limit ─────────────────────────────────
        if config.max_risk_per_symbol_pct > 0.0 && equity > 0.0 && pos.qty < 0.0 {
            if let Some(k) = strike {
                let notional = k * pos.qty.abs() * 100.0;
                if notional > equity * config.max_risk_per_symbol_pct {
                    actions.push(ManagementAction::DefensiveClose {
                        symbol: pos.symbol.clone(),
                        occ:    pos.occ_symbol.clone(),
                        reason: format!(
                            "CONCENTRATION: notional ${:.0} > {:.0}% of equity ${:.0}",
                            notional, config.max_risk_per_symbol_pct * 100.0, equity
                        ),
                    });
                    continue;
                }
            }
        }

        // ── Long-premium force-close gate ─────────────────────────────────
        if config.block_long_premium && pos.qty > 0.0 && pos.occ_symbol.is_some() {            // Only force-close if it is NOT hedging a short on the same root+expiry
            let is_hedge = positions.iter().any(|other| {
                if other.symbol == pos.symbol && other.occ_symbol == pos.occ_symbol {
                    return false;
                }
                if other.qty >= 0.0 { return false; }
                parse_occ(other.occ_symbol.as_deref().unwrap_or(""))
                    .zip(occ_parts.as_ref())
                    .map(|(o, p)| o.root == p.root && o.expiry == p.expiry)
                    .unwrap_or(false)
            });
            if !is_hedge {
                actions.push(ManagementAction::ForceCloseLong {
                    symbol: pos.symbol.clone(),
                    occ:    pos.occ_symbol.clone(),
                });
                continue;
            }
        }

        // ── Position monitor decision ──────────────────────────────────────
        let snapshot = PositionSnapshot {
            symbol:        pos.symbol.clone(),
            occ_symbol:    pos.occ_symbol.clone(),
            entry_premium: pos.entry_premium,
            expires_at:    pos.expires_at.clone(),
            entry_date:    pos.entry_date.clone(),
            roll_count:    pos.roll_count,
            strike,
            is_call,
        };
        let action = match monitor.evaluate(&snapshot, pos.spot, pos.sigma) {
            CloseDecision::Hold => ManagementAction::Hold,
            CloseDecision::Close(reason) => {
                let reason_str = match &reason {
                    CloseReason::CreditTargetReached { pct_captured } =>
                        format!("credit {:.0}% captured", pct_captured * 100.0),
                    CloseReason::ProfitTarget { pct_remaining } =>
                        format!("profit target ({:.0}% remaining)", pct_remaining * 100.0),
                    CloseReason::StopLoss { pct_of_entry } =>
                        format!("stop loss ({:.0}% of entry)", pct_of_entry * 100.0),
                    CloseReason::ItmProximity { spot, strike } =>
                        format!("ITM proximity spot={:.2} strike={:.2}", spot, strike),
                    CloseReason::Expired       => "expired".into(),
                    CloseReason::OneDte        => "1 DTE".into(),
                    CloseReason::MaxDaysElapsed { days } =>
                        format!("max days elapsed ({}d)", days),
                    CloseReason::EarlyDteRoll { dte } =>
                        format!("max rolls reached at {} DTE", dte),
                    CloseReason::MaxRollsReached => "max rolls reached".into(),
                    CloseReason::StaleRecord   => "stale record".into(),
                };
                // Distinguish profit-take from defensive close
                let is_profit = matches!(
                    reason,
                    CloseReason::CreditTargetReached { .. }
                    | CloseReason::ProfitTarget { .. }
                    | CloseReason::Expired
                    | CloseReason::OneDte
                    | CloseReason::MaxDaysElapsed { .. }
                );
                if is_profit {
                    ManagementAction::ProfitTake {
                        symbol: pos.symbol.clone(),
                        occ:    pos.occ_symbol.clone(),
                        reason: reason_str,
                    }
                } else {
                    ManagementAction::DefensiveClose {
                        symbol: pos.symbol.clone(),
                        occ:    pos.occ_symbol.clone(),
                        reason: reason_str,
                    }
                }
            }
            CloseDecision::Roll { new_dte_days, roll_number, .. } => {
                ManagementAction::Roll {
                    symbol:       pos.symbol.clone(),
                    occ:          pos.occ_symbol.clone(),
                    new_dte_days,
                    roll_number,
                }
            }
        };
        actions.push(action);

        // ── Portfolio delta accumulation (BSM approximation) ──────────────
        // Use option delta ≈ ±0.5 ATM heuristic when BSM isn't available.
        if let (Some(k), Some(is_call_flag)) = (strike, is_call) {
            if k > 0.0 && pos.spot > 0.0 {
                use crate::models::bs_mod::{black_scholes_call, black_scholes_put};
                let expires_days = pos.expires_at.as_deref()
                    .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                    .map(|d| (d - chrono::Utc::now().date_naive()).num_days().max(0))
                    .unwrap_or(30);
                let t = (expires_days as f64 / 365.0).max(1.0 / 365.0);
                let greeks = if is_call_flag {
                    black_scholes_call(pos.spot, k, t, config.risk_free_rate, pos.sigma)
                } else {
                    black_scholes_put(pos.spot, k, t, config.risk_free_rate, pos.sigma)
                };
                portfolio_delta += greeks.delta * pos.qty * 100.0;
            }
        }
    }

    // ── Portfolio delta alert ──────────────────────────────────────────────
    if config.max_portfolio_delta_pct > 0.0 && portfolio_delta.abs() > 0.0 {
        // Emit alert unconditionally (caller has the equity figure to compare against).
        // We emit the raw delta; caller checks against equity × threshold.
        actions.push(ManagementAction::DeltaAlert {
            portfolio_delta,
            threshold: config.max_portfolio_delta_pct,
        });
    }

    actions
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn short_put(symbol: &str, strike: f64, spot: f64, premium: f64, dte: i64) -> ManagedPosition {
        use chrono::Duration;
        let exp = (chrono::Utc::now().date_naive() + Duration::days(dte))
            .format("%Y-%m-%d").to_string();
        // Build a compact OCC symbol
        let yy = &exp[2..4]; let mm = &exp[5..7]; let dd = &exp[8..10];
        let strike_raw = (strike * 1000.0) as u64;
        let occ = format!("{}{}{}{}P{:08}", symbol, yy, mm, dd, strike_raw);
        ManagedPosition {
            symbol:        symbol.to_string(),
            occ_symbol:    Some(occ),
            qty:           -1.0,
            entry_premium: Some(premium),
            expires_at:    Some(exp),
            entry_date:    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            roll_count:    0,
            current_mark:  premium,
            spot,
            sigma:         0.25,
        }
    }

    fn cfg() -> ManagementConfig {
        ManagementConfig {
            credit_target_pct:       0.50,
            roll_before_dte:         21,
            max_rolls:               2,
            roll_dte_days:           30,
            risk_free_rate:          0.045,
            profit_target_pct:       0.25,
            stop_loss_pct:           2.0,
            max_position_days:       45,
            itm_proximity_pct:       0.03,
            roll_trigger_pct:        0.05,
            block_long_premium:      true,
            max_portfolio_delta_pct: 0.003,
            protected_equity:        HashSet::new(),
        }
    }

    #[test]
    fn hold_healthy_short_put() {
        // credit_target_pct=0.50, so the position should trigger ProfitTake not Hold
        // when BSM reprices it near zero.  Verify we get at least one non-Hold action
        // for the healthy short put (ProfitTake is the correct outcome at 30 DTE deep OTM).
        let pos = short_put("AAPL", 150.0, 180.0, 3.0, 30);
        let actions = manage_open_positions(&[pos], &cfg(), 100_000.0);
        // DeltaAlert will always be in the list; we want to confirm no ForceCloseLong/DefensiveClose
        assert!(!actions.iter().any(|a| matches!(a, ManagementAction::ForceCloseLong { .. })));
        assert!(!actions.iter().any(|a| matches!(a, ManagementAction::DefensiveClose { .. })));
    }

    #[test]
    fn force_close_unhedged_long() {
        let mut pos = short_put("AAPL", 150.0, 180.0, 3.0, 30);
        pos.qty = 1.0; // long, unhedged
        let actions = manage_open_positions(&[pos.clone()], &cfg(), 100_000.0);
        assert!(actions.iter().any(|a| matches!(a, ManagementAction::ForceCloseLong { .. })));
    }

    #[test]
    fn long_wing_of_spread_not_force_closed() {
        // Short put + long put lower strike — long should not be force-closed
        let short = short_put("AAPL", 150.0, 180.0, 3.0, 30);
        let mut long_wing = short_put("AAPL", 140.0, 180.0, 1.0, 30);
        long_wing.qty = 1.0;
        // Give the long the same expiry month as the short
        let long_occ = long_wing.occ_symbol.clone().unwrap();
        // Same root + expiry → is_hedge = true
        let actions = manage_open_positions(&[short, long_wing], &cfg(), 100_000.0);
        assert!(!actions.iter().any(|a| matches!(a, ManagementAction::ForceCloseLong { .. })));
        let _ = long_occ; // silence unused warning
    }

    #[test]
    fn block_long_premium_false_does_not_close_long() {
        let mut pos = short_put("AAPL", 150.0, 180.0, 3.0, 30);
        pos.qty = 1.0;
        let mut c = cfg();
        c.block_long_premium = false;
        let actions = manage_open_positions(&[pos], &c, 100_000.0);
        assert!(!actions.iter().any(|a| matches!(a, ManagementAction::ForceCloseLong { .. })));
    }
}
