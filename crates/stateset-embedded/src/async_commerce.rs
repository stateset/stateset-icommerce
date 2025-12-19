//! Async Commerce API for PostgreSQL
//!
//! This module provides async access to commerce operations when using PostgreSQL.
//! All methods are truly async (no blocking).
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{AsyncCommerce, CreateOrder, CreateOrderItem};
//! use rust_decimal_macros::dec;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let commerce = AsyncCommerce::connect("postgres://localhost/stateset").await?;
//!
//!     let order = commerce.orders().create(CreateOrder {
//!         customer_id: uuid::Uuid::new_v4(),
//!         items: vec![CreateOrderItem {
//!             sku: "SKU-001".into(),
//!             name: "Widget".into(),
//!             quantity: 2,
//!             unit_price: dec!(29.99),
//!             ..Default::default()
//!         }],
//!         ..Default::default()
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    // Order types
    CreateOrder, CreateOrderItem, Order, OrderFilter, OrderItem, OrderStatus, UpdateOrder,
    // Inventory types
    AdjustInventory, CreateInventoryItem, InventoryBalance, InventoryFilter, InventoryItem,
    InventoryReservation, InventoryTransaction, ReserveInventory, StockLevel,
    // Customer types
    CreateCustomer, CreateCustomerAddress, Customer, CustomerAddress, CustomerFilter,
    UpdateCustomer,
    // Product types
    CreateProduct, CreateProductVariant, Product, ProductFilter, ProductVariant, UpdateProduct,
    // Return types
    CreateReturn, Return, ReturnFilter, UpdateReturn,
    // Shipment types
    AddShipmentEvent, CreateShipment, CreateShipmentItem, Shipment, ShipmentEvent, ShipmentFilter,
    ShipmentItem, UpdateShipment,
    // Payment types
    CreatePayment, CreatePaymentMethod, CreateRefund, Payment, PaymentFilter, PaymentMethod,
    Refund, UpdatePayment,
    // Warranty types
    ClaimResolution, CreateWarranty, CreateWarrantyClaim, UpdateWarranty, UpdateWarrantyClaim,
    Warranty, WarrantyClaim, WarrantyClaimFilter, WarrantyFilter,
    // BOM types
    BillOfMaterials, BomComponent, BomFilter, CreateBom, CreateBomComponent, UpdateBom,
    // Work Order types
    AddWorkOrderMaterial, CreateWorkOrder, CreateWorkOrderTask, UpdateWorkOrder,
    UpdateWorkOrderTask, WorkOrder, WorkOrderFilter, WorkOrderMaterial, WorkOrderTask,
    // Purchase Order types
    CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, PurchaseOrder,
    PurchaseOrderFilter, PurchaseOrderItem, ReceivePurchaseOrderItems, Supplier, SupplierFilter,
    UpdatePurchaseOrder, UpdateSupplier,
    // Invoice types
    CreateInvoice, CreateInvoiceItem, Invoice, InvoiceFilter, InvoiceItem, RecordInvoicePayment,
    UpdateInvoice,
    // Cart types
    AddCartItem, Cart, CartAddress, CartFilter, CartItem, CheckoutResult, CreateCart,
    SetCartPayment, SetCartShipping, ShippingRate, UpdateCart, UpdateCartItem,
    // Analytics types
    AnalyticsQuery, CustomerMetrics, DemandForecast, FulfillmentMetrics, InventoryHealth,
    InventoryMovement, LowStockItem, OrderStatusBreakdown, ProductPerformance, ReturnMetrics,
    RevenueByPeriod, RevenueForecast, SalesSummary, TimeGranularity, TopCustomer, TopProduct,
    // Currency types
    ConversionResult, ConvertCurrency, Currency, ExchangeRate, ExchangeRateFilter,
    SetExchangeRate, StoreCurrencySettings,
    // Error types
    Result,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

/// Async commerce interface for PostgreSQL.
///
/// This provides a fully async API for PostgreSQL users who want to avoid
/// blocking operations. All methods are `async` and execute without blocking.
///
/// # Example
///
/// ```rust,ignore
/// use stateset_embedded::AsyncCommerce;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let commerce = AsyncCommerce::connect("postgres://localhost/stateset").await?;
///
///     // All operations are async
///     let orders = commerce.orders().list(Default::default()).await?;
///
///     Ok(())
/// }
/// ```
pub struct AsyncCommerce {
    db: Arc<PostgresDatabase>,
}

impl AsyncCommerce {
    /// Connect to PostgreSQL and create an async commerce instance.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string (e.g., "postgres://user:pass@localhost/db")
    pub async fn connect(url: &str) -> Result<Self> {
        let db = PostgresDatabase::connect(url).await?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Connect with custom options.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string
    /// * `max_connections` - Maximum number of connections in the pool
    /// * `acquire_timeout_secs` - Timeout in seconds for acquiring a connection
    pub async fn connect_with_options(
        url: &str,
        max_connections: u32,
        acquire_timeout_secs: u64,
    ) -> Result<Self> {
        let db = PostgresDatabase::connect_with_options(url, max_connections, acquire_timeout_secs)
            .await?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Create from an existing PostgresDatabase instance.
    pub fn from_database(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Access async order operations.
    pub fn orders(&self) -> AsyncOrders {
        AsyncOrders::new(self.db.clone())
    }

    /// Access async inventory operations.
    pub fn inventory(&self) -> AsyncInventory {
        AsyncInventory::new(self.db.clone())
    }

    /// Access async customer operations.
    pub fn customers(&self) -> AsyncCustomers {
        AsyncCustomers::new(self.db.clone())
    }

    /// Access async product operations.
    pub fn products(&self) -> AsyncProducts {
        AsyncProducts::new(self.db.clone())
    }

    /// Access async return operations.
    pub fn returns(&self) -> AsyncReturns {
        AsyncReturns::new(self.db.clone())
    }

    /// Access async shipment operations.
    pub fn shipments(&self) -> AsyncShipments {
        AsyncShipments::new(self.db.clone())
    }

    /// Access async payment operations.
    pub fn payments(&self) -> AsyncPayments {
        AsyncPayments::new(self.db.clone())
    }

    /// Access async warranty operations.
    pub fn warranties(&self) -> AsyncWarranties {
        AsyncWarranties::new(self.db.clone())
    }

    /// Access async BOM operations.
    pub fn bom(&self) -> AsyncBom {
        AsyncBom::new(self.db.clone())
    }

    /// Access async work order operations.
    pub fn work_orders(&self) -> AsyncWorkOrders {
        AsyncWorkOrders::new(self.db.clone())
    }

    /// Access async purchase order operations.
    pub fn purchase_orders(&self) -> AsyncPurchaseOrders {
        AsyncPurchaseOrders::new(self.db.clone())
    }

    /// Access async invoice operations.
    pub fn invoices(&self) -> AsyncInvoices {
        AsyncInvoices::new(self.db.clone())
    }

    /// Access async cart operations.
    pub fn carts(&self) -> AsyncCarts {
        AsyncCarts::new(self.db.clone())
    }

    /// Access async analytics operations.
    pub fn analytics(&self) -> AsyncAnalytics {
        AsyncAnalytics::new(self.db.clone())
    }

    /// Access async currency operations.
    pub fn currency(&self) -> AsyncCurrency {
        AsyncCurrency::new(self.db.clone())
    }

    /// Get the underlying database for advanced operations.
    pub fn database(&self) -> &PostgresDatabase {
        &self.db
    }
}

// ============================================================================
// Async Orders
// ============================================================================

/// Async order operations.
pub struct AsyncOrders {
    db: Arc<PostgresDatabase>,
}

impl AsyncOrders {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new order.
    pub async fn create(&self, input: CreateOrder) -> Result<Order> {
        self.db.orders().create_async(input).await
    }

    /// Get an order by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Order>> {
        self.db.orders().get_async(id).await
    }

    /// Get an order by order number.
    pub async fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        self.db.orders().get_by_number_async(order_number).await
    }

    /// Update an order.
    pub async fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        self.db.orders().update_async(id, input).await
    }

    /// Update order status.
    pub async fn update_status(&self, id: Uuid, status: OrderStatus) -> Result<Order> {
        self.db
            .orders()
            .update_async(
                id,
                UpdateOrder {
                    status: Some(status),
                    ..Default::default()
                },
            )
            .await
    }

    /// List orders with optional filtering.
    pub async fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        self.db.orders().list_async(filter).await
    }

    /// List orders for a specific customer.
    pub async fn list_for_customer(&self, customer_id: Uuid) -> Result<Vec<Order>> {
        self.db
            .orders()
            .list_async(OrderFilter {
                customer_id: Some(customer_id),
                ..Default::default()
            })
            .await
    }

    /// Delete an order.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.orders().delete_async(id).await
    }

    /// Add an item to an order.
    pub async fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        self.db.orders().add_item_async(order_id, item).await
    }

    /// Remove an item from an order.
    pub async fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        self.db.orders().remove_item_async(order_id, item_id).await
    }

    /// Count orders matching a filter.
    pub async fn count(&self, filter: OrderFilter) -> Result<u64> {
        self.db.orders().count_async(filter).await
    }

    /// Cancel an order.
    pub async fn cancel(&self, id: Uuid) -> Result<Order> {
        self.update_status(id, OrderStatus::Cancelled).await
    }

    /// Mark an order as shipped.
    pub async fn ship(&self, id: Uuid, tracking_number: Option<&str>) -> Result<Order> {
        self.db
            .orders()
            .update_async(
                id,
                UpdateOrder {
                    status: Some(OrderStatus::Shipped),
                    tracking_number: tracking_number.map(|s| s.to_string()),
                    ..Default::default()
                },
            )
            .await
    }

    /// Mark an order as delivered.
    pub async fn deliver(&self, id: Uuid) -> Result<Order> {
        self.update_status(id, OrderStatus::Delivered).await
    }
}

// ============================================================================
// Async Inventory
// ============================================================================

/// Async inventory operations.
pub struct AsyncInventory {
    db: Arc<PostgresDatabase>,
}

impl AsyncInventory {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new inventory item.
    pub async fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        self.db.inventory().create_item_async(input).await
    }

    /// Get inventory item by ID.
    pub async fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        self.db.inventory().get_item_async(id).await
    }

    /// Get inventory item by SKU.
    pub async fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        self.db.inventory().get_item_by_sku_async(sku).await
    }

    /// Get stock level for a SKU.
    pub async fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        self.db.inventory().get_stock_async(sku).await
    }

    /// Get balance at a specific location.
    pub async fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>> {
        self.db.inventory().get_balance_async(item_id, location_id).await
    }

    /// Adjust inventory quantity.
    pub async fn adjust_inventory(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        self.db.inventory().adjust_async(input).await
    }

    /// Convenience method to adjust inventory by SKU.
    pub async fn adjust(
        &self,
        sku: &str,
        quantity: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<InventoryTransaction> {
        self.adjust_inventory(AdjustInventory {
            sku: sku.to_string(),
            location_id: Some(1), // Default location
            quantity,
            reason: reason.to_string(),
            reference_type: None,
            reference_id: None,
        })
        .await
    }

    /// Reserve inventory.
    pub async fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        self.db.inventory().reserve_async(input).await
    }

    /// Release a reservation.
    pub async fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().release_reservation_async(reservation_id).await
    }

    /// Confirm a reservation.
    pub async fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.inventory().confirm_reservation_async(reservation_id).await
    }

    /// List inventory items.
    pub async fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        self.db.inventory().list_async(filter).await
    }

    /// Get items that need reordering.
    pub async fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        self.db.inventory().get_reorder_needed_async().await
    }

    /// Get transaction history.
    pub async fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        self.db.inventory().get_transactions_async(item_id, limit).await
    }
}

// ============================================================================
// Async Customers
// ============================================================================

/// Async customer operations.
pub struct AsyncCustomers {
    db: Arc<PostgresDatabase>,
}

impl AsyncCustomers {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new customer.
    pub async fn create(&self, input: CreateCustomer) -> Result<Customer> {
        self.db.customers().create_async(input).await
    }

    /// Get customer by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Customer>> {
        self.db.customers().get_async(id).await
    }

    /// Get customer by email.
    pub async fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        self.db.customers().get_by_email_async(email).await
    }

    /// Update a customer.
    pub async fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        self.db.customers().update_async(id, input).await
    }

    /// List customers.
    pub async fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        self.db.customers().list_async(filter).await
    }

    /// Delete a customer.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.customers().delete_async(id).await
    }

    /// Add an address to a customer.
    pub async fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        self.db.customers().add_address_async(input).await
    }

    /// Get addresses for a customer.
    pub async fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        self.db.customers().get_addresses_async(customer_id).await
    }

    /// Count customers.
    pub async fn count(&self, filter: CustomerFilter) -> Result<u64> {
        self.db.customers().count_async(filter).await
    }
}

// ============================================================================
// Async Products
// ============================================================================

/// Async product operations.
pub struct AsyncProducts {
    db: Arc<PostgresDatabase>,
}

impl AsyncProducts {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new product.
    pub async fn create(&self, input: CreateProduct) -> Result<Product> {
        self.db.products().create_async(input).await
    }

    /// Get product by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Product>> {
        self.db.products().get_async(id).await
    }

    /// Get product by slug.
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        self.db.products().get_by_slug_async(slug).await
    }

    /// Update a product.
    pub async fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        self.db.products().update_async(id, input).await
    }

    /// List products.
    pub async fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        self.db.products().list_async(filter).await
    }

    /// Delete a product.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.products().delete_async(id).await
    }

    /// Add a variant to a product.
    pub async fn add_variant(
        &self,
        product_id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        self.db.products().add_variant_public_async(product_id, variant).await
    }

    /// Get variant by ID.
    pub async fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        self.db.products().get_variant_async(id).await
    }

    /// Get variant by SKU.
    pub async fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>> {
        self.db.products().get_variant_by_sku_async(sku).await
    }

    /// Update a variant.
    pub async fn update_variant(
        &self,
        id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        self.db.products().update_variant_async(id, variant).await
    }

    /// Delete a variant.
    pub async fn delete_variant(&self, id: Uuid) -> Result<()> {
        self.db.products().delete_variant_async(id).await
    }

    /// Get all variants for a product.
    pub async fn get_variants(&self, product_id: Uuid) -> Result<Vec<ProductVariant>> {
        self.db.products().get_variants_async(product_id).await
    }

    /// Count products.
    pub async fn count(&self, filter: ProductFilter) -> Result<u64> {
        self.db.products().count_async(filter).await
    }
}

// ============================================================================
// Async Returns
// ============================================================================

/// Async return operations.
pub struct AsyncReturns {
    db: Arc<PostgresDatabase>,
}

impl AsyncReturns {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new return.
    pub async fn create(&self, input: CreateReturn) -> Result<Return> {
        self.db.returns().create_async(input).await
    }

    /// Get return by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Return>> {
        self.db.returns().get_async(id).await
    }

    /// Update a return.
    pub async fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        self.db.returns().update_async(id, input).await
    }

    /// List returns.
    pub async fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        self.db.returns().list_async(filter).await
    }

    /// Approve a return.
    pub async fn approve(&self, id: Uuid) -> Result<Return> {
        self.db.returns().approve_async(id).await
    }

    /// Reject a return.
    pub async fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        self.db.returns().reject_async(id, reason).await
    }

    /// Complete a return.
    pub async fn complete(&self, id: Uuid) -> Result<Return> {
        self.db.returns().complete_async(id).await
    }

    /// Cancel a return.
    pub async fn cancel(&self, id: Uuid) -> Result<Return> {
        self.db.returns().cancel_async(id).await
    }

    /// Count returns.
    pub async fn count(&self, filter: ReturnFilter) -> Result<u64> {
        self.db.returns().count_async(filter).await
    }
}

// ============================================================================
// Async Shipments
// ============================================================================

/// Async shipment operations.
pub struct AsyncShipments {
    db: Arc<PostgresDatabase>,
}

impl AsyncShipments {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new shipment.
    pub async fn create(&self, input: CreateShipment) -> Result<Shipment> {
        self.db.shipments().create_async(input).await
    }

    /// Get shipment by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Shipment>> {
        self.db.shipments().get_async(id).await
    }

    /// Get shipment by shipment number.
    pub async fn get_by_number(&self, shipment_number: &str) -> Result<Option<Shipment>> {
        self.db.shipments().get_by_number_async(shipment_number).await
    }

    /// Get shipment by tracking number.
    pub async fn get_by_tracking(&self, tracking_number: &str) -> Result<Option<Shipment>> {
        self.db.shipments().get_by_tracking_async(tracking_number).await
    }

    /// Update a shipment.
    pub async fn update(&self, id: Uuid, input: UpdateShipment) -> Result<Shipment> {
        self.db.shipments().update_async(id, input).await
    }

    /// List shipments.
    pub async fn list(&self, filter: ShipmentFilter) -> Result<Vec<Shipment>> {
        self.db.shipments().list_async(filter).await
    }

    /// Get shipments for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Shipment>> {
        self.db.shipments().for_order_async(order_id).await
    }

    /// Delete a shipment.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.shipments().delete_async(id).await
    }

    /// Mark shipment as processing.
    pub async fn mark_processing(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_processing_async(id).await
    }

    /// Mark shipment as ready.
    pub async fn mark_ready(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_ready_async(id).await
    }

    /// Ship with optional tracking number.
    pub async fn ship(&self, id: Uuid, tracking_number: Option<String>) -> Result<Shipment> {
        self.db.shipments().ship_async(id, tracking_number).await
    }

    /// Mark shipment as in transit.
    pub async fn mark_in_transit(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_in_transit_async(id).await
    }

    /// Mark shipment as out for delivery.
    pub async fn mark_out_for_delivery(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_out_for_delivery_async(id).await
    }

    /// Mark shipment as delivered.
    pub async fn mark_delivered(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_delivered_async(id).await
    }

    /// Mark shipment as failed.
    pub async fn mark_failed(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().mark_failed_async(id).await
    }

    /// Put shipment on hold.
    pub async fn hold(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().hold_async(id).await
    }

    /// Cancel shipment.
    pub async fn cancel(&self, id: Uuid) -> Result<Shipment> {
        self.db.shipments().cancel_async(id).await
    }

    /// Add item to shipment.
    pub async fn add_item(&self, shipment_id: Uuid, item: CreateShipmentItem) -> Result<ShipmentItem> {
        self.db.shipments().add_item_async(shipment_id, item).await
    }

    /// Remove item from shipment.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.shipments().remove_item_async(item_id).await
    }

    /// Get items in shipment.
    pub async fn get_items(&self, shipment_id: Uuid) -> Result<Vec<ShipmentItem>> {
        self.db.shipments().get_items_async(shipment_id).await
    }

    /// Add tracking event.
    pub async fn add_event(&self, shipment_id: Uuid, event: AddShipmentEvent) -> Result<ShipmentEvent> {
        self.db.shipments().add_event_async(shipment_id, event).await
    }

    /// Get tracking events.
    pub async fn get_events(&self, shipment_id: Uuid) -> Result<Vec<ShipmentEvent>> {
        self.db.shipments().get_events_async(shipment_id).await
    }

    /// Count shipments.
    pub async fn count(&self, filter: ShipmentFilter) -> Result<u64> {
        self.db.shipments().count_async(filter).await
    }
}

// ============================================================================
// Async Payments
// ============================================================================

/// Async payment operations.
pub struct AsyncPayments {
    db: Arc<PostgresDatabase>,
}

impl AsyncPayments {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new payment.
    pub async fn create(&self, input: CreatePayment) -> Result<Payment> {
        self.db.payments().create_async(input).await
    }

    /// Get payment by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Payment>> {
        self.db.payments().get_async(id).await
    }

    /// Get payment by payment number.
    pub async fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        self.db.payments().get_by_number_async(payment_number).await
    }

    /// Get payment by external ID.
    pub async fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        self.db.payments().get_by_external_id_async(external_id).await
    }

    /// Update a payment.
    pub async fn update(&self, id: Uuid, input: UpdatePayment) -> Result<Payment> {
        self.db.payments().update_async(id, input).await
    }

    /// List payments.
    pub async fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        self.db.payments().list_async(filter).await
    }

    /// Get payments for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_order_async(order_id).await
    }

    /// Get payments for an invoice.
    pub async fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_invoice_async(invoice_id).await
    }

    /// Mark payment as processing.
    pub async fn mark_processing(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().mark_processing_async(id).await
    }

    /// Mark payment as completed.
    pub async fn mark_completed(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().mark_completed_async(id).await
    }

    /// Mark payment as failed.
    pub async fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
        self.db.payments().mark_failed_async(id, reason, code).await
    }

    /// Cancel payment.
    pub async fn cancel(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().cancel_async(id).await
    }

    /// Create a refund.
    pub async fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        self.db.payments().create_refund_async(input).await
    }

    /// Get refund by ID.
    pub async fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        self.db.payments().get_refund_async(id).await
    }

    /// Get refunds for a payment.
    pub async fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
        self.db.payments().get_refunds_async(payment_id).await
    }

    /// Complete a refund.
    pub async fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        self.db.payments().complete_refund_async(id).await
    }

    /// Fail a refund.
    pub async fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        self.db.payments().fail_refund_async(id, reason).await
    }

    /// Create a payment method.
    pub async fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        self.db.payments().create_payment_method_async(input).await
    }

    /// Get payment methods for a customer.
    pub async fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        self.db.payments().get_payment_methods_async(customer_id).await
    }

    /// Delete a payment method.
    pub async fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        self.db.payments().delete_payment_method_async(id).await
    }

    /// Set default payment method.
    pub async fn set_default_payment_method(&self, customer_id: Uuid, method_id: Uuid) -> Result<()> {
        self.db.payments().set_default_payment_method_async(customer_id, method_id).await
    }

    /// Count payments.
    pub async fn count(&self, filter: PaymentFilter) -> Result<u64> {
        self.db.payments().count_async(filter).await
    }
}

// ============================================================================
// Async Warranties
// ============================================================================

/// Async warranty operations.
pub struct AsyncWarranties {
    db: Arc<PostgresDatabase>,
}

impl AsyncWarranties {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new warranty.
    pub async fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        self.db.warranties().create_async(input).await
    }

    /// Get warranty by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        self.db.warranties().get_async(id).await
    }

    /// Get warranty by warranty number.
    pub async fn get_by_number(&self, warranty_number: &str) -> Result<Option<Warranty>> {
        self.db.warranties().get_by_number_async(warranty_number).await
    }

    /// Get warranty by serial number.
    pub async fn get_by_serial(&self, serial_number: &str) -> Result<Option<Warranty>> {
        self.db.warranties().get_by_serial_async(serial_number).await
    }

    /// Update a warranty.
    pub async fn update(&self, id: Uuid, input: UpdateWarranty) -> Result<Warranty> {
        self.db.warranties().update_async(id, input).await
    }

    /// List warranties.
    pub async fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        self.db.warranties().list_async(filter).await
    }

    /// Get warranties for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_customer_async(customer_id).await
    }

    /// Get warranties for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_order_async(order_id).await
    }

    /// Void a warranty.
    pub async fn void(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().void_async(id).await
    }

    /// Expire a warranty.
    pub async fn expire(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().expire_async(id).await
    }

    /// Transfer warranty to new owner.
    pub async fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        self.db.warranties().transfer_async(id, new_customer_id).await
    }

    /// Create a warranty claim.
    pub async fn create_claim(&self, input: CreateWarrantyClaim) -> Result<WarrantyClaim> {
        self.db.warranties().create_claim_async(input).await
    }

    /// Get claim by ID.
    pub async fn get_claim(&self, id: Uuid) -> Result<Option<WarrantyClaim>> {
        self.db.warranties().get_claim_async(id).await
    }

    /// Get claim by claim number.
    pub async fn get_claim_by_number(&self, claim_number: &str) -> Result<Option<WarrantyClaim>> {
        self.db.warranties().get_claim_by_number_async(claim_number).await
    }

    /// Update a claim.
    pub async fn update_claim(&self, id: Uuid, input: UpdateWarrantyClaim) -> Result<WarrantyClaim> {
        self.db.warranties().update_claim_async(id, input).await
    }

    /// List claims.
    pub async fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().list_claims_async(filter).await
    }

    /// Get claims for a warranty.
    pub async fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().get_claims_async(warranty_id).await
    }

    /// Approve a claim.
    pub async fn approve_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.db.warranties().approve_claim_async(id).await
    }

    /// Deny a claim.
    pub async fn deny_claim(&self, id: Uuid, reason: &str) -> Result<WarrantyClaim> {
        self.db.warranties().deny_claim_async(id, reason).await
    }

    /// Complete a claim.
    pub async fn complete_claim(&self, id: Uuid, resolution: ClaimResolution) -> Result<WarrantyClaim> {
        self.db.warranties().complete_claim_async(id, resolution).await
    }

    /// Cancel a claim.
    pub async fn cancel_claim(&self, id: Uuid) -> Result<WarrantyClaim> {
        self.db.warranties().cancel_claim_async(id).await
    }

    /// Count warranties.
    pub async fn count(&self, filter: WarrantyFilter) -> Result<u64> {
        self.db.warranties().count_async(filter).await
    }

    /// Count claims.
    pub async fn count_claims(&self, filter: WarrantyClaimFilter) -> Result<u64> {
        self.db.warranties().count_claims_async(filter).await
    }
}

// ============================================================================
// Async BOM
// ============================================================================

/// Async Bill of Materials operations.
pub struct AsyncBom {
    db: Arc<PostgresDatabase>,
}

impl AsyncBom {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new BOM.
    pub async fn create(&self, input: CreateBom) -> Result<BillOfMaterials> {
        self.db.bom().create_async(input).await
    }

    /// Get BOM by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<BillOfMaterials>> {
        self.db.bom().get_async(id).await
    }

    /// Get BOM by BOM number.
    pub async fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        self.db.bom().get_by_number_async(bom_number).await
    }

    /// Update a BOM.
    pub async fn update(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials> {
        self.db.bom().update_async(id, input).await
    }

    /// List BOMs.
    pub async fn list(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>> {
        self.db.bom().list_async(filter).await
    }

    /// Delete a BOM.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.bom().delete_async(id).await
    }

    /// Add component to BOM.
    pub async fn add_component(&self, bom_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        self.db.bom().add_component_async(bom_id, component).await
    }

    /// Update a component.
    pub async fn update_component(&self, component_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        self.db.bom().update_component_async(component_id, component).await
    }

    /// Remove component from BOM.
    pub async fn remove_component(&self, component_id: Uuid) -> Result<()> {
        self.db.bom().remove_component_async(component_id).await
    }

    /// Activate a BOM.
    pub async fn activate(&self, id: Uuid) -> Result<BillOfMaterials> {
        self.db.bom().activate_async(id).await
    }

    /// Count BOMs.
    pub async fn count(&self, filter: BomFilter) -> Result<u64> {
        self.db.bom().count_async(filter).await
    }
}

// ============================================================================
// Async Work Orders
// ============================================================================

/// Async work order operations.
pub struct AsyncWorkOrders {
    db: Arc<PostgresDatabase>,
}

impl AsyncWorkOrders {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new work order.
    pub async fn create(&self, input: CreateWorkOrder) -> Result<WorkOrder> {
        self.db.work_orders().create_async(input).await
    }

    /// Get work order by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<WorkOrder>> {
        self.db.work_orders().get_async(id).await
    }

    /// Get work order by work order number.
    pub async fn get_by_number(&self, work_order_number: &str) -> Result<Option<WorkOrder>> {
        self.db.work_orders().get_by_number_async(work_order_number).await
    }

    /// Update a work order.
    pub async fn update(&self, id: Uuid, input: UpdateWorkOrder) -> Result<WorkOrder> {
        self.db.work_orders().update_async(id, input).await
    }

    /// List work orders.
    pub async fn list(&self, filter: WorkOrderFilter) -> Result<Vec<WorkOrder>> {
        self.db.work_orders().list_async(filter).await
    }

    /// Delete a work order.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.work_orders().delete_async(id).await
    }

    /// Start a work order.
    pub async fn start(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().start_async(id).await
    }

    /// Complete a work order.
    pub async fn complete(&self, id: Uuid, quantity_completed: Decimal) -> Result<WorkOrder> {
        self.db.work_orders().complete_async(id, quantity_completed).await
    }

    /// Put work order on hold.
    pub async fn hold(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().hold_async(id).await
    }

    /// Resume a held work order.
    pub async fn resume(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().resume_async(id).await
    }

    /// Cancel a work order.
    pub async fn cancel(&self, id: Uuid) -> Result<WorkOrder> {
        self.db.work_orders().cancel_async(id).await
    }

    /// Add task to work order.
    pub async fn add_task(&self, work_order_id: Uuid, task: CreateWorkOrderTask) -> Result<WorkOrderTask> {
        self.db.work_orders().add_task_async(work_order_id, task).await
    }

    /// Update a task.
    pub async fn update_task(&self, task_id: Uuid, task: UpdateWorkOrderTask) -> Result<WorkOrderTask> {
        self.db.work_orders().update_task_async(task_id, task).await
    }

    /// Remove task from work order.
    pub async fn remove_task(&self, task_id: Uuid) -> Result<()> {
        self.db.work_orders().remove_task_async(task_id).await
    }

    /// Start a task.
    pub async fn start_task(&self, task_id: Uuid) -> Result<WorkOrderTask> {
        self.db.work_orders().start_task_async(task_id).await
    }

    /// Complete a task.
    pub async fn complete_task(&self, task_id: Uuid, actual_hours: Option<Decimal>) -> Result<WorkOrderTask> {
        self.db.work_orders().complete_task_async(task_id, actual_hours).await
    }

    /// Add material to work order.
    pub async fn add_material(&self, work_order_id: Uuid, material: AddWorkOrderMaterial) -> Result<WorkOrderMaterial> {
        self.db.work_orders().add_material_async(work_order_id, material).await
    }

    /// Consume material.
    pub async fn consume_material(&self, material_id: Uuid, quantity: Decimal) -> Result<WorkOrderMaterial> {
        self.db.work_orders().consume_material_async(material_id, quantity).await
    }

    /// Count work orders.
    pub async fn count(&self, filter: WorkOrderFilter) -> Result<u64> {
        self.db.work_orders().count_async(filter).await
    }
}

// ============================================================================
// Async Purchase Orders
// ============================================================================

/// Async purchase order operations.
pub struct AsyncPurchaseOrders {
    db: Arc<PostgresDatabase>,
}

impl AsyncPurchaseOrders {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    // Supplier operations

    /// Create a new supplier.
    pub async fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier> {
        self.db.purchase_orders().create_supplier_async(input).await
    }

    /// Get supplier by ID.
    pub async fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>> {
        self.db.purchase_orders().get_supplier_async(id).await
    }

    /// Get supplier by code.
    pub async fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>> {
        self.db.purchase_orders().get_supplier_by_code_async(code).await
    }

    /// Update a supplier.
    pub async fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        self.db.purchase_orders().update_supplier_async(id, input).await
    }

    /// List suppliers.
    pub async fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        self.db.purchase_orders().list_suppliers_async(filter).await
    }

    /// Delete a supplier.
    pub async fn delete_supplier(&self, id: Uuid) -> Result<()> {
        self.db.purchase_orders().delete_supplier_async(id).await
    }

    // Purchase Order operations

    /// Create a new purchase order.
    pub async fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        self.db.purchase_orders().create_async(input).await
    }

    /// Get purchase order by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
        self.db.purchase_orders().get_async(id).await
    }

    /// Get purchase order by PO number.
    pub async fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        self.db.purchase_orders().get_by_number_async(po_number).await
    }

    /// Update a purchase order.
    pub async fn update(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        self.db.purchase_orders().update_async(id, input).await
    }

    /// List purchase orders.
    pub async fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        self.db.purchase_orders().list_async(filter).await
    }

    /// Get purchase orders for a supplier.
    pub async fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.db.purchase_orders().for_supplier_async(supplier_id).await
    }

    /// Delete a purchase order.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.purchase_orders().delete_async(id).await
    }

    /// Submit for approval.
    pub async fn submit(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().submit_for_approval_async(id).await
    }

    /// Approve purchase order.
    pub async fn approve(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder> {
        self.db.purchase_orders().approve_async(id, approved_by).await
    }

    /// Send to supplier.
    pub async fn send(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().send_async(id).await
    }

    /// Mark as acknowledged by supplier.
    pub async fn acknowledge(&self, id: Uuid, supplier_reference: Option<&str>) -> Result<PurchaseOrder> {
        self.db.purchase_orders().acknowledge_async(id, supplier_reference).await
    }

    /// Put on hold.
    pub async fn hold(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().hold_async(id).await
    }

    /// Cancel purchase order.
    pub async fn cancel(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().cancel_async(id).await
    }

    /// Receive items.
    pub async fn receive(&self, id: Uuid, items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder> {
        self.db.purchase_orders().receive_async(id, items).await
    }

    /// Complete purchase order.
    pub async fn complete(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().complete_async(id).await
    }

    /// Add item to purchase order.
    pub async fn add_item(&self, po_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().add_item_async(po_id, item).await
    }

    /// Update a PO item.
    pub async fn update_item(&self, item_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().update_item_async(item_id, item).await
    }

    /// Remove item from purchase order.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.purchase_orders().remove_item_async(item_id).await
    }

    /// Get items for purchase order.
    pub async fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        self.db.purchase_orders().get_items_async(po_id).await
    }

    /// Count purchase orders.
    pub async fn count(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        self.db.purchase_orders().count_async(filter).await
    }

    /// Count suppliers.
    pub async fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64> {
        self.db.purchase_orders().count_suppliers_async(filter).await
    }
}

// ============================================================================
// Async Invoices
// ============================================================================

/// Async invoice operations.
pub struct AsyncInvoices {
    db: Arc<PostgresDatabase>,
}

impl AsyncInvoices {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new invoice.
    pub async fn create(&self, input: CreateInvoice) -> Result<Invoice> {
        self.db.invoices().create_async(input).await
    }

    /// Get invoice by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Invoice>> {
        self.db.invoices().get_async(id).await
    }

    /// Get invoice by invoice number.
    pub async fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>> {
        self.db.invoices().get_by_number_async(invoice_number).await
    }

    /// Update an invoice.
    pub async fn update(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice> {
        self.db.invoices().update_async(id, input).await
    }

    /// List invoices.
    pub async fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        self.db.invoices().list_async(filter).await
    }

    /// Get invoices for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Invoice>> {
        self.db.invoices().for_customer_async(customer_id).await
    }

    /// Get invoices for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Invoice>> {
        self.db.invoices().for_order_async(order_id).await
    }

    /// Delete an invoice.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.invoices().delete_async(id).await
    }

    /// Send invoice to customer.
    pub async fn send(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().send_async(id).await
    }

    /// Mark invoice as viewed.
    pub async fn mark_viewed(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().mark_viewed_async(id).await
    }

    /// Record a payment on the invoice.
    pub async fn record_payment(&self, id: Uuid, payment: RecordInvoicePayment) -> Result<Invoice> {
        self.db.invoices().record_payment_async(id, payment).await
    }

    /// Void an invoice.
    pub async fn void(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().void_async(id).await
    }

    /// Write off an invoice.
    pub async fn write_off(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().write_off_async(id).await
    }

    /// Mark invoice as disputed.
    pub async fn dispute(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().dispute_async(id).await
    }

    /// Add item to invoice.
    pub async fn add_item(&self, invoice_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.db.invoices().add_item_async(invoice_id, item).await
    }

    /// Update an invoice item.
    pub async fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.db.invoices().update_item_async(item_id, item).await
    }

    /// Remove item from invoice.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.invoices().remove_item_async(item_id).await
    }

    /// Get items for invoice.
    pub async fn get_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        self.db.invoices().get_items_async(invoice_id).await
    }

    /// Recalculate invoice totals.
    pub async fn recalculate(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().recalculate_invoice_async(id).await
    }

    /// Get overdue invoices.
    pub async fn get_overdue(&self) -> Result<Vec<Invoice>> {
        self.db.invoices().get_overdue_async().await
    }

    /// Count invoices.
    pub async fn count(&self, filter: InvoiceFilter) -> Result<u64> {
        self.db.invoices().count_async(filter).await
    }
}

// ============================================================================
// Async Carts
// ============================================================================

/// Async cart operations.
pub struct AsyncCarts {
    db: Arc<PostgresDatabase>,
}

impl AsyncCarts {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new cart.
    pub async fn create(&self, input: CreateCart) -> Result<Cart> {
        self.db.carts().create_async(input).await
    }

    /// Get cart by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Cart>> {
        self.db.carts().get_async(id).await
    }

    /// Get cart by cart number.
    pub async fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        self.db.carts().get_by_number_async(cart_number).await
    }

    /// Update a cart.
    pub async fn update(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
        self.db.carts().update_async(id, input).await
    }

    /// List carts.
    pub async fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        self.db.carts().list_async(filter).await
    }

    /// Get carts for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        self.db.carts().for_customer_async(customer_id).await
    }

    /// Delete a cart.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.carts().delete_async(id).await
    }

    /// Add item to cart.
    pub async fn add_item(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        self.db.carts().add_item_async(cart_id, item).await
    }

    /// Update a cart item.
    pub async fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        self.db.carts().update_item_async(item_id, input).await
    }

    /// Remove item from cart.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.carts().remove_item_async(item_id).await
    }

    /// Get items for a cart.
    pub async fn get_items(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        self.db.carts().get_items_async(cart_id).await
    }

    /// Clear all items from cart.
    pub async fn clear_items(&self, cart_id: Uuid) -> Result<()> {
        self.db.carts().clear_items_async(cart_id).await
    }

    /// Set shipping address.
    pub async fn set_shipping_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_shipping_address_async(id, address).await
    }

    /// Set billing address.
    pub async fn set_billing_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_billing_address_async(id, address).await
    }

    /// Set shipping method.
    pub async fn set_shipping(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
        self.db.carts().set_shipping_async(id, shipping).await
    }

    /// Get available shipping rates for cart.
    pub async fn get_shipping_rates(&self, id: Uuid) -> Result<Vec<ShippingRate>> {
        self.db.carts().get_shipping_rates_async(id).await
    }

    /// Set payment method/token.
    pub async fn set_payment(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        self.db.carts().set_payment_async(id, payment).await
    }

    /// Apply coupon/discount code.
    pub async fn apply_discount(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        self.db.carts().apply_discount_async(id, coupon_code).await
    }

    /// Remove discount.
    pub async fn remove_discount(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().remove_discount_async(id).await
    }

    /// Mark cart as ready for payment.
    pub async fn mark_ready_for_payment(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().mark_ready_for_payment_async(id).await
    }

    /// Begin checkout process.
    pub async fn begin_checkout(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().begin_checkout_async(id).await
    }

    /// Complete checkout (creates order).
    pub async fn complete(&self, id: Uuid) -> Result<CheckoutResult> {
        self.db.carts().complete_async(id).await
    }

    /// Cancel a cart.
    pub async fn cancel(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().cancel_async(id).await
    }

    /// Mark cart as abandoned.
    pub async fn abandon(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().abandon_async(id).await
    }

    /// Expire a cart.
    pub async fn expire(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().expire_async(id).await
    }

    /// Reserve inventory for cart items.
    pub async fn reserve_inventory(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().reserve_inventory_async(id).await
    }

    /// Release inventory reservations.
    pub async fn release_inventory(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().release_inventory_async(id).await
    }

    /// Recalculate cart totals.
    pub async fn recalculate(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().recalculate_async(id).await
    }

    /// Set tax amount.
    pub async fn set_tax(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
        self.db.carts().set_tax_async(id, tax_amount).await
    }

    /// Get abandoned carts.
    pub async fn get_abandoned(&self) -> Result<Vec<Cart>> {
        self.db.carts().get_abandoned_async().await
    }

    /// Get expired carts.
    pub async fn get_expired(&self) -> Result<Vec<Cart>> {
        self.db.carts().get_expired_async().await
    }

    /// Count carts.
    pub async fn count(&self, filter: CartFilter) -> Result<u64> {
        self.db.carts().count_async(filter).await
    }
}

// ============================================================================
// Async Analytics
// ============================================================================

/// Async analytics operations.
pub struct AsyncAnalytics {
    db: Arc<PostgresDatabase>,
}

impl AsyncAnalytics {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Get sales summary for a time period.
    pub async fn sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary> {
        self.db.analytics().get_sales_summary_async(query).await
    }

    /// Get revenue broken down by time periods.
    pub async fn revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>> {
        self.db.analytics().get_revenue_by_period_async(query).await
    }

    /// Get top selling products.
    pub async fn top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        self.db.analytics().get_top_products_async(query).await
    }

    /// Get product performance.
    pub async fn product_performance(&self, query: AnalyticsQuery) -> Result<Vec<ProductPerformance>> {
        self.db.analytics().get_product_performance_async(query).await
    }

    /// Get customer metrics.
    pub async fn customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics> {
        self.db.analytics().get_customer_metrics_async(query).await
    }

    /// Get top customers by spend.
    pub async fn top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>> {
        self.db.analytics().get_top_customers_async(query).await
    }

    /// Get inventory health summary.
    pub async fn inventory_health(&self) -> Result<InventoryHealth> {
        self.db.analytics().get_inventory_health_async().await
    }

    /// Get low stock items.
    pub async fn low_stock_items(&self, threshold: Option<Decimal>) -> Result<Vec<LowStockItem>> {
        self.db.analytics().get_low_stock_items_async(threshold).await
    }

    /// Get inventory movement summary.
    pub async fn inventory_movement(&self, query: AnalyticsQuery) -> Result<Vec<InventoryMovement>> {
        self.db.analytics().get_inventory_movement_async(query).await
    }

    /// Get order status breakdown.
    pub async fn order_status_breakdown(&self, query: AnalyticsQuery) -> Result<OrderStatusBreakdown> {
        self.db.analytics().get_order_status_breakdown_async(query).await
    }

    /// Get fulfillment metrics.
    pub async fn fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics> {
        self.db.analytics().get_fulfillment_metrics_async(query).await
    }

    /// Get return metrics.
    pub async fn return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics> {
        self.db.analytics().get_return_metrics_async(query).await
    }

    /// Get demand forecast for SKUs.
    pub async fn demand_forecast(&self, skus: Option<Vec<String>>, days_ahead: u32) -> Result<Vec<DemandForecast>> {
        self.db.analytics().get_demand_forecast_async(skus, days_ahead).await
    }

    /// Get revenue forecast.
    pub async fn revenue_forecast(&self, periods_ahead: u32, granularity: TimeGranularity) -> Result<Vec<RevenueForecast>> {
        self.db.analytics().get_revenue_forecast_async(periods_ahead, granularity).await
    }
}

// ============================================================================
// Async Currency
// ============================================================================

/// Async currency operations.
pub struct AsyncCurrency {
    db: Arc<PostgresDatabase>,
}

impl AsyncCurrency {
    pub(crate) fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Get exchange rate between two currencies.
    pub async fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>> {
        self.db.currency().get_rate_async(from, to).await
    }

    /// Get all exchange rates for a base currency.
    pub async fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>> {
        self.db.currency().get_rates_for_async(base).await
    }

    /// List all exchange rates.
    pub async fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        self.db.currency().list_rates_async(filter).await
    }

    /// Set an exchange rate.
    pub async fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate> {
        self.db.currency().set_rate_async(input).await
    }

    /// Delete an exchange rate.
    pub async fn delete_rate(&self, id: Uuid) -> Result<()> {
        self.db.currency().delete_rate_async(id).await
    }

    /// Convert money between currencies.
    pub async fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult> {
        self.db.currency().convert_async(input).await
    }

    /// Get store currency settings.
    pub async fn get_settings(&self) -> Result<StoreCurrencySettings> {
        self.db.currency().get_settings_async().await
    }

    /// Update store currency settings.
    pub async fn update_settings(&self, settings: StoreCurrencySettings) -> Result<StoreCurrencySettings> {
        self.db.currency().update_settings_async(settings).await
    }
}
