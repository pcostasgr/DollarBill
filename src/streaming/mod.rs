// Alpaca WebSocket streaming for live market data
//
// Connects to Alpaca's real-time data stream (v2 / IEX feed) to receive
// trade ticks and quote updates without polling the REST API.
//
// The IEX feed is free; switch the URL constant to `sip` for the
// consolidated tape (requires a paid market-data subscription).

use std::error::Error;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Feed base URL — IEX tier (free).
const STREAM_URL: &str = "wss://stream.data.alpaca.markets/v2/iex";

// ─── Public data types ────────────────────────────────────────────────────

/// A real-time trade tick from the Alpaca data stream.
#[derive(Debug, Deserialize, Clone)]
pub struct StreamTrade {
    /// Ticker symbol
    #[serde(rename = "S")]
    pub symbol: String,
    /// Trade price
    #[serde(rename = "p")]
    pub price: f64,
    /// Number of shares in this trade
    #[serde(rename = "s")]
    pub size: u64,
    /// RFC-3339 timestamp
    #[serde(rename = "t")]
    pub timestamp: String,
}

/// Live best-bid/ask quote from the data stream.
#[derive(Debug, Deserialize, Clone)]
pub struct StreamQuote {
    #[serde(rename = "S")]
    pub symbol: String,
    #[serde(rename = "ap")]
    pub ask_price: f64,
    #[serde(rename = "as")]
    pub ask_size: u64,
    #[serde(rename = "bp")]
    pub bid_price: f64,
    #[serde(rename = "bs")]
    pub bid_size: u64,
    /// RFC-3339 timestamp
    #[serde(rename = "t")]
    pub timestamp: String,
}

/// Events emitted by [`AlpacaStream::next_event`].
#[derive(Debug)]
pub enum MarketEvent {
    Trade(StreamTrade),
    Quote(StreamQuote),
    /// The stream successfully reconnected after a transient connection drop.
    Reconnected,
    /// The server closed the connection and all reconnection attempts failed.
    Disconnected,
}

// ─── Stream handle ────────────────────────────────────────────────────────

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Authenticated, subscribed Alpaca WebSocket data stream.
///
/// # Example
/// ```no_run
/// use dollarbill::streaming::AlpacaStream;
/// # async fn run() {
/// let mut stream = AlpacaStream::connect("KEY", "SECRET", &["AAPL".to_string()])
///     .await
///     .unwrap();
/// while let Some(event) = stream.next_event().await {
///     println!("{:?}", event);
/// }
/// # }
/// ```
pub struct AlpacaStream {
    write:      futures_util::stream::SplitSink<WsStream, Message>,
    read:       futures_util::stream::SplitStream<WsStream>,
    /// Stored credentials and subscription list used for automatic reconnection.
    api_key:    String,
    api_secret: String,
    symbols:    Vec<String>,
}

impl AlpacaStream {
    /// Connect using explicit credentials.
    pub async fn connect(
        api_key: &str,
        api_secret: &str,
        symbols: &[String],
    ) -> Result<Self, Box<dyn Error>> {
        Self::connect_to(STREAM_URL, api_key, api_secret, symbols).await
    }

    /// Connect using `ALPACA_API_KEY` / `ALPACA_API_SECRET` environment
    /// variables (mirrors the pattern used by [`AlpacaClient::from_env`]).
    pub async fn connect_from_env(
        symbols: &[String],
    ) -> Result<Self, Box<dyn Error>> {
        let key = std::env::var("ALPACA_API_KEY")
            .map_err(|_| "ALPACA_API_KEY env var not set")?;
        let secret = std::env::var("ALPACA_API_SECRET")
            .map_err(|_| "ALPACA_API_SECRET env var not set")?;
        Self::connect(&key, &secret, symbols).await
    }

    async fn connect_to(
        url: &str,
        api_key: &str,
        api_secret: &str,
        symbols: &[String],
    ) -> Result<Self, Box<dyn Error>> {
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        // ── 1. Drain the initial "connected" banner ───────────────────────
        if let Some(msg) = read.next().await {
            let text = msg?.into_text()?;
            if !text.contains("connected") {
                return Err(format!("Unexpected banner from stream: {}", text).into());
            }
        }

        // ── 2. Authenticate ────────────────────────────────────────────────
        let auth = serde_json::json!({
            "action":  "auth",
            "key":     api_key,
            "secret":  api_secret,
        });
        write.send(Message::Text(auth.to_string())).await?;

        if let Some(msg) = read.next().await {
            let text = msg?.into_text()?;
            if !text.contains("authenticated") {
                return Err(format!("Alpaca auth failed: {}", text).into());
            }
        }

        // ── 3. Subscribe ───────────────────────────────────────────────────
        let tickers: Vec<&str> = symbols.iter().map(String::as_str).collect();
        let sub = serde_json::json!({
            "action": "subscribe",
            "trades": tickers,
            "quotes": tickers,
        });
        write.send(Message::Text(sub.to_string())).await?;

        // Drain subscription confirmation (may be empty array — ignore content)
        if let Some(msg) = read.next().await {
            let _ = msg?.into_text()?;
        }

        Ok(Self {
            write,
            read,
            api_key:    api_key.to_string(),
            api_secret: api_secret.to_string(),
            symbols:    symbols.to_vec(),
        })
    }

    /// Receive the next [`MarketEvent`].
    ///
    /// On network drops or server disconnections, automatically reconnects with
    /// exponential backoff (up to 10 attempts, 500 ms → 16 s per attempt).
    /// Returns `Some(MarketEvent::Reconnected)` when the stream is restored.
    /// Returns `Some(MarketEvent::Disconnected)` only after all reconnection
    /// attempts fail. Returns `None` on a clean server-side close.
    pub async fn next_event(&mut self) -> Option<MarketEvent> {
        loop {
            let raw = match self.read.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => {
                    eprintln!("⚠️  Stream error: {} — reconnecting…", e);
                    if self.reconnect_with_backoff().await.is_ok() {
                        return Some(MarketEvent::Reconnected);
                    }
                    return Some(MarketEvent::Disconnected);
                }
                None => {
                    eprintln!("⚠️  Stream closed by server — reconnecting…");
                    if self.reconnect_with_backoff().await.is_ok() {
                        return Some(MarketEvent::Reconnected);
                    }
                    return None;
                }
            };

            let text = match raw.to_text() {
                Ok(t)  => t.to_string(),
                Err(_) => continue,
            };

            // Alpaca sends JSON arrays, e.g. [{"T":"t","S":"AAPL",...}]
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                for item in arr {
                    match item.get("T").and_then(|v| v.as_str()) {
                        Some("t") => {
                            if let Ok(trade) = serde_json::from_value::<StreamTrade>(item) {
                                return Some(MarketEvent::Trade(trade));
                            }
                        }
                        Some("q") => {
                            if let Ok(quote) = serde_json::from_value::<StreamQuote>(item) {
                                return Some(MarketEvent::Quote(quote));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Reconnect to the Alpaca data stream with exponential backoff.
    ///
    /// Attempts up to 10 reconnections: 500 ms → doubling each time, capped at 16 s.
    async fn reconnect_with_backoff(&mut self) -> Result<(), Box<dyn Error>> {
        const MAX_ATTEMPTS: u32  = 10;
        const BASE_DELAY_MS: u64 = 500;
        const MAX_DELAY_MS:  u64 = 16_000;

        let key     = self.api_key.clone();
        let secret  = self.api_secret.clone();
        let symbols = self.symbols.clone();

        let mut delay_ms = BASE_DELAY_MS;
        for attempt in 1..=MAX_ATTEMPTS {
            eprintln!("   Reconnect attempt {}/{} (delay {}ms)…",
                attempt, MAX_ATTEMPTS, delay_ms);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            match Self::connect_to(STREAM_URL, &key, &secret, &symbols).await {
                Ok(new_stream) => {
                    let AlpacaStream { write, read, .. } = new_stream;
                    self.write = write;
                    self.read  = read;
                    eprintln!("✅ Stream reconnected (attempt {})", attempt);
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("   Attempt {} failed: {}", attempt, e);
                    delay_ms = (delay_ms * 2).min(MAX_DELAY_MS);
                }
            }
        }
        Err("Stream: all reconnection attempts exhausted".into())
    }

    /// Gracefully close the WebSocket connection.
    pub async fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.write.close().await?;
        Ok(())
    }
}
