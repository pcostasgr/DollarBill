// src/strategies/momentum.rs
//
// Implied-volatility momentum strategy.
//
// Rationale: when market IV significantly exceeds historical (realised) vol,
// the market is pricing in a forthcoming move — an "IV momentum" signal.
// Conversely, when IV compresses well below realised vol the market is
// underpricing future movement and straddles are cheap.
//
// All signals are fully deterministic functions of (spot, market_iv,
// model_iv, historical_vol) — no SystemTime, no RNG.

use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

/// IV-momentum trading strategy.
///
/// Measures the ratio `market_iv / historical_vol` to detect momentum.
/// * Ratio > 1 + threshold  →  market expects a move  →  buy straddle
/// * Ratio < 1 − threshold  →  vol compression          →  sell straddle
#[derive(Clone)]
pub struct MomentumStrategy {
    /// Lookback period hint (informational; actual vol is supplied externally)
    pub momentum_period: usize,
    /// Minimum IV-ratio deviation from 1.0 to trigger a signal
    pub threshold: f64,
    /// Minimum absolute IV level to avoid trading in dead markets
    pub min_iv: f64,
}

impl MomentumStrategy {
    pub fn new() -> Self {
        Self {
            momentum_period: 20,
            threshold: 0.15, // 15% ratio deviation (e.g. IV/HV > 1.15)
            min_iv: 0.10,    // ignore if market IV < 10%
        }
    }

    pub fn with_config(momentum_period: usize, threshold: f64, min_iv: f64) -> Self {
        Self {
            momentum_period,
            threshold,
            min_iv,
        }
    }

    /// Compute a momentum score from the IV / realised-vol ratio.
    ///
    /// Returns a value in roughly [-1, +1].  Positive = IV expanding,
    /// negative = IV compressing.
    fn iv_momentum_score(&self, market_iv: f64, historical_vol: f64) -> f64 {
        if historical_vol <= 1e-9 {
            return 0.0;
        }
        // ratio > 1 → IV leads realised vol upward (momentum)
        // ratio < 1 → IV compressing (anti-momentum)
        let ratio = market_iv / historical_vol;
        (ratio - 1.0).clamp(-1.0, 1.0)
    }
}

impl TradingStrategy for MomentumStrategy {
    fn name(&self) -> &str {
        "Momentum"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        // Skip if market IV is too low to trade
        if market_iv < self.min_iv {
            return vec![];
        }

        let score = self.iv_momentum_score(market_iv, historical_vol);

        // Model-IV confirmation: if model agrees with market, boost confidence
        let model_agrees = if score > 0.0 {
            model_iv > historical_vol
        } else {
            model_iv < historical_vol
        };
        let confirmation_bonus = if model_agrees { 0.10 } else { 0.0 };

        if score > self.threshold {
            // IV expanding well above realised vol — buy straddle
            let confidence = (score.min(0.8) + confirmation_bonus).min(1.0);
            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::BuyStraddle,
                strike: spot,
                expiry_days: 30,
                confidence,
                edge: (market_iv - historical_vol) * spot * 0.4, // rough vega-weighted edge
                strategy_name: self.name().to_string(),
            }]
        } else if score < -self.threshold {
            // IV compressing below realised vol — sell straddle
            let confidence = (score.abs().min(0.8) + confirmation_bonus).min(1.0);
            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellStraddle,
                strike: spot,
                expiry_days: 30,
                confidence,
                edge: (historical_vol - market_iv) * spot * 0.4,
                strategy_name: self.name().to_string(),
            }]
        } else {
            vec![]
        }
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 50000.0,
            max_delta: 25.0,
            max_vega: 150.0,
            stop_loss_pct: 2.0,
        }
    }
}