//! Vendor returns post stock and their linked supplier credit atomically.
use super::map_db_error;
use crate::stock_move::{LotEffect, StockMove};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, CreateVendorReturn, CurrencyCode, MovementType};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
fn line_value(quantity: Decimal, cost: Decimal, currency: CurrencyCode) -> DbResult<Decimal> {
    if quantity <= Decimal::ZERO || cost < Decimal::ZERO {
        return Err(invalid("Vendor return quantities must be positive and costs nonnegative"));
    }
    quantity
        .checked_mul(cost)
        .map(|amount| amount.round_dp(u32::from(currency.decimal_places())))
        .ok_or_else(|| invalid("Vendor return value overflow"))
}
pub(super) async fn validate_create(
    tx: &mut Transaction<'_, Postgres>,
    input: &CreateVendorReturn,
    currency: CurrencyCode,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    if input.items.is_empty() {
        return Err(invalid("A vendor return requires at least one item"));
    }
    let supplier =
        sqlx::query_scalar::<_, i32>("SELECT COUNT(*)::integer FROM suppliers WHERE id = $1")
            .bind(input.supplier_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
    if supplier != 1 {
        return Err(invalid("Unknown vendor return supplier"));
    }
    if let Some(po) = input.purchase_order_id {
        let source = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT supplier_id, currency FROM purchase_orders WHERE id = $1",
        )
        .bind(po)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| invalid("Unknown originating purchase order"))?;
        if source.0 != input.supplier_id || source.1 != currency.to_string() {
            return Err(invalid(
                "Vendor return supplier and currency must match its purchase order",
            ));
        }
    }
    let mut total = Decimal::ZERO;
    for item in &input.items {
        let value = line_value(item.quantity, item.unit_cost, currency)?;
        total = total.checked_add(value).ok_or_else(|| invalid("Vendor return value overflow"))?;
        let matches = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*)::integer FROM product_variants WHERE product_id = $1 AND sku = $2",
        )
        .bind(Uuid::from(item.product_id))
        .bind(&item.sku)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if matches != 1 {
            return Err(invalid("Vendor return SKU must identify a variant of its product"));
        }
        if super::stock_move::warehouse_of(tx.as_mut(), item.source_location_id).await?
            != input.warehouse_id
        {
            return Err(invalid("Vendor return source location belongs to another warehouse"));
        }
        if let Some(lot) = item.lot_id {
            let matches = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*)::integer FROM lots WHERE id = $1 AND sku = $2",
            )
            .bind(lot)
            .bind(&item.sku)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if matches != 1 {
                return Err(invalid("Vendor return lot must belong to the line SKU"));
            }
        }
    }
    if total <= Decimal::ZERO {
        return Err(invalid("Vendor return must have a positive credit value"));
    }
    Ok(())
}
pub(super) async fn validate_credit_link(
    tx: &mut Transaction<'_, Postgres>,
    supplier: Uuid,
    return_id: Uuid,
    amount: Decimal,
    currency: CurrencyCode,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    let (expected_supplier, status, expected_currency) =
        sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT supplier_id, status, currency FROM vendor_returns WHERE id = $1 FOR UPDATE",
        )
        .bind(return_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| invalid("Vendor credit cites an unknown return"))?;
    if expected_supplier != supplier
        || status != "processed"
        || expected_currency != currency.to_string()
    {
        return Err(invalid(
            "Vendor credit requires a processed return with the same supplier and currency",
        ));
    }
    let exists = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*)::integer FROM vendor_credits WHERE vendor_return_id = $1",
    )
    .bind(return_id)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if exists != 0 {
        return Err(invalid("A vendor credit already exists for this return"));
    }
    let lines = sqlx::query_as::<_, (Decimal, Decimal)>(
        "SELECT quantity, unit_cost FROM vendor_return_items WHERE vendor_return_id = $1",
    )
    .bind(return_id)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let mut expected = Decimal::ZERO;
    for (quantity, cost) in lines {
        expected = expected
            .checked_add(line_value(quantity, cost, currency)?)
            .ok_or_else(|| invalid("Vendor return value overflow"))?;
    }
    if amount <= Decimal::ZERO || amount != expected {
        return Err(invalid("Vendor credit amount must equal its return value"));
    }
    Ok(())
}
pub(super) async fn process(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let (supplier, status, currency, warehouse) = sqlx::query_as::<_, (Uuid, String, String, i32)>("SELECT supplier_id, status, currency, COALESCE(warehouse_id, 0) FROM vendor_returns WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.ok_or_else(|| invalid("Unknown vendor return"))?;
    if status != "pending" {
        return Err(invalid("Only submitted vendor returns can be processed"));
    }
    let currency: CurrencyCode =
        currency.parse().map_err(|_| invalid("Invalid vendor return currency"))?;
    let lines = sqlx::query_as::<_, (String, Decimal, Decimal, i32, Option<Uuid>)>("SELECT sku, quantity, unit_cost, COALESCE(source_location_id, 0), lot_id FROM vendor_return_items WHERE vendor_return_id = $1 ORDER BY sku, source_location_id, id").bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    if lines.is_empty() {
        return Err(invalid("Vendor return has no stock lines"));
    }
    let mut total = Decimal::ZERO;
    for (sku, quantity, cost, source, lot_id) in lines {
        if sku.is_empty()
            || super::stock_move::warehouse_of(tx.as_mut(), source).await? != warehouse
        {
            return Err(invalid("Vendor return has missing or mismatched stock routing"));
        }
        total = total
            .checked_add(line_value(quantity, cost, currency)?)
            .ok_or_else(|| invalid("Vendor return value overflow"))?;
        let mut posting = StockMove {
            id: Uuid::new_v4(),
            sku: &sku,
            lot_id,
            from: Some(source),
            to: None,
            quantity,
            release_reserved: Decimal::ZERO,
            reserve_received: Decimal::ZERO,
            movement_type: MovementType::Shipment,
            lot_effect: LotEffect::Track,
            check_sellable: false,
            reference_type: Some("vendor_return"),
            reference_id: Some(id),
            reference_label: None,
            reason: Some("Return to supplier"),
            performed_by: None,
        };
        if let Some(lot_id) = lot_id {
            let lot = super::PgLotRepository::load_lot_on(tx, lot_id)
                .await?
                .ok_or_else(|| invalid("Unknown vendor return lot"))?;
            if lot.sku != sku {
                return Err(invalid("Vendor return lot belongs to another SKU"));
            }
            let held = sqlx::query_scalar::<_, Decimal>("SELECT quantity FROM lot_stock_holds WHERE owner_id = $1 AND lot_id = $1 AND location_id = $2 FOR UPDATE").bind(lot_id).bind(source).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?.unwrap_or_default();
            let release = held.min(quantity);
            let remaining = lot
                .quantity_remaining
                .checked_sub(quantity)
                .ok_or_else(|| invalid("Lot quantity overflow"))?;
            let quarantined = lot
                .quantity_quarantined
                .checked_sub(release)
                .ok_or_else(|| invalid("Lot quarantine quantity overflow"))?;
            if quarantined < Decimal::ZERO || remaining < lot.quantity_reserved + quarantined {
                return Err(invalid(
                    "Vendor return cannot consume another reservation's lot stock",
                ));
            }
            let next_status =
                if remaining.is_zero() { "consumed".to_owned() } else { lot.status.to_string() };
            sqlx::query("UPDATE lots SET quantity_remaining = $1, quantity_quarantined = $2, status = $3, updated_at = $4 WHERE id = $5").bind(remaining).bind(quarantined).bind(next_status).bind(now).bind(lot_id).execute(tx.as_mut()).await.map_err(map_db_error)?;
            super::stock_lot::record_hold(tx, lot_id, lot_id, source, -release, now).await?;
            posting.release_reserved = release;
            posting.lot_effect = LotEffect::MetadataRecorded;
            sqlx::query("INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type, reference_id, from_location_id, reason, created_at) VALUES ($1, $2, 'consumed', $3, 'vendor_return', $4, $5, 'Returned to supplier', $6)").bind(Uuid::new_v4()).bind(lot_id).bind(-quantity).bind(id).bind(warehouse).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
        }
        super::stock_move::post_from_slices(tx, &posting, now).await?;
    }
    if total <= Decimal::ZERO {
        return Err(invalid("Vendor return must have a positive credit value"));
    }
    sqlx::query("UPDATE vendor_returns SET status = 'processed', processed_at = $1, updated_at = $1 WHERE id = $2").bind(now).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    validate_credit_link(tx, supplier, id, total, currency, now).await?;
    let credit = Uuid::new_v4();
    sqlx::query("INSERT INTO vendor_credits (id, number, supplier_id, vendor_return_id, amount, remaining, currency, status, memo, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5, $6, 'open', 'Credit for returned stock', $7, $7)").bind(credit).bind(format!("VC-{}", credit.simple())).bind(supplier).bind(id).bind(total).bind(currency.to_string()).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    sqlx::query("UPDATE vendor_returns SET credit_generated = TRUE WHERE id = $1")
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    Ok(())
}
