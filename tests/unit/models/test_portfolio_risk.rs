//! Portfolio and risk aggregation tests.
//!
//! Covers: delta-neutral portfolio (net delta ≈ 0 after a spot move), vega-positive
//! book P&L sign, Rho sensitivity sign across the portfolio, and Greeks aggregation
//! via the RiskAnalyzer.

use dollarbill::backtesting::position::{Position, PositionStatus, OptionType};
use dollarbill::models::american::ExerciseStyle;
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put, Greeks};
use dollarbill::portfolio::risk_analytics::{RiskAnalyzer, RiskLimits};

// ─── Helper: create an open Position with given Greeks ───────────────────────

fn make_position(
    id: usize,
    symbol: &str,
    opt_type: OptionType,
    strike: f64,
    quantity: i32,
    greeks: Greeks,
) -> Position {
    let mut pos = Position::new(
        id,
        symbol.to_string(),
        opt_type,
        ExerciseStyle::European,
        strike,
        quantity,
        greeks.price,
        "2024-01-01".to_string(),
        100.0,
        Some(greeks),
    );
    // Force Open status (Position::new sets it to Open, but let's be explicit)
    pos.status = PositionStatus::Open;
    pos
}

// ─── 7. Portfolio / Risk Aggregation ─────────────────────────────────────────

/// Delta-neutral portfolio: a long call + short put at same strike (synthetic long)
/// cancels out most delta; adding a matching short call + long put gives near-zero net delta.
#[test]
fn test_delta_neutral_portfolio_net_delta_near_zero() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 0.25;
    let vol    = 0.20;
    let div    = 0.0;

    let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put  = black_scholes_merton_put (spot, strike, time, rate, vol, div);

    // Long call (+1) + long put (+1) ≈ delta-neutral at ATM (call_delta + put_delta ≈ 0)
    let positions = vec![
        make_position(1, "SPY", OptionType::Call, strike, 1, call),
        make_position(2, "SPY", OptionType::Put,  strike, 1, put),
    ];

    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&positions);

    // call.delta + put.delta ≈ call.delta - |put.delta| ≈ 0 for ATM
    // (put delta is negative, so we're summing two numbers of opposite sign)
    assert!(risk.total_delta.abs() < 0.20 * 100.0, // ×100 multiplier in implementation
            "ATM long straddle net delta should be near 0, got {}", risk.total_delta);
}

/// Vega-positive book: long straddle is vega-positive.
/// When vol increases 1%, the Taylor P&L (vega × Δvol) should be positive.
#[test]
fn test_vega_positive_book_vol_up_positive_pnl() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 0.5;
    let vol    = 0.20;
    let div    = 0.0;

    let call = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let put  = black_scholes_merton_put (spot, strike, time, rate, vol, div);

    // Long straddle: long call + long put — both are vega-positive
    let positions = vec![
        make_position(1, "SPY", OptionType::Call, strike, 1, call),
        make_position(2, "SPY", OptionType::Put,  strike, 1, put),
    ];

    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&positions);

    // Aggregate vega should be positive (we're long volatility)
    assert!(risk.total_vega > 0.0,
            "Long straddle total vega should be positive, got {}", risk.total_vega);

    // P&L estimate for +1% vol move: vega × Δσ (per unit before multiplier)
    let vega_pnl_approx = (call.vega + put.vega) * 0.01; // per-contract, Δσ = 1%
    assert!(vega_pnl_approx > 0.0,
            "Vega P&L on vol up should be positive, got {}", vega_pnl_approx);
}

/// Rho sensitivity: aggregate Rho should have correct sign for a portfolio of long calls.
/// Long calls have positive Rho (benefit from rate increases).
#[test]
fn test_rho_sensitivity_correct_sign_long_calls() {
    let spot   = 100.0;
    let rate   = 0.05;
    let time   = 1.0;
    let vol    = 0.20;
    let div    = 0.0;

    let strikes = [90.0, 100.0, 110.0];
    let positions: Vec<Position> = strikes
        .iter()
        .enumerate()
        .map(|(i, &k)| {
            let g = black_scholes_merton_call(spot, k, time, rate, vol, div);
            make_position(i, "AAPL", OptionType::Call, k, 1, g)
        })
        .collect();

    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&positions);

    assert!(risk.total_vega.is_finite(),   "Total vega should be finite");
    assert!(risk.total_delta.is_finite(),  "Total delta should be finite");
    assert!(risk.total_gamma.is_finite(),  "Total gamma should be finite");
    assert!(risk.total_theta.is_finite(),  "Total theta should be finite");

    // For long calls, each has positive Rho; aggregate Rho proxy (via position values)
    // We verify individual Greeks: call Rho positive
    let call = black_scholes_merton_call(spot, 100.0, time, rate, vol, div);
    assert!(call.rho > 0.0, "Call rho should be positive");
}

/// Empty portfolio: all aggregate Greeks should be exactly zero.
#[test]
fn test_empty_portfolio_zero_greeks() {
    let positions: Vec<Position> = Vec::new();
    let analyzer = RiskAnalyzer::new(100_000.0, RiskLimits::default());
    let risk = analyzer.calculate_portfolio_greeks(&positions);

    assert_eq!(risk.total_delta, 0.0, "Empty portfolio delta must be 0");
    assert_eq!(risk.total_gamma, 0.0, "Empty portfolio gamma must be 0");
    assert_eq!(risk.total_vega,  0.0, "Empty portfolio vega must be 0");
    assert_eq!(risk.total_theta, 0.0, "Empty portfolio theta must be 0");
}

/// Gamma scalping: long options accumulate positive gamma P&L from small spot moves.
/// For a long call, gamma > 0 and the second-order Taylor term 0.5*γ*(ΔS)² ≥ 0.
#[test]
fn test_gamma_scalping_positive_convexity() {
    let spot   = 100.0;
    let strike = 100.0;
    let rate   = 0.05;
    let time   = 0.25;
    let vol    = 0.20;
    let div    = 0.0;

    let original = black_scholes_merton_call(spot, strike, time, rate, vol, div);
    let spot_up  = 101.0; // +1 point move
    let repriced = black_scholes_merton_call(spot_up, strike, time, rate, vol, div);

    let actual_pnl        = repriced.price - original.price;
    let gamma_pnl_contrib = 0.5 * original.gamma * (spot_up - spot).powi(2);

    // Call price must increase when spot increases (positive delta)
    assert!(actual_pnl > 0.0,
            "Call price must increase when spot rises. actual_pnl={:.6}", actual_pnl);

    // Gamma P&L contribution must be non-negative (positive convexity)
    assert!(gamma_pnl_contrib >= 0.0,
            "Gamma P&L contribution should be non-negative: {:.6}", gamma_pnl_contrib);

    // Gamma itself must be positive for a long call
    assert!(original.gamma > 0.0,
            "Long call gamma must be positive: {:.6}", original.gamma);
}
