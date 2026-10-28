//! Payment allocation locks and validates the funding source before any invoice.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(s: &str) -> CommerceError {
    CommerceError::ValidationError(s.into())
}
fn conflict(s: &str) -> CommerceError {
    CommerceError::Conflict(s.into())
}
pub(super) async fn apply_payment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &ApplyPaymentToInvoices,
) -> DbResult<Vec<ArPaymentApplication>> {
    if input.applications.is_empty() {
        return Ok(Vec::new());
    }
    let (amount, refunded, status, customer, currency) = sqlx::query_as::<_, (Decimal, Decimal, String, Option<Uuid>, String)>("SELECT amount,amount_refunded,status,customer_id,currency FROM payments WHERE id=$1 FOR UPDATE").bind(input.payment_id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if !matches!(status.as_str(), "completed" | "partially_refunded") {
        return Err(conflict("Only successfully captured funds can be applied to invoices"));
    }
    let customer = customer
        .ok_or_else(|| invalid("Invoice application requires an identified payment customer"))?;
    if refunded < Decimal::ZERO || refunded > amount {
        return Err(invalid("Invalid refunded payment balance"));
    }
    let available = amount - refunded;
    let applied = sqlx::query_scalar::<_, Decimal>(
        "SELECT applied_amount FROM ar_payment_applications WHERE payment_id=$1",
    )
    .bind(input.payment_id)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let sum = |a: Decimal, b: Decimal| {
        a.checked_add(b).ok_or_else(|| invalid("Payment allocation overflow"))
    };
    let existing = applied.into_iter().try_fold(Decimal::ZERO, sum)?;
    let mut requested = Decimal::ZERO;
    for app in &input.applications {
        if app.amount <= Decimal::ZERO {
            return Err(invalid("Payment application amount must be positive"));
        }
        requested = sum(requested, app.amount)?;
    }
    if sum(existing, requested)? > available {
        return Err(invalid("Applications exceed captured payment less refunds"));
    }
    let mut ordered: Vec<_> = input.applications.iter().enumerate().collect();
    ordered.sort_by_key(|(_, app)| app.invoice_id);
    let now = chrono::Utc::now();
    let mut result = Vec::new();
    for (index, app) in ordered {
        let (owner, status, denomination, due) = sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT customer_id,status,currency,balance_due FROM invoices WHERE id=$1 FOR UPDATE").bind(app.invoice_id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
        if !matches!(status.as_str(), "sent" | "viewed" | "partially_paid" | "overdue" | "disputed")
        {
            return Err(conflict(&format!(
                "Invoice {} is {status}; it must be issued and open to receive a payment application",
                app.invoice_id
            )));
        }
        if owner != customer {
            return Err(invalid("Payment and invoice must belong to the same customer"));
        }
        if denomination != currency {
            return Err(invalid("Payment and invoice currencies must match"));
        }
        if app.amount > due {
            return Err(invalid("Payment application exceeds the invoice balance"));
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO ar_payment_applications (id,payment_id,invoice_id,applied_amount,applied_date,created_at) VALUES ($1,$2,$3,$4,$5,$5)").bind(id).bind(input.payment_id).bind(app.invoice_id).bind(app.amount).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
        super::accounts_receivable::PgAccountsReceivableRepository::recalculate_invoice_tx(
            tx,
            app.invoice_id,
        )
        .await?;
        result.push((
            index,
            ArPaymentApplication {
                id,
                payment_id: input.payment_id,
                invoice_id: app.invoice_id,
                applied_amount: app.amount,
                applied_date: now,
                created_at: now,
            },
        ));
    }
    result.sort_by_key(|(index, _)| *index);
    Ok(result.into_iter().map(|(_, app)| app).collect())
}
pub(super) async fn unapply_payment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    application_id: Uuid,
) -> DbResult<()> {
    let (payment_id, invoice_id) = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT payment_id,invoice_id FROM ar_payment_applications WHERE id=$1",
    )
    .bind(application_id)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    // Keep lock order identical to application: payment, invoice, then application.
    let _payment = sqlx::query_scalar::<_, Uuid>("SELECT id FROM payments WHERE id=$1 FOR UPDATE")
        .bind(payment_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let status =
        sqlx::query_scalar::<_, String>("SELECT status FROM invoices WHERE id=$1 FOR UPDATE")
            .bind(invoice_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
    if matches!(status.as_str(), "voided" | "cancelled" | "written_off") {
        return Err(conflict(
            "Reverse the invoice write-off before unapplying payments; terminal invoices cannot be changed",
        ));
    }
    let _application = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM ar_payment_applications WHERE id=$1 FOR UPDATE",
    )
    .bind(application_id)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    sqlx::query("INSERT INTO ar_reversed_payment_applications(id,payment_id,invoice_id,applied_amount,applied_date,reversed_at) SELECT id,payment_id,invoice_id,applied_amount,applied_date,$2 FROM ar_payment_applications WHERE id=$1").bind(application_id).bind(chrono::Utc::now()).execute(tx.as_mut()).await.map_err(map_db_error)?;
    sqlx::query("DELETE FROM ar_payment_applications WHERE id=$1")
        .bind(application_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    super::accounts_receivable::PgAccountsReceivableRepository::recalculate_invoice_tx(
        tx, invoice_id,
    )
    .await?;
    Ok(())
}

/// Refunds and allocations serialize on the payment parent. An operator must
/// release allocated funds before refunding them, preserving invoice history.
pub(super) async fn require_unallocated(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    payment: Uuid,
    remaining: Decimal,
) -> DbResult<()> {
    let amounts = sqlx::query_scalar::<_, Decimal>(
        "SELECT applied_amount FROM ar_payment_applications WHERE payment_id=$1",
    )
    .bind(payment)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let applied = amounts.into_iter().try_fold(Decimal::ZERO, |sum, n| {
        sum.checked_add(n).ok_or_else(|| invalid("Payment application overflow"))
    })?;
    if applied > remaining {
        return Err(conflict("Unapply invoice allocations before refunding their funds"));
    }
    Ok(())
}
