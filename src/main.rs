// src/main.rs
mod bs_mod;
mod mock_requests;

use mock_requests::{fetch_mock_call_chain, test_mock_fetch};

#[tokio::main]
async fn main() {
    println!("Testing mock requests module...");
    test_mock_fetch().await;

    // Full pipeline demo with mock data
    let s = 687.00;
    let r = 0.0365;
    let t = 16.0 / 365.25;

    let chain = fetch_mock_call_chain().await;

    // Reuse our smile logic (adapt for real struct later)
    // For now, just print raw mock data
    println!("\nFull mock chain loaded — ready for real IV computation when you drop the API key.");
}
