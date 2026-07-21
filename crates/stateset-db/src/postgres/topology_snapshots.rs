//! PostgreSQL topology snapshot repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CaptureTopologySnapshot, CommerceError, HealthGrade, Result, TopologySnapshot,
    TopologySnapshotFilter, TopologySnapshotId, TopologySnapshotRepository,
};
use uuid::Uuid;

/// PostgreSQL implementation of `TopologySnapshotRepository`
#[derive(Debug, Clone)]
pub struct PgTopologySnapshotRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct TopologySnapshotRow {
    id: Uuid,
    channels_total: i64,
    channels_active: i64,
    warehouses_total: i64,
    products_total: i64,
    open_orders: i64,
    health: String,
    signals: serde_json::Value,
    captured_at: DateTime<Utc>,
}

/// Convert a `u64` metric to an `i64` column value.
fn metric_to_i64(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| {
        CommerceError::DatabaseError(format!(
            "topology_snapshot.{field} value {value} exceeds BIGINT range"
        ))
    })
}

impl PgTopologySnapshotRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_snapshot(row: TopologySnapshotRow) -> Result<TopologySnapshot> {
        let health: HealthGrade = row.health.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid topology_snapshot.health '{}': {}",
                row.health, e
            ))
        })?;
        Ok(TopologySnapshot {
            id: row.id.into(),
            channels_total: u64::try_from(row.channels_total).unwrap_or(0),
            channels_active: u64::try_from(row.channels_active).unwrap_or(0),
            warehouses_total: u64::try_from(row.warehouses_total).unwrap_or(0),
            products_total: u64::try_from(row.products_total).unwrap_or(0),
            open_orders: u64::try_from(row.open_orders).unwrap_or(0),
            health,
            signals: row.signals,
            captured_at: row.captured_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<TopologySnapshot>> {
        let row = sqlx::query_as::<_, TopologySnapshotRow>(
            "SELECT * FROM topology_snapshots WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_snapshot).transpose()
    }

    /// Capture a snapshot (async). The health grade is derived from the metrics.
    pub async fn capture_async(&self, input: CaptureTopologySnapshot) -> Result<TopologySnapshot> {
        let id = TopologySnapshotId::new();
        let now = Utc::now();
        let health = input.derive_health();

        sqlx::query(
            "INSERT INTO topology_snapshots (id, channels_total, channels_active, warehouses_total, products_total, open_orders, health, signals, captured_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(Uuid::from(id))
        .bind(metric_to_i64(input.channels_total, "channels_total")?)
        .bind(metric_to_i64(input.channels_active, "channels_active")?)
        .bind(metric_to_i64(input.warehouses_total, "warehouses_total")?)
        .bind(metric_to_i64(input.products_total, "products_total")?)
        .bind(metric_to_i64(input.open_orders, "open_orders")?)
        .bind(health.to_string())
        .bind(&input.signals)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a snapshot by ID (async)
    pub async fn get_async(&self, id: TopologySnapshotId) -> Result<Option<TopologySnapshot>> {
        self.fetch_async(id.into()).await
    }

    /// Get the most recent snapshot (async)
    pub async fn latest_async(&self) -> Result<Option<TopologySnapshot>> {
        let row = sqlx::query_as::<_, TopologySnapshotRow>(
            "SELECT * FROM topology_snapshots ORDER BY captured_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_snapshot).transpose()
    }

    /// List snapshots, most recent first (async)
    pub async fn list_async(
        &self,
        filter: TopologySnapshotFilter,
    ) -> Result<Vec<TopologySnapshot>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM topology_snapshots WHERE 1=1");
        let mut param_idx = 1;
        if filter.health.is_some() {
            query.push_str(&format!(" AND health = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY captured_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, TopologySnapshotRow>(&query);
        if let Some(health) = filter.health {
            q = q.bind(health.to_string());
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_snapshot).collect()
    }

    /// Delete a snapshot (async)
    pub async fn delete_async(&self, id: TopologySnapshotId) -> Result<()> {
        sqlx::query("DELETE FROM topology_snapshots WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }
}

impl TopologySnapshotRepository for PgTopologySnapshotRepository {
    fn capture(&self, input: CaptureTopologySnapshot) -> Result<TopologySnapshot> {
        super::block_on(self.capture_async(input))
    }

    fn get(&self, id: TopologySnapshotId) -> Result<Option<TopologySnapshot>> {
        super::block_on(self.get_async(id))
    }

    fn latest(&self) -> Result<Option<TopologySnapshot>> {
        super::block_on(self.latest_async())
    }

    fn list(&self, filter: TopologySnapshotFilter) -> Result<Vec<TopologySnapshot>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: TopologySnapshotId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }
}
