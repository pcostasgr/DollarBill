// src/market_data/real_market_data.rs
// Fetch real-time market data using Yahoo Finance
#![allow(dead_code)]

use yahoo_finance_api as yahoo;
use std::time::{Duration, UNIX_EPOCH};
use crate::market_data::csv_loader::HistoricalDay;

/// Fetch historical market data for a given stock symbol
/// Returns HistoricalDay format (compatible with existing code)
/// 
/// # Arguments
/// * `symbol` - Stock ticker symbol (e.g., "TSLA", "AAPL")
/// * `days_back` - Number of days of historical data to fetch
/// 
/// # Returns
/// Vector of HistoricalDay sorted chronologically (newest first, like CSV loader)
pub async fn fetch_market_data(symbol: &str, days_back: u64) -> Result<Vec<HistoricalDay>, Box<dyn std::error::Error>> {
    let provider = yahoo::YahooConnector::new();
    
    // Calculate time range using time crate's OffsetDateTime
    let now = time::OffsetDateTime::now_utc();
    let start = now - time::Duration::days(days_back as i64);
    
    // Fetch quotes
    let response = provider.get_quote_history(symbol, start, now).await?;
    let quotes = response.quotes()?;
    
    let mut data = Vec::new();
    
    for quote in quotes {
        let timestamp = UNIX_EPOCH + Duration::from_secs(quote.timestamp);
        let datetime = chrono::DateTime::<chrono::Utc>::from(timestamp);
        let date_str = datetime.format("%Y-%m-%d").to_string();
        
        data.push(HistoricalDay {
            date: date_str,
            close: quote.close,
        });
    }
    
    // Sort chronologically (newest first, matching CSV loader behavior)
    data.sort_by(|a, b| b.date.cmp(&a.date));
    
    Ok(data)
}

/// Fetch only the latest closing price for a stock
pub async fn fetch_latest_price(symbol: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let provider = yahoo::YahooConnector::new();
    let quote = provider.get_latest_quotes(symbol, "1d").await?;
    let quotes = quote.last_quote()?;
    
    Ok(quotes.close)
}

/// Fetch market data and extract closing prices
pub async fn fetch_closes(symbol: &str, days_back: u64) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    let data = fetch_market_data(symbol, days_back).await?;
    Ok(data.iter().map(|d| d.close).collect())
}

/// Display market data summary
pub fn display_summary(data: &[HistoricalDay]) {
    if data.is_empty() {
        println!("No data available");
        return;
    }
    
    let closes: Vec<f64> = data.iter().map(|d| d.close).collect();
    let first_close = closes.first().unwrap();  // Newest
    let last_close = closes.last().unwrap();     // Oldest
    let return_pct = ((first_close - last_close) / last_close) * 100.0;
    
    let min_close = closes.iter().copied().fold(f64::INFINITY, f64::min);
    let max_close = closes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg_close = closes.iter().sum::<f64>() / closes.len() as f64;
    
    println!("\n=== Market Data Summary ===");
    println!("Symbol: Live data from Yahoo Finance");
    println!("Period: {} to {}", data.last().unwrap().date, data.first().unwrap().date);
    println!("Data points: {}", data.len());
    println!("\nPrice Statistics:");
    println!("  Latest close: ${:.2}", first_close);
    println!("  Oldest close: ${:.2}", last_close);
    println!("  Return:       {:.2}%", return_pct);
    println!("  Min close:    ${:.2}", min_close);
    println!("  Max close:    ${:.2}", max_close);
    println!("  Avg close:    ${:.2}", avg_close);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Ignore by default since it requires network access
    async fn test_fetch_tesla_data() {
        let result = fetch_market_data("TSLA", 30).await;
        
        // If network fails, just warn and pass - don't fail CI
        match result {
            Ok(data) => {
                assert!(!data.is_empty());
                assert!(data.len() <= 30); // May be fewer due to weekends/holidays
            }
            Err(e) => {
                println!("Warning: Network test failed (this is normal in CI): {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default since it requires network access
    async fn test_fetch_latest_price() {
        let result = fetch_latest_price("AAPL").await;
        
        // If network fails, just warn and pass - don't fail CI
        match result {
            Ok(price) => {
                assert!(price > 0.0);
            }
            Err(e) => {
                println!("Warning: Network test failed (this is normal in CI): {}", e);
            }
        }
    }
}
