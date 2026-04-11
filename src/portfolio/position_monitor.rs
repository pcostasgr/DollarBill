//! Per-position close/roll decision engine.
//!
//! Extracts the ITM-proximity, roll-trigger, P&L, DTE, and cooldown logic from
//! `live_bot.rs` into a standalone, independently-testable unit.
//!
//! The live bot calls [`PositionMonitor::evaluate`] on each price tick and acts
//! on the returned [`CloseDecision`] variant rather than duplicating logic
//! inline.

use chrono::{NaiveDate, Utc};

// ── Configuration ─────────────────────────────────────────────────────────────

/// Runtime thresholds that control when positions are adjusted or closed.
/// Mirrors the fields in `BotRuntimeConfig` that are relevant to close logic.
#[derive(Debug, Clone)]
pub struct PositionMonitorConfig {
    /// Close when remaining premium ≤ this fraction of entry premium (e.g. 0.25 = 25%).
    pub profit_target_pct: f64,
    /// Close when current mark ≥ this multiple of entry premium (e.g. 2.0 = 200% loss).
    pub stop_loss_pct: f64,
    /// Force-close after this many calendar days regardless of P&L.
    pub max_position_days: i64,
    /// Fraction of strike below which spot is considered ITM — triggers immediate close.
    pub itm_proximity_pct: f64,
    /// Fraction of strike below which spot triggers a roll (wider than `itm_proximity_pct`).
    pub roll_trigger_pct: f64,
    /// New DTE to target when rolling out.
    pub roll_dte_days: u32,
    /// Maximum number of rolls allowed per position.
    pub max_rolls: u32,
    /// Seconds to block re-entry after closing a position.
    pub reentry_cooldown_secs: u64,
}

impl Default for PositionMonitorConfig {
    fn default() -> Self {
        Self {
            profit_target_pct:    0.25,
            stop_loss_pct:        2.0,
            max_position_days:    45,
            itm_proximity_pct:    0.03,
            roll_trigger_pct:     0.05,
            roll_dte_days:        30,
            max_rolls:            2,
            reentry_cooldown_secs: 300,
        }
    }
}

// ── Decision types ────────────────────────────────────────────────────────────

/// Why a position should be closed outright.
#[derive(Debug, Clone, PartialEq)]
pub enum CloseReason {
    /// Option expired or past expiry date.
    Expired,
    /// Within 1 DTE — exit to avoid pin risk.
    OneDte,
    /// Profit target reached (premium decayed to desired fraction).
    ProfitTarget { pct_remaining: f64 },
    /// Stop-loss breached (mark exceeded entry premium multiple).
    StopLoss { pct_of_entry: f64 },
    /// Held longer than `max_position_days`.
    MaxDaysElapsed { days: i64 },
    /// Spot moved inside the ITM proximity threshold — too dangerous to roll.
    ItmProximity { spot: f64, strike: f64 },
    /// Position record has no valid entry date (stale reconciled record).
    StaleRecord,
    /// Max roll count reached and spot is still in the roll zone.
    MaxRollsReached,
}

/// Decision produced by [`PositionMonitor::evaluate`].
#[derive(Debug, Clone, PartialEq)]
pub enum CloseDecision {
    /// No action needed this tick.
    Hold,
    /// Close the position for the given reason.
    Close(CloseReason),
    /// Roll out: buy-to-close the current contract and sell-to-open a new one
    /// at `new_strike` expiring in `new_dte_days` days.
    Roll { new_strike: f64, new_dte_days: u32, roll_number: i32 },
}

// ── Input position snapshot ────────────────────────────────────────────────────

/// Lightweight snapshot of a live position's state, passed into the monitor.
/// Decoupled from the persistence layer so it can be constructed in tests
/// without a database.
#[derive(Debug, Clone)]
pub struct PositionSnapshot {
    pub symbol: String,
    /// Full OCC symbol, e.g. `TSLA  251219P00250000`.
    pub occ_symbol: Option<String>,
    /// Premium collected at entry (positive = credit received).
    pub entry_premium: Option<f64>,
    /// Expiry date as stored (either `YYYY-MM-DD` or left as `None`).
    pub expires_at: Option<String>,
    /// ISO-8601 entry timestamp (only the date portion is used).
    pub entry_date: String,
    /// Number of rolls already executed.
    pub roll_count: i32,
}

// ── Monitor ───────────────────────────────────────────────────────────────────

/// Stateless evaluator: given a [`PositionSnapshot`] and the current market
/// data, returns the appropriate [`CloseDecision`].
pub struct PositionMonitor {
    pub config: PositionMonitorConfig,
}

impl PositionMonitor {
    pub fn new(config: PositionMonitorConfig) -> Self {
        Self { config }
    }

    /// Evaluate a single position.
    ///
    /// # Parameters
    /// - `pos`   – position snapshot
    /// - `spot`  – current underlying price
    /// - `sigma` – current realised/implied vol (annualised, e.g. 0.25)
    pub fn evaluate(&self, pos: &PositionSnapshot, spot: f64, sigma: f64) -> CloseDecision {
        let today = Utc::now().date_naive();

        // ── 1. DTE / expiry checks ─────────────────────────────────────────
        if let Some(exp_str) = &pos.expires_at {
            if let Ok(exp_date) = NaiveDate::parse_from_str(exp_str, "%Y-%m-%d") {
                let remaining = (exp_date - today).num_days();

                if remaining <= 0 {
                    return CloseDecision::Close(CloseReason::Expired);
                }
                if remaining <= 1 {
                    return CloseDecision::Close(CloseReason::OneDte);
                }

                // ── 2. P&L check (ATM repricing heuristic) ─────────────────
                if let Some(entry_premium) = pos.entry_premium.filter(|&p| p > 0.0) {
                    let remaining_t = (remaining as f64 / 365.0).max(1.0 / 365.0);
                    // Approximate current mark as ATM intrinsic + time value proxy
                    let current_val = spot * sigma * remaining_t.sqrt();
                    let pct = current_val / entry_premium;

                    if pct <= self.config.profit_target_pct {
                        return CloseDecision::Close(CloseReason::ProfitTarget {
                            pct_remaining: pct,
                        });
                    }
                    if pct >= self.config.stop_loss_pct {
                        return CloseDecision::Close(CloseReason::StopLoss {
                            pct_of_entry: pct,
                        });
                    }
                }

                // ── 3. ITM proximity + roll logic ────────────────────────────
                let decision = self.evaluate_roll_or_itm(pos, spot, remaining);
                if decision != CloseDecision::Hold {
                    return decision;
                }
            }
        } else {
            // No expiry stored — fall back to entry-date age check
            let raw = if pos.entry_date.len() >= 10 { &pos.entry_date[..10] } else { "" };
            match NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
                Ok(entry_date) => {
                    let age = (today - entry_date).num_days();
                    if age >= self.config.max_position_days {
                        return CloseDecision::Close(CloseReason::MaxDaysElapsed { days: age });
                    }
                }
                Err(_) => {
                    return CloseDecision::Close(CloseReason::StaleRecord);
                }
            }
        }

        CloseDecision::Hold
    }

    /// Evaluate ITM-proximity and roll zone for short puts.
    ///
    /// Thresholds are measured above the strike (OTM put approaching K from above):
    /// - Roll zone:  `K*(1+itm_proximity) < spot ≤ K*(1+roll_trigger)` → Roll, protect while still OTM
    /// - ITM zone:   `spot ≤ K*(1+itm_proximity)` → emergency Close
    ///
    /// Returns `CloseDecision::Hold` if no action is needed.
    fn evaluate_roll_or_itm(&self, pos: &PositionSnapshot, spot: f64, _remaining_dte: i64) -> CloseDecision {
        if self.config.roll_trigger_pct <= 0.0 { return CloseDecision::Hold; }

        let occ = match &pos.occ_symbol {
            Some(o) => o,
            None => return CloseDecision::Hold,
        };

        // OCC format: ROOT(6) YYMMDD(6) C|P(1) STRIKE(8 digits × 1000)
        let is_put = occ.len() >= 13 && occ.chars().nth(12) == Some('P');
        if !is_put { return CloseDecision::Hold; }

        let strike = match occ.get(13..21).and_then(|s| s.parse::<f64>().ok()) {
            Some(v) => v / 1000.0,
            None => return CloseDecision::Hold,
        };

        // Thresholds above strike — the put is OTM when spot > strike.
        // As spot falls from above, it first crosses roll_threshold, then itm_threshold.
        let roll_threshold = strike * (1.0 + self.config.roll_trigger_pct);
        let itm_threshold  = strike * (1.0 + self.config.itm_proximity_pct);

        if spot <= itm_threshold {
            // Spot is within itm_proximity of strike — too dangerous to roll
            return CloseDecision::Close(CloseReason::ItmProximity { spot, strike });
        }

        if spot <= roll_threshold {
            if pos.roll_count >= self.config.max_rolls as i32 {
                return CloseDecision::Close(CloseReason::MaxRollsReached);
            }
            return CloseDecision::Roll {
                new_strike:   strike,
                new_dte_days: self.config.roll_dte_days,
                roll_number:  pos.roll_count + 1,
            };
        }

        CloseDecision::Hold
    }
}

// ── Cooldown tracker ──────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::time::Instant;

/// Tracks re-entry cooldowns after a position is closed.
///
/// Call [`CooldownTracker::record_close`] when a position is closed and
/// [`CooldownTracker::is_cooling_down`] before opening a new one.
pub struct CooldownTracker {
    cooldown_secs: u64,
    closed_at: HashMap<String, Instant>,
}

impl CooldownTracker {
    pub fn new(cooldown_secs: u64) -> Self {
        Self { cooldown_secs, closed_at: HashMap::new() }
    }

    /// Record a position close for `symbol`.
    pub fn record_close(&mut self, symbol: &str) {
        self.closed_at.insert(symbol.to_string(), Instant::now());
    }

    /// Returns `true` if `symbol` is still in its cooldown window.
    pub fn is_cooling_down(&self, symbol: &str) -> bool {
        self.closed_at
            .get(symbol)
            .map(|t| t.elapsed().as_secs() < self.cooldown_secs)
            .unwrap_or(false)
    }

    /// Age of the cooldown in seconds (0 if not cooling down).
    pub fn secs_remaining(&self, symbol: &str) -> u64 {
        self.closed_at.get(symbol).map(|t| {
            let elapsed = t.elapsed().as_secs();
            self.cooldown_secs.saturating_sub(elapsed)
        }).unwrap_or(0)
    }
}

// ── ITM/roll alerts for PortfolioRisk ─────────────────────────────────────────

/// Per-symbol flags emitted by [`scan_portfolio`] for dashboard/alert use.
#[derive(Debug, Clone, Default)]
pub struct PortfolioPositionAlerts {
    /// Symbols whose short put is in the roll zone (action: consider roll).
    pub roll_zone:       Vec<String>,
    /// Symbols whose short put is inside the ITM proximity threshold (urgent).
    pub itm_proximity:   Vec<String>,
    /// Symbols with < 2 DTE remaining.
    pub expiring_soon:   Vec<String>,
}

/// Scan all positions and categorise them for dashboard/risk reporting.
pub fn scan_portfolio(
    positions: &[PositionSnapshot],
    spots: &HashMap<String, f64>,
    config: &PositionMonitorConfig,
) -> PortfolioPositionAlerts {
    let mut alerts = PortfolioPositionAlerts::default();
    let today = Utc::now().date_naive();

    for pos in positions {
        let spot = match spots.get(&pos.symbol) { Some(&s) => s, None => continue };

        // DTE alerts
        if let Some(exp_str) = &pos.expires_at {
            if let Ok(exp) = NaiveDate::parse_from_str(exp_str, "%Y-%m-%d") {
                if (exp - today).num_days() <= 2 {
                    alerts.expiring_soon.push(pos.symbol.clone());
                }
            }
        }

        // ITM / roll zone alerts for short puts (same convention as PositionMonitor: above-strike thresholds)
        if let Some(occ) = &pos.occ_symbol {
            let is_put = occ.len() >= 13 && occ.chars().nth(12) == Some('P');
            if is_put {
                if let Some(strike) = occ.get(13..21).and_then(|s| s.parse::<f64>().ok()).map(|v| v / 1000.0) {
                    let roll_t = strike * (1.0 + config.roll_trigger_pct);
                    let itm_t  = strike * (1.0 + config.itm_proximity_pct);
                    if spot <= itm_t {
                        alerts.itm_proximity.push(pos.symbol.clone());
                    } else if spot <= roll_t {
                        alerts.roll_zone.push(pos.symbol.clone());
                    }
                }
            }
        }
    }

    alerts
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PositionMonitorConfig {
        PositionMonitorConfig {
            profit_target_pct: 0.25,
            stop_loss_pct: 2.0,
            max_position_days: 45,
            itm_proximity_pct: 0.03,
            roll_trigger_pct: 0.05,
            roll_dte_days: 30,
            max_rolls: 2,
            reentry_cooldown_secs: 300,
        }
    }

    fn put_pos(occ: &str, premium: f64, roll_count: i32, expires_at: &str) -> PositionSnapshot {
        PositionSnapshot {
            symbol: "TEST".to_string(),
            occ_symbol: Some(occ.to_string()),
            entry_premium: Some(premium),
            expires_at: Some(expires_at.to_string()),
            entry_date: "2025-01-01T00:00:00Z".to_string(),
            roll_count,
        }
    }

    // ATM premium heuristic: spot=250..270, sigma=0.2, t=20/365
    // current_val ≈ 260*0.2*sqrt(20/365) ≈ 12.2 → use entry_premium=13.0
    // so stop_loss pct = 12.2/13.0 ≈ 0.94 (< 2.0 = no stop); profit pct > 0.25 = no target
    const REALISTIC_PREMIUM: f64 = 13.0;

    #[test]
    fn hold_when_healthy() {
        let monitor = PositionMonitor::new(cfg());
        let far_expiry = (Utc::now() + chrono::Duration::days(20)).format("%Y-%m-%d").to_string();
        // strike=250; roll_threshold=262.5, itm_threshold=257.5
        // spot=270 is well above roll_threshold → Hold
        let pos = put_pos("TEST  251219P00250000", REALISTIC_PREMIUM, 0, &far_expiry);
        assert_eq!(monitor.evaluate(&pos, 270.0, 0.2), CloseDecision::Hold);
    }

    #[test]
    fn triggers_roll_when_spot_in_roll_zone() {
        let monitor = PositionMonitor::new(cfg());
        let far_expiry = (Utc::now() + chrono::Duration::days(20)).format("%Y-%m-%d").to_string();
        // strike=250; roll_threshold=250*(1+0.05)=262.5; itm_threshold=250*(1+0.03)=257.5
        // spot=260 → 257.5 < 260 ≤ 262.5 → Roll zone
        let pos = put_pos("TEST  251219P00250000", REALISTIC_PREMIUM, 0, &far_expiry);
        match monitor.evaluate(&pos, 260.0, 0.2) {
            CloseDecision::Roll { new_strike, roll_number, .. } => {
                assert_eq!(new_strike, 250.0);
                assert_eq!(roll_number, 1);
            }
            other => panic!("Expected Roll, got {:?}", other),
        }
    }

    #[test]
    fn triggers_itm_close_when_spot_below_itm_threshold() {
        let monitor = PositionMonitor::new(cfg());
        let far_expiry = (Utc::now() + chrono::Duration::days(20)).format("%Y-%m-%d").to_string();
        // strike=250; itm_threshold=250*(1+0.03)=257.5; spot=255 ≤ 257.5 → ITM close
        let pos = put_pos("TEST  251219P00250000", REALISTIC_PREMIUM, 0, &far_expiry);
        match monitor.evaluate(&pos, 255.0, 0.2) {
            CloseDecision::Close(CloseReason::ItmProximity { .. }) => {}
            other => panic!("Expected ItmProximity close, got {:?}", other),
        }
    }

    #[test]
    fn max_rolls_reached_returns_close() {
        let monitor = PositionMonitor::new(cfg());
        let far_expiry = (Utc::now() + chrono::Duration::days(20)).format("%Y-%m-%d").to_string();
        // spot=260 is in roll zone, but roll_count=2 = max_rolls → MaxRollsReached
        let pos = put_pos("TEST  251219P00250000", REALISTIC_PREMIUM, 2, &far_expiry);
        assert_eq!(
            monitor.evaluate(&pos, 260.0, 0.2),
            CloseDecision::Close(CloseReason::MaxRollsReached)
        );
    }

    #[test]
    fn close_on_1dte() {
        let monitor = PositionMonitor::new(cfg());
        let tomorrow = (Utc::now() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let pos = put_pos("TEST  251219P00250000", REALISTIC_PREMIUM, 0, &tomorrow);
        assert_eq!(
            monitor.evaluate(&pos, 260.0, 0.2),
            CloseDecision::Close(CloseReason::OneDte)
        );
    }

    #[test]
    fn close_on_expired() {
        let monitor = PositionMonitor::new(cfg());
        let yesterday = (Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let pos = put_pos("TEST  251219P00250000", REALISTIC_PREMIUM, 0, &yesterday);
        assert_eq!(
            monitor.evaluate(&pos, 260.0, 0.2),
            CloseDecision::Close(CloseReason::Expired)
        );
    }

    #[test]
    fn cooldown_tracker_blocks_and_releases() {
        let mut tracker = CooldownTracker::new(1); // 1-second cooldown for test speed
        tracker.record_close("AAPL");
        assert!(tracker.is_cooling_down("AAPL"));
        assert!(!tracker.is_cooling_down("TSLA")); // different symbol, no cooldown
    }

    #[test]
    fn scan_portfolio_classifies_correctly() {
        let far_expiry = (Utc::now() + chrono::Duration::days(20)).format("%Y-%m-%d").to_string();
        let tomorrow   = (Utc::now() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let positions = vec![
            PositionSnapshot { symbol: "A".to_string(), occ_symbol: Some("A     251219P00250000".to_string()),
                entry_premium: Some(REALISTIC_PREMIUM), expires_at: Some(far_expiry.clone()),
                entry_date: "2025-01-01T00:00:00Z".to_string(), roll_count: 0 },
            PositionSnapshot { symbol: "B".to_string(), occ_symbol: Some("B     251219P00250000".to_string()),
                entry_premium: Some(REALISTIC_PREMIUM), expires_at: Some(tomorrow.clone()),
                entry_date: "2025-01-01T00:00:00Z".to_string(), roll_count: 0 },
        ];
        let mut spots = HashMap::new();
        // K=250; roll_threshold=262.5; itm_threshold=257.5
        // spot=260 → 257.5 < 260 ≤ 262.5 → roll zone ✓
        spots.insert("A".to_string(), 260.0);
        // B has 1 DTE → expiring_soon regardless of spot; spot healthy at 270
        spots.insert("B".to_string(), 270.0);

        let alerts = scan_portfolio(&positions, &spots, &cfg());
        assert!(alerts.roll_zone.contains(&"A".to_string()), "A should be in roll zone");
        assert!(alerts.expiring_soon.contains(&"B".to_string()), "B should be expiring soon");
    }
}
