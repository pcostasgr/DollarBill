// Example: Strategy Showcase - Demonstrates all 6 trading strategies
// 
// This example shows all the new strategies in action with sample data
// No API credentials needed - pure demonstration mode
//
// Run: cargo run --example strategy_showcase

use dollarbill::analysis::stock_classifier::StockClassifier;
use dollarbill::strategies::{
    matching::StrategyMatcher,
    momentum::MomentumStrategy,
    vol_mean_reversion::VolMeanReversion,
    cash_secured_puts::CashSecuredPuts,
    mean_reversion::MeanReversionStrategy,
    breakout::BreakoutStrategy,
    vol_arbitrage::VolatilityArbitrageStrategy,
    TradingStrategy,
};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("\n🎭 DollarBill Strategy Showcase");
    println!("{}","=".repeat(80));
    println!("Demonstrating all 6 trading strategies with comprehensive market analysis");
    
    // Initialize the strategy systems
    let classifier = StockClassifier::new();
    let mut matcher = StrategyMatcher::new();
    
    println!("\n🏭 Available Strategies:");
    println!("{}", "-".repeat(40));
    let strategies: Vec<Box<dyn TradingStrategy>> = vec![
        Box::new(VolMeanReversion::new()),
        Box::new(MomentumStrategy::new()),
        Box::new(CashSecuredPuts::new()),
        Box::new(MeanReversionStrategy::new()),
        Box::new(BreakoutStrategy::new()),
        Box::new(VolatilityArbitrageStrategy::new()),
    ];
    
    for strategy in &strategies {
        println!("  ✅ {}", strategy.name());
    }
    
    // Test symbols across different sectors and volatility profiles
    let test_symbols = vec![
        ("TSLA", "High-vol tech/auto", 200.0, 0.65, 0.45),
        ("AAPL", "Large-cap tech", 180.0, 0.35, 0.30),
        ("NVDA", "High-growth AI", 250.0, 0.55, 0.50),
        ("SPY", "Market index", 420.0, 0.20, 0.18),
        ("COIN", "Crypto/fintech", 90.0, 0.85, 0.70),
        ("GLD", "Precious metals", 180.0, 0.15, 0.12),
    ];
    
    println!("\n🎯 Strategy Analysis by Symbol");
    println!("{}", "=".repeat(80));
    
    for (symbol, description, spot, market_iv, model_iv) in test_symbols {
        println!("\n📈 {} - {}", symbol, description);
        println!("   Spot: ${:.2} | Market IV: {:.1}% | Model IV: {:.1}%", 
            spot, market_iv * 100.0, model_iv * 100.0);
        println!("{}", "-".repeat(60));
        
        // Test all strategies for this symbol
        let mut signal_count = 0;
        
        for strategy in &strategies {
            let signals = strategy.generate_signals(symbol, spot, market_iv, model_iv, 30.0);
            if !signals.is_empty() {
                let signal = &signals[0]; // Take first signal
                let emoji = match strategy.name() {
                    "Vol Mean Reversion" => "🔄",
                    "Momentum" => "📈", 
                    "Cash-Secured Puts" => "💰",
                    "Mean Reversion" => "🔀",
                    "Breakout" => "🚀",
                    "Vol Arbitrage" => "⚡",
                    _ => "📊"
                };
                
                println!("  {} {}: {:?} | Conf: {:.1}% | {}",
                    emoji, strategy.name(), signal.action, signal.confidence * 100.0,
                    if signal.confidence >= 0.15 { "🟢 TRADE" } else { "⚪ WEAK" });
                signal_count += 1;
            }
        }
        
        // Get personality-matched strategies
        let personality_matches = matcher.get_recommendations(symbol);
        
        if !personality_matches.recommended_strategy.is_empty() {
            println!("  🧠 Personality Match: {} (Conf: {:.1}%)", 
                personality_matches.recommended_strategy, 
                personality_matches.confidence_score * 100.0);
        }
        
        println!("  📊 Total Signals Generated: {}", signal_count);
    }
    
    // Market Scenario Analysis
    println!("\n📊 Market Scenario Analysis");
    println!("{}", "=".repeat(80));
    
    let scenarios = vec![
        ("Bull Market", "Strong uptrend with rising volatility", 0.35, 0.30),
        ("Bear Market", "Declining market with high volatility", 0.45, 0.35),
        ("Sideways Market", "Range-bound with low volatility", 0.20, 0.18),
        ("Volatility Spike", "Sudden fear event", 0.65, 0.40),
        ("Post-Earnings", "High implied vol compression", 0.25, 0.40),
    ];
    
    for (scenario_name, description, market_iv, model_iv) in scenarios {
        println!("\n🎬 {} - {}", scenario_name, description);
        println!("   Market IV: {:.1}% | Model IV: {:.1}%", market_iv * 100.0, model_iv * 100.0);
        
        let mut total_signals = 0;
        let mut tradeable_signals = 0;
        
        // Test TSLA in each scenario  
        for strategy in &strategies {
            let signals = strategy.generate_signals("TSLA", 200.0, market_iv, model_iv, 30.0);
            if !signals.is_empty() {
                total_signals += signals.len();
                for signal in signals {
                    if signal.confidence >= 0.15 {
                        tradeable_signals += 1;
                    }
                }
            }
        }
        
        println!("   📈 Signals: {} total, {} tradeable (≥15% confidence)", 
            total_signals, tradeable_signals);
    }
    
    println!("\n✅ Strategy Showcase Complete!");
    println!("   • 6 sophisticated trading strategies implemented");
    println!("   • Personality-based stock matching");  
    println!("   • Comprehensive risk management");
    println!("   • Multi-market scenario adaptability");
    println!("   • Ready for live trading with proper API credentials");
    
    Ok(())
}