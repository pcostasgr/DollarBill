#!/usr/bin/env python3
"""
Volatility Prediction Model for DollarBill Options Trading Platform

This script demonstrates LSTM-based volatility forecasting using historical
volatility surface data. It predicts future implied volatility patterns
to enhance options pricing and risk management.

Prerequisites:
pip install tensorflow pandas numpy scikit-learn

Usage:
python ml/volatility_predictor.py --train
python ml/volatility_predictor.py --predict data/tsla_vol_surface.csv
"""

import pandas as pd
import numpy as np
import tensorflow as tf
from sklearn.preprocessing import MinMaxScaler
from sklearn.metrics import mean_squared_error, mean_absolute_error
import argparse
import os
import json

class VolatilityPredictor:
    """LSTM model for predicting future volatility surfaces"""

    def __init__(self, model_path='models/volatility_predictor.h5'):
        self.model_path = model_path
        self.model = None
        self.scaler = MinMaxScaler()
        self.is_trained = False

        # Load existing model if available
        if os.path.exists(model_path):
            try:
                self.model = tf.keras.models.load_model(model_path)
                # Load scaler parameters
                if os.path.exists(model_path.replace('.h5', '_scaler.pkl')):
                    import joblib
                    self.scaler = joblib.load(model_path.replace('.h5', '_scaler.pkl'))
                self.is_trained = True
                print(f"[OK] Loaded trained model from {model_path}")
            except Exception as e:
                print(f"[WARN] Could not load model: {e}")

    def prepare_data(self, csv_path, sequence_length=10):
        """Prepare time series data for LSTM training"""
        print(f"Loading data from {csv_path}...")

        # Load volatility surface data
        df = pd.read_csv(csv_path)

        # Sort by date and strike
        df = df.sort_values(['date', 'strike'])

        # Group by date to create time series
        grouped = df.groupby('date')

        sequences = []
        targets = []

        dates = sorted(df['date'].unique())

        for i in range(len(dates) - sequence_length - 1):
            # Get sequence of volatility surfaces
            sequence_data = []
            for j in range(sequence_length):
                date = dates[i + j]
                day_data = grouped.get_group(date)[['implied_vol', 'volume']].values
                # Take mean IV and total volume for the day
                daily_iv = day_data[:, 0].mean()
                daily_volume = day_data[:, 1].sum()
                sequence_data.extend([daily_iv, daily_volume])

            # Target: next day's average IV
            next_date = dates[i + sequence_length]
            next_day_data = grouped.get_group(next_date)
            target_iv = next_day_data['implied_vol'].mean()

            sequences.append(sequence_data)
            targets.append(target_iv)

        X = np.array(sequences)
        y = np.array(targets)

        print(f"Prepared {len(X)} sequences of length {sequence_length}")

        return X, y

    def generate_synthetic_data(self, num_days=100, strikes_per_day=20):
        """Generate synthetic volatility surface data for demonstration"""
        print("Generating synthetic volatility data...")

        dates = pd.date_range('2023-01-01', periods=num_days, freq='D')
        data = []

        # Base volatility with some trends and seasonality
        base_vol = 0.3
        trend = 0.001  # Slight upward trend
        seasonal_amp = 0.05

        for i, date in enumerate(dates):
            # Add trend and seasonality
            vol_level = base_vol + trend * i + seasonal_amp * np.sin(2 * np.pi * i / 30)

            # Add random shocks
            if np.random.random() < 0.1:  # 10% chance of volatility spike
                vol_level *= (1 + np.random.uniform(0.2, 0.5))

            # Generate strikes around current price (assume ~$100 stock)
            current_price = 100 * (1 + 0.001 * i)  # Slight price appreciation
            strikes = np.linspace(current_price * 0.7, current_price * 1.3, strikes_per_day)

            for strike in strikes:
                moneyness = strike / current_price

                # Volatility smile/skew
                if moneyness < 1:
                    # Puts slightly higher vol
                    iv = vol_level * (1 + 0.1 * (1 - moneyness))
                else:
                    # Calls slightly lower vol
                    iv = vol_level * (1 + 0.05 * (moneyness - 1))

                # Add noise
                iv *= (1 + np.random.normal(0, 0.1))

                # Volume follows lognormal distribution
                volume = np.random.lognormal(6, 1)  # Mean ~400, but varies

                data.append({
                    'date': date.strftime('%Y-%m-%d'),
                    'strike': strike,
                    'implied_vol': iv,
                    'volume': volume,
                    'moneyness': moneyness
                })

        df = pd.DataFrame(data)
        return df

    def build_model(self, input_shape):
        """Build LSTM model architecture"""
        model = tf.keras.Sequential([
            tf.keras.layers.Input(shape=input_shape),
            tf.keras.layers.LSTM(64, return_sequences=True),
            tf.keras.layers.Dropout(0.2),
            tf.keras.layers.LSTM(32),
            tf.keras.layers.Dropout(0.2),
            tf.keras.layers.Dense(16, activation='relu'),
            tf.keras.layers.Dense(1)  # Predict next day's average IV
        ])

        model.compile(
            optimizer=tf.keras.optimizers.Adam(learning_rate=0.001),
            loss='mse',
            metrics=['mae']
        )

        return model

    def train(self, csv_path=None, epochs=50, batch_size=32):
        """Train the volatility prediction model"""
        if csv_path and os.path.exists(csv_path):
            df = pd.read_csv(csv_path)
        else:
            df = self.generate_synthetic_data()
            csv_path = 'data/synthetic_vol_data.csv'
            os.makedirs('data', exist_ok=True)
            df.to_csv(csv_path, index=False)
            print(f"[OK] Generated synthetic data saved to {csv_path}")

        # Prepare sequences
        X, y = self.prepare_data(csv_path)

        # Scale the data
        X_scaled = self.scaler.fit_transform(X)
        X_scaled = X_scaled.reshape(X_scaled.shape[0], -1, 1)  # Reshape for LSTM

        # Build model
        self.model = self.build_model((X_scaled.shape[1], 1))

        # Train model
        print("Training LSTM volatility predictor...")
        history = self.model.fit(
            X_scaled, y,
            epochs=epochs,
            batch_size=batch_size,
            validation_split=0.2,
            verbose=1
        )

        # Save model and scaler
        os.makedirs(os.path.dirname(self.model_path), exist_ok=True)
        self.model.save(self.model_path)

        import joblib
        joblib.dump(self.scaler, self.model_path.replace('.h5', '_scaler.pkl'))

        self.is_trained = True
        print(f"[OK] Model saved to {self.model_path}")

        # Print final metrics
        final_loss = history.history['loss'][-1]
        final_val_loss = history.history['val_loss'][-1]
        print(f"Final training loss: {final_loss:.4f}")
        print(f"Final validation loss: {final_val_loss:.4f}")
        return history

    def predict(self, csv_path, days_ahead=1):
        """Predict future volatility"""
        if not self.is_trained:
            raise ValueError("Model not trained. Call train() first.")

        # Load recent data
        df = pd.read_csv(csv_path)
        X, _ = self.prepare_data(csv_path)

        if len(X) == 0:
            raise ValueError("Not enough data for prediction")

        # Use most recent sequence
        recent_sequence = X[-1:]
        X_scaled = self.scaler.transform(recent_sequence)
        X_scaled = X_scaled.reshape(X_scaled.shape[0], -1, 1)

        # Make prediction
        predicted_iv = self.model.predict(X_scaled)[0][0]

        # Get current average IV for comparison
        current_iv = df['implied_vol'].mean()

        result = {
            'current_avg_iv': float(current_iv),
            'predicted_avg_iv': float(predicted_iv),
            'change_percent': float((predicted_iv - current_iv) / current_iv * 100),
            'direction': 'UP' if predicted_iv > current_iv else 'DOWN'
        }

        return result

def main():
    parser = argparse.ArgumentParser(description='Volatility Prediction Model')
    parser.add_argument('--train', action='store_true', help='Train the model')
    parser.add_argument('--predict', type=str, help='Predict using CSV file')
    parser.add_argument('--csv', type=str, default='data/tsla_vol_surface.csv',
                       help='CSV file for training or prediction')

    args = parser.parse_args()

    predictor = VolatilityPredictor()

    if args.train:
        predictor.train(args.csv)
    elif args.predict:
        if not predictor.is_trained:
            print("Model not trained. Training first...")
            predictor.train()

        result = predictor.predict(args.predict)
        print("[INFO] Volatility Prediction Results:")
        print(f"Current IV: {result['current_avg_iv']:.3f}")
        print(f"Predicted IV: {result['predicted_avg_iv']:.3f}")
        print(f"Change: {result['change_percent']:.2f}%")
        print(f"Direction: {result['direction']}")

        # Output JSON for Rust integration
        print(f"\n{json.dumps(result)}")

        # Output JSON for Rust integration
        print(f"\n{json.dumps(result)}")
    else:
        print("Usage:")
        print("  Train model: python volatility_predictor.py --train")
        print("  Make prediction: python volatility_predictor.py --predict data/tsla_vol_surface.csv")

if __name__ == "__main__":
    main()