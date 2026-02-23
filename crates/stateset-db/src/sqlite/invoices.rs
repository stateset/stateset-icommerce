//! SQLite implementation of invoice repository

use super::parse_helpers::parse_decimal as parse_decimal_with_context;
use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_enum, parse_enum_row, parse_uuid_opt_row,
    parse_uuid_row, sum_decimal_query, uuid_params,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Row, params};
use rust_decimal::Decimal;
use stateset_core::{
    BatchResult, CommerceError, CreateInvoice, CreateInvoiceItem, CustomerId, Invoice,
    InvoiceFilter, InvoiceId, InvoiceItem, InvoiceRepository, InvoiceStatus, OrderId, OrderItemId,
    ProductId, RecordInvoicePayment, Result, UpdateInvoice, generate_invoice_number,
    validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteInvoiceRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteInvoiceRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_invoice(row: &Row<'_>) -> rusqlite::Result<Invoice> {
        Ok(Invoice {
            id: InvoiceId::from(parse_uuid_row(&row.get::<_, String>("id")?, "invoice", "id")?),
            invoice_number: row.get("invoice_number")?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "invoice",
                "customer_id",
            )?),
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "invoice",
                "order_id",
            )?
            .map(OrderId::from),
            status: parse_enum_row(&row.get::<_, String>("status")?, "invoice", "status")?,
            invoice_type: parse_enum_row(
                &row.get::<_, String>("invoice_type")?,
                "invoice",
                "invoice_type",
            )?,
            invoice_date: parse_datetime_row(
                &row.get::<_, String>("invoice_date")?,
                "invoice",
                "invoice_date",
            )?,
            due_date: parse_datetime_row(
                &row.get::<_, String>("due_date")?,
                "invoice",
                "due_date",
            )?,
            payment_terms: row.get("payment_terms")?,
            currency: row.get("currency")?,
            billing_name: row.get("billing_name")?,
            billing_email: row.get("billing_email")?,
            billing_address: row.get("billing_address")?,
            billing_city: row.get("billing_city")?,
            billing_state: row.get("billing_state")?,
            billing_postal_code: row.get("billing_postal_code")?,
            billing_country: row.get("billing_country")?,
            subtotal: parse_decimal_row(&row.get::<_, String>("subtotal")?, "invoice", "subtotal")?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "invoice",
                "discount_amount",
            )?,
            discount_percent: parse_decimal_opt_row(
                row.get::<_, Option<String>>("discount_percent")?,
                "invoice",
                "discount_percent",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "invoice",
                "tax_amount",
            )?,
            tax_rate: parse_decimal_opt_row(
                row.get::<_, Option<String>>("tax_rate")?,
                "invoice",
                "tax_rate",
            )?,
            shipping_amount: parse_decimal_row(
                &row.get::<_, String>("shipping_amount")?,
                "invoice",
                "shipping_amount",
            )?,
            total: parse_decimal_row(&row.get::<_, String>("total")?, "invoice", "total")?,
            amount_paid: parse_decimal_row(
                &row.get::<_, String>("amount_paid")?,
                "invoice",
                "amount_paid",
            )?,
            balance_due: parse_decimal_row(
                &row.get::<_, String>("balance_due")?,
                "invoice",
                "balance_due",
            )?,
            po_number: row.get("po_number")?,
            notes: row.get("notes")?,
            terms: row.get("terms")?,
            footer: row.get("footer")?,
            sent_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("sent_at")?,
                "invoice",
                "sent_at",
            )?,
            viewed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("viewed_at")?,
                "invoice",
                "viewed_at",
            )?,
            paid_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("paid_at")?,
                "invoice",
                "paid_at",
            )?,
            voided_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("voided_at")?,
                "invoice",
                "voided_at",
            )?,
            items: Vec::new(),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "invoice",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "invoice",
                "updated_at",
            )?,
        })
    }

    fn row_to_invoice_item(row: &Row<'_>) -> rusqlite::Result<InvoiceItem> {
        Ok(InvoiceItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "invoice_item", "id")?,
            invoice_id: InvoiceId::from(parse_uuid_row(
                &row.get::<_, String>("invoice_id")?,
                "invoice_item",
                "invoice_id",
            )?),
            order_item_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_item_id")?,
                "invoice_item",
                "order_item_id",
            )?
            .map(OrderItemId::from),
            product_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("product_id")?,
                "invoice_item",
                "product_id",
            )?
            .map(ProductId::from),
            sku: row.get("sku")?,
            description: row.get("description")?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "invoice_item",
                "quantity",
            )?,
            unit_of_measure: row.get("unit_of_measure")?,
            unit_price: parse_decimal_row(
                &row.get::<_, String>("unit_price")?,
                "invoice_item",
                "unit_price",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "invoice_item",
                "discount_amount",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "invoice_item",
                "tax_amount",
            )?,
            line_total: parse_decimal_row(
                &row.get::<_, String>("line_total")?,
                "invoice_item",
                "line_total",
            )?,
            sort_order: row.get("sort_order")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "invoice_item",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "invoice_item",
                "updated_at",
            )?,
        })
    }

    fn get_invoice_items_with_conn(
        conn: &rusqlite::Connection,
        invoice_id: InvoiceId,
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

    fn get_invoice_with_conn(conn: &rusqlite::Connection, id: InvoiceId) -> Result<Option<Invoice>> {
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

    fn recalculate_with_conn(conn: &rusqlite::Connection, id: InvoiceId) -> Result<()> {
        // Calculate subtotal from items
        let invoice_id_param = id.to_string();
        let invoice_params: [&dyn rusqlite::ToSql; 1] = [&invoice_id_param];
        let subtotal = sum_decimal_query(
            conn,
            "SELECT line_total FROM invoice_items WHERE invoice_id = ?",
            &invoice_params,
            "invoice_item",
            "line_total",
        )?;

        let (discount_amount, tax_amount, shipping_amount, amount_paid): (String, String, String, String) = conn
            .query_row(
                "SELECT discount_amount, tax_amount, shipping_amount, amount_paid FROM invoices WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let total = subtotal
            - parse_decimal_with_context(&discount_amount, "invoice", "discount_amount")?
            + parse_decimal_with_context(&tax_amount, "invoice", "tax_amount")?
            + parse_decimal_with_context(&shipping_amount, "invoice", "shipping_amount")?;
        let balance_due =
            total - parse_decimal_with_context(&amount_paid, "invoice", "amount_paid")?;

        conn.execute(
            "UPDATE invoices SET subtotal = ?, total = ?, balance_due = ?, updated_at = ? WHERE id = ?",
            params![
                subtotal.to_string(),
                total.to_string(),
                balance_due.to_string(),
                chrono::Utc::now().to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn get_invoice_items(&self, invoice_id: InvoiceId) -> Result<Vec<InvoiceItem>> {
        let conn = self.conn()?;
        Self::get_invoice_items_with_conn(&conn, invoice_id)
    }
}

impl InvoiceRepository for SqliteInvoiceRepository {
    fn create(&self, input: CreateInvoice) -> Result<Invoice> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = InvoiceId::new();
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

    fn get(&self, id: InvoiceId) -> Result<Option<Invoice>> {
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

    fn update(&self, id: InvoiceId, input: UpdateInvoice) -> Result<Invoice> {
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
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows =
            stmt.query_map(params_refs.as_slice(), Self::row_to_invoice).map_err(map_db_error)?;

        let mut invoices = Vec::new();
        for row in rows {
            let mut invoice = row.map_err(map_db_error)?;
            invoice.items = Self::get_invoice_items_with_conn(&conn, invoice.id)?;
            invoices.push(invoice);
        }
        Ok(invoices)
    }

    fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Invoice>> {
        self.list(InvoiceFilter { customer_id: Some(customer_id), ..Default::default() })
    }

    fn for_order(&self, order_id: OrderId) -> Result<Vec<Invoice>> {
        self.list(InvoiceFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn delete(&self, id: InvoiceId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        let status: String = tx
            .query_row("SELECT status FROM invoices WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .map_err(map_db_error)?;

        if parse_enum::<InvoiceStatus>(&status, "invoice", "status")? != InvoiceStatus::Draft {
            return Err(CommerceError::ValidationError(
                "Can only delete draft invoices".to_string(),
            ));
        }

        tx.execute("DELETE FROM invoice_items WHERE invoice_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.execute("DELETE FROM invoices WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn send(&self, id: InvoiceId) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, sent_at = ?, updated_at = ? WHERE id = ?",
            params![
                InvoiceStatus::Sent.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn mark_viewed(&self, id: InvoiceId) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = CASE WHEN status = 'sent' THEN 'viewed' ELSE status END,
             viewed_at = COALESCE(viewed_at, ?), updated_at = ? WHERE id = ?",
            params![now.to_rfc3339(), now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn record_payment(&self, id: InvoiceId, payment: RecordInvoicePayment) -> Result<Invoice> {
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

        let total_dec = parse_decimal_with_context(&total, "invoice", "total")?;
        let amount_paid_dec = parse_decimal_with_context(&amount_paid, "invoice", "amount_paid")?;

        let new_amount_paid = amount_paid_dec + payment.amount;
        let new_balance = total_dec - new_amount_paid;

        let new_status = if new_balance <= Decimal::ZERO {
            InvoiceStatus::Paid
        } else {
            InvoiceStatus::PartiallyPaid
        };

        let paid_at = if new_status == InvoiceStatus::Paid { Some(now) } else { None };

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
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn void(&self, id: InvoiceId) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, voided_at = ?, updated_at = ? WHERE id = ?",
            params![
                InvoiceStatus::Voided.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn write_off(&self, id: InvoiceId) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, updated_at = ? WHERE id = ?",
            params![InvoiceStatus::WrittenOff.to_string(), now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn dispute(&self, id: InvoiceId) -> Result<Invoice> {
        let conn = self.conn()?;
        let now = chrono::Utc::now();

        conn.execute(
            "UPDATE invoices SET status = ?, updated_at = ? WHERE id = ?",
            params![InvoiceStatus::Disputed.to_string(), now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        Self::get_invoice_with_conn(&conn, id)?.ok_or(CommerceError::NotFound)
    }

    fn add_item(&self, invoice_id: InvoiceId, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_price - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

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

        let mut stmt =
            conn.prepare("SELECT * FROM invoice_items WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_invoice_item).map_err(map_db_error)
    }

    fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now();
        let line_total = item.quantity * item.unit_price - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

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
        )
        .map_err(map_db_error)?;

        let mut stmt =
            conn.prepare("SELECT * FROM invoice_items WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([item_id.to_string()], Self::row_to_invoice_item).map_err(map_db_error)
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        conn.execute("DELETE FROM invoice_items WHERE id = ?", [item_id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn get_items(&self, invoice_id: InvoiceId) -> Result<Vec<InvoiceItem>> {
        self.get_invoice_items(invoice_id)
    }

    fn recalculate(&self, id: InvoiceId) -> Result<Invoice> {
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

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateInvoice>) -> Result<BatchResult<Invoice>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(invoice) => result.record_success(invoice),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateInvoice>) -> Result<Vec<Invoice>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = InvoiceId::new();
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
                    invoice_number.clone(),
                    input.customer_id.to_string(),
                    input.order_id.map(|id| id.to_string()),
                    InvoiceStatus::Draft.to_string(),
                    input.invoice_type.unwrap_or_default().to_string(),
                    invoice_date.to_rfc3339(),
                    due_date.to_rfc3339(),
                    input.payment_terms.clone(),
                    input.currency.clone().unwrap_or_else(|| "USD".to_string()),
                    input.billing_name.clone(),
                    input.billing_email.clone(),
                    input.billing_address.clone(),
                    input.billing_city.clone(),
                    input.billing_state.clone(),
                    input.billing_postal_code.clone(),
                    input.billing_country.clone(),
                    "0",
                    input.discount_amount.unwrap_or_default().to_string(),
                    input.discount_percent.map(|d| d.to_string()),
                    input.tax_amount.unwrap_or_default().to_string(),
                    input.tax_rate.map(|d| d.to_string()),
                    input.shipping_amount.unwrap_or_default().to_string(),
                    "0",
                    "0",
                    "0",
                    input.po_number.clone(),
                    input.notes.clone(),
                    input.terms.clone(),
                    input.footer.clone(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;

            // Add items
            let mut items = Vec::with_capacity(input.items.len());
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
                        item_with_order.sku.clone(),
                        item_with_order.description.clone(),
                        item_with_order.quantity.to_string(),
                        item_with_order.unit_of_measure.clone(),
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

                items.push(InvoiceItem {
                    id: item_id,
                    invoice_id: id,
                    order_item_id: item_with_order.order_item_id,
                    product_id: item_with_order.product_id,
                    sku: item_with_order.sku,
                    description: item_with_order.description,
                    quantity: item_with_order.quantity,
                    unit_of_measure: item_with_order.unit_of_measure,
                    unit_price: item_with_order.unit_price,
                    discount_amount: item_with_order.discount_amount.unwrap_or_default(),
                    tax_amount: item_with_order.tax_amount.unwrap_or_default(),
                    line_total,
                    sort_order: item_with_order.sort_order.unwrap_or(0),
                    created_at: now,
                    updated_at: now,
                });
            }

            // Recalculate totals
            Self::recalculate_with_conn(&tx, id)?;

            // Query for the computed invoice values
            let invoice = tx
                .query_row(
                    "SELECT * FROM invoices WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_invoice,
                )
                .map_err(map_db_error)?;

            results.push(Invoice {
                id,
                invoice_number,
                customer_id: input.customer_id,
                order_id: input.order_id,
                status: InvoiceStatus::Draft,
                invoice_type: input.invoice_type.unwrap_or_default(),
                invoice_date,
                due_date,
                payment_terms: input.payment_terms,
                currency: input.currency.unwrap_or_else(|| "USD".to_string()),
                billing_name: input.billing_name,
                billing_email: input.billing_email,
                billing_address: input.billing_address,
                billing_city: input.billing_city,
                billing_state: input.billing_state,
                billing_postal_code: input.billing_postal_code,
                billing_country: input.billing_country,
                subtotal: invoice.subtotal,
                discount_amount: input.discount_amount.unwrap_or_default(),
                discount_percent: input.discount_percent,
                tax_amount: input.tax_amount.unwrap_or_default(),
                tax_rate: input.tax_rate,
                shipping_amount: input.shipping_amount.unwrap_or_default(),
                total: invoice.total,
                amount_paid: Decimal::ZERO,
                balance_due: invoice.balance_due,
                po_number: input.po_number,
                notes: input.notes,
                terms: input.terms,
                footer: input.footer,
                sent_at: None,
                viewed_at: None,
                paid_at: None,
                voided_at: None,
                items,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(InvoiceId, UpdateInvoice)>) -> Result<BatchResult<Invoice>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(invoice) => result.record_success(invoice),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(InvoiceId, UpdateInvoice)>) -> Result<Vec<Invoice>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
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
                    input.payment_terms.or(invoice.payment_terms.clone()),
                    input.billing_name.or(invoice.billing_name.clone()),
                    input.billing_email.or(invoice.billing_email.clone()),
                    input.billing_address.or(invoice.billing_address.clone()),
                    input.billing_city.or(invoice.billing_city.clone()),
                    input.billing_state.or(invoice.billing_state.clone()),
                    input.billing_postal_code.or(invoice.billing_postal_code.clone()),
                    input.billing_country.or(invoice.billing_country.clone()),
                    input.discount_amount.unwrap_or(invoice.discount_amount).to_string(),
                    input.discount_percent.map(|d| d.to_string()).or(invoice.discount_percent.map(|d| d.to_string())),
                    input.tax_amount.unwrap_or(invoice.tax_amount).to_string(),
                    input.tax_rate.map(|d| d.to_string()).or(invoice.tax_rate.map(|d| d.to_string())),
                    input.shipping_amount.unwrap_or(invoice.shipping_amount).to_string(),
                    input.po_number.or(invoice.po_number.clone()),
                    input.notes.or(invoice.notes.clone()),
                    input.terms.or(invoice.terms.clone()),
                    input.footer.or(invoice.footer.clone()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            ).map_err(map_db_error)?;

            Self::recalculate_with_conn(&tx, id)?;

            let updated_invoice = tx
                .query_row(
                    "SELECT * FROM invoices WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_invoice,
                )
                .map_err(map_db_error)?;

            results.push(updated_invoice);
        }

        tx.commit().map_err(map_db_error)?;

        // Load items for each invoice
        let conn = self.conn()?;
        for invoice in &mut results {
            invoice.items = Self::get_invoice_items_with_conn(&conn, invoice.id)?;
        }

        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<InvoiceId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id.into_uuid()),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<InvoiceId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Check that all invoices are in Draft status before deleting
        let placeholders = build_in_clause(ids.len());
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        {
            let sql = format!("SELECT id, status FROM invoices WHERE id IN ({})", placeholders);
            let mut stmt = tx.prepare(&sql).map_err(map_db_error)?;
            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(map_db_error)?;

            for row in rows {
                let (id_str, status) = row.map_err(map_db_error)?;
                if parse_enum::<InvoiceStatus>(&status, "invoice", "status")?
                    != InvoiceStatus::Draft
                {
                    return Err(CommerceError::ValidationError(format!(
                        "Can only delete draft invoices. Invoice {} has status {}",
                        id_str, status
                    )));
                }
            }
        }

        // Delete invoice items first
        let sql = format!("DELETE FROM invoice_items WHERE invoice_id IN ({})", placeholders);
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete invoices
        let sql = format!("DELETE FROM invoices WHERE id IN ({})", placeholders);
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<InvoiceId>) -> Result<Vec<Invoice>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM invoices WHERE id IN ({})", placeholders);

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let invoices = stmt
            .query_map(params_refs.as_slice(), Self::row_to_invoice)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for each invoice
        let mut result = vec![];
        for mut invoice in invoices {
            invoice.items = Self::get_invoice_items_with_conn(&conn, invoice.id)?;
            result.push(invoice);
        }

        Ok(result)
    }
}
