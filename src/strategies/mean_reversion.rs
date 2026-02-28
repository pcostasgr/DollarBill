// Mean Reversion Strategy — volatility mean-reversion
//
// Rationale: implied volatility tends to mean-revert. When market IV
// deviates significantly from the model's fair-value IV, the strategy
// sells or buys straddles betting on a return to fair value.
//
// Z-score = (market_iv − model_iv) / (historical_vol × vol_of_vol_scale)
//   z > +threshold  →  IV overpriced  →  sell straddle
//   z < −threshold  →  IV underpriced →  buy straddle
//
// All signals are deterministic functions of inputs — no SystemTime, no RNG.

use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
pub struct MeanReversionStrategy {
    pub lookback_period: usize,
    /// Z-score below which the market is "oversold" (vol underpriced)
    pub oversold_threshold: f64,
    /// Z-score above which the market is "overbought" (vol overpriced)
    pub overbought_threshold: f64,
    /// Minimum market IV to trade (avoid dead markets)
    pub min_volatility: f64,
    /// Scale factor for vol-of-vol estimate (fraction of historical_vol)
    pub vol_of_vol_scale: f64,
}

impl MeanReversionStrategy {
    pub fn new() -> Self {
        Self {
            lookback_period: 20,
            oversold_threshold: -2.0,
            overbought_threshold: 2.0,
            min_volatility: 0.15,
            vol_of_vol_scale: 0.25, // assume vol-of-vol ≈ 25% of historical_vol
        }
    }

    pub fn with_config(lookback: usize, oversold: f64, overbought: f64, min_vol: f64) -> Self {
        Self {
            lookback_period: lookback,
            oversold_threshold: oversold,
            overbought_threshold: overbought,
            min_volatility: min_vol,
            vol_of_vol_scale: 0.25,
        }
    }

    /// Compute a z-score measuring how far market IV is from model fair value,
    /// scaled by an estimate of vol-of-vol.
    fn iv_z_score(&self, market_iv: f64, model_iv: f64, historical_vol: f64) -> f64 {
        let vol_of_vol = historical_vol * self.vol_of_vol_scale;
        if vol_of_vol <= 1e-9 {
            return 0.0;
        }
        (market_iv - model_iv) / vol_of_vol
    }
}

impl TradingStrategy for MeanReversionStrategy {
    fn name(&self) -> &str {
        "Mean Reversion"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        // Only trade if sufficient implied volatility
        if market_iv < self.min_volatility {
            return vec![];
        }

        let z = self.iv_z_score(market_iv, model_iv, historical_vol);

        if z <= self.oversold_threshold {
            // IV is well below model fair value → underpriced → buy straddle
            let confidence = ((z.abs() - 1.5).max(0.0) / 2.0).min(0.85);
            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::BuyStraddle,
                strike: spot,
                expiry_days: 21,
                confidence,
                edge: (model_iv - market_iv) * spot * 0.4,
                strategy_name: self.name().to_string(),
            }]
        } else if z >= self.overbought_threshold {
            // IV is well above model fair value → overpriced → sell straddle
            let confidence = ((z - 1.5).max(0.0) / 2.0).min(0.85);
            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellStraddle,
                strike: spot,
                expiry_days: 21,
                confidence,
                edge: (market_iv - model_iv) * spot * 0.4,
                strategy_name: self.name().to_string(),
            }]
        } else {
            vec![]
        }
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 75000.0,
            max_delta: 30.0,
            max_vega: 200.0,
            stop_loss_pct: 1.5,
        }
    }
}