// Alpaca API client for paper trading and market data

use std::error::Error;
use std::time::Duration;
use log::{debug, warn, error};
use reqwest::{Client, header, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};
use super::types::*;
use crate::strategies::SignalAction;

const PAPER_TRADING_URL: &str = "https://paper-api.alpaca.markets";
const LIVE_TRADING_URL: &str = "https://api.alpaca.markets";
const MARKET_DATA_URL: &str = "https://data.alpaca.markets";

/// Seconds to wait between order-status polls when confirming a fill.
const FILL_POLL_INTERVAL_SECS: u64 = 2;
/// Maximum number of polls before we give up waiting for a fill.
const FILL_POLL_MAX_ATTEMPTS: u32 = 15;

/// Maximum number of retry attempts for transient HTTP errors.
const MAX_RETRIES: u32 = 3;

/// Returns true for HTTP status codes that are safe to retry.
fn is_transient(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

pub struct AlpacaClient {
    client: Client,
    api_key: String,
    api_secret: String,
    base_url: String,
    data_url: String,
}

impl AlpacaClient {
    /// Create a new Alpaca client for paper trading
    /// 
    /// Get your API keys from: https://app.alpaca.markets/paper/dashboard/overview
    /// 
    /// # Example
    /// ```no_run
    /// use dollarbill::alpaca::AlpacaClient;
    ///
    /// let client = AlpacaClient::new(
    ///     "YOUR_API_KEY".to_string(),
    ///     "YOUR_API_SECRET".to_string(),
    /// );
    /// # let _ = client; // silence unused warning in doctest
    /// ```
    pub fn new(api_key: String, api_secret: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            api_key,
            api_secret,
            base_url: PAPER_TRADING_URL.to_string(),
            data_url: MARKET_DATA_URL.to_string(),
        }
    }

    /// Create client from environment variables.
    ///
    /// Reads `ALPACA_API_KEY` and `ALPACA_API_SECRET`.
    /// Set `APCA_LIVE=1` to connect to the **live** brokerage endpoint
    /// (`https://api.alpaca.markets`) instead of paper trading.
    ///
    /// # Safety
    /// When `APCA_LIVE=1` orders affect **real money**. Triple-check your
    /// keys belong to a live account before setting this variable.
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        let api_key = std::env::var("ALPACA_API_KEY")
            .map_err(|_| "ALPACA_API_KEY environment variable not set")?;
        let api_secret = std::env::var("ALPACA_API_SECRET")
            .map_err(|_| "ALPACA_API_SECRET environment variable not set")?;

        let live = std::env::var("APCA_LIVE").unwrap_or_default() == "1";
        let base_url = if live {
            eprintln!("⚠️  LIVE TRADING MODE — orders affect real money");
            LIVE_TRADING_URL
        } else {
            PAPER_TRADING_URL
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        Ok(Self {
            client,
            api_key,
            api_secret,
            base_url: base_url.to_string(),
            data_url: MARKET_DATA_URL.to_string(),
        })
    }

    fn build_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert("APCA-API-KEY-ID", self.api_key.parse().unwrap());
        headers.insert("APCA-API-SECRET-KEY", self.api_secret.parse().unwrap());
        headers
    }

    async fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, endpoint);
        self.request_with_retry(|| {
            self.client.get(&url).headers(self.build_headers())
        }).await
    }

    async fn post<T: DeserializeOwned>(&self, endpoint: &str, body: &impl serde::Serialize) -> Result<T, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, endpoint);
        // Serialize body once so we can reuse it across retries
        let json_body = serde_json::to_value(body)?;
        self.request_with_retry(|| {
            self.client.post(&url).headers(self.build_headers()).json(&json_body)
        }).await
    }

    async fn delete<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, endpoint);
        self.request_with_retry(|| {
            self.client.delete(&url).headers(self.build_headers())
        }).await
    }

    /// Execute an HTTP request with exponential-backoff retry for transient errors.
    ///
    /// Retries on 429, 502, 503, 504, and network timeouts up to `MAX_RETRIES` times.
    async fn request_with_retry<T, F>(&self, build: F) -> Result<T, Box<dyn Error>>
    where
        T: DeserializeOwned,
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut last_err: Option<Box<dyn Error>> = None;

        for attempt in 0..=MAX_RETRIES {
            let result = build().send().await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let data = response.json().await?;
                        return Ok(data);
                    }
                    if is_transient(status) && attempt < MAX_RETRIES {
                        let error_text = response.text().await.unwrap_or_default();
                        eprintln!("⚠️  Transient API error {} (attempt {}/{}): {}",
                            status, attempt + 1, MAX_RETRIES, error_text);
                        let delay_ms = 500 * 2u64.pow(attempt);
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        last_err = Some(format!("API error {}: {}", status, error_text).into());
                        continue;
                    }
                    let error_text = response.text().await?;
                    return Err(format!("API error {}: {}", status, error_text).into());
                }
                Err(e) => {
                    let is_timeout = e.is_timeout() || e.is_connect();
                    if is_timeout && attempt < MAX_RETRIES {
                        eprintln!("⚠️  Network error (attempt {}/{}): {}",
                            attempt + 1, MAX_RETRIES, e);
                        let delay_ms = 500 * 2u64.pow(attempt);
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        last_err = Some(e.into());
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "Request failed after retries".into()))
    }

    // ============ Account / Clock Methods ============

    /// Get account information
    pub async fn get_account(&self) -> Result<Account, Box<dyn Error>> {
        self.get("/v2/account").await
    }

    /// Get the current market clock (is_open, next_open, next_close).
    pub async fn get_clock(&self) -> Result<Clock, Box<dyn Error>> {
        self.get("/v2/clock").await
    }

    /// Poll an order until it is filled, failed, or canceled, then return it.
    ///
    /// Polls every `FILL_POLL_INTERVAL_SECS` seconds up to `FILL_POLL_MAX_ATTEMPTS`
    /// times. Returns the final order regardless of terminal status so the caller
    /// can decide how to handle partial fills or rejections.
    pub async fn await_order_fill(&self, order_id: &str) -> Result<Order, Box<dyn Error>> {
        for _ in 0..FILL_POLL_MAX_ATTEMPTS {
            let order = self.get_order(order_id).await?;
            match order.status.as_str() {
                "filled" | "canceled" | "expired" | "rejected" | "done_for_day" => {
                    return Ok(order);
                }
                _ => {
                    tokio::time::sleep(Duration::from_secs(FILL_POLL_INTERVAL_SECS)).await;
                }
            }
        }
        // Timeout: cancel the dangling order to avoid an unintended late fill
        let total_wait_secs = FILL_POLL_INTERVAL_SECS * FILL_POLL_MAX_ATTEMPTS as u64;
        eprintln!("⚠️  Order {} not filled after {}s — attempting cancel",
            order_id, total_wait_secs);
        if let Err(e) = self.cancel_order(order_id).await {
            eprintln!("   Cancel failed (order may already be terminal): {}", e);
        }
        Err(format!("Order {} timed out after {}s and was canceled",
            order_id, total_wait_secs).into())
    }

    // ============ Position Methods ============

    /// Get all open positions
    pub async fn get_positions(&self) -> Result<Vec<Position>, Box<dyn Error>> {
        self.get("/v2/positions").await
    }

    /// Get a specific position
    pub async fn get_position(&self, symbol: &str) -> Result<Position, Box<dyn Error>> {
        self.get(&format!("/v2/positions/{}", symbol)).await
    }

    /// Close all positions
    pub async fn close_all_positions(&self) -> Result<Vec<Order>, Box<dyn Error>> {
        self.delete("/v2/positions").await
    }

    /// Close a specific position
    pub async fn close_position(&self, symbol: &str) -> Result<Order, Box<dyn Error>> {
        self.delete(&format!("/v2/positions/{}", symbol)).await
    }

    /// Close every open position (equity or options) whose Alpaca symbol root
    /// matches `underlying`.  Useful for multi-leg strategies where the
    /// individual OCC symbols are not tracked — queries live positions and
    /// closes each matching leg individually.
    ///
    /// Returns one `Result<Order>` per closed leg.
    pub async fn close_positions_for_underlying(
        &self,
        underlying: &str,
    ) -> Vec<Result<Order, Box<dyn Error>>> {
        let positions = match self.get_positions().await {
            Ok(p) => p,
            Err(e) => return vec![Err(e)],
        };
        let mut results = Vec::new();
        for pos in positions {
            // Match equity symbol exactly, or options OCC root (first ≤6 chars trimmed).
            let root = if pos.symbol.len() >= 6 {
                pos.symbol[..6].trim().to_string()
            } else {
                pos.symbol.clone()
            };
            if pos.symbol == underlying || root == underlying {
                results.push(self.close_position(&pos.symbol).await);
            }
        }
        results
    }

    // ============ Order Methods ============

    /// Submit a new order
    /// 
    /// # Example
    /// ```no_run
    /// use dollarbill::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = AlpacaClient::from_env()?;
    ///
    /// let order = OrderRequest {
    ///     symbol: "TSLA".to_string(),
    ///     qty: 10.0,
    ///     side: OrderSide::Buy,
    ///     r#type: OrderType::Market,
    ///     time_in_force: TimeInForce::Day,
    ///     limit_price: None,
    ///     stop_price: None,
    ///     extended_hours: None,
    ///     client_order_id: None,
    /// };
    ///
    /// let result = client.submit_order(&order).await?;
    /// println!("Order submitted: {:?}", result);
    /// Ok(())
    /// # }
    /// ```
    pub async fn submit_order(&self, order: &OrderRequest) -> Result<Order, Box<dyn Error>> {
        self.post("/v2/orders", order).await
    }

    /// Get all orders (optionally filtered by status)
    pub async fn get_orders(&self, status: Option<&str>) -> Result<Vec<Order>, Box<dyn Error>> {
        let endpoint = if let Some(status) = status {
            format!("/v2/orders?status={}", status)
        } else {
            "/v2/orders".to_string()
        };
        self.get(&endpoint).await
    }

    /// Get a specific order by ID
    pub async fn get_order(&self, order_id: &str) -> Result<Order, Box<dyn Error>> {
        self.get(&format!("/v2/orders/{}", order_id)).await
    }

    /// Cancel a specific order
    pub async fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>> {
        let _: serde_json::Value = self.delete(&format!("/v2/orders/{}", order_id)).await?;
        Ok(())
    }

    /// Cancel all orders
    pub async fn cancel_all_orders(&self) -> Result<Vec<Order>, Box<dyn Error>> {
        self.delete("/v2/orders").await
    }

    // ============ Market Data Methods ============

    /// Execute a GET request against the market data URL with retry support.
    async fn data_get<T: DeserializeOwned>(&self, url: &str) -> Result<T, Box<dyn Error>> {
        let url = url.to_string();
        self.request_with_retry(|| {
            self.client.get(&url).headers(self.build_headers())
        }).await
    }

    /// Get latest quote for a symbol
    pub async fn get_latest_quote(&self, symbol: &str) -> Result<Quote, Box<dyn Error>> {
        let url = format!("{}/v2/stocks/{}/quotes/latest", self.data_url, symbol);

        #[derive(Deserialize)]
        struct QuoteResponse {
            quote: Quote,
        }

        let data: QuoteResponse = self.data_get(&url).await?;
        Ok(data.quote)
    }

    /// Get latest trade for a symbol
    pub async fn get_latest_trade(&self, symbol: &str) -> Result<Trade, Box<dyn Error>> {
        let url = format!("{}/v2/stocks/{}/trades/latest", self.data_url, symbol);

        #[derive(Deserialize)]
        struct TradeResponse {
            trade: Trade,
        }

        let data: TradeResponse = self.data_get(&url).await?;
        Ok(data.trade)
    }

    /// Get market snapshot for a symbol (combines latest quote, trade, and bars)
    pub async fn get_snapshot(&self, symbol: &str) -> Result<Snapshot, Box<dyn Error>> {
        let url = format!("{}/v2/stocks/{}/snapshot", self.data_url, symbol);
        self.data_get(&url).await
    }

    /// Get historical bars (OHLCV data)
    pub async fn get_bars(
        &self,
        symbol: &str,
        timeframe: &str,  // "1Min", "5Min", "15Min", "1Hour", "1Day"
        start: &str,       // RFC3339 format: "2021-01-01T00:00:00Z"
        end: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<Bar>, Box<dyn Error>> {
        let mut url = format!(
            "{}/v2/stocks/{}/bars?timeframe={}&start={}&feed=iex",
            self.data_url, symbol, timeframe, start
        );
        
        if let Some(end) = end {
            url.push_str(&format!("&end={}", end));
        }
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
        }

        #[derive(Deserialize)]
        struct BarsResponse {
            bars: Vec<Bar>,
        }

        let data: BarsResponse = self.data_get(&url).await?;
        Ok(data.bars)
    }

    /// Submit a multi-leg options order (iron condor, vertical spread, straddle, etc.).
    ///
    /// Each leg's `symbol` must use OCC format — see [`occ_symbol`] to build it.
    pub async fn submit_options_order(&self, order: &OptionsOrderRequest) -> Result<Order, Box<dyn Error>> {
        self.post("/v2/orders", order).await
    }

    /// Parse an OCC symbol, then query Alpaca's live options chain to find the nearest
    /// actually-listed contract (by expiry closeness then strike distance).
    /// Returns the original symbol unchanged if parsing fails or the API is unavailable.
    pub async fn resolve_single_leg_occ(&self, occ: &str) -> String {
        // OCC format: ROOT__(6) YYMMDD(6) C|P(1) SSSSSSSS(8) = 21 chars
        if occ.len() < 21 {
            return occ.to_string();
        }
        let underlying = occ[..6].trim();
        let yy: u32 = match occ[6..8].parse() { Ok(v) => v, Err(_) => return occ.to_string() };
        let mm: u32 = match occ[8..10].parse() { Ok(v) => v, Err(_) => return occ.to_string() };
        let dd: u32 = match occ[10..12].parse() { Ok(v) => v, Err(_) => return occ.to_string() };
        let is_put = occ.chars().nth(12) == Some('P');
        let strike_raw: f64 = match occ[13..21].parse() { Ok(v) => v, Err(_) => return occ.to_string() };
        let target_strike = strike_raw / 1000.0;

        use chrono::{Duration, NaiveDate};
        #[derive(Deserialize)]
        struct Contract {
            symbol: String,
            strike_price: String,
            expiration_date: String,
        }
        #[derive(Deserialize)]
        struct ContractsResponse {
            option_contracts: Vec<Contract>,
        }

        let expiry = match NaiveDate::from_ymd_opt(2000 + yy as i32, mm, dd) {
            Some(d) => d,
            None => return occ.to_string(),
        };
        let from = expiry - Duration::days(7);
        let to   = expiry + Duration::days(7);
        let opt_type = if is_put { "put" } else { "call" };
        let strike_lo = (target_strike * 0.9).max(0.5);
        let strike_hi = target_strike * 1.1 + 5.0;

        let endpoint = format!(
            "/v2/options/contracts?underlying_symbols={}&type={}&expiration_date_gte={}&expiration_date_lte={}&strike_price_gte={:.2}&strike_price_lte={:.2}&limit=20&status=active",
            underlying, opt_type, from, to, strike_lo, strike_hi
        );

        match self.get::<ContractsResponse>(&endpoint).await {
            Ok(resp) if !resp.option_contracts.is_empty() => {
                let best = resp.option_contracts
                    .into_iter()
                    .min_by(|a, b| {
                        let sa: f64 = a.strike_price.parse().unwrap_or(f64::MAX);
                        let sb: f64 = b.strike_price.parse().unwrap_or(f64::MAX);
                        let da_exp = NaiveDate::parse_from_str(&a.expiration_date, "%Y-%m-%d")
                            .map(|d| (d - expiry).num_days().unsigned_abs())
                            .unwrap_or(999);
                        let db_exp = NaiveDate::parse_from_str(&b.expiration_date, "%Y-%m-%d")
                            .map(|d| (d - expiry).num_days().unsigned_abs())
                            .unwrap_or(999);
                        // Primary: closest expiry; secondary: closest strike
                        let score_a = da_exp * 1000 + ((sa - target_strike).abs() * 10.0) as u64;
                        let score_b = db_exp * 1000 + ((sb - target_strike).abs() * 10.0) as u64;
                        score_a.cmp(&score_b)
                    })
                    .map(|c| c.symbol)
                    .unwrap_or_else(|| occ.to_string());
                if best != occ {
                    debug!("Resolved OCC {} → {} (nearest listed contract)", occ, best);
                }
                best
            }
            Ok(_) => {
                error!("resolve_occ: no contracts returned for {} {} {} strike={} — using generated symbol {}",
                    underlying, expiry, opt_type, target_strike, occ);
                occ.to_string()
            }
            Err(e) => {
                error!("resolve_occ: API call failed for {} ({}): {} — using generated symbol {}", underlying, endpoint, e, occ);
                occ.to_string()
            }
        }
    }

    /// Round a strike price to the nearest standard listed option increment.
    /// Equity options list at $0.50 increments below $5, $1 below $25,
    /// $2.50 below $200, and $5 above $200.
    pub fn round_to_standard_strike(strike: f64) -> f64 {
        let increment = if strike < 5.0 {
            0.50
        } else if strike < 25.0 {
            1.0
        } else if strike <= 500.0 {
            5.0   // Most equity monthly options use $5 strike increments
        } else {
            10.0  // High-price stocks/ETFs (SPY >$500, etc.)
        };
        (strike / increment).round() * increment
    }

    /// Build an OCC option symbol from its components.
    ///
    /// # Arguments
    /// * `underlying` — ticker, e.g. `"AAPL"` (padded to 6 chars)
    /// * `expiry_yy/mm/dd` — two-digit year, month, day of expiry
    /// * `is_call` — `true` for call, `false` for put
    /// * `strike` — strike price in dollars, e.g. `150.0` → `"00150000"`
    pub fn occ_symbol(
        underlying: &str,
        expiry_yy: u32,
        expiry_mm: u32,
        expiry_dd: u32,
        is_call: bool,
        strike: f64,
    ) -> String {
        let strike = Self::round_to_standard_strike(strike);
        format!(
            "{:<6}{:02}{:02}{:02}{}{:08.0}",
            underlying,
            expiry_yy,
            expiry_mm,
            expiry_dd,
            if is_call { 'C' } else { 'P' },
            strike * 1000.0,
        )
    }

    /// Calculate the OCC expiry-date components `(two-digit year, month, day)` from a
    /// days-to-expiry count, rounded forward to the nearest Friday (standard equity
    /// options expiry day).
    ///
    /// # Example
    /// ```
    /// use dollarbill::alpaca::AlpacaClient;
    /// let (yy, mm, dd) = AlpacaClient::expiry_from_dte(30);
    /// // yy, mm, dd are a valid Friday ≥ 30 calendar days from today
    /// # let _ = (yy, mm, dd);
    /// ```
    pub fn expiry_from_dte(dte: usize) -> (u32, u32, u32) {
        use chrono::{Duration, Local, Datelike};
        let target = Local::now().date_naive() + Duration::days(dte as i64);
        // num_days_from_monday: Mon=0, Tue=1, …, Fri=4, Sat=5, Sun=6
        let dow = target.weekday().num_days_from_monday();
        let days_ahead = if dow <= 4 { 4 - dow } else { 11 - dow };
        let expiry = target + Duration::days(days_ahead as i64);
        ((expiry.year() % 100) as u32, expiry.month(), expiry.day())
    }

    /// Convert a [`SignalAction`] into an [`OptionsOrderRequest`] ready to submit to Alpaca.
    ///
    /// # Arguments
    /// * `signal`      — the strategy signal (BuyCall, SellPut, IronCondor, etc.)
    /// * `underlying`  — ticker symbol, e.g. `"AAPL"`
    /// * `contracts`   — number of contracts (1 contract = 100 shares)
    /// * `limit_price` — net debit/credit to pay/receive; `None` for market execution
    ///
    /// # Returns
    /// `Ok(OptionsOrderRequest)` that can be passed directly to [`submit_options_order`].
    /// Returns `Err` for signals that have no options representation (`NoAction`).
    ///
    /// [`submit_options_order`]: Self::submit_options_order
    pub fn signal_to_options_order(
        signal: &SignalAction,
        underlying: &str,
        contracts: u32,
        limit_price: Option<f64>,
    ) -> Result<OptionsOrderRequest, Box<dyn Error>> {
        let legs = Self::signal_to_legs(signal, underlying)?;
        let order_type = if limit_price.is_some() { OrderType::Limit } else { OrderType::Market };
        if legs.len() == 1 {
            // Single-leg: Alpaca requires symbol, qty, side, position_intent
            let (sym, side, intent) = legs.into_iter().next().unwrap();
            Ok(OptionsOrderRequest {
                r#type: order_type,
                time_in_force: TimeInForce::Day,
                symbol: Some(sym),
                qty: Some(contracts.to_string()),
                side: Some(side),
                position_intent: Some(intent.to_string()),
                order_class: None,
                legs: None,
                limit_price,
                client_order_id: None,
            })
        } else {
            // Multi-leg: qty at top level is required (number of strategy units)
            Ok(OptionsOrderRequest {
                r#type: order_type,
                time_in_force: TimeInForce::Day,
                symbol: None,
                qty: Some(contracts.to_string()),
                side: None,
                position_intent: None,
                order_class: Some("mleg".to_string()),
                legs: Some(legs.into_iter().map(|(sym, side, intent)| OptionsLeg {
                    symbol: sym,
                    ratio_qty: contracts,
                    side,
                    position_intent: intent.to_string(),
                }).collect()),
                limit_price,
                client_order_id: None,
            })
        }
    }

    /// Internal: build (occ_symbol, side, position_intent) tuples for each leg.
    fn signal_to_legs(
        signal: &SignalAction,
        underlying: &str,
    ) -> Result<Vec<(String, OrderSide, &'static str)>, Box<dyn Error>> {
        match signal {
            SignalAction::BuyCall { strike, days_to_expiry, .. } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![(Self::occ_symbol(underlying, yy, mm, dd, true, *strike),
                          OrderSide::Buy, "buy_to_open")])
            }
            SignalAction::BuyPut { strike, days_to_expiry, .. } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![(Self::occ_symbol(underlying, yy, mm, dd, false, *strike),
                          OrderSide::Buy, "buy_to_open")])
            }
            SignalAction::SellCall { strike, days_to_expiry, .. } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![(Self::occ_symbol(underlying, yy, mm, dd, true, *strike),
                          OrderSide::Sell, "sell_to_open")])
            }
            SignalAction::SellPut { strike, days_to_expiry, .. } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![(Self::occ_symbol(underlying, yy, mm, dd, false, *strike),
                          OrderSide::Sell, "sell_to_open")])
            }
            SignalAction::CreditCallSpread { sell_strike, buy_strike, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![
                    (Self::occ_symbol(underlying, yy, mm, dd, true, *sell_strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, true, *buy_strike),
                     OrderSide::Buy, "buy_to_open"),
                ])
            }
            SignalAction::CreditPutSpread { sell_strike, buy_strike, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *sell_strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *buy_strike),
                     OrderSide::Buy, "buy_to_open"),
                ])
            }
            SignalAction::IronCondor {
                sell_call_strike, buy_call_strike,
                sell_put_strike,  buy_put_strike,
                days_to_expiry,
            } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![
                    (Self::occ_symbol(underlying, yy, mm, dd, true,  *sell_call_strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, true,  *buy_call_strike),
                     OrderSide::Buy,  "buy_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *sell_put_strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *buy_put_strike),
                     OrderSide::Buy,  "buy_to_open"),
                ])
            }
            SignalAction::CoveredCall { sell_strike, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![(Self::occ_symbol(underlying, yy, mm, dd, true, *sell_strike),
                          OrderSide::Sell, "sell_to_open")])
            }
            SignalAction::SellStraddle { strike, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![
                    (Self::occ_symbol(underlying, yy, mm, dd, true,  *strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *strike),
                     OrderSide::Sell, "sell_to_open"),
                ])
            }
            SignalAction::BuyStraddle { strike, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![
                    (Self::occ_symbol(underlying, yy, mm, dd, true,  *strike),
                     OrderSide::Buy, "buy_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *strike),
                     OrderSide::Buy, "buy_to_open"),
                ])
            }
            SignalAction::IronButterfly { center_strike, wing_width, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![
                    (Self::occ_symbol(underlying, yy, mm, dd, true,  *center_strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, true,  *center_strike + *wing_width),
                     OrderSide::Buy,  "buy_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, *center_strike),
                     OrderSide::Sell, "sell_to_open"),
                    (Self::occ_symbol(underlying, yy, mm, dd, false, (*center_strike - *wing_width).max(0.01)),
                     OrderSide::Buy,  "buy_to_open"),
                ])
            }
            SignalAction::CashSecuredPut { strike, days_to_expiry } => {
                let (yy, mm, dd) = Self::expiry_from_dte(*days_to_expiry);
                Ok(vec![(Self::occ_symbol(underlying, yy, mm, dd, false, *strike),
                          OrderSide::Sell, "sell_to_open")])
            }
            SignalAction::ClosePosition { position_id } => {
                Err(format!("ClosePosition({position_id}) is not an options order signal").into())
            }
            SignalAction::NoAction => {
                Err("NoAction cannot be converted to an order".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]  // Only run when ALPACA_API_KEY and ALPACA_API_SECRET are set
    async fn test_get_account() {
        let client = AlpacaClient::from_env().expect("Failed to create client from env");
        let account = client.get_account().await.expect("Failed to get account");
        assert!(!account.id.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_positions() {
        let client = AlpacaClient::from_env().expect("Failed to create client from env");
        let _positions = client.get_positions().await.expect("Failed to get positions");
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_latest_quote() {
        let client = AlpacaClient::from_env().expect("Failed to create client from env");
        let quote = client.get_latest_quote("TSLA").await.expect("Failed to get quote");
        assert_eq!(quote.symbol, "TSLA");
    }

    #[test]
    fn test_occ_symbol_call() {
        let sym = AlpacaClient::occ_symbol("AAPL", 25, 1, 17, true, 150.0);
        assert_eq!(sym, "AAPL  250117C00150000");
    }

    #[test]
    fn test_occ_symbol_put() {
        let sym = AlpacaClient::occ_symbol("TSLA", 25, 6, 20, false, 200.0);
        assert_eq!(sym, "TSLA  250620P00200000");
    }

    #[test]
    fn test_occ_symbol_long_ticker() {
        // 6-char ticker should not be padded further
        let sym = AlpacaClient::occ_symbol("GOOGL1", 25, 3, 21, true, 175.5);
        assert_eq!(sym, "GOOGL1250321C00175500");
    }

    #[test]
    fn test_occ_symbol_fractional_strike() {
        // $2.50 strike → "00002500"
        let sym = AlpacaClient::occ_symbol("SPY", 25, 12, 19, false, 2.5);
        assert_eq!(sym, "SPY   251219P00002500");
    }

    #[test]
    fn test_expiry_from_dte_range() {
        // Result must be a valid calendar date (month 1–12, day 1–31, year 0–99).
        let (yy, mm, dd) = AlpacaClient::expiry_from_dte(30);
        assert!(yy <= 99, "two-digit year out of range: {}", yy);
        assert!((1..=12).contains(&mm), "month out of range: {}", mm);
        assert!((1..=31).contains(&dd), "day out of range: {}", dd);
    }

    #[test]
    fn test_expiry_from_dte_is_friday() {
        use chrono::{Datelike, NaiveDate};
        // The returned date must always fall on a Friday.
        for dte in [0usize, 1, 7, 14, 30, 45, 60] {
            let (yy, mm, dd) = AlpacaClient::expiry_from_dte(dte);
            let full_year = 2000 + yy as i32;
            let date = NaiveDate::from_ymd_opt(full_year, mm, dd)
                .expect("invalid date returned by expiry_from_dte");
            assert_eq!(
                date.weekday(),
                chrono::Weekday::Fri,
                "dte={} → {}/{}/{} is not a Friday",
                dte, yy, mm, dd
            );
        }
    }

    // ---- signal_to_options_order tests ----------------------------------------

    #[test]
    fn test_signal_to_options_order_buy_call_single_leg() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::BuyCall { strike: 150.0, days_to_expiry: 30, volatility: 0.3 };
        let req = AlpacaClient::signal_to_options_order(&sig, "AAPL", 1, Some(2.50))
            .expect("should succeed");
        // single-leg: top-level symbol/qty/side
        assert_eq!(req.side, Some(OrderSide::Buy));
        let sym = req.symbol.as_deref().unwrap();
        assert!(sym.contains('C'), "call OCC symbol must contain 'C'");
        assert_eq!(req.qty.as_deref(), Some("1"));
        assert_eq!(req.limit_price, Some(2.50));
    }

    #[test]
    fn test_signal_to_options_order_sell_put_single_leg() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::SellPut { strike: 140.0, days_to_expiry: 45, volatility: 0.25 };
        let req = AlpacaClient::signal_to_options_order(&sig, "AAPL", 2, None)
            .expect("should succeed");
        // single-leg: top-level symbol/qty/side
        assert_eq!(req.side, Some(OrderSide::Sell));
        let sym = req.symbol.as_deref().unwrap();
        assert!(sym.contains('P'), "put OCC symbol must contain 'P'");
        assert_eq!(req.qty.as_deref(), Some("2"));
    }

    #[test]
    fn test_signal_to_options_order_credit_call_spread_two_legs() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::CreditCallSpread {
            sell_strike: 155.0,
            buy_strike: 160.0,
            days_to_expiry: 30,
        };
        let req = AlpacaClient::signal_to_options_order(&sig, "AAPL", 1, Some(-1.50))
            .expect("should succeed");
        let legs = req.legs.as_ref().unwrap();
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].side, OrderSide::Sell);
        assert_eq!(legs[1].side, OrderSide::Buy);
        assert_eq!(req.order_class.as_deref(), Some("mleg"));
    }

    #[test]
    fn test_signal_to_options_order_iron_condor_four_legs() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::IronCondor {
            sell_call_strike: 160.0,
            buy_call_strike: 165.0,
            sell_put_strike: 140.0,
            buy_put_strike: 135.0,
            days_to_expiry: 45,
        };
        let req = AlpacaClient::signal_to_options_order(&sig, "SPY", 1, Some(-2.00))
            .expect("should succeed");
        let legs = req.legs.as_ref().unwrap();
        assert_eq!(legs.len(), 4, "iron condor needs exactly 4 legs");
        assert_eq!(req.order_class.as_deref(), Some("mleg"));
        // Leg order: sell call, buy call, sell put, buy put
        assert_eq!(legs[0].side, OrderSide::Sell);
        assert_eq!(legs[1].side, OrderSide::Buy);
        assert_eq!(legs[2].side, OrderSide::Sell);
        assert_eq!(legs[3].side, OrderSide::Buy);
    }

    #[test]
    fn test_signal_to_options_order_no_action_returns_err() {
        use crate::strategies::SignalAction;
        let result = AlpacaClient::signal_to_options_order(
            &SignalAction::NoAction, "AAPL", 1, None,
        );
        assert!(result.is_err(), "NoAction must return Err");
    }

    #[test]
    fn test_signal_to_options_order_covered_call_single_leg() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::CoveredCall { sell_strike: 155.0, days_to_expiry: 14 };
        let req = AlpacaClient::signal_to_options_order(&sig, "AAPL", 1, Some(1.00))
            .expect("should succeed");
        // single-leg: top-level symbol/qty/side
        assert_eq!(req.side, Some(OrderSide::Sell));
        assert_eq!(req.qty.as_deref(), Some("1"));
    }

    // ── New multi-leg variants ─────────────────────────────────────────────────

    #[test]
    fn test_signal_to_options_order_sell_straddle_two_legs() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::SellStraddle { strike: 200.0, days_to_expiry: 30 };
        let req = AlpacaClient::signal_to_options_order(&sig, "TSLA", 1, None)
            .expect("SellStraddle should produce a valid order");
        let legs = req.legs.as_ref().unwrap();
        assert_eq!(legs.len(), 2, "sell straddle needs exactly 2 legs");
        assert_eq!(req.order_class.as_deref(), Some("mleg"));
        // Both legs sell-to-open
        assert!(legs.iter().all(|l| l.side == OrderSide::Sell));
        assert!(legs.iter().all(|l| l.position_intent == "sell_to_open"));
        // One call leg, one put leg
        assert!(legs.iter().any(|l| l.symbol.contains('C')), "must have a call leg");
        assert!(legs.iter().any(|l| l.symbol.contains('P')), "must have a put leg");
        // Both legs reference the same strike (00200000)
        assert!(legs.iter().all(|l| l.symbol.contains("00200000")));
    }

    #[test]
    fn test_signal_to_options_order_buy_straddle_two_legs() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::BuyStraddle { strike: 150.0, days_to_expiry: 21 };
        let req = AlpacaClient::signal_to_options_order(&sig, "AAPL", 1, None)
            .expect("BuyStraddle should produce a valid order");
        let legs = req.legs.as_ref().unwrap();
        assert_eq!(legs.len(), 2, "buy straddle needs exactly 2 legs");
        assert_eq!(req.order_class.as_deref(), Some("mleg"));
        // Both legs buy-to-open
        assert!(legs.iter().all(|l| l.side == OrderSide::Buy));
        assert!(legs.iter().all(|l| l.position_intent == "buy_to_open"));
        // One call, one put
        assert!(legs.iter().any(|l| l.symbol.contains('C')));
        let legs2 = req.legs.as_ref().unwrap();
        assert!(legs2.iter().any(|l| l.symbol.contains('P')));
        // Both at the same strike (00150000)
        assert!(legs2.iter().all(|l| l.symbol.contains("00150000")));
    }

    #[test]
    fn test_signal_to_options_order_iron_butterfly_four_legs() {
        use crate::strategies::SignalAction;
        let sig = SignalAction::IronButterfly {
            center_strike: 400.0,
            wing_width: 10.0,
            days_to_expiry: 14,
        };
        let req = AlpacaClient::signal_to_options_order(&sig, "SPY", 1, Some(-2.00))
            .expect("IronButterfly should produce a valid order");
        let legs = req.legs.as_ref().unwrap();
        assert_eq!(legs.len(), 4, "iron butterfly needs exactly 4 legs");
        assert_eq!(req.order_class.as_deref(), Some("mleg"));
        // Leg order: sell ATM call, buy OTM call, sell ATM put, buy OTM put
        assert_eq!(legs[0].side, OrderSide::Sell); // sell call (ATM)
        assert_eq!(legs[1].side, OrderSide::Buy);  // buy  call (OTM)
        assert_eq!(legs[2].side, OrderSide::Sell); // sell put  (ATM)
        assert_eq!(legs[3].side, OrderSide::Buy);  // buy  put  (OTM)
        // ATM body strikes → 00400000
        assert!(legs[0].symbol.contains("00400000"), "sell-call must be ATM");
        assert!(legs[2].symbol.contains("00400000"), "sell-put must be ATM");
        // OTM wing strikes → 00410000 and 00390000
        assert!(legs[1].symbol.contains("00410000"), "buy-call wing should be center+width");
        assert!(legs[3].symbol.contains("00390000"), "buy-put wing should be center-width");
        // Correct option types
        assert!(legs[0].symbol.contains('C'));
        assert!(legs[1].symbol.contains('C'));
        assert!(legs[2].symbol.contains('P'));
        assert!(legs[3].symbol.contains('P'));
    }

    #[test]
    fn test_signal_to_options_order_cash_secured_put_single_leg() {
        use crate::strategies::SignalAction;
        // Strike = 5% OTM on a $170 stock → 170 * 0.95 = 161.5
        let sig = SignalAction::CashSecuredPut { strike: 161.5, days_to_expiry: 30 };
        let req = AlpacaClient::signal_to_options_order(&sig, "COIN", 1, Some(3.00))
            .expect("CashSecuredPut should produce a valid order");
        // single-leg: top-level symbol/qty/side
        assert_eq!(req.side, Some(OrderSide::Sell));
        let sym = req.symbol.as_deref().unwrap();
        assert!(sym.contains('P'), "must be a put");
        // Strike 161.5 → 00161500
        assert!(sym.contains("00161500"), "OCC strike encoding mismatch");
        assert_eq!(req.limit_price, Some(3.00));
    }
}
