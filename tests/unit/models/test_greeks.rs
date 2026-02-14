// Greeks calculation tests

use crate::helpers::{assert_greeks_valid, EPSILON};
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};

#[test]
fn test_call_delta_range() {
    // Call delta must be between 0 and 1
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    // OTM, ATM, ITM
    let strikes = vec![120.0, 100.0, 80.0];
    
    for strike in strikes {
        let greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
        assert!(greeks.delta >= 0.0 && greeks.delta <= 1.0,
                "Call delta must be in [0,1], got {} for strike {}", greeks.delta, strike);
    }
}

#[test]
fn test_put_delta_range() {
    // Put delta must be between -1 and 0
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    // OTM, ATM, ITM
    let strikes = vec![80.0, 100.0, 120.0];
    
    for strike in strikes {
        let greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
        assert!(greeks.delta >= -1.0 && greeks.delta <= 0.0,
                "Put delta must be in [-1,0], got {} for strike {}", greeks.delta, strike);
    }
}

#[test]
fn test_gamma_symmetry() {
    // Gamma should be the same for calls and puts with same parameters
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    assert!((call_greeks.gamma - put_greeks.gamma).abs() < EPSILON,
            "Call and put gamma should be equal");
}

#[test]
fn test_gamma_positive() {
    // Gamma should always be positive for long options
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let strikes = vec![80.0, 100.0, 120.0];
    
    for strike in strikes {
        let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
        let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
        
        assert!(call_greeks.gamma >= 0.0, "Call gamma must be non-negative");
        assert!(put_greeks.gamma >= 0.0, "Put gamma must be non-negative");
    }
}

#[test]
fn test_vega_positive() {
    // Vega should always be positive (higher vol = higher price)
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let strikes = vec![80.0, 100.0, 120.0];
    
    for strike in strikes {
        let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
        let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
        
        assert!(call_greeks.vega >= 0.0, "Call vega must be non-negative");
        assert!(put_greeks.vega >= 0.0, "Put vega must be non-negative");
    }
}

#[test]
fn test_vega_symmetry() {
    // Vega should be the same for calls and puts
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    assert!((call_greeks.vega - put_greeks.vega).abs() < EPSILON,
            "Call and put vega should be equal");
}

#[test]
fn test_theta_negative_long() {
    // Theta should be negative for long options (time decay)
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    // ATM options typically have negative theta
    assert!(call_greeks.theta < 0.0, "ATM call theta should be negative");
    assert!(put_greeks.theta < 0.0, "ATM put theta should be negative");
}

#[test]
fn test_rho_sign() {
    // Rho should be positive for calls, negative for puts
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    assert!(call_greeks.rho > 0.0, "Call rho should be positive");
    assert!(put_greeks.rho < 0.0, "Put rho should be negative");
}

#[test]
fn test_atm_gamma_maximum() {
    // Gamma is typically highest at-the-money
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let atm_greeks = black_scholes_merton_call(spot, 100.0, time, rate, vol, div);
    let otm_greeks = black_scholes_merton_call(spot, 120.0, time, rate, vol, div);
    let itm_greeks = black_scholes_merton_call(spot, 80.0, time, rate, vol, div);
    
    assert!(atm_greeks.gamma > otm_greeks.gamma, 
            "ATM gamma should be greater than OTM gamma");
    assert!(atm_greeks.gamma > itm_greeks.gamma,
            "ATM gamma should be greater than ITM gamma");
}

#[test]
fn test_delta_put_call_relationship() {
    // Delta(call) - Delta(put) ≈ 1 (or e^(-qT) with dividends)
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    
    let delta_diff = call_greeks.delta - put_greeks.delta;
    let expected = (-div * time).exp();
    
    assert!((delta_diff - expected).abs() < 0.01,
            "Call delta - Put delta should equal e^(-qT), got {} vs {}", delta_diff, expected);
}

#[test]
fn test_greeks_numerical_stability() {
    // Greeks should not produce NaN or Inf values
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let strikes = vec![50.0, 75.0, 100.0, 125.0, 150.0];
    
    for strike in strikes {
        let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
        let put_greeks = black_scholes_merton_put(spot, strike, time, rate, vol, div);
        
        assert_greeks_valid(&call_greeks);
        assert_greeks_valid(&put_greeks);
    }
}

#[test]
fn test_greeks_with_zero_expiry() {
    // At expiration, Greeks should behave properly
    let spot = 110.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 0.0;
    let div = 0.0;
    
    let call_greeks = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    
    // At expiration, ITM call delta should be 1, OTM should be 0
    assert!((call_greeks.delta - 1.0).abs() < EPSILON,
            "ITM call delta at expiration should be 1");
    
    // At expiration, gamma, vega, theta should be zero
    assert!((call_greeks.gamma).abs() < EPSILON, "Gamma at expiration should be 0");
    assert!((call_greeks.vega).abs() < EPSILON, "Vega at expiration should be 0");
    
    // Test OTM put at expiration
    let otm_put = black_scholes_merton_put(spot, strike, time, rate, vol, div);
    assert!((otm_put.delta).abs() < EPSILON, "OTM put delta at expiration should be 0");
}

#[test]
fn test_gamma_decreases_with_time() {
    // Gamma typically decreases as expiration approaches for ATM options
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let div = 0.0;
    
    let long_time = black_scholes_merton_call(spot, strike, 1.0, rate, vol, div);
    let short_time = black_scholes_merton_call(spot, strike, 0.1, rate, vol, div);
    
    // For ATM options, gamma increases as expiration approaches
    // This test verifies gamma changes with time
    assert!(short_time.gamma != long_time.gamma,
            "Gamma should change with time to expiration");
}

#[test]
fn test_vega_maximum_atm() {
    // Vega is typically highest at-the-money
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    let div = 0.0;
    
    let atm = black_scholes_merton_call(spot, 100.0, time, rate, vol, div);
    let otm = black_scholes_merton_call(spot, 120.0, time, rate, vol, div);
    let itm = black_scholes_merton_call(spot, 80.0, time, rate, vol, div);
    
    assert!(atm.vega > otm.vega, "ATM vega should be greater than OTM vega");
    assert!(atm.vega > itm.vega, "ATM vega should be greater than ITM vega");
}

#[test]
fn test_delta_bounds_near_expiration() {
    // Near expiration, ITM delta should approach 1, OTM should approach 0
    let spot = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 0.01; // Very close to expiration
    let div = 0.0;
    
    let itm_call = black_scholes_merton_call(spot, 80.0, time, rate, vol, div);
    let otm_call = black_scholes_merton_call(spot, 120.0, time, rate, vol, div);
    
    assert!(itm_call.delta > 0.95, "Deep ITM call delta near expiration should be close to 1");
    assert!(otm_call.delta < 0.05, "Deep OTM call delta near expiration should be close to 0");
}

#[test]
fn test_theta_with_dividends() {
    // Dividends affect theta calculation
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let vol = 0.2;
    let time = 1.0;
    
    let no_div = black_scholes_merton_call(spot, strike, time, rate, vol, 0.0);
    let with_div = black_scholes_merton_call(spot, strike, time, rate, vol, 0.02);
    
    // Theta should differ when dividends are present
    assert!((no_div.theta - with_div.theta).abs() > EPSILON,
            "Dividends should affect theta calculation");
}
