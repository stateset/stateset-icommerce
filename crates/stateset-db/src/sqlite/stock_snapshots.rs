//! SQLite implementation of the stock snapshot repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_uuid_opt_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CaptureStockSnapshot, CommerceError, Result, StockSnapshot, StockSnapshotFilter,
    StockSnapshotId, StockSnapshotLine, StockSnapshotLineId, StockSnapshotRepository,
};

#[derive(Debug)]
pub struct SqliteStockSnapshotRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteStockSnapshotRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_line(row: &rusqlite::Row<'_>) -> rusqlite::Result<StockSnapshotLine> {
        Ok(StockSnapshotLine {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "stock_snapshot_line", "id")?.into(),
            stock_snapshot_id: parse_uuid_row(
                &row.get::<_, String>("stock_snapshot_id")?,
                "stock_snapshot_line",
                "stock_snapshot_id",
            )?
            .into(),
            product_id: parse_uuid_opt_row(
                row.get("product_id")?,
                "stock_snapshot_line",
                "product_id",
            )?
            .map(Into::into),
            sku: row.get("sku")?,
            quantity_on_hand: parse_decimal_row(
                &row.get::<_, String>("quantity_on_hand")?,
                "stock_snapshot_line",
                "quantity_on_hand",
            )?,
            quantity_available: parse_decimal_row(
                &row.get::<_, String>("quantity_available")?,
                "stock_snapshot_line",
                "quantity_available",
            )?,
            location: row.get("location")?,
        })
    }

    fn row_to_head(row: &rusqlite::Row<'_>) -> rusqlite::Result<StockSnapshot> {
        Ok(StockSnapshot {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "stock_snapshot", "id")?.into(),
            label: row.get("label")?,
            total_skus: row.get::<_, i64>("total_skus")? as u64,
            total_units: parse_decimal_row(
                &row.get::<_, String>("total_units")?,
                "stock_snapshot",
                "total_units",
            )?,
            lines: Vec::new(),
            captured_at: parse_datetime_row(
                &row.get::<_, String>("captured_at")?,
                "stock_snapshot",
                "captured_at",
            )?,
        })
    }

    fn load_lines(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<Vec<StockSnapshotLine>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM stock_snapshot_lines WHERE stock_snapshot_id = ? ORDER BY sku",
        )?;
        stmt.query_map([id], Self::row_to_line)?.collect()
    }

    fn load_full(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<StockSnapshot> {
        let mut head =
            conn.query_row("SELECT * FROM stock_snapshots WHERE id = ?", [id], Self::row_to_head)?;
        head.lines = Self::load_lines(conn, id)?;
        Ok(head)
    }
}

impl StockSnapshotRepository for SqliteStockSnapshotRepository {
    fn inventory_aging(
        &self,
        as_of: chrono::NaiveDate,
    ) -> Result<stateset_core::InventoryAgingReport> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        super::stock_reports::aging(&tx, as_of).map_err(super::map_db_error)
    }

    fn capture(&self, input: CaptureStockSnapshot) -> Result<StockSnapshot> {
        if !input.lines.is_empty() {
            return Err(CommerceError::ValidationError(
                "Snapshot lines are read from the stock ledger; omit caller-supplied lines".into(),
            ));
        }
        let id = StockSnapshotId::new();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let lines = {
                let mut stmt = tx.prepare("SELECT v.product_id, i.sku, b.quantity_on_hand, b.quantity_available, w.code FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id LEFT JOIN product_variants v ON v.sku = i.sku LEFT JOIN warehouses w ON w.id = b.location_id ORDER BY i.sku, b.location_id")?;
                let rows = stmt.query_map([], |r| {
                    Ok(StockSnapshotLine {
                        id: StockSnapshotLineId::new(),
                        stock_snapshot_id: id,
                        product_id: parse_uuid_opt_row(r.get(0)?, "snapshot", "product_id")?
                            .map(Into::into),
                        sku: r.get(1)?,
                        quantity_on_hand: parse_decimal_row(
                            &r.get::<_, String>(2)?,
                            "snapshot",
                            "quantity_on_hand",
                        )?,
                        quantity_available: parse_decimal_row(
                            &r.get::<_, String>(3)?,
                            "snapshot",
                            "quantity_available",
                        )?,
                        location: r.get(4)?,
                    })
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let total_skus =
                lines.iter().map(|line| &line.sku).collect::<std::collections::HashSet<_>>().len();
            let total_units: rust_decimal::Decimal =
                lines.iter().map(|line| line.quantity_on_hand).sum();
            tx.execute(
                "INSERT INTO stock_snapshots (id, label, total_skus, total_units, captured_at)
                 VALUES (?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.label,
                    total_skus as i64,
                    total_units.to_string(),
                    &now,
                ],
            )?;
            for line in &lines {
                tx.execute(
                    "INSERT INTO stock_snapshot_lines (id, stock_snapshot_id, product_id, sku, quantity_on_hand, quantity_available, location)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        StockSnapshotLineId::new().to_string(),
                        &id_str,
                        line.product_id.map(|id| id.to_string()),
                        &line.sku,
                        line.quantity_on_hand.to_string(),
                        line.quantity_available.to_string(),
                        &line.location,
                    ],
                )?;
            }
            Self::load_full(tx, &id_str)
        })
    }

    fn get(&self, id: StockSnapshotId) -> Result<Option<StockSnapshot>> {
        let conn = self.conn()?;
        match Self::load_full(&conn, &id.to_string()) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn latest(&self) -> Result<Option<StockSnapshot>> {
        let conn = self.conn()?;
        let id: Option<String> = conn
            .query_row(
                "SELECT id FROM stock_snapshots ORDER BY captured_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok();
        match id {
            Some(id) => Self::load_full(&conn, &id).map(Some).map_err(map_db_error),
            None => Ok(None),
        }
    }

    fn list(&self, filter: StockSnapshotFilter) -> Result<Vec<StockSnapshot>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM stock_snapshots ORDER BY captured_at DESC".to_string();
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        // Header-level listing; lines omitted for brevity (fetch via get()).
        let rows = stmt
            .query_map([], Self::row_to_head)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: StockSnapshotId) -> Result<()> {
        let id_str = id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute("DELETE FROM stock_snapshot_lines WHERE stock_snapshot_id = ?", [&id_str])?;
            tx.execute("DELETE FROM stock_snapshots WHERE id = ?", [&id_str])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CaptureStockLine, CreateInventoryItem, InventoryRepository, ProductId, ReserveInventory,
    };

    fn test_repo() -> SqliteStockSnapshotRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let inventory = super::super::SqliteInventoryRepository::new(db.pool().clone());
        for (sku, quantity) in [("SKU-1", dec!(10)), ("SKU-2", dec!(5))] {
            inventory
                .create_item(CreateInventoryItem {
                    sku: sku.into(),
                    name: sku.into(),
                    initial_quantity: Some(quantity),
                    ..Default::default()
                })
                .unwrap();
        }
        inventory
            .reserve(ReserveInventory {
                sku: "SKU-1".into(),
                quantity: dec!(2),
                location_id: None,
                reference_type: "snapshot".into(),
                reference_id: "snapshot".into(),
                expires_in_seconds: None,
            })
            .unwrap();
        SqliteStockSnapshotRepository::new(db.pool().clone())
    }

    fn capture(repo: &SqliteStockSnapshotRepository) -> StockSnapshot {
        repo.capture(CaptureStockSnapshot { label: Some("EOM".into()), lines: vec![] })
            .expect("capture")
    }

    #[test]
    fn capture_computes_totals_and_lines() {
        let repo = test_repo();
        let s = capture(&repo);
        assert_eq!(s.total_skus, 2);
        assert_eq!(s.total_units, dec!(15));
        assert_eq!(s.lines.len(), 2);
        assert_eq!(s.total_available(), dec!(13));
    }

    #[test]
    fn capture_rejects_imported_values() {
        let repo = test_repo();
        assert!(
            repo.capture(CaptureStockSnapshot {
                label: None,
                lines: vec![CaptureStockLine {
                    product_id: ProductId::new(),
                    sku: "FAKE".into(),
                    quantity_on_hand: dec!(100),
                    quantity_available: dec!(100),
                    location: None
                }]
            })
            .is_err()
        );
    }

    #[test]
    fn latest_returns_most_recent() {
        let repo = test_repo();
        capture(&repo);
        let second = capture(&repo);
        let latest = repo.latest().expect("latest").expect("found");
        assert_eq!(latest.id, second.id);
        assert_eq!(latest.lines.len(), 2);
    }

    #[test]
    fn get_and_delete_cascades() {
        let repo = test_repo();
        let s = capture(&repo);
        assert!(repo.get(s.id).expect("get").is_some());
        repo.delete(s.id).expect("delete");
        assert!(repo.get(s.id).expect("get").is_none());
    }

    #[test]
    fn list_header_level() {
        let repo = test_repo();
        capture(&repo);
        capture(&repo);
        let all = repo.list(StockSnapshotFilter::default()).expect("list");
        assert_eq!(all.len(), 2);
    }
}
