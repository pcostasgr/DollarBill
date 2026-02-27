// Mispricing detection example
// Demonstrates identifying overpriced options for short selling

use dollarbill::strategies::mispricing::{detect_mispriced_options, generate_short_signals_from_mispricing, MispricingConfig, PricingModel};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("===============================================================");
    println!("MISPRICING DETECTION - Short Options Strategy");
    println!("Finding overpriced options to sell for premium collection");
    println!("===============================================================\n");

    // Configuration for mispricing detection
    let config = MispricingConfig {
        min_premium_threshold: 0.50,     // At least $0.50 premium to collect
        max_delta_for_short: 0.30,       // Max |delta| of 0.30 for short positions
        min_iv_rank: 0.0,                // Not implemented yet
        max_spread_pct: 20.0,            // Max 20% bid-ask spread
        min_volume: 10,                  // Minimum 10 contracts traded
        use_american_pricing: false,     // Use European pricing for speed
        pricing_model: PricingModel::BlackScholes, // Use Black-Scholes for fair value
    };

    // Test with available symbols that have options data
    let symbols = vec!["AAPL", "TSLA", "NVDA"];

    for symbol in &symbols {
        println!("🔍 Analyzing {} for mispriced options...", symbol);

        match detect_mispriced_options(symbol, &config) {
            Ok(mispriced_options) => {
                if mispriced_options.is_empty() {
                    println!("   No mispriced options found meeting criteria");
                } else {
                    println!("   Found {} potentially mispriced options:", mispriced_options.len());

                    // Show top 3 opportunities
                    for (i, result) in mispriced_options.iter().take(3).enumerate() {
                        let option_type = match result.option.option_type {
                            dollarbill::calibration::market_option::OptionType::Call => "Call",
                            dollarbill::calibration::market_option::OptionType::Put => "Put",
                        };

                        println!("   {}. ${:.0} {} (Δ={:.2}): Model=${:.2}, Market=${:.2}, Premium=${:.2} (+{:.1}%)",
                            i + 1,
                            result.option.strike,
                            option_type,
                            result.delta,
                            result.model_price,
                            result.market_price,
                            result.premium_available,
                            result.mispricing_pct
                        );
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error analyzing {}: {}", symbol, e);
                println!("      (This is expected if options data file doesn't exist)");
            }
        }

        println!();
    }

    // Generate signals for the first available symbol
    if let Some(symbol) = symbols.first() {
        println!("📊 Generating short signals for {}...", symbol);

        match generate_short_signals_from_mispricing(symbol, &config) {
            Ok(signals) => {
                if signals.is_empty() {
                    println!("   No short signals generated");
                } else {
                    println!("   Generated {} short signals:", signals.len());
                    for (i, signal) in signals.iter().enumerate() {
                        match signal {
                            dollarbill::backtesting::SignalAction::SellCall { strike, days_to_expiry, .. } => {
                                println!("   {}. Sell Call ${:.0} ({} days)", i + 1, strike, days_to_expiry);
                            }
                            dollarbill::backtesting::SignalAction::SellPut { strike, days_to_expiry, .. } => {
                                println!("   {}. Sell Put ${:.0} ({} days)", i + 1, strike, days_to_expiry);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error generating signals: {}", e);
            }
        }
    }

    println!("\n💡 Strategy Notes:");
    println!("  • Only considers options where model price < market price");
    println!("  • Filters by minimum premium, delta limits, and liquidity");
    println!("  • Uses Black-Scholes for fair value calculation");
    println!("  • Higher mispricing % = better short opportunity");
    println!("  • Lower delta = less directional risk");

    println!("\n⚠️  Risk Considerations:");
    println!("  • Short calls have unlimited upside risk");
    println!("  • Short puts risk stock dropping to zero");
    println!("  • Requires margin and may need position sizing");
    println!("  • Real trading needs additional filters and risk management");

    println!("\n===============================================================");
    println!("Mispricing Analysis Complete!");
    println!("===============================================================\n");

    Ok(())
}