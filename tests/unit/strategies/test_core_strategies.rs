// Tests for the four core trading strategies.
//
// All four strategies are now fully deterministic functions of their inputs —
// no SystemTime, no RNG.  Every test case is reproducible and exact.
//
// Strategy logic reference (default thresholds):
//   - MomentumStrategy      : threshold=0.15, min_iv=0.10
//   - MeanReversionStrategy : oversold=-2.0, overbought=2.0, vol_of_vol_scale=0.25
//   - BreakoutStrategy      : breakout_threshold=0.30, confirmation=1.10, min_iv=0.12
//   - VolatilityArbitrageStrategy : min_edge=0.015, iv_threshold=0.02

use dollarbill::strategies::{TradingStrategy, SignalAction};
use dollarbill::strategies::momentum::MomentumStrategy;
use dollarbill::strategies::mean_reversion::MeanReversionStrategy;
use dollarbill::strategies::breakout::BreakoutStrategy;
use dollarbill::strategies::vol_arbitrage::VolatilityArbitrageStrategy;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn first_action(signals: Vec<dollarbill::strategies::TradeSignal>) -> Option<SignalAction> {
    signals.into_iter().next().map(|s| s.action)
}

// ════════════════════════════════════════════════════════════════════════════
// MomentumStrategy
// ════════════════════════════════════════════════════════════════════════════

/// IV expanding sharply above historical vol → buy straddle.
///
/// market_iv / historical_vol = 0.35 / 0.25 = 1.40 → score = 0.40 > threshold (0.15).
#[test]
fn momentum_iv_expanding_signals_buy_straddle() {
    let s = MomentumStrategy::new();
    let signals = s.generate_signals("TSLA", 200.0, 0.35, 0.30, 0.25);
    assert!(
        matches!(first_action(signals), Some(SignalAction::BuyStraddle { .. })),
        "IV expanding 40% above HV should produce BuyStraddle"
    );
}

/// IV compressing well below historical vol → sell straddle.
///
/// market_iv / historical_vol = 0.17 / 0.25 = 0.68 → score = -0.32 < -threshold (-0.15).
#[test]
fn momentum_iv_compressing_signals_sell_straddle() {
    let s = MomentumStrategy::new();
    let signals = s.generate_signals("AAPL", 150.0, 0.17, 0.15, 0.25);
    assert!(
        matches!(first_action(signals), Some(SignalAction::SellStraddle { .. })),
        "IV compressing 32% below HV should produce SellStraddle"
    );
}

/// IV close to historical vol → no signal.
///
/// market_iv / historical_vol = 1.04 → score = 0.04, within threshold.
#[test]
fn momentum_no_edge_produces_no_signal() {
    let s = MomentumStrategy::new();
    let signals = s.generate_signals("SPY", 450.0, 0.26, 0.25, 0.25);
    assert!(signals.is_empty(), "Near-flat IV ratio should produce no signal");
}

/// Market IV below minimum threshold → no signal regardless of ratio.
#[test]
fn momentum_low_iv_produces_no_signal() {
    let s = MomentumStrategy::new();
    // Even though ratio is 5.0, IV is 0.05 < min_iv (0.10)
    let signals = s.generate_signals("GLD", 180.0, 0.05, 0.04, 0.01);
    assert!(signals.is_empty(), "Market IV below min_iv should produce no signal");
}

/// Confidence is capped at 1.0; edge is proportional to IV spread.
#[test]
fn momentum_signal_has_valid_confidence() {
    let s = MomentumStrategy::new();
    let signals = s.generate_signals("NVDA", 500.0, 0.80, 0.70, 0.40);
    assert!(!signals.is_empty());
    let sig = &signals[0];
    assert!(
        sig.confidence > 0.0 && sig.confidence <= 1.0,
        "Confidence out of range: {}",
        sig.confidence
    );
    assert!(sig.edge > 0.0, "Edge should be positive for IV-expanding signal");
}

// ════════════════════════════════════════════════════════════════════════════
// MeanReversionStrategy
// ════════════════════════════════════════════════════════════════════════════

/// IV well above model fair value → z-score > 2.0 → sell straddle.
///
/// z = (0.50 − 0.30) / (0.25 × 0.25) = 0.20 / 0.0625 = 3.2 > 2.0.
#[test]
fn mean_rev_iv_rich_signals_sell_straddle() {
    let s = MeanReversionStrategy::new();
    let signals = s.generate_signals("COIN", 100.0, 0.50, 0.30, 0.25);
    assert!(
        matches!(first_action(signals), Some(SignalAction::SellStraddle { .. })),
        "IV 3.2 z-scores above model should produce SellStraddle"
    );
}

/// IV well below model fair value → z-score < -2.0 → buy straddle.
///
/// z = (0.25 − 0.40) / (0.25 × 0.25) = −0.15 / 0.0625 = −2.4 < −2.0.
#[test]
fn mean_rev_iv_cheap_signals_buy_straddle() {
    let s = MeanReversionStrategy::new();
    let signals = s.generate_signals("AAPL", 180.0, 0.25, 0.40, 0.25);
    assert!(
        matches!(first_action(signals), Some(SignalAction::BuyStraddle { .. })),
        "IV 2.4 z-scores below model should produce BuyStraddle"
    );
}

/// IV at fair value → z-score near 0 → no signal.
#[test]
fn mean_rev_iv_at_fair_value_produces_no_signal() {
    let s = MeanReversionStrategy::new();
    let signals = s.generate_signals("MSFT", 300.0, 0.30, 0.30, 0.25);
    assert!(signals.is_empty(), "Zero z-score should produce no signal");
}

/// Market IV below minimum → no signal.
#[test]
fn mean_rev_low_iv_produces_no_signal() {
    let s = MeanReversionStrategy::new();
    // market_iv = 0.10 < min_volatility (0.15)
    let signals = s.generate_signals("SPY", 450.0, 0.10, 0.20, 0.20);
    assert!(signals.is_empty(), "Market IV below min_volatility should produce no signal");
}

// ════════════════════════════════════════════════════════════════════════════
// BreakoutStrategy
// ════════════════════════════════════════════════════════════════════════════

/// IV expands sharply and model confirms → iron butterfly.
///
/// expansion = 0.65/0.40 − 1 = 0.625 ≥ 0.30; model_ratio = 0.55/0.40 = 1.375 ≥ 1.10.
#[test]
fn breakout_iv_expansion_with_confirmation_signals_iron_butterfly() {
    let s = BreakoutStrategy::new();
    let signals = s.generate_signals("TSLA", 200.0, 0.65, 0.55, 0.40);
    match first_action(signals) {
        Some(SignalAction::IronButterfly { wing_width, .. }) => {
            assert!(wing_width > 0.0, "wing_width must be positive");
        }
        other => panic!("Expected IronButterfly, got {:?}", other),
    }
}

/// IV compressing below realised vol (expansion < 0) → sell straddle.
///
/// expansion = −0.08 < 0.05 → consolidation branch; compression = 0.5 > 0.2.
#[test]
fn breakout_consolidation_signals_sell_straddle() {
    let s = BreakoutStrategy::new();
    // market_iv (0.23) below historical_vol (0.25) → compression
    let signals = s.generate_signals("QQQ", 350.0, 0.23, 0.20, 0.25);
    assert!(
        matches!(first_action(signals), Some(SignalAction::SellStraddle { .. })),
        "IV below historical vol should produce SellStraddle"
    );
}

/// Moderate expansion, model doesn't confirm → no signal.
///
/// expansion = 0.28 (< 0.30 threshold) → no breakout; expansion ≥ 0.05 → no consolidation.
#[test]
fn breakout_moderate_expansion_no_model_confirmation_produces_no_signal() {
    let s = BreakoutStrategy::new();
    // expansion = 0.32/0.25 - 1 = 0.28 < 0.30; model_ratio = 0.25/0.25 = 1.0 < 1.10
    let signals = s.generate_signals("AMZN", 180.0, 0.32, 0.25, 0.25);
    assert!(signals.is_empty(), "Moderate inconclusive expansion should produce no signal");
}

/// IV below minimum → no signal.
#[test]
fn breakout_low_iv_produces_no_signal() {
    let s = BreakoutStrategy::new();
    // market_iv = 0.08 < min_iv (0.12)
    let signals = s.generate_signals("GLD", 180.0, 0.08, 0.07, 0.06);
    assert!(signals.is_empty(), "Market IV below min_iv should produce no signal");
}

// ════════════════════════════════════════════════════════════════════════════
// VolatilityArbitrageStrategy
// ════════════════════════════════════════════════════════════════════════════

/// IV rich vs. historical + rich vs. model, high market_iv → sell straddle.
///
/// historical_vol=0.35 → regime=1.0; vol_premium=0.15; model_edge=0.10; total=0.25 > 0.015.
/// market_iv=0.50 > 0.40 → SellStraddle branch.
#[test]
fn vol_arb_iv_rich_high_vol_signals_sell_straddle() {
    let s = VolatilityArbitrageStrategy::new();
    let signals = s.generate_signals("TSLA", 200.0, 0.50, 0.40, 0.35);
    assert!(
        matches!(first_action(signals), Some(SignalAction::SellStraddle { .. })),
        "Rich IV at high vol level should produce SellStraddle"
    );
}

/// IV cheap vs. historical and model → buy straddle.
///
/// vol_premium = -0.10; model_edge = -0.05; total = -0.15 < -0.015.
#[test]
fn vol_arb_iv_cheap_signals_buy_straddle() {
    let s = VolatilityArbitrageStrategy::new();
    let signals = s.generate_signals("AAPL", 150.0, 0.25, 0.30, 0.35);
    assert!(
        matches!(first_action(signals), Some(SignalAction::BuyStraddle { .. })),
        "Cheap IV should produce BuyStraddle"
    );
}

/// IV at fair value → no edge → no signal above confidence threshold.
#[test]
fn vol_arb_flat_iv_produces_no_signal() {
    let s = VolatilityArbitrageStrategy::new();
    // market_iv == model_iv == historical_vol → zero edge
    let signals = s.generate_signals("SPY", 450.0, 0.20, 0.20, 0.20);
    assert!(signals.is_empty(), "Zero edge should produce no signal");
}

/// IV moderately rich but below 0.40 → iron butterfly (not straddle).
///
/// vol_premium=0.10; model_edge=0.07; total=0.17 > 0.015; market_iv=0.35 ≤ 0.40.
#[test]
fn vol_arb_moderate_rich_iv_signals_iron_butterfly() {
    let s = VolatilityArbitrageStrategy::new();
    let signals = s.generate_signals("QQQ", 350.0, 0.35, 0.28, 0.25);
    match first_action(signals) {
        Some(SignalAction::IronButterfly { wing_width, .. }) => {
            assert!(wing_width > 0.0, "wing_width must be positive");
        }
        other => panic!("Expected IronButterfly for moderate rich IV, got {:?}", other),
    }
}

/// Signals have positive edge and valid confidence.
#[test]
fn vol_arb_signal_fields_are_valid() {
    let s = VolatilityArbitrageStrategy::new();
    let signals = s.generate_signals("NVDA", 500.0, 0.60, 0.45, 0.40);
    assert!(!signals.is_empty(), "Should fire a signal for high-vol rich IV");
    let sig = &signals[0];
    assert!(sig.confidence > 0.0 && sig.confidence <= 1.0);
    assert!(sig.edge > 0.0);
    assert!(sig.expiry_days > 0);
}
