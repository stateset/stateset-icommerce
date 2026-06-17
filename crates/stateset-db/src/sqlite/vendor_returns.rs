//! SQLite implementation of the vendor return repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_uuid_opt_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    CommerceError, CreateVendorReturn, CurrencyCode, Result, VendorReturn, VendorReturnFilter,
    VendorReturnId, VendorReturnItem, VendorReturnItemId, VendorReturnReason, VendorReturnStatus,
};

#[derive(Debug)]
pub struct SqliteVendorReturnRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteVendorReturnRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<VendorReturnItem> {
        Ok(VendorReturnItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "vendor_return_item", "id")?.into(),
            vendor_return_id: parse_uuid_row(
                &row.get::<_, String>("vendor_return_id")?,
                "vendor_return_item",
                "vendor_return_id",
            )?
            .into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "vendor_return_item",
                "product_id",
            )?
            .into(),
            sku: row.get("sku")?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "vendor_return_item",
                "quantity",
            )?,
            unit_cost: parse_decimal_row(
                &row.get::<_, String>("unit_cost")?,
                "vendor_return_item",
                "unit_cost",
            )?,
            reason: parse_enum_row::<VendorReturnReason>(
                &row.get::<_, String>("reason")?,
                "vendor_return_item",
                "reason",
            )?,
        })
    }

    fn row_to_head(row: &rusqlite::Row<'_>) -> rusqlite::Result<VendorReturn> {
        Ok(VendorReturn {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "vendor_return", "id")?.into(),
            number: row.get("number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "vendor_return",
                "supplier_id",
            )?,
            purchase_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("purchase_order_id")?,
                "vendor_return",
                "purchase_order_id",
            )?,
            status: parse_enum_row::<VendorReturnStatus>(
                &row.get::<_, String>("status")?,
                "vendor_return",
                "status",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "vendor_return",
                "currency",
            )?,
            items: Vec::new(),
            credit_generated: row.get::<_, i32>("credit_generated")? != 0,
            notes: row.get("notes")?,
            processed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("processed_at")?,
                "vendor_return",
                "processed_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "vendor_return",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "vendor_return",
                "updated_at",
            )?,
        })
    }

    fn load_items(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<Vec<VendorReturnItem>> {
        let mut stmt = conn
            .prepare("SELECT * FROM vendor_return_items WHERE vendor_return_id = ? ORDER BY sku")?;
        stmt.query_map([id], Self::row_to_item)?.collect()
    }

    fn load_full(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<VendorReturn> {
        let mut head =
            conn.query_row("SELECT * FROM vendor_returns WHERE id = ?", [id], Self::row_to_head)?;
        head.items = Self::load_items(conn, id)?;
        Ok(head)
    }

    /// Guard a status transition, returning the current status or `NoRows`.
    fn current_status(tx: &rusqlite::Connection, id: &str) -> rusqlite::Result<VendorReturnStatus> {
        let s: Option<String> = tx
            .query_row("SELECT status FROM vendor_returns WHERE id = ?", [id], |r| r.get(0))
            .optional()?;
        let s = s.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        parse_enum_row::<VendorReturnStatus>(&s, "vendor_return", "status")
    }

    fn conflict(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict(msg.to_string())))
    }
}

impl stateset_core::VendorReturnRepository for SqliteVendorReturnRepository {
    fn create(&self, input: CreateVendorReturn) -> Result<VendorReturn> {
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "a vendor return requires at least one item".into(),
            ));
        }
        let id = VendorReturnId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let number = format!("VR-{}", &id_str[..8]);
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO vendor_returns (id, number, supplier_id, purchase_order_id, status, currency, credit_generated, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 'draft', ?, 0, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &number,
                    input.supplier_id.to_string(),
                    input.purchase_order_id.map(|p| p.to_string()),
                    currency.to_string(),
                    &input.notes,
                    &now_str,
                    &now_str,
                ],
            )?;
            for item in &input.items {
                tx.execute(
                    "INSERT INTO vendor_return_items (id, vendor_return_id, product_id, sku, quantity, unit_cost, reason)
                     VALUES (?, ?, ?, '', ?, ?, ?)",
                    rusqlite::params![
                        VendorReturnItemId::new().to_string(),
                        &id_str,
                        item.product_id.to_string(),
                        item.quantity.to_string(),
                        item.unit_cost.to_string(),
                        item.reason.to_string(),
                    ],
                )?;
            }
            Self::load_full(tx, &id_str)
        })
    }

    fn get(&self, id: VendorReturnId) -> Result<Option<VendorReturn>> {
        let conn = self.conn()?;
        match Self::load_full(&conn, &id.to_string()) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: VendorReturnFilter) -> Result<Vec<VendorReturn>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM vendor_returns WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(supplier) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params.push(Box::new(supplier.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let heads = stmt
            .query_map(param_refs.as_slice(), Self::row_to_head)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        let mut out = Vec::with_capacity(heads.len());
        for mut head in heads {
            head.items = Self::load_items(&conn, &head.id.to_string()).map_err(map_db_error)?;
            out.push(head);
        }
        Ok(out)
    }

    fn submit(&self, id: VendorReturnId) -> Result<VendorReturn> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            if Self::current_status(tx, &id_str)? != VendorReturnStatus::Draft {
                return Err(Self::conflict("only draft vendor returns can be submitted"));
            }
            tx.execute(
                "UPDATE vendor_returns SET status = 'pending', updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn process(&self, id: VendorReturnId, generate_credit: bool) -> Result<VendorReturn> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let status = Self::current_status(tx, &id_str)?;
            if status.is_terminal() {
                return Err(Self::conflict("vendor return is already in a terminal state"));
            }
            tx.execute(
                "UPDATE vendor_returns SET status = 'processed', credit_generated = ?, processed_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![generate_credit as i32, &now, &now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn cancel(&self, id: VendorReturnId) -> Result<VendorReturn> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            if Self::current_status(tx, &id_str)? == VendorReturnStatus::Processed {
                return Err(Self::conflict("processed vendor returns cannot be cancelled"));
            }
            tx.execute(
                "UPDATE vendor_returns SET status = 'cancelled', updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CreateVendorReturnItem, ProductId, VendorReturnRepository};
    use uuid::Uuid;

    fn test_repo() -> SqliteVendorReturnRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteVendorReturnRepository::new(db.pool().clone())
    }

    fn new_return(repo: &SqliteVendorReturnRepository) -> VendorReturn {
        repo.create(CreateVendorReturn {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            currency: Some(CurrencyCode::USD),
            items: vec![CreateVendorReturnItem {
                product_id: ProductId::new(),
                quantity: dec!(3),
                unit_cost: dec!(10),
                reason: VendorReturnReason::Defective,
            }],
            notes: Some("defective batch".into()),
        })
        .expect("create vendor return")
    }

    #[test]
    fn create_get_with_items() {
        let repo = test_repo();
        let r = new_return(&repo);
        assert_eq!(r.status, VendorReturnStatus::Draft);
        assert_eq!(r.items.len(), 1);
        let fetched = repo.get(r.id).expect("get").expect("found");
        assert_eq!(fetched.total_credit(), dec!(30));
    }

    #[test]
    fn create_rejects_empty_items() {
        let repo = test_repo();
        let res = repo.create(CreateVendorReturn {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            currency: None,
            items: vec![],
            notes: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn submit_then_process_generates_credit() {
        let repo = test_repo();
        let r = new_return(&repo);
        let submitted = repo.submit(r.id).expect("submit");
        assert_eq!(submitted.status, VendorReturnStatus::Pending);
        let processed = repo.process(r.id, true).expect("process");
        assert_eq!(processed.status, VendorReturnStatus::Processed);
        assert!(processed.credit_generated);
        assert!(processed.processed_at.is_some());
    }

    #[test]
    fn submit_twice_conflicts() {
        let repo = test_repo();
        let r = new_return(&repo);
        repo.submit(r.id).expect("submit");
        assert!(repo.submit(r.id).is_err());
    }

    #[test]
    fn processed_cannot_be_cancelled() {
        let repo = test_repo();
        let r = new_return(&repo);
        repo.process(r.id, false).expect("process");
        assert!(repo.cancel(r.id).is_err());
    }

    #[test]
    fn list_filters_by_status() {
        let repo = test_repo();
        let a = new_return(&repo);
        new_return(&repo);
        repo.cancel(a.id).expect("cancel");
        let cancelled = repo
            .list(VendorReturnFilter {
                status: Some(VendorReturnStatus::Cancelled),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(cancelled.len(), 1);
    }
}
