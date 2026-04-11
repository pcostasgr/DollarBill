//! Portfolio-level Greeks engine with vanna/volga/charm, exposure vectors,
//! and hard-limit enforcement.
//!
//! Designed for live recomputation from scratch — does not depend on stale
//! entry-time Greeks.  All second-order cross-Greeks (vanna, volga, charm) use
//! closed-form BSM expressions.
//!
//! # Performance target
//! 20-leg book: full Greeks + exposure vectors < 2 ms (release).

use crate::models::bs_mod::{
    black_scholes_merton_call, black_scholes_merton_put, higher_order_greeks, Greeks,
    HigherOrderGreeks,
};

// ─── Option leg descriptor ───────────────────────────────────────────────────

/// A single option leg in the book.
#[derive(Debug, Clone)]
pub struct OptionLeg {
    pub strike: f64,
    pub time_to_expiry: f64,   // years
    pub sigma: f64,            // implied vol
    pub is_call: bool,
    pub quantity: i32,         // +long / −short
    pub dividend_yield: f64,   // continuous q (0.0 for most equity options)
}

// ─── Aggregated portfolio Greeks ─────────────────────────────────────────────

/// Full set of portfolio-level Greeks from a single recomputation pass.
#[derive(Debug, Clone, Default)]
pub struct PortfolioGreeks {
    // First-order
    pub net_delta: f64,
    pub net_gamma: f64,
    pub net_vega: f64,
    pub net_theta: f64,
    pub net_rho: f64,
    // Second-order cross-Greeks
    pub net_vanna: f64,        // ∂Δ/∂σ — delta sensitivity to vol
    pub net_volga: f64,        // ∂²V/∂σ² — vega convexity
    pub net_charm: f64,        // −∂Δ/∂t — delta decay
    // Third-order (from HigherOrderGreeks)
    pub net_speed: f64,
    pub net_zomma: f64,
    pub net_color: f64,
    // Portfolio value
    pub total_premium: f64,
}

impl PortfolioGreeks {
    /// Vega utilization: |net_vega| / gross_vega.
    /// 1.0 = all vega in one direction.  0.0 = perfectly offset.
    pub fn vega_utilization(&self) -> f64 {
        // Not computable from net alone; caller can supply gross_vega.
        // Provide a stub that returns 0 when vega is near-zero.
        if self.net_vega.abs() < 1e-12 { 0.0 } else { 1.0 }
    }

    /// Volga utilization: |net_volga| / max_volga (caller-supplied limit).
    pub fn volga_utilization(&self, limit: f64) -> f64 {
        if limit <= 0.0 { return 0.0; }
        (self.net_volga.abs() / limit).min(1.0)
    }
}

// ─── Exposure vectors ────────────────────────────────────────────────────────

/// Scenario-based exposure vectors: P&L impact of standardised shocks.
#[derive(Debug, Clone, Default)]
pub struct ExposureVector {
    /// P&L from +1% underlying move (≈ delta exposure).
    pub delta_1pct_up: f64,
    /// P&L from −1% underlying move.
    pub delta_1pct_down: f64,
    /// P&L from +1 vol-point parallel shift (+0.01 to all σ).
    pub vega_1pt_up: f64,
    /// P&L from −1 vol-point parallel shift.
    pub vega_1pt_down: f64,
    /// P&L from +5 vol-point parallel shift.
    pub vega_5pt_up: f64,
    /// P&L from −5 vol-point parallel shift.
    pub vega_5pt_down: f64,
}

// ─── Hard limits ─────────────────────────────────────────────────────────────

/// Portfolio-level Greek limits enforced before order submission.
#[derive(Debug, Clone)]
pub struct PortfolioLimits {
    /// Max |net delta| as fraction of equity (e.g. 0.30 = 30%).
    pub max_delta: f64,
    /// Max |net vega| in dollars per $1M notional (e.g. 500.0).
    pub max_vega: f64,
    /// Max |net volga| — tail convexity cap.
    pub max_volga: f64,
    /// Max |net charm| — overnight gamma risk cap.
    pub max_charm: f64,
}

impl Default for PortfolioLimits {
    fn default() -> Self {
        Self {
            max_delta: 0.30,
            max_vega: 500.0,
            max_volga: 200.0,
            max_charm: 50.0,
        }
    }
}

/// Result of a limit check.
#[derive(Debug, Clone)]
pub struct LimitBreach {
    pub greek: &'static str,
    pub current: f64,
    pub limit: f64,
}

// ─── Computation engine ──────────────────────────────────────────────────────

/// Compute all portfolio Greeks from scratch for the given book.
///
/// Each leg is repriced via BSM closed-form (first-order) and the analytical
/// `higher_order_greeks` function (second/third-order).  All Greeks are
/// multiplied by `quantity × 100` (options multiplier) and summed.
pub fn compute_book_greeks(spot: f64, rate: f64, legs: &[OptionLeg]) -> PortfolioGreeks {
    let mut pg = PortfolioGreeks::default();

    for leg in legs {
        let mult = leg.quantity as f64 * 100.0;
        let q = leg.dividend_yield;

        // First-order Greeks
        let g: Greeks = if leg.is_call {
            black_scholes_merton_call(spot, leg.strike, leg.time_to_expiry, rate, leg.sigma, q)
        } else {
            black_scholes_merton_put(spot, leg.strike, leg.time_to_expiry, rate, leg.sigma, q)
        };

        pg.net_delta += g.delta * mult;
        pg.net_gamma += g.gamma * mult;
        pg.net_vega  += g.vega  * mult;
        pg.net_theta += g.theta * mult;
        pg.net_rho   += g.rho   * mult;
        pg.total_premium += g.price * mult;

        // Second- and third-order Greeks
        let hog: HigherOrderGreeks = higher_order_greeks(
            spot, leg.strike, leg.time_to_expiry, rate, leg.sigma, q, leg.is_call,
        );

        pg.net_vanna += hog.vanna * mult;
        pg.net_volga += hog.volga * mult;
        pg.net_charm += hog.charm * mult;
        pg.net_speed += hog.speed * mult;
        pg.net_zomma += hog.zomma * mult;
        pg.net_color += hog.color * mult;
    }

    pg
}

/// Compute exposure vectors: bump spot and vol, reprice, measure P&L.
pub fn compute_exposure_vectors(
    spot: f64,
    rate: f64,
    legs: &[OptionLeg],
    base: &PortfolioGreeks,
) -> ExposureVector {
    let bump_spot = |pct: f64| -> f64 {
        let bumped_spot = spot * (1.0 + pct);
        let bumped = compute_book_greeks(bumped_spot, rate, legs);
        bumped.total_premium - base.total_premium
    };

    let bump_vol = |shift: f64| -> f64 {
        let bumped_legs: Vec<OptionLeg> = legs.iter().map(|l| {
            let mut bl = l.clone();
            bl.sigma = (bl.sigma + shift).max(0.001);
            bl
        }).collect();
        let bumped = compute_book_greeks(spot, rate, &bumped_legs);
        bumped.total_premium - base.total_premium
    };

    ExposureVector {
        delta_1pct_up:   bump_spot(0.01),
        delta_1pct_down: bump_spot(-0.01),
        vega_1pt_up:     bump_vol(0.01),
        vega_1pt_down:   bump_vol(-0.01),
        vega_5pt_up:     bump_vol(0.05),
        vega_5pt_down:   bump_vol(-0.05),
    }
}

/// Check portfolio Greeks against hard limits.  Returns a list of breaches
/// (empty = all clear).
pub fn check_limits(
    greeks: &PortfolioGreeks,
    limits: &PortfolioLimits,
    equity: f64,
) -> Vec<LimitBreach> {
    let mut breaches = Vec::new();

    let delta_abs = greeks.net_delta.abs();
    let delta_limit = limits.max_delta * equity;
    if delta_abs > delta_limit {
        breaches.push(LimitBreach {
            greek: "delta",
            current: delta_abs,
            limit: delta_limit,
        });
    }

    if greeks.net_vega.abs() > limits.max_vega {
        breaches.push(LimitBreach {
            greek: "vega",
            current: greeks.net_vega.abs(),
            limit: limits.max_vega,
        });
    }

    if greeks.net_volga.abs() > limits.max_volga {
        breaches.push(LimitBreach {
            greek: "volga",
            current: greeks.net_volga.abs(),
            limit: limits.max_volga,
        });
    }

    if greeks.net_charm.abs() > limits.max_charm {
        breaches.push(LimitBreach {
            greek: "charm",
            current: greeks.net_charm.abs(),
            limit: limits.max_charm,
        });
    }

    breaches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_iron_condor(spot: f64) -> Vec<OptionLeg> {
        // Short iron condor: sell OTM put spread + sell OTM call spread
        // 30-day, 25% IV
        let t = 30.0 / 365.0;
        let sigma = 0.25;
        vec![
            OptionLeg { strike: spot * 0.90, time_to_expiry: t, sigma, is_call: false, quantity: -1, dividend_yield: 0.0 }, // short put
            OptionLeg { strike: spot * 0.85, time_to_expiry: t, sigma, is_call: false, quantity:  1, dividend_yield: 0.0 }, // long put (wing)
            OptionLeg { strike: spot * 1.10, time_to_expiry: t, sigma, is_call: true,  quantity: -1, dividend_yield: 0.0 }, // short call
            OptionLeg { strike: spot * 1.15, time_to_expiry: t, sigma, is_call: true,  quantity:  1, dividend_yield: 0.0 }, // long call (wing)
        ]
    }

    #[test]
    fn iron_condor_near_delta_neutral() {
        let spot = 250.0;
        let pg = compute_book_greeks(spot, 0.045, &sample_iron_condor(spot));
        // Iron condor should be roughly delta-neutral
        assert!(pg.net_delta.abs() < 5.0,
            "Iron condor net delta should be near-zero, got {:.4}", pg.net_delta);
        // Should be short vega
        assert!(pg.net_vega < 0.0,
            "Iron condor should be short vega, got {:.4}", pg.net_vega);
        // Should have non-zero vanna and volga
        assert!(pg.net_vanna.abs() > 1e-6, "vanna should be non-zero");
        assert!(pg.net_volga.abs() > 1e-6, "volga should be non-zero");
    }

    #[test]
    fn exposure_vectors_sign_consistency() {
        let spot = 250.0;
        let legs = sample_iron_condor(spot);
        let base = compute_book_greeks(spot, 0.045, &legs);
        let ev = compute_exposure_vectors(spot, 0.045, &legs, &base);

        // For a roughly delta-neutral short premium strategy:
        // - delta exposure should be small
        // - vol down should help (short vega)
        assert!(ev.vega_1pt_down > 0.0 || base.net_vega.abs() < 1.0,
            "Short vega position should profit on vol down");
    }

    #[test]
    fn limit_check_catches_breach() {
        let pg = PortfolioGreeks {
            net_delta: 500.0,
            net_vega: 600.0,
            ..Default::default()
        };
        let limits = PortfolioLimits {
            max_delta: 0.30,
            max_vega: 500.0,
            max_volga: 200.0,
            max_charm: 50.0,
        };
        let breaches = check_limits(&pg, &limits, 1000.0);
        assert!(breaches.iter().any(|b| b.greek == "delta"), "Should breach delta");
        assert!(breaches.iter().any(|b| b.greek == "vega"), "Should breach vega");
    }

    #[test]
    fn twenty_leg_book_greeks_and_vvc() {
        // Verify all Greeks are finite for a 20-leg book
        let spot = 250.0;
        let rate = 0.045;
        let legs: Vec<OptionLeg> = (0..20).map(|i| {
            let frac = 0.85 + (i as f64) * 0.015;
            OptionLeg {
                strike: spot * frac,
                time_to_expiry: 30.0 / 365.0 + (i as f64) * 7.0 / 365.0,
                sigma: 0.20 + (i as f64) * 0.005,
                is_call: i % 2 == 0,
                quantity: if i % 3 == 0 { -1 } else { 1 },
                dividend_yield: 0.0,
            }
        }).collect();

        let pg = compute_book_greeks(spot, rate, &legs);
        assert!(pg.net_delta.is_finite());
        assert!(pg.net_gamma.is_finite());
        assert!(pg.net_vega.is_finite());
        assert!(pg.net_theta.is_finite());
        assert!(pg.net_vanna.is_finite());
        assert!(pg.net_volga.is_finite());
        assert!(pg.net_charm.is_finite());

        let ev = compute_exposure_vectors(spot, rate, &legs, &pg);
        assert!(ev.delta_1pct_up.is_finite());
        assert!(ev.vega_5pt_up.is_finite());
    }
}
