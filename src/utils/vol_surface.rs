#![allow(dead_code)]
// Volatility surface analysis and visualization
// Generates CSV data for volatility surface plotting

use crate::calibration::market_option::{MarketOption, OptionType};
use crate::models::bs_mod::black_scholes_merton_call;
use std::fs::File;
use std::io::Write;

/// Calculate implied volatility using Newton-Raphson method
pub fn implied_volatility_newton(
    market_price: f64,
    spot: f64,
    strike: f64,
    time_to_expiry: f64,
    rate: f64,
    is_call: bool,
) -> Option<f64> {
    let mut sigma = 0.3; // Initial guess
    let tolerance = 1e-6;
    let max_iterations = 100;
    let q = 0.0;
    
    for _ in 0..max_iterations {
        let greeks = if is_call {
            black_scholes_merton_call(spot, strike, time_to_expiry, rate, sigma, q)
        } else {
            crate::models::bs_mod::black_scholes_merton_put(spot, strike, time_to_expiry, rate, sigma, q)
        };
        
        let price_diff = greeks.price - market_price;
        
        if price_diff.abs() < tolerance {
            return Some(sigma);
        }
        
        // Vega check to avoid division by zero
        if greeks.vega.abs() < 1e-10 {
            return None;
        }
        
        // Newton-Raphson update
        sigma = sigma - price_diff / greeks.vega;
        
        // Keep sigma in reasonable range
        if sigma < 0.01 {
            sigma = 0.01;
        } else if sigma > 5.0 {
            sigma = 5.0;
        }
    }
    
    None // Failed to converge
}

#[derive(Debug, Clone)]
pub struct VolSurfacePoint {
    pub strike: f64,
    pub time_to_expiry: f64,
    pub implied_vol: f64,
    pub moneyness: f64, // strike / spot
    pub option_type: String,
    pub volume: i32,
}

/// Extract volatility surface from options data
pub fn extract_vol_surface(
    options: &[MarketOption],
    spot: f64,
    rate: f64,
) -> Vec<VolSurfacePoint> {
    let mut surface_points = Vec::new();
    
    for option in options {
        let market_price = option.mid_price();
        
        if market_price <= 0.0 {
            continue;
        }
        
        let is_call = matches!(option.option_type, OptionType::Call);
        
        if let Some(iv) = implied_volatility_newton(
            market_price,
            spot,
            option.strike,
            option.time_to_expiry,
            rate,
            is_call,
        ) {
            surface_points.push(VolSurfacePoint {
                strike: option.strike,
                time_to_expiry: option.time_to_expiry,
                implied_vol: iv,
                moneyness: option.strike / spot,
                option_type: if is_call { "Call".to_string() } else { "Put".to_string() },
                volume: option.volume,
            });
        }
    }
    
    surface_points
}

/// Save volatility surface to CSV for visualization
pub fn save_vol_surface_csv(
    points: &[VolSurfacePoint],
    symbol: &str,
    filename: &str,
) -> std::io::Result<()> {
    let mut file = File::create(filename)?;
    
    // Write header
    writeln!(file, "Symbol,Strike,TimeToExpiry,ImpliedVol,Moneyness,OptionType,Volume")?;
    
    // Write data
    for point in points {
        writeln!(
            file,
            "{},{},{:.6},{:.4},{:.4},{},{}",
            symbol,
            point.strike,
            point.time_to_expiry,
            point.implied_vol,
            point.moneyness,
            point.option_type,
            point.volume
        )?;
    }
    
    Ok(())
}

/// Print volatility smile (IV vs strike at fixed expiry)
pub fn print_vol_smile(points: &[VolSurfacePoint], symbol: &str) {
    if points.is_empty() {
        println!("No volatility surface data available");
        return;
    }
    
    println!("\n===============================================================");
    println!("📈 VOLATILITY SMILE - {}", symbol);
    println!("===============================================================");
    
    // Group by option type
    let mut calls: Vec<_> = points.iter().filter(|p| p.option_type == "Call").collect();
    let mut puts: Vec<_> = points.iter().filter(|p| p.option_type == "Put").collect();
    
    calls.sort_by(|a, b| a.strike.partial_cmp(&b.strike).unwrap());
    puts.sort_by(|a, b| a.strike.partial_cmp(&b.strike).unwrap());
    
    println!("\nCALLS:");
    println!("{:<10} {:<12} {:<10} {:<10}", "Strike", "Moneyness", "IV %", "Volume");
    println!("{:-<45}", "");
    for point in calls.iter().take(15) {
        println!("{:<10.2} {:<12.4} {:<10.2} {:<10}",
            point.strike,
            point.moneyness,
            point.implied_vol * 100.0,
            point.volume
        );
    }
    
    println!("\nPUTS:");
    println!("{:<10} {:<12} {:<10} {:<10}", "Strike", "Moneyness", "IV %", "Volume");
    println!("{:-<45}", "");
    for point in puts.iter().take(15) {
        println!("{:<10.2} {:<12.4} {:<10.2} {:<10}",
            point.strike,
            point.moneyness,
            point.implied_vol * 100.0,
            point.volume
        );
    }
    
    // Analyze volatility skew
    let atm_calls: Vec<_> = calls.iter().filter(|p| (p.moneyness - 1.0).abs() < 0.05).collect();
    let atm_puts: Vec<_> = puts.iter().filter(|p| (p.moneyness - 1.0).abs() < 0.05).collect();
    
    if !atm_calls.is_empty() && !atm_puts.is_empty() {
        let avg_call_iv: f64 = atm_calls.iter().map(|p| p.implied_vol).sum::<f64>() / atm_calls.len() as f64;
        let avg_put_iv: f64 = atm_puts.iter().map(|p| p.implied_vol).sum::<f64>() / atm_puts.len() as f64;
        
        println!("\n📊 ATM Volatility Analysis:");
        println!("  ATM Call IV:  {:.2}%", avg_call_iv * 100.0);
        println!("  ATM Put IV:   {:.2}%", avg_put_iv * 100.0);
        
        if (avg_put_iv - avg_call_iv).abs() > 0.02 {
            if avg_put_iv > avg_call_iv {
                println!("  ⚠ Put skew detected: Puts trading at {:.1}% premium", 
                    (avg_put_iv - avg_call_iv) * 100.0);
                println!("    Market pricing in downside protection");
            } else {
                println!("  ⚠ Call skew detected: Calls trading at {:.1}% premium",
                    (avg_call_iv - avg_put_iv) * 100.0);
                println!("    Market pricing in upside speculation");
            }
        } else {
            println!("  ✓ Balanced volatility: Call-Put IV difference < 2%");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_implied_vol() {
        // TSLA-like parameters
        let spot = 250.0;
        let strike = 250.0; // ATM
        let time = 30.0 / 365.0;
        let rate = 0.05;
        let market_price = 15.0;
        
        let iv = implied_volatility_newton(market_price, spot, strike, time, rate, true);
        assert!(iv.is_some());
        
        let iv_val = iv.unwrap();
        assert!(iv_val > 0.0 && iv_val < 2.0, "IV should be reasonable");
    }
}
