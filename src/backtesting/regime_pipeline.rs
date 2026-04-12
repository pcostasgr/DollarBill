//! Pre-trade decision gate: wires `PortfolioGreeks`, `RegimeDetector`, and
//! `PositionSizer` into a single checkpoint that must be passed before any new
//! position is opened or rolled.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dollarbill::backtesting::regime_pipeline::{RegimePipeline, RegimePipelineConfig};
//! use dollarbill::portfolio::{PositionSizer, SizingMethod};
//! use dollarbill::analysis::portfolio_greeks::PortfolioLimits;
//!
//! let mut pipeline = RegimePipeline::new(
//!     PositionSizer::new(100_000.0, 2.0, 10.0),
//!     PortfolioLimits::default(),
//! );
//!
//! // Inside the trading loop, before opening/rolling a position:
//! let decision = pipeline.pre_trade_check(
//!     date, spot, rate,
//!     &recent_closes,
//!     &current_book,
//!     base_option_price, volatility,
//!     SizingMethod::VolatilityBased,
//!     equity,
//! );
//!
//! if decision.should_flatten {
//!     trigger_auto_hedge_or_flatten();  // close / hedge the book
//! }
//! let size = decision.contracts;        // use this for the new order
//! ```

use crate::analysis::advanced_classifier::MarketRegime;
use crate::analysis::portfolio_greeks::{
    check_limits, compute_book_greeks, OptionLeg, PortfolioGreeks, PortfolioLimits,
};
use crate::analysis::regime_detector::RegimeDetector;
use crate::backtesting::audit_log::{AuditLog, RegimeSizingAuditEntry};
use crate::portfolio::{PositionSizer, SizingMethod};

// ─── Output ──────────────────────────────────────────────────────────────────

/// Output of one pre-trade pipeline run.
#[derive(Debug)]
pub struct PreTradeDecision {
    /// Regime-adjusted contract count (0 when `should_flatten`).
    pub contracts: i32,
    /// Detected market regime.
    pub regime: MarketRegime,
    /// `RegimeDetector::sizing_multiplier(regime)`.
    pub multiplier: f64,
    /// Portfolio Greeks of the current open book at today's spot.
    pub greeks: PortfolioGreeks,
    /// `true` when a limit breach requires auto-flatten / hedging.
    pub should_flatten: bool,
}

// ─── Pipeline ────────────────────────────────────────────────────────────────

/// Pre-trade decision gate combining PortfolioGreeks, RegimeDetector, and
/// PositionSizer.
///
/// Call [`Self::pre_trade_check`] once per day (or per signal) before
/// submitting any order.  The result tells you:
/// * whether to flatten the book first,
/// * how many contracts to trade,
/// * and the current regime + Greeks for downstream use.
///
/// Every call appends one entry to [`Self::audit_log`].
pub struct RegimePipeline {
    sizer:  PositionSizer,
    limits: PortfolioLimits,
    /// Append-only daily decision log — inspect after the backtest run.
    pub audit_log: AuditLog,
}

impl RegimePipeline {
    /// Create a new pipeline with the given sizer and risk limits.
    pub fn new(sizer: PositionSizer, limits: PortfolioLimits) -> Self {
        Self {
            sizer,
            limits,
            audit_log: AuditLog::new(),
        }
    }

    /// Run the full pre-trade pipeline for one trading day / signal.
    ///
    /// # Arguments
    /// | Parameter           | Description |
    /// |---------------------|-------------|
    /// | `date`              | "YYYY-MM-DD" date string for the audit log. |
    /// | `spot`              | Underlying spot price. |
    /// | `rate`              | Annualised risk-free rate. |
    /// | `recent_closes`     | Rolling window of daily closes for regime detection (≥ 20 recommended). |
    /// | `current_book`      | Option legs currently open in the portfolio. |
    /// | `base_option_price` | Mid-price of the target option for sizing. |
    /// | `base_volatility`   | Annualised vol for sizing. |
    /// | `sizing_method`     | Which sizing algorithm to apply. |
    /// | `equity`            | Current portfolio equity (dollars). |
    /// | `current_dd_frac`   | Current drawdown from equity peak as a fraction (0.0 = at peak). Used for P&L-aware flatten trigger. |
    ///
    /// # Returns
    /// A [`PreTradeDecision`] — also appended to `self.audit_log`.
    pub fn pre_trade_check(
        &mut self,
        date:              &str,
        spot:              f64,
        rate:              f64,
        recent_closes:     &[f64],
        current_book:      &[OptionLeg],
        base_option_price: f64,
        base_volatility:   f64,
        sizing_method:     SizingMethod,
        equity:            f64,
        // Current drawdown from equity peak as a fraction; used for P&L-aware flatten.
        current_dd_frac:   f64,
    ) -> PreTradeDecision {
        // ── 1. Portfolio Greeks of the current open book ──────────────────────
        let greeks = compute_book_greeks(spot, rate, current_book);

        // ── 2. Regime detection ───────────────────────────────────────────────
        let regime = if recent_closes.len() >= 10 {
            RegimeDetector::detect(recent_closes)
        } else {
            // Not enough history — conservative neutral default
            MarketRegime::MeanReverting
        };
        let multiplier = RegimeDetector::sizing_multiplier(&regime);

        // ── 3. Regime-aware position sizing ───────────────────────────────────
        let contracts = self.sizer.calculate_size_with_regime(
            sizing_method,
            base_option_price,
            base_volatility,
            None,
            None,
            None,
            &regime,
        );

        // ── 4. Limit check + P&L-aware trigger ─────────────────────────────────
        let breaches = check_limits(&greeks, &self.limits, equity);
        // Hybrid P&L-aware trigger (two paths):
        //   Path A – slow bleed:  DD > 6% from peak  AND  vega_util > 45%
        //            (any open condor carries ~46% util, so this fires on any
        //             position that has started bleeding)
        //   Path B – hard backstop: DD > 10% regardless of vega level
        let vega_util        = greeks.net_vega.abs() / self.limits.max_vega.max(1.0);
        let pnl_vega_trigger = (current_dd_frac > 0.06 && vega_util > 0.45)
                             || current_dd_frac > 0.10;
        let should_flatten   = !breaches.is_empty() || pnl_vega_trigger;

        // ── 5. Heuristic projected max-DD: |vega| × 1-vol-pt / equity ─────────
        let projected_max_dd_pct = if equity > 0.0 {
            (greeks.net_vega.abs() * 0.01 / equity) * 100.0
        } else {
            0.0
        };

        // ── 6. Audit entry ────────────────────────────────────────────────────
        let entry = RegimeSizingAuditEntry {
            date:               date.to_string(),
            regime:             regime_label(&regime),
            multiplier,
            portfolio_vega:     greeks.net_vega,
            portfolio_delta:    greeks.net_delta,
            net_contracts:      contracts,
            equity,
            auto_derisk:        should_flatten,
            projected_max_dd_pct,
        };
        self.audit_log.record(entry);

        PreTradeDecision {
            contracts,
            regime,
            multiplier,
            greeks,
            should_flatten,
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn regime_label(r: &MarketRegime) -> String {
    match r {
        MarketRegime::HighVol       => "HighVol".to_string(),
        MarketRegime::LowVol        => "LowVol".to_string(),
        MarketRegime::Trending      => "Trending".to_string(),
        MarketRegime::MeanReverting => "MeanReverting".to_string(),
        MarketRegime::EventDriven   => "EventDriven".to_string(),
    }
}
