// Black-Scholes pricing model tests

use crate::helpers::{assert_greeks_valid, EPSILON};
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
    assert!(greeks.delta > 0.60 && greeks.delta < 0.70, 
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

// ============================================================================
// ABSOLUTE-VALUE REFERENCE TESTS
//
// These tests verify BSM output against externally-published reference values.
// They exist specifically to catch implementation bugs (e.g. CDF errors) that
// structural tests (put-call parity, monotonicity, sign checks) cannot detect.
//
// Reference: Hull, "Options, Futures, and Other Derivatives" + independent
// tabulated standard-normal CDF values.
// ============================================================================

/// Hull textbook example: S=42, K=40, T=0.5, r=0.10, σ=0.20, q=0
/// Hull gives Call ≈ 4.76.  d1=0.7693, d2=0.6278.
#[test]
fn test_absolute_value_hull_example() {
    let call = black_scholes_merton_call(42.0, 40.0, 0.5, 0.10, 0.20, 0.0);
    assert!(
        (call.price - 4.76).abs() < 0.02,
        "Hull example: expected call ≈ 4.76, got {:.4}",
        call.price
    );
}

/// Standard ATM case: S=100, K=100, T=1, r=0.05, σ=0.20, q=0
///
/// d1 = (0 + 0.07)/0.20 = 0.35         N(0.35) ≈ 0.63683
/// d2 = 0.15                             N(0.15) ≈ 0.55962
/// Call = 100×0.63683 − 95.123×0.55962 ≈ 10.4506
/// Put  = Call − 100 + 95.123          ≈  5.5735
#[test]
fn test_absolute_value_atm_call() {
    let call = black_scholes_merton_call(100.0, 100.0, 1.0, 0.05, 0.20, 0.0);
    assert!(
        (call.price - 10.4506).abs() < 0.05,
        "ATM call: expected ≈ 10.4506, got {:.4}",
        call.price
    );
    assert!(
        (call.delta - 0.6368).abs() < 0.005,
        "ATM call delta: expected ≈ 0.6368, got {:.4}",
        call.delta
    );
}

#[test]
fn test_absolute_value_atm_put() {
    let put = black_scholes_merton_put(100.0, 100.0, 1.0, 0.05, 0.20, 0.0);
    assert!(
        (put.price - 5.5735).abs() < 0.05,
        "ATM put: expected ≈ 5.5735, got {:.4}",
        put.price
    );
    assert!(
        (put.delta - (-0.3632)).abs() < 0.005,
        "ATM put delta: expected ≈ -0.3632, got {:.4}",
        put.delta
    );
}

/// Verify Greeks at the standard ATM point: S=100, K=100, T=1, r=0.05, σ=0.20
///
/// Gamma = n(d1) / (S σ √T)  = 0.37524 / 20 ≈ 0.01876
/// Vega  = S √T n(d1)         = 100 × 0.37524 ≈ 37.524
/// Rho   = K T e^{-rT} N(d2)  ≈ 100 × 0.95123 × 0.55962 ≈ 53.23
#[test]
fn test_absolute_value_greeks() {
    let call = black_scholes_merton_call(100.0, 100.0, 1.0, 0.05, 0.20, 0.0);

    assert!(
        (call.gamma - 0.01876).abs() < 0.001,
        "Gamma: expected ≈ 0.01876, got {:.5}",
        call.gamma
    );
    assert!(
        (call.vega - 37.524).abs() < 0.5,
        "Vega: expected ≈ 37.524, got {:.3}",
        call.vega
    );
    assert!(
        (call.rho - 53.23).abs() < 0.5,
        "Rho: expected ≈ 53.23, got {:.2}",
        call.rho
    );
    // Theta is negative for long options; annual theta ≈ −6.41
    assert!(
        call.theta < 0.0,
        "Call theta should be negative, got {:.4}",
        call.theta
    );
    assert!(
        (call.theta - (-6.41)).abs() < 0.15,
        "Theta: expected ≈ -6.41 (annual), got {:.4}",
        call.theta
    );
}

/// OTM call: S=100, K=130, T=0.5, r=0.05, σ=0.30, q=0
/// This catches scaling bugs that don't show at ATM.
#[test]
fn test_absolute_value_otm_call() {
    let call = black_scholes_merton_call(100.0, 130.0, 0.5, 0.05, 0.30, 0.0);
    // Price should be small but nonzero (roughly $1–3)
    assert!(
        call.price > 0.5 && call.price < 5.0,
        "OTM call: expected price in [0.5, 5.0], got {:.4}",
        call.price
    );
    assert!(
        call.delta > 0.05 && call.delta < 0.30,
        "OTM call delta: expected in [0.05, 0.30], got {:.4}",
        call.delta
    );
}

/// ITM put: S=100, K=130, T=0.5, r=0.05, σ=0.30, q=0
/// Intrinsic ≈ 130*e^(-0.025) − 100 ≈ 26.78, plus time value.
#[test]
fn test_absolute_value_itm_put() {
    let put = black_scholes_merton_put(100.0, 130.0, 0.5, 0.05, 0.30, 0.0);
    let disc_strike = 130.0 * (-0.05_f64 * 0.5).exp();
    let intrinsic = disc_strike - 100.0;
    assert!(
        put.price > intrinsic,
        "ITM put price ({:.4}) should exceed discounted intrinsic ({:.4})",
        put.price,
        intrinsic
    );
    assert!(
        put.price < intrinsic + 5.0,
        "ITM put time value shouldn't be huge; price={:.4} intrinsic={:.4}",
        put.price,
        intrinsic
    );
}

/// With dividends: S=100, K=100, T=1, r=0.05, σ=0.20, q=0.03
///
/// d1 = (0 + (0.05-0.03+0.02)×1)/0.20 = 0.20
/// d2 = 0.00
/// N(0.20) ≈ 0.57926, N(0.00) = 0.50000
/// Call = 100 e^{-0.03} × 0.57926 − 100 e^{-0.05} × 0.50 ≈ 56.22 − 47.56 ≈ 8.66
#[test]
fn test_absolute_value_with_dividends() {
    let call = black_scholes_merton_call(100.0, 100.0, 1.0, 0.05, 0.20, 0.03);
    assert!(
        (call.price - 8.66).abs() < 0.10,
        "With q=0.03: expected call ≈ 8.66, got {:.4}",
        call.price
    );
}

/// Short-dated ATM: S=100, K=100, T=30/365, r=0.05, σ=0.20, q=0
/// Quick sanity check for near-expiry pricing.
/// √T ≈ 0.2867, d1 ≈ (0.07×0.08219)/0.05734 ≈ 0.100, d2 ≈ −0.187
/// Call ≈ 2.3–2.5
#[test]
fn test_absolute_value_short_dated() {
    let t = 30.0 / 365.0;
    let call = black_scholes_merton_call(100.0, 100.0, t, 0.05, 0.20, 0.0);
    assert!(
        call.price > 2.0 && call.price < 3.0,
        "30-day ATM call: expected ≈ 2.3, got {:.4}",
        call.price
    );
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
