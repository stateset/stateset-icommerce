//! PostgreSQL implementation of purchase order repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    generate_po_number, generate_supplier_code, CommerceError, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreateSupplier, PaymentTerms, PurchaseOrder, PurchaseOrderFilter,
    PurchaseOrderItem, PurchaseOrderRepository, PurchaseOrderStatus, ReceivePurchaseOrderItems,
    Result, Supplier, SupplierFilter, UpdatePurchaseOrder, UpdateSupplier,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgPurchaseOrderRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct SupplierRow {
    id: Uuid,
    supplier_code: String,
    name: String,
    contact_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    address: Option<String>,
    city: Option<String>,
    state: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
    tax_id: Option<String>,
    payment_terms: String,
    currency: String,
    lead_time_days: Option<i32>,
    minimum_order: Option<Decimal>,
    is_active: bool,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PurchaseOrderRow {
    id: Uuid,
    po_number: String,
    supplier_id: Uuid,
    status: String,
    order_date: DateTime<Utc>,
    expected_date: Option<DateTime<Utc>>,
    delivered_date: Option<DateTime<Utc>>,
    ship_to_address: Option<String>,
    ship_to_city: Option<String>,
    ship_to_state: Option<String>,
    ship_to_postal_code: Option<String>,
    ship_to_country: Option<String>,
    payment_terms: String,
    currency: String,
    subtotal: Decimal,
    tax_amount: Decimal,
    shipping_cost: Decimal,
    discount_amount: Decimal,
    total: Decimal,
    amount_paid: Decimal,
    supplier_reference: Option<String>,
    notes: Option<String>,
    supplier_notes: Option<String>,
    approved_by: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    sent_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PurchaseOrderItemRow {
    id: Uuid,
    purchase_order_id: Uuid,
    product_id: Option<Uuid>,
    sku: String,
    name: String,
    supplier_sku: Option<String>,
    quantity_ordered: Decimal,
    quantity_received: Decimal,
    unit_of_measure: Option<String>,
    unit_cost: Decimal,
    line_total: Decimal,
    tax_amount: Decimal,
    discount_amount: Decimal,
    expected_date: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPurchaseOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_payment_terms(s: &str) -> PaymentTerms {
        s.parse().unwrap_or_default()
    }

    fn parse_status(s: &str) -> PurchaseOrderStatus {
        s.parse().unwrap_or_default()
    }

    fn row_to_supplier(row: SupplierRow) -> Supplier {
        Supplier {
            id: row.id,
            supplier_code: row.supplier_code,
            name: row.name,
            contact_name: row.contact_name,
            email: row.email,
            phone: row.phone,
            website: row.website,
            address: row.address,
            city: row.city,
            state: row.state,
            postal_code: row.postal_code,
            country: row.country,
            tax_id: row.tax_id,
            payment_terms: Self::parse_payment_terms(&row.payment_terms),
            currency: row.currency,
            lead_time_days: row.lead_time_days,
            minimum_order: row.minimum_order,
            is_active: row.is_active,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_po(row: PurchaseOrderRow, items: Vec<PurchaseOrderItem>) -> PurchaseOrder {
        PurchaseOrder {
            id: row.id,
            po_number: row.po_number,
            supplier_id: row.supplier_id,
            status: Self::parse_status(&row.status),
            order_date: row.order_date,
            expected_date: row.expected_date,
            delivered_date: row.delivered_date,
            ship_to_address: row.ship_to_address,
            ship_to_city: row.ship_to_city,
            ship_to_state: row.ship_to_state,
            ship_to_postal_code: row.ship_to_postal_code,
            ship_to_country: row.ship_to_country,
            payment_terms: Self::parse_payment_terms(&row.payment_terms),
            currency: row.currency,
            subtotal: row.subtotal,
            tax_amount: row.tax_amount,
            shipping_cost: row.shipping_cost,
            discount_amount: row.discount_amount,
            total: row.total,
            amount_paid: row.amount_paid,
            supplier_reference: row.supplier_reference,
            notes: row.notes,
            supplier_notes: row.supplier_notes,
            approved_by: row.approved_by,
            approved_at: row.approved_at,
            items,
            sent_at: row.sent_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_item(row: PurchaseOrderItemRow) -> PurchaseOrderItem {
        PurchaseOrderItem {
            id: row.id,
            purchase_order_id: row.purchase_order_id,
            product_id: row.product_id,
            sku: row.sku,
            name: row.name,
            supplier_sku: row.supplier_sku,
            quantity_ordered: row.quantity_ordered,
            quantity_received: row.quantity_received,
            unit_of_measure: row.unit_of_measure,
            unit_cost: row.unit_cost,
            line_total: row.line_total,
            tax_amount: row.tax_amount,
            discount_amount: row.discount_amount,
            expected_date: row.expected_date,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn load_items_async(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        let rows = sqlx::query_as::<_, PurchaseOrderItemRow>(
            "SELECT * FROM purchase_order_items WHERE purchase_order_id = $1"
        )
        .bind(po_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    async fn recalculate_totals_async(&self, po_id: Uuid) -> Result<()> {
        let subtotal: Option<Decimal> = sqlx::query_scalar(
            "SELECT COALESCE(SUM(line_total), 0) FROM purchase_order_items WHERE purchase_order_id = $1"
        )
        .bind(po_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let subtotal = subtotal.unwrap_or_default();

        let (tax_amount, shipping_cost, discount_amount): (Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT tax_amount, shipping_cost, discount_amount FROM purchase_orders WHERE id = $1"
        )
        .bind(po_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let total = subtotal + tax_amount + shipping_cost - discount_amount;

        sqlx::query("UPDATE purchase_orders SET subtotal = $1, total = $2, updated_at = $3 WHERE id = $4")
            .bind(subtotal)
            .bind(total)
            .bind(Utc::now())
            .bind(po_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    // Supplier methods
    pub async fn create_supplier_async(&self, input: CreateSupplier) -> Result<Supplier> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let code = input.supplier_code.unwrap_or_else(generate_supplier_code);

        sqlx::query(
            "INSERT INTO suppliers (id, supplier_code, name, contact_name, email, phone, website,
             address, city, state, postal_code, country, tax_id, payment_terms, currency,
             lead_time_days, minimum_order, is_active, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)"
        )
        .bind(id)
        .bind(&code)
        .bind(&input.name)
        .bind(&input.contact_name)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.website)
        .bind(&input.address)
        .bind(&input.city)
        .bind(&input.state)
        .bind(&input.postal_code)
        .bind(&input.country)
        .bind(&input.tax_id)
        .bind(input.payment_terms.unwrap_or_default().to_string())
        .bind(input.currency.unwrap_or_else(|| "USD".to_string()))
        .bind(input.lead_time_days)
        .bind(input.minimum_order)
        .bind(true)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_supplier_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_supplier_async(&self, id: Uuid) -> Result<Option<Supplier>> {
        let row = sqlx::query_as::<_, SupplierRow>("SELECT * FROM suppliers WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_supplier))
    }

    pub async fn get_supplier_by_code_async(&self, code: &str) -> Result<Option<Supplier>> {
        let row = sqlx::query_as::<_, SupplierRow>("SELECT * FROM suppliers WHERE supplier_code = $1")
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_supplier))
    }

    pub async fn update_supplier_async(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        let supplier = self.get_supplier_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE suppliers SET name = $1, contact_name = $2, email = $3, phone = $4, website = $5,
             address = $6, city = $7, state = $8, postal_code = $9, country = $10, tax_id = $11,
             payment_terms = $12, currency = $13, lead_time_days = $14, minimum_order = $15,
             is_active = $16, notes = $17, updated_at = $18 WHERE id = $19"
        )
        .bind(input.name.unwrap_or(supplier.name))
        .bind(input.contact_name.or(supplier.contact_name))
        .bind(input.email.or(supplier.email))
        .bind(input.phone.or(supplier.phone))
        .bind(input.website.or(supplier.website))
        .bind(input.address.or(supplier.address))
        .bind(input.city.or(supplier.city))
        .bind(input.state.or(supplier.state))
        .bind(input.postal_code.or(supplier.postal_code))
        .bind(input.country.or(supplier.country))
        .bind(input.tax_id.or(supplier.tax_id))
        .bind(input.payment_terms.unwrap_or(supplier.payment_terms).to_string())
        .bind(input.currency.unwrap_or(supplier.currency))
        .bind(input.lead_time_days.or(supplier.lead_time_days))
        .bind(input.minimum_order.or(supplier.minimum_order))
        .bind(input.is_active.unwrap_or(supplier.is_active))
        .bind(input.notes.or(supplier.notes))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_supplier_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_suppliers_async(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let mut query = String::from("SELECT * FROM suppliers WHERE 1=1");

        if filter.active_only.unwrap_or(false) {
            query.push_str(" AND is_active = true");
        }

        query.push_str(&format!(" ORDER BY name ASC LIMIT {}", limit));

        let rows = sqlx::query_as::<_, SupplierRow>(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_supplier).collect())
    }

    pub async fn delete_supplier_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE suppliers SET is_active = false, updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    // Purchase Order methods
    pub async fn create_async(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        let supplier = self.get_supplier_async(input.supplier_id).await?.ok_or(CommerceError::NotFound)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let po_number = generate_po_number();
        let order_date = input.order_date.unwrap_or(now);

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO purchase_orders (id, po_number, supplier_id, status, order_date,
             expected_date, ship_to_address, ship_to_city, ship_to_state, ship_to_postal_code,
             ship_to_country, payment_terms, currency, subtotal, tax_amount, shipping_cost,
             discount_amount, total, amount_paid, notes, supplier_notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)"
        )
        .bind(id)
        .bind(&po_number)
        .bind(input.supplier_id)
        .bind(PurchaseOrderStatus::Draft.to_string())
        .bind(order_date)
        .bind(input.expected_date)
        .bind(&input.ship_to_address)
        .bind(&input.ship_to_city)
        .bind(&input.ship_to_state)
        .bind(&input.ship_to_postal_code)
        .bind(&input.ship_to_country)
        .bind(input.payment_terms.unwrap_or(supplier.payment_terms).to_string())
        .bind(input.currency.unwrap_or(supplier.currency))
        .bind(Decimal::ZERO)
        .bind(input.tax_amount.unwrap_or_default())
        .bind(input.shipping_cost.unwrap_or_default())
        .bind(input.discount_amount.unwrap_or_default())
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(&input.notes)
        .bind(&input.supplier_notes)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        for item in &input.items {
            let item_id = Uuid::new_v4();
            let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default()
                + item.tax_amount.unwrap_or_default();

            sqlx::query(
                "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
                 supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
                 line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)"
            )
            .bind(item_id)
            .bind(id)
            .bind(item.product_id)
            .bind(&item.sku)
            .bind(&item.name)
            .bind(&item.supplier_sku)
            .bind(item.quantity)
            .bind(Decimal::ZERO)
            .bind(&item.unit_of_measure)
            .bind(item.unit_cost)
            .bind(line_total)
            .bind(item.tax_amount.unwrap_or_default())
            .bind(item.discount_amount.unwrap_or_default())
            .bind(item.expected_date)
            .bind(&item.notes)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.recalculate_totals_async(id).await?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
        let row = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let items = self.load_items_async(row.id).await?;
                Ok(Some(Self::row_to_po(row, items)))
            }
            None => Ok(None),
        }
    }

    pub async fn get_by_number_async(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        let row = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE po_number = $1")
            .bind(po_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let items = self.load_items_async(row.id).await?;
                Ok(Some(Self::row_to_po(row, items)))
            }
            None => Ok(None),
        }
    }

    pub async fn update_async(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        let po = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        sqlx::query(
            "UPDATE purchase_orders SET expected_date = $1, ship_to_address = $2, ship_to_city = $3,
             ship_to_state = $4, ship_to_postal_code = $5, ship_to_country = $6, payment_terms = $7,
             tax_amount = $8, shipping_cost = $9, discount_amount = $10, notes = $11, supplier_notes = $12,
             supplier_reference = $13, updated_at = $14 WHERE id = $15"
        )
        .bind(input.expected_date.or(po.expected_date))
        .bind(input.ship_to_address.or(po.ship_to_address))
        .bind(input.ship_to_city.or(po.ship_to_city))
        .bind(input.ship_to_state.or(po.ship_to_state))
        .bind(input.ship_to_postal_code.or(po.ship_to_postal_code))
        .bind(input.ship_to_country.or(po.ship_to_country))
        .bind(input.payment_terms.unwrap_or(po.payment_terms).to_string())
        .bind(input.tax_amount.unwrap_or(po.tax_amount))
        .bind(input.shipping_cost.unwrap_or(po.shipping_cost))
        .bind(input.discount_amount.unwrap_or(po.discount_amount))
        .bind(input.notes.or(po.notes))
        .bind(input.supplier_notes.or(po.supplier_notes))
        .bind(input.supplier_reference.or(po.supplier_reference))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_totals_async(id).await?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_async(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let mut query = String::from("SELECT * FROM purchase_orders WHERE 1=1");
        let mut param_idx = 1;

        if filter.supplier_id.is_some() {
            query.push_str(&format!(" AND supplier_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(" ORDER BY order_date DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));

        let mut q = sqlx::query_as::<_, PurchaseOrderRow>(&query);

        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut orders = Vec::new();
        for row in rows {
            let items = self.load_items_async(row.id).await?;
            orders.push(Self::row_to_po(row, items));
        }
        Ok(orders)
    }

    pub async fn for_supplier_async(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.list_async(PurchaseOrderFilter { supplier_id: Some(supplier_id), ..Default::default() }).await
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let status: String = sqlx::query_scalar("SELECT status FROM purchase_orders WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        if Self::parse_status(&status) != PurchaseOrderStatus::Draft {
            return Err(CommerceError::ValidationError("Can only delete draft purchase orders".to_string()));
        }

        sqlx::query("DELETE FROM purchase_order_items WHERE purchase_order_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM purchase_orders WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    async fn update_status_async(&self, id: Uuid, status: PurchaseOrderStatus) -> Result<PurchaseOrder> {
        sqlx::query("UPDATE purchase_orders SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn submit_for_approval_async(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.update_status_async(id, PurchaseOrderStatus::PendingApproval).await
    }

    pub async fn approve_async(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder> {
        let now = Utc::now();
        sqlx::query("UPDATE purchase_orders SET status = $1, approved_by = $2, approved_at = $3, updated_at = $4 WHERE id = $5")
            .bind(PurchaseOrderStatus::Approved.to_string())
            .bind(approved_by)
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn send_async(&self, id: Uuid) -> Result<PurchaseOrder> {
        let now = Utc::now();
        sqlx::query("UPDATE purchase_orders SET status = $1, sent_at = $2, updated_at = $3 WHERE id = $4")
            .bind(PurchaseOrderStatus::Sent.to_string())
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn acknowledge_async(&self, id: Uuid, supplier_reference: Option<&str>) -> Result<PurchaseOrder> {
        let now = Utc::now();
        sqlx::query("UPDATE purchase_orders SET status = $1, supplier_reference = COALESCE($2, supplier_reference), updated_at = $3 WHERE id = $4")
            .bind(PurchaseOrderStatus::Acknowledged.to_string())
            .bind(supplier_reference)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn hold_async(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.update_status_async(id, PurchaseOrderStatus::OnHold).await
    }

    pub async fn cancel_async(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.update_status_async(id, PurchaseOrderStatus::Cancelled).await
    }

    pub async fn receive_async(&self, id: Uuid, items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder> {
        let now = Utc::now();

        for item in items.items {
            sqlx::query("UPDATE purchase_order_items SET quantity_received = quantity_received + $1, updated_at = $2 WHERE id = $3")
                .bind(item.quantity_received)
                .bind(now)
                .bind(item.item_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        // Check receipt status
        let items = self.load_items_async(id).await?;
        let all_received = items.iter().all(|i| i.quantity_received >= i.quantity_ordered);
        let any_received = items.iter().any(|i| i.quantity_received > Decimal::ZERO);

        let new_status = if all_received {
            PurchaseOrderStatus::Received
        } else if any_received {
            PurchaseOrderStatus::PartiallyReceived
        } else {
            return self.get_async(id).await?.ok_or(CommerceError::NotFound);
        };

        let delivered_date = if all_received { Some(now) } else { None };

        sqlx::query("UPDATE purchase_orders SET status = $1, delivered_date = COALESCE($2, delivered_date), updated_at = $3 WHERE id = $4")
            .bind(new_status.to_string())
            .bind(delivered_date)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn complete_async(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.update_status_async(id, PurchaseOrderStatus::Completed).await
    }

    pub async fn add_item_async(&self, po_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default() + item.tax_amount.unwrap_or_default();

        sqlx::query(
            "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
             supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
             line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)"
        )
        .bind(id)
        .bind(po_id)
        .bind(item.product_id)
        .bind(&item.sku)
        .bind(&item.name)
        .bind(&item.supplier_sku)
        .bind(item.quantity)
        .bind(Decimal::ZERO)
        .bind(&item.unit_of_measure)
        .bind(item.unit_cost)
        .bind(line_total)
        .bind(item.tax_amount.unwrap_or_default())
        .bind(item.discount_amount.unwrap_or_default())
        .bind(item.expected_date)
        .bind(&item.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_totals_async(po_id).await?;

        let row = sqlx::query_as::<_, PurchaseOrderItemRow>("SELECT * FROM purchase_order_items WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(Self::row_to_item(row))
    }

    pub async fn update_item_async(&self, item_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        let now = Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default() + item.tax_amount.unwrap_or_default();

        let po_id: Uuid = sqlx::query_scalar("SELECT purchase_order_id FROM purchase_order_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE purchase_order_items SET sku = $1, name = $2, supplier_sku = $3,
             quantity_ordered = $4, unit_cost = $5, line_total = $6, tax_amount = $7,
             discount_amount = $8, expected_date = $9, notes = $10, updated_at = $11 WHERE id = $12"
        )
        .bind(&item.sku)
        .bind(&item.name)
        .bind(&item.supplier_sku)
        .bind(item.quantity)
        .bind(item.unit_cost)
        .bind(line_total)
        .bind(item.tax_amount.unwrap_or_default())
        .bind(item.discount_amount.unwrap_or_default())
        .bind(item.expected_date)
        .bind(&item.notes)
        .bind(now)
        .bind(item_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_totals_async(po_id).await?;

        let row = sqlx::query_as::<_, PurchaseOrderItemRow>("SELECT * FROM purchase_order_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(Self::row_to_item(row))
    }

    pub async fn remove_item_async(&self, item_id: Uuid) -> Result<()> {
        let po_id: Uuid = sqlx::query_scalar("SELECT purchase_order_id FROM purchase_order_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM purchase_order_items WHERE id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.recalculate_totals_async(po_id).await?;
        Ok(())
    }

    pub async fn get_items_async(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        self.load_items_async(po_id).await
    }

    pub async fn count_async(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM purchase_orders WHERE 1=1");
        let mut param_idx = 1;

        if filter.supplier_id.is_some() {
            query.push_str(&format!(" AND supplier_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    pub async fn count_suppliers_async(&self, filter: SupplierFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM suppliers WHERE 1=1");

        if filter.active_only.unwrap_or(false) {
            query.push_str(" AND is_active = true");
        }

        let (count,): (i64,) = sqlx::query_as(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count as u64)
    }
}

impl PurchaseOrderRepository for PgPurchaseOrderRepository {
    fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier> {
        tokio::runtime::Handle::current().block_on(self.create_supplier_async(input))
    }

    fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>> {
        tokio::runtime::Handle::current().block_on(self.get_supplier_async(id))
    }

    fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>> {
        tokio::runtime::Handle::current().block_on(self.get_supplier_by_code_async(code))
    }

    fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        tokio::runtime::Handle::current().block_on(self.update_supplier_async(id, input))
    }

    fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        tokio::runtime::Handle::current().block_on(self.list_suppliers_async(filter))
    }

    fn delete_supplier(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_supplier_async(id))
    }

    fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(po_number))
    }

    fn update(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        tokio::runtime::Handle::current().block_on(self.for_supplier_async(supplier_id))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_async(id))
    }

    fn submit_for_approval(&self, id: Uuid) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.submit_for_approval_async(id))
    }

    fn approve(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.approve_async(id, approved_by))
    }

    fn send(&self, id: Uuid) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.send_async(id))
    }

    fn acknowledge(&self, id: Uuid, supplier_reference: Option<&str>) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.acknowledge_async(id, supplier_reference))
    }

    fn hold(&self, id: Uuid) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.hold_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.cancel_async(id))
    }

    fn receive(&self, id: Uuid, items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.receive_async(id, items))
    }

    fn complete(&self, id: Uuid) -> Result<PurchaseOrder> {
        tokio::runtime::Handle::current().block_on(self.complete_async(id))
    }

    fn add_item(&self, po_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        tokio::runtime::Handle::current().block_on(self.add_item_async(po_id, item))
    }

    fn update_item(&self, item_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        tokio::runtime::Handle::current().block_on(self.update_item_async(item_id, item))
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.remove_item_async(item_id))
    }

    fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        tokio::runtime::Handle::current().block_on(self.get_items_async(po_id))
    }

    fn count(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }

    fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_suppliers_async(filter))
    }
}
