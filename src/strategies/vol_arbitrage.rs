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
            min_edge: 0.015, // 1.5% minimum edge
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

    /// Calculate volatility risk premium
    fn calculate_vol_risk_premium(&self, symbol: &str, market_iv: f64, historical_vol: f64) -> f64 {
        // Risk premium is typically market IV - realized vol
        let base_premium = market_iv - historical_vol;
        
        // Add symbol-specific volatility regime detection
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let symbol_hash = symbol.chars().map(|c| c as u32).sum::<u32>() as f64;
        
        // Simulate regime changes
        let regime_factor = ((now as f64 * 0.001 + symbol_hash * 0.01).sin() + 1.0) * 0.5;
        let vol_regime = if regime_factor > 0.7 { 1.3 } else if regime_factor < 0.3 { 0.7 } else { 1.0 };
        
        base_premium * vol_regime
    }

    /// Determine optimal strategy based on IV/RV relationship
    fn select_vol_strategy(&self, iv_premium: f64, market_iv: f64) -> Option<SignalAction> {
        if iv_premium > self.min_edge {
            // IV is rich - sell volatility
            if market_iv > 0.4 {
                Some(SignalAction::SellStraddle) // High IV - sell straddle
            } else {
                Some(SignalAction::IronButterfly { wing_width: 0.1 }) // Moderate IV - iron butterfly
            }
        } else if iv_premium < -self.min_edge {
            // IV is cheap - buy volatility
            Some(SignalAction::BuyStraddle)
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

        if let Some(action) = self.select_vol_strategy(total_edge, market_iv) {
            // Calculate confidence based on edge magnitude and consistency
            let edge_confidence = (total_edge.abs() / 0.1).min(1.0); // Scale to 0-1
            let iv_confidence = (market_iv / 0.5).min(1.0); // Higher IV = more confidence
            let confidence = (edge_confidence * 0.7 + iv_confidence * 0.3).min(0.95);

            if confidence > 0.3 {
                // Select expiry based on volatility regime
                let expiry_days = if market_iv > 0.5 { 14 } else if market_iv > 0.3 { 21 } else { 30 };
                
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