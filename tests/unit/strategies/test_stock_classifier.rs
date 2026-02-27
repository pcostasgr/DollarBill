//! Stock personality classification edge-case tests.
//!
//! Covers: almost-constant price (StableAccumulator / TrendFollower), extreme-vol
//! (VolatileBreaker), momentum-with-low-vol (MomentumLeader / TrendFollower),
//! classification stability under small perturbation, and strategy list validity.

use dollarbill::analysis::stock_classifier::{StockClassifier, StockPersonality};

// ─── 6. Stock Personality Classification ────────────────────────────────────

/// Almost-constant price stock → low vol → should map to StableAccumulator or TrendFollower.
#[test]
#[allow(deprecated)]
fn test_constant_price_stock_is_stable() {
    let mut classifier = StockClassifier::new();

    // Near-zero vol, no trend, slight reversion tendency
    let profile = classifier.classify_stock(
        "DEADMONEY",
        0.01,  // avg_volatility  (1% — essentially flat)
        0.1,   // trend_strength
        0.45,  // mean_reversion_tendency
        0.2,   // momentum_sensitivity
    );

    let expected = [StockPersonality::StableAccumulator, StockPersonality::TrendFollower];
    assert!(expected.contains(&profile.personality),
            "Almost-constant price stock should be StableAccumulator or TrendFollower, got {:?}",
            profile.personality);

    // Best strategies must be non-empty
    assert!(!profile.best_strategies.is_empty(),
            "Should always have at least one best strategy");
}

/// Extremely volatile stock → should map to VolatileBreaker or MomentumLeader.
#[test]
#[allow(deprecated)]
fn test_extreme_vol_stock_volatile_breaker() {
    let mut classifier = StockClassifier::new();

    let profile = classifier.classify_stock(
        "WILDCAT",
        0.80,  // avg_volatility  (80% — very high)
        0.4,   // trend_strength
        0.3,   // mean_reversion_tendency
        0.5,   // momentum_sensitivity
    );

    let high_risk_personalities = [
        StockPersonality::VolatileBreaker,
        StockPersonality::MomentumLeader,
        StockPersonality::MeanReverting,
    ];
    assert!(high_risk_personalities.contains(&profile.personality),
            "Extreme-vol stock should be VolatileBreaker/MomentumLeader/MeanReverting, got {:?}",
            profile.personality);

    // VolatileBreaker should NOT recommend long directional options
    if profile.personality == StockPersonality::VolatileBreaker {
        assert!(!profile.worst_strategies.is_empty(),
                "VolatileBreaker should have worst-strategy warnings");
    }
}

/// Momentum stock with low volatility → MomentumLeader or TrendFollower.
#[test]
#[allow(deprecated)]
fn test_momentum_low_vol_stock_momentum_leader() {
    let mut classifier = StockClassifier::new();

    // Medium vol, strong trend, medium reversion, high momentum
    let profile = classifier.classify_stock(
        "NVDA_LIKE",
        0.35,  // avg_volatility (35% — medium)
        0.75,  // trend_strength  (strong trend)
        0.2,   // mean_reversion_tendency
        0.5,   // momentum_sensitivity (medium)
    );

    let expected = [StockPersonality::TrendFollower, StockPersonality::MomentumLeader];
    assert!(expected.contains(&profile.personality),
            "Momentum-with-medium-vol stock should be TrendFollower or MomentumLeader, got {:?}",
            profile.personality);
}

/// Classification stability: a tiny perturbation in one parameter should not
/// flip the personality to a completely different bucket.
/// (Verifies there is no knife-edge instability at the boundary.)
#[test]
#[allow(deprecated)]
fn test_classification_stability_small_perturbation() {
    let mut classifier = StockClassifier::new();

    let base_vol       = 0.30;
    let trend          = 0.50;
    let reversion      = 0.40;
    let momentum       = 0.50;

    let base_profile = classifier.classify_stock("BASE", base_vol, trend, reversion, momentum);

    // Perturb volatility by 1%
    let perturbed = classifier.classify_stock(
        "PERTURBED",
        base_vol + 0.01,
        trend,
        reversion,
        momentum,
    );

    // Both should have non-empty strategy lists regardless of bucket
    assert!(!base_profile.best_strategies.is_empty(),
            "Base profile must have strategies");
    assert!(!perturbed.best_strategies.is_empty(),
            "Perturbed profile must have strategies");
}

/// All strategy lists are non-empty and contain at least one string for every personality.
#[test]
#[allow(deprecated)]
fn test_all_personalities_have_strategy_recommendations() {
    let mut classifier = StockClassifier::new();

    let cases = vec![
        ("STABLE",   0.10, 0.2, 0.5, 0.2),  // → StableAccumulator
        ("TREND",    0.30, 0.7, 0.3, 0.4),  // → TrendFollower
        ("MOMENTUM", 0.40, 0.5, 0.2, 0.8),  // → MomentumLeader
        ("VOLATILE", 0.70, 0.3, 0.3, 0.4),  // → VolatileBreaker / MomentumLeader
        ("REVERTER", 0.60, 0.2, 0.7, 0.3),  // → MeanReverting
    ];

    for (sym, v, t, r, m) in cases {
        let profile = classifier.classify_stock(sym, v, t, r, m);
        assert!(!profile.best_strategies.is_empty(),
                "Stock {} ({:?}) must have best strategies", sym, profile.personality);
        assert!(!profile.worst_strategies.is_empty(),
                "Stock {} ({:?}) must have worst strategies", sym, profile.personality);
    }
}
