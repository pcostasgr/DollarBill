// Portfolio management module
// Provides position sizing, risk analytics, allocation, and performance tracking
#![allow(unused_imports)]

pub mod position_sizing;
pub mod risk_analytics;
pub mod allocation;
pub mod performance;
pub mod manager;

// Re-export key types for convenience
pub use position_sizing::{PositionSizer, MultiLegSizer, SizingMethod};
pub use risk_analytics::{RiskAnalyzer, PortfolioRisk, RiskLimits};
pub use allocation::{PortfolioAllocator, AllocationMethod, StrategyStats, StrategyAllocation};
pub use performance::{PerformanceAttribution, StrategyPerformance};
pub use manager::{PortfolioManager, PortfolioConfig, PortfolioDecision};
