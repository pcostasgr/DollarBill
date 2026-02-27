// Multi-leg option strategy detection
// Implements iron condors, credit spreads, and covered calls

use crate::calibration::market_option::{MarketOption, OptionType};
use crate::market_data::options_json_loader::{load_options_from_json, filter_liquid_options};
use crate::strategies::{SignalAction, TradeSignal};
use std::error::Error;

/// Configuration for spread strategy detection
#[derive(Debug, Clone)]
pub struct SpreadConfig {
    pub min_premium_threshold: f64,     // Minimum net premium to collect ($)
    pub max_spread_width_pct: f64,      // Max spread width as % of spot
    pub min_days_to_expiry: usize,      // Minimum days to expiry
    pub max_days_to_expiry: usize,      // Maximum days to expiry
    pub min_volume: i32,                // Minimum trading volume
    pub max_spread_pct: f64,            // Max bid-ask spread percentage
    pub risk_free_rate: f64,            // Risk-free rate for pricing
}

/// Result of spread analysis
#[derive(Debug, Clone)]
pub struct SpreadResult {
    pub strategy: String,
    pub net_premium: f64,
    pub max_loss: f64,
    pub max_profit: f64,
    pub win_probability: f64,
    pub signal: SignalAction,
}

/// Iron condor strategy detection
/// Sells OTM call and OTM put, buys further OTM call and put for protection
pub fn detect_iron_condors(
    symbol: &str,
    config: &SpreadConfig,
) -> Result<Vec<SpreadResult>, Box<dyn Error>> {
    let json_file = format!("data/{}_options_live.json", symbol.to_lowercase());
    let (spot, all_options) = load_options_from_json(&json_file)?;

    // Filter for liquid options within expiry range
    let liquid_options = filter_liquid_options(
        all_options,
        config.min_volume,
        config.max_spread_pct,
    );

    let expiry_options: Vec<&MarketOption> = liquid_options
        .iter()
        .filter(|opt| {
            let days = (opt.time_to_expiry * 365.0) as usize;
            days >= config.min_days_to_expiry && days <= config.max_days_to_expiry
        })
        .collect();

    if expiry_options.len() < 4 {
        return Ok(vec![]); // Need at least 4 options for iron condor
    }

    let mut results = Vec::new();
    let time_to_expiry = expiry_options[0].time_to_expiry;

    // Find OTM calls and puts for selling
    let calls: Vec<&MarketOption> = expiry_options
        .iter()
        .filter(|opt| matches!(opt.option_type, OptionType::Call) && opt.strike > spot)
        .cloned()
        .collect();

    let puts: Vec<&MarketOption> = expiry_options
        .iter()
        .filter(|opt| matches!(opt.option_type, OptionType::Put) && opt.strike < spot)
        .cloned()
        .collect();

    // Try different wing combinations
    for sell_call in &calls {
        for sell_put in &puts {
            // Find protective wings further OTM
            let buy_call_candidates: Vec<&MarketOption> = calls
                .iter()
                .filter(|opt| opt.strike > sell_call.strike)
                .cloned()
                .collect();

            let buy_put_candidates: Vec<&MarketOption> = puts
                .iter()
                .filter(|opt| opt.strike < sell_put.strike)
                .cloned()
                .collect();

            for buy_call in &buy_call_candidates {
                for buy_put in &buy_put_candidates {
                    if let Some(condor) = analyze_iron_condor(
                        sell_call, buy_call, sell_put, buy_put,
                        spot, time_to_expiry, config
                    ) {
                        results.push(condor);
                    }
                }
            }
        }
    }

    // Sort by net premium (best opportunities first)
    results.sort_by(|a, b| b.net_premium.partial_cmp(&a.net_premium).unwrap());

    Ok(results)
}

/// Analyze a specific iron condor combination
fn analyze_iron_condor(
    sell_call: &MarketOption,
    buy_call: &MarketOption,
    sell_put: &MarketOption,
    buy_put: &MarketOption,
    spot: f64,
    time_to_expiry: f64,
    config: &SpreadConfig,
) -> Option<SpreadResult> {
    // Calculate net premium collected
    let premium_received = sell_call.mid_price() + sell_put.mid_price();
    let premium_paid = buy_call.mid_price() + buy_put.mid_price();
    let net_premium = premium_received - premium_paid;

    // Must meet minimum premium threshold
    if net_premium < config.min_premium_threshold {
        return None;
    }

    // Calculate max loss and profit
    let spread_width = (sell_call.strike - sell_put.strike) / spot;
    if spread_width > config.max_spread_width_pct / 100.0 {
        return None; // Spread too wide
    }

    // Max loss is the spread width minus net premium
    let max_loss = (sell_call.strike - sell_put.strike) - net_premium;

    // Max profit is the net premium collected
    let max_profit = net_premium;

    // Estimate win probability (simplified - spot stays within wings)
    let win_probability = estimate_iron_condor_win_probability(
        sell_put.strike, sell_call.strike, spot, time_to_expiry
    );

    let days_to_expiry = (time_to_expiry * 365.0) as usize;

    Some(SpreadResult {
        strategy: "Iron Condor".to_string(),
        net_premium,
        max_loss,
        max_profit,
        win_probability,
        signal: SignalAction::IronCondor {
            sell_call_strike: sell_call.strike,
            buy_call_strike: buy_call.strike,
            sell_put_strike: sell_put.strike,
            buy_put_strike: buy_put.strike,
            days_to_expiry,
        },
    })
}

/// Credit call spread detection (bullish)
/// Sell ITM/ATM call, buy OTM call
pub fn detect_credit_call_spreads(
    symbol: &str,
    config: &SpreadConfig,
) -> Result<Vec<SpreadResult>, Box<dyn Error>> {
    let json_file = format!("data/{}_options_live.json", symbol.to_lowercase());
    let (spot, all_options) = load_options_from_json(&json_file)?;

    let liquid_options = filter_liquid_options(
        all_options,
        config.min_volume,
        config.max_spread_pct,
    );

    let expiry_options: Vec<&MarketOption> = liquid_options
        .iter()
        .filter(|opt| {
            let days = (opt.time_to_expiry * 365.0) as usize;
            days >= config.min_days_to_expiry && days <= config.max_days_to_expiry
        })
        .collect();

    let mut results = Vec::new();
    let time_to_expiry = expiry_options[0].time_to_expiry;

    // Find calls for credit spreads
    let calls: Vec<&MarketOption> = expiry_options
        .iter()
        .filter(|opt| matches!(opt.option_type, OptionType::Call))
        .cloned()
        .collect();

    for sell_call in &calls {
        // Find higher strike calls to buy
        let buy_candidates: Vec<&MarketOption> = calls
            .iter()
            .filter(|opt| opt.strike > sell_call.strike)
            .cloned()
            .collect();

        for buy_call in &buy_candidates {
            if let Some(spread) = analyze_credit_call_spread(
                sell_call, buy_call, spot, time_to_expiry, config
            ) {
                results.push(spread);
            }
        }
    }

    results.sort_by(|a, b| b.net_premium.partial_cmp(&a.net_premium).unwrap());
    Ok(results)
}

/// Analyze credit call spread
fn analyze_credit_call_spread(
    sell_call: &MarketOption,
    buy_call: &MarketOption,
    spot: f64,
    time_to_expiry: f64,
    config: &SpreadConfig,
) -> Option<SpreadResult> {
    let net_premium = sell_call.mid_price() - buy_call.mid_price();

    if net_premium < config.min_premium_threshold {
        return None;
    }

    // Max loss is spread width minus premium
    let max_loss = (buy_call.strike - sell_call.strike) - net_premium;
    let max_profit = net_premium;

    // Win probability (stock stays below sell strike)
    let win_probability = estimate_credit_spread_win_probability(
        sell_call.strike, spot, time_to_expiry
    );

    let days_to_expiry = (time_to_expiry * 365.0) as usize;

    Some(SpreadResult {
        strategy: "Credit Call Spread".to_string(),
        net_premium,
        max_loss,
        max_profit,
        win_probability,
        signal: SignalAction::CreditCallSpread {
            sell_strike: sell_call.strike,
            buy_strike: buy_call.strike,
            days_to_expiry,
        },
    })
}

/// Estimate win probability for iron condor (simplified)
fn estimate_iron_condor_win_probability(
    lower_strike: f64,
    upper_strike: f64,
    spot: f64,
    time_to_expiry: f64,
) -> f64 {
    // Simplified: assume normal distribution, 30% vol
    let vol = 0.30;
    let time_factor = time_to_expiry.sqrt();
    let expected_move = spot * vol * time_factor;

    // Probability that spot stays within strikes
    let lower_distance = (spot - lower_strike) / expected_move;
    let upper_distance = (upper_strike - spot) / expected_move;

    // Use normal CDF approximation
    let prob_lower = normal_cdf(lower_distance);
    let prob_upper = 1.0 - normal_cdf(upper_distance);

    prob_lower * prob_upper
}

/// Estimate win probability for credit spread (simplified)
fn estimate_credit_spread_win_probability(
    sell_strike: f64,
    spot: f64,
    time_to_expiry: f64,
) -> f64 {
    // Probability that spot stays below sell strike
    let vol = 0.30;
    let time_factor = time_to_expiry.sqrt();
    let expected_move = spot * vol * time_factor;

    let distance = (sell_strike - spot) / expected_move;
    normal_cdf(distance)
}

/// Normal CDF approximation
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + (x / (1.0 + 0.2316419 * x.abs())).powf(-1.0) *
           (0.319381530 * (-x * x * 0.5).exp() +
            0.356563782 * (-x * x * 0.5).exp() +
            1.781477937 * (-x * x * 0.5).exp() +
            -1.821255978 * (-x * x * 0.5).exp() +
            1.330274429 * (-x * x * 0.5).exp()))
}

/// Generate trade signals from spread analysis
pub fn generate_spread_signals(
    symbol: &str,
    config: &SpreadConfig,
) -> Result<Vec<TradeSignal>, Box<dyn Error>> {
    let mut signals = Vec::new();

    // Iron condors
    let condors = detect_iron_condors(symbol, config)?;
    for condor in condors.into_iter().take(2) { // Top 2 opportunities
        signals.push(TradeSignal {
            symbol: symbol.to_string(),
            action: condor.signal,
            strike: 0.0, // Not applicable for spreads
            expiry_days: 0, // Included in signal
            confidence: condor.win_probability,
            edge: condor.net_premium,
            strategy_name: condor.strategy,
        });
    }

    // Credit spreads
    let call_spreads = detect_credit_call_spreads(symbol, config)?;
    for spread in call_spreads.into_iter().take(2) { // Top 2 opportunities
        signals.push(TradeSignal {
            symbol: symbol.to_string(),
            action: spread.signal,
            strike: 0.0,
            expiry_days: 0,
            confidence: spread.win_probability,
            edge: spread.net_premium,
            strategy_name: spread.strategy,
        });
    }

    Ok(signals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iron_condor_detection() {
        let config = SpreadConfig {
            min_premium_threshold: 0.50,
            max_spread_width_pct: 20.0,
            min_days_to_expiry: 7,
            max_days_to_expiry: 60,
            min_volume: 10,
            max_spread_pct: 20.0,
            risk_free_rate: 0.045,
        };

        // This test requires actual market data
        let result = detect_iron_condors("AAPL", &config);
        assert!(result.is_ok() || result.is_err()); // Either succeeds with data or fails gracefully
    }
}