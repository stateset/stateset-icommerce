//! SQLite implementation of purchase order repository

use super::{map_db_error, parse_decimal};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::{params, Row};
use stateset_core::{
    CommerceError, CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier,
    PurchaseOrder, PurchaseOrderFilter, PurchaseOrderItem,
    PurchaseOrderRepository, PurchaseOrderStatus, ReceivePurchaseOrderItems,
    Result, Supplier, SupplierFilter, UpdatePurchaseOrder, UpdateSupplier,
    generate_po_number, generate_supplier_code,
};
use uuid::Uuid;

pub struct SqlitePurchaseOrderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePurchaseOrderRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_supplier(row: &Row) -> rusqlite::Result<Supplier> {
        Ok(Supplier {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
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
            payment_terms: row.get::<_, String>("payment_terms")?.parse().unwrap_or_default(),
            currency: row.get("currency")?,
            lead_time_days: row.get("lead_time_days")?,
            minimum_order: row.get::<_, Option<String>>("minimum_order")?.map(|s| parse_decimal(&s)),
            is_active: row.get::<_, i32>("is_active")? != 0,
            notes: row.get("notes")?,
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn row_to_po(row: &Row) -> rusqlite::Result<PurchaseOrder> {
        Ok(PurchaseOrder {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            po_number: row.get("po_number")?,
            supplier_id: row.get::<_, String>("supplier_id")?.parse().unwrap_or_default(),
            status: row.get::<_, String>("status")?.parse().unwrap_or_default(),
            order_date: row.get::<_, String>("order_date")?.parse().unwrap_or_default(),
            expected_date: row.get::<_, Option<String>>("expected_date")?.and_then(|s| s.parse().ok()),
            delivered_date: row.get::<_, Option<String>>("delivered_date")?.and_then(|s| s.parse().ok()),
            ship_to_address: row.get("ship_to_address")?,
            ship_to_city: row.get("ship_to_city")?,
            ship_to_state: row.get("ship_to_state")?,
            ship_to_postal_code: row.get("ship_to_postal_code")?,
            ship_to_country: row.get("ship_to_country")?,
            payment_terms: row.get::<_, String>("payment_terms")?.parse().unwrap_or_default(),
            currency: row.get("currency")?,
            subtotal: parse_decimal(&row.get::<_, String>("subtotal")?),
            tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
            shipping_cost: parse_decimal(&row.get::<_, String>("shipping_cost")?),
            discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
            total: parse_decimal(&row.get::<_, String>("total")?),
            amount_paid: parse_decimal(&row.get::<_, String>("amount_paid")?),
            supplier_reference: row.get("supplier_reference")?,
            notes: row.get("notes")?,
            supplier_notes: row.get("supplier_notes")?,
            approved_by: row.get("approved_by")?,
            approved_at: row.get::<_, Option<String>>("approved_at")?.and_then(|s| s.parse().ok()),
            items: Vec::new(),
            sent_at: row.get::<_, Option<String>>("sent_at")?.and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn row_to_po_item(row: &Row) -> rusqlite::Result<PurchaseOrderItem> {
        Ok(PurchaseOrderItem {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            purchase_order_id: row.get::<_, String>("purchase_order_id")?.parse().unwrap_or_default(),
            product_id: row.get::<_, Option<String>>("product_id")?.and_then(|s| s.parse().ok()),
            sku: row.get("sku")?,
            name: row.get("name")?,
            supplier_sku: row.get("supplier_sku")?,
            quantity_ordered: parse_decimal(&row.get::<_, String>("quantity_ordered")?),
            quantity_received: parse_decimal(&row.get::<_, String>("quantity_received")?),
            unit_of_measure: row.get("unit_of_measure")?,
            unit_cost: parse_decimal(&row.get::<_, String>("unit_cost")?),
            line_total: parse_decimal(&row.get::<_, String>("line_total")?),
            tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
            discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
            expected_date: row.get::<_, Option<String>>("expected_date")?.and_then(|s| s.parse().ok()),
            notes: row.get("notes")?,
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
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
        po_id: Uuid,
    ) -> Result<Vec<PurchaseOrderItem>> {
        let mut stmt = conn
            .prepare("SELECT * FROM purchase_order_items WHERE purchase_order_id = ?")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([po_id.to_string()], Self::row_to_po_item)
            .map_err(map_db_error)?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(map_db_error)?);
        }
        Ok(items)
    }

    fn get_po_with_conn(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<PurchaseOrder>> {
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

    fn get_po_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        let conn = self.conn()?;
        Self::get_po_items_with_conn(&conn, po_id)
    }

    fn recalculate_totals_with_conn(conn: &rusqlite::Connection, po_id: Uuid) -> Result<()> {
        // Calculate subtotal from items
        let subtotal: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(CAST(line_total AS REAL)), 0) FROM purchase_order_items WHERE purchase_order_id = ?",
                [po_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let (tax_amount, shipping_cost, discount_amount): (String, String, String) = conn
            .query_row(
                "SELECT tax_amount, shipping_cost, discount_amount FROM purchase_orders WHERE id = ?",
                [po_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;

        let subtotal_dec = Decimal::from_f64_retain(subtotal).unwrap_or_default();
        let total = subtotal_dec + parse_decimal(&tax_amount) + parse_decimal(&shipping_cost)
            - parse_decimal(&discount_amount);

        conn.execute(
            "UPDATE purchase_orders SET subtotal = ?, total = ?, updated_at = ? WHERE id = ?",
            params![
                subtotal_dec.to_string(),
                total.to_string(),
                chrono::Utc::now().to_rfc3339(),
                po_id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn recalculate_totals(&self, po_id: Uuid) -> Result<()> {
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
                input.currency.unwrap_or_else(|| "USD".to_string()),
                input.lead_time_days,
                input.minimum_order.map(|d| d.to_string()),
                1,
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

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
                input.minimum_order.map(|d| d.to_string()).or(supplier.minimum_order.map(|d| d.to_string())),
                input.is_active.unwrap_or(supplier.is_active) as i32,
                input.notes.or(supplier.notes),
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_supplier_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        let conn = self.conn()?;

        let mut sql = "SELECT * FROM suppliers WHERE 1=1".to_string();

        if filter.active_only.unwrap_or(false) {
            sql.push_str(" AND is_active = 1");
        }

        sql.push_str(" ORDER BY name ASC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map([], Self::row_to_supplier).map_err(map_db_error)?;

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
        ).map_err(map_db_error)?;
        Ok(())
    }

    fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get supplier for defaults
        let supplier =
            Self::get_supplier_with_conn(&tx, input.supplier_id)?.ok_or(CommerceError::NotFound)?;

        let id = Uuid::new_v4();
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
        ).map_err(map_db_error)?;

        // Add items
        for item in input.items {
            let item_id = Uuid::new_v4();
            let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default()
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

    fn get(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
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

    fn update(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
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
                input.expected_date.map(|d| d.to_rfc3339()).or(po.expected_date.map(|d| d.to_rfc3339())),
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
        ).map_err(map_db_error)?;

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

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
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

    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let status: String = tx
            .query_row(
                "SELECT status FROM purchase_orders WHERE id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if status.parse::<PurchaseOrderStatus>().unwrap_or_default() != PurchaseOrderStatus::Draft {
            return Err(CommerceError::ValidationError("Can only delete draft purchase orders".to_string()));
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

    fn submit_for_approval(&self, id: Uuid) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::PendingApproval.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn approve(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, approved_by = ?, approved_at = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Approved.to_string(), approved_by, now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn send(&self, id: Uuid) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, sent_at = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Sent.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn acknowledge(&self, id: Uuid, supplier_reference: Option<&str>) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, supplier_reference = COALESCE(?, supplier_reference), updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Acknowledged.to_string(), supplier_reference, now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn hold(&self, id: Uuid) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::OnHold.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn cancel(&self, id: Uuid) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Cancelled.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn receive(&self, id: Uuid, items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();

        let current_status: PurchaseOrderStatus = tx
            .query_row(
                "SELECT status FROM purchase_orders WHERE id = ?",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .map_err(map_db_error)?
            .parse()
            .unwrap_or_default();

        for item in items.items {
            tx.execute(
                "UPDATE purchase_order_items SET quantity_received = quantity_received + ?, updated_at = ? WHERE id = ?",
                params![item.quantity_received.to_string(), now.to_rfc3339(), item.item_id.to_string()],
            ).map_err(map_db_error)?;
        }

        // Check if fully or partially received
        let mut has_items = false;
        let mut all_received = true;
        let mut any_received = false;
        {
            let mut stmt = tx
                .prepare(
                    "SELECT quantity_ordered, quantity_received
                     FROM purchase_order_items
                     WHERE purchase_order_id = ?",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map([id.to_string()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .map_err(map_db_error)?;

            for row in rows {
                let (ordered, received) = row.map_err(map_db_error)?;
                let ordered_dec = parse_decimal(&ordered);
                let received_dec = parse_decimal(&received);

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
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn complete(&self, id: Uuid) -> Result<PurchaseOrder> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();
        conn.execute(
            "UPDATE purchase_orders SET status = ?, updated_at = ? WHERE id = ?",
            params![PurchaseOrderStatus::Completed.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;
        Self::get_po_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn add_item(&self, po_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default() + item.tax_amount.unwrap_or_default();

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
        ).map_err(map_db_error)?;

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

    fn update_item(&self, item_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default() + item.tax_amount.unwrap_or_default();

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
        ).map_err(map_db_error)?;

        Self::recalculate_totals_with_conn(&tx, po_id.parse().unwrap_or_default())?;

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

        tx.execute(
            "DELETE FROM purchase_order_items WHERE id = ?",
            [item_id.to_string()],
        )
        .map_err(map_db_error)?;

        Self::recalculate_totals_with_conn(&tx, po_id.parse().unwrap_or_default())?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
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

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
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
}
