// Live options chain feed — TTL-cached ATM IV refresh
//
// `LiveIvCache` is used by the trading bot to maintain a current implied-vol
// estimate per symbol.  Every `ttl_secs` seconds the cache is considered
// stale and the next call to `refresh_if_stale` triggers a new Yahoo fetch
// followed by a Newton-Raphson IV solve for near-ATM options.

use crate::market_data::real_market_data::fetch_latest_price;
use crate::market_data::real_option_data_yahoo::fetch_liquid_options;
use crate::calibration::market_option::OptionType;
use crate::utils::vol_surface::implied_volatility_newton;
use log::{info, warn};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ─── TTL-cached IV entry ───────────────────────────────────────────────────

struct IvEntry {
    iv:         f64,
    refreshed:  Instant,
}

// ─── Public cache struct ───────────────────────────────────────────────────

/// Per-symbol ATM implied-vol cache with configurable TTL.
///
/// Typical usage in the live bot:
/// ```no_run
/// # use dollarbill::market_data::options_feed::LiveIvCache;
/// # async fn run() {
/// let mut feed = LiveIvCache::new(900); // 15-minute TTL
/// if let Some(iv) = feed.refresh_if_stale("TSLA", 0.05).await {
///     println!("TSLA live IV: {:.1}%", iv * 100.0);
/// }
/// # }
/// ```
pub struct LiveIvCache {
    /// Seconds before a cached entry is considered stale.
    ttl: Duration,
    entries: HashMap<String, IvEntry>,
}

impl LiveIvCache {
    /// Create a new cache.  `ttl_secs` is the refresh interval in seconds
    /// (e.g. `900` = 15 minutes).
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            ttl:     Duration::from_secs(ttl_secs),
            entries: HashMap::new(),
        }
    }

    /// Return the cached IV for `symbol` if it is still fresh; `None` otherwise.
    pub fn get_cached_iv(&self, symbol: &str) -> Option<f64> {
        self.entries.get(symbol)
            .filter(|e| e.refreshed.elapsed() < self.ttl)
            .map(|e| e.iv)
    }

    /// Return the cached IV regardless of TTL (useful as a fallback).
    pub fn get_stale_iv(&self, symbol: &str) -> Option<f64> {
        self.entries.get(symbol).map(|e| e.iv)
    }

    /// Refresh the IV for `symbol` if the cached entry is missing or stale.
    ///
    /// Returns:
    /// - `Some(iv)` — freshly computed ATM IV (also stored in cache)
    /// - `None`     — fetch/parse failed; stale cached value is preserved
    pub async fn refresh_if_stale(&mut self, symbol: &str, rate: f64) -> Option<f64> {
        if let Some(iv) = self.get_cached_iv(symbol) {
            return Some(iv); // cache is fresh
        }

        info!("Refreshing live IV for {} (TTL expired or first run)", symbol);

        match Self::fetch_atm_iv(symbol, rate).await {
            Ok(iv) => {
                self.entries.insert(symbol.to_string(), IvEntry {
                    iv,
                    refreshed: Instant::now(),
                });
                info!("{} live ATM IV refreshed: {:.1}%", symbol, iv * 100.0);
                Some(iv)
            }
            Err(e) => {
                warn!("Could not fetch live IV for {}: {} — using prior value", symbol, e);
                self.get_stale_iv(symbol)
            }
        }
    }

    // ── Internal fetch ─────────────────────────────────────────────────────

    async fn fetch_atm_iv(
        symbol: &str,
        rate: f64,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch spot and nearest-expiry liquid options concurrently.
        // Map errors to String immediately so they are Send+Sync.
        let (spot_res, opts_res) = tokio::join!(
            fetch_latest_price(symbol),
            fetch_liquid_options(symbol, 0, 10, 25.0),
        );

        let spot    = spot_res.map_err(|e| e.to_string())?;
        let options = opts_res.map_err(|e| e.to_string())?;

        if options.is_empty() {
            return Err(format!("No liquid options returned for {}", symbol).into());
        }

        // Keep near-ATM options: |K/S - 1| ≤ 5%
        let atm_ivs: Vec<f64> = options.iter()
            .filter(|o| (o.strike / spot - 1.0).abs() <= 0.05 && o.time_to_expiry > 0.0)
            .filter_map(|o| {
                let mid   = (o.bid + o.ask) / 2.0;
                let is_call = matches!(o.option_type, OptionType::Call);
                implied_volatility_newton(mid, spot, o.strike, o.time_to_expiry, rate, is_call)
            })
            .filter(|iv| iv.is_finite() && *iv > 0.01 && *iv < 5.0)
            .collect();

        if atm_ivs.is_empty() {
            return Err(format!("No valid ATM IVs found for {}", symbol).into());
        }

        // Use median to suppress outliers
        let mut ivs = atm_ivs.clone();
        ivs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = ivs[ivs.len() / 2];

        Ok(median)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_returns_none_when_empty() {
        let cache = LiveIvCache::new(900);
        assert!(cache.get_cached_iv("TSLA").is_none());
        assert!(cache.get_stale_iv("TSLA").is_none());
    }

    #[test]
    fn stale_entry_preserved_after_ttl() {
        let mut cache = LiveIvCache::new(0); // TTL = 0 → always stale
        // Manually insert an expired entry
        cache.entries.insert("AAPL".to_string(), IvEntry {
            iv:        0.30,
            refreshed: Instant::now(),
        });
        // Fresh cache should return None (TTL=0 means already expired)
        assert!(cache.get_cached_iv("AAPL").is_none());
        // But stale fallback should still return the value
        assert_eq!(cache.get_stale_iv("AAPL"), Some(0.30));
    }

    #[test]
    fn fresh_entry_returned_correctly() {
        let mut cache = LiveIvCache::new(900);
        cache.entries.insert("NVDA".to_string(), IvEntry {
            iv:        0.55,
            refreshed: Instant::now(),
        });
        assert_eq!(cache.get_cached_iv("NVDA"), Some(0.55));
    }
}
