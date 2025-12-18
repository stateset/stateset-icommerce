//! Main Commerce struct - the entry point to the library

use crate::{Analytics, Bom, Carts, CurrencyOps, Customers, Inventory, Invoices, Orders, Payments, Products, PurchaseOrders, Returns, Shipments, Warranties, WorkOrders};
use stateset_core::CommerceError;
use stateset_db::{Database, DatabaseConfig};
use std::sync::Arc;

#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;

/// The main commerce interface.
///
/// This is the entry point to all commerce operations. Initialize it once
/// and use the accessor methods to perform operations.
///
/// # Example
///
/// ```rust,no_run
/// use stateset_embedded::Commerce;
///
/// // SQLite (default)
/// let commerce = Commerce::new("./store.db")?;
///
/// // Access different domains
/// let orders = commerce.orders();
/// let inventory = commerce.inventory();
/// let customers = commerce.customers();
/// let products = commerce.products();
/// let returns = commerce.returns();
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct Commerce {
    db: Arc<dyn Database>,
}

impl Commerce {
    /// Create a new Commerce instance with a SQLite database.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file. Creates if not exists.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    ///
    /// // File-based database
    /// let commerce = Commerce::new("./my-store.db")?;
    ///
    /// // In-memory database (useful for testing)
    /// let commerce = Commerce::new(":memory:")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn new(path: &str) -> Result<Self, CommerceError> {
        let config = if path == ":memory:" {
            DatabaseConfig::in_memory()
        } else {
            DatabaseConfig::sqlite(path)
        };

        let db = SqliteDatabase::new(&config)?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Create a Commerce instance connected to PostgreSQL.
    ///
    /// This requires the `postgres` feature to be enabled and creates
    /// a new Tokio runtime for async operations.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string (e.g., "postgres://user:pass@localhost/db")
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::with_postgres("postgres://localhost/stateset")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "postgres")]
    pub fn with_postgres(url: &str) -> Result<Self, CommerceError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {}", e)))?;

        let db = rt.block_on(PostgresDatabase::connect(url))?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Create a Commerce instance connected to PostgreSQL with custom options.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string
    /// * `max_connections` - Maximum number of connections in the pool
    /// * `acquire_timeout_secs` - Timeout in seconds for acquiring a connection
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::with_postgres_options(
    ///     "postgres://localhost/stateset",
    ///     20,  // max connections
    ///     60,  // timeout in seconds
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "postgres")]
    pub fn with_postgres_options(
        url: &str,
        max_connections: u32,
        acquire_timeout_secs: u64,
    ) -> Result<Self, CommerceError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {}", e)))?;

        let db = rt.block_on(PostgresDatabase::connect_with_options(
            url,
            max_connections,
            acquire_timeout_secs,
        ))?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Create a Commerce instance with a pre-connected database.
    ///
    /// This is useful when you want to manage the database connection yourself.
    pub fn with_database(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a Commerce instance with custom configuration.
    ///
    /// Use `CommerceBuilder` for more control over initialization.
    pub fn builder() -> CommerceBuilder {
        CommerceBuilder::default()
    }

    /// Access order operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateOrder, CreateOrderItem};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let order = commerce.orders().create(CreateOrder {
    ///     customer_id: Uuid::new_v4(),
    ///     items: vec![CreateOrderItem {
    ///         product_id: Uuid::new_v4(),
    ///         sku: "SKU-001".into(),
    ///         name: "Widget".into(),
    ///         quantity: 2,
    ///         unit_price: dec!(29.99),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn orders(&self) -> Orders {
        Orders::new(self.db.clone())
    }

    /// Access inventory operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateInventoryItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create inventory item
    /// commerce.inventory().create_item(CreateInventoryItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     initial_quantity: Some(dec!(100)),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Check stock
    /// let stock = commerce.inventory().get_stock("SKU-001")?;
    ///
    /// // Adjust stock
    /// commerce.inventory().adjust("SKU-001", dec!(-5), "Sold")?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn inventory(&self) -> Inventory {
        Inventory::new(self.db.clone())
    }

    /// Access customer operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCustomer};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let customer = commerce.customers().create(CreateCustomer {
    ///     email: "alice@example.com".into(),
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn customers(&self) -> Customers {
        Customers::new(self.db.clone())
    }

    /// Access product operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateProduct, CreateProductVariant};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let product = commerce.products().create(CreateProduct {
    ///     name: "Premium Widget".into(),
    ///     description: Some("A high-quality widget".into()),
    ///     variants: Some(vec![CreateProductVariant {
    ///         sku: "WIDGET-001".into(),
    ///         price: dec!(49.99),
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn products(&self) -> Products {
        Products::new(self.db.clone())
    }

    /// Access return operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateReturn, CreateReturnItem, ReturnReason};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let ret = commerce.returns().create(CreateReturn {
    ///     order_id: Uuid::new_v4(),
    ///     reason: ReturnReason::Defective,
    ///     items: vec![CreateReturnItem {
    ///         order_item_id: Uuid::new_v4(),
    ///         quantity: 1,
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn returns(&self) -> Returns {
        Returns::new(self.db.clone())
    }

    /// Access Bill of Materials (BOM) operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateBom, CreateBomComponent};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let bom = commerce.bom().create(CreateBom {
    ///     product_id: Uuid::new_v4(),
    ///     name: "Widget Assembly".into(),
    ///     components: Some(vec![
    ///         CreateBomComponent {
    ///             name: "Part A".into(),
    ///             quantity: dec!(2),
    ///             ..Default::default()
    ///         },
    ///     ]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn bom(&self) -> Bom {
        Bom::new(self.db.clone())
    }

    /// Access work order operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWorkOrder};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let wo = commerce.work_orders().create(CreateWorkOrder {
    ///     product_id: Uuid::new_v4(),
    ///     quantity_to_build: dec!(100),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Start production
    /// let wo = commerce.work_orders().start(wo.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders::new(self.db.clone())
    }

    /// Access shipment operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateShipment, CreateShipmentItem, ShippingCarrier};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let shipment = commerce.shipments().create(CreateShipment {
    ///     order_id: Uuid::new_v4(),
    ///     carrier: Some(ShippingCarrier::Ups),
    ///     recipient_name: "Alice Smith".into(),
    ///     shipping_address: "123 Main St, City, ST 12345".into(),
    ///     items: Some(vec![CreateShipmentItem {
    ///         sku: "SKU-001".into(),
    ///         name: "Widget".into(),
    ///         quantity: 2,
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Ship with tracking number
    /// let shipment = commerce.shipments().ship(shipment.id, Some("1Z999AA10123456784".into()))?;
    ///
    /// // Mark as delivered
    /// let shipment = commerce.shipments().mark_delivered(shipment.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn shipments(&self) -> Shipments {
        Shipments::new(self.db.clone())
    }

    /// Access payment operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePayment, PaymentMethodType, CardBrand};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let payment = commerce.payments().create(CreatePayment {
    ///     order_id: Some(Uuid::new_v4()),
    ///     payment_method: PaymentMethodType::CreditCard,
    ///     amount: dec!(99.99),
    ///     card_brand: Some(CardBrand::Visa),
    ///     card_last4: Some("4242".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Mark payment as completed
    /// let payment = commerce.payments().mark_completed(payment.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn payments(&self) -> Payments {
        Payments::new(self.db.clone())
    }

    /// Access warranty operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateWarranty, WarrantyType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let warranty = commerce.warranties().create(CreateWarranty {
    ///     customer_id: Uuid::new_v4(),
    ///     product_id: Some(Uuid::new_v4()),
    ///     warranty_type: Some(WarrantyType::Extended),
    ///     duration_months: Some(24),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Check if warranty is valid
    /// assert!(commerce.warranties().is_valid(warranty.id)?);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn warranties(&self) -> Warranties {
        Warranties::new(self.db.clone())
    }

    /// Access purchase order operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a supplier
    /// let supplier = commerce.purchase_orders().create_supplier(CreateSupplier {
    ///     name: "Acme Supplies".into(),
    ///     email: Some("orders@acme.com".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Create a purchase order
    /// let po = commerce.purchase_orders().create(CreatePurchaseOrder {
    ///     supplier_id: supplier.id,
    ///     items: vec![CreatePurchaseOrderItem {
    ///         sku: "PART-001".into(),
    ///         name: "Widget Part".into(),
    ///         quantity: dec!(100),
    ///         unit_cost: dec!(5.99),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// // Approve and send
    /// let po = commerce.purchase_orders().submit(po.id)?;
    /// let po = commerce.purchase_orders().approve(po.id, "admin")?;
    /// let po = commerce.purchase_orders().send(po.id)?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders::new(self.db.clone())
    }

    /// Access invoice operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateInvoice, CreateInvoiceItem, RecordInvoicePayment};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let invoice = commerce.invoices().create(CreateInvoice {
    ///     customer_id: Uuid::new_v4(),
    ///     billing_email: Some("customer@example.com".into()),
    ///     items: vec![CreateInvoiceItem {
    ///         description: "Professional Services".into(),
    ///         quantity: dec!(10),
    ///         unit_price: dec!(150.00),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// // Send and record payment
    /// let invoice = commerce.invoices().send(invoice.id)?;
    /// let invoice = commerce.invoices().record_payment(invoice.id, RecordInvoicePayment {
    ///     amount: dec!(1500.00),
    ///     payment_method: Some("credit_card".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn invoices(&self) -> Invoices {
        Invoices::new(self.db.clone())
    }

    /// Access cart and checkout operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem, CartAddress};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a cart
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("alice@example.com".into()),
    ///     customer_name: Some("Alice Smith".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Add items
    /// commerce.carts().add_item(cart.id, AddCartItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     quantity: 2,
    ///     unit_price: dec!(29.99),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Set shipping address
    /// commerce.carts().set_shipping_address(cart.id, CartAddress {
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     line1: "123 Main St".into(),
    ///     city: "Anytown".into(),
    ///     postal_code: "12345".into(),
    ///     country: "US".into(),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Complete checkout
    /// let result = commerce.carts().complete(cart.id)?;
    /// println!("Order created: {}", result.order_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn carts(&self) -> Carts {
        Carts::new(self.db.clone())
    }

    /// Access analytics and forecasting operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AnalyticsQuery, TimePeriod};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Get sales summary
    /// let summary = commerce.analytics().sales_summary(
    ///     AnalyticsQuery::new().period(TimePeriod::Last30Days)
    /// )?;
    /// println!("Revenue: ${}", summary.total_revenue);
    /// println!("Orders: {}", summary.order_count);
    ///
    /// // Get top products
    /// let top = commerce.analytics().top_products(
    ///     AnalyticsQuery::new().period(TimePeriod::ThisMonth).limit(10)
    /// )?;
    ///
    /// // Get inventory forecast
    /// let forecasts = commerce.analytics().demand_forecast(None, 30)?;
    /// for f in forecasts {
    ///     if let Some(days) = f.days_until_stockout {
    ///         if days < 14 {
    ///             println!("WARNING: {} will stock out in {} days", f.sku, days);
    ///         }
    ///     }
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn analytics(&self) -> Analytics {
        Analytics::new(self.db.clone())
    }

    /// Access currency and exchange rate operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, Currency, ConvertCurrency};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Get exchange rate
    /// if let Some(rate) = commerce.currency().get_rate(Currency::USD, Currency::EUR)? {
    ///     println!("1 USD = {} EUR", rate.rate);
    /// }
    ///
    /// // Convert currency
    /// let result = commerce.currency().convert(ConvertCurrency {
    ///     from: Currency::USD,
    ///     to: Currency::EUR,
    ///     amount: dec!(100.00),
    /// })?;
    /// println!("$100 USD = €{} EUR", result.converted_amount);
    ///
    /// // Set exchange rates
    /// commerce.currency().set_rate(stateset_embedded::SetExchangeRate {
    ///     base_currency: Currency::USD,
    ///     quote_currency: Currency::EUR,
    ///     rate: dec!(0.92),
    ///     source: Some("manual".into()),
    /// })?;
    ///
    /// // Update store settings
    /// let settings = commerce.currency().get_settings()?;
    /// println!("Base currency: {}", settings.base_currency);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn currency(&self) -> CurrencyOps {
        CurrencyOps::new(self.db.clone())
    }

    /// Get the underlying database (for advanced use cases).
    pub fn database(&self) -> &dyn Database {
        &*self.db
    }
}

/// Builder for creating a Commerce instance with custom configuration.
#[derive(Default)]
pub struct CommerceBuilder {
    sqlite_path: Option<String>,
    #[cfg(feature = "postgres")]
    postgres_url: Option<String>,
    max_connections: Option<u32>,
    #[cfg(feature = "postgres")]
    acquire_timeout_secs: Option<u64>,
}

impl CommerceBuilder {
    /// Set the SQLite database path.
    #[cfg(feature = "sqlite")]
    pub fn sqlite(mut self, path: &str) -> Self {
        self.sqlite_path = Some(path.to_string());
        self
    }

    /// Set the database path (alias for sqlite).
    #[cfg(feature = "sqlite")]
    pub fn database(self, path: &str) -> Self {
        self.sqlite(path)
    }

    /// Set the PostgreSQL connection URL.
    ///
    /// When this is set, the builder will create a PostgreSQL connection
    /// instead of SQLite.
    #[cfg(feature = "postgres")]
    pub fn postgres(mut self, url: &str) -> Self {
        self.postgres_url = Some(url.to_string());
        self
    }

    /// Set the maximum number of database connections.
    pub fn max_connections(mut self, count: u32) -> Self {
        self.max_connections = Some(count);
        self
    }

    /// Set the acquire timeout for PostgreSQL connections.
    #[cfg(feature = "postgres")]
    pub fn acquire_timeout_secs(mut self, secs: u64) -> Self {
        self.acquire_timeout_secs = Some(secs);
        self
    }

    /// Build the Commerce instance.
    pub fn build(self) -> Result<Commerce, CommerceError> {
        // Check if PostgreSQL URL is set
        #[cfg(feature = "postgres")]
        if let Some(url) = self.postgres_url {
            let max_conn = self.max_connections.unwrap_or(10);
            let timeout = self.acquire_timeout_secs.unwrap_or(30);
            return Commerce::with_postgres_options(&url, max_conn, timeout);
        }

        // Default to SQLite
        #[cfg(feature = "sqlite")]
        {
            let path = self.sqlite_path.unwrap_or_else(|| "stateset.db".to_string());

            let config = if path == ":memory:" {
                DatabaseConfig::in_memory()
            } else {
                let mut config = DatabaseConfig::sqlite(&path);
                if let Some(max) = self.max_connections {
                    config.max_connections = max;
                }
                config
            };

            let db = SqliteDatabase::new(&config)?;
            return Ok(Commerce { db: Arc::new(db) });
        }

        #[cfg(not(any(feature = "sqlite", feature = "postgres")))]
        Err(CommerceError::Internal(
            "No database backend enabled. Enable 'sqlite' or 'postgres' feature.".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_create_commerce() {
        let commerce = Commerce::new(":memory:").unwrap();
        assert!(commerce.orders().list(Default::default()).unwrap().is_empty());
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_builder() {
        let commerce = Commerce::builder()
            .database(":memory:")
            .max_connections(1)
            .build()
            .unwrap();

        assert!(commerce.customers().list(Default::default()).unwrap().is_empty());
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_bom_operations() {
        use rust_decimal_macros::dec;
        use stateset_core::{CreateBom, CreateBomComponent, BomStatus};

        let commerce = Commerce::new(":memory:").unwrap();
        let product_id = uuid::Uuid::new_v4();

        // Create a BOM
        let bom = commerce.bom().create(CreateBom {
            product_id,
            name: "Test BOM".into(),
            description: Some("Test description".into()),
            components: Some(vec![CreateBomComponent {
                name: "Component A".into(),
                component_sku: Some("COMP-A".into()),
                quantity: dec!(2),
                ..Default::default()
            }]),
            ..Default::default()
        }).unwrap();

        assert_eq!(bom.name, "Test BOM");
        assert_eq!(bom.status, BomStatus::Draft);
        assert!(bom.bom_number.starts_with("BOM-"));

        // Get components
        let components = commerce.bom().get_components(bom.id).unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].name, "Component A");

        // Activate
        let bom = commerce.bom().activate(bom.id).unwrap();
        assert_eq!(bom.status, BomStatus::Active);
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_work_order_operations() {
        use rust_decimal_macros::dec;
        use stateset_core::{CreateWorkOrder, WorkOrderStatus};

        let commerce = Commerce::new(":memory:").unwrap();
        let product_id = uuid::Uuid::new_v4();

        // Create work order
        let wo = commerce.work_orders().create(CreateWorkOrder {
            product_id,
            quantity_to_build: dec!(100),
            notes: Some("Test work order".into()),
            ..Default::default()
        }).unwrap();

        assert!(wo.work_order_number.starts_with("WO-"));
        assert_eq!(wo.status, WorkOrderStatus::Planned);
        assert_eq!(wo.quantity_to_build, dec!(100));

        // Start work order
        let wo = commerce.work_orders().start(wo.id).unwrap();
        assert_eq!(wo.status, WorkOrderStatus::InProgress);

        // Complete work order
        let wo = commerce.work_orders().complete(wo.id, dec!(100)).unwrap();
        assert_eq!(wo.status, WorkOrderStatus::Completed);
        assert_eq!(wo.quantity_completed, dec!(100));
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_shipment_operations() {
        use stateset_core::{CreateShipment, CreateShipmentItem, ShipmentStatus, ShippingCarrier};

        let commerce = Commerce::new(":memory:").unwrap();
        let order_id = uuid::Uuid::new_v4();

        // Create shipment
        let shipment = commerce.shipments().create(CreateShipment {
            order_id,
            carrier: Some(ShippingCarrier::Ups),
            recipient_name: "Alice Smith".into(),
            shipping_address: "123 Main St, City, ST 12345".into(),
            items: Some(vec![CreateShipmentItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                ..Default::default()
            }]),
            ..Default::default()
        }).unwrap();

        assert!(shipment.shipment_number.starts_with("SHP-"));
        assert_eq!(shipment.status, ShipmentStatus::Pending);
        assert_eq!(shipment.carrier, ShippingCarrier::Ups);

        // Get items
        let items = commerce.shipments().get_items(shipment.id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sku, "SKU-001");

        // Mark as processing
        let shipment = commerce.shipments().mark_processing(shipment.id).unwrap();
        assert_eq!(shipment.status, ShipmentStatus::Processing);

        // Ship with tracking number
        let shipment = commerce.shipments().ship(shipment.id, Some("1Z999AA10123456784".into())).unwrap();
        assert_eq!(shipment.status, ShipmentStatus::Shipped);
        assert_eq!(shipment.tracking_number, Some("1Z999AA10123456784".to_string()));
        assert!(shipment.tracking_url.is_some());

        // Mark in transit
        let shipment = commerce.shipments().mark_in_transit(shipment.id).unwrap();
        assert_eq!(shipment.status, ShipmentStatus::InTransit);

        // Mark delivered
        let shipment = commerce.shipments().mark_delivered(shipment.id).unwrap();
        assert_eq!(shipment.status, ShipmentStatus::Delivered);
        assert!(shipment.delivered_at.is_some());
    }
}
