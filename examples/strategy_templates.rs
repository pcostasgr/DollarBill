// Strategy Templates Example - Using Configurable Strategy Templates
//
// This example demonstrates how to use the strategy template system
// to quickly backtest different options strategies with custom parameters

use dollarbill::backtesting::engine::{BacktestEngine, BacktestConfig};
use dollarbill::strategies::templates::{
    IronCondorConfig, BullPutSpreadConfig, BearCallSpreadConfig,
    ShortStrangleConfig, CoveredCallConfig,
};
use dollarbill::market_data::csv_loader::load_csv_closes;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        STRATEGY TEMPLATES - Customizable Configurations     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    
    println!("This example shows how to use strategy templates to quickly");
    println!("backtest different options strategies with custom parameters.\n");
    
    // Test conservative vs aggressive iron condors
    test_iron_condor_variations()?;
    
    // Test different spread widths for credit spreads
    test_credit_spread_variations()?;
    
    println!("\n\n✅ Strategy templates demonstration completed!");
    println!("\n💡 KEY BENEFITS OF TEMPLATES:");
    println!("  • Quick strategy testing with different parameters");
    println!("  • Consistent strategy implementation");
    println!("  • Easy to customize for your risk tolerance");
    println!("  • Reusable across different symbols and timeframes");
    
    Ok(())
}

fn test_iron_condor_variations() -> Result<(), Box<dyn Error>> {
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("IRON CONDOR VARIATIONS - Conservative vs Aggressive");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    let symbol = "SPY";
    let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
    let historical_data = match load_csv_closes(&filename) {
        Ok(data) => data,
        Err(_) => {
            println!("⚠️  Data file not found: {}", filename);
            return Ok(());
        }
    };
    
    if historical_data.is_empty() {
        println!("❌ No data loaded");
        return Ok(());
    }
    
    // Conservative Iron Condor - Wide wings, lower premium
    println!("\n1️⃣  CONSERVATIVE IRON CONDOR");
    println!("───────────────────────────────────────────────────────────\n");
    
    let conservative_config = IronCondorConfig {
        days_to_expiry: 45,
        sell_put_pct: 0.93,    // Far OTM
        buy_put_pct: 0.88,     // 5% spread
        sell_call_pct: 1.07,   // Far OTM
        buy_call_pct: 1.12,    // 5% spread
    };
    
    println!("Configuration:");
    println!("  • Sell Put at {}% of spot", conservative_config.sell_put_pct * 100.0);
    println!("  • Buy Put at {}% of spot", conservative_config.buy_put_pct * 100.0);
    println!("  • Sell Call at {}% of spot", conservative_config.sell_call_pct * 100.0);
    println!("  • Buy Call at {}% of spot", conservative_config.buy_call_pct * 100.0);
    println!();
    
    let mut engine = BacktestEngine::new(BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 20.0,
        max_positions: 2,
        days_to_expiry: 45,
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        max_days_hold: 40,
        stop_loss_pct: Some(2.0),
        take_profit_pct: Some(0.50),
    });
    
    let result = engine.run_with_signals(
        symbol,
        historical_data.clone(),
        move |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.25);
            conservative_config.generate_signals(spot, hist_vol)
        },
    );
    
    result.print_summary();
    
    // Aggressive Iron Condor - Narrow wings, higher premium
    println!("\n\n2️⃣  AGGRESSIVE IRON CONDOR");
    println!("───────────────────────────────────────────────────────────\n");
    
    let aggressive_config = IronCondorConfig {
        days_to_expiry: 30,
        sell_put_pct: 0.97,    // Closer to money
        buy_put_pct: 0.94,     // 3% spread (narrower)
        sell_call_pct: 1.03,   // Closer to money
        buy_call_pct: 1.06,    // 3% spread (narrower)
    };
    
    println!("Configuration:");
    println!("  • Sell Put at {}% of spot", aggressive_config.sell_put_pct * 100.0);
    println!("  • Buy Put at {}% of spot", aggressive_config.buy_put_pct * 100.0);
    println!("  • Sell Call at {}% of spot", aggressive_config.sell_call_pct * 100.0);
    println!("  • Buy Call at {}% of spot", aggressive_config.buy_call_pct * 100.0);
    println!();
    
    let mut engine = BacktestEngine::new(BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 20.0,
        max_positions: 2,
        days_to_expiry: 30,
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        max_days_hold: 25,
        stop_loss_pct: Some(2.0),
        take_profit_pct: Some(0.60),
    });
    
    let result = engine.run_with_signals(
        symbol,
        historical_data,
        move |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.25);
            aggressive_config.generate_signals(spot, hist_vol)
        },
    );
    
    result.print_summary();
    
    println!("\n📊 COMPARISON:");
    println!("  Conservative: Lower premium, higher win rate, further strikes");
    println!("  Aggressive: Higher premium, lower win rate, closer strikes");
    
    Ok(())
}

fn test_credit_spread_variations() -> Result<(), Box<dyn Error>> {
    println!("\n\n═══════════════════════════════════════════════════════════════");
    println!("CREDIT SPREAD VARIATIONS - Different Spread Widths");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    let symbol = "AAPL";
    let filename = format!("data/{}_five_year.csv", symbol.to_lowercase());
    let historical_data = match load_csv_closes(&filename) {
        Ok(data) => data,
        Err(_) => {
            println!("⚠️  Data file not found: {}", filename);
            return Ok(());
        }
    };
    
    if historical_data.is_empty() {
        println!("❌ No data loaded");
        return Ok(());
    }
    
    // Narrow spread - higher credit, more risk
    println!("\n1️⃣  NARROW BULL PUT SPREAD (Higher Premium, More Risk)");
    println!("───────────────────────────────────────────────────────────\n");
    
    let narrow_spread = BullPutSpreadConfig {
        days_to_expiry: 30,
        sell_put_pct: 0.98,   // 2% below
        buy_put_pct: 0.95,    // 5% below (3% spread)
    };
    
    println!("Configuration:");
    println!("  • Sell Put: {}% of spot", narrow_spread.sell_put_pct * 100.0);
    println!("  • Buy Put: {}% of spot", narrow_spread.buy_put_pct * 100.0);
    println!("  • Spread Width: {}%", (narrow_spread.sell_put_pct - narrow_spread.buy_put_pct) * 100.0);
    println!();
    
    let mut engine = BacktestEngine::new(BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 15.0,
        max_positions: 3,
        days_to_expiry: 30,
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        max_days_hold: 25,
        stop_loss_pct: Some(2.0),
        take_profit_pct: Some(0.60),
    });
    
    let result = engine.run_with_signals(
        symbol,
        historical_data.clone(),
        move |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            narrow_spread.generate_signals(spot, hist_vol)
        },
    );
    
    result.print_summary();
    
    // Wide spread - lower credit, less risk
    println!("\n\n2️⃣  WIDE BULL PUT SPREAD (Lower Premium, Less Risk)");
    println!("───────────────────────────────────────────────────────────\n");
    
    let wide_spread = BullPutSpreadConfig {
        days_to_expiry: 30,
        sell_put_pct: 0.95,   // 5% below
        buy_put_pct: 0.88,    // 12% below (7% spread)
    };
    
    println!("Configuration:");
    println!("  • Sell Put: {}% of spot", wide_spread.sell_put_pct * 100.0);
    println!("  • Buy Put: {}% of spot", wide_spread.buy_put_pct * 100.0);
    println!("  • Spread Width: {}%", (wide_spread.sell_put_pct - wide_spread.buy_put_pct) * 100.0);
    println!();
    
    let mut engine = BacktestEngine::new(BacktestConfig {
        initial_capital: 100_000.0,
        position_size_pct: 15.0,
        max_positions: 3,
        days_to_expiry: 30,
        risk_free_rate: 0.045,
        commission_per_trade: 2.0,
        max_days_hold: 25,
        stop_loss_pct: Some(2.0),
        take_profit_pct: Some(0.60),
    });
    
    let result = engine.run_with_signals(
        symbol,
        historical_data,
        move |_symbol, spot, _day_idx, hist_vols| {
            let hist_vol = hist_vols.last().copied().unwrap_or(0.30);
            wide_spread.generate_signals(spot, hist_vol)
        },
    );
    
    result.print_summary();
    
    println!("\n📊 SPREAD WIDTH COMPARISON:");
    println!("  Narrow Spreads: Higher credit, less protection, closer to breakeven");
    println!("  Wide Spreads: Lower credit, more protection, further from breakeven");
    println!("  Choose based on your risk tolerance and market outlook");
    
    Ok(())
}
