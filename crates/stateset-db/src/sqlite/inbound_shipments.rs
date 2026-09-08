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

    /// Advance a shipment's status, refusing to move a cancelled one.
    ///
    /// The precondition is in the write, so it cannot be separated from the act.
    /// Without it [`Self::receive_line`]'s cancelled-shipment refusal was
    /// trivially bypassable: `cancel` then `mark_arrived` put the ASN back into
    /// a live status and the next receipt booked stock against a shipment nobody
    /// expected to take delivery of.
    fn advance_status(
        &self,
        id: InboundShipmentId,
        status: InboundShipmentStatus,
    ) -> Result<InboundShipment> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let rows = tx.execute(
                "UPDATE inbound_shipments SET status = ?, updated_at = ?
                 WHERE id = ? AND status <> 'cancelled'",
                rusqlite::params![status.to_string(), &now, &id_str],
            )?;
            if rows == 0 {
                let exists: bool = tx
                    .query_row("SELECT 1 FROM inbound_shipments WHERE id = ?", [&id_str], |_| {
                        Ok(true)
                    })
                    .optional()?
                    .unwrap_or(false);
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(if exists {
                    CommerceError::Conflict(
                        "Cannot change the status of a cancelled inbound shipment".into(),
                    )
                } else {
                    CommerceError::NotFound
                })));
            }
            Self::load_full(tx, &id_str)
        })
    }

    /// Write the cancel for a shipment the caller has already guarded.
    ///
    /// The precondition is the exact status string the guard decided on, so
    /// the write cannot land on a row that moved underneath the read, and a
    /// status list here can never drift away from the guard's. A write that
    /// matches nothing is a conflict, not a success: the previous version
    /// discarded `rows_affected` entirely, so a zero-row cancel returned the
    /// untouched shipment as though it had been cancelled.
    fn apply_cancel_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
        observed_status: &str,
        now: &str,
    ) -> rusqlite::Result<()> {
        let changed = tx.execute(
            "UPDATE inbound_shipments SET status = 'cancelled', updated_at = ?
             WHERE id = ? AND status = ?",
            rusqlite::params![now, id_str, observed_status],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::Conflict(format!(
                    "inbound shipment {id_str} was no longer in status {observed_status} when the cancel was applied"
                )),
            )));
        }
        Ok(())
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
        self.advance_status(id, InboundShipmentStatus::InTransit)
    }

    fn mark_arrived(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        self.advance_status(id, InboundShipmentStatus::Arrived)
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
            // Read the HEAD through this write transaction before touching the
            // line: a cancelled ASN must not take in stock, and the derived
            // status write below would otherwise reopen it as
            // `partially_received` / `received`. Mirrors the `FOR UPDATE` head
            // lock in `postgres/inbound_shipments.rs`.
            let head_status: Option<String> = tx
                .query_row("SELECT status FROM inbound_shipments WHERE id = ?", [&id_str], |r| {
                    r.get(0)
                })
                .optional()?;
            let Some(head_status) = head_status else {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            };
            if head_status == InboundShipmentStatus::Cancelled.to_string() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "Cannot receive against a cancelled inbound shipment".into(),
                    ),
                )));
            }

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

    /// Cancel an inbound shipment.
    ///
    /// The terminal-state guard reads the status and the cancel writes it, so
    /// both happen in one IMMEDIATE transaction and the write carries the
    /// precondition itself. Split across a pooled `get()` and a separate
    /// `set_status()` the guard decided on a status nobody held: a cancel could
    /// land on a shipment that reached `received` after the check, leaving a
    /// cancelled ASN holding received stock.
    fn cancel(&self, id: InboundShipmentId) -> Result<InboundShipment> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let current: Option<String> = tx
                .query_row("SELECT status FROM inbound_shipments WHERE id = ?", [&id_str], |r| {
                    r.get(0)
                })
                .optional()?;
            let Some(current) = current else {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            };
            let parsed: InboundShipmentStatus =
                parse_enum_row(&current, "inbound_shipment", "status")?;
            if parsed.is_terminal() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "Cannot cancel an inbound shipment in status {parsed}"
                    )),
                )));
            }
            Self::apply_cancel_in_tx(tx, &id_str, &current, &now)?;
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

    /// The cancel's own write must report a conflict when it matches no row,
    /// instead of reporting success on a shipment it never touched. The write
    /// half is exercised directly because within one IMMEDIATE transaction the
    /// guard and the write cannot legitimately disagree — the point of the
    /// check is that a future divergence fails closed rather than silently.
    #[test]
    fn a_cancel_that_writes_no_row_is_a_conflict() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        let id_str = s.id.to_string();
        let now = Utc::now().to_rfc3339();

        let mut conn = repo.pool.get().expect("connection");
        let tx = conn.transaction().expect("transaction");

        // The precondition the caller decided on no longer describes the row.
        let err =
            SqliteInboundShipmentRepository::apply_cancel_in_tx(&tx, &id_str, "arrived", &now)
                .expect_err("a zero-row cancel must not report success");
        let mapped = map_db_error(err);
        assert!(matches!(mapped, CommerceError::Conflict(_)), "got {mapped:?}");

        // The real precondition still writes exactly one row.
        SqliteInboundShipmentRepository::apply_cancel_in_tx(&tx, &id_str, "pending", &now)
            .expect("the observed status must still cancel");
        tx.commit().expect("commit");
        assert_eq!(
            repo.get(s.id).expect("get").expect("found").status,
            InboundShipmentStatus::Cancelled
        );
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

    /// `receive_line` never looked at the shipment's own status, so units could
    /// be booked against a cancelled ASN — and the derived-status write then
    /// quietly reopened it as `partially_received` / `received`. The head is now
    /// read through the write transaction and `Cancelled` is refused, matching
    /// `postgres/inbound_shipments.rs`.
    #[test]
    fn receive_line_refuses_a_cancelled_shipment() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        repo.cancel(s.id).expect("cancel shipment");

        let err = repo
            .receive_line(s.id, s.items[0].id, dec!(1))
            .expect_err("a cancelled shipment must refuse receipts");
        assert!(
            matches!(err, CommerceError::ValidationError(_)),
            "expected a validation error, got {err:?}"
        );

        let stored = repo.get(s.id).expect("get").expect("found");
        assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
        assert_eq!(stored.total_received(), Decimal::ZERO);
    }

    /// `mark_arrived` / `mark_in_transit` wrote the status with no precondition,
    /// so `cancel -> mark_arrived -> receive_line` walked straight around the
    /// refusal above and put stock on a cancelled ASN.
    #[test]
    fn status_advances_are_refused_on_a_cancelled_shipment() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        repo.cancel(s.id).expect("cancel shipment");

        let arrived = repo.mark_arrived(s.id).expect_err("cannot arrive a cancelled shipment");
        assert!(
            matches!(arrived, CommerceError::Conflict(_)),
            "expected Conflict, got {arrived:?}"
        );
        let transit = repo.mark_in_transit(s.id).expect_err("cannot ship a cancelled shipment");
        assert!(
            matches!(transit, CommerceError::Conflict(_)),
            "expected Conflict, got {transit:?}"
        );

        let stored = repo.get(s.id).expect("get").expect("found");
        assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
    }

    #[test]
    fn status_advances_still_work_on_a_live_shipment() {
        let repo = test_repo();
        let s = new_shipment(&repo);
        assert_eq!(
            repo.mark_in_transit(s.id).expect("mark in transit").status,
            InboundShipmentStatus::InTransit
        );
        assert_eq!(
            repo.mark_arrived(s.id).expect("mark arrived").status,
            InboundShipmentStatus::Arrived
        );
        let received = repo.receive_line(s.id, s.items[0].id, dec!(10)).expect("receive the line");
        assert_eq!(received.status, InboundShipmentStatus::Received);
        assert_eq!(received.total_received(), dec!(10));
    }

    /// A cancel racing a full receipt: exactly one may win, in either order.
    /// `cancel` read the status through a second pooled connection and then
    /// wrote it, and `receive_line` never read it at all, so both could report
    /// success and leave a cancelled shipment holding received stock.
    #[test]
    fn cancel_racing_a_full_receipt_admits_exactly_one() {
        use std::sync::{Arc, Barrier};

        let db = Arc::new(SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db"));
        for round in 0..40 {
            let repo = SqliteInboundShipmentRepository::new(db.pool().clone());
            let s = repo
                .create(CreateInboundShipment {
                    supplier_id: Uuid::new_v4(),
                    purchase_order_id: None,
                    warehouse_id: None,
                    carrier: None,
                    tracking_number: None,
                    expected_at: None,
                    items: vec![CreateInboundShipmentItem {
                        product_id: ProductId::new(),
                        sku: format!("SKU-RACE-{round}"),
                        quantity_expected: dec!(5),
                    }],
                    notes: None,
                })
                .expect("create shipment");
            let item_id = s.items[0].id;

            let barrier = Arc::new(Barrier::new(2));
            let receiving = {
                let (db, barrier, id) = (Arc::clone(&db), Arc::clone(&barrier), s.id);
                std::thread::spawn(move || {
                    let repo = SqliteInboundShipmentRepository::new(db.pool().clone());
                    barrier.wait();
                    repo.receive_line(id, item_id, dec!(5))
                })
            };
            let cancelling = {
                let (db, barrier, id) = (Arc::clone(&db), Arc::clone(&barrier), s.id);
                std::thread::spawn(move || {
                    let repo = SqliteInboundShipmentRepository::new(db.pool().clone());
                    barrier.wait();
                    repo.cancel(id)
                })
            };
            let received_ok = receiving.join().expect("receive thread").is_ok();
            let cancelled_ok = cancelling.join().expect("cancel thread").is_ok();
            assert!(
                received_ok ^ cancelled_ok,
                "round {round}: exactly one of the receipt and the cancel may win \
                 (receipt={received_ok}, cancel={cancelled_ok})"
            );

            let stored = repo.get(s.id).expect("get").expect("found");
            if cancelled_ok {
                assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
                assert_eq!(
                    stored.total_received(),
                    Decimal::ZERO,
                    "round {round}: a cancelled inbound shipment must not hold received stock"
                );
            } else {
                assert_eq!(stored.status, InboundShipmentStatus::Received);
                assert_eq!(stored.total_received(), dec!(5));
            }
        }
    }
}
