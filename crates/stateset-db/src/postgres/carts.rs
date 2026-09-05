//! PostgreSQL implementation of cart/checkout repository

use super::{PgOrderRepository, PgPromotionRepository, map_db_error};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, postgres::PgPool};
use stateset_core::{
    AddCartItem, BatchResult, Cart, CartAddress, CartFilter, CartId, CartItem, CartPaymentStatus,
    CartRepository, CartStatus, CartX402Payment, CheckoutResult, CommerceError, CreateCart,
    CreateOrder, CreateOrderItem, CurrencyCode, CustomerId, FulfillmentType, OrderStatus,
    PaymentStatus, Result, SetCartPayment, SetCartShipping, SetCartX402Payment, ShippingRate,
    UpdateCart, UpdateCartItem, X402Asset, X402AwaitingSettlementData, X402CheckoutResult,
    X402IntentCreatedData, X402IntentStatus, X402Network, X402PaymentRequiredData,
    validate_batch_size, validate_email, validate_money_scale, validate_price,
};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct CartRow {
    id: Uuid,
    cart_number: String,
    customer_id: Option<Uuid>,
    status: String,
    currency: CurrencyCode,
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
    x402_payer_address: Option<String>,
    x402_network: Option<String>,
    x402_asset: Option<String>,
    x402_intent_id: Option<Uuid>,
    x402_status: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CartRow {
    fn into_cart(self, items: Vec<CartItem>) -> Result<Cart> {
        let Self {
            id,
            cart_number,
            customer_id,
            status,
            currency,
            subtotal,
            tax_amount,
            shipping_amount,
            discount_amount,
            grand_total,
            customer_email,
            customer_phone,
            customer_name,
            shipping_address,
            billing_address,
            billing_same_as_shipping,
            fulfillment_type,
            shipping_method,
            shipping_carrier,
            estimated_delivery,
            payment_method,
            payment_token,
            payment_status,
            coupon_code,
            discount_description,
            order_id,
            order_number,
            notes,
            metadata,
            inventory_reserved,
            reservation_expires_at,
            x402_payer_address,
            x402_network,
            x402_asset,
            x402_intent_id,
            x402_status,
            expires_at,
            completed_at,
            created_at,
            updated_at,
        } = self;

        let status: CartStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid cart.status '{}': {}", status, e))
        })?;
        let payment_status: CartPaymentStatus = payment_status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cart.payment_status '{}': {}",
                payment_status, e
            ))
        })?;
        let fulfillment_type = match fulfillment_type {
            Some(value) => Some(value.parse::<FulfillmentType>().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid cart.fulfillment_type '{}': {}",
                    value, e
                ))
            })?),
            None => None,
        };
        let shipping_address =
            shipping_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for cart.shipping_address: {}",
                    e
                ))
            })?;
        let billing_address =
            billing_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for cart.billing_address: {}",
                    e
                ))
            })?;
        let x402_payment = match x402_payer_address {
            Some(payer_address) => {
                let network = match x402_network.as_deref() {
                    Some(value) => value.parse::<X402Network>().map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Invalid cart.x402_network '{}': {}",
                            value, e
                        ))
                    })?,
                    None => X402Network::default(),
                };
                let asset = match x402_asset.as_deref() {
                    Some(value) => value.parse::<X402Asset>().map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Invalid cart.x402_asset '{}': {}",
                            value, e
                        ))
                    })?,
                    None => X402Asset::default(),
                };
                let status = match x402_status.as_deref() {
                    Some(value) => value.parse::<X402IntentStatus>().map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Invalid cart.x402_status '{}': {}",
                            value, e
                        ))
                    })?,
                    None => X402IntentStatus::default(),
                };
                Some(CartX402Payment {
                    intent_id: x402_intent_id,
                    payer_address,
                    network,
                    asset,
                    status,
                })
            }
            None => None,
        };

        Ok(Cart {
            id: id.into(),
            cart_number,
            customer_id: customer_id.map(Into::into),
            status,
            currency,
            items,
            subtotal,
            tax_amount,
            shipping_amount,
            discount_amount,
            grand_total,
            customer_email,
            customer_phone,
            customer_name,
            shipping_address,
            billing_address,
            billing_same_as_shipping,
            fulfillment_type,
            shipping_method,
            shipping_carrier,
            estimated_delivery,
            payment_method,
            payment_token,
            payment_status,
            coupon_code,
            discount_description,
            order_id: order_id.map(Into::into),
            order_number,
            notes,
            metadata,
            inventory_reserved,
            reservation_expires_at,
            x402_payment,
            expires_at,
            completed_at,
            created_at,
            updated_at,
        })
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
        Self {
            id: row.id,
            cart_id: row.cart_id.into(),
            product_id: row.product_id.map(Into::into),
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
#[derive(Debug, Clone)]
pub struct PgCartRepository {
    pool: PgPool,
}

impl PgCartRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The customer a guest checkout mints its order for, resolved through
    /// the customers repository's own get-or-create
    /// ([`crate::postgres::customers::get_or_create_customer_with_conn_pg`])
    /// on the checkout's transaction.
    ///
    /// It used to open-code the lookup and the INSERT here. That INSERT never
    /// populated `email_key`, so a customer created by guest checkout was
    /// unreachable through `get_by_email` (which resolves through the key),
    /// and the raw-column lookup and `ON CONFLICT (email)` made two guests
    /// differing only in e-mail case two different customers. Delegating keeps
    /// guest checkout on exactly the same normalised identity as every other
    /// way a customer is created.
    async fn resolve_customer_id_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart: &Cart,
    ) -> Result<Uuid> {
        if let Some(id) = cart.customer_id {
            return Ok(id.into_uuid());
        }

        let email = cart.customer_email.as_deref().ok_or_else(|| {
            CommerceError::ValidationError("Customer ID or email required".to_string())
        })?;

        let (first_name, last_name) = cart
            .customer_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| {
                let mut parts = name.split_whitespace();
                let first = parts.next().unwrap_or("Guest").to_string();
                let rest = parts.collect::<Vec<_>>().join(" ");
                let last = if rest.is_empty() { "Customer".to_string() } else { rest };
                (first, last)
            })
            .unwrap_or_else(|| ("Guest".to_string(), "Customer".to_string()));

        let (customer, _created) = super::customers::get_or_create_customer_with_conn_pg(
            tx.as_mut(),
            &stateset_core::CreateCustomer {
                email: email.to_string(),
                first_name,
                last_name,
                phone: cart.customer_phone.clone(),
                accepts_marketing: None,
                tags: None,
                metadata: None,
            },
        )
        .await?;
        Ok(customer.id.into_uuid())
    }

    fn order_items_from_cart(cart: &Cart) -> Vec<CreateOrderItem> {
        cart.items
            .iter()
            .map(|item| CreateOrderItem {
                product_id: item.product_id.unwrap_or_else(|| Uuid::new_v4().into()),
                variant_id: item.variant_id,
                sku: item.sku.clone(),
                name: item.name.clone(),
                quantity: item.quantity,
                unit_price: item.unit_price,
                discount: Some(item.discount_amount),
                tax_amount: Some(item.tax_amount),
            })
            .collect()
    }

    fn billing_address_for_cart(cart: &Cart) -> Option<stateset_core::Address> {
        if cart.billing_same_as_shipping {
            cart.billing_address.clone().or_else(|| cart.shipping_address.clone()).map(Into::into)
        } else {
            cart.billing_address.clone().map(Into::into)
        }
    }

    fn generate_cart_number() -> String {
        let timestamp_ms = Utc::now().timestamp_millis();
        let random_suffix = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF_FFFF_FFFF) as u64;
        format!("CART-{timestamp_ms}-{random_suffix:016x}")
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

    async fn get_cart_items_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<CartItem>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<CartItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows: Vec<CartItemRow> =
            sqlx::query_as("SELECT * FROM cart_items WHERE cart_id = ANY($1) ORDER BY created_at")
                .bind(ids.to_vec())
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;
        for row in rows {
            let parent = row.cart_id;
            map.entry(parent).or_default().push(row.into());
        }
        Ok(map)
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
                Ok(Some(cart_row.into_cart(items)?))
            }
            None => Ok(None),
        }
    }

    async fn update_cart_totals_async(&self, cart_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        self.update_cart_totals_in_tx(&mut tx, cart_id).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Lock the cart row for the rest of `tx` so concurrent item mutations on
    /// the same cart serialize instead of each computing a subtotal that
    /// misses the other's line (lost update). `NO KEY UPDATE`, like checkout,
    /// so rows referencing `carts(id)` can still be inserted.
    async fn lock_cart_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart_id: Uuid,
    ) -> Result<CartRow> {
        sqlx::query_as("SELECT * FROM carts WHERE id = $1 FOR NO KEY UPDATE")
            .bind(cart_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)
    }

    /// Recompute the cart's subtotal, discount and grand total from its
    /// current lines, holding the cart row lock. Shared by every item
    /// mutation and by `recalculate`, so all of them agree.
    async fn update_cart_totals_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart_id: Uuid,
    ) -> Result<()> {
        let row = Self::lock_cart_in_tx(tx, cart_id).await?;
        let (previous_subtotal, previous_tax) = (row.subtotal, row.tax_amount);

        // Calculate subtotal from pre-tax line amounts to avoid double-counting tax.
        let subtotal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM((quantity * unit_price) - discount_amount), 0) FROM cart_items WHERE cart_id = $1",
        )
        .bind(cart_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Round the subtotal to the currency minor unit explicitly (rather than
        // relying on the DECIMAL(12,2) column to coerce), so the rounding
        // strategy matches the SQLite backend exactly.
        let subtotal = subtotal.round_dp(2);

        let item_rows: Vec<CartItemRow> =
            sqlx::query_as("SELECT * FROM cart_items WHERE cart_id = $1 ORDER BY created_at")
                .bind(cart_id)
                .fetch_all(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let mut cart = row.into_cart(item_rows.into_iter().map(Into::into).collect())?;
        cart.subtotal = subtotal;
        // Tax follows the lines it was computed on (see `rescale_tax`).
        cart.tax_amount = rescale_tax(previous_tax, previous_subtotal, subtotal);

        // The discount is derived from the cart as it is NOW, never a frozen
        // snapshot: a coupon is re-validated and re-priced against the new
        // contents, and any discount is capped at what the cart can cover.
        let derived = self.derive_discount(tx.as_mut(), &cart).await?;
        let grand_total = derived.grand_total(&cart);

        sqlx::query(
            "UPDATE carts SET subtotal = $1, discount_amount = $2, discount_description = $3,
             grand_total = $4, tax_amount = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(subtotal)
        .bind(derived.amount)
        .bind(&derived.description)
        .bind(grand_total)
        .bind(cart.tax_amount)
        .bind(Utc::now())
        .bind(cart_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Price the discount `cart` should carry right now.
    ///
    /// With a coupon on the cart, the coupon and its promotion are re-validated
    /// against the current contents (window, status, usage limits, conditions
    /// such as a minimum subtotal) and the discount re-priced from them. A
    /// coupon that no longer qualifies stays on the cart — so it revives on its
    /// own when the cart qualifies again — but contributes no discount, and the
    /// cart's `discount_description` says why; checkout refuses the cart while
    /// it is in that state (see [`DerivedDiscount::coupon_error`]).
    ///
    /// Every promotion type is priced by the core evaluator
    /// ([`stateset_core::Promotion::calculate_discount`]) against the current
    /// lines, so a bundle, tier or Buy-X-Get-Y coupon loses its discount the
    /// moment the cart stops qualifying. Every discount, coupon or manual, is
    /// capped at `subtotal + tax + shipping` so the cart never hands an order
    /// a discount larger than the order can absorb. Mirrors the SQLite twin.
    ///
    /// Runs on `conn` — the caller's cart-mutation / checkout transaction —
    /// so coupon and usage reads are consistent with the locked cart row and
    /// no second pooled connection is taken.
    async fn derive_discount(
        &self,
        conn: &mut sqlx::PgConnection,
        cart: &Cart,
    ) -> Result<DerivedDiscount> {
        let Some(code) = cart.coupon_code.as_deref() else {
            return Ok(DerivedDiscount::capped(cart, cart.discount_amount, None));
        };
        let promo_repo = PgPromotionRepository::new(self.pool.clone());
        match promo_repo.validate_coupon_for_cart_in_tx(conn, cart, code, Utc::now()).await {
            Ok((_coupon, promotion)) => {
                let amount = coupon_discount_amount(&promotion, cart);
                Ok(DerivedDiscount::capped(cart, amount, Some(promotion.name)))
            }
            Err(CommerceError::ValidationError(reason)) => Ok(DerivedDiscount {
                amount: Decimal::ZERO,
                description: Some(format!("Coupon {code} not applied: {reason}")),
                coupon_error: Some(reason),
            }),
            Err(other) => Err(other),
        }
    }

    async fn finalize_x402_checkout_async(&self, cart_id: Uuid) -> Result<X402CheckoutResult> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let result = self.complete_checkout_in_tx(&mut tx, cart_id, true, true).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(X402CheckoutResult::Completed(result))
    }

    // Async implementations
    pub async fn create_async(&self, input: CreateCart) -> Result<Cart> {
        let id = Uuid::new_v4();
        let cart_number = Self::generate_cart_number();
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        let expires_at = input.expires_in_minutes.map(|mins| now + Duration::minutes(mins));

        let shipping_address_json =
            input.shipping_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default());
        let billing_address_json =
            input.billing_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default());
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
        .bind(currency)
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Add initial items if provided, then price the cart through the
        // same totals path every later mutation uses (parity with SQLite).
        if let Some(input_items) = &input.items {
            for item_input in input_items {
                validate_add_item_money(currency, item_input)?;
                // Same line guard as `add_item_async`: `create` used to reach
                // `add_item_internal` directly, so a withdrawn catalogue SKU
                // (and a client-chosen price) entered the cart unchecked.
                guard_cart_line_with_conn_pg(
                    tx.as_mut(),
                    item_input.variant_id,
                    &item_input.sku,
                    item_input.unit_price,
                )
                .await?;
                self.add_item_internal(&mut tx, id, item_input.clone()).await?;
            }
            self.update_cart_totals_in_tx(&mut tx, id).await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
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
        let total =
            CartItem::calculate_total(item.quantity, item.unit_price, Decimal::ZERO, Decimal::ZERO);
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(CartItem {
            id: item_id,
            cart_id: cart_id.into(),
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
                Ok(Some(cart_row.into_cart(items)?))
            }
            None => Ok(None),
        }
    }

    pub async fn update_async(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
        let now = Utc::now();
        if let Some(discount) = input.discount_amount {
            if discount < Decimal::ZERO {
                return Err(CommerceError::ValidationError(format!(
                    "Cart discount must not be negative, got {discount}"
                )));
            }
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::lock_cart_in_tx(&mut tx, id).await?;
        let rows = sqlx::query(
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
                discount_amount = COALESCE($12, discount_amount),
                discount_description = COALESCE($13, discount_description),
                notes = COALESCE($14, notes),
                metadata = COALESCE($15, metadata),
                updated_at = $16
            WHERE id = $17"#,
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
        .bind(input.discount_amount)
        .bind(&input.discount_description)
        .bind(&input.notes)
        .bind(&input.metadata)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(CommerceError::NotFound);
        }
        // A discount / coupon written here must land in grand_total too:
        // never store a discount the totals do not reflect.
        self.update_cart_totals_in_tx(&mut tx, id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
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

        param_count += 1;
        sql.push_str(&format!(" LIMIT ${}", param_count));
        if filter.offset.is_some() {
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
        query = query.bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows: Vec<CartRow> = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.get_cart_items_batch_async(&ids).await?;
        let mut carts = Vec::new();
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            carts.push(row.into_cart(items)?);
        }

        Ok(carts)
    }

    pub async fn for_customer_async(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        self.list_async(CartFilter { customer_id: Some(customer_id.into()), ..Default::default() })
            .await
    }

    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM carts WHERE id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn add_item_async(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        // Serialize with other mutations of this cart before touching its lines.
        let row = Self::lock_cart_in_tx(&mut tx, cart_id).await?;
        validate_add_item_money(row.currency, &item)?;
        guard_cart_line_with_conn_pg(tx.as_mut(), item.variant_id, &item.sku, item.unit_price)
            .await?;
        let result = self.add_item_internal(&mut tx, cart_id, item).await?;
        self.update_cart_totals_in_tx(&mut tx, cart_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        Ok(result)
    }

    pub async fn update_item_async(
        &self,
        item_id: Uuid,
        input: UpdateCartItem,
    ) -> Result<CartItem> {
        validate_update_cart_item(&input)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();

        // Get cart_id for this item
        let cart_id: Uuid = sqlx::query_scalar("SELECT cart_id FROM cart_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        // Serialize with other mutations of this cart before touching its lines.
        let row = Self::lock_cart_in_tx(&mut tx, cart_id).await?;
        validate_update_cart_item_money(row.currency, &input)?;

        // Re-run the line guard against the CURRENT catalogue: a line added
        // while its SKU was sellable must not be grown, or repriced, after the
        // SKU was withdrawn. Shrinking a line (or removing it) stays allowed
        // so a cart holding a withdrawn SKU is never stuck.
        let (line_sku, line_variant_id, line_quantity): (String, Option<Uuid>, i32) =
            sqlx::query_as("SELECT sku, variant_id, quantity FROM cart_items WHERE id = $1")
                .bind(item_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if input.quantity.is_some_and(|qty| qty > line_quantity) {
            super::products::variant_is_purchasable_with_conn_pg(tx.as_mut(), &line_sku)
                .await?
                .ensure_sellable(&line_sku)?;
        }
        if let Some(unit_price) = input.unit_price {
            guard_cart_line_with_conn_pg(tx.as_mut(), line_variant_id, &line_sku, unit_price)
                .await?;
        }

        // Update item fields
        if let Some(qty) = input.quantity {
            sqlx::query("UPDATE cart_items SET quantity = $1, updated_at = $2 WHERE id = $3")
                .bind(qty)
                .bind(now)
                .bind(item_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }
        if let Some(price) = input.unit_price {
            sqlx::query("UPDATE cart_items SET unit_price = $1, updated_at = $2 WHERE id = $3")
                .bind(price)
                .bind(now)
                .bind(item_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }
        if let Some(discount) = input.discount_amount {
            sqlx::query(
                "UPDATE cart_items SET discount_amount = $1, updated_at = $2 WHERE id = $3",
            )
            .bind(discount)
            .bind(now)
            .bind(item_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        if let Some(meta) = &input.metadata {
            sqlx::query("UPDATE cart_items SET metadata = $1, updated_at = $2 WHERE id = $3")
                .bind(meta)
                .bind(now)
                .bind(item_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }

        // Recalculate item total
        let (qty, unit_price, discount, tax): (i32, Decimal, Decimal, Decimal) = sqlx::query_as(
            "SELECT quantity, unit_price, discount_amount, tax_amount FROM cart_items WHERE id = $1",
        )
        .bind(item_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let total = CartItem::calculate_total(qty, unit_price, discount, tax);

        sqlx::query("UPDATE cart_items SET total = $1 WHERE id = $2")
            .bind(total)
            .bind(item_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        self.update_cart_totals_in_tx(&mut tx, cart_id).await?;

        // Fetch updated item
        let item: CartItemRow = sqlx::query_as("SELECT * FROM cart_items WHERE id = $1")
            .bind(item_id)
            .fetch_one(tx.as_mut())
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
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Serialize with other mutations of this cart before touching its lines.
        Self::lock_cart_in_tx(&mut tx, cart_id).await?;

        sqlx::query("DELETE FROM cart_items WHERE id = $1")
            .bind(item_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        self.update_cart_totals_in_tx(&mut tx, cart_id).await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    pub async fn get_item_async(&self, item_id: Uuid) -> Result<Option<CartItem>> {
        let row: Option<CartItemRow> = sqlx::query_as("SELECT * FROM cart_items WHERE id = $1")
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(row.map(Into::into))
    }

    pub async fn get_items_async(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        self.get_cart_items_async(cart_id).await
    }

    pub async fn clear_items_async(&self, cart_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        Self::lock_cart_in_tx(&mut tx, cart_id).await?;

        sqlx::query("DELETE FROM cart_items WHERE cart_id = $1")
            .bind(cart_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        self.update_cart_totals_in_tx(&mut tx, cart_id).await?;

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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Set the cart's shipping address, method and charge.
    ///
    /// The charge is money written straight onto the cart, so it runs the same
    /// guard as [`Self::set_tax_async`] ([`Cart::ensure_money_settable`]) and
    /// the write plus the repricing happen in ONE transaction holding the
    /// cart's row lock. Before this, the UPDATE and `recalculate` were
    /// separate statements outside any transaction, so a concurrent line
    /// mutation could reprice between them.
    pub async fn set_shipping_async(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
        let address_json = serde_json::to_value(&shipping.shipping_address).unwrap_or_default();
        let shipping_amount = shipping.shipping_amount.unwrap_or_default();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = Self::lock_cart_in_tx(&mut tx, id).await?;
        row.into_cart(Vec::new())?.ensure_money_settable("shipping", shipping_amount)?;
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        // Reprice inside the same transaction, under the same lock.
        self.update_cart_totals_in_tx(&mut tx, id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
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
                currency: CurrencyCode::USD,
                estimated_days: Some(7),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "express".to_string(),
                carrier: "UPS".to_string(),
                service: "Express".to_string(),
                description: Some("Express shipping (2-3 business days)".to_string()),
                price: Decimal::new(1499, 2),
                currency: CurrencyCode::USD,
                estimated_days: Some(3),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "overnight".to_string(),
                carrier: "FedEx".to_string(),
                service: "Overnight".to_string(),
                description: Some("Next business day delivery".to_string()),
                price: Decimal::new(2999, 2),
                currency: CurrencyCode::USD,
                estimated_days: Some(1),
                estimated_delivery: None,
            },
        ])
    }

    pub async fn set_payment_async(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        let billing_json =
            payment.billing_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default());

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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn set_x402_payment_async(
        &self,
        id: Uuid,
        payment: SetCartX402Payment,
    ) -> Result<Cart> {
        sqlx::query(
            r#"UPDATE carts SET
                x402_payer_address = $1, x402_network = $2, x402_asset = $3,
                x402_status = $4, payment_method = 'x402', updated_at = $5
            WHERE id = $6"#,
        )
        .bind(payment.payer_address)
        .bind(payment.network.to_string())
        .bind(payment.asset.to_string().to_lowercase())
        .bind(X402IntentStatus::Created.to_string())
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn complete_with_x402_async(
        &self,
        id: Uuid,
        payee_address: &str,
    ) -> Result<X402CheckoutResult> {
        use rust_decimal::prelude::ToPrimitive;

        let cart = self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)?;

        if cart.status == CartStatus::Completed {
            if let (Some(order_id), Some(order_number)) = (cart.order_id, cart.order_number.clone())
            {
                return Ok(X402CheckoutResult::Completed(CheckoutResult {
                    cart_id: id.into(),
                    order_id,
                    order_number,
                    payment_id: None,
                    total_charged: cart.grand_total,
                    currency: cart.currency,
                }));
            }
        }

        // A cancelled/abandoned/expired cart must never be minted into an order.
        if !cart.is_checkoutable_status() {
            return Err(CommerceError::Conflict(format!(
                "Cart cannot be checked out in status: {}",
                cart.status
            )));
        }

        if !cart.is_ready_for_checkout() {
            return Err(CommerceError::ValidationError(
                "Cart is not ready for checkout - ensure items, customer info, and shipping address are set".to_string(),
            ));
        }

        let x402_payment = cart.x402_payment.as_ref().ok_or_else(|| {
            CommerceError::ValidationError(
                "x402 payment not configured. Call set_x402_payment first".to_string(),
            )
        })?;

        let decimals = x402_payment.asset.decimals();
        let multiplier = Decimal::from(10u64.pow(decimals as u32));
        let amount_scaled = cart.grand_total * multiplier;
        let amount = amount_scaled.to_u64().unwrap_or(0);
        let amount_display = format!("{:.6} {}", cart.grand_total, x402_payment.asset);

        if let Some(intent_id) = x402_payment.intent_id {
            type IntentStatusRow = (String, Option<String>, Option<i64>, Option<Uuid>);
            let row: Option<IntentStatusRow> = sqlx::query_as(
                "SELECT status, signing_hash, sequence_number, batch_id FROM x402_payment_intents WHERE id = $1",
            )
            .bind(intent_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

            if let Some((status_str, signing_hash, seq_num, batch_id)) = row {
                let status: X402IntentStatus = status_str.parse().unwrap_or_default();
                match status {
                    X402IntentStatus::Settled => {
                        return self.finalize_x402_checkout_async(id).await;
                    }
                    X402IntentStatus::Signed
                    | X402IntentStatus::Sequenced
                    | X402IntentStatus::Batched => {
                        return Ok(X402CheckoutResult::AwaitingSettlement(
                            X402AwaitingSettlementData {
                                cart_id: id.into(),
                                intent_id,
                                status,
                                sequence_number: seq_num.map(|n| n as u64),
                                batch_id,
                            },
                        ));
                    }
                    X402IntentStatus::Created => {
                        return Ok(X402CheckoutResult::IntentCreated(X402IntentCreatedData {
                            cart_id: id.into(),
                            intent_id,
                            signing_hash: signing_hash.unwrap_or_default(),
                            amount,
                            amount_display,
                            asset: x402_payment.asset,
                            network: x402_payment.network,
                            payee_address: payee_address.to_string(),
                            valid_until: 0,
                            nonce: 0,
                        }));
                    }
                    X402IntentStatus::Expired
                    | X402IntentStatus::Failed
                    | X402IntentStatus::Cancelled => {}
                    _ => {}
                }
            }
        }

        let chain_id = x402_payment.network.chain_id();
        Ok(X402CheckoutResult::PaymentRequired(X402PaymentRequiredData {
            cart_id: id.into(),
            payee_address: payee_address.to_string(),
            amount,
            amount_display,
            asset: x402_payment.asset,
            network: x402_payment.network,
            chain_id,
            valid_seconds: 3600,
        }))
    }

    pub async fn apply_discount_async(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        // Get the cart first to calculate the discount off its subtotal.
        let mut cart = self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)?;

        // Resolve the coupon and its promotion, and refuse anything that is
        // not redeemable right now (inactive/expired/exhausted coupon,
        // draft/paused/expired/exhausted promotion, unmet conditions such as
        // a minimum subtotal, per-customer limit reached). Mirrors the SQLite
        // backend; the checks live in `stateset-core` + the promotions repo.
        let promo_repo = PgPromotionRepository::new(self.pool.clone());
        let (_coupon, promotion) =
            promo_repo.validate_coupon_for_cart_async(&cart, coupon_code, Utc::now()).await?;

        cart.coupon_code = Some(coupon_code.to_uppercase());
        let discount_amount = coupon_discount_amount(&promotion, &cart);

        let discount_description = promotion.name;

        // Update the cart with the coupon + computed discount.
        sqlx::query(
            "UPDATE carts SET coupon_code = $1, discount_amount = $2, discount_description = $3,
             updated_at = $4 WHERE id = $5",
        )
        // Persist the canonical (uppercased) code: checkout consumes the coupon
        // by this value and codes are stored uppercased.
        .bind(coupon_code.to_uppercase())
        .bind(discount_amount)
        .bind(&discount_description)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Recalculate totals (grand_total reflects the new discount) and return.
        self.recalculate_async(id).await
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
        let cart = self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)?;

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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn begin_checkout_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'payment_pending', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn complete_async(&self, id: Uuid) -> Result<CheckoutResult> {
        self.complete_checkout_async(id, false).await
    }

    /// See [`CartRepository::complete_settled_externally`]: explicit opt-in to
    /// mint a `Paid` order with no engine-side payment record.
    pub async fn complete_settled_externally_async(&self, id: Uuid) -> Result<CheckoutResult> {
        self.complete_checkout_async(id, true).await
    }

    async fn complete_checkout_async(&self, id: Uuid, mark_paid: bool) -> Result<CheckoutResult> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let result = self.complete_checkout_in_tx(&mut tx, id, false, mark_paid).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(result)
    }

    /// Validate checkout exactly as apply does and return the re-derived money
    /// that the order would commit, without mutating the cart.
    pub(crate) async fn checkout_money_with_policy_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        stock_policy: stateset_core::StockPolicy,
    ) -> Result<(Decimal, CurrencyCode)> {
        let row: CartRow = sqlx::query_as("SELECT * FROM carts WHERE id = $1 FOR NO KEY UPDATE")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        let item_rows: Vec<CartItemRow> =
            sqlx::query_as("SELECT * FROM cart_items WHERE cart_id = $1 ORDER BY created_at")
                .bind(id)
                .fetch_all(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let cart = row.into_cart(item_rows.into_iter().map(Into::into).collect())?;
        if cart.status == CartStatus::Completed {
            return Err(CommerceError::Conflict(
                "cart was already checked out under a different economic command".into(),
            ));
        }
        if !cart.is_checkoutable_status() {
            return Err(CommerceError::Conflict(format!(
                "Cart cannot be checked out in status: {}",
                cart.status
            )));
        }
        if let Some(expired_at) = cart.expires_at.filter(|_| cart.is_expired()) {
            return Err(CommerceError::ValidationError(format!(
                "Cart expired at {}",
                expired_at.to_rfc3339()
            )));
        }
        if !cart.is_ready_for_checkout() {
            return Err(CommerceError::ValidationError("Cart is not ready for checkout".into()));
        }
        // Same coupon re-validation / discount derivation as
        // `complete_checkout_in_tx`, so a Preview never succeeds where Apply
        // would refuse (and reports the same error).
        let mut cart = cart;
        let derived = self.derive_discount(tx.as_mut(), &cart).await?;
        if let Some(reason) = derived.coupon_error {
            return Err(CommerceError::ValidationError(format!(
                "Coupon {} is no longer valid: {reason}",
                cart.coupon_code.as_deref().unwrap_or_default()
            )));
        }
        cart.discount_amount = derived.amount;
        let customer_id = if let Some(id) = cart.customer_id {
            id
        } else {
            let email = cart.customer_email.as_deref().ok_or_else(|| {
                CommerceError::ValidationError("Customer ID or email required".into())
            })?;
            validate_email(email)?;
            CustomerId::new()
        };
        // Apply consumes the cart's coupon inside the checkout transaction;
        // Preview must refuse wherever that consumption would. Read-only, and
        // against the customer Apply would resolve — not the throwaway id
        // above, which exists only to shape the order-validation input.
        let redeemer = preview_customer_id_with_conn_pg(tx.as_mut(), &cart).await?;
        ensure_cart_coupon_consumable_with_conn_pg(tx.as_mut(), &cart, redeemer).await?;
        // Apply also consumes the cart's automatic (no-code) promotions, which
        // carry their own usage limits; Preview must refuse those too.
        PgPromotionRepository::ensure_cart_promotions_consumable_on(tx.as_mut(), &cart, redeemer)
            .await?;
        let checkout_money = (derived.grand_total(&cart), cart.currency);
        let input = CreateOrder {
            customer_id,
            items: Self::order_items_from_cart(&cart),
            currency: Some(cart.currency),
            shipping_address: cart.shipping_address.clone().map(Into::into),
            billing_address: Self::billing_address_for_cart(&cart),
            notes: cart.notes,
            payment_method: cart.payment_method,
            shipping_method: cart.shipping_method,
            tax_amount: Some(cart.tax_amount),
            shipping_amount: Some(cart.shipping_amount),
            discount_amount: Some(cart.discount_amount),
            stock_policy,
        };
        PgOrderRepository::validate_create_order_in_tx(tx, &input).await?;
        Ok(checkout_money)
    }

    /// Complete a checkout inside a caller-owned transaction. This is the
    /// primitive used by the kernel so all business facts and governance facts
    /// either commit together or roll back together.
    pub(crate) async fn complete_checkout_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        x402_settled: bool,
        mark_paid: bool,
    ) -> Result<CheckoutResult> {
        self.complete_checkout_with_policy_in_tx(
            tx,
            id,
            x402_settled,
            mark_paid,
            stateset_core::StockPolicy::default(),
        )
        .await
    }

    pub(crate) async fn complete_checkout_with_policy_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        x402_settled: bool,
        mark_paid: bool,
        stock_policy: stateset_core::StockPolicy,
    ) -> Result<CheckoutResult> {
        // Lock the cart row so only one checkout can run at a time.

        // Use NO KEY UPDATE so a checkout can insert an order referencing `carts(id)` via a
        // foreign key (`orders.cart_id`) without deadlocking.
        let row: Option<CartRow> =
            sqlx::query_as("SELECT * FROM carts WHERE id = $1 FOR NO KEY UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        let row = match row {
            Some(row) => row,
            None => return Err(CommerceError::NotFound),
        };

        let item_rows: Vec<CartItemRow> =
            sqlx::query_as("SELECT * FROM cart_items WHERE cart_id = $1 ORDER BY created_at")
                .bind(id)
                .fetch_all(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        let items: Vec<CartItem> = item_rows.into_iter().map(Into::into).collect();
        let mut cart = row.into_cart(items)?;

        // Idempotent checkout: if already completed, return the existing order reference.
        if cart.status == CartStatus::Completed {
            if let (Some(order_id), Some(order_number)) = (cart.order_id, cart.order_number.clone())
            {
                return Ok(CheckoutResult {
                    cart_id: id.into(),
                    order_id,
                    order_number,
                    payment_id: None,
                    total_charged: cart.grand_total,
                    currency: cart.currency,
                });
            }
        }

        // A cancelled/abandoned/expired cart must never be minted into an order.
        if !cart.is_checkoutable_status() {
            return Err(CommerceError::Conflict(format!(
                "Cart cannot be checked out in status: {}",
                cart.status
            )));
        }

        if let Some(expired_at) = cart.expires_at.filter(|_| cart.is_expired()) {
            return Err(CommerceError::ValidationError(format!(
                "Cart expired at {}",
                expired_at.to_rfc3339()
            )));
        }

        if !cart.is_ready_for_checkout() {
            return Err(CommerceError::ValidationError(
                "Cart is not ready for checkout - ensure items, customer info, and shipping address are set".to_string(),
            ));
        }

        // Re-validate the coupon and re-derive the discount inside the
        // checkout transaction: the minted order's discount is always one the
        // coupon grants RIGHT NOW (not a snapshot from `apply_discount`), a
        // coupon that stopped qualifying since it was applied refuses the
        // checkout, and no discount ever exceeds what the order can absorb.
        let derived = self.derive_discount(tx.as_mut(), &cart).await?;
        if let Some(reason) = derived.coupon_error {
            return Err(CommerceError::ValidationError(format!(
                "Coupon {} is no longer valid: {reason}",
                cart.coupon_code.as_deref().unwrap_or_default()
            )));
        }
        cart.discount_amount = derived.amount;
        cart.grand_total = derived.grand_total(&cart);

        let customer_id = Self::resolve_customer_id_in_tx(tx, &cart).await?;
        let order_items = Self::order_items_from_cart(&cart);

        let shipping_address = cart.shipping_address.clone().map(Into::into);
        let billing_address = Self::billing_address_for_cart(&cart);

        let order_repo = PgOrderRepository::new(self.pool.clone());
        let mut order = order_repo
            .create_from_cart_in_tx(
                tx,
                id,
                CreateOrder {
                    customer_id: customer_id.into(),
                    items: order_items,
                    currency: Some(cart.currency),
                    shipping_address,
                    billing_address,
                    notes: cart.notes.clone(),
                    payment_method: cart.payment_method.clone(),
                    shipping_method: cart.shipping_method.clone(),
                    // Carry the cart's own money onto the order — see the SQLite twin.
                    tax_amount: Some(cart.tax_amount),
                    shipping_amount: Some(cart.shipping_amount),
                    discount_amount: Some(cart.discount_amount),
                    stock_policy,
                },
            )
            .await?;

        // Promote the order inside the same transaction that creates it and
        // completes the cart.
        sqlx::query(
            r#"UPDATE orders SET
                status = $2,
                payment_status = CASE WHEN $3 THEN $4 ELSE payment_status END,
                updated_at = $5, version = version + 1
            WHERE id = $1"#,
        )
        .bind(order.id)
        .bind(OrderStatus::Confirmed.to_string())
        .bind(mark_paid)
        .bind(PaymentStatus::Paid.to_string())
        .bind(Utc::now())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        order.status = OrderStatus::Confirmed;
        if mark_paid {
            order.payment_status = PaymentStatus::Paid;
        }
        order.version += 1;

        // Consume the cart's coupon in the same transaction as the order:
        // usage counters advance under their limits, and a coupon exhausted
        // since it was applied fails the checkout instead of being honoured.
        PgPromotionRepository::consume_cart_coupon_in_tx(
            tx,
            &cart,
            Some(CustomerId::from(customer_id)),
            order.id,
        )
        .await?;
        // Automatic (no-code) promotions are consumed here too: evaluation is
        // read-only, so this is the only place their usage advances.
        PgPromotionRepository::consume_cart_promotions_in_tx(
            tx,
            &cart,
            Some(CustomerId::from(customer_id)),
            order.id,
        )
        .await?;

        let now = Utc::now();
        let cart_update = if x402_settled {
            r#"UPDATE carts SET
                status = 'completed', order_id = $1, order_number = $2,
                payment_status = 'captured', x402_status = 'settled',
                completed_at = $3, updated_at = $4, customer_id = $5,
                discount_amount = $7, grand_total = $8
            WHERE id = $6"#
        } else if mark_paid {
            r#"UPDATE carts SET
                status = 'completed', order_id = $1, order_number = $2,
                payment_status = 'captured', completed_at = $3, updated_at = $4, customer_id = $5,
                discount_amount = $7, grand_total = $8
            WHERE id = $6"#
        } else {
            r#"UPDATE carts SET
                status = 'completed', order_id = $1, order_number = $2,
                completed_at = $3, updated_at = $4, customer_id = $5,
                discount_amount = $7, grand_total = $8
            WHERE id = $6"#
        };
        sqlx::query(cart_update)
            .bind(order.id)
            .bind(&order.order_number)
            .bind(now)
            .bind(now)
            .bind(customer_id)
            .bind(id)
            .bind(cart.discount_amount)
            .bind(cart.grand_total)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Ok(CheckoutResult {
            cart_id: id.into(),
            order_id: order.id,
            order_number: order.order_number,
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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn abandon_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'abandoned', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn expire_async(&self, id: Uuid) -> Result<Cart> {
        sqlx::query("UPDATE carts SET status = 'expired', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
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

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn recalculate_async(&self, id: Uuid) -> Result<Cart> {
        self.update_cart_totals_async(id).await?;
        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Set the cart's tax amount.
    ///
    /// Guarded by [`Cart::ensure_money_settable`] — non-negative, expressible
    /// in the cart's currency, and only while the cart is still active — and
    /// written together with the repricing in ONE transaction holding the
    /// cart's row lock (`FOR NO KEY UPDATE`), so a concurrent `add_item`
    /// cannot reprice between the write and the recalculation. Before this the
    /// amount was unchecked (a negative tax lowered `grand_total`, a completed
    /// cart could still be re-taxed) and the two statements ran unsynchronized.
    pub async fn set_tax_async(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = Self::lock_cart_in_tx(&mut tx, id).await?;
        row.into_cart(Vec::new())?.ensure_money_settable("tax", tax_amount)?;
        sqlx::query("UPDATE carts SET tax_amount = $1, updated_at = $2 WHERE id = $3")
            .bind(tax_amount)
            .bind(Utc::now())
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        // Reprice inside the same transaction, under the same lock.
        self.update_cart_totals_in_tx(&mut tx, id).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_cart_with_items(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_abandoned_async(&self) -> Result<Vec<Cart>> {
        self.list_async(CartFilter { status: Some(CartStatus::Abandoned), ..Default::default() })
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

        self.list_async(CartFilter { status: Some(CartStatus::Expired), ..Default::default() })
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

    // === Batch Operations ===

    /// Create multiple carts in a batch (async, non-atomic - partial success allowed)
    pub async fn create_batch_async(&self, inputs: Vec<CreateCart>) -> Result<BatchResult<Cart>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(cart) => result.record_success(cart),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple carts in a batch atomically (async - all-or-nothing)
    pub async fn create_batch_atomic_async(&self, inputs: Vec<CreateCart>) -> Result<Vec<Cart>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut carts = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let cart_number = Self::generate_cart_number();
            let now = Utc::now();
            let currency = input.currency.unwrap_or(CurrencyCode::USD);
            let expires_at = input.expires_in_minutes.map(|mins| now + Duration::minutes(mins));

            let shipping_address_json = input
                .shipping_address
                .as_ref()
                .map(|a| serde_json::to_value(a).unwrap_or_default());
            let billing_address_json =
                input.billing_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default());
            let metadata_json = input.metadata.clone();

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
            .bind(currency)
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
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Add initial items if provided, through the same money validation
            // and line guard `create_async` runs — this batch path used to
            // insert cart lines unguarded too.
            let mut items = vec![];
            if let Some(input_items) = &input.items {
                for item_input in input_items {
                    validate_add_item_money(currency, item_input)?;
                    guard_cart_line_with_conn_pg(
                        tx.as_mut(),
                        item_input.variant_id,
                        &item_input.sku,
                        item_input.unit_price,
                    )
                    .await?;
                    let item_id = Uuid::new_v4();
                    let requires_shipping = item_input.requires_shipping.unwrap_or(true);
                    let total = CartItem::calculate_total(
                        item_input.quantity,
                        item_input.unit_price,
                        Decimal::ZERO,
                        Decimal::ZERO,
                    );
                    let item_metadata_json = item_input.metadata.clone();

                    sqlx::query(
                        r#"INSERT INTO cart_items (
                            id, cart_id, product_id, variant_id, sku, name, description,
                            image_url, quantity, unit_price, original_price, discount_amount,
                            tax_amount, total, weight, requires_shipping, metadata,
                            created_at, updated_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
                    )
                    .bind(item_id)
                    .bind(id)
                    .bind(item_input.product_id)
                    .bind(item_input.variant_id)
                    .bind(&item_input.sku)
                    .bind(&item_input.name)
                    .bind(&item_input.description)
                    .bind(&item_input.image_url)
                    .bind(item_input.quantity)
                    .bind(item_input.unit_price)
                    .bind(item_input.original_price)
                    .bind(Decimal::ZERO)
                    .bind(Decimal::ZERO)
                    .bind(total)
                    .bind(item_input.weight)
                    .bind(requires_shipping)
                    .bind(&item_metadata_json)
                    .bind(now)
                    .bind(now)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;

                    items.push(CartItem {
                        id: item_id,
                        cart_id: id.into(),
                        product_id: item_input.product_id,
                        variant_id: item_input.variant_id,
                        sku: item_input.sku.clone(),
                        name: item_input.name.clone(),
                        description: item_input.description.clone(),
                        image_url: item_input.image_url.clone(),
                        quantity: item_input.quantity,
                        unit_price: item_input.unit_price,
                        original_price: item_input.original_price,
                        discount_amount: Decimal::ZERO,
                        tax_amount: Decimal::ZERO,
                        total,
                        weight: item_input.weight,
                        requires_shipping,
                        metadata: item_input.metadata.clone(),
                        created_at: now,
                        updated_at: now,
                    });
                }
            }

            // Calculate subtotal
            let subtotal: Decimal = items.iter().map(|i| i.total).sum();

            sqlx::query("UPDATE carts SET subtotal = $1, grand_total = $2 WHERE id = $3")
                .bind(subtotal)
                .bind(subtotal)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            carts.push(Cart {
                id: id.into(),
                cart_number,
                customer_id: input.customer_id,
                status: CartStatus::Active,
                currency,
                items,
                subtotal,
                tax_amount: Decimal::ZERO,
                shipping_amount: Decimal::ZERO,
                discount_amount: Decimal::ZERO,
                grand_total: subtotal,
                customer_email: input.customer_email,
                customer_phone: None,
                customer_name: input.customer_name,
                shipping_address: input.shipping_address,
                billing_address: input.billing_address,
                billing_same_as_shipping: false,
                fulfillment_type: None,
                shipping_method: None,
                shipping_carrier: None,
                estimated_delivery: None,
                payment_method: None,
                payment_token: None,
                payment_status: CartPaymentStatus::None,
                coupon_code: None,
                discount_description: None,
                order_id: None,
                order_number: None,
                notes: input.notes,
                metadata: input.metadata,
                inventory_reserved: false,
                reservation_expires_at: None,
                x402_payment: None,
                expires_at,
                completed_at: None,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(carts)
    }

    /// Update multiple carts in a batch (async, non-atomic - partial success allowed)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(Uuid, UpdateCart)>,
    ) -> Result<BatchResult<Cart>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(cart) => result.record_success(cart),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple carts in a batch atomically (async - all-or-nothing)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(Uuid, UpdateCart)>,
    ) -> Result<Vec<Cart>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut cart_ids = Vec::with_capacity(updates.len());
        let now = Utc::now();

        for (id, input) in updates {
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
            .bind(
                input
                    .shipping_address
                    .as_ref()
                    .map(|a| serde_json::to_value(a).unwrap_or_default()),
            )
            .bind(
                input.billing_address.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default()),
            )
            .bind(input.billing_same_as_shipping)
            .bind(input.fulfillment_type.map(|f| f.to_string()))
            .bind(&input.shipping_method)
            .bind(&input.shipping_carrier)
            .bind(&input.coupon_code)
            .bind(&input.notes)
            .bind(&input.metadata)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            cart_ids.push(id);
        }

        tx.commit().await.map_err(map_db_error)?;

        // Fetch updated carts
        let mut carts = Vec::with_capacity(cart_ids.len());
        for id in cart_ids {
            if let Some(cart) = self.get_cart_with_items(id).await? {
                carts.push(cart);
            }
        }

        Ok(carts)
    }

    /// Delete multiple carts in a batch (async, non-atomic - partial success allowed)
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

    /// Delete multiple carts in a batch atomically (async - all-or-nothing)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete cart items first (foreign key constraint)
        sqlx::query("DELETE FROM cart_items WHERE cart_id = ANY($1)")
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Delete carts
        sqlx::query("DELETE FROM carts WHERE id = ANY($1)")
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple carts by IDs (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Cart>> {
        validate_batch_size(&ids)?;

        let rows = sqlx::query_as::<_, CartRow>("SELECT * FROM carts WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let row_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.get_cart_items_batch_async(&row_ids).await?;
        let mut carts = Vec::with_capacity(rows.len());
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            carts.push(row.into_cart(items)?);
        }

        Ok(carts)
    }
}

impl CartRepository for PgCartRepository {
    fn create(&self, input: CreateCart) -> Result<Cart> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: CartId) -> Result<Option<Cart>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        super::block_on(self.get_by_number_async(cart_number))
    }

    fn update(&self, id: CartId, input: UpdateCart) -> Result<Cart> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        super::block_on(self.list_async(filter))
    }

    fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Cart>> {
        super::block_on(self.for_customer_async(customer_id.into_uuid()))
    }

    fn delete(&self, id: CartId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn add_item(&self, cart_id: CartId, item: AddCartItem) -> Result<CartItem> {
        super::block_on(self.add_item_async(cart_id.into_uuid(), item))
    }

    fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        super::block_on(self.update_item_async(item_id, input))
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        super::block_on(self.remove_item_async(item_id))
    }

    fn get_item(&self, item_id: Uuid) -> Result<Option<CartItem>> {
        super::block_on(self.get_item_async(item_id))
    }

    fn get_items(&self, cart_id: CartId) -> Result<Vec<CartItem>> {
        super::block_on(self.get_items_async(cart_id.into_uuid()))
    }

    fn clear_items(&self, cart_id: CartId) -> Result<()> {
        super::block_on(self.clear_items_async(cart_id.into_uuid()))
    }

    fn set_shipping_address(&self, id: CartId, address: CartAddress) -> Result<Cart> {
        super::block_on(self.set_shipping_address_async(id.into_uuid(), address))
    }

    fn set_billing_address(&self, id: CartId, address: CartAddress) -> Result<Cart> {
        super::block_on(self.set_billing_address_async(id.into_uuid(), address))
    }

    fn set_shipping(&self, id: CartId, shipping: SetCartShipping) -> Result<Cart> {
        super::block_on(self.set_shipping_async(id.into_uuid(), shipping))
    }

    fn get_shipping_rates(&self, id: CartId) -> Result<Vec<ShippingRate>> {
        super::block_on(self.get_shipping_rates_async(id.into_uuid()))
    }

    fn set_payment(&self, id: CartId, payment: SetCartPayment) -> Result<Cart> {
        super::block_on(self.set_payment_async(id.into_uuid(), payment))
    }

    fn set_x402_payment(&self, id: CartId, payment: SetCartX402Payment) -> Result<Cart> {
        super::block_on(self.set_x402_payment_async(id.into_uuid(), payment))
    }

    fn complete_with_x402(&self, id: CartId, payee_address: &str) -> Result<X402CheckoutResult> {
        super::block_on(self.complete_with_x402_async(id.into_uuid(), payee_address))
    }

    fn apply_discount(&self, id: CartId, coupon_code: &str) -> Result<Cart> {
        super::block_on(self.apply_discount_async(id.into_uuid(), coupon_code))
    }

    fn remove_discount(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.remove_discount_async(id.into_uuid()))
    }

    fn mark_ready_for_payment(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.mark_ready_for_payment_async(id.into_uuid()))
    }

    fn begin_checkout(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.begin_checkout_async(id.into_uuid()))
    }

    fn complete(&self, id: CartId) -> Result<CheckoutResult> {
        super::block_on(self.complete_async(id.into_uuid()))
    }

    fn complete_settled_externally(&self, id: CartId) -> Result<CheckoutResult> {
        super::block_on(self.complete_settled_externally_async(id.into_uuid()))
    }

    fn cancel(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.cancel_async(id.into_uuid()))
    }

    fn abandon(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.abandon_async(id.into_uuid()))
    }

    fn expire(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.expire_async(id.into_uuid()))
    }

    fn reserve_inventory(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.reserve_inventory_async(id.into_uuid()))
    }

    fn release_inventory(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.release_inventory_async(id.into_uuid()))
    }

    fn recalculate(&self, id: CartId) -> Result<Cart> {
        super::block_on(self.recalculate_async(id.into_uuid()))
    }

    fn set_tax(&self, id: CartId, tax_amount: Decimal) -> Result<Cart> {
        super::block_on(self.set_tax_async(id.into_uuid(), tax_amount))
    }

    fn get_abandoned(&self) -> Result<Vec<Cart>> {
        super::block_on(self.get_abandoned_async())
    }

    fn get_expired(&self) -> Result<Vec<Cart>> {
        super::block_on(self.get_expired_async())
    }

    fn count(&self, filter: CartFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateCart>) -> Result<BatchResult<Cart>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateCart>) -> Result<Vec<Cart>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(&self, updates: Vec<(CartId, UpdateCart)>) -> Result<BatchResult<Cart>> {
        let raw_updates: Vec<(Uuid, UpdateCart)> =
            updates.into_iter().map(|(id, input)| (id.into_uuid(), input)).collect();
        super::block_on(self.update_batch_async(raw_updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(CartId, UpdateCart)>) -> Result<Vec<Cart>> {
        let raw_updates: Vec<(Uuid, UpdateCart)> =
            updates.into_iter().map(|(id, input)| (id.into_uuid(), input)).collect();
        super::block_on(self.update_batch_atomic_async(raw_updates))
    }

    fn delete_batch(&self, ids: Vec<CartId>) -> Result<BatchResult<CartId>> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        let result = super::block_on(self.delete_batch_async(raw_ids))?;
        Ok(BatchResult {
            succeeded: result.succeeded.into_iter().map(CartId::from_uuid).collect(),
            failed: result.failed,
            total_attempted: result.total_attempted,
            success_count: result.success_count,
            failure_count: result.failure_count,
        })
    }

    fn delete_batch_atomic(&self, ids: Vec<CartId>) -> Result<()> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        super::block_on(self.delete_batch_atomic_async(raw_ids))
    }

    fn get_batch(&self, ids: Vec<CartId>) -> Result<Vec<Cart>> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        super::block_on(self.get_batch_async(raw_ids))
    }
}

/// Discount a coupon-activated promotion grants on `cart` RIGHT NOW, priced
/// by the core evaluator ([`stateset_core::Promotion::calculate_discount`])
/// against the cart's current lines — so bundle, tier, Buy-X-Get-Y and
/// product-scoped promotions are re-derived exactly like percentage / fixed
/// ones — then capped at what the cart can cover and rounded to the cart
/// currency's precision. Mirrors the SQLite twin so the two backends agree to
/// the cent.
pub(crate) fn coupon_discount_amount(promotion: &stateset_core::Promotion, cart: &Cart) -> Decimal {
    let request = stateset_core::ApplyPromotionsRequest::from_cart(
        cart,
        cart.coupon_code.as_deref().unwrap_or_default(),
    );
    let raw = promotion.calculate_discount(&request, Decimal::ZERO);
    cap_discount(cart, raw).round_dp(u32::from(cart.currency.decimal_places()))
}

/// Carry a cart's tax across a change of its lines.
///
/// The cart stores only a tax *amount* (set by `set_tax`, normally from the
/// tax engine), not the rate or jurisdiction it came from, so the storage
/// layer cannot re-run the engine. It keeps the effective rate instead: the
/// stored tax is scaled by `new_subtotal / previous_subtotal`, which is exact
/// for a single-rate cart and a best-effort estimate for mixed-rate lines —
/// the embedded cart accessor re-runs the tax engine after each mutation when
/// it can and overwrites this estimate. Tax that was never set (zero) stays
/// zero, and a cart emptied of lines carries no tax.
pub(crate) fn rescale_tax(
    previous_tax: Decimal,
    previous_subtotal: Decimal,
    new_subtotal: Decimal,
) -> Decimal {
    if previous_tax <= Decimal::ZERO || previous_subtotal <= Decimal::ZERO {
        return previous_tax.max(Decimal::ZERO);
    }
    if new_subtotal == previous_subtotal {
        return previous_tax;
    }
    if new_subtotal <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    (previous_tax * new_subtotal / previous_subtotal).round_dp(2)
}

/// The catalogue's own price for the line a cart mutation names, if the line
/// names a catalogue variant. Postgres twin of the SQLite
/// `catalog_unit_price_with_conn`.
pub(crate) async fn catalog_unit_price_with_conn_pg(
    conn: &mut sqlx::PgConnection,
    variant_id: Option<Uuid>,
    sku: &str,
) -> Result<Option<Decimal>> {
    if let Some(variant_id) = variant_id {
        let price: Option<Decimal> =
            sqlx::query_scalar("SELECT price FROM product_variants WHERE id = $1")
                .bind(variant_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?;
        if price.is_some() {
            return Ok(price);
        }
    }
    if sku.trim().is_empty() {
        return Ok(None);
    }
    sqlx::query_scalar("SELECT price FROM product_variants WHERE sku = $1")
        .bind(sku)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)
}

/// THE guard every path that puts a SKU on a cart line runs, inside that
/// path's own transaction. Postgres twin of the SQLite
/// `guard_cart_line_with_conn`; see it for the rules and why both live in the
/// repositories rather than in the embedded accessor.
pub(crate) async fn guard_cart_line_with_conn_pg(
    conn: &mut sqlx::PgConnection,
    variant_id: Option<Uuid>,
    sku: &str,
    unit_price: Decimal,
) -> Result<()> {
    super::products::variant_is_purchasable_with_conn_pg(&mut *conn, sku)
        .await?
        .ensure_sellable(sku)?;
    if let Some(catalog) = catalog_unit_price_with_conn_pg(&mut *conn, variant_id, sku).await? {
        if catalog != unit_price {
            return Err(CommerceError::ValidationError(format!(
                "unit_price {unit_price} for SKU '{sku}' does not match the catalog price \
                 {catalog}; catalog lines are priced from the catalog"
            )));
        }
    }
    Ok(())
}

/// The customer identity Apply would attribute this cart's redemption to,
/// resolved WITHOUT writing. Postgres twin of the SQLite
/// `preview_customer_id_with_conn`.
pub(crate) async fn preview_customer_id_with_conn_pg(
    conn: &mut sqlx::PgConnection,
    cart: &Cart,
) -> Result<Option<CustomerId>> {
    if let Some(customer_id) = cart.customer_id {
        return Ok(Some(customer_id));
    }
    let Some(email) = cart.customer_email.as_deref() else {
        return Ok(None);
    };
    // Same resolution as the customers repository's `LIVE_CUSTOMER_BY_EMAIL`
    // (normalised key first, legacy raw column second), id only.
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM customers \
         WHERE email_key = $1 OR (status <> 'deleted' AND LOWER(TRIM(email)) = $1) \
         ORDER BY CASE WHEN email_key = $1 THEN 0 ELSE 1 END, created_at, id \
         LIMIT 1",
    )
    .bind(stateset_core::Customer::normalize_email(email))
    .fetch_optional(conn)
    .await
    .map_err(map_db_error)?;
    Ok(id.map(CustomerId::from))
}

/// Read-only twin of the coupon consumption Apply performs
/// (`PgPromotionRepository::consume_cart_coupon_in_tx`). Postgres twin of the
/// SQLite `ensure_cart_coupon_consumable_with_conn`; see it for why Preview
/// needs this and which branches it mirrors.
pub(crate) async fn ensure_cart_coupon_consumable_with_conn_pg(
    conn: &mut sqlx::PgConnection,
    cart: &Cart,
    customer_id: Option<CustomerId>,
) -> Result<()> {
    let Some(code) = cart.coupon_code.as_deref() else {
        return Ok(());
    };
    // Coupon codes are stored uppercased; look up the same way consumption does.
    let coupon: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT id, promotion_id FROM coupon_codes WHERE code = $1")
            .bind(code.to_uppercase())
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?;
    let Some((coupon_id, promotion_id)) = coupon else {
        return Ok(());
    };

    let already_recorded: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM promotion_usage WHERE cart_id = $1 AND coupon_id = $2 LIMIT 1",
    )
    .bind(cart.id.into_uuid())
    .bind(coupon_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?;
    if already_recorded.is_some() {
        return Ok(());
    }

    if let Some(customer_id) = customer_id {
        let checks: [(&str, Uuid, &str, &str); 2] = [
            (
                "SELECT per_customer_limit FROM promotions WHERE id = $1",
                promotion_id,
                "SELECT COUNT(*) FROM promotion_usage WHERE promotion_id = $1 AND customer_id = $2",
                "Per-customer promotion usage limit reached",
            ),
            (
                "SELECT per_customer_limit FROM coupon_codes WHERE id = $1",
                coupon_id,
                "SELECT COUNT(*) FROM promotion_usage WHERE coupon_id = $1 AND customer_id = $2",
                "Per-customer coupon usage limit reached",
            ),
        ];
        for (limit_sql, id, count_sql, message) in checks {
            let limit: Option<i32> = sqlx::query_scalar(limit_sql)
                .bind(id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?
                .flatten();
            let Some(limit) = limit else { continue };
            let used: i64 = sqlx::query_scalar(count_sql)
                .bind(id)
                .bind(customer_id.into_uuid())
                .fetch_one(&mut *conn)
                .await
                .map_err(map_db_error)?;
            if used >= i64::from(limit) {
                return Err(CommerceError::ValidationError(message.to_string()));
            }
        }
    }

    // The total limits consumption advances under, checked the way the guarded
    // The guarded updates check them (`usage_count < limit`).
    let promotion: Option<(Option<i32>, i32)> =
        sqlx::query_as("SELECT total_usage_limit, usage_count FROM promotions WHERE id = $1")
            .bind(promotion_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?;
    match promotion {
        Some((Some(limit), used)) if used >= limit => {
            return Err(CommerceError::ValidationError(
                "Promotion not found or usage limit reached".to_string(),
            ));
        }
        Some(_) => {}
        None => {
            return Err(CommerceError::ValidationError(
                "Promotion not found or usage limit reached".to_string(),
            ));
        }
    }
    let coupon: (Option<i32>, i32) =
        sqlx::query_as("SELECT usage_limit, usage_count FROM coupon_codes WHERE id = $1")
            .bind(coupon_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(map_db_error)?;
    if let (Some(limit), used) = coupon {
        if used >= limit {
            return Err(CommerceError::ValidationError(
                "Coupon not found or usage limit reached".to_string(),
            ));
        }
    }
    Ok(())
}

/// Invariant M1 (`commerce.money.scale_exceeds_currency`) for a line being
/// added: no money input may carry more decimals than the cart currency's
/// minor unit, because the line total and cart subtotal are rounded to it
/// and a sub-minor-unit price would silently lose money.
pub(crate) fn validate_add_item_money(currency: CurrencyCode, item: &AddCartItem) -> Result<()> {
    validate_money_scale(currency, item.unit_price)?;
    if let Some(original) = item.original_price {
        validate_money_scale(currency, original)?;
    }
    Ok(())
}

/// Invariant M1 for a line update (see [`validate_add_item_money`]).
pub(crate) fn validate_update_cart_item_money(
    currency: CurrencyCode,
    input: &UpdateCartItem,
) -> Result<()> {
    if let Some(unit_price) = input.unit_price {
        validate_money_scale(currency, unit_price)?;
    }
    if let Some(discount) = input.discount_amount {
        validate_money_scale(currency, discount)?;
    }
    Ok(())
}

/// Outcome of [`PgCartRepository::derive_discount`].
#[derive(Debug, Clone)]
pub(crate) struct DerivedDiscount {
    /// Discount the cart carries, capped at what the cart can cover.
    pub(crate) amount: Decimal,
    /// `discount_description` to store (`None` keeps the stored one).
    pub(crate) description: Option<String>,
    /// Why the cart's coupon no longer qualifies, if it does not.
    pub(crate) coupon_error: Option<String>,
}

impl DerivedDiscount {
    fn capped(cart: &Cart, amount: Decimal, description: Option<String>) -> Self {
        Self {
            amount: cap_discount(cart, amount),
            description: description.or_else(|| cart.discount_description.clone()),
            coupon_error: None,
        }
    }

    /// `subtotal + tax + shipping - amount`, which the cap keeps non-negative.
    fn grand_total(&self, cart: &Cart) -> Decimal {
        (cart.subtotal + cart.tax_amount + cart.shipping_amount - self.amount)
            .round_dp(2)
            .max(Decimal::ZERO)
    }
}

/// Clamp a discount into `[0, subtotal + tax + shipping]` so the order minted
/// from the cart can never have a negative total.
pub(crate) fn cap_discount(cart: &Cart, amount: Decimal) -> Decimal {
    let coverable = (cart.subtotal + cart.tax_amount + cart.shipping_amount).max(Decimal::ZERO);
    amount.max(Decimal::ZERO).min(coverable)
}

/// Reject a cart-item update that would store a non-positive quantity, a
/// negative price or a negative line discount. Quantity 0 is NOT a silent
/// remove: callers must use `remove_item`. Mirrors the SQLite twin.
pub(crate) fn validate_update_cart_item(input: &UpdateCartItem) -> Result<()> {
    if let Some(quantity) = input.quantity {
        if quantity < 1 {
            return Err(CommerceError::ValidationError(format!(
                "Item quantity must be positive, got {quantity}; remove the item instead"
            )));
        }
    }
    if let Some(unit_price) = input.unit_price {
        validate_price(unit_price)?;
    }
    if let Some(discount) = input.discount_amount {
        if discount < Decimal::ZERO {
            return Err(CommerceError::ValidationError(format!(
                "Item discount must not be negative, got {discount}"
            )));
        }
    }
    Ok(())
}
