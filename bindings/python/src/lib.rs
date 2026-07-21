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
// PyO3 APIs intentionally expose rich keyword argument signatures to Python callers.
#![allow(clippy::too_many_arguments)]

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::marker::Ungil;
use pyo3::prelude::*;
use rust_decimal::Decimal;
// Use :: prefix to refer to the external crate, not the pymodule
use ::stateset_embedded::Commerce as RustCommerce;
use stateset_primitives::CurrencyCode;
use stateset_sdk::sync::SyncEvent as RustSyncEvent;
use stateset_sdk::{SyncRuntime as RustSyncRuntime, SyncRuntimeConfig as RustSyncRuntimeConfig};
use std::sync::{Arc, Mutex};

fn decimal_from_f64(value: f64, field: &str) -> PyResult<Decimal> {
    Decimal::from_f64_retain(value).ok_or_else(|| {
        PyValueError::new_err(format!("Invalid {field}: expected a finite decimal value"))
    })
}

fn optional_decimal_from_f64(value: Option<f64>, field: &str) -> PyResult<Option<Decimal>> {
    value.map(|value| decimal_from_f64(value, field)).transpose()
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

fn to_f64_result<T>(value: T, field: &str) -> PyResult<f64>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    value.try_into().map_err(|err| {
        PyRuntimeError::new_err(format!("Failed to convert {field} to float: {err}"))
    })
}

fn optional_to_f64_result<T>(value: Option<T>, field: &str) -> PyResult<Option<f64>>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    value.map(|inner| to_f64_result(inner, field)).transpose()
}

fn convert_output<T, U>(value: T) -> PyResult<U>
where
    U: TryFrom<T, Error = PyErr>,
{
    U::try_from(value)
}

fn convert_optional_output<T, U>(value: Option<T>) -> PyResult<Option<U>>
where
    U: TryFrom<T, Error = PyErr>,
{
    value.map(convert_output).transpose()
}

fn convert_outputs<T, U>(values: Vec<T>) -> PyResult<Vec<U>>
where
    U: TryFrom<T, Error = PyErr>,
{
    values.into_iter().map(convert_output).collect()
}

fn sync_runtime_error<E>(context: &str, error: E) -> PyErr
where
    E: std::fmt::Display,
{
    PyRuntimeError::new_err(format!("{context}: {error}"))
}

fn serialize_json<T>(value: &T, label: &str) -> PyResult<String>
where
    T: serde::Serialize,
{
    serde_json::to_string(value)
        .map_err(|error| PyRuntimeError::new_err(format!("Failed to serialize {label}: {error}")))
}

fn serialize_json_pretty<T>(value: &T, label: &str) -> PyResult<String>
where
    T: serde::Serialize,
{
    serde_json::to_string_pretty(value)
        .map_err(|error| PyRuntimeError::new_err(format!("Failed to serialize {label}: {error}")))
}

fn parse_json_value(value: &str, field: &str) -> PyResult<serde_json::Value> {
    serde_json::from_str(value)
        .map_err(|error| PyValueError::new_err(format!("Invalid {field} JSON: {error}")))
}

fn parse_uuid_str(value: &str, field: &str) -> PyResult<uuid::Uuid> {
    value.parse().map_err(|error| PyValueError::new_err(format!("Invalid {field} UUID: {error}")))
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn sync_sequence_authority_name(event: &RustSyncEvent) -> &'static str {
    if event.is_canonical_remote() { "canonical_remote" } else { "local_outbox" }
}

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
        let commerce = RustCommerce::new(&db_path).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to initialize commerce: {}", e))
        })?;

        Ok(Self { inner: Arc::new(Mutex::new(commerce)) })
    }

    /// Get the customers API.
    #[getter]
    fn customers(&self) -> Customers {
        Customers { commerce: self.inner.clone() }
    }

    /// Get the orders API.
    #[getter]
    fn orders(&self) -> Orders {
        Orders { commerce: self.inner.clone() }
    }

    /// Get the products API.
    #[getter]
    fn products(&self) -> Products {
        Products { commerce: self.inner.clone() }
    }

    /// Get the custom objects API (custom states / metaobjects).
    #[getter]
    fn custom_objects(&self) -> CustomObjectsApi {
        CustomObjectsApi { commerce: self.inner.clone() }
    }

    /// Alias for `custom_objects` (for users who prefer the "custom states" name).
    #[getter]
    fn custom_states(&self) -> CustomObjectsApi {
        self.custom_objects()
    }

    /// Get the inventory API.
    #[getter]
    fn inventory(&self) -> Inventory {
        Inventory { commerce: self.inner.clone() }
    }

    /// Get the returns API.
    #[getter]
    fn returns(&self) -> Returns {
        Returns { commerce: self.inner.clone() }
    }

    /// Get the gift cards API.
    #[getter]
    fn gift_cards(&self) -> GiftCards {
        GiftCards { commerce: self.inner.clone() }
    }

    /// Get the loyalty API.
    #[getter]
    fn loyalty(&self) -> Loyalty {
        Loyalty { commerce: self.inner.clone() }
    }

    /// Get the store credits API.
    #[getter]
    fn store_credits(&self) -> StoreCredits {
        StoreCredits { commerce: self.inner.clone() }
    }

    /// Get the product reviews API.
    #[getter]
    fn reviews(&self) -> Reviews {
        Reviews { commerce: self.inner.clone() }
    }

    /// Get the wishlists API.
    #[getter]
    fn wishlists(&self) -> Wishlists {
        Wishlists { commerce: self.inner.clone() }
    }

    /// Get the customer segments API.
    #[getter]
    fn segments(&self) -> Segments {
        Segments { commerce: self.inner.clone() }
    }

    /// Get the payments API.
    #[getter]
    fn payments(&self) -> Payments {
        Payments { commerce: self.inner.clone() }
    }

    /// Get the shipments API.
    #[getter]
    fn shipments(&self) -> Shipments {
        Shipments { commerce: self.inner.clone() }
    }

    /// Get the warranties API.
    #[getter]
    fn warranties(&self) -> Warranties {
        Warranties { commerce: self.inner.clone() }
    }

    /// Get the purchase orders API.
    #[getter]
    fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders { commerce: self.inner.clone() }
    }

    /// Get the invoices API.
    #[getter]
    fn invoices(&self) -> Invoices {
        Invoices { commerce: self.inner.clone() }
    }

    /// Get the bill of materials API.
    #[getter]
    fn bom(&self) -> BomApi {
        BomApi { commerce: self.inner.clone() }
    }

    /// Get the work orders API.
    #[getter]
    fn work_orders(&self) -> WorkOrders {
        WorkOrders { commerce: self.inner.clone() }
    }

    /// Get the carts API.
    #[getter]
    fn carts(&self) -> Carts {
        Carts { commerce: self.inner.clone() }
    }

    /// Get the analytics API.
    #[getter]
    fn analytics(&self) -> Analytics {
        Analytics { commerce: self.inner.clone() }
    }

    /// Get the currency API.
    #[getter]
    fn currency(&self) -> CurrencyOperations {
        CurrencyOperations { commerce: self.inner.clone() }
    }

    /// Get the subscriptions API.
    #[getter]
    fn subscriptions(&self) -> Subscriptions {
        Subscriptions { commerce: self.inner.clone() }
    }

    /// Get the promotions API.
    #[getter]
    fn promotions(&self) -> PromotionsApi {
        PromotionsApi { commerce: self.inner.clone() }
    }

    /// Get the tax API.
    #[getter]
    fn tax(&self) -> TaxApi {
        TaxApi { commerce: self.inner.clone() }
    }

    /// Get the quality control API.
    #[getter]
    fn quality(&self) -> QualityApi {
        QualityApi { commerce: self.inner.clone() }
    }

    /// Get the lots/batch tracking API.
    #[getter]
    fn lots(&self) -> LotsApi {
        LotsApi { commerce: self.inner.clone() }
    }

    /// Get the serial numbers API.
    #[getter]
    fn serials(&self) -> SerialsApi {
        SerialsApi { commerce: self.inner.clone() }
    }

    /// Get the warehouse API.
    #[getter]
    fn warehouse(&self) -> WarehouseApi {
        WarehouseApi { commerce: self.inner.clone() }
    }

    /// Get the receiving API.
    #[getter]
    fn receiving(&self) -> ReceivingApi {
        ReceivingApi { commerce: self.inner.clone() }
    }

    /// Get the fulfillment API.
    #[getter]
    fn fulfillment(&self) -> FulfillmentApi {
        FulfillmentApi { commerce: self.inner.clone() }
    }

    /// Get the accounts payable API.
    #[getter]
    fn accounts_payable(&self) -> AccountsPayableApi {
        AccountsPayableApi { commerce: self.inner.clone() }
    }

    /// Get the accounts receivable API.
    #[getter]
    fn accounts_receivable(&self) -> AccountsReceivableApi {
        AccountsReceivableApi { commerce: self.inner.clone() }
    }

    /// Get the cost accounting API.
    #[getter]
    fn cost_accounting(&self) -> CostAccountingApi {
        CostAccountingApi { commerce: self.inner.clone() }
    }

    /// Get the credit management API.
    #[getter]
    fn credit(&self) -> CreditApi {
        CreditApi { commerce: self.inner.clone() }
    }

    /// Get the backorder management API.
    #[getter]
    fn backorder(&self) -> BackorderApi {
        BackorderApi { commerce: self.inner.clone() }
    }

    /// Get the general ledger API.
    #[getter]
    fn general_ledger(&self) -> GeneralLedgerApi {
        GeneralLedgerApi { commerce: self.inner.clone() }
    }

    /// Get the fixed assets API.
    #[getter]
    fn fixed_assets(&self) -> FixedAssets {
        FixedAssets { commerce: self.inner.clone() }
    }

    /// Get the revenue recognition (ASC 606) API.
    #[getter]
    fn revenue_recognition(&self) -> RevenueRecognition {
        RevenueRecognition { commerce: self.inner.clone() }
    }

    /// Get the cycle counts API.
    #[getter]
    fn cycle_counts(&self) -> CycleCounts {
        CycleCounts { commerce: self.inner.clone() }
    }

    /// Get the vector search API for semantic search operations.
    ///
    /// Requires OPENAI_API_KEY environment variable to be set.
    ///
    /// Example:
    ///     vector = commerce.vector("sk-...")
    ///     results = vector.search_products("wireless bluetooth headphones", limit=10)
    fn vector(&self, openai_api_key: String) -> PyResult<VectorSearch> {
        Ok(VectorSearch { commerce: self.inner.clone(), api_key: openai_api_key })
    }
}

// ============================================================================
// Customer Types
// ============================================================================

/// Customer data returned from operations.
#[pyclass(skip_from_py_object)]
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let customer = commerce
            .customers()
            .get(uuid.into())
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::Order> for Order {
    type Error = PyErr;

    fn try_from(o: stateset_core::Order) -> PyResult<Self> {
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
                    Ok(OrderItem {
                        id: i.id.to_string(),
                        sku: i.sku,
                        name: i.name,
                        quantity: i.quantity,
                        unit_price: to_f64_result(i.unit_price, "order item unit price")?,
                        total: to_f64_result(i.total, "order item total")?,
                    })
                })
                .collect::<PyResult<Vec<_>>>()?,
            version: o.version,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        })
    }
}

/// Input for creating an order item.
#[pyclass(from_py_object)]
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
        Self { sku, name, quantity, unit_price, product_id, variant_id }
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let order_items: Vec<stateset_core::CreateOrderItem> = items
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
                    unit_price: decimal_from_f64(i.unit_price, "unit_price")?,
                    ..Default::default()
                })
            })
            .collect::<PyResult<Vec<_>>>()?;

        let order = commerce
            .orders()
            .create(stateset_core::CreateOrder {
                customer_id: cust_uuid,
                items: order_items,
                currency: currency.as_ref().and_then(|s| s.parse::<CurrencyCode>().ok()),
                notes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create order: {}", e)))?;

        convert_output(order)
    }

    /// Get an order by ID.
    ///
    /// Args:
    ///     id: Order UUID
    ///
    /// Returns:
    ///     Order or None if not found
    fn get(&self, id: String) -> PyResult<Option<Order>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order = commerce
            .orders()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get order: {}", e)))?;

        convert_optional_output(order)
    }

    /// List all orders.
    ///
    /// Returns:
    ///     List[Order]: All orders
    fn list(&self) -> PyResult<Vec<Order>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let orders = commerce
            .orders()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list orders: {}", e)))?;

        convert_outputs(orders)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

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
            .update_status(uuid.into(), order_status)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update order: {}", e)))?;

        convert_output(order)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order = commerce
            .orders()
            .ship(uuid.into(), tracking_number.as_deref())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to ship order: {}", e)))?;

        convert_output(order)
    }

    /// Cancel an order.
    ///
    /// Args:
    ///     id: Order UUID
    ///
    /// Returns:
    ///     Order: The cancelled order
    fn cancel(&self, id: String) -> PyResult<Order> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let order = commerce
            .orders()
            .cancel(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel order: {}", e)))?;

        convert_output(order)
    }

    /// Count orders.
    ///
    /// Returns:
    ///     int: Number of orders
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::ProductVariant> for ProductVariant {
    type Error = PyErr;

    fn try_from(v: stateset_core::ProductVariant) -> PyResult<Self> {
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

/// Input for creating a product variant.
#[pyclass(from_py_object)]
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
        Self { sku, name, price, compare_at_price }
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let variant_inputs = variants
            .map(|vs| {
                vs.into_iter()
                    .map(|v| {
                        Ok(stateset_core::CreateProductVariant {
                            sku: v.sku,
                            name: v.name,
                            price: decimal_from_f64(v.price, "price")?,
                            compare_at_price: optional_decimal_from_f64(
                                v.compare_at_price,
                                "compare_at_price",
                            )?,
                            ..Default::default()
                        })
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;

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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let product = commerce
            .products()
            .get(uuid.into())
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let variant = commerce
            .products()
            .get_variant_by_sku(&sku)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get variant: {}", e)))?;

        convert_optional_output(variant)
    }

    /// List all products.
    ///
    /// Returns:
    ///     List[Product]: All products
    fn list(&self) -> PyResult<Vec<Product>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let count = commerce
            .products()
            .count(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to count products: {}", e)))?;

        Ok(count as u32)
    }
}

// ============================================================================
// Custom Objects (Custom States / Metaobjects)
// ============================================================================

/// Custom field definition (output).
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CustomFieldDefinition {
    #[pyo3(get)]
    key: String,
    #[pyo3(get)]
    field_type: String,
    #[pyo3(get)]
    required: bool,
    #[pyo3(get)]
    list: bool,
    #[pyo3(get)]
    description: Option<String>,
}

impl From<stateset_core::CustomFieldDefinition> for CustomFieldDefinition {
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

/// Input for defining a custom field in a type schema.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct CustomFieldDefinitionInput {
    #[pyo3(get, set)]
    key: String,
    #[pyo3(get, set)]
    field_type: String,
    #[pyo3(get, set)]
    required: bool,
    #[pyo3(get, set)]
    list: bool,
    #[pyo3(get, set)]
    description: Option<String>,
}

#[pymethods]
impl CustomFieldDefinitionInput {
    #[new]
    #[pyo3(signature = (key, field_type, required=false, list=false, description=None))]
    fn new(
        key: String,
        field_type: String,
        required: bool,
        list: bool,
        description: Option<String>,
    ) -> Self {
        Self { key, field_type, required, list, description }
    }
}

/// Custom object type (schema) output.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CustomObjectType {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    handle: String,
    #[pyo3(get)]
    display_name: String,
    #[pyo3(get)]
    description: String,
    #[pyo3(get)]
    fields: Vec<CustomFieldDefinition>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
    #[pyo3(get)]
    version: i32,
}

impl From<stateset_core::CustomObjectType> for CustomObjectType {
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

/// Custom object record output.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CustomObject {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    type_id: String,
    #[pyo3(get)]
    type_handle: String,
    #[pyo3(get)]
    handle: Option<String>,
    #[pyo3(get)]
    owner_type: Option<String>,
    #[pyo3(get)]
    owner_id: Option<String>,
    /// Values JSON string (always an object).
    #[pyo3(get)]
    values_json: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
    #[pyo3(get)]
    version: i32,
}

impl From<stateset_core::CustomObject> for CustomObject {
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

fn parse_custom_field_type(s: &str) -> PyResult<stateset_core::CustomFieldType> {
    s.parse::<stateset_core::CustomFieldType>()
        .map_err(|e| PyValueError::new_err(format!("Invalid custom field type '{}': {}", s, e)))
}

/// Custom objects API for defining schemas and storing typed records.
#[pyclass]
pub struct CustomObjectsApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl CustomObjectsApi {
    // ------------------------------------------------------------------------
    // Types
    // ------------------------------------------------------------------------

    /// Create a new custom object type (schema).
    #[pyo3(signature = (handle, display_name, description=None, fields=None))]
    fn create_type(
        &self,
        handle: String,
        display_name: String,
        description: Option<String>,
        fields: Option<Vec<CustomFieldDefinitionInput>>,
    ) -> PyResult<CustomObjectType> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let mut out_fields = Vec::new();
        if let Some(fields) = fields {
            out_fields.reserve(fields.len());
            for f in fields {
                out_fields.push(stateset_core::CustomFieldDefinition {
                    key: f.key,
                    field_type: parse_custom_field_type(&f.field_type)?,
                    required: f.required,
                    list: f.list,
                    description: f.description,
                });
            }
        }

        let ty = commerce
            .custom_objects()
            .create_type(stateset_core::CreateCustomObjectType {
                handle,
                display_name,
                description,
                fields: out_fields,
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create custom object type: {}", e))
            })?;

        Ok(ty.into())
    }

    /// Get a custom object type by ID.
    fn get_type(&self, id: String) -> PyResult<Option<CustomObjectType>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ty = commerce.custom_objects().get_type(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get custom object type: {}", e))
        })?;

        Ok(ty.map(|t| t.into()))
    }

    /// Get a custom object type by handle.
    fn get_type_by_handle(&self, handle: String) -> PyResult<Option<CustomObjectType>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let ty = commerce.custom_objects().get_type_by_handle(&handle).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get custom object type: {}", e))
        })?;

        Ok(ty.map(|t| t.into()))
    }

    /// Update a custom object type.
    #[pyo3(signature = (id, display_name=None, description=None, fields=None))]
    fn update_type(
        &self,
        id: String,
        display_name: Option<String>,
        description: Option<String>,
        fields: Option<Vec<CustomFieldDefinitionInput>>,
    ) -> PyResult<CustomObjectType> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let fields = if let Some(fields) = fields {
            let mut out = Vec::with_capacity(fields.len());
            for f in fields {
                out.push(stateset_core::CustomFieldDefinition {
                    key: f.key,
                    field_type: parse_custom_field_type(&f.field_type)?,
                    required: f.required,
                    list: f.list,
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
                stateset_core::UpdateCustomObjectType { display_name, description, fields },
            )
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to update custom object type: {}", e))
            })?;

        Ok(updated.into())
    }

    /// List custom object types.
    #[pyo3(signature = (search=None, limit=None, offset=None))]
    fn list_types(
        &self,
        search: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<CustomObjectType>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let list = commerce
            .custom_objects()
            .list_types(stateset_core::CustomObjectTypeFilter { search, limit, offset })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to list custom object types: {}", e))
            })?;

        Ok(list.into_iter().map(|t| t.into()).collect())
    }

    /// Delete a custom object type.
    fn delete_type(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce.custom_objects().delete_type(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to delete custom object type: {}", e))
        })?;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------------

    /// Create a new custom object record.
    #[pyo3(signature = (type_handle, values_json, handle=None, owner_type=None, owner_id=None))]
    fn create_object(
        &self,
        type_handle: String,
        values_json: String,
        handle: Option<String>,
        owner_type: Option<String>,
        owner_id: Option<String>,
    ) -> PyResult<CustomObject> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let values: serde_json::Value = serde_json::from_str(&values_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid values_json: {}", e)))?;

        let obj = commerce
            .custom_objects()
            .create_object(stateset_core::CreateCustomObject {
                type_handle,
                handle,
                owner_type,
                owner_id,
                values,
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create custom object: {}", e))
            })?;

        Ok(obj.into())
    }

    /// Get a custom object record by ID.
    fn get_object(&self, id: String) -> PyResult<Option<CustomObject>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let obj = commerce
            .custom_objects()
            .get_object(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get custom object: {}", e)))?;

        Ok(obj.map(|o| o.into()))
    }

    /// Get a custom object record by type handle and object handle.
    fn get_object_by_handle(
        &self,
        type_handle: String,
        object_handle: String,
    ) -> PyResult<Option<CustomObject>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let obj = commerce
            .custom_objects()
            .get_object_by_handle(&type_handle, &object_handle)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get custom object: {}", e)))?;

        Ok(obj.map(|o| o.into()))
    }

    /// Update a custom object record.
    #[pyo3(signature = (id, handle=None, owner_type=None, owner_id=None, values_json=None))]
    fn update_object(
        &self,
        id: String,
        handle: Option<String>,
        owner_type: Option<String>,
        owner_id: Option<String>,
        values_json: Option<String>,
    ) -> PyResult<CustomObject> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let values = if let Some(values_json) = values_json {
            Some(
                serde_json::from_str(&values_json)
                    .map_err(|e| PyValueError::new_err(format!("Invalid values_json: {}", e)))?,
            )
        } else {
            None
        };

        let updated = commerce
            .custom_objects()
            .update_object(
                uuid,
                stateset_core::UpdateCustomObject { handle, owner_type, owner_id, values },
            )
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to update custom object: {}", e))
            })?;

        Ok(updated.into())
    }

    /// List custom object records.
    #[pyo3(signature = (type_handle=None, owner_type=None, owner_id=None, handle=None, limit=None, offset=None))]
    fn list_objects(
        &self,
        type_handle: Option<String>,
        owner_type: Option<String>,
        owner_id: Option<String>,
        handle: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<CustomObject>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let list = commerce
            .custom_objects()
            .list_objects(stateset_core::CustomObjectFilter {
                type_handle,
                owner_type,
                owner_id,
                handle,
                limit,
                offset,
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to list custom objects: {}", e))
            })?;

        Ok(list.into_iter().map(|o| o.into()).collect())
    }

    /// Delete a custom object record.
    fn delete_object(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce.custom_objects().delete_object(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to delete custom object: {}", e))
        })?;

        Ok(())
    }
}

// ============================================================================
// Inventory Types
// ============================================================================

/// Inventory item data.
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
        format!("StockLevel(sku='{}', available={})", self.sku, self.total_available)
    }
}

impl TryFrom<stateset_core::StockLevel> for StockLevel {
    type Error = PyErr;

    fn try_from(s: stateset_core::StockLevel) -> PyResult<Self> {
        Ok(Self {
            sku: s.sku,
            name: s.name,
            total_on_hand: to_f64_result(s.total_on_hand, "stock level total on hand")?,
            total_allocated: to_f64_result(s.total_allocated, "stock level total allocated")?,
            total_available: to_f64_result(s.total_available, "stock level total available")?,
        })
    }
}

/// Inventory reservation.
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::InventoryReservation> for Reservation {
    type Error = PyErr;

    fn try_from(r: stateset_core::InventoryReservation) -> PyResult<Self> {
        Ok(Self {
            id: r.id.to_string(),
            item_id: r.item_id,
            quantity: to_f64_result(r.quantity, "inventory reservation quantity")?,
            status: format!("{}", r.status),
        })
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let item = commerce
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku,
                name,
                description,
                initial_quantity: optional_decimal_from_f64(initial_quantity, "initial_quantity")?,
                reorder_point: optional_decimal_from_f64(reorder_point, "reorder_point")?,
                ..Default::default()
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create inventory item: {}", e))
            })?;

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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let stock = commerce
            .inventory()
            .get_stock(&sku)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get stock: {}", e)))?;

        convert_optional_output(stock)
    }

    /// Adjust inventory quantity.
    ///
    /// Args:
    ///     sku: Stock keeping unit
    ///     quantity: Quantity to add (positive) or remove (negative)
    ///     reason: Reason for adjustment
    fn adjust(&self, sku: String, quantity: f64, reason: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let qty = Decimal::from_f64_retain(quantity)
            .ok_or_else(|| PyValueError::new_err("Invalid quantity"))?;

        let reservation = commerce
            .inventory()
            .reserve(&sku, qty, &reference_type, &reference_id, expires_in_seconds)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to reserve inventory: {}", e)))?;

        convert_output(reservation)
    }

    /// Confirm a reservation (deducts from on-hand).
    ///
    /// Args:
    ///     reservation_id: Reservation UUID
    fn confirm_reservation(&self, reservation_id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = reservation_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce.inventory().confirm_reservation(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to confirm reservation: {}", e))
        })?;

        Ok(())
    }

    /// Release a reservation (returns to available).
    ///
    /// Args:
    ///     reservation_id: Reservation UUID
    fn release_reservation(&self, reservation_id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = reservation_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce.inventory().release_reservation(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to release reservation: {}", e))
        })?;

        Ok(())
    }
}

// ============================================================================
// Return Types
// ============================================================================

/// Return request data.
#[pyclass(skip_from_py_object)]
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
    idempotency_key: Option<String>,
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
            idempotency_key: r.idempotency_key,
            version: r.version,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// Input for creating a return item.
#[pyclass(from_py_object)]
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
        Self { order_item_id, quantity }
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
    #[pyo3(signature = (order_id, reason, items, reason_details=None, idempotency_key=None))]
    fn create(
        &self,
        order_id: String,
        reason: String,
        items: Vec<CreateReturnItemInput>,
        reason_details: Option<String>,
        idempotency_key: Option<String>,
    ) -> PyResult<Return> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let ord_uuid = order_id.parse().map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

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
                idempotency_key,
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .get(uuid.into())
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .approve(uuid.into())
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let ret = commerce
            .returns()
            .reject(uuid.into(), &reason)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to reject return: {}", e)))?;

        Ok(ret.into())
    }

    /// List all returns.
    ///
    /// Returns:
    ///     List[Return]: All returns
    fn list(&self) -> PyResult<Vec<Return>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
    idempotency_key: Option<String>,
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

impl TryFrom<stateset_core::Payment> for Payment {
    type Error = PyErr;

    fn try_from(p: stateset_core::Payment) -> PyResult<Self> {
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
            payment_method: format!("{}", p.payment_method),
            version: p.version,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

/// Refund data returned from operations.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Refund {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    payment_id: String,
    #[pyo3(get)]
    idempotency_key: Option<String>,
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

impl TryFrom<stateset_core::Refund> for Refund {
    type Error = PyErr;

    fn try_from(r: stateset_core::Refund) -> PyResult<Self> {
        Ok(Self {
            id: r.id.to_string(),
            payment_id: r.payment_id.to_string(),
            idempotency_key: r.idempotency_key,
            amount: to_f64_result(r.amount, "refund amount")?,
            status: format!("{}", r.status),
            reason: r.reason,
            created_at: r.created_at.to_rfc3339(),
        })
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
    #[pyo3(signature = (amount, currency=None, order_id=None, customer_id=None, payment_method=None, idempotency_key=None))]
    fn create(
        &self,
        amount: f64,
        currency: Option<String>,
        order_id: Option<String>,
        customer_id: Option<String>,
        payment_method: Option<String>,
        idempotency_key: Option<String>,
    ) -> PyResult<Payment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let order_uuid = order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let customer_uuid = customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let method = payment_method
            .map(|m| match m.to_lowercase().as_str() {
                "credit_card" => stateset_core::PaymentMethodType::CreditCard,
                "debit_card" => stateset_core::PaymentMethodType::DebitCard,
                "bank_transfer" => stateset_core::PaymentMethodType::BankTransfer,
                "paypal" => stateset_core::PaymentMethodType::PayPal,
                "crypto" => stateset_core::PaymentMethodType::Crypto,
                _ => stateset_core::PaymentMethodType::CreditCard,
            })
            .unwrap_or(stateset_core::PaymentMethodType::CreditCard);

        let payment = commerce
            .payments()
            .create(stateset_core::CreatePayment {
                order_id: order_uuid,
                customer_id: customer_uuid,
                idempotency_key,
                amount: decimal_from_f64(amount, "amount")?,
                currency: currency.as_ref().and_then(|s| s.parse::<CurrencyCode>().ok()),
                payment_method: method,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create payment: {}", e)))?;

        convert_output(payment)
    }

    /// Get a payment by ID.
    fn get(&self, id: String) -> PyResult<Option<Payment>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get payment: {}", e)))?;

        convert_optional_output(payment)
    }

    /// List all payments.
    fn list(&self) -> PyResult<Vec<Payment>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let payments = commerce
            .payments()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list payments: {}", e)))?;

        convert_outputs(payments)
    }

    /// Mark payment as completed.
    fn complete(&self, id: String) -> PyResult<Payment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_completed(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to complete payment: {}", e)))?;

        convert_output(payment)
    }

    /// Mark payment as failed.
    #[pyo3(signature = (id, reason, code=None))]
    fn mark_failed(&self, id: String, reason: String, code: Option<String>) -> PyResult<Payment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let payment = commerce
            .payments()
            .mark_failed(uuid.into(), &reason, code.as_deref())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to fail payment: {}", e)))?;

        convert_output(payment)
    }

    /// Create a refund for a payment.
    #[pyo3(signature = (payment_id, amount, reason=None, idempotency_key=None))]
    fn create_refund(
        &self,
        payment_id: String,
        amount: f64,
        reason: Option<String>,
        idempotency_key: Option<String>,
    ) -> PyResult<Refund> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            payment_id.parse().map_err(|_| PyValueError::new_err("Invalid payment UUID"))?;

        let refund = commerce
            .payments()
            .create_refund(stateset_core::CreateRefund {
                payment_id: uuid.into(),
                amount: Some(decimal_from_f64(amount, "amount")?),
                reason,
                idempotency_key,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create refund: {}", e)))?;

        convert_output(refund)
    }

    /// Count payments.
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let order_uuid =
            order_id.parse().map_err(|_| PyValueError::new_err("Invalid order UUID"))?;

        let carrier_type = carrier.map(|c| match c.to_lowercase().as_str() {
            "ups" => stateset_core::ShippingCarrier::Ups,
            "fedex" => stateset_core::ShippingCarrier::FedEx,
            "usps" => stateset_core::ShippingCarrier::Usps,
            "dhl" => stateset_core::ShippingCarrier::Dhl,
            _ => stateset_core::ShippingCarrier::Other,
        });

        let method = shipping_method.map(|m| match m.to_lowercase().as_str() {
            "standard" => stateset_core::ShippingMethod::Standard,
            "express" => stateset_core::ShippingMethod::Express,
            "overnight" => stateset_core::ShippingMethod::Overnight,
            "ground" => stateset_core::ShippingMethod::Ground,
            _ => stateset_core::ShippingMethod::Standard,
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get shipment: {}", e)))?;

        Ok(shipment.map(|s| s.into()))
    }

    /// List all shipments.
    fn list(&self) -> PyResult<Vec<Shipment>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .ship(uuid.into(), tracking_number)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to ship: {}", e)))?;

        Ok(shipment.into())
    }

    /// Mark shipment as delivered.
    fn mark_delivered(&self, id: String) -> PyResult<Shipment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .mark_delivered(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to deliver: {}", e)))?;

        Ok(shipment.into())
    }

    /// Cancel a shipment.
    fn cancel(&self, id: String) -> PyResult<Shipment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let shipment = commerce
            .shipments()
            .cancel(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel shipment: {}", e)))?;

        Ok(shipment.into())
    }

    /// Count shipments.
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let warranty = commerce
            .warranties()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get warranty: {}", e)))?;

        Ok(warranty.map(|w| w.into()))
    }

    /// List all warranties.
    fn list(&self) -> PyResult<Vec<Warranty>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let warranty_uuid =
            warranty_id.parse().map_err(|_| PyValueError::new_err("Invalid warranty UUID"))?;

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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .approve_claim(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to approve claim: {}", e)))?;

        Ok(claim.into())
    }

    /// Deny a warranty claim.
    fn deny_claim(&self, id: String, reason: String) -> PyResult<WarrantyClaim> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .deny_claim(uuid, &reason)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to deny claim: {}", e)))?;

        Ok(claim.into())
    }

    /// Complete a warranty claim with resolution.
    fn complete_claim(&self, id: String, resolution: String) -> PyResult<WarrantyClaim> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

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
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
        format!(
            "PurchaseOrder(number='{}', status='{}', total={})",
            self.po_number, self.status, self.total_amount
        )
    }
}

impl TryFrom<stateset_core::PurchaseOrder> for PurchaseOrder {
    type Error = PyErr;

    fn try_from(po: stateset_core::PurchaseOrder) -> PyResult<Self> {
        Ok(Self {
            id: po.id.to_string(),
            po_number: po.po_number,
            supplier_id: po.supplier_id.to_string(),
            status: format!("{}", po.status),
            total_amount: to_f64_result(po.total, "purchase order total")?,
            created_at: po.created_at.to_rfc3339(),
            updated_at: po.updated_at.to_rfc3339(),
        })
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
    fn create_supplier(
        &self,
        name: String,
        email: Option<String>,
        phone: Option<String>,
    ) -> PyResult<Supplier> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let supplier = commerce
            .purchase_orders()
            .get_supplier(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get supplier: {}", e)))?;

        Ok(supplier.map(|s| s.into()))
    }

    /// List all suppliers.
    fn list_suppliers(&self) -> PyResult<Vec<Supplier>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let suppliers = commerce
            .purchase_orders()
            .list_suppliers(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list suppliers: {}", e)))?;

        Ok(suppliers.into_iter().map(|s| s.into()).collect())
    }

    /// Create a new purchase order.
    fn create(&self, supplier_id: String) -> PyResult<PurchaseOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let supp_uuid =
            supplier_id.parse().map_err(|_| PyValueError::new_err("Invalid supplier UUID"))?;

        let po = commerce
            .purchase_orders()
            .create(stateset_core::CreatePurchaseOrder {
                supplier_id: supp_uuid,
                items: vec![],
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create PO: {}", e)))?;

        convert_output(po)
    }

    /// Get a purchase order by ID.
    fn get(&self, id: String) -> PyResult<Option<PurchaseOrder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get PO: {}", e)))?;

        convert_optional_output(po)
    }

    /// List all purchase orders.
    fn list(&self) -> PyResult<Vec<PurchaseOrder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let pos = commerce
            .purchase_orders()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list POs: {}", e)))?;

        convert_outputs(pos)
    }

    /// Submit PO for approval.
    fn submit(&self, id: String) -> PyResult<PurchaseOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .submit(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to submit PO: {}", e)))?;

        convert_output(po)
    }

    /// Approve a purchase order.
    fn approve(&self, id: String, approved_by: String) -> PyResult<PurchaseOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .approve(uuid, &approved_by)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to approve PO: {}", e)))?;

        convert_output(po)
    }

    /// Send PO to supplier.
    fn send(&self, id: String) -> PyResult<PurchaseOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .send(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to send PO: {}", e)))?;

        convert_output(po)
    }

    /// Cancel a purchase order.
    fn cancel(&self, id: String) -> PyResult<PurchaseOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let po = commerce
            .purchase_orders()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel PO: {}", e)))?;

        convert_output(po)
    }

    /// Count purchase orders.
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
        format!(
            "Invoice(number='{}', status='{}', total={})",
            self.invoice_number, self.status, self.total
        )
    }

    #[getter]
    fn balance_due(&self) -> f64 {
        self.total - self.amount_paid
    }
}

impl TryFrom<stateset_core::Invoice> for Invoice {
    type Error = PyErr;

    fn try_from(inv: stateset_core::Invoice) -> PyResult<Self> {
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
        })
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

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

        convert_output(invoice)
    }

    /// Get an invoice by ID.
    fn get(&self, id: String) -> PyResult<Option<Invoice>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get invoice: {}", e)))?;

        convert_optional_output(invoice)
    }

    /// List all invoices.
    fn list(&self) -> PyResult<Vec<Invoice>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let invoices = commerce
            .invoices()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list invoices: {}", e)))?;

        convert_outputs(invoices)
    }

    /// Send an invoice.
    fn send(&self, id: String) -> PyResult<Invoice> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .send(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to send invoice: {}", e)))?;

        convert_output(invoice)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .record_payment(
                uuid,
                stateset_core::RecordInvoicePayment {
                    amount: decimal_from_f64(amount, "amount")?,
                    payment_method,
                    reference,
                    ..Default::default()
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to record payment: {}", e)))?;

        convert_output(invoice)
    }

    /// Void an invoice.
    fn void(&self, id: String) -> PyResult<Invoice> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let invoice = commerce
            .invoices()
            .void(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to void invoice: {}", e)))?;

        convert_output(invoice)
    }

    /// Get overdue invoices.
    fn get_overdue(&self) -> PyResult<Vec<Invoice>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let invoices = commerce.invoices().get_overdue().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get overdue invoices: {}", e))
        })?;

        convert_outputs(invoices)
    }

    /// Count invoices.
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::BomComponent> for BomComponent {
    type Error = PyErr;

    fn try_from(c: stateset_core::BomComponent) -> PyResult<Self> {
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let prod_uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let bom = commerce
            .bom()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get BOM: {}", e)))?;

        Ok(bom.map(|b| b.into()))
    }

    /// List all BOMs.
    fn list(&self) -> PyResult<Vec<Bom>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = bom_id.parse().map_err(|_| PyValueError::new_err("Invalid BOM UUID"))?;

        let component = commerce
            .bom()
            .add_component(
                uuid,
                stateset_core::CreateBomComponent {
                    component_sku,
                    name,
                    quantity: decimal_from_f64(quantity, "quantity")?,
                    unit_of_measure,
                    ..Default::default()
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to add component: {}", e)))?;

        convert_output(component)
    }

    /// Get components for a BOM.
    fn get_components(&self, bom_id: String) -> PyResult<Vec<BomComponent>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid = bom_id.parse().map_err(|_| PyValueError::new_err("Invalid BOM UUID"))?;

        let components = commerce
            .bom()
            .get_components(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get components: {}", e)))?;

        convert_outputs(components)
    }

    /// Activate a BOM.
    fn activate(&self, id: String) -> PyResult<Bom> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let bom = commerce
            .bom()
            .activate(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to activate BOM: {}", e)))?;

        Ok(bom.into())
    }

    /// Count BOMs.
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::WorkOrder> for WorkOrder {
    type Error = PyErr;

    fn try_from(wo: stateset_core::WorkOrder) -> PyResult<Self> {
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let prod_uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

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
                quantity_to_build: decimal_from_f64(quantity_to_build, "quantity_to_build")?,
                priority: prio,
                notes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create work order: {}", e)))?;

        convert_output(wo)
    }

    /// Get a work order by ID.
    fn get(&self, id: String) -> PyResult<Option<WorkOrder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get work order: {}", e)))?;

        convert_optional_output(wo)
    }

    /// List all work orders.
    fn list(&self) -> PyResult<Vec<WorkOrder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let wos = commerce
            .work_orders()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list work orders: {}", e)))?;

        convert_outputs(wos)
    }

    /// Start a work order.
    fn start(&self, id: String) -> PyResult<WorkOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .start(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to start work order: {}", e)))?;

        convert_output(wo)
    }

    /// Complete a work order.
    fn complete(&self, id: String, quantity_completed: f64) -> PyResult<WorkOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .complete(uuid, decimal_from_f64(quantity_completed, "quantity_completed")?)
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to complete work order: {}", e))
            })?;

        convert_output(wo)
    }

    /// Cancel a work order.
    fn cancel(&self, id: String) -> PyResult<WorkOrder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let wo = commerce
            .work_orders()
            .cancel(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel work order: {}", e)))?;

        convert_output(wo)
    }

    /// Count work orders.
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
#[pyclass(from_py_object)]
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
        format!("CartAddress(name='{} {}', city='{}')", self.first_name, self.last_name, self.city)
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
#[pyclass(skip_from_py_object)]
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
        format!("CartItem(sku='{}', qty={}, total={})", self.sku, self.quantity, self.total)
    }
}

impl TryFrom<stateset_core::CartItem> for CartItem {
    type Error = PyErr;

    fn try_from(i: stateset_core::CartItem) -> PyResult<Self> {
        Ok(Self {
            id: i.id.to_string(),
            cart_id: i.cart_id.to_string(),
            product_id: i.product_id.map(|id| id.to_string()),
            variant_id: i.variant_id.map(|id| id.to_string()),
            sku: i.sku,
            name: i.name,
            description: i.description,
            image_url: i.image_url,
            quantity: i.quantity,
            unit_price: to_f64_result(i.unit_price, "cart item unit price")?,
            original_price: optional_to_f64_result(i.original_price, "cart item original price")?,
            discount_amount: to_f64_result(i.discount_amount, "cart item discount amount")?,
            tax_amount: to_f64_result(i.tax_amount, "cart item tax amount")?,
            total: to_f64_result(i.total, "cart item total")?,
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        })
    }
}

/// Shipping rate option.
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::ShippingRate> for ShippingRate {
    type Error = PyErr;

    fn try_from(r: stateset_core::ShippingRate) -> PyResult<Self> {
        Ok(Self {
            id: r.id,
            carrier: r.carrier,
            service: r.service,
            description: r.description,
            price: to_f64_result(r.price, "shipping rate price")?,
            currency: r.currency.to_string(),
            estimated_days: r.estimated_days,
            estimated_delivery: r.estimated_delivery.map(|d| d.to_rfc3339()),
        })
    }
}

/// Checkout result returned when completing a cart.
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::CheckoutResult> for CheckoutResult {
    type Error = PyErr;

    fn try_from(r: stateset_core::CheckoutResult) -> PyResult<Self> {
        Ok(Self {
            order_id: r.order_id.to_string(),
            order_number: r.order_number,
            cart_id: r.cart_id.to_string(),
            payment_id: r.payment_id.map(|id| id.to_string()),
            total_charged: to_f64_result(r.total_charged, "checkout total charged")?,
            currency: r.currency.to_string(),
        })
    }
}

/// Cart data returned from operations.
#[pyclass(skip_from_py_object)]
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

impl TryFrom<stateset_core::Cart> for Cart {
    type Error = PyErr;

    fn try_from(c: stateset_core::Cart) -> PyResult<Self> {
        let item_count = c.items.len() as i32;
        let items = convert_outputs(c.items)?;
        Ok(Self {
            id: c.id.to_string(),
            cart_number: c.cart_number,
            customer_id: c.customer_id.map(|id| id.to_string()),
            status: format!("{}", c.status),
            currency: c.currency.to_string(),
            subtotal: to_f64_result(c.subtotal, "cart subtotal")?,
            tax_amount: to_f64_result(c.tax_amount, "cart tax amount")?,
            shipping_amount: to_f64_result(c.shipping_amount, "cart shipping amount")?,
            discount_amount: to_f64_result(c.discount_amount, "cart discount amount")?,
            grand_total: to_f64_result(c.grand_total, "cart grand total")?,
            customer_email: c.customer_email,
            customer_name: c.customer_name,
            payment_method: c.payment_method,
            payment_status: format!("{}", c.payment_status),
            fulfillment_type: c
                .fulfillment_type
                .map(|ft| format!("{}", ft))
                .unwrap_or_else(|| "Shipping".to_string()),
            shipping_method: c.shipping_method,
            coupon_code: c.coupon_code,
            notes: c.notes,
            item_count,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
            expires_at: c.expires_at.map(|d| d.to_rfc3339()),
            _items: items,
            _shipping_address: c.shipping_address.map(|a| a.into()),
            _billing_address: c.billing_address.map(|a| a.into()),
        })
    }
}

/// Input for adding a cart item.
#[pyclass(from_py_object)]
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
        let commerce = self
            .commerce
            .lock()
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
                currency: currency.as_ref().and_then(|s| s.parse::<CurrencyCode>().ok()),
                expires_in_minutes,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create cart: {}", e)))?;

        convert_output(cart)
    }

    /// Get a cart by ID.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart or None if not found
    fn get(&self, id: String) -> PyResult<Option<Cart>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get cart: {}", e)))?;

        convert_optional_output(cart)
    }

    /// Get a cart by cart number.
    ///
    /// Args:
    ///     cart_number: Cart number string
    ///
    /// Returns:
    ///     Cart or None if not found
    fn get_by_number(&self, cart_number: String) -> PyResult<Option<Cart>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cart = commerce
            .carts()
            .get_by_number(&cart_number)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get cart: {}", e)))?;

        convert_optional_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .update(
                uuid.into(),
                stateset_core::UpdateCart {
                    customer_email,
                    customer_phone,
                    customer_name,
                    shipping_method,
                    coupon_code,
                    notes,
                    ..Default::default()
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update cart: {}", e)))?;

        convert_output(cart)
    }

    /// List all carts.
    ///
    /// Returns:
    ///     List[Cart]: All carts
    fn list(&self) -> PyResult<Vec<Cart>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let carts = commerce
            .carts()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// Get all carts for a customer.
    ///
    /// Args:
    ///     customer_id: Customer UUID
    ///
    /// Returns:
    ///     List[Cart]: Customer's carts
    fn for_customer(&self, customer_id: String) -> PyResult<Vec<Cart>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let carts = commerce
            .carts()
            .for_customer(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get customer carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// Delete a cart.
    ///
    /// Args:
    ///     id: Cart UUID
    fn delete(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .carts()
            .delete(uuid.into())
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let prod_uuid = item
            .product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid product UUID"))?;

        let var_uuid = item
            .variant_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid variant UUID"))?;

        let cart_item = commerce
            .carts()
            .add_item(
                uuid.into(),
                stateset_core::AddCartItem {
                    product_id: prod_uuid,
                    variant_id: var_uuid,
                    sku: item.sku,
                    name: item.name,
                    description: item.description,
                    image_url: item.image_url,
                    quantity: item.quantity,
                    unit_price: decimal_from_f64(item.unit_price, "unit_price")?,
                    original_price: optional_decimal_from_f64(
                        item.original_price,
                        "original_price",
                    )?,
                    weight: optional_decimal_from_f64(item.weight, "weight")?,
                    requires_shipping: item.requires_shipping,
                    metadata: None,
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to add item: {}", e)))?;

        convert_output(cart_item)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            item_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart_item = commerce
            .carts()
            .update_item(uuid, stateset_core::UpdateCartItem { quantity, ..Default::default() })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update item: {}", e)))?;

        convert_output(cart_item)
    }

    /// Remove an item from the cart.
    ///
    /// Args:
    ///     item_id: Cart item UUID
    fn remove_item(&self, item_id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            item_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let items = commerce
            .carts()
            .get_items(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get items: {}", e)))?;

        convert_outputs(items)
    }

    /// Clear all items from a cart.
    ///
    /// Args:
    ///     cart_id: Cart UUID
    fn clear_items(&self, cart_id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            cart_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .carts()
            .clear_items(uuid.into())
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart =
            commerce.carts().set_shipping_address(uuid.into(), (&address).into()).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to set shipping address: {}", e))
            })?;

        convert_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart =
            commerce.carts().set_billing_address(uuid.into(), (&address).into()).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to set billing address: {}", e))
            })?;

        convert_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let amount_dec = match shipping_amount {
            Some(v) => Some(
                Decimal::from_f64_retain(v)
                    .ok_or_else(|| PyValueError::new_err("Invalid shipping amount"))?,
            ),
            None => None,
        };

        let cart = commerce
            .carts()
            .set_shipping(
                uuid.into(),
                stateset_core::SetCartShipping {
                    shipping_address: (&address).into(),
                    shipping_method,
                    shipping_carrier,
                    shipping_amount: amount_dec,
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set shipping: {}", e)))?;

        convert_output(cart)
    }

    /// Get available shipping rates for the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     List[ShippingRate]: Available shipping options
    fn get_shipping_rates(&self, id: String) -> PyResult<Vec<ShippingRate>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let rates = commerce
            .carts()
            .get_shipping_rates(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get shipping rates: {}", e)))?;

        convert_outputs(rates)
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
    fn set_payment(
        &self,
        id: String,
        payment_method: String,
        payment_token: Option<String>,
    ) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_payment(
                uuid.into(),
                stateset_core::SetCartPayment {
                    payment_method,
                    payment_token,
                    ..Default::default()
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set payment: {}", e)))?;

        convert_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .apply_discount(uuid.into(), &coupon_code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to apply discount: {}", e)))?;

        convert_output(cart)
    }

    /// Remove the discount from the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn remove_discount(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .remove_discount(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to remove discount: {}", e)))?;

        convert_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .mark_ready_for_payment(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to mark ready: {}", e)))?;

        convert_output(cart)
    }

    /// Begin the checkout process.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn begin_checkout(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .begin_checkout(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to begin checkout: {}", e)))?;

        convert_output(cart)
    }

    /// Complete the checkout and create an order.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     CheckoutResult: Order creation result
    fn complete(&self, id: String) -> PyResult<CheckoutResult> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let result = commerce
            .carts()
            .complete(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to complete checkout: {}", e)))?;

        convert_output(result)
    }

    /// Cancel the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn cancel(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .cancel(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel cart: {}", e)))?;

        convert_output(cart)
    }

    /// Mark the cart as abandoned.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn abandon(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .abandon(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to abandon cart: {}", e)))?;

        convert_output(cart)
    }

    /// Expire the cart.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn expire(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .expire(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to expire cart: {}", e)))?;

        convert_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .reserve_inventory(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to reserve inventory: {}", e)))?;

        convert_output(cart)
    }

    /// Release inventory reservations.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn release_inventory(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .release_inventory(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to release inventory: {}", e)))?;

        convert_output(cart)
    }

    /// Recalculate cart totals.
    ///
    /// Args:
    ///     id: Cart UUID
    ///
    /// Returns:
    ///     Cart: Updated cart
    fn recalculate(&self, id: String) -> PyResult<Cart> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .recalculate(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to recalculate: {}", e)))?;

        convert_output(cart)
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cart = commerce
            .carts()
            .set_tax(uuid.into(), decimal_from_f64(tax_amount, "tax_amount")?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set tax: {}", e)))?;

        convert_output(cart)
    }

    // === Query Operations ===

    /// Get abandoned carts.
    ///
    /// Returns:
    ///     List[Cart]: Abandoned carts
    fn get_abandoned(&self) -> PyResult<Vec<Cart>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let carts = commerce.carts().get_abandoned().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get abandoned carts: {}", e))
        })?;

        convert_outputs(carts)
    }

    /// Get expired carts.
    ///
    /// Returns:
    ///     List[Cart]: Expired carts
    fn get_expired(&self) -> PyResult<Vec<Cart>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let carts = commerce
            .carts()
            .get_expired()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get expired carts: {}", e)))?;

        convert_outputs(carts)
    }

    /// Count carts.
    ///
    /// Returns:
    ///     int: Number of carts
    fn count(&self) -> PyResult<u32> {
        let commerce = self
            .commerce
            .lock()
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
    to_f64_or_nan(*d)
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
        _ => "unknown".to_string(),
    }
}

/// Demand forecast for a SKU.
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
    fn top_products(
        &self,
        period: Option<String>,
        limit: Option<u32>,
    ) -> PyResult<Vec<TopProduct>> {
        let commerce = self
            .commerce
            .lock()
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
    fn product_performance(
        &self,
        period: Option<String>,
        limit: Option<u32>,
    ) -> PyResult<Vec<ProductPerformance>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, limit);
        let rows = commerce.analytics().product_performance(q).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get product performance: {}", e))
        })?;

        Ok(rows.into_iter().map(|p| p.into()).collect())
    }

    /// Get customer metrics.
    #[pyo3(signature = (period=None))]
    fn customer_metrics(&self, period: Option<String>) -> PyResult<CustomerMetrics> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let metrics = commerce.analytics().customer_metrics(q).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get customer metrics: {}", e))
        })?;

        Ok(metrics.into())
    }

    /// Get top customers by spend.
    #[pyo3(signature = (period=None, limit=None))]
    fn top_customers(
        &self,
        period: Option<String>,
        limit: Option<u32>,
    ) -> PyResult<Vec<TopCustomer>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let health = commerce.analytics().inventory_health().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get inventory health: {}", e))
        })?;

        Ok(health.into())
    }

    /// Get low stock items.
    #[pyo3(signature = (threshold=None))]
    fn low_stock_items(&self, threshold: Option<f64>) -> PyResult<Vec<LowStockItem>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let threshold_dec = match threshold {
            Some(v) => Some(
                Decimal::from_f64_retain(v)
                    .ok_or_else(|| PyValueError::new_err("Invalid threshold"))?,
            ),
            None => None,
        };

        let rows = commerce.analytics().low_stock_items(threshold_dec).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get low stock items: {}", e))
        })?;

        Ok(rows.into_iter().map(|i| i.into()).collect())
    }

    /// Get inventory movement summary.
    #[pyo3(signature = (period=None))]
    fn inventory_movement(&self, period: Option<String>) -> PyResult<Vec<InventoryMovement>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let rows = commerce.analytics().inventory_movement(q).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get inventory movement: {}", e))
        })?;

        Ok(rows.into_iter().map(|m| m.into()).collect())
    }

    /// Get order status breakdown.
    #[pyo3(signature = (period=None))]
    fn order_status_breakdown(&self, period: Option<String>) -> PyResult<OrderStatusBreakdown> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let breakdown = commerce.analytics().order_status_breakdown(q).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get order status breakdown: {}", e))
        })?;

        Ok(breakdown.into())
    }

    /// Get fulfillment metrics.
    #[pyo3(signature = (period=None))]
    fn fulfillment_metrics(&self, period: Option<String>) -> PyResult<FulfillmentMetrics> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let q = build_analytics_query(period, None, None);
        let metrics = commerce.analytics().fulfillment_metrics(q).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get fulfillment metrics: {}", e))
        })?;

        Ok(metrics.into())
    }

    /// Get return metrics.
    #[pyo3(signature = (period=None))]
    fn return_metrics(&self, period: Option<String>) -> PyResult<ReturnMetrics> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let forecasts =
            commerce.analytics().demand_forecast(skus, days_ahead.unwrap_or(30)).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to get demand forecast: {}", e))
            })?;

        Ok(forecasts.into_iter().map(|f| f.into()).collect())
    }

    /// Get revenue forecast.
    #[pyo3(signature = (periods_ahead=None, granularity=None))]
    fn revenue_forecast(
        &self,
        periods_ahead: Option<u32>,
        granularity: Option<String>,
    ) -> PyResult<Vec<RevenueForecast>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let gran = granularity
            .as_deref()
            .map(parse_time_granularity)
            .unwrap_or(stateset_core::TimeGranularity::Month);

        let forecasts =
            commerce.analytics().revenue_forecast(periods_ahead.unwrap_or(3), gran).map_err(
                |e| PyRuntimeError::new_err(format!("Failed to get revenue forecast: {}", e)),
            )?;

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
        _ => "half_up".to_string(),
    }
}

/// Exchange rate between currencies.
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(skip_from_py_object)]
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
#[pyclass(from_py_object)]
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
        Self { base_currency, quote_currency, rate, source }
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
    fn get_rate(
        &self,
        from_currency: String,
        to_currency: String,
    ) -> PyResult<Option<ExchangeRate>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let rate = commerce
            .currency()
            .get_rate(parse_currency(&from_currency)?, parse_currency(&to_currency)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get rate: {}", e)))?;

        Ok(rate.map(|r| r.into()))
    }

    /// Get all exchange rates for a base currency.
    fn get_rates_for(&self, base_currency: String) -> PyResult<Vec<ExchangeRate>> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .currency()
            .delete_rate(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete rate: {}", e)))?;

        Ok(())
    }

    /// Convert an amount from one currency to another.
    fn convert(
        &self,
        from_currency: String,
        to_currency: String,
        amount: f64,
    ) -> PyResult<ConversionResult> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
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
                rounding_mode: rounding_mode
                    .as_deref()
                    .map(parse_rounding_mode)
                    .unwrap_or_default(),
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Set the store's base currency.
    fn set_base_currency(&self, currency_code: String) -> PyResult<StoreCurrencySettings> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let settings = commerce
            .currency()
            .set_base_currency(parse_currency(&currency_code)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to set base currency: {}", e)))?;

        Ok(settings.into())
    }

    /// Enable currencies for the store.
    fn enable_currencies(&self, currency_codes: Vec<String>) -> PyResult<StoreCurrencySettings> {
        let commerce = self
            .commerce
            .lock()
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
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        commerce
            .currency()
            .is_enabled(parse_currency(&currency_code)?)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to check currency: {}", e)))
    }

    /// Get the store's base currency code.
    fn base_currency(&self) -> PyResult<String> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let currency = commerce
            .currency()
            .base_currency()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get base currency: {}", e)))?;

        Ok(currency.code().to_string())
    }

    /// Get enabled currency codes.
    fn enabled_currencies(&self) -> PyResult<Vec<String>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let currencies = commerce.currency().enabled_currencies().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get enabled currencies: {}", e))
        })?;

        Ok(currencies.iter().map(|c| c.code().to_string()).collect())
    }

    /// Format an amount with currency symbol.
    fn format(&self, amount: f64, currency_code: String) -> PyResult<String> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let amount_dec = Decimal::from_f64_retain(amount)
            .ok_or_else(|| PyValueError::new_err("Invalid amount"))?;

        Ok(commerce.currency().format(amount_dec, parse_currency(&currency_code)?))
    }
}

// ============================================================================
// Subscription Types
// ============================================================================

/// Subscription plan data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SubscriptionPlan {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    billing_interval: String,
    #[pyo3(get)]
    billing_interval_count: i32,
    #[pyo3(get)]
    price: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    setup_fee: f64,
    #[pyo3(get)]
    trial_days: i32,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl TryFrom<stateset_core::SubscriptionPlan> for SubscriptionPlan {
    type Error = PyErr;

    fn try_from(p: stateset_core::SubscriptionPlan) -> PyResult<Self> {
        Ok(Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            billing_interval: format!("{:?}", p.billing_interval).to_lowercase(),
            billing_interval_count: 1, // Default to 1 since core doesn't have this field
            price: to_f64_result(p.price, "subscription plan price")?,
            currency: p.currency.to_string(),
            setup_fee: optional_to_f64_result(p.setup_fee, "subscription plan setup fee")?
                .unwrap_or(0.0),
            trial_days: p.trial_days,
            status: format!("{:?}", p.status).to_lowercase(),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

/// Subscription data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Subscription {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    subscription_number: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    plan_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    current_period_start: String,
    #[pyo3(get)]
    current_period_end: String,
    #[pyo3(get)]
    trial_ends_at: Option<String>,
    #[pyo3(get)]
    cancelled_at: Option<String>,
    #[pyo3(get)]
    ends_at: Option<String>,
    #[pyo3(get)]
    price: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl TryFrom<stateset_core::Subscription> for Subscription {
    type Error = PyErr;

    fn try_from(s: stateset_core::Subscription) -> PyResult<Self> {
        Ok(Self {
            id: s.id.to_string(),
            subscription_number: s.subscription_number,
            customer_id: s.customer_id.to_string(),
            plan_id: s.plan_id.to_string(),
            status: format!("{:?}", s.status).to_lowercase(),
            current_period_start: s.current_period_start.to_rfc3339(),
            current_period_end: s.current_period_end.to_rfc3339(),
            trial_ends_at: s.trial_ends_at.map(|d| d.to_rfc3339()),
            cancelled_at: s.cancelled_at.map(|d| d.to_rfc3339()),
            ends_at: s.ends_at.map(|d| d.to_rfc3339()),
            price: to_f64_result(s.price, "subscription price")?,
            currency: s.currency.to_string(),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        })
    }
}

/// Billing cycle data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct BillingCycle {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    cycle_number: i32,
    #[pyo3(get)]
    subscription_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    period_start: String,
    #[pyo3(get)]
    period_end: String,
    #[pyo3(get)]
    total: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    payment_id: Option<String>,
    #[pyo3(get)]
    invoice_id: Option<String>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl TryFrom<stateset_core::BillingCycle> for BillingCycle {
    type Error = PyErr;

    fn try_from(c: stateset_core::BillingCycle) -> PyResult<Self> {
        Ok(Self {
            id: c.id.to_string(),
            cycle_number: c.cycle_number,
            subscription_id: c.subscription_id.to_string(),
            status: format!("{:?}", c.status).to_lowercase(),
            period_start: c.period_start.to_rfc3339(),
            period_end: c.period_end.to_rfc3339(),
            total: to_f64_result(c.total, "billing cycle total")?,
            currency: c.currency.to_string(),
            payment_id: c.payment_id,
            invoice_id: c.invoice_id.map(|id| id.to_string()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        })
    }
}

/// Subscription event data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SubscriptionEvent {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    subscription_id: String,
    #[pyo3(get)]
    event_type: String,
    #[pyo3(get)]
    description: String,
    #[pyo3(get)]
    created_at: String,
}

impl From<stateset_core::SubscriptionEvent> for SubscriptionEvent {
    fn from(e: stateset_core::SubscriptionEvent) -> Self {
        Self {
            id: e.id.to_string(),
            subscription_id: e.subscription_id.to_string(),
            event_type: format!("{:?}", e.event_type).to_lowercase(),
            description: e.description,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Subscriptions API
// ============================================================================

/// Subscriptions API for subscription management.
#[pyclass]
pub struct Subscriptions {
    commerce: Arc<Mutex<RustCommerce>>,
}

fn parse_billing_interval(s: &str) -> PyResult<stateset_core::BillingInterval> {
    match s.to_lowercase().as_str() {
        "weekly" => Ok(stateset_core::BillingInterval::Weekly),
        "biweekly" => Ok(stateset_core::BillingInterval::Biweekly),
        "monthly" => Ok(stateset_core::BillingInterval::Monthly),
        "quarterly" => Ok(stateset_core::BillingInterval::Quarterly),
        "annual" | "yearly" => Ok(stateset_core::BillingInterval::Annual),
        _ => Err(PyValueError::new_err(format!("Invalid billing interval: {}", s))),
    }
}

#[pymethods]
impl Subscriptions {
    // ========================================================================
    // Subscription Plans
    // ========================================================================

    /// Create a subscription plan.
    #[pyo3(signature = (code, name, price, billing_interval=None, billing_interval_count=None, description=None, currency=None, setup_fee=None, trial_days=None))]
    fn create_plan(
        &self,
        code: String,
        name: String,
        price: f64,
        billing_interval: Option<String>,
        billing_interval_count: Option<i32>,
        description: Option<String>,
        currency: Option<String>,
        setup_fee: Option<f64>,
        trial_days: Option<i32>,
    ) -> PyResult<SubscriptionPlan> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let interval = billing_interval
            .as_deref()
            .map(parse_billing_interval)
            .transpose()?
            .unwrap_or(stateset_core::BillingInterval::Monthly);

        let plan = commerce
            .subscriptions()
            .create_plan(stateset_core::CreateSubscriptionPlan {
                code: Some(code),
                name,
                description,
                billing_interval: interval,
                custom_interval_days: billing_interval_count,
                price: decimal_from_f64(price, "price")?,
                currency: Some(
                    currency
                        .unwrap_or_else(|| "USD".to_string())
                        .parse::<CurrencyCode>()
                        .unwrap_or(CurrencyCode::USD),
                ),
                setup_fee: optional_decimal_from_f64(setup_fee, "setup_fee")?,
                trial_days,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create plan: {}", e)))?;

        convert_output(plan)
    }

    /// Get a subscription plan by ID.
    fn get_plan(&self, id: String) -> PyResult<Option<SubscriptionPlan>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let plan = commerce
            .subscriptions()
            .get_plan(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get plan: {}", e)))?;

        convert_optional_output(plan)
    }

    /// List all subscription plans.
    #[pyo3(signature = (status=None, billing_interval=None, limit=None, offset=None))]
    fn list_plans(
        &self,
        status: Option<String>,
        billing_interval: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<SubscriptionPlan>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let interval = billing_interval.as_deref().map(parse_billing_interval).transpose()?;

        let plan_status = status.as_deref().map(|s| match s.to_lowercase().as_str() {
            "draft" => stateset_core::PlanStatus::Draft,
            "active" => stateset_core::PlanStatus::Active,
            "archived" => stateset_core::PlanStatus::Archived,
            _ => stateset_core::PlanStatus::Draft,
        });

        let plans = commerce
            .subscriptions()
            .list_plans(stateset_core::SubscriptionPlanFilter {
                status: plan_status,
                billing_interval: interval,
                search: None,
                limit,
                offset,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list plans: {}", e)))?;

        convert_outputs(plans)
    }

    /// Activate a subscription plan.
    fn activate_plan(&self, id: String) -> PyResult<SubscriptionPlan> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let plan = commerce
            .subscriptions()
            .activate_plan(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to activate plan: {}", e)))?;

        convert_output(plan)
    }

    /// Archive a subscription plan.
    fn archive_plan(&self, id: String) -> PyResult<SubscriptionPlan> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let plan = commerce
            .subscriptions()
            .archive_plan(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to archive plan: {}", e)))?;

        convert_output(plan)
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    /// Subscribe a customer to a plan.
    #[pyo3(signature = (customer_id, plan_id, skip_trial=None, price=None))]
    fn subscribe(
        &self,
        customer_id: String,
        plan_id: String,
        skip_trial: Option<bool>,
        price: Option<f64>,
    ) -> PyResult<Subscription> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let plan_uuid = plan_id.parse().map_err(|_| PyValueError::new_err("Invalid plan UUID"))?;

        let subscription = commerce
            .subscriptions()
            .subscribe(stateset_core::CreateSubscription {
                customer_id: cust_uuid,
                plan_id: plan_uuid,
                skip_trial,
                price: optional_decimal_from_f64(price, "price")?,
                ..Default::default()
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create subscription: {}", e))
            })?;

        convert_output(subscription)
    }

    /// Get a subscription by ID.
    fn get(&self, id: String) -> PyResult<Option<Subscription>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let subscription = commerce
            .subscriptions()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get subscription: {}", e)))?;

        convert_optional_output(subscription)
    }

    /// Get a subscription by number.
    fn get_by_number(&self, number: String) -> PyResult<Option<Subscription>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let subscription = commerce
            .subscriptions()
            .get_by_number(&number)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get subscription: {}", e)))?;

        convert_optional_output(subscription)
    }

    /// List subscriptions.
    #[pyo3(signature = (customer_id=None, plan_id=None, status=None, limit=None, offset=None))]
    fn list(
        &self,
        customer_id: Option<String>,
        plan_id: Option<String>,
        status: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<Subscription>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cust_uuid = customer_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let p_uuid = plan_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid plan UUID"))?;

        let sub_status = status.as_deref().map(|s| match s.to_lowercase().as_str() {
            "pending" => stateset_core::SubscriptionStatus::Pending,
            "trial" | "trialing" => stateset_core::SubscriptionStatus::Trial,
            "active" => stateset_core::SubscriptionStatus::Active,
            "paused" => stateset_core::SubscriptionStatus::Paused,
            "past_due" | "pastdue" => stateset_core::SubscriptionStatus::PastDue,
            "cancelled" | "canceled" => stateset_core::SubscriptionStatus::Cancelled,
            "expired" => stateset_core::SubscriptionStatus::Expired,
            _ => stateset_core::SubscriptionStatus::Active,
        });

        let subscriptions = commerce
            .subscriptions()
            .list(stateset_core::SubscriptionFilter {
                customer_id: cust_uuid,
                plan_id: p_uuid,
                status: sub_status,
                from_date: None,
                to_date: None,
                search: None,
                limit,
                offset,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list subscriptions: {}", e)))?;

        convert_outputs(subscriptions)
    }

    /// Pause a subscription.
    #[pyo3(signature = (id, reason=None))]
    fn pause(&self, id: String, reason: Option<String>) -> PyResult<Subscription> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let subscription = commerce
            .subscriptions()
            .pause(uuid.into(), stateset_core::PauseSubscription { reason, resume_at: None })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to pause subscription: {}", e)))?;

        convert_output(subscription)
    }

    /// Resume a paused subscription.
    fn resume(&self, id: String) -> PyResult<Subscription> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let subscription = commerce.subscriptions().resume(uuid.into()).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to resume subscription: {}", e))
        })?;

        convert_output(subscription)
    }

    /// Cancel a subscription.
    #[pyo3(signature = (id, immediate=None, reason=None))]
    fn cancel(
        &self,
        id: String,
        immediate: Option<bool>,
        reason: Option<String>,
    ) -> PyResult<Subscription> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let subscription = commerce
            .subscriptions()
            .cancel(
                uuid.into(),
                stateset_core::CancelSubscription {
                    immediate,
                    reason: reason.clone(),
                    feedback: None,
                },
            )
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to cancel subscription: {}", e))
            })?;

        convert_output(subscription)
    }

    /// Skip the next billing cycle.
    #[pyo3(signature = (id, reason=None))]
    fn skip_next_cycle(&self, id: String, reason: Option<String>) -> PyResult<Subscription> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let subscription = commerce
            .subscriptions()
            .skip_next_cycle(uuid.into(), stateset_core::SkipBillingCycle { reason })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to skip billing cycle: {}", e)))?;

        convert_output(subscription)
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// List billing cycles.
    #[pyo3(signature = (subscription_id=None, status=None, limit=None, offset=None))]
    fn list_billing_cycles(
        &self,
        subscription_id: Option<String>,
        status: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<BillingCycle>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let sub_uuid = subscription_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| PyValueError::new_err("Invalid subscription UUID"))?;

        let cycle_status = status.as_deref().map(|s| match s.to_lowercase().as_str() {
            "scheduled" | "pending" => stateset_core::BillingCycleStatus::Scheduled,
            "processing" => stateset_core::BillingCycleStatus::Processing,
            "paid" => stateset_core::BillingCycleStatus::Paid,
            "failed" => stateset_core::BillingCycleStatus::Failed,
            "skipped" => stateset_core::BillingCycleStatus::Skipped,
            "refunded" => stateset_core::BillingCycleStatus::Refunded,
            "voided" => stateset_core::BillingCycleStatus::Voided,
            _ => stateset_core::BillingCycleStatus::Scheduled,
        });

        let cycles = commerce
            .subscriptions()
            .list_billing_cycles(stateset_core::BillingCycleFilter {
                subscription_id: sub_uuid,
                status: cycle_status,
                from_date: None,
                to_date: None,
                limit,
                offset,
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to list billing cycles: {}", e))
            })?;

        convert_outputs(cycles)
    }

    /// Get a billing cycle by ID.
    fn get_billing_cycle(&self, id: String) -> PyResult<Option<BillingCycle>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let cycle = commerce
            .subscriptions()
            .get_billing_cycle(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get billing cycle: {}", e)))?;

        convert_optional_output(cycle)
    }

    // ========================================================================
    // Events
    // ========================================================================

    /// Get events for a subscription.
    fn get_events(&self, subscription_id: String) -> PyResult<Vec<SubscriptionEvent>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid =
            subscription_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let events = commerce
            .subscriptions()
            .get_events(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get events: {}", e)))?;

        Ok(events.into_iter().map(|e| e.into()).collect())
    }
}

// ============================================================================
// Promotions Types
// ============================================================================

/// Promotion output
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Promotion {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    promotion_type: String,
    #[pyo3(get)]
    trigger: String,
    #[pyo3(get)]
    target: String,
    #[pyo3(get)]
    stacking: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    percentage_off: Option<f64>,
    #[pyo3(get)]
    fixed_amount_off: Option<f64>,
    #[pyo3(get)]
    max_discount_amount: Option<f64>,
    #[pyo3(get)]
    buy_quantity: Option<i32>,
    #[pyo3(get)]
    get_quantity: Option<i32>,
    #[pyo3(get)]
    starts_at: String,
    #[pyo3(get)]
    ends_at: Option<String>,
    #[pyo3(get)]
    total_usage_limit: Option<i32>,
    #[pyo3(get)]
    per_customer_limit: Option<i32>,
    #[pyo3(get)]
    usage_count: i32,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    priority: i32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl TryFrom<stateset_core::Promotion> for Promotion {
    type Error = PyErr;

    fn try_from(p: stateset_core::Promotion) -> PyResult<Self> {
        Ok(Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
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
            starts_at: p.starts_at.to_rfc3339(),
            ends_at: p.ends_at.map(|d| d.to_rfc3339()),
            total_usage_limit: p.total_usage_limit,
            per_customer_limit: p.per_customer_limit,
            usage_count: p.usage_count,
            currency: p.currency.to_string(),
            priority: p.priority,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

/// Coupon code output
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Coupon {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    promotion_id: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    usage_limit: Option<i32>,
    #[pyo3(get)]
    per_customer_limit: Option<i32>,
    #[pyo3(get)]
    usage_count: i32,
    #[pyo3(get)]
    starts_at: Option<String>,
    #[pyo3(get)]
    ends_at: Option<String>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::CouponCode> for Coupon {
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
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

/// Result of applying promotions
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ApplyPromotionsResult {
    #[pyo3(get)]
    original_subtotal: f64,
    #[pyo3(get)]
    total_discount: f64,
    #[pyo3(get)]
    discounted_subtotal: f64,
    #[pyo3(get)]
    original_shipping: f64,
    #[pyo3(get)]
    shipping_discount: f64,
    #[pyo3(get)]
    final_shipping: f64,
    #[pyo3(get)]
    grand_total: f64,
    #[pyo3(get)]
    applied_promotions: Vec<AppliedPromotion>,
}

impl TryFrom<stateset_core::ApplyPromotionsResult> for ApplyPromotionsResult {
    type Error = PyErr;

    fn try_from(r: stateset_core::ApplyPromotionsResult) -> PyResult<Self> {
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

/// An applied promotion
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct AppliedPromotion {
    #[pyo3(get)]
    promotion_id: String,
    #[pyo3(get)]
    promotion_name: String,
    #[pyo3(get)]
    coupon_code: Option<String>,
    #[pyo3(get)]
    discount_amount: f64,
    #[pyo3(get)]
    discount_type: String,
}

impl TryFrom<stateset_core::AppliedPromotion> for AppliedPromotion {
    type Error = PyErr;

    fn try_from(a: stateset_core::AppliedPromotion) -> PyResult<Self> {
        Ok(Self {
            promotion_id: a.promotion_id.to_string(),
            promotion_name: a.promotion_name,
            coupon_code: a.coupon_code,
            discount_amount: to_f64_result(a.discount_amount, "applied promotion discount amount")?,
            discount_type: format!("{:?}", a.discount_type).to_lowercase(),
        })
    }
}

/// Promotion usage record
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct PromotionUsage {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    promotion_id: String,
    #[pyo3(get)]
    coupon_id: Option<String>,
    #[pyo3(get)]
    customer_id: Option<String>,
    #[pyo3(get)]
    order_id: Option<String>,
    #[pyo3(get)]
    cart_id: Option<String>,
    #[pyo3(get)]
    discount_amount: f64,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    used_at: String,
}

impl TryFrom<stateset_core::PromotionUsage> for PromotionUsage {
    type Error = PyErr;

    fn try_from(u: stateset_core::PromotionUsage) -> PyResult<Self> {
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

// ============================================================================
// Promotions API
// ============================================================================

fn parse_promotion_type(s: &str) -> stateset_core::PromotionType {
    match s.to_lowercase().as_str() {
        "percentage_off" => stateset_core::PromotionType::PercentageOff,
        "fixed_amount_off" => stateset_core::PromotionType::FixedAmountOff,
        "buy_x_get_y" | "bogo" => stateset_core::PromotionType::BuyXGetY,
        "free_shipping" => stateset_core::PromotionType::FreeShipping,
        "tiered_discount" => stateset_core::PromotionType::TieredDiscount,
        "bundle_discount" => stateset_core::PromotionType::BundleDiscount,
        _ => stateset_core::PromotionType::PercentageOff,
    }
}

fn parse_promotion_trigger(s: &str) -> stateset_core::PromotionTrigger {
    match s.to_lowercase().as_str() {
        "automatic" => stateset_core::PromotionTrigger::Automatic,
        "coupon_code" => stateset_core::PromotionTrigger::CouponCode,
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
        "line_item" => stateset_core::PromotionTarget::LineItem,
        _ => stateset_core::PromotionTarget::Order,
    }
}

fn parse_stacking_behavior(s: &str) -> stateset_core::StackingBehavior {
    match s.to_lowercase().as_str() {
        "stackable" => stateset_core::StackingBehavior::Stackable,
        "exclusive" => stateset_core::StackingBehavior::Exclusive,
        "selective_stack" => stateset_core::StackingBehavior::SelectiveStack,
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

/// Promotions API for managing discounts and coupon codes
#[pyclass]
pub struct PromotionsApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl PromotionsApi {
    /// Create a new promotion.
    #[pyo3(signature = (name, promotion_type=None, trigger=None, target=None, stacking=None,
                        percentage_off=None, fixed_amount_off=None, max_discount_amount=None,
                        buy_quantity=None, get_quantity=None, starts_at=None, ends_at=None,
                        total_usage_limit=None, per_customer_limit=None, currency=None, priority=None))]
    fn create(
        &self,
        name: String,
        promotion_type: Option<String>,
        trigger: Option<String>,
        target: Option<String>,
        stacking: Option<String>,
        percentage_off: Option<f64>,
        fixed_amount_off: Option<f64>,
        max_discount_amount: Option<f64>,
        buy_quantity: Option<i32>,
        get_quantity: Option<i32>,
        starts_at: Option<String>,
        ends_at: Option<String>,
        total_usage_limit: Option<i32>,
        per_customer_limit: Option<i32>,
        currency: Option<String>,
        priority: Option<i32>,
    ) -> PyResult<Promotion> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let percentage_off = optional_decimal_from_f64(percentage_off, "percentage_off")?;
        let fixed_amount_off = optional_decimal_from_f64(fixed_amount_off, "fixed_amount_off")?;
        let max_discount_amount =
            optional_decimal_from_f64(max_discount_amount, "max_discount_amount")?;

        let create = stateset_core::CreatePromotion {
            code: None,
            name,
            description: None,
            internal_notes: None,
            promotion_type: promotion_type.map(|s| parse_promotion_type(&s)).unwrap_or_default(),
            trigger: trigger.map(|s| parse_promotion_trigger(&s)).unwrap_or_default(),
            target: target.map(|s| parse_promotion_target(&s)).unwrap_or_default(),
            stacking: stacking.map(|s| parse_stacking_behavior(&s)).unwrap_or_default(),
            percentage_off,
            fixed_amount_off,
            max_discount_amount,
            buy_quantity,
            get_quantity,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: starts_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            ends_at: ends_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            total_usage_limit,
            per_customer_limit,
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: currency.as_ref().and_then(|s| s.parse::<CurrencyCode>().ok()),
            priority,
            metadata: None,
        };

        let promo = commerce
            .promotions()
            .create(create)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create promotion: {}", e)))?;

        convert_output(promo)
    }

    /// Get a promotion by ID.
    fn get(&self, id: String) -> PyResult<Option<Promotion>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let promo = commerce
            .promotions()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get promotion: {}", e)))?;

        convert_optional_output(promo)
    }

    /// Get a promotion by its internal code.
    fn get_by_code(&self, code: String) -> PyResult<Option<Promotion>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let promo = commerce
            .promotions()
            .get_by_code(&code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get promotion: {}", e)))?;

        convert_optional_output(promo)
    }

    /// List promotions with optional filtering.
    #[pyo3(signature = (status=None, promotion_type=None, is_active=None, limit=None, offset=None))]
    fn list(
        &self,
        status: Option<String>,
        promotion_type: Option<String>,
        is_active: Option<bool>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> PyResult<Vec<Promotion>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let filter = stateset_core::PromotionFilter {
            status: status.map(|s| parse_promotion_status(&s)),
            promotion_type: promotion_type.map(|s| parse_promotion_type(&s)),
            trigger: None,
            is_active,
            search: None,
            limit: limit.map(|v| v as u32),
            offset: offset.map(|v| v as u32),
        };

        let promos = commerce
            .promotions()
            .list(filter)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list promotions: {}", e)))?;

        convert_outputs(promos)
    }

    /// Activate a promotion.
    fn activate(&self, id: String) -> PyResult<Promotion> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let promo = commerce
            .promotions()
            .activate(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to activate promotion: {}", e)))?;

        convert_output(promo)
    }

    /// Deactivate (pause) a promotion.
    fn deactivate(&self, id: String) -> PyResult<Promotion> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let promo = commerce.promotions().deactivate(uuid.into()).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to deactivate promotion: {}", e))
        })?;

        convert_output(promo)
    }

    /// Delete a promotion.
    fn delete(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        commerce
            .promotions()
            .delete(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete promotion: {}", e)))?;

        Ok(())
    }

    /// Get all active promotions.
    fn get_active(&self) -> PyResult<Vec<Promotion>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let promos = commerce.promotions().get_active().map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get active promotions: {}", e))
        })?;

        convert_outputs(promos)
    }

    // ========================================================================
    // Coupon Codes
    // ========================================================================

    /// Create a coupon code for a promotion.
    #[pyo3(signature = (promotion_id, code, usage_limit=None, per_customer_limit=None, starts_at=None, ends_at=None))]
    fn create_coupon(
        &self,
        promotion_id: String,
        code: String,
        usage_limit: Option<i32>,
        per_customer_limit: Option<i32>,
        starts_at: Option<String>,
        ends_at: Option<String>,
    ) -> PyResult<Coupon> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let promo_uuid =
            promotion_id.parse().map_err(|_| PyValueError::new_err("Invalid promotion UUID"))?;

        let create = stateset_core::CreateCouponCode {
            promotion_id: promo_uuid,
            code,
            usage_limit,
            per_customer_limit,
            starts_at: starts_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            ends_at: ends_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))
            }),
            metadata: None,
        };

        let coupon = commerce
            .promotions()
            .create_coupon(create)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create coupon: {}", e)))?;

        Ok(coupon.into())
    }

    /// Get a coupon by ID.
    fn get_coupon(&self, id: String) -> PyResult<Option<Coupon>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let coupon = commerce
            .promotions()
            .get_coupon(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// Get a coupon by its code.
    fn get_coupon_by_code(&self, code: String) -> PyResult<Option<Coupon>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let coupon = commerce
            .promotions()
            .get_coupon_by_code(&code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    /// List coupons with optional filtering.
    #[pyo3(signature = (promotion_id=None, status=None, limit=None, offset=None))]
    fn list_coupons(
        &self,
        promotion_id: Option<String>,
        status: Option<String>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> PyResult<Vec<Coupon>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let filter = stateset_core::CouponFilter {
            promotion_id: promotion_id.and_then(|s| s.parse().ok()),
            status: status.map(|s| parse_coupon_status(&s)),
            search: None,
            limit: limit.map(|v| v as u32),
            offset: offset.map(|v| v as u32),
        };

        let coupons = commerce
            .promotions()
            .list_coupons(filter)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list coupons: {}", e)))?;

        Ok(coupons.into_iter().map(|c| c.into()).collect())
    }

    /// Validate a coupon code.
    fn validate_coupon(&self, code: String) -> PyResult<Option<Coupon>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let coupon = commerce
            .promotions()
            .validate_coupon(&code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to validate coupon: {}", e)))?;

        Ok(coupon.map(|c| c.into()))
    }

    // ========================================================================
    // Apply Promotions
    // ========================================================================

    /// Apply promotions to cart/order items.
    #[pyo3(signature = (subtotal, coupon_codes=None, shipping_amount=None, currency=None))]
    fn apply(
        &self,
        subtotal: f64,
        coupon_codes: Option<Vec<String>>,
        shipping_amount: Option<f64>,
        currency: Option<String>,
    ) -> PyResult<ApplyPromotionsResult> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let shipping_amount = match shipping_amount {
            Some(amount) => decimal_from_f64(amount, "shipping_amount")?,
            None => Decimal::ZERO,
        };

        let request = stateset_core::ApplyPromotionsRequest {
            cart_id: None,
            customer_id: None,
            coupon_codes: coupon_codes.unwrap_or_default(),
            line_items: vec![],
            subtotal: decimal_from_f64(subtotal, "subtotal")?,
            shipping_amount,
            shipping_country: None,
            shipping_state: None,
            currency: currency
                .unwrap_or_else(|| "USD".to_string())
                .parse::<CurrencyCode>()
                .unwrap_or(CurrencyCode::USD),
            is_first_order: false,
        };

        let result = commerce
            .promotions()
            .apply(request)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to apply promotions: {}", e)))?;

        convert_output(result)
    }

    /// Record promotion usage (after order completion).
    #[pyo3(signature = (promotion_id, discount_amount, currency, coupon_id=None, customer_id=None, order_id=None, cart_id=None))]
    fn record_usage(
        &self,
        promotion_id: String,
        discount_amount: f64,
        currency: String,
        coupon_id: Option<String>,
        customer_id: Option<String>,
        order_id: Option<String>,
        cart_id: Option<String>,
    ) -> PyResult<PromotionUsage> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let promo_uuid =
            promotion_id.parse().map_err(|_| PyValueError::new_err("Invalid promotion UUID"))?;

        let usage = commerce
            .promotions()
            .record_usage(
                promo_uuid,
                coupon_id.and_then(|s| s.parse().ok()),
                customer_id.and_then(|s| s.parse().ok()),
                order_id.and_then(|s| s.parse().ok()),
                cart_id.and_then(|s| s.parse().ok()),
                decimal_from_f64(discount_amount, "discount_amount")?,
                &currency,
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to record usage: {}", e)))?;

        convert_output(usage)
    }
}

// ============================================================================
// Tax API
// ============================================================================

/// Tax jurisdiction data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct TaxJurisdiction {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    parent_id: Option<String>,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    level: String,
    #[pyo3(get)]
    country_code: String,
    #[pyo3(get)]
    state_code: Option<String>,
    #[pyo3(get)]
    county: Option<String>,
    #[pyo3(get)]
    city: Option<String>,
    #[pyo3(get)]
    postal_codes: Vec<String>,
    #[pyo3(get)]
    active: bool,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::TaxJurisdiction> for TaxJurisdiction {
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

/// Tax rate data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct TaxRate {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    jurisdiction_id: String,
    #[pyo3(get)]
    tax_type: String,
    #[pyo3(get)]
    product_category: String,
    #[pyo3(get)]
    rate: f64,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    is_compound: bool,
    #[pyo3(get)]
    priority: i32,
    #[pyo3(get)]
    effective_from: String,
    #[pyo3(get)]
    effective_to: Option<String>,
    #[pyo3(get)]
    active: bool,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl TryFrom<stateset_core::TaxRate> for TaxRate {
    type Error = PyErr;

    fn try_from(r: stateset_core::TaxRate) -> PyResult<Self> {
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
            effective_from: r.effective_from.to_string(),
            effective_to: r.effective_to.map(|d| d.to_string()),
            active: r.active,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
    }
}

/// Tax exemption data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct TaxExemption {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    exemption_type: String,
    #[pyo3(get)]
    certificate_number: Option<String>,
    #[pyo3(get)]
    issuing_authority: Option<String>,
    #[pyo3(get)]
    jurisdiction_ids: Vec<String>,
    #[pyo3(get)]
    exempt_categories: Vec<String>,
    #[pyo3(get)]
    effective_from: String,
    #[pyo3(get)]
    expires_at: Option<String>,
    #[pyo3(get)]
    verified: bool,
    #[pyo3(get)]
    notes: Option<String>,
    #[pyo3(get)]
    active: bool,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::TaxExemption> for TaxExemption {
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
            notes: e.notes,
            active: e.active,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

/// Tax settings data.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct TaxSettings {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    enabled: bool,
    #[pyo3(get)]
    calculation_method: String,
    #[pyo3(get)]
    compound_method: String,
    #[pyo3(get)]
    tax_shipping: bool,
    #[pyo3(get)]
    tax_handling: bool,
    #[pyo3(get)]
    tax_gift_wrap: bool,
    #[pyo3(get)]
    default_product_category: String,
    #[pyo3(get)]
    rounding_mode: String,
    #[pyo3(get)]
    decimal_places: i32,
    #[pyo3(get)]
    validate_addresses: bool,
    #[pyo3(get)]
    tax_provider: Option<String>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::TaxSettings> for TaxSettings {
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

/// Tax calculation result.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct TaxCalculationResult {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    total_tax: f64,
    #[pyo3(get)]
    subtotal: f64,
    #[pyo3(get)]
    total: f64,
    #[pyo3(get)]
    shipping_tax: f64,
    #[pyo3(get)]
    exemptions_applied: bool,
    #[pyo3(get)]
    calculated_at: String,
    #[pyo3(get)]
    is_estimate: bool,
}

impl TryFrom<stateset_core::TaxCalculationResult> for TaxCalculationResult {
    type Error = PyErr;

    fn try_from(r: stateset_core::TaxCalculationResult) -> PyResult<Self> {
        Ok(Self {
            id: r.id.to_string(),
            total_tax: to_f64_result(r.total_tax, "tax calculation total tax")?,
            subtotal: to_f64_result(r.subtotal, "tax calculation subtotal")?,
            total: to_f64_result(r.total, "tax calculation total")?,
            shipping_tax: to_f64_result(r.shipping_tax, "tax calculation shipping tax")?,
            exemptions_applied: r.exemptions_applied,
            calculated_at: r.calculated_at.to_rfc3339(),
            is_estimate: r.is_estimate,
        })
    }
}

/// US state tax info.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct UsStateTaxInfo {
    #[pyo3(get)]
    state_code: String,
    #[pyo3(get)]
    state_name: String,
    #[pyo3(get)]
    state_rate: f64,
    #[pyo3(get)]
    has_local_taxes: bool,
    #[pyo3(get)]
    origin_based: bool,
    #[pyo3(get)]
    tax_shipping: bool,
    #[pyo3(get)]
    tax_clothing: bool,
    #[pyo3(get)]
    tax_food: bool,
    #[pyo3(get)]
    tax_digital: bool,
}

impl TryFrom<stateset_core::UsStateTaxInfo> for UsStateTaxInfo {
    type Error = PyErr;

    fn try_from(i: stateset_core::UsStateTaxInfo) -> PyResult<Self> {
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

/// EU VAT info.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct EuVatInfo {
    #[pyo3(get)]
    country_code: String,
    #[pyo3(get)]
    country_name: String,
    #[pyo3(get)]
    standard_rate: f64,
    #[pyo3(get)]
    reduced_rate: Option<f64>,
    #[pyo3(get)]
    super_reduced_rate: Option<f64>,
    #[pyo3(get)]
    parking_rate: Option<f64>,
}

impl TryFrom<stateset_core::EuVatInfo> for EuVatInfo {
    type Error = PyErr;

    fn try_from(i: stateset_core::EuVatInfo) -> PyResult<Self> {
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

/// Canadian tax info.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CanadianTaxInfo {
    #[pyo3(get)]
    province_code: String,
    #[pyo3(get)]
    province_name: String,
    #[pyo3(get)]
    gst_rate: f64,
    #[pyo3(get)]
    pst_rate: Option<f64>,
    #[pyo3(get)]
    hst_rate: Option<f64>,
    #[pyo3(get)]
    qst_rate: Option<f64>,
    #[pyo3(get)]
    total_rate: f64,
}

impl TryFrom<stateset_core::CanadianTaxInfo> for CanadianTaxInfo {
    type Error = PyErr;

    fn try_from(i: stateset_core::CanadianTaxInfo) -> PyResult<Self> {
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

/// Tax operations API.
#[pyclass]
pub struct TaxApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl TaxApi {
    // ========================================================================
    // Jurisdiction Operations
    // ========================================================================

    /// Create a tax jurisdiction.
    #[pyo3(signature = (name, code, country_code, parent_id=None, level=None, state_code=None, county=None, city=None, postal_codes=None))]
    fn create_jurisdiction(
        &self,
        name: String,
        code: String,
        country_code: String,
        parent_id: Option<String>,
        level: Option<String>,
        state_code: Option<String>,
        county: Option<String>,
        city: Option<String>,
        postal_codes: Option<Vec<String>>,
    ) -> PyResult<TaxJurisdiction> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let create = stateset_core::CreateTaxJurisdiction {
            parent_id: parent_id.and_then(|s| s.parse().ok()),
            name,
            code,
            level: level.map(|s| parse_jurisdiction_level(&s)).unwrap_or_default(),
            country_code,
            state_code,
            county,
            city,
            postal_codes: postal_codes.unwrap_or_default(),
        };

        let jurisdiction = commerce.tax().create_jurisdiction(create).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to create jurisdiction: {}", e))
        })?;

        Ok(jurisdiction.into())
    }

    /// Get a jurisdiction by ID.
    fn get_jurisdiction(&self, id: String) -> PyResult<Option<TaxJurisdiction>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let jurisdiction = commerce
            .tax()
            .get_jurisdiction(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get jurisdiction: {}", e)))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// Get a jurisdiction by code.
    fn get_jurisdiction_by_code(&self, code: String) -> PyResult<Option<TaxJurisdiction>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let jurisdiction = commerce
            .tax()
            .get_jurisdiction_by_code(&code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get jurisdiction: {}", e)))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// List jurisdictions.
    #[pyo3(signature = (country_code=None, state_code=None, level=None, active_only=None))]
    fn list_jurisdictions(
        &self,
        country_code: Option<String>,
        state_code: Option<String>,
        level: Option<String>,
        active_only: Option<bool>,
    ) -> PyResult<Vec<TaxJurisdiction>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let filter = stateset_core::TaxJurisdictionFilter {
            country_code,
            state_code,
            level: level.map(|s| parse_jurisdiction_level(&s)),
            active_only: active_only.unwrap_or(false),
        };

        let jurisdictions = commerce
            .tax()
            .list_jurisdictions(filter)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list jurisdictions: {}", e)))?;

        Ok(jurisdictions.into_iter().map(|j| j.into()).collect())
    }

    // ========================================================================
    // Tax Rate Operations
    // ========================================================================

    /// Create a tax rate.
    #[pyo3(signature = (jurisdiction_id, rate, name, effective_from, tax_type=None, product_category=None, description=None, is_compound=None, priority=None, effective_to=None))]
    fn create_rate(
        &self,
        jurisdiction_id: String,
        rate: f64,
        name: String,
        effective_from: String,
        tax_type: Option<String>,
        product_category: Option<String>,
        description: Option<String>,
        is_compound: Option<bool>,
        priority: Option<i32>,
        effective_to: Option<String>,
    ) -> PyResult<TaxRate> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let jid = jurisdiction_id
            .parse()
            .map_err(|_| PyValueError::new_err("Invalid jurisdiction UUID"))?;

        let eff_from = chrono::NaiveDate::parse_from_str(&effective_from, "%Y-%m-%d")
            .map_err(|e| PyRuntimeError::new_err(format!("Invalid date format: {}", e)))?;

        let create = stateset_core::CreateTaxRate {
            jurisdiction_id: jid,
            tax_type: tax_type.map(|s| parse_tax_type(&s)).unwrap_or_default(),
            product_category: product_category
                .map(|s| parse_product_tax_category(&s))
                .unwrap_or_default(),
            rate: decimal_from_f64(rate, "rate")?,
            name,
            description,
            is_compound: is_compound.unwrap_or(false),
            priority: priority.unwrap_or(0),
            threshold_min: None,
            threshold_max: None,
            fixed_amount: None,
            effective_from: eff_from,
            effective_to: effective_to
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        };

        let rate_result = commerce
            .tax()
            .create_rate(create)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create rate: {}", e)))?;

        convert_output(rate_result)
    }

    /// Get a rate by ID.
    fn get_rate(&self, id: String) -> PyResult<Option<TaxRate>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let rate = commerce
            .tax()
            .get_rate(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get rate: {}", e)))?;

        convert_optional_output(rate)
    }

    /// List tax rates.
    #[pyo3(signature = (jurisdiction_id=None, tax_type=None, product_category=None, active_only=None))]
    fn list_rates(
        &self,
        jurisdiction_id: Option<String>,
        tax_type: Option<String>,
        product_category: Option<String>,
        active_only: Option<bool>,
    ) -> PyResult<Vec<TaxRate>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let filter = stateset_core::TaxRateFilter {
            jurisdiction_id: jurisdiction_id.and_then(|s| s.parse().ok()),
            tax_type: tax_type.map(|s| parse_tax_type(&s)),
            product_category: product_category.map(|s| parse_product_tax_category(&s)),
            active_only: active_only.unwrap_or(false),
            effective_date: None,
        };

        let rates = commerce
            .tax()
            .list_rates(filter)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list rates: {}", e)))?;

        convert_outputs(rates)
    }

    // ========================================================================
    // Exemption Operations
    // ========================================================================

    /// Create a tax exemption.
    #[pyo3(signature = (customer_id, exemption_type, effective_from, certificate_number=None, issuing_authority=None, jurisdiction_ids=None, exempt_categories=None, expires_at=None, notes=None))]
    fn create_exemption(
        &self,
        customer_id: String,
        exemption_type: String,
        effective_from: String,
        certificate_number: Option<String>,
        issuing_authority: Option<String>,
        jurisdiction_ids: Option<Vec<String>>,
        exempt_categories: Option<Vec<String>>,
        expires_at: Option<String>,
        notes: Option<String>,
    ) -> PyResult<TaxExemption> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let cid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;

        let eff_from = chrono::NaiveDate::parse_from_str(&effective_from, "%Y-%m-%d")
            .map_err(|e| PyRuntimeError::new_err(format!("Invalid date format: {}", e)))?;

        let create = stateset_core::CreateTaxExemption {
            customer_id: cid,
            exemption_type: parse_exemption_type(&exemption_type),
            certificate_number,
            issuing_authority,
            jurisdiction_ids: jurisdiction_ids
                .unwrap_or_default()
                .into_iter()
                .filter_map(|s| s.parse().ok())
                .collect(),
            exempt_categories: exempt_categories
                .unwrap_or_default()
                .into_iter()
                .map(|s| parse_product_tax_category(&s))
                .collect(),
            effective_from: eff_from,
            expires_at: expires_at
                .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            notes,
        };

        let exemption = commerce
            .tax()
            .create_exemption(create)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create exemption: {}", e)))?;

        Ok(exemption.into())
    }

    /// Get an exemption by ID.
    fn get_exemption(&self, id: String) -> PyResult<Option<TaxExemption>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let exemption = commerce
            .tax()
            .get_exemption(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get exemption: {}", e)))?;

        Ok(exemption.map(|e| e.into()))
    }

    /// Get exemptions for a customer.
    fn get_customer_exemptions(&self, customer_id: String) -> PyResult<Vec<TaxExemption>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let exemptions = commerce
            .tax()
            .get_customer_exemptions(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get exemptions: {}", e)))?;

        Ok(exemptions.into_iter().map(|e| e.into()).collect())
    }

    /// Check if a customer is tax exempt.
    fn customer_is_exempt(&self, customer_id: String) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let is_exempt = commerce
            .tax()
            .customer_is_exempt(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to check exemption: {}", e)))?;

        Ok(is_exempt)
    }

    // ========================================================================
    // Settings Operations
    // ========================================================================

    /// Get tax settings.
    fn get_settings(&self) -> PyResult<TaxSettings> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let settings = commerce
            .tax()
            .get_settings()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Enable or disable tax calculation.
    fn set_enabled(&self, enabled: bool) -> PyResult<TaxSettings> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let settings = commerce
            .tax()
            .set_enabled(enabled)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update settings: {}", e)))?;

        Ok(settings.into())
    }

    /// Check if tax calculation is enabled.
    fn is_enabled(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

        let enabled = commerce
            .tax()
            .is_enabled()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to check settings: {}", e)))?;

        Ok(enabled)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get US state tax information.
    #[staticmethod]
    fn get_us_state_info(state_code: String) -> PyResult<Option<UsStateTaxInfo>> {
        convert_optional_output(stateset_core::get_us_state_tax_info(&state_code))
    }

    /// Get EU VAT information.
    #[staticmethod]
    fn get_eu_vat_info(country_code: String) -> PyResult<Option<EuVatInfo>> {
        convert_optional_output(stateset_core::get_eu_vat_info(&country_code))
    }

    /// Get Canadian tax information.
    #[staticmethod]
    fn get_canadian_tax_info(province_code: String) -> PyResult<Option<CanadianTaxInfo>> {
        convert_optional_output(stateset_core::get_canadian_tax_info(&province_code))
    }

    /// Check if a country is in the EU.
    #[staticmethod]
    fn is_eu_country(country_code: String) -> bool {
        stateset_core::is_eu_member(&country_code)
    }
}

// ============================================================================
// Quality Control Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Inspection {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    inspection_number: String,
    #[pyo3(get)]
    inspection_type: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    reference_type: String,
    #[pyo3(get)]
    reference_id: String,
    #[pyo3(get)]
    inspector_id: Option<String>,
    #[pyo3(get)]
    notes: Option<String>,
    #[pyo3(get)]
    created_at: String,
}

#[pymethods]
impl Inspection {
    fn __repr__(&self) -> String {
        format!("Inspection(number='{}', status='{}')", self.inspection_number, self.status)
    }
}

impl From<stateset_core::Inspection> for Inspection {
    fn from(i: stateset_core::Inspection) -> Self {
        Self {
            id: i.id.to_string(),
            inspection_number: i.inspection_number,
            inspection_type: format!("{:?}", i.inspection_type),
            status: format!("{:?}", i.status),
            reference_type: i.reference_type,
            reference_id: i.reference_id.to_string(),
            inspector_id: i.inspector_id,
            notes: i.notes,
            created_at: i.created_at.to_rfc3339(),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct NonConformance {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    ncr_number: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    description: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    source: String,
    #[pyo3(get)]
    severity: String,
    #[pyo3(get)]
    quantity_affected: f64,
}

impl TryFrom<stateset_core::NonConformance> for NonConformance {
    type Error = PyErr;

    fn try_from(n: stateset_core::NonConformance) -> PyResult<Self> {
        Ok(Self {
            id: n.id.to_string(),
            ncr_number: n.ncr_number,
            sku: n.sku,
            description: n.description,
            status: format!("{:?}", n.status),
            source: format!("{:?}", n.source),
            severity: format!("{:?}", n.severity),
            quantity_affected: to_f64_result(
                n.quantity_affected,
                "non-conformance quantity affected",
            )?,
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct QualityHold {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    reason: String,
    #[pyo3(get)]
    quantity_held: f64,
    #[pyo3(get)]
    hold_type: String,
    #[pyo3(get)]
    placed_by: String,
}

impl TryFrom<stateset_core::QualityHold> for QualityHold {
    type Error = PyErr;

    fn try_from(h: stateset_core::QualityHold) -> PyResult<Self> {
        Ok(Self {
            id: h.id.to_string(),
            sku: h.sku,
            reason: h.reason,
            quantity_held: to_f64_result(h.quantity_held, "quality hold quantity held")?,
            hold_type: format!("{:?}", h.hold_type),
            placed_by: h.placed_by,
        })
    }
}

// ============================================================================
// Quality Control API
// ============================================================================

#[pyclass]
pub struct QualityApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl QualityApi {
    #[pyo3(signature = (reference_type, reference_id, inspection_type, inspector_id=None))]
    fn create_inspection(
        &self,
        reference_type: String,
        reference_id: String,
        inspection_type: String,
        inspector_id: Option<String>,
    ) -> PyResult<Inspection> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let itype = match inspection_type.to_lowercase().as_str() {
            "incoming" => stateset_core::InspectionType::Incoming,
            "receiving" => stateset_core::InspectionType::Receiving,
            "in_process" => stateset_core::InspectionType::InProcess,
            "final" => stateset_core::InspectionType::Final,
            "random" => stateset_core::InspectionType::Random,
            _ => stateset_core::InspectionType::Incoming,
        };
        let ref_uuid =
            reference_id.parse().map_err(|_| PyValueError::new_err("Invalid reference UUID"))?;
        let inspection = commerce
            .quality()
            .create_inspection(stateset_core::CreateInspection {
                inspection_type: itype,
                reference_type,
                reference_id: ref_uuid,
                inspector_id,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(inspection.into())
    }

    fn get_inspection(&self, id: String) -> PyResult<Option<Inspection>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .get_inspection(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(inspection.map(|i| i.into()))
    }

    fn list_inspections(&self) -> PyResult<Vec<Inspection>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let inspections = commerce
            .quality()
            .list_inspections(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(inspections.into_iter().map(|i| i.into()).collect())
    }

    fn complete_inspection(&self, id: String) -> PyResult<Inspection> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .complete_inspection(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(inspection.into())
    }

    fn create_ncr(
        &self,
        sku: String,
        description: String,
        quantity_affected: f64,
        source: String,
        severity: String,
    ) -> PyResult<NonConformance> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let src = match source.to_lowercase().as_str() {
            "inspection" => stateset_core::NonConformanceSource::Inspection,
            "production" | "production_defect" => {
                stateset_core::NonConformanceSource::ProductionDefect
            }
            "customer" | "customer_complaint" => {
                stateset_core::NonConformanceSource::CustomerComplaint
            }
            "supplier" | "supplier_issue" => stateset_core::NonConformanceSource::SupplierIssue,
            "internal_audit" => stateset_core::NonConformanceSource::InternalAudit,
            "shipping_damage" => stateset_core::NonConformanceSource::ShippingDamage,
            _ => stateset_core::NonConformanceSource::Inspection,
        };
        let sev = match severity.to_lowercase().as_str() {
            "critical" => stateset_core::Severity::Critical,
            "major" => stateset_core::Severity::Major,
            "minor" => stateset_core::Severity::Minor,
            "observation" => stateset_core::Severity::Observation,
            _ => stateset_core::Severity::Minor,
        };
        let ncr = commerce
            .quality()
            .create_ncr(stateset_core::CreateNonConformance {
                sku,
                description,
                quantity_affected: decimal_from_f64(quantity_affected, "quantity_affected")?,
                source: src,
                severity: sev,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(ncr)
    }

    fn list_ncrs(&self) -> PyResult<Vec<NonConformance>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let ncrs = commerce
            .quality()
            .list_ncrs(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(ncrs)
    }

    fn create_hold(&self, sku: String, reason: String, quantity: f64) -> PyResult<QualityHold> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let hold = commerce
            .quality()
            .create_hold(stateset_core::CreateQualityHold {
                sku,
                reason,
                quantity: decimal_from_f64(quantity, "quantity")?,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(hold)
    }

    #[pyo3(signature = (id, released_by, release_notes=None))]
    fn release_hold(
        &self,
        id: String,
        released_by: String,
        release_notes: Option<String>,
    ) -> PyResult<QualityHold> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let hold = commerce
            .quality()
            .release_hold(uuid, stateset_core::ReleaseQualityHold { released_by, release_notes })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(hold)
    }
}

// ============================================================================
// Lot/Batch Tracking Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Lot {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    lot_number: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    quantity_remaining: f64,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    expiration_date: Option<String>,
    #[pyo3(get)]
    created_at: String,
}

impl TryFrom<stateset_core::Lot> for Lot {
    type Error = PyErr;

    fn try_from(l: stateset_core::Lot) -> PyResult<Self> {
        Ok(Self {
            id: l.id.to_string(),
            lot_number: l.lot_number,
            sku: l.sku,
            quantity_remaining: to_f64_result(l.quantity_remaining, "lot quantity remaining")?,
            status: format!("{:?}", l.status),
            expiration_date: l.expiration_date.map(|d| d.to_rfc3339()),
            created_at: l.created_at.to_rfc3339(),
        })
    }
}

// ============================================================================
// Lots API
// ============================================================================

#[pyclass]
pub struct LotsApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl LotsApi {
    #[pyo3(signature = (sku, lot_number, quantity, expiration_date=None))]
    fn create_lot(
        &self,
        sku: String,
        lot_number: String,
        quantity: f64,
        expiration_date: Option<String>,
    ) -> PyResult<Lot> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let exp = expiration_date
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let lot = commerce
            .lots()
            .create(stateset_core::CreateLot {
                sku,
                lot_number: Some(lot_number),
                quantity: decimal_from_f64(quantity, "quantity")?,
                expiration_date: exp,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(lot)
    }

    fn get_lot(&self, id: String) -> PyResult<Option<Lot>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let lot = commerce.lots().get(uuid).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(lot)
    }

    fn list_lots(&self) -> PyResult<Vec<Lot>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let lots = commerce
            .lots()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(lots)
    }

    fn get_lots_by_sku(&self, sku: String) -> PyResult<Vec<Lot>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let lots = commerce
            .lots()
            .get_available_lots_for_sku(&sku)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(lots)
    }

    fn quarantine_lot(&self, id: String, reason: String) -> PyResult<Lot> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let lot = commerce
            .lots()
            .quarantine(uuid, &reason)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(lot)
    }

    fn get_expiring_lots(&self, days_ahead: i32) -> PyResult<Vec<Lot>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let lots = commerce
            .lots()
            .get_expiring_lots(days_ahead)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(lots)
    }
}

// ============================================================================
// Serial Number Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SerialNumber {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    serial: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    created_at: String,
}

impl From<stateset_core::SerialNumber> for SerialNumber {
    fn from(s: stateset_core::SerialNumber) -> Self {
        Self {
            id: s.id.to_string(),
            serial: s.serial,
            sku: s.sku,
            status: format!("{:?}", s.status),
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Serials API
// ============================================================================

#[pyclass]
pub struct SerialsApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl SerialsApi {
    #[pyo3(signature = (sku, serial=None))]
    fn create(&self, sku: String, serial: Option<String>) -> PyResult<SerialNumber> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let s = commerce
            .serials()
            .create(stateset_core::CreateSerialNumber { sku, serial, ..Default::default() })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(s.into())
    }

    fn get_by_serial(&self, serial: String) -> PyResult<Option<SerialNumber>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let s = commerce
            .serials()
            .get_by_serial(&serial)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(s.map(|s| s.into()))
    }

    fn list(&self) -> PyResult<Vec<SerialNumber>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let serials = commerce
            .serials()
            .list(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(serials.into_iter().map(|s| s.into()).collect())
    }

    fn change_status(&self, id: String, status: String) -> PyResult<SerialNumber> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let serial_status = match status.to_lowercase().as_str() {
            "available" => stateset_core::SerialStatus::Available,
            "sold" => stateset_core::SerialStatus::Sold,
            "returned" => stateset_core::SerialStatus::Returned,
            "scrapped" => stateset_core::SerialStatus::Scrapped,
            "reserved" => stateset_core::SerialStatus::Reserved,
            "shipped" => stateset_core::SerialStatus::Shipped,
            "quarantine" | "quarantined" => stateset_core::SerialStatus::Quarantined,
            _ => stateset_core::SerialStatus::Available,
        };
        let serial = commerce
            .serials()
            .change_status(stateset_core::ChangeSerialStatus {
                serial_id: uuid,
                new_status: serial_status,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(serial.into())
    }
}

// ============================================================================
// Warehouse Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Warehouse {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    address: Option<String>,
    #[pyo3(get)]
    is_active: bool,
}

impl From<stateset_core::Warehouse> for Warehouse {
    fn from(w: stateset_core::Warehouse) -> Self {
        // Convert WarehouseAddress to a simple string representation
        let addr_str = if w.address.street1.is_empty() && w.address.city.is_empty() {
            None
        } else {
            Some(format!("{}, {}, {}", w.address.street1, w.address.city, w.address.country))
        };
        Self {
            id: w.id.to_string(),
            code: w.code,
            name: w.name,
            address: addr_str,
            is_active: w.is_active,
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct WarehouseLocation {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    warehouse_id: String,
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    location_type: String,
}

impl From<stateset_core::Location> for WarehouseLocation {
    fn from(l: stateset_core::Location) -> Self {
        Self {
            id: l.id.to_string(),
            warehouse_id: l.warehouse_id.to_string(),
            code: l.code,
            location_type: format!("{:?}", l.location_type),
        }
    }
}

// ============================================================================
// Warehouse API
// ============================================================================

#[pyclass]
pub struct WarehouseApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl WarehouseApi {
    #[pyo3(signature = (code, name, address=None))]
    fn create_warehouse(
        &self,
        code: String,
        name: String,
        address: Option<String>,
    ) -> PyResult<Warehouse> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let _ = address; // Address is simplified - WarehouseAddress uses Default
        let warehouse = commerce
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse { code, name, ..Default::default() })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(warehouse.into())
    }

    fn get_warehouse(&self, id: String) -> PyResult<Option<Warehouse>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wh_id: i32 = id.parse().map_err(|_| PyValueError::new_err("Invalid warehouse ID"))?;
        let warehouse = commerce
            .warehouse()
            .get_warehouse(wh_id)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(warehouse.map(|w| w.into()))
    }

    fn list_warehouses(&self) -> PyResult<Vec<Warehouse>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let warehouses = commerce
            .warehouse()
            .list_warehouses(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(warehouses.into_iter().map(|w| w.into()).collect())
    }

    #[pyo3(signature = (warehouse_id, code, location_type=None))]
    fn create_location(
        &self,
        warehouse_id: String,
        code: String,
        location_type: Option<String>,
    ) -> PyResult<WarehouseLocation> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wh_id: i32 =
            warehouse_id.parse().map_err(|_| PyValueError::new_err("Invalid warehouse ID"))?;
        let loc_type = location_type
            .map(|t| match t.to_lowercase().as_str() {
                "pick" | "picking" => stateset_core::LocationType::Pick,
                "bulk" => stateset_core::LocationType::Bulk,
                "receiving" => stateset_core::LocationType::Receiving,
                "shipping" => stateset_core::LocationType::Shipping,
                "staging" => stateset_core::LocationType::Staging,
                "quarantine" => stateset_core::LocationType::Quarantine,
                _ => stateset_core::LocationType::Bulk,
            })
            .unwrap_or(stateset_core::LocationType::Bulk);
        let location = commerce
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id: wh_id,
                code: Some(code),
                location_type: loc_type,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(location.into())
    }

    fn get_locations_for_warehouse(
        &self,
        warehouse_id: String,
    ) -> PyResult<Vec<WarehouseLocation>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wh_id: i32 =
            warehouse_id.parse().map_err(|_| PyValueError::new_err("Invalid warehouse ID"))?;
        let locations = commerce
            .warehouse()
            .get_locations_for_warehouse(wh_id)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(locations.into_iter().map(|l| l.into()).collect())
    }
}

// ============================================================================
// Receiving Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Receipt {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    receipt_number: String,
    #[pyo3(get)]
    receipt_type: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    reference_id: Option<String>,
    #[pyo3(get)]
    supplier_id: Option<String>,
    #[pyo3(get)]
    warehouse_id: String,
    #[pyo3(get)]
    created_at: String,
}

impl From<stateset_core::Receipt> for Receipt {
    fn from(r: stateset_core::Receipt) -> Self {
        Self {
            id: r.id.to_string(),
            receipt_number: r.receipt_number,
            receipt_type: format!("{:?}", r.receipt_type),
            status: format!("{:?}", r.status),
            reference_id: r.reference_id.map(|id| id.to_string()),
            supplier_id: r.supplier_id.map(|id| id.to_string()),
            warehouse_id: r.warehouse_id.to_string(),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ReceiptLine {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    receipt_id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    expected_quantity: f64,
    #[pyo3(get)]
    received_quantity: f64,
    #[pyo3(get)]
    unit_cost: Option<f64>,
    #[pyo3(get)]
    status: String,
}

impl TryFrom<stateset_core::ReceiptItem> for ReceiptLine {
    type Error = PyErr;

    fn try_from(l: stateset_core::ReceiptItem) -> PyResult<Self> {
        Ok(Self {
            id: l.id.to_string(),
            receipt_id: l.receipt_id.to_string(),
            sku: l.sku,
            expected_quantity: to_f64_result(
                l.expected_quantity,
                "receipt line expected quantity",
            )?,
            received_quantity: to_f64_result(
                l.received_quantity,
                "receipt line received quantity",
            )?,
            unit_cost: optional_to_f64_result(l.unit_cost, "receipt line unit cost")?,
            status: format!("{:?}", l.status),
        })
    }
}

// ============================================================================
// Receiving API
// ============================================================================

#[pyclass]
pub struct ReceivingApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl ReceivingApi {
    #[pyo3(signature = (warehouse_id, supplier_id=None))]
    fn create_receipt(
        &self,
        warehouse_id: String,
        supplier_id: Option<String>,
    ) -> PyResult<Receipt> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wh_id: i32 =
            warehouse_id.parse().map_err(|_| PyValueError::new_err("Invalid warehouse ID"))?;
        let sup_uuid = supplier_id.and_then(|id| id.parse().ok());
        let receipt = commerce
            .receiving()
            .create_receipt(stateset_core::CreateReceipt {
                warehouse_id: wh_id,
                supplier_id: sup_uuid,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(receipt.into())
    }

    fn get_receipt(&self, id: String) -> PyResult<Option<Receipt>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .get_receipt(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(receipt.map(|r| r.into()))
    }

    fn list_receipts(&self) -> PyResult<Vec<Receipt>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let receipts = commerce
            .receiving()
            .list_receipts(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(receipts.into_iter().map(|r| r.into()).collect())
    }

    fn get_receipt_items(&self, receipt_id: String) -> PyResult<Vec<ReceiptLine>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = receipt_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let items = commerce
            .receiving()
            .get_receipt_items(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(items)
    }

    fn complete_receiving(&self, id: String) -> PyResult<Receipt> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .complete_receiving(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(receipt.into())
    }
}

// ============================================================================
// Fulfillment Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Wave {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    wave_number: String,
    #[pyo3(get)]
    warehouse_id: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    order_count: i32,
    #[pyo3(get)]
    pick_count: i32,
}

impl From<stateset_core::Wave> for Wave {
    fn from(w: stateset_core::Wave) -> Self {
        Self {
            id: w.id.to_string(),
            wave_number: w.wave_number,
            warehouse_id: w.warehouse_id.to_string(),
            status: format!("{:?}", w.status),
            order_count: w.order_count,
            pick_count: w.pick_count,
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct PickTask {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    order_id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    quantity_requested: f64,
    #[pyo3(get)]
    quantity_picked: f64,
    #[pyo3(get)]
    status: String,
}

impl TryFrom<stateset_core::PickTask> for PickTask {
    type Error = PyErr;

    fn try_from(t: stateset_core::PickTask) -> PyResult<Self> {
        Ok(Self {
            id: t.id.to_string(),
            order_id: t.order_id.to_string(),
            sku: t.sku,
            quantity_requested: to_f64_result(
                t.quantity_requested,
                "pick task quantity requested",
            )?,
            quantity_picked: to_f64_result(t.quantity_picked, "pick task quantity picked")?,
            status: format!("{:?}", t.status),
        })
    }
}

// ============================================================================
// Fulfillment API
// ============================================================================

#[pyclass]
pub struct FulfillmentApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl FulfillmentApi {
    fn create_wave(&self, warehouse_id: String, order_ids: Vec<String>) -> PyResult<Wave> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wh_id: i32 =
            warehouse_id.parse().map_err(|_| PyValueError::new_err("Invalid warehouse ID"))?;
        let order_typed_ids: Vec<stateset_core::OrderId> = order_ids
            .iter()
            .filter_map(|id| id.parse::<uuid::Uuid>().ok().map(Into::into))
            .collect();
        let wave = commerce
            .fulfillment()
            .create_wave(stateset_core::CreateWave {
                warehouse_id: wh_id,
                order_ids: order_typed_ids,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(wave.into())
    }

    fn get_wave(&self, id: String) -> PyResult<Option<Wave>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .get_wave(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(wave.map(|w| w.into()))
    }

    fn list_waves(&self) -> PyResult<Vec<Wave>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let waves = commerce
            .fulfillment()
            .list_waves(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(waves.into_iter().map(|w| w.into()).collect())
    }

    fn release_wave(&self, id: String) -> PyResult<Wave> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let wave = commerce
            .fulfillment()
            .release_wave(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(wave.into())
    }

    fn list_picks(&self) -> PyResult<Vec<PickTask>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let picks = commerce
            .fulfillment()
            .list_picks(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(picks)
    }

    fn complete_pick(&self, id: String, quantity_picked: f64) -> PyResult<PickTask> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let task = commerce
            .fulfillment()
            .complete_pick(stateset_core::CompletePick {
                pick_id: uuid,
                quantity_picked: decimal_from_f64(quantity_picked, "quantity_picked")?,
                quantity_short: None,
                short_reason: None,
                lot_id: None,
                serial_number: None,
                completed_by: None,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(task)
    }
}

// ============================================================================
// Accounts Payable Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Bill {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    bill_number: String,
    #[pyo3(get)]
    supplier_id: String,
    #[pyo3(get)]
    total_amount: f64,
    #[pyo3(get)]
    amount_paid: f64,
    #[pyo3(get)]
    amount_due: f64,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    due_date: String,
}

impl TryFrom<stateset_core::Bill> for Bill {
    type Error = PyErr;

    fn try_from(b: stateset_core::Bill) -> PyResult<Self> {
        Ok(Self {
            id: b.id.to_string(),
            bill_number: b.bill_number,
            supplier_id: b.supplier_id.to_string(),
            total_amount: to_f64_result(b.total_amount, "bill total amount")?,
            amount_paid: to_f64_result(b.amount_paid, "bill amount paid")?,
            amount_due: to_f64_result(b.amount_due, "bill amount due")?,
            status: format!("{:?}", b.status),
            due_date: b.due_date.to_rfc3339(),
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ApAgingSummary {
    #[pyo3(get)]
    current: f64,
    #[pyo3(get)]
    days_1_30: f64,
    #[pyo3(get)]
    days_31_60: f64,
    #[pyo3(get)]
    days_61_90: f64,
    #[pyo3(get)]
    days_over_90: f64,
    #[pyo3(get)]
    total: f64,
}

impl TryFrom<stateset_core::ApAgingSummary> for ApAgingSummary {
    type Error = PyErr;

    fn try_from(s: stateset_core::ApAgingSummary) -> PyResult<Self> {
        Ok(Self {
            current: to_f64_result(s.current, "accounts payable aging current")?,
            days_1_30: to_f64_result(s.days_1_30, "accounts payable aging 1-30 days")?,
            days_31_60: to_f64_result(s.days_31_60, "accounts payable aging 31-60 days")?,
            days_61_90: to_f64_result(s.days_61_90, "accounts payable aging 61-90 days")?,
            days_over_90: to_f64_result(s.days_over_90, "accounts payable aging over 90 days")?,
            total: to_f64_result(s.total, "accounts payable aging total")?,
        })
    }
}

// ============================================================================
// Accounts Payable API
// ============================================================================

#[pyclass]
pub struct AccountsPayableApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl AccountsPayableApi {
    fn create_bill(&self, supplier_id: String, due_date: String) -> PyResult<Bill> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = supplier_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let due = chrono::DateTime::parse_from_rfc3339(&due_date)
            .map_err(|_| PyValueError::new_err("Invalid due_date format"))?
            .with_timezone(&chrono::Utc);
        let bill = commerce
            .accounts_payable()
            .create_bill(stateset_core::CreateBill {
                supplier_id: uuid,
                due_date: due,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(bill)
    }

    fn get_bill(&self, id: String) -> PyResult<Option<Bill>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .get_bill(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(bill)
    }

    fn list_bills(&self) -> PyResult<Vec<Bill>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let bills = commerce
            .accounts_payable()
            .list_bills(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(bills)
    }

    fn approve_bill(&self, id: String) -> PyResult<Bill> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .approve_bill(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(bill)
    }

    #[pyo3(signature = (id, amount, payment_method=None))]
    fn pay_bill(&self, id: String, amount: f64, payment_method: Option<String>) -> PyResult<Bill> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let pm = payment_method
            .map(|s| match s.to_lowercase().as_str() {
                "check" => stateset_core::PaymentMethodAP::Check,
                "ach" => stateset_core::PaymentMethodAP::Ach,
                "wire" => stateset_core::PaymentMethodAP::Wire,
                "credit_card" => stateset_core::PaymentMethodAP::CreditCard,
                "cash" => stateset_core::PaymentMethodAP::Cash,
                _ => stateset_core::PaymentMethodAP::Other,
            })
            .unwrap_or(stateset_core::PaymentMethodAP::Check);
        let bill = commerce
            .accounts_payable()
            .pay_bill(
                uuid,
                stateset_core::PayBill {
                    amount: decimal_from_f64(amount, "amount")?,
                    payment_method: pm,
                    ..Default::default()
                },
            )
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(bill)
    }

    fn get_aging_summary(&self) -> PyResult<ApAgingSummary> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let summary = commerce
            .accounts_payable()
            .get_aging_summary()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(summary)
    }

    fn get_overdue_bills(&self) -> PyResult<Vec<Bill>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let bills = commerce
            .accounts_payable()
            .get_overdue_bills()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(bills)
    }

    /// Three-way match a bill against its purchase order and receipts.
    ///
    /// `tolerance_percent` is an exact decimal string (e.g. "5" for 5%);
    /// omit it for exact matching.
    #[pyo3(signature = (bill_id, tolerance_percent=None))]
    fn three_way_match(
        &self,
        bill_id: String,
        tolerance_percent: Option<String>,
    ) -> PyResult<ThreeWayMatchResult> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            bill_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let tolerance = tolerance_percent
            .map(|s| {
                s.parse::<Decimal>()
                    .map_err(|_| PyValueError::new_err("Invalid tolerance_percent decimal"))
            })
            .transpose()?;
        let result = commerce.accounts_payable().three_way_match(uuid, tolerance).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to three-way match bill: {}", e))
        })?;
        Ok(result.into())
    }
}

// ============================================================================
// Accounts Receivable Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ArAgingSummary {
    #[pyo3(get)]
    current: f64,
    #[pyo3(get)]
    days_1_30: f64,
    #[pyo3(get)]
    days_31_60: f64,
    #[pyo3(get)]
    days_61_90: f64,
    #[pyo3(get)]
    days_over_90: f64,
    #[pyo3(get)]
    total: f64,
}

impl TryFrom<stateset_core::ArAgingSummary> for ArAgingSummary {
    type Error = PyErr;

    fn try_from(s: stateset_core::ArAgingSummary) -> PyResult<Self> {
        Ok(Self {
            current: to_f64_result(s.current, "accounts receivable aging current")?,
            days_1_30: to_f64_result(s.days_1_30, "accounts receivable aging 1-30 days")?,
            days_31_60: to_f64_result(s.days_31_60, "accounts receivable aging 31-60 days")?,
            days_61_90: to_f64_result(s.days_61_90, "accounts receivable aging 61-90 days")?,
            days_over_90: to_f64_result(s.days_over_90, "accounts receivable aging over 90 days")?,
            total: to_f64_result(s.total, "accounts receivable aging total")?,
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CreditMemo {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    credit_memo_number: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    amount: f64,
    #[pyo3(get)]
    reason: String,
    #[pyo3(get)]
    status: String,
}

impl TryFrom<stateset_core::CreditMemo> for CreditMemo {
    type Error = PyErr;

    fn try_from(m: stateset_core::CreditMemo) -> PyResult<Self> {
        Ok(Self {
            id: m.id.to_string(),
            credit_memo_number: m.credit_memo_number,
            customer_id: m.customer_id.to_string(),
            amount: to_f64_result(m.amount, "credit memo amount")?,
            reason: format!("{:?}", m.reason),
            status: format!("{:?}", m.status),
        })
    }
}

// ============================================================================
// Accounts Receivable API
// ============================================================================

#[pyclass]
pub struct AccountsReceivableApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl AccountsReceivableApi {
    fn get_aging_summary(&self) -> PyResult<ArAgingSummary> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let summary = commerce
            .accounts_receivable()
            .get_aging_summary()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(summary)
    }

    fn create_credit_memo(
        &self,
        customer_id: String,
        amount: f64,
        reason: String,
    ) -> PyResult<CreditMemo> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let r = match reason.to_lowercase().as_str() {
            "returned_goods" | "return" => stateset_core::CreditMemoReason::ReturnedGoods,
            "pricing_error" | "price" => stateset_core::CreditMemoReason::PricingError,
            "overpayment" => stateset_core::CreditMemoReason::Overpayment,
            "damaged" => stateset_core::CreditMemoReason::Damaged,
            "service_credit" | "service" => stateset_core::CreditMemoReason::ServiceCredit,
            "goodwill" | "adjustment" => stateset_core::CreditMemoReason::GoodwillAdjustment,
            _ => stateset_core::CreditMemoReason::Other,
        };
        let memo = commerce
            .accounts_receivable()
            .create_credit_memo(stateset_core::CreateCreditMemo {
                customer_id: uuid,
                amount: decimal_from_f64(amount, "amount")?,
                reason: r,
                original_invoice_id: None,
                notes: None,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(memo)
    }

    #[pyo3(signature = (days=None))]
    fn get_dso(&self, days: Option<i32>) -> PyResult<f64> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let dso = commerce
            .accounts_receivable()
            .get_dso(days.unwrap_or(30))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        to_f64_result(dso, "days sales outstanding")
    }
}

// ============================================================================
// Cost Accounting Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ItemCost {
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    standard_cost: f64,
    #[pyo3(get)]
    average_cost: f64,
    #[pyo3(get)]
    last_cost: f64,
    #[pyo3(get)]
    material_cost: f64,
    #[pyo3(get)]
    labor_cost: f64,
    #[pyo3(get)]
    overhead_cost: f64,
}

impl TryFrom<stateset_core::ItemCost> for ItemCost {
    type Error = PyErr;

    fn try_from(c: stateset_core::ItemCost) -> PyResult<Self> {
        Ok(Self {
            sku: c.sku,
            standard_cost: to_f64_result(c.standard_cost, "standard cost")?,
            average_cost: to_f64_result(c.average_cost, "average cost")?,
            last_cost: to_f64_result(c.last_cost, "last cost")?,
            material_cost: to_f64_result(c.material_cost, "material cost")?,
            labor_cost: to_f64_result(c.labor_cost, "labor cost")?,
            overhead_cost: to_f64_result(c.overhead_cost, "overhead cost")?,
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct InventoryValuation {
    #[pyo3(get)]
    total_value: f64,
    #[pyo3(get)]
    total_quantity: f64,
    #[pyo3(get)]
    average_unit_cost: f64,
}

impl TryFrom<stateset_core::InventoryValuation> for InventoryValuation {
    type Error = PyErr;

    fn try_from(v: stateset_core::InventoryValuation) -> PyResult<Self> {
        Ok(Self {
            total_value: to_f64_result(v.total_value, "total value")?,
            total_quantity: to_f64_result(v.total_quantity, "total quantity")?,
            average_unit_cost: to_f64_result(v.average_unit_cost, "average unit cost")?,
        })
    }
}

// ============================================================================
// Cost Accounting API
// ============================================================================

#[pyclass]
pub struct CostAccountingApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl CostAccountingApi {
    fn get_item_cost(&self, sku: String) -> PyResult<Option<ItemCost>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let cost = commerce
            .cost_accounting()
            .get_item_cost(&sku)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(cost)
    }

    #[pyo3(signature = (sku, standard_cost=None, material_cost=None, labor_cost=None, overhead_cost=None))]
    fn set_item_cost(
        &self,
        sku: String,
        standard_cost: Option<f64>,
        material_cost: Option<f64>,
        labor_cost: Option<f64>,
        overhead_cost: Option<f64>,
    ) -> PyResult<ItemCost> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let cost = commerce
            .cost_accounting()
            .set_item_cost(stateset_core::SetItemCost {
                sku,
                standard_cost: optional_decimal_from_f64(standard_cost, "standard_cost")?,
                material_cost: optional_decimal_from_f64(material_cost, "material_cost")?,
                labor_cost: optional_decimal_from_f64(labor_cost, "labor_cost")?,
                overhead_cost: optional_decimal_from_f64(overhead_cost, "overhead_cost")?,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(cost)
    }

    fn update_average_cost(
        &self,
        sku: String,
        quantity: f64,
        unit_cost: f64,
    ) -> PyResult<ItemCost> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let cost = commerce
            .cost_accounting()
            .update_average_cost(
                &sku,
                decimal_from_f64(quantity, "quantity")?,
                decimal_from_f64(unit_cost, "unit_cost")?,
            )
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(cost)
    }

    #[pyo3(signature = (cost_method=None))]
    fn get_inventory_valuation(&self, cost_method: Option<String>) -> PyResult<InventoryValuation> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let method = cost_method
            .and_then(|m| match m.to_lowercase().as_str() {
                "standard" => Some(stateset_core::CostMethod::Standard),
                "average" => Some(stateset_core::CostMethod::Average),
                "fifo" => Some(stateset_core::CostMethod::Fifo),
                "lifo" => Some(stateset_core::CostMethod::Lifo),
                _ => None,
            })
            .unwrap_or(stateset_core::CostMethod::Average);
        let valuation = commerce
            .cost_accounting()
            .get_inventory_valuation(method)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(valuation)
    }

    fn get_total_inventory_value(&self) -> PyResult<f64> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let value = commerce
            .cost_accounting()
            .get_total_inventory_value()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        to_f64_result(value, "inventory value")
    }
}

// ============================================================================
// Credit Management Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CreditAccount {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    credit_limit: f64,
    #[pyo3(get)]
    current_balance: f64,
    #[pyo3(get)]
    available_credit: f64,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    payment_terms: Option<String>,
}

impl TryFrom<stateset_core::CreditAccount> for CreditAccount {
    type Error = PyErr;

    fn try_from(a: stateset_core::CreditAccount) -> PyResult<Self> {
        Ok(Self {
            id: a.id.to_string(),
            customer_id: a.customer_id.to_string(),
            credit_limit: to_f64_result(a.credit_limit, "credit limit")?,
            current_balance: to_f64_result(a.current_balance, "current balance")?,
            available_credit: to_f64_result(a.available_credit, "available credit")?,
            status: format!("{:?}", a.status),
            payment_terms: a.payment_terms,
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CreditCheckResult {
    #[pyo3(get)]
    approved: bool,
    #[pyo3(get)]
    reason: Option<String>,
    #[pyo3(get)]
    available_credit: f64,
    #[pyo3(get)]
    requires_approval: bool,
}

impl TryFrom<stateset_core::CreditCheckResult> for CreditCheckResult {
    type Error = PyErr;

    fn try_from(r: stateset_core::CreditCheckResult) -> PyResult<Self> {
        Ok(Self {
            approved: r.approved,
            reason: r.reason.map(|r| format!("{:?}", r)),
            available_credit: to_f64_result(r.available_credit, "available credit")?,
            requires_approval: r.requires_approval,
        })
    }
}

// ============================================================================
// Credit Management API
// ============================================================================

#[pyclass]
pub struct CreditApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl CreditApi {
    #[pyo3(signature = (customer_id, credit_limit, payment_terms=None))]
    fn create_credit_account(
        &self,
        customer_id: String,
        credit_limit: f64,
        payment_terms: Option<String>,
    ) -> PyResult<CreditAccount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let account = commerce
            .credit()
            .create_credit_account(stateset_core::CreateCreditAccount {
                customer_id: uuid,
                credit_limit: decimal_from_f64(credit_limit, "credit_limit")?,
                payment_terms,
                ..Default::default()
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(account)
    }

    fn get_credit_account_by_customer(
        &self,
        customer_id: String,
    ) -> PyResult<Option<CreditAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let account = commerce
            .credit()
            .get_credit_account_by_customer(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(account)
    }

    fn check_credit(&self, customer_id: String, order_amount: f64) -> PyResult<CreditCheckResult> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let result = commerce
            .credit()
            .check_credit(uuid, decimal_from_f64(order_amount, "order_amount")?)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(result)
    }

    fn adjust_credit_limit(
        &self,
        customer_id: String,
        new_limit: f64,
        reason: String,
    ) -> PyResult<CreditAccount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid = customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let account = commerce
            .credit()
            .adjust_credit_limit(uuid, decimal_from_f64(new_limit, "new_limit")?, &reason)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(account)
    }

    fn get_over_limit_customers(&self) -> PyResult<Vec<CreditAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let accounts = commerce
            .credit()
            .get_over_limit_customers()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(accounts)
    }
}

// ============================================================================
// Backorder Management Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct Backorder {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    backorder_number: String,
    #[pyo3(get)]
    order_id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    quantity_ordered: f64,
    #[pyo3(get)]
    quantity_fulfilled: f64,
    #[pyo3(get)]
    quantity_remaining: f64,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    priority: String,
}

impl TryFrom<stateset_core::Backorder> for Backorder {
    type Error = PyErr;

    fn try_from(b: stateset_core::Backorder) -> PyResult<Self> {
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
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct BackorderSummary {
    #[pyo3(get)]
    total_backorders: i32,
    #[pyo3(get)]
    total_quantity: f64,
    #[pyo3(get)]
    critical_count: i32,
    #[pyo3(get)]
    overdue_count: i32,
}

impl TryFrom<stateset_core::BackorderSummary> for BackorderSummary {
    type Error = PyErr;

    fn try_from(s: stateset_core::BackorderSummary) -> PyResult<Self> {
        Ok(Self {
            total_backorders: s.total_backorders,
            total_quantity: to_f64_result(s.total_quantity, "backorder total quantity")?,
            critical_count: s.critical_count,
            overdue_count: s.overdue_count,
        })
    }
}

// ============================================================================
// Backorder Management API
// ============================================================================

#[pyclass]
pub struct BackorderApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl BackorderApi {
    #[pyo3(signature = (order_id, customer_id, sku, quantity, priority=None))]
    fn create_backorder(
        &self,
        order_id: String,
        customer_id: String,
        sku: String,
        quantity: f64,
        priority: Option<String>,
    ) -> PyResult<Backorder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let ord_uuid = order_id.parse().map_err(|_| PyValueError::new_err("Invalid order UUID"))?;
        let cust_uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let prio = priority.and_then(|p| match p.to_lowercase().as_str() {
            "low" => Some(stateset_core::BackorderPriority::Low),
            "normal" => Some(stateset_core::BackorderPriority::Normal),
            "high" => Some(stateset_core::BackorderPriority::High),
            "critical" => Some(stateset_core::BackorderPriority::Critical),
            _ => None,
        });
        let backorder = commerce
            .backorder()
            .create_backorder(stateset_core::CreateBackorder {
                order_id: ord_uuid,
                customer_id: cust_uuid,
                sku,
                quantity: decimal_from_f64(quantity, "quantity")?,
                priority: prio,
                order_line_id: None,
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: None,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(backorder)
    }

    fn get_backorder(&self, id: String) -> PyResult<Option<Backorder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .get_backorder(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(backorder)
    }

    fn list_backorders(&self) -> PyResult<Vec<Backorder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let backorders = commerce
            .backorder()
            .list_backorders(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(backorders)
    }

    fn fulfill_backorder(&self, id: String, quantity: f64) -> PyResult<Backorder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .fulfill_backorder(stateset_core::FulfillBackorder {
                backorder_id: uuid,
                quantity: decimal_from_f64(quantity, "quantity")?,
                source_type: stateset_core::FulfillmentSourceType::Inventory,
                source_id: None,
                notes: None,
                fulfilled_by: None,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(backorder)
    }

    fn cancel_backorder(&self, id: String) -> PyResult<Backorder> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let backorder = commerce
            .backorder()
            .cancel_backorder(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(backorder)
    }

    fn get_summary(&self) -> PyResult<BackorderSummary> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let summary = commerce
            .backorder()
            .get_summary()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(summary)
    }

    fn get_overdue_backorders(&self) -> PyResult<Vec<Backorder>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let backorders = commerce
            .backorder()
            .get_overdue_backorders()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(backorders)
    }
}

// ============================================================================
// General Ledger Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct GlAccount {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    account_number: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    account_type: String,
    #[pyo3(get)]
    current_balance: f64,
    #[pyo3(get)]
    status: String,
}

impl TryFrom<stateset_core::GlAccount> for GlAccount {
    type Error = PyErr;

    fn try_from(a: stateset_core::GlAccount) -> PyResult<Self> {
        Ok(Self {
            id: a.id.to_string(),
            account_number: a.account_number,
            name: a.name,
            account_type: format!("{:?}", a.account_type),
            current_balance: to_f64_result(a.current_balance, "account balance")?,
            status: format!("{:?}", a.status),
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct JournalEntry {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    entry_number: String,
    #[pyo3(get)]
    description: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    entry_date: String,
}

impl From<stateset_core::JournalEntry> for JournalEntry {
    fn from(e: stateset_core::JournalEntry) -> Self {
        Self {
            id: e.id.to_string(),
            entry_number: e.entry_number,
            description: e.description,
            status: format!("{:?}", e.status),
            entry_date: e.entry_date.to_string(),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct TrialBalance {
    #[pyo3(get)]
    total_debits: f64,
    #[pyo3(get)]
    total_credits: f64,
    #[pyo3(get)]
    is_balanced: bool,
}

impl TryFrom<stateset_core::TrialBalance> for TrialBalance {
    type Error = PyErr;

    fn try_from(t: stateset_core::TrialBalance) -> PyResult<Self> {
        Ok(Self {
            total_debits: to_f64_result(t.total_debits, "trial balance total debits")?,
            total_credits: to_f64_result(t.total_credits, "trial balance total credits")?,
            is_balanced: t.is_balanced,
        })
    }
}

// ============================================================================
// General Ledger API
// ============================================================================

#[pyclass]
pub struct GeneralLedgerApi {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl GeneralLedgerApi {
    #[pyo3(signature = (account_number, name, account_type, description=None))]
    fn create_account(
        &self,
        account_number: String,
        name: String,
        account_type: String,
        description: Option<String>,
    ) -> PyResult<GlAccount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let acct_type = match account_type.to_lowercase().as_str() {
            "asset" => stateset_core::AccountType::Asset,
            "liability" => stateset_core::AccountType::Liability,
            "equity" => stateset_core::AccountType::Equity,
            "revenue" => stateset_core::AccountType::Revenue,
            "expense" => stateset_core::AccountType::Expense,
            _ => stateset_core::AccountType::Asset,
        };
        let account = commerce
            .general_ledger()
            .create_account(stateset_core::CreateGlAccount {
                account_number,
                name,
                account_type: acct_type,
                description,
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: None,
                currency: None,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(account)
    }

    fn get_account(&self, id: String) -> PyResult<Option<GlAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let account = commerce
            .general_ledger()
            .get_account(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(account)
    }

    fn get_account_by_number(&self, account_number: String) -> PyResult<Option<GlAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let account = commerce
            .general_ledger()
            .get_account_by_number(&account_number)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_optional_output(account)
    }

    fn list_accounts(&self) -> PyResult<Vec<GlAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let accounts = commerce
            .general_ledger()
            .list_accounts(Default::default())
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(accounts)
    }

    fn get_journal_entry(&self, id: String) -> PyResult<Option<JournalEntry>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .get_journal_entry(uuid)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(entry.map(|e| e.into()))
    }

    fn post_journal_entry(&self, id: String, posted_by: String) -> PyResult<JournalEntry> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .post_journal_entry(uuid, &posted_by)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(entry.into())
    }

    #[pyo3(signature = (as_of_date=None))]
    fn get_trial_balance(&self, as_of_date: Option<String>) -> PyResult<TrialBalance> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let date = as_of_date
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let balance = commerce
            .general_ledger()
            .get_trial_balance(date)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_output(balance)
    }

    /// Initialize the standard chart of accounts.
    fn initialize_chart_of_accounts(&self) -> PyResult<Vec<GlAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let accounts = commerce
            .general_ledger()
            .initialize_chart_of_accounts()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        convert_outputs(accounts)
    }

    /// Create an accounting period. Dates are ISO strings (YYYY-MM-DD).
    fn create_period(
        &self,
        period_name: String,
        fiscal_year: i32,
        period_number: i32,
        start_date: String,
        end_date: String,
    ) -> PyResult<GlPeriod> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let period = commerce
            .general_ledger()
            .create_period(stateset_core::CreateGlPeriod {
                period_name,
                fiscal_year,
                period_number,
                start_date: parse_iso_date_py(&start_date, "start_date")?,
                end_date: parse_iso_date_py(&end_date, "end_date")?,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create period: {}", e)))?;
        Ok(period.into())
    }

    /// Open a period (transition from future to open).
    fn open_period(&self, id: String) -> PyResult<GlPeriod> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let period = commerce
            .general_ledger()
            .open_period(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to open period: {}", e)))?;
        Ok(period.into())
    }

    /// Revalue foreign-currency account balances at the as-of exchange rate.
    ///
    /// `as_of_date` is an ISO date (YYYY-MM-DD); `base_currency` defaults to
    /// the store's configured base currency.
    #[pyo3(signature = (as_of_date, base_currency=None))]
    fn revalue(
        &self,
        as_of_date: String,
        base_currency: Option<String>,
    ) -> PyResult<RevaluationResult> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| PyValueError::new_err("Invalid date format"))?;
        let base = base_currency
            .map(|s| {
                s.parse::<stateset_core::Currency>()
                    .map_err(|_| PyValueError::new_err("Invalid base currency code"))
            })
            .transpose()?;
        let result = commerce
            .general_ledger()
            .revalue(date, base)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to revalue: {}", e)))?;
        Ok(result.into())
    }

    /// Close the month: post scheduled depreciation, recognize revenue
    /// through period end, revalue foreign-currency balances, then run the
    /// period close (closing entries + close period).
    ///
    /// Pass `dry_run=True` to compute per-step counts and amounts without
    /// writing anything.
    #[pyo3(signature = (
        period_id,
        dry_run=None,
        skip_depreciation=None,
        skip_revenue_recognition=None,
        skip_fx_revaluation=None,
        skip_period_close=None,
        closed_by=None,
    ))]
    fn close_month(
        &self,
        period_id: String,
        dry_run: Option<bool>,
        skip_depreciation: Option<bool>,
        skip_revenue_recognition: Option<bool>,
        skip_fx_revaluation: Option<bool>,
        skip_period_close: Option<bool>,
        closed_by: Option<String>,
    ) -> PyResult<CloseMonthReport> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            period_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let report = commerce
            .general_ledger()
            .close_month(
                uuid,
                stateset_core::CloseMonthOptions {
                    dry_run: dry_run.unwrap_or(false),
                    skip_depreciation: skip_depreciation.unwrap_or(false),
                    skip_revenue_recognition: skip_revenue_recognition.unwrap_or(false),
                    skip_fx_revaluation: skip_fx_revaluation.unwrap_or(false),
                    skip_period_close: skip_period_close.unwrap_or(false),
                    closed_by,
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to close month: {}", e)))?;
        Ok(report.into())
    }
}

// ============================================================================
// Vector Search Types
// ============================================================================

/// Vector search API for semantic similarity search.
///
/// Uses OpenAI text-embedding-3-small for generating embeddings.
#[pyclass]
pub struct VectorSearch {
    commerce: Arc<Mutex<RustCommerce>>,
    api_key: String,
}

/// Product search result with similarity score.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ProductSearchResult {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub description: String,
    #[pyo3(get)]
    pub distance: f64,
    #[pyo3(get)]
    pub score: f64,
}

/// Customer search result with similarity score.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CustomerSearchResult {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub email: String,
    #[pyo3(get)]
    pub distance: f64,
    #[pyo3(get)]
    pub score: f64,
}

/// Embedding statistics.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct EmbeddingStats {
    #[pyo3(get)]
    pub product_count: u64,
    #[pyo3(get)]
    pub customer_count: u64,
    #[pyo3(get)]
    pub order_count: u64,
    #[pyo3(get)]
    pub inventory_count: u64,
    #[pyo3(get)]
    pub total_count: u64,
    #[pyo3(get)]
    pub model: String,
    #[pyo3(get)]
    pub dimensions: u32,
}

#[pymethods]
impl VectorSearch {
    /// Search products using natural language query.
    ///
    /// Args:
    ///     query: Natural language search query (e.g., "wireless bluetooth headphones")
    ///     limit: Maximum number of results to return (default: 10)
    ///
    /// Returns:
    ///     List of ProductSearchResult sorted by relevance
    #[pyo3(signature = (query, limit=None))]
    fn search_products(
        &self,
        query: String,
        limit: Option<usize>,
    ) -> PyResult<Vec<ProductSearchResult>> {
        let vector = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?
        };
        let results = vector
            .search_products(&query, limit.unwrap_or(10))
            .map_err(|e| PyRuntimeError::new_err(format!("Search failed: {}", e)))?;

        Ok(results
            .into_iter()
            .map(|r| ProductSearchResult {
                id: r.entity.id.to_string(),
                name: r.entity.name.clone(),
                description: r.entity.description.clone(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Search customers using natural language query.
    ///
    /// Args:
    ///     query: Natural language search query (e.g., "enterprise customers in tech")
    ///     limit: Maximum number of results to return (default: 10)
    ///
    /// Returns:
    ///     List of CustomerSearchResult sorted by relevance
    #[pyo3(signature = (query, limit=None))]
    fn search_customers(
        &self,
        query: String,
        limit: Option<usize>,
    ) -> PyResult<Vec<CustomerSearchResult>> {
        let vector = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?
        };
        let results = vector
            .search_customers(&query, limit.unwrap_or(10))
            .map_err(|e| PyRuntimeError::new_err(format!("Search failed: {}", e)))?;

        Ok(results
            .into_iter()
            .map(|r| CustomerSearchResult {
                id: r.entity.id.to_string(),
                name: format!("{} {}", r.entity.first_name, r.entity.last_name),
                email: r.entity.email.clone(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Index a product for vector search.
    ///
    /// Args:
    ///     product_id: UUID of the product to index
    fn index_product(&self, product_id: String) -> PyResult<()> {
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let (product, vector) = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            let product = commerce
                .products()
                .get(uuid.into())
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get product: {}", e)))?
                .ok_or_else(|| PyValueError::new_err("Product not found"))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?;

            (product, vector)
        };
        vector
            .index_product(&product)
            .map_err(|e| PyRuntimeError::new_err(format!("Indexing failed: {}", e)))?;

        Ok(())
    }

    /// Index a customer for vector search.
    ///
    /// Args:
    ///     customer_id: UUID of the customer to index
    fn index_customer(&self, customer_id: String) -> PyResult<()> {
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;

        let (customer, vector) = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            let customer = commerce
                .customers()
                .get(uuid.into())
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to get customer: {}", e)))?
                .ok_or_else(|| PyValueError::new_err("Customer not found"))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?;

            (customer, vector)
        };
        vector
            .index_customer(&customer)
            .map_err(|e| PyRuntimeError::new_err(format!("Indexing failed: {}", e)))?;

        Ok(())
    }

    /// Index all products for vector search.
    ///
    /// Returns:
    ///     Number of products indexed
    fn index_all_products(&self) -> PyResult<u64> {
        let (products, vector) = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            let products = commerce
                .products()
                .list(Default::default())
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to list products: {}", e)))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?;

            (products, vector)
        };
        let count = vector
            .index_products(&products)
            .map_err(|e| PyRuntimeError::new_err(format!("Indexing failed: {}", e)))?;

        Ok(count as u64)
    }

    /// Index all customers for vector search.
    ///
    /// Returns:
    ///     Number of customers indexed
    fn index_all_customers(&self) -> PyResult<u64> {
        let (customers, vector) = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            let customers = commerce
                .customers()
                .list(Default::default())
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to list customers: {}", e)))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?;

            (customers, vector)
        };
        let count = vector
            .index_customers(&customers)
            .map_err(|e| PyRuntimeError::new_err(format!("Indexing failed: {}", e)))?;

        Ok(count as u64)
    }

    /// Get embedding statistics.
    ///
    /// Returns:
    ///     EmbeddingStats with counts by entity type
    fn stats(&self) -> PyResult<EmbeddingStats> {
        let vector = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?
        };
        let stats =
            vector.stats().map_err(|e| PyRuntimeError::new_err(format!("Stats failed: {}", e)))?;

        let product_count = *stats.counts.get(&stateset_core::EntityType::Product).unwrap_or(&0);
        let customer_count = *stats.counts.get(&stateset_core::EntityType::Customer).unwrap_or(&0);
        let order_count = *stats.counts.get(&stateset_core::EntityType::Order).unwrap_or(&0);
        let inventory_count =
            *stats.counts.get(&stateset_core::EntityType::InventoryItem).unwrap_or(&0);

        Ok(EmbeddingStats {
            product_count,
            customer_count,
            order_count,
            inventory_count,
            total_count: product_count + customer_count + order_count + inventory_count,
            model: stats.model,
            dimensions: stats.dimensions as u32,
        })
    }

    /// Clear all embeddings for a specific entity type.
    ///
    /// Args:
    ///     entity_type: One of "products", "customers", "orders", "inventory"
    ///
    /// Returns:
    ///     Number of embeddings cleared
    fn clear(&self, entity_type: String) -> PyResult<u64> {
        let et: stateset_core::EntityType =
            entity_type.parse().map_err(|e: String| PyValueError::new_err(e))?;

        let vector = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?
        };
        let count = vector
            .clear(et)
            .map_err(|e| PyRuntimeError::new_err(format!("Clear failed: {}", e)))?;

        Ok(count)
    }

    /// Clear all embeddings.
    ///
    /// Returns:
    ///     Total number of embeddings cleared
    fn clear_all(&self) -> PyResult<u64> {
        let vector = {
            let commerce = self
                .commerce
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;

            commerce
                .vector(self.api_key.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Vector init failed: {}", e)))?
        };
        let count = vector
            .clear_all()
            .map_err(|e| PyRuntimeError::new_err(format!("Clear failed: {}", e)))?;

        Ok(count)
    }
}

// ============================================================================
// Sync Types
// ============================================================================

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncEvent {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    sequence: u64,
    #[pyo3(get)]
    sequence_authority: String,
    #[pyo3(get)]
    canonical_sequence: Option<u64>,
    #[pyo3(get)]
    local_sequence: Option<u64>,
    #[pyo3(get)]
    event_type: String,
    #[pyo3(get)]
    entity_type: String,
    #[pyo3(get)]
    entity_id: String,
    #[pyo3(get)]
    payload_json: String,
    #[pyo3(get)]
    hash: String,
    #[pyo3(get)]
    signature: Option<String>,
    #[pyo3(get)]
    command_id: Option<String>,
    #[pyo3(get)]
    base_version: Option<u64>,
    #[pyo3(get)]
    source_agent_id: Option<String>,
    #[pyo3(get)]
    agent_key_id: Option<u32>,
    #[pyo3(get)]
    timestamp: String,
}

#[pymethods]
impl SyncEvent {
    fn __repr__(&self) -> String {
        format!(
            "SyncEvent(event_type='{}', entity_type='{}', entity_id='{}', sequence={})",
            self.event_type, self.entity_type, self.entity_id, self.sequence
        )
    }
}

impl From<&RustSyncEvent> for SyncEvent {
    fn from(event: &RustSyncEvent) -> Self {
        Self {
            id: event.id.to_string(),
            sequence: event.sequence,
            sequence_authority: sync_sequence_authority_name(event).to_string(),
            canonical_sequence: event.canonical_sequence(),
            local_sequence: event.local_sequence(),
            event_type: event.event_type.clone(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            payload_json: json_value_to_string(&event.payload),
            hash: event.hash.clone(),
            signature: event.signature.clone(),
            command_id: event.command_id.clone(),
            base_version: event.base_version,
            source_agent_id: event.source_agent_id.clone(),
            agent_key_id: event.agent_key_id,
            timestamp: event.timestamp.to_rfc3339(),
        }
    }
}

impl From<RustSyncEvent> for SyncEvent {
    fn from(event: RustSyncEvent) -> Self {
        Self::from(&event)
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncStatus {
    #[pyo3(get)]
    initialized: bool,
    #[pyo3(get)]
    local_head: u64,
    #[pyo3(get)]
    remote_head: u64,
    #[pyo3(get)]
    remote_state_root: Option<String>,
    #[pyo3(get)]
    last_commitment_id: Option<String>,
    #[pyo3(get)]
    remote_cursor: u64,
    #[pyo3(get)]
    next_pull_cursor: Option<u64>,
    #[pyo3(get)]
    last_acknowledged_remote_sequence: Option<u64>,
    #[pyo3(get)]
    pending: usize,
    #[pyo3(get)]
    dead_letters: usize,
    #[pyo3(get)]
    retained_confirmations: usize,
    #[pyo3(get)]
    lag: u64,
    #[pyo3(get)]
    caught_up: bool,
    #[pyo3(get)]
    last_push: Option<String>,
    #[pyo3(get)]
    last_pull: Option<String>,
    #[pyo3(get)]
    buffered_events: usize,
}

#[pymethods]
impl SyncStatus {
    fn __repr__(&self) -> String {
        format!(
            "SyncStatus(local_head={}, remote_head={}, pending={}, lag={})",
            self.local_head, self.remote_head, self.pending, self.lag
        )
    }
}

impl From<stateset_sdk::sync::SyncStatus> for SyncStatus {
    fn from(status: stateset_sdk::sync::SyncStatus) -> Self {
        Self {
            initialized: status.initialized,
            local_head: status.local_head,
            remote_head: status.remote_head,
            remote_state_root: status.remote_state_root,
            last_commitment_id: status.last_commitment_id,
            remote_cursor: status.remote_cursor,
            next_pull_cursor: status.next_pull_cursor,
            last_acknowledged_remote_sequence: status.last_acknowledged_remote_sequence,
            pending: status.pending,
            dead_letters: status.dead_letters,
            retained_confirmations: status.retained_confirmations,
            lag: status.lag,
            caught_up: status.caught_up,
            last_push: status.last_push.map(|value| value.to_rfc3339()),
            last_pull: status.last_pull.map(|value| value.to_rfc3339()),
            buffered_events: status.buffered_events,
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncRemoteHead {
    #[pyo3(get)]
    remote_head: u64,
    #[pyo3(get)]
    state_root: Option<String>,
    #[pyo3(get)]
    last_commitment_id: Option<String>,
}

impl From<stateset_sdk::sync::RemoteHead> for SyncRemoteHead {
    fn from(head: stateset_sdk::sync::RemoteHead) -> Self {
        Self {
            remote_head: head.remote_head,
            state_root: head.state_root,
            last_commitment_id: head.last_commitment_id,
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncAcknowledgement {
    #[pyo3(get)]
    event_id: String,
    #[pyo3(get)]
    remote_sequence: u64,
    #[pyo3(get)]
    receipt: Option<String>,
}

impl From<&stateset_sdk::sync::PushAcknowledgement> for SyncAcknowledgement {
    fn from(acknowledgement: &stateset_sdk::sync::PushAcknowledgement) -> Self {
        Self {
            event_id: acknowledgement.event_id.to_string(),
            remote_sequence: acknowledgement.remote_sequence,
            receipt: acknowledgement.receipt.clone(),
        }
    }
}

impl From<stateset_sdk::sync::PushAcknowledgement> for SyncAcknowledgement {
    fn from(acknowledgement: stateset_sdk::sync::PushAcknowledgement) -> Self {
        Self::from(&acknowledgement)
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncRejection {
    #[pyo3(get)]
    event_id: String,
    #[pyo3(get)]
    code: Option<String>,
    #[pyo3(get)]
    reason: Option<String>,
    #[pyo3(get)]
    retryable: Option<bool>,
}

impl From<&stateset_sdk::sync::PushRejection> for SyncRejection {
    fn from(rejection: &stateset_sdk::sync::PushRejection) -> Self {
        Self {
            event_id: rejection.event_id.to_string(),
            code: rejection.code.clone(),
            reason: rejection.reason.clone(),
            retryable: rejection.retryable,
        }
    }
}

impl From<stateset_sdk::sync::PushRejection> for SyncRejection {
    fn from(rejection: stateset_sdk::sync::PushRejection) -> Self {
        Self::from(&rejection)
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncPushResult {
    #[pyo3(get)]
    accepted: usize,
    #[pyo3(get)]
    remote_head: u64,
    #[pyo3(get)]
    acknowledged_head: Option<u64>,
    #[pyo3(get)]
    acknowledgements: Vec<SyncAcknowledgement>,
    #[pyo3(get)]
    rejections: Vec<SyncRejection>,
}

impl From<stateset_sdk::sync::PushResult> for SyncPushResult {
    fn from(result: stateset_sdk::sync::PushResult) -> Self {
        let acknowledged_head = result.acknowledged_head();
        Self {
            accepted: result.accepted,
            remote_head: result.remote_head,
            acknowledged_head,
            acknowledgements: result
                .acknowledgements
                .into_iter()
                .map(SyncAcknowledgement::from)
                .collect(),
            rejections: result.rejections.into_iter().map(SyncRejection::from).collect(),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncConfirmation {
    #[pyo3(get)]
    event_id: String,
    #[pyo3(get)]
    command_id: Option<String>,
    #[pyo3(get)]
    event_type: String,
    #[pyo3(get)]
    entity_type: String,
    #[pyo3(get)]
    entity_id: String,
    #[pyo3(get)]
    local_sequence: Option<u64>,
    #[pyo3(get)]
    remote_sequence: u64,
    #[pyo3(get)]
    hash: String,
    #[pyo3(get)]
    receipt: Option<String>,
    #[pyo3(get)]
    confirmed_at: String,
}

impl From<&stateset_sdk::sync::PushConfirmation> for SyncConfirmation {
    fn from(confirmation: &stateset_sdk::sync::PushConfirmation) -> Self {
        Self {
            event_id: confirmation.event_id.to_string(),
            command_id: confirmation.command_id.clone(),
            event_type: confirmation.event_type.clone(),
            entity_type: confirmation.entity_type.clone(),
            entity_id: confirmation.entity_id.clone(),
            local_sequence: confirmation.local_sequence,
            remote_sequence: confirmation.remote_sequence,
            hash: confirmation.hash.clone(),
            receipt: confirmation.receipt.clone(),
            confirmed_at: confirmation.confirmed_at.to_rfc3339(),
        }
    }
}

impl From<stateset_sdk::sync::PushConfirmation> for SyncConfirmation {
    fn from(confirmation: stateset_sdk::sync::PushConfirmation) -> Self {
        Self::from(&confirmation)
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncDeadLetter {
    #[pyo3(get)]
    event: SyncEvent,
    #[pyo3(get)]
    rejection: SyncRejection,
    #[pyo3(get)]
    rejected_at: String,
}

impl From<&stateset_sdk::sync::DeadLetter> for SyncDeadLetter {
    fn from(dead_letter: &stateset_sdk::sync::DeadLetter) -> Self {
        Self {
            event: SyncEvent::from(&dead_letter.event),
            rejection: SyncRejection::from(&dead_letter.rejection),
            rejected_at: dead_letter.rejected_at.to_rfc3339(),
        }
    }
}

impl From<stateset_sdk::sync::DeadLetter> for SyncDeadLetter {
    fn from(dead_letter: stateset_sdk::sync::DeadLetter) -> Self {
        Self::from(&dead_letter)
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncPullResult {
    #[pyo3(get)]
    events: Vec<SyncEvent>,
    #[pyo3(get)]
    remote_head: u64,
    #[pyo3(get)]
    has_more: bool,
}

impl From<stateset_sdk::sync::PullResult> for SyncPullResult {
    fn from(result: stateset_sdk::sync::PullResult) -> Self {
        Self {
            events: result.events.into_iter().map(SyncEvent::from).collect(),
            remote_head: result.remote_head,
            has_more: result.has_more,
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncSnapshot {
    #[pyo3(get)]
    status: SyncStatus,
    #[pyo3(get)]
    confirmations: Vec<SyncConfirmation>,
    #[pyo3(get)]
    dead_letters: Vec<SyncDeadLetter>,
    #[pyo3(get)]
    buffered_events: Vec<SyncEvent>,
}

impl From<stateset_sdk::SyncRuntimeSnapshot> for SyncSnapshot {
    fn from(snapshot: stateset_sdk::SyncRuntimeSnapshot) -> Self {
        Self {
            status: SyncStatus::from(snapshot.status),
            confirmations: snapshot.confirmations.into_iter().map(SyncConfirmation::from).collect(),
            dead_letters: snapshot.dead_letters.into_iter().map(SyncDeadLetter::from).collect(),
            buffered_events: snapshot.buffered_events.into_iter().map(SyncEvent::from).collect(),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SyncFullSyncResult {
    #[pyo3(get)]
    push: SyncPushResult,
    #[pyo3(get)]
    pull: SyncPullResult,
}

// ============================================================================
// Sync Runtime
// ============================================================================

/// Sync runtime for pushing local events to a remote sequencer and pulling
/// canonical remote events back into the local buffer.
///
/// Construct from a serialized `SyncRuntimeConfig` JSON document, or use
/// `from_file` / `from_env` for file and environment-backed config loading.
#[pyclass]
pub struct SyncRuntime {
    inner: Arc<Mutex<RustSyncRuntime>>,
}

impl SyncRuntime {
    fn from_runtime_config(config: RustSyncRuntimeConfig) -> PyResult<Self> {
        let runtime = RustSyncRuntime::from_runtime_config(config)
            .map_err(|error| sync_runtime_error("Failed to initialize sync runtime", error))?;
        Ok(Self { inner: Arc::new(Mutex::new(runtime)) })
    }

    fn with_runtime<T>(&self, f: impl FnOnce(&RustSyncRuntime) -> PyResult<T>) -> PyResult<T> {
        let runtime = self
            .inner
            .lock()
            .map_err(|error| PyRuntimeError::new_err(format!("Lock error: {error}")))?;
        f(&runtime)
    }

    fn with_runtime_mut<T>(
        &self,
        f: impl FnOnce(&mut RustSyncRuntime) -> PyResult<T>,
    ) -> PyResult<T> {
        let mut runtime = self
            .inner
            .lock()
            .map_err(|error| PyRuntimeError::new_err(format!("Lock error: {error}")))?;
        f(&mut runtime)
    }

    fn status_snapshot(&self) -> PyResult<stateset_sdk::sync::SyncStatus> {
        self.with_runtime(|runtime| Ok(runtime.status()))
    }

    fn run_async<T, F>(&self, py: Python<'_>, context: &str, f: F) -> PyResult<T>
    where
        F: FnOnce(
                &mut RustSyncRuntime,
                &tokio::runtime::Runtime,
            ) -> Result<T, stateset_sdk::sync::SyncError>
            + Ungil
            + Send,
        T: Ungil + Send,
    {
        let context = context.to_string();
        py.detach(move || {
            let mut runtime = self.inner.lock().map_err(|error| format!("Lock error: {error}"))?;
            let executor = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("Failed to initialize async runtime: {error}"))?;
            f(&mut runtime, &executor).map_err(|error| format!("{context}: {error}"))
        })
        .map_err(PyRuntimeError::new_err)
    }
}

#[pymethods]
impl SyncRuntime {
    /// Create a sync runtime from a JSON-serialized `SyncRuntimeConfig`.
    #[new]
    fn new(config_json: String) -> PyResult<Self> {
        let config = RustSyncRuntimeConfig::from_json_str(&config_json)
            .map_err(|error| sync_runtime_error("Failed to parse sync runtime config", error))?;
        Self::from_runtime_config(config)
    }

    /// Create a sync runtime from a JSON config file.
    #[staticmethod]
    fn from_file(path: String) -> PyResult<Self> {
        let config = RustSyncRuntimeConfig::from_file(&path)
            .map_err(|error| sync_runtime_error("Failed to load sync runtime config", error))?;
        Self::from_runtime_config(config)
    }

    /// Create a sync runtime from environment variables.
    ///
    /// Uses `STATESET_SYNC_` by default, or a custom prefix when provided.
    #[staticmethod]
    #[pyo3(signature = (prefix=None))]
    fn from_env(prefix: Option<String>) -> PyResult<Self> {
        let config = if let Some(prefix) = prefix {
            RustSyncRuntimeConfig::from_env_prefixed(&prefix)
        } else {
            RustSyncRuntimeConfig::from_env()
        }
        .map_err(|error| sync_runtime_error("Failed to load sync runtime config", error))?;
        Self::from_runtime_config(config)
    }

    /// Record a new local event from basic Python-friendly inputs.
    #[pyo3(signature = (event_type, entity_type, entity_id, payload_json, command_id=None, base_version=None, source_agent_id=None, agent_key_id=None, signature=None))]
    fn record(
        &self,
        event_type: String,
        entity_type: String,
        entity_id: String,
        payload_json: String,
        command_id: Option<String>,
        base_version: Option<u64>,
        source_agent_id: Option<String>,
        agent_key_id: Option<u32>,
        signature: Option<String>,
    ) -> PyResult<u64> {
        let payload = parse_json_value(&payload_json, "payload")?;
        let mut event = RustSyncEvent::new(event_type, entity_type, entity_id, payload);
        if let Some(command_id) = command_id {
            event = event.with_command_id(command_id);
        }
        if let Some(base_version) = base_version {
            event = event.with_base_version(base_version);
        }
        if let Some(source_agent_id) = source_agent_id {
            event = event.with_source_agent_id(source_agent_id);
        }
        if let Some(agent_key_id) = agent_key_id {
            event = event.with_agent_key_id(agent_key_id);
        }
        if let Some(signature) = signature {
            event = event.with_signature(signature);
        }
        self.with_runtime_mut(|runtime| {
            runtime
                .record(event)
                .map_err(|error| sync_runtime_error("Failed to record sync event", error))
        })
    }

    /// Record a full JSON-serialized `SyncEvent`.
    fn record_event_json(&self, event_json: String) -> PyResult<u64> {
        let event: RustSyncEvent = serde_json::from_str(&event_json)
            .map_err(|error| PyValueError::new_err(format!("Invalid sync event JSON: {error}")))?;
        self.with_runtime_mut(|runtime| {
            runtime
                .record(event)
                .map_err(|error| sync_runtime_error("Failed to record sync event", error))
        })
    }

    fn status(&self) -> PyResult<SyncStatus> {
        Ok(SyncStatus::from(self.status_snapshot()?))
    }

    fn snapshot(&self) -> PyResult<SyncSnapshot> {
        self.with_runtime(|runtime| Ok(SyncSnapshot::from(runtime.snapshot())))
    }

    fn confirmations(&self) -> PyResult<Vec<SyncConfirmation>> {
        self.with_runtime(|runtime| {
            Ok(runtime.confirmations().iter().map(SyncConfirmation::from).collect())
        })
    }

    fn confirmation_for_event(&self, event_id: String) -> PyResult<Option<SyncConfirmation>> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime(|runtime| {
            Ok(runtime.confirmation_for_event(event_id).map(SyncConfirmation::from))
        })
    }

    fn drain_confirmations(&self) -> PyResult<Vec<SyncConfirmation>> {
        self.with_runtime_mut(|runtime| {
            let confirmations = runtime
                .drain_confirmations()
                .map_err(|error| sync_runtime_error("Failed to drain confirmations", error))?;
            Ok(confirmations.into_iter().map(SyncConfirmation::from).collect())
        })
    }

    fn dead_letters(&self) -> PyResult<Vec<SyncDeadLetter>> {
        self.with_runtime(|runtime| {
            Ok(runtime.dead_letters().iter().map(SyncDeadLetter::from).collect())
        })
    }

    fn dead_letter_for_event(&self, event_id: String) -> PyResult<Option<SyncDeadLetter>> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime(|runtime| {
            Ok(runtime.dead_letter_for_event(event_id).map(SyncDeadLetter::from))
        })
    }

    fn discard_dead_letter(&self, event_id: String) -> PyResult<SyncDeadLetter> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime_mut(|runtime| {
            let dead_letter = runtime
                .discard_dead_letter(event_id)
                .map_err(|error| sync_runtime_error("Failed to discard dead letter", error))?;
            Ok(SyncDeadLetter::from(dead_letter))
        })
    }

    fn drain_dead_letters(&self) -> PyResult<Vec<SyncDeadLetter>> {
        self.with_runtime_mut(|runtime| {
            let dead_letters = runtime
                .drain_dead_letters()
                .map_err(|error| sync_runtime_error("Failed to drain dead letters", error))?;
            Ok(dead_letters.into_iter().map(SyncDeadLetter::from).collect())
        })
    }

    fn buffered_events(&self) -> PyResult<Vec<SyncEvent>> {
        self.with_runtime(|runtime| {
            Ok(runtime.engine().buffered_events().into_iter().map(SyncEvent::from).collect())
        })
    }

    fn drain_buffer(&self) -> PyResult<Vec<SyncEvent>> {
        self.with_runtime_mut(|runtime| {
            Ok(runtime.drain_buffer().into_iter().map(SyncEvent::from).collect())
        })
    }

    fn refresh_remote_head(&self, py: Python<'_>) -> PyResult<SyncRemoteHead> {
        let head = self.run_async(py, "Failed to refresh remote head", |runtime, executor| {
            executor.block_on(runtime.refresh_remote_head())
        })?;
        Ok(SyncRemoteHead::from(head))
    }

    fn push(&self, py: Python<'_>) -> PyResult<SyncPushResult> {
        let result = self.run_async(py, "Failed to push sync events", |runtime, executor| {
            executor.block_on(runtime.push())
        })?;
        Ok(SyncPushResult::from(result))
    }

    fn pull(&self, py: Python<'_>) -> PyResult<SyncPullResult> {
        let result = self.run_async(py, "Failed to pull sync events", |runtime, executor| {
            executor.block_on(runtime.pull())
        })?;
        Ok(SyncPullResult::from(result))
    }

    fn full_sync(&self, py: Python<'_>) -> PyResult<SyncFullSyncResult> {
        let (push, pull) =
            self.run_async(py, "Failed to perform full sync", |runtime, executor| {
                executor.block_on(runtime.full_sync())
            })?;
        Ok(SyncFullSyncResult {
            push: SyncPushResult::from(push),
            pull: SyncPullResult::from(pull),
        })
    }

    /// Serialize the current sync status as JSON.
    fn status_json(&self) -> PyResult<String> {
        serialize_json(&self.status_snapshot()?, "sync status")
    }

    /// Serialize the full runtime snapshot as JSON.
    #[pyo3(signature = (pretty=false))]
    fn snapshot_json(&self, pretty: bool) -> PyResult<String> {
        let snapshot = self.with_runtime(|runtime| Ok(runtime.snapshot()))?;
        if pretty {
            serialize_json_pretty(&snapshot, "sync snapshot")
        } else {
            serialize_json(&snapshot, "sync snapshot")
        }
    }

    /// Serialize retained confirmations as JSON.
    fn confirmations_json(&self) -> PyResult<String> {
        self.with_runtime(|runtime| serialize_json(&runtime.confirmations(), "confirmations"))
    }

    /// Serialize a retained confirmation for one event id as JSON.
    fn confirmation_for_event_json(&self, event_id: String) -> PyResult<String> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime(|runtime| {
            serialize_json(&runtime.confirmation_for_event(event_id), "confirmation lookup")
        })
    }

    /// Drain retained confirmations and serialize them as JSON.
    fn drain_confirmations_json(&self) -> PyResult<String> {
        self.with_runtime_mut(|runtime| {
            let confirmations = runtime
                .drain_confirmations()
                .map_err(|error| sync_runtime_error("Failed to drain confirmations", error))?;
            serialize_json(&confirmations, "drained confirmations")
        })
    }

    /// Serialize retained dead letters as JSON.
    fn dead_letters_json(&self) -> PyResult<String> {
        self.with_runtime(|runtime| serialize_json(&runtime.dead_letters(), "dead letters"))
    }

    /// Serialize a retained dead letter for one event id as JSON.
    fn dead_letter_for_event_json(&self, event_id: String) -> PyResult<String> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime(|runtime| {
            serialize_json(&runtime.dead_letter_for_event(event_id), "dead-letter lookup")
        })
    }

    /// Requeue a dead-lettered event back into the local outbox.
    fn requeue_dead_letter(&self, event_id: String) -> PyResult<u64> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime_mut(|runtime| {
            runtime
                .requeue_dead_letter(event_id)
                .map_err(|error| sync_runtime_error("Failed to requeue dead letter", error))
        })
    }

    /// Discard a dead-lettered event and serialize the removed record as JSON.
    fn discard_dead_letter_json(&self, event_id: String) -> PyResult<String> {
        let event_id = parse_uuid_str(&event_id, "event_id")?;
        self.with_runtime_mut(|runtime| {
            let dead_letter = runtime
                .discard_dead_letter(event_id)
                .map_err(|error| sync_runtime_error("Failed to discard dead letter", error))?;
            serialize_json(&dead_letter, "discarded dead letter")
        })
    }

    /// Drain retained dead letters and serialize them as JSON.
    fn drain_dead_letters_json(&self) -> PyResult<String> {
        self.with_runtime_mut(|runtime| {
            let dead_letters = runtime
                .drain_dead_letters()
                .map_err(|error| sync_runtime_error("Failed to drain dead letters", error))?;
            serialize_json(&dead_letters, "drained dead letters")
        })
    }

    /// Serialize buffered pulled events as JSON.
    fn buffered_events_json(&self) -> PyResult<String> {
        self.with_runtime(|runtime| {
            serialize_json(&runtime.engine().buffered_events(), "buffered events")
        })
    }

    /// Drain buffered pulled events and serialize them as JSON.
    fn drain_buffer_json(&self) -> PyResult<String> {
        self.with_runtime_mut(|runtime| {
            let events = runtime.drain_buffer();
            serialize_json(&events, "drained buffered events")
        })
    }

    /// Probe the remote sequencer health endpoint.
    fn healthcheck(&self, py: Python<'_>) -> PyResult<bool> {
        self.run_async(py, "Healthcheck failed", |runtime, executor| {
            executor.block_on(runtime.healthcheck()).map(|_| true)
        })
    }

    /// Refresh the known remote head and serialize it as JSON.
    fn refresh_remote_head_json(&self, py: Python<'_>) -> PyResult<String> {
        let head = self.run_async(py, "Failed to refresh remote head", |runtime, executor| {
            executor.block_on(runtime.refresh_remote_head())
        })?;
        serialize_json(&head, "remote head")
    }

    /// Push pending local events and serialize the result as JSON.
    fn push_json(&self, py: Python<'_>) -> PyResult<String> {
        let result = self.run_async(py, "Failed to push sync events", |runtime, executor| {
            executor.block_on(runtime.push())
        })?;
        serialize_json(&result, "push result")
    }

    /// Pull remote events and serialize the result as JSON.
    fn pull_json(&self, py: Python<'_>) -> PyResult<String> {
        let result = self.run_async(py, "Failed to pull sync events", |runtime, executor| {
            executor.block_on(runtime.pull())
        })?;
        serialize_json(&result, "pull result")
    }

    /// Perform a push followed by a pull and serialize the combined result as JSON.
    fn full_sync_json(&self, py: Python<'_>) -> PyResult<String> {
        let (push, pull) =
            self.run_async(py, "Failed to perform full sync", |runtime, executor| {
                executor.block_on(runtime.full_sync())
            })?;
        serialize_json(&serde_json::json!({ "push": push, "pull": pull }), "full sync result")
    }

    /// Whether the runtime is initialized.
    #[getter]
    fn initialized(&self) -> PyResult<bool> {
        Ok(self.status_snapshot()?.initialized)
    }

    /// Whether the runtime has no pending events and has observed the known remote head.
    #[getter]
    fn caught_up(&self) -> PyResult<bool> {
        Ok(self.status_snapshot()?.caught_up)
    }

    /// Current local outbox head sequence.
    #[getter]
    fn local_head(&self) -> PyResult<u64> {
        Ok(self.status_snapshot()?.local_head)
    }

    /// Current known remote head sequence.
    #[getter]
    fn remote_head(&self) -> PyResult<u64> {
        Ok(self.status_snapshot()?.remote_head)
    }

    /// Current observed canonical remote cursor.
    #[getter]
    fn remote_cursor(&self) -> PyResult<u64> {
        Ok(self.status_snapshot()?.remote_cursor)
    }

    /// Current next-pull continuation cursor, if present.
    #[getter]
    fn next_pull_cursor(&self) -> PyResult<Option<u64>> {
        Ok(self.status_snapshot()?.next_pull_cursor)
    }

    /// Current known remote state root, if present.
    #[getter]
    fn remote_state_root(&self) -> PyResult<Option<String>> {
        Ok(self.status_snapshot()?.remote_state_root)
    }

    /// Current known remote commitment id, if present.
    #[getter]
    fn last_commitment_id(&self) -> PyResult<Option<String>> {
        Ok(self.status_snapshot()?.last_commitment_id)
    }

    /// Latest canonical remote sequence acknowledged for a local push, if present.
    #[getter]
    fn last_acknowledged_remote_sequence(&self) -> PyResult<Option<u64>> {
        Ok(self.status_snapshot()?.last_acknowledged_remote_sequence)
    }

    /// Canonical lag between the known remote head and the local pull cursor.
    #[getter]
    fn lag(&self) -> PyResult<u64> {
        Ok(self.status_snapshot()?.lag)
    }

    /// Number of pending local events.
    #[getter]
    fn pending_count(&self) -> PyResult<usize> {
        self.with_runtime(|runtime| Ok(runtime.pending_count()))
    }

    /// Number of retained confirmations.
    #[getter]
    fn confirmation_count(&self) -> PyResult<usize> {
        self.with_runtime(|runtime| Ok(runtime.confirmation_count()))
    }

    /// Number of retained dead letters.
    #[getter]
    fn dead_letter_count(&self) -> PyResult<usize> {
        self.with_runtime(|runtime| Ok(runtime.dead_letter_count()))
    }

    /// Number of buffered pulled events.
    #[getter]
    fn buffered_count(&self) -> PyResult<usize> {
        self.with_runtime(|runtime| Ok(runtime.buffered_count()))
    }
}

// ============================================================================
// Cross-binding crypto primitives
// ============================================================================
//
// Thin Python wrappers over the `stateset-crypto` Rust crate so the Python
// binding can verify the language-neutral test corpus at
// `bindings/test-vectors/v1.json`. Counterpart in Rust:
// `crates/stateset-crypto/tests/cross_binding_vectors.rs`. Counterpart in
// Node: `bindings/node/test/cross-binding-vectors.js`.

/// RFC 8785 JCS canonical-form bytes for a JSON string.
///
/// Returns the canonical UTF-8 byte sequence (callers SHA-256 it themselves
/// when comparing against ground truth).
#[pyfunction]
fn jcs_canonicalize(json_str: &str) -> PyResult<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| PyValueError::new_err(format!("invalid JSON: {e}")))?;
    ::stateset_crypto::canonicalize::canonicalize_json_bytes(&value)
        .map_err(|e| PyRuntimeError::new_err(format!("canonicalize: {e}")))
}

/// VES v1.0 payload-plain hash.
///
/// Equivalent to `sha256(domain.PAYLOAD_PLAIN || optional_salt || jcs(payload))`.
/// Salt, when provided, must be exactly 16 bytes.
#[pyfunction]
#[pyo3(signature = (json_str, salt=None))]
fn payload_plain_hash(json_str: &str, salt: Option<&[u8]>) -> PyResult<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| PyValueError::new_err(format!("invalid JSON: {e}")))?;
    let salt_arr = match salt {
        None => None,
        Some(s) => {
            if s.len() != 16 {
                return Err(PyValueError::new_err(format!(
                    "salt must be exactly 16 bytes, got {}",
                    s.len()
                )));
            }
            let mut out = [0_u8; 16];
            out.copy_from_slice(s);
            Some(out)
        }
    };
    let digest = ::stateset_crypto::hash::compute_payload_plain_hash(&value, salt_arr.as_ref())
        .map_err(|e| PyRuntimeError::new_err(format!("payload_plain_hash: {e}")))?;
    Ok(digest.to_vec())
}

/// Merkle root for a list of 32-byte leaves.
///
/// Returns a 32-byte digest. Each leaf must be exactly 32 bytes. An empty
/// list yields the empty-tree sentinel from `stateset-crypto`.
#[pyfunction]
fn merkle_root(leaves: Vec<Vec<u8>>) -> PyResult<Vec<u8>> {
    let mut typed: Vec<[u8; 32]> = Vec::with_capacity(leaves.len());
    for (i, leaf) in leaves.iter().enumerate() {
        if leaf.len() != 32 {
            return Err(PyValueError::new_err(format!(
                "leaf {i} must be 32 bytes, got {}",
                leaf.len()
            )));
        }
        let mut buf = [0_u8; 32];
        buf.copy_from_slice(leaf);
        typed.push(buf);
    }
    Ok(::stateset_crypto::merkle::compute_merkle_root(&typed).to_vec())
}

// ============================================================================
// Module Definition
// ============================================================================

/// StateSet Embedded Commerce - Local-first commerce library
#[pymodule]
fn stateset_embedded(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Cross-binding crypto primitives (verified by `test_cross_binding_vectors.py`).
    m.add_function(wrap_pyfunction!(jcs_canonicalize, m)?)?;
    m.add_function(wrap_pyfunction!(payload_plain_hash, m)?)?;
    m.add_function(wrap_pyfunction!(merkle_root, m)?)?;

    // Core
    m.add_class::<Commerce>()?;
    m.add_class::<SyncRuntime>()?;
    m.add_class::<SyncEvent>()?;
    m.add_class::<SyncStatus>()?;
    m.add_class::<SyncRemoteHead>()?;
    m.add_class::<SyncAcknowledgement>()?;
    m.add_class::<SyncRejection>()?;
    m.add_class::<SyncPushResult>()?;
    m.add_class::<SyncConfirmation>()?;
    m.add_class::<SyncDeadLetter>()?;
    m.add_class::<SyncPullResult>()?;
    m.add_class::<SyncSnapshot>()?;
    m.add_class::<SyncFullSyncResult>()?;

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

    // Custom Objects (custom states / metaobjects)
    m.add_class::<CustomObjectsApi>()?;
    m.add_class::<CustomObjectType>()?;
    m.add_class::<CustomFieldDefinition>()?;
    m.add_class::<CustomFieldDefinitionInput>()?;
    m.add_class::<CustomObject>()?;

    // Inventory
    m.add_class::<Inventory>()?;
    m.add_class::<InventoryItem>()?;
    m.add_class::<StockLevel>()?;
    m.add_class::<Reservation>()?;

    // Returns
    m.add_class::<Returns>()?;
    m.add_class::<Return>()?;
    m.add_class::<CreateReturnItemInput>()?;
    m.add_class::<GiftCards>()?;
    m.add_class::<GiftCard>()?;
    m.add_class::<GiftCardTransaction>()?;
    m.add_class::<StoreCredits>()?;
    m.add_class::<StoreCredit>()?;
    m.add_class::<StoreCreditTransaction>()?;
    m.add_class::<Reviews>()?;
    m.add_class::<Review>()?;
    m.add_class::<ReviewSummary>()?;
    m.add_class::<Wishlists>()?;
    m.add_class::<Wishlist>()?;
    m.add_class::<WishlistItem>()?;
    m.add_class::<Segments>()?;
    m.add_class::<Segment>()?;
    m.add_class::<SegmentRule>()?;
    m.add_class::<SegmentRuleInput>()?;
    m.add_class::<SegmentMembership>()?;
    m.add_class::<Loyalty>()?;
    m.add_class::<LoyaltyProgram>()?;
    m.add_class::<LoyaltyTier>()?;
    m.add_class::<LoyaltyTierInput>()?;
    m.add_class::<LoyaltyAccount>()?;
    m.add_class::<LoyaltyTransaction>()?;
    m.add_class::<Reward>()?;

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

    // Subscriptions
    m.add_class::<Subscriptions>()?;
    m.add_class::<SubscriptionPlan>()?;
    m.add_class::<Subscription>()?;
    m.add_class::<BillingCycle>()?;
    m.add_class::<SubscriptionEvent>()?;

    // Promotions
    m.add_class::<PromotionsApi>()?;
    m.add_class::<Promotion>()?;
    m.add_class::<Coupon>()?;
    m.add_class::<ApplyPromotionsResult>()?;
    m.add_class::<AppliedPromotion>()?;
    m.add_class::<PromotionUsage>()?;

    // Tax
    m.add_class::<TaxApi>()?;
    m.add_class::<TaxJurisdiction>()?;
    m.add_class::<TaxRate>()?;
    m.add_class::<TaxExemption>()?;
    m.add_class::<TaxSettings>()?;
    m.add_class::<TaxCalculationResult>()?;
    m.add_class::<UsStateTaxInfo>()?;
    m.add_class::<EuVatInfo>()?;
    m.add_class::<CanadianTaxInfo>()?;

    // Quality Control
    m.add_class::<QualityApi>()?;
    m.add_class::<Inspection>()?;
    m.add_class::<NonConformance>()?;
    m.add_class::<QualityHold>()?;

    // Lots/Batch Tracking
    m.add_class::<LotsApi>()?;
    m.add_class::<Lot>()?;

    // Serial Numbers
    m.add_class::<SerialsApi>()?;
    m.add_class::<SerialNumber>()?;

    // Warehouse
    m.add_class::<WarehouseApi>()?;
    m.add_class::<Warehouse>()?;
    m.add_class::<WarehouseLocation>()?;

    // Receiving
    m.add_class::<ReceivingApi>()?;
    m.add_class::<Receipt>()?;
    m.add_class::<ReceiptLine>()?;

    // Fulfillment
    m.add_class::<FulfillmentApi>()?;
    m.add_class::<Wave>()?;
    m.add_class::<PickTask>()?;

    // Accounts Payable
    m.add_class::<AccountsPayableApi>()?;
    m.add_class::<Bill>()?;
    m.add_class::<ApAgingSummary>()?;

    // Accounts Receivable
    m.add_class::<AccountsReceivableApi>()?;
    m.add_class::<ArAgingSummary>()?;
    m.add_class::<CreditMemo>()?;

    // Cost Accounting
    m.add_class::<CostAccountingApi>()?;
    m.add_class::<ItemCost>()?;
    m.add_class::<InventoryValuation>()?;

    // Credit Management
    m.add_class::<CreditApi>()?;
    m.add_class::<CreditAccount>()?;
    m.add_class::<CreditCheckResult>()?;

    // Backorder Management
    m.add_class::<BackorderApi>()?;
    m.add_class::<Backorder>()?;
    m.add_class::<BackorderSummary>()?;

    // General Ledger
    m.add_class::<GeneralLedgerApi>()?;
    m.add_class::<GlAccount>()?;
    m.add_class::<JournalEntry>()?;
    m.add_class::<TrialBalance>()?;
    m.add_class::<GlPeriod>()?;
    m.add_class::<RevaluationLine>()?;
    m.add_class::<RevaluationResult>()?;
    m.add_class::<CloseMonthStep>()?;
    m.add_class::<CloseMonthReport>()?;

    // Three-Way Match (Accounts Payable)
    m.add_class::<ThreeWayMatchLine>()?;
    m.add_class::<ThreeWayMatchResult>()?;

    // Fixed Assets
    m.add_class::<FixedAssets>()?;
    m.add_class::<FixedAsset>()?;
    m.add_class::<AssetDisposal>()?;
    m.add_class::<DepreciationEntry>()?;
    m.add_class::<DepreciationSchedule>()?;

    // Revenue Recognition
    m.add_class::<RevenueRecognition>()?;
    m.add_class::<RevenueContract>()?;
    m.add_class::<PerformanceObligation>()?;
    m.add_class::<PerformanceObligationInput>()?;
    m.add_class::<RevenueScheduleEntry>()?;
    m.add_class::<RevenueSchedule>()?;

    // Cycle Counts
    m.add_class::<CycleCounts>()?;
    m.add_class::<CycleCount>()?;
    m.add_class::<CycleCountLine>()?;
    m.add_class::<CycleCountLineInput>()?;
    m.add_class::<RecordCycleCountLineInput>()?;

    // Vector Search
    m.add_class::<VectorSearch>()?;
    m.add_class::<ProductSearchResult>()?;
    m.add_class::<CustomerSearchResult>()?;
    m.add_class::<EmbeddingStats>()?;

    Ok(())
}

// ============================================================================
// Gift Cards  (money as exact decimal STRINGS, not f64)
// ============================================================================

#[pyclass]
pub struct GiftCard {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    code: String,
    /// Exact decimal string
    #[pyo3(get)]
    initial_balance: String,
    /// Exact decimal string
    #[pyo3(get)]
    current_balance: String,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    recipient_email: Option<String>,
    #[pyo3(get)]
    sender_name: Option<String>,
    #[pyo3(get)]
    message: Option<String>,
    #[pyo3(get)]
    expires_at: Option<String>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::GiftCard> for GiftCard {
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

#[pyclass]
pub struct GiftCardTransaction {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    gift_card_id: String,
    /// Exact decimal string
    #[pyo3(get)]
    amount: String,
    /// Exact decimal string
    #[pyo3(get)]
    balance_after: String,
    #[pyo3(get)]
    transaction_type: String,
    #[pyo3(get)]
    reference_id: Option<String>,
    #[pyo3(get)]
    created_at: String,
}

impl From<stateset_core::GiftCardTransaction> for GiftCardTransaction {
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

#[pyclass]
pub struct GiftCards {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl GiftCards {
    /// Whether the gift-cards backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.gift_cards().is_supported())
    }

    /// Create a gift card. `initial_balance` and money amounts are exact
    /// decimal strings (e.g. "50.00"). `expires_at` is an RFC 3339 timestamp.
    #[pyo3(signature = (initial_balance, currency, code=None, recipient_email=None, sender_name=None, message=None, expires_at=None))]
    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        initial_balance: String,
        currency: String,
        code: Option<String>,
        recipient_email: Option<String>,
        sender_name: Option<String>,
        message: Option<String>,
        expires_at: Option<String>,
    ) -> PyResult<GiftCard> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let initial_balance = initial_balance
            .parse::<Decimal>()
            .map_err(|_| PyValueError::new_err("Invalid initial_balance decimal"))?;
        let currency = currency
            .parse::<CurrencyCode>()
            .map_err(|_| PyValueError::new_err("Invalid currency code"))?;
        let expires_at = match expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| PyValueError::new_err("Invalid expires_at RFC 3339 timestamp"))?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let card = commerce
            .gift_cards()
            .create(stateset_core::CreateGiftCard {
                code,
                initial_balance,
                currency,
                recipient_email,
                sender_name,
                message,
                expires_at,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create gift card: {}", e)))?;
        Ok(card.into())
    }

    /// Get a gift card by ID.
    fn get(&self, id: String) -> PyResult<Option<GiftCard>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let card = commerce
            .gift_cards()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get gift card: {}", e)))?;
        Ok(card.map(Into::into))
    }

    /// Get a gift card by its redemption code.
    fn get_by_code(&self, code: String) -> PyResult<Option<GiftCard>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let card = commerce
            .gift_cards()
            .get_by_code(&code)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get gift card: {}", e)))?;
        Ok(card.map(Into::into))
    }

    /// Update a gift card's status and/or recipient email.
    #[pyo3(signature = (id, status=None, recipient_email=None))]
    fn update(
        &self,
        id: String,
        status: Option<String>,
        recipient_email: Option<String>,
    ) -> PyResult<GiftCard> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let status = match status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::GiftCardStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid gift card status"))?,
            ),
            None => None,
        };
        let card = commerce
            .gift_cards()
            .update(
                uuid.into(),
                stateset_core::UpdateGiftCard { status, recipient_email, ..Default::default() },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update gift card: {}", e)))?;
        Ok(card.into())
    }

    /// List gift cards, optionally filtered by status and/or code.
    #[pyo3(signature = (status=None, code=None, limit=None, offset=None))]
    fn list(
        &self,
        status: Option<String>,
        code: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<GiftCard>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let status = match status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::GiftCardStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid gift card status"))?,
            ),
            None => None,
        };
        let cards = commerce
            .gift_cards()
            .list(stateset_core::GiftCardFilter { status, code, limit, offset })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list gift cards: {}", e)))?;
        Ok(cards.into_iter().map(Into::into).collect())
    }

    /// Charge (debit) an amount from a gift card. `amount` is a decimal string.
    #[pyo3(signature = (id, amount, reference_id=None))]
    fn charge(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> PyResult<GiftCardTransaction> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| PyValueError::new_err("Invalid amount decimal"))?;
        let txn = commerce
            .gift_cards()
            .charge(uuid.into(), amount, reference_id)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to charge gift card: {}", e)))?;
        Ok(txn.into())
    }

    /// Refund (credit) an amount to a gift card. `amount` is a decimal string.
    #[pyo3(signature = (id, amount, reference_id=None))]
    fn refund(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> PyResult<GiftCardTransaction> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| PyValueError::new_err("Invalid amount decimal"))?;
        let txn = commerce
            .gift_cards()
            .refund(uuid.into(), amount, reference_id)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to refund gift card: {}", e)))?;
        Ok(txn.into())
    }

    /// Disable a gift card so it can no longer be used.
    fn disable(&self, id: String) -> PyResult<GiftCard> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let card = commerce
            .gift_cards()
            .disable(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to disable gift card: {}", e)))?;
        Ok(card.into())
    }

    /// Get the transaction history for a gift card.
    fn get_transactions(&self, gift_card_id: String) -> PyResult<Vec<GiftCardTransaction>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            gift_card_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let txns = commerce
            .gift_cards()
            .get_transactions(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get transactions: {}", e)))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Store Credits  (money as exact decimal STRINGS, not f64)
// ============================================================================

#[pyclass]
pub struct StoreCredit {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    customer_id: String,
    /// Exact decimal string
    #[pyo3(get)]
    original_balance: String,
    /// Exact decimal string
    #[pyo3(get)]
    current_balance: String,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    reason: String,
    #[pyo3(get)]
    reference_id: Option<String>,
    #[pyo3(get)]
    note: Option<String>,
    #[pyo3(get)]
    expires_at: Option<String>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::StoreCredit> for StoreCredit {
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

#[pyclass]
pub struct StoreCreditTransaction {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    store_credit_id: String,
    /// Exact decimal string (positive = credit, negative = debit)
    #[pyo3(get)]
    amount: String,
    /// Exact decimal string
    #[pyo3(get)]
    balance_after: String,
    #[pyo3(get)]
    transaction_type: String,
    #[pyo3(get)]
    reference_id: Option<String>,
    #[pyo3(get)]
    created_at: String,
}

impl From<stateset_core::StoreCreditTransaction> for StoreCreditTransaction {
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

#[pyclass]
pub struct StoreCredits {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl StoreCredits {
    /// Whether the store-credits backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.store_credits().is_supported())
    }

    /// Issue a store credit to a customer. `amount` is an exact decimal string
    /// (e.g. "25.00"). `reason` is one of return, loyalty, compensation,
    /// promotion, manual, gift_card (defaults to "return"). `expires_at` is an
    /// RFC 3339 timestamp.
    #[pyo3(signature = (customer_id, amount, currency, reason=None, reference_id=None, note=None, expires_at=None))]
    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        customer_id: String,
        amount: String,
        currency: String,
        reason: Option<String>,
        reference_id: Option<String>,
        note: Option<String>,
        expires_at: Option<String>,
    ) -> PyResult<StoreCredit> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| PyValueError::new_err("Invalid amount decimal"))?;
        let currency = currency
            .parse::<CurrencyCode>()
            .map_err(|_| PyValueError::new_err("Invalid currency code"))?;
        let reason = match reason.as_deref() {
            Some(s) => s
                .parse::<stateset_core::StoreCreditReason>()
                .map_err(|_| PyValueError::new_err("Invalid store credit reason"))?,
            None => stateset_core::StoreCreditReason::default(),
        };
        let expires_at = match expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| PyValueError::new_err("Invalid expires_at RFC 3339 timestamp"))?
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
                reference_id,
                note,
                expires_at,
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create store credit: {}", e))
            })?;
        Ok(credit.into())
    }

    /// Get a store credit by ID.
    fn get(&self, id: String) -> PyResult<Option<StoreCredit>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let credit = commerce
            .store_credits()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get store credit: {}", e)))?;
        Ok(credit.map(Into::into))
    }

    /// List store credits, optionally filtered by customer, status, or reason.
    #[pyo3(signature = (customer_id=None, status=None, reason=None, limit=None, offset=None))]
    fn list(
        &self,
        customer_id: Option<String>,
        status: Option<String>,
        reason: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<StoreCredit>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_id = match customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let status = match status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::StoreCreditStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid store credit status"))?,
            ),
            None => None,
        };
        let reason = match reason.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::StoreCreditReason>()
                    .map_err(|_| PyValueError::new_err("Invalid store credit reason"))?,
            ),
            None => None,
        };
        let credits = commerce
            .store_credits()
            .list(stateset_core::StoreCreditFilter { customer_id, status, reason, limit, offset })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list store credits: {}", e)))?;
        Ok(credits.into_iter().map(Into::into).collect())
    }

    /// Adjust a store credit balance. `amount` is a signed decimal string
    /// ("10.00" adds, "-10.00" subtracts). The balance may not go below zero.
    #[pyo3(signature = (id, amount, note=None, reference_id=None))]
    fn adjust(
        &self,
        id: String,
        amount: String,
        note: Option<String>,
        reference_id: Option<String>,
    ) -> PyResult<StoreCredit> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| PyValueError::new_err("Invalid amount decimal"))?;
        let credit = commerce
            .store_credits()
            .adjust(uuid.into(), stateset_core::AdjustStoreCredit { amount, note, reference_id })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to adjust store credit: {}", e))
            })?;
        Ok(credit.into())
    }

    /// Apply (redeem) an amount from a store credit, returning the ledger
    /// transaction. `amount` is a decimal string.
    #[pyo3(signature = (id, amount, reference_id=None))]
    fn apply(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> PyResult<StoreCreditTransaction> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| PyValueError::new_err("Invalid amount decimal"))?;
        let txn = commerce
            .store_credits()
            .apply(uuid.into(), amount, reference_id)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to apply store credit: {}", e)))?;
        Ok(txn.into())
    }

    /// Get the transaction history for a store credit.
    fn get_transactions(&self, store_credit_id: String) -> PyResult<Vec<StoreCreditTransaction>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            store_credit_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let txns = commerce
            .store_credits()
            .get_transactions(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get transactions: {}", e)))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Product reviews
// ============================================================================

#[pyclass]
pub struct Review {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    rating: u32,
    #[pyo3(get)]
    title: Option<String>,
    #[pyo3(get)]
    body: Option<String>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    verified_purchase: bool,
    #[pyo3(get)]
    helpful_count: u32,
    #[pyo3(get)]
    reported_count: u32,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::Review> for Review {
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

#[pyclass]
pub struct ReviewSummary {
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    average_rating: f64,
    #[pyo3(get)]
    total_reviews: u64,
    /// Counts for 1..5 stars (index 0 = 1 star)
    #[pyo3(get)]
    rating_distribution: Vec<u32>,
}

impl From<stateset_core::ReviewSummary> for ReviewSummary {
    fn from(s: stateset_core::ReviewSummary) -> Self {
        Self {
            product_id: s.product_id.to_string(),
            average_rating: s.average_rating,
            total_reviews: s.total_reviews,
            rating_distribution: s.rating_distribution.to_vec(),
        }
    }
}

#[pyclass]
pub struct Reviews {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Reviews {
    /// Whether the reviews backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.reviews().is_supported())
    }

    /// Create a product review. `rating` is a star rating 1–5.
    #[pyo3(signature = (product_id, customer_id, rating, title=None, body=None, verified_purchase=false))]
    fn create(
        &self,
        product_id: String,
        customer_id: String,
        rating: u32,
        title: Option<String>,
        body: Option<String>,
        verified_purchase: bool,
    ) -> PyResult<Review> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let product_uuid: uuid::Uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid product UUID"))?;
        let customer_uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let rating = u8::try_from(rating)
            .map_err(|_| PyValueError::new_err("rating must be between 1 and 5"))?;
        let review = commerce
            .reviews()
            .create(stateset_core::CreateReview {
                product_id: product_uuid.into(),
                customer_id: customer_uuid.into(),
                rating,
                title,
                body,
                verified_purchase,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create review: {}", e)))?;
        Ok(review.into())
    }

    /// Get a review by ID.
    fn get(&self, id: String) -> PyResult<Option<Review>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let review = commerce
            .reviews()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get review: {}", e)))?;
        Ok(review.map(Into::into))
    }

    /// Update a review's rating, title, body, and/or moderation status
    /// (pending, approved, rejected, flagged).
    #[pyo3(signature = (id, rating=None, title=None, body=None, status=None))]
    fn update(
        &self,
        id: String,
        rating: Option<u32>,
        title: Option<String>,
        body: Option<String>,
        status: Option<String>,
    ) -> PyResult<Review> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let rating = match rating {
            Some(r) => Some(
                u8::try_from(r)
                    .map_err(|_| PyValueError::new_err("rating must be between 1 and 5"))?,
            ),
            None => None,
        };
        let status = match status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::ReviewStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid review status"))?,
            ),
            None => None,
        };
        let review = commerce
            .reviews()
            .update(
                uuid.into(),
                stateset_core::UpdateReview {
                    rating,
                    title: title.map(Some),
                    body: body.map(Some),
                    status,
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update review: {}", e)))?;
        Ok(review.into())
    }

    /// List reviews, optionally filtered.
    #[pyo3(signature = (product_id=None, customer_id=None, status=None, min_rating=None, verified_only=None, limit=None, offset=None))]
    #[allow(clippy::too_many_arguments)]
    fn list(
        &self,
        product_id: Option<String>,
        customer_id: Option<String>,
        status: Option<String>,
        min_rating: Option<u32>,
        verified_only: Option<bool>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<Review>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let product_id = match product_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid product UUID"))?
                    .into(),
            ),
            None => None,
        };
        let customer_id = match customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let status = match status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::ReviewStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid review status"))?,
            ),
            None => None,
        };
        let min_rating = match min_rating {
            Some(r) => Some(
                u8::try_from(r).map_err(|_| PyValueError::new_err("min_rating out of range"))?,
            ),
            None => None,
        };
        let reviews = commerce
            .reviews()
            .list(stateset_core::ReviewFilter {
                product_id,
                customer_id,
                status,
                min_rating,
                verified_only,
                limit,
                offset,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list reviews: {}", e)))?;
        Ok(reviews.into_iter().map(Into::into).collect())
    }

    /// Delete a review.
    fn delete(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        commerce
            .reviews()
            .delete(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete review: {}", e)))?;
        Ok(())
    }

    /// Aggregate rating summary for a product (average, total, star distribution).
    fn get_summary(&self, product_id: String) -> PyResult<ReviewSummary> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid product UUID"))?;
        let summary = commerce
            .reviews()
            .get_summary(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get review summary: {}", e)))?;
        Ok(summary.into())
    }

    /// Increment the helpful counter on a review.
    fn mark_helpful(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        commerce.reviews().mark_helpful(uuid.into()).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to mark review helpful: {}", e))
        })?;
        Ok(())
    }

    /// Increment the reported counter on a review.
    fn mark_reported(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        commerce.reviews().mark_reported(uuid.into()).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to mark review reported: {}", e))
        })?;
        Ok(())
    }
}

// ============================================================================
// Wishlists
// ============================================================================

// Output-only; Clone is needed for the nested `items` getter on Wishlist, but
// this type is never extracted from Python.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct WishlistItem {
    #[pyo3(get)]
    product_id: String,
    #[pyo3(get)]
    variant_id: Option<String>,
    #[pyo3(get)]
    added_at: String,
    #[pyo3(get)]
    note: Option<String>,
    #[pyo3(get)]
    quantity: u32,
    #[pyo3(get)]
    priority: Option<i32>,
}

impl From<stateset_core::WishlistItem> for WishlistItem {
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

#[pyclass]
pub struct Wishlist {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    is_public: bool,
    #[pyo3(get)]
    items: Vec<WishlistItem>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::Wishlist> for Wishlist {
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

#[pyclass]
pub struct Wishlists {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Wishlists {
    /// Whether the wishlists backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.wishlists().is_supported())
    }

    /// Create a wishlist for a customer.
    #[pyo3(signature = (customer_id, name, is_public=false))]
    fn create(&self, customer_id: String, name: String, is_public: bool) -> PyResult<Wishlist> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let wishlist = commerce
            .wishlists()
            .create(stateset_core::CreateWishlist {
                customer_id: customer_uuid.into(),
                name,
                is_public,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create wishlist: {}", e)))?;
        Ok(wishlist.into())
    }

    /// Get a wishlist by ID.
    fn get(&self, id: String) -> PyResult<Option<Wishlist>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let wishlist = commerce
            .wishlists()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get wishlist: {}", e)))?;
        Ok(wishlist.map(Into::into))
    }

    /// Rename a wishlist and/or change its visibility.
    #[pyo3(signature = (id, name=None, is_public=None))]
    fn update(
        &self,
        id: String,
        name: Option<String>,
        is_public: Option<bool>,
    ) -> PyResult<Wishlist> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let wishlist = commerce
            .wishlists()
            .update(uuid.into(), stateset_core::UpdateWishlist { name, is_public })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update wishlist: {}", e)))?;
        Ok(wishlist.into())
    }

    /// List wishlists, optionally filtered by customer or visibility.
    #[pyo3(signature = (customer_id=None, is_public=None, limit=None, offset=None))]
    fn list(
        &self,
        customer_id: Option<String>,
        is_public: Option<bool>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<Wishlist>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_id = match customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let wishlists = commerce
            .wishlists()
            .list(stateset_core::WishlistFilter { customer_id, is_public, limit, offset })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list wishlists: {}", e)))?;
        Ok(wishlists.into_iter().map(Into::into).collect())
    }

    /// Delete a wishlist.
    fn delete(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        commerce
            .wishlists()
            .delete(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete wishlist: {}", e)))?;
        Ok(())
    }

    /// Add a product to a wishlist, returning the added item.
    #[pyo3(signature = (wishlist_id, product_id, variant_id=None, note=None, quantity=None, priority=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_item(
        &self,
        wishlist_id: String,
        product_id: String,
        variant_id: Option<String>,
        note: Option<String>,
        quantity: Option<u32>,
        priority: Option<i32>,
    ) -> PyResult<WishlistItem> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wishlist_uuid: uuid::Uuid =
            wishlist_id.parse().map_err(|_| PyValueError::new_err("Invalid wishlist UUID"))?;
        let product_uuid: uuid::Uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid product UUID"))?;
        let added = commerce
            .wishlists()
            .add_item(
                wishlist_uuid.into(),
                stateset_core::AddWishlistItem {
                    product_id: product_uuid.into(),
                    variant_id,
                    note,
                    quantity,
                    priority,
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to add wishlist item: {}", e)))?;
        Ok(added.into())
    }

    /// Remove a product from a wishlist.
    fn remove_item(&self, wishlist_id: String, product_id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let wishlist_uuid: uuid::Uuid =
            wishlist_id.parse().map_err(|_| PyValueError::new_err("Invalid wishlist UUID"))?;
        let product_uuid: uuid::Uuid =
            product_id.parse().map_err(|_| PyValueError::new_err("Invalid product UUID"))?;
        commerce.wishlists().remove_item(wishlist_uuid.into(), product_uuid.into()).map_err(
            |e| PyRuntimeError::new_err(format!("Failed to remove wishlist item: {}", e)),
        )?;
        Ok(())
    }
}

// ============================================================================
// Customer segments
// ============================================================================

/// A segment rule (field/operator/value) passed to create/update.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct SegmentRuleInput {
    #[pyo3(get, set)]
    field: String,
    /// One of: eq, neq, gt, gte, lt, lte, contains, in, between, starts_with,
    /// ends_with
    #[pyo3(get, set)]
    operator: String,
    #[pyo3(get, set)]
    value: String,
}

#[pymethods]
impl SegmentRuleInput {
    #[new]
    fn new(field: String, operator: String, value: String) -> Self {
        Self { field, operator, value }
    }
}

// Output-only; Clone is needed for the nested `rules` getter on Segment.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct SegmentRule {
    #[pyo3(get)]
    field: String,
    #[pyo3(get)]
    operator: String,
    #[pyo3(get)]
    value: String,
}

impl From<stateset_core::SegmentRule> for SegmentRule {
    fn from(r: stateset_core::SegmentRule) -> Self {
        Self { field: r.field, operator: format!("{}", r.operator), value: r.value }
    }
}

fn parse_segment_rules(rules: Vec<SegmentRuleInput>) -> PyResult<Vec<stateset_core::SegmentRule>> {
    rules
        .into_iter()
        .map(|r| {
            Ok(stateset_core::SegmentRule {
                field: r.field,
                operator: r.operator.parse::<stateset_core::SegmentOperator>().map_err(|_| {
                    PyValueError::new_err(format!("Invalid segment operator '{}'", r.operator))
                })?,
                value: r.value,
            })
        })
        .collect()
}

#[pyclass]
pub struct Segment {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    segment_type: String,
    #[pyo3(get)]
    rules: Vec<SegmentRule>,
    #[pyo3(get)]
    member_count: u64,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::Segment> for Segment {
    fn from(s: stateset_core::Segment) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            description: s.description,
            segment_type: format!("{}", s.segment_type),
            rules: s.rules.into_iter().map(Into::into).collect(),
            member_count: s.member_count,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[pyclass]
pub struct SegmentMembership {
    #[pyo3(get)]
    segment_id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    joined_at: String,
}

impl From<stateset_core::SegmentMembership> for SegmentMembership {
    fn from(m: stateset_core::SegmentMembership) -> Self {
        Self {
            segment_id: m.segment_id.to_string(),
            customer_id: m.customer_id.to_string(),
            joined_at: m.joined_at.to_rfc3339(),
        }
    }
}

#[pyclass]
pub struct Segments {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Segments {
    /// Whether the segments backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.segments().is_supported())
    }

    /// Create a customer segment. `segment_type` is "static" (default) or
    /// "dynamic"; `rules` is a list of `SegmentRuleInput`.
    #[pyo3(signature = (name, description=None, segment_type=None, rules=Vec::new()))]
    fn create(
        &self,
        name: String,
        description: Option<String>,
        segment_type: Option<String>,
        rules: Vec<SegmentRuleInput>,
    ) -> PyResult<Segment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let segment_type = match segment_type.as_deref() {
            Some(s) => s.parse::<stateset_core::SegmentType>().map_err(|_| {
                PyValueError::new_err("Invalid segment_type (use static or dynamic)")
            })?,
            None => stateset_core::SegmentType::default(),
        };
        let rules = parse_segment_rules(rules)?;
        let segment = commerce
            .segments()
            .create(stateset_core::CreateSegment { name, description, segment_type, rules })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create segment: {}", e)))?;
        Ok(segment.into())
    }

    /// Get a segment by ID.
    fn get(&self, id: String) -> PyResult<Option<Segment>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let segment = commerce
            .segments()
            .get(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get segment: {}", e)))?;
        Ok(segment.map(Into::into))
    }

    /// Update a segment's name, description, and/or rules.
    #[pyo3(signature = (id, name=None, description=None, rules=None))]
    fn update(
        &self,
        id: String,
        name: Option<String>,
        description: Option<String>,
        rules: Option<Vec<SegmentRuleInput>>,
    ) -> PyResult<Segment> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let rules = match rules {
            Some(r) => Some(parse_segment_rules(r)?),
            None => None,
        };
        let segment = commerce
            .segments()
            .update(
                uuid.into(),
                stateset_core::UpdateSegment { name, description: description.map(Some), rules },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update segment: {}", e)))?;
        Ok(segment.into())
    }

    /// List segments, optionally filtered by type or name.
    #[pyo3(signature = (segment_type=None, name=None, limit=None, offset=None))]
    fn list(
        &self,
        segment_type: Option<String>,
        name: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<Segment>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let segment_type = match segment_type.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::SegmentType>()
                    .map_err(|_| PyValueError::new_err("Invalid segment_type"))?,
            ),
            None => None,
        };
        let segments = commerce
            .segments()
            .list(stateset_core::SegmentFilter { segment_type, name, limit, offset })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list segments: {}", e)))?;
        Ok(segments.into_iter().map(Into::into).collect())
    }

    /// Delete a segment.
    fn delete(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        commerce
            .segments()
            .delete(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete segment: {}", e)))?;
        Ok(())
    }

    /// Add a customer to a (static) segment, returning the membership record.
    fn add_member(&self, segment_id: String, customer_id: String) -> PyResult<SegmentMembership> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| PyValueError::new_err("Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let membership = commerce
            .segments()
            .add_member(seg.into(), cust.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to add segment member: {}", e)))?;
        Ok(membership.into())
    }

    /// Remove a customer from a segment.
    fn remove_member(&self, segment_id: String, customer_id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| PyValueError::new_err("Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        commerce.segments().remove_member(seg.into(), cust.into()).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to remove segment member: {}", e))
        })?;
        Ok(())
    }

    /// List a segment's members.
    #[pyo3(signature = (segment_id, limit=None, offset=None))]
    fn list_members(
        &self,
        segment_id: String,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<SegmentMembership>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| PyValueError::new_err("Invalid segment UUID"))?;
        let members = commerce.segments().list_members(seg.into(), limit, offset).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to list segment members: {}", e))
        })?;
        Ok(members.into_iter().map(Into::into).collect())
    }

    /// Whether a customer is a member of a segment.
    fn is_member(&self, segment_id: String, customer_id: String) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let seg: uuid::Uuid =
            segment_id.parse().map_err(|_| PyValueError::new_err("Invalid segment UUID"))?;
        let cust: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        commerce.segments().is_member(seg.into(), cust.into()).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to check segment membership: {}", e))
        })
    }
}

// ============================================================================
// Loyalty  (points are integers; reward `value` is an exact decimal string)
// ============================================================================

#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct LoyaltyTierInput {
    #[pyo3(get, set)]
    name: String,
    #[pyo3(get, set)]
    min_points: i64,
    #[pyo3(get, set)]
    multiplier: f64,
    #[pyo3(get, set)]
    perks: Vec<String>,
}

#[pymethods]
impl LoyaltyTierInput {
    #[new]
    #[pyo3(signature = (name, min_points=0, multiplier=1.0, perks=Vec::new()))]
    fn new(name: String, min_points: i64, multiplier: f64, perks: Vec<String>) -> Self {
        Self { name, min_points, multiplier, perks }
    }
}

// Output-only; Clone is needed for the nested `tiers` getter on
// LoyaltyProgram, but this type is never extracted from Python.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct LoyaltyTier {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    min_points: i64,
    #[pyo3(get)]
    multiplier: f64,
    #[pyo3(get)]
    perks: Vec<String>,
}

impl From<stateset_core::LoyaltyTier> for LoyaltyTier {
    fn from(t: stateset_core::LoyaltyTier) -> Self {
        Self {
            name: t.name,
            min_points: t.min_points as i64,
            multiplier: t.multiplier,
            perks: t.perks,
        }
    }
}

#[pyclass]
pub struct LoyaltyProgram {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    points_per_dollar: u32,
    #[pyo3(get)]
    tiers: Vec<LoyaltyTier>,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::LoyaltyProgram> for LoyaltyProgram {
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

#[pyclass]
pub struct LoyaltyAccount {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    program_id: String,
    #[pyo3(get)]
    points_balance: i64,
    #[pyo3(get)]
    lifetime_points: i64,
    #[pyo3(get)]
    tier: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::LoyaltyAccount> for LoyaltyAccount {
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

#[pyclass]
pub struct LoyaltyTransaction {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    account_id: String,
    #[pyo3(get)]
    points: i64,
    #[pyo3(get)]
    transaction_type: String,
    #[pyo3(get)]
    reference_id: Option<String>,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    created_at: String,
}

impl From<stateset_core::LoyaltyTransaction> for LoyaltyTransaction {
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

#[pyclass]
pub struct Reward {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    program_id: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    points_cost: i64,
    #[pyo3(get)]
    reward_type: String,
    /// Exact decimal string, if set
    #[pyo3(get)]
    value: Option<String>,
    #[pyo3(get)]
    is_active: bool,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::Reward> for Reward {
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

#[pyclass]
pub struct Loyalty {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl Loyalty {
    /// Whether the loyalty backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.loyalty().is_supported())
    }

    /// Create a loyalty program. `tiers` is a list of LoyaltyTierInput.
    #[pyo3(signature = (name, points_per_dollar, description=None, tiers=Vec::new()))]
    fn create_program(
        &self,
        name: String,
        points_per_dollar: u32,
        description: Option<String>,
        tiers: Vec<LoyaltyTierInput>,
    ) -> PyResult<LoyaltyProgram> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let tiers = tiers
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
                name,
                description,
                points_per_dollar,
                tiers,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create program: {}", e)))?;
        Ok(program.into())
    }

    fn get_program(&self, id: String) -> PyResult<Option<LoyaltyProgram>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let program = commerce
            .loyalty()
            .get_program(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get program: {}", e)))?;
        Ok(program.map(Into::into))
    }

    fn list_programs(&self) -> PyResult<Vec<LoyaltyProgram>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let programs = commerce
            .loyalty()
            .list_programs()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list programs: {}", e)))?;
        Ok(programs.into_iter().map(Into::into).collect())
    }

    /// Enroll a customer in a loyalty program.
    fn enroll(&self, customer_id: String, program_id: String) -> PyResult<LoyaltyAccount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_id: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let program_id: uuid::Uuid =
            program_id.parse().map_err(|_| PyValueError::new_err("Invalid program UUID"))?;
        let account = commerce
            .loyalty()
            .enroll(stateset_core::EnrollCustomer {
                customer_id: customer_id.into(),
                program_id: program_id.into(),
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to enroll: {}", e)))?;
        Ok(account.into())
    }

    fn get_account(&self, id: String) -> PyResult<Option<LoyaltyAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let account = commerce
            .loyalty()
            .get_account(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get account: {}", e)))?;
        Ok(account.map(Into::into))
    }

    /// Adjust an account's points. `transaction_type` is e.g. "earn", "redeem".
    #[pyo3(signature = (account_id, points, transaction_type, reference_id=None, description=None))]
    fn adjust_points(
        &self,
        account_id: String,
        points: i64,
        transaction_type: String,
        reference_id: Option<String>,
        description: Option<String>,
    ) -> PyResult<LoyaltyTransaction> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let account_id: uuid::Uuid =
            account_id.parse().map_err(|_| PyValueError::new_err("Invalid account UUID"))?;
        let transaction_type = transaction_type
            .parse::<stateset_core::LoyaltyTransactionType>()
            .map_err(|_| PyValueError::new_err("Invalid transaction_type"))?;
        let txn = commerce
            .loyalty()
            .adjust_points(stateset_core::AdjustPoints {
                account_id: account_id.into(),
                points,
                transaction_type,
                reference_id,
                description,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to adjust points: {}", e)))?;
        Ok(txn.into())
    }

    #[pyo3(signature = (account_id, limit=None))]
    fn get_transactions(
        &self,
        account_id: String,
        limit: Option<u32>,
    ) -> PyResult<Vec<LoyaltyTransaction>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            account_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let txns = commerce
            .loyalty()
            .get_transactions(uuid.into(), limit)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get transactions: {}", e)))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }

    /// Create a reward. `value` is an exact decimal string (optional).
    #[pyo3(signature = (program_id, name, points_cost, reward_type, description=None, value=None))]
    fn create_reward(
        &self,
        program_id: String,
        name: String,
        points_cost: i64,
        reward_type: String,
        description: Option<String>,
        value: Option<String>,
    ) -> PyResult<Reward> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let program_id: uuid::Uuid =
            program_id.parse().map_err(|_| PyValueError::new_err("Invalid program UUID"))?;
        let reward_type = reward_type
            .parse::<stateset_core::RewardType>()
            .map_err(|_| PyValueError::new_err("Invalid reward_type"))?;
        let value = match value.as_deref() {
            Some(s) => Some(
                s.parse::<Decimal>().map_err(|_| PyValueError::new_err("Invalid value decimal"))?,
            ),
            None => None,
        };
        let reward = commerce
            .loyalty()
            .create_reward(stateset_core::CreateReward {
                program_id: program_id.into(),
                name,
                description,
                points_cost: points_cost.max(0) as u64,
                reward_type,
                value,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create reward: {}", e)))?;
        Ok(reward.into())
    }

    fn get_reward(&self, id: String) -> PyResult<Option<Reward>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let reward = commerce
            .loyalty()
            .get_reward(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get reward: {}", e)))?;
        Ok(reward.map(Into::into))
    }

    fn delete_reward(&self, id: String) -> PyResult<()> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        commerce
            .loyalty()
            .delete_reward(uuid.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to delete reward: {}", e)))?;
        Ok(())
    }

    fn get_account_by_customer(
        &self,
        customer_id: String,
        program_id: String,
    ) -> PyResult<Option<LoyaltyAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_id: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let program_id: uuid::Uuid =
            program_id.parse().map_err(|_| PyValueError::new_err("Invalid program UUID"))?;
        let account = commerce
            .loyalty()
            .get_account_by_customer(customer_id.into(), program_id.into())
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get account: {}", e)))?;
        Ok(account.map(Into::into))
    }

    #[pyo3(signature = (customer_id=None, program_id=None, tier=None, limit=None, offset=None))]
    fn list_accounts(
        &self,
        customer_id: Option<String>,
        program_id: Option<String>,
        tier: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<LoyaltyAccount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_id = match customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let program_id = match program_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid program UUID"))?
                    .into(),
            ),
            None => None,
        };
        let accounts = commerce
            .loyalty()
            .list_accounts(stateset_core::LoyaltyAccountFilter {
                customer_id,
                program_id,
                tier,
                limit,
                offset,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list accounts: {}", e)))?;
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    #[pyo3(signature = (program_id=None, reward_type=None, is_active=None, limit=None, offset=None))]
    fn list_rewards(
        &self,
        program_id: Option<String>,
        reward_type: Option<String>,
        is_active: Option<bool>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<Reward>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let program_id = match program_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| PyValueError::new_err("Invalid program UUID"))?
                    .into(),
            ),
            None => None,
        };
        let reward_type = match reward_type.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::RewardType>()
                    .map_err(|_| PyValueError::new_err("Invalid reward_type"))?,
            ),
            None => None,
        };
        let rewards = commerce
            .loyalty()
            .list_rewards(stateset_core::RewardFilter {
                program_id,
                reward_type,
                is_active,
                limit,
                offset,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list rewards: {}", e)))?;
        Ok(rewards.into_iter().map(Into::into).collect())
    }
}

// ============================================================================
// Finance helpers (exact decimal strings, ISO dates, snake_case enums)
// ============================================================================

fn parse_iso_date_py(s: &str, field: &str) -> PyResult<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| PyValueError::new_err(format!("Invalid {field} date (expected YYYY-MM-DD)")))
}

fn parse_decimal_py(s: &str, field: &str) -> PyResult<Decimal> {
    s.parse::<Decimal>().map_err(|_| PyValueError::new_err(format!("Invalid {field} decimal")))
}

fn parse_optional_uuid_py(s: Option<String>, field: &str) -> PyResult<Option<uuid::Uuid>> {
    s.map(|s| {
        s.parse::<uuid::Uuid>().map_err(|_| PyValueError::new_err(format!("Invalid {field} UUID")))
    })
    .transpose()
}

fn parse_depreciation_method_py(
    method: &str,
    rate: Option<&str>,
) -> PyResult<stateset_core::DepreciationMethod> {
    match method {
        "straight_line" => Ok(stateset_core::DepreciationMethod::StraightLine),
        "declining_balance" => {
            let rate = rate.ok_or_else(|| {
                PyValueError::new_err("declining_balance requires declining_balance_rate")
            })?;
            Ok(stateset_core::DepreciationMethod::DecliningBalance {
                rate: parse_decimal_py(rate, "declining_balance_rate")?,
            })
        }
        "units_of_production" => Ok(stateset_core::DepreciationMethod::UnitsOfProduction),
        _ => Err(PyValueError::new_err(
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

fn parse_recognition_method_py(
    method: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> PyResult<stateset_core::RecognitionMethod> {
    match method {
        "point_in_time" => Ok(stateset_core::RecognitionMethod::PointInTime),
        "ratable_over_time" => {
            let start = start.ok_or_else(|| {
                PyValueError::new_err("ratable_over_time requires recognition_start")
            })?;
            let end = end.ok_or_else(|| {
                PyValueError::new_err("ratable_over_time requires recognition_end")
            })?;
            Ok(stateset_core::RecognitionMethod::RatableOverTime {
                start: parse_iso_date_py(start, "recognition_start")?,
                end: parse_iso_date_py(end, "recognition_end")?,
            })
        }
        "milestone" => Ok(stateset_core::RecognitionMethod::Milestone),
        _ => Err(PyValueError::new_err(
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

// ============================================================================
// Fixed Assets  (money as exact decimal STRINGS, dates as ISO strings)
// ============================================================================

/// A recorded asset disposal. Money values are exact decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct AssetDisposal {
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    disposal_date: String,
    /// Exact decimal string
    #[pyo3(get)]
    proceeds: String,
    /// Exact decimal string
    #[pyo3(get)]
    book_value_at_disposal: String,
    /// Exact decimal string: proceeds - book value
    #[pyo3(get)]
    gain_loss: String,
    #[pyo3(get)]
    notes: Option<String>,
}

impl From<stateset_core::AssetDisposal> for AssetDisposal {
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

/// A fixed asset. Money values are exact decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct FixedAsset {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    asset_number: String,
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    description: Option<String>,
    #[pyo3(get)]
    category: String,
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    acquisition_date: String,
    /// Exact decimal string
    #[pyo3(get)]
    acquisition_cost: String,
    /// Exact decimal string
    #[pyo3(get)]
    salvage_value: String,
    #[pyo3(get)]
    useful_life_months: u32,
    /// straight_line, declining_balance, units_of_production
    #[pyo3(get)]
    depreciation_method: String,
    /// Set when depreciation_method is declining_balance
    #[pyo3(get)]
    declining_balance_rate: Option<String>,
    /// draft, in_service, fully_depreciated, disposed, written_off
    #[pyo3(get)]
    status: String,
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    in_service_date: Option<String>,
    #[pyo3(get)]
    location_id: Option<String>,
    #[pyo3(get)]
    asset_account_id: Option<String>,
    #[pyo3(get)]
    accumulated_depreciation_account_id: Option<String>,
    #[pyo3(get)]
    depreciation_expense_account_id: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    accumulated_depreciation: String,
    /// Exact decimal string: acquisition_cost - accumulated_depreciation
    #[pyo3(get)]
    book_value: String,
    #[pyo3(get)]
    currency: String,
    #[pyo3(get)]
    disposal: Option<AssetDisposal>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::FixedAsset> for FixedAsset {
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

/// One period in a depreciation schedule. Money values are decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct DepreciationEntry {
    #[pyo3(get)]
    period: u32,
    /// Exact decimal string
    #[pyo3(get)]
    amount: String,
    /// Exact decimal string
    #[pyo3(get)]
    accumulated: String,
    /// Exact decimal string
    #[pyo3(get)]
    book_value: String,
    /// scheduled or posted
    #[pyo3(get)]
    status: String,
}

impl From<stateset_core::DepreciationEntry> for DepreciationEntry {
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

/// A depreciation schedule for a fixed asset.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct DepreciationSchedule {
    #[pyo3(get)]
    asset_id: String,
    /// straight_line, declining_balance, units_of_production
    #[pyo3(get)]
    method: String,
    /// Set when method is declining_balance
    #[pyo3(get)]
    declining_balance_rate: Option<String>,
    #[pyo3(get)]
    entries: Vec<DepreciationEntry>,
    /// Exact decimal string
    #[pyo3(get)]
    total_depreciation: String,
}

impl From<stateset_core::DepreciationSchedule> for DepreciationSchedule {
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

/// Fixed asset operations. Money is exchanged as exact decimal strings,
/// dates as ISO strings (YYYY-MM-DD), enums as snake_case strings.
#[pyclass]
pub struct FixedAssets {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl FixedAssets {
    /// Whether the fixed-assets backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.fixed_assets().is_supported())
    }

    /// Create a fixed asset (draft).
    ///
    /// `category` is one of land, building, machinery, equipment, vehicle,
    /// furniture_and_fixtures, computer_hardware, software,
    /// leasehold_improvement, other. `depreciation_method` is straight_line,
    /// declining_balance (requires `declining_balance_rate`), or
    /// units_of_production.
    #[pyo3(signature = (
        name,
        category,
        acquisition_date,
        acquisition_cost,
        salvage_value,
        useful_life_months,
        depreciation_method,
        asset_number=None,
        description=None,
        declining_balance_rate=None,
        in_service_date=None,
        location_id=None,
        asset_account_id=None,
        accumulated_depreciation_account_id=None,
        depreciation_expense_account_id=None,
        currency=None,
    ))]
    fn create(
        &self,
        name: String,
        category: String,
        acquisition_date: String,
        acquisition_cost: String,
        salvage_value: String,
        useful_life_months: u32,
        depreciation_method: String,
        asset_number: Option<String>,
        description: Option<String>,
        declining_balance_rate: Option<String>,
        in_service_date: Option<String>,
        location_id: Option<String>,
        asset_account_id: Option<String>,
        accumulated_depreciation_account_id: Option<String>,
        depreciation_expense_account_id: Option<String>,
        currency: Option<String>,
    ) -> PyResult<FixedAsset> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let category = category
            .parse::<stateset_core::FixedAssetCategory>()
            .map_err(|_| PyValueError::new_err("Invalid fixed asset category"))?;
        let depreciation_method =
            parse_depreciation_method_py(&depreciation_method, declining_balance_rate.as_deref())?;
        let currency = currency
            .map(|s| {
                s.parse::<CurrencyCode>()
                    .map_err(|_| PyValueError::new_err("Invalid currency code"))
            })
            .transpose()?;
        let asset = commerce
            .fixed_assets()
            .create(stateset_core::CreateFixedAsset {
                asset_number,
                name,
                description,
                category,
                acquisition_date: parse_iso_date_py(&acquisition_date, "acquisition_date")?,
                acquisition_cost: parse_decimal_py(&acquisition_cost, "acquisition_cost")?,
                salvage_value: parse_decimal_py(&salvage_value, "salvage_value")?,
                useful_life_months,
                depreciation_method,
                in_service_date: in_service_date
                    .as_deref()
                    .map(|s| parse_iso_date_py(s, "in_service_date"))
                    .transpose()?,
                location_id: parse_optional_uuid_py(location_id, "location_id")?,
                asset_account_id: parse_optional_uuid_py(asset_account_id, "asset_account_id")?,
                accumulated_depreciation_account_id: parse_optional_uuid_py(
                    accumulated_depreciation_account_id,
                    "accumulated_depreciation_account_id",
                )?,
                depreciation_expense_account_id: parse_optional_uuid_py(
                    depreciation_expense_account_id,
                    "depreciation_expense_account_id",
                )?,
                currency,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create fixed asset: {}", e)))?;
        Ok(asset.into())
    }

    /// Get a fixed asset by ID.
    fn get(&self, id: String) -> PyResult<Option<FixedAsset>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let asset = commerce
            .fixed_assets()
            .get(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get fixed asset: {}", e)))?;
        Ok(asset.map(Into::into))
    }

    /// List fixed assets matching the filter.
    #[pyo3(signature = (
        category=None,
        status=None,
        location_id=None,
        acquired_from=None,
        acquired_to=None,
        search=None,
        limit=None,
        offset=None,
    ))]
    fn list(
        &self,
        category: Option<String>,
        status: Option<String>,
        location_id: Option<String>,
        acquired_from: Option<String>,
        acquired_to: Option<String>,
        search: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<FixedAsset>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let filter = stateset_core::FixedAssetFilter {
            category: category
                .map(|s| {
                    s.parse::<stateset_core::FixedAssetCategory>()
                        .map_err(|_| PyValueError::new_err("Invalid fixed asset category"))
                })
                .transpose()?,
            status: status
                .map(|s| {
                    s.parse::<stateset_core::FixedAssetStatus>()
                        .map_err(|_| PyValueError::new_err("Invalid fixed asset status"))
                })
                .transpose()?,
            location_id: parse_optional_uuid_py(location_id, "location_id")?,
            acquired_from: acquired_from
                .as_deref()
                .map(|s| parse_iso_date_py(s, "acquired_from"))
                .transpose()?,
            acquired_to: acquired_to
                .as_deref()
                .map(|s| parse_iso_date_py(s, "acquired_to"))
                .transpose()?,
            search,
            limit,
            offset,
            after_cursor: None,
        };
        let assets = commerce
            .fixed_assets()
            .list(filter)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list fixed assets: {}", e)))?;
        Ok(assets.into_iter().map(Into::into).collect())
    }

    /// Update a fixed asset's mutable fields.
    #[pyo3(signature = (
        id,
        name=None,
        description=None,
        category=None,
        salvage_value=None,
        useful_life_months=None,
        in_service_date=None,
        location_id=None,
        asset_account_id=None,
        accumulated_depreciation_account_id=None,
        depreciation_expense_account_id=None,
    ))]
    fn update(
        &self,
        id: String,
        name: Option<String>,
        description: Option<String>,
        category: Option<String>,
        salvage_value: Option<String>,
        useful_life_months: Option<u32>,
        in_service_date: Option<String>,
        location_id: Option<String>,
        asset_account_id: Option<String>,
        accumulated_depreciation_account_id: Option<String>,
        depreciation_expense_account_id: Option<String>,
    ) -> PyResult<FixedAsset> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let category = category
            .map(|s| {
                s.parse::<stateset_core::FixedAssetCategory>()
                    .map_err(|_| PyValueError::new_err("Invalid fixed asset category"))
            })
            .transpose()?;
        let asset = commerce
            .fixed_assets()
            .update(
                uuid,
                stateset_core::UpdateFixedAsset {
                    name,
                    description,
                    category,
                    salvage_value: salvage_value
                        .as_deref()
                        .map(|s| parse_decimal_py(s, "salvage_value"))
                        .transpose()?,
                    useful_life_months,
                    in_service_date: in_service_date
                        .as_deref()
                        .map(|s| parse_iso_date_py(s, "in_service_date"))
                        .transpose()?,
                    location_id: parse_optional_uuid_py(location_id, "location_id")?,
                    asset_account_id: parse_optional_uuid_py(asset_account_id, "asset_account_id")?,
                    accumulated_depreciation_account_id: parse_optional_uuid_py(
                        accumulated_depreciation_account_id,
                        "accumulated_depreciation_account_id",
                    )?,
                    depreciation_expense_account_id: parse_optional_uuid_py(
                        depreciation_expense_account_id,
                        "depreciation_expense_account_id",
                    )?,
                },
            )
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to update fixed asset: {}", e)))?;
        Ok(asset.into())
    }

    /// Place a draft asset in service on the given ISO date (YYYY-MM-DD).
    fn place_in_service(&self, id: String, date: String) -> PyResult<FixedAsset> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let date = parse_iso_date_py(&date, "date")?;
        let asset = commerce.fixed_assets().place_in_service(uuid, date).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to place asset in service: {}", e))
        })?;
        Ok(asset.into())
    }

    /// Dispose of an asset for the given proceeds (exact decimal string),
    /// recording gain/loss. `date` is an ISO date (YYYY-MM-DD); defaults to
    /// today.
    #[pyo3(signature = (id, proceeds, date=None, notes=None))]
    fn dispose(
        &self,
        id: String,
        proceeds: String,
        date: Option<String>,
        notes: Option<String>,
    ) -> PyResult<FixedAsset> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let proceeds = parse_decimal_py(&proceeds, "proceeds")?;
        let date = date
            .as_deref()
            .map(|s| parse_iso_date_py(s, "date"))
            .transpose()?
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let asset = commerce.fixed_assets().dispose(uuid, date, proceeds, notes).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to dispose fixed asset: {}", e))
        })?;
        Ok(asset.into())
    }

    /// Write off an asset (disposal with zero proceeds). `date` is an ISO
    /// date (YYYY-MM-DD); defaults to today.
    #[pyo3(signature = (id, date=None, notes=None))]
    fn write_off(
        &self,
        id: String,
        date: Option<String>,
        notes: Option<String>,
    ) -> PyResult<FixedAsset> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let date = date
            .as_deref()
            .map(|s| parse_iso_date_py(s, "date"))
            .transpose()?
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let asset = commerce.fixed_assets().write_off(uuid, date, notes).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to write off fixed asset: {}", e))
        })?;
        Ok(asset.into())
    }

    /// Generate and persist the depreciation schedule for an asset.
    fn generate_schedule(&self, id: String) -> PyResult<DepreciationSchedule> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let schedule = commerce
            .fixed_assets()
            .generate_schedule(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to generate schedule: {}", e)))?;
        Ok(schedule.into())
    }

    /// Get the persisted depreciation schedule for an asset, if generated.
    fn get_schedule(&self, id: String) -> PyResult<Option<DepreciationSchedule>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let schedule = commerce
            .fixed_assets()
            .get_schedule(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get schedule: {}", e)))?;
        Ok(schedule.map(Into::into))
    }

    /// Post the next `periods` scheduled depreciation entries.
    fn post_depreciation(&self, id: String, periods: u32) -> PyResult<FixedAsset> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let asset = commerce
            .fixed_assets()
            .post_depreciation(uuid, periods)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to post depreciation: {}", e)))?;
        Ok(asset.into())
    }
}

// ============================================================================
// Revenue Recognition  (all monetary values cross as exact decimal strings)
// ============================================================================

/// Input for a performance obligation under a revenue contract.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct PerformanceObligationInput {
    #[pyo3(get, set)]
    description: String,
    /// Exact decimal string; obligations must sum to the transaction price
    #[pyo3(get, set)]
    allocated_amount: String,
    /// point_in_time, ratable_over_time, milestone
    #[pyo3(get, set)]
    recognition_method: String,
    /// Exact decimal string
    #[pyo3(get, set)]
    standalone_selling_price: Option<String>,
    /// ISO date (YYYY-MM-DD); required for ratable_over_time
    #[pyo3(get, set)]
    recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); required for ratable_over_time
    #[pyo3(get, set)]
    recognition_end: Option<String>,
}

#[pymethods]
impl PerformanceObligationInput {
    #[new]
    #[pyo3(signature = (
        description,
        allocated_amount,
        recognition_method,
        standalone_selling_price=None,
        recognition_start=None,
        recognition_end=None,
    ))]
    fn new(
        description: String,
        allocated_amount: String,
        recognition_method: String,
        standalone_selling_price: Option<String>,
        recognition_start: Option<String>,
        recognition_end: Option<String>,
    ) -> Self {
        Self {
            description,
            allocated_amount,
            recognition_method,
            standalone_selling_price,
            recognition_start,
            recognition_end,
        }
    }
}

impl PerformanceObligationInput {
    fn into_core(self) -> PyResult<stateset_core::CreatePerformanceObligation> {
        Ok(stateset_core::CreatePerformanceObligation {
            description: self.description,
            standalone_selling_price: self
                .standalone_selling_price
                .as_deref()
                .map(|s| parse_decimal_py(s, "standalone_selling_price"))
                .transpose()?,
            allocated_amount: parse_decimal_py(&self.allocated_amount, "allocated_amount")?,
            recognition_method: parse_recognition_method_py(
                &self.recognition_method,
                self.recognition_start.as_deref(),
                self.recognition_end.as_deref(),
            )?,
        })
    }
}

/// A performance obligation. Money values are exact decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct PerformanceObligation {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    contract_id: String,
    #[pyo3(get)]
    description: String,
    /// Exact decimal string
    #[pyo3(get)]
    standalone_selling_price: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    allocated_amount: String,
    /// point_in_time, ratable_over_time, milestone
    #[pyo3(get)]
    recognition_method: String,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    #[pyo3(get)]
    recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    #[pyo3(get)]
    recognition_end: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    recognized_amount: String,
    /// Exact decimal string: allocated_amount - recognized_amount
    #[pyo3(get)]
    deferred_amount: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::PerformanceObligation> for PerformanceObligation {
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

/// A revenue contract (ASC 606). Money values are exact decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct RevenueContract {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    contract_number: String,
    #[pyo3(get)]
    customer_id: String,
    #[pyo3(get)]
    order_id: Option<String>,
    #[pyo3(get)]
    invoice_id: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    transaction_price: String,
    #[pyo3(get)]
    currency: String,
    /// draft, active, completed, cancelled
    #[pyo3(get)]
    status: String,
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    effective_date: String,
    #[pyo3(get)]
    obligations: Vec<PerformanceObligation>,
    /// Exact decimal string: total recognized across obligations
    #[pyo3(get)]
    total_recognized: String,
    /// Exact decimal string: transaction_price - total_recognized
    #[pyo3(get)]
    deferred_balance: String,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
}

impl From<stateset_core::RevenueContract> for RevenueContract {
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

/// One entry in a revenue recognition schedule.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct RevenueScheduleEntry {
    #[pyo3(get)]
    period: u32,
    /// ISO date (YYYY-MM-DD): first day of the entry's month
    #[pyo3(get)]
    period_start: String,
    /// Exact decimal string
    #[pyo3(get)]
    amount: String,
    /// deferred or recognized
    #[pyo3(get)]
    status: String,
}

impl From<stateset_core::RevenueScheduleEntry> for RevenueScheduleEntry {
    fn from(e: stateset_core::RevenueScheduleEntry) -> Self {
        Self {
            period: e.period,
            period_start: e.period_start.to_string(),
            amount: e.amount.to_string(),
            status: format!("{}", e.status),
        }
    }
}

/// A revenue recognition schedule for an obligation.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct RevenueSchedule {
    #[pyo3(get)]
    obligation_id: String,
    /// point_in_time, ratable_over_time, milestone
    #[pyo3(get)]
    method: String,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    #[pyo3(get)]
    recognition_start: Option<String>,
    /// ISO date (YYYY-MM-DD); set for ratable_over_time
    #[pyo3(get)]
    recognition_end: Option<String>,
    #[pyo3(get)]
    entries: Vec<RevenueScheduleEntry>,
    /// Exact decimal string
    #[pyo3(get)]
    total_amount: String,
    /// Exact decimal string: sum of recognized entries
    #[pyo3(get)]
    recognized_total: String,
    /// Exact decimal string: sum of deferred entries
    #[pyo3(get)]
    deferred_total: String,
}

impl From<stateset_core::RevenueSchedule> for RevenueSchedule {
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

/// Revenue recognition (ASC 606) operations. Money is exchanged as exact
/// decimal strings; dates as ISO strings; enums as snake_case strings.
#[pyclass]
pub struct RevenueRecognition {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl RevenueRecognition {
    /// Whether the revenue-recognition backend is available on this engine build.
    fn is_supported(&self) -> PyResult<bool> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        Ok(commerce.revenue_recognition().is_supported())
    }

    /// Create a revenue contract with its performance obligations.
    #[pyo3(signature = (
        customer_id,
        transaction_price,
        effective_date,
        obligations,
        contract_number=None,
        order_id=None,
        invoice_id=None,
        currency=None,
    ))]
    fn create_contract(
        &self,
        customer_id: String,
        transaction_price: String,
        effective_date: String,
        obligations: Vec<PerformanceObligationInput>,
        contract_number: Option<String>,
        order_id: Option<String>,
        invoice_id: Option<String>,
        currency: Option<String>,
    ) -> PyResult<RevenueContract> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let customer_id: uuid::Uuid =
            customer_id.parse().map_err(|_| PyValueError::new_err("Invalid customer UUID"))?;
        let currency = currency
            .map(|s| {
                s.parse::<CurrencyCode>()
                    .map_err(|_| PyValueError::new_err("Invalid currency code"))
            })
            .transpose()?;
        let obligations = obligations
            .into_iter()
            .map(PerformanceObligationInput::into_core)
            .collect::<PyResult<Vec<_>>>()?;
        let contract = commerce
            .revenue_recognition()
            .create_contract(stateset_core::CreateRevenueContract {
                contract_number,
                customer_id,
                order_id: parse_optional_uuid_py(order_id, "order_id")?,
                invoice_id: parse_optional_uuid_py(invoice_id, "invoice_id")?,
                transaction_price: parse_decimal_py(&transaction_price, "transaction_price")?,
                currency,
                effective_date: parse_iso_date_py(&effective_date, "effective_date")?,
                obligations,
            })
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create revenue contract: {}", e))
            })?;
        Ok(contract.into())
    }

    /// Get a revenue contract (with obligations) by ID.
    fn get_contract(&self, id: String) -> PyResult<Option<RevenueContract>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let contract = commerce.revenue_recognition().get_contract(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to get revenue contract: {}", e))
        })?;
        Ok(contract.map(Into::into))
    }

    /// List revenue contracts matching the filter.
    #[pyo3(signature = (
        customer_id=None,
        order_id=None,
        invoice_id=None,
        status=None,
        effective_from=None,
        effective_to=None,
        search=None,
        limit=None,
        offset=None,
    ))]
    fn list_contracts(
        &self,
        customer_id: Option<String>,
        order_id: Option<String>,
        invoice_id: Option<String>,
        status: Option<String>,
        effective_from: Option<String>,
        effective_to: Option<String>,
        search: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<RevenueContract>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let filter = stateset_core::RevenueContractFilter {
            customer_id: parse_optional_uuid_py(customer_id, "customer_id")?,
            order_id: parse_optional_uuid_py(order_id, "order_id")?,
            invoice_id: parse_optional_uuid_py(invoice_id, "invoice_id")?,
            status: status
                .map(|s| {
                    s.parse::<stateset_core::RevenueContractStatus>()
                        .map_err(|_| PyValueError::new_err("Invalid revenue contract status"))
                })
                .transpose()?,
            effective_from: effective_from
                .as_deref()
                .map(|s| parse_iso_date_py(s, "effective_from"))
                .transpose()?,
            effective_to: effective_to
                .as_deref()
                .map(|s| parse_iso_date_py(s, "effective_to"))
                .transpose()?,
            search,
            limit,
            offset,
            after_cursor: None,
        };
        let contracts = commerce.revenue_recognition().list_contracts(filter).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to list revenue contracts: {}", e))
        })?;
        Ok(contracts.into_iter().map(Into::into).collect())
    }

    /// Update a revenue contract (status transitions are guarded).
    #[pyo3(signature = (id, order_id=None, invoice_id=None, status=None, effective_date=None))]
    fn update_contract(
        &self,
        id: String,
        order_id: Option<String>,
        invoice_id: Option<String>,
        status: Option<String>,
        effective_date: Option<String>,
    ) -> PyResult<RevenueContract> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let status = status
            .map(|s| {
                s.parse::<stateset_core::RevenueContractStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid revenue contract status"))
            })
            .transpose()?;
        let contract = commerce
            .revenue_recognition()
            .update_contract(
                uuid,
                stateset_core::UpdateRevenueContract {
                    order_id: parse_optional_uuid_py(order_id, "order_id")?,
                    invoice_id: parse_optional_uuid_py(invoice_id, "invoice_id")?,
                    status,
                    effective_date: effective_date
                        .as_deref()
                        .map(|s| parse_iso_date_py(s, "effective_date"))
                        .transpose()?,
                },
            )
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to update revenue contract: {}", e))
            })?;
        Ok(contract.into())
    }

    /// List the performance obligations under a contract.
    fn list_obligations(&self, contract_id: String) -> PyResult<Vec<PerformanceObligation>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            contract_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let obligations = commerce
            .revenue_recognition()
            .list_obligations(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list obligations: {}", e)))?;
        Ok(obligations.into_iter().map(Into::into).collect())
    }

    /// Generate and persist the recognition schedule for an obligation.
    fn generate_schedule(&self, obligation_id: String) -> PyResult<RevenueSchedule> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let schedule = commerce
            .revenue_recognition()
            .generate_schedule(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to generate schedule: {}", e)))?;
        Ok(schedule.into())
    }

    /// Get the persisted recognition schedule for an obligation, if generated.
    fn get_schedule(&self, obligation_id: String) -> PyResult<Option<RevenueSchedule>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let schedule = commerce
            .revenue_recognition()
            .get_schedule(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get schedule: {}", e)))?;
        Ok(schedule.map(Into::into))
    }

    /// Recognize deferred entries with a period start on or before `through`
    /// (ISO date, YYYY-MM-DD).
    fn recognize(&self, obligation_id: String, through: String) -> PyResult<RevenueSchedule> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid =
            obligation_id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let through = parse_iso_date_py(&through, "through")?;
        let schedule = commerce
            .revenue_recognition()
            .recognize_period(uuid, through)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to recognize revenue: {}", e)))?;
        Ok(schedule.into())
    }
}

// ============================================================================
// Cycle Counts  (quantities cross as exact decimal strings)
// ============================================================================

/// Input for an expected cycle count line.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct CycleCountLineInput {
    #[pyo3(get, set)]
    sku: String,
    /// Exact decimal string
    #[pyo3(get, set)]
    expected_quantity: String,
    #[pyo3(get, set)]
    lot_id: Option<String>,
}

#[pymethods]
impl CycleCountLineInput {
    #[new]
    #[pyo3(signature = (sku, expected_quantity, lot_id=None))]
    fn new(sku: String, expected_quantity: String, lot_id: Option<String>) -> Self {
        Self { sku, expected_quantity, lot_id }
    }
}

/// Input for recording a physical count against a line.
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct RecordCycleCountLineInput {
    #[pyo3(get, set)]
    sku: String,
    /// Exact decimal string
    #[pyo3(get, set)]
    counted_quantity: String,
    #[pyo3(get, set)]
    lot_id: Option<String>,
}

#[pymethods]
impl RecordCycleCountLineInput {
    #[new]
    #[pyo3(signature = (sku, counted_quantity, lot_id=None))]
    fn new(sku: String, counted_quantity: String, lot_id: Option<String>) -> Self {
        Self { sku, counted_quantity, lot_id }
    }
}

/// One line of a cycle count. Quantities are exact decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CycleCountLine {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    cycle_count_id: String,
    #[pyo3(get)]
    sku: String,
    #[pyo3(get)]
    lot_id: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    expected_quantity: String,
    /// Exact decimal string
    #[pyo3(get)]
    counted_quantity: Option<String>,
    /// Exact decimal string: counted_quantity - expected_quantity
    #[pyo3(get)]
    variance: Option<String>,
}

impl From<stateset_core::CycleCountLine> for CycleCountLine {
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

/// A cycle count with its lines.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CycleCount {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    warehouse_id: i32,
    #[pyo3(get)]
    location_id: Option<i32>,
    /// draft, in_progress, completed, cancelled
    #[pyo3(get)]
    status: String,
    /// RFC 3339 timestamp
    #[pyo3(get)]
    scheduled_date: Option<String>,
    #[pyo3(get)]
    counted_by: Option<String>,
    #[pyo3(get)]
    lines: Vec<CycleCountLine>,
    #[pyo3(get)]
    created_at: String,
    #[pyo3(get)]
    updated_at: String,
    #[pyo3(get)]
    completed_at: Option<String>,
}

impl From<stateset_core::CycleCount> for CycleCount {
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

/// Cycle count operations. Quantities are exchanged as exact decimal
/// strings; enums as snake_case strings; timestamps as RFC 3339 strings.
#[pyclass]
pub struct CycleCounts {
    commerce: Arc<Mutex<RustCommerce>>,
}

#[pymethods]
impl CycleCounts {
    /// Create a cycle count (draft) with its expected lines.
    #[pyo3(signature = (warehouse_id, lines, location_id=None, scheduled_date=None, counted_by=None))]
    fn create(
        &self,
        warehouse_id: i32,
        lines: Vec<CycleCountLineInput>,
        location_id: Option<i32>,
        scheduled_date: Option<String>,
        counted_by: Option<String>,
    ) -> PyResult<CycleCount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let scheduled_date = match scheduled_date.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        PyValueError::new_err("Invalid scheduled_date RFC 3339 timestamp")
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let lines = lines
            .into_iter()
            .map(|l| -> PyResult<stateset_core::CreateCycleCountLine> {
                Ok(stateset_core::CreateCycleCountLine {
                    sku: l.sku,
                    lot_id: parse_optional_uuid_py(l.lot_id, "lot_id")?,
                    expected_quantity: parse_decimal_py(&l.expected_quantity, "expected_quantity")?,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let count = commerce
            .warehouse()
            .create_cycle_count(stateset_core::CreateCycleCount {
                warehouse_id,
                location_id,
                scheduled_date,
                counted_by,
                lines,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create cycle count: {}", e)))?;
        Ok(count.into())
    }

    /// Get a cycle count (with lines) by ID.
    fn get(&self, id: String) -> PyResult<Option<CycleCount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .get_cycle_count(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get cycle count: {}", e)))?;
        Ok(count.map(Into::into))
    }

    /// List cycle counts matching the filter.
    #[pyo3(signature = (warehouse_id=None, location_id=None, status=None, limit=None, offset=None))]
    fn list(
        &self,
        warehouse_id: Option<i32>,
        location_id: Option<i32>,
        status: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<Vec<CycleCount>> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let status = status
            .map(|s| {
                s.parse::<stateset_core::CycleCountStatus>()
                    .map_err(|_| PyValueError::new_err("Invalid cycle count status"))
            })
            .transpose()?;
        let counts = commerce
            .warehouse()
            .list_cycle_counts(stateset_core::CycleCountFilter {
                warehouse_id,
                location_id,
                status,
                limit,
                offset,
                after_cursor: None,
            })
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to list cycle counts: {}", e)))?;
        Ok(counts.into_iter().map(Into::into).collect())
    }

    /// Start a draft cycle count (draft -> in_progress).
    fn start(&self, id: String) -> PyResult<CycleCount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .start_cycle_count(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to start cycle count: {}", e)))?;
        Ok(count.into())
    }

    /// Record physical counts against an in-progress cycle count.
    fn record_counts(
        &self,
        id: String,
        counts: Vec<RecordCycleCountLineInput>,
    ) -> PyResult<CycleCount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let counts = counts
            .into_iter()
            .map(|c| -> PyResult<stateset_core::RecordCycleCountLine> {
                Ok(stateset_core::RecordCycleCountLine {
                    sku: c.sku,
                    lot_id: parse_optional_uuid_py(c.lot_id, "lot_id")?,
                    counted_quantity: parse_decimal_py(&c.counted_quantity, "counted_quantity")?,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let count = commerce.warehouse().record_cycle_counts(uuid, counts).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to record cycle counts: {}", e))
        })?;
        Ok(count.into())
    }

    /// Complete an in-progress cycle count, applying variance adjustments.
    fn complete(&self, id: String) -> PyResult<CycleCount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let count = commerce.warehouse().complete_cycle_count(uuid).map_err(|e| {
            PyRuntimeError::new_err(format!("Failed to complete cycle count: {}", e))
        })?;
        Ok(count.into())
    }

    /// Cancel a draft or in-progress cycle count. No adjustments are applied.
    fn cancel(&self, id: String) -> PyResult<CycleCount> {
        let commerce = self
            .commerce
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?;
        let uuid: uuid::Uuid = id.parse().map_err(|_| PyValueError::new_err("Invalid UUID"))?;
        let count = commerce
            .warehouse()
            .cancel_cycle_count(uuid)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to cancel cycle count: {}", e)))?;
        Ok(count.into())
    }
}

// ============================================================================
// Three-Way Match (Accounts Payable)
// ============================================================================

/// One line of a three-way match. Quantities and costs are decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ThreeWayMatchLine {
    #[pyo3(get)]
    po_line_id: Option<String>,
    #[pyo3(get)]
    bill_item_id: String,
    #[pyo3(get)]
    description: String,
    /// Exact decimal string
    #[pyo3(get)]
    ordered_quantity: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    ordered_unit_cost: Option<String>,
    /// Exact decimal string
    #[pyo3(get)]
    received_quantity: String,
    /// Exact decimal string
    #[pyo3(get)]
    billed_quantity: String,
    /// Exact decimal string
    #[pyo3(get)]
    billed_unit_cost: String,
    /// Exact decimal string: billed_quantity - received_quantity
    #[pyo3(get)]
    quantity_variance: String,
    /// Exact decimal string: billed_unit_cost - ordered_unit_cost
    #[pyo3(get)]
    price_variance: String,
    #[pyo3(get)]
    matched: bool,
    #[pyo3(get)]
    issues: Vec<String>,
}

impl From<stateset_core::ThreeWayMatchLine> for ThreeWayMatchLine {
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

/// Result of a three-way match run.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct ThreeWayMatchResult {
    /// Overall status: not_required, pending, matched, variance
    #[pyo3(get)]
    match_status: String,
    /// Number of variance lines (set when match_status is "variance")
    #[pyo3(get)]
    variance_line_count: Option<u32>,
    /// Tolerance applied, as an exact decimal string percentage (e.g. "5")
    #[pyo3(get)]
    tolerance_percent: String,
    #[pyo3(get)]
    lines: Vec<ThreeWayMatchLine>,
}

impl From<stateset_core::ThreeWayMatchResult> for ThreeWayMatchResult {
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

// ============================================================================
// General Ledger: periods, FX revaluation, month-end close
// ============================================================================

/// An accounting period.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct GlPeriod {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    period_name: String,
    #[pyo3(get)]
    fiscal_year: i32,
    #[pyo3(get)]
    period_number: i32,
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    start_date: String,
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    end_date: String,
    /// One of `future`, `open`, `closed`, `locked`
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    closed_by: Option<String>,
}

impl From<stateset_core::GlPeriod> for GlPeriod {
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

/// One revalued account line. Money values are exact decimal strings.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct RevaluationLine {
    #[pyo3(get)]
    account_id: String,
    #[pyo3(get)]
    account_number: String,
    #[pyo3(get)]
    account_name: String,
    #[pyo3(get)]
    currency: String,
    /// Side that increases this account: debit or credit
    #[pyo3(get)]
    normal_balance: String,
    /// Exact decimal string
    #[pyo3(get)]
    foreign_balance: String,
    /// Exact decimal string
    #[pyo3(get)]
    carrying_value: String,
    /// Exact decimal string
    #[pyo3(get)]
    rate: String,
    /// Exact decimal string
    #[pyo3(get)]
    revalued_value: String,
    /// Exact decimal string
    #[pyo3(get)]
    adjustment: String,
    /// Exact decimal string
    #[pyo3(get)]
    unrealized_gain_loss: String,
}

impl From<stateset_core::RevaluationLine> for RevaluationLine {
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

/// Result of an FX revaluation run.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct RevaluationResult {
    /// ISO date (YYYY-MM-DD)
    #[pyo3(get)]
    as_of_date: String,
    #[pyo3(get)]
    base_currency: String,
    /// Exact decimal string
    #[pyo3(get)]
    total_unrealized_gain_loss: String,
    #[pyo3(get)]
    lines: Vec<RevaluationLine>,
    /// Balanced adjusting entry; None when no adjustment was required.
    #[pyo3(get)]
    journal_entry: Option<JournalEntry>,
}

impl From<stateset_core::RevaluationResult> for RevaluationResult {
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

/// One step of a month-end close run.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CloseMonthStep {
    /// One of `executed`, `skipped`, `dry_run`
    #[pyo3(get)]
    status: String,
    /// Entries posted (or that would be posted in a dry run)
    #[pyo3(get)]
    entry_count: i64,
    /// Exact decimal string
    #[pyo3(get)]
    total_amount: String,
    /// Per-item failures that did not abort the close
    #[pyo3(get)]
    warnings: Vec<String>,
}

impl From<stateset_core::CloseMonthStepReport> for CloseMonthStep {
    fn from(step: stateset_core::CloseMonthStepReport) -> Self {
        Self {
            status: step.status.to_string(),
            entry_count: i64::try_from(step.entry_count).unwrap_or(i64::MAX),
            total_amount: step.total_amount.to_string(),
            warnings: step.warnings,
        }
    }
}

/// Report from a month-end close run.
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub struct CloseMonthReport {
    #[pyo3(get)]
    period_id: String,
    #[pyo3(get)]
    period_name: String,
    #[pyo3(get)]
    dry_run: bool,
    /// Step 1: scheduled depreciation due through period end
    #[pyo3(get)]
    depreciation: CloseMonthStep,
    /// Step 2: deferred revenue recognized through period end
    #[pyo3(get)]
    revenue_recognition: CloseMonthStep,
    /// Step 3: FX revaluation as of period end
    #[pyo3(get)]
    fx_revaluation: CloseMonthStep,
    /// Step 4: closing entries + close period
    #[pyo3(get)]
    period_close: CloseMonthStep,
    /// Posted closing entry; None for dry runs or skipped closes
    #[pyo3(get)]
    closing_entry: Option<JournalEntry>,
    /// Period status after the run (`closed` after a real close)
    #[pyo3(get)]
    period_status: String,
}

impl From<stateset_core::CloseMonthReport> for CloseMonthReport {
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
