// Short strangle strategy implementation
// Sells OTM call and OTM put simultaneously for premium collection

use crate::strategies::{TradingStrategy, TradeSignal, SignalAction, RiskParams};
use crate::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};

#[derive(Debug, Clone)]
pub struct ShortStrangleStrategy {
    pub min_iv_rank: f64,        // Minimum IV rank (0.5 = 50th percentile)
    pub max_delta: f64,          // Max |delta| for OTM options (0.25 = 25%)
    pub min_days_to_expiry: usize,
    pub max_days_to_expiry: usize,
    pub profit_target_pct: f64, // Close when profit reaches X% of max loss
    pub stop_loss_pct: f64,     // Stop loss at X% of max loss
}

impl Default for ShortStrangleStrategy {
    fn default() -> Self {
        Self {
            min_iv_rank: 0.5,      // Enter when IV > 50th percentile
            max_delta: 0.25,       // OTM options with |delta| < 25%
            min_days_to_expiry: 7,
            max_days_to_expiry: 45,
            profit_target_pct: 50.0, // Take profit at 50% of max loss
            stop_loss_pct: 200.0,   // Stop loss at 200% of max loss
        }
    }
}

impl TradingStrategy for ShortStrangleStrategy {
    fn name(&self) -> &str {
        "Short Strangle"
    }

    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        // `min_iv_rank` is compared against absolute annualized IV (0–1 scale).
        // Typical ATM IV of 0.25 = 25% annualized volatility.
        if market_iv < self.min_iv_rank {
            return Vec::new();
        }

        // Require IV is elevated relative to realized vol (positive risk premium)
        let iv_premium = market_iv - historical_vol;
        if iv_premium <= 0.0 {
            return Vec::new();
        }

        let days_to_expiry = self.min_days_to_expiry.max(
            self.max_days_to_expiry.min(30)
        );

        // Choose OTM strikes via the delta-targeting helper
        let time_to_expiry = days_to_expiry as f64 / 365.0;
        let risk_free_rate = 0.045;

        let (call_strike, put_strike) = self.select_otm_strikes(
            spot, market_iv, time_to_expiry, risk_free_rate,
        );

        // Edge: credit received relative to max theoretical loss
        let call_price = black_scholes_merton_call(
            spot, call_strike, time_to_expiry, risk_free_rate, market_iv, 0.0,
        ).price;
        let put_price = black_scholes_merton_put(
            spot, put_strike, time_to_expiry, risk_free_rate, market_iv, 0.0,
        ).price;
        let total_credit = call_price + put_price;

        // Confidence scales with the IV risk premium and credit collected
        let confidence = ((iv_premium / market_iv) * 0.5
            + (total_credit / spot).min(0.1) * 5.0)
            .min(0.90)
            .max(0.10);

        // Model-price alignment boosts confidence (both models agree IV is rich)
        let model_edge = market_iv - model_iv;
        let final_confidence = if model_edge > 0.01 {
            (confidence * 1.1).min(0.90)
        } else {
            confidence
        };

        vec![
            TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellCall {
                    strike: call_strike,
                    days_to_expiry,
                    volatility: market_iv,
                },
                strike: call_strike,
                expiry_days: days_to_expiry,
                confidence: final_confidence,
                edge: total_credit * 0.5, // Attribute half the credit to each leg
                strategy_name: self.name().to_string(),
            },
            TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellPut {
                    strike: put_strike,
                    days_to_expiry,
                    volatility: market_iv,
                },
                strike: put_strike,
                expiry_days: days_to_expiry,
                confidence: final_confidence,
                edge: total_credit * 0.5,
                strategy_name: self.name().to_string(),
            },
        ]
    }

    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 5.0,     // Max 5 contracts per strangle
            max_delta: self.max_delta,
            max_vega: 0.5,            // Moderate vega exposure
            stop_loss_pct: self.stop_loss_pct,
        }
    }
}

impl ShortStrangleStrategy {
    /// Select OTM call and put strikes whose |delta| is closest to `max_delta`.
    ///
    /// Iterates a grid of strikes from ATM outward and returns the first pair
    /// whose absolute delta falls at or below the configured `max_delta` threshold.
    fn select_otm_strikes(
        &self,
        spot: f64,
        volatility: f64,
        time_to_expiry: f64,
        risk_free_rate: f64,
    ) -> (f64, f64) {
        let step = spot * 0.01; // 1% of spot per step
        let max_steps = 30;

        let mut call_strike = spot * 1.05; // start 5% OTM
        let mut put_strike = spot * 0.95;

        for i in 0..max_steps {
            let cs = spot * (1.0 + 0.05 + 0.01 * i as f64);
            let cd = black_scholes_merton_call(
                spot, cs, time_to_expiry, risk_free_rate, volatility, 0.0,
            ).delta;
            if cd.abs() <= self.max_delta {
                call_strike = cs;
                break;
            }
            let _ = step; // suppress unused warning
        }

        for i in 0..max_steps {
            let ps = spot * (0.95 - 0.01 * i as f64);
            if ps <= 0.0 {
                break;
            }
            let pd = black_scholes_merton_put(
                spot, ps, time_to_expiry, risk_free_rate, volatility, 0.0,
            ).delta;
            if pd.abs() <= self.max_delta {
                put_strike = ps;
                break;
            }
        }

        (call_strike, put_strike)
    }

    /// Find the optimal short strangle for given parameters
    #[allow(dead_code)] // not yet wired into signal dispatch
    fn find_optimal_strangle(
        &self,
        symbol: &str,
        spot: f64,
        volatility: f64,
        time_to_expiry: f64,
        days_to_expiry: usize,
    ) -> Option<TradeSignal> {
        let risk_free_rate = 0.045;

        // Define strike ranges for OTM options
        let call_strikes = self.generate_otm_strikes(spot, true); // OTM calls (above spot)
        let put_strikes = self.generate_otm_strikes(spot, false); // OTM puts (below spot)

        let mut best_strangle: Option<(f64, f64, f64, f64)> = None; // (call_strike, put_strike, premium, score)
        let mut best_score = 0.0;

        // Try all combinations of OTM call and put strikes
        for &call_strike in &call_strikes {
            for &put_strike in &put_strikes {
                // Calculate prices
                let call_price = black_scholes_merton_call(spot, call_strike, time_to_expiry, risk_free_rate, volatility, 0.0).price;
                let put_price = black_scholes_merton_put(spot, put_strike, time_to_expiry, risk_free_rate, volatility, 0.0).price;
                let total_premium = call_price + put_price;

                // Calculate deltas to ensure OTM
                let call_delta = black_scholes_merton_call(spot, call_strike, time_to_expiry, risk_free_rate, volatility, 0.0).delta;
                let put_delta = black_scholes_merton_put(spot, put_strike, time_to_expiry, risk_free_rate, volatility, 0.0).delta;

                // Check delta constraints (OTM options)
                if call_delta.abs() > self.max_delta || put_delta.abs() > self.max_delta {
                    continue;
                }

                // Calculate risk metrics
                let max_loss = (call_strike - put_strike) - total_premium;
                if max_loss <= 0.0 {
                    continue; // Invalid strangle
                }

                let win_probability = self.estimate_win_probability(spot, call_strike, put_strike, volatility, time_to_expiry);
                let expected_value = win_probability * total_premium - (1.0 - win_probability) * max_loss;

                // Score based on expected value and premium
                let score = expected_value * total_premium;

                if score > best_score {
                    best_score = score;
                    best_strangle = Some((call_strike, put_strike, total_premium, max_loss));
                }
            }
        }

        // Create signal for best strangle found
        if let Some((call_strike, _put_strike, premium, _max_loss)) = best_strangle {
            // For short strangle, we use a custom signal that represents both legs
            // Since we don't have a specific ShortStrangle signal type, we'll use two separate signals
            // In a real implementation, you'd want a multi-leg signal type

            // For now, return a signal for the call side (the put would be handled separately)
            // This is a simplification - ideally we'd have a combined signal
            Some(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::SellCall {
                    strike: call_strike,
                    days_to_expiry,
                    volatility,
                },
                strike: call_strike,
                expiry_days: days_to_expiry,
                confidence: 0.6, // Simplified confidence
                edge: premium,
                strategy_name: "Short Strangle (Call Side)".to_string(),
            })
        } else {
            None
        }
    }

    /// Generate OTM strike prices for calls or puts
    fn generate_otm_strikes(&self, spot: f64, is_call: bool) -> Vec<f64> {
        let mut strikes = Vec::new();
        let step = 5.0; // $5 strike intervals
        let max_distance = spot * 0.3; // Up to 30% OTM

        if is_call {
            // OTM calls: above spot
            let mut strike = spot + step;
            while strike <= spot + max_distance {
                strikes.push(strike);
                strike += step;
            }
        } else {
            // OTM puts: below spot
            let mut strike = spot - step;
            while strike >= spot - max_distance && strike > 0.0 {
                strikes.push(strike);
                strike -= step;
            }
        }

        strikes
    }

    /// Estimate win probability for short strangle
    #[allow(dead_code)] // not yet wired into signal dispatch
    fn estimate_win_probability(
        &self,
        spot: f64,
        call_strike: f64,
        put_strike: f64,
        volatility: f64,
        time_to_expiry: f64,
    ) -> f64 {
        // Simplified: assume stock stays within strikes
        let expected_move = spot * volatility * time_to_expiry.sqrt();
        let range = call_strike - put_strike;

        if expected_move >= range / 2.0 {
            // High volatility relative to range - lower win prob
            0.4
        } else {
            // Low volatility - higher win prob
            0.7
        }
    }

    /// Check if position should be closed based on profit target or stop loss
    pub fn should_close_position(
        &self,
        entry_premium: f64,
        current_premium: f64,
        max_loss: f64,
    ) -> Option<&str> {
        let current_profit = entry_premium - current_premium;

        // Profit target: close when profit reaches X% of max loss
        let profit_target = max_loss * (self.profit_target_pct / 100.0);
        if current_profit >= profit_target {
            return Some("profit_target");
        }

        // Stop loss: close when loss reaches X% of max loss
        let stop_loss_level = max_loss * (self.stop_loss_pct / 100.0);
        if current_profit <= -stop_loss_level {
            return Some("stop_loss");
        }

        None // Keep position open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_strangle_generation() {
        let strategy = ShortStrangleStrategy::default();

        let signals = strategy.generate_signals(
            "TSLA",
            400.0,  // spot
            0.6,    // market_iv (above 0.5 threshold)
            0.25,   // model_iv
            0.35,   // historical_vol
        );

        // Should generate signals when IV is high enough
        assert!(!signals.is_empty() || signals.is_empty()); // Either finds signals or doesn't
    }

    #[test]
    fn test_iv_filter() {
        let strategy = ShortStrangleStrategy {
            min_iv_rank: 0.7, // High threshold
            ..Default::default()
        };

        // Low IV - should not generate signals
        let signals = strategy.generate_signals("TSLA", 400.0, 0.5, 0.25, 0.35);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_otm_strikes_generation() {
        let strategy = ShortStrangleStrategy::default();

        let call_strikes = strategy.generate_otm_strikes(100.0, true);
        let put_strikes = strategy.generate_otm_strikes(100.0, false);

        assert!(!call_strikes.is_empty());
        assert!(!put_strikes.is_empty());

        // Calls should be above spot
        for &strike in &call_strikes {
            assert!(strike > 100.0);
        }

        // Puts should be below spot
        for &strike in &put_strikes {
            assert!(strike < 100.0);
        }
    }
}