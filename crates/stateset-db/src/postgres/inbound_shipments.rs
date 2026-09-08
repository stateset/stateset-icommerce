//! PostgreSQL implementation of the inbound shipment repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CommerceError, CreateInboundShipment, InboundShipment, InboundShipmentFilter,
    InboundShipmentId, InboundShipmentItem, InboundShipmentItemId, InboundShipmentRepository,
    InboundShipmentStatus, Result,
};
use uuid::Uuid;

/// PostgreSQL-backed [`InboundShipmentRepository`].
#[derive(Debug, Clone)]
pub struct PgInboundShipmentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ShipmentRow {
    id: InboundShipmentId,
    number: String,
    supplier_id: Uuid,
    purchase_order_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    carrier: Option<String>,
    tracking_number: Option<String>,
    status: String,
    expected_at: Option<DateTime<Utc>>,
    received_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ItemRow {
    id: InboundShipmentItemId,
    inbound_shipment_id: InboundShipmentId,
    product_id: Uuid,
    sku: String,
    quantity_expected: Decimal,
    quantity_received: Decimal,
}

impl PgInboundShipmentRepository {
    /// Create a new repository over the given pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_head(row: ShipmentRow) -> Result<InboundShipment> {
        let status: InboundShipmentStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inbound_shipment.status '{}': {}",
                row.status, e
            ))
        })?;
        Ok(InboundShipment {
            id: row.id,
            number: row.number,
            supplier_id: row.supplier_id,
            purchase_order_id: row.purchase_order_id,
            warehouse_id: row.warehouse_id.map(Into::into),
            carrier: row.carrier,
            tracking_number: row.tracking_number,
            status,
            items: Vec::new(),
            expected_at: row.expected_at,
            received_at: row.received_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_item(row: ItemRow) -> InboundShipmentItem {
        InboundShipmentItem {
            id: row.id,
            inbound_shipment_id: row.inbound_shipment_id,
            product_id: row.product_id.into(),
            sku: row.sku,
            quantity_expected: row.quantity_expected,
            quantity_received: row.quantity_received,
        }
    }

    async fn load_items(&self, id: InboundShipmentId) -> Result<Vec<InboundShipmentItem>> {
        let rows = sqlx::query_as::<_, ItemRow>(
            "SELECT * FROM inbound_shipment_items WHERE inbound_shipment_id = $1 ORDER BY sku",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    async fn load_items_batch(
        &self,
        ids: &[InboundShipmentId],
    ) -> Result<std::collections::HashMap<InboundShipmentId, Vec<InboundShipmentItem>>> {
        let mut map: std::collections::HashMap<InboundShipmentId, Vec<InboundShipmentItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let uuids: Vec<Uuid> = ids.iter().map(|id| Uuid::from(*id)).collect();
        let rows = sqlx::query_as::<_, ItemRow>(
            "SELECT * FROM inbound_shipment_items WHERE inbound_shipment_id = ANY($1) ORDER BY sku",
        )
        .bind(uuids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.inbound_shipment_id;
            map.entry(parent).or_default().push(Self::row_to_item(row));
        }
        Ok(map)
    }

    async fn load_full(&self, id: InboundShipmentId) -> Result<Option<InboundShipment>> {
        let row = sqlx::query_as::<_, ShipmentRow>("SELECT * FROM inbound_shipments WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut head = Self::row_to_head(row)?;
        head.items = self.load_items(id).await?;
        Ok(Some(head))
    }

    async fn require_full(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.load_full(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Lock the inbound-shipment head row for the rest of the transaction and
    /// return its current status.
    ///
    /// Every write path that touches both the head and its lines takes this
    /// lock FIRST, so all of them acquire row locks in the same order (head,
    /// then lines) and cannot deadlock against each other. Returning the status
    /// under the lock is what lets callers decide on a value nobody can change
    /// underneath them.
    async fn lock_shipment(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: InboundShipmentId,
    ) -> Result<InboundShipmentStatus> {
        let locked: Option<(String,)> =
            sqlx::query_as("SELECT status FROM inbound_shipments WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let (status,) = locked.ok_or(CommerceError::NotFound)?;
        status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid inbound_shipment.status '{status}': {e}"))
        })
    }

    /// Load a shipment and its lines through the given transaction, so the read
    /// sees this transaction's own uncommitted writes.
    async fn load_full_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        id: InboundShipmentId,
    ) -> Result<InboundShipment> {
        let row = sqlx::query_as::<_, ShipmentRow>("SELECT * FROM inbound_shipments WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let mut head = Self::row_to_head(row)?;
        let items = sqlx::query_as::<_, ItemRow>(
            "SELECT * FROM inbound_shipment_items WHERE inbound_shipment_id = $1 ORDER BY sku",
        )
        .bind(id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        head.items = items.into_iter().map(Self::row_to_item).collect();
        Ok(head)
    }

    async fn set_status(
        &self,
        id: InboundShipmentId,
        status: InboundShipmentStatus,
    ) -> Result<InboundShipment> {
        sqlx::query("UPDATE inbound_shipments SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        self.require_full(id).await
    }

    /// Create a new inbound shipment.
    pub async fn create_async(&self, input: CreateInboundShipment) -> Result<InboundShipment> {
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "an inbound shipment requires at least one item".into(),
            ));
        }
        let id = InboundShipmentId::new();
        let now = Utc::now();
        let number = format!("ASN-{}", &id.to_string()[..8]);

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO inbound_shipments (id, number, supplier_id, purchase_order_id, warehouse_id, carrier, tracking_number, status, expected_at, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8, $9, $10, $10)",
        )
        .bind(id)
        .bind(&number)
        .bind(input.supplier_id)
        .bind(input.purchase_order_id)
        .bind(input.warehouse_id.map(|w| *w.as_uuid()))
        .bind(&input.carrier)
        .bind(&input.tracking_number)
        .bind(input.expected_at)
        .bind(&input.notes)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for item in &input.items {
            sqlx::query(
                "INSERT INTO inbound_shipment_items (id, inbound_shipment_id, product_id, sku, quantity_expected, quantity_received)
                 VALUES ($1, $2, $3, $4, $5, 0)",
            )
            .bind(InboundShipmentItemId::new())
            .bind(id)
            .bind(*item.product_id.as_uuid())
            .bind(&item.sku)
            .bind(item.quantity_expected)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.require_full(id).await
    }

    /// Get an inbound shipment by ID (with line items).
    pub async fn get_async(&self, id: InboundShipmentId) -> Result<Option<InboundShipment>> {
        self.load_full(id).await
    }

    /// List inbound shipments with filter.
    pub async fn list_async(&self, filter: InboundShipmentFilter) -> Result<Vec<InboundShipment>> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM inbound_shipments WHERE 1=1");
        if let Some(supplier) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier);
        }
        if let Some(warehouse) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(*warehouse.as_uuid());
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        builder.push(" ORDER BY created_at DESC");
        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(i64::from(offset));
        }

        let rows = builder
            .build_query_as::<ShipmentRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;
        let mut out = Vec::with_capacity(rows.len());
        let mut heads = Vec::with_capacity(rows.len());
        for row in rows {
            heads.push(Self::row_to_head(row)?);
        }
        let ids: Vec<InboundShipmentId> = heads.iter().map(|h| h.id).collect();
        let mut items_by_id = self.load_items_batch(&ids).await?;
        for mut head in heads {
            head.items = items_by_id.remove(&head.id).unwrap_or_default();
            out.push(head);
        }
        Ok(out)
    }

    /// Mark a shipment as in transit.
    pub async fn mark_in_transit_async(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::InTransit).await
    }

    /// Mark a shipment as arrived at the warehouse.
    pub async fn mark_arrived_async(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::Arrived).await
    }

    /// Receive a quantity against a single line, advancing the shipment status.
    ///
    /// The read of the line, the over-receipt check, the write and the derived
    /// head status all happen inside ONE transaction, with the shipment head and
    /// the line locked `FOR UPDATE`, and the write is an INCREMENT rather than
    /// an absolute quantity. Run as separate autocommit statements, two
    /// receivers scanning the same ASN line both read the same
    /// `quantity_received`, both passed the cap check and both wrote the same
    /// absolute total: the dock took in twice the units and recorded one lot of
    /// them, and concurrent partial receipts overwrote each other instead of
    /// accumulating. Neither self-corrected, because the write was absolute.
    ///
    /// Locking the HEAD (not only the line) also serializes receipts against
    /// different lines of the same shipment, so the status derived from all
    /// lines cannot be computed from a stale snapshot that leaves a fully
    /// received shipment stuck in `partially_received` — and it is what lets the
    /// cancelled-status check below mean something.
    pub async fn receive_line_async(
        &self,
        id: InboundShipmentId,
        item_id: InboundShipmentItemId,
        quantity: Decimal,
    ) -> Result<InboundShipment> {
        if quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("receive quantity must be positive".into()));
        }
        let now = Utc::now();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let status = Self::lock_shipment(&mut tx, id).await?;
        if status == InboundShipmentStatus::Cancelled {
            return Err(CommerceError::ValidationError(
                "Cannot receive against a cancelled inbound shipment".into(),
            ));
        }

        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "SELECT quantity_expected, quantity_received FROM inbound_shipment_items WHERE id = $1 AND inbound_shipment_id = $2 FOR UPDATE",
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
            "UPDATE inbound_shipment_items SET quantity_received = quantity_received + $1 WHERE id = $2",
        )
        .bind(quantity)
        .bind(item_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Recompute the head status from line receipts, still under the same
        // locks and reading this transaction's own write.
        let shipment = Self::load_full_tx(&mut tx, id).await?;
        let derived = shipment.derive_receipt_status();
        let received_at = if derived == InboundShipmentStatus::Received { Some(now) } else { None };
        sqlx::query(
            "UPDATE inbound_shipments SET status = $1, received_at = COALESCE($2, received_at), updated_at = $3 WHERE id = $4",
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

    /// Cancel an inbound shipment.
    ///
    /// The terminal-state guard reads the status and the cancel writes it, so
    /// both happen in one transaction with the head row locked `FOR UPDATE`.
    /// Split across two autocommit statements the guard decided on a status
    /// nobody held: concurrent cancels each saw a live shipment and each wrote,
    /// and a cancel could land on a shipment that became `received` after the
    /// check — leaving a cancelled ASN holding received stock.
    pub async fn cancel_async(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let status = Self::lock_shipment(&mut tx, id).await?;
        if status.is_terminal() {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel an inbound shipment in status {status}"
            )));
        }
        sqlx::query(
            "UPDATE inbound_shipments SET status = 'cancelled', updated_at = $1 WHERE id = $2",
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

impl InboundShipmentRepository for PgInboundShipmentRepository {
    fn create(&self, input: CreateInboundShipment) -> Result<InboundShipment> {
        block_on(self.create_async(input))
    }

    fn get(&self, id: InboundShipmentId) -> Result<Option<InboundShipment>> {
        block_on(self.get_async(id))
    }

    fn list(&self, filter: InboundShipmentFilter) -> Result<Vec<InboundShipment>> {
        block_on(self.list_async(filter))
    }

    fn mark_in_transit(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        block_on(self.mark_in_transit_async(id))
    }

    fn mark_arrived(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        block_on(self.mark_arrived_async(id))
    }

    fn receive_line(
        &self,
        id: InboundShipmentId,
        item_id: InboundShipmentItemId,
        quantity: Decimal,
    ) -> Result<InboundShipment> {
        block_on(self.receive_line_async(id, item_id, quantity))
    }

    fn cancel(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        block_on(self.cancel_async(id))
    }
}
