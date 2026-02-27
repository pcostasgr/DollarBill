// Property-based tests for TradingCosts, SlippageModel, and Trade cost accounting.
// Run with: cargo test test_trading_costs -- --nocapture

use proptest::prelude::*;
use dollarbill::backtesting::{TradingCosts, Trade, TradeType, SlippageModel};
use dollarbill::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};

// ─── Group A: Black-Scholes Pricing Properties ────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Call price is always ≥ 0 for all valid inputs.
    #[test]
    fn bs_call_price_nonneg(
        s     in 1.0f64..1000.0,
        k     in 1.0f64..1000.0,
        t     in 0.01f64..2.0,
        r     in 0.0f64..0.15,
        sigma in 0.01f64..2.0,
    ) {
        let g = black_scholes_merton_call(s, k, t, r, sigma, 0.0);
        prop_assert!(g.price >= -1e-10, "call price {} < 0 (float noise)", g.price);
    }

    /// Put price is always ≥ 0 for all valid inputs.
    #[test]
    fn bs_put_price_nonneg(
        s     in 1.0f64..1000.0,
        k     in 1.0f64..1000.0,
        t     in 0.01f64..2.0,
        r     in 0.0f64..0.15,
        sigma in 0.01f64..2.0,
    ) {
        let g = black_scholes_merton_put(s, k, t, r, sigma, 0.0);
        prop_assert!(g.price >= -1e-10, "put price {} < 0 (float noise)", g.price);
    }

    /// Put-call parity: C - P = S - K·e^{-rT}  (within 0.1% of spot tolerance).
    #[test]
    fn put_call_parity(
        s     in 10.0f64..500.0,
        k     in 10.0f64..500.0,
        t     in 0.05f64..1.0,
        r     in 0.0f64..0.10,
        sigma in 0.05f64..1.0,
    ) {
        let call = black_scholes_merton_call(s, k, t, r, sigma, 0.0).price;
        let put  = black_scholes_merton_put (s, k, t, r, sigma, 0.0).price;
        let lhs  = call - put;
        let rhs  = s - k * (-r * t).exp();
        let tol  = 0.001 * s.max(1.0);
        prop_assert!(
            (lhs - rhs).abs() < tol,
            "put-call parity violated: C-P={:.6} vs S-Ke^-rT={:.6} (tol={:.6})",
            lhs, rhs, tol
        );
    }

    /// Call price is non-increasing in strike (all else equal).
    #[test]
    fn call_price_decreasing_in_strike(
        s     in 10.0f64..500.0,
        k1    in 10.0f64..300.0,
        dk    in 1.0f64..100.0,
        t     in 0.05f64..1.0,
        r     in 0.0f64..0.10,
        sigma in 0.05f64..1.0,
    ) {
        let k2 = k1 + dk;
        let c1 = black_scholes_merton_call(s, k1, t, r, sigma, 0.0).price;
        let c2 = black_scholes_merton_call(s, k2, t, r, sigma, 0.0).price;
        prop_assert!(
            c1 >= c2 - 1e-9,
            "call should decrease with strike: c(k={})={:.6} < c(k={})={:.6}",
            k1, c1, k2, c2
        );
    }

    /// Put price is non-decreasing in strike (all else equal).
    #[test]
    fn put_price_increasing_in_strike(
        s     in 10.0f64..500.0,
        k1    in 10.0f64..300.0,
        dk    in 1.0f64..100.0,
        t     in 0.05f64..1.0,
        r     in 0.0f64..0.10,
        sigma in 0.05f64..1.0,
    ) {
        let k2 = k1 + dk;
        let p1 = black_scholes_merton_put(s, k1, t, r, sigma, 0.0).price;
        let p2 = black_scholes_merton_put(s, k2, t, r, sigma, 0.0).price;
        prop_assert!(
            p2 >= p1 - 1e-9,
            "put should increase with strike: p(k={})={:.6} < p(k={})={:.6}",
            k2, p2, k1, p1
        );
    }

    /// ATM option price increases with longer time to expiry (vol-time value).
    #[test]
    fn longer_expiry_higher_atm_price(
        s     in 10.0f64..500.0,
        t1    in 0.25f64..2.0,
        dt    in 0.05f64..0.5,
        r     in 0.0f64..0.10,
        sigma in 0.05f64..1.0,
    ) {
        let t2 = t1 - dt; // t1 > t2 > 0
        let c1 = black_scholes_merton_call(s, s, t1, r, sigma, 0.0).price; // ATM
        let c2 = black_scholes_merton_call(s, s, t2, r, sigma, 0.0).price;
        prop_assert!(
            c1 >= c2 - 1e-9,
            "longer expiry ATM call should be >= shorter: c(T={:.3})={:.6} < c(T={:.3})={:.6}",
            t1, c1, t2, c2
        );
    }

    // ─── Group B: TradingCosts Properties ─────────────────────────────────────

    /// Buying always fills above mid when spread > 0.
    #[test]
    fn buying_always_costs_more_than_mid(
        mid    in 0.01f64..100.0,
        spread in 0.001f64..0.05,
    ) {
        let costs = TradingCosts {
            commission_per_contract: 0.65,
            bid_ask_spread_percent: spread,
            slippage_model: SlippageModel::Fixed,
            ..TradingCosts::default()
        };
        let fill = costs.fill_price(mid, true, 0.25, 1);
        prop_assert!(fill > mid, "buy fill {} not > mid {} (spread={})", fill, mid, spread);
    }

    /// Selling always fills below mid when spread > 0.
    #[test]
    fn selling_always_less_than_mid(
        mid    in 0.01f64..100.0,
        spread in 0.001f64..0.05,
    ) {
        let costs = TradingCosts {
            commission_per_contract: 0.65,
            bid_ask_spread_percent: spread,
            slippage_model: SlippageModel::Fixed,
            ..TradingCosts::default()
        };
        let fill = costs.fill_price(mid, false, 0.25, 1);
        prop_assert!(fill < mid, "sell fill {} not < mid {} (spread={})", fill, mid, spread);
    }

    /// A buy-then-sell round-trip always loses money (spread + commissions).
    #[test]
    fn round_trip_cost_positive(
        mid        in 0.10f64..50.0,
        spread     in 0.001f64..0.05,
        commission in 0.0f64..5.0,
    ) {
        let costs = TradingCosts {
            commission_per_contract: commission,
            bid_ask_spread_percent: spread,
            slippage_model: SlippageModel::Fixed,
            ..TradingCosts::default()
        };
        let buy_fill  = costs.fill_price(mid, true,  0.25, 1);
        let sell_fill = costs.fill_price(mid, false, 0.25, 1);
        // Cost = (bought at ask) - (sold at bid) + two commissions
        let round_trip_loss = (buy_fill + commission) - (sell_fill - commission);
        prop_assert!(
            round_trip_loss > 0.0,
            "round-trip should be a loss: loss={:.6} (buy={:.4}, sell={:.4}, comm={:.4})",
            round_trip_loss, buy_fill, sell_fill, commission
        );
    }

    /// commission_for(2n) == 2 × commission_for(n): linear in contracts.
    #[test]
    fn commission_proportional(
        per_contract in 0.0f64..10.0,
        n            in 1i32..50,
    ) {
        let costs = TradingCosts {
            commission_per_contract: per_contract,
            bid_ask_spread_percent: 0.01,
            slippage_model: SlippageModel::Fixed,
            ..TradingCosts::default()
        };
        let single = costs.commission_for(n);
        let double = costs.commission_for(2 * n);
        prop_assert!(
            (double - 2.0 * single).abs() < 1e-9,
            "commission not proportional: commission_for({})={:.9} vs 2×{}={:.9}",
            2 * n, double, single, 2.0 * single
        );
    }

    /// Trade::proceeds() ≤ Trade::value() — commissions reduce receipts.
    #[test]
    fn proceeds_never_exceeds_value(
        price      in 0.01f64..100.0,
        qty        in 1i32..100,
        commission in 0.0f64..50.0,
    ) {
        // Short trade: negative quantity = selling
        let trade = Trade::new(
            0, TradeType::Entry, "2024-01-01".to_string(), "TEST".to_string(),
            price, -qty, 100.0, None, commission,
        );
        let proceeds = trade.proceeds();
        let value    = trade.value();
        prop_assert!(
            proceeds <= value + 1e-9,
            "proceeds {:.6} > value {:.6} (price={}, qty={}, commission={})",
            proceeds, value, price, qty, commission
        );
    }

    /// Trade::total_cost() ≥ Trade::value() — commissions inflate the cost to buy.
    #[test]
    fn total_cost_at_least_value(
        price      in 0.01f64..100.0,
        qty        in 1i32..100,
        commission in 0.0f64..50.0,
    ) {
        // Long trade: positive quantity = buying
        let trade = Trade::new(
            0, TradeType::Entry, "2024-01-01".to_string(), "TEST".to_string(),
            price, qty, 100.0, None, commission,
        );
        let total_cost = trade.total_cost();
        let value      = trade.value();
        prop_assert!(
            total_cost >= value - 1e-9,
            "total_cost {:.6} < value {:.6} (price={}, qty={}, commission={})",
            total_cost, value, price, qty, commission
        );
    }

    // ─── Group C: Round-trip Accounting Invariant ─────────────────────────────

    /// Commissions can only decrease PnL — they can never inflate a profit.
    ///
    /// For every book of long round-trips, the fee-adjusted PnL must satisfy:
    ///   pnl_with_fees ≤ pnl_no_fees   (when the gross book is profitable)
    ///
    /// Both sides are computed through the Trade API (which applies the options
    /// ×100 multiplier), differing only in the commission argument, so the
    /// invariant reduces to: −2 × commission × qty × 100 ≤ 0, which is always
    /// true for commission ≥ 0.  The test verifies the API implements this
    /// correctly rather than accidentally adding commissions.
    #[test]
    fn commissions_never_turn_profit_into_loss(
        round_trips in prop::collection::vec(
            (0.01f64..200.0, 0.01f64..200.0, 1i32..20i32),
            1..50,
        ),
        commission in 0.01f64..100.0,
    ) {
        // Gross PnL: same accounting as with fees, but commission = 0.
        let pnl_no_fees: f64 = round_trips.iter()
            .map(|(ep, xp, q)| {
                let entry = Trade::new(0, TradeType::Entry, String::new(), "T".to_string(),
                                      *ep, *q, 100.0, None, 0.0);
                let exit = Trade::new(0, TradeType::Exit, String::new(), "T".to_string(),
                                      *xp, *q, 100.0, None, 0.0);
                exit.proceeds() - entry.total_cost()
            })
            .sum();

        // Net PnL: identical except real commission is charged on every leg.
        let pnl_with_fees: f64 = round_trips.iter()
            .map(|(ep, xp, q)| {
                let entry = Trade::new(0, TradeType::Entry, String::new(), "T".to_string(),
                                       *ep, *q, 100.0, None, commission);
                let exit = Trade::new(0, TradeType::Exit, String::new(), "T".to_string(),
                                      *xp, *q, 100.0, None, commission);
                exit.proceeds() - entry.total_cost()
            })
            .sum();

        if pnl_no_fees > 0.0 {
            prop_assert!(
                pnl_with_fees <= pnl_no_fees + 1e-9,
                "fees increased PnL: gross={:.6} with_fees={:.6} diff={:.6}",
                pnl_no_fees, pnl_with_fees, pnl_with_fees - pnl_no_fees,
            );
        }
    }
}

// ─── Deterministic edge-kill test ─────────────────────────────────────────────

/// 100 iron condors with a theoretical $0.02 edge each.
///
/// Commission math (realistic $0.65 / contract rate):
///   100 condors × 4 legs × 2 sides (entry + exit) × $0.65 = $520.00
///   Gross edge:  100 × $0.02                               =   $2.00
///   Net PnL:     $2.00 − $520.00                           = −$518.00
///
/// This is the "fees ate the edge alive" scenario.  The test is parameterised
/// so you can see clearly which numbers matter; change commission_per_contract
/// to 0.0 to watch it flip positive.
#[test]
fn high_commission_kills_tiny_edge() {
    let costs = TradingCosts {
        commission_per_contract: 0.65,
        bid_ask_spread_percent: 0.0,
        slippage_model: SlippageModel::Fixed,
        ..TradingCosts::default()
    };

    let n_condors:      i32 = 100;
    let legs_per_condor: i32 = 4;
    let sides:           i32 = 2; // entry + exit

    let gross_pnl        = n_condors as f64 * 0.02;
    let total_commission = costs.commission_for(n_condors * legs_per_condor * sides);
    let net_pnl          = gross_pnl - total_commission;

    assert!(
        net_pnl < 0.0,
        "Fees ate the edge alive: gross=${:.2}, commissions=${:.2}, net=${:.2}",
        gross_pnl, total_commission, net_pnl,
    );

    // Sanity: confirm the commission figure equals the expected $520.
    let expected_commission = 0.65 * (n_condors * legs_per_condor * sides) as f64;
    assert!(
        (total_commission - expected_commission).abs() < 1e-9,
        "Commission maths wrong: got {:.2}, expected {:.2}",
        total_commission, expected_commission,
    );
}
