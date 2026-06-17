//! SQLite implementation of the purgatory (order ingestion staging) repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_json_row, parse_uuid_opt_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, IngestOrder, MapPurgatoryLine, PurgatoryFilter, PurgatoryLineItem,
    PurgatoryLineItemId, PurgatoryOrder, PurgatoryOrderId, PurgatoryRepository, Result,
};

#[derive(Debug)]
pub struct SqlitePurgatoryRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePurgatoryRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_line(row: &rusqlite::Row<'_>) -> rusqlite::Result<PurgatoryLineItem> {
        Ok(PurgatoryLineItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "purgatory_line", "id")?.into(),
            purgatory_order_id: parse_uuid_row(
                &row.get::<_, String>("purgatory_order_id")?,
                "purgatory_line",
                "purgatory_order_id",
            )?
            .into(),
            external_sku: row.get("external_sku")?,
            product_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("product_id")?,
                "purgatory_line",
                "product_id",
            )?
            .map(Into::into),
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "purgatory_line",
                "quantity",
            )?,
            ignore_item: row.get::<_, i32>("ignore_item")? != 0,
            non_physical: row.get::<_, i32>("non_physical")? != 0,
        })
    }

    fn row_to_head(row: &rusqlite::Row<'_>) -> rusqlite::Result<PurgatoryOrder> {
        let metadata_json: String = row.get("metadata")?;
        Ok(PurgatoryOrder {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "purgatory_order", "id")?.into(),
            channel_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("channel_id")?,
                "purgatory_order",
                "channel_id",
            )?
            .map(Into::into),
            external_order_id: row.get("external_order_id")?,
            external_status: row.get("external_status")?,
            is_posted: row.get::<_, i32>("is_posted")? != 0,
            hold_reason: row.get("hold_reason")?,
            metadata: parse_json_row(&metadata_json, "purgatory_order", "metadata")?,
            items: Vec::new(),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "purgatory_order",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "purgatory_order",
                "updated_at",
            )?,
        })
    }

    fn load_items(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<Vec<PurgatoryLineItem>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM purgatory_line_items WHERE purgatory_order_id = ? ORDER BY external_sku",
        )?;
        stmt.query_map([id], Self::row_to_line)?.collect()
    }

    fn load_full(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<PurgatoryOrder> {
        let mut head =
            conn.query_row("SELECT * FROM purgatory_orders WHERE id = ?", [id], Self::row_to_head)?;
        head.items = Self::load_items(conn, id)?;
        Ok(head)
    }
}

impl PurgatoryRepository for SqlitePurgatoryRepository {
    fn ingest(&self, input: IngestOrder) -> Result<PurgatoryOrder> {
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "an ingested order requires at least one line".into(),
            ));
        }
        let id = PurgatoryOrderId::new();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        let metadata_json = serde_json::to_string(&input.metadata)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO purgatory_orders (id, channel_id, external_order_id, external_status, is_posted, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 0, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.channel_id.map(|c| c.to_string()),
                    &input.external_order_id,
                    &input.external_status,
                    &metadata_json,
                    &now,
                    &now,
                ],
            )?;
            for item in &input.items {
                tx.execute(
                    "INSERT INTO purgatory_line_items (id, purgatory_order_id, external_sku, product_id, quantity, ignore_item, non_physical)
                     VALUES (?, ?, ?, ?, ?, 0, 0)",
                    rusqlite::params![
                        PurgatoryLineItemId::new().to_string(),
                        &id_str,
                        &item.external_sku,
                        item.product_id.map(|p| p.to_string()),
                        item.quantity.to_string(),
                    ],
                )?;
            }
            Self::load_full(tx, &id_str)
        })
    }

    fn get(&self, id: PurgatoryOrderId) -> Result<Option<PurgatoryOrder>> {
        let conn = self.conn()?;
        match Self::load_full(&conn, &id.to_string()) {
            Ok(o) => Ok(Some(o)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: PurgatoryFilter) -> Result<Vec<PurgatoryOrder>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM purgatory_orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(channel) = filter.channel_id {
            sql.push_str(" AND channel_id = ?");
            params.push(Box::new(channel.to_string()));
        }
        // Defaults to non-posted when not specified.
        let is_posted = filter.is_posted.unwrap_or(false);
        sql.push_str(" AND is_posted = ?");
        params.push(Box::new(is_posted as i32));
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

    fn map_line(
        &self,
        id: PurgatoryOrderId,
        line_id: PurgatoryLineItemId,
        input: MapPurgatoryLine,
    ) -> Result<PurgatoryOrder> {
        let id_str = id.to_string();
        let line_str = line_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut sets: Vec<String> = vec![];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
            if let Some(product_id) = input.product_id {
                sets.push("product_id = ?".into());
                params.push(Box::new(product_id.to_string()));
            }
            if let Some(ignore) = input.ignore_item {
                sets.push("ignore_item = ?".into());
                params.push(Box::new(ignore as i32));
            }
            if let Some(non_physical) = input.non_physical {
                sets.push("non_physical = ?".into());
                params.push(Box::new(non_physical as i32));
            }
            if !sets.is_empty() {
                let sql = format!(
                    "UPDATE purgatory_line_items SET {} WHERE id = ? AND purgatory_order_id = ?",
                    sets.join(", ")
                );
                params.push(Box::new(line_str.clone()));
                params.push(Box::new(id_str.clone()));
                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                tx.execute(&sql, param_refs.as_slice())?;
            }
            tx.execute(
                "UPDATE purgatory_orders SET updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn post(&self, id: PurgatoryOrderId) -> Result<PurgatoryOrder> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let order = Self::load_full(tx, &id_str)?;
            if order.is_posted {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("order is already posted".into()),
                )));
            }
            if !order.is_ready_to_post() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "{} line(s) still unresolved",
                        order.unresolved_count()
                    )),
                )));
            }
            tx.execute(
                "UPDATE purgatory_orders SET is_posted = 1, hold_reason = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn delete(&self, id: PurgatoryOrderId) -> Result<()> {
        let id_str = id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute("DELETE FROM purgatory_line_items WHERE purgatory_order_id = ?", [&id_str])?;
            tx.execute("DELETE FROM purgatory_orders WHERE id = ?", [&id_str])?;
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
    use stateset_core::{IngestLineItem, ProductId};

    fn test_repo() -> SqlitePurgatoryRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqlitePurgatoryRepository::new(db.pool().clone())
    }

    fn ingest(repo: &SqlitePurgatoryRepository, mapped: bool) -> PurgatoryOrder {
        repo.ingest(IngestOrder {
            channel_id: None,
            external_order_id: "SHOP-1001".into(),
            external_status: Some("paid".into()),
            metadata: serde_json::json!({"src": "shopify"}),
            items: vec![IngestLineItem {
                external_sku: "EXT-1".into(),
                quantity: dec!(2),
                product_id: if mapped { Some(ProductId::new()) } else { None },
            }],
        })
        .expect("ingest")
    }

    #[test]
    fn ingest_rejects_empty() {
        let repo = test_repo();
        let res = repo.ingest(IngestOrder {
            channel_id: None,
            external_order_id: "X".into(),
            external_status: None,
            metadata: serde_json::Value::Null,
            items: vec![],
        });
        assert!(res.is_err());
    }

    #[test]
    fn unmapped_order_cannot_post() {
        let repo = test_repo();
        let o = ingest(&repo, false);
        assert!(!o.is_ready_to_post());
        assert!(repo.post(o.id).is_err());
    }

    #[test]
    fn map_line_then_post() {
        let repo = test_repo();
        let o = ingest(&repo, false);
        let line = o.items[0].id;
        let mapped = repo
            .map_line(
                o.id,
                line,
                MapPurgatoryLine { product_id: Some(ProductId::new()), ..Default::default() },
            )
            .expect("map");
        assert!(mapped.is_ready_to_post());
        let posted = repo.post(o.id).expect("post");
        assert!(posted.is_posted);
        // double-post fails
        assert!(repo.post(o.id).is_err());
    }

    #[test]
    fn ignore_flag_resolves_line() {
        let repo = test_repo();
        let o = ingest(&repo, false);
        let line = o.items[0].id;
        repo.map_line(
            o.id,
            line,
            MapPurgatoryLine { ignore_item: Some(true), ..Default::default() },
        )
        .expect("ignore");
        assert!(repo.post(o.id).is_ok());
    }

    #[test]
    fn list_defaults_to_non_posted() {
        let repo = test_repo();
        let a = ingest(&repo, true);
        ingest(&repo, true);
        repo.post(a.id).expect("post");
        // default filter → only non-posted
        let pending = repo.list(PurgatoryFilter::default()).expect("list");
        assert_eq!(pending.len(), 1);
        let posted = repo
            .list(PurgatoryFilter { is_posted: Some(true), ..Default::default() })
            .expect("list");
        assert_eq!(posted.len(), 1);
    }
}
