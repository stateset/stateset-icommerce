//! PostgreSQL implementation of the stock snapshot repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CaptureStockSnapshot, CommerceError, Result, StockSnapshot, StockSnapshotFilter,
    StockSnapshotId, StockSnapshotLine, StockSnapshotLineId, StockSnapshotRepository,
};

/// PostgreSQL-backed [`StockSnapshotRepository`].
#[derive(Debug, Clone)]
pub struct PgStockSnapshotRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct SnapshotRow {
    id: StockSnapshotId,
    label: Option<String>,
    total_skus: i64,
    total_units: Decimal,
    captured_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LineRow {
    id: StockSnapshotLineId,
    stock_snapshot_id: StockSnapshotId,
    product_id: uuid::Uuid,
    sku: String,
    quantity_on_hand: Decimal,
    quantity_available: Decimal,
    location: Option<String>,
}

impl PgStockSnapshotRepository {
    /// Create a new repository over the given pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_head(row: SnapshotRow) -> StockSnapshot {
        StockSnapshot {
            id: row.id,
            label: row.label,
            total_skus: row.total_skus.max(0) as u64,
            total_units: row.total_units,
            lines: Vec::new(),
            captured_at: row.captured_at,
        }
    }

    fn row_to_line(row: LineRow) -> StockSnapshotLine {
        StockSnapshotLine {
            id: row.id,
            stock_snapshot_id: row.stock_snapshot_id,
            product_id: row.product_id.into(),
            sku: row.sku,
            quantity_on_hand: row.quantity_on_hand,
            quantity_available: row.quantity_available,
            location: row.location,
        }
    }

    async fn load_lines(&self, id: StockSnapshotId) -> Result<Vec<StockSnapshotLine>> {
        let rows = sqlx::query_as::<_, LineRow>(
            "SELECT * FROM stock_snapshot_lines WHERE stock_snapshot_id = $1 ORDER BY sku",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_line).collect())
    }

    async fn load_full(&self, id: StockSnapshotId) -> Result<Option<StockSnapshot>> {
        let row = sqlx::query_as::<_, SnapshotRow>("SELECT * FROM stock_snapshots WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut head = Self::row_to_head(row);
        head.lines = self.load_lines(id).await?;
        Ok(Some(head))
    }

    /// Capture a new stock snapshot.
    pub async fn capture_async(&self, input: CaptureStockSnapshot) -> Result<StockSnapshot> {
        if input.lines.is_empty() {
            return Err(CommerceError::ValidationError(
                "a stock snapshot requires at least one line".into(),
            ));
        }
        let id = StockSnapshotId::new();
        let now = Utc::now();
        let total_skus = input.total_skus();
        let total_units = input.total_units();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO stock_snapshots (id, label, total_skus, total_units, captured_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(&input.label)
        .bind(total_skus as i64)
        .bind(total_units)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for line in &input.lines {
            sqlx::query(
                "INSERT INTO stock_snapshot_lines (id, stock_snapshot_id, product_id, sku, quantity_on_hand, quantity_available, location)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(StockSnapshotLineId::new())
            .bind(id)
            .bind(*line.product_id.as_uuid())
            .bind(&line.sku)
            .bind(line.quantity_on_hand)
            .bind(line.quantity_available)
            .bind(&line.location)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.load_full(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create stock snapshot".into()))
    }

    /// Get a snapshot by ID (with lines).
    pub async fn get_async(&self, id: StockSnapshotId) -> Result<Option<StockSnapshot>> {
        self.load_full(id).await
    }

    /// Get the most recent snapshot, if any.
    pub async fn latest_async(&self) -> Result<Option<StockSnapshot>> {
        let id: Option<StockSnapshotId> =
            sqlx::query_scalar("SELECT id FROM stock_snapshots ORDER BY captured_at DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        match id {
            Some(id) => self.load_full(id).await,
            None => Ok(None),
        }
    }

    /// List snapshots (header-level; lines omitted, fetch via `get`).
    pub async fn list_async(&self, filter: StockSnapshotFilter) -> Result<Vec<StockSnapshot>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT * FROM stock_snapshots ORDER BY captured_at DESC",
        );
        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(i64::from(offset));
        }
        let rows = builder
            .build_query_as::<SnapshotRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_head).collect())
    }

    /// Delete a snapshot and its lines.
    pub async fn delete_async(&self, id: StockSnapshotId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query("DELETE FROM stock_snapshot_lines WHERE stock_snapshot_id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("DELETE FROM stock_snapshots WHERE id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }
}

impl StockSnapshotRepository for PgStockSnapshotRepository {
    fn capture(&self, input: CaptureStockSnapshot) -> Result<StockSnapshot> {
        block_on(self.capture_async(input))
    }

    fn get(&self, id: StockSnapshotId) -> Result<Option<StockSnapshot>> {
        block_on(self.get_async(id))
    }

    fn latest(&self) -> Result<Option<StockSnapshot>> {
        block_on(self.latest_async())
    }

    fn list(&self, filter: StockSnapshotFilter) -> Result<Vec<StockSnapshot>> {
        block_on(self.list_async(filter))
    }

    fn delete(&self, id: StockSnapshotId) -> Result<()> {
        block_on(self.delete_async(id))
    }
}
