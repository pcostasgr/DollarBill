// Backtest example using Heston model for option pricing
// Tests real volatility and momentum-based trading strategies with accurate P&L
// Config structs hold fields for planned strategy differentiation; not all are read yet.
#![allow(dead_code)]

use dollarbill::market_data::csv_loader::{load_csv_closes, HistoricalDay};
use dollarbill::models::heston_analytical::heston_call_carr_madan;
use dollarbill::models::heston::HestonParams;
use dollarbill::market_data::symbols::load_enabled_stocks;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::collections::HashMap;

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
}

#[derive(Debug, Deserialize)]
struct LongTermConfig {
    initial_capital: f64,
    days_to_expiry: usize,
    max_days_hold: usize,
    ma_short_period: usize,
    ma_long_period: usize,
}

// Heston parameters structure
#[derive(Debug, Deserialize, Serialize, Clone)]
struct HestonParamsLocal {
    kappa: f64,
    theta: f64,
    sigma: f64,
    rho: f64,
    v0: f64,
}

#[derive(Debug, Deserialize)]
struct HestonCalibrationData {
    symbol: String,
    spot_price: f64,
    heston_params: HestonParamsLocal,
    rmse: f64,
}

// Helper: Calculate momentum
fn calculate_momentum(prices: &[HistoricalDay], period: usize) -> f64 {
    if prices.len() < period + 1 {
        return 0.0;
    }
    let past = prices[prices.len() - period - 1].close;
    let current = prices[prices.len() - 1].close;
    (current - past) / past
}

// Helper: Calculate RSI
fn calculate_rsi(prices: &[HistoricalDay], period: usize) -> f64 {
    if prices.len() < period + 1 {
        return 50.0;
    }

    let mut gains = 0.0;
    let mut losses = 0.0;

    for i in (prices.len() - period)..prices.len() {
        let change = prices[i].close - prices[i - 1].close;
        if change > 0.0 {
            gains += change;
        } else {
            losses -= change;
        }
    }

    if losses == 0.0 {
        return 100.0;
    }

    let rs = gains / losses;
    100.0 - (100.0 / (1.0 + rs))
}

// Helper: Calculate moving average
fn calculate_ma(prices: &[HistoricalDay], period: usize) -> f64 {
    if prices.len() < period {
        return prices.last().unwrap().close;
    }

    let sum: f64 = prices.iter().rev().take(period).map(|p| p.close).sum();
    sum / period as f64
}

// Helper: Calculate historical volatility
fn calculate_historical_volatility(prices: &[HistoricalDay]) -> f64 {
    if prices.len() < 30 {
        return 0.0;
    }

    // Calculate daily returns
    let mut returns = Vec::new();
    for i in 1..prices.len() {
        let daily_return = (prices[i].close / prices[i - 1].close) - 1.0;
        returns.push(daily_return);
    }

    // Calculate standard deviation of returns
    let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance: f64 = returns.iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>() / returns.len() as f64;
    let daily_vol = variance.sqrt();

    // Annualize (252 trading days)
    daily_vol * (252.0_f64).sqrt() * 100.0  // Return as percentage
}

// Load Heston parameters for a symbol — used only for initial guess / fallback.
fn load_heston_params(symbol: &str) -> Result<HestonCalibrationData, Box<dyn Error>> {
    let filename = format!("data/{}_heston_params.json", symbol.to_lowercase());
    let content = fs::read_to_string(&filename)?;
    let params: HestonCalibrationData = serde_json::from_str(&content)?;
    Ok(params)
}

/// Estimate Heston-like parameters from a rolling window of price history ending at
/// `prices[..=up_to_index]`.  Only data that would be available at the simulated
/// trade date is used — **no look-ahead**.
///
/// Method: moment-matching on the realized variance series.
/// - `v0`    = short-window (21-day) realised variance
/// - `theta` = long-window (252-day) realised variance  
/// - `kappa` = Yule-Walker AR(1) mean-reversion speed of the 21-day rolling variance
/// - `sigma` = standard deviation of the 21-day rolling variance series (vol-of-vol)
/// - `rho`   = Pearson correlation between daily returns and daily |return| changes
///             (proxy for the spot-vol correlation)
///
/// Falls back to `fallback` when there is insufficient history.
fn estimate_rolling_heston_params(
    prices: &[HistoricalDay],
    up_to_index: usize,
    fallback: &HestonParamsLocal,
) -> HestonParamsLocal {
    const SHORT_WIN: usize = 21;
    const LONG_WIN: usize = 252;
    const VAR_WIN: usize = 63; // 3-month window for kappa/sigma estimation

    let end = up_to_index + 1; // exclusive upper bound
    if end < SHORT_WIN + 2 {
        return fallback.clone();
    }

    let slice = &prices[..end];

    // Daily log-returns
    let returns: Vec<f64> = slice
        .windows(2)
        .map(|w| (w[1].close / w[0].close).ln())
        .collect();

    if returns.len() < SHORT_WIN {
        return fallback.clone();
    }

    // ── v0: short-window realised variance ──────────────────────────────
    let short_slice = &returns[returns.len().saturating_sub(SHORT_WIN)..];
    let short_mean = short_slice.iter().sum::<f64>() / short_slice.len() as f64;
    let v0 = (short_slice.iter().map(|r| (r - short_mean).powi(2)).sum::<f64>()
        / short_slice.len() as f64)
        * 252.0; // annualise
    let v0 = v0.max(1e-6);

    // ── theta: long-window realised variance ────────────────────────────
    let long_slice = &returns[returns.len().saturating_sub(LONG_WIN)..];
    let long_mean = long_slice.iter().sum::<f64>() / long_slice.len() as f64;
    let theta = (long_slice.iter().map(|r| (r - long_mean).powi(2)).sum::<f64>()
        / long_slice.len() as f64)
        * 252.0;
    let theta = theta.max(1e-6);

    // ── Rolling 21-day variance series (for kappa and sigma) ────────────
    let var_end = returns.len();
    let var_start = var_end.saturating_sub(VAR_WIN + SHORT_WIN);
    let mut var_series: Vec<f64> = Vec::new();
    for j in (var_start + SHORT_WIN)..=var_end {
        let w = &returns[j.saturating_sub(SHORT_WIN)..j];
        let m = w.iter().sum::<f64>() / w.len() as f64;
        let v = (w.iter().map(|r| (r - m).powi(2)).sum::<f64>() / w.len() as f64) * 252.0;
        var_series.push(v.max(1e-6));
    }

    // ── kappa: Yule-Walker AR(1) on the variance series ─────────────────
    let kappa = if var_series.len() >= 4 {
        let n = var_series.len() as f64;
        let var_mean = var_series.iter().sum::<f64>() / n;
        let gamma0: f64 = var_series.iter().map(|v| (v - var_mean).powi(2)).sum::<f64>() / n;
        let gamma1: f64 = var_series
            .windows(2)
            .map(|w| (w[0] - var_mean) * (w[1] - var_mean))
            .sum::<f64>()
            / (n - 1.0);
        // AR(1) coefficient φ; then kappa ≈ -ln(φ) * 252 (daily → annual)
        let phi = if gamma0 > 1e-12 { (gamma1 / gamma0).clamp(-0.9999, 0.9999) } else { 0.5 };
        (-phi.ln() * 252.0).clamp(0.1, 20.0)
    } else {
        fallback.kappa
    };

    // ── sigma: annualised std-dev of the rolling variance series ─────────
    let sigma = if var_series.len() >= 4 {
        let n = var_series.len() as f64;
        let vm = var_series.iter().sum::<f64>() / n;
        let vv = (var_series.iter().map(|v| (v - vm).powi(2)).sum::<f64>() / n).sqrt();
        // Scale to vol-of-vol units: σ in the Heston SDE is the std dev of √v
        // Approximation: σ_Heston ≈ std(Δ√v) * √252 / √(θ)
        (vv * (252.0_f64).sqrt() / theta.sqrt()).clamp(0.05, 3.0)
    } else {
        fallback.sigma
    };

    // ── rho: spot-return vs Δ|return| correlation ────────────────────────
    let rho = if returns.len() >= SHORT_WIN * 2 {
        let r_slice = &returns[returns.len().saturating_sub(SHORT_WIN * 2)..];
        let abs_ret: Vec<f64> = r_slice.iter().map(|r| r.abs()).collect();
        let d_abs: Vec<f64> = abs_ret.windows(2).map(|w| w[1] - w[0]).collect();
        let rets_trim = &r_slice[1..]; // align with d_abs
        let n = d_abs.len() as f64;
        let mr = rets_trim.iter().sum::<f64>() / n;
        let md = d_abs.iter().sum::<f64>() / n;
        let cov: f64 = rets_trim.iter().zip(d_abs.iter()).map(|(r, d)| (r - mr) * (d - md)).sum::<f64>() / n;
        let sr = (rets_trim.iter().map(|r| (r - mr).powi(2)).sum::<f64>() / n).sqrt();
        let sd = (d_abs.iter().map(|d| (d - md).powi(2)).sum::<f64>() / n).sqrt();
        if sr > 1e-12 && sd > 1e-12 { (cov / (sr * sd)).clamp(-0.9999, 0.9999) } else { fallback.rho }
    } else {
        fallback.rho
    };

    HestonParamsLocal { kappa, theta, sigma, rho, v0 }
}

// Price option using Heston model
fn price_option_with_heston(
    spot: f64,
    strike: f64,
    time_to_expiry: f64,
    rate: f64,
    heston_params: &HestonParamsLocal,
    is_call: bool,
) -> f64 {
    let params = HestonParams {
        s0: spot,
        v0: heston_params.v0,
        kappa: heston_params.kappa,
        theta: heston_params.theta,
        sigma: heston_params.sigma,
        rho: heston_params.rho,
        r: rate,
        t: time_to_expiry,
    };

    if is_call {
        heston_call_carr_madan(spot, strike, time_to_expiry, rate, &params)
    } else {
        // For put: use put-call parity
        let call_price = heston_call_carr_madan(spot, strike, time_to_expiry, rate, &params);
        call_price - spot + strike * (-rate * time_to_expiry).exp()
    }
}

fn backtest_symbol_with_heston(
    symbol: &str,
    config: &StrategyConfig,
    heston_params: &HestonCalibrationData,
) -> Result<(), Box<dyn Error>> {
    // Load historical data
    let csv_file = format!("data/{}_five_year.csv", symbol.to_lowercase());
    let mut historical_data = load_csv_closes(&csv_file)?;

    // Reverse so we iterate forward through time (oldest first)
    historical_data.reverse();

    println!("  Loaded {} days of historical data", historical_data.len());
    println!("  Heston fallback params (used only when history < 23 days): κ={:.4}, θ={:.4}, σ={:.4}, ρ={:.4}, v₀={:.4}",
             heston_params.heston_params.kappa,
             heston_params.heston_params.theta,
             heston_params.heston_params.sigma,
             heston_params.heston_params.rho,
             heston_params.heston_params.v0);
    println!("  ⚡ Rolling calibration active — params re-estimated per trade date from trailing price history");

    // Measure historical volatility to select appropriate strategy
    let hist_vol = calculate_historical_volatility(&historical_data);
    println!("  Measured Historical Volatility: {:.1}% annualized", hist_vol);

    let strategy_type = if hist_vol > config.strategies.short_term.vol_threshold_high_vol {
        "HIGH-VOL"
    } else if hist_vol > config.strategies.short_term.vol_threshold_medium_vol {
        "MEDIUM-VOL"
    } else {
        "LOW-VOL"
    };

    println!("  🎯 Strategy: {} Momentum (Heston pricing)", strategy_type);

    if strategy_type == "LOW-VOL" {
        println!("  ⚠️  LOW-VOL (options buying not recommended - skipping)");
        return Ok(());
    }

    // Test Short-Term strategy
    println!("\n📊 STRATEGY Short-Term: Short-Term ({}-day options, {}-day hold)",
             config.strategies.short_term.days_to_expiry,
             config.strategies.short_term.max_days_hold);

    let results = run_heston_backtest_short(&historical_data, config, &config.strategies.short_term, heston_params)?;
    let mut best_sharpe = results.sharpe;
    let mut winner = "Short-Term";

    // Test Medium-Term strategy
    println!("\n📊 STRATEGY Medium-Term: Medium-Term ({}-day options, {}-day hold)",
             config.strategies.medium_term.days_to_expiry,
             config.strategies.medium_term.max_days_hold);

    let results = run_heston_backtest_medium(&historical_data, config, &config.strategies.medium_term, heston_params)?;
    if results.sharpe > best_sharpe {
        best_sharpe = results.sharpe;
        winner = "Medium-Term";
    }

    // Test Long-Term strategy
    println!("\n📊 STRATEGY Long-Term: Long-Term ({}-day options, {}-day hold)",
             config.strategies.long_term.days_to_expiry,
             config.strategies.long_term.max_days_hold);

    let results = run_heston_backtest_long(&historical_data, config, &config.strategies.long_term, heston_params)?;
    if results.sharpe > best_sharpe {
        best_sharpe = results.sharpe;
        winner = "Long-Term";
    }

    const MIN_WINNER_SHARPE: f64 = 1.0;
    if best_sharpe >= MIN_WINNER_SHARPE {
        println!("\n🏆 WINNER: {} - Best Sharpe Ratio: {:.2}", winner, best_sharpe);
    } else {
        println!("\n⚠️  No strategy qualified (min Sharpe {:.1}) — best was {} at {:.2}",
                 MIN_WINNER_SHARPE, winner, best_sharpe);
    }

    Ok(())
}

#[derive(Debug)]
struct BacktestResults {
    total_pnl: f64,
    sharpe: f64,
    max_drawdown: f64,
    win_rate: f64,
    total_trades: usize,
    avg_days_held: f64,
    profit_factor: f64,
}

fn run_heston_backtest_short(
    historical_data: &[HistoricalDay],
    config: &StrategyConfig,
    strategy_config: &ShortTermConfig,
    heston_params: &HestonCalibrationData,
) -> Result<BacktestResults, Box<dyn Error>> {
    run_heston_backtest_impl(historical_data, config, strategy_config, heston_params)
}

fn run_heston_backtest_medium(
    historical_data: &[HistoricalDay],
    config: &StrategyConfig,
    strategy_config: &MediumTermConfig,
    heston_params: &HestonCalibrationData,
) -> Result<BacktestResults, Box<dyn Error>> {
    run_heston_backtest_impl(historical_data, config, strategy_config, heston_params)
}

fn run_heston_backtest_long(
    historical_data: &[HistoricalDay],
    config: &StrategyConfig,
    strategy_config: &LongTermConfig,
    heston_params: &HestonCalibrationData,
) -> Result<BacktestResults, Box<dyn Error>> {
    run_heston_backtest_impl(historical_data, config, strategy_config, heston_params)
}

fn run_heston_backtest_impl<T>(
    historical_data: &[HistoricalDay],
    config: &StrategyConfig,
    strategy_config: &T,
    heston_params: &HestonCalibrationData,
) -> Result<BacktestResults, Box<dyn Error>>
where
    T: StrategyConfigTrait,
{
    let initial_cap = strategy_config.get_initial_capital();
    let mut capital = initial_cap;
    let mut positions: Vec<Position> = Vec::new();
    let mut equity_curve = vec![capital];
    let mut trade_log = Vec::new();

    let lookback_period = 50; // For technical indicators

    for i in lookback_period..historical_data.len() {
        let current_date = &historical_data[i].date;
        let current_price = historical_data[i].close;

        // Update existing positions
        let mut positions_to_remove = Vec::new();
        for (idx, pos) in positions.iter_mut().enumerate() {
            let days_held = (i - pos.entry_index) as f64;

            // Check exit conditions
            if days_held >= pos.max_hold_days as f64 {
                // Exit position: reprice with rolling params as of exit date
                let rolling_exit_params = estimate_rolling_heston_params(
                    historical_data,
                    i,
                    &heston_params.heston_params,
                );
                let time_to_expiry = (pos.expiry_days as f64 - days_held) / 365.0;
                let exit_price = price_option_with_heston(
                    current_price,
                    pos.strike,
                    time_to_expiry.max(0.01),
                    config.backtest.risk_free_rate,
                    &rolling_exit_params,
                    pos.is_call,
                );

                let pnl = (exit_price - pos.entry_price) * pos.quantity as f64 * 100.0 - config.backtest.commission_per_trade;
                capital += pnl;

                trade_log.push(TradeRecord {
                    entry_date: historical_data[pos.entry_index].date.clone(),
                    exit_date: current_date.clone(),
                    symbol: pos.symbol.clone(),
                    option_type: if pos.is_call { "CALL".to_string() } else { "PUT".to_string() },
                    strike: pos.strike,
                    entry_price: pos.entry_price,
                    exit_price,
                    days_held: days_held as usize,
                    pnl,
                    result: if pnl > 0.0 { "WIN" } else { "LOSS" },
                });

                positions_to_remove.push(idx);
            }
        }

        // Remove positions that were closed
        for &idx in positions_to_remove.iter().rev() {
            positions.remove(idx);
        }

        // Check for new signals (halt new entries if 20% portfolio drawdown reached)
        if positions.len() < config.backtest.max_positions && capital >= initial_cap * 0.80 {
            if let Some(signal) = generate_signal(historical_data, i, lookback_period, &heston_params.symbol) {
                // ── Rolling calibration: only use data up to today ──────────────
                // This mirrors the lagged information a live trader would have and
                // eliminates the look-ahead bias from full-period pre-calibrated params.
                let rolling_params = estimate_rolling_heston_params(
                    historical_data,
                    i,
                    &heston_params.heston_params,
                );
                // Calculate option price using rolling (non-look-ahead) Heston params
                let time_to_expiry = strategy_config.get_days_to_expiry() as f64 / 365.0;
                let option_price = price_option_with_heston(
                    current_price,
                    signal.strike,
                    time_to_expiry,
                    config.backtest.risk_free_rate,
                    &rolling_params,
                    signal.is_call,
                );

                // Fixed 2% of initial capital per trade — avoids runaway exponential sizing
                let position_size = initial_cap * 0.02;
                let max_quantity = (position_size / (option_price * 100.0)).floor() as i32; // Each contract = 100 shares
                let quantity = max_quantity.min(5).max(1); // Cap at 5 contracts to limit concentration

                if quantity > 0 {
                    positions.push(Position {
                        symbol: signal.symbol,
                        entry_index: i,
                        strike: signal.strike,
                        entry_price: option_price,
                        quantity,
                        max_hold_days: strategy_config.get_max_days_hold(),
                        expiry_days: strategy_config.get_days_to_expiry(),
                        is_call: signal.is_call,
                    });

                    capital -= option_price * quantity as f64 * 100.0 + config.backtest.commission_per_trade;
                }
            }
        }

        equity_curve.push(capital);
    }

    // Calculate metrics
    let total_pnl = capital - strategy_config.get_initial_capital();
    let returns: Vec<f64> = equity_curve.windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();

    let sharpe = if returns.len() > 1 {
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let std_return = (returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / returns.len() as f64).sqrt();
        if std_return > 0.0 {
            (mean_return / std_return) * (252.0_f64).sqrt()
        } else {
            0.0
        }
    } else {
        0.0
    };

    let max_drawdown = calculate_max_drawdown(&equity_curve);
    let win_rate = trade_log.iter().filter(|t| t.pnl > 0.0).count() as f64 / trade_log.len() as f64 * 100.0;
    let avg_days_held = trade_log.iter().map(|t| t.days_held).sum::<usize>() as f64 / trade_log.len() as f64;
    let profit_factor = {
        let winning_trades: f64 = trade_log.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let losing_trades: f64 = trade_log.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum();
        if losing_trades > 0.0 { winning_trades / losing_trades } else { f64::INFINITY }
    };

    // Print results
    println!("===========================================================================");
    println!("BACKTEST RESULTS - {}", heston_params.symbol);
    println!("===========================================================================");
    println!("Period: {} to {}", historical_data[lookback_period].date, historical_data.last().unwrap().date);
    println!("Initial Capital: ${:.2}", strategy_config.get_initial_capital());
    println!("Final Capital: ${:.2}", capital);
    println!("📊 PERFORMANCE METRICS");
    println!("---------------------------------------------------------------------------");
    println!("Total P&L:        ${:>12.2}  ({:>6.2}%)", total_pnl, (total_pnl / strategy_config.get_initial_capital()) * 100.0);
    println!("Sharpe Ratio:            {:.2}", sharpe);
    println!("Max Drawdown:     ${:>12.2}  ({:>6.2}%)", max_drawdown, (max_drawdown / strategy_config.get_initial_capital()) * 100.0);
    println!("📈 TRADE STATISTICS");
    println!("---------------------------------------------------------------------------");
    println!("Total Trades:                {}", trade_log.len());
    println!("Winning Trades:              {}", trade_log.iter().filter(|t| t.pnl > 0.0).count());
    println!("Average Win:      ${:>12.2}", trade_log.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum::<f64>() / trade_log.iter().filter(|t| t.pnl > 0.0).count().max(1) as f64);
    println!("Average Loss:     ${:>12.2}", trade_log.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl).sum::<f64>() / trade_log.iter().filter(|t| t.pnl < 0.0).count().max(1) as f64);
    println!("Profit Factor:            {:.2}", profit_factor);
    println!("Avg Days Held:             {:.1}", avg_days_held);
    println!("Total Commissions:${:>12.2}", config.backtest.commission_per_trade * trade_log.len() as f64);

    Ok(BacktestResults {
        total_pnl,
        sharpe,
        max_drawdown,
        win_rate,
        total_trades: trade_log.len(),
        avg_days_held,
        profit_factor,
    })
}

// Strategy config trait for polymorphism
trait StrategyConfigTrait {
    fn get_initial_capital(&self) -> f64;
    fn get_days_to_expiry(&self) -> usize;
    fn get_max_days_hold(&self) -> usize;
}

impl StrategyConfigTrait for ShortTermConfig {
    fn get_initial_capital(&self) -> f64 { self.initial_capital }
    fn get_days_to_expiry(&self) -> usize { self.days_to_expiry }
    fn get_max_days_hold(&self) -> usize { self.max_days_hold }
}

impl StrategyConfigTrait for MediumTermConfig {
    fn get_initial_capital(&self) -> f64 { self.initial_capital }
    fn get_days_to_expiry(&self) -> usize { self.days_to_expiry }
    fn get_max_days_hold(&self) -> usize { self.max_days_hold }
}

impl StrategyConfigTrait for LongTermConfig {
    fn get_initial_capital(&self) -> f64 { self.initial_capital }
    fn get_days_to_expiry(&self) -> usize { self.days_to_expiry }
    fn get_max_days_hold(&self) -> usize { self.max_days_hold }
}

#[derive(Debug)]
struct Signal {
    symbol: String,
    strike: f64,
    is_call: bool,
}

#[derive(Debug)]
struct Position {
    symbol: String,
    entry_index: usize,
    strike: f64,
    entry_price: f64,
    quantity: i32,
    max_hold_days: usize,
    expiry_days: usize,
    is_call: bool,
}

#[derive(Debug)]
struct TradeRecord {
    entry_date: String,
    exit_date: String,
    symbol: String,
    option_type: String,
    strike: f64,
    entry_price: f64,
    exit_price: f64,
    days_held: usize,
    pnl: f64,
    result: &'static str,
}

fn generate_signal(
    historical_data: &[HistoricalDay],
    current_index: usize,
    lookback: usize,
    symbol: &str,
) -> Option<Signal> {
    let current_price = historical_data[current_index].close;

    // Simple momentum signal - buy calls on upward momentum
    let momentum = calculate_momentum(&historical_data[current_index.saturating_sub(lookback)..=current_index], 5);

    if momentum > 0.02 { // 2% upward momentum
        Some(Signal {
            symbol: symbol.to_string(),
            strike: current_price * 1.05, // 5% OTM
            is_call: true,
        })
    } else {
        None
    }
}

fn calculate_max_drawdown(equity_curve: &[f64]) -> f64 {
    let mut max_drawdown = 0.0;
    let mut peak = equity_curve[0];

    for &value in equity_curve {
        if value > peak {
            peak = value;
        }
        let drawdown = peak - value;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    max_drawdown
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("\n{}", "=".repeat(80));
    println!("OPTIONS STRATEGY BACKTESTER WITH HESTON PRICING");
    println!("Historical Performance Analysis with Heston Model P&L");
    println!("{}", "=".repeat(80));
    println!();

    // Load configuration
    let config_content = fs::read_to_string("config/strategy_config.json")
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: StrategyConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    println!("📋 Loaded configuration from config/strategy_config.json");

    // Load enabled symbols
    let symbols = load_enabled_stocks()?;
    println!("🎯 Testing symbols: {:?}", symbols);
    println!();

    // Load Heston parameters for all symbols
    let mut heston_params_map = HashMap::new();
    for symbol in &symbols {
        match load_heston_params(symbol) {
            Ok(params) => {
                heston_params_map.insert(symbol.clone(), params);
            }
            Err(e) => {
                println!("⚠️  Failed to load Heston params for {}: {}", symbol, e);
            }
        }
    }

    // Test each symbol
    for symbol in &symbols {
        if let Some(heston_params) = heston_params_map.get(symbol) {
            println!("\n🔍 Backtesting {} with Heston pricing...", symbol);
            if let Err(e) = backtest_symbol_with_heston(symbol, &config, heston_params) {
                println!("❌ Backtest failed for {}: {}", symbol, e);
            }
        } else {
            println!("\n⚠️  Skipping {} - no Heston parameters available", symbol);
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("BACKTESTING COMPLETE");
    println!("{}", "=".repeat(80));

    Ok(())
}