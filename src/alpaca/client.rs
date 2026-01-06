// Alpaca API client for paper trading and market data

use std::error::Error;
use reqwest::{Client, header};
use serde::{Deserialize, de::DeserializeOwned};
use super::types::*;

const PAPER_TRADING_URL: &str = "https://paper-api.alpaca.markets";
const MARKET_DATA_URL: &str = "https://data.alpaca.markets";

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
    /// use black_scholes_rust::alpaca::AlpacaClient;
    /// 
    /// let client = AlpacaClient::new(
    ///     "YOUR_API_KEY".to_string(),
    ///     "YOUR_API_SECRET".to_string(),
    /// );
    /// ```
    pub fn new(api_key: String, api_secret: String) -> Self {
        let client = Client::new();
        Self {
            client,
            api_key,
            api_secret,
            base_url: PAPER_TRADING_URL.to_string(),
            data_url: MARKET_DATA_URL.to_string(),
        }
    }

    /// Create client from environment variables
    /// Looks for ALPACA_API_KEY and ALPACA_API_SECRET
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        let api_key = std::env::var("ALPACA_API_KEY")
            .map_err(|_| "ALPACA_API_KEY environment variable not set")?;
        let api_secret = std::env::var("ALPACA_API_SECRET")
            .map_err(|_| "ALPACA_API_SECRET environment variable not set")?;
        Ok(Self::new(api_key, api_secret))
    }

    fn build_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert("APCA-API-KEY-ID", self.api_key.parse().unwrap());
        headers.insert("APCA-API-SECRET-KEY", self.api_secret.parse().unwrap());
        headers
    }

    async fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, endpoint);
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        let data = response.json().await?;
        Ok(data)
    }

    async fn post<T: DeserializeOwned>(&self, endpoint: &str, body: &impl serde::Serialize) -> Result<T, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, endpoint);
        let response = self.client
            .post(&url)
            .headers(self.build_headers())
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        let data = response.json().await?;
        Ok(data)
    }

    async fn delete<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, endpoint);
        let response = self.client
            .delete(&url)
            .headers(self.build_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        let data = response.json().await?;
        Ok(data)
    }

    // ============ Account Methods ============

    /// Get account information
    pub async fn get_account(&self) -> Result<Account, Box<dyn Error>> {
        self.get("/v2/account").await
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

    // ============ Order Methods ============

    /// Submit a new order
    /// 
    /// # Example
    /// ```no_run
    /// use black_scholes_rust::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
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
    /// # Ok(())
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

    /// Get latest quote for a symbol
    pub async fn get_latest_quote(&self, symbol: &str) -> Result<Quote, Box<dyn Error>> {
        let url = format!("{}/v2/stocks/{}/quotes/latest", self.data_url, symbol);
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        #[derive(Deserialize)]
        struct QuoteResponse {
            quote: Quote,
        }

        let data: QuoteResponse = response.json().await?;
        Ok(data.quote)
    }

    /// Get latest trade for a symbol
    pub async fn get_latest_trade(&self, symbol: &str) -> Result<Trade, Box<dyn Error>> {
        let url = format!("{}/v2/stocks/{}/trades/latest", self.data_url, symbol);
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        #[derive(Deserialize)]
        struct TradeResponse {
            trade: Trade,
        }

        let data: TradeResponse = response.json().await?;
        Ok(data.trade)
    }

    /// Get market snapshot for a symbol (combines latest quote, trade, and bars)
    pub async fn get_snapshot(&self, symbol: &str) -> Result<Snapshot, Box<dyn Error>> {
        let url = format!("{}/v2/stocks/{}/snapshot", self.data_url, symbol);
        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        let data: Snapshot = response.json().await?;
        Ok(data)
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

        let response = self.client
            .get(&url)
            .headers(self.build_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(format!("API error {}: {}", status, error_text).into());
        }

        #[derive(Deserialize)]
        struct BarsResponse {
            bars: Vec<Bar>,
        }

        let data: BarsResponse = response.json().await?;
        Ok(data.bars)
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
        println!("Account: {:#?}", account);
        assert!(!account.id.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_positions() {
        let client = AlpacaClient::from_env().expect("Failed to create client from env");
        let positions = client.get_positions().await.expect("Failed to get positions");
        println!("Positions: {:#?}", positions);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_latest_quote() {
        let client = AlpacaClient::from_env().expect("Failed to create client from env");
        let quote = client.get_latest_quote("TSLA").await.expect("Failed to get quote");
        println!("Latest TSLA quote: {:#?}", quote);
        assert_eq!(quote.symbol, "TSLA");
    }
}
