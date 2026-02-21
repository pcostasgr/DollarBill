// Multi-leg option strategy templates
// Provides configurable templates for common options strategies

use crate::backtesting::engine::SignalAction;

/// Configuration for Iron Condor strategy
#[derive(Debug, Clone)]
pub struct IronCondorConfig {
    /// Days to expiration for all legs
    pub days_to_expiry: usize,
    /// Lower put spread: sell at this % of spot
    pub sell_put_pct: f64,
    /// Lower put spread: buy at this % of spot
    pub buy_put_pct: f64,
    /// Upper call spread: sell at this % of spot
    pub sell_call_pct: f64,
    /// Upper call spread: buy at this % of spot
    pub buy_call_pct: f64,
}

impl Default for IronCondorConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 45,
            sell_put_pct: 0.95,   // Sell put 5% below spot
            buy_put_pct: 0.90,    // Buy put 10% below spot
            sell_call_pct: 1.05,  // Sell call 5% above spot
            buy_call_pct: 1.10,   // Buy call 10% above spot
        }
    }
}

impl IronCondorConfig {
    /// Generate iron condor signals for given market conditions
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        vec![
            // Put spread (lower side)
            SignalAction::SellPut {
                strike: spot * self.sell_put_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            SignalAction::BuyPut {
                strike: spot * self.buy_put_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            // Call spread (upper side)
            SignalAction::SellCall {
                strike: spot * self.sell_call_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            SignalAction::BuyCall {
                strike: spot * self.buy_call_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
    
    /// Calculate theoretical max profit (net credit)
    pub fn max_profit_estimate(&self, spot: f64, volatility: f64) -> f64 {
        // This is a simplified estimate - actual calculation requires option pricing
        let put_spread_width = spot * (self.sell_put_pct - self.buy_put_pct);
        let call_spread_width = spot * (self.buy_call_pct - self.sell_call_pct);
        
        // Rough estimate: credit is typically 20-40% of spread width
        (put_spread_width + call_spread_width) * 0.30 * 100.0 // per contract
    }
    
    /// Calculate max loss (width - credit)
    pub fn max_loss_estimate(&self, spot: f64, volatility: f64) -> f64 {
        let spread_width = spot * (self.buy_call_pct - self.sell_call_pct);
        spread_width * 100.0 - self.max_profit_estimate(spot, volatility)
    }
}

/// Configuration for Bull Put Spread (credit spread, bullish bias)
#[derive(Debug, Clone)]
pub struct BullPutSpreadConfig {
    pub days_to_expiry: usize,
    pub sell_put_pct: f64,  // Higher strike (sell)
    pub buy_put_pct: f64,   // Lower strike (buy)
}

impl Default for BullPutSpreadConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 30,
            sell_put_pct: 0.97,  // Sell put 3% below spot
            buy_put_pct: 0.92,   // Buy put 8% below spot
        }
    }
}

impl BullPutSpreadConfig {
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        vec![
            SignalAction::SellPut {
                strike: spot * self.sell_put_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            SignalAction::BuyPut {
                strike: spot * self.buy_put_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
}

/// Configuration for Bear Call Spread (credit spread, bearish bias)
#[derive(Debug, Clone)]
pub struct BearCallSpreadConfig {
    pub days_to_expiry: usize,
    pub sell_call_pct: f64,  // Lower strike (sell)
    pub buy_call_pct: f64,   // Higher strike (buy)
}

impl Default for BearCallSpreadConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 30,
            sell_call_pct: 1.03,  // Sell call 3% above spot
            buy_call_pct: 1.08,   // Buy call 8% above spot
        }
    }
}

impl BearCallSpreadConfig {
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        vec![
            SignalAction::SellCall {
                strike: spot * self.sell_call_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            SignalAction::BuyCall {
                strike: spot * self.buy_call_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
}

/// Configuration for Short Straddle (high premium, unlimited risk)
#[derive(Debug, Clone)]
pub struct ShortStraddleConfig {
    pub days_to_expiry: usize,
    pub strike_pct: f64,  // Typically ATM (1.00)
}

impl Default for ShortStraddleConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 30,
            strike_pct: 1.00,  // At the money
        }
    }
}

impl ShortStraddleConfig {
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        let strike = spot * self.strike_pct;
        vec![
            SignalAction::SellCall {
                strike,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            SignalAction::SellPut {
                strike,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
}

/// Configuration for Short Strangle (like straddle but OTM strikes)
#[derive(Debug, Clone)]
pub struct ShortStrangleConfig {
    pub days_to_expiry: usize,
    pub put_strike_pct: f64,   // Below spot
    pub call_strike_pct: f64,  // Above spot
}

impl Default for ShortStrangleConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 30,
            put_strike_pct: 0.95,   // 5% below
            call_strike_pct: 1.05,  // 5% above
        }
    }
}

impl ShortStrangleConfig {
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        vec![
            SignalAction::SellPut {
                strike: spot * self.put_strike_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
            SignalAction::SellCall {
                strike: spot * self.call_strike_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
}

/// Configuration for Covered Call (sell call against stock position)
#[derive(Debug, Clone)]
pub struct CoveredCallConfig {
    pub days_to_expiry: usize,
    pub call_strike_pct: f64,  // Typically slightly OTM
}

impl Default for CoveredCallConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 30,
            call_strike_pct: 1.05,  // 5% above current price
        }
    }
}

impl CoveredCallConfig {
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        vec![
            SignalAction::SellCall {
                strike: spot * self.call_strike_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
}

/// Configuration for Cash-Secured Put
#[derive(Debug, Clone)]
pub struct CashSecuredPutConfig {
    pub days_to_expiry: usize,
    pub put_strike_pct: f64,  // Typically slightly OTM
}

impl Default for CashSecuredPutConfig {
    fn default() -> Self {
        Self {
            days_to_expiry: 30,
            put_strike_pct: 0.95,  // 5% below current price
        }
    }
}

impl CashSecuredPutConfig {
    pub fn generate_signals(&self, spot: f64, volatility: f64) -> Vec<SignalAction> {
        vec![
            SignalAction::SellPut {
                strike: spot * self.put_strike_pct,
                days_to_expiry: self.days_to_expiry,
                volatility,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iron_condor_default() {
        let config = IronCondorConfig::default();
        let signals = config.generate_signals(100.0, 0.25);
        
        assert_eq!(signals.len(), 4);
        // Verify it's properly structured with 2 puts and 2 calls
    }

    #[test]
    fn test_bull_put_spread() {
        let config = BullPutSpreadConfig::default();
        let signals = config.generate_signals(100.0, 0.25);
        
        assert_eq!(signals.len(), 2);
    }

    #[test]
    fn test_bear_call_spread() {
        let config = BearCallSpreadConfig::default();
        let signals = config.generate_signals(100.0, 0.25);
        
        assert_eq!(signals.len(), 2);
    }
}
