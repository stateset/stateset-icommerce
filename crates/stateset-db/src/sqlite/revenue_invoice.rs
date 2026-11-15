//! Revenue contracts must own a matching, not-yet-posted invoice.
use rusqlite::Transaction;
use rust_decimal::Decimal;
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(stateset_core::CommerceError::ValidationError(message.into()))
}
pub(super) fn validate_link(
    tx: &Transaction<'_>,
    contract: Uuid,
    invoice: Uuid,
    customer: Uuid,
    price: Decimal,
    currency: &str,
) -> DbResult<()> {
    let source = tx.query_row("SELECT customer_id, total, tax_amount, shipping_amount, currency, status FROM invoices WHERE id = ?1", rusqlite::params![(invoice).to_string()], |r| Ok((super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?, super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?, super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?, r.get::<_, String>(4)?, r.get::<_, String>(5)?)))?;
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
    let other = tx.query_row(
        "SELECT COUNT(*) FROM revenue_contracts WHERE invoice_id = ?1 AND id != ?2",
        rusqlite::params![(invoice).to_string(), (contract).to_string()],
        |r| r.get::<_, i32>(0),
    )?;
    if other != 0 {
        return Err(invalid("Invoice already belongs to a revenue contract"));
    }
    let posted = tx.query_row("SELECT COUNT(*) FROM gl_journal_entries WHERE source_document_type = 'invoice' AND source_document_id = ?1 AND status != 'voided'", rusqlite::params![(invoice).to_string()], |r| r.get::<_, i32>(0))?;
    if posted != 0 {
        return Err(invalid("Link the revenue contract before posting its invoice"));
    }
    Ok(())
}
pub(super) fn is_deferred(tx: &Transaction<'_>, invoice: Uuid) -> DbResult<bool> {
    let rows = {
        let mut stmt = tx.prepare("SELECT c.customer_id, c.transaction_price, c.currency, c.status, i.customer_id, i.total, i.tax_amount, i.shipping_amount, i.currency FROM revenue_contracts c JOIN invoices i ON i.id = c.invoice_id WHERE c.invoice_id = ?1")?;
        let rows = stmt.query_map(rusqlite::params![(invoice).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                super::parse_uuid_row(&r.get::<_, String>(4)?, "stock", "id")?,
                super::parse_decimal_row(&r.get::<_, String>(5)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(6)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(7)?, "stock", "quantity")?,
                r.get::<_, String>(8)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
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
pub(super) fn require_funded(
    tx: &Transaction<'_>,
    obligation: Uuid,
    deferred: Uuid,
) -> DbResult<()> {
    let invoices = tx.query_row("SELECT c.invoice_id FROM revenue_contracts c JOIN performance_obligations o ON o.contract_id = c.id WHERE o.id = ?1", rusqlite::params![(obligation).to_string()], |r| super::parse_uuid_opt_row(r.get(0)?, "stock", "id"))?;
    if let Some(invoice) = invoices {
        let funded = tx.query_row("SELECT COUNT(*) FROM gl_journal_entries e JOIN gl_journal_entry_lines l ON l.journal_entry_id = e.id WHERE e.source_document_type = 'invoice' AND e.source_document_id = ?1 AND e.status = 'posted' AND l.account_id = ?2 AND CAST(l.credit_amount AS NUMERIC) > 0", rusqlite::params![(invoice).to_string(), (deferred).to_string()], |r| r.get::<_, i32>(0))?;
        if funded == 0 {
            return Err(invalid(
                "Post the linked invoice to unearned revenue before recognizing it",
            ));
        }
    }
    Ok(())
}
