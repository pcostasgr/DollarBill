// Portfolio manager - orchestrates position sizing, risk analytics, allocation, and performance tracking

use crate::backtesting::position::Position;
use crate::portfolio::position_sizing::{PositionSizer, MultiLegSizer, SizingMethod};
use crate::portfolio::risk_analytics::{RiskAnalyzer, PortfolioRisk, RiskLimits};
use crate::portfolio::allocation::{PortfolioAllocator, AllocationMethod, StrategyStats};
use crate::portfolio::performance::PerformanceAttribution;
use std::collections::HashMap;

/// Portfolio manager configuration
#[derive(Debug, Clone)]
pub struct PortfolioConfig {
    pub initial_capital: f64,
    pub max_risk_per_trade: f64,
    pub max_position_pct: f64,
    pub sizing_method: SizingMethod,
    pub allocation_method: AllocationMethod,
    pub risk_limits: RiskLimits,
}

impl Default for PortfolioConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            max_risk_per_trade: 2.0,      // 2% risk per trade
            max_position_pct: 10.0,       // 10% max position size
            sizing_method: SizingMethod::FixedFractional(5.0),
            allocation_method: AllocationMethod::RiskParity,
            risk_limits: RiskLimits::default(),
        }
    }
}

/// Portfolio manager decision
#[derive(Debug, Clone)]
pub struct PortfolioDecision {
    pub can_trade: bool,
    pub suggested_size: i32,
    pub risk_warnings: Vec<String>,
    pub allocation_info: Option<String>,
}

/// Main portfolio manager
pub struct PortfolioManager {
    config: PortfolioConfig,
    position_sizer: PositionSizer,
    multi_leg_sizer: MultiLegSizer,
    risk_analyzer: RiskAnalyzer,
    allocator: PortfolioAllocator,
    performance: PerformanceAttribution,
    current_positions: Vec<Position>,
    current_capital: f64,
}

impl PortfolioManager {
    pub fn new(config: PortfolioConfig) -> Self {
        let position_sizer = PositionSizer::new(
            config.initial_capital,
            config.max_risk_per_trade,
            config.max_position_pct,
        );

        let multi_leg_sizer = MultiLegSizer::new(
            config.initial_capital,
            config.max_risk_per_trade,
            config.max_position_pct,
        );

        let risk_analyzer = RiskAnalyzer::new(
            config.initial_capital,
            config.risk_limits.clone(),
        );

        let allocator = PortfolioAllocator::new(
            config.initial_capital,
            config.allocation_method,
        );

        let performance = PerformanceAttribution::new();

        let initial_capital = config.initial_capital;
        
        Self {
            config,
            position_sizer,
            multi_leg_sizer,
            risk_analyzer,
            allocator,
            performance,
            current_positions: Vec::new(),
            current_capital: initial_capital,
        }
    }

    /// Update current positions
    pub fn update_positions(&mut self, positions: Vec<Position>) {
        self.current_positions = positions;
    }

    /// Update current capital
    pub fn update_capital(&mut self, capital: f64) {
        self.current_capital = capital;
        self.position_sizer.update_account(capital);
        self.multi_leg_sizer.update_account(capital);
        self.risk_analyzer.update_portfolio_value(capital);
        self.allocator.update_capital(capital);
    }

    /// Check if we can take a new position
    pub fn can_take_position(
        &self,
        strategy: &str,
        option_price: f64,
        volatility: f64,
        contracts: i32,
    ) -> PortfolioDecision {
        let mut warnings = Vec::new();

        // Check portfolio risk limits
        let current_risk = self.risk_analyzer.calculate_portfolio_greeks(&self.current_positions);
        let risk_violations = self.risk_analyzer.check_risk_limits(&current_risk);
        
        if !risk_violations.is_empty() {
            warnings.extend(risk_violations);
        }

        // Check allocation capacity
        let position_value = (contracts as f64) * option_price * 100.0;
        let has_capacity = self.allocator.has_capacity(strategy, position_value);
        
        if !has_capacity {
            warnings.push(format!("Strategy {} at capacity", strategy));
        }

        // Validate position size
        let is_valid = self.position_sizer.validate_position(contracts, option_price);
        
        if !is_valid {
            warnings.push("Position size exceeds limits".to_string());
        }

        // Calculate suggested size
        let suggested_size = self.position_sizer.calculate_size(
            self.config.sizing_method,
            option_price,
            volatility,
            None,
            None,
            None,
        );

        PortfolioDecision {
            can_trade: warnings.is_empty() && has_capacity && is_valid,
            suggested_size,
            risk_warnings: warnings,
            allocation_info: self.allocator.get_allocation(strategy)
                .map(|a| format!("Current: {:.1}%, Target: {:.1}%", a.current_pct, a.target_pct)),
        }
    }

    /// Calculate position size for a simple option trade
    pub fn calculate_position_size(
        &self,
        option_price: f64,
        volatility: f64,
        win_rate: Option<f64>,
        avg_win: Option<f64>,
        avg_loss: Option<f64>,
    ) -> i32 {
        self.position_sizer.calculate_size(
            self.config.sizing_method,
            option_price,
            volatility,
            win_rate,
            avg_win,
            avg_loss,
        )
    }

    /// Calculate position size for iron condor
    pub fn calculate_iron_condor_size(
        &self,
        max_loss: f64,
        net_credit: f64,
        volatility: f64,
    ) -> i32 {
        self.multi_leg_sizer.iron_condor_size(
            self.config.sizing_method,
            max_loss,
            net_credit,
            volatility,
        )
    }

    /// Calculate position size for credit spread
    pub fn calculate_credit_spread_size(
        &self,
        spread_width: f64,
        net_credit: f64,
        volatility: f64,
    ) -> i32 {
        self.multi_leg_sizer.credit_spread_size(
            self.config.sizing_method,
            spread_width,
            net_credit,
            volatility,
        )
    }

    /// Get current portfolio risk metrics
    pub fn get_portfolio_risk(&self) -> PortfolioRisk {
        self.risk_analyzer.calculate_portfolio_greeks(&self.current_positions)
    }

    /// Add strategy to allocation mix
    pub fn add_strategy(
        &mut self,
        name: String,
        capacity: f64,
        min_pct: f64,
        max_pct: f64,
    ) {
        self.allocator.add_strategy(name, capacity, min_pct, max_pct);
    }

    /// Calculate strategy allocations
    pub fn optimize_allocations(&mut self, strategy_stats: &HashMap<String, StrategyStats>) {
        self.allocator.calculate_allocations(strategy_stats);
    }

    /// Get rebalancing trades
    pub fn get_rebalancing_recommendations(&self) -> Vec<(String, f64, String)> {
        self.allocator.get_rebalancing_trades()
            .into_iter()
            .map(|t| (t.strategy, t.dollar_change, t.action))
            .collect()
    }

    /// Calculate performance for a strategy
    pub fn calculate_strategy_performance(&mut self, strategy: &str, positions: &[Position]) {
        self.performance.calculate_strategy_performance(strategy, positions);
    }

    /// Get best performing strategy
    pub fn best_strategy(&self) -> Option<String> {
        self.performance.best_strategy()
    }

    /// Print portfolio summary
    pub fn print_summary(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║             PORTFOLIO MANAGEMENT SUMMARY                  ║");
        println!("╚═══════════════════════════════════════════════════════════╝");
        
        println!("\n📊 Account Information:");
        println!("   Current Capital:     ${:.2}", self.current_capital);
        println!("   Open Positions:      {}", self.current_positions.len());
        
        let risk = self.get_portfolio_risk();
        
        println!("\n⚠️  Risk Metrics:");
        println!("   Total Delta:         {:.2}", risk.total_delta);
        println!("   Total Gamma:         {:.2}", risk.total_gamma);
        println!("   Total Theta:         {:.2}", risk.total_theta);
        println!("   Total Vega:          {:.2}", risk.total_vega);
        println!("   Net Exposure:        ${:.2}", risk.net_exposure);
        println!("   Gross Exposure:      ${:.2}", risk.gross_exposure);
        println!("   VaR (95%):           ${:.2}", risk.var_95);
        println!("   VaR (99%):           ${:.2}", risk.var_99);
        println!("   Concentration Risk:  {:.1}%", risk.concentration_risk);
        
        let violations = self.risk_analyzer.check_risk_limits(&risk);
        if !violations.is_empty() {
            println!("\n🚨 Risk Limit Violations:");
            for violation in violations {
                println!("   ⚠️  {}", violation);
            }
        } else {
            println!("\n✅ All risk limits satisfied");
        }
        
        println!("\n💰 Strategy Allocations:");
        for (name, allocation) in self.allocator.get_all_allocations().iter() {
            println!("   {}: {:.1}% (target: {:.1}%)", 
                name, allocation.current_pct, allocation.target_pct);
        }
        
        println!();
    }

    /// Print detailed performance report
    pub fn print_performance_report(&self, strategy: &str) {
        self.performance.print_summary(strategy);
    }

    /// Get configuration
    pub fn config(&self) -> &PortfolioConfig {
        &self.config
    }

    /// Get current capital
    pub fn capital(&self) -> f64 {
        self.current_capital
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtesting::position::{Position, PositionStatus, OptionType};
    use crate::models::bs_mod::Greeks;

    fn create_test_position(id: usize, quantity: i32, price: f64) -> Position {
        Position {
            id,
            symbol: "TEST".to_string(),
            option_type: OptionType::Call,
            strike: 100.0,
            quantity,
            entry_price: price,
            entry_date: "2024-01-01".to_string(),
            entry_spot: 100.0,
            exit_price: None,
            exit_date: None,
            exit_spot: None,
            status: PositionStatus::Open,
            days_held: 0,
            entry_greeks: Some(Greeks {
                price,
                delta: 0.5,
                gamma: 0.05,
                theta: -0.02,
                vega: 0.15,
                rho: 0.1,
            }),
            realized_pnl: 0.0,
            unrealized_pnl: 0.0,
        }
    }

    #[test]
    fn test_portfolio_manager_creation() {
        let config = PortfolioConfig::default();
        let manager = PortfolioManager::new(config);
        
        assert_eq!(manager.capital(), 100_000.0);
    }

    #[test]
    fn test_capital_update() {
        let config = PortfolioConfig::default();
        let mut manager = PortfolioManager::new(config);
        
        manager.update_capital(150_000.0);
        assert_eq!(manager.capital(), 150_000.0);
    }

    #[test]
    fn test_position_size_calculation() {
        let config = PortfolioConfig::default();
        let manager = PortfolioManager::new(config);
        
        let size = manager.calculate_position_size(2.50, 0.30, None, None, None);
        assert!(size > 0);
    }

    #[test]
    fn test_iron_condor_sizing() {
        let config = PortfolioConfig::default();
        let manager = PortfolioManager::new(config);
        
        let size = manager.calculate_iron_condor_size(500.0, 1.50, 0.30);
        assert!(size > 0);
    }

    #[test]
    fn test_credit_spread_sizing() {
        let config = PortfolioConfig::default();
        let manager = PortfolioManager::new(config);
        
        let size = manager.calculate_credit_spread_size(5.0, 1.50, 0.25);
        assert!(size > 0);
    }

    #[test]
    fn test_can_take_position() {
        let config = PortfolioConfig::default();
        let mut manager = PortfolioManager::new(config);
        
        manager.add_strategy("TestStrategy".to_string(), 50_000.0, 10.0, 40.0);
        
        let decision = manager.can_take_position("TestStrategy", 2.50, 0.30, 20);
        assert!(decision.suggested_size > 0);
    }

    #[test]
    fn test_portfolio_risk_calculation() {
        let config = PortfolioConfig::default();
        let mut manager = PortfolioManager::new(config);
        
        let positions = vec![
            create_test_position(1, 10, 2.50),
            create_test_position(2, -5, 1.50),
        ];
        
        manager.update_positions(positions);
        let risk = manager.get_portfolio_risk();
        
        assert!(risk.total_delta != 0.0);
    }

    #[test]
    fn test_strategy_allocation() {
        let config = PortfolioConfig {
            allocation_method: AllocationMethod::EqualWeight,
            ..Default::default()
        };
        let mut manager = PortfolioManager::new(config);
        
        manager.add_strategy("Strategy1".to_string(), 50_000.0, 10.0, 50.0);
        manager.add_strategy("Strategy2".to_string(), 50_000.0, 10.0, 50.0);
        
        let stats = HashMap::new();
        manager.optimize_allocations(&stats);
        
        // Both strategies should get ~50% allocation
        let allocation1 = manager.allocator.get_allocation("Strategy1").unwrap();
        assert!((allocation1.target_pct - 50.0).abs() < 5.0);
    }

    #[test]
    fn test_risk_limit_enforcement() {
        let config = PortfolioConfig {
            risk_limits: RiskLimits {
                max_concentration_pct: 15.0, // Strict limit
                ..Default::default()
            },
            ..Default::default()
        };
        let mut manager = PortfolioManager::new(config);
        
        // Large position that would exceed concentration
        let positions = vec![
            create_test_position(1, 100, 5.0), // $50k position in $100k portfolio
        ];
        
        manager.update_positions(positions);
        let risk = manager.get_portfolio_risk();
        let violations = manager.risk_analyzer.check_risk_limits(&risk);
        
        // Should have concentration violation
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_performance_tracking() {
        let config = PortfolioConfig::default();
        let mut manager = PortfolioManager::new(config);
        
        let mut positions = vec![create_test_position(1, 10, 2.50)];
        positions[0].close(3.00, "2024-01-10".to_string(), 105.0, 10);
        positions[0].realized_pnl = 50.0; // Override
        
        manager.calculate_strategy_performance("TestStrategy", &positions);
        
        // Performance should be tracked
        assert!(manager.best_strategy().is_some());
    }
}
