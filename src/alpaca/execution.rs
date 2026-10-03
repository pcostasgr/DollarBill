//! Translate broker execution reports into signed, per-contract positions.
use super::{occ::parse_occ, types::Order, AlpacaClient};
use crate::persistence::{PositionLegRecord, PositionRecord};
use crate::risk::{ManagedPosition, ManagementAction};
use crate::risk::invariants::InvariantPosition;

fn nonnegative_number(value: &str) -> Result<f64, String> {
    let number = value.parse::<f64>().map_err(|_| format!("Invalid execution value: {value}"))?;
    if !number.is_finite() || number < 0.0 {
        return Err(format!("Invalid execution value: {value}"));
    }
    Ok(number)
}

pub(super) fn reconcile_positions(
    broker: &[super::types::Position], persisted: &[PositionRecord], now: &str,
) -> Result<Vec<PositionRecord>, String> {
    let mut records = std::collections::BTreeMap::<String, PositionRecord>::new();
    for position in broker {
        let occ = if position.asset_class == "us_option" {
            Some(parse_occ(&position.symbol).ok_or_else(|| format!("Invalid broker OCC: {}", position.symbol))?)
        } else { None };
        let root = occ.as_ref().map(|p| p.root.clone()).unwrap_or_else(|| position.symbol.clone());
        let quantity = position.qty.parse::<f64>().map_err(|e| e.to_string())?;
        if !quantity.is_finite() { return Err("Non-finite broker quantity".into()); }
        let quantity = match position.side.as_str() {
            "short" => -quantity.abs(), "long" => quantity.abs(),
            side => return Err(format!("Invalid broker position side: {side}")),
        };
        let price = nonnegative_number(&position.avg_entry_price)?;
        let old = persisted.iter().find(|p| p.symbol == root);
        let record = records.entry(root.clone()).or_insert_with(|| PositionRecord {
            symbol: root, qty: quantity, entry_price: price,
            entry_date: old.map(|p| p.entry_date.clone()).unwrap_or_else(|| now.to_string()),
            strategy: old.and_then(|p| p.strategy.clone()), expires_at: None,
            premium_collected: Some(price), occ_symbol: None,
            roll_count: old.map(|p| p.roll_count).unwrap_or(0), legs: Vec::new(),
        });
        if let Some(parts) = occ {
            record.legs.push(PositionLegRecord { occ_symbol: position.symbol.clone(), qty: quantity, entry_price: price });
            if record.legs.len() == 1 {
                record.occ_symbol = Some(position.symbol.clone());
                record.expires_at = Some(parts.expiry_str);
            } else {
                record.occ_symbol = None;
                record.qty = 0.0; // No single signed quantity describes a spread.
            }
        }
    }
    Ok(records.into_values().collect())
}

pub(super) fn position_from_fill(
    order: &Order, underlying: &str, strategy: Option<String>, fallback_time: &str,
) -> Result<Option<PositionRecord>, String> {
    if !order.is_terminal() { return Err(format!("Order {} is still {}", order.id, order.status)); }
    let reports: Vec<&Order> = match &order.legs {
        Some(legs) if !legs.is_empty() => legs.iter().collect(),
        _ => vec![order],
    };
    let mut legs = Vec::new();
    for report in reports {
        let quantity = nonnegative_number(&report.filled_qty)?;
        if quantity == 0.0 { continue; }
        let parts = parse_occ(&report.symbol).ok_or_else(|| format!("Invalid filled OCC: {}", report.symbol))?;
        if parts.root != underlying { return Err(format!("Unexpected fill root: {}", parts.root)); }
        let sign = match report.side.as_str() {
            "buy" => 1.0,
            "sell" => -1.0,
            side => return Err(format!("Invalid fill side: {side}")),
        };
        let price = nonnegative_number(report.filled_avg_price.as_deref()
            .ok_or_else(|| format!("Missing fill price for {}", report.symbol))?)?;
        legs.push(PositionLegRecord { occ_symbol: report.symbol.clone(), qty: sign * quantity, entry_price: price });
    }
    if legs.is_empty() {
        if nonnegative_number(&order.filled_qty)? > 0.0 {
            return Err("Parent reports executions but leg fills are missing".into());
        }
        return Ok(None);
    }
    let single = legs.len() == 1;
    let qty = if single { legs[0].qty } else { nonnegative_number(&order.filled_qty)? };
    let price = if single { legs[0].entry_price } else {
        // Aggregate net premium for display; management uses the actual legs.
        legs.iter().map(|l| -l.qty * l.entry_price).sum::<f64>() / qty.max(1.0)
    };
    let expires_at = legs.iter().filter_map(|l| parse_occ(&l.occ_symbol).map(|p| p.expiry_str)).min();
    Ok(Some(PositionRecord {
        symbol: underlying.to_string(), qty, entry_price: price,
        entry_date: order.filled_at.as_deref().unwrap_or(fallback_time).to_string(),
        strategy, expires_at, premium_collected: Some(price),
        occ_symbol: if single { Some(legs[0].occ_symbol.clone()) } else { None },
        roll_count: 0, legs,
    }))
}

pub(super) fn managed_positions(pos: &PositionRecord, spot: f64, sigma: f64) -> Vec<ManagedPosition> {
    let snapshot = |occ: Option<String>, qty, premium, expires_at| ManagedPosition {
        symbol: pos.symbol.clone(), occ_symbol: occ, qty, entry_premium: premium,
        expires_at, entry_date: pos.entry_date.clone(), roll_count: pos.roll_count,
        current_mark: premium.unwrap_or(pos.entry_price), spot, sigma,
    };
    if pos.legs.is_empty() {
        vec![snapshot(pos.occ_symbol.clone(), pos.qty, pos.premium_collected, pos.expires_at.clone())]
    } else {
        pos.legs.iter().map(|leg| snapshot(Some(leg.occ_symbol.clone()), leg.qty,
            Some(leg.entry_price), parse_occ(&leg.occ_symbol).map(|p| p.expiry_str))).collect()
    }
}

pub(super) fn invariant_positions(pos: &PositionRecord) -> Vec<InvariantPosition> {
    managed_positions(pos, 0.0, 0.0).into_iter().map(|p| InvariantPosition {
        symbol: p.symbol, occ_symbol: p.occ_symbol, qty: p.qty, current_mark: p.current_mark,
    }).collect()
}

/// Entry halts never suppress exits. A roll opens new risk, so replace it
/// with a close when entries are halted or when it belongs to a spread.
pub(super) fn management_action(actions: &[ManagementAction], entries_halted: bool, multi_leg: bool) -> Option<ManagementAction> {
    let action = actions.iter().find(|a| matches!(a,
        ManagementAction::DefensiveClose { .. } | ManagementAction::ForceCloseLong { .. }))
        .or_else(|| actions.iter().find(|a| !matches!(a, ManagementAction::Hold | ManagementAction::DeltaAlert { .. })))?;
    Some(match action {
        ManagementAction::Roll { symbol, occ, .. } if entries_halted || multi_leg => ManagementAction::DefensiveClose {
            symbol: symbol.clone(), occ: occ.clone(), reason: "Close instead of opening a replacement while entries are halted or a spread needs management".into(),
        },
        _ => action.clone(),
    })
}

/// Close shorts before removing their hedges. Preserve local state if any
/// close is rejected, partial, or unresolved, so later ticks can retry safely.
pub(super) async fn close_tracked_position(client: &AlpacaClient, pos: &PositionRecord) -> Result<(), Box<dyn std::error::Error>> {
    let mut legs = pos.legs.clone();
    legs.sort_by(|a, b| a.qty.total_cmp(&b.qty));
    let symbols = if legs.is_empty() {
        vec![pos.occ_symbol.as_deref().unwrap_or(&pos.symbol).to_string()]
    } else { legs.into_iter().map(|l| l.occ_symbol).collect() };
    for symbol in symbols {
        if let Err(e) = client.close_position_and_wait(&symbol).await {
            // Confirm absence on the position endpoint itself: a 404 while
            // polling an order does not prove that the position is gone.
            match client.get_position(&symbol).await {
                Err(check) if check.to_string().contains("404") => {},
                _ => return Err(e),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::{manage_open_positions, ManagementConfig};
    use crate::risk::invariants::{assert_invariants, BotState};

    fn report(status: &str, side: &str, qty: &str) -> Order {
        serde_json::from_value(serde_json::json!({
            "id":"o1", "client_order_id":"c1", "created_at":"2030-01-01T00:00:00Z",
            "asset_id":"a1", "symbol":"AAPL300215P00100000", "asset_class":"us_option",
            "qty":"5", "filled_qty":qty, "order_type":"market", "side":side,
            "time_in_force":"day", "filled_avg_price":"2.50", "status":status,
            "extended_hours":false
        })).unwrap()
    }

    #[test]
    fn only_executed_quantity_creates_a_position() {
        assert!(position_from_fill(&report("accepted", "sell", "0"), "AAPL", None, "now").is_err());
        for status in ["rejected", "canceled", "expired"] {
            assert!(position_from_fill(&report(status, "sell", "0"), "AAPL", None, "now").unwrap().is_none());
        }
        let pos = position_from_fill(&report("canceled", "sell", "2"), "AAPL", None, "now").unwrap().unwrap();
        assert_eq!(pos.qty, -2.0);
        assert_eq!(pos.entry_price, 2.5);
        assert_eq!(pos.expires_at.as_deref(), Some("2030-02-15"));
        assert_eq!(pos.legs[0].qty, -2.0);
        let mut malformed = report("filled", "sell", "2");
        malformed.filled_avg_price = None;
        assert!(position_from_fill(&malformed, "AAPL", None, "now").is_err());
    }

    fn spread() -> PositionRecord {
        let short = report("filled", "sell", "2");
        let mut long = report("filled", "buy", "2");
        long.symbol = "AAPL300215P00095000".into();
        long.filled_avg_price = Some("1.00".into());
        let mut parent = short.clone();
        parent.symbol = "AAPL".into();
        parent.legs = Some(vec![short, long]);
        position_from_fill(&parent, "AAPL", Some("spread".into()), "2030-01-01T00:00:00Z").unwrap().unwrap()
    }

    #[test]
    fn spread_legs_keep_signs_and_pass_hedge_invariants() {
        let pos = spread();
        let legs = invariant_positions(&pos);
        assert_eq!(legs.iter().map(|p| p.qty).collect::<Vec<_>>(), vec![-2.0, 2.0]);
        let state = BotState {
            positions: legs, equity: 100_000.0, start_of_day_equity: 100_000.0,
            was_circuit_broken: false, circuit_broken: false, max_risk_capital_pct: 0.05,
            max_daily_drawdown_pct: 0.05, block_long_premium: true, protected_equity: Default::default(),
        };
        assert!(assert_invariants(&state).is_empty());
    }

    #[tokio::test]
    async fn signed_spread_legs_survive_database_round_trip() {
        let store = crate::persistence::TradeStore::new(":memory:").await.unwrap();
        let original = spread();
        store.upsert_position(&original).await.unwrap();
        let restored = store.get_open_positions().await.unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].legs, original.legs);
        assert_eq!(invariant_positions(&restored[0])[0].qty, -2.0);
    }

    #[test]
    fn entry_halt_preserves_defensive_exit_and_converts_roll_to_close() {
        let pos = position_from_fill(&report("filled", "sell", "2"), "AAPL", None,
            "2030-01-01T00:00:00Z").unwrap().unwrap();
        let config = ManagementConfig {
            credit_target_pct: 0.5, roll_before_dte: 21, max_rolls: 2, roll_dte_days: 30,
            risk_free_rate: 0.045, profit_target_pct: 0.25, stop_loss_pct: 2.0,
            max_position_days: 90, itm_proximity_pct: 0.03, roll_trigger_pct: 0.05,
            block_long_premium: true, max_portfolio_delta_pct: 0.0,
            protected_equity: Default::default(), max_risk_per_symbol_pct: 0.0,
        };
        let today = chrono::NaiveDate::from_ymd_opt(2030, 1, 2).unwrap();
        let actions = manage_open_positions(&managed_positions(&pos, 70.0, 0.25), &config, 100_000.0, today);
        assert!(matches!(management_action(&actions, true, false), Some(ManagementAction::DefensiveClose { .. })));
        let roll = ManagementAction::Roll { symbol: "AAPL".into(), occ: pos.occ_symbol,
            new_dte_days: 30, roll_number: 1 };
        assert_eq!(management_action(&[roll.clone()], false, false), Some(roll.clone()));
        assert!(matches!(management_action(&[roll.clone()], true, false), Some(ManagementAction::DefensiveClose { .. })));
        assert!(matches!(management_action(&[roll], false, true), Some(ManagementAction::DefensiveClose { .. })));
    }

    #[test]
    fn reconciliation_repairs_legacy_sign_and_preserves_all_compact_occ_legs() {
        let broker_position = |symbol: &str, side: &str| serde_json::from_value(serde_json::json!({
            "asset_id":"a1", "symbol":symbol, "exchange":"OPRA", "asset_class":"us_option",
            "avg_entry_price":"2.5", "qty":"2", "side":side, "market_value":"0", "cost_basis":"0",
            "unrealized_pl":"0", "unrealized_plpc":"0", "unrealized_intraday_pl":"0",
            "unrealized_intraday_plpc":"0", "current_price":"2", "lastday_price":"2", "change_today":"0"
        })).unwrap();
        let mut legacy = spread();
        legacy.qty = 2.0;
        legacy.legs.clear();
        let broker = vec![broker_position("AAPL300215P00100000", "short"),
            broker_position("AAPL300215P00095000", "long")];
        let records = reconcile_positions(&broker, &[legacy], "now").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].symbol, "AAPL");
        assert_eq!(records[0].legs[0].qty, -2.0);
        assert_eq!(records[0].legs[1].qty, 2.0);
    }
}
