// Liquidity tier system and mid-price market impact.
//
// Separates two distinct liquidity effects that are often conflated:
//
//   1. Bid-ask spread (in TradingCosts) — the roundtrip cost to trade one
//      contract regardless of order size.
//
//   2. Mid-price impact — large orders move the consensus mid price before and
//      during execution.  A 100-lot sell in a thinly traded name creates supply
//      pressure that pushes the mid lower even before the ask is touched.
//
// The two effects compound: a 100-lot block trade in a MidCap name pays both a
// wider bid-ask AND suffers permanent mid-price depression.
//
// Model:  √-participation  (Almgren et al. 2005, Gatheral 2010)
//
//   total_impact = λ × √(order_value / ADV) × mid_price
//
// Typical λ values by market-cap quintile are set in `LiquidityTier`.

/// Market-cap / liquidity classification for an underlying.
///
/// Each tier bundles three empirically grounded parameters:
///   `base_half_spread_bps` — intrinsic one-way half-spread for a 1-contract order.
///   `impact_coefficient`   — Kyle's lambda (λ) for the √(dV) mid-price impact.
///   `permanent_fraction`   — fraction of impact that persists after the trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityTier {
    /// >$500 B market cap, >$2 B avg daily value.
    /// Examples: SPY, QQQ, AAPL, MSFT, NVDA, AMZN, META, GOOGL.
    /// Half-spread ≈ 3 bps; impact λ ≈ 0.03.
    MegaCap,

    /// $50 B – $500 B market cap, $200 M – $2 B ADV.
    /// Examples: TSLA, AMD, QCOM, COIN, IWM, GLD, TLT.
    /// Half-spread ≈ 10 bps; impact λ ≈ 0.08.
    LargeCap,

    /// $2 B – $50 B market cap, $10 M – $200 M ADV.
    /// Examples: PLTR, mid-size industrials.
    /// Half-spread ≈ 25 bps; impact λ ≈ 0.20.
    MidCap,

    /// $300 M – $2 B market cap, $1 M – $10 M ADV.
    /// Half-spread ≈ 70 bps; impact λ ≈ 0.50.
    SmallCap,

    /// <$300 M market cap, <$1 M ADV.
    /// Half-spread ≈ 200 bps; impact λ ≈ 1.20.
    MicroCap,
}

impl LiquidityTier {
    /// Intrinsic one-way half-spread for a single-contract order, in basis points.
    ///
    /// Add to `TradingCosts.bid_ask_spread_percent / 200` at execution time for
    /// tier-aware total half-spread.
    pub fn base_half_spread_bps(&self) -> f64 {
        match self {
            Self::MegaCap  =>   3.0,
            Self::LargeCap =>  10.0,
            Self::MidCap   =>  25.0,
            Self::SmallCap =>  70.0,
            Self::MicroCap => 200.0,
        }
    }

    /// Kyle's lambda (λ): mid-price moves by λ × √(order_value / ADV) × mid.
    ///
    /// Units: dimensionless.  Multiply by mid-price to get dollar displacement
    /// per unit of √(participation rate).
    pub fn impact_coefficient(&self) -> f64 {
        match self {
            Self::MegaCap  => 0.03,
            Self::LargeCap => 0.08,
            Self::MidCap   => 0.20,
            Self::SmallCap => 0.50,
            Self::MicroCap => 1.20,
        }
    }

    /// Fraction of mid-price impact that is permanent (survives after the trade).
    ///
    /// The remainder is temporary (bid-ask bounce, price reversal within minutes).
    /// Empirical range: 50–70% permanent across all cap tiers.
    pub fn permanent_fraction(&self) -> f64 {
        match self {
            Self::MegaCap  => 0.50,
            Self::LargeCap => 0.55,
            Self::MidCap   => 0.60,
            Self::SmallCap => 0.65,
            Self::MicroCap => 0.70,
        }
    }

    /// `cap_multiplier` compatible with `SlippageModel::FullMarketImpact`.
    ///
    /// This is `base_half_spread_bps / MegaCap.base_half_spread_bps`, so a
    /// `LiquidityTier` can be used to set `cap_multiplier` automatically instead
    /// of passing a raw number.
    pub fn cap_multiplier(&self) -> f64 {
        self.base_half_spread_bps() / LiquidityTier::MegaCap.base_half_spread_bps()
    }
}

// ─── Mid-price impact model ────────────────────────────────────────────────────

/// Mid-price market-impact calculator (Almgren et al. √-participation model).
///
/// Models the price movement caused by the trader's own order — separate from
/// the bid-ask spread.  Large orders consume depth and push the mid price
/// against the direction of the trade.
///
/// # Two impact components
///
/// * **Permanent impact** — information leakage; the market prices in the trade
///   permanently.  Affects all subsequent fills and exit prices.
/// * **Temporary impact** — execution footprint; reverses quickly, but is
///   absorbed as an up-front cost at fill time.
///
/// # Formula
///
///   total      = λ × √(order_value / ADV) × mid
///   permanent  = total × permanent_fraction(tier)
///   temporary  = total × (1 − permanent_fraction(tier))
///
/// where `order_value = |contracts| × 100 × mid_price`.
pub struct MidPriceImpact {
    /// Liquidity tier of the underlying, driving λ and permanent_fraction.
    pub tier: LiquidityTier,

    /// Average daily value traded (stock price × daily volume), in dollars.
    ///
    /// Typical: SPY $30 B, AAPL $6 B, TSLA $1.5 B, COIN $500 M, PLTR $400 M.
    pub avg_daily_value: f64,
}

impl MidPriceImpact {
    /// Create an impact calculator with explicit tier and daily trading value.
    pub fn new(tier: LiquidityTier, avg_daily_value: f64) -> Self {
        Self { tier, avg_daily_value }
    }

    /// Convenience constructor from well-known ticker symbols.
    ///
    /// Uses representative ADV figures sourced from 2024–2025 market data.
    /// Returns `None` for unknown tickers; callers should default to
    /// `MidPriceImpact::new(LiquidityTier::MidCap, 50_000_000.0)`.
    pub fn for_symbol(symbol: &str) -> Option<Self> {
        let (tier, adv) = match symbol.to_uppercase().as_str() {
            "SPY"  => (LiquidityTier::MegaCap,  30_000_000_000.0),
            "QQQ"  => (LiquidityTier::MegaCap,  15_000_000_000.0),
            "IWM"  => (LiquidityTier::MegaCap,   4_000_000_000.0),
            "AAPL" => (LiquidityTier::MegaCap,   6_000_000_000.0),
            "MSFT" => (LiquidityTier::MegaCap,   4_000_000_000.0),
            "NVDA" => (LiquidityTier::MegaCap,  12_000_000_000.0),
            "AMZN" => (LiquidityTier::MegaCap,   3_000_000_000.0),
            "META" => (LiquidityTier::MegaCap,   2_500_000_000.0),
            "GOOGL"=> (LiquidityTier::MegaCap,   2_000_000_000.0),
            "GLD"  => (LiquidityTier::LargeCap,  1_000_000_000.0),
            "TLT"  => (LiquidityTier::LargeCap,    500_000_000.0),
            "TSLA" => (LiquidityTier::LargeCap,  1_500_000_000.0),
            "AMD"  => (LiquidityTier::LargeCap,  1_200_000_000.0),
            "QCOM" => (LiquidityTier::LargeCap,    600_000_000.0),
            "COIN" => (LiquidityTier::LargeCap,    500_000_000.0),
            "PLTR" => (LiquidityTier::LargeCap,    400_000_000.0),
            _ => return None,
        };
        Some(Self::new(tier, adv))
    }

    // ─── Impact calculations ──────────────────────────────────────────────────

    /// Total mid-price displacement for an order.
    ///
    /// # Parameters
    /// * `mid_price`   — pre-trade mid-market price of the option.
    /// * `order_value` — `|contracts| × 100 × mid_price` in dollars.
    /// * `is_buying`   — direction: true = buy (mid moves up), false = sell (mid moves down).
    ///
    /// # Returns
    /// Absolute mid-price change in dollars (always ≥ 0).
    pub fn total_impact(&self, mid_price: f64, order_value: f64) -> f64 {
        let lambda = self.tier.impact_coefficient();
        let participation = (order_value / self.avg_daily_value.max(1.0)).sqrt();
        lambda * participation * mid_price
    }

    /// Permanent mid-price impact: the component that survives after the trade.
    ///
    /// Used to mark down (for a sell) or mark up (for a buy) all subsequent
    /// mid-price references in the same simulation bar.
    ///
    /// Sign convention: positive = mid-price moved against the trader.
    pub fn permanent_impact(&self, mid_price: f64, order_value: f64) -> f64 {
        self.total_impact(mid_price, order_value) * self.tier.permanent_fraction()
    }

    /// Temporary mid-price impact: the component absorbed at fill, reverting after.
    ///
    /// Represents the extra cost beyond the permanent impact paid at execution.
    pub fn temporary_impact(&self, mid_price: f64, order_value: f64) -> f64 {
        let total = self.total_impact(mid_price, order_value);
        total * (1.0 - self.tier.permanent_fraction())
    }

    /// Total execution cost from mid-price impact in dollars.
    ///
    /// This is the dollar cost above the pre-trade mid price caused purely by
    /// the trader's own order moving the market, separate from the bid-ask spread.
    ///
    /// Formula:  λ × √(order_value / ADV) × order_value
    pub fn total_impact_cost(&self, order_value: f64) -> f64 {
        let lambda = self.tier.impact_coefficient();
        let participation = (order_value / self.avg_daily_value.max(1.0)).sqrt();
        lambda * participation * order_value
    }

    /// Impact cost expressed in basis points of the order value.
    ///
    /// Useful for comparing impact cost to commission and spread cost on an
    /// apples-to-apples bps basis.
    ///
    /// Formula:  λ × √(order_value / ADV) × 10 000
    pub fn impact_cost_bps(&self, order_value: f64) -> f64 {
        let lambda = self.tier.impact_coefficient();
        let participation = (order_value / self.avg_daily_value.max(1.0)).sqrt();
        lambda * participation * 10_000.0
    }

    /// Effective mid price after applying permanent impact from an order.
    ///
    /// The mid price is pushed up (buy) or down (sell) by the permanent component.
    /// Subsequent limit orders or next-bar prices should reflect this adjustment.
    pub fn adjusted_mid(&self, mid_price: f64, order_value: f64, is_buying: bool) -> f64 {
        let perm = self.permanent_impact(mid_price, order_value);
        if is_buying {
            mid_price + perm
        } else {
            mid_price - perm
        }
    }
}

// ─── Tier comparison table (for documentation / debug output) ─────────────────

impl LiquidityTier {
    /// Ordered slice of all tiers from most to least liquid.
    pub fn all_tiers() -> [LiquidityTier; 5] {
        [
            LiquidityTier::MegaCap,
            LiquidityTier::LargeCap,
            LiquidityTier::MidCap,
            LiquidityTier::SmallCap,
            LiquidityTier::MicroCap,
        ]
    }

    /// Human-readable label for use in reports.
    pub fn label(&self) -> &'static str {
        match self {
            Self::MegaCap  => "MegaCap  (>$500B)",
            Self::LargeCap => "LargeCap ($50B-$500B)",
            Self::MidCap   => "MidCap   ($2B-$50B)",
            Self::SmallCap => "SmallCap ($300M-$2B)",
            Self::MicroCap => "MicroCap (<$300M)",
        }
    }
}
