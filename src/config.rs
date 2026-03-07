use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::models::heston_analytical::IntegrationMethod;

// ═══════════════════════════════════════════════════════════════════════════
// Stock universe config  (config/stocks.json)
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// Volatility-surface / pricing config  (config/vol_surface_config.json)
// ═══════════════════════════════════════════════════════════════════════════

/// Top-level wrapper for `config/vol_surface_config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolSurfaceConfigFile {
    pub volatility_surface: VolSurfaceConfig,
}

/// Integration method tag as it appears in the JSON.
///
/// Use [`VolSurfaceConfig::integration_method()`] to convert to the
/// engine-level [`IntegrationMethod`] enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrationMethodConfig {
    #[serde(rename = "Carr_Madan")]
    CarrMadan,
    #[serde(rename = "Gauss_Laguerre")]
    GaussLaguerre,
}

impl Default for IntegrationMethodConfig {
    fn default() -> Self {
        IntegrationMethodConfig::CarrMadan
    }
}

/// Analysis sub-section of the vol-surface config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub min_strikes_around_atm: usize,
    pub max_strikes_around_atm: usize,
    pub moneyness_tolerance: f64,
}

/// Calibration sub-section of the vol-surface config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub initial_vol_guess: f64,
}

/// Core volatility-surface / Heston pricing configuration.
///
/// # JSON example
///
/// ```json
/// {
///   "volatility_surface": {
///     "risk_free_rate": 0.05,
///     "integration_method": "Gauss_Laguerre",
///     "gauss_laguerre_nodes": 64,
///     "analysis": { ... },
///     "calibration": { ... }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolSurfaceConfig {
    pub risk_free_rate: f64,

    /// `"Carr_Madan"` or `"Gauss_Laguerre"`.  Defaults to Carr-Madan.
    #[serde(default)]
    pub integration_method: IntegrationMethodConfig,

    /// Number of Gauss-Laguerre nodes (ignored when method is Carr-Madan).
    /// Valid range: 2–128.  Defaults to 32.
    #[serde(default = "default_gl_nodes")]
    pub gauss_laguerre_nodes: usize,

    pub analysis: AnalysisConfig,
    pub calibration: CalibrationConfig,
}

fn default_gl_nodes() -> usize {
    32
}

impl VolSurfaceConfig {
    /// Convert the flat config fields into the engine-level
    /// [`IntegrationMethod`] used by the Heston pricing functions.
    pub fn integration_method(&self) -> IntegrationMethod {
        match self.integration_method {
            IntegrationMethodConfig::CarrMadan => IntegrationMethod::CarrMadan,
            IntegrationMethodConfig::GaussLaguerre => IntegrationMethod::GaussLaguerre {
                nodes: self.gauss_laguerre_nodes.clamp(2, 128),
            },
        }
    }
}

impl VolSurfaceConfigFile {
    /// Load from a JSON file (typically `config/vol_surface_config.json`).
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: VolSurfaceConfigFile = serde_json::from_str(&content)?;
        Ok(config)
    }
}