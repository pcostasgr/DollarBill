// examples/strategy_deployment.rs
use dollarbill::strategies::{StrategyRegistry, factory::StrategyFactory, TradingStrategy};
use std::error::Error;

/// Demonstrates different strategy deployment patterns
fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 DollarBill Strategy Deployment Demo");
    println!("=====================================");

    // Example 1: Manual strategy registration
    println!("\n📋 Example 1: Manual Strategy Registration");
    demo_manual_registration()?;

    // Example 2: Configuration-driven deployment
    println!("\n⚙️ Example 2: Configuration-Driven Deployment");
    demo_config_deployment()?;

    // Example 3: Strategy comparison
    println!("\n📊 Example 3: Strategy Performance Comparison");
    demo_strategy_comparison()?;

    // Example 4: Ensemble Strategy
    println!("\n🎭 Example 4: Ensemble Strategy");
    demo_ensemble_strategy()?;

    Ok(())
}

/// Demonstrate manual strategy registration
fn demo_manual_registration() -> Result<(), Box<dyn Error>> {
    use dollarbill::strategies::{vol_mean_reversion::VolMeanReversion, momentum::MomentumStrategy};

    let mut registry = StrategyRegistry::new();

    // Register strategies manually
    registry.register(Box::new(VolMeanReversion::new()));
    registry.register(Box::new(MomentumStrategy::new()));

    println!("Registered strategies:");
    for (i, name) in registry.list_strategies().iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }

    // Generate signals for a sample symbol
    let test_symbols = vec!["AAPL", "MSFT", "GOOGL"];
    let test_spot = 150.0;
    let test_market_iv = 0.25;
    let test_model_iv = 0.22;
    let test_hist_vol = 0.20;

    for symbol in test_symbols {
        println!("\n📈 Signals for {} at ${:.2}:", symbol, test_spot);

        let signals = registry.generate_all_signals(
            symbol, test_spot, test_market_iv, test_model_iv, test_hist_vol
        );

        if signals.is_empty() {
            println!("  No signals generated");
        } else {
            for signal in signals {
                println!("  {}: {:?} at ${:.2}, Confidence: {:.1}%",
                        signal.strategy_name,
                        signal.action,
                        signal.strike,
                        signal.confidence * 100.0);
            }
        }
    }

    Ok(())
}

/// Demonstrate configuration-driven deployment
fn demo_config_deployment() -> Result<(), Box<dyn Error>> {
    println!("Loading strategies from config/strategy_deployment.json...");

    let registry = StrategyFactory::load_strategy_registry("config/strategy_deployment.json")?;

    println!("Successfully loaded {} strategies from configuration:",
             registry.list_strategies().len());

    for (i, name) in registry.list_strategies().iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }

    // Test with different market conditions
    let market_scenarios = vec![
        ("Normal Market", 150.0, 0.22, 0.20, 0.18),
        ("High Vol Market", 150.0, 0.35, 0.32, 0.25),
        ("Low Vol Market", 150.0, 0.15, 0.18, 0.12),
    ];

    for (scenario_name, spot, market_iv, model_iv, hist_vol) in market_scenarios {
        println!("\n🌍 {} (IV: {:.1}%, Spot: ${:.2}):",
                scenario_name, market_iv * 100.0, spot);

        let signals = registry.generate_all_signals(
            "AAPL", spot, market_iv, model_iv, hist_vol
        );

        if signals.is_empty() {
            println!("  No signals generated");
        } else {
            for signal in signals {
                println!("  {}: {:?}, Edge: ${:.2}",
                        signal.strategy_name, signal.action, signal.edge);
            }
        }
    }

    Ok(())
}

/// Demonstrate strategy performance comparison
fn demo_strategy_comparison() -> Result<(), Box<dyn Error>> {
    println!("Comparing strategy performance across different market conditions...");

    let registry = StrategyFactory::create_default_registry();

    // Simulate different market conditions
    let conditions = vec![
        ("Bull Market", 180.0, 0.20, 0.18, 0.15),
        ("Bear Market", 120.0, 0.30, 0.28, 0.25),
        ("Sideways Market", 150.0, 0.18, 0.20, 0.16),
        ("High Vol Spike", 150.0, 0.45, 0.35, 0.30),
    ];

    println!("\nStrategy Performance Matrix:");
    println!("{:<15} {:<12} {:<8} {:<8} {:<8}",
             "Strategy", "Condition", "Signals", "Avg Conf", "Avg Edge");

    for strategy_name in registry.list_strategies() {
        for (condition_name, spot, market_iv, model_iv, hist_vol) in &conditions {
            let signals = registry.generate_all_signals(
                "TEST", *spot, *market_iv, *model_iv, *hist_vol
            );

            let strategy_signals: Vec<_> = signals.into_iter()
                .filter(|s| s.strategy_name == strategy_name)
                .collect();

            let avg_confidence = if strategy_signals.is_empty() {
                0.0
            } else {
                strategy_signals.iter().map(|s| s.confidence).sum::<f64>() / strategy_signals.len() as f64
            };

            let avg_edge = if strategy_signals.is_empty() {
                0.0
            } else {
                strategy_signals.iter().map(|s| s.edge).sum::<f64>() / strategy_signals.len() as f64
            };

            println!("{:<15} {:<12} {:<8} {:<8.1} ${:<8.2}",
                     strategy_name,
                     condition_name,
                     strategy_signals.len(),
                     avg_confidence * 100.0,
                     avg_edge);
        }
        println!();
    }

    Ok(())
}

/// Demonstrate ensemble strategy
fn demo_ensemble_strategy() -> Result<(), Box<dyn Error>> {
    use dollarbill::strategies::{vol_mean_reversion::VolMeanReversion, momentum::MomentumStrategy, ensemble::EnsembleStrategy};

    let mut ensemble = EnsembleStrategy::new();

    // Add individual strategies with weights
    ensemble.add_strategy(Box::new(VolMeanReversion::new()), 0.6);
    ensemble.add_strategy(Box::new(MomentumStrategy::new()), 0.4);

    println!("Ensemble strategy combines:");
    println!("  - Vol Mean Reversion (60% weight)");
    println!("  - Momentum (40% weight)");

    // Test ensemble in different conditions
    let test_conditions = vec![
        ("Normal Market", 150.0, 0.22, 0.20, 0.18),
        ("High Vol Spike", 150.0, 0.45, 0.35, 0.30),
        ("Low Vol Market", 150.0, 0.15, 0.18, 0.12),
    ];

    for (condition_name, spot, market_iv, model_iv, hist_vol) in test_conditions {
        println!("\n🌍 {}:", condition_name);

        let signals = ensemble.generate_signals("AAPL", spot, market_iv, model_iv, hist_vol);

        if signals.is_empty() {
            println!("  No ensemble signals generated");
        } else {
            for signal in signals {
                println!("  Ensemble: {:?}, Confidence: {:.1}%, Edge: ${:.2}",
                        signal.action,
                        signal.confidence * 100.0,
                        signal.edge);
            }
        }

        // Compare with individual strategies
        let vol_signals = VolMeanReversion::new().generate_signals("AAPL", spot, market_iv, model_iv, hist_vol);
        let mom_signals = MomentumStrategy::new().generate_signals("AAPL", spot, market_iv, model_iv, hist_vol);

        println!("  Individual strategies:");
        if !vol_signals.is_empty() {
            println!("    Vol Mean Reversion: {:?}", vol_signals[0].action);
        } else {
            println!("    Vol Mean Reversion: No signal");
        }
        if !mom_signals.is_empty() {
            println!("    Momentum: {:?}", mom_signals[0].action);
        } else {
            println!("    Momentum: No signal");
        }
    }

    Ok(())
}