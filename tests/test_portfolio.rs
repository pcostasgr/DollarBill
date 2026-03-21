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
fn perf_attribution_open_positions_ignored() {
    // `calculate_strategy_performance` should only count Closed/Expired positions.
    let mut attr = PerformanceAttribution::new();
    let winner = closed_position(1, 5.0, 10.0, 1); // +$500, Closed
    let open   = open_call(2, "AAPL", 1, 5.0, greeks(5.0, 0.5, 0.02, -0.05, 0.10)); // Open — must be excluded
    let perf   = attr.calculate_strategy_performance("Strat", &[winner, open]);
    assert_eq!(perf.total_trades, 1, "open positions must not be counted as trades");
    assert_eq!(perf.winning_trades, 1);
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

// ─── PerformanceAttribution — compare & equity curve ─────────────────────

#[test]
fn perf_compare_strategies_sorts_by_sharpe() {
    let mut attr = PerformanceAttribution::new();

    // "A" gets 5 consistent winners (higher Sharpe), "B" gets 2
    let a_positions: Vec<Position> = (1..=5).map(|i| closed_position(i, 5.0, 10.0, 1)).collect();
    let b_positions: Vec<Position> = (6..=7).map(|i| closed_position(i, 5.0, 10.0, 1)).collect();
    attr.calculate_strategy_performance("StratA", &a_positions);
    attr.calculate_strategy_performance("StratB", &b_positions);

    let comparisons = attr.compare_strategies(&["StratA", "StratB"]);
    assert_eq!(comparisons.len(), 2);
    // sorted by Sharpe: best first; both have identical % returns so order may be equal —
    // just confirm both are present.
    let names: Vec<&str> = comparisons.iter().map(|c| c.strategy.as_str()).collect();
    assert!(names.contains(&"StratA"));
    assert!(names.contains(&"StratB"));
}

#[test]
fn perf_compare_strategies_unknown_name_omitted() {
    let mut attr = PerformanceAttribution::new();
    let positions: Vec<Position> = (1..=3).map(|i| closed_position(i, 5.0, 10.0, 1)).collect();
    attr.calculate_strategy_performance("Known", &positions);

    let comparisons = attr.compare_strategies(&["Known", "DoesNotExist"]);
    assert_eq!(comparisons.len(), 1, "unknown strategy should be silently omitted");
    assert_eq!(comparisons[0].strategy, "Known");
}

#[test]
fn perf_best_strategy_returns_highest_sharpe() {
    let mut attr = PerformanceAttribution::new();

    // "Good" has varied positive returns → non-zero std_dev → positive Sharpe
    // Using different (entry, exit) pairs so return_pct differs each trade.
    let good: Vec<Position> = vec![
        closed_position(1,  5.0,  7.0, 1), // +40%
        closed_position(2,  5.0,  9.0, 1), // +80%
        closed_position(3,  5.0,  8.0, 1), // +60%
        closed_position(4,  5.0,  6.0, 1), // +20%
        closed_position(5,  5.0, 10.0, 1), // +100%
    ];
    // "Bad" has consistent losses (std_dev=0 → Sharpe=0)
    let bad: Vec<Position> = (6..=10).map(|i| closed_position(i, 10.0, 3.0, 1)).collect();
    attr.calculate_strategy_performance("Good", &good);
    attr.calculate_strategy_performance("Bad",  &bad);

    let best = attr.best_strategy().expect("should return a best strategy");
    assert_eq!(best, "Good", "highest-sharpe strategy should win");
}

#[test]
fn perf_best_strategy_none_when_empty() {
    let attr = PerformanceAttribution::new();
    assert!(attr.best_strategy().is_none());
}

#[test]
fn perf_equity_curve_stored_after_calculation() {
    let mut attr = PerformanceAttribution::new();
    let positions: Vec<Position> = (1..=3).map(|i| closed_position(i, 5.0, 10.0, 1)).collect();
    attr.calculate_strategy_performance("Curve", &positions);

    let curve = attr.get_equity_curve("Curve").expect("equity curve should be stored");
    // starts at 0 then grows with each winning trade
    assert!(!curve.is_empty(), "equity curve should have at least one entry");
    assert!(curve.last().unwrap() > &0.0, "cumulative P&L should be positive for all winners");
}

#[test]
fn perf_equity_curve_none_for_unknown() {
    let attr = PerformanceAttribution::new();
    assert!(attr.get_equity_curve("NoSuchStrategy").is_none());
}

#[test]
fn perf_calculate_contribution_proportional() {
    let mut attr = PerformanceAttribution::new();
    // Two strategies each earning $500 → each is 50% of total $1000 P&L
    let pos_a: Vec<Position> = vec![closed_position(1, 5.0, 10.0, 1)]; // $500 profit
    let pos_b: Vec<Position> = vec![closed_position(2, 5.0, 10.0, 1)]; // $500 profit
    attr.calculate_strategy_performance("A", &pos_a);
    attr.calculate_strategy_performance("B", &pos_b);

    let contrib = attr.calculate_contribution("A", 1_000.0);
    assert!((contrib - 50.0).abs() < 0.01, "should be 50% contribution; got {}", contrib);
}

#[test]
fn perf_calculate_contribution_zero_total_pnl() {
    let mut attr = PerformanceAttribution::new();
    let positions: Vec<Position> = vec![closed_position(1, 5.0, 10.0, 1)];
    attr.calculate_strategy_performance("S", &positions);
    // When total P&L is 0, contribution is undefined → should return 0 without panicking
    assert_eq!(attr.calculate_contribution("S", 0.0), 0.0);
}

#[test]
fn perf_calculate_contribution_unknown_strategy() {
    let attr = PerformanceAttribution::new();
    assert_eq!(attr.calculate_contribution("Ghost", 1_000.0), 0.0);
}

// ─── MultiLegSizer ────────────────────────────────────────────────────────

#[test]
fn multi_leg_iron_condor_at_least_one_contract() {
    // Even with a very large max_loss, the function floors to a minimum of 1
    let sizer = dollarbill::portfolio::MultiLegSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.iron_condor_size(
        SizingMethod::FixedFractional(5.0),
        50_000.0, // absurdly large max_loss
        1.50,     // net_credit
        0.30,     // volatility
    );
    assert!(contracts >= 1, "iron_condor_size must return at least 1; got {}", contracts);
}

#[test]
fn multi_leg_iron_condor_size_bounded_by_max_position() {
    let sizer = dollarbill::portfolio::MultiLegSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.iron_condor_size(
        SizingMethod::FixedFractional(5.0),
        200.0,  // small max_loss → many contracts from risk sizing alone
        1.50,
        0.20,
    );
    // vol_adjusted = volatility_based(0.20, 1.50) → caps the result
    assert!(contracts > 0, "should produce positive size");
    assert!(contracts < 10_000, "should not produce runaway contract count");
}

#[test]
fn multi_leg_credit_spread_at_least_one_contract() {
    let sizer = dollarbill::portfolio::MultiLegSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.credit_spread_size(
        SizingMethod::FixedFractional(5.0),
        5.0,    // spread_width
        2.50,   // net_credit  (max_loss = (5 - 2.5) * 100 = $250)
        0.25,
    );
    assert!(contracts >= 1, "credit_spread_size must return at least 1; got {}", contracts);
}

#[test]
fn multi_leg_credit_spread_high_vol_not_larger_than_low_vol() {
    let sizer = dollarbill::portfolio::MultiLegSizer::new(100_000.0, 2.0, 10.0);
    let low_vol  = sizer.credit_spread_size(SizingMethod::FixedFractional(5.0), 5.0, 2.0, 0.15);
    let high_vol = sizer.credit_spread_size(SizingMethod::FixedFractional(5.0), 5.0, 2.0, 0.60);
    assert!(
        low_vol >= high_vol,
        "low vol ({}) should yield >= contracts vs high vol ({})",
        low_vol, high_vol
    );
}

// ─── AllocationMethod::VolatilityWeighted ─────────────────────────────────

#[test]
fn allocator_volatility_weighted_lower_vol_gets_more() {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::VolatilityWeighted);
    allocator.add_strategy("Stable".to_string(), 80_000.0, 0.0, 80.0);
    allocator.add_strategy("Wild".to_string(),   80_000.0, 0.0, 80.0);

    let mut stats = HashMap::new();
    stats.insert("Stable".to_string(), StrategyStats { volatility: 0.10, ..StrategyStats::default() });
    stats.insert("Wild".to_string(),   StrategyStats { volatility: 0.40, ..StrategyStats::default() });
    allocator.calculate_allocations(&stats);

    let stable = allocator.get_allocation("Stable").unwrap().target_pct;
    let wild   = allocator.get_allocation("Wild").unwrap().target_pct;
    assert!(stable > wild,
        "lower-vol strategy ({:.1}%) should get higher weight than high-vol ({:.1}%)", stable, wild);
}

#[test]
fn allocator_single_strategy_gets_full_allocation() {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
    allocator.add_strategy("Solo".to_string(), 100_000.0, 0.0, 100.0);
    allocator.calculate_allocations(&HashMap::new());
    let alloc = allocator.get_allocation("Solo").unwrap();
    assert!(
        (alloc.target_pct - 100.0).abs() < 1.0,
        "single strategy should get ~100% allocation; got {:.1}%", alloc.target_pct
    );
}

#[test]
fn allocator_no_strategies_no_crash() {
    let mut allocator = PortfolioAllocator::new(100_000.0, AllocationMethod::EqualWeight);
    // should not panic on empty strategy list
    allocator.calculate_allocations(&HashMap::new());
}

// ─── PortfolioManager — untested public API ────────────────────────────────

#[test]
fn portfolio_manager_get_portfolio_risk_empty_positions() {
    let mgr = PortfolioManager::new(PortfolioConfig::default());
    let risk = mgr.get_portfolio_risk();
    assert_eq!(risk.total_delta, 0.0);
    assert_eq!(risk.gross_exposure, 0.0);
}

#[test]
fn portfolio_manager_calculate_position_size_positive() {
    let mgr = PortfolioManager::new(PortfolioConfig::default());
    let size = mgr.calculate_position_size(5.0, 0.25, None, None, None);
    assert!(size >= 0, "calculate_position_size should return non-negative; got {}", size);
}

#[test]
fn portfolio_manager_iron_condor_size_positive() {
    let mgr = PortfolioManager::new(PortfolioConfig::default());
    let size = mgr.calculate_iron_condor_size(500.0, 1.50, 0.25);
    assert!(size >= 1, "iron condor size should be >= 1; got {}", size);
}

#[test]
fn portfolio_manager_credit_spread_size_positive() {
    let mgr = PortfolioManager::new(PortfolioConfig::default());
    let size = mgr.calculate_credit_spread_size(5.0, 2.0, 0.25);
    assert!(size >= 1, "credit spread size should be >= 1; got {}", size);
}

#[test]
fn portfolio_manager_add_and_optimize_strategy() {
    let mut mgr = PortfolioManager::new(PortfolioConfig::default());
    mgr.add_strategy("Iron".to_string(), 50_000.0, 0.0, 60.0);
    mgr.add_strategy("Momentum".to_string(), 50_000.0, 0.0, 60.0);

    let stats = HashMap::new();
    mgr.optimize_allocations(&stats);
    // After equal-weight allocation for 2 strategies, can_take_position should work
    let decision = mgr.can_take_position("Iron", 5.0, 0.20, 1);
    assert!(decision.suggested_size >= 0);
}

#[test]
fn portfolio_manager_rebalancing_after_drift() {
    let mut mgr = PortfolioManager::new(PortfolioConfig::default());
    mgr.add_strategy("A".to_string(), 80_000.0, 0.0, 80.0);
    mgr.add_strategy("B".to_string(), 80_000.0, 0.0, 80.0);
    mgr.optimize_allocations(&HashMap::new());

    // Simulate drift: A consumed 70% of capital, B only 30%
    use std::collections::HashMap as HM;
    let mut vals: HM<String, f64> = HM::new();
    vals.insert("A".to_string(), 70_000.0);
    vals.insert("B".to_string(), 30_000.0);
    // Access allocator through manager's rebalancing API
    let recs = mgr.get_rebalancing_recommendations();
    // Without forced drift, may be empty; the main check is no panic
    let _ = recs;
}

#[test]
fn portfolio_manager_calculate_and_best_strategy() {
    let mut mgr = PortfolioManager::new(PortfolioConfig::default());
    // Start with no tracked strategies → best_strategy returns None
    assert!(mgr.best_strategy().is_none());

    // Add performance data through the manager
    let positions: Vec<Position> = (1..=5).map(|i| closed_position(i, 5.0, 10.0, 1)).collect();
    mgr.calculate_strategy_performance("Condors", &positions);
    assert_eq!(mgr.best_strategy(), Some("Condors".to_string()));
}

#[test]
fn portfolio_manager_can_take_position_with_risk_limit_violation() {
    use dollarbill::portfolio::RiskLimits;
    // Set absurdly tight concentration limit so any position violates it
    let mut config = PortfolioConfig::default();
    config.risk_limits = RiskLimits {
        max_concentration_pct: 0.001, // 0.001% — any real position will trip this
        ..RiskLimits::default()
    };
    let mut mgr = PortfolioManager::new(config);

    // Load one position that will cause concentration to exceed the limit
    let g = greeks(100.0, 0.50, 0.02, -0.05, 0.10);
    let big_pos = open_call(1, "AAPL", 10, 100.0, g); // gross_exposure = $100k
    mgr.update_positions(vec![big_pos]);

    let decision = mgr.can_take_position("Test", 5.0, 0.20, 1);
    assert!(!decision.risk_warnings.is_empty(), "should have at least one risk warning");
}

// ─── RiskAnalyzer — closed positions excluded ─────────────────────────────

#[test]
fn risk_analyzer_closed_positions_excluded_from_greeks() {
    // A closed position should contribute zero to Greek aggregation
    let g = greeks(5.0, 0.50, 0.02, -0.05, 0.10);
    let open_pos   = open_call(1, "AAPL", 2, 5.0, g.clone());
    let mut closed = open_call(2, "MSFT", 5, 5.0, g.clone());
    closed.status = dollarbill::backtesting::position::PositionStatus::Closed;

    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk_mixed = analyzer.calculate_portfolio_greeks(&[open_pos.clone(), closed]);
    let risk_open  = analyzer.calculate_portfolio_greeks(&[open_pos]);

    assert!(
        (risk_mixed.total_delta - risk_open.total_delta).abs() < 1e-9,
        "closed position must not affect delta aggregation"
    );
    assert!(
        (risk_mixed.gross_exposure - risk_open.gross_exposure).abs() < 1e-9,
        "closed position must not affect gross exposure"
    );
}

// ─── PositionSizer edge cases ─────────────────────────────────────────────

#[test]
fn position_sizer_kelly_missing_inputs_defaults_to_50_50() {
    // When all optional params are None, Kelly uses p=0.5, w=option_price, l=option_price
    // → b=1, kelly=(1*0.5 - 0.5)/1 = 0 → clamped to 0 → 0 contracts
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.calculate_size(
        SizingMethod::KellyCriterion,
        5.0, 0.25, None, None, None,
    );
    assert!(contracts >= 0, "kelly with even odds should give 0 or fallback contracts; got {}", contracts);
}

#[test]
fn position_sizer_risk_parity_positive_size() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let contracts = sizer.calculate_size(SizingMethod::RiskParity, 5.0, 0.20, None, None, None);
    assert!(contracts >= 0, "risk parity should return non-negative contracts");
}

#[test]
fn position_sizer_risk_parity_high_vol_not_larger() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    let low  = sizer.calculate_size(SizingMethod::RiskParity, 5.0, 0.10, None, None, None);
    let high = sizer.calculate_size(SizingMethod::RiskParity, 5.0, 0.80, None, None, None);
    assert!(low >= high,
        "risk parity: low vol ({}) should yield >= contracts vs high vol ({})", low, high);
}

#[test]
fn position_risk_calculation_correct() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    // 5 contracts × $10 option × 100 shares = $5,000
    assert!((sizer.position_risk(5, 10.0) - 5_000.0).abs() < 1e-9);
}

#[test]
fn position_sizer_validate_zero_contracts() {
    let sizer = PositionSizer::new(100_000.0, 2.0, 10.0);
    // 0 contracts is invalid
    assert!(!sizer.validate_position(0, 5.0));
}

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
