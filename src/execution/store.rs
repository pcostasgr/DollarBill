//! Append-only SQLite journal. Sequence validation and insertion share one
//! transaction; conflicting retries, malformed history and schema drift fail.
use super::*;
use sqlx::{Row, SqlitePool, sqlite::{SqliteConnectOptions, SqlitePoolOptions}};

pub struct EventStore { pool: SqlitePool }
pub type StoreResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl EventStore {
    pub async fn open(path: &str) -> StoreResult<Self> {
        let pool = SqlitePoolOptions::new().max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(path).create_if_missing(true)).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS trading_events_v1 (
            run_id TEXT NOT NULL, sequence INTEGER NOT NULL CHECK(sequence > 0),
            event_json TEXT NOT NULL, PRIMARY KEY(run_id, sequence))").execute(&pool).await?;
        sqlx::query("CREATE TRIGGER IF NOT EXISTS trading_events_v1_no_update
            BEFORE UPDATE ON trading_events_v1 BEGIN SELECT RAISE(ABORT, 'event journal is append-only'); END")
            .execute(&pool).await?;
        sqlx::query("CREATE TRIGGER IF NOT EXISTS trading_events_v1_no_delete
            BEFORE DELETE ON trading_events_v1 BEGIN SELECT RAISE(ABORT, 'event journal is append-only'); END")
            .execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn append(&self, event: &TradingEvent) -> StoreResult<()> {
        self.append_batch(std::slice::from_ref(event)).await
    }

    /// A broker report's fills, terminal state and receipt commit together.
    /// Retrying after an uncertain commit cannot double-book a partial report.
    pub async fn append_batch(&self, batch: &[TradingEvent]) -> StoreResult<()> {
        let Some(first) = batch.first() else { return Ok(()) };
        require(batch.iter().all(|e| e.run_id == first.run_id && e.sequence <= i64::MAX as u64), "mixed run IDs or sequence outside SQLite range")?;
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query("SELECT event_json FROM trading_events_v1 WHERE run_id = ? ORDER BY sequence")
            .bind(&first.run_id).fetch_all(&mut *tx).await?;
        let mut events: Vec<TradingEvent> = rows.iter().map(|r| serde_json::from_str(r.get::<&str, _>("event_json")))
            .collect::<std::result::Result<_, _>>()?;
        let mut state = if events.is_empty() { ReplayState::default() } else { replay(&events)? };
        for event in batch {
            if let Some(existing) = events.iter().find(|e| e.sequence == event.sequence) {
                require(existing == event, "conflicting event retry")?;
                continue;
            }
            state.apply(event)?;
            sqlx::query("INSERT INTO trading_events_v1 (run_id, sequence, event_json) VALUES (?, ?, ?)")
                .bind(&event.run_id).bind(event.sequence as i64).bind(serde_json::to_string(event)?)
                .execute(&mut *tx).await?;
            events.push(event.clone());
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn events(&self, run_id: &str) -> StoreResult<Vec<TradingEvent>> {
        let rows = sqlx::query("SELECT event_json FROM trading_events_v1 WHERE run_id = ? ORDER BY sequence")
            .bind(run_id).fetch_all(&self.pool).await?;
        let events: Vec<TradingEvent> = rows.iter().map(|r| serde_json::from_str(r.get::<&str, _>("event_json")))
            .collect::<std::result::Result<_, _>>()?;
        if !events.is_empty() { replay(&events)?; }
        Ok(events)
    }
    pub async fn close(self) { self.pool.close().await; }
}
