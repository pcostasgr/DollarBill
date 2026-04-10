// Volatility Arbitrage Strategy - Exploit IV vs realized volatility differences
use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
pub struct VolatilityArbitrageStrategy {
    pub iv_threshold: f64,
    pub lookback_days: usize,
    pub min_edge: f64,
    pub max_dte: i32,
}

impl VolatilityArbitrageStrategy {
    pub fn new() -> Self {
        Self {
            iv_threshold: 0.02, // 2% minimum IV difference
            lookback_days: 30,
            min_edge: 0.010, // 1.0% minimum edge
            max_dte: 45,
        }
    }

    pub fn with_config(iv_thresh: f64, lookback: usize, edge: f64, dte: i32) -> Self {
        Self {
            iv_threshold: iv_thresh,
            lookback_days: lookback,
            min_edge: edge,
            max_dte: dte,
        }
    }

    /// Calculate volatility risk premium.
    ///
    /// The risk premium is `market_iv − historical_vol`, scaled by a
    /// regime multiplier derived from the *level* of realized volatility.
    /// High-vol regimes tend to produce larger absolute risk premiums;
    /// low-vol regimes see premium compression.
    fn calculate_vol_risk_premium(&self, _symbol: &str, market_iv: f64, historical_vol: f64) -> f64 {
        let base_premium = market_iv - historical_vol;

        // Regime classification from realized-vol level (annualized).
        // Thresholds approximate VIX regimes: low (<20%), normal (20–40%), high (>40%).
        let vol_regime = if historical_vol > 0.40 {
            1.20 // High-vol regime: risk premium tends to be elevated
        } else if historical_vol >= 0.20 {
            1.00 // Normal regime: no adjustment
        } else {
            0.80 // Low-vol regime: risk premium is compressed
        };

        base_premium * vol_regime
    }

    /// Determine optimal strategy based on IV/RV relationship
    fn select_vol_strategy(&self, iv_premium: f64, market_iv: f64, spot: f64, expiry_days: usize) -> Option<SignalAction> {
        if iv_premium > self.min_edge {
            // IV is rich - sell volatility
            if market_iv > 0.4 {
                Some(SignalAction::SellStraddle { strike: spot, days_to_expiry: expiry_days }) // High IV - sell straddle
            } else {
                // Moderate IV - iron butterfly with 10% wings
                Some(SignalAction::IronButterfly {
                    center_strike: spot,
                    wing_width: spot * 0.10,
                    days_to_expiry: expiry_days,
                })
            }
        } else if iv_premium < -self.min_edge {
            // IV is cheap - buy volatility
            Some(SignalAction::BuyStraddle { strike: spot, days_to_expiry: expiry_days })
        } else {
            None // No clear edge
        }
    }
}

impl TradingStrategy for VolatilityArbitrageStrategy {
    fn name(&self) -> &str {
        "Vol Arbitrage"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        let vol_premium = self.calculate_vol_risk_premium(symbol, market_iv, historical_vol);
        let iv_model_edge = market_iv - model_iv;
        let total_edge = vol_premium + iv_model_edge;
        
        let mut signals = vec![];
        // Select expiry based on volatility regime (needed by select_vol_strategy for OCC symbols)
        let expiry_days = if market_iv > 0.5 { 14 } else if market_iv > 0.3 { 21 } else { 30 };

        if let Some(action) = self.select_vol_strategy(total_edge, market_iv, spot, expiry_days) {
            // Calculate confidence based on edge magnitude and consistency
            let edge_confidence = (total_edge.abs() / 0.1).min(1.0); // Scale to 0-1
            let iv_confidence = (market_iv / 0.5).min(1.0); // Higher IV = more confidence
            let confidence = (edge_confidence * 0.7 + iv_confidence * 0.3).min(0.95);

            if confidence > 0.3 {
                
                signals.push(TradeSignal {
                    symbol: symbol.to_string(),
                    action,
                    strike: spot,
                    expiry_days,
                    confidence,
                    edge: total_edge * spot,
                    strategy_name: self.name().to_string(),
                });
            }
        }

        signals
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 100000.0,
            max_delta: 20.0, // Delta-neutral strategy
            max_vega: 300.0, // High vega exposure is the point
            stop_loss_pct: 1.0, // Tight stops for arb strategies
        }
    }
}