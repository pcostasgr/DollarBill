// Short strangle backtesting example
// Tests the short strangle strategy with historical data

use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig};
use dollarbill::market_data::csv_loader::load_csv_closes;
use dollarbill::strategies::{short_strangle::ShortStrangleStrategy, TradingStrategy};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===============================================================");
    println!("SHORT STRANGLE BACKTESTING STRATEGY");
    println!("Testing premium collection with OTM call + put sales");
    println!("===============================================================\n");

    // Load historical data for testing
    let symbol = "TSLA";
    let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
    let historical_data = load_csv_closes(&filename)
        .map_err(|e| format!("Failed to load historical data for {}: {}", symbol, e))?;

    if historical_data.is_empty() {
        eprintln!("No historical data found for {}", symbol);
        return Ok(());
    }

    println!("📊 Loaded {} days of historical data for {}", historical_data.len(), symbol);
    println!("Period: {} to {}\n",
        historical_data.last().unwrap().date,
        historical_data.first().unwrap().date);

    // Configure backtest
    let backtest_config = BacktestConfig {
        initial_capital: 100_000.0,
        trading_costs: Default::default(),
        risk_free_rate: 0.045,
        max_positions: 3,           // Max 3 strangles open simultaneously
        position_size_pct: 5.0,     // 5% of capital per position
        days_to_expiry: 30,         // 30-day options
        max_days_hold: 21,          // Close after 21 days
        stop_loss_pct: Some(200.0), // 200% stop loss
        take_profit_pct: Some(50.0), // 50% profit target
        use_portfolio_management: false,
        ..BacktestConfig::default()
    };

    // Create short strangle strategy
    let strangle_strategy = ShortStrangleStrategy {
        min_iv_rank: 0.6,         // Enter when IV > 60th percentile
        max_delta: 0.30,          // Relaxed: OTM options with |delta| < 30%
        min_days_to_expiry: 14,   // At least 2 weeks
        max_days_to_expiry: 45,   // Max 6 weeks
        profit_target_pct: 50.0,  // Take profit at 50% of max loss
        stop_loss_pct: 200.0,     // Stop loss at 200% of max loss
    };

    println!("🎯 Strategy Configuration:");
    println!("  • Min IV Rank: {:.1}%", strangle_strategy.min_iv_rank * 100.0);
    println!("  • Max Delta: {:.1}%", strangle_strategy.max_delta * 100.0);
    println!("  • Expiry Range: {}-{} days", strangle_strategy.min_days_to_expiry, strangle_strategy.max_days_to_expiry);
    println!("  • Profit Target: {:.0}% of max loss", strangle_strategy.profit_target_pct);
    println!("  • Stop Loss: {:.0}% of max loss", strangle_strategy.stop_loss_pct);
    println!();

    // Run backtest with signal generator
    let mut engine = BacktestEngine::new(backtest_config);

    let result = engine.run_with_signals(
        symbol,
        historical_data,
        |symbol, spot, day_idx, hist_vols| {
            // Simplified signal generation - in practice you'd use market IV
            let market_iv = 0.7; // Increased to meet min_iv_rank threshold
            let model_iv = 0.25;
            let historical_vol = hist_vols.get(day_idx).copied().unwrap_or(0.3);

            let signals = strangle_strategy.generate_signals(symbol, spot, market_iv, model_iv, historical_vol);
            signals.into_iter()
                .map(|signal| signal.action)
                .collect()
        }
    );

    // Display results
    println!("📈 Backtest Results:");
    println!("  • Total P&L: ${:.2} ({:.2}%)",
        result.metrics.total_pnl,
        result.metrics.total_return_pct);
    println!("  • Max Drawdown: ${:.2} ({:.2}%)",
        result.metrics.max_drawdown,
        result.metrics.max_drawdown_pct);
    println!("  • Win Rate: {:.1}%", result.metrics.win_rate);
    println!("  • Profit Factor: {:.2}", result.metrics.profit_factor);
    println!("  • Total Trades: {}", result.metrics.total_trades);
    println!("  • Sharpe Ratio: {:.2}", result.metrics.sharpe_ratio);
    println!("  • Avg Days Held: {:.1}", result.metrics.avg_days_held);
    println!();

    // Strategy-specific analysis
    println!("📊 Strategy Analysis:");
    println!("  • Average Win: ${:.2}", result.metrics.avg_win);
    println!("  • Average Loss: ${:.2}", result.metrics.avg_loss);
    println!("  • Largest Win: ${:.2}", result.metrics.largest_win);
    println!("  • Largest Loss: ${:.2}", result.metrics.largest_loss);
    println!("  • Average Days Held: {:.1}", result.metrics.avg_days_held);
    println!();

    // Risk metrics
    println!("⚠️  Risk Metrics:");
    println!("  • Max Drawdown: ${:.2} ({:.2}%)", result.metrics.max_drawdown, result.metrics.max_drawdown_pct);
    println!("  • Profit Factor: {:.2}", result.metrics.profit_factor);
    println!();

    // Performance commentary
    if result.metrics.total_return_pct > 0.0 {
        println!("✅ Strategy Performance: PROFITABLE");
        if result.metrics.sharpe_ratio > 1.0 {
            println!("   • Good risk-adjusted returns (Sharpe > 1.0)");
        }
        if result.metrics.win_rate > 0.6 {
            println!("   • Strong win rate suggests good entry timing");
        }
    } else {
        println!("❌ Strategy Performance: UNPROFITABLE");
        if result.metrics.max_drawdown_pct > 20.0 {
            println!("   • Large drawdowns indicate need for better risk management");
        }
    }

    println!("\n💡 Short Strangle Insights:");
    println!("  • Best in high IV environments (volatility contraction)");
    println!("  • Profit from time decay and moderate moves");
    println!("  • Risk management critical due to unlimited loss potential");
    println!("  • Consider adding position sizing based on IV rank");

    println!("\n===============================================================");
    println!("Short Strangle Backtest Complete!");
    println!("===============================================================");

    Ok(())
}