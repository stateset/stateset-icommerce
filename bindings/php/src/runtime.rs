//! PHP bindings for StateSet Embedded Commerce
//!
//! Provides a local-first commerce library with SQLite storage for PHP.
//!
//! ```php
//! <?php
//! use StateSet\Commerce;
//!
//! $commerce = new Commerce("./store.db");
//! $customer = $commerce->customers()->create("alice@example.com", "Alice", "Smith");
//! ```

use ext_php_rs::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use stateset_embedded::Commerce as RustCommerce;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Macros
// ============================================================================

macro_rules! lock_commerce {
    ($commerce:expr) => {
        $commerce.lock().map_err(|e| PhpException::default(format!("Lock error: {}", e)))?
    };
}

macro_rules! parse_uuid {
    ($id:expr, $name:expr) => {
        $id.parse().map_err(|_| PhpException::default(format!("Invalid {} UUID", $name)))?
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

fn to_f64_result<T>(value: T, field: &str) -> PhpResult<f64>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    value.try_into().map_err(|err| {
        PhpException::default(format!("Failed to convert {} to float: {}", field, err))
    })
}

fn decimal_from_f64(value: f64, field: &str) -> PhpResult<Decimal> {
    Decimal::from_f64_retain(value).ok_or_else(|| {
        PhpException::default(format!("Invalid numeric value for {}: {}", field, value))
    })
}

fn optional_decimal_from_f64(value: Option<f64>, field: &str) -> PhpResult<Option<Decimal>> {
    value.map(|inner| decimal_from_f64(inner, field)).transpose()
}

// ============================================================================
// Commerce
// ============================================================================

/// Main Commerce instance for local commerce operations.
#[php_class(name = "StateSet\\Commerce")]
#[derive(Clone)]
pub struct Commerce {
    inner: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Commerce {
    pub fn __construct(db_path: String) -> PhpResult<Self> {
        let commerce = RustCommerce::new(&db_path)
            .map_err(|e| PhpException::default(format!("Failed to initialize commerce: {}", e)))?;

        Ok(Self { inner: Arc::new(Mutex::new(commerce)) })
    }

    pub fn customers(&self) -> Customers {
        Customers { commerce: self.inner.clone() }
    }

    pub fn orders(&self) -> Orders {
        Orders { commerce: self.inner.clone() }
    }

    pub fn products(&self) -> Products {
        Products { commerce: self.inner.clone() }
    }

    pub fn inventory(&self) -> Inventory {
        Inventory { commerce: self.inner.clone() }
    }

    pub fn returns(&self) -> Returns {
        Returns { commerce: self.inner.clone() }
    }

    pub fn payments(&self) -> Payments {
        Payments { commerce: self.inner.clone() }
    }

    pub fn shipments(&self) -> Shipments {
        Shipments { commerce: self.inner.clone() }
    }

    pub fn warranties(&self) -> Warranties {
        Warranties { commerce: self.inner.clone() }
    }

    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders { commerce: self.inner.clone() }
    }

    pub fn invoices(&self) -> Invoices {
        Invoices { commerce: self.inner.clone() }
    }

    pub fn bom(&self) -> BomApi {
        BomApi { commerce: self.inner.clone() }
    }

    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders { commerce: self.inner.clone() }
    }

    pub fn carts(&self) -> Carts {
        Carts { commerce: self.inner.clone() }
    }

    pub fn analytics(&self) -> Analytics {
        Analytics { commerce: self.inner.clone() }
    }

    pub fn currency(&self) -> CurrencyOps {
        CurrencyOps { commerce: self.inner.clone() }
    }

    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions { commerce: self.inner.clone() }
    }

    pub fn promotions(&self) -> Promotions {
        Promotions { commerce: self.inner.clone() }
    }

    pub fn tax(&self) -> Tax {
        Tax { commerce: self.inner.clone() }
    }

    pub fn quality(&self) -> Quality {
        Quality { commerce: self.inner.clone() }
    }

    pub fn lots(&self) -> Lots {
        Lots { commerce: self.inner.clone() }
    }

    pub fn serials(&self) -> Serials {
        Serials { commerce: self.inner.clone() }
    }

    pub fn warehouse(&self) -> WarehouseApi {
        WarehouseApi { commerce: self.inner.clone() }
    }

    pub fn receiving(&self) -> Receiving {
        Receiving { commerce: self.inner.clone() }
    }

    pub fn fulfillment(&self) -> Fulfillment {
        Fulfillment { commerce: self.inner.clone() }
    }

    pub fn accounts_payable(&self) -> AccountsPayable {
        AccountsPayable { commerce: self.inner.clone() }
    }

    pub fn accounts_receivable(&self) -> AccountsReceivable {
        AccountsReceivable { commerce: self.inner.clone() }
    }

    pub fn cost_accounting(&self) -> CostAccounting {
        CostAccounting { commerce: self.inner.clone() }
    }

    pub fn credit(&self) -> CreditApi {
        CreditApi { commerce: self.inner.clone() }
    }

    pub fn backorders(&self) -> Backorders {
        Backorders { commerce: self.inner.clone() }
    }

    pub fn general_ledger(&self) -> GeneralLedger {
        GeneralLedger { commerce: self.inner.clone() }
    }
}

// ============================================================================
// Customer Types
// ============================================================================

#[php_class(name = "StateSet\\Customer")]
#[derive(Clone)]
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

#[php_impl]
impl Customer {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_email(&self) -> String {
        self.email.clone()
    }

    #[getter]
    pub fn get_first_name(&self) -> String {
        self.first_name.clone()
    }

    #[getter]
    pub fn get_last_name(&self) -> String {
        self.last_name.clone()
    }

    #[getter]
    pub fn get_phone(&self) -> Option<String> {
        self.phone.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_accepts_marketing(&self) -> bool {
        self.accepts_marketing
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn get_full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    pub fn __to_string(&self) -> String {
        format!(
            "Customer(id={}, email={}, name={} {})",
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

#[php_class(name = "StateSet\\Customers")]
#[derive(Clone)]
pub struct Customers {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Customers {
    pub fn create(
        &self,
        email: String,
        first_name: String,
        last_name: String,
        phone: Option<String>,
        accepts_marketing: Option<bool>,
    ) -> PhpResult<Customer> {
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
            .map_err(|e| PhpException::default(format!("Failed to create customer: {}", e)))?;

        Ok(customer.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Customer>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "customer");

        let customer = commerce
            .customers()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get customer: {}", e)))?;

        Ok(customer.map(|c| c.into()))
    }

    pub fn get_by_email(&self, email: String) -> PhpResult<Option<Customer>> {
        let commerce = lock_commerce!(self.commerce);

        let customer = commerce
            .customers()
            .get_by_email(&email)
            .map_err(|e| PhpException::default(format!("Failed to get customer: {}", e)))?;

        Ok(customer.map(|c| c.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Customer>> {
        let commerce = lock_commerce!(self.commerce);

        let customers = commerce
            .customers()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list customers: {}", e)))?;

        Ok(customers.into_iter().map(|c| c.into()).collect())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .customers()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count customers: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// Order Types
// ============================================================================

#[php_class(name = "StateSet\\OrderItem")]
#[derive(Clone)]
pub struct OrderItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

#[php_impl]
impl OrderItem {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_sku(&self) -> String {
        self.sku.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_quantity(&self) -> i32 {
        self.quantity
    }

    #[getter]
    pub fn get_unit_price(&self) -> f64 {
        self.unit_price
    }

    #[getter]
    pub fn get_total(&self) -> f64 {
        self.total
    }

    pub fn __to_string(&self) -> String {
        format!("OrderItem(sku={}, qty={}, price={})", self.sku, self.quantity, self.unit_price)
    }
}

#[php_class(name = "StateSet\\Order")]
#[derive(Clone)]
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

#[php_impl]
impl Order {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_order_number(&self) -> String {
        self.order_number.clone()
    }

    #[getter]
    pub fn get_customer_id(&self) -> String {
        self.customer_id.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_total_amount(&self) -> f64 {
        self.total_amount
    }

    #[getter]
    pub fn get_currency(&self) -> String {
        self.currency.clone()
    }

    #[getter]
    pub fn get_payment_status(&self) -> String {
        self.payment_status.clone()
    }

    #[getter]
    pub fn get_fulfillment_status(&self) -> String {
        self.fulfillment_status.clone()
    }

    #[getter]
    pub fn get_tracking_number(&self) -> Option<String> {
        self.tracking_number.clone()
    }

    #[getter]
    pub fn get_items(&self) -> Vec<OrderItem> {
        self.items.clone()
    }

    #[getter]
    pub fn get_version(&self) -> i32 {
        self.version
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn get_item_count(&self) -> usize {
        self.items.len()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "Order(number={}, status={}, total={} {})",
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

#[php_class(name = "StateSet\\Orders")]
#[derive(Clone)]
pub struct Orders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Orders {
    pub fn create(
        &self,
        customer_id: String,
        items: Vec<ext_php_rs::types::ZendHashTable>,
        currency: Option<String>,
        notes: Option<String>,
    ) -> PhpResult<Order> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");

        let order_items: Vec<stateset_core::CreateOrderItem> = items
            .into_iter()
            .map(|h| {
                let sku: String = h.get("sku").and_then(|v| v.string().ok()).unwrap_or_default();
                let name: String = h.get("name").and_then(|v| v.string().ok()).unwrap_or_default();
                let quantity: i32 =
                    h.get("quantity").and_then(|v| v.long().ok()).map(|l| l as i32).unwrap_or(1);
                let unit_price: f64 =
                    h.get("unit_price").and_then(|v| v.double().ok()).unwrap_or(0.0);

                Ok(stateset_core::CreateOrderItem {
                    product_id: Default::default(),
                    variant_id: None,
                    sku,
                    name,
                    quantity,
                    unit_price: decimal_from_f64(unit_price, "unit_price")?,
                    ..Default::default()
                })
            })
            .collect::<PhpResult<Vec<_>>>()?;

        let order = commerce
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id: cust_uuid,
                items: order_items,
                currency,
                notes,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create order: {}", e)))?;

        Ok(order.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Order>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce
            .orders()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get order: {}", e)))?;

        Ok(order.map(|o| o.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Order>> {
        let commerce = lock_commerce!(self.commerce);

        let orders = commerce
            .orders()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list orders: {}", e)))?;

        Ok(orders.into_iter().map(|o| o.into()).collect())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .orders()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count orders: {}", e)))?;

        Ok(count)
    }

    pub fn ship(
        &self,
        id: String,
        tracking_number: Option<String>,
        carrier: Option<String>,
    ) -> PhpResult<Order> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce
            .orders()
            .ship(uuid, tracking_number, carrier)
            .map_err(|e| PhpException::default(format!("Failed to ship order: {}", e)))?;

        Ok(order.into())
    }

    pub fn cancel(&self, id: String, reason: Option<String>) -> PhpResult<Order> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce
            .orders()
            .cancel(uuid, reason)
            .map_err(|e| PhpException::default(format!("Failed to cancel order: {}", e)))?;

        Ok(order.into())
    }

    pub fn confirm(&self, id: String) -> PhpResult<Order> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce
            .orders()
            .confirm(uuid)
            .map_err(|e| PhpException::default(format!("Failed to confirm order: {}", e)))?;

        Ok(order.into())
    }

    pub fn deliver(&self, id: String) -> PhpResult<Order> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "order");

        let order = commerce
            .orders()
            .deliver(uuid)
            .map_err(|e| PhpException::default(format!("Failed to deliver order: {}", e)))?;

        Ok(order.into())
    }
}

// ============================================================================
// Product Types
// ============================================================================

#[php_class(name = "StateSet\\ProductVariant")]
#[derive(Clone)]
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

#[php_impl]
impl ProductVariant {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_sku(&self) -> String {
        self.sku.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_price(&self) -> f64 {
        self.price
    }

    #[getter]
    pub fn get_compare_at_price(&self) -> Option<f64> {
        self.compare_at_price
    }

    #[getter]
    pub fn get_inventory_quantity(&self) -> i32 {
        self.inventory_quantity
    }

    #[getter]
    pub fn get_weight(&self) -> Option<f64> {
        self.weight
    }

    #[getter]
    pub fn get_barcode(&self) -> Option<String> {
        self.barcode.clone()
    }
}

#[php_class(name = "StateSet\\Product")]
#[derive(Clone)]
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

#[php_impl]
impl Product {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_description(&self) -> Option<String> {
        self.description.clone()
    }

    #[getter]
    pub fn get_vendor(&self) -> Option<String> {
        self.vendor.clone()
    }

    #[getter]
    pub fn get_product_type(&self) -> Option<String> {
        self.product_type.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_tags(&self) -> Vec<String> {
        self.tags.clone()
    }

    #[getter]
    pub fn get_variants(&self) -> Vec<ProductVariant> {
        self.variants.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!("Product(id={}, name={}, status={})", self.id, self.name, self.status)
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

#[php_class(name = "StateSet\\Products")]
#[derive(Clone)]
pub struct Products {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Products {
    pub fn create(
        &self,
        name: String,
        description: Option<String>,
        vendor: Option<String>,
        product_type: Option<String>,
    ) -> PhpResult<Product> {
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
            .map_err(|e| PhpException::default(format!("Failed to create product: {}", e)))?;

        Ok(product.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Product>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "product");

        let product = commerce
            .products()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get product: {}", e)))?;

        Ok(product.map(|p| p.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Product>> {
        let commerce = lock_commerce!(self.commerce);

        let products = commerce
            .products()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list products: {}", e)))?;

        Ok(products.into_iter().map(|p| p.into()).collect())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .products()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count products: {}", e)))?;

        Ok(count)
    }

    pub fn get_by_sku(&self, sku: String) -> PhpResult<Option<ProductVariant>> {
        let commerce = lock_commerce!(self.commerce);

        let variant = commerce
            .products()
            .get_variant_by_sku(&sku)
            .map_err(|e| PhpException::default(format!("Failed to get variant: {}", e)))?;

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

#[php_class(name = "StateSet\\InventoryItem")]
#[derive(Clone)]
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

#[php_impl]
impl InventoryItem {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_sku(&self) -> String {
        self.sku.clone()
    }

    #[getter]
    pub fn get_quantity_on_hand(&self) -> i32 {
        self.quantity_on_hand
    }

    #[getter]
    pub fn get_quantity_reserved(&self) -> i32 {
        self.quantity_reserved
    }

    #[getter]
    pub fn get_quantity_available(&self) -> i32 {
        self.quantity_available
    }

    #[getter]
    pub fn get_reorder_point(&self) -> Option<i32> {
        self.reorder_point
    }

    #[getter]
    pub fn get_reorder_quantity(&self) -> Option<i32> {
        self.reorder_quantity
    }

    #[getter]
    pub fn get_location_id(&self) -> Option<String> {
        self.location_id.clone()
    }

    pub fn __to_string(&self) -> String {
        format!("InventoryItem(sku={}, available={})", self.sku, self.quantity_available)
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

#[php_class(name = "StateSet\\Inventory")]
#[derive(Clone)]
pub struct Inventory {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Inventory {
    pub fn create(
        &self,
        sku: String,
        quantity: i32,
        reorder_point: Option<i32>,
        reorder_quantity: Option<i32>,
    ) -> PhpResult<InventoryItem> {
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
            .map_err(|e| PhpException::default(format!("Failed to create inventory: {}", e)))?;

        Ok(item.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<InventoryItem>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");

        let item = commerce
            .inventory()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get inventory: {}", e)))?;

        Ok(item.map(|i| i.into()))
    }

    pub fn get_by_sku(&self, sku: String) -> PhpResult<Option<InventoryItem>> {
        let commerce = lock_commerce!(self.commerce);

        let item = commerce
            .inventory()
            .get_by_sku(&sku)
            .map_err(|e| PhpException::default(format!("Failed to get inventory: {}", e)))?;

        Ok(item.map(|i| i.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<InventoryItem>> {
        let commerce = lock_commerce!(self.commerce);

        let items = commerce
            .inventory()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list inventory: {}", e)))?;

        Ok(items.into_iter().map(|i| i.into()).collect())
    }

    pub fn adjust(
        &self,
        id: String,
        adjustment: i32,
        reason: Option<String>,
    ) -> PhpResult<InventoryItem> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");

        let item = commerce
            .inventory()
            .adjust(uuid, adjustment, reason)
            .map_err(|e| PhpException::default(format!("Failed to adjust inventory: {}", e)))?;

        Ok(item.into())
    }

    pub fn reserve(
        &self,
        id: String,
        quantity: i32,
        order_id: Option<String>,
    ) -> PhpResult<InventoryItem> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");
        let order_uuid = order_id.and_then(|s| s.parse().ok());

        let item = commerce
            .inventory()
            .reserve(uuid, quantity, order_uuid)
            .map_err(|e| PhpException::default(format!("Failed to reserve inventory: {}", e)))?;

        Ok(item.into())
    }

    pub fn release(&self, id: String, quantity: i32) -> PhpResult<InventoryItem> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "inventory");

        let item = commerce
            .inventory()
            .release(uuid, quantity)
            .map_err(|e| PhpException::default(format!("Failed to release inventory: {}", e)))?;

        Ok(item.into())
    }
}

// ============================================================================
// Returns Types & API
// ============================================================================

#[php_class(name = "StateSet\\ReturnRequest")]
#[derive(Clone)]
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

#[php_impl]
impl Return {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_order_id(&self) -> String {
        self.order_id.clone()
    }

    #[getter]
    pub fn get_customer_id(&self) -> String {
        self.customer_id.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_reason(&self) -> String {
        self.reason.clone()
    }

    #[getter]
    pub fn get_refund_amount(&self) -> f64 {
        self.refund_amount
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!("Return(id={}, status={}, refund={})", self.id, self.status, self.refund_amount)
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

#[php_class(name = "StateSet\\Returns")]
#[derive(Clone)]
pub struct Returns {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Returns {
    pub fn create(&self, order_id: String, reason: String) -> PhpResult<Return> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");

        let ret = commerce
            .returns()
            .create(stateset_core::CreateReturn { order_id: uuid, reason, ..Default::default() })
            .map_err(|e| PhpException::default(format!("Failed to create return: {}", e)))?;

        Ok(ret.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Return>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "return");

        let ret = commerce
            .returns()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get return: {}", e)))?;

        Ok(ret.map(|r| r.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Return>> {
        let commerce = lock_commerce!(self.commerce);

        let returns = commerce
            .returns()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list returns: {}", e)))?;

        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    pub fn approve(&self, id: String, refund_amount: Option<f64>) -> PhpResult<Return> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "return");
        let amount = optional_decimal_from_f64(refund_amount, "refund_amount")?;

        let ret = commerce
            .returns()
            .approve(uuid, amount)
            .map_err(|e| PhpException::default(format!("Failed to approve return: {}", e)))?;

        Ok(ret.into())
    }

    pub fn reject(&self, id: String, reason: Option<String>) -> PhpResult<Return> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "return");

        let ret = commerce
            .returns()
            .reject(uuid, reason)
            .map_err(|e| PhpException::default(format!("Failed to reject return: {}", e)))?;

        Ok(ret.into())
    }
}

// ============================================================================
// Payments API
// ============================================================================

#[php_class(name = "StateSet\\Payments")]
#[derive(Clone)]
pub struct Payments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Payments {
    pub fn record(&self, order_id: String, amount: f64, method: Option<String>) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");
        let decimal_amount = decimal_from_f64(amount, "amount")?;

        commerce
            .payments()
            .record(uuid, decimal_amount, method)
            .map_err(|e| PhpException::default(format!("Failed to record payment: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Carts Types & API
// ============================================================================

#[php_class(name = "StateSet\\CartItem")]
#[derive(Clone)]
pub struct CartItem {
    id: String,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: f64,
    total: f64,
}

#[php_impl]
impl CartItem {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_sku(&self) -> String {
        self.sku.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_quantity(&self) -> i32 {
        self.quantity
    }

    #[getter]
    pub fn get_unit_price(&self) -> f64 {
        self.unit_price
    }

    #[getter]
    pub fn get_total(&self) -> f64 {
        self.total
    }
}

#[php_class(name = "StateSet\\Cart")]
#[derive(Clone)]
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

#[php_impl]
impl Cart {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_customer_id(&self) -> Option<String> {
        self.customer_id.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_items(&self) -> Vec<CartItem> {
        self.items.clone()
    }

    #[getter]
    pub fn get_subtotal(&self) -> f64 {
        self.subtotal
    }

    #[getter]
    pub fn get_total(&self) -> f64 {
        self.total
    }

    #[getter]
    pub fn get_currency(&self) -> String {
        self.currency.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "Cart(id={}, items={}, total={} {})",
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

#[php_class(name = "StateSet\\Carts")]
#[derive(Clone)]
pub struct Carts {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Carts {
    pub fn create(&self, customer_id: Option<String>, currency: Option<String>) -> PhpResult<Cart> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = customer_id.and_then(|s| s.parse().ok());

        let cart = commerce
            .carts()
            .create(stateset_core::CreateCart {
                customer_id: cust_uuid,
                currency,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create cart: {}", e)))?;

        Ok(cart.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Cart>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "cart");

        let cart = commerce
            .carts()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get cart: {}", e)))?;

        Ok(cart.map(|c| c.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Cart>> {
        let commerce = lock_commerce!(self.commerce);

        let carts = commerce
            .carts()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    pub fn add_item(
        &self,
        cart_id: String,
        sku: String,
        name: String,
        quantity: i32,
        unit_price: f64,
    ) -> PhpResult<Cart> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(cart_id, "cart");
        let price = decimal_from_f64(unit_price, "unit_price")?;

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
            .map_err(|e| PhpException::default(format!("Failed to add item: {}", e)))?;

        Ok(cart.into())
    }

    pub fn checkout(&self, cart_id: String) -> PhpResult<Order> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(cart_id, "cart");

        let order = commerce
            .carts()
            .checkout(uuid)
            .map_err(|e| PhpException::default(format!("Failed to checkout: {}", e)))?;

        Ok(order.into())
    }
}

// ============================================================================
// Analytics API
// ============================================================================

#[php_class(name = "StateSet\\SalesSummary")]
#[derive(Clone)]
pub struct SalesSummary {
    total_revenue: f64,
    total_orders: i64,
    average_order_value: f64,
    total_items_sold: i64,
}

#[php_impl]
impl SalesSummary {
    #[getter]
    pub fn get_total_revenue(&self) -> f64 {
        self.total_revenue
    }

    #[getter]
    pub fn get_total_orders(&self) -> i64 {
        self.total_orders
    }

    #[getter]
    pub fn get_average_order_value(&self) -> f64 {
        self.average_order_value
    }

    #[getter]
    pub fn get_total_items_sold(&self) -> i64 {
        self.total_items_sold
    }
}

#[php_class(name = "StateSet\\Analytics")]
#[derive(Clone)]
pub struct Analytics {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Analytics {
    pub fn sales_summary(&self, days: Option<i64>) -> PhpResult<SalesSummary> {
        let commerce = lock_commerce!(self.commerce);

        let summary = commerce
            .analytics()
            .sales_summary(days.unwrap_or(30))
            .map_err(|e| PhpException::default(format!("Failed to get sales summary: {}", e)))?;

        Ok(SalesSummary {
            total_revenue: to_f64_or_nan(summary.total_revenue),
            total_orders: summary.total_orders,
            average_order_value: to_f64_or_nan(summary.average_order_value),
            total_items_sold: summary.total_items_sold,
        })
    }
}

// ============================================================================
// Shipments Types & API
// ============================================================================

#[php_class(name = "StateSet\\Shipment")]
#[derive(Clone)]
pub struct Shipment {
    id: String,
    order_id: String,
    tracking_number: Option<String>,
    carrier: Option<String>,
    status: String,
    shipped_at: Option<String>,
    delivered_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[php_impl]
impl Shipment {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_order_id(&self) -> String {
        self.order_id.clone()
    }

    #[getter]
    pub fn get_tracking_number(&self) -> Option<String> {
        self.tracking_number.clone()
    }

    #[getter]
    pub fn get_carrier(&self) -> Option<String> {
        self.carrier.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_shipped_at(&self) -> Option<String> {
        self.shipped_at.clone()
    }

    #[getter]
    pub fn get_delivered_at(&self) -> Option<String> {
        self.delivered_at.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "Shipment(id={}, status={}, tracking={:?})",
            self.id, self.status, self.tracking_number
        )
    }
}

impl From<stateset_core::Shipment> for Shipment {
    fn from(s: stateset_core::Shipment) -> Self {
        Self {
            id: s.id.to_string(),
            order_id: s.order_id.to_string(),
            tracking_number: s.tracking_number,
            carrier: s.carrier,
            status: format!("{}", s.status),
            shipped_at: s.shipped_at.map(|t| t.to_rfc3339()),
            delivered_at: s.delivered_at.map(|t| t.to_rfc3339()),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\Shipments")]
#[derive(Clone)]
pub struct Shipments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Shipments {
    pub fn create(
        &self,
        order_id: String,
        tracking_number: Option<String>,
        carrier: Option<String>,
    ) -> PhpResult<Shipment> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");

        let shipment = commerce
            .shipments()
            .create(stateset_core::CreateShipment {
                order_id: uuid,
                tracking_number,
                carrier,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create shipment: {}", e)))?;

        Ok(shipment.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Shipment>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");

        let shipment = commerce
            .shipments()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get shipment: {}", e)))?;

        Ok(shipment.map(|s| s.into()))
    }

    pub fn get_by_tracking(&self, tracking_number: String) -> PhpResult<Option<Shipment>> {
        let commerce = lock_commerce!(self.commerce);

        let shipment = commerce
            .shipments()
            .get_by_tracking(&tracking_number)
            .map_err(|e| PhpException::default(format!("Failed to get shipment: {}", e)))?;

        Ok(shipment.map(|s| s.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Shipment>> {
        let commerce = lock_commerce!(self.commerce);

        let shipments = commerce
            .shipments()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list shipments: {}", e)))?;

        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    pub fn for_order(&self, order_id: String) -> PhpResult<Vec<Shipment>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(order_id, "order");

        let shipments = commerce
            .shipments()
            .for_order(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get shipments: {}", e)))?;

        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    pub fn ship(&self, id: String) -> PhpResult<Shipment> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");

        let shipment = commerce
            .shipments()
            .ship(uuid)
            .map_err(|e| PhpException::default(format!("Failed to ship: {}", e)))?;

        Ok(shipment.into())
    }

    pub fn mark_delivered(&self, id: String) -> PhpResult<Shipment> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");

        let shipment = commerce
            .shipments()
            .mark_delivered(uuid)
            .map_err(|e| PhpException::default(format!("Failed to mark delivered: {}", e)))?;

        Ok(shipment.into())
    }

    pub fn cancel(&self, id: String) -> PhpResult<Shipment> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "shipment");

        let shipment = commerce
            .shipments()
            .cancel(uuid)
            .map_err(|e| PhpException::default(format!("Failed to cancel shipment: {}", e)))?;

        Ok(shipment.into())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .shipments()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count shipments: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// Warranties Types & API
// ============================================================================

#[php_class(name = "StateSet\\Warranty")]
#[derive(Clone)]
pub struct Warranty {
    id: String,
    product_id: String,
    order_id: Option<String>,
    customer_id: String,
    warranty_type: String,
    status: String,
    start_date: String,
    end_date: String,
    created_at: String,
}

#[php_impl]
impl Warranty {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_product_id(&self) -> String {
        self.product_id.clone()
    }

    #[getter]
    pub fn get_order_id(&self) -> Option<String> {
        self.order_id.clone()
    }

    #[getter]
    pub fn get_customer_id(&self) -> String {
        self.customer_id.clone()
    }

    #[getter]
    pub fn get_warranty_type(&self) -> String {
        self.warranty_type.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_start_date(&self) -> String {
        self.start_date.clone()
    }

    #[getter]
    pub fn get_end_date(&self) -> String {
        self.end_date.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!("Warranty(id={}, type={}, status={})", self.id, self.warranty_type, self.status)
    }
}

impl From<stateset_core::Warranty> for Warranty {
    fn from(w: stateset_core::Warranty) -> Self {
        Self {
            id: w.id.to_string(),
            product_id: w.product_id.to_string(),
            order_id: w.order_id.map(|id| id.to_string()),
            customer_id: w.customer_id.to_string(),
            warranty_type: format!("{}", w.warranty_type),
            status: format!("{}", w.status),
            start_date: w.start_date.to_rfc3339(),
            end_date: w.end_date.to_rfc3339(),
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\WarrantyClaim")]
#[derive(Clone)]
pub struct WarrantyClaim {
    id: String,
    warranty_id: String,
    description: String,
    status: String,
    resolution: Option<String>,
    created_at: String,
}

#[php_impl]
impl WarrantyClaim {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_warranty_id(&self) -> String {
        self.warranty_id.clone()
    }

    #[getter]
    pub fn get_description(&self) -> String {
        self.description.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_resolution(&self) -> Option<String> {
        self.resolution.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<stateset_core::WarrantyClaim> for WarrantyClaim {
    fn from(c: stateset_core::WarrantyClaim) -> Self {
        Self {
            id: c.id.to_string(),
            warranty_id: c.warranty_id.to_string(),
            description: c.description,
            status: format!("{}", c.status),
            resolution: c.resolution,
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\Warranties")]
#[derive(Clone)]
pub struct Warranties {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Warranties {
    pub fn create(
        &self,
        product_id: String,
        customer_id: String,
        warranty_type: String,
        duration_months: i32,
    ) -> PhpResult<Warranty> {
        let commerce = lock_commerce!(self.commerce);
        let prod_uuid = parse_uuid!(product_id, "product");
        let cust_uuid = parse_uuid!(customer_id, "customer");

        let warranty = commerce
            .warranties()
            .create(stateset_core::CreateWarranty {
                product_id: prod_uuid,
                customer_id: cust_uuid,
                warranty_type,
                duration_months,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create warranty: {}", e)))?;

        Ok(warranty.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Warranty>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "warranty");

        let warranty = commerce
            .warranties()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get warranty: {}", e)))?;

        Ok(warranty.map(|w| w.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Warranty>> {
        let commerce = lock_commerce!(self.commerce);

        let warranties = commerce
            .warranties()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list warranties: {}", e)))?;

        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    pub fn for_customer(&self, customer_id: String) -> PhpResult<Vec<Warranty>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");

        let warranties = commerce
            .warranties()
            .for_customer(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get warranties: {}", e)))?;

        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    pub fn is_valid(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "warranty");

        let valid = commerce
            .warranties()
            .is_valid(uuid)
            .map_err(|e| PhpException::default(format!("Failed to check warranty: {}", e)))?;

        Ok(valid)
    }

    pub fn create_claim(
        &self,
        warranty_id: String,
        description: String,
    ) -> PhpResult<WarrantyClaim> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(warranty_id, "warranty");

        let claim = commerce
            .warranties()
            .create_claim(stateset_core::CreateWarrantyClaim {
                warranty_id: uuid,
                description,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create claim: {}", e)))?;

        Ok(claim.into())
    }

    pub fn approve_claim(
        &self,
        claim_id: String,
        resolution: Option<String>,
    ) -> PhpResult<WarrantyClaim> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(claim_id, "claim");

        let claim = commerce
            .warranties()
            .approve_claim(uuid, resolution)
            .map_err(|e| PhpException::default(format!("Failed to approve claim: {}", e)))?;

        Ok(claim.into())
    }

    pub fn deny_claim(&self, claim_id: String, reason: Option<String>) -> PhpResult<WarrantyClaim> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(claim_id, "claim");

        let claim = commerce
            .warranties()
            .deny_claim(uuid, reason)
            .map_err(|e| PhpException::default(format!("Failed to deny claim: {}", e)))?;

        Ok(claim.into())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .warranties()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count warranties: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// PurchaseOrders Types & API
// ============================================================================

#[php_class(name = "StateSet\\Supplier")]
#[derive(Clone)]
pub struct Supplier {
    id: String,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    status: String,
    created_at: String,
}

#[php_impl]
impl Supplier {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_email(&self) -> Option<String> {
        self.email.clone()
    }

    #[getter]
    pub fn get_phone(&self) -> Option<String> {
        self.phone.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<stateset_core::Supplier> for Supplier {
    fn from(s: stateset_core::Supplier) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            email: s.email,
            phone: s.phone,
            status: format!("{}", s.status),
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\PurchaseOrder")]
#[derive(Clone)]
pub struct PurchaseOrder {
    id: String,
    po_number: String,
    supplier_id: String,
    status: String,
    total_amount: f64,
    currency: String,
    expected_date: Option<String>,
    created_at: String,
    updated_at: String,
}

#[php_impl]
impl PurchaseOrder {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_po_number(&self) -> String {
        self.po_number.clone()
    }

    #[getter]
    pub fn get_supplier_id(&self) -> String {
        self.supplier_id.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_total_amount(&self) -> f64 {
        self.total_amount
    }

    #[getter]
    pub fn get_currency(&self) -> String {
        self.currency.clone()
    }

    #[getter]
    pub fn get_expected_date(&self) -> Option<String> {
        self.expected_date.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "PurchaseOrder(number={}, status={}, total={})",
            self.po_number, self.status, self.total_amount
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
            expected_date: p.expected_date.map(|d| d.to_rfc3339()),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\PurchaseOrders")]
#[derive(Clone)]
pub struct PurchaseOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl PurchaseOrders {
    pub fn create_supplier(
        &self,
        name: String,
        email: Option<String>,
        phone: Option<String>,
    ) -> PhpResult<Supplier> {
        let commerce = lock_commerce!(self.commerce);

        let supplier = commerce
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                name,
                email,
                phone,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create supplier: {}", e)))?;

        Ok(supplier.into())
    }

    pub fn get_supplier(&self, id: String) -> PhpResult<Option<Supplier>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "supplier");

        let supplier = commerce
            .purchase_orders()
            .get_supplier(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get supplier: {}", e)))?;

        Ok(supplier.map(|s| s.into()))
    }

    pub fn list_suppliers(&self) -> PhpResult<Vec<Supplier>> {
        let commerce = lock_commerce!(self.commerce);

        let suppliers = commerce
            .purchase_orders()
            .list_suppliers(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list suppliers: {}", e)))?;

        Ok(suppliers.into_iter().map(|s| s.into()).collect())
    }

    pub fn create(
        &self,
        supplier_id: String,
        currency: Option<String>,
    ) -> PhpResult<PurchaseOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(supplier_id, "supplier");

        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id: uuid,
                currency,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create PO: {}", e)))?;

        Ok(po.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<PurchaseOrder>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");

        let po = commerce
            .purchase_orders()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get PO: {}", e)))?;

        Ok(po.map(|p| p.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<PurchaseOrder>> {
        let commerce = lock_commerce!(self.commerce);

        let pos = commerce
            .purchase_orders()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list POs: {}", e)))?;

        Ok(pos.into_iter().map(|p| p.into()).collect())
    }

    pub fn submit(&self, id: String) -> PhpResult<PurchaseOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");

        let po = commerce
            .purchase_orders()
            .submit(uuid)
            .map_err(|e| PhpException::default(format!("Failed to submit PO: {}", e)))?;

        Ok(po.into())
    }

    pub fn approve(&self, id: String) -> PhpResult<PurchaseOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");

        let po = commerce
            .purchase_orders()
            .approve(uuid)
            .map_err(|e| PhpException::default(format!("Failed to approve PO: {}", e)))?;

        Ok(po.into())
    }

    pub fn cancel(&self, id: String) -> PhpResult<PurchaseOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");

        let po = commerce
            .purchase_orders()
            .cancel(uuid)
            .map_err(|e| PhpException::default(format!("Failed to cancel PO: {}", e)))?;

        Ok(po.into())
    }

    pub fn complete(&self, id: String) -> PhpResult<PurchaseOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "purchase_order");

        let po = commerce
            .purchase_orders()
            .complete(uuid)
            .map_err(|e| PhpException::default(format!("Failed to complete PO: {}", e)))?;

        Ok(po.into())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .purchase_orders()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count POs: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// Invoices Types & API
// ============================================================================

#[php_class(name = "StateSet\\Invoice")]
#[derive(Clone)]
pub struct Invoice {
    id: String,
    invoice_number: String,
    customer_id: String,
    order_id: Option<String>,
    status: String,
    subtotal: f64,
    tax: f64,
    total: f64,
    currency: String,
    due_date: Option<String>,
    paid_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[php_impl]
impl Invoice {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_invoice_number(&self) -> String {
        self.invoice_number.clone()
    }

    #[getter]
    pub fn get_customer_id(&self) -> String {
        self.customer_id.clone()
    }

    #[getter]
    pub fn get_order_id(&self) -> Option<String> {
        self.order_id.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_subtotal(&self) -> f64 {
        self.subtotal
    }

    #[getter]
    pub fn get_tax(&self) -> f64 {
        self.tax
    }

    #[getter]
    pub fn get_total(&self) -> f64 {
        self.total
    }

    #[getter]
    pub fn get_currency(&self) -> String {
        self.currency.clone()
    }

    #[getter]
    pub fn get_due_date(&self) -> Option<String> {
        self.due_date.clone()
    }

    #[getter]
    pub fn get_paid_at(&self) -> Option<String> {
        self.paid_at.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "Invoice(number={}, status={}, total={})",
            self.invoice_number, self.status, self.total
        )
    }
}

impl From<stateset_core::Invoice> for Invoice {
    fn from(i: stateset_core::Invoice) -> Self {
        Self {
            id: i.id.to_string(),
            invoice_number: i.invoice_number,
            customer_id: i.customer_id.to_string(),
            order_id: i.order_id.map(|id| id.to_string()),
            status: format!("{}", i.status),
            subtotal: to_f64_or_nan(i.subtotal),
            tax: to_f64_or_nan(i.tax),
            total: to_f64_or_nan(i.total),
            currency: i.currency,
            due_date: i.due_date.map(|d| d.to_rfc3339()),
            paid_at: i.paid_at.map(|d| d.to_rfc3339()),
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\Invoices")]
#[derive(Clone)]
pub struct Invoices {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Invoices {
    pub fn create(
        &self,
        customer_id: String,
        order_id: Option<String>,
        due_days: Option<i32>,
    ) -> PhpResult<Invoice> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let order_uuid = order_id.and_then(|s| s.parse().ok());

        let invoice = commerce
            .invoices()
            .create(stateset_core::CreateInvoice {
                customer_id: cust_uuid,
                order_id: order_uuid,
                due_days,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create invoice: {}", e)))?;

        Ok(invoice.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Invoice>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");

        let invoice = commerce
            .invoices()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get invoice: {}", e)))?;

        Ok(invoice.map(|i| i.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Invoice>> {
        let commerce = lock_commerce!(self.commerce);

        let invoices = commerce
            .invoices()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    pub fn for_customer(&self, customer_id: String) -> PhpResult<Vec<Invoice>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");

        let invoices = commerce
            .invoices()
            .for_customer(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    pub fn send(&self, id: String) -> PhpResult<Invoice> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");

        let invoice = commerce
            .invoices()
            .send(uuid)
            .map_err(|e| PhpException::default(format!("Failed to send invoice: {}", e)))?;

        Ok(invoice.into())
    }

    pub fn record_payment(&self, id: String, amount: f64) -> PhpResult<Invoice> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");
        let decimal_amount = decimal_from_f64(amount, "amount")?;

        let invoice = commerce
            .invoices()
            .record_payment(uuid, decimal_amount)
            .map_err(|e| PhpException::default(format!("Failed to record payment: {}", e)))?;

        Ok(invoice.into())
    }

    pub fn void(&self, id: String) -> PhpResult<Invoice> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "invoice");

        let invoice = commerce
            .invoices()
            .void(uuid)
            .map_err(|e| PhpException::default(format!("Failed to void invoice: {}", e)))?;

        Ok(invoice.into())
    }

    pub fn get_overdue(&self) -> PhpResult<Vec<Invoice>> {
        let commerce = lock_commerce!(self.commerce);

        let invoices = commerce
            .invoices()
            .get_overdue()
            .map_err(|e| PhpException::default(format!("Failed to get overdue invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    pub fn customer_balance(&self, customer_id: String) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");

        let balance = commerce
            .invoices()
            .customer_balance(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get balance: {}", e)))?;

        to_f64_result(balance, "customer balance")
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .invoices()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count invoices: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// BOM Types & API
// ============================================================================

#[php_class(name = "StateSet\\BomComponent")]
#[derive(Clone)]
pub struct BomComponent {
    id: String,
    bom_id: String,
    component_sku: String,
    quantity: i32,
    unit_cost: f64,
}

#[php_impl]
impl BomComponent {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_bom_id(&self) -> String {
        self.bom_id.clone()
    }

    #[getter]
    pub fn get_component_sku(&self) -> String {
        self.component_sku.clone()
    }

    #[getter]
    pub fn get_quantity(&self) -> i32 {
        self.quantity
    }

    #[getter]
    pub fn get_unit_cost(&self) -> f64 {
        self.unit_cost
    }
}

impl From<stateset_core::BomComponent> for BomComponent {
    fn from(c: stateset_core::BomComponent) -> Self {
        Self {
            id: c.id.to_string(),
            bom_id: c.bom_id.to_string(),
            component_sku: c.component_sku,
            quantity: c.quantity,
            unit_cost: to_f64_or_nan(c.unit_cost),
        }
    }
}

#[php_class(name = "StateSet\\BillOfMaterials")]
#[derive(Clone)]
pub struct BillOfMaterials {
    id: String,
    product_id: String,
    name: String,
    version: String,
    status: String,
    total_cost: f64,
    created_at: String,
    updated_at: String,
}

#[php_impl]
impl BillOfMaterials {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_product_id(&self) -> String {
        self.product_id.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_version(&self) -> String {
        self.version.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_total_cost(&self) -> f64 {
        self.total_cost
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!("BillOfMaterials(id={}, name={}, version={})", self.id, self.name, self.version)
    }
}

impl From<stateset_core::BillOfMaterials> for BillOfMaterials {
    fn from(b: stateset_core::BillOfMaterials) -> Self {
        Self {
            id: b.id.to_string(),
            product_id: b.product_id.to_string(),
            name: b.name,
            version: b.version,
            status: format!("{}", b.status),
            total_cost: to_f64_or_nan(b.total_cost),
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\BomApi")]
#[derive(Clone)]
pub struct BomApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl BomApi {
    pub fn create(
        &self,
        product_id: String,
        name: String,
        version: Option<String>,
    ) -> PhpResult<BillOfMaterials> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(product_id, "product");

        let bom = commerce
            .bom()
            .create(stateset_core::CreateBom {
                product_id: uuid,
                name,
                version,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create BOM: {}", e)))?;

        Ok(bom.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<BillOfMaterials>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bom");

        let bom = commerce
            .bom()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get BOM: {}", e)))?;

        Ok(bom.map(|b| b.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<BillOfMaterials>> {
        let commerce = lock_commerce!(self.commerce);

        let boms = commerce
            .bom()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list BOMs: {}", e)))?;

        Ok(boms.into_iter().map(|b| b.into()).collect())
    }

    pub fn add_component(
        &self,
        bom_id: String,
        component_sku: String,
        quantity: i32,
        unit_cost: f64,
    ) -> PhpResult<BomComponent> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(bom_id, "bom");
        let cost = decimal_from_f64(unit_cost, "unit_cost")?;

        let component = commerce
            .bom()
            .add_component(stateset_core::AddBomComponent {
                bom_id: uuid,
                component_sku,
                quantity,
                unit_cost: cost,
            })
            .map_err(|e| PhpException::default(format!("Failed to add component: {}", e)))?;

        Ok(component.into())
    }

    pub fn get_components(&self, bom_id: String) -> PhpResult<Vec<BomComponent>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(bom_id, "bom");

        let components = commerce
            .bom()
            .get_components(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get components: {}", e)))?;

        Ok(components.into_iter().map(|c| c.into()).collect())
    }

    pub fn remove_component(&self, component_id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(component_id, "component");

        commerce
            .bom()
            .remove_component(uuid)
            .map_err(|e| PhpException::default(format!("Failed to remove component: {}", e)))?;

        Ok(true)
    }

    pub fn activate(&self, id: String) -> PhpResult<BillOfMaterials> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bom");

        let bom = commerce
            .bom()
            .activate(uuid)
            .map_err(|e| PhpException::default(format!("Failed to activate BOM: {}", e)))?;

        Ok(bom.into())
    }

    pub fn delete(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bom");

        commerce
            .bom()
            .delete(uuid)
            .map_err(|e| PhpException::default(format!("Failed to delete BOM: {}", e)))?;

        Ok(true)
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .bom()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count BOMs: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// WorkOrders Types & API
// ============================================================================

#[php_class(name = "StateSet\\WorkOrder")]
#[derive(Clone)]
pub struct WorkOrder {
    id: String,
    work_order_number: String,
    bom_id: String,
    quantity: i32,
    status: String,
    priority: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[php_impl]
impl WorkOrder {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_work_order_number(&self) -> String {
        self.work_order_number.clone()
    }

    #[getter]
    pub fn get_bom_id(&self) -> String {
        self.bom_id.clone()
    }

    #[getter]
    pub fn get_quantity(&self) -> i32 {
        self.quantity
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_priority(&self) -> String {
        self.priority.clone()
    }

    #[getter]
    pub fn get_started_at(&self) -> Option<String> {
        self.started_at.clone()
    }

    #[getter]
    pub fn get_completed_at(&self) -> Option<String> {
        self.completed_at.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "WorkOrder(number={}, status={}, qty={})",
            self.work_order_number, self.status, self.quantity
        )
    }
}

impl From<stateset_core::WorkOrder> for WorkOrder {
    fn from(w: stateset_core::WorkOrder) -> Self {
        Self {
            id: w.id.to_string(),
            work_order_number: w.work_order_number,
            bom_id: w.bom_id.to_string(),
            quantity: w.quantity,
            status: format!("{}", w.status),
            priority: format!("{}", w.priority),
            started_at: w.started_at.map(|t| t.to_rfc3339()),
            completed_at: w.completed_at.map(|t| t.to_rfc3339()),
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\WorkOrders")]
#[derive(Clone)]
pub struct WorkOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl WorkOrders {
    pub fn create(
        &self,
        bom_id: String,
        quantity: i32,
        priority: Option<String>,
    ) -> PhpResult<WorkOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(bom_id, "bom");

        let work_order = commerce
            .work_orders()
            .create(stateset_core::CreateWorkOrder {
                bom_id: uuid,
                quantity,
                priority,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create work order: {}", e)))?;

        Ok(work_order.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<WorkOrder>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");

        let work_order = commerce
            .work_orders()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get work order: {}", e)))?;

        Ok(work_order.map(|w| w.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<WorkOrder>> {
        let commerce = lock_commerce!(self.commerce);

        let work_orders = commerce
            .work_orders()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list work orders: {}", e)))?;

        Ok(work_orders.into_iter().map(|w| w.into()).collect())
    }

    pub fn start(&self, id: String) -> PhpResult<WorkOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");

        let work_order = commerce
            .work_orders()
            .start(uuid)
            .map_err(|e| PhpException::default(format!("Failed to start work order: {}", e)))?;

        Ok(work_order.into())
    }

    pub fn complete(&self, id: String) -> PhpResult<WorkOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");

        let work_order = commerce
            .work_orders()
            .complete(uuid)
            .map_err(|e| PhpException::default(format!("Failed to complete work order: {}", e)))?;

        Ok(work_order.into())
    }

    pub fn hold(&self, id: String) -> PhpResult<WorkOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");

        let work_order = commerce
            .work_orders()
            .hold(uuid)
            .map_err(|e| PhpException::default(format!("Failed to hold work order: {}", e)))?;

        Ok(work_order.into())
    }

    pub fn resume(&self, id: String) -> PhpResult<WorkOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");

        let work_order = commerce
            .work_orders()
            .resume(uuid)
            .map_err(|e| PhpException::default(format!("Failed to resume work order: {}", e)))?;

        Ok(work_order.into())
    }

    pub fn cancel(&self, id: String) -> PhpResult<WorkOrder> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "work_order");

        let work_order = commerce
            .work_orders()
            .cancel(uuid)
            .map_err(|e| PhpException::default(format!("Failed to cancel work order: {}", e)))?;

        Ok(work_order.into())
    }

    pub fn count(&self) -> PhpResult<i64> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .work_orders()
            .count(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to count work orders: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// CurrencyOps Types & API
// ============================================================================

#[php_class(name = "StateSet\\ExchangeRate")]
#[derive(Clone)]
pub struct ExchangeRate {
    from_currency: String,
    to_currency: String,
    rate: f64,
    updated_at: String,
}

#[php_impl]
impl ExchangeRate {
    #[getter]
    pub fn get_from_currency(&self) -> String {
        self.from_currency.clone()
    }

    #[getter]
    pub fn get_to_currency(&self) -> String {
        self.to_currency.clone()
    }

    #[getter]
    pub fn get_rate(&self) -> f64 {
        self.rate
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }
}

impl From<stateset_core::ExchangeRate> for ExchangeRate {
    fn from(r: stateset_core::ExchangeRate) -> Self {
        Self {
            from_currency: r.from_currency,
            to_currency: r.to_currency,
            rate: to_f64_or_nan(r.rate),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\CurrencyOps")]
#[derive(Clone)]
pub struct CurrencyOps {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl CurrencyOps {
    pub fn get_rate(&self, from: String, to: String) -> PhpResult<Option<ExchangeRate>> {
        let commerce = lock_commerce!(self.commerce);

        let rate = commerce
            .currency()
            .get_rate(&from, &to)
            .map_err(|e| PhpException::default(format!("Failed to get rate: {}", e)))?;

        Ok(rate.map(|r| r.into()))
    }

    pub fn list_rates(&self) -> PhpResult<Vec<ExchangeRate>> {
        let commerce = lock_commerce!(self.commerce);

        let rates = commerce
            .currency()
            .list_rates()
            .map_err(|e| PhpException::default(format!("Failed to list rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    pub fn set_rate(&self, from: String, to: String, rate: f64) -> PhpResult<ExchangeRate> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_rate = decimal_from_f64(rate, "rate")?;

        let exchange_rate = commerce
            .currency()
            .set_rate(&from, &to, decimal_rate)
            .map_err(|e| PhpException::default(format!("Failed to set rate: {}", e)))?;

        Ok(exchange_rate.into())
    }

    pub fn convert(&self, amount: f64, from: String, to: String) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_amount = decimal_from_f64(amount, "amount")?;

        let converted = commerce
            .currency()
            .convert(decimal_amount, &from, &to)
            .map_err(|e| PhpException::default(format!("Failed to convert: {}", e)))?;

        to_f64_result(converted, "converted amount")
    }

    pub fn base_currency(&self) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);

        let base = commerce
            .currency()
            .base_currency()
            .map_err(|e| PhpException::default(format!("Failed to get base currency: {}", e)))?;

        Ok(base)
    }

    pub fn enabled_currencies(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let currencies = commerce
            .currency()
            .enabled_currencies()
            .map_err(|e| PhpException::default(format!("Failed to get currencies: {}", e)))?;

        Ok(currencies)
    }

    pub fn format(&self, amount: f64, currency: String) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_amount = decimal_from_f64(amount, "amount")?;

        let formatted = commerce
            .currency()
            .format(decimal_amount, &currency)
            .map_err(|e| PhpException::default(format!("Failed to format: {}", e)))?;

        Ok(formatted)
    }
}

// ============================================================================
// Subscriptions Types & API
// ============================================================================

#[php_class(name = "StateSet\\SubscriptionPlan")]
#[derive(Clone)]
pub struct SubscriptionPlan {
    id: String,
    name: String,
    description: Option<String>,
    price: f64,
    currency: String,
    interval: String,
    interval_count: i32,
    trial_days: Option<i32>,
    status: String,
    created_at: String,
}

#[php_impl]
impl SubscriptionPlan {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_description(&self) -> Option<String> {
        self.description.clone()
    }

    #[getter]
    pub fn get_price(&self) -> f64 {
        self.price
    }

    #[getter]
    pub fn get_currency(&self) -> String {
        self.currency.clone()
    }

    #[getter]
    pub fn get_interval(&self) -> String {
        self.interval.clone()
    }

    #[getter]
    pub fn get_interval_count(&self) -> i32 {
        self.interval_count
    }

    #[getter]
    pub fn get_trial_days(&self) -> Option<i32> {
        self.trial_days
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<stateset_core::SubscriptionPlan> for SubscriptionPlan {
    fn from(p: stateset_core::SubscriptionPlan) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            price: to_f64_or_nan(p.price),
            currency: p.currency,
            interval: format!("{}", p.interval),
            interval_count: p.interval_count,
            trial_days: p.trial_days,
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\Subscription")]
#[derive(Clone)]
pub struct Subscription {
    id: String,
    plan_id: String,
    customer_id: String,
    status: String,
    current_period_start: String,
    current_period_end: String,
    canceled_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[php_impl]
impl Subscription {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_plan_id(&self) -> String {
        self.plan_id.clone()
    }

    #[getter]
    pub fn get_customer_id(&self) -> String {
        self.customer_id.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_current_period_start(&self) -> String {
        self.current_period_start.clone()
    }

    #[getter]
    pub fn get_current_period_end(&self) -> String {
        self.current_period_end.clone()
    }

    #[getter]
    pub fn get_canceled_at(&self) -> Option<String> {
        self.canceled_at.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    #[getter]
    pub fn get_updated_at(&self) -> String {
        self.updated_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!("Subscription(id={}, status={})", self.id, self.status)
    }
}

impl From<stateset_core::Subscription> for Subscription {
    fn from(s: stateset_core::Subscription) -> Self {
        Self {
            id: s.id.to_string(),
            plan_id: s.plan_id.to_string(),
            customer_id: s.customer_id.to_string(),
            status: format!("{}", s.status),
            current_period_start: s.current_period_start.to_rfc3339(),
            current_period_end: s.current_period_end.to_rfc3339(),
            canceled_at: s.canceled_at.map(|t| t.to_rfc3339()),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\Subscriptions")]
#[derive(Clone)]
pub struct Subscriptions {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Subscriptions {
    pub fn create_plan(
        &self,
        name: String,
        price: f64,
        interval: String,
        interval_count: Option<i32>,
        trial_days: Option<i32>,
    ) -> PhpResult<SubscriptionPlan> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_price = decimal_from_f64(price, "price")?;

        let plan = commerce
            .subscriptions()
            .create_plan(stateset_core::CreateSubscriptionPlan {
                name,
                price: decimal_price,
                interval,
                interval_count,
                trial_days,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create plan: {}", e)))?;

        Ok(plan.into())
    }

    pub fn get_plan(&self, id: String) -> PhpResult<Option<SubscriptionPlan>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "plan");

        let plan = commerce
            .subscriptions()
            .get_plan(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get plan: {}", e)))?;

        Ok(plan.map(|p| p.into()))
    }

    pub fn list_plans(&self) -> PhpResult<Vec<SubscriptionPlan>> {
        let commerce = lock_commerce!(self.commerce);

        let plans = commerce
            .subscriptions()
            .list_plans(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list plans: {}", e)))?;

        Ok(plans.into_iter().map(|p| p.into()).collect())
    }

    pub fn subscribe(&self, plan_id: String, customer_id: String) -> PhpResult<Subscription> {
        let commerce = lock_commerce!(self.commerce);
        let plan_uuid = parse_uuid!(plan_id, "plan");
        let cust_uuid = parse_uuid!(customer_id, "customer");

        let subscription = commerce
            .subscriptions()
            .subscribe(stateset_core::CreateSubscription {
                plan_id: plan_uuid,
                customer_id: cust_uuid,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to subscribe: {}", e)))?;

        Ok(subscription.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Subscription>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");

        let subscription = commerce
            .subscriptions()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get subscription: {}", e)))?;

        Ok(subscription.map(|s| s.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Subscription>> {
        let commerce = lock_commerce!(self.commerce);

        let subscriptions = commerce
            .subscriptions()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list subscriptions: {}", e)))?;

        Ok(subscriptions.into_iter().map(|s| s.into()).collect())
    }

    pub fn pause(&self, id: String) -> PhpResult<Subscription> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");

        let subscription = commerce
            .subscriptions()
            .pause(uuid)
            .map_err(|e| PhpException::default(format!("Failed to pause subscription: {}", e)))?;

        Ok(subscription.into())
    }

    pub fn resume(&self, id: String) -> PhpResult<Subscription> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");

        let subscription = commerce
            .subscriptions()
            .resume(uuid)
            .map_err(|e| PhpException::default(format!("Failed to resume subscription: {}", e)))?;

        Ok(subscription.into())
    }

    pub fn cancel(&self, id: String, at_period_end: Option<bool>) -> PhpResult<Subscription> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");

        let subscription = commerce
            .subscriptions()
            .cancel(uuid, at_period_end.unwrap_or(true))
            .map_err(|e| PhpException::default(format!("Failed to cancel subscription: {}", e)))?;

        Ok(subscription.into())
    }

    pub fn for_customer(&self, customer_id: String) -> PhpResult<Vec<Subscription>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(customer_id, "customer");

        let subscriptions = commerce
            .subscriptions()
            .for_customer(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get subscriptions: {}", e)))?;

        Ok(subscriptions.into_iter().map(|s| s.into()).collect())
    }

    pub fn is_active(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "subscription");

        let active = commerce
            .subscriptions()
            .is_active(uuid)
            .map_err(|e| PhpException::default(format!("Failed to check subscription: {}", e)))?;

        Ok(active)
    }
}

// ============================================================================
// Promotions Types & API
// ============================================================================

#[php_class(name = "StateSet\\Promotion")]
#[derive(Clone)]
pub struct Promotion {
    id: String,
    code: String,
    name: String,
    description: Option<String>,
    discount_type: String,
    discount_value: f64,
    min_purchase: Option<f64>,
    max_uses: Option<i32>,
    uses_count: i32,
    starts_at: Option<String>,
    ends_at: Option<String>,
    status: String,
    created_at: String,
}

#[php_impl]
impl Promotion {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_code(&self) -> String {
        self.code.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_description(&self) -> Option<String> {
        self.description.clone()
    }

    #[getter]
    pub fn get_discount_type(&self) -> String {
        self.discount_type.clone()
    }

    #[getter]
    pub fn get_discount_value(&self) -> f64 {
        self.discount_value
    }

    #[getter]
    pub fn get_min_purchase(&self) -> Option<f64> {
        self.min_purchase
    }

    #[getter]
    pub fn get_max_uses(&self) -> Option<i32> {
        self.max_uses
    }

    #[getter]
    pub fn get_uses_count(&self) -> i32 {
        self.uses_count
    }

    #[getter]
    pub fn get_starts_at(&self) -> Option<String> {
        self.starts_at.clone()
    }

    #[getter]
    pub fn get_ends_at(&self) -> Option<String> {
        self.ends_at.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }

    #[getter]
    pub fn get_created_at(&self) -> String {
        self.created_at.clone()
    }

    pub fn __to_string(&self) -> String {
        format!(
            "Promotion(code={}, type={}, value={})",
            self.code, self.discount_type, self.discount_value
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
            uses_count: p.uses_count,
            starts_at: p.starts_at.map(|t| t.to_rfc3339()),
            ends_at: p.ends_at.map(|t| t.to_rfc3339()),
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[php_class(name = "StateSet\\Promotions")]
#[derive(Clone)]
pub struct Promotions {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Promotions {
    pub fn create(
        &self,
        code: String,
        name: String,
        discount_type: String,
        discount_value: f64,
        min_purchase: Option<f64>,
        max_uses: Option<i32>,
    ) -> PhpResult<Promotion> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_value = decimal_from_f64(discount_value, "discount_value")?;
        let decimal_min = optional_decimal_from_f64(min_purchase, "min_purchase")?;

        let promotion = commerce
            .promotions()
            .create(stateset_core::CreatePromotion {
                code,
                name,
                discount_type,
                discount_value: decimal_value,
                min_purchase: decimal_min,
                max_uses,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create promotion: {}", e)))?;

        Ok(promotion.into())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<Promotion>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");

        let promotion = commerce
            .promotions()
            .get(uuid.into())
            .map_err(|e| PhpException::default(format!("Failed to get promotion: {}", e)))?;

        Ok(promotion.map(|p| p.into()))
    }

    pub fn get_by_code(&self, code: String) -> PhpResult<Option<Promotion>> {
        let commerce = lock_commerce!(self.commerce);

        let promotion = commerce
            .promotions()
            .get_by_code(&code)
            .map_err(|e| PhpException::default(format!("Failed to get promotion: {}", e)))?;

        Ok(promotion.map(|p| p.into()))
    }

    pub fn list(&self) -> PhpResult<Vec<Promotion>> {
        let commerce = lock_commerce!(self.commerce);

        let promotions = commerce
            .promotions()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list promotions: {}", e)))?;

        Ok(promotions.into_iter().map(|p| p.into()).collect())
    }

    pub fn activate(&self, id: String) -> PhpResult<Promotion> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");

        let promotion = commerce
            .promotions()
            .activate(uuid)
            .map_err(|e| PhpException::default(format!("Failed to activate promotion: {}", e)))?;

        Ok(promotion.into())
    }

    pub fn deactivate(&self, id: String) -> PhpResult<Promotion> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");

        let promotion = commerce
            .promotions()
            .deactivate(uuid)
            .map_err(|e| PhpException::default(format!("Failed to deactivate promotion: {}", e)))?;

        Ok(promotion.into())
    }

    pub fn get_active(&self) -> PhpResult<Vec<Promotion>> {
        let commerce = lock_commerce!(self.commerce);

        let promotions = commerce.promotions().get_active().map_err(|e| {
            PhpException::default(format!("Failed to get active promotions: {}", e))
        })?;

        Ok(promotions.into_iter().map(|p| p.into()).collect())
    }

    pub fn is_valid(&self, code: String, order_total: Option<f64>) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_total = optional_decimal_from_f64(order_total, "order_total")?;

        let valid = commerce
            .promotions()
            .is_valid(&code, decimal_total)
            .map_err(|e| PhpException::default(format!("Failed to validate promotion: {}", e)))?;

        Ok(valid)
    }

    pub fn delete(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "promotion");

        commerce
            .promotions()
            .delete(uuid)
            .map_err(|e| PhpException::default(format!("Failed to delete promotion: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Tax Types & API
// ============================================================================

#[php_class(name = "StateSet\\TaxJurisdiction")]
#[derive(Clone)]
pub struct TaxJurisdiction {
    id: String,
    name: String,
    country: String,
    state: Option<String>,
    city: Option<String>,
    postal_code: Option<String>,
    status: String,
}

#[php_impl]
impl TaxJurisdiction {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_country(&self) -> String {
        self.country.clone()
    }

    #[getter]
    pub fn get_state(&self) -> Option<String> {
        self.state.clone()
    }

    #[getter]
    pub fn get_city(&self) -> Option<String> {
        self.city.clone()
    }

    #[getter]
    pub fn get_postal_code(&self) -> Option<String> {
        self.postal_code.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }
}

impl From<stateset_core::TaxJurisdiction> for TaxJurisdiction {
    fn from(j: stateset_core::TaxJurisdiction) -> Self {
        Self {
            id: j.id.to_string(),
            name: j.name,
            country: j.country,
            state: j.state,
            city: j.city,
            postal_code: j.postal_code,
            status: format!("{}", j.status),
        }
    }
}

#[php_class(name = "StateSet\\TaxRate")]
#[derive(Clone)]
pub struct TaxRate {
    id: String,
    jurisdiction_id: String,
    name: String,
    rate: f64,
    tax_type: String,
    status: String,
}

#[php_impl]
impl TaxRate {
    #[getter]
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    #[getter]
    pub fn get_jurisdiction_id(&self) -> String {
        self.jurisdiction_id.clone()
    }

    #[getter]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    #[getter]
    pub fn get_rate(&self) -> f64 {
        self.rate
    }

    #[getter]
    pub fn get_tax_type(&self) -> String {
        self.tax_type.clone()
    }

    #[getter]
    pub fn get_status(&self) -> String {
        self.status.clone()
    }
}

impl From<stateset_core::TaxRate> for TaxRate {
    fn from(r: stateset_core::TaxRate) -> Self {
        Self {
            id: r.id.to_string(),
            jurisdiction_id: r.jurisdiction_id.to_string(),
            name: r.name,
            rate: to_f64_or_nan(r.rate),
            tax_type: format!("{}", r.tax_type),
            status: format!("{}", r.status),
        }
    }
}

#[php_class(name = "StateSet\\Tax")]
#[derive(Clone)]
pub struct Tax {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Tax {
    pub fn calculate(&self, amount: f64, jurisdiction_id: String) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);
        let decimal_amount = decimal_from_f64(amount, "amount")?;
        let uuid = parse_uuid!(jurisdiction_id, "jurisdiction");

        let tax = commerce
            .tax()
            .calculate(decimal_amount, uuid)
            .map_err(|e| PhpException::default(format!("Failed to calculate tax: {}", e)))?;

        to_f64_result(tax, "tax amount")
    }

    pub fn get_effective_rate(&self, jurisdiction_id: String) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(jurisdiction_id, "jurisdiction");

        let rate = commerce
            .tax()
            .get_effective_rate(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get rate: {}", e)))?;

        to_f64_result(rate, "tax rate")
    }

    pub fn list_jurisdictions(&self) -> PhpResult<Vec<TaxJurisdiction>> {
        let commerce = lock_commerce!(self.commerce);

        let jurisdictions = commerce
            .tax()
            .list_jurisdictions(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list jurisdictions: {}", e)))?;

        Ok(jurisdictions.into_iter().map(|j| j.into()).collect())
    }

    pub fn create_jurisdiction(
        &self,
        name: String,
        country: String,
        state: Option<String>,
        city: Option<String>,
    ) -> PhpResult<TaxJurisdiction> {
        let commerce = lock_commerce!(self.commerce);

        let jurisdiction = commerce
            .tax()
            .create_jurisdiction(stateset_core::CreateTaxJurisdiction {
                name,
                country,
                state,
                city,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create jurisdiction: {}", e)))?;

        Ok(jurisdiction.into())
    }

    pub fn list_rates(&self, jurisdiction_id: String) -> PhpResult<Vec<TaxRate>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(jurisdiction_id, "jurisdiction");

        let rates = commerce
            .tax()
            .list_rates(uuid)
            .map_err(|e| PhpException::default(format!("Failed to list rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    pub fn create_rate(
        &self,
        jurisdiction_id: String,
        name: String,
        rate: f64,
        tax_type: String,
    ) -> PhpResult<TaxRate> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(jurisdiction_id, "jurisdiction");
        let decimal_rate = decimal_from_f64(rate, "rate")?;

        let tax_rate = commerce
            .tax()
            .create_rate(stateset_core::CreateTaxRate {
                jurisdiction_id: uuid,
                name,
                rate: decimal_rate,
                tax_type,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create rate: {}", e)))?;

        Ok(tax_rate.into())
    }

    pub fn is_enabled(&self) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);

        let enabled = commerce
            .tax()
            .is_enabled()
            .map_err(|e| PhpException::default(format!("Failed to check tax status: {}", e)))?;

        Ok(enabled)
    }

    pub fn set_enabled(&self, enabled: bool) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);

        commerce
            .tax()
            .set_enabled(enabled)
            .map_err(|e| PhpException::default(format!("Failed to set tax status: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Quality API
// ============================================================================

#[php_class(name = "StateSet\\Quality")]
#[derive(Clone)]
pub struct Quality {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Quality {
    pub fn create_inspection(
        &self,
        inspection_type: String,
        reference_type: String,
        reference_id: String,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let ref_uuid = parse_uuid!(reference_id, "reference");

        let inspection = commerce
            .quality()
            .create_inspection(stateset_core::CreateInspection {
                inspection_type,
                reference_type,
                reference_id: ref_uuid,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create inspection: {}", e)))?;

        Ok(inspection.id.to_string())
    }

    pub fn list_inspections(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let inspections = commerce
            .quality()
            .list_inspections(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list inspections: {}", e)))?;

        Ok(inspections.into_iter().map(|i| i.id.to_string()).collect())
    }

    pub fn create_hold(
        &self,
        sku: String,
        quantity: i32,
        reason: String,
        hold_type: String,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);

        let hold = commerce
            .quality()
            .create_hold(stateset_core::CreateQualityHold {
                sku,
                quantity_held: quantity,
                reason,
                hold_type,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create hold: {}", e)))?;

        Ok(hold.id.to_string())
    }

    pub fn release_hold(&self, id: String, released_by: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "hold");

        commerce
            .quality()
            .release_hold(uuid, &released_by)
            .map_err(|e| PhpException::default(format!("Failed to release hold: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Lots API
// ============================================================================

#[php_class(name = "StateSet\\Lots")]
#[derive(Clone)]
pub struct Lots {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Lots {
    pub fn create(&self, sku: String, quantity_produced: i32) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);

        let lot = commerce
            .lots()
            .create(stateset_core::CreateLot { sku, quantity_produced, ..Default::default() })
            .map_err(|e| PhpException::default(format!("Failed to create lot: {}", e)))?;

        Ok(lot.id.to_string())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "lot");

        let lot = commerce
            .lots()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get lot: {}", e)))?;

        Ok(lot.map(|l| l.lot_number))
    }

    pub fn list(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let lots = commerce
            .lots()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list lots: {}", e)))?;

        Ok(lots.into_iter().map(|l| l.id.to_string()).collect())
    }

    pub fn quarantine(&self, id: String, reason: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "lot");

        commerce
            .lots()
            .quarantine(uuid, &reason)
            .map_err(|e| PhpException::default(format!("Failed to quarantine: {}", e)))?;

        Ok(true)
    }

    pub fn release_quarantine(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "lot");

        commerce
            .lots()
            .release_quarantine(uuid)
            .map_err(|e| PhpException::default(format!("Failed to release: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Serials API
// ============================================================================

#[php_class(name = "StateSet\\Serials")]
#[derive(Clone)]
pub struct Serials {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Serials {
    pub fn create(&self, sku: String, lot_number: Option<String>) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);

        let serial = commerce
            .serials()
            .create(stateset_core::CreateSerial { sku, lot_number, ..Default::default() })
            .map_err(|e| PhpException::default(format!("Failed to create serial: {}", e)))?;

        Ok(serial.serial_number)
    }

    pub fn get(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "serial");

        let serial = commerce
            .serials()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get serial: {}", e)))?;

        Ok(serial.map(|s| s.serial_number))
    }

    pub fn list(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let serials = commerce
            .serials()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list serials: {}", e)))?;

        Ok(serials.into_iter().map(|s| s.serial_number).collect())
    }

    pub fn mark_sold(
        &self,
        id: String,
        customer_id: String,
        order_id: Option<String>,
    ) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "serial");
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let ord_uuid = order_id
            .map(|o| o.parse())
            .transpose()
            .map_err(|_| PhpException::default("Invalid order UUID".to_string()))?;

        commerce
            .serials()
            .mark_sold(uuid, cust_uuid, ord_uuid)
            .map_err(|e| PhpException::default(format!("Failed to mark sold: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Warehouse API
// ============================================================================

#[php_class(name = "StateSet\\Warehouse")]
#[derive(Clone)]
pub struct WarehouseApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl WarehouseApi {
    pub fn create_warehouse(
        &self,
        code: String,
        name: String,
        warehouse_type: String,
    ) -> PhpResult<i32> {
        let commerce = lock_commerce!(self.commerce);

        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code,
                name,
                warehouse_type,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create warehouse: {}", e)))?;

        Ok(warehouse.id)
    }

    pub fn get_warehouse(&self, id: i32) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);

        let warehouse = commerce
            .warehouse()
            .get_warehouse(id)
            .map_err(|e| PhpException::default(format!("Failed to get warehouse: {}", e)))?;

        Ok(warehouse.map(|w| w.name))
    }

    pub fn list_warehouses(&self) -> PhpResult<Vec<i32>> {
        let commerce = lock_commerce!(self.commerce);

        let warehouses = commerce
            .warehouse()
            .list_warehouses()
            .map_err(|e| PhpException::default(format!("Failed to list warehouses: {}", e)))?;

        Ok(warehouses.into_iter().map(|w| w.id).collect())
    }

    pub fn create_location(
        &self,
        warehouse_id: i32,
        location_type: String,
        zone: Option<String>,
        aisle: Option<String>,
    ) -> PhpResult<i32> {
        let commerce = lock_commerce!(self.commerce);

        let location = commerce
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id,
                location_type,
                zone,
                aisle,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create location: {}", e)))?;

        Ok(location.id)
    }
}

// ============================================================================
// Receiving API
// ============================================================================

#[php_class(name = "StateSet\\Receiving")]
#[derive(Clone)]
pub struct Receiving {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Receiving {
    pub fn create_receipt(
        &self,
        receipt_type: String,
        warehouse_id: i32,
        po_id: Option<String>,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let po_uuid = po_id
            .map(|p| p.parse())
            .transpose()
            .map_err(|_| PhpException::default("Invalid PO UUID".to_string()))?;

        let receipt = commerce
            .receiving()
            .create_receipt(stateset_core::CreateReceipt {
                receipt_type,
                warehouse_id,
                purchase_order_id: po_uuid,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create receipt: {}", e)))?;

        Ok(receipt.id.to_string())
    }

    pub fn get_receipt(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "receipt");

        let receipt = commerce
            .receiving()
            .get_receipt(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get receipt: {}", e)))?;

        Ok(receipt.map(|r| r.receipt_number))
    }

    pub fn list_receipts(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let receipts = commerce
            .receiving()
            .list_receipts(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list receipts: {}", e)))?;

        Ok(receipts.into_iter().map(|r| r.id.to_string()).collect())
    }

    pub fn complete_receipt(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "receipt");

        commerce
            .receiving()
            .complete_receipt(uuid)
            .map_err(|e| PhpException::default(format!("Failed to complete receipt: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Fulfillment API
// ============================================================================

#[php_class(name = "StateSet\\Fulfillment")]
#[derive(Clone)]
pub struct Fulfillment {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Fulfillment {
    pub fn create_wave(
        &self,
        warehouse_id: i32,
        order_ids: Vec<String>,
        priority: i32,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let uuids: Result<Vec<uuid::Uuid>, _> = order_ids.iter().map(|id| id.parse()).collect();
        let uuids = uuids.map_err(|_| PhpException::default("Invalid order UUID".to_string()))?;

        let wave = commerce
            .fulfillment()
            .create_wave(stateset_core::CreateWave {
                warehouse_id,
                order_ids: uuids.into_iter().map(|u| u.into()).collect(),
                priority,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create wave: {}", e)))?;

        Ok(wave.id.to_string())
    }

    pub fn get_wave(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "wave");

        let wave = commerce
            .fulfillment()
            .get_wave(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get wave: {}", e)))?;

        Ok(wave.map(|w| w.wave_number))
    }

    pub fn release_wave(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "wave");

        commerce
            .fulfillment()
            .release_wave(uuid)
            .map_err(|e| PhpException::default(format!("Failed to release wave: {}", e)))?;

        Ok(true)
    }

    pub fn complete_wave(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "wave");

        commerce
            .fulfillment()
            .complete_wave(uuid)
            .map_err(|e| PhpException::default(format!("Failed to complete wave: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Accounts Payable API
// ============================================================================

#[php_class(name = "StateSet\\AccountsPayable")]
#[derive(Clone)]
pub struct AccountsPayable {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl AccountsPayable {
    pub fn create_bill(
        &self,
        supplier_id: String,
        due_date: String,
        payment_terms: Option<String>,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let supp_uuid = parse_uuid!(supplier_id, "supplier");

        let bill = commerce
            .accounts_payable()
            .create_bill(stateset_core::CreateBill {
                supplier_id: supp_uuid,
                due_date,
                payment_terms,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create bill: {}", e)))?;

        Ok(bill.id.to_string())
    }

    pub fn get_bill(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "bill");

        let bill = commerce
            .accounts_payable()
            .get_bill(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get bill: {}", e)))?;

        Ok(bill.map(|b| b.bill_number))
    }

    pub fn list_bills(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let bills = commerce
            .accounts_payable()
            .list_bills(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list bills: {}", e)))?;

        Ok(bills.into_iter().map(|b| b.id.to_string()).collect())
    }

    pub fn get_total_outstanding(&self) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);

        let total = commerce
            .accounts_payable()
            .get_total_outstanding()
            .map_err(|e| PhpException::default(format!("Failed to get total: {}", e)))?;

        to_f64_result(total, "accounts payable total outstanding")
    }
}

// ============================================================================
// Accounts Receivable API
// ============================================================================

#[php_class(name = "StateSet\\AccountsReceivable")]
#[derive(Clone)]
pub struct AccountsReceivable {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl AccountsReceivable {
    pub fn get_total_outstanding(&self) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);

        let total = commerce
            .accounts_receivable()
            .get_total_outstanding()
            .map_err(|e| PhpException::default(format!("Failed to get total: {}", e)))?;

        to_f64_result(total, "accounts receivable total outstanding")
    }

    pub fn get_dso(&self, days: i32) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);

        let dso = commerce
            .accounts_receivable()
            .get_dso(days)
            .map_err(|e| PhpException::default(format!("Failed to get DSO: {}", e)))?;

        Ok(dso)
    }

    pub fn create_credit_memo(
        &self,
        customer_id: String,
        amount: f64,
        reason: String,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let decimal_amount = decimal_from_f64(amount, "amount")?;

        let memo = commerce
            .accounts_receivable()
            .create_credit_memo(stateset_core::CreateCreditMemo {
                customer_id: cust_uuid,
                amount: decimal_amount,
                reason,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create credit memo: {}", e)))?;

        Ok(memo.id.to_string())
    }
}

// ============================================================================
// Cost Accounting API
// ============================================================================

#[php_class(name = "StateSet\\CostAccounting")]
#[derive(Clone)]
pub struct CostAccounting {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl CostAccounting {
    pub fn get_item_cost(&self, sku: String) -> PhpResult<Option<f64>> {
        let commerce = lock_commerce!(self.commerce);

        let cost = commerce
            .cost_accounting()
            .get_item_cost(&sku)
            .map_err(|e| PhpException::default(format!("Failed to get cost: {}", e)))?;

        Ok(cost.map(|c| to_f64_or_nan(c.current_cost)))
    }

    pub fn set_item_cost(
        &self,
        sku: String,
        standard_cost: f64,
        current_cost: Option<f64>,
    ) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let std = decimal_from_f64(standard_cost, "standard_cost")?;
        let curr = optional_decimal_from_f64(current_cost, "current_cost")?;

        commerce
            .cost_accounting()
            .set_item_cost(&sku, std, curr)
            .map_err(|e| PhpException::default(format!("Failed to set cost: {}", e)))?;

        Ok(true)
    }

    pub fn get_total_inventory_value(&self) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);

        let total = commerce
            .cost_accounting()
            .get_total_inventory_value()
            .map_err(|e| PhpException::default(format!("Failed to get total: {}", e)))?;

        to_f64_result(total, "inventory value")
    }
}

// ============================================================================
// Credit API
// ============================================================================

#[php_class(name = "StateSet\\Credit")]
#[derive(Clone)]
pub struct CreditApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl CreditApi {
    pub fn create_account(&self, customer_id: String, credit_limit: f64) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let limit = decimal_from_f64(credit_limit, "credit_limit")?;

        let account = commerce
            .credit()
            .create_account(stateset_core::CreateCreditAccount {
                customer_id: cust_uuid,
                credit_limit: limit,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create account: {}", e)))?;

        Ok(account.id.to_string())
    }

    pub fn check_credit(&self, customer_id: String, order_amount: f64) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let amount = decimal_from_f64(order_amount, "order_amount")?;

        let result = commerce
            .credit()
            .check_credit(cust_uuid, amount)
            .map_err(|e| PhpException::default(format!("Failed to check credit: {}", e)))?;

        Ok(result.approved)
    }

    pub fn adjust_limit(
        &self,
        customer_id: String,
        new_limit: f64,
        reason: String,
    ) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let cust_uuid = parse_uuid!(customer_id, "customer");
        let limit = decimal_from_f64(new_limit, "new_limit")?;

        commerce
            .credit()
            .adjust_limit(cust_uuid, limit, &reason)
            .map_err(|e| PhpException::default(format!("Failed to adjust limit: {}", e)))?;

        Ok(true)
    }
}

// ============================================================================
// Backorders API
// ============================================================================

#[php_class(name = "StateSet\\Backorders")]
#[derive(Clone)]
pub struct Backorders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl Backorders {
    pub fn create(
        &self,
        order_id: String,
        sku: String,
        quantity: i32,
        expected_date: Option<String>,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);
        let ord_uuid = parse_uuid!(order_id, "order");

        let backorder = commerce
            .backorders()
            .create(stateset_core::CreateBackorder {
                order_id: ord_uuid,
                sku,
                quantity,
                expected_date,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create backorder: {}", e)))?;

        Ok(backorder.id.to_string())
    }

    pub fn get(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "backorder");

        let backorder = commerce
            .backorders()
            .get(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get backorder: {}", e)))?;

        Ok(backorder.map(|b| b.backorder_number))
    }

    pub fn list(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let backorders = commerce
            .backorders()
            .list(Default::default())
            .map_err(|e| PhpException::default(format!("Failed to list backorders: {}", e)))?;

        Ok(backorders.into_iter().map(|b| b.id.to_string()).collect())
    }

    pub fn cancel(&self, id: String) -> PhpResult<bool> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "backorder");

        commerce
            .backorders()
            .cancel(uuid)
            .map_err(|e| PhpException::default(format!("Failed to cancel backorder: {}", e)))?;

        Ok(true)
    }

    pub fn count_pending(&self) -> PhpResult<i32> {
        let commerce = lock_commerce!(self.commerce);

        let count = commerce
            .backorders()
            .count_pending()
            .map_err(|e| PhpException::default(format!("Failed to count: {}", e)))?;

        Ok(count as i32)
    }
}

// ============================================================================
// General Ledger API
// ============================================================================

#[php_class(name = "StateSet\\GeneralLedger")]
#[derive(Clone)]
pub struct GeneralLedger {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[php_impl]
impl GeneralLedger {
    pub fn create_account(
        &self,
        account_number: String,
        name: String,
        account_type: String,
    ) -> PhpResult<String> {
        let commerce = lock_commerce!(self.commerce);

        let account = commerce
            .general_ledger()
            .create_account(stateset_core::CreateGlAccount {
                account_number,
                name,
                account_type,
                ..Default::default()
            })
            .map_err(|e| PhpException::default(format!("Failed to create account: {}", e)))?;

        Ok(account.id.to_string())
    }

    pub fn get_account(&self, id: String) -> PhpResult<Option<String>> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(id, "account");

        let account = commerce
            .general_ledger()
            .get_account(uuid)
            .map_err(|e| PhpException::default(format!("Failed to get account: {}", e)))?;

        Ok(account.map(|a| a.name))
    }

    pub fn list_accounts(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let accounts = commerce
            .general_ledger()
            .list_accounts()
            .map_err(|e| PhpException::default(format!("Failed to list accounts: {}", e)))?;

        Ok(accounts.into_iter().map(|a| a.id.to_string()).collect())
    }

    pub fn get_account_balance(
        &self,
        account_id: String,
        as_of_date: Option<String>,
    ) -> PhpResult<f64> {
        let commerce = lock_commerce!(self.commerce);
        let uuid = parse_uuid!(account_id, "account");

        let balance = commerce
            .general_ledger()
            .get_account_balance(uuid, as_of_date.as_deref())
            .map_err(|e| PhpException::default(format!("Failed to get balance: {}", e)))?;

        to_f64_result(balance, "account balance")
    }

    pub fn initialize_chart_of_accounts(&self) -> PhpResult<Vec<String>> {
        let commerce = lock_commerce!(self.commerce);

        let accounts = commerce
            .general_ledger()
            .initialize_chart_of_accounts()
            .map_err(|e| PhpException::default(format!("Failed to initialize COA: {}", e)))?;

        Ok(accounts.into_iter().map(|a| a.id.to_string()).collect())
    }
}

// ============================================================================
// Module Registration
// ============================================================================

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
}
