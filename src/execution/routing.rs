//! The same strategy conversion and pre-submit risk checks for every venue.
use super::{simulator::ExecutionVenue, *};
use crate::{risk::guards::{check_all, DailyRiskLimits}, strategies::{SignalAction, TradeSignal}};

pub struct RiskSnapshot {
    pub start_equity: f64,
    pub current_equity: f64,
    pub trades_today: usize,
    pub daily_limits: DailyRiskLimits,
    /// Unreserved buying power supplied by the account/risk model. Pending
    /// canonical orders are reserved below; do not subtract them twice here.
    pub buying_power: Money,
    pub max_quote_age_ms: i64,
}

fn fresh_quote<'a>(leg: &OrderLeg, quotes: &'a [Quote], now: i64, max_age: i64) -> Result<&'a Quote> {
    let matches: Vec<_> = quotes.iter().filter(|q| q.instrument == leg.instrument).collect();
    require(matches.len() == 1, "exactly one quote per instrument required")?;
    let q = matches[0];
    require(max_age >= 0 && q.exchange_timestamp_ms >= 0 && q.exchange_timestamp_ms <= q.received_at_ms
        && q.received_at_ms <= now && now.saturating_sub(q.exchange_timestamp_ms) <= max_age,
        "quote stale at submission")?;
    require(q.bid <= q.ask && q.ask.value() > Decimal::ZERO, "invalid submission quote")?;
    if let Instrument::Option(c) = &leg.instrument {
        let day = chrono::DateTime::from_timestamp_millis(now).ok_or_else(|| DomainError("invalid clock".into()))?.date_naive();
        require(c.expiry >= day.format("%Y-%m-%d").to_string(), "expired option")?;
    }
    Ok(q)
}

fn reservation(leg: &OrderLeg, qty: u32, quote: &Quote) -> Result<Decimal> {
    // Deliberately conservative independent-leg stress collateral. No spread
    // netting; short calls still have unbounded risk and this is not Reg-T.
    let per_unit = match (&leg.instrument, leg.side) {
        (Instrument::Option(c), Side::Sell) => c.strike.value().max(quote.ask.value()),
        (Instrument::Equity { .. }, Side::Sell) => arithmetic(quote.ask.value().checked_mul(Decimal::new(15, 1)))?,
        _ => leg.limit.map(|p| p.value()).unwrap_or(quote.ask.value()),
    };
    arithmetic(arithmetic(per_unit.checked_mul(Decimal::from(qty)))?.checked_mul(Decimal::from(leg.instrument.multiplier())))
}

pub fn check_intent(state: &ReplayState, intent: &OrderIntent, quotes: &[Quote], risk: &RiskSnapshot, now: i64) -> Result<()> {
    intent.validate()?;
    require(now >= state.last_timestamp_ms && risk.buying_power.0 >= Decimal::ZERO, "invalid risk clock/buying power")?;
    require(risk.start_equity.is_finite() && risk.start_equity > 0.0 && risk.current_equity.is_finite(), "invalid risk equity")?;
    if let Some(limit) = risk.daily_limits.max_daily_drawdown_pct {
        require(limit.is_finite() && (0.0..=1.0).contains(&limit), "invalid drawdown limit")?;
    }
    let mut required = Decimal::ZERO;
    for leg in &intent.legs {
        let quote = fresh_quote(leg, quotes, now, risk.max_quote_age_ms)?;
        require(!state.expired_instruments.contains(&leg.instrument.key()), "settled contract")?;
        if intent.reduce_only {
            let capacity = state.portfolio.positions.get(&leg.instrument.key()).filter(|p| p.quantity.signum() == -leg.side.sign())
                .map(|p| p.quantity.unsigned_abs()).unwrap_or(0);
            let pending: u64 = state.orders.values().filter(|o| !o.status.is_terminal()).flat_map(|o| o.intent.legs.iter().zip(&o.filled_quantities))
                .filter(|(l, _)| l.instrument == leg.instrument && l.side == leg.side)
                .map(|(l, filled)| u64::from(l.quantity.value() - filled)).sum();
            require(capacity.saturating_sub(pending) >= u64::from(leg.quantity.value()), "reduce-only capacity already consumed/reserved")?;
        } else { required = arithmetic(required.checked_add(reservation(leg, leg.quantity.value(), quote)?))?; }
    }
    if intent.reduce_only { return Ok(()); }
    require(!state.circuit_broken, "circuit breaker halted new risk")?;
    let guard = check_all(risk.start_equity, risk.current_equity, risk.trades_today, &risk.daily_limits);
    if let crate::risk::guards::GuardAction::Halt { reason } = guard { return Err(DomainError(reason)); }
    for order in state.orders.values().filter(|o| !o.status.is_terminal() && !o.intent.reduce_only) {
        for (leg, filled) in order.intent.legs.iter().zip(&order.filled_quantities) {
            let remaining = leg.quantity.value() - filled;
            if remaining > 0 {
                required = arithmetic(required.checked_add(reservation(leg, remaining,
                    fresh_quote(leg, quotes, now, risk.max_quote_age_ms)?)?))?;
            }
        }
    }
    require(required <= risk.buying_power.0, "insufficient buying power including pending orders")
}

pub async fn route_intent<V: ExecutionVenue>(venue: &mut V, intent: OrderIntent, quotes: &[Quote], risk: &RiskSnapshot, now: i64) -> Result<Vec<TradingEvent>> {
    if let Some(existing) = venue.snapshot().orders.get(&intent.client_order_id) {
        require(existing.intent == intent, "conflicting client order ID")?;
        return venue.submit(intent, now).await;
    }
    intent.validate()?;
    let signal_id = format!("signal:{}", intent.client_order_id);
    let decision_id = format!("risk:{}", intent.client_order_id);
    require(!venue.snapshot().signals.contains_key(&signal_id), "rejected signal requires a new client order ID")?;
    let decision = check_intent(venue.snapshot(), &intent, quotes, risk, now);
    venue.record(EventPayload::SignalGenerated { signal_id: signal_id.clone(), strategy_id: intent.strategy_id.clone(),
        instrument: intent.legs[0].instrument.clone(), reason: "strategy intent".into() }, now).await?;
    venue.record(EventPayload::RiskDecisionRecorded { decision_id, signal_id: signal_id.clone(), approved: decision.is_ok(),
        reasons: vec![decision.as_ref().err().map(|e| e.0.clone()).unwrap_or_else(|| "shared pre-submit checks passed".into())] }, now).await?;
    if let Err(error) = decision {
        venue.record(EventPayload::SignalRejected { signal_id, reason: error.0.clone() }, now).await?;
        return Err(error);
    }
    venue.submit(intent, now).await
}

/// Convert existing strategies' signals without changing their implementation.
/// Expiry is derived from the supplied simulation/session date, never wall time.
pub fn signal_intent(signal: &TradeSignal, client_order_id: String, quantity: Quantity, date: chrono::NaiveDate) -> Result<OrderIntent> {
    use SignalAction::*;
    let mut legs = Vec::new();
    let mut add = |kind, side, strike: f64, days: usize| -> Result<()> {
        require(strike.is_finite() && strike > 0.0, "invalid strategy strike")?;
        let expiry = date.checked_add_days(chrono::Days::new(u64::try_from(days).map_err(|_| DomainError("expiry overflow".into()))?))
            .ok_or_else(|| DomainError("expiry overflow".into()))?.format("%Y-%m-%d").to_string();
        let value: Decimal = strike.to_string().parse().map_err(|_| DomainError("invalid decimal strike".into()))?;
        legs.push(OrderLeg { instrument: Instrument::Option(OptionContract { underlying: signal.symbol.clone(), expiry, kind, strike: Price::try_from(value)? }), side, quantity, limit: None });
        Ok(())
    };
    match signal.action {
        BuyCall { strike, days_to_expiry, .. } => add(OptionKind::Call, Side::Buy, strike, days_to_expiry)?,
        BuyPut { strike, days_to_expiry, .. } => add(OptionKind::Put, Side::Buy, strike, days_to_expiry)?,
        SellCall { strike, days_to_expiry, .. } | CoveredCall { sell_strike: strike, days_to_expiry } => add(OptionKind::Call, Side::Sell, strike, days_to_expiry)?,
        SellPut { strike, days_to_expiry, .. } | CashSecuredPut { strike, days_to_expiry } => add(OptionKind::Put, Side::Sell, strike, days_to_expiry)?,
        BuyStraddle { strike, days_to_expiry } | SellStraddle { strike, days_to_expiry } => {
            let side = if matches!(signal.action, BuyStraddle { .. }) { Side::Buy } else { Side::Sell };
            add(OptionKind::Call, side, strike, days_to_expiry)?; add(OptionKind::Put, side, strike, days_to_expiry)?;
        }
        IronCondor { sell_call_strike, buy_call_strike, sell_put_strike, buy_put_strike, days_to_expiry } => {
            add(OptionKind::Call, Side::Sell, sell_call_strike, days_to_expiry)?; add(OptionKind::Call, Side::Buy, buy_call_strike, days_to_expiry)?;
            add(OptionKind::Put, Side::Sell, sell_put_strike, days_to_expiry)?; add(OptionKind::Put, Side::Buy, buy_put_strike, days_to_expiry)?;
        }
        CreditCallSpread { sell_strike, buy_strike, days_to_expiry } => {
            add(OptionKind::Call, Side::Sell, sell_strike, days_to_expiry)?; add(OptionKind::Call, Side::Buy, buy_strike, days_to_expiry)?;
        }
        CreditPutSpread { sell_strike, buy_strike, days_to_expiry } => {
            add(OptionKind::Put, Side::Sell, sell_strike, days_to_expiry)?; add(OptionKind::Put, Side::Buy, buy_strike, days_to_expiry)?;
        }
        IronButterfly { center_strike, wing_width, days_to_expiry } => {
            add(OptionKind::Call, Side::Sell, center_strike, days_to_expiry)?; add(OptionKind::Put, Side::Sell, center_strike, days_to_expiry)?;
            add(OptionKind::Call, Side::Buy, center_strike + wing_width, days_to_expiry)?; add(OptionKind::Put, Side::Buy, center_strike - wing_width, days_to_expiry)?;
        }
        ClosePosition { .. } | NoAction => return Err(DomainError("close/no-action requires explicit position intent or no submission".into())),
    }
    let intent = OrderIntent { client_order_id, strategy_id: signal.strategy_name.clone(), reduce_only: false, legs };
    intent.validate()?;
    Ok(intent)
}
