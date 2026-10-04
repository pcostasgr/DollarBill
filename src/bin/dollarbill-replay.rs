//! Offline execution/replay CLI. Never loads credentials or contacts a broker.
use clap::{Parser, Subcommand};
use dollarbill::{build_info::BuildInfo, domain::*, execution::{*, simulator::*, store::EventStore}};
use serde::{Deserialize, Serialize};
use std::{fs, io::{BufRead, BufReader, Write}, path::{Path, PathBuf}};

#[derive(Parser)]
#[command(version, about = "Deterministic offline execution and portfolio replay")]
struct Cli { #[command(subcommand)] command: Command }
#[derive(Subcommand)]
enum Command {
    BuildInfo,
    Simulate {
        #[arg(long)] scenario: PathBuf,
        /// New directory; existing output directories are never overwritten.
        #[arg(long)] output: PathBuf,
    },
    Replay { #[arg(long)] events: PathBuf },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    schema_version: u32,
    run_id: String,
    initial_cash: Money,
    config: SimulationConfig,
    actions: Vec<Action>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Submit { timestamp_ms: i64, intent: OrderIntent },
    Quote { quote: Quote },
    Cancel { timestamp_ms: i64, order_id: String },
    CircuitBreaker { timestamp_ms: i64, reason: String },
    SubmitTimeout { timestamp_ms: i64, intent: OrderIntent },
    Settle { timestamp_ms: i64, instrument: Instrument, quantity: Quantity, assigned: bool, settlement_id: String, fee: Money },
    Expire { timestamp_ms: i64, instrument: Instrument, spot: Price, threshold: Price, fee: Money },
}

type CliResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn write_json(path: &Path, value: &impl Serialize) -> CliResult<()> {
    let mut file = fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    writeln!(file)?;
    Ok(())
}

#[tokio::main]
async fn main() -> CliResult<()> {
    match Cli::parse().command {
        Command::BuildInfo => println!("{}", serde_json::to_string_pretty(&BuildInfo::current())?),
        Command::Replay { events } => {
            let events: Vec<TradingEvent> = BufReader::new(fs::File::open(events)?).lines()
                .map(|line| -> CliResult<_> { Ok(serde_json::from_str(&line?)?) }).collect::<CliResult<_>>()?;
            println!("{}", serde_json::to_string_pretty(&replay(&events)?)?);
        }
        Command::Simulate { scenario, output } => {
            let input = fs::read(&scenario)?;
            let scenario: Scenario = serde_json::from_slice(&input)?;
            if scenario.schema_version != 1 { return Err("unsupported scenario schema".into()); }
            let manifest = RunManifest {
                build: BuildInfo::current(), config_sha256: sha256(&serde_json::to_vec(&scenario.config)?),
                dataset_sha256: sha256(&input), initial_cash: scenario.initial_cash, currency: "USD".into(),
                execution_model: "independent_legs_bid_ask_v1".into(),
            };
            let mut venue = SimulatedVenue::new(scenario.run_id.clone(), manifest.clone(), scenario.config.clone())?;
            for action in &scenario.actions {
                match action {
                    Action::Submit { timestamp_ms, intent } => { venue.submit(intent.clone(), *timestamp_ms).await?; }
                    Action::Quote { quote } => { venue.on_quote(quote.clone())?; }
                    Action::Cancel { timestamp_ms, order_id } => { venue.cancel(order_id, *timestamp_ms).await?; }
                    Action::CircuitBreaker { timestamp_ms, reason } => { venue.trip_circuit_breaker(*timestamp_ms, reason.clone())?; }
                    Action::SubmitTimeout { timestamp_ms, intent } => {
                        let outcome = venue.submit(intent.clone(), *timestamp_ms).await;
                        if outcome.is_ok() || venue.poll(&intent.client_order_id).await?.status != OrderStatus::Accepted {
                            return Err("expected timeout after acceptance".into());
                        }
                    }
                    Action::Settle { timestamp_ms, instrument, quantity, assigned, settlement_id, fee } => {
                        venue.settle(instrument.clone(), *quantity, *assigned, settlement_id.clone(), *fee, *timestamp_ms)?;
                    }
                    Action::Expire { timestamp_ms, instrument, spot, threshold, fee } => {
                        venue.expire(instrument.clone(), *spot, *threshold, *fee, *timestamp_ms)?;
                    }
                }
            }
            if replay(venue.events())? != *venue.state() { return Err("simulation/replay divergence".into()); }
            if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) { fs::create_dir_all(parent)?; }
            fs::create_dir(&output)?;
            // Persist and reopen before reporting success; test the actual journal path.
            let database = output.join("events.sqlite");
            let database = database.to_str().ok_or("non-UTF8 database path")?;
            let store = EventStore::open(database).await?;
            for event in venue.events() { store.append(event).await?; }
            store.close().await;
            let reopened = EventStore::open(database).await?;
            let events = reopened.events(&scenario.run_id).await?;
            if replay(&events)? != *venue.state() { return Err("database recovery/replay divergence".into()); }
            reopened.close().await;
            let mut file = fs::File::create(output.join("events.jsonl"))?;
            for event in &events { serde_json::to_writer(&mut file, event)?; writeln!(file)?; }
            file.sync_all()?;
            fs::write(output.join("scenario.json"), &input)?;
            write_json(&output.join("config_snapshot.json"), &scenario.config)?;
            write_json(&output.join("state.json"), venue.state())?;
            write_json(&output.join("orders.json"), &venue.state().orders)?;
            let fills: Vec<_> = events.iter().filter_map(|event| if let EventPayload::FillReceived { fill } = &event.payload { Some(fill) } else { None }).collect();
            write_json(&output.join("fills.json"), &fills)?;
            write_json(&output.join("positions.json"), &venue.state().portfolio.positions)?;
            write_json(&output.join("metrics.json"), &serde_json::json!({
                "event_count": events.len(), "fill_count": fills.len(), "cash": venue.state().portfolio.cash,
                "realized_pnl": venue.state().portfolio.realized_pnl, "fees": venue.state().portfolio.total_fees,
                "open_instruments": venue.state().portfolio.positions.len(), "replay_equal": true,
                "note": "Cash and realized P&L only; no mark-to-market performance claim"
            }))?;
            // Completion manifest is written last. Its absence denotes an incomplete export.
            write_json(&output.join("run.json"), &manifest)?;
            println!("{} events; simulation, JSON replay and reopened SQLite state agree. Output: {}", events.len(), output.display());
        }
    }
    Ok(())
}
