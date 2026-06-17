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
            .query_map(param_refs.as_slice(), Self::row_to_order_head)
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
        let id_str = id.to_string();
        let item_str = item_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let current: Option<String> = tx
                .query_row(
                    "SELECT quantity_received FROM transfer_order_items WHERE id = ? AND transfer_order_id = ?",
                    rusqlite::params![&item_str, &id_str],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(current) = current else {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            };
            let current: Decimal = current.parse().unwrap_or(Decimal::ZERO);
            let new_received = current + quantity;
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

    fn cancel(&self, id: TransferOrderId) -> Result<TransferOrder> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
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
    fn cancel_sets_status() {
        let repo = test_repo();
        let o = new_order(&repo);
        let cancelled = repo.cancel(o.id).expect("cancel");
        assert_eq!(cancelled.status, TransferOrderStatus::Cancelled);
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
