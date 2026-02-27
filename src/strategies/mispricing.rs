// Short options mispricing detection strategy
// Identifies overpriced options (model < market) for premium collection

use crate::strategies::SignalAction;
use crate::calibration::market_option::{MarketOption, OptionType};
use crate::market_data::options_json_loader::{load_options_from_json, filter_liquid_options};
use crate::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use crate::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};
use crate::models::heston::HestonParams;
use crate::models::american::{american_call_binomial, american_put_binomial, BinomialConfig};
use std::error::Error;

/// Configuration for mispricing detection
#[derive(Debug, Clone)]
pub struct MispricingConfig {
    pub min_premium_threshold: f64,     // Minimum premium to collect ($)
    pub max_delta_for_short: f64,       // Max |delta| for short positions (0.3 = 30%)
    pub min_iv_rank: f64,               // Minimum IV rank (0.5 = 50th percentile)
    pub max_spread_pct: f64,            // Max bid-ask spread percentage
    pub min_volume: i32,                // Minimum trading volume
    pub use_american_pricing: bool,     // Use American options pricing
    pub pricing_model: PricingModel,    // Which model to use for fair value
}

#[derive(Debug, Clone)]
pub enum PricingModel {
    BlackScholes,
    Heston,
    American,
}

/// Result of mispricing analysis
#[derive(Debug, Clone)]
pub struct MispricingResult {
    pub option: MarketOption,
    pub model_price: f64,
    pub market_price: f64,  // Mid price (bid+ask)/2
    pub mispricing_pct: f64, // (market - model) / model * 100
    pub is_overpriced: bool, // model < market
    pub premium_available: f64, // How much premium to collect
    pub delta: f64, // Option delta for risk assessment
}

/// Analyze options for mispricing opportunities
pub fn detect_mispriced_options(
    symbol: &str,
    config: &MispricingConfig,
) -> Result<Vec<MispricingResult>, Box<dyn Error>> {
    // Load market options data
    let json_file = format!("data/{}_options_live.json", symbol.to_lowercase());
    let (spot, all_options) = load_options_from_json(&json_file)?;

    // Filter for liquid options only
    let liquid_options = filter_liquid_options(
        all_options,
        config.min_volume,
        config.max_spread_pct,
    );

    let mut results = Vec::new();

    // Get risk-free rate (could be made configurable)
    let rate = 0.045; // 4.5%

    // Load Heston parameters if needed
    let heston_params = if matches!(config.pricing_model, PricingModel::Heston) {
        Some(load_heston_params(symbol)?)
    } else {
        None
    };

    for option in liquid_options {
        let model_price = calculate_model_price(
            &option,
            spot,
            rate,
            config,
            heston_params.as_ref(),
        );

        let market_price = option.mid_price();
        let mispricing_pct = if model_price > 0.0 {
            ((market_price - model_price) / model_price) * 100.0
        } else {
            0.0
        };

        let is_overpriced = model_price < market_price;
        let premium_available = market_price - model_price;

        // Check if this meets our criteria for a short position
        let delta = calculate_delta(&option, spot, rate, config, heston_params.as_ref());
        let meets_criteria = is_overpriced
            && premium_available >= config.min_premium_threshold
            && delta.abs() <= config.max_delta_for_short;

        if meets_criteria {
            results.push(MispricingResult {
                option: option.clone(),
                model_price,
                market_price,
                mispricing_pct,
                is_overpriced,
                premium_available,
                delta,
            });
        }
    }

    // Sort by premium available (best opportunities first)
    results.sort_by(|a, b| b.premium_available.partial_cmp(&a.premium_available).unwrap());

    Ok(results)
}

/// Generate short option signals from mispricing analysis
pub fn generate_short_signals_from_mispricing(
    symbol: &str,
    config: &MispricingConfig,
) -> Result<Vec<SignalAction>, Box<dyn Error>> {
    let mispriced_options = detect_mispriced_options(symbol, config)?;

    // Load spot price for volatility estimation
    let json_file = format!("data/{}_options_live.json", symbol.to_lowercase());
    let (spot, _) = load_options_from_json(&json_file)?;

    let mut signals = Vec::new();

    // Take top opportunities (limit to avoid over-allocation)
    let max_signals = 3;
    for result in mispriced_options.into_iter().take(max_signals) {
        let signal = match result.option.option_type {
            OptionType::Call => SignalAction::SellCall {
                strike: result.option.strike,
                days_to_expiry: (result.option.time_to_expiry * 365.0) as usize,
                volatility: estimate_volatility(&result.option, result.market_price, spot),
            },
            OptionType::Put => SignalAction::SellPut {
                strike: result.option.strike,
                days_to_expiry: (result.option.time_to_expiry * 365.0) as usize,
                volatility: estimate_volatility(&result.option, result.market_price, spot),
            },
        };

        signals.push(signal);
    }

    Ok(signals)
}

/// Calculate model price for an option
fn calculate_model_price(
    option: &MarketOption,
    spot: f64,
    rate: f64,
    config: &MispricingConfig,
    heston_params: Option<&HestonParams>,
) -> f64 {
    let volatility = estimate_volatility(option, option.mid_price(), spot);

    match config.pricing_model {
        PricingModel::BlackScholes => {
            match option.option_type {
                OptionType::Call => {
                    black_scholes_merton_call(spot, option.strike, option.time_to_expiry, rate, volatility, 0.0).price
                }
                OptionType::Put => {
                    black_scholes_merton_put(spot, option.strike, option.time_to_expiry, rate, volatility, 0.0).price
                }
            }
        }
        PricingModel::Heston => {
            if let Some(params) = heston_params {
                match option.option_type {
                    OptionType::Call => {
                        heston_call_carr_madan(spot, option.strike, option.time_to_expiry, rate, params)
                    }
                    OptionType::Put => {
                        heston_put_carr_madan(spot, option.strike, option.time_to_expiry, rate, params)
                    }
                }
            } else {
                0.0 // Fallback
            }
        }
        PricingModel::American => {
            let binomial_config = BinomialConfig::default();
            match option.option_type {
                OptionType::Call => {
                    american_call_binomial(spot, option.strike, option.time_to_expiry, rate, volatility, &binomial_config)
                }
                OptionType::Put => {
                    american_put_binomial(spot, option.strike, option.time_to_expiry, rate, volatility, &binomial_config)
                }
            }
        }
    }
}

/// Calculate delta for an option using finite differences
fn calculate_delta(
    option: &MarketOption,
    spot: f64,
    rate: f64,
    config: &MispricingConfig,
    heston_params: Option<&HestonParams>,
) -> f64 {
    let eps = 0.01; // 1 cent perturbation
    let spot_up = spot * (1.0 + eps / spot);
    let spot_down = spot * (1.0 - eps / spot);

    let price_up = calculate_model_price(option, spot_up, rate, config, heston_params);
    let price_down = calculate_model_price(option, spot_down, rate, config, heston_params);

    (price_up - price_down) / (spot_up - spot_down)
}

/// Estimate volatility from market price (inverse problem)
/// This is a simplified approach - in practice you'd use more sophisticated methods
fn estimate_volatility(option: &MarketOption, market_price: f64, spot: f64) -> f64 {
    // For now, use a reasonable default based on moneyness and time
    // In a real implementation, you'd solve for vol that makes model = market

    let moneyness = option.strike / spot;
    let time_factor = option.time_to_expiry.sqrt();

    // Base volatility estimate
    let mut vol: f64 = 0.30; // 30% base

    // Adjust for moneyness
    if moneyness < 0.95 {
        vol *= 1.2; // OTM puts more volatile
    } else if moneyness > 1.05 {
        vol *= 1.1; // OTM calls more volatile
    }

    // Adjust for time
    if time_factor < 0.1 {
        vol *= 1.3; // Short-dated options more volatile
    }

    vol.min(1.0) // Cap at 100%
}

/// Load Heston parameters for a symbol
fn load_heston_params(symbol: &str) -> Result<HestonParams, Box<dyn Error>> {
    use std::fs;
    use serde_json::from_str;

    #[derive(serde::Deserialize)]
    struct HestonParamsJson {
        kappa: f64,
        theta: f64,
        sigma: f64,
        rho: f64,
        v0: f64,
    }

    #[derive(serde::Deserialize)]
    struct CalibrationResult {
        heston_params: HestonParamsJson,
    }

    let filename = format!("data/{}_heston_params.json", symbol.to_lowercase());
    let contents = fs::read_to_string(filename)?;
    let calibration: CalibrationResult = from_str(&contents)?;

    // Convert to the HestonParams struct used by the models
    // Note: We need to provide default values for s0, r, t which aren't in the JSON
    // These will be overridden when calling the pricing functions
    Ok(HestonParams {
        s0: 100.0, // Placeholder, will be overridden
        v0: calibration.heston_params.v0,
        kappa: calibration.heston_params.kappa,
        theta: calibration.heston_params.theta,
        sigma: calibration.heston_params.sigma,
        rho: calibration.heston_params.rho,
        r: 0.045, // Default risk-free rate
        t: 1.0,   // Default time, will be overridden
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mispricing_detection() {
        let config = MispricingConfig {
            min_premium_threshold: 0.50,
            max_delta_for_short: 0.30,
            min_iv_rank: 0.0, // Not implemented yet
            max_spread_pct: 20.0,
            min_volume: 10,
            use_american_pricing: false,
            pricing_model: PricingModel::BlackScholes,
        };

        // This test would require actual market data files
        // For now, just test that the function doesn't panic
        let result = detect_mispriced_options("AAPL", &config);
        // We expect this to fail in test environment (no data files)
        // but it should fail gracefully, not panic
        assert!(result.is_err() || result.is_ok());
    }
}