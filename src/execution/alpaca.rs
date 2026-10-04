//! Explicit paper/live adapters. No environment-driven activation and no
//! implicit resubmission after an ambiguous HTTP outcome. Stream executions
//! (not cumulative average prices) are the accounting source of truth.
use super::{journal::ExecutionJournal, simulator::ExecutionVenue, *};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlpacaEnvironment { Paper, Live }

#[allow(async_fn_in_trait)]
pub trait AlpacaTransport {
    async fn submit(&mut self, request: Value) -> Result<Value>;
    async fn lookup(&self, client_order_id: &str) -> Result<Value>;
    async fn cancel(&mut self, broker_order_id: &str) -> Result<()>;
}

pub struct AlpacaHttp {
    client: reqwest::Client,
    base: &'static str,
}
impl AlpacaHttp {
    pub fn new(environment: AlpacaEnvironment, key: &str, secret: &str) -> Result<Self> {
        use reqwest::header::{HeaderMap, HeaderValue};
        require(!key.is_empty() && !secret.is_empty(), "missing Alpaca credentials")?;
        let mut headers = HeaderMap::new();
        for (name, value) in [("APCA-API-KEY-ID", key), ("APCA-API-SECRET-KEY", secret)] {
            let mut header = HeaderValue::from_str(value).map_err(|_| DomainError("invalid credential header".into()))?;
            header.set_sensitive(true);
            headers.insert(name, header);
        }
        let client = reqwest::Client::builder().default_headers(headers).timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none()).build().map_err(|_| DomainError("HTTP client creation failed".into()))?;
        Ok(Self { client, base: match environment { AlpacaEnvironment::Paper => "https://paper-api.alpaca.markets", AlpacaEnvironment::Live => "https://api.alpaca.markets" } })
    }
    async fn response(request: reqwest::RequestBuilder) -> Result<Value> {
        let response = request.send().await.map_err(|_| DomainError("Alpaca transport outcome unknown; reconcile by client order ID".into()))?;
        let status = response.status();
        require(status.is_success(), &format!("Alpaca HTTP {status}; reconcile before retry"))?;
        response.json().await.map_err(|_| DomainError("invalid Alpaca response; reconcile".into()))
    }
}
impl AlpacaTransport for AlpacaHttp {
    async fn submit(&mut self, request: Value) -> Result<Value> {
        Self::response(self.client.post(format!("{}/v2/orders", self.base)).json(&request)).await
    }
    async fn lookup(&self, client_order_id: &str) -> Result<Value> {
        Self::response(self.client.get(format!("{}/v2/orders:by_client_order_id", self.base)).query(&[("client_order_id", client_order_id)])).await
    }
    async fn cancel(&mut self, broker_order_id: &str) -> Result<()> {
        require(broker_order_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "invalid broker ID")?;
        let response = self.client.delete(format!("{}/v2/orders/{broker_order_id}", self.base)).send().await
            .map_err(|_| DomainError("cancel outcome unknown; reconcile".into()))?;
        require(response.status().is_success(), "cancel not confirmed; reconcile trade updates")
    }
}

pub fn symbol(instrument: &Instrument) -> Result<String> {
    instrument.validate()?;
    match instrument {
        Instrument::Equity { symbol } => Ok(symbol.clone()),
        Instrument::Option(c) => {
            require(c.underlying.len() <= 6, "OCC root too long")?;
            let strike = arithmetic(c.strike.value().checked_mul(Decimal::from(1000)))?;
            require(strike.fract() == Decimal::ZERO && strike < Decimal::from(100_000_000), "strike cannot be represented exactly in OCC")?;
            let date = chrono::NaiveDate::parse_from_str(&c.expiry, "%Y-%m-%d").unwrap();
            require((2000..2100).contains(&chrono::Datelike::year(&date)), "OCC year outside supported century")?;
            Ok(format!("{}{}{}{:0>8}", c.underlying, date.format("%y%m%d"), if c.kind == OptionKind::Call { "C" } else { "P" }, strike.normalize()))
        }
    }
}

pub fn request(intent: &OrderIntent) -> Result<Value> {
    intent.validate()?;
    require(intent.client_order_id.len() <= 48, "Alpaca client ID exceeds 48 characters")?;
    let side = |s| if s == Side::Buy { "buy" } else { "sell" };
    let position_intent = |s| match (s, intent.reduce_only) {
        (Side::Buy, false) => "buy_to_open", (Side::Sell, false) => "sell_to_open",
        (Side::Buy, true) => "buy_to_close", (Side::Sell, true) => "sell_to_close",
    };
    if intent.legs.len() == 1 {
        let leg = &intent.legs[0];
        let mut body = json!({"client_order_id": intent.client_order_id, "symbol": symbol(&leg.instrument)?,
            "qty": leg.quantity.value().to_string(), "side": side(leg.side),
            "type": if leg.limit.is_some() { "limit" } else { "market" }, "time_in_force": "day"});
        if let Some(limit) = leg.limit { body["limit_price"] = json!(limit.value().to_string()); }
        if matches!(leg.instrument, Instrument::Option(_)) { body["position_intent"] = json!(position_intent(leg.side)); }
        return Ok(body);
    }
    require(intent.legs.iter().all(|l| matches!(l.instrument, Instrument::Option(_)) && l.limit.is_none()),
        "Alpaca multi-leg requires options with market execution; per-leg limits cannot become a net limit")?;
    let mut units = intent.legs[0].quantity.value();
    for leg in &intent.legs { let mut b = leg.quantity.value(); while b != 0 { let r = units % b; units = b; b = r; } }
    let legs = intent.legs.iter().map(|l| Ok(json!({"symbol": symbol(&l.instrument)?, "ratio_qty": l.quantity.value() / units,
        "side": side(l.side), "position_intent": position_intent(l.side)}))).collect::<Result<Vec<_>>>()?;
    Ok(json!({"client_order_id": intent.client_order_id, "order_class": "mleg", "qty": units.to_string(), "type": "market", "time_in_force": "day", "legs": legs}))
}

pub struct AlpacaVenue<T: AlpacaTransport> {
    journal: ExecutionJournal,
    transport: T,
}
pub type AlpacaPaperVenue = AlpacaVenue<AlpacaHttp>;
pub type AlpacaLiveVenue = AlpacaVenue<AlpacaHttp>;
impl AlpacaVenue<AlpacaHttp> {
    pub fn paper(journal: ExecutionJournal, key: &str, secret: &str) -> Result<Self> {
        Ok(Self::new(journal, AlpacaHttp::new(AlpacaEnvironment::Paper, key, secret)?))
    }
    pub fn live(journal: ExecutionJournal, key: &str, secret: &str) -> Result<Self> {
        Ok(Self::new(journal, AlpacaHttp::new(AlpacaEnvironment::Live, key, secret)?))
    }
}
impl<T: AlpacaTransport> AlpacaVenue<T> {
    pub fn new(journal: ExecutionJournal, transport: T) -> Self { Self { journal, transport } }
    pub fn journal(&self) -> &ExecutionJournal { &self.journal }
    pub async fn close(self) { self.journal.close().await; }

    /// Ingest an actual trade_updates message, including per-leg executions.
    /// `fees` is an explicit per-execution fee map from the caller's fee source;
    /// missing fees fail instead of quietly treating an unknown fee as zero.
    pub async fn on_trade_update(&mut self, update: &Value, fees: &BTreeMap<String, Money>, received_at_ms: i64) -> Result<()> {
        let data = update.get("data").unwrap_or(update);
        let order = &data["order"];
        let id = string(order, "client_order_id")?;
        let intent = self.journal.state().orders.get(id).ok_or_else(|| DomainError("unknown client order ID".into()))?.intent.clone();
        let status = status(string(order, "status")?)?;
        let rows: Vec<&Value> = if intent.legs.len() == 1 { vec![order] } else {
            order["legs"].as_array().ok_or_else(|| DomainError("missing nested order legs".into()))?.iter().collect()
        };
        let mut cumulative = Vec::new();
        for leg in &intent.legs {
            let expected = symbol(&leg.instrument)?;
            let matching: Vec<_> = rows.iter().filter(|r| r["symbol"].as_str() == Some(&expected)).collect();
            require(matching.len() == 1, "broker order instrument mismatch")?;
            let row = matching[0];
            require(row["side"].as_str() == Some(if leg.side == Side::Buy { "buy" } else { "sell" }), "broker order side mismatch")?;
            require(integer(row, "qty")? == leg.quantity.value(), "broker order quantity mismatch")?;
            cumulative.push(integer(row, "filled_qty")?);
        }
        let fill_event = matches!(data["event"].as_str(), Some("fill" | "partial_fill"));
        let mut fills = Vec::new();
        if fill_event {
            let executions: Vec<&Value> = if intent.legs.len() == 1 { vec![data] } else {
                data["legs"].as_array().ok_or_else(|| DomainError("missing per-leg executions".into()))?.iter().collect()
            };
            for execution in executions {
                let execution_id = string(execution, "execution_id")?.to_string();
                let leg_index = if intent.legs.len() == 1 { 0 } else {
                    let execution_symbol = string(execution, "symbol")?;
                    intent.legs.iter().position(|l| symbol(&l.instrument).ok().as_deref() == Some(execution_symbol))
                        .ok_or_else(|| DomainError("unknown execution instrument".into()))?
                };
                fills.push(Fill { execution_id: execution_id.clone(), order_id: id.into(), leg_index,
                    quantity: Quantity::try_from(integer(execution, "qty")?)?, price: Price::try_from(decimal(execution, "price")?)?,
                    fee: *fees.get(&execution_id).ok_or_else(|| DomainError("missing explicit execution fee".into()))? });
            }
        }
        let report_id = data["event_id"].as_str().map(str::to_string).unwrap_or_else(|| sha256(&serde_json::to_vec(data).unwrap()));
        self.journal.ingest_report(BrokerReport { report_id, order_id: id.into(), status, cumulative_quantities: cumulative,
            fills, reason: Some(data["event"].as_str().unwrap_or("broker update").into()) }, received_at_ms).await.map_err(storage)
    }

    pub async fn reconcile_ack(&mut self, id: &str, now: i64) -> Result<()> {
        let response = self.transport.lookup(id).await?;
        self.ack(id, response, now).await
    }
    async fn ack(&mut self, id: &str, response: Value, now: i64) -> Result<()> {
        require(string(&response, "client_order_id")? == id, "acknowledgement client ID mismatch")?;
        // Filled snapshots alone cannot book executions; stream ingestion must
        // provide the incremental prices and execution IDs before completion.
        let broker_status = status(string(&response, "status")?)?;
        let local = &self.journal.state().orders[id];
        if matches!(local.status, OrderStatus::Created | OrderStatus::Submitted) {
            let payload = if broker_status == OrderStatus::Rejected {
                EventPayload::OrderRejected { order_id: id.into(), reason: "broker rejected order".into() }
            } else { EventPayload::OrderAccepted { order_id: id.into() } };
            if local.status == OrderStatus::Created {
                self.journal.record(EventPayload::OrderSubmitted { order_id: id.into() }, now).await.map_err(storage)?;
            }
            self.journal.record(payload, now).await.map_err(storage)?;
        }
        Ok(())
    }
}
fn storage(error: impl std::fmt::Display) -> DomainError { DomainError(error.to_string()) }
fn string<'a>(v: &'a Value, field: &str) -> Result<&'a str> { v[field].as_str().ok_or_else(|| DomainError(format!("missing {field}"))) }
fn decimal(v: &Value, field: &str) -> Result<Decimal> { string(v, field)?.parse().map_err(|_| DomainError(format!("invalid {field}"))) }
fn integer(v: &Value, field: &str) -> Result<u32> {
    let d = decimal(v, field)?;
    require(d >= Decimal::ZERO && d.fract() == Decimal::ZERO, "fractional/negative contract quantity")?;
    d.to_string().parse().or_else(|_| d.normalize().to_string().parse()).map_err(|_| DomainError("quantity overflow".into()))
}
fn status(s: &str) -> Result<OrderStatus> {
    match s {
        "accepted" | "new" | "pending_new" => Ok(OrderStatus::Accepted),
        "partially_filled" => Ok(OrderStatus::PartiallyFilled), "filled" => Ok(OrderStatus::Filled),
        "canceled" | "expired" => Ok(OrderStatus::Cancelled), "rejected" => Ok(OrderStatus::Rejected),
        _ => Err(DomainError(format!("unsupported broker status {s}; reconcile"))),
    }
}
impl<T: AlpacaTransport> ExecutionVenue for AlpacaVenue<T> {
    fn snapshot(&self) -> &ReplayState { self.journal.state() }
    async fn record(&mut self, payload: EventPayload, now: i64) -> Result<()> { self.journal.record(payload, now).await.map_err(storage) }
    async fn submit(&mut self, intent: OrderIntent, now: i64) -> Result<Vec<TradingEvent>> {
        let body = request(&intent)?;
        let id = intent.client_order_id.clone();
        if let Some(existing) = self.journal.state().orders.get(&id) {
            require(existing.intent == intent, "client ID reused with different intent")?;
            // A restart might find a persisted intent whose send outcome is
            // unknown. Lookup only; never turn a missing response into a POST.
            if !existing.status.is_terminal() { self.reconcile_ack(&id, now).await?; }
            return Ok(Vec::new());
        }
        let start = self.journal.state().last_sequence as usize;
        self.journal.prepare_order(intent, now).await.map_err(storage)?;
        self.record(EventPayload::OrderSubmitted { order_id: id.clone() }, now).await?;
        match self.transport.submit(body).await {
            Ok(response) => self.ack(&id, response, now).await?,
            Err(error) => {
                self.record(EventPayload::SubmissionOutcomeUnknown { order_id: id, reason: error.0.clone() }, now).await?;
                return Err(error);
            }
        }
        Ok(self.journal.events().await.map_err(storage)?[start..].to_vec())
    }
    async fn cancel(&mut self, id: &str, _now: i64) -> Result<Vec<TradingEvent>> {
        let order = self.journal.state().orders.get(id).ok_or_else(|| DomainError("unknown order".into()))?;
        if order.status.is_terminal() { return Ok(Vec::new()); }
        let response = self.transport.lookup(id).await?;
        require(string(&response, "client_order_id")? == id, "cancel lookup identity mismatch")?;
        self.transport.cancel(string(&response, "id")?).await?;
        // HTTP 204 acknowledges the request, not the final cancellation.
        Ok(Vec::new())
    }
    async fn poll(&self, id: &str) -> Result<Order> {
        self.journal.state().orders.get(id).cloned().ok_or_else(|| DomainError("unknown order".into()))
    }
}
