#![allow(dead_code)]
// Market option data structures for calibration

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionType {
    Call,
    Put,
}

/// Market-observed option data point
#[derive(Debug, Clone)]
pub struct MarketOption {
    pub strike: f64,
    pub time_to_expiry: f64,  // Years
    pub bid: f64,
    pub ask: f64,
    pub option_type: OptionType,
    pub volume: i32,
    pub open_interest: i32,
}

impl MarketOption {
    /// Get mid-point price (average of bid/ask)
    pub fn mid_price(&self) -> f64 {
        (self.bid + self.ask) / 2.0
    }
    
    /// Get bid-ask spread in dollars
    pub fn spread(&self) -> f64 {
        self.ask - self.bid
    }
    
    /// Get bid-ask spread as percentage of mid
    pub fn spread_pct(&self) -> f64 {
        self.spread() / self.mid_price() * 100.0
    }
    
    /// Check if option is liquid enough for calibration
    pub fn is_liquid(&self, min_volume: i32, max_spread_pct: f64) -> bool {
        self.volume >= min_volume && self.spread_pct() <= max_spread_pct
    }
}

/// Liquidity filter criteria
#[derive(Debug, Clone)]
pub struct LiquidityFilter {
    pub min_volume: i32,
    pub max_spread_pct: f64,
    pub min_days_to_expiry: f64,
    pub max_days_to_expiry: f64,
}

impl Default for LiquidityFilter {
    fn default() -> Self {
        Self {
            min_volume: 50,         // At least 50 contracts traded
            max_spread_pct: 10.0,   // Spread < 10% of mid
            min_days_to_expiry: 7.0,   // At least 1 week
            max_days_to_expiry: 90.0,  // Max ~3 months
        }
    }
}

impl LiquidityFilter {
    /// Filter market data to only liquid options
    pub fn apply(&self, options: &[MarketOption]) -> Vec<MarketOption> {
        options.iter()
            .filter(|opt| {
                let days = opt.time_to_expiry * 365.0;
                opt.is_liquid(self.min_volume, self.max_spread_pct)
                    && days >= self.min_days_to_expiry
                    && days <= self.max_days_to_expiry
            })
            .cloned()
            .collect()
    }
}
