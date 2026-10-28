//! Supplier credits and advances settle their target in the same transaction.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CurrencyCode};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(s: &str) -> CommerceError {
    CommerceError::ValidationError(s.into())
}
fn conflict(s: &str) -> CommerceError {
    CommerceError::Conflict(s.into())
}
pub(super) async fn apply(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: &str,
    target: Uuid,
    supplier: Uuid,
    currency: CurrencyCode,
    delta: Decimal,
) -> DbResult<()> {
    let (owner, denomination, status, total, paid) = match kind {
"bill" => sqlx::query_as::<_, (Uuid, String, String, Decimal, Decimal)>("SELECT supplier_id,currency,status,total_amount,amount_paid FROM ap_bills WHERE id = $1 FOR UPDATE").bind(target).fetch_one(tx.as_mut()).await.map_err(map_db_error)?,
"payment_obligation" => sqlx::query_as::<_, (Uuid, String, String, Decimal, Decimal)>("SELECT supplier_id,currency,status,amount,amount_paid FROM payment_obligations WHERE id = $1 FOR UPDATE").bind(target).fetch_one(tx.as_mut()).await.map_err(map_db_error)?,
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
            sqlx::query("UPDATE ap_bills SET amount_paid=$1, amount_due=$2, status=$3, updated_at=CURRENT_TIMESTAMP WHERE id=$4").bind(updated).bind(due).bind(next).bind(target).execute(tx.as_mut()).await.map_err(map_db_error)?;
        }
        _ => {
            sqlx::query("UPDATE payment_obligations SET amount_paid=$1, status=$2, updated_at=CURRENT_TIMESTAMP WHERE id=$3").bind(updated).bind(next).bind(target).execute(tx.as_mut()).await.map_err(map_db_error)?;
        }
    }
    Ok(())
}
pub(super) async fn bill_credits(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bill: Uuid,
) -> DbResult<Decimal> {
    let amounts = sqlx::query_scalar::<_, Decimal>("SELECT amount FROM prepayment_applications WHERE target_type='bill' AND target_id=$1 AND reversed=FALSE UNION ALL SELECT amount FROM vendor_credit_applications WHERE target_type='bill' AND target_id=$1 AND reversed=FALSE").bind(bill).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    amounts.into_iter().try_fold(Decimal::ZERO, |sum, n| {
        sum.checked_add(n).ok_or_else(|| invalid("Supplier credits overflow"))
    })
}

pub(super) async fn resolve_dispute(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bill: Uuid,
) -> DbResult<()> {
    let (status,previous,paid,due) = sqlx::query_as::<_, (String, Option<String>, Decimal, Decimal)>("SELECT status,disputed_from_status,amount_paid,amount_due FROM ap_bills WHERE id=$1 FOR UPDATE").bind(bill).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
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
    sqlx::query("UPDATE ap_bills SET status=$1, disputed_from_status=NULL,updated_at=CURRENT_TIMESTAMP WHERE id=$2").bind(restored).bind(bill).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
