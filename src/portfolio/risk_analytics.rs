// Portfolio-level risk analytics

use crate::backtesting::position::{Position, PositionStatus, OptionType};
use crate::models::bs_mod::Greeks;
use std::collections::HashMap;

/// Portfolio risk metrics
#[derive(Debug, Clone)]
pub struct PortfolioRisk {
    pub total_delta: f64,
    pub total_gamma: f64,
    pub total_theta: f64,
    pub total_vega: f64,
    pub net_exposure: f64,      // Net delta exposure in dollars
    pub gross_exposure: f64,    // Gross exposure (sum of absolute values)
    pub beta_weighted_delta: f64,
    pub var_95: f64,            // 95% Value at Risk
    pub var_99: f64,            // 99% Value at Risk
    pub concentration_risk: f64, // Maximum single position as % of portfolio
}

impl Default for PortfolioRisk {
    fn default() -> Self {
        Self {
            total_delta: 0.0,
            total_gamma: 0.0,
            total_theta: 0.0,
            total_vega: 0.0,
            net_exposure: 0.0,
            gross_exposure: 0.0,
            beta_weighted_delta: 0.0,
            var_95: 0.0,
            var_99: 0.0,
            concentration_risk: 0.0,
        }
    }
}

/// Portfolio risk analyzer
pub struct RiskAnalyzer {
    portfolio_value: f64,
    risk_limits: RiskLimits,
}

/// Risk limits and constraints
#[derive(Debug, Clone)]
pub struct RiskLimits {
    pub max_portfolio_delta: f64,      // Maximum net delta
    pub max_portfolio_gamma: f64,      // Maximum gamma exposure
    pub max_portfolio_vega: f64,       // Maximum vega exposure
    pub max_concentration_pct: f64,    // Max % in single position
    pub max_var_pct: f64,              // Max VaR as % of portfolio
    pub max_sector_exposure_pct: f64,  // Max exposure to single sector
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_portfolio_delta: 0.3,     // 30% net delta
            max_portfolio_gamma: 0.1,      // 10% gamma
            max_portfolio_vega: 0.15,      // 15% vega
            max_concentration_pct: 20.0,   // 20% max per position
            max_var_pct: 10.0,             // 10% max VaR
            max_sector_exposure_pct: 40.0, // 40% per sector
        }
    }
}

impl RiskAnalyzer {
    pub fn new(portfolio_value: f64, risk_limits: RiskLimits) -> Self {
        Self {
            portfolio_value,
            risk_limits,
        }
    }

    /// Calculate aggregated portfolio Greeks
    pub fn calculate_portfolio_greeks(&self, positions: &[Position]) -> PortfolioRisk {
        let mut risk = PortfolioRisk::default();
        
        let mut position_values = Vec::new();
        
        for pos in positions.iter().filter(|p| matches!(p.status, PositionStatus::Open)) {
            if let Some(greeks) = &pos.entry_greeks {
                let position_multiplier = pos.quantity as f64 * 100.0; // Options multiplier
                
                // Aggregate Greeks
                risk.total_delta += greeks.delta * position_multiplier;
                risk.total_gamma += greeks.gamma * position_multiplier;
                risk.total_theta += greeks.theta * position_multiplier;
                risk.total_vega += greeks.vega * position_multiplier;
                
                // Track position value
                let position_value = greeks.price * position_multiplier.abs();
                position_values.push(position_value);
                risk.gross_exposure += position_value;
            }
        }
        
        // Net exposure (accounting for long/short)
        risk.net_exposure = risk.total_delta * 100.0; // Delta in dollars
        
        // Concentration risk
        if !position_values.is_empty() && self.portfolio_value > 0.0 {
            risk.concentration_risk = position_values.iter()
                .map(|&v| (v / self.portfolio_value) * 100.0)
                .fold(0.0, f64::max);
        }
        
        // VaR calculation (simplified parametric VaR)
        risk.var_95 = self.calculate_var(positions, 1.645); // 95% confidence
        risk.var_99 = self.calculate_var(positions, 2.326); // 99% confidence
        
        risk
    }

    /// Calculate Value at Risk (parametric method)
    fn calculate_var(&self, positions: &[Position], z_score: f64) -> f64 {
        // Simplified VaR: assumes normal distribution
        // VaR = Portfolio Value * Volatility * Z-score
        
        let mut total_variance = 0.0;
        
        for pos in positions.iter().filter(|p| matches!(p.status, PositionStatus::Open)) {
            if let Some(greeks) = &pos.entry_greeks {
                // Use vega as proxy for volatility sensitivity
                let position_value = greeks.price * (pos.quantity as f64).abs() * 100.0;
                let vol_contribution = greeks.vega * position_value / 100.0; // Normalize
                total_variance += vol_contribution.powi(2);
            }
        }
        
        let portfolio_vol = total_variance.sqrt();
        portfolio_vol * z_score
    }

    /// Check if portfolio violates risk limits
    pub fn check_risk_limits(&self, risk: &PortfolioRisk) -> Vec<String> {
        let mut violations = Vec::new();
        
        // Normalize Greeks relative to portfolio value
        let delta_pct = (risk.total_delta.abs() / self.portfolio_value).min(1.0);
        let gamma_pct = (risk.total_gamma.abs() / self.portfolio_value).min(1.0);
        let vega_pct = (risk.total_vega.abs() / self.portfolio_value).min(1.0);
        let var_pct = (risk.var_95 / self.portfolio_value) * 100.0;
        
        if delta_pct > self.risk_limits.max_portfolio_delta {
            violations.push(format!(
                "Portfolio delta {:.1}% exceeds limit {:.1}%",
                delta_pct * 100.0,
                self.risk_limits.max_portfolio_delta * 100.0
            ));
        }
        
        if gamma_pct > self.risk_limits.max_portfolio_gamma {
            violations.push(format!(
                "Portfolio gamma {:.1}% exceeds limit {:.1}%",
                gamma_pct * 100.0,
                self.risk_limits.max_portfolio_gamma * 100.0
            ));
        }
        
        if vega_pct > self.risk_limits.max_portfolio_vega {
            violations.push(format!(
                "Portfolio vega {:.1}% exceeds limit {:.1}%",
                vega_pct * 100.0,
                self.risk_limits.max_portfolio_vega * 100.0
            ));
        }
        
        if risk.concentration_risk > self.risk_limits.max_concentration_pct {
            violations.push(format!(
                "Concentration {:.1}% exceeds limit {:.1}%",
                risk.concentration_risk,
                self.risk_limits.max_concentration_pct
            ));
        }
        
        if var_pct > self.risk_limits.max_var_pct {
            violations.push(format!(
                "VaR {:.1}% exceeds limit {:.1}%",
                var_pct,
                self.risk_limits.max_var_pct
            ));
        }
        
        violations
    }

    /// Calculate correlation between positions (simplified)
    pub fn calculate_correlation(&self, symbols: &[String]) -> HashMap<String, f64> {
        // Simplified: assume sector-based correlation
        // In production, use historical price correlation
        
        let mut correlations = HashMap::new();
        
        // Group symbols by sector (simplified heuristic)
        for symbol in symbols {
            let correlation = match symbol.as_str() {
                "SPY" | "QQQ" | "IWM" => 0.85, // High market correlation
                "TSLA" | "NVDA" | "AMD" => 0.70, // Tech sector
                "AAPL" | "MSFT" | "GOOGL" | "META" => 0.75, // Large cap tech
                "GLD" | "TLT" => 0.30, // Low correlation assets
                _ => 0.50, // Default moderate correlation
            };
            correlations.insert(symbol.clone(), correlation);
        }
        
        correlations
    }

    /// Diversification score (0-100, higher is better)
    pub fn diversification_score(&self, positions: &[Position]) -> f64 {
        if positions.is_empty() {
            return 0.0;
        }
        
        // Collect unique symbols
        let mut symbols = std::collections::HashSet::new();
        for pos in positions.iter().filter(|p| matches!(p.status, PositionStatus::Open)) {
            symbols.insert(pos.symbol.clone());
        }
        
        let unique_count = symbols.len();
        
        if unique_count == 0 {
            return 0.0;
        }
        
        // Base score from number of positions (logarithmic)
        let diversity_score = ((unique_count as f64).ln() / 3.0_f64.ln()).min(1.0) * 50.0;
        
        // Calculate concentration risk
        let risk = self.calculate_portfolio_greeks(positions);
        let concentration_penalty = (risk.concentration_risk / 100.0) * 50.0;
        
        (diversity_score + 50.0 - concentration_penalty).max(0.0).min(100.0)
    }

    /// Update portfolio value
    pub fn update_portfolio_value(&mut self, new_value: f64) {
        self.portfolio_value = new_value;
    }
}

/// Correlation matrix for multi-asset portfolios
pub struct CorrelationMatrix {
    symbols: Vec<String>,
    matrix: Vec<Vec<f64>>,
}

impl CorrelationMatrix {
    pub fn new(symbols: Vec<String>) -> Self {
        let n = symbols.len();
        let matrix = vec![vec![0.5; n]; n]; // Default 0.5 correlation
        
        // Set diagonal to 1.0 (perfect self-correlation)
        let mut matrix_mut = matrix;
        for i in 0..n {
            matrix_mut[i][i] = 1.0;
        }
        
        Self {
            symbols,
            matrix: matrix_mut,
        }
    }

    pub fn get_correlation(&self, symbol1: &str, symbol2: &str) -> f64 {
        let idx1 = self.symbols.iter().position(|s| s == symbol1);
        let idx2 = self.symbols.iter().position(|s| s == symbol2);
        
        match (idx1, idx2) {
            (Some(i), Some(j)) => self.matrix[i][j],
            _ => 0.5, // Default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_position_with_greeks(
        id: usize,
        symbol: &str,
        quantity: i32,
        price: f64,
        greeks: Greeks,
    ) -> Position {
        Position {
            id,
            symbol: symbol.to_string(),
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
            entry_greeks: Some(greeks),
            realized_pnl: 0.0,
            unrealized_pnl: 0.0,
        }
    }

    #[test]
    fn test_portfolio_greeks_calculation() {
        let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
        
        let greeks1 = Greeks {
            price: 2.50,
            delta: 0.5,
            gamma: 0.05,
            theta: -0.02,
            vega: 0.15,
            rho: 0.1,
        };
        
        let greeks2 = Greeks {
            price: 1.50,
            delta: -0.3,
            gamma: 0.03,
            theta: -0.01,
            vega: 0.10,
            rho: 0.05,
        };
        
        let positions = vec![
            create_test_position_with_greeks(1, "AAPL", 10, 2.50, greeks1),
            create_test_position_with_greeks(2, "TSLA", -5, 1.50, greeks2),
        ];
        
        let risk = analyzer.calculate_portfolio_greeks(&positions);
        
        // Delta: (0.5 * 10 * 100) + (-0.3 * -5 * 100) = 500 + 150 = 650
        assert!((risk.total_delta - 650.0).abs() < 1.0);
        
        // Check that other Greeks are calculated
        assert!(risk.total_gamma > 0.0);
        assert!(risk.total_theta < 0.0);
        assert!(risk.total_vega > 0.0);
    }

    #[test]
    fn test_concentration_risk() {
        let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
        
        let greeks = Greeks {
            price: 5.0,
            delta: 0.5,
            gamma: 0.05,
            theta: -0.02,
            vega: 0.15,
            rho: 0.1,
        };
        
        // Large position: 40 contracts @ $5 = $20k (20% of portfolio)
        let positions = vec![
            create_test_position_with_greeks(1, "AAPL", 40, 5.0, greeks),
        ];
        
        let risk = analyzer.calculate_portfolio_greeks(&positions);
        
        // Concentration should be ~20%
        assert!(risk.concentration_risk >= 19.0 && risk.concentration_risk <= 21.0);
    }

    #[test]
    fn test_risk_limit_violations() {
        let limits = RiskLimits {
            max_portfolio_delta: 0.1,  // 10% max delta
            max_concentration_pct: 15.0,
            ..Default::default()
        };
        
        let analyzer = RiskAnalyzer::new(100_000.0, limits);
        
        let greeks = Greeks {
            price: 5.0,
            delta: 0.8,  // High delta
            gamma: 0.05,
            theta: -0.02,
            vega: 0.15,
            rho: 0.1,
        };
        
        // 50 contracts with high delta
        let positions = vec![
            create_test_position_with_greeks(1, "TSLA", 50, 5.0, greeks),
        ];
        
        let risk = analyzer.calculate_portfolio_greeks(&positions);
        let violations = analyzer.check_risk_limits(&risk);
        
        // Should have at least one violation (delta or concentration)
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_diversification_score() {
        let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
        
        let greeks = Greeks {
            price: 2.0,
            delta: 0.5,
            gamma: 0.05,
            theta: -0.02,
            vega: 0.15,
            rho: 0.1,
        };
        
        // Single position - low diversity
        let single_pos = vec![
            create_test_position_with_greeks(1, "AAPL", 10, 2.0, greeks.clone()),
        ];
        
        // Multiple positions - higher diversity
        let multi_pos = vec![
            create_test_position_with_greeks(1, "AAPL", 10, 2.0, greeks.clone()),
            create_test_position_with_greeks(2, "TSLA", 10, 2.0, greeks.clone()),
            create_test_position_with_greeks(3, "MSFT", 10, 2.0, greeks.clone()),
        ];
        
        let single_score = analyzer.diversification_score(&single_pos);
        let multi_score = analyzer.diversification_score(&multi_pos);
        
        assert!(multi_score > single_score);
    }

    #[test]
    fn test_var_calculation() {
        let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
        
        let greeks = Greeks {
            price: 3.0,
            delta: 0.6,
            gamma: 0.05,
            theta: -0.02,
            vega: 0.20,  // 20% vega
            rho: 0.1,
        };
        
        let positions = vec![
            create_test_position_with_greeks(1, "AAPL", 20, 3.0, greeks),
        ];
        
        let risk = analyzer.calculate_portfolio_greeks(&positions);
        
        // VaR should be positive and 99% > 95%
        assert!(risk.var_95 > 0.0);
        assert!(risk.var_99 > risk.var_95);
    }

    #[test]
    fn test_correlation_matrix() {
        let symbols = vec!["AAPL".to_string(), "TSLA".to_string(), "GLD".to_string()];
        let matrix = CorrelationMatrix::new(symbols);
        
        // Self-correlation should be 1.0
        assert_eq!(matrix.get_correlation("AAPL", "AAPL"), 1.0);
        
        // Cross-correlation should be default (0.5)
        let corr = matrix.get_correlation("AAPL", "TSLA");
        assert!(corr > 0.0 && corr <= 1.0);
    }
}
