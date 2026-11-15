//! Backorder fulfillment from received PO stock, in the caller transaction.
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::{OptionalExtension, Transaction};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, FulfillBackorder, MovementType};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn validate_source(
    tx: &Transaction<'_>,
    input: &FulfillBackorder,
    sku: &str,
    now: &str,
) -> DbResult<Uuid> {
    let _ = now;
    let po = input
        .source_id
        .ok_or_else(|| invalid("Purchase-order fulfillment requires a source purchase order ID"))?;
    let status = tx
        .query_row(
            "SELECT status FROM purchase_orders WHERE id = ?1",
            rusqlite::params![(po).to_string()],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| invalid("Unknown source purchase order"))?;
    if !matches!(status.as_str(), "partially_received" | "received" | "completed") {
        return Err(invalid(
            "Source purchase order has no received stock available for fulfillment",
        ));
    }
    let lines = {
        let mut stmt = tx.prepare("SELECT quantity_received FROM purchase_order_items WHERE purchase_order_id = ?1 AND sku = ?2")?;
        let rows = stmt.query_map(rusqlite::params![(po).to_string(), sku], |r| {
            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if lines.iter().all(|quantity| *quantity <= Decimal::ZERO) {
        return Err(invalid("Source purchase order has not received the backordered SKU"));
    }
    Ok(po)
}
pub(super) fn consume(
    tx: &Transaction<'_>,
    input: &FulfillBackorder,
    sku: &str,
    warehouse: Option<i32>,
    po: Uuid,
    now: &str,
) -> DbResult<()> {
    let placements = {
        let mut stmt = tx.prepare("SELECT p.id, p.to_location_id, p.lot_id, p.quantity, r.warehouse_id FROM put_aways p JOIN receipt_items i ON i.id = p.receipt_item_id JOIN receipts r ON r.id = p.receipt_id JOIN purchase_order_items l ON l.id = i.po_line_id WHERE r.reference_type = 'purchase_order' AND r.reference_id = ?1 AND l.purchase_order_id = ?1 AND l.sku = ?2 AND p.sku = ?2 AND p.status = 'completed' AND r.status != 'cancelled' ORDER BY r.warehouse_id, p.to_location_id, p.id")?;
        let rows = stmt.query_map(rusqlite::params![(po).to_string(), sku], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, i32>(1)?,
                super::parse_uuid_opt_row(r.get(2)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                r.get::<_, i32>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut remaining = input.quantity;
    for (put_away, location, lot, received, source_warehouse) in placements {
        if remaining.is_zero() {
            break;
        }
        if warehouse.is_some_and(|warehouse| warehouse != source_warehouse) {
            continue;
        }
        let prior = {
            let mut stmt =
                tx.prepare("SELECT quantity FROM backorder_receipt_stock WHERE put_away_id = ?1")?;
            let rows = stmt.query_map(rusqlite::params![(put_away).to_string()], |r| {
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let used: Decimal = prior.into_iter().sum();
        let attributable = received - used;
        if attributable <= Decimal::ZERO {
            continue;
        }
        let lot_key = lot.map(|id| id.to_string()).unwrap_or_default();
        let available = tx.query_row("SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = ?1 AND sku = ?2 AND lot_id = ?3", rusqlite::params![location, sku, lot_key], |r| Ok((super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?))).optional()?.map(|(on_hand, reserved)| on_hand - reserved).unwrap_or_default();
        let take = remaining.min(attributable).min(available);
        if take <= Decimal::ZERO {
            continue;
        }
        super::stock_move::post(
            tx,
            &StockMove {
                id: Uuid::new_v4(),
                sku,
                lot_id: lot,
                from: Some(location),
                to: None,
                quantity: take,
                release_reserved: Decimal::ZERO,
                reserve_received: Decimal::ZERO,
                movement_type: MovementType::Shipment,
                lot_effect: LotEffect::Track,
                check_sellable: true,
                reference_type: Some("backorder"),
                reference_id: Some(input.backorder_id),
                reference_label: None,
                reason: Some("Backorder shipment from received purchase order"),
                performed_by: input.fulfilled_by.as_deref(),
            },
            now,
        )?;
        let current = tx.query_row("SELECT quantity FROM backorder_receipt_stock WHERE backorder_id = ?1 AND put_away_id = ?2", rusqlite::params![(input.backorder_id).to_string(), (put_away).to_string()], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
        tx.execute("INSERT INTO backorder_receipt_stock (backorder_id, put_away_id, quantity) VALUES (?1, ?2, ?3) ON CONFLICT(backorder_id, put_away_id) DO UPDATE SET quantity = excluded.quantity", rusqlite::params![(input.backorder_id).to_string(), (put_away).to_string(), (current + take).to_string()])?;
        remaining -= take;
    }
    if remaining > Decimal::ZERO {
        return Err(invalid(
            "Source purchase order has insufficient unreserved stock at its receiving destinations",
        ));
    }
    Ok(())
}
