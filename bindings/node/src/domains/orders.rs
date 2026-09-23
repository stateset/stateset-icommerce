//! Orders API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Orders API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateOrderItemInput {
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    /// Float unit price. Optional: send `unit_price_exact` instead for exact money.
    pub unit_price: Option<f64>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
}

/// Exact-money order item input. Monetary values are base-10 strings.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateOrderItemExactInput {
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: String,
    pub tax_amount: Option<String>,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct OrderAddressInput {
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
}

pub(crate) fn input_to_order_address(input: OrderAddressInput) -> stateset_core::Address {
    stateset_core::Address {
        line1: input.line1,
        line2: input.line2,
        city: input.city,
        state: input.state,
        postal_code: input.postal_code,
        country: input.country,
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateOrderInput {
    pub customer_id: String,
    pub cart_id: Option<String>,
    pub items: Vec<CreateOrderItemInput>,
    pub currency: Option<String>,
    pub notes: Option<String>,
    /// Defaults to `allow_backorder`.
    #[napi(ts_type = "StockPolicy")]
    pub stock_policy: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_address: Option<OrderAddressInput>,
    pub billing_address: Option<OrderAddressInput>,
}

/// Exact-money order input. Prefer this for financial and agent integrations.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateOrderExactInput {
    pub customer_id: String,
    pub cart_id: Option<String>,
    pub items: Vec<CreateOrderItemExactInput>,
    pub currency: Option<String>,
    pub notes: Option<String>,
    /// Defaults to `allow_backorder`.
    #[napi(ts_type = "StockPolicy")]
    pub stock_policy: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_address: Option<OrderAddressInput>,
    pub billing_address: Option<OrderAddressInput>,
}

pub(crate) fn parse_stock_policy(value: Option<String>) -> Result<stateset_core::StockPolicy> {
    match value.as_deref().map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        None | Some("") | Some("allow_backorder") | Some("allow-backorder") => {
            Ok(stateset_core::StockPolicy::AllowBackorder)
        }
        Some("reject_if_insufficient") | Some("reject-if-insufficient") => {
            Ok(stateset_core::StockPolicy::RejectIfInsufficient)
        }
        Some(other) => Err(coded(
            ErrCode::Validation,
            format!(
                "Invalid stock policy '{other}'; expected allow_backorder or reject_if_insufficient"
            ),
        )),
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct OrderItemOutput {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    /// @deprecated Use the `unitPriceExact` twin; float money will be removed in 2.0.
    pub unit_price: f64,
    /// Exact base-10 unit price. Prefer this field for calculations.
    pub unit_price_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 line total. Prefer this field for calculations.
    pub total_exact: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct OrderAddressOutput {
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
}

impl From<stateset_core::Address> for OrderAddressOutput {
    fn from(address: stateset_core::Address) -> Self {
        Self {
            line1: address.line1,
            line2: address.line2,
            city: address.city,
            state: address.state,
            postal_code: address.postal_code,
            country: address.country,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct OrderOutput {
    pub id: String,
    pub order_number: String,
    pub customer_id: String,
    #[napi(ts_type = "OrderStatus")]
    pub status: String,
    /// @deprecated Use the `totalAmountExact` twin; float money will be removed in 2.0.
    pub total_amount: f64,
    /// Exact base-10 order total. Prefer this field for calculations.
    pub total_amount_exact: String,
    pub currency: String,
    #[napi(ts_type = "PaymentStatus")]
    pub payment_status: String,
    #[napi(ts_type = "FulfillmentStatus")]
    pub fulfillment_status: String,
    pub tracking_number: Option<String>,
    pub shipping_method: Option<String>,
    pub notes: Option<String>,
    pub shipping_address: Option<OrderAddressOutput>,
    pub billing_address: Option<OrderAddressOutput>,
    pub items: Vec<OrderItemOutput>,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Order> for OrderOutput {
    type Error = Error;

    fn try_from(o: stateset_core::Order) -> Result<Self> {
        let (total_amount, total_amount_exact) = money_pair(o.total_amount, "order total amount")?;
        Ok(Self {
            id: o.id.to_string(),
            order_number: o.order_number,
            customer_id: o.customer_id.to_string(),
            status: format!("{}", o.status),
            total_amount,
            total_amount_exact,
            currency: o.currency.to_string(),
            payment_status: format!("{}", o.payment_status),
            fulfillment_status: format!("{}", o.fulfillment_status),
            tracking_number: o.tracking_number,
            shipping_method: o.shipping_method,
            notes: o.notes,
            shipping_address: o.shipping_address.map(Into::into),
            billing_address: o.billing_address.map(Into::into),
            items: o
                .items
                .into_iter()
                .map(|i| {
                    let (unit_price, unit_price_exact) =
                        money_pair(i.unit_price, "order item unit price")?;
                    let (total, total_exact) = money_pair(i.total, "order item total")?;
                    Ok(OrderItemOutput {
                        id: i.id.to_string(),
                        sku: i.sku,
                        name: i.name,
                        quantity: i.quantity,
                        unit_price,
                        unit_price_exact,
                        total,
                        total_exact,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            version: o.version,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        })
    }
}

#[napi]
pub struct Orders {
    pub(crate) commerce: Handle,
}

#[napi]
impl Orders {
    #[napi]
    pub async fn create(&self, input: CreateOrderInput) -> Result<OrderOutput> {
        let commerce = self.commerce.get()?;

        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let stock_policy = parse_stock_policy(input.stock_policy)?;
        let shipping_address = input.shipping_address.map(input_to_order_address);
        let billing_address = input.billing_address.map(input_to_order_address);

        let items: Vec<stateset_core::CreateOrderItem> = input
            .items
            .into_iter()
            .map(|i| {
                let product_id = parse_optional_id(i.product_id, "product")?.unwrap_or_default();
                let variant_id = parse_optional_id(i.variant_id, "variant")?;

                Ok(stateset_core::CreateOrderItem {
                    product_id,
                    variant_id,
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: money_input(
                        i.unit_price_exact.as_deref(),
                        i.unit_price,
                        "order item unit price",
                    )?,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let create = stateset_core::CreateOrder {
            customer_id,
            items,
            currency: parse_optional_currency(input.currency)?,
            notes: input.notes,
            stock_policy,
            shipping_method: input.shipping_method,
            shipping_address,
            billing_address,
            ..Default::default()
        };
        let order = match input.cart_id {
            Some(cart_id) => commerce.orders().create_from_cart(
                cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?,
                create,
            ),
            None => commerce.orders().create(create),
        }
        .map_err(|e| wrap(ErrCode::Internal, "Failed to create order", e))?;

        convert_output(order)
    }

    /// Create an order without any floating-point conversion.
    #[napi]
    pub async fn create_exact(&self, input: CreateOrderExactInput) -> Result<OrderOutput> {
        let commerce = self.commerce.get()?;
        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let stock_policy = parse_stock_policy(input.stock_policy)?;
        let shipping_address = input.shipping_address.map(input_to_order_address);
        let billing_address = input.billing_address.map(input_to_order_address);
        let items = input
            .items
            .into_iter()
            .map(|i| {
                let product_id = parse_optional_id(i.product_id, "product")?.unwrap_or_default();
                let variant_id = parse_optional_id(i.variant_id, "variant")?;
                Ok(stateset_core::CreateOrderItem {
                    product_id,
                    variant_id,
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: parse_decimal_str(&i.unit_price, "order item unit price")?,
                    tax_amount: i
                        .tax_amount
                        .as_deref()
                        .map(|value| parse_decimal_str(value, "order item tax amount"))
                        .transpose()?,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let create = stateset_core::CreateOrder {
            customer_id,
            items,
            currency: parse_optional_currency(input.currency)?,
            notes: input.notes,
            stock_policy,
            shipping_method: input.shipping_method,
            shipping_address,
            billing_address,
            ..Default::default()
        };
        let order = match input.cart_id {
            Some(cart_id) => commerce.orders().create_from_cart(
                cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?,
                create,
            ),
            None => commerce.orders().create(create),
        }
        .map_err(|e| wrap(ErrCode::Internal, "Failed to create order", e))?;
        convert_output(order)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<OrderOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let order = commerce
            .orders()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get order", e))?;

        convert_optional_output(order)
    }

    /// List orders, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every order).
    #[napi]
    pub async fn list(&self, filter: Option<OrderFilterInput>) -> Result<Vec<OrderOutput>> {
        let commerce = self.commerce.get()?;
        let filter = order_filter_from_input(filter)?;
        let orders = commerce
            .orders()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list orders", e))?;

        convert_outputs(orders)
    }

    #[napi(ts_args_type = "id: string, status: OrderStatusUpdate")]
    pub async fn update_status(&self, id: String, status: String) -> Result<OrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let order_status = match status.to_lowercase().as_str() {
            "pending" => stateset_core::OrderStatus::Pending,
            "confirmed" => stateset_core::OrderStatus::Confirmed,
            "processing" => stateset_core::OrderStatus::Processing,
            "shipped" => stateset_core::OrderStatus::Shipped,
            "delivered" => stateset_core::OrderStatus::Delivered,
            "cancelled" => stateset_core::OrderStatus::Cancelled,
            "refunded" => stateset_core::OrderStatus::Refunded,
            _ => return Err(wrap(ErrCode::Validation, "Invalid status", status)),
        };

        let order = commerce
            .orders()
            .update_status(uuid.into(), order_status)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update order", e))?;

        convert_output(order)
    }

    #[napi]
    pub async fn ship(&self, id: String, tracking_number: Option<String>) -> Result<OrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let order = commerce
            .orders()
            .ship(uuid.into(), tracking_number.as_deref())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to ship order", e))?;

        convert_output(order)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<OrderOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let order = commerce
            .orders()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel order", e))?;

        convert_output(order)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .orders()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count orders", e))?;

        Ok(count as u32)
    }
}
