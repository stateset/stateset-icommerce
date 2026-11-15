//! SQLite implementation of the production batch repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_enum_row, parse_json_row,
    parse_uuid_opt_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateProductionBatch, ProductionBatch, ProductionBatchFilter,
    ProductionBatchId, ProductionBatchRepository, ProductionBatchStatus, Result,
    UpdateProductionBatch,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteProductionBatchRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteProductionBatchRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_batch(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProductionBatch> {
        if row.get::<_, bool>("unresolved_links")? {
            return Err(super::bins::smuggle(CommerceError::ValidationError(
                "Legacy production batch has missing work-order links; reconcile before reading"
                    .into(),
            )));
        }
        let work_order_ids_json: String = row.get("work_order_ids")?;
        Ok(ProductionBatch {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "production_batch", "id")?.into(),
            name: row.get("name")?,
            status: parse_enum_row::<ProductionBatchStatus>(
                &row.get::<_, String>("status")?,
                "production_batch",
                "status",
            )?,
            vendor_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("vendor_id")?,
                "production_batch",
                "vendor_id",
            )?,
            work_order_ids: parse_json_row(
                &work_order_ids_json,
                "production_batch",
                "work_order_ids",
            )?,
            notes: row.get("notes")?,
            scheduled_start: parse_datetime_opt_row(
                row.get::<_, Option<String>>("scheduled_start")?,
                "production_batch",
                "scheduled_start",
            )?,
            scheduled_end: parse_datetime_opt_row(
                row.get::<_, Option<String>>("scheduled_end")?,
                "production_batch",
                "scheduled_end",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "production_batch",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "production_batch",
                "updated_at",
            )?,
        })
    }
}

impl ProductionBatchRepository for SqliteProductionBatchRepository {
    fn create(&self, input: CreateProductionBatch) -> Result<ProductionBatch> {
        let id = ProductionBatchId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let work_order_ids_json = serde_json::to_string(&input.work_order_ids)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO production_batches (id, name, status, vendor_id, work_order_ids, notes, scheduled_start, scheduled_end, created_at, updated_at)
                 VALUES (?, ?, 'planned', ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    input.vendor_id.map(|v| v.to_string()),
                    &work_order_ids_json,
                    &input.notes,
                    input.scheduled_start.map(|d| d.to_rfc3339()),
                    input.scheduled_end.map(|d| d.to_rfc3339()),
                    &now_str,
                    &now_str,
                ],
            )?;
            super::production_batch_members::add(tx, id.into(), &input.work_order_ids, &now_str)?;
            tx.query_row(
                "SELECT * FROM production_batch_state WHERE id = ?",
                [&id_str],
                Self::row_to_batch,
            )
        })
    }

    fn get(&self, id: ProductionBatchId) -> Result<Option<ProductionBatch>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM production_batch_state WHERE id = ?",
            [id.to_string()],
            Self::row_to_batch,
        ) {
            Ok(b) => Ok(Some(b)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(
        &self,
        id: ProductionBatchId,
        input: UpdateProductionBatch,
    ) -> Result<ProductionBatch> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            super::production_batch_members::update_status(tx, id.into(), input.status, &now_str)?;
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(vendor) = input.vendor_id {
                sets.push("vendor_id = ?".into());
                params.push(Box::new(vendor.to_string()));
            }

            if let Some(ref notes) = input.notes {
                sets.push("notes = ?".into());
                params.push(Box::new(notes.clone()));
            }
            if let Some(start) = input.scheduled_start {
                sets.push("scheduled_start = ?".into());
                params.push(Box::new(start.to_rfc3339()));
            }
            if let Some(end) = input.scheduled_end {
                sets.push("scheduled_end = ?".into());
                params.push(Box::new(end.to_rfc3339()));
            }

            let sql = format!("UPDATE production_batches SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row(
                "SELECT * FROM production_batch_state WHERE id = ?",
                [&id_str],
                Self::row_to_batch,
            )
        })
    }

    fn list(&self, filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM production_batch_state WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(vendor) = filter.vendor_id {
            sql.push_str(" AND vendor_id = ?");
            params.push(Box::new(vendor.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_batch)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: ProductionBatchId) -> Result<()> {
        with_immediate_transaction(&self.pool, |tx| {
            super::production_batch_members::guard(tx, id.into(), true, &Utc::now().to_rfc3339())?;
            tx.execute("DELETE FROM production_batches WHERE id = ?1", [id.to_string()])?;
            Ok(())
        })
    }

    fn add_work_orders(
        &self,
        id: ProductionBatchId,
        work_order_ids: Vec<Uuid>,
    ) -> Result<ProductionBatch> {
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            super::production_batch_members::guard(tx, id.into(), true, &now)?;
            super::production_batch_members::add(tx, id.into(), &work_order_ids, &now)?;
            tx.query_row(
                "SELECT * FROM production_batch_state WHERE id = ?1",
                [id.to_string()],
                Self::row_to_batch,
            )
        })
    }

    fn remove_work_order(
        &self,
        id: ProductionBatchId,
        work_order_id: Uuid,
    ) -> Result<ProductionBatch> {
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            super::production_batch_members::guard(tx, id.into(), true, &now)?;
            tx.execute("DELETE FROM production_batch_work_orders WHERE batch_id = ?1 AND work_order_id = ?2", rusqlite::params![id.to_string(), work_order_id.to_string()])?;
            super::production_batch_members::sync(tx, id.into(), &now)?;
            tx.query_row(
                "SELECT * FROM production_batch_state WHERE id = ?1",
                [id.to_string()],
                Self::row_to_batch,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteProductionBatchRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteProductionBatchRepository::new(db.pool().clone())
    }

    fn new_batch(repo: &SqliteProductionBatchRepository, name: &str) -> ProductionBatch {
        repo.create(CreateProductionBatch {
            name: name.into(),
            vendor_id: None,
            work_order_ids: vec![],
            notes: None,
            scheduled_start: None,
            scheduled_end: None,
        })
        .expect("create batch")
    }

    #[test]
    fn create_get_update() {
        let repo = test_repo();
        let b = new_batch(&repo, "Batch A");
        assert_eq!(b.status, ProductionBatchStatus::Planned);
        let updated = repo
            .update(
                b.id,
                UpdateProductionBatch { name: Some("Edited".into()), ..Default::default() },
            )
            .expect("update");
        assert_eq!(updated.name, "Edited");
        assert_eq!(updated.status, ProductionBatchStatus::Planned);
    }

    #[test]
    fn add_and_remove_work_orders_idempotent() {
        let repo = test_repo();
        let b = new_batch(&repo, "Batch B");
        use stateset_core::{ProductRepository, WorkOrderRepository};
        let products = super::super::SqliteProductRepository::new(repo.pool.clone());
        let product = products
            .create(stateset_core::CreateProduct {
                name: "Batch product".into(),
                ..Default::default()
            })
            .unwrap();
        let orders = super::super::SqliteWorkOrderRepository::new(repo.pool.clone());
        let input = || stateset_core::CreateWorkOrder {
            product_id: product.id,
            quantity_to_build: rust_decimal::Decimal::ONE,
            ..Default::default()
        };
        let wo1 = orders.create(input()).unwrap().id;
        let wo2 = orders.create(input()).unwrap().id;
        let updated = repo.add_work_orders(b.id, vec![wo1, wo2, wo1]).expect("add");
        assert_eq!(updated.work_order_count(), 2);
        // adding wo1 again is a no-op
        let again = repo.add_work_orders(b.id, vec![wo1]).expect("add again");
        assert_eq!(again.work_order_count(), 2);
        let removed = repo.remove_work_order(b.id, wo1).expect("remove");
        assert_eq!(removed.work_order_count(), 1);
        assert_eq!(removed.work_order_ids, vec![wo2]);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = test_repo();
        let a = new_batch(&repo, "A");
        new_batch(&repo, "B");
        repo.update(
            a.id,
            UpdateProductionBatch {
                status: Some(ProductionBatchStatus::Cancelled),
                ..Default::default()
            },
        )
        .expect("update");
        let completed = repo
            .list(ProductionBatchFilter {
                status: Some(ProductionBatchStatus::Cancelled),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn delete_removes_batch() {
        let repo = test_repo();
        let b = new_batch(&repo, "Doomed");
        repo.delete(b.id).expect("delete");
        assert!(repo.get(b.id).expect("get").is_none());
    }
}
