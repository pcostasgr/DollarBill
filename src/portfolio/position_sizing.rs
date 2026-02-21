// Position sizing algorithms for portfolio management

/// Position sizing method
#[derive(Debug, Clone, Copy)]
pub enum SizingMethod {
    /// Fixed percentage of account
    FixedFractional(f64),
    /// Kelly Criterion (optimal growth rate)
    KellyCriterion,
    /// Volatility-adjusted sizing
    VolatilityBased,
    /// Risk parity across positions
    RiskParity,
    /// Fixed dollar amount
    FixedDollar(f64),
}

/// Position size calculator
pub struct PositionSizer {
    account_value: f64,
    max_risk_per_trade: f64,  // Maximum risk % per trade
    max_position_pct: f64,    // Maximum position size as % of account
}

impl PositionSizer {
    pub fn new(account_value: f64, max_risk_per_trade: f64, max_position_pct: f64) -> Self {
        Self {
            account_value,
            max_risk_per_trade,
            max_position_pct,
        }
    }

    /// Update account value
    pub fn update_account(&mut self, new_value: f64) {
        self.account_value = new_value;
    }

    /// Calculate position size in contracts
    pub fn calculate_size(
        &self,
        method: SizingMethod,
        option_price: f64,
        volatility: f64,
        win_rate: Option<f64>,
        avg_win: Option<f64>,
        avg_loss: Option<f64>,
    ) -> i32 {
        let size = match method {
            SizingMethod::FixedFractional(pct) => {
                self.fixed_fractional(pct, option_price)
            }
            SizingMethod::KellyCriterion => {
                self.kelly_criterion(win_rate, avg_win, avg_loss, option_price)
            }
            SizingMethod::VolatilityBased => {
                self.volatility_based(volatility, option_price)
            }
            SizingMethod::RiskParity => {
                self.risk_parity(volatility, option_price)
            }
            SizingMethod::FixedDollar(amount) => {
                self.fixed_dollar(amount, option_price)
            }
        };

        // Apply maximum position size limit
        let max_contracts = self.max_position_contracts(option_price);
        size.min(max_contracts)
    }

    /// Fixed fractional position sizing
    fn fixed_fractional(&self, pct: f64, option_price: f64) -> i32 {
        let position_value = self.account_value * (pct / 100.0);
        let contract_value = option_price * 100.0; // Options are per 100 shares
        (position_value / contract_value).floor() as i32
    }

    /// Kelly Criterion: f* = (bp - q) / b
    /// where b = avg_win/avg_loss, p = win_rate, q = 1-p
    fn kelly_criterion(
        &self,
        win_rate: Option<f64>,
        avg_win: Option<f64>,
        avg_loss: Option<f64>,
        option_price: f64,
    ) -> i32 {
        // Need historical stats for Kelly
        let p = win_rate.unwrap_or(0.5);
        let w = avg_win.unwrap_or(option_price);
        let l = avg_loss.unwrap_or(option_price).abs();
        
        if l == 0.0 {
            return self.fixed_fractional(2.0, option_price);
        }

        let b = w / l;  // Payoff ratio
        let q = 1.0 - p;
        let kelly_pct = ((b * p - q) / b).max(0.0).min(0.25); // Cap at 25% (fractional Kelly)
        
        let position_value = self.account_value * kelly_pct;
        let contract_value = option_price * 100.0;
        (position_value / contract_value).floor() as i32
    }

    /// Volatility-based sizing: higher volatility = smaller size
    fn volatility_based(&self, volatility: f64, option_price: f64) -> i32 {
        // Target risk level as percentage of account
        let target_risk = self.max_risk_per_trade;
        
        // Adjust for volatility (annual volatility)
        let vol_adjustment = if volatility > 0.0 {
            1.0 / volatility.sqrt()
        } else {
            1.0
        };
        
        let adjusted_pct = (target_risk * vol_adjustment).min(self.max_position_pct);
        let position_value = self.account_value * (adjusted_pct / 100.0);
        let contract_value = option_price * 100.0;
        
        (position_value / contract_value).floor() as i32
    }

    /// Risk parity: equal risk contribution
    fn risk_parity(&self, volatility: f64, option_price: f64) -> i32 {
        // Allocate inversely proportional to volatility
        let vol = volatility.max(0.01); // Avoid division by zero
        let base_allocation = 1.0 / vol;
        
        // Scale to account size
        let position_value = (self.account_value * base_allocation * 0.01).min(
            self.account_value * (self.max_position_pct / 100.0)
        );
        
        let contract_value = option_price * 100.0;
        (position_value / contract_value).floor() as i32
    }

    /// Fixed dollar amount
    fn fixed_dollar(&self, amount: f64, option_price: f64) -> i32 {
        let contract_value = option_price * 100.0;
        (amount / contract_value).floor() as i32
    }

    /// Maximum contracts based on position size limit
    fn max_position_contracts(&self, option_price: f64) -> i32 {
        let max_value = self.account_value * (self.max_position_pct / 100.0);
        let contract_value = option_price * 100.0;
        (max_value / contract_value).floor() as i32
    }

    /// Calculate risk amount for a position
    pub fn position_risk(&self, contracts: i32, option_price: f64) -> f64 {
        (contracts as f64) * option_price * 100.0
    }

    /// Check if position fits within risk limits
    pub fn validate_position(&self, contracts: i32, option_price: f64) -> bool {
        let risk = self.position_risk(contracts, option_price);
        let risk_pct = (risk / self.account_value) * 100.0;
        
        risk_pct <= self.max_position_pct && contracts > 0
    }
}

/// Multi-leg position sizing
pub struct MultiLegSizer {
    sizer: PositionSizer,
}

impl MultiLegSizer {
    pub fn new(account_value: f64, max_risk_per_trade: f64, max_position_pct: f64) -> Self {
        Self {
            sizer: PositionSizer::new(account_value, max_risk_per_trade, max_position_pct),
        }
    }

    /// Calculate size for iron condor (4 legs)
    pub fn iron_condor_size(
        &self,
        method: SizingMethod,
        max_loss: f64,  // Maximum loss if breached
        net_credit: f64, // Net credit received
        volatility: f64,
    ) -> i32 {
        // Size based on maximum potential loss, not credit
        let risk_amount = self.sizer.account_value * (self.sizer.max_risk_per_trade / 100.0);
        
        // How many spreads fit in our risk budget?
        let contracts = (risk_amount / max_loss).floor() as i32;
        
        // Validate against volatility
        let vol_adjusted = self.sizer.volatility_based(volatility, net_credit);
        
        contracts.min(vol_adjusted).max(1)
    }

    /// Calculate size for credit spread (2 legs)
    pub fn credit_spread_size(
        &self,
        method: SizingMethod,
        spread_width: f64,
        net_credit: f64,
        volatility: f64,
    ) -> i32 {
        // Max loss = spread width - credit
        let max_loss = (spread_width - net_credit) * 100.0;
        let risk_amount = self.sizer.account_value * (self.sizer.max_risk_per_trade / 100.0);
        
        let contracts = (risk_amount / max_loss).floor() as i32;
        let vol_adjusted = self.sizer.volatility_based(volatility, net_credit);
        
        contracts.min(vol_adjusted).max(1)
    }

    /// Update account value
    pub fn update_account(&mut self, new_value: f64) {
        self.sizer.update_account(new_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_fractional_sizing() {
        let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
        
        // 5% of $100k = $5k, option price $2.50 => 20 contracts
        let size = sizer.calculate_size(
            SizingMethod::FixedFractional(5.0),
            2.50,
            0.30,
            None,
            None,
            None,
        );
        
        assert_eq!(size, 20); // $5000 / ($2.50 * 100) = 20
    }

    #[test]
    fn test_max_position_limit() {
        let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
        
        // Try to allocate 20% but max is 10%
        let size = sizer.calculate_size(
            SizingMethod::FixedFractional(20.0),
            1.0,
            0.30,
            None,
            None,
            None,
        );
        
        // Should be capped at 10% = $10k / $100 = 100 contracts
        assert_eq!(size, 100);
    }

    #[test]
    fn test_volatility_based_sizing() {
        let sizer = PositionSizer::new(100_000.0, 2.0, 15.0);
        
        // Low volatility should give larger size
        let low_vol_size = sizer.volatility_based(0.20, 2.0);
        
        // High volatility should give smaller size
        let high_vol_size = sizer.volatility_based(0.80, 2.0);
        
        assert!(low_vol_size > high_vol_size);
    }

    #[test]
    fn test_kelly_criterion() {
        let sizer = PositionSizer::new(100_000.0, 2.0, 25.0);
        
        // 60% win rate, avg win $300, avg loss $200
        let size = sizer.calculate_size(
            SizingMethod::KellyCriterion,
            2.0,
            0.30,
            Some(0.60),
            Some(300.0),
            Some(200.0),
        );
        
        assert!(size > 0);
        assert!(size <= 125); // Max 25% of account
    }

    #[test]
    fn test_iron_condor_sizing() {
        let sizer = MultiLegSizer::new(100_000.0, 2.0, 10.0);
        
        // Max loss $500 per spread, credit $150, 30% IV
        let size = sizer.iron_condor_size(
            SizingMethod::FixedFractional(2.0),
            500.0,
            1.50,
            0.30,
        );
        
        // $2k risk / $500 max loss = 4 contracts
        assert!(size >= 1 && size <= 10);
    }

    #[test]
    fn test_credit_spread_sizing() {
        let sizer = MultiLegSizer::new(50_000.0, 1.5, 8.0);
        
        // $5 spread, $1.50 credit, 25% IV
        let size = sizer.credit_spread_size(
            SizingMethod::FixedFractional(1.5),
            5.0,
            1.50,
            0.25,
        );
        
        // Max loss = ($5 - $1.50) * 100 = $350 per spread
        // $750 risk / $350 = 2 contracts
        assert!(size >= 1);
    }

    #[test]
    fn test_position_validation() {
        let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
        
        // Valid: 20 contracts @ $2.50 = $5k (5%)
        assert!(sizer.validate_position(20, 2.50));
        
        // Invalid: 200 contracts @ $2.50 = $50k (50%)
        assert!(!sizer.validate_position(200, 2.50));
        
        // Invalid: 0 contracts
        assert!(!sizer.validate_position(0, 2.50));
    }
}
