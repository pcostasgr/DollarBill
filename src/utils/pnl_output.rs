// src/utils/pnl_output.rs
// P&L post-mortem attribution for rolling ATM call over last N days

use crate::models::bs_mod::{black_scholes_merton_call, pnl_attribution};
use crate::market_data::csv_loader::HistoricalDay;

pub fn show_pnl_post_mortem(history: &[HistoricalDay], n_days: usize, sigma: f64) {
    let r = 0.04;
    let q = 0.0;

    println!("\nP&L Post-Mortem — Rolling 30-day ATM Call (Last {} Days)", n_days - 1);
    println!("{:<20} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}", 
             "Date", "Close", "Delta", "Gamma", "Vega", "Theta", "Rho", "Total PNL");

    let mut cumulative = 0.0;
    for i in 1..n_days {
        let prev = &history[i];
        let curr = &history[i - 1];

        let s_prev = prev.close;
        let k = s_prev.round();
        let t = 30.0 / 365.25;

        let greeks = black_scholes_merton_call(s_prev, k, t, r, sigma, q);

        let delta_s = curr.close - s_prev;
        let delta_sigma = 0.0;
        let delta_t = 1.0 / 252.0;
        let delta_r = 0.0;

        let daily_pnl = pnl_attribution(&greeks, delta_s, delta_sigma, delta_t, delta_r);
        cumulative += daily_pnl;

        println!(
            "{:<20} {:<10.2} {:<10.4} {:<10.4} {:<10.4} {:<10.4} {:<10.4} {:<10.4}",
            curr.date, curr.close,
            greeks.delta * delta_s,
            0.5 * greeks.gamma * delta_s.powi(2),
            greeks.vega * delta_sigma,
            greeks.theta * delta_t,
            greeks.rho * delta_r,
            daily_pnl
        );
    }
    println!("\nCumulative P&L: {:.4}", cumulative);
}