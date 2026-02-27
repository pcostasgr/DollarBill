// Property-based tests for personality classification stability.
// Verifies that `classify_personality_advanced` obeys invariants across 1000
// randomly generated AdvancedStockFeatures inputs.
//
// Run with: cargo test test_personality_props -- --nocapture

use proptest::prelude::*;
use dollarbill::analysis::advanced_classifier::{
    AdvancedStockClassifier, AdvancedStockFeatures, MarketRegime,
};
use dollarbill::analysis::stock_classifier::StockPersonality;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_classifier() -> AdvancedStockClassifier {
    AdvancedStockClassifier::new()
}

/// Construct AdvancedStockFeatures from the core features used by the scorer.
/// Non-tested fields are held at neutral mid-range values.
fn make_features(
    volatility_percentile:       f64, // [0,1]
    trend_strength:              f64, // [0,1]
    momentum_acceleration:       f64, // [0,1]
    trend_persistence:           f64, // [0,1]
    breakout_frequency:          f64, // [0,1]
    mean_reversion_speed:        f64, // [0,1]
    mean_reversion_strength:     f64, // [0,1]
    support_resistance_strength: f64, // [0,1]
    beta_stability:              f64, // [0,1]
) -> AdvancedStockFeatures {
    AdvancedStockFeatures {
        volatility_percentile,
        vol_regime:                   MarketRegime::LowVol,
        vol_persistence:              0.5,
        realized_vs_implied:          1.0,
        trend_strength,
        momentum_acceleration,
        trend_persistence,
        breakout_frequency,
        mean_reversion_speed,
        mean_reversion_strength,
        support_resistance_strength,
        sector_correlation:           0.5,
        market_beta:                  1.0,
        beta_stability,
        sector:                       "Technology".to_string(),
        sector_relative_vol:          0.0,
        sector_relative_momentum:     0.0,
    }
}

// ─── Property Tests ───────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// The classifier is a pure function: same inputs always produce the same
    /// (personality, confidence) pair.
    #[test]
    fn classification_deterministic(
        vol_pct   in 0.0f64..1.0,
        trend_str in 0.0f64..1.0,
        mom_acc   in 0.0f64..1.0,
        trend_per in 0.0f64..1.0,
        breakout  in 0.0f64..1.0,
        rev_speed in 0.0f64..1.0,
        rev_str   in 0.0f64..1.0,
        sr_str    in 0.0f64..1.0,
        beta_stab in 0.0f64..1.0,
    ) {
        let features = make_features(
            vol_pct, trend_str, mom_acc, trend_per, breakout,
            rev_speed, rev_str, sr_str, beta_stab,
        );
        let cls = make_classifier();
        let (p1, c1) = cls.classify_personality_advanced(&features);
        let (p2, c2) = cls.classify_personality_advanced(&features);
        prop_assert_eq!(p1, p2, "personality not deterministic");
        prop_assert!(
            (c1 - c2).abs() < 1e-12,
            "confidence not deterministic: {} vs {}", c1, c2
        );
    }

    /// Confidence is always a valid probability in [0, 1].
    #[test]
    fn confidence_always_in_unit_interval(
        vol_pct   in 0.0f64..1.0,
        trend_str in 0.0f64..1.0,
        mom_acc   in 0.0f64..1.0,
        trend_per in 0.0f64..1.0,
        breakout  in 0.0f64..1.0,
        rev_speed in 0.0f64..1.0,
        rev_str   in 0.0f64..1.0,
        sr_str    in 0.0f64..1.0,
        beta_stab in 0.0f64..1.0,
    ) {
        let features = make_features(
            vol_pct, trend_str, mom_acc, trend_per, breakout,
            rev_speed, rev_str, sr_str, beta_stab,
        );
        let (_, confidence) = make_classifier().classify_personality_advanced(&features);
        prop_assert!(
            confidence >= 0.0 && confidence <= 1.0,
            "confidence {} not in [0, 1]", confidence
        );
    }

    /// Extreme low vol (< 0.05) can never produce VolatileBreaker, which requires
    /// volatility_percentile > 0.9.
    #[test]
    fn stable_never_volatile_when_extreme_low_vol(
        trend_str in 0.0f64..1.0,
    ) {
        let features = make_features(0.02, trend_str, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        let (personality, _) = make_classifier().classify_personality_advanced(&features);
        prop_assert_ne!(
            personality, StockPersonality::VolatileBreaker,
            "vol_percentile=0.02 should never give VolatileBreaker"
        );
    }

    /// Extreme high vol (> 0.98) with high breakout and low beta stability can
    /// never produce StableAccumulator (needs vol < 0.4).
    #[test]
    fn volatile_never_stable_when_extreme_high_vol(
        breakout  in 0.71f64..1.0,
        beta_stab in 0.0f64..0.39,
    ) {
        let features = make_features(0.99, 0.0, 0.0, 0.0, breakout, 0.0, 0.0, 0.0, beta_stab);
        let (personality, _) = make_classifier().classify_personality_advanced(&features);
        prop_assert_ne!(
            personality, StockPersonality::StableAccumulator,
            "vol_percentile=0.99 + high breakout should never give StableAccumulator"
        );
    }

    /// Low vol + high trend + very stable beta favors StableAccumulator,
    /// never VolatileBreaker.
    #[test]
    fn extreme_low_vol_and_stable_beta_not_volatile(
        trend_str in 0.4f64..1.0,
        beta_stab in 0.8f64..1.0,
    ) {
        let features = make_features(0.01, trend_str, 0.1, 0.3, 0.1, 0.3, 0.3, 0.6, beta_stab);
        let (personality, _) = make_classifier().classify_personality_advanced(&features);
        prop_assert_ne!(
            personality, StockPersonality::VolatileBreaker,
            "extreme low vol + high beta stability should never yield VolatileBreaker"
        );
    }

    /// When all momentum signals are at ceiling values the scorer awards the
    /// maximum possible score to MomentumLeader, which must win.
    ///
    /// Scores with these inputs:
    ///   MomentumLeader  = 3.0 (mom>0.6) + 2.5 (trend>0.7) + 2.0 (vol>0.75) + 1.5 (breakout>0.6) = 9.0
    ///   VolatileBreaker = 3.0 (vol>0.9)  + 2.5 (breakout>0.7) + 2.0 (beta<0.4)                  = 7.5
    #[test]
    fn momentum_leader_when_all_momentum_signals_max(
        trend_per in 0.71f64..1.0,
        mom_acc   in 0.61f64..1.0,
    ) {
        let features = make_features(0.9, 0.8, mom_acc, trend_per, 0.9, 0.0, 0.0, 0.0, 0.1);
        let (personality, confidence) = make_classifier().classify_personality_advanced(&features);
        prop_assert_eq!(
            personality, StockPersonality::MomentumLeader,
            "all momentum signals at max should give MomentumLeader, got confidence={:.3}",
            confidence
        );
    }

    /// When all mean-reversion signals are at ceiling values the scorer awards
    /// maximum score to MeanReverting.
    ///
    /// Scores with these inputs:
    ///   MeanReverting   = 3.0 (speed>0.7) + 2.5 (strength>0.6) + 2.0 (sr>0.6) + 1.5 (trend_per<0.4) = 9.0
    ///   TrendFollower   ≤ 2.0 (vol in range)
    #[test]
    fn mean_reverting_with_all_reversion_signals_max(
        rev_speed in 0.71f64..1.0,
        rev_str   in 0.61f64..1.0,
        sr_str    in 0.61f64..1.0,
    ) {
        let features = make_features(0.5, 0.0, 0.0, 0.1, 0.0, rev_speed, rev_str, sr_str, 0.5);
        let (personality, _) = make_classifier().classify_personality_advanced(&features);
        prop_assert_eq!(
            personality, StockPersonality::MeanReverting,
            "all reversion signals at max + no trend should give MeanReverting"
        );
    }

    /// Extreme volatility + high breakout + low beta stability must give VolatileBreaker.
    ///
    /// Scores:
    ///   VolatileBreaker = 3.0 (vol>0.9) + 2.5 (breakout>0.7) + 2.0 (beta<0.4) = 7.5
    ///   MomentumLeader  = 2.0 (vol>0.75) + 1.5 (breakout>0.6)                  = 3.5
    #[test]
    fn volatile_breaker_when_extreme_vol_and_breakouts(
        breakout  in 0.71f64..1.0,
        beta_stab in 0.0f64..0.39,
    ) {
        let features = make_features(0.95, 0.0, 0.0, 0.0, breakout, 0.0, 0.0, 0.0, beta_stab);
        let (personality, _) = make_classifier().classify_personality_advanced(&features);
        prop_assert_eq!(
            personality, StockPersonality::VolatileBreaker,
            "extreme vol + high breakout + low beta_stability should give VolatileBreaker"
        );
    }

    /// Personality label is one of the five known variants — the function never
    /// returns a garbage discriminant value.
    #[test]
    fn personality_always_one_of_five_variants(
        vol_pct   in 0.0f64..1.0,
        trend_str in 0.0f64..1.0,
        mom_acc   in 0.0f64..1.0,
        trend_per in 0.0f64..1.0,
        breakout  in 0.0f64..1.0,
        rev_speed in 0.0f64..1.0,
        rev_str   in 0.0f64..1.0,
        sr_str    in 0.0f64..1.0,
        beta_stab in 0.0f64..1.0,
    ) {
        let features = make_features(
            vol_pct, trend_str, mom_acc, trend_per, breakout,
            rev_speed, rev_str, sr_str, beta_stab,
        );
        let (personality, _) = make_classifier().classify_personality_advanced(&features);
        let valid = matches!(
            personality,
            StockPersonality::MomentumLeader
                | StockPersonality::MeanReverting
                | StockPersonality::TrendFollower
                | StockPersonality::VolatileBreaker
                | StockPersonality::StableAccumulator
        );
        prop_assert!(valid, "unexpected personality variant: {:?}", personality);
    }
}
