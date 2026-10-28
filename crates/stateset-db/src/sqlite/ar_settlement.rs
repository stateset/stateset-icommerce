//! Payment allocation locks and validates the funding source before any invoice.
use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(s: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(s.into()))
}
fn conflict(s: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::Conflict(s.into()))
}
pub(super) fn apply_payment(
    tx: &rusqlite::Transaction<'_>,
    input: &ApplyPaymentToInvoices,
) -> DbResult<Vec<ArPaymentApplication>> {
    if input.applications.is_empty() {
        return Ok(Vec::new());
    }
    let (amount, refunded, status, customer, currency) = tx.query_row(
        "SELECT amount,amount_refunded,status,customer_id,currency FROM payments WHERE id=?1",
        rusqlite::params![(input.payment_id).to_string()],
        |r| {
            Ok((
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                r.get::<_, String>(2)?,
                super::parse_uuid_opt_row(r.get(3)?, "stock", "id")?,
                r.get::<_, String>(4)?,
            ))
        },
    )?;
    if !matches!(status.as_str(), "completed" | "partially_refunded") {
        return Err(conflict("Only successfully captured funds can be applied to invoices"));
    }
    let customer = customer
        .ok_or_else(|| invalid("Invoice application requires an identified payment customer"))?;
    if refunded < Decimal::ZERO || refunded > amount {
        return Err(invalid("Invalid refunded payment balance"));
    }
    let available = amount - refunded;
    let applied = {
        let mut stmt =
            tx.prepare("SELECT applied_amount FROM ar_payment_applications WHERE payment_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(input.payment_id).to_string()], |r| {
            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
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
        let (owner, status, denomination, due) = tx.query_row(
            "SELECT customer_id,status,currency,balance_due FROM invoices WHERE id=?1",
            rusqlite::params![(app.invoice_id).to_string()],
            |r| {
                Ok((
                    super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                ))
            },
        )?;
        if !matches!(status.as_str(), "sent" | "viewed" | "partially_paid" | "overdue" | "disputed")
        {
            return Err(conflict(
                "Invoice must be issued and open to receive a payment application",
            ));
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
        tx.execute("INSERT INTO ar_payment_applications (id,payment_id,invoice_id,applied_amount,applied_date,created_at) VALUES (?1,?2,?3,?4,?5,?5)", rusqlite::params![(id).to_string(), (input.payment_id).to_string(), (app.invoice_id).to_string(), (app.amount).to_string(), now.to_rfc3339()])?;
        super::accounts_receivable::SqliteAccountsReceivableRepository::recalculate_invoice_with_conn(tx, app.invoice_id.into()).map_err(super::bins::smuggle)?;
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
pub(super) fn unapply_payment(
    tx: &rusqlite::Transaction<'_>,
    application_id: Uuid,
) -> DbResult<()> {
    let (payment_id, invoice_id) = tx.query_row(
        "SELECT payment_id,invoice_id FROM ar_payment_applications WHERE id=?1",
        rusqlite::params![(application_id).to_string()],
        |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_uuid_row(&r.get::<_, String>(1)?, "stock", "id")?,
            ))
        },
    )?;
    // Keep lock order identical to application: payment, invoice, then application.
    let _payment = tx.query_row(
        "SELECT id FROM payments WHERE id=?1",
        rusqlite::params![(payment_id).to_string()],
        |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
    )?;
    let status = tx.query_row(
        "SELECT status FROM invoices WHERE id=?1",
        rusqlite::params![(invoice_id).to_string()],
        |r| r.get::<_, String>(0),
    )?;
    if matches!(status.as_str(), "voided" | "cancelled" | "written_off") {
        return Err(conflict(
            "Reverse the invoice write-off before unapplying payments; terminal invoices cannot be changed",
        ));
    }
    let _application = tx.query_row(
        "SELECT id FROM ar_payment_applications WHERE id=?1",
        rusqlite::params![(application_id).to_string()],
        |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
    )?;
    tx.execute("INSERT INTO ar_reversed_payment_applications(id,payment_id,invoice_id,applied_amount,applied_date,reversed_at) SELECT id,payment_id,invoice_id,applied_amount,applied_date,?2 FROM ar_payment_applications WHERE id=?1", rusqlite::params![(application_id).to_string(), chrono::Utc::now().to_rfc3339()])?;
    tx.execute(
        "DELETE FROM ar_payment_applications WHERE id=?1",
        rusqlite::params![(application_id).to_string()],
    )?;
    super::accounts_receivable::SqliteAccountsReceivableRepository::recalculate_invoice_with_conn(
        tx,
        invoice_id.into(),
    )
    .map_err(super::bins::smuggle)?;
    Ok(())
}

/// Refunds and allocations serialize on the payment parent. An operator must
/// release allocated funds before refunding them, preserving invoice history.
pub(super) fn require_unallocated(
    tx: &rusqlite::Transaction<'_>,
    payment: Uuid,
    remaining: Decimal,
) -> DbResult<()> {
    let amounts = {
        let mut stmt =
            tx.prepare("SELECT applied_amount FROM ar_payment_applications WHERE payment_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(payment).to_string()], |r| {
            super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let applied = amounts.into_iter().try_fold(Decimal::ZERO, |sum, n| {
        sum.checked_add(n).ok_or_else(|| invalid("Payment application overflow"))
    })?;
    if applied > remaining {
        return Err(conflict("Unapply invoice allocations before refunding their funds"));
    }
    Ok(())
}
