//! SQLite Shipment repository implementation

use super::{
    map_db_error, parse_datetime, parse_datetime_opt, parse_datetime_row, parse_decimal_opt,
    parse_enum, parse_uuid, parse_uuid_row,
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

/// Shipment statuses that precede the carrier hand-off, as a SQL `IN (...)`
/// body. When the order they belong to ships in full, these follow it to
/// `shipped`; `on_hold` is left alone (a hold is an explicit decision the
/// order ship must not override), as is `cancelled` and everything at or past
/// `shipped`. `readytoship` is the legacy spelling `ShipmentStatus` still
/// parses. Mirrored exactly in the Postgres backend.
const PRE_SHIP_STATUSES_SQL: &str = "('pending', 'processing', 'ready_to_ship', 'readytoship')";

/// Carry a fully shipped order onto its open shipment records, inside the
/// caller's transaction.
///
/// Every shipment of `order_id` still in a pre-ship status (`pending`,
/// `processing`, `ready_to_ship`) becomes `shipped` with `shipped_at = now`.
/// A shipment that already carries a tracking number keeps it; one without
/// adopts the order's `tracking_number` (and the carrier's tracking URL for
/// it). Each moved shipment records a `shipment.status_changed` outbox fact
/// in the same transaction. Returns the recorded facts' event ids (one per
/// moved shipment).
///
/// A *partial* order shipment does not call this: which package carried
/// which units is not knowable from the order lines, so those shipments are
/// advanced explicitly with `ShipmentRepository::ship`.
pub(crate) fn ship_open_shipments_for_order_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
    tracking_number: Option<&str>,
    now: chrono::DateTime<Utc>,
) -> rusqlite::Result<Vec<Uuid>> {
    let open: Vec<(String, String, String, Option<String>, i64)> = {
        let mut stmt = tx.prepare(&format!(
            "SELECT id, status, carrier, tracking_number, version FROM shipments
             WHERE order_id = ? AND status IN {PRE_SHIP_STATUSES_SQL} ORDER BY created_at, id"
        ))?;
        stmt.query_map([order_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut event_ids = Vec::with_capacity(open.len());
    for (shipment_id, previous_status, carrier, own_tracking, version) in open {
        let next_version = version.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?;
        let adopted = if own_tracking.is_none() { tracking_number } else { None };
        let tracking_url = adopted.and_then(|tn| {
            carrier.parse::<ShippingCarrier>().ok().and_then(|c| c.tracking_url(tn))
        });
        let rows = tx.execute(
            &format!(
                "UPDATE shipments SET status = 'shipped', version = version + 1,
                        tracking_number = COALESCE(tracking_number, ?),
                        tracking_url = COALESCE(?, tracking_url),
                        shipped_at = COALESCE(shipped_at, ?), updated_at = ?
                 WHERE id = ? AND status IN {PRE_SHIP_STATUSES_SQL}"
            ),
            rusqlite::params![
                adopted,
                tracking_url,
                now.to_rfc3339(),
                now.to_rfc3339(),
                shipment_id
            ],
        )?;
        if rows == 0 {
            continue;
        }
        let event_id = super::kernel_outbox::record_outbox_fact(
            tx,
            crate::kernel_outbox::RecordedFact {
                event_type: "shipment.status_changed",
                aggregate_type: "shipment",
                aggregate_id: &shipment_id,
                payload: serde_json::json!({
                    "shipment_id": shipment_id,
                    "order_id": order_id,
                    "previous_status": previous_status,
                    "status": ShipmentStatus::Shipped.to_string(),
                    "tracking_number": own_tracking.as_deref().or(adopted),
                    "reason": "order_shipped",
                    "version": next_version,
                }),
            },
        )?;
        event_ids.push(event_id);
    }
    Ok(event_ids)
}

/// Shipment statuses at or past the carrier hand-off
/// ([`ShipmentStatus::has_left`]), as a SQL `IN (...)` body including the
/// legacy spellings `ShipmentStatus` still parses. Mirrored in Postgres.
const LEFT_STATUSES_SQL: &str = "('shipped', 'in_transit', 'intransit', 'out_for_delivery', \
     'outfordelivery', 'delivered', 'failed', 'returned')";

/// Shipments still waiting to leave, holds included, as a SQL `IN (...)` body.
const OPEN_STATUSES_SQL: &str =
    "('pending', 'processing', 'ready_to_ship', 'readytoship', 'on_hold', 'onhold')";

/// The first shipment of an order that is on hold, by number.
pub(crate) fn held_shipment_for_order_in_tx(
    tx: &rusqlite::Connection,
    order_id: &str,
) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    tx.query_row(
        "SELECT shipment_number FROM shipments
         WHERE order_id = ? AND status IN ('on_hold', 'onhold') ORDER BY created_at, id LIMIT 1",
        [order_id],
        |row| row.get(0),
    )
    .optional()
}

/// The first shipment of an order that has left the building
/// ([`ShipmentStatus::has_left`]), as `"<number> (<status>)"`.
pub(crate) fn left_shipment_for_order_in_tx(
    tx: &rusqlite::Connection,
    order_id: &str,
) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    tx.query_row(
        &format!(
            "SELECT shipment_number, status FROM shipments
             WHERE order_id = ? AND status IN {LEFT_STATUSES_SQL} ORDER BY created_at, id LIMIT 1"
        ),
        [order_id],
        |row| Ok(format!("{} ({})", row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .optional()
}

/// Refuse to finish shipping an order while one of its shipments is on hold.
///
/// A hold is an explicit decision (an address check, a fraud review) that an
/// order ship must not override, and a fully shipped order may not keep a
/// package waiting to leave. So the ship that would complete the order is
/// refused until the hold is released or the shipment cancelled.
pub(crate) fn ensure_no_held_shipments_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
) -> rusqlite::Result<()> {
    match held_shipment_for_order_in_tx(tx, order_id)? {
        Some(number) => Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            CommerceError::Conflict(format!(
                "order {order_id} cannot finish shipping while shipment {number} is on hold; \
                 release or cancel the hold first"
            )),
        ))),
        None => Ok(()),
    }
}

/// Refuse to cancel an order once any of its shipments has left the building
/// (shipped, in transit, delivered, ...): those units need a return.
pub(crate) fn ensure_no_shipment_left_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
) -> rusqlite::Result<()> {
    match left_shipment_for_order_in_tx(tx, order_id)? {
        Some(shipment) => Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            CommerceError::Conflict(format!(
                "order {order_id} cannot be cancelled: shipment {shipment} has already left; \
                 create a return for the shipped units instead"
            )),
        ))),
        None => Ok(()),
    }
}

/// Cancel every shipment of a cancelled order that never left (pending,
/// processing, ready to ship or on hold), inside the caller's transaction.
/// Each records a `shipment.status_changed` fact (`reason: order_cancelled`);
/// returns the facts' event ids.
pub(crate) fn cancel_open_shipments_for_order_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
    now: chrono::DateTime<Utc>,
) -> rusqlite::Result<Vec<Uuid>> {
    let open: Vec<(String, String, i64)> = {
        let mut stmt = tx.prepare(&format!(
            "SELECT id, status, version FROM shipments
             WHERE order_id = ? AND status IN {OPEN_STATUSES_SQL} ORDER BY created_at, id"
        ))?;
        stmt.query_map([order_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut event_ids = Vec::with_capacity(open.len());
    for (shipment_id, previous_status, version) in open {
        let next_version = version.checked_add(1).ok_or(rusqlite::Error::InvalidQuery)?;
        let rows = tx.execute(
            &format!(
                "UPDATE shipments SET status = 'cancelled', version = version + 1, updated_at = ?
                 WHERE id = ? AND status IN {OPEN_STATUSES_SQL}"
            ),
            rusqlite::params![now.to_rfc3339(), shipment_id],
        )?;
        if rows == 0 {
            continue;
        }
        let event_id = super::kernel_outbox::record_outbox_fact(
            tx,
            crate::kernel_outbox::RecordedFact {
                event_type: "shipment.status_changed",
                aggregate_type: "shipment",
                aggregate_id: &shipment_id,
                payload: serde_json::json!({
                    "shipment_id": shipment_id,
                    "order_id": order_id,
                    "previous_status": previous_status,
                    "status": ShipmentStatus::Cancelled.to_string(),
                    "reason": "order_cancelled",
                    "version": next_version,
                }),
            },
        )?;
        event_ids.push(event_id);
    }
    Ok(event_ids)
}

/// Refuse a new shipment (or a new shipment line) for an order that is
/// closed to fulfilment: fully shipped, delivered, cancelled or refunded.
/// A shipment for an order this store does not hold is left alone.
fn ensure_order_accepts_shipments(conn: &rusqlite::Connection, order_id: OrderId) -> Result<()> {
    use rusqlite::OptionalExtension;
    let status: Option<String> = conn
        .query_row("SELECT status FROM orders WHERE id = ?", [order_id.to_string()], |row| {
            row.get(0)
        })
        .optional()
        .map_err(map_db_error)?;
    match status {
        Some(status) => crate::shipment_allocations::validate_order_status(&status),
        None => Ok(()),
    }
}

/// The order lines (`id`, `sku`, `quantity`, `shipped_quantity`) of an order.
fn order_lines_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
) -> Result<Vec<(Uuid, String, i32, i32)>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, sku, quantity, shipped_quantity FROM order_items WHERE order_id = ? ORDER BY rowid",
        )
        .map_err(map_db_error)?;
    stmt.query_map([order_id], |row| {
        Ok((
            parse_uuid_row(&row.get::<_, String>(0)?, "order_item", "id")?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
        ))
    })
    .map_err(map_db_error)?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(map_db_error)
}

/// Σ item quantity per order line over the order's open (waiting to leave)
/// or `delivered` shipments, optionally excluding one shipment.
fn manifest_units_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
    delivered: bool,
    excluding: Option<&str>,
) -> Result<std::collections::BTreeMap<Uuid, i64>> {
    let mut stmt = tx
        .prepare(&if delivered {
            "SELECT si.order_item_id, si.quantity FROM shipment_items si
             JOIN shipments s ON s.id = si.shipment_id
             WHERE s.order_id = ? AND s.id != ? AND s.status = 'delivered'
               AND si.order_item_id IS NOT NULL"
                .to_string()
        } else {
            format!(
                "SELECT si.order_item_id, si.quantity FROM shipment_items si
                 JOIN shipments s ON s.id = si.shipment_id
                 WHERE s.order_id = ? AND s.id != ? AND s.status IN {OPEN_STATUSES_SQL}
                   AND si.order_item_id IS NOT NULL"
            )
        })
        .map_err(map_db_error)?;
    let rows = stmt
        .query_map([order_id, excluding.unwrap_or("")], |row| {
            Ok((
                parse_uuid_row(&row.get::<_, String>(0)?, "shipment_item", "order_item_id")?,
                row.get::<_, i64>(1)?,
            ))
        })
        .map_err(map_db_error)?;
    let mut units = std::collections::BTreeMap::new();
    for row in rows {
        let (line, quantity) = row.map_err(map_db_error)?;
        *units.entry(line).or_default() += quantity;
    }
    Ok(units)
}

/// A shipment just left the building: ship its units on the order, in the
/// caller's transaction (shipment → order sync).
///
/// Each manifest line ships its quantity on its order line, capped at what
/// the line still has unshipped, so a unit already shipped through the order
/// itself is never counted twice. A shipment without items carries the
/// order's remainder: every unshipped unit not promised to another open
/// shipment. The order then goes through `SqliteOrderRepository::apply_update_in_tx`
/// exactly like an explicit order ship (status `partially_shipped`/`shipped`,
/// fulfilment status, reservations fulfilled), which may walk a
/// pending/confirmed order through processing. Its own carry-along of open
/// shipments is plain SQL, so the two directions cannot recurse.
///
/// A shipment whose order is cancelled or refunded cannot leave; one whose
/// order is not in this store, or that moves no unit, changes nothing.
fn ship_order_for_shipment_in_tx(
    tx: &rusqlite::Transaction<'_>,
    shipment: &Shipment,
) -> Result<()> {
    use rusqlite::OptionalExtension;
    use stateset_core::{OrderStatus, ShipmentLineInput, UpdateOrder};
    let order_id = shipment.order_id.to_string();
    let Some(status) = tx
        .query_row("SELECT status FROM orders WHERE id = ?", [&order_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(map_db_error)?
    else {
        return Ok(());
    };
    let status: OrderStatus = parse_enum(&status, "order", "status")?;
    if matches!(status, OrderStatus::Cancelled | OrderStatus::Refunded) {
        return Err(CommerceError::ValidationError(format!(
            "shipment {} cannot ship: its order is {status}",
            shipment.shipment_number
        )));
    }
    let lines = order_lines_in_tx(tx, &order_id)?;
    let mut wanted: std::collections::BTreeMap<Uuid, i64> = std::collections::BTreeMap::new();
    if shipment.items.is_empty() {
        let promised = manifest_units_in_tx(tx, &order_id, false, Some(&shipment.id.to_string()))?;
        for (line, _, quantity, shipped) in &lines {
            let open = i64::from(*quantity) - i64::from(*shipped);
            wanted.insert(*line, open - promised.get(line).copied().unwrap_or(0));
        }
    } else {
        for item in &shipment.items {
            let line = item.order_item_id.or_else(|| {
                let mut by_sku = lines.iter().filter(|(_, sku, ..)| *sku == item.sku);
                match (by_sku.next(), by_sku.next()) {
                    (Some((id, ..)), None) => Some(*id),
                    _ => None,
                }
            });
            if let Some(line) = line {
                *wanted.entry(line).or_default() += i64::from(item.quantity);
            }
        }
    }
    let ship_lines: Vec<ShipmentLineInput> = lines
        .iter()
        .filter_map(|(line, _, quantity, shipped)| {
            let open = i64::from(*quantity) - i64::from(*shipped);
            let units = wanted.get(line).copied().unwrap_or(0).min(open);
            (units > 0).then(|| ShipmentLineInput {
                order_item_id: (*line).into(),
                quantity: i32::try_from(units).unwrap_or(i32::MAX),
            })
        })
        .collect();
    if ship_lines.is_empty() {
        return Ok(());
    }
    let outcome = super::orders::SqliteOrderRepository::apply_update_in_tx(
        tx,
        shipment.order_id,
        &UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        &super::orders::ShipMode::Lines(&ship_lines),
        true,
    )
    .map_err(map_db_error)?;
    match outcome.post_commit_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// A shipment was just delivered: deliver its order when the evidence is
/// complete, in the caller's transaction (shipment → order sync).
///
/// There is no per-line delivered quantity, so this is derived
/// conservatively: the order must be fully `shipped`, every one of its
/// shipments that was not cancelled must be `delivered`, and those delivered
/// manifests must cover every ordered unit. Anything less (units shipped
/// without a shipment record, an itemless package, one still in transit)
/// leaves the order `shipped` for an explicit `OrderRepository::deliver`.
fn deliver_order_for_shipment_in_tx(
    tx: &rusqlite::Transaction<'_>,
    shipment: &Shipment,
) -> Result<()> {
    use rusqlite::OptionalExtension;
    use stateset_core::{OrderStatus, UpdateOrder};
    let order_id = shipment.order_id.to_string();
    let Some(status) = tx
        .query_row("SELECT status FROM orders WHERE id = ?", [&order_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(map_db_error)?
    else {
        return Ok(());
    };
    if parse_enum::<OrderStatus>(&status, "order", "status")? != OrderStatus::Shipped {
        return Ok(());
    }
    let statuses: Vec<String> = {
        let mut stmt =
            tx.prepare("SELECT status FROM shipments WHERE order_id = ?").map_err(map_db_error)?;
        stmt.query_map([&order_id], |row| row.get(0))
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?
    };
    for status in &statuses {
        let status: ShipmentStatus = parse_enum(status, "shipment", "status")?;
        if !matches!(status, ShipmentStatus::Delivered | ShipmentStatus::Cancelled) {
            return Ok(());
        }
    }
    let delivered = manifest_units_in_tx(tx, &order_id, true, None)?;
    let covered = order_lines_in_tx(tx, &order_id)?.iter().all(|(line, _, quantity, _)| {
        delivered.get(line).copied().unwrap_or(0) >= i64::from(*quantity)
    });
    if !covered {
        return Ok(());
    }
    let outcome = super::orders::SqliteOrderRepository::apply_update_in_tx(
        tx,
        shipment.order_id,
        &UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() },
        &super::orders::ShipMode::None,
        false,
    )
    .map_err(map_db_error)?;
    match outcome.post_commit_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Insert a shipment (and its lines) on the caller's transaction (shared by
/// [`ShipmentRepository::create`] and the governed `shipments.create` kernel
/// command).
pub(crate) fn create_shipment_tx(
    tx: &rusqlite::Connection,
    mut input: CreateShipment,
) -> Result<Shipment> {
    crate::shipment_updates::validate_create_items(&input)?;
    ensure_order_accepts_shipments(tx, input.order_id)?;
    if let Some(items) = input.items.as_ref().filter(|items| !items.is_empty()) {
        input.items =
            Some(SqliteShipmentRepository::normalize_items_tx(tx, input.order_id, items)?);
    }
    let id = Uuid::new_v4();
    let shipment_number = Shipment::generate_shipment_number();
    let now = Utc::now();
    let carrier = input.carrier.unwrap_or_default();
    let method = input.shipping_method.unwrap_or_default();
    let tracking_url = input.tracking_number.as_ref().and_then(|tn| carrier.tracking_url(tn));

    let mut items = Vec::new();
    {
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

/// SQLite implementation of `ShipmentRepository`
#[derive(Debug)]
pub struct SqliteShipmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteShipmentRepository {
    fn normalize_items_tx(
        tx: &rusqlite::Connection,
        order_id: OrderId,
        inputs: &[CreateShipmentItem],
    ) -> Result<Vec<CreateShipmentItem>> {
        let status: String = match tx.query_row(
            "SELECT status FROM orders WHERE id = ?",
            [order_id.to_string()],
            |row| row.get(0),
        ) {
            Ok(status) => status,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(CommerceError::OrderNotFound(order_id.into_uuid()));
            }
            Err(error) => return Err(map_db_error(error)),
        };
        crate::shipment_allocations::validate_order_status(&status)?;
        let mut stmt = tx
            .prepare("SELECT id, product_id, sku, quantity FROM order_items WHERE order_id = ?")
            .map_err(map_db_error)?;
        let lines = stmt
            .query_map([order_id.to_string()], |row| {
                Ok(crate::shipment_allocations::Line {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "order_item", "id")?,
                    product_id: ProductId::from(parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "order_item",
                        "product_id",
                    )?),
                    sku: row.get(2)?,
                    quantity: row.get(3)?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;
        let mut stmt = tx.prepare("SELECT si.order_item_id, si.product_id, si.sku, si.quantity FROM shipment_items si JOIN shipments s ON s.id = si.shipment_id WHERE s.order_id = ? AND s.status != 'cancelled'").map_err(map_db_error)?;
        let existing = stmt
            .query_map([order_id.to_string()], |row| {
                Ok(crate::shipment_allocations::Assignment {
                    order_item_id: row
                        .get::<_, Option<String>>(0)?
                        .map(|s| parse_uuid_row(&s, "shipment_item", "order_item_id"))
                        .transpose()?,
                    product_id: row
                        .get::<_, Option<String>>(1)?
                        .map(|s| {
                            parse_uuid_row(&s, "shipment_item", "product_id").map(ProductId::from)
                        })
                        .transpose()?,
                    sku: row.get(2)?,
                    quantity: row.get(3)?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;
        crate::shipment_allocations::normalize(&lines, &existing, inputs)
    }

    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn load_items(
        conn: &rusqlite::Connection,
        shipment_id: ShipmentId,
    ) -> Result<Vec<ShipmentItem>> {
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

    fn load_events(
        conn: &rusqlite::Connection,
        shipment_id: ShipmentId,
    ) -> Result<Vec<ShipmentEvent>> {
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

    fn get_with_conn(conn: &rusqlite::Connection, id: ShipmentId) -> Result<Option<Shipment>> {
        let shipment_data = {
            let result = conn.query_row(
                "SELECT id, shipment_number, order_id, status, carrier, shipping_method,
                        tracking_number, tracking_url, recipient_name, recipient_email, recipient_phone,
                        shipping_address, weight_kg, dimensions, shipping_cost, insurance_amount,
                        signature_required, shipped_at, estimated_delivery, delivered_at, notes,
                        created_at, updated_at, version
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
                        row.get::<_, i32>(23)?,
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
                version,
            )) => {
                let shipment_id = ShipmentId::from(parse_uuid(&id_str, "shipment", "id")?);
                let items = Self::load_items(conn, shipment_id)?;
                let events = Self::load_events(conn, shipment_id)?;

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
                    version,
                    created_at: parse_datetime(&created_at, "shipment", "created_at")?,
                    updated_at: parse_datetime(&updated_at, "shipment", "updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    fn update_tx(
        tx: &rusqlite::Transaction<'_>,
        id: ShipmentId,
        input: UpdateShipment,
    ) -> Result<Shipment> {
        let mut shipment = Self::get_with_conn(tx, id)?.ok_or(CommerceError::NotFound)?;
        let previous_status = shipment.status;
        let previous_version = shipment.version;
        let changed = crate::shipment_updates::apply(&mut shipment, input, Utc::now())?;
        if changed.is_empty() {
            return Ok(shipment);
        }
        let rows = tx.execute(
            "UPDATE shipments SET status = ?, carrier = ?, tracking_number = ?, tracking_url = ?,
             recipient_name = ?, recipient_email = ?, recipient_phone = ?, shipping_address = ?,
             weight_kg = ?, dimensions = ?, shipping_cost = ?, estimated_delivery = ?, notes = ?,
             shipped_at = ?, delivered_at = ?, version = ?, updated_at = ? WHERE id = ? AND version = ?",
            rusqlite::params![
                shipment.status.to_string(), shipment.carrier.to_string(), shipment.tracking_number,
                shipment.tracking_url, shipment.recipient_name, shipment.recipient_email,
                shipment.recipient_phone, shipment.shipping_address,
                shipment.weight_kg.map(|v| v.to_string()), shipment.dimensions,
                shipment.shipping_cost.map(|v| v.to_string()), shipment.estimated_delivery.map(|v| v.to_rfc3339()),
                shipment.notes, shipment.shipped_at.map(|v| v.to_rfc3339()), shipment.delivered_at.map(|v| v.to_rfc3339()),
                shipment.version, shipment.updated_at.to_rfc3339(), id.to_string(), previous_version,
            ],
        ).map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::OptimisticLockFailure);
        }
        super::kernel_outbox::record_outbox_fact(
            tx,
            crate::kernel_outbox::RecordedFact {
                event_type: "shipments.updated.v1",
                aggregate_type: "shipment",
                aggregate_id: &id.to_string(),
                payload: crate::shipment_updates::fact(&shipment, previous_status, &changed),
            },
        )
        .map_err(map_db_error)?;
        // Shipment → order: the package's units ship (and deliver) on its
        // order in this same transaction.
        if shipment.status.has_left() && !previous_status.has_left() {
            ship_order_for_shipment_in_tx(tx, &shipment)?;
        }
        if shipment.status == ShipmentStatus::Delivered
            && previous_status != ShipmentStatus::Delivered
        {
            deliver_order_for_shipment_in_tx(tx, &shipment)?;
        }
        Ok(shipment)
    }

    fn record_item_change_tx(
        tx: &rusqlite::Transaction<'_>,
        shipment: &mut Shipment,
        item: &ShipmentItem,
        event_type: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let previous_version = shipment.version;
        crate::shipment_updates::change_contents(shipment, now)?;
        let rows = tx
            .execute(
                "UPDATE shipments SET version = ?, updated_at = ? WHERE id = ? AND version = ?",
                rusqlite::params![
                    shipment.version,
                    shipment.updated_at.to_rfc3339(),
                    shipment.id.to_string(),
                    previous_version
                ],
            )
            .map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::OptimisticLockFailure);
        }
        super::kernel_outbox::record_outbox_fact(
            tx,
            crate::kernel_outbox::RecordedFact {
                event_type,
                aggregate_type: "shipment",
                aggregate_id: &shipment.id.to_string(),
                payload: crate::shipment_updates::item_fact(shipment, item),
            },
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn update_status(&self, id: ShipmentId, status: ShipmentStatus) -> Result<Shipment> {
        self.update(id, UpdateShipment { status: Some(status), ..Default::default() })
    }
}

impl ShipmentRepository for SqliteShipmentRepository {
    fn create(&self, mut input: CreateShipment) -> Result<Shipment> {
        crate::shipment_updates::validate_create_items(&input)?;
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

            ensure_order_accepts_shipments(&tx, input.order_id)?;
            if let Some(inputs) = input.items.as_ref().filter(|items| !items.is_empty()) {
                input.items = Some(Self::normalize_items_tx(&tx, input.order_id, inputs)?);
            }

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

            super::kernel_outbox::record_outbox_fact(&tx, crate::kernel_outbox::RecordedFact {
                event_type: "shipments.created.v1", aggregate_type: "shipment", aggregate_id: &id.to_string(),
                payload: serde_json::json!({ "id": id, "order_id": input.order_id, "status": "pending", "version": 1,
                    "carrier": carrier, "tracking_number": input.tracking_number,
                    "shipping_cost": input.shipping_cost.map(|amount| amount.to_string()), "items": &items }),
            }).map_err(map_db_error)?;
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
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let result = Self::get_with_conn(&tx, id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(result)
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
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let shipment = Self::update_tx(&tx, id, input)?;
        tx.commit().map_err(map_db_error)?;
        Ok(shipment)
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
        self.cancel(id).map(|_| ())
    }

    fn mark_processing(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Processing)
    }

    fn mark_ready(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::ReadyToShip)
    }

    fn ship(&self, id: ShipmentId, tracking_number: Option<String>) -> Result<Shipment> {
        self.update(
            id,
            UpdateShipment {
                status: Some(ShipmentStatus::Shipped),
                tracking_number,
                ..Default::default()
            },
        )
    }

    fn mark_in_transit(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::InTransit)
    }

    fn mark_out_for_delivery(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::OutForDelivery)
    }

    fn mark_delivered(&self, id: ShipmentId) -> Result<Shipment> {
        self.update_status(id, ShipmentStatus::Delivered)
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
        self.add_item_with_version(shipment_id, item, None)
    }

    fn add_item_with_version(
        &self,
        shipment_id: ShipmentId,
        item: CreateShipmentItem,
        expected_version: Option<i32>,
    ) -> Result<ShipmentItem> {
        crate::shipment_updates::validate_item(&item)?;
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut shipment = Self::get_with_conn(&tx, shipment_id)?.ok_or(CommerceError::NotFound)?;
        crate::shipment_updates::check_version(&shipment, expected_version)?;
        crate::shipment_updates::ensure_items_editable(&shipment)?;
        let item = Self::normalize_items_tx(&tx, shipment.order_id, &[item])?
            .pop()
            .ok_or_else(|| CommerceError::Internal("Missing normalized shipment item".into()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        tx.execute(
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

        let created = ShipmentItem {
            id,
            shipment_id,
            order_item_id: item.order_item_id,
            product_id: item.product_id,
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            created_at: now,
            updated_at: now,
        };
        Self::record_item_change_tx(&tx, &mut shipment, &created, "shipments.item_added.v1", now)?;
        tx.commit().map_err(map_db_error)?;
        Ok(created)
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.remove_item_with_version(item_id, None)
    }

    fn remove_item_with_version(&self, item_id: Uuid, expected_version: Option<i32>) -> Result<()> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let parent: String = match tx.query_row(
            "SELECT shipment_id FROM shipment_items WHERE id = ?",
            [item_id.to_string()],
            |row| row.get(0),
        ) {
            Ok(parent) => parent,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Err(CommerceError::NotFound),
            Err(error) => return Err(map_db_error(error)),
        };
        let shipment_id = ShipmentId::from(parse_uuid(&parent, "shipment_item", "shipment_id")?);
        let mut shipment = Self::get_with_conn(&tx, shipment_id)?.ok_or(CommerceError::NotFound)?;
        crate::shipment_updates::check_version(&shipment, expected_version)?;
        crate::shipment_updates::ensure_items_editable(&shipment)?;
        let item = shipment
            .items
            .iter()
            .find(|item| item.id == item_id)
            .cloned()
            .ok_or(CommerceError::NotFound)?;
        tx.execute("DELETE FROM shipment_items WHERE id = ?", [item_id.to_string()])
            .map_err(map_db_error)?;
        Self::record_item_change_tx(
            &tx,
            &mut shipment,
            &item,
            "shipments.item_removed.v1",
            Utc::now(),
        )?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_items(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentItem>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Self::load_items(&conn, shipment_id)
    }

    fn add_event(&self, shipment_id: ShipmentId, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(map_db_error)?;
        let mut shipment = Self::get_with_conn(&tx, shipment_id)?.ok_or(CommerceError::NotFound)?;
        let previous_version = shipment.version;
        let event = crate::shipment_updates::prepare_event(&mut shipment, event, Utc::now())?;

        tx.execute(
            "INSERT INTO shipment_events (id, shipment_id, event_type, location, description, event_time, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                event.id.to_string(),
                shipment_id.to_string(),
                event.event_type,
                event.location,
                event.description,
                event.event_time.to_rfc3339(),
                event.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;
        let rows = tx
            .execute(
                "UPDATE shipments SET version = ?, updated_at = ? WHERE id = ? AND version = ?",
                rusqlite::params![
                    shipment.version,
                    shipment.updated_at.to_rfc3339(),
                    shipment_id.to_string(),
                    previous_version
                ],
            )
            .map_err(map_db_error)?;
        if rows != 1 {
            return Err(CommerceError::OptimisticLockFailure);
        }
        super::kernel_outbox::record_outbox_fact(
            &tx,
            crate::kernel_outbox::RecordedFact {
                event_type: "shipments.event_added.v1",
                aggregate_type: "shipment",
                aggregate_id: &shipment_id.to_string(),
                payload: crate::shipment_updates::event_fact(&shipment, &event),
            },
        )
        .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(event)
    }

    fn get_events(&self, shipment_id: ShipmentId) -> Result<Vec<ShipmentEvent>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Self::load_events(&conn, shipment_id)
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

        for mut input in inputs {
            crate::shipment_updates::validate_create_items(&input)?;
            ensure_order_accepts_shipments(&tx, input.order_id)?;
            if let Some(items) = input.items.as_ref().filter(|items| !items.is_empty()) {
                input.items = Some(Self::normalize_items_tx(&tx, input.order_id, items)?);
            }
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

            super::kernel_outbox::record_outbox_fact(&tx, crate::kernel_outbox::RecordedFact {
                event_type: "shipments.created.v1", aggregate_type: "shipment", aggregate_id: &id.to_string(),
                payload: serde_json::json!({ "id": id, "order_id": input.order_id, "status": "pending", "version": 1,
                    "carrier": carrier, "tracking_number": input.tracking_number,
                    "shipping_cost": input.shipping_cost.map(|amount| amount.to_string()), "items": &items }),
            }).map_err(map_db_error)?;
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
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());
        for (id, input) in updates {
            results.push(Self::update_tx(&tx, id, input)?);
        }
        tx.commit().map_err(map_db_error)?;
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
        self.update_batch_atomic(
            ids.into_iter()
                .map(|id| {
                    (
                        id,
                        UpdateShipment {
                            status: Some(ShipmentStatus::Cancelled),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        )
        .map(|_| ())
    }

    fn get_batch(&self, ids: Vec<ShipmentId>) -> Result<Vec<Shipment>> {
        validate_batch_size(&ids)?;
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut shipments = Vec::with_capacity(ids.len());
        let mut seen = std::collections::HashSet::with_capacity(ids.len());
        for id in ids.into_iter().filter(|id| seen.insert(*id)) {
            if let Some(shipment) = Self::get_with_conn(&tx, id)? {
                shipments.push(shipment);
            }
        }
        tx.commit().map_err(map_db_error)?;
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
