// SQLite persistence for trade history and open positions
//
// `TradeStore::new(path)` opens (or creates) a SQLite database at `path`
// and runs an inline schema migration so callers never need to manage the DB
// schema manually.

use sqlx::{Row, SqlitePool};

// ─── Public record types ──────────────────────────────────────────────────

/// A single trade execution persisted to the database.
#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub symbol:      String,
    /// "buy" | "sell" | "sell_short" | "buy_to_cover" | "tick"
    pub action:      String,
    pub quantity:    f64,
    pub price:       f64,
    pub order_id:    Option<String>,
    pub fill_status: Option<String>,
    pub strategy:    Option<String>,
    /// RFC-3339 timestamp
    pub timestamp:   String,
}

/// An open (or recently closed) position.
#[derive(Debug, Clone)]
pub struct PositionRecord {
    pub symbol:      String,
    pub qty:         f64,
    pub entry_price: f64,
    /// RFC-3339 date of entry
    pub entry_date:  String,
    pub strategy:    Option<String>,
}

// ─── TradeStore ───────────────────────────────────────────────────────────

/// Persistent trade and position store backed by SQLite.
///
/// # Example
/// ```no_run
/// use dollarbill::persistence::{TradeStore, TradeRecord};
/// # async fn run() {
/// let store = TradeStore::new("data/trades.db").await.unwrap();
/// let rec = TradeRecord {
///     symbol: "AAPL".into(), action: "buy".into(),
///     quantity: 1.0, price: 195.0,
///     order_id: None, fill_status: Some("filled".into()),
///     strategy: Some("Momentum".into()),
///     timestamp: "2025-01-01T09:30:00Z".into(),
/// };
/// store.insert_trade(&rec).await.unwrap();
/// # }
/// ```
pub struct TradeStore {
    pool: SqlitePool,
}

impl TradeStore {
    /// Open (or create) `db_path` and run schema migrations.
    pub async fn new(db_path: &str) -> Result<Self, sqlx::Error> {
        // `mode=rwc` creates the file if it does not exist.
        let url = format!("sqlite:{}?mode=rwc", db_path);
        let pool = SqlitePool::connect(&url).await?;
        Self::migrate(&pool).await?;
        Ok(Self { pool })
    }

    /// Idempotent schema migration — safe to call on every start-up.
    async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS trades (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol      TEXT    NOT NULL,
                action      TEXT    NOT NULL,
                quantity    REAL    NOT NULL,
                price       REAL    NOT NULL,
                order_id    TEXT,
                fill_status TEXT,
                strategy    TEXT,
                timestamp   TEXT    NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS positions (
                symbol      TEXT PRIMARY KEY,
                qty         REAL NOT NULL,
                entry_price REAL NOT NULL,
                entry_date  TEXT NOT NULL,
                strategy    TEXT
            )",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Insert a new trade execution record.
    pub async fn insert_trade(&self, r: &TradeRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO trades
             (symbol, action, quantity, price, order_id, fill_status, strategy, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&r.symbol)
        .bind(&r.action)
        .bind(r.quantity)
        .bind(r.price)
        .bind(&r.order_id)
        .bind(&r.fill_status)
        .bind(&r.strategy)
        .bind(&r.timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert or replace an open position (symbol is the primary key).
    pub async fn upsert_position(&self, p: &PositionRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO positions
             (symbol, qty, entry_price, entry_date, strategy)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&p.symbol)
        .bind(p.qty)
        .bind(p.entry_price)
        .bind(&p.entry_date)
        .bind(&p.strategy)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a position (call when a position is closed or filled).
    pub async fn close_position(&self, symbol: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM positions WHERE symbol = ?1")
            .bind(symbol)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Return all currently open positions.
    pub async fn get_open_positions(&self) -> Result<Vec<PositionRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT symbol, qty, entry_price, entry_date, strategy FROM positions",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PositionRecord {
                symbol:      row.get("symbol"),
                qty:         row.get("qty"),
                entry_price: row.get("entry_price"),
                entry_date:  row.get("entry_date"),
                strategy:    row.get("strategy"),
            })
            .collect())
    }

    /// Return the most recent `limit` trade records, newest first.
    pub async fn get_trade_history(
        &self,
        limit: u32,
    ) -> Result<Vec<TradeRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT symbol, action, quantity, price, order_id, fill_status, strategy, timestamp
             FROM trades
             ORDER BY id DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeRecord {
                symbol:      row.get("symbol"),
                action:      row.get("action"),
                quantity:    row.get("quantity"),
                price:       row.get("price"),
                order_id:    row.get("order_id"),
                fill_status: row.get("fill_status"),
                strategy:    row.get("strategy"),
                timestamp:   row.get("timestamp"),
            })
            .collect())
    }
}
