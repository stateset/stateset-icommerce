//! SQLite cart repository implementation

use super::{map_db_error, parse_decimal};
use chrono::{Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AddCartItem, Cart, CartAddress, CartFilter, CartItem, CartPaymentStatus, CartRepository,
    CartStatus, CheckoutResult, CommerceError, CreateCart, FulfillmentType, Result,
    SetCartPayment, SetCartShipping, ShippingRate, UpdateCart, UpdateCartItem,
};
use uuid::Uuid;

/// SQLite implementation of CartRepository
pub struct SqliteCartRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCartRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_cart_number() -> String {
        let timestamp = Utc::now().timestamp();
        let random: u32 = (Uuid::new_v4().as_u128() % 10000) as u32;
        format!("CART-{}-{:04}", timestamp, random)
    }

    fn row_to_cart(row: &rusqlite::Row) -> rusqlite::Result<Cart> {
        let shipping_addr: Option<String> = row.get("shipping_address")?;
        let billing_addr: Option<String> = row.get("billing_address")?;
        let metadata: Option<String> = row.get("metadata")?;

        Ok(Cart {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            cart_number: row.get("cart_number")?,
            customer_id: row
                .get::<_, Option<String>>("customer_id")?
                .and_then(|s| s.parse().ok()),
            status: parse_cart_status(&row.get::<_, String>("status")?),
            currency: row.get("currency")?,

            items: vec![], // Loaded separately

            subtotal: parse_decimal(&row.get::<_, String>("subtotal")?),
            tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
            shipping_amount: parse_decimal(&row.get::<_, String>("shipping_amount")?),
            discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
            grand_total: parse_decimal(&row.get::<_, String>("grand_total")?),

            customer_email: row.get("customer_email")?,
            customer_phone: row.get("customer_phone")?,
            customer_name: row.get("customer_name")?,

            shipping_address: shipping_addr.and_then(|s| serde_json::from_str(&s).ok()),
            billing_address: billing_addr.and_then(|s| serde_json::from_str(&s).ok()),
            billing_same_as_shipping: row.get::<_, i32>("billing_same_as_shipping")? == 1,

            fulfillment_type: row
                .get::<_, Option<String>>("fulfillment_type")?
                .map(|s| parse_fulfillment_type(&s)),
            shipping_method: row.get("shipping_method")?,
            shipping_carrier: row.get("shipping_carrier")?,
            estimated_delivery: row
                .get::<_, Option<String>>("estimated_delivery")?
                .and_then(|s| s.parse().ok()),

            payment_method: row.get("payment_method")?,
            payment_token: row.get("payment_token")?,
            payment_status: parse_payment_status(&row.get::<_, String>("payment_status")?),

            coupon_code: row.get("coupon_code")?,
            discount_description: row.get("discount_description")?,

            order_id: row
                .get::<_, Option<String>>("order_id")?
                .and_then(|s| s.parse().ok()),
            order_number: row.get("order_number")?,

            notes: row.get("notes")?,
            metadata: metadata.and_then(|s| serde_json::from_str(&s).ok()),

            inventory_reserved: row.get::<_, i32>("inventory_reserved")? == 1,
            reservation_expires_at: row
                .get::<_, Option<String>>("reservation_expires_at")?
                .and_then(|s| s.parse().ok()),

            expires_at: row
                .get::<_, Option<String>>("expires_at")?
                .and_then(|s| s.parse().ok()),
            completed_at: row
                .get::<_, Option<String>>("completed_at")?
                .and_then(|s| s.parse().ok()),
            created_at: row
                .get::<_, String>("created_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
            updated_at: row
                .get::<_, String>("updated_at")?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    fn load_cart_items_with_conn(
        conn: &r2d2::PooledConnection<SqliteConnectionManager>,
        cart_id: Uuid,
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
            .query_map([cart_id.to_string()], |row| {
                let metadata: Option<String> = row.get("metadata")?;
                Ok(CartItem {
                    id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                    cart_id: row.get::<_, String>("cart_id")?.parse().unwrap_or_default(),
                    product_id: row
                        .get::<_, Option<String>>("product_id")?
                        .and_then(|s| s.parse().ok()),
                    variant_id: row
                        .get::<_, Option<String>>("variant_id")?
                        .and_then(|s| s.parse().ok()),
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    image_url: row.get("image_url")?,
                    quantity: row.get("quantity")?,
                    unit_price: parse_decimal(&row.get::<_, String>("unit_price")?),
                    original_price: row
                        .get::<_, Option<String>>("original_price")?
                        .map(|s| parse_decimal(&s)),
                    discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
                    tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
                    total: parse_decimal(&row.get::<_, String>("total")?),
                    weight: row
                        .get::<_, Option<String>>("weight")?
                        .map(|s| parse_decimal(&s)),
                    requires_shipping: row.get::<_, i32>("requires_shipping")? == 1,
                    metadata: metadata.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row
                        .get::<_, String>("created_at")?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: row
                        .get::<_, String>("updated_at")?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn update_cart_totals(
        &self,
        conn: &rusqlite::Connection,
        cart_id: Uuid,
    ) -> Result<()> {
        // Calculate subtotal from items
        let subtotal: String = conn
            .query_row(
                "SELECT COALESCE(CAST(SUM(CAST(total AS REAL)) AS TEXT), '0') FROM cart_items WHERE cart_id = ?",
                [cart_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        // Get current tax and shipping
        let (tax, shipping, discount): (String, String, String) = conn
            .query_row(
                "SELECT tax_amount, shipping_amount, discount_amount FROM carts WHERE id = ?",
                [cart_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;

        // Calculate grand total
        let subtotal_dec = parse_decimal(&subtotal);
        let tax_dec = parse_decimal(&tax);
        let shipping_dec = parse_decimal(&shipping);
        let discount_dec = parse_decimal(&discount);
        let grand_total = subtotal_dec + tax_dec + shipping_dec - discount_dec;

        conn.execute(
            "UPDATE carts SET subtotal = ?, grand_total = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                subtotal,
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
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
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
            .map(|a| serde_json::to_string(a).unwrap_or_default());
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());
        let metadata_json = input
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

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
            expires_at,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };

        // Recalculate totals
        cart.recalculate_totals();

        Ok(cart)
    }

    fn get(&self, id: Uuid) -> Result<Option<Cart>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM carts WHERE id = ?",
            [id.to_string()],
            Self::row_to_cart,
        );

        match result {
            Ok(mut cart) => {
                cart.items = Self::load_cart_items_with_conn(&conn, id)?;
                Ok(Some(cart))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
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

    fn update(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
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
            params.push(Box::new(if *same { 1 } else { 0 }));
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
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        {
            let conn = self.conn()?;
            conn.execute(&sql, params_refs.as_slice())
                .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
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

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let carts = stmt
            .query_map(params_refs.as_slice(), Self::row_to_cart)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        let mut result = vec![];
        for mut cart in carts {
            cart.items = Self::load_cart_items_with_conn(&conn, cart.id)?;
            result.push(cart);
        }

        Ok(result)
    }

    fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        self.list(CartFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        tx.execute("DELETE FROM cart_items WHERE cart_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.execute("DELETE FROM carts WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn add_item(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let result = self.add_item_internal(&tx, cart_id, item)?;
        self.update_cart_totals(&tx, cart_id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(result)
    }

    fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
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
        if let Some(meta) = &input.metadata {
            updates.push("metadata = ?");
            params.push(Box::new(serde_json::to_string(meta).unwrap_or_default()));
        }

        params.push(Box::new(item_id.to_string()));

        let sql = format!("UPDATE cart_items SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        tx.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

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
            parse_decimal(&unit_price),
            parse_decimal(&discount),
            parse_decimal(&tax),
        );

        tx.execute(
            "UPDATE cart_items SET total = ? WHERE id = ?",
            rusqlite::params![total.to_string(), item_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Update cart totals
        let cart_uuid: Uuid = cart_id.parse().unwrap_or_default();
        self.update_cart_totals(&tx, cart_uuid)?;

        // Return updated item
        let item = tx
            .query_row(
                "SELECT * FROM cart_items WHERE id = ?",
                [item_id.to_string()],
                |row| {
                    let metadata: Option<String> = row.get("metadata")?;
                    Ok(CartItem {
                        id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                        cart_id: row.get::<_, String>("cart_id")?.parse().unwrap_or_default(),
                        product_id: row
                            .get::<_, Option<String>>("product_id")?
                            .and_then(|s| s.parse().ok()),
                        variant_id: row
                            .get::<_, Option<String>>("variant_id")?
                            .and_then(|s| s.parse().ok()),
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        description: row.get("description")?,
                        image_url: row.get("image_url")?,
                        quantity: row.get("quantity")?,
                        unit_price: parse_decimal(&row.get::<_, String>("unit_price")?),
                        original_price: row
                            .get::<_, Option<String>>("original_price")?
                            .map(|s| parse_decimal(&s)),
                        discount_amount: parse_decimal(&row.get::<_, String>("discount_amount")?),
                        tax_amount: parse_decimal(&row.get::<_, String>("tax_amount")?),
                        total: parse_decimal(&row.get::<_, String>("total")?),
                        weight: row
                            .get::<_, Option<String>>("weight")?
                            .map(|s| parse_decimal(&s)),
                        requires_shipping: row.get::<_, i32>("requires_shipping")? == 1,
                        metadata: metadata.and_then(|s| serde_json::from_str(&s).ok()),
                        created_at: row
                            .get::<_, String>("created_at")?
                            .parse()
                            .unwrap_or_else(|_| Utc::now()),
                        updated_at: row
                            .get::<_, String>("updated_at")?
                            .parse()
                            .unwrap_or_else(|_| Utc::now()),
                    })
                },
            )
            .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Ok(item)
    }

    fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;

        // Get cart_id before deleting
        let cart_id: String = tx
            .query_row(
                "SELECT cart_id FROM cart_items WHERE id = ?",
                [item_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        tx.execute(
            "DELETE FROM cart_items WHERE id = ?",
            [item_id.to_string()],
        )
        .map_err(map_db_error)?;

        let cart_uuid: Uuid = cart_id.parse().unwrap_or_default();
        self.update_cart_totals(&tx, cart_uuid)?;
        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn get_items(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        let conn = self.conn()?;
        Self::load_cart_items_with_conn(&conn, cart_id)
    }

    fn clear_items(&self, cart_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        tx.execute(
            "DELETE FROM cart_items WHERE cart_id = ?",
            [cart_id.to_string()],
        )
        .map_err(map_db_error)?;
        self.update_cart_totals(&tx, cart_id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn set_shipping_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        let address_json = serde_json::to_string(&address).unwrap_or_default();

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET shipping_address = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![address_json, Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn set_billing_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        let address_json = serde_json::to_string(&address).unwrap_or_default();

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET billing_address = ?, billing_same_as_shipping = 0, updated_at = ? WHERE id = ?",
                rusqlite::params![address_json, Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn set_shipping(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
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

    fn get_shipping_rates(&self, _id: Uuid) -> Result<Vec<ShippingRate>> {
        // This would typically integrate with shipping providers
        // For now, return some default rates
        Ok(vec![
            ShippingRate {
                id: "standard".to_string(),
                carrier: "USPS".to_string(),
                service: "Ground".to_string(),
                description: Some("Standard shipping (5-7 business days)".to_string()),
                price: Decimal::new(599, 2), // $5.99
                currency: "USD".to_string(),
                estimated_days: Some(7),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "express".to_string(),
                carrier: "UPS".to_string(),
                service: "Express".to_string(),
                description: Some("Express shipping (2-3 business days)".to_string()),
                price: Decimal::new(1499, 2), // $14.99
                currency: "USD".to_string(),
                estimated_days: Some(3),
                estimated_delivery: None,
            },
            ShippingRate {
                id: "overnight".to_string(),
                carrier: "FedEx".to_string(),
                service: "Overnight".to_string(),
                description: Some("Next business day delivery".to_string()),
                price: Decimal::new(2999, 2), // $29.99
                currency: "USD".to_string(),
                estimated_days: Some(1),
                estimated_delivery: None,
            },
        ])
    }

    fn set_payment(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        let billing_update = if let Some(addr) = &payment.billing_address {
            let json = serde_json::to_string(addr).unwrap_or_default();
            format!(", billing_address = '{}'", json)
        } else {
            String::new()
        };

        {
            let conn = self.conn()?;
            conn.execute(
                &format!(
                    "UPDATE carts SET payment_method = ?, payment_token = ?, payment_status = 'method_selected',
                 updated_at = ?{} WHERE id = ?",
                    billing_update
                ),
                rusqlite::params![
                    payment.payment_method,
                    payment.payment_token,
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn apply_discount(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        // In a real implementation, this would validate the coupon
        // and calculate the discount amount
        // For now, we'll just store the coupon code
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET coupon_code = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![coupon_code, Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn remove_discount(&self, id: Uuid) -> Result<Cart> {
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

    fn mark_ready_for_payment(&self, id: Uuid) -> Result<Cart> {
        let cart = self.get(id)?
            .ok_or_else(|| CommerceError::NotFound)?;

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

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn begin_checkout(&self, id: Uuid) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'payment_pending', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn complete(&self, id: Uuid) -> Result<CheckoutResult> {
        let cart = self.get(id)?
            .ok_or_else(|| CommerceError::NotFound)?;

        let conn = self.conn()?;
        let now = Utc::now();
        let order_id = Uuid::new_v4();
        let order_number = format!(
            "ORD-{}-{:04}",
            now.timestamp(),
            (Uuid::new_v4().as_u128() % 10000) as u32
        );

        conn.execute(
            "UPDATE carts SET status = 'completed', order_id = ?, order_number = ?,
             payment_status = 'captured', completed_at = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                order_id.to_string(),
                &order_number,
                now.to_rfc3339(),
                now.to_rfc3339(),
                id.to_string()
            ],
        )
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

    fn cancel(&self, id: Uuid) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'cancelled', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn abandon(&self, id: Uuid) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'abandoned', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn expire(&self, id: Uuid) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET status = 'expired', updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn reserve_inventory(&self, id: Uuid) -> Result<Cart> {
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

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn release_inventory(&self, id: Uuid) -> Result<Cart> {
        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE carts SET inventory_reserved = 0, reservation_expires_at = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn recalculate(&self, id: Uuid) -> Result<Cart> {
        {
            let conn = self.conn()?;
            self.update_cart_totals(&conn, id)?;
        }

        self.get(id)?.ok_or_else(|| CommerceError::NotFound)
    }

    fn set_tax(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
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
        self.list(CartFilter {
            status: Some(CartStatus::Abandoned),
            ..Default::default()
        })
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

        self.list(CartFilter {
            status: Some(CartStatus::Expired),
            ..Default::default()
        })
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

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }
}

// Internal helper methods
impl SqliteCartRepository {
    fn add_item_internal(
        &self,
        conn: &rusqlite::Connection,
        cart_id: Uuid,
        item: AddCartItem,
    ) -> Result<CartItem> {
        let item_id = Uuid::new_v4();
        let now = Utc::now();
        let requires_shipping = item.requires_shipping.unwrap_or(true);

        let total = CartItem::calculate_total(
            item.quantity,
            item.unit_price,
            Decimal::ZERO,
            Decimal::ZERO,
        );

        let metadata_json = item
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

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
                if requires_shipping { 1 } else { 0 },
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
