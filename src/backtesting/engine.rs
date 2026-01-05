// Backtesting engine - orchestrates historical simulation

use crate::backtesting::position::{Position, PositionStatus, OptionType};
use crate::backtesting::trade::{Trade, TradeType};
use crate::backtesting::metrics::{BacktestResult, PerformanceMetrics, EquityCurve};
use crate::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use crate::market_data::csv_loader::HistoricalDay;

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub initial_capital: f64,
    pub commission_per_trade: f64,
    pub risk_free_rate: f64,
    pub max_positions: usize,
    pub position_size_pct: f64,  // Percentage of capital per position
    pub days_to_expiry: usize,  // Option expiration in days
    pub max_days_hold: usize,  // Maximum days to hold before forced close
    pub stop_loss_pct: Option<f64>,  // Optional stop loss
    pub take_profit_pct: Option<f64>,  // Optional take profit
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            commission_per_trade: 1.0,
            risk_free_rate: 0.05,
            max_positions: 10,
            position_size_pct: 10.0,
            days_to_expiry: 30,  // 30-day options
            max_days_hold: 21,  // Close after 21 days (70% of expiry)
            stop_loss_pct: Some(50.0),  // 50% stop loss
            take_profit_pct: Some(100.0),  // 100% take profit
        }
    }
}

pub struct BacktestEngine {
    config: BacktestConfig,
    positions: Vec<Position>,
    trades: Vec<Trade>,
    equity_curve: EquityCurve,
    current_capital: f64,
    position_counter: usize,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            current_capital: config.initial_capital,
            config,
            positions: Vec::new(),
            trades: Vec::new(),
            equity_curve: EquityCurve::new(),
            position_counter: 0,
        }
    }
    
    /// Run backtest on historical data with a simple volatility strategy
    /// Buys calls when IV is low, sells when IV normalizes
    pub fn run_simple_strategy(
        &mut self,
        symbol: &str,
        historical_data: Vec<HistoricalDay>,
        volatility_threshold: f64,  // IV threshold for entry
    ) -> BacktestResult {
        if historical_data.is_empty() {
            return self.generate_result(symbol, "N/A".to_string(), "N/A".to_string());
        }
        
        let start_date = historical_data.last().unwrap().date.clone();
        let end_date = historical_data.first().unwrap().date.clone();
        
        // Calculate historical volatility for each day
        let hist_vols = self.calculate_rolling_volatility(&historical_data, 20);
        
        // Iterate through each day
        for (day_idx, day) in historical_data.iter().enumerate() {
            let spot = day.close;
            
            // Don't open new positions if we're too close to the end of backtest
            let days_remaining = historical_data.len().saturating_sub(day_idx + 1);
            let can_trade = days_remaining >= self.config.max_days_hold;
            
            // Update equity curve
            self.update_open_positions(&day.date, spot, &hist_vols, day_idx);
            let total_equity = self.current_capital + self.unrealized_pnl();
            self.equity_curve.add_point(day.date.clone(), total_equity);
            
            // Generate signals based on volatility
            if let Some(&hist_vol) = hist_vols.get(day_idx) {
                // Strategy: Buy ATM calls when vol is below threshold
                if can_trade && hist_vol < volatility_threshold && self.can_open_position() {
                    self.open_call_position(
                        symbol,
                        spot,
                        spot,  // ATM strike
                        30,    // 30 days to expiry
                        hist_vol,
                        &day.date,
                    );
                }
            }
            
            // Check exit conditions for all open positions
            self.check_exit_conditions(&day.date, spot, &hist_vols, day_idx);
        }
        
        // Close all remaining positions at end
        let final_day = historical_data.first().unwrap();
        self.close_all_positions(&final_day.date, final_day.close);
        
        self.generate_result(symbol, start_date, end_date)
    }
    
    /// Run backtest with custom signal generator function
    pub fn run_with_signals<F>(
        &mut self,
        symbol: &str,
        historical_data: Vec<HistoricalDay>,
        mut signal_fn: F,
    ) -> BacktestResult
    where
        F: FnMut(&str, f64, usize, &[f64]) -> Vec<SignalAction>,
    {
        if historical_data.is_empty() {
            return self.generate_result(symbol, "N/A".to_string(), "N/A".to_string());
        }
        
        let start_date = historical_data.last().unwrap().date.clone();
        let end_date = historical_data.first().unwrap().date.clone();
        
        let hist_vols = self.calculate_rolling_volatility(&historical_data, 20);
        
        for (day_idx, day) in historical_data.iter().enumerate() {
            let spot = day.close;
            
            // Don't open new positions if we're too close to the end of backtest
            // (not enough remaining data to properly manage the position)
            let days_remaining = historical_data.len().saturating_sub(day_idx + 1);
            let can_trade = days_remaining >= self.config.max_days_hold;
            
            // Update positions and equity
            self.update_open_positions(&day.date, spot, &hist_vols, day_idx);
            let total_equity = self.current_capital + self.unrealized_pnl();
            self.equity_curve.add_point(day.date.clone(), total_equity);
            
            // Get signals from custom function
            let signals = signal_fn(symbol, spot, day_idx, &hist_vols);
            
            // Execute signals
            for signal in signals {
                if can_trade && self.can_open_position() {
                    match signal {
                        SignalAction::BuyCall { strike, days_to_expiry, volatility } => {
                            self.open_call_position(symbol, spot, strike, days_to_expiry, volatility, &day.date);
                        },
                        SignalAction::BuyPut { strike, days_to_expiry, volatility } => {
                            self.open_put_position(symbol, spot, strike, days_to_expiry, volatility, &day.date);
                        },
                        SignalAction::ClosePosition { position_id } => {
                            if let Some(hist_vol) = hist_vols.get(day_idx) {
                                self.close_position_by_id(position_id, &day.date, spot, *hist_vol, 0);
                            }
                        },
                    }
                }
            }
            
            // Check exit conditions for all open positions
            self.check_exit_conditions(&day.date, spot, &hist_vols, day_idx);
        }
        
        // Close all remaining positions
        let final_day = historical_data.first().unwrap();
        self.close_all_positions(&final_day.date, final_day.close);
        
        self.generate_result(symbol, start_date, end_date)
    }
    
    fn open_call_position(
        &mut self,
        symbol: &str,
        spot: f64,
        strike: f64,
        _days_to_expiry: usize,  // Use config instead
        volatility: f64,
        date: &str,
    ) {
        let time_to_expiry = self.config.days_to_expiry as f64 / 365.0;
        let greeks = black_scholes_merton_call(
            spot,
            strike,
            time_to_expiry,
            self.config.risk_free_rate,
            volatility,
            0.0,
        );
        
        let position_size = self.calculate_position_size(greeks.price);
        if position_size == 0 {
            return;
        }
        
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Call,
            strike,
            position_size,
            greeks.price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            greeks.price,
            position_size,
            spot,
            Some(greeks),
            self.config.commission_per_trade,
        );
        
        self.current_capital -= trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    fn open_put_position(
        &mut self,
        symbol: &str,
        spot: f64,
        strike: f64,
        _days_to_expiry: usize,  // Use config instead
        volatility: f64,
        date: &str,
    ) {
        let time_to_expiry = self.config.days_to_expiry as f64 / 365.0;
        let greeks = black_scholes_merton_put(
            spot,
            strike,
            time_to_expiry,
            self.config.risk_free_rate,
            volatility,
            0.0,
        );
        
        let position_size = self.calculate_position_size(greeks.price);
        if position_size == 0 {
            return;
        }
        
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Put,
            strike,
            position_size,
            greeks.price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            greeks.price,
            position_size,
            spot,
            Some(greeks),
            self.config.commission_per_trade,
        );
        
        self.current_capital -= trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    fn update_open_positions(&mut self, date: &str, spot: f64, hist_vols: &[f64], day_idx: usize) {
        if let Some(&volatility) = hist_vols.get(day_idx) {
            // Calculate days held for all positions first (avoid borrow conflict)
            let positions_data: Vec<_> = self.positions.iter()
                .filter(|p| matches!(p.status, PositionStatus::Open))
                .map(|p| (p.id, p.entry_date.clone(), p.strike, matches!(p.option_type, OptionType::Call)))
                .collect();
            
            for (pos_id, entry_date, strike, is_call) in positions_data {
                let days_held = self.calculate_days_between(&entry_date, date);
                let days_to_expiry = self.config.days_to_expiry.saturating_sub(days_held).max(1);
                let time_to_expiry = days_to_expiry as f64 / 365.0;
                
                // Price the option at current spot and vol
                let current_price = if is_call {
                    black_scholes_merton_call(
                        spot,
                        strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        0.0,
                    ).price
                } else {
                    black_scholes_merton_put(
                        spot,
                        strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        0.0,
                    ).price
                };
                
                // Update the position
                if let Some(position) = self.positions.iter_mut().find(|p| p.id == pos_id) {
                    position.update_unrealized_pnl(current_price);
                }
            }
        }
    }
    
    fn check_exit_conditions(&mut self, date: &str, spot: f64, hist_vols: &[f64], day_idx: usize) {
        let volatility = hist_vols.get(day_idx).copied().unwrap_or(0.25);  // Default vol if missing
        
        let mut positions_to_close = Vec::new();
        
        for position in self.positions.iter() {
            if !matches!(position.status, PositionStatus::Open) {
                continue;
            }
            
            let days_held = self.calculate_days_between(&position.entry_date, date);
            
            // Time-based exit: Close after max_days_hold or at expiry
            if days_held >= self.config.max_days_hold {
                positions_to_close.push((position.id, days_held, "Max Hold Period".to_string()));
                continue;
            }
            
            // Also close if approaching expiry (within 2 days)
            if days_held >= self.config.days_to_expiry.saturating_sub(2) {
                positions_to_close.push((position.id, days_held, "Near Expiry".to_string()));
                continue;
            }
            
            // Stop loss
            if let Some(stop_pct) = self.config.stop_loss_pct {
                let loss_pct = (position.unrealized_pnl / (position.entry_price * position.quantity.abs() as f64 * 100.0)) * 100.0;
                if loss_pct < -stop_pct {
                    positions_to_close.push((position.id, days_held, "Stop Loss".to_string()));
                    continue;
                }
            }
            
            // Take profit
            if let Some(profit_pct) = self.config.take_profit_pct {
                let profit_pct_realized = (position.unrealized_pnl / (position.entry_price * position.quantity.abs() as f64 * 100.0)) * 100.0;
                if profit_pct_realized > profit_pct {
                    positions_to_close.push((position.id, days_held, "Take Profit".to_string()));
                }
            }
        }
        
        // Execute closes
        for (position_id, days_held, _reason) in positions_to_close {
            self.close_position_by_id(position_id, date, spot, volatility, days_held);
        }
    }
    
    fn close_position_by_id(&mut self, position_id: usize, date: &str, spot: f64, volatility: f64, days_held: usize) {
        if let Some(position) = self.positions.iter_mut().find(|p| p.id == position_id) {
            if !matches!(position.status, PositionStatus::Open) {
                return;
            }
            
            let days_to_expiry = self.config.days_to_expiry.saturating_sub(days_held).max(1);
            let time_to_expiry = days_to_expiry as f64 / 365.0;
            
            let exit_price = match position.option_type {
                OptionType::Call => {
                    black_scholes_merton_call(
                        spot,
                        position.strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        0.0,
                    ).price
                },
                OptionType::Put => {
                    black_scholes_merton_put(
                        spot,
                        position.strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        0.0,
                    ).price
                },
            };
            
            position.close(exit_price, date.to_string(), spot, days_held);
            
            let trade = Trade::new(
                position_id,
                TradeType::Exit,
                date.to_string(),
                position.symbol.clone(),
                exit_price,
                position.quantity,
                spot,
                None,
                self.config.commission_per_trade,
            );
            
            self.current_capital += trade.value() - self.config.commission_per_trade;
            self.trades.push(trade);
        }
    }
    
    fn close_all_positions(&mut self, date: &str, spot: f64) {
        let positions_to_close: Vec<_> = self.positions.iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.status, PositionStatus::Open))
            .map(|(idx, p)| (idx, p.entry_date.clone()))
            .collect();
        
        for (idx, entry_date) in positions_to_close {
            let days_held = self.calculate_days_between(&entry_date, date);
            self.positions[idx].expire(date.to_string(), spot, days_held);
            self.current_capital += 0.0;  // Expired worthless
        }
    }
    
    fn can_open_position(&self) -> bool {
        let open_positions = self.positions.iter()
            .filter(|p| matches!(p.status, PositionStatus::Open))
            .count();
        
        open_positions < self.config.max_positions && self.current_capital > 0.0
    }
    
    fn calculate_position_size(&self, option_price: f64) -> i32 {
        let available_capital = self.current_capital * (self.config.position_size_pct / 100.0);
        let contracts = (available_capital / (option_price * 100.0)).floor() as i32;
        contracts.max(0).min(10)  // Limit to 10 contracts max per position
    }
    
    fn unrealized_pnl(&self) -> f64 {
        self.positions.iter()
            .filter(|p| matches!(p.status, PositionStatus::Open))
            .map(|p| p.unrealized_pnl)
            .sum()
    }
    
    fn calculate_rolling_volatility(&self, data: &[HistoricalDay], window: usize) -> Vec<f64> {
        let mut vols = Vec::new();
        
        for i in 0..data.len() {
            let start = if i + window < data.len() { i } else { data.len().saturating_sub(window) };
            let end = i + 1;
            
            if end - start < 2 {
                vols.push(0.25);  // Default
                continue;
            }
            
            let window_data = &data[start..end];
            let mut returns = Vec::new();
            
            for j in 1..window_data.len() {
                let ret = (window_data[j].close / window_data[j-1].close).ln();
                returns.push(ret);
            }
            
            if returns.is_empty() {
                vols.push(0.25);
                continue;
            }
            
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>() / returns.len() as f64;
            
            let annual_vol = variance.sqrt() * (252.0_f64).sqrt();
            vols.push(annual_vol.max(0.10).min(2.0));  // Clamp between 10% and 200%
        }
        
        vols
    }
    
    fn calculate_days_between(&self, start: &str, end: &str) -> usize {
        use chrono::NaiveDate;
        
        // Parse dates - Yahoo Finance format: "2025-01-03 00:00:00-05:00"
        let parse_date = |s: &str| -> Option<NaiveDate> {
            // Extract just the date part (YYYY-MM-DD)
            let date_str = s.split_whitespace().next()?;
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
        };
        
        match (parse_date(start), parse_date(end)) {
            (Some(start_dt), Some(end_dt)) => {
                (end_dt - start_dt).num_days().abs() as usize
            },
            _ => 1  // Fallback
        }
    }
    
    fn generate_result(&self, symbol: &str, start_date: String, end_date: String) -> BacktestResult {
        let metrics = PerformanceMetrics::calculate(
            &self.positions,
            &self.trades,
            self.config.initial_capital,
            &self.equity_curve,
        );
        
        BacktestResult {
            symbol: symbol.to_string(),
            start_date,
            end_date,
            initial_capital: self.config.initial_capital,
            final_capital: self.current_capital,
            positions: self.positions.clone(),
            trades: self.trades.clone(),
            equity_curve: self.equity_curve.clone(),
            metrics,
        }
    }
}

/// Signal actions for custom strategies
#[derive(Debug, Clone)]
pub enum SignalAction {
    BuyCall { strike: f64, days_to_expiry: usize, volatility: f64 },
    BuyPut { strike: f64, days_to_expiry: usize, volatility: f64 },
    ClosePosition { position_id: usize },
}
