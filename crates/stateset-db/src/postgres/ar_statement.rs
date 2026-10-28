//! One consistent database snapshot supplies the full statement.
use super::map_db_error;
use crate::ar_statement::{Activity, Document};
use rust_decimal::Decimal;
use stateset_core::*;
use std::collections::BTreeMap;
use uuid::Uuid;
pub(super) async fn generate(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request: GenerateStatementRequest,
) -> std::result::Result<CustomerStatement, CommerceError> {
    let customer = request.customer_id;
    let (name, email) = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT first_name || ' ' || last_name,email FROM customers WHERE id=$1",
    )
    .bind(customer)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    let default_currency = super::resolve_currency_with_executor(None, tx.as_mut()).await?;
    let documents=sqlx::query_as::<_, (Uuid, String, String, Decimal, String, String, Option<String>, Decimal)>("SELECT id,invoice_number,currency,total,to_char((COALESCE(sent_at,created_at)) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),to_char((due_date) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),to_char((voided_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),direct_amount_paid FROM invoices WHERE customer_id=$1 AND status!='draft' AND (status NOT IN ('voided','cancelled') OR sent_at IS NOT NULL)").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT pa.invoice_id,to_char((pa.applied_date) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),p.payment_number,pa.applied_amount FROM ar_payment_applications pa JOIN payments p ON p.id=pa.payment_id JOIN invoices i ON i.id=pa.invoice_id WHERE i.customer_id=$1").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT pa.invoice_id,to_char((pa.applied_date) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),p.payment_number,pa.applied_amount FROM ar_reversed_payment_applications pa JOIN payments p ON p.id=pa.payment_id JOIN invoices i ON i.id=pa.invoice_id WHERE i.customer_id=$1").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT pa.invoice_id,to_char((pa.reversed_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),p.payment_number,pa.applied_amount FROM ar_reversed_payment_applications pa JOIN payments p ON p.id=pa.payment_id JOIN invoices i ON i.id=pa.invoice_id WHERE i.customer_id=$1").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT dp.invoice_id,to_char((dp.recorded_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),dp.payment_id::text,dp.amount FROM invoice_direct_payments dp JOIN invoices i ON i.id=dp.invoice_id WHERE i.customer_id=$1").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT ca.invoice_id,to_char((ca.applied_date) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),cm.credit_memo_number,ca.applied_amount FROM ar_credit_memo_applications ca JOIN ar_credit_memos cm ON cm.id=ca.credit_memo_id JOIN invoices i ON i.id=ca.invoice_id WHERE i.customer_id=$1 AND cm.status!='voided'").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT wo.invoice_id,to_char((wo.created_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),wo.write_off_number,wo.amount FROM ar_write_offs wo JOIN invoices i ON i.id=wo.invoice_id WHERE i.customer_id=$1").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
    let rows=sqlx::query_as::<_, (Uuid, String, String, Decimal)>("SELECT wo.invoice_id,to_char((wo.reversed_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),wo.write_off_number,wo.amount FROM ar_write_offs wo JOIN invoices i ON i.id=wo.invoice_id WHERE i.customer_id=$1 AND wo.reversed_at IS NOT NULL").bind(customer).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
}
