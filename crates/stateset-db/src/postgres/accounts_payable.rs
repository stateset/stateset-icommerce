//! PostgreSQL implementation for Accounts Payable

use super::{block_on, map_db_error};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AccountsPayableRepository, ApAgingSummary, BatchResult, Bill, BillFilter, BillItem,
    BillPayment, BillPaymentFilter, BillStatus, CommerceError, CreateBill, CreateBillItem,
    CreateBillPayment, CreatePaymentRun, PaymentAllocation, PaymentMethodAP, PaymentRun,
    PaymentRunFilter, PaymentRunStatus, PaymentStatusAP, Result, SupplierApSummary, UpdateBill,
    generate_ap_payment_number, generate_bill_number, generate_payment_run_number,
    validate_batch_size,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgAccountsPayableRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct BillRow {
    id: Uuid,
    bill_number: String,
    supplier_id: Uuid,
    supplier_name: Option<String>,
    purchase_order_id: Option<Uuid>,
    status: String,
    bill_date: NaiveDate,
    due_date: NaiveDate,
    payment_terms: Option<String>,
    subtotal: Decimal,
    tax_amount: Decimal,
    shipping_amount: Decimal,
    discount_amount: Decimal,
    total_amount: Decimal,
    amount_paid: Decimal,
    amount_due: Decimal,
    currency: String,
    reference_number: Option<String>,
    memo: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct BillItemRow {
    id: Uuid,
    bill_id: Uuid,
    line_number: i32,
    description: String,
    account_code: Option<String>,
    quantity: Decimal,
    unit_price: Decimal,
    amount: Decimal,
    tax_rate: Option<Decimal>,
    tax_amount: Decimal,
    po_line_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PaymentRow {
    id: Uuid,
    payment_number: String,
    supplier_id: Uuid,
    payment_date: NaiveDate,
    payment_method: String,
    amount: Decimal,
    currency: String,
    reference_number: Option<String>,
    bank_account: Option<String>,
    check_number: Option<String>,
    memo: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PaymentAllocationRow {
    id: Uuid,
    payment_id: Uuid,
    bill_id: Uuid,
    amount: Decimal,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PaymentRunRow {
    id: Uuid,
    run_number: String,
    status: String,
    payment_date: NaiveDate,
    payment_method: String,
    total_amount: Decimal,
    payment_count: i32,
    notes: Option<String>,
    created_by: Option<String>,
    approved_by: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    processed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgAccountsPayableRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_bill(row: BillRow) -> Result<Bill> {
        let BillRow {
            id,
            bill_number,
            supplier_id,
            supplier_name,
            purchase_order_id,
            status,
            bill_date,
            due_date,
            payment_terms,
            subtotal,
            tax_amount,
            shipping_amount,
            discount_amount,
            total_amount,
            amount_paid,
            amount_due,
            currency,
            reference_number,
            memo,
            created_at,
            updated_at,
        } = row;

        let status: BillStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid bill.status '{}': {}", status, e))
        })?;

        Ok(Bill {
            id,
            bill_number,
            supplier_id,
            supplier_name,
            purchase_order_id,
            status,
            bill_date: from_date(bill_date),
            due_date: from_date(due_date),
            payment_terms,
            subtotal,
            tax_amount,
            shipping_amount,
            discount_amount,
            total_amount,
            amount_paid,
            amount_due,
            currency,
            reference_number,
            memo,
            created_at,
            updated_at,
        })
    }

    fn row_to_bill_item(row: BillItemRow) -> BillItem {
        BillItem {
            id: row.id,
            bill_id: row.bill_id,
            line_number: row.line_number,
            description: row.description,
            account_code: row.account_code,
            quantity: row.quantity,
            unit_price: row.unit_price,
            amount: row.amount,
            tax_rate: row.tax_rate,
            tax_amount: row.tax_amount,
            po_line_id: row.po_line_id,
            created_at: row.created_at,
        }
    }

    fn row_to_payment(row: PaymentRow) -> Result<BillPayment> {
        let PaymentRow {
            id,
            payment_number,
            supplier_id,
            payment_date,
            payment_method,
            amount,
            currency,
            reference_number,
            bank_account,
            check_number,
            memo,
            status,
            created_at,
            updated_at,
        } = row;

        let payment_method: PaymentMethodAP = payment_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid bill_payment.payment_method '{}': {}",
                payment_method, e
            ))
        })?;
        let status: PaymentStatusAP = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid bill_payment.status '{}': {}", status, e))
        })?;

        Ok(BillPayment {
            id,
            payment_number,
            supplier_id,
            payment_date: from_date(payment_date),
            payment_method,
            amount,
            currency,
            reference_number,
            bank_account,
            check_number,
            memo,
            status,
            created_at,
            updated_at,
        })
    }

    fn row_to_payment_allocation(row: PaymentAllocationRow) -> PaymentAllocation {
        PaymentAllocation {
            id: row.id,
            payment_id: row.payment_id,
            bill_id: row.bill_id,
            amount: row.amount,
            created_at: row.created_at,
        }
    }

    fn row_to_payment_run(row: PaymentRunRow) -> Result<PaymentRun> {
        let PaymentRunRow {
            id,
            run_number,
            status,
            payment_date,
            payment_method,
            total_amount,
            payment_count,
            notes,
            created_by,
            approved_by,
            approved_at,
            processed_at,
            created_at,
            updated_at,
        } = row;

        let status: PaymentRunStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid payment_run.status '{}': {}", status, e))
        })?;
        let payment_method: PaymentMethodAP = payment_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment_run.payment_method '{}': {}",
                payment_method, e
            ))
        })?;

        Ok(PaymentRun {
            id,
            run_number,
            status,
            payment_date: from_date(payment_date),
            payment_method,
            total_amount,
            payment_count,
            notes,
            created_by,
            approved_by,
            approved_at,
            processed_at,
            created_at,
            updated_at,
        })
    }

    async fn recalculate_bill(&self, bill_id: Uuid) -> Result<()> {
        let (subtotal, tax_amount): (Decimal, Decimal) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount), 0), COALESCE(SUM(tax_amount), 0) FROM ap_bill_items WHERE bill_id = $1",
        )
        .bind(bill_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let total = subtotal + tax_amount;

        let (amount_paid,): (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount), 0) FROM ap_payment_allocations WHERE bill_id = $1",
        )
        .bind(bill_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let amount_due = total - amount_paid;

        sqlx::query(
            "UPDATE ap_bills SET subtotal = $1, tax_amount = $2, total_amount = $3, amount_paid = $4, amount_due = $5 WHERE id = $6",
        )
        .bind(subtotal)
        .bind(tax_amount)
        .bind(total)
        .bind(amount_paid)
        .bind(amount_due)
        .bind(bill_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn create_bill_async(&self, input: CreateBill) -> Result<Bill> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let bill_number = input.bill_number.unwrap_or_else(generate_bill_number);
        let bill_date = to_date(input.bill_date.unwrap_or(now));
        let due_date = to_date(input.due_date);

        sqlx::query(
            r#"
            INSERT INTO ap_bills (id, bill_number, supplier_id, purchase_order_id, status, bill_date, due_date,
             payment_terms, currency, reference_number, memo, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$12)
            "#,
        )
        .bind(id)
        .bind(&bill_number)
        .bind(input.supplier_id)
        .bind(input.purchase_order_id)
        .bind(BillStatus::Draft.to_string())
        .bind(bill_date)
        .bind(due_date)
        .bind(&input.payment_terms)
        .bind(input.currency.clone().unwrap_or_else(|| "USD".to_string()))
        .bind(&input.reference_number)
        .bind(&input.memo)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        for item in input.items.iter() {
            self.add_bill_item_async(
                id,
                CreateBillItem {
                    description: item.description.clone(),
                    account_code: item.account_code.clone(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    tax_rate: item.tax_rate,
                    po_line_id: item.po_line_id,
                },
            )
            .await?;
        }

        self.recalculate_bill(id).await?;

        self.get_bill_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create bill".into()))
    }

    pub async fn get_bill_async(&self, id: Uuid) -> Result<Option<Bill>> {
        let row = sqlx::query_as::<_, BillRow>("SELECT * FROM ap_bills WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_bill).transpose()
    }

    pub async fn get_bill_by_number_async(&self, number: &str) -> Result<Option<Bill>> {
        let row = sqlx::query_as::<_, BillRow>("SELECT * FROM ap_bills WHERE bill_number = $1")
            .bind(number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_bill).transpose()
    }

    pub async fn update_bill_async(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        let due_date = input.due_date.map(to_date);

        sqlx::query(
            r#"
            UPDATE ap_bills SET
                due_date = COALESCE($1, due_date),
                payment_terms = COALESCE($2, payment_terms),
                reference_number = COALESCE($3, reference_number),
                memo = COALESCE($4, memo)
            WHERE id = $5
            "#,
        )
        .bind(due_date)
        .bind(&input.payment_terms)
        .bind(&input.reference_number)
        .bind(&input.memo)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_bill_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update bill".into()))
    }

    pub async fn list_bills_async(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM ap_bills WHERE 1=1");

        if let Some(supplier_id) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(purchase_order_id) = filter.purchase_order_id {
            builder.push(" AND purchase_order_id = ").push_bind(purchase_order_id);
        }
        if filter.overdue_only == Some(true) {
            builder.push(" AND due_date < CURRENT_DATE AND status NOT IN ('paid', 'cancelled')");
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND bill_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_value) = filter.to_date {
            builder.push(" AND bill_date <= ").push_bind(to_date(to_date_value));
        }
        if let Some(min_amount) = filter.min_amount {
            builder.push(" AND total_amount >= ").push_bind(min_amount);
        }
        if let Some(max_amount) = filter.max_amount {
            builder.push(" AND total_amount <= ").push_bind(max_amount);
        }

        builder.push(" ORDER BY due_date");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<BillRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_bill).collect::<Result<Vec<_>>>()
    }

    pub async fn delete_bill_async(&self, id: Uuid) -> Result<()> {
        let bill = self.get_bill_async(id).await?.ok_or(CommerceError::NotFound)?;

        if bill.status != BillStatus::Draft {
            return Err(CommerceError::ValidationError("Can only delete draft bills".into()));
        }

        sqlx::query("DELETE FROM ap_bills WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn approve_bill_async(&self, id: Uuid) -> Result<Bill> {
        sqlx::query(
            "UPDATE ap_bills SET status = $1 WHERE id = $2 AND status IN ('draft','pending')",
        )
        .bind(BillStatus::Approved.to_string())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_bill_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to approve bill".into()))
    }

    pub async fn cancel_bill_async(&self, id: Uuid) -> Result<Bill> {
        sqlx::query("UPDATE ap_bills SET status = $1 WHERE id = $2")
            .bind(BillStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_bill_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel bill".into()))
    }

    pub async fn dispute_bill_async(&self, id: Uuid) -> Result<Bill> {
        sqlx::query("UPDATE ap_bills SET status = $1 WHERE id = $2")
            .bind(BillStatus::Disputed.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_bill_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to dispute bill".into()))
    }

    pub async fn get_bill_items_async(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        let rows = sqlx::query_as::<_, BillItemRow>(
            "SELECT * FROM ap_bill_items WHERE bill_id = $1 ORDER BY line_number",
        )
        .bind(bill_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_bill_item).collect())
    }

    pub async fn add_bill_item_async(
        &self,
        bill_id: Uuid,
        item: CreateBillItem,
    ) -> Result<BillItem> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        let line_number: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(line_number), 0) + 1 FROM ap_bill_items WHERE bill_id = $1",
        )
        .bind(bill_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let amount = item.quantity * item.unit_price;
        let tax_amount =
            item.tax_rate.map(|r| amount * r / Decimal::from(100)).unwrap_or(Decimal::ZERO);

        sqlx::query(
            r#"
            INSERT INTO ap_bill_items (id, bill_id, line_number, description, account_code, quantity, unit_price, amount, tax_rate, tax_amount, po_line_id, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            "#,
        )
        .bind(id)
        .bind(bill_id)
        .bind(line_number.0 as i32)
        .bind(&item.description)
        .bind(&item.account_code)
        .bind(item.quantity)
        .bind(item.unit_price)
        .bind(amount)
        .bind(item.tax_rate)
        .bind(tax_amount)
        .bind(item.po_line_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_bill(bill_id).await?;

        self.get_bill_item_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create bill item".into()))
    }

    async fn get_bill_item_async(&self, id: Uuid) -> Result<Option<BillItem>> {
        let row = sqlx::query_as::<_, BillItemRow>("SELECT * FROM ap_bill_items WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_bill_item))
    }

    pub async fn remove_bill_item_async(&self, item_id: Uuid) -> Result<()> {
        let row = sqlx::query_as::<_, (Uuid,)>("SELECT bill_id FROM ap_bill_items WHERE id = $1")
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        sqlx::query("DELETE FROM ap_bill_items WHERE id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.recalculate_bill(row.0).await?;

        Ok(())
    }

    pub async fn count_bills_async(&self, filter: BillFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ap_bills WHERE 1=1");

        if let Some(supplier_id) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if filter.overdue_only == Some(true) {
            builder.push(" AND due_date < CURRENT_DATE AND status NOT IN ('paid', 'cancelled')");
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn get_overdue_bills_async(&self) -> Result<Vec<Bill>> {
        self.list_bills_async(BillFilter { overdue_only: Some(true), ..Default::default() }).await
    }

    pub async fn get_bills_due_soon_async(&self, days: i32) -> Result<Vec<Bill>> {
        let rows = sqlx::query_as::<_, BillRow>(
            "SELECT * FROM ap_bills WHERE due_date <= CURRENT_DATE + $1 AND status NOT IN ('paid', 'cancelled') ORDER BY due_date",
        )
        .bind(days)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_bill).collect::<Result<Vec<_>>>()
    }

    pub async fn create_payment_async(&self, input: CreateBillPayment) -> Result<BillPayment> {
        let CreateBillPayment {
            supplier_id,
            payment_date,
            payment_method,
            amount,
            currency,
            reference_number,
            bank_account,
            check_number,
            memo,
            allocations,
        } = input;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let payment_number = generate_ap_payment_number();
        let payment_date = to_date(payment_date.unwrap_or(now));

        sqlx::query(
            r#"
            INSERT INTO ap_payments (
                id, payment_number, supplier_id, payment_date, payment_method, amount, currency,
                reference_number, bank_account, check_number, memo, status, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$13)
            "#,
        )
        .bind(id)
        .bind(&payment_number)
        .bind(supplier_id)
        .bind(payment_date)
        .bind(payment_method.to_string())
        .bind(amount)
        .bind(currency.unwrap_or_else(|| "USD".to_string()))
        .bind(&reference_number)
        .bind(&bank_account)
        .bind(&check_number)
        .bind(&memo)
        .bind(PaymentStatusAP::Pending.to_string())
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for alloc in &allocations {
            let alloc_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO ap_payment_allocations (id, payment_id, bill_id, amount, created_at) VALUES ($1,$2,$3,$4,$5)",
            )
            .bind(alloc_id)
            .bind(id)
            .bind(alloc.bill_id)
            .bind(alloc.amount)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        for alloc in allocations {
            self.recalculate_bill(alloc.bill_id).await?;

            let bill = self.get_bill_async(alloc.bill_id).await?.ok_or(CommerceError::NotFound)?;
            let new_status = if bill.amount_due <= Decimal::ZERO {
                BillStatus::Paid
            } else if bill.amount_paid > Decimal::ZERO {
                BillStatus::PartiallyPaid
            } else {
                bill.status
            };

            sqlx::query("UPDATE ap_bills SET status = $1 WHERE id = $2")
                .bind(new_status.to_string())
                .bind(alloc.bill_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        self.get_payment_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create payment".into()))
    }

    pub async fn get_payment_async(&self, id: Uuid) -> Result<Option<BillPayment>> {
        let row = sqlx::query_as::<_, PaymentRow>("SELECT * FROM ap_payments WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    pub async fn get_payment_by_number_async(&self, number: &str) -> Result<Option<BillPayment>> {
        let row =
            sqlx::query_as::<_, PaymentRow>("SELECT * FROM ap_payments WHERE payment_number = $1")
                .bind(number)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        row.map(Self::row_to_payment).transpose()
    }

    pub async fn list_payments_async(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM ap_payments WHERE 1=1");

        if let Some(supplier_id) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(method) = filter.payment_method {
            builder.push(" AND payment_method = ").push_bind(method.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND payment_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_value) = filter.to_date {
            builder.push(" AND payment_date <= ").push_bind(to_date(to_date_value));
        }

        builder.push(" ORDER BY payment_date DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PaymentRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_payment).collect::<Result<Vec<_>>>()
    }

    pub async fn void_payment_async(&self, id: Uuid) -> Result<BillPayment> {
        sqlx::query("UPDATE ap_payments SET status = $1 WHERE id = $2")
            .bind(PaymentStatusAP::Voided.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_payment_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to void payment".into()))
    }

    pub async fn clear_payment_async(&self, id: Uuid) -> Result<BillPayment> {
        sqlx::query("UPDATE ap_payments SET status = $1 WHERE id = $2")
            .bind(PaymentStatusAP::Cleared.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_payment_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to clear payment".into()))
    }

    pub async fn get_payment_allocations_async(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<PaymentAllocation>> {
        let rows = sqlx::query_as::<_, PaymentAllocationRow>(
            "SELECT * FROM ap_payment_allocations WHERE payment_id = $1",
        )
        .bind(payment_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_payment_allocation).collect())
    }

    pub async fn get_payments_for_bill_async(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        let rows = sqlx::query_as::<_, PaymentRow>(
            "SELECT p.* FROM ap_payments p JOIN ap_payment_allocations a ON p.id = a.payment_id WHERE a.bill_id = $1",
        )
        .bind(bill_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_payment).collect::<Result<Vec<_>>>()
    }

    pub async fn count_payments_async(&self, filter: BillPaymentFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ap_payments WHERE 1=1");

        if let Some(supplier_id) = filter.supplier_id {
            builder.push(" AND supplier_id = ").push_bind(supplier_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(method) = filter.payment_method {
            builder.push(" AND payment_method = ").push_bind(method.to_string());
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_payment_run_async(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let run_number = generate_payment_run_number();
        let payment_date = to_date(input.payment_date);

        let mut total = Decimal::ZERO;
        for bill_id in &input.bill_ids {
            if let Some(bill) = self.get_bill_async(*bill_id).await? {
                total += bill.amount_due;
            }
        }

        sqlx::query(
            r#"
            INSERT INTO ap_payment_runs (id, run_number, status, payment_date, payment_method, total_amount, payment_count, notes, created_by, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)
            "#,
        )
        .bind(id)
        .bind(&run_number)
        .bind(PaymentRunStatus::Draft.to_string())
        .bind(payment_date)
        .bind(input.payment_method.to_string())
        .bind(total)
        .bind(input.bill_ids.len() as i32)
        .bind(&input.notes)
        .bind(&input.created_by)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        for bill_id in input.bill_ids {
            sqlx::query("INSERT INTO ap_payment_run_bills (run_id, bill_id) VALUES ($1,$2)")
                .bind(id)
                .bind(bill_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        self.get_payment_run_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create payment run".into()))
    }

    pub async fn get_payment_run_async(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        let row = sqlx::query_as::<_, PaymentRunRow>("SELECT * FROM ap_payment_runs WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_payment_run).transpose()
    }

    pub async fn list_payment_runs_async(
        &self,
        filter: PaymentRunFilter,
    ) -> Result<Vec<PaymentRun>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM ap_payment_runs WHERE 1=1");

        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND payment_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_value) = filter.to_date {
            builder.push(" AND payment_date <= ").push_bind(to_date(to_date_value));
        }

        builder.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PaymentRunRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_payment_run).collect::<Result<Vec<_>>>()
    }

    pub async fn approve_payment_run_async(
        &self,
        id: Uuid,
        approved_by: &str,
    ) -> Result<PaymentRun> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE ap_payment_runs SET status = $1, approved_by = $2, approved_at = $3 WHERE id = $4",
        )
        .bind(PaymentRunStatus::Approved.to_string())
        .bind(approved_by)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_payment_run_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to approve run".into()))
    }

    pub async fn process_payment_run_async(&self, id: Uuid) -> Result<PaymentRun> {
        let now = Utc::now();

        sqlx::query("UPDATE ap_payment_runs SET status = $1, processed_at = $2 WHERE id = $3")
            .bind(PaymentRunStatus::Completed.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_payment_run_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to process run".into()))
    }

    pub async fn cancel_payment_run_async(&self, id: Uuid) -> Result<PaymentRun> {
        sqlx::query("UPDATE ap_payment_runs SET status = $1 WHERE id = $2")
            .bind(PaymentRunStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_payment_run_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel run".into()))
    }

    pub async fn get_payment_run_bills_async(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        let rows = sqlx::query_as::<_, BillRow>(
            "SELECT b.* FROM ap_bills b JOIN ap_payment_run_bills rb ON b.id = rb.bill_id WHERE rb.run_id = $1",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_bill).collect::<Result<Vec<_>>>()
    }

    pub async fn get_aging_summary_async(&self) -> Result<ApAgingSummary> {
        let current: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0) FROM ap_bills WHERE due_date >= CURRENT_DATE AND status NOT IN ('paid', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let days_1_30: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0) FROM ap_bills WHERE due_date < CURRENT_DATE AND due_date >= CURRENT_DATE - INTERVAL '30 days' AND status NOT IN ('paid', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let days_31_60: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0) FROM ap_bills WHERE due_date < CURRENT_DATE - INTERVAL '30 days' AND due_date >= CURRENT_DATE - INTERVAL '60 days' AND status NOT IN ('paid', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let days_61_90: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0) FROM ap_bills WHERE due_date < CURRENT_DATE - INTERVAL '60 days' AND due_date >= CURRENT_DATE - INTERVAL '90 days' AND status NOT IN ('paid', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let days_over_90: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0) FROM ap_bills WHERE due_date < CURRENT_DATE - INTERVAL '90 days' AND status NOT IN ('paid', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let total = current.0 + days_1_30.0 + days_31_60.0 + days_61_90.0 + days_over_90.0;

        Ok(ApAgingSummary {
            current: current.0,
            days_1_30: days_1_30.0,
            days_31_60: days_31_60.0,
            days_61_90: days_61_90.0,
            days_over_90: days_over_90.0,
            total,
        })
    }

    pub async fn get_supplier_summary_async(
        &self,
        supplier_id: Uuid,
    ) -> Result<Option<SupplierApSummary>> {
        let supplier_exists: Option<i32> =
            sqlx::query_scalar("SELECT 1 FROM suppliers WHERE id = $1")
                .bind(supplier_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        if supplier_exists.is_none() {
            return Ok(None);
        }

        let row: (Decimal, Decimal, i32) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0), COALESCE(SUM(CASE WHEN due_date < CURRENT_DATE THEN amount_due ELSE 0 END), 0), COUNT(*) FROM ap_bills WHERE supplier_id = $1 AND status NOT IN ('paid', 'cancelled')",
        )
        .bind(supplier_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Some(SupplierApSummary {
            supplier_id,
            supplier_name: None,
            total_outstanding: row.0,
            total_overdue: row.1,
            bill_count: row.2,
        }))
    }

    pub async fn get_total_outstanding_async(&self) -> Result<Decimal> {
        let row: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(amount_due), 0) FROM ap_bills WHERE status NOT IN ('paid', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.0)
    }

    pub async fn create_bills_batch_async(
        &self,
        inputs: Vec<CreateBill>,
    ) -> Result<BatchResult<Bill>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::new();
        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_bill_async(input).await {
                Ok(bill) => result.record_success(bill),
                Err(e) => result.record_failure(index, None, &e),
            }
        }
        Ok(result)
    }

    pub async fn get_bills_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM ap_bills WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<BillRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_bill).collect::<Result<Vec<_>>>()
    }
}

impl AccountsPayableRepository for PgAccountsPayableRepository {
    fn create_bill(&self, input: CreateBill) -> Result<Bill> {
        block_on(self.create_bill_async(input))
    }

    fn get_bill(&self, id: Uuid) -> Result<Option<Bill>> {
        block_on(self.get_bill_async(id))
    }

    fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>> {
        block_on(self.get_bill_by_number_async(number))
    }

    fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        block_on(self.update_bill_async(id, input))
    }

    fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        block_on(self.list_bills_async(filter))
    }

    fn delete_bill(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_bill_async(id))
    }

    fn approve_bill(&self, id: Uuid) -> Result<Bill> {
        block_on(self.approve_bill_async(id))
    }

    fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        block_on(self.cancel_bill_async(id))
    }

    fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        block_on(self.dispute_bill_async(id))
    }

    fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        block_on(self.get_bill_items_async(bill_id))
    }

    fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem> {
        block_on(self.add_bill_item_async(bill_id, item))
    }

    fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        block_on(self.remove_bill_item_async(item_id))
    }

    fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        block_on(self.count_bills_async(filter))
    }

    fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        block_on(self.get_overdue_bills_async())
    }

    fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        block_on(self.get_bills_due_soon_async(days))
    }

    fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment> {
        block_on(self.create_payment_async(input))
    }

    fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>> {
        block_on(self.get_payment_async(id))
    }

    fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>> {
        block_on(self.get_payment_by_number_async(number))
    }

    fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        block_on(self.list_payments_async(filter))
    }

    fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        block_on(self.void_payment_async(id))
    }

    fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        block_on(self.clear_payment_async(id))
    }

    fn get_payment_allocations(&self, payment_id: Uuid) -> Result<Vec<PaymentAllocation>> {
        block_on(self.get_payment_allocations_async(payment_id))
    }

    fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        block_on(self.get_payments_for_bill_async(bill_id))
    }

    fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64> {
        block_on(self.count_payments_async(filter))
    }

    fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        block_on(self.create_payment_run_async(input))
    }

    fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        block_on(self.get_payment_run_async(id))
    }

    fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        block_on(self.list_payment_runs_async(filter))
    }

    fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        block_on(self.approve_payment_run_async(id, approved_by))
    }

    fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        block_on(self.process_payment_run_async(id))
    }

    fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        block_on(self.cancel_payment_run_async(id))
    }

    fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        block_on(self.get_payment_run_bills_async(run_id))
    }

    fn get_aging_summary(&self) -> Result<ApAgingSummary> {
        block_on(self.get_aging_summary_async())
    }

    fn get_supplier_summary(&self, supplier_id: Uuid) -> Result<Option<SupplierApSummary>> {
        block_on(self.get_supplier_summary_async(supplier_id))
    }

    fn get_total_outstanding(&self) -> Result<Decimal> {
        block_on(self.get_total_outstanding_async())
    }

    fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>> {
        block_on(self.create_bills_batch_async(inputs))
    }

    fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        block_on(self.get_bills_batch_async(ids))
    }
}

fn to_date(dt: DateTime<Utc>) -> NaiveDate {
    dt.date_naive()
}

fn from_date(date: NaiveDate) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(date.and_time(NaiveTime::MIN), Utc)
}
