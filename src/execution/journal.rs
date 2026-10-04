//! Durable orchestration boundary: persist intents before broker I/O, ingest
//! normalized execution reports atomically, resume state from committed events.
//! This component itself never submits orders or loads broker credentials.
use super::{store::{EventStore, StoreResult}, *};

pub struct ExecutionJournal {
    store: EventStore,
    state: ReplayState,
    source: String,
}

impl ExecutionJournal {
    pub async fn start(store: EventStore, run_id: &str, manifest: RunManifest, source: &str) -> StoreResult<Self> {
        require(store.events(run_id).await?.is_empty(), "run already exists; resume it instead")?;
        let event = TradingEvent::next(&ReplayState::default(), run_id, source, 0, EventPayload::RunStarted { manifest })?;
        store.append(&event).await?;
        let state = replay(&[event])?;
        Ok(Self { store, state, source: source.into() })
    }

    pub async fn resume(store: EventStore, run_id: &str, source: &str) -> StoreResult<Self> {
        require(!source.trim().is_empty(), "missing journal source")?;
        let state = replay(&store.events(run_id).await?)?;
        Ok(Self { store, state, source: source.into() })
    }
    pub fn state(&self) -> &ReplayState { &self.state }
    pub async fn events(&self) -> StoreResult<Vec<TradingEvent>> {
        self.store.events(self.state.run_id.as_deref().unwrap()).await
    }
    pub async fn close(self) { self.store.close().await; }

    async fn commit(&mut self, payloads: Vec<EventPayload>, timestamp_ms: i64) -> StoreResult<()> {
        let mut next = self.state.clone();
        let mut events = Vec::new();
        for payload in payloads {
            let event = TradingEvent::next(&next, next.run_id.as_deref().unwrap(), &self.source, timestamp_ms, payload)?;
            next.apply(&event)?;
            events.push(event);
        }
        self.store.append_batch(&events).await?;
        self.state = next; // Publish only after the durable transaction succeeds.
        Ok(())
    }

    pub async fn record(&mut self, payload: EventPayload, timestamp_ms: i64) -> StoreResult<()> {
        self.commit(vec![payload], timestamp_ms).await
    }

    /// Must succeed before the caller sends this client order ID to a broker.
    pub async fn prepare_order(&mut self, intent: OrderIntent, timestamp_ms: i64) -> StoreResult<()> {
        if let Some(existing) = self.state.orders.get(&intent.client_order_id) {
            require(existing.intent == intent, "client order ID reused with different intent")?;
            return Ok(());
        }
        self.record(EventPayload::OrderCreated { intent }, timestamp_ms).await
    }

    /// Reports carry individual executions, not estimates inferred from a
    /// cumulative average. Duplicate reports and repeated fills are idempotent.
    pub async fn ingest_report(&mut self, report: BrokerReport, received_at_ms: i64) -> StoreResult<()> {
        if let Some(existing) = self.state.broker_reports.get(&report.report_id) {
            require(existing == &report, "broker report ID reused with different contents")?;
            return Ok(());
        }
        require(!report.report_id.is_empty(), "missing broker report ID")?;
        require(!matches!(report.status, OrderStatus::Created | OrderStatus::Submitted), "report is not an execution acknowledgement")?;
        let mut shadow = self.state.clone();
        let mut payloads = Vec::new();
        // Validate the whole report against a shadow copy before writing any part.
        {
        let mut stage = |payload: EventPayload| -> Result<()> {
            let event = TradingEvent::next(&shadow, shadow.run_id.as_deref().unwrap(), &self.source, received_at_ms, payload.clone())?;
            shadow.apply(&event)?;
            payloads.push(payload);
            Ok(())
        };
        let order = self.state.orders.get(&report.order_id).ok_or_else(|| DomainError("unknown broker order; reconcile before ingestion".into()))?;
        if order.status == OrderStatus::Created {
            stage(EventPayload::OrderSubmitted { order_id: report.order_id.clone() })?;
        }
        if matches!(order.status, OrderStatus::Created | OrderStatus::Submitted) && report.status != OrderStatus::Rejected {
            stage(EventPayload::OrderAccepted { order_id: report.order_id.clone() })?;
        }
        let mut seen_in_report = BTreeSet::new();
        for fill in &report.fills {
            require(fill.order_id == report.order_id, "report contains another order's fill")?;
            require(seen_in_report.insert(fill.execution_id.clone()), "duplicate execution within report")?;
            if let Some(existing) = self.state.fills.get(&fill.execution_id) {
                require(existing == fill, "execution ID reused with different contents")?;
            } else {
                stage(EventPayload::FillReceived { fill: fill.clone() })?;
            }
        }
        }
        let current = shadow.orders.get(&report.order_id).unwrap();
        let terminal = match report.status {
            OrderStatus::Cancelled if current.status != OrderStatus::Cancelled => Some(EventPayload::OrderCancelled {
                order_id: report.order_id.clone(), reason: report.reason.clone().unwrap_or_else(|| "broker cancellation".into()) }),
            OrderStatus::Rejected if current.status != OrderStatus::Rejected => Some(EventPayload::OrderRejected {
                order_id: report.order_id.clone(), reason: report.reason.clone().unwrap_or_else(|| "broker rejection".into()) }),
            _ => None,
        };
        if let Some(payload) = terminal { payloads.push(payload); }
        // The receipt checks status and cumulative quantities against the fills.
        payloads.push(EventPayload::BrokerReportRecorded { report });
        self.commit(payloads, received_at_ms).await
    }
}
