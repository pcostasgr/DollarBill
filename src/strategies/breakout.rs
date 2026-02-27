// Breakout Strategy - Capture momentum breakouts from consolidation
use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
pub struct BreakoutStrategy {
    pub consolidation_period: usize,
    pub breakout_threshold: f64,
    pub volume_threshold: f64,
    pub min_range: f64,
}

impl BreakoutStrategy {
    pub fn new() -> Self {
        Self {
            consolidation_period: 15,
            breakout_threshold: 0.03, // 3% breakout from range
            volume_threshold: 1.5, // 1.5x average volume
            min_range: 0.02, // 2% minimum trading range
        }
    }

    pub fn with_config(period: usize, threshold: f64, volume: f64, range: f64) -> Self {
        Self {
            consolidation_period: period,
            breakout_threshold: threshold,
            volume_threshold: volume,
            min_range: range,
        }
    }

    /// Detect breakout from consolidation pattern
    fn detect_breakout(&self, symbol: &str, spot: f64) -> (bool, f64, f64) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let symbol_hash = symbol.chars().map(|c| c as u32).sum::<u32>() as f64;
        
        // Simulate consolidation range
        let range_size = spot * (0.02 + (symbol_hash * 0.001).sin().abs() * 0.03);
        let _range_high = spot * (1.0 + range_size * 0.5);
        let _range_low = spot * (1.0 - range_size * 0.5);
        
        // Simulate current price relative to range
        let time_factor = (now as f64 * 0.01 + symbol_hash * 0.1).sin();
        let current_relative = 0.5 + time_factor * 0.6; // -0.1 to 1.1 range
        
        let is_breakout = current_relative > 1.0 + self.breakout_threshold || 
                         current_relative < -self.breakout_threshold;
        
        let breakout_strength = if current_relative > 1.0 {
            (current_relative - 1.0) * 10.0
        } else if current_relative < 0.0 {
            current_relative.abs() * 10.0
        } else {
            0.0
        };
        
        // Simulate volume surge
        let volume_multiplier = 1.0 + breakout_strength * 2.0;
        
        (is_breakout, breakout_strength.min(1.0), volume_multiplier)
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
        let (is_breakout, strength, volume_mult) = self.detect_breakout(symbol, spot);
        let mut signals = vec![];

        if is_breakout && volume_mult > self.volume_threshold {
            // Strong breakout with volume confirmation
            let iv_edge = market_iv - model_iv;
            let confidence = (strength * 0.6 + (volume_mult - 1.0) * 0.4).min(0.9);
            
            // Use Iron Butterfly for limited risk breakout play
            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::IronButterfly { wing_width: spot * 0.05 },
                strike: spot,
                expiry_days: 14, // Short term for breakout momentum
                confidence,
                edge: iv_edge * spot * 0.5 + historical_vol * spot * 0.2,
                strategy_name: self.name().to_string(),
            });
        } else if !is_breakout {
            // Consolidation - sell premium
            let consolidation_confidence = (1.0 - strength) * 0.5; // Lower confidence for range-bound
            
            if consolidation_confidence > 0.2 {
                signals.push(TradeSignal {
                    symbol: symbol.to_string(),
                    action: SignalAction::SellStraddle,
                    strike: spot,
                    expiry_days: 30,
                    confidence: consolidation_confidence,
                    edge: market_iv * spot * 0.3,
                    strategy_name: self.name().to_string(),
                });
            }
        }

        signals
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