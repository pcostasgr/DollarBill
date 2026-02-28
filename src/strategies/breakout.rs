// Breakout Strategy — IV expansion / compression detection
//
// Rationale: a large gap between implied and realised volatility signals
// that the market is pricing in a regime change ("breakout").  When IV
// expands sharply above historical vol *and* the model confirms, we buy
// an iron butterfly to capture the expected large move with limited risk.
// Conversely, when IV is compressing toward historical vol (consolidation),
// we sell straddles to harvest theta.
//
// All signals are deterministic functions of inputs — no SystemTime, no RNG.

use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
pub struct BreakoutStrategy {
    /// Informational lookback hint
    pub consolidation_period: usize,
    /// Minimum IV-expansion ratio (market_iv / historical_vol − 1) to fire a
    /// breakout signal.
    pub breakout_threshold: f64,
    /// Minimum model-IV / historical-vol ratio for confirmation
    pub confirmation_threshold: f64,
    /// Minimum market IV (absolute) to trade at all
    pub min_iv: f64,
}

impl BreakoutStrategy {
    pub fn new() -> Self {
        Self {
            consolidation_period: 15,
            breakout_threshold: 0.30, // IV must be ≥ 30% above realised vol
            confirmation_threshold: 1.10, // model IV must also be ≥ 10% above HV
            min_iv: 0.12,
        }
    }

    pub fn with_config(period: usize, threshold: f64, confirmation: f64, min_iv: f64) -> Self {
        Self {
            consolidation_period: period,
            breakout_threshold: threshold,
            confirmation_threshold: confirmation,
            min_iv,
        }
    }

    /// Measure the IV-expansion factor: how far market IV exceeds realised vol.
    ///
    /// Returns (expansion_factor, model_confirms).
    ///   expansion_factor = market_iv / historical_vol − 1   (positive = IV expanding)
    ///   model_confirms   = model_iv / historical_vol ≥ confirmation_threshold
    fn detect_iv_breakout(
        &self,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> (f64, bool) {
        if historical_vol <= 1e-9 {
            return (0.0, false);
        }
        let expansion = market_iv / historical_vol - 1.0;
        let model_ratio = model_iv / historical_vol;
        (expansion, model_ratio >= self.confirmation_threshold)
    }
}

impl TradingStrategy for BreakoutStrategy {
    fn name(&self) -> &str {
        "Breakout"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        if market_iv < self.min_iv {
            return vec![];
        }

        let (expansion, model_confirms) = self.detect_iv_breakout(market_iv, model_iv, historical_vol);

        if expansion >= self.breakout_threshold && model_confirms {
            // IV expanding sharply and model agrees → breakout regime.
            // Buy an iron butterfly: limited risk, profits from large move.
            let strength = ((expansion - self.breakout_threshold) * 5.0).min(1.0);
            let confidence = (strength * 0.7 + 0.2).min(0.9); // 20%–90%

            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::IronButterfly { wing_width: spot * 0.05 },
                strike: spot,
                expiry_days: 14,
                confidence,
                edge: (market_iv - model_iv).abs() * spot * 0.5
                    + historical_vol * spot * 0.2,
                strategy_name: self.name().to_string(),
            }]
        } else if expansion < 0.05 {
            // IV close to or below realised vol → consolidation.
            // Sell straddle to harvest theta in a range-bound market.
            let compression = (1.0 - expansion.max(0.0) / 0.05) * 0.5; // 0–0.5
            if compression > 0.2 {
                vec![TradeSignal {
                    symbol: symbol.to_string(),
                    action: SignalAction::SellStraddle,
                    strike: spot,
                    expiry_days: 30,
                    confidence: compression.min(0.85),
                    edge: market_iv * spot * 0.3,
                    strategy_name: self.name().to_string(),
                }]
            } else {
                vec![]
            }
        } else {
            // Moderate IV expansion, no clear breakout — no signal
            vec![]
        }
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 60000.0,
            max_delta: 40.0,
            max_vega: 180.0,
            stop_loss_pct: 2.5,
        }
    }
}