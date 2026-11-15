//! Source document eligibility and subledger backlinks, in the posting transaction.
use stateset_core::CommerceError;
use uuid::Uuid;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn validate(
    tx: &rusqlite::Connection,
    kind: Option<&str>,
    id: Option<Uuid>,
) -> rusqlite::Result<()> {
    let (Some(kind), Some(id)) = (kind, id) else {
        return Ok(());
    };
    match kind {
        "invoice" => {
            let status = tx.query_row(
                "SELECT status FROM invoices WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if !["sent", "viewed", "partially_paid", "paid", "overdue", "written_off", "disputed"]
                .contains(&status.as_str())
            {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
            let invoice_type = tx.query_row(
                "SELECT invoice_type FROM invoices WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if invoice_type == "proforma" {
                return Err(invalid("Proforma invoices do not post to the ledger"));
            }
        }
        "refund" => {
            let status = tx.query_row(
                "SELECT status FROM refunds WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if status != "completed" {
                return Err(invalid("Only completed refunds post to the ledger"));
            }
        }
        "payment" => {
            let status = tx.query_row(
                "SELECT status FROM payments WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if !["completed", "partially_refunded", "refunded", "disputed"]
                .contains(&status.as_str())
            {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "bill" => {
            let status = tx.query_row(
                "SELECT status FROM ap_bills WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if !["approved", "partially_paid", "paid", "overdue", "disputed"]
                .contains(&status.as_str())
            {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "bill_payment" => {
            let status = tx.query_row(
                "SELECT status FROM ap_payments WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if !["processed", "cleared"].contains(&status.as_str()) {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "credit_memo" => {
            let status = tx.query_row(
                "SELECT status FROM ar_credit_memos WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, String>(0),
            )?;
            if !["open", "partially_applied", "fully_applied"].contains(&status.as_str()) {
                return Err(invalid(&format!("Cannot post {kind} in status {status}")));
            }
        }
        "write_off" => {
            let reversed = tx.query_row(
                "SELECT CAST(reversed_at AS TEXT) FROM ar_write_offs WHERE id = ?1",
                rusqlite::params![(id).to_string()],
                |r| r.get::<_, Option<String>>(0),
            )?;
            if reversed.is_some() {
                return Err(invalid("Cannot post a reversed write-off"));
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn link(
    tx: &rusqlite::Connection,
    kind: Option<&str>,
    id: Option<Uuid>,
    entry: Uuid,
) -> rusqlite::Result<()> {
    let (Some(kind), Some(id)) = (kind, id) else {
        return Ok(());
    };
    match kind {
        "write_off" => {
            tx.execute(
                "UPDATE ar_write_offs SET gl_journal_entry_id = ?1 WHERE id = ?2",
                rusqlite::params![(entry).to_string(), (id).to_string()],
            )?;
        }
        "credit_memo" => {
            tx.execute(
                "UPDATE ar_credit_memos SET gl_journal_entry_id = ?1 WHERE id = ?2",
                rusqlite::params![(entry).to_string(), (id).to_string()],
            )?;
        }
        _ => {}
    }
    Ok(())
}
