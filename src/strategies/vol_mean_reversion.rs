// Volatility Mean Reversion Strategy
use super::{TradingStrategy, TradeSignal, SignalAction, RiskParams};

#[derive(Clone)]
pub struct VolMeanReversion {
    pub zscore_threshold: f64,
    pub edge_threshold: f64,
}

impl VolMeanReversion {
    pub fn new() -> Self {
        Self {
            zscore_threshold: 1.5,
            edge_threshold: 0.05,
        }
    }
    
    pub fn with_config(zscore: f64, edge: f64) -> Self {
        Self {
            zscore_threshold: zscore,
            edge_threshold: edge,
        }
    }
}

impl TradingStrategy for VolMeanReversion {
    fn name(&self) -> &str {
        "Vol Mean Reversion"
    }
    
    fn generate_signals(
        &self,
        symbol: &str,
        spot: f64,
        market_iv: f64,
        model_iv: f64,
        historical_vol: f64,
    ) -> Vec<TradeSignal> {
        let mut signals = vec![];
        
        // Calculate z-score (how many std devs from mean)
        let vol_mean = historical_vol;
        let vol_std = historical_vol * 0.2; // Assume 20% std dev
        let zscore = (market_iv - vol_mean) / vol_std;
        
        let edge = market_iv - model_iv;
        
        // Signal generation logic
        if zscore > self.zscore_threshold && edge > self.edge_threshold {
            println!("\n🔴 SELL SIGNAL: {} - Vol Mean Reversion", symbol);
            println!("   Market IV ({:.1}%) is {:.1} std devs above mean", 
                     market_iv * 100.0, zscore);
            println!("   Model fair value: {:.1}%", model_iv * 100.0);
            println!("   Edge: {:.1}% overpriced", edge * 100.0);
            
            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::IronButterfly { wing_width: 50.0 },
                strike: spot,
                expiry_days: 30,
                confidence: (zscore / 3.0).min(1.0),
                edge: edge * spot * 0.4, // Rough vega estimate
                strategy_name: self.name().to_string(),
            });
        } else if zscore < -self.zscore_threshold && edge < -self.edge_threshold {
            println!("\n🟢 BUY SIGNAL: {} - Vol Mean Reversion", symbol);
            println!("   Market IV ({:.1}%) is {:.1} std devs below mean", 
                     market_iv * 100.0, zscore.abs());
            println!("   Model fair value: {:.1}%", model_iv * 100.0);
            println!("   Edge: {:.1}% underpriced", edge.abs() * 100.0);
            
            signals.push(TradeSignal {
                symbol: symbol.to_string(),
                action: SignalAction::BuyStraddle,
                strike: spot,
                expiry_days: 30,
                confidence: (zscore.abs() / 3.0).min(1.0),
                edge: edge.abs() * spot * 0.4,
                strategy_name: self.name().to_string(),
            });
        } else {
            println!("\n⚪ NO SIGNAL: {} - Vol Mean Reversion", symbol);
            println!("   Market IV: {:.1}%, Model IV: {:.1}%, Z-score: {:.2}", 
                     market_iv * 100.0, model_iv * 100.0, zscore);
        }
        
        signals
    }
    
    fn risk_params(&self) -> RiskParams {
        RiskParams {
            max_position_size: 10000.0,
            max_delta: 50.0,
            max_vega: 200.0,
            stop_loss_pct: 1.5,
        }
    }
}
