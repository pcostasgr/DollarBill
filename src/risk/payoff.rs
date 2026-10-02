/// Payoff-aware option risk classification.
///
/// Replaces notional approximations like `strike × |qty| × 100` with the actual
/// worst-case expiration payoff for a combination of options on the same
/// underlying + expiry. See TestFiles/DollarBill_RECOMMENDED_CHANGES.md #3/#4.
///
/// The expiration payoff of any combination of calls/puts on one underlying is
/// piecewise-linear in the spot price, with slope changes only at the strikes
/// held. Its minimum over `[0, ∞)` — the worst-case loss — therefore always
/// occurs either at spot = 0 or at one of the held strikes, UNLESS the net
/// call quantity is negative (uncovered short calls), in which case loss keeps
/// growing as spot → ∞ and risk is unbounded. This lets one general algorithm
/// correctly handle naked legs, vertical spreads, iron condors, and butterflies
/// without hand-written formulas per structure.
use std::collections::HashMap;

/// One option leg's exposure, scoped to a single underlying + expiry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionExposure {
    pub is_call: bool,
    pub strike: f64,
    /// Signed contract count: positive = long, negative = short.
    pub quantity: i32,
}

/// Worst-case loss classification for a group of option exposures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskClassification {
    /// Loss is capped at `max_loss` dollars (ignoring premium received/paid).
    DefinedRisk { max_loss: f64 },
    /// `defined_loss` dollars are capped, but `naked_quantity` contracts remain
    /// uncovered short calls with unbounded upside loss.
    PartiallyDefined { defined_loss: f64, naked_quantity: i32 },
    /// No long calls at all cap the short calls in this group — loss is
    /// unbounded as the underlying rallies.
    UnboundedRisk,
}

impl RiskClassification {
    /// Dollar loss that is actually capped (0.0 for pure `UnboundedRisk`).
    pub fn defined_dollars(&self) -> f64 {
        match self {
            RiskClassification::DefinedRisk { max_loss } => *max_loss,
            RiskClassification::PartiallyDefined { defined_loss, .. } => *defined_loss,
            RiskClassification::UnboundedRisk => 0.0,
        }
    }

    /// True when any part of this group's risk is unbounded.
    pub fn has_unbounded_risk(&self) -> bool {
        matches!(
            self,
            RiskClassification::UnboundedRisk | RiskClassification::PartiallyDefined { .. }
        )
    }
}

/// Classify a single underlying+expiry group of option exposures.
pub fn classify_group(exposures: &[OptionExposure]) -> RiskClassification {
    if exposures.is_empty() {
        return RiskClassification::DefinedRisk { max_loss: 0.0 };
    }

    // Net call quantity determines the payoff's slope as spot -> infinity:
    // each long call contributes +1 share of slope, each short call -1.
    let net_call_qty: i32 = exposures.iter()
        .filter(|e| e.is_call)
        .map(|e| e.quantity)
        .sum();
    let has_any_long_call = exposures.iter().any(|e| e.is_call && e.quantity > 0);

    if net_call_qty < 0 && !has_any_long_call {
        return RiskClassification::UnboundedRisk;
    }

    // The piecewise-linear payoff's minimum on a bounded (or fully-covered)
    // book always lands at spot = 0 or at one of the held strikes.
    let mut candidates: Vec<f64> = exposures.iter().map(|e| e.strike).collect();
    candidates.push(0.0);

    let payoff_at = |spot: f64| -> f64 {
        exposures.iter().map(|e| {
            let intrinsic = if e.is_call {
                (spot - e.strike).max(0.0)
            } else {
                (e.strike - spot).max(0.0)
            };
            intrinsic * e.quantity as f64
        }).sum()
    };

    let worst_payoff = candidates.iter()
        .map(|&s| payoff_at(s))
        .fold(f64::INFINITY, f64::min);
    let defined_loss = (-worst_payoff).max(0.0) * 100.0;

    if net_call_qty < 0 {
        RiskClassification::PartiallyDefined { defined_loss, naked_quantity: -net_call_qty }
    } else {
        RiskClassification::DefinedRisk { max_loss: defined_loss }
    }
}

/// A fully-identified option exposure (adds the grouping key to `OptionExposure`).
#[derive(Debug, Clone)]
pub struct IdentifiedExposure {
    pub root: String,
    pub expiry: String,
    pub exposure: OptionExposure,
}

/// Portfolio-wide worst-case loss across all underlying+expiry groups.
///
/// Returns the total defined (capped) dollar loss and whether any group
/// carries unbounded risk (naked short calls with insufficient long-call
/// coverage).
pub fn portfolio_max_loss(positions: &[IdentifiedExposure]) -> (f64, bool) {
    let mut groups: HashMap<(&str, &str), Vec<OptionExposure>> = HashMap::new();
    for p in positions {
        groups.entry((p.root.as_str(), p.expiry.as_str())).or_default().push(p.exposure);
    }

    let mut total_defined = 0.0;
    let mut any_unbounded = false;
    for exposures in groups.values() {
        let classification = classify_group(exposures);
        any_unbounded |= classification.has_unbounded_risk();
        total_defined += classification.defined_dollars();
    }
    (total_defined, any_unbounded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(strike: f64, qty: i32) -> OptionExposure {
        OptionExposure { is_call: true, strike, quantity: qty }
    }
    fn put(strike: f64, qty: i32) -> OptionExposure {
        OptionExposure { is_call: false, strike, quantity: qty }
    }

    #[test]
    fn empty_group_is_zero_defined_risk() {
        assert_eq!(classify_group(&[]), RiskClassification::DefinedRisk { max_loss: 0.0 });
    }

    #[test]
    fn naked_short_call_is_unbounded() {
        let group = [call(100.0, -1)];
        assert_eq!(classify_group(&group), RiskClassification::UnboundedRisk);
    }

    #[test]
    fn naked_short_put_is_bounded_by_strike() {
        // No premium netting here — pure intrinsic worst case at spot = 0.
        let group = [put(100.0, -1)];
        let c = classify_group(&group);
        assert_eq!(c, RiskClassification::DefinedRisk { max_loss: 10_000.0 });
    }

    #[test]
    fn call_does_not_hedge_put_automatically() {
        // +10 calls and -1 put are unrelated payoffs — the put side is naked
        // and the call side is a simple long (zero risk), so overall this
        // specific combination must NOT be reported as fully hedged/defined
        // with zero loss; the short put alone carries defined (bounded) risk.
        let group = [call(150.0, 10), put(140.0, -1)];
        let c = classify_group(&group);
        match c {
            RiskClassification::DefinedRisk { max_loss } => assert!(max_loss > 0.0,
                "a naked short put must still contribute nonzero defined risk"),
            other => panic!("expected DefinedRisk, got {other:?}"),
        }
    }

    #[test]
    fn vertical_call_spread_max_loss_is_width_times_100() {
        let group = [call(100.0, -1), call(105.0, 1)];
        assert_eq!(classify_group(&group), RiskClassification::DefinedRisk { max_loss: 500.0 });
    }

    #[test]
    fn iron_condor_max_loss_uses_wider_wing() {
        // Put wing = 5 (90/95), call wing = 5 (105/110) -> both equal, 500.
        let group = [put(90.0, 1), put(95.0, -1), call(105.0, -1), call(110.0, 1)];
        assert_eq!(classify_group(&group), RiskClassification::DefinedRisk { max_loss: 500.0 });
    }

    #[test]
    fn iron_condor_asymmetric_wings_uses_wider_one() {
        // Call wing = 3 (105/108), put wing = 15 (95/80) -> wider wing wins.
        let group = [put(80.0, 1), put(95.0, -1), call(105.0, -1), call(108.0, 1)];
        assert_eq!(classify_group(&group), RiskClassification::DefinedRisk { max_loss: 1_500.0 });
    }

    #[test]
    fn partial_quantity_coverage_leaves_naked_remainder() {
        // 10 short calls, 6 long calls at a wider strike -> 4 remain naked.
        let group = [call(100.0, -10), call(105.0, 6)];
        match classify_group(&group) {
            RiskClassification::PartiallyDefined { naked_quantity, .. } => {
                assert_eq!(naked_quantity, 4);
            }
            other => panic!("expected PartiallyDefined, got {other:?}"),
        }
    }

    #[test]
    fn full_quantity_coverage_is_defined_risk() {
        // 6 short calls, 6 long calls at a wider strike -> fully covered.
        let group = [call(100.0, -6), call(105.0, 6)];
        assert_eq!(classify_group(&group), RiskClassification::DefinedRisk { max_loss: 3_000.0 });
    }

    #[test]
    fn portfolio_max_loss_aggregates_across_groups_and_flags_unbounded() {
        let positions = vec![
            IdentifiedExposure { root: "AAPL".into(), expiry: "2026-12-18".into(), exposure: call(100.0, -1) },
            IdentifiedExposure { root: "TSLA".into(), expiry: "2026-12-18".into(), exposure: put(90.0, 1) },
            IdentifiedExposure { root: "TSLA".into(), expiry: "2026-12-18".into(), exposure: put(95.0, -1) },
        ];
        let (defined, unbounded) = portfolio_max_loss(&positions);
        assert!(unbounded, "AAPL naked short call must flag portfolio-level unbounded risk");
        assert_eq!(defined, 500.0, "TSLA put spread's defined loss must still be counted");
    }
}
