//! SQLite Shipment repository implementation

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use std::str::FromStr;
use stateset_core::{
    AddShipmentEvent, CommerceError, CreateShipment, CreateShipmentItem, Result, Shipment,
    ShipmentEvent, ShipmentFilter, ShipmentItem, ShipmentRepository, ShipmentStatus,
    ShippingCarrier, ShippingMethod, UpdateShipment,
};
use uuid::Uuid;

/// SQLite implementation of ShipmentRepository
pub struct SqliteShipmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteShipmentRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn parse_status(s: &str) -> ShipmentStatus {
        match s {
            "processing" => ShipmentStatus::Processing,
            "ready_to_ship" => ShipmentStatus::ReadyToShip,
            "shipped" => ShipmentStatus::Shipped,
            "in_transit" => ShipmentStatus::InTransit,
            "out_for_delivery" => ShipmentStatus::OutForDelivery,
            "delivered" => ShipmentStatus::Delivered,
            "failed" => ShipmentStatus::Failed,
            "returned" => ShipmentStatus::Returned,
            "cancelled" => ShipmentStatus::Cancelled,
            "on_hold" => ShipmentStatus::OnHold,
            _ => ShipmentStatus::Pending,
        }
    }

    fn parse_carrier(s: &str) -> ShippingCarrier {
        match s {
            "ups" => ShippingCarrier::Ups,
            "fedex" => ShippingCarrier::FedEx,
            "usps" => ShippingCarrier::Usps,
            "dhl" => ShippingCarrier::Dhl,
            "ontrac" => ShippingCarrier::OnTrac,
            "lasership" => ShippingCarrier::LaserShip,
            _ => ShippingCarrier::Other,
        }
    }

    fn parse_method(s: &str) -> ShippingMethod {
        match s {
            "express" => ShippingMethod::Express,
            "overnight" => ShippingMethod::Overnight,
            "two_day" => ShippingMethod::TwoDay,
            "ground" => ShippingMethod::Ground,
            "international" => ShippingMethod::International,
            "same_day" => ShippingMethod::SameDay,
            "freight" => ShippingMethod::Freight,
            _ => ShippingMethod::Standard,
        }
    }

    fn parse_datetime(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
    }

    fn parse_optional_datetime(s: Option<String>) -> Option<DateTime<Utc>> {
        s.map(|s| Self::parse_datetime(&s))
    }

    fn load_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, shipment_id, order_item_id, product_id, sku, name, quantity, created_at, updated_at
                 FROM shipment_items WHERE shipment_id = ?",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([shipment_id.to_string()], |row| {
                Ok(ShipmentItem {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    shipment_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    order_item_id: row
                        .get::<_, Option<String>>(2)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    product_id: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    sku: row.get(4)?,
                    name: row.get(5)?,
                    quantity: row.get(6)?,
                    created_at: Self::parse_datetime(&row.get::<_, String>(7)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(8)?),
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(items)
    }

    fn load_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, shipment_id, event_type, location, description, event_time, created_at
                 FROM shipment_events WHERE shipment_id = ? ORDER BY event_time DESC",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([shipment_id.to_string()], |row| {
                Ok(ShipmentEvent {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    shipment_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    event_type: row.get(2)?,
                    location: row.get(3)?,
                    description: row.get(4)?,
                    event_time: Self::parse_datetime(&row.get::<_, String>(5)?),
                    created_at: Self::parse_datetime(&row.get::<_, String>(6)?),
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(events)
    }

    fn update_status(&self, id: Uuid, status: ShipmentStatus) -> Result<Shipment> {
        let now = Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        let tracking_url = input
            .tracking_number
            .as_ref()
            .and_then(|tn| carrier.tracking_url(tn));

        let mut items = Vec::new();
        {
            let mut conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let tx = conn
                .transaction()
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
                    input.signature_required.unwrap_or(false) as i32,
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

            tx.commit()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

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

    fn get(&self, id: Uuid) -> Result<Option<Shipment>> {
        let shipment_data = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
                let shipment_id = Uuid::parse_str(&id_str).unwrap_or_default();
                let items = self.load_items(shipment_id)?;
                let events = self.load_events(shipment_id)?;

                Ok(Some(Shipment {
                    id: shipment_id,
                    shipment_number,
                    order_id: Uuid::parse_str(&order_id).unwrap_or_default(),
                    status: Self::parse_status(&status),
                    carrier: Self::parse_carrier(&carrier),
                    shipping_method: Self::parse_method(&shipping_method),
                    tracking_number,
                    tracking_url,
                    recipient_name,
                    recipient_email,
                    recipient_phone,
                    shipping_address,
                    weight_kg: weight_kg.and_then(|s| Decimal::from_str(&s).ok()),
                    dimensions,
                    shipping_cost: shipping_cost.and_then(|s| Decimal::from_str(&s).ok()),
                    insurance_amount: insurance_amount.and_then(|s| Decimal::from_str(&s).ok()),
                    signature_required: signature_required != 0,
                    shipped_at: Self::parse_optional_datetime(shipped_at),
                    estimated_delivery: Self::parse_optional_datetime(estimated_delivery),
                    delivered_at: Self::parse_optional_datetime(delivered_at),
                    notes,
                    items,
                    events,
                    version: 1, // Default to 1 for backwards compatibility
                    created_at: Self::parse_datetime(&created_at),
                    updated_at: Self::parse_datetime(&updated_at),
                }))
            }
            None => Ok(None),
        }
    }

    fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        let id_result = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM shipments WHERE shipment_number = ?",
                [shipment_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(Uuid::parse_str(&id_str).unwrap_or_default()),
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
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM shipments WHERE tracking_number = ?",
                [tracking_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(Uuid::parse_str(&id_str).unwrap_or_default()),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        };

        match id_result {
            Some(id) => self.get(id),
            None => Ok(None),
        }
    }

    fn update(&self, id: Uuid, input: UpdateShipment) -> Result<Shipment> {
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
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

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let limit = filter.limit.unwrap_or(100) as i64;
            let offset = filter.offset.unwrap_or(0) as i64;

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

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let mut id_list = Vec::new();
            for row in rows {
                let id_str = row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                id_list.push(Uuid::parse_str(&id_str).unwrap_or_default());
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

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Shipment>> {
        self.list(ShipmentFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE shipments SET status = 'cancelled', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn mark_processing(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Processing)
    }

    fn mark_ready(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::ReadyToShip)
    }

    fn ship(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment> {
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let tracking_url = tracking_number
            .as_ref()
            .and_then(|tn| existing.carrier.tracking_url(tn));

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn mark_in_transit(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::InTransit)
    }

    fn mark_out_for_delivery(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::OutForDelivery)
    }

    fn mark_delivered(&self, id: Uuid) -> Result<Shipment> {
        let now = Utc::now();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "UPDATE shipments SET status = 'delivered', delivered_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_failed(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Failed)
    }

    fn hold(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::OnHold)
    }

    fn cancel(&self, id: Uuid) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Cancelled)
    }

    fn add_item(&self, shipment_id: Uuid, item: CreateShipmentItem) -> Result<ShipmentItem> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "DELETE FROM shipment_items WHERE id = ?",
            [item_id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn get_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        self.load_items(shipment_id)
    }

    fn add_event(&self, shipment_id: Uuid, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn get_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        self.load_events(shipment_id)
    }

    fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(count as u64)
    }
}
