// Cash-Secured Put Strategy
use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
#[allow(dead_code)] // Part of strategy API, may be used by external code
pub struct CashSecuredPuts {
    pub premium_threshold: f64,
    pub strike_otm_pct: f64,
    pub min_iv_edge: f64,
}

impl CashSecuredPuts {
    pub fn new() -> Self {
        Self {
            premium_threshold: 0.02, // 2% minimum premium
            strike_otm_pct: 0.05,     // 5% OTM strikes
            min_iv_edge: 0.03,        // 3% minimum IV edge
        }
    }

    pub fn with_config(premium_thresh: f64, strike_otm: f64, iv_edge: f64) -> Self {
        Self {
            premium_threshold: premium_thresh,
            strike_otm_pct: strike_otm,
            min_iv_edge: iv_edge,
        }
    }
}

impl TradingStrategy for CashSecuredPuts {
    fn name(&self) -> &str {
        "Cash-Secured Puts"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        _historical_vol: f64,
    ) -> Vec<TradeSignal> {
        let mut signals = vec![];

        // Calculate edge (market IV vs model fair value)
        let iv_edge = market_iv - model_iv;

        // Strike price calculation (OTM put)
        let strike = spot * (1.0 - self.strike_otm_pct);

        // Estimate premium using Black-Scholes approximation
        // Premium ≈ spot * IV * sqrt(time) * 0.4 (rough approximation for ATM)
        let time_factor = (30.0f64 / 365.0).sqrt(); // 30 days
        let estimated_premium_pct = market_iv * time_factor * 0.4;
        let estimated_premium = spot * estimated_premium_pct;

        // Signal conditions for cash-secured puts:
        // 1. Market IV significantly above model IV (overpriced vol)
        // 2. Sufficient premium to justify the risk
        // 3. Stock in stable/accumulating phase (handled by personality matching)

        if iv_edge > self.min_iv_edge && estimated_premium_pct > self.premium_threshold {
            println!("\n💰 CASH-SECURED PUT SIGNAL: {} - Premium: ${:.2} ({:.1}%)",
                     symbol, estimated_premium, estimated_premium_pct * 100.0);
            println!("   Spot: ${:.2}, Strike: ${:.2} ({:.1}% OTM)",
                     spot, strike, self.strike_otm_pct * 100.0);
            println!("   Market IV: {:.1}%, Model IV: {:.1}%, Edge: {:.1}%",
                     market_iv * 100.0, model_iv * 100.0, iv_edge * 100.0);
            println!("   Cash Required: ${:.2} per contract", strike * 100.0);

            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::CashSecuredPut { strike_pct: self.strike_otm_pct },
                strike: strike,
                expiry_days: 30,
                confidence: (iv_edge / 0.1).min(1.0), // Scale confidence by edge
                edge: estimated_premium,
                strategy_name: self.name().to_string(),
            });
        } else {
            println!("\n⚪ NO SIGNAL: {} - Cash-Secured Puts", symbol);
            println!("   Market IV: {:.1}%, Model IV: {:.1}%, Edge: {:.1}%",
                     market_iv * 100.0, model_iv * 100.0, iv_edge * 100.0);
            println!("   Est. Premium: {:.1}% (min: {:.1}%)",
                     estimated_premium_pct * 100.0, self.premium_threshold * 100.0);
        }

        signals
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 25000.0, // Higher position size since cash-secured
            max_delta: -50.0,           // Negative delta (put selling)
            max_vega: -100.0,          // Negative vega (vol selling)
            stop_loss_pct: 2.0,        // Wider stops for cash-secured puts
        }
    }
}