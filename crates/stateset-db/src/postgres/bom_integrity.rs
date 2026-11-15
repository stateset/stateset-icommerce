//! BOM revisions and graph validation in the caller transaction.
use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use stateset_core::{BomStatus, CommerceError, UpdateBom};
use uuid::Uuid;
type DbResult<T> = stateset_core::Result<T>;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
fn conflict(message: &str) -> CommerceError {
    CommerceError::Conflict(message.into())
}
pub(super) async fn lock_graph(tx: &mut Transaction<'_, Postgres>) -> DbResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(42420017)")
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    Ok(())
}
pub(super) async fn editable(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM manufacturing_boms WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| invalid("Unknown BOM"))?;
    if status != "draft" {
        return Err(conflict("Published BOM revisions are immutable; create a new draft revision"));
    }
    Ok(())
}
pub(super) async fn validate_revision(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    let (product, revision) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT product_id, revision FROM manufacturing_boms WHERE id = $1",
    )
    .bind(id)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if revision.trim().is_empty() {
        return Err(invalid("BOM revision cannot be blank"));
    }
    let duplicates = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM manufacturing_boms WHERE product_id = $1 AND revision = $2 AND id != $3",
    )
    .bind(product)
    .bind(&revision)
    .bind(id)
    .fetch_all(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    if !duplicates.is_empty() {
        return Err(conflict(
            "A BOM revision already exists for this product; choose a new revision",
        ));
    }
    Ok(())
}
pub(super) async fn validate_graph(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let _ = now;
    let root: Uuid = sqlx::query_scalar("SELECT product_id FROM manufacturing_boms WHERE id = $1")
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
    let edges = sqlx::query_as::<_, (Uuid, Option<Uuid>, Option<Uuid>)>("SELECT b.product_id, c.component_product_id, v.product_id FROM manufacturing_bom_components c JOIN manufacturing_boms b ON b.id = c.bom_id LEFT JOIN product_variants v ON v.sku = c.component_sku WHERE b.status != 'obsolete'").fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
    crate::bom_integrity::graph(root, edges)
}
pub(super) async fn prepare_update(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    input: &UpdateBom,
    now: DateTime<Utc>,
) -> DbResult<()> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM manufacturing_boms WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| invalid("Unknown BOM"))?;
    let editing = input.name.is_some() || input.description.is_some() || input.revision.is_some();
    if status != "draft"
        && (editing || input.status.is_some_and(|target| target != BomStatus::Obsolete))
    {
        return Err(conflict("Published BOM revisions are immutable; create a new draft revision"));
    }
    if input.status == Some(BomStatus::Active) {
        let quantities = sqlx::query_scalar::<_, Decimal>(
            "SELECT quantity FROM manufacturing_bom_components WHERE bom_id = $1",
        )
        .bind(id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if quantities.iter().any(|quantity| *quantity <= Decimal::ZERO) {
            return Err(invalid("BOM component quantity must be positive"));
        }
        // Activating a revision retires the previous active definition; existing work orders retain their snapshot.
        sqlx::query("UPDATE manufacturing_boms SET status = 'obsolete', updated_at = $1 WHERE product_id = (SELECT product_id FROM manufacturing_boms WHERE id = $2) AND id != $2 AND status = 'active'").bind(now).bind(id).execute(tx.as_mut()).await.map_err(map_db_error)?;
    }
    Ok(())
}
