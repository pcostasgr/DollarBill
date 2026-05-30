/// Spot price connectors for Heston recalibration.
///
/// # Extending
/// To add a new provider:
/// 1. Add a variant to [`crate::config::SpotPriceSource`] in `src/config.rs`.
/// 2. Add a `fetch_spot_<provider>` async function in this file.
/// 3. Add the corresponding arm to the match in `fetch_spot`.
use std::error::Error;

use crate::config::SpotPriceSource;

/// Fetch the current spot price for `symbol` using the configured `source`.
///
/// `alpaca_fetch` is a fallible async closure that wraps the Alpaca client
/// call.  It is only invoked when `source == SpotPriceSource::Alpaca`, so no
/// credentials are required when using the Yahoo connector.
pub async fn fetch_spot<F, Fut>(
    source: &SpotPriceSource,
    symbol: &str,
    alpaca_fetch: F,
) -> Result<f64, Box<dyn Error>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<f64, Box<dyn Error>>>,
{
    match source {
        SpotPriceSource::Alpaca  => alpaca_fetch().await,
        SpotPriceSource::Yahoo   => fetch_spot_yahoo(symbol).await,
        SpotPriceSource::Finnhub => fetch_spot_finnhub(symbol).await,
    }
}

// ── Yahoo Finance connector ───────────────────────────────────────────────

/// Fetch spot price via Yahoo Finance chart API (free, no credentials).
///
/// Uses the v8 chart endpoint which returns `meta.regularMarketPrice` even
/// outside market hours.
pub async fn fetch_spot_yahoo(
    symbol: &str,
) -> Result<f64, Box<dyn Error>> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d",
        symbol
    );

    let client = reqwest::Client::new();
    let json: serde_json::Value = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .send()
        .await?
        .json()
        .await?;

    let price = json["chart"]["result"][0]["meta"]["regularMarketPrice"]
        .as_f64()
        .ok_or_else(|| -> Box<dyn Error> {
            "Yahoo spot: regularMarketPrice field missing or null".into()
        })?;

    Ok(price)
}

// ── Finnhub connector ────────────────────────────────────────────────────

/// Fetch spot price via Finnhub quote API (free tier, real-time during
/// market hours, 60 calls/min).
///
/// Reads the API key from the `DOLLARBILL_FINNHUB_KEY` environment variable.
pub async fn fetch_spot_finnhub(
    symbol: &str,
) -> Result<f64, Box<dyn Error>> {
    let key = std::env::var("DOLLARBILL_FINNHUB_KEY")
        .map_err(|_| -> Box<dyn Error> {
            "DOLLARBILL_FINNHUB_KEY env var not set".into()
        })?;

    let url = format!(
        "https://finnhub.io/api/v1/quote?symbol={}&token={}",
        symbol, key
    );

    let client = reqwest::Client::new();
    let json: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await?
        .json()
        .await?;

    // Finnhub returns { "c": <current>, "h": ..., "l": ..., "o": ..., "pc": ..., "t": ... }
    let price = json["c"]
        .as_f64()
        .filter(|&p| p > 0.0)
        .ok_or_else(|| -> Box<dyn Error> {
            format!("Finnhub: current price ('c') missing or zero for {}", symbol).into()
        })?;

    Ok(price)
}
