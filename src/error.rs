// Unified error type for the DollarBill library.
//
// All public functions that can fail should return `Result<T, DollarBillError>`.
// Internal helpers that are guaranteed-safe by construction may still use
// `.expect()` with a message that documents the invariant.

use std::fmt;

/// Top-level error enum covering all failure modes in the library.
#[derive(Debug)]
pub enum DollarBillError {
    /// Historical price data was empty when at least one bar was required.
    EmptyHistoricalData,

    /// A requested position could not be located by its ID.
    PositionNotFound(u64),

    /// An option pricing model received out-of-range parameters.
    InvalidPricingParams(String),

    /// The Feller condition was violated in a Heston model construction.
    FellerViolation { kappa: f64, theta: f64, sigma: f64 },

    /// A calibration or optimization routine failed to converge.
    CalibrationFailed(String),

    /// A required configuration value was missing or invalid.
    ConfigError(String),

    /// I/O or parsing failure (wraps `std::io::Error` or `serde_json` errors).
    Io(String),

    /// A computation produced a non-finite (`NaN` or `±Inf`) result.
    NonFiniteResult(String),

    /// Risk limit was breached; contains a human-readable description.
    RiskLimitBreached(String),
}

impl fmt::Display for DollarBillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHistoricalData => {
                write!(f, "historical data slice is empty")
            }
            Self::PositionNotFound(id) => {
                write!(f, "position with id={id} not found")
            }
            Self::InvalidPricingParams(msg) => {
                write!(f, "invalid pricing parameters: {msg}")
            }
            Self::FellerViolation { kappa, theta, sigma } => {
                write!(
                    f,
                    "Feller condition violated: 2κθ={:.4} < σ²={:.4} (κ={kappa}, θ={theta}, σ={sigma})",
                    2.0 * kappa * theta,
                    sigma * sigma,
                )
            }
            Self::CalibrationFailed(msg) => {
                write!(f, "calibration failed: {msg}")
            }
            Self::ConfigError(msg) => {
                write!(f, "configuration error: {msg}")
            }
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::NonFiniteResult(context) => {
                write!(f, "non-finite result in {context}")
            }
            Self::RiskLimitBreached(msg) => {
                write!(f, "risk limit breached: {msg}")
            }
        }
    }
}

impl std::error::Error for DollarBillError {}

// Convenience conversions from stdlib error types.

impl From<std::io::Error> for DollarBillError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for DollarBillError {
    fn from(e: serde_json::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for DollarBillError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        Self::Io(e.to_string())
    }
}
