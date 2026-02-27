// Backtesting engine - orchestrates historical simulation

use crate::backtesting::position::{Position, PositionStatus, OptionType};
use crate::backtesting::trade::{Trade, TradeType};
use crate::backtesting::metrics::{BacktestResult, PerformanceMetrics, EquityCurve};
use crate::models::american::{american_call_binomial, american_put_binomial, BinomialConfig, ExerciseStyle};
use crate::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use crate::market_data::csv_loader::HistoricalDay;
use crate::portfolio::{PortfolioManager, PortfolioConfig, SizingMethod, AllocationMethod, RiskLimits};
use crate::strategies::SignalAction;

#[derive(Debug, Clone)]
pub struct TradingCosts {
    pub commission_per_contract: f64,  // $0.50–$2.50 per contract (realistic)
    pub bid_ask_spread_percent: f64,   // 0.5–5% of mid price (market impact)
}

impl Default for TradingCosts {
    fn default() -> Self {
        Self {
            commission_per_contract: 1.0,  // $1 per contract
            bid_ask_spread_percent: 1.0,   // 1% spread
        }
    }
}

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub initial_capital: f64,
    pub trading_costs: TradingCosts,  // Replace flat commission with realistic costs
    pub risk_free_rate: f64,
    pub max_positions: usize,
    pub position_size_pct: f64,  // Percentage of capital per position (used if no portfolio manager)
    pub days_to_expiry: usize,  // Option expiration in days
    pub max_days_hold: usize,  // Maximum days to hold before forced close
    pub stop_loss_pct: Option<f64>,  // Optional stop loss
    pub take_profit_pct: Option<f64>,  // Optional take profit
    pub use_portfolio_management: bool,  // Enable portfolio-based position sizing and risk limits
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            trading_costs: TradingCosts::default(),
            risk_free_rate: 0.05,
            max_positions: 10,
            position_size_pct: 10.0,
            days_to_expiry: 30,  // 30-day options
            max_days_hold: 21,  // Close after 21 days (70% of expiry)
            stop_loss_pct: Some(50.0),  // 50% stop loss
            take_profit_pct: Some(100.0),  // 100% take profit
            use_portfolio_management: false,  // Disabled by default for backward compatibility
        }
    }
}

pub struct BacktestEngine {
    config: BacktestConfig,
    portfolio_manager: Option<PortfolioManager>,
    positions: Vec<Position>,
    trades: Vec<Trade>,
    equity_curve: EquityCurve,
    current_capital: f64,
    position_counter: usize,
}

impl BacktestEngine {
    /// Calculate effective price including bid-ask spread
    /// For buying: price * (1 + spread/2) 
    /// For selling: price * (1 - spread/2)
    fn effective_price(&self, mid_price: f64, is_buying: bool) -> f64 {
        let spread_factor = self.config.trading_costs.bid_ask_spread_percent / 100.0;
        if is_buying {
            mid_price * (1.0 + spread_factor / 2.0)
        } else {
            mid_price * (1.0 - spread_factor / 2.0)
        }
    }
    
    /// Calculate total commission for a trade
    fn calculate_commission(&self, quantity: i32) -> f64 {
        self.config.trading_costs.commission_per_contract * quantity.abs() as f64
    }
    pub fn new(config: BacktestConfig) -> Self {
        let portfolio_manager = if config.use_portfolio_management {
            Some(PortfolioManager::new(PortfolioConfig {
                initial_capital: config.initial_capital,
                max_risk_per_trade: 2.0,
                max_position_pct: 10.0,
                sizing_method: SizingMethod::VolatilityBased,
                allocation_method: AllocationMethod::RiskParity,
                risk_limits: RiskLimits::default(),
            }))
        } else {
            None
        };
        
        Self {
            current_capital: config.initial_capital,
            config,
            portfolio_manager,
            positions: Vec::new(),
            trades: Vec::new(),
            equity_curve: EquityCurve::new(),
            position_counter: 0,
        }
    }
    
    /// Create a new backtest engine with custom portfolio configuration
    pub fn new_with_portfolio(config: BacktestConfig, portfolio_config: PortfolioConfig) -> Self {
        Self {
            current_capital: config.initial_capital,
            config,
            portfolio_manager: Some(PortfolioManager::new(portfolio_config)),
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
                        self.config.days_to_expiry,
                        hist_vol,
                        &day.date,
                        ExerciseStyle::American,  // Use American options by default
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
                            self.open_call_position(symbol, spot, strike, days_to_expiry, volatility, &day.date, ExerciseStyle::European);
                        },
                        SignalAction::BuyPut { strike, days_to_expiry, volatility } => {
                            self.open_put_position(symbol, spot, strike, days_to_expiry, volatility, &day.date, ExerciseStyle::European);
                        },
                        SignalAction::SellCall { strike, days_to_expiry, volatility } => {
                            self.open_short_call_position(symbol, spot, strike, days_to_expiry, volatility, &day.date);
                        },
                        SignalAction::SellPut { strike, days_to_expiry, volatility } => {
                            self.open_short_put_position(symbol, spot, strike, days_to_expiry, volatility, &day.date);
                        },
                        SignalAction::ClosePosition { position_id } => {
                            if let Some(hist_vol) = hist_vols.get(day_idx) {
                                self.close_position_by_id(position_id, &day.date, spot, *hist_vol, 0);
                            }
                        },
                        // Phase 6: Multi-leg spread strategies
                        SignalAction::IronCondor { sell_call_strike, buy_call_strike, sell_put_strike, buy_put_strike, days_to_expiry } => {
                            self.open_iron_condor(symbol, spot, sell_call_strike, buy_call_strike, sell_put_strike, buy_put_strike, days_to_expiry, &day.date);
                        },
                        SignalAction::CreditCallSpread { sell_strike, buy_strike, days_to_expiry } => {
                            self.open_credit_call_spread(symbol, spot, sell_strike, buy_strike, days_to_expiry, &day.date);
                        },
                        SignalAction::CreditPutSpread { sell_strike, buy_strike, days_to_expiry } => {
                            self.open_credit_put_spread(symbol, spot, sell_strike, buy_strike, days_to_expiry, &day.date);
                        },
                        SignalAction::CoveredCall { sell_strike, days_to_expiry } => {
                            self.open_covered_call(symbol, spot, sell_strike, days_to_expiry, &day.date);
                        },
                        // Legacy signals (keep for backward compatibility)
                        SignalAction::SellStraddle => {
                            // Implement sell straddle logic
                        },
                        SignalAction::BuyStraddle => {
                            // Implement buy straddle logic
                        },
                        SignalAction::IronButterfly { wing_width } => {
                            // Implement iron butterfly logic
                        },
                        SignalAction::CashSecuredPut { strike_pct } => {
                            // Implement cash-secured put logic
                        },
                        SignalAction::NoAction => {
                            // Do nothing
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
        exercise_style: ExerciseStyle,
    ) {
        let time_to_expiry = self.config.days_to_expiry as f64 / 365.0;
        
        // Choose pricing model based on exercise style
        let (price, greeks) = match exercise_style {
            ExerciseStyle::European => {
                let greeks = black_scholes_merton_call(
                    spot,
                    strike,
                    time_to_expiry,
                    self.config.risk_free_rate,
                    volatility,
                    0.0,
                );
                (greeks.price, greeks)
            },
            ExerciseStyle::American => {
                let config = BinomialConfig::default();
                let price = american_call_binomial(spot, strike, time_to_expiry, self.config.risk_free_rate, volatility, &config);
                let greeks = crate::models::american::american_call_greeks(spot, strike, time_to_expiry, self.config.risk_free_rate, volatility, &config);
                (price, greeks)
            }
        };
        
        // Calculate position size first
        let position_size = self.calculate_position_size_with_vol(price, volatility);
        if position_size == 0 {
            return;
        }
        
        // Check portfolio risk limits if enabled
        if !self.can_open_position_with_portfolio_check(symbol, price, volatility, position_size) {
            return;
        }
        
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Call,
            exercise_style,
            strike,
            position_size,
            price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        // Calculate effective price with bid-ask spread (buying)
        let effective_price = self.effective_price(price, true);
        let commission = self.calculate_commission(position_size);
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,  // Use effective price instead of mid price
            position_size,
            spot,
            Some(greeks),
            commission,
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
        exercise_style: ExerciseStyle,
    ) {
        let time_to_expiry = self.config.days_to_expiry as f64 / 365.0;
        
        // Choose pricing model based on exercise style
        let (price, greeks) = match exercise_style {
            ExerciseStyle::European => {
                let greeks = black_scholes_merton_put(
                    spot,
                    strike,
                    time_to_expiry,
                    self.config.risk_free_rate,
                    volatility,
                    0.0,
                );
                (greeks.price, greeks)
            },
            ExerciseStyle::American => {
                let config = BinomialConfig::default();
                let price = american_put_binomial(spot, strike, time_to_expiry, self.config.risk_free_rate, volatility, &config);
                let greeks = crate::models::american::american_put_greeks(spot, strike, time_to_expiry, self.config.risk_free_rate, volatility, &config);
                (price, greeks)
            }
        };
        
        // Calculate position size first
        let position_size = self.calculate_position_size_with_vol(price, volatility);
        if position_size == 0 {
            return;
        }
        
        // Check portfolio risk limits if enabled
        if !self.can_open_position_with_portfolio_check(symbol, price, volatility, position_size) {
            return;
        }
        
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Put,
            exercise_style,
            strike,
            position_size,
            price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        // Calculate effective price with bid-ask spread (buying)
        let effective_price = self.effective_price(price, true);
        let commission = self.calculate_commission(position_size);
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,  // Use effective price instead of mid price
            position_size,
            spot,
            Some(greeks),
            commission,
        );
        
        self.current_capital -= trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    fn open_short_call_position(
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
        
        // Calculate position size first
        let position_size = self.calculate_position_size_with_vol(greeks.price, volatility);
        if position_size == 0 {
            return;
        }
        
        // Check portfolio risk limits if enabled
        if !self.can_open_position_with_portfolio_check(symbol, greeks.price, volatility, position_size) {
            return;
        }
        
        // For short positions, quantity is negative
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Call,
            ExerciseStyle::European,
            strike,
            -(position_size as i32),  // Negative for short
            greeks.price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        // Calculate effective price with bid-ask spread (selling/short)
        let effective_price = self.effective_price(greeks.price, false);
        let commission = self.calculate_commission(position_size as i32);
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,  // Use effective price instead of mid price
            -(position_size as i32),  // Negative for short
            spot,
            Some(greeks),
            commission,
        );
        
        // For short options, we receive premium (credit) minus commission
        self.current_capital += trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    fn open_short_put_position(
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
        
        // Calculate position size first
        let position_size = self.calculate_position_size_with_vol(greeks.price, volatility);
        if position_size == 0 {
            return;
        }
        
        // Check portfolio risk limits if enabled
        if !self.can_open_position_with_portfolio_check(symbol, greeks.price, volatility, position_size) {
            return;
        }
        
        // For short positions, quantity is negative
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Put,
            ExerciseStyle::European,
            strike,
            -(position_size as i32),  // Negative for short
            greeks.price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        // Calculate effective price with bid-ask spread (selling/short)
        let effective_price = self.effective_price(greeks.price, false);
        let commission = self.calculate_commission(position_size as i32);
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,  // Use effective price instead of mid price
            -(position_size as i32),  // Negative for short
            spot,
            Some(greeks),
            commission,
        );
        
        // For short options, we receive premium (credit) minus commission
        self.current_capital += trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    // Phase 6: Multi-leg spread position methods
    
    /// Open an iron condor (4-leg spread)
    /// Sells OTM call and OTM put, buys further OTM call and put for protection
    fn open_iron_condor(
        &mut self,
        symbol: &str,
        spot: f64,
        sell_call_strike: f64,
        buy_call_strike: f64,
        sell_put_strike: f64,
        buy_put_strike: f64,
        days_to_expiry: usize,
        date: &str,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        let volatility = 0.30; // Simplified - in practice would use market vol
        
        // Calculate prices for all legs
        let sell_call_price = black_scholes_merton_call(spot, sell_call_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let buy_call_price = black_scholes_merton_call(spot, buy_call_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let sell_put_price = black_scholes_merton_put(spot, sell_put_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let buy_put_price = black_scholes_merton_put(spot, buy_put_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        
        // Net premium received (positive for credit)
        let net_premium = sell_call_price + sell_put_price - buy_call_price - buy_put_price;
        
        // Calculate position size based on max loss (spread width - premium)
        let spread_width = sell_call_strike - sell_put_strike;
        let max_loss = spread_width - net_premium;
        let position_size = self.calculate_position_size_for_spread(max_loss.abs());
        
        if position_size == 0 {
            return;
        }
        
        // Open all four legs
        self.open_spread_leg(symbol, spot, sell_call_strike, -(position_size as i32), OptionType::Call, sell_call_price, date, time_to_expiry, volatility);
        self.open_spread_leg(symbol, spot, buy_call_strike, position_size as i32, OptionType::Call, buy_call_price, date, time_to_expiry, volatility);
        self.open_spread_leg(symbol, spot, sell_put_strike, -(position_size as i32), OptionType::Put, sell_put_price, date, time_to_expiry, volatility);
        self.open_spread_leg(symbol, spot, buy_put_strike, position_size as i32, OptionType::Put, buy_put_price, date, time_to_expiry, volatility);
    }
    
    /// Open a credit call spread (bullish)
    /// Sells ITM/ATM call, buys OTM call
    fn open_credit_call_spread(
        &mut self,
        symbol: &str,
        spot: f64,
        sell_strike: f64,
        buy_strike: f64,
        days_to_expiry: usize,
        date: &str,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        let volatility = 0.30;
        
        let sell_price = black_scholes_merton_call(spot, sell_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let buy_price = black_scholes_merton_call(spot, buy_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let net_premium = sell_price - buy_price;
        
        // Max loss is spread width minus premium
        let max_loss = (buy_strike - sell_strike) - net_premium;
        let position_size = self.calculate_position_size_for_spread(max_loss.abs());
        
        if position_size == 0 {
            return;
        }
        
        self.open_spread_leg(symbol, spot, sell_strike, -(position_size as i32), OptionType::Call, sell_price, date, time_to_expiry, volatility);
        self.open_spread_leg(symbol, spot, buy_strike, position_size as i32, OptionType::Call, buy_price, date, time_to_expiry, volatility);
    }
    
    /// Open a credit put spread (bearish)
    /// Sells ITM/ATM put, buys OTM put
    fn open_credit_put_spread(
        &mut self,
        symbol: &str,
        spot: f64,
        sell_strike: f64,
        buy_strike: f64,
        days_to_expiry: usize,
        date: &str,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        let volatility = 0.30;
        
        let sell_price = black_scholes_merton_put(spot, sell_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let buy_price = black_scholes_merton_put(spot, buy_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let net_premium = sell_price - buy_price;
        
        // Max loss is spread width minus premium
        let max_loss = (sell_strike - buy_strike) - net_premium;
        let position_size = self.calculate_position_size_for_spread(max_loss.abs());
        
        if position_size == 0 {
            return;
        }
        
        self.open_spread_leg(symbol, spot, sell_strike, -(position_size as i32), OptionType::Put, sell_price, date, time_to_expiry, volatility);
        self.open_spread_leg(symbol, spot, buy_strike, position_size as i32, OptionType::Put, buy_price, date, time_to_expiry, volatility);
    }
    
    /// Open a covered call
    /// Buy stock, sell call against it
    fn open_covered_call(
        &mut self,
        symbol: &str,
        spot: f64,
        sell_strike: f64,
        days_to_expiry: usize,
        date: &str,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        let volatility = 0.30;
        
        // Buy 100 shares of stock
        let shares_to_buy = 100;
        let stock_cost = spot * shares_to_buy as f64;
        
        // Sell call
        let call_price = black_scholes_merton_call(spot, sell_strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0).price;
        let call_premium = call_price * shares_to_buy as f64;
        
        // Net debit = stock cost - call premium
        let net_debit = stock_cost - call_premium;
        
        // Check if we have enough capital
        if net_debit > self.current_capital {
            return;
        }
        
        // Open stock position (long)
        self.open_stock_position(symbol, shares_to_buy, spot, date);
        
        // Open short call position
        self.open_spread_leg(symbol, spot, sell_strike, -(shares_to_buy as i32), OptionType::Call, call_price, date, time_to_expiry, volatility);
    }
    
    /// Helper method to open individual spread legs
    fn open_spread_leg(
        &mut self,
        symbol: &str,
        spot: f64,
        strike: f64,
        quantity: i32,
        option_type: OptionType,
        price: f64,
        date: &str,
        time_to_expiry: f64,
        volatility: f64,
    ) {
        let greeks = match option_type {
            OptionType::Call => black_scholes_merton_call(spot, strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0),
            OptionType::Put => black_scholes_merton_put(spot, strike, time_to_expiry, self.config.risk_free_rate, volatility, 0.0),
        };
        
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            option_type,
            ExerciseStyle::European,
            strike,
            quantity,
            price,
            date.to_string(),
            spot,
            Some(greeks),
        );
        
        let effective_price = self.effective_price(price, quantity > 0);
        let commission = self.calculate_commission(quantity.abs());
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,
            quantity,
            spot,
            Some(greeks),
            commission,
        );
        
        // Update capital (positive for credits, negative for debits)
        self.current_capital -= trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    /// Open a stock position (for covered calls)
    fn open_stock_position(&mut self, symbol: &str, shares: i32, price: f64, date: &str) {
        // Create a synthetic stock position (using a call with strike=0 as proxy)
        let position = Position::new(
            self.position_counter,
            symbol.to_string(),
            OptionType::Call, // Using Call as proxy for stock
            ExerciseStyle::European,
            0.0, // Strike = 0 for stock
            shares,
            price,
            date.to_string(),
            price,
            None, // No Greeks for stock
        );
        
        let commission = self.calculate_commission(shares.abs());
        
        let trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            price,
            shares,
            price,
            None,
            commission,
        );
        
        self.current_capital -= trade.total_cost();
        self.positions.push(position);
        self.trades.push(trade);
        self.position_counter += 1;
    }
    
    /// Calculate position size for spreads based on max loss
    fn calculate_position_size_for_spread(&self, max_loss_per_share: f64) -> usize {
        let max_loss_per_contract = max_loss_per_share * 100.0; // Options are for 100 shares
        let max_risk_per_position = self.current_capital * 0.02; // 2% of capital max risk
        
        if max_loss_per_contract <= 0.0 {
            return 1; // Minimum 1 contract
        }
        
        let max_contracts = (max_risk_per_position / max_loss_per_contract) as usize;
        max_contracts.max(1).min(10) // Between 1 and 10 contracts
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
            
            let exit_price = match (&position.option_type, &position.exercise_style) {
                (OptionType::Call, ExerciseStyle::European) => {
                    black_scholes_merton_call(
                        spot,
                        position.strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        0.0,
                    ).price
                },
                (OptionType::Call, ExerciseStyle::American) => {
                    let config = BinomialConfig::default();
                    american_call_binomial(
                        spot,
                        position.strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        &config,
                    )
                },
                (OptionType::Put, ExerciseStyle::European) => {
                    black_scholes_merton_put(
                        spot,
                        position.strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        0.0,
                    ).price
                },
                (OptionType::Put, ExerciseStyle::American) => {
                    let config = BinomialConfig::default();
                    american_put_binomial(
                        spot,
                        position.strike,
                        time_to_expiry,
                        self.config.risk_free_rate,
                        volatility,
                        &config,
                    )
                },
            };
            
            position.close(exit_price, date.to_string(), spot, days_held);
            
            // Extract data before calling methods to avoid borrowing issues
            let quantity = position.quantity;
            let symbol = position.symbol.clone();
            let is_selling = quantity > 0;
            
            // Now call methods (no mutable borrow conflict)
            let effective_exit_price = self.effective_price(exit_price, !is_selling);
            let commission = self.calculate_commission(quantity);
            
            let trade = Trade::new(
                position_id,
                TradeType::Exit,
                date.to_string(),
                symbol,
                effective_exit_price,
                quantity,
                spot,
                None,
                commission,
            );
            
            // Calculate P&L: for long positions we sell (credit), for short positions we buy (debit)
            if quantity > 0 {
                // Long position: receive exit price minus commission
                self.current_capital += trade.total_cost();
            } else {
                // Short position: pay exit price plus commission
                self.current_capital -= trade.total_cost();
            }
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
        
        if open_positions >= self.config.max_positions {
            return false;
        }
        
        if self.current_capital <= 0.0 {
            return false;
        }
        
        // No additional portfolio risk checks if portfolio management disabled
        true
    }
    
    fn can_open_position_with_portfolio_check(
        &self,
        symbol: &str,
        option_price: f64,
        volatility: f64,
        contracts: i32,
    ) -> bool {
        if !self.can_open_position() {
            return false;
        }
        
        // If portfolio management enabled, check risk limits
        if let Some(ref manager) = self.portfolio_manager {
            let decision = manager.can_take_position(symbol, option_price, volatility, contracts);
            decision.can_trade
        } else {
            true
        }
    }
    
    fn calculate_position_size(&self, option_price: f64) -> i32 {
        self.calculate_position_size_with_vol(option_price, 0.30)  // Default 30% vol
    }
    
    fn calculate_position_size_with_vol(&self, option_price: f64, volatility: f64) -> i32 {
        if let Some(ref manager) = self.portfolio_manager {
            // Use portfolio manager for intelligent sizing
            manager.calculate_position_size(
                option_price,
                volatility,
                None,  // win_rate - could be calculated from historical trades
                None,  // avg_win
                None,  // avg_loss
            )
        } else {
            // Fallback to simple percentage-based sizing
            let available_capital = self.current_capital * (self.config.position_size_pct / 100.0);
            let contracts = (available_capital / (option_price * 100.0)).floor() as i32;
            contracts.max(0).min(10)  // Limit to 10 contracts max per position
        }
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


