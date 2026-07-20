//! SQLite implementation of purchase order repository

use super::parse_helpers::parse_decimal as parse_decimal_with_context;
use super::{
    build_in_clause,
    map_db_error,
    params_refs,
    parse_datetime_opt_row,
    parse_datetime_row,
    parse_decimal_opt_row,
    parse_decimal_row,
    parse_enum_row,
    // Non-row variants for Result-returning functions
    parse_uuid,
    parse_uuid_opt_row,
    parse_uuid_row,
    sum_decimal_query,
    uuid_params,
    with_immediate_transaction,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Row, params};
use rust_decimal::Decimal;
use stateset_core::{
    BatchResult, CommerceError, CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier,
    ProductId, PurchaseOrder, PurchaseOrderFilter, PurchaseOrderId, PurchaseOrderItem,
    PurchaseOrderRepository, PurchaseOrderStatus, ReceivePurchaseOrderItems, Result, Supplier,
    SupplierFilter, UpdatePurchaseOrder, UpdateSupplier, generate_po_number,
    generate_supplier_code, validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqlitePurchaseOrderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePurchaseOrderRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_supplier(row: &Row<'_>) -> rusqlite::Result<Supplier> {
        Ok(Supplier {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "supplier", "id")?,
            supplier_code: row.get("supplier_code")?,
            name: row.get("name")?,
            contact_name: row.get("contact_name")?,
            email: row.get("email")?,
            phone: row.get("phone")?,
            website: row.get("website")?,
            address: row.get("address")?,
            city: row.get("city")?,
            state: row.get("state")?,
            postal_code: row.get("postal_code")?,
            country: row.get("country")?,
            tax_id: row.get("tax_id")?,
            payment_terms: parse_enum_row(
                &row.get::<_, String>("payment_terms")?,
                "supplier",
                "payment_terms",
            )?,
            currency: row.get("currency")?,
            lead_time_days: row.get("lead_time_days")?,
            minimum_order: parse_decimal_opt_row(
                row.get::<_, Option<String>>("minimum_order")?,
                "supplier",
                "minimum_order",
            )?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "supplier",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "supplier",
                "updated_at",
            )?,
        })
    }

    fn row_to_po(row: &Row<'_>) -> rusqlite::Result<PurchaseOrder> {
        Ok(PurchaseOrder {
            id: PurchaseOrderId::from(parse_uuid_row(
                &row.get::<_, String>("id")?,
                "purchase_order",
                "id",
            )?),
            po_number: row.get("po_number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "purchase_order",
                "supplier_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "purchase_order", "status")?,
            order_date: parse_datetime_row(
                &row.get::<_, String>("order_date")?,
                "purchase_order",
                "order_date",
            )?,
            expected_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expected_date")?,
                "purchase_order",
                "expected_date",
            )?,
            delivered_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("delivered_date")?,
                "purchase_order",
                "delivered_date",
            )?,
            ship_to_address: row.get("ship_to_address")?,
            ship_to_city: row.get("ship_to_city")?,
            ship_to_state: row.get("ship_to_state")?,
            ship_to_postal_code: row.get("ship_to_postal_code")?,
            ship_to_country: row.get("ship_to_country")?,
            payment_terms: parse_enum_row(
                &row.get::<_, String>("payment_terms")?,
                "purchase_order",
                "payment_terms",
            )?,
            currency: row.get("currency")?,
            subtotal: parse_decimal_row(
                &row.get::<_, String>("subtotal")?,
                "purchase_order",
                "subtotal",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "purchase_order",
                "tax_amount",
            )?,
            shipping_cost: parse_decimal_row(
                &row.get::<_, String>("shipping_cost")?,
                "purchase_order",
                "shipping_cost",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "purchase_order",
                "discount_amount",
            )?,
            total: parse_decimal_row(&row.get::<_, String>("total")?, "purchase_order", "total")?,
            amount_paid: parse_decimal_row(
                &row.get::<_, String>("amount_paid")?,
                "purchase_order",
                "amount_paid",
            )?,
            supplier_reference: row.get("supplier_reference")?,
            notes: row.get("notes")?,
            supplier_notes: row.get("supplier_notes")?,
            approved_by: row.get("approved_by")?,
            approved_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("approved_at")?,
                "purchase_order",
                "approved_at",
            )?,
            items: Vec::new(),
            sent_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("sent_at")?,
                "purchase_order",
                "sent_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "purchase_order",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "purchase_order",
                "updated_at",
            )?,
        })
    }

    fn row_to_po_item(row: &Row<'_>) -> rusqlite::Result<PurchaseOrderItem> {
        Ok(PurchaseOrderItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "purchase_order_item", "id")?,
            purchase_order_id: PurchaseOrderId::from(parse_uuid_row(
                &row.get::<_, String>("purchase_order_id")?,
                "purchase_order_item",
                "purchase_order_id",
            )?),
            product_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("product_id")?,
                "purchase_order_item",
                "product_id",
            )?
            .map(ProductId::from),
            sku: row.get("sku")?,
            name: row.get("name")?,
            supplier_sku: row.get("supplier_sku")?,
            quantity_ordered: parse_decimal_row(
                &row.get::<_, String>("quantity_ordered")?,
                "purchase_order_item",
                "quantity_ordered",
            )?,
            quantity_received: parse_decimal_row(
                &row.get::<_, String>("quantity_received")?,
                "purchase_order_item",
                "quantity_received",
            )?,
            unit_of_measure: row.get("unit_of_measure")?,
            unit_cost: parse_decimal_row(
                &row.get::<_, String>("unit_cost")?,
                "purchase_order_item",
                "unit_cost",
            )?,
            line_total: parse_decimal_row(
                &row.get::<_, String>("line_total")?,
                "purchase_order_item",
                "line_total",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "purchase_order_item",
                "tax_amount",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "purchase_order_item",
                "discount_amount",
            )?,
            expected_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expected_date")?,
                "purchase_order_item",
                "expected_date",
            )?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "purchase_order_item",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "purchase_order_item",
                "updated_at",
            )?,
        })
    }

    fn get_supplier_with_conn(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<Supplier>> {
        let result = conn.query_row(
            "SELECT * FROM suppliers WHERE id = ?",
            [id.to_string()],
            Self::row_to_supplier,
        );
        match result {
            Ok(supplier) => Ok(Some(supplier)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_po_items_with_conn(
        conn: &rusqlite::Connection,
        po_id: PurchaseOrderId,
    ) -> Result<Vec<PurchaseOrderItem>> {
        let mut stmt = conn
            .prepare("SELECT * FROM purchase_order_items WHERE purchase_order_id = ?")
            .map_err(map_db_error)?;
        let rows =
            stmt.query_map([po_id.to_string()], Self::row_to_po_item).map_err(map_db_error)?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(map_db_error)?);
        }
        Ok(items)
    }

    /// Load the PO's current status and reject the update when the state
    /// machine (`PurchaseOrderStatus::can_transition_to`) forbids it.
    fn ensure_transition(
        conn: &rusqlite::Connection,
        id: PurchaseOrderId,
        target: PurchaseOrderStatus,
    ) -> Result<()> {
        let status: String = conn
            .query_row("SELECT status FROM purchase_orders WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                other => map_db_error(other),
            })?;
        let current: PurchaseOrderStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid purchase_order.status '{status}': {e}"))
        })?;
        if !current.can_transition_to(target) {
            return Err(CommerceError::ValidationError(format!(
                "Cannot transition purchase order from {current} to {target}"
            )));
        }
        Ok(())
    }

    fn get_po_with_conn(
        conn: &rusqlite::Connection,
        id: PurchaseOrderId,
    ) -> Result<Option<PurchaseOrder>> {
        let result = conn.query_row(
            "SELECT * FROM purchase_orders WHERE id = ?",
            [id.to_string()],
            Self::row_to_po,
        );
        match result {
            Ok(mut po) => {
                po.items = Self::get_po_items_with_conn(conn, id)?;
                Ok(Some(po))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_po_items(&self, po_id: PurchaseOrderId) -> Result<Vec<PurchaseOrderItem>> {
        let conn = self.conn()?;
        Self::get_po_items_with_conn(&conn, po_id)
    }

    fn recalculate_totals_with_conn(
        conn: &rusqlite::Connection,
        po_id: PurchaseOrderId,
    ) -> Result<()> {
        // Calculate subtotal from items
        let po_id_param = po_id.to_string();
        let po_params: [&dyn rusqlite::ToSql; 1] = [&po_id_param];
        let subtotal = sum_decimal_query(
            conn,
            "SELECT line_total FROM purchase_order_items WHERE purchase_order_id = ?",
            &po_params,
            "purchase_order_item",
            "line_total",
        )?;

        let (tax_amount, shipping_cost, discount_amount): (String, String, String) = conn
            .query_row(
                "SELECT tax_amount, shipping_cost, discount_amount FROM purchase_orders WHERE id = ?",
                [po_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;

        let total = subtotal
            + parse_decimal_with_context(&tax_amount, "purchase_order", "tax_amount")?
            + parse_decimal_with_context(&shipping_cost, "purchase_order", "shipping_cost")?
            - parse_decimal_with_context(&discount_amount, "purchase_order", "discount_amount")?;

        conn.execute(
            "UPDATE purchase_orders SET subtotal = ?, total = ?, updated_at = ? WHERE id = ?",
            params![
                subtotal.to_string(),
                total.to_string(),
                chrono::Utc::now().to_rfc3339(),
                po_id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    #[allow(dead_code)]
    fn recalculate_totals(&self, po_id: PurchaseOrderId) -> Result<()> {
        let conn = self.conn()?;
        Self::recalculate_totals_with_conn(&conn, po_id)
    }
}

impl PurchaseOrderRepository for SqlitePurchaseOrderRepository {
    fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let supplier_code = input.supplier_code.unwrap_or_else(generate_supplier_code);

        conn.execute(
            "INSERT INTO suppliers (id, supplier_code, name, contact_name, email, phone, website,
             address, city, state, postal_code, country, tax_id, payment_terms, currency,
             lead_time_days, minimum_order, is_active, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                supplier_code,
                input.name,
                input.contact_name,
                input.email,
                input.phone,
                input.website,
                input.address,
                input.city,
                input.state,
                input.postal_code,
                input.country,
                input.tax_id,
                input.payment_terms.unwrap_or_default().to_string(),
                input.currency.unwrap_or_default(),
                input.lead_time_days,
                input.minimum_order.map(|d| d.to_string()),
                1,
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Self::get_supplier_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>> {
        let conn = self.conn()?;
        Self::get_supplier_with_conn(&conn, id)
    }

    fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM suppliers WHERE supplier_code = ?",
            [code],
            Self::row_to_supplier,
        );
        match result {
            Ok(supplier) => Ok(Some(supplier)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();
        let supplier = tx
            .query_row(
                "SELECT * FROM suppliers WHERE id = ?",
                [id.to_string()],
                Self::row_to_supplier,
            )
            .map_err(map_db_error)?;

        tx.execute(
            "UPDATE suppliers SET name = ?, contact_name = ?, email = ?, phone = ?, website = ?,
             address = ?, city = ?, state = ?, postal_code = ?, country = ?, tax_id = ?,
             payment_terms = ?, currency = ?, lead_time_days = ?, minimum_order = ?,
             is_active = ?, notes = ?, updated_at = ? WHERE id = ?",
            params![
                input.name.unwrap_or(supplier.name),
                input.contact_name.or(supplier.contact_name),
                input.email.or(supplier.email),
                input.phone.or(supplier.phone),
                input.website.or(supplier.website),
                input.address.or(supplier.address),
                input.city.or(supplier.city),
                input.state.or(supplier.state),
                input.postal_code.or(supplier.postal_code),
                input.country.or(supplier.country),
                input.tax_id.or(supplier.tax_id),
                input.payment_terms.unwrap_or(supplier.payment_terms).to_string(),
                input.currency.unwrap_or(supplier.currency),
                input.lead_time_days.or(supplier.lead_time_days),
                input
                    .minimum_order
                    .map(|d| d.to_string())
                    .or(supplier.minimum_order.map(|d| d.to_string())),
                i32::from(input.is_active.unwrap_or(supplier.is_active)),
                input.notes.or(supplier.notes),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_supplier_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        let conn = self.conn()?;

        let mut sql = "SELECT * FROM suppliers WHERE 1=1".to_string();
        let mut bindings: Vec<String> = Vec::new();

        if let Some(name) = filter.name.as_ref() {
            sql.push_str(" AND LOWER(name) LIKE LOWER(?)");
            bindings.push(format!("%{name}%"));
        }
        if let Some(country) = filter.country.as_ref() {
            sql.push_str(" AND country = ?");
            bindings.push(country.clone());
        }
        if filter.active_only.unwrap_or(false) {
            sql.push_str(" AND is_active = 1");
        }

        sql.push_str(" ORDER BY name ASC");

        // Match Postgres: default the page size to 100 and always honor the
        // offset (SQLite previously applied offset only when a limit was set).
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let bind_refs: Vec<&dyn rusqlite::ToSql> =
            bindings.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows =
            stmt.query_map(bind_refs.as_slice(), Self::row_to_supplier).map_err(map_db_error)?;

        let mut suppliers = Vec::new();
        for row in rows {
            suppliers.push(row.map_err(map_db_error)?);
        }
        Ok(suppliers)
    }

    fn delete_supplier(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE suppliers SET is_active = 0, updated_at = ? WHERE id = ?",
            params![now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get supplier for defaults
        let supplier =
            Self::get_supplier_with_conn(&tx, input.supplier_id)?.ok_or(CommerceError::NotFound)?;

        let id = PurchaseOrderId::new();
        let now = chrono::Utc::now();
        let po_number = generate_po_number();
        let order_date = input.order_date.unwrap_or(now);

        tx.execute(
            "INSERT INTO purchase_orders (id, po_number, supplier_id, status, order_date,
             expected_date, ship_to_address, ship_to_city, ship_to_state, ship_to_postal_code,
             ship_to_country, payment_terms, currency, subtotal, tax_amount, shipping_cost,
             discount_amount, total, amount_paid, notes, supplier_notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                po_number,
                input.supplier_id.to_string(),
                PurchaseOrderStatus::Draft.to_string(),
                order_date.to_rfc3339(),
                input.expected_date.map(|d| d.to_rfc3339()),
                input.ship_to_address,
                input.ship_to_city,
                input.ship_to_state,
                input.ship_to_postal_code,
                input.ship_to_country,
                input.payment_terms.unwrap_or(supplier.payment_terms).to_string(),
                input.currency.unwrap_or(supplier.currency),
                "0",
                input.tax_amount.unwrap_or_default().to_string(),
                input.shipping_cost.unwrap_or_default().to_string(),
                input.discount_amount.unwrap_or_default().to_string(),
                "0",
                "0",
                input.notes,
                input.supplier_notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Add items
        for item in input.items {
            let item_id = Uuid::new_v4();
            let line_total = item.quantity * item.unit_cost
                - item.discount_amount.unwrap_or_default()
                + item.tax_amount.unwrap_or_default();

            tx.execute(
                "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
                 supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
                 line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    item_id.to_string(),
                    id.to_string(),
                    item.product_id.map(|id| id.to_string()),
                    item.sku,
                    item.name,
                    item.supplier_sku,
                    item.quantity.to_string(),
                    "0",
                    item.unit_of_measure,
                    item.unit_cost.to_string(),
                    line_total.to_string(),
                    item.tax_amount.unwrap_or_default().to_string(),
                    item.discount_amount.unwrap_or_default().to_string(),
                    item.expected_date.map(|d| d.to_rfc3339()),
                    item.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        }

        // Recalculate totals
        Self::recalculate_totals_with_conn(&tx, id)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: PurchaseOrderId) -> Result<Option<PurchaseOrder>> {
        let conn = self.conn()?;
        Self::get_po_with_conn(&conn, id)
    }

    fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM purchase_orders WHERE po_number = ?",
            [po_number],
            Self::row_to_po,
        );
        match result {
            Ok(mut po) => {
                po.items = Self::get_po_items_with_conn(&conn, po.id)?;
                Ok(Some(po))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: PurchaseOrderId, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();
        let po = tx
            .query_row(
                "SELECT * FROM purchase_orders WHERE id = ?",
                [id.to_string()],
                Self::row_to_po,
            )
            .map_err(map_db_error)?;

        tx.execute(
            "UPDATE purchase_orders SET expected_date = ?, ship_to_address = ?, ship_to_city = ?,
             ship_to_state = ?, ship_to_postal_code = ?, ship_to_country = ?, payment_terms = ?,
             tax_amount = ?, shipping_cost = ?, discount_amount = ?, notes = ?, supplier_notes = ?,
             supplier_reference = ?, updated_at = ? WHERE id = ?",
            params![
                input
                    .expected_date
                    .map(|d| d.to_rfc3339())
                    .or(po.expected_date.map(|d| d.to_rfc3339())),
                input.ship_to_address.or(po.ship_to_address),
                input.ship_to_city.or(po.ship_to_city),
                input.ship_to_state.or(po.ship_to_state),
                input.ship_to_postal_code.or(po.ship_to_postal_code),
                input.ship_to_country.or(po.ship_to_country),
                input.payment_terms.unwrap_or(po.payment_terms).to_string(),
                input.tax_amount.unwrap_or(po.tax_amount).to_string(),
                input.shipping_cost.unwrap_or(po.shipping_cost).to_string(),
                input.discount_amount.unwrap_or(po.discount_amount).to_string(),
                input.notes.or(po.notes),
                input.supplier_notes.or(po.supplier_notes),
                input.supplier_reference.or(po.supplier_reference),
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        Self::recalculate_totals_with_conn(&tx, id)?;
        tx.commit().map_err(map_db_error)?;

        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        let conn = self.conn()?;

        let mut sql = "SELECT * FROM purchase_orders WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(supplier_id) = &filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params_vec.push(Box::new(supplier_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY order_date DESC");

        // Apply pagination the same way Postgres does: default the page size to
        // 100 and always honor the offset (SQLite previously ignored `offset` and
        // had no default cap, so it returned a different page than Postgres).
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_po).map_err(map_db_error)?;

        let mut orders = Vec::new();
        for row in rows {
            let mut po = row.map_err(map_db_error)?;
            po.items = Self::get_po_items_with_conn(&conn, po.id)?;
            orders.push(po);
        }
        Ok(orders)
    }

    fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.list(PurchaseOrderFilter { supplier_id: Some(supplier_id), ..Default::default() })
    }

    fn delete(&self, id: PurchaseOrderId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let status: String = tx
            .query_row("SELECT status FROM purchase_orders WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .map_err(map_db_error)?;

        let parsed_status: PurchaseOrderStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid purchase_order.status '{status}': {e}"))
        })?;
        if parsed_status != PurchaseOrderStatus::Draft {
            return Err(CommerceError::ValidationError(
                "Can only delete draft purchase orders".to_string(),
            ));
        }

        tx.execute(
            "DELETE FROM purchase_order_items WHERE purchase_order_id = ?",
            [id.to_string()],
        )
        .map_err(map_db_error)?;
        tx.execute("DELETE FROM purchase_orders WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn submit_for_approval(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::PendingApproval)?;
        let now = chrono::Utc::now();
        let rows_affected = conn
            .execute(
                "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
                params![
                    PurchaseOrderStatus::PendingApproval.to_string(),
                    now.to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::NotFound);
        }
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn approve(&self, id: PurchaseOrderId, approved_by: &str) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::Approved)?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, approved_by = ?, approved_at = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Approved.to_string(), approved_by, now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn send(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::Sent)?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, sent_at = ?, updated_at = ? WHERE id = ?",
            params![
                PurchaseOrderStatus::Sent.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn acknowledge(
        &self,
        id: PurchaseOrderId,
        supplier_reference: Option<&str>,
    ) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::Acknowledged)?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, supplier_reference = COALESCE(?, supplier_reference), updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Acknowledged.to_string(), supplier_reference, now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn hold(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::OnHold)?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::OnHold.to_string(), now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::Cancelled)?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Cancelled.to_string(), now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn receive(
        &self,
        id: PurchaseOrderId,
        items: ReceivePurchaseOrderItems,
    ) -> Result<PurchaseOrder> {
        let now = chrono::Utc::now();

        // `receive` is a read-check-write of each item's `quantity_received`.
        // Run it under an IMMEDIATE (retrying) transaction so concurrent receipts
        // serialize and every one lands — a plain deferred transaction lets
        // simultaneous receipts race and drop each other (matching the atomic
        // conditional UPDATE the Postgres backend uses). Domain errors are carried
        // out via `ToSqlConversionFailure` so they are not retried as lock errors.
        let smuggle = |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

        with_immediate_transaction(&self.pool, |tx| {
            let status: String = tx.query_row(
                "SELECT status FROM purchase_orders WHERE id = ?",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )?;
            let current_status: PurchaseOrderStatus = status.parse().map_err(|e| {
                smuggle(CommerceError::DatabaseError(format!(
                    "Invalid purchase_order.status '{status}': {e}"
                )))
            })?;

            for item in &items.items {
                if item.quantity_received <= Decimal::ZERO {
                    return Err(smuggle(CommerceError::ValidationError(
                        "Received quantity must be greater than zero".to_string(),
                    )));
                }

                let (ordered_str, received_str): (String, String) = tx.query_row(
                    "SELECT quantity_ordered, quantity_received
                     FROM purchase_order_items
                     WHERE id = ?1 AND purchase_order_id = ?2",
                    params![item.item_id.to_string(), id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;

                let ordered = parse_decimal_with_context(
                    &ordered_str,
                    "purchase_order_item",
                    "quantity_ordered",
                )
                .map_err(smuggle)?;
                let received = parse_decimal_with_context(
                    &received_str,
                    "purchase_order_item",
                    "quantity_received",
                )
                .map_err(smuggle)?;
                let new_received = received + item.quantity_received;

                if new_received > ordered {
                    return Err(smuggle(CommerceError::ValidationError(format!(
                        "Receiving {} would exceed ordered quantity {} for item {}",
                        new_received, ordered, item.item_id
                    ))));
                }

                tx.execute(
                    "UPDATE purchase_order_items
                     SET quantity_received = ?, updated_at = ?
                     WHERE id = ? AND purchase_order_id = ?",
                    params![
                        new_received.to_string(),
                        now.to_rfc3339(),
                        item.item_id.to_string(),
                        id.to_string()
                    ],
                )?;
            }

            // Check if fully or partially received
            let mut has_items = false;
            let mut all_received = true;
            let mut any_received = false;
            {
                let mut stmt = tx.prepare(
                    "SELECT quantity_ordered, quantity_received
                     FROM purchase_order_items
                     WHERE purchase_order_id = ?",
                )?;
                let rows = stmt.query_map([id.to_string()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;

                for row in rows {
                    let (ordered, received) = row?;
                    let ordered_dec = parse_decimal_with_context(
                        &ordered,
                        "purchase_order_item",
                        "quantity_ordered",
                    )
                    .map_err(smuggle)?;
                    let received_dec = parse_decimal_with_context(
                        &received,
                        "purchase_order_item",
                        "quantity_received",
                    )
                    .map_err(smuggle)?;

                    has_items = true;
                    all_received &= received_dec >= ordered_dec;
                    any_received |= received_dec > Decimal::ZERO;
                }
            }

            let new_status = if !has_items {
                current_status
            } else if all_received {
                PurchaseOrderStatus::Received
            } else if any_received {
                PurchaseOrderStatus::PartiallyReceived
            } else {
                current_status
            };

            tx.execute(
                "UPDATE purchase_orders SET status = ?, delivered_date = CASE WHEN ? = 'received' THEN ? ELSE delivered_date END, updated_at = ? WHERE id = ?",
                params![new_status.to_string(), new_status.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
            )?;

            Self::get_po_with_conn(tx, id)
                .map_err(smuggle)?
                .ok_or_else(|| smuggle(CommerceError::NotFound))
        })
    }

    fn complete(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        Self::ensure_transition(&conn, id, PurchaseOrderStatus::Completed)?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Completed.to_string(), now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn add_item(
        &self,
        po_id: PurchaseOrderId,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

        tx.execute(
            "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
             supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
             line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                po_id.to_string(),
                item.product_id.map(|id| id.to_string()),
                item.sku,
                item.name,
                item.supplier_sku,
                item.quantity.to_string(),
                "0",
                item.unit_of_measure,
                item.unit_cost.to_string(),
                line_total.to_string(),
                item.tax_amount.unwrap_or_default().to_string(),
                item.discount_amount.unwrap_or_default().to_string(),
                item.expected_date.map(|d| d.to_rfc3339()),
                item.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Self::recalculate_totals_with_conn(&tx, po_id)?;

        let item = tx
            .query_row(
                "SELECT * FROM purchase_order_items WHERE id = ?",
                [id.to_string()],
                Self::row_to_po_item,
            )
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Ok(item)
    }

    fn update_item(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

        // Get PO ID for recalculation
        let po_id: String = tx
            .query_row(
                "SELECT purchase_order_id FROM purchase_order_items WHERE id = ?",
                [item_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        tx.execute(
            "UPDATE purchase_order_items SET sku = ?, name = ?, supplier_sku = ?,
             quantity_ordered = ?, unit_cost = ?, line_total = ?, tax_amount = ?,
             discount_amount = ?, expected_date = ?, notes = ?, updated_at = ? WHERE id = ?",
            params![
                item.sku,
                item.name,
                item.supplier_sku,
                item.quantity.to_string(),
                item.unit_cost.to_string(),
                line_total.to_string(),
                item.tax_amount.unwrap_or_default().to_string(),
                item.discount_amount.unwrap_or_default().to_string(),
                item.expected_date.map(|d| d.to_rfc3339()),
                item.notes,
                now.to_rfc3339(),
                item_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        Self::recalculate_totals_with_conn(
            &tx,
            parse_uuid(&po_id, "purchase_order_item", "purchase_order_id")?.into(),
        )?;

        let item = tx
            .query_row(
                "SELECT * FROM purchase_order_items WHERE id = ?",
                [item_id.to_string()],
                Self::row_to_po_item,
            )
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Ok(item)
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let po_id: String = tx
            .query_row(
                "SELECT purchase_order_id FROM purchase_order_items WHERE id = ?",
                [item_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        tx.execute("DELETE FROM purchase_order_items WHERE id = ?", [item_id.to_string()])
            .map_err(map_db_error)?;

        Self::recalculate_totals_with_conn(
            &tx,
            parse_uuid(&po_id, "purchase_order_item", "purchase_order_id")?.into(),
        )?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_items(&self, po_id: PurchaseOrderId) -> Result<Vec<PurchaseOrderItem>> {
        self.get_po_items(po_id)
    }

    fn count(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        let conn = self.conn()?;

        let mut sql = "SELECT COUNT(*) FROM purchase_orders WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(supplier_id) = &filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params_vec.push(Box::new(supplier_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64> {
        let conn = self.conn()?;

        let mut sql = "SELECT COUNT(*) FROM suppliers WHERE 1=1".to_string();

        if filter.active_only.unwrap_or(false) {
            sql.push_str(" AND is_active = 1");
        }

        let count: i64 = conn.query_row(&sql, [], |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<BatchResult<PurchaseOrder>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(po) => result.record_success(po),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<Vec<PurchaseOrder>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            // Get supplier for defaults
            let supplier = Self::get_supplier_with_conn(&tx, input.supplier_id)?
                .ok_or(CommerceError::NotFound)?;

            let id = PurchaseOrderId::new();
            let now = chrono::Utc::now();
            let po_number = generate_po_number();
            let order_date = input.order_date.unwrap_or(now);

            tx.execute(
                "INSERT INTO purchase_orders (id, po_number, supplier_id, status, order_date,
                 expected_date, ship_to_address, ship_to_city, ship_to_state, ship_to_postal_code,
                 ship_to_country, payment_terms, currency, subtotal, tax_amount, shipping_cost,
                 discount_amount, total, amount_paid, notes, supplier_notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id.to_string(),
                    po_number,
                    input.supplier_id.to_string(),
                    PurchaseOrderStatus::Draft.to_string(),
                    order_date.to_rfc3339(),
                    input.expected_date.map(|d| d.to_rfc3339()),
                    input.ship_to_address,
                    input.ship_to_city,
                    input.ship_to_state,
                    input.ship_to_postal_code,
                    input.ship_to_country,
                    input.payment_terms.unwrap_or(supplier.payment_terms).to_string(),
                    input.currency.unwrap_or(supplier.currency),
                    "0",
                    input.tax_amount.unwrap_or_default().to_string(),
                    input.shipping_cost.unwrap_or_default().to_string(),
                    input.discount_amount.unwrap_or_default().to_string(),
                    "0",
                    "0",
                    input.notes,
                    input.supplier_notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            // Add items
            for item in &input.items {
                let item_id = Uuid::new_v4();
                let line_total = item.quantity * item.unit_cost
                    - item.discount_amount.unwrap_or_default()
                    + item.tax_amount.unwrap_or_default();

                tx.execute(
                    "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
                     supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
                     line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        item_id.to_string(),
                        id.to_string(),
                        item.product_id.map(|id| id.to_string()),
                        item.sku,
                        item.name,
                        item.supplier_sku,
                        item.quantity.to_string(),
                        "0",
                        item.unit_of_measure,
                        item.unit_cost.to_string(),
                        line_total.to_string(),
                        item.tax_amount.unwrap_or_default().to_string(),
                        item.discount_amount.unwrap_or_default().to_string(),
                        item.expected_date.map(|d| d.to_rfc3339()),
                        item.notes,
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                    ],
                )
                .map_err(map_db_error)?;
            }

            // Recalculate totals
            Self::recalculate_totals_with_conn(&tx, id)?;

            // Get the created PO
            let po = Self::get_po_with_conn(&tx, id)?.ok_or(CommerceError::NotFound)?;
            results.push(po);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(
        &self,
        updates: Vec<(PurchaseOrderId, UpdatePurchaseOrder)>,
    ) -> Result<BatchResult<PurchaseOrder>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(po) => result.record_success(po),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(PurchaseOrderId, UpdatePurchaseOrder)>,
    ) -> Result<Vec<PurchaseOrder>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = chrono::Utc::now();
            let po = tx
                .query_row(
                    "SELECT * FROM purchase_orders WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_po,
                )
                .map_err(map_db_error)?;

            tx.execute(
                "UPDATE purchase_orders SET expected_date = ?, ship_to_address = ?, ship_to_city = ?,
                 ship_to_state = ?, ship_to_postal_code = ?, ship_to_country = ?, payment_terms = ?,
                 tax_amount = ?, shipping_cost = ?, discount_amount = ?, notes = ?, supplier_notes = ?,
                 supplier_reference = ?, updated_at = ? WHERE id = ?",
                params![
                    input
                        .expected_date
                        .map(|d| d.to_rfc3339())
                        .or(po.expected_date.map(|d| d.to_rfc3339())),
                    input.ship_to_address.or(po.ship_to_address),
                    input.ship_to_city.or(po.ship_to_city),
                    input.ship_to_state.or(po.ship_to_state),
                    input.ship_to_postal_code.or(po.ship_to_postal_code),
                    input.ship_to_country.or(po.ship_to_country),
                    input.payment_terms.unwrap_or(po.payment_terms).to_string(),
                    input.tax_amount.unwrap_or(po.tax_amount).to_string(),
                    input.shipping_cost.unwrap_or(po.shipping_cost).to_string(),
                    input.discount_amount.unwrap_or(po.discount_amount).to_string(),
                    input.notes.or(po.notes),
                    input.supplier_notes.or(po.supplier_notes),
                    input.supplier_reference.or(po.supplier_reference),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(map_db_error)?;

            Self::recalculate_totals_with_conn(&tx, id)?;

            let updated_po = Self::get_po_with_conn(&tx, id)?.ok_or(CommerceError::NotFound)?;
            results.push(updated_po);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<PurchaseOrderId>) -> Result<BatchResult<PurchaseOrderId>> {
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

    fn delete_batch_atomic(&self, ids: Vec<PurchaseOrderId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();

        // Verify all POs are in draft status before deleting
        for id in &ids {
            let status: String = tx
                .query_row(
                    "SELECT status FROM purchase_orders WHERE id = ?",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;

            let parsed_status: PurchaseOrderStatus = status.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid purchase_order.status '{status}': {e}"
                ))
            })?;
            if parsed_status != PurchaseOrderStatus::Draft {
                return Err(CommerceError::ValidationError(
                    "Can only delete draft purchase orders".to_string(),
                ));
            }
        }

        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        // Delete purchase order items first
        let sql =
            format!("DELETE FROM purchase_order_items WHERE purchase_order_id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete purchase orders
        let sql = format!("DELETE FROM purchase_orders WHERE id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<PurchaseOrderId>) -> Result<Vec<PurchaseOrder>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM purchase_orders WHERE id IN ({placeholders})");

        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_po).map_err(map_db_error)?;

        let mut orders = Vec::new();
        for row in rows {
            let mut po = row.map_err(map_db_error)?;
            po.items = Self::get_po_items_with_conn(&conn, po.id)?;
            orders.push(po);
        }

        Ok(orders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, PurchaseOrderFilter,
        PurchaseOrderRepository, PurchaseOrderStatus, ReceivePurchaseOrderItem,
        ReceivePurchaseOrderItems, SupplierFilter,
    };

    fn fresh_repo() -> SqlitePurchaseOrderRepository {
        SqliteDatabase::in_memory().expect("in-memory").purchase_orders()
    }

    fn make_supplier(repo: &SqlitePurchaseOrderRepository, name: &str) -> Supplier {
        repo.create_supplier(CreateSupplier {
            name: name.into(),
            supplier_code: None,
            contact_name: Some("Buyer Co".into()),
            email: Some("buyer@example.com".into()),
            phone: None,
            website: None,
            address: None,
            city: None,
            state: None,
            postal_code: None,
            country: Some("US".into()),
            tax_id: None,
            payment_terms: None,
            currency: None,
            lead_time_days: Some(7),
            minimum_order: None,
            notes: None,
        })
        .expect("create supplier")
    }

    fn make_po_item(sku: &str, qty: Decimal, cost: Decimal) -> CreatePurchaseOrderItem {
        CreatePurchaseOrderItem {
            sku: sku.into(),
            name: format!("Item {sku}"),
            quantity: qty,
            unit_cost: cost,
            unit_of_measure: Some("EA".into()),
            ..Default::default()
        }
    }

    #[test]
    fn create_supplier_persists_with_generated_code_when_omitted() {
        let repo = fresh_repo();
        let s = make_supplier(&repo, "ACME Corp");
        assert_eq!(s.name, "ACME Corp");
        assert!(!s.supplier_code.is_empty());
        let by_id = repo.get_supplier(s.id).expect("ok").expect("found");
        assert_eq!(by_id.id, s.id);
        let by_code = repo.get_supplier_by_code(&s.supplier_code).expect("ok").expect("found");
        assert_eq!(by_code.id, s.id);
    }

    #[test]
    fn list_suppliers_filters_by_name() {
        let repo = fresh_repo();
        make_supplier(&repo, "Acme Corp");
        make_supplier(&repo, "Acme Subsidiary");
        make_supplier(&repo, "Globex");

        let acmes = repo
            .list_suppliers(SupplierFilter { name: Some("Acme".into()), ..Default::default() })
            .expect("list");
        assert_eq!(acmes.len(), 2);
    }

    #[test]
    fn list_suppliers_applies_offset_and_default_limit() {
        let repo = fresh_repo();
        make_supplier(&repo, "S-a");
        make_supplier(&repo, "S-b");
        make_supplier(&repo, "S-c");

        // All three by default (ordered by name ASC).
        assert_eq!(repo.list_suppliers(SupplierFilter::default()).expect("all").len(), 3);

        // offset skips rows even when no explicit limit is given (Postgres applies
        // offset; SQLite used to ignore it unless a limit was also set).
        let offset1 = repo
            .list_suppliers(SupplierFilter { offset: Some(1), ..Default::default() })
            .expect("offset");
        assert_eq!(offset1.len(), 2, "offset must skip rows");

        // offset past the end returns nothing.
        let past = repo
            .list_suppliers(SupplierFilter { offset: Some(10), ..Default::default() })
            .expect("past");
        assert!(past.is_empty(), "offset past the end returns nothing");
    }

    #[test]
    fn create_po_starts_in_draft_with_lines() {
        let repo = fresh_repo();
        let supplier = make_supplier(&repo, "ACME");
        let po = repo
            .create(CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![
                    make_po_item("SKU-A", dec!(10), dec!(5)),
                    make_po_item("SKU-B", dec!(2), dec!(15)),
                ],
                ..Default::default()
            })
            .expect("create");
        assert_eq!(po.status, PurchaseOrderStatus::Draft);
        assert!(!po.po_number.is_empty());

        let items = repo.get_items(po.id).expect("items");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn get_and_get_by_number_round_trips() {
        let repo = fresh_repo();
        let supplier = make_supplier(&repo, "ACME");
        let po = repo
            .create(CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![make_po_item("SKU-X", dec!(1), dec!(1))],
                ..Default::default()
            })
            .expect("create");
        let by_id = repo.get(po.id).expect("ok").expect("found");
        assert_eq!(by_id.id, po.id);
        let by_num = repo.get_by_number(&po.po_number).expect("ok").expect("found");
        assert_eq!(by_num.id, po.id);
        assert!(repo.get_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn approve_transitions_status() {
        let repo = fresh_repo();
        let supplier = make_supplier(&repo, "ACME");
        let po = repo
            .create(CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![make_po_item("SKU-AP", dec!(1), dec!(10))],
                ..Default::default()
            })
            .expect("create");
        repo.submit_for_approval(po.id).expect("submit");
        let approved = repo.approve(po.id, "manager").expect("approve");
        assert_eq!(approved.status, PurchaseOrderStatus::Approved);

        // Guard: an illegal transition (Approved → PendingApproval) is rejected.
        let err = repo.submit_for_approval(po.id).expect_err("approved cannot re-enter approval");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    #[test]
    fn cancel_transitions_status() {
        let repo = fresh_repo();
        let supplier = make_supplier(&repo, "ACME");
        let po = repo
            .create(CreatePurchaseOrder {
                supplier_id: supplier.id,
                items: vec![make_po_item("SKU-CA", dec!(1), dec!(10))],
                ..Default::default()
            })
            .expect("create");
        let cancelled = repo.cancel(po.id).expect("cancel");
        assert_eq!(cancelled.status, PurchaseOrderStatus::Cancelled);
    }

    #[test]
    fn list_filters_by_supplier() {
        let repo = fresh_repo();
        let s1 = make_supplier(&repo, "S1");
        let s2 = make_supplier(&repo, "S2");
        repo.create(CreatePurchaseOrder {
            supplier_id: s1.id,
            items: vec![make_po_item("SKU-A", dec!(1), dec!(1))],
            ..Default::default()
        })
        .expect("c1");
        repo.create(CreatePurchaseOrder {
            supplier_id: s1.id,
            items: vec![make_po_item("SKU-B", dec!(1), dec!(1))],
            ..Default::default()
        })
        .expect("c2");
        repo.create(CreatePurchaseOrder {
            supplier_id: s2.id,
            items: vec![make_po_item("SKU-C", dec!(1), dec!(1))],
            ..Default::default()
        })
        .expect("c3");

        let for_s1 = repo
            .list(PurchaseOrderFilter { supplier_id: Some(s1.id), ..Default::default() })
            .expect("list");
        assert_eq!(for_s1.len(), 2);
    }

    #[test]
    fn receive_updates_quantities_and_rejects_over_receipt() {
        let repo = fresh_repo();
        let s = make_supplier(&repo, "RCV");
        let po = repo
            .create(CreatePurchaseOrder {
                supplier_id: s.id,
                items: vec![make_po_item("SKU-Q", dec!(10), dec!(1))],
                ..Default::default()
            })
            .expect("create po");
        let po_id = po.id;
        let item_id = po.items[0].id;
        let recv = |qty: Decimal| {
            repo.receive(
                po_id,
                ReceivePurchaseOrderItems {
                    items: vec![ReceivePurchaseOrderItem {
                        item_id,
                        quantity_received: qty,
                        notes: None,
                    }],
                    notes: None,
                },
            )
        };

        // Zero quantity is rejected.
        assert!(recv(dec!(0)).is_err(), "zero quantity must be rejected");

        // A partial receipt records the quantity and marks the PO PartiallyReceived.
        let after = recv(dec!(4)).expect("partial receive");
        assert_eq!(after.items[0].quantity_received, dec!(4));
        assert_eq!(after.status, PurchaseOrderStatus::PartiallyReceived);

        // Receiving more than the remaining (4 + 7 > 10) is rejected.
        assert!(recv(dec!(7)).is_err(), "over-receipt must be rejected");

        // Receiving exactly the remaining 6 fully receives the PO.
        let done = recv(dec!(6)).expect("final receive");
        assert_eq!(done.items[0].quantity_received, dec!(10));
        assert_eq!(done.status, PurchaseOrderStatus::Received);
    }

    #[test]
    fn receive_accumulates_concurrent_partial_receipts_without_lost_updates() {
        use std::sync::{Arc, Barrier};

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let s = make_supplier(&db.purchase_orders(), "RACE");
        let po = db
            .purchase_orders()
            .create(CreatePurchaseOrder {
                supplier_id: s.id,
                items: vec![make_po_item("SKU-R", dec!(100), dec!(1))],
                ..Default::default()
            })
            .expect("create po");
        let po_id = po.id;
        let item_id = po.items[0].id;

        // Fire many concurrent partial receipts of 2 each (all individually valid
        // against ordered 100). Every one that returns Ok must have actually added
        // its 2 to quantity_received; a racy read-check-write with an absolute
        // write would lose some, leaving received < 2 * successes.
        let n = 8;
        let barrier = Arc::new(Barrier::new(n));
        let handles: Vec<_> = (0..n)
            .map(|_| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    db.purchase_orders().receive(
                        po_id,
                        ReceivePurchaseOrderItems {
                            items: vec![ReceivePurchaseOrderItem {
                                item_id,
                                quantity_received: dec!(2),
                                notes: None,
                            }],
                            notes: None,
                        },
                    )
                })
            })
            .collect();

        let results: Vec<_> =
            handles.into_iter().map(|h| h.join().expect("thread panicked")).collect();
        // A transient lock error is acceptable only if the caller retries; the
        // retrying immediate transaction makes every receipt land.
        assert!(
            results.iter().all(|r| r.is_ok()
                || format!("{:?}", r.as_ref().unwrap_err()).to_lowercase().contains("lock")),
            "unexpected non-lock failure: {results:?}"
        );

        // Every one of the n receipts of 2 must land — no receipt lost to an
        // unretried lock conflict.
        let po = db.purchase_orders().get(po_id).expect("get").expect("po exists");
        assert_eq!(
            po.items[0].quantity_received,
            Decimal::from(n as u64) * dec!(2),
            "all {n} receipts of 2 must accumulate, got {}",
            po.items[0].quantity_received
        );
    }

    #[test]
    fn list_applies_offset_and_pagination() {
        let repo = fresh_repo();
        let s = make_supplier(&repo, "PAGER");
        for i in 0..3 {
            repo.create(CreatePurchaseOrder {
                supplier_id: s.id,
                items: vec![make_po_item(&format!("SKU-{i}"), dec!(1), dec!(1))],
                ..Default::default()
            })
            .expect("create po");
        }

        let base = PurchaseOrderFilter { supplier_id: Some(s.id), ..Default::default() };

        // No pagination → all three.
        assert_eq!(repo.list(base.clone()).expect("all").len(), 3);

        // offset must skip rows (Postgres applies it; SQLite used to ignore it).
        let offset1 = repo.list(PurchaseOrderFilter { offset: Some(1), ..base }).expect("offset");
        assert_eq!(offset1.len(), 2, "offset must skip rows");

        // limit + offset selects a page.
        let page = repo
            .list(PurchaseOrderFilter { limit: Some(2), offset: Some(1), ..base })
            .expect("page");
        assert_eq!(page.len(), 2);

        // offset past the end returns nothing.
        let past = repo.list(PurchaseOrderFilter { offset: Some(10), ..base }).expect("past");
        assert!(past.is_empty(), "offset past the end returns nothing");
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let s = make_supplier(&repo, "ACME");
        let po_draft = repo
            .create(CreatePurchaseOrder {
                supplier_id: s.id,
                items: vec![make_po_item("SKU-D", dec!(1), dec!(1))],
                ..Default::default()
            })
            .expect("c1");
        let po_to_approve = repo
            .create(CreatePurchaseOrder {
                supplier_id: s.id,
                items: vec![make_po_item("SKU-E", dec!(1), dec!(1))],
                ..Default::default()
            })
            .expect("c2");
        repo.submit_for_approval(po_to_approve.id).expect("submit");
        repo.approve(po_to_approve.id, "manager").expect("approve");

        let drafts = repo
            .list(PurchaseOrderFilter {
                status: Some(PurchaseOrderStatus::Draft),
                ..Default::default()
            })
            .expect("drafts");
        let approved = repo
            .list(PurchaseOrderFilter {
                status: Some(PurchaseOrderStatus::Approved),
                ..Default::default()
            })
            .expect("approved");
        assert!(drafts.iter().any(|p| p.id == po_draft.id));
        assert!(approved.iter().any(|p| p.id == po_to_approve.id));
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let supplier = make_supplier(&repo, "ACME");
        let result = repo
            .create_batch(vec![
                CreatePurchaseOrder {
                    supplier_id: supplier.id,
                    items: vec![make_po_item("SKU-1", dec!(1), dec!(1))],
                    ..Default::default()
                },
                CreatePurchaseOrder {
                    supplier_id: supplier.id,
                    items: vec![make_po_item("SKU-2", dec!(2), dec!(2))],
                    ..Default::default()
                },
            ])
            .expect("batch");
        assert_eq!(result.success_count, 2);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(PurchaseOrderId::new()).expect("ok").is_none());
    }

    #[test]
    fn get_supplier_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_supplier(Uuid::new_v4()).expect("ok").is_none());
    }
}
