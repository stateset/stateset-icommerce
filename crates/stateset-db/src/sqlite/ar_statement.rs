//! One consistent database snapshot supplies the full statement.
use crate::ar_statement::{Activity, Document};
use rust_decimal::Decimal;
use stateset_core::*;
use std::collections::BTreeMap;
use uuid::Uuid;
pub(super) fn generate(
    tx: &rusqlite::Transaction<'_>,
    request: GenerateStatementRequest,
) -> std::result::Result<CustomerStatement, rusqlite::Error> {
    let customer = request.customer_id;
    let (name, email) = tx.query_row(
        "SELECT first_name || ' ' || last_name,email FROM customers WHERE id=?1",
        rusqlite::params![(customer).to_string()],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
    )?;
    let default_currency = super::resolve_currency_in_tx(None, tx)?;
    let documents = {
        let mut stmt = tx.prepare("SELECT id,invoice_number,currency,total,COALESCE(sent_at,created_at),due_date,voided_at,direct_amount_paid FROM invoices WHERE customer_id=?1 AND status!='draft' AND (status NOT IN ('voided','cancelled') OR sent_at IS NOT NULL)")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                super::parse_decimal_row(&r.get::<_, String>(7)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let documents = documents
        .into_iter()
        .map(|(id, number, currency, total, issued, due, voided, direct)| Document {
            id,
            number,
            currency,
            total,
            issued,
            due,
            voided,
            direct,
        })
        .collect();
    let mut activity = Vec::new();
    let mut direct_totals: BTreeMap<Uuid, Decimal> = BTreeMap::new();
    let rows = {
        let mut stmt = tx.prepare("SELECT pa.invoice_id,pa.applied_date,p.payment_number,pa.applied_amount FROM ar_payment_applications pa JOIN payments p ON p.id=pa.payment_id JOIN invoices i ON i.id=pa.invoice_id WHERE i.customer_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount: -amount,
            kind: StatementTransactionType::Payment,
            description: "Payment application".into(),
        });
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT pa.invoice_id,pa.applied_date,p.payment_number,pa.applied_amount FROM ar_reversed_payment_applications pa JOIN payments p ON p.id=pa.payment_id JOIN invoices i ON i.id=pa.invoice_id WHERE i.customer_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount: -amount,
            kind: StatementTransactionType::Payment,
            description: "Payment application".into(),
        });
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT pa.invoice_id,pa.reversed_at,p.payment_number,pa.applied_amount FROM ar_reversed_payment_applications pa JOIN payments p ON p.id=pa.payment_id JOIN invoices i ON i.id=pa.invoice_id WHERE i.customer_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount,
            kind: StatementTransactionType::Payment,
            description: "Payment application reversed".into(),
        });
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT dp.invoice_id,dp.recorded_at,dp.payment_id,dp.amount FROM invoice_direct_payments dp JOIN invoices i ON i.id=dp.invoice_id WHERE i.customer_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        *direct_totals.entry(invoice).or_default() += amount;
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount: -amount,
            kind: StatementTransactionType::Payment,
            description: "Direct payment".into(),
        });
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT ca.invoice_id,ca.applied_date,cm.credit_memo_number,ca.applied_amount FROM ar_credit_memo_applications ca JOIN ar_credit_memos cm ON cm.id=ca.credit_memo_id JOIN invoices i ON i.id=ca.invoice_id WHERE i.customer_id=?1 AND cm.status!='voided'")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount: -amount,
            kind: StatementTransactionType::CreditMemo,
            description: "Credit memo".into(),
        });
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT wo.invoice_id,wo.created_at,wo.write_off_number,wo.amount FROM ar_write_offs wo JOIN invoices i ON i.id=wo.invoice_id WHERE i.customer_id=?1")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount: -amount,
            kind: StatementTransactionType::WriteOff,
            description: "Write-off".into(),
        });
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT wo.invoice_id,wo.reversed_at,wo.write_off_number,wo.amount FROM ar_write_offs wo JOIN invoices i ON i.id=wo.invoice_id WHERE i.customer_id=?1 AND wo.reversed_at IS NOT NULL")?;
        let rows = stmt.query_map(rusqlite::params![(customer).to_string()], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (invoice, date, reference, amount) in rows {
        activity.push(Activity {
            invoice,
            date,
            reference,
            amount,
            kind: StatementTransactionType::WriteOff,
            description: "Write-off reversed".into(),
        });
    }
    crate::ar_statement::build(
        request,
        name,
        email,
        default_currency,
        documents,
        activity,
        direct_totals,
    )
    .map_err(super::bins::smuggle)
}
