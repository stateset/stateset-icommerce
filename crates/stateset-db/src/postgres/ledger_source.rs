//! Source document eligibility and subledger backlinks, in the posting transaction.
use super::map_db_error;
use stateset_core::CommerceError;
use uuid::Uuid;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn validate(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: Option<&str>,
    id: Option<Uuid>,
) -> stateset_core::Result<()> {
    let (Some(kind), Some(id)) = (kind, id) else {
        return Ok(());
    };
    match kind {
        "invoice" => {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM invoices WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if !["sent", "viewed", "partially_paid", "paid", "overdue", "written_off", "disputed"]
                .contains(&status.as_str())
            {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
            let invoice_type =
                sqlx::query_scalar::<_, String>("SELECT invoice_type FROM invoices WHERE id = $1")
                    .bind(id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            if invoice_type == "proforma" {
                return Err(invalid("Proforma invoices do not post to the ledger"));
            }
        }
        "refund" => {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM refunds WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if status != "completed" {
                return Err(invalid("Only completed refunds post to the ledger"));
            }
        }
        "payment" => {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM payments WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if !["completed", "partially_refunded", "refunded", "disputed"]
                .contains(&status.as_str())
            {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "bill" => {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM ap_bills WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if !["approved", "partially_paid", "paid", "overdue", "disputed"]
                .contains(&status.as_str())
            {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "bill_payment" => {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM ap_payments WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if !["processed", "cleared"].contains(&status.as_str()) {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "credit_memo" => {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM ar_credit_memos WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if !["open", "partially_applied", "fully_applied"].contains(&status.as_str()) {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "write_off" => {
            let reversed = sqlx::query_scalar::<_, Option<String>>(
                "SELECT CAST(reversed_at AS TEXT) FROM ar_write_offs WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if reversed.is_some() {
                return Err(invalid("Cannot post a reversed write-off"));
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) async fn link(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: Option<&str>,
    id: Option<Uuid>,
    entry: Uuid,
) -> stateset_core::Result<()> {
    let (Some(kind), Some(id)) = (kind, id) else {
        return Ok(());
    };
    match kind {
        "write_off" => {
            sqlx::query("UPDATE ar_write_offs SET gl_journal_entry_id = $1 WHERE id = $2")
                .bind(entry)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }
        "credit_memo" => {
            sqlx::query("UPDATE ar_credit_memos SET gl_journal_entry_id = $1 WHERE id = $2")
                .bind(entry)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }
        _ => {}
    }
    Ok(())
}
