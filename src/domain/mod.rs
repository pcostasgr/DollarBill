//! Provider-independent USD instruments, order intents and execution reports.
//! All external inputs are validated before they can change portfolio state.
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError(pub String);
impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for DomainError {}
pub type Result<T> = std::result::Result<T, DomainError>;
pub(crate) fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(DomainError(message.into())) }
}

/// Exact USD amount; a negative amount represents a debit or liability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Money(pub Decimal);

/// Per-share price (options use an explicit 100-share multiplier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "Decimal", into = "Decimal")]
pub struct Price(Decimal);
impl Price { pub fn value(self) -> Decimal { self.0 } }
impl TryFrom<Decimal> for Price {
    type Error = DomainError;
    fn try_from(value: Decimal) -> Result<Self> {
        require(value >= Decimal::ZERO, "price must be nonnegative")?;
        Ok(Self(value))
    }
}
impl From<Price> for Decimal { fn from(value: Price) -> Self { value.0 } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Quantity(u32);
impl Quantity { pub fn value(self) -> u32 { self.0 } }
impl TryFrom<u32> for Quantity {
    type Error = DomainError;
    fn try_from(value: u32) -> Result<Self> {
        require(value > 0, "quantity must be positive")?;
        Ok(Self(value))
    }
}
impl From<Quantity> for u32 { fn from(value: Quantity) -> Self { value.0 } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind { Call, Put }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionContract {
    pub underlying: String,
    /// ISO calendar date. Corporate-action adjusted contracts are not supported yet.
    pub expiry: String,
    pub kind: OptionKind,
    pub strike: Price,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Instrument { Equity { symbol: String }, Option(OptionContract) }
impl Instrument {
    pub fn validate(&self) -> Result<()> {
        let symbol = match self {
            Self::Equity { symbol } => symbol,
            Self::Option(option) => {
                require(option.strike.value() > Decimal::ZERO, "strike must be positive")?;
                let date = chrono::NaiveDate::parse_from_str(&option.expiry, "%Y-%m-%d")
                    .map_err(|_| DomainError("invalid option expiry".into()))?;
                require(date.format("%Y-%m-%d").to_string() == option.expiry, "expiry must be YYYY-MM-DD")?;
                &option.underlying
            }
        };
        require(!symbol.is_empty() && symbol.len() <= 16 && symbol.chars().all(|c| c.is_ascii_uppercase() || c == '.' || c == '-'), "invalid instrument symbol")
    }
    pub fn key(&self) -> String {
        match self {
            Self::Equity { symbol } => format!("equity:{symbol}"),
            Self::Option(c) => format!("option:{}:{}:{}:{}", c.underlying, c.expiry,
                if c.kind == OptionKind::Call { "call" } else { "put" }, c.strike.value().normalize()),
        }
    }
    pub fn multiplier(&self) -> u32 { if matches!(self, Self::Option(_)) { 100 } else { 1 } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side { Buy, Sell }
impl Side { pub fn sign(self) -> i64 { if self == Self::Buy { 1 } else { -1 } } }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderLeg {
    pub instrument: Instrument,
    pub side: Side,
    pub quantity: Quantity,
    /// Per-leg limit. None means market; this is not a net spread limit.
    pub limit: Option<Price>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderIntent {
    pub client_order_id: String,
    pub strategy_id: String,
    #[serde(default)]
    pub reduce_only: bool,
    pub legs: Vec<OrderLeg>,
}
impl OrderIntent {
    pub fn validate(&self) -> Result<()> {
        require(!self.client_order_id.trim().is_empty(), "missing client order ID")?;
        require(!self.strategy_id.trim().is_empty(), "missing strategy ID")?;
        require(!self.legs.is_empty() && self.legs.len() <= 4, "orders require one to four legs")?;
        let mut keys = std::collections::BTreeSet::new();
        for leg in &self.legs {
            leg.instrument.validate()?;
            require(keys.insert(leg.instrument.key()), "duplicate instrument in order")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fill {
    pub execution_id: String,
    pub order_id: String,
    pub leg_index: usize,
    /// Incremental quantity; adapters must convert cumulative broker reports.
    pub quantity: Quantity,
    pub price: Price,
    pub fee: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus { Created, Submitted, Accepted, PartiallyFilled, Filled, Cancelled, Rejected }
impl OrderStatus {
    pub fn is_terminal(self) -> bool { matches!(self, Self::Filled | Self::Cancelled | Self::Rejected) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub intent: OrderIntent,
    pub status: OrderStatus,
    pub filled_quantities: Vec<u32>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub instrument: Instrument,
    pub bid: Price,
    pub ask: Price,
    pub exchange_timestamp_ms: i64,
    pub received_at_ms: i64,
    /// Total executable contracts/shares on this snapshot, shared across orders.
    pub available_quantity: u32,
}

/// Normalized broker report. Fills must have broker execution IDs and incremental
/// quantities. A cumulative average price is not an individual execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerReport {
    pub report_id: String,
    pub order_id: String,
    pub status: OrderStatus,
    pub cumulative_quantities: Vec<u32>,
    pub fills: Vec<Fill>,
    pub reason: Option<String>,
}
