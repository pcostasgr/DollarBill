// src/main.rs
// Demo driver with timing for the vol smile computation

mod bs_mod;

use bs_mod::{compute_mock_vol_smile, MockCall};
use std::time::Instant;

fn main() {
    let s = 687.00;
    let r = 0.0365;
    let t = 16.0 / 365.25;

    let mock_chain = vec![
        MockCall { strike: 680.0, market_price: 15.50 },
        MockCall { strike: 685.0, market_price: 11.20 },
        MockCall { strike: 687.0, market_price: 9.80 },
        MockCall { strike: 690.0, market_price: 8.10 },
        MockCall { strike: 695.0, market_price: 5.70 },
        MockCall { strike: 700.0, market_price: 3.90 },
    ];

    println!("Mock SPY Vol Smile — Jan 16 2026 Expiration (T ≈ {:.4} years)", t);
    println!("Spot: {:.2}, r: {:.1}%", s, r * 100.0);

    // Timing starts here
    let start = Instant::now();

    let smile = compute_mock_vol_smile(s, t, r, mock_chain);

    let duration = start.elapsed();

    // Timing ends

    println!("{:<8} {:<12} {:<12} {:<8}", "Strike", "Mkt Price", "BS Price", "IV");

    for (strike, market_price, iv_opt) in smile.iter() {
        let bs_price = iv_opt.map_or(0.0, |iv| {
            bs_mod::black_scholes_call(s, *strike, t, r, iv).price
        });

        let iv_str = iv_opt
            .map(|v| format!("{:.2}%", v * 100.0))
            .unwrap_or("N/A".to_string());

        println!(
            "{:<8} {:<12.2} {:<12.2} {:<8}",
            strike, market_price, bs_price, iv_str
        );
    }

    // Print the timing result
    println!("\nCalculation time: {:.6} milliseconds", duration.as_secs_f64() * 1000.0);
}
