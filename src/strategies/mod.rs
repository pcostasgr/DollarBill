// Trading strategies module
use crate::models::heston::HestonParams;

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
    SellStraddle,
    BuyStraddle,
    IronButterfly { wing_width: f64 },
    CashSecuredPut { strike_pct: f64 },
    NoAction,
}

#[derive(Debug, Clone)]
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
