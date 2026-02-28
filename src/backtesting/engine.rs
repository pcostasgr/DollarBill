#![allow(dead_code)]
// Backtesting engine - orchestrates historical simulation

use crate::backtesting::position::{Position, PositionStatus, OptionType};
use crate::backtesting::trade::{Trade, TradeType};
use crate::backtesting::metrics::{BacktestResult, PerformanceMetrics, EquityCurve};
use crate::models::american::{american_call_binomial, american_put_binomial, BinomialConfig, ExerciseStyle};
use crate::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use crate::market_data::csv_loader::HistoricalDay;
use crate::portfolio::{PortfolioManager, PortfolioConfig, SizingMethod, AllocationMethod, RiskLimits};
use crate::strategies::SignalAction;

/// How the bid-ask spread (and market impact) scales beyond a flat percentage.
///
/// All models start from `bid_ask_spread_percent / 2` as the one-way half-spread.
#[derive(Debug, Clone)]
pub enum SlippageModel {
    /// Constant half-spread — the same percentage regardless of volatility or size.
    /// This is the original behaviour and the default.
    Fixed,

    /// Half-spread grows linearly with realised volatility relative to a 25% baseline.
    /// spread = base * (1 + multiplier * (vol / 0.25 − 1)).
    /// `multiplier = 1.0` means at 50% vol you pay 3× the base spread.
    VolatilityScaled { multiplier: f64 },

    /// Square-root market-impact model commonly used in option markets.
    /// impact_bps is added on top of the flat spread for every √lot traded.
    /// spread = base + (impact_bps / 10_000) × √contracts
    SizeImpact { impact_bps: f64 },

    /// Panic-widening model: below `normal_vol` the spread is the base rate;
    /// above it the spread widens as (vol / normal_vol)^panic_exponent, simulating
    /// the liquidity drought during market panics (e.g. VIX 80 in March 2020).
    ///
    /// Example: normal_vol=0.20, panic_exponent=2.0 → at vol=0.80 (4×)
    /// the spread is 16× wider, matching typical bid-ask blow-out in a crash.
    PanicWidening {
        /// Annualised vol below which the spread stays at the base rate.
        normal_vol: f64,
        /// Exponent applied to (vol / normal_vol) when vol > normal_vol.
        /// 1.5 = moderate widening; 2.0 = severe panic widening.
        panic_exponent: f64,
    },

    /// Full market-impact model combining three real-world liquidity effects:
    ///
    /// 1. **Cap-class multiplier** — small/illiquid stocks have intrinsically
    ///    wider spreads.  `cap_multiplier = 1.0` for large-caps (SPY, AAPL),
    ///    `3.0` for micro-caps or thinly traded names.
    ///
    /// 2. **√-contract size impact** — large orders move the market proportional
    ///    to the square-root of the number of contracts traded.
    ///    `size_impact_bps` bps are added per √lot.
    ///
    /// 3. **Panic widening** — when realised vol exceeds `normal_vol` the
    ///    combined base+size spread is multiplied by (vol/normal_vol)^panic_exponent.
    ///
    /// Formula:
    ///   base  = (bid_ask_spread_percent / 200) × cap_multiplier
    ///   size  = (size_impact_bps / 10_000) × √contracts
    ///   panic = if vol > normal_vol { (vol/normal_vol)^panic_exponent } else { 1 }
    ///   half_spread = (base + size) × panic
    FullMarketImpact {
        /// Base spread multiplier for illiquidity class (1.0 = large-cap, 3.0 = small-cap).
        cap_multiplier: f64,
        /// Market impact in basis points per √contract traded.
        size_impact_bps: f64,
        /// Annualised vol threshold below which no panic widening occurs.
        normal_vol: f64,
        /// Exponent applied to (vol / normal_vol) when vol > normal_vol.
        panic_exponent: f64,
    },
}

/// Models what fraction of a requested order actually executes.
///
/// In liquid markets orders fill completely; during a panic, market depth
/// dries up and the effective fill rate drops — a large vol-seller in
/// March 2020 might only fill 60% of the desired contracts before the market
/// moved away.
#[derive(Debug, Clone)]
pub enum PartialFillModel {
    /// All orders execute completely (default; original behaviour).
    AlwaysFull,

    /// Fill rate scales down as volatility rises above `normal_vol`.
    ///
    /// fill_rate = clamp(normal_vol / vol, min_fill_rate, 1.0)
    ///
    /// Example: normal_vol=0.25, min_fill_rate=0.30, vol=0.80
    ///   → fill_rate = clamp(0.25/0.80, 0.30, 1.0) = 0.313 (31% fill).
    VolScaled {
        /// At or below this vol the fill rate is 1.0.
        normal_vol: f64,
        /// Floor for the fill fraction even in the worst panic (e.g. 0.25 = 25%).
        min_fill_rate: f64,
    },
}

impl PartialFillModel {
    /// Compute the fill fraction [0, 1] for a given realised volatility.
    pub fn fill_rate(&self, volatility: f64) -> f64 {
        match self {
            Self::AlwaysFull => 1.0,
            Self::VolScaled { normal_vol, min_fill_rate } => {
                if volatility <= *normal_vol {
                    1.0
                } else {
                    (normal_vol / volatility).clamp(*min_fill_rate, 1.0)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradingCosts {
    /// Flat commission charged per contract on every leg, entry and exit.
    /// Typical range: $0.50 (low-cost broker) to $2.50 (full service).
    pub commission_per_contract: f64,
    /// Base bid-ask spread as a % of the option mid-price (round-trip = 2× this half).
    /// Typical range: 0.5% (liquid large-cap) to 5% (illiquid small-cap).
    pub bid_ask_spread_percent: f64,
    /// Controls how slippage scales beyond the flat spread.
    pub slippage_model: SlippageModel,
    /// Controls what fraction of the desired order size actually fills.
    /// `AlwaysFull` (default) gives the original 100%-fill behaviour.
    pub partial_fill_model: PartialFillModel,
}

impl TradingCosts {
    /// One-way half-spread fraction to apply to the mid price.
    ///
    /// Buying: fill = mid × (1 + half_spread)
    /// Selling: fill = mid × (1 − half_spread)
    pub fn half_spread(&self, volatility: f64, contracts: i32) -> f64 {
        let base = self.bid_ask_spread_percent / 200.0;  // half of the full spread %
        match &self.slippage_model {
            SlippageModel::Fixed => base,
            SlippageModel::VolatilityScaled { multiplier } => {
                let vol_ratio = (volatility / 0.25).clamp(0.5, 5.0);
                base * (1.0 + multiplier * (vol_ratio - 1.0).max(0.0))
            }
            SlippageModel::SizeImpact { impact_bps } => {
                let impact = (impact_bps / 10_000.0) * (contracts.max(1) as f64).sqrt();
                base + impact
            }
            SlippageModel::PanicWidening { normal_vol, panic_exponent } => {
                if volatility <= *normal_vol {
                    base
                } else {
                    let vol_ratio = (volatility / normal_vol).min(10.0);
                    base * vol_ratio.powf(*panic_exponent)
                }
            }
            SlippageModel::FullMarketImpact {
                cap_multiplier,
                size_impact_bps,
                normal_vol,
                panic_exponent,
            } => {
                let illiq_base = (self.bid_ask_spread_percent / 200.0) * cap_multiplier;
                let size_impact = (size_impact_bps / 10_000.0)
                    * (contracts.max(1) as f64).sqrt();
                let panic_mult = if volatility > *normal_vol {
                    (volatility / normal_vol).min(10.0).powf(*panic_exponent)
                } else {
                    1.0
                };
                (illiq_base + size_impact) * panic_mult
            }
        }
    }

    /// Effective fill price given the mid-market price and trade direction.
    pub fn fill_price(&self, mid: f64, is_buying: bool, volatility: f64, contracts: i32) -> f64 {
        let h = self.half_spread(volatility, contracts);
        if is_buying { mid * (1.0 + h) } else { mid * (1.0 - h) }
    }

    /// One-way slippage cost in dollars: how much you lose to the spread on a single leg.
    pub fn one_way_slippage(&self, mid: f64, contracts: i32, volatility: f64) -> f64 {
        let h = self.half_spread(volatility, contracts);
        mid * contracts.abs() as f64 * 100.0 * h
    }

    /// Total commission for a trade with the given number of contracts.
    pub fn commission_for(&self, contracts: i32) -> f64 {
        self.commission_per_contract * contracts.abs() as f64
    }

    /// Effective number of contracts to fill after applying the partial-fill model.
    /// Returns a value in [0, requested]; may be 0 in extreme panic with VolScaled.
    pub fn apply_partial_fill(&self, requested: i32, volatility: f64) -> i32 {
        let rate = self.partial_fill_model.fill_rate(volatility);
        ((requested.abs() as f64 * rate).floor() as i32).max(0)
    }
}

impl Default for TradingCosts {
    fn default() -> Self {
        Self {
            commission_per_contract: 1.0,   // $1 per contract
            bid_ask_spread_percent: 1.0,    // 1% spread (realistic for SPY options)
            slippage_model: SlippageModel::Fixed,
            partial_fill_model: PartialFillModel::AlwaysFull,
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
    /// Continuous annual dividend yield for the underlying asset.  Passed to the
    /// CRR binomial tree when pricing American options so early-exercise is
    /// correctly penalised for high-dividend stocks.  Default 0.0 (no dividends).
    pub dividend_yield: f64,
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
            dividend_yield: 0.0,
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
    /// Effective fill price — prefers actual vol and contract count.
    fn effective_price_ctx(&self, mid: f64, is_buying: bool, vol: f64, contracts: i32) -> f64 {
        self.config.trading_costs.fill_price(mid, is_buying, vol, contracts)
    }

    /// Total commission for `contracts` lots (already the sum, not per-contract).
    fn calculate_commission(&self, contracts: i32) -> f64 {
        self.config.trading_costs.commission_for(contracts)
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
            let current_vol = hist_vols.get(day_idx).copied().unwrap_or(0.25);
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
                            self.open_iron_condor(symbol, spot, sell_call_strike, buy_call_strike, sell_put_strike, buy_put_strike, days_to_expiry, &day.date, current_vol);
                        },
                        SignalAction::CreditCallSpread { sell_strike, buy_strike, days_to_expiry } => {
                            self.open_credit_call_spread(symbol, spot, sell_strike, buy_strike, days_to_expiry, &day.date, current_vol);
                        },
                        SignalAction::CreditPutSpread { sell_strike, buy_strike, days_to_expiry } => {
                            self.open_credit_put_spread(symbol, spot, sell_strike, buy_strike, days_to_expiry, &day.date, current_vol);
                        },
                        SignalAction::CoveredCall { sell_strike, days_to_expiry } => {
                            self.open_covered_call(symbol, spot, sell_strike, days_to_expiry, &day.date, current_vol);
                        },
                        // Multi-leg strategies via legacy signal variants
                        SignalAction::SellStraddle => {
                            // Sell ATM straddle: short call + short put at spot
                            self.open_short_call_position(symbol, spot, spot, 30, current_vol, &day.date);
                            self.open_short_put_position(symbol, spot, spot, 30, current_vol, &day.date);
                        },
                        SignalAction::BuyStraddle => {
                            // Buy ATM straddle: long call + long put at spot
                            self.open_call_position(symbol, spot, spot, 30, current_vol, &day.date, ExerciseStyle::European);
                            self.open_put_position(symbol, spot, spot, 30, current_vol, &day.date, ExerciseStyle::European);
                        },
                        SignalAction::IronButterfly { wing_width } => {
                            // Iron butterfly: sell ATM call + sell ATM put,
                            // buy OTM call @ spot+wing, buy OTM put @ spot-wing
                            let days = 30;
                            self.open_iron_condor(
                                symbol, spot,
                                spot,              // sell call strike (ATM)
                                spot + wing_width, // buy call strike  (OTM)
                                spot,              // sell put strike  (ATM)
                                spot - wing_width, // buy put strike   (OTM)
                                days, &day.date, current_vol,
                            );
                        },
                        SignalAction::CashSecuredPut { strike_pct } => {
                            // Cash-secured put: sell OTM put at spot*(1 - strike_pct)
                            let put_strike = spot * (1.0 - strike_pct);
                            self.open_short_put_position(symbol, spot, put_strike, 30, current_vol, &day.date);
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
                let config = BinomialConfig {
                    use_dividends: self.config.dividend_yield > 0.0,
                    dividend_yield: self.config.dividend_yield,
                    ..BinomialConfig::default()
                };
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
        // Partial-fill model: in panic regimes only a fraction of the order executes.
        let position_size = self.config.trading_costs.apply_partial_fill(position_size, volatility);
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
        
        // Effective fill price when buying (we pay more than mid due to spread)
        let effective_price = self.effective_price_ctx(price, true, volatility, position_size);
        let commission = self.calculate_commission(position_size);
        let slippage = self.config.trading_costs.one_way_slippage(price, position_size, volatility);
        
        let mut trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,
            position_size,
            spot,
            Some(greeks),
            commission,
        );
        trade.mid_price = price;
        trade.slippage = slippage;
        
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
                let config = BinomialConfig {
                    use_dividends: self.config.dividend_yield > 0.0,
                    dividend_yield: self.config.dividend_yield,
                    ..BinomialConfig::default()
                };
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
        // Partial-fill model: in panic regimes only a fraction of the order executes.
        let position_size = self.config.trading_costs.apply_partial_fill(position_size, volatility);
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
        
        // Effective fill price when buying (we pay more than mid due to spread)
        let effective_price = self.effective_price_ctx(price, true, volatility, position_size);
        let commission = self.calculate_commission(position_size);
        let slippage = self.config.trading_costs.one_way_slippage(price, position_size, volatility);
        
        let mut trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,
            position_size,
            spot,
            Some(greeks),
            commission,
        );
        trade.mid_price = price;
        trade.slippage = slippage;
        
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
        // Partial-fill model: in panic regimes only a fraction of the order executes.
        let position_size = self.config.trading_costs.apply_partial_fill(position_size, volatility);
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
        
        // Effective fill price when selling (we get less than mid due to spread)
        let effective_price = self.effective_price_ctx(greeks.price, false, volatility, position_size as i32);
        let commission = self.calculate_commission(position_size as i32);
        let slippage = self.config.trading_costs.one_way_slippage(greeks.price, position_size as i32, volatility);
        
        let mut trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,
            -(position_size as i32),  // Negative for short
            spot,
            Some(greeks),
            commission,
        );
        trade.mid_price = greeks.price;
        trade.slippage = slippage;
        
        // For short options: credit received = effective_price × qty × 100 − commission
        self.current_capital += trade.proceeds();
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
        
        // Partial-fill model: in panic regimes only a fraction of the order executes.
        let position_size = self.config.trading_costs.apply_partial_fill(position_size, volatility);
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
        
        // Effective fill price when selling (we get less than mid due to spread)
        let effective_price = self.effective_price_ctx(greeks.price, false, volatility, position_size as i32);
        let commission = self.calculate_commission(position_size as i32);
        let slippage = self.config.trading_costs.one_way_slippage(greeks.price, position_size as i32, volatility);
        
        let mut trade = Trade::new(
            self.position_counter,
            TradeType::Entry,
            date.to_string(),
            symbol.to_string(),
            effective_price,
            -(position_size as i32),  // Negative for short
            spot,
            Some(greeks),
            commission,
        );
        trade.mid_price = greeks.price;
        trade.slippage = slippage;
        
        // For short puts: credit received = effective_price × qty × 100 − commission
        self.current_capital += trade.proceeds();
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
        volatility: f64,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        
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
        volatility: f64,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        
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
        volatility: f64,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        
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
        volatility: f64,
    ) {
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        
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
        
        let effective_price = self.effective_price_ctx(price, quantity > 0, volatility, quantity.abs());
        let commission = self.calculate_commission(quantity.abs());
        let slippage = self.config.trading_costs.one_way_slippage(price, quantity.abs(), volatility);
        
        let mut trade = Trade::new(
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
        trade.mid_price = price;
        trade.slippage = slippage;
        
        // Long legs are debits (we pay), short legs are credits (we receive)
        if quantity > 0 {
            self.current_capital -= trade.total_cost();
        } else {
            self.current_capital += trade.proceeds();
        }
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
            let abs_qty = quantity.unsigned_abs() as i32;
            
            // Now call methods (no mutable borrow conflict)
            // is_selling=true (long exit → selling) means !is_selling=false=not buying
            let effective_exit_price = self.effective_price_ctx(exit_price, !is_selling, volatility, abs_qty);
            let commission = self.calculate_commission(quantity);
            let slippage = (effective_exit_price - exit_price).abs();
            
            let mut trade = Trade::new(
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
            trade.mid_price = exit_price;
            trade.slippage = slippage;
            
            // Calculate P&L:
            // Long exit  (selling to close): receive proceeds − commission
            // Short exit (buying to cover):  pay fill price + commission
            if quantity > 0 {
                self.current_capital += trade.proceeds();
            } else {
                self.current_capital -= trade.total_cost();
            }
            self.trades.push(trade);
        }
    }
    
    fn close_all_positions(&mut self, date: &str, spot: f64) {
        let positions_to_close: Vec<_> = self.positions.iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.status, PositionStatus::Open))
            .map(|(idx, p)| {
                let is_call = matches!(p.option_type, OptionType::Call);
                (idx, p.entry_date.clone(), is_call, p.strike, p.quantity)
            })
            .collect();
        
        for (idx, entry_date, is_call, strike, quantity) in positions_to_close {
            let days_held = self.calculate_days_between(&entry_date, date);
            
            // Calculate intrinsic value at expiry
            let intrinsic = if is_call {
                (spot - strike).max(0.0)
            } else {
                (strike - spot).max(0.0)
            };
            
            if intrinsic > 0.0 {
                // ITM: close at intrinsic value and settle capital
                self.positions[idx].close(intrinsic, date.to_string(), spot, days_held);
                // Long (qty > 0) receives intrinsic; short (qty < 0) pays intrinsic
                self.current_capital += intrinsic * quantity as f64 * 100.0;
            } else {
                // OTM: expires worthless
                self.positions[idx].expire(date.to_string(), spot, days_held);
            }
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


