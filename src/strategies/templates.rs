// Multi-leg option strategy templates
// Provides configurable templates for common options strategies

use crate::strategies::SignalAction;

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
    pub fn max_profit_estimate(&self, spot: f64, _volatility: f64) -> f64 {
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
        
        assert_eq!(signals.len(), 4, "Iron condor should have 4 legs");
        
        // Verify strikes are in correct order (with floating point tolerance)
        // Expected: Buy Put @ 90, Sell Put @ 95, Sell Call @ 105, Buy Call @ 110
        if let SignalAction::SellPut { strike, .. } = &signals[0] {
            assert!((*strike - 95.0).abs() < 0.01, "Sell put strike should be at 95% of spot");
        } else {
            panic!("First leg should be SellPut");
        }
        
        if let SignalAction::BuyPut { strike, .. } = &signals[1] {
            assert!((*strike - 90.0).abs() < 0.01, "Buy put strike should be at 90% of spot");
        } else {
            panic!("Second leg should be BuyPut");
        }
        
        if let SignalAction::SellCall { strike, .. } = &signals[2] {
            assert!((*strike - 105.0).abs() < 0.01, "Sell call strike should be at 105% of spot");
        } else {
            panic!("Third leg should be SellCall");
        }
        
        if let SignalAction::BuyCall { strike, .. } = &signals[3] {
            assert!((*strike - 110.0).abs() < 0.01, "Buy call strike should be at 110% of spot");
        } else {
            panic!("Fourth leg should be BuyCall");
        }
    }

    #[test]
    fn test_iron_condor_custom_config() {
        let config = IronCondorConfig {
            days_to_expiry: 60,
            sell_put_pct: 0.93,
            buy_put_pct: 0.88,
            sell_call_pct: 1.07,
            buy_call_pct: 1.12,
        };
        
        let signals = config.generate_signals(200.0, 0.30);
        assert_eq!(signals.len(), 4);
        
        // Verify custom strikes with spot at $200
        if let SignalAction::SellPut { strike, days_to_expiry, .. } = &signals[0] {
            assert!((*strike - 186.0).abs() < 0.01); // 93% of 200
            assert_eq!(*days_to_expiry, 60);
        }
        
        if let SignalAction::BuyPut { strike, .. } = &signals[1] {
            assert!((*strike - 176.0).abs() < 0.01); // 88% of 200
        }
    }

    #[test]
    fn test_bull_put_spread() {
        let config = BullPutSpreadConfig::default();
        let signals = config.generate_signals(100.0, 0.25);
        
        assert_eq!(signals.len(), 2, "Bull put spread should have 2 legs");
        
        // First should be sell put (higher strike)
        if let SignalAction::SellPut { strike, .. } = &signals[0] {
            assert!((*strike - 97.0).abs() < 0.01);
        } else {
            panic!("First leg should be SellPut");
        }
        
        // Second should be buy put (lower strike)
        if let SignalAction::BuyPut { strike, .. } = &signals[1] {
            assert!((*strike - 92.0).abs() < 0.01);
        } else {
            panic!("Second leg should be BuyPut");
        }
    }

    #[test]
    fn test_bear_call_spread() {
        let config = BearCallSpreadConfig::default();
        let signals = config.generate_signals(100.0, 0.25);
        
        assert_eq!(signals.len(), 2, "Bear call spread should have 2 legs");
        
        // First should be sell call (lower strike)
        if let SignalAction::SellCall { strike, .. } = &signals[0] {
            assert!((*strike - 103.0).abs() < 0.01);
        } else {
            panic!("First leg should be SellCall");
        }
        
        // Second should be buy call (higher strike)
        if let SignalAction::BuyCall { strike, .. } = &signals[1] {
            assert!((*strike - 108.0).abs() < 0.01);
        } else {
            panic!("Second leg should be BuyCall");
        }
    }

    #[test]
    fn test_short_straddle() {
        let config = ShortStraddleConfig::default();
        let signals = config.generate_signals(150.0, 0.35);
        
        assert_eq!(signals.len(), 2);
        
        // Both strikes should be ATM
        if let SignalAction::SellCall { strike, .. } = &signals[0] {
            assert!((*strike - 150.0).abs() < 0.01);
        }
        
        if let SignalAction::SellPut { strike, .. } = &signals[1] {
            assert!((*strike - 150.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_short_strangle() {
        let config = ShortStrangleConfig::default();
        let signals = config.generate_signals(100.0, 0.25);
        
        assert_eq!(signals.len(), 2);
        
        // Put should be below spot, call above
        if let SignalAction::SellPut { strike, .. } = &signals[0] {
            assert!((*strike - 95.0).abs() < 0.01);
        }
        
        if let SignalAction::SellCall { strike, .. } = &signals[1] {
            assert!((*strike - 105.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_covered_call() {
        let config = CoveredCallConfig::default();
        let signals = config.generate_signals(100.0, 0.20);
        
        assert_eq!(signals.len(), 1);
        
        if let SignalAction::SellCall { strike, days_to_expiry, .. } = &signals[0] {
            assert!((*strike - 105.0).abs() < 0.01);
            assert_eq!(*days_to_expiry, 30);
        } else {
            panic!("Should be SellCall");
        }
    }

    #[test]
    fn test_cash_secured_put() {
        let config = CashSecuredPutConfig::default();
        let signals = config.generate_signals(100.0, 0.20);
        
        assert_eq!(signals.len(), 1);
        
        if let SignalAction::SellPut { strike, days_to_expiry, .. } = &signals[0] {
            assert!((*strike - 95.0).abs() < 0.01);
            assert_eq!(*days_to_expiry, 30);
        } else {
            panic!("Should be SellPut");
        }
    }

    #[test]
    fn test_iron_condor_with_different_spot_prices() {
        let config = IronCondorConfig::default();
        
        // Test with spot at $50
        let signals_50 = config.generate_signals(50.0, 0.25);
        if let SignalAction::SellPut { strike, .. } = &signals_50[0] {
            assert_eq!(*strike, 47.5); // 95% of 50
        }
        
        // Test with spot at $500
        let signals_500 = config.generate_signals(500.0, 0.25);
        if let SignalAction::SellPut { strike, .. } = &signals_500[0] {
            assert_eq!(*strike, 475.0); // 95% of 500
        }
    }

    #[test]
    fn test_volatility_passed_to_signals() {
        let config = IronCondorConfig::default();
        let test_vol = 0.42;
        let signals = config.generate_signals(100.0, test_vol);
        
        // Verify volatility is passed through
        for signal in signals {
            match signal {
                SignalAction::SellPut { volatility, .. } |
                SignalAction::BuyPut { volatility, .. } |
                SignalAction::SellCall { volatility, .. } |
                SignalAction::BuyCall { volatility, .. } => {
                    assert_eq!(volatility, test_vol);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_days_to_expiry_consistency() {
        let test_dte = 21;
        let config = IronCondorConfig {
            days_to_expiry: test_dte,
            ..Default::default()
        };
        
        let signals = config.generate_signals(100.0, 0.25);
        
        // All legs should have same DTE
        for signal in signals {
            match signal {
                SignalAction::SellPut { days_to_expiry, .. } |
                SignalAction::BuyPut { days_to_expiry, .. } |
                SignalAction::SellCall { days_to_expiry, .. } |
                SignalAction::BuyCall { days_to_expiry, .. } => {
                    assert_eq!(days_to_expiry, test_dte);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_spread_widths() {
        // Test that spreads maintain proper width relationships
        let config = IronCondorConfig {
            days_to_expiry: 45,
            sell_put_pct: 0.96,
            buy_put_pct: 0.92,   // 4% width
            sell_call_pct: 1.04,
            buy_call_pct: 1.08,  // 4% width
        };
        
        let signals = config.generate_signals(100.0, 0.25);
        
        // Extract strikes
        let mut strikes = Vec::new();
        for signal in signals {
            match signal {
                SignalAction::SellPut { strike, .. } |
                SignalAction::BuyPut { strike, .. } |
                SignalAction::SellCall { strike, .. } |
                SignalAction::BuyCall { strike, .. } => {
                    strikes.push(strike);
                }
                _ => {}
            }
        }
        
        // Verify put spread width
        let put_width = (strikes[0] - strikes[1]).abs(); // Sell - Buy
        assert!((put_width - 4.0).abs() < 0.01, "Put spread width should be ~4.0");
        
        // Verify call spread width
        let call_width = (strikes[3] - strikes[2]).abs(); // Buy - Sell
        assert!((call_width - 4.0).abs() < 0.01, "Call spread width should be ~4.0");
    }
}
