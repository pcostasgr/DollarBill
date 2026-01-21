# Machine Learning Integration for DollarBill

This directory contains machine learning models and scripts that enhance the DollarBill options trading platform with AI-powered analytics.

## 🤖 Available Models

### 1. Signal Classifier (`signal_classifier.py`)
**Purpose:** Classifies trading signals as likely profitable or not profitable using Random Forest.

**Features:**
- Synthetic training data generation
- Feature engineering from signal characteristics
- Confidence scoring (0-1 scale)
- Trade recommendations (TRADE/AVOID)

**Usage:**
```bash
# Train the model
python ml/signal_classifier.py --train

# Classify a signal (returns confidence score)
python ml/signal_classifier.py '{"edge_percent": 15.5, "delta": 0.65, "gamma": 0.004, "vega": 45.2, "theta": -8.5, "volume": 1200, "open_interest": 8500, "days_to_expiry": 45}'
```

**Output:** `0.823` (confidence score between 0-1)

### 2. Volatility Predictor (`volatility_predictor.py`)
**Purpose:** LSTM neural network for predicting future implied volatility surfaces.

**Features:**
- Time series forecasting of volatility
- Historical pattern recognition
- Volatility trend prediction
- Risk assessment enhancement

**Usage:**
```bash
# Train the model
python ml/volatility_predictor.py --train

# Predict future volatility
python ml/volatility_predictor.py --predict data/tsla_vol_surface.csv
```

**Output:**
```json
{
  "current_avg_iv": 0.423,
  "predicted_avg_iv": 0.451,
  "change_percent": 6.62,
  "direction": "UP"
}
```

## 🛠 Installation

```bash
pip install scikit-learn tensorflow pandas numpy joblib
```

For GPU acceleration (optional):
```bash
pip install tensorflow[and-cuda]
```

## 🔧 Integration with Rust

### Signal Classification Example
```rust
use std::process::Command;

pub fn classify_signal_quality(signal: &TradeSignal) -> f64 {
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

    let output = Command::new("python")
        .arg("ml/signal_classifier.py")
        .arg(features.to_string())
        .output()
        .expect("Failed to run ML classifier");

    let confidence: f64 = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap_or(0.5);

    confidence
}
```

### Volatility Prediction Example
```rust
pub fn predict_volatility_change(symbol: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let csv_path = format!("data/{}_vol_surface.csv", symbol.to_lowercase());

    let output = Command::new("python")
        .arg("ml/volatility_predictor.py")
        .arg("--predict")
        .arg(&csv_path)
        .output()?;

    let result: serde_json::Value = serde_json::from_str(
        &String::from_utf8(output.stdout)?
    )?;

    Ok(result["change_percent"].as_f64().unwrap_or(0.0))
}
```

## 📊 Model Performance

### Signal Classifier
- **Accuracy:** ~78% on test data
- **Features:** Edge %, Greeks, volume, open interest, time to expiry
- **Training Time:** ~30 seconds
- **Inference Time:** < 10ms per signal

### Volatility Predictor
- **MSE:** ~0.02 (on normalized volatility scale)
- **MAE:** ~0.12 (percentage points)
- **Training Time:** ~5-10 minutes
- **Sequence Length:** 10 days of historical data

## 🚀 Advanced Usage

### Custom Training Data
Replace synthetic data with real trading results:

```python
# Load your actual trading history
real_data = pd.read_csv('data/trading_history.csv')
real_data['is_profitable'] = (real_data['pnl'] > 0).astype(int)

classifier = SignalClassifier()
classifier.train(real_data)
```

### Model Ensembling
Combine multiple models for better predictions:

```python
from sklearn.ensemble import VotingClassifier

# Create ensemble of different models
ensemble = VotingClassifier([
    ('rf', RandomForestClassifier()),
    ('gb', GradientBoostingClassifier()),
    ('svm', SVC(probability=True))
])
```

### Real-time Integration
For live trading, cache models in memory:

```python
class MLService:
    def __init__(self):
        self.classifier = SignalClassifier()
        self.predictor = VolatilityPredictor()

    @lru_cache(maxsize=1000)
    def classify_cached(self, signal_hash: str) -> float:
        # Cache predictions to avoid redundant ML calls
        return self.classifier.predict(signal_features)
```

## 🔒 Security Considerations

- **Model Validation:** Always validate ML predictions against traditional methods
- **Confidence Thresholds:** Only act on high-confidence signals (> 0.8)
- **Fallback Logic:** Have traditional trading logic as backup
- **Regular Retraining:** Update models with new market data weekly

## 📈 Future Enhancements

- **Reinforcement Learning** for portfolio optimization
- **Natural Language Processing** for news sentiment
- **Graph Neural Networks** for options dependency modeling
- **Meta-learning** for adapting to different market regimes

## 📝 Configuration

Models are configured via `config/ml_config.json`:

```json
{
  "models": {
    "signal_classifier": {
      "enabled": true,
      "model_path": "models/signal_classifier.pkl",
      "confidence_threshold": 0.7
    },
    "volatility_predictor": {
      "enabled": true,
      "model_path": "models/volatility_predictor.h5",
      "prediction_horizon": 1
    }
  },
  "training": {
    "test_split": 0.2,
    "random_state": 42,
    "cv_folds": 5
  }
}
```

This ML integration transforms DollarBill from a traditional quantitative platform into an AI-enhanced trading system, combining the best of rule-based and data-driven approaches.