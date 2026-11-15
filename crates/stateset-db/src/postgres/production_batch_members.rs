//! Relational production-batch membership, guarded in the caller transaction.
use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use stateset_core::{CommerceError, ProductionBatchStatus};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn guard(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    planned_only: bool,
    now: DateTime<Utc>,
) -> DbResult<String> {
    let _ = now;
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM production_batches WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| invalid("Unknown production batch"))?;
    // Lock members while checking edits, so a concurrent release cannot cross the guard.
    let _members = sqlx::query_scalar::<_, Uuid>("SELECT w.id FROM manufacturing_work_orders w JOIN production_batch_work_orders l ON l.work_order_id = w.id WHERE l.batch_id = $1 ORDER BY w.id FOR SHARE OF w").bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    let (status, unresolved) = sqlx::query_as::<_, (String, i32)>("SELECT status, CASE WHEN unresolved_links THEN 1 ELSE 0 END FROM production_batch_state WHERE id = $1").bind(id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
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
pub(super) async fn add(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    members: &[Uuid],
    now: DateTime<Utc>,
) -> DbResult<()> {
    for member in members {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM manufacturing_work_orders WHERE id = $1 FOR SHARE",
        )
        .bind(*member)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| invalid("Production batch cites an unknown work order"))?;
        sqlx::query("INSERT INTO production_batch_work_orders (batch_id, work_order_id) VALUES ($1, $2) ON CONFLICT(batch_id, work_order_id) DO NOTHING").bind(id).bind(*member).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    sync(tx, id, now).await?;
    Ok(())
}
pub(super) async fn sync(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    sqlx::query("UPDATE production_batches SET work_order_ids = COALESCE((SELECT jsonb_agg(work_order_id::text ORDER BY work_order_id) FROM production_batch_work_orders WHERE batch_id = $1), '[]'::jsonb), updated_at = $2 WHERE id = $1").bind(id).bind(now).execute(tx.as_mut()).await.map_err(map_db_error)?;
    Ok(())
}
pub(super) async fn update_status(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    desired: Option<ProductionBatchStatus>,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let current = guard(tx, id, false, now).await?;
    if let Some(desired) = desired {
        if desired.to_string() != current {
            if desired != ProductionBatchStatus::Cancelled {
                return Err(invalid("Batch status is derived from its work orders"));
            }
            let statuses = sqlx::query_scalar::<_, String>("SELECT w.status FROM manufacturing_work_orders w JOIN production_batch_work_orders l ON l.work_order_id = w.id WHERE l.batch_id = $1").bind(id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
            if statuses.iter().any(|status| status != "cancelled") {
                return Err(invalid("Cancel the member work orders before cancelling their batch"));
            }
            sqlx::query(
                "UPDATE production_batches SET status = 'cancelled', updated_at = $1 WHERE id = $2",
            )
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
    }
    Ok(())
}
