// Live options chain feed — TTL-cached ATM IV refresh
//
// `LiveIvCache` is used by the trading bot to maintain a current implied-vol
// estimate per symbol.  Every `ttl_secs` seconds the cache is considered
// stale and the next call to `refresh_if_stale` triggers a new Yahoo fetch
// followed by a bracketed IV solve for near-ATM options.

use crate::market_data::real_option_data_yahoo::{fetch_yahoo_option_chain, YahooOptionChain};
use crate::calibration::market_option::OptionType;
use crate::utils::vol_surface::implied_volatility_newton;
use log::{info, warn};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Live IV inputs are validated independently of cache refresh time.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LiveIvPolicy {
    pub min_remaining_secs: i64,
    pub max_spot_age_secs: i64,
}
impl Default for LiveIvPolicy {
    fn default() -> Self { Self { min_remaining_secs: 86400, max_spot_age_secs: 120 } }
}

#[derive(Clone, Copy)]
struct RetryState { failures: u32, next_attempt: Instant }

/// Reject stale or future-dated spot quotes before deriving market IV. Yahoo
/// does not supply bid/ask update timestamps here; spot freshness alone does
/// not certify option-quote freshness or executability.
fn chain_atm_iv(chain: &YahooOptionChain, rate: f64, policy: LiveIvPolicy, now: i64) -> Result<f64, String> {
    if policy.min_remaining_secs < 0 || policy.max_spot_age_secs <= 0 || !rate.is_finite() {
        return Err("Invalid live IV policy or rate".into());
    }
    chain.validate_spot_age(now, policy.max_spot_age_secs)?;
    if chain.expiration_timestamp <= now || chain.expiration_timestamp.saturating_sub(now) < policy.min_remaining_secs {
        return Err("Chain expiry is below minimum remaining horizon".into());
    }
    let mut ivs = Vec::new();
    let mut liquid = 0;
    let mut near_atm = 0;
    for opt in &chain.options {
        if ![opt.strike,opt.bid,opt.ask,opt.time_to_expiry].iter().all(|v| v.is_finite())
            || opt.strike <= 0.0 || opt.time_to_expiry <= 0.0 || opt.bid <= 0.0 || opt.ask < opt.bid { continue; }
        let mid = opt.mid_price();
        if opt.volume < 10 || (opt.ask-opt.bid)/mid*100.0 > 25.0 { continue; }
        liquid += 1;
        if (opt.strike/chain.spot-1.0).abs() > 0.05 { continue; }
        near_atm += 1;
        if let Some(iv) = implied_volatility_newton(mid,chain.spot,opt.strike,opt.time_to_expiry,rate,opt.option_type==OptionType::Call) {
            if iv.is_finite() && iv > 0.01 && iv < 5.0 { ivs.push(iv); }
        }
    }
    if ivs.is_empty() {
        return Err(format!("No usable ATM IV: {} contracts, {liquid} liquid, {near_atm} near ATM; rejected quotes/bounds/IV range",chain.options.len()));
    }
    ivs.sort_by(f64::total_cmp);
    Ok(ivs[ivs.len()/2])
}

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
    retries: HashMap<String, RetryState>,
}

impl LiveIvCache {
    /// Create a new cache.  `ttl_secs` is the refresh interval in seconds
    /// (e.g. `900` = 15 minutes).
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            ttl:     Duration::from_secs(ttl_secs),
            entries: HashMap::new(),
            retries: HashMap::new(),
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

    /// Insert/overwrite the cached IV for `symbol`, stamped with the current
    /// time. Lets a background task fetch `fetch_atm_iv` *outside* any lock
    /// (the fetch does real network I/O) and only take the lock briefly to
    /// store the result.
    pub fn store(&mut self, symbol: &str, iv: f64) {
        if !iv.is_finite() || iv <= 0.01 || iv >= 5.0 { return; }
        self.retries.remove(symbol);
        self.entries.insert(symbol.to_string(), IvEntry { iv, refreshed: Instant::now() });
    }

    /// Age since the last successful observation, including expired observations.
    pub fn age_secs(&self, symbol: &str) -> Option<u64> {
        self.entries.get(symbol).map(|e| e.refreshed.elapsed().as_secs())
    }

    pub fn can_refresh(&self, symbol: &str) -> bool {
        self.get_cached_iv(symbol).is_none() && self.retries.get(symbol)
            .map_or(true, |r| Instant::now() >= r.next_attempt)
    }

    /// Exponential retry delay: 60s, 120s, 240s, 480s, then capped at 900s.
    pub fn record_failure(&mut self, symbol: &str) -> u64 {
        let failures = self.retries.get(symbol).map_or(1, |r| r.failures.saturating_add(1));
        let delay = (60_u64 << failures.saturating_sub(1).min(4)).min(900);
        self.retries.insert(symbol.into(), RetryState { failures, next_attempt: Instant::now()+Duration::from_secs(delay) });
        delay
    }

    /// Refresh the IV for `symbol` if the cached entry is missing or stale.
    ///
    /// Returns:
    /// - `Some(iv)` — freshly computed ATM IV (also stored in cache)
    /// - `None`     — unavailable or backing off; stale values are retained only for diagnostics
    pub async fn refresh_if_stale(&mut self, symbol: &str, rate: f64) -> Option<f64> {
        if let Some(iv) = self.get_cached_iv(symbol) {
            return Some(iv); // cache is fresh
        }

        if !self.can_refresh(symbol) { return None; }

        info!("Refreshing live IV for {} (TTL expired or first run)", symbol);

        match Self::fetch_atm_iv(symbol, rate).await {
            Ok(iv) => {
                self.store(symbol, iv);
                info!("{} live ATM IV refreshed: {:.1}%", symbol, iv * 100.0);
                Some(iv)
            }
            Err(e) => {
                let delay = self.record_failure(symbol);
                warn!("Live IV unavailable for {}: {}; retry in {}s; prior IV age={:?}s", symbol, e, delay, self.age_secs(symbol));
                None
            }
        }
    }

    // ── ATM-IV fetch ────────────────────────────────────────────────────────
    // `pub` so a background refresh task can call it directly (without `&mut
    // self`) and only lock the cache for the brief `store()` call afterwards.

    pub async fn fetch_atm_iv(
        symbol: &str,
        rate: f64,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        Self::fetch_atm_iv_with_policy(symbol, rate, LiveIvPolicy::default()).await
    }

    pub async fn fetch_atm_iv_with_policy(symbol: &str, rate: f64, policy: LiveIvPolicy)
        -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        if policy.min_remaining_secs < 0 || policy.max_spot_age_secs <= 0 {
            return Err("Invalid live IV policy".into());
        }
        let chain = fetch_yahoo_option_chain(symbol, 0, policy.min_remaining_secs).await
            .map_err(|e| format!("IV chain stage for {symbol}: {e}"))?;
        let iv = chain_atm_iv(&chain, rate, policy, chrono::Utc::now().timestamp())
            .map_err(|e| format!("IV validation stage for {symbol}: {e}"))?;
        info!("{} market IV source=Yahoo-chain, spot_timestamp={}, expiry={}, IV={:.1}%",
            symbol,chain.spot_timestamp,chain.expiration_timestamp,iv*100.0);
        Ok(iv)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_chain() -> YahooOptionChain {
        let now=chrono::Utc::now().timestamp();
        let price=crate::models::bs_mod::black_scholes_call(100.0,100.0,7.0/365.0,0.05,0.3).price;
        YahooOptionChain {spot:100.0,spot_timestamp:now,expiration_timestamp:now+7*86400,fetched_at:now,
            options:vec![crate::calibration::market_option::MarketOption {strike:100.0,bid:price-0.01,ask:price+0.01,
                volume:100,open_interest:100,option_type:OptionType::Call,time_to_expiry:7.0/365.0}]}
    }

    #[test]
    fn computes_iv_from_chain_without_separate_spot_request() {
        let chain=valid_chain();
        assert!((chain_atm_iv(&chain,0.05,LiveIvPolicy::default(),chain.fetched_at).unwrap()-0.3).abs()<1e-8);
    }

    #[test]
    fn stale_missing_future_spot_and_short_expiry_are_rejected() {
        let policy=LiveIvPolicy::default();
        for offset in [-121,31] {
            let mut chain=valid_chain();chain.spot_timestamp=chain.fetched_at+offset;
            assert!(chain_atm_iv(&chain,0.05,policy,chain.fetched_at).unwrap_err().contains("spot"));
        }
        let mut chain=valid_chain();chain.expiration_timestamp=chain.fetched_at+3600;
        assert!(chain_atm_iv(&chain,0.05,policy,chain.fetched_at).is_err());
        chain.expiration_timestamp=chain.fetched_at-1;
        assert!(chain_atm_iv(&chain,0.05,policy,chain.fetched_at).is_err());
    }

    #[test]
    fn diagnostics_distinguish_liquidity_from_iv_bounds_failure() {
        let mut chain=valid_chain();chain.options[0].volume=0;
        assert!(chain_atm_iv(&chain,0.05,LiveIvPolicy::default(),chain.fetched_at).unwrap_err().contains("0 liquid"));
        chain.options[0].volume=100;chain.options[0].bid=101.0;chain.options[0].ask=102.0;
        let error=chain_atm_iv(&chain,0.05,LiveIvPolicy::default(),chain.fetched_at).unwrap_err();
        assert!(error.contains("1 near ATM"));
    }

    #[tokio::test]
    async fn failures_back_off_and_never_return_expired_iv_as_live() {
        let mut cache=LiveIvCache::new(0);
        cache.store("TEST",0.3);
        for delay in [60,120,240,480,900,900] {assert_eq!(cache.record_failure("TEST"),delay);}
        assert!(!cache.can_refresh("TEST"));
        assert_eq!(cache.refresh_if_stale("TEST",0.05).await,None);
        assert_eq!(cache.get_stale_iv("TEST"),Some(0.3));
        assert_eq!(cache.get_cached_iv("TEST"),None);
        cache.retries.get_mut("TEST").unwrap().next_attempt=Instant::now()-Duration::from_secs(1);
        assert!(cache.can_refresh("TEST"));
        cache.store("TEST",0.4);
        assert!(!cache.retries.contains_key("TEST"));
        for invalid in [f64::NAN,0.0,f64::INFINITY] {cache.store("TEST",invalid);}
        assert_eq!(cache.get_stale_iv("TEST"),Some(0.4));
    }

    #[test]
    fn partial_policy_configuration_retains_freshness_defaults() {
        let policy:LiveIvPolicy=serde_json::from_str(r#"{"min_remaining_secs":172800}"#).unwrap();
        assert_eq!(policy.min_remaining_secs,172800);
        assert_eq!(policy.max_spot_age_secs,120);
        let config:crate::config::BotRuntimeConfig=serde_json::from_str("{}").unwrap();
        assert_eq!(config.live_iv_policy.min_remaining_secs,86400);
    }

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
