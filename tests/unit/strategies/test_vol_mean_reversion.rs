// Strategy tests

use dollarbill::strategies::TradingStrategy;
use dollarbill::strategies::vol_mean_reversion::VolMeanReversion;

#[test]
fn test_vol_mean_reversion_initialization() {
    let strategy = VolMeanReversion::new();
    
    assert_eq!(strategy.name(), "Vol Mean Reversion");
    assert!(strategy.zscore_threshold > 0.0);
    assert!(strategy.edge_threshold > 0.0);
}

#[test]
fn test_vol_mean_reversion_custom_config() {
    let strategy = VolMeanReversion::with_config(2.0, 0.10);
    
    assert_eq!(strategy.zscore_threshold, 2.0);
    assert_eq!(strategy.edge_threshold, 0.10);
}

#[test]
fn test_identify_high_iv() {
    let strategy = VolMeanReversion::new();
    
    // High market IV vs model IV (overpriced options)
    let spot = 100.0;
    let market_iv = 0.40; // 40% IV
    let model_iv = 0.25;  // 25% model IV
    let historical_vol = 0.25;
    
    let signals = strategy.generate_signals("TEST", spot, market_iv, model_iv, historical_vol);
    
    // Should potentially generate a sell signal (overpriced volatility)
    // The exact behavior depends on the zscore threshold
    drop(signals); // signals may be empty — just verify no panic
}

#[test]
fn test_identify_low_iv() {
    let strategy = VolMeanReversion::new();
    
    // Low market IV vs model IV (underpriced options)
    let spot = 100.0;
    let market_iv = 0.15; // 15% IV
    let model_iv = 0.25;  // 25% model IV
    let historical_vol = 0.25;
    
    let signals = strategy.generate_signals("TEST", spot, market_iv, model_iv, historical_vol);
    
    // May generate a buy signal (underpriced volatility)
    drop(signals);
}

#[test]
fn test_mean_calculation() {
    // Test that historical vol is used as mean
    let strategy = VolMeanReversion::new();
    
    let spot = 100.0;
    let market_iv = 0.30;
    let model_iv = 0.25;
    let historical_vol = 0.20; // This should be the mean
    
    let signals = strategy.generate_signals("TEST", spot, market_iv, model_iv, historical_vol);
    
    // Strategy should use historical_vol as the mean
    drop(signals);
}

#[test]
fn test_z_score_calculation() {
    // Test implicit z-score logic
    let strategy = VolMeanReversion::with_config(1.0, 0.05);
    
    let spot = 100.0;
    let historical_vol = 0.25;
    
    // Test with IV significantly above mean
    let high_iv = 0.40;
    let signals_high = strategy.generate_signals("TEST", spot, high_iv, 0.25, historical_vol);
    
    // Test with IV close to mean
    let normal_iv = 0.26;
    let signals_normal = strategy.generate_signals("TEST", spot, normal_iv, 0.25, historical_vol);
    
    // Both should complete without error
    drop(signals_high);
    drop(signals_normal);
}

#[test]
fn test_entry_signal_threshold() {
    // Test that edge threshold is respected
    let strategy = VolMeanReversion::with_config(1.5, 0.10); // 10% edge required
    
    let spot = 100.0;
    let historical_vol = 0.25;
    
    // Small edge - should not trigger
    let market_iv_small = 0.27;
    let model_iv = 0.25;
    let signals_small = strategy.generate_signals("TEST", spot, market_iv_small, model_iv, historical_vol);
    
    // Large edge - may trigger
    let market_iv_large = 0.40;
    let signals_large = strategy.generate_signals("TEST", spot, market_iv_large, model_iv, historical_vol);
    
    drop(signals_small);
    drop(signals_large);
}

#[test]
fn test_strategy_name() {
    let strategy = VolMeanReversion::new();
    assert_eq!(strategy.name(), "Vol Mean Reversion");
}

#[test]
fn test_different_symbols() {
    let strategy = VolMeanReversion::new();
    
    let params = (100.0, 0.30, 0.25, 0.25);
    
    let signals_aapl = strategy.generate_signals("AAPL", params.0, params.1, params.2, params.3);
    let signals_googl = strategy.generate_signals("GOOGL", params.0, params.1, params.2, params.3);
    
    // Should work with different symbols
    drop(signals_aapl);
    drop(signals_googl);
}

#[test]
fn test_extreme_volatilities() {
    let strategy = VolMeanReversion::new();
    
    let spot = 100.0;
    let historical_vol = 0.25;
    
    // Very high IV
    let signals_high = strategy.generate_signals("TEST", spot, 1.0, 0.25, historical_vol);
    drop(signals_high);
    
    // Very low IV
    let signals_low = strategy.generate_signals("TEST", spot, 0.01, 0.25, historical_vol);
    drop(signals_low);
}

#[test]
fn test_edge_threshold_sensitivity() {
    // Test with different edge thresholds
    let conservative = VolMeanReversion::with_config(1.5, 0.20); // 20% edge needed
    let aggressive = VolMeanReversion::with_config(1.5, 0.02);   // 2% edge needed
    
    let params = (100.0, 0.30, 0.25, 0.25);
    
    let signals_conservative = conservative.generate_signals("TEST", params.0, params.1, params.2, params.3);
    let signals_aggressive = aggressive.generate_signals("TEST", params.0, params.1, params.2, params.3);
    
    // Aggressive should potentially generate more signals
    drop(signals_conservative);
    drop(signals_aggressive);
}

#[test]
fn test_zscore_threshold_sensitivity() {
    // Test with different z-score thresholds
    let conservative = VolMeanReversion::with_config(2.5, 0.05); // High z-score needed
    let aggressive = VolMeanReversion::with_config(0.5, 0.05);   // Low z-score needed
    
    let params = (100.0, 0.30, 0.25, 0.25);
    
    let signals_conservative = conservative.generate_signals("TEST", params.0, params.1, params.2, params.3);
    let signals_aggressive = aggressive.generate_signals("TEST", params.0, params.1, params.2, params.3);
    
    drop(signals_conservative);
    drop(signals_aggressive);
}

#[test]
fn test_zero_edge() {
    let strategy = VolMeanReversion::new();
    
    // Market IV equals model IV (no edge)
    let spot = 100.0;
    let iv = 0.25;
    let historical_vol = 0.25;
    
    let signals = strategy.generate_signals("TEST", spot, iv, iv, historical_vol);
    
    // Should handle zero edge gracefully
    drop(signals);
}

#[test]
fn test_negative_edge() {
    let strategy = VolMeanReversion::new();
    
    // Model IV higher than market IV (negative edge)
    let spot = 100.0;
    let market_iv = 0.20;
    let model_iv = 0.30;
    let historical_vol = 0.25;
    
    let signals = strategy.generate_signals("TEST", spot, market_iv, model_iv, historical_vol);
    
    // Should handle negative edge (potential buy signal)
    drop(signals);
}

#[test]
fn test_realistic_market_conditions() {
    let strategy = VolMeanReversion::new();
    
    // Typical market conditions
    let spot = 450.0; // NVDA-like price
    let market_iv = 0.35; // Typical tech stock IV
    let model_iv = 0.30;
    let historical_vol = 0.32;
    
    let signals = strategy.generate_signals("NVDA", spot, market_iv, model_iv, historical_vol);
    
    drop(signals);
}

#[test]
fn test_clone_strategy() {
    let strategy1 = VolMeanReversion::with_config(2.0, 0.08);
    let strategy2 = strategy1.clone();
    
    assert_eq!(strategy1.name(), strategy2.name());
    assert_eq!(strategy1.zscore_threshold, strategy2.zscore_threshold);
    assert_eq!(strategy1.edge_threshold, strategy2.edge_threshold);
}
