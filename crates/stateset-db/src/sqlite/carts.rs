//! SQLite cart repository implementation

use super::parse_helpers::{parse_decimal as parse_decimal_err, parse_uuid};
use super::{
    SqliteOrderRepository, SqlitePromotionRepository, build_in_clause, map_db_error, params_refs,
    parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row, parse_decimal_row,
    parse_enum_row, parse_json_opt_row, parse_uuid_opt_row, parse_uuid_row, uuid_params,
    with_immediate_transaction,
};
use chrono::{Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    AddCartItem, BatchResult, Cart, CartAddress, CartFilter, CartId, CartItem, CartPaymentStatus,
    CartRepository, CartStatus, CartX402Payment, CheckoutResult, CommerceError, CreateCart,
    CreateOrder, CreateOrderItem, CurrencyCode, CustomerId, OrderId, OrderStatus, PaymentStatus,
    ProductId, PromotionType, Result, SetCartPayment, SetCartShipping, SetCartX402Payment,
    ShippingRate, UpdateCart, UpdateCartItem, X402AwaitingSettlementData, X402CheckoutResult,
    X402IntentCreatedData, X402IntentStatus, X402PaymentRequiredData, validate_batch_size,
    validate_currency_code, validate_email, validate_phone, validate_price, validate_required_text,
};
use uuid::Uuid;

/// SQLite implementation of `CartRepository`
#[derive(Debug)]
pub struct SqliteCartRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCartRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_cart_number() -> String {
        let timestamp_ms = Utc::now().timestamp_millis();
        let random_suffix = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF_FFFF_FFFF) as u64;
        format!("CART-{timestamp_ms}-{random_suffix:016x}")
    }

    fn row_to_cart(row: &rusqlite::Row<'_>) -> rusqlite::Result<Cart> {
        let shipping_addr: Option<String> = row.get("shipping_address")?;
        let billing_addr: Option<String> = row.get("billing_address")?;
        let metadata: Option<String> = row.get("metadata")?;

        Ok(Cart {
            id: CartId::from(parse_uuid_row(&row.get::<_, String>("id")?, "cart", "id")?),
            cart_number: row.get("cart_number")?,
            customer_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("customer_id")?,
                "cart",
                "customer_id",
            )?
            .map(CustomerId::from),
            status: parse_enum_row(&row.get::<_, String>("status")?, "cart", "status")?,
            currency: row.get("currency")?,

            items: vec![], // Loaded separately

            subtotal: parse_decimal_row(&row.get::<_, String>("subtotal")?, "cart", "subtotal")?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "cart",
                "tax_amount",
            )?,
            shipping_amount: parse_decimal_row(
                &row.get::<_, String>("shipping_amount")?,
                "cart",
                "shipping_amount",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "cart",
                "discount_amount",
            )?,
            grand_total: parse_decimal_row(
                &row.get::<_, String>("grand_total")?,
                "cart",
                "grand_total",
            )?,

            customer_email: row.get("customer_email")?,
            customer_phone: row.get("customer_phone")?,
            customer_name: row.get("customer_name")?,

            shipping_address: parse_json_opt_row(shipping_addr, "cart", "shipping_address")?,
            billing_address: parse_json_opt_row(billing_addr, "cart", "billing_address")?,
            billing_same_as_shipping: row.get::<_, i32>("billing_same_as_shipping")? == 1,

            fulfillment_type: match row.get::<_, Option<String>>("fulfillment_type")? {
                Some(value) => Some(parse_enum_row(&value, "cart", "fulfillment_type")?),
                None => None,
            },
            shipping_method: row.get("shipping_method")?,
            shipping_carrier: row.get("shipping_carrier")?,
            estimated_delivery: parse_datetime_opt_row(
                row.get::<_, Option<String>>("estimated_delivery")?,
                "cart",
                "estimated_delivery",
            )?,

            payment_method: row.get("payment_method")?,
            payment_token: row.get("payment_token")?,
            payment_status: parse_enum_row(
                &row.get::<_, String>("payment_status")?,
                "cart",
                "payment_status",
            )?,

            coupon_code: row.get("coupon_code")?,
            discount_description: row.get("discount_description")?,

            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "cart",
                "order_id",
            )?
            .map(OrderId::from),
            order_number: row.get("order_number")?,

            notes: row.get("notes")?,
            metadata: parse_json_opt_row(metadata, "cart", "metadata")?,

            inventory_reserved: row.get::<_, i32>("inventory_reserved")? == 1,
            reservation_expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("reservation_expires_at")?,
                "cart",
                "reservation_expires_at",
            )?,

            // x402 payment fields
            x402_payment: {
                let payer: Option<String> = row.get("x402_payer_address")?;
                if let Some(payer_address) = payer {
                    let network_str: Option<String> = row.get("x402_network")?;
                    let asset_str: Option<String> = row.get("x402_asset")?;
                    let intent_id: Option<String> = row.get("x402_intent_id")?;
                    let status_str: Option<String> = row.get("x402_status")?;
                    Some(CartX402Payment {
                        intent_id: parse_uuid_opt_row(intent_id, "cart", "x402_intent_id")?,
                        payer_address,
                        network: match network_str {
                            Some(value) => parse_enum_row(&value, "cart", "x402_network")?,
                            None => Default::default(),
                        },
                        asset: match asset_str {
                            Some(value) => parse_enum_row(&value, "cart", "x402_asset")?,
                            None => Default::default(),
                        },
                        status: match status_str {
                            Some(value) => parse_enum_row(&value, "cart", "x402_status")?,
                            None => Default::default(),
                        },
                    })
                } else {
                    None
                }
            },

            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("expires_at")?,
                "cart",
                "expires_at",
            )?,
            completed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("completed_at")?,
                "cart",
                "completed_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "cart",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "cart",
                "updated_at",
            )?,
        })
    }

    fn load_cart_items_with_conn(
        conn: &rusqlite::Connection,
        cart_id: CartId,
    ) -> Result<Vec<CartItem>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, cart_id, product_id, variant_id, sku, name, description, image_url,
                        quantity, unit_price, original_price, discount_amount, tax_amount, total,
                        weight, requires_shipping, metadata, created_at, updated_at
                 FROM cart_items WHERE cart_id = ?",
            )
            .map_err(map_db_error)?;

        let items = stmt
            .query_map([cart_id.to_string()], Self::row_to_cart_item)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn row_to_cart_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<CartItem> {
        let metadata: Option<String> = row.get("metadata")?;
        Ok(CartItem {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "cart_item", "id")?,
            cart_id: CartId::from(parse_uuid_row(
                &row.get::<_, String>("cart_id")?,
                "cart_item",
                "cart_id",
            )?),
            product_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("product_id")?,
                "cart_item",
                "product_id",
            )?
            .map(ProductId::from),
            variant_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("variant_id")?,
                "cart_item",
                "variant_id",
            )?,
            sku: row.get("sku")?,
            name: row.get("name")?,
            description: row.get("description")?,
            image_url: row.get("image_url")?,
            quantity: row.get("quantity")?,
            unit_price: parse_decimal_row(
                &row.get::<_, String>("unit_price")?,
                "cart_item",
                "unit_price",
            )?,
            original_price: parse_decimal_opt_row(
                row.get::<_, Option<String>>("original_price")?,
                "cart_item",
                "original_price",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "cart_item",
                "discount_amount",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "cart_item",
                "tax_amount",
            )?,
            total: parse_decimal_row(&row.get::<_, String>("total")?, "cart_item", "total")?,
            weight: parse_decimal_opt_row(
                row.get::<_, Option<String>>("weight")?,
                "cart_item",
                "weight",
            )?,
            requires_shipping: row.get::<_, i32>("requires_shipping")? == 1,
            metadata: parse_json_opt_row(metadata, "cart_item", "metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "cart_item",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "cart_item",
                "updated_at",
            )?,
        })
    }

    fn load_cart_items_batch(
        conn: &rusqlite::Connection,
        ids: &[CartId],
    ) -> Result<std::collections::HashMap<CartId, Vec<CartItem>>> {
        let mut map: std::collections::HashMap<CartId, Vec<CartItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let placeholders = build_in_clause(chunk.len());
            let sql = format!(
                "SELECT id, cart_id, product_id, variant_id, sku, name, description, image_url,
                        quantity, unit_price, original_price, discount_amount, tax_amount, total,
                        weight, requires_shipping, metadata, created_at, updated_at
                 FROM cart_items WHERE cart_id IN ({placeholders})"
            );
            let id_strs: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                id_strs.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
            let rows = stmt
                .query_map(param_refs.as_slice(), Self::row_to_cart_item)
                .map_err(map_db_error)?;
            for row in rows {
                let item = row.map_err(map_db_error)?;
                map.entry(item.cart_id).or_default().push(item);
            }
        }
        Ok(map)
    }

    fn load_cart_with_conn(conn: &rusqlite::Connection, id: CartId) -> Result<Option<Cart>> {
        let result =
            conn.query_row("SELECT * FROM carts WHERE id = ?", [id.to_string()], Self::row_to_cart);

        match result {
            Ok(mut cart) => {
                cart.items = Self::load_cart_items_with_conn(conn, id)?;
                Ok(Some(cart))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(map_db_error(error)),
        }
    }

    /// Finalize x402 checkout after payment settlement
    fn finalize_x402_checkout(&self, cart_id: CartId) -> Result<X402CheckoutResult> {
        let result = with_immediate_transaction(&self.pool, |tx| {
            self.complete_checkout_in_tx(tx, cart_id, true, true)
        })?;
        Ok(X402CheckoutResult::Completed(result))
    }

    fn update_cart_totals(&self, conn: &rusqlite::Connection, cart_id: CartId) -> Result<()> {
        // Calculate subtotal from pre-tax line amounts to avoid double-counting tax.
        let mut subtotal = Decimal::ZERO;
        let mut stmt = conn
            .prepare(
                "SELECT quantity, unit_price, discount_amount FROM cart_items WHERE cart_id = ?",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([cart_id.to_string()], |row| {
                Ok((
                    row.get::<_, i32>("quantity")?,
                    row.get::<_, String>("unit_price")?,
                    row.get::<_, String>("discount_amount")?,
                ))
            })
            .map_err(map_db_error)?;

        for row in rows {
            let (quantity, unit_price, discount_amount) = row.map_err(map_db_error)?;
            let line_subtotal = parse_decimal_err(&unit_price, "cart_item", "unit_price")?
                * Decimal::from(quantity)
                - parse_decimal_err(&discount_amount, "cart_item", "discount_amount")?;
            subtotal += line_subtotal;
        }

        // Round the subtotal to the currency minor unit so the stored value is a
        // real money amount (a sub-cent subtotal like 9.999 is not chargeable),
        // matching the Postgres DECIMAL(12,2) columns and the order pipeline.
        let subtotal = subtotal.round_dp(2);

        // Get current tax and shipping
        let (tax, shipping, discount): (String, String, String) = conn
            .query_row(
                "SELECT tax_amount, shipping_amount, discount_amount FROM carts WHERE id = ?",
                [cart_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;

        // Calculate grand total, clamped at zero: a discount larger than
        // subtotal + tax + shipping must not produce a negative total (which
        // would charge the buyer a negative amount at checkout).
        let tax_dec = parse_decimal_err(&tax, "cart", "tax_amount")?;
        let shipping_dec = parse_decimal_err(&shipping, "cart", "shipping_amount")?;
        let discount_dec = parse_decimal_err(&discount, "cart", "discount_amount")?;
        let grand_total =
            (subtotal + tax_dec + shipping_dec - discount_dec).round_dp(2).max(Decimal::ZERO);

        conn.execute(
            "UPDATE carts SET subtotal = ?, grand_total = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                subtotal.to_string(),
                grand_total.to_string(),
                Utc::now().to_rfc3339(),
                cart_id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }
}

impl CartRepository for SqliteCartRepository {
    fn create(&self, input: CreateCart) -> Result<Cart> {
        // Validate currency if provided
        if let Some(ref currency) = input.currency {
            validate_currency_code(currency.as_str())?;
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let id = CartId::new();
        let cart_number = Self::generate_cart_number();
        let now = Utc::now();
        let currency = input.currency.unwrap_or_default();

        let expires_at = input.expires_in_minutes.map(|mins| now + Duration::minutes(mins));

        let shipping_address_json =
            input.shipping_address.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default());
        let billing_address_json =
            input.billing_address.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default());
        let metadata_json =
            input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());

        tx.execute(
            "INSERT INTO carts (id, cart_number, customer_id, status, currency,
                               subtotal, tax_amount, shipping_amount, discount_amount, grand_total,
                               customer_email, customer_name, shipping_address, billing_address,
                               notes, metadata, expires_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &cart_number,
                input.customer_id.map(|c| c.to_string()),
                "active",
                &currency,
                "0",
                "0",
                "0",
                "0",
                "0",
                &input.customer_email,
                &input.customer_name,
                &shipping_address_json,
                &billing_address_json,
                &input.notes,
                &metadata_json,
                expires_at.map(|e| e.to_rfc3339()),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Add initial items if provided
        let mut items = vec![];
        if let Some(input_items) = &input.items {
            for item_input in input_items {
                let item = self.add_item_internal(&tx, id, item_input.clone())?;
                items.push(item);
            }
            self.update_cart_totals(&tx, id)?;
        }

        tx.commit().map_err(map_db_error)?;

        let mut cart = Cart {
            id,
            cart_number,
            customer_id: input.customer_id,
            status: CartStatus::Active,
            currency,
            items,
            subtotal: Decimal::ZERO,
            tax_amount: Decimal::ZERO,
            shipping_amount: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            grand_total: Decimal::ZERO,
            customer_email: input.customer_email,
            customer_phone: None,
            customer_name: input.customer_name,
            shipping_address: input.shipping_address,
            billing_address: input.billing_address,
            billing_same_as_shipping: true,
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
        };

        // Recalculate totals
        cart.recalculate_totals();

        Ok(cart)
    }

    fn get(&self, id: CartId) -> Result<Option<Cart>> {
        let conn = self.conn()?;
        Self::load_cart_with_conn(&conn, id)
    }

    fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM carts WHERE cart_number = ?",
            [cart_number],
            Self::row_to_cart,
        );

        match result {
            Ok(mut cart) => {
                cart.items = Self::load_cart_items_with_conn(&conn, cart.id)?;
                Ok(Some(cart))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: CartId, input: UpdateCart) -> Result<Cart> {
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(customer_id) = &input.customer_id {
            updates.push("customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(email) = &input.customer_email {
            updates.push("customer_email = ?");
            params.push(Box::new(email.clone()));
        }
        if let Some(phone) = &input.customer_phone {
            updates.push("customer_phone = ?");
            params.push(Box::new(phone.clone()));
        }
        if let Some(name) = &input.customer_name {
            updates.push("customer_name = ?");
            params.push(Box::new(name.clone()));
        }
        if let Some(addr) = &input.shipping_address {
            updates.push("shipping_address = ?");
            params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
        }
        if let Some(addr) = &input.billing_address {
            updates.push("billing_address = ?");
            params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
        }
        if let Some(same) = &input.billing_same_as_shipping {
            updates.push("billing_same_as_shipping = ?");
            params.push(Box::new(i32::from(*same)));
        }
        if let Some(ft) = &input.fulfillment_type {
            updates.push("fulfillment_type = ?");
            params.push(Box::new(ft.to_string()));
        }
        if let Some(method) = &input.shipping_method {
            updates.push("shipping_method = ?");
            params.push(Box::new(method.clone()));
        }
        if let Some(carrier) = &input.shipping_carrier {
            updates.push("shipping_carrier = ?");
            params.push(Box::new(carrier.clone()));
        }
        if let Some(coupon) = &input.coupon_code {
            updates.push("coupon_code = ?");
            params.push(Box::new(coupon.clone()));
        }
        if let Some(discount_amount) = &input.discount_amount {
            updates.push("discount_amount = ?");
            params.push(Box::new(discount_amount.to_string()));
        }
        if let Some(description) = &input.discount_description {
            updates.push("discount_description = ?");
            params.push(Box::new(description.clone()));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }
        if let Some(meta) = &input.metadata {
            updates.push("metadata = ?");
            params.push(Box::new(serde_json::to_string(meta).unwrap_or_default()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE carts SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        {
            let conn = self.conn()?;
            conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM carts WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(email) = &filter.customer_email {
            sql.push_str(" AND customer_email = ?");
            params.push(Box::new(email.clone()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(has_items) = &filter.has_items {
            if *has_items {
                sql.push_str(" AND id IN (SELECT DISTINCT cart_id FROM cart_items)");
            } else {
                sql.push_str(" AND id NOT IN (SELECT DISTINCT cart_id FROM cart_items)");
            }
        }
        if let Some(true) = &filter.is_abandoned {
            sql.push_str(" AND status = 'abandoned'");
        }
        if let Some(from) = &filter.created_after {
            sql.push_str(" AND created_at >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(to) = &filter.created_before {
            sql.push_str(" AND created_at <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let carts = stmt
            .query_map(params_refs.as_slice(), Self::row_to_cart)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        let ids: Vec<CartId> = carts.iter().map(|c| c.id).collect();
        let mut items_by_id = Self::load_cart_items_batch(&conn, &ids)?;
        let mut result = vec![];
        for mut cart in carts {
            cart.items = items_by_id.remove(&cart.id).unwrap_or_default();
            result.push(cart);
        }

        Ok(result)
    }

    fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Cart>> {
        self.list(CartFilter { customer_id: Some(customer_id), ..Default::default() })
    }

    fn delete(&self, id: CartId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        tx.execute("DELETE FROM cart_items WHERE cart_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.execute("DELETE FROM carts WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn add_item(&self, cart_id: CartId, item: AddCartItem) -> Result<CartItem> {
        // Validate item quantity (must be positive)
        if item.quantity <= 0 {
            return Err(CommerceError::ValidationError(format!(
                "Item quantity must be positive, got {} for '{}'",
                item.quantity, item.name
            )));
        }

        // Validate item price
        validate_price(item.unit_price)?;
        if let Some(original_price) = item.original_price {
            validate_price(original_price)?;
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let result = self.add_item_internal(&tx, cart_id, item)?;
        self.update_cart_totals(&tx, cart_id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(result)
    }

    fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let now = Utc::now();

        // Get cart_id for this item
        let cart_id: String = tx
            .query_row(
                "SELECT cart_id FROM cart_items WHERE id = ?",
                [item_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(qty) = input.quantity {
            updates.push("quantity = ?");
            params.push(Box::new(qty));
        }
        if let Some(price) = input.unit_price {
            updates.push("unit_price = ?");
            params.push(Box::new(price.to_string()));
        }
        if let Some(discount) = input.discount_amount {
            updates.push("discount_amount = ?");
            params.push(Box::new(discount.to_string()));
        }
        if let Some(meta) = &input.metadata {
            updates.push("metadata = ?");
            params.push(Box::new(serde_json::to_string(meta).unwrap_or_default()));
        }

        params.push(Box::new(item_id.to_string()));

        let sql = format!("UPDATE cart_items SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Recalculate item total
        let (qty, unit_price, discount, tax): (i32, String, String, String) = tx
            .query_row(
                "SELECT quantity, unit_price, discount_amount, tax_amount FROM cart_items WHERE id = ?",
                [item_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let total = CartItem::calculate_total(
            qty,
            parse_decimal_err(&unit_price, "cart_item", "unit_price")?,
            parse_decimal_err(&discount, "cart_item", "discount_amount")?,
            parse_decimal_err(&tax, "cart_item", "tax_amount")?,
        );

        tx.execute(
            "UPDATE cart_items SET total = ? WHERE id = ?",
            rusqlite::params![total.to_string(), item_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update cart totals
        let cart_uuid = CartId::from(parse_uuid(&cart_id, "cart_item", "cart_id")?);
        self.update_cart_totals(&tx, cart_uuid)?;

        // Return updated item
        let item = tx
            .query_row("SELECT * FROM cart_items WHERE id = ?", [item_id.to_string()], |row| {
                let metadata: Option<String> = row.get("metadata")?;
                Ok(CartItem {
                    id: parse_uuid_row(&row.get::<_, String>("id")?, "cart_item", "id")?,
                    cart_id: CartId::from(parse_uuid_row(
                        &row.get::<_, String>("cart_id")?,
                        "cart_item",
                        "cart_id",
                    )?),
                    product_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>("product_id")?,
                        "cart_item",
                        "product_id",
                    )?
                    .map(ProductId::from),
                    variant_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>("variant_id")?,
                        "cart_item",
                        "variant_id",
                    )?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    image_url: row.get("image_url")?,
                    quantity: row.get("quantity")?,
                    unit_price: parse_decimal_row(
                        &row.get::<_, String>("unit_price")?,
                        "cart_item",
                        "unit_price",
                    )?,
                    original_price: parse_decimal_opt_row(
                        row.get::<_, Option<String>>("original_price")?,
                        "cart_item",
                        "original_price",
                    )?,
                    discount_amount: parse_decimal_row(
                        &row.get::<_, String>("discount_amount")?,
                        "cart_item",
                        "discount_amount",
                    )?,
                    tax_amount: parse_decimal_row(
                        &row.get::<_, String>("tax_amount")?,
                        "cart_item",
                        "tax_amount",
                    )?,
                    total: parse_decimal_row(
                        &row.get::<_, String>("total")?,
                        "cart_item",
                        "total",
                    )?,
                    weight: parse_decimal_opt_row(
                        row.get::<_, Option<String>>("weight")?,
                        "cart_item",
                        "weight",
                    )?,
                    requires_shipping: row.get::<_, i32>("requires_shipping")? == 1,
                    metadata: parse_json_opt_row(metadata, "cart_item", "metadata")?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "cart_item",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "cart_item",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Ok(item)
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Get cart_id before deleting
        let cart_id: String = tx
            .query_row(
                "SELECT cart_id FROM cart_items WHERE id = ?",
                [item_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        tx.execute("DELETE FROM cart_items WHERE id = ?", [item_id.to_string()])
            .map_err(map_db_error)?;

        let cart_uuid = CartId::from(parse_uuid(&cart_id, "cart_item", "cart_id")?);
        self.update_cart_totals(&tx, cart_uuid)?;
        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn get_items(&self, cart_id: CartId) -> Result<Vec<CartItem>> {
        let conn = self.conn()?;
        Self::load_cart_items_with_conn(&conn, cart_id)
    }

    fn clear_items(&self, cart_id: CartId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        tx.execute("DELETE FROM cart_items WHERE cart_id = ?", [cart_id.to_string()])
            .map_err(map_db_error)?;
        self.update_cart_totals(&tx, cart_id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn set_shipping_address(&self, id: CartId, address: CartAddress) -> Result<Cart> {
        let address_json = serde_json::to_string(&address).unwrap_or_default();

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET shipping_address = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![address_json, Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn set_billing_address(&self, id: CartId, address: CartAddress) -> Result<Cart> {
        let address_json = serde_json::to_string(&address).unwrap_or_default();

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET billing_address = ?, billing_same_as_shipping = 0, updated_at = ? WHERE id = ?",
                rusqlite::params![address_json, Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn set_shipping(&self, id: CartId, shipping: SetCartShipping) -> Result<Cart> {
        let address_json = serde_json::to_string(&shipping.shipping_address).unwrap_or_default();
        let shipping_amount = shipping.shipping_amount.unwrap_or_default();

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET shipping_address = ?, shipping_method = ?, shipping_carrier = ?,
             shipping_amount = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    address_json,
                    shipping.shipping_method,
                    shipping.shipping_carrier,
                    shipping_amount.to_string(),
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        // Recalculate grand total
        self.recalculate(id)
    }

    fn get_shipping_rates(&self, _id: CartId) -> Result<Vec<ShippingRate>> {
        // This would typically integrate with shipping providers
        // For now, return some default rates
        Ok(vec![
            ShippingRate {
                id: "standard".to_string(),
                carrier: "USPS".to_string(),
                service: "Ground".to_string(),
                description: Some("Standard shipping (5-7 business days)".to_string()),
                price: Decimal::new(599, 2), // $5.99
                currency: CurrencyCode::default(),
                estimated_days: Some(7),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "express".to_string(),
                carrier: "UPS".to_string(),
                service: "Express".to_string(),
                description: Some("Express shipping (2-3 business days)".to_string()),
                price: Decimal::new(1499, 2), // $14.99
                currency: CurrencyCode::default(),
                estimated_days: Some(3),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "overnight".to_string(),
                carrier: "FedEx".to_string(),
                service: "Overnight".to_string(),
                description: Some("Next business day delivery".to_string()),
                price: Decimal::new(2999, 2), // $29.99
                currency: CurrencyCode::default(),
                estimated_days: Some(1),
                estimated_delivery: None,
            },
        ])
    }

    fn set_payment(&self, id: CartId, payment: SetCartPayment) -> Result<Cart> {
        let billing_json = payment
            .billing_address
            .as_ref()
            .map(|addr| serde_json::to_string(addr).unwrap_or_default());

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET payment_method = ?, payment_token = ?, payment_status = 'method_selected',
                 billing_address = COALESCE(?, billing_address), updated_at = ? WHERE id = ?",
                rusqlite::params![
                    payment.payment_method,
                    payment.payment_token,
                    billing_json,
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn set_x402_payment(&self, id: CartId, payment: SetCartX402Payment) -> Result<Cart> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE carts SET
                x402_payer_address = ?, x402_network = ?, x402_asset = ?,
                x402_status = ?, payment_method = 'x402', updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                payment.payer_address,
                payment.network.to_string(),
                payment.asset.to_string().to_lowercase(),
                X402IntentStatus::Created.to_string(),
                Utc::now().to_rfc3339(),
                id.to_string()
            ],
        )
        .map_err(map_db_error)?;

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete_with_x402(&self, id: CartId, payee_address: &str) -> Result<X402CheckoutResult> {
        use rust_decimal::prelude::ToPrimitive;

        let cart = self.get(id)?.ok_or(CommerceError::NotFound)?;

        if cart.status == CartStatus::Completed {
            if let (Some(order_id), Some(order_number)) = (cart.order_id, cart.order_number.clone())
            {
                return Ok(X402CheckoutResult::Completed(CheckoutResult {
                    cart_id: id,
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

        // Validate cart is ready for checkout
        if !cart.is_ready_for_checkout() {
            return Err(CommerceError::ValidationError(
                "Cart is not ready for checkout - ensure items, customer info, and shipping address are set".to_string(),
            ));
        }

        // Ensure x402 payment is configured
        let x402_payment = cart.x402_payment.as_ref().ok_or_else(|| {
            CommerceError::ValidationError(
                "x402 payment not configured. Call set_x402_payment first".to_string(),
            )
        })?;

        // Calculate amount in smallest unit
        let decimals = x402_payment.asset.decimals();
        let multiplier = rust_decimal::Decimal::from(10u64.pow(u32::from(decimals)));
        let amount_scaled = cart.grand_total * multiplier;
        let amount = amount_scaled.to_u64().unwrap_or(0);
        let amount_display = format!("{:.6} {}", cart.grand_total, x402_payment.asset);

        // Check if there's an existing intent
        if let Some(intent_id) = x402_payment.intent_id {
            // Get the intent status from x402_payment_intents table
            let conn = self.conn()?;

            type IntentStatusRow = (String, Option<String>, Option<i64>, Option<String>);

            let status_result: Option<IntentStatusRow> = conn
                .query_row(
                    "SELECT status, signing_hash, sequence_number, batch_id FROM x402_payment_intents WHERE id = ?",
                    [intent_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(map_db_error)?;

            if let Some((status_str, signing_hash, seq_num, batch_id_str)) = status_result {
                let status: X402IntentStatus = status_str.parse().unwrap_or_default();
                match status {
                    X402IntentStatus::Settled => {
                        // Payment is settled - complete the checkout
                        return self.finalize_x402_checkout(id);
                    }
                    X402IntentStatus::Signed
                    | X402IntentStatus::Sequenced
                    | X402IntentStatus::Batched => {
                        // Awaiting settlement
                        return Ok(X402CheckoutResult::AwaitingSettlement(
                            X402AwaitingSettlementData {
                                cart_id: id,
                                intent_id,
                                status,
                                sequence_number: seq_num.map(|n| n as u64),
                                batch_id: batch_id_str.and_then(|s| s.parse().ok()),
                            },
                        ));
                    }
                    X402IntentStatus::Created => {
                        // Intent exists but not signed yet
                        return Ok(X402CheckoutResult::IntentCreated(X402IntentCreatedData {
                            cart_id: id,
                            intent_id,
                            signing_hash: signing_hash.unwrap_or_default(),
                            amount,
                            amount_display,
                            asset: x402_payment.asset,
                            network: x402_payment.network,
                            payee_address: payee_address.to_string(),
                            valid_until: 0, // Would need to fetch from intent
                            nonce: 0,
                        }));
                    }
                    X402IntentStatus::Expired
                    | X402IntentStatus::Failed
                    | X402IntentStatus::Cancelled
                    | _ => {
                        // Need to create a new intent
                    }
                }
            }
        }

        // No valid intent exists - return PaymentRequired
        let chain_id = x402_payment.network.chain_id();
        Ok(X402CheckoutResult::PaymentRequired(X402PaymentRequiredData {
            cart_id: id,
            payee_address: payee_address.to_string(),
            amount,
            amount_display,
            asset: x402_payment.asset,
            network: x402_payment.network,
            chain_id,
            valid_seconds: 3600, // 1 hour default
        }))
    }

    fn apply_discount(&self, id: CartId, coupon_code: &str) -> Result<Cart> {
        // Get the cart first to calculate discount
        let cart = self.get(id)?.ok_or(CommerceError::NotFound)?;

        // Look up the coupon and its promotion
        let promo_repo = SqlitePromotionRepository::new(self.pool.clone());
        let coupon = promo_repo.get_coupon_by_code(coupon_code)?.ok_or_else(|| {
            CommerceError::ValidationError(format!("Invalid coupon code: {coupon_code}"))
        })?;

        let promotion = promo_repo
            .get(coupon.promotion_id)?
            .ok_or_else(|| CommerceError::ValidationError("Promotion not found".into()))?;

        // Calculate the discount based on promotion type
        let subtotal = cart.subtotal;
        let discount_amount = match promotion.promotion_type {
            PromotionType::PercentageOff => {
                let percentage = promotion.percentage_off.unwrap_or(Decimal::ZERO);
                let discount = subtotal * percentage;
                // Apply max discount cap if set
                if let Some(max) = promotion.max_discount_amount {
                    discount.min(max)
                } else {
                    discount
                }
            }
            PromotionType::FixedAmountOff => {
                promotion.fixed_amount_off.unwrap_or(Decimal::ZERO).min(subtotal)
            }
            _ => Decimal::ZERO, // Other types not fully implemented
        };

        let discount_description = Some(promotion.name);

        // Update the cart with the discount
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET coupon_code = ?, discount_amount = ?, discount_description = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    coupon_code,
                    discount_amount.to_string(),
                    discount_description,
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        // Recalculate totals and return
        self.recalculate(id)
    }

    fn remove_discount(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET coupon_code = NULL, discount_amount = '0', discount_description = NULL,
             updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.recalculate(id)
    }

    fn mark_ready_for_payment(&self, id: CartId) -> Result<Cart> {
        let cart = self.get(id)?.ok_or(CommerceError::NotFound)?;

        if !cart.is_ready_for_checkout() {
            return Err(CommerceError::ValidationError(
                "Cart is not ready for checkout".to_string(),
            ));
        }

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'ready_for_payment', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn begin_checkout(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'payment_pending', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn complete(&self, id: CartId) -> Result<CheckoutResult> {
        with_immediate_transaction(&self.pool, |tx| {
            self.complete_checkout_in_tx(tx, id, false, false)
        })
    }

    fn complete_settled_externally(&self, id: CartId) -> Result<CheckoutResult> {
        with_immediate_transaction(&self.pool, |tx| {
            self.complete_checkout_in_tx(tx, id, false, true)
        })
    }

    fn cancel(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'cancelled', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn abandon(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'abandoned', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn expire(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'expired', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn reserve_inventory(&self, id: CartId) -> Result<Cart> {
        let reservation_expires = Utc::now() + Duration::minutes(15);

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET inventory_reserved = 1, reservation_expires_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    reservation_expires.to_rfc3339(),
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn release_inventory(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET inventory_reserved = 0, reservation_expires_at = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn recalculate(&self, id: CartId) -> Result<Cart> {
        {
            let conn = self.conn()?;
            self.update_cart_totals(&conn, id)?;
        }

        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn set_tax(&self, id: CartId, tax_amount: Decimal) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET tax_amount = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![tax_amount.to_string(), Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.recalculate(id)
    }

    fn get_abandoned(&self) -> Result<Vec<Cart>> {
        self.list(CartFilter { status: Some(CartStatus::Abandoned), ..Default::default() })
    }

    fn get_expired(&self) -> Result<Vec<Cart>> {
        let now = Utc::now();

        // Also mark expired carts
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'expired' WHERE status = 'active' AND expires_at IS NOT NULL AND expires_at < ?",
                [now.to_rfc3339()],
            )
            .map_err(map_db_error)?;
        }

        self.list(CartFilter { status: Some(CartStatus::Expired), ..Default::default() })
    }

    fn count(&self, filter: CartFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM carts WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateCart>) -> Result<BatchResult<Cart>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(cart) => result.record_success(cart),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateCart>) -> Result<Vec<Cart>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = CartId::new();
            let cart_number = Self::generate_cart_number();
            let now = Utc::now();
            let currency = input.currency.unwrap_or_default();

            let expires_at = input.expires_in_minutes.map(|mins| now + Duration::minutes(mins));

            let shipping_address_json = input
                .shipping_address
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default());
            let billing_address_json = input
                .billing_address
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default());
            let metadata_json =
                input.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());

            tx.execute(
                "INSERT INTO carts (id, cart_number, customer_id, status, currency,
                                   subtotal, tax_amount, shipping_amount, discount_amount, grand_total,
                                   customer_email, customer_name, shipping_address, billing_address,
                                   notes, metadata, expires_at, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &cart_number,
                    input.customer_id.map(|c| c.to_string()),
                    "active",
                    &currency,
                    "0",
                    "0",
                    "0",
                    "0",
                    "0",
                    &input.customer_email,
                    &input.customer_name,
                    &shipping_address_json,
                    &billing_address_json,
                    &input.notes,
                    &metadata_json,
                    expires_at.map(|e| e.to_rfc3339()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            // Add initial items if provided
            let mut items = vec![];
            if let Some(input_items) = &input.items {
                for item_input in input_items {
                    let item = self.add_item_internal(&tx, id, item_input.clone())?;
                    items.push(item);
                }
                self.update_cart_totals(&tx, id)?;
            }

            let mut cart = Cart {
                id,
                cart_number,
                customer_id: input.customer_id,
                status: CartStatus::Active,
                currency,
                items,
                subtotal: Decimal::ZERO,
                tax_amount: Decimal::ZERO,
                shipping_amount: Decimal::ZERO,
                discount_amount: Decimal::ZERO,
                grand_total: Decimal::ZERO,
                customer_email: input.customer_email,
                customer_phone: None,
                customer_name: input.customer_name,
                shipping_address: input.shipping_address,
                billing_address: input.billing_address,
                billing_same_as_shipping: true,
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
                expires_at,
                completed_at: None,
                x402_payment: None,
                created_at: now,
                updated_at: now,
            };

            cart.recalculate_totals();
            results.push(cart);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(CartId, UpdateCart)>) -> Result<BatchResult<Cart>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(cart) => result.record_success(cart),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(CartId, UpdateCart)>) -> Result<Vec<Cart>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(customer_id) = &input.customer_id {
                update_parts.push("customer_id = ?");
                params.push(Box::new(customer_id.to_string()));
            }
            if let Some(email) = &input.customer_email {
                update_parts.push("customer_email = ?");
                params.push(Box::new(email.clone()));
            }
            if let Some(phone) = &input.customer_phone {
                update_parts.push("customer_phone = ?");
                params.push(Box::new(phone.clone()));
            }
            if let Some(name) = &input.customer_name {
                update_parts.push("customer_name = ?");
                params.push(Box::new(name.clone()));
            }
            if let Some(addr) = &input.shipping_address {
                update_parts.push("shipping_address = ?");
                params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
            }
            if let Some(addr) = &input.billing_address {
                update_parts.push("billing_address = ?");
                params.push(Box::new(serde_json::to_string(addr).unwrap_or_default()));
            }
            if let Some(same) = &input.billing_same_as_shipping {
                update_parts.push("billing_same_as_shipping = ?");
                params.push(Box::new(i32::from(*same)));
            }
            if let Some(ft) = &input.fulfillment_type {
                update_parts.push("fulfillment_type = ?");
                params.push(Box::new(ft.to_string()));
            }
            if let Some(method) = &input.shipping_method {
                update_parts.push("shipping_method = ?");
                params.push(Box::new(method.clone()));
            }
            if let Some(carrier) = &input.shipping_carrier {
                update_parts.push("shipping_carrier = ?");
                params.push(Box::new(carrier.clone()));
            }
            if let Some(coupon) = &input.coupon_code {
                update_parts.push("coupon_code = ?");
                params.push(Box::new(coupon.clone()));
            }
            if let Some(notes) = &input.notes {
                update_parts.push("notes = ?");
                params.push(Box::new(notes.clone()));
            }
            if let Some(meta) = &input.metadata {
                update_parts.push("metadata = ?");
                params.push(Box::new(serde_json::to_string(meta).unwrap_or_default()));
            }

            params.push(Box::new(id.to_string()));

            let sql = format!("UPDATE carts SET {} WHERE id = ?", update_parts.join(", "));
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();
            let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

            if rows_affected == 0 {
                return Err(CommerceError::NotFound);
            }

            // Fetch the updated cart
            let cart = tx
                .query_row("SELECT * FROM carts WHERE id = ?", [id.to_string()], Self::row_to_cart)
                .map_err(map_db_error)?;

            results.push(cart);
        }

        tx.commit().map_err(map_db_error)?;

        // Load items for all carts in one batched query
        let conn = self.conn()?;
        let ids: Vec<CartId> = results.iter().map(|c| c.id).collect();
        let mut items_by_id = Self::load_cart_items_batch(&conn, &ids)?;
        for cart in &mut results {
            cart.items = items_by_id.remove(&cart.id).unwrap_or_default();
        }

        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<CartId>) -> Result<BatchResult<CartId>> {
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

    fn delete_batch_atomic(&self, ids: Vec<CartId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        // Delete cart items first
        let sql = format!("DELETE FROM cart_items WHERE cart_id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete carts
        let sql = format!("DELETE FROM carts WHERE id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<CartId>) -> Result<Vec<Cart>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM carts WHERE id IN ({placeholders})");

        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let carts = stmt
            .query_map(params_refs.as_slice(), Self::row_to_cart)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for all carts in one batched query
        let cart_ids: Vec<CartId> = carts.iter().map(|c| c.id).collect();
        let mut items_by_id = Self::load_cart_items_batch(&conn, &cart_ids)?;
        let mut result = vec![];
        for mut cart in carts {
            cart.items = items_by_id.remove(&cart.id).unwrap_or_default();
            result.push(cart);
        }

        Ok(result)
    }
}

// Internal helper methods
impl SqliteCartRepository {
    fn add_item_internal(
        &self,
        conn: &rusqlite::Connection,
        cart_id: CartId,
        item: AddCartItem,
    ) -> Result<CartItem> {
        let item_id = Uuid::new_v4();
        let now = Utc::now();
        let requires_shipping = item.requires_shipping.unwrap_or(true);

        let total =
            CartItem::calculate_total(item.quantity, item.unit_price, Decimal::ZERO, Decimal::ZERO);

        let metadata_json =
            item.metadata.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());

        conn.execute(
            "INSERT INTO cart_items (id, cart_id, product_id, variant_id, sku, name, description,
                                     image_url, quantity, unit_price, original_price, discount_amount,
                                     tax_amount, total, weight, requires_shipping, metadata,
                                     created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                item_id.to_string(),
                cart_id.to_string(),
                item.product_id.map(|p| p.to_string()),
                item.variant_id.map(|v| v.to_string()),
                item.sku,
                item.name,
                item.description,
                item.image_url,
                item.quantity,
                item.unit_price.to_string(),
                item.original_price.map(|p| p.to_string()),
                "0",
                "0",
                total.to_string(),
                item.weight.map(|w| w.to_string()),
                i32::from(requires_shipping),
                metadata_json,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
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

    fn resolve_customer_id_with_conn(
        conn: &rusqlite::Connection,
        cart: &Cart,
    ) -> Result<CustomerId> {
        if let Some(customer_id) = cart.customer_id {
            return Ok(customer_id);
        }

        let email = cart.customer_email.as_deref().ok_or_else(|| {
            CommerceError::ValidationError(
                "Cart must have a customer_id or customer_email to create an order".to_string(),
            )
        })?;

        validate_email(email)?;

        let (first_name, last_name) = split_customer_name(cart.customer_name.as_deref());
        validate_required_text("customer.first_name", &first_name, 100)?;
        validate_required_text("customer.last_name", &last_name, 100)?;
        if let Some(phone) = &cart.customer_phone {
            validate_phone(phone)?;
        }

        let result = conn.query_row("SELECT id FROM customers WHERE email = ?", [email], |row| {
            row.get::<_, String>(0)
        });

        match result {
            Ok(id) => Ok(CustomerId::from(parse_uuid(&id, "customer", "id")?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let customer_id = CustomerId::new();
                let now = Utc::now();
                conn.execute(
                    "INSERT INTO customers (id, email, first_name, last_name, phone, status,
                                            accepts_marketing, email_verified, tags, metadata,
                                            created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        customer_id.to_string(),
                        email,
                        &first_name,
                        &last_name,
                        &cart.customer_phone,
                        "active",
                        0,
                        0,
                        "[]",
                        Option::<String>::None,
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                    ],
                )
                .map_err(map_db_error)?;
                Ok(customer_id)
            }
            Err(error) => Err(map_db_error(error)),
        }
    }

    fn resolve_customer_id_in_tx(
        tx: &rusqlite::Transaction<'_>,
        cart: &Cart,
    ) -> std::result::Result<CustomerId, rusqlite::Error> {
        Self::resolve_customer_id_with_conn(tx, cart)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
    }

    pub(crate) fn validate_checkout_in_tx(
        tx: &rusqlite::Transaction<'_>,
        cart_id: CartId,
    ) -> std::result::Result<(), rusqlite::Error> {
        let mut cart = tx.query_row(
            "SELECT * FROM carts WHERE id = ?",
            [cart_id.to_string()],
            Self::row_to_cart,
        )?;
        cart.items = Self::load_cart_items_with_conn(tx, cart_id)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        if cart.status == CartStatus::Completed {
            return Ok(());
        }
        if !cart.is_checkoutable_status() {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::Conflict(format!(
                    "Cart cannot be checked out in status: {}",
                    cart.status
                )),
            )));
        }
        if !cart.is_ready_for_checkout() {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::ValidationError("Cart is not ready for checkout".into()),
            )));
        }
        let customer_id = if let Some(id) = cart.customer_id {
            id
        } else {
            let email = cart.customer_email.as_deref().ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::ValidationError(
                    "Customer ID or email required".into(),
                )))
            })?;
            validate_email(email)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            CustomerId::new()
        };
        let input = CreateOrder {
            customer_id,
            items: cart
                .items
                .iter()
                .map(|item| CreateOrderItem {
                    product_id: item.product_id.unwrap_or_else(ProductId::new),
                    variant_id: item.variant_id,
                    sku: item.sku.clone(),
                    name: item.name.clone(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    discount: Some(item.discount_amount),
                    tax_amount: Some(item.tax_amount),
                })
                .collect(),
            currency: Some(cart.currency),
            shipping_address: cart.shipping_address.clone().map(Into::into),
            billing_address: if cart.billing_same_as_shipping {
                cart.billing_address
                    .clone()
                    .or_else(|| cart.shipping_address.clone())
                    .map(Into::into)
            } else {
                cart.billing_address.clone().map(Into::into)
            },
            notes: cart.notes,
            payment_method: cart.payment_method,
            shipping_method: cart.shipping_method,
            stock_policy: stateset_core::StockPolicy::default(),
        };
        SqliteOrderRepository::validate_create_order_in_tx(tx, &input)
    }

    /// `mark_paid` promotes the minted order straight to `PaymentStatus::Paid`
    /// without a payment record. That is only correct when settlement happened
    /// out of band (x402, ACP, external PSP) and the caller opted in
    /// explicitly; the plain checkout path leaves payment pending so a
    /// miswired integration cannot mint revenue-recognized orders with no
    /// payment trail.
    pub(crate) fn complete_checkout_in_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        cart_id: CartId,
        x402_settled: bool,
        mark_paid: bool,
    ) -> std::result::Result<CheckoutResult, rusqlite::Error> {
        let mut cart = match tx.query_row(
            "SELECT * FROM carts WHERE id = ?",
            [cart_id.to_string()],
            Self::row_to_cart,
        ) {
            Ok(cart) => cart,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::NotFound,
                )));
            }
            Err(error) => return Err(error),
        };
        cart.items = Self::load_cart_items_with_conn(tx, cart_id)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

        if cart.status == CartStatus::Completed {
            if let (Some(order_id), Some(order_number)) = (cart.order_id, cart.order_number.clone())
            {
                return Ok(CheckoutResult {
                    cart_id,
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
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::Conflict(format!(
                    "Cart cannot be checked out in status: {}",
                    cart.status
                )),
            )));
        }

        if !cart.is_ready_for_checkout() {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::ValidationError(
                    "Cart is not ready for checkout - ensure items, customer info, and shipping address are set".to_string(),
                ),
            )));
        }

        let customer_id = Self::resolve_customer_id_in_tx(tx, &cart)?;
        let order_items: Vec<CreateOrderItem> = cart
            .items
            .iter()
            .map(|item| CreateOrderItem {
                product_id: item.product_id.unwrap_or_else(ProductId::new),
                variant_id: item.variant_id,
                sku: item.sku.clone(),
                name: item.name.clone(),
                quantity: item.quantity,
                unit_price: item.unit_price,
                discount: Some(item.discount_amount),
                tax_amount: Some(item.tax_amount),
            })
            .collect();

        let shipping_address = cart.shipping_address.clone().map(Into::into);
        let billing_address = if cart.billing_same_as_shipping {
            cart.billing_address.clone().or_else(|| cart.shipping_address.clone()).map(Into::into)
        } else {
            cart.billing_address.clone().map(Into::into)
        };

        let mut order = SqliteOrderRepository::create_from_cart_in_tx(
            tx,
            cart_id.into_uuid(),
            &CreateOrder {
                customer_id,
                items: order_items,
                currency: Some(cart.currency),
                shipping_address,
                billing_address,
                notes: cart.notes.clone(),
                payment_method: cart.payment_method.clone(),
                shipping_method: cart.shipping_method.clone(),
                stock_policy: stateset_core::StockPolicy::default(),
            },
        )?;

        if order.status != OrderStatus::Confirmed
            && !order.status.can_transition_to(OrderStatus::Confirmed)
        {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::InvalidOrderStatusTransition {
                    from: order.status.to_string(),
                    to: OrderStatus::Confirmed.to_string(),
                },
            )));
        }

        let target_payment_status =
            if mark_paid { PaymentStatus::Paid } else { order.payment_status };
        if order.status != OrderStatus::Confirmed || order.payment_status != target_payment_status {
            let now = Utc::now();
            tx.execute(
                "UPDATE orders SET status = ?, payment_status = ?, updated_at = ?, version = version + 1 WHERE id = ?",
                rusqlite::params![
                    OrderStatus::Confirmed.to_string(),
                    target_payment_status.to_string(),
                    now.to_rfc3339(),
                    order.id.to_string(),
                ],
            )?;
            order.status = OrderStatus::Confirmed;
            order.payment_status = target_payment_status;
            order.updated_at = now;
            order.version += 1;
        }

        let completed_at = Utc::now();
        if x402_settled {
            tx.execute(
                "UPDATE carts SET
                    status = 'completed', order_id = ?, order_number = ?,
                    payment_status = 'captured', x402_status = 'settled',
                    completed_at = ?, updated_at = ?, customer_id = ?
                 WHERE id = ?",
                rusqlite::params![
                    order.id.to_string(),
                    &order.order_number,
                    completed_at.to_rfc3339(),
                    completed_at.to_rfc3339(),
                    customer_id.to_string(),
                    cart_id.to_string()
                ],
            )?;
        } else if mark_paid {
            tx.execute(
                "UPDATE carts SET status = 'completed', order_id = ?, order_number = ?,
                 payment_status = 'captured', completed_at = ?, updated_at = ?, customer_id = ? WHERE id = ?",
                rusqlite::params![
                    order.id.to_string(),
                    &order.order_number,
                    completed_at.to_rfc3339(),
                    completed_at.to_rfc3339(),
                    customer_id.to_string(),
                    cart_id.to_string()
                ],
            )?;
        } else {
            tx.execute(
                "UPDATE carts SET status = 'completed', order_id = ?, order_number = ?,
                 completed_at = ?, updated_at = ?, customer_id = ? WHERE id = ?",
                rusqlite::params![
                    order.id.to_string(),
                    &order.order_number,
                    completed_at.to_rfc3339(),
                    completed_at.to_rfc3339(),
                    customer_id.to_string(),
                    cart_id.to_string()
                ],
            )?;
        }

        Ok(CheckoutResult {
            cart_id,
            order_id: order.id,
            order_number: order.order_number,
            payment_id: None,
            total_charged: cart.grand_total,
            currency: cart.currency,
        })
    }
}

fn split_customer_name(name: Option<&str>) -> (String, String) {
    let trimmed = name.unwrap_or("").trim();
    if trimmed.is_empty() {
        return ("Guest".to_string(), "Customer".to_string());
    }

    let mut parts = trimmed.split_whitespace();
    let first_name = parts.next().unwrap_or("Guest");
    let last_name = parts.collect::<Vec<_>>().join(" ");

    if last_name.is_empty() {
        (first_name.to_string(), "Customer".to_string())
    } else {
        (first_name.to_string(), last_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CartStatus, CommerceError};

    fn fresh_repo() -> SqliteCartRepository {
        SqliteDatabase::in_memory().expect("in-memory sqlite").carts()
    }

    fn addr() -> CartAddress {
        CartAddress {
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            company: None,
            line1: "1 Babbage Way".into(),
            line2: None,
            city: "London".into(),
            state: None,
            postal_code: "NW1".into(),
            country: "GB".into(),
            phone: Some("+44 20 7946 0000".into()),
            email: Some("ada@example.com".into()),
        }
    }

    fn add_item(sku: &str, qty: i32, price: Decimal) -> AddCartItem {
        AddCartItem {
            sku: sku.into(),
            name: format!("Item {sku}"),
            quantity: qty,
            unit_price: price,
            ..Default::default()
        }
    }

    #[test]
    fn create_cart_minimal_starts_active_and_zeroed() {
        let repo = fresh_repo();
        let cart = repo
            .create(CreateCart {
                customer_email: Some("buyer@example.com".into()),
                customer_name: Some("Test Buyer".into()),
                ..Default::default()
            })
            .expect("create");
        assert_eq!(cart.status, CartStatus::Active);
        assert_eq!(cart.subtotal, dec!(0));
        assert_eq!(cart.tax_amount, dec!(0));
        assert_eq!(cart.shipping_amount, dec!(0));
        assert_eq!(cart.grand_total, dec!(0));
        assert!(cart.items.is_empty());
        assert!(cart.cart_number.starts_with("CART-"));
    }

    #[test]
    fn create_cart_with_items_persists_them() {
        let repo = fresh_repo();
        let cart = repo
            .create(CreateCart {
                customer_email: Some("with-items@example.com".into()),
                items: Some(vec![add_item("SKU-A", 2, dec!(10)), add_item("SKU-B", 1, dec!(5))]),
                ..Default::default()
            })
            .expect("create");
        let items = repo.get_items(cart.id).expect("get_items");
        assert_eq!(items.len(), 2);
        let fresh = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(fresh.subtotal, dec!(25));
    }

    #[test]
    fn get_by_number_round_trips() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let by_num = repo.get_by_number(&cart.cart_number).expect("get_by_number").expect("found");
        assert_eq!(by_num.id, cart.id);
        assert!(repo.get_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn add_item_recomputes_subtotal() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        repo.add_item(cart.id, add_item("SKU-X", 3, dec!(7))).expect("add");
        let fresh = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(fresh.subtotal, dec!(21));
    }

    #[test]
    fn grand_total_never_goes_negative_from_oversized_discount() {
        // A cart-level discount larger than subtotal + tax + shipping must not
        // drive the grand total negative — a negative total would mean charging
        // the buyer a negative amount (crediting them) at checkout.
        let repo = fresh_repo();
        let cart = repo
            .create(CreateCart {
                items: Some(vec![add_item("SKU-A", 2, dec!(10)), add_item("SKU-B", 1, dec!(5))]),
                ..Default::default()
            })
            .expect("create"); // subtotal $25

        repo.update(cart.id, UpdateCart { discount_amount: Some(dec!(100)), ..Default::default() })
            .expect("set oversized discount");
        let recalculated = repo.recalculate(cart.id).expect("recalc");

        assert_eq!(recalculated.subtotal, dec!(25));
        assert_eq!(
            recalculated.grand_total,
            dec!(0),
            "grand total must clamp at zero, not go negative: {recalculated:?}"
        );
    }

    #[test]
    fn update_item_changes_quantity_and_recomputes() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let item = repo.add_item(cart.id, add_item("SKU-U", 1, dec!(10))).expect("add");
        let updated = repo
            .update_item(item.id, UpdateCartItem { quantity: Some(5), ..Default::default() })
            .expect("update");
        assert_eq!(updated.quantity, 5);
        let fresh = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(fresh.subtotal, dec!(50));
    }

    #[test]
    fn remove_item_drops_line_and_decrements_subtotal() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let item_a = repo.add_item(cart.id, add_item("SKU-A", 1, dec!(8))).expect("a");
        let _item_b = repo.add_item(cart.id, add_item("SKU-B", 1, dec!(2))).expect("b");
        repo.remove_item(item_a.id).expect("remove");
        let items = repo.get_items(cart.id).expect("items");
        assert_eq!(items.len(), 1);
        let fresh = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(fresh.subtotal, dec!(2));
    }

    #[test]
    fn set_shipping_address_persists() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let updated = repo.set_shipping_address(cart.id, addr()).expect("set");
        assert!(updated.shipping_address.is_some());
        let stored = updated.shipping_address.expect("addr");
        assert_eq!(stored.first_name, "Ada");
        assert_eq!(stored.country, "GB");
    }

    #[test]
    fn set_shipping_applies_amount_to_total() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        repo.add_item(cart.id, add_item("SKU-S", 1, dec!(20))).expect("add");
        let updated = repo
            .set_shipping(
                cart.id,
                SetCartShipping {
                    shipping_address: addr(),
                    shipping_method: Some("standard".into()),
                    shipping_carrier: Some("usps".into()),
                    shipping_amount: Some(dec!(7)),
                },
            )
            .expect("set shipping");
        assert_eq!(updated.shipping_amount, dec!(7));
        assert!(updated.grand_total >= dec!(27));
    }

    #[test]
    fn set_payment_records_method_and_token() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let updated = repo
            .set_payment(
                cart.id,
                SetCartPayment {
                    payment_method: "credit_card".into(),
                    payment_token: Some("tok_123".into()),
                    billing_address: None,
                },
            )
            .expect("set payment");
        assert_eq!(updated.payment_method.as_deref(), Some("credit_card"));
    }

    #[test]
    fn apply_discount_with_invalid_coupon_returns_validation_error() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let err = repo.apply_discount(cart.id, "DOES-NOT-EXIST").expect_err("err");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    #[test]
    fn abandon_marks_status_abandoned() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        let abandoned = repo.abandon(cart.id).expect("abandon");
        assert_eq!(abandoned.status, CartStatus::Abandoned);
    }

    /// Create a cart that satisfies every `is_ready_for_checkout` requirement
    /// (item, customer email/name, shipping address) so that `complete()` will
    /// mint an order unless the lifecycle guard rejects it.
    fn checkoutable_cart(repo: &SqliteCartRepository) -> Cart {
        let cart = repo
            .create(CreateCart {
                customer_email: Some("buyer@example.com".into()),
                customer_name: Some("Ada Lovelace".into()),
                items: Some(vec![add_item("SKU-CHK", 1, dec!(10))]),
                shipping_address: Some(addr()),
                ..Default::default()
            })
            .expect("create");
        repo.set_shipping_address(cart.id, addr()).expect("ship addr");
        repo.get(cart.id).expect("ok").expect("found")
    }

    #[test]
    fn complete_checks_out_active_cart() {
        let repo = fresh_repo();
        let cart = checkoutable_cart(&repo);
        assert_eq!(cart.status, CartStatus::Active);
        let result = repo.complete(cart.id).expect("checkout should succeed");
        assert!(result.order_number.starts_with("ORD-") || !result.order_number.is_empty());
        let completed = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(completed.status, CartStatus::Completed);
        assert!(completed.order_id.is_some());
    }

    #[test]
    fn complete_leaves_payment_pending_without_explicit_settlement() {
        use stateset_core::OrderRepository as _;

        let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
        let repo = db.carts();
        let cart = checkoutable_cart(&repo);
        let result = repo.complete(cart.id).expect("checkout should succeed");
        assert!(result.payment_id.is_none());

        let order = db.orders().get(result.order_id).expect("ok").expect("order exists");
        assert_eq!(order.status, stateset_core::OrderStatus::Confirmed);
        assert_eq!(
            order.payment_status,
            stateset_core::PaymentStatus::Pending,
            "plain complete() must not mark an order paid with no payment record; \
             out-of-band settlement uses complete_settled_externally()"
        );
    }

    #[test]
    fn complete_settled_externally_marks_order_paid() {
        use stateset_core::OrderRepository as _;

        let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
        let repo = db.carts();
        let cart = checkoutable_cart(&repo);
        let result = repo.complete_settled_externally(cart.id).expect("checkout should succeed");

        let order = db.orders().get(result.order_id).expect("ok").expect("order exists");
        assert_eq!(order.status, stateset_core::OrderStatus::Confirmed);
        assert_eq!(order.payment_status, stateset_core::PaymentStatus::Paid);

        let completed = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(completed.status, CartStatus::Completed);
    }

    #[test]
    fn complete_rejects_cancelled_cart() {
        let repo = fresh_repo();
        let cart = checkoutable_cart(&repo);
        repo.cancel(cart.id).expect("cancel");
        let err = repo.complete(cart.id).expect_err("cancelled cart must not check out");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        // No order was minted.
        let after = repo.get(cart.id).expect("ok").expect("found");
        assert_eq!(after.status, CartStatus::Cancelled);
        assert!(after.order_id.is_none());
    }

    #[test]
    fn complete_rejects_abandoned_cart() {
        let repo = fresh_repo();
        let cart = checkoutable_cart(&repo);
        repo.abandon(cart.id).expect("abandon");
        let err = repo.complete(cart.id).expect_err("abandoned cart must not check out");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        let after = repo.get(cart.id).expect("ok").expect("found");
        assert!(after.order_id.is_none());
    }

    #[test]
    fn complete_rejects_expired_cart() {
        let repo = fresh_repo();
        let cart = checkoutable_cart(&repo);
        repo.expire(cart.id).expect("expire");
        let err = repo.complete(cart.id).expect_err("expired cart must not check out");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        let after = repo.get(cart.id).expect("ok").expect("found");
        assert!(after.order_id.is_none());
    }

    #[test]
    fn delete_removes_cart() {
        let repo = fresh_repo();
        let cart = repo.create(CreateCart::default()).expect("create");
        repo.delete(cart.id).expect("delete");
        assert!(repo.get(cart.id).expect("ok").is_none());
    }

    #[test]
    fn list_filters_by_customer_email() {
        let repo = fresh_repo();
        repo.create(CreateCart {
            customer_email: Some("alice@example.com".into()),
            ..Default::default()
        })
        .expect("alice");
        repo.create(CreateCart {
            customer_email: Some("bob@example.com".into()),
            ..Default::default()
        })
        .expect("bob");
        repo.create(CreateCart {
            customer_email: Some("alice@example.com".into()),
            ..Default::default()
        })
        .expect("alice2");

        let alices = repo
            .list(CartFilter {
                customer_email: Some("alice@example.com".into()),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(alices.len(), 2);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        let cart_a = repo.create(CreateCart::default()).expect("a");
        let _cart_b = repo.create(CreateCart::default()).expect("b");
        repo.abandon(cart_a.id).expect("abandon");

        let active = repo
            .list(CartFilter { status: Some(CartStatus::Active), ..Default::default() })
            .expect("active");
        let abandoned = repo
            .list(CartFilter { status: Some(CartStatus::Abandoned), ..Default::default() })
            .expect("abandoned");
        assert_eq!(active.len(), 1);
        assert_eq!(abandoned.len(), 1);
    }

    #[test]
    fn create_batch_returns_all_succeeded() {
        let repo = fresh_repo();
        let batch = repo
            .create_batch(vec![
                CreateCart { customer_email: Some("c1@example.com".into()), ..Default::default() },
                CreateCart { customer_email: Some("c2@example.com".into()), ..Default::default() },
                CreateCart { customer_email: Some("c3@example.com".into()), ..Default::default() },
            ])
            .expect("batch");
        assert_eq!(batch.success_count, 3);
        assert_eq!(batch.failure_count, 0);
    }

    #[test]
    fn get_batch_returns_only_existing() {
        let repo = fresh_repo();
        let cart_a = repo.create(CreateCart::default()).expect("a");
        let cart_b = repo.create(CreateCart::default()).expect("b");
        let stranger = CartId::new();
        let fetched = repo.get_batch(vec![cart_a.id, cart_b.id, stranger]).expect("get_batch");
        assert_eq!(fetched.len(), 2);
    }

    #[test]
    fn get_returns_none_for_missing_id() {
        let repo = fresh_repo();
        assert!(repo.get(CartId::new()).expect("ok").is_none());
    }

    #[test]
    fn get_abandoned_returns_only_abandoned() {
        let repo = fresh_repo();
        let active = repo.create(CreateCart::default()).expect("active");
        let to_abandon = repo.create(CreateCart::default()).expect("to-abandon");
        repo.abandon(to_abandon.id).expect("abandon");

        let abandoned = repo.get_abandoned().expect("get_abandoned");
        let ids: Vec<CartId> = abandoned.iter().map(|c| c.id).collect();
        assert!(ids.contains(&to_abandon.id));
        assert!(!ids.contains(&active.id));
    }
}
