// Scrape live options data from Yahoo Finance
// Uses undocumented Yahoo API endpoint (free but unofficial)

use crate::calibration::market_option::{MarketOption, OptionType};
use serde_json::Value;
use std::error::Error;

/// Fetch options chain from Yahoo Finance for a given symbol
/// 
/// # Arguments
/// * `symbol` - Stock ticker (e.g., "TSLA", "AAPL")
/// * `expiration_index` - Which expiration to fetch (0 = nearest, 1 = next, etc.)
/// 
/// # Returns
/// Vec<MarketOption> containing both calls and puts for the selected expiration
pub async fn fetch_yahoo_options(
    symbol: &str,
    expiration_index: usize,
) -> Result<Vec<MarketOption>, Box<dyn Error>> {
    // Yahoo Finance options endpoint
    let url = format!(
        "https://query1.finance.yahoo.com/v7/finance/options/{}",
        symbol
    );
    
    println!("Fetching options from Yahoo Finance for {}...", symbol);
    
    // Build request with proper headers (Yahoo requires User-Agent)
    let client = reqwest::Client::new();
    let response_text = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await?
        .text()
        .await?;
    
    // Parse JSON response
    let response: Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("JSON parse error: {}. Response: {}", e, &response_text[..200.min(response_text.len())]))?;
    
    // Navigate JSON structure
    let option_chain = &response["optionChain"]["result"][0];
    
    // Get available expiration dates
    let expirations = option_chain["expirationDates"]
        .as_array()
        .ok_or("No expiration dates found")?;
    
    if expiration_index >= expirations.len() {
        return Err(format!(
            "Expiration index {} out of range (available: {})",
            expiration_index,
            expirations.len()
        ).into());
    }
    
    let expiration_timestamp = expirations[expiration_index]
        .as_i64()
        .ok_or("Invalid expiration timestamp")?;
    
    // Fetch specific expiration
    let url_with_date = format!(
        "https://query1.finance.yahoo.com/v7/finance/options/{}?date={}",
        symbol, expiration_timestamp
    );
    
    let response_text = client
        .get(&url_with_date)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await?
        .text()
        .await?;
    
    let response: Value = serde_json::from_str(&response_text)?;
    
    let options_data = &response["optionChain"]["result"][0]["options"][0];
    
    // Get spot price and current time for TTM calculation
    let spot = option_chain["quote"]["regularMarketPrice"]
        .as_f64()
        .unwrap_or(0.0);
    
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    
    let time_to_expiry = ((expiration_timestamp - current_time) as f64) / 86400.0 / 365.0;
    
    let mut market_options = Vec::new();
    
    // Parse calls
    if let Some(calls) = options_data["calls"].as_array() {
        for call in calls {
            if let Some(option) = parse_option_contract(call, OptionType::Call, time_to_expiry) {
                market_options.push(option);
            }
        }
    }
    
    // Parse puts
    if let Some(puts) = options_data["puts"].as_array() {
        for put in puts {
            if let Some(option) = parse_option_contract(put, OptionType::Put, time_to_expiry) {
                market_options.push(option);
            }
        }
    }
    
    println!(
        "✓ Fetched {} options (spot: ${:.2}, TTM: {:.3} years)",
        market_options.len(),
        spot,
        time_to_expiry
    );
    
    Ok(market_options)
}

/// Parse a single option contract from Yahoo JSON
fn parse_option_contract(
    contract: &Value,
    option_type: OptionType,
    time_to_expiry: f64,
) -> Option<MarketOption> {
    let strike = contract["strike"].as_f64()?;
    let bid = contract["bid"].as_f64().unwrap_or(0.0);
    let ask = contract["ask"].as_f64().unwrap_or(0.0);
    let volume = contract["volume"].as_i64().unwrap_or(0) as i32;
    let open_interest = contract["openInterest"].as_i64().unwrap_or(0) as i32;
    
    // Skip contracts with no bid/ask
    if bid <= 0.0 || ask <= 0.0 {
        return None;
    }
    
    Some(MarketOption {
        strike,
        time_to_expiry,
        option_type,
        bid,
        ask,
        volume,
        open_interest,
    })
}

/// Get list of available expiration dates for a symbol
pub async fn get_available_expirations(symbol: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let url = format!(
        "https://query1.finance.yahoo.com/v7/finance/options/{}",
        symbol
    );
    
    let client = reqwest::Client::new();
    let response_text = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await?
        .text()
        .await?;
    
    let response: Value = serde_json::from_str(&response_text)?;
    
    // Debug: print response structure
    if let Some(error) = response.get("finance").and_then(|f| f.get("error")) {
        return Err(format!("Yahoo API error: {:?}", error).into());
    }
    
    let expirations = response["optionChain"]["result"][0]["expirationDates"]
        .as_array()
        .ok_or_else(|| format!("No expirations found. Response structure: {:?}", 
            response.get("optionChain").and_then(|o| o.get("result"))))?;
    
    let dates: Vec<String> = expirations
        .iter()
        .filter_map(|ts| {
            let timestamp = ts.as_i64()?;
            let datetime = chrono::DateTime::from_timestamp(timestamp, 0)?;
            Some(datetime.format("%Y-%m-%d").to_string())
        })
        .collect();
    
    Ok(dates)
}

/// Fetch options and filter by liquidity
pub async fn fetch_liquid_options(
    symbol: &str,
    expiration_index: usize,
    min_volume: i32,
    max_spread_pct: f64,
) -> Result<Vec<MarketOption>, Box<dyn Error>> {
    let mut options = fetch_yahoo_options(symbol, expiration_index).await?;
    
    // Filter by liquidity
    options.retain(|opt| {
        let spread_pct = ((opt.ask - opt.bid) / opt.mid_price()) * 100.0;
        opt.volume >= min_volume && spread_pct <= max_spread_pct
    });
    
    println!(
        "✓ Filtered to {} liquid options (volume >= {}, spread <= {:.1}%)",
        options.len(),
        min_volume,
        max_spread_pct
    );
    
    Ok(options)
}

/// Display options chain summary
pub fn display_options_summary(options: &[MarketOption]) {
    if options.is_empty() {
        println!("No options data");
        return;
    }
    
    let calls: Vec<_> = options.iter().filter(|o| matches!(o.option_type, OptionType::Call)).collect();
    let puts: Vec<_> = options.iter().filter(|o| matches!(o.option_type, OptionType::Put)).collect();
    
    println!("\n=== Options Chain Summary ===");
    println!("Total contracts: {} ({} calls, {} puts)", options.len(), calls.len(), puts.len());
    
    if !calls.is_empty() {
        let strikes: Vec<f64> = calls.iter().map(|o| o.strike).collect();
        println!("\nCalls:");
        println!("  Strike range: ${:.2} - ${:.2}", 
            strikes.iter().copied().fold(f64::INFINITY, f64::min),
            strikes.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        );
        println!("  Avg bid-ask spread: ${:.3}", 
            calls.iter().map(|o| o.spread()).sum::<f64>() / calls.len() as f64
        );
    }
    
    if !puts.is_empty() {
        let strikes: Vec<f64> = puts.iter().map(|o| o.strike).collect();
        println!("\nPuts:");
        println!("  Strike range: ${:.2} - ${:.2}", 
            strikes.iter().copied().fold(f64::INFINITY, f64::min),
            strikes.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        );
        println!("  Avg bid-ask spread: ${:.3}", 
            puts.iter().map(|o| o.spread()).sum::<f64>() / puts.len() as f64
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fetch_expirations() {
        let expirations = get_available_expirations("TSLA").await;
        assert!(expirations.is_ok());
        let dates = expirations.unwrap();
        assert!(!dates.is_empty());
        println!("Available expirations: {:?}", dates);
    }
    
    #[tokio::test]
    async fn test_fetch_options() {
        let options = fetch_yahoo_options("TSLA", 0).await;
        assert!(options.is_ok());
        let opts = options.unwrap();
        assert!(!opts.is_empty());
        println!("Fetched {} options", opts.len());
    }
}
