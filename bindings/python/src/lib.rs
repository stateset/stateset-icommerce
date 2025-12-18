//! Python bindings for StateSet Embedded Commerce
//!
//! Provides a local-first commerce library with SQLite storage.
//!
//! ```python
//! from stateset_embedded import Commerce
//!
//! commerce = Commerce("./store.db")
//! customer = commerce.customers.create(
//!     email="alice@example.com",
//!     first_name="Alice",
//!     last_name="Smith"
//! )
//! ```

use pyo3::prelude::*;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use rust_decimal::Decimal;
// Use :: prefix to refer to the external crate, not the pymodule
use ::stateset_embedded::Commerce as RustCommerce;
use std::sync::{Arc, Mutex};

// ============================================================================
// Commerce
// ============================================================================

/// Main Commerce instance for local commerce operations.
///
/// Example:
///     commerce = Commerce("./store.db")
///     commerce = Commerce(":memory:")  # In-memory database
#[pyclass]
pub struct Commerce {
    inner: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Commerce {
    /// Create a new Commerce instance with a database path.
    ///
    /// Args:
    ///     db_path: Path to SQLite database file, or ":memory:" for in-memory.
    #[new]
    fn new(db_path: String) -> PyResult<Self> {
        let commerce = RustCommerce::new(&db_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to initialize commerce: {}", e)))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(commerce)),
        })
    }

    /// Get the customers API.
    #[getter]
    fn customers(&self) -> Customers {
        Customers {
            commerce: self.inner.clone(),
        }
    }

    /// Get the orders API.
    #[getter]
    fn orders(&self) -> Orders {
        Orders {
            commerce: self.inner.clone(),
        }
    }

    /// Get the products API.
    #[getter]
    fn products(&self) -> Products {
        Products {
            commerce: self.inner.clone(),
        }
    }

    /// Get the inventory API.
    #[getter]
    fn inventory(&self) -> Inventory {
        Inventory {
            commerce: self.inner.clone(),
        }
    }

    /// Get the returns API.
    #[getter]
    fn returns(&self) -> Returns {
        Returns {
            commerce: self.inner.clone(),
        }
    }

    /// Get the payments API.
    #[getter]
    fn payments(&self) -> Payments {
        Payments {
            commerce: self.inner.clone(),
        }
    }

    /// Get the shipments API.
    #[getter]
    fn shipments(&self) -> Shipments {
        Shipments {
            commerce: self.inner.clone(),
        }
    }

    /// Get the warranties API.
    #[getter]
    fn warranties(&self) -> Warranties {
        Warranties {
            commerce: self.inner.clone(),
        }
    }

    /// Get the purchase orders API.
    #[getter]
    fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders {
            commerce: self.inner.clone(),
        }
    }

    /// Get the invoices API.
    #[getter]
    fn invoices(&self) -> Invoices {
        Invoices {
            commerce: self.inner.clone(),
        }
    }

    /// Get the bill of materials API.
    #[getter]
    fn bom(&self) -> BomApi {
        BomApi {
            commerce: self.inner.clone(),
        }
    }

    /// Get the work orders API.
    #[getter]
    fn work_orders(&self) -> WorkOrders {
        WorkOrders {
            commerce: self.inner.clone(),
        }
    }

    /// Get the carts API.
    #[getter]
    fn carts(&self) -> Carts {
        Carts {
            commerce: self.inner.clone(),
        }
    }

    /// Get the analytics API.
    #[getter]
    fn analytics(&self) -> Analytics {
        Analytics {
            commerce: self.inner.clone(),
        }
    }

    /// Get the currency API.
    #[getter]
    fn currency(&self) -> CurrencyOperations {
        CurrencyOperations {
            commerce: self.inner.clone(),
        }
    }
}

// ============================================================================
// Customer Types
// ============================================================================

/// Customer data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Customer {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    email: String,
    #[pyo3(get)]
    first_name: String,
    #[pyo3(get)]
    last_name: String,
    #[pyo3(get)]
    phone: Option<String>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    accepts_marketing: bool,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl Customer {
    fn __repr__(&self) -> String {
        format!(
            "Customer(id='{}', email='{}', name='{} {}')",
            self.id, self.email, self.first_name, self.last_name
        )
    }

    /// Get the full name.
    #[getter]
    fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
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

/// Customer management operations.
#[pyclass]
pub struct Customers {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Customers {
    /// Create a new customer.
    ///
    /// Args:
    ///     email: Customer email address (required)
    ///     first_name: First name (required)
    ///     last_name: Last name (required)
    ///     phone: Phone number (optional)
    ///     accepts_marketing: Marketing opt-in (optional, default False)
    ///
    /// Returns:
    ///     Customer: The created customer
    #[pyo3(signature = (email, first_name, last_name, phone=None, accepts_marketing=None))]
    fn create(
        &self,
        email: String,
        first_name: String,
        last_name: String,
        phone: Option<String>,
        accepts_marketing: Option<bool>,
    ) -> PyResult<Customer> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

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
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create customer: {}", e)))?;

        Ok(customer.into())
    }

    /// Get a customer by ID.
    ///
    /// Args:
    ///     id: Customer UUID
    ///
    /// Returns:
    ///     Customer or None if not found
    fn get(&self, id: String) -> PyResult<Option<Customer>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let customer = commerce
            .customers()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get customer: {}", e)))?;

        Ok(customer.map(|c| c.into()))
    }

    /// Get a customer by email.
    ///
    /// Args:
    ///     email: Customer email address
    ///
    /// Returns:
    ///     Customer or None if not found
    fn get_by_email(&self, email: String) -> PyResult<Option<Customer>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let customer = commerce
            .customers()
            .get_by_email(&email)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get customer: {}", e)))?;

        Ok(customer.map(|c| c.into()))
    }

    /// List all customers.
    ///
    /// Returns:
    ///     List[Customer]: All customers
    fn list(&self) -> PyResult<Vec<Customer>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let customers = commerce
            .customers()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list customers: {}", e)))?;

        Ok(customers.into_iter().map(|c| c.into()).collect())
    }

    /// Count customers.
    ///
    /// Returns:
    ///     int: Number of customers
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .customers()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count customers: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Order Types
// ============================================================================

/// Order line item.
#[pyclass]
#[derive(Clone)]
pub struct OrderItem {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    quantity: i32,
    #[pyo3(get)]
    unit_price: f64,
    #[pyo3(get)]
    total: f64,
}

#[pymethods]
impl OrderItem {
    fn __repr__(&self) -> String {
        format!("OrderItem(sku='{}', qty={}, price={})", self.sku, self.quantity, self.unit_price)
    }
}

/// Order data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Order {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    order_number: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    total_amount: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    payment_status: String,
    #[pyo3(get)]
    fulfillment_status: String,
    #[pyo3(get)]
    tracking_number: Option<String>,
    #[pyo3(get)]
    items: Vec<OrderItem>,
    #[pyo3(get)]
    version: i32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl Order {
    fn __repr__(&self) -> String {
        format!(
            "Order(number='{}', status='{}', total={} {})",
            self.order_number, self.status, self.total_amount, self.currency
        )
    }

    /// Get the number of items in the order.
    #[getter]
    fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl From<stateset_core::Order> for Order {
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
                .map(|i| OrderItem {
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

/// Input for creating an order item.
#[pyclass]
#[derive(Clone)]
pub struct CreateOrderItemInput {
    #[pyo3(get, set)]
    sku: String,
    #[pyo3(get, set)]
    name: String,
    #[pyo3(get, set)]
    quantity: i32,
    #[pyo3(get, set)]
    unit_price: f64,
    #[pyo3(get, set)]
    product_id: Option<String>,
    #[pyo3(get, set)]
    variant_id: Option<String>,
}

#[pymethods]
impl CreateOrderItemInput {
    #[new]
    #[pyo3(signature = (sku, name, quantity, unit_price, product_id=None, variant_id=None))]
    fn new(
        sku: String,
        name: String,
        quantity: i32,
        unit_price: f64,
        product_id: Option<String>,
        variant_id: Option<String>,
    ) -> Self {
        Self {
            sku,
            name,
            quantity,
            unit_price,
            product_id,
            variant_id,
        }
    }
}

// ============================================================================
// Orders API
// ============================================================================

/// Order management operations.
#[pyclass]
pub struct Orders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Orders {
    /// Create a new order.
    ///
    /// Args:
    ///     customer_id: Customer UUID
    ///     items: List of CreateOrderItemInput
    ///     currency: Currency code (default "USD")
    ///     notes: Order notes (optional)
    ///
    /// Returns:
    ///     Order: The created order
    #[pyo3(signature = (customer_id, items, currency=None, notes=None))]
    fn create(
        &self,
        customer_id: String,
        items: Vec<CreateOrderItemInput>,
        currency: Option<String>,
        notes: Option<String>,
    ) -> PyResult<Order> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid = customer_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let order_items: Vec<stateset_core::CreateOrderItem> = items
            .into_iter()
            .map(|i| {
                let product_id = i.product_id.and_then(|s| s.parse().ok()).unwrap_or_default();
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
                customer_id: cust_uuid,
                items: order_items,
                currency,
                notes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create order: {}", e)))?;

        Ok(order.into())
    }

    /// Get an order by ID.
    ///
    /// Args:
    ///     id: Order UUID
    ///
    /// Returns:
    ///     Order or None if not found
    fn get(&self, id: String) -> PyResult<Option<Order>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order = commerce
            .orders()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get order: {}", e)))?;

        Ok(order.map(|o| o.into()))
    }

    /// List all orders.
    ///
    /// Returns:
    ///     List[Order]: All orders
    fn list(&self) -> PyResult<Vec<Order>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let orders = commerce
            .orders()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list orders: {}", e)))?;

        Ok(orders.into_iter().map(|o| o.into()).collect())
    }

    /// Update order status.
    ///
    /// Args:
    ///     id: Order UUID
    ///     status: New status (pending, confirmed, processing, shipped, delivered, cancelled, refunded)
    ///
    /// Returns:
    ///     Order: The updated order
    fn update_status(&self, id: String, status: String) -> PyResult<Order> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order_status = match status.to_lowercase().as_str() {
            "pending" => stateset_core::OrderStatus::Pending,
            "confirmed" => stateset_core::OrderStatus::Confirmed,
            "processing" => stateset_core::OrderStatus::Processing,
            "shipped" => stateset_core::OrderStatus::Shipped,
            "delivered" => stateset_core::OrderStatus::Delivered,
            "cancelled" => stateset_core::OrderStatus::Cancelled,
            "refunded" => stateset_core::OrderStatus::Refunded,
            _ => return Err(PyValueError::new_err(format!("Invalid status: {}", status))),
        };

        let order = commerce
            .orders()
            .update_status(uuid, order_status)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update order: {}", e)))?;

        Ok(order.into())
    }

    /// Ship an order.
    ///
    /// Args:
    ///     id: Order UUID
    ///     tracking_number: Tracking number (optional)
    ///
    /// Returns:
    ///     Order: The shipped order
    #[pyo3(signature = (id, tracking_number=None))]
    fn ship(&self, id: String, tracking_number: Option<String>) -> PyResult<Order> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order = commerce
            .orders()
            .ship(uuid, tracking_number.as_deref())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to ship order: {}", e)))?;

        Ok(order.into())
    }

    /// Cancel an order.
    ///
    /// Args:
    ///     id: Order UUID
    ///
    /// Returns:
    ///     Order: The cancelled order
    fn cancel(&self, id: String) -> PyResult<Order> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order = commerce
            .orders()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel order: {}", e)))?;

        Ok(order.into())
    }

    /// Count orders.
    ///
    /// Returns:
    ///     int: Number of orders
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .orders()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count orders: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Product Types
// ============================================================================

/// Product data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Product {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    slug: String,
    #[pyo3(get)]
    description: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl Product {
    fn __repr__(&self) -> String {
        format!("Product(name='{}', slug='{}', status='{}')", self.name, self.slug, self.status)
    }
}

impl From<stateset_core::Product> for Product {
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

/// Product variant data.
#[pyclass]
#[derive(Clone)]
pub struct ProductVariant {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    price: f64,
    #[pyo3(get)]
    compare_at_price: Option<f64>,
    #[pyo3(get)]
    is_default: bool,
}

#[pymethods]
impl ProductVariant {
    fn __repr__(&self) -> String {
        format!("ProductVariant(sku='{}', price={})", self.sku, self.price)
    }
}

impl From<stateset_core::ProductVariant> for ProductVariant {
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

/// Input for creating a product variant.
#[pyclass]
#[derive(Clone)]
pub struct CreateProductVariantInput {
    #[pyo3(get, set)]
    sku: String,
    #[pyo3(get, set)]
    name: Option<String>,
    #[pyo3(get, set)]
    price: f64,
    #[pyo3(get, set)]
    compare_at_price: Option<f64>,
}

#[pymethods]
impl CreateProductVariantInput {
    #[new]
    #[pyo3(signature = (sku, price, name=None, compare_at_price=None))]
    fn new(sku: String, price: f64, name: Option<String>, compare_at_price: Option<f64>) -> Self {
        Self {
            sku,
            name,
            price,
            compare_at_price,
        }
    }
}

// ============================================================================
// Products API
// ============================================================================

/// Product catalog operations.
#[pyclass]
pub struct Products {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Products {
    /// Create a new product.
    ///
    /// Args:
    ///     name: Product name
    ///     description: Product description (optional)
    ///     variants: List of CreateProductVariantInput (optional)
    ///
    /// Returns:
    ///     Product: The created product
    #[pyo3(signature = (name, description=None, variants=None))]
    fn create(
        &self,
        name: String,
        description: Option<String>,
        variants: Option<Vec<CreateProductVariantInput>>,
    ) -> PyResult<Product> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let variant_inputs = variants.map(|vs| {
            vs.into_iter()
                .map(|v| stateset_core::CreateProductVariant {
                    sku: v.sku,
                    name: v.name,
                    price: Decimal::from_f64_retain(v.price).unwrap_or_default(),
                    compare_at_price: v.compare_at_price.and_then(|p| Decimal::from_f64_retain(p)),
                    ..Default::default()
                })
                .collect()
        });

        let product = commerce
            .products()
            .create(stateset_core::CreateProduct {
                name,
                description,
                variants: variant_inputs,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create product: {}", e)))?;

        Ok(product.into())
    }

    /// Get a product by ID.
    ///
    /// Args:
    ///     id: Product UUID
    ///
    /// Returns:
    ///     Product or None if not found
    fn get(&self, id: String) -> PyResult<Option<Product>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let product = commerce
            .products()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get product: {}", e)))?;

        Ok(product.map(|p| p.into()))
    }

    /// Get a product variant by SKU.
    ///
    /// Args:
    ///     sku: Product variant SKU
    ///
    /// Returns:
    ///     ProductVariant or None if not found
    fn get_variant_by_sku(&self, sku: String) -> PyResult<Option<ProductVariant>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let variant = commerce
            .products()
            .get_variant_by_sku(&sku)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get variant: {}", e)))?;

        Ok(variant.map(|v| v.into()))
    }

    /// List all products.
    ///
    /// Returns:
    ///     List[Product]: All products
    fn list(&self) -> PyResult<Vec<Product>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let products = commerce
            .products()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list products: {}", e)))?;

        Ok(products.into_iter().map(|p| p.into()).collect())
    }

    /// Count products.
    ///
    /// Returns:
    ///     int: Number of products
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .products()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count products: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Inventory Types
// ============================================================================

/// Inventory item data.
#[pyclass]
#[derive(Clone)]
pub struct InventoryItem {
    #[pyo3(get)]
    id: i64,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    unit_of_measure: String,
    #[pyo3(get)]
    is_active: bool,
}

#[pymethods]
impl InventoryItem {
    fn __repr__(&self) -> String {
        format!("InventoryItem(sku='{}', name='{}')", self.sku, self.name)
    }
}

impl From<stateset_core::InventoryItem> for InventoryItem {
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

/// Stock level information.
#[pyclass]
#[derive(Clone)]
pub struct StockLevel {
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    total_on_hand: f64,
    #[pyo3(get)]
    total_allocated: f64,
    #[pyo3(get)]
    total_available: f64,
}

#[pymethods]
impl StockLevel {
    fn __repr__(&self) -> String {
        format!(
            "StockLevel(sku='{}', available={})",
            self.sku, self.total_available
        )
    }
}

impl From<stateset_core::StockLevel> for StockLevel {
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

/// Inventory reservation.
#[pyclass]
#[derive(Clone)]
pub struct Reservation {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    item_id: i64,
    #[pyo3(get)]
    quantity: f64,
    #[pyo3(get)]
    status: String,
}

#[pymethods]
impl Reservation {
    fn __repr__(&self) -> String {
        format!("Reservation(id='{}', qty={}, status='{}')", self.id, self.quantity, self.status)
    }
}

impl From<stateset_core::InventoryReservation> for Reservation {
    fn from(r: stateset_core::InventoryReservation) -> Self {
        Self {
            id: r.id.to_string(),
            item_id: r.item_id,
            quantity: r.quantity.to_string().parse().unwrap_or(0.0),
            status: format!("{}", r.status),
        }
    }
}

// ============================================================================
// Inventory API
// ============================================================================

/// Inventory management operations.
#[pyclass]
pub struct Inventory {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Inventory {
    /// Create a new inventory item.
    ///
    /// Args:
    ///     sku: Stock keeping unit
    ///     name: Item name
    ///     description: Item description (optional)
    ///     initial_quantity: Starting quantity (optional, default 0)
    ///     reorder_point: Reorder alert threshold (optional)
    ///
    /// Returns:
    ///     InventoryItem: The created item
    #[pyo3(signature = (sku, name, description=None, initial_quantity=None, reorder_point=None))]
    fn create_item(
        &self,
        sku: String,
        name: String,
        description: Option<String>,
        initial_quantity: Option<f64>,
        reorder_point: Option<f64>,
    ) -> PyResult<InventoryItem> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let item = commerce
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku,
                name,
                description,
                initial_quantity: initial_quantity.and_then(|q| Decimal::from_f64_retain(q)),
                reorder_point: reorder_point.and_then(|r| Decimal::from_f64_retain(r)),
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create inventory item: {}", e)))?;

        Ok(item.into())
    }

    /// Get stock level for a SKU.
    ///
    /// Args:
    ///     sku: Stock keeping unit
    ///
    /// Returns:
    ///     StockLevel or None if not found
    fn get_stock(&self, sku: String) -> PyResult<Option<StockLevel>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let stock = commerce
            .inventory()
            .get_stock(&sku)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get stock: {}", e)))?;

        Ok(stock.map(|s| s.into()))
    }

    /// Adjust inventory quantity.
    ///
    /// Args:
    ///     sku: Stock keeping unit
    ///     quantity: Quantity to add (positive) or remove (negative)
    ///     reason: Reason for adjustment
    fn adjust(&self, sku: String, quantity: f64, reason: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| PyValueError::new_err("Invalid quantity"))?;

        commerce
            .inventory()
            .adjust(&sku, qty, &reason)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to adjust inventory: {}", e)))?;

        Ok(())
    }

    /// Reserve inventory for an order.
    ///
    /// Args:
    ///     sku: Stock keeping unit
    ///     quantity: Quantity to reserve
    ///     reference_type: Type of reference (e.g., "order")
    ///     reference_id: Reference identifier
    ///     expires_in_seconds: Reservation expiry time (optional)
    ///
    /// Returns:
    ///     Reservation: The created reservation
    #[pyo3(signature = (sku, quantity, reference_type, reference_id, expires_in_seconds=None))]
    fn reserve(
        &self,
        sku: String,
        quantity: f64,
        reference_type: String,
        reference_id: String,
        expires_in_seconds: Option<i64>,
    ) -> PyResult<Reservation> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| PyValueError::new_err("Invalid quantity"))?;

        let reservation = commerce
            .inventory()
            .reserve(&sku, qty, &reference_type, &reference_id, expires_in_seconds)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to reserve inventory: {}", e)))?;

        Ok(reservation.into())
    }

    /// Confirm a reservation (deducts from on-hand).
    ///
    /// Args:
    ///     reservation_id: Reservation UUID
    fn confirm_reservation(&self, reservation_id: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = reservation_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .inventory()
            .confirm_reservation(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to confirm reservation: {}", e)))?;

        Ok(())
    }

    /// Release a reservation (returns to available).
    ///
    /// Args:
    ///     reservation_id: Reservation UUID
    fn release_reservation(&self, reservation_id: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = reservation_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .inventory()
            .release_reservation(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to release reservation: {}", e)))?;

        Ok(())
    }
}

// ============================================================================
// Return Types
// ============================================================================

/// Return request data.
#[pyclass]
#[derive(Clone)]
pub struct Return {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    order_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    reason: String,
    #[pyo3(get)]
    version: i32,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Return {
    fn __repr__(&self) -> String {
        format!("Return(id='{}', status='{}', reason='{}')", self.id, self.status, self.reason)
    }
}

impl From<stateset_core::Return> for Return {
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

/// Input for creating a return item.
#[pyclass]
#[derive(Clone)]
pub struct CreateReturnItemInput {
    #[pyo3(get, set)]
    order_item_id: String,
    #[pyo3(get, set)]
    quantity: i32,
}

#[pymethods]
impl CreateReturnItemInput {
    #[new]
    fn new(order_item_id: String, quantity: i32) -> Self {
        Self {
            order_item_id,
            quantity,
        }
    }
}

// ============================================================================
// Returns API
// ============================================================================

/// Return processing operations.
#[pyclass]
pub struct Returns {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Returns {
    /// Create a new return request.
    ///
    /// Args:
    ///     order_id: Order UUID
    ///     reason: Return reason (defective, not_as_described, wrong_item, etc.)
    ///     items: List of CreateReturnItemInput
    ///     reason_details: Additional details (optional)
    ///
    /// Returns:
    ///     Return: The created return
    #[pyo3(signature = (order_id, reason, items, reason_details=None))]
    fn create(
        &self,
        order_id: String,
        reason: String,
        items: Vec<CreateReturnItemInput>,
        reason_details: Option<String>,
    ) -> PyResult<Return> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let ord_uuid = order_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let return_reason = match reason.to_lowercase().as_str() {
            "defective" => stateset_core::ReturnReason::Defective,
            "not_as_described" => stateset_core::ReturnReason::NotAsDescribed,
            "wrong_item" => stateset_core::ReturnReason::WrongItem,
            "no_longer_needed" => stateset_core::ReturnReason::NoLongerNeeded,
            "changed_mind" => stateset_core::ReturnReason::ChangedMind,
            "better_price_found" => stateset_core::ReturnReason::BetterPriceFound,
            "damaged" => stateset_core::ReturnReason::Damaged,
            _ => stateset_core::ReturnReason::Other,
        };

        let return_items: Vec<stateset_core::CreateReturnItem> = items
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
                order_id: ord_uuid,
                reason: return_reason,
                reason_details,
                items: return_items,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create return: {}", e)))?;

        Ok(ret.into())
    }

    /// Get a return by ID.
    ///
    /// Args:
    ///     id: Return UUID
    ///
    /// Returns:
    ///     Return or None if not found
    fn get(&self, id: String) -> PyResult<Option<Return>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get return: {}", e)))?;

        Ok(ret.map(|r| r.into()))
    }

    /// Approve a return request.
    ///
    /// Args:
    ///     id: Return UUID
    ///
    /// Returns:
    ///     Return: The approved return
    fn approve(&self, id: String) -> PyResult<Return> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .approve(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to approve return: {}", e)))?;

        Ok(ret.into())
    }

    /// Reject a return request.
    ///
    /// Args:
    ///     id: Return UUID
    ///     reason: Rejection reason
    ///
    /// Returns:
    ///     Return: The rejected return
    fn reject(&self, id: String, reason: String) -> PyResult<Return> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .reject(uuid, &reason)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to reject return: {}", e)))?;

        Ok(ret.into())
    }

    /// List all returns.
    ///
    /// Returns:
    ///     List[Return]: All returns
    fn list(&self) -> PyResult<Vec<Return>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let returns = commerce
            .returns()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list returns: {}", e)))?;

        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    /// Count returns.
    ///
    /// Returns:
    ///     int: Number of returns
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .returns()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count returns: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Payment Types
// ============================================================================

/// Payment data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Payment {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    payment_number: String,
    #[pyo3(get)]
    order_id: Option<String>,
    #[pyo3(get)]
    invoice_id: Option<String>,
    #[pyo3(get)]
    customer_id: Option<String>,
    #[pyo3(get)]
    amount: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    payment_method: String,
    #[pyo3(get)]
    version: i32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl Payment {
    fn __repr__(&self) -> String {
        format!(
            "Payment(number='{}', amount={} {}, status='{}')",
            self.payment_number, self.amount, self.currency, self.status
        )
    }
}

impl From<stateset_core::Payment> for Payment {
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
            payment_method: format!("{}", p.payment_method),
            version: p.version,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

/// Refund data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Refund {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    payment_id: String,
    #[pyo3(get)]
    amount: f64,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    reason: Option<String>,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Refund {
    fn __repr__(&self) -> String {
        format!("Refund(id='{}', amount={}, status='{}')", self.id, self.amount, self.status)
    }
}

impl From<stateset_core::Refund> for Refund {
    fn from(r: stateset_core::Refund) -> Self {
        Self {
            id: r.id.to_string(),
            payment_id: r.payment_id.to_string(),
            amount: r.amount.to_string().parse().unwrap_or(0.0),
            status: format!("{}", r.status),
            reason: r.reason,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Payments API
// ============================================================================

/// Payment processing operations.
#[pyclass]
pub struct Payments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Payments {
    /// Create a new payment.
    ///
    /// Args:
    ///     amount: Payment amount
    ///     currency: Currency code (default "USD")
    ///     order_id: Associated order UUID (optional)
    ///     customer_id: Customer UUID (optional)
    ///     payment_method: Payment method type (optional)
    ///
    /// Returns:
    ///     Payment: The created payment
    #[pyo3(signature = (amount, currency=None, order_id=None, customer_id=None, payment_method=None))]
    fn create(
        &self,
        amount: f64,
        currency: Option<String>,
        order_id: Option<String>,
        customer_id: Option<String>,
        payment_method: Option<String>,
    ) -> PyResult<Payment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let order_uuid = order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let customer_uuid = customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let method = payment_method.map(|m| match m.to_lowercase().as_str() {
            "credit_card" => stateset_core::PaymentMethodType::CreditCard,
            "debit_card" => stateset_core::PaymentMethodType::DebitCard,
            "bank_transfer" => stateset_core::PaymentMethodType::BankTransfer,
            "paypal" => stateset_core::PaymentMethodType::PayPal,
            "crypto" => stateset_core::PaymentMethodType::Crypto,
            _ => stateset_core::PaymentMethodType::CreditCard,
        }).unwrap_or(stateset_core::PaymentMethodType::CreditCard);

        let payment = commerce
            .payments()
            .create(stateset_core::CreatePayment {
                order_id: order_uuid,
                customer_id: customer_uuid,
                amount: Decimal::from_f64_retain(amount).unwrap_or_default(),
                currency,
                payment_method: method,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create payment: {}", e)))?;

        Ok(payment.into())
    }

    /// Get a payment by ID.
    fn get(&self, id: String) -> PyResult<Option<Payment>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get payment: {}", e)))?;

        Ok(payment.map(|p| p.into()))
    }

    /// List all payments.
    fn list(&self) -> PyResult<Vec<Payment>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let payments = commerce
            .payments()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list payments: {}", e)))?;

        Ok(payments.into_iter().map(|p| p.into()).collect())
    }

    /// Mark payment as completed.
    fn complete(&self, id: String) -> PyResult<Payment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_completed(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to complete payment: {}", e)))?;

        Ok(payment.into())
    }

    /// Mark payment as failed.
    #[pyo3(signature = (id, reason, code=None))]
    fn mark_failed(&self, id: String, reason: String, code: Option<String>) -> PyResult<Payment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_failed(uuid, &reason, code.as_deref())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to fail payment: {}", e)))?;

        Ok(payment.into())
    }

    /// Create a refund for a payment.
    #[pyo3(signature = (payment_id, amount, reason=None))]
    fn create_refund(&self, payment_id: String, amount: f64, reason: Option<String>) -> PyResult<Refund> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = payment_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid payment UUID"))?;

        let refund = commerce
            .payments()
            .create_refund(stateset_core::CreateRefund {
                payment_id: uuid,
                amount: Some(Decimal::from_f64_retain(amount).unwrap_or_default()),
                reason,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create refund: {}", e)))?;

        Ok(refund.into())
    }

    /// Count payments.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .payments()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count payments: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Shipment Types
// ============================================================================

/// Shipment data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Shipment {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    shipment_number: String,
    #[pyo3(get)]
    order_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    carrier: String,
    #[pyo3(get)]
    shipping_method: String,
    #[pyo3(get)]
    tracking_number: Option<String>,
    #[pyo3(get)]
    tracking_url: Option<String>,
    #[pyo3(get)]
    recipient_name: String,
    #[pyo3(get)]
    shipping_address: String,
    #[pyo3(get)]
    version: i32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl Shipment {
    fn __repr__(&self) -> String {
        format!(
            "Shipment(number='{}', status='{}', carrier='{}')",
            self.shipment_number, self.status, self.carrier
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

// ============================================================================
// Shipments API
// ============================================================================

/// Shipment management operations.
#[pyclass]
pub struct Shipments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Shipments {
    /// Create a new shipment.
    #[pyo3(signature = (order_id, recipient_name, shipping_address, carrier=None, shipping_method=None, tracking_number=None))]
    fn create(
        &self,
        order_id: String,
        recipient_name: String,
        shipping_address: String,
        carrier: Option<String>,
        shipping_method: Option<String>,
        tracking_number: Option<String>,
    ) -> PyResult<Shipment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let order_uuid = order_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let carrier_type = carrier.and_then(|c| match c.to_lowercase().as_str() {
            "ups" => Some(stateset_core::ShippingCarrier::Ups),
            "fedex" => Some(stateset_core::ShippingCarrier::FedEx),
            "usps" => Some(stateset_core::ShippingCarrier::Usps),
            "dhl" => Some(stateset_core::ShippingCarrier::Dhl),
            _ => Some(stateset_core::ShippingCarrier::Other),
        });

        let method = shipping_method.and_then(|m| match m.to_lowercase().as_str() {
            "standard" => Some(stateset_core::ShippingMethod::Standard),
            "express" => Some(stateset_core::ShippingMethod::Express),
            "overnight" => Some(stateset_core::ShippingMethod::Overnight),
            "ground" => Some(stateset_core::ShippingMethod::Ground),
            _ => Some(stateset_core::ShippingMethod::Standard),
        });

        let shipment = commerce
            .shipments()
            .create(stateset_core::CreateShipment {
                order_id: order_uuid,
                recipient_name,
                shipping_address,
                carrier: carrier_type,
                shipping_method: method,
                tracking_number,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create shipment: {}", e)))?;

        Ok(shipment.into())
    }

    /// Get a shipment by ID.
    fn get(&self, id: String) -> PyResult<Option<Shipment>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get shipment: {}", e)))?;

        Ok(shipment.map(|s| s.into()))
    }

    /// List all shipments.
    fn list(&self) -> PyResult<Vec<Shipment>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let shipments = commerce
            .shipments()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list shipments: {}", e)))?;

        Ok(shipments.into_iter().map(|s| s.into()).collect())
    }

    /// Ship a shipment with optional tracking number.
    #[pyo3(signature = (id, tracking_number=None))]
    fn ship(&self, id: String, tracking_number: Option<String>) -> PyResult<Shipment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .ship(uuid, tracking_number)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to ship: {}", e)))?;

        Ok(shipment.into())
    }

    /// Mark shipment as delivered.
    fn mark_delivered(&self, id: String) -> PyResult<Shipment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .mark_delivered(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to deliver: {}", e)))?;

        Ok(shipment.into())
    }

    /// Cancel a shipment.
    fn cancel(&self, id: String) -> PyResult<Shipment> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel shipment: {}", e)))?;

        Ok(shipment.into())
    }

    /// Count shipments.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .shipments()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count shipments: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Warranty Types
// ============================================================================

/// Warranty data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Warranty {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    warranty_number: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    product_id: Option<String>,
    #[pyo3(get)]
    order_id: Option<String>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    warranty_type: String,
    #[pyo3(get)]
    start_date: String,
    #[pyo3(get)]
    end_date: String,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Warranty {
    fn __repr__(&self) -> String {
        format!(
            "Warranty(number='{}', status='{}', type='{}')",
            self.warranty_number, self.status, self.warranty_type
        )
    }
}

impl From<stateset_core::Warranty> for Warranty {
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

/// Warranty claim data.
#[pyclass]
#[derive(Clone)]
pub struct WarrantyClaim {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    claim_number: String,
    #[pyo3(get)]
    warranty_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    issue_description: String,
    #[pyo3(get)]
    resolution: String,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl WarrantyClaim {
    fn __repr__(&self) -> String {
        format!("WarrantyClaim(number='{}', status='{}')", self.claim_number, self.status)
    }
}

impl From<stateset_core::WarrantyClaim> for WarrantyClaim {
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

// ============================================================================
// Warranties API
// ============================================================================

/// Warranty management operations.
#[pyclass]
pub struct Warranties {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Warranties {
    /// Register a new warranty.
    #[pyo3(signature = (customer_id, product_id=None, order_id=None, duration_months=None))]
    fn create(
        &self,
        customer_id: String,
        product_id: Option<String>,
        order_id: Option<String>,
        duration_months: Option<i32>,
    ) -> PyResult<Warranty> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid = customer_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let prod_uuid = product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

        let order_uuid = order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let warranty = commerce
            .warranties()
            .create(stateset_core::CreateWarranty {
                customer_id: cust_uuid,
                product_id: prod_uuid,
                order_id: order_uuid,
                duration_months,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create warranty: {}", e)))?;

        Ok(warranty.into())
    }

    /// Get a warranty by ID.
    fn get(&self, id: String) -> PyResult<Option<Warranty>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let warranty = commerce
            .warranties()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get warranty: {}", e)))?;

        Ok(warranty.map(|w| w.into()))
    }

    /// List all warranties.
    fn list(&self) -> PyResult<Vec<Warranty>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let warranties = commerce
            .warranties()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list warranties: {}", e)))?;

        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    /// File a warranty claim.
    #[pyo3(signature = (warranty_id, issue_description, contact_email=None))]
    fn create_claim(
        &self,
        warranty_id: String,
        issue_description: String,
        contact_email: Option<String>,
    ) -> PyResult<WarrantyClaim> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let warranty_uuid = warranty_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid warranty UUID"))?;

        let claim = commerce
            .warranties()
            .create_claim(stateset_core::CreateWarrantyClaim {
                warranty_id: warranty_uuid,
                issue_description,
                contact_email,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create claim: {}", e)))?;

        Ok(claim.into())
    }

    /// Approve a warranty claim.
    fn approve_claim(&self, id: String) -> PyResult<WarrantyClaim> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .approve_claim(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to approve claim: {}", e)))?;

        Ok(claim.into())
    }

    /// Deny a warranty claim.
    fn deny_claim(&self, id: String, reason: String) -> PyResult<WarrantyClaim> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .deny_claim(uuid, &reason)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to deny claim: {}", e)))?;

        Ok(claim.into())
    }

    /// Complete a warranty claim with resolution.
    fn complete_claim(&self, id: String, resolution: String) -> PyResult<WarrantyClaim> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let res = match resolution.to_lowercase().as_str() {
            "repair" => stateset_core::ClaimResolution::Repair,
            "replacement" => stateset_core::ClaimResolution::Replacement,
            "refund" => stateset_core::ClaimResolution::Refund,
            "store_credit" => stateset_core::ClaimResolution::StoreCredit,
            "denied" => stateset_core::ClaimResolution::Denied,
            _ => stateset_core::ClaimResolution::None,
        };

        let claim = commerce
            .warranties()
            .complete_claim(uuid, res)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to complete claim: {}", e)))?;

        Ok(claim.into())
    }

    /// Count warranties.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .warranties()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count warranties: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Purchase Order Types
// ============================================================================

/// Supplier data.
#[pyclass]
#[derive(Clone)]
pub struct Supplier {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    supplier_code: String,
    #[pyo3(get)]
    email: Option<String>,
    #[pyo3(get)]
    phone: Option<String>,
    #[pyo3(get)]
    is_active: bool,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Supplier {
    fn __repr__(&self) -> String {
        format!("Supplier(name='{}', code='{}')", self.name, self.supplier_code)
    }
}

impl From<stateset_core::Supplier> for Supplier {
    fn from(s: stateset_core::Supplier) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            supplier_code: s.supplier_code,
            email: s.email,
            phone: s.phone,
            is_active: s.is_active,
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

/// Purchase order data.
#[pyclass]
#[derive(Clone)]
pub struct PurchaseOrder {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    po_number: String,
    #[pyo3(get)]
    supplier_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    total_amount: f64,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl PurchaseOrder {
    fn __repr__(&self) -> String {
        format!("PurchaseOrder(number='{}', status='{}', total={})", self.po_number, self.status, self.total_amount)
    }
}

impl From<stateset_core::PurchaseOrder> for PurchaseOrder {
    fn from(po: stateset_core::PurchaseOrder) -> Self {
        Self {
            id: po.id.to_string(),
            po_number: po.po_number,
            supplier_id: po.supplier_id.to_string(),
            status: format!("{}", po.status),
            total_amount: po.total.to_string().parse().unwrap_or(0.0),
            created_at: po.created_at.to_rfc3339(),
            updated_at: po.updated_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Purchase Orders API
// ============================================================================

/// Purchase order management operations.
#[pyclass]
pub struct PurchaseOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl PurchaseOrders {
    /// Create a new supplier.
    #[pyo3(signature = (name, email=None, phone=None))]
    fn create_supplier(&self, name: String, email: Option<String>, phone: Option<String>) -> PyResult<Supplier> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let supplier = commerce
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                name,
                email,
                phone,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create supplier: {}", e)))?;

        Ok(supplier.into())
    }

    /// Get a supplier by ID.
    fn get_supplier(&self, id: String) -> PyResult<Option<Supplier>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let supplier = commerce
            .purchase_orders()
            .get_supplier(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get supplier: {}", e)))?;

        Ok(supplier.map(|s| s.into()))
    }

    /// List all suppliers.
    fn list_suppliers(&self) -> PyResult<Vec<Supplier>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let suppliers = commerce
            .purchase_orders()
            .list_suppliers(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list suppliers: {}", e)))?;

        Ok(suppliers.into_iter().map(|s| s.into()).collect())
    }

    /// Create a new purchase order.
    fn create(&self, supplier_id: String) -> PyResult<PurchaseOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let supp_uuid = supplier_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid supplier UUID"))?;

        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id: supp_uuid,
                items: vec![],
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create PO: {}", e)))?;

        Ok(po.into())
    }

    /// Get a purchase order by ID.
    fn get(&self, id: String) -> PyResult<Option<PurchaseOrder>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get PO: {}", e)))?;

        Ok(po.map(|p| p.into()))
    }

    /// List all purchase orders.
    fn list(&self) -> PyResult<Vec<PurchaseOrder>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let pos = commerce
            .purchase_orders()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list POs: {}", e)))?;

        Ok(pos.into_iter().map(|p| p.into()).collect())
    }

    /// Submit PO for approval.
    fn submit(&self, id: String) -> PyResult<PurchaseOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .submit(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to submit PO: {}", e)))?;

        Ok(po.into())
    }

    /// Approve a purchase order.
    fn approve(&self, id: String, approved_by: String) -> PyResult<PurchaseOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .approve(uuid, &approved_by)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to approve PO: {}", e)))?;

        Ok(po.into())
    }

    /// Send PO to supplier.
    fn send(&self, id: String) -> PyResult<PurchaseOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .send(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to send PO: {}", e)))?;

        Ok(po.into())
    }

    /// Cancel a purchase order.
    fn cancel(&self, id: String) -> PyResult<PurchaseOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel PO: {}", e)))?;

        Ok(po.into())
    }

    /// Count purchase orders.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .purchase_orders()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count POs: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Invoice Types
// ============================================================================

/// Invoice data.
#[pyclass]
#[derive(Clone)]
pub struct Invoice {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    invoice_number: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    order_id: Option<String>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    subtotal: f64,
    #[pyo3(get)]
    tax_amount: f64,
    #[pyo3(get)]
    total: f64,
    #[pyo3(get)]
    amount_paid: f64,
    #[pyo3(get)]
    due_date: String,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Invoice {
    fn __repr__(&self) -> String {
        format!("Invoice(number='{}', status='{}', total={})", self.invoice_number, self.status, self.total)
    }

    #[getter]
    fn balance_due(&self) -> f64 {
        self.total - self.amount_paid
    }
}

impl From<stateset_core::Invoice> for Invoice {
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
        }
    }
}

// ============================================================================
// Invoices API
// ============================================================================

/// Invoice management operations.
#[pyclass]
pub struct Invoices {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Invoices {
    /// Create a new invoice.
    #[pyo3(signature = (customer_id, order_id=None, billing_email=None))]
    fn create(
        &self,
        customer_id: String,
        order_id: Option<String>,
        billing_email: Option<String>,
    ) -> PyResult<Invoice> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid = customer_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let order_uuid = order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let invoice = commerce
            .invoices()
            .create(stateset_core::CreateInvoice {
                customer_id: cust_uuid,
                order_id: order_uuid,
                billing_email,
                items: vec![],
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create invoice: {}", e)))?;

        Ok(invoice.into())
    }

    /// Get an invoice by ID.
    fn get(&self, id: String) -> PyResult<Option<Invoice>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get invoice: {}", e)))?;

        Ok(invoice.map(|i| i.into()))
    }

    /// List all invoices.
    fn list(&self) -> PyResult<Vec<Invoice>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let invoices = commerce
            .invoices()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    /// Send an invoice.
    fn send(&self, id: String) -> PyResult<Invoice> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .send(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to send invoice: {}", e)))?;

        Ok(invoice.into())
    }

    /// Record a payment against an invoice.
    #[pyo3(signature = (id, amount, payment_method=None, reference=None))]
    fn record_payment(
        &self,
        id: String,
        amount: f64,
        payment_method: Option<String>,
        reference: Option<String>,
    ) -> PyResult<Invoice> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .record_payment(uuid, stateset_core::RecordInvoicePayment {
                amount: Decimal::from_f64_retain(amount).unwrap_or_default(),
                payment_method,
                reference,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to record payment: {}", e)))?;

        Ok(invoice.into())
    }

    /// Void an invoice.
    fn void(&self, id: String) -> PyResult<Invoice> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .void(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to void invoice: {}", e)))?;

        Ok(invoice.into())
    }

    /// Get overdue invoices.
    fn get_overdue(&self) -> PyResult<Vec<Invoice>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let invoices = commerce
            .invoices()
            .get_overdue()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get overdue invoices: {}", e)))?;

        Ok(invoices.into_iter().map(|i| i.into()).collect())
    }

    /// Count invoices.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .invoices()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count invoices: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// BOM Types
// ============================================================================

/// Bill of Materials data.
#[pyclass]
#[derive(Clone)]
pub struct Bom {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    bom_number: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    revision: String,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Bom {
    fn __repr__(&self) -> String {
        format!("Bom(number='{}', name='{}', status='{}')", self.bom_number, self.name, self.status)
    }
}

impl From<stateset_core::BillOfMaterials> for Bom {
    fn from(bom: stateset_core::BillOfMaterials) -> Self {
        Self {
            id: bom.id.to_string(),
            bom_number: bom.bom_number,
            name: bom.name,
            product_id: bom.product_id.to_string(),
            status: format!("{}", bom.status),
            revision: bom.revision,
            created_at: bom.created_at.to_rfc3339(),
        }
    }
}

/// BOM component data.
#[pyclass]
#[derive(Clone)]
pub struct BomComponent {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    bom_id: String,
    #[pyo3(get)]
    component_sku: Option<String>,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    quantity: f64,
    #[pyo3(get)]
    unit_of_measure: String,
}

#[pymethods]
impl BomComponent {
    fn __repr__(&self) -> String {
        format!("BomComponent(name='{}', qty={})", self.name, self.quantity)
    }
}

impl From<stateset_core::BomComponent> for BomComponent {
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

// ============================================================================
// BOM API
// ============================================================================

/// Bill of Materials management operations.
#[pyclass]
pub struct BomApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl BomApi {
    /// Create a new BOM.
    #[pyo3(signature = (name, product_id, description=None, revision=None))]
    fn create(
        &self,
        name: String,
        product_id: String,
        description: Option<String>,
        revision: Option<String>,
    ) -> PyResult<Bom> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let prod_uuid = product_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

        let bom = commerce
            .bom()
            .create(stateset_core::CreateBom {
                name,
                product_id: prod_uuid,
                description,
                revision,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create BOM: {}", e)))?;

        Ok(bom.into())
    }

    /// Get a BOM by ID.
    fn get(&self, id: String) -> PyResult<Option<Bom>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let bom = commerce
            .bom()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get BOM: {}", e)))?;

        Ok(bom.map(|b| b.into()))
    }

    /// List all BOMs.
    fn list(&self) -> PyResult<Vec<Bom>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let boms = commerce
            .bom()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list BOMs: {}", e)))?;

        Ok(boms.into_iter().map(|b| b.into()).collect())
    }

    /// Add a component to a BOM.
    #[pyo3(signature = (bom_id, name, quantity, component_sku=None, unit_of_measure=None))]
    fn add_component(
        &self,
        bom_id: String,
        name: String,
        quantity: f64,
        component_sku: Option<String>,
        unit_of_measure: Option<String>,
    ) -> PyResult<BomComponent> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = bom_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid BOM UUID"))?;

        let component = commerce
            .bom()
            .add_component(uuid, stateset_core::CreateBomComponent {
                component_sku,
                name,
                quantity: Decimal::from_f64_retain(quantity).unwrap_or_default(),
                unit_of_measure,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to add component: {}", e)))?;

        Ok(component.into())
    }

    /// Get components for a BOM.
    fn get_components(&self, bom_id: String) -> PyResult<Vec<BomComponent>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = bom_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid BOM UUID"))?;

        let components = commerce
            .bom()
            .get_components(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get components: {}", e)))?;

        Ok(components.into_iter().map(|c| c.into()).collect())
    }

    /// Activate a BOM.
    fn activate(&self, id: String) -> PyResult<Bom> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let bom = commerce
            .bom()
            .activate(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to activate BOM: {}", e)))?;

        Ok(bom.into())
    }

    /// Count BOMs.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .bom()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count BOMs: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Work Order Types
// ============================================================================

/// Work order data.
#[pyclass]
#[derive(Clone)]
pub struct WorkOrder {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    work_order_number: String,
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    bom_id: Option<String>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    priority: String,
    #[pyo3(get)]
    quantity_to_build: f64,
    #[pyo3(get)]
    quantity_completed: f64,
    #[pyo3(get)]
    version: i32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl WorkOrder {
    fn __repr__(&self) -> String {
        format!(
            "WorkOrder(number='{}', status='{}', qty={}/{})",
            self.work_order_number, self.status, self.quantity_completed, self.quantity_to_build
        )
    }
}

impl From<stateset_core::WorkOrder> for WorkOrder {
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

// ============================================================================
// Work Orders API
// ============================================================================

/// Work order management operations.
#[pyclass]
pub struct WorkOrders {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl WorkOrders {
    /// Create a new work order.
    #[pyo3(signature = (product_id, quantity_to_build, bom_id=None, priority=None, notes=None))]
    fn create(
        &self,
        product_id: String,
        quantity_to_build: f64,
        bom_id: Option<String>,
        priority: Option<String>,
        notes: Option<String>,
    ) -> PyResult<WorkOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let prod_uuid = product_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

        let bom_uuid = bom_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid BOM UUID"))?;

        let prio = priority.and_then(|p| match p.to_lowercase().as_str() {
            "low" => Some(stateset_core::WorkOrderPriority::Low),
            "normal" => Some(stateset_core::WorkOrderPriority::Normal),
            "high" => Some(stateset_core::WorkOrderPriority::High),
            "urgent" => Some(stateset_core::WorkOrderPriority::Urgent),
            _ => None,
        });

        let wo = commerce
            .work_orders()
            .create(stateset_core::CreateWorkOrder {
                product_id: prod_uuid,
                bom_id: bom_uuid,
                quantity_to_build: Decimal::from_f64_retain(quantity_to_build).unwrap_or_default(),
                priority: prio,
                notes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create work order: {}", e)))?;

        Ok(wo.into())
    }

    /// Get a work order by ID.
    fn get(&self, id: String) -> PyResult<Option<WorkOrder>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get work order: {}", e)))?;

        Ok(wo.map(|w| w.into()))
    }

    /// List all work orders.
    fn list(&self) -> PyResult<Vec<WorkOrder>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let wos = commerce
            .work_orders()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list work orders: {}", e)))?;

        Ok(wos.into_iter().map(|w| w.into()).collect())
    }

    /// Start a work order.
    fn start(&self, id: String) -> PyResult<WorkOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .start(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to start work order: {}", e)))?;

        Ok(wo.into())
    }

    /// Complete a work order.
    fn complete(&self, id: String, quantity_completed: f64) -> PyResult<WorkOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .complete(uuid, Decimal::from_f64_retain(quantity_completed).unwrap_or_default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to complete work order: {}", e)))?;

        Ok(wo.into())
    }

    /// Cancel a work order.
    fn cancel(&self, id: String) -> PyResult<WorkOrder> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel work order: {}", e)))?;

        Ok(wo.into())
    }

    /// Count work orders.
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .work_orders()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count work orders: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Cart Types
// ============================================================================

/// Cart address data.
#[pyclass]
#[derive(Clone)]
pub struct CartAddress {
    #[pyo3(get)]
    first_name: String,
    #[pyo3(get)]
    last_name: String,
    #[pyo3(get)]
    company: Option<String>,
    #[pyo3(get)]
    line1: String,
    #[pyo3(get)]
    line2: Option<String>,
    #[pyo3(get)]
    city: String,
    #[pyo3(get)]
    state: Option<String>,
    #[pyo3(get)]
    postal_code: String,
    #[pyo3(get)]
    country: String,
    #[pyo3(get)]
    phone: Option<String>,
    #[pyo3(get)]
    email: Option<String>,
}

#[pymethods]
impl CartAddress {
    #[new]
    #[pyo3(signature = (first_name, last_name, line1, city, postal_code, country, company=None, line2=None, state=None, phone=None, email=None))]
    fn new(
        first_name: String,
        last_name: String,
        line1: String,
        city: String,
        postal_code: String,
        country: String,
        company: Option<String>,
        line2: Option<String>,
        state: Option<String>,
        phone: Option<String>,
        email: Option<String>,
    ) -> Self {
        Self {
            first_name,
            last_name,
            company,
            line1,
            line2,
            city,
            state,
            postal_code,
            country,
            phone,
            email,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "CartAddress(name='{} {}', city='{}')",
            self.first_name, self.last_name, self.city
        )
    }
}

impl From<stateset_core::CartAddress> for CartAddress {
    fn from(a: stateset_core::CartAddress) -> Self {
        Self {
            first_name: a.first_name,
            last_name: a.last_name,
            company: a.company,
            line1: a.line1,
            line2: a.line2,
            city: a.city,
            state: a.state,
            postal_code: a.postal_code,
            country: a.country,
            phone: a.phone,
            email: a.email,
        }
    }
}

impl From<&CartAddress> for stateset_core::CartAddress {
    fn from(a: &CartAddress) -> Self {
        Self {
            first_name: a.first_name.clone(),
            last_name: a.last_name.clone(),
            company: a.company.clone(),
            line1: a.line1.clone(),
            line2: a.line2.clone(),
            city: a.city.clone(),
            state: a.state.clone(),
            postal_code: a.postal_code.clone(),
            country: a.country.clone(),
            phone: a.phone.clone(),
            email: a.email.clone(),
        }
    }
}

/// Cart item data.
#[pyclass]
#[derive(Clone)]
pub struct CartItem {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    cart_id: String,
    #[pyo3(get)]
    product_id: Option<String>,
    #[pyo3(get)]
    variant_id: Option<String>,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    image_url: Option<String>,
    #[pyo3(get)]
    quantity: i32,
    #[pyo3(get)]
    unit_price: f64,
    #[pyo3(get)]
    original_price: Option<f64>,
    #[pyo3(get)]
    discount_amount: f64,
    #[pyo3(get)]
    tax_amount: f64,
    #[pyo3(get)]
    total: f64,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

#[pymethods]
impl CartItem {
    fn __repr__(&self) -> String {
        format!(
            "CartItem(sku='{}', qty={}, total={})",
            self.sku, self.quantity, self.total
        )
    }
}

impl From<stateset_core::CartItem> for CartItem {
    fn from(i: stateset_core::CartItem) -> Self {
        Self {
            id: i.id.to_string(),
            cart_id: i.cart_id.to_string(),
            product_id: i.product_id.map(|id| id.to_string()),
            variant_id: i.variant_id.map(|id| id.to_string()),
            sku: i.sku,
            name: i.name,
            description: i.description,
            image_url: i.image_url,
            quantity: i.quantity,
            unit_price: i.unit_price.to_string().parse().unwrap_or(0.0),
            original_price: i.original_price.map(|p| p.to_string().parse().unwrap_or(0.0)),
            discount_amount: i.discount_amount.to_string().parse().unwrap_or(0.0),
            tax_amount: i.tax_amount.to_string().parse().unwrap_or(0.0),
            total: i.total.to_string().parse().unwrap_or(0.0),
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

/// Shipping rate option.
#[pyclass]
#[derive(Clone)]
pub struct ShippingRate {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    carrier: String,
    #[pyo3(get)]
    service: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    price: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    estimated_days: Option<i32>,
    #[pyo3(get)]
    estimated_delivery: Option<String>,
}

#[pymethods]
impl ShippingRate {
    fn __repr__(&self) -> String {
        format!(
            "ShippingRate(carrier='{}', service='{}', price={})",
            self.carrier, self.service, self.price
        )
    }
}

impl From<stateset_core::ShippingRate> for ShippingRate {
    fn from(r: stateset_core::ShippingRate) -> Self {
        Self {
            id: r.id,
            carrier: r.carrier,
            service: r.service,
            description: r.description,
            price: r.price.to_string().parse().unwrap_or(0.0),
            currency: r.currency,
            estimated_days: r.estimated_days,
            estimated_delivery: r.estimated_delivery.map(|d| d.to_rfc3339()),
        }
    }
}

/// Checkout result returned when completing a cart.
#[pyclass]
#[derive(Clone)]
pub struct CheckoutResult {
    #[pyo3(get)]
    order_id: String,
    #[pyo3(get)]
    order_number: String,
    #[pyo3(get)]
    cart_id: String,
    #[pyo3(get)]
    payment_id: Option<String>,
    #[pyo3(get)]
    total_charged: f64,
    #[pyo3(get)]
    currency: String,
}

#[pymethods]
impl CheckoutResult {
    fn __repr__(&self) -> String {
        format!(
            "CheckoutResult(order='{}', total={} {})",
            self.order_number, self.total_charged, self.currency
        )
    }
}

impl From<stateset_core::CheckoutResult> for CheckoutResult {
    fn from(r: stateset_core::CheckoutResult) -> Self {
        Self {
            order_id: r.order_id.to_string(),
            order_number: r.order_number,
            cart_id: r.cart_id.to_string(),
            payment_id: r.payment_id.map(|id| id.to_string()),
            total_charged: r.total_charged.to_string().parse().unwrap_or(0.0),
            currency: r.currency,
        }
    }
}

/// Cart data returned from operations.
#[pyclass]
#[derive(Clone)]
pub struct Cart {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    cart_number: String,
    #[pyo3(get)]
    customer_id: Option<String>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    subtotal: f64,
    #[pyo3(get)]
    tax_amount: f64,
    #[pyo3(get)]
    shipping_amount: f64,
    #[pyo3(get)]
    discount_amount: f64,
    #[pyo3(get)]
    grand_total: f64,
    #[pyo3(get)]
    customer_email: Option<String>,
    #[pyo3(get)]
    customer_name: Option<String>,
    #[pyo3(get)]
    payment_method: Option<String>,
    #[pyo3(get)]
    payment_status: String,
    #[pyo3(get)]
    fulfillment_type: String,
    #[pyo3(get)]
    shipping_method: Option<String>,
    #[pyo3(get)]
    coupon_code: Option<String>,
    #[pyo3(get)]
    notes: Option<String>,
    #[pyo3(get)]
    item_count: i32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
    #[pyo3(get)]
    expires_at: Option<String>,
    // Store items separately
    _items: Vec<CartItem>,
    _shipping_address: Option<CartAddress>,
    _billing_address: Option<CartAddress>,
}

#[pymethods]
impl Cart {
    fn __repr__(&self) -> String {
        format!(
            "Cart(number='{}', status='{}', total={} {})",
            self.cart_number, self.status, self.grand_total, self.currency
        )
    }

    /// Get cart items.
    #[getter]
    fn items(&self) -> Vec<CartItem> {
        self._items.clone()
    }

    /// Get the shipping address.
    #[getter]
    fn shipping_address(&self) -> Option<CartAddress> {
        self._shipping_address.clone()
    }

    /// Get the billing address.
    #[getter]
    fn billing_address(&self) -> Option<CartAddress> {
        self._billing_address.clone()
    }
}

impl From<stateset_core::Cart> for Cart {
    fn from(c: stateset_core::Cart) -> Self {
        let item_count = c.items.len() as i32;
        Self {
            id: c.id.to_string(),
            cart_number: c.cart_number,
            customer_id: c.customer_id.map(|id| id.to_string()),
            status: format!("{}", c.status),
            currency: c.currency,
            subtotal: c.subtotal.to_string().parse().unwrap_or(0.0),
            tax_amount: c.tax_amount.to_string().parse().unwrap_or(0.0),
            shipping_amount: c.shipping_amount.to_string().parse().unwrap_or(0.0),
            discount_amount: c.discount_amount.to_string().parse().unwrap_or(0.0),
            grand_total: c.grand_total.to_string().parse().unwrap_or(0.0),
            customer_email: c.customer_email,
            customer_name: c.customer_name,
            payment_method: c.payment_method,
            payment_status: format!("{}", c.payment_status),
            fulfillment_type: c.fulfillment_type.map(|ft| format!("{}", ft)).unwrap_or_else(|| "Shipping".to_string()),
            shipping_method: c.shipping_method,
            coupon_code: c.coupon_code,
            notes: c.notes,
            item_count,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
            expires_at: c.expires_at.map(|d| d.to_rfc3339()),
            _items: c.items.into_iter().map(|i| i.into()).collect(),
            _shipping_address: c.shipping_address.map(|a| a.into()),
            _billing_address: c.billing_address.map(|a| a.into()),
        }
    }
}

/// Input for adding a cart item.
#[pyclass]
#[derive(Clone)]
pub struct AddCartItemInput {
    #[pyo3(get, set)]
    sku: String,
    #[pyo3(get, set)]
    name: String,
    #[pyo3(get, set)]
    quantity: i32,
    #[pyo3(get, set)]
    unit_price: f64,
    #[pyo3(get, set)]
    product_id: Option<String>,
    #[pyo3(get, set)]
    variant_id: Option<String>,
    #[pyo3(get, set)]
    description: Option<String>,
    #[pyo3(get, set)]
    image_url: Option<String>,
    #[pyo3(get, set)]
    original_price: Option<f64>,
    #[pyo3(get, set)]
    weight: Option<f64>,
    #[pyo3(get, set)]
    requires_shipping: Option<bool>,
}

#[pymethods]
impl AddCartItemInput {
    #[new]
    #[pyo3(signature = (sku, name, quantity, unit_price, product_id=None, variant_id=None, description=None, image_url=None, original_price=None, weight=None, requires_shipping=None))]
    fn new(
        sku: String,
        name: String,
        quantity: i32,
        unit_price: f64,
        product_id: Option<String>,
        variant_id: Option<String>,
        description: Option<String>,
        image_url: Option<String>,
        original_price: Option<f64>,
        weight: Option<f64>,
        requires_shipping: Option<bool>,
    ) -> Self {
        Self {
            sku,
            name,
            quantity,
            unit_price,
            product_id,
            variant_id,
            description,
            image_url,
            original_price,
            weight,
            requires_shipping,
        }
    }
}

// ============================================================================
// Carts API
// ============================================================================

/// Cart and checkout management operations.
#[pyclass]
pub struct Carts {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Carts {
    /// Create a new cart.
    ///
    /// Args:
    ///     customer_id: Customer UUID (optional for guest checkout)
    ///     customer_email: Customer email (optional)
    ///     customer_name: Customer name (optional)
    ///     currency: Currency code (default "USD")
    ///     expires_in_minutes: Cart expiration time (optional)
    ///
    /// Returns:
    ///     Cart: The created cart
    #[pyo3(signature = (customer_id=None, customer_email=None, customer_name=None, currency=None, expires_in_minutes=None))]
    fn create(
        &self,
        customer_id: Option<String>,
        customer_email: Option<String>,
        customer_name: Option<String>,
        currency: Option<String>,
        expires_in_minutes: Option<i64>,
    ) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid = customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let cart = commerce
            .carts()
            .create(stateset_core::CreateCart {
                customer_id: cust_uuid,
                customer_email,
                customer_name,
                currency,
                expires_in_minutes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Get a cart by ID.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart or None if not found
    fn get(&self, id: String) -> PyResult<Option<Cart>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get cart: {}", e)))?;

        Ok(cart.map(|c| c.into()))
    }

    /// Get a cart by cart number.
    ///
    /// Args:
    ///     cart_number: Cart number string
    ///
    /// Returns:
    ///     Cart or None if not found
    fn get_by_number(&self, cart_number: String) -> PyResult<Option<Cart>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cart = commerce
            .carts()
            .get_by_number(&cart_number)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get cart: {}", e)))?;

        Ok(cart.map(|c| c.into()))
    }

    /// Update a cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///     customer_email: Customer email (optional)
    ///     customer_phone: Customer phone (optional)
    ///     customer_name: Customer name (optional)
    ///     shipping_method: Shipping method string (optional)
    ///     coupon_code: Coupon code (optional)
    ///     notes: Notes (optional)
    ///
    /// Returns:
    ///     Cart: Updated cart
    #[pyo3(signature = (id, customer_email=None, customer_phone=None, customer_name=None, shipping_method=None, coupon_code=None, notes=None))]
    fn update(
        &self,
        id: String,
        customer_email: Option<String>,
        customer_phone: Option<String>,
        customer_name: Option<String>,
        shipping_method: Option<String>,
        coupon_code: Option<String>,
        notes: Option<String>,
    ) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .update(uuid, stateset_core::UpdateCart {
                customer_email,
                customer_phone,
                customer_name,
                shipping_method,
                coupon_code,
                notes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update cart: {}", e)))?;

        Ok(cart.into())
    }

    /// List all carts.
    ///
    /// Returns:
    ///     List[Cart]: All carts
    fn list(&self) -> PyResult<Vec<Cart>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let carts = commerce
            .carts()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// Get all carts for a customer.
    ///
    /// Args:
    ///     customer_id: Customer UUID
    ///
    /// Returns:
    ///     List[Cart]: Customer's carts
    fn for_customer(&self, customer_id: String) -> PyResult<Vec<Cart>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = customer_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let carts = commerce
            .carts()
            .for_customer(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get customer carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// Delete a cart.
    ///
    /// Args:
    ///     id: Cart UUID
    fn delete(&self, id: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .carts()
            .delete(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete cart: {}", e)))?;

        Ok(())
    }

    // === Item Operations ===

    /// Add an item to the cart.
    ///
    /// Args:
    ///     cart_id: Cart UUID
    ///     item: AddCartItemInput
    ///
    /// Returns:
    ///     CartItem: The added item
    fn add_item(&self, cart_id: String, item: AddCartItemInput) -> PyResult<CartItem> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = cart_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let prod_uuid = item.product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

        let var_uuid = item.variant_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid variant UUID"))?;

        let cart_item = commerce
            .carts()
            .add_item(uuid, stateset_core::AddCartItem {
                product_id: prod_uuid,
                variant_id: var_uuid,
                sku: item.sku,
                name: item.name,
                description: item.description,
                image_url: item.image_url,
                quantity: item.quantity,
                unit_price: Decimal::from_str_exact(&item.unit_price.to_string()).unwrap_or_default(),
                original_price: item.original_price.map(|p| Decimal::from_str_exact(&p.to_string()).unwrap_or_default()),
                weight: item.weight.map(|w| Decimal::from_str_exact(&w.to_string()).unwrap_or_default()),
                requires_shipping: item.requires_shipping,
                metadata: None,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to add item: {}", e)))?;

        Ok(cart_item.into())
    }

    /// Update a cart item.
    ///
    /// Args:
    ///     item_id: Cart item UUID
    ///     quantity: New quantity (optional)
    ///
    /// Returns:
    ///     CartItem: The updated item
    #[pyo3(signature = (item_id, quantity=None))]
    fn update_item(&self, item_id: String, quantity: Option<i32>) -> PyResult<CartItem> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = item_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart_item = commerce
            .carts()
            .update_item(uuid, stateset_core::UpdateCartItem {
                quantity,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update item: {}", e)))?;

        Ok(cart_item.into())
    }

    /// Remove an item from the cart.
    ///
    /// Args:
    ///     item_id: Cart item UUID
    fn remove_item(&self, item_id: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = item_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .carts()
            .remove_item(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to remove item: {}", e)))?;

        Ok(())
    }

    /// Get all items in a cart.
    ///
    /// Args:
    ///     cart_id: Cart UUID
    ///
    /// Returns:
    ///     List[CartItem]: Cart items
    fn get_items(&self, cart_id: String) -> PyResult<Vec<CartItem>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = cart_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let items = commerce
            .carts()
            .get_items(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get items: {}", e)))?;

        Ok(items.into_iter().map(|i| i.into()).collect())
    }

    /// Clear all items from a cart.
    ///
    /// Args:
    ///     cart_id: Cart UUID
    fn clear_items(&self, cart_id: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = cart_id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .carts()
            .clear_items(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to clear items: {}", e)))?;

        Ok(())
    }

    // === Address Operations ===

    /// Set the shipping address.
    ///
    /// Args:
    ///     id: Cart UUID
    ///     address: CartAddress
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn set_shipping_address(&self, id: String, address: CartAddress) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_shipping_address(uuid, (&address).into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set shipping address: {}", e)))?;

        Ok(cart.into())
    }

    /// Set the billing address.
    ///
    /// Args:
    ///     id: Cart UUID
    ///     address: CartAddress
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn set_billing_address(&self, id: String, address: CartAddress) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_billing_address(uuid, (&address).into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set billing address: {}", e)))?;

        Ok(cart.into())
    }

    // === Shipping Operations ===

    /// Set shipping selection (address + method/carrier/amount).
    ///
    /// Args:
    ///     id: Cart UUID
    ///     address: CartAddress
    ///     shipping_method: Shipping method (optional)
    ///     shipping_carrier: Shipping carrier (optional)
    ///     shipping_amount: Shipping amount (optional)
    ///
    /// Returns:
    ///     Cart: Updated cart
    #[pyo3(signature = (id, address, shipping_method=None, shipping_carrier=None, shipping_amount=None))]
    fn set_shipping(
        &self,
        id: String,
        address: CartAddress,
        shipping_method: Option<String>,
        shipping_carrier: Option<String>,
        shipping_amount: Option<f64>,
    ) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let amount_dec = match shipping_amount {
            Some(v) => Some(
                Decimal::from_f64_retain(v)
                    .ok_or_else(|| PyValueError::new_err("Invalid shipping amount"))?,
            ),
            None => None,
        };

        let cart = commerce
            .carts()
            .set_shipping(uuid, stateset_core::SetCartShipping {
                shipping_address: (&address).into(),
                shipping_method,
                shipping_carrier,
                shipping_amount: amount_dec,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set shipping: {}", e)))?;

        Ok(cart.into())
    }

    /// Get available shipping rates for the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     List[ShippingRate]: Available shipping options
    fn get_shipping_rates(&self, id: String) -> PyResult<Vec<ShippingRate>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let rates = commerce
            .carts()
            .get_shipping_rates(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get shipping rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    // === Payment Operations ===

    /// Set the payment method.
    ///
    /// Args:
    ///     id: Cart UUID
    ///     payment_method: Payment method string (e.g., "credit_card")
    ///     payment_token: Payment token (optional)
    ///
    /// Returns:
    ///     Cart: Updated cart
    #[pyo3(signature = (id, payment_method, payment_token=None))]
    fn set_payment(&self, id: String, payment_method: String, payment_token: Option<String>) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_payment(uuid, stateset_core::SetCartPayment {
                payment_method,
                payment_token,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set payment: {}", e)))?;

        Ok(cart.into())
    }

    // === Discount Operations ===

    /// Apply a coupon code to the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///     coupon_code: Coupon/discount code
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn apply_discount(&self, id: String, coupon_code: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .apply_discount(uuid, &coupon_code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to apply discount: {}", e)))?;

        Ok(cart.into())
    }

    /// Remove the discount from the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn remove_discount(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .remove_discount(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to remove discount: {}", e)))?;

        Ok(cart.into())
    }

    // === Checkout Flow ===

    /// Mark the cart as ready for payment.
    ///
    /// Validates that all required info is present.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn mark_ready_for_payment(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .mark_ready_for_payment(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to mark ready: {}", e)))?;

        Ok(cart.into())
    }

    /// Begin the checkout process.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn begin_checkout(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .begin_checkout(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to begin checkout: {}", e)))?;

        Ok(cart.into())
    }

    /// Complete the checkout and create an order.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     CheckoutResult: Order creation result
    fn complete(&self, id: String) -> PyResult<CheckoutResult> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let result = commerce
            .carts()
            .complete(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to complete checkout: {}", e)))?;

        Ok(result.into())
    }

    /// Cancel the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn cancel(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Mark the cart as abandoned.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn abandon(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .abandon(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to abandon cart: {}", e)))?;

        Ok(cart.into())
    }

    /// Expire the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn expire(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .expire(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to expire cart: {}", e)))?;

        Ok(cart.into())
    }

    // === Inventory Operations ===

    /// Reserve inventory for cart items.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn reserve_inventory(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .reserve_inventory(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to reserve inventory: {}", e)))?;

        Ok(cart.into())
    }

    /// Release inventory reservations.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn release_inventory(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .release_inventory(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to release inventory: {}", e)))?;

        Ok(cart.into())
    }

    /// Recalculate cart totals.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn recalculate(&self, id: String) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .recalculate(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to recalculate: {}", e)))?;

        Ok(cart.into())
    }

    /// Set the tax amount for the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///     tax_amount: Tax amount
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn set_tax(&self, id: String, tax_amount: f64) -> PyResult<Cart> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_tax(uuid, Decimal::from_str_exact(&tax_amount.to_string()).unwrap_or_default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set tax: {}", e)))?;

        Ok(cart.into())
    }

    // === Query Operations ===

    /// Get abandoned carts.
    ///
    /// Returns:
    ///     List[Cart]: Abandoned carts
    fn get_abandoned(&self) -> PyResult<Vec<Cart>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let carts = commerce
            .carts()
            .get_abandoned()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get abandoned carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// Get expired carts.
    ///
    /// Returns:
    ///     List[Cart]: Expired carts
    fn get_expired(&self) -> PyResult<Vec<Cart>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let carts = commerce
            .carts()
            .get_expired()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get expired carts: {}", e)))?;

        Ok(carts.into_iter().map(|c| c.into()).collect())
    }

    /// Count carts.
    ///
    /// Returns:
    ///     int: Number of carts
    fn count(&self) -> PyResult<u32> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .carts()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count carts: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Analytics Types
// ============================================================================

fn dec_to_f64(d: &Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

fn parse_time_period(period: &str) -> stateset_core::TimePeriod {
    match period.to_lowercase().as_str() {
        "today" => stateset_core::TimePeriod::Today,
        "yesterday" => stateset_core::TimePeriod::Yesterday,
        "last7days" | "last_7_days" => stateset_core::TimePeriod::Last7Days,
        "last30days" | "last_30_days" => stateset_core::TimePeriod::Last30Days,
        "this_month" | "thismonth" => stateset_core::TimePeriod::ThisMonth,
        "last_month" | "lastmonth" => stateset_core::TimePeriod::LastMonth,
        "this_quarter" | "thisquarter" => stateset_core::TimePeriod::ThisQuarter,
        "last_quarter" | "lastquarter" => stateset_core::TimePeriod::LastQuarter,
        "this_year" | "thisyear" => stateset_core::TimePeriod::ThisYear,
        "last_year" | "lastyear" => stateset_core::TimePeriod::LastYear,
        "all_time" | "alltime" | "all" => stateset_core::TimePeriod::AllTime,
        _ => stateset_core::TimePeriod::Last30Days,
    }
}

fn parse_time_granularity(granularity: &str) -> stateset_core::TimeGranularity {
    match granularity.to_lowercase().as_str() {
        "hour" | "hourly" => stateset_core::TimeGranularity::Hour,
        "day" | "daily" => stateset_core::TimeGranularity::Day,
        "week" | "weekly" => stateset_core::TimeGranularity::Week,
        "month" | "monthly" => stateset_core::TimeGranularity::Month,
        "quarter" | "quarterly" => stateset_core::TimeGranularity::Quarter,
        "year" | "yearly" => stateset_core::TimeGranularity::Year,
        _ => stateset_core::TimeGranularity::Day,
    }
}

fn build_analytics_query(
    period: Option<String>,
    granularity: Option<String>,
    limit: Option<u32>,
) -> stateset_core::AnalyticsQuery {
    let mut q = stateset_core::AnalyticsQuery::new();
    if let Some(p) = period {
        q = q.period(parse_time_period(&p));
    }
    if let Some(g) = granularity {
        q = q.granularity(parse_time_granularity(&g));
    }
    if let Some(l) = limit {
        q = q.limit(l);
    }
    q
}

/// Sales summary metrics.
#[pyclass]
#[derive(Clone)]
pub struct SalesSummary {
    #[pyo3(get)]
    total_revenue: f64,
    #[pyo3(get)]
    order_count: u32,
    #[pyo3(get)]
    average_order_value: f64,
    #[pyo3(get)]
    items_sold: u32,
    #[pyo3(get)]
    unique_customers: u32,
}

impl From<stateset_core::SalesSummary> for SalesSummary {
    fn from(s: stateset_core::SalesSummary) -> Self {
        Self {
            total_revenue: dec_to_f64(&s.total_revenue),
            order_count: s.order_count as u32,
            average_order_value: dec_to_f64(&s.average_order_value),
            items_sold: s.items_sold as u32,
            unique_customers: s.unique_customers as u32,
        }
    }
}

/// Revenue metrics grouped by time period.
#[pyclass]
#[derive(Clone)]
pub struct RevenueByPeriod {
    #[pyo3(get)]
    period: String,
    #[pyo3(get)]
    revenue: f64,
    #[pyo3(get)]
    order_count: u32,
    #[pyo3(get)]
    period_start: String,
}

impl From<stateset_core::RevenueByPeriod> for RevenueByPeriod {
    fn from(r: stateset_core::RevenueByPeriod) -> Self {
        Self {
            period: r.period,
            revenue: dec_to_f64(&r.revenue),
            order_count: r.order_count as u32,
            period_start: r.period_start.to_rfc3339(),
        }
    }
}

/// Top selling product metrics.
#[pyclass]
#[derive(Clone)]
pub struct TopProduct {
    #[pyo3(get)]
    product_id: Option<String>,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    units_sold: u32,
    #[pyo3(get)]
    revenue: f64,
    #[pyo3(get)]
    order_count: u32,
}

impl From<stateset_core::TopProduct> for TopProduct {
    fn from(p: stateset_core::TopProduct) -> Self {
        Self {
            product_id: p.product_id.map(|id| id.to_string()),
            sku: p.sku,
            name: p.name,
            units_sold: p.units_sold as u32,
            revenue: dec_to_f64(&p.revenue),
            order_count: p.order_count as u32,
        }
    }
}

/// Product performance with period comparison.
#[pyclass]
#[derive(Clone)]
pub struct ProductPerformance {
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    units_sold: u32,
    #[pyo3(get)]
    revenue: f64,
    #[pyo3(get)]
    previous_units_sold: u32,
    #[pyo3(get)]
    previous_revenue: f64,
    #[pyo3(get)]
    units_growth_percent: f64,
    #[pyo3(get)]
    revenue_growth_percent: f64,
}

impl From<stateset_core::ProductPerformance> for ProductPerformance {
    fn from(p: stateset_core::ProductPerformance) -> Self {
        Self {
            product_id: p.product_id.to_string(),
            sku: p.sku,
            name: p.name,
            units_sold: p.units_sold as u32,
            revenue: dec_to_f64(&p.revenue),
            previous_units_sold: p.previous_units_sold as u32,
            previous_revenue: dec_to_f64(&p.previous_revenue),
            units_growth_percent: dec_to_f64(&p.units_growth_percent),
            revenue_growth_percent: dec_to_f64(&p.revenue_growth_percent),
        }
    }
}

/// Customer segment metrics.
#[pyclass]
#[derive(Clone)]
pub struct CustomerMetrics {
    #[pyo3(get)]
    total_customers: u32,
    #[pyo3(get)]
    new_customers: u32,
    #[pyo3(get)]
    returning_customers: u32,
    #[pyo3(get)]
    average_lifetime_value: f64,
    #[pyo3(get)]
    average_orders_per_customer: f64,
}

impl From<stateset_core::CustomerMetrics> for CustomerMetrics {
    fn from(m: stateset_core::CustomerMetrics) -> Self {
        Self {
            total_customers: m.total_customers as u32,
            new_customers: m.new_customers as u32,
            returning_customers: m.returning_customers as u32,
            average_lifetime_value: dec_to_f64(&m.average_lifetime_value),
            average_orders_per_customer: dec_to_f64(&m.average_orders_per_customer),
        }
    }
}

/// Top customer by spend.
#[pyclass]
#[derive(Clone)]
pub struct TopCustomer {
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    email: String,
    #[pyo3(get)]
    order_count: u32,
    #[pyo3(get)]
    total_spent: f64,
    #[pyo3(get)]
    average_order_value: f64,
}

impl From<stateset_core::TopCustomer> for TopCustomer {
    fn from(c: stateset_core::TopCustomer) -> Self {
        Self {
            customer_id: c.customer_id.to_string(),
            name: c.name,
            email: c.email,
            order_count: c.order_count as u32,
            total_spent: dec_to_f64(&c.total_spent),
            average_order_value: dec_to_f64(&c.average_order_value),
        }
    }
}

/// Inventory health summary.
#[pyclass]
#[derive(Clone)]
pub struct InventoryHealth {
    #[pyo3(get)]
    total_skus: u32,
    #[pyo3(get)]
    in_stock_skus: u32,
    #[pyo3(get)]
    low_stock_skus: u32,
    #[pyo3(get)]
    out_of_stock_skus: u32,
    #[pyo3(get)]
    total_value: f64,
}

impl From<stateset_core::InventoryHealth> for InventoryHealth {
    fn from(h: stateset_core::InventoryHealth) -> Self {
        Self {
            total_skus: h.total_skus as u32,
            in_stock_skus: h.in_stock_skus as u32,
            low_stock_skus: h.low_stock_skus as u32,
            out_of_stock_skus: h.out_of_stock_skus as u32,
            total_value: dec_to_f64(&h.total_value),
        }
    }
}

/// Low stock item.
#[pyclass]
#[derive(Clone)]
pub struct LowStockItem {
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    on_hand: f64,
    #[pyo3(get)]
    allocated: f64,
    #[pyo3(get)]
    available: f64,
    #[pyo3(get)]
    reorder_point: Option<f64>,
    #[pyo3(get)]
    average_daily_sales: Option<f64>,
    #[pyo3(get)]
    days_of_stock: Option<f64>,
}

impl From<stateset_core::LowStockItem> for LowStockItem {
    fn from(i: stateset_core::LowStockItem) -> Self {
        Self {
            sku: i.sku,
            name: i.name,
            on_hand: dec_to_f64(&i.on_hand),
            allocated: dec_to_f64(&i.allocated),
            available: dec_to_f64(&i.available),
            reorder_point: i.reorder_point.as_ref().map(dec_to_f64),
            average_daily_sales: i.average_daily_sales.as_ref().map(dec_to_f64),
            days_of_stock: i.days_of_stock.as_ref().map(dec_to_f64),
        }
    }
}

/// Inventory movement summary.
#[pyclass]
#[derive(Clone)]
pub struct InventoryMovement {
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    units_sold: u32,
    #[pyo3(get)]
    units_received: u32,
    #[pyo3(get)]
    units_returned: u32,
    #[pyo3(get)]
    units_adjusted: i32,
    #[pyo3(get)]
    net_change: i32,
}

impl From<stateset_core::InventoryMovement> for InventoryMovement {
    fn from(m: stateset_core::InventoryMovement) -> Self {
        Self {
            sku: m.sku,
            name: m.name,
            units_sold: m.units_sold as u32,
            units_received: m.units_received as u32,
            units_returned: m.units_returned as u32,
            units_adjusted: m.units_adjusted as i32,
            net_change: m.net_change as i32,
        }
    }
}

/// Order status breakdown.
#[pyclass]
#[derive(Clone)]
pub struct OrderStatusBreakdown {
    #[pyo3(get)]
    pending: u32,
    #[pyo3(get)]
    confirmed: u32,
    #[pyo3(get)]
    processing: u32,
    #[pyo3(get)]
    shipped: u32,
    #[pyo3(get)]
    delivered: u32,
    #[pyo3(get)]
    cancelled: u32,
    #[pyo3(get)]
    refunded: u32,
}

impl From<stateset_core::OrderStatusBreakdown> for OrderStatusBreakdown {
    fn from(b: stateset_core::OrderStatusBreakdown) -> Self {
        Self {
            pending: b.pending as u32,
            confirmed: b.confirmed as u32,
            processing: b.processing as u32,
            shipped: b.shipped as u32,
            delivered: b.delivered as u32,
            cancelled: b.cancelled as u32,
            refunded: b.refunded as u32,
        }
    }
}

/// Order fulfillment metrics.
#[pyclass]
#[derive(Clone)]
pub struct FulfillmentMetrics {
    #[pyo3(get)]
    avg_time_to_ship_hours: Option<f64>,
    #[pyo3(get)]
    avg_time_to_deliver_hours: Option<f64>,
    #[pyo3(get)]
    on_time_shipping_percent: Option<f64>,
    #[pyo3(get)]
    on_time_delivery_percent: Option<f64>,
    #[pyo3(get)]
    shipped_today: u32,
    #[pyo3(get)]
    awaiting_shipment: u32,
}

impl From<stateset_core::FulfillmentMetrics> for FulfillmentMetrics {
    fn from(m: stateset_core::FulfillmentMetrics) -> Self {
        Self {
            avg_time_to_ship_hours: m.avg_time_to_ship_hours.as_ref().map(dec_to_f64),
            avg_time_to_deliver_hours: m.avg_time_to_deliver_hours.as_ref().map(dec_to_f64),
            on_time_shipping_percent: m.on_time_shipping_percent.as_ref().map(dec_to_f64),
            on_time_delivery_percent: m.on_time_delivery_percent.as_ref().map(dec_to_f64),
            shipped_today: m.shipped_today as u32,
            awaiting_shipment: m.awaiting_shipment as u32,
        }
    }
}

/// Return metrics.
#[pyclass]
#[derive(Clone)]
pub struct ReturnMetrics {
    #[pyo3(get)]
    total_returns: u32,
    #[pyo3(get)]
    return_rate_percent: f64,
    #[pyo3(get)]
    total_refunded: f64,
}

impl From<stateset_core::ReturnMetrics> for ReturnMetrics {
    fn from(m: stateset_core::ReturnMetrics) -> Self {
        Self {
            total_returns: m.total_returns as u32,
            return_rate_percent: dec_to_f64(&m.return_rate_percent),
            total_refunded: dec_to_f64(&m.total_refunded),
        }
    }
}

fn trend_to_string(t: &stateset_core::Trend) -> String {
    match t {
        stateset_core::Trend::Rising => "rising".to_string(),
        stateset_core::Trend::Stable => "stable".to_string(),
        stateset_core::Trend::Falling => "falling".to_string(),
    }
}

/// Demand forecast for a SKU.
#[pyclass]
#[derive(Clone)]
pub struct DemandForecast {
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    average_daily_demand: f64,
    #[pyo3(get)]
    forecasted_demand: f64,
    #[pyo3(get)]
    confidence: f64,
    #[pyo3(get)]
    current_stock: f64,
    #[pyo3(get)]
    days_until_stockout: Option<i32>,
    #[pyo3(get)]
    recommended_reorder_qty: Option<f64>,
    #[pyo3(get)]
    trend: String,
}

impl From<stateset_core::DemandForecast> for DemandForecast {
    fn from(f: stateset_core::DemandForecast) -> Self {
        Self {
            sku: f.sku,
            name: f.name,
            average_daily_demand: dec_to_f64(&f.average_daily_demand),
            forecasted_demand: dec_to_f64(&f.forecasted_demand),
            confidence: dec_to_f64(&f.confidence),
            current_stock: dec_to_f64(&f.current_stock),
            days_until_stockout: f.days_until_stockout,
            recommended_reorder_qty: f.recommended_reorder_qty.as_ref().map(dec_to_f64),
            trend: trend_to_string(&f.trend),
        }
    }
}

/// Revenue forecast.
#[pyclass]
#[derive(Clone)]
pub struct RevenueForecast {
    #[pyo3(get)]
    period: String,
    #[pyo3(get)]
    forecasted_revenue: f64,
    #[pyo3(get)]
    lower_bound: f64,
    #[pyo3(get)]
    upper_bound: f64,
    #[pyo3(get)]
    confidence_level: f64,
    #[pyo3(get)]
    based_on_periods: u32,
}

impl From<stateset_core::RevenueForecast> for RevenueForecast {
    fn from(f: stateset_core::RevenueForecast) -> Self {
        Self {
            period: f.period,
            forecasted_revenue: dec_to_f64(&f.forecasted_revenue),
            lower_bound: dec_to_f64(&f.lower_bound),
            upper_bound: dec_to_f64(&f.upper_bound),
            confidence_level: dec_to_f64(&f.confidence_level),
            based_on_periods: f.based_on_periods,
        }
    }
}

// ============================================================================
// Analytics API
// ============================================================================

/// Business intelligence and forecasting operations.
#[pyclass]
pub struct Analytics {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Analytics {
    /// Get sales summary.
    #[pyo3(signature = (period=None, limit=None))]
    fn sales_summary(&self, period: Option<String>, limit: Option<u32>) -> PyResult<SalesSummary> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, limit);
        let summary = commerce
            .analytics()
            .sales_summary(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get sales summary: {}", e)))?;

        Ok(summary.into())
    }

    /// Get revenue by period.
    #[pyo3(signature = (period=None, granularity=None))]
    fn revenue_by_period(
        &self,
        period: Option<String>,
        granularity: Option<String>,
    ) -> PyResult<Vec<RevenueByPeriod>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, granularity, None);
        let rows = commerce
            .analytics()
            .revenue_by_period(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get revenue: {}", e)))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get top selling products.
    #[pyo3(signature = (period=None, limit=None))]
    fn top_products(&self, period: Option<String>, limit: Option<u32>) -> PyResult<Vec<TopProduct>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, limit);
        let rows = commerce
            .analytics()
            .top_products(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get top products: {}", e)))?;

        Ok(rows.into_iter().map(|p| p.into()).collect())
    }

    /// Get product performance with period comparison.
    #[pyo3(signature = (period=None, limit=None))]
    fn product_performance(&self, period: Option<String>, limit: Option<u32>) -> PyResult<Vec<ProductPerformance>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, limit);
        let rows = commerce
            .analytics()
            .product_performance(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get product performance: {}", e)))?;

        Ok(rows.into_iter().map(|p| p.into()).collect())
    }

    /// Get customer metrics.
    #[pyo3(signature = (period=None))]
    fn customer_metrics(&self, period: Option<String>) -> PyResult<CustomerMetrics> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let metrics = commerce
            .analytics()
            .customer_metrics(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get customer metrics: {}", e)))?;

        Ok(metrics.into())
    }

    /// Get top customers by spend.
    #[pyo3(signature = (period=None, limit=None))]
    fn top_customers(&self, period: Option<String>, limit: Option<u32>) -> PyResult<Vec<TopCustomer>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, limit);
        let rows = commerce
            .analytics()
            .top_customers(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get top customers: {}", e)))?;

        Ok(rows.into_iter().map(|c| c.into()).collect())
    }

    /// Get inventory health summary.
    fn inventory_health(&self) -> PyResult<InventoryHealth> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let health = commerce
            .analytics()
            .inventory_health()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get inventory health: {}", e)))?;

        Ok(health.into())
    }

    /// Get low stock items.
    #[pyo3(signature = (threshold=None))]
    fn low_stock_items(&self, threshold: Option<f64>) -> PyResult<Vec<LowStockItem>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let threshold_dec = match threshold {
            Some(v) => Some(
                Decimal::from_f64_retain(v)
                    .ok_or_else(|| PyValueError::new_err("Invalid threshold"))?,
            ),
            None => None,
        };

        let rows = commerce
            .analytics()
            .low_stock_items(threshold_dec)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get low stock items: {}", e)))?;

        Ok(rows.into_iter().map(|i| i.into()).collect())
    }

    /// Get inventory movement summary.
    #[pyo3(signature = (period=None))]
    fn inventory_movement(&self, period: Option<String>) -> PyResult<Vec<InventoryMovement>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let rows = commerce
            .analytics()
            .inventory_movement(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get inventory movement: {}", e)))?;

        Ok(rows.into_iter().map(|m| m.into()).collect())
    }

    /// Get order status breakdown.
    #[pyo3(signature = (period=None))]
    fn order_status_breakdown(&self, period: Option<String>) -> PyResult<OrderStatusBreakdown> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let breakdown = commerce
            .analytics()
            .order_status_breakdown(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get order status breakdown: {}", e)))?;

        Ok(breakdown.into())
    }

    /// Get fulfillment metrics.
    #[pyo3(signature = (period=None))]
    fn fulfillment_metrics(&self, period: Option<String>) -> PyResult<FulfillmentMetrics> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let metrics = commerce
            .analytics()
            .fulfillment_metrics(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get fulfillment metrics: {}", e)))?;

        Ok(metrics.into())
    }

    /// Get return metrics.
    #[pyo3(signature = (period=None))]
    fn return_metrics(&self, period: Option<String>) -> PyResult<ReturnMetrics> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let metrics = commerce
            .analytics()
            .return_metrics(q)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get return metrics: {}", e)))?;

        Ok(metrics.into())
    }

    /// Get demand forecast.
    #[pyo3(signature = (skus=None, days_ahead=None))]
    fn demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: Option<u32>,
    ) -> PyResult<Vec<DemandForecast>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let forecasts = commerce
            .analytics()
            .demand_forecast(skus, days_ahead.unwrap_or(30))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get demand forecast: {}", e)))?;

        Ok(forecasts.into_iter().map(|f| f.into()).collect())
    }

    /// Get revenue forecast.
    #[pyo3(signature = (periods_ahead=None, granularity=None))]
    fn revenue_forecast(
        &self,
        periods_ahead: Option<u32>,
        granularity: Option<String>,
    ) -> PyResult<Vec<RevenueForecast>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let gran = granularity
            .as_deref()
            .map(parse_time_granularity)
            .unwrap_or(stateset_core::TimeGranularity::Month);

        let forecasts = commerce
            .analytics()
            .revenue_forecast(periods_ahead.unwrap_or(3), gran)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get revenue forecast: {}", e)))?;

        Ok(forecasts.into_iter().map(|f| f.into()).collect())
    }
}

// ============================================================================
// Currency Types + API
// ============================================================================

fn parse_currency(code: &str) -> PyResult<stateset_core::Currency> {
    use std::str::FromStr;
    stateset_core::Currency::from_str(code)
        .map_err(|e| PyValueError::new_err(format!("Invalid currency code '{}': {}", code, e)))
}

fn parse_rounding_mode(mode: &str) -> stateset_core::RoundingMode {
    match mode.to_lowercase().as_str() {
        "half_down" => stateset_core::RoundingMode::HalfDown,
        "up" => stateset_core::RoundingMode::Up,
        "down" => stateset_core::RoundingMode::Down,
        "half_even" => stateset_core::RoundingMode::HalfEven,
        _ => stateset_core::RoundingMode::HalfUp,
    }
}

fn rounding_mode_to_string(mode: &stateset_core::RoundingMode) -> String {
    match mode {
        stateset_core::RoundingMode::HalfUp => "half_up".to_string(),
        stateset_core::RoundingMode::HalfDown => "half_down".to_string(),
        stateset_core::RoundingMode::Up => "up".to_string(),
        stateset_core::RoundingMode::Down => "down".to_string(),
        stateset_core::RoundingMode::HalfEven => "half_even".to_string(),
    }
}

/// Exchange rate between currencies.
#[pyclass]
#[derive(Clone)]
pub struct ExchangeRate {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    base_currency: String,
    #[pyo3(get)]
    quote_currency: String,
    #[pyo3(get)]
    rate: f64,
    #[pyo3(get)]
    source: String,
    #[pyo3(get)]
    rate_at: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::ExchangeRate> for ExchangeRate {
    fn from(r: stateset_core::ExchangeRate) -> Self {
        Self {
            id: r.id.to_string(),
            base_currency: r.base_currency.code().to_string(),
            quote_currency: r.quote_currency.code().to_string(),
            rate: dec_to_f64(&r.rate),
            source: r.source,
            rate_at: r.rate_at.to_rfc3339(),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

/// Result of a currency conversion.
#[pyclass]
#[derive(Clone)]
pub struct ConversionResult {
    #[pyo3(get)]
    original_amount: f64,
    #[pyo3(get)]
    original_currency: String,
    #[pyo3(get)]
    converted_amount: f64,
    #[pyo3(get)]
    target_currency: String,
    #[pyo3(get)]
    rate: f64,
    #[pyo3(get)]
    inverse_rate: f64,
    #[pyo3(get)]
    rate_at: String,
}

impl From<stateset_core::ConversionResult> for ConversionResult {
    fn from(r: stateset_core::ConversionResult) -> Self {
        Self {
            original_amount: dec_to_f64(&r.original_amount),
            original_currency: r.original_currency.code().to_string(),
            converted_amount: dec_to_f64(&r.converted_amount),
            target_currency: r.target_currency.code().to_string(),
            rate: dec_to_f64(&r.rate),
            inverse_rate: dec_to_f64(&r.inverse_rate),
            rate_at: r.rate_at.to_rfc3339(),
        }
    }
}

/// Store currency settings.
#[pyclass]
#[derive(Clone)]
pub struct StoreCurrencySettings {
    #[pyo3(get)]
    base_currency: String,
    #[pyo3(get)]
    enabled_currencies: Vec<String>,
    #[pyo3(get)]
    auto_convert: bool,
    #[pyo3(get)]
    rounding_mode: String,
}

impl From<stateset_core::StoreCurrencySettings> for StoreCurrencySettings {
    fn from(s: stateset_core::StoreCurrencySettings) -> Self {
        Self {
            base_currency: s.base_currency.code().to_string(),
            enabled_currencies: s.enabled_currencies.iter().map(|c| c.code().to_string()).collect(),
            auto_convert: s.auto_convert,
            rounding_mode: rounding_mode_to_string(&s.rounding_mode),
        }
    }
}

/// Input for setting an exchange rate.
#[pyclass]
#[derive(Clone)]
pub struct SetExchangeRateInput {
    #[pyo3(get, set)]
    base_currency: String,
    #[pyo3(get, set)]
    quote_currency: String,
    #[pyo3(get, set)]
    rate: f64,
    #[pyo3(get, set)]
    source: Option<String>,
}

#[pymethods]
impl SetExchangeRateInput {
    #[new]
    #[pyo3(signature = (base_currency, quote_currency, rate, source=None))]
    fn new(
        base_currency: String,
        quote_currency: String,
        rate: f64,
        source: Option<String>,
    ) -> Self {
        Self {
            base_currency,
            quote_currency,
            rate,
            source,
        }
    }
}

/// Currency and exchange rate operations.
#[pyclass]
pub struct CurrencyOperations {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl CurrencyOperations {
    /// Get exchange rate between two currencies.
    fn get_rate(&self, from_currency: String, to_currency: String) -> PyResult<Option<ExchangeRate>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let rate = commerce
            .currency()
            .get_rate(parse_currency(&from_currency)?, parse_currency(&to_currency)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get rate: {}", e)))?;

        Ok(rate.map(|r| r.into()))
    }

    /// Get all exchange rates for a base currency.
    fn get_rates_for(&self, base_currency: String) -> PyResult<Vec<ExchangeRate>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let rates = commerce
            .currency()
            .get_rates_for(parse_currency(&base_currency)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    /// List exchange rates with optional filtering.
    #[pyo3(signature = (base_currency=None, quote_currency=None))]
    fn list_rates(
        &self,
        base_currency: Option<String>,
        quote_currency: Option<String>,
    ) -> PyResult<Vec<ExchangeRate>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let base = match base_currency {
            Some(c) => Some(parse_currency(&c)?),
            None => None,
        };
        let quote = match quote_currency {
            Some(c) => Some(parse_currency(&c)?),
            None => None,
        };

        let rates = commerce
            .currency()
            .list_rates(stateset_core::ExchangeRateFilter {
                base_currency: base,
                quote_currency: quote,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list rates: {}", e)))?;

        Ok(rates.into_iter().map(|r| r.into()).collect())
    }

    /// Set an exchange rate.
    #[pyo3(signature = (base_currency, quote_currency, rate, source=None))]
    fn set_rate(
        &self,
        base_currency: String,
        quote_currency: String,
        rate: f64,
        source: Option<String>,
    ) -> PyResult<ExchangeRate> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let rate_dec = Decimal::from_f64_retain(rate)
            .ok_or_else(|| PyValueError::new_err("Invalid exchange rate"))?;

        let rate = commerce
            .currency()
            .set_rate(stateset_core::SetExchangeRate {
                base_currency: parse_currency(&base_currency)?,
                quote_currency: parse_currency(&quote_currency)?,
                rate: rate_dec,
                source,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set rate: {}", e)))?;

        Ok(rate.into())
    }

    /// Set multiple exchange rates.
    fn set_rates(&self, rates: Vec<SetExchangeRateInput>) -> PyResult<Vec<ExchangeRate>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let mut inputs = Vec::with_capacity(rates.len());
        for r in rates {
            let rate_dec = Decimal::from_f64_retain(r.rate)
                .ok_or_else(|| PyValueError::new_err("Invalid exchange rate"))?;

            inputs.push(stateset_core::SetExchangeRate {
                base_currency: parse_currency(&r.base_currency)?,
                quote_currency: parse_currency(&r.quote_currency)?,
                rate: rate_dec,
                source: r.source,
            });
        }

        let results = commerce
            .currency()
            .set_rates(inputs)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set rates: {}", e)))?;

        Ok(results.into_iter().map(|r| r.into()).collect())
    }

    /// Delete an exchange rate by ID.
    fn delete_rate(&self, id: String) -> PyResult<()> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = id.parse()
            .map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .currency()
            .delete_rate(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete rate: {}", e)))?;

        Ok(())
    }

    /// Convert an amount from one currency to another.
    fn convert(&self, from_currency: String, to_currency: String, amount: f64) -> PyResult<ConversionResult> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let amount_dec = Decimal::from_f64_retain(amount)
            .ok_or_else(|| PyValueError::new_err("Invalid amount"))?;

        let result = commerce
            .currency()
            .convert(stateset_core::ConvertCurrency {
                from: parse_currency(&from_currency)?,
                to: parse_currency(&to_currency)?,
                amount: amount_dec,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to convert currency: {}", e)))?;

        Ok(result.into())
    }

    /// Get store currency settings.
    fn get_settings(&self) -> PyResult<StoreCurrencySettings> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let settings = commerce
            .currency()
            .get_settings()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Update store currency settings.
    #[pyo3(signature = (base_currency, enabled_currencies, auto_convert=None, rounding_mode=None))]
    fn update_settings(
        &self,
        base_currency: String,
        enabled_currencies: Vec<String>,
        auto_convert: Option<bool>,
        rounding_mode: Option<String>,
    ) -> PyResult<StoreCurrencySettings> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let mut enabled = Vec::with_capacity(enabled_currencies.len());
        for c in &enabled_currencies {
            enabled.push(parse_currency(c)?);
        }

        let settings = commerce
            .currency()
            .update_settings(stateset_core::StoreCurrencySettings {
                base_currency: parse_currency(&base_currency)?,
                enabled_currencies: enabled,
                auto_convert: auto_convert.unwrap_or(true),
                rounding_mode: rounding_mode.as_deref().map(parse_rounding_mode).unwrap_or_default(),
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Set the store's base currency.
    fn set_base_currency(&self, currency_code: String) -> PyResult<StoreCurrencySettings> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let settings = commerce
            .currency()
            .set_base_currency(parse_currency(&currency_code)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set base currency: {}", e)))?;

        Ok(settings.into())
    }

    /// Enable currencies for the store.
    fn enable_currencies(&self, currency_codes: Vec<String>) -> PyResult<StoreCurrencySettings> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let mut currencies = Vec::with_capacity(currency_codes.len());
        for c in &currency_codes {
            currencies.push(parse_currency(c)?);
        }

        let settings = commerce
            .currency()
            .enable_currencies(currencies)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to enable currencies: {}", e)))?;

        Ok(settings.into())
    }

    /// Check if a currency is enabled for the store.
    fn is_enabled(&self, currency_code: String) -> PyResult<bool> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        commerce
            .currency()
            .is_enabled(parse_currency(&currency_code)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to check currency: {}", e)))
    }

    /// Get the store's base currency code.
    fn base_currency(&self) -> PyResult<String> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let currency = commerce
            .currency()
            .base_currency()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get base currency: {}", e)))?;

        Ok(currency.code().to_string())
    }

    /// Get enabled currency codes.
    fn enabled_currencies(&self) -> PyResult<Vec<String>> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let currencies = commerce
            .currency()
            .enabled_currencies()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get enabled currencies: {}", e)))?;

        Ok(currencies.iter().map(|c| c.code().to_string()).collect())
    }

    /// Format an amount with currency symbol.
    fn format(&self, amount: f64, currency_code: String) -> PyResult<String> {
        let commerce = self.commerce.lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let amount_dec = Decimal::from_f64_retain(amount)
            .ok_or_else(|| PyValueError::new_err("Invalid amount"))?;

        Ok(commerce
            .currency()
            .format(amount_dec, parse_currency(&currency_code)?))
    }
}

// ============================================================================
// Module Definition
// ============================================================================

/// StateSet Embedded Commerce - Local-first commerce library
#[pymodule]
fn stateset_embedded(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Core
    m.add_class::<Commerce>()?;

    // Customers
    m.add_class::<Customers>()?;
    m.add_class::<Customer>()?;

    // Orders
    m.add_class::<Orders>()?;
    m.add_class::<Order>()?;
    m.add_class::<OrderItem>()?;
    m.add_class::<CreateOrderItemInput>()?;

    // Products
    m.add_class::<Products>()?;
    m.add_class::<Product>()?;
    m.add_class::<ProductVariant>()?;
    m.add_class::<CreateProductVariantInput>()?;

    // Inventory
    m.add_class::<Inventory>()?;
    m.add_class::<InventoryItem>()?;
    m.add_class::<StockLevel>()?;
    m.add_class::<Reservation>()?;

    // Returns
    m.add_class::<Returns>()?;
    m.add_class::<Return>()?;
    m.add_class::<CreateReturnItemInput>()?;

    // Payments
    m.add_class::<Payments>()?;
    m.add_class::<Payment>()?;
    m.add_class::<Refund>()?;

    // Shipments
    m.add_class::<Shipments>()?;
    m.add_class::<Shipment>()?;

    // Warranties
    m.add_class::<Warranties>()?;
    m.add_class::<Warranty>()?;
    m.add_class::<WarrantyClaim>()?;

    // Purchase Orders
    m.add_class::<PurchaseOrders>()?;
    m.add_class::<Supplier>()?;
    m.add_class::<PurchaseOrder>()?;

    // Invoices
    m.add_class::<Invoices>()?;
    m.add_class::<Invoice>()?;

    // Bill of Materials
    m.add_class::<BomApi>()?;
    m.add_class::<Bom>()?;
    m.add_class::<BomComponent>()?;

    // Work Orders
    m.add_class::<WorkOrders>()?;
    m.add_class::<WorkOrder>()?;

    // Carts
    m.add_class::<Carts>()?;
    m.add_class::<Cart>()?;
    m.add_class::<CartItem>()?;
    m.add_class::<CartAddress>()?;
    m.add_class::<AddCartItemInput>()?;
    m.add_class::<ShippingRate>()?;
    m.add_class::<CheckoutResult>()?;

    // Analytics
    m.add_class::<Analytics>()?;
    m.add_class::<SalesSummary>()?;
    m.add_class::<RevenueByPeriod>()?;
    m.add_class::<TopProduct>()?;
    m.add_class::<ProductPerformance>()?;
    m.add_class::<CustomerMetrics>()?;
    m.add_class::<TopCustomer>()?;
    m.add_class::<InventoryHealth>()?;
    m.add_class::<LowStockItem>()?;
    m.add_class::<InventoryMovement>()?;
    m.add_class::<OrderStatusBreakdown>()?;
    m.add_class::<FulfillmentMetrics>()?;
    m.add_class::<ReturnMetrics>()?;
    m.add_class::<DemandForecast>()?;
    m.add_class::<RevenueForecast>()?;

    // Currency
    m.add_class::<CurrencyOperations>()?;
    m.add_class::<ExchangeRate>()?;
    m.add_class::<ConversionResult>()?;
    m.add_class::<StoreCurrencySettings>()?;
    m.add_class::<SetExchangeRateInput>()?;

    Ok(())
}
