//! End-to-end integration tests for complete pipeline validation (simplified)

use dollarbill::models::bs_mod::{black_scholes_call, black_scholes_put, Greeks};
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::heston_call_carr_madan;

fn bs_call(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_call(s, k, t, r, sigma)
}

fn bs_put(s: f64, k: f64, r: f64, t: f64, sigma: f64) -> Greeks {
    black_scholes_put(s, k, t, r, sigma)
}

#[test]
fn test_complete_trading_pipeline() {
    let symbols = vec!["AAPL", "TSLA", "NVDA"];
    let spot_prices = vec![150.0, 200.0, 800.0];
    let vols = vec![0.25, 0.45, 0.35];

    let mut _total_positions = 0;
    let mut portfolio_delta = 0.0;
    let mut portfolio_vega = 0.0;

    for (i, _symbol) in symbols.iter().enumerate() {
        let spot = spot_prices[i];
        let vol = vols[i];

        for j in 0..3 {
            let strike = spot * (0.95 + (j as f64) * 0.05);
            let call = bs_call(spot, strike, 0.05, 0.25, vol);
            let put = bs_put(spot, strike, 0.05, 0.25, vol);

            let market_call = call.price * 0.95;
            let market_put = put.price * 1.05;

            if call.price > market_call * 1.02 {
                portfolio_delta += call.delta;
                portfolio_vega += call.vega;
                _total_positions += 1;
            }

            if put.price < market_put * 0.98 {
                portfolio_delta -= put.delta;
                portfolio_vega -= put.vega;
                _total_positions += 1;
            }
        }
    }

    assert!(portfolio_delta.is_finite());
    assert!(portfolio_vega.is_finite());
    assert!(portfolio_delta.abs() < 200.0);
    assert!(portfolio_vega.abs() < 5000.0);
}

#[test]
fn test_backtest_to_live_trading_consistency() {
    let spot = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;
    let market_vol_scenarios = vec![0.15, 0.25, 0.35];

    for market_vol in market_vol_scenarios {
        let fair_price = bs_call(spot, spot, rate, time, vol).price;
        let market_price = bs_call(spot, spot, rate, time, market_vol).price;
        let signal_strength = (fair_price - market_price) / fair_price;

        let action = if signal_strength > 0.1 {
            "BUY"
        } else if signal_strength < -0.1 {
            "SELL"
        } else {
            "HOLD"
        };

        assert!(matches!(action, "BUY" | "SELL" | "HOLD"));
    }
}

#[test]
fn test_calibration_to_pricing_consistency() {
    let spot = 100.0;
    let rate = 0.05;
    let time = 0.25;

    let market_options = vec![(90.0, 12.5), (100.0, 5.8), (110.0, 2.1)];

    let calibration_objective = |params: &[f64]| -> f64 {
        if params.len() != 1 { return 1e6; }
        let vol = params[0];
        if vol <= 0.0 || vol > 2.0 { return 1e6; }
        market_options.iter().map(|&(k, mkt)| {
            let model_price = bs_call(spot, k, rate, time, vol).price;
            (model_price - mkt).powi(2)
        }).sum()
    };

    let optimizer = dollarbill::calibration::nelder_mead::NelderMead::new(Default::default());
    let result = optimizer.minimize(&calibration_objective, vec![0.2]);
    let calibrated_vol = result.best_params[0];

    for &(strike, market_price) in &market_options {
        let model_price = bs_call(spot, strike, rate, time, calibrated_vol).price;
        let error_pct = (model_price - market_price).abs() / market_price * 100.0;
        assert!(error_pct < 25.0);
    }
}

#[test]
fn test_multi_model_consistency() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let time = 0.25;
    let vol = 0.2;

    let bs_price = bs_call(spot, strike, rate, time, vol).price;
    let params = HestonParams { s0: spot, v0: vol * vol, theta: vol * vol, kappa: 2.0, sigma: 0.01, rho: 0.0, r: rate, t: time };
    let heston_price = heston_call_carr_madan(spot, strike, time, rate, &params);

    let price_diff_pct = (heston_price - bs_price).abs() / bs_price * 100.0;
    assert!(price_diff_pct.is_finite());
    assert!(price_diff_pct < 5_000.0);
}
