// Strategy matching engine
// Intelligently matches strategies to stocks based on personality analysis

use std::collections::HashMap;
use std::error::Error;
use crate::analysis::stock_classifier::StockClassifier;
use crate::analysis::performance_matrix::{PerformanceMatrix, StrategyRecommendations};
use crate::strategies::{TradingStrategy, momentum::MomentumStrategy, vol_mean_reversion::VolMeanReversion, cash_secured_puts::CashSecuredPuts, mean_reversion::MeanReversionStrategy, breakout::BreakoutStrategy, vol_arbitrage::VolatilityArbitrageStrategy};

/// Main strategy matching engine
pub struct StrategyMatcher {
    pub classifier: StockClassifier,
    pub performance_matrix: PerformanceMatrix,
    strategy_cache: HashMap<String, String>, // stock -> strategy_name
}

impl StrategyMatcher {
    /// Create new strategy matcher
    pub fn new() -> Self {
        Self {
            classifier: StockClassifier::new(),
            performance_matrix: PerformanceMatrix::new(),
            strategy_cache: HashMap::new(),
        }
    }

    /// Load matcher from saved data
    pub fn load_from_files(
        classifier_file: &str,
        performance_file: &str
    ) -> Result<Self, Box<dyn Error>> {
        let classifier = StockClassifier::load_from_file(classifier_file)?;
        let performance_matrix = PerformanceMatrix::load_from_file(performance_file)?;

        Ok(Self {
            classifier,
            performance_matrix,
            strategy_cache: HashMap::new(),
        })
    }

    /// Build matcher from historical backtest data
    pub fn build_from_backtests(symbols: &[String]) -> Result<Self, Box<dyn Error>> {
        let mut matcher = Self::new();

        println!("🏗️  Building strategy matcher from historical data...");

        // Analyze each stock
        for symbol in symbols {
            println!("  📊 Analyzing {}...", symbol);

            match matcher.classifier.classify_stock_enhanced(symbol, "Technology") {
                Ok(profile) => {
                    // classify_stock_enhanced already prints detailed classification output
                    println!("    🏆 Best strategies: {:?}", profile.best_strategies);
                }
                Err(e) => {
                    println!("    ⚠️  Failed to analyze {}: {}", symbol, e);
                }
            }
        }

        // Load performance data from backtests (if available)
        matcher.load_performance_data()?;

        println!("✅ Strategy matcher built successfully!");
        Ok(matcher)
    }

    /// Get optimal strategy for a stock
    pub fn get_optimal_strategy(&mut self, symbol: &str) -> Result<Box<dyn TradingStrategy>, Box<dyn Error>> {
        // Check cache first
        if let Some(strategy_name) = self.strategy_cache.get(symbol) {
            return self.create_strategy(strategy_name);
        }

        // Get recommendations from performance matrix
        let recommendations = self.performance_matrix.generate_recommendations(symbol);

        // If we have performance data, use the best strategy
        if recommendations.confidence_score > 0.3 {
            let strategy = self.create_strategy(&recommendations.recommended_strategy)?;
            self.strategy_cache.insert(symbol.to_string(), recommendations.recommended_strategy.clone());
            return Ok(strategy);
        }

        // Fallback to personality-based selection
        let profile = match self.classifier.get_profile(symbol) {
            Some(p) => p,
            None => return Err(format!("No profile available for {}", symbol).into()),
        };

        let strategy_name = profile.best_strategies.first()
            .ok_or_else(|| format!("No strategies available for {}", symbol))?;

        let strategy = self.create_strategy(strategy_name)?;
        self.strategy_cache.insert(symbol.to_string(), strategy_name.clone());
        Ok(strategy)
    }

    /// Get strategy recommendations for a stock
    pub fn get_recommendations(&self, symbol: &str) -> StrategyRecommendations {
        self.performance_matrix.generate_recommendations(symbol)
    }

    /// Create strategy instance by name
    fn create_strategy(&self, strategy_name: &str) -> Result<Box<dyn TradingStrategy>, Box<dyn Error>> {
        match strategy_name {
            "Short-Term Momentum" | "Momentum Trading" => {
                Ok(Box::new(MomentumStrategy::new()))
            }
            "Volatility Mean Reversion" | "Mean Reversion" => {
                Ok(Box::new(VolMeanReversion::new()))
            }
            "Cash-Secured Puts" => {
                Ok(Box::new(CashSecuredPuts::new()))
            }
            "Statistical Arbitrage" => {
                Ok(Box::new(MeanReversionStrategy::new()))
            }
            "Breakout Trading" | "Breakout" => {
                Ok(Box::new(BreakoutStrategy::new()))
            }
            "Vol Arbitrage" | "Volatility Arbitrage" | "Volatility Trading" => {
                Ok(Box::new(VolatilityArbitrageStrategy::new()))
            }
            // Map personality-recommended strategies to existing implementations
            "Medium-Term RSI" | "Moving Average Crossover" | "Trend Following" => {
                // RSI and moving averages are momentum-based, map to momentum strategy
                Ok(Box::new(MomentumStrategy::new()))
            }
            "Iron Butterfly" | "Calendar Spreads" | "Volatility Harvesting" => {
                // Volatility strategies map to vol arbitrage for more sophisticated approach
                Ok(Box::new(VolatilityArbitrageStrategy::new()))
            }
            "Covered Calls" | "Cash-Secured Put" => {
                // Income strategies map to cash-secured puts
                Ok(Box::new(CashSecuredPuts::new()))
            }
            "Short-Term Scalping" | "High-Frequency Trading" => {
                // Fast trading strategies map to breakout
                Ok(Box::new(BreakoutStrategy::new()))
            }
            // Add more strategies as they become available
            _ => {
                // Default fallback with variety rotation
                let strategies = ["Momentum", "Mean Reversion", "Breakout", "Vol Arbitrage"];
                let hash = strategy_name.chars().map(|c| c as usize).sum::<usize>();
                let idx = hash % strategies.len();
                
                match strategies[idx] {
                    "Mean Reversion" => Ok(Box::new(MeanReversionStrategy::new())),
                    "Breakout" => Ok(Box::new(BreakoutStrategy::new())),
                    "Vol Arbitrage" => Ok(Box::new(VolatilityArbitrageStrategy::new())),
                    _ => Ok(Box::new(MomentumStrategy::new())),
                }
            }
        }
    }

    /// Load performance data from backtest results.
    ///
    /// In production, call `PerformanceMatrix::load_from_file()` with actual
    /// persisted backtest output. The previous implementation hard-coded five
    /// fabricated `PerformanceMetrics` entries for NVDA and TSLA — those have
    /// been removed so the matcher starts empty and is populated only from
    /// real data.
    fn load_performance_data(&mut self) -> Result<(), Box<dyn Error>> {
        // No-op: real data should be loaded via PerformanceMatrix::load_from_file()
        // or added incrementally through add_result() after running backtests.
        Ok(())
    }

    /// Get classifier reference
    pub fn get_classifier(&self) -> &StockClassifier {
        &self.classifier
    }

    /// Get performance matrix reference
    pub fn get_performance_matrix(&self) -> &PerformanceMatrix {
        &self.performance_matrix
    }

    /// Save matcher state to files
    pub fn save_to_files(&self, classifier_file: &str, performance_file: &str) -> Result<(), Box<dyn Error>> {
        self.classifier.save_to_file(classifier_file)?;
        self.performance_matrix.save_to_file(performance_file)?;
        Ok(())
    }

    /// Print comprehensive analysis
    pub fn print_analysis(&self) {
        println!("\n{}", "=".repeat(80));
        println!("STRATEGY MATCHING ANALYSIS");
        println!("{}", "=".repeat(80));

        println!("\n📊 STOCK PERSONALITIES:");
        for (symbol, profile) in self.classifier.get_all_profiles() {
            println!("  {}: {:?}", symbol, profile.personality);
            println!("    Best: {:?}, Worst: {:?}", profile.best_strategies, profile.worst_strategies);
        }

        println!("\n🎯 RECOMMENDATIONS:");
        for stock in self.performance_matrix.get_all_stocks() {
            let recs = self.get_recommendations(&stock);
            if recs.confidence_score > 0.0 {
                println!("  {}: {} (Confidence: {:.1}%)",
                        stock, recs.recommended_strategy, recs.confidence_score * 100.0);
                println!("    Reasoning: {}", recs.reasoning);
            }
        }

        println!("\n{}", "=".repeat(80));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::stock_classifier::StockPersonality;

    #[test]
    fn test_strategy_matching() {
        let mut matcher = StrategyMatcher::new();

        // Add a mock profile for NVDA
        #[allow(deprecated)]
        let profile = matcher.classifier.classify_stock("NVDA", 0.6, 0.8, 0.3, 0.8);
        assert_eq!(profile.personality, StockPersonality::MomentumLeader);

        // Test strategy selection
        let strategy = matcher.get_optimal_strategy("NVDA");
        assert!(strategy.is_ok()); // Should work with personality-based fallback
    }
}