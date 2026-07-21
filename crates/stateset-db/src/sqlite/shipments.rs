//! SQLite Shipment repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime, parse_datetime_opt,
    parse_datetime_row, parse_decimal_opt, parse_enum, parse_uuid, parse_uuid_row, uuid_params,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AddShipmentEvent, BatchResult, CommerceError, CreateShipment, CreateShipmentItem, OrderId,
    ProductId, Result, Shipment, ShipmentEvent, ShipmentFilter, ShipmentId, ShipmentItem,
    ShipmentRepository, ShipmentStatus, ShippingCarrier, UpdateShipment, validate_batch_size,
};
use uuid::Uuid;

/// SQLite implementation of `ShipmentRepository`
#[derive(Debug)]
pub struct SqliteShipmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteShipmentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn load_items(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentItem>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at
                 FROM shipment_items WHERE shipment_id = ?",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([shipment_id.to_string()], |row| {
                Ok(ShipmentItem {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "shipment_item", "id")?,
                    shipment_id: ShipmentId::from(parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "shipment_item",
                        "shipment_id",
                    )?),
                    order_item_id: row
                        .get::<_, Option<String>>(2)?
                        .map(|s| parse_uuid_row(&s, "shipment_item", "order_item_id"))
                        .transpose()?,
                    product_id: row
                        .get::<_, Option<String>>(3)?
                        .map(|s| parse_uuid_row(&s, "shipment_item", "product_id"))
                        .transpose()?
                        .map(ProductId::from),
                    sku: row.get(4)?,
                    name: row.get(5)?,
                    quantity: row.get(6)?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(7)?,
                        "shipment_item",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(8)?,
                        "shipment_item",
                        "updated_at",
                    )?,
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(items)
    }

    fn load_events(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentEvent>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, shipment_id, event_type, location, description, event_time, created_at
                 FROM shipment_events WHERE shipment_id = ? ORDER BY event_time DESC",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([shipment_id.to_string()], |row| {
                Ok(ShipmentEvent {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "shipment_event", "id")?,
                    shipment_id: ShipmentId::from(parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "shipment_event",
                        "shipment_id",
                    )?),
                    event_type: row.get(2)?,
                    location: row.get(3)?,
                    description: row.get(4)?,
                    event_time: parse_datetime_row(
                        &row.get::<_, String>(5)?,
                        "shipment_event",
                        "event_time",
                    )?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(6)?,
                        "shipment_event",
                        "created_at",
                    )?,
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(events)
    }

    fn update_status(&self, id: ShipmentId, status: ShipmentStatus) -> Result<Shipment> {
        let now = Utc::now();

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE shipments SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.to_string(), now.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }
}

impl ShipmentRepository for SqliteShipmentRepository {
    fn create(&self, input: CreateShipment) -> Result<Shipment> {
        let id = Uuid::new_v4();
        let shipment_number = Shipment::generate_shipment_number();
        let now = Utc::now();
        let carrier = input.carrier.unwrap_or_default();
        let method = input.shipping_method.unwrap_or_default();
        let tracking_url = input.tracking_number.as_ref().and_then(|tn| carrier.tracking_url(tn));

        let mut items = Vec::new();
        {
            let mut conn =
                self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = super::begin_immediate(&mut conn)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            tx.execute(
                "INSERT INTO shipments (id, shipment_number, order_id, status, carrier, shipping_method,
                 tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                 shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                 signature_required, estimated_delivery, notes, created_at, updated_at)
                 VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    shipment_number,
                    input.order_id.to_string(),
                    carrier.to_string(),
                    method.to_string(),
                    input.tracking_number,
                    tracking_url,
                    input.recipient_name,
                    input.recipient_email,
                    input.recipient_phone,
                    input.shipping_address,
                    input.weight_kg.map(|w| w.to_string()),
                    input.dimensions,
                    input.shipping_cost.map(|c| c.to_string()),
                    input.insurance_amount.map(|a| a.to_string()),
                    i32::from(input.signature_required.unwrap_or(false)),
                    input.estimated_delivery.map(|dt| dt.to_rfc3339()),
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            if let Some(item_inputs) = &input.items {
                for item_input in item_inputs {
                    let item_id = Uuid::new_v4();

                    tx.execute(
                        "INSERT INTO shipment_items (id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![
                            item_id.to_string(),
                            id.to_string(),
                            item_input.order_item_id.map(|u| u.to_string()),
                            item_input.product_id.map(|u| u.to_string()),
                            item_input.sku,
                            item_input.name,
                            item_input.quantity,
                            now.to_rfc3339(),
                            now.to_rfc3339(),
                        ],
                    )
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

                    items.push(ShipmentItem {
                        id: item_id,
                        shipment_id: ShipmentId::from(id),
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

            tx.commit().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        Ok(Shipment {
            id: ShipmentId::from(id),
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

    fn get(&self, id: ShipmentId) -> Result<Option<Shipment>> {
        let shipment_data = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                        tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                        shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                        signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                        created_at, updated_at
                 FROM shipments WHERE id = ?",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, Option<String>>(14)?,
                        row.get::<_, Option<String>>(15)?,
                        row.get::<_, i32>(16)?,
                        row.get::<_, Option<String>>(17)?,
                        row.get::<_, Option<String>>(18)?,
                        row.get::<_, Option<String>>(19)?,
                        row.get::<_, Option<String>>(20)?,
                        row.get::<_, String>(21)?,
                        row.get::<_, String>(22)?,
                    ))
                },
            );

            match result {
                Ok(data) => Some(data),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        };

        match shipment_data {
            Some((
                id_str,
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
                created_at,
                updated_at,
            )) => {
                let shipment_id = ShipmentId::from(parse_uuid(&id_str, "shipment", "id")?);
                let items = self.load_items(shipment_id)?;
                let events = self.load_events(shipment_id)?;

                Ok(Some(Shipment {
                    id: shipment_id,
                    shipment_number,
                    order_id: OrderId::from(parse_uuid(&order_id, "shipment", "order_id")?),
                    status: parse_enum(&status, "shipment", "status")?,
                    carrier: parse_enum(&carrier, "shipment", "carrier")?,
                    shipping_method: parse_enum(&shipping_method, "shipment", "shipping_method")?,
                    tracking_number,
                    tracking_url,
                    recipient_name,
                    recipient_email,
                    recipient_phone,
                    shipping_address,
                    weight_kg: parse_decimal_opt(weight_kg, "shipment", "weight_kg")?,
                    dimensions,
                    shipping_cost: parse_decimal_opt(shipping_cost, "shipment", "shipping_cost")?,
                    insurance_amount: parse_decimal_opt(
                        insurance_amount,
                        "shipment",
                        "insurance_amount",
                    )?,
                    signature_required: signature_required != 0,
                    shipped_at: parse_datetime_opt(shipped_at, "shipment", "shipped_at")?,
                    estimated_delivery: parse_datetime_opt(
                        estimated_delivery,
                        "shipment",
                        "estimated_delivery",
                    )?,
                    delivered_at: parse_datetime_opt(delivered_at, "shipment", "delivered_at")?,
                    notes,
                    items,
                    events,
                    version: 1, // Default to 1 for backwards compatibility
                    created_at: parse_datetime(&created_at, "shipment", "created_at")?,
                    updated_at: parse_datetime(&updated_at, "shipment", "updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        let id_result = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM shipments WHERE shipment_number = ?",
                [shipment_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(ShipmentId::from(parse_uuid(&id_str, "shipment", "id")?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        };

        match id_result {
            Some(id) => self.get(id),
            None => Ok(None),
        }
    }

    fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>> {
        let id_result = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM shipments WHERE tracking_number = ?",
                [tracking_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(ShipmentId::from(parse_uuid(&id_str, "shipment", "id")?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        };

        match id_result {
            Some(id) => self.get(id),
            None => Ok(None),
        }
    }

    fn update(&self, id: ShipmentId, input: UpdateShipment) -> Result<Shipment> {
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_status = input.status.unwrap_or(existing.status);
        let new_carrier = input.carrier.unwrap_or(existing.carrier);
        let new_tracking = input.tracking_number.or(existing.tracking_number);
        let new_tracking_url = new_tracking.as_ref().and_then(|tn| new_carrier.tracking_url(tn));
        let new_recipient_name = input.recipient_name.unwrap_or(existing.recipient_name);
        let new_recipient_email = input.recipient_email.or(existing.recipient_email);
        let new_recipient_phone = input.recipient_phone.or(existing.recipient_phone);
        let new_shipping_address = input.shipping_address.unwrap_or(existing.shipping_address);
        let new_weight = input.weight_kg.or(existing.weight_kg);
        let new_dimensions = input.dimensions.or(existing.dimensions);
        let new_shipping_cost = input.shipping_cost.or(existing.shipping_cost);
        let new_estimated_delivery = input.estimated_delivery.or(existing.estimated_delivery);
        let new_notes = input.notes.or(existing.notes);

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE shipments SET status = ?, carrier = ?, tracking_number = ?, tracking_url = ?,
                 recipient_name = ?, recipient_email = ?, recipient_phone = ?, shipping_address = ?,
                 weight_kg = ?, dimensions = ?, shipping_cost = ?, estimated_delivery = ?, notes = ?,
                 updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_status.to_string(),
                    new_carrier.to_string(),
                    new_tracking,
                    new_tracking_url,
                    new_recipient_name,
                    new_recipient_email,
                    new_recipient_phone,
                    new_shipping_address,
                    new_weight.map(|w| w.to_string()),
                    new_dimensions,
                    new_shipping_cost.map(|c| c.to_string()),
                    new_estimated_delivery.map(|dt| dt.to_rfc3339()),
                    new_notes,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        let ids = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let limit = i64::from(filter.limit.unwrap_or(100));
            let offset = i64::from(filter.offset.unwrap_or(0));

            let mut sql = "SELECT id FROM shipments WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(order_id) = filter.order_id {
                sql.push_str(" AND order_id = ?");
                params.push(Box::new(order_id.to_string()));
            }

            if let Some(status) = filter.status {
                sql.push_str(" AND status = ?");
                params.push(Box::new(status.to_string()));
            }

            if let Some(carrier) = filter.carrier {
                sql.push_str(" AND carrier = ?");
                params.push(Box::new(carrier.to_string()));
            }

            if let Some(tracking_number) = filter.tracking_number {
                sql.push_str(" AND tracking_number = ?");
                params.push(Box::new(tracking_number));
            }

            sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
            params.push(Box::new(limit));
            params.push(Box::new(offset));

            let mut stmt =
                conn.prepare(&sql).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let mut id_list = Vec::new();
            for row in rows {
                let id_str = row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                id_list.push(ShipmentId::from(parse_uuid(&id_str, "shipment", "id")?));
            }
            id_list
        };

        let mut shipments = Vec::new();
        for id in ids {
            if let Some(shipment) = self.get(id)? {
                shipments.push(shipment);
            }
        }

        Ok(shipments)
    }

    fn for_order(&self, order_id: OrderId) -> Result<Vec<Shipment>> {
        self.list(ShipmentFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn delete(&self, id: ShipmentId) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE shipments SET status = 'cancelled', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn mark_processing(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Processing)
    }

    fn mark_ready(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::ReadyToShip)
    }

    fn ship(&self, id: ShipmentId, tracking_number: Option<String>) -> Result<Shipment> {
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let tracking_url =
            tracking_number.as_ref().and_then(|tn| existing.carrier.tracking_url(tn));

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE shipments SET status = 'shipped', tracking_number = COALESCE(?, tracking_number),
                 tracking_url = COALESCE(?, tracking_url), shipped_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    tracking_number,
                    tracking_url,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_in_transit(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::InTransit)
    }

    fn mark_out_for_delivery(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::OutForDelivery)
    }

    fn mark_delivered(&self, id: ShipmentId) -> Result<Shipment> {
        let now = Utc::now();

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE shipments SET status = 'delivered', delivered_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_failed(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Failed)
    }

    fn hold(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::OnHold)
    }

    fn cancel(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Cancelled)
    }

    fn add_item(&self, shipment_id: ShipmentId, item: CreateShipmentItem) -> Result<ShipmentItem> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO shipment_items (id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                shipment_id.to_string(),
                item.order_item_id.map(|u| u.to_string()),
                item.product_id.map(|u| u.to_string()),
                item.sku,
                item.name,
                item.quantity,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute("DELETE FROM shipment_items WHERE id = ?", [item_id.to_string()])
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn get_items(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentItem>> {
        self.load_items(shipment_id)
    }

    fn add_event(&self, shipment_id: ShipmentId, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let event_time = event.event_time.unwrap_or(now);

        conn.execute(
            "INSERT INTO shipment_events (id, shipment_id, event_type, location, description, event_time, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                shipment_id.to_string(),
                event.event_type,
                event.location,
                event.description,
                event_time.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn get_events(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentEvent>> {
        self.load_events(shipment_id)
    }

    fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM shipments WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(order_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        if let Some(carrier) = filter.carrier {
            sql.push_str(" AND carrier = ?");
            params.push(Box::new(carrier.to_string()));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateShipment>) -> Result<BatchResult<Shipment>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(shipment) => result.record_success(shipment),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateShipment>) -> Result<Vec<Shipment>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let shipment_number = Shipment::generate_shipment_number();
            let now = Utc::now();
            let carrier = input.carrier.unwrap_or_default();
            let method = input.shipping_method.unwrap_or_default();
            let tracking_url =
                input.tracking_number.as_ref().and_then(|tn| carrier.tracking_url(tn));

            tx.execute(
                "INSERT INTO shipments (id, shipment_number, order_id, status, carrier, shipping_method,
                 tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                 shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                 signature_required, estimated_delivery, notes, created_at, updated_at)
                 VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    shipment_number,
                    input.order_id.to_string(),
                    carrier.to_string(),
                    method.to_string(),
                    input.tracking_number,
                    tracking_url,
                    input.recipient_name,
                    input.recipient_email,
                    input.recipient_phone,
                    input.shipping_address,
                    input.weight_kg.map(|w| w.to_string()),
                    input.dimensions,
                    input.shipping_cost.map(|c| c.to_string()),
                    input.insurance_amount.map(|a| a.to_string()),
                    i32::from(input.signature_required.unwrap_or(false)),
                    input.estimated_delivery.map(|dt| dt.to_rfc3339()),
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            let mut items = Vec::new();
            if let Some(item_inputs) = &input.items {
                for item_input in item_inputs {
                    let item_id = Uuid::new_v4();

                    tx.execute(
                        "INSERT INTO shipment_items (id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![
                            item_id.to_string(),
                            id.to_string(),
                            item_input.order_item_id.map(|u| u.to_string()),
                            item_input.product_id.map(|u| u.to_string()),
                            item_input.sku,
                            item_input.name,
                            item_input.quantity,
                            now.to_rfc3339(),
                            now.to_rfc3339(),
                        ],
                    )
                    .map_err(map_db_error)?;

                    items.push(ShipmentItem {
                        id: item_id,
                        shipment_id: ShipmentId::from(id),
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

            results.push(Shipment {
                id: ShipmentId::from(id),
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

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(
        &self,
        updates: Vec<(ShipmentId, UpdateShipment)>,
    ) -> Result<BatchResult<Shipment>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(shipment) => result.record_success(shipment),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(ShipmentId, UpdateShipment)>,
    ) -> Result<Vec<Shipment>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut updated_ids = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            // Get existing shipment data
            type ShipmentExistingRow = (
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
            );

            let existing_data: ShipmentExistingRow = tx
                .query_row(
                    "SELECT carrier, shipping_method, tracking_number, recipient_name, recipient_email, recipient_phone, shipping_address, weight_kg, dimensions FROM shipments WHERE id = ?",
                    [id.to_string()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                            row.get(8)?,
                        ))
                    },
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                    e => map_db_error(e),
                })?;

            let existing_carrier: ShippingCarrier =
                parse_enum(&existing_data.0, "shipment", "carrier")?;
            let new_status = input.status.map(|s| s.to_string());
            let new_carrier = input.carrier.unwrap_or(existing_carrier);
            let new_tracking = input.tracking_number.or(existing_data.2);
            let new_tracking_url =
                new_tracking.as_ref().and_then(|tn| new_carrier.tracking_url(tn));
            let new_recipient_name = input.recipient_name.unwrap_or(existing_data.3);
            let new_recipient_email = input.recipient_email.or(existing_data.4);
            let new_recipient_phone = input.recipient_phone.or(existing_data.5);
            let new_shipping_address = input.shipping_address.unwrap_or(existing_data.6);
            let new_weight = input.weight_kg.map(|w| w.to_string()).or(existing_data.7);
            let new_dimensions = input.dimensions.or(existing_data.8);
            let new_shipping_cost = input.shipping_cost.map(|c| c.to_string());
            let new_estimated_delivery = input.estimated_delivery.map(|dt| dt.to_rfc3339());
            let new_notes = input.notes;

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(status) = new_status {
                update_parts.push("status = ?");
                params.push(Box::new(status));
            }
            update_parts.push("carrier = ?");
            params.push(Box::new(new_carrier.to_string()));
            update_parts.push("tracking_number = ?");
            params.push(Box::new(new_tracking));
            update_parts.push("tracking_url = ?");
            params.push(Box::new(new_tracking_url));
            update_parts.push("recipient_name = ?");
            params.push(Box::new(new_recipient_name));
            update_parts.push("recipient_email = ?");
            params.push(Box::new(new_recipient_email));
            update_parts.push("recipient_phone = ?");
            params.push(Box::new(new_recipient_phone));
            update_parts.push("shipping_address = ?");
            params.push(Box::new(new_shipping_address));
            update_parts.push("weight_kg = ?");
            params.push(Box::new(new_weight));
            update_parts.push("dimensions = ?");
            params.push(Box::new(new_dimensions));
            if let Some(cost) = new_shipping_cost {
                update_parts.push("shipping_cost = ?");
                params.push(Box::new(cost));
            }
            if let Some(delivery) = new_estimated_delivery {
                update_parts.push("estimated_delivery = ?");
                params.push(Box::new(delivery));
            }
            if let Some(notes) = new_notes {
                update_parts.push("notes = ?");
                params.push(Box::new(notes));
            }

            params.push(Box::new(id.to_string()));

            let sql = format!("UPDATE shipments SET {} WHERE id = ?", update_parts.join(", "));

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();
            tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

            updated_ids.push(id);
        }

        tx.commit().map_err(map_db_error)?;

        // Fetch all updated shipments
        let mut results = Vec::with_capacity(updated_ids.len());
        for id in updated_ids {
            if let Some(shipment) = self.get(id)? {
                results.push(shipment);
            }
        }

        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<ShipmentId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id: Uuid = id.into();
            match self.delete(id) {
                Ok(()) => result.record_success(raw_id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<ShipmentId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| (*id).into()).collect();
        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        // Delete shipment events first
        let sql = format!("DELETE FROM shipment_events WHERE shipment_id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete shipment items
        let sql = format!("DELETE FROM shipment_items WHERE shipment_id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete shipments (mark as cancelled)
        let now = Utc::now().to_rfc3339();
        for id in &ids {
            tx.execute(
                "UPDATE shipments SET status = 'cancelled', updated_at = ? WHERE id = ?",
                rusqlite::params![now, id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<ShipmentId>) -> Result<Vec<Shipment>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let placeholders = build_in_clause(ids.len());
        let sql = format!(
            "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                    tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                    shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                    signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                    created_at, updated_at
             FROM shipments WHERE id IN ({placeholders})"
        );

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| (*id).into()).collect();
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, i32>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                    row.get::<_, Option<String>>(19)?,
                    row.get::<_, Option<String>>(20)?,
                    row.get::<_, String>(21)?,
                    row.get::<_, String>(22)?,
                ))
            })
            .map_err(map_db_error)?;

        let mut shipments = Vec::new();
        for row in rows {
            let (
                id_str,
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
                created_at,
                updated_at,
            ) = row.map_err(map_db_error)?;

            let shipment_id = ShipmentId::from(parse_uuid(&id_str, "shipment", "id")?);
            let items = self.load_items(shipment_id)?;
            let events = self.load_events(shipment_id)?;

            shipments.push(Shipment {
                id: shipment_id,
                shipment_number,
                order_id: OrderId::from(parse_uuid(&order_id, "shipment", "order_id")?),
                status: parse_enum(&status, "shipment", "status")?,
                carrier: parse_enum(&carrier, "shipment", "carrier")?,
                shipping_method: parse_enum(&shipping_method, "shipment", "shipping_method")?,
                tracking_number,
                tracking_url,
                recipient_name,
                recipient_email,
                recipient_phone,
                shipping_address,
                weight_kg: parse_decimal_opt(weight_kg, "shipment", "weight_kg")?,
                dimensions,
                shipping_cost: parse_decimal_opt(shipping_cost, "shipment", "shipping_cost")?,
                insurance_amount: parse_decimal_opt(
                    insurance_amount,
                    "shipment",
                    "insurance_amount",
                )?,
                signature_required: signature_required != 0,
                shipped_at: parse_datetime_opt(shipped_at, "shipment", "shipped_at")?,
                estimated_delivery: parse_datetime_opt(
                    estimated_delivery,
                    "shipment",
                    "estimated_delivery",
                )?,
                delivered_at: parse_datetime_opt(delivered_at, "shipment", "delivered_at")?,
                notes,
                items,
                events,
                version: 1,
                created_at: parse_datetime(&created_at, "shipment", "created_at")?,
                updated_at: parse_datetime(&updated_at, "shipment", "updated_at")?,
            });
        }

        Ok(shipments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateShipment, OrderId, ShipmentFilter, ShipmentRepository, ShipmentStatus,
        ShippingCarrier, ShippingMethod,
    };

    fn fresh_repo() -> SqliteShipmentRepository {
        SqliteDatabase::in_memory().expect("in-memory").shipments()
    }

    fn make_shipment(repo: &SqliteShipmentRepository, tracking: Option<&str>) -> Shipment {
        repo.create(CreateShipment {
            order_id: OrderId::new(),
            carrier: Some(ShippingCarrier::Ups),
            shipping_method: Some(ShippingMethod::Ground),
            tracking_number: tracking.map(String::from),
            recipient_name: "Ada Lovelace".into(),
            recipient_email: Some("ada@example.com".into()),
            recipient_phone: None,
            shipping_address: "1 Babbage Way, London".into(),
            weight_kg: Some(dec!(2.5)),
            dimensions: Some("30x20x10cm".into()),
            shipping_cost: Some(dec!(8.99)),
            insurance_amount: None,
            signature_required: Some(false),
            estimated_delivery: None,
            notes: None,
            items: None,
        })
        .expect("create shipment")
    }

    #[test]
    fn create_shipment_round_trips() {
        let repo = fresh_repo();
        let s = make_shipment(&repo, Some("1Z9999"));
        assert_eq!(s.recipient_name, "Ada Lovelace");
        assert_eq!(s.tracking_number.as_deref(), Some("1Z9999"));
        assert_eq!(s.carrier, ShippingCarrier::Ups);
        assert!(!s.shipment_number.is_empty());

        let by_id = repo.get(s.id).expect("ok").expect("found");
        assert_eq!(by_id.id, s.id);
        let by_num = repo.get_by_number(&s.shipment_number).expect("ok").expect("found");
        assert_eq!(by_num.id, s.id);
        assert!(repo.get_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn get_by_tracking_finds_shipment() {
        let repo = fresh_repo();
        let s = make_shipment(&repo, Some("TRACK-XYZ"));
        let by_track = repo.get_by_tracking("TRACK-XYZ").expect("ok").expect("found");
        assert_eq!(by_track.id, s.id);
        assert!(repo.get_by_tracking("missing").expect("ok").is_none());
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let pending = make_shipment(&repo, Some("P1"));
        let to_cancel = make_shipment(&repo, Some("P2"));
        repo.cancel(to_cancel.id).expect("cancel");

        let pendings = repo
            .list(ShipmentFilter { status: Some(ShipmentStatus::Pending), ..Default::default() })
            .expect("pending");
        let cancelleds = repo
            .list(ShipmentFilter { status: Some(ShipmentStatus::Cancelled), ..Default::default() })
            .expect("cancelled");
        assert!(pendings.iter().any(|s| s.id == pending.id));
        assert!(cancelleds.iter().any(|s| s.id == to_cancel.id));
    }

    #[test]
    fn list_filters_by_carrier() {
        let repo = fresh_repo();
        make_shipment(&repo, Some("UPS-1"));
        make_shipment(&repo, Some("UPS-2"));
        repo.create(CreateShipment {
            order_id: OrderId::new(),
            carrier: Some(ShippingCarrier::FedEx),
            shipping_method: Some(ShippingMethod::Express),
            tracking_number: Some("FEDEX-1".into()),
            recipient_name: "Test".into(),
            recipient_email: None,
            recipient_phone: None,
            shipping_address: "123 Test St".into(),
            weight_kg: None,
            dimensions: None,
            shipping_cost: None,
            insurance_amount: None,
            signature_required: None,
            estimated_delivery: None,
            notes: None,
            items: None,
        })
        .expect("fedex");

        let ups = repo
            .list(ShipmentFilter { carrier: Some(ShippingCarrier::Ups), ..Default::default() })
            .expect("ups");
        assert!(ups.iter().all(|s| s.carrier == ShippingCarrier::Ups));
        assert!(ups.len() >= 2);
    }

    #[test]
    fn cancel_transitions_to_cancelled() {
        let repo = fresh_repo();
        let s = make_shipment(&repo, Some("CANCEL-1"));
        let cancelled = repo.cancel(s.id).expect("cancel");
        assert_eq!(cancelled.status, ShipmentStatus::Cancelled);
    }

    #[test]
    fn get_items_returns_empty_for_shipment_without_items() {
        let repo = fresh_repo();
        let s = make_shipment(&repo, Some("NO-ITEMS"));
        let items = repo.get_items(s.id).expect("items");
        assert!(items.is_empty());
    }

    #[test]
    fn get_events_returns_at_most_one_initial_event() {
        let repo = fresh_repo();
        let s = make_shipment(&repo, Some("NO-EVENTS"));
        let events = repo.get_events(s.id).expect("events");
        assert!(events.len() <= 1);
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let result = repo
            .create_batch(vec![
                CreateShipment {
                    order_id: OrderId::new(),
                    carrier: Some(ShippingCarrier::Ups),
                    shipping_method: Some(ShippingMethod::Ground),
                    tracking_number: Some("B1".into()),
                    recipient_name: "X".into(),
                    recipient_email: None,
                    recipient_phone: None,
                    shipping_address: "addr".into(),
                    weight_kg: None,
                    dimensions: None,
                    shipping_cost: None,
                    insurance_amount: None,
                    signature_required: None,
                    estimated_delivery: None,
                    notes: None,
                    items: None,
                },
                CreateShipment {
                    order_id: OrderId::new(),
                    carrier: Some(ShippingCarrier::FedEx),
                    shipping_method: Some(ShippingMethod::Express),
                    tracking_number: Some("B2".into()),
                    recipient_name: "Y".into(),
                    recipient_email: None,
                    recipient_phone: None,
                    shipping_address: "addr".into(),
                    weight_kg: None,
                    dimensions: None,
                    shipping_cost: None,
                    insurance_amount: None,
                    signature_required: None,
                    estimated_delivery: None,
                    notes: None,
                    items: None,
                },
            ])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(stateset_core::ShipmentId::new()).expect("ok").is_none());
    }
}
