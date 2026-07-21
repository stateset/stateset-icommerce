//! PostgreSQL implementation of purchase order repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BatchResult, CommerceError, CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier,
    CurrencyCode, PaymentTerms, PurchaseOrder, PurchaseOrderFilter, PurchaseOrderId,
    PurchaseOrderItem, PurchaseOrderRepository, PurchaseOrderStatus, ReceivePurchaseOrderItems,
    Result, Supplier, SupplierFilter, UpdatePurchaseOrder, UpdateSupplier, generate_po_number,
    generate_supplier_code, validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
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
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_supplier(row: SupplierRow) -> Result<Supplier> {
        let SupplierRow {
            id,
            supplier_code,
            name,
            contact_name,
            email,
            phone,
            website,
            address,
            city,
            state,
            postal_code,
            country,
            tax_id,
            payment_terms,
            currency,
            lead_time_days,
            minimum_order,
            is_active,
            notes,
            created_at,
            updated_at,
        } = row;

        let payment_terms: PaymentTerms = payment_terms.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid supplier.payment_terms '{}': {}",
                payment_terms, e
            ))
        })?;
        let currency: CurrencyCode = currency.parse().unwrap_or(CurrencyCode::USD);

        Ok(Supplier {
            id,
            supplier_code,
            name,
            contact_name,
            email,
            phone,
            website,
            address,
            city,
            state,
            postal_code,
            country,
            tax_id,
            payment_terms,
            currency,
            lead_time_days,
            minimum_order,
            is_active,
            notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_po(row: PurchaseOrderRow, items: Vec<PurchaseOrderItem>) -> Result<PurchaseOrder> {
        let PurchaseOrderRow {
            id,
            po_number,
            supplier_id,
            status,
            order_date,
            expected_date,
            delivered_date,
            ship_to_address,
            ship_to_city,
            ship_to_state,
            ship_to_postal_code,
            ship_to_country,
            payment_terms,
            currency,
            subtotal,
            tax_amount,
            shipping_cost,
            discount_amount,
            total,
            amount_paid,
            supplier_reference,
            notes,
            supplier_notes,
            approved_by,
            approved_at,
            sent_at,
            created_at,
            updated_at,
        } = row;

        let status: PurchaseOrderStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid purchase_order.status '{}': {}",
                status, e
            ))
        })?;
        let payment_terms: PaymentTerms = payment_terms.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid purchase_order.payment_terms '{}': {}",
                payment_terms, e
            ))
        })?;
        let currency: CurrencyCode = currency.parse().unwrap_or(CurrencyCode::USD);

        Ok(PurchaseOrder {
            id: id.into(),
            po_number,
            supplier_id,
            status,
            order_date,
            expected_date,
            delivered_date,
            ship_to_address,
            ship_to_city,
            ship_to_state,
            ship_to_postal_code,
            ship_to_country,
            payment_terms,
            currency,
            subtotal,
            tax_amount,
            shipping_cost,
            discount_amount,
            total,
            amount_paid,
            supplier_reference,
            notes,
            supplier_notes,
            approved_by,
            approved_at,
            items,
            sent_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_item(row: PurchaseOrderItemRow) -> PurchaseOrderItem {
        PurchaseOrderItem {
            id: row.id,
            purchase_order_id: row.purchase_order_id.into(),
            product_id: row.product_id.map(Into::into),
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
            "SELECT * FROM purchase_order_items WHERE purchase_order_id = $1",
        )
        .bind(po_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    async fn load_items_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<PurchaseOrderItem>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<PurchaseOrderItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows = sqlx::query_as::<_, PurchaseOrderItemRow>(
            "SELECT * FROM purchase_order_items WHERE purchase_order_id = ANY($1)",
        )
        .bind(ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.purchase_order_id;
            map.entry(parent).or_default().push(Self::row_to_item(row));
        }
        Ok(map)
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

        sqlx::query(
            "UPDATE purchase_orders SET subtotal = $1, total = $2, updated_at = $3 WHERE id = $4",
        )
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
        .bind(input.currency.unwrap_or(CurrencyCode::USD).as_str())
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

        row.map(Self::row_to_supplier).transpose()
    }

    pub async fn get_supplier_by_code_async(&self, code: &str) -> Result<Option<Supplier>> {
        let row =
            sqlx::query_as::<_, SupplierRow>("SELECT * FROM suppliers WHERE supplier_code = $1")
                .bind(code)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        row.map(Self::row_to_supplier).transpose()
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
        .bind(input.currency.unwrap_or(supplier.currency).as_str())
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
        let limit = super::effective_limit(filter.limit);
        let offset = filter.offset.unwrap_or(0) as i64;
        let mut query = String::from("SELECT * FROM suppliers WHERE 1=1");
        let mut param_idx = 1;

        // Honor the same filters as the SQLite backend (previously Postgres only
        // applied active_only and silently ignored name/country).
        if filter.name.is_some() {
            query.push_str(&format!(" AND name ILIKE ${param_idx}"));
            param_idx += 1;
        }
        if filter.country.is_some() {
            query.push_str(&format!(" AND country = ${param_idx}"));
            param_idx += 1;
        }
        if filter.active_only.unwrap_or(false) {
            query.push_str(" AND is_active = true");
        }

        query.push_str(&format!(" ORDER BY name ASC LIMIT ${param_idx} OFFSET ${}", param_idx + 1));

        let mut q = sqlx::query_as::<_, SupplierRow>(&query);
        if let Some(name) = filter.name {
            q = q.bind(format!("%{name}%"));
        }
        if let Some(country) = filter.country {
            q = q.bind(country);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut suppliers = Vec::with_capacity(rows.len());
        for row in rows {
            suppliers.push(Self::row_to_supplier(row)?);
        }
        Ok(suppliers)
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
        let supplier =
            self.get_supplier_async(input.supplier_id).await?.ok_or(CommerceError::NotFound)?;

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
        .bind(input.currency.unwrap_or(supplier.currency).as_str())
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for item in &input.items {
            let item_id = Uuid::new_v4();
            let line_total = item.quantity * item.unit_cost
                - item.discount_amount.unwrap_or_default()
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
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.recalculate_totals_async(id).await?;
        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
        let row =
            sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let items = self.load_items_async(row.id).await?;
                Ok(Some(Self::row_to_po(row, items)?))
            }
            None => Ok(None),
        }
    }

    pub async fn get_by_number_async(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        let row = sqlx::query_as::<_, PurchaseOrderRow>(
            "SELECT * FROM purchase_orders WHERE po_number = $1",
        )
        .bind(po_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let items = self.load_items_async(row.id).await?;
                Ok(Some(Self::row_to_po(row, items)?))
            }
            None => Ok(None),
        }
    }

    pub async fn update_async(
        &self,
        id: Uuid,
        input: UpdatePurchaseOrder,
    ) -> Result<PurchaseOrder> {
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
        let after_cursor = super::parse_after_cursor(filter.after_cursor.as_ref())?;
        let limit = super::effective_limit(filter.limit);
        // Offset pagination applies only in non-cursor mode.
        let offset = if after_cursor.is_none() { filter.offset.unwrap_or(0) as i64 } else { 0 };

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
        if filter.from_date.is_some() {
            query.push_str(&format!(" AND order_date >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.to_date.is_some() {
            query.push_str(&format!(" AND order_date <= ${}", param_idx));
            param_idx += 1;
        }
        if filter.min_total.is_some() {
            query.push_str(&format!(" AND total >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.max_total.is_some() {
            query.push_str(&format!(" AND total <= ${}", param_idx));
            param_idx += 1;
        }

        if after_cursor.is_some() {
            // Keyset cursor: (order_date, id) for stable DESC ordering
            query.push_str(&format!(
                " AND (order_date < ${param_idx} OR (order_date = ${param_idx} AND id < ${}))",
                param_idx + 1
            ));
            param_idx += 2;
        }

        query.push_str(&format!(
            " ORDER BY order_date DESC, id DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PurchaseOrderRow>(&query);

        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            q = q.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            q = q.bind(to_date);
        }
        if let Some(min_total) = filter.min_total {
            q = q.bind(min_total);
        }
        if let Some(max_total) = filter.max_total {
            q = q.bind(max_total);
        }
        if let Some((cursor_date, cursor_id)) = after_cursor {
            q = q.bind(cursor_date).bind(cursor_id);
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.load_items_batch_async(&ids).await?;
        let mut orders = Vec::new();
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            orders.push(Self::row_to_po(row, items)?);
        }
        Ok(orders)
    }

    pub async fn for_supplier_async(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.list_async(PurchaseOrderFilter {
            supplier_id: Some(supplier_id),
            ..Default::default()
        })
        .await
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let status: String = sqlx::query_scalar("SELECT status FROM purchase_orders WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        let parsed_status: PurchaseOrderStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid purchase_order.status '{}': {}",
                status, e
            ))
        })?;
        if parsed_status != PurchaseOrderStatus::Draft {
            return Err(CommerceError::ValidationError(
                "Can only delete draft purchase orders".to_string(),
            ));
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

    async fn update_status_async(
        &self,
        id: Uuid,
        status: PurchaseOrderStatus,
    ) -> Result<PurchaseOrder> {
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
        sqlx::query(
            "UPDATE purchase_orders SET status = $1, sent_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(PurchaseOrderStatus::Sent.to_string())
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn acknowledge_async(
        &self,
        id: Uuid,
        supplier_reference: Option<&str>,
    ) -> Result<PurchaseOrder> {
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

    pub async fn receive_async(
        &self,
        id: Uuid,
        items: ReceivePurchaseOrderItems,
    ) -> Result<PurchaseOrder> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        for item in &items.items {
            if item.quantity_received <= Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Received quantity must be greater than zero".to_string(),
                ));
            }

            let result = sqlx::query(
                "UPDATE purchase_order_items
                 SET quantity_received = quantity_received + $1, updated_at = $2
                 WHERE id = $3
                   AND purchase_order_id = $4
                   AND quantity_received + $1 <= quantity_ordered",
            )
            .bind(item.quantity_received)
            .bind(now)
            .bind(item.item_id)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            if result.rows_affected() == 0 {
                let exists: Option<bool> = sqlx::query_scalar(
                    "SELECT TRUE FROM purchase_order_items WHERE id = $1 AND purchase_order_id = $2",
                )
                .bind(item.item_id)
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;

                if exists.is_none() {
                    return Err(CommerceError::NotFound);
                }

                return Err(CommerceError::ValidationError(format!(
                    "Receiving quantity would exceed ordered quantity for item {}",
                    item.item_id
                )));
            }
        }

        // Check receipt status using the same transaction snapshot.
        let item_rows: Vec<(Decimal, Decimal)> = sqlx::query_as(
            "SELECT quantity_ordered, quantity_received FROM purchase_order_items WHERE purchase_order_id = $1",
        )
        .bind(id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let all_received = !item_rows.is_empty()
            && item_rows.iter().all(|(ordered, received)| received >= ordered);
        let any_received = item_rows.iter().any(|(_, received)| *received > Decimal::ZERO);

        let new_status = if all_received {
            PurchaseOrderStatus::Received
        } else if any_received {
            PurchaseOrderStatus::PartiallyReceived
        } else {
            tx.commit().await.map_err(map_db_error)?;
            return self.get_async(id).await?.ok_or(CommerceError::NotFound);
        };

        let delivered_date = if all_received { Some(now) } else { None };

        sqlx::query("UPDATE purchase_orders SET status = $1, delivered_date = COALESCE($2, delivered_date), updated_at = $3 WHERE id = $4")
            .bind(new_status.to_string())
            .bind(delivered_date)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn complete_async(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.update_status_async(id, PurchaseOrderStatus::Completed).await
    }

    pub async fn add_item_async(
        &self,
        po_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

        sqlx::query(
            "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
             supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
             line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
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

        let row = sqlx::query_as::<_, PurchaseOrderItemRow>(
            "SELECT * FROM purchase_order_items WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Self::row_to_item(row))
    }

    pub async fn update_item_async(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        let now = Utc::now();
        let line_total = item.quantity * item.unit_cost - item.discount_amount.unwrap_or_default()
            + item.tax_amount.unwrap_or_default();

        let po_id: Uuid =
            sqlx::query_scalar("SELECT purchase_order_id FROM purchase_order_items WHERE id = $1")
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

        let row = sqlx::query_as::<_, PurchaseOrderItemRow>(
            "SELECT * FROM purchase_order_items WHERE id = $1",
        )
        .bind(item_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Self::row_to_item(row))
    }

    pub async fn remove_item_async(&self, item_id: Uuid) -> Result<()> {
        let po_id: Uuid =
            sqlx::query_scalar("SELECT purchase_order_id FROM purchase_order_items WHERE id = $1")
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
            param_idx += 1;
        }
        if filter.from_date.is_some() {
            query.push_str(&format!(" AND order_date >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.to_date.is_some() {
            query.push_str(&format!(" AND order_date <= ${}", param_idx));
            param_idx += 1;
        }
        if filter.min_total.is_some() {
            query.push_str(&format!(" AND total >= ${}", param_idx));
            param_idx += 1;
        }
        if filter.max_total.is_some() {
            query.push_str(&format!(" AND total <= ${}", param_idx));
            param_idx += 1;
        }
        let _ = param_idx;

        let mut q = sqlx::query_as::<_, (i64,)>(&query);

        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            q = q.bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            q = q.bind(to_date);
        }
        if let Some(min_total) = filter.min_total {
            q = q.bind(min_total);
        }
        if let Some(max_total) = filter.max_total {
            q = q.bind(max_total);
        }

        let (count,) = q.fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(count as u64)
    }

    pub async fn count_suppliers_async(&self, filter: SupplierFilter) -> Result<u64> {
        let mut query = String::from("SELECT COUNT(*) FROM suppliers WHERE 1=1");

        if filter.active_only.unwrap_or(false) {
            query.push_str(" AND is_active = true");
        }

        let (count,): (i64,) =
            sqlx::query_as(&query).fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    /// Create multiple purchase orders in a batch (async, non-atomic with partial success)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreatePurchaseOrder>,
    ) -> Result<BatchResult<PurchaseOrder>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(po) => result.record_success(po),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple purchase orders in a batch atomically (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreatePurchaseOrder>,
    ) -> Result<Vec<PurchaseOrder>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut created_ids = Vec::with_capacity(inputs.len());

        for input in inputs {
            // Verify supplier exists
            let supplier_exists: Option<(Uuid,)> =
                sqlx::query_as("SELECT id FROM suppliers WHERE id = $1")
                    .bind(input.supplier_id)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;

            if supplier_exists.is_none() {
                return Err(CommerceError::NotFound);
            }

            // Get supplier info for defaults
            let (payment_terms, currency_str): (String, String) =
                sqlx::query_as("SELECT payment_terms, currency FROM suppliers WHERE id = $1")
                    .bind(input.supplier_id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            let currency: CurrencyCode = currency_str.parse().unwrap_or(CurrencyCode::USD);

            let id = Uuid::new_v4();
            let now = Utc::now();
            let po_number = generate_po_number();
            let order_date = input.order_date.unwrap_or(now);

            sqlx::query(
                "INSERT INTO purchase_orders (id, po_number, supplier_id, status, order_date,
                 expected_date, ship_to_address, ship_to_city, ship_to_state, ship_to_postal_code,
                 ship_to_country, payment_terms, currency, subtotal, tax_amount, shipping_cost,
                 discount_amount, total, amount_paid, notes, supplier_notes, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)",
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
            .bind(
                input
                    .payment_terms
                    .map(|pt| pt.to_string())
                    .unwrap_or(payment_terms),
            )
            .bind(input.currency.unwrap_or(currency).as_str())
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
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Insert items
            for item in &input.items {
                let item_id = Uuid::new_v4();
                let line_total = item.quantity * item.unit_cost
                    - item.discount_amount.unwrap_or_default()
                    + item.tax_amount.unwrap_or_default();

                sqlx::query(
                    "INSERT INTO purchase_order_items (id, purchase_order_id, product_id, sku, name,
                     supplier_sku, quantity_ordered, quantity_received, unit_of_measure, unit_cost,
                     line_total, tax_amount, discount_amount, expected_date, notes, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
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
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }

            // Calculate and update totals
            let subtotal: Option<Decimal> = sqlx::query_scalar(
                "SELECT COALESCE(SUM(line_total), 0) FROM purchase_order_items WHERE purchase_order_id = $1",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let subtotal = subtotal.unwrap_or_default();
            let tax_amount = input.tax_amount.unwrap_or_default();
            let shipping_cost = input.shipping_cost.unwrap_or_default();
            let discount_amount = input.discount_amount.unwrap_or_default();
            let total = subtotal + tax_amount + shipping_cost - discount_amount;

            sqlx::query(
                "UPDATE purchase_orders SET subtotal = $1, total = $2, updated_at = $3 WHERE id = $4",
            )
            .bind(subtotal)
            .bind(total)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            created_ids.push(id);
        }

        tx.commit().await.map_err(map_db_error)?;

        // Fetch all created purchase orders
        let mut orders = Vec::with_capacity(created_ids.len());
        for id in created_ids {
            if let Some(po) = self.get_async(id).await? {
                orders.push(po);
            }
        }

        Ok(orders)
    }

    /// Update multiple purchase orders in a batch (async, non-atomic with partial success)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(Uuid, UpdatePurchaseOrder)>,
    ) -> Result<BatchResult<PurchaseOrder>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(po) => result.record_success(po),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple purchase orders in a batch atomically (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(Uuid, UpdatePurchaseOrder)>,
    ) -> Result<Vec<PurchaseOrder>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut updated_ids = Vec::with_capacity(updates.len());
        let now = Utc::now();

        for (id, input) in updates {
            // Get existing purchase order with lock
            let existing = sqlx::query_as::<_, PurchaseOrderRow>(
                "SELECT * FROM purchase_orders WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

            sqlx::query(
                "UPDATE purchase_orders SET expected_date = $1, ship_to_address = $2, ship_to_city = $3,
                 ship_to_state = $4, ship_to_postal_code = $5, ship_to_country = $6, payment_terms = $7,
                 tax_amount = $8, shipping_cost = $9, discount_amount = $10, notes = $11, supplier_notes = $12,
                 supplier_reference = $13, updated_at = $14 WHERE id = $15",
            )
            .bind(input.expected_date.or(existing.expected_date))
            .bind(input.ship_to_address.or(existing.ship_to_address))
            .bind(input.ship_to_city.or(existing.ship_to_city))
            .bind(input.ship_to_state.or(existing.ship_to_state))
            .bind(input.ship_to_postal_code.or(existing.ship_to_postal_code))
            .bind(input.ship_to_country.or(existing.ship_to_country))
            .bind(
                input
                    .payment_terms
                    .map(|pt| pt.to_string())
                    .unwrap_or(existing.payment_terms),
            )
            .bind(input.tax_amount.unwrap_or(existing.tax_amount))
            .bind(input.shipping_cost.unwrap_or(existing.shipping_cost))
            .bind(input.discount_amount.unwrap_or(existing.discount_amount))
            .bind(input.notes.or(existing.notes))
            .bind(input.supplier_notes.or(existing.supplier_notes))
            .bind(input.supplier_reference.or(existing.supplier_reference))
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Recalculate totals within transaction
            let subtotal: Option<Decimal> = sqlx::query_scalar(
                "SELECT COALESCE(SUM(line_total), 0) FROM purchase_order_items WHERE purchase_order_id = $1",
            )
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let subtotal = subtotal.unwrap_or_default();

            let (tax_amount, shipping_cost, discount_amount): (Decimal, Decimal, Decimal) =
                sqlx::query_as(
                    "SELECT tax_amount, shipping_cost, discount_amount FROM purchase_orders WHERE id = $1",
                )
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            let total = subtotal + tax_amount + shipping_cost - discount_amount;

            sqlx::query(
                "UPDATE purchase_orders SET subtotal = $1, total = $2, updated_at = $3 WHERE id = $4",
            )
            .bind(subtotal)
            .bind(total)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            updated_ids.push(id);
        }

        tx.commit().await.map_err(map_db_error)?;

        // Fetch all updated purchase orders
        let mut orders = Vec::with_capacity(updated_ids.len());
        for id in updated_ids {
            if let Some(po) = self.get_async(id).await? {
                orders.push(po);
            }
        }

        Ok(orders)
    }

    /// Delete multiple purchase orders in a batch (async, non-atomic with partial success)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple purchase orders in a batch atomically (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Verify all are drafts before deleting
        let non_draft_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM purchase_orders WHERE id = ANY($1) AND status != 'draft'",
        )
        .bind(&ids)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if non_draft_count.0 > 0 {
            return Err(CommerceError::ValidationError(
                "Can only delete draft purchase orders".to_string(),
            ));
        }

        // Delete order items first (foreign key constraint)
        sqlx::query("DELETE FROM purchase_order_items WHERE purchase_order_id = ANY($1)")
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Delete purchase orders
        sqlx::query("DELETE FROM purchase_orders WHERE id = ANY($1)")
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple purchase orders by IDs (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<PurchaseOrder>> {
        validate_batch_size(&ids)?;

        let rows = sqlx::query_as::<_, PurchaseOrderRow>(
            "SELECT * FROM purchase_orders WHERE id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let batch_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.load_items_batch_async(&batch_ids).await?;
        let mut orders = Vec::with_capacity(rows.len());
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            orders.push(Self::row_to_po(row, items)?);
        }

        Ok(orders)
    }
}

impl PurchaseOrderRepository for PgPurchaseOrderRepository {
    fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier> {
        super::block_on(self.create_supplier_async(input))
    }

    fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>> {
        super::block_on(self.get_supplier_async(id))
    }

    fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>> {
        super::block_on(self.get_supplier_by_code_async(code))
    }

    fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        super::block_on(self.update_supplier_async(id, input))
    }

    fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        super::block_on(self.list_suppliers_async(filter))
    }

    fn delete_supplier(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_supplier_async(id))
    }

    fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PurchaseOrderId) -> Result<Option<PurchaseOrder>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        super::block_on(self.get_by_number_async(po_number))
    }

    fn update(&self, id: PurchaseOrderId, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        super::block_on(self.list_async(filter))
    }

    fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        super::block_on(self.for_supplier_async(supplier_id))
    }

    fn delete(&self, id: PurchaseOrderId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn submit_for_approval(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        super::block_on(self.submit_for_approval_async(id.into_uuid()))
    }

    fn approve(&self, id: PurchaseOrderId, approved_by: &str) -> Result<PurchaseOrder> {
        super::block_on(self.approve_async(id.into_uuid(), approved_by))
    }

    fn send(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        super::block_on(self.send_async(id.into_uuid()))
    }

    fn acknowledge(
        &self,
        id: PurchaseOrderId,
        supplier_reference: Option<&str>,
    ) -> Result<PurchaseOrder> {
        super::block_on(self.acknowledge_async(id.into_uuid(), supplier_reference))
    }

    fn hold(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        super::block_on(self.hold_async(id.into_uuid()))
    }

    fn cancel(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        super::block_on(self.cancel_async(id.into_uuid()))
    }

    fn receive(
        &self,
        id: PurchaseOrderId,
        items: ReceivePurchaseOrderItems,
    ) -> Result<PurchaseOrder> {
        super::block_on(self.receive_async(id.into_uuid(), items))
    }

    fn complete(&self, id: PurchaseOrderId) -> Result<PurchaseOrder> {
        super::block_on(self.complete_async(id.into_uuid()))
    }

    fn add_item(
        &self,
        po_id: PurchaseOrderId,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        super::block_on(self.add_item_async(po_id.into_uuid(), item))
    }

    fn update_item(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        super::block_on(self.update_item_async(item_id, item))
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        super::block_on(self.remove_item_async(item_id))
    }

    fn get_items(&self, po_id: PurchaseOrderId) -> Result<Vec<PurchaseOrderItem>> {
        super::block_on(self.get_items_async(po_id.into_uuid()))
    }

    fn count(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64> {
        super::block_on(self.count_suppliers_async(filter))
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<BatchResult<PurchaseOrder>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<Vec<PurchaseOrder>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(PurchaseOrderId, UpdatePurchaseOrder)>,
    ) -> Result<BatchResult<PurchaseOrder>> {
        let raw_updates: Vec<(Uuid, UpdatePurchaseOrder)> =
            updates.into_iter().map(|(id, input)| (id.into_uuid(), input)).collect();
        super::block_on(self.update_batch_async(raw_updates))
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(PurchaseOrderId, UpdatePurchaseOrder)>,
    ) -> Result<Vec<PurchaseOrder>> {
        let raw_updates: Vec<(Uuid, UpdatePurchaseOrder)> =
            updates.into_iter().map(|(id, input)| (id.into_uuid(), input)).collect();
        super::block_on(self.update_batch_atomic_async(raw_updates))
    }

    fn delete_batch(&self, ids: Vec<PurchaseOrderId>) -> Result<BatchResult<PurchaseOrderId>> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        let result = super::block_on(self.delete_batch_async(raw_ids))?;
        Ok(BatchResult {
            succeeded: result.succeeded.into_iter().map(PurchaseOrderId::from_uuid).collect(),
            failed: result.failed,
            total_attempted: result.total_attempted,
            success_count: result.success_count,
            failure_count: result.failure_count,
        })
    }

    fn delete_batch_atomic(&self, ids: Vec<PurchaseOrderId>) -> Result<()> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        super::block_on(self.delete_batch_atomic_async(raw_ids))
    }

    fn get_batch(&self, ids: Vec<PurchaseOrderId>) -> Result<Vec<PurchaseOrder>> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        super::block_on(self.get_batch_async(raw_ids))
    }
}
