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
