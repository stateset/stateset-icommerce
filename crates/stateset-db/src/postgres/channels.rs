//! PostgreSQL channel repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    Channel, ChannelFilter, ChannelId, ChannelProductMapping, ChannelProductSyncItem,
    ChannelRepository, ChannelStatus, ChannelType, CommerceError, CreateChannel, Result,
    UpdateChannel,
};
use uuid::Uuid;

/// PostgreSQL implementation of `ChannelRepository`
#[derive(Debug, Clone)]
pub struct PgChannelRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ChannelRow {
    id: Uuid,
    name: String,
    channel_type: String,
    integration: Option<String>,
    status: String,
    api_locked: bool,
    default_warehouse_id: Option<Uuid>,
    tags: serde_json::Value,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct MappingRow {
    channel_id: Uuid,
    channel_sku: String,
    product_id: Uuid,
    internal_sku: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgChannelRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_channel(row: ChannelRow) -> Result<Channel> {
        let channel_type: ChannelType = row.channel_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid channel.channel_type '{}': {}",
                row.channel_type, e
            ))
        })?;
        let status: ChannelStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid channel.status '{}': {}", row.status, e))
        })?;
        let tags: Vec<String> = serde_json::from_value(row.tags)
            .map_err(|e| CommerceError::DatabaseError(format!("Invalid channel.tags: {e}")))?;
        Ok(Channel {
            id: row.id.into(),
            name: row.name,
            channel_type,
            integration: row.integration,
            status,
            api_locked: row.api_locked,
            default_warehouse_id: row.default_warehouse_id.map(Into::into),
            tags,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_mapping(row: MappingRow) -> ChannelProductMapping {
        ChannelProductMapping {
            channel_id: row.channel_id.into(),
            channel_sku: row.channel_sku,
            product_id: row.product_id.into(),
            internal_sku: row.internal_sku,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<Channel>> {
        let row = sqlx::query_as::<_, ChannelRow>("SELECT * FROM channels WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_channel).transpose()
    }

    /// Create a channel (async)
    pub async fn create_async(&self, input: CreateChannel) -> Result<Channel> {
        let id = ChannelId::new();
        let now = Utc::now();
        let tags = serde_json::to_value(&input.tags)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO channels (id, name, channel_type, integration, status, api_locked, default_warehouse_id, tags, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, 'active', FALSE, $5, $6, $7, $8, $8)",
        )
        .bind(Uuid::from(id))
        .bind(&input.name)
        .bind(input.channel_type.to_string())
        .bind(&input.integration)
        .bind(input.default_warehouse_id.map(Uuid::from))
        .bind(tags)
        .bind(&input.metadata)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a channel by ID (async)
    pub async fn get_async(&self, id: ChannelId) -> Result<Option<Channel>> {
        self.fetch_async(id.into()).await
    }

    /// Update a channel with PATCH/merge semantics (async).
    ///
    /// Rejects mutations on API-locked channels with `Conflict`, mirroring
    /// the SQLite implementation.
    pub async fn update_async(&self, id: ChannelId, input: UpdateChannel) -> Result<Channel> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        if existing.api_locked {
            return Err(CommerceError::Conflict("channel is API-locked".into()));
        }
        let now = Utc::now();

        let name = input.name.unwrap_or(existing.name);
        let integration = input.integration.or(existing.integration);
        let status = input.status.unwrap_or(existing.status);
        let default_warehouse_id = input
            .default_warehouse_id
            .map(Uuid::from)
            .or(existing.default_warehouse_id.map(Uuid::from));
        let tags = serde_json::to_value(input.tags.as_ref().unwrap_or(&existing.tags))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let metadata = input.metadata.unwrap_or(existing.metadata);

        sqlx::query(
            "UPDATE channels SET name = $1, integration = $2, status = $3, default_warehouse_id = $4, tags = $5, metadata = $6, updated_at = $7 WHERE id = $8",
        )
        .bind(&name)
        .bind(&integration)
        .bind(status.to_string())
        .bind(default_warehouse_id)
        .bind(tags)
        .bind(&metadata)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List channels (async). Soft-deleted channels are always excluded.
    pub async fn list_async(&self, filter: ChannelFilter) -> Result<Vec<Channel>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM channels WHERE status != 'deleted'");
        let mut param_idx = 1;
        if filter.channel_type.is_some() {
            query.push_str(&format!(" AND channel_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        if filter.integration.is_some() {
            query.push_str(&format!(" AND integration = ${param_idx}"));
            param_idx += 1;
        }
        if filter.api_locked.is_some() {
            query.push_str(&format!(" AND api_locked = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, ChannelRow>(&query);
        if let Some(t) = filter.channel_type {
            q = q.bind(t.to_string());
        }
        if let Some(s) = filter.status {
            q = q.bind(s.to_string());
        }
        if let Some(integration) = filter.integration {
            q = q.bind(integration);
        }
        if let Some(locked) = filter.api_locked {
            q = q.bind(locked);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_channel).collect()
    }

    /// Soft-delete a channel (async). Errors if the channel is API-locked.
    pub async fn delete_async(&self, id: ChannelId) -> Result<()> {
        let locked: Option<bool> =
            sqlx::query_scalar("SELECT api_locked FROM channels WHERE id = $1")
                .bind(Uuid::from(id))
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        let locked = locked.ok_or(CommerceError::NotFound)?;
        if locked {
            return Err(CommerceError::Conflict("channel is API-locked".into()));
        }
        sqlx::query("UPDATE channels SET status = 'deleted' WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Set the channel's lock state (async). Lock changes bypass the lock check.
    pub async fn set_lock_async(&self, id: ChannelId, locked: bool) -> Result<Channel> {
        let now = Utc::now();
        sqlx::query("UPDATE channels SET api_locked = $1, updated_at = $2 WHERE id = $3")
            .bind(locked)
            .bind(now)
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Bulk upsert/delete channel SKU mappings (async). Returns affected count.
    pub async fn sync_products_async(
        &self,
        id: ChannelId,
        items: Vec<ChannelProductSyncItem>,
    ) -> Result<u64> {
        let channel_id = Uuid::from(id);
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut affected: u64 = 0;
        for item in &items {
            if item.delete {
                let result = sqlx::query(
                    "DELETE FROM channel_product_mappings WHERE channel_id = $1 AND channel_sku = $2",
                )
                .bind(channel_id)
                .bind(&item.channel_sku)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                affected += result.rows_affected();
            } else {
                let product_id = item.product_id.ok_or_else(|| {
                    CommerceError::ValidationError(
                        "product_id is required when delete is false".into(),
                    )
                })?;
                let internal_sku = item.internal_sku.clone().unwrap_or_default();
                let result = sqlx::query(
                    "INSERT INTO channel_product_mappings (channel_id, channel_sku, product_id, internal_sku, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $5)
                     ON CONFLICT (channel_id, channel_sku) DO UPDATE SET
                        product_id = EXCLUDED.product_id,
                        internal_sku = EXCLUDED.internal_sku,
                        updated_at = EXCLUDED.updated_at",
                )
                .bind(channel_id)
                .bind(&item.channel_sku)
                .bind(Uuid::from(product_id))
                .bind(&internal_sku)
                .bind(now)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                affected += result.rows_affected();
            }
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(affected)
    }

    /// List a channel's SKU mappings (async), ordered by channel SKU.
    pub async fn list_product_mappings_async(
        &self,
        id: ChannelId,
    ) -> Result<Vec<ChannelProductMapping>> {
        let rows = sqlx::query_as::<_, MappingRow>(
            "SELECT * FROM channel_product_mappings WHERE channel_id = $1 ORDER BY channel_sku",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_mapping).collect())
    }
}

impl ChannelRepository for PgChannelRepository {
    fn create(&self, input: CreateChannel) -> Result<Channel> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ChannelId) -> Result<Option<Channel>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: ChannelId, input: UpdateChannel) -> Result<Channel> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ChannelFilter) -> Result<Vec<Channel>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: ChannelId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn set_lock(&self, id: ChannelId, locked: bool) -> Result<Channel> {
        super::block_on(self.set_lock_async(id, locked))
    }

    fn sync_products(&self, id: ChannelId, items: Vec<ChannelProductSyncItem>) -> Result<u64> {
        super::block_on(self.sync_products_async(id, items))
    }

    fn list_product_mappings(&self, id: ChannelId) -> Result<Vec<ChannelProductMapping>> {
        super::block_on(self.list_product_mappings_async(id))
    }
}
