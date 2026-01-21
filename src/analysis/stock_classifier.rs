// Stock personality classification system
// Analyzes historical performance to match optimal strategies to stocks

use std::collections::HashMap;
use std::error::Error;
use serde::{Deserialize, Serialize};

/// Stock personality types based on historical behavior patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StockPersonality {
    /// High momentum, trend-following stocks (NVDA, PLTR)
    /// Best: Short-term momentum strategies
    /// Worst: Long-term holding, mean reversion
    MomentumLeader,

    /// Volatile stocks that tend to revert to mean (TSLA, COIN)
    /// Best: Volatility mean reversion, iron butterflies
    /// Worst: Momentum strategies, trend following
    MeanReverting,

    /// Stable stocks that follow broader market trends (MSFT, GOOGL)
    /// Best: Medium-term RSI + momentum, covered calls
    /// Worst: Short-term scalping, high-frequency
    TrendFollower,

    /// Extremely volatile stocks prone to breakouts (high vol tech)
    /// Best: Iron butterflies, volatility harvesting
    /// Worst: Directional bets, long options
    VolatileBreaker,

    /// Low volatility, stable accumulation stocks
    /// Best: Cash-secured puts, covered calls
    /// Worst: Calls, speculative strategies
    StableAccumulator,
}

/// Performance metrics for strategy-stock combinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub total_return: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: usize,
}

/// Comprehensive stock profile with personality and performance data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockProfile {
    pub symbol: String,
    pub personality: StockPersonality,
    pub avg_volatility: f64,
    pub trend_strength: f64,
    pub mean_reversion_tendency: f64,
    pub momentum_sensitivity: f64,
    pub best_strategies: Vec<String>,
    pub worst_strategies: Vec<String>,
    pub strategy_performance: HashMap<String, StrategyPerformance>,
}

/// Main stock classification engine
pub struct StockClassifier {
    profiles: HashMap<String, StockProfile>,
}

impl StockClassifier {
    /// Create new classifier with empty profiles
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    /// Classify a stock based on historical metrics
    pub fn classify_stock(
        &mut self,
        symbol: &str,
        avg_volatility: f64,
        trend_strength: f64,
        mean_reversion_tendency: f64,
        momentum_sensitivity: f64,
    ) -> StockProfile {
        let personality = self.determine_personality(
            avg_volatility,
            trend_strength,
            mean_reversion_tendency,
            momentum_sensitivity,
        );

        let (best_strategies, worst_strategies) = self.get_strategy_recommendations(&personality);

        let profile = StockProfile {
            symbol: symbol.to_string(),
            personality,
            avg_volatility,
            trend_strength,
            mean_reversion_tendency,
            momentum_sensitivity,
            best_strategies,
            worst_strategies,
            strategy_performance: HashMap::new(),
        };

        self.profiles.insert(symbol.to_string(), profile.clone());
        profile
    }

    /// Determine stock personality based on metrics
    fn determine_personality(
        &self,
        volatility: f64,
        trend_strength: f64,
        reversion_tendency: f64,
        momentum_sensitivity: f64,
    ) -> StockPersonality {
        // High volatility threshold (>50%)
        if volatility > 0.5 {
            if momentum_sensitivity > 0.7 {
                StockPersonality::MomentumLeader
            } else if reversion_tendency > 0.6 {
                StockPersonality::MeanReverting
            } else {
                StockPersonality::VolatileBreaker
            }
        }
        // Medium volatility (25-50%)
        else if volatility > 0.25 {
            if trend_strength > 0.6 {
                StockPersonality::TrendFollower
            } else if reversion_tendency > 0.5 {
                StockPersonality::MeanReverting
            } else {
                StockPersonality::MomentumLeader
            }
        }
        // Low volatility (<25%)
        else {
            if reversion_tendency > 0.4 {
                StockPersonality::StableAccumulator
            } else {
                StockPersonality::TrendFollower
            }
        }
    }

    /// Get strategy recommendations for a personality type
    fn get_strategy_recommendations(&self, personality: &StockPersonality) -> (Vec<String>, Vec<String>) {
        match personality {
            StockPersonality::MomentumLeader => (
                vec![
                    "Short-Term Momentum".to_string(),
                    "Breakout Trading".to_string(),
                    "Trend Following".to_string(),
                ],
                vec![
                    "Long-Term Holding".to_string(),
                    "Mean Reversion".to_string(),
                    "Cash-Secured Puts".to_string(),
                ],
            ),
            StockPersonality::MeanReverting => (
                vec![
                    "Volatility Mean Reversion".to_string(),
                    "Iron Butterfly".to_string(),
                    "Iron Condor".to_string(),
                ],
                vec![
                    "Momentum Trading".to_string(),
                    "Trend Following".to_string(),
                    "Breakout Trading".to_string(),
                ],
            ),
            StockPersonality::TrendFollower => (
                vec![
                    "Medium-Term RSI".to_string(),
                    "Moving Average Crossover".to_string(),
                    "Covered Calls".to_string(),
                ],
                vec![
                    "Short-Term Scalping".to_string(),
                    "High-Frequency Trading".to_string(),
                    "Iron Butterfly".to_string(),
                ],
            ),
            StockPersonality::VolatileBreaker => (
                vec![
                    "Iron Butterfly".to_string(),
                    "Volatility Harvesting".to_string(),
                    "Calendar Spreads".to_string(),
                ],
                vec![
                    "Directional Calls".to_string(),
                    "Directional Puts".to_string(),
                    "Long Options".to_string(),
                ],
            ),
            StockPersonality::StableAccumulator => (
                vec![
                    "Cash-Secured Puts".to_string(),
                    "Covered Calls".to_string(),
                    "Collar Strategy".to_string(),
                ],
                vec![
                    "Long Calls".to_string(),
                    "Speculative Strategies".to_string(),
                    "Iron Butterfly".to_string(),
                ],
            ),
        }
    }

    /// Get optimal strategy for a stock
    pub fn get_optimal_strategy(&self, symbol: &str) -> Option<String> {
        self.profiles.get(symbol)
            .and_then(|profile| profile.best_strategies.first())
            .cloned()
    }

    /// Get all profiles
    pub fn get_all_profiles(&self) -> &HashMap<String, StockProfile> {
        &self.profiles
    }

    /// Get profile for specific stock
    pub fn get_profile(&self, symbol: &str) -> Option<&StockProfile> {
        self.profiles.get(symbol)
    }

    /// Load profiles from JSON file
    pub fn load_from_file(filepath: &str) -> Result<Self, Box<dyn Error>> {
        let content = std::fs::read_to_string(filepath)?;
        let profiles: HashMap<String, StockProfile> = serde_json::from_str(&content)?;

        Ok(Self { profiles })
    }

    /// Save profiles to JSON file
    pub fn save_to_file(&self, filepath: &str) -> Result<(), Box<dyn Error>> {
        let json = serde_json::to_string_pretty(&self.profiles)?;
        std::fs::write(filepath, json)?;
        Ok(())
    }

    /// Analyze historical data to extract metrics for classification
    pub fn analyze_historical_data(symbol: &str) -> Result<(f64, f64, f64, f64), Box<dyn Error>> {
        // Load historical data
        let csv_file = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let historical_data = crate::market_data::csv_loader::load_csv_closes(&csv_file)?;

        if historical_data.len() < 100 {
            return Err(format!("Insufficient data for {}", symbol).into());
        }

        // Calculate average volatility (annualized)
        let returns: Vec<f64> = historical_data.windows(2)
            .map(|w| (w[1].close / w[0].close) - 1.0)
            .collect();

        let avg_volatility = if returns.len() > 1 {
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>() / returns.len() as f64;
            variance.sqrt() * (252.0_f64).sqrt() * 100.0 // Annualized percentage
        } else {
            0.0
        };

        // Calculate trend strength (R-squared of linear regression)
        let trend_strength = calculate_trend_strength(&historical_data);

        // Calculate mean reversion tendency (autocorrelation of returns)
        let mean_reversion_tendency = calculate_mean_reversion_tendency(&returns);

        // Calculate momentum sensitivity (correlation with momentum indicators)
        let momentum_sensitivity = calculate_momentum_sensitivity(&historical_data);

        Ok((avg_volatility, trend_strength, mean_reversion_tendency, momentum_sensitivity))
    }
}

/// Calculate trend strength using R-squared of linear regression
fn calculate_trend_strength(data: &[crate::market_data::csv_loader::HistoricalDay]) -> f64 {
    if data.len() < 50 {
        return 0.0;
    }

    let n = data.len() as f64;
    let prices: Vec<f64> = data.iter().map(|d| d.close).collect();

    // Simple linear regression: y = mx + b
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = prices.iter().sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut denominator = 0.0;

    for (i, &price) in prices.iter().enumerate() {
        let x = i as f64;
        numerator += (x - x_mean) * (price - y_mean);
        denominator += (x - x_mean).powi(2);
    }

    if denominator == 0.0 {
        return 0.0;
    }

    let slope = numerator / denominator;

    // Calculate R-squared
    let mut ss_res = 0.0;
    let mut ss_tot = 0.0;

    for (i, &price) in prices.iter().enumerate() {
        let x = i as f64;
        let predicted = y_mean + slope * (x - x_mean);
        ss_res += (price - predicted).powi(2);
        ss_tot += (price - y_mean).powi(2);
    }

    if ss_tot == 0.0 {
        return 0.0;
    }

    1.0 - (ss_res / ss_tot)
}

/// Calculate mean reversion tendency using return autocorrelation
fn calculate_mean_reversion_tendency(returns: &[f64]) -> f64 {
    if returns.len() < 20 {
        return 0.0;
    }

    // Calculate autocorrelation at lag 1 (negative = mean reverting)
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;

    let mut numerator = 0.0;
    let mut denominator = 0.0;

    for i in 1..returns.len() {
        let ret_t = returns[i] - mean;
        let ret_t_minus_1 = returns[i - 1] - mean;

        numerator += ret_t * ret_t_minus_1;
        denominator += ret_t_minus_1.powi(2);
    }

    if denominator == 0.0 {
        return 0.0;
    }

    // Return absolute value (higher = more mean reverting)
    (numerator / denominator).abs()
}

/// Calculate momentum sensitivity using correlation with momentum indicators
fn calculate_momentum_sensitivity(data: &[crate::market_data::csv_loader::HistoricalDay]) -> f64 {
    if data.len() < 50 {
        return 0.0;
    }

    let prices: Vec<f64> = data.iter().map(|d| d.close).collect();

    // Calculate 5-day momentum
    let mut momentum: Vec<f64> = Vec::new();
    for i in 5..prices.len() {
        let mom = (prices[i] / prices[i - 5] - 1.0) * 100.0;
        momentum.push(mom);
    }

    // Calculate price changes
    let mut price_changes: Vec<f64> = Vec::new();
    for i in 1..prices.len() {
        let change = (prices[i] / prices[i - 1] - 1.0) * 100.0;
        price_changes.push(change);
    }

    // Align the series (momentum is shorter)
    let aligned_changes = &price_changes[4..]; // Skip first 4 to align with momentum

    if momentum.len() != aligned_changes.len() || momentum.len() < 10 {
        return 0.0;
    }

    // Calculate correlation
    let n = momentum.len() as f64;
    let mom_mean = momentum.iter().sum::<f64>() / n;
    let change_mean = aligned_changes.iter().sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut mom_var = 0.0;
    let mut change_var = 0.0;

    for i in 0..momentum.len() {
        let mom_diff = momentum[i] - mom_mean;
        let change_diff = aligned_changes[i] - change_mean;

        numerator += mom_diff * change_diff;
        mom_var += mom_diff.powi(2);
        change_var += change_diff.powi(2);
    }

    if mom_var == 0.0 || change_var == 0.0 {
        return 0.0;
    }

    let correlation = numerator / (mom_var.sqrt() * change_var.sqrt());

    // Return absolute value (higher = more momentum sensitive)
    correlation.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_classification() {
        let mut classifier = StockClassifier::new();

        // Test high volatility momentum leader (like NVDA)
        let profile = classifier.classify_stock("NVDA", 0.6, 0.8, 0.3, 0.8);
        assert_eq!(profile.personality, StockPersonality::MomentumLeader);
        assert!(profile.best_strategies.contains(&"Short-Term Momentum".to_string()));

        // Test mean reverting stock (like TSLA)
        let profile = classifier.classify_stock("TSLA", 0.7, 0.4, 0.7, 0.4);
        assert_eq!(profile.personality, StockPersonality::MeanReverting);
        assert!(profile.best_strategies.contains(&"Volatility Mean Reversion".to_string()));
    }
}