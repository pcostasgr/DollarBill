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

    // ─── Proposal 4: ±1 % price-noise invariance ──────────────────────────────

    /// When the classifier returns a *high confidence* result for a clearly
    /// dominant personality, adding ±1 % noise to every feature must not flip
    /// the label.
    ///
    /// "High confidence" = the base confidence score is above 0.55 (well above
    /// the 0.5 decision boundary).  We test two canonical high-confidence cases:
    ///
    ///   MomentumLeader  — all momentum features clearly above thresholds
    ///   VolatileBreaker — extreme vol + high breakout + low beta stability
    ///
    /// A flip under sub-1 % noise at high confidence indicates an instability
    /// in the scorer's threshold placement.
    #[test]
    fn momentum_leader_classification_stable_under_1pct_noise(
        noise in -0.01f64..0.01f64,
    ) {
        // All momentum features well clear of their thresholds:
        //   mom_acc > 0.61, trend > 0.71, vol > 0.75, breakout > 0.6
        // Scores: MomentumLeader ≈ 9.0, next best ≈ 3.5 — a 5.5-point gap
        let features = make_features(
            (0.90 + noise).clamp(0.0, 1.0),   // volatility_percentile
            (0.85 + noise).clamp(0.0, 1.0),   // trend_strength
            (0.80 + noise).clamp(0.0, 1.0),   // momentum_acceleration
            (0.80 + noise).clamp(0.0, 1.0),   // trend_persistence
            (0.80 + noise).clamp(0.0, 1.0),   // breakout_frequency
            0.0,                               // mean_reversion_speed (neutral)
            0.0,                               // mean_reversion_strength
            0.0,                               // support_resistance_strength
            0.1,                               // beta_stability
        );
        let (personality, confidence) = make_classifier().classify_personality_advanced(&features);

        // Only assert stability when confidence is still high after perturbation
        if confidence > 0.55 {
            prop_assert_eq!(
                personality,
                StockPersonality::MomentumLeader,
                "MomentumLeader flipped under {:.4}% noise (confidence={:.3})",
                noise * 100.0, confidence
            );
        }
    }

    #[test]
    fn volatile_breaker_classification_stable_under_1pct_noise(
        noise in -0.01f64..0.01f64,
    ) {
        // VolatileBreaker features well clear of thresholds:
        //   vol > 0.9, breakout > 0.7, beta_stability < 0.4
        // Scores: VolatileBreaker ≈ 7.5, MomentumLeader ≈ 3.5 — a 4-point gap
        let features = make_features(
            (0.97 + noise).clamp(0.0, 1.0),   // volatility_percentile
            0.0,                               // trend_strength (neutral)
            0.0,                               // momentum_acceleration
            0.0,                               // trend_persistence
            (0.85 + noise).clamp(0.0, 1.0),   // breakout_frequency
            0.0,                               // mean_reversion_speed
            0.0,                               // mean_reversion_strength
            0.0,                               // support_resistance_strength
            (0.15 + noise).clamp(0.0, 1.0),   // beta_stability (low = volatile)
        );
        let (personality, confidence) = make_classifier().classify_personality_advanced(&features);

        if confidence > 0.55 {
            prop_assert_eq!(
                personality,
                StockPersonality::VolatileBreaker,
                "VolatileBreaker flipped under {:.4}% noise (confidence={:.3})",
                noise * 100.0, confidence
            );
        }
    }

    // ── Extended noise robustness: ±2% perturbation tests ────────────────────
    //
    // Proposal: "Personality fuzz robustness — ±0.5–2% price noise → classification
    // flip?  Extends the existing ±1% tests to ±2% for deeper robustness coverage."

    /// MomentumLeader features placed 10–15 pp above all thresholds must still
    /// classify correctly when features are perturbed by up to ±2%.
    #[test]
    fn momentum_leader_stable_under_2pct_noise(
        noise in -0.02f64..0.02f64,
    ) {
        let features = make_features(
            (0.90 + noise).clamp(0.0, 1.0),
            (0.85 + noise).clamp(0.0, 1.0),
            (0.80 + noise).clamp(0.0, 1.0),
            (0.80 + noise).clamp(0.0, 1.0),
            (0.80 + noise).clamp(0.0, 1.0),
            0.0,
            0.0,
            0.0,
            0.1,
        );
        let (personality, confidence) = make_classifier().classify_personality_advanced(&features);

        if confidence > 0.50 {
            prop_assert_eq!(
                personality,
                StockPersonality::MomentumLeader,
                "MomentumLeader flipped under {:.4}% noise (confidence={:.3})",
                noise * 100.0, confidence
            );
        }
    }

    /// VolatileBreaker features placed well above all thresholds must still
    /// classify correctly under ±2% perturbation.
    #[test]
    fn volatile_breaker_stable_under_2pct_noise(
        noise in -0.02f64..0.02f64,
    ) {
        let features = make_features(
            (0.97 + noise).clamp(0.0, 1.0),
            0.0,
            0.0,
            0.0,
            (0.85 + noise).clamp(0.0, 1.0),
            0.0,
            0.0,
            0.0,
            (0.15 + noise).clamp(0.0, 1.0),
        );
        let (personality, confidence) = make_classifier().classify_personality_advanced(&features);

        if confidence > 0.50 {
            prop_assert_eq!(
                personality,
                StockPersonality::VolatileBreaker,
                "VolatileBreaker flipped under {:.4}% noise (confidence={:.3})",
                noise * 100.0, confidence
            );
        }
    }

    /// MeanReverting features (extreme mean-reversion scores, no momentum) must
    /// remain stable under ±2% feature perturbation.
    #[test]
    fn mean_reverting_stable_under_2pct_noise(
        noise in -0.02f64..0.02f64,
    ) {
        // MeanReverting dominates when: mean_reversion_speed > 0.6,
        // mean_reversion_strength > 0.6, support_resistance > 0.5.
        // Set features 25 pp above these thresholds for a comfortable margin.
        let features = make_features(
            (0.30 + noise).clamp(0.0, 1.0),  // vol low (not a volatile stock)
            0.0,                              // trend_strength (no momentum)
            0.0,
            0.0,
            0.0,
            (0.85 + noise).clamp(0.0, 1.0),  // mean_reversion_speed high
            (0.85 + noise).clamp(0.0, 1.0),  // mean_reversion_strength high
            (0.80 + noise).clamp(0.0, 1.0),  // support_resistance_strength high
            0.5,                              // beta_stability neutral
        );
        let (personality, confidence) = make_classifier().classify_personality_advanced(&features);

        if confidence > 0.50 {
            prop_assert_eq!(
                personality,
                StockPersonality::MeanReverting,
                "MeanReverting flipped under {:.4}% noise (confidence={:.3})",
                noise * 100.0, confidence
            );
        }
    }
}

// ─── Deterministic flip-rate measurement ─────────────────────────────────────
//
// Proptest can assert per-sample invariants but cannot count aggregate flip
// rates.  This deterministic test sweeps 50 evenly-spaced noise values across
// the ±2% range for features positioned near a classification boundary and
// asserts that the flip rate is < 30%.  This gives an empirical bound on the
// "danger zone" for live trading decisions.

#[test]
fn classification_near_boundary_flip_rate_under_30pct_over_2pct_noise() {
    let classifier = make_classifier();
    let n_samples       = 50_usize;
    let noise_range     = 0.02_f64;  // ±2%

    // Features near the MomentumLeader boundary — just 5 pp above each threshold.
    // At this margin, a 2% perturbation can theoretically reach the boundary.
    let base_vol_pct  = 0.66_f64;  // threshold ≈ 0.61 + 5 pp margin
    let base_trend    = 0.76_f64;  // threshold ≈ 0.71 + 5 pp margin
    let base_mom_acc  = 0.66_f64;  // threshold ≈ 0.61 + 5 pp margin
    let base_breakout = 0.65_f64;  // threshold ≈ 0.60 + 5 pp margin

    // Determine the "baseline" classification (no noise)
    let baseline_features = make_features(
        base_vol_pct, base_trend, base_mom_acc, base_breakout, base_breakout,
        0.0, 0.0, 0.0, 0.1,
    );
    let (baseline_personality, _) =
        classifier.classify_personality_advanced(&baseline_features);

    let mut flip_count = 0_usize;

    for i in 0..n_samples {
        let noise = -noise_range + (2.0 * noise_range / (n_samples as f64 - 1.0)) * i as f64;
        let features = make_features(
            (base_vol_pct  + noise).clamp(0.0, 1.0),
            (base_trend    + noise).clamp(0.0, 1.0),
            (base_mom_acc  + noise).clamp(0.0, 1.0),
            (base_breakout + noise).clamp(0.0, 1.0),
            (base_breakout + noise).clamp(0.0, 1.0),
            0.0, 0.0, 0.0, 0.1,
        );
        let (personality, _) = classifier.classify_personality_advanced(&features);
        if personality != baseline_personality {
            flip_count += 1;
        }
    }

    let flip_rate = flip_count as f64 / n_samples as f64;
    assert!(
        flip_rate < 0.30,
        "Classification flip rate near boundary is {:.1}% ({}/{}) over ±2% noise — \
         exceeds 30% threshold; boundary too unstable for live trading",
        flip_rate * 100.0, flip_count, n_samples
    );
}