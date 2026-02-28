// Advanced stock personality classification system
// Multi-dimensional analysis with regime detection and adaptive thresholds

use std::collections::HashMap;
use std::error::Error;
use serde::{Deserialize, Serialize};
use crate::market_data::csv_loader::{load_csv_closes, HistoricalDay};

/// Market regime classification for context-aware analysis
#[derive(Debug, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum MarketRegime {
    LowVol,              // VIX < 20, calm markets
    HighVol,             // VIX > 30, stressed markets  
    Trending,            // Strong directional momentum
    MeanReverting,       // Range-bound, choppy
    EventDriven,         // Earnings/news dominated
}

/// Multi-dimensional stock analysis features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedStockFeatures {
    // Volatility Analysis (percentile-based)
    pub volatility_percentile: f64,        // Vol rank vs historical (0-1)
    pub vol_regime: MarketRegime,           // Current vol environment
    pub vol_persistence: f64,              // How long vol regimes last
    pub realized_vs_implied: f64,          // RV/IV ratio
    
    // Trend & Momentum (time-weighted)
    pub trend_strength: f64,               // Directional consistency (0-1)
    pub momentum_acceleration: f64,        // Rate of change of momentum
    pub trend_persistence: f64,            // How long trends last (0-1)
    pub breakout_frequency: f64,           // Rate of range breakouts
    
    // Mean Reversion
    pub mean_reversion_speed: f64,         // How fast it reverts (0-1)
    pub mean_reversion_strength: f64,      // How much it reverts (0-1)
    pub support_resistance_strength: f64,  // Technical level strength
    
    // Cross-Asset Relationships
    pub sector_correlation: f64,           // Correlation with sector (-1 to 1)
    pub market_beta: f64,                  // Sensitivity to market
    pub beta_stability: f64,               // How stable is beta (0-1)
    
    // Sector Normalization
    pub sector: String,
    pub sector_relative_vol: f64,          // Vol relative to sector avg
    pub sector_relative_momentum: f64,     // Momentum vs sector
}

/// Enhanced strategy performance with regime awareness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedStrategyPerformance {
    pub overall_sharpe: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub regime_performance: HashMap<MarketRegime, RegimePerformance>,
    pub confidence_score: f64,             // How confident we are (0-1)
    pub sample_size: usize,               // Number of trades/periods
    pub last_updated: String,             // Timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimePerformance {
    pub sharpe: f64,
    pub return_pct: f64,
    pub max_dd: f64,
    pub trade_count: usize,
}

/// Sector-based normalization parameters
#[derive(Debug, Clone)]
pub struct SectorStats {
    pub avg_volatility: f64,
    pub avg_beta: f64,
    pub avg_momentum: f64,
    pub volatility_range: (f64, f64),      // (min, max)
    pub beta_range: (f64, f64),
    pub momentum_range: (f64, f64),
}

/// Advanced stock classifier with multi-dimensional analysis
#[allow(dead_code)]
pub struct AdvancedStockClassifier {
    features_cache: HashMap<String, AdvancedStockFeatures>,
    sector_stats: HashMap<String, SectorStats>,
    market_regime_history: Vec<(String, MarketRegime)>, // (date, regime)
    volatility_percentiles: HashMap<String, Vec<f64>>,  // Rolling percentiles
    // Performance optimization caches
    returns_cache: HashMap<String, Vec<f64>>,          // Cached return calculations
    volatility_cache: HashMap<String, Vec<f64>>,       // Cached volatility series
    last_analysis_date: HashMap<String, String>,       // Track when last analyzed
}

impl AdvancedStockClassifier {
    /// Create new advanced classifier
    pub fn new() -> Self {
        Self {
            features_cache: HashMap::new(),
            sector_stats: HashMap::new(),
            market_regime_history: Vec::new(),
            volatility_percentiles: HashMap::new(),
            returns_cache: HashMap::new(),
            volatility_cache: HashMap::new(),
            last_analysis_date: HashMap::new(),
        }
    }

    /// Analyze stock and extract advanced features - OPTIMIZED
    pub fn analyze_stock_advanced_optimized(&mut self, symbol: &str, sector: &str) -> Result<AdvancedStockFeatures, Box<dyn Error>> {
        // Load historical data
        let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let history = load_csv_closes(&filename)?;
        
        if history.len() < 252 {
            return Err(format!("Insufficient data for {}: {} days", symbol, history.len()).into());
        }

        // Check cache first
        if let Some(cached_features) = self.features_cache.get(symbol) {
            if let Some(last_date) = self.last_analysis_date.get(symbol) {
                if history.last().map(|h| h.date.clone()).unwrap_or_default() == *last_date {
                    return Ok(cached_features.clone());
                }
            }
        }

        // Calculate features with optimized methods
        let features = AdvancedStockFeatures {
            volatility_percentile: self.calculate_volatility_percentile(symbol, &history)?,
            vol_regime: self.determine_vol_regime(&history)?,
            vol_persistence: self.calculate_vol_persistence(&history)?,
            realized_vs_implied: 1.0, // Placeholder - would need options data
            
            trend_strength: self.calculate_trend_strength(&history)?,
            momentum_acceleration: self.calculate_momentum_acceleration(&history)?,
            trend_persistence: self.calculate_trend_persistence(&history)?,
            breakout_frequency: self.calculate_breakout_frequency(&history)?,
            
            mean_reversion_speed: self.calculate_reversion_speed(&history)?,
            mean_reversion_strength: self.calculate_reversion_strength(&history)?,
            support_resistance_strength: self.calculate_sr_strength(&history)?,
            
            sector_correlation: 0.7, // Placeholder - would need sector index data
            market_beta: self.calculate_beta(&history)?,
            beta_stability: self.calculate_beta_stability(&history)?,
            
            sector: sector.to_string(),
            sector_relative_vol: self.calculate_sector_relative_vol(symbol, sector, &history)?,
            sector_relative_momentum: self.calculate_sector_relative_momentum(symbol, sector, &history)?,
        };
        
        // Update cache
        self.features_cache.insert(symbol.to_string(), features.clone());
        if let Some(last_day) = history.last() {
            self.last_analysis_date.insert(symbol.to_string(), last_day.date.clone());
        }
        
        Ok(features)
    }

    /// Clear old cache entries to prevent memory bloat
    pub fn cleanup_cache(&mut self, max_entries: usize) {
        if self.features_cache.len() > max_entries {
            // Keep only the most recent entries
            let mut entries: Vec<_> = self.features_cache.keys().cloned().collect();
            entries.sort();
            
            while self.features_cache.len() > max_entries && !entries.is_empty() {
                let oldest_key = entries.remove(0);
                self.features_cache.remove(&oldest_key);
                self.returns_cache.remove(&oldest_key);
                self.volatility_cache.remove(&oldest_key);
                self.last_analysis_date.remove(&oldest_key);
            }
        }
    }

    /// Get cache statistics for performance monitoring
    pub fn get_cache_stats(&self) -> (usize, usize, usize, usize) {
        (
            self.features_cache.len(),
            self.returns_cache.len(),
            self.volatility_cache.len(),
            self.last_analysis_date.len(),
        )
    }

    /// Calculate volatility percentile using cached returns for better performance
    fn calculate_volatility_percentile(&mut self, symbol: &str, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const LOOKBACK_DAYS: usize = 252 * 2; // 2 years for percentile calc
        const CURRENT_WINDOW: usize = 30;     // Current vol calculation window

        if history.len() < LOOKBACK_DAYS {
            return Ok(0.5); // Default to median if insufficient data
        }

        // Use cached returns for performance
        let returns = self.calculate_returns_cached(symbol, history);
        let mut rolling_vols = Vec::new();
        
        for i in CURRENT_WINDOW..returns.len() {
            let window_returns = &returns[i-CURRENT_WINDOW..i];
            
            if !window_returns.is_empty() {
                let mean_return = window_returns.iter().sum::<f64>() / window_returns.len() as f64;
                let variance = window_returns.iter()
                    .map(|r| (r - mean_return).powi(2))
                    .sum::<f64>() / (window_returns.len() - 1) as f64;
                let annualized_vol = (variance * 252.0).sqrt();
                rolling_vols.push(annualized_vol);
            }
        }

        if rolling_vols.is_empty() {
            return Ok(0.5);
        }

        // Current volatility (last 30 days)
        let current_vol = rolling_vols.last().unwrap_or(&0.0);
        
        // Calculate percentile rank efficiently
        let mut sorted_vols = rolling_vols.clone();
        sorted_vols.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let percentile = match sorted_vols.binary_search_by(|v| v.partial_cmp(current_vol).unwrap()) {
            Ok(index) => index as f64 / sorted_vols.len() as f64,
            Err(index) => index as f64 / sorted_vols.len() as f64,
        };

        // Cache for future use
        self.volatility_percentiles.insert(symbol.to_string(), sorted_vols);
        
        Ok(percentile)
    }

    /// Determine current volatility regime
    fn determine_vol_regime(&self, history: &[HistoricalDay]) -> Result<MarketRegime, Box<dyn Error>> {
        const WINDOW: usize = 20; // 20-day lookback
        
        if history.len() < WINDOW {
            return Ok(MarketRegime::LowVol);
        }

        let recent = &history[history.len()-WINDOW..];
        let returns: Vec<f64> = recent.windows(2)
            .map(|pair| (pair[1].close / pair[0].close).ln())
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;
        let annualized_vol = (variance * 252.0).sqrt();

        // Classify regime based on volatility and trend
        let trend_strength = self.calculate_trend_strength_simple(&recent)?;
        
        Ok(match (annualized_vol, trend_strength) {
            (vol, _) if vol > 0.4 => MarketRegime::HighVol,
            (vol, trend) if vol < 0.15 && trend.abs() < 0.3 => MarketRegime::LowVol,
            (_, trend) if trend.abs() > 0.6 => MarketRegime::Trending,
            _ => MarketRegime::MeanReverting,
        })
    }

    /// Calculate how long volatility regimes persist
    fn calculate_vol_persistence(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        // Simplified: measure autocorrelation of volatility
        // Higher values = more persistent vol regimes
        const WINDOW: usize = 20;
        
        if history.len() < WINDOW * 3 {
            return Ok(0.5); // Default
        }

        let mut vol_changes = Vec::new();
        for i in WINDOW..history.len()-WINDOW {
            let vol1 = self.calculate_window_volatility(&history[i-WINDOW..i])?;
            let vol2 = self.calculate_window_volatility(&history[i..i+WINDOW])?;
            vol_changes.push(vol2 / vol1 - 1.0);
        }

        // Calculate autocorrelation at lag 1
        if vol_changes.len() < 2 {
            return Ok(0.5);
        }

        let mean_change = vol_changes.iter().sum::<f64>() / vol_changes.len() as f64;
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for i in 0..vol_changes.len()-1 {
            let x = vol_changes[i] - mean_change;
            let y = vol_changes[i+1] - mean_change;
            numerator += x * y;
            denominator += x * x;
        }

        let correlation = if denominator != 0.0 { numerator / denominator } else { 0.0 };
        Ok((correlation + 1.0) / 2.0) // Map to 0-1 range
    }

    /// Helper: calculate volatility for a window
    fn calculate_window_volatility(&self, window: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        if window.len() < 2 {
            return Ok(0.0);
        }

        let returns: Vec<f64> = window.windows(2)
            .map(|pair| (pair[1].close / pair[0].close).ln())
            .collect();

        if returns.is_empty() {
            return Ok(0.0);
        }

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;
        
        Ok((variance * 252.0).sqrt())
    }

    /// Calculate trend strength using linear regression
    fn calculate_trend_strength(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const WINDOW: usize = 60; // 60-day trend
        
        if history.len() < WINDOW {
            return Ok(0.0);
        }

        let recent = &history[history.len()-WINDOW..];
        let prices: Vec<f64> = recent.iter().map(|d| d.close.ln()).collect();
        
        // Simple linear regression
        let n = prices.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = prices.iter().sum::<f64>() / n;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for (i, &price) in prices.iter().enumerate() {
            let x_diff = i as f64 - x_mean;
            let y_diff = price - y_mean;
            numerator += x_diff * y_diff;
            denominator += x_diff * x_diff;
        }
        
        let slope = if denominator != 0.0 { numerator / denominator } else { 0.0 };
        
        // Convert slope to strength measure (0-1)
        let annualized_slope = slope * 252.0; // Daily to annual
        Ok((annualized_slope.tanh() + 1.0) / 2.0) // Map to 0-1
    }

    /// Helper for simple trend calculation
    fn calculate_trend_strength_simple(&self, window: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        if window.len() < 2 {
            return Ok(0.0);
        }
        
        let start_price = window.first().unwrap().close;
        let end_price = window.last().unwrap().close;
        let return_pct = (end_price / start_price - 1.0) * 100.0;
        
        Ok(return_pct / 10.0) // Normalize roughly to -1 to 1
    }

    /// Calculate momentum acceleration (rate of change of momentum)
    fn calculate_momentum_acceleration(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const LONG_WINDOW: usize = 60;
        
        if history.len() < LONG_WINDOW * 2 {
            return Ok(0.0);
        }

        // Calculate momentum at two different periods
        let recent = &history[history.len()-LONG_WINDOW..];
        let previous = &history[history.len()-LONG_WINDOW*2..history.len()-LONG_WINDOW];
        
        let momentum_recent = self.calculate_momentum_simple(recent)?;
        let momentum_previous = self.calculate_momentum_simple(previous)?;
        
        let acceleration = momentum_recent - momentum_previous;
        Ok((acceleration.tanh() + 1.0) / 2.0) // Normalize to 0-1
    }

    /// Helper: calculate simple momentum
    fn calculate_momentum_simple(&self, window: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        if window.len() < 2 {
            return Ok(0.0);
        }
        
        let start_price = window.first().unwrap().close;
        let end_price = window.last().unwrap().close;
        Ok(end_price / start_price - 1.0)
    }

    /// Calculate how long trends persist using trend direction autocorrelation
    fn calculate_trend_persistence(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const WINDOW: usize = 20; // Rolling window for trend calculation
        
        if history.len() < WINDOW * 3 {
            return Ok(0.5); // Default for insufficient data
        }

        let mut trend_directions = Vec::new();
        
        // Calculate rolling trend directions (1 for up, -1 for down, 0 for flat)
        for i in WINDOW..history.len() {
            let window_start = &history[i-WINDOW];
            let window_end = &history[i];
            let trend = (window_end.close / window_start.close - 1.0) * 100.0;
            
            let direction = if trend > 1.0 { 1.0 } else if trend < -1.0 { -1.0 } else { 0.0 };
            trend_directions.push(direction);
        }

        if trend_directions.len() < 2 {
            return Ok(0.5);
        }

        // Calculate autocorrelation at lag 1 to measure trend persistence
        let mean_direction = trend_directions.iter().sum::<f64>() / trend_directions.len() as f64;
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for i in 0..trend_directions.len()-1 {
            let x = trend_directions[i] - mean_direction;
            let y = trend_directions[i+1] - mean_direction;
            numerator += x * y;
            denominator += x * x;
        }

        let correlation = if denominator != 0.0 { numerator / denominator } else { 0.0 };
        
        // Map correlation to 0-1 range (higher = more persistent trends)
        Ok((correlation + 1.0) / 2.0)
    }
    /// Calculate breakout frequency based on volatility spikes above normal ranges
    fn calculate_breakout_frequency(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const LOOKBACK_DAYS: usize = 252; // 1 year lookback
        const VOLATILITY_WINDOW: usize = 20; // Rolling vol calculation
        
        if history.len() < LOOKBACK_DAYS {
            return Ok(0.3); // Default
        }

        let recent_history = &history[history.len()-LOOKBACK_DAYS..];
        let mut rolling_vols = Vec::new();
        
        // Calculate rolling volatilities
        for i in VOLATILITY_WINDOW..recent_history.len() {
            let window = &recent_history[i-VOLATILITY_WINDOW..i];
            let returns: Vec<f64> = window.windows(2)
                .map(|pair| (pair[1].close / pair[0].close).ln())
                .collect();
            
            if !returns.is_empty() {
                let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance = returns.iter()
                    .map(|r| (r - mean_return).powi(2))
                    .sum::<f64>() / (returns.len() - 1) as f64;
                let vol = variance.sqrt();
                rolling_vols.push(vol);
            }
        }

        if rolling_vols.is_empty() {
            return Ok(0.3);
        }

        // Calculate mean and standard deviation of volatility
        let mean_vol = rolling_vols.iter().sum::<f64>() / rolling_vols.len() as f64;
        let vol_variance = rolling_vols.iter()
            .map(|v| (v - mean_vol).powi(2))
            .sum::<f64>() / (rolling_vols.len() - 1) as f64;
        let vol_std = vol_variance.sqrt();
        
        // Count breakouts (vol spikes > 2 standard deviations above mean)
        let breakout_threshold = mean_vol + 2.0 * vol_std;
        let breakout_count = rolling_vols.iter()
            .filter(|&&vol| vol > breakout_threshold)
            .count();
        
        // Calculate frequency as percentage
        let frequency = breakout_count as f64 / rolling_vols.len() as f64;
        
        // Cap at reasonable maximum (20% breakout frequency is very high)
        Ok(frequency.min(0.2) * 5.0) // Scale to 0-1 range
    }
    /// Calculate mean reversion speed using half-life of deviations from moving average
    fn calculate_reversion_speed(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const MA_PERIOD: usize = 50; // Moving average period
        const LOOKBACK_DAYS: usize = 252; // 1 year analysis
        
        if history.len() < MA_PERIOD + LOOKBACK_DAYS {
            return Ok(0.4); // Default
        }

        let recent_history = &history[history.len()-LOOKBACK_DAYS..];
        let mut deviations = Vec::new();
        
        // Calculate deviations from moving average
        for i in MA_PERIOD..recent_history.len() {
            let ma_window = &recent_history[i-MA_PERIOD..i];
            let moving_average = ma_window.iter()
                .map(|d| d.close)
                .sum::<f64>() / MA_PERIOD as f64;
            
            let current_price = recent_history[i].close;
            let deviation = (current_price - moving_average) / moving_average;
            deviations.push(deviation.abs()); // Use absolute deviation
        }

        if deviations.len() < 10 {
            return Ok(0.4);
        }

        // Calculate autocorrelation of absolute deviations to measure persistence
        let mean_deviation = deviations.iter().sum::<f64>() / deviations.len() as f64;
        let mut autocorr_sum = 0.0;
        let mut variance_sum = 0.0;
        let valid_pairs = deviations.len() - 1;

        for i in 0..valid_pairs {
            let x = deviations[i] - mean_deviation;
            let y = deviations[i+1] - mean_deviation;
            autocorr_sum += x * y;
            variance_sum += x * x;
        }

        let autocorr = if variance_sum != 0.0 { 
            autocorr_sum / variance_sum 
        } else { 
            0.0 
        };
        
        // Convert autocorrelation to reversion speed
        // Higher autocorrelation = slower reversion, lower autocorr = faster reversion
        let reversion_speed = 1.0 - autocorr.abs().min(1.0);
        
        Ok(reversion_speed.max(0.0).min(1.0))
    }
    /// Calculate mean reversion strength using deviation recovery analysis
    fn calculate_reversion_strength(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const MA_PERIOD: usize = 50;
        const LOOKBACK_DAYS: usize = 252;
        const RECOVERY_WINDOW: usize = 10; // Days to measure recovery
        
        if history.len() < MA_PERIOD + LOOKBACK_DAYS + RECOVERY_WINDOW {
            return Ok(0.5); // Default
        }

        let recent_history = &history[history.len()-(LOOKBACK_DAYS + RECOVERY_WINDOW)..];
        let mut reversion_events = Vec::new();
        
        for i in MA_PERIOD..recent_history.len()-RECOVERY_WINDOW {
            // Calculate moving average
            let ma_window = &recent_history[i-MA_PERIOD..i];
            let moving_average = ma_window.iter()
                .map(|d| d.close)
                .sum::<f64>() / MA_PERIOD as f64;
            
            let current_price = recent_history[i].close;
            let deviation_pct = (current_price - moving_average) / moving_average;
            
            // Look for significant deviations (>2%)
            if deviation_pct.abs() > 0.02 {
                // Measure how much it reverts in the next RECOVERY_WINDOW days
                let future_window = &recent_history[i+1..i+1+RECOVERY_WINDOW];
                if let Some(future_price) = future_window.last() {
                    let future_deviation = (future_price.close - moving_average) / moving_average;
                    
                    // Calculate reversion strength (how much of deviation was recovered)
                    let reversion = if deviation_pct.abs() > 0.0 {
                        1.0 - (future_deviation.abs() / deviation_pct.abs())
                    } else {
                        0.0
                    };
                    
                    reversion_events.push(reversion.max(0.0).min(1.0));
                }
            }
        }

        if reversion_events.is_empty() {
            return Ok(0.5);
        }

        // Average reversion strength across all events
        let avg_reversion = reversion_events.iter().sum::<f64>() / reversion_events.len() as f64;
        
        Ok(avg_reversion)
    }
    fn calculate_sr_strength(&self, _history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> { Ok(0.6) }
    /// Calculate beta using simplified market sensitivity (assuming SPY-like behavior)
    fn calculate_beta(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const LOOKBACK_DAYS: usize = 252; // 1 year for beta calculation
        
        if history.len() < LOOKBACK_DAYS {
            return Ok(1.0); // Default market beta
        }

        let recent_history = &history[history.len()-LOOKBACK_DAYS..];
        let returns: Vec<f64> = recent_history.windows(2)
            .map(|pair| (pair[1].close / pair[0].close).ln())
            .collect();
        
        if returns.len() < 50 {
            return Ok(1.0);
        }

        // Calculate stock volatility as proxy for beta sensitivity
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;
        let volatility = (variance * 252.0).sqrt(); // Annualized
        
        // Estimate beta based on volatility relative to market (assume market vol ~15-20%)
        let market_vol = 0.18; // Typical S&P 500 volatility
        let estimated_beta = volatility / market_vol;
        
        // Cap beta at reasonable range
        Ok(estimated_beta.max(0.1).min(3.0))
    }
    /// Calculate beta stability using rolling beta calculations
    fn calculate_beta_stability(&self, history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> {
        const BETA_WINDOW: usize = 60; // 60-day rolling beta
        const MIN_PERIODS: usize = 5; // Minimum periods for stability calc
        
        if history.len() < BETA_WINDOW * MIN_PERIODS {
            return Ok(0.7); // Default stability
        }

        let mut rolling_betas = Vec::new();
        
        // Calculate rolling betas
        for i in BETA_WINDOW..history.len() {
            let window = &history[i-BETA_WINDOW..i];
            let returns: Vec<f64> = window.windows(2)
                .map(|pair| (pair[1].close / pair[0].close).ln())
                .collect();
            
            if returns.len() > 10 {
                let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance = returns.iter()
                    .map(|r| (r - mean_return).powi(2))
                    .sum::<f64>() / (returns.len() - 1) as f64;
                let volatility = (variance * 252.0).sqrt();
                
                let market_vol = 0.18;
                let beta = volatility / market_vol;
                rolling_betas.push(beta);
            }
        }

        if rolling_betas.len() < MIN_PERIODS {
            return Ok(0.7);
        }

        // Calculate coefficient of variation (stability measure)
        let mean_beta = rolling_betas.iter().sum::<f64>() / rolling_betas.len() as f64;
        let beta_variance = rolling_betas.iter()
            .map(|b| (b - mean_beta).powi(2))
            .sum::<f64>() / (rolling_betas.len() - 1) as f64;
        let beta_std = beta_variance.sqrt();
        
        let coefficient_of_variation = if mean_beta != 0.0 {
            beta_std / mean_beta.abs()
        } else {
            1.0
        };
        
        // Convert to stability score (lower CV = higher stability)
        let stability = 1.0 / (1.0 + coefficient_of_variation);
        
        Ok(stability.max(0.0).min(1.0))
    }
    fn calculate_sector_relative_vol(&self, _symbol: &str, _sector: &str, _history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> { Ok(1.0) }
    fn calculate_sector_relative_momentum(&self, _symbol: &str, _sector: &str, _history: &[HistoricalDay]) -> Result<f64, Box<dyn Error>> { Ok(1.0) }

    /// Optimized return calculation with caching
    fn calculate_returns_cached(&mut self, symbol: &str, history: &[HistoricalDay]) -> Vec<f64> {
        // Check if we have cached returns
        if let Some(cached_returns) = self.returns_cache.get(symbol) {
            if cached_returns.len() == history.len() - 1 {
                return cached_returns.clone();
            }
        }

        // Calculate fresh returns
        let returns: Vec<f64> = history.windows(2)
            .map(|pair| (pair[1].close / pair[0].close).ln())
            .collect();
        
        // Cache the results
        self.returns_cache.insert(symbol.to_string(), returns.clone());
        
        returns
    }

    pub fn classify_personality_advanced(&self, features: &AdvancedStockFeatures) -> (crate::analysis::stock_classifier::StockPersonality, f64) {
        use crate::analysis::stock_classifier::StockPersonality;
        
        // Fixed-order array — deterministic tie-breaking (last max wins).
        let mut scores: [(StockPersonality, f64); 5] = [
            (StockPersonality::MomentumLeader,   0.0),
            (StockPersonality::MeanReverting,    0.0),
            (StockPersonality::TrendFollower,    0.0),
            (StockPersonality::VolatileBreaker,  0.0),
            (StockPersonality::StableAccumulator, 0.0),
        ];
        
        // Momentum Leader scoring (high momentum + trend + vol)
        if features.momentum_acceleration > 0.6 {
            scores[0].1 += 3.0;
        }
        if features.trend_persistence > 0.7 {
            scores[0].1 += 2.5;
        }
        if features.volatility_percentile > 0.75 {
            scores[0].1 += 2.0;
        }
        if features.breakout_frequency > 0.6 {
            scores[0].1 += 1.5;
        }
        
        // Mean Reverting scoring (high reversion + low trend persistence)
        if features.mean_reversion_speed > 0.7 {
            scores[1].1 += 3.0;
        }
        if features.mean_reversion_strength > 0.6 {
            scores[1].1 += 2.5;
        }
        if features.support_resistance_strength > 0.6 {
            scores[1].1 += 2.0;
        }
        if features.trend_persistence < 0.4 {
            scores[1].1 += 1.5;
        }
        
        // Trend Follower scoring (sustained trends + medium vol)
        if features.trend_strength > 0.7 && features.trend_persistence > 0.6 {
            scores[2].1 += 3.0;
        }
        if features.volatility_percentile > 0.3 && features.volatility_percentile < 0.8 {
            scores[2].1 += 2.0;
        }
        if features.beta_stability > 0.7 {
            scores[2].1 += 1.5;
        }
        
        // Volatile Breaker scoring (extreme vol + breakouts)
        if features.volatility_percentile > 0.9 {
            scores[3].1 += 3.0;
        }
        if features.breakout_frequency > 0.7 {
            scores[3].1 += 2.5;
        }
        if features.beta_stability < 0.4 {
            scores[3].1 += 2.0;
        }
        
        // Stable Accumulator scoring (low vol + steady trends)
        if features.volatility_percentile < 0.4 {
            scores[4].1 += 3.0;
        }
        if features.trend_strength > 0.4 && features.volatility_percentile < 0.6 {
            scores[4].1 += 2.5;
        }
        if features.beta_stability > 0.8 {
            scores[4].1 += 2.0;
        }
        if features.support_resistance_strength > 0.5 {
            scores[4].1 += 1.5;
        }
        
        // Find best match — deterministic: scans left-to-right, last equal wins.
        let max_entry = scores.into_iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((StockPersonality::StableAccumulator, 0.0));
        
        let confidence = (max_entry.1 / 10.0_f64).min(1.0_f64); // Max possible score ~10
        
        (max_entry.0, confidence)
    }
}