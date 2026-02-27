// Portfolio Management Example
// Demonstrates position sizing, risk analytics, allocation, and performance tracking

use dollarbill::portfolio::{
    PortfolioManager, PortfolioConfig, SizingMethod, AllocationMethod, 
    RiskLimits, StrategyStats,
};
use dollarbill::backtesting::position::{Position, PositionStatus, OptionType};
use dollarbill::models::american::ExerciseStyle;
use dollarbill::models::bs_mod::Greeks;
use std::collections::HashMap;

fn main() {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║        PORTFOLIO MANAGEMENT DEMONSTRATION                ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // ========== CONFIGURATION ==========
    println!("📋 Step 1: Configure Portfolio Manager\n");
    
    let config = PortfolioConfig {
        initial_capital: 100_000.0,
        max_risk_per_trade: 2.0,    // 2% risk per trade
        max_position_pct: 10.0,      // 10% max position size
        sizing_method: SizingMethod::VolatilityBased,
        allocation_method: AllocationMethod::RiskParity,
        risk_limits: RiskLimits {
            max_portfolio_delta: 0.30,    // 30% delta
            max_concentration_pct: 20.0,   // 20% max per position
            ..Default::default()
        },
    };

    let mut manager = PortfolioManager::new(config);
    
    println!("   Initial Capital: $100,000");
    println!("   Risk per Trade:  2%");
    println!("   Max Position:    10%");
    println!("   Sizing Method:   Volatility-Based");
    println!("   Allocation:      Risk Parity\n");

    // ========== ADD STRATEGIES ==========
    println!("📊 Step 2: Add Strategy Allocations\n");
    
    manager.add_strategy("IronCondor".to_string(), 40_000.0, 15.0, 35.0);
    manager.add_strategy("CreditSpreads".to_string(), 40_000.0, 15.0, 35.0);
    manager.add_strategy("Straddles".to_string(), 30_000.0, 10.0, 25.0);
    
    println!("   ✓ Iron Condor    (15-35% allocation, $40k capacity)");
    println!("   ✓ Credit Spreads (15-35% allocation, $40k capacity)");
    println!("   ✓ Straddles      (10-25% allocation, $30k capacity)\n");

    // ========== OPTIMIZE ALLOCATIONS ==========
    println!("🎯 Step 3: Optimize Strategy Allocations\n");
    
    let mut strategy_stats = HashMap::new();
    
    strategy_stats.insert("IronCondor".to_string(), StrategyStats {
        sharpe_ratio: 1.5,
        volatility: 0.20,  // Lower volatility
        win_rate: 0.65,
        avg_return: 0.08,
        max_drawdown: 0.15,
    });
    
    strategy_stats.insert("CreditSpreads".to_string(), StrategyStats {
        sharpe_ratio: 1.2,
        volatility: 0.25,  // Medium volatility
        win_rate: 0.60,
        avg_return: 0.06,
        max_drawdown: 0.18,
    });
    
    strategy_stats.insert("Straddles".to_string(), StrategyStats {
        sharpe_ratio: 0.8,
        volatility: 0.40,  // Higher volatility
        win_rate: 0.45,
        avg_return: 0.10,
        max_drawdown: 0.25,
    });
    
    manager.optimize_allocations(&strategy_stats);
    
    println!("   Strategy Stats:");
    println!("   ┌─────────────────┬──────────┬────────────┬──────────┐");
    println!("   │ Strategy        │ Sharpe   │ Volatility │ Win Rate │");
    println!("   ├─────────────────┼──────────┼────────────┼──────────┤");
    println!("   │ Iron Condor     │   1.50   │    20%     │   65%    │");
    println!("   │ Credit Spreads  │   1.20   │    25%     │   60%    │");
    println!("   │ Straddles       │   0.80   │    40%     │   45%    │");
    println!("   └─────────────────┴──────────┴────────────┴──────────┘\n");

    // ========== POSITION SIZING EXAMPLES ==========
    println!("💼 Step 4: Calculate Position Sizes\n");
    
    // Example 1: Simple option trade
    let size1 = manager.calculate_position_size(2.50, 0.30, Some(0.60), Some(300.0), Some(200.0));
    println!("   Simple Option Trade:");
    println!("     Price: $2.50, IV: 30%, Win Rate: 60%");
    println!("     → Suggested Size: {} contracts\n", size1);
    
    // Example 2: Iron condor
    let ic_size = manager.calculate_iron_condor_size(500.0, 1.50, 0.35);
    println!("   Iron Condor:");
    println!("     Max Loss: $500, Credit: $1.50, IV: 35%");
    println!("     → Suggested Size: {} spreads\n", ic_size);
    
    // Example 3: Credit spread
    let cs_size = manager.calculate_credit_spread_size(5.0, 1.25, 0.28);
    println!("   Credit Spread:");
    println!("     Width: $5, Credit: $1.25, IV: 28%");
    println!("     → Suggested Size: {} spreads\n", cs_size);

    // ========== SIMULATE POSITIONS ==========
    println!("📍 Step 5: Simulate Open Positions\n");
    
    let positions = create_sample_positions();
    manager.update_positions(positions.clone());
    
    println!("   Created {} open positions", positions.len());
    for (i, pos) in positions.iter().enumerate() {
        println!("     {}. {} {} @ ${:.2} ({})",
            i + 1,
            pos.symbol,
            match pos.option_type {
                OptionType::Call => "Call",
                OptionType::Put => "Put",
            },
            pos.entry_price,
            if pos.quantity > 0 { "LONG" } else { "SHORT" }
        );
    }
    println!();

    // ========== RISK ANALYSIS ==========
    println!("⚠️  Step 6: Analyze Portfolio Risk\n");
    
    let risk = manager.get_portfolio_risk();
    
    println!("   Portfolio Greeks:");
    println!("     Delta:  {:>10.2}", risk.total_delta);
    println!("     Gamma:  {:>10.2}", risk.total_gamma);
    println!("     Theta:  {:>10.2}", risk.total_theta);
    println!("     Vega:   {:>10.2}", risk.total_vega);
    println!();
    println!("   Risk Metrics:");
    println!("     Net Exposure:       ${:>10.2}", risk.net_exposure);
    println!("     Gross Exposure:     ${:>10.2}", risk.gross_exposure);
    println!("     VaR (95%):          ${:>10.2}", risk.var_95);
    println!("     VaR (99%):          ${:>10.2}", risk.var_99);
    println!("     Concentration Risk: {:>10.1}%", risk.concentration_risk);
    println!();

    // ========== POSITION APPROVAL ==========
    println!("✅ Step 7: Check if New Position Can Be Added\n");
    
    let decision = manager.can_take_position("IronCondor", 2.0, 0.30, 20);
    
    println!("   Attempting to add 20 contracts @ $2.00 (IV: 30%)");
    println!("   Can Trade:       {}", if decision.can_trade { "✅ YES" } else { "❌ NO" });
    println!("   Suggested Size:  {} contracts", decision.suggested_size);
    
    if !decision.risk_warnings.is_empty() {
        println!("   Warnings:");
        for warning in &decision.risk_warnings {
            println!("     ⚠️  {}", warning);
        }
    }
    
    if let Some(ref info) = decision.allocation_info {
        println!("   Allocation: {}", info);
    }
    println!();

    // ========== REBALANCING ==========
    println!("🔄 Step 8: Get Rebalancing Recommendations\n");
    
    let rebalances = manager.get_rebalancing_recommendations();
    
    if rebalances.is_empty() {
        println!("   ✓ Portfolio is balanced - no rebalancing needed\n");
    } else {
        println!("   Rebalancing Trades:");
        for (strategy, amount, action) in rebalances {
            println!("     {} {} by ${:>10.2}", action, strategy, amount.abs());
        }
        println!();
    }

    // ========== PERFORMANCE TRACKING ==========
    println!("📈 Step 9: Track Strategy Performance\n");
    
    // Create closed positions for performance tracking
    let closed_positions = create_closed_positions();
    
    manager.calculate_strategy_performance("IronCondor", &closed_positions);
    manager.print_performance_report("IronCondor");
    
    if let Some(best) = manager.best_strategy() {
        println!("🏆 Best Performing Strategy: {}\n", best);
    }

    // ========== SUMMARY ==========
    println!("
═══════════════════════════════════════════════════════════════");
    manager.print_summary();
    
    println!("\n✨ Portfolio Management Features Demonstrated:");
    println!("   1. ✅ Position sizing (volatility-based)");
    println!("   2. ✅ Multi-strategy allocation (risk parity)");
    println!("   3. ✅ Portfolio risk analytics (Greeks, VaR)");
    println!("   4. ✅ Risk limit enforcement");
    println!("   5. ✅ Rebalancing recommendations");
    println!("   6. ✅ Performance attribution");
    println!("\n🎯 Portfolio management system ready for live trading!");
}

/// Create sample open positions for demonstration
fn create_sample_positions() -> Vec<Position> {
    vec![
        create_position(1, "AAPL", 10, 2.50, 0.5),
        create_position(2, "TSLA", -5, 3.20, -0.3),
        create_position(3, "SPY", 15, 1.80, 0.4),
        create_position(4, "QQQ", -8, 2.90, -0.35),
    ]
}

/// Create sample closed positions for performance tracking
fn create_closed_positions() -> Vec<Position> {
    let mut positions = vec![];
    
    // Winners
    let mut pos1 = create_position(10, "AAPL", 10, 2.00, 0.5);
    pos1.close(2.50, "2024-02-15".to_string(), 175.0, 10);
    pos1.realized_pnl = 500.0;
    positions.push(pos1);
    
    let mut pos2 = create_position(11, "TSLA", 5, 3.00, 0.6);
    pos2.close(4.00, "2024-02-16".to_string(), 250.0, 8);
    pos2.realized_pnl = 500.0;
    positions.push(pos2);
    
    // Losers
    let mut pos3 = create_position(12, "SPY", 20, 1.50, 0.4);
    pos3.close(1.00, "2024-02-17".to_string(), 510.0, 12);
    pos3.realized_pnl = -1000.0;
    positions.push(pos3);
    
    let mut pos4 = create_position(13, "QQQ", 15, 2.50, 0.45);
    pos4.close(2.00, "2024-02-18".to_string(), 445.0, 9);
    pos4.realized_pnl = -750.0;
    positions.push(pos4);
    
    // More winners
    let mut pos5 = create_position(14, "NVDA", 8, 4.00, 0.7);
    pos5.close(5.50, "2024-02-19".to_string(), 870.0, 7);
    pos5.realized_pnl = 1200.0;
    positions.push(pos5);
    
    positions
}

/// Helper to create a position
fn create_position(id: usize, symbol: &str, quantity: i32, price: f64, delta: f64) -> Position {
    Position {
        id,
        symbol: symbol.to_string(),
        option_type: if delta > 0.0 { OptionType::Call } else { OptionType::Put },
        exercise_style: ExerciseStyle::European,
        strike: 100.0,
        quantity,
        entry_price: price,
        entry_date: "2024-02-01".to_string(),
        entry_spot: 100.0,
        exit_price: None,
        exit_date: None,
        exit_spot: None,
        status: PositionStatus::Open,
        days_held: 0,
        entry_greeks: Some(Greeks {
            price,
            delta,
            gamma: 0.05,
            theta: -0.02,
            vega: 0.15,
            rho: 0.1,
        }),
        realized_pnl: 0.0,
        unrealized_pnl: 0.0,
    }
}
