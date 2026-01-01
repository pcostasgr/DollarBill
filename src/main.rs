// src/main.rs
mod bs_mod;
mod mock_requests;

use bs_mod::{black_scholes_call, implied_vol_call};
use mock_requests::fetch_mock_call_chain;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let s = 687.00;
    let r = 0.0365;
    let t = 16.0 / 365.25;

    println!("Loading mock SPY call chain (Jan 16 2026)");
    println!("Spot: {:.2}, r: {:.1}%, T ≈ {:.4} years", s, r * 100.0, t);

    let start = Instant::now();
    let chain = fetch_mock_call_chain().await;
    let load_time = start.elapsed();

    println!("Loaded {} mock contracts... well, instantly (mock)", chain.len());

    let calc_start = Instant::now();

    println!("{:<8} {:<12} {:<12} {:<8} {:<10}", "Strike", "Mkt Price", "BS Price", "IV", "Action");

    for contract in chain {
        let iv_result = implied_vol_call(
            contract.market_price,
            s,
            contract.strike,
            t,
            r,
            0.3,
            1e-8,
            100,
        );

        let iv_opt = iv_result.ok();
        let bs_price = iv_opt.map_or(0.0, |iv| black_scholes_call(s, contract.strike, t, r, iv).price);

        let iv_str = iv_opt
            .map(|v| format!("{:.2}%", v * 100.0))
            .unwrap_or("N/A".to_string());

        let action = if let Some(iv) = iv_opt {
            if iv > 0.15 { "SELL" }
            else if iv < 0.12 { "BUY" }
            else { "HOLD" }
        } else {
            "N/A"
        };

        println!(
            "{:<8} {:<12.2} {:<12.2} {:<8} {:<10}",
            contract.strike as i32, contract.market_price, bs_price, iv_str, action
        );
    }

    let calc_time = calc_start.elapsed();
    println!("\nIV smile calculated in {:.6} ms", calc_time.as_secs_f64() * 1000.0);
    println!("Mock load 'time': {:.6} ms (fake async)", load_time.as_secs_f64() * 1000.0);
}