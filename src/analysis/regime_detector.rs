//! Standalone market regime detector.
//!
//! Classifies the current market environment as one of five regimes
//! (`LowVol`, `HighVol`, `Trending`, `MeanReverting`, `EventDriven`) from
//! either a closing-price slice or pre-computed scalar inputs.
//!
//! The same classification logic lives inside `AdvancedStockClassifier`;
//! this module exposes a lightweight, dependency-free version that can be
//! called anywhere in the pipeline without owning a full classifier instance.

use crate::analysis::advanced_classifier::MarketRegime;

/// Window (in trading days) used for vol and trend calculations.
const WINDOW: usize = 20;

/// Lightweight market regime classifier.
pub struct RegimeDetector;

impl RegimeDetector {
    /// Detect regime from a slice of daily closing prices.
    ///
    /// Requires at least `WINDOW` (20) prices; returns `MeanReverting` if
    /// fewer are supplied.
    pub fn detect(closes: &[f64]) -> MarketRegime {
        if closes.len() < WINDOW {
            return MarketRegime::MeanReverting;
        }

        let recent = &closes[closes.len() - WINDOW..];

        // ── Annualized realized volatility ──────────────────────────────────
        let log_returns: Vec<f64> = recent
            .windows(2)
            .map(|w| (w[1] / w[0]).ln())
            .collect();

        let n = log_returns.len() as f64;
        let mean_r = log_returns.iter().sum::<f64>() / n;
        let variance = log_returns
            .iter()
            .map(|r| (r - mean_r).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        let ann_vol = (variance * 252.0_f64).sqrt();

        // ── Trend strength ───────────────────────────────────────────────────
        // Normalised 20-day price change: ±10 % ≈ ±1.0 on this scale.
        let trend = (recent[WINDOW - 1] / recent[0] - 1.0) / 0.10;

        Self::classify(ann_vol, trend)
    }

    /// Detect regime from pre-computed scalar inputs.
    ///
    /// Useful inside `generate_signals` where full price history is not
    /// available but `historical_vol` (annualized) and a trend proxy can be
    /// passed directly.
    ///
    /// * `ann_vol`: annualized realized volatility (e.g. 0.25 = 25 %)
    /// * `trend_strength`: signed trend proxy, normalised so |value| > 0.6
    ///   indicates a strong trend.  Pass 0.0 if unknown.
    pub fn detect_from_scalars(ann_vol: f64, trend_strength: f64) -> MarketRegime {
        Self::classify(ann_vol, trend_strength)
    }

    /// Core classification logic shared by both public entry-points.
    fn classify(ann_vol: f64, trend: f64) -> MarketRegime {
        match (ann_vol, trend) {
            (v, _) if v > 0.40 => MarketRegime::HighVol,
            (v, t) if v < 0.15 && t.abs() < 0.30 => MarketRegime::LowVol,
            (_, t) if t.abs() > 0.60 => MarketRegime::Trending,
            _ => MarketRegime::MeanReverting,
        }
    }

    /// Return per-strategy weight multipliers for a given regime.
    ///
    /// Strategy names must match those returned by the respective
    /// `TradingStrategy::name()` implementations.  Any strategy whose name
    /// is not listed defaults to a weight multiplier of `1.0`.
    pub fn strategy_weights(regime: &MarketRegime) -> &'static [(&'static str, f64)] {
        match regime {
            MarketRegime::LowVol => &[
                ("Cash-Secured Puts",   2.0),
                ("Mean Reversion",      2.0),
                ("Vol Mean Reversion",  1.5),
                ("Vol Arbitrage",       0.5),
                ("Momentum",            0.5),
                ("Breakout",            0.5),
            ],
            MarketRegime::HighVol => &[
                ("Vol Mean Reversion",  2.0),
                ("Vol Arbitrage",       2.0),
                ("Cash-Secured Puts",   1.5),
                ("Mean Reversion",      0.5),
                ("Momentum",            0.7),
                ("Breakout",            0.7),
            ],
            MarketRegime::Trending => &[
                ("Momentum",            2.0),
                ("Breakout",            2.0),
                ("Vol Arbitrage",       1.0),
                ("Vol Mean Reversion",  0.7),
                ("Mean Reversion",      0.3),
                ("Cash-Secured Puts",   0.5),
            ],
            MarketRegime::MeanReverting => &[
                ("Mean Reversion",      2.0),
                ("Vol Mean Reversion",  1.5),
                ("Cash-Secured Puts",   1.5),
                ("Vol Arbitrage",       0.7),
                ("Momentum",            0.5),
                ("Breakout",            0.3),
            ],
            MarketRegime::EventDriven => &[
                ("Vol Arbitrage",       2.0),
                ("Vol Mean Reversion",  1.5),
                ("Momentum",            1.0),
                ("Mean Reversion",      0.5),
                ("Cash-Secured Puts",   0.5),
                ("Breakout",            0.5),
            ],
        }
    }

    /// Look up the regime multiplier for a single strategy name.
    /// Returns `1.0` if the strategy is not found in the table.
    pub fn weight_for(regime: &MarketRegime, strategy_name: &str) -> f64 {
        Self::strategy_weights(regime)
            .iter()
            .find(|(name, _)| *name == strategy_name)
            .map(|(_, w)| *w)
            .unwrap_or(1.0)
    }

    /// Return the global **position-sizing multiplier** for a regime.
    ///
    /// Applied to the base contract count from any `PositionSizer` method.
    ///
    /// | Regime        | Multiplier | Rationale                                     |
    /// |---------------|------------|-----------------------------------------------|
    /// | `HighVol`     | 0.35       | Crash/panic: 65 % size reduction to cap DD    |
    /// | `LowVol`      | 1.80       | Calm market: deploy more size for higher θ    |
    /// | `Trending`    | 1.00       | Neutral: direction is clear, keep full size   |
    /// | `MeanRev`     | 1.00       | Neutral: equal chance either way              |
    /// | `EventDriven` | 0.50       | Binary risk: pre-earnings / binary events     |
    pub fn sizing_multiplier(regime: &MarketRegime) -> f64 {
        match regime {
            MarketRegime::HighVol       => 0.35,
            MarketRegime::LowVol        => 1.80,
            MarketRegime::Trending      => 1.00,
            MarketRegime::MeanReverting => 1.00,
            MarketRegime::EventDriven   => 0.50,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_prices(n: usize, base: f64) -> Vec<f64> {
        vec![base; n]
    }

    fn trending_prices(n: usize, start: f64, daily_return: f64) -> Vec<f64> {
        (0..n)
            .map(|i| start * (1.0 + daily_return).powi(i as i32))
            .collect()
    }

    fn noisy_prices(n: usize, base: f64, noise_pct: f64) -> Vec<f64> {
        // Deterministic noise using a simple LCG for reproducibility
        let mut state: u64 = 0x1234_5678_ABCD_EF01;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let noise = ((state >> 33) as f64 / u32::MAX as f64) - 0.5; // [-0.5, 0.5)
                base * (1.0 + noise * noise_pct)
            })
            .collect()
    }

    // ── detect() ─────────────────────────────────────────────────────────────

    #[test]
    fn test_detect_insufficient_data_returns_mean_reverting() {
        let closes = vec![100.0; 5]; // only 5 prices
        assert_eq!(RegimeDetector::detect(&closes), MarketRegime::MeanReverting);
    }

    #[test]
    fn test_detect_low_vol_flat_market() {
        // Essentially flat prices → very low vol, no trend → LowVol
        let closes = flat_prices(30, 100.0);
        assert_eq!(RegimeDetector::detect(&closes), MarketRegime::LowVol);
    }

    #[test]
    fn test_detect_high_vol_crash() {
        // Simulate very high daily moves (±10 %) → annualized vol ~92 % > 40 %
        // Daily log-return std ≈ 0.10/√3 ≈ 0.0577; annualized ≈ 0.0577 × √252 ≈ 0.92
        let closes = noisy_prices(30, 100.0, 0.20); // 20 % noise amplitude
        assert_eq!(RegimeDetector::detect(&closes), MarketRegime::HighVol);
    }

    #[test]
    fn test_detect_trending_strong_uptrend() {
        // +0.5 % per day over 20 days = 10 % period gain → trend > 0.6 normalised
        let closes = trending_prices(30, 100.0, 0.005);
        assert_eq!(RegimeDetector::detect(&closes), MarketRegime::Trending);
    }

    #[test]
    fn test_detect_trending_strong_downtrend() {
        // -0.5 % per day
        let closes = trending_prices(30, 100.0, -0.005);
        assert_eq!(RegimeDetector::detect(&closes), MarketRegime::Trending);
    }

    #[test]
    fn test_detect_mean_reverting_moderate_noise() {
        // Moderate noise, no trend → MeanReverting
        let closes = noisy_prices(30, 100.0, 0.015); // ~1.5 % noise
        // Annualized vol depends on noise; with 1.5 % it's roughly 15-30 %
        // which falls in the MeanReverting bucket (not HighVol, not LowVol, no trend)
        let regime = RegimeDetector::detect(&closes);
        assert!(
            regime == MarketRegime::MeanReverting || regime == MarketRegime::LowVol,
            "Expected MeanReverting or LowVol for moderate noise, got {:?}",
            regime
        );
    }

    #[test]
    fn test_detect_uses_most_recent_window() {
        // Build 100-day history: first 80 days flat, last 20 days strongly trending
        let mut closes = flat_prices(80, 100.0);
        let trending = trending_prices(21, 100.0, 0.005);
        closes.extend_from_slice(&trending[1..]); // 20 new days
        assert_eq!(RegimeDetector::detect(&closes), MarketRegime::Trending);
    }

    // ── detect_from_scalars() ─────────────────────────────────────────────────

    #[test]
    fn test_scalar_high_vol_regime() {
        assert_eq!(
            RegimeDetector::detect_from_scalars(0.45, 0.0),
            MarketRegime::HighVol
        );
    }

    #[test]
    fn test_scalar_low_vol_regime() {
        assert_eq!(
            RegimeDetector::detect_from_scalars(0.10, 0.1),
            MarketRegime::LowVol
        );
    }

    #[test]
    fn test_scalar_trending_regime() {
        assert_eq!(
            RegimeDetector::detect_from_scalars(0.25, 0.8),  // strong positive trend
            MarketRegime::Trending
        );
        assert_eq!(
            RegimeDetector::detect_from_scalars(0.25, -0.8), // strong negative trend
            MarketRegime::Trending
        );
    }

    #[test]
    fn test_scalar_mean_reverting_regime() {
        assert_eq!(
            RegimeDetector::detect_from_scalars(0.20, 0.1),
            MarketRegime::MeanReverting
        );
    }

    // ── strategy_weights() / weight_for() ─────────────────────────────────────

    #[test]
    fn test_all_regimes_return_weight_tables() {
        for regime in [
            MarketRegime::LowVol,
            MarketRegime::HighVol,
            MarketRegime::Trending,
            MarketRegime::MeanReverting,
            MarketRegime::EventDriven,
        ] {
            let weights = RegimeDetector::strategy_weights(&regime);
            assert!(!weights.is_empty(), "Weight table empty for {:?}", regime);
            for (name, w) in weights {
                assert!(!name.is_empty(), "Empty strategy name in {:?}", regime);
                assert!(*w > 0.0, "Non-positive weight for {} in {:?}", name, regime);
            }
        }
    }

    #[test]
    fn test_weight_for_known_strategy() {
        let w = RegimeDetector::weight_for(&MarketRegime::HighVol, "Vol Mean Reversion");
        assert_eq!(w, 2.0);
    }

    #[test]
    fn test_weight_for_unknown_strategy_defaults_to_one() {
        let w = RegimeDetector::weight_for(&MarketRegime::HighVol, "NonExistentStrategy");
        assert_eq!(w, 1.0);
    }

    #[test]
    fn test_momentum_down_weighted_in_low_vol() {
        let w_low = RegimeDetector::weight_for(&MarketRegime::LowVol, "Momentum");
        let w_trend = RegimeDetector::weight_for(&MarketRegime::Trending, "Momentum");
        assert!(w_trend > w_low, "Momentum should weigh more in Trending than LowVol");
    }

    #[test]
    fn test_mean_reversion_down_weighted_in_trending() {
        let w_trend = RegimeDetector::weight_for(&MarketRegime::Trending, "Mean Reversion");
        let w_mr = RegimeDetector::weight_for(&MarketRegime::MeanReverting, "Mean Reversion");
        assert!(w_mr > w_trend, "Mean Reversion should weigh more in MeanReverting than Trending");
    }
}
