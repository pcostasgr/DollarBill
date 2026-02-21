// Iron Condor Strategy - Limited Risk Income Strategy
// Combines a bull put spread and bear call spread for neutral market outlook
//
// Structure (all OTM):
//   Buy Put (far OTM) <- Sell Put (near OTM) | Spot | Sell Call (near OTM) -> Buy Call (far OTM)
//
// Maximum Profit: Net premium received
// Maximum Loss: Width of spread - net premium
// Best Case: Stock stays between short strikes until expiration

use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig, SignalAction};
use dollarbill::market_data::csv_loader::load_csv_closes;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("===============================================================");
    println!("IRON CONDOR STRATEGY BACKTEST");
    println!("Market Outlook: Neutral (expect low volatility)");
    println!("Risk Profile: Limited risk, limited profit");
    println!("===============================================================\n");

    // Iron Condor Configuration
    // This strategy works best in low-volatility environments
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 20.0,  // 20% per iron condor (4-leg position)
        max_positions: 2,          // Limit to 2 iron condors at a time
        days_to_expiry: 45,        // 45 DTE is common for iron condors
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        
        // Exit conditions
        max_days_hold: 40,         // Close before expiration if held this long
        stop_loss_pct: Some(2.0),       // Exit at 200% of premium received (max loss)
        take_profit_pct: Some(0.50),    // Lock in 50% of max profit early
        use_portfolio_management: false,
    };

    println!("Strategy Parameters:");
    println!("  ├─ Initial Capital: ${:.0}", config.initial_capital);
    println!("  ├─ Position Size: {}%", config.position_size_pct);
    println!("  ├─ Max Positions: {}", config.max_positions);
    println!("  ├─ Days to Expiry: {}", config.days_to_expiry);
    println!("  ├─ Take Profit: {}% of max profit", config.take_profit_pct.unwrap() * 100.0);
    println!("  └─ Stop Loss: {}% of max loss", config.stop_loss_pct.unwrap() * 100.0);
    println!();

    // Test on multiple symbols with different volatility profiles
    let symbols = vec![
        ("SPY", "S&P 500 ETF - Low volatility"),
        ("QQQ", "Nasdaq ETF - Medium volatility"),
        ("TSLA", "Tesla - High volatility"),
    ];
    
    for (symbol, description) in &symbols {
        println!("\n===============================================================");
        println!("Backtesting {} - {}", symbol, description);
        println!("===============================================================\n");
        
        // Load historical data
        let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let historical_data = match load_csv_closes(&filename) {
            Ok(data) => data,
            Err(_) => {
                println!("⚠️  Data file not found: {}", filename);
                continue;
            }
        };
        
        if historical_data.is_empty() {
            println!("❌ No data loaded for {}", symbol);
            continue;
        }
        
        // Run backtest with iron condor strategy
        let mut engine = BacktestEngine::new(config.clone());
        let result = engine.run_with_signals(
            symbol,
            historical_data,
            |_symbol, spot, _day_idx, hist_vols| {
                // Iron Condor Signal Generator
                let mut signals = Vec::new();
                
                // Get current volatility for pricing
                let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
                
                // IRON CONDOR LEGS (4-leg strategy):
                
                // 1. PUT SPREAD (Bull Put Spread) - Lower side protection
                //    Sell Put at 95% of spot (collect premium)
                let sell_put_strike = spot * 0.95;
                signals.push(SignalAction::SellPut {
                    strike: sell_put_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                //    Buy Put at 90% of spot (limit downside risk)
                let buy_put_strike = spot * 0.90;
                signals.push(SignalAction::BuyPut {
                    strike: buy_put_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                // 2. CALL SPREAD (Bear Call Spread) - Upper side protection
                //    Sell Call at 105% of spot (collect premium)
                let sell_call_strike = spot * 1.05;
                signals.push(SignalAction::SellCall {
                    strike: sell_call_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                //    Buy Call at 110% of spot (limit upside risk)
                let buy_call_strike = spot * 1.10;
                signals.push(SignalAction::BuyCall {
                    strike: buy_call_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                // Return all 4 legs as a coordinated iron condor
                signals
            },
        );
        
        // Print results
        result.print_summary();
        
        // Print iron condor specific insights
        println!("\n📊 IRON CONDOR STRATEGY INSIGHTS");
        println!("═══════════════════════════════════════════════════════════");
        
        // Calculate win rate from closed positions
        let closed_positions: Vec<_> = result.positions.iter()
            .filter(|p| matches!(p.status, dollarbill::backtesting::position::PositionStatus::Closed 
                                         | dollarbill::backtesting::position::PositionStatus::Expired))
            .collect();
        
        let total_positions = closed_positions.len();
        let winning_positions = closed_positions.iter().filter(|p| p.realized_pnl > 0.0).count();
        let win_rate = if total_positions > 0 {
            (winning_positions as f64 / total_positions as f64) * 100.0
        } else {
            0.0
        };
        
        println!("\n Trading Statistics:");
        println!("  ├─ Total Positions: {}", total_positions);
        println!("  ├─ Winning Positions: {}", winning_positions);
        println!("  ├─ Win Rate: {:.1}%", win_rate);
        
        if !closed_positions.is_empty() {
            let avg_pnl: f64 = closed_positions.iter().map(|p| p.realized_pnl).sum::<f64>() 
                              / closed_positions.len() as f64;
            let avg_days: f64 = closed_positions.iter().map(|p| p.days_held as f64).sum::<f64>()
                              / closed_positions.len() as f64;
            
            println!("  ├─ Avg P&L per Position: ${:.2}", avg_pnl);
            println!("  └─ Avg Days Held: {:.1} days", avg_days);
        }

        
        // Strategy Education
        println!("\n💡 WHAT IS AN IRON CONDOR?");
        println!("  An iron condor profits when the stock stays within a range.");
        println!("  You collect premium from selling options closer to the money,");
        println!("  and buy options further out to limit your risk.");
        println!();
        println!("  Maximum Profit: Premium collected (if stock stays in range)");
        println!("  Maximum Loss: Width of spread - premium (if stock moves too far)");
        println!("  Best Environment: Low volatility, rangebound markets");
        println!();
        println!("  This backtest uses:");
        println!("    • Sell Put at 95% of spot");
        println!("    • Buy Put at 90% of spot (5% spread width)");
        println!("    • Sell Call at 105% of spot");
        println!("    • Buy Call at 110% of spot (5% spread width)");
        println!("═══════════════════════════════════════════════════════════");
    }
    
    println!("\n\n✅ Iron Condor backtest completed!");
    println!("\n💡 TIPS FOR LIVE TRADING:");
    println!("  1. Use on high-probability underlyings (SPY, QQQ)");
    println!("  2. Target 45-60 DTE for best theta decay");
    println!("  3. Close at 50-75% of max profit to reduce risk");
    println!("  4. Avoid earnings announcements and major events");
    println!("  5. Monitor implied volatility - high IV is better for entries");
    
    Ok(())
}
