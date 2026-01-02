// src/main.rs
mod bs_mod;
mod csv_loader;
mod action_table_out;
mod pnl_output;

use csv_loader::load_csv_closes;
use bs_mod::compute_historical_vol;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let csv_file = "tesla_one_year.csv";
    let n_days = 10;

    let start = Instant::now();
    let history = match load_csv_closes(csv_file) {
        Ok(h) => h,
        Err(e) => {
            println!("CSV load failed: {}", e);
            return;
        }
    };
    let load_time = start.elapsed();

    println!("Loaded {} trading days", history.len());

    let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
    let sigma = compute_historical_vol(&closes);
    println!("Historical Volatility: {:.2}%", sigma * 100.0);

    // 1. Action table
    action_table_out::show_action_table(&history, n_days, sigma);

    // 2. P&L post-mortem
    if history.len() >= n_days {
        pnl_output::show_pnl_post_mortem(&history, n_days, sigma);
    }

    println!("CSV load time: {:.6} ms", load_time.as_secs_f64() * 1000.0);
}