// src/strategies/ensemble.rs
use super::{TradingStrategy, TradeSignal, RiskParams};
use crate::analysis::advanced_classifier::MarketRegime;
use crate::analysis::regime_detector::RegimeDetector;
use std::collections::HashMap;

/// Ensemble strategy that combines multiple strategies with regime-aware weights.
///
/// When a `MarketRegime` is set (via `set_regime` or detected from
/// `historical_vol`), each contributing strategy's base weight is multiplied
/// by its regime-specific factor from `RegimeDetector::weight_for`.
/// Strategies that perform poorly in the current regime are down-weighted;
/// those well-suited to the regime are up-weighted.
pub struct EnsembleStrategy {
    strategies: Vec<Box<dyn TradingStrategy>>,
    /// Base weights assigned at `add_strategy` time (by the caller).
    weights: HashMap<String, f64>,
    /// Optional override: if set, skips the vol-scalar auto-detection in
    /// `generate_signals` and uses this regime directly.
    current_regime: Option<MarketRegime>,
}

impl EnsembleStrategy {
    /// Create an empty ensemble with no regime override (regime is auto-detected
    /// from `historical_vol` at signal-generation time).
    pub fn new() -> Self {
        Self {
            strategies: vec![],
            weights: HashMap::new(),
            current_regime: None,
        }
    }

    /// Set the active regime, overriding auto-detection.
    pub fn set_regime(&mut self, regime: MarketRegime) {
        self.current_regime = Some(regime);
    }

    /// Builder-pattern variant of `set_regime`.
    pub fn with_regime(mut self, regime: MarketRegime) -> Self {
        self.current_regime = Some(regime);
        self
    }

    /// Add a strategy with a base weight.  The effective weight at
    /// signal-aggregation time is `base_weight × regime_multiplier`.
    pub fn add_strategy(&mut self, strategy: Box<dyn TradingStrategy>, weight: f64) {
        let name = strategy.name().to_string();
        self.weights.insert(name, weight);
        self.strategies.push(strategy);
    }

    /// Effective weight for a strategy given the current regime.
    fn effective_weight(&self, strategy_name: &str, regime: &MarketRegime) -> f64 {
        let base = self.weights.get(strategy_name).copied().unwrap_or(1.0);
        let multiplier = RegimeDetector::weight_for(regime, strategy_name);
        base * multiplier
    }

    /// Aggregate signals from all strategies, applying regime-aware weighting.
    fn aggregate_signals(&self, signals: Vec<TradeSignal>, regime: &MarketRegime) -> Vec<TradeSignal> {
        let mut aggregated: HashMap<String, Vec<TradeSignal>> = HashMap::new();

        // Group signals by (action-type, symbol) key
        for signal in signals {
            let key = format!("{:?}_{}", signal.action, signal.symbol);
            aggregated.entry(key).or_default().push(signal);
        }

        let mut final_signals = vec![];

        for (_, group_signals) in aggregated {
            if group_signals.is_empty() {
                continue;
            }

            let total_weight: f64 = group_signals
                .iter()
                .map(|s| self.effective_weight(&s.strategy_name, regime))
                .sum();

            if total_weight == 0.0 {
                continue;
            }

            let weighted_confidence: f64 = group_signals
                .iter()
                .map(|s| s.confidence * self.effective_weight(&s.strategy_name, regime))
                .sum::<f64>()
                / total_weight;

            let weighted_edge: f64 = group_signals
                .iter()
                .map(|s| s.edge * self.effective_weight(&s.strategy_name, regime))
                .sum::<f64>()
                / total_weight;

            let mut ensemble_signal = group_signals[0].clone();
            ensemble_signal.confidence = weighted_confidence.min(1.0);
            ensemble_signal.edge = weighted_edge;
            ensemble_signal.strategy_name = format!("Ensemble[{:?}]", regime);

            // Gate on weighted confidence
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
        // Determine active regime: use override when set, otherwise auto-detect
        // from historical_vol.  Trend strength is unknown here, so pass 0.0
        // (conservative: only HighVol / LowVol will be detected via vol alone;
        // Trending requires explicit regime injection via set_regime).
        let regime = self
            .current_regime
            .clone()
            .unwrap_or_else(|| RegimeDetector::detect_from_scalars(historical_vol, 0.0));

        // Collect signals from all constituent strategies
        let mut all_signals = vec![];
        for strategy in &self.strategies {
            let signals =
                strategy.generate_signals(symbol, spot, market_iv, model_iv, historical_vol);
            all_signals.extend(signals);
        }

        self.aggregate_signals(all_signals, &regime)
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 30_000.0,
            max_delta: 20.0,
            max_vega: 120.0,
            stop_loss_pct: 1.5,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::{
        momentum::MomentumStrategy,
        vol_mean_reversion::VolMeanReversion,
        mean_reversion::MeanReversionStrategy,
    };

    fn build_ensemble(regime: MarketRegime) -> EnsembleStrategy {
        let mut e = EnsembleStrategy::new().with_regime(regime);
        e.add_strategy(Box::new(MomentumStrategy::new()), 1.0);
        e.add_strategy(Box::new(VolMeanReversion::new()), 1.0);
        e.add_strategy(Box::new(MeanReversionStrategy::new()), 1.0);
        e
    }

    #[test]
    fn test_ensemble_returns_signals_in_trending_regime() {
        let ensemble = build_ensemble(MarketRegime::Trending);
        let signals =
            ensemble.generate_signals("TSLA", 200.0, 0.35, 0.30, 0.20);
        // Not asserting on count — strategies may filter by market conditions —
        // but any returned signal must satisfy basic invariants.
        for s in &signals {
            assert!(!s.symbol.is_empty());
            assert!(s.confidence >= 0.0 && s.confidence <= 1.0);
            assert!(s.strategy_name.contains("Ensemble"));
        }
    }

    #[test]
    fn test_ensemble_returns_signals_in_high_vol_regime() {
        let ensemble = build_ensemble(MarketRegime::HighVol);
        let signals =
            ensemble.generate_signals("SPY", 500.0, 0.40, 0.35, 0.42);
        for s in &signals {
            assert!(s.confidence >= 0.0 && s.confidence <= 1.0);
        }
    }

    #[test]
    fn test_ensemble_strategy_name_encodes_regime() {
        let ensemble = build_ensemble(MarketRegime::LowVol);
        let signals =
            ensemble.generate_signals("AAPL", 180.0, 0.20, 0.18, 0.12);
        for s in &signals {
            assert!(
                s.strategy_name.contains("LowVol"),
                "Expected LowVol in strategy name, got {}",
                s.strategy_name
            );
        }
    }

    #[test]
    fn test_set_regime_overrides_auto_detection() {
        // historical_vol = 0.50 would normally auto-detect as HighVol
        // but we override to MeanReverting
        let mut ensemble = EnsembleStrategy::new();
        ensemble.set_regime(MarketRegime::MeanReverting);
        ensemble.add_strategy(Box::new(MomentumStrategy::new()), 1.0);

        let signals =
            ensemble.generate_signals("NVDA", 800.0, 0.45, 0.40, 0.50);
        for s in &signals {
            assert!(
                s.strategy_name.contains("MeanReverting"),
                "Set regime override not respected, got: {}",
                s.strategy_name
            );
        }
    }

    #[test]
    fn test_auto_detection_high_vol_from_historical_vol() {
        // No regime override; historical_vol = 0.50 → auto-detects HighVol
        let mut ensemble = EnsembleStrategy::new();
        ensemble.add_strategy(Box::new(VolMeanReversion::new()), 1.0);

        let signals =
            ensemble.generate_signals("TSLA", 200.0, 0.50, 0.45, 0.50);
        for s in &signals {
            assert!(
                s.strategy_name.contains("HighVol"),
                "Expected auto-detected HighVol, got: {}",
                s.strategy_name
            );
        }
    }

    #[test]
    fn test_with_regime_builder_pattern() {
        let ensemble = EnsembleStrategy::new()
            .with_regime(MarketRegime::EventDriven);
        assert_eq!(ensemble.current_regime, Some(MarketRegime::EventDriven));
    }

    #[test]
    fn test_effective_weight_applies_multiplier() {
        let mut ensemble = EnsembleStrategy::new();
        ensemble.add_strategy(Box::new(MomentumStrategy::new()), 1.0);

        // In Trending regime Momentum multiplier = 2.0 → effective = 2.0
        let w_trending =
            ensemble.effective_weight("Momentum", &MarketRegime::Trending);
        // In LowVol regime Momentum multiplier = 0.5 → effective = 0.5
        let w_low = ensemble.effective_weight("Momentum", &MarketRegime::LowVol);

        assert!(
            w_trending > w_low,
            "Trending weight ({}) should exceed LowVol weight ({})",
            w_trending,
            w_low
        );
    }
}