//! Relational production-batch membership, guarded in the caller transaction.
use rusqlite::{OptionalExtension, Transaction};
use stateset_core::{CommerceError, ProductionBatchStatus};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn guard(
    tx: &Transaction<'_>,
    id: Uuid,
    planned_only: bool,
    now: &str,
) -> DbResult<String> {
    let _ = now;
    tx.query_row(
        "SELECT id FROM production_batches WHERE id = ?1",
        rusqlite::params![(id).to_string()],
        |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
    )
    .optional()?
    .ok_or_else(|| invalid("Unknown production batch"))?;
    // Lock members while checking edits, so a concurrent release cannot cross the guard.
    let _members = {
        let mut stmt = tx.prepare("SELECT w.id FROM manufacturing_work_orders w JOIN production_batch_work_orders l ON l.work_order_id = w.id WHERE l.batch_id = ?1 ORDER BY w.id")?;
        let rows = stmt.query_map(rusqlite::params![(id).to_string()], |r| {
            super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let (status, unresolved) = tx.query_row("SELECT status, CASE WHEN unresolved_links THEN 1 ELSE 0 END FROM production_batch_state WHERE id = ?1", rusqlite::params![(id).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?)))?;
    if unresolved != 0 {
        return Err(invalid(
            "Legacy production batch has missing work-order links; reconcile before editing",
        ));
    }
    if planned_only && status != "planned" {
        return Err(invalid("Batch membership can change only before production starts"));
    }
    Ok(status)
}
pub(super) fn add(tx: &Transaction<'_>, id: Uuid, members: &[Uuid], now: &str) -> DbResult<()> {
    for member in members {
        tx.query_row(
            "SELECT id FROM manufacturing_work_orders WHERE id = ?1",
            rusqlite::params![(*member).to_string()],
            |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
        )
        .optional()?
        .ok_or_else(|| invalid("Production batch cites an unknown work order"))?;
        tx.execute("INSERT INTO production_batch_work_orders (batch_id, work_order_id) VALUES (?1, ?2) ON CONFLICT(batch_id, work_order_id) DO NOTHING", rusqlite::params![(id).to_string(), (*member).to_string()])?;
    }
    sync(tx, id, now)?;
    Ok(())
}
pub(super) fn sync(tx: &Transaction<'_>, id: Uuid, now: &str) -> DbResult<()> {
    tx.execute("UPDATE production_batches SET work_order_ids = (SELECT json_group_array(work_order_id) FROM production_batch_work_orders WHERE batch_id = ?1), updated_at = ?2 WHERE id = ?1", rusqlite::params![(id).to_string(), now])?;
    Ok(())
}
pub(super) fn update_status(
    tx: &Transaction<'_>,
    id: Uuid,
    desired: Option<ProductionBatchStatus>,
    now: &str,
) -> DbResult<()> {
    let current = guard(tx, id, false, now)?;
    if let Some(desired) = desired {
        if desired.to_string() != current {
            if desired != ProductionBatchStatus::Cancelled {
                return Err(invalid("Batch status is derived from its work orders"));
            }
            let statuses = {
                let mut stmt = tx.prepare("SELECT w.status FROM manufacturing_work_orders w JOIN production_batch_work_orders l ON l.work_order_id = w.id WHERE l.batch_id = ?1")?;
                let rows =
                    stmt.query_map(rusqlite::params![(id).to_string()], |r| r.get::<_, String>(0))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            if statuses.iter().any(|status| status != "cancelled") {
                return Err(invalid("Cancel the member work orders before cancelling their batch"));
            }
            tx.execute(
                "UPDATE production_batches SET status = 'cancelled', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, (id).to_string()],
            )?;
        }
    }
    Ok(())
}
