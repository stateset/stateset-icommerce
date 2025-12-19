//! SQLite order repository implementation

use super::{map_db_error, parse_decimal};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateOrder, CreateOrderItem, FulfillmentStatus, Order, OrderFilter,
    OrderItem, OrderRepository, OrderStatus, PaymentStatus, Result, UpdateOrder,
};
use uuid::Uuid;

/// SQLite implementation of OrderRepository
pub struct SqliteOrderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteOrderRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_order_number() -> String {
        let timestamp = Utc::now().timestamp();
        let random: u32 = (Uuid::new_v4().as_u128() % 10000) as u32;
        format!("ORD-{}-{:04}", timestamp, random)
    }

    fn row_to_order(row: &rusqlite::Row) -> rusqlite::Result<Order> {
        let shipping_addr: Option<String> = row.get("shipping_address")?;
        let billing_addr: Option<String> = row.get("billing_address")?;

        Ok(Order {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            order_number: row.get("order_number")?,
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
            status: parse_order_status(&row.get::<_, String>("status")?),
            order_date: row
                .get::<_, String>("order_date")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
            total_amount: parse_decimal(&row.get::<_, String>("total_amount")?),
            currency: row.get("currency")?,
            payment_status: parse_payment_status(&row.get::<_, String>("payment_status")?),
            fulfillment_status: parse_fulfillment_status(&row.get::<_, String>("fulfillment_status")?),
            payment_method: row.get("payment_method")?,
            shipping_method: row.get("shipping_method")?,
            tracking_number: row.get("tracking_number")?,
            notes: row.get("notes")?,
            shipping_address: shipping_addr.and_then(|s| serde_json::from_str(&s).ok()),
            billing_address: billing_addr.and_then(|s| serde_json::from_str(&s).ok()),
            items: vec![], // Loaded separately
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: row
                .get::<_, String>("created_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
            updated_at: row
                .get::<_, String>("updated_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    fn load_order_items_with_conn(
        conn: &r2d2::PooledConnection<SqliteConnectionManager>,
        order_id: Uuid,
    ) -> Result<Vec<OrderItem>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, order_id, product_id, variant_id, sku, name, quantity,
                        unit_price, discount, tax_amount, total
                 FROM order_items WHERE order_id = ?",
            )
            .map_err(map_db_error)?;

        let items = stmt
            .query_map([order_id.to_string()], |row| {
                Ok(OrderItem {
                    id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                    order_id: row.get::<_, String>("order_id")?.parse().unwrap_or_default(),
                    product_id: row.get::<_, String>("product_id")?.parse().unwrap_or_default(),
                    variant_id: row
                        .get::<_, Option<String>>("variant_id")?
                        .and_then(|s| s.parse().ok()),
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    quantity: row.get("quantity")?,
                    unit_price: parse_decimal(&row.get::<_, String>("unit_price")?),
                    discount: parse_decimal(&row.get::<_, String>("discount")?),
                    tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
                    total: parse_decimal(&row.get::<_, String>("total")?),
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }
}

impl OrderRepository for SqliteOrderRepository {
    fn create(&self, input: CreateOrder) -> Result<Order> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let order_number = Self::generate_order_number();
        let now = Utc::now();
        let currency = input.currency.clone().unwrap_or_else(|| "USD".to_string());

        // Calculate total
        let total: Decimal = input
            .items
            .iter()
            .map(|item| {
                let subtotal = item.unit_price * Decimal::from(item.quantity);
                let discount = item.discount.unwrap_or_default();
                let tax = item.tax_amount.unwrap_or_default();
                subtotal - discount + tax
            })
            .sum();

        let shipping_address_json = input
            .shipping_address
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

        tx.execute(
            "INSERT INTO orders (id, order_number, customer_id, status, order_date, total_amount,
                                 currency, payment_status, fulfillment_status, payment_method,
                                 shipping_method, notes, shipping_address, billing_address,
                                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &order_number,
                input.customer_id.to_string(),
                "pending",
                now.to_rfc3339(),
                total.to_string(),
                &currency,
                "pending",
                "unfulfilled",
                &input.payment_method,
                &input.shipping_method,
                &input.notes,
                &shipping_address_json,
                &billing_address_json,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Insert order items and build items vec
        let mut items = Vec::with_capacity(input.items.len());
        for item in &input.items {
            let item_id = Uuid::new_v4();
            let item_total = OrderItem::calculate_total(
                item.quantity,
                item.unit_price,
                item.discount.unwrap_or_default(),
                item.tax_amount.unwrap_or_default(),
            );

            tx.execute(
                "INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                          quantity, unit_price, discount, tax_amount, total)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    item_id.to_string(),
                    id.to_string(),
                    item.product_id.to_string(),
                    item.variant_id.map(|v| v.to_string()),
                    &item.sku,
                    &item.name,
                    item.quantity,
                    item.unit_price.to_string(),
                    item.discount.unwrap_or_default().to_string(),
                    item.tax_amount.unwrap_or_default().to_string(),
                    item_total.to_string(),
                ],
            )
            .map_err(map_db_error)?;

            items.push(OrderItem {
                id: item_id,
                order_id: id,
                product_id: item.product_id,
                variant_id: item.variant_id,
                sku: item.sku.clone(),
                name: item.name.clone(),
                quantity: item.quantity,
                unit_price: item.unit_price,
                discount: item.discount.unwrap_or_default(),
                tax_amount: item.tax_amount.unwrap_or_default(),
                total: item_total,
            });
        }

        tx.commit().map_err(map_db_error)?;

        Ok(Order {
            id,
            order_number,
            customer_id: input.customer_id,
            status: OrderStatus::Pending,
            order_date: now,
            total_amount: total,
            currency,
            payment_status: PaymentStatus::Pending,
            fulfillment_status: FulfillmentStatus::Unfulfilled,
            payment_method: input.payment_method,
            shipping_method: input.shipping_method,
            tracking_number: None,
            notes: input.notes,
            shipping_address: input.shipping_address,
            billing_address: input.billing_address,
            items,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    fn get(&self, id: Uuid) -> Result<Option<Order>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM orders WHERE id = ?",
            [id.to_string()],
            Self::row_to_order,
        );

        match result {
            Ok(mut order) => {
                order.items = Self::load_order_items_with_conn(&conn, id)?;
                Ok(Some(order))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM orders WHERE order_number = ?",
            [order_number],
            Self::row_to_order,
        );

        match result {
            Ok(mut order) => {
                order.items = Self::load_order_items_with_conn(&conn, order.id)?;
                Ok(Some(order))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        let conn = self.conn()?;
        let now = Utc::now();

        // Build dynamic update
        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(payment_status) = &input.payment_status {
            updates.push("payment_status = ?");
            params.push(Box::new(payment_status.to_string()));
        }
        if let Some(fulfillment_status) = &input.fulfillment_status {
            updates.push("fulfillment_status = ?");
            params.push(Box::new(fulfillment_status.to_string()));
        }
        if let Some(tracking) = &input.tracking_number {
            updates.push("tracking_number = ?");
            params.push(Box::new(tracking.clone()));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }
        if let Some(addr) = &input.shipping_address {
            updates.push("shipping_address = ?");
            params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
        }
        if let Some(addr) = &input.billing_address {
            updates.push("billing_address = ?");
            params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!(
            "UPDATE orders SET {} WHERE id = ?",
            updates.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        // Now fetch the updated order using the same connection
        let result = conn.query_row(
            "SELECT * FROM orders WHERE id = ?",
            [id.to_string()],
            Self::row_to_order,
        );

        match result {
            Ok(mut order) => {
                order.items = Self::load_order_items_with_conn(&conn, id)?;
                Ok(order)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(CommerceError::OrderNotFound(id)),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(from) = &filter.from_date {
            sql.push_str(" AND order_date >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(to) = &filter.to_date {
            sql.push_str(" AND order_date <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        sql.push_str(" ORDER BY order_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let orders = stmt
            .query_map(params_refs.as_slice(), Self::row_to_order)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for each order using the same connection
        let mut result = vec![];
        for mut order in orders {
            order.items = Self::load_order_items_with_conn(&conn, order.id)?;
            result.push(order);
        }

        Ok(result)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        tx.execute("DELETE FROM order_items WHERE order_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.execute("DELETE FROM orders WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let item_id = Uuid::new_v4();
        let item_total = OrderItem::calculate_total(
            item.quantity,
            item.unit_price,
            item.discount.unwrap_or_default(),
            item.tax_amount.unwrap_or_default(),
        );

        tx.execute(
            "INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                      quantity, unit_price, discount, tax_amount, total)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                item_id.to_string(),
                order_id.to_string(),
                item.product_id.to_string(),
                item.variant_id.map(|v| v.to_string()),
                item.sku,
                item.name,
                item.quantity,
                item.unit_price.to_string(),
                item.discount.unwrap_or_default().to_string(),
                item.tax_amount.unwrap_or_default().to_string(),
                item_total.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        // Update order total
        self.update_order_total(&tx, order_id)?;
        tx.commit().map_err(map_db_error)?;

        Ok(OrderItem {
            id: item_id,
            order_id,
            product_id: item.product_id,
            variant_id: item.variant_id,
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            unit_price: item.unit_price,
            discount: item.discount.unwrap_or_default(),
            tax_amount: item.tax_amount.unwrap_or_default(),
            total: item_total,
        })
    }

    fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        tx.execute(
            "DELETE FROM order_items WHERE id = ? AND order_id = ?",
            [item_id.to_string(), order_id.to_string()],
        )
        .map_err(map_db_error)?;

        self.update_order_total(&tx, order_id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn count(&self, filter: OrderFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }
}

impl SqliteOrderRepository {
    fn update_order_total(
        &self,
        conn: &rusqlite::Connection,
        order_id: Uuid,
    ) -> Result<()> {
        let total: String = conn
            .query_row(
                "SELECT COALESCE(SUM(CAST(total AS REAL)), 0) FROM order_items WHERE order_id = ?",
                [order_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        conn.execute(
            "UPDATE orders SET total_amount = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![total, Utc::now().to_rfc3339(), order_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }
}

fn parse_order_status(s: &str) -> OrderStatus {
    match s {
        "pending" => OrderStatus::Pending,
        "confirmed" => OrderStatus::Confirmed,
        "processing" => OrderStatus::Processing,
        "shipped" => OrderStatus::Shipped,
        "delivered" => OrderStatus::Delivered,
        "cancelled" => OrderStatus::Cancelled,
        "refunded" => OrderStatus::Refunded,
        _ => OrderStatus::Pending,
    }
}

fn parse_payment_status(s: &str) -> PaymentStatus {
    match s {
        "pending" => PaymentStatus::Pending,
        "authorized" => PaymentStatus::Authorized,
        "paid" => PaymentStatus::Paid,
        "partially_paid" | "partiallypaid" => PaymentStatus::PartiallyPaid,
        "refunded" => PaymentStatus::Refunded,
        "partially_refunded" | "partiallyrefunded" => PaymentStatus::PartiallyRefunded,
        "failed" => PaymentStatus::Failed,
        _ => PaymentStatus::Pending,
    }
}

fn parse_fulfillment_status(s: &str) -> FulfillmentStatus {
    match s {
        "unfulfilled" => FulfillmentStatus::Unfulfilled,
        "partially_fulfilled" | "partiallyfulfilled" => FulfillmentStatus::PartiallyFulfilled,
        "fulfilled" => FulfillmentStatus::Fulfilled,
        "shipped" => FulfillmentStatus::Shipped,
        "delivered" => FulfillmentStatus::Delivered,
        _ => FulfillmentStatus::Unfulfilled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_payment_status_snake_case_and_legacy() {
        assert_eq!(parse_payment_status("partially_paid"), PaymentStatus::PartiallyPaid);
        assert_eq!(parse_payment_status("partiallypaid"), PaymentStatus::PartiallyPaid);
        assert_eq!(
            parse_payment_status("partially_refunded"),
            PaymentStatus::PartiallyRefunded
        );
        assert_eq!(
            parse_payment_status("partiallyrefunded"),
            PaymentStatus::PartiallyRefunded
        );
    }

    #[test]
    fn parses_fulfillment_status_snake_case_and_legacy() {
        assert_eq!(
            parse_fulfillment_status("partially_fulfilled"),
            FulfillmentStatus::PartiallyFulfilled
        );
        assert_eq!(
            parse_fulfillment_status("partiallyfulfilled"),
            FulfillmentStatus::PartiallyFulfilled
        );
    }
}
