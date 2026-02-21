// Credit Spreads Strategy - Directional Income Strategies
//
// BULL PUT SPREAD (Bullish Bias):
//   Sell Put (higher strike) + Buy Put (lower strike)
//   Collect credit, profit if stock stays above short strike
//
// BEAR CALL SPREAD (Bearish Bias):
//   Sell Call (lower strike) + Buy Call (higher strike)
//   Collect credit, profit if stock stays below short strike
//
// Both strategies have:
//   - Limited profit (credit received)
//   - Limited risk (spread width - credit)
//   - Defined risk/reward before entry

use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig, SignalAction};
use dollarbill::market_data::csv_loader::load_csv_closes;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("===============================================================");
    println!("CREDIT SPREADS STRATEGY BACKTEST");
    println!("Testing: Bull Put Spreads & Bear Call Spreads");
    println!("===============================================================\n");

    // Test both strategies
    test_bull_put_spreads()?;
    test_bear_call_spreads()?;
    
    println!("\n\n✅ Credit spreads backtest completed!");
    println!("\n💡 KEY TAKEAWAYS:");
    println!("  • Credit spreads offer defined risk with limited profit");
    println!("  • Bull put spreads work best in uptrending markets");
    println!("  • Bear call spreads work best in downtrending markets");
    println!("  • Target high-probability strikes (70-80% probability OTM)");
    println!("  • Manage winners early (50-75% of max profit)");
    
    Ok(())
}

fn test_bull_put_spreads() -> Result<(), Box<dyn Error>> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║           BULL PUT SPREAD - Bullish Income Strategy         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Strategy: Sell Put (near money) + Buy Put (far money)");
    println!("Outlook: Bullish to neutral");
    println!("Max Profit: Credit received");
    println!("Max Loss: Spread width - credit");
    println!();

    let config = BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 15.0,  // 15% per spread
        max_positions: 3,
        days_to_expiry: 30,
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        
        max_days_hold: 25,
        stop_loss_pct: Some(2.0),       // 200% loss
        take_profit_pct: Some(0.60),    // 60% profit
        use_portfolio_management: false,
    };

    let symbols = vec!["SPY", "AAPL", "MSFT"];
    
    for symbol in &symbols {
        println!("\n───────────────────────────────────────────────────────────");
        println!("Testing {} - Bull Put Spread", symbol);
        println!("───────────────────────────────────────────────────────────");
        
        let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let historical_data = match load_csv_closes(&filename) {
            Ok(data) => data,
            Err(_) => {
                println!("⚠️  Data file not found: {}", filename);
                continue;
            }
        };
        
        if historical_data.is_empty() {
            continue;
        }
        
        let mut engine = BacktestEngine::new(config.clone());
        let result = engine.run_with_signals(
            symbol,
            historical_data,
            |_symbol, spot, _day_idx, hist_vols| {
                let mut signals = Vec::new();
                let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
                
                // BULL PUT SPREAD
                // Sell put at 97% of spot (collect premium)
                let sell_put_strike = spot * 0.97;
                signals.push(SignalAction::SellPut {
                    strike: sell_put_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                // Buy put at 92% of spot (define max loss)
                let buy_put_strike = spot * 0.92;
                signals.push(SignalAction::BuyPut {
                    strike: buy_put_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                signals
            },
        );
        
        result.print_summary();
        print_spread_stats(&result.positions, "Bull Put Spread");
    }
    
    Ok(())
}

fn test_bear_call_spreads() -> Result<(), Box<dyn Error>> {
    println!("\n\n╔══════════════════════════════════════════════════════════════╗");
    println!("║          BEAR CALL SPREAD - Bearish Income Strategy         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Strategy: Sell Call (near money) + Buy Call (far money)");
    println!("Outlook: Bearish to neutral");
    println!("Max Profit: Credit received");
    println!("Max Loss: Spread width - credit");
    println!();

    let config = BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 15.0,
        max_positions: 3,
        days_to_expiry: 30,
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        
        max_days_hold: 25,
        stop_loss_pct: Some(2.0),
        take_profit_pct: Some(0.60),
        use_portfolio_management: false,
    };

    let symbols = vec!["SPY", "QQQ", "IWM"];
    
    for symbol in &symbols {
        println!("\n───────────────────────────────────────────────────────────");
        println!("Testing {} - Bear Call Spread", symbol);
        println!("───────────────────────────────────────────────────────────");
        
        let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let historical_data = match load_csv_closes(&filename) {
            Ok(data) => data,
            Err(_) => {
                println!("⚠️  Data file not found: {}", filename);
                continue;
            }
        };
        
        if historical_data.is_empty() {
            continue;
        }
        
        let mut engine = BacktestEngine::new(config.clone());
        let result = engine.run_with_signals(
            symbol,
            historical_data,
            |_symbol, spot, _day_idx, hist_vols| {
                let mut signals = Vec::new();
                let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
                
                // BEAR CALL SPREAD
                // Sell call at 103% of spot (collect premium)
                let sell_call_strike = spot * 1.03;
                signals.push(SignalAction::SellCall {
                    strike: sell_call_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                // Buy call at 108% of spot (define max loss)
                let buy_call_strike = spot * 1.08;
                signals.push(SignalAction::BuyCall {
                    strike: buy_call_strike,
                    days_to_expiry: config.days_to_expiry,
                    volatility: hist_vol,
                });
                
                signals
            },
        );
        
        result.print_summary();
        print_spread_stats(&result.positions, "Bear Call Spread");
    }
    
    Ok(())
}

fn print_spread_stats(positions: &[dollarbill::backtesting::position::Position], strategy_name: &str) {
    println!("\n📊 {} STATISTICS", strategy_name.to_uppercase());
    println!("═══════════════════════════════════════════════════════════");
    
    // Filter to closed positions only
    let closed: Vec<_> = positions.iter()
        .filter(|p| matches!(p.status, dollarbill::backtesting::position::PositionStatus::Closed 
                                     | dollarbill::backtesting::position::PositionStatus::Expired))
        .collect();
    
    if closed.is_empty() {
        println!("  No closed positions");
        return;
    }
    
    let total = closed.len();
    let winners = closed.iter().filter(|p| p.realized_pnl > 0.0).count();
    let losers = closed.iter().filter(|p| p.realized_pnl <= 0.0).count();
    let win_rate = (winners as f64 / total as f64) * 100.0;
    
    let total_profit: f64 = closed.iter().filter(|p| p.realized_pnl > 0.0).map(|p| p.realized_pnl).sum();
    let total_loss: f64 = closed.iter().filter(|p| p.realized_pnl <= 0.0).map(|p| p.realized_pnl).sum();
    let net_pnl = total_profit + total_loss;
    
    let avg_win = if winners > 0 { total_profit / winners as f64 } else { 0.0 };
    let avg_loss = if losers > 0 { total_loss / losers as f64 } else { 0.0 };
    
    println!("  Position Summary:");
    println!("    ├─ Total Positions: {}", total);
    println!("    ├─ Winners: {} ({:.1}%)", winners, win_rate);
    println!("    └─ Losers: {} ({:.1}%)", losers, 100.0 - win_rate);
    
    println!("\n  Profit/Loss:");
    println!("    ├─ Net P&L: ${:.2}", net_pnl);
    println!("    ├─ Avg Win: ${:.2}", avg_win);
    println!("    ├─ Avg Loss: ${:.2}", avg_loss);
    
    if avg_loss != 0.0 {
        let profit_factor = total_profit / total_loss.abs();
        println!("    └─ Profit Factor: {:.2}", profit_factor);
    }
    
    let avg_days: f64 = closed.iter().map(|p| p.days_held as f64).sum::<f64>() / total as f64;
    println!("\n  Holding Period:");
    println!("    └─ Avg Days: {:.1}", avg_days);
    
    println!("═══════════════════════════════════════════════════════════");
}
