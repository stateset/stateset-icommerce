//! SQLite implementation of invoice repository

use super::{map_db_error, parse_decimal};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::{params, Row};
use stateset_core::{
    CommerceError, CreateInvoice, CreateInvoiceItem, Invoice, InvoiceFilter,
    InvoiceItem, InvoiceRepository, InvoiceStatus,
    RecordInvoicePayment, Result, UpdateInvoice, generate_invoice_number,
};
use uuid::Uuid;

pub struct SqliteInvoiceRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteInvoiceRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_invoice(row: &Row) -> rusqlite::Result<Invoice> {
        Ok(Invoice {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            invoice_number: row.get("invoice_number")?,
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
            order_id: row.get::<_, Option<String>>("order_id")?.and_then(|s| s.parse().ok()),
            status: row.get::<_, String>("status")?.parse().unwrap_or_default(),
            invoice_type: row.get::<_, String>("invoice_type")?.parse().unwrap_or_default(),
            invoice_date: row.get::<_, String>("invoice_date")?.parse().unwrap_or_default(),
            due_date: row.get::<_, String>("due_date")?.parse().unwrap_or_default(),
            payment_terms: row.get("payment_terms")?,
            currency: row.get("currency")?,
            billing_name: row.get("billing_name")?,
            billing_email: row.get("billing_email")?,
            billing_address: row.get("billing_address")?,
            billing_city: row.get("billing_city")?,
            billing_state: row.get("billing_state")?,
            billing_postal_code: row.get("billing_postal_code")?,
            billing_country: row.get("billing_country")?,
            subtotal: parse_decimal(&row.get::<_, String>("subtotal")?),
            discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
            discount_percent: row.get::<_, Option<String>>("discount_percent")?.map(|s| parse_decimal(&s)),
            tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
            tax_rate: row.get::<_, Option<String>>("tax_rate")?.map(|s| parse_decimal(&s)),
            shipping_amount: parse_decimal(&row.get::<_, String>("shipping_amount")?),
            total: parse_decimal(&row.get::<_, String>("total")?),
            amount_paid: parse_decimal(&row.get::<_, String>("amount_paid")?),
            balance_due: parse_decimal(&row.get::<_, String>("balance_due")?),
            po_number: row.get("po_number")?,
            notes: row.get("notes")?,
            terms: row.get("terms")?,
            footer: row.get("footer")?,
            sent_at: row.get::<_, Option<String>>("sent_at")?.and_then(|s| s.parse().ok()),
            viewed_at: row.get::<_, Option<String>>("viewed_at")?.and_then(|s| s.parse().ok()),
            paid_at: row.get::<_, Option<String>>("paid_at")?.and_then(|s| s.parse().ok()),
            voided_at: row.get::<_, Option<String>>("voided_at")?.and_then(|s| s.parse().ok()),
            items: Vec::new(),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn row_to_invoice_item(row: &Row) -> rusqlite::Result<InvoiceItem> {
        Ok(InvoiceItem {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            invoice_id: row.get::<_, String>("invoice_id")?.parse().unwrap_or_default(),
            order_item_id: row.get::<_, Option<String>>("order_item_id")?.and_then(|s| s.parse().ok()),
            product_id: row.get::<_, Option<String>>("product_id")?.and_then(|s| s.parse().ok()),
            sku: row.get("sku")?,
            description: row.get("description")?,
            quantity: parse_decimal(&row.get::<_, String>("quantity")?),
            unit_of_measure: row.get("unit_of_measure")?,
            unit_price: parse_decimal(&row.get::<_, String>("unit_price")?),
            discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
            tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
            line_total: parse_decimal(&row.get::<_, String>("line_total")?),
            sort_order: row.get("sort_order")?,
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_default(),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_default(),
        })
    }

    fn get_invoice_items_with_conn(
        conn: &rusqlite::Connection,
        invoice_id: Uuid,
    ) -> Result<Vec<InvoiceItem>> {
        let mut stmt = conn
            .prepare("SELECT * FROM invoice_items WHERE invoice_id = ? ORDER BY sort_order")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([invoice_id.to_string()], Self::row_to_invoice_item)
            .map_err(map_db_error)?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(map_db_error)?);
        }
        Ok(items)
    }

    fn get_invoice_with_conn(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<Invoice>> {
        let result = conn.query_row(
            "SELECT * FROM invoices WHERE id = ?",
            [id.to_string()],
            Self::row_to_invoice,
        );

        match result {
            Ok(mut invoice) => {
                invoice.items = Self::get_invoice_items_with_conn(conn, id)?;
                Ok(Some(invoice))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn recalculate_with_conn(conn: &rusqlite::Connection, id: Uuid) -> Result<()> {
        // Calculate subtotal from items
        let subtotal: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(CAST(line_total AS REAL)), 0) FROM invoice_items WHERE invoice_id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let (discount_amount, tax_amount, shipping_amount, amount_paid): (String, String, String, String) = conn
            .query_row(
                "SELECT discount_amount, tax_amount, shipping_amount, amount_paid FROM invoices WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let subtotal_dec = Decimal::from_f64_retain(subtotal).unwrap_or_default();
        let total = subtotal_dec - parse_decimal(&discount_amount)
            + parse_decimal(&tax_amount)
            + parse_decimal(&shipping_amount);
        let balance_due = total - parse_decimal(&amount_paid);

        conn.execute(
            "UPDATE invoices SET subtotal = ?, total = ?, balance_due = ?, updated_at = ? WHERE id = ?",
            params![
                subtotal_dec.to_string(),
                total.to_string(),
                balance_due.to_string(),
                chrono::Utc::now().to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn get_invoice_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        let conn = self.conn()?;
        Self::get_invoice_items_with_conn(&conn, invoice_id)
    }
}

impl InvoiceRepository for SqliteInvoiceRepository {
    fn create(&self, input: CreateInvoice) -> Result<Invoice> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let invoice_number = generate_invoice_number();
        let invoice_date = input.invoice_date.unwrap_or(now);
        let due_date = input.due_date.unwrap_or_else(|| {
            invoice_date + chrono::Duration::days(input.days_until_due.unwrap_or(30) as i64)
        });

        tx.execute(
            "INSERT INTO invoices (id, invoice_number, customer_id, order_id, status, invoice_type,
             invoice_date, due_date, payment_terms, currency, billing_name, billing_email,
             billing_address, billing_city, billing_state, billing_postal_code, billing_country,
             subtotal, discount_amount, discount_percent, tax_amount, tax_rate, shipping_amount,
             total, amount_paid, balance_due, po_number, notes, terms, footer, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                invoice_number,
                input.customer_id.to_string(),
                input.order_id.map(|id| id.to_string()),
                InvoiceStatus::Draft.to_string(),
                input.invoice_type.unwrap_or_default().to_string(),
                invoice_date.to_rfc3339(),
                due_date.to_rfc3339(),
                input.payment_terms,
                input.currency.unwrap_or_else(|| "USD".to_string()),
                input.billing_name,
                input.billing_email,
                input.billing_address,
                input.billing_city,
                input.billing_state,
                input.billing_postal_code,
                input.billing_country,
                "0",
                input.discount_amount.unwrap_or_default().to_string(),
                input.discount_percent.map(|d| d.to_string()),
                input.tax_amount.unwrap_or_default().to_string(),
                input.tax_rate.map(|d| d.to_string()),
                input.shipping_amount.unwrap_or_default().to_string(),
                "0",
                "0",
                "0",
                input.po_number,
                input.notes,
                input.terms,
                input.footer,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        // Add items
        for (i, item) in input.items.into_iter().enumerate() {
            let item_id = Uuid::new_v4();
            let mut item_with_order = item;
            if item_with_order.sort_order.is_none() {
                item_with_order.sort_order = Some(i as i32);
            }

            let line_total = item_with_order.quantity * item_with_order.unit_price
                - item_with_order.discount_amount.unwrap_or_default()
                + item_with_order.tax_amount.unwrap_or_default();

            tx.execute(
                "INSERT INTO invoice_items (id, invoice_id, order_item_id, product_id, sku, description,
                 quantity, unit_of_measure, unit_price, discount_amount, tax_amount, line_total,
                 sort_order, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    item_id.to_string(),
                    id.to_string(),
                    item_with_order.order_item_id.map(|id| id.to_string()),
                    item_with_order.product_id.map(|id| id.to_string()),
                    item_with_order.sku,
                    item_with_order.description,
                    item_with_order.quantity.to_string(),
                    item_with_order.unit_of_measure,
                    item_with_order.unit_price.to_string(),
                    item_with_order.discount_amount.unwrap_or_default().to_string(),
                    item_with_order.tax_amount.unwrap_or_default().to_string(),
                    line_total.to_string(),
                    item_with_order.sort_order.unwrap_or(0),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        }

        // Recalculate totals
        Self::recalculate_with_conn(&tx, id)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn get(&self, id: Uuid) -> Result<Option<Invoice>> {
        let conn = self.conn()?;
        Self::get_invoice_with_conn(&conn, id)
    }

    fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM invoices WHERE invoice_number = ?",
            [invoice_number],
            Self::row_to_invoice,
        );

        match result {
            Ok(mut invoice) => {
                invoice.items = Self::get_invoice_items_with_conn(&conn, invoice.id)?;
                Ok(Some(invoice))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();
        let invoice = tx
            .query_row(
                "SELECT * FROM invoices WHERE id = ?",
                [id.to_string()],
                Self::row_to_invoice,
            )
            .map_err(map_db_error)?;

        tx.execute(
            "UPDATE invoices SET due_date = ?, payment_terms = ?, billing_name = ?, billing_email = ?,
             billing_address = ?, billing_city = ?, billing_state = ?, billing_postal_code = ?,
             billing_country = ?, discount_amount = ?, discount_percent = ?, tax_amount = ?,
             tax_rate = ?, shipping_amount = ?, po_number = ?, notes = ?, terms = ?, footer = ?,
             updated_at = ? WHERE id = ?",
            params![
                input.due_date.map(|d| d.to_rfc3339()).unwrap_or_else(|| invoice.due_date.to_rfc3339()),
                input.payment_terms.or(invoice.payment_terms),
                input.billing_name.or(invoice.billing_name),
                input.billing_email.or(invoice.billing_email),
                input.billing_address.or(invoice.billing_address),
                input.billing_city.or(invoice.billing_city),
                input.billing_state.or(invoice.billing_state),
                input.billing_postal_code.or(invoice.billing_postal_code),
                input.billing_country.or(invoice.billing_country),
                input.discount_amount.unwrap_or(invoice.discount_amount).to_string(),
                input.discount_percent.map(|d| d.to_string()).or(invoice.discount_percent.map(|d| d.to_string())),
                input.tax_amount.unwrap_or(invoice.tax_amount).to_string(),
                input.tax_rate.map(|d| d.to_string()).or(invoice.tax_rate.map(|d| d.to_string())),
                input.shipping_amount.unwrap_or(invoice.shipping_amount).to_string(),
                input.po_number.or(invoice.po_number),
                input.notes.or(invoice.notes),
                input.terms.or(invoice.terms),
                input.footer.or(invoice.footer),
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        Self::recalculate_with_conn(&tx, id)?;
        tx.commit().map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        let conn = self.conn()?;

        let mut sql = "SELECT * FROM invoices WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }
        if filter.overdue_only.unwrap_or(false) {
            sql.push_str(" AND due_date < datetime('now') AND status NOT IN ('paid', 'voided')");
        }

        sql.push_str(" ORDER BY invoice_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_invoice).map_err(map_db_error)?;

        let mut invoices = Vec::new();
        for row in rows {
            let mut invoice = row.map_err(map_db_error)?;
            invoice.items = Self::get_invoice_items_with_conn(&conn, invoice.id)?;
            invoices.push(invoice);
        }
        Ok(invoices)
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Invoice>> {
        self.list(InvoiceFilter { customer_id: Some(customer_id), ..Default::default() })
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Invoice>> {
        self.list(InvoiceFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let status: String = tx
            .query_row(
                "SELECT status FROM invoices WHERE id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if status.parse::<InvoiceStatus>().unwrap_or_default() != InvoiceStatus::Draft {
            return Err(CommerceError::ValidationError("Can only delete draft invoices".to_string()));
        }

        tx.execute("DELETE FROM invoice_items WHERE invoice_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.execute("DELETE FROM invoices WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn send(&self, id: Uuid) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, sent_at = ?, updated_at = ? WHERE id = ?",
            params![InvoiceStatus::Sent.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_viewed(&self, id: Uuid) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = CASE WHEN status = 'sent' THEN 'viewed' ELSE status END,
             viewed_at = COALESCE(viewed_at, ?), updated_at = ? WHERE id = ?",
            params![now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn record_payment(&self, id: Uuid, payment: RecordInvoicePayment) -> Result<Invoice> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = chrono::Utc::now();

        let (total, amount_paid): (String, String) = tx
            .query_row(
                "SELECT total, amount_paid FROM invoices WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(map_db_error)?;

        let total_dec = parse_decimal(&total);
        let amount_paid_dec = parse_decimal(&amount_paid);

        let new_amount_paid = amount_paid_dec + payment.amount;
        let new_balance = total_dec - new_amount_paid;

        let new_status = if new_balance <= Decimal::ZERO {
            InvoiceStatus::Paid
        } else {
            InvoiceStatus::PartiallyPaid
        };

        let paid_at = if new_status == InvoiceStatus::Paid {
            Some(now)
        } else {
            None
        };

        tx.execute(
            "UPDATE invoices SET amount_paid = ?, balance_due = ?, status = ?,
             paid_at = COALESCE(?, paid_at), updated_at = ? WHERE id = ?",
            params![
                new_amount_paid.to_string(),
                new_balance.to_string(),
                new_status.to_string(),
                paid_at.map(|d| d.to_rfc3339()),
                now.to_rfc3339(),
                id.to_string(),
            ],
        ).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn void(&self, id: Uuid) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, voided_at = ?, updated_at = ? WHERE id = ?",
            params![InvoiceStatus::Voided.to_string(), now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn write_off(&self, id: Uuid) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, updated_at = ? WHERE id = ?",
            params![InvoiceStatus::WrittenOff.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn dispute(&self, id: Uuid) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, updated_at = ? WHERE id = ?",
            params![InvoiceStatus::Disputed.to_string(), now.to_rfc3339(), id.to_string()],
        ).map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn add_item(&self, invoice_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_price - item.discount_amount.unwrap_or_default() + item.tax_amount.unwrap_or_default();

        conn.execute(
            "INSERT INTO invoice_items (id, invoice_id, order_item_id, product_id, sku, description,
             quantity, unit_of_measure, unit_price, discount_amount, tax_amount, line_total,
             sort_order, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                id.to_string(),
                invoice_id.to_string(),
                item.order_item_id.map(|id| id.to_string()),
                item.product_id.map(|id| id.to_string()),
                item.sku,
                item.description,
                item.quantity.to_string(),
                item.unit_of_measure,
                item.unit_price.to_string(),
                item.discount_amount.unwrap_or_default().to_string(),
                item.tax_amount.unwrap_or_default().to_string(),
                line_total.to_string(),
                item.sort_order.unwrap_or(0),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        let mut stmt = conn.prepare("SELECT * FROM invoice_items WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_invoice_item).map_err(map_db_error)
    }

    fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_price - item.discount_amount.unwrap_or_default() + item.tax_amount.unwrap_or_default();

        conn.execute(
            "UPDATE invoice_items SET sku = ?, description = ?, quantity = ?, unit_of_measure = ?,
             unit_price = ?, discount_amount = ?, tax_amount = ?, line_total = ?, sort_order = ?,
             updated_at = ? WHERE id = ?",
            params![
                item.sku,
                item.description,
                item.quantity.to_string(),
                item.unit_of_measure,
                item.unit_price.to_string(),
                item.discount_amount.unwrap_or_default().to_string(),
                item.tax_amount.unwrap_or_default().to_string(),
                line_total.to_string(),
                item.sort_order.unwrap_or(0),
                now.to_rfc3339(),
                item_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        let mut stmt = conn.prepare("SELECT * FROM invoice_items WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([item_id.to_string()], Self::row_to_invoice_item).map_err(map_db_error)
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        conn.execute("DELETE FROM invoice_items WHERE id = ?", [item_id.to_string()]).map_err(map_db_error)?;
        Ok(())
    }

    fn get_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        self.get_invoice_items(invoice_id)
    }

    fn recalculate(&self, id: Uuid) -> Result<Invoice> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        Self::recalculate_with_conn(&tx, id)?;
        tx.commit().map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn get_overdue(&self) -> Result<Vec<Invoice>> {
        self.list(InvoiceFilter { overdue_only: Some(true), ..Default::default() })
    }

    fn count(&self, filter: InvoiceFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM invoices WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params_vec.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }
        if filter.overdue_only.unwrap_or(false) {
            sql.push_str(" AND due_date < datetime('now') AND status NOT IN ('paid', 'voided')");
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }
}
