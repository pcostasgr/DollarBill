// Backtest example - demonstrates the backtesting framework
// Tests real volatility and momentum-based trading strategies

use dollarbill::backtesting::{BacktestEngine, BacktestConfig, SignalAction};
use dollarbill::market_data::csv_loader::load_csv_closes;
use std::error::Error;

// Helper: Calculate z-score for volatility
fn calculate_vol_zscore(current_vol: f64, hist_vols: &[f64], lookback: usize) -> f64 {
    if hist_vols.len() < lookback {
        return 0.0;
    }
    let recent = &hist_vols[hist_vols.len().saturating_sub(lookback)..];
    let mean: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
    let variance: f64 = recent.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / recent.len() as f64;
    let std_dev = variance.sqrt();
    if std_dev > 0.0 {
        (current_vol - mean) / std_dev
    } else {
        0.0
    }
}

// Helper: Calculate RSI
fn calculate_rsi(prices: &[(String, f64)], period: usize) -> f64 {
    if prices.len() < period + 1 {
        return 50.0; // Neutral
    }
    
    let recent = &prices[prices.len().saturating_sub(period + 1)..];
    let mut gains = 0.0;
    let mut losses = 0.0;
    
    for i in 1..recent.len() {
        let change = recent[i].1 - recent[i-1].1;
        if change > 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }
    
    let avg_gain = gains / period as f64;
    let avg_loss = losses / period as f64;
    
    if avg_loss == 0.0 {
        return 100.0;
    }
    
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

// Helper: Calculate price momentum
fn calculate_momentum(prices: &[(String, f64)], period: usize) -> f64 {
    if prices.len() < period + 1 {
        return 0.0;
    }
    let current = prices.last().unwrap().1;
    let past = prices[prices.len() - period - 1].1;
    (current - past) / past
}

// Helper: Calculate annualized volatility from price data
fn calculate_historical_volatility(prices: &[(String, f64)]) -> f64 {
    if prices.len() < 30 {
        return 0.0;
    }
    
    // Calculate daily returns
    let mut returns = Vec::new();
    for i in 1..prices.len() {
        let daily_return = (prices[i].1 / prices[i-1].1) - 1.0;
        returns.push(daily_return);
    }
    
    // Calculate standard deviation of returns
    let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance: f64 = returns.iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>() / returns.len() as f64;
    let daily_vol = variance.sqrt();
    
    // Annualize (sqrt(252 trading days))
    daily_vol * (252.0_f64).sqrt() * 100.0  // Return as percentage
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("\n{}", "=".repeat(80));
    println!("OPTIONS STRATEGY BACKTESTER");
    println!("Historical Performance Analysis with Full P&L Tracking");
    println!("{}", "=".repeat(80));
    println!();
    
    // Test all available symbols
    let symbols = vec!["TSLA", "AAPL", "NVDA", "MSFT", "META", "GOOGL", "AMZN"];
    
    for symbol in symbols {
        println!("\n🔍 Backtesting {}...", symbol);
        backtest_symbol(symbol)?;
    }
    
    Ok(())
}

fn backtest_symbol(symbol: &str) -> Result<(), Box<dyn Error>> {
    // Load historical data
    let csv_file = format!("data/{}_five_year.csv", symbol.to_lowercase());
    let mut historical_data = load_csv_closes(&csv_file)?;
    
    // Reverse so we iterate forward through time (oldest first)
    historical_data.reverse();
    
    println!("  Loaded {} days of historical data", historical_data.len());
    
    // Measure historical volatility to select appropriate strategy
    let prices: Vec<(String, f64)> = historical_data
        .iter()
        .map(|d| (d.date.clone(), d.close))
        .collect();
    let annual_vol = calculate_historical_volatility(&prices);
    
    println!("  Measured Volatility: {:.1}% annualized", annual_vol);
    
    // Adaptive strategy selection based on volatility
    if annual_vol > 50.0 {
        println!("  🎯 Strategy: HIGH-VOL Momentum (aggressive)");
    } else if annual_vol > 35.0 {
        println!("  🎯 Strategy: MEDIUM-VOL Momentum (conservative)");
    } else {
        println!("  ⚠️  Strategy: LOW-VOL (options buying not recommended)");
    }
    
    // Configure backtest with different holding periods
    let config_short = BacktestConfig {
        initial_capital: 100_000.0,
        commission_per_trade: 1.0,
        risk_free_rate: 0.05,
        max_positions: 5,
        position_size_pct: 20.0,
        days_to_expiry: 14,  // 2-week options
        max_days_hold: 10,   // Close after 10 days
        stop_loss_pct: Some(50.0),
        take_profit_pct: Some(100.0),
    };
    
    let config_medium = BacktestConfig {
        initial_capital: 100_000.0,
        commission_per_trade: 1.0,
        risk_free_rate: 0.05,
        max_positions: 5,
        position_size_pct: 20.0,
        days_to_expiry: 30,  // 1-month options
        max_days_hold: 21,   // Close after 3 weeks
        stop_loss_pct: Some(50.0),
        take_profit_pct: Some(100.0),
    };
    
    let config_long = BacktestConfig {
        initial_capital: 100_000.0,
        commission_per_trade: 1.0,
        risk_free_rate: 0.05,
        max_positions: 5,
        position_size_pct: 20.0,
        days_to_expiry: 60,  // 2-month options
        max_days_hold: 45,   // Close after 6 weeks
        stop_loss_pct: Some(50.0),
        take_profit_pct: Some(100.0),
    };
    
    // Run strategy 1: Short-term (adaptive based on volatility)
    println!("\n📊 STRATEGY 1: Short-Term (14-day options, 10-day hold)");
    let mut engine = BacktestEngine::new(config_short.clone());
    
    // Adjust threshold based on volatility
    let vol_threshold = if annual_vol > 50.0 {
        0.35  // High-vol: only trade when vol is really low
    } else if annual_vol > 35.0 {
        0.30  // Medium-vol: moderate threshold
    } else {
        0.25  // Low-vol: lower threshold (but likely won't help much)
    };
    
    let result = engine.run_simple_strategy(
        symbol,
        historical_data.clone(),
        vol_threshold,
    );
    
    result.print_summary();
    result.print_trades(10);
    
    // Run strategy 2: Medium-term (1 month) - Adaptive Momentum
    println!("\n\n📊 STRATEGY 2: Medium-Term (30-day options, 21-day hold)");
    let mut engine2 = BacktestEngine::new(config_medium.clone());
    let historical_data_clone2 = historical_data.clone();
    let result2 = engine2.run_with_signals(
        symbol,
        historical_data.clone(),
        move |_symbol, spot, day_idx, hist_vols| {
            let mut signals = Vec::new();
            
            // Need at least 30 days of history
            if day_idx < 30 {
                return signals;
            }
            
            if let Some(&current_vol) = hist_vols.get(day_idx) {
                // Get price data
                let prices: Vec<(String, f64)> = historical_data_clone2[..=day_idx]
                    .iter()
                    .map(|d| (d.date.clone(), d.close))
                    .collect();
                
                let momentum_5d = calculate_momentum(&prices, 5);
                let momentum_10d = calculate_momentum(&prices, 10);
                let rsi = calculate_rsi(&prices, 14);
                
                // Adaptive thresholds based on symbol volatility
                let (momentum_threshold, strike_mult) = if annual_vol > 50.0 {
                    (0.02, 1.01)  // High-vol: 2% threshold, 1% OTM
                } else if annual_vol > 35.0 {
                    (0.015, 1.005) // Medium-vol: 1.5% threshold, 0.5% OTM
                } else {
                    (0.01, 1.0)    // Low-vol: 1% threshold, ATM (but likely won't help)
                };
                
                // SIGNAL 1: Upward momentum with room to run
                if momentum_5d > momentum_threshold && momentum_10d > 0.0 && rsi < 70.0 {
                    signals.push(SignalAction::BuyCall {
                        strike: spot * strike_mult,
                        days_to_expiry: 30,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 2: Downward momentum with room to run
                else if momentum_5d < -momentum_threshold && momentum_10d < 0.0 && rsi > 30.0 {
                    signals.push(SignalAction::BuyPut {
                        strike: spot * (2.0 - strike_mult),  // Mirror of call strike
                        days_to_expiry: 30,
                        volatility: current_vol,
                    });
                }
            }
            
            signals
        },
    );
    
    result2.print_summary();
    result2.print_trades(10);
    
    // Run strategy 3: Long-term (2 months) - Adaptive RSI + Momentum
    println!("\n\n📊 STRATEGY 3: Long-Term (60-day options, 45-day hold)");
    let mut engine3 = BacktestEngine::new(config_long.clone());
    let historical_data_clone = historical_data.clone();
    let result3 = engine3.run_with_signals(
        symbol,
        historical_data.clone(),
        move |_symbol, spot, day_idx, hist_vols| {
            let mut signals = Vec::new();
            
            // Need at least 30 days of history for RSI
            if day_idx < 30 {
                return signals;
            }
            
            if let Some(&current_vol) = hist_vols.get(day_idx) {
                // Get price data for technical analysis
                let prices: Vec<(String, f64)> = historical_data_clone[..=day_idx]
                    .iter()
                    .map(|d| (d.date.clone(), d.close))
                    .collect();
                
                let rsi = calculate_rsi(&prices, 14);
                let momentum_10d = calculate_momentum(&prices, 10);
                let vol_zscore = calculate_vol_zscore(current_vol, &hist_vols[..day_idx], 20);
                
                // Adaptive thresholds based on volatility
                let (rsi_oversold, rsi_overbought, momentum_threshold) = if annual_vol > 50.0 {
                    (40.0, 60.0, 0.005)  // High-vol: wider bands, small momentum OK
                } else if annual_vol > 35.0 {
                    (35.0, 65.0, 0.01)   // Medium-vol: tighter bands, need more momentum
                } else {
                    (30.0, 70.0, 0.015)  // Low-vol: extreme bands, need strong momentum
                };
                
                // SIGNAL 1: Oversold + Positive Momentum
                if rsi < rsi_oversold && momentum_10d > momentum_threshold {
                    signals.push(SignalAction::BuyCall {
                        strike: spot * 1.02,  // 2% OTM for leverage
                        days_to_expiry: 60,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 2: Overbought + Negative Momentum
                else if rsi > rsi_overbought && momentum_10d < -momentum_threshold {
                    signals.push(SignalAction::BuyPut {
                        strike: spot * 0.98,  // 2% OTM
                        days_to_expiry: 60,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 3: Strong momentum breakout (only for high-vol stocks)
                else if annual_vol > 50.0 && momentum_10d.abs() > 0.03 && vol_zscore > 0.3 {
                    if momentum_10d > 0.0 {
                        signals.push(SignalAction::BuyCall {
                            strike: spot,
                            days_to_expiry: 60,
                            volatility: current_vol,
                        });
                    } else {
                        signals.push(SignalAction::BuyPut {
                            strike: spot,
                            days_to_expiry: 60,
                            volatility: current_vol,
                        });
                    }
                }
            }
            
            signals
        },
    );
    
    result3.print_summary();
    result3.print_trades(10);
    
    // Compare strategies
    println!("\n\n📈 HOLDING PERIOD COMPARISON - {}", symbol);
    println!("{}", "=".repeat(80));
    println!("{:<30} {:>15} {:>15} {:>15}",
             "Metric", "Short (14d)", "Medium (30d)", "Long (60d)");
    println!("{}", "-".repeat(80));
    
    println!("{:<30} {:>14.2}% {:>14.2}% {:>14.2}%",
             "Total Return",
             result.metrics.total_return_pct,
             result2.metrics.total_return_pct,
             result3.metrics.total_return_pct);
    
    println!("{:<30} {:>15.2} {:>15.2} {:>15.2}",
             "Sharpe Ratio",
             result.metrics.sharpe_ratio,
             result2.metrics.sharpe_ratio,
             result3.metrics.sharpe_ratio);
    
    println!("{:<30} {:>14.2}% {:>14.2}% {:>14.2}%",
             "Win Rate",
             result.metrics.win_rate,
             result2.metrics.win_rate,
             result3.metrics.win_rate);
    
    println!("{:<30} {:>14.2}% {:>14.2}% {:>14.2}%",
             "Max Drawdown",
             result.metrics.max_drawdown_pct,
             result2.metrics.max_drawdown_pct,
             result3.metrics.max_drawdown_pct);
    
    println!("{:<30} {:>15} {:>15} {:>15}",
             "Total Trades",
             result.metrics.total_trades,
             result2.metrics.total_trades,
             result3.metrics.total_trades);
    
    println!("{:<30} {:>15.2} {:>15.2} {:>15.2}",
             "Avg Days Held",
             result.metrics.avg_days_held,
             result2.metrics.avg_days_held,
             result3.metrics.avg_days_held);
    
    println!("{:<30} {:>15.2} {:>15.2} {:>15.2}",
             "Profit Factor",
             result.metrics.profit_factor,
             result2.metrics.profit_factor,
             result3.metrics.profit_factor);
    
    println!("{}", "=".repeat(80));
    
    // Determine best holding period
    let best_sharpe = result.metrics.sharpe_ratio
        .max(result2.metrics.sharpe_ratio)
        .max(result3.metrics.sharpe_ratio);
    
    if result.metrics.sharpe_ratio == best_sharpe {
        println!("🏆 WINNER: Short-Term (14-day) - Best Sharpe Ratio: {:.2}", best_sharpe);
    } else if result2.metrics.sharpe_ratio == best_sharpe {
        println!("🏆 WINNER: Medium-Term (30-day) - Best Sharpe Ratio: {:.2}", best_sharpe);
    } else {
        println!("🏆 WINNER: Long-Term (60-day) - Best Sharpe Ratio: {:.2}", best_sharpe);
    }
    
    Ok(())
}
