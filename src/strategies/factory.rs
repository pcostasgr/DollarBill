// src/strategies/factory.rs
use super::{TradingStrategy, StrategyRegistry, vol_mean_reversion::VolMeanReversion, momentum::MomentumStrategy, cash_secured_puts::CashSecuredPuts};
use serde_json::Value;
use std::error::Error;
use std::fs;

/// Factory for creating trading strategies from configuration
pub struct StrategyFactory;

impl StrategyFactory {
    /// Create a strategy instance from configuration JSON
    pub fn create_from_config(config: &Value) -> Result<Box<dyn TradingStrategy>, Box<dyn Error>> {
        let strategy_type = config["type"].as_str().unwrap_or("vol_mean_reversion");

        match strategy_type {
            "vol_mean_reversion" => {
                let zscore = config["zscore_threshold"].as_f64().unwrap_or(1.5);
                let edge = config["edge_threshold"].as_f64().unwrap_or(0.05);
                Ok(Box::new(VolMeanReversion::with_config(zscore, edge)))
            },
            "momentum" => {
                let period = config["momentum_period"].as_u64().unwrap_or(20) as usize;
                let threshold = config["threshold"].as_f64().unwrap_or(0.05);
                let min_volume = config["min_volume"].as_u64().unwrap_or(100000);
                Ok(Box::new(MomentumStrategy::with_config(period, threshold, min_volume)))
            },
            "cash_secured_puts" => {
                let premium_thresh = config["premium_threshold"].as_f64().unwrap_or(0.02);
                let strike_otm = config["strike_otm_pct"].as_f64().unwrap_or(0.05);
                let iv_edge = config["min_iv_edge"].as_f64().unwrap_or(0.03);
                Ok(Box::new(CashSecuredPuts::with_config(premium_thresh, strike_otm, iv_edge)))
            },
            _ => Err(format!("Unknown strategy type: {}", strategy_type).into())
        }
    }

    /// Load a complete strategy registry from configuration file
    pub fn load_strategy_registry(config_path: &str) -> Result<StrategyRegistry, Box<dyn Error>> {
        let config_content = fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config file {}: {}", config_path, e))?;

        let config: Value = serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config JSON: {}", e))?;

        let mut registry = StrategyRegistry::new();

        if let Some(strategies) = config["strategies"].as_array() {
            for strategy_config in strategies {
                if strategy_config["enabled"].as_bool().unwrap_or(false) {
                    let strategy = Self::create_from_config(strategy_config)?;
                    registry.register(strategy);
                }
            }
        }

        Ok(registry)
    }

    /// Create a registry with default strategies for quick testing
    pub fn create_default_registry() -> StrategyRegistry {
        let mut registry = StrategyRegistry::new();
        registry.register(Box::new(VolMeanReversion::new()));
        registry.register(Box::new(MomentumStrategy::new()));
        registry.register(Box::new(CashSecuredPuts::new()));
        registry
    }
}