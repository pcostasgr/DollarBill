// src/mock_requests.rs
// Mock async requests — returns hardcoded realistic SPY call chain
// For testing the full pipeline without API keys or network

use std::vec::Vec;

/// Mock call contract — minimal fields we need for pricing
#[derive(Debug, Clone)]
pub struct MockContract {
    pub strike: f64,
    pub market_price: f64,  // mid or last premium
    pub expiration_date: String,
}

/// Fetch mock call chain for SPY Jan 16 2026
/// Returns realistic short-dated chain around spot ~687
pub async fn fetch_mock_call_chain() -> Vec<MockContract> {
    // Hardcoded realistic data as of Dec 31, 2025 / Jan 01, 2026
    vec![
        MockContract { strike: 680.0, market_price: 15.50, expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 685.0, market_price: 11.20, expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 687.0, market_price: 9.80,  expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 690.0, market_price: 8.10,  expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 695.0, market_price: 5.70,  expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 700.0, market_price: 3.90,  expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 705.0, market_price: 2.45,  expiration_date: "2026-01-16".to_string() },
        MockContract { strike: 710.0, market_price: 1.30,  expiration_date: "2026-01-16".to_string() },
    ]
}

/// Test function to verify mock works
pub async fn test_mock_fetch() {
    let chain = fetch_mock_call_chain().await;
    println!("Mock fetched {} contracts", chain.len());
    for c in chain.iter().take(5) {
        println!("Strike: {} | Premium: {} | Exp: {}", c.strike, c.market_price, c.expiration_date);
    }
}