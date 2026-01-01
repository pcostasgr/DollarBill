// src/polygon_requests.rs
// Polygon.io integration — async fetch of option chain snapshots
// Endpoint: https://api.polygon.io/v3/snapshot/options/{underlying}
// Returns premiums (last quote), implied vol, greeks, etc.
// Free tier limited, paid for heavy use — get key at polygon.io

use reqwest;
use serde::Deserialize;
use std::error::Error;

const POLYGON_API_KEY: &str = "YOUR_POLYGON_API_KEY_HERE";  // Replace with real key

#[derive(Debug, Deserialize)]
pub struct GreekData {
    pub delta: Option<f64>,
    pub gamma: Option<f64>,
    pub theta: Option<f64>,
    pub vega: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct LastQuote {
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub midpoint: Option<f64>,  // Often used as premium proxy
}

#[derive(Debug, Deserialize)]
pub struct SnapshotValue {
    pub implied_volatility: Option<f64>,
    pub greeks: Option<GreekData>,
    pub last_quote: Option<LastQuote>,
    // Add more fields if needed (open_interest, etc.)
}

#[derive(Debug, Deserialize)]
pub struct SnapshotContract {
    pub strike_price: f64,
    // pub contract_type: String,  // Filtered to call
}

#[derive(Debug, Deserialize)]
pub struct SnapshotResult {
    pub contract: SnapshotContract,
    pub value: SnapshotValue,
}

#[derive(Debug, Deserialize)]
struct PolygonSnapshotResponse {
    results: Vec<SnapshotResult>,
    status: String,
}

/// Fetch call option chain snapshot for underlying + optional filters
/// Example: underlying = "SPY", expiration_date = "2026-01-16"
pub async fn fetch_polygon_call_snapshot(
    underlying: &str,
    expiration_date: Option<&str>,
) -> Result<Vec<SnapshotResult>, Box<dyn Error>> {
    let mut url = format!(
        "https://api.polygon.io/v3/snapshot/options/{}?contract_type=call&apiKey={}",
        underlying, POLYGON_API_KEY
    );

    if let Some(exp) = expiration_date {
        url.push_str("&expiration_date=");
        url.push_str(exp);
    }

    // Add more filters if needed: &strike_price.gte=680 etc.

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?.json::<PolygonSnapshotResponse>().await?;

    if response.status != "OK" {
        return Err(format!("Polygon API error: {}", response.status).into());
    }

    Ok(response.results)
}

/// Test function — fetch SPY Jan 16 2026 calls and print premiums/IV
pub async fn test_polygon_snapshot() -> Result<(), Box<dyn Error>> {
    let results = fetch_polygon_call_snapshot("SPY", Some("2026-01-16")).await?;

    println!("Fetched {} SPY call snapshots for 2026-01-16", results.len());
    for res in results.iter().take(10) {
        let premium = res.value.last_quote.as_ref()
            .and_then(|q| q.midpoint.or(q.ask).or(q.bid))
            .unwrap_or(0.0);
        let iv = res.value.implied_volatility.unwrap_or(0.0) * 100.0;  // to %
        println!(
            "Strike: {:.2} | Premium: {:.2} | IV: {:.2}%",
            res.contract.strike_price, premium, iv
        );
    }
    Ok(())
}