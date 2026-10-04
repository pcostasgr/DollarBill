//! Versioned execution events, exact fill accounting and deterministic replay.
pub mod simulator;
pub mod store;
pub mod journal;
pub mod alpaca;
pub mod routing;
use crate::{build_info::BuildInfo, domain::*};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub build: BuildInfo,
    pub config_sha256: String,
    pub dataset_sha256: String,
    pub initial_cash: Money,
    pub currency: String,
    pub execution_model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventPayload {
    RunStarted { manifest: RunManifest },
    OrderCreated { intent: OrderIntent },
    OrderSubmitted { order_id: String },
    OrderAccepted { order_id: String },
    OrderRejected { order_id: String, reason: String },
    OrderCancelled { order_id: String, reason: String },
    FillReceived { fill: Fill },
    QuoteReceived { quote: Quote },
    QuoteRejected { instrument: Instrument, reason: String },
    CircuitBreakerTriggered { reason: String },
    SignalGenerated { signal_id: String, strategy_id: String, instrument: Instrument, reason: String },
    SignalRejected { signal_id: String, reason: String },
    RiskDecisionRecorded { decision_id: String, signal_id: String, approved: bool, reasons: Vec<String> },
    InvariantViolation { reason: String },
    SubmissionOutcomeUnknown { order_id: String, reason: String },
    OptionExpired { settlement_id: String, instrument: Instrument, quantity: Quantity },
    ContractExpired { instrument: Instrument },
    BrokerReportRecorded { report: BrokerReport },
    OptionAssigned { settlement_id: String, instrument: Instrument, quantity: Quantity, fee: Money },
    OptionExercised { settlement_id: String, instrument: Instrument, quantity: Quantity, fee: Money },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventContext {
    pub strategy_id: Option<String>,
    pub instrument_id: Option<String>,
    pub correlation_id: Option<String>,
    pub config_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalRecord {
    pub strategy_id: String,
    pub instrument: Instrument,
    pub rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradingEvent {
    pub schema_version: u32,
    pub run_id: String,
    pub sequence: u64,
    /// Explicit event time. Replay never reads the wall clock.
    pub timestamp_ms: i64,
    pub source: String,
    pub context: EventContext,
    pub payload: EventPayload,
}

impl TradingEvent {
    pub fn next(state: &ReplayState, run_id: &str, source: &str, timestamp_ms: i64, payload: EventPayload) -> Result<Self> {
        Ok(Self {
            schema_version: EVENT_SCHEMA_VERSION, run_id: run_id.into(), source: source.into(),
            sequence: state.last_sequence.checked_add(1).ok_or_else(|| DomainError("event sequence overflow".into()))?,
            timestamp_ms, context: event_context(state, &payload), payload,
        })
    }
}

fn event_context(state: &ReplayState, payload: &EventPayload) -> EventContext {
    let mut context = EventContext {
        strategy_id: None, instrument_id: None, correlation_id: None,
        config_sha256: state.manifest.as_ref().map(|m| m.config_sha256.clone()).unwrap_or_default(),
    };
    let order_id = match payload {
        EventPayload::RunStarted { manifest } => { context.config_sha256 = manifest.config_sha256.clone(); None }
        EventPayload::OrderCreated { intent } => {
            context.strategy_id = Some(intent.strategy_id.clone());
            context.correlation_id = Some(intent.client_order_id.clone());
            if intent.legs.len() == 1 { context.instrument_id = Some(intent.legs[0].instrument.key()); }
            None
        }
        EventPayload::OrderSubmitted { order_id } | EventPayload::OrderAccepted { order_id }
        | EventPayload::OrderRejected { order_id, .. } | EventPayload::OrderCancelled { order_id, .. }
        | EventPayload::SubmissionOutcomeUnknown { order_id, .. } => Some(order_id),
        EventPayload::FillReceived { fill } => {
            context.instrument_id = state.orders.get(&fill.order_id).and_then(|o| o.intent.legs.get(fill.leg_index)).map(|l| l.instrument.key());
            Some(&fill.order_id)
        }
        EventPayload::BrokerReportRecorded { report } => Some(&report.order_id),
        EventPayload::QuoteReceived { quote } => { context.instrument_id = Some(quote.instrument.key()); None }
        EventPayload::QuoteRejected { instrument, .. } | EventPayload::ContractExpired { instrument } => { context.instrument_id = Some(instrument.key()); None }
        EventPayload::SignalGenerated { signal_id, strategy_id, instrument, .. } => {
            context.strategy_id = Some(strategy_id.clone()); context.instrument_id = Some(instrument.key());
            context.correlation_id = Some(signal_id.clone()); None
        }
        EventPayload::SignalRejected { signal_id, .. } | EventPayload::RiskDecisionRecorded { signal_id, .. } => {
            context.correlation_id = Some(signal_id.clone());
            if let Some(signal) = state.signals.get(signal_id) {
                context.strategy_id = Some(signal.strategy_id.clone()); context.instrument_id = Some(signal.instrument.key());
            }
            None
        }
        EventPayload::OptionAssigned { settlement_id, instrument, .. } | EventPayload::OptionExercised { settlement_id, instrument, .. }
        | EventPayload::OptionExpired { settlement_id, instrument, .. } => {
            context.correlation_id = Some(settlement_id.clone()); context.instrument_id = Some(instrument.key()); None
        }
        _ => None,
    };
    if let Some(id) = order_id {
        context.correlation_id = Some(id.clone());
        if let Some(order) = state.orders.get(id) {
            context.strategy_id = Some(order.intent.strategy_id.clone());
            if context.instrument_id.is_none() && order.intent.legs.len() == 1 {
                context.instrument_id = Some(order.intent.legs[0].instrument.key());
            }
        }
    }
    context
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub instrument: Instrument,
    pub quantity: i64,
    /// Signed cost basis in USD, excluding fees.
    pub cost_basis: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Portfolio {
    pub cash: Money,
    pub realized_pnl: Money,
    pub total_fees: Money,
    pub positions: BTreeMap<String, Position>,
}

fn arithmetic(value: Option<Decimal>) -> Result<Decimal> {
    value.ok_or_else(|| DomainError("accounting overflow".into()))
}

impl Portfolio {
    fn apply_fill(&mut self, leg: &OrderLeg, fill: &Fill) -> Result<()> {
        require(fill.fee.0 >= Decimal::ZERO, "fill fee must be nonnegative")?;
        let signed_qty = i64::from(fill.quantity.value()) * leg.side.sign();
        let unit = arithmetic(fill.price.value().checked_mul(Decimal::from(leg.instrument.multiplier())))?;
        let gross = arithmetic(unit.checked_mul(Decimal::from(signed_qty)))?;
        self.cash.0 = arithmetic(arithmetic(self.cash.0.checked_sub(gross))?.checked_sub(fill.fee.0))?;
        self.total_fees.0 = arithmetic(self.total_fees.0.checked_add(fill.fee.0))?;
        self.realized_pnl.0 = arithmetic(self.realized_pnl.0.checked_sub(fill.fee.0))?;
        let key = leg.instrument.key();
        let position = self.positions.entry(key.clone()).or_insert_with(|| Position {
            instrument: leg.instrument.clone(), quantity: 0, cost_basis: Money::default(),
        });
        if position.quantity == 0 || position.quantity.signum() == signed_qty.signum() {
            position.cost_basis.0 = arithmetic(position.cost_basis.0.checked_add(gross))?;
        } else {
            let closed = position.quantity.unsigned_abs().min(signed_qty.unsigned_abs());
            let basis_closed = if closed == position.quantity.unsigned_abs() {
                position.cost_basis.0
            } else {
                arithmetic(arithmetic(position.cost_basis.0.checked_mul(Decimal::from(closed)))?
                    .checked_div(Decimal::from(position.quantity.unsigned_abs())))?
            };
            let exit_value = arithmetic(unit.checked_mul(Decimal::from(closed as i64 * position.quantity.signum())))?;
            self.realized_pnl.0 = arithmetic(self.realized_pnl.0.checked_add(arithmetic(exit_value.checked_sub(basis_closed))?))?;
            let remaining_new = signed_qty.unsigned_abs() - closed;
            position.cost_basis.0 = if remaining_new > 0 {
                arithmetic(unit.checked_mul(Decimal::from(remaining_new as i64 * signed_qty.signum())))?
            } else { arithmetic(position.cost_basis.0.checked_sub(basis_closed))? };
        }
        position.quantity = position.quantity.checked_add(signed_qty)
            .ok_or_else(|| DomainError("position quantity overflow".into()))?;
        if position.quantity == 0 { self.positions.remove(&key); }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReplayState {
    pub run_id: Option<String>,
    pub manifest: Option<RunManifest>,
    pub last_sequence: u64,
    pub last_timestamp_ms: i64,
    pub portfolio: Portfolio,
    pub orders: BTreeMap<String, Order>,
    pub execution_ids: BTreeSet<String>,
    pub fills: BTreeMap<String, Fill>,
    pub broker_reports: BTreeMap<String, BrokerReport>,
    pub settlement_ids: BTreeSet<String>,
    pub signals: BTreeMap<String, SignalRecord>,
    pub risk_decisions: BTreeSet<String>,
    pub circuit_broken: bool,
    pub expired_instruments: BTreeSet<String>,
    pub last_quote_timestamps: BTreeMap<String, i64>,
}

impl ReplayState {
    /// Validate on a copy; rejected events never leave half-applied accounting.
    pub fn apply(&mut self, event: &TradingEvent) -> Result<()> {
        let mut next = self.clone();
        next.apply_inner(event)?;
        *self = next;
        Ok(())
    }

    fn apply_inner(&mut self, event: &TradingEvent) -> Result<()> {
        require(event.schema_version == EVENT_SCHEMA_VERSION, "unsupported event schema")?;
        require(!event.run_id.trim().is_empty() && !event.source.trim().is_empty(), "missing event identity/source")?;
        require(self.last_sequence.checked_add(1) == Some(event.sequence), "event sequence gap or duplicate")?;
        require(event.timestamp_ms >= 0 && event.timestamp_ms >= self.last_timestamp_ms, "event time moved backwards")?;
        require(event.context == event_context(self, &event.payload), "event provenance disagrees with run/order context")?;
        if let Some(run_id) = &self.run_id {
            require(run_id == &event.run_id, "mixed run IDs")?;
        } else {
            require(matches!(event.payload, EventPayload::RunStarted { .. }), "first event must start a run")?;
            self.run_id = Some(event.run_id.clone());
        }
        match &event.payload {
            EventPayload::RunStarted { manifest } => {
                require(self.manifest.is_none(), "run already started")?;
                require(manifest.currency == "USD", "only USD accounting is supported")?;
                require(manifest.initial_cash.0 >= Decimal::ZERO, "negative initial cash")?;
                for hash in [&manifest.config_sha256, &manifest.dataset_sha256] {
                    require(hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()), "invalid input SHA-256")?;
                }
                self.portfolio.cash = manifest.initial_cash;
                self.manifest = Some(manifest.clone());
            }
            EventPayload::OrderCreated { intent } => {
                intent.validate()?;
                require(!self.orders.contains_key(&intent.client_order_id), "duplicate client order ID")?;
                self.orders.insert(intent.client_order_id.clone(), Order {
                    intent: intent.clone(), status: OrderStatus::Created,
                    filled_quantities: vec![0; intent.legs.len()], created_at_ms: event.timestamp_ms,
                });
            }
            EventPayload::OrderSubmitted { order_id } => {
                self.transition(order_id, &[OrderStatus::Created], OrderStatus::Submitted)?;
            }
            EventPayload::OrderAccepted { order_id } => {
                self.transition(order_id, &[OrderStatus::Submitted], OrderStatus::Accepted)?;
            }
            EventPayload::OrderRejected { order_id, reason } => {
                require(!reason.trim().is_empty(), "rejection needs a reason")?;
                self.transition(order_id, &[OrderStatus::Created, OrderStatus::Submitted, OrderStatus::Accepted], OrderStatus::Rejected)?;
            }
            EventPayload::OrderCancelled { order_id, reason } => {
                require(!reason.trim().is_empty(), "cancellation needs a reason")?;
                self.transition(order_id, &[OrderStatus::Submitted, OrderStatus::Accepted, OrderStatus::PartiallyFilled], OrderStatus::Cancelled)?;
            }
            EventPayload::FillReceived { fill } => {
                require(!fill.execution_id.trim().is_empty() && !self.execution_ids.contains(&fill.execution_id), "duplicate or missing execution ID")?;
                let order = self.orders.get_mut(&fill.order_id).ok_or_else(|| DomainError("fill for unknown order".into()))?;
                require(matches!(order.status, OrderStatus::Accepted | OrderStatus::PartiallyFilled), "fill is invalid in current order state")?;
                let leg = order.intent.legs.get(fill.leg_index).ok_or_else(|| DomainError("fill references unknown leg".into()))?;
                require(!self.expired_instruments.contains(&leg.instrument.key()), "fill after contract expiry")?;
                let qty = order.filled_quantities[fill.leg_index].checked_add(fill.quantity.value())
                    .ok_or_else(|| DomainError("fill quantity overflow".into()))?;
                require(qty <= leg.quantity.value(), "fill exceeds requested quantity")?;
                self.portfolio.apply_fill(leg, fill)?;
                order.filled_quantities[fill.leg_index] = qty;
                order.status = if order.intent.legs.iter().zip(&order.filled_quantities)
                    .all(|(leg, &filled)| leg.quantity.value() == filled) { OrderStatus::Filled } else { OrderStatus::PartiallyFilled };
                self.execution_ids.insert(fill.execution_id.clone());
                self.fills.insert(fill.execution_id.clone(), fill.clone());
            }
            EventPayload::QuoteReceived { quote } => {
                quote.instrument.validate()?;
                require(quote.bid <= quote.ask, "crossed quote in accepted event")?;
                require(quote.exchange_timestamp_ms >= 0 && quote.exchange_timestamp_ms <= quote.received_at_ms
                    && quote.received_at_ms == event.timestamp_ms, "invalid quote timestamp")?;
                require(self.last_quote_timestamps.get(&quote.instrument.key()).is_none_or(|previous| quote.exchange_timestamp_ms > *previous), "duplicate/out-of-order quote snapshot")?;
                self.last_quote_timestamps.insert(quote.instrument.key(), quote.exchange_timestamp_ms);
            }
            EventPayload::QuoteRejected { instrument, reason } => {
                instrument.validate()?;
                require(!reason.is_empty(), "quote rejection needs a reason")?;
            }
            EventPayload::CircuitBreakerTriggered { reason } => {
                require(!reason.is_empty(), "circuit breaker needs a reason")?;
                self.circuit_broken = true;
            }
            EventPayload::SignalGenerated { signal_id, strategy_id, instrument, reason } => {
                instrument.validate()?;
                require(!signal_id.is_empty() && !strategy_id.is_empty() && !reason.is_empty(), "signal requires identity and reason")?;
                require(!self.signals.contains_key(signal_id), "duplicate signal ID")?;
                self.signals.insert(signal_id.clone(), SignalRecord { strategy_id: strategy_id.clone(), instrument: instrument.clone(), rejected: false });
            }
            EventPayload::SignalRejected { signal_id, reason } => {
                require(!reason.is_empty(), "signal rejection needs a reason")?;
                let signal = self.signals.get_mut(signal_id).ok_or_else(|| DomainError("unknown signal".into()))?;
                require(!signal.rejected, "signal already rejected")?;
                signal.rejected = true;
            }
            EventPayload::RiskDecisionRecorded { decision_id, signal_id, reasons, .. } => {
                require(self.signals.contains_key(signal_id), "risk decision references unknown signal")?;
                require(!decision_id.is_empty() && !reasons.is_empty() && reasons.iter().all(|r| !r.trim().is_empty()), "risk decision needs identity and reasons")?;
                require(self.risk_decisions.insert(decision_id.clone()), "duplicate risk decision ID")?;
            }
            EventPayload::InvariantViolation { reason } => {
                require(!reason.trim().is_empty(), "invariant violation needs a reason")?;
                self.circuit_broken = true;
            }
            EventPayload::SubmissionOutcomeUnknown { order_id, reason } => {
                require(self.orders.contains_key(order_id) && !reason.trim().is_empty(), "unknown submission requires order and reason")?;
            }
            EventPayload::ContractExpired { instrument } => {
                instrument.validate()?;
                let option = match instrument { Instrument::Option(c) => c, _ => return Err(DomainError("expiry requires an option".into())) };
                let day = chrono::DateTime::from_timestamp_millis(event.timestamp_ms).ok_or_else(|| DomainError("invalid expiry clock".into()))?.date_naive();
                require(day > chrono::NaiveDate::parse_from_str(&option.expiry, "%Y-%m-%d").unwrap(), "expiry processing requires next UTC day")?;
                require(!self.portfolio.positions.contains_key(&instrument.key()), "expiry leaves unsettled position")?;
                require(self.expired_instruments.insert(instrument.key()), "contract already expired")?;
            }
            EventPayload::OptionExpired { settlement_id, instrument, quantity } => {
                require(matches!(instrument, Instrument::Option(_)), "expiry requires option")?;
                require(!settlement_id.is_empty() && self.settlement_ids.insert(settlement_id.clone()), "duplicate/missing settlement ID")?;
                let position = self.portfolio.positions.get(&instrument.key()).ok_or_else(|| DomainError("expiry of absent option".into()))?;
                require(position.quantity.unsigned_abs() >= u64::from(quantity.value()), "expiry exceeds held contracts")?;
                let side = if position.quantity < 0 { Side::Buy } else { Side::Sell };
                let leg = OrderLeg { instrument: instrument.clone(), side, quantity: *quantity, limit: None };
                let fill = Fill { execution_id: settlement_id.clone(), order_id: String::new(), leg_index: 0,
                    quantity: *quantity, price: Price::try_from(Decimal::ZERO)?, fee: Money::default() };
                self.portfolio.apply_fill(&leg, &fill)?;
            }
            EventPayload::BrokerReportRecorded { report } => {
                require(!report.report_id.is_empty() && !self.broker_reports.contains_key(&report.report_id), "duplicate broker report")?;
                let order = self.orders.get(&report.order_id).ok_or_else(|| DomainError("report references unknown order".into()))?;
                require(order.status == report.status && order.filled_quantities == report.cumulative_quantities, "broker report not reconciled with executions")?;
                for fill in &report.fills {
                    require(fill.order_id == report.order_id && self.fills.get(&fill.execution_id) == Some(fill), "broker report execution mismatch")?;
                }
                self.broker_reports.insert(report.report_id.clone(), report.clone());
            }
            EventPayload::OptionAssigned { settlement_id, instrument, quantity, fee }
            | EventPayload::OptionExercised { settlement_id, instrument, quantity, fee } => {
                require(!settlement_id.is_empty() && self.settlement_ids.insert(settlement_id.clone()), "duplicate/missing settlement ID")?;
                let option = match instrument { Instrument::Option(c) => c, _ => return Err(DomainError("settlement requires option contract".into())) };
                let position = self.portfolio.positions.get(&instrument.key()).ok_or_else(|| DomainError("settlement of absent option".into()))?;
                let assigned = matches!(event.payload, EventPayload::OptionAssigned { .. });
                require((assigned && position.quantity < 0) || (!assigned && position.quantity > 0), "assignment/exercise direction mismatch")?;
                require(position.quantity.unsigned_abs() >= quantity.value() as u64, "settlement exceeds held contracts")?;
                let close_side = if assigned { Side::Buy } else { Side::Sell };
                let stock_side = if (assigned && option.kind == OptionKind::Put) || (!assigned && option.kind == OptionKind::Call) { Side::Buy } else { Side::Sell };
                let shares = quantity.value().checked_mul(100).ok_or_else(|| DomainError("settlement quantity overflow".into()))?;
                let close = OrderLeg { instrument: instrument.clone(), side: close_side, quantity: *quantity, limit: None };
                let fill = Fill { execution_id: settlement_id.clone(), order_id: String::new(), leg_index: 0,
                    quantity: *quantity, price: Price::try_from(Decimal::ZERO)?, fee: *fee };
                self.portfolio.apply_fill(&close, &fill)?;
                let stock = OrderLeg { instrument: Instrument::Equity { symbol: option.underlying.clone() }, side: stock_side,
                    quantity: Quantity::try_from(shares)?, limit: None };
                let delivery = Fill { quantity: stock.quantity, price: option.strike, fee: Money::default(), ..fill };
                self.portfolio.apply_fill(&stock, &delivery)?;
            }
        }
        self.last_sequence = event.sequence;
        self.last_timestamp_ms = event.timestamp_ms;
        Ok(())
    }

    fn transition(&mut self, id: &str, allowed: &[OrderStatus], target: OrderStatus) -> Result<()> {
        let order = self.orders.get_mut(id).ok_or_else(|| DomainError("unknown order".into()))?;
        require(allowed.contains(&order.status), "invalid order state transition")?;
        order.status = target;
        Ok(())
    }
}

pub fn replay(events: &[TradingEvent]) -> Result<ReplayState> {
    require(!events.is_empty(), "empty event log")?;
    let mut state = ReplayState::default();
    for event in events { state.apply(event)?; }
    Ok(state)
}

pub fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
