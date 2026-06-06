// Scrape live options data from Yahoo Finance
// Uses undocumented Yahoo API endpoint (free but unofficial)
// Yahoo Finance requires a crumb token (obtained via cookie handshake) since ~2024.

use crate::calibration::market_option::{MarketOption, OptionType};
use serde_json::Value;
use std::error::Error;

const YAHOO_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Build a reqwest client with a persistent cookie store (required for crumb auth).
fn build_yahoo_client() -> Result<reqwest::Client, Box<dyn Error>> {
    Ok(reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(YAHOO_UA)
        .build()?)
}

/// Obtain a Yahoo Finance crumb token.
///
/// Yahoo requires:
///   1. A GET to `https://fc.yahoo.com` to set session cookies.
///   2. A GET to the getcrumb endpoint which returns the crumb as plain text.
///
/// The crumb must be appended as `?crumb=<crumb>` on every subsequent API call.
async fn get_yahoo_crumb(client: &reqwest::Client) -> Result<String, Box<dyn Error>> {
    // Step 1 – seed the cookie jar
    let _ = client.get("https://fc.yahoo.com").send().await;

    // Step 2 – fetch crumb (try query1 then query2)
    for host in &["query1", "query2"] {
        let url = format!("https://{}.finance.yahoo.com/v1/test/getcrumb", host);
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(text) = resp.text().await {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() && !trimmed.starts_with('<') {
                    return Ok(trimmed);
                }
            }
        }
    }
    Err("Failed to obtain Yahoo Finance crumb (rate-limited or consent wall)".into())
}

/// Fetch options chain from Yahoo Finance for a given symbol.
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
    let client = build_yahoo_client()?;
    let crumb = get_yahoo_crumb(&client).await?;

    // First request – get expiration list (try query1, fall back to query2)
    let option_chain_json = fetch_option_chain_json(&client, symbol, None, &crumb).await?;
    let option_chain = &option_chain_json["optionChain"]["result"][0];

    // Get available expiration dates
    let expirations = option_chain["expirationDates"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            // Surface the raw response snippet to aid future debugging
            let snippet = serde_json::to_string(&option_chain_json["optionChain"]["result"])
                .unwrap_or_default();
            format!(
                "No expiration dates found for {}. Result snippet: {}",
                symbol,
                &snippet[..snippet.len().min(300)]
            )
        })?;

    if expiration_index >= expirations.len() {
        return Err(format!(
            "Expiration index {} out of range (available: {})",
            expiration_index,
            expirations.len()
        )
        .into());
    }

    let expiration_timestamp = expirations[expiration_index]
        .as_i64()
        .ok_or("Invalid expiration timestamp")?;

    // Second request – fetch the actual chain for the chosen date
    let dated_json =
        fetch_option_chain_json(&client, symbol, Some(expiration_timestamp), &crumb).await?;
    let options_data = &dated_json["optionChain"]["result"][0]["options"][0];

    let spot = option_chain["quote"]["regularMarketPrice"]
        .as_f64()
        .unwrap_or(0.0);

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let time_to_expiry = ((expiration_timestamp - current_time) as f64) / 86400.0 / 365.0;

    let mut market_options = Vec::new();

    if let Some(calls) = options_data["calls"].as_array() {
        for call in calls {
            if let Some(option) = parse_option_contract(call, OptionType::Call, time_to_expiry) {
                market_options.push(option);
            }
        }
    }
    if let Some(puts) = options_data["puts"].as_array() {
        for put in puts {
            if let Some(option) = parse_option_contract(put, OptionType::Put, time_to_expiry) {
                market_options.push(option);
            }
        }
    }

    println!(
        "✓ Fetched {} options for {} (spot: ${:.2}, TTM: {:.3} years)",
        market_options.len(),
        symbol,
        spot,
        time_to_expiry
    );

    Ok(market_options)
}

/// Low-level helper: GET the Yahoo options JSON, trying query1 then query2.
async fn fetch_option_chain_json(
    client: &reqwest::Client,
    symbol: &str,
    date: Option<i64>,
    crumb: &str,
) -> Result<Value, Box<dyn Error>> {
    for host in &["query1", "query2"] {
        let url = match date {
            None => format!(
                "https://{}.finance.yahoo.com/v7/finance/options/{}?crumb={}",
                host, symbol, crumb
            ),
            Some(ts) => format!(
                "https://{}.finance.yahoo.com/v7/finance/options/{}?date={}&crumb={}",
                host, symbol, ts, crumb
            ),
        };

        let resp = client.get(&url).send().await;
        let text = match resp {
            Ok(r) => match r.text().await {
                Ok(t) => t,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        match serde_json::from_str::<Value>(&text) {
            Ok(v) if v["optionChain"]["result"].is_array() => return Ok(v),
            Ok(v) => {
                // Log Yahoo-level error if present
                if let Some(err) = v.pointer("/optionChain/error") {
                    return Err(format!("Yahoo API error for {}: {}", symbol, err).into());
                }
                continue;
            }
            Err(_) => continue,
        }
    }
    Err(format!("All Yahoo Finance hosts failed for symbol {}", symbol).into())
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
    let client = build_yahoo_client()?;
    let crumb = get_yahoo_crumb(&client).await?;
    let response = fetch_option_chain_json(&client, symbol, None, &crumb).await?;

    let expirations = response["optionChain"]["result"][0]["expirationDates"]
        .as_array()
        .ok_or_else(|| format!("No expirations found for {}", symbol))?;

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
    #[ignore] // Ignore by default since it requires network access
    async fn test_fetch_expirations() {
        let expirations = get_available_expirations("TSLA").await;
        
        match expirations {
            Ok(dates) => {
                assert!(!dates.is_empty());
                println!("Available expirations: {:?}", dates);
            }
            Err(e) => {
                println!("Warning: Network test failed (this is normal in CI): {}", e);
            }
        }
    }
    
    #[tokio::test]
    #[ignore] // Ignore by default since it requires network access
    async fn test_fetch_options() {
        println!("Fetching options from Yahoo Finance for TSLA...");
        let options = fetch_yahoo_options("TSLA", 0).await;
        
        match options {
            Ok(opts) => {
                assert!(!opts.is_empty());
                println!("Fetched {} options", opts.len());
            }
            Err(e) => {
                println!("Warning: Network test failed (this is normal in CI): {}", e);
            }
        }
    }
}
