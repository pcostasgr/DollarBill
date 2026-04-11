// Calibration module - fits model parameters to market data
// Optimizer: CMA-ES (replaces the old custom Nelder-Mead simplex)
#![allow(unused_imports)]

pub mod market_option;
pub mod heston_calibrator;
pub mod cmaes;

pub use market_option::{MarketOption, OptionType, LiquidityFilter};
pub use heston_calibrator::{calibrate_heston, CalibrationResult, CalibParams, create_mock_market_data};
pub use cmaes::{Cmaes, CmaesConfig, CmaesResult};
