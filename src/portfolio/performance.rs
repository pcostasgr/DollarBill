#![allow(dead_code)]
// Performance attribution and tracking

use crate::backtesting::position::{Position, PositionStatus};
use std::collections::HashMap;

/// Performance metrics for a strategy
#[derive(Debug, Clone)]
pub struct StrategyPerformance {
    pub strategy_name: String,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub gross_profit: f64,
    pub gross_loss: f64,
    pub net_profit: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    pub avg_return_pct: f64,
    pub roi: f64,
}

impl Default for StrategyPerformance {
    fn default() -> Self {
        Self {
            strategy_name: String::new(),
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            gross_profit: 0.0,
            gross_loss: 0.0,
            net_profit: 0.0,
            win_rate: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            profit_factor: 0.0,
            sharpe_ratio: 0.0,
            sortino_ratio: 0.0,
            max_drawdown: 0.0,
            max_drawdown_pct: 0.0,
            avg_return_pct: 0.0,
            roi: 0.0,
        }
    }
}

/// Performance attribution analyzer
pub struct PerformanceAttribution {
    strategy_performance: HashMap<String, StrategyPerformance>,
    equity_curves: HashMap<String, Vec<f64>>,
}

impl PerformanceAttribution {
    pub fn new() -> Self {
        Self {
            strategy_performance: HashMap::new(),
            equity_curves: HashMap::new(),
        }
    }

    /// Calculate performance for a specific strategy
    pub fn calculate_strategy_performance(
        &mut self,
        strategy_name: &str,
        positions: &[Position],
    ) -> StrategyPerformance {
        let mut perf = StrategyPerformance {
            strategy_name: strategy_name.to_string(),
            ..Default::default()
        };

        let mut returns = Vec::new();
        let mut equity_curve = vec![0.0];
        let mut cumulative_pnl = 0.0;

        for pos in positions.iter().filter(|p| matches!(p.status, PositionStatus::Closed | PositionStatus::Expired)) {
            perf.total_trades += 1;

            if pos.is_winner() {
                perf.winning_trades += 1;
                perf.gross_profit += pos.realized_pnl;
            } else {
                perf.losing_trades += 1;
                perf.gross_loss += pos.realized_pnl.abs();
            }

            // Track returns
            let cost_basis = pos.entry_price * (pos.quantity.abs() as f64) * 100.0;
            if cost_basis > 0.0 {
                let return_pct = (pos.realized_pnl / cost_basis) * 100.0;
                returns.push(return_pct);
            }

            // Update equity curve
            cumulative_pnl += pos.realized_pnl;
            equity_curve.push(cumulative_pnl);
        }

        // Calculate aggregate metrics
        perf.net_profit = perf.gross_profit - perf.gross_loss;
        
        if perf.total_trades > 0 {
            perf.win_rate = (perf.winning_trades as f64 / perf.total_trades as f64) * 100.0;
        }

        if perf.winning_trades > 0 {
            perf.avg_win = perf.gross_profit / perf.winning_trades as f64;
        }

        if perf.losing_trades > 0 {
            perf.avg_loss = perf.gross_loss / perf.losing_trades as f64;
        }

        if perf.gross_loss > 0.0 {
            perf.profit_factor = perf.gross_profit / perf.gross_loss;
        }

        // Calculate risk-adjusted returns
        if !returns.is_empty() {
            perf.avg_return_pct = returns.iter().sum::<f64>() / returns.len() as f64;
            perf.sharpe_ratio = self.calculate_sharpe_ratio(&returns);
            perf.sortino_ratio = self.calculate_sortino_ratio(&returns);
        }

        // Calculate drawdown
        let (max_dd, max_dd_pct) = self.calculate_drawdown(&equity_curve);
        perf.max_drawdown = max_dd;
        perf.max_drawdown_pct = max_dd_pct;

        // Calculate total ROI (assuming starting capital)
        if !equity_curve.is_empty() {
            let initial_capital = 100_000.0; // Assumption
            perf.roi = (perf.net_profit / initial_capital) * 100.0;
        }

        // Store for later retrieval
        self.strategy_performance.insert(strategy_name.to_string(), perf.clone());
        self.equity_curves.insert(strategy_name.to_string(), equity_curve);

        perf
    }

    /// Calculate Sharpe ratio
    fn calculate_sharpe_ratio(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        
        // Calculate standard deviation
        let variance: f64 = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev > 0.0 {
            // Assuming 5% risk-free rate annualized, daily ~0.014%
            let risk_free_rate = 0.014;
            (mean_return - risk_free_rate) / std_dev
        } else {
            0.0
        }
    }

    /// Calculate Sortino ratio (downside deviation only)
    fn calculate_sortino_ratio(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        
        // Calculate downside deviation (only negative returns)
        let downside_returns: Vec<f64> = returns.iter()
            .filter(|&&r| r < 0.0)
            .copied()
            .collect();

        if downside_returns.is_empty() {
            return 0.0;
        }

        let downside_variance: f64 = downside_returns.iter()
            .map(|r| r.powi(2))
            .sum::<f64>() / downside_returns.len() as f64;
        let downside_dev = downside_variance.sqrt();

        if downside_dev > 0.0 {
            let risk_free_rate = 0.014;
            (mean_return - risk_free_rate) / downside_dev
        } else {
            0.0
        }
    }

    /// Calculate maximum drawdown
    fn calculate_drawdown(&self, equity_curve: &[f64]) -> (f64, f64) {
        if equity_curve.len() < 2 {
            return (0.0, 0.0);
        }

        let mut max_value = equity_curve[0];
        let mut max_drawdown = 0.0;
        let mut max_drawdown_pct = 0.0;

        for &value in equity_curve.iter() {
            if value > max_value {
                max_value = value;
            }

            let drawdown = max_value - value;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
                if max_value != 0.0 {
                    max_drawdown_pct = (drawdown / max_value.abs()) * 100.0;
                }
            }
        }

        (max_drawdown, max_drawdown_pct)
    }

    /// Compare strategies side by side
    pub fn compare_strategies(&self, strategy_names: &[&str]) -> Vec<StrategyComparison> {
        let mut comparisons = Vec::new();

        for &name in strategy_names {
            if let Some(perf) = self.strategy_performance.get(name) {
                comparisons.push(StrategyComparison {
                    strategy: name.to_string(),
                    win_rate: perf.win_rate,
                    profit_factor: perf.profit_factor,
                    sharpe_ratio: perf.sharpe_ratio,
                    max_drawdown_pct: perf.max_drawdown_pct,
                    net_profit: perf.net_profit,
                    roi: perf.roi,
                });
            }
        }

        // Sort by Sharpe ratio (best first)
        comparisons.sort_by(|a, b| b.sharpe_ratio.partial_cmp(&a.sharpe_ratio).unwrap());

        comparisons
    }

    /// Get best performing strategy
    pub fn best_strategy(&self) -> Option<String> {
        self.strategy_performance.iter()
            .max_by(|(_, a), (_, b)| a.sharpe_ratio.partial_cmp(&b.sharpe_ratio).unwrap())
            .map(|(name, _)| name.clone())
    }

    /// Get equity curve for a strategy
    pub fn get_equity_curve(&self, strategy: &str) -> Option<&Vec<f64>> {
        self.equity_curves.get(strategy)
    }

    /// Calculate contribution to portfolio P&L
    pub fn calculate_contribution(&self, strategy: &str, total_portfolio_pnl: f64) -> f64 {
        if let Some(perf) = self.strategy_performance.get(strategy) {
            if total_portfolio_pnl != 0.0 {
                (perf.net_profit / total_portfolio_pnl) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Print performance summary
    pub fn print_summary(&self, strategy: &str) {
        if let Some(perf) = self.strategy_performance.get(strategy) {
            println!("\n{} Performance Summary", strategy);
            println!("═══════════════════════════════════════════");
            println!("Total Trades:      {}", perf.total_trades);
            println!("Winning Trades:    {} ({:.1}%)", perf.winning_trades, perf.win_rate);
            println!("Losing Trades:     {}", perf.losing_trades);
            println!();
            println!("Gross Profit:      ${:.2}", perf.gross_profit);
            println!("Gross Loss:        ${:.2}", perf.gross_loss);
            println!("Net Profit:        ${:.2}", perf.net_profit);
            println!();
            println!("Avg Win:           ${:.2}", perf.avg_win);
            println!("Avg Loss:          ${:.2}", perf.avg_loss);
            println!("Profit Factor:     {:.2}", perf.profit_factor);
            println!();
            println!("Sharpe Ratio:      {:.2}", perf.sharpe_ratio);
            println!("Sortino Ratio:     {:.2}", perf.sortino_ratio);
            println!("Max Drawdown:      ${:.2} ({:.1}%)", perf.max_drawdown, perf.max_drawdown_pct);
            println!("ROI:               {:.2}%", perf.roi);
            println!("═══════════════════════════════════════════\n");
        }
    }
}

/// Strategy comparison data
#[derive(Debug, Clone)]
pub struct StrategyComparison {
    pub strategy: String,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub net_profit: f64,
    pub roi: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtesting::position::{Position, OptionType};
    use crate::models::american::ExerciseStyle;

    fn create_test_position(id: usize, pnl: f64) -> Position {
        let mut pos = Position {
            id,
            symbol: "TEST".to_string(),
            option_type: OptionType::Call,
            exercise_style: ExerciseStyle::European,
            strike: 100.0,
            quantity: 10,
            entry_price: 2.0,
            entry_date: "2024-01-01".to_string(),
            entry_spot: 100.0,
            exit_price: None,
            exit_date: None,
            exit_spot: None,
            status: PositionStatus::Open,
            days_held: 0,
            entry_greeks: None,
            realized_pnl: pnl,
            unrealized_pnl: 0.0,
        };
        pos.close(2.5, "2024-01-10".to_string(), 105.0, 10);
        pos.realized_pnl = pnl; // Override for test
        pos
    }

    #[test]
    fn test_strategy_performance_calculation() {
        let mut attribution = PerformanceAttribution::new();

        let positions = vec![
            create_test_position(1, 150.0),   // Winner
            create_test_position(2, -100.0),  // Loser
            create_test_position(3, 200.0),   // Winner
            create_test_position(4, -50.0),   // Loser
        ];

        let perf = attribution.calculate_strategy_performance("TestStrategy", &positions);

        assert_eq!(perf.total_trades, 4);
        assert_eq!(perf.winning_trades, 2);
        assert_eq!(perf.losing_trades, 2);
        assert_eq!(perf.win_rate, 50.0);
        assert_eq!(perf.gross_profit, 350.0);
        assert_eq!(perf.gross_loss, 150.0);
        assert_eq!(perf.net_profit, 200.0);
    }

    #[test]
    fn test_profit_factor() {
        let mut attribution = PerformanceAttribution::new();

        let positions = vec![
            create_test_position(1, 300.0),   // Winner
            create_test_position(2, -100.0),  // Loser
        ];

        let perf = attribution.calculate_strategy_performance("TestStrategy", &positions);

        // Profit factor = 300 / 100 = 3.0
        assert!((perf.profit_factor - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_avg_win_loss() {
        let mut attribution = PerformanceAttribution::new();

        let positions = vec![
            create_test_position(1, 200.0),
            create_test_position(2, 400.0),
            create_test_position(3, -100.0),
            create_test_position(4, -200.0),
        ];

        let perf = attribution.calculate_strategy_performance("TestStrategy", &positions);

        assert_eq!(perf.avg_win, 300.0);   // (200 + 400) / 2
        assert_eq!(perf.avg_loss, 150.0);  // (100 + 200) / 2
    }

    #[test]
    fn test_drawdown_calculation() {
        let attribution = PerformanceAttribution::new();

        let equity_curve = vec![0.0, 1000.0, 1500.0, 800.0, 1200.0, 600.0];
        let (max_dd, max_dd_pct) = attribution.calculate_drawdown(&equity_curve);

        // Max drawdown: from 1500 to 600 = 900
        assert!((max_dd - 900.0).abs() < 0.01);
        assert!(max_dd_pct > 0.0);
    }

    #[test]
    fn test_sharpe_ratio() {
        let attribution = PerformanceAttribution::new();

        let returns = vec![1.0, 2.0, -0.5, 1.5, 0.5, 2.5];
        let sharpe = attribution.calculate_sharpe_ratio(&returns);

        // Should calculate positive Sharpe for positive returns
        assert!(sharpe > 0.0);
    }

    #[test]
    fn test_sortino_ratio() {
        let attribution = PerformanceAttribution::new();

        let returns = vec![2.0, 3.0, -1.0, 2.5, -0.5, 3.5];
        let sortino = attribution.calculate_sortino_ratio(&returns);

        // Should calculate Sortino ratio
        assert!(sortino != 0.0);
    }

    #[test]
    fn test_strategy_comparison() {
        let mut attribution = PerformanceAttribution::new();

        let positions1 = vec![
            create_test_position(1, 300.0),
            create_test_position(2, -100.0),
        ];

        let positions2 = vec![
            create_test_position(3, 100.0),
            create_test_position(4, -50.0),
        ];

        attribution.calculate_strategy_performance("StrategyA", &positions1);
        attribution.calculate_strategy_performance("StrategyB", &positions2);

        let comparisons = attribution.compare_strategies(&["StrategyA", "StrategyB"]);

        assert_eq!(comparisons.len(), 2);
    }

    #[test]
    fn test_best_strategy() {
        let mut attribution = PerformanceAttribution::new();

        let good_positions = vec![
            create_test_position(1, 500.0),
            create_test_position(2, -50.0),
        ];

        let bad_positions = vec![
            create_test_position(3, 100.0),
            create_test_position(4, -200.0),
        ];

        attribution.calculate_strategy_performance("GoodStrategy", &good_positions);
        attribution.calculate_strategy_performance("BadStrategy", &bad_positions);

        let best = attribution.best_strategy();

        // Best strategy should be the one with better Sharpe
        assert!(best.is_some());
    }

    #[test]
    fn test_contribution_calculation() {
        let mut attribution = PerformanceAttribution::new();

        let positions = vec![
            create_test_position(1, 200.0),
            create_test_position(2, -100.0),
        ];

        attribution.calculate_strategy_performance("TestStrategy", &positions);

        let contribution = attribution.calculate_contribution("TestStrategy", 500.0);

        // Net profit = 100, total P&L = 500 => contribution = 20%
        assert!((contribution - 20.0).abs() < 1.0);
    }

    #[test]
    fn test_empty_positions() {
        let mut attribution = PerformanceAttribution::new();

        let positions: Vec<Position> = vec![];
        let perf = attribution.calculate_strategy_performance("EmptyStrategy", &positions);

        assert_eq!(perf.total_trades, 0);
        assert_eq!(perf.net_profit, 0.0);
        assert_eq!(perf.win_rate, 0.0);
    }
}
