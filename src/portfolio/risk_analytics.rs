// Portfolio-level risk analytics

use crate::backtesting::position::{Position, PositionStatus, OptionType};
use crate::models::bs_mod::{Greeks, HigherOrderGreeks};
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
    // Second-order cross-Greeks
    pub total_vanna: f64,       // ∂Δ/∂σ — delta sensitivity to vol
    pub total_volga: f64,       // ∂²V/∂σ² — vega convexity
    pub total_charm: f64,       // −∂Δ/∂t — delta decay
    pub var_95: f64,            // 95% Value at Risk
    pub var_99: f64,            // 99% Value at Risk
    pub cvar_95: f64,           // 95% Conditional VaR (Expected Shortfall)
    pub cvar_99: f64,           // 99% Conditional VaR (Expected Shortfall)
    pub concentration_risk: f64, // Maximum single position as % of portfolio
    pub gross_delta_dollars: f64,
    pub gamma_loss_1pct: f64,
    pub vega_loss_1pct: f64,
    pub volga_loss_1pct: f64,
    pub charm_loss_1day: f64,
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
            total_vanna: 0.0,
            total_volga: 0.0,
            total_charm: 0.0,
            var_95: 0.0,
            var_99: 0.0,
            cvar_95: 0.0,
            cvar_99: 0.0,
            concentration_risk: 0.0,
            gross_delta_dollars: 0.0, gamma_loss_1pct: 0.0, vega_loss_1pct: 0.0,
            volga_loss_1pct: 0.0, charm_loss_1day: 0.0,
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
    pub max_portfolio_delta: f64,      // Gross delta dollars / account equity
    pub max_portfolio_gamma: f64,      // Gamma P&L on 1% spot move / equity
    pub max_portfolio_vega: f64,       // Vega P&L on 0.01 absolute IV move / equity
    pub max_portfolio_volga: f64,      // Volga P&L on 0.01 absolute IV move / equity
    pub max_portfolio_charm: f64,      // One-day delta drift on 1% spot move / equity
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
            max_portfolio_volga: 0.10,     // 10% volga
            max_portfolio_charm: 0.08,     // 8% charm
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
        let mut factors: HashMap<&str, [f64; 5]> = HashMap::new();
        
        for pos in positions.iter().filter(|p| matches!(p.status, PositionStatus::Open)) {
            let factor = factors.entry(&pos.symbol).or_insert([0.0; 5]);
            let valid = pos.entry_spot.is_finite() && pos.entry_spot > 0.0
                && pos.entry_greeks.as_ref().is_some_and(|g|
                    [g.price, g.delta, g.gamma, g.vega, g.theta].iter().all(|x| x.is_finite()));
            if !valid {
                return PortfolioRisk { var_95: f64::INFINITY, var_99: f64::INFINITY,
                    cvar_95: f64::INFINITY, cvar_99: f64::INFINITY, ..risk };
            }
            if let Some(greeks) = &pos.entry_greeks {
                let position_multiplier = pos.quantity as f64 * 100.0; // Options multiplier
                let delta_dollars = greeks.delta * position_multiplier * pos.entry_spot;
                risk.net_exposure += delta_dollars;
                factor[0] += delta_dollars;
                factor[1] += 0.5 * greeks.gamma * position_multiplier * (0.01 * pos.entry_spot).powi(2);
                factor[2] += greeks.vega * position_multiplier * 0.01;
                
                // Aggregate Greeks
                risk.total_delta += greeks.delta * position_multiplier;
                risk.total_gamma += greeks.gamma * position_multiplier;
                risk.total_theta += greeks.theta * position_multiplier;
                risk.total_vega += greeks.vega * position_multiplier;
            }

            // Aggregate higher-order Greeks if available
            if let Some(hog) = &pos.entry_higher_greeks {
                let position_multiplier = pos.quantity as f64 * 100.0;
                risk.total_vanna += hog.vanna * position_multiplier;
                risk.total_volga += hog.volga * position_multiplier;
                risk.total_charm += hog.charm * position_multiplier;
                factor[3] += 0.5 * hog.volga * position_multiplier * 0.01_f64.powi(2);
                factor[4] += hog.charm * position_multiplier * pos.entry_spot * 0.01 / 365.0;
            }

            if let Some(greeks) = &pos.entry_greeks {
                let position_multiplier = pos.quantity as f64 * 100.0;
                // Track position value
                let position_value = greeks.price * position_multiplier.abs();
                position_values.push(position_value);
                risk.gross_exposure += position_value;
            }
        }
        
        // Net exposure (accounting for long/short)
        for f in factors.values() {
            risk.gross_delta_dollars += f[0].abs();
            risk.gamma_loss_1pct += f[1].abs();
            risk.vega_loss_1pct += f[2].abs();
            risk.volga_loss_1pct += f[3].abs();
            risk.charm_loss_1day += f[4].abs();
        }
        
        // Concentration risk
        if !position_values.is_empty() && self.portfolio_value > 0.0 {
            risk.concentration_risk = position_values.iter()
                .map(|&v| (v / self.portfolio_value) * 100.0)
                .fold(0.0, f64::max);
        }
        
        // VaR calculation (simplified parametric VaR)
        risk.var_95 = self.calculate_var(positions, 1.645); // 95% confidence
        risk.var_99 = self.calculate_var(positions, 2.326); // 99% confidence

        // Conditional VaR (Expected Shortfall)
        // For normal distribution: CVaR_α = σ · φ(z_α) / (1 − α)
        // where φ is the standard normal PDF at quantile z_α.
        // This is always ≥ VaR_α, consistent with coherent risk measures.
        risk.cvar_95 = self.calculate_parametric_cvar(positions, 1.645, 0.05);
        risk.cvar_99 = self.calculate_parametric_cvar(positions, 2.326, 0.01);
        
        risk
    }

    /// Calculate Conditional Value at Risk (Expected Shortfall) using the parametric method.
    ///
    /// Under a normal distribution assumption, CVaR at confidence level `(1 - tail_prob)` is:
    ///
    ///   CVaR_α = σ · φ(z_α) / tail_prob
    ///
    /// where:
    ///   - σ is the portfolio volatility (same as used in VaR)
    ///   - z_α is the standard normal quantile corresponding to confidence level α
    ///   - φ(z_α) is the standard normal PDF evaluated at z_α
    ///   - tail_prob = 1 − α (e.g., 0.05 for 95% CVaR)
    ///
    /// CVaR is always ≥ VaR for the same confidence level and satisfies
    /// the coherent risk measure axioms (unlike VaR, which is not sub-additive).
    fn calculate_parametric_cvar(&self, positions: &[Position], z_score: f64, tail_prob: f64) -> f64 {
        let portfolio_vol = self.portfolio_vol_from_positions(positions);
        let frac_1_sqrt_2pi: f64 = 0.39894228040143267793994605993439;
        let pdf_at_z = frac_1_sqrt_2pi * (-0.5 * z_score * z_score).exp();
        portfolio_vol * pdf_at_z / tail_prob
    }

    /// Calculate Value at Risk (parametric method)
    fn calculate_var(&self, positions: &[Position], z_score: f64) -> f64 {
        // Simplified VaR: assumes normal distribution
        // VaR = Portfolio Value * Volatility * Z-score
        
        let portfolio_vol = self.portfolio_vol_from_positions(positions);
        portfolio_vol * z_score
    }

    /// Compute portfolio volatility proxy (shared between VaR and CVaR).
    fn portfolio_vol_from_positions(&self, positions: &[Position]) -> f64 {
        // One-day linear normal stress proxy, NOT a calibrated VaR forecast:
        // spot sigma=20%/sqrt(252), IV sigma=0.01. Net identical-underlying
        // exposures first; align adverse shocks across underlyings, with spot
        // and IV factors independent. Curvature is checked separately.
        let mut factors: HashMap<&str, (f64, f64)> = HashMap::new();
        for pos in positions.iter().filter(|p| matches!(p.status, PositionStatus::Open)) {
            if let Some(greeks) = &pos.entry_greeks {
                let f = factors.entry(&pos.symbol).or_default();
                f.0 += greeks.delta * pos.quantity as f64 * 100.0 * pos.entry_spot * 0.20 / 252.0_f64.sqrt();
                f.1 += greeks.vega * pos.quantity as f64 * 100.0 * 0.01;
            }
        }
        let spot: f64 = factors.values().map(|f| f.0.abs()).sum();
        let iv: f64 = factors.values().map(|f| f.1.abs()).sum();
        spot.hypot(iv)
    }

    /// Check if portfolio violates risk limits
    pub fn check_risk_limits(&self, risk: &PortfolioRisk) -> Vec<String> {
        let mut violations = Vec::new();
        if !self.portfolio_value.is_finite() || self.portfolio_value <= 0.0
            || ![risk.var_95, risk.gross_delta_dollars, risk.gamma_loss_1pct,
                risk.vega_loss_1pct, risk.volga_loss_1pct, risk.charm_loss_1day].iter().all(|v| v.is_finite()) {
            return vec!["Invalid or unmarked portfolio risk inputs".into()];
        }
        
        // Normalize Greeks relative to portfolio value
        let delta_pct = risk.gross_delta_dollars / self.portfolio_value;
        let gamma_pct = risk.gamma_loss_1pct / self.portfolio_value;
        let vega_pct = risk.vega_loss_1pct / self.portfolio_value;
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

        // Higher-order Greek limits
        let volga_pct = risk.volga_loss_1pct / self.portfolio_value;
        if volga_pct > self.risk_limits.max_portfolio_volga {
            violations.push(format!(
                "Portfolio volga {:.1}% exceeds limit {:.1}%",
                volga_pct * 100.0,
                self.risk_limits.max_portfolio_volga * 100.0
            ));
        }

        let charm_pct = risk.charm_loss_1day / self.portfolio_value;
        if charm_pct > self.risk_limits.max_portfolio_charm {
            violations.push(format!(
                "Portfolio charm {:.1}% exceeds limit {:.1}%",
                charm_pct * 100.0,
                self.risk_limits.max_portfolio_charm * 100.0
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
    use crate::models::american::ExerciseStyle;

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
            exercise_style: ExerciseStyle::European,
            strike: 100.0,
            quantity,
            entry_price: price,
            entry_date: "2024-01-01".to_string(),
            entry_spot: 100.0,
            expiry_date: None,
            exit_price: None,
            exit_date: None,
            exit_spot: None,
            status: PositionStatus::Open,
            days_held: 0,
            entry_greeks: Some(greeks),
            entry_higher_greeks: None,
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
