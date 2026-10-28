//! Vendor returns post stock and their linked supplier credit atomically.
use crate::stock_move::{LotEffect, StockMove};
use rusqlite::{Connection, OptionalExtension};
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateVendorReturn, CurrencyCode, MovementType};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
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
pub(super) fn validate_create(
    tx: &Connection,
    input: &CreateVendorReturn,
    currency: CurrencyCode,
    now: &str,
) -> DbResult<()> {
    let _ = now;
    if input.items.is_empty() {
        return Err(invalid("A vendor return requires at least one item"));
    }
    let supplier = tx.query_row(
        "SELECT COUNT(*) FROM suppliers WHERE id = ?1",
        rusqlite::params![(input.supplier_id).to_string()],
        |r| r.get::<_, i32>(0),
    )?;
    if supplier != 1 {
        return Err(invalid("Unknown vendor return supplier"));
    }
    if let Some(po) = input.purchase_order_id {
        let source = tx
            .query_row(
                "SELECT supplier_id, currency FROM purchase_orders WHERE id = ?1",
                rusqlite::params![(po).to_string()],
                |r| {
                    Ok((
                        super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                        r.get::<_, String>(1)?,
                    ))
                },
            )
            .optional()?
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
        let matches = tx.query_row(
            "SELECT COUNT(*) FROM product_variants WHERE product_id = ?1 AND sku = ?2",
            rusqlite::params![(Uuid::from(item.product_id)).to_string(), &item.sku],
            |r| r.get::<_, i32>(0),
        )?;
        if matches != 1 {
            return Err(invalid("Vendor return SKU must identify a variant of its product"));
        }
        if super::stock_move::warehouse_of(tx, item.source_location_id)? != input.warehouse_id {
            return Err(invalid("Vendor return source location belongs to another warehouse"));
        }
        if let Some(lot) = item.lot_id {
            let matches = tx.query_row(
                "SELECT COUNT(*) FROM lots WHERE id = ?1 AND sku = ?2",
                rusqlite::params![(lot).to_string(), &item.sku],
                |r| r.get::<_, i32>(0),
            )?;
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
pub(super) fn validate_credit_link(
    tx: &Connection,
    supplier: Uuid,
    return_id: Uuid,
    amount: Decimal,
    currency: CurrencyCode,
    now: &str,
) -> DbResult<()> {
    let _ = now;
    let (expected_supplier, status, expected_currency) = tx
        .query_row(
            "SELECT supplier_id, status, currency FROM vendor_returns WHERE id = ?1",
            rusqlite::params![(return_id).to_string()],
            |r| {
                Ok((
                    super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| invalid("Vendor credit cites an unknown return"))?;
    if expected_supplier != supplier
        || status != "processed"
        || expected_currency != currency.to_string()
    {
        return Err(invalid(
            "Vendor credit requires a processed return with the same supplier and currency",
        ));
    }
    let exists = tx.query_row(
        "SELECT COUNT(*) FROM vendor_credits WHERE vendor_return_id = ?1",
        rusqlite::params![(return_id).to_string()],
        |r| r.get::<_, i32>(0),
    )?;
    if exists != 0 {
        return Err(invalid("A vendor credit already exists for this return"));
    }
    let lines = {
        let mut stmt = tx.prepare(
            "SELECT quantity, unit_cost FROM vendor_return_items WHERE vendor_return_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![(return_id).to_string()], |r| {
            Ok((
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
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
pub(super) fn process(tx: &Connection, id: Uuid, now: &str) -> DbResult<()> {
    let (supplier, status, currency, warehouse) = tx.query_row("SELECT supplier_id, status, currency, COALESCE(warehouse_id, 0) FROM vendor_returns WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i32>(3)?))).optional()?.ok_or_else(|| invalid("Unknown vendor return"))?;
    if status != "pending" {
        return Err(invalid("Only submitted vendor returns can be processed"));
    }
    let currency: CurrencyCode =
        currency.parse().map_err(|_| invalid("Invalid vendor return currency"))?;
    let lines = {
        let mut stmt = tx.prepare("SELECT sku, quantity, unit_cost, COALESCE(source_location_id, 0), lot_id FROM vendor_return_items WHERE vendor_return_id = ?1 ORDER BY sku, source_location_id, id")?;
        let rows = stmt.query_map(rusqlite::params![(id).to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                r.get::<_, i32>(3)?,
                super::parse_uuid_opt_row(r.get(4)?, "stock", "id")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if lines.is_empty() {
        return Err(invalid("Vendor return has no stock lines"));
    }
    let mut total = Decimal::ZERO;
    for (sku, quantity, cost, source, lot_id) in lines {
        if sku.is_empty() || super::stock_move::warehouse_of(tx, source)? != warehouse {
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
            let lot = super::SqliteLotRepository::load_lot_on(tx, lot_id)
                .map_err(super::bins::smuggle)?
                .ok_or_else(|| invalid("Unknown vendor return lot"))?;
            if lot.sku != sku {
                return Err(invalid("Vendor return lot belongs to another SKU"));
            }
            let held = tx.query_row("SELECT quantity FROM lot_stock_holds WHERE owner_id = ?1 AND lot_id = ?1 AND location_id = ?2", rusqlite::params![(lot_id).to_string(), source], |r| super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")).optional()?.unwrap_or_default();
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
            tx.execute("UPDATE lots SET quantity_remaining = ?1, quantity_quarantined = ?2, status = ?3, updated_at = ?4 WHERE id = ?5", rusqlite::params![(remaining).to_string(), (quarantined).to_string(), next_status, now, (lot_id).to_string()])?;
            super::stock_lot::record_hold(tx, lot_id, lot_id, source, -release, now)?;
            posting.release_reserved = release;
            posting.lot_effect = LotEffect::MetadataRecorded;
            tx.execute("INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type, reference_id, from_location_id, reason, created_at) VALUES (?1, ?2, 'consumed', ?3, 'vendor_return', ?4, ?5, 'Returned to supplier', ?6)", rusqlite::params![(Uuid::new_v4()).to_string(), (lot_id).to_string(), (-quantity).to_string(), (id).to_string(), warehouse, now])?;
        }
        super::stock_move::post_from_slices(tx, &posting, now)?;
    }
    if total <= Decimal::ZERO {
        return Err(invalid("Vendor return must have a positive credit value"));
    }
    tx.execute("UPDATE vendor_returns SET status = 'processed', processed_at = ?1, updated_at = ?1 WHERE id = ?2", rusqlite::params![now, (id).to_string()])?;
    validate_credit_link(tx, supplier, id, total, currency, now)?;
    let credit = Uuid::new_v4();
    tx.execute("INSERT INTO vendor_credits (id, number, supplier_id, vendor_return_id, amount, remaining, currency, status, memo, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, 'open', 'Credit for returned stock', ?7, ?7)", rusqlite::params![(credit).to_string(), format!("VC-{}", credit.simple()), (supplier).to_string(), (id).to_string(), (total).to_string(), currency.to_string(), now])?;
    tx.execute(
        "UPDATE vendor_returns SET credit_generated = 1 WHERE id = ?1",
        rusqlite::params![(id).to_string()],
    )?;
    Ok(())
}
