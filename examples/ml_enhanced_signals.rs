use std::process::Command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ML-enhanced trading signal with AI predictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLSignal {
    pub symbol: String,
    pub signal_type: String,
    pub strike: f64,
    pub edge_percent: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub volume: u64,
    pub open_interest: u64,
    pub days_to_expiry: u64,

    // ML-enhanced fields
    pub ml_confidence: f64,
    pub ml_recommendation: String,
    pub volatility_prediction: Option<VolatilityPrediction>,
    pub risk_adjusted_score: f64,
}

/// ML service for integrating Python models
pub struct MLService {
    python_path: String,
    models_dir: String,
    cache: HashMap<String, f64>, // Simple cache for repeated predictions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityPrediction {
    pub current_avg_iv: f64,
    pub predicted_avg_iv: f64,
    pub change_percent: f64,
    pub direction: String,
}

impl MLService {
    pub fn new() -> Self {
        Self {
            python_path: "python".to_string(),
            models_dir: "ml".to_string(),
            cache: HashMap::new(),
        }
    }

    /// Classify signal quality using ML model
    pub fn classify_signal(&mut self, signal: &TradeSignal) -> Result<f64, Box<dyn std::error::Error>> {
        // Create cache key from signal characteristics
        let cache_key = format!(
            "{:.2}:{:.3}:{:.4}:{:.1}:{:.0}:{:.0}:{:.0}",
            signal.edge_percent, signal.delta, signal.gamma,
            signal.vega, signal.theta, signal.volume, signal.days_to_expiry
        );

        // Check cache first
        if let Some(confidence) = self.cache.get(&cache_key) {
            return Ok(*confidence);
        }

        // Prepare signal features for ML model
        let features = serde_json::json!({
            "edge_percent": signal.edge_percent,
            "delta": signal.delta,
            "gamma": signal.gamma,
            "vega": signal.vega,
            "theta": signal.theta,
            "volume": signal.volume,
            "open_interest": signal.open_interest,
            "days_to_expiry": signal.days_to_expiry
        });

        // Write features to a temporary file
        let temp_file = std::env::temp_dir().join(format!("ml_signal_{}.json", signal.symbol));
        std::fs::write(&temp_file, features.to_string())?;

        // Call Python ML classifier with file path
        let output = Command::new(&self.python_path)
            .arg(format!("{}/signal_classifier.py", self.models_dir))
            .arg("--predict")
            .arg(&temp_file)
            .output()?;

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_file);

        if !output.status.success() {
            return Err(format!("ML classification failed: {}",
                String::from_utf8_lossy(&output.stderr)).into());
        }

        // Parse confidence score from output
        let stdout = String::from_utf8(output.stdout)?;
        let confidence: f64 = stdout.trim().parse()
            .map_err(|_| format!("Failed to parse ML confidence score from: '{}'", stdout))?;

        // Cache the result
        self.cache.insert(cache_key, confidence);

        Ok(confidence)
    }

    /// Predict future volatility changes
    pub fn predict_volatility(&self, symbol: &str) -> Result<VolatilityPrediction, Box<dyn std::error::Error>> {
        let csv_path = format!("data/{}_vol_surface.csv", symbol.to_lowercase());

        // Check if volatility data exists
        if !std::path::Path::new(&csv_path).exists() {
            return Err(format!("Volatility data not found: {}", csv_path).into());
        }

        let output = Command::new(&self.python_path)
            .arg(format!("{}/volatility_predictor.py", self.models_dir))
            .arg("--predict")
            .arg(&csv_path)
            .output()?;

        if !output.status.success() {
            return Err(format!("Volatility prediction failed: {}",
                String::from_utf8_lossy(&output.stderr)).into());
        }

        let stdout = String::from_utf8(output.stdout)?;
        let prediction: VolatilityPrediction = serde_json::from_str(&stdout)?;

        Ok(prediction)
    }

    /// Enhance traditional signals with ML predictions
    pub fn enhance_signals(&mut self, signals: Vec<TradeSignal>) -> Result<Vec<MLSignal>, Box<dyn std::error::Error>> {
        let mut enhanced_signals = Vec::new();

        for signal in signals {
            // Get ML confidence score
            let ml_confidence = match self.classify_signal(&signal) {
                Ok(conf) => conf,
                Err(e) => {
                    eprintln!("Warning: ML classification failed for {}: {}", signal.symbol, e);
                    0.5 // Default neutral confidence
                }
            };

            // Get volatility prediction
            let vol_prediction = match self.predict_volatility(&signal.symbol) {
                Ok(pred) => Some(pred),
                Err(e) => {
                    eprintln!("Warning: Volatility prediction failed for {}: {}", signal.symbol, e);
                    None
                }
            };

            // Calculate risk-adjusted score
            let risk_adjusted_score = self.calculate_risk_adjusted_score(&signal, ml_confidence, &vol_prediction);

            // Determine ML recommendation
            let ml_recommendation = self.get_recommendation(ml_confidence, risk_adjusted_score);

            let enhanced = MLSignal {
                symbol: signal.symbol.clone(),
                signal_type: signal.signal_type.clone(),
                strike: signal.strike,
                edge_percent: signal.edge_percent,
                delta: signal.delta,
                gamma: signal.gamma,
                vega: signal.vega,
                theta: signal.theta,
                volume: signal.volume,
                open_interest: signal.open_interest,
                days_to_expiry: signal.days_to_expiry,
                ml_confidence,
                ml_recommendation,
                volatility_prediction: vol_prediction,
                risk_adjusted_score,
            };

            enhanced_signals.push(enhanced);
        }

        Ok(enhanced_signals)
    }

    /// Calculate risk-adjusted score combining traditional and ML metrics
    fn calculate_risk_adjusted_score(&self, signal: &TradeSignal, ml_confidence: f64, vol_pred: &Option<VolatilityPrediction>) -> f64 {
        // Base score from edge and Greeks
        let base_score = signal.edge_percent / 100.0;

        // ML confidence contribution
        let ml_score = ml_confidence;

        // Risk penalty for high delta exposure
        let delta_penalty = (signal.delta.abs() - 0.5).max(0.0) * 0.2;

        // Theta decay penalty (negative theta is bad for holding)
        let theta_penalty = (signal.theta.abs() / 10.0).min(1.0) * 0.1;

        // Volatility prediction bonus/penalty
        let vol_bonus = if let Some(pred) = vol_pred {
            if pred.direction == "UP" && signal.vega > 0.0 {
                0.1 // Bonus for long vega in rising vol environment
            } else if pred.direction == "DOWN" && signal.vega < 0.0 {
                0.1 // Bonus for short vega in falling vol environment
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Combine all factors
        let raw_score = base_score * 0.4 + ml_score * 0.4 + vol_bonus * 0.2;
        let penalties = delta_penalty + theta_penalty;

        (raw_score - penalties).max(0.0).min(1.0)
    }

    /// Get trading recommendation based on ML and risk metrics
    fn get_recommendation(&self, ml_confidence: f64, risk_adjusted_score: f64) -> String {
        if ml_confidence > 0.8 && risk_adjusted_score > 0.7 {
            "STRONG_BUY".to_string()
        } else if ml_confidence > 0.6 && risk_adjusted_score > 0.5 {
            "BUY".to_string()
        } else if ml_confidence < 0.4 || risk_adjusted_score < 0.3 {
            "AVOID".to_string()
        } else {
            "HOLD".to_string()
        }
    }

    /// Train ML models (call this periodically)
    pub fn train_models(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Training signal classifier...");
        let output1 = Command::new(&self.python_path)
            .arg(format!("{}/signal_classifier.py", self.models_dir))
            .arg("--train")
            .output()?;

        if !output1.status.success() {
            return Err(format!("Signal classifier training failed: {}",
                String::from_utf8_lossy(&output1.stderr)).into());
        }

        println!("Training volatility predictor...");
        let output2 = Command::new(&self.python_path)
            .arg(format!("{}/volatility_predictor.py", self.models_dir))
            .arg("--train")
            .output()?;

        if !output2.status.success() {
            return Err(format!("Volatility predictor training failed: {}",
                String::from_utf8_lossy(&output2.stderr)).into());
        }

        // Clear cache after retraining
        self.cache.clear();

        Ok(())
    }
}

/// Example usage in main trading loop
pub fn run_ml_enhanced_trading() -> Result<(), Box<dyn std::error::Error>> {
    let mut ml_service = MLService::new();

    // Train models on startup (or load pre-trained)
    println!("Initializing ML models...");
    if let Err(e) = ml_service.train_models() {
        eprintln!("Warning: ML training failed, using defaults: {}", e);
    }

    // Get traditional signals from your existing pipeline
    let traditional_signals = generate_trading_signals()?;

    // Enhance with ML predictions
    let enhanced_signals = ml_service.enhance_signals(traditional_signals)?;

    // Filter and rank signals based on ML insights
    let top_signals: Vec<_> = enhanced_signals.into_iter()
        .filter(|s| s.ml_confidence > 0.6 && s.risk_adjusted_score > 0.5)
        .take(10) // Top 10 signals
        .collect();

    // Display results
    println!("\n🤖 ML-Enhanced Trading Signals");
    println!("================================");
    for signal in &top_signals {
        println!("{} {} ${:.0} | Edge: {:.1}% | ML Conf: {:.1}% | Risk Score: {:.1}% | Rec: {}",
                 signal.symbol, signal.signal_type, signal.strike,
                 signal.edge_percent, signal.ml_confidence * 100.0,
                 signal.risk_adjusted_score * 100.0, signal.ml_recommendation);

        if let Some(vol) = &signal.volatility_prediction {
            println!("  📊 Vol Prediction: {:.1}% → {:.1}% ({:.1}%)",
                     vol.current_avg_iv * 100.0, vol.predicted_avg_iv * 100.0,
                     vol.change_percent);
        }
    }

    Ok(())
}

// Placeholder for your existing signal generation
fn generate_trading_signals() -> Result<Vec<TradeSignal>, Box<dyn std::error::Error>> {
    // This would be your existing signal generation logic
    // For demo purposes, create some sample signals
    let sample_signals = vec![
        TradeSignal {
            symbol: "AAPL".to_string(),
            signal_type: "CALL".to_string(),
            strike: 150.0,
            edge_percent: 12.5,
            delta: 0.65,
            gamma: 0.004,
            vega: 45.2,
            theta: -8.5,
            volume: 1200,
            open_interest: 8500,
            days_to_expiry: 45,
        },
        TradeSignal {
            symbol: "TSLA".to_string(),
            signal_type: "PUT".to_string(),
            strike: 200.0,
            edge_percent: 8.3,
            delta: -0.45,
            gamma: 0.003,
            vega: 38.7,
            theta: -6.2,
            volume: 2100,
            open_interest: 6200,
            days_to_expiry: 30,
        },
        TradeSignal {
            symbol: "NVDA".to_string(),
            signal_type: "CALL".to_string(),
            strike: 400.0,
            edge_percent: 15.7,
            delta: 0.72,
            gamma: 0.005,
            vega: 52.1,
            theta: -9.8,
            volume: 1800,
            open_interest: 9200,
            days_to_expiry: 60,
        },
    ];

    Ok(sample_signals)
}

// Placeholder for TradeSignal struct (from your existing code)
#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub symbol: String,
    pub signal_type: String,
    pub strike: f64,
    pub edge_percent: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub volume: u64,
    pub open_interest: u64,
    pub days_to_expiry: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_ml_enhanced_trading()
}