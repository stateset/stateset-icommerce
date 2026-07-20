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
        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(i64::from(limit));
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(i64::from(offset));
        }

        let rows = builder
            .build_query_as::<ShipmentRow>()
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

    /// Mark a shipment as in transit.
    pub async fn mark_in_transit_async(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::InTransit).await
    }

    /// Mark a shipment as arrived at the warehouse.
    pub async fn mark_arrived_async(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::Arrived).await
    }

    /// Receive a quantity against a single line, advancing the shipment status.
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

        let row: Option<(Decimal, Decimal)> = sqlx::query_as(
            "SELECT quantity_expected, quantity_received FROM inbound_shipment_items WHERE id = $1 AND inbound_shipment_id = $2",
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
                "receiving {quantity} would exceed the {expected} expected on this line"
            )));
        }
        sqlx::query("UPDATE inbound_shipment_items SET quantity_received = $1 WHERE id = $2")
            .bind(new_received)
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        let shipment = self.require_full(id).await?;
        let derived = shipment.derive_receipt_status();
        let received_at = if derived == InboundShipmentStatus::Received { Some(now) } else { None };
        sqlx::query(
            "UPDATE inbound_shipments SET status = $1, received_at = COALESCE($2, received_at), updated_at = $3 WHERE id = $4",
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

    /// Cancel an inbound shipment.
    pub async fn cancel_async(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::Cancelled).await
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
