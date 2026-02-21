// Multi-strategy allocation and portfolio optimization

use std::collections::HashMap;
use crate::backtesting::position::Position;

/// Strategy allocation
#[derive(Debug, Clone)]
pub struct StrategyAllocation {
    pub strategy_name: String,
    pub target_pct: f64,       // Target allocation %
    pub current_pct: f64,      // Current allocation %
    pub capacity: f64,         // Maximum capital this strategy can handle
    pub min_allocation: f64,   // Minimum allocation %
    pub max_allocation: f64,   // Maximum allocation %
}

/// Allocation method
#[derive(Debug, Clone, Copy)]
pub enum AllocationMethod {
    /// Equal weight across all strategies
    EqualWeight,
    /// Risk parity (equal risk contribution)
    RiskParity,
    /// Performance-weighted (better performers get more)
    PerformanceWeighted,
    /// Volatility-weighted (inverse volatility)
    VolatilityWeighted,
    /// Custom weights
    Custom,
}

/// Portfolio allocator
pub struct PortfolioAllocator {
    total_capital: f64,
    allocations: HashMap<String, StrategyAllocation>,
    method: AllocationMethod,
}

impl PortfolioAllocator {
    pub fn new(total_capital: f64, method: AllocationMethod) -> Self {
        Self {
            total_capital,
            allocations: HashMap::new(),
            method,
        }
    }

    /// Add a strategy to the allocation mix
    pub fn add_strategy(
        &mut self,
        name: String,
        capacity: f64,
        min_pct: f64,
        max_pct: f64,
    ) {
        let allocation = StrategyAllocation {
            strategy_name: name.clone(),
            target_pct: 0.0,
            current_pct: 0.0,
            capacity,
            min_allocation: min_pct,
            max_allocation: max_pct,
        };
        self.allocations.insert(name, allocation);
    }

    /// Calculate optimal allocations based on method
    pub fn calculate_allocations(
        &mut self,
        strategy_stats: &HashMap<String, StrategyStats>,
    ) {
        match self.method {
            AllocationMethod::EqualWeight => self.equal_weight_allocation(),
            AllocationMethod::RiskParity => self.risk_parity_allocation(strategy_stats),
            AllocationMethod::PerformanceWeighted => self.performance_weighted_allocation(strategy_stats),
            AllocationMethod::VolatilityWeighted => self.volatility_weighted_allocation(strategy_stats),
            AllocationMethod::Custom => {}, // Custom set externally
        }
    }

    /// Equal weight allocation
    fn equal_weight_allocation(&mut self) {
        let n = self.allocations.len() as f64;
        if n == 0.0 {
            return;
        }
        
        let equal_pct = 100.0 / n;
        
        for allocation in self.allocations.values_mut() {
            allocation.target_pct = equal_pct.min(allocation.max_allocation)
                                            .max(allocation.min_allocation);
        }
        
        // Normalize to 100%
        self.normalize_allocations();
    }

    /// Risk parity allocation (equal risk contribution)
    fn risk_parity_allocation(&mut self, strategy_stats: &HashMap<String, StrategyStats>) {
        let mut risk_weights = HashMap::new();
        let mut total_inverse_vol = 0.0;
        
        // Calculate inverse volatility weights
        for (name, stats) in strategy_stats {
            let vol = stats.volatility.max(0.01); // Avoid division by zero
            let weight = 1.0 / vol;
            risk_weights.insert(name.clone(), weight);
            total_inverse_vol += weight;
        }
        
        // Assign allocations proportional to inverse volatility
        for (name, allocation) in &mut self.allocations {
            if let Some(&weight) = risk_weights.get(name) {
                let target = (weight / total_inverse_vol) * 100.0;
                allocation.target_pct = target.min(allocation.max_allocation)
                                             .max(allocation.min_allocation);
            }
        }
        
        self.normalize_allocations();
    }

    /// Performance-weighted allocation
    fn performance_weighted_allocation(&mut self, strategy_stats: &HashMap<String, StrategyStats>) {
        let mut performance_weights = HashMap::new();
        let mut total_sharpe = 0.0;
        
        // Use Sharpe ratio as performance metric
        for (name, stats) in strategy_stats {
            let sharpe = stats.sharpe_ratio.max(0.0); // Only positive Sharpe
            performance_weights.insert(name.clone(), sharpe);
            total_sharpe += sharpe;
        }
        
        if total_sharpe == 0.0 {
            // Fallback to equal weight if all Sharpe ratios are 0
            self.equal_weight_allocation();
            return;
        }
        
        // Assign allocations proportional to Sharpe ratio
        for (name, allocation) in &mut self.allocations {
            if let Some(&sharpe) = performance_weights.get(name) {
                let target = (sharpe / total_sharpe) * 100.0;
                allocation.target_pct = target.min(allocation.max_allocation)
                                             .max(allocation.min_allocation);
            }
        }
        
        self.normalize_allocations();
    }

    /// Volatility-weighted allocation (similar to risk parity)
    fn volatility_weighted_allocation(&mut self, strategy_stats: &HashMap<String, StrategyStats>) {
        self.risk_parity_allocation(strategy_stats); // Same as risk parity
    }

    /// Normalize allocations to sum to 100%
    fn normalize_allocations(&mut self) {
        let total: f64 = self.allocations.values().map(|a| a.target_pct).sum();
        
        if total > 0.0 && (total - 100.0).abs() > 0.01 {
            // If total < 100%, we can scale up respecting max constraints
            // If total > 100%, we need to scale down
            
            if total < 100.0 {
                // Try to scale up, but respect max constraints
                let scale = 100.0 / total;
                let mut can_scale_all = true;
                
                // Check if scaling would violate any max constraint
                for allocation in self.allocations.values() {
                    if allocation.target_pct * scale > allocation.max_allocation {
                        can_scale_all = false;
                        break;
                    }
                }
                
                if can_scale_all {
                    for allocation in self.allocations.values_mut() {
                        allocation.target_pct *= scale;
                    }
                } // else: Leave as-is, constraints prevent reaching 100%
            } else {
                // Scale down to 100%
                let scale = 100.0 / total;
                for allocation in self.allocations.values_mut() {
                    allocation.target_pct *= scale;
                    // Re-apply constraints after scaling
                    allocation.target_pct = allocation.target_pct
                        .min(allocation.max_allocation)
                        .max(allocation.min_allocation);
                }
            }
        }
    }

    /// Update current allocations from positions
    pub fn update_current_allocations(&mut self, positions: &[Position]) {
        // Group positions by strategy
        let mut strategy_values = HashMap::new();
        let mut total_value = 0.0;
        
        for pos in positions {
            let value = pos.entry_price * (pos.quantity.abs() as f64) * 100.0;
            *strategy_values.entry("default".to_string()).or_insert(0.0) += value;
            total_value += value;
        }
        
        // Update current percentages
        for (name, allocation) in &mut self.allocations {
            if let Some(&value) = strategy_values.get(name) {
                allocation.current_pct = if total_value > 0.0 {
                    (value / total_value) * 100.0
                } else {
                    0.0
                };
            } else {
                allocation.current_pct = 0.0;
            }
        }
    }

    /// Get rebalancing trades needed
    pub fn get_rebalancing_trades(&self) -> Vec<RebalanceTrade> {
        let mut trades = Vec::new();
        
        for allocation in self.allocations.values() {
            let diff = allocation.target_pct - allocation.current_pct;
            
            // Only rebalance if difference > 5%
            if diff.abs() > 5.0 {
                let dollar_amount = (diff / 100.0) * self.total_capital;
                trades.push(RebalanceTrade {
                    strategy: allocation.strategy_name.clone(),
                    current_pct: allocation.current_pct,
                    target_pct: allocation.target_pct,
                    dollar_change: dollar_amount,
                    action: if dollar_amount > 0.0 { "BUY" } else { "SELL" }.to_string(),
                });
            }
        }
        
        trades
    }

    /// Check if strategy has capacity for new allocation
    pub fn has_capacity(&self, strategy: &str, additional_capital: f64) -> bool {
        if let Some(allocation) = self.allocations.get(strategy) {
            let current_capital = (allocation.current_pct / 100.0) * self.total_capital;
            current_capital + additional_capital <= allocation.capacity
        } else {
            false
        }
    }

    /// Get allocation for a strategy
    pub fn get_allocation(&self, strategy: &str) -> Option<&StrategyAllocation> {
        self.allocations.get(strategy)
    }

    /// Update total capital
    pub fn update_capital(&mut self, new_capital: f64) {
        self.total_capital = new_capital;
    }

    /// Get all allocations
    pub fn get_all_allocations(&self) -> &HashMap<String, StrategyAllocation> {
        &self.allocations
    }
}

/// Rebalancing trade action
#[derive(Debug, Clone)]
pub struct RebalanceTrade {
    pub strategy: String,
    pub current_pct: f64,
    pub target_pct: f64,
    pub dollar_change: f64,
    pub action: String,
}

/// Strategy statistics for allocation
#[derive(Debug, Clone)]
pub struct StrategyStats {
    pub sharpe_ratio: f64,
    pub volatility: f64,
    pub win_rate: f64,
    pub avg_return: f64,
    pub max_drawdown: f64,
}

impl Default for StrategyStats {
    fn default() -> Self {
        Self {
            sharpe_ratio: 0.0,
            volatility: 0.20, // Default 20% vol
            win_rate: 0.5,
            avg_return: 0.0,
            max_drawdown: 0.0,
        }
    }
}

/// Strategy capacity calculator
pub struct CapacityAnalyzer;

impl CapacityAnalyzer {
    /// Estimate strategy capacity based on market conditions
    pub fn estimate_capacity(
        strategy_type: &str,
        market_liquidity: f64,
    ) -> f64 {
        // Simplified capacity model
        match strategy_type {
            "IronCondor" | "CreditSpreads" => market_liquidity * 0.10, // 10% of liquidity
            "Straddle" | "Strangle" => market_liquidity * 0.05,        // 5% (higher impact)
            "CoveredCall" | "CashSecuredPut" => market_liquidity * 0.20, // 20% (less impact)
            "Momentum" | "MeanReversion" => market_liquidity * 0.15,    // 15%
            _ => market_liquidity * 0.10, // Default 10%
        }
    }

    /// Check if adding position would exceed capacity
    pub fn check_capacity_constraint(
        current_allocation: f64,
        new_position_size: f64,
        total_capacity: f64,
    ) -> bool {
        current_allocation + new_position_size <= total_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_weight_allocation() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
        
        allocator.add_strategy("IronCondor".to_string(), 50_000.0, 10.0, 40.0);
        allocator.add_strategy("CreditSpreads".to_string(), 50_000.0, 10.0, 40.0);
        allocator.add_strategy("Straddles".to_string(), 50_000.0, 10.0, 40.0);
        
        let stats = HashMap::new();
        allocator.calculate_allocations(&stats);
        
        // Should allocate ~33.33% to each
        for allocation in allocator.allocations.values() {
            assert!((allocation.target_pct - 33.33).abs() < 1.0);
        }
    }

    #[test]
    fn test_risk_parity_allocation() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::RiskParity);
        
        allocator.add_strategy("LowVol".to_string(), 50_000.0, 5.0, 50.0);
        allocator.add_strategy("HighVol".to_string(), 50_000.0, 5.0, 50.0);
        
        let mut stats = HashMap::new();
        stats.insert("LowVol".to_string(), StrategyStats {
            volatility: 0.10, // Low volatility
            ..Default::default()
        });
        stats.insert("HighVol".to_string(), StrategyStats {
            volatility: 0.40, // High volatility
            ..Default::default()
        });
        
        allocator.calculate_allocations(&stats);
        
        let low_vol = allocator.get_allocation("LowVol").unwrap();
        let high_vol = allocator.get_allocation("HighVol").unwrap();
        
        // Low vol should get more allocation
        assert!(low_vol.target_pct > high_vol.target_pct);
    }

    #[test]
    fn test_performance_weighted_allocation() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::PerformanceWeighted);
        
        allocator.add_strategy("Winner".to_string(), 50_000.0, 5.0, 60.0);
        allocator.add_strategy("Loser".to_string(), 50_000.0, 5.0, 60.0);
        
        let mut stats = HashMap::new();
        stats.insert("Winner".to_string(), StrategyStats {
            sharpe_ratio: 2.0,
            ..Default::default()
        });
        stats.insert("Loser".to_string(), StrategyStats {
            sharpe_ratio: 0.5,
            ..Default::default()
        });
        
        allocator.calculate_allocations(&stats);
        
        let winner = allocator.get_allocation("Winner").unwrap();
        let loser = allocator.get_allocation("Loser").unwrap();
        
        // Winner should get more allocation
        assert!(winner.target_pct > loser.target_pct);
    }

    #[test]
    fn test_allocation_constraints() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
        
        // Add strategy with tight constraints
        allocator.add_strategy("Constrained".to_string(), 20_000.0, 15.0, 25.0);
        allocator.add_strategy("Normal".to_string(), 80_000.0, 5.0, 80.0);
        
        let stats = HashMap::new();
        allocator.calculate_allocations(&stats);
        
        let constrained = allocator.get_allocation("Constrained").unwrap();
        let normal = allocator.get_allocation("Normal").unwrap();
        
        println!("Constrained: {:.2}%", constrained.target_pct);
        println!("Normal: {:.2}%", normal.target_pct);
        
        // Constrained should be capped at 25%
        assert!(constrained.target_pct <= 25.0);
        assert!(constrained.target_pct >= 15.0);
        
        // Normal should get the remainder (can't scale constrained beyond max)
        assert!(normal.target_pct >= 50.0);
    }

    #[test]
    fn test_rebalancing_trades() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
        
        allocator.add_strategy("Strategy1".to_string(), 60_000.0, 10.0, 60.0);
        allocator.add_strategy("Strategy2".to_string(), 60_000.0, 10.0, 60.0);
        
        let stats = HashMap::new();
        allocator.calculate_allocations(&stats);
        
        // Manually set current allocations (out of balance)
        allocator.allocations.get_mut("Strategy1").unwrap().current_pct = 30.0;
        allocator.allocations.get_mut("Strategy2").unwrap().current_pct = 70.0;
        
        let trades = allocator.get_rebalancing_trades();
        
        // Should suggest rebalancing trades
        assert!(!trades.is_empty());
    }

    #[test]
    fn test_capacity_check() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
        
        allocator.add_strategy("Limited".to_string(), 30_000.0, 10.0, 50.0);
        
        // Set current allocation to 25% = $25k
        allocator.allocations.get_mut("Limited").unwrap().current_pct = 25.0;
        
        // Should have capacity for $5k more
        assert!(allocator.has_capacity("Limited", 5_000.0));
        
        // Should NOT have capacity for $10k more
        assert!(!allocator.has_capacity("Limited", 10_000.0));
    }

    #[test]
    fn test_capacity_estimation() {
        let liquidity = 1_000_000.0;
        
        let iron_condor_cap = CapacityAnalyzer::estimate_capacity("IronCondor", liquidity);
        let straddle_cap = CapacityAnalyzer::estimate_capacity("Straddle", liquidity);
        
        // Iron condor should have higher capacity (less market impact)
        assert!(iron_condor_cap > straddle_cap);
    }

    #[test]
    fn test_normalization() {
        let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::Custom);
        
        allocator.add_strategy("A".to_string(), 50_000.0, 0.0, 100.0);
        allocator.add_strategy("B".to_string(), 50_000.0, 0.0, 100.0);
        allocator.add_strategy("C".to_string(), 50_000.0, 0.0, 100.0);
        
        // Set custom weights that don't sum to 100
        allocator.allocations.get_mut("A").unwrap().target_pct = 20.0;
        allocator.allocations.get_mut("B").unwrap().target_pct = 30.0;
        allocator.allocations.get_mut("C").unwrap().target_pct = 40.0;
        
        allocator.normalize_allocations();
        
        let total: f64 = allocator.allocations.values().map(|a| a.target_pct).sum();
        
        // Should sum to 100%
        assert!((total - 100.0).abs() < 0.01);
    }
}
