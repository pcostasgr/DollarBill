#!/usr/bin/env python3
import sys
sys.path.append('ml')
from signal_classifier import SignalClassifier

print('Creating classifier...')
classifier = SignalClassifier()
print('Training...')
accuracy = classifier.train()
print(f'Done! Model accuracy: {accuracy:.3f}')

# Test prediction
test_signal = {
    'edge_percent': 15.5,
    'delta': 0.65,
    'gamma': 0.004,
    'vega': 45.2,
    'theta': -8.5,
    'volume': 1200,
    'open_interest': 8500,
    'days_to_expiry': 45
}

print('Testing prediction...')
result = classifier.predict(test_signal)
print(f'Prediction result: {result}')