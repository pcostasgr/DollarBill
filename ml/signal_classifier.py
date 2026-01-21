#!/usr/bin/env python3
"""
Machine Learning Integration Example for DollarBill Options Trading Platform

This script demonstrates how to integrate ML models with the Rust-based options trading platform.
It shows signal classification, volatility prediction, and sentiment analysis integration.

Prerequisites:
pip install scikit-learn pandas numpy tensorflow transformers

Usage:
python ml/signal_classifier.py '{"edge_percent": 15.5, "delta": 0.65, "gamma": 0.004, "vega": 45.2, "theta": -8.5, "volume": 1200, "open_interest": 8500, "days_to_expiry": 45}'
"""

import json
import sys
import pandas as pd
import numpy as np
from sklearn.ensemble import RandomForestClassifier
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import accuracy_score, classification_report
import joblib
import os

class SignalClassifier:
    """ML model to classify trading signals as profitable or not"""

    def __init__(self, model_path='models/signal_classifier.pkl'):
        self.model_path = model_path
        self.model = None
        self.scaler = None
        self.is_trained = False

        # Load existing model if available
        if os.path.exists(model_path):
            try:
                self.model = joblib.load(model_path)
                self.scaler = joblib.load(model_path.replace('.pkl', '_scaler.pkl'))
                self.is_trained = True
                # Only print in interactive mode, not when called from Rust
                if __name__ == "__main__":
                    print(f"[OK] Loaded trained model from {model_path}")
            except Exception as e:
                if __name__ == "__main__":
                    print(f"[WARN] Could not load model: {e}")

    def generate_training_data(self, num_samples=10000):
        """Generate synthetic training data for demonstration"""
        np.random.seed(42)

        data = []
        for _ in range(num_samples):
            # Generate realistic signal features
            edge_percent = np.random.normal(12, 8)  # Mean 12%, std 8%
            delta = np.random.normal(0.5, 0.3)      # Typical delta range
            gamma = np.random.normal(0.005, 0.003)  # Gamma exposure
            vega = np.random.normal(50, 30)         # Vega exposure
            theta = np.random.normal(-10, 5)        # Theta decay
            volume = np.random.poisson(1000)        # Trading volume
            open_interest = np.random.poisson(5000) # Open interest
            days_to_expiry = np.random.randint(7, 90) # Time to expiry

            # Create profitability label based on realistic rules
            # Higher edge, reasonable delta, good volume = more likely profitable
            profit_probability = (
                0.3 * min(edge_percent / 20, 1) +           # Edge contribution
                0.2 * (1 - abs(delta - 0.5)) +              # Delta neutrality bonus
                0.2 * min(volume / 2000, 1) +               # Volume bonus
                0.1 * min(open_interest / 10000, 1) +       # Liquidity bonus
                0.1 * min(days_to_expiry / 60, 1) +         # Time bonus
                0.1 * np.random.random()                     # Random factor
            )

            is_profitable = profit_probability > 0.5

            data.append({
                'edge_percent': edge_percent,
                'delta': delta,
                'gamma': gamma,
                'vega': vega,
                'theta': theta,
                'volume': volume,
                'open_interest': open_interest,
                'days_to_expiry': days_to_expiry,
                'is_profitable': int(is_profitable)
            })

        return pd.DataFrame(data)

    def train(self, df=None):
        """Train the signal classification model"""
        if df is None:
            print("Generating synthetic training data...")
            df = self.generate_training_data()

        # Prepare features and target
        feature_cols = ['edge_percent', 'delta', 'gamma', 'vega', 'theta',
                       'volume', 'open_interest', 'days_to_expiry']
        X = df[feature_cols]
        y = df['is_profitable']

        # Split data
        X_train, X_test, y_train, y_test = train_test_split(
            X, y, test_size=0.2, random_state=42, stratify=y
        )

        # Scale features
        self.scaler = StandardScaler()
        X_train_scaled = self.scaler.fit_transform(X_train)
        X_test_scaled = self.scaler.transform(X_test)

        # Train Random Forest model
        self.model = RandomForestClassifier(
            n_estimators=100,
            max_depth=10,
            random_state=42,
            class_weight='balanced'
        )

        print("Training signal classifier...")
        self.model.fit(X_train_scaled, y_train)

        # Evaluate model
        y_pred = self.model.predict(X_test_scaled)
        accuracy = accuracy_score(y_test, y_pred)

        print(f"[OK] Model trained with accuracy: {accuracy:.2f}")
        print("\nClassification Report:")
        print(classification_report(y_test, y_pred,
                                  target_names=['Not Profitable', 'Profitable']))

        # Save model
        os.makedirs(os.path.dirname(self.model_path), exist_ok=True)
        joblib.dump(self.model, self.model_path)
        joblib.dump(self.scaler, self.model_path.replace('.pkl', '_scaler.pkl'))

        self.is_trained = True
        print(f"[OK] Model saved to {self.model_path}")

        return accuracy

    def predict(self, signal_features):
        """Predict if a signal is likely to be profitable"""
        if not self.is_trained:
            raise ValueError("Model not trained. Call train() first.")

        # Convert input to DataFrame with correct column order
        if isinstance(signal_features, str):
            signal_features = json.loads(signal_features)

        # Ensure correct column order matches training
        feature_cols = ['edge_percent', 'delta', 'gamma', 'vega', 'theta',
                       'volume', 'open_interest', 'days_to_expiry']
        df = pd.DataFrame([signal_features])[feature_cols]

        # Scale features
        X_scaled = self.scaler.transform(df)

        # Get prediction probability
        prob_profitable = self.model.predict_proba(X_scaled)[0][1]

        # Get prediction
        prediction = self.model.predict(X_scaled)[0]

        return {
            'is_profitable': bool(prediction),
            'confidence': float(prob_profitable),
            'recommendation': 'TRADE' if prob_profitable > 0.7 else 'AVOID'
        }

def main():
    """Main function for command-line usage"""
    if len(sys.argv) == 1:
        print("Usage:")
        print("  Train model: python signal_classifier.py --train")
        print("  Classify signal: python signal_classifier.py '<json_signal_features>'")
        return

    if len(sys.argv) == 2 and sys.argv[1] == "--train":
        # Training mode
        classifier = SignalClassifier()
        print("Training new model...")
        classifier.train()
        return

    if len(sys.argv) == 3 and sys.argv[1] == "--predict":
        # Prediction mode with file
        classifier = SignalClassifier()
        if not classifier.is_trained:
            print("Model not trained. Training first...")
            classifier.train()

        # Read features from file
        with open(sys.argv[2], 'r') as f:
            signal_features = json.load(f)

        result = classifier.predict(signal_features)
        # For Rust integration, only output the confidence score
        print(f"{result['confidence']:.3f}")
        return

    if len(sys.argv) == 2:
        # Direct JSON prediction mode
        signal_json = sys.argv[1]

        # Initialize classifier
        classifier = SignalClassifier()

        # Train model if not exists
        if not classifier.is_trained:
            print("Training new model...")
            classifier.train()

        # Make prediction
        try:
            result = classifier.predict(signal_json)
            print(f"ML Confidence: {result['confidence']:.3f}")
            print(f"Recommendation: {result['recommendation']}")

            # Output just the confidence score for Rust integration
            print(f"{result['confidence']:.3f}")

        except Exception as e:
            print(f"Error: {e}")
            sys.exit(1)
    else:
        print("Invalid arguments. Use --train or provide JSON signal features.")
        sys.exit(1)

if __name__ == "__main__":
    main()