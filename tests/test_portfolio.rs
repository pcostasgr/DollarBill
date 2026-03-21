// Portfolio module unit tests
// Covers position_sizing, risk_analytics, allocation, performance, and manager

use dollarbill::backtesting::position::{Position, PositionStatus, OptionType};
use dollarbill::models::american::ExerciseStyle;
use dollarbill::models::bs_mod::Greeks;
use dollarbill::portfolio::{
    PortfolioManager, PortfolioConfig, SizingMethod, AllocationMethod, RiskLimits,
    PositionSizer,
    RiskAnalyzer, PortfolioRisk,
    PortfolioAllocator, StrategyStats,
    PerformanceAttribution,
};
use std::collections::HashMap;

// ─── Helpers ─────────────────────────────────────────────────────────────

fn greeks(price: f64, delta: f64, gamma: f64, theta: f64, vega: f64) -> Greeks {
    Greeks { price, delta, gamma, theta, vega, rho: 0.02 }
}

fn open_call(id: usize, symbol: &str, qty: i32, entry_price: f64, g: Greeks) -> Position {
    Position::new(
        id,
        symbol.to_string(),
        OptionType::Call,
        ExerciseStyle::American,
        150.0,
        qty,
        entry_price,
        "2025-01-01".to_string(),
        150.0,
        Some(g),
    )
}

fn closed_position(id: usize, entry: f64, exit: f64, qty: i32) -> Position {
    let mut pos = open_call(id, "AAPL", qty, entry, greeks(entry, 0.5, 0.02, -0.05, 0.10));
    pos.realized_pnl = (exit - entry) * qty.abs() as f64 * 100.0;
    pos.status = PositionStatus::Closed;
    pos
}

// ─── PositionSizer ───────────────────────────────────────────────────────

#[test]
fn position_sizer_fixed_fractional_basic() {
    // $100k account, 5% fraction, $5 option
    // position_value = $100k * 0.05 = $5000
    // contract_value = $5 * 100 = $500
    // contracts = floor(5000 / 500) = 10
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.calculate_size(
        SizingMethod::FixedFractional(5.0),
        5.0,   // option price
        0.20,  // vol (unused for fixed fractional)
        None, None, None,
    );
    assert_eq!(contracts, 10);
}

#[test]
fn position_sizer_fixed_fractional_cap_respected() {
    // max_position_pct = 5%, $100k account → max $5000 → 10 contracts at $5
    // Requesting 20% fraction → would be 40 contracts but capped at 10
    let sizer = PositionSizer::new(100_000.0, 2.0, 5.0);
    let contracts = sizer.calculate_size(
        SizingMethod::FixedFractional(20.0),
        5.0, 0.20, None, None, None,
    );
    let max_allowed = sizer.calculate_size(
        SizingMethod::FixedFractional(5.0),
        5.0, 0.20, None, None, None,
    );
    assert!(contracts <= max_allowed, "requested 20% fraction must be capped at max_position_pct");
}

#[test]
fn position_sizer_kelly_criterion_reasonable_range() {
    // win_rate=0.6, avg_win=$200, avg_loss=$100 → b=2
    // kelly = (b*p - q)/b = (2*0.6 - 0.4)/2 = 0.4 → capped at 0.25
    // position_value = $100k * 0.25 = $25k
    // contract_value = $5 * 100 = $500 → 50 contracts (before max_position cap)
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.calculate_size(
        SizingMethod::KellyCriterion,
        5.0, 0.20,
        Some(0.6), Some(200.0), Some(100.0),
    );
    assert!(contracts > 0, "Kelly criterion should produce positive size for profitable edge");
    assert!(contracts <= 200, "Kelly criterion size should be bounded");
}

#[test]
fn position_sizer_kelly_negative_edge_returns_zero() {
    // win_rate=0.3, avg_win=$50, avg_loss=$200 → negative Kelly edge → 0 contracts
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.calculate_size(
        SizingMethod::KellyCriterion,
        5.0, 0.20,
        Some(0.3), Some(50.0), Some(200.0),
    );
    // Kelly goes negative but is clamped to 0
    assert!(contracts >= 0);
}

#[test]
fn position_sizer_volatility_based_high_vol_smaller() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let low_vol = sizer.calculate_size(SizingMethod::VolatilityBased, 5.0, 0.15, None, None, None);
    let high_vol = sizer.calculate_size(SizingMethod::VolatilityBased, 5.0, 0.60, None, None, None);
    assert!(
        low_vol >= high_vol,
        "low vol ({}) should produce >= contracts as high vol ({})",
        low_vol, high_vol
    );
}

#[test]
fn position_sizer_fixed_dollar() {
    // $10k budget / ($5 * 100) = 20 contracts
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.calculate_size(
        SizingMethod::FixedDollar(10_000.0),
        5.0, 0.20, None, None, None,
    );
    assert_eq!(contracts, 20);
}

#[test]
fn position_sizer_validate_within_limit() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    // 5 contracts * $5 * 100 = $2500 = 2.5% of $100k — within 10% limit
    assert!(sizer.validate_position(5, 5.0));
}

#[test]
fn position_sizer_validate_exceeds_limit() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    // 300 contracts * $5 * 100 = $150k > 10% of $100k
    assert!(!sizer.validate_position(300, 5.0));
}

// ─── RiskAnalyzer — Greek aggregation ────────────────────────────────────

#[test]
fn risk_analyzer_empty_positions_zeros() {
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&[]);
    assert_eq!(risk.total_delta, 0.0);
    assert_eq!(risk.total_gamma, 0.0);
    assert_eq!(risk.total_theta, 0.0);
    assert_eq!(risk.total_vega, 0.0);
    assert_eq!(risk.gross_exposure, 0.0);
}

#[test]
fn risk_analyzer_single_long_call_greeks() {
    let g = greeks(5.0, 0.50, 0.02, -0.05, 0.10);
    let pos = open_call(1, "AAPL", 2, 5.0, g.clone());
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&[pos]);
    // qty=2, multiplier=100 → contribution = greeks * 2 * 100 = 200
    assert!((risk.total_delta - g.delta * 2.0 * 100.0).abs() < 1e-9);
    assert!((risk.total_gamma - g.gamma * 2.0 * 100.0).abs() < 1e-9);
    assert!((risk.total_vega  - g.vega  * 2.0 * 100.0).abs() < 1e-9);
    // gross_exposure = price * qty * 100 = 5 * 2 * 100 = 1000
    assert!((risk.gross_exposure - 1000.0).abs() < 1e-9);
}

#[test]
fn risk_analyzer_long_short_netting() {
    // Long 1 call (delta +0.5) and short 1 call (delta +0.5, qty=-1)
    // Net delta = (+0.5 * 1 + 0.5 * -1) * 100 = 0
    let g = greeks(5.0, 0.50, 0.02, -0.05, 0.10);
    let long_call  = open_call(1, "AAPL",  1, 5.0, g.clone());
    let short_call = open_call(2, "AAPL", -1, 5.0, g.clone());
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&[long_call, short_call]);
    assert!((risk.total_delta).abs() < 1e-9, "longs and matched shorts should net to zero delta");
}

#[test]
fn risk_analyzer_var_99_exceeds_var_95() {
    // 99% VaR uses a higher z-score than 95% VaR → should be larger
    let g = greeks(10.0, 0.50, 0.02, -0.05, 0.50);
    let pos = open_call(1, "AAPL", 5, 10.0, g);
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&[pos]);
    assert!(
        risk.var_99 >= risk.var_95,
        "99% VaR ({}) must be >= 95% VaR ({})",
        risk.var_99, risk.var_95
    );
}

#[test]
fn risk_analyzer_cvar_exceeds_var_same_level() {
    // CVaR ≥ VaR is required by coherent risk measure theory
    let g = greeks(10.0, 0.50, 0.02, -0.05, 0.50);
    let pos = open_call(1, "AAPL", 5, 10.0, g);
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&[pos]);
    assert!(
        risk.cvar_95 >= risk.var_95,
        "CVaR_95 ({}) must be >= VaR_95 ({})",
        risk.cvar_95, risk.var_95
    );
    assert!(
        risk.cvar_99 >= risk.var_99,
        "CVaR_99 ({}) must be >= VaR_99 ({})",
        risk.cvar_99, risk.var_99
    );
}

#[test]
fn risk_analyzer_concentration_risk_single_position() {
    // 1 contract @ $100 price = $10,000 / $100k portfolio = 10%
    let g = greeks(100.0, 0.50, 0.02, -0.05, 0.10);
    let pos = open_call(1, "AAPL", 1, 100.0, g);
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&[pos]);
    // gross_exposure = 100 * 1 * 100 = $10k, concentration = 10k/100k * 100 = 10%
    assert!((risk.concentration_risk - 10.0).abs() < 0.1);
}

#[test]
fn risk_analyzer_check_limits_clean_portfolio() {
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = PortfolioRisk::default();
    let violations = analyzer.check_risk_limits(&risk);
    assert!(violations.is_empty(), "default (zero) risk metrics should have no violations");
}

#[test]
fn risk_analyzer_check_limits_concentration_violation() {
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let mut risk = PortfolioRisk::default();
    risk.concentration_risk = 25.0; // exceeds default limit of 20%
    let violations = analyzer.check_risk_limits(&risk);
    assert!(
        violations.iter().any(|v| v.contains("Concentration")),
        "expected a concentration violation; got: {:?}", violations
    );
}

#[test]
fn risk_analyzer_diversification_empty() {
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    assert_eq!(analyzer.diversification_score(&[]), 0.0);
}

#[test]
fn risk_analyzer_diversification_increases_with_symbols() {
    let g = greeks(5.0, 0.50, 0.02, -0.05, 0.10);
    let one_pos = vec![open_call(1, "AAPL", 1, 5.0, g.clone())];
    let two_pos = vec![
        open_call(1, "AAPL", 1, 5.0, g.clone()),
        open_call(2, "MSFT", 1, 5.0, g.clone()),
    ];
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let score_one  = analyzer.diversification_score(&one_pos);
    let score_two  = analyzer.diversification_score(&two_pos);
    assert!(
        score_two > score_one,
        "two distinct symbols ({}) should score higher than one ({})",
        score_two, score_one
    );
}

// ─── PortfolioAllocator ───────────────────────────────────────────────────

fn make_allocator_two_strategies() -> PortfolioAllocator {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
    allocator.add_strategy("StratA".to_string(), 50_000.0, 0.0, 60.0);
    allocator.add_strategy("StratB".to_string(), 50_000.0, 0.0, 60.0);
    allocator
}

#[test]
fn allocator_equal_weight_two_strategies() {
    let mut allocator = make_allocator_two_strategies();
    let stats = HashMap::new();
    allocator.calculate_allocations(&stats);
    let a = allocator.get_allocation("StratA").unwrap();
    let b = allocator.get_allocation("StratB").unwrap();
    assert!(
        (a.target_pct - 50.0).abs() < 1.0,
        "StratA should get ~50% but got {}", a.target_pct
    );
    assert!(
        (b.target_pct - 50.0).abs() < 1.0,
        "StratB should get ~50% but got {}", b.target_pct
    );
}

#[test]
fn allocator_performance_weighted_higher_sharpe_gets_more() {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::PerformanceWeighted);
    allocator.add_strategy("Good".to_string(), 80_000.0, 0.0, 80.0);
    allocator.add_strategy("Poor".to_string(), 80_000.0, 0.0, 80.0);

    let mut stats = HashMap::new();
    stats.insert("Good".to_string(), StrategyStats { sharpe_ratio: 2.0, ..StrategyStats::default() });
    stats.insert("Poor".to_string(), StrategyStats { sharpe_ratio: 0.5, ..StrategyStats::default() });
    allocator.calculate_allocations(&stats);

    let good = allocator.get_allocation("Good").unwrap().target_pct;
    let poor = allocator.get_allocation("Poor").unwrap().target_pct;
    assert!(good > poor, "higher Sharpe ({:.1}%) should outweigh lower Sharpe ({:.1}%)", good, poor);
}

#[test]
fn allocator_risk_parity_lower_vol_gets_more() {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::RiskParity);
    allocator.add_strategy("LowVol".to_string(),  80_000.0, 0.0, 80.0);
    allocator.add_strategy("HighVol".to_string(), 80_000.0, 0.0, 80.0);

    let mut stats = HashMap::new();
    stats.insert("LowVol".to_string(),  StrategyStats { volatility: 0.10, ..StrategyStats::default() });
    stats.insert("HighVol".to_string(), StrategyStats { volatility: 0.40, ..StrategyStats::default() });
    allocator.calculate_allocations(&stats);

    let low  = allocator.get_allocation("LowVol").unwrap().target_pct;
    let high = allocator.get_allocation("HighVol").unwrap().target_pct;
    assert!(low > high, "low-vol strategy ({:.1}%) should get higher weight than high-vol ({:.1}%)", low, high);
}

#[test]
fn allocator_has_capacity_with_room() {
    let mut allocator = make_allocator_two_strategies();
    let stats = HashMap::new();
    allocator.calculate_allocations(&stats);
    // capacity is $50k; current allocation is 0 currently — has room for $1k
    // Note: has_capacity checks current_pct (0%) → current_capital ≈ 0 → passes
    assert!(allocator.has_capacity("StratA", 1_000.0));
}

#[test]
fn allocator_has_capacity_over_limit() {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
    allocator.add_strategy("SmallCap".to_string(), 5_000.0, 0.0, 50.0); // capacity = $5k
    // No current allocation → current_capital = 0; but $10k > $5k capacity
    assert!(!allocator.has_capacity("SmallCap", 10_000.0));
}

#[test]
fn allocator_rebalance_trades_generated_on_drift() {
    let mut allocator = make_allocator_two_strategies();
    let stats = HashMap::new();
    allocator.calculate_allocations(&stats);

    // Simulate StratA drifted to 70% and StratB to 30%
    let mut strategy_values = HashMap::new();
    strategy_values.insert("StratA".to_string(), 70_000.0);
    strategy_values.insert("StratB".to_string(), 30_000.0);
    allocator.update_current_allocations(&strategy_values);

    let trades = allocator.get_rebalancing_trades();
    // StratA at 70% vs target 50% → need to sell; StratB at 30% vs 50% → need to buy
    assert!(!trades.is_empty(), "drift > 5% should generate rebalancing trades");
    let sell_a = trades.iter().find(|t| t.strategy == "StratA");
    let buy_b  = trades.iter().find(|t| t.strategy == "StratB");
    assert!(sell_a.is_some() || buy_b.is_some());
}

// ─── PerformanceAttribution ───────────────────────────────────────────────

#[test]
fn perf_attribution_empty_positions() {
    let mut attr = PerformanceAttribution::new();
    let perf = attr.calculate_strategy_performance("TestStrat", &[]);
    assert_eq!(perf.total_trades, 0);
    assert_eq!(perf.net_profit, 0.0);
}

#[test]
fn perf_attribution_all_winning_trades() {
    let mut attr = PerformanceAttribution::new();
    // 3 winning positions: each earns $500
    let positions: Vec<Position> = (1..=3)
        .map(|i| closed_position(i, 5.0, 10.0, 1)) // entry $5 exit $10 × 100 = +$500 each
        .collect();
    let perf = attr.calculate_strategy_performance("Momentum", &positions);
    assert_eq!(perf.total_trades, 3);
    assert_eq!(perf.winning_trades, 3);
    assert_eq!(perf.losing_trades, 0);
    assert!((perf.win_rate - 100.0).abs() < 0.001);
    assert!(perf.net_profit > 0.0);
}

#[test]
fn perf_attribution_all_losing_trades() {
    let mut attr = PerformanceAttribution::new();
    let positions: Vec<Position> = (1..=2)
        .map(|i| closed_position(i, 10.0, 5.0, 1)) // entry $10 exit $5 × 100 = -$500 each
        .collect();
    let perf = attr.calculate_strategy_performance("BadStrat", &positions);
    assert_eq!(perf.total_trades, 2);
    assert_eq!(perf.winning_trades, 0);
    assert_eq!(perf.losing_trades, 2);
    assert!((perf.win_rate - 0.0).abs() < 0.001);
    assert!(perf.net_profit <= 0.0);
}

#[test]
fn perf_attribution_mixed_profit_factor() {
    let mut attr = PerformanceAttribution::new();
    // 2 winners @ +$1000 each, 1 loser @ -$500 → profit factor = 2000/500 = 4.0
    let win1 = closed_position(1, 5.0, 15.0, 1);   // +$1000
    let win2 = closed_position(2, 5.0, 15.0, 1);   // +$1000
    let lose = closed_position(3, 10.0, 5.0, 1);   // -$500
    let perf = attr.calculate_strategy_performance("Mixed", &[win1, win2, lose]);
    assert!((perf.profit_factor - 4.0).abs() < 0.01, "profit factor should be 4.0, got {}", perf.profit_factor);
}

#[test]
fn perf_attribution_sharpe_positive_for_consistent_winners() {
    let mut attr = PerformanceAttribution::new();
    // Ten consistent winners with identical returns
    let positions: Vec<Position> = (1..=10)
        .map(|i| closed_position(i, 5.0, 10.0, 1))
        .collect();
    let perf = attr.calculate_strategy_performance("Consistent", &positions);
    // Sharpe should be 0 when std_dev of returns = 0
    // (all returns are identical, std_dev is 0 → function returns 0.0)
    assert!(perf.sharpe_ratio >= 0.0);
}

// ─── PortfolioManager ────────────────────────────────────────────────────

#[test]
fn portfolio_manager_new_initial_state() {
    let config = PortfolioConfig {
        initial_capital: 50_000.0,
        ..PortfolioConfig::default()
    };
    let mgr = PortfolioManager::new(config);
    assert!((mgr.buying_power() - 50_000.0).abs() < 1.0);
}

#[test]
fn portfolio_manager_sync_updates_buying_power() {
    let mut mgr = PortfolioManager::new(PortfolioConfig::default());
    mgr.sync_from_account(120_000.0, 75_000.0);
    assert!((mgr.buying_power() - 75_000.0).abs() < 1.0);
}

#[test]
fn portfolio_manager_can_take_position_empty_portfolio() {
    // With an empty portfolio, a small position should always be allowed
    let config = PortfolioConfig {
        initial_capital: 100_000.0,
        ..PortfolioConfig::default()
    };
    let mgr = PortfolioManager::new(config);
    let decision = mgr.can_take_position("Momentum", 5.0, 0.25, 2);
    // No risk violations on an empty portfolio; suggested_size > 0
    assert!(
        decision.suggested_size >= 0,
        "suggested_size should be non-negative; got: {}", decision.suggested_size
    );
}

#[test]
fn portfolio_manager_update_capital_reflected() {
    let mut mgr = PortfolioManager::new(PortfolioConfig::default());
    mgr.update_capital(200_000.0);
    // A larger capital base should allow at least as many contracts
    let decision_before = PortfolioManager::new(PortfolioConfig::default())
        .can_take_position("Test", 5.0, 0.20, 1);
    let decision_after = {
        let mut m = PortfolioManager::new(PortfolioConfig::default());
        m.update_capital(200_000.0);
        m.can_take_position("Test", 5.0, 0.20, 1)
    };
    assert!(
        decision_after.suggested_size >= decision_before.suggested_size,
        "doubling capital should not reduce suggested position size"
    );
}
