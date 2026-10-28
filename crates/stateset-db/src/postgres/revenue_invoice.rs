//! Revenue contracts must own a matching, not-yet-posted invoice.
use super::map_db_error;
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> stateset_core::CommerceError {
    stateset_core::CommerceError::ValidationError(message.into())
}
pub(super) async fn validate_link(
    tx: &mut Transaction<'_, Postgres>,
    contract: Uuid,
    invoice: Uuid,
    customer: Uuid,
    price: Decimal,
    currency: &str,
) -> DbResult<()> {
    let source = sqlx::query_as::<_, (Uuid, Decimal, Decimal, Decimal, String, String)>("SELECT customer_id, total, tax_amount, shipping_amount, currency, status FROM invoices WHERE id = $1 FOR UPDATE").bind(invoice).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if source.0 != customer
        || source.4 != currency
        || source.1.checked_sub(source.2).and_then(|v| v.checked_sub(source.3)) != Some(price)
    {
        return Err(invalid(
            "Revenue contract customer, currency and price must match the invoice net of tax and shipping",
        ));
    }
    if matches!(source.5.as_str(), "voided" | "cancelled" | "written_off") {
        return Err(invalid("Revenue contract requires a live invoice"));
    }
    let other = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*)::INTEGER FROM revenue_contracts WHERE invoice_id = $1 AND id != $2",
    )
    .bind(invoice)
    .bind(contract)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if other != 0 {
        return Err(invalid("Invoice already belongs to a revenue contract"));
    }
    let posted = sqlx::query_scalar::<_, i32>("SELECT COUNT(*)::INTEGER FROM gl_journal_entries WHERE source_document_type = 'invoice' AND source_document_id = $1 AND status != 'voided'").bind(invoice).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if posted != 0 {
        return Err(invalid("Link the revenue contract before posting its invoice"));
    }
    Ok(())
}
pub(super) async fn is_deferred(
    tx: &mut Transaction<'_, Postgres>,
    invoice: Uuid,
) -> DbResult<bool> {
    let rows = sqlx::query_as::<_, (Uuid, Decimal, String, String, Uuid, Decimal, Decimal, Decimal, String)>("SELECT c.customer_id, c.transaction_price, c.currency, c.status, i.customer_id, i.total, i.tax_amount, i.shipping_amount, i.currency FROM revenue_contracts c JOIN invoices i ON i.id = c.invoice_id WHERE c.invoice_id = $1").bind(invoice).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    if rows.len() > 1 {
        return Err(invalid("Invoice has multiple revenue contracts; reconcile before posting"));
    }
    let Some(c) = rows.first() else {
        return Ok(false);
    };
    if c.3 == "cancelled" {
        return Err(invalid("Cannot post an invoice linked to a cancelled revenue contract"));
    }
    if c.0 != c.4
        || c.2 != c.8
        || c.5.checked_sub(c.6).and_then(|v| v.checked_sub(c.7)) != Some(c.1)
    {
        return Err(invalid("Invoice no longer matches its revenue contract"));
    }
    Ok(true)
}
pub(super) async fn require_funded(
    tx: &mut Transaction<'_, Postgres>,
    obligation: Uuid,
    deferred: Uuid,
) -> DbResult<()> {
    let invoices = sqlx::query_scalar::<_, Option<Uuid>>("SELECT c.invoice_id FROM revenue_contracts c JOIN performance_obligations o ON o.contract_id = c.id WHERE o.id = $1").bind(obligation).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if let Some(invoice) = invoices {
        let funded = sqlx::query_scalar::<_, i32>("SELECT COUNT(*)::INTEGER FROM gl_journal_entries e JOIN gl_journal_entry_lines l ON l.journal_entry_id = e.id WHERE e.source_document_type = 'invoice' AND e.source_document_id = $1 AND e.status = 'posted' AND l.account_id = $2 AND l.credit_amount > 0").bind(invoice).bind(deferred).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
        if funded == 0 {
            return Err(invalid(
                "Post the linked invoice to unearned revenue before recognizing it",
            ));
        }
    }
    Ok(())
}
