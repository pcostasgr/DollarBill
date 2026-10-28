use dollarbill::models::{bs_mod::*, american::*};
use dollarbill::portfolio::{PositionSizer, MultiLegSizer, SizingMethod, RiskAnalyzer, RiskLimits, PerformanceAttribution};
use dollarbill::analysis::advanced_classifier::MarketRegime;
use dollarbill::backtesting::position::{Position, OptionType};

fn position(id: usize, qty: i32, spot: f64) -> Position {
    Position::new(id, "AUDIT".into(), OptionType::Call, ExerciseStyle::European,
        spot, qty, 2.0, "2026-01-01".into(), spot,
        Some(Greeks { price: 2.0, delta: 0.5, gamma: 0.01, vega: 10.0, theta: -1.0, rho: 0.0 }))
}

#[test]
fn pricing_expiry_and_zero_volatility_preserve_intrinsic_and_parity() {
    for (s, k, r, q) in [(120.0,100.0,0.05,0.0),(100.0,100.0,0.0,0.0),(80.0,100.0,-0.05,0.02)] {
        let call=black_scholes_merton_call(s,k,1.0,r,0.0,q);
        let put=black_scholes_merton_put(s,k,1.0,r,0.0,q);
        assert!((call.price-put.price-(s*(-q).exp()-k*(-r).exp())).abs()<1e-10);
        assert!([call.price,call.delta,call.gamma,put.price,put.delta,put.gamma].iter().all(|v|v.is_finite()));
        let cfg=BinomialConfig {use_dividends:true,dividend_yield:q,..Default::default()};
        assert!((american_call_binomial(s,k,0.0,r,0.2,&cfg)-(s-k).max(0.0)).abs()<1e-10);
        assert!(american_call_binomial(s,k,1.0,r,0.0,&cfg)>=(s-k).max(0.0));
        assert!(american_put_binomial(s,k,1.0,r,0.0,&cfg)>=(k-s).max(0.0));
        assert!((european_call_binomial(s,k,1.0,r,0.0,&cfg)-call.price).abs()<1e-10);
    }
    assert!(black_scholes_call(-1.0,100.0,1.0,0.05,0.2).price.is_nan());
    assert!(american_call_binomial(100.0,100.0,1.0,0.05,0.2,&BinomialConfig {n_steps:0,..Default::default()}).is_nan());
}

#[test]
fn volatility_requires_two_returns_and_valid_observations() {
    assert_eq!(compute_historical_vol(&[100.0,101.0]),0.0);
    assert!(compute_historical_vol(&[100.0,0.0,101.0]).is_nan());
    let now=std::time::Instant::now();
    assert_eq!(compute_historical_vol_ticks(&[(100.0,now),(101.0,now)]),0.0);
    assert!(compute_historical_vol_ticks(&[(100.0,now),(101.0,now),(102.0,now)]).is_nan());
}

#[test]
fn sizing_caps_apply_after_regime_and_unaffordable_means_zero() {
    let sizer=PositionSizer::new(100_000.0,2.0,10.0);
    assert_eq!(sizer.calculate_size_with_regime(SizingMethod::FixedFractional(10.0),1.0,0.1,None,None,None,&MarketRegime::LowVol),100);
    for price in [-1.0,0.0,f64::NAN,f64::INFINITY] {
        assert!(!sizer.validate_position(1,price));
        assert_eq!(sizer.calculate_size(SizingMethod::FixedFractional(5.0),price,0.2,None,None,None),0);
    }
    let multi=MultiLegSizer::new(100_000.0,2.0,10.0);
    assert_eq!(multi.iron_condor_size(SizingMethod::FixedFractional(5.0),50_000.0,1.5,0.3),0);
    assert_eq!(multi.credit_spread_size(SizingMethod::FixedFractional(5.0),500.0,1.5,0.3),0);
}

#[test]
fn risk_uses_spot_dollars_and_nets_identical_signed_factors() {
    let analyzer=RiskAnalyzer::new(100_000.0,RiskLimits::default());
    let low=analyzer.calculate_portfolio_greeks(&[position(1,1,100.0)]);
    let high=analyzer.calculate_portfolio_greeks(&[position(1,1,1000.0)]);
    assert_eq!(low.net_exposure,5000.0);
    assert_eq!(high.net_exposure,50_000.0);
    assert!(!analyzer.check_risk_limits(&high).is_empty());
    let offset=analyzer.calculate_portfolio_greeks(&[position(1,1,100.0),position(2,-1,100.0)]);
    assert_eq!(offset.net_exposure,0.0);
    assert_eq!(offset.var_95,0.0);
    assert_eq!(offset.gamma_loss_1pct,0.0);
    let mut missing=position(1,1,100.0); missing.entry_greeks=None;
    assert!(!analyzer.check_risk_limits(&analyzer.calculate_portfolio_greeks(&[missing])).is_empty());
}

#[test]
fn attribution_uses_capital_exit_order_and_zero_hurdle_trade_ratios() {
    let mut win=position(1,1,100.0); win.close(3.0,"2026-01-02".into(),100.0,1);
    let mut loss=position(2,1,100.0); loss.close(1.5,"2026-01-03".into(),100.0,1);
    let mut attr=PerformanceAttribution::with_initial_capital(10_000.0);
    let perf=attr.calculate_strategy_performance("audit",&[loss,win.clone()]);
    assert!((perf.max_drawdown_pct-50.0/10100.0*100.0).abs()<1e-10);
    assert!((perf.roi-0.5).abs()<1e-10);
    assert_eq!(attr.get_equity_curve("audit").unwrap(),&vec![10_000.0,10_100.0,10_050.0]);
    let winner=attr.calculate_strategy_performance("winner",&[win]);
    assert!(winner.profit_factor.is_infinite());
    assert!(winner.sharpe_ratio.is_infinite());
    assert!(winner.sortino_ratio.is_infinite());
}

#[test]
fn historical_engine_normalizes_chronology_before_callbacks() {
    use dollarbill::backtesting::engine::{BacktestConfig,BacktestEngine};
    use dollarbill::market_data::csv_loader::HistoricalDay;
    let days: Vec<_>=(1..=60).map(|day| HistoricalDay {
        date:(chrono::NaiveDate::from_ymd_opt(2026,1,1).unwrap()+chrono::Duration::days(day)).to_string(),
        close:100.0+day as f64,
    }).rev().collect();
    let mut engine=BacktestEngine::new(BacktestConfig::default());
    let mut last=0.0;
    engine.run_with_signals("TEST",days,|_,spot,_,past| {
        assert!(spot>=last); last=spot;
        assert!(!past.is_empty()); Vec::new()
    });
    assert!(last>100.0);
}
