//! Carts/Checkout API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Carts/Checkout API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CartAddressInput {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AddCartItemInput {
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub quantity: i32,
    /// Float unit price. Optional: send `unit_price_exact` instead for exact money.
    pub unit_price: Option<f64>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
    pub original_price: Option<f64>,
    /// Exact base-10 original price. Takes precedence over `original_price` when present.
    pub original_price_exact: Option<String>,
    pub weight: Option<f64>,
    pub requires_shipping: Option<bool>,
}

/// Exact-money cart item input. Monetary values are base-10 strings.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AddCartItemExactInput {
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub quantity: i32,
    pub unit_price: String,
    pub original_price: Option<String>,
    pub weight: Option<String>,
    pub requires_shipping: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCartInput {
    pub customer_id: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub currency: Option<String>,
    pub shipping_address: Option<CartAddressInput>,
    pub billing_address: Option<CartAddressInput>,
    pub notes: Option<String>,
    pub expires_in_minutes: Option<i64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCartInput {
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_name: Option<String>,
    pub shipping_method: Option<String>,
    pub coupon_code: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCartItemInput {
    pub quantity: Option<i32>,
    pub unit_price: Option<f64>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SetCartPaymentInput {
    pub payment_method: String,
    pub payment_token: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SetCartShippingInput {
    pub shipping_address: CartAddressInput,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    pub shipping_amount: Option<f64>,
    /// Exact base-10 shipping amount. Takes precedence over `shipping_amount` when present.
    pub shipping_amount_exact: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CartItemOutput {
    pub id: String,
    pub cart_id: String,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub quantity: i32,
    /// @deprecated Use the `unitPriceExact` twin; float money will be removed in 2.0.
    pub unit_price: f64,
    pub unit_price_exact: String,
    /// @deprecated Use the `originalPriceExact` twin; float money will be removed in 2.0.
    pub original_price: Option<f64>,
    pub original_price_exact: Option<String>,
    /// @deprecated Use the `discountAmountExact` twin; float money will be removed in 2.0.
    pub discount_amount: f64,
    pub discount_amount_exact: String,
    /// @deprecated Use the `taxAmountExact` twin; float money will be removed in 2.0.
    pub tax_amount: f64,
    pub tax_amount_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    pub total_exact: String,
    pub requires_shipping: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::CartItem> for CartItemOutput {
    type Error = Error;

    fn try_from(item: stateset_core::CartItem) -> Result<Self> {
        let (unit_price, unit_price_exact) = money_pair(item.unit_price, "cart item unit price")?;
        let (original_price, original_price_exact) =
            optional_money_pair(item.original_price, "cart item original price")?;
        let (discount_amount, discount_amount_exact) =
            money_pair(item.discount_amount, "cart item discount amount")?;
        let (tax_amount, tax_amount_exact) = money_pair(item.tax_amount, "cart item tax amount")?;
        let (total, total_exact) = money_pair(item.total, "cart item total")?;
        Ok(Self {
            id: item.id.to_string(),
            cart_id: item.cart_id.to_string(),
            product_id: item.product_id.map(|id| id.to_string()),
            variant_id: item.variant_id.map(|id| id.to_string()),
            sku: item.sku,
            name: item.name,
            description: item.description,
            image_url: item.image_url,
            quantity: item.quantity,
            unit_price,
            unit_price_exact,
            original_price,
            original_price_exact,
            discount_amount,
            discount_amount_exact,
            tax_amount,
            tax_amount_exact,
            total,
            total_exact,
            requires_shipping: item.requires_shipping,
            created_at: item.created_at.to_rfc3339(),
            updated_at: item.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CartAddressOutput {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub email: Option<String>,
}

impl From<stateset_core::CartAddress> for CartAddressOutput {
    fn from(addr: stateset_core::CartAddress) -> Self {
        Self {
            first_name: addr.first_name,
            last_name: addr.last_name,
            company: addr.company,
            line1: addr.line1,
            line2: addr.line2,
            city: addr.city,
            state: addr.state,
            postal_code: addr.postal_code,
            country: addr.country,
            phone: addr.phone,
            email: addr.email,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CartOutput {
    pub id: String,
    pub cart_number: String,
    pub customer_id: Option<String>,
    #[napi(ts_type = "CartStatus")]
    pub status: String,
    pub currency: String,
    /// @deprecated Use the `subtotalExact` twin; float money will be removed in 2.0.
    pub subtotal: f64,
    pub subtotal_exact: String,
    /// @deprecated Use the `taxAmountExact` twin; float money will be removed in 2.0.
    pub tax_amount: f64,
    pub tax_amount_exact: String,
    /// @deprecated Use the `shippingAmountExact` twin; float money will be removed in 2.0.
    pub shipping_amount: f64,
    pub shipping_amount_exact: String,
    /// @deprecated Use the `discountAmountExact` twin; float money will be removed in 2.0.
    pub discount_amount: f64,
    pub discount_amount_exact: String,
    /// @deprecated Use the `grandTotalExact` twin; float money will be removed in 2.0.
    pub grand_total: f64,
    pub grand_total_exact: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_name: Option<String>,
    pub shipping_address: Option<CartAddressOutput>,
    pub billing_address: Option<CartAddressOutput>,
    pub billing_same_as_shipping: bool,
    #[napi(ts_type = "FulfillmentType")]
    pub fulfillment_type: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    pub payment_method: Option<String>,
    #[napi(ts_type = "CartPaymentStatus")]
    pub payment_status: String,
    pub coupon_code: Option<String>,
    pub order_id: Option<String>,
    pub order_number: Option<String>,
    pub inventory_reserved: bool,
    pub item_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Cart> for CartOutput {
    type Error = Error;

    fn try_from(cart: stateset_core::Cart) -> Result<Self> {
        // Compute item_count first before any fields are moved
        let item_count = cart.item_count();
        let (subtotal, subtotal_exact) = money_pair(cart.subtotal, "cart subtotal")?;
        let (tax_amount, tax_amount_exact) = money_pair(cart.tax_amount, "cart tax amount")?;
        let (shipping_amount, shipping_amount_exact) =
            money_pair(cart.shipping_amount, "cart shipping amount")?;
        let (discount_amount, discount_amount_exact) =
            money_pair(cart.discount_amount, "cart discount amount")?;
        let (grand_total, grand_total_exact) = money_pair(cart.grand_total, "cart grand total")?;
        Ok(Self {
            id: cart.id.to_string(),
            cart_number: cart.cart_number,
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: format!("{}", cart.status),
            currency: cart.currency.to_string(),
            subtotal,
            subtotal_exact,
            tax_amount,
            tax_amount_exact,
            shipping_amount,
            shipping_amount_exact,
            discount_amount,
            discount_amount_exact,
            grand_total,
            grand_total_exact,
            customer_email: cart.customer_email,
            customer_phone: cart.customer_phone,
            customer_name: cart.customer_name,
            shipping_address: cart.shipping_address.map(|a| a.into()),
            billing_address: cart.billing_address.map(|a| a.into()),
            billing_same_as_shipping: cart.billing_same_as_shipping,
            fulfillment_type: cart.fulfillment_type.map(|ft| format!("{}", ft)),
            shipping_method: cart.shipping_method,
            shipping_carrier: cart.shipping_carrier,
            payment_method: cart.payment_method,
            payment_status: format!("{}", cart.payment_status),
            coupon_code: cart.coupon_code,
            order_id: cart.order_id.map(|id| id.to_string()),
            order_number: cart.order_number,
            inventory_reserved: cart.inventory_reserved,
            item_count,
            created_at: cart.created_at.to_rfc3339(),
            updated_at: cart.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CheckoutResultOutput {
    pub cart_id: String,
    pub order_id: String,
    pub order_number: String,
    pub payment_id: Option<String>,
    /// @deprecated Use the `totalChargedExact` twin; float money will be removed in 2.0.
    pub total_charged: f64,
    pub total_charged_exact: String,
    pub currency: String,
}

impl TryFrom<stateset_core::CheckoutResult> for CheckoutResultOutput {
    type Error = Error;

    fn try_from(result: stateset_core::CheckoutResult) -> Result<Self> {
        let (total_charged, total_charged_exact) =
            money_pair(result.total_charged, "checkout total charged")?;
        Ok(Self {
            cart_id: result.cart_id.to_string(),
            order_id: result.order_id.to_string(),
            order_number: result.order_number,
            payment_id: result.payment_id.map(|id| id.to_string()),
            total_charged,
            total_charged_exact,
            currency: result.currency.to_string(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingRateOutput {
    pub id: String,
    pub carrier: String,
    pub service: String,
    pub description: Option<String>,
    /// @deprecated Use the `priceExact` twin; float money will be removed in 2.0.
    pub price: f64,
    /// Exact base-10 price, straight from the engine's `Decimal`. Prefer this field for money.
    pub price_exact: String,
    pub currency: String,
    pub estimated_days: Option<i32>,
}

impl TryFrom<stateset_core::ShippingRate> for ShippingRateOutput {
    type Error = Error;

    fn try_from(rate: stateset_core::ShippingRate) -> Result<Self> {
        let (price, price_exact) = money_pair(rate.price, "shipping rate price")?;
        Ok(Self {
            id: rate.id,
            carrier: rate.carrier,
            service: rate.service,
            description: rate.description,
            price,
            price_exact,
            currency: rate.currency.to_string(),
            estimated_days: rate.estimated_days,
        })
    }
}

pub(crate) fn input_to_cart_address(input: CartAddressInput) -> stateset_core::CartAddress {
    stateset_core::CartAddress {
        first_name: input.first_name,
        last_name: input.last_name,
        company: input.company,
        line1: input.line1,
        line2: input.line2,
        city: input.city,
        state: input.state,
        postal_code: input.postal_code,
        country: input.country,
        phone: input.phone,
        email: input.email,
    }
}

#[napi]
pub struct Carts {
    pub(crate) commerce: Handle,
}

#[napi]
impl Carts {
    /// Create a new cart
    #[napi]
    pub async fn create(&self, input: CreateCartInput) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;

        let customer_id = input
            .customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;

        let cart = commerce
            .carts()
            .create(stateset_core::CreateCart {
                customer_id,
                customer_email: input.customer_email,
                customer_name: input.customer_name,
                currency: parse_optional_currency(input.currency)?,
                shipping_address: input.shipping_address.map(input_to_cart_address),
                billing_address: input.billing_address.map(input_to_cart_address),
                notes: input.notes,
                expires_in_minutes: input.expires_in_minutes,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create cart", e))?;

        convert_output(cart)
    }

    /// Get a cart by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CartOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get cart", e))?;

        convert_optional_output(cart)
    }

    /// Get a cart by cart number
    #[napi]
    pub async fn get_by_number(&self, cart_number: String) -> Result<Option<CartOutput>> {
        let commerce = self.commerce.get()?;

        let cart = commerce
            .carts()
            .get_by_number(&cart_number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get cart", e))?;

        convert_optional_output(cart)
    }

    /// Update a cart
    #[napi]
    pub async fn update(&self, id: String, input: UpdateCartInput) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .update(
                uuid.into(),
                stateset_core::UpdateCart {
                    customer_email: input.customer_email,
                    customer_phone: input.customer_phone,
                    customer_name: input.customer_name,
                    shipping_method: input.shipping_method,
                    coupon_code: input.coupon_code,
                    notes: input.notes,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update cart", e))?;

        convert_output(cart)
    }

    /// List carts, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every cart).
    #[napi]
    pub async fn list(&self, filter: Option<CartFilterInput>) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.get()?;
        let filter = cart_filter_from_input(filter)?;
        let carts = commerce
            .carts()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list carts", e))?;

        convert_outputs(carts)
    }

    /// List carts for a customer
    #[napi]
    pub async fn for_customer(&self, customer_id: String) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;

        let carts = commerce
            .carts()
            .for_customer(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get customer carts", e))?;

        convert_outputs(carts)
    }

    /// Delete a cart
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        commerce
            .carts()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete cart", e))?;

        Ok(())
    }

    /// Add an item to the cart
    #[napi]
    pub async fn add_item(
        &self,
        cart_id: String,
        item: AddCartItemInput,
    ) -> Result<CartItemOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?;

        let product_id = item
            .product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;

        let variant_id = item
            .variant_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid variant UUID"))?;

        let cart_item = commerce
            .carts()
            .add_item(
                uuid.into(),
                stateset_core::AddCartItem {
                    product_id,
                    variant_id,
                    sku: item.sku,
                    name: item.name,
                    description: item.description,
                    image_url: item.image_url,
                    quantity: item.quantity,
                    unit_price: money_input(
                        item.unit_price_exact.as_deref(),
                        item.unit_price,
                        "cart item unit price",
                    )?,
                    original_price: optional_money_input(
                        item.original_price_exact.as_deref(),
                        item.original_price,
                        "cart item original price",
                    )?,
                    weight: optional_decimal_from_f64(item.weight, "cart item weight")?,
                    requires_shipping: item.requires_shipping,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add item", e))?;

        convert_output(cart_item)
    }

    /// Add a cart item without any floating-point conversion.
    #[napi]
    pub async fn add_item_exact(
        &self,
        cart_id: String,
        item: AddCartItemExactInput,
    ) -> Result<CartItemOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?;
        let product_id = item
            .product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;
        let variant_id = item
            .variant_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid variant UUID"))?;
        let cart_item = commerce
            .carts()
            .add_item(
                uuid.into(),
                stateset_core::AddCartItem {
                    product_id,
                    variant_id,
                    sku: item.sku,
                    name: item.name,
                    description: item.description,
                    image_url: item.image_url,
                    quantity: item.quantity,
                    unit_price: parse_decimal_str(&item.unit_price, "cart item unit price")?,
                    original_price: item
                        .original_price
                        .as_deref()
                        .map(|value| parse_decimal_str(value, "cart item original price"))
                        .transpose()?,
                    weight: item
                        .weight
                        .as_deref()
                        .map(|value| parse_decimal_str(value, "cart item weight"))
                        .transpose()?,
                    requires_shipping: item.requires_shipping,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add item", e))?;
        convert_output(cart_item)
    }

    /// Update a cart item
    #[napi]
    pub async fn update_item(
        &self,
        item_id: String,
        input: UpdateCartItemInput,
    ) -> Result<CartItemOutput> {
        let commerce = self.commerce.get()?;
        let uuid = item_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid item UUID"))?;

        let cart_item = commerce
            .carts()
            .update_item(
                uuid,
                stateset_core::UpdateCartItem {
                    quantity: input.quantity,
                    unit_price: optional_money_input(
                        input.unit_price_exact.as_deref(),
                        input.unit_price,
                        "cart item unit price",
                    )?,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update item", e))?;

        convert_output(cart_item)
    }

    /// Remove an item from the cart
    #[napi]
    pub async fn remove_item(&self, item_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = item_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid item UUID"))?;

        commerce
            .carts()
            .remove_item(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to remove item", e))?;

        Ok(())
    }

    /// Get items in a cart
    #[napi]
    pub async fn get_items(&self, cart_id: String) -> Result<Vec<CartItemOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?;

        let items = commerce
            .carts()
            .get_items(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get items", e))?;

        convert_outputs(items)
    }

    /// Clear all items from the cart
    #[napi]
    pub async fn clear_items(&self, cart_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?;

        commerce
            .carts()
            .clear_items(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to clear items", e))?;

        Ok(())
    }

    /// Set the shipping address
    #[napi]
    pub async fn set_shipping_address(
        &self,
        id: String,
        address: CartAddressInput,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_shipping_address(uuid.into(), input_to_cart_address(address))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set shipping address", e))?;

        convert_output(cart)
    }

    /// Set shipping selection (address + method/carrier/amount)
    #[napi]
    pub async fn set_shipping(
        &self,
        id: String,
        input: SetCartShippingInput,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let shipping_amount = optional_money_input(
            input.shipping_amount_exact.as_deref(),
            input.shipping_amount,
            "cart shipping amount",
        )?;

        let cart = commerce
            .carts()
            .set_shipping(
                uuid.into(),
                stateset_core::SetCartShipping {
                    shipping_address: input_to_cart_address(input.shipping_address),
                    shipping_method: input.shipping_method,
                    shipping_carrier: input.shipping_carrier,
                    shipping_amount,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set shipping", e))?;

        convert_output(cart)
    }

    /// Set the billing address
    #[napi]
    pub async fn set_billing_address(
        &self,
        id: String,
        address: CartAddressInput,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_billing_address(uuid.into(), input_to_cart_address(address))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set billing address", e))?;

        convert_output(cart)
    }

    /// Get available shipping rates
    #[napi]
    pub async fn get_shipping_rates(&self, id: String) -> Result<Vec<ShippingRateOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let rates = commerce
            .carts()
            .get_shipping_rates(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get shipping rates", e))?;

        convert_outputs(rates)
    }

    /// Set payment method
    #[napi]
    pub async fn set_payment(&self, id: String, input: SetCartPaymentInput) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_payment(
                uuid.into(),
                stateset_core::SetCartPayment {
                    payment_method: input.payment_method,
                    payment_token: input.payment_token,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set payment", e))?;

        convert_output(cart)
    }

    /// Apply a discount/coupon code
    #[napi]
    pub async fn apply_discount(&self, id: String, coupon_code: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .apply_discount(uuid.into(), &coupon_code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to apply discount", e))?;

        convert_output(cart)
    }

    /// Remove discount from cart
    #[napi]
    pub async fn remove_discount(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .remove_discount(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to remove discount", e))?;

        convert_output(cart)
    }

    /// Mark cart as ready for payment
    #[napi]
    pub async fn mark_ready_for_payment(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .mark_ready_for_payment(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark ready", e))?;

        convert_output(cart)
    }

    /// Begin checkout process
    #[napi]
    pub async fn begin_checkout(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .begin_checkout(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to begin checkout", e))?;

        convert_output(cart)
    }

    /// Complete checkout and create order
    #[napi]
    pub async fn complete(&self, id: String) -> Result<CheckoutResultOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let result = commerce
            .carts()
            .complete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete checkout", e))?;

        convert_output(result)
    }

    /// Cancel a cart
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel cart", e))?;

        convert_output(cart)
    }

    /// Mark cart as abandoned
    #[napi]
    pub async fn abandon(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .abandon(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to abandon cart", e))?;

        convert_output(cart)
    }

    /// Mark cart as expired
    #[napi]
    pub async fn expire(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .expire(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to expire cart", e))?;

        convert_output(cart)
    }

    /// Reserve inventory for cart items
    #[napi]
    pub async fn reserve_inventory(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .reserve_inventory(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reserve inventory", e))?;

        convert_output(cart)
    }

    /// Release reserved inventory for cart items
    #[napi]
    pub async fn release_inventory(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .release_inventory(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to release inventory", e))?;

        convert_output(cart)
    }

    /// Recalculate cart totals
    #[napi]
    pub async fn recalculate(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .recalculate(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to recalculate", e))?;

        convert_output(cart)
    }

    /// Set tax amount.
    ///
    /// `tax_amount_exact` is the exact base-10 form and wins when present; the
    /// `f64` is what callers sent before it existed and still works alone.
    #[napi]
    pub async fn set_tax(
        &self,
        id: String,
        tax_amount: Option<f64>,
        tax_amount_exact: Option<String>,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_tax(
                uuid.into(),
                money_input(tax_amount_exact.as_deref(), tax_amount, "cart tax amount")?,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set tax", e))?;

        convert_output(cart)
    }

    /// Get abandoned carts
    #[napi]
    pub async fn get_abandoned(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.get()?;
        let carts = commerce
            .carts()
            .get_abandoned()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get abandoned carts", e))?;

        convert_outputs(carts)
    }

    /// Get expired carts
    #[napi]
    pub async fn get_expired(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.get()?;
        let carts = commerce
            .carts()
            .get_expired()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get expired carts", e))?;

        convert_outputs(carts)
    }

    /// Count carts
    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .carts()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count carts", e))?;

        Ok(count as u32)
    }
}
