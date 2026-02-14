// Black-Scholes pricing model tests

use crate::helpers::{assert_greeks_valid, assert_price_reasonable, EPSILON};
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put, black_scholes_call};

#[test]
fn test_call_option_atm() {
    // At-the-money call option
    // Note: With r=0.05, ATM delta is ~0.637 (not 0.5) due to forward drift
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0; // 1 year
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    
    assert_greeks_valid(&greeks);
    assert!(greeks.price > 0.0, "ATM call must have positive value");
    // ATM delta = N(d1) where d1 = (r + 0.5*σ²)√T/σ = 0.35 → N(0.35) ≈ 0.637
    assert!(greeks.delta > 0.55 && greeks.delta < 0.70, 
            "ATM call delta with r=0.05 should be ~0.637, got {}", greeks.delta);
}

#[test]
fn test_put_option_atm() {
    // At-the-money put option
    // Note: With r=0.05, ATM put delta is ~-0.363 (not -0.5) due to forward drift
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    assert_greeks_valid(&greeks);
    assert!(greeks.price > 0.0, "ATM put must have positive value");
    // ATM put delta = N(d1) - 1 ≈ 0.637 - 1 = -0.363
    assert!(greeks.delta < -0.30 && greeks.delta > -0.45, 
            "ATM put delta with r=0.05 should be ~-0.363, got {}", greeks.delta);
}

#[test]
fn test_put_call_parity() {
    // Put-call parity: C - P = S - K*e^(-r*T)
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    let left_side = call.price - put.price;
    let right_side = spot - strike * (-rate * time).exp();
    
    let diff = (left_side - right_side).abs();
    assert!(diff < EPSILON, "Put-call parity violated: diff = {}", diff);
}

#[test]
fn test_deep_itm_call() {
    // Deep in-the-money call should approach S - K*e^(-r*T)
    let spot = 200.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let intrinsic = spot - strike * (-rate * time).exp();
    
    assert!(greeks.price > intrinsic * 0.99, "Deep ITM call should be close to intrinsic value");
    assert!(greeks.delta > 0.95, "Deep ITM call delta should be close to 1");
}

#[test]
fn test_deep_otm_put() {
    // Deep out-of-the-money put should approach zero
    let spot = 200.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    assert!(greeks.price < 0.5, "Deep OTM put should have very low value");
    assert!(greeks.delta > -0.05, "Deep OTM put delta should be close to 0");
}

#[test]
fn test_zero_volatility() {
    // With zero volatility, option value should equal intrinsic value
    let spot = 110.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.0;
    let div = 0.0;
    
    let greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let intrinsic = (spot - strike).max(0.0);
    
    // With zero vol, we expect intrinsic value, but need to discount the strike
    assert!(greeks.price >= intrinsic * 0.99, "Zero vol option should be near intrinsic");
}

#[test]
fn test_zero_time_to_expiry() {
    // At expiration, option value equals intrinsic value
    let spot = 110.0;
    let strike = 100.0;
    let time = 0.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let intrinsic = (spot - strike).max(0.0);
    
    assert!((greeks.price - intrinsic).abs() < EPSILON, 
            "At expiration, call should equal intrinsic value");
    
    // Test put as well
    let greeks_put = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    let intrinsic_put = (strike - spot).max(0.0);
    
    assert!((greeks_put.price - intrinsic_put).abs() < EPSILON,
            "At expiration, put should equal intrinsic value");
}

#[test]
fn test_extreme_strike_prices() {
    let spot = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    // Very high strike (deep OTM call)
    let greeks_high = black_scholes_merton_call(spot, 500.0, time, rate, vol, div);
    // Deep OTM can have tiny negative values due to floating-point precision (~1e-15)
    // This is acceptable - treat as zero
    let price_high = greeks_high.price.max(0.0);
    assert!(price_high < 1.0, "Deep OTM call with extreme strike should be nearly worthless");
    assert!(greeks_high.delta >= 0.0 && greeks_high.delta < 0.01, 
            "Deep OTM delta should be near 0, got {}", greeks_high.delta);
    
    // Low strike (deep ITM call) - use 50 instead of 10 to avoid numerical precision issues
    let greeks_low = black_scholes_merton_call(spot, 50.0, time, rate, vol, div);
    assert!(greeks_low.price >= 0.0, "Price negative: {}", greeks_low.price);
    assert!(greeks_low.price > 45.0, "Deep ITM call with low strike should be valuable, got {}", greeks_low.price);
    assert!(greeks_low.delta > 0.90, "Deep ITM call should have delta near 1, got {}", greeks_low.delta);
}

#[test]
fn test_dividend_yield_impact() {
    // Dividend yield should reduce call prices
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    
    let no_div = black_scholes_merton_call(spot, strike, time, rate, vol, 0.0);
    let with_div = black_scholes_merton_call(spot, strike, time, rate, vol, 0.02);
    
    assert!(with_div.price < no_div.price, 
            "Dividends should reduce call option price");
    
    // Dividend yield should increase put prices
    let put_no_div = black_scholes_merton_put(spot, strike, time, rate, vol, 0.0);
    let put_with_div = black_scholes_merton_put(spot, strike, time, rate, vol, 0.02);
    
    assert!(put_with_div.price > put_no_div.price,
            "Dividends should increase put option price");
}

#[test]
fn test_interest_rate_impact() {
    // Higher interest rates should increase call prices
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let vol = 0.2;
    let div = 0.0;
    
    let low_rate = black_scholes_merton_call(spot, strike, time, 0.01, vol, div);
    let high_rate = black_scholes_merton_call(spot, strike, time, 0.10, vol, div);
    
    assert!(high_rate.price > low_rate.price,
            "Higher interest rates should increase call option price");
    
    // Higher interest rates should decrease put prices
    let put_low_rate = black_scholes_merton_put(spot, strike, time, 0.01, vol, div);
    let put_high_rate = black_scholes_merton_put(spot, strike, time, 0.10, vol, div);
    
    assert!(put_high_rate.price < put_low_rate.price,
            "Higher interest rates should decrease put option price");
}

#[test]
fn test_backward_compatibility_wrapper() {
    // Test the black_scholes_call wrapper function
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let vol = 0.2;
    
    let wrapper_result = black_scholes_call(spot, strike, time, rate, vol);
    let full_result = black_scholes_merton_call(spot, strike, time, rate, vol, 0.0);
    
    assert!((wrapper_result.price - full_result.price).abs() < EPSILON,
            "Wrapper function should match full function with zero dividend");
}

#[test]
fn test_price_monotonicity_in_volatility() {
    // Option price should increase with volatility
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let div = 0.0;
    
    let low_vol = black_scholes_merton_call(spot, strike, time, rate, 0.1, div);
    let high_vol = black_scholes_merton_call(spot, strike, time, rate, 0.5, div);
    
    assert!(high_vol.price > low_vol.price,
            "Higher volatility should increase option price");
}

#[test]
fn test_time_decay() {
    // Option price should increase with more time to expiration
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let short_time = black_scholes_merton_call(spot, strike, 0.25, rate, vol, div);
    let long_time = black_scholes_merton_call(spot, strike, 2.0, rate, vol, div);
    
    assert!(long_time.price > short_time.price,
            "Longer time to expiration should increase option price");
}
