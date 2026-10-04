//! Failure-class tests for the provider-independent execution foundation.
use dollarbill::{build_info::BuildInfo, domain::*, execution::{*, simulator::*, store::EventStore}};
use proptest::prelude::*;
use rust_decimal::Decimal;
use dollarbill::execution::journal::ExecutionJournal;
use dollarbill::execution::{alpaca::{AlpacaTransport, AlpacaVenue}, routing::*};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockBrokerState { posts: Vec<Value>, cancels: usize, timeout: bool }
struct MockBroker(Arc<Mutex<MockBrokerState>>);
impl AlpacaTransport for MockBroker {
    async fn submit(&mut self, body: Value) -> dollarbill::domain::Result<Value> {
        let mut state = self.0.lock().unwrap();
        state.posts.push(body.clone());
        if state.timeout { return Err(DomainError("timeout after acceptance".into())); }
        Ok(json!({"id":"broker-1", "client_order_id":body["client_order_id"], "status":"accepted"}))
    }
    async fn lookup(&self, id: &str) -> dollarbill::domain::Result<Value> {
        Ok(json!({"id":"broker-1", "client_order_id":id, "status":"accepted"}))
    }
    async fn cancel(&mut self, _: &str) -> dollarbill::domain::Result<()> {
        self.0.lock().unwrap().cancels += 1;
        Ok(())
    }
}
fn risk() -> RiskSnapshot {
    RiskSnapshot { start_equity: 1000.0, current_equity: 1000.0, trades_today: 0,
        daily_limits: Default::default(), buying_power: money("1000"), max_quote_age_ms: 500 }
}

#[tokio::test]
async fn repeated_snapshot_cannot_replenish_consumed_liquidity() {
    let mut venue = venue();
    venue.submit(intent("buy", Side::Buy, 4), 1).await.unwrap();
    let original = quote(2, "10", 2);
    venue.on_quote(original.clone()).unwrap();
    let mut repeated = original;
    repeated.received_at_ms = 3;
    venue.on_quote(repeated).unwrap();
    assert_eq!(venue.state().orders["buy"].filled_quantities, vec![2]);
    assert_eq!(venue.state().portfolio.cash, money("980"));
    assert!(matches!(venue.events().last().unwrap().payload, EventPayload::QuoteRejected { .. }));
    assert_eq!(replay(venue.events()).unwrap(), *venue.state());
}

#[tokio::test]
async fn accepted_timeout_is_recovered_without_a_second_submission() {
    let mut cfg = config();
    cfg.timeout_after_acceptance.insert("buy".into());
    let mut sim = SimulatedVenue::new("test-run".into(), manifest(&cfg), cfg).unwrap();
    let order = intent("buy", Side::Buy, 2);
    assert!(sim.submit(order.clone(), 1).await.is_err());
    assert_eq!(sim.poll("buy").await.unwrap().status, OrderStatus::Accepted);
    assert!(sim.submit(order.clone(), 2).await.unwrap().is_empty());
    sim.on_quote(quote(3, "10", 2)).unwrap();
    assert_eq!(sim.state().portfolio.cash, money("980"));
    assert_eq!(replay(sim.events()).unwrap(), *sim.state());

    let store = EventStore::open(":memory:").await.unwrap();
    let journal = ExecutionJournal::start(store, "test-run", manifest(&config()), "alpaca-test").await.unwrap();
    let broker = Arc::new(Mutex::new(MockBrokerState { timeout: true, ..Default::default() }));
    let mut venue = AlpacaVenue::new(journal, MockBroker(broker.clone()));
    assert!(venue.submit(order.clone(), 1).await.is_err());
    assert_eq!(venue.snapshot().orders["buy"].status, OrderStatus::Submitted);
    venue.submit(order, 2).await.unwrap();
    assert_eq!(broker.lock().unwrap().posts.len(), 1);
    assert_eq!(venue.snapshot().orders["buy"].status, OrderStatus::Accepted);
    assert_eq!(venue.snapshot().portfolio.cash, money("1000"));
}

#[tokio::test]
async fn alpaca_fill_during_cancel_uses_execution_price_once() {
    let store = EventStore::open(":memory:").await.unwrap();
    let journal = ExecutionJournal::start(store, "test-run", manifest(&config()), "alpaca-test").await.unwrap();
    let broker = Arc::new(Mutex::new(MockBrokerState::default()));
    let mut venue = AlpacaVenue::new(journal, MockBroker(broker.clone()));
    venue.submit(intent("buy", Side::Buy, 2), 1).await.unwrap();
    venue.cancel("buy", 2).await.unwrap();
    assert_eq!(venue.poll("buy").await.unwrap().status, OrderStatus::Accepted);
    let message = json!({"data":{"event":"fill", "execution_id":"fill1", "qty":"2", "price":"10.25",
        "order":{"client_order_id":"buy", "symbol":"TEST", "side":"buy", "qty":"2", "filled_qty":"2", "status":"filled", "filled_avg_price":"999"}}});
    let fees = [("fill1".into(), money("0.20"))].into_iter().collect();
    assert!(venue.on_trade_update(&message, &Default::default(), 3).await.is_err());
    venue.on_trade_update(&message, &fees, 3).await.unwrap();
    venue.on_trade_update(&message, &fees, 4).await.unwrap();
    assert_eq!(venue.snapshot().portfolio.cash, money("979.30"));
    assert_eq!(venue.poll("buy").await.unwrap().status, OrderStatus::Filled);
    assert_eq!(venue.snapshot().fills.len(), 1);
    assert_eq!(replay(&venue.journal().events().await.unwrap()).unwrap(), *venue.snapshot());
}

#[tokio::test]
async fn unchanged_momentum_strategy_and_risk_route_to_simulator_and_alpaca() {
    use dollarbill::strategies::{TradingStrategy, momentum::MomentumStrategy};
    let strategy = MomentumStrategy::new();
    let signals = strategy.generate_signals("TEST", 10.0, 0.5, 0.3, 0.2);
    assert!(!signals.is_empty());
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    let order = signal_intent(&signals[0], "momentum-1".into(), Quantity::try_from(1).unwrap(), date).unwrap();
    let now = date.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp_millis();
    let quotes: Vec<_> = order.legs.iter().map(|l| Quote { instrument: l.instrument.clone(), ..quote(now, "1", 1) }).collect();
    let mut sim = venue();
    let store = EventStore::open(":memory:").await.unwrap();
    let journal = ExecutionJournal::start(store, "test-run", manifest(&config()), "alpaca-test").await.unwrap();
    let broker = Arc::new(Mutex::new(MockBrokerState::default()));
    let mut alpaca = AlpacaVenue::new(journal, MockBroker(broker.clone()));
    let mut limits = risk();
    limits.buying_power = money("100000");
    route_intent(&mut sim, order.clone(), &quotes, &limits, now).await.unwrap();
    route_intent(&mut alpaca, order.clone(), &quotes, &limits, now).await.unwrap();
    assert_eq!(sim.snapshot().orders, alpaca.snapshot().orders);
    assert_eq!(sim.snapshot().risk_decisions, alpaca.snapshot().risk_decisions);
    assert_eq!(broker.lock().unwrap().posts[0]["order_class"], "mleg");
    for quote in &quotes { sim.on_quote(quote.clone()).unwrap(); }
    let rows: Vec<_> = order.legs.iter().map(|l| json!({"symbol": dollarbill::execution::alpaca::symbol(&l.instrument).unwrap(),
        "side": if l.side == Side::Buy { "buy" } else { "sell" }, "qty":"1", "filled_qty":"1"})).collect();
    let executions: Vec<_> = order.legs.iter().enumerate().map(|(i,l)| json!({"execution_id":format!("leg-{i}"),
        "symbol": dollarbill::execution::alpaca::symbol(&l.instrument).unwrap(), "qty":"1", "price":"1"})).collect();
    let fees = order.legs.iter().enumerate().map(|(i,_)| (format!("leg-{i}"), money("0"))).collect();
    let update = json!({"data":{"event_id":"mleg-fill", "event":"fill", "order":{"client_order_id":"momentum-1", "status":"filled", "legs": rows}, "legs":executions}});
    alpaca.on_trade_update(&update, &fees, now).await.unwrap();
    alpaca.on_trade_update(&update, &fees, now).await.unwrap();
    assert_eq!(sim.snapshot().portfolio, alpaca.snapshot().portfolio);
    assert_eq!(sim.snapshot().orders, alpaca.snapshot().orders);
    // Risk was checked at quote time, but the same quote is now stale before submit.
    let mut stale_order = order;
    stale_order.client_order_id = "stale".into();
    assert!(route_intent(&mut sim, stale_order.clone(), &quotes, &limits, now + 501).await.is_err());
    assert!(route_intent(&mut alpaca, stale_order, &quotes, &limits, now + 501).await.is_err());
    assert_eq!(broker.lock().unwrap().posts.len(), 1);
}

#[tokio::test]
async fn buying_power_reserves_pending_orders_and_releases_cancellations() {
    let mut venue = venue();
    let quotes = [quote(1, "10", 100)];
    let mut limits = risk();
    limits.buying_power = money("100");
    route_intent(&mut venue, intent("first", Side::Buy, 8), &quotes, &limits, 1).await.unwrap();
    assert!(route_intent(&mut venue, intent("blocked", Side::Buy, 3), &quotes, &limits, 2).await.is_err());
    venue.cancel("first", 3).await.unwrap();
    route_intent(&mut venue, intent("released", Side::Buy, 3), &quotes, &limits, 4).await.unwrap();
    assert_eq!(venue.snapshot().orders.len(), 2);
    assert!(venue.snapshot().signals["signal:blocked"].rejected);
}

#[tokio::test]
async fn expiry_settles_itm_and_otm_and_cancels_pending_contract_orders() {
    let start = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp_millis();
    for (kind, side, spot, stock_qty, cash) in [
        (OptionKind::Call, Side::Buy, "12", 100, "-200"),
        (OptionKind::Call, Side::Sell, "12", -100, "2200"),
        (OptionKind::Put, Side::Buy, "8", -100, "1800"),
        (OptionKind::Put, Side::Sell, "8", 100, "200"),
        (OptionKind::Call, Side::Buy, "9", 0, "800"),
        (OptionKind::Put, Side::Sell, "11", 0, "1200"),
    ] {
        let mut venue = venue();
        let option = Instrument::Option(OptionContract { underlying: "TEST".into(), expiry: "2026-10-02".into(), kind, strike: price("10") });
        let mut open = intent("option", side, 1); open.legs[0].instrument = option.clone();
        venue.submit(open.clone(), start).await.unwrap();
        venue.on_quote(Quote { instrument: option.clone(), ..quote(start + 1, "2", 1) }).unwrap();
        open.client_order_id = "pending".into();
        venue.submit(open, start + 2).await.unwrap();
        let before = venue.state().clone();
        assert!(venue.expire(option.clone(), price(spot), price("0.01"), money("0"), start + 3).is_err());
        assert_eq!(*venue.state(), before);
        venue.expire(option.clone(), price(spot), price("0.01"), money("0"), start + 86_400_000).unwrap();
        assert_eq!(venue.state().portfolio.cash, money(cash));
        assert_eq!(venue.state().portfolio.positions.get("equity:TEST").map(|p| p.quantity).unwrap_or(0), stock_qty);
        assert_eq!(venue.state().orders["pending"].status, OrderStatus::Cancelled);
        assert!(!venue.state().portfolio.positions.contains_key(&option.key()));
        assert_eq!(replay(venue.events()).unwrap(), *venue.state());
        assert!(venue.expire(option, price(spot), price("0.01"), money("0"), start + 86_400_001).is_err());
    }
}

fn report(id: &str, qty: u32, status: OrderStatus, fills: Vec<Fill>) -> BrokerReport {
    BrokerReport { report_id: id.into(), order_id: "buy".into(), status,
        cumulative_quantities: vec![qty], fills, reason: None }
}

#[tokio::test]
async fn sell_commissions_cannot_overdraw_simulated_cash() {
    let mut cfg = config();
    cfg.fee_per_unit = money("1002");
    let mut venue = SimulatedVenue::new("test-run".into(), manifest(&cfg), cfg).unwrap();
    venue.submit(intent("sell", Side::Sell, 1), 1).await.unwrap();
    venue.on_quote(quote(2, "1", 1)).unwrap();
    assert_eq!(venue.state().portfolio.cash, money("1000"));
    assert!(venue.state().portfolio.positions.is_empty());
    assert_eq!(venue.state().orders["sell"].status, OrderStatus::Cancelled);
}

#[tokio::test]
async fn journal_recovers_partial_reports_and_deduplicates_executions() {
    let db = std::env::temp_dir().join(format!("journal-{}-{}.sqlite", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let store = EventStore::open(db.to_str().unwrap()).await.unwrap();
    let mut journal = ExecutionJournal::start(store, "test-run", manifest(&config()), "broker-test").await.unwrap();
    journal.prepare_order(intent("buy", Side::Buy, 5), 1).await.unwrap();
    journal.close().await;
    let store = EventStore::open(db.to_str().unwrap()).await.unwrap();
    let mut journal = ExecutionJournal::resume(store, "test-run", "broker-test").await.unwrap();
    assert_eq!(journal.state().orders["buy"].status, OrderStatus::Created);
    let fill = Fill { execution_id: "execution-1".into(), order_id: "buy".into(), leg_index: 0,
        quantity: Quantity::try_from(2).unwrap(), price: price("10"), fee: money("0.20") };
    let partial = report("partial", 2, OrderStatus::PartiallyFilled, vec![fill.clone()]);
    journal.ingest_report(partial.clone(), 2).await.unwrap();
    journal.close().await;
    let store = EventStore::open(db.to_str().unwrap()).await.unwrap();
    let mut journal = ExecutionJournal::resume(store, "test-run", "broker-test").await.unwrap();
    let before = journal.state().clone();
    journal.ingest_report(partial, 3).await.unwrap();
    assert_eq!(*journal.state(), before);
    let invalid = report("missing-executions", 5, OrderStatus::Filled, vec![]);
    assert!(journal.ingest_report(invalid, 3).await.is_err());
    assert_eq!(*journal.state(), before);
    let mut altered = fill.clone();
    altered.price = price("11");
    assert!(journal.ingest_report(report("conflict", 2, OrderStatus::PartiallyFilled, vec![altered]), 3).await.is_err());
    journal.ingest_report(report("cancel", 2, OrderStatus::Cancelled, vec![fill]), 4).await.unwrap();
    assert_eq!(journal.state().portfolio.cash, money("979.80"));
    assert_eq!(journal.state().fills.len(), 1);
    assert_eq!(replay(&journal.events().await.unwrap()).unwrap(), *journal.state());
    journal.close().await;
    std::fs::remove_file(db).unwrap();
}

#[tokio::test]
async fn store_batch_rolls_back_earlier_valid_events_when_later_event_fails() {
    let venue = venue();
    let store = EventStore::open(":memory:").await.unwrap();
    store.append(&venue.events()[0]).await.unwrap();
    let created = event(venue.state(), EventPayload::OrderCreated { intent: intent("buy", Side::Buy, 1) });
    let mut next = venue.state().clone();
    next.apply(&created).unwrap();
    let invalid = event(&next, EventPayload::OrderAccepted { order_id: "buy".into() });
    assert!(store.append_batch(&[created, invalid]).await.is_err());
    assert_eq!(store.events("test-run").await.unwrap(), venue.events());
}

#[tokio::test]
async fn signal_risk_provenance_and_invariant_failure_replay() {
    let store = EventStore::open(":memory:").await.unwrap();
    let mut journal = ExecutionJournal::start(store, "test-run", manifest(&config()), "strategy").await.unwrap();
    journal.record(EventPayload::SignalGenerated { signal_id: "signal".into(), strategy_id: "momentum".into(), instrument: instrument(), reason: "threshold".into() }, 1).await.unwrap();
    journal.record(EventPayload::RiskDecisionRecorded { decision_id: "risk".into(), signal_id: "signal".into(), approved: false, reasons: vec!["exposure".into()] }, 2).await.unwrap();
    journal.record(EventPayload::SignalRejected { signal_id: "signal".into(), reason: "risk denied".into() }, 3).await.unwrap();
    journal.record(EventPayload::InvariantViolation { reason: "position mismatch".into() }, 4).await.unwrap();
    let events = journal.events().await.unwrap();
    assert_eq!(events[2].context.strategy_id.as_deref(), Some("momentum"));
    assert!(journal.state().circuit_broken);
    let mut tampered = events.clone();
    tampered[2].context.config_sha256 = sha256(b"different");
    assert!(replay(&tampered).is_err());
    assert_eq!(replay(&events).unwrap(), *journal.state());
}

#[tokio::test]
async fn assignment_and_exercise_deliver_stock_without_changing_pending_orders() {
    for (side, kind, assigned, expected_cash, expected_pnl) in [
        (Side::Sell, OptionKind::Put, true, "199", "199"),
        (Side::Buy, OptionKind::Call, false, "-201", "-201"),
    ] {
        let option = Instrument::Option(OptionContract { underlying: "TEST".into(), expiry: "2030-02-15".into(), kind, strike: price("10") });
        let mut venue = venue();
        let mut order = intent("option", side, 1);
        order.legs[0].instrument = option.clone();
        venue.submit(order, 1).await.unwrap();
        let mut q = quote(2, "2", 1);
        q.instrument = option.clone();
        venue.on_quote(q).unwrap();
        venue.submit(intent("pending", Side::Buy, 1), 3).await.unwrap();
        let mut state = venue.state().clone();
        let pending = state.orders["pending"].clone();
        let payload = if assigned {
            EventPayload::OptionAssigned { settlement_id: "settlement".into(), instrument: option, quantity: Quantity::try_from(1).unwrap(), fee: money("1") }
        } else {
            EventPayload::OptionExercised { settlement_id: "settlement".into(), instrument: option, quantity: Quantity::try_from(1).unwrap(), fee: money("1") }
        };
        let settlement = event(&state, payload.clone());
        state.apply(&settlement).unwrap();
        assert_eq!(state.portfolio.cash, money(expected_cash));
        assert_eq!(state.portfolio.realized_pnl, money(expected_pnl));
        assert_eq!(state.portfolio.positions.len(), 1);
        assert_eq!(state.portfolio.positions["equity:TEST"].quantity, 100);
        assert_eq!(state.orders["pending"], pending);
        let before = state.clone();
        assert!(state.apply(&event(&before, payload)).is_err());
        assert_eq!(state, before);
        let mut history = venue.events().to_vec();
        history.push(settlement);
        assert_eq!(replay(&history).unwrap(), state);
    }
}

fn money(value: &str) -> Money { Money(value.parse().unwrap()) }
fn price(value: &str) -> Price { Price::try_from(value.parse::<Decimal>().unwrap()).unwrap() }
fn config() -> SimulationConfig {
    SimulationConfig { latency_ms: 0, max_quote_age_ms: 500,
        fee_per_unit: money("0"), slippage_per_unit: price("0"), timeout_after_acceptance: Default::default() }
}
fn manifest(cfg: &SimulationConfig) -> RunManifest {
    RunManifest { build: BuildInfo::current(), config_sha256: sha256(&serde_json::to_vec(cfg).unwrap()),
        dataset_sha256: sha256(b"test-fixture"), initial_cash: money("1000"), currency: "USD".into(), execution_model: "test".into() }
}
fn venue() -> SimulatedVenue { SimulatedVenue::new("test-run".into(), manifest(&config()), config()).unwrap() }
fn instrument() -> Instrument { Instrument::Equity { symbol: "TEST".into() } }
fn intent(id: &str, side: Side, qty: u32) -> OrderIntent {
    OrderIntent { client_order_id: id.into(), strategy_id: "regression".into(), reduce_only: false,
        legs: vec![OrderLeg { instrument: instrument(), side, quantity: Quantity::try_from(qty).unwrap(), limit: None }] }
}
fn quote(timestamp: i64, px: &str, available: u32) -> Quote {
    Quote { instrument: instrument(), bid: price(px), ask: price(px), exchange_timestamp_ms: timestamp,
        received_at_ms: timestamp, available_quantity: available }
}
fn event(state: &ReplayState, payload: EventPayload) -> TradingEvent {
    TradingEvent::next(state, "test-run", "test", state.last_timestamp_ms, payload).unwrap()
}

#[tokio::test]
async fn partial_cancel_replay_and_restart_preserve_only_executed_inventory() {
    let mut venue = venue();
    venue.submit(intent("buy", Side::Buy, 10), 1).await.unwrap();
    venue.on_quote(quote(2, "10", 3)).unwrap();
    assert_eq!(venue.poll("buy").await.unwrap().status, OrderStatus::PartiallyFilled);
    venue.cancel("buy", 3).await.unwrap();
    venue.on_quote(quote(4, "10", 100)).unwrap();
    assert_eq!(venue.state().portfolio.cash, money("970"));
    assert_eq!(venue.state().portfolio.positions["equity:TEST"].quantity, 3);
    assert_eq!(&replay(venue.events()).unwrap(), venue.state());

    let db = std::env::temp_dir().join(format!("dollarbill-events-{}-{}.sqlite", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let store = EventStore::open(db.to_str().unwrap()).await.unwrap();
    let split = venue.events().iter().position(|e| matches!(e.payload, EventPayload::FillReceived { .. })).unwrap() + 1;
    for e in &venue.events()[..split] { store.append(e).await.unwrap(); }
    store.close().await;
    let store = EventStore::open(db.to_str().unwrap()).await.unwrap();
    assert_eq!(replay(&store.events("test-run").await.unwrap()).unwrap().portfolio.cash, money("970"));
    for e in &venue.events()[split..] { store.append(e).await.unwrap(); }
    assert_eq!(&replay(&store.events("test-run").await.unwrap()).unwrap(), venue.state());
    store.close().await;
    std::fs::remove_file(db).unwrap();
}

#[tokio::test]
async fn append_retries_are_idempotent_and_conflicting_or_invalid_events_are_atomic() {
    let venue = venue();
    let store = EventStore::open(":memory:").await.unwrap();
    let start = &venue.events()[0];
    store.append(start).await.unwrap();
    store.append(start).await.unwrap();
    let mut conflict = start.clone();
    conflict.source = "different".into();
    assert!(store.append(&conflict).await.is_err());
    let bad = event(venue.state(), EventPayload::OrderAccepted { order_id: "missing".into() });
    assert!(store.append(&bad).await.is_err());
    assert_eq!(store.events("test-run").await.unwrap(), venue.events());
}

#[tokio::test]
async fn duplicate_ids_overfills_and_post_cancel_fills_do_not_mutate_portfolio() {
    let mut venue = venue();
    let order = intent("same", Side::Buy, 2);
    venue.submit(order.clone(), 1).await.unwrap();
    assert!(venue.submit(order, 1).await.unwrap().is_empty());
    assert!(venue.submit(intent("same", Side::Buy, 3), 1).await.is_err());
    venue.on_quote(quote(2, "1", 1)).unwrap();
    let fill = venue.events().iter().find_map(|e| if let EventPayload::FillReceived { fill } = &e.payload { Some(fill.clone()) } else { None }).unwrap();
    let mut state = venue.state().clone();
    let original = state.clone();
    let duplicate = event(&state, EventPayload::FillReceived { fill: fill.clone() });
    assert!(state.apply(&duplicate).is_err());
    let mut too_many = fill.clone();
    too_many.execution_id = "overfill".into();
    too_many.quantity = Quantity::try_from(2).unwrap();
    assert!(state.apply(&event(&original, EventPayload::FillReceived { fill: too_many })).is_err());
    assert_eq!(state, original);
    venue.cancel("same", 3).await.unwrap();
    let mut late = fill;
    late.execution_id = "late".into();
    let mut state = venue.state().clone();
    let original = state.clone();
    assert!(state.apply(&event(&original, EventPayload::FillReceived { fill: late })).is_err());
    assert_eq!(state, original);
}

#[tokio::test]
async fn cash_conservation_through_position_reversal_and_fees() {
    let mut cfg = config();
    cfg.fee_per_unit = money("0.01");
    let mut venue = SimulatedVenue::new("test-run".into(), manifest(&cfg), cfg).unwrap();
    for (index, (id, side, quantity, px)) in [
        ("open", Side::Buy, 10, "10"), ("reverse", Side::Sell, 15, "12"), ("cover", Side::Buy, 5, "8")
    ].into_iter().enumerate() {
        venue.submit(intent(id, side, quantity), index as i64 * 2 + 1).await.unwrap();
        venue.on_quote(quote(index as i64 * 2 + 2, px, quantity)).unwrap();
    }
    assert!(venue.state().portfolio.positions.is_empty());
    assert_eq!(venue.state().portfolio.cash, money("1039.70"));
    assert_eq!(venue.state().portfolio.realized_pnl, money("39.70"));
    assert_eq!(venue.state().portfolio.total_fees, money("0.30"));
    assert_eq!(&replay(venue.events()).unwrap(), venue.state());
}

#[tokio::test]
async fn stale_crossed_quotes_latency_limits_and_shared_liquidity() {
    let mut cfg = config(); cfg.latency_ms = 10;
    let mut venue = SimulatedVenue::new("test-run".into(), manifest(&cfg), cfg).unwrap();
    let mut order = intent("first", Side::Buy, 3); order.legs[0].limit = Some(price("10"));
    venue.submit(order, 1).await.unwrap();
    venue.submit(intent("second", Side::Buy, 3), 2).await.unwrap();
    venue.on_quote(quote(3, "10", 3)).unwrap(); // latency
    let mut stale = quote(1000, "10", 3); stale.exchange_timestamp_ms = 0;
    venue.on_quote(stale).unwrap();
    let mut crossed = quote(1001, "10", 3); crossed.bid = price("11");
    venue.on_quote(crossed).unwrap();
    assert!(venue.state().portfolio.positions.is_empty());
    venue.on_quote(quote(1002, "10", 2)).unwrap();
    assert_eq!(venue.poll("first").await.unwrap().filled_quantities, vec![2]);
    assert_eq!(venue.poll("second").await.unwrap().filled_quantities, vec![0]);
    venue.on_quote(quote(1003, "11", 1)).unwrap(); // first order limit prevents fill
    assert_eq!(venue.poll("second").await.unwrap().filled_quantities, vec![1]);
}

#[tokio::test]
async fn breaker_mid_partial_fill_cancels_entries_but_allows_reduction() {
    let mut venue = venue();
    venue.submit(intent("entry", Side::Buy, 10), 1).await.unwrap();
    venue.on_quote(quote(2, "10", 3)).unwrap();
    venue.trip_circuit_breaker(3, "test loss limit".into()).unwrap();
    assert_eq!(venue.poll("entry").await.unwrap().status, OrderStatus::Cancelled);
    venue.submit(intent("blocked", Side::Buy, 1), 4).await.unwrap();
    assert_eq!(venue.poll("blocked").await.unwrap().status, OrderStatus::Rejected);
    let mut close = intent("close", Side::Sell, 3); close.reduce_only = true;
    venue.submit(close, 5).await.unwrap();
    venue.on_quote(quote(6, "9", 10)).unwrap();
    assert!(venue.state().portfolio.positions.is_empty());
    assert_eq!(venue.state().portfolio.cash, money("997"));
}

#[tokio::test]
async fn unfunded_orders_cancel_without_fabricating_fills() {
    let mut venue = venue();
    venue.submit(intent("unfunded", Side::Buy, 1000), 1).await.unwrap();
    venue.on_quote(quote(2, "20", 1000)).unwrap();
    assert_eq!(venue.poll("unfunded").await.unwrap().status, OrderStatus::Cancelled);
    assert_eq!(venue.state().portfolio.cash, money("1000"));
    assert!(venue.state().execution_ids.is_empty());
}

#[test]
fn deserialize_rejects_invalid_units_and_replay_rejects_schema_or_sequence_drift() {
    assert!(serde_json::from_str::<Price>("\"-1\"").is_err());
    assert!(serde_json::from_str::<Quantity>("0").is_err());
    let venue = venue();
    let mut events = venue.events().to_vec();
    events[0].schema_version = 99;
    assert!(replay(&events).is_err());
    events[0].schema_version = 1;
    events[0].sequence = 2;
    assert!(replay(&events).is_err());
    assert!(replay(&[]).is_err());
}

#[test]
fn partial_multileg_fixture_replays_identically_through_cli_and_sqlite() {
    let root = std::env::temp_dir().join(format!("dollarbill-replay-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let binary = env!("CARGO_BIN_EXE_dollarbill-replay");
    for suffix in ["first", "second"] {
        let output = std::process::Command::new(binary).args(["simulate", "--scenario",
            "tests/fixtures/execution/partial_multileg_fill.json", "--output"])
            .arg(root.join(suffix)).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
    let first = root.join("first");
    let state: ReplayState = serde_json::from_slice(&std::fs::read(first.join("state.json")).unwrap()).unwrap();
    assert_eq!(state.portfolio.cash, money("100249.20"));
    assert!(state.portfolio.positions.is_empty());
    assert_eq!(state.orders["condor-1"].filled_quantities, vec![1, 1, 2, 0]);
    assert_eq!(state.orders["condor-1"].status, OrderStatus::Cancelled);
    assert_eq!(state.orders["flatten-1"].status, OrderStatus::Filled);
    for artifact in ["events.jsonl", "state.json", "run.json", "fills.json", "positions.json", "metrics.json"] {
        assert_eq!(std::fs::read(first.join(artifact)).unwrap(), std::fs::read(root.join("second").join(artifact)).unwrap());
    }
    let output = std::process::Command::new(binary).args(["replay", "--events"])
        .arg(first.join("events.jsonl")).output().unwrap();
    assert!(output.status.success());
    assert_eq!(serde_json::from_slice::<ReplayState>(&output.stdout).unwrap(), state);
    // A damaged final event is an error, not a partially recovered success.
    let damaged = root.join("truncated.jsonl");
    std::fs::write(&damaged, "{\"schema_version\":1").unwrap();
    assert!(!std::process::Command::new(binary).args(["replay", "--events"]).arg(damaged).output().unwrap().status.success());
    // Only this test's uniquely-created temporary directory is removed.
    let resolved = root.canonicalize().unwrap();
    assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(resolved).unwrap();
}

proptest! {
    #[test]
    fn replay_cash_equals_signed_fill_ledger(q in 1u32..1000, cents in 1i64..10000, is_buy in any::<bool>()) {
        let mut state = venue().state().clone();
        let side = if is_buy { Side::Buy } else { Side::Sell };
        let order = intent("property", side, q);
        for payload in [EventPayload::OrderCreated { intent: order },
            EventPayload::OrderSubmitted { order_id: "property".into() },
            EventPayload::OrderAccepted { order_id: "property".into() }] {
            state.apply(&event(&state, payload)).unwrap();
        }
        let unit = Decimal::new(cents, 2);
        let fill = Fill { execution_id: "fill".into(), order_id: "property".into(), leg_index: 0,
            quantity: Quantity::try_from(q).unwrap(), price: Price::try_from(unit).unwrap(), fee: money("0.01") };
        state.apply(&event(&state, EventPayload::FillReceived { fill })).unwrap();
        prop_assert_eq!(state.portfolio.cash.0, Decimal::from(1000) - unit * Decimal::from(q) * Decimal::from(side.sign()) - Decimal::new(1, 2));
        prop_assert_eq!(state.portfolio.positions["equity:TEST"].quantity, q as i64 * side.sign());
        prop_assert_eq!(state.orders["property"].status, OrderStatus::Filled);
        let original = state.clone();
        let illegal = event(&state, EventPayload::OrderCancelled { order_id: "property".into(), reason: "late cancel".into() });
        prop_assert!(state.apply(&illegal).is_err());
        prop_assert_eq!(state, original);
    }
}
