//! PostgreSQL implementation of cart/checkout repository

use super::map_db_error;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{postgres::PgPool, FromRow, Row};
use stateset_core::{
    AddCartItem, Cart, CartAddress, CartFilter, CartItem, CartPaymentStatus, CartRepository,
    CartStatus, CheckoutResult, CommerceError, CreateCart, FulfillmentType, Result,
    SetCartPayment, SetCartShipping, ShippingRate, UpdateCart, UpdateCartItem,
};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct CartRow {
    id: Uuid,
    cart_number: String,
    customer_id: Option<Uuid>,
    status: String,
    currency: String,
    subtotal: Decimal,
    tax_amount: Decimal,
    shipping_amount: Decimal,
    discount_amount: Decimal,
    grand_total: Decimal,
    customer_email: Option<String>,
    customer_phone: Option<String>,
    customer_name: Option<String>,
    shipping_address: Option<serde_json::Value>,
    billing_address: Option<serde_json::Value>,
    billing_same_as_shipping: bool,
    fulfillment_type: Option<String>,
    shipping_method: Option<String>,
    shipping_carrier: Option<String>,
    estimated_delivery: Option<DateTime<Utc>>,
    payment_method: Option<String>,
    payment_token: Option<String>,
    payment_status: String,
    coupon_code: Option<String>,
    discount_description: Option<String>,
    order_id: Option<Uuid>,
    order_number: Option<String>,
    notes: Option<String>,
    metadata: Option<serde_json::Value>,
    inventory_reserved: bool,
    reservation_expires_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CartRow {
    fn into_cart(self, items: Vec<CartItem>) -> Cart {
        Cart {
            id: self.id,
            cart_number: self.cart_number,
            customer_id: self.customer_id,
            status: parse_cart_status(&self.status),
            currency: self.currency,
            items,
            subtotal: self.subtotal,
            tax_amount: self.tax_amount,
            shipping_amount: self.shipping_amount,
            discount_amount: self.discount_amount,
            grand_total: self.grand_total,
            customer_email: self.customer_email,
            customer_phone: self.customer_phone,
            customer_name: self.customer_name,
            shipping_address: self
                .shipping_address
                .and_then(|v| serde_json::from_value(v).ok()),
            billing_address: self
                .billing_address
                .and_then(|v| serde_json::from_value(v).ok()),
            billing_same_as_shipping: self.billing_same_as_shipping,
            fulfillment_type: self.fulfillment_type.map(|s| parse_fulfillment_type(&s)),
            shipping_method: self.shipping_method,
            shipping_carrier: self.shipping_carrier,
            estimated_delivery: self.estimated_delivery,
            payment_method: self.payment_method,
            payment_token: self.payment_token,
            payment_status: parse_payment_status(&self.payment_status),
            coupon_code: self.coupon_code,
            discount_description: self.discount_description,
            order_id: self.order_id,
            order_number: self.order_number,
            notes: self.notes,
            metadata: self.metadata,
            inventory_reserved: self.inventory_reserved,
            reservation_expires_at: self.reservation_expires_at,
            expires_at: self.expires_at,
            completed_at: self.completed_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct CartItemRow {
    id: Uuid,
    cart_id: Uuid,
    product_id: Option<Uuid>,
    variant_id: Option<Uuid>,
    sku: String,
    name: String,
    description: Option<String>,
    image_url: Option<String>,
    quantity: i32,
    unit_price: Decimal,
    original_price: Option<Decimal>,
    discount_amount: Decimal,
    tax_amount: Decimal,
    total: Decimal,
    weight: Option<Decimal>,
    requires_shipping: bool,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CartItemRow> for CartItem {
    fn from(row: CartItemRow) -> Self {
        CartItem {
            id: row.id,
            cart_id: row.cart_id,
            product_id: row.product_id,
            variant_id: row.variant_id,
            sku: row.sku,
            name: row.name,
            description: row.description,
            image_url: row.image_url,
            quantity: row.quantity,
            unit_price: row.unit_price,
            original_price: row.original_price,
            discount_amount: row.discount_amount,
            tax_amount: row.tax_amount,
            total: row.total,
            weight: row.weight,
            requires_shipping: row.requires_shipping,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// PostgreSQL cart repository
pub struct PgCartRepository {
    pool: PgPool,
}

impl PgCartRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn generate_cart_number() -> String {
        let timestamp = Utc::now().timestamp();
        let random: u32 = (Uuid::new_v4().as_u128() % 10000) as u32;
        format!("CART-{}-{:04}", timestamp, random)
    }

    async fn get_cart_items_async(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        let rows: Vec<CartItemRow> =
            sqlx::query_as("SELECT * FROM cart_items WHERE cart_id = $1 ORDER BY created_at")
                .bind(cart_id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn get_cart_with_items(&self, id: Uuid) -> Result<Option<Cart>> {
        let row: Option<CartRow> = sqlx::query_as("SELECT * FROM carts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(cart_row) => {
                let items = self.get_cart_items_async(id).await?;
                Ok(Some(cart_row.into_cart(items)))
            }
            None => Ok(None),
        }
    }

    async fn update_cart_totals_async(&self, cart_id: Uuid) -> Result<()> {
        // Calculate subtotal from items
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM cart_items WHERE cart_id = $1",
        )
        .bind(cart_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Get current tax, shipping, and discount
        let row: (Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT tax_amount, shipping_amount, discount_amount FROM carts WHERE id = $1",
        )
        .bind(cart_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let (tax_amount, shipping_amount, discount_amount) = row;
        let grand_total = subtotal + tax_amount + shipping_amount - discount_amount;

        sqlx::query(
            "UPDATE carts SET subtotal = $1, grand_total = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(subtotal)
        .bind(grand_total)
        .bind(Utc::now())
        .bind(cart_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    // Async implementations
    pub async fn create_async(&self, input: CreateCart) -> Result<Cart> {
        let id = Uuid::new_v4();
        let cart_number = Self::generate_cart_number();
        let now = Utc::now();
        let currency = input.currency.clone().unwrap_or_else(|| "USD".to_string());
        let expires_at = input
            .expires_in_minutes
            .map(|mins| now + Duration::minutes(mins));

        let shipping_address_json = input
            .shipping_address
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or_default());
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or_default());
        let metadata_json = input.metadata.clone();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            r#"INSERT INTO carts (
                id, cart_number, customer_id, status, currency,
                subtotal, tax_amount, shipping_amount, discount_amount, grand_total,
                customer_email, customer_name, shipping_address, billing_address,
                notes, metadata, expires_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
        )
        .bind(id)
        .bind(&cart_number)
        .bind(input.customer_id)
        .bind("active")
        .bind(&currency)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(&input.customer_email)
        .bind(&input.customer_name)
        .bind(&shipping_address_json)
        .bind(&billing_address_json)
        .bind(&input.notes)
        .bind(&metadata_json)
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        // Add initial items if provided
        let mut items = vec![];
        if let Some(input_items) = &input.items {
            for item_input in input_items {
                let item = self
                    .add_item_internal(&mut tx, id, item_input.clone())
                    .await?;
                items.push(item);
            }
        }

        // Update totals
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM cart_items WHERE cart_id = $1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        sqlx::query("UPDATE carts SET subtotal = $1, grand_total = $2 WHERE id = $3")
            .bind(subtotal)
            .bind(subtotal)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    async fn add_item_internal(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart_id: Uuid,
        item: AddCartItem,
    ) -> Result<CartItem> {
        let item_id = Uuid::new_v4();
        let now = Utc::now();
        let requires_shipping = item.requires_shipping.unwrap_or(true);
        let total = CartItem::calculate_total(item.quantity, item.unit_price, Decimal::ZERO, Decimal::ZERO);
        let metadata_json = item.metadata.clone();

        sqlx::query(
            r#"INSERT INTO cart_items (
                id, cart_id, product_id, variant_id, sku, name, description,
                image_url, quantity, unit_price, original_price, discount_amount,
                tax_amount, total, weight, requires_shipping, metadata,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
        )
        .bind(item_id)
        .bind(cart_id)
        .bind(item.product_id)
        .bind(item.variant_id)
        .bind(&item.sku)
        .bind(&item.name)
        .bind(&item.description)
        .bind(&item.image_url)
        .bind(item.quantity)
        .bind(item.unit_price)
        .bind(item.original_price)
        .bind(Decimal::ZERO)
        .bind(Decimal::ZERO)
        .bind(total)
        .bind(item.weight)
        .bind(requires_shipping)
        .bind(&metadata_json)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await
        .map_err(map_db_error)?;

        Ok(CartItem {
            id: item_id,
            cart_id,
            product_id: item.product_id,
            variant_id: item.variant_id,
            sku: item.sku,
            name: item.name,
            description: item.description,
            image_url: item.image_url,
            quantity: item.quantity,
            unit_price: item.unit_price,
            original_price: item.original_price,
            discount_amount: Decimal::ZERO,
            tax_amount: Decimal::ZERO,
            total,
            weight: item.weight,
            requires_shipping,
            metadata: item.metadata,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_async(&self, id: Uuid) -> Result<Option<Cart>> {
        self.get_cart_with_items(id).await
    }

    pub async fn get_by_number_async(&self, cart_number: &str) -> Result<Option<Cart>> {
        let row: Option<CartRow> = sqlx::query_as("SELECT * FROM carts WHERE cart_number = $1")
            .bind(cart_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(cart_row) => {
                let items = self.get_cart_items_async(cart_row.id).await?;
                Ok(Some(cart_row.into_cart(items)))
            }
            None => Ok(None),
        }
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
        let now = Utc::now();

        sqlx::query(
            r#"UPDATE carts SET
                customer_id = COALESCE($1, customer_id),
                customer_email = COALESCE($2, customer_email),
                customer_phone = COALESCE($3, customer_phone),
                customer_name = COALESCE($4, customer_name),
                shipping_address = COALESCE($5, shipping_address),
                billing_address = COALESCE($6, billing_address),
                billing_same_as_shipping = COALESCE($7, billing_same_as_shipping),
                fulfillment_type = COALESCE($8, fulfillment_type),
                shipping_method = COALESCE($9, shipping_method),
                shipping_carrier = COALESCE($10, shipping_carrier),
                coupon_code = COALESCE($11, coupon_code),
                notes = COALESCE($12, notes),
                metadata = COALESCE($13, metadata),
                updated_at = $14
            WHERE id = $15"#,
        )
        .bind(input.customer_id)
        .bind(&input.customer_email)
        .bind(&input.customer_phone)
        .bind(&input.customer_name)
        .bind(input.shipping_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default()))
        .bind(input.billing_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default()))
        .bind(input.billing_same_as_shipping)
        .bind(input.fulfillment_type.map(|f| f.to_string()))
        .bind(&input.shipping_method)
        .bind(&input.shipping_carrier)
        .bind(&input.coupon_code)
        .bind(&input.notes)
        .bind(&input.metadata)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn list_async(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        let mut sql = "SELECT * FROM carts WHERE 1=1".to_string();
        let mut param_count = 0;

        if filter.customer_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND customer_id = ${}", param_count));
        }
        if filter.customer_email.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND customer_email = ${}", param_count));
        }
        if filter.status.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND status = ${}", param_count));
        }
        if let Some(has_items) = filter.has_items {
            if has_items {
                sql.push_str(" AND id IN (SELECT DISTINCT cart_id FROM cart_items)");
            } else {
                sql.push_str(" AND id NOT IN (SELECT DISTINCT cart_id FROM cart_items)");
            }
        }
        if let Some(true) = filter.is_abandoned {
            sql.push_str(" AND status = 'abandoned'");
        }
        if filter.created_after.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND created_at >= ${}", param_count));
        }
        if filter.created_before.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND created_at <= ${}", param_count));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            param_count += 1;
            sql.push_str(&format!(" LIMIT ${}", param_count));
        }
        if let Some(offset) = filter.offset {
            param_count += 1;
            sql.push_str(&format!(" OFFSET ${}", param_count));
        }

        let mut query = sqlx::query_as::<_, CartRow>(&sql);

        if let Some(customer_id) = filter.customer_id {
            query = query.bind(customer_id);
        }
        if let Some(email) = filter.customer_email {
            query = query.bind(email);
        }
        if let Some(status) = filter.status {
            query = query.bind(status.to_string());
        }
        if let Some(from) = filter.created_after {
            query = query.bind(from);
        }
        if let Some(to) = filter.created_before {
            query = query.bind(to);
        }
        if let Some(limit) = filter.limit {
            query = query.bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows: Vec<CartRow> = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut carts = Vec::new();
        for row in rows {
            let items = self.get_cart_items_async(row.id).await?;
            carts.push(row.into_cart(items));
        }

        Ok(carts)
    }

    pub async fn for_customer_async(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        self.list_async(CartFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .await
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM carts WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn add_item_async(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let result = self.add_item_internal(&mut tx, cart_id, item).await?;

        // Update cart totals
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM cart_items WHERE cart_id = $1",
        )
        .bind(cart_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let (tax, shipping, discount): (Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT tax_amount, shipping_amount, discount_amount FROM carts WHERE id = $1",
        )
        .bind(cart_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let grand_total = subtotal + tax + shipping - discount;

        sqlx::query(
            "UPDATE carts SET subtotal = $1, grand_total = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(subtotal)
        .bind(grand_total)
        .bind(Utc::now())
        .bind(cart_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(result)
    }

    pub async fn update_item_async(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // Get cart_id for this item
        let cart_id: Uuid = sqlx::query_scalar("SELECT cart_id FROM cart_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;

        // Update item fields
        if let Some(qty) = input.quantity {
            sqlx::query("UPDATE cart_items SET quantity = $1, updated_at = $2 WHERE id = $3")
                .bind(qty)
                .bind(now)
                .bind(item_id)
                .execute(&mut *tx)
                .await
                .map_err(map_db_error)?;
        }
        if let Some(price) = input.unit_price {
            sqlx::query("UPDATE cart_items SET unit_price = $1, updated_at = $2 WHERE id = $3")
                .bind(price)
                .bind(now)
                .bind(item_id)
                .execute(&mut *tx)
                .await
                .map_err(map_db_error)?;
        }
        if let Some(meta) = &input.metadata {
            sqlx::query("UPDATE cart_items SET metadata = $1, updated_at = $2 WHERE id = $3")
                .bind(meta)
                .bind(now)
                .bind(item_id)
                .execute(&mut *tx)
                .await
                .map_err(map_db_error)?;
        }

        // Recalculate item total
        let (qty, unit_price, discount, tax): (i32, Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT quantity, unit_price, discount_amount, tax_amount FROM cart_items WHERE id = $1",
        )
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let total = CartItem::calculate_total(qty, unit_price, discount, tax);

        sqlx::query("UPDATE cart_items SET total = $1 WHERE id = $2")
            .bind(total)
            .bind(item_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        // Update cart totals
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM cart_items WHERE cart_id = $1",
        )
        .bind(cart_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let (cart_tax, shipping, discount_cart): (Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT tax_amount, shipping_amount, discount_amount FROM carts WHERE id = $1",
        )
        .bind(cart_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let grand_total = subtotal + cart_tax + shipping - discount_cart;

        sqlx::query(
            "UPDATE carts SET subtotal = $1, grand_total = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(subtotal)
        .bind(grand_total)
        .bind(now)
        .bind(cart_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        // Fetch updated item
        let item: CartItemRow = sqlx::query_as("SELECT * FROM cart_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(item.into())
    }

    pub async fn remove_item_async(&self, item_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Get cart_id before deleting
        let cart_id: Uuid = sqlx::query_scalar("SELECT cart_id FROM cart_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM cart_items WHERE id = $1")
            .bind(item_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        // Update cart totals
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM cart_items WHERE cart_id = $1",
        )
        .bind(cart_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let (tax, shipping, discount): (Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT tax_amount, shipping_amount, discount_amount FROM carts WHERE id = $1",
        )
        .bind(cart_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let grand_total = subtotal + tax + shipping - discount;

        sqlx::query(
            "UPDATE carts SET subtotal = $1, grand_total = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(subtotal)
        .bind(grand_total)
        .bind(Utc::now())
        .bind(cart_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_items_async(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        self.get_cart_items_async(cart_id).await
    }

    pub async fn clear_items_async(&self, cart_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
            .bind(cart_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE carts SET subtotal = 0, grand_total = 0, updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(cart_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn set_shipping_address_async(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        let address_json = serde_json::to_value(&address).unwrap_or_default();

        sqlx::query("UPDATE carts SET shipping_address = $1, updated_at = $2 WHERE id = $3")
            .bind(&address_json)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn set_billing_address_async(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        let address_json = serde_json::to_value(&address).unwrap_or_default();

        sqlx::query(
            "UPDATE carts SET billing_address = $1, billing_same_as_shipping = false, updated_at = $2 WHERE id = $3",
        )
        .bind(&address_json)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn set_shipping_async(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
        let address_json = serde_json::to_value(&shipping.shipping_address).unwrap_or_default();
        let shipping_amount = shipping.shipping_amount.unwrap_or_default();

        sqlx::query(
            r#"UPDATE carts SET
                shipping_address = $1, shipping_method = $2, shipping_carrier = $3,
                shipping_amount = $4, updated_at = $5
            WHERE id = $6"#,
        )
        .bind(&address_json)
        .bind(&shipping.shipping_method)
        .bind(&shipping.shipping_carrier)
        .bind(shipping_amount)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_async(id).await
    }

    pub async fn get_shipping_rates_async(&self, _id: Uuid) -> Result<Vec<ShippingRate>> {
        // Default rates - would integrate with shipping providers in real implementation
        Ok(vec![
            ShippingRate {
                id: "standard".to_string(),
                carrier: "USPS".to_string(),
                service: "Ground".to_string(),
                description: Some("Standard shipping (5-7 business days)".to_string()),
                price: Decimal::new(599, 2),
                currency: "USD".to_string(),
                estimated_days: Some(7),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "express".to_string(),
                carrier: "UPS".to_string(),
                service: "Express".to_string(),
                description: Some("Express shipping (2-3 business days)".to_string()),
                price: Decimal::new(1499, 2),
                currency: "USD".to_string(),
                estimated_days: Some(3),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "overnight".to_string(),
                carrier: "FedEx".to_string(),
                service: "Overnight".to_string(),
                description: Some("Next business day delivery".to_string()),
                price: Decimal::new(2999, 2),
                currency: "USD".to_string(),
                estimated_days: Some(1),
                estimated_delivery: None,
            },
        ])
    }

    pub async fn set_payment_async(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        let billing_json = payment
            .billing_address
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or_default());

        if let Some(billing) = billing_json {
            sqlx::query(
                r#"UPDATE carts SET
                    payment_method = $1, payment_token = $2, payment_status = 'method_selected',
                    billing_address = $3, updated_at = $4
                WHERE id = $5"#,
            )
            .bind(&payment.payment_method)
            .bind(&payment.payment_token)
            .bind(&billing)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        } else {
            sqlx::query(
                r#"UPDATE carts SET
                    payment_method = $1, payment_token = $2, payment_status = 'method_selected',
                    updated_at = $3
                WHERE id = $4"#,
            )
            .bind(&payment.payment_method)
            .bind(&payment.payment_token)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        }

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn apply_discount_async(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        sqlx::query("UPDATE carts SET coupon_code = $1, updated_at = $2 WHERE id = $3")
            .bind(coupon_code)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn remove_discount_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query(
            r#"UPDATE carts SET
                coupon_code = NULL, discount_amount = 0, discount_description = NULL,
                updated_at = $1
            WHERE id = $2"#,
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.recalculate_async(id).await
    }

    pub async fn mark_ready_for_payment_async(&self, id: Uuid) -> Result<Cart> {
        let cart = self
            .get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        if !cart.is_ready_for_checkout() {
            return Err(CommerceError::ValidationError(
                "Cart is not ready for checkout".to_string(),
            ));
        }

        sqlx::query("UPDATE carts SET status = 'ready_for_payment', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn begin_checkout_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'payment_pending', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn complete_async(&self, id: Uuid) -> Result<CheckoutResult> {
        let cart = self
            .get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)?;

        let now = Utc::now();
        let order_id = Uuid::new_v4();
        let order_number = format!(
            "ORD-{}-{:04}",
            now.timestamp(),
            (Uuid::new_v4().as_u128() % 10000) as u32
        );

        sqlx::query(
            r#"UPDATE carts SET
                status = 'completed', order_id = $1, order_number = $2,
                payment_status = 'captured', completed_at = $3, updated_at = $4
            WHERE id = $5"#,
        )
        .bind(order_id)
        .bind(&order_number)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CheckoutResult {
            cart_id: id,
            order_id,
            order_number,
            payment_id: None,
            total_charged: cart.grand_total,
            currency: cart.currency,
        })
    }

    pub async fn cancel_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn abandon_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'abandoned', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn expire_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'expired', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn reserve_inventory_async(&self, id: Uuid) -> Result<Cart> {
        let reservation_expires = Utc::now() + Duration::minutes(15);

        sqlx::query(
            "UPDATE carts SET inventory_reserved = true, reservation_expires_at = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(reservation_expires)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn release_inventory_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query(
            "UPDATE carts SET inventory_reserved = false, reservation_expires_at = NULL, updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn recalculate_async(&self, id: Uuid) -> Result<Cart> {
        self.update_cart_totals_async(id).await?;
        self.get_cart_with_items(id)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    pub async fn set_tax_async(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
        sqlx::query("UPDATE carts SET tax_amount = $1, updated_at = $2 WHERE id = $3")
            .bind(tax_amount)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.recalculate_async(id).await
    }

    pub async fn get_abandoned_async(&self) -> Result<Vec<Cart>> {
        self.list_async(CartFilter {
            status: Some(CartStatus::Abandoned),
            ..Default::default()
        })
        .await
    }

    pub async fn get_expired_async(&self) -> Result<Vec<Cart>> {
        let now = Utc::now();

        // Mark expired carts
        sqlx::query(
            "UPDATE carts SET status = 'expired' WHERE status = 'active' AND expires_at IS NOT NULL AND expires_at < $1",
        )
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.list_async(CartFilter {
            status: Some(CartStatus::Expired),
            ..Default::default()
        })
        .await
    }

    pub async fn count_async(&self, filter: CartFilter) -> Result<u64> {
        let mut sql = "SELECT COUNT(*) FROM carts WHERE 1=1".to_string();
        let mut param_count = 0;

        if filter.customer_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND customer_id = ${}", param_count));
        }
        if filter.status.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND status = ${}", param_count));
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

impl CartRepository for PgCartRepository {
    fn create(&self, input: CreateCart) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Cart>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(cart_number))
    }

    fn update(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        tokio::runtime::Handle::current().block_on(self.for_customer_async(customer_id))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_async(id))
    }

    fn add_item(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        tokio::runtime::Handle::current().block_on(self.add_item_async(cart_id, item))
    }

    fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        tokio::runtime::Handle::current().block_on(self.update_item_async(item_id, input))
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.remove_item_async(item_id))
    }

    fn get_items(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        tokio::runtime::Handle::current().block_on(self.get_items_async(cart_id))
    }

    fn clear_items(&self, cart_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.clear_items_async(cart_id))
    }

    fn set_shipping_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.set_shipping_address_async(id, address))
    }

    fn set_billing_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.set_billing_address_async(id, address))
    }

    fn set_shipping(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.set_shipping_async(id, shipping))
    }

    fn get_shipping_rates(&self, id: Uuid) -> Result<Vec<ShippingRate>> {
        tokio::runtime::Handle::current().block_on(self.get_shipping_rates_async(id))
    }

    fn set_payment(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.set_payment_async(id, payment))
    }

    fn apply_discount(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.apply_discount_async(id, coupon_code))
    }

    fn remove_discount(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.remove_discount_async(id))
    }

    fn mark_ready_for_payment(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.mark_ready_for_payment_async(id))
    }

    fn begin_checkout(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.begin_checkout_async(id))
    }

    fn complete(&self, id: Uuid) -> Result<CheckoutResult> {
        tokio::runtime::Handle::current().block_on(self.complete_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.cancel_async(id))
    }

    fn abandon(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.abandon_async(id))
    }

    fn expire(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.expire_async(id))
    }

    fn reserve_inventory(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.reserve_inventory_async(id))
    }

    fn release_inventory(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.release_inventory_async(id))
    }

    fn recalculate(&self, id: Uuid) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.recalculate_async(id))
    }

    fn set_tax(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
        tokio::runtime::Handle::current().block_on(self.set_tax_async(id, tax_amount))
    }

    fn get_abandoned(&self) -> Result<Vec<Cart>> {
        tokio::runtime::Handle::current().block_on(self.get_abandoned_async())
    }

    fn get_expired(&self) -> Result<Vec<Cart>> {
        tokio::runtime::Handle::current().block_on(self.get_expired_async())
    }

    fn count(&self, filter: CartFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}

fn parse_cart_status(s: &str) -> CartStatus {
    match s {
        "active" => CartStatus::Active,
        "ready_for_payment" => CartStatus::ReadyForPayment,
        "payment_pending" => CartStatus::PaymentPending,
        "completed" => CartStatus::Completed,
        "abandoned" => CartStatus::Abandoned,
        "cancelled" => CartStatus::Cancelled,
        "expired" => CartStatus::Expired,
        _ => CartStatus::Active,
    }
}

fn parse_payment_status(s: &str) -> CartPaymentStatus {
    match s {
        "none" => CartPaymentStatus::None,
        "method_selected" => CartPaymentStatus::MethodSelected,
        "authorized" => CartPaymentStatus::Authorized,
        "captured" => CartPaymentStatus::Captured,
        "failed" => CartPaymentStatus::Failed,
        "refunded" => CartPaymentStatus::Refunded,
        _ => CartPaymentStatus::None,
    }
}

fn parse_fulfillment_type(s: &str) -> FulfillmentType {
    match s {
        "shipping" => FulfillmentType::Shipping,
        "pickup" => FulfillmentType::Pickup,
        "digital" => FulfillmentType::Digital,
        _ => FulfillmentType::Shipping,
    }
}
