//! Ruby bindings for StateSet Embedded Commerce
//!
//! Provides a local-first commerce library with SQLite storage for Ruby.
//!
//! ```ruby
//! require 'stateset_embedded'
//!
//! commerce = StateSet::Commerce.new("./store.db")
//! customer = commerce.customers.create(
//!   email: "alice@example.com",
//!   first_name: "Alice",
//!   last_name: "Smith"
//! )
//! ```

use magnus::{
    class, define_module, exception, function, method, prelude::*, DataTypeFunctions, Error, RHash,
    Ruby, Symbol, TypedData, Value,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use stateset_embedded::Commerce as RustCommerce;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Macros
// ============================================================================

macro_rules! lock_commerce {
    ($commerce:expr) => {
        $commerce
            .lock()
            .map_err(|e| Error::new(exception::runtime_error(), format!("Lock error: {}", e)))?
    };
}

macro_rules! parse_uuid {
    ($id:expr, $name:expr) => {
        $id.parse()
            .map_err(|_| Error::new(exception::arg_error(), format!("Invalid {} UUID", $name)))?
    };
}

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

// ============================================================================
// Commerce
// ============================================================================

/// Main Commerce instance for local commerce operations.
#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Commerce", free_immediately, size)]
pub struct Commerce {
    inner: Arc<Mutex<RustCommerce>>,
}

impl Commerce {
    fn new(db_path: String) -> Result<Self, Error> {
        let commerce = RustCommerce::new(&db_path).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to initialize commerce: {}", e),
            )
        })?;

        Ok(Self {
            inner: Arc::new(Mutex::new(commerce)),
        })
    }

    fn customers(&self) -> Customers {
        Customers {
            commerce: self.inner.clone(),
        }
    }

    fn orders(&self) -> Orders {
        Orders {
            commerce: self.inner.clone(),
        }
    }

    fn products(&self) -> Products {
        Products {
            commerce: self.inner.clone(),
        }
    }

    fn inventory(&self) -> Inventory {
        Inventory {
            commerce: self.inner.clone(),
        }
    }

    fn returns(&self) -> Returns {
        Returns {
            commerce: self.inner.clone(),
        }
    }

    fn payments(&self) -> Payments {
        Payments {
            commerce: self.inner.clone(),
        }
    }

    fn shipments(&self) -> Shipments {
        Shipments {
            commerce: self.inner.clone(),
        }
    }

    fn warranties(&self) -> Warranties {
        Warranties {
            commerce: self.inner.clone(),
        }
    }

    fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders {
            commerce: self.inner.clone(),
        }
    }

    fn invoices(&self) -> Invoices {
        Invoices {
            commerce: self.inner.clone(),
        }
    }

    fn bom(&self) -> BomApi {
        BomApi {
            commerce: self.inner.clone(),
        }
    }

    fn work_orders(&self) -> WorkOrders {
        WorkOrders {
            commerce: self.inner.clone(),
        }
    }

    fn carts(&self) -> Carts {
        Carts {
            commerce: self.inner.clone(),
        }
    }

    fn analytics(&self) -> Analytics {
        Analytics {
            commerce: self.inner.clone(),
        }
    }

    fn currency(&self) -> CurrencyOps {
        CurrencyOps {
            commerce: self.inner.clone(),
        }
    }

    fn subscriptions(&self) -> Subscriptions {
        Subscriptions {
            commerce: self.inner.clone(),
        }
    }

    fn promotions(&self) -> Promotions {
        Promotions {
            commerce: self.inner.clone(),
        }
    }

    fn tax(&self) -> Tax {
        Tax {
            commerce: self.inner.clone(),
        }
    }
}

// ============================================================================
// Customer Types
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Customer", free_immediately, size)]
pub struct Customer {
    id: String,
    email: String,
    first_name: String,
    last_name: String,
    phone: Option<String>,
    status: String,
    accepts_marketing: bool,
    created_at: String,
    updated_at: String,
}

impl Customer {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn email(&self) -> String {
        self.email.clone()
    }

    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    fn last_name(&self) -> String {
        self.last_name.clone()
    }

    fn phone(&self) -> Option<String> {
        self.phone.clone()
    }

    fn status(&self) -> String {
        self.status.clone()
    }

    fn accepts_marketing(&self) -> bool {
        self.accepts_marketing
    }

    fn created_at(&self) -> String {
        self.created_at.clone()
    }

    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }

    fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Customer id=\"{}\" email=\"{}\" name=\"{} {}\">",
            self.id, self.email, self.first_name, self.last_name
        )
    }
}

impl From<stateset_core::Customer> for Customer {
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

// ============================================================================
// Customers API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Customers", free_immediately, size)]
pub struct Customers {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Customers {
    fn create(
        &self,
        email: String,
        first_name: String,
        last_name: String,
        phone: Option<String>,
        accepts_marketing: Option<bool>,
    ) -> Result<Customer, Error> {
        let commerce = lock_commerce!(self.commerce);

        let customer = commerce
            .customers()
            .create(stateset_core::CreateCustomer {
                email,
                first_name,
                last_name,
                phone,
                accepts_marketing,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create customer: {}", e),
                )
            })?;

        Ok(customer.into())
    }

    fn get(&self, id: String) -> Result<Option<Customer>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "customer");

        let customer = commerce.customers().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get customer: {}", e),
            )
        })?;

        Ok(customer.map(|c| c.into()))
    }

    fn get_by_email(&self, email: String) -> Result<Option<Customer>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let customer = commerce.customers().get_by_email(&email).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get customer: {}", e),
            )
        })?;

        Ok(customer.map(|c| c.into()))
    }

    fn list(&self) -> Result<Vec<Customer>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let customers = commerce.customers().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list customers: {}", e),
            )
        })?;

        Ok(customers.into_iter().map(|c| c.into()).collect())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .customers()
            .count(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to count customers: {}", e),
                )
            })?;

        Ok(count)
    }
}

// ============================================================================
// Order Types
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::OrderItem", free_immediately, size)]
pub struct OrderItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

impl OrderItem {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn sku(&self) -> String {
        self.sku.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn quantity(&self) -> i32 {
        self.quantity
    }
    fn unit_price(&self) -> f64 {
        self.unit_price
    }
    fn total(&self) -> f64 {
        self.total
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::OrderItem sku=\"{}\" qty={} price={}>",
            self.sku, self.quantity, self.unit_price
        )
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Order", free_immediately, size)]
pub struct Order {
    id: String,
    order_number: String,
    customer_id: String,
    status: String,
    total_amount: f64,
    currency: String,
    payment_status: String,
    fulfillment_status: String,
    tracking_number: Option<String>,
    items: Vec<OrderItem>,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl Order {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn order_number(&self) -> String {
        self.order_number.clone()
    }
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn total_amount(&self) -> f64 {
        self.total_amount
    }
    fn currency(&self) -> String {
        self.currency.clone()
    }
    fn payment_status(&self) -> String {
        self.payment_status.clone()
    }
    fn fulfillment_status(&self) -> String {
        self.fulfillment_status.clone()
    }
    fn tracking_number(&self) -> Option<String> {
        self.tracking_number.clone()
    }
    fn items(&self) -> Vec<OrderItem> {
        self.items.clone()
    }
    fn version(&self) -> i32 {
        self.version
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn item_count(&self) -> usize {
        self.items.len()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Order number=\"{}\" status=\"{}\" total={} {}>",
            self.order_number, self.status, self.total_amount, self.currency
        )
    }
}

impl From<stateset_core::Order> for Order {
    fn from(o: stateset_core::Order) -> Self {
        Self {
            id: o.id.to_string(),
            order_number: o.order_number,
            customer_id: o.customer_id.to_string(),
            status: format!("{}", o.status),
            total_amount: to_f64_or_nan(o.total_amount),
            currency: o.currency,
            payment_status: format!("{}", o.payment_status),
            fulfillment_status: format!("{}", o.fulfillment_status),
            tracking_number: o.tracking_number,
            items: o
                .items
                .into_iter()
                .map(|i| OrderItem {
                    id: i.id.to_string(),
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: to_f64_or_nan(i.unit_price),
                    total: to_f64_or_nan(i.total),
                })
                .collect(),
            version: o.version,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Orders API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Orders", free_immediately, size)]
pub struct Orders {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Orders {
    fn create(
        &self,
        customer_id: String,
        items: Vec<RHash>,
        currency: Option<String>,
        notes: Option<String>,
    ) -> Result<Order, Error> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");

        let order_items: Vec<stateset_core::CreateOrderItem> = items
            .into_iter()
            .map(|h| {
                let sku: String = h.fetch(Symbol::new("sku")).unwrap_or_default();
                let name: String = h.fetch(Symbol::new("name")).unwrap_or_default();
                let quantity: i32 = h.fetch(Symbol::new("quantity")).unwrap_or(1);
                let unit_price: f64 = h.fetch(Symbol::new("unit_price")).unwrap_or(0.0);

                stateset_core::CreateOrderItem {
                    product_id: Default::default(),
                    variant_id: None,
                    sku,
                    name,
                    quantity,
                    unit_price: Decimal::from_f64_retain(unit_price).unwrap_or_default(),
                    ..Default::default()
                }
            })
            .collect();

        let order = commerce
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id: cust_uuid,
                items: order_items,
                currency,
                notes,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create order: {}", e),
                )
            })?;

        Ok(order.into())
    }

    fn get(&self, id: String) -> Result<Option<Order>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce.orders().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get order: {}", e),
            )
        })?;

        Ok(order.map(|o| o.into()))
    }

    fn list(&self) -> Result<Vec<Order>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let orders = commerce.orders().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list orders: {}", e),
            )
        })?;

        Ok(orders.into_iter().map(|o| o.into()).collect())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce.orders().count(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to count orders: {}", e),
            )
        })?;

        Ok(count)
    }

    fn ship(
        &self,
        id: String,
        tracking_number: Option<String>,
        carrier: Option<String>,
    ) -> Result<Order, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce
            .orders()
            .ship(uuid, tracking_number, carrier)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to ship order: {}", e),
                )
            })?;

        Ok(order.into())
    }

    fn cancel(&self, id: String, reason: Option<String>) -> Result<Order, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce.orders().cancel(uuid, reason).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to cancel order: {}", e),
            )
        })?;

        Ok(order.into())
    }

    fn confirm(&self, id: String) -> Result<Order, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce.orders().confirm(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to confirm order: {}", e),
            )
        })?;

        Ok(order.into())
    }

    fn deliver(&self, id: String) -> Result<Order, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce.orders().deliver(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to deliver order: {}", e),
            )
        })?;

        Ok(order.into())
    }
}

// ============================================================================
// Product Types
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::ProductVariant", free_immediately, size)]
pub struct ProductVariant {
    id: String,
    sku: String,
    name: String,
    price: f64,
    compare_at_price: Option<f64>,
    inventory_quantity: i32,
    weight: Option<f64>,
    barcode: Option<String>,
}

impl ProductVariant {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn sku(&self) -> String {
        self.sku.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn price(&self) -> f64 {
        self.price
    }
    fn compare_at_price(&self) -> Option<f64> {
        self.compare_at_price
    }
    fn inventory_quantity(&self) -> i32 {
        self.inventory_quantity
    }
    fn weight(&self) -> Option<f64> {
        self.weight
    }
    fn barcode(&self) -> Option<String> {
        self.barcode.clone()
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Product", free_immediately, size)]
pub struct Product {
    id: String,
    name: String,
    description: Option<String>,
    vendor: Option<String>,
    product_type: Option<String>,
    status: String,
    tags: Vec<String>,
    variants: Vec<ProductVariant>,
    created_at: String,
    updated_at: String,
}

impl Product {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn description(&self) -> Option<String> {
        self.description.clone()
    }
    fn vendor(&self) -> Option<String> {
        self.vendor.clone()
    }
    fn product_type(&self) -> Option<String> {
        self.product_type.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn tags(&self) -> Vec<String> {
        self.tags.clone()
    }
    fn variants(&self) -> Vec<ProductVariant> {
        self.variants.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Product id=\"{}\" name=\"{}\" status=\"{}\">",
            self.id, self.name, self.status
        )
    }
}

impl From<stateset_core::Product> for Product {
    fn from(p: stateset_core::Product) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            vendor: p.vendor,
            product_type: p.product_type,
            status: format!("{}", p.status),
            tags: p.tags,
            variants: p
                .variants
                .into_iter()
                .map(|v| ProductVariant {
                    id: v.id.to_string(),
                    sku: v.sku,
                    name: v.name,
                    price: to_f64_or_nan(v.price),
                    compare_at_price: v.compare_at_price.and_then(|p| p.to_f64()),
                    inventory_quantity: v.inventory_quantity,
                    weight: v.weight.and_then(|w| w.to_f64()),
                    barcode: v.barcode,
                })
                .collect(),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Products API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Products", free_immediately, size)]
pub struct Products {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Products {
    fn create(
        &self,
        name: String,
        description: Option<String>,
        vendor: Option<String>,
        product_type: Option<String>,
    ) -> Result<Product, Error> {
        let commerce = lock_commerce!(self.commerce);

        let product = commerce
            .products()
            .create(stateset_core::CreateProduct {
                name,
                description,
                vendor,
                product_type,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create product: {}", e),
                )
            })?;

        Ok(product.into())
    }

    fn get(&self, id: String) -> Result<Option<Product>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "product");

        let product = commerce.products().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get product: {}", e),
            )
        })?;

        Ok(product.map(|p| p.into()))
    }

    fn list(&self) -> Result<Vec<Product>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let products = commerce.products().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list products: {}", e),
            )
        })?;

        Ok(products.into_iter().map(|p| p.into()).collect())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce.products().count(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to count products: {}", e),
            )
        })?;

        Ok(count)
    }

    fn get_by_sku(&self, sku: String) -> Result<Option<ProductVariant>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let variant = commerce.products().get_variant_by_sku(&sku).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get variant: {}", e),
            )
        })?;

        Ok(variant.map(|v| ProductVariant {
            id: v.id.to_string(),
            sku: v.sku,
            name: v.name,
            price: to_f64_or_nan(v.price),
            compare_at_price: v.compare_at_price.and_then(|p| p.to_f64()),
            inventory_quantity: v.inventory_quantity,
            weight: v.weight.and_then(|w| w.to_f64()),
            barcode: v.barcode,
        }))
    }
}

// ============================================================================
// Inventory Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::InventoryItem", free_immediately, size)]
pub struct InventoryItem {
    id: String,
    sku: String,
    quantity_on_hand: i32,
    quantity_reserved: i32,
    quantity_available: i32,
    reorder_point: Option<i32>,
    reorder_quantity: Option<i32>,
    location_id: Option<String>,
}

impl InventoryItem {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn sku(&self) -> String {
        self.sku.clone()
    }
    fn quantity_on_hand(&self) -> i32 {
        self.quantity_on_hand
    }
    fn quantity_reserved(&self) -> i32 {
        self.quantity_reserved
    }
    fn quantity_available(&self) -> i32 {
        self.quantity_available
    }
    fn reorder_point(&self) -> Option<i32> {
        self.reorder_point
    }
    fn reorder_quantity(&self) -> Option<i32> {
        self.reorder_quantity
    }
    fn location_id(&self) -> Option<String> {
        self.location_id.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::InventoryItem sku=\"{}\" available={}>",
            self.sku, self.quantity_available
        )
    }
}

impl From<stateset_core::InventoryItem> for InventoryItem {
    fn from(i: stateset_core::InventoryItem) -> Self {
        Self {
            id: i.id.to_string(),
            sku: i.sku,
            quantity_on_hand: i.quantity_on_hand,
            quantity_reserved: i.quantity_reserved,
            quantity_available: i.quantity_available,
            reorder_point: i.reorder_point,
            reorder_quantity: i.reorder_quantity,
            location_id: i.location_id.map(|id| id.to_string()),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Inventory", free_immediately, size)]
pub struct Inventory {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Inventory {
    fn create(
        &self,
        sku: String,
        quantity: i32,
        reorder_point: Option<i32>,
        reorder_quantity: Option<i32>,
    ) -> Result<InventoryItem, Error> {
        let commerce = lock_commerce!(self.commerce);

        let item = commerce
            .inventory()
            .create(stateset_core::CreateInventoryItem {
                sku,
                quantity_on_hand: quantity,
                reorder_point,
                reorder_quantity,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create inventory: {}", e),
                )
            })?;

        Ok(item.into())
    }

    fn get(&self, id: String) -> Result<Option<InventoryItem>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");

        let item = commerce.inventory().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get inventory: {}", e),
            )
        })?;

        Ok(item.map(|i| i.into()))
    }

    fn get_by_sku(&self, sku: String) -> Result<Option<InventoryItem>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let item = commerce.inventory().get_by_sku(&sku).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get inventory: {}", e),
            )
        })?;

        Ok(item.map(|i| i.into()))
    }

    fn list(&self) -> Result<Vec<InventoryItem>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let items = commerce.inventory().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list inventory: {}", e),
            )
        })?;

        Ok(items.into_iter().map(|i| i.into()).collect())
    }

    fn adjust(
        &self,
        id: String,
        adjustment: i32,
        reason: Option<String>,
    ) -> Result<InventoryItem, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");

        let item = commerce
            .inventory()
            .adjust(uuid, adjustment, reason)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to adjust inventory: {}", e),
                )
            })?;

        Ok(item.into())
    }

    fn reserve(
        &self,
        id: String,
        quantity: i32,
        order_id: Option<String>,
    ) -> Result<InventoryItem, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");
        let order_uuid = order_id.map(|s| s.parse().ok()).flatten();

        let item = commerce
            .inventory()
            .reserve(uuid, quantity, order_uuid)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to reserve inventory: {}", e),
                )
            })?;

        Ok(item.into())
    }

    fn release(&self, id: String, quantity: i32) -> Result<InventoryItem, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");

        let item = commerce.inventory().release(uuid, quantity).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to release inventory: {}", e),
            )
        })?;

        Ok(item.into())
    }
}

// ============================================================================
// Returns Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Return", free_immediately, size)]
pub struct Return {
    id: String,
    order_id: String,
    customer_id: String,
    status: String,
    reason: String,
    refund_amount: f64,
    created_at: String,
    updated_at: String,
}

impl Return {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn order_id(&self) -> String {
        self.order_id.clone()
    }
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn reason(&self) -> String {
        self.reason.clone()
    }
    fn refund_amount(&self) -> f64 {
        self.refund_amount
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Return id=\"{}\" status=\"{}\" refund={}>",
            self.id, self.status, self.refund_amount
        )
    }
}

impl From<stateset_core::Return> for Return {
    fn from(r: stateset_core::Return) -> Self {
        Self {
            id: r.id.to_string(),
            order_id: r.order_id.to_string(),
            customer_id: r.customer_id.to_string(),
            status: format!("{}", r.status),
            reason: r.reason,
            refund_amount: to_f64_or_nan(r.refund_amount),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Returns", free_immediately, size)]
pub struct Returns {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Returns {
    fn create(&self, order_id: String, reason: String) -> Result<Return, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");

        let ret = commerce
            .returns()
            .create(stateset_core::CreateReturn {
                order_id: uuid,
                reason,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create return: {}", e),
                )
            })?;

        Ok(ret.into())
    }

    fn get(&self, id: String) -> Result<Option<Return>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "return");

        let ret = commerce.returns().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get return: {}", e),
            )
        })?;

        Ok(ret.map(|r| r.into()))
    }

    fn list(&self) -> Result<Vec<Return>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let returns = commerce.returns().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list returns: {}", e),
            )
        })?;

        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    fn approve(&self, id: String, refund_amount: Option<f64>) -> Result<Return, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "return");
        let amount = refund_amount.map(|a| Decimal::from_f64_retain(a).unwrap_or_default());

        let ret = commerce.returns().approve(uuid, amount).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to approve return: {}", e),
            )
        })?;

        Ok(ret.into())
    }

    fn reject(&self, id: String, reason: Option<String>) -> Result<Return, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "return");

        let ret = commerce.returns().reject(uuid, reason).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to reject return: {}", e),
            )
        })?;

        Ok(ret.into())
    }
}

// ============================================================================
// Payments API (Stub)
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Payments", free_immediately, size)]
pub struct Payments {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Payments {
    fn record(&self, order_id: String, amount: f64, method: Option<String>) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");
        let decimal_amount = Decimal::from_f64_retain(amount).unwrap_or_default();

        commerce
            .payments()
            .record(uuid, decimal_amount, method)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to record payment: {}", e),
                )
            })?;

        Ok(true)
    }
}

// ============================================================================
// Shipments Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Shipment", free_immediately, size)]
pub struct Shipment {
    id: String,
    shipment_number: String,
    order_id: String,
    status: String,
    carrier: Option<String>,
    tracking_number: Option<String>,
    shipping_method: Option<String>,
    weight: Option<f64>,
    estimated_delivery: Option<String>,
    shipped_at: Option<String>,
    delivered_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl Shipment {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn shipment_number(&self) -> String {
        self.shipment_number.clone()
    }
    fn order_id(&self) -> String {
        self.order_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn carrier(&self) -> Option<String> {
        self.carrier.clone()
    }
    fn tracking_number(&self) -> Option<String> {
        self.tracking_number.clone()
    }
    fn shipping_method(&self) -> Option<String> {
        self.shipping_method.clone()
    }
    fn weight(&self) -> Option<f64> {
        self.weight
    }
    fn estimated_delivery(&self) -> Option<String> {
        self.estimated_delivery.clone()
    }
    fn shipped_at(&self) -> Option<String> {
        self.shipped_at.clone()
    }
    fn delivered_at(&self) -> Option<String> {
        self.delivered_at.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Shipment number=\"{}\" status=\"{}\">",
            self.shipment_number, self.status
        )
    }
}

impl From<stateset_core::Shipment> for Shipment {
    fn from(s: stateset_core::Shipment) -> Self {
        Self {
            id: s.id.to_string(),
            shipment_number: s.shipment_number,
            order_id: s.order_id.to_string(),
            status: format!("{}", s.status),
            carrier: s.carrier,
            tracking_number: s.tracking_number,
            shipping_method: s.shipping_method,
            weight: s.weight.and_then(|w| w.to_f64()),
            estimated_delivery: s.estimated_delivery.map(|d| d.to_rfc3339()),
            shipped_at: s.shipped_at.map(|d| d.to_rfc3339()),
            delivered_at: s.delivered_at.map(|d| d.to_rfc3339()),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Shipments", free_immediately, size)]
pub struct Shipments {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Shipments {
    fn create(
        &self,
        order_id: String,
        carrier: Option<String>,
        shipping_method: Option<String>,
    ) -> Result<Shipment, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");
        let shipment = commerce
            .shipments()
            .create(stateset_core::CreateShipment {
                order_id: uuid,
                carrier,
                shipping_method,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create shipment: {}", e),
                )
            })?;
        Ok(shipment.into())
    }

    fn get(&self, id: String) -> Result<Option<Shipment>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");
        let shipment = commerce.shipments().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get shipment: {}", e),
            )
        })?;
        Ok(shipment.map(|s| s.into()))
    }

    fn get_by_tracking(&self, tracking_number: String) -> Result<Option<Shipment>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let shipment = commerce
            .shipments()
            .get_by_tracking(&tracking_number)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to get shipment: {}", e),
                )
            })?;
        Ok(shipment.map(|s| s.into()))
    }

    fn list(&self) -> Result<Vec<Shipment>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let shipments = commerce.shipments().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list shipments: {}", e),
            )
        })?;
        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    fn for_order(&self, order_id: String) -> Result<Vec<Shipment>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");
        let shipments = commerce.shipments().for_order(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get shipments: {}", e),
            )
        })?;
        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    fn ship(&self, id: String, tracking_number: Option<String>) -> Result<Shipment, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");
        let shipment = commerce
            .shipments()
            .ship(uuid, tracking_number)
            .map_err(|e| {
                Error::new(exception::runtime_error(), format!("Failed to ship: {}", e))
            })?;
        Ok(shipment.into())
    }

    fn mark_delivered(&self, id: String) -> Result<Shipment, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");
        let shipment = commerce.shipments().mark_delivered(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to mark delivered: {}", e),
            )
        })?;
        Ok(shipment.into())
    }

    fn cancel(&self, id: String) -> Result<Shipment, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");
        let shipment = commerce.shipments().cancel(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to cancel: {}", e),
            )
        })?;
        Ok(shipment.into())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let count = commerce
            .shipments()
            .count(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to count: {}", e),
                )
            })?;
        Ok(count as i64)
    }
}

// ============================================================================
// Warranties Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Warranty", free_immediately, size)]
pub struct Warranty {
    id: String,
    warranty_number: String,
    order_id: String,
    customer_id: String,
    product_id: Option<String>,
    serial_number: Option<String>,
    status: String,
    warranty_type: String,
    start_date: String,
    end_date: String,
    created_at: String,
    updated_at: String,
}

impl Warranty {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn warranty_number(&self) -> String {
        self.warranty_number.clone()
    }
    fn order_id(&self) -> String {
        self.order_id.clone()
    }
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn product_id(&self) -> Option<String> {
        self.product_id.clone()
    }
    fn serial_number(&self) -> Option<String> {
        self.serial_number.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn warranty_type(&self) -> String {
        self.warranty_type.clone()
    }
    fn start_date(&self) -> String {
        self.start_date.clone()
    }
    fn end_date(&self) -> String {
        self.end_date.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Warranty number=\"{}\" status=\"{}\">",
            self.warranty_number, self.status
        )
    }
}

impl From<stateset_core::Warranty> for Warranty {
    fn from(w: stateset_core::Warranty) -> Self {
        Self {
            id: w.id.to_string(),
            warranty_number: w.warranty_number,
            order_id: w.order_id.to_string(),
            customer_id: w.customer_id.to_string(),
            product_id: w.product_id.map(|p| p.to_string()),
            serial_number: w.serial_number,
            status: format!("{}", w.status),
            warranty_type: format!("{}", w.warranty_type),
            start_date: w.start_date.to_string(),
            end_date: w.end_date.to_string(),
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::WarrantyClaim", free_immediately, size)]
pub struct WarrantyClaim {
    id: String,
    claim_number: String,
    warranty_id: String,
    status: String,
    description: String,
    resolution: Option<String>,
    created_at: String,
    updated_at: String,
}

impl WarrantyClaim {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn claim_number(&self) -> String {
        self.claim_number.clone()
    }
    fn warranty_id(&self) -> String {
        self.warranty_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn description(&self) -> String {
        self.description.clone()
    }
    fn resolution(&self) -> Option<String> {
        self.resolution.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
}

impl From<stateset_core::WarrantyClaim> for WarrantyClaim {
    fn from(c: stateset_core::WarrantyClaim) -> Self {
        Self {
            id: c.id.to_string(),
            claim_number: c.claim_number,
            warranty_id: c.warranty_id.to_string(),
            status: format!("{}", c.status),
            description: c.description,
            resolution: c.resolution,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Warranties", free_immediately, size)]
pub struct Warranties {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Warranties {
    fn create(
        &self,
        order_id: String,
        customer_id: String,
        warranty_type: String,
        duration_months: i32,
    ) -> Result<Warranty, Error> {
        let commerce = lock_commerce!(self.commerce);
        let order_uuid = parse_uuid!(order_id, "order");
        let customer_uuid = parse_uuid!(customer_id, "customer");
        let warranty = commerce
            .warranties()
            .create(stateset_core::CreateWarranty {
                order_id: order_uuid,
                customer_id: customer_uuid,
                warranty_type: warranty_type.parse().unwrap_or_default(),
                duration_months,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create warranty: {}", e),
                )
            })?;
        Ok(warranty.into())
    }

    fn get(&self, id: String) -> Result<Option<Warranty>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "warranty");
        let warranty = commerce.warranties().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get warranty: {}", e),
            )
        })?;
        Ok(warranty.map(|w| w.into()))
    }

    fn list(&self) -> Result<Vec<Warranty>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let warranties = commerce
            .warranties()
            .list(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list warranties: {}", e),
                )
            })?;
        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    fn for_customer(&self, customer_id: String) -> Result<Vec<Warranty>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");
        let warranties = commerce.warranties().for_customer(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get warranties: {}", e),
            )
        })?;
        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    fn is_valid(&self, id: String) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "warranty");
        let valid = commerce.warranties().is_valid(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to check warranty: {}", e),
            )
        })?;
        Ok(valid)
    }

    fn create_claim(
        &self,
        warranty_id: String,
        description: String,
    ) -> Result<WarrantyClaim, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(warranty_id, "warranty");
        let claim = commerce
            .warranties()
            .create_claim(stateset_core::CreateWarrantyClaim {
                warranty_id: uuid,
                description,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create claim: {}", e),
                )
            })?;
        Ok(claim.into())
    }

    fn approve_claim(&self, id: String) -> Result<WarrantyClaim, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "claim");
        let claim = commerce.warranties().approve_claim(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to approve claim: {}", e),
            )
        })?;
        Ok(claim.into())
    }

    fn deny_claim(&self, id: String, reason: String) -> Result<WarrantyClaim, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "claim");
        let claim = commerce
            .warranties()
            .deny_claim(uuid, &reason)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to deny claim: {}", e),
                )
            })?;
        Ok(claim.into())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let count = commerce
            .warranties()
            .count(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to count: {}", e),
                )
            })?;
        Ok(count as i64)
    }
}

// ============================================================================
// Purchase Orders Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Supplier", free_immediately, size)]
pub struct Supplier {
    id: String,
    code: String,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

impl Supplier {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn code(&self) -> String {
        self.code.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn email(&self) -> Option<String> {
        self.email.clone()
    }
    fn phone(&self) -> Option<String> {
        self.phone.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
}

impl From<stateset_core::Supplier> for Supplier {
    fn from(s: stateset_core::Supplier) -> Self {
        Self {
            id: s.id.to_string(),
            code: s.code,
            name: s.name,
            email: s.email,
            phone: s.phone,
            status: format!("{}", s.status),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::PurchaseOrder", free_immediately, size)]
pub struct PurchaseOrder {
    id: String,
    po_number: String,
    supplier_id: String,
    status: String,
    total_amount: f64,
    currency: String,
    expected_delivery: Option<String>,
    created_at: String,
    updated_at: String,
}

impl PurchaseOrder {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn po_number(&self) -> String {
        self.po_number.clone()
    }
    fn supplier_id(&self) -> String {
        self.supplier_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn total_amount(&self) -> f64 {
        self.total_amount
    }
    fn currency(&self) -> String {
        self.currency.clone()
    }
    fn expected_delivery(&self) -> Option<String> {
        self.expected_delivery.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::PurchaseOrder number=\"{}\" status=\"{}\">",
            self.po_number, self.status
        )
    }
}

impl From<stateset_core::PurchaseOrder> for PurchaseOrder {
    fn from(p: stateset_core::PurchaseOrder) -> Self {
        Self {
            id: p.id.to_string(),
            po_number: p.po_number,
            supplier_id: p.supplier_id.to_string(),
            status: format!("{}", p.status),
            total_amount: to_f64_or_nan(p.total_amount),
            currency: p.currency,
            expected_delivery: p.expected_delivery.map(|d| d.to_string()),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::PurchaseOrders", free_immediately, size)]
pub struct PurchaseOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl PurchaseOrders {
    fn create_supplier(
        &self,
        code: String,
        name: String,
        email: Option<String>,
    ) -> Result<Supplier, Error> {
        let commerce = lock_commerce!(self.commerce);
        let supplier = commerce
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                code,
                name,
                email,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create supplier: {}", e),
                )
            })?;
        Ok(supplier.into())
    }

    fn get_supplier(&self, id: String) -> Result<Option<Supplier>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "supplier");
        let supplier = commerce.purchase_orders().get_supplier(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get supplier: {}", e),
            )
        })?;
        Ok(supplier.map(|s| s.into()))
    }

    fn list_suppliers(&self) -> Result<Vec<Supplier>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let suppliers = commerce
            .purchase_orders()
            .list_suppliers(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list suppliers: {}", e),
                )
            })?;
        Ok(suppliers.into_iter().map(|s| s.into()).collect())
    }

    fn create(
        &self,
        supplier_id: String,
        currency: Option<String>,
    ) -> Result<PurchaseOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(supplier_id, "supplier");
        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id: uuid,
                currency,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create PO: {}", e),
                )
            })?;
        Ok(po.into())
    }

    fn get(&self, id: String) -> Result<Option<PurchaseOrder>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");
        let po = commerce.purchase_orders().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get PO: {}", e),
            )
        })?;
        Ok(po.map(|p| p.into()))
    }

    fn list(&self) -> Result<Vec<PurchaseOrder>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let pos = commerce
            .purchase_orders()
            .list(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list POs: {}", e),
                )
            })?;
        Ok(pos.into_iter().map(|p| p.into()).collect())
    }

    fn submit(&self, id: String) -> Result<PurchaseOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");
        let po = commerce.purchase_orders().submit(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to submit PO: {}", e),
            )
        })?;
        Ok(po.into())
    }

    fn approve(&self, id: String, approved_by: String) -> Result<PurchaseOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");
        let po = commerce
            .purchase_orders()
            .approve(uuid, &approved_by)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to approve PO: {}", e),
                )
            })?;
        Ok(po.into())
    }

    fn cancel(&self, id: String) -> Result<PurchaseOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");
        let po = commerce.purchase_orders().cancel(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to cancel PO: {}", e),
            )
        })?;
        Ok(po.into())
    }

    fn complete(&self, id: String) -> Result<PurchaseOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");
        let po = commerce.purchase_orders().complete(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to complete PO: {}", e),
            )
        })?;
        Ok(po.into())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let count = commerce
            .purchase_orders()
            .count(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to count: {}", e),
                )
            })?;
        Ok(count as i64)
    }
}

// ============================================================================
// Invoices Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Invoice", free_immediately, size)]
pub struct Invoice {
    id: String,
    invoice_number: String,
    customer_id: String,
    order_id: Option<String>,
    status: String,
    subtotal: f64,
    tax_amount: f64,
    total_amount: f64,
    amount_paid: f64,
    amount_due: f64,
    currency: String,
    due_date: Option<String>,
    created_at: String,
    updated_at: String,
}

impl Invoice {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn invoice_number(&self) -> String {
        self.invoice_number.clone()
    }
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn order_id(&self) -> Option<String> {
        self.order_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn subtotal(&self) -> f64 {
        self.subtotal
    }
    fn tax_amount(&self) -> f64 {
        self.tax_amount
    }
    fn total_amount(&self) -> f64 {
        self.total_amount
    }
    fn amount_paid(&self) -> f64 {
        self.amount_paid
    }
    fn amount_due(&self) -> f64 {
        self.amount_due
    }
    fn currency(&self) -> String {
        self.currency.clone()
    }
    fn due_date(&self) -> Option<String> {
        self.due_date.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Invoice number=\"{}\" due={}>",
            self.invoice_number, self.amount_due
        )
    }
}

impl From<stateset_core::Invoice> for Invoice {
    fn from(i: stateset_core::Invoice) -> Self {
        Self {
            id: i.id.to_string(),
            invoice_number: i.invoice_number,
            customer_id: i.customer_id.to_string(),
            order_id: i.order_id.map(|o| o.to_string()),
            status: format!("{}", i.status),
            subtotal: to_f64_or_nan(i.subtotal),
            tax_amount: to_f64_or_nan(i.tax_amount),
            total_amount: to_f64_or_nan(i.total_amount),
            amount_paid: to_f64_or_nan(i.amount_paid),
            amount_due: to_f64_or_nan(i.amount_due),
            currency: i.currency,
            due_date: i.due_date.map(|d| d.to_string()),
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Invoices", free_immediately, size)]
pub struct Invoices {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Invoices {
    fn create(
        &self,
        customer_id: String,
        order_id: Option<String>,
        due_days: Option<i32>,
    ) -> Result<Invoice, Error> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let order_uuid = order_id.map(|s| s.parse().ok()).flatten();
        let invoice = commerce
            .invoices()
            .create(stateset_core::CreateInvoice {
                customer_id: cust_uuid,
                order_id: order_uuid,
                due_days,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create invoice: {}", e),
                )
            })?;
        Ok(invoice.into())
    }

    fn get(&self, id: String) -> Result<Option<Invoice>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");
        let invoice = commerce.invoices().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get invoice: {}", e),
            )
        })?;
        Ok(invoice.map(|i| i.into()))
    }

    fn list(&self) -> Result<Vec<Invoice>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let invoices = commerce.invoices().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list invoices: {}", e),
            )
        })?;
        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    fn for_customer(&self, customer_id: String) -> Result<Vec<Invoice>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");
        let invoices = commerce.invoices().for_customer(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get invoices: {}", e),
            )
        })?;
        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    fn send(&self, id: String) -> Result<Invoice, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");
        let invoice = commerce.invoices().send(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to send invoice: {}", e),
            )
        })?;
        Ok(invoice.into())
    }

    fn record_payment(
        &self,
        id: String,
        amount: f64,
        method: Option<String>,
    ) -> Result<Invoice, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");
        let invoice = commerce
            .invoices()
            .record_payment(
                uuid,
                stateset_core::RecordInvoicePayment {
                    amount: Decimal::from_f64_retain(amount).unwrap_or_default(),
                    payment_method: method,
                    ..Default::default()
                },
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to record payment: {}", e),
                )
            })?;
        Ok(invoice.into())
    }

    fn void(&self, id: String) -> Result<Invoice, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");
        let invoice = commerce.invoices().void(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to void invoice: {}", e),
            )
        })?;
        Ok(invoice.into())
    }

    fn get_overdue(&self) -> Result<Vec<Invoice>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let invoices = commerce.invoices().get_overdue().map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get overdue: {}", e),
            )
        })?;
        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    fn customer_balance(&self, customer_id: String) -> Result<f64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");
        let balance = commerce.invoices().customer_balance(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get balance: {}", e),
            )
        })?;
        Ok(to_f64_or_nan(balance))
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let count = commerce.invoices().count(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to count: {}", e),
            )
        })?;
        Ok(count as i64)
    }
}

// ============================================================================
// BOM Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::BillOfMaterials", free_immediately, size)]
pub struct BillOfMaterials {
    id: String,
    bom_number: String,
    name: String,
    product_id: Option<String>,
    status: String,
    version: i32,
    created_at: String,
    updated_at: String,
}

impl BillOfMaterials {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn bom_number(&self) -> String {
        self.bom_number.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn product_id(&self) -> Option<String> {
        self.product_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn version(&self) -> i32 {
        self.version
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::BillOfMaterials number=\"{}\" name=\"{}\">",
            self.bom_number, self.name
        )
    }
}

impl From<stateset_core::BillOfMaterials> for BillOfMaterials {
    fn from(b: stateset_core::BillOfMaterials) -> Self {
        Self {
            id: b.id.to_string(),
            bom_number: b.bom_number,
            name: b.name,
            product_id: b.product_id.map(|p| p.to_string()),
            status: format!("{}", b.status),
            version: b.version,
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::BomComponent", free_immediately, size)]
pub struct BomComponent {
    id: String,
    bom_id: String,
    sku: String,
    name: String,
    quantity: f64,
    unit: String,
}

impl BomComponent {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn bom_id(&self) -> String {
        self.bom_id.clone()
    }
    fn sku(&self) -> String {
        self.sku.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn quantity(&self) -> f64 {
        self.quantity
    }
    fn unit(&self) -> String {
        self.unit.clone()
    }
}

impl From<stateset_core::BomComponent> for BomComponent {
    fn from(c: stateset_core::BomComponent) -> Self {
        Self {
            id: c.id.to_string(),
            bom_id: c.bom_id.to_string(),
            sku: c.sku,
            name: c.name,
            quantity: to_f64_or_nan(c.quantity),
            unit: c.unit,
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::BomApi", free_immediately, size)]
pub struct BomApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl BomApi {
    fn create(&self, name: String, product_id: Option<String>) -> Result<BillOfMaterials, Error> {
        let commerce = lock_commerce!(self.commerce);
        let prod_uuid = product_id.map(|s| s.parse().ok()).flatten();
        let bom = commerce
            .bom()
            .create(stateset_core::CreateBom {
                name,
                product_id: prod_uuid,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create BOM: {}", e),
                )
            })?;
        Ok(bom.into())
    }

    fn get(&self, id: String) -> Result<Option<BillOfMaterials>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bom");
        let bom = commerce.bom().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get BOM: {}", e),
            )
        })?;
        Ok(bom.map(|b| b.into()))
    }

    fn list(&self) -> Result<Vec<BillOfMaterials>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let boms = commerce.bom().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list BOMs: {}", e),
            )
        })?;
        Ok(boms.into_iter().map(|b| b.into()).collect())
    }

    fn add_component(
        &self,
        bom_id: String,
        sku: String,
        name: String,
        quantity: f64,
        unit: String,
    ) -> Result<BomComponent, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(bom_id, "bom");
        let component = commerce
            .bom()
            .add_component(
                uuid,
                stateset_core::CreateBomComponent {
                    sku,
                    name,
                    quantity: Decimal::from_f64_retain(quantity).unwrap_or_default(),
                    unit,
                    ..Default::default()
                },
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to add component: {}", e),
                )
            })?;
        Ok(component.into())
    }

    fn get_components(&self, bom_id: String) -> Result<Vec<BomComponent>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(bom_id, "bom");
        let components = commerce.bom().get_components(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get components: {}", e),
            )
        })?;
        Ok(components.into_iter().map(|c| c.into()).collect())
    }

    fn remove_component(&self, component_id: String) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(component_id, "component");
        commerce.bom().remove_component(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to remove component: {}", e),
            )
        })?;
        Ok(true)
    }

    fn activate(&self, id: String) -> Result<BillOfMaterials, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bom");
        let bom = commerce.bom().activate(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to activate BOM: {}", e),
            )
        })?;
        Ok(bom.into())
    }

    fn delete(&self, id: String) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bom");
        commerce.bom().delete(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to delete BOM: {}", e),
            )
        })?;
        Ok(true)
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let count = commerce.bom().count(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to count: {}", e),
            )
        })?;
        Ok(count as i64)
    }
}

// ============================================================================
// Work Orders Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::WorkOrder", free_immediately, size)]
pub struct WorkOrder {
    id: String,
    work_order_number: String,
    bom_id: Option<String>,
    product_id: Option<String>,
    status: String,
    quantity_ordered: f64,
    quantity_completed: f64,
    priority: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl WorkOrder {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn work_order_number(&self) -> String {
        self.work_order_number.clone()
    }
    fn bom_id(&self) -> Option<String> {
        self.bom_id.clone()
    }
    fn product_id(&self) -> Option<String> {
        self.product_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn quantity_ordered(&self) -> f64 {
        self.quantity_ordered
    }
    fn quantity_completed(&self) -> f64 {
        self.quantity_completed
    }
    fn priority(&self) -> String {
        self.priority.clone()
    }
    fn started_at(&self) -> Option<String> {
        self.started_at.clone()
    }
    fn completed_at(&self) -> Option<String> {
        self.completed_at.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::WorkOrder number=\"{}\" status=\"{}\">",
            self.work_order_number, self.status
        )
    }
}

impl From<stateset_core::WorkOrder> for WorkOrder {
    fn from(w: stateset_core::WorkOrder) -> Self {
        Self {
            id: w.id.to_string(),
            work_order_number: w.work_order_number,
            bom_id: w.bom_id.map(|b| b.to_string()),
            product_id: w.product_id.map(|p| p.to_string()),
            status: format!("{}", w.status),
            quantity_ordered: to_f64_or_nan(w.quantity_ordered),
            quantity_completed: to_f64_or_nan(w.quantity_completed),
            priority: format!("{}", w.priority),
            started_at: w.started_at.map(|d| d.to_rfc3339()),
            completed_at: w.completed_at.map(|d| d.to_rfc3339()),
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::WorkOrders", free_immediately, size)]
pub struct WorkOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl WorkOrders {
    fn create(
        &self,
        bom_id: Option<String>,
        product_id: Option<String>,
        quantity: f64,
    ) -> Result<WorkOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let bom_uuid = bom_id.map(|s| s.parse().ok()).flatten();
        let prod_uuid = product_id.map(|s| s.parse().ok()).flatten();
        let wo = commerce
            .work_orders()
            .create(stateset_core::CreateWorkOrder {
                bom_id: bom_uuid,
                product_id: prod_uuid,
                quantity_ordered: Decimal::from_f64_retain(quantity).unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create work order: {}", e),
                )
            })?;
        Ok(wo.into())
    }

    fn get(&self, id: String) -> Result<Option<WorkOrder>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");
        let wo = commerce.work_orders().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get work order: {}", e),
            )
        })?;
        Ok(wo.map(|w| w.into()))
    }

    fn list(&self) -> Result<Vec<WorkOrder>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let wos = commerce
            .work_orders()
            .list(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list work orders: {}", e),
                )
            })?;
        Ok(wos.into_iter().map(|w| w.into()).collect())
    }

    fn start(&self, id: String) -> Result<WorkOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");
        let wo = commerce.work_orders().start(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to start: {}", e),
            )
        })?;
        Ok(wo.into())
    }

    fn complete(&self, id: String, quantity_completed: f64) -> Result<WorkOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");
        let wo = commerce
            .work_orders()
            .complete(
                uuid,
                Decimal::from_f64_retain(quantity_completed).unwrap_or_default(),
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to complete: {}", e),
                )
            })?;
        Ok(wo.into())
    }

    fn hold(&self, id: String) -> Result<WorkOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");
        let wo = commerce.work_orders().hold(uuid).map_err(|e| {
            Error::new(exception::runtime_error(), format!("Failed to hold: {}", e))
        })?;
        Ok(wo.into())
    }

    fn resume(&self, id: String) -> Result<WorkOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");
        let wo = commerce.work_orders().resume(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to resume: {}", e),
            )
        })?;
        Ok(wo.into())
    }

    fn cancel(&self, id: String) -> Result<WorkOrder, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");
        let wo = commerce.work_orders().cancel(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to cancel: {}", e),
            )
        })?;
        Ok(wo.into())
    }

    fn count(&self) -> Result<i64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let count = commerce
            .work_orders()
            .count(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to count: {}", e),
                )
            })?;
        Ok(count as i64)
    }
}

// ============================================================================
// Carts Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::CartItem", free_immediately, size)]
pub struct CartItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

impl CartItem {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn sku(&self) -> String {
        self.sku.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn quantity(&self) -> i32 {
        self.quantity
    }
    fn unit_price(&self) -> f64 {
        self.unit_price
    }
    fn total(&self) -> f64 {
        self.total
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Cart", free_immediately, size)]
pub struct Cart {
    id: String,
    customer_id: Option<String>,
    status: String,
    items: Vec<CartItem>,
    subtotal: f64,
    total: f64,
    currency: String,
    created_at: String,
    updated_at: String,
}

impl Cart {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn customer_id(&self) -> Option<String> {
        self.customer_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn items(&self) -> Vec<CartItem> {
        self.items.clone()
    }
    fn subtotal(&self) -> f64 {
        self.subtotal
    }
    fn total(&self) -> f64 {
        self.total
    }
    fn currency(&self) -> String {
        self.currency.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Cart id=\"{}\" items={} total={} {}>",
            self.id,
            self.items.len(),
            self.total,
            self.currency
        )
    }
}

impl From<stateset_core::Cart> for Cart {
    fn from(c: stateset_core::Cart) -> Self {
        Self {
            id: c.id.to_string(),
            customer_id: c.customer_id.map(|id| id.to_string()),
            status: format!("{}", c.status),
            items: c
                .items
                .into_iter()
                .map(|i| CartItem {
                    id: i.id.to_string(),
                    sku: i.sku,
                    name: i.name,
                    quantity: i.quantity,
                    unit_price: to_f64_or_nan(i.unit_price),
                    total: to_f64_or_nan(i.total),
                })
                .collect(),
            subtotal: to_f64_or_nan(c.subtotal),
            total: to_f64_or_nan(c.total),
            currency: c.currency,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Carts", free_immediately, size)]
pub struct Carts {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Carts {
    fn create(&self, customer_id: Option<String>, currency: Option<String>) -> Result<Cart, Error> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = customer_id.map(|s| s.parse().ok()).flatten();

        let cart = commerce
            .carts()
            .create(stateset_core::CreateCart {
                customer_id: cust_uuid,
                currency,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create cart: {}", e),
                )
            })?;

        Ok(cart.into())
    }

    fn get(&self, id: String) -> Result<Option<Cart>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "cart");

        let cart = commerce.carts().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get cart: {}", e),
            )
        })?;

        Ok(cart.map(|c| c.into()))
    }

    fn list(&self) -> Result<Vec<Cart>, Error> {
        let commerce = lock_commerce!(self.commerce);

        let carts = commerce.carts().list(Default::default()).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to list carts: {}", e),
            )
        })?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    fn add_item(
        &self,
        cart_id: String,
        sku: String,
        name: String,
        quantity: i32,
        unit_price: f64,
    ) -> Result<Cart, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(cart_id, "cart");
        let price = Decimal::from_f64_retain(unit_price).unwrap_or_default();

        let cart = commerce
            .carts()
            .add_item(
                uuid,
                stateset_core::AddCartItem {
                    sku,
                    name,
                    quantity,
                    unit_price: price,
                    ..Default::default()
                },
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to add item: {}", e),
                )
            })?;

        Ok(cart.into())
    }

    fn checkout(&self, cart_id: String) -> Result<Order, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(cart_id, "cart");

        let order = commerce.carts().checkout(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to checkout: {}", e),
            )
        })?;

        Ok(order.into())
    }
}

// ============================================================================
// Analytics API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::SalesSummary", free_immediately, size)]
pub struct SalesSummary {
    total_revenue: f64,
    total_orders: i64,
    average_order_value: f64,
    total_items_sold: i64,
}

impl SalesSummary {
    fn total_revenue(&self) -> f64 {
        self.total_revenue
    }
    fn total_orders(&self) -> i64 {
        self.total_orders
    }
    fn average_order_value(&self) -> f64 {
        self.average_order_value
    }
    fn total_items_sold(&self) -> i64 {
        self.total_items_sold
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Analytics", free_immediately, size)]
pub struct Analytics {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Analytics {
    fn sales_summary(&self, days: Option<i64>) -> Result<SalesSummary, Error> {
        let commerce = lock_commerce!(self.commerce);

        let summary = commerce
            .analytics()
            .sales_summary(days.unwrap_or(30))
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to get sales summary: {}", e),
                )
            })?;

        Ok(SalesSummary {
            total_revenue: to_f64_or_nan(summary.total_revenue),
            total_orders: summary.total_orders,
            average_order_value: to_f64_or_nan(summary.average_order_value),
            total_items_sold: summary.total_items_sold,
        })
    }
}

// ============================================================================
// Currency Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::ExchangeRate", free_immediately, size)]
pub struct ExchangeRate {
    id: String,
    from_currency: String,
    to_currency: String,
    rate: f64,
    effective_date: String,
    created_at: String,
}

impl ExchangeRate {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn from_currency(&self) -> String {
        self.from_currency.clone()
    }
    fn to_currency(&self) -> String {
        self.to_currency.clone()
    }
    fn rate(&self) -> f64 {
        self.rate
    }
    fn effective_date(&self) -> String {
        self.effective_date.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<stateset_core::ExchangeRate> for ExchangeRate {
    fn from(r: stateset_core::ExchangeRate) -> Self {
        Self {
            id: r.id.to_string(),
            from_currency: format!("{}", r.from_currency),
            to_currency: format!("{}", r.to_currency),
            rate: to_f64_or_nan(r.rate),
            effective_date: r.effective_date.to_string(),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::CurrencyOps", free_immediately, size)]
pub struct CurrencyOps {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl CurrencyOps {
    fn get_rate(&self, from: String, to: String) -> Result<Option<ExchangeRate>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let from_currency = from.parse().unwrap_or_default();
        let to_currency = to.parse().unwrap_or_default();
        let rate = commerce
            .currency()
            .get_rate(from_currency, to_currency)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to get rate: {}", e),
                )
            })?;
        Ok(rate.map(|r| r.into()))
    }

    fn list_rates(&self) -> Result<Vec<ExchangeRate>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let rates = commerce
            .currency()
            .list_rates(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list rates: {}", e),
                )
            })?;
        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    fn set_rate(&self, from: String, to: String, rate: f64) -> Result<ExchangeRate, Error> {
        let commerce = lock_commerce!(self.commerce);
        let exchange_rate = commerce
            .currency()
            .set_rate(stateset_core::SetExchangeRate {
                from_currency: from.parse().unwrap_or_default(),
                to_currency: to.parse().unwrap_or_default(),
                rate: Decimal::from_f64_retain(rate).unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to set rate: {}", e),
                )
            })?;
        Ok(exchange_rate.into())
    }

    fn convert(&self, amount: f64, from: String, to: String) -> Result<f64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let result = commerce
            .currency()
            .convert_amount(
                Decimal::from_f64_retain(amount).unwrap_or_default(),
                from.parse().unwrap_or_default(),
                to.parse().unwrap_or_default(),
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to convert: {}", e),
                )
            })?;
        Ok(to_f64_or_nan(result))
    }

    fn base_currency(&self) -> Result<String, Error> {
        let commerce = lock_commerce!(self.commerce);
        let currency = commerce.currency().base_currency().map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get base currency: {}", e),
            )
        })?;
        Ok(format!("{}", currency))
    }

    fn enabled_currencies(&self) -> Result<Vec<String>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let currencies = commerce.currency().enabled_currencies().map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get currencies: {}", e),
            )
        })?;
        Ok(currencies.into_iter().map(|c| format!("{}", c)).collect())
    }

    fn format(&self, amount: f64, currency: String) -> String {
        let commerce = self.commerce.lock().unwrap();
        commerce.currency().format(
            Decimal::from_f64_retain(amount).unwrap_or_default(),
            currency.parse().unwrap_or_default(),
        )
    }
}

// ============================================================================
// Subscriptions Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::SubscriptionPlan", free_immediately, size)]
pub struct SubscriptionPlan {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    price: f64,
    currency: String,
    billing_interval: String,
    trial_days: Option<i32>,
    status: String,
    created_at: String,
    updated_at: String,
}

impl SubscriptionPlan {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn code(&self) -> String {
        self.code.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn description(&self) -> Option<String> {
        self.description.clone()
    }
    fn price(&self) -> f64 {
        self.price
    }
    fn currency(&self) -> String {
        self.currency.clone()
    }
    fn billing_interval(&self) -> String {
        self.billing_interval.clone()
    }
    fn trial_days(&self) -> Option<i32> {
        self.trial_days
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
}

impl From<stateset_core::SubscriptionPlan> for SubscriptionPlan {
    fn from(p: stateset_core::SubscriptionPlan) -> Self {
        Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            price: to_f64_or_nan(p.price),
            currency: p.currency,
            billing_interval: format!("{}", p.billing_interval),
            trial_days: p.trial_days,
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Subscription", free_immediately, size)]
pub struct Subscription {
    id: String,
    subscription_number: String,
    customer_id: String,
    plan_id: String,
    status: String,
    current_period_start: String,
    current_period_end: String,
    trial_end: Option<String>,
    canceled_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl Subscription {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn subscription_number(&self) -> String {
        self.subscription_number.clone()
    }
    fn customer_id(&self) -> String {
        self.customer_id.clone()
    }
    fn plan_id(&self) -> String {
        self.plan_id.clone()
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn current_period_start(&self) -> String {
        self.current_period_start.clone()
    }
    fn current_period_end(&self) -> String {
        self.current_period_end.clone()
    }
    fn trial_end(&self) -> Option<String> {
        self.trial_end.clone()
    }
    fn canceled_at(&self) -> Option<String> {
        self.canceled_at.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Subscription number=\"{}\" status=\"{}\">",
            self.subscription_number, self.status
        )
    }
}

impl From<stateset_core::Subscription> for Subscription {
    fn from(s: stateset_core::Subscription) -> Self {
        Self {
            id: s.id.to_string(),
            subscription_number: s.subscription_number,
            customer_id: s.customer_id.to_string(),
            plan_id: s.plan_id.to_string(),
            status: format!("{}", s.status),
            current_period_start: s.current_period_start.to_rfc3339(),
            current_period_end: s.current_period_end.to_rfc3339(),
            trial_end: s.trial_end.map(|d| d.to_rfc3339()),
            canceled_at: s.canceled_at.map(|d| d.to_rfc3339()),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Subscriptions", free_immediately, size)]
pub struct Subscriptions {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Subscriptions {
    fn create_plan(
        &self,
        code: String,
        name: String,
        price: f64,
        currency: String,
        billing_interval: String,
    ) -> Result<SubscriptionPlan, Error> {
        let commerce = lock_commerce!(self.commerce);
        let plan = commerce
            .subscriptions()
            .create_plan(stateset_core::CreateSubscriptionPlan {
                code,
                name,
                price: Decimal::from_f64_retain(price).unwrap_or_default(),
                currency,
                billing_interval: billing_interval.parse().unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create plan: {}", e),
                )
            })?;
        Ok(plan.into())
    }

    fn get_plan(&self, id: String) -> Result<Option<SubscriptionPlan>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "plan");
        let plan = commerce.subscriptions().get_plan(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get plan: {}", e),
            )
        })?;
        Ok(plan.map(|p| p.into()))
    }

    fn list_plans(&self) -> Result<Vec<SubscriptionPlan>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let plans = commerce
            .subscriptions()
            .list_plans(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list plans: {}", e),
                )
            })?;
        Ok(plans.into_iter().map(|p| p.into()).collect())
    }

    fn subscribe(&self, customer_id: String, plan_id: String) -> Result<Subscription, Error> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let plan_uuid = parse_uuid!(plan_id, "plan");
        let sub = commerce
            .subscriptions()
            .subscribe(stateset_core::CreateSubscription {
                customer_id: cust_uuid,
                plan_id: plan_uuid,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to subscribe: {}", e),
                )
            })?;
        Ok(sub.into())
    }

    fn get(&self, id: String) -> Result<Option<Subscription>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");
        let sub = commerce.subscriptions().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get subscription: {}", e),
            )
        })?;
        Ok(sub.map(|s| s.into()))
    }

    fn list(&self) -> Result<Vec<Subscription>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let subs = commerce
            .subscriptions()
            .list(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list subscriptions: {}", e),
                )
            })?;
        Ok(subs.into_iter().map(|s| s.into()).collect())
    }

    fn pause(&self, id: String) -> Result<Subscription, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");
        let sub = commerce
            .subscriptions()
            .pause(uuid, stateset_core::PauseSubscription::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to pause: {}", e),
                )
            })?;
        Ok(sub.into())
    }

    fn resume(&self, id: String) -> Result<Subscription, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");
        let sub = commerce.subscriptions().resume(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to resume: {}", e),
            )
        })?;
        Ok(sub.into())
    }

    fn cancel(&self, id: String, immediately: bool) -> Result<Subscription, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");
        let sub = commerce
            .subscriptions()
            .cancel(
                uuid,
                stateset_core::CancelSubscription {
                    cancel_immediately: immediately,
                    ..Default::default()
                },
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to cancel: {}", e),
                )
            })?;
        Ok(sub.into())
    }

    fn for_customer(&self, customer_id: String) -> Result<Vec<Subscription>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");
        let subs = commerce
            .subscriptions()
            .get_customer_subscriptions(uuid)
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to get subscriptions: {}", e),
                )
            })?;
        Ok(subs.into_iter().map(|s| s.into()).collect())
    }

    fn is_active(&self, id: String) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");
        let active = commerce.subscriptions().is_active(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to check: {}", e),
            )
        })?;
        Ok(active)
    }
}

// ============================================================================
// Promotions Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Promotion", free_immediately, size)]
pub struct Promotion {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    discount_type: String,
    discount_value: f64,
    min_purchase: Option<f64>,
    max_uses: Option<i32>,
    times_used: i32,
    status: String,
    starts_at: Option<String>,
    ends_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl Promotion {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn code(&self) -> String {
        self.code.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn description(&self) -> Option<String> {
        self.description.clone()
    }
    fn discount_type(&self) -> String {
        self.discount_type.clone()
    }
    fn discount_value(&self) -> f64 {
        self.discount_value
    }
    fn min_purchase(&self) -> Option<f64> {
        self.min_purchase
    }
    fn max_uses(&self) -> Option<i32> {
        self.max_uses
    }
    fn times_used(&self) -> i32 {
        self.times_used
    }
    fn status(&self) -> String {
        self.status.clone()
    }
    fn starts_at(&self) -> Option<String> {
        self.starts_at.clone()
    }
    fn ends_at(&self) -> Option<String> {
        self.ends_at.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
    fn updated_at(&self) -> String {
        self.updated_at.clone()
    }
    fn inspect(&self) -> String {
        format!(
            "#<StateSet::Promotion code=\"{}\" status=\"{}\">",
            self.code, self.status
        )
    }
}

impl From<stateset_core::Promotion> for Promotion {
    fn from(p: stateset_core::Promotion) -> Self {
        Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            discount_type: format!("{}", p.discount_type),
            discount_value: to_f64_or_nan(p.discount_value),
            min_purchase: p.min_purchase.and_then(|m| m.to_f64()),
            max_uses: p.max_uses,
            times_used: p.times_used,
            status: format!("{}", p.status),
            starts_at: p.starts_at.map(|d| d.to_rfc3339()),
            ends_at: p.ends_at.map(|d| d.to_rfc3339()),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Promotions", free_immediately, size)]
pub struct Promotions {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Promotions {
    fn create(
        &self,
        code: String,
        name: String,
        discount_type: String,
        discount_value: f64,
    ) -> Result<Promotion, Error> {
        let commerce = lock_commerce!(self.commerce);
        let promo = commerce
            .promotions()
            .create(stateset_core::CreatePromotion {
                code,
                name,
                discount_type: discount_type.parse().unwrap_or_default(),
                discount_value: Decimal::from_f64_retain(discount_value).unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create promotion: {}", e),
                )
            })?;
        Ok(promo.into())
    }

    fn get(&self, id: String) -> Result<Option<Promotion>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");
        let promo = commerce.promotions().get(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get promotion: {}", e),
            )
        })?;
        Ok(promo.map(|p| p.into()))
    }

    fn get_by_code(&self, code: String) -> Result<Option<Promotion>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let promo = commerce.promotions().get_by_code(&code).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get promotion: {}", e),
            )
        })?;
        Ok(promo.map(|p| p.into()))
    }

    fn list(&self) -> Result<Vec<Promotion>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let promos = commerce
            .promotions()
            .list(Default::default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to list promotions: {}", e),
                )
            })?;
        Ok(promos.into_iter().map(|p| p.into()).collect())
    }

    fn activate(&self, id: String) -> Result<Promotion, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");
        let promo = commerce.promotions().activate(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to activate: {}", e),
            )
        })?;
        Ok(promo.into())
    }

    fn deactivate(&self, id: String) -> Result<Promotion, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");
        let promo = commerce.promotions().deactivate(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to deactivate: {}", e),
            )
        })?;
        Ok(promo.into())
    }

    fn get_active(&self) -> Result<Vec<Promotion>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let promos = commerce.promotions().get_active().map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to get active: {}", e),
            )
        })?;
        Ok(promos.into_iter().map(|p| p.into()).collect())
    }

    fn is_valid(&self, id: String) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");
        let valid = commerce.promotions().is_valid(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to check: {}", e),
            )
        })?;
        Ok(valid)
    }

    fn delete(&self, id: String) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");
        commerce.promotions().delete(uuid).map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to delete: {}", e),
            )
        })?;
        Ok(true)
    }
}

// ============================================================================
// Tax Types & API
// ============================================================================

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::TaxJurisdiction", free_immediately, size)]
pub struct TaxJurisdiction {
    id: String,
    code: String,
    name: String,
    country: String,
    state: Option<String>,
    created_at: String,
}

impl TaxJurisdiction {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn code(&self) -> String {
        self.code.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn country(&self) -> String {
        self.country.clone()
    }
    fn state(&self) -> Option<String> {
        self.state.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<stateset_core::TaxJurisdiction> for TaxJurisdiction {
    fn from(j: stateset_core::TaxJurisdiction) -> Self {
        Self {
            id: j.id.to_string(),
            code: j.code,
            name: j.name,
            country: j.country,
            state: j.state,
            created_at: j.created_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::TaxRate", free_immediately, size)]
pub struct TaxRate {
    id: String,
    jurisdiction_id: String,
    name: String,
    rate: f64,
    category: String,
    created_at: String,
}

impl TaxRate {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn jurisdiction_id(&self) -> String {
        self.jurisdiction_id.clone()
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    fn rate(&self) -> f64 {
        self.rate
    }
    fn category(&self) -> String {
        self.category.clone()
    }
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<stateset_core::TaxRate> for TaxRate {
    fn from(r: stateset_core::TaxRate) -> Self {
        Self {
            id: r.id.to_string(),
            jurisdiction_id: r.jurisdiction_id.to_string(),
            name: r.name,
            rate: to_f64_or_nan(r.rate),
            category: format!("{}", r.category),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[derive(Clone)]
#[magnus::wrap(class = "StateSet::Tax", free_immediately, size)]
pub struct Tax {
    commerce: Arc<Mutex<RustCommerce>>,
}

impl Tax {
    fn calculate(
        &self,
        unit_price: f64,
        quantity: f64,
        category: String,
        country: String,
        state: Option<String>,
    ) -> Result<f64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let address = stateset_core::TaxAddress {
            country,
            state,
            city: None,
            postal_code: None,
            line1: None,
        };
        let result = commerce
            .tax()
            .calculate_for_item(
                Decimal::from_f64_retain(unit_price).unwrap_or_default(),
                Decimal::from_f64_retain(quantity).unwrap_or_default(),
                category.parse().unwrap_or_default(),
                &address,
            )
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to calculate: {}", e),
                )
            })?;
        Ok(to_f64_or_nan(result))
    }

    fn get_effective_rate(
        &self,
        category: String,
        country: String,
        state: Option<String>,
    ) -> Result<f64, Error> {
        let commerce = lock_commerce!(self.commerce);
        let address = stateset_core::TaxAddress {
            country,
            state,
            city: None,
            postal_code: None,
            line1: None,
        };
        let rate = commerce
            .tax()
            .get_effective_rate(&address, category.parse().unwrap_or_default())
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to get rate: {}", e),
                )
            })?;
        Ok(to_f64_or_nan(rate))
    }

    fn list_jurisdictions(&self) -> Result<Vec<TaxJurisdiction>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let jurisdictions = commerce
            .tax()
            .list_jurisdictions(Default::default())
            .map_err(|e| {
                Error::new(exception::runtime_error(), format!("Failed to list: {}", e))
            })?;
        Ok(jurisdictions.into_iter().map(|j| j.into()).collect())
    }

    fn create_jurisdiction(
        &self,
        code: String,
        name: String,
        country: String,
        state: Option<String>,
    ) -> Result<TaxJurisdiction, Error> {
        let commerce = lock_commerce!(self.commerce);
        let j = commerce
            .tax()
            .create_jurisdiction(stateset_core::CreateTaxJurisdiction {
                code,
                name,
                country,
                state,
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create: {}", e),
                )
            })?;
        Ok(j.into())
    }

    fn list_rates(&self) -> Result<Vec<TaxRate>, Error> {
        let commerce = lock_commerce!(self.commerce);
        let rates = commerce.tax().list_rates(Default::default()).map_err(|e| {
            Error::new(exception::runtime_error(), format!("Failed to list: {}", e))
        })?;
        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    fn create_rate(
        &self,
        jurisdiction_id: String,
        name: String,
        rate: f64,
        category: String,
    ) -> Result<TaxRate, Error> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(jurisdiction_id, "jurisdiction");
        let r = commerce
            .tax()
            .create_rate(stateset_core::CreateTaxRate {
                jurisdiction_id: uuid,
                name,
                rate: Decimal::from_f64_retain(rate).unwrap_or_default(),
                category: category.parse().unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|e| {
                Error::new(
                    exception::runtime_error(),
                    format!("Failed to create: {}", e),
                )
            })?;
        Ok(r.into())
    }

    fn is_enabled(&self) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        let enabled = commerce.tax().is_enabled().map_err(|e| {
            Error::new(
                exception::runtime_error(),
                format!("Failed to check: {}", e),
            )
        })?;
        Ok(enabled)
    }

    fn set_enabled(&self, enabled: bool) -> Result<bool, Error> {
        let commerce = lock_commerce!(self.commerce);
        commerce
            .tax()
            .set_enabled(enabled)
            .map_err(|e| Error::new(exception::runtime_error(), format!("Failed to set: {}", e)))?;
        Ok(enabled)
    }
}

// ============================================================================
// Module Initialization
// ============================================================================

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("StateSet")?;

    // Commerce
    let commerce_class = module.define_class("Commerce", ruby.class_object())?;
    commerce_class.define_singleton_method("new", function!(Commerce::new, 1))?;
    commerce_class.define_method("customers", method!(Commerce::customers, 0))?;
    commerce_class.define_method("orders", method!(Commerce::orders, 0))?;
    commerce_class.define_method("products", method!(Commerce::products, 0))?;
    commerce_class.define_method("inventory", method!(Commerce::inventory, 0))?;
    commerce_class.define_method("returns", method!(Commerce::returns, 0))?;
    commerce_class.define_method("payments", method!(Commerce::payments, 0))?;
    commerce_class.define_method("shipments", method!(Commerce::shipments, 0))?;
    commerce_class.define_method("warranties", method!(Commerce::warranties, 0))?;
    commerce_class.define_method("purchase_orders", method!(Commerce::purchase_orders, 0))?;
    commerce_class.define_method("invoices", method!(Commerce::invoices, 0))?;
    commerce_class.define_method("bom", method!(Commerce::bom, 0))?;
    commerce_class.define_method("work_orders", method!(Commerce::work_orders, 0))?;
    commerce_class.define_method("carts", method!(Commerce::carts, 0))?;
    commerce_class.define_method("analytics", method!(Commerce::analytics, 0))?;
    commerce_class.define_method("currency", method!(Commerce::currency, 0))?;
    commerce_class.define_method("subscriptions", method!(Commerce::subscriptions, 0))?;
    commerce_class.define_method("promotions", method!(Commerce::promotions, 0))?;
    commerce_class.define_method("tax", method!(Commerce::tax, 0))?;

    // Customer
    let customer_class = module.define_class("Customer", ruby.class_object())?;
    customer_class.define_method("id", method!(Customer::id, 0))?;
    customer_class.define_method("email", method!(Customer::email, 0))?;
    customer_class.define_method("first_name", method!(Customer::first_name, 0))?;
    customer_class.define_method("last_name", method!(Customer::last_name, 0))?;
    customer_class.define_method("phone", method!(Customer::phone, 0))?;
    customer_class.define_method("status", method!(Customer::status, 0))?;
    customer_class.define_method("accepts_marketing", method!(Customer::accepts_marketing, 0))?;
    customer_class.define_method("created_at", method!(Customer::created_at, 0))?;
    customer_class.define_method("updated_at", method!(Customer::updated_at, 0))?;
    customer_class.define_method("full_name", method!(Customer::full_name, 0))?;
    customer_class.define_method("inspect", method!(Customer::inspect, 0))?;
    customer_class.define_method("to_s", method!(Customer::inspect, 0))?;

    // Customers API
    let customers_class = module.define_class("Customers", ruby.class_object())?;
    customers_class.define_method("create", method!(Customers::create, 5))?;
    customers_class.define_method("get", method!(Customers::get, 1))?;
    customers_class.define_method("get_by_email", method!(Customers::get_by_email, 1))?;
    customers_class.define_method("list", method!(Customers::list, 0))?;
    customers_class.define_method("count", method!(Customers::count, 0))?;

    // OrderItem
    let order_item_class = module.define_class("OrderItem", ruby.class_object())?;
    order_item_class.define_method("id", method!(OrderItem::id, 0))?;
    order_item_class.define_method("sku", method!(OrderItem::sku, 0))?;
    order_item_class.define_method("name", method!(OrderItem::name, 0))?;
    order_item_class.define_method("quantity", method!(OrderItem::quantity, 0))?;
    order_item_class.define_method("unit_price", method!(OrderItem::unit_price, 0))?;
    order_item_class.define_method("total", method!(OrderItem::total, 0))?;
    order_item_class.define_method("inspect", method!(OrderItem::inspect, 0))?;

    // Order
    let order_class = module.define_class("Order", ruby.class_object())?;
    order_class.define_method("id", method!(Order::id, 0))?;
    order_class.define_method("order_number", method!(Order::order_number, 0))?;
    order_class.define_method("customer_id", method!(Order::customer_id, 0))?;
    order_class.define_method("status", method!(Order::status, 0))?;
    order_class.define_method("total_amount", method!(Order::total_amount, 0))?;
    order_class.define_method("currency", method!(Order::currency, 0))?;
    order_class.define_method("payment_status", method!(Order::payment_status, 0))?;
    order_class.define_method("fulfillment_status", method!(Order::fulfillment_status, 0))?;
    order_class.define_method("tracking_number", method!(Order::tracking_number, 0))?;
    order_class.define_method("items", method!(Order::items, 0))?;
    order_class.define_method("version", method!(Order::version, 0))?;
    order_class.define_method("created_at", method!(Order::created_at, 0))?;
    order_class.define_method("updated_at", method!(Order::updated_at, 0))?;
    order_class.define_method("item_count", method!(Order::item_count, 0))?;
    order_class.define_method("inspect", method!(Order::inspect, 0))?;
    order_class.define_method("to_s", method!(Order::inspect, 0))?;

    // Orders API
    let orders_class = module.define_class("Orders", ruby.class_object())?;
    orders_class.define_method("create", method!(Orders::create, 4))?;
    orders_class.define_method("get", method!(Orders::get, 1))?;
    orders_class.define_method("list", method!(Orders::list, 0))?;
    orders_class.define_method("count", method!(Orders::count, 0))?;
    orders_class.define_method("ship", method!(Orders::ship, 3))?;
    orders_class.define_method("cancel", method!(Orders::cancel, 2))?;
    orders_class.define_method("confirm", method!(Orders::confirm, 1))?;
    orders_class.define_method("deliver", method!(Orders::deliver, 1))?;

    // ProductVariant
    let variant_class = module.define_class("ProductVariant", ruby.class_object())?;
    variant_class.define_method("id", method!(ProductVariant::id, 0))?;
    variant_class.define_method("sku", method!(ProductVariant::sku, 0))?;
    variant_class.define_method("name", method!(ProductVariant::name, 0))?;
    variant_class.define_method("price", method!(ProductVariant::price, 0))?;
    variant_class.define_method(
        "compare_at_price",
        method!(ProductVariant::compare_at_price, 0),
    )?;
    variant_class.define_method(
        "inventory_quantity",
        method!(ProductVariant::inventory_quantity, 0),
    )?;
    variant_class.define_method("weight", method!(ProductVariant::weight, 0))?;
    variant_class.define_method("barcode", method!(ProductVariant::barcode, 0))?;

    // Product
    let product_class = module.define_class("Product", ruby.class_object())?;
    product_class.define_method("id", method!(Product::id, 0))?;
    product_class.define_method("name", method!(Product::name, 0))?;
    product_class.define_method("description", method!(Product::description, 0))?;
    product_class.define_method("vendor", method!(Product::vendor, 0))?;
    product_class.define_method("product_type", method!(Product::product_type, 0))?;
    product_class.define_method("status", method!(Product::status, 0))?;
    product_class.define_method("tags", method!(Product::tags, 0))?;
    product_class.define_method("variants", method!(Product::variants, 0))?;
    product_class.define_method("created_at", method!(Product::created_at, 0))?;
    product_class.define_method("updated_at", method!(Product::updated_at, 0))?;
    product_class.define_method("inspect", method!(Product::inspect, 0))?;
    product_class.define_method("to_s", method!(Product::inspect, 0))?;

    // Products API
    let products_class = module.define_class("Products", ruby.class_object())?;
    products_class.define_method("create", method!(Products::create, 4))?;
    products_class.define_method("get", method!(Products::get, 1))?;
    products_class.define_method("list", method!(Products::list, 0))?;
    products_class.define_method("count", method!(Products::count, 0))?;
    products_class.define_method("get_by_sku", method!(Products::get_by_sku, 1))?;

    // InventoryItem
    let inv_item_class = module.define_class("InventoryItem", ruby.class_object())?;
    inv_item_class.define_method("id", method!(InventoryItem::id, 0))?;
    inv_item_class.define_method("sku", method!(InventoryItem::sku, 0))?;
    inv_item_class.define_method(
        "quantity_on_hand",
        method!(InventoryItem::quantity_on_hand, 0),
    )?;
    inv_item_class.define_method(
        "quantity_reserved",
        method!(InventoryItem::quantity_reserved, 0),
    )?;
    inv_item_class.define_method(
        "quantity_available",
        method!(InventoryItem::quantity_available, 0),
    )?;
    inv_item_class.define_method("reorder_point", method!(InventoryItem::reorder_point, 0))?;
    inv_item_class.define_method(
        "reorder_quantity",
        method!(InventoryItem::reorder_quantity, 0),
    )?;
    inv_item_class.define_method("location_id", method!(InventoryItem::location_id, 0))?;
    inv_item_class.define_method("inspect", method!(InventoryItem::inspect, 0))?;
    inv_item_class.define_method("to_s", method!(InventoryItem::inspect, 0))?;

    // Inventory API
    let inventory_class = module.define_class("Inventory", ruby.class_object())?;
    inventory_class.define_method("create", method!(Inventory::create, 4))?;
    inventory_class.define_method("get", method!(Inventory::get, 1))?;
    inventory_class.define_method("get_by_sku", method!(Inventory::get_by_sku, 1))?;
    inventory_class.define_method("list", method!(Inventory::list, 0))?;
    inventory_class.define_method("adjust", method!(Inventory::adjust, 3))?;
    inventory_class.define_method("reserve", method!(Inventory::reserve, 3))?;
    inventory_class.define_method("release", method!(Inventory::release, 2))?;

    // Return
    let return_class = module.define_class("Return", ruby.class_object())?;
    return_class.define_method("id", method!(Return::id, 0))?;
    return_class.define_method("order_id", method!(Return::order_id, 0))?;
    return_class.define_method("customer_id", method!(Return::customer_id, 0))?;
    return_class.define_method("status", method!(Return::status, 0))?;
    return_class.define_method("reason", method!(Return::reason, 0))?;
    return_class.define_method("refund_amount", method!(Return::refund_amount, 0))?;
    return_class.define_method("created_at", method!(Return::created_at, 0))?;
    return_class.define_method("updated_at", method!(Return::updated_at, 0))?;
    return_class.define_method("inspect", method!(Return::inspect, 0))?;
    return_class.define_method("to_s", method!(Return::inspect, 0))?;

    // Returns API
    let returns_class = module.define_class("Returns", ruby.class_object())?;
    returns_class.define_method("create", method!(Returns::create, 2))?;
    returns_class.define_method("get", method!(Returns::get, 1))?;
    returns_class.define_method("list", method!(Returns::list, 0))?;
    returns_class.define_method("approve", method!(Returns::approve, 2))?;
    returns_class.define_method("reject", method!(Returns::reject, 2))?;

    // Payments API
    let payments_class = module.define_class("Payments", ruby.class_object())?;
    payments_class.define_method("record", method!(Payments::record, 3))?;

    // Shipments API (stub)
    let _shipments_class = module.define_class("Shipments", ruby.class_object())?;

    // Warranties API (stub)
    let _warranties_class = module.define_class("Warranties", ruby.class_object())?;

    // PurchaseOrders API (stub)
    let _po_class = module.define_class("PurchaseOrders", ruby.class_object())?;

    // Invoices API (stub)
    let _invoices_class = module.define_class("Invoices", ruby.class_object())?;

    // BomApi (stub)
    let _bom_class = module.define_class("BomApi", ruby.class_object())?;

    // WorkOrders API (stub)
    let _wo_class = module.define_class("WorkOrders", ruby.class_object())?;

    // CartItem
    let cart_item_class = module.define_class("CartItem", ruby.class_object())?;
    cart_item_class.define_method("id", method!(CartItem::id, 0))?;
    cart_item_class.define_method("sku", method!(CartItem::sku, 0))?;
    cart_item_class.define_method("name", method!(CartItem::name, 0))?;
    cart_item_class.define_method("quantity", method!(CartItem::quantity, 0))?;
    cart_item_class.define_method("unit_price", method!(CartItem::unit_price, 0))?;
    cart_item_class.define_method("total", method!(CartItem::total, 0))?;

    // Cart
    let cart_class = module.define_class("Cart", ruby.class_object())?;
    cart_class.define_method("id", method!(Cart::id, 0))?;
    cart_class.define_method("customer_id", method!(Cart::customer_id, 0))?;
    cart_class.define_method("status", method!(Cart::status, 0))?;
    cart_class.define_method("items", method!(Cart::items, 0))?;
    cart_class.define_method("subtotal", method!(Cart::subtotal, 0))?;
    cart_class.define_method("total", method!(Cart::total, 0))?;
    cart_class.define_method("currency", method!(Cart::currency, 0))?;
    cart_class.define_method("created_at", method!(Cart::created_at, 0))?;
    cart_class.define_method("updated_at", method!(Cart::updated_at, 0))?;
    cart_class.define_method("inspect", method!(Cart::inspect, 0))?;
    cart_class.define_method("to_s", method!(Cart::inspect, 0))?;

    // Carts API
    let carts_class = module.define_class("Carts", ruby.class_object())?;
    carts_class.define_method("create", method!(Carts::create, 2))?;
    carts_class.define_method("get", method!(Carts::get, 1))?;
    carts_class.define_method("list", method!(Carts::list, 0))?;
    carts_class.define_method("add_item", method!(Carts::add_item, 5))?;
    carts_class.define_method("checkout", method!(Carts::checkout, 1))?;

    // SalesSummary
    let sales_class = module.define_class("SalesSummary", ruby.class_object())?;
    sales_class.define_method("total_revenue", method!(SalesSummary::total_revenue, 0))?;
    sales_class.define_method("total_orders", method!(SalesSummary::total_orders, 0))?;
    sales_class.define_method(
        "average_order_value",
        method!(SalesSummary::average_order_value, 0),
    )?;
    sales_class.define_method(
        "total_items_sold",
        method!(SalesSummary::total_items_sold, 0),
    )?;

    // Analytics API
    let analytics_class = module.define_class("Analytics", ruby.class_object())?;
    analytics_class.define_method("sales_summary", method!(Analytics::sales_summary, 1))?;

    // CurrencyOps API (stub)
    let _currency_class = module.define_class("CurrencyOps", ruby.class_object())?;

    // Subscriptions API (stub)
    let _subs_class = module.define_class("Subscriptions", ruby.class_object())?;

    // Promotions API (stub)
    let _promos_class = module.define_class("Promotions", ruby.class_object())?;

    // Tax API (stub)
    let _tax_class = module.define_class("Tax", ruby.class_object())?;

    Ok(())
}
