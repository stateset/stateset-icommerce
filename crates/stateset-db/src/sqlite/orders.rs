//! SQLite order repository implementation

use super::{
    build_in_clause, map_db_error, params_refs, uuid_params,
    parse_datetime_row, parse_decimal_row, parse_enum, parse_enum_row,
    parse_json_opt_row, parse_uuid_row, sum_decimal_query, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    validate_batch_size, validate_currency_code, validate_postal_code, validate_price,
    validate_required_text, validate_required_uuid, validate_sku, Address, BatchResult,
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
        let now = Utc::now();
        let timestamp = now.timestamp();
        let nanos = now.timestamp_subsec_nanos();
        // Use 8 hex chars from UUID for better entropy (over 4 billion combinations)
        let random: u32 = (Uuid::new_v4().as_u128() % 0xFFFFFFFF) as u32;
        format!("ORD-{}-{:06}-{:08X}", timestamp, nanos / 1000, random)
    }

    fn row_to_order(row: &rusqlite::Row) -> rusqlite::Result<Order> {
        let shipping_addr: Option<Address> = parse_json_opt_row(
            row.get::<_, Option<String>>("shipping_address")?,
            "order",
            "shipping_address",
        )?;
        let billing_addr: Option<Address> = parse_json_opt_row(
            row.get::<_, Option<String>>("billing_address")?,
            "order",
            "billing_address",
        )?;

        Ok(Order {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "order", "id")?,
            order_number: row.get("order_number")?,
            customer_id: parse_uuid_row(&row.get::<_, String>("customer_id")?, "order", "customer_id")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "order", "status")?,
            order_date: parse_datetime_row(&row.get::<_, String>("order_date")?, "order", "order_date")?,
            total_amount: parse_decimal_row(&row.get::<_, String>("total_amount")?, "order", "total_amount")?,
            currency: row.get("currency")?,
            payment_status: parse_enum_row(&row.get::<_, String>("payment_status")?, "order", "payment_status")?,
            fulfillment_status: parse_enum_row(&row.get::<_, String>("fulfillment_status")?, "order", "fulfillment_status")?,
            payment_method: row.get("payment_method")?,
            shipping_method: row.get("shipping_method")?,
            tracking_number: row.get("tracking_number")?,
            notes: row.get("notes")?,
            shipping_address: shipping_addr,
            billing_address: billing_addr,
            items: vec![], // Loaded separately
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "order", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "order", "updated_at")?,
        })
    }

    fn validate_order_item_input(item: &CreateOrderItem) -> Result<()> {
        validate_required_uuid("order_item.product_id", item.product_id)?;
        if let Some(variant_id) = item.variant_id {
            validate_required_uuid("order_item.variant_id", variant_id)?;
        }
        validate_sku(&item.sku)?;
        validate_required_text("order_item.name", &item.name, 255)?;

        if item.quantity <= 0 {
            return Err(CommerceError::InvalidInput {
                field: "order_item.quantity".to_string(),
                message: "must be greater than zero".into(),
            });
        }

        validate_price(item.unit_price)?;
        if let Some(discount) = item.discount {
            validate_price(discount)?;
        }
        if let Some(tax) = item.tax_amount {
            validate_price(tax)?;
        }

        let subtotal = item.unit_price * Decimal::from(item.quantity);
        let discount = item.discount.unwrap_or_default();
        let tax = item.tax_amount.unwrap_or_default();
        if discount > subtotal {
            return Err(CommerceError::ValidationError(
                "Order item discount cannot exceed subtotal".into(),
            ));
        }

        let total = subtotal - discount + tax;
        if total < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Order item total cannot be negative".into(),
            ));
        }

        Ok(())
    }

    fn validate_address_input(address: &Address, field_prefix: &str) -> Result<()> {
        validate_required_text(&format!("{field_prefix}.line1"), &address.line1, 255)?;
        validate_required_text(&format!("{field_prefix}.city"), &address.city, 255)?;
        validate_postal_code(&address.postal_code)?;
        validate_required_text(&format!("{field_prefix}.country"), &address.country, 64)?;

        if let Some(line2) = &address.line2 {
            validate_required_text(&format!("{field_prefix}.line2"), line2, 255)?;
        }
        if let Some(state) = &address.state {
            validate_required_text(&format!("{field_prefix}.state"), state, 64)?;
        }

        Ok(())
    }

    fn validate_order_input(input: &CreateOrder) -> Result<()> {
        validate_required_uuid("order.customer_id", input.customer_id)?;

        if let Some(ref currency) = input.currency {
            validate_currency_code(currency)?;
        }

        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "Order must have at least one item".into(),
            ));
        }

        for item in &input.items {
            Self::validate_order_item_input(item)?;
        }

        if let Some(address) = &input.shipping_address {
            Self::validate_address_input(address, "order.shipping_address")?;
        }
        if let Some(address) = &input.billing_address {
            Self::validate_address_input(address, "order.billing_address")?;
        }

        Ok(())
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
                    id: parse_uuid_row(&row.get::<_, String>("id")?, "order_item", "id")?,
                    order_id: parse_uuid_row(&row.get::<_, String>("order_id")?, "order_item", "order_id")?,
                    product_id: parse_uuid_row(&row.get::<_, String>("product_id")?, "order_item", "product_id")?,
                    variant_id: row
                        .get::<_, Option<String>>("variant_id")?
                        .and_then(|s| s.parse().ok()),
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    quantity: row.get("quantity")?,
                    unit_price: parse_decimal_row(&row.get::<_, String>("unit_price")?, "order_item", "unit_price")?,
                    discount: parse_decimal_row(&row.get::<_, String>("discount")?, "order_item", "discount")?,
                    tax_amount: parse_decimal_row(&row.get::<_, String>("tax_amount")?, "order_item", "tax_amount")?,
                    total: parse_decimal_row(&row.get::<_, String>("total")?, "order_item", "total")?,
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
        Self::validate_order_input(&input)?;

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

        let input = input.clone();

        with_immediate_transaction(&self.pool, |tx| {
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
            )?;

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
                )?;

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

            Ok(Order {
                id,
                order_number: order_number.clone(),
                customer_id: input.customer_id,
                status: OrderStatus::Pending,
                order_date: now,
                total_amount: total,
                currency: currency.clone(),
                payment_status: PaymentStatus::Pending,
                fulfillment_status: FulfillmentStatus::Unfulfilled,
                payment_method: input.payment_method.clone(),
                shipping_method: input.shipping_method.clone(),
                tracking_number: None,
                notes: input.notes.clone(),
                shipping_address: input.shipping_address.clone(),
                billing_address: input.billing_address.clone(),
                items,
                version: 1,
                created_at: now,
                updated_at: now,
            })
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
        let (current_version, current_status_raw, current_payment_status_raw): (i32, String, String) = conn
            .query_row(
                "SELECT version, status, payment_status FROM orders WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::OrderNotFound(id),
                e => map_db_error(e),
            })?;
        let current_status: OrderStatus = parse_enum(&current_status_raw, "order", "status")?;
        let current_payment_status: PaymentStatus =
            parse_enum(&current_payment_status_raw, "order", "payment_status")?;

        // Build dynamic update
        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            let next_status = *status;
            if !current_status.can_transition_to(next_status) {
                if next_status == OrderStatus::Cancelled {
                    return Err(CommerceError::OrderCannotBeCancelled(
                        current_status.to_string(),
                    ));
                }

                return Err(CommerceError::InvalidOrderStatusTransition {
                    from: current_status.to_string(),
                    to: next_status.to_string(),
                });
            }

            if next_status == OrderStatus::Refunded {
                let effective_payment_status =
                    input.payment_status.unwrap_or(current_payment_status);
                if !matches!(
                    effective_payment_status,
                    PaymentStatus::Paid
                        | PaymentStatus::PartiallyPaid
                        | PaymentStatus::Refunded
                        | PaymentStatus::PartiallyRefunded
                ) {
                    return Err(CommerceError::OrderCannotBeRefunded(
                        effective_payment_status.to_string(),
                    ));
                }
            }

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
            Self::validate_address_input(addr, "order.shipping_address")?;
            updates.push("shipping_address = ?");
            params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
        }
        if let Some(addr) = &input.billing_address {
            Self::validate_address_input(addr, "order.billing_address")?;
            updates.push("billing_address = ?");
            params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
        }

        updates.push("version = version + 1");
        params.push(Box::new(id.to_string()));
        params.push(Box::new(current_version));

        let sql = format!(
            "UPDATE orders SET {} WHERE id = ? AND version = ?",
            updates.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows_affected = conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

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
        validate_required_uuid("order.id", order_id)?;
        Self::validate_order_item_input(&item)?;

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

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateOrder>) -> Result<BatchResult<Order>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(order) => result.record_success(order),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateOrder>) -> Result<Vec<Order>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            Self::validate_order_input(&input)?;

            let id = Uuid::new_v4();
            let order_number = Self::generate_order_number();
            let now = Utc::now();
            let currency = input.currency.clone().unwrap_or_else(|| "USD".to_string());

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

            results.push(Order {
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
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<BatchResult<Order>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(order) => result.record_success(order),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<Vec<Order>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        // For atomic updates, we use a transaction and fail on any error
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();
            let (current_version, current_status_raw, current_payment_status_raw): (i32, String, String) = tx
                .query_row(
                    "SELECT version, status, payment_status FROM orders WHERE id = ?",
                    [id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::OrderNotFound(id),
                    e => map_db_error(e),
                })?;
            let current_status: OrderStatus = parse_enum(&current_status_raw, "order", "status")?;
            let current_payment_status: PaymentStatus =
                parse_enum(&current_payment_status_raw, "order", "payment_status")?;

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(status) = &input.status {
                let next_status = *status;
                if !current_status.can_transition_to(next_status) {
                    if next_status == OrderStatus::Cancelled {
                        return Err(CommerceError::OrderCannotBeCancelled(
                            current_status.to_string(),
                        ));
                    }

                    return Err(CommerceError::InvalidOrderStatusTransition {
                        from: current_status.to_string(),
                        to: next_status.to_string(),
                    });
                }

                if next_status == OrderStatus::Refunded {
                    let effective_payment_status =
                        input.payment_status.unwrap_or(current_payment_status);
                    if !matches!(
                        effective_payment_status,
                        PaymentStatus::Paid
                            | PaymentStatus::PartiallyPaid
                            | PaymentStatus::Refunded
                            | PaymentStatus::PartiallyRefunded
                    ) {
                        return Err(CommerceError::OrderCannotBeRefunded(
                            effective_payment_status.to_string(),
                        ));
                    }
                }

                update_parts.push("status = ?");
                params.push(Box::new(status.to_string()));
            }
            if let Some(payment_status) = &input.payment_status {
                update_parts.push("payment_status = ?");
                params.push(Box::new(payment_status.to_string()));
            }
            if let Some(fulfillment_status) = &input.fulfillment_status {
                update_parts.push("fulfillment_status = ?");
                params.push(Box::new(fulfillment_status.to_string()));
            }
            if let Some(tracking) = &input.tracking_number {
                update_parts.push("tracking_number = ?");
                params.push(Box::new(tracking.clone()));
            }
            if let Some(notes) = &input.notes {
                update_parts.push("notes = ?");
                params.push(Box::new(notes.clone()));
            }
            if let Some(addr) = &input.shipping_address {
                Self::validate_address_input(addr, "order.shipping_address")?;
                update_parts.push("shipping_address = ?");
                params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
            }
            if let Some(addr) = &input.billing_address {
                Self::validate_address_input(addr, "order.billing_address")?;
                update_parts.push("billing_address = ?");
                params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
            }

            update_parts.push("version = version + 1");
            params.push(Box::new(id.to_string()));
            params.push(Box::new(current_version));

            let sql = format!(
                "UPDATE orders SET {} WHERE id = ? AND version = ?",
                update_parts.join(", ")
            );

            let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
            if rows_affected == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "order".to_string(),
                    id: id.to_string(),
                    expected_version: current_version,
                });
            }

            let order = tx
                .query_row(
                    "SELECT * FROM orders WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_order,
                )
                .map_err(map_db_error)?;

            results.push(order);
        }

        tx.commit().map_err(map_db_error)?;

        // Load items for each order
        let conn = self.conn()?;
        for order in &mut results {
            order.items = Self::load_order_items_with_conn(&conn, order.id)?;
        }

        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        // Delete order items first
        let sql = format!(
            "DELETE FROM order_items WHERE order_id IN ({})",
            placeholders
        );
        tx.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        // Delete orders
        let sql = format!("DELETE FROM orders WHERE id IN ({})", placeholders);
        tx.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Order>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM orders WHERE id IN ({})", placeholders);

        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let orders = stmt
            .query_map(params_refs.as_slice(), Self::row_to_order)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for each order
        let mut result = vec![];
        for mut order in orders {
            order.items = Self::load_order_items_with_conn(&conn, order.id)?;
            result.push(order);
        }

        Ok(result)
    }
}

impl SqliteOrderRepository {
    fn update_order_total(
        &self,
        conn: &rusqlite::Connection,
        order_id: Uuid,
    ) -> Result<()> {
        let current_version: i32 = conn
            .query_row(
                "SELECT version FROM orders WHERE id = ?",
                [order_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::OrderNotFound(order_id),
                e => map_db_error(e),
            })?;

        let order_id_param = order_id.to_string();
        let order_params: [&dyn rusqlite::ToSql; 1] = [&order_id_param];
        let total = sum_decimal_query(
            conn,
            "SELECT total FROM order_items WHERE order_id = ?",
            &order_params,
            "order_item",
            "total",
        )?;
        let total = format!("{:.2}", total);

        let rows_affected = conn
            .execute(
                "UPDATE orders SET total_amount = ?, updated_at = ?, version = version + 1 WHERE id = ? AND version = ?",
                rusqlite::params![
                    total,
                    Utc::now().to_rfc3339(),
                    order_id.to_string(),
                    current_version
                ],
            )
            .map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".to_string(),
                id: order_id.to_string(),
                expected_version: current_version,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_payment_status_snake_case_and_legacy() {
        assert_eq!(PaymentStatus::from_str("partially_paid").unwrap(), PaymentStatus::PartiallyPaid);
        assert_eq!(PaymentStatus::from_str("partiallypaid").unwrap(), PaymentStatus::PartiallyPaid);
        assert_eq!(
            PaymentStatus::from_str("partially_refunded").unwrap(),
            PaymentStatus::PartiallyRefunded
        );
        assert_eq!(
            PaymentStatus::from_str("partiallyrefunded").unwrap(),
            PaymentStatus::PartiallyRefunded
        );
    }

    #[test]
    fn parses_fulfillment_status_snake_case_and_legacy() {
        assert_eq!(
            FulfillmentStatus::from_str("partially_fulfilled").unwrap(),
            FulfillmentStatus::PartiallyFulfilled
        );
        assert_eq!(
            FulfillmentStatus::from_str("partiallyfulfilled").unwrap(),
            FulfillmentStatus::PartiallyFulfilled
        );
    }
}
