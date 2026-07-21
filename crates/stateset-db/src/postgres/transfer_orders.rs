//! PostgreSQL implementation of the transfer order repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CommerceError, CreateTransferOrder, Result, TransferOrder, TransferOrderFilter,
    TransferOrderId, TransferOrderItem, TransferOrderItemId, TransferOrderRepository,
    TransferOrderStatus,
};

/// PostgreSQL-backed [`TransferOrderRepository`].
#[derive(Debug, Clone)]
pub struct PgTransferOrderRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct OrderRow {
    id: TransferOrderId,
    number: String,
    source_warehouse_id: uuid::Uuid,
    destination_warehouse_id: uuid::Uuid,
    status: String,
    expected_at: Option<DateTime<Utc>>,
    shipped_at: Option<DateTime<Utc>>,
    received_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ItemRow {
    id: TransferOrderItemId,
    transfer_order_id: TransferOrderId,
    product_id: uuid::Uuid,
    sku: String,
    quantity: Decimal,
    quantity_shipped: Decimal,
    quantity_received: Decimal,
}

impl PgTransferOrderRepository {
    /// Create a new repository over the given pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_head(row: OrderRow) -> Result<TransferOrder> {
        let status: TransferOrderStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid transfer_order.status '{}': {}",
                row.status, e
            ))
        })?;
        Ok(TransferOrder {
            id: row.id,
            number: row.number,
            source_warehouse_id: row.source_warehouse_id.into(),
            destination_warehouse_id: row.destination_warehouse_id.into(),
            status,
            items: Vec::new(),
            expected_at: row.expected_at,
            shipped_at: row.shipped_at,
            received_at: row.received_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_item(row: ItemRow) -> TransferOrderItem {
        TransferOrderItem {
            id: row.id,
            transfer_order_id: row.transfer_order_id,
            product_id: row.product_id.into(),
            sku: row.sku,
            quantity: row.quantity,
            quantity_shipped: row.quantity_shipped,
            quantity_received: row.quantity_received,
        }
    }

    async fn load_items(&self, id: TransferOrderId) -> Result<Vec<TransferOrderItem>> {
        let rows = sqlx::query_as::<_, ItemRow>(
            "SELECT * FROM transfer_order_items WHERE transfer_order_id = $1 ORDER BY sku",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    async fn load_full(&self, id: TransferOrderId) -> Result<Option<TransferOrder>> {
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM transfer_orders WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut order = Self::row_to_head(row)?;
        order.items = self.load_items(id).await?;
        Ok(Some(order))
    }

    async fn require_full(&self, id: TransferOrderId) -> Result<TransferOrder> {
        self.load_full(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Create a new transfer order.
    pub async fn create_async(&self, input: CreateTransferOrder) -> Result<TransferOrder> {
        if input.source_warehouse_id == input.destination_warehouse_id {
            return Err(CommerceError::ValidationError(
                "source and destination warehouses must differ".into(),
            ));
        }
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "a transfer order requires at least one item".into(),
            ));
        }
        let id = TransferOrderId::new();
        let now = Utc::now();
        // Human-readable number derived from a short id fragment.
        let number = format!("TO-{}", &id.to_string()[..8]);

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO transfer_orders (id, number, source_warehouse_id, destination_warehouse_id, status, expected_at, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7, $7)",
        )
        .bind(id)
        .bind(&number)
        .bind(*input.source_warehouse_id.as_uuid())
        .bind(*input.destination_warehouse_id.as_uuid())
        .bind(input.expected_at)
        .bind(&input.notes)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for item in &input.items {
            sqlx::query(
                "INSERT INTO transfer_order_items (id, transfer_order_id, product_id, sku, quantity, quantity_shipped, quantity_received)
                 VALUES ($1, $2, $3, '', $4, 0, 0)",
            )
            .bind(TransferOrderItemId::new())
            .bind(id)
            .bind(*item.product_id.as_uuid())
            .bind(item.quantity)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.require_full(id).await
    }

    /// Get a transfer order by ID (with line items).
    pub async fn get_async(&self, id: TransferOrderId) -> Result<Option<TransferOrder>> {
        self.load_full(id).await
    }

    /// List transfer orders with filter.
    pub async fn list_async(&self, filter: TransferOrderFilter) -> Result<Vec<TransferOrder>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM transfer_orders WHERE 1=1");
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(src) = filter.source_warehouse_id {
            builder.push(" AND source_warehouse_id = ").push_bind(*src.as_uuid());
        }
        if let Some(dest) = filter.destination_warehouse_id {
            builder.push(" AND destination_warehouse_id = ").push_bind(*dest.as_uuid());
        }
        builder.push(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(i64::from(limit));
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(i64::from(offset));
        }

        let rows = builder
            .build_query_as::<OrderRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let mut head = Self::row_to_head(row)?;
            head.items = self.load_items(head.id).await?;
            out.push(head);
        }
        Ok(out)
    }

    /// Mark a transfer order as shipped from the source.
    pub async fn ship_async(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        // Shipping sets quantity_shipped = quantity on each line.
        sqlx::query(
            "UPDATE transfer_order_items SET quantity_shipped = quantity WHERE transfer_order_id = $1",
        )
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        sqlx::query(
            "UPDATE transfer_orders SET status = 'in_transit', shipped_at = $1, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.require_full(id).await
    }

    /// Receive quantities at the destination for a single line.
    pub async fn receive_line_async(
        &self,
        id: TransferOrderId,
        item_id: TransferOrderItemId,
        quantity: Decimal,
    ) -> Result<TransferOrder> {
        if quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("receive quantity must be positive".into()));
        }
        let now = Utc::now();

        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "SELECT quantity, quantity_received FROM transfer_order_items WHERE id = $1 AND transfer_order_id = $2",
        )
        .bind(item_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        let Some((expected, current)) = row else {
            return Err(CommerceError::NotFound);
        };
        let new_received = current + quantity;
        if new_received > expected {
            return Err(CommerceError::ValidationError(format!(
                "receiving {quantity} would exceed the {expected} expected on this line ({current} already received)"
            )));
        }
        sqlx::query("UPDATE transfer_order_items SET quantity_received = $1 WHERE id = $2")
            .bind(new_received)
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        // Recompute order status from line receipts.
        let order = self.require_full(id).await?;
        let derived = order.derive_receipt_status();
        let received_at = if derived == TransferOrderStatus::Received { Some(now) } else { None };
        sqlx::query(
            "UPDATE transfer_orders SET status = $1, received_at = COALESCE($2, received_at), updated_at = $3 WHERE id = $4",
        )
        .bind(derived.to_string())
        .bind(received_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.require_full(id).await
    }

    /// Cancel a transfer order.
    pub async fn cancel_async(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let current = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        if matches!(current.status, TransferOrderStatus::Received | TransferOrderStatus::Cancelled)
        {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel a transfer order in status {}",
                current.status
            )));
        }
        sqlx::query(
            "UPDATE transfer_orders SET status = 'cancelled', updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        self.require_full(id).await
    }
}

impl TransferOrderRepository for PgTransferOrderRepository {
    fn create(&self, input: CreateTransferOrder) -> Result<TransferOrder> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: TransferOrderId) -> Result<Option<TransferOrder>> {
        block_on(self.get_async(id))
    }

    fn list(&self, filter: TransferOrderFilter) -> Result<Vec<TransferOrder>> {
        block_on(self.list_async(filter))
    }

    fn ship(&self, id: TransferOrderId) -> Result<TransferOrder> {
        block_on(self.ship_async(id))
    }

    fn receive_line(
        &self,
        id: TransferOrderId,
        item_id: TransferOrderItemId,
        quantity: Decimal,
    ) -> Result<TransferOrder> {
        block_on(self.receive_line_async(id, item_id, quantity))
    }

    fn cancel(&self, id: TransferOrderId) -> Result<TransferOrder> {
        block_on(self.cancel_async(id))
    }
}
