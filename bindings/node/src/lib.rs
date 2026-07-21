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

    /// Get the gift cards API
    #[napi(getter)]
    pub fn gift_cards(&self) -> GiftCards {
        GiftCards { commerce: self.inner.clone() }
    }

    /// Get the loyalty API
    #[napi(getter)]
    pub fn loyalty(&self) -> Loyalty {
        Loyalty { commerce: self.inner.clone() }
    }

    /// Get the store credits API
    #[napi(getter)]
    pub fn store_credits(&self) -> StoreCredits {
        StoreCredits { commerce: self.inner.clone() }
    }

    /// Get the product reviews API
    #[napi(getter)]
    pub fn reviews(&self) -> Reviews {
        Reviews { commerce: self.inner.clone() }
    }

    /// Get the wishlists API
    #[napi(getter)]
    pub fn wishlists(&self) -> Wishlists {
        Wishlists { commerce: self.inner.clone() }
    }

    /// Get the customer segments API
    #[napi(getter)]
    pub fn segments(&self) -> Segments {
        Segments { commerce: self.inner.clone() }
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

    /// Get the fixed assets API
    #[napi(getter)]
    pub fn fixed_assets(&self) -> FixedAssets {
        FixedAssets { commerce: self.inner.clone() }
    }

    /// Get the revenue recognition (ASC 606) API
    #[napi(getter)]
    pub fn revenue_recognition(&self) -> RevenueRecognition {
        RevenueRecognition { commerce: self.inner.clone() }
    }

    /// Get the cycle counts API
    #[napi(getter)]
    pub fn cycle_counts(&self) -> CycleCounts {
        CycleCounts { commerce: self.inner.clone() }
    }

    /// Get the EDI documents API (trading-partner document tracking)
    #[napi(getter)]
    pub fn edi_documents(&self) -> EdiDocuments {
        EdiDocuments { commerce: self.inner.clone() }
    }

    /// Get the activity logs API (append-only subject history)
    #[napi(getter)]
    pub fn activity_logs(&self) -> ActivityLogs {
        ActivityLogs { commerce: self.inner.clone() }
    }

    /// Get the channels API (sales / fulfillment channels)
    #[napi(getter)]
    pub fn channels(&self) -> Channels {
        Channels { commerce: self.inner.clone() }
    }

    /// Get the companies API (B2B accounts and contacts)
    #[napi(getter)]
    pub fn companies(&self) -> Companies {
        Companies { commerce: self.inner.clone() }
    }

    /// Get the units of measure API (unit classes, UOMs, conversion rules)
    #[napi(getter)]
    pub fn units_of_measure(&self) -> UnitsOfMeasure {
        UnitsOfMeasure { commerce: self.inner.clone() }
    }

    /// Get the shipping zones API (geographic zones, methods, rates)
    #[napi(getter)]
    pub fn shipping_zones(&self) -> ShippingZones {
        ShippingZones { commerce: self.inner.clone() }
    }

    /// Get the stock snapshots API (point-in-time inventory)
    #[napi(getter)]
    pub fn stock_snapshots(&self) -> StockSnapshots {
        StockSnapshots { commerce: self.inner.clone() }
    }

    /// Get the print stations API (paired agents + print job queue)
    #[napi(getter)]
    pub fn print_stations(&self) -> PrintStations {
        PrintStations { commerce: self.inner.clone() }
    }

    /// Get the integration mappings API (external↔internal value translation)
    #[napi(getter)]
    pub fn integration_mappings(&self) -> IntegrationMappings {
        IntegrationMappings { commerce: self.inner.clone() }
    }

    /// Get the integration field mappings API (field-path mappings)
    #[napi(getter)]
    pub fn integration_field_mappings(&self) -> IntegrationFieldMappings {
        IntegrationFieldMappings { commerce: self.inner.clone() }
    }

    /// Get the payment obligations API (scheduled AP payments)
    #[napi(getter)]
    pub fn payment_obligations(&self) -> PaymentObligations {
        PaymentObligations { commerce: self.inner.clone() }
    }

    /// Get the maintenance API (backup, restore, export, import)
    #[napi(getter)]
    pub fn maintenance(&self) -> Maintenance {
        Maintenance { commerce: self.inner.clone() }
    }

    /// Get the purgatory API (order ingestion staging)
    #[napi(getter)]
    pub fn purgatory(&self) -> Purgatory {
        Purgatory { commerce: self.inner.clone() }
    }

    /// Get the topology snapshots API (operational topology health)
    #[napi(getter)]
    pub fn topology_snapshots(&self) -> TopologySnapshots {
        TopologySnapshots { commerce: self.inner.clone() }
    }

    /// Get the fraud API (risk assessments and detection rules)
    #[napi(getter)]
    pub fn fraud(&self) -> Fraud {
        Fraud { commerce: self.inner.clone() }
    }

    /// Get the search configuration API (search tuning profiles)
    #[napi(getter)]
    pub fn search_config(&self) -> SearchConfigs {
        SearchConfigs { commerce: self.inner.clone() }
    }

    /// Get the ERC-8004 API (trustless agent identity, reputation, validation)
    #[napi(getter)]
    pub fn erc8004(&self) -> Erc8004 {
        Erc8004 { commerce: self.inner.clone() }
    }

    /// Get the vendor returns API (return-to-supplier)
    #[napi(getter)]
    pub fn vendor_returns(&self) -> VendorReturns {
        VendorReturns { commerce: self.inner.clone() }
    }

    /// Get the prepayments API (advance payments to suppliers)
    #[napi(getter)]
    pub fn prepayments(&self) -> Prepayments {
        Prepayments { commerce: self.inner.clone() }
    }

    /// Get the vendor credits API (supplier-owed credits)
    #[napi(getter)]
    pub fn vendor_credits(&self) -> VendorCredits {
        VendorCredits { commerce: self.inner.clone() }
    }

    /// Get the price schedules API (time-bounded pricing)
    #[napi(getter)]
    pub fn price_schedules(&self) -> PriceSchedules {
        PriceSchedules { commerce: self.inner.clone() }
    }

    /// Get the price levels API (B2B pricing tiers)
    #[napi(getter)]
    pub fn price_levels(&self) -> PriceLevels {
        PriceLevels { commerce: self.inner.clone() }
    }

    /// Get the transfer orders API (inter-warehouse stock movement)
    #[napi(getter)]
    pub fn transfer_orders(&self) -> TransferOrders {
        TransferOrders { commerce: self.inner.clone() }
    }

    /// Get the production batches API (grouping manufacturing work orders)
    #[napi(getter)]
    pub fn production_batches(&self) -> ProductionBatches {
        ProductionBatches { commerce: self.inner.clone() }
    }

    /// Get the supplier SKUs API (per-supplier SKU / unit-cost overrides)
    #[napi(getter)]
    pub fn supplier_skus(&self) -> SupplierSkus {
        SupplierSkus { commerce: self.inner.clone() }
    }

    /// Get the inbound shipments API (advance ship notices)
    #[napi(getter)]
    pub fn inbound_shipments(&self) -> InboundShipments {
        InboundShipments { commerce: self.inner.clone() }
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
            ..Default::default()
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
            ..Default::default()
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

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ThreeWayMatchLineOutput {
    pub po_line_id: Option<String>,
    pub bill_item_id: String,
    pub description: String,
    /// Exact decimal string
    pub ordered_quantity: Option<String>,
    /// Exact decimal string
    pub ordered_unit_cost: Option<String>,
    /// Exact decimal string
    pub received_quantity: String,
    /// Exact decimal string
    pub billed_quantity: String,
    /// Exact decimal string
    pub billed_unit_cost: String,
    /// Exact decimal string: billed_quantity - received_quantity
    pub quantity_variance: String,
    /// Exact decimal string: billed_unit_cost - ordered_unit_cost
    pub price_variance: String,
    pub matched: bool,
    pub issues: Vec<String>,
}

impl From<stateset_core::ThreeWayMatchLine> for ThreeWayMatchLineOutput {
    fn from(l: stateset_core::ThreeWayMatchLine) -> Self {
        Self {
            po_line_id: l.po_line_id.map(|id| id.to_string()),
            bill_item_id: l.bill_item_id.to_string(),
            description: l.description,
            ordered_quantity: l.ordered_quantity.map(|d| d.to_string()),
            ordered_unit_cost: l.ordered_unit_cost.map(|d| d.to_string()),
            received_quantity: l.received_quantity.to_string(),
            billed_quantity: l.billed_quantity.to_string(),
            billed_unit_cost: l.billed_unit_cost.to_string(),
            quantity_variance: l.quantity_variance.to_string(),
            price_variance: l.price_variance.to_string(),
            matched: l.matched,
            issues: l.issues,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ThreeWayMatchOutput {
    /// Overall status: not_required, pending, matched, variance
    pub match_status: String,
    /// Number of variance lines (set when match_status is "variance")
    pub variance_line_count: Option<u32>,
    /// Tolerance applied, as an exact decimal string percentage (e.g. "5")
    pub tolerance_percent: String,
    pub lines: Vec<ThreeWayMatchLineOutput>,
}

impl From<stateset_core::ThreeWayMatchResult> for ThreeWayMatchOutput {
    fn from(r: stateset_core::ThreeWayMatchResult) -> Self {
        let (match_status, variance_line_count) = match r.match_status {
            stateset_core::MatchStatus::NotRequired => ("not_required".to_string(), None),
            stateset_core::MatchStatus::Pending => ("pending".to_string(), None),
            stateset_core::MatchStatus::Matched => ("matched".to_string(), None),
            stateset_core::MatchStatus::Variance { variance_line_count } => {
                ("variance".to_string(), Some(variance_line_count as u32))
            }
            _ => ("unknown".to_string(), None),
        };
        Self {
            match_status,
            variance_line_count,
            tolerance_percent: r.tolerance_percent.to_string(),
            lines: r.lines.into_iter().map(Into::into).collect(),
        }
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

    /// Three-way match a bill against its purchase order and receipts.
    ///
    /// `tolerance_percent` is an exact decimal string (e.g. "5" for 5%);
    /// omit it for exact matching.
    #[napi]
    pub async fn three_way_match(
        &self,
        bill_id: String,
        tolerance_percent: Option<String>,
    ) -> Result<ThreeWayMatchOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = bill_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let tolerance = tolerance_percent
            .map(|s| {
                s.parse::<Decimal>()
                    .map_err(|_| Error::from_reason("Invalid tolerance_percent decimal"))
            })
            .transpose()?;
        let result = commerce
            .accounts_payable()
            .three_way_match(uuid, tolerance)
            .map_err(|e| Error::from_reason(format!("Failed to three-way match bill: {}", e)))?;
        Ok(result.into())
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

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevaluationLineOutput {
    pub account_id: String,
    pub account_number: String,
    pub account_name: String,
    pub currency: String,
    /// Side that increases this account: debit or credit
    pub normal_balance: String,
    /// Exact decimal string
    pub foreign_balance: String,
    /// Exact decimal string
    pub carrying_value: String,
    /// Exact decimal string
    pub rate: String,
    /// Exact decimal string
    pub revalued_value: String,
    /// Exact decimal string
    pub adjustment: String,
    /// Exact decimal string
    pub unrealized_gain_loss: String,
}

impl From<stateset_core::RevaluationLine> for RevaluationLineOutput {
    fn from(l: stateset_core::RevaluationLine) -> Self {
        Self {
            account_id: l.account_id.to_string(),
            account_number: l.account_number,
            account_name: l.account_name,
            currency: l.currency.to_string(),
            normal_balance: match l.normal_balance {
                stateset_core::BalanceSide::Debit => "debit".to_string(),
                stateset_core::BalanceSide::Credit => "credit".to_string(),
                _ => "unknown".to_string(),
            },
            foreign_balance: l.foreign_balance.to_string(),
            carrying_value: l.carrying_value.to_string(),
            rate: l.rate.to_string(),
            revalued_value: l.revalued_value.to_string(),
            adjustment: l.adjustment.to_string(),
            unrealized_gain_loss: l.unrealized_gain_loss.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevaluationOutput {
    /// ISO date (YYYY-MM-DD)
    pub as_of_date: String,
    pub base_currency: String,
    /// Exact decimal string
    pub total_unrealized_gain_loss: String,
    pub lines: Vec<RevaluationLineOutput>,
    /// Balanced adjusting entry; None when no adjustment was required.
    pub journal_entry: Option<JournalEntryOutput>,
}

impl From<stateset_core::RevaluationResult> for RevaluationOutput {
    fn from(r: stateset_core::RevaluationResult) -> Self {
        Self {
            as_of_date: r.as_of_date.to_string(),
            base_currency: r.base_currency.to_string(),
            total_unrealized_gain_loss: r.total_unrealized_gain_loss.to_string(),
            lines: r.lines.into_iter().map(Into::into).collect(),
            journal_entry: r.journal_entry.map(Into::into),
        }
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

    /// Revalue foreign-currency account balances at the as-of exchange rate.
    ///
    /// `as_of_date` is an ISO date (YYYY-MM-DD); `base_currency` defaults to
    /// the store's configured base currency.
    #[napi]
    pub async fn revalue(
        &self,
        as_of_date: String,
        base_currency: Option<String>,
    ) -> Result<RevaluationOutput> {
        let commerce = self.commerce.lock().await;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| Error::from_reason("Invalid date format"))?;
        let base = base_currency
            .map(|s| {
                s.parse::<stateset_core::Currency>()
                    .map_err(|_| Error::from_reason("Invalid base currency code"))
            })
            .transpose()?;
        let result = commerce
            .general_ledger()
            .revalue(date, base)
            .map_err(|e| Error::from_reason(format!("Failed to revalue: {}", e)))?;
        Ok(result.into())
    }

    /// Create an accounting period.
    #[napi]
    pub async fn create_period(&self, input: CreateGlPeriodInput) -> Result<GlPeriodOutput> {
        let commerce = self.commerce.lock().await;
        let period = commerce
            .general_ledger()
            .create_period(stateset_core::CreateGlPeriod {
                period_name: input.period_name,
                fiscal_year: input.fiscal_year,
                period_number: input.period_number,
                start_date: chrono::NaiveDate::parse_from_str(&input.start_date, "%Y-%m-%d")
                    .map_err(|_| Error::from_reason("Invalid start date format"))?,
                end_date: chrono::NaiveDate::parse_from_str(&input.end_date, "%Y-%m-%d")
                    .map_err(|_| Error::from_reason("Invalid end date format"))?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create period: {}", e)))?;
        Ok(period.into())
    }

    /// Open a period (transition from future to open).
    #[napi]
    pub async fn open_period(&self, id: String) -> Result<GlPeriodOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let period = commerce
            .general_ledger()
            .open_period(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to open period: {}", e)))?;
        Ok(period.into())
    }

    /// List accounting periods with optional filtering.
    #[napi]
    pub async fn list_periods(
        &self,
        filter: Option<GlPeriodFilterInput>,
    ) -> Result<Vec<GlPeriodOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();
        let status = filter
            .status
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::PeriodStatus>()
                    .map_err(|_| Error::from_reason(format!("Invalid period status: {}", s)))
            })
            .transpose()?;
        let periods = commerce
            .general_ledger()
            .list_periods(stateset_core::GlPeriodFilter {
                fiscal_year: filter.fiscal_year,
                status,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list periods: {}", e)))?;
        Ok(periods.into_iter().map(Into::into).collect())
    }

    /// Close the month: post scheduled depreciation, recognize revenue
    /// through period end, revalue foreign-currency balances, then run the
    /// period close (closing entries + close period).
    ///
    /// Pass `{ dryRun: true }` to compute per-step counts and amounts without
    /// writing anything.
    #[napi]
    pub async fn close_month(
        &self,
        period_id: String,
        options: Option<CloseMonthOptionsInput>,
    ) -> Result<CloseMonthReportOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = period_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let options = options.unwrap_or_default();
        let report = commerce
            .general_ledger()
            .close_month(
                uuid,
                stateset_core::CloseMonthOptions {
                    dry_run: options.dry_run.unwrap_or(false),
                    skip_depreciation: options.skip_depreciation.unwrap_or(false),
                    skip_revenue_recognition: options.skip_revenue_recognition.unwrap_or(false),
                    skip_fx_revaluation: options.skip_fx_revaluation.unwrap_or(false),
                    skip_period_close: options.skip_period_close.unwrap_or(false),
                    closed_by: options.closed_by,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to close month: {}", e)))?;
        Ok(report.into())
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateGlPeriodInput {
    /// Display name, typically `YYYY-MM`
    pub period_name: String,
    pub fiscal_year: i32,
    /// Sequential number within the fiscal year (1-12 for monthly)
    pub period_number: i32,
    /// First date of the period (inclusive), ISO date (YYYY-MM-DD)
    pub start_date: String,
    /// Last date of the period (inclusive), ISO date (YYYY-MM-DD)
    pub end_date: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GlPeriodOutput {
    pub id: String,
    pub period_name: String,
    pub fiscal_year: i32,
    pub period_number: i32,
    /// ISO date (YYYY-MM-DD)
    pub start_date: String,
    /// ISO date (YYYY-MM-DD)
    pub end_date: String,
    /// One of `future`, `open`, `closed`, `locked`
    pub status: String,
    pub closed_by: Option<String>,
}

impl From<stateset_core::GlPeriod> for GlPeriodOutput {
    fn from(p: stateset_core::GlPeriod) -> Self {
        Self {
            id: p.id.to_string(),
            period_name: p.period_name,
            fiscal_year: p.fiscal_year,
            period_number: p.period_number,
            start_date: p.start_date.to_string(),
            end_date: p.end_date.to_string(),
            status: p.status.to_string(),
            closed_by: p.closed_by,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct GlPeriodFilterInput {
    /// Filter by fiscal year
    pub fiscal_year: Option<i32>,
    /// Filter by status: one of `future`, `open`, `closed`, `locked`
    pub status: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CloseMonthOptionsInput {
    /// Compute per-step counts/amounts without writing anything
    pub dry_run: Option<bool>,
    /// Skip posting scheduled fixed-asset depreciation
    pub skip_depreciation: Option<bool>,
    /// Skip recognizing deferred revenue through period end
    pub skip_revenue_recognition: Option<bool>,
    /// Skip FX revaluation of foreign-currency accounts
    pub skip_fx_revaluation: Option<bool>,
    /// Skip the final period close (closing entries + close period)
    pub skip_period_close: Option<bool>,
    /// Actor recorded as the closer; defaults to `system`
    pub closed_by: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CloseMonthStepOutput {
    /// One of `executed`, `skipped`, `dry_run`
    pub status: String,
    /// Entries posted (or that would be posted in a dry run)
    pub entry_count: i64,
    /// Exact decimal string
    pub total_amount: String,
    /// Per-item failures that did not abort the close
    pub warnings: Vec<String>,
}

impl From<stateset_core::CloseMonthStepReport> for CloseMonthStepOutput {
    fn from(step: stateset_core::CloseMonthStepReport) -> Self {
        Self {
            status: step.status.to_string(),
            entry_count: i64::try_from(step.entry_count).unwrap_or(i64::MAX),
            total_amount: step.total_amount.to_string(),
            warnings: step.warnings,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CloseMonthReportOutput {
    pub period_id: String,
    pub period_name: String,
    pub dry_run: bool,
    /// Step 1: scheduled depreciation due through period end
    pub depreciation: CloseMonthStepOutput,
    /// Step 2: deferred revenue recognized through period end
    pub revenue_recognition: CloseMonthStepOutput,
    /// Step 3: FX revaluation as of period end
    pub fx_revaluation: CloseMonthStepOutput,
    /// Step 4: closing entries + close period
    pub period_close: CloseMonthStepOutput,
    /// Posted closing entry; None for dry runs or skipped closes
    pub closing_entry: Option<JournalEntryOutput>,
    /// Period status after the run (`closed` after a real close)
    pub period_status: String,
}

impl From<stateset_core::CloseMonthReport> for CloseMonthReportOutput {
    fn from(r: stateset_core::CloseMonthReport) -> Self {
        Self {
            period_id: r.period_id.to_string(),
            period_name: r.period_name,
            dry_run: r.dry_run,
            depreciation: r.depreciation.into(),
            revenue_recognition: r.revenue_recognition.into(),
            fx_revaluation: r.fx_revaluation.into(),
            period_close: r.period_close.into(),
            closing_entry: r.closing_entry.map(Into::into),
            period_status: r.period_status.to_string(),
        }
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
    pub signature_scheme: Option<String>,
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
                signature_scheme: input
                    .signature_scheme
                    .as_deref()
                    .map(parse_x402_signature_scheme)
                    .transpose()?,
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

// ============================================================================
// Gift Cards  (money represented as exact decimal STRINGS, not f64 — new code
// avoids the precision loss of the binding's older f64 money fields)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateGiftCardInput {
    /// Redemption code (auto-generated if omitted)
    pub code: Option<String>,
    /// Initial balance as an exact decimal string, e.g. "50.00"
    pub initial_balance: String,
    /// Currency code, e.g. "USD"
    pub currency: String,
    pub recipient_email: Option<String>,
    pub sender_name: Option<String>,
    pub message: Option<String>,
    /// RFC 3339 expiry timestamp
    pub expires_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateGiftCardInput {
    pub status: Option<String>,
    pub recipient_email: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GiftCardFilterInput {
    pub status: Option<String>,
    pub code: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GiftCardOutput {
    pub id: String,
    pub code: String,
    /// Exact decimal string
    pub initial_balance: String,
    /// Exact decimal string
    pub current_balance: String,
    pub currency: String,
    pub status: String,
    pub recipient_email: Option<String>,
    pub sender_name: Option<String>,
    pub message: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::GiftCard> for GiftCardOutput {
    fn from(g: stateset_core::GiftCard) -> Self {
        Self {
            id: g.id.to_string(),
            code: g.code,
            initial_balance: g.initial_balance.to_string(),
            current_balance: g.current_balance.to_string(),
            currency: g.currency.to_string(),
            status: format!("{}", g.status),
            recipient_email: g.recipient_email,
            sender_name: g.sender_name,
            message: g.message,
            expires_at: g.expires_at.map(|d| d.to_rfc3339()),
            created_at: g.created_at.to_rfc3339(),
            updated_at: g.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GiftCardTransactionOutput {
    pub id: String,
    pub gift_card_id: String,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub balance_after: String,
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::GiftCardTransaction> for GiftCardTransactionOutput {
    fn from(t: stateset_core::GiftCardTransaction) -> Self {
        Self {
            id: t.id.to_string(),
            gift_card_id: t.gift_card_id.to_string(),
            amount: t.amount.to_string(),
            balance_after: t.balance_after.to_string(),
            transaction_type: format!("{}", t.transaction_type),
            reference_id: t.reference_id,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct GiftCards {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl GiftCards {
    /// Whether the gift-cards backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.gift_cards().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateGiftCardInput) -> Result<GiftCardOutput> {
        let commerce = self.commerce.lock().await;
        let initial_balance = input
            .initial_balance
            .parse::<Decimal>()
            .map_err(|_| Error::from_reason("Invalid initial_balance decimal"))?;
        let currency = input
            .currency
            .parse::<CurrencyCode>()
            .map_err(|_| Error::from_reason("Invalid currency code"))?;
        let expires_at = match input.expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| Error::from_reason("Invalid expires_at RFC 3339 timestamp"))?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let card = commerce
            .gift_cards()
            .create(stateset_core::CreateGiftCard {
                code: input.code,
                initial_balance,
                currency,
                recipient_email: input.recipient_email,
                sender_name: input.sender_name,
                message: input.message,
                expires_at,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create gift card: {}", e)))?;
        Ok(card.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<GiftCardOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let card = commerce
            .gift_cards()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get gift card: {}", e)))?;
        Ok(card.map(Into::into))
    }

    #[napi]
    pub async fn get_by_code(&self, code: String) -> Result<Option<GiftCardOutput>> {
        let commerce = self.commerce.lock().await;
        let card = commerce
            .gift_cards()
            .get_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get gift card by code: {}", e)))?;
        Ok(card.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateGiftCardInput) -> Result<GiftCardOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let status = match input.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::GiftCardStatus>()
                    .map_err(|_| Error::from_reason("Invalid gift card status"))?,
            ),
            None => None,
        };
        let card = commerce
            .gift_cards()
            .update(
                uuid.into(),
                stateset_core::UpdateGiftCard {
                    status,
                    recipient_email: input.recipient_email,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update gift card: {}", e)))?;
        Ok(card.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<GiftCardFilterInput>) -> Result<Vec<GiftCardOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or(GiftCardFilterInput {
            status: None,
            code: None,
            limit: None,
            offset: None,
        });
        let status = match filter.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::GiftCardStatus>()
                    .map_err(|_| Error::from_reason("Invalid gift card status"))?,
            ),
            None => None,
        };
        let cards = commerce
            .gift_cards()
            .list(stateset_core::GiftCardFilter {
                status,
                code: filter.code,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list gift cards: {}", e)))?;
        Ok(cards.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn charge(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransactionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let amount =
            amount.parse::<Decimal>().map_err(|_| Error::from_reason("Invalid amount decimal"))?;
        let txn = commerce
            .gift_cards()
            .charge(uuid.into(), amount, reference_id)
            .map_err(|e| Error::from_reason(format!("Failed to charge gift card: {}", e)))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn refund(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransactionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let amount =
            amount.parse::<Decimal>().map_err(|_| Error::from_reason("Invalid amount decimal"))?;
        let txn = commerce
            .gift_cards()
            .refund(uuid.into(), amount, reference_id)
            .map_err(|e| Error::from_reason(format!("Failed to refund gift card: {}", e)))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn disable(&self, id: String) -> Result<GiftCardOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let card = commerce
            .gift_cards()
            .disable(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to disable gift card: {}", e)))?;
        Ok(card.into())
    }

    #[napi]
    pub async fn get_transactions(
        &self,
        gift_card_id: String,
    ) -> Result<Vec<GiftCardTransactionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            gift_card_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let txns = commerce
            .gift_cards()
            .get_transactions(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get transactions: {}", e)))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Store credits  (all monetary values cross as exact decimal strings)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateStoreCreditInput {
    /// Customer UUID that owns the credit
    pub customer_id: String,
    /// Amount to issue as an exact decimal string, e.g. "25.00"
    pub amount: String,
    /// Currency code, e.g. "USD"
    pub currency: String,
    /// Reason: return, loyalty, compensation, promotion, manual, gift_card
    /// (defaults to "return")
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub note: Option<String>,
    /// RFC 3339 expiry timestamp (None = never expires)
    pub expires_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AdjustStoreCreditInput {
    /// Signed adjustment as an exact decimal string ("10.00" adds, "-10.00"
    /// subtracts). The balance may not be driven below zero.
    pub amount: String,
    pub note: Option<String>,
    pub reference_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCreditFilterInput {
    pub customer_id: Option<String>,
    pub status: Option<String>,
    pub reason: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCreditOutput {
    pub id: String,
    pub customer_id: String,
    /// Exact decimal string
    pub original_balance: String,
    /// Exact decimal string
    pub current_balance: String,
    pub currency: String,
    pub status: String,
    pub reason: String,
    pub reference_id: Option<String>,
    pub note: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::StoreCredit> for StoreCreditOutput {
    fn from(c: stateset_core::StoreCredit) -> Self {
        Self {
            id: c.id.to_string(),
            customer_id: c.customer_id.to_string(),
            original_balance: c.original_balance.to_string(),
            current_balance: c.current_balance.to_string(),
            currency: c.currency.to_string(),
            status: format!("{}", c.status),
            reason: format!("{}", c.reason),
            reference_id: c.reference_id,
            note: c.note,
            expires_at: c.expires_at.map(|d| d.to_rfc3339()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCreditTransactionOutput {
    pub id: String,
    pub store_credit_id: String,
    /// Exact decimal string (positive = credit, negative = debit)
    pub amount: String,
    /// Exact decimal string
    pub balance_after: String,
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::StoreCreditTransaction> for StoreCreditTransactionOutput {
    fn from(t: stateset_core::StoreCreditTransaction) -> Self {
        Self {
            id: t.id.to_string(),
            store_credit_id: t.store_credit_id.to_string(),
            amount: t.amount.to_string(),
            balance_after: t.balance_after.to_string(),
            transaction_type: format!("{}", t.transaction_type),
            reference_id: t.reference_id,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct StoreCredits {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl StoreCredits {
    /// Whether the store-credits backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.store_credits().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateStoreCreditInput) -> Result<StoreCreditOutput> {
        let commerce = self.commerce.lock().await;
        let customer_uuid: uuid::Uuid =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let amount = input
            .amount
            .parse::<Decimal>()
            .map_err(|_| Error::from_reason("Invalid amount decimal"))?;
        let currency = input
            .currency
            .parse::<CurrencyCode>()
            .map_err(|_| Error::from_reason("Invalid currency code"))?;
        let reason = match input.reason.as_deref() {
            Some(s) => s
                .parse::<stateset_core::StoreCreditReason>()
                .map_err(|_| Error::from_reason("Invalid store credit reason"))?,
            None => stateset_core::StoreCreditReason::default(),
        };
        let expires_at = match input.expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| Error::from_reason("Invalid expires_at RFC 3339 timestamp"))?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let credit = commerce
            .store_credits()
            .create(stateset_core::CreateStoreCredit {
                customer_id: customer_uuid.into(),
                amount,
                currency,
                reason,
                reference_id: input.reference_id,
                note: input.note,
                expires_at,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create store credit: {}", e)))?;
        Ok(credit.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<StoreCreditOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let credit = commerce
            .store_credits()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get store credit: {}", e)))?;
        Ok(credit.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<StoreCreditFilterInput>,
    ) -> Result<Vec<StoreCreditOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or(StoreCreditFilterInput {
            customer_id: None,
            status: None,
            reason: None,
            limit: None,
            offset: None,
        });
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let status = match filter.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::StoreCreditStatus>()
                    .map_err(|_| Error::from_reason("Invalid store credit status"))?,
            ),
            None => None,
        };
        let reason = match filter.reason.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::StoreCreditReason>()
                    .map_err(|_| Error::from_reason("Invalid store credit reason"))?,
            ),
            None => None,
        };
        let credits = commerce
            .store_credits()
            .list(stateset_core::StoreCreditFilter {
                customer_id,
                status,
                reason,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list store credits: {}", e)))?;
        Ok(credits.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn adjust(
        &self,
        id: String,
        input: AdjustStoreCreditInput,
    ) -> Result<StoreCreditOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let amount = input
            .amount
            .parse::<Decimal>()
            .map_err(|_| Error::from_reason("Invalid amount decimal"))?;
        let credit = commerce
            .store_credits()
            .adjust(
                uuid.into(),
                stateset_core::AdjustStoreCredit {
                    amount,
                    note: input.note,
                    reference_id: input.reference_id,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to adjust store credit: {}", e)))?;
        Ok(credit.into())
    }

    /// Apply (redeem) an amount from the credit, returning the ledger transaction.
    #[napi]
    pub async fn apply(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransactionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let amount =
            amount.parse::<Decimal>().map_err(|_| Error::from_reason("Invalid amount decimal"))?;
        let txn = commerce
            .store_credits()
            .apply(uuid.into(), amount, reference_id)
            .map_err(|e| Error::from_reason(format!("Failed to apply store credit: {}", e)))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn get_transactions(
        &self,
        store_credit_id: String,
    ) -> Result<Vec<StoreCreditTransactionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            store_credit_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let txns = commerce
            .store_credits()
            .get_transactions(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get transactions: {}", e)))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Product reviews
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReviewInput {
    pub product_id: String,
    pub customer_id: String,
    /// Star rating 1–5
    pub rating: u32,
    pub title: Option<String>,
    pub body: Option<String>,
    pub verified_purchase: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateReviewInput {
    pub rating: Option<u32>,
    pub title: Option<String>,
    pub body: Option<String>,
    /// Moderation status: pending, approved, rejected, flagged
    pub status: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ReviewFilterInput {
    pub product_id: Option<String>,
    pub customer_id: Option<String>,
    pub status: Option<String>,
    pub min_rating: Option<u32>,
    pub verified_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewOutput {
    pub id: String,
    pub product_id: String,
    pub customer_id: String,
    pub rating: u32,
    pub title: Option<String>,
    pub body: Option<String>,
    pub status: String,
    pub verified_purchase: bool,
    pub helpful_count: u32,
    pub reported_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Review> for ReviewOutput {
    fn from(r: stateset_core::Review) -> Self {
        Self {
            id: r.id.to_string(),
            product_id: r.product_id.to_string(),
            customer_id: r.customer_id.to_string(),
            rating: u32::from(r.rating),
            title: r.title,
            body: r.body,
            status: format!("{}", r.status),
            verified_purchase: r.verified_purchase,
            helpful_count: r.helpful_count,
            reported_count: r.reported_count,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewSummaryOutput {
    pub product_id: String,
    pub average_rating: f64,
    pub total_reviews: i64,
    /// Counts for 1★, 2★, 3★, 4★, 5★ (index 0 = 1 star)
    pub rating_distribution: Vec<u32>,
}

impl From<stateset_core::ReviewSummary> for ReviewSummaryOutput {
    fn from(s: stateset_core::ReviewSummary) -> Self {
        Self {
            product_id: s.product_id.to_string(),
            average_rating: s.average_rating,
            total_reviews: s.total_reviews as i64,
            rating_distribution: s.rating_distribution.to_vec(),
        }
    }
}

#[napi]
pub struct Reviews {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Reviews {
    /// Whether the reviews backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.reviews().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateReviewInput) -> Result<ReviewOutput> {
        let commerce = self.commerce.lock().await;
        let product_id: uuid::Uuid =
            input.product_id.parse().map_err(|_| Error::from_reason("Invalid product UUID"))?;
        let customer_id: uuid::Uuid =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let rating = u8::try_from(input.rating)
            .map_err(|_| Error::from_reason("rating must be between 1 and 5"))?;
        let review = commerce
            .reviews()
            .create(stateset_core::CreateReview {
                product_id: product_id.into(),
                customer_id: customer_id.into(),
                rating,
                title: input.title,
                body: input.body,
                verified_purchase: input.verified_purchase.unwrap_or(false),
            })
            .map_err(|e| Error::from_reason(format!("Failed to create review: {}", e)))?;
        Ok(review.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ReviewOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let review = commerce
            .reviews()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get review: {}", e)))?;
        Ok(review.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateReviewInput) -> Result<ReviewOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let rating = match input.rating {
            Some(r) => Some(
                u8::try_from(r)
                    .map_err(|_| Error::from_reason("rating must be between 1 and 5"))?,
            ),
            None => None,
        };
        let status = match input.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::ReviewStatus>()
                    .map_err(|_| Error::from_reason("Invalid review status"))?,
            ),
            None => None,
        };
        let review = commerce
            .reviews()
            .update(
                uuid.into(),
                stateset_core::UpdateReview {
                    rating,
                    title: input.title.map(Some),
                    body: input.body.map(Some),
                    status,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update review: {}", e)))?;
        Ok(review.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<ReviewFilterInput>) -> Result<Vec<ReviewOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();
        let product_id = match filter.product_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid product UUID"))?
                    .into(),
            ),
            None => None,
        };
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let status = match filter.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::ReviewStatus>()
                    .map_err(|_| Error::from_reason("Invalid review status"))?,
            ),
            None => None,
        };
        let min_rating = match filter.min_rating {
            Some(r) => {
                Some(u8::try_from(r).map_err(|_| Error::from_reason("min_rating out of range"))?)
            }
            None => None,
        };
        let reviews = commerce
            .reviews()
            .list(stateset_core::ReviewFilter {
                product_id,
                customer_id,
                status,
                min_rating,
                verified_only: filter.verified_only,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list reviews: {}", e)))?;
        Ok(reviews.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        commerce
            .reviews()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete review: {}", e)))?;
        Ok(())
    }

    /// Aggregate rating summary for a product (average, total, star distribution).
    #[napi]
    pub async fn get_summary(&self, product_id: String) -> Result<ReviewSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| Error::from_reason("Invalid product UUID"))?;
        let summary = commerce
            .reviews()
            .get_summary(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get review summary: {}", e)))?;
        Ok(summary.into())
    }

    #[napi]
    pub async fn mark_helpful(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        commerce
            .reviews()
            .mark_helpful(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to mark review helpful: {}", e)))?;
        Ok(())
    }

    #[napi]
    pub async fn mark_reported(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        commerce
            .reviews()
            .mark_reported(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to mark review reported: {}", e)))?;
        Ok(())
    }
}

// ============================================================================
// Wishlists
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWishlistInput {
    pub customer_id: String,
    pub name: String,
    pub is_public: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateWishlistInput {
    pub name: Option<String>,
    pub is_public: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AddWishlistItemInput {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub note: Option<String>,
    pub quantity: Option<u32>,
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WishlistFilterInput {
    pub customer_id: Option<String>,
    pub is_public: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WishlistItemOutput {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub added_at: String,
    pub note: Option<String>,
    pub quantity: u32,
    pub priority: Option<i32>,
}

impl From<stateset_core::WishlistItem> for WishlistItemOutput {
    fn from(i: stateset_core::WishlistItem) -> Self {
        Self {
            product_id: i.product_id.to_string(),
            variant_id: i.variant_id,
            added_at: i.added_at.to_rfc3339(),
            note: i.note,
            quantity: i.quantity,
            priority: i.priority,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WishlistOutput {
    pub id: String,
    pub customer_id: String,
    pub name: String,
    pub is_public: bool,
    pub items: Vec<WishlistItemOutput>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Wishlist> for WishlistOutput {
    fn from(w: stateset_core::Wishlist) -> Self {
        Self {
            id: w.id.to_string(),
            customer_id: w.customer_id.to_string(),
            name: w.name,
            is_public: w.is_public,
            items: w.items.into_iter().map(Into::into).collect(),
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Wishlists {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Wishlists {
    /// Whether the wishlists backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.wishlists().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateWishlistInput) -> Result<WishlistOutput> {
        let commerce = self.commerce.lock().await;
        let customer_id: uuid::Uuid =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let wishlist = commerce
            .wishlists()
            .create(stateset_core::CreateWishlist {
                customer_id: customer_id.into(),
                name: input.name,
                is_public: input.is_public.unwrap_or(false),
            })
            .map_err(|e| Error::from_reason(format!("Failed to create wishlist: {}", e)))?;
        Ok(wishlist.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WishlistOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let wishlist = commerce
            .wishlists()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get wishlist: {}", e)))?;
        Ok(wishlist.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateWishlistInput) -> Result<WishlistOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let wishlist = commerce
            .wishlists()
            .update(
                uuid.into(),
                stateset_core::UpdateWishlist { name: input.name, is_public: input.is_public },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update wishlist: {}", e)))?;
        Ok(wishlist.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<WishlistFilterInput>) -> Result<Vec<WishlistOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let wishlists = commerce
            .wishlists()
            .list(stateset_core::WishlistFilter {
                customer_id,
                is_public: filter.is_public,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list wishlists: {}", e)))?;
        Ok(wishlists.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        commerce
            .wishlists()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete wishlist: {}", e)))?;
        Ok(())
    }

    /// Add a product to a wishlist, returning the added item.
    #[napi]
    pub async fn add_item(
        &self,
        wishlist_id: String,
        item: AddWishlistItemInput,
    ) -> Result<WishlistItemOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            wishlist_id.parse().map_err(|_| Error::from_reason("Invalid wishlist UUID"))?;
        let product_id: uuid::Uuid =
            item.product_id.parse().map_err(|_| Error::from_reason("Invalid product UUID"))?;
        let added = commerce
            .wishlists()
            .add_item(
                uuid.into(),
                stateset_core::AddWishlistItem {
                    product_id: product_id.into(),
                    variant_id: item.variant_id,
                    note: item.note,
                    quantity: item.quantity,
                    priority: item.priority,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to add wishlist item: {}", e)))?;
        Ok(added.into())
    }

    #[napi]
    pub async fn remove_item(&self, wishlist_id: String, product_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            wishlist_id.parse().map_err(|_| Error::from_reason("Invalid wishlist UUID"))?;
        let product_uuid: uuid::Uuid =
            product_id.parse().map_err(|_| Error::from_reason("Invalid product UUID"))?;
        commerce
            .wishlists()
            .remove_item(uuid.into(), product_uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to remove wishlist item: {}", e)))?;
        Ok(())
    }
}

// ============================================================================
// Customer segments
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentRuleInput {
    pub field: String,
    /// One of: eq, neq, gt, gte, lt, lte, contains, in, between, starts_with,
    /// ends_with
    pub operator: String,
    pub value: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentRuleOutput {
    pub field: String,
    pub operator: String,
    pub value: String,
}

impl From<stateset_core::SegmentRule> for SegmentRuleOutput {
    fn from(r: stateset_core::SegmentRule) -> Self {
        Self { field: r.field, operator: format!("{}", r.operator), value: r.value }
    }
}

fn parse_segment_rules(rules: Vec<SegmentRuleInput>) -> Result<Vec<stateset_core::SegmentRule>> {
    rules
        .into_iter()
        .map(|r| {
            Ok(stateset_core::SegmentRule {
                field: r.field,
                operator: r.operator.parse::<stateset_core::SegmentOperator>().map_err(|_| {
                    Error::from_reason(format!("Invalid segment operator '{}'", r.operator))
                })?,
                value: r.value,
            })
        })
        .collect()
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSegmentInput {
    pub name: String,
    pub description: Option<String>,
    /// "static" (default) or "dynamic"
    pub segment_type: Option<String>,
    pub rules: Option<Vec<SegmentRuleInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSegmentInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub rules: Option<Vec<SegmentRuleInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SegmentFilterInput {
    pub segment_type: Option<String>,
    pub name: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub segment_type: String,
    pub rules: Vec<SegmentRuleOutput>,
    pub member_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Segment> for SegmentOutput {
    fn from(s: stateset_core::Segment) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            description: s.description,
            segment_type: format!("{}", s.segment_type),
            rules: s.rules.into_iter().map(Into::into).collect(),
            member_count: s.member_count as i64,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SegmentMembershipOutput {
    pub segment_id: String,
    pub customer_id: String,
    pub joined_at: String,
}

impl From<stateset_core::SegmentMembership> for SegmentMembershipOutput {
    fn from(m: stateset_core::SegmentMembership) -> Self {
        Self {
            segment_id: m.segment_id.to_string(),
            customer_id: m.customer_id.to_string(),
            joined_at: m.joined_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Segments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Segments {
    /// Whether the segments backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.segments().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateSegmentInput) -> Result<SegmentOutput> {
        let commerce = self.commerce.lock().await;
        let segment_type = match input.segment_type.as_deref() {
            Some(s) => s
                .parse::<stateset_core::SegmentType>()
                .map_err(|_| Error::from_reason("Invalid segment_type (use static or dynamic)"))?,
            None => stateset_core::SegmentType::default(),
        };
        let rules = parse_segment_rules(input.rules.unwrap_or_default())?;
        let segment = commerce
            .segments()
            .create(stateset_core::CreateSegment {
                name: input.name,
                description: input.description,
                segment_type,
                rules,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create segment: {}", e)))?;
        Ok(segment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SegmentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let segment = commerce
            .segments()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get segment: {}", e)))?;
        Ok(segment.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateSegmentInput) -> Result<SegmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let rules = match input.rules {
            Some(r) => Some(parse_segment_rules(r)?),
            None => None,
        };
        let segment = commerce
            .segments()
            .update(
                uuid.into(),
                stateset_core::UpdateSegment {
                    name: input.name,
                    description: input.description.map(Some),
                    rules,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update segment: {}", e)))?;
        Ok(segment.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<SegmentFilterInput>) -> Result<Vec<SegmentOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();
        let segment_type = match filter.segment_type.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::SegmentType>()
                    .map_err(|_| Error::from_reason("Invalid segment_type"))?,
            ),
            None => None,
        };
        let segments = commerce
            .segments()
            .list(stateset_core::SegmentFilter {
                segment_type,
                name: filter.name,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list segments: {}", e)))?;
        Ok(segments.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        commerce
            .segments()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete segment: {}", e)))?;
        Ok(())
    }

    /// Add a customer to a (static) segment, returning the membership record.
    #[napi]
    pub async fn add_member(
        &self,
        segment_id: String,
        customer_id: String,
    ) -> Result<SegmentMembershipOutput> {
        let commerce = self.commerce.lock().await;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| Error::from_reason("Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let membership = commerce
            .segments()
            .add_member(seg.into(), cust.into())
            .map_err(|e| Error::from_reason(format!("Failed to add segment member: {}", e)))?;
        Ok(membership.into())
    }

    #[napi]
    pub async fn remove_member(&self, segment_id: String, customer_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| Error::from_reason("Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        commerce
            .segments()
            .remove_member(seg.into(), cust.into())
            .map_err(|e| Error::from_reason(format!("Failed to remove segment member: {}", e)))?;
        Ok(())
    }

    #[napi]
    pub async fn list_members(
        &self,
        segment_id: String,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembershipOutput>> {
        let commerce = self.commerce.lock().await;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| Error::from_reason("Invalid segment UUID"))?;
        let members = commerce
            .segments()
            .list_members(seg.into(), limit, offset)
            .map_err(|e| Error::from_reason(format!("Failed to list segment members: {}", e)))?;
        Ok(members.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn is_member(&self, segment_id: String, customer_id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| Error::from_reason("Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        commerce
            .segments()
            .is_member(seg.into(), cust.into())
            .map_err(|e| Error::from_reason(format!("Failed to check segment membership: {}", e)))
    }
}

// ============================================================================
// Loyalty  (points are integers; reward `value` is an exact decimal string)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyTierInput {
    pub name: String,
    pub min_points: i64,
    pub multiplier: f64,
    pub perks: Vec<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyTierOutput {
    pub name: String,
    pub min_points: i64,
    pub multiplier: f64,
    pub perks: Vec<String>,
}

impl From<stateset_core::LoyaltyTier> for LoyaltyTierOutput {
    fn from(t: stateset_core::LoyaltyTier) -> Self {
        Self {
            name: t.name,
            min_points: t.min_points as i64,
            multiplier: t.multiplier,
            perks: t.perks,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateLoyaltyProgramInput {
    pub name: String,
    pub description: Option<String>,
    pub points_per_dollar: u32,
    pub tiers: Option<Vec<LoyaltyTierInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyProgramOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub points_per_dollar: u32,
    pub tiers: Vec<LoyaltyTierOutput>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::LoyaltyProgram> for LoyaltyProgramOutput {
    fn from(p: stateset_core::LoyaltyProgram) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            points_per_dollar: p.points_per_dollar,
            tiers: p.tiers.into_iter().map(Into::into).collect(),
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EnrollCustomerInput {
    pub customer_id: String,
    pub program_id: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyAccountOutput {
    pub id: String,
    pub customer_id: String,
    pub program_id: String,
    pub points_balance: i64,
    pub lifetime_points: i64,
    pub tier: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::LoyaltyAccount> for LoyaltyAccountOutput {
    fn from(a: stateset_core::LoyaltyAccount) -> Self {
        Self {
            id: a.id.to_string(),
            customer_id: a.customer_id.to_string(),
            program_id: a.program_id.to_string(),
            points_balance: a.points_balance,
            lifetime_points: a.lifetime_points as i64,
            tier: a.tier,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AdjustPointsInput {
    pub account_id: String,
    pub points: i64,
    /// Transaction type, e.g. "earn", "redeem", "adjust", "expire"
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyTransactionOutput {
    pub id: String,
    pub account_id: String,
    pub points: i64,
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::LoyaltyTransaction> for LoyaltyTransactionOutput {
    fn from(t: stateset_core::LoyaltyTransaction) -> Self {
        Self {
            id: t.id.to_string(),
            account_id: t.account_id.to_string(),
            points: t.points,
            transaction_type: format!("{}", t.transaction_type),
            reference_id: t.reference_id,
            description: t.description,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRewardInput {
    pub program_id: String,
    pub name: String,
    pub description: Option<String>,
    pub points_cost: i64,
    /// Reward type, e.g. "discount", "free_product", "free_shipping"
    pub reward_type: String,
    /// Monetary value as an exact decimal string (optional)
    pub value: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RewardOutput {
    pub id: String,
    pub program_id: String,
    pub name: String,
    pub description: Option<String>,
    pub points_cost: i64,
    pub reward_type: String,
    pub value: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Reward> for RewardOutput {
    fn from(r: stateset_core::Reward) -> Self {
        Self {
            id: r.id.to_string(),
            program_id: r.program_id.to_string(),
            name: r.name,
            description: r.description,
            points_cost: r.points_cost as i64,
            reward_type: format!("{}", r.reward_type),
            value: r.value.map(|v| v.to_string()),
            is_active: r.is_active,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyAccountFilterInput {
    pub customer_id: Option<String>,
    pub program_id: Option<String>,
    pub tier: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RewardFilterInput {
    pub program_id: Option<String>,
    pub reward_type: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi]
pub struct Loyalty {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Loyalty {
    /// Whether the loyalty backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.loyalty().is_supported())
    }

    #[napi]
    pub async fn create_program(
        &self,
        input: CreateLoyaltyProgramInput,
    ) -> Result<LoyaltyProgramOutput> {
        let commerce = self.commerce.lock().await;
        let tiers = input
            .tiers
            .unwrap_or_default()
            .into_iter()
            .map(|t| stateset_core::LoyaltyTier {
                name: t.name,
                min_points: t.min_points.max(0) as u64,
                multiplier: t.multiplier,
                perks: t.perks,
            })
            .collect();
        let program = commerce
            .loyalty()
            .create_program(stateset_core::CreateLoyaltyProgram {
                name: input.name,
                description: input.description,
                points_per_dollar: input.points_per_dollar,
                tiers,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create loyalty program: {}", e)))?;
        Ok(program.into())
    }

    #[napi]
    pub async fn get_program(&self, id: String) -> Result<Option<LoyaltyProgramOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let program = commerce
            .loyalty()
            .get_program(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get loyalty program: {}", e)))?;
        Ok(program.map(Into::into))
    }

    #[napi]
    pub async fn list_programs(&self) -> Result<Vec<LoyaltyProgramOutput>> {
        let commerce = self.commerce.lock().await;
        let programs = commerce
            .loyalty()
            .list_programs()
            .map_err(|e| Error::from_reason(format!("Failed to list loyalty programs: {}", e)))?;
        Ok(programs.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn enroll(&self, input: EnrollCustomerInput) -> Result<LoyaltyAccountOutput> {
        let commerce = self.commerce.lock().await;
        let customer_id: uuid::Uuid =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let program_id: uuid::Uuid =
            input.program_id.parse().map_err(|_| Error::from_reason("Invalid program UUID"))?;
        let account = commerce
            .loyalty()
            .enroll(stateset_core::EnrollCustomer {
                customer_id: customer_id.into(),
                program_id: program_id.into(),
            })
            .map_err(|e| Error::from_reason(format!("Failed to enroll customer: {}", e)))?;
        Ok(account.into())
    }

    #[napi]
    pub async fn get_account(&self, id: String) -> Result<Option<LoyaltyAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let account = commerce
            .loyalty()
            .get_account(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get loyalty account: {}", e)))?;
        Ok(account.map(Into::into))
    }

    #[napi]
    pub async fn get_account_by_customer(
        &self,
        customer_id: String,
        program_id: String,
    ) -> Result<Option<LoyaltyAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let customer_id: uuid::Uuid =
            customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let program_id: uuid::Uuid =
            program_id.parse().map_err(|_| Error::from_reason("Invalid program UUID"))?;
        let account = commerce
            .loyalty()
            .get_account_by_customer(customer_id.into(), program_id.into())
            .map_err(|e| Error::from_reason(format!("Failed to get loyalty account: {}", e)))?;
        Ok(account.map(Into::into))
    }

    #[napi]
    pub async fn list_accounts(
        &self,
        filter: Option<LoyaltyAccountFilterInput>,
    ) -> Result<Vec<LoyaltyAccountOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or(LoyaltyAccountFilterInput {
            customer_id: None,
            program_id: None,
            tier: None,
            limit: None,
            offset: None,
        });
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let program_id = match filter.program_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid program UUID"))?
                    .into(),
            ),
            None => None,
        };
        let accounts = commerce
            .loyalty()
            .list_accounts(stateset_core::LoyaltyAccountFilter {
                customer_id,
                program_id,
                tier: filter.tier,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list loyalty accounts: {}", e)))?;
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn adjust_points(
        &self,
        input: AdjustPointsInput,
    ) -> Result<LoyaltyTransactionOutput> {
        let commerce = self.commerce.lock().await;
        let account_id: uuid::Uuid =
            input.account_id.parse().map_err(|_| Error::from_reason("Invalid account UUID"))?;
        let transaction_type = input
            .transaction_type
            .parse::<stateset_core::LoyaltyTransactionType>()
            .map_err(|_| Error::from_reason("Invalid transaction_type"))?;
        let txn = commerce
            .loyalty()
            .adjust_points(stateset_core::AdjustPoints {
                account_id: account_id.into(),
                points: input.points,
                transaction_type,
                reference_id: input.reference_id,
                description: input.description,
            })
            .map_err(|e| Error::from_reason(format!("Failed to adjust points: {}", e)))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn get_transactions(
        &self,
        account_id: String,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransactionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            account_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let txns = commerce
            .loyalty()
            .get_transactions(uuid.into(), limit)
            .map_err(|e| Error::from_reason(format!("Failed to get transactions: {}", e)))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn create_reward(&self, input: CreateRewardInput) -> Result<RewardOutput> {
        let commerce = self.commerce.lock().await;
        let program_id: uuid::Uuid =
            input.program_id.parse().map_err(|_| Error::from_reason("Invalid program UUID"))?;
        let reward_type = input
            .reward_type
            .parse::<stateset_core::RewardType>()
            .map_err(|_| Error::from_reason("Invalid reward_type"))?;
        let value = match input.value.as_deref() {
            Some(s) => Some(
                s.parse::<Decimal>().map_err(|_| Error::from_reason("Invalid value decimal"))?,
            ),
            None => None,
        };
        let reward = commerce
            .loyalty()
            .create_reward(stateset_core::CreateReward {
                program_id: program_id.into(),
                name: input.name,
                description: input.description,
                points_cost: input.points_cost.max(0) as u64,
                reward_type,
                value,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create reward: {}", e)))?;
        Ok(reward.into())
    }

    #[napi]
    pub async fn get_reward(&self, id: String) -> Result<Option<RewardOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let reward = commerce
            .loyalty()
            .get_reward(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get reward: {}", e)))?;
        Ok(reward.map(Into::into))
    }

    #[napi]
    pub async fn list_rewards(
        &self,
        filter: Option<RewardFilterInput>,
    ) -> Result<Vec<RewardOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or(RewardFilterInput {
            program_id: None,
            reward_type: None,
            is_active: None,
            limit: None,
            offset: None,
        });
        let program_id = match filter.program_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| Error::from_reason("Invalid program UUID"))?
                    .into(),
            ),
            None => None,
        };
        let reward_type = match filter.reward_type.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::RewardType>()
                    .map_err(|_| Error::from_reason("Invalid reward_type"))?,
            ),
            None => None,
        };
        let rewards = commerce
            .loyalty()
            .list_rewards(stateset_core::RewardFilter {
                program_id,
                reward_type,
                is_active: filter.is_active,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list rewards: {}", e)))?;
        Ok(rewards.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_reward(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        commerce
            .loyalty()
            .delete_reward(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete reward: {}", e)))?;
        Ok(())
    }
}

// ============================================================================
// Fixed Assets  (all monetary values cross as exact decimal strings)
// ============================================================================

fn parse_iso_date(s: &str, field: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| Error::from_reason(format!("Invalid {field} date (expected YYYY-MM-DD)")))
}

fn parse_decimal_str(s: &str, field: &str) -> Result<Decimal> {
    s.parse::<Decimal>().map_err(|_| Error::from_reason(format!("Invalid {field} decimal")))
}

fn parse_optional_uuid(s: Option<String>, field: &str) -> Result<Option<uuid::Uuid>> {
    s.map(|s| {
        s.parse::<uuid::Uuid>().map_err(|_| Error::from_reason(format!("Invalid {field} UUID")))
    })
    .transpose()
}

fn parse_depreciation_method(
    method: &str,
    rate: Option<&str>,
) -> Result<stateset_core::DepreciationMethod> {
    match method {
        "straight_line" => Ok(stateset_core::DepreciationMethod::StraightLine),
        "declining_balance" => {
            let rate = rate.ok_or_else(|| {
                Error::from_reason("declining_balance requires declining_balance_rate")
            })?;
            Ok(stateset_core::DepreciationMethod::DecliningBalance {
                rate: parse_decimal_str(rate, "declining_balance_rate")?,
            })
        }
        "units_of_production" => Ok(stateset_core::DepreciationMethod::UnitsOfProduction),
        _ => Err(Error::from_reason(
            "Invalid depreciation method (expected straight_line, declining_balance, or units_of_production)",
        )),
    }
}

fn depreciation_method_parts(
    method: stateset_core::DepreciationMethod,
) -> (String, Option<String>) {
    match method {
        stateset_core::DepreciationMethod::StraightLine => ("straight_line".to_string(), None),
        stateset_core::DepreciationMethod::DecliningBalance { rate } => {
            ("declining_balance".to_string(), Some(rate.to_string()))
        }
        stateset_core::DepreciationMethod::UnitsOfProduction => {
            ("units_of_production".to_string(), None)
        }
        _ => ("unknown".to_string(), None),
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFixedAssetInput {
    /// Optional asset number; auto-generated when omitted (FA-...)
    pub asset_number: Option<String>,
    pub name: String,
    pub description: Option<String>,
    /// Category: land, building, machinery, equipment, vehicle,
    /// furniture_and_fixtures, computer_hardware, software,
    /// leasehold_improvement, other
    pub category: String,
    /// ISO date (YYYY-MM-DD)
    pub acquisition_date: String,
    /// Exact decimal string, e.g. "10000.00"
    pub acquisition_cost: String,
    /// Exact decimal string
    pub salvage_value: String,
    pub useful_life_months: u32,
    /// straight_line, declining_balance, units_of_production
    pub depreciation_method: String,
    /// Required for declining_balance: periodic rate as exact decimal string
    /// strictly between 0 and 1 (e.g. "0.2" for 20%)
    pub declining_balance_rate: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
    pub asset_account_id: Option<String>,
    pub accumulated_depreciation_account_id: Option<String>,
    pub depreciation_expense_account_id: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateFixedAssetInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    /// Exact decimal string
    pub salvage_value: Option<String>,
    pub useful_life_months: Option<u32>,
    /// ISO date (YYYY-MM-DD)
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
    pub asset_account_id: Option<String>,
    pub accumulated_depreciation_account_id: Option<String>,
    pub depreciation_expense_account_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FixedAssetFilterInput {
    pub category: Option<String>,
    /// draft, in_service, fully_depreciated, disposed, written_off
    pub status: Option<String>,
    pub location_id: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub acquired_from: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub acquired_to: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AssetDisposalOutput {
    /// ISO date (YYYY-MM-DD)
    pub disposal_date: String,
    /// Exact decimal string
    pub proceeds: String,
    /// Exact decimal string
    pub book_value_at_disposal: String,
    /// Exact decimal string: proceeds - book value
    pub gain_loss: String,
    pub notes: Option<String>,
}

impl From<stateset_core::AssetDisposal> for AssetDisposalOutput {
    fn from(d: stateset_core::AssetDisposal) -> Self {
        Self {
            disposal_date: d.disposal_date.to_string(),
            proceeds: d.proceeds.to_string(),
            book_value_at_disposal: d.book_value_at_disposal.to_string(),
            gain_loss: d.gain_loss.to_string(),
            notes: d.notes,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FixedAssetOutput {
    pub id: String,
    pub asset_number: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    /// ISO date (YYYY-MM-DD)
    pub acquisition_date: String,
    /// Exact decimal string
    pub acquisition_cost: String,
    /// Exact decimal string
    pub salvage_value: String,
    pub useful_life_months: u32,
    /// straight_line, declining_balance, units_of_production
    pub depreciation_method: String,
    /// Set when depreciation_method is declining_balance
    pub declining_balance_rate: Option<String>,
    /// draft, in_service, fully_depreciated, disposed, written_off
    pub status: String,
    /// ISO date (YYYY-MM-DD)
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
    pub asset_account_id: Option<String>,
    pub accumulated_depreciation_account_id: Option<String>,
    pub depreciation_expense_account_id: Option<String>,
    /// Exact decimal string
    pub accumulated_depreciation: String,
    /// Exact decimal string: acquisition_cost - accumulated_depreciation
    pub book_value: String,
    pub currency: String,
    pub disposal: Option<AssetDisposalOutput>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::FixedAsset> for FixedAssetOutput {
    fn from(a: stateset_core::FixedAsset) -> Self {
        let book_value = a.book_value();
        let (method, rate) = depreciation_method_parts(a.depreciation_method);
        Self {
            id: a.id.to_string(),
            asset_number: a.asset_number,
            name: a.name,
            description: a.description,
            category: format!("{}", a.category),
            acquisition_date: a.acquisition_date.to_string(),
            acquisition_cost: a.acquisition_cost.to_string(),
            salvage_value: a.salvage_value.to_string(),
            useful_life_months: a.useful_life_months,
            depreciation_method: method,
            declining_balance_rate: rate,
            status: format!("{}", a.status),
            in_service_date: a.in_service_date.map(|d| d.to_string()),
            location_id: a.location_id.map(|id| id.to_string()),
            asset_account_id: a.asset_account_id.map(|id| id.to_string()),
            accumulated_depreciation_account_id: a
                .accumulated_depreciation_account_id
                .map(|id| id.to_string()),
            depreciation_expense_account_id: a
                .depreciation_expense_account_id
                .map(|id| id.to_string()),
            accumulated_depreciation: a.accumulated_depreciation.to_string(),
            book_value: book_value.to_string(),
            currency: a.currency.to_string(),
            disposal: a.disposal.map(Into::into),
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct DepreciationEntryOutput {
    pub period: u32,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub accumulated: String,
    /// Exact decimal string
    pub book_value: String,
    /// scheduled or posted
    pub status: String,
}

impl From<stateset_core::DepreciationEntry> for DepreciationEntryOutput {
    fn from(e: stateset_core::DepreciationEntry) -> Self {
        Self {
            period: e.period,
            amount: e.amount.to_string(),
            accumulated: e.accumulated.to_string(),
            book_value: e.book_value.to_string(),
            status: format!("{}", e.status),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct DepreciationScheduleOutput {
    pub asset_id: String,
    /// straight_line, declining_balance, units_of_production
    pub method: String,
    /// Set when method is declining_balance
    pub declining_balance_rate: Option<String>,
    pub entries: Vec<DepreciationEntryOutput>,
    /// Exact decimal string
    pub total_depreciation: String,
}

impl From<stateset_core::DepreciationSchedule> for DepreciationScheduleOutput {
    fn from(s: stateset_core::DepreciationSchedule) -> Self {
        let (method, rate) = depreciation_method_parts(s.method);
        Self {
            asset_id: s.asset_id.to_string(),
            method,
            declining_balance_rate: rate,
            entries: s.entries.into_iter().map(Into::into).collect(),
            total_depreciation: s.total_depreciation.to_string(),
        }
    }
}

#[napi]
pub struct FixedAssets {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl FixedAssets {
    /// Whether the fixed-assets backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.fixed_assets().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateFixedAssetInput) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.lock().await;
        let category = input
            .category
            .parse::<stateset_core::FixedAssetCategory>()
            .map_err(|_| Error::from_reason("Invalid fixed asset category"))?;
        let depreciation_method = parse_depreciation_method(
            &input.depreciation_method,
            input.declining_balance_rate.as_deref(),
        )?;
        let currency = input
            .currency
            .map(|s| {
                s.parse::<CurrencyCode>().map_err(|_| Error::from_reason("Invalid currency code"))
            })
            .transpose()?;
        let asset = commerce
            .fixed_assets()
            .create(stateset_core::CreateFixedAsset {
                asset_number: input.asset_number,
                name: input.name,
                description: input.description,
                category,
                acquisition_date: parse_iso_date(&input.acquisition_date, "acquisition_date")?,
                acquisition_cost: parse_decimal_str(&input.acquisition_cost, "acquisition_cost")?,
                salvage_value: parse_decimal_str(&input.salvage_value, "salvage_value")?,
                useful_life_months: input.useful_life_months,
                depreciation_method,
                in_service_date: input
                    .in_service_date
                    .as_deref()
                    .map(|s| parse_iso_date(s, "in_service_date"))
                    .transpose()?,
                location_id: parse_optional_uuid(input.location_id, "location_id")?,
                asset_account_id: parse_optional_uuid(input.asset_account_id, "asset_account_id")?,
                accumulated_depreciation_account_id: parse_optional_uuid(
                    input.accumulated_depreciation_account_id,
                    "accumulated_depreciation_account_id",
                )?,
                depreciation_expense_account_id: parse_optional_uuid(
                    input.depreciation_expense_account_id,
                    "depreciation_expense_account_id",
                )?,
                currency,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create fixed asset: {}", e)))?;
        Ok(asset.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<FixedAssetOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let asset = commerce
            .fixed_assets()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get fixed asset: {}", e)))?;
        Ok(asset.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<FixedAssetFilterInput>,
    ) -> Result<Vec<FixedAssetOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::FixedAssetFilter::default()),
            |f| -> Result<stateset_core::FixedAssetFilter> {
                Ok(stateset_core::FixedAssetFilter {
                    category: f
                        .category
                        .map(|s| {
                            s.parse::<stateset_core::FixedAssetCategory>()
                                .map_err(|_| Error::from_reason("Invalid fixed asset category"))
                        })
                        .transpose()?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::FixedAssetStatus>()
                                .map_err(|_| Error::from_reason("Invalid fixed asset status"))
                        })
                        .transpose()?,
                    location_id: parse_optional_uuid(f.location_id, "location_id")?,
                    acquired_from: f
                        .acquired_from
                        .as_deref()
                        .map(|s| parse_iso_date(s, "acquired_from"))
                        .transpose()?,
                    acquired_to: f
                        .acquired_to
                        .as_deref()
                        .map(|s| parse_iso_date(s, "acquired_to"))
                        .transpose()?,
                    search: f.search,
                    limit: f.limit,
                    offset: f.offset,
                    after_cursor: None,
                })
            },
        )?;
        let assets = commerce
            .fixed_assets()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list fixed assets: {}", e)))?;
        Ok(assets.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateFixedAssetInput,
    ) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let category = input
            .category
            .map(|s| {
                s.parse::<stateset_core::FixedAssetCategory>()
                    .map_err(|_| Error::from_reason("Invalid fixed asset category"))
            })
            .transpose()?;
        let asset = commerce
            .fixed_assets()
            .update(
                uuid,
                stateset_core::UpdateFixedAsset {
                    name: input.name,
                    description: input.description,
                    category,
                    salvage_value: input
                        .salvage_value
                        .as_deref()
                        .map(|s| parse_decimal_str(s, "salvage_value"))
                        .transpose()?,
                    useful_life_months: input.useful_life_months,
                    in_service_date: input
                        .in_service_date
                        .as_deref()
                        .map(|s| parse_iso_date(s, "in_service_date"))
                        .transpose()?,
                    location_id: parse_optional_uuid(input.location_id, "location_id")?,
                    asset_account_id: parse_optional_uuid(
                        input.asset_account_id,
                        "asset_account_id",
                    )?,
                    accumulated_depreciation_account_id: parse_optional_uuid(
                        input.accumulated_depreciation_account_id,
                        "accumulated_depreciation_account_id",
                    )?,
                    depreciation_expense_account_id: parse_optional_uuid(
                        input.depreciation_expense_account_id,
                        "depreciation_expense_account_id",
                    )?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update fixed asset: {}", e)))?;
        Ok(asset.into())
    }

    /// Place a draft asset in service on the given ISO date (YYYY-MM-DD).
    #[napi]
    pub async fn place_in_service(&self, id: String, date: String) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let date = parse_iso_date(&date, "date")?;
        let asset = commerce
            .fixed_assets()
            .place_in_service(uuid, date)
            .map_err(|e| Error::from_reason(format!("Failed to place asset in service: {}", e)))?;
        Ok(asset.into())
    }

    /// Dispose of an asset for the given proceeds (exact decimal string),
    /// recording gain/loss. `date` is an ISO date (YYYY-MM-DD); defaults to today.
    #[napi]
    pub async fn dispose(
        &self,
        id: String,
        proceeds: String,
        date: Option<String>,
        notes: Option<String>,
    ) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let proceeds = parse_decimal_str(&proceeds, "proceeds")?;
        let date = date
            .as_deref()
            .map(|s| parse_iso_date(s, "date"))
            .transpose()?
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let asset = commerce
            .fixed_assets()
            .dispose(uuid, date, proceeds, notes)
            .map_err(|e| Error::from_reason(format!("Failed to dispose fixed asset: {}", e)))?;
        Ok(asset.into())
    }

    /// Write off an asset (disposal with zero proceeds). `date` is an ISO date
    /// (YYYY-MM-DD); defaults to today.
    #[napi]
    pub async fn write_off(
        &self,
        id: String,
        date: Option<String>,
        notes: Option<String>,
    ) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let date = date
            .as_deref()
            .map(|s| parse_iso_date(s, "date"))
            .transpose()?
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let asset = commerce
            .fixed_assets()
            .write_off(uuid, date, notes)
            .map_err(|e| Error::from_reason(format!("Failed to write off fixed asset: {}", e)))?;
        Ok(asset.into())
    }

    /// Generate and persist the depreciation schedule for an asset.
    #[napi]
    pub async fn generate_schedule(&self, id: String) -> Result<DepreciationScheduleOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let schedule = commerce
            .fixed_assets()
            .generate_schedule(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to generate schedule: {}", e)))?;
        Ok(schedule.into())
    }

    /// Get the persisted depreciation schedule for an asset, if generated.
    #[napi]
    pub async fn get_schedule(&self, id: String) -> Result<Option<DepreciationScheduleOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let schedule = commerce
            .fixed_assets()
            .get_schedule(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get schedule: {}", e)))?;
        Ok(schedule.map(Into::into))
    }

    /// Post the next `periods` scheduled depreciation entries.
    #[napi]
    pub async fn post_depreciation(&self, id: String, periods: u32) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let asset = commerce
            .fixed_assets()
            .post_depreciation(uuid, periods)
            .map_err(|e| Error::from_reason(format!("Failed to post depreciation: {}", e)))?;
        Ok(asset.into())
    }
}

// ============================================================================
// Revenue Recognition  (all monetary values cross as exact decimal strings)
// ============================================================================

fn parse_recognition_method(
    method: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> Result<stateset_core::RecognitionMethod> {
    match method {
        "point_in_time" => Ok(stateset_core::RecognitionMethod::PointInTime),
        "ratable_over_time" => {
            let start = start.ok_or_else(|| {
                Error::from_reason("ratable_over_time requires recognition_start")
            })?;
            let end = end
                .ok_or_else(|| Error::from_reason("ratable_over_time requires recognition_end"))?;
            Ok(stateset_core::RecognitionMethod::RatableOverTime {
                start: parse_iso_date(start, "recognition_start")?,
                end: parse_iso_date(end, "recognition_end")?,
            })
        }
        "milestone" => Ok(stateset_core::RecognitionMethod::Milestone),
        _ => Err(Error::from_reason(
            "Invalid recognition method (expected point_in_time, ratable_over_time, or milestone)",
        )),
    }
}

fn recognition_method_parts(
    method: stateset_core::RecognitionMethod,
) -> (String, Option<String>, Option<String>) {
    match method {
        stateset_core::RecognitionMethod::PointInTime => ("point_in_time".to_string(), None, None),
        stateset_core::RecognitionMethod::RatableOverTime { start, end } => {
            ("ratable_over_time".to_string(), Some(start.to_string()), Some(end.to_string()))
        }
        stateset_core::RecognitionMethod::Milestone => ("milestone".to_string(), None, None),
        _ => ("unknown".to_string(), None, None),
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePerformanceObligationInput {
    pub description: String,
    /// Exact decimal string
    pub standalone_selling_price: Option<String>,
    /// Exact decimal string; obligations must sum to the transaction price
    pub allocated_amount: String,
    /// point_in_time, ratable_over_time, milestone
    pub recognition_method: String,
    /// ISO date (YYYY-MM-DD); required for ratable_over_time
    pub recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); required for ratable_over_time
    pub recognition_end: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRevenueContractInput {
    /// Optional contract number; auto-generated when omitted (RC-...)
    pub contract_number: Option<String>,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// Exact decimal string
    pub transaction_price: String,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_date: String,
    pub obligations: Vec<CreatePerformanceObligationInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateRevenueContractInput {
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// draft, active, completed, cancelled (transition-guarded)
    pub status: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_date: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueContractFilterInput {
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// draft, active, completed, cancelled
    pub status: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_from: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub effective_to: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PerformanceObligationOutput {
    pub id: String,
    pub contract_id: String,
    pub description: String,
    /// Exact decimal string
    pub standalone_selling_price: Option<String>,
    /// Exact decimal string
    pub allocated_amount: String,
    /// point_in_time, ratable_over_time, milestone
    pub recognition_method: String,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_end: Option<String>,
    /// Exact decimal string
    pub recognized_amount: String,
    /// Exact decimal string: allocated_amount - recognized_amount
    pub deferred_amount: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PerformanceObligation> for PerformanceObligationOutput {
    fn from(o: stateset_core::PerformanceObligation) -> Self {
        let deferred = o.deferred_amount();
        let (method, start, end) = recognition_method_parts(o.recognition_method);
        Self {
            id: o.id.to_string(),
            contract_id: o.contract_id.to_string(),
            description: o.description,
            standalone_selling_price: o.standalone_selling_price.map(|d| d.to_string()),
            allocated_amount: o.allocated_amount.to_string(),
            recognition_method: method,
            recognition_start: start,
            recognition_end: end,
            recognized_amount: o.recognized_amount.to_string(),
            deferred_amount: deferred.to_string(),
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueContractOutput {
    pub id: String,
    pub contract_number: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    /// Exact decimal string
    pub transaction_price: String,
    pub currency: String,
    /// draft, active, completed, cancelled
    pub status: String,
    /// ISO date (YYYY-MM-DD)
    pub effective_date: String,
    pub obligations: Vec<PerformanceObligationOutput>,
    /// Exact decimal string: total recognized across obligations
    pub total_recognized: String,
    /// Exact decimal string: transaction_price - total_recognized
    pub deferred_balance: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::RevenueContract> for RevenueContractOutput {
    fn from(c: stateset_core::RevenueContract) -> Self {
        let total_recognized = c.total_recognized();
        let deferred_balance = c.deferred_balance();
        Self {
            id: c.id.to_string(),
            contract_number: c.contract_number,
            customer_id: c.customer_id.to_string(),
            order_id: c.order_id.map(|id| id.to_string()),
            invoice_id: c.invoice_id.map(|id| id.to_string()),
            transaction_price: c.transaction_price.to_string(),
            currency: c.currency.to_string(),
            status: format!("{}", c.status),
            effective_date: c.effective_date.to_string(),
            obligations: c.obligations.into_iter().map(Into::into).collect(),
            total_recognized: total_recognized.to_string(),
            deferred_balance: deferred_balance.to_string(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueScheduleEntryOutput {
    pub period: u32,
    /// ISO date (YYYY-MM-DD): first day of the entry's month
    pub period_start: String,
    /// Exact decimal string
    pub amount: String,
    /// deferred or recognized
    pub status: String,
}

impl From<stateset_core::RevenueScheduleEntry> for RevenueScheduleEntryOutput {
    fn from(e: stateset_core::RevenueScheduleEntry) -> Self {
        Self {
            period: e.period,
            period_start: e.period_start.to_string(),
            amount: e.amount.to_string(),
            status: format!("{}", e.status),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevenueScheduleOutput {
    pub obligation_id: String,
    /// point_in_time, ratable_over_time, milestone
    pub method: String,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    pub recognition_end: Option<String>,
    pub entries: Vec<RevenueScheduleEntryOutput>,
    /// Exact decimal string
    pub total_amount: String,
    /// Exact decimal string: sum of recognized entries
    pub recognized_total: String,
    /// Exact decimal string: sum of deferred entries
    pub deferred_total: String,
}

impl From<stateset_core::RevenueSchedule> for RevenueScheduleOutput {
    fn from(s: stateset_core::RevenueSchedule) -> Self {
        let recognized_total = s.recognized_total();
        let deferred_total = s.deferred_total();
        let (method, start, end) = recognition_method_parts(s.method);
        Self {
            obligation_id: s.obligation_id.to_string(),
            method,
            recognition_start: start,
            recognition_end: end,
            entries: s.entries.into_iter().map(Into::into).collect(),
            total_amount: s.total_amount.to_string(),
            recognized_total: recognized_total.to_string(),
            deferred_total: deferred_total.to_string(),
        }
    }
}

#[napi]
pub struct RevenueRecognition {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl RevenueRecognition {
    /// Whether the revenue-recognition backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.revenue_recognition().is_supported())
    }

    #[napi]
    pub async fn create_contract(
        &self,
        input: CreateRevenueContractInput,
    ) -> Result<RevenueContractOutput> {
        let commerce = self.commerce.lock().await;
        let customer_id: uuid::Uuid =
            input.customer_id.parse().map_err(|_| Error::from_reason("Invalid customer UUID"))?;
        let currency = input
            .currency
            .map(|s| {
                s.parse::<CurrencyCode>().map_err(|_| Error::from_reason("Invalid currency code"))
            })
            .transpose()?;
        let obligations = input
            .obligations
            .into_iter()
            .map(|o| -> Result<stateset_core::CreatePerformanceObligation> {
                Ok(stateset_core::CreatePerformanceObligation {
                    description: o.description,
                    standalone_selling_price: o
                        .standalone_selling_price
                        .as_deref()
                        .map(|s| parse_decimal_str(s, "standalone_selling_price"))
                        .transpose()?,
                    allocated_amount: parse_decimal_str(&o.allocated_amount, "allocated_amount")?,
                    recognition_method: parse_recognition_method(
                        &o.recognition_method,
                        o.recognition_start.as_deref(),
                        o.recognition_end.as_deref(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let contract = commerce
            .revenue_recognition()
            .create_contract(stateset_core::CreateRevenueContract {
                contract_number: input.contract_number,
                customer_id,
                order_id: parse_optional_uuid(input.order_id, "order_id")?,
                invoice_id: parse_optional_uuid(input.invoice_id, "invoice_id")?,
                transaction_price: parse_decimal_str(
                    &input.transaction_price,
                    "transaction_price",
                )?,
                currency,
                effective_date: parse_iso_date(&input.effective_date, "effective_date")?,
                obligations,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create revenue contract: {}", e)))?;
        Ok(contract.into())
    }

    #[napi]
    pub async fn get_contract(&self, id: String) -> Result<Option<RevenueContractOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let contract = commerce
            .revenue_recognition()
            .get_contract(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get revenue contract: {}", e)))?;
        Ok(contract.map(Into::into))
    }

    #[napi]
    pub async fn list_contracts(
        &self,
        filter: Option<RevenueContractFilterInput>,
    ) -> Result<Vec<RevenueContractOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::RevenueContractFilter::default()),
            |f| -> Result<stateset_core::RevenueContractFilter> {
                Ok(stateset_core::RevenueContractFilter {
                    customer_id: parse_optional_uuid(f.customer_id, "customer_id")?,
                    order_id: parse_optional_uuid(f.order_id, "order_id")?,
                    invoice_id: parse_optional_uuid(f.invoice_id, "invoice_id")?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::RevenueContractStatus>()
                                .map_err(|_| Error::from_reason("Invalid revenue contract status"))
                        })
                        .transpose()?,
                    effective_from: f
                        .effective_from
                        .as_deref()
                        .map(|s| parse_iso_date(s, "effective_from"))
                        .transpose()?,
                    effective_to: f
                        .effective_to
                        .as_deref()
                        .map(|s| parse_iso_date(s, "effective_to"))
                        .transpose()?,
                    search: f.search,
                    limit: f.limit,
                    offset: f.offset,
                    after_cursor: None,
                })
            },
        )?;
        let contracts = commerce
            .revenue_recognition()
            .list_contracts(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list revenue contracts: {}", e)))?;
        Ok(contracts.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn update_contract(
        &self,
        id: String,
        input: UpdateRevenueContractInput,
    ) -> Result<RevenueContractOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let status = input
            .status
            .map(|s| {
                s.parse::<stateset_core::RevenueContractStatus>()
                    .map_err(|_| Error::from_reason("Invalid revenue contract status"))
            })
            .transpose()?;
        let contract = commerce
            .revenue_recognition()
            .update_contract(
                uuid,
                stateset_core::UpdateRevenueContract {
                    order_id: parse_optional_uuid(input.order_id, "order_id")?,
                    invoice_id: parse_optional_uuid(input.invoice_id, "invoice_id")?,
                    status,
                    effective_date: input
                        .effective_date
                        .as_deref()
                        .map(|s| parse_iso_date(s, "effective_date"))
                        .transpose()?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update revenue contract: {}", e)))?;
        Ok(contract.into())
    }

    /// List the performance obligations under a contract.
    #[napi]
    pub async fn list_obligations(
        &self,
        contract_id: String,
    ) -> Result<Vec<PerformanceObligationOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            contract_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let obligations = commerce
            .revenue_recognition()
            .list_obligations(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to list obligations: {}", e)))?;
        Ok(obligations.into_iter().map(Into::into).collect())
    }

    /// Generate and persist the recognition schedule for an obligation.
    #[napi]
    pub async fn generate_schedule(&self, obligation_id: String) -> Result<RevenueScheduleOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let schedule = commerce
            .revenue_recognition()
            .generate_schedule(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to generate schedule: {}", e)))?;
        Ok(schedule.into())
    }

    /// Get the persisted recognition schedule for an obligation, if generated.
    #[napi]
    pub async fn get_schedule(
        &self,
        obligation_id: String,
    ) -> Result<Option<RevenueScheduleOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let schedule = commerce
            .revenue_recognition()
            .get_schedule(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get schedule: {}", e)))?;
        Ok(schedule.map(Into::into))
    }

    /// Recognize deferred entries with a period start on or before `through`
    /// (ISO date, YYYY-MM-DD).
    #[napi]
    pub async fn recognize(
        &self,
        obligation_id: String,
        through: String,
    ) -> Result<RevenueScheduleOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let through = parse_iso_date(&through, "through")?;
        let schedule = commerce
            .revenue_recognition()
            .recognize_period(uuid, through)
            .map_err(|e| Error::from_reason(format!("Failed to recognize revenue: {}", e)))?;
        Ok(schedule.into())
    }
}

// ============================================================================
// Cycle Counts  (quantities cross as exact decimal strings)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCycleCountLineInput {
    pub sku: String,
    pub lot_id: Option<String>,
    /// Exact decimal string
    pub expected_quantity: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCycleCountInput {
    pub warehouse_id: i32,
    /// Optional single location scope; omit to count across the warehouse.
    pub location_id: Option<i32>,
    /// RFC 3339 timestamp
    pub scheduled_date: Option<String>,
    pub counted_by: Option<String>,
    pub lines: Vec<CreateCycleCountLineInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RecordCycleCountLineInput {
    pub sku: String,
    pub lot_id: Option<String>,
    /// Exact decimal string
    pub counted_quantity: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CycleCountFilterInput {
    pub warehouse_id: Option<i32>,
    pub location_id: Option<i32>,
    /// draft, in_progress, completed, cancelled
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CycleCountLineOutput {
    pub id: String,
    pub cycle_count_id: String,
    pub sku: String,
    pub lot_id: Option<String>,
    /// Exact decimal string
    pub expected_quantity: String,
    /// Exact decimal string
    pub counted_quantity: Option<String>,
    /// Exact decimal string: counted_quantity - expected_quantity
    pub variance: Option<String>,
}

impl From<stateset_core::CycleCountLine> for CycleCountLineOutput {
    fn from(l: stateset_core::CycleCountLine) -> Self {
        Self {
            id: l.id.to_string(),
            cycle_count_id: l.cycle_count_id.to_string(),
            sku: l.sku,
            lot_id: l.lot_id.map(|id| id.to_string()),
            expected_quantity: l.expected_quantity.to_string(),
            counted_quantity: l.counted_quantity.map(|d| d.to_string()),
            variance: l.variance.map(|d| d.to_string()),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CycleCountOutput {
    pub id: String,
    pub warehouse_id: i32,
    pub location_id: Option<i32>,
    /// draft, in_progress, completed, cancelled
    pub status: String,
    pub scheduled_date: Option<String>,
    pub counted_by: Option<String>,
    pub lines: Vec<CycleCountLineOutput>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

impl From<stateset_core::CycleCount> for CycleCountOutput {
    fn from(c: stateset_core::CycleCount) -> Self {
        Self {
            id: c.id.to_string(),
            warehouse_id: c.warehouse_id,
            location_id: c.location_id,
            status: format!("{}", c.status),
            scheduled_date: c.scheduled_date.map(|d| d.to_rfc3339()),
            counted_by: c.counted_by,
            lines: c.lines.into_iter().map(Into::into).collect(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
            completed_at: c.completed_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[napi]
pub struct CycleCounts {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl CycleCounts {
    /// Create a cycle count (draft) with its expected lines.
    #[napi]
    pub async fn create(&self, input: CreateCycleCountInput) -> Result<CycleCountOutput> {
        let commerce = self.commerce.lock().await;
        let scheduled_date = match input.scheduled_date.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| Error::from_reason("Invalid scheduled_date RFC 3339 timestamp"))?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let lines = input
            .lines
            .into_iter()
            .map(|l| -> Result<stateset_core::CreateCycleCountLine> {
                Ok(stateset_core::CreateCycleCountLine {
                    sku: l.sku,
                    lot_id: parse_optional_uuid(l.lot_id, "lot_id")?,
                    expected_quantity: parse_decimal_str(
                        &l.expected_quantity,
                        "expected_quantity",
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .warehouse()
            .create_cycle_count(stateset_core::CreateCycleCount {
                warehouse_id: input.warehouse_id,
                location_id: input.location_id,
                scheduled_date,
                counted_by: input.counted_by,
                lines,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create cycle count: {}", e)))?;
        Ok(count.into())
    }

    /// Get a cycle count (with lines) by ID.
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CycleCountOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .get_cycle_count(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get cycle count: {}", e)))?;
        Ok(count.map(Into::into))
    }

    /// List cycle counts matching the filter.
    #[napi]
    pub async fn list(
        &self,
        filter: Option<CycleCountFilterInput>,
    ) -> Result<Vec<CycleCountOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::CycleCountFilter::default()),
            |f| -> Result<stateset_core::CycleCountFilter> {
                Ok(stateset_core::CycleCountFilter {
                    warehouse_id: f.warehouse_id,
                    location_id: f.location_id,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::CycleCountStatus>()
                                .map_err(|_| Error::from_reason("Invalid cycle count status"))
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                    after_cursor: None,
                })
            },
        )?;
        let counts = commerce
            .warehouse()
            .list_cycle_counts(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list cycle counts: {}", e)))?;
        Ok(counts.into_iter().map(Into::into).collect())
    }

    /// Start a draft cycle count (draft -> in_progress).
    #[napi]
    pub async fn start(&self, id: String) -> Result<CycleCountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .start_cycle_count(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to start cycle count: {}", e)))?;
        Ok(count.into())
    }

    /// Record physical counts against an in-progress cycle count.
    #[napi]
    pub async fn record_counts(
        &self,
        id: String,
        counts: Vec<RecordCycleCountLineInput>,
    ) -> Result<CycleCountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let counts = counts
            .into_iter()
            .map(|c| -> Result<stateset_core::RecordCycleCountLine> {
                Ok(stateset_core::RecordCycleCountLine {
                    sku: c.sku,
                    lot_id: parse_optional_uuid(c.lot_id, "lot_id")?,
                    counted_quantity: parse_decimal_str(&c.counted_quantity, "counted_quantity")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .warehouse()
            .record_cycle_counts(uuid, counts)
            .map_err(|e| Error::from_reason(format!("Failed to record cycle counts: {}", e)))?;
        Ok(count.into())
    }

    /// Complete an in-progress cycle count, applying variance adjustments.
    #[napi]
    pub async fn complete(&self, id: String) -> Result<CycleCountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .complete_cycle_count(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to complete cycle count: {}", e)))?;
        Ok(count.into())
    }

    /// Cancel a draft or in-progress cycle count. No adjustments are applied.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<CycleCountOutput> {
        let commerce = self.commerce.lock().await;
        let uuid: uuid::Uuid = id.parse().map_err(|_| Error::from_reason("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .cancel_cycle_count(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel cycle count: {}", e)))?;
        Ok(count.into())
    }
}

// ============================================================================
// EDI Documents API
// ============================================================================

fn parse_edi_direction(s: &str) -> Result<stateset_core::EdiDirection> {
    s.parse::<stateset_core::EdiDirection>()
        .map_err(|_| Error::from_reason(format!("Invalid EDI direction: {}", s)))
}

fn parse_edi_status(s: &str) -> Result<stateset_core::EdiStatus> {
    s.parse::<stateset_core::EdiStatus>()
        .map_err(|_| Error::from_reason(format!("Invalid EDI status: {}", s)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EdiDocumentOutput {
    pub id: String,
    /// EDI document type (e.g. `850`, `855`, `856`, `810`)
    pub document_type: String,
    /// One of `inbound`, `outbound`
    pub direction: String,
    /// One of `pending`, `sent`, `acknowledged`, `processed`, `error`
    pub status: String,
    /// Trading partner name / id
    pub partner: Option<String>,
    /// Related business reference (PO number, order number, etc.)
    pub reference: Option<String>,
    /// Raw EDI payload
    pub payload: Option<String>,
    /// Error detail when `status = error`
    pub error_message: Option<String>,
    /// RFC 3339 timestamp
    pub created_at: String,
    /// RFC 3339 timestamp
    pub updated_at: String,
}

impl From<stateset_core::EdiDocument> for EdiDocumentOutput {
    fn from(d: stateset_core::EdiDocument) -> Self {
        Self {
            id: d.id.to_string(),
            document_type: d.document_type,
            direction: d.direction.to_string(),
            status: d.status.to_string(),
            partner: d.partner,
            reference: d.reference,
            payload: d.payload,
            error_message: d.error_message,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateEdiDocumentInput {
    /// EDI document type (e.g. `850`, `855`, `856`, `810`)
    pub document_type: String,
    /// One of `inbound`, `outbound` (defaults to `inbound`)
    pub direction: Option<String>,
    /// Trading partner name / id
    pub partner: Option<String>,
    /// Related business reference (PO number, order number, etc.)
    pub reference: Option<String>,
    /// Raw EDI payload
    pub payload: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EdiDocumentFilterInput {
    /// Filter by document type (e.g. `850`)
    pub document_type: Option<String>,
    /// Filter by direction: `inbound` or `outbound`
    pub direction: Option<String>,
    /// Filter by status: `pending`, `sent`, `acknowledged`, `processed`, `error`
    pub status: Option<String>,
    /// Filter by trading partner
    pub partner: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EdiCountOutput {
    /// The group key (status or document type)
    pub key: String,
    /// Number of documents in the group
    pub count: i64,
}

impl From<stateset_core::EdiCount> for EdiCountOutput {
    fn from(c: stateset_core::EdiCount) -> Self {
        Self { key: c.key, count: i64::try_from(c.count).unwrap_or(i64::MAX) }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EdiSummaryOutput {
    /// Total document count
    pub total: i64,
    /// Counts grouped by status
    pub by_status: Vec<EdiCountOutput>,
    /// Counts grouped by document type
    pub by_type: Vec<EdiCountOutput>,
}

impl From<stateset_core::EdiAggregateSummary> for EdiSummaryOutput {
    fn from(s: stateset_core::EdiAggregateSummary) -> Self {
        Self {
            total: i64::try_from(s.total).unwrap_or(i64::MAX),
            by_status: s.by_status.into_iter().map(Into::into).collect(),
            by_type: s.by_type.into_iter().map(Into::into).collect(),
        }
    }
}

#[napi]
pub struct EdiDocuments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl EdiDocuments {
    /// Create / ingest an EDI document.
    #[napi]
    pub async fn create(&self, input: CreateEdiDocumentInput) -> Result<EdiDocumentOutput> {
        let commerce = self.commerce.lock().await;
        let direction =
            input.direction.as_deref().map(parse_edi_direction).transpose()?.unwrap_or_default();
        let doc = commerce
            .edi_documents()
            .create(stateset_core::CreateEdiDocument {
                document_type: input.document_type,
                direction,
                partner: input.partner,
                reference: input.reference,
                payload: input.payload,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create EDI document: {}", e)))?;
        Ok(doc.into())
    }

    /// Get an EDI document by ID.
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<EdiDocumentOutput>> {
        let commerce = self.commerce.lock().await;
        let doc_id = id
            .parse::<stateset_core::EdiDocumentId>()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;
        let doc = commerce
            .edi_documents()
            .get(doc_id)
            .map_err(|e| Error::from_reason(format!("Failed to get EDI document: {}", e)))?;
        Ok(doc.map(Into::into))
    }

    /// List EDI documents with optional filtering.
    #[napi]
    pub async fn list(
        &self,
        filter: Option<EdiDocumentFilterInput>,
    ) -> Result<Vec<EdiDocumentOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();
        let docs = commerce
            .edi_documents()
            .list(stateset_core::EdiDocumentFilter {
                document_type: filter.document_type,
                direction: filter.direction.as_deref().map(parse_edi_direction).transpose()?,
                status: filter.status.as_deref().map(parse_edi_status).transpose()?,
                partner: filter.partner,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| Error::from_reason(format!("Failed to list EDI documents: {}", e)))?;
        Ok(docs.into_iter().map(Into::into).collect())
    }

    /// Update an EDI document's status.
    ///
    /// `status` is one of `pending`, `sent`, `acknowledged`, `processed`, `error`;
    /// `error_message` records failure detail when the status is `error`.
    #[napi]
    pub async fn set_status(
        &self,
        id: String,
        status: String,
        error_message: Option<String>,
    ) -> Result<EdiDocumentOutput> {
        let commerce = self.commerce.lock().await;
        let doc_id = id
            .parse::<stateset_core::EdiDocumentId>()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;
        let status = parse_edi_status(&status)?;
        let doc = commerce
            .edi_documents()
            .set_status(doc_id, status, error_message)
            .map_err(|e| Error::from_reason(format!("Failed to set EDI document status: {}", e)))?;
        Ok(doc.into())
    }

    /// Aggregate summary across all EDI documents (counts by status and type).
    #[napi]
    pub async fn summary(&self) -> Result<EdiSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let summary = commerce
            .edi_documents()
            .summary()
            .map_err(|e| Error::from_reason(format!("Failed to get EDI summary: {}", e)))?;
        Ok(summary.into())
    }
}

// ============================================================================
// Shared helpers for the procurement / pricing / logistics domains below
// (money as exact decimal STRINGS, timestamps as RFC 3339 strings,
// enums as snake_case strings)
// ============================================================================

fn parse_rfc3339_opt(
    s: Option<String>,
    field: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    s.as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&chrono::Utc))
                .map_err(|_| Error::from_reason(format!("Invalid {field} RFC 3339 timestamp")))
        })
        .transpose()
}

fn parse_currency_opt(s: Option<String>) -> Result<Option<CurrencyCode>> {
    s.map(|s| s.parse::<CurrencyCode>().map_err(|_| Error::from_reason("Invalid currency code")))
        .transpose()
}

fn parse_uuid_str(s: &str, field: &str) -> Result<uuid::Uuid> {
    s.parse::<uuid::Uuid>().map_err(|_| Error::from_reason(format!("Invalid {field} UUID")))
}

fn parse_optional_decimal_str(s: Option<String>, field: &str) -> Result<Option<Decimal>> {
    s.as_deref().map(|s| parse_decimal_str(s, field)).transpose()
}

// ============================================================================
// Prepayments  (advance payments to suppliers)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePrepaymentInput {
    pub supplier_id: String,
    /// Exact decimal string, e.g. "1000.00"
    pub amount: String,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// Payment method (e.g. "wire", "ach")
    pub method: Option<String>,
    pub reference: Option<String>,
    pub memo: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApplyPrepaymentInput {
    /// bill or payment_obligation
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrepaymentFilterInput {
    pub supplier_id: Option<String>,
    /// open, applied, refunded, cancelled
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrepaymentOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub remaining: String,
    pub currency: String,
    /// open, applied, refunded, cancelled
    pub status: String,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub memo: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Prepayment> for PrepaymentOutput {
    fn from(p: stateset_core::Prepayment) -> Self {
        Self {
            id: p.id.to_string(),
            number: p.number,
            supplier_id: p.supplier_id.to_string(),
            amount: p.amount.to_string(),
            remaining: p.remaining.to_string(),
            currency: p.currency.to_string(),
            status: format!("{}", p.status),
            method: p.method,
            reference: p.reference,
            memo: p.memo,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrepaymentApplicationOutput {
    pub id: String,
    pub prepayment_id: String,
    /// bill or payment_obligation
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
    pub reversed: bool,
    pub created_at: String,
}

impl From<stateset_core::PrepaymentApplication> for PrepaymentApplicationOutput {
    fn from(a: stateset_core::PrepaymentApplication) -> Self {
        Self {
            id: a.id.to_string(),
            prepayment_id: a.prepayment_id.to_string(),
            target_type: format!("{}", a.target_type),
            target_id: a.target_id.to_string(),
            amount: a.amount.to_string(),
            reversed: a.reversed,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Prepayments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Prepayments {
    /// Whether the prepayments backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.prepayments().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreatePrepaymentInput) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.lock().await;
        let prepayment = commerce
            .prepayments()
            .create(stateset_core::CreatePrepayment {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                amount: parse_decimal_str(&input.amount, "amount")?,
                currency: parse_currency_opt(input.currency)?,
                method: input.method,
                reference: input.reference,
                memo: input.memo,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create prepayment: {}", e)))?;
        Ok(prepayment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PrepaymentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let prepayment = commerce
            .prepayments()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get prepayment: {}", e)))?;
        Ok(prepayment.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PrepaymentFilterInput>,
    ) -> Result<Vec<PrepaymentOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PrepaymentFilter::default()),
            |f| -> Result<stateset_core::PrepaymentFilter> {
                Ok(stateset_core::PrepaymentFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::PrepaymentStatus>()
                                .map_err(|_| Error::from_reason("Invalid prepayment status"))
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let prepayments = commerce
            .prepayments()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list prepayments: {}", e)))?;
        Ok(prepayments.into_iter().map(Into::into).collect())
    }

    /// Apply a prepayment against a bill or payment obligation.
    #[napi]
    pub async fn apply(&self, id: String, input: ApplyPrepaymentInput) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let target_type = input
            .target_type
            .parse::<stateset_core::PrepaymentTargetType>()
            .map_err(|_| Error::from_reason("Invalid prepayment target type"))?;
        let prepayment = commerce
            .prepayments()
            .apply(
                uuid.into(),
                stateset_core::ApplyPrepayment {
                    target_type,
                    target_id: parse_uuid_str(&input.target_id, "target_id")?,
                    amount: parse_decimal_str(&input.amount, "amount")?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to apply prepayment: {}", e)))?;
        Ok(prepayment.into())
    }

    /// List applications for a prepayment.
    #[napi]
    pub async fn list_applications(&self, id: String) -> Result<Vec<PrepaymentApplicationOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let applications = commerce.prepayments().list_applications(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to list prepayment applications: {}", e))
        })?;
        Ok(applications.into_iter().map(Into::into).collect())
    }

    /// Reverse a previously-recorded application.
    #[napi]
    pub async fn reverse_application(
        &self,
        id: String,
        application_id: String,
    ) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let app_uuid = parse_uuid_str(&application_id, "application")?;
        let prepayment =
            commerce.prepayments().reverse_application(uuid.into(), app_uuid.into()).map_err(
                |e| Error::from_reason(format!("Failed to reverse prepayment application: {}", e)),
            )?;
        Ok(prepayment.into())
    }

    /// Refund the remaining balance, closing the prepayment.
    #[napi]
    pub async fn refund(&self, id: String) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let prepayment = commerce
            .prepayments()
            .refund(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to refund prepayment: {}", e)))?;
        Ok(prepayment.into())
    }
}

// ============================================================================
// Vendor Credits  (supplier-owed credits)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateVendorCreditInput {
    pub supplier_id: String,
    pub vendor_return_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    pub memo: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApplyVendorCreditInput {
    /// bill or payment_obligation
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorCreditFilterInput {
    pub supplier_id: Option<String>,
    /// open, applied, cancelled
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorCreditOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub vendor_return_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub remaining: String,
    pub currency: String,
    /// open, applied, cancelled
    pub status: String,
    pub memo: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::VendorCredit> for VendorCreditOutput {
    fn from(c: stateset_core::VendorCredit) -> Self {
        Self {
            id: c.id.to_string(),
            number: c.number,
            supplier_id: c.supplier_id.to_string(),
            vendor_return_id: c.vendor_return_id.map(|id| id.to_string()),
            amount: c.amount.to_string(),
            remaining: c.remaining.to_string(),
            currency: c.currency.to_string(),
            status: format!("{}", c.status),
            memo: c.memo,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorCreditApplicationOutput {
    pub id: String,
    pub vendor_credit_id: String,
    /// bill or payment_obligation
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
    pub reversed: bool,
    pub created_at: String,
}

impl From<stateset_core::VendorCreditApplication> for VendorCreditApplicationOutput {
    fn from(a: stateset_core::VendorCreditApplication) -> Self {
        Self {
            id: a.id.to_string(),
            vendor_credit_id: a.vendor_credit_id.to_string(),
            target_type: format!("{}", a.target_type),
            target_id: a.target_id.to_string(),
            amount: a.amount.to_string(),
            reversed: a.reversed,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct VendorCredits {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl VendorCredits {
    /// Whether the vendor-credits backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.vendor_credits().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateVendorCreditInput) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.lock().await;
        let credit = commerce
            .vendor_credits()
            .create(stateset_core::CreateVendorCredit {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                vendor_return_id: parse_optional_uuid(input.vendor_return_id, "vendor_return_id")?,
                amount: parse_decimal_str(&input.amount, "amount")?,
                currency: parse_currency_opt(input.currency)?,
                memo: input.memo,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create vendor credit: {}", e)))?;
        Ok(credit.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<VendorCreditOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let credit = commerce
            .vendor_credits()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get vendor credit: {}", e)))?;
        Ok(credit.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<VendorCreditFilterInput>,
    ) -> Result<Vec<VendorCreditOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::VendorCreditFilter::default()),
            |f| -> Result<stateset_core::VendorCreditFilter> {
                Ok(stateset_core::VendorCreditFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::VendorCreditStatus>()
                                .map_err(|_| Error::from_reason("Invalid vendor credit status"))
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let credits = commerce
            .vendor_credits()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list vendor credits: {}", e)))?;
        Ok(credits.into_iter().map(Into::into).collect())
    }

    /// Apply a vendor credit against a bill or payment obligation.
    #[napi]
    pub async fn apply(
        &self,
        id: String,
        input: ApplyVendorCreditInput,
    ) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let target_type = input
            .target_type
            .parse::<stateset_core::VendorCreditTargetType>()
            .map_err(|_| Error::from_reason("Invalid vendor credit target type"))?;
        let credit = commerce
            .vendor_credits()
            .apply(
                uuid.into(),
                stateset_core::ApplyVendorCredit {
                    target_type,
                    target_id: parse_uuid_str(&input.target_id, "target_id")?,
                    amount: parse_decimal_str(&input.amount, "amount")?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to apply vendor credit: {}", e)))?;
        Ok(credit.into())
    }

    /// List applications for a vendor credit.
    #[napi]
    pub async fn list_applications(
        &self,
        id: String,
    ) -> Result<Vec<VendorCreditApplicationOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let applications =
            commerce.vendor_credits().list_applications(uuid.into()).map_err(|e| {
                Error::from_reason(format!("Failed to list vendor credit applications: {}", e))
            })?;
        Ok(applications.into_iter().map(Into::into).collect())
    }

    /// Reverse a previously-recorded application.
    #[napi]
    pub async fn reverse_application(
        &self,
        id: String,
        application_id: String,
    ) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let app_uuid = parse_uuid_str(&application_id, "application")?;
        let credit = commerce
            .vendor_credits()
            .reverse_application(uuid.into(), app_uuid.into())
            .map_err(|e| {
                Error::from_reason(format!("Failed to reverse vendor credit application: {}", e))
            })?;
        Ok(credit.into())
    }

    /// Cancel a vendor credit.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let credit = commerce
            .vendor_credits()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel vendor credit: {}", e)))?;
        Ok(credit.into())
    }
}

// ============================================================================
// Price Schedules  (time-bounded pricing)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePriceScheduleInput {
    pub name: String,
    pub code: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// RFC 3339 timestamp
    pub starts_at: Option<String>,
    /// RFC 3339 timestamp
    pub ends_at: Option<String>,
    /// Priority used to break ties (higher wins); default 0
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdatePriceScheduleInput {
    pub name: Option<String>,
    pub code: Option<String>,
    /// RFC 3339 timestamp
    pub starts_at: Option<String>,
    /// RFC 3339 timestamp
    pub ends_at: Option<String>,
    pub is_active: Option<bool>,
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceScheduleFilterInput {
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceScheduleOutput {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub currency: String,
    /// RFC 3339 timestamp
    pub starts_at: Option<String>,
    /// RFC 3339 timestamp
    pub ends_at: Option<String>,
    pub is_active: bool,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceSchedule> for PriceScheduleOutput {
    fn from(s: stateset_core::PriceSchedule) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            code: s.code,
            currency: s.currency.to_string(),
            starts_at: s.starts_at.map(|d| d.to_rfc3339()),
            ends_at: s.ends_at.map(|d| d.to_rfc3339()),
            is_active: s.is_active,
            priority: s.priority,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceScheduleEntryOutput {
    pub price_schedule_id: String,
    pub product_id: String,
    /// Exact decimal string
    pub price: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceScheduleEntry> for PriceScheduleEntryOutput {
    fn from(e: stateset_core::PriceScheduleEntry) -> Self {
        Self {
            price_schedule_id: e.price_schedule_id.to_string(),
            product_id: e.product_id.to_string(),
            price: e.price.to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct PriceSchedules {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl PriceSchedules {
    /// Whether the price-schedules backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.price_schedules().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreatePriceScheduleInput) -> Result<PriceScheduleOutput> {
        let commerce = self.commerce.lock().await;
        let schedule = commerce
            .price_schedules()
            .create(stateset_core::CreatePriceSchedule {
                name: input.name,
                code: input.code,
                currency: parse_currency_opt(input.currency)?,
                starts_at: parse_rfc3339_opt(input.starts_at, "starts_at")?,
                ends_at: parse_rfc3339_opt(input.ends_at, "ends_at")?,
                priority: input.priority.unwrap_or(0),
            })
            .map_err(|e| Error::from_reason(format!("Failed to create price schedule: {}", e)))?;
        Ok(schedule.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PriceScheduleOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let schedule = commerce
            .price_schedules()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get price schedule: {}", e)))?;
        Ok(schedule.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdatePriceScheduleInput,
    ) -> Result<PriceScheduleOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let schedule = commerce
            .price_schedules()
            .update(
                uuid.into(),
                stateset_core::UpdatePriceSchedule {
                    name: input.name,
                    code: input.code,
                    starts_at: parse_rfc3339_opt(input.starts_at, "starts_at")?,
                    ends_at: parse_rfc3339_opt(input.ends_at, "ends_at")?,
                    is_active: input.is_active,
                    priority: input.priority,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update price schedule: {}", e)))?;
        Ok(schedule.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PriceScheduleFilterInput>,
    ) -> Result<Vec<PriceScheduleOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::PriceScheduleFilter::default, |f| {
            stateset_core::PriceScheduleFilter {
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let schedules = commerce
            .price_schedules()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list price schedules: {}", e)))?;
        Ok(schedules.into_iter().map(Into::into).collect())
    }

    /// Delete a price schedule and its entries.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        commerce
            .price_schedules()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete price schedule: {}", e)))?;
        Ok(())
    }

    /// Upsert a per-product scheduled price (exact decimal string).
    #[napi]
    pub async fn set_entry(
        &self,
        id: String,
        product_id: String,
        price: String,
    ) -> Result<PriceScheduleEntryOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        let entry = commerce
            .price_schedules()
            .set_entry(uuid.into(), product_uuid.into(), parse_decimal_str(&price, "price")?)
            .map_err(|e| {
                Error::from_reason(format!("Failed to set price schedule entry: {}", e))
            })?;
        Ok(entry.into())
    }

    /// Remove a per-product entry.
    #[napi]
    pub async fn delete_entry(&self, id: String, product_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        commerce.price_schedules().delete_entry(uuid.into(), product_uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to delete price schedule entry: {}", e))
        })?;
        Ok(())
    }

    /// List per-product entries for a schedule.
    #[napi]
    pub async fn list_entries(&self, id: String) -> Result<Vec<PriceScheduleEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let entries = commerce.price_schedules().list_entries(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to list price schedule entries: {}", e))
        })?;
        Ok(entries.into_iter().map(Into::into).collect())
    }

    /// Resolve the effective scheduled price for a product at an instant
    /// (`at` is an RFC 3339 timestamp; defaults to now). Returns an exact
    /// decimal string, or null when no schedule applies.
    #[napi]
    pub async fn resolve_price(
        &self,
        product_id: String,
        at: Option<String>,
    ) -> Result<Option<String>> {
        let commerce = self.commerce.lock().await;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        let at = parse_rfc3339_opt(at, "at")?.unwrap_or_else(chrono::Utc::now);
        let price = commerce
            .price_schedules()
            .resolve_price(product_uuid.into(), at)
            .map_err(|e| Error::from_reason(format!("Failed to resolve scheduled price: {}", e)))?;
        Ok(price.map(|p| p.to_string()))
    }
}

// ============================================================================
// Price Levels  (B2B pricing tiers)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePriceLevelInput {
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    /// none, percentage_discount, percentage_markup (default none)
    pub adjustment_type: Option<String>,
    /// Percentage as exact decimal string (e.g. "10" for 10%); default "0"
    pub adjustment_value: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdatePriceLevelInput {
    pub name: Option<String>,
    pub description: Option<String>,
    /// none, percentage_discount, percentage_markup
    pub adjustment_type: Option<String>,
    /// Percentage as exact decimal string
    pub adjustment_value: Option<String>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceLevelFilterInput {
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceLevelOutput {
    pub id: String,
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    /// none, percentage_discount, percentage_markup
    pub adjustment_type: String,
    /// Percentage as exact decimal string
    pub adjustment_value: String,
    pub currency: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceLevel> for PriceLevelOutput {
    fn from(l: stateset_core::PriceLevel) -> Self {
        Self {
            id: l.id.to_string(),
            name: l.name,
            code: l.code,
            description: l.description,
            adjustment_type: format!("{}", l.adjustment_type),
            adjustment_value: l.adjustment_value.to_string(),
            currency: l.currency.to_string(),
            is_active: l.is_active,
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceLevelEntryOutput {
    pub price_level_id: String,
    pub product_id: String,
    /// Exact decimal string
    pub price: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceLevelEntry> for PriceLevelEntryOutput {
    fn from(e: stateset_core::PriceLevelEntry) -> Self {
        Self {
            price_level_id: e.price_level_id.to_string(),
            product_id: e.product_id.to_string(),
            price: e.price.to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct PriceLevels {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl PriceLevels {
    /// Whether the price-levels backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.price_levels().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreatePriceLevelInput) -> Result<PriceLevelOutput> {
        let commerce = self.commerce.lock().await;
        let adjustment_type = input
            .adjustment_type
            .map(|s| {
                s.parse::<stateset_core::PriceAdjustmentType>()
                    .map_err(|_| Error::from_reason("Invalid price adjustment type"))
            })
            .transpose()?
            .unwrap_or_default();
        let adjustment_value = input
            .adjustment_value
            .as_deref()
            .map(|s| parse_decimal_str(s, "adjustment_value"))
            .transpose()?
            .unwrap_or(Decimal::ZERO);
        let level = commerce
            .price_levels()
            .create(stateset_core::CreatePriceLevel {
                name: input.name,
                code: input.code,
                description: input.description,
                adjustment_type,
                adjustment_value,
                currency: parse_currency_opt(input.currency)?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create price level: {}", e)))?;
        Ok(level.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PriceLevelOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price level")?;
        let level = commerce
            .price_levels()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get price level: {}", e)))?;
        Ok(level.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdatePriceLevelInput,
    ) -> Result<PriceLevelOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price level")?;
        let adjustment_type = input
            .adjustment_type
            .map(|s| {
                s.parse::<stateset_core::PriceAdjustmentType>()
                    .map_err(|_| Error::from_reason("Invalid price adjustment type"))
            })
            .transpose()?;
        let level = commerce
            .price_levels()
            .update(
                uuid.into(),
                stateset_core::UpdatePriceLevel {
                    name: input.name,
                    description: input.description,
                    adjustment_type,
                    adjustment_value: parse_optional_decimal_str(
                        input.adjustment_value,
                        "adjustment_value",
                    )?,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update price level: {}", e)))?;
        Ok(level.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PriceLevelFilterInput>,
    ) -> Result<Vec<PriceLevelOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::PriceLevelFilter::default, |f| {
            stateset_core::PriceLevelFilter {
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let levels = commerce
            .price_levels()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list price levels: {}", e)))?;
        Ok(levels.into_iter().map(Into::into).collect())
    }

    /// Delete a price level and its entries.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price level")?;
        commerce
            .price_levels()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete price level: {}", e)))?;
        Ok(())
    }

    /// Upsert a per-product fixed price entry (exact decimal string).
    #[napi]
    pub async fn set_entry(
        &self,
        id: String,
        product_id: String,
        price: String,
    ) -> Result<PriceLevelEntryOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price level")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        let entry = commerce
            .price_levels()
            .set_entry(uuid.into(), product_uuid.into(), parse_decimal_str(&price, "price")?)
            .map_err(|e| Error::from_reason(format!("Failed to set price level entry: {}", e)))?;
        Ok(entry.into())
    }

    /// Remove a per-product entry.
    #[napi]
    pub async fn delete_entry(&self, id: String, product_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price level")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        commerce.price_levels().delete_entry(uuid.into(), product_uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to delete price level entry: {}", e))
        })?;
        Ok(())
    }

    /// List per-product entries for a level.
    #[napi]
    pub async fn list_entries(&self, id: String) -> Result<Vec<PriceLevelEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "price level")?;
        let entries = commerce.price_levels().list_entries(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to list price level entries: {}", e))
        })?;
        Ok(entries.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Transfer Orders  (inter-warehouse stock movement)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateTransferOrderItemInput {
    pub product_id: String,
    /// Exact decimal string
    pub quantity: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateTransferOrderInput {
    pub source_warehouse_id: String,
    pub destination_warehouse_id: String,
    pub items: Vec<CreateTransferOrderItemInput>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TransferOrderFilterInput {
    /// draft, pending, in_transit, partially_received, received, cancelled
    pub status: Option<String>,
    pub source_warehouse_id: Option<String>,
    pub destination_warehouse_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TransferOrderItemOutput {
    pub id: String,
    pub transfer_order_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity: String,
    /// Exact decimal string
    pub quantity_shipped: String,
    /// Exact decimal string
    pub quantity_received: String,
}

impl From<stateset_core::TransferOrderItem> for TransferOrderItemOutput {
    fn from(i: stateset_core::TransferOrderItem) -> Self {
        Self {
            id: i.id.to_string(),
            transfer_order_id: i.transfer_order_id.to_string(),
            product_id: i.product_id.to_string(),
            sku: i.sku,
            quantity: i.quantity.to_string(),
            quantity_shipped: i.quantity_shipped.to_string(),
            quantity_received: i.quantity_received.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TransferOrderOutput {
    pub id: String,
    pub number: String,
    pub source_warehouse_id: String,
    pub destination_warehouse_id: String,
    /// draft, pending, in_transit, partially_received, received, cancelled
    pub status: String,
    pub items: Vec<TransferOrderItemOutput>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    /// RFC 3339 timestamp
    pub shipped_at: Option<String>,
    /// RFC 3339 timestamp
    pub received_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TransferOrder> for TransferOrderOutput {
    fn from(o: stateset_core::TransferOrder) -> Self {
        Self {
            id: o.id.to_string(),
            number: o.number,
            source_warehouse_id: o.source_warehouse_id.to_string(),
            destination_warehouse_id: o.destination_warehouse_id.to_string(),
            status: format!("{}", o.status),
            items: o.items.into_iter().map(Into::into).collect(),
            expected_at: o.expected_at.map(|d| d.to_rfc3339()),
            shipped_at: o.shipped_at.map(|d| d.to_rfc3339()),
            received_at: o.received_at.map(|d| d.to_rfc3339()),
            notes: o.notes,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct TransferOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl TransferOrders {
    /// Whether the transfer-orders backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.transfer_orders().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateTransferOrderInput) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.lock().await;
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::CreateTransferOrderItem> {
                Ok(stateset_core::CreateTransferOrderItem {
                    product_id: parse_uuid_str(&i.product_id, "product")?.into(),
                    quantity: parse_decimal_str(&i.quantity, "quantity")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let order = commerce
            .transfer_orders()
            .create(stateset_core::CreateTransferOrder {
                source_warehouse_id: parse_uuid_str(
                    &input.source_warehouse_id,
                    "source_warehouse_id",
                )?
                .into(),
                destination_warehouse_id: parse_uuid_str(
                    &input.destination_warehouse_id,
                    "destination_warehouse_id",
                )?
                .into(),
                items,
                expected_at: parse_rfc3339_opt(input.expected_at, "expected_at")?,
                notes: input.notes,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create transfer order: {}", e)))?;
        Ok(order.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<TransferOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let order = commerce
            .transfer_orders()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get transfer order: {}", e)))?;
        Ok(order.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<TransferOrderFilterInput>,
    ) -> Result<Vec<TransferOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::TransferOrderFilter::default()),
            |f| -> Result<stateset_core::TransferOrderFilter> {
                Ok(stateset_core::TransferOrderFilter {
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::TransferOrderStatus>()
                                .map_err(|_| Error::from_reason("Invalid transfer order status"))
                        })
                        .transpose()?,
                    source_warehouse_id: parse_optional_uuid(
                        f.source_warehouse_id,
                        "source_warehouse_id",
                    )?
                    .map(Into::into),
                    destination_warehouse_id: parse_optional_uuid(
                        f.destination_warehouse_id,
                        "destination_warehouse_id",
                    )?
                    .map(Into::into),
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let orders = commerce
            .transfer_orders()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list transfer orders: {}", e)))?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// Mark a transfer order as shipped from the source.
    #[napi]
    pub async fn ship(&self, id: String) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let order = commerce
            .transfer_orders()
            .ship(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to ship transfer order: {}", e)))?;
        Ok(order.into())
    }

    /// Receive a quantity (exact decimal string) against a single line at the
    /// destination.
    #[napi]
    pub async fn receive_line(
        &self,
        id: String,
        item_id: String,
        quantity: String,
    ) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let item_uuid = parse_uuid_str(&item_id, "transfer order item")?;
        let order = commerce
            .transfer_orders()
            .receive_line(uuid.into(), item_uuid.into(), parse_decimal_str(&quantity, "quantity")?)
            .map_err(|e| {
                Error::from_reason(format!("Failed to receive transfer order line: {}", e))
            })?;
        Ok(order.into())
    }

    /// Cancel a transfer order.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<TransferOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "transfer order")?;
        let order = commerce
            .transfer_orders()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel transfer order: {}", e)))?;
        Ok(order.into())
    }
}

// ============================================================================
// Production Batches  (grouping manufacturing work orders)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateProductionBatchInput {
    pub name: String,
    pub vendor_id: Option<String>,
    /// Work order UUIDs to link at creation
    pub work_order_ids: Option<Vec<String>>,
    pub notes: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_end: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateProductionBatchInput {
    pub name: Option<String>,
    pub vendor_id: Option<String>,
    /// planned, in_progress, completed, cancelled
    pub status: Option<String>,
    pub notes: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_end: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductionBatchFilterInput {
    /// planned, in_progress, completed, cancelled
    pub status: Option<String>,
    pub vendor_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductionBatchOutput {
    pub id: String,
    pub name: String,
    /// planned, in_progress, completed, cancelled
    pub status: String,
    pub vendor_id: Option<String>,
    pub work_order_ids: Vec<String>,
    pub notes: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_start: Option<String>,
    /// RFC 3339 timestamp
    pub scheduled_end: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ProductionBatch> for ProductionBatchOutput {
    fn from(b: stateset_core::ProductionBatch) -> Self {
        Self {
            id: b.id.to_string(),
            name: b.name,
            status: format!("{}", b.status),
            vendor_id: b.vendor_id.map(|id| id.to_string()),
            work_order_ids: b.work_order_ids.iter().map(ToString::to_string).collect(),
            notes: b.notes,
            scheduled_start: b.scheduled_start.map(|d| d.to_rfc3339()),
            scheduled_end: b.scheduled_end.map(|d| d.to_rfc3339()),
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct ProductionBatches {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl ProductionBatches {
    /// Whether the production-batches backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.production_batches().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateProductionBatchInput) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.lock().await;
        let work_order_ids = input
            .work_order_ids
            .unwrap_or_default()
            .into_iter()
            .map(|s| parse_uuid_str(&s, "work_order"))
            .collect::<Result<Vec<_>>>()?;
        let batch = commerce
            .production_batches()
            .create(stateset_core::CreateProductionBatch {
                name: input.name,
                vendor_id: parse_optional_uuid(input.vendor_id, "vendor_id")?,
                work_order_ids,
                notes: input.notes,
                scheduled_start: parse_rfc3339_opt(input.scheduled_start, "scheduled_start")?,
                scheduled_end: parse_rfc3339_opt(input.scheduled_end, "scheduled_end")?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create production batch: {}", e)))?;
        Ok(batch.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ProductionBatchOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let batch = commerce
            .production_batches()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get production batch: {}", e)))?;
        Ok(batch.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateProductionBatchInput,
    ) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let status = input
            .status
            .map(|s| {
                s.parse::<stateset_core::ProductionBatchStatus>()
                    .map_err(|_| Error::from_reason("Invalid production batch status"))
            })
            .transpose()?;
        let batch = commerce
            .production_batches()
            .update(
                uuid.into(),
                stateset_core::UpdateProductionBatch {
                    name: input.name,
                    vendor_id: parse_optional_uuid(input.vendor_id, "vendor_id")?,
                    status,
                    notes: input.notes,
                    scheduled_start: parse_rfc3339_opt(input.scheduled_start, "scheduled_start")?,
                    scheduled_end: parse_rfc3339_opt(input.scheduled_end, "scheduled_end")?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update production batch: {}", e)))?;
        Ok(batch.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<ProductionBatchFilterInput>,
    ) -> Result<Vec<ProductionBatchOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ProductionBatchFilter::default()),
            |f| -> Result<stateset_core::ProductionBatchFilter> {
                Ok(stateset_core::ProductionBatchFilter {
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::ProductionBatchStatus>()
                                .map_err(|_| Error::from_reason("Invalid production batch status"))
                        })
                        .transpose()?,
                    vendor_id: parse_optional_uuid(f.vendor_id, "vendor_id")?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let batches = commerce
            .production_batches()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list production batches: {}", e)))?;
        Ok(batches.into_iter().map(Into::into).collect())
    }

    /// Delete a production batch.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "production batch")?;
        commerce
            .production_batches()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete production batch: {}", e)))?;
        Ok(())
    }

    /// Link work orders to a batch.
    #[napi]
    pub async fn add_work_orders(
        &self,
        id: String,
        work_order_ids: Vec<String>,
    ) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let work_order_ids = work_order_ids
            .into_iter()
            .map(|s| parse_uuid_str(&s, "work_order"))
            .collect::<Result<Vec<_>>>()?;
        let batch = commerce
            .production_batches()
            .add_work_orders(uuid.into(), work_order_ids)
            .map_err(|e| Error::from_reason(format!("Failed to add work orders: {}", e)))?;
        Ok(batch.into())
    }

    /// Remove a work order from a batch.
    #[napi]
    pub async fn remove_work_order(
        &self,
        id: String,
        work_order_id: String,
    ) -> Result<ProductionBatchOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "production batch")?;
        let work_order_uuid = parse_uuid_str(&work_order_id, "work_order")?;
        let batch =
            commerce
                .production_batches()
                .remove_work_order(uuid.into(), work_order_uuid)
                .map_err(|e| Error::from_reason(format!("Failed to remove work order: {}", e)))?;
        Ok(batch.into())
    }
}

// ============================================================================
// Supplier SKUs  (per-supplier SKU / unit-cost overrides)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSupplierSkuInput {
    pub product_id: String,
    pub supplier_id: String,
    pub sku: String,
    /// Exact decimal string
    pub unit_cost: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// Exact decimal string
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSupplierSkuInput {
    pub sku: Option<String>,
    /// Exact decimal string
    pub unit_cost: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// Exact decimal string
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
    pub is_preferred: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SupplierSkuFilterInput {
    pub supplier_id: Option<String>,
    pub product_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BulkSupplierSkuItemInput {
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub unit_cost: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SupplierSkuOutput {
    pub id: String,
    pub product_id: String,
    pub supplier_id: String,
    pub sku: String,
    /// Exact decimal string
    pub unit_cost: Option<String>,
    pub currency: String,
    /// Exact decimal string
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
    pub is_preferred: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::SupplierSku> for SupplierSkuOutput {
    fn from(s: stateset_core::SupplierSku) -> Self {
        Self {
            id: s.id.to_string(),
            product_id: s.product_id.to_string(),
            supplier_id: s.supplier_id.to_string(),
            sku: s.sku,
            unit_cost: s.unit_cost.map(|c| c.to_string()),
            currency: s.currency.to_string(),
            min_order_qty: s.min_order_qty.map(|q| q.to_string()),
            lead_time_days: s.lead_time_days,
            is_preferred: s.is_preferred,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct SupplierSkus {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl SupplierSkus {
    /// Whether the supplier-SKUs backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.supplier_skus().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateSupplierSkuInput) -> Result<SupplierSkuOutput> {
        let commerce = self.commerce.lock().await;
        let record = commerce
            .supplier_skus()
            .create(stateset_core::CreateSupplierSku {
                product_id: parse_uuid_str(&input.product_id, "product")?.into(),
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier")?,
                sku: input.sku,
                unit_cost: parse_optional_decimal_str(input.unit_cost, "unit_cost")?,
                currency: parse_currency_opt(input.currency)?,
                min_order_qty: parse_optional_decimal_str(input.min_order_qty, "min_order_qty")?,
                lead_time_days: input.lead_time_days,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create supplier SKU: {}", e)))?;
        Ok(record.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SupplierSkuOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "supplier SKU")?;
        let record = commerce
            .supplier_skus()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get supplier SKU: {}", e)))?;
        Ok(record.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateSupplierSkuInput,
    ) -> Result<SupplierSkuOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "supplier SKU")?;
        let record = commerce
            .supplier_skus()
            .update(
                uuid.into(),
                stateset_core::UpdateSupplierSku {
                    sku: input.sku,
                    unit_cost: parse_optional_decimal_str(input.unit_cost, "unit_cost")?,
                    currency: parse_currency_opt(input.currency)?,
                    min_order_qty: parse_optional_decimal_str(
                        input.min_order_qty,
                        "min_order_qty",
                    )?,
                    lead_time_days: input.lead_time_days,
                    is_preferred: input.is_preferred,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update supplier SKU: {}", e)))?;
        Ok(record.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<SupplierSkuFilterInput>,
    ) -> Result<Vec<SupplierSkuOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::SupplierSkuFilter::default()),
            |f| -> Result<stateset_core::SupplierSkuFilter> {
                Ok(stateset_core::SupplierSkuFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    product_id: parse_optional_uuid(f.product_id, "product_id")?.map(Into::into),
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let records = commerce
            .supplier_skus()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list supplier SKUs: {}", e)))?;
        Ok(records.into_iter().map(Into::into).collect())
    }

    /// Delete a supplier SKU.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "supplier SKU")?;
        commerce
            .supplier_skus()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete supplier SKU: {}", e)))?;
        Ok(())
    }

    /// Bulk upsert supplier SKUs for a supplier, keyed by internal product.
    /// Returns the number of records upserted.
    #[napi]
    pub async fn bulk_upsert(
        &self,
        supplier_id: String,
        items: Vec<BulkSupplierSkuItemInput>,
    ) -> Result<i64> {
        let commerce = self.commerce.lock().await;
        let supplier_uuid = parse_uuid_str(&supplier_id, "supplier")?;
        let items = items
            .into_iter()
            .map(|i| -> Result<stateset_core::BulkSupplierSkuItem> {
                Ok(stateset_core::BulkSupplierSkuItem {
                    product_id: parse_uuid_str(&i.product_id, "product")?.into(),
                    sku: i.sku,
                    unit_cost: parse_optional_decimal_str(i.unit_cost, "unit_cost")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce.supplier_skus().bulk_upsert(supplier_uuid, items).map_err(|e| {
            Error::from_reason(format!("Failed to bulk upsert supplier SKUs: {}", e))
        })?;
        i64::try_from(count).map_err(|_| Error::from_reason("Bulk upsert count exceeds i64 range"))
    }
}

// ============================================================================
// Inbound Shipments  (advance ship notices)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInboundShipmentItemInput {
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_expected: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInboundShipmentInput {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub warehouse_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    pub items: Vec<CreateInboundShipmentItemInput>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InboundShipmentFilterInput {
    pub supplier_id: Option<String>,
    pub warehouse_id: Option<String>,
    /// pending, in_transit, arrived, partially_received, received, cancelled
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InboundShipmentItemOutput {
    pub id: String,
    pub inbound_shipment_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_expected: String,
    /// Exact decimal string
    pub quantity_received: String,
}

impl From<stateset_core::InboundShipmentItem> for InboundShipmentItemOutput {
    fn from(i: stateset_core::InboundShipmentItem) -> Self {
        Self {
            id: i.id.to_string(),
            inbound_shipment_id: i.inbound_shipment_id.to_string(),
            product_id: i.product_id.to_string(),
            sku: i.sku,
            quantity_expected: i.quantity_expected.to_string(),
            quantity_received: i.quantity_received.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InboundShipmentOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub warehouse_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// pending, in_transit, arrived, partially_received, received, cancelled
    pub status: String,
    pub items: Vec<InboundShipmentItemOutput>,
    /// RFC 3339 timestamp
    pub expected_at: Option<String>,
    /// RFC 3339 timestamp
    pub received_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::InboundShipment> for InboundShipmentOutput {
    fn from(s: stateset_core::InboundShipment) -> Self {
        Self {
            id: s.id.to_string(),
            number: s.number,
            supplier_id: s.supplier_id.to_string(),
            purchase_order_id: s.purchase_order_id.map(|id| id.to_string()),
            warehouse_id: s.warehouse_id.map(|id| id.to_string()),
            carrier: s.carrier,
            tracking_number: s.tracking_number,
            status: format!("{}", s.status),
            items: s.items.into_iter().map(Into::into).collect(),
            expected_at: s.expected_at.map(|d| d.to_rfc3339()),
            received_at: s.received_at.map(|d| d.to_rfc3339()),
            notes: s.notes,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct InboundShipments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl InboundShipments {
    /// Whether the inbound-shipments backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.inbound_shipments().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateInboundShipmentInput) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::CreateInboundShipmentItem> {
                Ok(stateset_core::CreateInboundShipmentItem {
                    product_id: parse_uuid_str(&i.product_id, "product")?.into(),
                    sku: i.sku,
                    quantity_expected: parse_decimal_str(
                        &i.quantity_expected,
                        "quantity_expected",
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let shipment = commerce
            .inbound_shipments()
            .create(stateset_core::CreateInboundShipment {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier")?,
                purchase_order_id: parse_optional_uuid(
                    input.purchase_order_id,
                    "purchase_order_id",
                )?,
                warehouse_id: parse_optional_uuid(input.warehouse_id, "warehouse_id")?
                    .map(Into::into),
                carrier: input.carrier,
                tracking_number: input.tracking_number,
                expected_at: parse_rfc3339_opt(input.expected_at, "expected_at")?,
                items,
                notes: input.notes,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create inbound shipment: {}", e)))?;
        Ok(shipment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<InboundShipmentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce
            .inbound_shipments()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get inbound shipment: {}", e)))?;
        Ok(shipment.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<InboundShipmentFilterInput>,
    ) -> Result<Vec<InboundShipmentOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::InboundShipmentFilter::default()),
            |f| -> Result<stateset_core::InboundShipmentFilter> {
                Ok(stateset_core::InboundShipmentFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    warehouse_id: parse_optional_uuid(f.warehouse_id, "warehouse_id")?
                        .map(Into::into),
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::InboundShipmentStatus>()
                                .map_err(|_| Error::from_reason("Invalid inbound shipment status"))
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let shipments = commerce
            .inbound_shipments()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list inbound shipments: {}", e)))?;
        Ok(shipments.into_iter().map(Into::into).collect())
    }

    /// Mark a shipment as in transit.
    #[napi]
    pub async fn mark_in_transit(&self, id: String) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce.inbound_shipments().mark_in_transit(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to mark inbound shipment in transit: {}", e))
        })?;
        Ok(shipment.into())
    }

    /// Mark a shipment as arrived.
    #[napi]
    pub async fn mark_arrived(&self, id: String) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce.inbound_shipments().mark_arrived(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to mark inbound shipment arrived: {}", e))
        })?;
        Ok(shipment.into())
    }

    /// Receive a quantity (exact decimal string) against a single line.
    #[napi]
    pub async fn receive_line(
        &self,
        id: String,
        item_id: String,
        quantity: String,
    ) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let item_uuid = parse_uuid_str(&item_id, "inbound shipment item")?;
        let shipment = commerce
            .inbound_shipments()
            .receive_line(uuid.into(), item_uuid.into(), parse_decimal_str(&quantity, "quantity")?)
            .map_err(|e| {
                Error::from_reason(format!("Failed to receive inbound shipment line: {}", e))
            })?;
        Ok(shipment.into())
    }

    /// Cancel an inbound shipment.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<InboundShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "inbound shipment")?;
        let shipment = commerce
            .inbound_shipments()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel inbound shipment: {}", e)))?;
        Ok(shipment.into())
    }
}

// ============================================================================
// Activity logs  (append-only subject history)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RecordActivityInput {
    /// Subject record type (e.g. "sales_order")
    pub subject_type: String,
    pub subject_id: String,
    /// Machine action key (e.g. "status_changed")
    pub action: String,
    pub summary: String,
    /// user, system, integration, agent
    pub actor_kind: Option<String>,
    pub actor: Option<String>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityLogFilterInput {
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub action: Option<String>,
    /// user, system, integration, agent
    pub actor_kind: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityLogEntryOutput {
    pub id: String,
    pub subject_type: String,
    pub subject_id: String,
    pub action: String,
    pub summary: String,
    /// user, system, integration, agent
    pub actor_kind: String,
    pub actor: Option<String>,
    /// Metadata as JSON
    pub metadata: String,
    pub created_at: String,
}

impl From<stateset_core::ActivityLogEntry> for ActivityLogEntryOutput {
    fn from(e: stateset_core::ActivityLogEntry) -> Self {
        Self {
            id: e.id.to_string(),
            subject_type: e.subject_type,
            subject_id: e.subject_id.to_string(),
            action: e.action,
            summary: e.summary,
            actor_kind: e.actor_kind.to_string(),
            actor: e.actor,
            metadata: e.metadata.to_string(),
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

fn parse_actor_kind(s: &str) -> Result<stateset_core::ActorKind> {
    s.parse::<stateset_core::ActorKind>()
        .map_err(|_| Error::from_reason(format!("Invalid actor kind: {}", s)))
}

fn parse_metadata_json(s: Option<String>) -> Result<serde_json::Value> {
    match s {
        Some(s) => serde_json::from_str(&s)
            .map_err(|e| Error::from_reason(format!("Invalid metadata JSON: {}", e))),
        None => Ok(serde_json::Value::Null),
    }
}

#[napi]
pub struct ActivityLogs {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl ActivityLogs {
    /// Whether the activity-logs backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.activity_logs().is_supported())
    }

    /// Record an activity log entry.
    #[napi]
    pub async fn record(&self, input: RecordActivityInput) -> Result<ActivityLogEntryOutput> {
        let commerce = self.commerce.lock().await;
        let actor_kind = match input.actor_kind.as_deref() {
            Some(s) => parse_actor_kind(s)?,
            None => stateset_core::ActorKind::default(),
        };
        let entry = commerce
            .activity_logs()
            .record(stateset_core::RecordActivity {
                subject_type: input.subject_type,
                subject_id: parse_uuid_str(&input.subject_id, "subject_id")?,
                action: input.action,
                summary: input.summary,
                actor_kind,
                actor: input.actor,
                metadata: parse_metadata_json(input.metadata)?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to record activity: {}", e)))?;
        Ok(entry.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ActivityLogEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "activity_log")?;
        let entry = commerce
            .activity_logs()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get activity log entry: {}", e)))?;
        Ok(entry.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<ActivityLogFilterInput>,
    ) -> Result<Vec<ActivityLogEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ActivityLogFilter::default()),
            |f| -> Result<stateset_core::ActivityLogFilter> {
                Ok(stateset_core::ActivityLogFilter {
                    subject_type: f.subject_type,
                    subject_id: parse_optional_uuid(f.subject_id, "subject_id")?,
                    action: f.action,
                    actor_kind: f.actor_kind.as_deref().map(parse_actor_kind).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let entries = commerce
            .activity_logs()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list activity logs: {}", e)))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }

    /// Full history for a single subject, most recent first.
    #[napi]
    pub async fn history_for_subject(
        &self,
        subject_type: String,
        subject_id: String,
    ) -> Result<Vec<ActivityLogEntryOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&subject_id, "subject_id")?;
        let entries = commerce
            .activity_logs()
            .history_for_subject(&subject_type, uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get activity history: {}", e)))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Channels  (sales / fulfillment channels)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateChannelInput {
    pub name: String,
    /// sales_channel, fulfillment_channel, end_to_end_channel
    pub channel_type: String,
    pub integration: Option<String>,
    pub default_warehouse_id: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateChannelInput {
    pub name: Option<String>,
    pub integration: Option<String>,
    /// active, paused, deleted
    pub status: Option<String>,
    pub default_warehouse_id: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelFilterInput {
    /// sales_channel, fulfillment_channel, end_to_end_channel
    pub channel_type: Option<String>,
    /// active, paused, deleted
    pub status: Option<String>,
    pub integration: Option<String>,
    pub api_locked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelProductSyncItemInput {
    pub channel_sku: String,
    pub product_id: Option<String>,
    pub internal_sku: Option<String>,
    /// When true, remove the mapping instead of upserting it
    pub delete: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelOutput {
    pub id: String,
    pub name: String,
    /// sales_channel, fulfillment_channel, end_to_end_channel
    pub channel_type: String,
    pub integration: Option<String>,
    /// active, paused, deleted
    pub status: String,
    pub api_locked: bool,
    pub default_warehouse_id: Option<String>,
    pub tags: Vec<String>,
    /// Metadata as JSON
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Channel> for ChannelOutput {
    fn from(c: stateset_core::Channel) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            channel_type: c.channel_type.to_string(),
            integration: c.integration,
            status: c.status.to_string(),
            api_locked: c.api_locked,
            default_warehouse_id: c.default_warehouse_id.map(|w| w.to_string()),
            tags: c.tags,
            metadata: c.metadata.to_string(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ChannelProductMappingOutput {
    pub channel_id: String,
    pub channel_sku: String,
    pub product_id: String,
    pub internal_sku: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ChannelProductMapping> for ChannelProductMappingOutput {
    fn from(m: stateset_core::ChannelProductMapping) -> Self {
        Self {
            channel_id: m.channel_id.to_string(),
            channel_sku: m.channel_sku,
            product_id: m.product_id.to_string(),
            internal_sku: m.internal_sku,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

fn parse_channel_type(s: &str) -> Result<stateset_core::ChannelType> {
    s.parse::<stateset_core::ChannelType>()
        .map_err(|_| Error::from_reason(format!("Invalid channel type: {}", s)))
}

fn parse_channel_status(s: &str) -> Result<stateset_core::ChannelStatus> {
    s.parse::<stateset_core::ChannelStatus>()
        .map_err(|_| Error::from_reason(format!("Invalid channel status: {}", s)))
}

#[napi]
pub struct Channels {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Channels {
    /// Whether the channels backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.channels().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateChannelInput) -> Result<ChannelOutput> {
        let commerce = self.commerce.lock().await;
        let channel = commerce
            .channels()
            .create(stateset_core::CreateChannel {
                name: input.name,
                channel_type: parse_channel_type(&input.channel_type)?,
                integration: input.integration,
                default_warehouse_id: parse_optional_uuid(
                    input.default_warehouse_id,
                    "default_warehouse_id",
                )?
                .map(Into::into),
                tags: input.tags.unwrap_or_default(),
                metadata: parse_metadata_json(input.metadata)?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create channel: {}", e)))?;
        Ok(channel.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ChannelOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "channel")?;
        let channel = commerce
            .channels()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get channel: {}", e)))?;
        Ok(channel.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateChannelInput) -> Result<ChannelOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "channel")?;
        let channel = commerce
            .channels()
            .update(
                uuid.into(),
                stateset_core::UpdateChannel {
                    name: input.name,
                    integration: input.integration,
                    status: input.status.as_deref().map(parse_channel_status).transpose()?,
                    default_warehouse_id: parse_optional_uuid(
                        input.default_warehouse_id,
                        "default_warehouse_id",
                    )?
                    .map(Into::into),
                    tags: input.tags,
                    metadata: input.metadata.map(Some).map(parse_metadata_json).transpose()?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update channel: {}", e)))?;
        Ok(channel.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<ChannelFilterInput>) -> Result<Vec<ChannelOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ChannelFilter::default()),
            |f| -> Result<stateset_core::ChannelFilter> {
                Ok(stateset_core::ChannelFilter {
                    channel_type: f.channel_type.as_deref().map(parse_channel_type).transpose()?,
                    status: f.status.as_deref().map(parse_channel_status).transpose()?,
                    integration: f.integration,
                    api_locked: f.api_locked,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let channels = commerce
            .channels()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list channels: {}", e)))?;
        Ok(channels.into_iter().map(Into::into).collect())
    }

    /// Soft-delete a channel.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "channel")?;
        commerce
            .channels()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete channel: {}", e)))
    }

    /// Lock or unlock a channel against external mutations.
    #[napi]
    pub async fn set_lock(&self, id: String, locked: bool) -> Result<ChannelOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "channel")?;
        let channel = commerce
            .channels()
            .set_lock(uuid.into(), locked)
            .map_err(|e| Error::from_reason(format!("Failed to set channel lock: {}", e)))?;
        Ok(channel.into())
    }

    /// Bulk upsert/delete channel SKU mappings. Returns the affected count.
    #[napi]
    pub async fn sync_products(
        &self,
        id: String,
        items: Vec<ChannelProductSyncItemInput>,
    ) -> Result<u32> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "channel")?;
        let items = items
            .into_iter()
            .map(|i| -> Result<stateset_core::ChannelProductSyncItem> {
                Ok(stateset_core::ChannelProductSyncItem {
                    channel_sku: i.channel_sku,
                    product_id: parse_optional_uuid(i.product_id, "product_id")?.map(Into::into),
                    internal_sku: i.internal_sku,
                    delete: i.delete.unwrap_or(false),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .channels()
            .sync_products(uuid.into(), items)
            .map_err(|e| Error::from_reason(format!("Failed to sync channel products: {}", e)))?;
        u32::try_from(count).map_err(|_| Error::from_reason("Sync count overflowed u32"))
    }

    /// List a channel's SKU mappings.
    #[napi]
    pub async fn list_product_mappings(
        &self,
        id: String,
    ) -> Result<Vec<ChannelProductMappingOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "channel")?;
        let mappings = commerce.channels().list_product_mappings(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to list channel product mappings: {}", e))
        })?;
        Ok(mappings.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Companies  (B2B accounts and contacts)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCompanyInput {
    pub name: String,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// ISO 4217 currency code
    pub currency: Option<String>,
    pub payment_terms_days: Option<i32>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateCompanyInput {
    pub name: Option<String>,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// ISO 4217 currency code
    pub currency: Option<String>,
    pub payment_terms_days: Option<i32>,
    /// active, inactive
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyFilterInput {
    /// active, inactive
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateContactInput {
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub title: Option<String>,
    pub company_ids: Option<Vec<String>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyOutput {
    pub id: String,
    pub name: String,
    pub reference: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub payment_terms_days: Option<i32>,
    /// active, inactive
    pub status: String,
    pub tags: Vec<String>,
    /// Metadata as JSON
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Company> for CompanyOutput {
    fn from(c: stateset_core::Company) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            reference: c.reference,
            email: c.email,
            phone: c.phone,
            currency: c.currency.to_string(),
            payment_terms_days: c.payment_terms_days,
            status: c.status.to_string(),
            tags: c.tags,
            metadata: c.metadata.to_string(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyShippingAddressOutput {
    pub id: String,
    pub company_id: String,
    pub label: Option<String>,
    pub name: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CompanyShippingAddress> for CompanyShippingAddressOutput {
    fn from(a: stateset_core::CompanyShippingAddress) -> Self {
        Self {
            id: a.id.to_string(),
            company_id: a.company_id.to_string(),
            label: a.label,
            name: a.name,
            line1: a.line1,
            line2: a.line2,
            city: a.city,
            region: a.region,
            postal_code: a.postal_code,
            country: a.country,
            is_default: a.is_default,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CompanyPriceOverrideOutput {
    pub company_id: String,
    pub product_id: String,
    /// Exact decimal string
    pub price: String,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::CompanyPriceOverride> for CompanyPriceOverrideOutput {
    fn from(o: stateset_core::CompanyPriceOverride) -> Self {
        Self {
            company_id: o.company_id.to_string(),
            product_id: o.product_id.to_string(),
            price: o.price.to_string(),
            currency: o.currency.to_string(),
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ContactOutput {
    pub id: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub title: Option<String>,
    pub company_ids: Vec<String>,
    pub portal_enabled: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Contact> for ContactOutput {
    fn from(c: stateset_core::Contact) -> Self {
        Self {
            id: c.id.to_string(),
            first_name: c.first_name,
            last_name: c.last_name,
            email: c.email,
            phone: c.phone,
            title: c.title,
            company_ids: c.company_ids.iter().map(ToString::to_string).collect(),
            portal_enabled: c.portal_enabled,
            is_active: c.is_active,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

fn parse_company_status(s: &str) -> Result<stateset_core::CompanyStatus> {
    s.parse::<stateset_core::CompanyStatus>()
        .map_err(|_| Error::from_reason(format!("Invalid company status: {}", s)))
}

#[napi]
pub struct Companies {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Companies {
    /// Whether the companies backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.companies().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateCompanyInput) -> Result<CompanyOutput> {
        let commerce = self.commerce.lock().await;
        let company = commerce
            .companies()
            .create(stateset_core::CreateCompany {
                name: input.name,
                reference: input.reference,
                email: input.email,
                phone: input.phone,
                currency: parse_currency_opt(input.currency)?,
                payment_terms_days: input.payment_terms_days,
                tags: input.tags.unwrap_or_default(),
                metadata: parse_metadata_json(input.metadata)?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create company: {}", e)))?;
        Ok(company.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CompanyOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "company")?;
        let company = commerce
            .companies()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get company: {}", e)))?;
        Ok(company.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateCompanyInput) -> Result<CompanyOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "company")?;
        let company = commerce
            .companies()
            .update(
                uuid.into(),
                stateset_core::UpdateCompany {
                    name: input.name,
                    reference: input.reference,
                    email: input.email,
                    phone: input.phone,
                    currency: parse_currency_opt(input.currency)?,
                    payment_terms_days: input.payment_terms_days,
                    status: input.status.as_deref().map(parse_company_status).transpose()?,
                    tags: input.tags,
                    metadata: input.metadata.map(Some).map(parse_metadata_json).transpose()?,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update company: {}", e)))?;
        Ok(company.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<CompanyFilterInput>) -> Result<Vec<CompanyOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::CompanyFilter::default()),
            |f| -> Result<stateset_core::CompanyFilter> {
                Ok(stateset_core::CompanyFilter {
                    status: f.status.as_deref().map(parse_company_status).transpose()?,
                    search: f.search,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let companies = commerce
            .companies()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list companies: {}", e)))?;
        Ok(companies.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "company")?;
        commerce
            .companies()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete company: {}", e)))
    }

    /// List a company's shipping addresses.
    #[napi]
    pub async fn list_addresses(&self, id: String) -> Result<Vec<CompanyShippingAddressOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "company")?;
        let addresses = commerce
            .companies()
            .list_addresses(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to list company addresses: {}", e)))?;
        Ok(addresses.into_iter().map(Into::into).collect())
    }

    /// List a company's product price overrides.
    #[napi]
    pub async fn list_price_overrides(
        &self,
        id: String,
    ) -> Result<Vec<CompanyPriceOverrideOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "company")?;
        let overrides = commerce.companies().list_price_overrides(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to list company price overrides: {}", e))
        })?;
        Ok(overrides.into_iter().map(Into::into).collect())
    }

    /// Create a contact linked to one or more companies.
    #[napi]
    pub async fn create_contact(&self, input: CreateContactInput) -> Result<ContactOutput> {
        let commerce = self.commerce.lock().await;
        let company_ids = input
            .company_ids
            .unwrap_or_default()
            .iter()
            .map(|id| Ok(parse_uuid_str(id, "company_id")?.into()))
            .collect::<Result<Vec<_>>>()?;
        let contact = commerce
            .companies()
            .create_contact(stateset_core::CreateContact {
                first_name: input.first_name,
                last_name: input.last_name,
                email: input.email,
                phone: input.phone,
                title: input.title,
                company_ids,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create contact: {}", e)))?;
        Ok(contact.into())
    }

    #[napi]
    pub async fn get_contact(&self, id: String) -> Result<Option<ContactOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "contact")?;
        let contact = commerce
            .companies()
            .get_contact(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get contact: {}", e)))?;
        Ok(contact.map(Into::into))
    }

    /// List contacts for a company.
    #[napi]
    pub async fn list_contacts(&self, company_id: String) -> Result<Vec<ContactOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&company_id, "company")?;
        let contacts = commerce
            .companies()
            .list_contacts(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to list contacts: {}", e)))?;
        Ok(contacts.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Units of measure  (unit classes, UOMs, conversion rules)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateUnitClassInput {
    pub name: String,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateUnitOfMeasureInput {
    pub unit_class_id: String,
    pub name: String,
    pub abbreviation: String,
    /// Exact decimal string relative to the class base unit
    pub factor: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitOfMeasureFilterInput {
    pub class_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateUnitConversionRuleInput {
    /// SYSTEM or SKU
    pub rule_type: String,
    pub product_id: Option<String>,
    pub from_uom_id: String,
    pub to_uom_id: String,
    /// Exact decimal string
    pub factor: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitClassOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub base_uom_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::UnitClass> for UnitClassOutput {
    fn from(c: stateset_core::UnitClass) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            description: c.description,
            base_uom_id: c.base_uom_id.map(|id| id.to_string()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitOfMeasureOutput {
    pub id: String,
    pub unit_class_id: String,
    pub name: String,
    pub abbreviation: String,
    /// Exact decimal string
    pub factor: String,
    pub is_base: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::UnitOfMeasure> for UnitOfMeasureOutput {
    fn from(u: stateset_core::UnitOfMeasure) -> Self {
        Self {
            id: u.id.to_string(),
            unit_class_id: u.unit_class_id.to_string(),
            name: u.name,
            abbreviation: u.abbreviation,
            factor: u.factor.to_string(),
            is_base: u.is_base,
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitConversionRuleOutput {
    pub id: String,
    /// SYSTEM or SKU
    pub rule_type: String,
    pub product_id: Option<String>,
    pub from_uom_id: String,
    pub to_uom_id: String,
    /// Exact decimal string
    pub factor: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::UnitConversionRule> for UnitConversionRuleOutput {
    fn from(r: stateset_core::UnitConversionRule) -> Self {
        Self {
            id: r.id.to_string(),
            rule_type: r.rule_type.to_string(),
            product_id: r.product_id.map(|id| id.to_string()),
            from_uom_id: r.from_uom_id.to_string(),
            to_uom_id: r.to_uom_id.to_string(),
            factor: r.factor.to_string(),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

fn parse_conversion_rule_type(s: &str) -> Result<stateset_core::ConversionRuleType> {
    s.parse::<stateset_core::ConversionRuleType>()
        .map_err(|_| Error::from_reason(format!("Invalid conversion rule type: {}", s)))
}

#[napi]
pub struct UnitsOfMeasure {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl UnitsOfMeasure {
    /// Whether the units-of-measure backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.units_of_measure().is_supported())
    }

    #[napi]
    pub async fn create_class(&self, input: CreateUnitClassInput) -> Result<UnitClassOutput> {
        let commerce = self.commerce.lock().await;
        let class = commerce
            .units_of_measure()
            .create_class(stateset_core::CreateUnitClass {
                name: input.name,
                description: input.description,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create unit class: {}", e)))?;
        Ok(class.into())
    }

    #[napi]
    pub async fn list_classes(&self) -> Result<Vec<UnitClassOutput>> {
        let commerce = self.commerce.lock().await;
        let classes = commerce
            .units_of_measure()
            .list_classes()
            .map_err(|e| Error::from_reason(format!("Failed to list unit classes: {}", e)))?;
        Ok(classes.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_class(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "unit_class")?;
        commerce
            .units_of_measure()
            .delete_class(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete unit class: {}", e)))
    }

    #[napi]
    pub async fn create_uom(&self, input: CreateUnitOfMeasureInput) -> Result<UnitOfMeasureOutput> {
        let commerce = self.commerce.lock().await;
        let uom = commerce
            .units_of_measure()
            .create_uom(stateset_core::CreateUnitOfMeasure {
                unit_class_id: parse_uuid_str(&input.unit_class_id, "unit_class_id")?.into(),
                name: input.name,
                abbreviation: input.abbreviation,
                factor: parse_decimal_str(&input.factor, "factor")?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create unit of measure: {}", e)))?;
        Ok(uom.into())
    }

    #[napi]
    pub async fn list_uoms(
        &self,
        filter: Option<UnitOfMeasureFilterInput>,
    ) -> Result<Vec<UnitOfMeasureOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::UnitOfMeasureFilter::default()),
            |f| -> Result<stateset_core::UnitOfMeasureFilter> {
                Ok(stateset_core::UnitOfMeasureFilter {
                    class_id: parse_optional_uuid(f.class_id, "class_id")?.map(Into::into),
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let uoms = commerce
            .units_of_measure()
            .list_uoms(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list units of measure: {}", e)))?;
        Ok(uoms.into_iter().map(Into::into).collect())
    }

    /// Mark a UOM as the base unit for its class.
    #[napi]
    pub async fn set_base_uom(&self, id: String) -> Result<UnitOfMeasureOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "unit_of_measure")?;
        let uom = commerce.units_of_measure().set_base_uom(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to set base unit of measure: {}", e))
        })?;
        Ok(uom.into())
    }

    #[napi]
    pub async fn delete_uom(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "unit_of_measure")?;
        commerce
            .units_of_measure()
            .delete_uom(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete unit of measure: {}", e)))
    }

    #[napi]
    pub async fn create_rule(
        &self,
        input: CreateUnitConversionRuleInput,
    ) -> Result<UnitConversionRuleOutput> {
        let commerce = self.commerce.lock().await;
        let rule = commerce
            .units_of_measure()
            .create_rule(stateset_core::CreateUnitConversionRule {
                rule_type: parse_conversion_rule_type(&input.rule_type)?,
                product_id: parse_optional_uuid(input.product_id, "product_id")?.map(Into::into),
                from_uom_id: parse_uuid_str(&input.from_uom_id, "from_uom_id")?.into(),
                to_uom_id: parse_uuid_str(&input.to_uom_id, "to_uom_id")?.into(),
                factor: parse_decimal_str(&input.factor, "factor")?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create conversion rule: {}", e)))?;
        Ok(rule.into())
    }

    #[napi]
    pub async fn list_rules(&self) -> Result<Vec<UnitConversionRuleOutput>> {
        let commerce = self.commerce.lock().await;
        let rules = commerce
            .units_of_measure()
            .list_rules()
            .map_err(|e| Error::from_reason(format!("Failed to list conversion rules: {}", e)))?;
        Ok(rules.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_rule(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "unit_conversion_rule")?;
        commerce
            .units_of_measure()
            .delete_rule(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete conversion rule: {}", e)))
    }
}

// ============================================================================
// Shipping zones  (geographic zones + zone shipping methods and rates)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateShippingZoneInput {
    pub name: String,
    pub countries: Option<Vec<String>>,
    pub regions: Option<Vec<String>>,
    pub postal_codes: Option<Vec<String>>,
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateShippingZoneInput {
    pub name: Option<String>,
    pub countries: Option<Vec<String>>,
    pub regions: Option<Vec<String>>,
    pub postal_codes: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingZoneFilterInput {
    pub country: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingConditionInput {
    /// Exact decimal string
    pub min_weight: Option<String>,
    /// Exact decimal string
    pub max_weight: Option<String>,
    /// Exact decimal string
    pub min_price: Option<String>,
    /// Exact decimal string
    pub max_price: Option<String>,
    /// Exact decimal string
    pub rate: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateZoneShippingMethodInput {
    pub zone_id: String,
    pub name: String,
    pub carrier: Option<String>,
    /// flat, weight_based, price_based, calculated, free
    pub method_type: String,
    /// Exact decimal string
    pub base_rate: String,
    /// ISO 4217 currency code
    pub currency: String,
    pub min_delivery_days: Option<i32>,
    pub max_delivery_days: Option<i32>,
    pub conditions: Option<Vec<ShippingConditionInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingMethodFilterInput {
    pub zone_id: Option<String>,
    pub carrier: Option<String>,
    /// flat, weight_based, price_based, calculated, free
    pub method_type: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingRateRequestInput {
    pub country: String,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    /// Exact decimal string
    pub weight: Option<String>,
    /// Exact decimal string
    pub order_total: Option<String>,
    /// ISO 4217 currency code
    pub currency: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingZoneOutput {
    pub id: String,
    pub name: String,
    pub countries: Vec<String>,
    pub regions: Vec<String>,
    pub postal_codes: Vec<String>,
    pub priority: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ShippingZone> for ShippingZoneOutput {
    fn from(z: stateset_core::ShippingZone) -> Self {
        Self {
            id: z.id.to_string(),
            name: z.name,
            countries: z.countries,
            regions: z.regions,
            postal_codes: z.postal_codes,
            priority: z.priority,
            is_active: z.is_active,
            created_at: z.created_at.to_rfc3339(),
            updated_at: z.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ShippingConditionOutput {
    /// Exact decimal string
    pub min_weight: Option<String>,
    /// Exact decimal string
    pub max_weight: Option<String>,
    /// Exact decimal string
    pub min_price: Option<String>,
    /// Exact decimal string
    pub max_price: Option<String>,
    /// Exact decimal string
    pub rate: String,
}

impl From<stateset_core::ShippingCondition> for ShippingConditionOutput {
    fn from(c: stateset_core::ShippingCondition) -> Self {
        Self {
            min_weight: c.min_weight.map(|d| d.to_string()),
            max_weight: c.max_weight.map(|d| d.to_string()),
            min_price: c.min_price.map(|d| d.to_string()),
            max_price: c.max_price.map(|d| d.to_string()),
            rate: c.rate.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingMethodOutput {
    pub id: String,
    pub zone_id: String,
    pub name: String,
    pub carrier: Option<String>,
    /// flat, weight_based, price_based, calculated, free
    pub method_type: String,
    /// Exact decimal string
    pub base_rate: String,
    pub currency: String,
    pub min_delivery_days: Option<i32>,
    pub max_delivery_days: Option<i32>,
    pub conditions: Vec<ShippingConditionOutput>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::ZoneShippingMethod> for ZoneShippingMethodOutput {
    fn from(m: stateset_core::ZoneShippingMethod) -> Self {
        Self {
            id: m.id.to_string(),
            zone_id: m.zone_id.to_string(),
            name: m.name,
            carrier: m.carrier,
            method_type: m.method_type.to_string(),
            base_rate: m.base_rate.to_string(),
            currency: m.currency.to_string(),
            min_delivery_days: m.min_delivery_days,
            max_delivery_days: m.max_delivery_days,
            conditions: m.conditions.into_iter().map(Into::into).collect(),
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ZoneShippingRateOutput {
    pub method_id: String,
    pub method_name: String,
    pub carrier: Option<String>,
    /// Exact decimal string
    pub rate: String,
    pub currency: String,
    pub min_delivery_days: Option<i32>,
    pub max_delivery_days: Option<i32>,
}

impl From<stateset_core::ZoneShippingRate> for ZoneShippingRateOutput {
    fn from(r: stateset_core::ZoneShippingRate) -> Self {
        Self {
            method_id: r.method_id.to_string(),
            method_name: r.method_name,
            carrier: r.carrier,
            rate: r.rate.to_string(),
            currency: r.currency.to_string(),
            min_delivery_days: r.min_delivery_days,
            max_delivery_days: r.max_delivery_days,
        }
    }
}

fn parse_shipping_method_type(s: &str) -> Result<stateset_core::ShippingMethodType> {
    s.parse::<stateset_core::ShippingMethodType>()
        .map_err(|_| Error::from_reason(format!("Invalid shipping method type: {}", s)))
}

fn parse_currency_required(s: &str) -> Result<CurrencyCode> {
    s.parse::<CurrencyCode>()
        .map_err(|_| Error::from_reason(format!("Invalid currency code: {}", s)))
}

fn build_shipping_conditions(
    conditions: Option<Vec<ShippingConditionInput>>,
) -> Result<Vec<stateset_core::ShippingCondition>> {
    conditions
        .unwrap_or_default()
        .into_iter()
        .map(|c| -> Result<stateset_core::ShippingCondition> {
            Ok(stateset_core::ShippingCondition {
                min_weight: parse_optional_decimal_str(c.min_weight, "min_weight")?,
                max_weight: parse_optional_decimal_str(c.max_weight, "max_weight")?,
                min_price: parse_optional_decimal_str(c.min_price, "min_price")?,
                max_price: parse_optional_decimal_str(c.max_price, "max_price")?,
                rate: parse_decimal_str(&c.rate, "rate")?,
            })
        })
        .collect()
}

#[napi]
pub struct ShippingZones {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl ShippingZones {
    /// Whether the shipping-zones backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.shipping_zones().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateShippingZoneInput) -> Result<ShippingZoneOutput> {
        let commerce = self.commerce.lock().await;
        let zone = commerce
            .shipping_zones()
            .create(stateset_core::CreateShippingZone {
                name: input.name,
                countries: input.countries.unwrap_or_default(),
                regions: input.regions.unwrap_or_default(),
                postal_codes: input.postal_codes.unwrap_or_default(),
                priority: input.priority,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create shipping zone: {}", e)))?;
        Ok(zone.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ShippingZoneOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "shipping_zone")?;
        let zone = commerce
            .shipping_zones()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get shipping zone: {}", e)))?;
        Ok(zone.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateShippingZoneInput,
    ) -> Result<ShippingZoneOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "shipping_zone")?;
        let zone = commerce
            .shipping_zones()
            .update(
                uuid.into(),
                stateset_core::UpdateShippingZone {
                    name: input.name,
                    countries: input.countries,
                    regions: input.regions,
                    postal_codes: input.postal_codes,
                    priority: input.priority,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update shipping zone: {}", e)))?;
        Ok(zone.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<ShippingZoneFilterInput>,
    ) -> Result<Vec<ShippingZoneOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::ShippingZoneFilter::default, |f| {
            stateset_core::ShippingZoneFilter {
                country: f.country,
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let zones = commerce
            .shipping_zones()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list shipping zones: {}", e)))?;
        Ok(zones.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "shipping_zone")?;
        commerce
            .shipping_zones()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete shipping zone: {}", e)))
    }

    /// Find zones whose geographic criteria match a destination.
    #[napi]
    pub async fn find_matching_zones(
        &self,
        country: String,
        region: Option<String>,
        postal_code: Option<String>,
    ) -> Result<Vec<ShippingZoneOutput>> {
        let commerce = self.commerce.lock().await;
        let zones = commerce
            .shipping_zones()
            .find_matching_zones(&country, region.as_deref(), postal_code.as_deref())
            .map_err(|e| Error::from_reason(format!("Failed to find matching zones: {}", e)))?;
        Ok(zones.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn create_method(
        &self,
        input: CreateZoneShippingMethodInput,
    ) -> Result<ZoneShippingMethodOutput> {
        let commerce = self.commerce.lock().await;
        let method = commerce
            .shipping_zones()
            .create_method(stateset_core::CreateZoneShippingMethod {
                zone_id: parse_uuid_str(&input.zone_id, "zone_id")?.into(),
                name: input.name,
                carrier: input.carrier,
                method_type: parse_shipping_method_type(&input.method_type)?,
                base_rate: parse_decimal_str(&input.base_rate, "base_rate")?,
                currency: parse_currency_required(&input.currency)?,
                min_delivery_days: input.min_delivery_days,
                max_delivery_days: input.max_delivery_days,
                conditions: build_shipping_conditions(input.conditions)?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create shipping method: {}", e)))?;
        Ok(method.into())
    }

    #[napi]
    pub async fn get_method(&self, id: String) -> Result<Option<ZoneShippingMethodOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "shipping_method")?;
        let method = commerce
            .shipping_zones()
            .get_method(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get shipping method: {}", e)))?;
        Ok(method.map(Into::into))
    }

    #[napi]
    pub async fn list_methods(
        &self,
        filter: Option<ZoneShippingMethodFilterInput>,
    ) -> Result<Vec<ZoneShippingMethodOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ZoneShippingMethodFilter::default()),
            |f| -> Result<stateset_core::ZoneShippingMethodFilter> {
                Ok(stateset_core::ZoneShippingMethodFilter {
                    zone_id: parse_optional_uuid(f.zone_id, "zone_id")?.map(Into::into),
                    carrier: f.carrier,
                    method_type: f
                        .method_type
                        .as_deref()
                        .map(parse_shipping_method_type)
                        .transpose()?,
                    is_active: f.is_active,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let methods = commerce
            .shipping_zones()
            .list_methods(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list shipping methods: {}", e)))?;
        Ok(methods.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_method(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "shipping_method")?;
        commerce
            .shipping_zones()
            .delete_method(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete shipping method: {}", e)))
    }

    /// Calculate available shipping rates for a destination.
    #[napi]
    pub async fn calculate_rates(
        &self,
        request: ZoneShippingRateRequestInput,
    ) -> Result<Vec<ZoneShippingRateOutput>> {
        let commerce = self.commerce.lock().await;
        let rates = commerce
            .shipping_zones()
            .calculate_rates(stateset_core::ZoneShippingRateRequest {
                country: request.country,
                region: request.region,
                postal_code: request.postal_code,
                weight: parse_optional_decimal_str(request.weight, "weight")?,
                order_total: parse_optional_decimal_str(request.order_total, "order_total")?,
                currency: parse_currency_required(&request.currency)?,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to calculate shipping rates: {}", e))
            })?;
        Ok(rates.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Stock snapshots  (point-in-time inventory)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptureStockLineInput {
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_on_hand: String,
    /// Exact decimal string
    pub quantity_available: String,
    pub location: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptureStockSnapshotInput {
    pub label: Option<String>,
    pub lines: Vec<CaptureStockLineInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockSnapshotFilterInput {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockSnapshotLineOutput {
    pub id: String,
    pub stock_snapshot_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity_on_hand: String,
    /// Exact decimal string
    pub quantity_available: String,
    pub location: Option<String>,
}

impl From<stateset_core::StockSnapshotLine> for StockSnapshotLineOutput {
    fn from(l: stateset_core::StockSnapshotLine) -> Self {
        Self {
            id: l.id.to_string(),
            stock_snapshot_id: l.stock_snapshot_id.to_string(),
            product_id: l.product_id.to_string(),
            sku: l.sku,
            quantity_on_hand: l.quantity_on_hand.to_string(),
            quantity_available: l.quantity_available.to_string(),
            location: l.location,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockSnapshotOutput {
    pub id: String,
    pub label: Option<String>,
    pub total_skus: String,
    /// Exact decimal string
    pub total_units: String,
    pub lines: Vec<StockSnapshotLineOutput>,
    pub captured_at: String,
}

impl From<stateset_core::StockSnapshot> for StockSnapshotOutput {
    fn from(s: stateset_core::StockSnapshot) -> Self {
        Self {
            id: s.id.to_string(),
            label: s.label,
            total_skus: s.total_skus.to_string(),
            total_units: s.total_units.to_string(),
            lines: s.lines.into_iter().map(Into::into).collect(),
            captured_at: s.captured_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct StockSnapshots {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl StockSnapshots {
    /// Whether the stock-snapshots backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.stock_snapshots().is_supported())
    }

    /// Capture a new snapshot; totals are computed from the supplied lines.
    #[napi]
    pub async fn capture(&self, input: CaptureStockSnapshotInput) -> Result<StockSnapshotOutput> {
        let commerce = self.commerce.lock().await;
        let lines = input
            .lines
            .into_iter()
            .map(|l| -> Result<stateset_core::CaptureStockLine> {
                Ok(stateset_core::CaptureStockLine {
                    product_id: parse_uuid_str(&l.product_id, "product_id")?.into(),
                    sku: l.sku,
                    quantity_on_hand: parse_decimal_str(&l.quantity_on_hand, "quantity_on_hand")?,
                    quantity_available: parse_decimal_str(
                        &l.quantity_available,
                        "quantity_available",
                    )?,
                    location: l.location,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let snapshot = commerce
            .stock_snapshots()
            .capture(stateset_core::CaptureStockSnapshot { label: input.label, lines })
            .map_err(|e| Error::from_reason(format!("Failed to capture stock snapshot: {}", e)))?;
        Ok(snapshot.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<StockSnapshotOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "stock_snapshot")?;
        let snapshot = commerce
            .stock_snapshots()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get stock snapshot: {}", e)))?;
        Ok(snapshot.map(Into::into))
    }

    /// Most recent snapshot, if any.
    #[napi]
    pub async fn latest(&self) -> Result<Option<StockSnapshotOutput>> {
        let commerce = self.commerce.lock().await;
        let snapshot = commerce.stock_snapshots().latest().map_err(|e| {
            Error::from_reason(format!("Failed to get latest stock snapshot: {}", e))
        })?;
        Ok(snapshot.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<StockSnapshotFilterInput>,
    ) -> Result<Vec<StockSnapshotOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::StockSnapshotFilter::default, |f| {
            stateset_core::StockSnapshotFilter { limit: f.limit, offset: f.offset }
        });
        let snapshots = commerce
            .stock_snapshots()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list stock snapshots: {}", e)))?;
        Ok(snapshots.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "stock_snapshot")?;
        commerce
            .stock_snapshots()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete stock snapshot: {}", e)))
    }
}

// ============================================================================
// Print stations  (paired agents + print job queue)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePrintStationInput {
    pub name: String,
    pub printers: Option<Vec<String>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EnqueuePrintJobInput {
    pub printer_name: Option<String>,
    /// zpl or pdf
    pub payload_kind: Option<String>,
    pub payload: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrintJobFilterInput {
    /// queued, picked_up, printed, failed
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrintStationOutput {
    pub id: String,
    pub name: String,
    pub printers: Vec<String>,
    pub revoked: bool,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PrintStation> for PrintStationOutput {
    fn from(s: stateset_core::PrintStation) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            printers: s.printers,
            revoked: s.revoked,
            last_seen_at: s.last_seen_at.map(|d| d.to_rfc3339()),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PairStationResultOutput {
    pub station: PrintStationOutput,
    /// One-time pairing token; shown only at pairing time
    pub token: String,
}

impl From<stateset_core::PairStationResult> for PairStationResultOutput {
    fn from(r: stateset_core::PairStationResult) -> Self {
        Self { station: r.station.into(), token: r.token }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrintJobOutput {
    pub id: String,
    pub station_id: String,
    pub printer_name: Option<String>,
    /// zpl or pdf
    pub payload_kind: String,
    pub payload: String,
    /// queued, picked_up, printed, failed
    pub status: String,
    pub created_at: String,
    pub picked_up_at: Option<String>,
}

impl From<stateset_core::PrintJob> for PrintJobOutput {
    fn from(j: stateset_core::PrintJob) -> Self {
        Self {
            id: j.id.to_string(),
            station_id: j.station_id.to_string(),
            printer_name: j.printer_name,
            payload_kind: j.payload_kind.to_string(),
            payload: j.payload,
            status: j.status.to_string(),
            created_at: j.created_at.to_rfc3339(),
            picked_up_at: j.picked_up_at.map(|d| d.to_rfc3339()),
        }
    }
}

fn parse_print_payload_kind(s: &str) -> Result<stateset_core::PrintPayloadKind> {
    s.parse::<stateset_core::PrintPayloadKind>()
        .map_err(|_| Error::from_reason(format!("Invalid print payload kind: {}", s)))
}

fn parse_print_job_status(s: &str) -> Result<stateset_core::PrintJobStatus> {
    s.parse::<stateset_core::PrintJobStatus>()
        .map_err(|_| Error::from_reason(format!("Invalid print job status: {}", s)))
}

#[napi]
pub struct PrintStations {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl PrintStations {
    /// Whether the print-stations backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.print_stations().is_supported())
    }

    /// Pair a new station, returning the station and its one-time token.
    #[napi]
    pub async fn pair(&self, input: CreatePrintStationInput) -> Result<PairStationResultOutput> {
        let commerce = self.commerce.lock().await;
        let result = commerce
            .print_stations()
            .pair(stateset_core::CreatePrintStation {
                name: input.name,
                printers: input.printers.unwrap_or_default(),
            })
            .map_err(|e| Error::from_reason(format!("Failed to pair print station: {}", e)))?;
        Ok(result.into())
    }

    #[napi]
    pub async fn list_stations(&self) -> Result<Vec<PrintStationOutput>> {
        let commerce = self.commerce.lock().await;
        let stations = commerce
            .print_stations()
            .list_stations()
            .map_err(|e| Error::from_reason(format!("Failed to list print stations: {}", e)))?;
        Ok(stations.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn get_station(&self, id: String) -> Result<Option<PrintStationOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "print_station")?;
        let station = commerce
            .print_stations()
            .get_station(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get print station: {}", e)))?;
        Ok(station.map(Into::into))
    }

    #[napi]
    pub async fn revoke_station(&self, id: String) -> Result<PrintStationOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "print_station")?;
        let station = commerce
            .print_stations()
            .revoke_station(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to revoke print station: {}", e)))?;
        Ok(station.into())
    }

    #[napi]
    pub async fn enqueue_job(
        &self,
        station_id: String,
        input: EnqueuePrintJobInput,
    ) -> Result<PrintJobOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&station_id, "print_station")?;
        let payload_kind = match input.payload_kind.as_deref() {
            Some(s) => parse_print_payload_kind(s)?,
            None => stateset_core::PrintPayloadKind::default(),
        };
        let job = commerce
            .print_stations()
            .enqueue_job(
                uuid.into(),
                stateset_core::EnqueuePrintJob {
                    printer_name: input.printer_name,
                    payload_kind,
                    payload: input.payload,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to enqueue print job: {}", e)))?;
        Ok(job.into())
    }

    /// Pick up the next queued job for a station.
    #[napi]
    pub async fn next_job(&self, station_id: String) -> Result<Option<PrintJobOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&station_id, "print_station")?;
        let job = commerce
            .print_stations()
            .next_job(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get next print job: {}", e)))?;
        Ok(job.map(Into::into))
    }

    /// Mark a job printed (success) or failed.
    #[napi]
    pub async fn complete_job(&self, job_id: String, success: bool) -> Result<PrintJobOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&job_id, "print_job")?;
        let job = commerce
            .print_stations()
            .complete_job(uuid.into(), success)
            .map_err(|e| Error::from_reason(format!("Failed to complete print job: {}", e)))?;
        Ok(job.into())
    }

    #[napi]
    pub async fn list_jobs(
        &self,
        station_id: String,
        filter: Option<PrintJobFilterInput>,
    ) -> Result<Vec<PrintJobOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&station_id, "print_station")?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PrintJobFilter::default()),
            |f| -> Result<stateset_core::PrintJobFilter> {
                Ok(stateset_core::PrintJobFilter {
                    status: f.status.as_deref().map(parse_print_job_status).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let jobs = commerce
            .print_stations()
            .list_jobs(uuid.into(), filter)
            .map_err(|e| Error::from_reason(format!("Failed to list print jobs: {}", e)))?;
        Ok(jobs.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Integration Mappings
// ============================================================================

fn parse_naive_date(s: &str, field: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| Error::from_reason(format!("Invalid {field} date (expected YYYY-MM-DD)")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateIntegrationMappingInput {
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
    pub internal_value: String,
}

impl From<CreateIntegrationMappingInput> for stateset_core::CreateIntegrationMapping {
    fn from(i: CreateIntegrationMappingInput) -> Self {
        Self {
            integration: i.integration,
            mapping_group: i.mapping_group,
            field_name: i.field_name,
            external_value: i.external_value,
            internal_value: i.internal_value,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateIntegrationMappingInput {
    pub internal_value: Option<String>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationMappingFilterInput {
    pub integration: Option<String>,
    pub mapping_group: Option<String>,
    pub field_name: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct MappingLookupInput {
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationMappingOutput {
    pub id: String,
    pub integration: String,
    pub mapping_group: String,
    pub field_name: String,
    pub external_value: String,
    pub internal_value: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::IntegrationMapping> for IntegrationMappingOutput {
    fn from(m: stateset_core::IntegrationMapping) -> Self {
        Self {
            id: m.id.to_string(),
            integration: m.integration,
            mapping_group: m.mapping_group,
            field_name: m.field_name,
            external_value: m.external_value,
            internal_value: m.internal_value,
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct IntegrationMappings {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl IntegrationMappings {
    /// Whether the integration-mappings backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.integration_mappings().is_supported())
    }

    #[napi]
    pub async fn create(
        &self,
        input: CreateIntegrationMappingInput,
    ) -> Result<IntegrationMappingOutput> {
        let commerce = self.commerce.lock().await;
        let mapping = commerce.integration_mappings().create(input.into()).map_err(|e| {
            Error::from_reason(format!("Failed to create integration mapping: {}", e))
        })?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<IntegrationMappingOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "integration_mapping")?;
        let mapping = commerce
            .integration_mappings()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get integration mapping: {}", e)))?;
        Ok(mapping.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateIntegrationMappingInput,
    ) -> Result<IntegrationMappingOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "integration_mapping")?;
        let mapping = commerce
            .integration_mappings()
            .update(
                uuid.into(),
                stateset_core::UpdateIntegrationMapping {
                    internal_value: input.internal_value,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| {
                Error::from_reason(format!("Failed to update integration mapping: {}", e))
            })?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<IntegrationMappingFilterInput>,
    ) -> Result<Vec<IntegrationMappingOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::IntegrationMappingFilter::default, |f| {
            stateset_core::IntegrationMappingFilter {
                integration: f.integration,
                mapping_group: f.mapping_group,
                field_name: f.field_name,
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let mappings = commerce.integration_mappings().list(filter).map_err(|e| {
            Error::from_reason(format!("Failed to list integration mappings: {}", e))
        })?;
        Ok(mappings.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "integration_mapping")?;
        commerce
            .integration_mappings()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete integration mapping: {}", e)))
    }

    /// Bulk upsert mappings; returns the number of rows affected as a string.
    #[napi]
    pub async fn bulk_upsert(&self, items: Vec<CreateIntegrationMappingInput>) -> Result<String> {
        let commerce = self.commerce.lock().await;
        let affected = commerce
            .integration_mappings()
            .bulk_upsert(items.into_iter().map(Into::into).collect())
            .map_err(|e| {
                Error::from_reason(format!("Failed to bulk upsert integration mappings: {}", e))
            })?;
        Ok(affected.to_string())
    }

    /// Resolve the internal value for an external value.
    #[napi]
    pub async fn resolve(&self, lookup: MappingLookupInput) -> Result<Option<String>> {
        let commerce = self.commerce.lock().await;
        commerce
            .integration_mappings()
            .resolve(&stateset_core::MappingLookup {
                integration: lookup.integration,
                mapping_group: lookup.mapping_group,
                field_name: lookup.field_name,
                external_value: lookup.external_value,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to resolve integration mapping: {}", e))
            })
    }
}

// ============================================================================
// Integration Field Mappings
// ============================================================================

fn parse_field_transform(s: &str) -> Result<stateset_core::FieldTransform> {
    s.parse::<stateset_core::FieldTransform>()
        .map_err(|_| Error::from_reason(format!("Invalid field transform: {}", s)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateIntegrationFieldMappingInput {
    pub integration_account: String,
    pub mapping_group: String,
    pub source_field: String,
    pub destination_field: String,
    pub template: Option<String>,
    /// Snake-case transform: `none`, `uppercase`, `lowercase`, `trim`
    pub transform: Option<String>,
    pub fallback: Option<String>,
}

impl TryFrom<CreateIntegrationFieldMappingInput> for stateset_core::CreateIntegrationFieldMapping {
    type Error = Error;

    fn try_from(i: CreateIntegrationFieldMappingInput) -> Result<Self> {
        Ok(Self {
            integration_account: i.integration_account,
            mapping_group: i.mapping_group,
            source_field: i.source_field,
            destination_field: i.destination_field,
            template: i.template,
            transform: i
                .transform
                .as_deref()
                .map(parse_field_transform)
                .transpose()?
                .unwrap_or_default(),
            fallback: i.fallback,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateIntegrationFieldMappingInput {
    pub destination_field: Option<String>,
    pub template: Option<String>,
    pub transform: Option<String>,
    pub fallback: Option<String>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationFieldMappingFilterInput {
    pub integration_account: Option<String>,
    pub mapping_group: Option<String>,
    pub source_field: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IntegrationFieldMappingOutput {
    pub id: String,
    pub integration_account: String,
    pub mapping_group: String,
    pub source_field: String,
    pub destination_field: String,
    pub template: Option<String>,
    /// Snake-case transform
    pub transform: String,
    pub fallback: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::IntegrationFieldMapping> for IntegrationFieldMappingOutput {
    fn from(m: stateset_core::IntegrationFieldMapping) -> Self {
        Self {
            id: m.id.to_string(),
            integration_account: m.integration_account,
            mapping_group: m.mapping_group,
            source_field: m.source_field,
            destination_field: m.destination_field,
            template: m.template,
            transform: m.transform.to_string(),
            fallback: m.fallback,
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct IntegrationFieldMappings {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl IntegrationFieldMappings {
    /// Whether the integration field-mappings backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.integration_field_mappings().is_supported())
    }

    #[napi]
    pub async fn create(
        &self,
        input: CreateIntegrationFieldMappingInput,
    ) -> Result<IntegrationFieldMappingOutput> {
        let commerce = self.commerce.lock().await;
        let mapping =
            commerce.integration_field_mappings().create(input.try_into()?).map_err(|e| {
                Error::from_reason(format!("Failed to create integration field mapping: {}", e))
            })?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<IntegrationFieldMappingOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "integration_field_mapping")?;
        let mapping = commerce.integration_field_mappings().get(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to get integration field mapping: {}", e))
        })?;
        Ok(mapping.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateIntegrationFieldMappingInput,
    ) -> Result<IntegrationFieldMappingOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "integration_field_mapping")?;
        let mapping = commerce
            .integration_field_mappings()
            .update(
                uuid.into(),
                stateset_core::UpdateIntegrationFieldMapping {
                    destination_field: input.destination_field,
                    template: input.template,
                    transform: input.transform.as_deref().map(parse_field_transform).transpose()?,
                    fallback: input.fallback,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| {
                Error::from_reason(format!("Failed to update integration field mapping: {}", e))
            })?;
        Ok(mapping.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<IntegrationFieldMappingFilterInput>,
    ) -> Result<Vec<IntegrationFieldMappingOutput>> {
        let commerce = self.commerce.lock().await;
        let filter =
            filter.map_or_else(stateset_core::IntegrationFieldMappingFilter::default, |f| {
                stateset_core::IntegrationFieldMappingFilter {
                    integration_account: f.integration_account,
                    mapping_group: f.mapping_group,
                    source_field: f.source_field,
                    is_active: f.is_active,
                    limit: f.limit,
                    offset: f.offset,
                }
            });
        let mappings = commerce.integration_field_mappings().list(filter).map_err(|e| {
            Error::from_reason(format!("Failed to list integration field mappings: {}", e))
        })?;
        Ok(mappings.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "integration_field_mapping")?;
        commerce.integration_field_mappings().delete(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to delete integration field mapping: {}", e))
        })
    }

    /// Bulk create field mappings; returns the number of rows affected as a string.
    #[napi]
    pub async fn bulk_create(
        &self,
        items: Vec<CreateIntegrationFieldMappingInput>,
    ) -> Result<String> {
        let commerce = self.commerce.lock().await;
        let items = items
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<stateset_core::CreateIntegrationFieldMapping>>>()?;
        let affected = commerce.integration_field_mappings().bulk_create(items).map_err(|e| {
            Error::from_reason(format!("Failed to bulk create integration field mappings: {}", e))
        })?;
        Ok(affected.to_string())
    }

    /// Bulk delete field mappings by ID; returns the number of rows affected as a string.
    #[napi]
    pub async fn bulk_delete(&self, ids: Vec<String>) -> Result<String> {
        let commerce = self.commerce.lock().await;
        let ids = ids
            .iter()
            .map(|id| {
                parse_uuid_str(id, "integration_field_mapping")
                    .map(stateset_core::IntegrationFieldMappingId::from)
            })
            .collect::<Result<Vec<_>>>()?;
        let affected = commerce.integration_field_mappings().bulk_delete(ids).map_err(|e| {
            Error::from_reason(format!("Failed to bulk delete integration field mappings: {}", e))
        })?;
        Ok(affected.to_string())
    }

    /// Distinct mapping groups for an integration account.
    #[napi]
    pub async fn distinct_groups(&self, integration_account: String) -> Result<Vec<String>> {
        let commerce = self.commerce.lock().await;
        commerce
            .integration_field_mappings()
            .distinct_groups(&integration_account)
            .map_err(|e| Error::from_reason(format!("Failed to list mapping groups: {}", e)))
    }
}

// ============================================================================
// Payment Obligations
// ============================================================================

fn parse_payment_obligation_status(s: &str) -> Result<stateset_core::PaymentObligationStatus> {
    s.parse::<stateset_core::PaymentObligationStatus>()
        .map_err(|_| Error::from_reason(format!("Invalid payment obligation status: {}", s)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePaymentObligationInput {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    pub currency: Option<String>,
    /// Date string (YYYY-MM-DD)
    pub due_date: String,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentObligationFilterInput {
    pub supplier_id: Option<String>,
    /// Snake-case status
    pub status: Option<String>,
    /// Date string (YYYY-MM-DD)
    pub due_before: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentObligationOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub amount_paid: String,
    /// Exact decimal string
    pub outstanding: String,
    pub currency: String,
    /// Date string (YYYY-MM-DD)
    pub due_date: String,
    /// Snake-case status
    pub status: String,
    pub linked_bill_ids: Vec<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PaymentObligation> for PaymentObligationOutput {
    fn from(o: stateset_core::PaymentObligation) -> Self {
        let outstanding = o.outstanding().to_string();
        Self {
            id: o.id.to_string(),
            number: o.number,
            supplier_id: o.supplier_id.to_string(),
            purchase_order_id: o.purchase_order_id.map(|id| id.to_string()),
            amount: o.amount.to_string(),
            amount_paid: o.amount_paid.to_string(),
            outstanding,
            currency: o.currency.to_string(),
            due_date: o.due_date.to_string(),
            status: o.status.to_string(),
            linked_bill_ids: o.linked_bill_ids.iter().map(ToString::to_string).collect(),
            notes: o.notes,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentObligationDashboardOutput {
    pub open_count: String,
    /// Exact decimal string
    pub total_outstanding: String,
    pub overdue_count: String,
    /// Exact decimal string
    pub overdue_amount: String,
}

impl From<stateset_core::PaymentObligationDashboard> for PaymentObligationDashboardOutput {
    fn from(d: stateset_core::PaymentObligationDashboard) -> Self {
        Self {
            open_count: d.open_count.to_string(),
            total_outstanding: d.total_outstanding.to_string(),
            overdue_count: d.overdue_count.to_string(),
            overdue_amount: d.overdue_amount.to_string(),
        }
    }
}

#[napi]
pub struct PaymentObligations {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl PaymentObligations {
    /// Whether the payment-obligations backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.payment_obligations().is_supported())
    }

    #[napi]
    pub async fn create(
        &self,
        input: CreatePaymentObligationInput,
    ) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.lock().await;
        let obligation = commerce
            .payment_obligations()
            .create(stateset_core::CreatePaymentObligation {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                purchase_order_id: parse_optional_uuid(
                    input.purchase_order_id,
                    "purchase_order_id",
                )?,
                amount: parse_decimal_str(&input.amount, "amount")?,
                currency: parse_currency_opt(input.currency)?,
                due_date: parse_naive_date(&input.due_date, "due_date")?,
                notes: input.notes,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to create payment obligation: {}", e))
            })?;
        Ok(obligation.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PaymentObligationOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let obligation = commerce
            .payment_obligations()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get payment obligation: {}", e)))?;
        Ok(obligation.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PaymentObligationFilterInput>,
    ) -> Result<Vec<PaymentObligationOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PaymentObligationFilter::default()),
            |f| -> Result<stateset_core::PaymentObligationFilter> {
                Ok(stateset_core::PaymentObligationFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f.status.as_deref().map(parse_payment_obligation_status).transpose()?,
                    due_before: f
                        .due_before
                        .as_deref()
                        .map(|d| parse_naive_date(d, "due_before"))
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let obligations = commerce.payment_obligations().list(filter).map_err(|e| {
            Error::from_reason(format!("Failed to list payment obligations: {}", e))
        })?;
        Ok(obligations.into_iter().map(Into::into).collect())
    }

    /// Record a payment against an obligation.
    #[napi]
    pub async fn record_payment(
        &self,
        id: String,
        amount: String,
    ) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let obligation = commerce
            .payment_obligations()
            .record_payment(uuid.into(), parse_decimal_str(&amount, "amount")?)
            .map_err(|e| Error::from_reason(format!("Failed to record payment: {}", e)))?;
        Ok(obligation.into())
    }

    /// Set the obligation status (e.g. `scheduled`, `cancelled`).
    #[napi]
    pub async fn set_status(&self, id: String, status: String) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let obligation = commerce
            .payment_obligations()
            .set_status(uuid.into(), parse_payment_obligation_status(&status)?)
            .map_err(|e| {
                Error::from_reason(format!("Failed to set payment obligation status: {}", e))
            })?;
        Ok(obligation.into())
    }

    /// Link an AP bill to an obligation.
    #[napi]
    pub async fn link_bill(&self, id: String, bill_id: String) -> Result<PaymentObligationOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "payment_obligation")?;
        let bill_uuid = parse_uuid_str(&bill_id, "bill_id")?;
        let obligation = commerce
            .payment_obligations()
            .link_bill(uuid.into(), bill_uuid)
            .map_err(|e| Error::from_reason(format!("Failed to link bill: {}", e)))?;
        Ok(obligation.into())
    }

    /// Aggregate dashboard summary as of the given date (YYYY-MM-DD).
    #[napi]
    pub async fn dashboard(&self, today: String) -> Result<PaymentObligationDashboardOutput> {
        let commerce = self.commerce.lock().await;
        let day = parse_naive_date(&today, "today")?;
        let dashboard = commerce.payment_obligations().dashboard(day).map_err(|e| {
            Error::from_reason(format!("Failed to build payment obligation dashboard: {}", e))
        })?;
        Ok(dashboard.into())
    }
}

// ============================================================================
// Purgatory (order ingestion staging)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IngestLineItemInput {
    pub external_sku: String,
    /// Exact decimal string
    pub quantity: String,
    pub product_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IngestOrderInput {
    pub channel_id: Option<String>,
    pub external_order_id: String,
    pub external_status: Option<String>,
    /// JSON string
    pub metadata: Option<String>,
    pub items: Vec<IngestLineItemInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct MapPurgatoryLineInput {
    pub product_id: Option<String>,
    pub ignore_item: Option<bool>,
    pub non_physical: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurgatoryFilterInput {
    pub channel_id: Option<String>,
    pub is_posted: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurgatoryLineItemOutput {
    pub id: String,
    pub purgatory_order_id: String,
    pub external_sku: String,
    pub product_id: Option<String>,
    /// Exact decimal string
    pub quantity: String,
    pub ignore_item: bool,
    pub non_physical: bool,
    pub is_resolved: bool,
}

impl From<stateset_core::PurgatoryLineItem> for PurgatoryLineItemOutput {
    fn from(l: stateset_core::PurgatoryLineItem) -> Self {
        let is_resolved = l.is_resolved();
        Self {
            id: l.id.to_string(),
            purgatory_order_id: l.purgatory_order_id.to_string(),
            external_sku: l.external_sku,
            product_id: l.product_id.map(|id| id.to_string()),
            quantity: l.quantity.to_string(),
            ignore_item: l.ignore_item,
            non_physical: l.non_physical,
            is_resolved,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PurgatoryOrderOutput {
    pub id: String,
    pub channel_id: Option<String>,
    pub external_order_id: String,
    pub external_status: Option<String>,
    pub is_posted: bool,
    pub hold_reason: Option<String>,
    /// JSON string
    pub metadata: String,
    pub items: Vec<PurgatoryLineItemOutput>,
    pub is_ready_to_post: bool,
    pub unresolved_count: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PurgatoryOrder> for PurgatoryOrderOutput {
    fn from(o: stateset_core::PurgatoryOrder) -> Self {
        let is_ready_to_post = o.is_ready_to_post();
        let unresolved_count = o.unresolved_count().to_string();
        Self {
            id: o.id.to_string(),
            channel_id: o.channel_id.map(|id| id.to_string()),
            external_order_id: o.external_order_id,
            external_status: o.external_status,
            is_posted: o.is_posted,
            hold_reason: o.hold_reason,
            metadata: serde_json::to_string(&o.metadata).unwrap_or_else(|_| "null".to_string()),
            items: o.items.into_iter().map(Into::into).collect(),
            is_ready_to_post,
            unresolved_count,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Purgatory {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Purgatory {
    /// Whether the purgatory backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.purgatory().is_supported())
    }

    /// Ingest an external order into purgatory.
    #[napi]
    pub async fn ingest(&self, input: IngestOrderInput) -> Result<PurgatoryOrderOutput> {
        let commerce = self.commerce.lock().await;
        let metadata = match input.metadata {
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| Error::from_reason(format!("Invalid metadata JSON: {}", e)))?,
            None => serde_json::Value::Null,
        };
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::IngestLineItem> {
                Ok(stateset_core::IngestLineItem {
                    external_sku: i.external_sku,
                    quantity: parse_decimal_str(&i.quantity, "quantity")?,
                    product_id: parse_optional_uuid(i.product_id, "product_id")?.map(Into::into),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let order = commerce
            .purgatory()
            .ingest(stateset_core::IngestOrder {
                channel_id: parse_optional_uuid(input.channel_id, "channel_id")?.map(Into::into),
                external_order_id: input.external_order_id,
                external_status: input.external_status,
                metadata,
                items,
            })
            .map_err(|e| Error::from_reason(format!("Failed to ingest purgatory order: {}", e)))?;
        Ok(order.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PurgatoryOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        let order = commerce
            .purgatory()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get purgatory order: {}", e)))?;
        Ok(order.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PurgatoryFilterInput>,
    ) -> Result<Vec<PurgatoryOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PurgatoryFilter::default()),
            |f| -> Result<stateset_core::PurgatoryFilter> {
                Ok(stateset_core::PurgatoryFilter {
                    channel_id: parse_optional_uuid(f.channel_id, "channel_id")?.map(Into::into),
                    is_posted: f.is_posted,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let orders = commerce
            .purgatory()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list purgatory orders: {}", e)))?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// Map a staged line to a product and/or toggle its flags.
    #[napi]
    pub async fn map_line(
        &self,
        id: String,
        line_id: String,
        input: MapPurgatoryLineInput,
    ) -> Result<PurgatoryOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        let line_uuid = parse_uuid_str(&line_id, "purgatory_line_item")?;
        let order = commerce
            .purgatory()
            .map_line(
                uuid.into(),
                line_uuid.into(),
                stateset_core::MapPurgatoryLine {
                    product_id: parse_optional_uuid(input.product_id, "product_id")?
                        .map(Into::into),
                    ignore_item: input.ignore_item,
                    non_physical: input.non_physical,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to map purgatory line: {}", e)))?;
        Ok(order.into())
    }

    /// Post the order out of purgatory.
    #[napi]
    pub async fn post(&self, id: String) -> Result<PurgatoryOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        let order = commerce
            .purgatory()
            .post(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to post purgatory order: {}", e)))?;
        Ok(order.into())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "purgatory_order")?;
        commerce
            .purgatory()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete purgatory order: {}", e)))
    }
}

// ============================================================================
// Topology Snapshots
// ============================================================================

fn parse_health_grade(s: &str) -> Result<stateset_core::HealthGrade> {
    s.parse::<stateset_core::HealthGrade>()
        .map_err(|_| Error::from_reason(format!("Invalid health grade: {}", s)))
}

fn parse_u64_str(s: &str, field: &str) -> Result<u64> {
    s.parse::<u64>().map_err(|_| Error::from_reason(format!("Invalid {field} count")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptureTopologySnapshotInput {
    pub channels_total: String,
    pub channels_active: String,
    pub warehouses_total: String,
    pub products_total: String,
    pub open_orders: String,
    /// JSON string
    pub signals: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TopologySnapshotFilterInput {
    /// Snake-case health grade: `unknown`, `healthy`, `degraded`, `critical`
    pub health: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TopologySnapshotOutput {
    pub id: String,
    pub channels_total: String,
    pub channels_active: String,
    pub warehouses_total: String,
    pub products_total: String,
    pub open_orders: String,
    /// Snake-case health grade
    pub health: String,
    /// JSON string
    pub signals: String,
    pub captured_at: String,
}

impl From<stateset_core::TopologySnapshot> for TopologySnapshotOutput {
    fn from(s: stateset_core::TopologySnapshot) -> Self {
        Self {
            id: s.id.to_string(),
            channels_total: s.channels_total.to_string(),
            channels_active: s.channels_active.to_string(),
            warehouses_total: s.warehouses_total.to_string(),
            products_total: s.products_total.to_string(),
            open_orders: s.open_orders.to_string(),
            health: s.health.to_string(),
            signals: serde_json::to_string(&s.signals).unwrap_or_else(|_| "null".to_string()),
            captured_at: s.captured_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct TopologySnapshots {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl TopologySnapshots {
    /// Whether the topology-snapshots backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.topology_snapshots().is_supported())
    }

    /// Capture a new snapshot; health is derived from the supplied metrics.
    #[napi]
    pub async fn capture(
        &self,
        input: CaptureTopologySnapshotInput,
    ) -> Result<TopologySnapshotOutput> {
        let commerce = self.commerce.lock().await;
        let signals = match input.signals {
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| Error::from_reason(format!("Invalid signals JSON: {}", e)))?,
            None => serde_json::Value::Null,
        };
        let snapshot = commerce
            .topology_snapshots()
            .capture(stateset_core::CaptureTopologySnapshot {
                channels_total: parse_u64_str(&input.channels_total, "channels_total")?,
                channels_active: parse_u64_str(&input.channels_active, "channels_active")?,
                warehouses_total: parse_u64_str(&input.warehouses_total, "warehouses_total")?,
                products_total: parse_u64_str(&input.products_total, "products_total")?,
                open_orders: parse_u64_str(&input.open_orders, "open_orders")?,
                signals,
            })
            .map_err(|e| {
                Error::from_reason(format!("Failed to capture topology snapshot: {}", e))
            })?;
        Ok(snapshot.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<TopologySnapshotOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "topology_snapshot")?;
        let snapshot = commerce
            .topology_snapshots()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get topology snapshot: {}", e)))?;
        Ok(snapshot.map(Into::into))
    }

    /// Most recent snapshot, if any.
    #[napi]
    pub async fn latest(&self) -> Result<Option<TopologySnapshotOutput>> {
        let commerce = self.commerce.lock().await;
        let snapshot = commerce.topology_snapshots().latest().map_err(|e| {
            Error::from_reason(format!("Failed to get latest topology snapshot: {}", e))
        })?;
        Ok(snapshot.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<TopologySnapshotFilterInput>,
    ) -> Result<Vec<TopologySnapshotOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::TopologySnapshotFilter::default()),
            |f| -> Result<stateset_core::TopologySnapshotFilter> {
                Ok(stateset_core::TopologySnapshotFilter {
                    health: f.health.as_deref().map(parse_health_grade).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let snapshots = commerce
            .topology_snapshots()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list topology snapshots: {}", e)))?;
        Ok(snapshots.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "topology_snapshot")?;
        commerce
            .topology_snapshots()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete topology snapshot: {}", e)))
    }
}

// ============================================================================
// Vendor Returns
// ============================================================================

fn parse_vendor_return_status(s: &str) -> Result<stateset_core::VendorReturnStatus> {
    s.parse::<stateset_core::VendorReturnStatus>()
        .map_err(|_| Error::from_reason(format!("Invalid vendor return status: {}", s)))
}

fn parse_vendor_return_reason(s: &str) -> Result<stateset_core::VendorReturnReason> {
    s.parse::<stateset_core::VendorReturnReason>()
        .map_err(|_| Error::from_reason(format!("Invalid vendor return reason: {}", s)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateVendorReturnItemInput {
    pub product_id: String,
    /// Exact decimal string
    pub quantity: String,
    /// Exact decimal string
    pub unit_cost: String,
    /// Snake-case reason: `defective`, `overage`, `wrong_item`, `other`
    pub reason: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateVendorReturnInput {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub currency: Option<String>,
    pub items: Vec<CreateVendorReturnItemInput>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorReturnFilterInput {
    pub supplier_id: Option<String>,
    /// Snake-case status
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorReturnItemOutput {
    pub id: String,
    pub vendor_return_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity: String,
    /// Exact decimal string
    pub unit_cost: String,
    /// Exact decimal string
    pub line_total: String,
    /// Snake-case reason
    pub reason: String,
}

impl From<stateset_core::VendorReturnItem> for VendorReturnItemOutput {
    fn from(i: stateset_core::VendorReturnItem) -> Self {
        let line_total = i.line_total().to_string();
        Self {
            id: i.id.to_string(),
            vendor_return_id: i.vendor_return_id.to_string(),
            product_id: i.product_id.to_string(),
            sku: i.sku,
            quantity: i.quantity.to_string(),
            unit_cost: i.unit_cost.to_string(),
            line_total,
            reason: i.reason.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorReturnOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Snake-case status
    pub status: String,
    pub currency: String,
    pub items: Vec<VendorReturnItemOutput>,
    /// Exact decimal string
    pub total_credit: String,
    pub credit_generated: bool,
    pub notes: Option<String>,
    pub processed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::VendorReturn> for VendorReturnOutput {
    fn from(r: stateset_core::VendorReturn) -> Self {
        let total_credit = r.total_credit().to_string();
        Self {
            id: r.id.to_string(),
            number: r.number,
            supplier_id: r.supplier_id.to_string(),
            purchase_order_id: r.purchase_order_id.map(|id| id.to_string()),
            status: r.status.to_string(),
            currency: r.currency.to_string(),
            items: r.items.into_iter().map(Into::into).collect(),
            total_credit,
            credit_generated: r.credit_generated,
            notes: r.notes,
            processed_at: r.processed_at.map(|d| d.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct VendorReturns {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl VendorReturns {
    /// Whether the vendor-returns backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.vendor_returns().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateVendorReturnInput) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.lock().await;
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::CreateVendorReturnItem> {
                Ok(stateset_core::CreateVendorReturnItem {
                    product_id: parse_uuid_str(&i.product_id, "product_id")?.into(),
                    quantity: parse_decimal_str(&i.quantity, "quantity")?,
                    unit_cost: parse_decimal_str(&i.unit_cost, "unit_cost")?,
                    reason: i
                        .reason
                        .as_deref()
                        .map(parse_vendor_return_reason)
                        .transpose()?
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let vendor_return = commerce
            .vendor_returns()
            .create(stateset_core::CreateVendorReturn {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                purchase_order_id: parse_optional_uuid(
                    input.purchase_order_id,
                    "purchase_order_id",
                )?,
                currency: parse_currency_opt(input.currency)?,
                items,
                notes: input.notes,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create vendor return: {}", e)))?;
        Ok(vendor_return.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<VendorReturnOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get vendor return: {}", e)))?;
        Ok(vendor_return.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<VendorReturnFilterInput>,
    ) -> Result<Vec<VendorReturnOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::VendorReturnFilter::default()),
            |f| -> Result<stateset_core::VendorReturnFilter> {
                Ok(stateset_core::VendorReturnFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f.status.as_deref().map(parse_vendor_return_status).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let returns = commerce
            .vendor_returns()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list vendor returns: {}", e)))?;
        Ok(returns.into_iter().map(Into::into).collect())
    }

    /// Submit a draft vendor return to the supplier.
    #[napi]
    pub async fn submit(&self, id: String) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .submit(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to submit vendor return: {}", e)))?;
        Ok(vendor_return.into())
    }

    /// Process a vendor return, optionally generating a vendor credit.
    #[napi]
    pub async fn process(&self, id: String, generate_credit: bool) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .process(uuid.into(), generate_credit)
            .map_err(|e| Error::from_reason(format!("Failed to process vendor return: {}", e)))?;
        Ok(vendor_return.into())
    }

    /// Cancel a vendor return.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .cancel(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to cancel vendor return: {}", e)))?;
        Ok(vendor_return.into())
    }
}

// ============================================================================
// Fraud (risk assessment + rules)
// ============================================================================

fn parse_fraud_decision(s: &str) -> Result<stateset_core::FraudDecision> {
    s.parse::<stateset_core::FraudDecision>()
        .map_err(|_| Error::from_reason(format!("Invalid fraud decision: {}", s)))
}

fn parse_fraud_signal_type(s: &str) -> Result<stateset_core::FraudSignalType> {
    s.parse::<stateset_core::FraudSignalType>()
        .map_err(|_| Error::from_reason(format!("Invalid fraud signal type: {}", s)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFraudSignalInput {
    /// Snake-case signal type: `velocity_spike`, `address_mismatch`, ...
    pub signal_type: String,
    /// Confidence score (0.0 - 1.0)
    pub score: f64,
    pub details: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFraudAssessmentInput {
    pub order_id: String,
    pub signals: Vec<CreateFraudSignalInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudAssessmentFilterInput {
    /// Snake-case decision: `accept`, `review`, `reject`
    pub decision: Option<String>,
    pub min_risk_score: Option<f64>,
    pub unreviewed_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudSignalOutput {
    pub order_id: String,
    /// Snake-case signal type
    pub signal_type: String,
    pub score: f64,
    pub details: String,
    pub detected_at: String,
}

impl From<stateset_core::FraudSignal> for FraudSignalOutput {
    fn from(s: stateset_core::FraudSignal) -> Self {
        Self {
            order_id: s.order_id.to_string(),
            signal_type: s.signal_type.to_string(),
            score: s.score,
            details: s.details,
            detected_at: s.detected_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudAssessmentOutput {
    pub order_id: String,
    pub risk_score: f64,
    pub signals: Vec<FraudSignalOutput>,
    /// Snake-case decision
    pub decision: String,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub needs_review: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::FraudAssessment> for FraudAssessmentOutput {
    fn from(a: stateset_core::FraudAssessment) -> Self {
        let needs_review = a.needs_review();
        Self {
            order_id: a.order_id.to_string(),
            risk_score: a.risk_score,
            signals: a.signals.into_iter().map(Into::into).collect(),
            decision: a.decision.to_string(),
            reviewed_by: a.reviewed_by,
            review_notes: a.review_notes,
            needs_review,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFraudRuleInput {
    pub name: String,
    pub description: Option<String>,
    /// Snake-case signal type
    pub signal_type: String,
    pub threshold: f64,
    /// Snake-case decision to apply when the rule triggers
    pub action: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateFraudRuleInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub threshold: Option<f64>,
    /// Snake-case decision
    pub action: Option<String>,
    pub enabled: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudRuleFilterInput {
    /// Snake-case signal type
    pub signal_type: Option<String>,
    /// Snake-case decision
    pub action: Option<String>,
    pub enabled: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudRuleOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Snake-case signal type
    pub signal_type: String,
    pub threshold: f64,
    /// Snake-case decision
    pub action: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::FraudRule> for FraudRuleOutput {
    fn from(r: stateset_core::FraudRule) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            description: r.description,
            signal_type: r.signal_type.to_string(),
            threshold: r.threshold,
            action: r.action.to_string(),
            enabled: r.enabled,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Fraud {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Fraud {
    /// Whether the fraud backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.fraud().is_supported())
    }

    /// Create a fraud assessment for an order.
    #[napi]
    pub async fn create_assessment(
        &self,
        input: CreateFraudAssessmentInput,
    ) -> Result<FraudAssessmentOutput> {
        let commerce = self.commerce.lock().await;
        let signals = input
            .signals
            .into_iter()
            .map(|s| -> Result<stateset_core::CreateFraudSignal> {
                Ok(stateset_core::CreateFraudSignal {
                    signal_type: parse_fraud_signal_type(&s.signal_type)?,
                    score: s.score,
                    details: s.details,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let assessment = commerce
            .fraud()
            .create_assessment(stateset_core::CreateFraudAssessment {
                order_id: parse_uuid_str(&input.order_id, "order_id")?.into(),
                signals,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create fraud assessment: {}", e)))?;
        Ok(assessment.into())
    }

    #[napi]
    pub async fn get_assessment(&self, order_id: String) -> Result<Option<FraudAssessmentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&order_id, "order_id")?;
        let assessment = commerce
            .fraud()
            .get_assessment(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get fraud assessment: {}", e)))?;
        Ok(assessment.map(Into::into))
    }

    #[napi]
    pub async fn list_assessments(
        &self,
        filter: Option<FraudAssessmentFilterInput>,
    ) -> Result<Vec<FraudAssessmentOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::FraudAssessmentFilter::default()),
            |f| -> Result<stateset_core::FraudAssessmentFilter> {
                Ok(stateset_core::FraudAssessmentFilter {
                    decision: f.decision.as_deref().map(parse_fraud_decision).transpose()?,
                    min_risk_score: f.min_risk_score,
                    unreviewed_only: f.unreviewed_only,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let assessments = commerce
            .fraud()
            .list_assessments(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list fraud assessments: {}", e)))?;
        Ok(assessments.into_iter().map(Into::into).collect())
    }

    /// Record a manual review decision on an assessment.
    #[napi]
    pub async fn review_assessment(
        &self,
        order_id: String,
        decision: String,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&order_id, "order_id")?;
        let assessment = commerce
            .fraud()
            .review_assessment(uuid.into(), parse_fraud_decision(&decision)?, reviewer, notes)
            .map_err(|e| Error::from_reason(format!("Failed to review fraud assessment: {}", e)))?;
        Ok(assessment.into())
    }

    #[napi]
    pub async fn create_rule(&self, input: CreateFraudRuleInput) -> Result<FraudRuleOutput> {
        let commerce = self.commerce.lock().await;
        let rule = commerce
            .fraud()
            .create_rule(stateset_core::CreateFraudRule {
                name: input.name,
                description: input.description,
                signal_type: parse_fraud_signal_type(&input.signal_type)?,
                threshold: input.threshold,
                action: parse_fraud_decision(&input.action)?,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create fraud rule: {}", e)))?;
        Ok(rule.into())
    }

    #[napi]
    pub async fn get_rule(&self, id: String) -> Result<Option<FraudRuleOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "fraud_rule")?;
        let rule = commerce
            .fraud()
            .get_rule(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get fraud rule: {}", e)))?;
        Ok(rule.map(Into::into))
    }

    #[napi]
    pub async fn update_rule(
        &self,
        id: String,
        input: UpdateFraudRuleInput,
    ) -> Result<FraudRuleOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "fraud_rule")?;
        let rule = commerce
            .fraud()
            .update_rule(
                uuid.into(),
                stateset_core::UpdateFraudRule {
                    name: input.name,
                    description: input.description.map(Some),
                    threshold: input.threshold,
                    action: input.action.as_deref().map(parse_fraud_decision).transpose()?,
                    enabled: input.enabled,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update fraud rule: {}", e)))?;
        Ok(rule.into())
    }

    #[napi]
    pub async fn list_rules(
        &self,
        filter: Option<FraudRuleFilterInput>,
    ) -> Result<Vec<FraudRuleOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(
            || Ok(stateset_core::FraudRuleFilter::default()),
            |f| -> Result<stateset_core::FraudRuleFilter> {
                Ok(stateset_core::FraudRuleFilter {
                    signal_type: f
                        .signal_type
                        .as_deref()
                        .map(parse_fraud_signal_type)
                        .transpose()?,
                    action: f.action.as_deref().map(parse_fraud_decision).transpose()?,
                    enabled: f.enabled,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let rules = commerce
            .fraud()
            .list_rules(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list fraud rules: {}", e)))?;
        Ok(rules.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_rule(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "fraud_rule")?;
        commerce
            .fraud()
            .delete_rule(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete fraud rule: {}", e)))
    }

    /// All currently enabled fraud rules.
    #[napi]
    pub async fn get_active_rules(&self) -> Result<Vec<FraudRuleOutput>> {
        let commerce = self.commerce.lock().await;
        let rules = commerce
            .fraud()
            .get_active_rules()
            .map_err(|e| Error::from_reason(format!("Failed to get active fraud rules: {}", e)))?;
        Ok(rules.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Search configuration
// ============================================================================

fn parse_tokenizer(s: &str) -> Result<stateset_core::Tokenizer> {
    s.parse::<stateset_core::Tokenizer>()
        .map_err(|_| Error::from_reason(format!("Invalid tokenizer: {}", s)))
}

fn parse_facet_type(s: &str) -> Result<stateset_core::FacetType> {
    s.parse::<stateset_core::FacetType>()
        .map_err(|_| Error::from_reason(format!("Invalid facet type: {}", s)))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchFieldInput {
    pub field_name: String,
    pub weight: f64,
    /// Snake-case tokenizer: `standard`, `ngram`, `edge`, `keyword`
    pub tokenizer: Option<String>,
    pub enabled: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FacetConfigInput {
    pub field_name: String,
    /// Snake-case facet type: `value`, `range`, `hierarchical`
    pub facet_type: Option<String>,
    pub display_name: String,
    pub sort_order: Option<i32>,
    pub max_values: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SynonymGroupInput {
    pub canonical: String,
    pub synonyms: Vec<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BoostRuleInput {
    pub field: String,
    pub value_match: String,
    pub boost_factor: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSearchConfigInput {
    pub name: String,
    pub description: Option<String>,
    pub searchable_fields: Option<Vec<SearchFieldInput>>,
    pub facets: Option<Vec<FacetConfigInput>>,
    pub synonyms: Option<Vec<SynonymGroupInput>>,
    pub boost_rules: Option<Vec<BoostRuleInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSearchConfigInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub searchable_fields: Option<Vec<SearchFieldInput>>,
    pub facets: Option<Vec<FacetConfigInput>>,
    pub synonyms: Option<Vec<SynonymGroupInput>>,
    pub boost_rules: Option<Vec<BoostRuleInput>>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchConfigFilterInput {
    pub is_active: Option<bool>,
    pub name: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchFieldOutput {
    pub field_name: String,
    pub weight: f64,
    /// Snake-case tokenizer
    pub tokenizer: String,
    pub enabled: bool,
}

impl From<stateset_core::SearchField> for SearchFieldOutput {
    fn from(f: stateset_core::SearchField) -> Self {
        Self {
            field_name: f.field_name,
            weight: f.weight,
            tokenizer: f.tokenizer.to_string(),
            enabled: f.enabled,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FacetConfigOutput {
    pub field_name: String,
    /// Snake-case facet type
    pub facet_type: String,
    pub display_name: String,
    pub sort_order: i32,
    pub max_values: Option<u32>,
}

impl From<stateset_core::FacetConfig> for FacetConfigOutput {
    fn from(f: stateset_core::FacetConfig) -> Self {
        Self {
            field_name: f.field_name,
            facet_type: f.facet_type.to_string(),
            display_name: f.display_name,
            sort_order: f.sort_order,
            max_values: f.max_values,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SynonymGroupOutput {
    pub canonical: String,
    pub synonyms: Vec<String>,
}

impl From<stateset_core::SynonymGroup> for SynonymGroupOutput {
    fn from(g: stateset_core::SynonymGroup) -> Self {
        Self { canonical: g.canonical, synonyms: g.synonyms }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BoostRuleOutput {
    pub field: String,
    pub value_match: String,
    pub boost_factor: f64,
}

impl From<stateset_core::BoostRule> for BoostRuleOutput {
    fn from(b: stateset_core::BoostRule) -> Self {
        Self { field: b.field, value_match: b.value_match, boost_factor: b.boost_factor }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SearchConfigOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub searchable_fields: Vec<SearchFieldOutput>,
    pub facets: Vec<FacetConfigOutput>,
    pub synonyms: Vec<SynonymGroupOutput>,
    pub boost_rules: Vec<BoostRuleOutput>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::SearchConfig> for SearchConfigOutput {
    fn from(c: stateset_core::SearchConfig) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            description: c.description,
            searchable_fields: c.searchable_fields.into_iter().map(Into::into).collect(),
            facets: c.facets.into_iter().map(Into::into).collect(),
            synonyms: c.synonyms.into_iter().map(Into::into).collect(),
            boost_rules: c.boost_rules.into_iter().map(Into::into).collect(),
            is_active: c.is_active,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

fn convert_search_fields(fields: Vec<SearchFieldInput>) -> Result<Vec<stateset_core::SearchField>> {
    fields
        .into_iter()
        .map(|f| -> Result<stateset_core::SearchField> {
            Ok(stateset_core::SearchField {
                field_name: f.field_name,
                weight: f.weight,
                tokenizer: f
                    .tokenizer
                    .as_deref()
                    .map(parse_tokenizer)
                    .transpose()?
                    .unwrap_or_default(),
                enabled: f.enabled.unwrap_or(true),
            })
        })
        .collect()
}

fn convert_facets(facets: Vec<FacetConfigInput>) -> Result<Vec<stateset_core::FacetConfig>> {
    facets
        .into_iter()
        .map(|f| -> Result<stateset_core::FacetConfig> {
            Ok(stateset_core::FacetConfig {
                field_name: f.field_name,
                facet_type: f
                    .facet_type
                    .as_deref()
                    .map(parse_facet_type)
                    .transpose()?
                    .unwrap_or_default(),
                display_name: f.display_name,
                sort_order: f.sort_order.unwrap_or(0),
                max_values: f.max_values,
            })
        })
        .collect()
}

fn convert_synonyms(groups: Vec<SynonymGroupInput>) -> Vec<stateset_core::SynonymGroup> {
    groups
        .into_iter()
        .map(|g| stateset_core::SynonymGroup { canonical: g.canonical, synonyms: g.synonyms })
        .collect()
}

fn convert_boost_rules(rules: Vec<BoostRuleInput>) -> Vec<stateset_core::BoostRule> {
    rules
        .into_iter()
        .map(|b| stateset_core::BoostRule {
            field: b.field,
            value_match: b.value_match,
            boost_factor: b.boost_factor,
        })
        .collect()
}

#[napi]
pub struct SearchConfigs {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl SearchConfigs {
    /// Whether the search-configuration backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.search_config().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateSearchConfigInput) -> Result<SearchConfigOutput> {
        let commerce = self.commerce.lock().await;
        let config = commerce
            .search_config()
            .create(stateset_core::CreateSearchConfig {
                name: input.name,
                description: input.description,
                searchable_fields: convert_search_fields(
                    input.searchable_fields.unwrap_or_default(),
                )?,
                facets: convert_facets(input.facets.unwrap_or_default())?,
                synonyms: convert_synonyms(input.synonyms.unwrap_or_default()),
                boost_rules: convert_boost_rules(input.boost_rules.unwrap_or_default()),
            })
            .map_err(|e| Error::from_reason(format!("Failed to create search config: {}", e)))?;
        Ok(config.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SearchConfigOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "search_config")?;
        let config = commerce
            .search_config()
            .get(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to get search config: {}", e)))?;
        Ok(config.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateSearchConfigInput,
    ) -> Result<SearchConfigOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "search_config")?;
        let config = commerce
            .search_config()
            .update(
                uuid.into(),
                stateset_core::UpdateSearchConfig {
                    name: input.name,
                    description: input.description.map(Some),
                    searchable_fields: input
                        .searchable_fields
                        .map(convert_search_fields)
                        .transpose()?,
                    facets: input.facets.map(convert_facets).transpose()?,
                    synonyms: input.synonyms.map(convert_synonyms),
                    boost_rules: input.boost_rules.map(convert_boost_rules),
                    is_active: input.is_active,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update search config: {}", e)))?;
        Ok(config.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<SearchConfigFilterInput>,
    ) -> Result<Vec<SearchConfigOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::SearchConfigFilter::default, |f| {
            stateset_core::SearchConfigFilter {
                is_active: f.is_active,
                name: f.name,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let configs = commerce
            .search_config()
            .list(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list search configs: {}", e)))?;
        Ok(configs.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "search_config")?;
        commerce
            .search_config()
            .delete(uuid.into())
            .map_err(|e| Error::from_reason(format!("Failed to delete search config: {}", e)))
    }

    /// The currently active search configuration, if any.
    #[napi]
    pub async fn get_active(&self) -> Result<Option<SearchConfigOutput>> {
        let commerce = self.commerce.lock().await;
        let config = commerce.search_config().get_active().map_err(|e| {
            Error::from_reason(format!("Failed to get active search config: {}", e))
        })?;
        Ok(config.map(Into::into))
    }

    /// Make a configuration active, deactivating the current one.
    #[napi]
    pub async fn set_active(&self, id: String) -> Result<SearchConfigOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = parse_uuid_str(&id, "search_config")?;
        let config = commerce.search_config().set_active(uuid.into()).map_err(|e| {
            Error::from_reason(format!("Failed to set active search config: {}", e))
        })?;
        Ok(config.into())
    }
}

// ============================================================================
// ERC-8004 Trustless Agents (identity / reputation / validation)
// ============================================================================

fn parse_wallet_proof_type(s: &str) -> Result<stateset_core::AgentWalletProofType> {
    s.parse::<stateset_core::AgentWalletProofType>()
        .map_err(|_| Error::from_reason(format!("Invalid agent wallet proof type: {}", s)))
}

fn parse_i128_str(s: &str, field: &str) -> Result<i128> {
    s.parse::<i128>()
        .map_err(|_| Error::from_reason(format!("Invalid {field}: expected integer string")))
}

fn parse_u8_field(value: u32, field: &str) -> Result<u8> {
    u8::try_from(value).map_err(|_| Error::from_reason(format!("Invalid {field}: out of range")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentIdentityInput {
    pub agent_registry: String,
    pub agent_id: String,
    pub agent_uri: String,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    /// JSON-encoded registration document
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    /// Snake-case proof type
    pub wallet_proof_type: Option<String>,
    pub wallet_proof: Option<String>,
    /// Chain id as a decimal string
    pub wallet_proof_chain_id: Option<String>,
    /// RFC3339 timestamp
    pub wallet_proof_deadline: Option<String>,
    pub active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateAgentIdentityInput {
    pub agent_uri: Option<String>,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    /// JSON-encoded registration document
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    /// Snake-case proof type
    pub wallet_proof_type: Option<String>,
    pub wallet_proof: Option<String>,
    /// Chain id as a decimal string
    pub wallet_proof_chain_id: Option<String>,
    /// RFC3339 timestamp
    pub wallet_proof_deadline: Option<String>,
    pub active: Option<bool>,
}

/// Optional on-chain proof data accompanying a wallet binding.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AgentWalletProofInput {
    /// Snake-case proof type
    pub proof_type: Option<String>,
    pub proof: Option<String>,
    /// Chain id as a decimal string
    pub proof_chain_id: Option<String>,
    /// RFC3339 timestamp
    pub proof_deadline: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentIdentityFilterInput {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentIdentityOutput {
    pub id: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub agent_uri: String,
    pub agent_wallet: Option<String>,
    pub owner_address: Option<String>,
    pub agent_card_id: Option<String>,
    /// JSON-encoded registration document
    pub registration: Option<String>,
    pub registration_hash: Option<String>,
    /// Snake-case proof type
    pub wallet_proof_type: Option<String>,
    pub wallet_proof: Option<String>,
    /// Chain id as a decimal string
    pub wallet_proof_chain_id: Option<String>,
    pub wallet_proof_deadline: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::AgentIdentity> for AgentIdentityOutput {
    fn from(i: stateset_core::AgentIdentity) -> Self {
        Self {
            id: i.id.to_string(),
            agent_registry: i.agent_registry,
            agent_id: i.agent_id,
            agent_uri: i.agent_uri,
            agent_wallet: i.agent_wallet,
            owner_address: i.owner_address,
            agent_card_id: i.agent_card_id.map(|id| id.to_string()),
            registration: i.registration,
            registration_hash: i.registration_hash,
            wallet_proof_type: i.wallet_proof_type.map(|t| t.to_string()),
            wallet_proof: i.wallet_proof,
            wallet_proof_chain_id: i.wallet_proof_chain_id.map(|c| c.to_string()),
            wallet_proof_deadline: i.wallet_proof_deadline.map(|d| d.to_rfc3339()),
            active: i.active,
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentFeedbackInput {
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    /// Signed integer value as a decimal string
    pub value: String,
    /// Number of decimal places encoded in `value`
    pub value_decimals: u32,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub feedback_hash: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentFeedbackFilterInput {
    pub agent_registry: Option<String>,
    pub agent_id: Option<String>,
    pub client_addresses: Option<Vec<String>>,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub include_revoked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentFeedbackOutput {
    pub id: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub client_address: String,
    /// Feedback index as a decimal string
    pub feedback_index: String,
    /// Signed integer value as a decimal string
    pub value: String,
    pub value_decimals: u32,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub feedback_hash: Option<String>,
    pub is_revoked: bool,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

impl From<stateset_core::AgentFeedback> for AgentFeedbackOutput {
    fn from(f: stateset_core::AgentFeedback) -> Self {
        Self {
            id: f.id.to_string(),
            agent_registry: f.agent_registry,
            agent_id: f.agent_id,
            client_address: f.client_address,
            feedback_index: f.feedback_index.to_string(),
            value: f.value.to_string(),
            value_decimals: u32::from(f.value_decimals),
            tag1: f.tag1,
            tag2: f.tag2,
            endpoint: f.endpoint,
            feedback_uri: f.feedback_uri,
            feedback_hash: f.feedback_hash,
            is_revoked: f.is_revoked,
            created_at: f.created_at.to_rfc3339(),
            revoked_at: f.revoked_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FeedbackSummaryOutput {
    /// Count as a decimal string
    pub count: String,
    /// Aggregate value as a decimal string
    pub summary_value: String,
    pub summary_value_decimals: u32,
}

impl From<stateset_core::FeedbackSummary> for FeedbackSummaryOutput {
    fn from(s: stateset_core::FeedbackSummary) -> Self {
        Self {
            count: s.count.to_string(),
            summary_value: s.summary_value.to_string(),
            summary_value_decimals: u32::from(s.summary_value_decimals),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentValidationRequestInput {
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub request_uri: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentValidationRequestOutput {
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub request_uri: String,
    pub created_at: String,
}

impl From<stateset_core::AgentValidationRequest> for AgentValidationRequestOutput {
    fn from(r: stateset_core::AgentValidationRequest) -> Self {
        Self {
            request_hash: r.request_hash,
            agent_registry: r.agent_registry,
            agent_id: r.agent_id,
            validator_address: r.validator_address,
            request_uri: r.request_uri,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateAgentValidationResponseInput {
    /// Validation score (0-100)
    pub response: u32,
    pub response_uri: Option<String>,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentValidationResponseOutput {
    pub id: String,
    pub request_hash: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub validator_address: String,
    pub response: u32,
    pub response_uri: Option<String>,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::AgentValidationResponse> for AgentValidationResponseOutput {
    fn from(r: stateset_core::AgentValidationResponse) -> Self {
        Self {
            id: r.id.to_string(),
            request_hash: r.request_hash,
            agent_registry: r.agent_registry,
            agent_id: r.agent_id,
            validator_address: r.validator_address,
            response: u32::from(r.response),
            response_uri: r.response_uri,
            response_hash: r.response_hash,
            tag: r.tag,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentValidationStatusOutput {
    pub validator_address: String,
    pub agent_registry: String,
    pub agent_id: String,
    pub response: u32,
    pub response_hash: Option<String>,
    pub tag: Option<String>,
    pub last_update: String,
}

impl From<stateset_core::AgentValidationStatus> for AgentValidationStatusOutput {
    fn from(s: stateset_core::AgentValidationStatus) -> Self {
        Self {
            validator_address: s.validator_address,
            agent_registry: s.agent_registry,
            agent_id: s.agent_id,
            response: u32::from(s.response),
            response_hash: s.response_hash,
            tag: s.tag,
            last_update: s.last_update.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ValidationSummaryOutput {
    /// Count as a decimal string
    pub count: String,
    pub average_response: u32,
}

impl From<stateset_core::ValidationSummary> for ValidationSummaryOutput {
    fn from(s: stateset_core::ValidationSummary) -> Self {
        Self { count: s.count.to_string(), average_response: u32::from(s.average_response) }
    }
}

#[napi]
pub struct Erc8004 {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Erc8004 {
    // ---- Identity registry ----

    /// Register a new agent identity.
    #[napi]
    pub async fn register_identity(
        &self,
        input: CreateAgentIdentityInput,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.lock().await;
        let identity = commerce
            .erc8004()
            .register_identity(stateset_core::CreateAgentIdentity {
                agent_registry: input.agent_registry,
                agent_id: input.agent_id,
                agent_uri: input.agent_uri,
                agent_wallet: input.agent_wallet,
                owner_address: input.owner_address,
                agent_card_id: parse_optional_uuid(input.agent_card_id, "agent_card_id")?,
                registration: input.registration,
                registration_hash: input.registration_hash,
                wallet_proof_type: input
                    .wallet_proof_type
                    .as_deref()
                    .map(parse_wallet_proof_type)
                    .transpose()?,
                wallet_proof: input.wallet_proof,
                wallet_proof_chain_id: input
                    .wallet_proof_chain_id
                    .as_deref()
                    .map(|c| parse_u64_str(c, "wallet_proof_chain_id"))
                    .transpose()?,
                wallet_proof_deadline: parse_rfc3339_opt(
                    input.wallet_proof_deadline,
                    "wallet_proof_deadline",
                )?,
                active: input.active,
            })
            .map_err(|e| Error::from_reason(format!("Failed to register agent identity: {}", e)))?;
        Ok(identity.into())
    }

    #[napi]
    pub async fn get_identity(
        &self,
        agent_registry: String,
        agent_id: String,
    ) -> Result<Option<AgentIdentityOutput>> {
        let commerce = self.commerce.lock().await;
        let identity = commerce
            .erc8004()
            .get_identity(&agent_registry, &agent_id)
            .map_err(|e| Error::from_reason(format!("Failed to get agent identity: {}", e)))?;
        Ok(identity.map(Into::into))
    }

    #[napi]
    pub async fn get_identity_by_wallet(
        &self,
        agent_wallet: String,
    ) -> Result<Option<AgentIdentityOutput>> {
        let commerce = self.commerce.lock().await;
        let identity = commerce.erc8004().get_identity_by_wallet(&agent_wallet).map_err(|e| {
            Error::from_reason(format!("Failed to get agent identity by wallet: {}", e))
        })?;
        Ok(identity.map(Into::into))
    }

    #[napi]
    pub async fn update_identity(
        &self,
        agent_registry: String,
        agent_id: String,
        input: UpdateAgentIdentityInput,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.lock().await;
        let identity = commerce
            .erc8004()
            .update_identity(
                &agent_registry,
                &agent_id,
                stateset_core::UpdateAgentIdentity {
                    agent_uri: input.agent_uri,
                    agent_wallet: input.agent_wallet,
                    owner_address: input.owner_address,
                    agent_card_id: parse_optional_uuid(input.agent_card_id, "agent_card_id")?,
                    registration: input.registration,
                    registration_hash: input.registration_hash,
                    wallet_proof_type: input
                        .wallet_proof_type
                        .as_deref()
                        .map(parse_wallet_proof_type)
                        .transpose()?,
                    wallet_proof: input.wallet_proof,
                    wallet_proof_chain_id: input
                        .wallet_proof_chain_id
                        .as_deref()
                        .map(|c| parse_u64_str(c, "wallet_proof_chain_id"))
                        .transpose()?,
                    wallet_proof_deadline: parse_rfc3339_opt(
                        input.wallet_proof_deadline,
                        "wallet_proof_deadline",
                    )?,
                    active: input.active,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to update agent identity: {}", e)))?;
        Ok(identity.into())
    }

    /// Bind a wallet to an agent identity, with optional on-chain proof data.
    #[napi]
    pub async fn set_agent_wallet(
        &self,
        agent_registry: String,
        agent_id: String,
        agent_wallet: String,
        proof: Option<AgentWalletProofInput>,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.lock().await;
        let proof = proof.unwrap_or_default();
        let identity = commerce
            .erc8004()
            .set_agent_wallet(
                &agent_registry,
                &agent_id,
                &agent_wallet,
                proof.proof_type.as_deref().map(parse_wallet_proof_type).transpose()?,
                proof.proof.as_deref(),
                proof
                    .proof_chain_id
                    .as_deref()
                    .map(|c| parse_u64_str(c, "proof_chain_id"))
                    .transpose()?,
                parse_rfc3339_opt(proof.proof_deadline, "proof_deadline")?,
            )
            .map_err(|e| Error::from_reason(format!("Failed to set agent wallet: {}", e)))?;
        Ok(identity.into())
    }

    /// Clear the wallet binding on an agent identity.
    #[napi]
    pub async fn clear_agent_wallet(
        &self,
        agent_registry: String,
        agent_id: String,
    ) -> Result<AgentIdentityOutput> {
        let commerce = self.commerce.lock().await;
        let identity = commerce
            .erc8004()
            .clear_agent_wallet(&agent_registry, &agent_id)
            .map_err(|e| Error::from_reason(format!("Failed to clear agent wallet: {}", e)))?;
        Ok(identity.into())
    }

    #[napi]
    pub async fn list_identities(
        &self,
        filter: Option<AgentIdentityFilterInput>,
    ) -> Result<Vec<AgentIdentityOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = build_agent_identity_filter(filter)?;
        let identities = commerce
            .erc8004()
            .list_identities(filter)
            .map_err(|e| Error::from_reason(format!("Failed to list agent identities: {}", e)))?;
        Ok(identities.into_iter().map(Into::into).collect())
    }

    /// Count identities matching a filter (returned as a decimal string).
    #[napi]
    pub async fn count_identities(
        &self,
        filter: Option<AgentIdentityFilterInput>,
    ) -> Result<String> {
        let commerce = self.commerce.lock().await;
        let filter = build_agent_identity_filter(filter)?;
        let count = commerce
            .erc8004()
            .count_identities(filter)
            .map_err(|e| Error::from_reason(format!("Failed to count agent identities: {}", e)))?;
        Ok(count.to_string())
    }

    // ---- Reputation registry ----

    /// Give feedback about an agent.
    #[napi]
    pub async fn give_feedback(
        &self,
        input: CreateAgentFeedbackInput,
    ) -> Result<AgentFeedbackOutput> {
        let commerce = self.commerce.lock().await;
        let feedback = commerce
            .erc8004()
            .give_feedback(stateset_core::CreateAgentFeedback {
                agent_registry: input.agent_registry,
                agent_id: input.agent_id,
                client_address: input.client_address,
                value: parse_i128_str(&input.value, "value")?,
                value_decimals: parse_u8_field(input.value_decimals, "value_decimals")?,
                tag1: input.tag1,
                tag2: input.tag2,
                endpoint: input.endpoint,
                feedback_uri: input.feedback_uri,
                feedback_hash: input.feedback_hash,
            })
            .map_err(|e| Error::from_reason(format!("Failed to give agent feedback: {}", e)))?;
        Ok(feedback.into())
    }

    /// Revoke a previously given feedback entry.
    #[napi]
    pub async fn revoke_feedback(
        &self,
        agent_registry: String,
        agent_id: String,
        client_address: String,
        feedback_index: String,
    ) -> Result<AgentFeedbackOutput> {
        let commerce = self.commerce.lock().await;
        let feedback = commerce
            .erc8004()
            .revoke_feedback(
                &agent_registry,
                &agent_id,
                &client_address,
                parse_u64_str(&feedback_index, "feedback_index")?,
            )
            .map_err(|e| Error::from_reason(format!("Failed to revoke agent feedback: {}", e)))?;
        Ok(feedback.into())
    }

    #[napi]
    pub async fn read_feedback(
        &self,
        agent_registry: String,
        agent_id: String,
        client_address: String,
        feedback_index: String,
    ) -> Result<Option<AgentFeedbackOutput>> {
        let commerce = self.commerce.lock().await;
        let feedback = commerce
            .erc8004()
            .read_feedback(
                &agent_registry,
                &agent_id,
                &client_address,
                parse_u64_str(&feedback_index, "feedback_index")?,
            )
            .map_err(|e| Error::from_reason(format!("Failed to read agent feedback: {}", e)))?;
        Ok(feedback.map(Into::into))
    }

    #[napi]
    pub async fn read_all_feedback(
        &self,
        filter: Option<AgentFeedbackFilterInput>,
    ) -> Result<Vec<AgentFeedbackOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.map_or_else(stateset_core::AgentFeedbackFilter::default, |f| {
            stateset_core::AgentFeedbackFilter {
                agent_registry: f.agent_registry,
                agent_id: f.agent_id,
                client_addresses: f.client_addresses,
                tag1: f.tag1,
                tag2: f.tag2,
                include_revoked: f.include_revoked,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let feedback = commerce
            .erc8004()
            .read_all_feedback(filter)
            .map_err(|e| Error::from_reason(format!("Failed to read agent feedback: {}", e)))?;
        Ok(feedback.into_iter().map(Into::into).collect())
    }

    /// Aggregate feedback summary for an agent.
    #[napi]
    pub async fn feedback_summary(
        &self,
        agent_registry: String,
        agent_id: String,
        client_addresses: Option<Vec<String>>,
        tag1: Option<String>,
        tag2: Option<String>,
    ) -> Result<FeedbackSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let summary = commerce
            .erc8004()
            .feedback_summary(
                &agent_registry,
                &agent_id,
                client_addresses.unwrap_or_default(),
                tag1,
                tag2,
            )
            .map_err(|e| Error::from_reason(format!("Failed to get feedback summary: {}", e)))?;
        Ok(summary.into())
    }

    // ---- Validation registry ----

    /// Submit a validation request for an agent.
    #[napi]
    pub async fn request_validation(
        &self,
        input: CreateAgentValidationRequestInput,
    ) -> Result<AgentValidationRequestOutput> {
        let commerce = self.commerce.lock().await;
        let request = commerce
            .erc8004()
            .request_validation(stateset_core::CreateAgentValidationRequest {
                request_hash: input.request_hash,
                agent_registry: input.agent_registry,
                agent_id: input.agent_id,
                validator_address: input.validator_address,
                request_uri: input.request_uri,
            })
            .map_err(|e| Error::from_reason(format!("Failed to request validation: {}", e)))?;
        Ok(request.into())
    }

    /// Record a validator's response to a validation request.
    #[napi]
    pub async fn respond_validation(
        &self,
        request_hash: String,
        input: CreateAgentValidationResponseInput,
    ) -> Result<AgentValidationResponseOutput> {
        let commerce = self.commerce.lock().await;
        let response = commerce
            .erc8004()
            .respond_validation(
                &request_hash,
                stateset_core::CreateAgentValidationResponse {
                    response: parse_u8_field(input.response, "response")?,
                    response_uri: input.response_uri,
                    response_hash: input.response_hash,
                    tag: input.tag,
                },
            )
            .map_err(|e| Error::from_reason(format!("Failed to respond to validation: {}", e)))?;
        Ok(response.into())
    }

    #[napi]
    pub async fn validation_status(
        &self,
        request_hash: String,
    ) -> Result<Option<AgentValidationStatusOutput>> {
        let commerce = self.commerce.lock().await;
        let status = commerce
            .erc8004()
            .validation_status(&request_hash)
            .map_err(|e| Error::from_reason(format!("Failed to get validation status: {}", e)))?;
        Ok(status.map(Into::into))
    }

    /// Aggregate validation summary for an agent.
    #[napi]
    pub async fn validation_summary(
        &self,
        agent_registry: String,
        agent_id: String,
        validator_addresses: Option<Vec<String>>,
        tag: Option<String>,
    ) -> Result<ValidationSummaryOutput> {
        let commerce = self.commerce.lock().await;
        let summary = commerce
            .erc8004()
            .validation_summary(&agent_registry, &agent_id, validator_addresses, tag)
            .map_err(|e| Error::from_reason(format!("Failed to get validation summary: {}", e)))?;
        Ok(summary.into())
    }
}

fn build_agent_identity_filter(
    filter: Option<AgentIdentityFilterInput>,
) -> Result<stateset_core::AgentIdentityFilter> {
    filter.map_or_else(
        || Ok(stateset_core::AgentIdentityFilter::default()),
        |f| -> Result<stateset_core::AgentIdentityFilter> {
            Ok(stateset_core::AgentIdentityFilter {
                agent_registry: f.agent_registry,
                agent_id: f.agent_id,
                agent_wallet: f.agent_wallet,
                owner_address: f.owner_address,
                agent_card_id: parse_optional_uuid(f.agent_card_id, "agent_card_id")?,
                active: f.active,
                limit: f.limit,
                offset: f.offset,
            })
        },
    )
}

// ---------------------------------------------------------------------------
// Maintenance (backup / restore / structured export & import)
// ---------------------------------------------------------------------------

/// Manifest written alongside a database backup.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct BackupManifestOutput {
    pub manifest_version: u32,
    pub schema_version: String,
    pub migration_count: u32,
    pub engine_version: String,
    pub created_at: String,
    pub source_path: String,
    pub size_bytes: i64,
    pub checksum: String,
}

/// Result of a database backup.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct BackupReportOutput {
    pub backup_path: String,
    pub manifest_path: String,
    pub manifest: BackupManifestOutput,
}

/// Options controlling a restore.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RestoreOptionsInput {
    pub overwrite: Option<bool>,
    pub skip_checksum: Option<bool>,
    pub allow_newer_schema: Option<bool>,
}

/// Result of a database restore.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct RestoreReportOutput {
    pub target_path: String,
    pub schema_version: String,
    pub size_bytes: i64,
    pub checksum_verified: bool,
    pub replaced_existing: bool,
}

/// Per-domain record count.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct DomainCountOutput {
    pub domain: String,
    pub count: u32,
}

/// Options controlling a structured export.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ExportOptionsInput {
    pub domains: Option<Vec<String>>,
    pub page_size: Option<u32>,
    pub pretty: Option<bool>,
}

/// Result of a structured export.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ExportReportOutput {
    pub counts: Vec<DomainCountOutput>,
    pub total: u32,
}

/// Options controlling a structured import.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ImportOptionsInput {
    pub domains: Option<Vec<String>>,
    /// `skip` (default) or `fail`.
    pub on_conflict: Option<String>,
    pub dry_run: Option<bool>,
}

/// Result of a structured import.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct ImportReportOutput {
    pub created: Vec<DomainCountOutput>,
    pub skipped: Vec<DomainCountOutput>,
    pub unsupported_domains: Vec<String>,
    pub total_created: u32,
}

/// Domains that structured export/import can cover.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct PortableDomainsOutput {
    pub exportable: Vec<String>,
    pub importable: Vec<String>,
}

fn domain_counts(counts: Vec<(String, usize)>) -> Vec<DomainCountOutput> {
    counts
        .into_iter()
        .map(|(domain, count)| DomainCountOutput { domain, count: count as u32 })
        .collect()
}

#[napi]
pub struct Maintenance {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Maintenance {
    /// Whether file-level backup and restore are available on this instance.
    #[napi]
    pub async fn supports_backup(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.maintenance().supports_backup())
    }

    /// Alias of `supportsBackup`, matching the other accessor modules.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.maintenance().supports_backup())
    }

    /// Take a consistent backup to `backupPath`, writing a sidecar manifest.
    #[napi]
    pub async fn backup(&self, backup_path: String) -> Result<BackupReportOutput> {
        let commerce = self.commerce.lock().await;
        let report = commerce
            .maintenance()
            .backup_to(&backup_path)
            .map_err(|e| Error::from_reason(format!("Failed to back up database: {}", e)))?;
        let m = report.manifest;
        Ok(BackupReportOutput {
            backup_path: report.backup_path.display().to_string(),
            manifest_path: report.manifest_path.display().to_string(),
            manifest: BackupManifestOutput {
                manifest_version: m.manifest_version,
                schema_version: m.schema_version,
                migration_count: m.migration_count as u32,
                engine_version: m.engine_version,
                created_at: m.created_at.to_rfc3339(),
                source_path: m.source_path,
                size_bytes: m.size_bytes as i64,
                checksum: m.checksum,
            },
        })
    }

    /// Alias of `backup`.
    #[napi]
    pub async fn backup_to(&self, backup_path: String) -> Result<BackupReportOutput> {
        self.backup(backup_path).await
    }

    /// Restore a backup to `targetPath`.
    #[napi]
    pub async fn restore(
        &self,
        backup_path: String,
        target_path: String,
        options: Option<RestoreOptionsInput>,
    ) -> Result<RestoreReportOutput> {
        let commerce = self.commerce.lock().await;
        let opts = options.unwrap_or(RestoreOptionsInput {
            overwrite: None,
            skip_checksum: None,
            allow_newer_schema: None,
        });
        let restore_options = stateset_embedded::maintenance::RestoreOptions {
            overwrite: opts.overwrite.unwrap_or(false),
            skip_checksum: opts.skip_checksum.unwrap_or(false),
            allow_newer_schema: opts.allow_newer_schema.unwrap_or(false),
        };
        let report = commerce
            .maintenance()
            .restore_from(&backup_path, &target_path, &restore_options)
            .map_err(|e| Error::from_reason(format!("Failed to restore database: {}", e)))?;
        Ok(RestoreReportOutput {
            target_path: report.target_path.display().to_string(),
            schema_version: report.schema_version,
            size_bytes: report.size_bytes as i64,
            checksum_verified: report.checksum_verified,
            replaced_existing: report.replaced_existing,
        })
    }

    /// Alias of `restore`.
    #[napi]
    pub async fn restore_from(
        &self,
        backup_path: String,
        target_path: String,
        options: Option<RestoreOptionsInput>,
    ) -> Result<RestoreReportOutput> {
        self.restore(backup_path, target_path, options).await
    }

    /// Write a structured JSON export to `path`.
    #[napi]
    pub async fn export(
        &self,
        path: String,
        options: Option<ExportOptionsInput>,
    ) -> Result<ExportReportOutput> {
        let commerce = self.commerce.lock().await;
        let mut export_options = stateset_embedded::maintenance::ExportOptions::default();
        if let Some(o) = options {
            if let Some(domains) = o.domains {
                export_options.domains = domains;
            }
            if let Some(page_size) = o.page_size.filter(|p| *p > 0) {
                export_options.page_size = page_size;
            }
            if let Some(pretty) = o.pretty {
                export_options.pretty = pretty;
            }
        }
        let report = commerce
            .maintenance()
            .export_to_file_with(&path, &export_options)
            .map_err(|e| Error::from_reason(format!("Failed to export data: {}", e)))?;
        Ok(ExportReportOutput { total: report.total as u32, counts: domain_counts(report.counts) })
    }

    /// Alias of `export`.
    #[napi]
    pub async fn export_to_file(
        &self,
        path: String,
        options: Option<ExportOptionsInput>,
    ) -> Result<ExportReportOutput> {
        self.export(path, options).await
    }

    /// Read a structured JSON export from `path` and replay it.
    #[napi]
    pub async fn import(
        &self,
        path: String,
        options: Option<ImportOptionsInput>,
    ) -> Result<ImportReportOutput> {
        let commerce = self.commerce.lock().await;
        let mut import_options = stateset_embedded::maintenance::ImportOptions::default();
        if let Some(o) = options {
            if let Some(domains) = o.domains {
                import_options.domains = domains;
            }
            if let Some(policy) = o.on_conflict {
                import_options.on_conflict = match policy.as_str() {
                    "skip" => stateset_embedded::maintenance::ConflictPolicy::Skip,
                    "fail" => stateset_embedded::maintenance::ConflictPolicy::Fail,
                    other => {
                        return Err(Error::from_reason(format!(
                            "Invalid onConflict '{}': expected 'skip' or 'fail'",
                            other
                        )));
                    }
                };
            }
            if let Some(dry_run) = o.dry_run {
                import_options.dry_run = dry_run;
            }
        }
        let report = commerce
            .maintenance()
            .import_from_file(&path, &import_options)
            .map_err(|e| Error::from_reason(format!("Failed to import data: {}", e)))?;
        Ok(ImportReportOutput {
            created: domain_counts(report.created),
            skipped: domain_counts(report.skipped),
            unsupported_domains: report.unsupported_domains,
            total_created: report.total_created as u32,
        })
    }

    /// Alias of `import`.
    #[napi]
    pub async fn import_from_file(
        &self,
        path: String,
        options: Option<ImportOptionsInput>,
    ) -> Result<ImportReportOutput> {
        self.import(path, options).await
    }

    /// Domains the structured export covers, in export order.
    #[napi]
    pub async fn exportable_domains(&self) -> Result<Vec<String>> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.maintenance().exportable_domains().into_iter().map(ToOwned::to_owned).collect())
    }

    /// Domains the structured import can write.
    #[napi]
    pub async fn importable_domains(&self) -> Result<Vec<String>> {
        let commerce = self.commerce.lock().await;
        Ok(commerce.maintenance().importable_domains().into_iter().map(ToOwned::to_owned).collect())
    }

    /// Both portable domain lists in one call.
    #[napi]
    pub async fn list_portable_domains(&self) -> Result<PortableDomainsOutput> {
        let commerce = self.commerce.lock().await;
        let maintenance = commerce.maintenance();
        Ok(PortableDomainsOutput {
            exportable: maintenance
                .exportable_domains()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            importable: maintenance
                .importable_domains()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
        })
    }
}
