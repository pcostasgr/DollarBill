// Test helper utilities and fixtures

use dollarbill::models::bs_mod::Greeks;
use dollarbill::market_data::csv_loader::HistoricalDay;

/// Generate synthetic stock data for testing
pub fn generate_synthetic_stock_data(
    start_price: f64,
    days: usize,
    drift: f64,
    volatility: f64,
) -> Vec<HistoricalDay> {
    let mut data = Vec::with_capacity(days);
    let mut price = start_price;
    let dt: f64 = 1.0 / 252.0; // One trading day
    
    for i in 0..days {
        let date = format!("2024-01-{:02}", (i % 30) + 1);
        
        // Simple geometric Brownian motion simulation
        let random_shock = if i % 2 == 0 { volatility * dt.sqrt() } else { -volatility * dt.sqrt() };
        price *= 1.0 + drift * dt + random_shock;
        
        data.push(HistoricalDay {
            date: date.clone(),
            close: price,
        });
    }
    
    data
}

/// Custom assertion for Greeks validity
pub fn assert_greeks_valid(greeks: &Greeks) {
    assert!(greeks.price.is_finite(), "Price must be finite");
    assert!(greeks.price >= 0.0, "Price must be non-negative");
    assert!(greeks.delta.is_finite(), "Delta must be finite");
    assert!(greeks.gamma.is_finite(), "Gamma must be finite");
    assert!(greeks.gamma >= 0.0, "Gamma must be non-negative");
    assert!(greeks.vega.is_finite(), "Vega must be finite");
    assert!(greeks.vega >= 0.0, "Vega must be non-negative");
    assert!(greeks.theta.is_finite(), "Theta must be finite");
    assert!(greeks.rho.is_finite(), "Rho must be finite");
}

/// Assert price is reasonable (basic sanity checks)
pub fn assert_price_reasonable(price: f64, spot: f64, strike: f64) {
    assert!(price.is_finite(), "Price must be finite");
    assert!(price >= 0.0, "Price must be non-negative");
    assert!(price <= spot.max(strike) * 2.0, "Price unreasonably high");
}

/// Constant for numerical comparisons
pub const EPSILON: f64 = 1e-6;

/// Macro for approximate equality
#[macro_export]
macro_rules! assert_approx_eq {
    ($left:expr, $right:expr, $epsilon:expr) => {
        let diff = ($left - $right).abs();
        assert!(
            diff < $epsilon,
            "assertion failed: `(left ≈ right)`\n  left: `{:?}`,\n right: `{:?}`,\n  diff: `{:?}`,\n epsilon: `{:?}`",
            $left, $right, diff, $epsilon
        );
    };
    ($left:expr, $right:expr) => {
        assert_approx_eq!($left, $right, crate::helpers::EPSILON);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_stock_data_generation() {
        let data = generate_synthetic_stock_data(100.0, 10, 0.1, 0.2);
        assert_eq!(data.len(), 10);
        assert!(data[0].close > 0.0);
    }
}
