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

    async fn load_items_batch(
        &self,
        ids: &[TransferOrderId],
    ) -> Result<std::collections::HashMap<TransferOrderId, Vec<TransferOrderItem>>> {
        let mut map: std::collections::HashMap<TransferOrderId, Vec<TransferOrderItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let uuids: Vec<sqlx::types::Uuid> =
            ids.iter().map(|id| sqlx::types::Uuid::from(*id)).collect();
        let rows = sqlx::query_as::<_, ItemRow>(
            "SELECT * FROM transfer_order_items WHERE transfer_order_id = ANY($1) ORDER BY sku",
        )
        .bind(uuids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.transfer_order_id;
            map.entry(parent).or_default().push(Self::row_to_item(row));
        }
        Ok(map)
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
        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(i64::from(offset));
        }

        let rows = builder
            .build_query_as::<OrderRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;
        let mut out = Vec::with_capacity(rows.len());
        let mut heads = Vec::with_capacity(rows.len());
        for row in rows {
            heads.push(Self::row_to_head(row)?);
        }
        let ids: Vec<TransferOrderId> = heads.iter().map(|h| h.id).collect();
        let mut items_by_id = self.load_items_batch(&ids).await?;
        for mut head in heads {
            head.items = items_by_id.remove(&head.id).unwrap_or_default();
            out.push(head);
        }
        Ok(out)
    }

    /// Lock the transfer-order head row for the rest of the transaction and
    /// return its current status.
    ///
    /// Every write path that touches both the head and its lines takes this lock
    /// FIRST, so all of them acquire row locks in the same order (head, then
    /// lines) and cannot deadlock against each other. Returning the status under
    /// the lock is what lets callers decide on a value nobody can change
    /// underneath them.
    async fn lock_order(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: TransferOrderId,
    ) -> Result<TransferOrderStatus> {
        let locked: Option<(String,)> =
            sqlx::query_as("SELECT status FROM transfer_orders WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let (status,) = locked.ok_or(CommerceError::NotFound)?;
        status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid transfer_order.status '{status}': {e}"))
        })
    }

    /// Load a transfer order and its lines using the given transaction, so the
    /// read sees this transaction's own uncommitted writes.
    async fn load_full_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: TransferOrderId,
    ) -> Result<TransferOrder> {
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM transfer_orders WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let mut order = Self::row_to_head(row)?;
        let items = sqlx::query_as::<_, ItemRow>(
            "SELECT * FROM transfer_order_items WHERE transfer_order_id = $1 ORDER BY sku",
        )
        .bind(id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        order.items = items.into_iter().map(Self::row_to_item).collect();
        Ok(order)
    }

    /// Mark a transfer order as shipped from the source.
    pub async fn ship_async(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let _status = Self::lock_order(&mut tx, id).await?;
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
    ///
    /// The read of the line, the over-receipt check, the write and the derived
    /// order status all happen inside ONE transaction, with the order head and
    /// the line locked `FOR UPDATE`, and the write is an INCREMENT rather than an
    /// absolute quantity. Running these as separate autocommit statements let two
    /// clerks scanning receipts against the same line both read the same
    /// `quantity_received`, both pass the cap check and both write the same
    /// absolute total: a 100-unit line took in 200 units but recorded 100 and
    /// closed as `received`, and partial receipts overwrote each other instead of
    /// accumulating. Neither self-corrected, because the write was absolute.
    ///
    /// Locking the HEAD (not only the line) also serializes receipts against
    /// different lines of the same order, so the status derived from all lines
    /// cannot be computed from a stale snapshot that leaves a fully received
    /// order stuck in `partially_received`.
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

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let status = Self::lock_order(&mut tx, id).await?;
        if status == TransferOrderStatus::Cancelled {
            return Err(CommerceError::ValidationError(
                "Cannot receive against a cancelled transfer order".into(),
            ));
        }

        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "SELECT quantity, quantity_received FROM transfer_order_items WHERE id = $1 AND transfer_order_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(id)
        .fetch_optional(tx.as_mut())
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
        sqlx::query(
            "UPDATE transfer_order_items SET quantity_received = quantity_received + $1 WHERE id = $2",
        )
        .bind(quantity)
        .bind(item_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Recompute order status from line receipts, still under the same locks
        // and reading this transaction's own write.
        let order = Self::load_full_tx(&mut tx, id).await?;
        let derived = order.derive_receipt_status();
        let received_at = if derived == TransferOrderStatus::Received { Some(now) } else { None };
        sqlx::query(
            "UPDATE transfer_orders SET status = $1, received_at = COALESCE($2, received_at), updated_at = $3 WHERE id = $4",
        )
        .bind(derived.to_string())
        .bind(received_at)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.require_full(id).await
    }

    /// Cancel a transfer order.
    ///
    /// The terminal-state guard reads the status and the cancel writes it, so
    /// both happen in one transaction with the row locked `FOR UPDATE`. Split
    /// across two autocommit statements the guard decided on a status nobody
    /// held: concurrent cancels each saw a live order and each wrote, and a
    /// cancel could land on an order that became `received` after the check.
    pub async fn cancel_async(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let status = Self::lock_order(&mut tx, id).await?;
        if status.is_terminal() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel a transfer order in status {status}"
            )));
        }
        sqlx::query(
            "UPDATE transfer_orders SET status = 'cancelled', updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
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
