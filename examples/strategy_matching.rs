// Strategy matching example
// Demonstrates intelligent stock-strategy matching based on personality analysis

use dollarbill::analysis::stock_classifier::{StockClassifier, StockPersonality};
use dollarbill::analysis::performance_matrix::{PerformanceMatrix, PerformanceMetrics};
use dollarbill::strategies::matching::StrategyMatcher;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 DollarBill Strategy Matching Demo");
    println!("=====================================");

    // Create a stock classifier
    let mut classifier = StockClassifier::new();

    // Analyze some stocks with different characteristics
    println!("\n📊 Analyzing Stock Personalities...");

    // NVDA: High momentum, high volatility
    let nvda_profile = classifier.classify_stock("NVDA", 0.8, 0.9, 0.2, 0.9);
    println!("NVDA: {:?} - {:?}", nvda_profile.personality, nvda_profile.best_strategies);

    // TSLA: High volatility, mean-reverting tendencies
    let tsla_profile = classifier.classify_stock("TSLA", 0.6, 0.3, 0.7, 0.8);
    println!("TSLA: {:?} - {:?}", tsla_profile.personality, tsla_profile.best_strategies);

    // AAPL: Stable, trend-following
    let aapl_profile = classifier.classify_stock("AAPL", 0.4, 0.6, 0.3, 0.4);
    println!("AAPL: {:?} - {:?}", aapl_profile.personality, aapl_profile.best_strategies);

    // Create performance matrix
    let mut performance_matrix = PerformanceMatrix::new();

    // Add historical performance data
    println!("\n📈 Loading Performance Data...");

    // NVDA with Short-Term Momentum: Excellent performance
    let nvda_momentum = PerformanceMetrics {
        total_return: 2.70, // +270%
        sharpe_ratio: 1.85,
        max_drawdown: 0.25,
        win_rate: 68.0,
        profit_factor: 5.51,
        total_trades: 385,
        avg_holding_period: 10.0,
    };
    performance_matrix.add_result("NVDA", "Short-Term Momentum", nvda_momentum);

    // NVDA with Long-Term Holding: Poor performance
    let nvda_holding = PerformanceMetrics {
        total_return: -1.24, // -124%
        sharpe_ratio: 0.0,
        max_drawdown: 1.33,
        win_rate: 38.0,
        profit_factor: 2.06,
        total_trades: 353,
        avg_holding_period: 10.0,
    };
    performance_matrix.add_result("NVDA", "Long-Term Holding", nvda_holding);

    // TSLA with Short-Term Momentum: Poor performance
    let tsla_momentum = PerformanceMetrics {
        total_return: -0.85, // -85%
        sharpe_ratio: 0.2,
        max_drawdown: 0.8,
        win_rate: 42.0,
        profit_factor: 1.8,
        total_trades: 320,
        avg_holding_period: 10.0,
    };
    performance_matrix.add_result("TSLA", "Short-Term Momentum", tsla_momentum);

    // TSLA with Volatility Mean Reversion: Good performance
    let tsla_vol_reversion = PerformanceMetrics {
        total_return: 1.45, // +145%
        sharpe_ratio: 1.2,
        max_drawdown: 0.35,
        win_rate: 62.0,
        profit_factor: 3.2,
        total_trades: 280,
        avg_holding_period: 10.0,
    };
    performance_matrix.add_result("TSLA", "Volatility Mean Reversion", tsla_vol_reversion);

    // Create strategy matcher
    let mut matcher = StrategyMatcher::new();
    matcher.classifier = classifier;
    matcher.performance_matrix = performance_matrix;

    // Get recommendations
    println!("\n🎯 Strategy Recommendations:");

    for stock in &["NVDA", "TSLA", "AAPL"] {
        let recs = matcher.get_recommendations(stock);
        println!("  {}: {} (Confidence: {:.1}%)",
                stock, recs.recommended_strategy, recs.confidence_score * 100.0);
        println!("    Reasoning: {}", recs.reasoning);
    }

    // Test optimal strategy selection
    println!("\n🚀 Testing Optimal Strategy Selection:");

    for stock in &["NVDA", "TSLA"] {
        match matcher.get_optimal_strategy(stock) {
            Ok(strategy) => {
                println!("  {}: Selected {}", stock, strategy.name());
            }
            Err(e) => {
                println!("  {}: Error - {}", stock, e);
            }
        }
    }

    println!("\n✅ Strategy matching demo completed!");
    println!("   This demonstrates how stock personality analysis");
    println!("   combined with historical performance data enables");
    println!("   intelligent strategy selection for optimal returns.");

    Ok(())
}