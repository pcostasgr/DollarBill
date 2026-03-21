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

// ═════════════════════════════════════════════════════════════════════════
// Live-trading bot runtime config  (config/trading_bot_config.json)
// ═════════════════════════════════════════════════════════════════════════

/// Live-trading parameters loaded from the `"bot_runtime"` key in
/// `config/trading_bot_config.json`.  Every field has a sensible default
/// so the bot starts even if the JSON file is absent or incomplete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotRuntimeConfig {
    /// Minimum signal confidence (0–1) required before placing an order.
    #[serde(default = "BotRuntimeConfig::default_min_confidence")]
    pub min_confidence: f64,
    /// Halt new orders once estimated daily spend exceeds this fraction of equity (0–1).
    #[serde(default = "BotRuntimeConfig::default_max_daily_loss_pct")]
    pub max_daily_loss_pct: f64,
    /// Minimum seconds between signals on the same symbol.
    #[serde(default = "BotRuntimeConfig::default_signal_cooldown_secs")]
    pub signal_cooldown_secs: u64,
    /// Rolling price-buffer size (number of ticks to keep per symbol).
    #[serde(default = "BotRuntimeConfig::default_max_price_buf")]
    pub max_price_buf: usize,
    /// Minimum ticks in buffer before HV-21 can be computed (need 22 log-returns).
    #[serde(default = "BotRuntimeConfig::default_min_prices_for_hv")]
    pub min_prices_for_hv: usize,
    /// Close short position when the repriced option value falls to this fraction
    /// of the collected premium (e.g. 0.50 = take profit at 50% of max profit).
    #[serde(default = "BotRuntimeConfig::default_profit_target_pct")]
    pub profit_target_pct: f64,
    /// Close short position when the repriced option value reaches this multiple
    /// of the collected premium (e.g. 2.0 = stop loss at 200% of premium).
    #[serde(default = "BotRuntimeConfig::default_stop_loss_pct")]
    pub stop_loss_pct: f64,
    /// Force-close any open position after this many calendar days regardless of P&L.
    #[serde(default = "BotRuntimeConfig::default_max_position_days")]
    pub max_position_days: i64,
    /// Minimum HV percentile vs 252-day history required to enter a short-vol
    /// position (0–1). Signals skipped when realized vol is below this floor.
    #[serde(default = "BotRuntimeConfig::default_min_vol_percentile")]
    pub min_vol_percentile: f64,
}

impl BotRuntimeConfig {
    fn default_min_confidence()       -> f64  { 0.60 }
    fn default_max_daily_loss_pct()   -> f64  { 0.05 }
    fn default_signal_cooldown_secs() -> u64  { 300  }
    fn default_max_price_buf()        -> usize { 50  }
    fn default_min_prices_for_hv()    -> usize { 22  }
    fn default_profit_target_pct()    -> f64  { 0.50 }
    fn default_stop_loss_pct()        -> f64  { 2.00 }
    fn default_max_position_days()    -> i64  { 21   }
    fn default_min_vol_percentile()   -> f64  { 0.40 }
}

impl Default for BotRuntimeConfig {
    fn default() -> Self {
        Self {
            min_confidence:       Self::default_min_confidence(),
            max_daily_loss_pct:   Self::default_max_daily_loss_pct(),
            signal_cooldown_secs: Self::default_signal_cooldown_secs(),
            max_price_buf:        Self::default_max_price_buf(),
            min_prices_for_hv:    Self::default_min_prices_for_hv(),
            profit_target_pct:    Self::default_profit_target_pct(),
            stop_loss_pct:        Self::default_stop_loss_pct(),
            max_position_days:    Self::default_max_position_days(),
            min_vol_percentile:   Self::default_min_vol_percentile(),
        }
    }
}

/// Top-level wrapper matching `config/trading_bot_config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingBotConfigFile {
    /// Runtime parameters used by the live-trading bot.
    #[serde(default)]
    pub bot_runtime: BotRuntimeConfig,
}

impl TradingBotConfigFile {
    /// Load runtime config from `config/trading_bot_config.json`.
    ///
    /// Falls back to [`BotRuntimeConfig::default`] on any file or parse error
    /// so the bot always starts with safe defaults.
    pub fn load() -> BotRuntimeConfig {
        let path = "config/trading_bot_config.json";
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str::<TradingBotConfigFile>(&content)
                .map(|f| f.bot_runtime)
                .unwrap_or_else(|e| {
                    log::warn!("trading_bot_config.json parse error: {} -- using defaults", e);
                    BotRuntimeConfig::default()
                }),
            Err(_) => {
                log::warn!("config/trading_bot_config.json not found -- using defaults");
                BotRuntimeConfig::default()
            }
        }
    }
}