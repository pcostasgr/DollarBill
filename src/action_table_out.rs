// src/action_table_out.rs
// Shows the Action table (BUY/SELL/HOLD) for last N days using real closes as ATM strikes

use crate::bs_mod::{black_scholes_merton_call, Greeks};

pub fn show_action_table(history: &[crate::csv_loader::HistoricalDay], n_days: usize, sigma: f64) {
    let r = 0.04;
    let t = 30.0 / 365.25;
    let q = 0.0;

    println!("\nAction Table — Hypothetical 30-day ATM Calls (Last {} Days)", n_days);
    println!("{:<20} {:<10} {:<10} {:<8} {:<8}", "Date", "Close", "Call Price", "IV %", "Action");

    for day in history.iter().take(n_days) {
        let s = day.close;
        let k = s.round();
        let greeks: Greeks = black_scholes_merton_call(s, k, t, r, sigma, q);

        let action = if sigma > 0.15 {
            "SELL"
        } else if sigma < 0.12 {
            "BUY"
        } else {
            "HOLD"
        };

        println!(
            "{:<20} {:<10.2} {:<10.2} {:<8.2} {:<8}",
            day.date, s, greeks.price, sigma * 100.0, action
        );
    }
}