#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockConfig {
    pub symbol: String,
    pub market: Option<String>,
    pub sector: Option<String>,
    pub enabled: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StocksConfig {
    pub stocks: Vec<StockConfig>,
}

impl StocksConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: StocksConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn enabled_symbols(&self) -> Vec<String> {
        self.stocks
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.symbol.clone())
            .collect()
    }

    pub fn symbols_by_market(&self, market: &str) -> Vec<String> {
        self.stocks
            .iter()
            .filter(|s| s.enabled && s.market.as_ref().map_or(false, |m| m == market))
            .map(|s| s.symbol.clone())
            .collect()
    }
}