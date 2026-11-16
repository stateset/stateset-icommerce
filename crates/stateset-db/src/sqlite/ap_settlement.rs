//! Supplier credits and advances settle their target in the same transaction.
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CurrencyCode};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(s: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(s.into()))
}
fn conflict(s: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::Conflict(s.into()))
}
pub(super) fn apply(
    tx: &rusqlite::Connection,
    kind: &str,
    target: Uuid,
    supplier: Uuid,
    currency: CurrencyCode,
    delta: Decimal,
) -> DbResult<()> {
    let (owner, denomination, status, total, paid) = match kind {
"bill" => tx.query_row("SELECT supplier_id,currency,status,total_amount,amount_paid FROM ap_bills WHERE id = ?1", rusqlite::params![(target).to_string()], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(4)?, "stock", "quantity")?)))?,
"payment_obligation" => tx.query_row("SELECT supplier_id,currency,status,amount,amount_paid FROM payment_obligations WHERE id = ?1", rusqlite::params![(target).to_string()], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(4)?, "stock", "quantity")?)))?,
_ => return Err(invalid("Unsupported supplier settlement target")),
};
    if owner != supplier {
        return Err(invalid("Settlement supplier must match its target"));
    }
    if denomination != currency.as_str() {
        return Err(invalid("Settlement currency must match its target"));
    }
    if (kind == "bill"
        && !matches!(status.as_str(), "approved" | "partially_paid" | "overdue" | "paid"))
        || (kind == "payment_obligation"
            && !matches!(status.as_str(), "pending" | "scheduled" | "partially_paid" | "paid"))
    {
        return Err(conflict("Target is not eligible for settlement or reversal"));
    }
    let updated = paid.checked_add(delta).ok_or_else(|| invalid("Settlement amount overflow"))?;
    if delta.is_zero() || updated < Decimal::ZERO || updated > total {
        return Err(invalid("Settlement exceeds the target balance"));
    }
    let due = total - updated;
    let next = if due.is_zero() {
        "paid"
    } else if updated > Decimal::ZERO {
        "partially_paid"
    } else if kind == "bill" {
        if status == "overdue" { "overdue" } else { "approved" }
    } else if status == "scheduled" {
        "scheduled"
    } else {
        "pending"
    };
    match kind {
        "bill" => {
            tx.execute("UPDATE ap_bills SET amount_paid=?1, amount_due=?2, status=?3, updated_at=CURRENT_TIMESTAMP WHERE id=?4", rusqlite::params![(updated).to_string(), (due).to_string(), next, (target).to_string()])?;
        }
        _ => {
            tx.execute("UPDATE payment_obligations SET amount_paid=?1, status=?2, updated_at=CURRENT_TIMESTAMP WHERE id=?3", rusqlite::params![(updated).to_string(), next, (target).to_string()])?;
        }
    }
    Ok(())
}
pub(super) fn bill_credits(tx: &rusqlite::Connection, bill: Uuid) -> DbResult<Decimal> {
    let amounts = {
        let mut stmt = tx.prepare("SELECT amount FROM prepayment_applications WHERE target_type='bill' AND target_id=?1 AND reversed=0 UNION ALL SELECT amount FROM vendor_credit_applications WHERE target_type='bill' AND target_id=?1 AND reversed=0")?;
        let rows = stmt.query_map(rusqlite::params![(bill).to_string()], |r| {
            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    amounts.into_iter().try_fold(Decimal::ZERO, |sum, n| {
        sum.checked_add(n).ok_or_else(|| invalid("Supplier credits overflow"))
    })
}

pub(super) fn resolve_dispute(tx: &rusqlite::Connection, bill: Uuid) -> DbResult<()> {
    let (status, previous, paid, due) = tx.query_row(
        "SELECT status,disputed_from_status,amount_paid,amount_due FROM ap_bills WHERE id=?1",
        rusqlite::params![(bill).to_string()],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        },
    )?;
    if status != "disputed" {
        return Err(conflict("Bill is not disputed"));
    }
    let previous = previous
        .ok_or_else(|| conflict("Legacy bill dispute requires prior-state reconciliation"))?;
    if !matches!(previous.as_str(), "draft" | "pending" | "approved" | "partially_paid" | "overdue")
    {
        return Err(conflict("Invalid saved bill dispute state"));
    }
    let restored = if due.is_zero() {
        "paid"
    } else if paid > Decimal::ZERO {
        "partially_paid"
    } else if previous == "partially_paid" {
        "approved"
    } else {
        &previous
    };
    tx.execute("UPDATE ap_bills SET status=?1, disputed_from_status=NULL,updated_at=CURRENT_TIMESTAMP WHERE id=?2", rusqlite::params![restored, (bill).to_string()])?;
    Ok(())
}
