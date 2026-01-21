// src/strategies/momentum.rs
use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

/// Momentum-based trading strategy
/// Buys when momentum is strong, sells when momentum weakens
#[derive(Clone)]
pub struct MomentumStrategy {
    pub momentum_period: usize,
    pub threshold: f64,
    pub min_volume: u64,
}

impl MomentumStrategy {
    pub fn new() -> Self {
        Self {
            momentum_period: 20,
            threshold: 0.05, // 5% momentum threshold
            min_volume: 100000,
        }
    }

    pub fn with_config(momentum_period: usize, threshold: f64, min_volume: u64) -> Self {
        Self {
            momentum_period,
            threshold,
            min_volume,
        }
    }

    /// Calculate momentum as percentage change over period
    fn calculate_momentum(&self, symbol: &str) -> f64 {
        // In a real implementation, this would fetch historical prices
        // For demo purposes, we'll simulate momentum calculation
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64;

        // Simulate momentum based on symbol hash and time
        let symbol_hash = symbol.chars().map(|c| c as u32).sum::<u32>() as f64;
        let momentum = ((symbol_hash * 0.001 + now * 0.0001).sin() + 1.0) * 0.1;

        momentum
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
        let momentum_score = self.calculate_momentum(symbol);

        if momentum_score > self.threshold {
            // Strong upward momentum - buy calls or straddle
            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::BuyStraddle,
                strike: spot,
                expiry_days: 30,
                confidence: momentum_score.min(1.0),
                edge: spot * market_iv * 0.3, // Rough vega estimate
                strategy_name: self.name().to_string(),
            }]
        } else if momentum_score < -self.threshold {
            // Strong downward momentum - sell straddle or buy puts
            vec![TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellStraddle,
                strike: spot,
                expiry_days: 30,
                confidence: momentum_score.abs().min(1.0),
                edge: spot * market_iv * 0.3,
                strategy_name: self.name().to_string(),
            }]
        } else {
            // No strong momentum signal
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