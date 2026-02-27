// Short options strategy backtesting example
// Demonstrates selling calls and puts for premium collection

use dollarbill::backtesting::{BacktestEngine, BacktestConfig, SignalAction};
use dollarbill::market_data::csv_loader::load_csv_closes;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("===============================================================");
    println!("SHORT OPTIONS BACKTEST - Premium Collection Strategy");
    println!("Testing: Covered Calls + Cash-Secured Puts");
    println!("===============================================================\n");

    // Configuration
    let config = BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 15.0,  // 15% per position
        max_positions: 3,
        days_to_expiry: 30,
        risk_free_rate: 0.045,
        
        // Exit conditions
        max_days_hold: 25,        // Exit before expiration
        stop_loss_pct: Some(2.0),      // 200% loss (let winners run on shorts)
        take_profit_pct: Some(0.50),   // 50% profit on premium
        use_portfolio_management: false,
        ..Default::default()
    };

    let symbols = vec!["AAPL", "TSLA"];
    
    for symbol in &symbols {
        println!("\n===============================================================");
        println!("Backtesting {} - Short Options Strategy", symbol);
        println!("===============================================================\n");
        
        // Load historical data
        let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let historical_data = load_csv_closes(&filename)?;
        
        if historical_data.is_empty() {
            eprintln!("❌ No data loaded for {}", symbol);
            continue;
        }
        
        // Run backtest with short options strategy
        let mut engine = BacktestEngine::new(config.clone());
        let result = engine.run_with_signals(
            symbol,
            historical_data,
            |_symbol, spot, _day_idx, hist_vols| {
                let mut signals = Vec::new();
                
                // Get current volatility
                let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
                
                // Sell OTM call (5% above spot) for covered call strategy
                let call_strike = spot * 1.05;
                signals.push(SignalAction::SellCall {
                    strike: call_strike,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                });
                
                // Sell OTM put (5% below spot) for cash-secured put strategy
                let put_strike = spot * 0.95;
                signals.push(SignalAction::SellPut {
                    strike: put_strike,
                    days_to_expiry: 30,
                    volatility: hist_vol,
                });
                
                signals
            },
        );
        
        // Print results
        result.print_summary();
        
        // Print strategy-specific insights
        println!("\n📊 SHORT OPTIONS INSIGHTS");
        println!("---------------------------------------------------------------");
        
        let total_trades = result.metrics.total_trades;
        let win_rate = result.metrics.win_rate;
        
        println!("Premium Collection Strategy:");
        println!("  Total trades: {}", total_trades);
        println!("  Win rate: {:.1}%", win_rate);
        println!("  Avg profit per trade: ${:.2}", result.metrics.avg_win);
        
        if win_rate > 70.0 {
            println!("\n✅ High win rate - Short options profitable");
            println!("   Strategy: Collect premium, exit at 50% profit");
        } else {
            println!("\n⚠️  Lower win rate - Market moved against positions");
            println!("   Consider wider strikes or different timeframes");
        }
        
        println!("\n💡 Risk Notes:");
        println!("  • Short calls: Unlimited upside risk (stock can rise infinitely)");
        println!("  • Short puts: Risk if stock drops (max loss = strike price)");
        println!("  • Margin required: Not modeled in this backtest");
        println!("  • Real trading: Use covered calls (own stock) or cash-secured puts");
    }

    println!("\n===============================================================");
    println!("Backtest Complete!");
    println!("===============================================================\n");

    Ok(())
}
