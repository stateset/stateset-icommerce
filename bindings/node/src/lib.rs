#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_embedded::Commerce as RustCommerce;
use std::sync::Arc;
use tokio::sync::Mutex;

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

        Ok(Self {
            inner: Arc::new(Mutex::new(commerce)),
        })
    }

    /// Get the customers API
    #[napi(getter)]
    pub fn customers(&self) -> Customers {
        Customers {
            commerce: self.inner.clone(),
        }
    }

    /// Get the orders API
    #[napi(getter)]
    pub fn orders(&self) -> Orders {
        Orders {
            commerce: self.inner.clone(),
        }
    }

    /// Get the products API
    #[napi(getter)]
    pub fn products(&self) -> Products {
        Products {
            commerce: self.inner.clone(),
        }
    }

    /// Get the inventory API
    #[napi(getter)]
    pub fn inventory(&self) -> Inventory {
        Inventory {
            commerce: self.inner.clone(),
        }
    }

    /// Get the returns API
    #[napi(getter)]
    pub fn returns(&self) -> Returns {
        Returns {
            commerce: self.inner.clone(),
        }
    }

    /// Get the payments API
    #[napi(getter)]
    pub fn payments(&self) -> Payments {
        Payments {
            commerce: self.inner.clone(),
        }
    }

    /// Get the shipments API
    #[napi(getter)]
    pub fn shipments(&self) -> Shipments {
        Shipments {
            commerce: self.inner.clone(),
        }
    }

    /// Get the warranties API
    #[napi(getter)]
    pub fn warranties(&self) -> Warranties {
        Warranties {
            commerce: self.inner.clone(),
        }
    }

    /// Get the purchase orders API
    #[napi(getter)]
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders {
            commerce: self.inner.clone(),
        }
    }

    /// Get the invoices API
    #[napi(getter)]
    pub fn invoices(&self) -> Invoices {
        Invoices {
            commerce: self.inner.clone(),
        }
    }

    /// Get the bill of materials API
    #[napi(getter)]
    pub fn bom(&self) -> Bom {
        Bom {
            commerce: self.inner.clone(),
        }
    }

    /// Get the work orders API
    #[napi(getter)]
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders {
            commerce: self.inner.clone(),
        }
    }

    /// Get the carts/checkout API
    #[napi(getter)]
    pub fn carts(&self) -> Carts {
        Carts {
            commerce: self.inner.clone(),
        }
    }

    /// Get the analytics API
    #[napi(getter)]
    pub fn analytics(&self) -> Analytics {
        Analytics {
            commerce: self.inner.clone(),
        }
    }

    /// Get the currency API
    #[napi(getter)]
    pub fn currency(&self) -> CurrencyOperations {
        CurrencyOperations {
            commerce: self.inner.clone(),
        }
    }

    /// Get the subscriptions API
    #[napi(getter)]
    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions {
            commerce: self.inner.clone(),
        }
    }

    /// Get the promotions API
    #[napi(getter)]
    pub fn promotions(&self) -> Promotions {
        Promotions {
            commerce: self.inner.clone(),
        }
    }

    /// Get the tax API
    #[napi(getter)]
    pub fn tax(&self) -> Tax {
        Tax {
            commerce: self.inner.clone(),
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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let customer = commerce
            .customers()
            .get(uuid)
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

impl From<stateset_core::Order> for OrderOutput {
    fn from(o: stateset_core::Order) -> Self {
        Self {
            id: o.id.to_string(),
            order_number: o.order_number,
            customer_id: o.customer_id.to_string(),
            status: format!("{}", o.status),
            total_amount: o.total_amount.to_string().parse().unwrap_or(0.0),
            currency: o.currency,
            payment_status: format!("{}", o.payment_status),
            fulfillment_status: format!("{}", o.fulfillment_status),
            tracking_number: o.tracking_number,
            items: o
                .items
                .into_iter()
                .map(|i| OrderItemOutput {
                    id: i.id.to_string(),
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: i.unit_price.to_string().parse().unwrap_or(0.0),
                    total: i.total.to_string().parse().unwrap_or(0.0),
                })
                .collect(),
            version: o.version,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
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

        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let items: Vec<stateset_core::CreateOrderItem> = input
            .items
            .into_iter()
            .map(|i| {
                let product_id = i
                    .product_id
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_default();
                let variant_id = i.variant_id.and_then(|s| s.parse().ok());

                stateset_core::CreateOrderItem {
                    product_id,
                    variant_id,
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: Decimal::from_f64_retain(i.unit_price).unwrap_or_default(),
                    ..Default::default()
                }
            })
            .collect();

        let order = commerce
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id,
                items,
                currency: input.currency,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create order: {}", e)))?;

        Ok(order.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<OrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order = commerce
            .orders()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get order: {}", e)))?;

        Ok(order.map(|o| o.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<OrderOutput>> {
        let commerce = self.commerce.lock().await;
        let orders = commerce
            .orders()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list orders: {}", e)))?;

        Ok(orders.into_iter().map(|o| o.into()).collect())
    }

    #[napi]
    pub async fn update_status(&self, id: String, status: String) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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
            .update_status(uuid, order_status)
            .map_err(|e| Error::from_reason(format!("Failed to update order: {}", e)))?;

        Ok(order.into())
    }

    #[napi]
    pub async fn ship(&self, id: String, tracking_number: Option<String>) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order = commerce
            .orders()
            .ship(uuid, tracking_number.as_deref())
            .map_err(|e| Error::from_reason(format!("Failed to ship order: {}", e)))?;

        Ok(order.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<OrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let order = commerce
            .orders()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel order: {}", e)))?;

        Ok(order.into())
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

impl From<stateset_core::ProductVariant> for ProductVariantOutput {
    fn from(v: stateset_core::ProductVariant) -> Self {
        Self {
            id: v.id.to_string(),
            product_id: v.product_id.to_string(),
            sku: v.sku,
            name: v.name,
            price: v.price.to_string().parse().unwrap_or(0.0),
            compare_at_price: v.compare_at_price.map(|d| d.to_string().parse().unwrap_or(0.0)),
            is_default: v.is_default,
        }
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

        let variants = input.variants.map(|vs| {
            vs.into_iter()
                .map(|v| stateset_core::CreateProductVariant {
                    sku: v.sku,
                    name: v.name,
                    price: Decimal::from_f64_retain(v.price).unwrap_or_default(),
                    compare_at_price: v
                        .compare_at_price
                        .and_then(|p| Decimal::from_f64_retain(p)),
                    ..Default::default()
                })
                .collect()
        });

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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let product = commerce
            .products()
            .get(uuid)
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

        Ok(variant.map(|v| v.into()))
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

impl From<stateset_core::StockLevel> for StockLevelOutput {
    fn from(s: stateset_core::StockLevel) -> Self {
        Self {
            sku: s.sku,
            name: s.name,
            total_on_hand: s.total_on_hand.to_string().parse().unwrap_or(0.0),
            total_allocated: s.total_allocated.to_string().parse().unwrap_or(0.0),
            total_available: s.total_available.to_string().parse().unwrap_or(0.0),
        }
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

impl From<stateset_core::InventoryReservation> for ReservationOutput {
    fn from(r: stateset_core::InventoryReservation) -> Self {
        Self {
            id: r.id.to_string(),
            item_id: r.item_id,
            quantity: r.quantity.to_string().parse().unwrap_or(0.0),
            status: format!("{}", r.status),
        }
    }
}

#[napi]
pub struct Inventory {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[napi]
impl Inventory {
    #[napi]
    pub async fn create_item(&self, input: CreateInventoryItemInput) -> Result<InventoryItemOutput> {
        let commerce = self.commerce.lock().await;

        let item = commerce
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku: input.sku,
                name: input.name,
                description: input.description,
                initial_quantity: input
                    .initial_quantity
                    .and_then(|q| Decimal::from_f64_retain(q)),
                reorder_point: input.reorder_point.and_then(|r| Decimal::from_f64_retain(r)),
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

        Ok(stock.map(|s| s.into()))
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

        Ok(reservation.into())
    }

    #[napi]
    pub async fn confirm_reservation(&self, reservation_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = reservation_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        commerce
            .inventory()
            .confirm_reservation(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to confirm reservation: {}", e)))?;

        Ok(())
    }

    #[napi]
    pub async fn release_reservation(&self, reservation_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = reservation_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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

        let order_id = input
            .order_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid order UUID"))?;

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
                items,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create return: {}", e)))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ReturnOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get return: {}", e)))?;

        Ok(ret.map(|r| r.into()))
    }

    #[napi]
    pub async fn approve(&self, id: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .approve(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to approve return: {}", e)))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn reject(&self, id: String, reason: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .reject(uuid, &reason)
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
    pub amount: f64,
    pub currency: String,
    pub status: String,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Payment> for PaymentOutput {
    fn from(p: stateset_core::Payment) -> Self {
        Self {
            id: p.id.to_string(),
            payment_number: p.payment_number,
            order_id: p.order_id.map(|id| id.to_string()),
            invoice_id: p.invoice_id.map(|id| id.to_string()),
            customer_id: p.customer_id.map(|id| id.to_string()),
            amount: p.amount.to_string().parse().unwrap_or(0.0),
            currency: p.currency,
            status: format!("{}", p.status),
            version: p.version,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRefundInput {
    pub payment_id: String,
    pub amount: f64,
    pub reason: Option<String>,
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
}

impl From<stateset_core::Refund> for RefundOutput {
    fn from(r: stateset_core::Refund) -> Self {
        Self {
            id: r.id.to_string(),
            refund_number: r.refund_number,
            payment_id: r.payment_id.to_string(),
            amount: r.amount.to_string().parse().unwrap_or(0.0),
            status: format!("{}", r.status),
            reason: r.reason,
            created_at: r.created_at.to_rfc3339(),
        }
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

        let payment_method = input.payment_method.map(|m| match m.to_lowercase().as_str() {
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
        }).unwrap_or(stateset_core::PaymentMethodType::CreditCard);

        let payment = commerce
            .payments()
            .create(stateset_core::CreatePayment {
                order_id,
                invoice_id,
                customer_id,
                amount: Decimal::from_f64_retain(input.amount).unwrap_or_default(),
                currency: input.currency,
                payment_method,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create payment: {}", e)))?;

        Ok(payment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PaymentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get payment: {}", e)))?;

        Ok(payment.map(|p| p.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<PaymentOutput>> {
        let commerce = self.commerce.lock().await;
        let payments = commerce
            .payments()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list payments: {}", e)))?;

        Ok(payments.into_iter().map(|p| p.into()).collect())
    }

    #[napi]
    pub async fn mark_completed(&self, id: String) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_completed(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to complete payment: {}", e)))?;

        Ok(payment.into())
    }

    #[napi]
    pub async fn mark_failed(&self, id: String, reason: String, code: Option<String>) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_failed(uuid, &reason, code.as_deref())
            .map_err(|e| Error::from_reason(format!("Failed to fail payment: {}", e)))?;

        Ok(payment.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<PaymentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel payment: {}", e)))?;

        Ok(payment.into())
    }

    #[napi]
    pub async fn create_refund(&self, input: CreateRefundInput) -> Result<RefundOutput> {
        let commerce = self.commerce.lock().await;
        let payment_id = input
            .payment_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid payment UUID"))?;

        let refund = commerce
            .payments()
            .create_refund(stateset_core::CreateRefund {
                payment_id,
                amount: Some(Decimal::from_f64_retain(input.amount).unwrap_or_default()),
                reason: input.reason,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create refund: {}", e)))?;

        Ok(refund.into())
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

        let order_id = input
            .order_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let carrier = input.carrier.and_then(|c| match c.to_lowercase().as_str() {
            "ups" => Some(stateset_core::ShippingCarrier::Ups),
            "fedex" => Some(stateset_core::ShippingCarrier::FedEx),
            "usps" => Some(stateset_core::ShippingCarrier::Usps),
            "dhl" => Some(stateset_core::ShippingCarrier::Dhl),
            _ => Some(stateset_core::ShippingCarrier::Other),
        });

        let shipping_method = input.shipping_method.and_then(|m| match m.to_lowercase().as_str() {
            "standard" => Some(stateset_core::ShippingMethod::Standard),
            "express" => Some(stateset_core::ShippingMethod::Express),
            "overnight" => Some(stateset_core::ShippingMethod::Overnight),
            "ground" => Some(stateset_core::ShippingMethod::Ground),
            "twoday" | "two_day" => Some(stateset_core::ShippingMethod::TwoDay),
            "sameday" | "same_day" => Some(stateset_core::ShippingMethod::SameDay),
            "international" => Some(stateset_core::ShippingMethod::International),
            "freight" => Some(stateset_core::ShippingMethod::Freight),
            _ => Some(stateset_core::ShippingMethod::Standard),
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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .get(uuid)
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
    pub async fn ship(&self, id: String, tracking_number: Option<String>) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .ship(uuid, tracking_number)
            .map_err(|e| Error::from_reason(format!("Failed to ship: {}", e)))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn deliver(&self, id: String) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .mark_delivered(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to deliver: {}", e)))?;

        Ok(shipment.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<ShipmentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .cancel(uuid)
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

        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid customer UUID"))?;

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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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
    pub async fn create_claim(&self, input: CreateWarrantyClaimInput) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let warranty_id = input
            .warranty_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid warranty UUID"))?;

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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .approve_claim(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to approve claim: {}", e)))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn deny_claim(&self, id: String, reason: String) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .deny_claim(uuid, &reason)
            .map_err(|e| Error::from_reason(format!("Failed to deny claim: {}", e)))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn complete_claim(&self, id: String, resolution: String) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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

impl From<stateset_core::PurchaseOrder> for PurchaseOrderOutput {
    fn from(po: stateset_core::PurchaseOrder) -> Self {
        Self {
            id: po.id.to_string(),
            po_number: po.po_number,
            supplier_id: po.supplier_id.to_string(),
            status: format!("{}", po.status),
            subtotal: po.subtotal.to_string().parse().unwrap_or(0.0),
            total: po.total.to_string().parse().unwrap_or(0.0),
            created_at: po.created_at.to_rfc3339(),
            updated_at: po.updated_at.to_rfc3339(),
        }
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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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

        let supplier_id = input
            .supplier_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid supplier UUID"))?;

        let items: Vec<stateset_core::CreatePurchaseOrderItem> = input
            .items
            .into_iter()
            .map(|i| stateset_core::CreatePurchaseOrderItem {
                sku: i.sku,
                name: i.name,
                quantity: Decimal::from_f64_retain(i.quantity).unwrap_or_default(),
                unit_cost: Decimal::from_f64_retain(i.unit_cost).unwrap_or_default(),
                ..Default::default()
            })
            .collect();

        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id,
                items,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create PO: {}", e)))?;

        Ok(po.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PurchaseOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get PO: {}", e)))?;

        Ok(po.map(|p| p.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<PurchaseOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let pos = commerce
            .purchase_orders()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list POs: {}", e)))?;

        Ok(pos.into_iter().map(|p| p.into()).collect())
    }

    #[napi]
    pub async fn submit(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .submit(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to submit PO: {}", e)))?;

        Ok(po.into())
    }

    #[napi]
    pub async fn approve(&self, id: String, approved_by: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .approve(uuid, &approved_by)
            .map_err(|e| Error::from_reason(format!("Failed to approve PO: {}", e)))?;

        Ok(po.into())
    }

    #[napi]
    pub async fn send(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .send(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to send PO: {}", e)))?;

        Ok(po.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<PurchaseOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel PO: {}", e)))?;

        Ok(po.into())
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

impl From<stateset_core::Invoice> for InvoiceOutput {
    fn from(inv: stateset_core::Invoice) -> Self {
        Self {
            id: inv.id.to_string(),
            invoice_number: inv.invoice_number,
            customer_id: inv.customer_id.to_string(),
            order_id: inv.order_id.map(|id| id.to_string()),
            status: format!("{}", inv.status),
            subtotal: inv.subtotal.to_string().parse().unwrap_or(0.0),
            tax_amount: inv.tax_amount.to_string().parse().unwrap_or(0.0),
            total: inv.total.to_string().parse().unwrap_or(0.0),
            amount_paid: inv.amount_paid.to_string().parse().unwrap_or(0.0),
            due_date: inv.due_date.to_rfc3339(),
            created_at: inv.created_at.to_rfc3339(),
            updated_at: inv.updated_at.to_rfc3339(),
        }
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

        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| Error::from_reason("Invalid order UUID"))?;

        let items: Vec<stateset_core::CreateInvoiceItem> = input
            .items
            .into_iter()
            .map(|i| stateset_core::CreateInvoiceItem {
                description: i.description,
                quantity: Decimal::from_f64_retain(i.quantity).unwrap_or_default(),
                unit_price: Decimal::from_f64_retain(i.unit_price).unwrap_or_default(),
                sku: i.sku,
                ..Default::default()
            })
            .collect();

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

        Ok(invoice.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<InvoiceOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get invoice: {}", e)))?;

        Ok(invoice.map(|i| i.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<InvoiceOutput>> {
        let commerce = self.commerce.lock().await;
        let invoices = commerce
            .invoices()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    #[napi]
    pub async fn send(&self, id: String) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .send(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to send invoice: {}", e)))?;

        Ok(invoice.into())
    }

    #[napi]
    pub async fn void(&self, id: String) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .void(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to void invoice: {}", e)))?;

        Ok(invoice.into())
    }

    #[napi]
    pub async fn record_payment(&self, id: String, input: RecordPaymentInput) -> Result<InvoiceOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .record_payment(uuid, stateset_core::RecordInvoicePayment {
                amount: Decimal::from_f64_retain(input.amount).unwrap_or_default(),
                payment_method: input.payment_method,
                reference: input.reference,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to record payment: {}", e)))?;

        Ok(invoice.into())
    }

    #[napi]
    pub async fn get_overdue(&self) -> Result<Vec<InvoiceOutput>> {
        let commerce = self.commerce.lock().await;
        let invoices = commerce
            .invoices()
            .get_overdue()
            .map_err(|e| Error::from_reason(format!("Failed to get overdue invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
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

impl From<stateset_core::BomComponent> for BomComponentOutput {
    fn from(c: stateset_core::BomComponent) -> Self {
        Self {
            id: c.id.to_string(),
            bom_id: c.bom_id.to_string(),
            component_sku: c.component_sku,
            name: c.name,
            quantity: c.quantity.to_string().parse().unwrap_or(0.0),
            unit_of_measure: c.unit_of_measure,
        }
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

        let product_id = input
            .product_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid product UUID"))?;

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
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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
    pub async fn add_component(&self, bom_id: String, input: CreateBomComponentInput) -> Result<BomComponentOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = bom_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid BOM UUID"))?;

        let component = commerce
            .bom()
            .add_component(uuid, stateset_core::CreateBomComponent {
                component_sku: input.component_sku,
                name: input.name,
                quantity: Decimal::from_f64_retain(input.quantity).unwrap_or_default(),
                unit_of_measure: input.unit_of_measure,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to add component: {}", e)))?;

        Ok(component.into())
    }

    #[napi]
    pub async fn get_components(&self, bom_id: String) -> Result<Vec<BomComponentOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = bom_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid BOM UUID"))?;

        let components = commerce
            .bom()
            .get_components(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get components: {}", e)))?;

        Ok(components.into_iter().map(|c| c.into()).collect())
    }

    #[napi]
    pub async fn activate(&self, id: String) -> Result<BomOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

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

impl From<stateset_core::WorkOrder> for WorkOrderOutput {
    fn from(wo: stateset_core::WorkOrder) -> Self {
        Self {
            id: wo.id.to_string(),
            work_order_number: wo.work_order_number,
            product_id: wo.product_id.to_string(),
            bom_id: wo.bom_id.map(|id| id.to_string()),
            status: format!("{}", wo.status),
            priority: format!("{}", wo.priority),
            quantity_to_build: wo.quantity_to_build.to_string().parse().unwrap_or(0.0),
            quantity_completed: wo.quantity_completed.to_string().parse().unwrap_or(0.0),
            version: wo.version,
            created_at: wo.created_at.to_rfc3339(),
            updated_at: wo.updated_at.to_rfc3339(),
        }
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

        let product_id = input
            .product_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid product UUID"))?;

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
                quantity_to_build: Decimal::from_f64_retain(input.quantity_to_build).unwrap_or_default(),
                priority,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create work order: {}", e)))?;

        Ok(wo.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WorkOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get work order: {}", e)))?;

        Ok(wo.map(|w| w.into()))
    }

    #[napi]
    pub async fn list(&self) -> Result<Vec<WorkOrderOutput>> {
        let commerce = self.commerce.lock().await;
        let orders = commerce
            .work_orders()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list work orders: {}", e)))?;

        Ok(orders.into_iter().map(|w| w.into()).collect())
    }

    #[napi]
    pub async fn start(&self, id: String) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .start(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to start work order: {}", e)))?;

        Ok(wo.into())
    }

    #[napi]
    pub async fn complete(&self, id: String, quantity_completed: f64) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .complete(uuid, Decimal::from_f64_retain(quantity_completed).unwrap_or_default())
            .map_err(|e| Error::from_reason(format!("Failed to complete work order: {}", e)))?;

        Ok(wo.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<WorkOrderOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel work order: {}", e)))?;

        Ok(wo.into())
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

impl From<stateset_core::CartItem> for CartItemOutput {
    fn from(item: stateset_core::CartItem) -> Self {
        Self {
            id: item.id.to_string(),
            cart_id: item.cart_id.to_string(),
            product_id: item.product_id.map(|id| id.to_string()),
            variant_id: item.variant_id.map(|id| id.to_string()),
            sku: item.sku,
            name: item.name,
            description: item.description,
            image_url: item.image_url,
            quantity: item.quantity,
            unit_price: item.unit_price.to_string().parse().unwrap_or(0.0),
            original_price: item.original_price.map(|p| p.to_string().parse().unwrap_or(0.0)),
            discount_amount: item.discount_amount.to_string().parse().unwrap_or(0.0),
            tax_amount: item.tax_amount.to_string().parse().unwrap_or(0.0),
            total: item.total.to_string().parse().unwrap_or(0.0),
            requires_shipping: item.requires_shipping,
            created_at: item.created_at.to_rfc3339(),
            updated_at: item.updated_at.to_rfc3339(),
        }
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

impl From<stateset_core::Cart> for CartOutput {
    fn from(cart: stateset_core::Cart) -> Self {
        // Compute item_count first before any fields are moved
        let item_count = cart.item_count();
        Self {
            id: cart.id.to_string(),
            cart_number: cart.cart_number,
            customer_id: cart.customer_id.map(|id| id.to_string()),
            status: format!("{}", cart.status),
            currency: cart.currency,
            subtotal: cart.subtotal.to_string().parse().unwrap_or(0.0),
            tax_amount: cart.tax_amount.to_string().parse().unwrap_or(0.0),
            shipping_amount: cart.shipping_amount.to_string().parse().unwrap_or(0.0),
            discount_amount: cart.discount_amount.to_string().parse().unwrap_or(0.0),
            grand_total: cart.grand_total.to_string().parse().unwrap_or(0.0),
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
        }
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

impl From<stateset_core::CheckoutResult> for CheckoutResultOutput {
    fn from(result: stateset_core::CheckoutResult) -> Self {
        Self {
            cart_id: result.cart_id.to_string(),
            order_id: result.order_id.to_string(),
            order_number: result.order_number,
            payment_id: result.payment_id.map(|id| id.to_string()),
            total_charged: result.total_charged.to_string().parse().unwrap_or(0.0),
            currency: result.currency,
        }
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

impl From<stateset_core::ShippingRate> for ShippingRateOutput {
    fn from(rate: stateset_core::ShippingRate) -> Self {
        Self {
            id: rate.id,
            carrier: rate.carrier,
            service: rate.service,
            description: rate.description,
            price: rate.price.to_string().parse().unwrap_or(0.0),
            currency: rate.currency,
            estimated_days: rate.estimated_days,
        }
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
                currency: input.currency,
                shipping_address: input.shipping_address.map(input_to_cart_address),
                billing_address: input.billing_address.map(input_to_cart_address),
                notes: input.notes,
                expires_in_minutes: input.expires_in_minutes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to create cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Get a cart by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get cart: {}", e)))?;

        Ok(cart.map(|c| c.into()))
    }

    /// Get a cart by cart number
    #[napi]
    pub async fn get_by_number(&self, cart_number: String) -> Result<Option<CartOutput>> {
        let commerce = self.commerce.lock().await;

        let cart = commerce
            .carts()
            .get_by_number(&cart_number)
            .map_err(|e| Error::from_reason(format!("Failed to get cart: {}", e)))?;

        Ok(cart.map(|c| c.into()))
    }

    /// Update a cart
    #[napi]
    pub async fn update(&self, id: String, input: UpdateCartInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .update(uuid, stateset_core::UpdateCart {
                customer_email: input.customer_email,
                customer_phone: input.customer_phone,
                customer_name: input.customer_name,
                shipping_method: input.shipping_method,
                coupon_code: input.coupon_code,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to update cart: {}", e)))?;

        Ok(cart.into())
    }

    /// List all carts
    #[napi]
    pub async fn list(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let carts = commerce
            .carts()
            .list(Default::default())
            .map_err(|e| Error::from_reason(format!("Failed to list carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// List carts for a customer
    #[napi]
    pub async fn for_customer(&self, customer_id: String) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = customer_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid customer UUID"))?;

        let carts = commerce
            .carts()
            .for_customer(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get customer carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// Delete a cart
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        commerce
            .carts()
            .delete(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to delete cart: {}", e)))?;

        Ok(())
    }

    /// Add an item to the cart
    #[napi]
    pub async fn add_item(&self, cart_id: String, item: AddCartItemInput) -> Result<CartItemOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = cart_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid cart UUID"))?;

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
            .add_item(uuid, stateset_core::AddCartItem {
                product_id,
                variant_id,
                sku: item.sku,
                name: item.name,
                description: item.description,
                image_url: item.image_url,
                quantity: item.quantity,
                unit_price: Decimal::from_f64_retain(item.unit_price).unwrap_or_default(),
                original_price: item.original_price.map(|p| Decimal::from_f64_retain(p).unwrap_or_default()),
                weight: item.weight.map(|w| Decimal::from_f64_retain(w).unwrap_or_default()),
                requires_shipping: item.requires_shipping,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to add item: {}", e)))?;

        Ok(cart_item.into())
    }

    /// Update a cart item
    #[napi]
    pub async fn update_item(&self, item_id: String, input: UpdateCartItemInput) -> Result<CartItemOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = item_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid item UUID"))?;

        let cart_item = commerce
            .carts()
            .update_item(uuid, stateset_core::UpdateCartItem {
                quantity: input.quantity,
                unit_price: input.unit_price.map(|p| Decimal::from_f64_retain(p).unwrap_or_default()),
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to update item: {}", e)))?;

        Ok(cart_item.into())
    }

    /// Remove an item from the cart
    #[napi]
    pub async fn remove_item(&self, item_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = item_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid item UUID"))?;

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
        let uuid = cart_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid cart UUID"))?;

        let items = commerce
            .carts()
            .get_items(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get items: {}", e)))?;

        Ok(items.into_iter().map(|i| i.into()).collect())
    }

    /// Clear all items from the cart
    #[napi]
    pub async fn clear_items(&self, cart_id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = cart_id
            .parse()
            .map_err(|_| Error::from_reason("Invalid cart UUID"))?;

        commerce
            .carts()
            .clear_items(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to clear items: {}", e)))?;

        Ok(())
    }

    /// Set the shipping address
    #[napi]
    pub async fn set_shipping_address(&self, id: String, address: CartAddressInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_shipping_address(uuid, input_to_cart_address(address))
            .map_err(|e| Error::from_reason(format!("Failed to set shipping address: {}", e)))?;

        Ok(cart.into())
    }

    /// Set shipping selection (address + method/carrier/amount)
    #[napi]
    pub async fn set_shipping(&self, id: String, input: SetCartShippingInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let shipping_amount = match input.shipping_amount {
            Some(amount) => Some(
                Decimal::from_f64_retain(amount)
                    .ok_or_else(|| Error::from_reason("Invalid shipping amount"))?,
            ),
            None => None,
        };

        let cart = commerce
            .carts()
            .set_shipping(uuid, stateset_core::SetCartShipping {
                shipping_address: input_to_cart_address(input.shipping_address),
                shipping_method: input.shipping_method,
                shipping_carrier: input.shipping_carrier,
                shipping_amount,
            })
            .map_err(|e| Error::from_reason(format!("Failed to set shipping: {}", e)))?;

        Ok(cart.into())
    }

    /// Set the billing address
    #[napi]
    pub async fn set_billing_address(&self, id: String, address: CartAddressInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_billing_address(uuid, input_to_cart_address(address))
            .map_err(|e| Error::from_reason(format!("Failed to set billing address: {}", e)))?;

        Ok(cart.into())
    }

    /// Get available shipping rates
    #[napi]
    pub async fn get_shipping_rates(&self, id: String) -> Result<Vec<ShippingRateOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let rates = commerce
            .carts()
            .get_shipping_rates(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get shipping rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    /// Set payment method
    #[napi]
    pub async fn set_payment(&self, id: String, input: SetCartPaymentInput) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_payment(uuid, stateset_core::SetCartPayment {
                payment_method: input.payment_method,
                payment_token: input.payment_token,
                ..Default::default()
            })
            .map_err(|e| Error::from_reason(format!("Failed to set payment: {}", e)))?;

        Ok(cart.into())
    }

    /// Apply a discount/coupon code
    #[napi]
    pub async fn apply_discount(&self, id: String, coupon_code: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .apply_discount(uuid, &coupon_code)
            .map_err(|e| Error::from_reason(format!("Failed to apply discount: {}", e)))?;

        Ok(cart.into())
    }

    /// Remove discount from cart
    #[napi]
    pub async fn remove_discount(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .remove_discount(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to remove discount: {}", e)))?;

        Ok(cart.into())
    }

    /// Mark cart as ready for payment
    #[napi]
    pub async fn mark_ready_for_payment(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .mark_ready_for_payment(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to mark ready: {}", e)))?;

        Ok(cart.into())
    }

    /// Begin checkout process
    #[napi]
    pub async fn begin_checkout(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .begin_checkout(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to begin checkout: {}", e)))?;

        Ok(cart.into())
    }

    /// Complete checkout and create order
    #[napi]
    pub async fn complete(&self, id: String) -> Result<CheckoutResultOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let result = commerce
            .carts()
            .complete(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to complete checkout: {}", e)))?;

        Ok(result.into())
    }

    /// Cancel a cart
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .cancel(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to cancel cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Mark cart as abandoned
    #[napi]
    pub async fn abandon(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .abandon(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to abandon cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Mark cart as expired
    #[napi]
    pub async fn expire(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .expire(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to expire cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Reserve inventory for cart items
    #[napi]
    pub async fn reserve_inventory(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .reserve_inventory(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to reserve inventory: {}", e)))?;

        Ok(cart.into())
    }

    /// Release reserved inventory for cart items
    #[napi]
    pub async fn release_inventory(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .release_inventory(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to release inventory: {}", e)))?;

        Ok(cart.into())
    }

    /// Recalculate cart totals
    #[napi]
    pub async fn recalculate(&self, id: String) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .recalculate(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to recalculate: {}", e)))?;

        Ok(cart.into())
    }

    /// Set tax amount
    #[napi]
    pub async fn set_tax(&self, id: String, tax_amount: f64) -> Result<CartOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = id
            .parse()
            .map_err(|_| Error::from_reason("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_tax(uuid, Decimal::from_f64_retain(tax_amount).unwrap_or_default())
            .map_err(|e| Error::from_reason(format!("Failed to set tax: {}", e)))?;

        Ok(cart.into())
    }

    /// Get abandoned carts
    #[napi]
    pub async fn get_abandoned(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let carts = commerce
            .carts()
            .get_abandoned()
            .map_err(|e| Error::from_reason(format!("Failed to get abandoned carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// Get expired carts
    #[napi]
    pub async fn get_expired(&self) -> Result<Vec<CartOutput>> {
        let commerce = self.commerce.lock().await;
        let carts = commerce
            .carts()
            .get_expired()
            .map_err(|e| Error::from_reason(format!("Failed to get expired carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
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
    pub async fn sales_summary(&self, query: Option<AnalyticsQueryInput>) -> Result<SalesSummaryOutput> {
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
            total_revenue: summary.total_revenue.to_string().parse().unwrap_or(0.0),
            order_count: summary.order_count as u32,
            average_order_value: summary.average_order_value.to_string().parse().unwrap_or(0.0),
            items_sold: summary.items_sold as u32,
            unique_customers: summary.unique_customers as u32,
        })
    }

    /// Get revenue broken down by time periods
    #[napi]
    pub async fn revenue_by_period(&self, query: Option<AnalyticsQueryInput>) -> Result<Vec<RevenueByPeriodOutput>> {
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

        Ok(revenue.into_iter().map(|r| RevenueByPeriodOutput {
            period: r.period,
            revenue: r.revenue.to_string().parse().unwrap_or(0.0),
            order_count: r.order_count as u32,
            period_start: r.period_start.to_rfc3339(),
        }).collect())
    }

    /// Get top selling products
    #[napi]
    pub async fn top_products(&self, query: Option<AnalyticsQueryInput>) -> Result<Vec<TopProductOutput>> {
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

        Ok(products.into_iter().map(|p| TopProductOutput {
            product_id: p.product_id.map(|id| id.to_string()),
            sku: p.sku,
            name: p.name,
            units_sold: p.units_sold as u32,
            revenue: p.revenue.to_string().parse().unwrap_or(0.0),
            order_count: p.order_count as u32,
        }).collect())
    }

    /// Get product performance with period comparison
    #[napi]
    pub async fn product_performance(&self, query: Option<AnalyticsQueryInput>) -> Result<Vec<ProductPerformanceOutput>> {
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
                revenue: p.revenue.to_string().parse().unwrap_or(0.0),
                previous_units_sold: p.previous_units_sold as u32,
                previous_revenue: p.previous_revenue.to_string().parse().unwrap_or(0.0),
                units_growth_percent: p.units_growth_percent.to_string().parse().unwrap_or(0.0),
                revenue_growth_percent: p.revenue_growth_percent.to_string().parse().unwrap_or(0.0),
            })
            .collect())
    }

    /// Get customer metrics
    #[napi]
    pub async fn customer_metrics(&self, query: Option<AnalyticsQueryInput>) -> Result<CustomerMetricsOutput> {
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
            average_lifetime_value: metrics.average_lifetime_value.to_string().parse().unwrap_or(0.0),
            average_orders_per_customer: metrics.average_orders_per_customer.to_string().parse().unwrap_or(0.0),
        })
    }

    /// Get top customers by spend
    #[napi]
    pub async fn top_customers(&self, query: Option<AnalyticsQueryInput>) -> Result<Vec<TopCustomerOutput>> {
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

        Ok(customers.into_iter().map(|c| TopCustomerOutput {
            customer_id: c.customer_id.to_string(),
            name: c.name,
            email: c.email,
            order_count: c.order_count as u32,
            total_spent: c.total_spent.to_string().parse().unwrap_or(0.0),
            average_order_value: c.average_order_value.to_string().parse().unwrap_or(0.0),
        }).collect())
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
            total_value: health.total_value.to_string().parse().unwrap_or(0.0),
        })
    }

    /// Get low stock items
    #[napi]
    pub async fn low_stock_items(&self, threshold: Option<f64>) -> Result<Vec<LowStockItemOutput>> {
        let commerce = self.commerce.lock().await;

        let threshold_dec = threshold.map(|t| Decimal::from_f64_retain(t).unwrap_or_default());

        let items = commerce
            .analytics()
            .low_stock_items(threshold_dec)
            .map_err(|e| Error::from_reason(format!("Failed to get low stock items: {}", e)))?;

        Ok(items.into_iter().map(|i| LowStockItemOutput {
            sku: i.sku,
            name: i.name,
            on_hand: i.on_hand.to_string().parse().unwrap_or(0.0),
            allocated: i.allocated.to_string().parse().unwrap_or(0.0),
            available: i.available.to_string().parse().unwrap_or(0.0),
            reorder_point: i.reorder_point.map(|d| d.to_string().parse().unwrap_or(0.0)),
            average_daily_sales: i.average_daily_sales.map(|d| d.to_string().parse().unwrap_or(0.0)),
            days_of_stock: i.days_of_stock.map(|d| d.to_string().parse().unwrap_or(0.0)),
        }).collect())
    }

    /// Get inventory movement summary
    #[napi]
    pub async fn inventory_movement(&self, query: Option<AnalyticsQueryInput>) -> Result<Vec<InventoryMovementOutput>> {
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
    pub async fn demand_forecast(&self, skus: Option<Vec<String>>, days_ahead: Option<u32>) -> Result<Vec<DemandForecastOutput>> {
        let commerce = self.commerce.lock().await;

        let forecasts = commerce
            .analytics()
            .demand_forecast(skus, days_ahead.unwrap_or(30))
            .map_err(|e| Error::from_reason(format!("Failed to get demand forecast: {}", e)))?;

        Ok(forecasts.into_iter().map(|f| DemandForecastOutput {
            sku: f.sku,
            name: f.name,
            average_daily_demand: f.average_daily_demand.to_string().parse().unwrap_or(0.0),
            forecasted_demand: f.forecasted_demand.to_string().parse().unwrap_or(0.0),
            confidence: f.confidence.to_string().parse().unwrap_or(0.0),
            current_stock: f.current_stock.to_string().parse().unwrap_or(0.0),
            days_until_stockout: f.days_until_stockout,
            recommended_reorder_qty: f.recommended_reorder_qty.map(|d| d.to_string().parse().unwrap_or(0.0)),
            trend: format!("{:?}", f.trend),
        }).collect())
    }

    /// Get revenue forecast
    #[napi]
    pub async fn revenue_forecast(&self, periods_ahead: Option<u32>, granularity: Option<String>) -> Result<Vec<RevenueForecastOutput>> {
        let commerce = self.commerce.lock().await;

        let gran = granularity.map(|g| parse_granularity(&g)).unwrap_or(stateset_embedded::TimeGranularity::Month);

        let forecasts = commerce
            .analytics()
            .revenue_forecast(periods_ahead.unwrap_or(3), gran)
            .map_err(|e| Error::from_reason(format!("Failed to get revenue forecast: {}", e)))?;

        Ok(forecasts.into_iter().map(|f| RevenueForecastOutput {
            period: f.period,
            forecasted_revenue: f.forecasted_revenue.to_string().parse().unwrap_or(0.0),
            lower_bound: f.lower_bound.to_string().parse().unwrap_or(0.0),
            upper_bound: f.upper_bound.to_string().parse().unwrap_or(0.0),
            confidence_level: f.confidence_level.to_string().parse().unwrap_or(0.0),
            based_on_periods: f.based_on_periods,
        }).collect())
    }

    /// Get order status breakdown
    #[napi]
    pub async fn order_status_breakdown(&self, query: Option<AnalyticsQueryInput>) -> Result<OrderStatusBreakdownOutput> {
        let commerce = self.commerce.lock().await;

        let mut q = stateset_embedded::AnalyticsQuery::new();
        if let Some(ref input) = query {
            if let Some(ref period) = input.period {
                q = q.period(parse_period(period));
            }
        }

        let breakdown = commerce
            .analytics()
            .order_status_breakdown(q)
            .map_err(|e| Error::from_reason(format!("Failed to get order status breakdown: {}", e)))?;

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
    pub async fn fulfillment_metrics(&self, query: Option<AnalyticsQueryInput>) -> Result<FulfillmentMetricsOutput> {
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
            avg_time_to_ship_hours: metrics
                .avg_time_to_ship_hours
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            avg_time_to_deliver_hours: metrics
                .avg_time_to_deliver_hours
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            on_time_shipping_percent: metrics
                .on_time_shipping_percent
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            on_time_delivery_percent: metrics
                .on_time_delivery_percent
                .map(|d| d.to_string().parse().unwrap_or(0.0)),
            shipped_today: metrics.shipped_today as u32,
            awaiting_shipment: metrics.awaiting_shipment as u32,
        })
    }

    /// Get return metrics
    #[napi]
    pub async fn return_metrics(&self, query: Option<AnalyticsQueryInput>) -> Result<ReturnMetricsOutput> {
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
            return_rate_percent: metrics.return_rate_percent.to_string().parse().unwrap_or(0.0),
            total_refunded: metrics.total_refunded.to_string().parse().unwrap_or(0.0),
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
    }
}

fn exchange_rate_to_output(rate: stateset_embedded::ExchangeRate) -> ExchangeRateOutput {
    ExchangeRateOutput {
        id: rate.id.to_string(),
        base_currency: rate.base_currency.code().to_string(),
        quote_currency: rate.quote_currency.code().to_string(),
        rate: rate.rate.to_string().parse().unwrap_or(0.0),
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
    pub async fn list_rates(&self, filter: Option<ExchangeRateFilterInput>) -> Result<Vec<ExchangeRateOutput>> {
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
    pub async fn set_rates(&self, inputs: Vec<SetExchangeRateInput>) -> Result<Vec<ExchangeRateOutput>> {
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
            original_amount: result.original_amount.to_string().parse().unwrap_or(0.0),
            original_currency: result.original_currency.code().to_string(),
            converted_amount: result.converted_amount.to_string().parse().unwrap_or(0.0),
            target_currency: result.target_currency.code().to_string(),
            rate: result.rate.to_string().parse().unwrap_or(0.0),
            inverse_rate: result.inverse_rate.to_string().parse().unwrap_or(0.0),
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
            enabled_currencies: settings.enabled_currencies.iter().map(|c| c.code().to_string()).collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Update store currency settings
    #[napi]
    pub async fn update_settings(&self, input: StoreCurrencySettingsInput) -> Result<StoreCurrencySettingsOutput> {
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
                rounding_mode: input.rounding_mode.as_deref().map(parse_rounding_mode).unwrap_or_default(),
            })
            .map_err(|e| Error::from_reason(format!("Failed to update settings: {}", e)))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings.enabled_currencies.iter().map(|c| c.code().to_string()).collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Set the store's base currency
    #[napi]
    pub async fn set_base_currency(&self, currency_code: String) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.lock().await;
        let currency = parse_currency(&currency_code)?;

        let settings = commerce
            .currency()
            .set_base_currency(currency)
            .map_err(|e| Error::from_reason(format!("Failed to set base currency: {}", e)))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings.enabled_currencies.iter().map(|c| c.code().to_string()).collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Enable currencies for the store
    #[napi]
    pub async fn enable_currencies(&self, currency_codes: Vec<String>) -> Result<StoreCurrencySettingsOutput> {
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
            enabled_currencies: settings.enabled_currencies.iter().map(|c| c.code().to_string()).collect(),
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
#[derive(Serialize, Deserialize, Clone)]
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

impl From<stateset_core::SubscriptionPlan> for SubscriptionPlanOutput {
    fn from(p: stateset_core::SubscriptionPlan) -> Self {
        Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            status: format!("{:?}", p.status).to_lowercase(),
            billing_interval: format!("{}", p.billing_interval),
            custom_interval_days: p.custom_interval_days,
            price: p.price.to_string().parse().unwrap_or(0.0),
            setup_fee: p.setup_fee.map(|d| d.to_string().parse().unwrap_or(0.0)),
            currency: p.currency,
            trial_days: p.trial_days,
            trial_requires_payment_method: p.trial_requires_payment_method,
            min_cycles: p.min_cycles,
            max_cycles: p.max_cycles,
            discount_percent: p.discount_percent.map(|d| d.to_string().parse().unwrap_or(0.0)),
            discount_amount: p.discount_amount.map(|d| d.to_string().parse().unwrap_or(0.0)),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
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
#[derive(Serialize, Deserialize, Clone)]
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

impl From<stateset_core::Subscription> for SubscriptionOutput {
    fn from(s: stateset_core::Subscription) -> Self {
        Self {
            id: s.id.to_string(),
            subscription_number: s.subscription_number,
            customer_id: s.customer_id.to_string(),
            plan_id: s.plan_id.to_string(),
            plan_name: s.plan_name,
            status: format!("{}", s.status),
            billing_interval: format!("{}", s.billing_interval),
            custom_interval_days: s.custom_interval_days,
            price: s.price.to_string().parse().unwrap_or(0.0),
            currency: s.currency,
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
            discount_percent: s.discount_percent.map(|d| d.to_string().parse().unwrap_or(0.0)),
            discount_amount: s.discount_amount.map(|d| d.to_string().parse().unwrap_or(0.0)),
            coupon_code: s.coupon_code,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PauseSubscriptionInput {
    pub reason: Option<String>,
    pub resume_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CancelSubscriptionInput {
    pub reason: Option<String>,
    pub immediate: Option<bool>,
    pub feedback: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SkipBillingCycleInput {
    pub reason: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
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
            subtotal: b.subtotal.to_string().parse().unwrap_or(0.0),
            discount: b.discount.to_string().parse().unwrap_or(0.0),
            tax: b.tax.to_string().parse().unwrap_or(0.0),
            total: b.total.to_string().parse().unwrap_or(0.0),
            currency: b.currency,
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
    pub async fn create_plan(&self, input: CreateSubscriptionPlanInput) -> Result<SubscriptionPlanOutput> {
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
                price: Decimal::try_from(input.price)
                    .map_err(|e| Error::from_reason(format!("Invalid price: {}", e)))?,
                setup_fee: input.setup_fee.map(|f| Decimal::try_from(f).unwrap_or_default()),
                currency: input.currency,
                trial_days: input.trial_days,
                trial_requires_payment_method: input.trial_requires_payment_method,
                min_cycles: input.min_cycles,
                max_cycles: input.max_cycles,
                discount_percent: input.discount_percent.map(|d| Decimal::try_from(d).unwrap_or_default()),
                discount_amount: input.discount_amount.map(|d| Decimal::try_from(d).unwrap_or_default()),
                items: None,
                metadata: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create plan: {}", e)))?;

        Ok(plan.into())
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

        Ok(plan.map(|p| p.into()))
    }

    /// Get a subscription plan by code
    #[napi]
    pub async fn get_plan_by_code(&self, code: String) -> Result<Option<SubscriptionPlanOutput>> {
        let commerce = self.commerce.lock().await;

        let plan = commerce
            .subscriptions()
            .get_plan_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get plan: {}", e)))?;

        Ok(plan.map(|p| p.into()))
    }

    /// List subscription plans
    #[napi]
    pub async fn list_plans(&self, filter: Option<SubscriptionPlanFilterInput>) -> Result<Vec<SubscriptionPlanOutput>> {
        let commerce = self.commerce.lock().await;
        let f = filter.unwrap_or_default();

        let plans = commerce
            .subscriptions()
            .list_plans(stateset_core::SubscriptionPlanFilter {
                status: f.status.as_ref().and_then(|s| parse_plan_status(s).ok()),
                billing_interval: f.billing_interval.as_ref().and_then(|s| parse_billing_interval(s).ok()),
                search: f.search,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| Error::from_reason(format!("Failed to list plans: {}", e)))?;

        Ok(plans.into_iter().map(|p| p.into()).collect())
    }

    /// Update a subscription plan
    #[napi]
    pub async fn update_plan(&self, id: String, input: UpdateSubscriptionPlanInput) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let plan = commerce
            .subscriptions()
            .update_plan(uuid, stateset_core::UpdateSubscriptionPlan {
                name: input.name,
                description: input.description,
                status: None,
                price: input.price.map(|p| Decimal::try_from(p).unwrap_or_default()),
                setup_fee: input.setup_fee.map(|f| Decimal::try_from(f).unwrap_or_default()),
                trial_days: input.trial_days,
                trial_requires_payment_method: input.trial_requires_payment_method,
                min_cycles: input.min_cycles,
                max_cycles: input.max_cycles,
                discount_percent: input.discount_percent.map(|d| Decimal::try_from(d).unwrap_or_default()),
                discount_amount: input.discount_amount.map(|d| Decimal::try_from(d).unwrap_or_default()),
                metadata: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to update plan: {}", e)))?;

        Ok(plan.into())
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

        Ok(plan.into())
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

        Ok(plan.into())
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
                customer_id,
                plan_id,
                payment_method_id: input.payment_method_id,
                skip_trial: input.skip_trial,
                price: input.price.map(|p| Decimal::try_from(p).unwrap_or_default()),
                coupon_code: input.coupon_code,
                start_date,
                items: None,
                shipping_address: None,
                billing_address: None,
                metadata: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to create subscription: {}", e)))?;

        Ok(subscription.into())
    }

    /// Get a subscription by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SubscriptionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let subscription = commerce
            .subscriptions()
            .get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get subscription: {}", e)))?;

        Ok(subscription.map(|s| s.into()))
    }

    /// Get a subscription by number
    #[napi]
    pub async fn get_by_number(&self, number: String) -> Result<Option<SubscriptionOutput>> {
        let commerce = self.commerce.lock().await;

        let subscription = commerce
            .subscriptions()
            .get_by_number(&number)
            .map_err(|e| Error::from_reason(format!("Failed to get subscription: {}", e)))?;

        Ok(subscription.map(|s| s.into()))
    }

    /// List subscriptions
    #[napi]
    pub async fn list(&self, filter: Option<SubscriptionFilterInput>) -> Result<Vec<SubscriptionOutput>> {
        let commerce = self.commerce.lock().await;
        let f = filter.unwrap_or_default();

        let customer_id = f.customer_id.as_ref().and_then(|s| uuid::Uuid::parse_str(s).ok());
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

        Ok(subscriptions.into_iter().map(|s| s.into()).collect())
    }

    /// Update a subscription
    #[napi]
    pub async fn update(&self, id: String, input: UpdateSubscriptionInput) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let next_billing_date = input.next_billing_date.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let subscription = commerce
            .subscriptions()
            .update(uuid, stateset_core::UpdateSubscription {
                status: input.status.as_ref().and_then(|s| parse_subscription_status(s).ok()),
                price: input.price.map(|p| Decimal::try_from(p).unwrap_or_default()),
                payment_method_id: input.payment_method_id,
                next_billing_date,
                discount_percent: input.discount_percent.map(|d| Decimal::try_from(d).unwrap_or_default()),
                discount_amount: input.discount_amount.map(|d| Decimal::try_from(d).unwrap_or_default()),
                coupon_code: input.coupon_code,
                shipping_address: None,
                billing_address: None,
                metadata: None,
            })
            .map_err(|e| Error::from_reason(format!("Failed to update subscription: {}", e)))?;

        Ok(subscription.into())
    }

    /// Pause a subscription
    #[napi]
    pub async fn pause(&self, id: String, input: Option<PauseSubscriptionInput>) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let i = input.unwrap_or_default();
        let resume_at = i.resume_at.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
        });

        let subscription = commerce
            .subscriptions()
            .pause(uuid, stateset_core::PauseSubscription {
                reason: i.reason,
                resume_at,
            })
            .map_err(|e| Error::from_reason(format!("Failed to pause subscription: {}", e)))?;

        Ok(subscription.into())
    }

    /// Resume a paused subscription
    #[napi]
    pub async fn resume(&self, id: String) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let subscription = commerce
            .subscriptions()
            .resume(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to resume subscription: {}", e)))?;

        Ok(subscription.into())
    }

    /// Cancel a subscription
    #[napi]
    pub async fn cancel(&self, id: String, input: Option<CancelSubscriptionInput>) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let i = input.unwrap_or_default();

        let subscription = commerce
            .subscriptions()
            .cancel(uuid, stateset_core::CancelSubscription {
                reason: i.reason,
                immediate: i.immediate,
                feedback: i.feedback,
            })
            .map_err(|e| Error::from_reason(format!("Failed to cancel subscription: {}", e)))?;

        Ok(subscription.into())
    }

    /// Skip the next billing cycle
    #[napi]
    pub async fn skip_billing(&self, id: String, input: Option<SkipBillingCycleInput>) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let i = input.unwrap_or_default();

        let subscription = commerce
            .subscriptions()
            .skip_next_cycle(uuid, stateset_core::SkipBillingCycle {
                reason: i.reason,
            })
            .map_err(|e| Error::from_reason(format!("Failed to skip billing: {}", e)))?;

        Ok(subscription.into())
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// List billing cycles for a subscription
    #[napi]
    pub async fn list_billing_cycles(&self, filter: Option<BillingCycleFilterInput>) -> Result<Vec<BillingCycleOutput>> {
        let commerce = self.commerce.lock().await;
        let f = filter.unwrap_or_default();

        let subscription_id = f.subscription_id.as_ref().and_then(|s| uuid::Uuid::parse_str(s).ok());
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
    pub async fn get_events(&self, subscription_id: String, limit: Option<i32>) -> Result<Vec<SubscriptionEventOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&subscription_id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let events = commerce
            .subscriptions()
            .get_events(uuid, limit.map(|v| v as u32))
            .map_err(|e| Error::from_reason(format!("Failed to get events: {}", e)))?;

        Ok(events.into_iter().map(|e| e.into()).collect())
    }
}

impl Default for SubscriptionPlanFilterInput {
    fn default() -> Self {
        Self {
            status: None,
            billing_interval: None,
            search: None,
            limit: None,
            offset: None,
        }
    }
}

impl Default for SubscriptionFilterInput {
    fn default() -> Self {
        Self {
            customer_id: None,
            plan_id: None,
            status: None,
            from_date: None,
            to_date: None,
            search: None,
            limit: None,
            offset: None,
        }
    }
}

impl Default for PauseSubscriptionInput {
    fn default() -> Self {
        Self {
            reason: None,
            resume_at: None,
        }
    }
}

impl Default for CancelSubscriptionInput {
    fn default() -> Self {
        Self {
            reason: None,
            immediate: None,
            feedback: None,
        }
    }
}

impl Default for SkipBillingCycleInput {
    fn default() -> Self {
        Self {
            reason: None,
        }
    }
}

impl Default for BillingCycleFilterInput {
    fn default() -> Self {
        Self {
            subscription_id: None,
            status: None,
            from_date: None,
            to_date: None,
            limit: None,
            offset: None,
        }
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

impl From<stateset_core::Promotion> for PromotionOutput {
    fn from(p: stateset_core::Promotion) -> Self {
        Self {
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
            percentage_off: p.percentage_off.map(|d| d.to_string().parse().unwrap_or(0.0)),
            fixed_amount_off: p.fixed_amount_off.map(|d| d.to_string().parse().unwrap_or(0.0)),
            max_discount_amount: p.max_discount_amount.map(|d| d.to_string().parse().unwrap_or(0.0)),
            buy_quantity: p.buy_quantity,
            get_quantity: p.get_quantity,
            get_discount_percent: p.get_discount_percent.map(|d| d.to_string().parse().unwrap_or(0.0)),
            starts_at: p.starts_at.to_rfc3339(),
            ends_at: p.ends_at.map(|d| d.to_rfc3339()),
            total_usage_limit: p.total_usage_limit,
            per_customer_limit: p.per_customer_limit,
            usage_count: p.usage_count,
            currency: p.currency,
            priority: p.priority,
            metadata: p.metadata.map(|m| m.to_string()),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
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

impl From<stateset_core::ApplyPromotionsResult> for ApplyPromotionsOutput {
    fn from(r: stateset_core::ApplyPromotionsResult) -> Self {
        Self {
            original_subtotal: r.original_subtotal.to_string().parse().unwrap_or(0.0),
            total_discount: r.total_discount.to_string().parse().unwrap_or(0.0),
            discounted_subtotal: r.discounted_subtotal.to_string().parse().unwrap_or(0.0),
            original_shipping: r.original_shipping.to_string().parse().unwrap_or(0.0),
            shipping_discount: r.shipping_discount.to_string().parse().unwrap_or(0.0),
            final_shipping: r.final_shipping.to_string().parse().unwrap_or(0.0),
            grand_total: r.grand_total.to_string().parse().unwrap_or(0.0),
            applied_promotions: r.applied_promotions.into_iter().map(|a| AppliedPromotionOutput {
                promotion_id: a.promotion_id.to_string(),
                promotion_name: a.promotion_name,
                coupon_code: a.coupon_code,
                discount_amount: a.discount_amount.to_string().parse().unwrap_or(0.0),
                discount_type: format!("{:?}", a.discount_type).to_lowercase(),
            }).collect(),
        }
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

impl From<stateset_core::PromotionUsage> for PromotionUsageOutput {
    fn from(u: stateset_core::PromotionUsage) -> Self {
        Self {
            id: u.id.to_string(),
            promotion_id: u.promotion_id.to_string(),
            coupon_id: u.coupon_id.map(|id| id.to_string()),
            customer_id: u.customer_id.map(|id| id.to_string()),
            order_id: u.order_id.map(|id| id.to_string()),
            cart_id: u.cart_id.map(|id| id.to_string()),
            discount_amount: u.discount_amount.to_string().parse().unwrap_or(0.0),
            currency: u.currency,
            used_at: u.used_at.to_rfc3339(),
        }
    }
}

fn parse_promotion_type(s: &str) -> stateset_core::PromotionType {
    match s.to_lowercase().as_str() {
        "percentage_off" | "percentageoff" => stateset_core::PromotionType::PercentageOff,
        "fixed_amount_off" | "fixedamountoff" => stateset_core::PromotionType::FixedAmountOff,
        "buy_x_get_y" | "buyxgety" | "bogo" => stateset_core::PromotionType::BuyXGetY,
        "free_shipping" | "freeshipping" => stateset_core::PromotionType::FreeShipping,
        "tiered_discount" | "tiereddiscount" => stateset_core::PromotionType::TieredDiscount,
        "bundle" | "bundle_discount" | "bundlediscount" => stateset_core::PromotionType::BundleDiscount,
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
            promotion_type: input.promotion_type.map(|s| parse_promotion_type(&s)).unwrap_or_default(),
            trigger: input.trigger.map(|s| parse_promotion_trigger(&s)).unwrap_or_default(),
            target: input.target.map(|s| parse_promotion_target(&s)).unwrap_or_default(),
            stacking: input.stacking.map(|s| parse_stacking_behavior(&s)).unwrap_or_default(),
            percentage_off: input.percentage_off.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            fixed_amount_off: input.fixed_amount_off.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            max_discount_amount: input.max_discount_amount.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            buy_quantity: input.buy_quantity,
            get_quantity: input.get_quantity,
            get_discount_percent: input.get_discount_percent.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            tiers: input.tiers.and_then(|s| serde_json::from_str(&s).ok()),
            bundle_product_ids: input.bundle_product_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            bundle_discount: input.bundle_discount.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            starts_at: input.starts_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            ends_at: input.ends_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            conditions: None,
            applicable_product_ids: input.applicable_product_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            applicable_category_ids: input.applicable_category_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            applicable_skus: input.applicable_skus,
            excluded_product_ids: input.excluded_product_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            excluded_category_ids: input.excluded_category_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            eligible_customer_ids: input.eligible_customer_ids.map(|ids| {
                ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect()
            }),
            eligible_customer_groups: input.eligible_customer_groups,
            currency: input.currency,
            priority: input.priority,
            metadata: input.metadata.and_then(|s| serde_json::from_str(&s).ok()),
        };

        let promo = commerce.promotions().create(create)
            .map_err(|e| Error::from_reason(format!("Failed to create promotion: {}", e)))?;

        Ok(promo.into())
    }

    /// Get a promotion by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let promo = commerce.promotions().get(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get promotion: {}", e)))?;

        Ok(promo.map(|p| p.into()))
    }

    /// Get a promotion by its internal code
    #[napi]
    pub async fn get_by_code(&self, code: String) -> Result<Option<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let promo = commerce.promotions().get_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get promotion: {}", e)))?;

        Ok(promo.map(|p| p.into()))
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

        let promos = commerce.promotions().list(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list promotions: {}", e)))?;

        Ok(promos.into_iter().map(|p| p.into()).collect())
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
            percentage_off: input.percentage_off.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            fixed_amount_off: input.fixed_amount_off.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            max_discount_amount: input.max_discount_amount.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            starts_at: input.starts_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            ends_at: input.ends_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            total_usage_limit: input.total_usage_limit,
            per_customer_limit: input.per_customer_limit,
            priority: input.priority,
            metadata: None,
        };

        let promo = commerce.promotions().update(uuid, update)
            .map_err(|e| Error::from_reason(format!("Failed to update promotion: {}", e)))?;

        Ok(promo.into())
    }

    /// Delete a promotion
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        commerce.promotions().delete(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to delete promotion: {}", e)))?;

        Ok(())
    }

    /// Activate a promotion
    #[napi]
    pub async fn activate(&self, id: String) -> Result<PromotionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let promo = commerce.promotions().activate(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to activate promotion: {}", e)))?;

        Ok(promo.into())
    }

    /// Deactivate (pause) a promotion
    #[napi]
    pub async fn deactivate(&self, id: String) -> Result<PromotionOutput> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let promo = commerce.promotions().deactivate(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to deactivate promotion: {}", e)))?;

        Ok(promo.into())
    }

    /// Get all currently active promotions
    #[napi]
    pub async fn get_active(&self) -> Result<Vec<PromotionOutput>> {
        let commerce = self.commerce.lock().await;
        let promos = commerce.promotions().get_active()
            .map_err(|e| Error::from_reason(format!("Failed to get active promotions: {}", e)))?;

        Ok(promos.into_iter().map(|p| p.into()).collect())
    }

    /// Check if a promotion is currently valid
    #[napi]
    pub async fn is_valid(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let valid = commerce.promotions().is_valid(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to check promotion validity: {}", e)))?;

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
            promotion_id,
            code: input.code,
            usage_limit: input.usage_limit,
            per_customer_limit: input.per_customer_limit,
            starts_at: input.starts_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            ends_at: input.ends_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            metadata: input.metadata.and_then(|s| serde_json::from_str(&s).ok()),
        };

        let coupon = commerce.promotions().create_coupon(create)
            .map_err(|e| Error::from_reason(format!("Failed to create coupon: {}", e)))?;

        Ok(coupon.into())
    }

    /// Get a coupon by ID
    #[napi]
    pub async fn get_coupon(&self, id: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let coupon = commerce.promotions().get_coupon(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// Get a coupon by its code
    #[napi]
    pub async fn get_coupon_by_code(&self, code: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let coupon = commerce.promotions().get_coupon_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// List coupons with optional filtering
    #[napi]
    pub async fn list_coupons(&self, filter: Option<CouponFilterInput>) -> Result<Vec<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let filter = filter.unwrap_or_default();

        let core_filter = stateset_core::CouponFilter {
            promotion_id: filter.promotion_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            status: filter.status.map(|s| parse_coupon_status(&s)),
            search: filter.search,
            limit: filter.limit.map(|v| v as u32),
            offset: filter.offset.map(|v| v as u32),
        };

        let coupons = commerce.promotions().list_coupons(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list coupons: {}", e)))?;

        Ok(coupons.into_iter().map(|c| c.into()).collect())
    }

    /// Validate a coupon code
    #[napi]
    pub async fn validate_coupon(&self, code: String) -> Result<Option<CouponOutput>> {
        let commerce = self.commerce.lock().await;
        let coupon = commerce.promotions().validate_coupon(&code)
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
            cart_id: input.cart_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            customer_id: input.customer_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            coupon_codes: input.coupon_codes.unwrap_or_default(),
            line_items: input.line_items.into_iter().map(|item| {
                stateset_core::PromotionLineItem {
                    id: item.id,
                    product_id: item.product_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
                    variant_id: item.variant_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
                    sku: item.sku,
                    category_ids: item.category_ids
                        .map(|ids| ids.into_iter().filter_map(|s| uuid::Uuid::parse_str(&s).ok()).collect())
                        .unwrap_or_default(),
                    quantity: item.quantity,
                    unit_price: Decimal::from_f64_retain(item.unit_price).unwrap_or_default(),
                    line_total: Decimal::from_f64_retain(item.line_total).unwrap_or_default(),
                }
            }).collect(),
            subtotal: Decimal::from_f64_retain(input.subtotal).unwrap_or_default(),
            shipping_amount: Decimal::from_f64_retain(input.shipping_amount.unwrap_or(0.0)).unwrap_or_default(),
            shipping_country: input.shipping_country,
            shipping_state: input.shipping_state,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            is_first_order: false,
        };

        let result = commerce.promotions().apply(request)
            .map_err(|e| Error::from_reason(format!("Failed to apply promotions: {}", e)))?;

        Ok(result.into())
    }

    /// Record promotion usage (after order completion)
    #[napi]
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

        let usage = commerce.promotions().record_usage(
            promotion_uuid,
            coupon_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            customer_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            order_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            cart_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            Decimal::from_f64_retain(discount_amount).unwrap_or_default(),
            &currency,
        ).map_err(|e| Error::from_reason(format!("Failed to record usage: {}", e)))?;

        Ok(usage.into())
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

impl From<stateset_core::TaxRate> for TaxRateOutput {
    fn from(r: stateset_core::TaxRate) -> Self {
        Self {
            id: r.id.to_string(),
            jurisdiction_id: r.jurisdiction_id.to_string(),
            tax_type: r.tax_type.as_str().to_string(),
            product_category: r.product_category.as_str().to_string(),
            rate: r.rate.to_string().parse().unwrap_or(0.0),
            name: r.name,
            description: r.description,
            is_compound: r.is_compound,
            priority: r.priority,
            threshold_min: r.threshold_min.map(|d| d.to_string().parse().unwrap_or(0.0)),
            threshold_max: r.threshold_max.map(|d| d.to_string().parse().unwrap_or(0.0)),
            fixed_amount: r.fixed_amount.map(|d| d.to_string().parse().unwrap_or(0.0)),
            effective_from: r.effective_from.to_string(),
            effective_to: r.effective_to.map(|d| d.to_string()),
            active: r.active,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
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

impl From<stateset_core::TaxBreakdown> for TaxBreakdownOutput {
    fn from(b: stateset_core::TaxBreakdown) -> Self {
        Self {
            jurisdiction_id: b.jurisdiction_id.to_string(),
            jurisdiction_name: b.jurisdiction_name,
            tax_type: b.tax_type.as_str().to_string(),
            rate_name: b.rate_name,
            rate: b.rate.to_string().parse().unwrap_or(0.0),
            taxable_amount: b.taxable_amount.to_string().parse().unwrap_or(0.0),
            tax_amount: b.tax_amount.to_string().parse().unwrap_or(0.0),
            is_compound: b.is_compound,
        }
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

impl From<stateset_core::TaxDetail> for TaxDetailOutput {
    fn from(d: stateset_core::TaxDetail) -> Self {
        Self {
            tax_type: d.tax_type.as_str().to_string(),
            jurisdiction_name: d.jurisdiction_name,
            rate: d.rate.to_string().parse().unwrap_or(0.0),
            amount: d.amount.to_string().parse().unwrap_or(0.0),
        }
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

impl From<stateset_core::LineItemTax> for LineItemTaxOutput {
    fn from(t: stateset_core::LineItemTax) -> Self {
        Self {
            line_item_id: t.line_item_id,
            taxable_amount: t.taxable_amount.to_string().parse().unwrap_or(0.0),
            tax_amount: t.tax_amount.to_string().parse().unwrap_or(0.0),
            effective_rate: t.effective_rate.to_string().parse().unwrap_or(0.0),
            is_exempt: t.is_exempt,
            exemption_reason: t.exemption_reason,
            tax_details: t.tax_details.into_iter().map(|d| d.into()).collect(),
        }
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

impl From<stateset_core::ExemptionDetails> for ExemptionDetailsOutput {
    fn from(e: stateset_core::ExemptionDetails) -> Self {
        Self {
            exemption_id: e.exemption_id.to_string(),
            exemption_type: format!("{:?}", e.exemption_type).to_lowercase(),
            certificate_number: e.certificate_number,
            amount_exempt: e.amount_exempt.to_string().parse().unwrap_or(0.0),
            tax_saved: e.tax_saved.to_string().parse().unwrap_or(0.0),
        }
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

impl From<stateset_core::JurisdictionSummary> for JurisdictionSummaryOutput {
    fn from(s: stateset_core::JurisdictionSummary) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            code: s.code,
            level: format!("{:?}", s.level).to_lowercase(),
            total_rate: s.total_rate.to_string().parse().unwrap_or(0.0),
            total_tax: s.total_tax.to_string().parse().unwrap_or(0.0),
        }
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

impl From<stateset_core::TaxCalculationResult> for TaxCalculationOutput {
    fn from(r: stateset_core::TaxCalculationResult) -> Self {
        Self {
            id: r.id.to_string(),
            total_tax: r.total_tax.to_string().parse().unwrap_or(0.0),
            subtotal: r.subtotal.to_string().parse().unwrap_or(0.0),
            total: r.total.to_string().parse().unwrap_or(0.0),
            shipping_tax: r.shipping_tax.to_string().parse().unwrap_or(0.0),
            tax_breakdown: r.tax_breakdown.into_iter().map(|b| b.into()).collect(),
            line_item_taxes: r.line_item_taxes.into_iter().map(|t| t.into()).collect(),
            exemptions_applied: r.exemptions_applied,
            exemption_details: r.exemption_details.map(|e| e.into()),
            jurisdictions: r.jurisdictions.into_iter().map(|s| s.into()).collect(),
            calculated_at: r.calculated_at.to_rfc3339(),
            is_estimate: r.is_estimate,
        }
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

impl From<stateset_core::UsStateTaxInfo> for UsStateTaxInfoOutput {
    fn from(i: stateset_core::UsStateTaxInfo) -> Self {
        Self {
            state_code: i.state_code,
            state_name: i.state_name,
            state_rate: i.state_rate.to_string().parse().unwrap_or(0.0),
            has_local_taxes: i.has_local_taxes,
            origin_based: i.origin_based,
            tax_shipping: i.tax_shipping,
            tax_clothing: i.tax_clothing,
            tax_food: i.tax_food,
            tax_digital: i.tax_digital,
        }
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

impl From<stateset_core::EuVatInfo> for EuVatInfoOutput {
    fn from(i: stateset_core::EuVatInfo) -> Self {
        Self {
            country_code: i.country_code,
            country_name: i.country_name,
            standard_rate: i.standard_rate.to_string().parse().unwrap_or(0.0),
            reduced_rate: i.reduced_rate.map(|d| d.to_string().parse().unwrap_or(0.0)),
            super_reduced_rate: i.super_reduced_rate.map(|d| d.to_string().parse().unwrap_or(0.0)),
            parking_rate: i.parking_rate.map(|d| d.to_string().parse().unwrap_or(0.0)),
        }
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

impl From<stateset_core::CanadianTaxInfo> for CanadianTaxInfoOutput {
    fn from(i: stateset_core::CanadianTaxInfo) -> Self {
        Self {
            province_code: i.province_code,
            province_name: i.province_name,
            gst_rate: i.gst_rate.to_string().parse().unwrap_or(0.0),
            pst_rate: i.pst_rate.map(|d| d.to_string().parse().unwrap_or(0.0)),
            hst_rate: i.hst_rate.map(|d| d.to_string().parse().unwrap_or(0.0)),
            qst_rate: i.qst_rate.map(|d| d.to_string().parse().unwrap_or(0.0)),
            total_rate: i.total_rate.to_string().parse().unwrap_or(0.0),
        }
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

        let line_items: Vec<stateset_core::TaxLineItem> = input.line_items.into_iter().map(|item| {
            stateset_core::TaxLineItem {
                id: item.id,
                sku: item.sku,
                product_id: item.product_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
                quantity: Decimal::from_f64_retain(item.quantity).unwrap_or(Decimal::ONE),
                unit_price: Decimal::from_f64_retain(item.unit_price).unwrap_or_default(),
                discount_amount: Decimal::from_f64_retain(item.discount_amount.unwrap_or(0.0)).unwrap_or_default(),
                tax_category: item.tax_category.map(|s| parse_product_tax_category(&s)).unwrap_or_default(),
                tax_code: item.tax_code,
                description: item.description,
            }
        }).collect();

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
            shipping_amount: input.shipping_amount.map(|a| Decimal::from_f64_retain(a).unwrap_or_default()),
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            transaction_date: input.transaction_date.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            prices_include_tax: input.prices_include_tax.unwrap_or(false),
        };

        let result = commerce.tax().calculate(request)
            .map_err(|e| Error::from_reason(format!("Failed to calculate tax: {}", e)))?;

        Ok(result.into())
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

        let tax = commerce.tax().calculate_for_item(
            Decimal::from_f64_retain(unit_price).unwrap_or_default(),
            Decimal::from_f64_retain(quantity).unwrap_or(Decimal::ONE),
            category.map(|s| parse_product_tax_category(&s)).unwrap_or_default(),
            &address,
        ).map_err(|e| Error::from_reason(format!("Failed to calculate tax: {}", e)))?;

        Ok(tax.to_string().parse().unwrap_or(0.0))
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

        let rate = commerce.tax().get_effective_rate(
            &tax_address,
            category.map(|s| parse_product_tax_category(&s)).unwrap_or_default(),
        ).map_err(|e| Error::from_reason(format!("Failed to get rate: {}", e)))?;

        Ok(rate.to_string().parse().unwrap_or(0.0))
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

        let jurisdiction = commerce.tax().get_jurisdiction(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get jurisdiction: {}", e)))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// Get a jurisdiction by code
    #[napi]
    pub async fn get_jurisdiction_by_code(&self, code: String) -> Result<Option<TaxJurisdictionOutput>> {
        let commerce = self.commerce.lock().await;

        let jurisdiction = commerce.tax().get_jurisdiction_by_code(&code)
            .map_err(|e| Error::from_reason(format!("Failed to get jurisdiction: {}", e)))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// List jurisdictions with optional filtering
    #[napi]
    pub async fn list_jurisdictions(&self, filter: Option<JurisdictionFilterInput>) -> Result<Vec<TaxJurisdictionOutput>> {
        let commerce = self.commerce.lock().await;

        let f = filter.unwrap_or_default();
        let core_filter = stateset_core::TaxJurisdictionFilter {
            country_code: f.country_code,
            state_code: f.state_code,
            level: f.level.map(|s| parse_jurisdiction_level(&s)),
            active_only: f.active_only.unwrap_or(false),
        };

        let jurisdictions = commerce.tax().list_jurisdictions(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list jurisdictions: {}", e)))?;

        Ok(jurisdictions.into_iter().map(|j| j.into()).collect())
    }

    /// Create a new jurisdiction
    #[napi]
    pub async fn create_jurisdiction(&self, input: CreateJurisdictionInput) -> Result<TaxJurisdictionOutput> {
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

        let jurisdiction = commerce.tax().create_jurisdiction(create)
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

        let rate = commerce.tax().get_rate(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get rate: {}", e)))?;

        Ok(rate.map(|r| r.into()))
    }

    /// List tax rates with optional filtering
    #[napi]
    pub async fn list_rates(&self, filter: Option<TaxRateFilterInput>) -> Result<Vec<TaxRateOutput>> {
        let commerce = self.commerce.lock().await;

        let f = filter.unwrap_or_default();
        let core_filter = stateset_core::TaxRateFilter {
            jurisdiction_id: f.jurisdiction_id.and_then(|s| uuid::Uuid::parse_str(&s).ok()),
            tax_type: f.tax_type.map(|s| parse_tax_type(&s)),
            product_category: f.product_category.map(|s| parse_product_tax_category(&s)),
            active_only: f.active_only.unwrap_or(false),
            effective_date: f.effective_date.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        };

        let rates = commerce.tax().list_rates(core_filter)
            .map_err(|e| Error::from_reason(format!("Failed to list rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    /// Create a new tax rate
    #[napi]
    pub async fn create_rate(&self, input: CreateTaxRateInput) -> Result<TaxRateOutput> {
        let commerce = self.commerce.lock().await;

        let jurisdiction_id = uuid::Uuid::parse_str(&input.jurisdiction_id)
            .map_err(|e| Error::from_reason(format!("Invalid jurisdiction UUID: {}", e)))?;

        let effective_from = chrono::NaiveDate::parse_from_str(&input.effective_from, "%Y-%m-%d")
            .map_err(|e| Error::from_reason(format!("Invalid date format: {}", e)))?;

        let create = stateset_core::CreateTaxRate {
            jurisdiction_id,
            tax_type: input.tax_type.map(|s| parse_tax_type(&s)).unwrap_or_default(),
            product_category: input.product_category.map(|s| parse_product_tax_category(&s)).unwrap_or_default(),
            rate: Decimal::from_f64_retain(input.rate).unwrap_or_default(),
            name: input.name,
            description: input.description,
            is_compound: input.is_compound.unwrap_or(false),
            priority: input.priority.unwrap_or(0),
            threshold_min: input.threshold_min.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            threshold_max: input.threshold_max.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            fixed_amount: input.fixed_amount.map(|v| Decimal::from_f64_retain(v).unwrap_or_default()),
            effective_from,
            effective_to: input.effective_to.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        };

        let rate = commerce.tax().create_rate(create)
            .map_err(|e| Error::from_reason(format!("Failed to create rate: {}", e)))?;

        Ok(rate.into())
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

        let exemption = commerce.tax().get_exemption(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get exemption: {}", e)))?;

        Ok(exemption.map(|e| e.into()))
    }

    /// Get exemptions for a customer
    #[napi]
    pub async fn get_customer_exemptions(&self, customer_id: String) -> Result<Vec<TaxExemptionOutput>> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let exemptions = commerce.tax().get_customer_exemptions(uuid)
            .map_err(|e| Error::from_reason(format!("Failed to get exemptions: {}", e)))?;

        Ok(exemptions.into_iter().map(|e| e.into()).collect())
    }

    /// Create a tax exemption
    #[napi]
    pub async fn create_exemption(&self, input: CreateExemptionInput) -> Result<TaxExemptionOutput> {
        let commerce = self.commerce.lock().await;

        let customer_id = uuid::Uuid::parse_str(&input.customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid customer UUID: {}", e)))?;

        let effective_from = chrono::NaiveDate::parse_from_str(&input.effective_from, "%Y-%m-%d")
            .map_err(|e| Error::from_reason(format!("Invalid date format: {}", e)))?;

        let create = stateset_core::CreateTaxExemption {
            customer_id,
            exemption_type: parse_exemption_type(&input.exemption_type),
            certificate_number: input.certificate_number,
            issuing_authority: input.issuing_authority,
            jurisdiction_ids: input.jurisdiction_ids
                .unwrap_or_default()
                .into_iter()
                .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
                .collect(),
            exempt_categories: input.exempt_categories
                .unwrap_or_default()
                .into_iter()
                .map(|s| parse_product_tax_category(&s))
                .collect(),
            effective_from,
            expires_at: input.expires_at.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            notes: input.notes,
        };

        let exemption = commerce.tax().create_exemption(create)
            .map_err(|e| Error::from_reason(format!("Failed to create exemption: {}", e)))?;

        Ok(exemption.into())
    }

    /// Check if a customer is tax exempt
    #[napi]
    pub async fn customer_is_exempt(&self, customer_id: String) -> Result<bool> {
        let commerce = self.commerce.lock().await;
        let uuid = uuid::Uuid::parse_str(&customer_id)
            .map_err(|e| Error::from_reason(format!("Invalid UUID: {}", e)))?;

        let is_exempt = commerce.tax().customer_is_exempt(uuid)
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

        let settings = commerce.tax().get_settings()
            .map_err(|e| Error::from_reason(format!("Failed to get settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Update tax settings
    #[napi]
    pub async fn update_settings(&self, input: TaxSettingsInput) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.lock().await;

        let mut settings = commerce.tax().get_settings()
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

        let updated = commerce.tax().update_settings(settings)
            .map_err(|e| Error::from_reason(format!("Failed to update settings: {}", e)))?;

        Ok(updated.into())
    }

    /// Enable or disable tax calculation
    #[napi]
    pub async fn set_enabled(&self, enabled: bool) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.lock().await;

        let settings = commerce.tax().set_enabled(enabled)
            .map_err(|e| Error::from_reason(format!("Failed to update settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Check if tax calculation is enabled
    #[napi]
    pub async fn is_enabled(&self) -> Result<bool> {
        let commerce = self.commerce.lock().await;

        let enabled = commerce.tax().is_enabled()
            .map_err(|e| Error::from_reason(format!("Failed to check settings: {}", e)))?;

        Ok(enabled)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get US state tax information
    #[napi]
    pub fn get_us_state_info(state_code: String) -> Option<UsStateTaxInfoOutput> {
        stateset_core::get_us_state_tax_info(&state_code).map(|i| i.into())
    }

    /// Get EU VAT information
    #[napi]
    pub fn get_eu_vat_info(country_code: String) -> Option<EuVatInfoOutput> {
        stateset_core::get_eu_vat_info(&country_code).map(|i| i.into())
    }

    /// Get Canadian tax information
    #[napi]
    pub fn get_canadian_tax_info(province_code: String) -> Option<CanadianTaxInfoOutput> {
        stateset_core::get_canadian_tax_info(&province_code).map(|i| i.into())
    }

    /// Check if a country is in the EU
    #[napi]
    pub fn is_eu_country(country_code: String) -> bool {
        stateset_core::is_eu_member(&country_code)
    }
}

