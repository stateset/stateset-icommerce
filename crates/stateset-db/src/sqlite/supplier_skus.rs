//! SQLite implementation of the supplier SKU repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_opt_row, parse_enum_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    BulkSupplierSkuItem, CommerceError, CreateSupplierSku, CurrencyCode, Result, SupplierSku,
    SupplierSkuFilter, SupplierSkuId, SupplierSkuRepository, UpdateSupplierSku,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteSupplierSkuRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSupplierSkuRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_sku(row: &rusqlite::Row<'_>) -> rusqlite::Result<SupplierSku> {
        Ok(SupplierSku {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "supplier_sku", "id")?.into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "supplier_sku",
                "product_id",
            )?
            .into(),
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "supplier_sku",
                "supplier_id",
            )?,
            sku: row.get("sku")?,
            unit_cost: parse_decimal_opt_row(
                row.get::<_, Option<String>>("unit_cost")?,
                "supplier_sku",
                "unit_cost",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "supplier_sku",
                "currency",
            )?,
            min_order_qty: parse_decimal_opt_row(
                row.get::<_, Option<String>>("min_order_qty")?,
                "supplier_sku",
                "min_order_qty",
            )?,
            lead_time_days: row.get("lead_time_days")?,
            is_preferred: row.get::<_, i32>("is_preferred")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "supplier_sku",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "supplier_sku",
                "updated_at",
            )?,
        })
    }
}

impl SupplierSkuRepository for SqliteSupplierSkuRepository {
    fn create(&self, input: CreateSupplierSku) -> Result<SupplierSku> {
        let id = SupplierSkuId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO supplier_skus (id, product_id, supplier_id, sku, unit_cost, currency, min_order_qty, lead_time_days, is_preferred, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.product_id.to_string(),
                    input.supplier_id.to_string(),
                    &input.sku,
                    input.unit_cost.map(|c| c.to_string()),
                    currency.to_string(),
                    input.min_order_qty.map(|q| q.to_string()),
                    input.lead_time_days,
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row("SELECT * FROM supplier_skus WHERE id = ?", [&id_str], Self::row_to_sku)
        })
    }

    fn get(&self, id: SupplierSkuId) -> Result<Option<SupplierSku>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM supplier_skus WHERE id = ?",
            [id.to_string()],
            Self::row_to_sku,
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: SupplierSkuId, input: UpdateSupplierSku) -> Result<SupplierSku> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref sku) = input.sku {
                sets.push("sku = ?".into());
                params.push(Box::new(sku.clone()));
            }
            if let Some(cost) = input.unit_cost {
                sets.push("unit_cost = ?".into());
                params.push(Box::new(cost.to_string()));
            }
            if let Some(currency) = input.currency {
                sets.push("currency = ?".into());
                params.push(Box::new(currency.to_string()));
            }
            if let Some(min) = input.min_order_qty {
                sets.push("min_order_qty = ?".into());
                params.push(Box::new(min.to_string()));
            }
            if let Some(lead) = input.lead_time_days {
                sets.push("lead_time_days = ?".into());
                params.push(Box::new(lead));
            }
            if let Some(pref) = input.is_preferred {
                sets.push("is_preferred = ?".into());
                params.push(Box::new(pref as i32));
            }

            let sql = format!("UPDATE supplier_skus SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM supplier_skus WHERE id = ?", [&id_str], Self::row_to_sku)
        })
    }

    fn list(&self, filter: SupplierSkuFilter) -> Result<Vec<SupplierSku>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM supplier_skus WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(supplier) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params.push(Box::new(supplier.to_string()));
        }
        if let Some(product) = filter.product_id {
            sql.push_str(" AND product_id = ?");
            params.push(Box::new(product.to_string()));
        }
        sql.push_str(" ORDER BY is_preferred DESC, sku ASC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_sku)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: SupplierSkuId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM supplier_skus WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn bulk_upsert(&self, supplier_id: Uuid, items: Vec<BulkSupplierSkuItem>) -> Result<u64> {
        let supplier_str = supplier_id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut affected: u64 = 0;
            for item in &items {
                let id_str = SupplierSkuId::new().to_string();
                affected += tx.execute(
                    "INSERT INTO supplier_skus (id, product_id, supplier_id, sku, unit_cost, currency, is_preferred, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, 'USD', 0, ?, ?)
                     ON CONFLICT(product_id, supplier_id, sku) DO UPDATE SET
                        unit_cost = excluded.unit_cost,
                        updated_at = excluded.updated_at",
                    rusqlite::params![
                        &id_str,
                        item.product_id.to_string(),
                        &supplier_str,
                        &item.sku,
                        item.unit_cost.map(|c| c.to_string()),
                        &now_str,
                        &now_str,
                    ],
                )? as u64;
            }
            Ok(affected)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::ProductId;

    fn test_repo() -> SqliteSupplierSkuRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteSupplierSkuRepository::new(db.pool().clone())
    }

    fn new_sku(
        repo: &SqliteSupplierSkuRepository,
        supplier: Uuid,
        product: ProductId,
        sku: &str,
    ) -> SupplierSku {
        repo.create(CreateSupplierSku {
            product_id: product,
            supplier_id: supplier,
            sku: sku.into(),
            unit_cost: Some(dec!(9.99)),
            currency: Some(CurrencyCode::USD),
            min_order_qty: Some(dec!(100)),
            lead_time_days: Some(14),
        })
        .expect("create supplier sku")
    }

    #[test]
    fn create_get_update() {
        let repo = test_repo();
        let s = new_sku(&repo, Uuid::new_v4(), ProductId::new(), "ACME-1");
        assert_eq!(s.effective_cost(), dec!(9.99));
        let fetched = repo.get(s.id).expect("get").expect("found");
        assert_eq!(fetched.sku, "ACME-1");

        let updated = repo
            .update(
                s.id,
                UpdateSupplierSku {
                    unit_cost: Some(dec!(8.50)),
                    is_preferred: Some(true),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.unit_cost, Some(dec!(8.50)));
        assert!(updated.is_preferred);
    }

    #[test]
    fn list_filters_by_supplier_and_product() {
        let repo = test_repo();
        let supplier = Uuid::new_v4();
        let product = ProductId::new();
        new_sku(&repo, supplier, product, "A");
        new_sku(&repo, Uuid::new_v4(), ProductId::new(), "B");
        let by_supplier = repo
            .list(SupplierSkuFilter { supplier_id: Some(supplier), ..Default::default() })
            .expect("list");
        assert_eq!(by_supplier.len(), 1);
        let by_product = repo
            .list(SupplierSkuFilter { product_id: Some(product), ..Default::default() })
            .expect("list");
        assert_eq!(by_product.len(), 1);
    }

    #[test]
    fn bulk_upsert_inserts_then_updates() {
        let repo = test_repo();
        let supplier = Uuid::new_v4();
        let product = ProductId::new();
        let n = repo
            .bulk_upsert(
                supplier,
                vec![BulkSupplierSkuItem {
                    product_id: product,
                    sku: "X-1".into(),
                    unit_cost: Some(dec!(5)),
                }],
            )
            .expect("bulk insert");
        assert_eq!(n, 1);
        // upsert same triple updates cost
        repo.bulk_upsert(
            supplier,
            vec![BulkSupplierSkuItem {
                product_id: product,
                sku: "X-1".into(),
                unit_cost: Some(dec!(7)),
            }],
        )
        .expect("bulk update");
        let all = repo
            .list(SupplierSkuFilter { supplier_id: Some(supplier), ..Default::default() })
            .expect("list");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].unit_cost, Some(dec!(7)));
    }

    #[test]
    fn delete_removes() {
        let repo = test_repo();
        let s = new_sku(&repo, Uuid::new_v4(), ProductId::new(), "D");
        repo.delete(s.id).expect("delete");
        assert!(repo.get(s.id).expect("get").is_none());
    }
}
