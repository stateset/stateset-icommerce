//! Backorder fulfillment from received PO stock, in the caller transaction.
use super::map_db_error;
use crate::stock_move::{LotEffect, StockMove};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, FulfillBackorder, MovementType};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn validate_source(
    tx: &mut Transaction<'_, Postgres>,
    input: &FulfillBackorder,
    sku: &str,
    now: DateTime<Utc>,
) -> DbResult<Uuid> {
    let _ = now;
    let po = input
        .source_id
        .ok_or_else(|| invalid("Purchase-order fulfillment requires a source purchase order ID"))?;
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM purchase_orders WHERE id = $1 FOR UPDATE",
    )
    .bind(po)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| invalid("Unknown source purchase order"))?;
    if !matches!(status.as_str(), "partially_received" | "received" | "completed") {
        return Err(invalid(
            "Source purchase order has no received stock available for fulfillment",
        ));
    }
    let lines = sqlx::query_scalar::<_, Decimal>("SELECT quantity_received FROM purchase_order_items WHERE purchase_order_id = $1 AND sku = $2").bind(po).bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    if lines.iter().all(|quantity| *quantity <= Decimal::ZERO) {
        return Err(invalid("Source purchase order has not received the backordered SKU"));
    }
    Ok(po)
}
pub(super) async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    input: &FulfillBackorder,
    sku: &str,
    warehouse: Option<i32>,
    po: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let placements = sqlx::query_as::<_, (Uuid, i32, Option<Uuid>, Decimal, i32)>("SELECT p.id, p.to_location_id, p.lot_id, p.quantity, r.warehouse_id FROM put_aways p JOIN receipt_items i ON i.id = p.receipt_item_id JOIN receipts r ON r.id = p.receipt_id JOIN purchase_order_items l ON l.id = i.po_line_id WHERE r.reference_type = 'purchase_order' AND r.reference_id = $1 AND l.purchase_order_id = $1 AND l.sku = $2 AND p.sku = $2 AND p.status = 'completed' AND r.status != 'cancelled' ORDER BY r.warehouse_id, p.to_location_id, p.id").bind(po).bind(sku).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let mut remaining = input.quantity;
    for (put_away, location, lot, received, source_warehouse) in placements {
        if remaining.is_zero() {
            break;
        }
        if warehouse.is_some_and(|warehouse| warehouse != source_warehouse) {
            continue;
        }
        let prior = sqlx::query_scalar::<_, Decimal>(
            "SELECT quantity FROM backorder_receipt_stock WHERE put_away_id = $1",
        )
        .bind(put_away)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let used: Decimal = prior.into_iter().sum();
        let attributable = received - used;
        if attributable <= Decimal::ZERO {
            continue;
        }
        let lot_key = lot.unwrap_or(Uuid::nil());
        let available = sqlx::query_as::<_, (Decimal, Decimal)>("SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = $1 AND sku = $2 AND lot_id = $3").bind(location).bind(sku).bind(lot_key).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.map(|(on_hand, reserved)| on_hand - reserved).unwrap_or_default();
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
        )
        .await?;
        let current = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM backorder_receipt_stock WHERE backorder_id = $1 AND put_away_id = $2").bind(input.backorder_id).bind(put_away).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
        sqlx::query("INSERT INTO backorder_receipt_stock (backorder_id, put_away_id, quantity) VALUES ($1, $2, $3) ON CONFLICT(backorder_id, put_away_id) DO UPDATE SET quantity = excluded.quantity").bind(input.backorder_id).bind(put_away).bind(current + take).execute(tx.as_mut()).await.map_err(map_db_error)?;
        remaining -= take;
    }
    if remaining > Decimal::ZERO {
        return Err(invalid(
            "Source purchase order has insufficient unreserved stock at its receiving destinations",
        ));
    }
    Ok(())
}
