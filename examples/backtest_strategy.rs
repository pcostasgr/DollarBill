// Backtest example - demonstrates the backtesting framework
// Tests real volatility and momentum-based trading strategies

use dollarbill::backtesting::{BacktestEngine, BacktestConfig, SignalAction, TradingCosts, SlippageModel, PartialFillModel};
use dollarbill::market_data::csv_loader::load_csv_closes;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;

// Configuration structures
#[derive(Debug, Deserialize)]
struct StockConfig {
    stocks: Vec<StockEntry>,
}

#[derive(Debug, Deserialize)]
struct StockEntry {
    symbol: String,
    enabled: bool,
}

// Function to load symbols from config file
fn load_symbols_from_config() -> Result<Vec<String>, Box<dyn Error>> {
    let config_path = "config/stocks.json";
    let config_content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read stocks config: {}", e))?;
    let config: StockConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse stocks config: {}", e))?;
    
    Ok(config.stocks.into_iter()
        .filter(|stock| stock.enabled)
        .map(|stock| stock.symbol)
        .collect())
}

// Configuration structures
#[derive(Debug, Deserialize)]
struct StrategyConfig {
    backtest: BacktestCommon,
    strategies: Strategies,
}

#[derive(Debug, Deserialize)]
struct BacktestCommon {
    commission_per_trade: f64,
    risk_free_rate: f64,
    max_positions: usize,
    position_size_pct: f64,
    stop_loss_pct: f64,
    take_profit_pct: f64,
}

#[derive(Debug, Deserialize)]
struct Strategies {
    short_term: ShortTermConfig,
    medium_term: MediumTermConfig,
    long_term: LongTermConfig,
}

#[derive(Debug, Deserialize)]
struct ShortTermConfig {
    initial_capital: f64,
    days_to_expiry: usize,
    max_days_hold: usize,
    vol_threshold_high_vol: f64,
    vol_threshold_medium_vol: f64,
    vol_threshold_low_vol: f64,
}

#[derive(Debug, Deserialize)]
struct MediumTermConfig {
    initial_capital: f64,
    days_to_expiry: usize,
    max_days_hold: usize,
    rsi_oversold: f64,
    rsi_overbought: f64,
    momentum_threshold: f64,
    vol_zscore_lookback: usize,
    strike_offsets: StrikeOffsets,
    momentum_breakout: MomentumBreakout,
}

#[derive(Debug, Deserialize)]
struct LongTermConfig {
    initial_capital: f64,
    days_to_expiry: usize,
    max_days_hold: usize,
    rsi_period: usize,
    momentum_period: usize,
    vol_zscore_lookback: usize,
    volatility_thresholds: VolatilityThresholds,
    rsi_momentum_thresholds: RsiMomentumThresholds,
    strike_offsets: StrikeOffsets,
    momentum_breakout: MomentumBreakout,
    rsi_divergence: RsiDivergence,
}

#[derive(Debug, Deserialize)]
struct VolatilityThresholds {
    high_vol_threshold: f64,
    medium_vol_threshold: f64,
}

#[derive(Debug, Deserialize)]
struct RsiMomentumThresholds {
    high_vol: VolThresholds,
    medium_vol: VolThresholds,
    low_vol: VolThresholds,
}

#[derive(Debug, Deserialize)]
struct VolThresholds {
    rsi_oversold: f64,
    rsi_overbought: f64,
    momentum_threshold: f64,
}

#[derive(Debug, Deserialize)]
struct StrikeOffsets {
    call_otm_pct: f64,
    put_otm_pct: f64,
}

#[derive(Debug, Deserialize)]
struct MomentumBreakout {
    momentum_threshold: f64,
    vol_zscore_threshold: f64,
    call_otm_pct: f64,
    put_otm_pct: f64,
}

#[derive(Debug, Deserialize)]
struct RsiDivergence {
    rsi_oversold: f64,
    rsi_overbought: f64,
    momentum_threshold: f64,
    call_otm_pct: f64,
    put_otm_pct: f64,
}

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
    
    // Load configuration
    let config_content = fs::read_to_string("config/strategy_config.json")
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: StrategyConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;
    
    println!("📋 Loaded configuration from config/strategy_config.json");

    // Parse command line arguments for symbols
    let args: Vec<String> = std::env::args().collect();
    let symbols_to_test: Vec<String> = if args.len() > 1 {
        args[1..].to_vec() // Skip the program name
    } else {
        // Load enabled symbols from config
        load_symbols_from_config().unwrap_or_else(|_| {
            println!("Warning: Could not load stocks.json, using default symbols");
            vec!["AAPL".to_string(), "NVDA".to_string(), "MSFT".to_string(), "AMZN".to_string()]
        })
    };

    println!("🎯 Testing symbols: {:?}", symbols_to_test);
    println!();

    // Test specified symbols
    for symbol in &symbols_to_test {
        println!("\n🔍 Backtesting {}...", symbol);
        backtest_symbol(symbol, &config)?;
    }
    
    Ok(())
}

fn backtest_symbol(symbol: &str, config: &StrategyConfig) -> Result<(), Box<dyn Error>> {
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
    if annual_vol > config.strategies.long_term.volatility_thresholds.high_vol_threshold {
        println!("  🎯 Strategy: HIGH-VOL Momentum (aggressive)");
    } else if annual_vol > config.strategies.long_term.volatility_thresholds.medium_vol_threshold {
        println!("  🎯 Strategy: MEDIUM-VOL Momentum (conservative)");
    } else {
        println!("  ⚠️  Strategy: LOW-VOL (options buying not recommended)");
    }
    
    // Configure backtest with different holding periods
    let config_short = BacktestConfig {
        initial_capital: config.strategies.short_term.initial_capital,
        trading_costs: TradingCosts { commission_per_contract: config.backtest.commission_per_trade, bid_ask_spread_percent: 0.0, slippage_model: SlippageModel::Fixed, ..TradingCosts::default() },
        risk_free_rate: config.backtest.risk_free_rate,
        max_positions: config.backtest.max_positions,
        position_size_pct: config.backtest.position_size_pct,
        days_to_expiry: config.strategies.short_term.days_to_expiry,
        max_days_hold: config.strategies.short_term.max_days_hold,
        stop_loss_pct: Some(config.backtest.stop_loss_pct),
        take_profit_pct: Some(config.backtest.take_profit_pct),
        use_portfolio_management: false,
        ..BacktestConfig::default()
    };
    
    let config_medium = BacktestConfig {
        initial_capital: config.strategies.medium_term.initial_capital,
        trading_costs: TradingCosts { commission_per_contract: config.backtest.commission_per_trade, bid_ask_spread_percent: 0.0, slippage_model: SlippageModel::Fixed, ..TradingCosts::default() },
        risk_free_rate: config.backtest.risk_free_rate,
        max_positions: config.backtest.max_positions,
        position_size_pct: config.backtest.position_size_pct,
        days_to_expiry: config.strategies.medium_term.days_to_expiry,
        max_days_hold: config.strategies.medium_term.max_days_hold,
        stop_loss_pct: Some(config.backtest.stop_loss_pct),
        take_profit_pct: Some(config.backtest.take_profit_pct),
        use_portfolio_management: false,
        ..BacktestConfig::default()
    };
    
    // Long-term strategy uses FullMarketImpact: realistic liquidity cost model combining
    // small-cap spread widening × √contract size impact × panic-driven blow-out.
    // cap_multiplier=1.5 (large-cap bias), size_impact_bps=8 (moderate impact),
    // normal_vol=0.25 (25% is the long-run avg), panic_exponent=2.0 (quadratic widening).
    let config_long = BacktestConfig {
        initial_capital: config.strategies.long_term.initial_capital,
        trading_costs: TradingCosts {
            commission_per_contract: config.backtest.commission_per_trade,
            bid_ask_spread_percent: 1.0,
            slippage_model: SlippageModel::FullMarketImpact {
                cap_multiplier: 1.5,
                size_impact_bps: 8.0,
                normal_vol: 0.25,
                panic_exponent: 2.0,
            },
            partial_fill_model: PartialFillModel::VolScaled {
                normal_vol: 0.25,
                min_fill_rate: 0.40,
            },
        },
        risk_free_rate: config.backtest.risk_free_rate,
        max_positions: config.backtest.max_positions,
        position_size_pct: config.backtest.position_size_pct,
        days_to_expiry: config.strategies.long_term.days_to_expiry,
        max_days_hold: config.strategies.long_term.max_days_hold,
        stop_loss_pct: Some(config.backtest.stop_loss_pct),
        take_profit_pct: Some(config.backtest.take_profit_pct),
        use_portfolio_management: false,
        ..BacktestConfig::default()
    };
    
    // Run strategy 1: Short-term (adaptive based on volatility)
    println!("\n📊 STRATEGY 1: Short-Term (14-day options, 10-day hold)");
    let mut engine = BacktestEngine::new(config_short.clone());
    
    // Adjust threshold based on volatility
    let vol_threshold = if annual_vol > config.strategies.long_term.volatility_thresholds.high_vol_threshold {
        config.strategies.short_term.vol_threshold_high_vol
    } else if annual_vol > config.strategies.long_term.volatility_thresholds.medium_vol_threshold {
        config.strategies.short_term.vol_threshold_medium_vol
    } else {
        config.strategies.short_term.vol_threshold_low_vol
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
                let vol_zscore = calculate_vol_zscore(current_vol, &hist_vols[..day_idx], config.strategies.medium_term.vol_zscore_lookback);
                
                // SIGNAL 1: RSI oversold with positive momentum
                if rsi < config.strategies.medium_term.rsi_oversold && momentum_10d > config.strategies.medium_term.momentum_threshold {
                    signals.push(SignalAction::BuyCall {
                        strike: spot * config.strategies.medium_term.strike_offsets.call_otm_pct,
                        days_to_expiry: config.strategies.medium_term.days_to_expiry,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 2: RSI overbought with negative momentum
                else if rsi > config.strategies.medium_term.rsi_overbought && momentum_10d < -config.strategies.medium_term.momentum_threshold {
                    signals.push(SignalAction::BuyPut {
                        strike: spot * config.strategies.medium_term.strike_offsets.put_otm_pct,
                        days_to_expiry: config.strategies.medium_term.days_to_expiry,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 3: Strong momentum with volatility spike
                if momentum_10d.abs() > config.strategies.medium_term.momentum_breakout.momentum_threshold && vol_zscore > config.strategies.medium_term.momentum_breakout.vol_zscore_threshold {
                    if momentum_10d > 0.0 {
                        signals.push(SignalAction::BuyCall {
                            strike: spot * config.strategies.medium_term.momentum_breakout.call_otm_pct,
                            days_to_expiry: config.strategies.medium_term.days_to_expiry,
                            volatility: current_vol,
                        });
                    } else {
                        signals.push(SignalAction::BuyPut {
                            strike: spot * config.strategies.medium_term.momentum_breakout.put_otm_pct,
                            days_to_expiry: config.strategies.medium_term.days_to_expiry,
                            volatility: current_vol,
                        });
                    }
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
                
                let rsi = calculate_rsi(&prices, config.strategies.long_term.rsi_period);
                let momentum_10d = calculate_momentum(&prices, config.strategies.long_term.momentum_period);
                let vol_zscore = calculate_vol_zscore(current_vol, &hist_vols[..day_idx], config.strategies.long_term.vol_zscore_lookback);
                
                // Adaptive thresholds based on volatility - RELAXED for less conservative approach
                let (rsi_oversold, rsi_overbought, momentum_threshold) = if annual_vol > config.strategies.long_term.volatility_thresholds.high_vol_threshold {
                    (config.strategies.long_term.rsi_momentum_thresholds.high_vol.rsi_oversold,
                     config.strategies.long_term.rsi_momentum_thresholds.high_vol.rsi_overbought,
                     config.strategies.long_term.rsi_momentum_thresholds.high_vol.momentum_threshold)
                } else if annual_vol > config.strategies.long_term.volatility_thresholds.medium_vol_threshold {
                    (config.strategies.long_term.rsi_momentum_thresholds.medium_vol.rsi_oversold,
                     config.strategies.long_term.rsi_momentum_thresholds.medium_vol.rsi_overbought,
                     config.strategies.long_term.rsi_momentum_thresholds.medium_vol.momentum_threshold)
                } else {
                    (config.strategies.long_term.rsi_momentum_thresholds.low_vol.rsi_oversold,
                     config.strategies.long_term.rsi_momentum_thresholds.low_vol.rsi_overbought,
                     config.strategies.long_term.rsi_momentum_thresholds.low_vol.momentum_threshold)
                };
                
                // SIGNAL 1: Oversold + Positive Momentum
                if rsi < rsi_oversold && momentum_10d > momentum_threshold {
                    signals.push(SignalAction::BuyCall {
                        strike: spot * config.strategies.long_term.strike_offsets.call_otm_pct,
                        days_to_expiry: config.strategies.long_term.days_to_expiry,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 2: Overbought + Negative Momentum
                else if rsi > rsi_overbought && momentum_10d < -momentum_threshold {
                    signals.push(SignalAction::BuyPut {
                        strike: spot * config.strategies.long_term.strike_offsets.put_otm_pct,
                        days_to_expiry: config.strategies.long_term.days_to_expiry,
                        volatility: current_vol,
                    });
                }
                
                // SIGNAL 3: Strong momentum breakout (relaxed for all stocks)
                if momentum_10d.abs() > config.strategies.long_term.momentum_breakout.momentum_threshold && vol_zscore > config.strategies.long_term.momentum_breakout.vol_zscore_threshold {
                    if momentum_10d > 0.0 {
                        signals.push(SignalAction::BuyCall {
                            strike: spot * config.strategies.long_term.momentum_breakout.call_otm_pct,
                            days_to_expiry: config.strategies.long_term.days_to_expiry,
                            volatility: current_vol,
                        });
                    } else {
                        signals.push(SignalAction::BuyPut {
                            strike: spot * config.strategies.long_term.momentum_breakout.put_otm_pct,
                            days_to_expiry: config.strategies.long_term.days_to_expiry,
                            volatility: current_vol,
                        });
                    }
                }
                
                // SIGNAL 4: RSI divergence with moderate momentum (new signal)
                else if (rsi < config.strategies.long_term.rsi_divergence.rsi_oversold && momentum_10d > config.strategies.long_term.rsi_divergence.momentum_threshold) || 
                        (rsi > config.strategies.long_term.rsi_divergence.rsi_overbought && momentum_10d < -config.strategies.long_term.rsi_divergence.momentum_threshold) {
                    if rsi < config.strategies.long_term.rsi_divergence.rsi_oversold {
                        signals.push(SignalAction::BuyCall {
                            strike: spot * config.strategies.long_term.rsi_divergence.call_otm_pct,
                            days_to_expiry: config.strategies.long_term.days_to_expiry,
                            volatility: current_vol,
                        });
                    } else {
                        signals.push(SignalAction::BuyPut {
                            strike: spot * config.strategies.long_term.rsi_divergence.put_otm_pct,
                            days_to_expiry: config.strategies.long_term.days_to_expiry,
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
