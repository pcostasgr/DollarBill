// Iron condor and spread strategy detection example
// Demonstrates multi-leg option strategies for premium collection

use dollarbill::strategies::spreads::{detect_iron_condors, detect_credit_call_spreads, SpreadConfig, generate_spread_signals};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===============================================================");
    println!("IRON CONDOR & SPREAD STRATEGY DETECTOR");
    println!("Multi-leg options strategies for premium collection");
    println!("===============================================================\n");

    // Configuration for spread detection
    let config = SpreadConfig {
        min_premium_threshold: 0.50,     // At least $0.50 net premium
        max_spread_width_pct: 15.0,      // Max 15% spread width
        min_days_to_expiry: 7,           // At least 1 week
        max_days_to_expiry: 60,          // Max 2 months
        min_volume: 10,                  // Minimum volume
        max_spread_pct: 20.0,            // Max 20% bid-ask spread
        risk_free_rate: 0.045,           // 4.5% risk-free rate
    };

    // Test symbols with options data
    let symbols = vec!["AAPL", "TSLA", "NVDA"];

    for symbol in &symbols {
        println!("🔍 Analyzing {} for spread opportunities...", symbol);

        // Iron Condors
        match detect_iron_condors(symbol, &config) {
            Ok(condors) => {
                if condors.is_empty() {
                    println!("   No iron condor opportunities found");
                } else {
                    println!("   Found {} iron condor opportunities:", condors.len());
                    for (i, condor) in condors.iter().take(3).enumerate() {
                        println!("   {}. ${:.0}P/${:.0}C wings: ${:.2} premium, Max Loss: ${:.0}, Win Prob: {:.1}%",
                            i + 1,
                            condor.signal.iron_condor_sell_put_strike(),
                            condor.signal.iron_condor_sell_call_strike(),
                            condor.net_premium,
                            condor.max_loss,
                            condor.win_probability * 100.0
                        );
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error analyzing iron condors: {}", e);
            }
        }

        // Credit Call Spreads
        match detect_credit_call_spreads(symbol, &config) {
            Ok(spreads) => {
                if spreads.is_empty() {
                    println!("   No credit call spread opportunities found");
                } else {
                    println!("   Found {} credit call spread opportunities:", spreads.len());
                    for (i, spread) in spreads.iter().take(3).enumerate() {
                        println!("   {}. Sell ${:.0}C/Buy ${:.0}C: ${:.2} premium, Max Loss: ${:.0}, Win Prob: {:.1}%",
                            i + 1,
                            spread.signal.credit_call_spread_sell_strike(),
                            spread.signal.credit_call_spread_buy_strike(),
                            spread.net_premium,
                            spread.max_loss,
                            spread.win_probability * 100.0
                        );
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error analyzing credit spreads: {}", e);
            }
        }

        println!();
    }

    // Generate signals for all strategies
    println!("📊 Generating comprehensive spread signals...\n");

    for symbol in &symbols {
        match generate_spread_signals(symbol, &config) {
            Ok(signals) => {
                if signals.is_empty() {
                    println!("{}: No spread signals generated", symbol);
                } else {
                    println!("{}: Generated {} spread signals", symbol, signals.len());
                    for signal in signals {
                        match &signal.action {
                            dollarbill::strategies::SignalAction::IronCondor { sell_call_strike, buy_call_strike, sell_put_strike, buy_put_strike, .. } => {
                                println!("  🦅 Iron Condor: Sell {:.0}P/{:.0}C, Buy {:.0}P/{:.0}C (Premium: ${:.2})",
                                    sell_put_strike, sell_call_strike, buy_put_strike, buy_call_strike, signal.edge);
                            }
                            dollarbill::strategies::SignalAction::CreditCallSpread { sell_strike, buy_strike, .. } => {
                                println!("  📈 Credit Call Spread: Sell {:.0}C, Buy {:.0}C (Premium: ${:.2})",
                                    sell_strike, buy_strike, signal.edge);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => {
                println!("{}: ❌ Error generating signals: {}", symbol, e);
            }
        }
    }

    println!("\n💡 Strategy Notes:");
    println!("  • Iron Condors: Sell OTM call+put, buy further OTM protection");
    println!("  • Credit Spreads: Sell closer strike, buy further strike");
    println!("  • Higher premium = better risk/reward");
    println!("  • Higher win probability = more conservative");

    println!("\n⚠️  Risk Considerations:");
    println!("  • Maximum loss occurs if stock moves beyond wings");
    println!("  • Time decay works in your favor");
    println!("  • Requires margin for short options");
    println!("  • Best in low volatility, sideways markets");

    println!("\n===============================================================");
    println!("Spread Strategy Analysis Complete!");
    println!("===============================================================");

    Ok(())
}