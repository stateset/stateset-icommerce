#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{CartId, CustomerId, OrderId, ProductId, PromotionId, SubscriptionId};
use stateset_embedded::Commerce as RustCommerce;
use stateset_embedded::CurrencyCode;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

fn to_f64_or_nan<T>(value: T) -> f64
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    match value.try_into() {
        Ok(converted) => converted,
        Err(err) => {
            eprintln!("stateset-embedded: failed to convert to f64: {}", err);
            f64::NAN
        }
    }
}

fn to_f64_result<T>(value: T, field: &str) -> Result<f64>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    value
        .try_into()
        .map_err(|err| Error::from_reason(format!("Failed to convert {field} to f64: {err}")))
}

fn optional_to_f64_result<T>(value: Option<T>, field: &str) -> Result<Option<f64>>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    value.map(|inner| to_f64_result(inner, field)).transpose()
}

fn convert_output<T, U>(value: T) -> Result<U>
where
    U: TryFrom<T, Error = Error>,
{
    U::try_from(value)
}

fn convert_optional_output<T, U>(value: Option<T>) -> Result<Option<U>>
where
    U: TryFrom<T, Error = Error>,
{
    value.map(convert_output).transpose()
}

fn convert_outputs<T, U>(values: Vec<T>) -> Result<Vec<U>>
where
    U: TryFrom<T, Error = Error>,
{
    values.into_iter().map(convert_output).collect()
}

fn decimal_from_f64(value: f64, field: &str) -> Result<Decimal> {
    Decimal::from_f64_retain(value).ok_or_else(|| Error::from_reason(format!("Invalid {field}")))
}

fn optional_decimal_from_f64(value: Option<f64>, field: &str) -> Result<Option<Decimal>> {
    value.map(|value| decimal_from_f64(value, field)).transpose()
}

/// JavaScript-friendly Commerce instance
#[napi]
pub struct Commerce {
    inner: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Commerce {
    /// Create a new Commerce instance with a database path
    /// Use ":memory:" for an in-memory database
    #[napi(constructor)]
    pub fn new(db_path: String) -> Result<Self> {
        let commerce = RustCommerce::new(&db_path)
            .map_err(|e| Error::from_reason(format!("Failed to initialize commerce: {}", e)))?;

        Ok(Self { inner: Arc::new(Mutex::new(commerce)) })
    }

    /// Get the customers API
    #[napi(getter)]
    pub fn customers(&self) -> Customers {
        Customers { commerce: self.inner.clone() }
    }

    /// Get the orders API
    #[napi(getter)]
    pub fn orders(&self) -> Orders {
        Orders { commerce: self.inner.clone() }
    }

    /// Get the products API
    #[napi(getter)]
    pub fn products(&self) -> Products {
        Products { commerce: self.inner.clone() }
    }

    /// Get the custom objects API (custom states / metaobjects)
    #[napi(getter)]
    pub fn custom_objects(&self) -> CustomObjects {
        CustomObjects { commerce: self.inner.clone() }
    }

    /// Alias for `custom_objects` (for users who prefer the "custom states" name)
    #[napi(getter)]
    pub fn custom_states(&self) -> CustomObjects {
        self.custom_objects()
    }

    /// Get the inventory API
    #[napi(getter)]
    pub fn inventory(&self) -> Inventory {
        Inventory { commerce: self.inner.clone() }
    }

    /// Get the returns API
    #[napi(getter)]
    pub fn returns(&self) -> Returns {
        Returns { commerce: self.inner.clone() }
    }

    /// Get the payments API
    #[napi(getter)]
    pub fn payments(&self) -> Payments {
        Payments { commerce: self.inner.clone() }
    }

    /// Get the x402 payment protocol API
    #[napi(getter)]
    pub fn x402(&self) -> X402 {
        X402 { commerce: self.inner.clone() }
    }

    /// Get the shipments API
    #[napi(getter)]
    pub fn shipments(&self) -> Shipments {
        Shipments { commerce: self.inner.clone() }
    }

    /// Get the warranties API
    #[napi(getter)]
    pub fn warranties(&self) -> Warranties {
        Warranties { commerce: self.inner.clone() }
    }

    /// Get the purchase orders API
    #[napi(getter)]
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders { commerce: self.inner.clone() }
    }

    /// Get the invoices API
    #[napi(getter)]
    pub fn invoices(&self) -> Invoices {
        Invoices { commerce: self.inner.clone() }
    }

    /// Get the bill of materials API
    #[napi(getter)]
    pub fn bom(&self) -> Bom {
        Bom { commerce: self.inner.clone() }
    }

    /// Get the work orders API
    #[napi(getter)]
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders { commerce: self.inner.clone() }
    }

    /// Get the carts/checkout API
    #[napi(getter)]
    pub fn carts(&self) -> Carts {
        Carts { commerce: self.inner.clone() }
    }

    /// Get the analytics API
    #[napi(getter)]
    pub fn analytics(&self) -> Analytics {
        Analytics { commerce: self.inner.clone() }
    }

    /// Get the currency API
    #[napi(getter)]
    pub fn currency(&self) -> CurrencyOperations {
        CurrencyOperations { commerce: self.inner.clone() }
    }

    /// Get the subscriptions API
    #[napi(getter)]
    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions { commerce: self.inner.clone() }
    }

    /// Get the promotions API
    #[napi(getter)]
    pub fn promotions(&self) -> Promotions {
        Promotions { commerce: self.inner.clone() }
    }

    /// Get the tax API
    #[napi(getter)]
    pub fn tax(&self) -> Tax {
        Tax { commerce: self.inner.clone() }
    }

    /// Get the quality control API
    #[napi(getter)]
    pub fn quality(&self) -> Quality {
        Quality { commerce: self.inner.clone() }
    }

    /// Get the lot/batch tracking API
    #[napi(getter)]
    pub fn lots(&self) -> Lots {
        Lots { commerce: self.inner.clone() }
    }

    /// Get the serial number API
    #[napi(getter)]
    pub fn serials(&self) -> Serials {
        Serials { commerce: self.inner.clone() }
    }

    /// Get the warehouse API
    #[napi(getter)]
    pub fn warehouse(&self) -> Warehouse {
        Warehouse { commerce: self.inner.clone() }
    }

    /// Get the receiving API
    #[napi(getter)]
    pub fn receiving(&self) -> Receiving {
        Receiving { commerce: self.inner.clone() }
    }

    /// Get the fulfillment API
    #[napi(getter)]
    pub fn fulfillment(&self) -> Fulfillment {
        Fulfillment { commerce: self.inner.clone() }
    }

    /// Get the accounts payable API
    #[napi(getter)]
    pub fn accounts_payable(&self) -> AccountsPayable {
        AccountsPayable { commerce: self.inner.clone() }
    }

    /// Get the accounts receivable API
    #[napi(getter)]
    pub fn accounts_receivable(&self) -> AccountsReceivable {
        AccountsReceivable { commerce: self.inner.clone() }
    }

    /// Get the cost accounting API
    #[napi(getter)]
    pub fn cost_accounting(&self) -> CostAccounting {
        CostAccounting { commerce: self.inner.clone() }
    }

    /// Get the credit management API
    #[napi(getter)]
    pub fn credit(&self) -> Credit {
        Credit { commerce: self.inner.clone() }
    }

    /// Get the backorder management API
    #[napi(getter)]
    pub fn backorder(&self) -> Backorders {
        Backorders { commerce: self.inner.clone() }
    }

    /// Get the general ledger API
    #[napi(getter)]
    pub fn general_ledger(&self) -> GeneralLedger {
        GeneralLedger { commerce: self.inner.clone() }
    }

    /// Get the events API (pub/sub and webhook management)
    #[napi(getter)]
    pub fn events(&self) -> Events {
        Events { commerce: self.inner.clone() }
    }

    /// Create a vector search instance with the given OpenAI API key
    ///
    /// Vector search enables semantic similarity search across products,
    /// customers, orders, and inventory items using OpenAI embeddings.
    #[napi]
    pub fn vector(&self, api_key: String) -> VectorSearch {
        VectorSearch { commerce: self.inner.clone(), api_key }
    }
}

// ============================================================================
// Events API
// ============================================================================

#[napi(object)]
pub struct CreateWebhookInput {
    /// Display name
    pub name: Option<String>,
    /// Target URL for POST requests
    pub url: String,
    /// Optional secret for HMAC signature
    pub secret: Option<String>,
    /// Event types to receive (empty or omitted = all events)
    pub event_types: Option<Vec<String>>,
}

#[napi(object)]
pub struct WebhookOutput {
    pub id: String,
    pub name: String,
    pub url: String,
    pub has_secret: bool,
    pub event_types: Vec<String>,
    pub active: bool,
    pub created_at: String,
}

impl From<stateset_embedded::Webhook> for WebhookOutput {
    fn from(w: stateset_embedded::Webhook) -> Self {
        Self {
            id: w.id.to_string(),
            name: w.name,
            url: w.url,
            has_secret: w.secret.is_some(),
            event_types: w.event_types,
            active: w.active,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Events {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Events {
    /// Subscribe to all commerce events.
    #[napi]
    pub async fn subscribe(&self) -> Result<CommerceEventSubscription> {
        let commerce = self.commerce.lock().await;
        let subscription = commerce.subscribe_events();
        Ok(CommerceEventSubscription::new(subscription, None))
    }

    /// Subscribe to a subset of commerce events by event type.
    ///
    /// Event types must match `CommerceEvent::event_type()` values (snake_case),
    /// e.g. "order_created", "inventory_adjusted".
    #[napi]
    pub async fn subscribe_filtered(
        &self,
        event_types: Vec<String>,
    ) -> Result<CommerceEventSubscription> {
        let commerce = self.commerce.lock().await;
        let subscription = commerce.subscribe_events();

        let allowed_event_types: HashSet<String> = event_types.into_iter().collect();
        let allowed_event_types =
            if allowed_event_types.is_empty() { None } else { Some(allowed_event_types) };

        Ok(CommerceEventSubscription::new(subscription, allowed_event_types))
    }

    /// List registered webhooks.
    #[napi]
    pub async fn list_webhooks(&self) -> Result<Vec<WebhookOutput>> {
        let commerce = self.commerce.lock().await;
        let webhooks = commerce.list_webhooks();
        Ok(webhooks.into_iter().map(|w| w.into()).collect())
    }

    /// Register a webhook endpoint for event delivery.
    #[napi]
    pub async fn register_webhook(&self, input: CreateWebhookInput) -> Result<Option<String>> {
        let commerce = self.commerce.lock().await;

        let name = input.name.unwrap_or_else(|| "Webhook".to_string());
        let mut webhook = stateset_embedded::Webhook::new(name, input.url);
        if let Some(secret) = input.secret {
            webhook = webhook.with_secret(secret);
        }
        if let Some(events) = input.event_types {
            if !events.is_empty() {
                webhook = webhook.with_events(events);
            }
        }

        Ok(Some(commerce.register_webhook(webhook).to_string()))
    }

    /// Unregister a webhook endpoint.
    #[napi]
    pub async fn unregister_webhook(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;
        Ok(commerce.unregister_webhook(uuid))
    }
}

#[napi]
pub struct CommerceEventSubscription {
    inner: Mutex<stateset_embedded::EventSubscription>,
    allowed_event_types: Option<HashSet<String>>,
}

impl CommerceEventSubscription {
    fn new(
        inner: stateset_embedded::EventSubscription,
        allowed_event_types: Option<HashSet<String>>,
    ) -> Self {
        Self { inner: Mutex::new(inner), allowed_event_types }
    }
}

#[napi]
impl CommerceEventSubscription {
    /// Receive the next event (or `null` if the stream is closed).
    ///
    /// Returned events are JSON objects and include an `event_type` field (snake_case).
    #[napi]
    pub async fn recv(&self) -> Result<Option<serde_json::Value>> {
        let mut subscription = self.inner.lock().await;

        loop {
            let Some(event) = subscription.recv().await else {
                return Ok(None);
            };

            let event_type = event.event_type().to_string();
            if let Some(allowed) = &self.allowed_event_types {
                if !allowed.contains(&event_type) {
                    continue;
                }
            }

            let mut value = serde_json::to_value(&event).map_err(|e| {
                Error::from_reason(format!("Failed to serialize commerce event: {}", e))
            })?;

            // Provide a stable `event_type` field for CLI/tools (the serde tag is `type`).
            if let serde_json::Value::Object(ref mut map) = value {
                map.insert("event_type".to_string(), serde_json::Value::String(event_type));
            }

            return Ok(Some(value));
        }
    }
}

// ============================================================================
// Customers API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomerInput {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomerOutput {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub status: String,
    pub accepts_marketing: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Customer> for CustomerOutput {
    fn from(c: stateset_core::Customer) -> Self {
        Self {
            id: c.id.to_string(),
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
            phone: c.phone,
            status: format!("{}", c.status),
            accepts_marketing: c.accepts_marketing,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Customers {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Customers {
    #[napi]
    pub async fn create(&self, input: CreateCustomerInput) -> Result<CustomerOutput> {
        let commerce = self.commerce.lock().await;
        let customer = commerce
            .customers()
            .create(stateset_core::CreateCustomer {
                email: input.email,
                first_name: input.first_name,
                last_name: input.last_name,
                phone: input.phone,
                accepts_marketing: input.accepts_marketing,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create customer: {}", e)))?;

        Ok(customer.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CustomerOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let customer = commerce
            .customers()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get customer: {}", e)))?;

        Ok(customer.map(|c| c.into()))
    }

    #[napi]
    pub async fn get_by_email(&self, email: String) -> Result<Option<CustomerOutput>> {
        let commerce = self.commerce.lock().await;
        let customer = commerce
            .customers()
            .get_by_email(&email)
            .map_err(|e| Error::from_reason(format!("Failed to get customer: {}", e)))?;

        Ok(customer.map(|c| c.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<CustomerOutput>> {
        let commerce = self.commerce.lock().await;
        let customers = commerce
            .customers()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list customers: {}", e)))?;

        Ok(customers.into_iter().map(|c| c.into()).collect())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .customers()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count customers: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Orders API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateOrderItemInput {
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateOrderInput {
    pub customer_id: String,
    pub items: Vec<CreateOrderItemInput>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct OrderItemOutput {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub total: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct OrderOutput {
    pub id: String,
    pub order_number: String,
    pub customer_id: String,
    pub status: String,
    pub total_amount: f64,
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub tracking_number: Option<String>,
    pub items: Vec<OrderItemOutput>,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Order> for OrderOutput {
    type Error = Error;

    fn try_from(o: stateset_core::Order) -> Result<Self> {
        Ok(Self {
            id: o.id.to_string(),
            order_number: o.order_number,
            customer_id: o.customer_id.to_string(),
            status: format!("{}", o.status),
            total_amount: to_f64_result(o.total_amount, "order total amount")?,
            currency: o.currency.to_string(),
            payment_status: format!("{}", o.payment_status),
            fulfillment_status: format!("{}", o.fulfillment_status),
            tracking_number: o.tracking_number,
            items: o
                .items
                .into_iter()
                .map(|i| {
                    Ok(OrderItemOutput {
                        id: i.id.to_string(),
                        sku: i.sku,
                        name: i.name,
                        quantity: i.quantity,
                        unit_price: to_f64_result(i.unit_price, "order item unit price")?,
                        total: to_f64_result(i.total, "order item total")?,
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
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Orders {
    #[napi]
    pub async fn create(&self, input: CreateOrderInput) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let items: Vec<stateset_core::CreateOrderItem> = input
            .items
            .into_iter()
            .map(|i| {
                let product_id = i.product_id.and_then(|s| s.parse().ok()).unwrap_or_default();
                let variant_id = i.variant_id.and_then(|s| s.parse().ok());

                Ok(stateset_core::CreateOrderItem {
                    product_id,
                    variant_id,
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: decimal_from_f64(i.unit_price, "order item unit price")?,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let order = commerce
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id,
                items,
                currency: input.currency.and_then(|s| s.parse::<CurrencyCode>().ok()),
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create order: {}", e)))?;

        convert_output(order)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<OrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order = commerce
            .orders()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get order: {}", e)))?;

        convert_optional_output(order)
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<OrderOutput>> {
        let commerce = self.commerce.lock().await;
        let orders = commerce
            .orders()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list orders: {}", e)))?;

        convert_outputs(orders)
    }

    #[napi]
    pub async fn update_status(&self, id: String, status: String) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order_status = match status.to_lowercase().as_str() {
            "pending" => stateset_core::OrderStatus::Pending,
            "confirmed" => stateset_core::OrderStatus::Confirmed,
            "processing" => stateset_core::OrderStatus::Processing,
            "shipped" => stateset_core::OrderStatus::Shipped,
            "delivered" => stateset_core::OrderStatus::Delivered,
            "cancelled" => stateset_core::OrderStatus::Cancelled,
            "refunded" => stateset_core::OrderStatus::Refunded,
            _ => return Err(Error::from_reason(format!("Invalid status: {}", status))),
        };

        let order = commerce
            .orders()
            .update_status(uuid.into(), order_status)
            .map_err(|e| Error::from_reason(format!("Failed to update order: {}", e)))?;

        convert_output(order)
    }

    #[napi]
    pub async fn ship(&self, id: String, tracking_number: Option<String>) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order = commerce
            .orders()
            .ship(uuid.into(), tracking_number.as_deref())
            .map_err(|e| Error::from_reason(format!("Failed to ship order: {}", e)))?;

        convert_output(order)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order = commerce
            .orders()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel order: {}", e)))?;

        convert_output(order)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .orders()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count orders: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Products API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateProductVariantInput {
    pub sku: String,
    pub name: Option<String>,
    pub price: f64,
    pub compare_at_price: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateProductInput {
    pub name: String,
    pub description: Option<String>,
    pub variants: Option<Vec<CreateProductVariantInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductOutput {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Product> for ProductOutput {
    fn from(p: stateset_core::Product) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            slug: p.slug,
            description: p.description,
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductVariantOutput {
    pub id: String,
    pub product_id: String,
    pub sku: String,
    pub name: String,
    pub price: f64,
    pub compare_at_price: Option<f64>,
    pub is_default: bool,
}

impl TryFrom<stateset_core::ProductVariant> for ProductVariantOutput {
    type Error = Error;

    fn try_from(v: stateset_core::ProductVariant) -> Result<Self> {
        Ok(Self {
            id: v.id.to_string(),
            product_id: v.product_id.to_string(),
            sku: v.sku,
            name: v.name,
            price: to_f64_result(v.price, "product variant price")?,
            compare_at_price: optional_to_f64_result(
                v.compare_at_price,
                "product variant compare at price",
            )?,
            is_default: v.is_default,
        })
    }
}

#[napi]
pub struct Products {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Products {
    #[napi]
    pub async fn create(&self, input: CreateProductInput) -> Result<ProductOutput> {
        let commerce = self.commerce.lock().await;

        let variants = input
            .variants
            .map(|vs| {
                vs.into_iter()
                    .map(|v| {
                        Ok(stateset_core::CreateProductVariant {
                            sku: v.sku,
                            name: v.name,
                            price: decimal_from_f64(v.price, "variant price")?,
                            compare_at_price: optional_decimal_from_f64(
                                v.compare_at_price,
                                "variant compare at price",
                            )?,
                            ..Default::default()
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let product = commerce
            .products()
            .create(stateset_core::CreateProduct {
                name: input.name,
                description: input.description,
                variants,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create product: {}", e)))?;

        Ok(product.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ProductOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let product = commerce
            .products()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get product: {}", e)))?;

        Ok(product.map(|p| p.into()))
    }

    #[napi]
    pub async fn get_variant_by_sku(&self, sku: String) -> Result<Option<ProductVariantOutput>> {
        let commerce = self.commerce.lock().await;
        let variant = commerce
            .products()
            .get_variant_by_sku(&sku)
            .map_err(|e| Error::from_reason(format!("Failed to get variant: {}", e)))?;

        convert_optional_output(variant)
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<ProductOutput>> {
        let commerce = self.commerce.lock().await;
        let products = commerce
            .products()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list products: {}", e)))?;

        Ok(products.into_iter().map(|p| p.into()).collect())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .products()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count products: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Custom Objects API (custom states / metaobjects)
// ============================================================================

fn parse_custom_field_type(s: &str) -> Result<stateset_core::CustomFieldType> {
    s.parse::<stateset_core::CustomFieldType>()
        .map_err(|e| Error::from_reason(format!("Invalid custom field type '{}': {}", s, e)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomFieldDefinitionInput {
    pub key: String,
    pub field_type: String,
    pub required: Option<bool>,
    pub list: Option<bool>,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomFieldDefinitionOutput {
    pub key: String,
    pub field_type: String,
    pub required: bool,
    pub list: bool,
    pub description: Option<String>,
}

impl From<stateset_core::CustomFieldDefinition> for CustomFieldDefinitionOutput {
    fn from(f: stateset_core::CustomFieldDefinition) -> Self {
        Self {
            key: f.key,
            field_type: f.field_type.to_string(),
            required: f.required,
            list: f.list,
            description: f.description,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomObjectTypeInput {
    pub handle: String,
    pub display_name: String,
    pub description: Option<String>,
    pub fields: Vec<CustomFieldDefinitionInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCustomObjectTypeInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub fields: Option<Vec<CustomFieldDefinitionInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CustomObjectTypeFilterInput {
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomObjectTypeOutput {
    pub id: String,
    pub handle: String,
    pub display_name: String,
    pub description: String,
    pub fields: Vec<CustomFieldDefinitionOutput>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i32,
}

impl From<stateset_core::CustomObjectType> for CustomObjectTypeOutput {
    fn from(t: stateset_core::CustomObjectType) -> Self {
        Self {
            id: t.id.to_string(),
            handle: t.handle,
            display_name: t.display_name,
            description: t.description,
            fields: t.fields.into_iter().map(|f| f.into()).collect(),
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
            version: t.version,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCustomObjectInput {
    pub type_handle: String,
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    /// JSON string representing record values (must be an object).
    pub values_json: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCustomObjectInput {
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    /// JSON string representing record values (must be an object).
    pub values_json: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CustomObjectFilterInput {
    pub type_handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub handle: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomObjectOutput {
    pub id: String,
    pub type_id: String,
    pub type_handle: String,
    pub handle: Option<String>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub values_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i32,
}

impl From<stateset_core::CustomObject> for CustomObjectOutput {
    fn from(o: stateset_core::CustomObject) -> Self {
        let values_json = serde_json::to_string(&o.values).unwrap_or_else(|_| "{}".to_string());
        Self {
            id: o.id.to_string(),
            type_id: o.type_id.to_string(),
            type_handle: o.type_handle,
            handle: o.handle,
            owner_type: o.owner_type,
            owner_id: o.owner_id,
            values_json,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
            version: o.version,
        }
    }
}

#[napi]
pub struct CustomObjects {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl CustomObjects {
    #[napi]
    pub async fn create_type(
        &self,
        input: CreateCustomObjectTypeInput,
    ) -> Result<CustomObjectTypeOutput> {
        let commerce = self.commerce.lock().await;

        let mut fields = Vec::with_capacity(input.fields.len());
        for f in input.fields {
            fields.push(stateset_core::CustomFieldDefinition {
                key: f.key,
                field_type: parse_custom_field_type(&f.field_type)?,
                required: f.required.unwrap_or(false),
                list: f.list.unwrap_or(false),
                description: f.description,
            });
        }

        let ty = commerce
            .custom_objects()
            .create_type(stateset_core::CreateCustomObjectType {
                handle: input.handle,
                display_name: input.display_name,
                description: input.description,
                fields,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to create custom object type: {}", e))
            })?;

        Ok(ty.into())
    }

    #[napi]
    pub async fn get_type(&self, id: String) -> Result<Option<CustomObjectTypeOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let ty = commerce
            .custom_objects()
            .get_type(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get custom object type: {}", e)))?;

        Ok(ty.map(|t| t.into()))
    }

    #[napi]
    pub async fn get_type_by_handle(
        &self,
        handle: String,
    ) -> Result<Option<CustomObjectTypeOutput>> {
        let commerce = self.commerce.lock().await;
        let ty = commerce
            .custom_objects()
            .get_type_by_handle(&handle)
            .map_err(|e| Error::from_reason(format!("Failed to get custom object type: {}", e)))?;

        Ok(ty.map(|t| t.into()))
    }

    #[napi]
    pub async fn update_type(
        &self,
        id: String,
        input: UpdateCustomObjectTypeInput,
    ) -> Result<CustomObjectTypeOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let fields = if let Some(fields) = input.fields {
            let mut out = Vec::with_capacity(fields.len());
            for f in fields {
                out.push(stateset_core::CustomFieldDefinition {
                    key: f.key,
                    field_type: parse_custom_field_type(&f.field_type)?,
                    required: f.required.unwrap_or(false),
                    list: f.list.unwrap_or(false),
                    description: f.description,
                });
            }
            Some(out)
        } else {
            None
        };

        let updated = commerce
            .custom_objects()
            .update_type(
                uuid,
                stateset_core::UpdateCustomObjectType {
                    display_name: input.display_name,
                    description: input.description,
                    fields,
                },
            )
            .map_err(|e| {
                Error::from_reason(format!("Failed to update custom object type: {}", e))
            })?;

        Ok(updated.into())
    }

    #[napi]
    pub async fn list_types(
        &self,
        filter: Option<CustomObjectTypeFilterInput>,
    ) -> Result<Vec<CustomObjectTypeOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();

        let list = commerce
            .custom_objects()
            .list_types(stateset_core::CustomObjectTypeFilter {
                search: filter.search,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to list custom object types: {}", e))
            })?;

        Ok(list.into_iter().map(|t| t.into()).collect())
    }

    #[napi]
    pub async fn delete_type(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        commerce.custom_objects().delete_type(uuid).map_err(|e| {
            Error::from_reason(format!("Failed to delete custom object type: {}", e))
        })?;

        Ok(())
    }

    #[napi]
    pub async fn create_object(
        &self,
        input: CreateCustomObjectInput,
    ) -> Result<CustomObjectOutput> {
        let commerce = self.commerce.lock().await;

        let values: serde_json::Value = serde_json::from_str(&input.values_json)
            .map_err(|e| Error::from_reason(format!("Invalid valuesJson: {}", e)))?;

        let obj = commerce
            .custom_objects()
            .create_object(stateset_core::CreateCustomObject {
                type_handle: input.type_handle,
                handle: input.handle,
                owner_type: input.owner_type,
                owner_id: input.owner_id,
                values,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create custom object: {}", e)))?;

        Ok(obj.into())
    }

    #[napi]
    pub async fn get_object(&self, id: String) -> Result<Option<CustomObjectOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let obj = commerce
            .custom_objects()
            .get_object(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get custom object: {}", e)))?;

        Ok(obj.map(|o| o.into()))
    }

    #[napi]
    pub async fn get_object_by_handle(
        &self,
        type_handle: String,
        object_handle: String,
    ) -> Result<Option<CustomObjectOutput>> {
        let commerce = self.commerce.lock().await;
        let obj = commerce
            .custom_objects()
            .get_object_by_handle(&type_handle, &object_handle)
            .map_err(|e| Error::from_reason(format!("Failed to get custom object: {}", e)))?;

        Ok(obj.map(|o| o.into()))
    }

    #[napi]
    pub async fn update_object(
        &self,
        id: String,
        input: UpdateCustomObjectInput,
    ) -> Result<CustomObjectOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let values = match input.values_json {
            Some(s) => Some(
                serde_json::from_str(&s)
                    .map_err(|e| Error::from_reason(format!("Invalid valuesJson: {}", e)))?,
            ),
            None => None,
        };

        let updated = commerce
            .custom_objects()
            .update_object(
                uuid,
                stateset_core::UpdateCustomObject {
                    handle: input.handle,
                    owner_type: input.owner_type,
                    owner_id: input.owner_id,
                    values,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update custom object: {}", e)))?;

        Ok(updated.into())
    }

    #[napi]
    pub async fn list_objects(
        &self,
        filter: Option<CustomObjectFilterInput>,
    ) -> Result<Vec<CustomObjectOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();

        let list = commerce
            .custom_objects()
            .list_objects(stateset_core::CustomObjectFilter {
                type_handle: filter.type_handle,
                owner_type: filter.owner_type,
                owner_id: filter.owner_id,
                handle: filter.handle,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list custom objects: {}", e)))?;

        Ok(list.into_iter().map(|o| o.into()).collect())
    }

    #[napi]
    pub async fn delete_object(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        commerce
            .custom_objects()
            .delete_object(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to delete custom object: {}", e)))?;

        Ok(())
    }
}

// ============================================================================
// Inventory API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInventoryItemInput {
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub initial_quantity: Option<f64>,
    pub reorder_point: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InventoryItemOutput {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: String,
    pub is_active: bool,
}

impl From<stateset_core::InventoryItem> for InventoryItemOutput {
    fn from(i: stateset_core::InventoryItem) -> Self {
        Self {
            id: i.id,
            sku: i.sku,
            name: i.name,
            description: i.description,
            unit_of_measure: i.unit_of_measure,
            is_active: i.is_active,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockLevelOutput {
    pub sku: String,
    pub name: String,
    pub total_on_hand: f64,
    pub total_allocated: f64,
    pub total_available: f64,
}

impl TryFrom<stateset_core::StockLevel> for StockLevelOutput {
    type Error = Error;

    fn try_from(s: stateset_core::StockLevel) -> Result<Self> {
        Ok(Self {
            sku: s.sku,
            name: s.name,
            total_on_hand: to_f64_result(s.total_on_hand, "stock total on hand")?,
            total_allocated: to_f64_result(s.total_allocated, "stock total allocated")?,
            total_available: to_f64_result(s.total_available, "stock total available")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReservationOutput {
    pub id: String,
    pub item_id: i64,
    pub quantity: f64,
    pub status: String,
}

impl TryFrom<stateset_core::InventoryReservation> for ReservationOutput {
    type Error = Error;

    fn try_from(r: stateset_core::InventoryReservation) -> Result<Self> {
        Ok(Self {
            id: r.id.to_string(),
            item_id: r.item_id,
            quantity: to_f64_result(r.quantity, "reservation quantity")?,
            status: format!("{}", r.status),
        })
    }
}

#[napi]
pub struct Inventory {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Inventory {
    #[napi]
    pub async fn create_item(
        &self,
        input: CreateInventoryItemInput,
    ) -> Result<InventoryItemOutput> {
        let commerce = self.commerce.lock().await;

        let item = commerce
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku: input.sku,
                name: input.name,
                description: input.description,
                initial_quantity: optional_decimal_from_f64(
                    input.initial_quantity,
                    "inventory initial quantity",
                )?,
                reorder_point: optional_decimal_from_f64(
                    input.reorder_point,
                    "inventory reorder point",
                )?,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create inventory item: {}", e)))?;

        Ok(item.into())
    }

    #[napi]
    pub async fn get_stock(&self, sku: String) -> Result<Option<StockLevelOutput>> {
        let commerce = self.commerce.lock().await;
        let stock = commerce
            .inventory()
            .get_stock(&sku)
            .map_err(|e| Error::from_reason(format!("Failed to get stock: {}", e)))?;

        convert_optional_output(stock)
    }

    #[napi]
    pub async fn adjust(&self, sku: String, quantity: f64, reason: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| Error::from_reason("Invalid quantity"))?;

        commerce
            .inventory()
            .adjust(&sku, qty, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to adjust inventory: {}", e)))?;

        Ok(())
    }

    #[napi]
    pub async fn reserve(
        &self,
        sku: String,
        quantity: f64,
        reference_type: String,
        reference_id: String,
        expires_in_seconds: Option<i64>,
    ) -> Result<ReservationOutput> {
        let commerce = self.commerce.lock().await;
        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| Error::from_reason("Invalid quantity"))?;

        let reservation = commerce
            .inventory()
            .reserve(&sku, qty, &reference_type, &reference_id, expires_in_seconds)
            .map_err(|e| Error::from_reason(format!("Failed to reserve inventory: {}", e)))?;

        convert_output(reservation)
    }

    #[napi]
    pub async fn confirm_reservation(&self, reservation_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = reservation_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        commerce
            .inventory()
            .confirm_reservation(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to confirm reservation: {}", e)))?;

        Ok(())
    }

    #[napi]
    pub async fn release_reservation(&self, reservation_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = reservation_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        commerce
            .inventory()
            .release_reservation(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to release reservation: {}", e)))?;

        Ok(())
    }
}

// ============================================================================
// Returns API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReturnItemInput {
    pub order_item_id: String,
    pub quantity: i32,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReturnInput {
    pub order_id: String,
    pub reason: String,
    pub reason_details: Option<String>,
    pub idempotency_key: Option<String>,
    pub items: Vec<CreateReturnItemInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReturnOutput {
    pub id: String,
    pub order_id: String,
    pub status: String,
    pub reason: String,
    pub version: i32,
    pub created_at: String,
    pub idempotency_key: Option<String>,
}

impl From<stateset_core::Return> for ReturnOutput {
    fn from(r: stateset_core::Return) -> Self {
        Self {
            id: r.id.to_string(),
            order_id: r.order_id.to_string(),
            status: format!("{}", r.status),
            reason: format!("{}", r.reason),
            version: r.version,
            created_at: r.created_at.to_rfc3339(),
            idempotency_key: r.idempotency_key,
        }
    }
}

#[napi]
pub struct Returns {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Returns {
    #[napi]
    pub async fn create(&self, input: CreateReturnInput) -> Result<ReturnOutput> {
        let commerce = self.commerce.lock().await;

        let order_id =
            input.order_id.parse().map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let reason = match input.reason.to_lowercase().as_str() {
            "defective" => stateset_core::ReturnReason::Defective,
            "not_as_described" => stateset_core::ReturnReason::NotAsDescribed,
            "wrong_item" => stateset_core::ReturnReason::WrongItem,
            "no_longer_needed" => stateset_core::ReturnReason::NoLongerNeeded,
            "changed_mind" => stateset_core::ReturnReason::ChangedMind,
            "better_price_found" => stateset_core::ReturnReason::BetterPriceFound,
            "damaged" => stateset_core::ReturnReason::Damaged,
            _ => stateset_core::ReturnReason::Other,
        };

        let items: Vec<stateset_core::CreateReturnItem> = input
            .items
            .into_iter()
            .map(|i| {
                let order_item_id = i.order_item_id.parse().unwrap_or_default();
                stateset_core::CreateReturnItem {
                    order_item_id,
                    quantity: i.quantity,
                    ..Default::default()
                }
            })
            .collect();

        let ret = commerce
            .returns()
            .create(stateset_core::CreateReturn {
                order_id,
                reason,
                reason_details: input.reason_details,
                idempotency_key: input.idempotency_key,
                items,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create return: {}", e)))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ReturnOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get return: {}", e)))?;

        Ok(ret.map(|r| r.into()))
    }

    #[napi]
    pub async fn approve(&self, id: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .approve(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to approve return: {}", e)))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn reject(&self, id: String, reason: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .reject(uuid.into(), &reason)
            .map_err(|e| Error::from_reason(format!("Failed to reject return: {}", e)))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<ReturnOutput>> {
        let commerce = self.commerce.lock().await;
        let returns = commerce
            .returns()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list returns: {}", e)))?;

        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .returns()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count returns: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Payments API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePaymentInput {
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub customer_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub amount: f64,
    pub currency: Option<String>,
    pub payment_method: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentOutput {
    pub id: String,
    pub payment_number: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub customer_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub amount: f64,
    pub currency: String,
    pub status: String,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Payment> for PaymentOutput {
    type Error = Error;

    fn try_from(p: stateset_core::Payment) -> Result<Self> {
        Ok(Self {
            id: p.id.to_string(),
            payment_number: p.payment_number,
            order_id: p.order_id.map(|id| id.to_string()),
            invoice_id: p.invoice_id.map(|id| id.to_string()),
            customer_id: p.customer_id.map(|id| id.to_string()),
            idempotency_key: p.idempotency_key,
            amount: to_f64_result(p.amount, "payment amount")?,
            currency: p.currency.to_string(),
            status: format!("{}", p.status),
            version: p.version,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRefundInput {
    pub payment_id: String,
    pub amount: f64,
    pub reason: Option<String>,
    pub idempotency_key: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RefundOutput {
    pub id: String,
    pub refund_number: String,
    pub payment_id: String,
    pub amount: f64,
    pub status: String,
    pub reason: Option<String>,
    pub created_at: String,
    pub idempotency_key: Option<String>,
}

impl TryFrom<stateset_core::Refund> for RefundOutput {
    type Error = Error;

    fn try_from(r: stateset_core::Refund) -> Result<Self> {
        Ok(Self {
            id: r.id.to_string(),
            refund_number: r.refund_number,
            payment_id: r.payment_id.to_string(),
            amount: to_f64_result(r.amount, "refund amount")?,
            status: format!("{}", r.status),
            reason: r.reason,
            created_at: r.created_at.to_rfc3339(),
            idempotency_key: r.idempotency_key,
        })
    }
}

#[napi]
pub struct Payments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Payments {
    #[napi]
    pub async fn create(&self, input: CreatePaymentInput) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id = input
            .customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let invoice_id = input
            .invoice_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid invoice UUID"))?;

        let payment_method = input
            .payment_method
            .map(|m| match m.to_lowercase().as_str() {
                "credit_card" => stateset_core::PaymentMethodType::CreditCard,
                "debit_card" => stateset_core::PaymentMethodType::DebitCard,
                "bank_transfer" => stateset_core::PaymentMethodType::BankTransfer,
                "paypal" => stateset_core::PaymentMethodType::PayPal,
                "applepay" | "apple_pay" => stateset_core::PaymentMethodType::ApplePay,
                "googlepay" | "google_pay" => stateset_core::PaymentMethodType::GooglePay,
                "crypto" => stateset_core::PaymentMethodType::Crypto,
                "storecredit" | "store_credit" => stateset_core::PaymentMethodType::StoreCredit,
                "giftcard" | "gift_card" => stateset_core::PaymentMethodType::GiftCard,
                _ => stateset_core::PaymentMethodType::CreditCard,
            })
            .unwrap_or(stateset_core::PaymentMethodType::CreditCard);

        let payment = commerce
            .payments()
            .create(stateset_core::CreatePayment {
                order_id,
                invoice_id,
                customer_id,
                idempotency_key: input.idempotency_key,
                amount: decimal_from_f64(input.amount, "payment amount")?,
                currency: input.currency.and_then(|s| s.parse::<CurrencyCode>().ok()),
                payment_method,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create payment: {}", e)))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PaymentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get payment: {}", e)))?;

        convert_optional_output(payment)
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<PaymentOutput>> {
        let commerce = self.commerce.lock().await;
        let payments = commerce
            .payments()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list payments: {}", e)))?;

        convert_outputs(payments)
    }

    #[napi]
    pub async fn mark_completed(&self, id: String) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_completed(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to complete payment: {}", e)))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn mark_failed(
        &self,
        id: String,
        reason: String,
        code: Option<String>,
    ) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_failed(uuid.into(), &reason, code.as_deref())
            .map_err(|e| Error::from_reason(format!("Failed to fail payment: {}", e)))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel payment: {}", e)))?;

        convert_output(payment)
    }

    #[napi]
    pub async fn create_refund(&self, input: CreateRefundInput) -> Result<RefundOutput> {
        let commerce = self.commerce.lock().await;
        let payment_id =
            input.payment_id.parse().map_err(|_| Error::from_reason("Invalid payment UUID"))?;

        let refund = commerce
            .payments()
            .create_refund(stateset_core::CreateRefund {
                payment_id,
                amount: Some(decimal_from_f64(input.amount, "refund amount")?),
                reason: input.reason,
                idempotency_key: input.idempotency_key,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create refund: {}", e)))?;

        convert_output(refund)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .payments()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count payments: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Shipments API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateShipmentInput {
    pub order_id: String,
    pub recipient_name: String,
    pub shipping_address: String,
    pub carrier: Option<String>,
    pub shipping_method: Option<String>,
    pub tracking_number: Option<String>,
    pub recipient_email: Option<String>,
    pub recipient_phone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShipmentOutput {
    pub id: String,
    pub shipment_number: String,
    pub order_id: String,
    pub status: String,
    pub carrier: String,
    pub shipping_method: String,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub recipient_name: String,
    pub shipping_address: String,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Shipment> for ShipmentOutput {
    fn from(s: stateset_core::Shipment) -> Self {
        Self {
            id: s.id.to_string(),
            shipment_number: s.shipment_number,
            order_id: s.order_id.to_string(),
            status: format!("{}", s.status),
            carrier: format!("{}", s.carrier),
            shipping_method: format!("{}", s.shipping_method),
            tracking_number: s.tracking_number,
            tracking_url: s.tracking_url,
            recipient_name: s.recipient_name,
            shipping_address: s.shipping_address,
            version: s.version,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Shipments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Shipments {
    #[napi]
    pub async fn create(&self, input: CreateShipmentInput) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;

        let order_id =
            input.order_id.parse().map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let carrier = input.carrier.map(|c| match c.to_lowercase().as_str() {
            "ups" => stateset_core::ShippingCarrier::Ups,
            "fedex" => stateset_core::ShippingCarrier::FedEx,
            "usps" => stateset_core::ShippingCarrier::Usps,
            "dhl" => stateset_core::ShippingCarrier::Dhl,
            _ => stateset_core::ShippingCarrier::Other,
        });

        let shipping_method = input.shipping_method.map(|m| match m.to_lowercase().as_str() {
            "standard" => stateset_core::ShippingMethod::Standard,
            "express" => stateset_core::ShippingMethod::Express,
            "overnight" => stateset_core::ShippingMethod::Overnight,
            "ground" => stateset_core::ShippingMethod::Ground,
            "twoday" | "two_day" => stateset_core::ShippingMethod::TwoDay,
            "sameday" | "same_day" => stateset_core::ShippingMethod::SameDay,
            "international" => stateset_core::ShippingMethod::International,
            "freight" => stateset_core::ShippingMethod::Freight,
            _ => stateset_core::ShippingMethod::Standard,
        });

        let shipment = commerce
            .shipments()
            .create(stateset_core::CreateShipment {
                order_id,
                carrier,
                shipping_method,
                tracking_number: input.tracking_number,
                recipient_name: input.recipient_name,
                recipient_email: input.recipient_email,
                recipient_phone: input.recipient_phone,
                shipping_address: input.shipping_address,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create shipment: {}", e)))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ShipmentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get shipment: {}", e)))?;

        Ok(shipment.map(|s| s.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<ShipmentOutput>> {
        let commerce = self.commerce.lock().await;
        let shipments = commerce
            .shipments()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list shipments: {}", e)))?;

        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    #[napi]
    pub async fn ship(
        &self,
        id: String,
        tracking_number: Option<String>,
    ) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .ship(uuid.into(), tracking_number)
            .map_err(|e| Error::from_reason(format!("Failed to ship: {}", e)))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn deliver(&self, id: String) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .mark_delivered(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to deliver: {}", e)))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel shipment: {}", e)))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .shipments()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count shipments: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Warranties API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWarrantyInput {
    pub customer_id: String,
    pub product_id: Option<String>,
    pub order_id: Option<String>,
    pub warranty_type: Option<String>,
    pub duration_months: Option<i32>,
    pub serial_number: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WarrantyOutput {
    pub id: String,
    pub warranty_number: String,
    pub customer_id: String,
    pub product_id: Option<String>,
    pub order_id: Option<String>,
    pub status: String,
    pub warranty_type: String,
    pub start_date: String,
    pub end_date: String,
    pub created_at: String,
}

impl From<stateset_core::Warranty> for WarrantyOutput {
    fn from(w: stateset_core::Warranty) -> Self {
        Self {
            id: w.id.to_string(),
            warranty_number: w.warranty_number,
            customer_id: w.customer_id.to_string(),
            product_id: w.product_id.map(|id| id.to_string()),
            order_id: w.order_id.map(|id| id.to_string()),
            status: format!("{}", w.status),
            warranty_type: format!("{}", w.warranty_type),
            start_date: w.start_date.to_rfc3339(),
            end_date: w.end_date.map(|d| d.to_rfc3339()).unwrap_or_default(),
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWarrantyClaimInput {
    pub warranty_id: String,
    pub issue_description: String,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WarrantyClaimOutput {
    pub id: String,
    pub claim_number: String,
    pub warranty_id: String,
    pub status: String,
    pub issue_description: String,
    pub resolution: String,
    pub created_at: String,
}

impl From<stateset_core::WarrantyClaim> for WarrantyClaimOutput {
    fn from(c: stateset_core::WarrantyClaim) -> Self {
        Self {
            id: c.id.to_string(),
            claim_number: c.claim_number,
            warranty_id: c.warranty_id.to_string(),
            status: format!("{}", c.status),
            issue_description: c.issue_description,
            resolution: format!("{}", c.resolution),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Warranties {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Warranties {
    #[napi]
    pub async fn create(&self, input: CreateWarrantyInput) -> Result<WarrantyOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let product_id = input
            .product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid product UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let warranty_type = input.warranty_type.and_then(|t| match t.to_lowercase().as_str() {
            "standard" => Some(stateset_core::WarrantyType::Standard),
            "extended" => Some(stateset_core::WarrantyType::Extended),
            "limited" => Some(stateset_core::WarrantyType::Limited),
            "lifetime" => Some(stateset_core::WarrantyType::Lifetime),
            _ => None,
        });

        let warranty = commerce
            .warranties()
            .create(stateset_core::CreateWarranty {
                customer_id,
                product_id,
                order_id,
                warranty_type,
                duration_months: input.duration_months,
                serial_number: input.serial_number,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create warranty: {}", e)))?;

        Ok(warranty.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WarrantyOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let warranty = commerce
            .warranties()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get warranty: {}", e)))?;

        Ok(warranty.map(|w| w.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<WarrantyOutput>> {
        let commerce = self.commerce.lock().await;
        let warranties = commerce
            .warranties()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list warranties: {}", e)))?;

        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    #[napi]
    pub async fn create_claim(
        &self,
        input: CreateWarrantyClaimInput,
    ) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let warranty_id =
            input.warranty_id.parse().map_err(|_| Error::from_reason("Invalid warranty UUID"))?;

        let claim = commerce
            .warranties()
            .create_claim(stateset_core::CreateWarrantyClaim {
                warranty_id,
                issue_description: input.issue_description,
                contact_email: input.contact_email,
                contact_phone: input.contact_phone,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create claim: {}", e)))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn approve_claim(&self, id: String) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .approve_claim(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to approve claim: {}", e)))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn deny_claim(&self, id: String, reason: String) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .deny_claim(uuid, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to deny claim: {}", e)))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn complete_claim(
        &self,
        id: String,
        resolution: String,
    ) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let res = match resolution.to_lowercase().as_str() {
            "repair" => stateset_core::ClaimResolution::Repair,
            "replacement" => stateset_core::ClaimResolution::Replacement,
            "refund" => stateset_core::ClaimResolution::Refund,
            "store_credit" | "storecredit" => stateset_core::ClaimResolution::StoreCredit,
            "denied" => stateset_core::ClaimResolution::Denied,
            _ => stateset_core::ClaimResolution::None,
        };

        let claim = commerce
            .warranties()
            .complete_claim(uuid, res)
            .map_err(|e| Error::from_reason(format!("Failed to complete claim: {}", e)))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .warranties()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count warranties: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Purchase Orders API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSupplierInput {
    pub name: String,
    pub supplier_code: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SupplierOutput {
    pub id: String,
    pub name: String,
    pub supplier_code: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

impl From<stateset_core::Supplier> for SupplierOutput {
    fn from(s: stateset_core::Supplier) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            supplier_code: Some(s.supplier_code),
            email: s.email,
            phone: s.phone,
            is_active: s.is_active,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePurchaseOrderItemInput {
    pub sku: String,
    pub name: String,
    pub quantity: f64,
    pub unit_cost: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePurchaseOrderInput {
    pub supplier_id: String,
    pub items: Vec<CreatePurchaseOrderItemInput>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurchaseOrderOutput {
    pub id: String,
    pub po_number: String,
    pub supplier_id: String,
    pub status: String,
    pub subtotal: f64,
    pub total: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::PurchaseOrder> for PurchaseOrderOutput {
    type Error = Error;

    fn try_from(po: stateset_core::PurchaseOrder) -> Result<Self> {
        Ok(Self {
            id: po.id.to_string(),
            po_number: po.po_number,
            supplier_id: po.supplier_id.to_string(),
            status: format!("{}", po.status),
            subtotal: to_f64_result(po.subtotal, "purchase order subtotal")?,
            total: to_f64_result(po.total, "purchase order total")?,
            created_at: po.created_at.to_rfc3339(),
            updated_at: po.updated_at.to_rfc3339(),
        })
    }
}

#[napi]
pub struct PurchaseOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl PurchaseOrders {
    #[napi]
    pub async fn create_supplier(&self, input: CreateSupplierInput) -> Result<SupplierOutput> {
        let commerce = self.commerce.lock().await;

        let supplier = commerce
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                name: input.name,
                supplier_code: input.supplier_code,
                email: input.email,
                phone: input.phone,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create supplier: {}", e)))?;

        Ok(supplier.into())
    }

    #[napi]
    pub async fn get_supplier(&self, id: String) -> Result<Option<SupplierOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let supplier = commerce
            .purchase_orders()
            .get_supplier(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get supplier: {}", e)))?;

        Ok(supplier.map(|s| s.into()))
    }

    #[napi]
    pub async fn list_suppliers(&self) -> Result<Vec<SupplierOutput>> {
        let commerce = self.commerce.lock().await;
        let suppliers = commerce
            .purchase_orders()
            .list_suppliers(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list suppliers: {}", e)))?;

        Ok(suppliers.into_iter().map(|s| s.into()).collect())
    }

    #[napi]
    pub async fn create(&self, input: CreatePurchaseOrderInput) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;

        let supplier_id =
            input.supplier_id.parse().map_err(|_| Error::from_reason("Invalid supplier UUID"))?;

        let items: Vec<stateset_core::CreatePurchaseOrderItem> = input
            .items
            .into_iter()
            .map(|i| {
                Ok(stateset_core::CreatePurchaseOrderItem {
                    sku: i.sku,
                    name: i.name,
                    quantity: decimal_from_f64(i.quantity, "purchase order item quantity")?,
                    unit_cost: decimal_from_f64(i.unit_cost, "purchase order item unit cost")?,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id,
                items,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create PO: {}", e)))?;

        convert_output(po)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PurchaseOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get PO: {}", e)))?;

        convert_optional_output(po)
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<PurchaseOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let pos = commerce
            .purchase_orders()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list POs: {}", e)))?;

        convert_outputs(pos)
    }

    #[napi]
    pub async fn submit(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .submit(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to submit PO: {}", e)))?;

        convert_output(po)
    }

    #[napi]
    pub async fn approve(&self, id: String, approved_by: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .approve(uuid, &approved_by)
            .map_err(|e| Error::from_reason(format!("Failed to approve PO: {}", e)))?;

        convert_output(po)
    }

    #[napi]
    pub async fn send(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .send(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to send PO: {}", e)))?;

        convert_output(po)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel PO: {}", e)))?;

        convert_output(po)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .purchase_orders()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count POs: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Invoices API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInvoiceItemInput {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub sku: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInvoiceInput {
    pub customer_id: String,
    pub order_id: Option<String>,
    pub items: Vec<CreateInvoiceItemInput>,
    pub billing_email: Option<String>,
    pub billing_name: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InvoiceOutput {
    pub id: String,
    pub invoice_number: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub status: String,
    pub subtotal: f64,
    pub tax_amount: f64,
    pub total: f64,
    pub amount_paid: f64,
    pub due_date: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Invoice> for InvoiceOutput {
    type Error = Error;

    fn try_from(inv: stateset_core::Invoice) -> Result<Self> {
        Ok(Self {
            id: inv.id.to_string(),
            invoice_number: inv.invoice_number,
            customer_id: inv.customer_id.to_string(),
            order_id: inv.order_id.map(|id| id.to_string()),
            status: format!("{}", inv.status),
            subtotal: to_f64_result(inv.subtotal, "invoice subtotal")?,
            tax_amount: to_f64_result(inv.tax_amount, "invoice tax amount")?,
            total: to_f64_result(inv.total, "invoice total")?,
            amount_paid: to_f64_result(inv.amount_paid, "invoice amount paid")?,
            due_date: inv.due_date.to_rfc3339(),
            created_at: inv.created_at.to_rfc3339(),
            updated_at: inv.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RecordPaymentInput {
    pub amount: f64,
    pub payment_method: Option<String>,
    pub reference: Option<String>,
}

#[napi]
pub struct Invoices {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Invoices {
    #[napi]
    pub async fn create(&self, input: CreateInvoiceInput) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let items: Vec<stateset_core::CreateInvoiceItem> = input
            .items
            .into_iter()
            .map(|i| {
                Ok(stateset_core::CreateInvoiceItem {
                    description: i.description,
                    quantity: decimal_from_f64(i.quantity, "invoice item quantity")?,
                    unit_price: decimal_from_f64(i.unit_price, "invoice item unit price")?,
                    sku: i.sku,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let invoice = commerce
            .invoices()
            .create(stateset_core::CreateInvoice {
                customer_id,
                order_id,
                items,
                billing_email: input.billing_email,
                billing_name: input.billing_name,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create invoice: {}", e)))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<InvoiceOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get invoice: {}", e)))?;

        convert_optional_output(invoice)
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<InvoiceOutput>> {
        let commerce = self.commerce.lock().await;
        let invoices = commerce
            .invoices()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list invoices: {}", e)))?;

        convert_outputs(invoices)
    }

    #[napi]
    pub async fn send(&self, id: String) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .send(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to send invoice: {}", e)))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn void(&self, id: String) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .void(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to void invoice: {}", e)))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn record_payment(
        &self,
        id: String,
        input: RecordPaymentInput,
    ) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .record_payment(
                uuid,
                stateset_core::RecordInvoicePayment {
                    amount: decimal_from_f64(input.amount, "invoice payment amount")?,
                    payment_method: input.payment_method,
                    reference: input.reference,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to record payment: {}", e)))?;

        convert_output(invoice)
    }

    #[napi]
    pub async fn get_overdue(&self) -> Result<Vec<InvoiceOutput>> {
        let commerce = self.commerce.lock().await;
        let invoices = commerce
            .invoices()
            .get_overdue()
            .map_err(|e| Error::from_reason(format!("Failed to get overdue invoices: {}", e)))?;

        convert_outputs(invoices)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .invoices()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count invoices: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Bill of Materials API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBomInput {
    pub name: String,
    pub product_id: String,
    pub description: Option<String>,
    pub revision: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BomOutput {
    pub id: String,
    pub bom_number: String,
    pub name: String,
    pub product_id: String,
    pub status: String,
    pub revision: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::BillOfMaterials> for BomOutput {
    fn from(bom: stateset_core::BillOfMaterials) -> Self {
        Self {
            id: bom.id.to_string(),
            bom_number: bom.bom_number,
            name: bom.name,
            product_id: bom.product_id.to_string(),
            status: format!("{}", bom.status),
            revision: bom.revision,
            created_at: bom.created_at.to_rfc3339(),
            updated_at: bom.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBomComponentInput {
    pub component_sku: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit_of_measure: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BomComponentOutput {
    pub id: String,
    pub bom_id: String,
    pub component_sku: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit_of_measure: String,
}

impl TryFrom<stateset_core::BomComponent> for BomComponentOutput {
    type Error = Error;

    fn try_from(c: stateset_core::BomComponent) -> Result<Self> {
        Ok(Self {
            id: c.id.to_string(),
            bom_id: c.bom_id.to_string(),
            component_sku: c.component_sku,
            name: c.name,
            quantity: to_f64_result(c.quantity, "bom component quantity")?,
            unit_of_measure: c.unit_of_measure,
        })
    }
}

#[napi]
pub struct Bom {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Bom {
    #[napi]
    pub async fn create(&self, input: CreateBomInput) -> Result<BomOutput> {
        let commerce = self.commerce.lock().await;

        let product_id =
            input.product_id.parse().map_err(|_| Error::from_reason("Invalid product UUID"))?;

        let bom = commerce
            .bom()
            .create(stateset_core::CreateBom {
                name: input.name,
                product_id,
                description: input.description,
                revision: input.revision,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create BOM: {}", e)))?;

        Ok(bom.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<BomOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let bom = commerce
            .bom()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get BOM: {}", e)))?;

        Ok(bom.map(|b| b.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<BomOutput>> {
        let commerce = self.commerce.lock().await;
        let boms = commerce
            .bom()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list BOMs: {}", e)))?;

        Ok(boms.into_iter().map(|b| b.into()).collect())
    }

    #[napi]
    pub async fn add_component(
        &self,
        bom_id: String,
        input: CreateBomComponentInput,
    ) -> Result<BomComponentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = bom_id.parse().map_err(|_| Error::from_reason("Invalid BOM UUID"))?;

        let component = commerce
            .bom()
            .add_component(
                uuid,
                stateset_core::CreateBomComponent {
                    component_sku: input.component_sku,
                    name: input.name,
                    quantity: decimal_from_f64(input.quantity, "bom component quantity")?,
                    unit_of_measure: input.unit_of_measure,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to add component: {}", e)))?;

        convert_output(component)
    }

    #[napi]
    pub async fn get_components(&self, bom_id: String) -> Result<Vec<BomComponentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = bom_id.parse().map_err(|_| Error::from_reason("Invalid BOM UUID"))?;

        let components = commerce
            .bom()
            .get_components(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get components: {}", e)))?;

        convert_outputs(components)
    }

    #[napi]
    pub async fn activate(&self, id: String) -> Result<BomOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let bom = commerce
            .bom()
            .activate(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to activate BOM: {}", e)))?;

        Ok(bom.into())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .bom()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count BOMs: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Work Orders API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWorkOrderInput {
    pub product_id: String,
    pub bom_id: Option<String>,
    pub quantity_to_build: f64,
    pub priority: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WorkOrderOutput {
    pub id: String,
    pub work_order_number: String,
    pub product_id: String,
    pub bom_id: Option<String>,
    pub status: String,
    pub priority: String,
    pub quantity_to_build: f64,
    pub quantity_completed: f64,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::WorkOrder> for WorkOrderOutput {
    type Error = Error;

    fn try_from(wo: stateset_core::WorkOrder) -> Result<Self> {
        Ok(Self {
            id: wo.id.to_string(),
            work_order_number: wo.work_order_number,
            product_id: wo.product_id.to_string(),
            bom_id: wo.bom_id.map(|id| id.to_string()),
            status: format!("{}", wo.status),
            priority: format!("{}", wo.priority),
            quantity_to_build: to_f64_result(wo.quantity_to_build, "work order quantity to build")?,
            quantity_completed: to_f64_result(
                wo.quantity_completed,
                "work order quantity completed",
            )?,
            version: wo.version,
            created_at: wo.created_at.to_rfc3339(),
            updated_at: wo.updated_at.to_rfc3339(),
        })
    }
}

#[napi]
pub struct WorkOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl WorkOrders {
    #[napi]
    pub async fn create(&self, input: CreateWorkOrderInput) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;

        let product_id =
            input.product_id.parse().map_err(|_| Error::from_reason("Invalid product UUID"))?;

        let bom_id = input
            .bom_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid BOM UUID"))?;

        let priority = input.priority.and_then(|p| match p.to_lowercase().as_str() {
            "low" => Some(stateset_core::WorkOrderPriority::Low),
            "normal" => Some(stateset_core::WorkOrderPriority::Normal),
            "high" => Some(stateset_core::WorkOrderPriority::High),
            "urgent" => Some(stateset_core::WorkOrderPriority::Urgent),
            _ => None,
        });

        let wo = commerce
            .work_orders()
            .create(stateset_core::CreateWorkOrder {
                product_id,
                bom_id,
                quantity_to_build: decimal_from_f64(
                    input.quantity_to_build,
                    "work order quantity to build",
                )?,
                priority,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create work order: {}", e)))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WorkOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get work order: {}", e)))?;

        convert_optional_output(wo)
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<WorkOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let orders = commerce
            .work_orders()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list work orders: {}", e)))?;

        convert_outputs(orders)
    }

    #[napi]
    pub async fn start(&self, id: String) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .start(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to start work order: {}", e)))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn complete(&self, id: String, quantity_completed: f64) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .complete(uuid, decimal_from_f64(quantity_completed, "work order quantity completed")?)
            .map_err(|e| Error::from_reason(format!("Failed to complete work order: {}", e)))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel work order: {}", e)))?;

        convert_output(wo)
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .work_orders()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count work orders: {}", e)))?;

        Ok(count as u32)
    }
}

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
    pub unit_price: f64,
    pub original_price: Option<f64>,
    pub weight: Option<f64>,
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
    pub unit_price: f64,
    pub original_price: Option<f64>,
    pub discount_amount: f64,
    pub tax_amount: f64,
    pub total: f64,
    pub requires_shipping: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::CartItem> for CartItemOutput {
    type Error = Error;

    fn try_from(item: stateset_core::CartItem) -> Result<Self> {
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
            unit_price: to_f64_result(item.unit_price, "cart item unit price")?,
            original_price: optional_to_f64_result(
                item.original_price,
                "cart item original price",
            )?,
            discount_amount: to_f64_result(item.discount_amount, "cart item discount amount")?,
            tax_amount: to_f64_result(item.tax_amount, "cart item tax amount")?,
            total: to_f64_result(item.total, "cart item total")?,
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
    pub status: String,
    pub currency: String,
    pub subtotal: f64,
    pub tax_amount: f64,
    pub shipping_amount: f64,
    pub discount_amount: f64,
    pub grand_total: f64,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_name: Option<String>,
    pub shipping_address: Option<CartAddressOutput>,
    pub billing_address: Option<CartAddressOutput>,
    pub billing_same_as_shipping: bool,
    pub fulfillment_type: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    pub payment_method: Option<String>,
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
        Ok(Self {
            id: cart.id.to_string(),
            cart_number: cart.cart_number,
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: format!("{}", cart.status),
            currency: cart.currency.to_string(),
            subtotal: to_f64_result(cart.subtotal, "cart subtotal")?,
            tax_amount: to_f64_result(cart.tax_amount, "cart tax amount")?,
            shipping_amount: to_f64_result(cart.shipping_amount, "cart shipping amount")?,
            discount_amount: to_f64_result(cart.discount_amount, "cart discount amount")?,
            grand_total: to_f64_result(cart.grand_total, "cart grand total")?,
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
    pub total_charged: f64,
    pub currency: String,
}

impl TryFrom<stateset_core::CheckoutResult> for CheckoutResultOutput {
    type Error = Error;

    fn try_from(result: stateset_core::CheckoutResult) -> Result<Self> {
        Ok(Self {
            cart_id: result.cart_id.to_string(),
            order_id: result.order_id.to_string(),
            order_number: result.order_number,
            payment_id: result.payment_id.map(|id| id.to_string()),
            total_charged: to_f64_result(result.total_charged, "checkout total charged")?,
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
    pub price: f64,
    pub currency: String,
    pub estimated_days: Option<i32>,
}

impl TryFrom<stateset_core::ShippingRate> for ShippingRateOutput {
    type Error = Error;

    fn try_from(rate: stateset_core::ShippingRate) -> Result<Self> {
        Ok(Self {
            id: rate.id,
            carrier: rate.carrier,
            service: rate.service,
            description: rate.description,
            price: to_f64_result(rate.price, "shipping rate price")?,
            currency: rate.currency.to_string(),
            estimated_days: rate.estimated_days,
        })
    }
}

fn input_to_cart_address(input: CartAddressInput) -> stateset_core::CartAddress {
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
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Carts {
    /// Create a new cart
    #[napi]
    pub async fn create(&self, input: CreateCartInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id = input
            .customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let cart = commerce
            .carts()
            .create(stateset_core::CreateCart {
                customer_id,
                customer_email: input.customer_email,
                customer_name: input.customer_name,
                currency: input.currency.and_then(|s| s.parse::<CurrencyCode>().ok()),
                shipping_address: input.shipping_address.map(input_to_cart_address),
                billing_address: input.billing_address.map(input_to_cart_address),
                notes: input.notes,
                expires_in_minutes: input.expires_in_minutes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create cart: {}", e)))?;

        convert_output(cart)
    }

    /// Get a cart by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get cart: {}", e)))?;

        convert_optional_output(cart)
    }

    /// Get a cart by cart number
    #[napi]
    pub async fn get_by_number(&self, cart_number: String) -> Result<Option<CartOutput>> {
        let commerce = self.commerce.lock().await;

        let cart = commerce
            .carts()
            .get_by_number(&cart_number)
            .map_err(|e| Error::from_reason(format!("Failed to get cart: {}", e)))?;

        convert_optional_output(cart)
    }

    /// Update a cart
    #[napi]
    pub async fn update(&self, id: String, input: UpdateCartInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

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
            .map_err(|e| Error::from_reason(format!("Failed to update cart: {}", e)))?;

        convert_output(cart)
    }

    /// List all carts
    #[napi]
    pub async fn list(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let carts = commerce
            .carts()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// List carts for a customer
    #[napi]
    pub async fn for_customer(&self, customer_id: String) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let carts = commerce
            .carts()
            .for_customer(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get customer carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// Delete a cart
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        commerce
            .carts()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete cart: {}", e)))?;

        Ok(())
    }

    /// Add an item to the cart
    #[napi]
    pub async fn add_item(
        &self,
        cart_id: String,
        item: AddCartItemInput,
    ) -> Result<CartItemOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| Error::from_reason("Invalid cart UUID"))?;

        let product_id = item
            .product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid product UUID"))?;

        let variant_id = item
            .variant_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid variant UUID"))?;

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
                    unit_price: decimal_from_f64(item.unit_price, "cart item unit price")?,
                    original_price: optional_decimal_from_f64(
                        item.original_price,
                        "cart item original price",
                    )?,
                    weight: optional_decimal_from_f64(item.weight, "cart item weight")?,
                    requires_shipping: item.requires_shipping,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to add item: {}", e)))?;

        convert_output(cart_item)
    }

    /// Update a cart item
    #[napi]
    pub async fn update_item(
        &self,
        item_id: String,
        input: UpdateCartItemInput,
    ) -> Result<CartItemOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = item_id.parse().map_err(|_| Error::from_reason("Invalid item UUID"))?;

        let cart_item = commerce
            .carts()
            .update_item(
                uuid,
                stateset_core::UpdateCartItem {
                    quantity: input.quantity,
                    unit_price: optional_decimal_from_f64(
                        input.unit_price,
                        "cart item unit price",
                    )?,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update item: {}", e)))?;

        convert_output(cart_item)
    }

    /// Remove an item from the cart
    #[napi]
    pub async fn remove_item(&self, item_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = item_id.parse().map_err(|_| Error::from_reason("Invalid item UUID"))?;

        commerce
            .carts()
            .remove_item(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to remove item: {}", e)))?;

        Ok(())
    }

    /// Get items in a cart
    #[napi]
    pub async fn get_items(&self, cart_id: String) -> Result<Vec<CartItemOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| Error::from_reason("Invalid cart UUID"))?;

        let items = commerce
            .carts()
            .get_items(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get items: {}", e)))?;

        convert_outputs(items)
    }

    /// Clear all items from the cart
    #[napi]
    pub async fn clear_items(&self, cart_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| Error::from_reason("Invalid cart UUID"))?;

        commerce
            .carts()
            .clear_items(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to clear items: {}", e)))?;

        Ok(())
    }

    /// Set the shipping address
    #[napi]
    pub async fn set_shipping_address(
        &self,
        id: String,
        address: CartAddressInput,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_shipping_address(uuid.into(), input_to_cart_address(address))
            .map_err(|e| Error::from_reason(format!("Failed to set shipping address: {}", e)))?;

        convert_output(cart)
    }

    /// Set shipping selection (address + method/carrier/amount)
    #[napi]
    pub async fn set_shipping(
        &self,
        id: String,
        input: SetCartShippingInput,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipping_amount = match input.shipping_amount {
            Some(amount) => Some(
                Decimal::from_f64_retain(amount)
                    .ok_or_else(|| Error::from_reason("Invalid shipping amount"))?,
            ),
            None => None,
        };

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
            .map_err(|e| Error::from_reason(format!("Failed to set shipping: {}", e)))?;

        convert_output(cart)
    }

    /// Set the billing address
    #[napi]
    pub async fn set_billing_address(
        &self,
        id: String,
        address: CartAddressInput,
    ) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_billing_address(uuid.into(), input_to_cart_address(address))
            .map_err(|e| Error::from_reason(format!("Failed to set billing address: {}", e)))?;

        convert_output(cart)
    }

    /// Get available shipping rates
    #[napi]
    pub async fn get_shipping_rates(&self, id: String) -> Result<Vec<ShippingRateOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let rates = commerce
            .carts()
            .get_shipping_rates(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get shipping rates: {}", e)))?;

        convert_outputs(rates)
    }

    /// Set payment method
    #[napi]
    pub async fn set_payment(&self, id: String, input: SetCartPaymentInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

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
            .map_err(|e| Error::from_reason(format!("Failed to set payment: {}", e)))?;

        convert_output(cart)
    }

    /// Apply a discount/coupon code
    #[napi]
    pub async fn apply_discount(&self, id: String, coupon_code: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .apply_discount(uuid.into(), &coupon_code)
            .map_err(|e| Error::from_reason(format!("Failed to apply discount: {}", e)))?;

        convert_output(cart)
    }

    /// Remove discount from cart
    #[napi]
    pub async fn remove_discount(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .remove_discount(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to remove discount: {}", e)))?;

        convert_output(cart)
    }

    /// Mark cart as ready for payment
    #[napi]
    pub async fn mark_ready_for_payment(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .mark_ready_for_payment(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to mark ready: {}", e)))?;

        convert_output(cart)
    }

    /// Begin checkout process
    #[napi]
    pub async fn begin_checkout(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .begin_checkout(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to begin checkout: {}", e)))?;

        convert_output(cart)
    }

    /// Complete checkout and create order
    #[napi]
    pub async fn complete(&self, id: String) -> Result<CheckoutResultOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let result = commerce
            .carts()
            .complete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to complete checkout: {}", e)))?;

        convert_output(result)
    }

    /// Cancel a cart
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel cart: {}", e)))?;

        convert_output(cart)
    }

    /// Mark cart as abandoned
    #[napi]
    pub async fn abandon(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .abandon(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to abandon cart: {}", e)))?;

        convert_output(cart)
    }

    /// Mark cart as expired
    #[napi]
    pub async fn expire(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .expire(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to expire cart: {}", e)))?;

        convert_output(cart)
    }

    /// Reserve inventory for cart items
    #[napi]
    pub async fn reserve_inventory(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .reserve_inventory(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to reserve inventory: {}", e)))?;

        convert_output(cart)
    }

    /// Release reserved inventory for cart items
    #[napi]
    pub async fn release_inventory(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .release_inventory(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to release inventory: {}", e)))?;

        convert_output(cart)
    }

    /// Recalculate cart totals
    #[napi]
    pub async fn recalculate(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .recalculate(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to recalculate: {}", e)))?;

        convert_output(cart)
    }

    /// Set tax amount
    #[napi]
    pub async fn set_tax(&self, id: String, tax_amount: f64) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_tax(uuid.into(), decimal_from_f64(tax_amount, "tax amount")?)
            .map_err(|e| Error::from_reason(format!("Failed to set tax: {}", e)))?;

        convert_output(cart)
    }

    /// Get abandoned carts
    #[napi]
    pub async fn get_abandoned(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let carts = commerce
            .carts()
            .get_abandoned()
            .map_err(|e| Error::from_reason(format!("Failed to get abandoned carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// Get expired carts
    #[napi]
    pub async fn get_expired(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let carts = commerce
            .carts()
            .get_expired()
            .map_err(|e| Error::from_reason(format!("Failed to get expired carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// Count carts
    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .carts()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count carts: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Analytics API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AnalyticsQueryInput {
    /// Time period: today, yesterday, last7days, last30days, this_month, last_month, this_quarter,
    /// last_quarter, this_year, last_year, all_time
    pub period: Option<String>,
    /// Granularity: hour, day, week, month, quarter, year
    pub granularity: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct SalesSummaryOutput {
    pub total_revenue: f64,
    pub order_count: u32,
    pub average_order_value: f64,
    pub items_sold: u32,
    pub unique_customers: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct RevenueByPeriodOutput {
    pub period: String,
    pub revenue: f64,
    pub order_count: u32,
    pub period_start: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct TopProductOutput {
    pub product_id: Option<String>,
    pub sku: String,
    pub name: String,
    pub units_sold: u32,
    pub revenue: f64,
    pub order_count: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ProductPerformanceOutput {
    pub product_id: String,
    pub sku: String,
    pub name: String,
    pub units_sold: u32,
    pub revenue: f64,
    pub previous_units_sold: u32,
    pub previous_revenue: f64,
    pub units_growth_percent: f64,
    pub revenue_growth_percent: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct CustomerMetricsOutput {
    pub total_customers: u32,
    pub new_customers: u32,
    pub returning_customers: u32,
    pub average_lifetime_value: f64,
    pub average_orders_per_customer: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct TopCustomerOutput {
    pub customer_id: String,
    pub name: String,
    pub email: String,
    pub order_count: u32,
    pub total_spent: f64,
    pub average_order_value: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct InventoryHealthOutput {
    pub total_skus: u32,
    pub in_stock_skus: u32,
    pub low_stock_skus: u32,
    pub out_of_stock_skus: u32,
    pub total_value: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct LowStockItemOutput {
    pub sku: String,
    pub name: String,
    pub on_hand: f64,
    pub allocated: f64,
    pub available: f64,
    pub reorder_point: Option<f64>,
    pub average_daily_sales: Option<f64>,
    pub days_of_stock: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct InventoryMovementOutput {
    pub sku: String,
    pub name: String,
    pub units_sold: u32,
    pub units_received: u32,
    pub units_returned: u32,
    pub units_adjusted: i32,
    pub net_change: i32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct DemandForecastOutput {
    pub sku: String,
    pub name: String,
    pub average_daily_demand: f64,
    pub forecasted_demand: f64,
    pub confidence: f64,
    pub current_stock: f64,
    pub days_until_stockout: Option<i32>,
    pub recommended_reorder_qty: Option<f64>,
    pub trend: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct RevenueForecastOutput {
    pub period: String,
    pub forecasted_revenue: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub confidence_level: f64,
    pub based_on_periods: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct OrderStatusBreakdownOutput {
    pub pending: u32,
    pub confirmed: u32,
    pub processing: u32,
    pub shipped: u32,
    pub delivered: u32,
    pub cancelled: u32,
    pub refunded: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct FulfillmentMetricsOutput {
    pub avg_time_to_ship_hours: Option<f64>,
    pub avg_time_to_deliver_hours: Option<f64>,
    pub on_time_shipping_percent: Option<f64>,
    pub on_time_delivery_percent: Option<f64>,
    pub shipped_today: u32,
    pub awaiting_shipment: u32,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ReturnMetricsOutput {
    pub total_returns: u32,
    pub return_rate_percent: f64,
    pub total_refunded: f64,
}

/// Analytics and forecasting API
#[napi]
pub struct Analytics {
    commerce: Arc<Mutex<RustCommerce>>,
}

fn parse_period(period: &str) -> stateset_embedded::TimePeriod {
    match period.to_lowercase().as_str() {
        "today" => stateset_embedded::TimePeriod::Today,
        "yesterday" => stateset_embedded::TimePeriod::Yesterday,
        "last7days" | "last_7_days" => stateset_embedded::TimePeriod::Last7Days,
        "last30days" | "last_30_days" => stateset_embedded::TimePeriod::Last30Days,
        "this_month" | "thismonth" => stateset_embedded::TimePeriod::ThisMonth,
        "last_month" | "lastmonth" => stateset_embedded::TimePeriod::LastMonth,
        "this_quarter" | "thisquarter" => stateset_embedded::TimePeriod::ThisQuarter,
        "last_quarter" | "lastquarter" => stateset_embedded::TimePeriod::LastQuarter,
        "this_year" | "thisyear" => stateset_embedded::TimePeriod::ThisYear,
        "last_year" | "lastyear" => stateset_embedded::TimePeriod::LastYear,
        "all_time" | "alltime" | "all" => stateset_embedded::TimePeriod::AllTime,
        _ => stateset_embedded::TimePeriod::Last30Days,
    }
}

fn parse_granularity(granularity: &str) -> stateset_embedded::TimeGranularity {
    match granularity.to_lowercase().as_str() {
        "hour" | "hourly" => stateset_embedded::TimeGranularity::Hour,
        "day" | "daily" => stateset_embedded::TimeGranularity::Day,
        "week" | "weekly" => stateset_embedded::TimeGranularity::Week,
        "month" | "monthly" => stateset_embedded::TimeGranularity::Month,
        "quarter" | "quarterly" => stateset_embedded::TimeGranularity::Quarter,
        "year" | "yearly" => stateset_embedded::TimeGranularity::Year,
        _ => stateset_embedded::TimeGranularity::Day,
    }
}

#[napi]
impl Analytics {
    /// Get sales summary for a time period
    #[napi]
    pub async fn sales_summary(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<SalesSummaryOutput> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
            if let Some(limit) = input.limit {
                q = q.limit(limit);
            }
        }

        let summary = commerce
            .analytics()
            .sales_summary(q)
            .map_err(|e| Error::from_reason(format!("Failed to get sales summary: {}", e)))?;

        Ok(SalesSummaryOutput {
            total_revenue: to_f64_or_nan(summary.total_revenue),
            order_count: summary.order_count as u32,
            average_order_value: to_f64_or_nan(summary.average_order_value),
            items_sold: summary.items_sold as u32,
            unique_customers: summary.unique_customers as u32,
        })
    }

    /// Get revenue broken down by time periods
    #[napi]
    pub async fn revenue_by_period(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<RevenueByPeriodOutput>> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
            if let Some(ref granularity) = input.granularity {
                q = q.granularity(parse_granularity(granularity));
            }
        }

        let revenue = commerce
            .analytics()
            .revenue_by_period(q)
            .map_err(|e| Error::from_reason(format!("Failed to get revenue: {}", e)))?;

        Ok(revenue
            .into_iter()
            .map(|r| RevenueByPeriodOutput {
                period: r.period,
                revenue: to_f64_or_nan(r.revenue),
                order_count: r.order_count as u32,
                period_start: r.period_start.to_rfc3339(),
            })
            .collect())
    }

    /// Get top selling products
    #[napi]
    pub async fn top_products(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<TopProductOutput>> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
            if let Some(limit) = input.limit {
                q = q.limit(limit);
            }
        }

        let products = commerce
            .analytics()
            .top_products(q)
            .map_err(|e| Error::from_reason(format!("Failed to get top products: {}", e)))?;

        Ok(products
            .into_iter()
            .map(|p| TopProductOutput {
                product_id: p.product_id.map(|id| id.to_string()),
                sku: p.sku,
                name: p.name,
                units_sold: p.units_sold as u32,
                revenue: to_f64_or_nan(p.revenue),
                order_count: p.order_count as u32,
            })
            .collect())
    }

    /// Get product performance with period comparison
    #[napi]
    pub async fn product_performance(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<ProductPerformanceOutput>> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
            if let Some(limit) = input.limit {
                q = q.limit(limit);
            }
        }

        let perf = commerce
            .analytics()
            .product_performance(q)
            .map_err(|e| Error::from_reason(format!("Failed to get product performance: {}", e)))?;

        Ok(perf
            .into_iter()
            .map(|p| ProductPerformanceOutput {
                product_id: p.product_id.to_string(),
                sku: p.sku,
                name: p.name,
                units_sold: p.units_sold as u32,
                revenue: to_f64_or_nan(p.revenue),
                previous_units_sold: p.previous_units_sold as u32,
                previous_revenue: to_f64_or_nan(p.previous_revenue),
                units_growth_percent: to_f64_or_nan(p.units_growth_percent),
                revenue_growth_percent: to_f64_or_nan(p.revenue_growth_percent),
            })
            .collect())
    }

    /// Get customer metrics
    #[napi]
    pub async fn customer_metrics(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<CustomerMetricsOutput> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
        }

        let metrics = commerce
            .analytics()
            .customer_metrics(q)
            .map_err(|e| Error::from_reason(format!("Failed to get customer metrics: {}", e)))?;

        Ok(CustomerMetricsOutput {
            total_customers: metrics.total_customers as u32,
            new_customers: metrics.new_customers as u32,
            returning_customers: metrics.returning_customers as u32,
            average_lifetime_value: to_f64_or_nan(metrics.average_lifetime_value),
            average_orders_per_customer: to_f64_or_nan(metrics.average_orders_per_customer),
        })
    }

    /// Get top customers by spend
    #[napi]
    pub async fn top_customers(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<TopCustomerOutput>> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
            if let Some(limit) = input.limit {
                q = q.limit(limit);
            }
        }

        let customers = commerce
            .analytics()
            .top_customers(q)
            .map_err(|e| Error::from_reason(format!("Failed to get top customers: {}", e)))?;

        Ok(customers
            .into_iter()
            .map(|c| TopCustomerOutput {
                customer_id: c.customer_id.to_string(),
                name: c.name,
                email: c.email,
                order_count: c.order_count as u32,
                total_spent: to_f64_or_nan(c.total_spent),
                average_order_value: to_f64_or_nan(c.average_order_value),
            })
            .collect())
    }

    /// Get inventory health summary
    #[napi]
    pub async fn inventory_health(&self) -> Result<InventoryHealthOutput> {
        let commerce = self.commerce.lock().await;

        let health = commerce
            .analytics()
            .inventory_health()
            .map_err(|e| Error::from_reason(format!("Failed to get inventory health: {}", e)))?;

        Ok(InventoryHealthOutput {
            total_skus: health.total_skus as u32,
            in_stock_skus: health.in_stock_skus as u32,
            low_stock_skus: health.low_stock_skus as u32,
            out_of_stock_skus: health.out_of_stock_skus as u32,
            total_value: to_f64_or_nan(health.total_value),
        })
    }

    /// Get low stock items
    #[napi]
    pub async fn low_stock_items(&self, threshold: Option<f64>) -> Result<Vec<LowStockItemOutput>> {
        let commerce = self.commerce.lock().await;

        let threshold_dec = optional_decimal_from_f64(threshold, "low stock threshold")?;

        let items = commerce
            .analytics()
            .low_stock_items(threshold_dec)
            .map_err(|e| Error::from_reason(format!("Failed to get low stock items: {}", e)))?;

        Ok(items
            .into_iter()
            .map(|i| LowStockItemOutput {
                sku: i.sku,
                name: i.name,
                on_hand: to_f64_or_nan(i.on_hand),
                allocated: to_f64_or_nan(i.allocated),
                available: to_f64_or_nan(i.available),
                reorder_point: i.reorder_point.map(to_f64_or_nan),
                average_daily_sales: i.average_daily_sales.map(to_f64_or_nan),
                days_of_stock: i.days_of_stock.map(to_f64_or_nan),
            })
            .collect())
    }

    /// Get inventory movement summary
    #[napi]
    pub async fn inventory_movement(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<Vec<InventoryMovementOutput>> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
        }

        let movements = commerce
            .analytics()
            .inventory_movement(q)
            .map_err(|e| Error::from_reason(format!("Failed to get inventory movement: {}", e)))?;

        Ok(movements
            .into_iter()
            .map(|m| InventoryMovementOutput {
                sku: m.sku,
                name: m.name,
                units_sold: m.units_sold as u32,
                units_received: m.units_received as u32,
                units_returned: m.units_returned as u32,
                units_adjusted: m.units_adjusted as i32,
                net_change: m.net_change as i32,
            })
            .collect())
    }

    /// Get demand forecast for inventory items
    #[napi]
    pub async fn demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: Option<u32>,
    ) -> Result<Vec<DemandForecastOutput>> {
        let commerce = self.commerce.lock().await;

        let forecasts = commerce
            .analytics()
            .demand_forecast(skus, days_ahead.unwrap_or(30))
            .map_err(|e| Error::from_reason(format!("Failed to get demand forecast: {}", e)))?;

        Ok(forecasts
            .into_iter()
            .map(|f| DemandForecastOutput {
                sku: f.sku,
                name: f.name,
                average_daily_demand: to_f64_or_nan(f.average_daily_demand),
                forecasted_demand: to_f64_or_nan(f.forecasted_demand),
                confidence: to_f64_or_nan(f.confidence),
                current_stock: to_f64_or_nan(f.current_stock),
                days_until_stockout: f.days_until_stockout,
                recommended_reorder_qty: f.recommended_reorder_qty.map(to_f64_or_nan),
                trend: format!("{:?}", f.trend),
            })
            .collect())
    }

    /// Get revenue forecast
    #[napi]
    pub async fn revenue_forecast(
        &self,
        periods_ahead: Option<u32>,
        granularity: Option<String>,
    ) -> Result<Vec<RevenueForecastOutput>> {
        let commerce = self.commerce.lock().await;

        let gran = granularity
            .map(|g| parse_granularity(&g))
            .unwrap_or(stateset_embedded::TimeGranularity::Month);

        let forecasts = commerce
            .analytics()
            .revenue_forecast(periods_ahead.unwrap_or(3), gran)
            .map_err(|e| Error::from_reason(format!("Failed to get revenue forecast: {}", e)))?;

        Ok(forecasts
            .into_iter()
            .map(|f| RevenueForecastOutput {
                period: f.period,
                forecasted_revenue: to_f64_or_nan(f.forecasted_revenue),
                lower_bound: to_f64_or_nan(f.lower_bound),
                upper_bound: to_f64_or_nan(f.upper_bound),
                confidence_level: to_f64_or_nan(f.confidence_level),
                based_on_periods: f.based_on_periods,
            })
            .collect())
    }

    /// Get order status breakdown
    #[napi]
    pub async fn order_status_breakdown(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<OrderStatusBreakdownOutput> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
        }

        let breakdown = commerce.analytics().order_status_breakdown(q).map_err(|e| {
            Error::from_reason(format!("Failed to get order status breakdown: {}", e))
        })?;

        Ok(OrderStatusBreakdownOutput {
            pending: breakdown.pending as u32,
            confirmed: breakdown.confirmed as u32,
            processing: breakdown.processing as u32,
            shipped: breakdown.shipped as u32,
            delivered: breakdown.delivered as u32,
            cancelled: breakdown.cancelled as u32,
            refunded: breakdown.refunded as u32,
        })
    }

    /// Get fulfillment metrics
    #[napi]
    pub async fn fulfillment_metrics(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<FulfillmentMetricsOutput> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
        }

        let metrics = commerce
            .analytics()
            .fulfillment_metrics(q)
            .map_err(|e| Error::from_reason(format!("Failed to get fulfillment metrics: {}", e)))?;

        Ok(FulfillmentMetricsOutput {
            avg_time_to_ship_hours: metrics.avg_time_to_ship_hours.map(to_f64_or_nan),
            avg_time_to_deliver_hours: metrics.avg_time_to_deliver_hours.map(to_f64_or_nan),
            on_time_shipping_percent: metrics.on_time_shipping_percent.map(to_f64_or_nan),
            on_time_delivery_percent: metrics.on_time_delivery_percent.map(to_f64_or_nan),
            shipped_today: metrics.shipped_today as u32,
            awaiting_shipment: metrics.awaiting_shipment as u32,
        })
    }

    /// Get return metrics
    #[napi]
    pub async fn return_metrics(
        &self,
        query: Option<AnalyticsQueryInput>,
    ) -> Result<ReturnMetricsOutput> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
        }

        let metrics = commerce
            .analytics()
            .return_metrics(q)
            .map_err(|e| Error::from_reason(format!("Failed to get return metrics: {}", e)))?;

        Ok(ReturnMetricsOutput {
            total_returns: metrics.total_returns as u32,
            return_rate_percent: to_f64_or_nan(metrics.return_rate_percent),
            total_refunded: to_f64_or_nan(metrics.total_refunded),
        })
    }
}

// ============================================================================
// Currency API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SetExchangeRateInput {
    /// Base currency code (e.g., "USD")
    pub base_currency: String,
    /// Quote currency code (e.g., "EUR")
    pub quote_currency: String,
    /// Exchange rate (e.g., 0.92 for USD to EUR)
    pub rate: f64,
    /// Optional source of the rate (e.g., "manual", "api")
    pub source: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ConvertCurrencyInput {
    /// Source currency code
    pub from: String,
    /// Target currency code
    pub to: String,
    /// Amount to convert
    pub amount: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ExchangeRateFilterInput {
    /// Filter by base currency
    pub base_currency: Option<String>,
    /// Filter by quote currency
    pub quote_currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ExchangeRateOutput {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: f64,
    pub source: String,
    pub rate_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ConversionResultOutput {
    pub original_amount: f64,
    pub original_currency: String,
    pub converted_amount: f64,
    pub target_currency: String,
    pub rate: f64,
    pub inverse_rate: f64,
    pub rate_at: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCurrencySettingsInput {
    /// Base currency for the store
    pub base_currency: String,
    /// List of enabled currency codes
    pub enabled_currencies: Vec<String>,
    /// Whether to auto-convert prices
    pub auto_convert: Option<bool>,
    /// Rounding mode: "half_up", "half_down", "up", "down", "half_even"
    pub rounding_mode: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct StoreCurrencySettingsOutput {
    pub base_currency: String,
    pub enabled_currencies: Vec<String>,
    pub auto_convert: bool,
    pub rounding_mode: String,
}

fn parse_currency(code: &str) -> Result<stateset_embedded::Currency> {
    use std::str::FromStr;
    stateset_embedded::Currency::from_str(code)
        .map_err(|e| Error::from_reason(format!("Invalid currency code '{}': {}", code, e)))
}

fn parse_rounding_mode(mode: &str) -> stateset_embedded::RoundingMode {
    match mode.to_lowercase().as_str() {
        "half_down" => stateset_embedded::RoundingMode::HalfDown,
        "up" => stateset_embedded::RoundingMode::Up,
        "down" => stateset_embedded::RoundingMode::Down,
        "half_even" => stateset_embedded::RoundingMode::HalfEven,
        _ => stateset_embedded::RoundingMode::HalfUp,
    }
}

fn rounding_mode_to_string(mode: &stateset_embedded::RoundingMode) -> String {
    match mode {
        stateset_embedded::RoundingMode::HalfUp => "half_up".to_string(),
        stateset_embedded::RoundingMode::HalfDown => "half_down".to_string(),
        stateset_embedded::RoundingMode::Up => "up".to_string(),
        stateset_embedded::RoundingMode::Down => "down".to_string(),
        stateset_embedded::RoundingMode::HalfEven => "half_even".to_string(),
        &_ => "half_up".to_string(),
    }
}

fn exchange_rate_to_output(rate: stateset_embedded::ExchangeRate) -> ExchangeRateOutput {
    ExchangeRateOutput {
        id: rate.id.to_string(),
        base_currency: rate.base_currency.code().to_string(),
        quote_currency: rate.quote_currency.code().to_string(),
        rate: to_f64_or_nan(rate.rate),
        source: rate.source,
        rate_at: rate.rate_at.to_rfc3339(),
        created_at: rate.created_at.to_rfc3339(),
        updated_at: rate.updated_at.to_rfc3339(),
    }
}

/// Currency and exchange rate operations API
#[napi]
pub struct CurrencyOperations {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl CurrencyOperations {
    /// Get exchange rate between two currencies
    #[napi]
    pub async fn get_rate(&self, from: String, to: String) -> Result<Option<ExchangeRateOutput>> {
        let commerce = self.commerce.lock().await;
        let from_currency = parse_currency(&from)?;
        let to_currency = parse_currency(&to)?;

        let rate = commerce
            .currency()
            .get_rate(from_currency, to_currency)
            .map_err(|e| Error::from_reason(format!("Failed to get rate: {}", e)))?;

        Ok(rate.map(exchange_rate_to_output))
    }

    /// Get all exchange rates for a base currency
    #[napi]
    pub async fn get_rates_for(&self, base_currency: String) -> Result<Vec<ExchangeRateOutput>> {
        let commerce = self.commerce.lock().await;
        let currency = parse_currency(&base_currency)?;

        let rates = commerce
            .currency()
            .get_rates_for(currency)
            .map_err(|e| Error::from_reason(format!("Failed to get rates: {}", e)))?;

        Ok(rates.into_iter().map(exchange_rate_to_output).collect())
    }

    /// List exchange rates with optional filtering
    #[napi]
    pub async fn list_rates(
        &self,
        filter: Option<ExchangeRateFilterInput>,
    ) -> Result<Vec<ExchangeRateOutput>> {
        let commerce = self.commerce.lock().await;

        let mut f = stateset_embedded::ExchangeRateFilter::default();
        if let Some(ref input) = filter {
            if let Some(ref base) = input.base_currency {
                f.base_currency = Some(parse_currency(base)?);
            }
            if let Some(ref quote) = input.quote_currency {
                f.quote_currency = Some(parse_currency(quote)?);
            }
        }

        let rates = commerce
            .currency()
            .list_rates(f)
            .map_err(|e| Error::from_reason(format!("Failed to list rates: {}", e)))?;

        Ok(rates.into_iter().map(exchange_rate_to_output).collect())
    }

    /// Set an exchange rate
    #[napi]
    pub async fn set_rate(&self, input: SetExchangeRateInput) -> Result<ExchangeRateOutput> {
        let commerce = self.commerce.lock().await;

        let rate = commerce
            .currency()
            .set_rate(stateset_embedded::SetExchangeRate {
                base_currency: parse_currency(&input.base_currency)?,
                quote_currency: parse_currency(&input.quote_currency)?,
                rate: Decimal::try_from(input.rate)
                    .map_err(|e| Error::from_reason(format!("Invalid rate: {}", e)))?,
                source: input.source,
            })
            .map_err(|e| Error::from_reason(format!("Failed to set rate: {}", e)))?;

        Ok(exchange_rate_to_output(rate))
    }

    /// Set multiple exchange rates at once
    #[napi]
    pub async fn set_rates(
        &self,
        inputs: Vec<SetExchangeRateInput>,
    ) -> Result<Vec<ExchangeRateOutput>> {
        let commerce = self.commerce.lock().await;

        let mut rates = Vec::new();
        for input in inputs {
            rates.push(stateset_embedded::SetExchangeRate {
                base_currency: parse_currency(&input.base_currency)?,
                quote_currency: parse_currency(&input.quote_currency)?,
                rate: Decimal::try_from(input.rate)
                    .map_err(|e| Error::from_reason(format!("Invalid rate: {}", e)))?,
                source: input.source,
            });
        }

        let results = commerce
            .currency()
            .set_rates(rates)
            .map_err(|e| Error::from_reason(format!("Failed to set rates: {}", e)))?;

        Ok(results.into_iter().map(exchange_rate_to_output).collect())
    }

    /// Delete an exchange rate by ID
    #[napi]
    pub async fn delete_rate(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let rate_id = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid rate ID: {}", e)))?;

        commerce
            .currency()
            .delete_rate(rate_id)
            .map_err(|e| Error::from_reason(format!("Failed to delete rate: {}", e)))?;

        Ok(true)
    }

    /// Convert an amount from one currency to another
    #[napi]
    pub async fn convert(&self, input: ConvertCurrencyInput) -> Result<ConversionResultOutput> {
        let commerce = self.commerce.lock().await;

        let result = commerce
            .currency()
            .convert(stateset_embedded::ConvertCurrency {
                from: parse_currency(&input.from)?,
                to: parse_currency(&input.to)?,
                amount: Decimal::try_from(input.amount)
                    .map_err(|e| Error::from_reason(format!("Invalid amount: {}", e)))?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to convert currency: {}", e)))?;

        Ok(ConversionResultOutput {
            original_amount: to_f64_or_nan(result.original_amount),
            original_currency: result.original_currency.code().to_string(),
            converted_amount: to_f64_or_nan(result.converted_amount),
            target_currency: result.target_currency.code().to_string(),
            rate: to_f64_or_nan(result.rate),
            inverse_rate: to_f64_or_nan(result.inverse_rate),
            rate_at: result.rate_at.to_rfc3339(),
        })
    }

    /// Get store currency settings
    #[napi]
    pub async fn get_settings(&self) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.lock().await;

        let settings = commerce
            .currency()
            .get_settings()
            .map_err(|e| Error::from_reason(format!("Failed to get settings: {}", e)))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Update store currency settings
    #[napi]
    pub async fn update_settings(
        &self,
        input: StoreCurrencySettingsInput,
    ) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.lock().await;

        let mut enabled = Vec::new();
        for code in &input.enabled_currencies {
            enabled.push(parse_currency(code)?);
        }

        let settings = commerce
            .currency()
            .update_settings(stateset_embedded::StoreCurrencySettings {
                base_currency: parse_currency(&input.base_currency)?,
                enabled_currencies: enabled,
                auto_convert: input.auto_convert.unwrap_or(true),
                rounding_mode: input
                    .rounding_mode
                    .as_deref()
                    .map(parse_rounding_mode)
                    .unwrap_or_default(),
            })
            .map_err(|e| Error::from_reason(format!("Failed to update settings: {}", e)))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Set the store's base currency
    #[napi]
    pub async fn set_base_currency(
        &self,
        currency_code: String,
    ) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.lock().await;
        let currency = parse_currency(&currency_code)?;

        let settings = commerce
            .currency()
            .set_base_currency(currency)
            .map_err(|e| Error::from_reason(format!("Failed to set base currency: {}", e)))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Enable currencies for the store
    #[napi]
    pub async fn enable_currencies(
        &self,
        currency_codes: Vec<String>,
    ) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.lock().await;

        let mut currencies = Vec::new();
        for code in &currency_codes {
            currencies.push(parse_currency(code)?);
        }

        let settings = commerce
            .currency()
            .enable_currencies(currencies)
            .map_err(|e| Error::from_reason(format!("Failed to enable currencies: {}", e)))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Check if a currency is enabled
    #[napi]
    pub async fn is_enabled(&self, currency_code: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let currency = parse_currency(&currency_code)?;

        commerce
            .currency()
            .is_enabled(currency)
            .map_err(|e| Error::from_reason(format!("Failed to check currency: {}", e)))
    }

    /// Get the store's base currency
    #[napi]
    pub async fn get_base_currency(&self) -> Result<String> {
        let commerce = self.commerce.lock().await;

        let currency = commerce
            .currency()
            .base_currency()
            .map_err(|e| Error::from_reason(format!("Failed to get base currency: {}", e)))?;

        Ok(currency.code().to_string())
    }

    /// Get all enabled currencies
    #[napi]
    pub async fn get_enabled_currencies(&self) -> Result<Vec<String>> {
        let commerce = self.commerce.lock().await;

        let currencies = commerce
            .currency()
            .enabled_currencies()
            .map_err(|e| Error::from_reason(format!("Failed to get enabled currencies: {}", e)))?;

        Ok(currencies.iter().map(|c| c.code().to_string()).collect())
    }

    /// Format an amount with currency symbol
    #[napi]
    pub async fn format(&self, amount: f64, currency_code: String) -> Result<String> {
        let commerce = self.commerce.lock().await;
        let currency = parse_currency(&currency_code)?;
        let amount_decimal = Decimal::try_from(amount)
            .map_err(|e| Error::from_reason(format!("Invalid amount: {}", e)))?;

        Ok(commerce.currency().format(amount_decimal, currency))
    }
}

// ============================================================================
// Subscriptions API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSubscriptionPlanInput {
    pub name: String,
    pub description: Option<String>,
    pub code: Option<String>,
    pub billing_interval: String,
    pub custom_interval_days: Option<i32>,
    pub price: f64,
    pub setup_fee: Option<f64>,
    pub currency: Option<String>,
    pub trial_days: Option<i32>,
    pub trial_requires_payment_method: Option<bool>,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSubscriptionPlanInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub setup_fee: Option<f64>,
    pub trial_days: Option<i32>,
    pub trial_requires_payment_method: Option<bool>,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SubscriptionPlanFilterInput {
    pub status: Option<String>,
    pub billing_interval: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionPlanOutput {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub billing_interval: String,
    pub custom_interval_days: Option<i32>,
    pub price: f64,
    pub setup_fee: Option<f64>,
    pub currency: String,
    pub trial_days: i32,
    pub trial_requires_payment_method: bool,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::SubscriptionPlan> for SubscriptionPlanOutput {
    type Error = Error;

    fn try_from(p: stateset_core::SubscriptionPlan) -> Result<Self> {
        Ok(Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            status: format!("{:?}", p.status).to_lowercase(),
            billing_interval: format!("{}", p.billing_interval),
            custom_interval_days: p.custom_interval_days,
            price: to_f64_result(p.price, "subscription plan price")?,
            setup_fee: optional_to_f64_result(p.setup_fee, "subscription plan setup fee")?,
            currency: p.currency.to_string(),
            trial_days: p.trial_days,
            trial_requires_payment_method: p.trial_requires_payment_method,
            min_cycles: p.min_cycles,
            max_cycles: p.max_cycles,
            discount_percent: optional_to_f64_result(
                p.discount_percent,
                "subscription plan discount percent",
            )?,
            discount_amount: optional_to_f64_result(
                p.discount_amount,
                "subscription plan discount amount",
            )?,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSubscriptionInput {
    pub customer_id: String,
    pub plan_id: String,
    pub payment_method_id: Option<String>,
    pub skip_trial: Option<bool>,
    pub price: Option<f64>,
    pub coupon_code: Option<String>,
    pub start_date: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSubscriptionInput {
    pub status: Option<String>,
    pub price: Option<f64>,
    pub payment_method_id: Option<String>,
    pub next_billing_date: Option<String>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
    pub coupon_code: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SubscriptionFilterInput {
    pub customer_id: Option<String>,
    pub plan_id: Option<String>,
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionOutput {
    pub id: String,
    pub subscription_number: String,
    pub customer_id: String,
    pub plan_id: String,
    pub plan_name: String,
    pub status: String,
    pub billing_interval: String,
    pub custom_interval_days: Option<i32>,
    pub price: f64,
    pub currency: String,
    pub payment_method_id: Option<String>,
    pub started_at: String,
    pub current_period_start: String,
    pub current_period_end: String,
    pub next_billing_date: Option<String>,
    pub trial_ends_at: Option<String>,
    pub paused_at: Option<String>,
    pub resume_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub ends_at: Option<String>,
    pub billing_cycle_count: i32,
    pub failed_payment_attempts: i32,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
    pub coupon_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Subscription> for SubscriptionOutput {
    type Error = Error;

    fn try_from(s: stateset_core::Subscription) -> Result<Self> {
        Ok(Self {
            id: s.id.to_string(),
            subscription_number: s.subscription_number,
            customer_id: s.customer_id.to_string(),
            plan_id: s.plan_id.to_string(),
            plan_name: s.plan_name,
            status: format!("{}", s.status),
            billing_interval: format!("{}", s.billing_interval),
            custom_interval_days: s.custom_interval_days,
            price: to_f64_result(s.price, "subscription price")?,
            currency: s.currency.to_string(),
            payment_method_id: s.payment_method_id,
            started_at: s.started_at.to_rfc3339(),
            current_period_start: s.current_period_start.to_rfc3339(),
            current_period_end: s.current_period_end.to_rfc3339(),
            next_billing_date: s.next_billing_date.map(|d| d.to_rfc3339()),
            trial_ends_at: s.trial_ends_at.map(|d| d.to_rfc3339()),
            paused_at: s.paused_at.map(|d| d.to_rfc3339()),
            resume_at: s.resume_at.map(|d| d.to_rfc3339()),
            cancelled_at: s.cancelled_at.map(|d| d.to_rfc3339()),
            ends_at: s.ends_at.map(|d| d.to_rfc3339()),
            billing_cycle_count: s.billing_cycle_count,
            failed_payment_attempts: s.failed_payment_attempts,
            discount_percent: optional_to_f64_result(
                s.discount_percent,
                "subscription discount percent",
            )?,
            discount_amount: optional_to_f64_result(
                s.discount_amount,
                "subscription discount amount",
            )?,
            coupon_code: s.coupon_code,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PauseSubscriptionInput {
    pub reason: Option<String>,
    pub resume_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CancelSubscriptionInput {
    pub reason: Option<String>,
    pub immediate: Option<bool>,
    pub feedback: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SkipBillingCycleInput {
    pub reason: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BillingCycleFilterInput {
    pub subscription_id: Option<String>,
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillingCycleOutput {
    pub id: String,
    pub subscription_id: String,
    pub cycle_number: i32,
    pub status: String,
    pub period_start: String,
    pub period_end: String,
    pub subtotal: f64,
    pub discount: f64,
    pub tax: f64,
    pub total: f64,
    pub currency: String,
    pub payment_id: Option<String>,
    pub billed_at: Option<String>,
    pub failure_reason: Option<String>,
    pub retry_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::BillingCycle> for BillingCycleOutput {
    fn from(b: stateset_core::BillingCycle) -> Self {
        Self {
            id: b.id.to_string(),
            subscription_id: b.subscription_id.to_string(),
            cycle_number: b.cycle_number,
            status: format!("{:?}", b.status).to_lowercase(),
            period_start: b.period_start.to_rfc3339(),
            period_end: b.period_end.to_rfc3339(),
            subtotal: to_f64_or_nan(b.subtotal),
            discount: to_f64_or_nan(b.discount),
            tax: to_f64_or_nan(b.tax),
            total: to_f64_or_nan(b.total),
            currency: b.currency.to_string(),
            payment_id: b.payment_id,
            billed_at: b.billed_at.map(|d| d.to_rfc3339()),
            failure_reason: b.failure_reason,
            retry_count: b.retry_count,
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionEventOutput {
    pub id: String,
    pub subscription_id: String,
    pub event_type: String,
    pub description: String,
    pub data: Option<String>,
    pub triggered_by: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::SubscriptionEvent> for SubscriptionEventOutput {
    fn from(e: stateset_core::SubscriptionEvent) -> Self {
        Self {
            id: e.id.to_string(),
            subscription_id: e.subscription_id.to_string(),
            event_type: format!("{:?}", e.event_type).to_lowercase(),
            description: e.description,
            data: e.data.map(|d| serde_json::to_string(&d).unwrap_or_default()),
            triggered_by: e.triggered_by,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

fn parse_billing_interval(s: &str) -> Result<stateset_core::BillingInterval> {
    match s.to_lowercase().as_str() {
        "weekly" => Ok(stateset_core::BillingInterval::Weekly),
        "biweekly" => Ok(stateset_core::BillingInterval::Biweekly),
        "monthly" => Ok(stateset_core::BillingInterval::Monthly),
        "bimonthly" => Ok(stateset_core::BillingInterval::Bimonthly),
        "quarterly" => Ok(stateset_core::BillingInterval::Quarterly),
        "semiannual" => Ok(stateset_core::BillingInterval::Semiannual),
        "annual" => Ok(stateset_core::BillingInterval::Annual),
        "custom" => Ok(stateset_core::BillingInterval::Custom),
        _ => Err(Error::from_reason(format!("Invalid billing interval: {}", s))),
    }
}

fn parse_plan_status(s: &str) -> Result<stateset_core::PlanStatus> {
    match s.to_lowercase().as_str() {
        "draft" => Ok(stateset_core::PlanStatus::Draft),
        "active" => Ok(stateset_core::PlanStatus::Active),
        "archived" => Ok(stateset_core::PlanStatus::Archived),
        _ => Err(Error::from_reason(format!("Invalid plan status: {}", s))),
    }
}

fn parse_subscription_status(s: &str) -> Result<stateset_core::SubscriptionStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(stateset_core::SubscriptionStatus::Pending),
        "trial" => Ok(stateset_core::SubscriptionStatus::Trial),
        "active" => Ok(stateset_core::SubscriptionStatus::Active),
        "paused" => Ok(stateset_core::SubscriptionStatus::Paused),
        "past_due" => Ok(stateset_core::SubscriptionStatus::PastDue),
        "cancelled" => Ok(stateset_core::SubscriptionStatus::Cancelled),
        "expired" => Ok(stateset_core::SubscriptionStatus::Expired),
        _ => Err(Error::from_reason(format!("Invalid subscription status: {}", s))),
    }
}

fn parse_billing_cycle_status(s: &str) -> Result<stateset_core::BillingCycleStatus> {
    match s.to_lowercase().as_str() {
        "scheduled" => Ok(stateset_core::BillingCycleStatus::Scheduled),
        "processing" => Ok(stateset_core::BillingCycleStatus::Processing),
        "paid" => Ok(stateset_core::BillingCycleStatus::Paid),
        "failed" => Ok(stateset_core::BillingCycleStatus::Failed),
        "skipped" => Ok(stateset_core::BillingCycleStatus::Skipped),
        "refunded" => Ok(stateset_core::BillingCycleStatus::Refunded),
        "voided" => Ok(stateset_core::BillingCycleStatus::Voided),
        _ => Err(Error::from_reason(format!("Invalid billing cycle status: {}", s))),
    }
}

#[napi]
pub struct Subscriptions {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Subscriptions {
    // ========================================================================
    // Subscription Plans
    // ========================================================================

    /// Create a new subscription plan
    #[napi]
    pub async fn create_plan(
        &self,
        input: CreateSubscriptionPlanInput,
    ) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.lock().await;
        let billing_interval = parse_billing_interval(&input.billing_interval)?;

        let plan = commerce
            .subscriptions()
            .create_plan(stateset_core::CreateSubscriptionPlan {
                name: input.name,
                description: input.description,
                code: input.code,
                billing_interval,
                custom_interval_days: input.custom_interval_days,
                price: decimal_from_f64(input.price, "subscription plan price")?,
                setup_fee: optional_decimal_from_f64(
                    input.setup_fee,
                    "subscription plan setup fee",
                )?,
                currency: input.currency.and_then(|s| s.parse::<CurrencyCode>().ok()),
                trial_days: input.trial_days,
                trial_requires_payment_method: input.trial_requires_payment_method,
                min_cycles: input.min_cycles,
                max_cycles: input.max_cycles,
                discount_percent: optional_decimal_from_f64(
                    input.discount_percent,
                    "subscription plan discount percent",
                )?,
                discount_amount: optional_decimal_from_f64(
                    input.discount_amount,
                    "subscription plan discount amount",
                )?,
                items: None,
                metadata: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create plan: {}", e)))?;

        convert_output(plan)
    }

    /// Get a subscription plan by ID
    #[napi]
    pub async fn get_plan(&self, id: String) -> Result<Option<SubscriptionPlanOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let plan = commerce
            .subscriptions()
            .get_plan(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get plan: {}", e)))?;

        convert_optional_output(plan)
    }

    /// Get a subscription plan by code
    #[napi]
    pub async fn get_plan_by_code(&self, code: String) -> Result<Option<SubscriptionPlanOutput>> {
        let commerce = self.commerce.lock().await;

        let plan = commerce
            .subscriptions()
            .get_plan_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get plan: {}", e)))?;

        convert_optional_output(plan)
    }

    /// List subscription plans
    #[napi]
    pub async fn list_plans(
        &self,
        filter: Option<SubscriptionPlanFilterInput>,
    ) -> Result<Vec<SubscriptionPlanOutput>> {
        let commerce = self.commerce.lock().await;
        let f = filter.unwrap_or_default();

        let plans = commerce
            .subscriptions()
            .list_plans(stateset_core::SubscriptionPlanFilter {
                status: f.status.as_ref().and_then(|s| parse_plan_status(s).ok()),
                billing_interval: f
                    .billing_interval
                    .as_ref()
                    .and_then(|s| parse_billing_interval(s).ok()),
                search: f.search,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| Error::from_reason(format!("Failed to list plans: {}", e)))?;

        convert_outputs(plans)
    }

    /// Update a subscription plan
    #[napi]
    pub async fn update_plan(
        &self,
        id: String,
        input: UpdateSubscriptionPlanInput,
    ) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let plan = commerce
            .subscriptions()
            .update_plan(
                uuid,
                stateset_core::UpdateSubscriptionPlan {
                    name: input.name,
                    description: input.description,
                    status: None,
                    price: optional_decimal_from_f64(input.price, "subscription plan price")?,
                    setup_fee: optional_decimal_from_f64(
                        input.setup_fee,
                        "subscription plan setup fee",
                    )?,
                    trial_days: input.trial_days,
                    trial_requires_payment_method: input.trial_requires_payment_method,
                    min_cycles: input.min_cycles,
                    max_cycles: input.max_cycles,
                    discount_percent: optional_decimal_from_f64(
                        input.discount_percent,
                        "subscription plan discount percent",
                    )?,
                    discount_amount: optional_decimal_from_f64(
                        input.discount_amount,
                        "subscription plan discount amount",
                    )?,
                    metadata: None,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update plan: {}", e)))?;

        convert_output(plan)
    }

    /// Activate a subscription plan
    #[napi]
    pub async fn activate_plan(&self, id: String) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let plan = commerce
            .subscriptions()
            .activate_plan(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to activate plan: {}", e)))?;

        convert_output(plan)
    }

    /// Archive a subscription plan
    #[napi]
    pub async fn archive_plan(&self, id: String) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let plan = commerce
            .subscriptions()
            .archive_plan(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to archive plan: {}", e)))?;

        convert_output(plan)
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    /// Create a subscription for a customer
    #[napi]
    pub async fn subscribe(&self, input: CreateSubscriptionInput) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let customer_id = uuid::Uuid::parse_str(&input.customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid customer UUID: {}", e)))?;
        let plan_id = uuid::Uuid::parse_str(&input.plan_id)
            .map_err(|e| Error::from_reason(format!("Invalid plan UUID: {}", e)))?;

        let start_date = input.start_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let subscription = commerce
            .subscriptions()
            .subscribe(stateset_core::CreateSubscription {
                customer_id: customer_id.into(),
                plan_id,
                payment_method_id: input.payment_method_id,
                skip_trial: input.skip_trial,
                price: optional_decimal_from_f64(input.price, "subscription price")?,
                coupon_code: input.coupon_code,
                start_date,
                items: None,
                shipping_address: None,
                billing_address: None,
                metadata: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create subscription: {}", e)))?;

        convert_output(subscription)
    }

    /// Get a subscription by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SubscriptionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let subscription = commerce
            .subscriptions()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get subscription: {}", e)))?;

        convert_optional_output(subscription)
    }

    /// Get a subscription by number
    #[napi]
    pub async fn get_by_number(&self, number: String) -> Result<Option<SubscriptionOutput>> {
        let commerce = self.commerce.lock().await;

        let subscription = commerce
            .subscriptions()
            .get_by_number(&number)
            .map_err(|e| Error::from_reason(format!("Failed to get subscription: {}", e)))?;

        convert_optional_output(subscription)
    }

    /// List subscriptions
    #[napi]
    pub async fn list(
        &self,
        filter: Option<SubscriptionFilterInput>,
    ) -> Result<Vec<SubscriptionOutput>> {
        let commerce = self.commerce.lock().await;
        let f = filter.unwrap_or_default();

        let customer_id = f
            .customer_id
            .as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(CustomerId::from);
        let plan_id = f.plan_id.as_ref().and_then(|s| uuid::Uuid::parse_str(s).ok());
        let from_date = f.from_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });
        let to_date = f.to_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let subscriptions = commerce
            .subscriptions()
            .list(stateset_core::SubscriptionFilter {
                customer_id,
                plan_id,
                status: f.status.as_ref().and_then(|s| parse_subscription_status(s).ok()),
                from_date,
                to_date,
                search: f.search,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| Error::from_reason(format!("Failed to list subscriptions: {}", e)))?;

        convert_outputs(subscriptions)
    }

    /// Update a subscription
    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateSubscriptionInput,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let next_billing_date = input.next_billing_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let subscription = commerce
            .subscriptions()
            .update(
                uuid.into(),
                stateset_core::UpdateSubscription {
                    status: input.status.as_ref().and_then(|s| parse_subscription_status(s).ok()),
                    price: optional_decimal_from_f64(input.price, "subscription price")?,
                    payment_method_id: input.payment_method_id,
                    next_billing_date,
                    discount_percent: optional_decimal_from_f64(
                        input.discount_percent,
                        "subscription discount percent",
                    )?,
                    discount_amount: optional_decimal_from_f64(
                        input.discount_amount,
                        "subscription discount amount",
                    )?,
                    coupon_code: input.coupon_code,
                    shipping_address: None,
                    billing_address: None,
                    metadata: None,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update subscription: {}", e)))?;

        convert_output(subscription)
    }

    /// Pause a subscription
    #[napi]
    pub async fn pause(
        &self,
        id: String,
        input: Option<PauseSubscriptionInput>,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let i = input.unwrap_or_default();
        let resume_at = i.resume_at.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let subscription = commerce
            .subscriptions()
            .pause(uuid.into(), stateset_core::PauseSubscription { reason: i.reason, resume_at })
            .map_err(|e| Error::from_reason(format!("Failed to pause subscription: {}", e)))?;

        convert_output(subscription)
    }

    /// Resume a paused subscription
    #[napi]
    pub async fn resume(&self, id: String) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let subscription = commerce
            .subscriptions()
            .resume(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to resume subscription: {}", e)))?;

        convert_output(subscription)
    }

    /// Cancel a subscription
    #[napi]
    pub async fn cancel(
        &self,
        id: String,
        input: Option<CancelSubscriptionInput>,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let i = input.unwrap_or_default();

        let subscription = commerce
            .subscriptions()
            .cancel(
                uuid.into(),
                stateset_core::CancelSubscription {
                    reason: i.reason,
                    immediate: i.immediate,
                    feedback: i.feedback,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to cancel subscription: {}", e)))?;

        convert_output(subscription)
    }

    /// Skip the next billing cycle
    #[napi]
    pub async fn skip_billing(
        &self,
        id: String,
        input: Option<SkipBillingCycleInput>,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let i = input.unwrap_or_default();

        let subscription = commerce
            .subscriptions()
            .skip_next_cycle(uuid.into(), stateset_core::SkipBillingCycle { reason: i.reason })
            .map_err(|e| Error::from_reason(format!("Failed to skip billing: {}", e)))?;

        convert_output(subscription)
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// List billing cycles for a subscription
    #[napi]
    pub async fn list_billing_cycles(
        &self,
        filter: Option<BillingCycleFilterInput>,
    ) -> Result<Vec<BillingCycleOutput>> {
        let commerce = self.commerce.lock().await;
        let f = filter.unwrap_or_default();

        let subscription_id = f
            .subscription_id
            .as_ref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(SubscriptionId::from);
        let from_date = f.from_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });
        let to_date = f.to_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let cycles = commerce
            .subscriptions()
            .list_billing_cycles(stateset_core::BillingCycleFilter {
                subscription_id,
                status: f.status.as_ref().and_then(|s| parse_billing_cycle_status(s).ok()),
                from_date,
                to_date,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| Error::from_reason(format!("Failed to list billing cycles: {}", e)))?;

        Ok(cycles.into_iter().map(|c| c.into()).collect())
    }

    /// Get a billing cycle by ID
    #[napi]
    pub async fn get_billing_cycle(&self, id: String) -> Result<Option<BillingCycleOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let cycle = commerce
            .subscriptions()
            .get_billing_cycle(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get billing cycle: {}", e)))?;

        Ok(cycle.map(|c| c.into()))
    }

    // ========================================================================
    // Events
    // ========================================================================

    /// Get events for a subscription
    #[napi]
    pub async fn get_events(
        &self,
        subscription_id: String,
    ) -> Result<Vec<SubscriptionEventOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&subscription_id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let events = commerce
            .subscriptions()
            .get_events(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get events: {}", e)))?;

        Ok(events.into_iter().map(|e| e.into()).collect())
    }
}

// ============================================================================
// Promotions
// ============================================================================

/// Promotions API for managing discounts and coupon codes
#[napi]
pub struct Promotions {
    commerce: Arc<Mutex<RustCommerce>>,
}

/// Input for creating a promotion
#[napi(object)]
#[derive(Default)]
pub struct CreatePromotionInput {
    /// Optional promotion code (auto-generated if not provided)
    pub code: Option<String>,
    /// Display name
    pub name: String,
    /// Description for customers
    pub description: Option<String>,
    /// Internal notes
    pub internal_notes: Option<String>,

    /// Type: percentage_off, fixed_amount_off, buy_x_get_y, free_shipping, tiered_discount, bundle
    pub promotion_type: Option<String>,
    /// Trigger: automatic, coupon_code, both
    pub trigger: Option<String>,
    /// Target: order, product, category, shipping, line_item
    pub target: Option<String>,
    /// Stacking: stackable, exclusive, selective_stack
    pub stacking: Option<String>,

    /// Percentage off (0.0-1.0, e.g., 0.20 for 20%)
    pub percentage_off: Option<f64>,
    /// Fixed amount off
    pub fixed_amount_off: Option<f64>,
    /// Maximum discount amount (cap)
    pub max_discount_amount: Option<f64>,

    /// Buy X quantity (for BOGO)
    pub buy_quantity: Option<i32>,
    /// Get Y quantity (for BOGO)
    pub get_quantity: Option<i32>,
    /// Discount on "get" items (1.0 = free, 0.5 = 50% off)
    pub get_discount_percent: Option<f64>,

    /// Tiered discount rules as JSON
    pub tiers: Option<String>,

    /// Bundle product IDs as JSON array
    pub bundle_product_ids: Option<Vec<String>>,
    /// Bundle discount
    pub bundle_discount: Option<f64>,

    /// Start date (RFC3339)
    pub starts_at: Option<String>,
    /// End date (RFC3339)
    pub ends_at: Option<String>,

    /// Total usage limit
    pub total_usage_limit: Option<i32>,
    /// Per customer usage limit
    pub per_customer_limit: Option<i32>,

    /// Applicable product IDs
    pub applicable_product_ids: Option<Vec<String>>,
    /// Applicable category IDs
    pub applicable_category_ids: Option<Vec<String>>,
    /// Applicable SKUs
    pub applicable_skus: Option<Vec<String>>,
    /// Excluded product IDs
    pub excluded_product_ids: Option<Vec<String>>,
    /// Excluded category IDs
    pub excluded_category_ids: Option<Vec<String>>,

    /// Eligible customer IDs
    pub eligible_customer_ids: Option<Vec<String>>,
    /// Eligible customer groups
    pub eligible_customer_groups: Option<Vec<String>>,

    /// Currency code
    pub currency: Option<String>,
    /// Priority (lower = applied first)
    pub priority: Option<i32>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

/// Input for updating a promotion
#[napi(object)]
#[derive(Default)]
pub struct UpdatePromotionInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub internal_notes: Option<String>,
    pub status: Option<String>,
    pub percentage_off: Option<f64>,
    pub fixed_amount_off: Option<f64>,
    pub max_discount_amount: Option<f64>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub total_usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub priority: Option<i32>,
}

/// Filter for listing promotions
#[napi(object)]
#[derive(Default)]
pub struct PromotionFilterInput {
    /// Filter by status
    pub status: Option<String>,
    /// Filter by promotion type
    pub promotion_type: Option<String>,
    /// Filter by trigger
    pub trigger: Option<String>,
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Search term
    pub search: Option<String>,
    /// Max results
    pub limit: Option<i32>,
    /// Offset for pagination
    pub offset: Option<i32>,
}

/// Promotion output
#[napi(object)]
pub struct PromotionOutput {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub internal_notes: Option<String>,
    pub promotion_type: String,
    pub trigger: String,
    pub target: String,
    pub stacking: String,
    pub status: String,
    pub percentage_off: Option<f64>,
    pub fixed_amount_off: Option<f64>,
    pub max_discount_amount: Option<f64>,
    pub buy_quantity: Option<i32>,
    pub get_quantity: Option<i32>,
    pub get_discount_percent: Option<f64>,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub total_usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub usage_count: i32,
    pub currency: String,
    pub priority: i32,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Promotion> for PromotionOutput {
    type Error = Error;

    fn try_from(p: stateset_core::Promotion) -> Result<Self> {
        Ok(Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            internal_notes: p.internal_notes,
            promotion_type: format!("{:?}", p.promotion_type).to_lowercase(),
            trigger: format!("{:?}", p.trigger).to_lowercase(),
            target: format!("{:?}", p.target).to_lowercase(),
            stacking: format!("{:?}", p.stacking).to_lowercase(),
            status: format!("{:?}", p.status).to_lowercase(),
            percentage_off: optional_to_f64_result(p.percentage_off, "promotion percentage off")?,
            fixed_amount_off: optional_to_f64_result(
                p.fixed_amount_off,
                "promotion fixed amount off",
            )?,
            max_discount_amount: optional_to_f64_result(
                p.max_discount_amount,
                "promotion max discount amount",
            )?,
            buy_quantity: p.buy_quantity,
            get_quantity: p.get_quantity,
            get_discount_percent: optional_to_f64_result(
                p.get_discount_percent,
                "promotion get discount percent",
            )?,
            starts_at: p.starts_at.to_rfc3339(),
            ends_at: p.ends_at.map(|d| d.to_rfc3339()),
            total_usage_limit: p.total_usage_limit,
            per_customer_limit: p.per_customer_limit,
            usage_count: p.usage_count,
            currency: p.currency.to_string(),
            priority: p.priority,
            metadata: p.metadata.map(|m| m.to_string()),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

/// Input for creating a coupon code
#[napi(object)]
pub struct CreateCouponInput {
    /// Promotion ID this coupon is for
    pub promotion_id: String,
    /// The coupon code customers enter
    pub code: String,
    /// Usage limit for this coupon
    pub usage_limit: Option<i32>,
    /// Per customer limit
    pub per_customer_limit: Option<i32>,
    /// Start date (RFC3339)
    pub starts_at: Option<String>,
    /// End date (RFC3339)
    pub ends_at: Option<String>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

/// Filter for listing coupons
#[napi(object)]
#[derive(Default)]
pub struct CouponFilterInput {
    pub promotion_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Coupon code output
#[napi(object)]
pub struct CouponOutput {
    pub id: String,
    pub promotion_id: String,
    pub code: String,
    pub status: String,
    pub usage_limit: Option<i32>,
    pub per_customer_limit: Option<i32>,
    pub usage_count: i32,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CouponCode> for CouponOutput {
    fn from(c: stateset_core::CouponCode) -> Self {
        Self {
            id: c.id.to_string(),
            promotion_id: c.promotion_id.to_string(),
            code: c.code,
            status: format!("{:?}", c.status).to_lowercase(),
            usage_limit: c.usage_limit,
            per_customer_limit: c.per_customer_limit,
            usage_count: c.usage_count,
            starts_at: c.starts_at.map(|d| d.to_rfc3339()),
            ends_at: c.ends_at.map(|d| d.to_rfc3339()),
            metadata: c.metadata.map(|m| m.to_string()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

/// Input for applying promotions
#[napi(object)]
pub struct ApplyPromotionsInput {
    pub cart_id: Option<String>,
    pub customer_id: Option<String>,
    pub coupon_codes: Option<Vec<String>>,
    pub line_items: Vec<PromotionLineItemInput>,
    pub subtotal: f64,
    pub shipping_amount: Option<f64>,
    pub shipping_country: Option<String>,
    pub shipping_state: Option<String>,
    pub currency: Option<String>,
}

/// Line item input for promotion calculation
#[napi(object)]
pub struct PromotionLineItemInput {
    pub id: String,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
    pub sku: Option<String>,
    pub category_ids: Option<Vec<String>>,
    pub quantity: i32,
    pub unit_price: f64,
    pub line_total: f64,
}

/// Result of applying promotions
#[napi(object)]
pub struct ApplyPromotionsOutput {
    pub original_subtotal: f64,
    pub total_discount: f64,
    pub discounted_subtotal: f64,
    pub original_shipping: f64,
    pub shipping_discount: f64,
    pub final_shipping: f64,
    pub grand_total: f64,
    pub applied_promotions: Vec<AppliedPromotionOutput>,
}

/// An applied promotion
#[napi(object)]
pub struct AppliedPromotionOutput {
    pub promotion_id: String,
    pub promotion_name: String,
    pub coupon_code: Option<String>,
    pub discount_amount: f64,
    pub discount_type: String,
}

impl TryFrom<stateset_core::AppliedPromotion> for AppliedPromotionOutput {
    type Error = Error;

    fn try_from(a: stateset_core::AppliedPromotion) -> Result<Self> {
        Ok(Self {
            promotion_id: a.promotion_id.to_string(),
            promotion_name: a.promotion_name,
            coupon_code: a.coupon_code,
            discount_amount: to_f64_result(a.discount_amount, "applied promotion discount amount")?,
            discount_type: format!("{:?}", a.discount_type).to_lowercase(),
        })
    }
}

impl TryFrom<stateset_core::ApplyPromotionsResult> for ApplyPromotionsOutput {
    type Error = Error;

    fn try_from(r: stateset_core::ApplyPromotionsResult) -> Result<Self> {
        Ok(Self {
            original_subtotal: to_f64_result(r.original_subtotal, "promotion original subtotal")?,
            total_discount: to_f64_result(r.total_discount, "promotion total discount")?,
            discounted_subtotal: to_f64_result(
                r.discounted_subtotal,
                "promotion discounted subtotal",
            )?,
            original_shipping: to_f64_result(r.original_shipping, "promotion original shipping")?,
            shipping_discount: to_f64_result(r.shipping_discount, "promotion shipping discount")?,
            final_shipping: to_f64_result(r.final_shipping, "promotion final shipping")?,
            grand_total: to_f64_result(r.grand_total, "promotion grand total")?,
            applied_promotions: convert_outputs(r.applied_promotions)?,
        })
    }
}

/// Promotion usage record output
#[napi(object)]
pub struct PromotionUsageOutput {
    pub id: String,
    pub promotion_id: String,
    pub coupon_id: Option<String>,
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub cart_id: Option<String>,
    pub discount_amount: f64,
    pub currency: String,
    pub used_at: String,
}

impl TryFrom<stateset_core::PromotionUsage> for PromotionUsageOutput {
    type Error = Error;

    fn try_from(u: stateset_core::PromotionUsage) -> Result<Self> {
        Ok(Self {
            id: u.id.to_string(),
            promotion_id: u.promotion_id.to_string(),
            coupon_id: u.coupon_id.map(|id| id.to_string()),
            customer_id: u.customer_id.map(|id| id.to_string()),
            order_id: u.order_id.map(|id| id.to_string()),
            cart_id: u.cart_id.map(|id| id.to_string()),
            discount_amount: to_f64_result(u.discount_amount, "promotion usage discount amount")?,
            currency: u.currency.to_string(),
            used_at: u.used_at.to_rfc3339(),
        })
    }
}

fn parse_promotion_type(s: &str) -> stateset_core::PromotionType {
    match s.to_lowercase().as_str() {
        "percentage_off" | "percentageoff" => stateset_core::PromotionType::PercentageOff,
        "fixed_amount_off" | "fixedamountoff" => stateset_core::PromotionType::FixedAmountOff,
        "buy_x_get_y" | "buyxgety" | "bogo" => stateset_core::PromotionType::BuyXGetY,
        "free_shipping" | "freeshipping" => stateset_core::PromotionType::FreeShipping,
        "tiered_discount" | "tiereddiscount" => stateset_core::PromotionType::TieredDiscount,
        "bundle" | "bundle_discount" | "bundlediscount" => {
            stateset_core::PromotionType::BundleDiscount
        }
        _ => stateset_core::PromotionType::PercentageOff,
    }
}

fn parse_promotion_trigger(s: &str) -> stateset_core::PromotionTrigger {
    match s.to_lowercase().as_str() {
        "automatic" | "auto" => stateset_core::PromotionTrigger::Automatic,
        "coupon_code" | "couponcode" | "coupon" => stateset_core::PromotionTrigger::CouponCode,
        "both" => stateset_core::PromotionTrigger::Both,
        _ => stateset_core::PromotionTrigger::Automatic,
    }
}

fn parse_promotion_target(s: &str) -> stateset_core::PromotionTarget {
    match s.to_lowercase().as_str() {
        "order" => stateset_core::PromotionTarget::Order,
        "product" => stateset_core::PromotionTarget::Product,
        "category" => stateset_core::PromotionTarget::Category,
        "shipping" => stateset_core::PromotionTarget::Shipping,
        "line_item" | "lineitem" => stateset_core::PromotionTarget::LineItem,
        _ => stateset_core::PromotionTarget::Order,
    }
}

fn parse_stacking_behavior(s: &str) -> stateset_core::StackingBehavior {
    match s.to_lowercase().as_str() {
        "stackable" => stateset_core::StackingBehavior::Stackable,
        "exclusive" => stateset_core::StackingBehavior::Exclusive,
        "selective_stack" | "selectivestack" => stateset_core::StackingBehavior::SelectiveStack,
        _ => stateset_core::StackingBehavior::Stackable,
    }
}

fn parse_promotion_status(s: &str) -> stateset_core::PromotionStatus {
    match s.to_lowercase().as_str() {
        "draft" => stateset_core::PromotionStatus::Draft,
        "scheduled" => stateset_core::PromotionStatus::Scheduled,
        "active" => stateset_core::PromotionStatus::Active,
        "paused" => stateset_core::PromotionStatus::Paused,
        "expired" => stateset_core::PromotionStatus::Expired,
        "exhausted" => stateset_core::PromotionStatus::Exhausted,
        "archived" => stateset_core::PromotionStatus::Archived,
        _ => stateset_core::PromotionStatus::Draft,
    }
}

fn parse_coupon_status(s: &str) -> stateset_core::CouponStatus {
    match s.to_lowercase().as_str() {
        "active" => stateset_core::CouponStatus::Active,
        "disabled" => stateset_core::CouponStatus::Disabled,
        "exhausted" => stateset_core::CouponStatus::Exhausted,
        "expired" => stateset_core::CouponStatus::Expired,
        _ => stateset_core::CouponStatus::Active,
    }
}

#[napi]
impl Promotions {
    /// Create a new promotion
    #[napi]
    pub async fn create(&self, input: CreatePromotionInput) -> Result<PromotionOutput> {
        let commerce = self.commerce.lock().await;
        let create = stateset_core::CreatePromotion {
            code: input.code,
            name: input.name,
            description: input.description,
            internal_notes: input.internal_notes,
            promotion_type: input
                .promotion_type
                .map(|s| parse_promotion_type(&s))
                .unwrap_or_default(),
            trigger: input.trigger.map(|s| parse_promotion_trigger(&s)).unwrap_or_default(),
            target: input.target.map(|s| parse_promotion_target(&s)).unwrap_or_default(),
            stacking: input.stacking.map(|s| parse_stacking_behavior(&s)).unwrap_or_default(),
            percentage_off: optional_decimal_from_f64(
                input.percentage_off,
                "promotion percentage off",
            )?,
            fixed_amount_off: optional_decimal_from_f64(
                input.fixed_amount_off,
                "promotion fixed amount off",
            )?,
            max_discount_amount: optional_decimal_from_f64(
                input.max_discount_amount,
                "promotion max discount amount",
            )?,
            buy_quantity: input.buy_quantity,
            get_quantity: input.get_quantity,
            get_discount_percent: optional_decimal_from_f64(
                input.get_discount_percent,
                "promotion get discount percent",
            )?,
            tiers: input.tiers.and_then(|s| serde_json::from_str(&s).ok()),
            bundle_product_ids: input.bundle_product_ids.map(|ids| {
                ids.into_iter()
                    .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                    .map(ProductId::from)
                    .collect()
            }),
            bundle_discount: optional_decimal_from_f64(
                input.bundle_discount,
                "promotion bundle discount",
            )?,
            starts_at: input.starts_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            ends_at: input.ends_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            conditions: None,
            applicable_product_ids: input.applicable_product_ids.map(|ids| {
                ids.into_iter()
                    .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                    .map(ProductId::from)
                    .collect()
            }),
            applicable_category_ids: input.applicable_category_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            applicable_skus: input.applicable_skus,
            excluded_product_ids: input.excluded_product_ids.map(|ids| {
                ids.into_iter()
                    .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                    .map(ProductId::from)
                    .collect()
            }),
            excluded_category_ids: input.excluded_category_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            eligible_customer_ids: input.eligible_customer_ids.map(|ids| {
                ids.into_iter()
                    .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                    .map(CustomerId::from)
                    .collect()
            }),
            eligible_customer_groups: input.eligible_customer_groups,
            currency: input.currency.and_then(|s| s.parse::<CurrencyCode>().ok()),
            priority: input.priority,
            metadata: input.metadata.and_then(|s| serde_json::from_str(&s).ok()),
        };

        let promo = commerce
            .promotions()
            .create(create)
            .map_err(|e| Error::from_reason(format!("Failed to create promotion: {}", e)))?;

        convert_output(promo)
    }

    /// Get a promotion by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let promo = commerce
            .promotions()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get promotion: {}", e)))?;

        convert_optional_output(promo)
    }

    /// Get a promotion by its internal code
    #[napi]
    pub async fn get_by_code(&self, code: String) -> Result<Option<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let promo = commerce
            .promotions()
            .get_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get promotion: {}", e)))?;

        convert_optional_output(promo)
    }

    /// List promotions with optional filtering
    #[napi]
    pub async fn list(&self, filter: Option<PromotionFilterInput>) -> Result<Vec<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();

        let core_filter = stateset_core::PromotionFilter {
            status: filter.status.map(|s| parse_promotion_status(&s)),
            promotion_type: filter.promotion_type.map(|s| parse_promotion_type(&s)),
            trigger: filter.trigger.map(|s| parse_promotion_trigger(&s)),
            is_active: filter.is_active,
            search: filter.search,
            limit: filter.limit.map(|v| v as u32),
            offset: filter.offset.map(|v| v as u32),
        };

        let promos = commerce
            .promotions()
            .list(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list promotions: {}", e)))?;

        convert_outputs(promos)
    }

    /// Update a promotion
    #[napi]
    pub async fn update(&self, id: String, input: UpdatePromotionInput) -> Result<PromotionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let update = stateset_core::UpdatePromotion {
            name: input.name,
            description: input.description,
            internal_notes: input.internal_notes,
            status: input.status.map(|s| parse_promotion_status(&s)),
            percentage_off: optional_decimal_from_f64(
                input.percentage_off,
                "promotion percentage off",
            )?,
            fixed_amount_off: optional_decimal_from_f64(
                input.fixed_amount_off,
                "promotion fixed amount off",
            )?,
            max_discount_amount: optional_decimal_from_f64(
                input.max_discount_amount,
                "promotion max discount amount",
            )?,
            starts_at: input.starts_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            ends_at: input.ends_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            priority: input.priority,
            metadata: None,
        };

        let promo = commerce
            .promotions()
            .update(uuid.into(), update)
            .map_err(|e| Error::from_reason(format!("Failed to update promotion: {}", e)))?;

        convert_output(promo)
    }

    /// Delete a promotion
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        commerce
            .promotions()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete promotion: {}", e)))?;

        Ok(())
    }

    /// Activate a promotion
    #[napi]
    pub async fn activate(&self, id: String) -> Result<PromotionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let promo = commerce
            .promotions()
            .activate(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to activate promotion: {}", e)))?;

        convert_output(promo)
    }

    /// Deactivate (pause) a promotion
    #[napi]
    pub async fn deactivate(&self, id: String) -> Result<PromotionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let promo = commerce
            .promotions()
            .deactivate(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to deactivate promotion: {}", e)))?;

        convert_output(promo)
    }

    /// Get all currently active promotions
    #[napi]
    pub async fn get_active(&self) -> Result<Vec<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let promos = commerce
            .promotions()
            .get_active()
            .map_err(|e| Error::from_reason(format!("Failed to get active promotions: {}", e)))?;

        convert_outputs(promos)
    }

    /// Check if a promotion is currently valid
    #[napi]
    pub async fn is_valid(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let valid = commerce.promotions().is_valid(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to check promotion validity: {}", e))
        })?;

        Ok(valid)
    }

    // ========================================================================
    // Coupon Codes
    // ========================================================================

    /// Create a coupon code for a promotion
    #[napi]
    pub async fn create_coupon(&self, input: CreateCouponInput) -> Result<CouponOutput> {
        let commerce = self.commerce.lock().await;
        let promotion_id = uuid::Uuid::parse_str(&input.promotion_id)
            .map_err(|e| Error::from_reason(format!("Invalid promotion UUID: {}", e)))?;

        let create = stateset_core::CreateCouponCode {
            promotion_id: promotion_id.into(),
            code: input.code,
            usage_limit: input.usage_limit,
            per_customer_limit: input.per_customer_limit,
            starts_at: input.starts_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            ends_at: input.ends_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            metadata: input.metadata.and_then(|s| serde_json::from_str(&s).ok()),
        };

        let coupon = commerce
            .promotions()
            .create_coupon(create)
            .map_err(|e| Error::from_reason(format!("Failed to create coupon: {}", e)))?;

        Ok(coupon.into())
    }

    /// Get a coupon by ID
    #[napi]
    pub async fn get_coupon(&self, id: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let coupon = commerce
            .promotions()
            .get_coupon(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// Get a coupon by its code
    #[napi]
    pub async fn get_coupon_by_code(&self, code: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let coupon = commerce
            .promotions()
            .get_coupon_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// List coupons with optional filtering
    #[napi]
    pub async fn list_coupons(
        &self,
        filter: Option<CouponFilterInput>,
    ) -> Result<Vec<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();

        let core_filter = stateset_core::CouponFilter {
            promotion_id: filter
                .promotion_id
                .and_then(|s| uuid::Uuid::parse_str(&s).ok())
                .map(PromotionId::from),
            status: filter.status.map(|s| parse_coupon_status(&s)),
            search: filter.search,
            limit: filter.limit.map(|v| v as u32),
            offset: filter.offset.map(|v| v as u32),
        };

        let coupons = commerce
            .promotions()
            .list_coupons(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list coupons: {}", e)))?;

        Ok(coupons.into_iter().map(|c| c.into()).collect())
    }

    /// Validate a coupon code
    #[napi]
    pub async fn validate_coupon(&self, code: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let coupon = commerce
            .promotions()
            .validate_coupon(&code)
            .map_err(|e| Error::from_reason(format!("Failed to validate coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    /// Apply promotions to cart/order items
    #[napi]
    pub async fn apply(&self, input: ApplyPromotionsInput) -> Result<ApplyPromotionsOutput> {
        let commerce = self.commerce.lock().await;
        let request = stateset_core::ApplyPromotionsRequest {
            cart_id: input.cart_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()).map(CartId::from),
            customer_id: input
                .customer_id
                .and_then(|s| uuid::Uuid::parse_str(&s).ok())
                .map(CustomerId::from),
            coupon_codes: input.coupon_codes.unwrap_or_default(),
            line_items: input
                .line_items
                .into_iter()
                .map(|item| {
                    Ok(stateset_core::PromotionLineItem {
                        id: item.id,
                        product_id: item
                            .product_id
                            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
                            .map(ProductId::from),
                        variant_id: item.variant_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
                        sku: item.sku,
                        category_ids: item
                            .category_ids
                            .map(|ids| {
                                ids.into_iter()
                                    .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                                    .collect()
                            })
                            .unwrap_or_default(),
                        quantity: item.quantity,
                        unit_price: decimal_from_f64(
                            item.unit_price,
                            "promotion line item unit price",
                        )?,
                        line_total: decimal_from_f64(
                            item.line_total,
                            "promotion line item line total",
                        )?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            subtotal: decimal_from_f64(input.subtotal, "promotion subtotal")?,
            shipping_amount: input
                .shipping_amount
                .map(|amount| decimal_from_f64(amount, "promotion shipping amount"))
                .transpose()?
                .unwrap_or_default(),
            shipping_country: input.shipping_country,
            shipping_state: input.shipping_state,
            currency: input
                .currency
                .unwrap_or_else(|| "USD".to_string())
                .parse::<CurrencyCode>()
                .unwrap_or(CurrencyCode::USD),
            is_first_order: false,
        };

        let result = commerce
            .promotions()
            .apply(request)
            .map_err(|e| Error::from_reason(format!("Failed to apply promotions: {}", e)))?;

        convert_output(result)
    }

    /// Record promotion usage (after order completion)
    #[napi]
    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        promotion_id: String,
        coupon_id: Option<String>,
        customer_id: Option<String>,
        order_id: Option<String>,
        cart_id: Option<String>,
        discount_amount: f64,
        currency: String,
    ) -> Result<PromotionUsageOutput> {
        let commerce = self.commerce.lock().await;
        let promotion_uuid = uuid::Uuid::parse_str(&promotion_id)
            .map_err(|e| Error::from_reason(format!("Invalid promotion UUID: {}", e)))?;

        let usage = commerce
            .promotions()
            .record_usage(
                promotion_uuid.into(),
                coupon_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
                customer_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()).map(CustomerId::from),
                order_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()).map(OrderId::from),
                cart_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()).map(CartId::from),
                decimal_from_f64(discount_amount, "promotion discount amount")?,
                &currency,
            )
            .map_err(|e| Error::from_reason(format!("Failed to record usage: {}", e)))?;

        convert_output(usage)
    }
}

// ============================================================================
// Tax API
// ============================================================================

// --- Input Types ---

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxAddressInput {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxLineItemInput {
    pub id: String,
    pub sku: Option<String>,
    pub product_id: Option<String>,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount_amount: Option<f64>,
    pub tax_category: Option<String>,
    pub tax_code: Option<String>,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxCalculationInput {
    pub line_items: Vec<TaxLineItemInput>,
    pub shipping_address: TaxAddressInput,
    pub billing_address: Option<TaxAddressInput>,
    pub customer_id: Option<String>,
    pub shipping_amount: Option<f64>,
    pub currency: Option<String>,
    pub transaction_date: Option<String>,
    pub prices_include_tax: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreateJurisdictionInput {
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub level: Option<String>,
    pub country_code: String,
    pub state_code: Option<String>,
    pub county: Option<String>,
    pub city: Option<String>,
    pub postal_codes: Option<Vec<String>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreateTaxRateInput {
    pub jurisdiction_id: String,
    pub tax_type: Option<String>,
    pub product_category: Option<String>,
    pub rate: f64,
    pub name: String,
    pub description: Option<String>,
    pub is_compound: Option<bool>,
    pub priority: Option<i32>,
    pub threshold_min: Option<f64>,
    pub threshold_max: Option<f64>,
    pub fixed_amount: Option<f64>,
    pub effective_from: String,
    pub effective_to: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreateExemptionInput {
    pub customer_id: String,
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub jurisdiction_ids: Option<Vec<String>>,
    pub exempt_categories: Option<Vec<String>>,
    pub effective_from: String,
    pub expires_at: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxRateFilterInput {
    pub jurisdiction_id: Option<String>,
    pub tax_type: Option<String>,
    pub product_category: Option<String>,
    pub active_only: Option<bool>,
    pub effective_date: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct JurisdictionFilterInput {
    pub country_code: Option<String>,
    pub state_code: Option<String>,
    pub level: Option<String>,
    pub active_only: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxSettingsInput {
    pub enabled: Option<bool>,
    pub calculation_method: Option<String>,
    pub compound_method: Option<String>,
    pub tax_shipping: Option<bool>,
    pub tax_handling: Option<bool>,
    pub tax_gift_wrap: Option<bool>,
    pub default_product_category: Option<String>,
    pub rounding_mode: Option<String>,
    pub decimal_places: Option<i32>,
    pub validate_addresses: Option<bool>,
    pub tax_provider: Option<String>,
}

// --- Output Types ---

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxJurisdictionOutput {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub level: String,
    pub country_code: String,
    pub state_code: Option<String>,
    pub county: Option<String>,
    pub city: Option<String>,
    pub postal_codes: Vec<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TaxJurisdiction> for TaxJurisdictionOutput {
    fn from(j: stateset_core::TaxJurisdiction) -> Self {
        Self {
            id: j.id.to_string(),
            parent_id: j.parent_id.map(|u| u.to_string()),
            name: j.name,
            code: j.code,
            level: format!("{:?}", j.level).to_lowercase(),
            country_code: j.country_code,
            state_code: j.state_code,
            county: j.county,
            city: j.city,
            postal_codes: j.postal_codes,
            active: j.active,
            created_at: j.created_at.to_rfc3339(),
            updated_at: j.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxRateOutput {
    pub id: String,
    pub jurisdiction_id: String,
    pub tax_type: String,
    pub product_category: String,
    pub rate: f64,
    pub name: String,
    pub description: Option<String>,
    pub is_compound: bool,
    pub priority: i32,
    pub threshold_min: Option<f64>,
    pub threshold_max: Option<f64>,
    pub fixed_amount: Option<f64>,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::TaxRate> for TaxRateOutput {
    type Error = Error;

    fn try_from(r: stateset_core::TaxRate) -> Result<Self> {
        Ok(Self {
            id: r.id.to_string(),
            jurisdiction_id: r.jurisdiction_id.to_string(),
            tax_type: r.tax_type.as_str().to_string(),
            product_category: r.product_category.as_str().to_string(),
            rate: to_f64_result(r.rate, "tax rate")?,
            name: r.name,
            description: r.description,
            is_compound: r.is_compound,
            priority: r.priority,
            threshold_min: optional_to_f64_result(r.threshold_min, "tax threshold min")?,
            threshold_max: optional_to_f64_result(r.threshold_max, "tax threshold max")?,
            fixed_amount: optional_to_f64_result(r.fixed_amount, "tax fixed amount")?,
            effective_from: r.effective_from.to_string(),
            effective_to: r.effective_to.map(|d| d.to_string()),
            active: r.active,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxExemptionOutput {
    pub id: String,
    pub customer_id: String,
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub jurisdiction_ids: Vec<String>,
    pub exempt_categories: Vec<String>,
    pub effective_from: String,
    pub expires_at: Option<String>,
    pub verified: bool,
    pub verified_at: Option<String>,
    pub notes: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TaxExemption> for TaxExemptionOutput {
    fn from(e: stateset_core::TaxExemption) -> Self {
        Self {
            id: e.id.to_string(),
            customer_id: e.customer_id.to_string(),
            exemption_type: format!("{:?}", e.exemption_type).to_lowercase(),
            certificate_number: e.certificate_number,
            issuing_authority: e.issuing_authority,
            jurisdiction_ids: e.jurisdiction_ids.iter().map(|u| u.to_string()).collect(),
            exempt_categories: e.exempt_categories.iter().map(|c| c.as_str().to_string()).collect(),
            effective_from: e.effective_from.to_string(),
            expires_at: e.expires_at.map(|d| d.to_string()),
            verified: e.verified,
            verified_at: e.verified_at.map(|d| d.to_rfc3339()),
            notes: e.notes,
            active: e.active,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxBreakdownOutput {
    pub jurisdiction_id: String,
    pub jurisdiction_name: String,
    pub tax_type: String,
    pub rate_name: String,
    pub rate: f64,
    pub taxable_amount: f64,
    pub tax_amount: f64,
    pub is_compound: bool,
}

impl TryFrom<stateset_core::TaxBreakdown> for TaxBreakdownOutput {
    type Error = Error;

    fn try_from(b: stateset_core::TaxBreakdown) -> Result<Self> {
        Ok(Self {
            jurisdiction_id: b.jurisdiction_id.to_string(),
            jurisdiction_name: b.jurisdiction_name,
            tax_type: b.tax_type.as_str().to_string(),
            rate_name: b.rate_name,
            rate: to_f64_result(b.rate, "tax breakdown rate")?,
            taxable_amount: to_f64_result(b.taxable_amount, "tax breakdown taxable amount")?,
            tax_amount: to_f64_result(b.tax_amount, "tax breakdown tax amount")?,
            is_compound: b.is_compound,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxDetailOutput {
    pub tax_type: String,
    pub jurisdiction_name: String,
    pub rate: f64,
    pub amount: f64,
}

impl TryFrom<stateset_core::TaxDetail> for TaxDetailOutput {
    type Error = Error;

    fn try_from(d: stateset_core::TaxDetail) -> Result<Self> {
        Ok(Self {
            tax_type: d.tax_type.as_str().to_string(),
            jurisdiction_name: d.jurisdiction_name,
            rate: to_f64_result(d.rate, "tax detail rate")?,
            amount: to_f64_result(d.amount, "tax detail amount")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LineItemTaxOutput {
    pub line_item_id: String,
    pub taxable_amount: f64,
    pub tax_amount: f64,
    pub effective_rate: f64,
    pub is_exempt: bool,
    pub exemption_reason: Option<String>,
    pub tax_details: Vec<TaxDetailOutput>,
}

impl TryFrom<stateset_core::LineItemTax> for LineItemTaxOutput {
    type Error = Error;

    fn try_from(t: stateset_core::LineItemTax) -> Result<Self> {
        Ok(Self {
            line_item_id: t.line_item_id,
            taxable_amount: to_f64_result(t.taxable_amount, "line item tax taxable amount")?,
            tax_amount: to_f64_result(t.tax_amount, "line item tax amount")?,
            effective_rate: to_f64_result(t.effective_rate, "line item effective tax rate")?,
            is_exempt: t.is_exempt,
            exemption_reason: t.exemption_reason,
            tax_details: convert_outputs(t.tax_details)?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ExemptionDetailsOutput {
    pub exemption_id: String,
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub amount_exempt: f64,
    pub tax_saved: f64,
}

impl TryFrom<stateset_core::ExemptionDetails> for ExemptionDetailsOutput {
    type Error = Error;

    fn try_from(e: stateset_core::ExemptionDetails) -> Result<Self> {
        Ok(Self {
            exemption_id: e.exemption_id.to_string(),
            exemption_type: format!("{:?}", e.exemption_type).to_lowercase(),
            certificate_number: e.certificate_number,
            amount_exempt: to_f64_result(e.amount_exempt, "tax exemption amount exempt")?,
            tax_saved: to_f64_result(e.tax_saved, "tax exemption tax saved")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct JurisdictionSummaryOutput {
    pub id: String,
    pub name: String,
    pub code: String,
    pub level: String,
    pub total_rate: f64,
    pub total_tax: f64,
}

impl TryFrom<stateset_core::JurisdictionSummary> for JurisdictionSummaryOutput {
    type Error = Error;

    fn try_from(s: stateset_core::JurisdictionSummary) -> Result<Self> {
        Ok(Self {
            id: s.id.to_string(),
            name: s.name,
            code: s.code,
            level: format!("{:?}", s.level).to_lowercase(),
            total_rate: to_f64_result(s.total_rate, "jurisdiction total rate")?,
            total_tax: to_f64_result(s.total_tax, "jurisdiction total tax")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxCalculationOutput {
    pub id: String,
    pub total_tax: f64,
    pub subtotal: f64,
    pub total: f64,
    pub shipping_tax: f64,
    pub tax_breakdown: Vec<TaxBreakdownOutput>,
    pub line_item_taxes: Vec<LineItemTaxOutput>,
    pub exemptions_applied: bool,
    pub exemption_details: Option<ExemptionDetailsOutput>,
    pub jurisdictions: Vec<JurisdictionSummaryOutput>,
    pub calculated_at: String,
    pub is_estimate: bool,
}

impl TryFrom<stateset_core::TaxCalculationResult> for TaxCalculationOutput {
    type Error = Error;

    fn try_from(r: stateset_core::TaxCalculationResult) -> Result<Self> {
        Ok(Self {
            id: r.id.to_string(),
            total_tax: to_f64_result(r.total_tax, "tax calculation total tax")?,
            subtotal: to_f64_result(r.subtotal, "tax calculation subtotal")?,
            total: to_f64_result(r.total, "tax calculation total")?,
            shipping_tax: to_f64_result(r.shipping_tax, "tax calculation shipping tax")?,
            tax_breakdown: convert_outputs(r.tax_breakdown)?,
            line_item_taxes: convert_outputs(r.line_item_taxes)?,
            exemptions_applied: r.exemptions_applied,
            exemption_details: convert_optional_output(r.exemption_details)?,
            jurisdictions: convert_outputs(r.jurisdictions)?,
            calculated_at: r.calculated_at.to_rfc3339(),
            is_estimate: r.is_estimate,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxSettingsOutput {
    pub id: String,
    pub enabled: bool,
    pub calculation_method: String,
    pub compound_method: String,
    pub tax_shipping: bool,
    pub tax_handling: bool,
    pub tax_gift_wrap: bool,
    pub default_product_category: String,
    pub rounding_mode: String,
    pub decimal_places: i32,
    pub validate_addresses: bool,
    pub tax_provider: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TaxSettings> for TaxSettingsOutput {
    fn from(s: stateset_core::TaxSettings) -> Self {
        Self {
            id: s.id.to_string(),
            enabled: s.enabled,
            calculation_method: format!("{:?}", s.calculation_method).to_lowercase(),
            compound_method: format!("{:?}", s.compound_method).to_lowercase(),
            tax_shipping: s.tax_shipping,
            tax_handling: s.tax_handling,
            tax_gift_wrap: s.tax_gift_wrap,
            default_product_category: s.default_product_category.as_str().to_string(),
            rounding_mode: s.rounding_mode,
            decimal_places: s.decimal_places,
            validate_addresses: s.validate_addresses,
            tax_provider: s.tax_provider,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UsStateTaxInfoOutput {
    pub state_code: String,
    pub state_name: String,
    pub state_rate: f64,
    pub has_local_taxes: bool,
    pub origin_based: bool,
    pub tax_shipping: bool,
    pub tax_clothing: bool,
    pub tax_food: bool,
    pub tax_digital: bool,
}

impl TryFrom<stateset_core::UsStateTaxInfo> for UsStateTaxInfoOutput {
    type Error = Error;

    fn try_from(i: stateset_core::UsStateTaxInfo) -> Result<Self> {
        Ok(Self {
            state_code: i.state_code,
            state_name: i.state_name,
            state_rate: to_f64_result(i.state_rate, "US state tax rate")?,
            has_local_taxes: i.has_local_taxes,
            origin_based: i.origin_based,
            tax_shipping: i.tax_shipping,
            tax_clothing: i.tax_clothing,
            tax_food: i.tax_food,
            tax_digital: i.tax_digital,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EuVatInfoOutput {
    pub country_code: String,
    pub country_name: String,
    pub standard_rate: f64,
    pub reduced_rate: Option<f64>,
    pub super_reduced_rate: Option<f64>,
    pub parking_rate: Option<f64>,
}

impl TryFrom<stateset_core::EuVatInfo> for EuVatInfoOutput {
    type Error = Error;

    fn try_from(i: stateset_core::EuVatInfo) -> Result<Self> {
        Ok(Self {
            country_code: i.country_code,
            country_name: i.country_name,
            standard_rate: to_f64_result(i.standard_rate, "EU VAT standard rate")?,
            reduced_rate: optional_to_f64_result(i.reduced_rate, "EU VAT reduced rate")?,
            super_reduced_rate: optional_to_f64_result(
                i.super_reduced_rate,
                "EU VAT super reduced rate",
            )?,
            parking_rate: optional_to_f64_result(i.parking_rate, "EU VAT parking rate")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CanadianTaxInfoOutput {
    pub province_code: String,
    pub province_name: String,
    pub gst_rate: f64,
    pub pst_rate: Option<f64>,
    pub hst_rate: Option<f64>,
    pub qst_rate: Option<f64>,
    pub total_rate: f64,
}

impl TryFrom<stateset_core::CanadianTaxInfo> for CanadianTaxInfoOutput {
    type Error = Error;

    fn try_from(i: stateset_core::CanadianTaxInfo) -> Result<Self> {
        Ok(Self {
            province_code: i.province_code,
            province_name: i.province_name,
            gst_rate: to_f64_result(i.gst_rate, "Canadian tax GST rate")?,
            pst_rate: optional_to_f64_result(i.pst_rate, "Canadian tax PST rate")?,
            hst_rate: optional_to_f64_result(i.hst_rate, "Canadian tax HST rate")?,
            qst_rate: optional_to_f64_result(i.qst_rate, "Canadian tax QST rate")?,
            total_rate: to_f64_result(i.total_rate, "Canadian tax total rate")?,
        })
    }
}

// --- Helper Functions ---

fn parse_tax_type(s: &str) -> stateset_core::TaxType {
    match s.to_lowercase().as_str() {
        "sales_tax" => stateset_core::TaxType::SalesTax,
        "vat" => stateset_core::TaxType::Vat,
        "gst" => stateset_core::TaxType::Gst,
        "hst" => stateset_core::TaxType::Hst,
        "pst" => stateset_core::TaxType::Pst,
        "qst" => stateset_core::TaxType::Qst,
        "consumption_tax" => stateset_core::TaxType::ConsumptionTax,
        "custom" => stateset_core::TaxType::Custom,
        _ => stateset_core::TaxType::SalesTax,
    }
}

fn parse_product_tax_category(s: &str) -> stateset_core::ProductTaxCategory {
    match s.to_lowercase().as_str() {
        "standard" => stateset_core::ProductTaxCategory::Standard,
        "reduced" => stateset_core::ProductTaxCategory::Reduced,
        "super_reduced" => stateset_core::ProductTaxCategory::SuperReduced,
        "zero_rated" => stateset_core::ProductTaxCategory::ZeroRated,
        "exempt" => stateset_core::ProductTaxCategory::Exempt,
        "digital" => stateset_core::ProductTaxCategory::Digital,
        "clothing" => stateset_core::ProductTaxCategory::Clothing,
        "food" => stateset_core::ProductTaxCategory::Food,
        "prepared_food" => stateset_core::ProductTaxCategory::PreparedFood,
        "medical" => stateset_core::ProductTaxCategory::Medical,
        "educational" => stateset_core::ProductTaxCategory::Educational,
        "luxury" => stateset_core::ProductTaxCategory::Luxury,
        _ => stateset_core::ProductTaxCategory::Standard,
    }
}

fn parse_jurisdiction_level(s: &str) -> stateset_core::JurisdictionLevel {
    match s.to_lowercase().as_str() {
        "country" => stateset_core::JurisdictionLevel::Country,
        "state" => stateset_core::JurisdictionLevel::State,
        "county" => stateset_core::JurisdictionLevel::County,
        "city" => stateset_core::JurisdictionLevel::City,
        "district" => stateset_core::JurisdictionLevel::District,
        "special" => stateset_core::JurisdictionLevel::Special,
        _ => stateset_core::JurisdictionLevel::Country,
    }
}

fn parse_exemption_type(s: &str) -> stateset_core::ExemptionType {
    match s.to_lowercase().as_str() {
        "resale" => stateset_core::ExemptionType::Resale,
        "non_profit" | "nonprofit" => stateset_core::ExemptionType::NonProfit,
        "government" => stateset_core::ExemptionType::Government,
        "educational" => stateset_core::ExemptionType::Educational,
        "religious" => stateset_core::ExemptionType::Religious,
        "medical" => stateset_core::ExemptionType::Medical,
        "manufacturing" => stateset_core::ExemptionType::Manufacturing,
        "agricultural" => stateset_core::ExemptionType::Agricultural,
        "export" => stateset_core::ExemptionType::Export,
        "diplomatic" => stateset_core::ExemptionType::Diplomatic,
        _ => stateset_core::ExemptionType::Other,
    }
}

fn parse_calculation_method(s: &str) -> stateset_core::TaxCalculationMethod {
    match s.to_lowercase().as_str() {
        "inclusive" => stateset_core::TaxCalculationMethod::Inclusive,
        _ => stateset_core::TaxCalculationMethod::Exclusive,
    }
}

fn parse_compound_method(s: &str) -> stateset_core::TaxCompoundMethod {
    match s.to_lowercase().as_str() {
        "compound" => stateset_core::TaxCompoundMethod::Compound,
        "separate" => stateset_core::TaxCompoundMethod::Separate,
        _ => stateset_core::TaxCompoundMethod::Combined,
    }
}

// --- Tax Struct ---

#[napi]
pub struct Tax {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Tax {
    // ========================================================================
    // Tax Calculation
    // ========================================================================

    /// Calculate tax for a transaction
    #[napi]
    pub async fn calculate(&self, input: TaxCalculationInput) -> Result<TaxCalculationOutput> {
        let commerce = self.commerce.lock().await;

        let line_items: Vec<stateset_core::TaxLineItem> = input
            .line_items
            .into_iter()
            .map(|item| {
                Ok(stateset_core::TaxLineItem {
                    id: item.id,
                    sku: item.sku,
                    product_id: item
                        .product_id
                        .and_then(|s| uuid::Uuid::parse_str(&s).ok())
                        .map(ProductId::from),
                    quantity: decimal_from_f64(item.quantity, "tax line item quantity")?,
                    unit_price: decimal_from_f64(item.unit_price, "tax line item unit price")?,
                    discount_amount: optional_decimal_from_f64(
                        item.discount_amount,
                        "tax line item discount amount",
                    )?
                    .unwrap_or_default(),
                    tax_category: item
                        .tax_category
                        .map(|s| parse_product_tax_category(&s))
                        .unwrap_or_default(),
                    tax_code: item.tax_code,
                    description: item.description,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let shipping_address = stateset_core::TaxAddress {
            line1: input.shipping_address.line1,
            line2: input.shipping_address.line2,
            city: input.shipping_address.city,
            state: input.shipping_address.state,
            postal_code: input.shipping_address.postal_code,
            country: input.shipping_address.country,
        };

        let billing_address = input.billing_address.map(|addr| stateset_core::TaxAddress {
            line1: addr.line1,
            line2: addr.line2,
            city: addr.city,
            state: addr.state,
            postal_code: addr.postal_code,
            country: addr.country,
        });

        let request = stateset_core::TaxCalculationRequest {
            line_items,
            shipping_address,
            billing_address,
            customer_id: input.customer_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            shipping_amount: optional_decimal_from_f64(
                input.shipping_amount,
                "tax shipping amount",
            )?,
            currency: input
                .currency
                .unwrap_or_else(|| "USD".to_string())
                .parse::<CurrencyCode>()
                .unwrap_or(CurrencyCode::USD),
            transaction_date: input
                .transaction_date
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            prices_include_tax: input.prices_include_tax.unwrap_or(false),
        };

        let result = commerce
            .tax()
            .calculate(request)
            .map_err(|e| Error::from_reason(format!("Failed to calculate tax: {}", e)))?;

        convert_output(result)
    }

    /// Calculate tax for a single item
    #[napi]
    pub async fn calculate_for_item(
        &self,
        unit_price: f64,
        quantity: f64,
        category: Option<String>,
        shipping_address: TaxAddressInput,
    ) -> Result<f64> {
        let commerce = self.commerce.lock().await;

        let address = stateset_core::TaxAddress {
            line1: shipping_address.line1,
            line2: shipping_address.line2,
            city: shipping_address.city,
            state: shipping_address.state,
            postal_code: shipping_address.postal_code,
            country: shipping_address.country,
        };

        let tax = commerce
            .tax()
            .calculate_for_item(
                decimal_from_f64(unit_price, "tax unit price")?,
                decimal_from_f64(quantity, "tax quantity")?,
                category.map(|s| parse_product_tax_category(&s)).unwrap_or_default(),
                &address,
            )
            .map_err(|e| Error::from_reason(format!("Failed to calculate tax: {}", e)))?;

        to_f64_result(tax, "tax amount")
    }

    /// Get the effective tax rate for an address and category
    #[napi]
    pub async fn get_effective_rate(
        &self,
        address: TaxAddressInput,
        category: Option<String>,
    ) -> Result<f64> {
        let commerce = self.commerce.lock().await;

        let tax_address = stateset_core::TaxAddress {
            line1: address.line1,
            line2: address.line2,
            city: address.city,
            state: address.state,
            postal_code: address.postal_code,
            country: address.country,
        };

        let rate = commerce
            .tax()
            .get_effective_rate(
                &tax_address,
                category.map(|s| parse_product_tax_category(&s)).unwrap_or_default(),
            )
            .map_err(|e| Error::from_reason(format!("Failed to get rate: {}", e)))?;

        to_f64_result(rate, "tax rate")
    }

    // ========================================================================
    // Jurisdiction Operations
    // ========================================================================

    /// Get a jurisdiction by ID
    #[napi]
    pub async fn get_jurisdiction(&self, id: String) -> Result<Option<TaxJurisdictionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let jurisdiction = commerce
            .tax()
            .get_jurisdiction(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get jurisdiction: {}", e)))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// Get a jurisdiction by code
    #[napi]
    pub async fn get_jurisdiction_by_code(
        &self,
        code: String,
    ) -> Result<Option<TaxJurisdictionOutput>> {
        let commerce = self.commerce.lock().await;

        let jurisdiction = commerce
            .tax()
            .get_jurisdiction_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get jurisdiction: {}", e)))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// List jurisdictions with optional filtering
    #[napi]
    pub async fn list_jurisdictions(
        &self,
        filter: Option<JurisdictionFilterInput>,
    ) -> Result<Vec<TaxJurisdictionOutput>> {
        let commerce = self.commerce.lock().await;

        let f = filter.unwrap_or_default();
        let core_filter = stateset_core::TaxJurisdictionFilter {
            country_code: f.country_code,
            state_code: f.state_code,
            level: f.level.map(|s| parse_jurisdiction_level(&s)),
            active_only: f.active_only.unwrap_or(false),
        };

        let jurisdictions = commerce
            .tax()
            .list_jurisdictions(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list jurisdictions: {}", e)))?;

        Ok(jurisdictions.into_iter().map(|j| j.into()).collect())
    }

    /// Create a new jurisdiction
    #[napi]
    pub async fn create_jurisdiction(
        &self,
        input: CreateJurisdictionInput,
    ) -> Result<TaxJurisdictionOutput> {
        let commerce = self.commerce.lock().await;

        let create = stateset_core::CreateTaxJurisdiction {
            parent_id: input.parent_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            name: input.name,
            code: input.code,
            level: input.level.map(|s| parse_jurisdiction_level(&s)).unwrap_or_default(),
            country_code: input.country_code,
            state_code: input.state_code,
            county: input.county,
            city: input.city,
            postal_codes: input.postal_codes.unwrap_or_default(),
        };

        let jurisdiction = commerce
            .tax()
            .create_jurisdiction(create)
            .map_err(|e| Error::from_reason(format!("Failed to create jurisdiction: {}", e)))?;

        Ok(jurisdiction.into())
    }

    // ========================================================================
    // Tax Rate Operations
    // ========================================================================

    /// Get a tax rate by ID
    #[napi]
    pub async fn get_rate(&self, id: String) -> Result<Option<TaxRateOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let rate = commerce
            .tax()
            .get_rate(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get rate: {}", e)))?;

        convert_optional_output(rate)
    }

    /// List tax rates with optional filtering
    #[napi]
    pub async fn list_rates(
        &self,
        filter: Option<TaxRateFilterInput>,
    ) -> Result<Vec<TaxRateOutput>> {
        let commerce = self.commerce.lock().await;

        let f = filter.unwrap_or_default();
        let core_filter = stateset_core::TaxRateFilter {
            jurisdiction_id: f.jurisdiction_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            tax_type: f.tax_type.map(|s| parse_tax_type(&s)),
            product_category: f.product_category.map(|s| parse_product_tax_category(&s)),
            active_only: f.active_only.unwrap_or(false),
            effective_date: f
                .effective_date
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        };

        let rates = commerce
            .tax()
            .list_rates(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list rates: {}", e)))?;

        convert_outputs(rates)
    }

    /// Create a new tax rate
    #[napi]
    pub async fn create_rate(&self, input: CreateTaxRateInput) -> Result<TaxRateOutput> {
        let commerce = self.commerce.lock().await;

        let jurisdiction_id = uuid::Uuid::parse_str(&input.jurisdiction_id)
            .map_err(|e| Error::from_reason(format!("Invalid jurisdiction UUID: {}", e)))?;

        let effective_from =
            chrono::NaiveDate::parse_from_str(&input.effective_from, "%Y-%m-%d")
                .map_err(|e| Error::from_reason(format!("Invalid date format: {}", e)))?;

        let create = stateset_core::CreateTaxRate {
            jurisdiction_id,
            tax_type: input.tax_type.map(|s| parse_tax_type(&s)).unwrap_or_default(),
            product_category: input
                .product_category
                .map(|s| parse_product_tax_category(&s))
                .unwrap_or_default(),
            rate: decimal_from_f64(input.rate, "tax rate")?,
            name: input.name,
            description: input.description,
            is_compound: input.is_compound.unwrap_or(false),
            priority: input.priority.unwrap_or(0),
            threshold_min: optional_decimal_from_f64(input.threshold_min, "tax threshold min")?,
            threshold_max: optional_decimal_from_f64(input.threshold_max, "tax threshold max")?,
            fixed_amount: optional_decimal_from_f64(input.fixed_amount, "tax fixed amount")?,
            effective_from,
            effective_to: input
                .effective_to
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        };

        let rate = commerce
            .tax()
            .create_rate(create)
            .map_err(|e| Error::from_reason(format!("Failed to create rate: {}", e)))?;

        convert_output(rate)
    }

    // ========================================================================
    // Exemption Operations
    // ========================================================================

    /// Get an exemption by ID
    #[napi]
    pub async fn get_exemption(&self, id: String) -> Result<Option<TaxExemptionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let exemption = commerce
            .tax()
            .get_exemption(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get exemption: {}", e)))?;

        Ok(exemption.map(|e| e.into()))
    }

    /// Get exemptions for a customer
    #[napi]
    pub async fn get_customer_exemptions(
        &self,
        customer_id: String,
    ) -> Result<Vec<TaxExemptionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let exemptions = commerce
            .tax()
            .get_customer_exemptions(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get exemptions: {}", e)))?;

        Ok(exemptions.into_iter().map(|e| e.into()).collect())
    }

    /// Create a tax exemption
    #[napi]
    pub async fn create_exemption(
        &self,
        input: CreateExemptionInput,
    ) -> Result<TaxExemptionOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id = uuid::Uuid::parse_str(&input.customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid customer UUID: {}", e)))?;

        let effective_from =
            chrono::NaiveDate::parse_from_str(&input.effective_from, "%Y-%m-%d")
                .map_err(|e| Error::from_reason(format!("Invalid date format: {}", e)))?;

        let create = stateset_core::CreateTaxExemption {
            customer_id,
            exemption_type: parse_exemption_type(&input.exemption_type),
            certificate_number: input.certificate_number,
            issuing_authority: input.issuing_authority,
            jurisdiction_ids: input
                .jurisdiction_ids
                .unwrap_or_default()
                .into_iter()
                .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                .collect(),
            exempt_categories: input
                .exempt_categories
                .unwrap_or_default()
                .into_iter()
                .map(|s| parse_product_tax_category(&s))
                .collect(),
            effective_from,
            expires_at: input
                .expires_at
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            notes: input.notes,
        };

        let exemption = commerce
            .tax()
            .create_exemption(create)
            .map_err(|e| Error::from_reason(format!("Failed to create exemption: {}", e)))?;

        Ok(exemption.into())
    }

    /// Check if a customer is tax exempt
    #[napi]
    pub async fn customer_is_exempt(&self, customer_id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let is_exempt = commerce
            .tax()
            .customer_is_exempt(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to check exemption: {}", e)))?;

        Ok(is_exempt)
    }

    // ========================================================================
    // Settings Operations
    // ========================================================================

    /// Get tax settings
    #[napi]
    pub async fn get_settings(&self) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.lock().await;

        let settings = commerce
            .tax()
            .get_settings()
            .map_err(|e| Error::from_reason(format!("Failed to get settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Update tax settings
    #[napi]
    pub async fn update_settings(&self, input: TaxSettingsInput) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.lock().await;

        let mut settings = commerce
            .tax()
            .get_settings()
            .map_err(|e| Error::from_reason(format!("Failed to get settings: {}", e)))?;

        if let Some(enabled) = input.enabled {
            settings.enabled = enabled;
        }
        if let Some(method) = input.calculation_method {
            settings.calculation_method = parse_calculation_method(&method);
        }
        if let Some(method) = input.compound_method {
            settings.compound_method = parse_compound_method(&method);
        }
        if let Some(tax_shipping) = input.tax_shipping {
            settings.tax_shipping = tax_shipping;
        }
        if let Some(tax_handling) = input.tax_handling {
            settings.tax_handling = tax_handling;
        }
        if let Some(tax_gift_wrap) = input.tax_gift_wrap {
            settings.tax_gift_wrap = tax_gift_wrap;
        }
        if let Some(category) = input.default_product_category {
            settings.default_product_category = parse_product_tax_category(&category);
        }
        if let Some(mode) = input.rounding_mode {
            settings.rounding_mode = mode;
        }
        if let Some(places) = input.decimal_places {
            settings.decimal_places = places;
        }
        if let Some(validate) = input.validate_addresses {
            settings.validate_addresses = validate;
        }
        if let Some(provider) = input.tax_provider {
            settings.tax_provider = Some(provider);
        }

        let updated = commerce
            .tax()
            .update_settings(settings)
            .map_err(|e| Error::from_reason(format!("Failed to update settings: {}", e)))?;

        Ok(updated.into())
    }

    /// Enable or disable tax calculation
    #[napi]
    pub async fn set_enabled(&self, enabled: bool) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.lock().await;

        let settings = commerce
            .tax()
            .set_enabled(enabled)
            .map_err(|e| Error::from_reason(format!("Failed to update settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Check if tax calculation is enabled
    #[napi]
    pub async fn is_enabled(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;

        let enabled = commerce
            .tax()
            .is_enabled()
            .map_err(|e| Error::from_reason(format!("Failed to check settings: {}", e)))?;

        Ok(enabled)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get US state tax information
    #[napi]
    pub fn get_us_state_info(state_code: String) -> Result<Option<UsStateTaxInfoOutput>> {
        convert_optional_output(stateset_core::get_us_state_tax_info(&state_code))
    }

    /// Get EU VAT information
    #[napi]
    pub fn get_eu_vat_info(country_code: String) -> Result<Option<EuVatInfoOutput>> {
        convert_optional_output(stateset_core::get_eu_vat_info(&country_code))
    }

    /// Get Canadian tax information
    #[napi]
    pub fn get_canadian_tax_info(province_code: String) -> Result<Option<CanadianTaxInfoOutput>> {
        convert_optional_output(stateset_core::get_canadian_tax_info(&province_code))
    }

    /// Check if a country is in the EU
    #[napi]
    pub fn is_eu_country(country_code: String) -> bool {
        stateset_core::is_eu_member(&country_code)
    }
}

// ============================================================================
// Quality Control API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInspectionInput {
    pub inspection_type: String,
    pub reference_type: String,
    pub reference_id: String,
    pub warehouse_id: Option<i32>,
    pub assigned_to: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InspectionOutput {
    pub id: String,
    pub inspection_number: String,
    pub inspection_type: String,
    pub reference_type: String,
    pub reference_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Inspection> for InspectionOutput {
    fn from(i: stateset_core::Inspection) -> Self {
        Self {
            id: i.id.to_string(),
            inspection_number: i.inspection_number,
            inspection_type: format!("{:?}", i.inspection_type),
            reference_type: i.reference_type,
            reference_id: i.reference_id.to_string(),
            status: format!("{:?}", i.status),
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateNcrInput {
    pub source: String,
    pub severity: String,
    pub sku: String,
    pub quantity_affected: f64,
    pub description: String,
    pub lot_number: Option<String>,
    pub location_id: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct NcrOutput {
    pub id: String,
    pub ncr_number: String,
    pub source: String,
    pub severity: String,
    pub sku: String,
    pub quantity_affected: f64,
    pub status: String,
    pub description: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::NonConformance> for NcrOutput {
    type Error = Error;

    fn try_from(n: stateset_core::NonConformance) -> Result<Self> {
        Ok(Self {
            id: n.id.to_string(),
            ncr_number: n.ncr_number,
            source: format!("{:?}", n.source),
            severity: format!("{:?}", n.severity),
            sku: n.sku,
            quantity_affected: to_f64_result(n.quantity_affected, "ncr quantity affected")?,
            status: format!("{:?}", n.status),
            description: n.description,
            created_at: n.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateQualityHoldInput {
    pub sku: String,
    pub lot_number: Option<String>,
    pub quantity_held: f64,
    pub reason: String,
    pub hold_type: String,
    pub placed_by: Option<String>,
    pub location_id: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct QualityHoldOutput {
    pub id: String,
    pub sku: String,
    pub lot_number: Option<String>,
    pub quantity_held: f64,
    pub reason: String,
    pub hold_type: String,
    pub status: String,
    pub placed_at: String,
}

impl TryFrom<stateset_core::QualityHold> for QualityHoldOutput {
    type Error = Error;

    fn try_from(h: stateset_core::QualityHold) -> Result<Self> {
        Ok(Self {
            id: h.id.to_string(),
            sku: h.sku,
            lot_number: h.lot_number,
            quantity_held: to_f64_result(h.quantity_held, "quality hold quantity held")?,
            reason: h.reason,
            hold_type: format!("{:?}", h.hold_type),
            status: if h.released_at.is_some() {
                "released".to_string()
            } else {
                "held".to_string()
            },
            placed_at: h.placed_at.to_rfc3339(),
        })
    }
}

fn parse_inspection_type(s: &str) -> stateset_core::InspectionType {
    match s.to_lowercase().as_str() {
        "receiving" => stateset_core::InspectionType::Receiving,
        "in_process" | "inprocess" => stateset_core::InspectionType::InProcess,
        "final" => stateset_core::InspectionType::Final,
        "random" => stateset_core::InspectionType::Random,
        _ => stateset_core::InspectionType::Receiving,
    }
}

fn parse_ncr_source(s: &str) -> stateset_core::NonConformanceSource {
    match s.to_lowercase().as_str() {
        "inspection" => stateset_core::NonConformanceSource::Inspection,
        "production" | "production_defect" => stateset_core::NonConformanceSource::ProductionDefect,
        "customer" | "customer_complaint" => stateset_core::NonConformanceSource::CustomerComplaint,
        "supplier" | "supplier_issue" => stateset_core::NonConformanceSource::SupplierIssue,
        "internal_audit" => stateset_core::NonConformanceSource::InternalAudit,
        "shipping_damage" => stateset_core::NonConformanceSource::ShippingDamage,
        _ => stateset_core::NonConformanceSource::Inspection,
    }
}

fn parse_severity(s: &str) -> stateset_core::Severity {
    match s.to_lowercase().as_str() {
        "critical" => stateset_core::Severity::Critical,
        "major" => stateset_core::Severity::Major,
        "minor" => stateset_core::Severity::Minor,
        "observation" => stateset_core::Severity::Observation,
        _ => stateset_core::Severity::Minor,
    }
}

fn parse_hold_type(s: &str) -> stateset_core::HoldType {
    match s.to_lowercase().as_str() {
        "quality_inspection" | "qualityinspection" => stateset_core::HoldType::QualityInspection,
        "damage" | "damaged" => stateset_core::HoldType::Damaged,
        "regulatory" | "regulatory_hold" => stateset_core::HoldType::RegulatoryHold,
        "customer_return" | "customerreturn" => stateset_core::HoldType::CustomerReturn,
        "recall" => stateset_core::HoldType::Recall,
        "expired" => stateset_core::HoldType::Expired,
        "quarantine" => stateset_core::HoldType::Quarantine,
        "investigation" | "investigation_hold" => stateset_core::HoldType::InvestigationHold,
        _ => stateset_core::HoldType::QualityInspection,
    }
}

#[napi]
pub struct Quality {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Quality {
    /// Create a new inspection
    #[napi]
    pub async fn create_inspection(
        &self,
        input: CreateInspectionInput,
    ) -> Result<InspectionOutput> {
        let commerce = self.commerce.lock().await;
        let reference_id =
            input.reference_id.parse().map_err(|_| Error::from_reason("Invalid reference UUID"))?;

        let inspection = commerce
            .quality()
            .create_inspection(stateset_core::CreateInspection {
                inspection_type: parse_inspection_type(&input.inspection_type),
                reference_type: input.reference_type,
                reference_id,
                inspector_id: input.assigned_to,
                scheduled_at: None,
                notes: input.notes,
                items: vec![],
            })
            .map_err(|e| Error::from_reason(format!("Failed to create inspection: {}", e)))?;

        Ok(inspection.into())
    }

    /// Get an inspection by ID
    #[napi]
    pub async fn get_inspection(&self, id: String) -> Result<Option<InspectionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .get_inspection(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get inspection: {}", e)))?;
        Ok(inspection.map(|i| i.into()))
    }

    /// List all inspections
    #[napi]
    pub async fn list_inspections(&self) -> Result<Vec<InspectionOutput>> {
        let commerce = self.commerce.lock().await;
        let inspections = commerce
            .quality()
            .list_inspections(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list inspections: {}", e)))?;
        Ok(inspections.into_iter().map(|i| i.into()).collect())
    }

    /// Start an inspection
    #[napi]
    pub async fn start_inspection(&self, id: String) -> Result<InspectionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .start_inspection(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to start inspection: {}", e)))?;
        Ok(inspection.into())
    }

    /// Complete an inspection
    #[napi]
    pub async fn complete_inspection(&self, id: String) -> Result<InspectionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .complete_inspection(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to complete inspection: {}", e)))?;
        Ok(inspection.into())
    }

    /// Create a non-conformance report
    #[napi]
    pub async fn create_ncr(&self, input: CreateNcrInput) -> Result<NcrOutput> {
        let commerce = self.commerce.lock().await;
        let ncr = commerce
            .quality()
            .create_ncr(stateset_core::CreateNonConformance {
                inspection_id: None,
                source: parse_ncr_source(&input.source),
                severity: parse_severity(&input.severity),
                sku: input.sku,
                lot_number: input.lot_number,
                serial_number: None,
                quantity_affected: decimal_from_f64(
                    input.quantity_affected,
                    "ncr quantity affected",
                )?,
                description: input.description,
                assigned_to: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create NCR: {}", e)))?;
        convert_output(ncr)
    }

    /// Get an NCR by ID
    #[napi]
    pub async fn get_ncr(&self, id: String) -> Result<Option<NcrOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let ncr = commerce
            .quality()
            .get_ncr(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get NCR: {}", e)))?;
        convert_optional_output(ncr)
    }

    /// List all NCRs
    #[napi]
    pub async fn list_ncrs(&self) -> Result<Vec<NcrOutput>> {
        let commerce = self.commerce.lock().await;
        let ncrs = commerce
            .quality()
            .list_ncrs(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list NCRs: {}", e)))?;
        convert_outputs(ncrs)
    }

    /// Close an NCR
    #[napi]
    pub async fn close_ncr(&self, id: String) -> Result<NcrOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let ncr = commerce
            .quality()
            .close_ncr(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to close NCR: {}", e)))?;
        convert_output(ncr)
    }

    /// Create a quality hold
    #[napi]
    pub async fn create_hold(&self, input: CreateQualityHoldInput) -> Result<QualityHoldOutput> {
        let commerce = self.commerce.lock().await;
        let hold = commerce
            .quality()
            .create_hold(stateset_core::CreateQualityHold {
                sku: input.sku,
                lot_number: input.lot_number,
                serial_number: None,
                location_id: input.location_id,
                quantity: decimal_from_f64(input.quantity_held, "quality hold quantity")?,
                reason: input.reason,
                hold_type: parse_hold_type(&input.hold_type),
                ncr_id: None,
                inspection_id: None,
                placed_by: input.placed_by.unwrap_or_default(),
                expires_at: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create hold: {}", e)))?;
        convert_output(hold)
    }

    /// Get a quality hold by ID
    #[napi]
    pub async fn get_hold(&self, id: String) -> Result<Option<QualityHoldOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let hold = commerce
            .quality()
            .get_hold(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get hold: {}", e)))?;
        convert_optional_output(hold)
    }

    /// List all quality holds
    #[napi]
    pub async fn list_holds(&self) -> Result<Vec<QualityHoldOutput>> {
        let commerce = self.commerce.lock().await;
        let holds = commerce
            .quality()
            .list_holds(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list holds: {}", e)))?;
        convert_outputs(holds)
    }

    /// Release a quality hold
    #[napi]
    pub async fn release_hold(
        &self,
        id: String,
        released_by: String,
        notes: Option<String>,
    ) -> Result<QualityHoldOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let hold = commerce
            .quality()
            .release_hold(
                uuid,
                stateset_core::ReleaseQualityHold { released_by, release_notes: notes },
            )
            .map_err(|e| Error::from_reason(format!("Failed to release hold: {}", e)))?;
        convert_output(hold)
    }

    /// Get all active holds
    #[napi]
    pub async fn get_active_holds(&self) -> Result<Vec<QualityHoldOutput>> {
        let commerce = self.commerce.lock().await;
        let holds = commerce
            .quality()
            .get_active_holds()
            .map_err(|e| Error::from_reason(format!("Failed to get active holds: {}", e)))?;
        convert_outputs(holds)
    }

    /// Count active holds
    #[napi]
    pub async fn count_active_holds(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .quality()
            .count_active_holds()
            .map_err(|e| Error::from_reason(format!("Failed to count holds: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Lots/Batch Tracking API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateLotInput {
    pub lot_number: Option<String>,
    pub sku: String,
    pub quantity_produced: f64,
    pub production_date: Option<String>,
    pub expiration_date: Option<String>,
    pub supplier_lot_number: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LotOutput {
    pub id: String,
    pub lot_number: String,
    pub sku: String,
    pub quantity_produced: f64,
    pub quantity_available: f64,
    pub quantity_reserved: f64,
    pub status: String,
    pub production_date: Option<String>,
    pub expiration_date: Option<String>,
    pub created_at: String,
}

impl TryFrom<stateset_core::Lot> for LotOutput {
    type Error = Error;

    fn try_from(l: stateset_core::Lot) -> Result<Self> {
        let qty_available = l.quantity_available();
        Ok(Self {
            id: l.id.to_string(),
            lot_number: l.lot_number,
            sku: l.sku,
            quantity_produced: to_f64_result(l.quantity_produced, "lot quantity produced")?,
            quantity_available: to_f64_result(qty_available, "lot quantity available")?,
            quantity_reserved: to_f64_result(l.quantity_reserved, "lot quantity reserved")?,
            status: format!("{:?}", l.status),
            production_date: Some(l.production_date.to_rfc3339()),
            expiration_date: l.expiration_date.map(|d| d.to_rfc3339()),
            created_at: l.created_at.to_rfc3339(),
        })
    }
}

#[napi]
pub struct Lots {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Lots {
    /// Create a new lot
    #[napi]
    pub async fn create(&self, input: CreateLotInput) -> Result<LotOutput> {
        let commerce = self.commerce.lock().await;
        let lot = commerce
            .lots()
            .create(stateset_core::CreateLot {
                lot_number: input.lot_number,
                sku: input.sku,
                quantity: decimal_from_f64(input.quantity_produced, "lot quantity produced")?,
                production_date: input.production_date.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&chrono::Utc))
                }),
                expiration_date: input.expiration_date.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&chrono::Utc))
                }),
                supplier_lot: input.supplier_lot_number,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create lot: {}", e)))?;
        convert_output(lot)
    }

    /// Get a lot by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let lot = commerce
            .lots()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get lot: {}", e)))?;
        convert_optional_output(lot)
    }

    /// Get a lot by lot number
    #[napi]
    pub async fn get_by_number(&self, lot_number: String) -> Result<Option<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lot = commerce
            .lots()
            .get_by_number(&lot_number)
            .map_err(|e| Error::from_reason(format!("Failed to get lot: {}", e)))?;
        convert_optional_output(lot)
    }

    /// List all lots
    #[napi]
    pub async fn list(&self) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lots = commerce
            .lots()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list lots: {}", e)))?;
        convert_outputs(lots)
    }

    /// Get active lots for a SKU
    #[napi]
    pub async fn get_active_lots(&self, sku: String) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lots = commerce
            .lots()
            .get_active_lots(&sku)
            .map_err(|e| Error::from_reason(format!("Failed to get active lots: {}", e)))?;
        convert_outputs(lots)
    }

    /// Get available lots for a SKU (FIFO order)
    #[napi]
    pub async fn get_available_lots_for_sku(&self, sku: String) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lots = commerce
            .lots()
            .get_available_lots_for_sku(&sku)
            .map_err(|e| Error::from_reason(format!("Failed to get available lots: {}", e)))?;
        convert_outputs(lots)
    }

    /// Quarantine a lot
    #[napi]
    pub async fn quarantine(&self, id: String, reason: String) -> Result<LotOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let lot = commerce
            .lots()
            .quarantine(uuid, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to quarantine lot: {}", e)))?;
        convert_output(lot)
    }

    /// Release a lot from quarantine
    #[napi]
    pub async fn release_quarantine(&self, id: String) -> Result<LotOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let lot = commerce
            .lots()
            .release_quarantine(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to release lot: {}", e)))?;
        convert_output(lot)
    }

    /// Get expiring lots within days
    #[napi]
    pub async fn get_expiring_lots(&self, days: i32) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lots = commerce
            .lots()
            .get_expiring_lots(days)
            .map_err(|e| Error::from_reason(format!("Failed to get expiring lots: {}", e)))?;
        convert_outputs(lots)
    }

    /// Get expired lots
    #[napi]
    pub async fn get_expired_lots(&self) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lots = commerce
            .lots()
            .get_expired_lots()
            .map_err(|e| Error::from_reason(format!("Failed to get expired lots: {}", e)))?;
        convert_outputs(lots)
    }

    /// Get quarantined lots
    #[napi]
    pub async fn get_quarantined(&self) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.lock().await;
        let lots = commerce
            .lots()
            .get_quarantined()
            .map_err(|e| Error::from_reason(format!("Failed to get quarantined lots: {}", e)))?;
        convert_outputs(lots)
    }

    /// Count lots
    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .lots()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count lots: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Serial Numbers API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSerialInput {
    pub serial: Option<String>,
    pub sku: String,
    pub lot_number: Option<String>,
    pub manufactured_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SerialOutput {
    pub id: String,
    pub serial: String,
    pub sku: String,
    pub lot_id: Option<String>,
    pub status: String,
    pub owner_id: Option<String>,
    pub location_id: Option<i32>,
    pub created_at: String,
}

impl From<stateset_core::SerialNumber> for SerialOutput {
    fn from(s: stateset_core::SerialNumber) -> Self {
        Self {
            id: s.id.to_string(),
            serial: s.serial,
            sku: s.sku,
            lot_id: s.lot_id.map(|id| id.to_string()),
            status: format!("{:?}", s.status),
            owner_id: s.current_owner_id.map(|id| id.to_string()),
            location_id: s.current_location_id,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Serials {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Serials {
    /// Create a serial number
    #[napi]
    pub async fn create(&self, input: CreateSerialInput) -> Result<SerialOutput> {
        let commerce = self.commerce.lock().await;
        let serial = commerce
            .serials()
            .create(stateset_core::CreateSerialNumber {
                serial: input.serial,
                sku: input.sku,
                lot_number: input.lot_number,
                manufactured_at: input.manufactured_at.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&chrono::Utc))
                }),
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create serial: {}", e)))?;
        Ok(serial.into())
    }

    /// Get a serial by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SerialOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let serial = commerce
            .serials()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get serial: {}", e)))?;
        Ok(serial.map(|s| s.into()))
    }

    /// Get a serial by serial number string
    #[napi]
    pub async fn get_by_serial(&self, serial: String) -> Result<Option<SerialOutput>> {
        let commerce = self.commerce.lock().await;
        let s = commerce
            .serials()
            .get_by_serial(&serial)
            .map_err(|e| Error::from_reason(format!("Failed to get serial: {}", e)))?;
        Ok(s.map(|s| s.into()))
    }

    /// List all serials
    #[napi]
    pub async fn list(&self) -> Result<Vec<SerialOutput>> {
        let commerce = self.commerce.lock().await;
        let serials = commerce
            .serials()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list serials: {}", e)))?;
        Ok(serials.into_iter().map(|s| s.into()).collect())
    }

    /// Get available serials for a SKU
    #[napi]
    pub async fn get_available(&self, sku: String, limit: u32) -> Result<Vec<SerialOutput>> {
        let commerce = self.commerce.lock().await;
        let serials = commerce
            .serials()
            .get_available(&sku, limit)
            .map_err(|e| Error::from_reason(format!("Failed to get available serials: {}", e)))?;
        Ok(serials.into_iter().map(|s| s.into()).collect())
    }

    /// Mark a serial as sold
    #[napi]
    pub async fn mark_sold(
        &self,
        id: String,
        customer_id: String,
        order_id: Option<String>,
    ) -> Result<SerialOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id.parse().map_err(|_| Error::from_reason("Invalid serial UUID"))?;
        let cust_uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let order_uuid = order_id.and_then(|s| s.parse().ok());
        let serial = commerce
            .serials()
            .mark_sold(uuid, cust_uuid, order_uuid)
            .map_err(|e| Error::from_reason(format!("Failed to mark sold: {}", e)))?;
        Ok(serial.into())
    }

    /// Quarantine a serial
    #[napi]
    pub async fn quarantine(&self, id: String, reason: String) -> Result<SerialOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let serial = commerce
            .serials()
            .quarantine(uuid, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to quarantine serial: {}", e)))?;
        Ok(serial.into())
    }

    /// Check if a serial is available
    #[napi]
    pub async fn is_available(&self, serial: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let available = commerce
            .serials()
            .is_available(&serial)
            .map_err(|e| Error::from_reason(format!("Failed to check availability: {}", e)))?;
        Ok(available)
    }

    /// Count serials
    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .serials()
            .count(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count serials: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Warehouse API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWarehouseInput {
    pub code: String,
    pub name: String,
    pub warehouse_type: Option<String>,
    pub timezone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WarehouseOutput {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub warehouse_type: String,
    pub is_active: bool,
    pub timezone: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::Warehouse> for WarehouseOutput {
    fn from(w: stateset_core::Warehouse) -> Self {
        Self {
            id: w.id,
            code: w.code,
            name: w.name,
            warehouse_type: format!("{:?}", w.warehouse_type),
            is_active: w.is_active,
            timezone: w.timezone,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateLocationInput {
    pub warehouse_id: i32,
    pub location_type: String,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub bin: Option<String>,
    pub is_pickable: Option<bool>,
    pub is_receivable: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LocationOutput {
    pub id: i32,
    pub warehouse_id: i32,
    pub code: String,
    pub location_type: String,
    pub zone: Option<String>,
    pub aisle: Option<String>,
    pub rack: Option<String>,
    pub bin: Option<String>,
    pub is_active: bool,
    pub is_pickable: bool,
    pub is_receivable: bool,
}

impl From<stateset_core::Location> for LocationOutput {
    fn from(l: stateset_core::Location) -> Self {
        Self {
            id: l.id,
            warehouse_id: l.warehouse_id,
            code: l.code,
            location_type: format!("{:?}", l.location_type),
            zone: l.zone,
            aisle: l.aisle,
            rack: l.rack,
            bin: l.bin,
            is_active: l.is_active,
            is_pickable: l.is_pickable,
            is_receivable: l.is_receivable,
        }
    }
}

fn parse_warehouse_type(s: &str) -> stateset_core::WarehouseType {
    match s.to_lowercase().as_str() {
        "distribution" => stateset_core::WarehouseType::Distribution,
        "manufacturing" => stateset_core::WarehouseType::Manufacturing,
        "retail" => stateset_core::WarehouseType::Retail,
        "thirdparty" | "third_party" => stateset_core::WarehouseType::ThirdParty,
        _ => stateset_core::WarehouseType::Distribution,
    }
}

fn parse_location_type(s: &str) -> stateset_core::LocationType {
    match s.to_lowercase().as_str() {
        "pick" => stateset_core::LocationType::Pick,
        "bulk" => stateset_core::LocationType::Bulk,
        "receiving" => stateset_core::LocationType::Receiving,
        "shipping" => stateset_core::LocationType::Shipping,
        "staging" => stateset_core::LocationType::Staging,
        "quarantine" => stateset_core::LocationType::Quarantine,
        "returns" => stateset_core::LocationType::Returns,
        _ => stateset_core::LocationType::Pick,
    }
}

#[napi]
pub struct Warehouse {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Warehouse {
    /// Create a new warehouse
    #[napi]
    pub async fn create_warehouse(&self, input: CreateWarehouseInput) -> Result<WarehouseOutput> {
        let commerce = self.commerce.lock().await;
        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: input.code,
                name: input.name,
                warehouse_type: input
                    .warehouse_type
                    .map(|s| parse_warehouse_type(&s))
                    .unwrap_or(stateset_core::WarehouseType::Distribution),
                timezone: input.timezone,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create warehouse: {}", e)))?;
        Ok(warehouse.into())
    }

    /// Get a warehouse by ID
    #[napi]
    pub async fn get_warehouse(&self, id: i32) -> Result<Option<WarehouseOutput>> {
        let commerce = self.commerce.lock().await;
        let warehouse = commerce
            .warehouse()
            .get_warehouse(id)
            .map_err(|e| Error::from_reason(format!("Failed to get warehouse: {}", e)))?;
        Ok(warehouse.map(|w| w.into()))
    }

    /// Get a warehouse by code
    #[napi]
    pub async fn get_warehouse_by_code(&self, code: String) -> Result<Option<WarehouseOutput>> {
        let commerce = self.commerce.lock().await;
        let warehouse = commerce
            .warehouse()
            .get_warehouse_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get warehouse: {}", e)))?;
        Ok(warehouse.map(|w| w.into()))
    }

    /// List all warehouses
    #[napi]
    pub async fn list_warehouses(&self) -> Result<Vec<WarehouseOutput>> {
        let commerce = self.commerce.lock().await;
        let warehouses = commerce
            .warehouse()
            .list_warehouses(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list warehouses: {}", e)))?;
        Ok(warehouses.into_iter().map(|w| w.into()).collect())
    }

    /// Create a new location
    #[napi]
    pub async fn create_location(&self, input: CreateLocationInput) -> Result<LocationOutput> {
        let commerce = self.commerce.lock().await;
        let location = commerce
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id: input.warehouse_id,
                location_type: parse_location_type(&input.location_type),
                zone: input.zone,
                aisle: input.aisle,
                rack: input.rack,
                bin: input.bin,
                is_pickable: input.is_pickable,
                is_receivable: input.is_receivable,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create location: {}", e)))?;
        Ok(location.into())
    }

    /// Get a location by ID
    #[napi]
    pub async fn get_location(&self, id: i32) -> Result<Option<LocationOutput>> {
        let commerce = self.commerce.lock().await;
        let location = commerce
            .warehouse()
            .get_location(id)
            .map_err(|e| Error::from_reason(format!("Failed to get location: {}", e)))?;
        Ok(location.map(|l| l.into()))
    }

    /// List locations in a warehouse
    #[napi]
    pub async fn list_locations(&self, warehouse_id: Option<i32>) -> Result<Vec<LocationOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = stateset_core::LocationFilter { warehouse_id, ..Default::default() };
        let locations = commerce
            .warehouse()
            .list_locations(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list locations: {}", e)))?;
        Ok(locations.into_iter().map(|l| l.into()).collect())
    }

    /// Get pickable locations for a SKU
    #[napi]
    pub async fn get_pickable_locations(
        &self,
        warehouse_id: i32,
        sku: String,
    ) -> Result<Vec<LocationOutput>> {
        let commerce = self.commerce.lock().await;
        let locations = commerce
            .warehouse()
            .get_pickable_locations(warehouse_id, &sku)
            .map_err(|e| Error::from_reason(format!("Failed to get pickable locations: {}", e)))?;
        Ok(locations.into_iter().map(|l| l.into()).collect())
    }

    /// Get total available quantity for a SKU in a warehouse
    #[napi]
    pub async fn get_total_available(&self, warehouse_id: i32, sku: String) -> Result<f64> {
        let commerce = self.commerce.lock().await;
        let total = commerce
            .warehouse()
            .get_total_available(warehouse_id, &sku)
            .map_err(|e| Error::from_reason(format!("Failed to get total: {}", e)))?;
        to_f64_result(total, "warehouse total available")
    }

    /// Count warehouses
    #[napi]
    pub async fn count_warehouses(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .warehouse()
            .count_warehouses(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count warehouses: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Receiving API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReceiptInput {
    pub receipt_type: String,
    pub warehouse_id: i32,
    pub purchase_order_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReceiptOutput {
    pub id: String,
    pub receipt_number: String,
    pub receipt_type: String,
    pub warehouse_id: i32,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::Receipt> for ReceiptOutput {
    fn from(r: stateset_core::Receipt) -> Self {
        Self {
            id: r.id.to_string(),
            receipt_number: r.receipt_number,
            receipt_type: format!("{:?}", r.receipt_type),
            warehouse_id: r.warehouse_id,
            status: format!("{:?}", r.status),
            carrier: r.carrier,
            tracking_number: r.tracking_number,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

fn parse_receipt_type(s: &str) -> stateset_core::ReceiptType {
    match s.to_lowercase().as_str() {
        "purchase_order" | "purchaseorder" | "po" => stateset_core::ReceiptType::PurchaseOrder,
        "return" | "customer_return" => stateset_core::ReceiptType::Return,
        "transfer" => stateset_core::ReceiptType::Transfer,
        "adjustment" => stateset_core::ReceiptType::Adjustment,
        _ => stateset_core::ReceiptType::PurchaseOrder,
    }
}

#[napi]
pub struct Receiving {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Receiving {
    /// Create a new receipt
    #[napi]
    pub async fn create_receipt(&self, input: CreateReceiptInput) -> Result<ReceiptOutput> {
        let commerce = self.commerce.lock().await;
        let receipt = commerce
            .receiving()
            .create_receipt(stateset_core::CreateReceipt {
                receipt_number: None,
                receipt_type: parse_receipt_type(&input.receipt_type),
                reference_type: input
                    .purchase_order_id
                    .as_ref()
                    .map(|_| "purchase_order".to_string()),
                reference_id: input.purchase_order_id.and_then(|s| s.parse().ok()),
                supplier_id: None,
                warehouse_id: input.warehouse_id,
                carrier: input.carrier,
                tracking_number: input.tracking_number,
                expected_date: None,
                notes: None,
                created_by: None,
                items: vec![],
            })
            .map_err(|e| Error::from_reason(format!("Failed to create receipt: {}", e)))?;
        Ok(receipt.into())
    }

    /// Get a receipt by ID
    #[napi]
    pub async fn get_receipt(&self, id: String) -> Result<Option<ReceiptOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .get_receipt(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get receipt: {}", e)))?;
        Ok(receipt.map(|r| r.into()))
    }

    /// Get a receipt by receipt number
    #[napi]
    pub async fn get_receipt_by_number(&self, number: String) -> Result<Option<ReceiptOutput>> {
        let commerce = self.commerce.lock().await;
        let receipt = commerce
            .receiving()
            .get_receipt_by_number(&number)
            .map_err(|e| Error::from_reason(format!("Failed to get receipt: {}", e)))?;
        Ok(receipt.map(|r| r.into()))
    }

    /// List all receipts
    #[napi]
    pub async fn list_receipts(&self) -> Result<Vec<ReceiptOutput>> {
        let commerce = self.commerce.lock().await;
        let receipts = commerce
            .receiving()
            .list_receipts(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list receipts: {}", e)))?;
        Ok(receipts.into_iter().map(|r| r.into()).collect())
    }

    /// Start receiving
    #[napi]
    pub async fn start_receiving(&self, id: String) -> Result<ReceiptOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .start_receiving(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to start receiving: {}", e)))?;
        Ok(receipt.into())
    }

    /// Complete receiving
    #[napi]
    pub async fn complete_receiving(&self, id: String) -> Result<ReceiptOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .complete_receiving(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to complete receiving: {}", e)))?;
        Ok(receipt.into())
    }

    /// Cancel a receipt
    #[napi]
    pub async fn cancel_receipt(&self, id: String) -> Result<ReceiptOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .cancel_receipt(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel receipt: {}", e)))?;
        Ok(receipt.into())
    }

    /// Create a receipt from a purchase order
    #[napi]
    pub async fn create_receipt_from_po(
        &self,
        po_id: String,
        warehouse_id: i32,
    ) -> Result<ReceiptOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = po_id.parse().map_err(|_| Error::from_reason("Invalid PO UUID"))?;
        let receipt = commerce
            .receiving()
            .create_receipt_from_po(uuid, warehouse_id)
            .map_err(|e| Error::from_reason(format!("Failed to create receipt from PO: {}", e)))?;
        Ok(receipt.into())
    }

    /// Count receipts
    #[napi]
    pub async fn count_receipts(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .receiving()
            .count_receipts(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count receipts: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Fulfillment API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWaveInput {
    pub warehouse_id: i32,
    pub order_ids: Vec<String>,
    pub priority: Option<i32>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WaveOutput {
    pub id: String,
    pub wave_number: String,
    pub warehouse_id: i32,
    pub order_count: i32,
    pub status: String,
    pub created_at: String,
}

impl From<stateset_core::Wave> for WaveOutput {
    fn from(w: stateset_core::Wave) -> Self {
        Self {
            id: w.id.to_string(),
            wave_number: w.wave_number,
            warehouse_id: w.warehouse_id,
            order_count: w.order_count,
            status: format!("{:?}", w.status),
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PickTaskOutput {
    pub id: String,
    pub wave_id: Option<String>,
    pub order_id: String,
    pub sku: String,
    pub quantity_requested: f64,
    pub quantity_picked: f64,
    pub status: String,
    pub source_location_id: i32,
}

impl TryFrom<stateset_core::PickTask> for PickTaskOutput {
    type Error = Error;

    fn try_from(p: stateset_core::PickTask) -> Result<Self> {
        Ok(Self {
            id: p.id.to_string(),
            wave_id: p.wave_id.map(|id| id.to_string()),
            order_id: p.order_id.to_string(),
            sku: p.sku,
            quantity_requested: to_f64_result(p.quantity_requested, "pick quantity requested")?,
            quantity_picked: to_f64_result(p.quantity_picked, "pick quantity picked")?,
            status: format!("{:?}", p.status),
            source_location_id: p.source_location_id,
        })
    }
}

#[napi]
pub struct Fulfillment {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Fulfillment {
    /// Create a wave
    #[napi]
    pub async fn create_wave(&self, input: CreateWaveInput) -> Result<WaveOutput> {
        let commerce = self.commerce.lock().await;
        let order_ids: Vec<OrderId> = input
            .order_ids
            .iter()
            .filter_map(|s| s.parse::<uuid::Uuid>().ok())
            .map(OrderId::from)
            .collect();
        let wave = commerce
            .fulfillment()
            .create_wave(stateset_core::CreateWave {
                warehouse_id: input.warehouse_id,
                order_ids,
                priority: input.priority,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create wave: {}", e)))?;
        Ok(wave.into())
    }

    /// Get a wave by ID
    #[napi]
    pub async fn get_wave(&self, id: String) -> Result<Option<WaveOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .get_wave(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get wave: {}", e)))?;
        Ok(wave.map(|w| w.into()))
    }

    /// List all waves
    #[napi]
    pub async fn list_waves(&self) -> Result<Vec<WaveOutput>> {
        let commerce = self.commerce.lock().await;
        let waves = commerce
            .fulfillment()
            .list_waves(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list waves: {}", e)))?;
        Ok(waves.into_iter().map(|w| w.into()).collect())
    }

    /// Release a wave for picking
    #[napi]
    pub async fn release_wave(&self, id: String) -> Result<WaveOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .release_wave(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to release wave: {}", e)))?;
        Ok(wave.into())
    }

    /// Complete a wave
    #[napi]
    pub async fn complete_wave(&self, id: String) -> Result<WaveOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .complete_wave(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to complete wave: {}", e)))?;
        Ok(wave.into())
    }

    /// Cancel a wave
    #[napi]
    pub async fn cancel_wave(&self, id: String) -> Result<WaveOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .cancel_wave(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel wave: {}", e)))?;
        Ok(wave.into())
    }

    /// Get a pick task by ID
    #[napi]
    pub async fn get_pick(&self, id: String) -> Result<Option<PickTaskOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .get_pick(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get pick: {}", e)))?;
        convert_optional_output(pick)
    }

    /// List pick tasks
    #[napi]
    pub async fn list_picks(&self) -> Result<Vec<PickTaskOutput>> {
        let commerce = self.commerce.lock().await;
        let picks = commerce
            .fulfillment()
            .list_picks(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list picks: {}", e)))?;
        convert_outputs(picks)
    }

    /// Assign a pick task
    #[napi]
    pub async fn assign_pick(&self, id: String, assigned_to: String) -> Result<PickTaskOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .assign_pick(uuid, &assigned_to)
            .map_err(|e| Error::from_reason(format!("Failed to assign pick: {}", e)))?;
        convert_output(pick)
    }

    /// Start a pick task
    #[napi]
    pub async fn start_pick(&self, id: String) -> Result<PickTaskOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .start_pick(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to start pick: {}", e)))?;
        convert_output(pick)
    }

    /// Cancel a pick task
    #[napi]
    pub async fn cancel_pick(&self, id: String) -> Result<PickTaskOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let pick = commerce
            .fulfillment()
            .cancel_pick(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel pick: {}", e)))?;
        convert_output(pick)
    }

    /// Check if an order is ready to pack
    #[napi]
    pub async fn is_order_ready_to_pack(&self, order_id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = order_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let ready = commerce
            .fulfillment()
            .is_order_ready_to_pack(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to check: {}", e)))?;
        Ok(ready)
    }

    /// Check if an order is ready to ship
    #[napi]
    pub async fn is_order_ready_to_ship(&self, order_id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = order_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let ready = commerce
            .fulfillment()
            .is_order_ready_to_ship(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to check: {}", e)))?;
        Ok(ready)
    }

    /// Count waves
    #[napi]
    pub async fn count_waves(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .fulfillment()
            .count_waves(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count waves: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Accounts Payable API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBillInput {
    pub supplier_id: String,
    pub due_date: String,
    pub payment_terms: Option<String>,
    pub reference_number: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillOutput {
    pub id: String,
    pub bill_number: String,
    pub supplier_id: String,
    pub status: String,
    pub total_amount: f64,
    pub amount_paid: f64,
    pub amount_due: f64,
    pub due_date: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::Bill> for BillOutput {
    type Error = Error;

    fn try_from(b: stateset_core::Bill) -> Result<Self> {
        Ok(Self {
            id: b.id.to_string(),
            bill_number: b.bill_number,
            supplier_id: b.supplier_id.to_string(),
            status: format!("{:?}", b.status),
            total_amount: to_f64_result(b.total_amount, "bill total amount")?,
            amount_paid: to_f64_result(b.amount_paid, "bill amount paid")?,
            amount_due: to_f64_result(b.amount_due, "bill amount due")?,
            due_date: b.due_date.to_rfc3339(),
            created_at: b.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApAgingSummaryOutput {
    pub current: f64,
    pub days_1_30: f64,
    pub days_31_60: f64,
    pub days_61_90: f64,
    pub days_over_90: f64,
    pub total: f64,
}

impl TryFrom<stateset_core::ApAgingSummary> for ApAgingSummaryOutput {
    type Error = Error;

    fn try_from(a: stateset_core::ApAgingSummary) -> Result<Self> {
        Ok(Self {
            current: to_f64_result(a.current, "accounts payable aging current")?,
            days_1_30: to_f64_result(a.days_1_30, "accounts payable aging 1-30 days")?,
            days_31_60: to_f64_result(a.days_31_60, "accounts payable aging 31-60 days")?,
            days_61_90: to_f64_result(a.days_61_90, "accounts payable aging 61-90 days")?,
            days_over_90: to_f64_result(a.days_over_90, "accounts payable aging over 90 days")?,
            total: to_f64_result(a.total, "accounts payable aging total")?,
        })
    }
}

#[napi]
pub struct AccountsPayable {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl AccountsPayable {
    /// Create a bill
    #[napi]
    pub async fn create_bill(&self, input: CreateBillInput) -> Result<BillOutput> {
        let commerce = self.commerce.lock().await;
        let supplier_id =
            input.supplier_id.parse().map_err(|_| Error::from_reason("Invalid supplier UUID"))?;
        let due_date = chrono::DateTime::parse_from_rfc3339(&input.due_date)
            .map_err(|_| Error::from_reason("Invalid due date format"))?
            .with_timezone(&chrono::Utc);
        let bill = commerce
            .accounts_payable()
            .create_bill(stateset_core::CreateBill {
                bill_number: None,
                supplier_id,
                purchase_order_id: None,
                bill_date: None,
                due_date,
                payment_terms: input.payment_terms,
                currency: None,
                reference_number: input.reference_number,
                memo: input.notes,
                items: vec![],
            })
            .map_err(|e| Error::from_reason(format!("Failed to create bill: {}", e)))?;
        convert_output(bill)
    }

    /// Get a bill by ID
    #[napi]
    pub async fn get_bill(&self, id: String) -> Result<Option<BillOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .get_bill(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get bill: {}", e)))?;
        convert_optional_output(bill)
    }

    /// Get a bill by bill number
    #[napi]
    pub async fn get_bill_by_number(&self, number: String) -> Result<Option<BillOutput>> {
        let commerce = self.commerce.lock().await;
        let bill = commerce
            .accounts_payable()
            .get_bill_by_number(&number)
            .map_err(|e| Error::from_reason(format!("Failed to get bill: {}", e)))?;
        convert_optional_output(bill)
    }

    /// List all bills
    #[napi]
    pub async fn list_bills(&self) -> Result<Vec<BillOutput>> {
        let commerce = self.commerce.lock().await;
        let bills = commerce
            .accounts_payable()
            .list_bills(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list bills: {}", e)))?;
        convert_outputs(bills)
    }

    /// Approve a bill
    #[napi]
    pub async fn approve_bill(&self, id: String) -> Result<BillOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .approve_bill(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to approve bill: {}", e)))?;
        convert_output(bill)
    }

    /// Cancel a bill
    #[napi]
    pub async fn cancel_bill(&self, id: String) -> Result<BillOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .cancel_bill(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel bill: {}", e)))?;
        convert_output(bill)
    }

    /// Get overdue bills
    #[napi]
    pub async fn get_overdue_bills(&self) -> Result<Vec<BillOutput>> {
        let commerce = self.commerce.lock().await;
        let bills = commerce
            .accounts_payable()
            .get_overdue_bills()
            .map_err(|e| Error::from_reason(format!("Failed to get overdue bills: {}", e)))?;
        convert_outputs(bills)
    }

    /// Get bills due soon
    #[napi]
    pub async fn get_bills_due_soon(&self, days: i32) -> Result<Vec<BillOutput>> {
        let commerce = self.commerce.lock().await;
        let bills = commerce
            .accounts_payable()
            .get_bills_due_soon(days)
            .map_err(|e| Error::from_reason(format!("Failed to get bills: {}", e)))?;
        convert_outputs(bills)
    }

    /// Get aging summary
    #[napi]
    pub async fn get_aging_summary(&self) -> Result<ApAgingSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let aging = commerce
            .accounts_payable()
            .get_aging_summary()
            .map_err(|e| Error::from_reason(format!("Failed to get aging: {}", e)))?;
        convert_output(aging)
    }

    /// Get total outstanding
    #[napi]
    pub async fn get_total_outstanding(&self) -> Result<f64> {
        let commerce = self.commerce.lock().await;
        let total = commerce
            .accounts_payable()
            .get_total_outstanding()
            .map_err(|e| Error::from_reason(format!("Failed to get total: {}", e)))?;
        to_f64_result(total, "accounts payable total outstanding")
    }

    /// Count bills
    #[napi]
    pub async fn count_bills(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .accounts_payable()
            .count_bills(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to count bills: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// Accounts Receivable API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ArAgingSummaryOutput {
    pub current: f64,
    pub days_1_30: f64,
    pub days_31_60: f64,
    pub days_61_90: f64,
    pub days_over_90: f64,
    pub total: f64,
}

impl TryFrom<stateset_core::ArAgingSummary> for ArAgingSummaryOutput {
    type Error = Error;

    fn try_from(a: stateset_core::ArAgingSummary) -> Result<Self> {
        Ok(Self {
            current: to_f64_result(a.current, "accounts receivable aging current")?,
            days_1_30: to_f64_result(a.days_1_30, "accounts receivable aging 1-30 days")?,
            days_31_60: to_f64_result(a.days_31_60, "accounts receivable aging 31-60 days")?,
            days_61_90: to_f64_result(a.days_61_90, "accounts receivable aging 61-90 days")?,
            days_over_90: to_f64_result(a.days_over_90, "accounts receivable aging over 90 days")?,
            total: to_f64_result(a.total, "accounts receivable aging total")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCreditMemoInput {
    pub customer_id: String,
    pub original_invoice_id: Option<String>,
    pub reason: String,
    pub amount: f64,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditMemoOutput {
    pub id: String,
    pub credit_memo_number: String,
    pub customer_id: String,
    pub amount: f64,
    pub status: String,
    pub reason: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::CreditMemo> for CreditMemoOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditMemo) -> Result<Self> {
        Ok(Self {
            id: c.id.to_string(),
            credit_memo_number: c.credit_memo_number,
            customer_id: c.customer_id.to_string(),
            amount: to_f64_result(c.amount, "credit memo amount")?,
            status: format!("{:?}", c.status),
            reason: format!("{:?}", c.reason),
            created_at: c.created_at.to_rfc3339(),
        })
    }
}

fn parse_credit_memo_reason(s: &str) -> stateset_core::CreditMemoReason {
    match s.to_lowercase().as_str() {
        "returned_goods" | "returnedgoods" | "return" => {
            stateset_core::CreditMemoReason::ReturnedGoods
        }
        "pricing_error" | "pricingerror" | "billing_error" => {
            stateset_core::CreditMemoReason::PricingError
        }
        "overpayment" => stateset_core::CreditMemoReason::Overpayment,
        "damaged" => stateset_core::CreditMemoReason::Damaged,
        "service_credit" | "servicecredit" => stateset_core::CreditMemoReason::ServiceCredit,
        "goodwill" | "goodwill_adjustment" => stateset_core::CreditMemoReason::GoodwillAdjustment,
        _ => stateset_core::CreditMemoReason::Other,
    }
}

#[napi]
pub struct AccountsReceivable {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl AccountsReceivable {
    /// Get AR aging summary
    #[napi]
    pub async fn get_aging_summary(&self) -> Result<ArAgingSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let aging = commerce
            .accounts_receivable()
            .get_aging_summary()
            .map_err(|e| Error::from_reason(format!("Failed to get aging: {}", e)))?;
        convert_output(aging)
    }

    /// Get total outstanding
    #[napi]
    pub async fn get_total_outstanding(&self) -> Result<f64> {
        let commerce = self.commerce.lock().await;
        let total = commerce
            .accounts_receivable()
            .get_total_outstanding()
            .map_err(|e| Error::from_reason(format!("Failed to get total: {}", e)))?;
        to_f64_result(total, "accounts receivable total outstanding")
    }

    /// Get Days Sales Outstanding (DSO)
    #[napi]
    pub async fn get_dso(&self, days: i32) -> Result<f64> {
        let commerce = self.commerce.lock().await;
        let dso = commerce
            .accounts_receivable()
            .get_dso(days)
            .map_err(|e| Error::from_reason(format!("Failed to get DSO: {}", e)))?;
        to_f64_result(dso, "days sales outstanding")
    }

    /// Create a credit memo
    #[napi]
    pub async fn create_credit_memo(
        &self,
        input: CreateCreditMemoInput,
    ) -> Result<CreditMemoOutput> {
        let commerce = self.commerce.lock().await;
        let customer_id =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let memo = commerce
            .accounts_receivable()
            .create_credit_memo(stateset_core::CreateCreditMemo {
                customer_id,
                original_invoice_id: input.original_invoice_id.and_then(|s| s.parse().ok()),
                reason: parse_credit_memo_reason(&input.reason),
                amount: decimal_from_f64(input.amount, "credit memo amount")?,
                notes: input.notes,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create credit memo: {}", e)))?;
        convert_output(memo)
    }

    /// Get a credit memo by ID
    #[napi]
    pub async fn get_credit_memo(&self, id: String) -> Result<Option<CreditMemoOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let memo = commerce
            .accounts_receivable()
            .get_credit_memo(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get credit memo: {}", e)))?;
        convert_optional_output(memo)
    }

    /// List credit memos
    #[napi]
    pub async fn list_credit_memos(&self) -> Result<Vec<CreditMemoOutput>> {
        let commerce = self.commerce.lock().await;
        let memos = commerce
            .accounts_receivable()
            .list_credit_memos(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list credit memos: {}", e)))?;
        convert_outputs(memos)
    }

    /// Void a credit memo
    #[napi]
    pub async fn void_credit_memo(&self, id: String) -> Result<CreditMemoOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let memo = commerce
            .accounts_receivable()
            .void_credit_memo(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to void credit memo: {}", e)))?;
        convert_output(memo)
    }

    /// Get unapplied credits for a customer
    #[napi]
    pub async fn get_unapplied_credits(
        &self,
        customer_id: String,
    ) -> Result<Vec<CreditMemoOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let memos = commerce
            .accounts_receivable()
            .get_unapplied_credits(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get credits: {}", e)))?;
        convert_outputs(memos)
    }
}

// ============================================================================
// Cost Accounting API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SetItemCostInput {
    pub sku: String,
    pub cost_method: Option<String>,
    pub standard_cost: Option<f64>,
    pub material_cost: Option<f64>,
    pub labor_cost: Option<f64>,
    pub overhead_cost: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ItemCostOutput {
    pub id: String,
    pub sku: String,
    pub cost_method: String,
    pub standard_cost: f64,
    pub average_cost: f64,
    pub last_cost: f64,
    pub material_cost: f64,
    pub labor_cost: f64,
    pub overhead_cost: f64,
}

impl TryFrom<stateset_core::ItemCost> for ItemCostOutput {
    type Error = Error;

    fn try_from(c: stateset_core::ItemCost) -> Result<Self> {
        Ok(Self {
            id: c.id.to_string(),
            sku: c.sku,
            cost_method: format!("{:?}", c.cost_method),
            standard_cost: to_f64_result(c.standard_cost, "standard cost")?,
            average_cost: to_f64_result(c.average_cost, "average cost")?,
            last_cost: to_f64_result(c.last_cost, "last cost")?,
            material_cost: to_f64_result(c.material_cost, "material cost")?,
            labor_cost: to_f64_result(c.labor_cost, "labor cost")?,
            overhead_cost: to_f64_result(c.overhead_cost, "overhead cost")?,
        })
    }
}

fn parse_cost_method(s: &str) -> stateset_core::CostMethod {
    match s.to_lowercase().as_str() {
        "standard" => stateset_core::CostMethod::Standard,
        "average" => stateset_core::CostMethod::Average,
        "fifo" => stateset_core::CostMethod::Fifo,
        "lifo" => stateset_core::CostMethod::Lifo,
        _ => stateset_core::CostMethod::Average,
    }
}

#[napi]
pub struct CostAccounting {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl CostAccounting {
    /// Get item cost
    #[napi]
    pub async fn get_item_cost(&self, sku: String) -> Result<Option<ItemCostOutput>> {
        let commerce = self.commerce.lock().await;
        let cost = commerce
            .cost_accounting()
            .get_item_cost(&sku)
            .map_err(|e| Error::from_reason(format!("Failed to get cost: {}", e)))?;
        convert_optional_output(cost)
    }

    /// Set item cost
    #[napi]
    pub async fn set_item_cost(&self, input: SetItemCostInput) -> Result<ItemCostOutput> {
        let commerce = self.commerce.lock().await;
        let cost = commerce
            .cost_accounting()
            .set_item_cost(stateset_core::SetItemCost {
                sku: input.sku,
                cost_method: input.cost_method.map(|s| parse_cost_method(&s)),
                standard_cost: optional_decimal_from_f64(
                    input.standard_cost,
                    "item standard cost",
                )?,
                material_cost: optional_decimal_from_f64(
                    input.material_cost,
                    "item material cost",
                )?,
                labor_cost: optional_decimal_from_f64(input.labor_cost, "item labor cost")?,
                overhead_cost: optional_decimal_from_f64(
                    input.overhead_cost,
                    "item overhead cost",
                )?,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to set cost: {}", e)))?;
        convert_output(cost)
    }

    /// List all item costs
    #[napi]
    pub async fn list_item_costs(&self) -> Result<Vec<ItemCostOutput>> {
        let commerce = self.commerce.lock().await;
        let costs = commerce
            .cost_accounting()
            .list_item_costs(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list costs: {}", e)))?;
        convert_outputs(costs)
    }

    /// Update average cost
    #[napi]
    pub async fn update_average_cost(
        &self,
        sku: String,
        quantity: f64,
        unit_cost: f64,
    ) -> Result<ItemCostOutput> {
        let commerce = self.commerce.lock().await;
        let cost = commerce
            .cost_accounting()
            .update_average_cost(
                &sku,
                decimal_from_f64(quantity, "average cost quantity")?,
                decimal_from_f64(unit_cost, "average cost unit cost")?,
            )
            .map_err(|e| Error::from_reason(format!("Failed to update cost: {}", e)))?;
        convert_output(cost)
    }

    /// Get total inventory value
    #[napi]
    pub async fn get_total_inventory_value(&self) -> Result<f64> {
        let commerce = self.commerce.lock().await;
        let total = commerce
            .cost_accounting()
            .get_total_inventory_value()
            .map_err(|e| Error::from_reason(format!("Failed to get value: {}", e)))?;
        to_f64_result(total, "inventory value")
    }
}

// ============================================================================
// Credit Management API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCreditAccountInput {
    pub customer_id: String,
    pub credit_limit: f64,
    pub payment_terms: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditAccountOutput {
    pub id: String,
    pub customer_id: String,
    pub credit_limit: f64,
    pub credit_used: f64,
    pub credit_available: f64,
    pub status: String,
    pub payment_terms: Option<String>,
}

impl TryFrom<stateset_core::CreditAccount> for CreditAccountOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditAccount) -> Result<Self> {
        Ok(Self {
            id: c.id.to_string(),
            customer_id: c.customer_id.to_string(),
            credit_limit: to_f64_result(c.credit_limit, "credit limit")?,
            credit_used: to_f64_result(c.current_balance, "credit used")?,
            credit_available: to_f64_result(c.available_credit, "credit available")?,
            status: format!("{:?}", c.status),
            payment_terms: c.payment_terms,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditCheckOutput {
    pub approved: bool,
    pub reason: Option<String>,
    pub available_credit: f64,
    pub requires_approval: bool,
}

impl TryFrom<stateset_core::CreditCheckResult> for CreditCheckOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditCheckResult) -> Result<Self> {
        Ok(Self {
            approved: c.approved,
            reason: c.reason,
            available_credit: to_f64_result(c.available_credit, "available credit")?,
            requires_approval: c.requires_approval,
        })
    }
}

#[napi]
pub struct Credit {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Credit {
    /// Create a credit account
    #[napi]
    pub async fn create_credit_account(
        &self,
        input: CreateCreditAccountInput,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.lock().await;
        let customer_id =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let account = commerce
            .credit()
            .create_credit_account(stateset_core::CreateCreditAccount {
                customer_id,
                credit_limit: decimal_from_f64(input.credit_limit, "credit limit")?,
                payment_terms: input.payment_terms,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create credit account: {}", e)))?;
        convert_output(account)
    }

    /// Get a credit account by ID
    #[napi]
    pub async fn get_credit_account(&self, id: String) -> Result<Option<CreditAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .credit()
            .get_credit_account(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get account: {}", e)))?;
        convert_optional_output(account)
    }

    /// Get credit account by customer
    #[napi]
    pub async fn get_credit_account_by_customer(
        &self,
        customer_id: String,
    ) -> Result<Option<CreditAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .credit()
            .get_credit_account_by_customer(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get account: {}", e)))?;
        convert_optional_output(account)
    }

    /// List credit accounts
    #[napi]
    pub async fn list_credit_accounts(&self) -> Result<Vec<CreditAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let accounts = commerce
            .credit()
            .list_credit_accounts(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list accounts: {}", e)))?;
        convert_outputs(accounts)
    }

    /// Check credit
    #[napi]
    pub async fn check_credit(
        &self,
        customer_id: String,
        order_amount: f64,
    ) -> Result<CreditCheckOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let result = commerce
            .credit()
            .check_credit(uuid, decimal_from_f64(order_amount, "order amount")?)
            .map_err(|e| Error::from_reason(format!("Failed to check credit: {}", e)))?;
        convert_output(result)
    }

    /// Adjust credit limit
    #[napi]
    pub async fn adjust_credit_limit(
        &self,
        customer_id: String,
        new_limit: f64,
        reason: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .credit()
            .adjust_credit_limit(uuid, decimal_from_f64(new_limit, "new credit limit")?, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to adjust limit: {}", e)))?;
        convert_output(account)
    }

    /// Suspend credit account
    #[napi]
    pub async fn suspend_credit_account(
        &self,
        customer_id: String,
        reason: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .credit()
            .suspend_credit_account(uuid, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to suspend account: {}", e)))?;
        convert_output(account)
    }

    /// Reactivate credit account
    #[napi]
    pub async fn reactivate_credit_account(
        &self,
        customer_id: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .credit()
            .reactivate_credit_account(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to reactivate account: {}", e)))?;
        convert_output(account)
    }

    /// Get over-limit customers
    #[napi]
    pub async fn get_over_limit_customers(&self) -> Result<Vec<CreditAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let accounts = commerce
            .credit()
            .get_over_limit_customers()
            .map_err(|e| Error::from_reason(format!("Failed to get accounts: {}", e)))?;
        convert_outputs(accounts)
    }
}

// ============================================================================
// Backorder Management API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBackorderInput {
    pub order_id: String,
    pub customer_id: String,
    pub sku: String,
    pub quantity: f64,
    pub priority: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BackorderOutput {
    pub id: String,
    pub backorder_number: String,
    pub order_id: String,
    pub customer_id: String,
    pub sku: String,
    pub quantity_ordered: f64,
    pub quantity_fulfilled: f64,
    pub quantity_remaining: f64,
    pub status: String,
    pub priority: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::Backorder> for BackorderOutput {
    type Error = Error;

    fn try_from(b: stateset_core::Backorder) -> Result<Self> {
        Ok(Self {
            id: b.id.to_string(),
            backorder_number: b.backorder_number,
            order_id: b.order_id.to_string(),
            customer_id: b.customer_id.to_string(),
            sku: b.sku,
            quantity_ordered: to_f64_result(b.quantity_ordered, "backorder quantity ordered")?,
            quantity_fulfilled: to_f64_result(
                b.quantity_fulfilled,
                "backorder quantity fulfilled",
            )?,
            quantity_remaining: to_f64_result(
                b.quantity_remaining,
                "backorder quantity remaining",
            )?,
            status: format!("{:?}", b.status),
            priority: format!("{:?}", b.priority),
            created_at: b.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BackorderSummaryOutput {
    pub total_backorders: i32,
    pub critical_count: i32,
    pub overdue_count: i32,
    pub total_value: f64,
}

impl TryFrom<stateset_core::BackorderSummary> for BackorderSummaryOutput {
    type Error = Error;

    fn try_from(s: stateset_core::BackorderSummary) -> Result<Self> {
        Ok(Self {
            total_backorders: s.total_backorders,
            critical_count: s.critical_count,
            overdue_count: s.overdue_count,
            total_value: to_f64_result(s.total_quantity, "backorder total quantity")?,
        })
    }
}

fn parse_backorder_priority(s: &str) -> stateset_core::BackorderPriority {
    match s.to_lowercase().as_str() {
        "critical" => stateset_core::BackorderPriority::Critical,
        "high" => stateset_core::BackorderPriority::High,
        "normal" => stateset_core::BackorderPriority::Normal,
        "low" => stateset_core::BackorderPriority::Low,
        _ => stateset_core::BackorderPriority::Normal,
    }
}

#[napi]
pub struct Backorders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Backorders {
    /// Create a backorder
    #[napi]
    pub async fn create_backorder(&self, input: CreateBackorderInput) -> Result<BackorderOutput> {
        let commerce = self.commerce.lock().await;
        let order_id =
            input.order_id.parse().map_err(|_| Error::from_reason("Invalid order UUID"))?;
        let customer_id =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let backorder = commerce
            .backorder()
            .create_backorder(stateset_core::CreateBackorder {
                order_id,
                order_line_id: None,
                customer_id,
                sku: input.sku,
                quantity: decimal_from_f64(input.quantity, "backorder quantity")?,
                priority: input.priority.map(|s| parse_backorder_priority(&s)),
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: input.notes,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create backorder: {}", e)))?;
        convert_output(backorder)
    }

    /// Get a backorder by ID
    #[napi]
    pub async fn get_backorder(&self, id: String) -> Result<Option<BackorderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .get_backorder(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get backorder: {}", e)))?;
        convert_optional_output(backorder)
    }

    /// Get a backorder by number
    #[napi]
    pub async fn get_backorder_by_number(&self, number: String) -> Result<Option<BackorderOutput>> {
        let commerce = self.commerce.lock().await;
        let backorder = commerce
            .backorder()
            .get_backorder_by_number(&number)
            .map_err(|e| Error::from_reason(format!("Failed to get backorder: {}", e)))?;
        convert_optional_output(backorder)
    }

    /// List all backorders
    #[napi]
    pub async fn list_backorders(&self) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.lock().await;
        let backorders = commerce
            .backorder()
            .list_backorders(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list backorders: {}", e)))?;
        convert_outputs(backorders)
    }

    /// Cancel a backorder
    #[napi]
    pub async fn cancel_backorder(&self, id: String) -> Result<BackorderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .cancel_backorder(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel backorder: {}", e)))?;
        convert_output(backorder)
    }

    /// Get backorders for an order
    #[napi]
    pub async fn get_backorders_for_order(&self, order_id: String) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = order_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let backorders = commerce
            .backorder()
            .get_backorders_for_order(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get backorders: {}", e)))?;
        convert_outputs(backorders)
    }

    /// Get backorders for a SKU
    #[napi]
    pub async fn get_backorders_for_sku(&self, sku: String) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.lock().await;
        let backorders = commerce
            .backorder()
            .get_backorders_for_sku(&sku)
            .map_err(|e| Error::from_reason(format!("Failed to get backorders: {}", e)))?;
        convert_outputs(backorders)
    }

    /// Get overdue backorders
    #[napi]
    pub async fn get_overdue_backorders(&self) -> Result<Vec<BackorderOutput>> {
        let commerce = self.commerce.lock().await;
        let backorders = commerce
            .backorder()
            .get_overdue_backorders()
            .map_err(|e| Error::from_reason(format!("Failed to get overdue backorders: {}", e)))?;
        convert_outputs(backorders)
    }

    /// Get backorder summary
    #[napi]
    pub async fn get_summary(&self) -> Result<BackorderSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let summary = commerce
            .backorder()
            .get_summary()
            .map_err(|e| Error::from_reason(format!("Failed to get summary: {}", e)))?;
        convert_output(summary)
    }

    /// Count pending backorders
    #[napi]
    pub async fn count_pending(&self) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let count = commerce
            .backorder()
            .count_pending()
            .map_err(|e| Error::from_reason(format!("Failed to count backorders: {}", e)))?;
        Ok(count as u32)
    }
}

// ============================================================================
// General Ledger API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateGlAccountInput {
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub description: Option<String>,
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GlAccountOutput {
    pub id: String,
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub balance: f64,
    pub status: String,
    pub description: Option<String>,
}

impl TryFrom<stateset_core::GlAccount> for GlAccountOutput {
    type Error = Error;

    fn try_from(a: stateset_core::GlAccount) -> Result<Self> {
        Ok(Self {
            id: a.id.to_string(),
            account_number: a.account_number,
            name: a.name,
            account_type: format!("{:?}", a.account_type),
            balance: to_f64_result(a.current_balance, "account balance")?,
            status: format!("{:?}", a.status),
            description: a.description,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct JournalEntryOutput {
    pub id: String,
    pub entry_number: String,
    pub entry_date: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
}

impl From<stateset_core::JournalEntry> for JournalEntryOutput {
    fn from(e: stateset_core::JournalEntry) -> Self {
        Self {
            id: e.id.to_string(),
            entry_number: e.entry_number,
            entry_date: e.entry_date.to_string(),
            description: e.description,
            status: format!("{:?}", e.status),
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TrialBalanceOutput {
    pub as_of_date: String,
    pub total_debits: f64,
    pub total_credits: f64,
    pub is_balanced: bool,
}

impl TryFrom<stateset_core::TrialBalance> for TrialBalanceOutput {
    type Error = Error;

    fn try_from(t: stateset_core::TrialBalance) -> Result<Self> {
        Ok(Self {
            as_of_date: t.as_of_date.to_string(),
            total_debits: to_f64_result(t.total_debits, "trial balance total debits")?,
            total_credits: to_f64_result(t.total_credits, "trial balance total credits")?,
            is_balanced: t.is_balanced,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BalanceSheetOutput {
    pub as_of_date: String,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub total_equity: f64,
}

impl TryFrom<stateset_core::BalanceSheet> for BalanceSheetOutput {
    type Error = Error;

    fn try_from(b: stateset_core::BalanceSheet) -> Result<Self> {
        Ok(Self {
            as_of_date: b.as_of_date.to_string(),
            total_assets: to_f64_result(b.total_assets, "balance sheet total assets")?,
            total_liabilities: to_f64_result(
                b.total_liabilities,
                "balance sheet total liabilities",
            )?,
            total_equity: to_f64_result(b.total_equity, "balance sheet total equity")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IncomeStatementOutput {
    pub period_start: String,
    pub period_end: String,
    pub total_revenue: f64,
    pub total_expenses: f64,
    pub net_income: f64,
}

impl TryFrom<stateset_core::IncomeStatement> for IncomeStatementOutput {
    type Error = Error;

    fn try_from(i: stateset_core::IncomeStatement) -> Result<Self> {
        Ok(Self {
            period_start: i.period_start.to_string(),
            period_end: i.period_end.to_string(),
            total_revenue: to_f64_result(i.total_revenue, "income statement total revenue")?,
            total_expenses: to_f64_result(i.total_expenses, "income statement total expenses")?,
            net_income: to_f64_result(i.net_income, "income statement net income")?,
        })
    }
}

fn parse_account_type(s: &str) -> stateset_core::AccountType {
    match s.to_lowercase().as_str() {
        "asset" => stateset_core::AccountType::Asset,
        "liability" => stateset_core::AccountType::Liability,
        "equity" => stateset_core::AccountType::Equity,
        "revenue" => stateset_core::AccountType::Revenue,
        "expense" => stateset_core::AccountType::Expense,
        _ => stateset_core::AccountType::Asset,
    }
}

#[napi]
pub struct GeneralLedger {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl GeneralLedger {
    /// Create a GL account
    #[napi]
    pub async fn create_account(&self, input: CreateGlAccountInput) -> Result<GlAccountOutput> {
        let commerce = self.commerce.lock().await;
        let account = commerce
            .general_ledger()
            .create_account(stateset_core::CreateGlAccount {
                account_number: input.account_number,
                name: input.name,
                description: input.description,
                account_type: parse_account_type(&input.account_type),
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: Some(true),
                currency: input.currency.and_then(|s| s.parse::<CurrencyCode>().ok()),
            })
            .map_err(|e| Error::from_reason(format!("Failed to create account: {}", e)))?;
        convert_output(account)
    }

    /// Get a GL account by ID
    #[napi]
    pub async fn get_account(&self, id: String) -> Result<Option<GlAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .general_ledger()
            .get_account(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get account: {}", e)))?;
        convert_optional_output(account)
    }

    /// Get a GL account by account number
    #[napi]
    pub async fn get_account_by_number(
        &self,
        account_number: String,
    ) -> Result<Option<GlAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let account = commerce
            .general_ledger()
            .get_account_by_number(&account_number)
            .map_err(|e| Error::from_reason(format!("Failed to get account: {}", e)))?;
        convert_optional_output(account)
    }

    /// List GL accounts
    #[napi]
    pub async fn list_accounts(&self) -> Result<Vec<GlAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let accounts = commerce
            .general_ledger()
            .list_accounts(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list accounts: {}", e)))?;
        convert_outputs(accounts)
    }

    /// Initialize standard chart of accounts
    #[napi]
    pub async fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let accounts = commerce
            .general_ledger()
            .initialize_chart_of_accounts()
            .map_err(|e| Error::from_reason(format!("Failed to initialize chart: {}", e)))?;
        convert_outputs(accounts)
    }

    /// Get a journal entry by ID
    #[napi]
    pub async fn get_journal_entry(&self, id: String) -> Result<Option<JournalEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .get_journal_entry(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get entry: {}", e)))?;
        Ok(entry.map(|e| e.into()))
    }

    /// List journal entries
    #[napi]
    pub async fn list_journal_entries(&self) -> Result<Vec<JournalEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let entries = commerce
            .general_ledger()
            .list_journal_entries(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list entries: {}", e)))?;
        Ok(entries.into_iter().map(|e| e.into()).collect())
    }

    /// Post a journal entry
    #[napi]
    pub async fn post_journal_entry(
        &self,
        id: String,
        posted_by: String,
    ) -> Result<JournalEntryOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .post_journal_entry(uuid, &posted_by)
            .map_err(|e| Error::from_reason(format!("Failed to post entry: {}", e)))?;
        Ok(entry.into())
    }

    /// Void a journal entry
    #[napi]
    pub async fn void_journal_entry(&self, id: String) -> Result<JournalEntryOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .void_journal_entry(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to void entry: {}", e)))?;
        Ok(entry.into())
    }

    /// Get trial balance
    #[napi]
    pub async fn get_trial_balance(&self, as_of_date: String) -> Result<TrialBalanceOutput> {
        let commerce = self.commerce.lock().await;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| Error::from_reason("Invalid date format"))?;
        let balance = commerce
            .general_ledger()
            .get_trial_balance(date)
            .map_err(|e| Error::from_reason(format!("Failed to get trial balance: {}", e)))?;
        convert_output(balance)
    }

    /// Get balance sheet
    #[napi]
    pub async fn get_balance_sheet(&self, as_of_date: String) -> Result<BalanceSheetOutput> {
        let commerce = self.commerce.lock().await;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| Error::from_reason("Invalid date format"))?;
        let sheet = commerce
            .general_ledger()
            .get_balance_sheet(date)
            .map_err(|e| Error::from_reason(format!("Failed to get balance sheet: {}", e)))?;
        convert_output(sheet)
    }

    /// Get income statement
    #[napi]
    pub async fn get_income_statement(
        &self,
        start_date: String,
        end_date: String,
    ) -> Result<IncomeStatementOutput> {
        let commerce = self.commerce.lock().await;
        let start = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
            .map_err(|_| Error::from_reason("Invalid start date format"))?;
        let end = chrono::NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
            .map_err(|_| Error::from_reason("Invalid end date format"))?;
        let statement = commerce
            .general_ledger()
            .get_income_statement(start, end)
            .map_err(|e| Error::from_reason(format!("Failed to get income statement: {}", e)))?;
        convert_output(statement)
    }

    /// Get account balance
    #[napi]
    pub async fn get_account_balance(
        &self,
        account_id: String,
        as_of_date: Option<String>,
    ) -> Result<f64> {
        let commerce = self.commerce.lock().await;
        let uuid = account_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let date = as_of_date.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        let balance = commerce
            .general_ledger()
            .get_account_balance(uuid, date)
            .map_err(|e| Error::from_reason(format!("Failed to get balance: {}", e)))?;
        optional_to_f64_result(balance, "account balance")?
            .ok_or_else(|| Error::from_reason("Account balance unavailable"))
    }
}

// ============================================================================
// x402 Payment Protocol API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreateIntentInput {
    pub payer_address: String,
    pub payee_address: String,
    pub amount: i64,
    pub asset: Option<String>,
    pub network: Option<String>,
    pub nonce: Option<i64>,
    pub validity_seconds: Option<i64>,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
    pub description: Option<String>,
    pub cart_id: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub merchant_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SigningHashInput {
    pub payer_address: String,
    pub payee_address: String,
    pub amount: i64,
    pub asset: String,
    pub network: String,
    pub chain_id: i64,
    pub valid_until: i64,
    pub nonce: i64,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SignatureBundleInput {
    pub ml_dsa_65_signature: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402PublicKeyBundleInput {
    pub ml_dsa_65_public_key: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SignatureBundleOutput {
    pub ml_dsa_65_signature: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402PublicKeyBundleOutput {
    pub ml_dsa_65_public_key: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SignIntentInput {
    pub intent_id: Option<String>,
    pub signature_scheme: Option<String>,
    pub signature: String,
    pub public_key: String,
    pub signature_bundle: Option<X402SignatureBundleInput>,
    pub public_key_bundle: Option<X402PublicKeyBundleInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct X402IntentFilterInput {
    pub payer_address: Option<String>,
    pub payee_address: Option<String>,
    pub status: Option<String>,
    pub network: Option<String>,
    pub asset: Option<String>,
    pub order_id: Option<String>,
    pub batch_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402IntentOutput {
    pub id: String,
    pub version: String,
    pub status: String,
    pub payer_address: String,
    pub payee_address: String,
    pub amount: i64,
    pub amount_decimal: f64,
    pub asset: String,
    pub network: String,
    pub chain_id: i64,
    pub token_address: Option<String>,
    pub created_at_unix: i64,
    pub valid_until: i64,
    pub nonce: i64,
    pub idempotency_key: Option<String>,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
    pub description: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub merchant_id: Option<String>,
    pub signing_hash: Option<String>,
    pub payer_signature_scheme: Option<String>,
    pub payer_signature: Option<String>,
    pub payer_public_key: Option<String>,
    pub payer_signature_bundle: Option<X402SignatureBundleOutput>,
    pub payer_public_key_bundle: Option<X402PublicKeyBundleOutput>,
    pub sequence_number: Option<i64>,
    pub sequenced_at: Option<String>,
    pub batch_id: Option<String>,
    pub batch_merkle_root: Option<String>,
    pub inclusion_proof: Option<Vec<String>>,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub gas_used: Option<i64>,
    pub settled_at: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::X402PaymentIntent> for X402IntentOutput {
    fn from(intent: stateset_core::X402PaymentIntent) -> Self {
        Self {
            id: intent.id.to_string(),
            version: intent.version,
            status: intent.status.to_string(),
            payer_address: intent.payer_address,
            payee_address: intent.payee_address,
            amount: intent.amount as i64,
            amount_decimal: to_f64_or_nan(intent.amount_decimal),
            asset: intent.asset.to_string().to_lowercase(),
            network: intent.network.to_string(),
            chain_id: intent.chain_id as i64,
            token_address: intent.token_address,
            created_at_unix: intent.created_at_unix as i64,
            valid_until: intent.valid_until as i64,
            nonce: intent.nonce as i64,
            idempotency_key: intent.idempotency_key,
            resource_uri: intent.resource_uri,
            resource_method: intent.resource_method,
            description: intent.description,
            order_id: intent.order_id.map(|id| id.to_string()),
            invoice_id: intent.invoice_id.map(|id| id.to_string()),
            merchant_id: intent.merchant_id,
            signing_hash: intent.signing_hash,
            payer_signature_scheme: intent.payer_signature_scheme.map(|scheme| scheme.to_string()),
            payer_signature: intent.payer_signature,
            payer_public_key: intent.payer_public_key,
            payer_signature_bundle: intent.payer_signature_bundle.map(|bundle| {
                X402SignatureBundleOutput {
                    ml_dsa_65_signature: Buffer::from(bundle.ml_dsa_65_signature.as_slice()),
                }
            }),
            payer_public_key_bundle: intent.payer_public_key_bundle.map(|bundle| {
                X402PublicKeyBundleOutput {
                    ml_dsa_65_public_key: Buffer::from(bundle.ml_dsa_65_public_key.as_slice()),
                }
            }),
            sequence_number: intent.sequence_number.map(|n| n as i64),
            sequenced_at: intent.sequenced_at.map(|d| d.to_rfc3339()),
            batch_id: intent.batch_id.map(|id| id.to_string()),
            batch_merkle_root: intent.batch_merkle_root,
            inclusion_proof: intent.inclusion_proof,
            tx_hash: intent.tx_hash,
            block_number: intent.block_number.map(|n| n as i64),
            gas_used: intent.gas_used.map(|n| n as i64),
            settled_at: intent.settled_at.map(|d| d.to_rfc3339()),
            metadata: intent.metadata,
            created_at: intent.created_at.to_rfc3339(),
            updated_at: intent.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402AgentCardInput {
    pub name: String,
    pub description: Option<String>,
    pub wallet_address: String,
    pub public_key: String,
    pub supported_networks: Option<Vec<String>>,
    pub supported_assets: Option<Vec<String>>,
    pub a2a_skills: Option<Vec<String>>,
    pub trust_level: Option<String>,
    pub endpoint_url: Option<String>,
    pub endpoint_protocol: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
    pub business_category: Option<String>,
    pub max_transaction_amount: Option<i64>,
    pub daily_volume_limit: Option<i64>,
    pub requires_kyc: Option<bool>,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct X402AgentCardFilterInput {
    pub wallet_address: Option<String>,
    pub trust_level: Option<String>,
    pub min_trust_level: Option<String>,
    pub network: Option<String>,
    pub asset: Option<String>,
    pub skill: Option<String>,
    pub active: Option<bool>,
    pub merchant_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402AgentCardOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub wallet_address: String,
    pub public_key: String,
    pub supported_networks: Vec<String>,
    pub supported_assets: Vec<String>,
    pub a2a_skills: Vec<String>,
    pub trust_level: String,
    pub verified_at: Option<String>,
    pub verification_method: Option<String>,
    pub endpoint_url: Option<String>,
    pub endpoint_protocol: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
    pub business_category: Option<String>,
    pub max_transaction_amount: Option<i64>,
    pub daily_volume_limit: Option<i64>,
    pub requires_kyc: bool,
    pub active: bool,
    pub suspended_at: Option<String>,
    pub suspension_reason: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::AgentCard> for X402AgentCardOutput {
    fn from(card: stateset_core::AgentCard) -> Self {
        Self {
            id: card.id.to_string(),
            name: card.name,
            description: card.description,
            wallet_address: card.wallet_address,
            public_key: card.public_key,
            supported_networks: card
                .supported_networks
                .into_iter()
                .map(|n| n.to_string())
                .collect(),
            supported_assets: card
                .supported_assets
                .into_iter()
                .map(|a| a.to_string().to_lowercase())
                .collect(),
            a2a_skills: card.a2a_skills.into_iter().map(|s| s.to_string()).collect(),
            trust_level: card.trust_level.to_string(),
            verified_at: card.verified_at.map(|d| d.to_rfc3339()),
            verification_method: card.verification_method,
            endpoint_url: card.endpoint_url,
            endpoint_protocol: card.endpoint_protocol,
            merchant_id: card.merchant_id,
            merchant_name: card.merchant_name,
            business_category: card.business_category,
            max_transaction_amount: card.max_transaction_amount.map(|v| v as i64),
            daily_volume_limit: card.daily_volume_limit.map(|v| v as i64),
            requires_kyc: card.requires_kyc,
            active: card.active,
            suspended_at: card.suspended_at.map(|d| d.to_rfc3339()),
            suspension_reason: card.suspension_reason,
            metadata: card.metadata,
            created_at: card.created_at.to_rfc3339(),
            updated_at: card.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditBalanceInput {
    pub payer_address: String,
    pub asset: Option<String>,
    pub network: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditAdjustmentInput {
    pub payer_address: String,
    pub asset: Option<String>,
    pub network: Option<String>,
    pub amount: i64,
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct X402CreditTransactionFilterInput {
    pub payer_address: Option<String>,
    pub asset: Option<String>,
    pub network: Option<String>,
    pub direction: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditAccountOutput {
    pub id: String,
    pub payer_address: String,
    pub asset: String,
    pub network: String,
    pub balance: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::X402CreditAccount> for X402CreditAccountOutput {
    fn from(account: stateset_core::X402CreditAccount) -> Self {
        Self {
            id: account.id.to_string(),
            payer_address: account.payer_address,
            asset: account.asset.to_string().to_lowercase(),
            network: account.network.to_string(),
            balance: account.balance as i64,
            created_at: account.created_at.to_rfc3339(),
            updated_at: account.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditTransactionOutput {
    pub id: String,
    pub account_id: String,
    pub payer_address: String,
    pub asset: String,
    pub network: String,
    pub direction: String,
    pub amount: i64,
    pub balance_after: i64,
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::X402CreditTransaction> for X402CreditTransactionOutput {
    fn from(txn: stateset_core::X402CreditTransaction) -> Self {
        Self {
            id: txn.id.to_string(),
            account_id: txn.account_id.to_string(),
            payer_address: txn.payer_address,
            asset: txn.asset.to_string().to_lowercase(),
            network: txn.network.to_string(),
            direction: txn.direction.to_string(),
            amount: txn.amount as i64,
            balance_after: txn.balance_after as i64,
            reason: txn.reason,
            reference_id: txn.reference_id,
            metadata: txn.metadata,
            created_at: txn.created_at.to_rfc3339(),
        }
    }
}

fn parse_x402_asset(s: &str) -> Result<stateset_core::X402Asset> {
    s.parse::<stateset_core::X402Asset>()
        .map_err(|e| Error::from_reason(format!("Invalid x402 asset: {}", e)))
}

fn parse_x402_network(s: &str) -> Result<stateset_core::X402Network> {
    s.parse::<stateset_core::X402Network>()
        .map_err(|e| Error::from_reason(format!("Invalid x402 network: {}", e)))
}

fn parse_x402_status(s: &str) -> Result<stateset_core::X402IntentStatus> {
    s.parse::<stateset_core::X402IntentStatus>()
        .map_err(|e| Error::from_reason(format!("Invalid x402 status: {}", e)))
}

fn parse_x402_signature_scheme(s: &str) -> Result<stateset_core::X402SignatureScheme> {
    s.parse::<stateset_core::X402SignatureScheme>()
        .map_err(|e| Error::from_reason(format!("Invalid x402 signature scheme: {}", e)))
}

fn parse_trust_level(s: &str) -> Result<stateset_core::TrustLevel> {
    s.parse::<stateset_core::TrustLevel>()
        .map_err(|e| Error::from_reason(format!("Invalid trust level: {}", e)))
}

fn parse_a2a_skill(s: &str) -> Result<stateset_core::A2ASkill> {
    s.parse::<stateset_core::A2ASkill>()
        .map_err(|e| Error::from_reason(format!("Invalid A2A skill: {}", e)))
}

fn parse_credit_direction(s: &str) -> Result<stateset_core::X402CreditDirection> {
    s.parse::<stateset_core::X402CreditDirection>()
        .map_err(|e| Error::from_reason(format!("Invalid credit direction: {}", e)))
}

fn parse_uuid_opt(value: Option<String>) -> Result<Option<uuid::Uuid>> {
    match value {
        Some(id) => Ok(Some(id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?)),
        None => Ok(None),
    }
}

fn parse_amount(value: i64) -> Result<u64> {
    if value < 0 {
        return Err(Error::from_reason("Amount must be >= 0"));
    }
    Ok(value as u64)
}

fn parse_u64_field(field: &str, value: i64) -> Result<u64> {
    if value < 0 {
        return Err(Error::from_reason(format!("{} must be >= 0", field)));
    }
    Ok(value as u64)
}

fn parse_u64_opt(field: &str, value: Option<i64>) -> Result<Option<u64>> {
    match value {
        Some(val) => Ok(Some(parse_u64_field(field, val)?)),
        None => Ok(None),
    }
}

/// Compute the sequencer-compatible x402 signing hash for a payment intent shape.
#[napi]
pub fn ves_x402_compute_signing_hash(input: X402SigningHashInput) -> Result<Buffer> {
    use sha2::{Digest, Sha256};

    let amount = parse_amount(input.amount)?;
    let chain_id = parse_u64_field("chain_id", input.chain_id)?;
    let valid_until = parse_u64_field("valid_until", input.valid_until)?;
    let nonce = parse_u64_field("nonce", input.nonce)?;
    let asset = parse_x402_asset(&input.asset)?;
    let network = parse_x402_network(&input.network)?;

    let mut hasher = Sha256::new();
    hasher.update(stateset_core::X402_DOMAIN_SEPARATOR.as_bytes());
    hasher.update(input.payer_address.as_bytes());
    hasher.update(input.payee_address.as_bytes());
    hasher.update(amount.to_be_bytes());
    hasher.update(format!("{:?}", asset).to_lowercase().as_bytes());
    hasher.update(network.to_string().as_bytes());
    hasher.update(chain_id.to_be_bytes());
    hasher.update(valid_until.to_be_bytes());
    hasher.update(nonce.to_be_bytes());

    match input.resource_uri {
        Some(uri) => {
            hasher.update([1u8]);
            hasher.update((uri.len() as u64).to_be_bytes());
            hasher.update(uri.as_bytes());
        }
        None => hasher.update([0u8]),
    }

    match input.resource_method {
        Some(method) => {
            hasher.update([1u8]);
            hasher.update((method.len() as u64).to_be_bytes());
            hasher.update(method.as_bytes());
        }
        None => hasher.update([0u8]),
    }

    let result: [u8; 32] = hasher.finalize().into();
    Ok(Buffer::from(result.as_slice()))
}

#[napi]
pub struct X402 {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl X402 {
    #[napi]
    pub async fn create_intent(&self, input: X402CreateIntentInput) -> Result<X402IntentOutput> {
        let commerce = self.commerce.lock().await;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let amount = parse_amount(input.amount)?;

        let intent = commerce
            .x402()
            .create_intent(stateset_core::CreateX402PaymentIntent {
                payer_address: input.payer_address,
                payee_address: input.payee_address,
                amount,
                asset,
                network,
                nonce: parse_u64_opt("nonce", input.nonce)?,
                validity_seconds: parse_u64_opt("validity_seconds", input.validity_seconds)?,
                resource_uri: input.resource_uri,
                resource_method: input.resource_method,
                description: input.description,
                cart_id: parse_uuid_opt(input.cart_id)?,
                order_id: parse_uuid_opt(input.order_id)?,
                invoice_id: parse_uuid_opt(input.invoice_id)?,
                merchant_id: input.merchant_id,
                idempotency_key: input.idempotency_key,
                metadata: input.metadata,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create x402 intent: {}", e)))?;

        Ok(intent.into())
    }

    #[napi]
    pub async fn sign_intent(
        &self,
        intent_id: String,
        input: X402SignIntentInput,
    ) -> Result<X402IntentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = intent_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let signed = commerce
            .x402()
            .sign_intent(
                uuid,
                stateset_core::SignX402PaymentIntent {
                    intent_id: uuid,
                    signature_scheme: input
                        .signature_scheme
                        .as_deref()
                        .map(parse_x402_signature_scheme)
                        .transpose()?,
                    signature: input.signature,
                    public_key: input.public_key,
                    signature_bundle: input.signature_bundle.map(|bundle| {
                        stateset_core::X402SignatureBundle {
                            ml_dsa_65_signature: bundle.ml_dsa_65_signature.as_ref().to_vec(),
                        }
                    }),
                    public_key_bundle: input.public_key_bundle.map(|bundle| {
                        stateset_core::X402PublicKeyBundle {
                            ml_dsa_65_public_key: bundle.ml_dsa_65_public_key.as_ref().to_vec(),
                        }
                    }),
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to sign x402 intent: {}", e)))?;

        Ok(signed.into())
    }

    #[napi]
    pub async fn get_intent(&self, id: String) -> Result<Option<X402IntentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let intent = commerce
            .x402()
            .get_intent(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get x402 intent: {}", e)))?;
        Ok(intent.map(|i| i.into()))
    }

    #[napi]
    pub async fn list_intents(
        &self,
        filter: X402IntentFilterInput,
    ) -> Result<Vec<X402IntentOutput>> {
        let commerce = self.commerce.lock().await;
        let intents = commerce
            .x402()
            .list_intents(stateset_core::X402PaymentIntentFilter {
                payer_address: filter.payer_address,
                payee_address: filter.payee_address,
                status: match filter.status {
                    Some(val) => Some(parse_x402_status(&val)?),
                    None => None,
                },
                network: match filter.network {
                    Some(val) => Some(parse_x402_network(&val)?),
                    None => None,
                },
                asset: match filter.asset {
                    Some(val) => Some(parse_x402_asset(&val)?),
                    None => None,
                },
                order_id: parse_uuid_opt(filter.order_id)?,
                batch_id: parse_uuid_opt(filter.batch_id)?,
                limit: filter.limit,
                offset: filter.offset,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to list x402 intents: {}", e)))?;

        Ok(intents.into_iter().map(|i| i.into()).collect())
    }

    #[napi]
    pub async fn mark_settled(
        &self,
        intent_id: String,
        tx_hash: String,
        block_number: i64,
    ) -> Result<X402IntentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = intent_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let block_number = parse_u64_field("block_number", block_number)?;
        let intent = commerce
            .x402()
            .mark_settled(uuid, &tx_hash, block_number)
            .map_err(|e| Error::from_reason(format!("Failed to mark settled: {}", e)))?;
        Ok(intent.into())
    }

    #[napi]
    pub async fn get_next_nonce(&self, payer_address: String) -> Result<i64> {
        let commerce = self.commerce.lock().await;
        let nonce = commerce
            .x402()
            .get_next_nonce(&payer_address)
            .map_err(|e| Error::from_reason(format!("Failed to get nonce: {}", e)))?;
        i64::try_from(nonce).map_err(|_| Error::from_reason("Nonce too large to fit in i64"))
    }

    #[napi]
    pub async fn register_agent(&self, input: X402AgentCardInput) -> Result<X402AgentCardOutput> {
        let commerce = self.commerce.lock().await;
        let supported_networks = match input.supported_networks {
            Some(list) => {
                let mut parsed = Vec::with_capacity(list.len());
                for item in list {
                    parsed.push(parse_x402_network(&item)?);
                }
                Some(parsed)
            }
            None => None,
        };
        let supported_assets = match input.supported_assets {
            Some(list) => {
                let mut parsed = Vec::with_capacity(list.len());
                for item in list {
                    parsed.push(parse_x402_asset(&item)?);
                }
                Some(parsed)
            }
            None => None,
        };
        let a2a_skills = match input.a2a_skills {
            Some(list) => {
                let mut parsed = Vec::with_capacity(list.len());
                for item in list {
                    parsed.push(parse_a2a_skill(&item)?);
                }
                Some(parsed)
            }
            None => None,
        };
        let trust_level = match input.trust_level {
            Some(val) => Some(parse_trust_level(&val)?),
            None => None,
        };
        let max_transaction_amount = match input.max_transaction_amount {
            Some(val) => Some(parse_amount(val)?),
            None => None,
        };
        let daily_volume_limit = match input.daily_volume_limit {
            Some(val) => Some(parse_amount(val)?),
            None => None,
        };

        let card = commerce
            .x402()
            .register_agent(stateset_core::CreateAgentCard {
                name: input.name,
                description: input.description,
                wallet_address: input.wallet_address,
                public_key: input.public_key,
                supported_networks,
                supported_assets,
                a2a_skills,
                trust_level,
                endpoint_url: input.endpoint_url,
                endpoint_protocol: input.endpoint_protocol,
                merchant_id: input.merchant_id,
                merchant_name: input.merchant_name,
                business_category: input.business_category,
                max_transaction_amount,
                daily_volume_limit,
                requires_kyc: input.requires_kyc,
                metadata: input.metadata,
            })
            .map_err(|e| Error::from_reason(format!("Failed to register agent: {}", e)))?;

        Ok(card.into())
    }

    #[napi]
    pub async fn discover_agents(
        &self,
        network: Option<String>,
        asset: Option<String>,
        skill: Option<String>,
        trust_level: Option<String>,
    ) -> Result<Vec<X402AgentCardOutput>> {
        let commerce = self.commerce.lock().await;
        let network = match network {
            Some(val) => Some(parse_x402_network(&val)?),
            None => None,
        };
        let asset = match asset {
            Some(val) => Some(parse_x402_asset(&val)?),
            None => None,
        };
        let skill = match skill {
            Some(val) => Some(parse_a2a_skill(&val)?),
            None => None,
        };
        let trust_level = match trust_level {
            Some(val) => Some(parse_trust_level(&val)?),
            None => None,
        };

        let agents = commerce
            .x402()
            .discover_agents(network, asset, skill, trust_level)
            .map_err(|e| Error::from_reason(format!("Failed to discover agents: {}", e)))?;

        Ok(agents.into_iter().map(|a| a.into()).collect())
    }

    #[napi]
    pub async fn get_agent(&self, id: String) -> Result<Option<X402AgentCardOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let agent = commerce
            .x402()
            .get_agent(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get agent: {}", e)))?;
        Ok(agent.map(|a| a.into()))
    }

    #[napi]
    pub async fn get_agent_by_wallet(
        &self,
        wallet_address: String,
    ) -> Result<Option<X402AgentCardOutput>> {
        let commerce = self.commerce.lock().await;
        let agent = commerce
            .x402()
            .get_agent_by_wallet(&wallet_address)
            .map_err(|e| Error::from_reason(format!("Failed to get agent: {}", e)))?;
        Ok(agent.map(|a| a.into()))
    }

    #[napi]
    pub async fn verify_agent(&self, id: String) -> Result<X402AgentCardOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let agent = commerce
            .x402()
            .verify_agent(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to verify agent: {}", e)))?;
        Ok(agent.into())
    }

    #[napi]
    pub async fn list_agents(
        &self,
        filter: X402AgentCardFilterInput,
    ) -> Result<Vec<X402AgentCardOutput>> {
        let commerce = self.commerce.lock().await;
        let agents = commerce
            .x402()
            .list_agents(stateset_core::AgentCardFilter {
                wallet_address: filter.wallet_address,
                trust_level: match filter.trust_level {
                    Some(val) => Some(parse_trust_level(&val)?),
                    None => None,
                },
                min_trust_level: match filter.min_trust_level {
                    Some(val) => Some(parse_trust_level(&val)?),
                    None => None,
                },
                network: match filter.network {
                    Some(val) => Some(parse_x402_network(&val)?),
                    None => None,
                },
                asset: match filter.asset {
                    Some(val) => Some(parse_x402_asset(&val)?),
                    None => None,
                },
                skill: match filter.skill {
                    Some(val) => Some(parse_a2a_skill(&val)?),
                    None => None,
                },
                active: filter.active,
                merchant_id: filter.merchant_id,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list agents: {}", e)))?;

        Ok(agents.into_iter().map(|a| a.into()).collect())
    }

    #[napi]
    pub async fn get_credit_balance(&self, input: X402CreditBalanceInput) -> Result<i64> {
        let commerce = self.commerce.lock().await;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let balance = commerce
            .x402()
            .get_credit_balance(&input.payer_address, asset, network)
            .map_err(|e| Error::from_reason(format!("Failed to get credit balance: {}", e)))?;
        Ok(balance as i64)
    }

    #[napi]
    pub async fn get_credit_account(
        &self,
        input: X402CreditBalanceInput,
    ) -> Result<Option<X402CreditAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let account = commerce
            .x402()
            .get_credit_account(&input.payer_address, asset, network)
            .map_err(|e| Error::from_reason(format!("Failed to get credit account: {}", e)))?;
        Ok(account.map(|a| a.into()))
    }

    #[napi]
    pub async fn credit_account(
        &self,
        input: X402CreditAdjustmentInput,
    ) -> Result<X402CreditTransactionOutput> {
        let commerce = self.commerce.lock().await;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let amount = parse_amount(input.amount)?;
        let txn = commerce
            .x402()
            .credit_account(
                &input.payer_address,
                asset,
                network,
                amount,
                input.reason,
                input.reference_id,
                input.metadata,
            )
            .map_err(|e| Error::from_reason(format!("Failed to credit account: {}", e)))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn debit_account(
        &self,
        input: X402CreditAdjustmentInput,
    ) -> Result<X402CreditTransactionOutput> {
        let commerce = self.commerce.lock().await;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let amount = parse_amount(input.amount)?;
        let txn = commerce
            .x402()
            .debit_account(
                &input.payer_address,
                asset,
                network,
                amount,
                input.reason,
                input.reference_id,
                input.metadata,
            )
            .map_err(|e| Error::from_reason(format!("Failed to debit account: {}", e)))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn list_credit_transactions(
        &self,
        filter: X402CreditTransactionFilterInput,
    ) -> Result<Vec<X402CreditTransactionOutput>> {
        let commerce = self.commerce.lock().await;
        let transactions = commerce
            .x402()
            .list_credit_transactions(stateset_core::X402CreditTransactionFilter {
                payer_address: filter.payer_address,
                asset: match filter.asset {
                    Some(val) => Some(parse_x402_asset(&val)?),
                    None => None,
                },
                network: match filter.network {
                    Some(val) => Some(parse_x402_network(&val)?),
                    None => None,
                },
                direction: match filter.direction {
                    Some(val) => Some(parse_credit_direction(&val)?),
                    None => None,
                },
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to list credit transactions: {}", e))
            })?;

        Ok(transactions.into_iter().map(|t| t.into()).collect())
    }
}

// ============================================================================
// Vector Search API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct VectorSearchResultOutput {
    pub id: String,
    pub name: String,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ProductSearchResultOutput {
    pub product: ProductOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct CustomerSearchResultOutput {
    pub customer: CustomerOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct OrderSearchResultOutput {
    pub order: OrderOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct InventorySearchResultOutput {
    pub item: InventoryItemOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct EmbeddingStatsOutput {
    pub product_count: u32,
    pub customer_count: u32,
    pub order_count: u32,
    pub inventory_count: u32,
    pub model: String,
    pub dimensions: u32,
}

/// Vector search operations for semantic similarity search
#[napi]
pub struct VectorSearch {
    commerce: Arc<Mutex<RustCommerce>>,
    api_key: String,
}

#[napi]
impl VectorSearch {
    /// Search products using natural language query
    #[napi]
    pub async fn search_products(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<ProductSearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let results = vector
            .search_products(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| Error::from_reason(format!("Failed to search products: {}", e)))?;

        Ok(results
            .into_iter()
            .map(|r| ProductSearchResultOutput {
                product: r.entity.into(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Search customers using natural language query
    #[napi]
    pub async fn search_customers(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<CustomerSearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let results = vector
            .search_customers(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| Error::from_reason(format!("Failed to search customers: {}", e)))?;

        Ok(results
            .into_iter()
            .map(|r| CustomerSearchResultOutput {
                customer: r.entity.into(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Search orders using natural language query
    #[napi]
    pub async fn search_orders(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<OrderSearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let results = vector
            .search_orders(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| Error::from_reason(format!("Failed to search orders: {}", e)))?;

        results
            .into_iter()
            .map(|r| {
                Ok(OrderSearchResultOutput {
                    order: convert_output(r.entity)?,
                    distance: r.distance as f64,
                    score: r.score as f64,
                })
            })
            .collect()
    }

    /// Search inventory items using natural language query
    #[napi]
    pub async fn search_inventory(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<InventorySearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let results = vector
            .search_inventory(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| Error::from_reason(format!("Failed to search inventory: {}", e)))?;

        Ok(results
            .into_iter()
            .map(|r| InventorySearchResultOutput {
                item: r.entity.into(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Index a product for vector search
    #[napi]
    pub async fn index_product(&self, product_id: String) -> Result<()> {
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let (product, vector) = {
            let commerce = self.commerce.lock().await;
            let product = commerce
                .products()
                .get(uuid.into())
                .map_err(|e| Error::from_reason(format!("Failed to get product: {}", e)))?
                .ok_or_else(|| Error::from_reason("Product not found"))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (product, vector)
        };

        vector
            .index_product(&product)
            .map_err(|e| Error::from_reason(format!("Failed to index product: {}", e)))?;

        Ok(())
    }

    /// Index a customer for vector search
    #[napi]
    pub async fn index_customer(&self, customer_id: String) -> Result<()> {
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let (customer, vector) = {
            let commerce = self.commerce.lock().await;
            let customer = commerce
                .customers()
                .get(uuid.into())
                .map_err(|e| Error::from_reason(format!("Failed to get customer: {}", e)))?
                .ok_or_else(|| Error::from_reason("Customer not found"))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (customer, vector)
        };

        vector
            .index_customer(&customer)
            .map_err(|e| Error::from_reason(format!("Failed to index customer: {}", e)))?;

        Ok(())
    }

    /// Index an order for vector search
    #[napi]
    pub async fn index_order(&self, order_id: String) -> Result<()> {
        let uuid: uuid::Uuid = order_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;

        let (order, vector) = {
            let commerce = self.commerce.lock().await;
            let order = commerce
                .orders()
                .get(uuid.into())
                .map_err(|e| Error::from_reason(format!("Failed to get order: {}", e)))?
                .ok_or_else(|| Error::from_reason("Order not found"))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (order, vector)
        };

        vector
            .index_order(&order)
            .map_err(|e| Error::from_reason(format!("Failed to index order: {}", e)))?;

        Ok(())
    }

    /// Index an inventory item for vector search
    #[napi]
    pub async fn index_inventory_item(&self, item_id: String) -> Result<()> {
        let item_id =
            item_id.parse::<i64>().map_err(|_| Error::from_reason("Invalid inventory item ID"))?;

        let (item, vector) = {
            let commerce = self.commerce.lock().await;
            let item = commerce
                .inventory()
                .get_item(item_id)
                .map_err(|e| Error::from_reason(format!("Failed to get inventory item: {}", e)))?
                .ok_or_else(|| Error::from_reason("Inventory item not found"))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (item, vector)
        };

        vector
            .index_inventory_item(&item)
            .map_err(|e| Error::from_reason(format!("Failed to index inventory item: {}", e)))?;

        Ok(())
    }

    /// Index all products for vector search
    #[napi]
    pub async fn index_all_products(&self) -> Result<u32> {
        let (products, vector) = {
            let commerce = self.commerce.lock().await;
            let products = commerce
                .products()
                .list(Default::default())
                .map_err(|e| Error::from_reason(format!("Failed to list products: {}", e)))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (products, vector)
        };

        let count = vector
            .index_products(&products)
            .map_err(|e| Error::from_reason(format!("Failed to index products: {}", e)))?;

        Ok(count as u32)
    }

    /// Index all customers for vector search
    #[napi]
    pub async fn index_all_customers(&self) -> Result<u32> {
        let (customers, vector) = {
            let commerce = self.commerce.lock().await;
            let customers = commerce
                .customers()
                .list(Default::default())
                .map_err(|e| Error::from_reason(format!("Failed to list customers: {}", e)))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (customers, vector)
        };

        let count = vector
            .index_customers(&customers)
            .map_err(|e| Error::from_reason(format!("Failed to index customers: {}", e)))?;

        Ok(count as u32)
    }

    /// Index all orders for vector search
    #[napi]
    pub async fn index_all_orders(&self) -> Result<u32> {
        let (orders, vector) = {
            let commerce = self.commerce.lock().await;
            let orders = commerce
                .orders()
                .list(Default::default())
                .map_err(|e| Error::from_reason(format!("Failed to list orders: {}", e)))?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (orders, vector)
        };

        let count = vector
            .index_orders(&orders)
            .map_err(|e| Error::from_reason(format!("Failed to index orders: {}", e)))?;

        Ok(count as u32)
    }

    /// Index all inventory items for vector search
    #[napi]
    pub async fn index_all_inventory(&self) -> Result<u32> {
        let (items, vector) = {
            let commerce = self.commerce.lock().await;
            let items = commerce.inventory().list(Default::default()).map_err(|e| {
                Error::from_reason(format!("Failed to list inventory items: {}", e))
            })?;

            let vector = commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?;

            (items, vector)
        };

        let count = vector
            .index_inventory_items(&items)
            .map_err(|e| Error::from_reason(format!("Failed to index inventory items: {}", e)))?;

        Ok(count as u32)
    }

    /// Get embedding statistics
    #[napi]
    pub async fn stats(&self) -> Result<EmbeddingStatsOutput> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let stats = vector
            .stats()
            .map_err(|e| Error::from_reason(format!("Failed to get stats: {}", e)))?;

        Ok(EmbeddingStatsOutput {
            product_count: *stats.counts.get(&stateset_core::EntityType::Product).unwrap_or(&0)
                as u32,
            customer_count: *stats.counts.get(&stateset_core::EntityType::Customer).unwrap_or(&0)
                as u32,
            order_count: *stats.counts.get(&stateset_core::EntityType::Order).unwrap_or(&0) as u32,
            inventory_count: *stats
                .counts
                .get(&stateset_core::EntityType::InventoryItem)
                .unwrap_or(&0) as u32,
            model: stats.model,
            dimensions: stats.dimensions as u32,
        })
    }

    /// Clear all embeddings for a specific entity type
    #[napi]
    pub async fn clear(&self, entity_type: String) -> Result<u32> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let et: stateset_core::EntityType =
            entity_type.parse().map_err(|e: String| Error::from_reason(e))?;

        let count = vector
            .clear(et)
            .map_err(|e| Error::from_reason(format!("Failed to clear embeddings: {}", e)))?;

        Ok(count as u32)
    }

    /// Clear all embeddings
    #[napi]
    pub async fn clear_all(&self) -> Result<u32> {
        let vector = {
            let commerce = self.commerce.lock().await;
            commerce.vector(self.api_key.clone()).map_err(|e| {
                Error::from_reason(format!("Failed to initialize vector search: {}", e))
            })?
        };

        let count = vector
            .clear_all()
            .map_err(|e| Error::from_reason(format!("Failed to clear all embeddings: {}", e)))?;

        Ok(count as u32)
    }
}

// =============================================================================
// VES v1.0 Cryptographic Operations (stateset-crypto)
// =============================================================================

#[napi(object)]
pub struct HybridSigningKeypairOutput {
    pub ed25519_public_key: Buffer,
    pub ed25519_private_key: Buffer,
    pub ml_dsa_65_public_key: Buffer,
    pub ml_dsa_65_seed: Buffer,
}

#[napi(object)]
pub struct HybridSignatureBundleOutput {
    pub ed25519_signature: Buffer,
    pub ml_dsa_65_signature: Buffer,
}

#[napi(object)]
pub struct HybridRecipientKeypairOutput {
    pub kid: u32,
    pub x25519_public_key: Buffer,
    pub x25519_private_key: Buffer,
    pub ml_kem_768_public_key: Buffer,
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct HybridPayloadAadParamsInput {
    pub ves_version: u32,
    pub tenant_id: String,
    pub store_id: String,
    pub event_id: String,
    pub source_agent_id: String,
    pub agent_key_id: u32,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub created_at: String,
    pub payload_plain_hash: Buffer,
}

#[napi(object)]
pub struct HybridRecipientPublicKeyInput {
    pub kid: u32,
    pub x25519_public_key: Buffer,
    pub ml_kem_768_public_key: Buffer,
}

#[napi(object)]
pub struct HybridRecipientPrivateKeyInput {
    pub x25519_private_key: Buffer,
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct HybridEncryptionResultOutput {
    pub payload_encrypted_json: String,
    pub salt: Buffer,
    pub payload_plain_hash: Buffer,
    pub payload_cipher_hash: Buffer,
}

/// Canonicalize a JSON string per RFC 8785 JCS
#[napi]
pub fn jcs_canonicalize(json_str: String) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| Error::from_reason(format!("Invalid JSON: {}", e)))?;
    stateset_crypto::canonicalize::canonicalize_json(&value)
        .map_err(|e| Error::from_reason(format!("JCS error: {}", e)))
}

/// Compute domain-separated SHA-256 hash
///
/// domain: one of "PAYLOAD_PLAIN", "PAYLOAD_AAD", "PAYLOAD_CIPHER", "RECIPIENTS",
///         "EVENTSIG", "LEAF", "NODE", "PAD_LEAF", "STREAM", "RECEIPT"
/// data: hex-encoded data to hash (after the domain prefix)
#[napi]
pub fn domain_hash(domain: String, data: Buffer) -> Result<Buffer> {
    use sha2::{Digest, Sha256};

    let prefix: &[u8] = match domain.as_str() {
        "PAYLOAD_PLAIN" => stateset_crypto::domain::PAYLOAD_PLAIN,
        "PAYLOAD_AAD" => stateset_crypto::domain::PAYLOAD_AAD,
        "PAYLOAD_CIPHER" => stateset_crypto::domain::PAYLOAD_CIPHER,
        "RECIPIENTS" => stateset_crypto::domain::RECIPIENTS,
        "EVENTSIG" => stateset_crypto::domain::EVENTSIG,
        "LEAF" => stateset_crypto::domain::LEAF,
        "NODE" => stateset_crypto::domain::NODE,
        "PAD_LEAF" => stateset_crypto::domain::PAD_LEAF,
        "STREAM" => stateset_crypto::domain::STREAM,
        "RECEIPT" => stateset_crypto::domain::RECEIPT,
        _ => return Err(Error::from_reason(format!("Unknown domain: {}", domain))),
    };

    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(data.as_ref());
    let result: [u8; 32] = hasher.finalize().into();
    Ok(Buffer::from(result.as_slice()))
}

/// Sign a 32-byte hash with Ed25519
///
/// Returns 64-byte signature
#[napi]
pub fn ed25519_sign(hash: Buffer, private_key: Buffer) -> Result<Buffer> {
    if hash.len() != 32 {
        return Err(Error::from_reason("Hash must be 32 bytes"));
    }
    if private_key.len() != 32 {
        return Err(Error::from_reason("Private key must be 32 bytes"));
    }
    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(hash.as_ref());
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(private_key.as_ref());

    let sig = stateset_crypto::sign::sign_event_hash(&hash_arr, &key_arr)
        .map_err(|e| Error::from_reason(format!("Sign error: {}", e)))?;
    Ok(Buffer::from(sig.as_slice()))
}

/// Verify an Ed25519 signature
///
/// Returns true if signature is valid
#[napi]
pub fn ed25519_verify(hash: Buffer, signature: Buffer, public_key: Buffer) -> Result<bool> {
    if hash.len() != 32 || signature.len() != 64 || public_key.len() != 32 {
        return Ok(false);
    }
    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(hash.as_ref());
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(signature.as_ref());
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(public_key.as_ref());

    Ok(stateset_crypto::sign::verify_event_signature(&hash_arr, &sig_arr, &key_arr))
}

/// Generate a hybrid `Ed25519 + ML-DSA-65` signing keypair.
#[napi]
pub fn ves_hybrid_generate_signing_keypair() -> Result<HybridSigningKeypairOutput> {
    let keypair = stateset_crypto::pqc::generate_hybrid_signing_keypair()
        .map_err(|e| Error::from_reason(format!("Hybrid signing key generation failed: {}", e)))?;

    Ok(HybridSigningKeypairOutput {
        ed25519_public_key: Buffer::from(keypair.public.ed25519_public_key.as_slice()),
        ed25519_private_key: Buffer::from(keypair.private.ed25519_private_key.as_slice()),
        ml_dsa_65_public_key: Buffer::from(keypair.public.ml_dsa_65_public_key.as_slice()),
        ml_dsa_65_seed: Buffer::from(keypair.private.ml_dsa_65_seed.as_slice()),
    })
}

/// Sign a 32-byte hash with the hybrid `Ed25519 + ML-DSA-65` profile.
#[napi]
pub fn ves_hybrid_sign_event_hash(
    hash: Buffer,
    ed25519_private_key: Buffer,
    ml_dsa_65_seed: Buffer,
) -> Result<HybridSignatureBundleOutput> {
    if hash.len() != 32 {
        return Err(Error::from_reason("Hash must be 32 bytes"));
    }
    if ed25519_private_key.len() != 32 {
        return Err(Error::from_reason("Ed25519 private key must be 32 bytes"));
    }
    if ml_dsa_65_seed.len() != 32 {
        return Err(Error::from_reason("ML-DSA-65 seed must be 32 bytes"));
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(hash.as_ref());
    let mut ed25519_private_key_arr = [0u8; 32];
    ed25519_private_key_arr.copy_from_slice(ed25519_private_key.as_ref());
    let mut ml_dsa_65_seed_arr = [0u8; 32];
    ml_dsa_65_seed_arr.copy_from_slice(ml_dsa_65_seed.as_ref());

    let signature = stateset_crypto::pqc::hybrid_sign_event_hash(
        &hash_arr,
        &stateset_crypto::pqc::HybridSigningPrivateKey {
            ed25519_private_key: ed25519_private_key_arr,
            ml_dsa_65_seed: ml_dsa_65_seed_arr,
        },
    )
    .map_err(|e| Error::from_reason(format!("Hybrid signing failed: {}", e)))?;

    Ok(HybridSignatureBundleOutput {
        ed25519_signature: Buffer::from(signature.ed25519_signature.as_slice()),
        ml_dsa_65_signature: Buffer::from(signature.ml_dsa_65_signature.as_slice()),
    })
}

/// Verify a 32-byte hash with the hybrid `Ed25519 + ML-DSA-65` profile.
#[napi]
pub fn ves_hybrid_verify_event_signature(
    hash: Buffer,
    ed25519_signature: Buffer,
    ml_dsa_65_signature: Buffer,
    ed25519_public_key: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    if hash.len() != 32 || ed25519_signature.len() != 64 || ed25519_public_key.len() != 32 {
        return Ok(false);
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(hash.as_ref());
    let mut ed25519_signature_arr = [0u8; 64];
    ed25519_signature_arr.copy_from_slice(ed25519_signature.as_ref());
    let mut ed25519_public_key_arr = [0u8; 32];
    ed25519_public_key_arr.copy_from_slice(ed25519_public_key.as_ref());

    Ok(stateset_crypto::pqc::hybrid_verify_event_signature(
        &hash_arr,
        &stateset_crypto::pqc::HybridSignatureBundle {
            ed25519_signature: ed25519_signature_arr,
            ml_dsa_65_signature: ml_dsa_65_signature.as_ref().to_vec(),
        },
        &stateset_crypto::pqc::HybridSigningPublicKey {
            ed25519_public_key: ed25519_public_key_arr,
            ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
        },
    ))
}

/// Return the fixed-seed ML-DSA-65 public key used by cross-language test vectors.
#[napi]
pub fn ves_test_vector_ml_dsa_public_key() -> Buffer {
    Buffer::from(
        stateset_crypto::pqc::test_vector_ml_dsa_public_key(
            &stateset_crypto::pqc::TEST_VECTOR_SIGNING_SEED,
        )
        .as_slice(),
    )
}

/// Generate a hybrid `X25519 + ML-KEM-768` recipient keypair.
#[napi]
pub fn ves_hybrid_generate_recipient_keypair(kid: u32) -> Result<HybridRecipientKeypairOutput> {
    let keypair = stateset_crypto::pqc::generate_hybrid_recipient_keypair(kid).map_err(|e| {
        Error::from_reason(format!("Hybrid recipient key generation failed: {}", e))
    })?;

    Ok(HybridRecipientKeypairOutput {
        kid: keypair.public.kid,
        x25519_public_key: Buffer::from(keypair.public.x25519_public_key.as_slice()),
        x25519_private_key: Buffer::from(keypair.private.x25519_private_key.as_slice()),
        ml_kem_768_public_key: Buffer::from(keypair.public.ml_kem_768_public_key.as_slice()),
        ml_kem_768_seed: Buffer::from(keypair.private.ml_kem_768_seed.as_slice()),
    })
}

/// Return the fixed-seed ML-KEM-768 public key used by cross-language test vectors.
#[napi]
pub fn ves_test_vector_ml_kem_public_key() -> Buffer {
    Buffer::from(
        stateset_crypto::pqc::test_vector_ml_kem_public_key(
            &stateset_crypto::pqc::TEST_VECTOR_KEM_SEED,
        )
        .as_slice(),
    )
}

/// Encrypt a JSON payload using hybrid `X25519 + ML-KEM-768` recipient wrapping.
#[napi]
pub fn ves_hybrid_encrypt_payload(
    payload_json: String,
    aad_params: HybridPayloadAadParamsInput,
    recipients: Vec<HybridRecipientPublicKeyInput>,
) -> Result<HybridEncryptionResultOutput> {
    if aad_params.payload_plain_hash.len() != 32 {
        return Err(Error::from_reason("payload_plain_hash must be 32 bytes"));
    }

    let payload: serde_json::Value = serde_json::from_str(&payload_json)
        .map_err(|e| Error::from_reason(format!("Invalid payload JSON: {}", e)))?;

    let mut payload_plain_hash = [0u8; 32];
    payload_plain_hash.copy_from_slice(aad_params.payload_plain_hash.as_ref());

    let recipient_keys = recipients
        .into_iter()
        .map(|recipient| {
            let mut x25519_public_key = [0u8; 32];
            if recipient.x25519_public_key.len() != 32 {
                return Err(Error::from_reason("x25519_public_key must be 32 bytes".to_string()));
            }
            x25519_public_key.copy_from_slice(recipient.x25519_public_key.as_ref());

            Ok(stateset_crypto::pqc::HybridRecipientPublicKey {
                kid: recipient.kid,
                x25519_public_key,
                ml_kem_768_public_key: recipient.ml_kem_768_public_key.as_ref().to_vec(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let aad = stateset_crypto::hash::PayloadAadParams {
        ves_version: aad_params.ves_version,
        tenant_id: &aad_params.tenant_id,
        store_id: &aad_params.store_id,
        event_id: &aad_params.event_id,
        source_agent_id: &aad_params.source_agent_id,
        agent_key_id: aad_params.agent_key_id,
        entity_type: &aad_params.entity_type,
        entity_id: &aad_params.entity_id,
        event_type: &aad_params.event_type,
        created_at: &aad_params.created_at,
        payload_plain_hash: &payload_plain_hash,
    };

    let encrypted =
        stateset_crypto::pqc::encrypt_payload_hybrid(&payload, &aad, &recipient_keys)
            .map_err(|e| Error::from_reason(format!("Hybrid payload encryption failed: {}", e)))?;

    Ok(HybridEncryptionResultOutput {
        payload_encrypted_json: serde_json::to_string(&encrypted.payload_encrypted).map_err(
            |e| Error::from_reason(format!("Failed to serialize encrypted payload: {}", e)),
        )?,
        salt: Buffer::from(encrypted.salt.as_slice()),
        payload_plain_hash: Buffer::from(encrypted.payload_plain_hash.as_slice()),
        payload_cipher_hash: Buffer::from(encrypted.payload_cipher_hash.as_slice()),
    })
}

/// Decrypt a JSON payload using hybrid `X25519 + ML-KEM-768` recipient wrapping.
#[napi]
pub fn ves_hybrid_decrypt_payload(
    payload_encrypted_json: String,
    payload_aad: Buffer,
    recipient_kid: u32,
    recipient_private_key: HybridRecipientPrivateKeyInput,
    expected_plain_hash: Buffer,
) -> Result<String> {
    if payload_aad.len() != 32 {
        return Err(Error::from_reason("payload_aad must be 32 bytes"));
    }
    if recipient_private_key.x25519_private_key.len() != 32 {
        return Err(Error::from_reason("x25519_private_key must be 32 bytes"));
    }
    if recipient_private_key.ml_kem_768_seed.len() != 64 {
        return Err(Error::from_reason("ml_kem_768_seed must be 64 bytes"));
    }
    if expected_plain_hash.len() != 32 {
        return Err(Error::from_reason("expected_plain_hash must be 32 bytes"));
    }

    let payload_encrypted: serde_json::Value = serde_json::from_str(&payload_encrypted_json)
        .map_err(|e| Error::from_reason(format!("Invalid encrypted payload JSON: {}", e)))?;

    let mut payload_aad_arr = [0u8; 32];
    payload_aad_arr.copy_from_slice(payload_aad.as_ref());
    let mut x25519_private_key = [0u8; 32];
    x25519_private_key.copy_from_slice(recipient_private_key.x25519_private_key.as_ref());
    let mut ml_kem_768_seed = [0u8; 64];
    ml_kem_768_seed.copy_from_slice(recipient_private_key.ml_kem_768_seed.as_ref());
    let mut expected_plain_hash_arr = [0u8; 32];
    expected_plain_hash_arr.copy_from_slice(expected_plain_hash.as_ref());

    let decrypted = stateset_crypto::pqc::decrypt_payload_hybrid(
        &payload_encrypted,
        &payload_aad_arr,
        recipient_kid,
        &stateset_crypto::pqc::HybridRecipientPrivateKey { x25519_private_key, ml_kem_768_seed },
        &expected_plain_hash_arr,
    )
    .map_err(|e| Error::from_reason(format!("Hybrid payload decryption failed: {}", e)))?;

    serde_json::to_string(&decrypted)
        .map_err(|e| Error::from_reason(format!("Failed to serialize decrypted payload: {}", e)))
}

// =============================================================================
// PQC-Strict Operations (ML-DSA-65 only, ML-KEM-768 only)
// =============================================================================

#[napi(object)]
pub struct StrictSigningKeypairOutput {
    pub ml_dsa_65_public_key: Buffer,
    pub ml_dsa_65_seed: Buffer,
}

#[napi(object)]
pub struct StrictRecipientKeypairOutput {
    pub kid: u32,
    pub ml_kem_768_public_key: Buffer,
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct StrictRecipientPrivateKeyInput {
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct StrictRecipientPublicKeyInput {
    pub kid: u32,
    pub ml_kem_768_public_key: Buffer,
}

#[napi(object)]
pub struct StrictEncryptionResultOutput {
    pub payload_encrypted_json: String,
    pub salt: Buffer,
    pub payload_plain_hash: Buffer,
    pub payload_cipher_hash: Buffer,
}

/// Generate an ML-DSA-65-only signing keypair for PQC-strict mode.
#[napi]
pub fn ves_strict_generate_signing_keypair() -> Result<StrictSigningKeypairOutput> {
    let keypair = stateset_crypto::pqc::generate_strict_signing_keypair()
        .map_err(|e| Error::from_reason(format!("Strict signing key generation failed: {}", e)))?;

    Ok(StrictSigningKeypairOutput {
        ml_dsa_65_public_key: Buffer::from(keypair.public.ml_dsa_65_public_key.as_slice()),
        ml_dsa_65_seed: Buffer::from(keypair.private.ml_dsa_65_seed.as_slice()),
    })
}

/// Sign a 32-byte hash with ML-DSA-65 only (PQC-strict mode).
#[napi]
pub fn ves_strict_sign_event_hash(hash: Buffer, ml_dsa_65_seed: Buffer) -> Result<Buffer> {
    if hash.len() != 32 {
        return Err(Error::from_reason("Hash must be 32 bytes"));
    }
    if ml_dsa_65_seed.len() != 32 {
        return Err(Error::from_reason("ML-DSA-65 seed must be 32 bytes"));
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(hash.as_ref());
    let mut seed_arr = [0u8; 32];
    seed_arr.copy_from_slice(ml_dsa_65_seed.as_ref());

    let signature = stateset_crypto::pqc::strict_sign_event_hash(
        &hash_arr,
        &stateset_crypto::pqc::StrictSigningPrivateKey { ml_dsa_65_seed: seed_arr },
    )
    .map_err(|e| Error::from_reason(format!("Strict signing failed: {}", e)))?;

    Ok(Buffer::from(signature))
}

/// Verify a 32-byte hash with ML-DSA-65 only (PQC-strict mode).
#[napi]
pub fn ves_strict_verify_event_signature(
    hash: Buffer,
    ml_dsa_65_signature: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    if hash.len() != 32 {
        return Ok(false);
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(hash.as_ref());

    Ok(stateset_crypto::pqc::strict_verify_event_signature(
        &hash_arr,
        ml_dsa_65_signature.as_ref(),
        &stateset_crypto::pqc::StrictSigningPublicKey {
            ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
        },
    ))
}

/// Generate an ML-KEM-768-only recipient keypair for PQC-strict mode.
#[napi]
pub fn ves_strict_generate_recipient_keypair(kid: u32) -> Result<StrictRecipientKeypairOutput> {
    let keypair = stateset_crypto::pqc::generate_strict_recipient_keypair(kid).map_err(|e| {
        Error::from_reason(format!("Strict recipient key generation failed: {}", e))
    })?;

    Ok(StrictRecipientKeypairOutput {
        kid: keypair.public.kid,
        ml_kem_768_public_key: Buffer::from(keypair.public.ml_kem_768_public_key.as_slice()),
        ml_kem_768_seed: Buffer::from(keypair.private.ml_kem_768_seed.as_slice()),
    })
}

/// Encrypt a JSON payload using ML-KEM-768-only recipient wrapping (PQC-strict).
#[napi]
pub fn ves_strict_encrypt_payload(
    payload_json: String,
    aad_params: HybridPayloadAadParamsInput,
    recipients: Vec<StrictRecipientPublicKeyInput>,
) -> Result<StrictEncryptionResultOutput> {
    if aad_params.payload_plain_hash.len() != 32 {
        return Err(Error::from_reason("payload_plain_hash must be 32 bytes"));
    }

    let payload: serde_json::Value = serde_json::from_str(&payload_json)
        .map_err(|e| Error::from_reason(format!("Invalid payload JSON: {}", e)))?;

    let mut payload_plain_hash = [0u8; 32];
    payload_plain_hash.copy_from_slice(aad_params.payload_plain_hash.as_ref());

    let recipient_keys: Vec<stateset_crypto::pqc::StrictRecipientPublicKey> = recipients
        .into_iter()
        .map(|r| stateset_crypto::pqc::StrictRecipientPublicKey {
            kid: r.kid,
            ml_kem_768_public_key: r.ml_kem_768_public_key.as_ref().to_vec(),
        })
        .collect();

    let aad = stateset_crypto::hash::PayloadAadParams {
        ves_version: aad_params.ves_version,
        tenant_id: &aad_params.tenant_id,
        store_id: &aad_params.store_id,
        event_id: &aad_params.event_id,
        source_agent_id: &aad_params.source_agent_id,
        agent_key_id: aad_params.agent_key_id,
        entity_type: &aad_params.entity_type,
        entity_id: &aad_params.entity_id,
        event_type: &aad_params.event_type,
        created_at: &aad_params.created_at,
        payload_plain_hash: &payload_plain_hash,
    };

    let encrypted =
        stateset_crypto::pqc::encrypt_payload_strict(&payload, &aad, &recipient_keys)
            .map_err(|e| Error::from_reason(format!("Strict payload encryption failed: {}", e)))?;

    Ok(StrictEncryptionResultOutput {
        payload_encrypted_json: serde_json::to_string(&encrypted.payload_encrypted)
            .map_err(|e| Error::from_reason(format!("Failed to serialize: {}", e)))?,
        salt: Buffer::from(encrypted.salt.as_slice()),
        payload_plain_hash: Buffer::from(encrypted.payload_plain_hash.as_slice()),
        payload_cipher_hash: Buffer::from(encrypted.payload_cipher_hash.as_slice()),
    })
}

/// Decrypt a JSON payload using ML-KEM-768-only recipient wrapping (PQC-strict).
#[napi]
pub fn ves_strict_decrypt_payload(
    payload_encrypted_json: String,
    payload_aad: Buffer,
    recipient_kid: u32,
    recipient_private_key: StrictRecipientPrivateKeyInput,
    expected_plain_hash: Buffer,
) -> Result<String> {
    if payload_aad.len() != 32 {
        return Err(Error::from_reason("payload_aad must be 32 bytes"));
    }
    if recipient_private_key.ml_kem_768_seed.len() != 64 {
        return Err(Error::from_reason("ml_kem_768_seed must be 64 bytes"));
    }
    if expected_plain_hash.len() != 32 {
        return Err(Error::from_reason("expected_plain_hash must be 32 bytes"));
    }

    let payload_encrypted: serde_json::Value = serde_json::from_str(&payload_encrypted_json)
        .map_err(|e| Error::from_reason(format!("Invalid encrypted payload JSON: {}", e)))?;

    let mut aad_arr = [0u8; 32];
    aad_arr.copy_from_slice(payload_aad.as_ref());
    let mut seed = [0u8; 64];
    seed.copy_from_slice(recipient_private_key.ml_kem_768_seed.as_ref());
    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(expected_plain_hash.as_ref());

    let decrypted = stateset_crypto::pqc::decrypt_payload_strict(
        &payload_encrypted,
        &aad_arr,
        recipient_kid,
        &stateset_crypto::pqc::StrictRecipientPrivateKey { ml_kem_768_seed: seed },
        &hash_arr,
    )
    .map_err(|e| Error::from_reason(format!("Strict payload decryption failed: {}", e)))?;

    serde_json::to_string(&decrypted)
        .map_err(|e| Error::from_reason(format!("Failed to serialize: {}", e)))
}

/// Generate a hybrid signing proof-of-possession bundle.
#[napi]
pub fn ves_hybrid_generate_signing_pop(
    ed25519_private_key: Buffer,
    ml_dsa_65_seed: Buffer,
    ed25519_public_key: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<HybridSignatureBundleOutput> {
    if ed25519_private_key.len() != 32
        || ml_dsa_65_seed.len() != 32
        || ed25519_public_key.len() != 32
    {
        return Err(Error::from_reason("Key sizes invalid"));
    }

    let mut ed_priv = [0u8; 32];
    ed_priv.copy_from_slice(ed25519_private_key.as_ref());
    let mut ml_seed = [0u8; 32];
    ml_seed.copy_from_slice(ml_dsa_65_seed.as_ref());
    let mut ed_pub = [0u8; 32];
    ed_pub.copy_from_slice(ed25519_public_key.as_ref());

    let keypair = stateset_crypto::pqc::HybridSigningKeypair {
        public: stateset_crypto::pqc::HybridSigningPublicKey {
            ed25519_public_key: ed_pub,
            ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
        },
        private: stateset_crypto::pqc::HybridSigningPrivateKey {
            ed25519_private_key: ed_priv,
            ml_dsa_65_seed: ml_seed,
        },
    };

    let pop = stateset_crypto::pqc::generate_hybrid_signing_pop(&keypair)
        .map_err(|e| Error::from_reason(format!("PoP generation failed: {}", e)))?;

    Ok(HybridSignatureBundleOutput {
        ed25519_signature: Buffer::from(pop.ed25519_signature.as_slice()),
        ml_dsa_65_signature: Buffer::from(pop.ml_dsa_65_signature.as_slice()),
    })
}

/// Verify a hybrid signing proof-of-possession bundle.
#[napi]
pub fn ves_hybrid_verify_signing_pop(
    ed25519_signature: Buffer,
    ml_dsa_65_signature: Buffer,
    ed25519_public_key: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    if ed25519_signature.len() != 64 || ed25519_public_key.len() != 32 {
        return Ok(false);
    }

    let mut ed_sig = [0u8; 64];
    ed_sig.copy_from_slice(ed25519_signature.as_ref());
    let mut ed_pub = [0u8; 32];
    ed_pub.copy_from_slice(ed25519_public_key.as_ref());

    Ok(stateset_crypto::pqc::verify_hybrid_signing_pop(
        &stateset_crypto::pqc::HybridSignatureBundle {
            ed25519_signature: ed_sig,
            ml_dsa_65_signature: ml_dsa_65_signature.as_ref().to_vec(),
        },
        &stateset_crypto::pqc::HybridSigningPublicKey {
            ed25519_public_key: ed_pub,
            ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
        },
    ))
}

/// Generate a PQC-strict signing proof-of-possession.
#[napi]
pub fn ves_strict_generate_signing_pop(
    ml_dsa_65_seed: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<Buffer> {
    if ml_dsa_65_seed.len() != 32 {
        return Err(Error::from_reason("ML-DSA-65 seed must be 32 bytes"));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(ml_dsa_65_seed.as_ref());

    let keypair = stateset_crypto::pqc::StrictSigningKeypair {
        public: stateset_crypto::pqc::StrictSigningPublicKey {
            ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
        },
        private: stateset_crypto::pqc::StrictSigningPrivateKey { ml_dsa_65_seed: seed },
    };

    let pop = stateset_crypto::pqc::generate_strict_signing_pop(&keypair)
        .map_err(|e| Error::from_reason(format!("Strict PoP generation failed: {}", e)))?;
    Ok(Buffer::from(pop))
}

/// Verify a PQC-strict signing proof-of-possession.
#[napi]
pub fn ves_strict_verify_signing_pop(
    ml_dsa_65_signature: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    Ok(stateset_crypto::pqc::verify_strict_signing_pop(
        ml_dsa_65_signature.as_ref(),
        &stateset_crypto::pqc::StrictSigningPublicKey {
            ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
        },
    ))
}

/// Encrypt a buffer with AES-256-GCM
///
/// Returns nonce (12 bytes) || ciphertext || tag (16 bytes)
#[napi]
pub fn aes_gcm_encrypt(plaintext: Buffer, key: Buffer, aad: Buffer) -> Result<Buffer> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

    if key.len() != 32 {
        return Err(Error::from_reason("Key must be 32 bytes"));
    }

    let aes_key = Key::<Aes256Gcm>::from_slice(key.as_ref());
    let cipher = Aes256Gcm::new(aes_key);

    let mut nonce_bytes = [0u8; 12];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let payload = aes_gcm::aead::Payload { msg: plaintext.as_ref(), aad: aad.as_ref() };
    let ciphertext_tag = cipher
        .encrypt(nonce, payload)
        .map_err(|e| Error::from_reason(format!("Encryption failed: {}", e)))?;

    let mut result = Vec::with_capacity(12 + ciphertext_tag.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext_tag);
    Ok(Buffer::from(result))
}

/// Decrypt a buffer with AES-256-GCM
///
/// Input: nonce (12 bytes) || ciphertext || tag (16 bytes)
#[napi]
pub fn aes_gcm_decrypt(encrypted: Buffer, key: Buffer, aad: Buffer) -> Result<Buffer> {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

    if key.len() != 32 {
        return Err(Error::from_reason("Key must be 32 bytes"));
    }
    if encrypted.len() < 28 {
        return Err(Error::from_reason("Encrypted data too short (need at least nonce + tag)"));
    }

    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext_tag = &encrypted[12..];

    let aes_key = Key::<Aes256Gcm>::from_slice(key.as_ref());
    let cipher = Aes256Gcm::new(aes_key);

    let payload = aes_gcm::aead::Payload { msg: ciphertext_tag, aad: aad.as_ref() };
    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|e| Error::from_reason(format!("Decryption failed: {}", e)))?;

    Ok(Buffer::from(plaintext))
}

/// Compute Merkle root from an array of 32-byte leaf hashes
#[napi]
pub fn merkle_root(leaves: Vec<Buffer>) -> Result<Buffer> {
    let leaf_arrays: std::result::Result<Vec<[u8; 32]>, _> = leaves
        .iter()
        .map(|b| {
            if b.len() != 32 {
                Err(Error::from_reason("Each leaf must be 32 bytes"))
            } else {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(b.as_ref());
                Ok(arr)
            }
        })
        .collect();

    let leaf_arrays = leaf_arrays?;
    let root = stateset_crypto::merkle::compute_merkle_root(&leaf_arrays);
    Ok(Buffer::from(root.as_slice()))
}
