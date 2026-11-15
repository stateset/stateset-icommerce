//! BOM revisions and graph validation in the caller transaction.
use rusqlite::{OptionalExtension, Transaction};
use rust_decimal::Decimal;
use stateset_core::{BomStatus, CommerceError, UpdateBom};
use uuid::Uuid;
type DbResult<T> = rusqlite::Result<T>;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
fn conflict(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::Conflict(message.into()))
}
pub(super) fn editable(tx: &Transaction<'_>, id: Uuid, now: &str) -> DbResult<()> {
    let _ = now;
    let status = tx
        .query_row(
            "SELECT status FROM manufacturing_boms WHERE id = ?1",
            rusqlite::params![(id).to_string()],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| invalid("Unknown BOM"))?;
    if status != "draft" {
        return Err(conflict("Published BOM revisions are immutable; create a new draft revision"));
    }
    Ok(())
}
pub(super) fn validate_revision(tx: &Transaction<'_>, id: Uuid, now: &str) -> DbResult<()> {
    let _ = now;
    let (product, revision) = tx.query_row(
        "SELECT product_id, revision FROM manufacturing_boms WHERE id = ?1",
        rusqlite::params![(id).to_string()],
        |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                r.get::<_, String>(1)?,
            ))
        },
    )?;
    if revision.trim().is_empty() {
        return Err(invalid("BOM revision cannot be blank"));
    }
    let duplicates = {
        let mut stmt = tx.prepare("SELECT id FROM manufacturing_boms WHERE product_id = ?1 AND revision = ?2 AND id != ?3")?;
        let rows = stmt.query_map(
            rusqlite::params![(product).to_string(), &revision, (id).to_string()],
            |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id"),
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if !duplicates.is_empty() {
        return Err(conflict(
            "A BOM revision already exists for this product; choose a new revision",
        ));
    }
    Ok(())
}
pub(super) fn validate_graph(tx: &Transaction<'_>, id: Uuid, now: &str) -> DbResult<()> {
    let _ = now;
    let root = tx.query_row(
        "SELECT product_id FROM manufacturing_boms WHERE id = ?1",
        [id.to_string()],
        |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "bom", "product_id"),
    )?;
    let edges = {
        let mut stmt = tx.prepare("SELECT b.product_id, c.component_product_id, v.product_id FROM manufacturing_bom_components c JOIN manufacturing_boms b ON b.id = c.bom_id LEFT JOIN product_variants v ON v.sku = c.component_sku WHERE b.status != 'obsolete'")?;
        let rows = stmt.query_map(rusqlite::params![], |r| {
            Ok((
                super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")?,
                super::parse_uuid_opt_row(r.get(1)?, "stock", "id")?,
                super::parse_uuid_opt_row(r.get(2)?, "stock", "id")?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    crate::bom_integrity::graph(root, edges).map_err(super::bins::smuggle)
}
pub(super) fn prepare_update(
    tx: &Transaction<'_>,
    id: Uuid,
    input: &UpdateBom,
    now: &str,
) -> DbResult<()> {
    let status = tx
        .query_row(
            "SELECT status FROM manufacturing_boms WHERE id = ?1",
            rusqlite::params![(id).to_string()],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| invalid("Unknown BOM"))?;
    let editing = input.name.is_some() || input.description.is_some() || input.revision.is_some();
    if status != "draft"
        && (editing || input.status.is_some_and(|target| target != BomStatus::Obsolete))
    {
        return Err(conflict("Published BOM revisions are immutable; create a new draft revision"));
    }
    if input.status == Some(BomStatus::Active) {
        let quantities = {
            let mut stmt =
                tx.prepare("SELECT quantity FROM manufacturing_bom_components WHERE bom_id = ?1")?;
            let rows = stmt.query_map(rusqlite::params![(id).to_string()], |r| {
                super::parse_decimal_row(&r.get::<_, String>(0)?, "stock", "quantity")
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        if quantities.iter().any(|quantity| *quantity <= Decimal::ZERO) {
            return Err(invalid("BOM component quantity must be positive"));
        }
        // Activating a revision retires the previous active definition; existing work orders retain their snapshot.
        tx.execute("UPDATE manufacturing_boms SET status = 'obsolete', updated_at = ?1 WHERE product_id = (SELECT product_id FROM manufacturing_boms WHERE id = ?2) AND id != ?2 AND status = 'active'", rusqlite::params![now, (id).to_string()])?;
    }
    Ok(())
}
