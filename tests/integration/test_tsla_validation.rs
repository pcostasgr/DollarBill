//! Tesla 1-year validation suite (2025–2026 dataset)
//!
//! Exercises regime detection, HV-percentile computation, and strategy-weight
//! selection against the real Feb–Apr 2025 TSLA drawdown (peak ≈ $480,
//! trough ≈ $221 on 2025-04-08 — a ~54% decline in ≈47 trading days).
//!
//! Tests designed to be fast (no network, no Alpaca keys): all inputs come
//! from `data/tesla_one_year.csv` and pure library logic.

use dollarbill::market_data::csv_loader::load_csv_closes;
use dollarbill::analysis::regime_detector::RegimeDetector;
use dollarbill::analysis::advanced_classifier::MarketRegime;
use dollarbill::strategies::spreads::{rolling_hv21, iv_rank};

const TSLA_CSV: &str = "data/tesla_one_year.csv";
/// Approximate date the 2025 crash trough was reached.
const CRASH_LOW_DATE:  &str = "2025-04-08";
/// A later "calm" reference point after the post-crash recovery.
const CALM_REF_DATE:   &str = "2025-08-15";

// ─── helper ──────────────────────────────────────────────────────────────────

/// Compute annualised realised volatility from a chronological (oldest-first)
/// closes slice using overlapping log-returns.
fn ann_rv(closes: &[f64]) -> f64 {
    if closes.len() < 2 {
        return 0.0;
    }
    let log_returns: Vec<f64> = closes.windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .collect();
    let n   = log_returns.len() as f64;
    let mu  = log_returns.iter().sum::<f64>() / n;
    let var = log_returns.iter().map(|r| (r - mu).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (var * 252.0_f64).sqrt()
}

// ─── data integrity ──────────────────────────────────────────────────────────

#[test]
fn tsla_csv_loads_full_year() {
    let days = load_csv_closes(TSLA_CSV).expect("tesla_one_year.csv must exist and parse");
    assert!(
        days.len() >= 200,
        "Expected ≥200 trading days, got {} — CSV may be truncated",
        days.len()
    );
}

#[test]
fn tsla_csv_spans_crash_window() {
    let days = load_csv_closes(TSLA_CSV).expect("CSV load");
    // csv_loader returns newest-first; last element = oldest day
    let oldest = days.last().expect("non-empty").date.as_str();
    let newest = days.first().expect("non-empty").date.as_str();
    assert!(
        oldest <= CRASH_LOW_DATE,
        "CSV starts at {} but must begin before crash low {}",
        oldest, CRASH_LOW_DATE
    );
    assert!(
        newest >= CALM_REF_DATE,
        "CSV ends at {} but must reach past calm reference {}",
        newest, CALM_REF_DATE
    );
}

// ─── max drawdown ─────────────────────────────────────────────────────────────

#[test]
fn tsla_2025_max_drawdown_exceeds_40_pct() {
    let days = load_csv_closes(TSLA_CSV).expect("CSV load");
    let closes: Vec<f64> = days.iter().map(|d| d.close).collect();
    let peak   = closes.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let trough = closes.iter().cloned().fold(f64::INFINITY,     f64::min);
    let max_dd = (peak - trough) / peak;
    assert!(
        max_dd > 0.40,
        "TSLA 2025 max drawdown should exceed 40 % (peak ${:.2}, trough ${:.2}, dd={:.1}%)",
        peak, trough, max_dd * 100.0
    );
}

// ─── regime detection ─────────────────────────────────────────────────────────

#[test]
fn tsla_crash_window_regime_is_high_vol_or_trending() {
    let days = load_csv_closes(TSLA_CSV).expect("CSV load");
    // Reverse to chronological (oldest first) for regime detector
    let chron: Vec<_> = days.iter().rev().collect();
    let closes: Vec<f64> = chron.iter().map(|d| d.close).collect();

    // Find the index of the first date >= CRASH_LOW_DATE in chronological order
    let crash_idx = chron.iter().enumerate()
        .find(|(_, d)| d.date.as_str() >= CRASH_LOW_DATE)
        .map(|(i, _)| i)
        .expect("CRASH_LOW_DATE must exist in CSV data");

    // Feed the 30 closes ending at (and including) the crash trough to the detector.
    // RegimeDetector::detect uses the last 20 of the provided slice, so 30 gives
    // a comfortable buffer while keeping the focus on the crash window.
    let end   = (crash_idx + 1).min(closes.len());
    let start = end.saturating_sub(30);
    let regime = RegimeDetector::detect(&closes[start..end]);

    assert!(
        matches!(regime, MarketRegime::HighVol | MarketRegime::Trending),
        "Expected HighVol or Trending during the TSLA crash trough (2025-04-08), got {:?}",
        regime
    );
}

#[test]
fn tsla_crash_hv_rank_exceeds_calm_hv_rank() {
    // TSLA is structurally HighVol all year, so comparing absolute regimes
    // is not meaningful.  Instead verify the crash window registers a
    // *higher HV rank* than the post-recovery window, confirming the detector
    // correctly orders the two periods.
    let days = load_csv_closes(TSLA_CSV).expect("CSV load");
    let chron: Vec<_> = days.iter().rev().collect();
    let closes: Vec<f64> = chron.iter().map(|d| d.close).collect();
    let hv_series = rolling_hv21(&closes);

    let dates: Vec<&str> = chron.iter().map(|d| d.date.as_str()).collect();

    let hv_idx = |target: &str| -> usize {
        let close_idx = dates.iter().enumerate()
            .find(|(_, d)| **d >= target)
            .map(|(i, _)| i)
            .unwrap_or(hv_series.len() + 21);
        close_idx.saturating_sub(21).min(hv_series.len() - 1)
    };

    let crash_hv = hv_series[hv_idx(CRASH_LOW_DATE)];
    let calm_hv  = hv_series[hv_idx(CALM_REF_DATE)];

    let crash_rank = iv_rank(&hv_series, crash_hv);
    let calm_rank  = iv_rank(&hv_series, calm_hv);

    assert!(
        crash_rank > calm_rank,
        "Crash HV rank ({:.1}%) must exceed calm HV rank ({:.1}%)",
        crash_rank * 100.0, calm_rank * 100.0
    );
}

// ─── HV percentile ────────────────────────────────────────────────────────────

#[test]
fn tsla_crash_hv21_rank_above_60th_percentile() {
    let days = load_csv_closes(TSLA_CSV).expect("CSV load");
    // Chronological oldest-first for rolling_hv21
    let closes: Vec<f64> = days.iter().rev().map(|d| d.close).collect();
    let dates:  Vec<&str> = days.iter().rev().map(|d| d.date.as_str()).collect();

    let hv_series = rolling_hv21(&closes);
    assert!(!hv_series.is_empty(), "rolling_hv21 returned empty series");

    // hv_series[k] measures vol ending at closes[21 + k].
    // Find the HV value that corresponds to the crash trough.
    let crash_close_idx = dates.iter().enumerate()
        .find(|(_, d)| **d >= CRASH_LOW_DATE)
        .map(|(i, _)| i)
        .expect("CRASH_LOW_DATE must exist");

    // The HV window whose last close is crash_close_idx:
    // hv_series[k] ends at closes[21+k]  =>  k = crash_close_idx - 21
    let hv_idx = crash_close_idx.saturating_sub(21).min(hv_series.len() - 1);
    let crash_hv = hv_series[hv_idx];

    let rank = iv_rank(&hv_series, crash_hv);
    assert!(
        rank > 0.60,
        "TSLA HV-21 rank at crash trough should be >60th percentile, got {:.1}% (crash_hv={:.1}%)",
        rank * 100.0, crash_hv * 100.0
    );
}

#[test]
fn tsla_crash_vol_exceeds_calm_vol() {
    let days = load_csv_closes(TSLA_CSV).expect("CSV load");
    let chron: Vec<_> = days.iter().rev().collect();
    let closes: Vec<f64> = chron.iter().map(|d| d.close).collect();

    let crash_idx = chron.iter().enumerate()
        .find(|(_, d)| d.date.as_str() >= CRASH_LOW_DATE)
        .map(|(i, _)| i)
        .expect("crash date exists");

    let calm_idx = chron.iter().enumerate()
        .filter(|(_, d)| d.date.as_str() <= CALM_REF_DATE)
        .last()
        .map(|(i, _)| i)
        .expect("calm date exists");

    // 21-close slices for each period
    let crash_end  = (crash_idx + 1).min(closes.len());
    let crash_start = crash_end.saturating_sub(21);
    let crash_vol   = ann_rv(&closes[crash_start..crash_end]);

    let calm_end   = (calm_idx + 1).min(closes.len());
    let calm_start  = calm_end.saturating_sub(21);
    let calm_vol    = ann_rv(&closes[calm_start..calm_end]);

    assert!(
        crash_vol > calm_vol,
        "Crash period HV ({:.1}%) must exceed calm period HV ({:.1}%)",
        crash_vol * 100.0, calm_vol * 100.0
    );
}

// ─── strategy weights ─────────────────────────────────────────────────────────

#[test]
fn high_vol_weights_favour_vol_strategies_over_momentum() {
    let weights = RegimeDetector::strategy_weights(&MarketRegime::HighVol);
    let w = |name: &str| -> f64 {
        weights.iter().find(|(n, _)| *n == name).map(|(_, w)| *w).unwrap_or(1.0)
    };

    let vol_mr  = w("Vol Mean Reversion");
    let vol_arb = w("Vol Arbitrage");
    let mom     = w("Momentum");
    let brk     = w("Breakout");

    assert!(
        vol_mr > mom,
        "HighVol: Vol Mean Reversion ({}) should outweigh Momentum ({})",
        vol_mr, mom
    );
    assert!(
        vol_arb > brk,
        "HighVol: Vol Arbitrage ({}) should outweigh Breakout ({})",
        vol_arb, brk
    );
}

#[test]
fn low_vol_weights_favour_csp_over_vol_arb() {
    let weights = RegimeDetector::strategy_weights(&MarketRegime::LowVol);
    let w = |name: &str| -> f64 {
        weights.iter().find(|(n, _)| *n == name).map(|(_, w)| *w).unwrap_or(1.0)
    };

    let csp     = w("Cash-Secured Puts");
    let vol_arb = w("Vol Arbitrage");

    assert!(
        csp > vol_arb,
        "LowVol: Cash-Secured Puts ({}) should outweigh Vol Arbitrage ({})",
        csp, vol_arb
    );
}
