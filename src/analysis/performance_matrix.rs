// Performance matrix for strategy-stock combinations
// Tracks historical performance to guide strategy selection

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Performance metrics for strategy-stock combinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_return: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: usize,
    pub avg_holding_period: f64,
}

/// Performance matrix mapping (stock, strategy) -> performance
pub struct PerformanceMatrix {
    matrix: HashMap<(String, String), PerformanceMetrics>,
    stock_summaries: HashMap<String, StockSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSummary {
    pub best_strategy: String,
    pub best_sharpe: f64,
    pub worst_strategy: String,
    pub worst_sharpe: f64,
    pub total_strategies_tested: usize,
    pub avg_performance: PerformanceMetrics,
}

impl PerformanceMatrix {
    /// Create new empty performance matrix
    pub fn new() -> Self {
        Self {
            matrix: HashMap::new(),
            stock_summaries: HashMap::new(),
        }
    }

    /// Add performance result for a strategy-stock combination
    pub fn add_result(&mut self, stock: &str, strategy: &str, metrics: PerformanceMetrics) {
        let key = (stock.to_string(), strategy.to_string());
        self.matrix.insert(key, metrics);
        self.update_stock_summary(stock);
    }

    /// Get performance metrics for a specific stock-strategy combination
    pub fn get_performance(&self, stock: &str, strategy: &str) -> Option<&PerformanceMetrics> {
        let key = (stock.to_string(), strategy.to_string());
        self.matrix.get(&key)
    }

    /// Get all strategies tested for a stock
    pub fn get_strategies_for_stock(&self, stock: &str) -> Vec<String> {
        self.matrix
            .keys()
            .filter(|(s, _)| s == stock)
            .map(|(_, strategy)| strategy.clone())
            .collect()
    }

    /// Get all stocks in the matrix
    pub fn get_all_stocks(&self) -> Vec<String> {
        let mut stocks: Vec<String> = self.stock_summaries.keys().cloned().collect();
        stocks.sort();
        stocks
    }

    /// Generate strategy recommendations for a stock
    pub fn generate_recommendations(&self, stock: &str) -> StrategyRecommendations {
        let summary = match self.stock_summaries.get(stock) {
            Some(s) => s,
            None => return StrategyRecommendations {
                recommended_strategy: "Unknown".to_string(),
                avoid_strategy: "Unknown".to_string(),
                confidence_score: 0.0,
                reasoning: "No data available for this stock".to_string(),
            },
        };

        let confidence = self.calculate_confidence(stock, &summary.best_strategy, &summary.worst_strategy);

        StrategyRecommendations {
            recommended_strategy: summary.best_strategy.clone(),
            avoid_strategy: summary.worst_strategy.clone(),
            confidence_score: confidence,
            reasoning: format!(
                "Based on {} strategies tested. Best Sharpe: {:.2} vs Worst: {:.2}",
                summary.total_strategies_tested,
                summary.best_sharpe,
                summary.worst_sharpe
            ),
        }
    }

    /// Calculate confidence score for recommendations
    fn calculate_confidence(&self, stock: &str, best: &str, worst: &str) -> f64 {
        let best_metrics = match self.get_performance(stock, best) {
            Some(m) => m,
            None => return 0.0,
        };
        let worst_metrics = match self.get_performance(stock, worst) {
            Some(m) => m,
            None => return 0.0,
        };

        // Confidence based on Sharpe ratio difference
        let sharpe_diff = best_metrics.sharpe_ratio - worst_metrics.sharpe_ratio;
        let normalized_diff = sharpe_diff.abs() / (best_metrics.sharpe_ratio.abs() + worst_metrics.sharpe_ratio.abs() + 0.1);

        // Scale to 0-1 range
        (normalized_diff * 2.0).min(1.0)
    }

    /// Update stock summary after adding new result
    fn update_stock_summary(&mut self, stock: &str) {
        let strategies = self.get_strategies_for_stock(stock);
        if strategies.is_empty() {
            return;
        }

        let mut best_strategy = &strategies[0];
        let mut best_sharpe = f64::NEG_INFINITY;
        let mut worst_strategy = &strategies[0];
        let mut worst_sharpe = f64::INFINITY;
        let mut total_return = 0.0;
        let mut total_sharpe = 0.0;
        let mut total_drawdown = 0.0;
        let mut total_win_rate = 0.0;
        let mut total_profit_factor = 0.0;
        let mut total_trades = 0;
        let mut total_holding = 0.0;

        for strategy in &strategies {
            if let Some(metrics) = self.get_performance(stock, strategy) {
                if metrics.sharpe_ratio > best_sharpe {
                    best_sharpe = metrics.sharpe_ratio;
                    best_strategy = strategy;
                }
                if metrics.sharpe_ratio < worst_sharpe {
                    worst_sharpe = metrics.sharpe_ratio;
                    worst_strategy = strategy;
                }

                total_return += metrics.total_return;
                total_sharpe += metrics.sharpe_ratio;
                total_drawdown += metrics.max_drawdown;
                total_win_rate += metrics.win_rate;
                total_profit_factor += metrics.profit_factor;
                total_trades += metrics.total_trades;
                total_holding += metrics.avg_holding_period;
            }
        }

        let count = strategies.len() as f64;
        let avg_metrics = PerformanceMetrics {
            total_return: total_return / count,
            sharpe_ratio: total_sharpe / count,
            max_drawdown: total_drawdown / count,
            win_rate: total_win_rate / count,
            profit_factor: total_profit_factor / count,
            total_trades: (total_trades as f64 / count) as usize,
            avg_holding_period: total_holding / count,
        };

        let summary = StockSummary {
            best_strategy: best_strategy.clone(),
            best_sharpe,
            worst_strategy: worst_strategy.clone(),
            worst_sharpe,
            total_strategies_tested: strategies.len(),
            avg_performance: avg_metrics,
        };

        self.stock_summaries.insert(stock.to_string(), summary);
    }

    /// Load performance matrix from JSON file
    pub fn load_from_file(filepath: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(filepath)?;
        let data: PerformanceMatrixData = serde_json::from_str(&content)?;
        Ok(data.into_matrix())
    }

    /// Save performance matrix to JSON file
    pub fn save_to_file(&self, filepath: &str) -> Result<(), Box<dyn std::error::Error>> {
        let data = PerformanceMatrixData::from_matrix(self);
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(filepath, json)?;
        Ok(())
    }
}

/// Strategy recommendations for a stock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRecommendations {
    pub recommended_strategy: String,
    pub avoid_strategy: String,
    pub confidence_score: f64,
    pub reasoning: String,
}

/// Serializable data structure for performance matrix
#[derive(Debug, Serialize, Deserialize)]
struct PerformanceMatrixData {
    matrix: HashMap<String, PerformanceMetrics>,
    stock_summaries: HashMap<String, StockSummary>,
}

impl PerformanceMatrixData {
    fn from_matrix(matrix: &PerformanceMatrix) -> Self {
        let mut serializable_matrix = HashMap::new();
        for ((stock, strategy), metrics) in &matrix.matrix {
            let key = format!("{}_{}", stock, strategy);
            serializable_matrix.insert(key, metrics.clone());
        }

        Self {
            matrix: serializable_matrix,
            stock_summaries: matrix.stock_summaries.clone(),
        }
    }

    fn into_matrix(self) -> PerformanceMatrix {
        let mut matrix = HashMap::new();
        for (key, metrics) in self.matrix {
            // Split the key back into stock and strategy
            if let Some(underscore_pos) = key.rfind('_') {
                let stock = key[..underscore_pos].to_string();
                let strategy = key[underscore_pos + 1..].to_string();
                matrix.insert((stock, strategy), metrics);
            }
        }

        PerformanceMatrix {
            matrix,
            stock_summaries: self.stock_summaries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_matrix() {
        let mut matrix = PerformanceMatrix::new();

        // Add NVDA Short-Term Momentum results (+270% return)
        let metrics1 = PerformanceMetrics {
            total_return: 2.70, // +270%
            sharpe_ratio: 1.85,
            max_drawdown: 0.25,
            win_rate: 68.0,
            profit_factor: 5.51,
            total_trades: 385,
            avg_holding_period: 10.0,
        };

        let metrics2 = PerformanceMetrics {
            total_return: -1.24, // -124%
            sharpe_ratio: 0.0,
            max_drawdown: 1.33,
            win_rate: 38.0,
            profit_factor: 2.06,
            total_trades: 353,
            avg_holding_period: 10.0,
        };

        matrix.add_result("NVDA", "Short-Term Momentum", metrics1);
        matrix.add_result("NVDA", "Long-Term Holding", metrics2);

        // Test recommendations
        let recs = matrix.generate_recommendations("NVDA");
        assert_eq!(recs.recommended_strategy, "Short-Term Momentum");
        assert_eq!(recs.avoid_strategy, "Long-Term Holding");
        assert!(recs.confidence_score > 0.5); // High confidence due to big Sharpe difference
    }
}