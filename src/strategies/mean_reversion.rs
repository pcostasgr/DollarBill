// Mean Reversion Strategy - Buy oversold, sell overbought
use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
pub struct MeanReversionStrategy {
    pub lookback_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub min_volatility: f64,
}

impl MeanReversionStrategy {
    pub fn new() -> Self {
        Self {
            lookback_period: 20,
            oversold_threshold: -2.0, // 2 standard deviations below mean
            overbought_threshold: 2.0, // 2 standard deviations above mean
            min_volatility: 0.15, // 15% minimum IV
        }
    }

    pub fn with_config(lookback: usize, oversold: f64, overbought: f64, min_vol: f64) -> Self {
        Self {
            lookback_period: lookback,
            oversold_threshold: oversold,
            overbought_threshold: overbought,
            min_volatility: min_vol,
        }
    }

    /// Calculate Z-score for mean reversion signals
    fn calculate_z_score(&self, symbol: &str, spot: f64) -> f64 {
        // Simulate price mean and standard deviation
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let symbol_hash = symbol.chars().map(|c| c as u32).sum::<u32>() as f64;
        
        // Simulate historical mean around current spot
        let mean = spot * (1.0 + (symbol_hash * 0.0001).sin() * 0.02);
        let std_dev = spot * 0.05; // 5% standard deviation
        
        // Add some time-based variation
        let price_offset = (now as f64 * 0.001 + symbol_hash * 0.01).sin() * std_dev;
        let adjusted_spot = spot + price_offset;
        
        (adjusted_spot - mean) / std_dev
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
        _historical_vol: f64,
    ) -> Vec<TradeSignal> {
        let z_score = self.calculate_z_score(symbol, spot);
        let mut signals = vec![];

        // Only trade if sufficient volatility
        if market_iv < self.min_volatility {
            return signals;
        }

        if z_score <= self.oversold_threshold {
            // Oversold - expect mean reversion upward
            let confidence = (z_score.abs() - 1.5).max(0.0) / 2.0; // Scale 1.5-3.5 to 0-1
            
            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::BuyStraddle, // Long volatility for reversal
                strike: spot,
                expiry_days: 21, // 3 weeks for reversion
                confidence: confidence.min(0.85),
                edge: (market_iv - model_iv) * spot * 0.4,
                strategy_name: self.name().to_string(),
            });
        } else if z_score >= self.overbought_threshold {
            // Overbought - expect mean reversion downward
            let confidence = (z_score - 1.5).max(0.0) / 2.0;
            
            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellStraddle, // Short volatility
                strike: spot,
                expiry_days: 21,
                confidence: confidence.min(0.85),
                edge: (market_iv - model_iv) * spot * 0.4,
                strategy_name: self.name().to_string(),
            });
        }

        signals
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