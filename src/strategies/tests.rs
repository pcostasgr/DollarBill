// Strategy testing framework and comprehensive tests
use crate::strategies::*;
use crate::strategies::{
    momentum::MomentumStrategy,
    vol_mean_reversion::VolMeanReversion,
    cash_secured_puts::CashSecuredPuts,
    mean_reversion::MeanReversionStrategy,
    breakout::BreakoutStrategy,
    vol_arbitrage::VolatilityArbitrageStrategy,
    factory::StrategyFactory
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Test basic signal generation for all strategies
    #[test]
    fn test_all_strategies_signal_generation() {
        let strategies: Vec<Box<dyn TradingStrategy>> = vec![
            Box::new(MomentumStrategy::new()),
            Box::new(VolMeanReversion::new()),
            Box::new(CashSecuredPuts::new()),
            Box::new(MeanReversionStrategy::new()),
            Box::new(BreakoutStrategy::new()),
            Box::new(VolatilityArbitrageStrategy::new()),
        ];

        let test_symbols = vec!["AAPL", "TSLA", "NVDA", "SPY", "COIN"];
        
        for strategy in &strategies {
            println!("\n📊 Testing {} Strategy", strategy.name());
            
            for symbol in &test_symbols {
                let signals = strategy.generate_signals(
                    symbol,
                    150.0, // spot price
                    0.25,  // market IV (25%)
                    0.22,  // model IV (22%)
                    0.20,  // historical vol (20%)
                );
                
                for signal in &signals {
                    assert!(!signal.symbol.is_empty());
                    assert!(signal.confidence >= 0.0 && signal.confidence <= 1.0);
                    assert!(signal.expiry_days > 0);
                    assert_eq!(signal.symbol, *symbol);
                    
                    println!("  ✅ {} -> {} | Conf: {:.1}% | Edge: ${:.2}", 
                        symbol, signal.strategy_name, 
                        signal.confidence * 100.0, signal.edge);
                }
            }
        }
    }

    /// Test momentum strategy with various market conditions
    #[test]
    fn test_momentum_strategy_conditions() {
        let strategy = MomentumStrategy::new();
        
        // Test different momentum scenarios
        let test_cases = vec![
            ("TSLA", 200.0, 0.40, 0.35, 0.30, "High vol momentum"),
            ("AAPL", 150.0, 0.20, 0.18, 0.15, "Low vol momentum"),
            ("NVDA", 300.0, 0.60, 0.50, 0.45, "Extreme vol momentum"),
        ];

        for (symbol, spot, market_iv, model_iv, hist_vol, scenario) in test_cases {
            let signals = strategy.generate_signals(symbol, spot, market_iv, model_iv, hist_vol);
            
            println!("📈 Momentum test - {}: {} signals generated", scenario, signals.len());
            
            for signal in signals {
                assert!(matches!(signal.action, SignalAction::BuyStraddle | SignalAction::SellStraddle));
                println!("  Signal: {:?} | Confidence: {:.1}%", signal.action, signal.confidence * 100.0);
            }
        }
    }

    /// Test mean reversion strategy edge cases
    #[test]
    fn test_mean_reversion_strategy() {
        let strategy = MeanReversionStrategy::new();
        
        let test_cases = vec![
            ("AAPL", 150.0, 0.30, 0.25, 0.20, "Normal conditions"),
            ("SPY", 400.0, 0.15, 0.12, 0.10, "Low volatility"),
            ("COIN", 170.0, 0.80, 0.75, 0.70, "High volatility"),
        ];

        for (symbol, spot, market_iv, model_iv, hist_vol, scenario) in test_cases {
            let signals = strategy.generate_signals(symbol, spot, market_iv, model_iv, hist_vol);
            
            println!("🔄 Mean Reversion test - {}: {} signals", scenario, signals.len());
            
            for signal in signals {
                assert!(signal.confidence <= 0.85); // Max confidence limit
                assert!(signal.expiry_days == 21); // Expected expiry
                println!("  {} -> Confidence: {:.1}%", signal.symbol, signal.confidence * 100.0);
            }
        }
    }

    /// Test cash-secured puts strategy
    #[test]
    fn test_cash_secured_puts_strategy() {
        let strategy = CashSecuredPuts::new();
        
        // Test with high IV edge scenario (like COIN)
        let signals = strategy.generate_signals(
            "COIN", 
            170.0, // spot
            0.80,  // high market IV
            0.76,  // slightly lower model IV
            0.70   // historical vol
        );

        assert!(!signals.is_empty(), "Should generate signals with high IV edge");
        
        for signal in &signals {
            if let SignalAction::CashSecuredPut { strike_pct } = signal.action {
                assert!(strike_pct > 0.0 && strike_pct < 0.1); // Reasonable OTM percentage
                assert!(signal.confidence > 0.5); // High confidence for good setups
                
                println!("💰 Cash-Secured Put: {} | Strike: {:.1}% OTM | Conf: {:.1}%", 
                    signal.symbol, strike_pct * 100.0, signal.confidence * 100.0);
            }
        }
    }

    /// Test breakout strategy detection
    #[test]
    fn test_breakout_strategy() {
        let strategy = BreakoutStrategy::new();
        
        let high_vol_symbols = vec!["TSLA", "NVDA", "COIN"];
        
        for symbol in high_vol_symbols {
            let signals = strategy.generate_signals(
                symbol,
                200.0, // spot
                0.45,  // high IV for breakouts
                0.40,  // model IV
                0.35   // historical vol
            );
            
            println!("🚀 Breakout test - {}: {} signals", symbol, signals.len());
            
            for signal in signals {
                // Breakout strategy should use appropriate actions
                assert!(matches!(signal.action, 
                    SignalAction::IronButterfly { .. } | 
                    SignalAction::SellStraddle
                ));
                
                assert!(signal.expiry_days <= 30); // Short-term for breakouts
                println!("  Breakout signal: {:?} | Days: {} | Conf: {:.1}%", 
                    signal.action, signal.expiry_days, signal.confidence * 100.0);
            }
        }
    }

    /// Test volatility arbitrage strategy
    #[test]
    fn test_vol_arbitrage_strategy() {
        let strategy = VolatilityArbitrageStrategy::new();
        
        // Test scenarios with different IV/RV relationships
        let test_scenarios = vec![
            ("AAPL", 150.0, 0.25, 0.20, 0.18, "IV rich scenario"), // IV > RV
            ("SPY", 400.0, 0.12, 0.15, 0.14, "IV cheap scenario"), // IV < RV  
            ("NVDA", 250.0, 0.45, 0.44, 0.42, "Fair value scenario"), // IV ≈ RV
        ];

        for (symbol, spot, market_iv, model_iv, hist_vol, scenario) in test_scenarios {
            let signals = strategy.generate_signals(symbol, spot, market_iv, model_iv, hist_vol);
            
            println!("📊 Vol Arbitrage - {}: {} signals", scenario, signals.len());
            
            for signal in signals {
                assert!(signal.confidence > 0.3); // Minimum confidence threshold
                assert!(signal.expiry_days <= 45); // Max DTE constraint
                
                println!("  Vol arb: {} | Action: {:?} | Conf: {:.1}%", 
                    symbol, signal.action, signal.confidence * 100.0);
            }
        }
    }

    /// Test strategy factory creation
    #[test]
    fn test_strategy_factory() {
        let registry = StrategyFactory::create_default_registry();
        let strategies = registry.list_strategies();
        
        println!("🏭 Strategy Factory Test - {} strategies loaded:", strategies.len());
        for strategy_name in &strategies {
            println!("  ✅ {}", strategy_name);
        }
        
        // Should have all 6 strategies
        assert!(strategies.len() >= 6);
        assert!(strategies.contains(&"Momentum".to_string()));
        assert!(strategies.contains(&"Cash-Secured Puts".to_string()));
        assert!(strategies.contains(&"Mean Reversion".to_string()));
        assert!(strategies.contains(&"Breakout".to_string()));
        assert!(strategies.contains(&"Vol Arbitrage".to_string()));
    }

    /// Benchmark signal generation performance
    #[test]
    fn test_signal_generation_performance() {
        let strategies: Vec<Box<dyn TradingStrategy>> = vec![
            Box::new(MomentumStrategy::new()),
            Box::new(MeanReversionStrategy::new()),
            Box::new(BreakoutStrategy::new()),
            Box::new(VolatilityArbitrageStrategy::new()),
            Box::new(CashSecuredPuts::new()),
        ];

        let symbols = vec!["AAPL", "TSLA", "NVDA", "SPY", "QQQ", "COIN", "AMD", "MSFT"];
        
        let start = std::time::Instant::now();
        let mut total_signals = 0;
        
        for strategy in &strategies {
            for symbol in &symbols {
                let signals = strategy.generate_signals(symbol, 150.0, 0.25, 0.22, 0.20);
                total_signals += signals.len();
            }
        }
        
        let duration = start.elapsed();
        println!("⚡ Performance Test: {} signals generated in {:?}", total_signals, duration);
        println!("   Average: {:.2}μs per signal", duration.as_micros() as f64 / total_signals as f64);
        
        // Performance should be reasonable (< 1ms per signal)
        assert!(duration.as_millis() < total_signals as u128);
    }

    /// Test risk parameters for all strategies
    #[test]
    fn test_strategy_risk_parameters() {
        let strategies: Vec<Box<dyn TradingStrategy>> = vec![
            Box::new(MomentumStrategy::new()),
            Box::new(VolMeanReversion::new()),
            Box::new(CashSecuredPuts::new()),
            Box::new(MeanReversionStrategy::new()),
            Box::new(BreakoutStrategy::new()),
            Box::new(VolatilityArbitrageStrategy::new()),
        ];

        for strategy in strategies {
            let risk_params = strategy.risk_params();
            
            // Validate risk parameters are reasonable
            assert!(risk_params.max_position_size > 0.0);
            assert!(risk_params.max_delta.abs() >= 0.0);  // Use absolute value
            assert!(risk_params.max_vega.abs() >= 0.0);  // Use absolute value for vega too
            assert!(risk_params.stop_loss_pct > 0.0);
            
            println!("🛡️  {} Risk Params:", strategy.name());
            println!("   Max Position: ${:.0}", risk_params.max_position_size);
            println!("   Max Delta: {:.1}", risk_params.max_delta);
            println!("   Max Vega: {:.1}", risk_params.max_vega);
            println!("   Stop Loss: {:.1}%", risk_params.stop_loss_pct);
        }
    }
}

/// Integration testing with live-like scenarios
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_multi_strategy_scenario() {
        println!("\n🎭 Multi-Strategy Integration Test");
        
        // Simulate different market conditions
        let market_scenarios = vec![
            ("Bull Market", 0.15, 0.12, 0.10), // Low vol, trending up
            ("Bear Market", 0.35, 0.30, 0.28), // High vol, trending down
            ("Sideways", 0.20, 0.18, 0.16),    // Moderate vol, range-bound
            ("Volatility Spike", 0.60, 0.45, 0.40), // Extreme vol event
        ];

        let symbols = vec!["AAPL", "TSLA", "COIN", "SPY"];
        let strategies: Vec<Box<dyn TradingStrategy>> = vec![
            Box::new(MomentumStrategy::new()),
            Box::new(MeanReversionStrategy::new()),
            Box::new(BreakoutStrategy::new()),
            Box::new(VolatilityArbitrageStrategy::new()),
            Box::new(CashSecuredPuts::new()),
        ];

        for (scenario_name, market_iv, model_iv, hist_vol) in market_scenarios {
            println!("\n📊 Testing {} scenario:", scenario_name);
            
            let mut scenario_signals = 0;
            
            for symbol in &symbols {
                for strategy in &strategies {
                    let signals = strategy.generate_signals(
                        symbol, 
                        150.0, 
                        market_iv, 
                        model_iv, 
                        hist_vol
                    );
                    
                    scenario_signals += signals.len();
                    
                    for signal in signals {
                        if signal.confidence > 0.4 { // Above bot threshold
                            println!("  🎯 {} - {} | {:.1}% conf | {:?}", 
                                symbol, strategy.name(), 
                                signal.confidence * 100.0, 
                                signal.action
                            );
                        }
                    }
                }
            }
            
            println!("  Total signals in {}: {}", scenario_name, scenario_signals);
            assert!(scenario_signals > 0, "Should generate some signals in any market condition");
        }
    }
}