// src/strategies/ensemble.rs
use super::{TradingStrategy, TradeSignal, RiskParams};
use std::collections::HashMap;

/// Ensemble strategy that combines multiple strategies with weights
#[allow(dead_code)] // Part of strategy API, may be used by external code
pub struct EnsembleStrategy {
    strategies: Vec<Box<dyn TradingStrategy>>,
    weights: HashMap<String, f64>,
}

impl EnsembleStrategy {
    pub fn new() -> Self {
        Self {
            strategies: vec![],
            weights: HashMap::new(),
        }
    }

    pub fn add_strategy(&mut self, strategy: Box<dyn TradingStrategy>, weight: f64) {
        let name = strategy.name().to_string();
        self.weights.insert(name, weight);
        self.strategies.push(strategy);
    }

    /// Aggregate signals from all strategies
    fn aggregate_signals(&self, signals: Vec<TradeSignal>) -> Vec<TradeSignal> {
        let mut aggregated: HashMap<String, Vec<TradeSignal>> = HashMap::new();

        // Group signals by action type
        for signal in signals {
            let key = format!("{:?}_{}", signal.action, signal.symbol);
            aggregated.entry(key).or_insert(vec![]).push(signal);
        }

        let mut final_signals = vec![];

        // For each action group, create weighted average signal
        for (_, group_signals) in aggregated {
            if group_signals.is_empty() {
                continue;
            }

            let total_weight: f64 = group_signals.iter()
                .map(|s| self.weights.get(&s.strategy_name).unwrap_or(&1.0))
                .sum();

            let weighted_confidence: f64 = group_signals.iter()
                .map(|s| s.confidence * self.weights.get(&s.strategy_name).unwrap_or(&1.0))
                .sum::<f64>() / total_weight;

            let weighted_edge: f64 = group_signals.iter()
                .map(|s| s.edge * self.weights.get(&s.strategy_name).unwrap_or(&1.0))
                .sum::<f64>() / total_weight;

            // Use the first signal as template, but update with weighted values
            let mut ensemble_signal = group_signals[0].clone();
            ensemble_signal.confidence = weighted_confidence.min(1.0);
            ensemble_signal.edge = weighted_edge;
            ensemble_signal.strategy_name = "Ensemble".to_string();

            // Only include signals with sufficient confidence
            if weighted_confidence > 0.3 {
                final_signals.push(ensemble_signal);
            }
        }

        final_signals
    }
}

impl TradingStrategy for EnsembleStrategy {
    fn name(&self) -> &str {
        "Ensemble"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        let mut all_signals = vec![];

        // Collect signals from all strategies
        for strategy in &self.strategies {
            let signals = strategy.generate_signals(symbol, spot, market_iv, model_iv, historical_vol);
            all_signals.extend(signals);
        }

        // Aggregate and weight the signals
        self.aggregate_signals(all_signals)
    }

    fn risk_params(&self) -> RiskParams {
        // Use conservative risk parameters for ensemble
        RiskParams {
            max_position_size: 30000.0, // Smaller than individual strategies
            max_delta: 20.0,
            max_vega: 120.0,
            stop_loss_pct: 1.5,
        }
    }
}