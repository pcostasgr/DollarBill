#![allow(dead_code)]
// Performance metrics and analytics

use crate::backtesting::position::Position;
use crate::backtesting::trade::Trade;

#[derive(Debug, Clone)]
#[allow(dead_code)] // Used by external examples and may be expanded
pub struct EquityCurve {
    pub dates: Vec<String>,
    pub equity: Vec<f64>,
    pub drawdown: Vec<f64>,
}

impl EquityCurve {
    pub fn new() -> Self {
        Self {
            dates: Vec::new(),
            equity: Vec::new(),
            drawdown: Vec::new(),
        }
    }
    
    pub fn add_point(&mut self, date: String, equity: f64) {
        self.dates.push(date);
        self.equity.push(equity);
        
        // Calculate drawdown from peak
        let peak = self.equity.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let drawdown = if peak > 0.0 {
            ((equity - peak) / peak) * 100.0
        } else {
            0.0
        };
        self.drawdown.push(drawdown);
    }
}

#[derive(Debug)]
#[allow(dead_code)] // Used by external examples and may be expanded
pub struct PerformanceMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    
    pub total_pnl: f64,
    pub total_return_pct: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub profit_factor: f64,
    
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    
    pub avg_days_held: f64,
    pub total_commissions: f64,
    pub total_slippage: f64,
}

impl PerformanceMetrics {
    pub fn calculate(
        positions: &[Position],
        trades: &[Trade],
        initial_capital: f64,
        equity_curve: &EquityCurve,
    ) -> Self {
        let closed_positions: Vec<_> = positions.iter()
            .filter(|p| matches!(p.status, crate::backtesting::position::PositionStatus::Closed | crate::backtesting::position::PositionStatus::Expired))
            .collect();
        
        let total_trades = closed_positions.len();
        let winning_trades = closed_positions.iter().filter(|p| p.is_winner()).count();
        let losing_trades = total_trades - winning_trades;
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };
        
        // P&L calculations
        let total_pnl: f64 = closed_positions.iter().map(|p| p.realized_pnl).sum();
        let total_return_pct = if initial_capital > 0.0 {
            (total_pnl / initial_capital) * 100.0
        } else {
            0.0
        };
        
        let wins: Vec<f64> = closed_positions.iter()
            .filter(|p| p.is_winner())
            .map(|p| p.realized_pnl)
            .collect();
        let losses: Vec<f64> = closed_positions.iter()
            .filter(|p| !p.is_winner())
            .map(|p| p.realized_pnl.abs())
            .collect();
        
        let avg_win = if !wins.is_empty() {
            wins.iter().sum::<f64>() / wins.len() as f64
        } else {
            0.0
        };
        
        let avg_loss = if !losses.is_empty() {
            losses.iter().sum::<f64>() / losses.len() as f64
        } else {
            0.0
        };
        
        let largest_win = wins.iter().fold(0.0_f64, |a, &b| a.max(b));
        let largest_loss = losses.iter().fold(0.0_f64, |a, &b| a.max(b));
        
        let total_wins: f64 = wins.iter().sum();
        let total_losses: f64 = losses.iter().sum();
        let profit_factor = if total_losses > 0.0 {
            total_wins / total_losses
        } else if total_wins > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        
        // Sharpe ratio (simplified - assumes daily returns)
        let sharpe_ratio = Self::calculate_sharpe_ratio(&equity_curve.equity, initial_capital);
        
        // Max drawdown
        let (max_drawdown, max_drawdown_pct) = Self::calculate_max_drawdown(&equity_curve.equity);
        
        // Average holding period
        let avg_days_held = if !closed_positions.is_empty() {
            closed_positions.iter().map(|p| p.days_held as f64).sum::<f64>() / closed_positions.len() as f64
        } else {
            0.0
        };
        
        // Total commissions and slippage
        let total_commissions: f64 = trades.iter().map(|t| t.commission).sum();
        let total_slippage: f64 = trades.iter().map(|t| t.slippage).sum();
        
        Self {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_pnl,
            total_return_pct,
            avg_win,
            avg_loss,
            largest_win,
            largest_loss,
            profit_factor,
            sharpe_ratio,
            max_drawdown,
            max_drawdown_pct,
            avg_days_held,
            total_commissions,
            total_slippage,
        }
    }
    
    fn calculate_sharpe_ratio(equity_curve: &[f64], _initial_capital: f64) -> f64 {
        if equity_curve.len() < 2 {
            return 0.0;
        }
        
        // Calculate daily returns
        let mut returns = Vec::new();
        for i in 1..equity_curve.len() {
            let ret = (equity_curve[i] - equity_curve[i-1]) / equity_curve[i-1];
            returns.push(ret);
        }
        
        if returns.is_empty() {
            return 0.0;
        }
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev > 0.0 {
            // Annualize assuming 252 trading days
            mean_return / std_dev * (252.0_f64).sqrt()
        } else {
            0.0
        }
    }
    
    fn calculate_max_drawdown(equity_curve: &[f64]) -> (f64, f64) {
        if equity_curve.is_empty() {
            return (0.0, 0.0);
        }
        
        let mut peak = equity_curve[0];
        let mut max_dd = 0.0_f64;
        let mut max_dd_pct = 0.0_f64;
        
        for &equity in equity_curve {
            if equity > peak {
                peak = equity;
            }
            let dd = peak - equity;
            let dd_pct = if peak > 0.0 {
                (dd / peak) * 100.0
            } else {
                0.0
            };
            
            if dd > max_dd {
                max_dd = dd;
                max_dd_pct = dd_pct;
            }
        }
        
        (max_dd, max_dd_pct)
    }
}

#[derive(Debug)]
#[allow(dead_code)] // Used by external examples and may be expanded
pub struct BacktestResult {
    pub symbol: String,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub positions: Vec<Position>,
    pub trades: Vec<Trade>,
    pub equity_curve: EquityCurve,
    pub metrics: PerformanceMetrics,
}

impl BacktestResult {
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(80));
        println!("BACKTEST RESULTS - {}", self.symbol);
        println!("{}", "=".repeat(80));
        println!("Period: {} to {}", self.start_date, self.end_date);
        println!("Initial Capital: ${:.2}", self.initial_capital);
        println!("Final Capital: ${:.2}", self.final_capital);
        println!();
        
        println!("📊 PERFORMANCE METRICS");
        println!("{}", "-".repeat(80));
        println!("Total P&L:        ${:>12.2}  ({:>6.2}%)", 
                 self.metrics.total_pnl, self.metrics.total_return_pct);
        println!("Sharpe Ratio:     {:>12.2}", self.metrics.sharpe_ratio);
        println!("Max Drawdown:     ${:>12.2}  ({:>6.2}%)", 
                 self.metrics.max_drawdown, self.metrics.max_drawdown_pct);
        println!();
        
        println!("📈 TRADE STATISTICS");
        println!("{}", "-".repeat(80));
        println!("Total Trades:     {:>12}", self.metrics.total_trades);
        println!("Winning Trades:   {:>12}  ({:>6.2}%)", 
                 self.metrics.winning_trades, self.metrics.win_rate);
        println!("Losing Trades:    {:>12}", self.metrics.losing_trades);
        println!();
        println!("Average Win:      ${:>12.2}", self.metrics.avg_win);
        println!("Average Loss:     ${:>12.2}", self.metrics.avg_loss);
        println!("Largest Win:      ${:>12.2}", self.metrics.largest_win);
        println!("Largest Loss:     ${:>12.2}", self.metrics.largest_loss);
        println!("Profit Factor:    {:>12.2}", self.metrics.profit_factor);
        println!();
        println!("Avg Days Held:    {:>12.1}", self.metrics.avg_days_held);
        println!("Total Commissions:${:>12.2}", self.metrics.total_commissions);
        println!("Total Slippage:   ${:>12.2}", self.metrics.total_slippage);
        println!("{}", "=".repeat(80));
    }
    
    pub fn print_trades(&self, limit: usize) {
        println!("\n📋 TRADE HISTORY (Top {} by P&L)", limit);
        println!("{}", "-".repeat(120));
        println!("{:<6} {:<8} {:<5} {:<8} {:<10} {:<10} {:<10} {:<12} {:<8} {:<8}",
                 "ID", "Symbol", "Type", "Strike", "Entry", "Exit", "Days", "P&L", "ROI %", "Result");
        println!("{}", "-".repeat(120));
        
        let mut sorted_positions = self.positions.clone();
        sorted_positions.sort_by(|a, b| b.realized_pnl.partial_cmp(&a.realized_pnl).unwrap());
        
        for pos in sorted_positions.iter().take(limit) {
            if !matches!(pos.status, crate::backtesting::position::PositionStatus::Closed | 
                                     crate::backtesting::position::PositionStatus::Expired) {
                continue;
            }
            
            let option_type = match pos.option_type {
                crate::backtesting::position::OptionType::Call => "CALL",
                crate::backtesting::position::OptionType::Put => "PUT",
            };
            
            let result = if pos.is_winner() { "WIN ✓" } else { "LOSS ✗" };
            
            println!("{:<6} {:<8} {:<5} ${:<7.2} ${:<9.2} ${:<9.2} {:<10} ${:<11.2} {:>7.1}%  {}",
                     pos.id,
                     pos.symbol,
                     option_type,
                     pos.strike,
                     pos.entry_price,
                     pos.exit_price.unwrap_or(0.0),
                     pos.days_held,
                     pos.realized_pnl,
                     pos.roi(),
                     result);
        }
    }
}
