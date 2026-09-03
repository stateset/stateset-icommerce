//! SQLite implementation of the transfer order repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateTransferOrder, Result, TransferOrder, TransferOrderFilter,
    TransferOrderId, TransferOrderItem, TransferOrderItemId, TransferOrderStatus,
};

#[derive(Debug)]
pub struct SqliteTransferOrderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteTransferOrderRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<TransferOrderItem> {
        Ok(TransferOrderItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "transfer_order_item", "id")?.into(),
            transfer_order_id: parse_uuid_row(
                &row.get::<_, String>("transfer_order_id")?,
                "transfer_order_item",
                "transfer_order_id",
            )?
            .into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "transfer_order_item",
                "product_id",
            )?
            .into(),
            sku: row.get("sku")?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "transfer_order_item",
                "quantity",
            )?,
            quantity_shipped: parse_decimal_row(
                &row.get::<_, String>("quantity_shipped")?,
                "transfer_order_item",
                "quantity_shipped",
            )?,
            quantity_received: parse_decimal_row(
                &row.get::<_, String>("quantity_received")?,
                "transfer_order_item",
                "quantity_received",
            )?,
        })
    }

    fn row_to_order_head(row: &rusqlite::Row<'_>) -> rusqlite::Result<TransferOrder> {
        Ok(TransferOrder {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "transfer_order", "id")?.into(),
            number: row.get("number")?,
            source_warehouse_id: parse_uuid_row(
                &row.get::<_, String>("source_warehouse_id")?,
                "transfer_order",
                "source_warehouse_id",
            )?
            .into(),
            destination_warehouse_id: parse_uuid_row(
                &row.get::<_, String>("destination_warehouse_id")?,
                "transfer_order",
                "destination_warehouse_id",
            )?
            .into(),
            status: parse_enum_row::<TransferOrderStatus>(
                &row.get::<_, String>("status")?,
                "transfer_order",
                "status",
            )?,
            items: Vec::new(),
            expected_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expected_at")?,
                "transfer_order",
                "expected_at",
            )?,
            shipped_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("shipped_at")?,
                "transfer_order",
                "shipped_at",
            )?,
            received_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("received_at")?,
                "transfer_order",
                "received_at",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "transfer_order",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "transfer_order",
                "updated_at",
            )?,
        })
    }

    fn load_items(
        conn: &rusqlite::Connection,
        order_id: &str,
    ) -> rusqlite::Result<Vec<TransferOrderItem>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM transfer_order_items WHERE transfer_order_id = ? ORDER BY sku",
        )?;
        let items = stmt
            .query_map([order_id], Self::row_to_item)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    fn load_items_batch(
        conn: &rusqlite::Connection,
        ids: &[String],
    ) -> rusqlite::Result<std::collections::HashMap<String, Vec<TransferOrderItem>>> {
        let mut map: std::collections::HashMap<String, Vec<TransferOrderItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let placeholders = super::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT * FROM transfer_order_items WHERE transfer_order_id IN ({placeholders}) ORDER BY sku"
            );
            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                let parent: String = row.get("transfer_order_id")?;
                Ok((parent, Self::row_to_item(row)?))
            })?;
            for row in rows {
                let (parent, item) = row?;
                map.entry(parent).or_default().push(item);
            }
        }
        Ok(map)
    }

    fn load_full(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<TransferOrder> {
        let mut order = conn.query_row(
            "SELECT * FROM transfer_orders WHERE id = ?",
            [id],
            Self::row_to_order_head,
        )?;
        order.items = Self::load_items(conn, id)?;
        Ok(order)
    }
}

impl stateset_core::TransferOrderRepository for SqliteTransferOrderRepository {
    fn create(&self, input: CreateTransferOrder) -> Result<TransferOrder> {
        if input.source_warehouse_id == input.destination_warehouse_id {
            return Err(CommerceError::ValidationError(
                "source and destination warehouses must differ".into(),
            ));
        }
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "a transfer order requires at least one item".into(),
            ));
        }
        let id = TransferOrderId::new();
        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        // Human-readable number derived from the timestamp + short id fragment.
        let number = format!("TO-{}", &id_str[..8]);

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO transfer_orders (id, number, source_warehouse_id, destination_warehouse_id, status, expected_at, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 'draft', ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &number,
                    input.source_warehouse_id.to_string(),
                    input.destination_warehouse_id.to_string(),
                    input.expected_at.map(|d| d.to_rfc3339()),
                    &input.notes,
                    &now_str,
                    &now_str,
                ],
            )?;

            for item in &input.items {
                let item_id = TransferOrderItemId::new().to_string();
                tx.execute(
                    "INSERT INTO transfer_order_items (id, transfer_order_id, product_id, sku, quantity, quantity_shipped, quantity_received)
                     VALUES (?, ?, ?, '', ?, '0', '0')",
                    rusqlite::params![
                        &item_id,
                        &id_str,
                        item.product_id.to_string(),
                        item.quantity.to_string(),
                    ],
                )?;
            }

            Self::load_full(tx, &id_str)
        })
    }

    fn get(&self, id: TransferOrderId) -> Result<Option<TransferOrder>> {
        let conn = self.conn()?;
        match Self::load_full(&conn, &id.to_string()) {
            Ok(o) => Ok(Some(o)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: TransferOrderFilter) -> Result<Vec<TransferOrder>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM transfer_orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(src) = filter.source_warehouse_id {
            sql.push_str(" AND source_warehouse_id = ?");
            params.push(Box::new(src.to_string()));
        }
        if let Some(dest) = filter.destination_warehouse_id {
            sql.push_str(" AND destination_warehouse_id = ?");
            params.push(Box::new(dest.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let heads = stmt
            .query_map(param_refs.as_slice(), Self::row_to_order_head)
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

    fn ship(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            // Shipping sets quantity_shipped = quantity on each line.
            tx.execute(
                "UPDATE transfer_order_items SET quantity_shipped = quantity WHERE transfer_order_id = ?",
                [&id_str],
            )?;
            tx.execute(
                "UPDATE transfer_orders SET status = 'in_transit', shipped_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    fn receive_line(
        &self,
        id: TransferOrderId,
        item_id: TransferOrderItemId,
        quantity: Decimal,
    ) -> Result<TransferOrder> {
        if quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("receive quantity must be positive".into()));
        }
        let id_str = id.to_string();
        let item_str = item_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            // Decide on the order status under the same write transaction that
            // records the receipt: units cannot be booked in against a transfer
            // order that a concurrent cancel has already closed.
            let status: String = tx.query_row(
                "SELECT status FROM transfer_orders WHERE id = ?",
                [&id_str],
                |row| row.get(0),
            )?;
            if status == TransferOrderStatus::Cancelled.to_string() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "Cannot receive against a cancelled transfer order".into(),
                    ),
                )));
            }
            let row: Option<(String, String)> = tx
                .query_row(
                    "SELECT quantity, quantity_received FROM transfer_order_items WHERE id = ? AND transfer_order_id = ?",
                    rusqlite::params![&item_str, &id_str],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((expected, current)) = row else {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            };
            let expected: Decimal = expected.parse().unwrap_or(Decimal::ZERO);
            let current: Decimal = current.parse().unwrap_or(Decimal::ZERO);
            let new_received = current + quantity;
            if new_received > expected {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "receiving {quantity} would exceed the {expected} expected on this line ({current} already received)"
                    )),
                )));
            }
            tx.execute(
                "UPDATE transfer_order_items SET quantity_received = ? WHERE id = ?",
                rusqlite::params![new_received.to_string(), &item_str],
            )?;

            // Recompute order status from line receipts.
            let order = Self::load_full(tx, &id_str)?;
            let derived = order.derive_receipt_status();
            let received_at =
                if derived == TransferOrderStatus::Received { Some(now.clone()) } else { None };
            tx.execute(
                "UPDATE transfer_orders SET status = ?, received_at = COALESCE(?, received_at), updated_at = ? WHERE id = ?",
                rusqlite::params![derived.to_string(), received_at, &now, &id_str],
            )?;
            Self::load_full(tx, &id_str)
        })
    }

    /// Cancel a transfer order.
    ///
    /// The terminal-state read and the cancel write share one IMMEDIATE
    /// transaction. Reading the status on a pooled connection and writing on a
    /// later transaction decided the guard on a status nobody held, so
    /// concurrent cancels each saw a live order and each wrote.
    fn cancel(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let status: String = tx.query_row(
                "SELECT status FROM transfer_orders WHERE id = ?",
                [&id_str],
                |row| row.get(0),
            )?;
            if matches!(status.as_str(), "received" | "cancelled") {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "Cannot cancel a transfer order in status {status}"
                    )),
                )));
            }
            tx.execute(
                "UPDATE transfer_orders SET status = 'cancelled', updated_at = ? WHERE id = ?",
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
    use stateset_core::{CreateTransferOrderItem, ProductId, TransferOrderRepository, WarehouseId};

    fn test_repo() -> SqliteTransferOrderRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteTransferOrderRepository::new(db.pool().clone())
    }

    fn new_order(repo: &SqliteTransferOrderRepository) -> TransferOrder {
        repo.create(CreateTransferOrder {
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            items: vec![CreateTransferOrderItem {
                product_id: ProductId::new(),
                quantity: dec!(10),
            }],
            expected_at: None,
            notes: Some("restock".into()),
        })
        .expect("create order")
    }

    #[test]
    fn create_rejects_same_warehouse() {
        let repo = test_repo();
        let w = WarehouseId::new();
        let res = repo.create(CreateTransferOrder {
            source_warehouse_id: w,
            destination_warehouse_id: w,
            items: vec![CreateTransferOrderItem {
                product_id: ProductId::new(),
                quantity: dec!(1),
            }],
            expected_at: None,
            notes: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn create_and_get_with_items() {
        let repo = test_repo();
        let o = new_order(&repo);
        assert_eq!(o.status, TransferOrderStatus::Draft);
        assert_eq!(o.items.len(), 1);
        let fetched = repo.get(o.id).expect("get").expect("found");
        assert_eq!(fetched.total_quantity(), dec!(10));
    }

    #[test]
    fn ship_then_receive_transitions_status() {
        let repo = test_repo();
        let o = new_order(&repo);
        let shipped = repo.ship(o.id).expect("ship");
        assert_eq!(shipped.status, TransferOrderStatus::InTransit);
        assert_eq!(shipped.items[0].quantity_shipped, dec!(10));

        let item_id = shipped.items[0].id;
        let partial = repo.receive_line(o.id, item_id, dec!(4)).expect("receive partial");
        assert_eq!(partial.status, TransferOrderStatus::PartiallyReceived);

        let full = repo.receive_line(o.id, item_id, dec!(6)).expect("receive rest");
        assert_eq!(full.status, TransferOrderStatus::Received);
        assert!(full.received_at.is_some());
    }

    #[test]
    fn receive_line_rejects_over_receipt() {
        let repo = test_repo();
        let o = new_order(&repo);
        let shipped = repo.ship(o.id).expect("ship");
        let item_id = shipped.items[0].id;
        // Line expects 10; receiving 11 at once is rejected.
        assert!(repo.receive_line(o.id, item_id, dec!(11)).is_err());
        // Non-positive quantities are rejected.
        assert!(repo.receive_line(o.id, item_id, dec!(0)).is_err());
        // After receiving 7, receiving another 4 (total 11) is rejected.
        repo.receive_line(o.id, item_id, dec!(7)).expect("receive 7");
        assert!(repo.receive_line(o.id, item_id, dec!(4)).is_err());
        // Exact remaining (3) still succeeds.
        let full = repo.receive_line(o.id, item_id, dec!(3)).expect("receive rest");
        assert_eq!(full.status, TransferOrderStatus::Received);
    }

    #[test]
    fn receive_line_rejects_cancelled_order() {
        let repo = test_repo();
        let o = new_order(&repo);
        let shipped = repo.ship(o.id).expect("ship");
        let item_id = shipped.items[0].id;
        repo.cancel(o.id).expect("cancel");
        // Units cannot be booked in against a cancelled transfer order: the
        // status is sticky in `derive_receipt_status`, so without this guard the
        // receipt silently raised `quantity_received` on a cancelled order.
        let err = repo.receive_line(o.id, item_id, dec!(1)).expect_err("receive after cancel");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
        let stored = repo.get(o.id).expect("get").expect("found");
        assert_eq!(stored.total_received(), dec!(0));
    }

    #[test]
    fn cancel_sets_status() {
        let repo = test_repo();
        let o = new_order(&repo);
        let cancelled = repo.cancel(o.id).expect("cancel");
        assert_eq!(cancelled.status, TransferOrderStatus::Cancelled);

        // Terminal-state guard: cancelling again is rejected.
        let err = repo.cancel(o.id).expect_err("already cancelled");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    #[test]
    fn list_filters_by_status() {
        let repo = test_repo();
        let a = new_order(&repo);
        new_order(&repo);
        repo.cancel(a.id).expect("cancel");
        let cancelled = repo
            .list(TransferOrderFilter {
                status: Some(TransferOrderStatus::Cancelled),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(cancelled.len(), 1);
    }
}
