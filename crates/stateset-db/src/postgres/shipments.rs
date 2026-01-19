//! PostgreSQL implementation of shipment repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    validate_batch_size, AddShipmentEvent, BatchResult, CommerceError, CreateShipment,
    CreateShipmentItem, Result, Shipment, ShipmentEvent, ShipmentFilter, ShipmentItem,
    ShipmentRepository, ShipmentStatus, ShippingCarrier, ShippingMethod, UpdateShipment,
};
use uuid::Uuid;

/// PostgreSQL shipment repository
#[derive(Clone)]
pub struct PgShipmentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ShipmentRow {
    id: Uuid,
    shipment_number: String,
    order_id: Uuid,
    status: String,
    carrier: String,
    shipping_method: String,
    tracking_number: Option<String>,
    tracking_url: Option<String>,
    recipient_name: String,
    recipient_email: Option<String>,
    recipient_phone: Option<String>,
    shipping_address: String,
    weight_kg: Option<Decimal>,
    dimensions: Option<String>,
    shipping_cost: Option<Decimal>,
    insurance_amount: Option<Decimal>,
    signature_required: bool,
    shipped_at: Option<DateTime<Utc>>,
    estimated_delivery: Option<DateTime<Utc>>,
    delivered_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ShipmentItemRow {
    id: Uuid,
    shipment_id: Uuid,
    order_item_id: Option<Uuid>,
    product_id: Option<Uuid>,
    sku: String,
    name: String,
    quantity: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ShipmentEventRow {
    id: Uuid,
    shipment_id: Uuid,
    event_type: String,
    location: Option<String>,
    description: Option<String>,
    event_time: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl PgShipmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    fn row_to_shipment(
        row: ShipmentRow,
        items: Vec<ShipmentItem>,
        events: Vec<ShipmentEvent>,
    ) -> Result<Shipment> {
        let ShipmentRow {
            id,
            shipment_number,
            order_id,
            status,
            carrier,
            shipping_method,
            tracking_number,
            tracking_url,
            recipient_name,
            recipient_email,
            recipient_phone,
            shipping_address,
            weight_kg,
            dimensions,
            shipping_cost,
            insurance_amount,
            signature_required,
            shipped_at,
            estimated_delivery,
            delivered_at,
            notes,
            version,
            created_at,
            updated_at,
        } = row;

        let status: ShipmentStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid shipment.status '{}': {}",
                status.as_str(),
                e
            ))
        })?;
        let carrier: ShippingCarrier = carrier.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid shipment.carrier '{}': {}",
                carrier.as_str(),
                e
            ))
        })?;
        let shipping_method: ShippingMethod = shipping_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid shipment.shipping_method '{}': {}",
                shipping_method.as_str(),
                e
            ))
        })?;

        Ok(Shipment {
            id,
            shipment_number,
            order_id,
            status,
            carrier,
            shipping_method,
            tracking_number,
            tracking_url,
            recipient_name,
            recipient_email,
            recipient_phone,
            shipping_address,
            weight_kg,
            dimensions,
            shipping_cost,
            insurance_amount,
            signature_required,
            shipped_at,
            estimated_delivery,
            delivered_at,
            notes,
            items,
            events,
            version,
            created_at,
            updated_at,
        })
    }

    fn row_to_item(row: ShipmentItemRow) -> ShipmentItem {
        ShipmentItem {
            id: row.id,
            shipment_id: row.shipment_id,
            order_item_id: row.order_item_id,
            product_id: row.product_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_event(row: ShipmentEventRow) -> ShipmentEvent {
        ShipmentEvent {
            id: row.id,
            shipment_id: row.shipment_id,
            event_type: row.event_type,
            location: row.location,
            description: row.description,
            event_time: row.event_time,
            created_at: row.created_at,
        }
    }

    async fn load_items_async(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        let rows = sqlx::query_as::<_, ShipmentItemRow>(
            "SELECT id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at
             FROM shipment_items WHERE shipment_id = $1"
        )
        .bind(shipment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    async fn load_events_async(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        let rows = sqlx::query_as::<_, ShipmentEventRow>(
            "SELECT id, shipment_id, event_type, location, description, event_time, created_at
             FROM shipment_events WHERE shipment_id = $1 ORDER BY event_time DESC"
        )
        .bind(shipment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_event).collect())
    }

    async fn update_status_async(&self, id: Uuid, status: ShipmentStatus) -> Result<Shipment> {
        let now = Utc::now();

        sqlx::query("UPDATE shipments SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Create shipment (async)
    pub async fn create_async(&self, input: CreateShipment) -> Result<Shipment> {
        let id = Uuid::new_v4();
        let shipment_number = Shipment::generate_shipment_number();
        let now = Utc::now();
        let carrier = input.carrier.unwrap_or_default();
        let method = input.shipping_method.unwrap_or_default();
        let tracking_url = input
            .tracking_number
            .as_ref()
            .and_then(|tn| carrier.tracking_url(tn));

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO shipments (id, shipment_number, order_id, status, carrier, shipping_method,
             tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
             shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
             signature_required, estimated_delivery, notes, created_at, updated_at)
             VALUES ($1, $2, $3, 'pending', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)"
        )
        .bind(id)
        .bind(&shipment_number)
        .bind(input.order_id)
        .bind(carrier.to_string())
        .bind(method.to_string())
        .bind(&input.tracking_number)
        .bind(&tracking_url)
        .bind(&input.recipient_name)
        .bind(&input.recipient_email)
        .bind(&input.recipient_phone)
        .bind(&input.shipping_address)
        .bind(input.weight_kg)
        .bind(&input.dimensions)
        .bind(input.shipping_cost)
        .bind(input.insurance_amount)
        .bind(input.signature_required.unwrap_or(false))
        .bind(input.estimated_delivery)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let mut items = Vec::new();
        if let Some(item_inputs) = &input.items {
            for item_input in item_inputs {
                let item_id = Uuid::new_v4();

                sqlx::query(
                    "INSERT INTO shipment_items (id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
                )
                .bind(item_id)
                .bind(id)
                .bind(item_input.order_item_id)
                .bind(item_input.product_id)
                .bind(&item_input.sku)
                .bind(&item_input.name)
                .bind(item_input.quantity)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(map_db_error)?;

                items.push(ShipmentItem {
                    id: item_id,
                    shipment_id: id,
                    order_item_id: item_input.order_item_id,
                    product_id: item_input.product_id,
                    sku: item_input.sku.clone(),
                    name: item_input.name.clone(),
                    quantity: item_input.quantity,
                    created_at: now,
                    updated_at: now,
                });
            }
        }

        tx.commit().await.map_err(map_db_error)?;

        Ok(Shipment {
            id,
            shipment_number,
            order_id: input.order_id,
            status: ShipmentStatus::Pending,
            carrier,
            shipping_method: method,
            tracking_number: input.tracking_number,
            tracking_url,
            recipient_name: input.recipient_name,
            recipient_email: input.recipient_email,
            recipient_phone: input.recipient_phone,
            shipping_address: input.shipping_address,
            weight_kg: input.weight_kg,
            dimensions: input.dimensions,
            shipping_cost: input.shipping_cost,
            insurance_amount: input.insurance_amount,
            signature_required: input.signature_required.unwrap_or(false),
            shipped_at: None,
            estimated_delivery: input.estimated_delivery,
            delivered_at: None,
            notes: input.notes,
            items,
            events: vec![],
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get shipment by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Shipment>> {
        let row = sqlx::query_as::<_, ShipmentRow>(
            "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                    tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                    shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                    signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                    created_at, updated_at
             FROM shipments WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let items = self.load_items_async(row.id).await?;
                let events = self.load_events_async(row.id).await?;
                Ok(Some(Self::row_to_shipment(row, items, events)?))
            }
            None => Ok(None),
        }
    }

    /// Get shipment by number (async)
    pub async fn get_by_number_async(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        let id: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM shipments WHERE shipment_number = $1")
            .bind(shipment_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match id {
            Some((id,)) => self.get_async(id).await,
            None => Ok(None),
        }
    }

    /// Get shipment by tracking number (async)
    pub async fn get_by_tracking_async(&self, tracking_number: &str) -> Result<Option<Shipment>> {
        let id: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM shipments WHERE tracking_number = $1")
            .bind(tracking_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match id {
            Some((id,)) => self.get_async(id).await,
            None => Ok(None),
        }
    }

    /// Update shipment (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateShipment) -> Result<Shipment> {
        let existing = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_status = input.status.unwrap_or(existing.status);
        let new_carrier = input.carrier.unwrap_or(existing.carrier);
        let new_tracking = input.tracking_number.or(existing.tracking_number);
        let new_tracking_url = new_tracking
            .as_ref()
            .and_then(|tn| new_carrier.tracking_url(tn));
        let new_recipient_name = input.recipient_name.unwrap_or(existing.recipient_name);
        let new_recipient_email = input.recipient_email.or(existing.recipient_email);
        let new_recipient_phone = input.recipient_phone.or(existing.recipient_phone);
        let new_shipping_address = input.shipping_address.unwrap_or(existing.shipping_address);
        let new_weight = input.weight_kg.or(existing.weight_kg);
        let new_dimensions = input.dimensions.or(existing.dimensions);
        let new_shipping_cost = input.shipping_cost.or(existing.shipping_cost);
        let new_estimated_delivery = input.estimated_delivery.or(existing.estimated_delivery);
        let new_notes = input.notes.or(existing.notes);

        sqlx::query(
            "UPDATE shipments SET status = $1, carrier = $2, tracking_number = $3, tracking_url = $4,
             recipient_name = $5, recipient_email = $6, recipient_phone = $7, shipping_address = $8,
             weight_kg = $9, dimensions = $10, shipping_cost = $11, estimated_delivery = $12, notes = $13,
             updated_at = $14 WHERE id = $15"
        )
        .bind(new_status.to_string())
        .bind(new_carrier.to_string())
        .bind(&new_tracking)
        .bind(&new_tracking_url)
        .bind(&new_recipient_name)
        .bind(&new_recipient_email)
        .bind(&new_recipient_phone)
        .bind(&new_shipping_address)
        .bind(new_weight)
        .bind(&new_dimensions)
        .bind(new_shipping_cost)
        .bind(new_estimated_delivery)
        .bind(&new_notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List shipments (async)
    pub async fn list_async(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let mut query = String::from("SELECT id FROM shipments WHERE 1=1");
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.carrier.is_some() {
            query.push_str(&format!(" AND carrier = ${}", param_idx));
            param_idx += 1;
        }
        if filter.tracking_number.is_some() {
            query.push_str(&format!(" AND tracking_number = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

        let mut q = sqlx::query_as::<_, (Uuid,)>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(carrier) = filter.carrier {
            q = q.bind(carrier.to_string());
        }
        if let Some(tracking_number) = &filter.tracking_number {
            q = q.bind(tracking_number);
        }

        q = q.bind(limit).bind(offset);

        let ids = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut shipments = Vec::new();
        for (id,) in ids {
            if let Some(shipment) = self.get_async(id).await? {
                shipments.push(shipment);
            }
        }

        Ok(shipments)
    }

    /// Get shipments for order (async)
    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<Shipment>> {
        self.list_async(ShipmentFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
        .await
    }

    /// Delete shipment (async) - marks as cancelled
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE shipments SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Mark as processing (async)
    pub async fn mark_processing_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::Processing).await
    }

    /// Mark as ready to ship (async)
    pub async fn mark_ready_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::ReadyToShip).await
    }

    /// Ship the shipment (async)
    pub async fn ship_async(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment> {
        let existing = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let tracking_url = tracking_number
            .as_ref()
            .and_then(|tn| existing.carrier.tracking_url(tn));

        sqlx::query(
            "UPDATE shipments SET status = 'shipped', tracking_number = COALESCE($1, tracking_number),
             tracking_url = COALESCE($2, tracking_url), shipped_at = $3, updated_at = $4 WHERE id = $5"
        )
        .bind(&tracking_number)
        .bind(&tracking_url)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Mark as in transit (async)
    pub async fn mark_in_transit_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::InTransit).await
    }

    /// Mark as out for delivery (async)
    pub async fn mark_out_for_delivery_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::OutForDelivery).await
    }

    /// Mark as delivered (async)
    pub async fn mark_delivered_async(&self, id: Uuid) -> Result<Shipment> {
        let now = Utc::now();

        sqlx::query("UPDATE shipments SET status = 'delivered', delivered_at = $1, updated_at = $2 WHERE id = $3")
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Mark as failed (async)
    pub async fn mark_failed_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::Failed).await
    }

    /// Put on hold (async)
    pub async fn hold_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::OnHold).await
    }

    /// Cancel shipment (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<Shipment> {
        self.update_status_async(id, ShipmentStatus::Cancelled).await
    }

    /// Add item to shipment (async)
    pub async fn add_item_async(&self, shipment_id: Uuid, item: CreateShipmentItem) -> Result<ShipmentItem> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO shipment_items (id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
        )
        .bind(id)
        .bind(shipment_id)
        .bind(item.order_item_id)
        .bind(item.product_id)
        .bind(&item.sku)
        .bind(&item.name)
        .bind(item.quantity)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(ShipmentItem {
            id,
            shipment_id,
            order_item_id: item.order_item_id,
            product_id: item.product_id,
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            created_at: now,
            updated_at: now,
        })
    }

    /// Remove item from shipment (async)
    pub async fn remove_item_async(&self, item_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM shipment_items WHERE id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Get items for shipment (async)
    pub async fn get_items_async(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        self.load_items_async(shipment_id).await
    }

    /// Add tracking event (async)
    pub async fn add_event_async(&self, shipment_id: Uuid, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let event_time = event.event_time.unwrap_or(now);

        sqlx::query(
            "INSERT INTO shipment_events (id, shipment_id, event_type, location, description, event_time, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
        .bind(id)
        .bind(shipment_id)
        .bind(&event.event_type)
        .bind(&event.location)
        .bind(&event.description)
        .bind(event_time)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(ShipmentEvent {
            id,
            shipment_id,
            event_type: event.event_type,
            location: event.location,
            description: event.description,
            event_time,
            created_at: now,
        })
    }

    /// Get events for shipment (async)
    pub async fn get_events_async(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        self.load_events_async(shipment_id).await
    }

    /// Count shipments (async)
    pub async fn count_async(&self, filter: ShipmentFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM shipments WHERE 1=1");
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.carrier.is_some() {
            query.push_str(&format!(" AND carrier = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(carrier) = filter.carrier {
            q = q.bind(carrier.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count as u64)
    }

    // ==================== Batch Operations ====================

    /// Create multiple shipments in a batch (async, non-atomic)
    pub async fn create_batch_async(&self, inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(shipment) => result.record_success(shipment),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple shipments in a batch atomically (async)
    pub async fn create_batch_atomic_async(&self, inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut shipments = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let shipment_number = Shipment::generate_shipment_number();
            let now = Utc::now();
            let carrier = input.carrier.unwrap_or_default();
            let method = input.shipping_method.unwrap_or_default();
            let tracking_url = input
                .tracking_number
                .as_ref()
                .and_then(|tn| carrier.tracking_url(tn));

            sqlx::query(
                "INSERT INTO shipments (id, shipment_number, order_id, status, carrier, shipping_method,
                 tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                 shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                 signature_required, estimated_delivery, notes, created_at, updated_at)
                 VALUES ($1, $2, $3, 'pending', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)"
            )
            .bind(id)
            .bind(&shipment_number)
            .bind(input.order_id)
            .bind(carrier.to_string())
            .bind(method.to_string())
            .bind(&input.tracking_number)
            .bind(&tracking_url)
            .bind(&input.recipient_name)
            .bind(&input.recipient_email)
            .bind(&input.recipient_phone)
            .bind(&input.shipping_address)
            .bind(input.weight_kg)
            .bind(&input.dimensions)
            .bind(input.shipping_cost)
            .bind(input.insurance_amount)
            .bind(input.signature_required.unwrap_or(false))
            .bind(input.estimated_delivery)
            .bind(&input.notes)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

            let mut items = Vec::new();
            if let Some(item_inputs) = &input.items {
                for item_input in item_inputs {
                    let item_id = Uuid::new_v4();

                    sqlx::query(
                        "INSERT INTO shipment_items (id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
                    )
                    .bind(item_id)
                    .bind(id)
                    .bind(item_input.order_item_id)
                    .bind(item_input.product_id)
                    .bind(&item_input.sku)
                    .bind(&item_input.name)
                    .bind(item_input.quantity)
                    .bind(now)
                    .bind(now)
                    .execute(&mut *tx)
                    .await
                    .map_err(map_db_error)?;

                    items.push(ShipmentItem {
                        id: item_id,
                        shipment_id: id,
                        order_item_id: item_input.order_item_id,
                        product_id: item_input.product_id,
                        sku: item_input.sku.clone(),
                        name: item_input.name.clone(),
                        quantity: item_input.quantity,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }

            shipments.push(Shipment {
                id,
                shipment_number,
                order_id: input.order_id,
                status: ShipmentStatus::Pending,
                carrier,
                shipping_method: method,
                tracking_number: input.tracking_number,
                tracking_url,
                recipient_name: input.recipient_name,
                recipient_email: input.recipient_email,
                recipient_phone: input.recipient_phone,
                shipping_address: input.shipping_address,
                weight_kg: input.weight_kg,
                dimensions: input.dimensions,
                shipping_cost: input.shipping_cost,
                insurance_amount: input.insurance_amount,
                signature_required: input.signature_required.unwrap_or(false),
                shipped_at: None,
                estimated_delivery: input.estimated_delivery,
                delivered_at: None,
                notes: input.notes,
                items,
                events: vec![],
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(shipments)
    }

    /// Update multiple shipments in a batch (async, non-atomic)
    pub async fn update_batch_async(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<BatchResult<Shipment>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(shipment) => result.record_success(shipment),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple shipments in a batch atomically (async)
    pub async fn update_batch_atomic_async(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<Vec<Shipment>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut shipments = Vec::with_capacity(updates.len());
        let now = Utc::now();

        for (id, input) in updates {
            // Get existing shipment with lock
            let existing_row = sqlx::query_as::<_, ShipmentRow>(
                "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                        tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                        shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                        signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                        version, created_at, updated_at
                 FROM shipments WHERE id = $1 FOR UPDATE"
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

            let existing_status: ShipmentStatus = existing_row.status.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid shipment.status '{}': {}",
                    existing_row.status.as_str(),
                    e
                ))
            })?;
            let existing_carrier: ShippingCarrier = existing_row.carrier.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid shipment.carrier '{}': {}",
                    existing_row.carrier.as_str(),
                    e
                ))
            })?;

            let new_status = input.status.unwrap_or(existing_status);
            let new_carrier = input.carrier.unwrap_or(existing_carrier);
            let new_tracking = input.tracking_number.or(existing_row.tracking_number.clone());
            let new_tracking_url = new_tracking
                .as_ref()
                .and_then(|tn| new_carrier.tracking_url(tn));
            let new_recipient_name = input.recipient_name.unwrap_or(existing_row.recipient_name.clone());
            let new_recipient_email = input.recipient_email.or(existing_row.recipient_email.clone());
            let new_recipient_phone = input.recipient_phone.or(existing_row.recipient_phone.clone());
            let new_shipping_address = input.shipping_address.unwrap_or(existing_row.shipping_address.clone());
            let new_weight = input.weight_kg.or(existing_row.weight_kg);
            let new_dimensions = input.dimensions.or(existing_row.dimensions.clone());
            let new_shipping_cost = input.shipping_cost.or(existing_row.shipping_cost);
            let new_estimated_delivery = input.estimated_delivery.or(existing_row.estimated_delivery);
            let new_notes = input.notes.or(existing_row.notes.clone());

            sqlx::query(
                "UPDATE shipments SET status = $1, carrier = $2, tracking_number = $3, tracking_url = $4,
                 recipient_name = $5, recipient_email = $6, recipient_phone = $7, shipping_address = $8,
                 weight_kg = $9, dimensions = $10, shipping_cost = $11, estimated_delivery = $12, notes = $13,
                 updated_at = $14 WHERE id = $15"
            )
            .bind(new_status.to_string())
            .bind(new_carrier.to_string())
            .bind(&new_tracking)
            .bind(&new_tracking_url)
            .bind(&new_recipient_name)
            .bind(&new_recipient_email)
            .bind(&new_recipient_phone)
            .bind(&new_shipping_address)
            .bind(new_weight)
            .bind(&new_dimensions)
            .bind(new_shipping_cost)
            .bind(new_estimated_delivery)
            .bind(&new_notes)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

            // Fetch updated shipment row
            let updated_row = sqlx::query_as::<_, ShipmentRow>(
                "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                        tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                        shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                        signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                        version, created_at, updated_at
                 FROM shipments WHERE id = $1"
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;

            // Load items and events
            let item_rows = sqlx::query_as::<_, ShipmentItemRow>(
                "SELECT id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at
                 FROM shipment_items WHERE shipment_id = $1"
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .map_err(map_db_error)?;

            let event_rows = sqlx::query_as::<_, ShipmentEventRow>(
                "SELECT id, shipment_id, event_type, location, description, event_time, created_at
                 FROM shipment_events WHERE shipment_id = $1 ORDER BY event_time DESC"
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .map_err(map_db_error)?;

            let items: Vec<ShipmentItem> = item_rows.into_iter().map(Self::row_to_item).collect();
            let events: Vec<ShipmentEvent> = event_rows.into_iter().map(Self::row_to_event).collect();

            shipments.push(Self::row_to_shipment(updated_row, items, events)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(shipments)
    }

    /// Delete multiple shipments in a batch (async, non-atomic)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple shipments in a batch atomically (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete shipment events first (foreign key constraint)
        sqlx::query("DELETE FROM shipment_events WHERE shipment_id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        // Delete shipment items (foreign key constraint)
        sqlx::query("DELETE FROM shipment_items WHERE shipment_id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        // Mark shipments as cancelled (soft delete)
        sqlx::query("UPDATE shipments SET status = 'cancelled', updated_at = $1 WHERE id = ANY($2)")
            .bind(Utc::now())
            .bind(&ids)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple shipments by IDs (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Shipment>> {
        validate_batch_size(&ids)?;

        let rows = sqlx::query_as::<_, ShipmentRow>(
            "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                    tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                    shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                    signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                    version, created_at, updated_at
             FROM shipments WHERE id = ANY($1)"
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut shipments = Vec::with_capacity(rows.len());
        for row in rows {
            let items = self.load_items_async(row.id).await?;
            let events = self.load_events_async(row.id).await?;
            shipments.push(Self::row_to_shipment(row, items, events)?);
        }

        Ok(shipments)
    }

    // ==================== Sync Batch Wrappers ====================

    /// Create multiple shipments in a batch (sync, non-atomic)
    pub fn create_batch(&self, inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>> {
        super::block_on(self.create_batch_async(inputs))
    }

    /// Create multiple shipments in a batch atomically (sync)
    pub fn create_batch_atomic(&self, inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    /// Update multiple shipments in a batch (sync, non-atomic)
    pub fn update_batch(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<BatchResult<Shipment>> {
        super::block_on(self.update_batch_async(updates))
    }

    /// Update multiple shipments in a batch atomically (sync)
    pub fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<Vec<Shipment>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    /// Delete multiple shipments in a batch (sync, non-atomic)
    pub fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    /// Delete multiple shipments in a batch atomically (sync)
    pub fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    /// Get multiple shipments by IDs (sync)
    pub fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Shipment>> {
        super::block_on(self.get_batch_async(ids))
    }
}

impl ShipmentRepository for PgShipmentRepository {
    fn create(&self, input: CreateShipment) -> Result<Shipment> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Shipment>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        super::block_on(self.get_by_number_async(shipment_number))
    }

    fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>> {
        super::block_on(self.get_by_tracking_async(tracking_number))
    }

    fn update(&self, id: Uuid, input: UpdateShipment) -> Result<Shipment> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        super::block_on(self.list_async(filter))
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Shipment>> {
        super::block_on(self.for_order_async(order_id))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn mark_processing(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.mark_processing_async(id))
    }

    fn mark_ready(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.mark_ready_async(id))
    }

    fn ship(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment> {
        super::block_on(self.ship_async(id, tracking_number))
    }

    fn mark_in_transit(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.mark_in_transit_async(id))
    }

    fn mark_out_for_delivery(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.mark_out_for_delivery_async(id))
    }

    fn mark_delivered(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.mark_delivered_async(id))
    }

    fn mark_failed(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.mark_failed_async(id))
    }

    fn hold(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.hold_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<Shipment> {
        super::block_on(self.cancel_async(id))
    }

    fn add_item(&self, shipment_id: Uuid, item: CreateShipmentItem) -> Result<ShipmentItem> {
        super::block_on(self.add_item_async(shipment_id, item))
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        super::block_on(self.remove_item_async(item_id))
    }

    fn get_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        super::block_on(self.get_items_async(shipment_id))
    }

    fn add_event(&self, shipment_id: Uuid, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        super::block_on(self.add_event_async(shipment_id, event))
    }

    fn get_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        super::block_on(self.get_events_async(shipment_id))
    }

    fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    fn create_batch(&self, inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>> {
        self.create_batch(inputs)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>> {
        self.create_batch_atomic(inputs)
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<BatchResult<Shipment>> {
        self.update_batch(updates)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateShipment)>) -> Result<Vec<Shipment>> {
        self.update_batch_atomic(updates)
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        self.delete_batch(ids)
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        self.delete_batch_atomic(ids)
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Shipment>> {
        self.get_batch(ids)
    }
}
