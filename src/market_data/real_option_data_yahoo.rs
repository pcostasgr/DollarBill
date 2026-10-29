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
        .timeout(std::time::Duration::from_secs(20))
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
        if let Ok(resp) = client.get(&url).send().await.and_then(|r| r.error_for_status()) {
            if let Ok(text) = resp.text().await {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() && !trimmed.starts_with('<') && !trimmed.contains(char::is_whitespace) && !trimmed.starts_with('{') {
                    return Ok(trimmed);
                }
            }
        }
    }
    Err("Failed to obtain Yahoo Finance crumb (rate-limited or consent wall)".into())
}

/// A chain and its underlying quote from the same Yahoo response.
#[derive(Debug)]
pub struct YahooOptionChain {
    pub spot: f64,
    pub spot_timestamp: i64,
    pub expiration_timestamp: i64,
    pub fetched_at: i64,
    pub options: Vec<MarketOption>,
}

impl YahooOptionChain {
    /// Validate the underlying quote's timestamp, not just HTTP retrieval time.
    pub fn validate_spot_age(&self, now: i64, max_age_secs: i64) -> Result<(), String> {
        let age = now.saturating_sub(self.spot_timestamp);
        if max_age_secs <= 0 || self.spot_timestamp <= 0 || age < -30 || age > max_age_secs {
            return Err(format!("Stale/invalid chain spot: quote_timestamp={}, age={}s, maximum={}s",
                self.spot_timestamp, age, max_age_secs));
        }
        if !self.spot.is_finite() || self.spot <= 0.0 {
            return Err("Invalid chain spot price".into());
        }
        Ok(())
    }
}

/// Select among eligible expiration date markers, sorted and deduplicated.
/// Yahoo dates are not exchange settlement instants. We conservatively use UTC
/// midnight and exclude the current UTC date; no same-day trading is inferred.
fn select_expiration(dates: &Value, index: usize, now: i64, min_remaining_secs: i64) -> Result<i64, String> {
    if min_remaining_secs < 0 { return Err("Negative minimum expiry horizon".into()); }
    let today = chrono::DateTime::from_timestamp(now, 0).ok_or("Invalid current timestamp")?.date_naive();
    let mut eligible: Vec<i64> = dates.as_array().ok_or("Missing expiration list")?.iter()
        .filter_map(Value::as_i64)
        .filter(|ts| *ts > now && ts.saturating_sub(now) >= min_remaining_secs)
        .filter(|ts| chrono::DateTime::from_timestamp(*ts, 0).map(|d| d.date_naive() > today).unwrap_or(false))
        .collect();
    eligible.sort_unstable();
    eligible.dedup();
    eligible.get(index).copied().ok_or_else(|| format!(
        "No eligible expiration at index {index} ({} available; minimum remaining {min_remaining_secs}s)", eligible.len()))
}

fn parse_chain(response: &Value, expiration: i64, now: i64) -> Result<YahooOptionChain, String> {
    let result = &response["optionChain"]["result"][0];
    let chain = &result["options"][0];
    if chain["expirationDate"].as_i64() != Some(expiration) {
        return Err("Chain expiration does not match requested date".into());
    }
    let t = (expiration - now) as f64 / (86400.0 * 365.0);
    if !t.is_finite() || t <= 0.0 { return Err("Expired option chain".into()); }
    let spot = result["quote"]["regularMarketPrice"].as_f64().ok_or("Missing chain spot price")?;
    if !spot.is_finite() || spot <= 0.0 { return Err("Invalid chain spot price".into()); }
    let spot_timestamp = result["quote"]["regularMarketTime"].as_i64().ok_or("Missing chain spot timestamp")?;
    let mut options = Vec::new();
    for (key, kind) in [("calls", OptionType::Call), ("puts", OptionType::Put)] {
        if let Some(contracts) = chain[key].as_array() {
            for contract in contracts {
                if contract["expiration"].as_i64() != Some(expiration) { continue; }
                if let Some(option) = parse_option_contract(contract, kind, t) { options.push(option); }
            }
        }
    }
    Ok(YahooOptionChain { spot, spot_timestamp, expiration_timestamp: expiration, fetched_at: now, options })
}

/// Fetch one eligible chain. Index zero is the nearest eligible expiry, not
/// necessarily the first raw date returned by Yahoo. Minimum horizon is seconds.
pub async fn fetch_yahoo_option_chain(
    symbol: &str, expiration_index: usize, min_remaining_secs: i64,
) -> Result<YahooOptionChain, Box<dyn Error>> {
    let client = build_yahoo_client()?;
    let crumb = get_yahoo_crumb(&client).await?;
    let listing = fetch_option_chain_json(&client, symbol, None, &crumb).await?;
    let now = chrono::Utc::now().timestamp();
    let expiration = select_expiration(&listing["optionChain"]["result"][0]["expirationDates"],
        expiration_index, now, min_remaining_secs).map_err(|e| format!("Yahoo expiry selection for {symbol}: {e}"))?;
    let response = fetch_option_chain_json(&client, symbol, Some(expiration), &crumb).await?;
    let chain = parse_chain(&response, expiration, chrono::Utc::now().timestamp())
        .map_err(|e| format!("Yahoo chain validation for {symbol}: {e}"))?;
    if chain.expiration_timestamp - chain.fetched_at < min_remaining_secs {
        return Err(format!("Yahoo expiry for {symbol} fell below minimum horizon during fetch").into());
    }
    log::debug!("Yahoo chain {}: {} contracts, spot={} quote_timestamp={} expiry={}",
        symbol, chain.options.len(), chain.spot, chain.spot_timestamp, chain.expiration_timestamp);
    Ok(chain)
}

/// Fetch valid future contracts, retaining the existing Vec API for callers.
pub async fn fetch_yahoo_options(symbol: &str, expiration_index: usize) -> Result<Vec<MarketOption>, Box<dyn Error>> {
    Ok(fetch_yahoo_option_chain(symbol, expiration_index, 0).await?.options)
}

/// Low-level helper: GET the Yahoo options JSON, trying query1 then query2.
async fn fetch_option_chain_json(
    client: &reqwest::Client,
    symbol: &str,
    date: Option<i64>,
    crumb: &str,
) -> Result<Value, Box<dyn Error>> {
    let sources = [
        ("query1", format!("https://query1.finance.yahoo.com/v7/finance/options/{symbol}")),
        ("query2", format!("https://query2.finance.yahoo.com/v7/finance/options/{symbol}")),
    ];
    fetch_chain_from_sources(client, symbol, date, crumb, &sources).await
}

async fn fetch_chain_from_sources(
    client: &reqwest::Client, symbol: &str, date: Option<i64>, crumb: &str,
    sources: &[(&str, String)],
) -> Result<Value, Box<dyn Error>> {
    let mut failures = Vec::new();
    for (host, url) in sources {
        let mut request = client.get(url).query(&[("crumb", crumb)]);
        if let Some(ts) = date { request = request.query(&[("date", ts)]); }
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                // Do not log reqwest URLs: the query contains the session crumb.
                failures.push(format!("{host}: {}", if e.is_timeout() { "request timeout" } else { "transport failure" }));
                continue;
            }
        };
        let status = response.status();
        if !status.is_success() {
            failures.push(format!("{host}: HTTP {status}"));
            continue;
        }
        let value: Value = match response.json().await {
            Ok(v) => v,
            Err(_) => { failures.push(format!("{host}: invalid JSON response")); continue; }
        };
        if !value["optionChain"]["error"].is_null() {
            failures.push(format!("{host}: Yahoo API error"));
            continue;
        }
        if value["optionChain"]["result"].as_array().map_or(true, |v| v.is_empty()) {
            failures.push(format!("{host}: empty or missing option chain"));
            continue;
        }
        return Ok(value);
    }
    Err(format!("Yahoo chain fetch for {symbol} failed: {}", failures.join("; ")).into())
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
    let volume = contract["volume"].as_i64().and_then(|v| i32::try_from(v).ok()).unwrap_or(0);
    let open_interest = contract["openInterest"].as_i64().and_then(|v| i32::try_from(v).ok()).unwrap_or(0);
    
    // Skip contracts with no bid/ask
    if ![strike, bid, ask, time_to_expiry].iter().all(|v| v.is_finite())
        || strike <= 0.0 || time_to_expiry <= 0.0 || bid <= 0.0 || ask < bid {
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
    
    fn now() -> i64 { chrono::DateTime::parse_from_rfc3339("2026-10-29T21:00:00Z").unwrap().timestamp() }

    #[test]
    fn skips_past_and_too_short_expiries_without_clamping_maturity() {
        let today = chrono::DateTime::parse_from_rfc3339("2026-10-29T00:00:00Z").unwrap().timestamp();
        let dates = serde_json::json!([today-86400,today,today+86400,today+4*86400,today+4*86400]);
        assert_eq!(select_expiration(&dates,0,now(),0).unwrap(),today+86400);
        assert_eq!(select_expiration(&dates,0,now(),86400).unwrap(),today+4*86400);
        assert!(select_expiration(&dates,1,now(),86400).is_err());
        assert!(select_expiration(&serde_json::json!([today-86400,today]),0,now(),0).is_err());
        assert!(select_expiration(&dates,0,now(),-1).is_err());
    }

    #[test]
    fn chain_requires_matching_expiry_and_uses_dated_response_spot() {
        let exp=now()+7*86400;
        let mut value=serde_json::json!({"optionChain":{"result":[{
            "quote":{"regularMarketPrice":100.0,"regularMarketTime":now()-10},
            "options":[{"expirationDate":exp,"calls":[
                {"expiration":exp,"strike":100.0,"bid":2.0,"ask":2.1,"volume":100},
                {"expiration":exp-86400,"strike":100.0,"bid":2.0,"ask":2.1},
                {"expiration":exp,"strike":100.0,"bid":3.0,"ask":2.1}
            ],"puts":[]}]
        }]}});
        let chain=parse_chain(&value,exp,now()).unwrap();
        assert_eq!(chain.spot,100.0);
        assert_eq!(chain.spot_timestamp,now()-10);
        assert_eq!(chain.options.len(),1);
        assert!((chain.options[0].time_to_expiry-7.0/365.0).abs()<1e-12);
        assert!(parse_chain(&value,exp+86400,now()).is_err());
        assert!(parse_chain(&value,exp,exp+1).is_err());
        value["optionChain"]["result"][0]["quote"]["regularMarketTime"]=Value::Null;
        assert!(parse_chain(&value,exp,now()).is_err());
    }

    #[test]
    fn invalid_quotes_and_expired_inputs_are_rejected() {
        let good=serde_json::json!({"strike":100.0,"bid":2.0,"ask":2.1});
        assert!(parse_option_contract(&good,OptionType::Call,0.1).is_some());
        for t in [0.0,-0.002,f64::NAN] {assert!(parse_option_contract(&good,OptionType::Call,t).is_none());}
        for (key,value) in [("strike",0.0),("bid",0.0),("ask",1.0)] {
            let mut bad=good.clone();bad[key]=serde_json::json!(value);
            assert!(parse_option_contract(&bad,OptionType::Call,0.1).is_none());
        }
    }

    async fn mock_response(status: &str, body: &str) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url=format!("http://{}",listener.local_addr().unwrap());
        let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
        let handle=tokio::spawn(async move {
            let (mut socket,_)=tokio::time::timeout(std::time::Duration::from_secs(5),listener.accept()).await.unwrap().unwrap();
            let mut request=vec![0;4096];socket.read(&mut request).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        (url,handle)
    }

    #[tokio::test]
    async fn http_failure_falls_back_to_second_host() {
        let (one,h1)=mock_response("429 Too Many Requests","{}").await;
        let (two,h2)=mock_response("200 OK",r#"{"optionChain":{"result":[{"ok":true}],"error":null}}"#).await;
        let client=reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(2)).build().unwrap();
        let value=fetch_chain_from_sources(&client,"TEST",Some(now()),"test/session",&[("query1",one),("query2",two)]).await.unwrap();
        assert_eq!(value["optionChain"]["result"][0]["ok"],true);
        h1.await.unwrap();h2.await.unwrap();
    }

    #[tokio::test]
    async fn errors_report_stages_without_exposing_session_crumb() {
        let (one,h1)=mock_response("403 Forbidden","{}").await;
        let (two,h2)=mock_response("200 OK",r#"{"optionChain":{"result":[],"error":null}}"#).await;
        let client=reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(2)).build().unwrap();
        let error=fetch_chain_from_sources(&client,"TEST",None,"private-token",&[("query1",one),("query2",two)]).await.unwrap_err().to_string();
        assert!(error.contains("query1: HTTP 403"));
        assert!(error.contains("query2: empty or missing"));
        assert!(!error.contains("private-token"));
        h1.await.unwrap();h2.await.unwrap();
    }

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
