//! Deterministic first execution model: independent legs, bid/ask fills,
//! per-snapshot liquidity, latency, limits, fees, cash checks and cancellation.
//! Explicit settlement scenarios; no exchange calendar or corporate actions.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationConfig {
    pub latency_ms: i64,
    pub max_quote_age_ms: i64,
    pub fee_per_unit: Money,
    pub slippage_per_unit: Price,
    /// Inject one ambiguous acknowledgement per listed client order ID.
    #[serde(default)]
    pub timeout_after_acceptance: BTreeSet<String>,
}
impl SimulationConfig {
    pub fn validate(&self) -> Result<()> {
        require(self.latency_ms >= 0 && self.max_quote_age_ms >= 0, "negative time constraint")?;
        require(self.fee_per_unit.0 >= Decimal::ZERO, "negative commission")
    }
}

/// Venue boundary contains no provider DTOs. Reports remain explicit events;
/// adapter acknowledgements alone cannot mutate portfolio quantities.
#[allow(async_fn_in_trait)]
pub trait ExecutionVenue {
    fn snapshot(&self) -> &ReplayState;
    async fn record(&mut self, payload: EventPayload, timestamp_ms: i64) -> Result<()>;
    async fn submit(&mut self, intent: OrderIntent, timestamp_ms: i64) -> Result<Vec<TradingEvent>>;
    async fn cancel(&mut self, order_id: &str, timestamp_ms: i64) -> Result<Vec<TradingEvent>>;
    async fn poll(&self, order_id: &str) -> Result<Order>;
}

#[derive(Debug, Clone)]
pub struct SimulatedVenue {
    state: ReplayState,
    events: Vec<TradingEvent>,
    config: SimulationConfig,
}

impl SimulatedVenue {
    pub fn new(run_id: String, manifest: RunManifest, config: SimulationConfig) -> Result<Self> {
        config.validate()?;
        let mut venue = Self { state: ReplayState::default(), events: Vec::new(), config };
        let event = TradingEvent::next(&venue.state, &run_id, "simulated_venue_v1", 0, EventPayload::RunStarted { manifest })?;
        venue.state.apply(&event)?;
        venue.events.push(event);
        Ok(venue)
    }
    pub fn state(&self) -> &ReplayState { &self.state }
    pub fn events(&self) -> &[TradingEvent] { &self.events }

    fn emit(&mut self, timestamp_ms: i64, payload: EventPayload) -> Result<()> {
        let event = TradingEvent::next(&self.state, self.state.run_id.as_deref().unwrap(), "simulated_venue_v1", timestamp_ms, payload)?;
        self.state.apply(&event)?;
        self.events.push(event);
        Ok(())
    }

    fn reduction_capacity(&self, leg: &OrderLeg) -> u32 {
        self.state.portfolio.positions.get(&leg.instrument.key())
            .filter(|p| p.quantity.signum() == -leg.side.sign())
            .map(|p| p.quantity.unsigned_abs().min(u32::MAX as u64) as u32).unwrap_or(0)
    }

    /// Invalid market snapshots are recorded as rejections, never used for fills.
    pub fn on_quote(&mut self, quote: Quote) -> Result<Vec<TradingEvent>> {
        let mut next = self.clone();
        let start = next.events.len();
        next.quote_inner(quote)?;
        let emitted = next.events[start..].to_vec();
        *self = next;
        Ok(emitted)
    }

    fn quote_inner(&mut self, quote: Quote) -> Result<()> {
        quote.instrument.validate()?;
        let timestamp = quote.received_at_ms;
        let valid_time = quote.exchange_timestamp_ms >= 0 && timestamp >= quote.exchange_timestamp_ms;
        let fresh = valid_time && timestamp.saturating_sub(quote.exchange_timestamp_ms) <= self.config.max_quote_age_ms;
        let newer = self.state.last_quote_timestamps.get(&quote.instrument.key()).is_none_or(|previous| quote.exchange_timestamp_ms > *previous);
        let not_expired = match &quote.instrument {
            Instrument::Option(c) => chrono::DateTime::from_timestamp_millis(timestamp)
                .map(|t| c.expiry >= t.date_naive().format("%Y-%m-%d").to_string()).unwrap_or(false),
            _ => true,
        };
        if !fresh || !newer || quote.bid > quote.ask || quote.ask.value() == Decimal::ZERO || !not_expired
            || self.state.expired_instruments.contains(&quote.instrument.key()) {
            return self.emit(timestamp, EventPayload::QuoteRejected { instrument: quote.instrument,
                reason: "stale, repeated, out-of-order, future, crossed, zero-ask or expired quote".into() });
        }
        self.emit(timestamp, EventPayload::QuoteReceived { quote: quote.clone() })?;
        let mut liquidity = quote.available_quantity;
        // FIFO priority is deterministic, with ID as a same-time tie-break.
        let mut orders: Vec<_> = self.state.orders.values().cloned().collect();
        orders.sort_by(|a, b| (a.created_at_ms, &a.intent.client_order_id).cmp(&(b.created_at_ms, &b.intent.client_order_id)));
        for order in orders {
            if liquidity == 0 { break; }
            if !matches!(order.status, OrderStatus::Accepted | OrderStatus::PartiallyFilled)
                || timestamp.saturating_sub(order.created_at_ms) < self.config.latency_ms { continue; }
            for (index, leg) in order.intent.legs.iter().enumerate() {
                if leg.instrument != quote.instrument { continue; }
                let remaining = leg.quantity.value() - order.filled_quantities[index];
                let mut quantity = remaining.min(liquidity);
                if order.intent.reduce_only { quantity = quantity.min(self.reduction_capacity(leg)); }
                if quantity == 0 { continue; }
                let raw_price = if leg.side == Side::Buy {
                    arithmetic(quote.ask.value().checked_add(self.config.slippage_per_unit.value()))?
                } else {
                    arithmetic(quote.bid.value().checked_sub(self.config.slippage_per_unit.value()))?.max(Decimal::ZERO)
                };
                if let Some(limit) = leg.limit {
                    if (leg.side == Side::Buy && raw_price > limit.value())
                        || (leg.side == Side::Sell && raw_price < limit.value()) { continue; }
                }
                let fee = arithmetic(self.config.fee_per_unit.0.checked_mul(Decimal::from(quantity)))?;
                let gross = arithmetic(arithmetic(raw_price.checked_mul(Decimal::from(quantity)))?
                    .checked_mul(Decimal::from(leg.instrument.multiplier())))?;
                let cash_required = if leg.side == Side::Buy {
                    arithmetic(gross.checked_add(fee))?
                } else {
                    arithmetic(fee.checked_sub(gross))?
                };
                if cash_required > self.state.portfolio.cash.0 {
                    self.emit(timestamp, EventPayload::OrderCancelled { order_id: order.intent.client_order_id.clone(),
                        reason: "insufficient cash at execution".into() })?;
                    break;
                }
                let fill = Fill {
                    execution_id: format!("{}:{}:{}", self.state.run_id.as_deref().unwrap(), order.intent.client_order_id, self.state.last_sequence + 1),
                    order_id: order.intent.client_order_id.clone(), leg_index: index,
                    quantity: Quantity::try_from(quantity)?, price: Price::try_from(raw_price)?, fee: Money(fee),
                };
                self.emit(timestamp, EventPayload::FillReceived { fill })?;
                liquidity -= quantity;
            }
        }
        Ok(())
    }

    pub fn trip_circuit_breaker(&mut self, timestamp_ms: i64, reason: String) -> Result<Vec<TradingEvent>> {
        let mut next = self.clone();
        let start = next.events.len();
        next.emit(timestamp_ms, EventPayload::CircuitBreakerTriggered { reason })?;
        let ids: Vec<_> = next.state.orders.values().filter(|o| !o.status.is_terminal() && !o.intent.reduce_only)
            .map(|o| o.intent.client_order_id.clone()).collect();
        for order_id in ids {
            next.emit(timestamp_ms, EventPayload::OrderCancelled { order_id, reason: "circuit breaker halted new risk".into() })?;
        }
        let events = next.events[start..].to_vec();
        *self = next;
        Ok(events)
    }

    /// Explicit early assignment/exercise. Pending orders remain pending and
    /// reduce-only capacity is rechecked when they next encounter a quote.
    pub fn settle(&mut self, instrument: Instrument, quantity: Quantity, assigned: bool,
        settlement_id: String, fee: Money, timestamp_ms: i64) -> Result<Vec<TradingEvent>> {
        let start = self.events.len();
        let payload = if assigned { EventPayload::OptionAssigned { settlement_id, instrument, quantity, fee } }
            else { EventPayload::OptionExercised { settlement_id, instrument, quantity, fee } };
        self.emit(timestamp_ms, payload)?;
        Ok(self.events[start..].to_vec())
    }

    /// Deterministic end-of-expiry processing, explicitly supplied settlement
    /// spot and exercise threshold. Call after the expiry date (UTC), not as a
    /// model of the exchange's intraday cutoff or contrary exercise decisions.
    pub fn expire(&mut self, instrument: Instrument, spot: Price, threshold: Price,
        fee: Money, timestamp_ms: i64) -> Result<Vec<TradingEvent>> {
        require(threshold.value() > Decimal::ZERO, "exercise threshold must be positive")?;
        require(fee.0 >= Decimal::ZERO, "negative settlement fee")?;
        instrument.validate()?;
        let contract = match &instrument { Instrument::Option(c) => c, _ => return Err(DomainError("expiry requires option".into())) };
        let day = chrono::DateTime::from_timestamp_millis(timestamp_ms).ok_or_else(|| DomainError("invalid expiry time".into()))?.date_naive();
        require(day > chrono::NaiveDate::parse_from_str(&contract.expiry, "%Y-%m-%d").unwrap(), "expiry processing requires next UTC day")?;
        require(!self.state.expired_instruments.contains(&instrument.key()), "contract already expired")?;
        let mut next = self.clone();
        let start = next.events.len();
        let orders: Vec<_> = next.state.orders.values().filter(|o| !o.status.is_terminal()
            && o.intent.legs.iter().any(|l| l.instrument == instrument)).map(|o| o.intent.client_order_id.clone()).collect();
        for order_id in orders {
            next.emit(timestamp_ms, EventPayload::OrderCancelled { order_id, reason: "contract expiry".into() })?;
        }
        if let Some(position) = next.state.portfolio.positions.get(&instrument.key()).cloned() {
            let quantity = Quantity::try_from(u32::try_from(position.quantity.unsigned_abs()).map_err(|_| DomainError("settlement quantity overflow".into()))?)?;
            let intrinsic = match contract.kind { OptionKind::Call => spot.value() - contract.strike.value(), OptionKind::Put => contract.strike.value() - spot.value() };
            let settlement_id = format!("expiry:{}:{}", next.state.run_id.as_deref().unwrap(), instrument.key());
            if intrinsic >= threshold.value() {
                next.settle(instrument.clone(), quantity, position.quantity < 0, settlement_id, fee, timestamp_ms)?;
            } else {
                next.emit(timestamp_ms, EventPayload::OptionExpired { settlement_id, instrument: instrument.clone(), quantity })?;
            }
        }
        next.emit(timestamp_ms, EventPayload::ContractExpired { instrument })?;
        let events = next.events[start..].to_vec();
        *self = next;
        Ok(events)
    }
}

impl ExecutionVenue for SimulatedVenue {
    fn snapshot(&self) -> &ReplayState { &self.state }
    async fn record(&mut self, payload: EventPayload, timestamp_ms: i64) -> Result<()> { self.emit(timestamp_ms, payload) }
    async fn submit(&mut self, intent: OrderIntent, timestamp_ms: i64) -> Result<Vec<TradingEvent>> {
        intent.validate()?;
        if let Some(existing) = self.state.orders.get(&intent.client_order_id) {
            require(existing.intent == intent, "client order ID reused for a different intent")?;
            return Ok(Vec::new()); // Idempotent retry, including after terminal status.
        }
        let mut next = self.clone();
        let start = next.events.len();
        let order_id = intent.client_order_id.clone();
        let ambiguous = next.config.timeout_after_acceptance.contains(&order_id);
        let bad_reduction = intent.reduce_only && intent.legs.iter().any(|l| next.reduction_capacity(l) < l.quantity.value());
        let halted = next.state.circuit_broken && !intent.reduce_only;
        let day = chrono::DateTime::from_timestamp_millis(timestamp_ms).ok_or_else(|| DomainError("invalid submission time".into()))?.date_naive().format("%Y-%m-%d").to_string();
        let expired = intent.legs.iter().any(|l| next.state.expired_instruments.contains(&l.instrument.key())
            || matches!(&l.instrument, Instrument::Option(c) if c.expiry < day));
        next.emit(timestamp_ms, EventPayload::OrderCreated { intent })?;
        next.emit(timestamp_ms, EventPayload::OrderSubmitted { order_id: order_id.clone() })?;
        if bad_reduction || halted || expired {
            next.emit(timestamp_ms, EventPayload::OrderRejected { order_id, reason: if expired { "expired contract".into() } else if halted {
                "circuit breaker halted new risk".into()
            } else { "reduce-only order would create exposure".into() } })?;
        } else {
            next.emit(timestamp_ms, EventPayload::OrderAccepted { order_id: order_id.clone() })?;
            if ambiguous { next.emit(timestamp_ms, EventPayload::SubmissionOutcomeUnknown { order_id,
                reason: "injected timeout after broker acceptance; poll by client order ID".into() })?; }
        }
        let events = next.events[start..].to_vec();
        *self = next;
        if ambiguous && !bad_reduction && !halted && !expired { return Err(DomainError("submission outcome unknown; poll existing client order ID".into())); }
        Ok(events)
    }
    async fn cancel(&mut self, order_id: &str, timestamp_ms: i64) -> Result<Vec<TradingEvent>> {
        let order = self.poll(order_id).await?;
        if order.status == OrderStatus::Cancelled { return Ok(Vec::new()); }
        let start = self.events.len();
        self.emit(timestamp_ms, EventPayload::OrderCancelled { order_id: order_id.into(), reason: "requested cancellation".into() })?;
        Ok(self.events[start..].to_vec())
    }
    async fn poll(&self, order_id: &str) -> Result<Order> {
        self.state.orders.get(order_id).cloned().ok_or_else(|| DomainError("unknown order".into()))
    }
}
