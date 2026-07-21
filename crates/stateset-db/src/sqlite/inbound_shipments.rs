//! SQLite implementation of the inbound shipment repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_uuid_opt_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateInboundShipment, InboundShipment, InboundShipmentFilter,
    InboundShipmentId, InboundShipmentItem, InboundShipmentItemId, InboundShipmentStatus, Result,
};

#[derive(Debug)]
pub struct SqliteInboundShipmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteInboundShipmentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<InboundShipmentItem> {
        Ok(InboundShipmentItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "inbound_shipment_item", "id")?.into(),
            inbound_shipment_id: parse_uuid_row(
                &row.get::<_, String>("inbound_shipment_id")?,
                "inbound_shipment_item",
                "inbound_shipment_id",
            )?
            .into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "inbound_shipment_item",
                "product_id",
            )?
            .into(),
            sku: row.get("sku")?,
            quantity_expected: parse_decimal_row(
                &row.get::<_, String>("quantity_expected")?,
                "inbound_shipment_item",
                "quantity_expected",
            )?,
            quantity_received: parse_decimal_row(
                &row.get::<_, String>("quantity_received")?,
                "inbound_shipment_item",
                "quantity_received",
            )?,
        })
    }

    fn row_to_head(row: &rusqlite::Row<'_>) -> rusqlite::Result<InboundShipment> {
        Ok(InboundShipment {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "inbound_shipment", "id")?.into(),
            number: row.get("number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "inbound_shipment",
                "supplier_id",
            )?,
            purchase_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("purchase_order_id")?,
                "inbound_shipment",
                "purchase_order_id",
            )?,
            warehouse_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("warehouse_id")?,
                "inbound_shipment",
                "warehouse_id",
            )?
            .map(Into::into),
            carrier: row.get("carrier")?,
            tracking_number: row.get("tracking_number")?,
            status: parse_enum_row::<InboundShipmentStatus>(
                &row.get::<_, String>("status")?,
                "inbound_shipment",
                "status",
            )?,
            items: Vec::new(),
            expected_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expected_at")?,
                "inbound_shipment",
                "expected_at",
            )?,
            received_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("received_at")?,
                "inbound_shipment",
                "received_at",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "inbound_shipment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "inbound_shipment",
                "updated_at",
            )?,
        })
    }

    fn load_items(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<Vec<InboundShipmentItem>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM inbound_shipment_items WHERE inbound_shipment_id = ? ORDER BY sku",
        )?;
        stmt.query_map([id], Self::row_to_item)?.collect()
    }

    fn load_items_batch(
        conn: &rusqlite::Connection,
        ids: &[String],
    ) -> rusqlite::Result<std::collections::HashMap<String, Vec<InboundShipmentItem>>> {
        let mut map: std::collections::HashMap<String, Vec<InboundShipmentItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let placeholders = super::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT * FROM inbound_shipment_items WHERE inbound_shipment_id IN ({placeholders}) ORDER BY sku"
            );
            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                let parent: String = row.get("inbound_shipment_id")?;
                Ok((parent, Self::row_to_item(row)?))
            })?;
            for row in rows {
                let (parent, item) = row?;
                map.entry(parent).or_default().push(item);
            }
        }
        Ok(map)
    }

    fn load_full(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<InboundShipment> {
        let mut head = conn.query_row(
            "SELECT * FROM inbound_shipments WHERE id = ?",
            [id],
            Self::row_to_head,
        )?;
        head.items = Self::load_items(conn, id)?;
        Ok(head)
    }

    fn set_status(
        &self,
        id: InboundShipmentId,
        status: InboundShipmentStatus,
    ) -> Result<InboundShipment> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE inbound_shipments SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.to_string(), &now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }
}

impl stateset_core::InboundShipmentRepository for SqliteInboundShipmentRepository {
    fn create(&self, input: CreateInboundShipment) -> Result<InboundShipment> {
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "an inbound shipment requires at least one item".into(),
            ));
        }
        let id = InboundShipmentId::new();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        let number = format!("ASN-{}", &id_str[..8]);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO inbound_shipments (id, number, supplier_id, purchase_order_id, warehouse_id, carrier, tracking_number, status, expected_at, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &number,
                    input.supplier_id.to_string(),
                    input.purchase_order_id.map(|p| p.to_string()),
                    input.warehouse_id.map(|w| w.to_string()),
                    &input.carrier,
                    &input.tracking_number,
                    input.expected_at.map(|d| d.to_rfc3339()),
                    &input.notes,
                    &now,
                    &now,
                ],
            )?;
            for item in &input.items {
                tx.execute(
                    "INSERT INTO inbound_shipment_items (id, inbound_shipment_id, product_id, sku, quantity_expected, quantity_received)
                     VALUES (?, ?, ?, ?, ?, '0')",
                    rusqlite::params![
                        InboundShipmentItemId::new().to_string(),
                        &id_str,
                        item.product_id.to_string(),
                        &item.sku,
                        item.quantity_expected.to_string(),
                    ],
                )?;
            }
            Self::load_full(tx, &id_str)
        })
    }

    fn get(&self, id: InboundShipmentId) -> Result<Option<InboundShipment>> {
        let conn = self.conn()?;
        match Self::load_full(&conn, &id.to_string()) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: InboundShipmentFilter) -> Result<Vec<InboundShipment>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM inbound_shipments WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(supplier) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params.push(Box::new(supplier.to_string()));
        }
        if let Some(warehouse) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params.push(Box::new(warehouse.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let heads = stmt
            .query_map(param_refs.as_slice(), Self::row_to_head)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        let ids: Vec<String> = heads.iter().map(|h| h.id.to_string()).collect();
        let mut items_by_id = Self::load_items_batch(&conn, &ids).map_err(map_db_error)?;
        let mut out = Vec::with_capacity(heads.len());
        for mut head in heads {
            head.items = items_by_id.remove(&head.id.to_string()).unwrap_or_default();
            out.push(head);
        }
        Ok(out)
    }

    fn mark_in_transit(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::InTransit)
    }

    fn mark_arrived(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.set_status(id, InboundShipmentStatus::Arrived)
    }

    fn receive_line(
        &self,
        id: InboundShipmentId,
        item_id: InboundShipmentItemId,
        quantity: Decimal,
    ) -> Result<InboundShipment> {
        if quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("receive quantity must be positive".into()));
        }
        let id_str = id.to_string();
        let item_str = item_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let row: Option<(String, String)> = tx
                .query_row(
                    "SELECT quantity_expected, quantity_received FROM inbound_shipment_items WHERE id = ? AND inbound_shipment_id = ?",
                    rusqlite::params![&item_str, &id_str],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((expected, current)) = row else {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            };
            let expected: Decimal = expected.parse().unwrap_or(Decimal::ZERO);
            let new_received: Decimal =
                current.parse::<Decimal>().unwrap_or(Decimal::ZERO) + quantity;
            if new_received > expected {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "receiving {quantity} would exceed the {expected} expected on this line"
                    )),
                )));
            }
            tx.execute(
                "UPDATE inbound_shipment_items SET quantity_received = ? WHERE id = ?",
                rusqlite::params![new_received.to_string(), &item_str],
            )?;

            let shipment = Self::load_full(tx, &id_str)?;
            let derived = shipment.derive_receipt_status();
            let received_at =
                if derived == InboundShipmentStatus::Received { Some(now.clone()) } else { None };
            tx.execute(
                "UPDATE inbound_shipments SET status = ?, received_at = COALESCE(?, received_at), updated_at = ? WHERE id = ?",
                rusqlite::params![derived.to_string(), received_at, &now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn cancel(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        let current = self.get(id)?.ok_or(CommerceError::NotFound)?;
        if matches!(
            current.status,
            InboundShipmentStatus::Received | InboundShipmentStatus::Cancelled
        ) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot cancel an inbound shipment in status {}",
                current.status
            )));
        }
        self.set_status(id, InboundShipmentStatus::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CreateInboundShipmentItem, InboundShipmentRepository, ProductId};
    use uuid::Uuid;

    fn test_repo() -> SqliteInboundShipmentRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteInboundShipmentRepository::new(db.pool().clone())
    }

    fn new_shipment(repo: &SqliteInboundShipmentRepository) -> InboundShipment {
        repo.create(CreateInboundShipment {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            warehouse_id: None,
            carrier: Some("DHL".into()),
            tracking_number: Some("1Z999".into()),
            expected_at: None,
            items: vec![CreateInboundShipmentItem {
                product_id: ProductId::new(),
                sku: "SKU-1".into(),
                quantity_expected: dec!(10),
            }],
            notes: None,
        })
        .expect("create shipment")
    }

    #[test]
    fn create_rejects_empty_items() {
        let repo = test_repo();
        let res = repo.create(CreateInboundShipment {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            warehouse_id: None,
            carrier: None,
            tracking_number: None,
            expected_at: None,
            items: vec![],
            notes: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn create_and_get() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        assert_eq!(s.status, InboundShipmentStatus::Pending);
        assert_eq!(s.items.len(), 1);
        let fetched = repo.get(s.id).expect("get").expect("found");
        assert_eq!(fetched.total_expected(), dec!(10));
    }

    #[test]
    fn lifecycle_transitions() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        assert_eq!(
            repo.mark_in_transit(s.id).expect("transit").status,
            InboundShipmentStatus::InTransit
        );
        assert_eq!(
            repo.mark_arrived(s.id).expect("arrived").status,
            InboundShipmentStatus::Arrived
        );
        let item = s.items[0].id;
        let partial = repo.receive_line(s.id, item, dec!(4)).expect("partial");
        assert_eq!(partial.status, InboundShipmentStatus::PartiallyReceived);
        let full = repo.receive_line(s.id, item, dec!(6)).expect("full");
        assert_eq!(full.status, InboundShipmentStatus::Received);
        assert!(full.received_at.is_some());
    }

    #[test]
    fn receive_line_rejects_over_receipt() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        let item = s.items[0].id;
        // Line expects 10; receiving 11 at once is rejected.
        assert!(repo.receive_line(s.id, item, dec!(11)).is_err());
        // After receiving 6, another 5 (total 11) is rejected.
        repo.receive_line(s.id, item, dec!(6)).expect("receive 6");
        assert!(repo.receive_line(s.id, item, dec!(5)).is_err());
        // Exact remaining (4) still succeeds.
        let full = repo.receive_line(s.id, item, dec!(4)).expect("receive rest");
        assert_eq!(full.status, InboundShipmentStatus::Received);
    }

    #[test]
    fn cancel_sets_status() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        assert_eq!(repo.cancel(s.id).expect("cancel").status, InboundShipmentStatus::Cancelled);

        // Terminal-state guard: cancelling again is rejected.
        let err = repo.cancel(s.id).expect_err("already cancelled");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    #[test]
    fn list_filters_by_status() {
        let repo = test_repo();
        let a = new_shipment(&repo);
        new_shipment(&repo);
        repo.cancel(a.id).expect("cancel");
        let cancelled = repo
            .list(InboundShipmentFilter {
                status: Some(InboundShipmentStatus::Cancelled),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(cancelled.len(), 1);
    }
}
