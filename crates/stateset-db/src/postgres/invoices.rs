//! PostgreSQL implementation of invoice repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{postgres::PgPool, FromRow, Row};
use stateset_core::{
    CommerceError, CreateInvoice, CreateInvoiceItem, Invoice, InvoiceFilter, InvoiceItem,
    InvoiceRepository, InvoiceStatus, InvoiceType, RecordInvoicePayment, Result, UpdateInvoice,
    generate_invoice_number,
};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct InvoiceRow {
    id: Uuid,
    invoice_number: String,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    status: String,
    invoice_type: String,
    invoice_date: DateTime<Utc>,
    due_date: DateTime<Utc>,
    payment_terms: Option<String>,
    currency: String,
    billing_name: Option<String>,
    billing_email: Option<String>,
    billing_address: Option<String>,
    billing_city: Option<String>,
    billing_state: Option<String>,
    billing_postal_code: Option<String>,
    billing_country: Option<String>,
    subtotal: Decimal,
    discount_amount: Decimal,
    discount_percent: Option<Decimal>,
    tax_amount: Decimal,
    tax_rate: Option<Decimal>,
    shipping_amount: Decimal,
    total: Decimal,
    amount_paid: Decimal,
    balance_due: Decimal,
    po_number: Option<String>,
    notes: Option<String>,
    terms: Option<String>,
    footer: Option<String>,
    sent_at: Option<DateTime<Utc>>,
    viewed_at: Option<DateTime<Utc>>,
    paid_at: Option<DateTime<Utc>>,
    voided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl InvoiceRow {
    fn into_invoice(self, items: Vec<InvoiceItem>) -> Invoice {
        Invoice {
            id: self.id,
            invoice_number: self.invoice_number,
            customer_id: self.customer_id,
            order_id: self.order_id,
            status: self.status.parse().unwrap_or_default(),
            invoice_type: self.invoice_type.parse().unwrap_or_default(),
            invoice_date: self.invoice_date,
            due_date: self.due_date,
            payment_terms: self.payment_terms,
            currency: self.currency,
            billing_name: self.billing_name,
            billing_email: self.billing_email,
            billing_address: self.billing_address,
            billing_city: self.billing_city,
            billing_state: self.billing_state,
            billing_postal_code: self.billing_postal_code,
            billing_country: self.billing_country,
            subtotal: self.subtotal,
            discount_amount: self.discount_amount,
            discount_percent: self.discount_percent,
            tax_amount: self.tax_amount,
            tax_rate: self.tax_rate,
            shipping_amount: self.shipping_amount,
            total: self.total,
            amount_paid: self.amount_paid,
            balance_due: self.balance_due,
            po_number: self.po_number,
            notes: self.notes,
            terms: self.terms,
            footer: self.footer,
            sent_at: self.sent_at,
            viewed_at: self.viewed_at,
            paid_at: self.paid_at,
            voided_at: self.voided_at,
            items,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct InvoiceItemRow {
    id: Uuid,
    invoice_id: Uuid,
    order_item_id: Option<Uuid>,
    product_id: Option<Uuid>,
    sku: Option<String>,
    description: String,
    quantity: Decimal,
    unit_of_measure: Option<String>,
    unit_price: Decimal,
    discount_amount: Decimal,
    tax_amount: Decimal,
    line_total: Decimal,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<InvoiceItemRow> for InvoiceItem {
    fn from(row: InvoiceItemRow) -> Self {
        InvoiceItem {
            id: row.id,
            invoice_id: row.invoice_id,
            order_item_id: row.order_item_id,
            product_id: row.product_id,
            sku: row.sku,
            description: row.description,
            quantity: row.quantity,
            unit_of_measure: row.unit_of_measure,
            unit_price: row.unit_price,
            discount_amount: row.discount_amount,
            tax_amount: row.tax_amount,
            line_total: row.line_total,
            sort_order: row.sort_order,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// PostgreSQL invoice repository
pub struct PgInvoiceRepository {
    pool: PgPool,
}

impl PgInvoiceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn get_invoice_items_async(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        let rows: Vec<InvoiceItemRow> = sqlx::query_as(
            "SELECT * FROM invoice_items WHERE invoice_id = $1 ORDER BY sort_order",
        )
        .bind(invoice_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn get_invoice_with_items(&self, id: Uuid) -> Result<Option<Invoice>> {
        let row: Option<InvoiceRow> = sqlx::query_as("SELECT * FROM invoices WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(invoice_row) => {
                let items = self.get_invoice_items_async(id).await?;
                Ok(Some(invoice_row.into_invoice(items)))
            }
            None => Ok(None),
        }
    }

    async fn recalculate_async(&self, id: Uuid) -> Result<()> {
        // Calculate subtotal from items
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(line_total), 0) FROM invoice_items WHERE invoice_id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Get current discount, tax, shipping, and amount_paid
        let row: (Decimal, Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT discount_amount, tax_amount, shipping_amount, amount_paid FROM invoices WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let (discount_amount, tax_amount, shipping_amount, amount_paid) = row;
        let total = subtotal - discount_amount + tax_amount + shipping_amount;
        let balance_due = total - amount_paid;

        sqlx::query(
            "UPDATE invoices SET subtotal = $1, total = $2, balance_due = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(subtotal)
        .bind(total)
        .bind(balance_due)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    // Async implementations
    pub async fn create_async(&self, input: CreateInvoice) -> Result<Invoice> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let invoice_number = generate_invoice_number();
        let invoice_date = input.invoice_date.unwrap_or(now);
        let due_date = input.due_date.unwrap_or_else(|| {
            invoice_date + chrono::Duration::days(input.days_until_due.unwrap_or(30) as i64)
        });

        // Start transaction
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            r#"INSERT INTO invoices (
                id, invoice_number, customer_id, order_id, status, invoice_type,
                invoice_date, due_date, payment_terms, currency, billing_name, billing_email,
                billing_address, billing_city, billing_state, billing_postal_code, billing_country,
                subtotal, discount_amount, discount_percent, tax_amount, tax_rate, shipping_amount,
                total, amount_paid, balance_due, po_number, notes, terms, footer, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32
            )"#,
        )
        .bind(id)
        .bind(&invoice_number)
        .bind(input.customer_id)
        .bind(input.order_id)
        .bind(InvoiceStatus::Draft.to_string())
        .bind(input.invoice_type.unwrap_or_default().to_string())
        .bind(invoice_date)
        .bind(due_date)
        .bind(&input.payment_terms)
        .bind(input.currency.unwrap_or_else(|| "USD".to_string()))
        .bind(&input.billing_name)
        .bind(&input.billing_email)
        .bind(&input.billing_address)
        .bind(&input.billing_city)
        .bind(&input.billing_state)
        .bind(&input.billing_postal_code)
        .bind(&input.billing_country)
        .bind(Decimal::ZERO)
        .bind(input.discount_amount.unwrap_or_default())
        .bind(input.discount_percent)
        .bind(input.tax_amount.unwrap_or_default())
        .bind(input.tax_rate)
        .bind(input.shipping_amount.unwrap_or_default())
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(&input.po_number)
        .bind(&input.notes)
        .bind(&input.terms)
        .bind(&input.footer)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        // Add items
        for (i, item) in input.items.into_iter().enumerate() {
            let item_id = Uuid::new_v4();
            let sort_order = item.sort_order.unwrap_or(i as i32);
            let line_total = item.quantity * item.unit_price
                - item.discount_amount.unwrap_or_default()
                + item.tax_amount.unwrap_or_default();

            sqlx::query(
                r#"INSERT INTO invoice_items (
                    id, invoice_id, order_item_id, product_id, sku, description,
                    quantity, unit_of_measure, unit_price, discount_amount, tax_amount, line_total,
                    sort_order, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"#,
            )
            .bind(item_id)
            .bind(id)
            .bind(item.order_item_id)
            .bind(item.product_id)
            .bind(&item.sku)
            .bind(&item.description)
            .bind(item.quantity)
            .bind(&item.unit_of_measure)
            .bind(item.unit_price)
            .bind(item.discount_amount.unwrap_or_default())
            .bind(item.tax_amount.unwrap_or_default())
            .bind(line_total)
            .bind(sort_order)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }

        // Recalculate totals within transaction
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(line_total), 0) FROM invoice_items WHERE invoice_id = $1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let discount_amount = input.discount_amount.unwrap_or_default();
        let tax_amount = input.tax_amount.unwrap_or_default();
        let shipping_amount = input.shipping_amount.unwrap_or_default();
        let total = subtotal - discount_amount + tax_amount + shipping_amount;
        let balance_due = total;

        sqlx::query(
            "UPDATE invoices SET subtotal = $1, total = $2, balance_due = $3 WHERE id = $4",
        )
        .bind(subtotal)
        .bind(total)
        .bind(balance_due)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<Invoice>> {
        self.get_invoice_with_items(id).await
    }

    pub async fn get_by_number_async(&self, invoice_number: &str) -> Result<Option<Invoice>> {
        let row: Option<InvoiceRow> =
            sqlx::query_as("SELECT * FROM invoices WHERE invoice_number = $1")
                .bind(invoice_number)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        match row {
            Some(invoice_row) => {
                let items = self.get_invoice_items_async(invoice_row.id).await?;
                Ok(Some(invoice_row.into_invoice(items)))
            }
            None => Ok(None),
        }
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice> {
        let invoice = self
            .get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            r#"UPDATE invoices SET
                due_date = $1, payment_terms = $2, billing_name = $3, billing_email = $4,
                billing_address = $5, billing_city = $6, billing_state = $7, billing_postal_code = $8,
                billing_country = $9, discount_amount = $10, discount_percent = $11, tax_amount = $12,
                tax_rate = $13, shipping_amount = $14, po_number = $15, notes = $16, terms = $17,
                footer = $18, updated_at = $19
            WHERE id = $20"#,
        )
        .bind(input.due_date.unwrap_or(invoice.due_date))
        .bind(input.payment_terms.or(invoice.payment_terms))
        .bind(input.billing_name.or(invoice.billing_name))
        .bind(input.billing_email.or(invoice.billing_email))
        .bind(input.billing_address.or(invoice.billing_address))
        .bind(input.billing_city.or(invoice.billing_city))
        .bind(input.billing_state.or(invoice.billing_state))
        .bind(input.billing_postal_code.or(invoice.billing_postal_code))
        .bind(input.billing_country.or(invoice.billing_country))
        .bind(input.discount_amount.unwrap_or(invoice.discount_amount))
        .bind(input.discount_percent.or(invoice.discount_percent))
        .bind(input.tax_amount.unwrap_or(invoice.tax_amount))
        .bind(input.tax_rate.or(invoice.tax_rate))
        .bind(input.shipping_amount.unwrap_or(invoice.shipping_amount))
        .bind(input.po_number.or(invoice.po_number))
        .bind(input.notes.or(invoice.notes))
        .bind(input.terms.or(invoice.terms))
        .bind(input.footer.or(invoice.footer))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_async(id).await?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn list_async(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        let mut sql = "SELECT * FROM invoices WHERE 1=1".to_string();
        let mut param_count = 0;

        if filter.customer_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND customer_id = ${}", param_count));
        }
        if filter.order_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND order_id = ${}", param_count));
        }
        if filter.status.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND status = ${}", param_count));
        }
        if filter.invoice_type.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND invoice_type = ${}", param_count));
        }
        if filter.overdue_only.unwrap_or(false) {
            sql.push_str(" AND due_date < NOW() AND status NOT IN ('paid', 'voided')");
        }
        if filter.from_date.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND invoice_date >= ${}", param_count));
        }
        if filter.to_date.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND invoice_date <= ${}", param_count));
        }
        if filter.due_from.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND due_date >= ${}", param_count));
        }
        if filter.due_to.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND due_date <= ${}", param_count));
        }
        if filter.min_total.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND total >= ${}", param_count));
        }
        if filter.max_total.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND total <= ${}", param_count));
        }
        if filter.min_balance.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND balance_due >= ${}", param_count));
        }
        if filter.invoice_number.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND invoice_number ILIKE ${}", param_count));
        }

        sql.push_str(" ORDER BY invoice_date DESC");

        if let Some(limit) = filter.limit {
            param_count += 1;
            sql.push_str(&format!(" LIMIT ${}", param_count));
        }
        if let Some(offset) = filter.offset {
            param_count += 1;
            sql.push_str(&format!(" OFFSET ${}", param_count));
        }

        let mut query = sqlx::query_as::<_, InvoiceRow>(&sql);

        if let Some(customer_id) = filter.customer_id {
            query = query.bind(customer_id);
        }
        if let Some(order_id) = filter.order_id {
            query = query.bind(order_id);
        }
        if let Some(status) = filter.status {
            query = query.bind(status.to_string());
        }
        if let Some(invoice_type) = filter.invoice_type {
            query = query.bind(invoice_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            query = query.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            query = query.bind(to_date);
        }
        if let Some(due_from) = filter.due_from {
            query = query.bind(due_from);
        }
        if let Some(due_to) = filter.due_to {
            query = query.bind(due_to);
        }
        if let Some(min_total) = filter.min_total {
            query = query.bind(min_total);
        }
        if let Some(max_total) = filter.max_total {
            query = query.bind(max_total);
        }
        if let Some(min_balance) = filter.min_balance {
            query = query.bind(min_balance);
        }
        if let Some(invoice_number) = filter.invoice_number {
            query = query.bind(format!("%{}%", invoice_number));
        }
        if let Some(limit) = filter.limit {
            query = query.bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows: Vec<InvoiceRow> = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut invoices = Vec::new();
        for row in rows {
            let items = self.get_invoice_items_async(row.id).await?;
            invoices.push(row.into_invoice(items));
        }

        Ok(invoices)
    }

    pub async fn for_customer_async(&self, customer_id: Uuid) -> Result<Vec<Invoice>> {
        self.list_async(InvoiceFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .await
    }

    pub async fn for_order_async(&self, order_id: Uuid) -> Result<Vec<Invoice>> {
        self.list_async(InvoiceFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
        .await
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        // Check status first
        let status: String =
            sqlx::query_scalar("SELECT status FROM invoices WHERE id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        if status.parse::<InvoiceStatus>().unwrap_or_default() != InvoiceStatus::Draft {
            return Err(CommerceError::ValidationError(
                "Can only delete draft invoices".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query("DELETE FROM invoice_items WHERE invoice_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM invoices WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn send_async(&self, id: Uuid) -> Result<Invoice> {
        let now = Utc::now();

        sqlx::query("UPDATE invoices SET status = $1, sent_at = $2, updated_at = $3 WHERE id = $4")
            .bind(InvoiceStatus::Sent.to_string())
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn mark_viewed_async(&self, id: Uuid) -> Result<Invoice> {
        let now = Utc::now();

        sqlx::query(
            r#"UPDATE invoices SET
                status = CASE WHEN status = 'sent' THEN 'viewed' ELSE status END,
                viewed_at = COALESCE(viewed_at, $1),
                updated_at = $2
            WHERE id = $3"#,
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn record_payment_async(
        &self,
        id: Uuid,
        payment: RecordInvoicePayment,
    ) -> Result<Invoice> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        let (total, amount_paid): (Decimal, Decimal) =
            sqlx::query_as("SELECT total, amount_paid FROM invoices WHERE id = $1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_db_error)?;

        let new_amount_paid = amount_paid + payment.amount;
        let new_balance = total - new_amount_paid;

        let new_status = if new_balance <= Decimal::ZERO {
            InvoiceStatus::Paid
        } else {
            InvoiceStatus::PartiallyPaid
        };

        let paid_at: Option<DateTime<Utc>> = if new_status == InvoiceStatus::Paid {
            Some(now)
        } else {
            None
        };

        sqlx::query(
            r#"UPDATE invoices SET
                amount_paid = $1, balance_due = $2, status = $3,
                paid_at = COALESCE($4, paid_at), updated_at = $5
            WHERE id = $6"#,
        )
        .bind(new_amount_paid)
        .bind(new_balance)
        .bind(new_status.to_string())
        .bind(paid_at)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn void_async(&self, id: Uuid) -> Result<Invoice> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE invoices SET status = $1, voided_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(InvoiceStatus::Voided.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn write_off_async(&self, id: Uuid) -> Result<Invoice> {
        let now = Utc::now();

        sqlx::query("UPDATE invoices SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(InvoiceStatus::WrittenOff.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn dispute_async(&self, id: Uuid) -> Result<Invoice> {
        let now = Utc::now();

        sqlx::query("UPDATE invoices SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(InvoiceStatus::Disputed.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn add_item_async(
        &self,
        invoice_id: Uuid,
        item: CreateInvoiceItem,
    ) -> Result<InvoiceItem> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let line_total = item.quantity * item.unit_price
            - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

        sqlx::query(
            r#"INSERT INTO invoice_items (
                id, invoice_id, order_item_id, product_id, sku, description,
                quantity, unit_of_measure, unit_price, discount_amount, tax_amount, line_total,
                sort_order, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"#,
        )
        .bind(id)
        .bind(invoice_id)
        .bind(item.order_item_id)
        .bind(item.product_id)
        .bind(&item.sku)
        .bind(&item.description)
        .bind(item.quantity)
        .bind(&item.unit_of_measure)
        .bind(item.unit_price)
        .bind(item.discount_amount.unwrap_or_default())
        .bind(item.tax_amount.unwrap_or_default())
        .bind(line_total)
        .bind(item.sort_order.unwrap_or(0))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row: InvoiceItemRow = sqlx::query_as("SELECT * FROM invoice_items WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.into())
    }

    pub async fn update_item_async(
        &self,
        item_id: Uuid,
        item: CreateInvoiceItem,
    ) -> Result<InvoiceItem> {
        let now = Utc::now();
        let line_total = item.quantity * item.unit_price
            - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

        sqlx::query(
            r#"UPDATE invoice_items SET
                sku = $1, description = $2, quantity = $3, unit_of_measure = $4,
                unit_price = $5, discount_amount = $6, tax_amount = $7, line_total = $8,
                sort_order = $9, updated_at = $10
            WHERE id = $11"#,
        )
        .bind(&item.sku)
        .bind(&item.description)
        .bind(item.quantity)
        .bind(&item.unit_of_measure)
        .bind(item.unit_price)
        .bind(item.discount_amount.unwrap_or_default())
        .bind(item.tax_amount.unwrap_or_default())
        .bind(line_total)
        .bind(item.sort_order.unwrap_or(0))
        .bind(now)
        .bind(item_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row: InvoiceItemRow = sqlx::query_as("SELECT * FROM invoice_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.into())
    }

    pub async fn remove_item_async(&self, item_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM invoice_items WHERE id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_items_async(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        self.get_invoice_items_async(invoice_id).await
    }

    pub async fn recalculate_invoice_async(&self, id: Uuid) -> Result<Invoice> {
        self.recalculate_async(id).await?;
        self.get_invoice_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn get_overdue_async(&self) -> Result<Vec<Invoice>> {
        self.list_async(InvoiceFilter {
            overdue_only: Some(true),
            ..Default::default()
        })
        .await
    }

    pub async fn count_async(&self, filter: InvoiceFilter) -> Result<u64> {
        let mut sql = "SELECT COUNT(*) FROM invoices WHERE 1=1".to_string();
        let mut param_count = 0;

        if filter.customer_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND customer_id = ${}", param_count));
        }
        if filter.status.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND status = ${}", param_count));
        }
        if filter.overdue_only.unwrap_or(false) {
            sql.push_str(" AND due_date < NOW() AND status NOT IN ('paid', 'voided')");
        }

        let mut query = sqlx::query_scalar::<_, i64>(&sql);

        if let Some(customer_id) = filter.customer_id {
            query = query.bind(customer_id);
        }
        if let Some(status) = filter.status {
            query = query.bind(status.to_string());
        }

        let count = query.fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count as u64)
    }
}

impl InvoiceRepository for PgInvoiceRepository {
    fn create(&self, input: CreateInvoice) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Invoice>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(invoice_number))
    }

    fn update(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Invoice>> {
        tokio::runtime::Handle::current().block_on(self.for_customer_async(customer_id))
    }

    fn for_order(&self, order_id: Uuid) -> Result<Vec<Invoice>> {
        tokio::runtime::Handle::current().block_on(self.for_order_async(order_id))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_async(id))
    }

    fn send(&self, id: Uuid) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.send_async(id))
    }

    fn mark_viewed(&self, id: Uuid) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.mark_viewed_async(id))
    }

    fn record_payment(&self, id: Uuid, payment: RecordInvoicePayment) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.record_payment_async(id, payment))
    }

    fn void(&self, id: Uuid) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.void_async(id))
    }

    fn write_off(&self, id: Uuid) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.write_off_async(id))
    }

    fn dispute(&self, id: Uuid) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.dispute_async(id))
    }

    fn add_item(&self, invoice_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        tokio::runtime::Handle::current().block_on(self.add_item_async(invoice_id, item))
    }

    fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        tokio::runtime::Handle::current().block_on(self.update_item_async(item_id, item))
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.remove_item_async(item_id))
    }

    fn get_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        tokio::runtime::Handle::current().block_on(self.get_items_async(invoice_id))
    }

    fn recalculate(&self, id: Uuid) -> Result<Invoice> {
        tokio::runtime::Handle::current().block_on(self.recalculate_invoice_async(id))
    }

    fn get_overdue(&self) -> Result<Vec<Invoice>> {
        tokio::runtime::Handle::current().block_on(self.get_overdue_async())
    }

    fn count(&self, filter: InvoiceFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}
