//! PostgreSQL purgatory (order ingestion staging) repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, IngestOrder, MapPurgatoryLine, PurgatoryFilter, PurgatoryLineItem,
    PurgatoryLineItemId, PurgatoryOrder, PurgatoryOrderId, PurgatoryRepository, Result,
};
use uuid::Uuid;

/// PostgreSQL implementation of `PurgatoryRepository`
#[derive(Debug, Clone)]
pub struct PgPurgatoryRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PurgatoryOrderRow {
    id: Uuid,
    channel_id: Option<Uuid>,
    external_order_id: String,
    external_status: Option<String>,
    is_posted: bool,
    hold_reason: Option<String>,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PurgatoryLineRow {
    id: Uuid,
    purgatory_order_id: Uuid,
    external_sku: String,
    product_id: Option<Uuid>,
    quantity: Decimal,
    ignore_item: bool,
    non_physical: bool,
}

impl PgPurgatoryRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_head(row: PurgatoryOrderRow) -> PurgatoryOrder {
        PurgatoryOrder {
            id: row.id.into(),
            channel_id: row.channel_id.map(Into::into),
            external_order_id: row.external_order_id,
            external_status: row.external_status,
            is_posted: row.is_posted,
            hold_reason: row.hold_reason,
            metadata: row.metadata,
            items: Vec::new(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_line(row: PurgatoryLineRow) -> PurgatoryLineItem {
        PurgatoryLineItem {
            id: row.id.into(),
            purgatory_order_id: row.purgatory_order_id.into(),
            external_sku: row.external_sku,
            product_id: row.product_id.map(Into::into),
            quantity: row.quantity,
            ignore_item: row.ignore_item,
            non_physical: row.non_physical,
        }
    }

    async fn fetch_full_conn(
        conn: &mut sqlx::PgConnection,
        id: Uuid,
    ) -> Result<Option<PurgatoryOrder>> {
        let head =
            sqlx::query_as::<_, PurgatoryOrderRow>("SELECT * FROM purgatory_orders WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?;
        let Some(head) = head else { return Ok(None) };
        let lines = sqlx::query_as::<_, PurgatoryLineRow>(
            "SELECT * FROM purgatory_line_items WHERE purgatory_order_id = $1 ORDER BY external_sku",
        )
        .bind(id)
        .fetch_all(&mut *conn)
        .await
        .map_err(map_db_error)?;
        let mut order = Self::row_to_head(head);
        order.items = lines.into_iter().map(Self::row_to_line).collect();
        Ok(Some(order))
    }

    async fn fetch_full_async(&self, id: Uuid) -> Result<Option<PurgatoryOrder>> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        Self::fetch_full_conn(conn.as_mut(), id).await
    }

    /// Ingest an order into purgatory (async)
    pub async fn ingest_async(&self, input: IngestOrder) -> Result<PurgatoryOrder> {
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "an ingested order requires at least one line".into(),
            ));
        }
        let id = PurgatoryOrderId::new();
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO purgatory_orders (id, channel_id, external_order_id, external_status, is_posted, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, FALSE, $5, $6, $6)",
        )
        .bind(Uuid::from(id))
        .bind(input.channel_id.map(Uuid::from))
        .bind(&input.external_order_id)
        .bind(&input.external_status)
        .bind(&input.metadata)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for item in &input.items {
            sqlx::query(
                "INSERT INTO purgatory_line_items (id, purgatory_order_id, external_sku, product_id, quantity, ignore_item, non_physical)
                 VALUES ($1, $2, $3, $4, $5, FALSE, FALSE)",
            )
            .bind(Uuid::from(PurgatoryLineItemId::new()))
            .bind(Uuid::from(id))
            .bind(&item.external_sku)
            .bind(item.product_id.map(Uuid::from))
            .bind(item.quantity)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        tx.commit().await.map_err(map_db_error)?;

        self.fetch_full_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a purgatory order by ID with line items (async)
    pub async fn get_async(&self, id: PurgatoryOrderId) -> Result<Option<PurgatoryOrder>> {
        self.fetch_full_async(id.into()).await
    }

    /// List purgatory orders with filter (async); defaults to non-posted.
    pub async fn list_async(&self, filter: PurgatoryFilter) -> Result<Vec<PurgatoryOrder>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));
        // Defaults to non-posted when not specified.
        let is_posted = filter.is_posted.unwrap_or(false);

        let mut query = String::from("SELECT * FROM purgatory_orders WHERE 1=1");
        let mut param_idx = 1;
        if filter.channel_id.is_some() {
            query.push_str(&format!(" AND channel_id = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(" AND is_posted = ${param_idx}"));
        param_idx += 1;
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PurgatoryOrderRow>(&query);
        if let Some(channel) = filter.channel_id {
            q = q.bind(Uuid::from(channel));
        }
        let rows = q
            .bind(is_posted)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut orders: Vec<PurgatoryOrder> = rows.into_iter().map(Self::row_to_head).collect();
        if orders.is_empty() {
            return Ok(orders);
        }

        let ids: Vec<Uuid> = orders.iter().map(|o| Uuid::from(o.id)).collect();
        let lines = sqlx::query_as::<_, PurgatoryLineRow>(
            "SELECT * FROM purgatory_line_items WHERE purgatory_order_id = ANY($1) ORDER BY external_sku",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut by_parent: std::collections::HashMap<Uuid, Vec<PurgatoryLineItem>> =
            std::collections::HashMap::with_capacity(orders.len());
        for line in lines {
            by_parent.entry(line.purgatory_order_id).or_default().push(Self::row_to_line(line));
        }
        for order in &mut orders {
            order.items = by_parent.remove(&Uuid::from(order.id)).unwrap_or_default();
        }
        Ok(orders)
    }

    /// Map / flag a purgatory line (async)
    pub async fn map_line_async(
        &self,
        id: PurgatoryOrderId,
        line_id: PurgatoryLineItemId,
        input: MapPurgatoryLine,
    ) -> Result<PurgatoryOrder> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let mut sets: Vec<String> = vec![];
        let mut param_idx = 1;
        if input.product_id.is_some() {
            sets.push(format!("product_id = ${param_idx}"));
            param_idx += 1;
        }
        if input.ignore_item.is_some() {
            sets.push(format!("ignore_item = ${param_idx}"));
            param_idx += 1;
        }
        if input.non_physical.is_some() {
            sets.push(format!("non_physical = ${param_idx}"));
            param_idx += 1;
        }
        if !sets.is_empty() {
            let sql = format!(
                "UPDATE purgatory_line_items SET {} WHERE id = ${} AND purgatory_order_id = ${}",
                sets.join(", "),
                param_idx,
                param_idx + 1
            );
            let mut q = sqlx::query(&sql);
            if let Some(product_id) = input.product_id {
                q = q.bind(Uuid::from(product_id));
            }
            if let Some(ignore) = input.ignore_item {
                q = q.bind(ignore);
            }
            if let Some(non_physical) = input.non_physical {
                q = q.bind(non_physical);
            }
            q.bind(Uuid::from(line_id))
                .bind(Uuid::from(id))
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }
        sqlx::query("UPDATE purgatory_orders SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        self.fetch_full_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Post the order out of purgatory (async)
    pub async fn post_async(&self, id: PurgatoryOrderId) -> Result<PurgatoryOrder> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let order =
            Self::fetch_full_conn(tx.as_mut(), id.into()).await?.ok_or(CommerceError::NotFound)?;
        if order.is_posted {
            return Err(CommerceError::Conflict("order is already posted".into()));
        }
        if !order.is_ready_to_post() {
            return Err(CommerceError::ValidationError(format!(
                "{} line(s) still unresolved",
                order.unresolved_count()
            )));
        }
        sqlx::query(
            "UPDATE purgatory_orders SET is_posted = TRUE, hold_reason = NULL, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        self.fetch_full_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Delete a purgatory order and its lines (async)
    pub async fn delete_async(&self, id: PurgatoryOrderId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query("DELETE FROM purgatory_line_items WHERE purgatory_order_id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("DELETE FROM purgatory_orders WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }
}

impl PurgatoryRepository for PgPurgatoryRepository {
    fn ingest(&self, input: IngestOrder) -> Result<PurgatoryOrder> {
        super::block_on(self.ingest_async(input))
    }

    fn get(&self, id: PurgatoryOrderId) -> Result<Option<PurgatoryOrder>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: PurgatoryFilter) -> Result<Vec<PurgatoryOrder>> {
        super::block_on(self.list_async(filter))
    }

    fn map_line(
        &self,
        id: PurgatoryOrderId,
        line_id: PurgatoryLineItemId,
        input: MapPurgatoryLine,
    ) -> Result<PurgatoryOrder> {
        super::block_on(self.map_line_async(id, line_id, input))
    }

    fn post(&self, id: PurgatoryOrderId) -> Result<PurgatoryOrder> {
        super::block_on(self.post_async(id))
    }

    fn delete(&self, id: PurgatoryOrderId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }
}
