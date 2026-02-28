// Trading strategies module

/// Core trait all strategies must implement
pub trait TradingStrategy: Send + Sync {
    /// Strategy name for logging
    fn name(&self) -> &str;
    
    /// Generate trading signals from market data
    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal>;
    
    /// Risk parameters
    fn risk_params(&self) -> RiskParams;
}

#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub symbol: String,
    pub action: SignalAction,
    pub strike: f64,
    pub expiry_days: usize,
    pub confidence: f64,
    pub edge: f64,
    pub strategy_name: String,
}

#[derive(Debug, Clone)]
pub enum SignalAction {
    // Single option signals
    BuyCall { strike: f64, days_to_expiry: usize, volatility: f64 },
    BuyPut { strike: f64, days_to_expiry: usize, volatility: f64 },
    SellCall { strike: f64, days_to_expiry: usize, volatility: f64 },
    SellPut { strike: f64, days_to_expiry: usize, volatility: f64 },
    
    // Position management
    ClosePosition { position_id: usize },
    
    // Multi-leg strategies
    SellStraddle,
    BuyStraddle,
    IronButterfly { wing_width: f64 },
    CashSecuredPut { strike_pct: f64 },
    
    // Spread strategies (Phase 5)
    IronCondor { 
        sell_call_strike: f64, 
        buy_call_strike: f64, 
        sell_put_strike: f64, 
        buy_put_strike: f64,
        days_to_expiry: usize 
    },
    CreditCallSpread { 
        sell_strike: f64, 
        buy_strike: f64, 
        days_to_expiry: usize 
    },
    CreditPutSpread { 
        sell_strike: f64, 
        buy_strike: f64, 
        days_to_expiry: usize 
    },
    CoveredCall { 
        sell_strike: f64, 
        days_to_expiry: usize 
    },
    
    NoAction,
}

impl SignalAction {
    /// Get sell put strike for iron condor
    pub fn iron_condor_sell_put_strike(&self) -> f64 {
        match self {
            SignalAction::IronCondor { sell_put_strike, .. } => *sell_put_strike,
            _ => 0.0,
        }
    }

    /// Get sell call strike for iron condor
    pub fn iron_condor_sell_call_strike(&self) -> f64 {
        match self {
            SignalAction::IronCondor { sell_call_strike, .. } => *sell_call_strike,
            _ => 0.0,
        }
    }

    /// Get sell strike for credit call spread
    pub fn credit_call_spread_sell_strike(&self) -> f64 {
        match self {
            SignalAction::CreditCallSpread { sell_strike, .. } => *sell_strike,
            _ => 0.0,
        }
    }

    /// Get buy strike for credit call spread
    pub fn credit_call_spread_buy_strike(&self) -> f64 {
        match self {
            SignalAction::CreditCallSpread { buy_strike, .. } => *buy_strike,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Part of strategy API
pub struct RiskParams {
    pub max_position_size: f64,
    pub max_delta: f64,
    pub max_vega: f64,
    pub stop_loss_pct: f64,
}

/// Strategy registry to manage multiple strategies
pub struct StrategyRegistry {
    strategies: Vec<Box<dyn TradingStrategy>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        Self { strategies: vec![] }
    }
    
    pub fn register(&mut self, strategy: Box<dyn TradingStrategy>) {
        self.strategies.push(strategy);
    }
    
    pub fn generate_all_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        let mut all_signals = vec![];
        
        for strategy in &self.strategies {
            let signals = strategy.generate_signals(symbol, spot, market_iv, model_iv, historical_vol);
            all_signals.extend(signals);
        }
        
        all_signals
    }
    
    pub fn list_strategies(&self) -> Vec<String> {
        self.strategies.iter().map(|s| s.name().to_string()).collect()
    }
}

// Strategy implementations
pub mod vol_mean_reversion;
pub mod momentum;
pub mod cash_secured_puts;
pub mod ensemble;
pub mod factory;
pub mod matching;
pub mod mean_reversion;
pub mod breakout;
pub mod vol_arbitrage;
pub mod tests;

// Multi-leg option strategy templates
pub mod templates;

// Short options mispricing detection
pub mod mispricing;

// Multi-leg spread strategies (Phase 5)
pub mod spreads;

// Short strangle strategy (Phase 6)
pub mod short_strangle;
