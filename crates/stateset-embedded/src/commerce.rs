//! Main Commerce struct - the entry point to the library

use crate::{Analytics, Bom, Carts, CurrencyOps, Customers, Inventory, Invoices, Orders, Payments, Products, Promotions, PurchaseOrders, Returns, Shipments, Subscriptions, Tax, Warranties, WorkOrders};
use stateset_core::CommerceError;
use stateset_db::{Database, DatabaseConfig};
use std::sync::Arc;

#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;

#[cfg(feature = "events")]
use crate::events::{EventSystem, EventConfig, EventSubscription, Webhook};

#[cfg(feature = "sqlite")]
use stateset_db::sqlite::SqliteTaxRepository;

#[cfg(feature = "sqlite")]
use stateset_db::sqlite::SqlitePromotionRepository;

#[cfg(feature = "sqlite")]
use stateset_db::sqlite::SqliteSubscriptionRepository;

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
    #[cfg(feature = "sqlite")]
    sqlite_db: Option<Arc<SqliteDatabase>>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
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

        let db = Arc::new(SqliteDatabase::new(&config)?);

        Ok(Self {
            db: db.clone(),
            sqlite_db: Some(db),
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        })
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

        Ok(Self {
            db: Arc::new(db),
            #[cfg(feature = "sqlite")]
            sqlite_db: None,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        })
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

        Ok(Self {
            db: Arc::new(db),
            #[cfg(feature = "sqlite")]
            sqlite_db: None,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        })
    }

    /// Create a Commerce instance with a pre-connected database.
    ///
    /// This is useful when you want to manage the database connection yourself.
    /// Note: Tax operations will not be available when using this method.
    pub fn with_database(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            #[cfg(feature = "sqlite")]
            sqlite_db: None,
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        }
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

    /// Access tax calculation and management operations.
    ///
    /// Provides multi-jurisdiction tax calculation with support for:
    /// - US sales tax (state, county, city levels)
    /// - EU VAT (standard, reduced, zero-rated)
    /// - Canadian GST/HST/PST/QST
    /// - Customer exemptions (resale, non-profit, etc.)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, TaxCalculationRequest, TaxLineItem, TaxAddress, ProductTaxCategory};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Calculate tax for a transaction
    /// let result = commerce.tax().calculate(TaxCalculationRequest {
    ///     line_items: vec![TaxLineItem {
    ///         id: "item-1".into(),
    ///         quantity: dec!(2),
    ///         unit_price: dec!(29.99),
    ///         tax_category: ProductTaxCategory::Standard,
    ///         ..Default::default()
    ///     }],
    ///     shipping_address: TaxAddress {
    ///         country: "US".into(),
    ///         state: Some("CA".into()),
    ///         ..Default::default()
    ///     },
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Tax: ${}", result.total_tax);
    /// println!("Total: ${}", result.total);
    ///
    /// // Check effective rate for an address
    /// let rate = commerce.tax().get_effective_rate(
    ///     &TaxAddress { country: "US".into(), state: Some("TX".into()), ..Default::default() },
    ///     ProductTaxCategory::Standard,
    /// )?;
    /// println!("Texas tax rate: {}%", rate * dec!(100));
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn tax(&self) -> Tax {
        let repo = self.sqlite_db
            .as_ref()
            .map(|db| SqliteTaxRepository::new(db.pool().clone()))
            .expect("Tax operations require SQLite database");
        Tax::new(repo)
    }

    /// Access promotions and discount operations.
    ///
    /// Provides comprehensive promotions engine supporting:
    /// - Percentage and fixed amount discounts
    /// - Buy X Get Y (BOGO) promotions
    /// - Free shipping offers
    /// - Tiered discounts based on spend/quantity
    /// - Coupon code management
    /// - Automatic promotions
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePromotion, PromotionType, ApplyPromotionsRequest, PromotionLineItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a 20% off promotion
    /// let promo = commerce.promotions().create(CreatePromotion {
    ///     name: "Summer Sale".into(),
    ///     promotion_type: PromotionType::PercentageOff,
    ///     percentage_off: Some(dec!(0.20)),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Activate it
    /// commerce.promotions().activate(promo.id)?;
    ///
    /// // Create a coupon code
    /// commerce.promotions().create_coupon(stateset_embedded::CreateCouponCode {
    ///     promotion_id: promo.id,
    ///     code: "SUMMER20".into(),
    ///     usage_limit: Some(100),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Apply promotions to a cart
    /// let result = commerce.promotions().apply(ApplyPromotionsRequest {
    ///     subtotal: dec!(100.00),
    ///     coupon_codes: vec!["SUMMER20".into()],
    ///     line_items: vec![PromotionLineItem {
    ///         id: "item-1".into(),
    ///         quantity: 2,
    ///         unit_price: dec!(50.00),
    ///         line_total: dec!(100.00),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Discount: ${}", result.total_discount);
    /// println!("Final total: ${}", result.grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn promotions(&self) -> Promotions {
        let repo = self.sqlite_db
            .as_ref()
            .map(|db| SqlitePromotionRepository::new(db.pool().clone()))
            .expect("Promotion operations require SQLite database");
        Promotions::new(repo)
    }

    /// Access subscription management operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateSubscriptionPlan, CreateSubscription, BillingInterval};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a subscription plan
    /// let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
    ///     name: "Monthly Coffee Box".into(),
    ///     billing_interval: BillingInterval::Monthly,
    ///     price: dec!(29.99),
    ///     trial_days: Some(14),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Activate the plan
    /// commerce.subscriptions().activate_plan(plan.id)?;
    ///
    /// // Subscribe a customer
    /// let subscription = commerce.subscriptions().subscribe(CreateSubscription {
    ///     customer_id: Uuid::new_v4(),
    ///     plan_id: plan.id,
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Subscription #{} created", subscription.subscription_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn subscriptions(&self) -> Subscriptions {
        let repo = self.sqlite_db
            .as_ref()
            .map(|db| SqliteSubscriptionRepository::new(db.pool().clone()))
            .expect("Subscription operations require SQLite database");
        Subscriptions::new(repo)
    }

    /// Calculate and apply tax to a cart based on its shipping address.
    ///
    /// This method:
    /// 1. Retrieves the cart and its items
    /// 2. Uses the shipping address to determine tax jurisdiction
    /// 3. Calculates tax for each item based on jurisdiction and product category
    /// 4. Applies customer exemptions if applicable
    /// 5. Updates the cart with the calculated tax
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
    /// // Create a cart with items
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("alice@example.com".into()),
    ///     ..Default::default()
    /// })?;
    ///
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
    ///     city: "Los Angeles".into(),
    ///     state: Some("CA".into()),
    ///     postal_code: "90210".into(),
    ///     country: "US".into(),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Calculate and apply tax
    /// let result = commerce.calculate_cart_tax(cart.id)?;
    /// println!("Tax: ${}", result.total_tax);
    /// println!("Updated cart total: ${}", commerce.carts().get(cart.id)?.unwrap().grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn calculate_cart_tax(&self, cart_id: uuid::Uuid) -> stateset_core::Result<stateset_core::TaxCalculationResult> {
        use stateset_core::{TaxAddress, TaxCalculationRequest, TaxLineItem, ProductTaxCategory};
        use rust_decimal::Decimal;

        // Get the cart
        let cart = self.carts().get(cart_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        // Need a shipping address to calculate tax
        let shipping_address = cart.shipping_address
            .ok_or_else(|| stateset_core::CommerceError::ValidationError("Shipping address required to calculate tax".into()))?;

        // Convert CartAddress to TaxAddress
        let tax_address = TaxAddress {
            country: shipping_address.country,
            state: shipping_address.state,
            city: Some(shipping_address.city),
            postal_code: Some(shipping_address.postal_code),
            line1: Some(shipping_address.line1),
            line2: shipping_address.line2,
        };

        // Convert cart items to TaxLineItems
        let line_items: Vec<TaxLineItem> = cart.items.iter().map(|item| {
            TaxLineItem {
                id: item.id.to_string(),
                sku: Some(item.sku.clone()),
                product_id: item.product_id,
                quantity: Decimal::from(item.quantity),
                unit_price: item.unit_price,
                discount_amount: item.discount_amount,
                tax_category: ProductTaxCategory::Standard, // Default to standard, can be enhanced
                tax_code: None,
                description: Some(item.name.clone()),
            }
        }).collect();

        // Build tax calculation request
        let request = TaxCalculationRequest {
            line_items,
            shipping_address: tax_address,
            customer_id: cart.customer_id,
            currency: cart.currency.clone(),
            shipping_amount: Some(cart.shipping_amount),
            ..Default::default()
        };

        // Calculate tax
        let result = self.tax().calculate(request)?;

        // Apply tax to cart
        self.carts().set_tax(cart_id, result.total_tax)?;

        Ok(result)
    }

    /// Calculate and apply promotions to a cart.
    ///
    /// This method:
    /// 1. Retrieves the cart and its items
    /// 2. Finds all applicable automatic promotions
    /// 3. Validates any coupon codes applied to the cart
    /// 4. Calculates discounts respecting stacking rules
    /// 5. Updates the cart with the calculated discount
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Create a cart with items
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("alice@example.com".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// commerce.carts().add_item(cart.id, AddCartItem {
    ///     sku: "SKU-001".into(),
    ///     name: "Widget".into(),
    ///     quantity: 2,
    ///     unit_price: dec!(49.99),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Apply a coupon code
    /// commerce.carts().apply_discount(cart.id, "SUMMER20")?;
    ///
    /// // Calculate and apply promotions
    /// let result = commerce.apply_cart_promotions(cart.id)?;
    /// println!("Discount: ${}", result.total_discount);
    /// println!("Applied promotions: {:?}", result.applied_promotions.len());
    ///
    /// // Cart now has discount applied
    /// let updated_cart = commerce.carts().get(cart.id)?.unwrap();
    /// println!("New total: ${}", updated_cart.grand_total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "sqlite")]
    pub fn apply_cart_promotions(&self, cart_id: uuid::Uuid) -> stateset_core::Result<stateset_core::ApplyPromotionsResult> {
        use stateset_core::{ApplyPromotionsRequest, PromotionLineItem};

        // Get the cart
        let cart = self.carts().get(cart_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        // Convert cart items to PromotionLineItems
        let line_items: Vec<PromotionLineItem> = cart.items.iter().map(|item| {
            PromotionLineItem {
                id: item.id.to_string(),
                product_id: item.product_id,
                variant_id: item.variant_id,
                sku: Some(item.sku.clone()),
                category_ids: vec![], // Could be enhanced to load from product
                quantity: item.quantity,
                unit_price: item.unit_price,
                line_total: item.total,
            }
        }).collect();

        // Build promotion request
        let coupon_codes = cart.coupon_code
            .map(|c| vec![c])
            .unwrap_or_default();

        let request = ApplyPromotionsRequest {
            cart_id: Some(cart_id),
            customer_id: cart.customer_id,
            subtotal: cart.subtotal,
            shipping_amount: cart.shipping_amount,
            shipping_country: cart.shipping_address.as_ref().map(|a| a.country.clone()),
            shipping_state: cart.shipping_address.as_ref().and_then(|a| a.state.clone()),
            currency: cart.currency.clone(),
            coupon_codes,
            line_items,
            is_first_order: false, // Could check customer order history
        };

        // Apply promotions
        let result = self.promotions().apply(request)?;

        // Update cart discount amount
        let conn = self.sqlite_db
            .as_ref()
            .expect("Promotion operations require SQLite database")
            .conn()?;

        let discount_description = result.applied_promotions.iter()
            .map(|p| p.promotion_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        conn.execute(
            "UPDATE carts SET discount_amount = ?, discount_description = ?, updated_at = ? WHERE id = ?",
            [
                &result.total_discount.to_string(),
                &discount_description,
                &chrono::Utc::now().to_rfc3339(),
                &cart_id.to_string(),
            ],
        ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Update individual item discounts if there are line item discounts
        for line_discount in &result.line_item_discounts {
            if let Ok(item_id) = line_discount.line_item_id.parse::<uuid::Uuid>() {
                conn.execute(
                    "UPDATE cart_items SET discount_amount = ?, updated_at = ? WHERE id = ?",
                    [
                        &line_discount.discount_amount.to_string(),
                        &chrono::Utc::now().to_rfc3339(),
                        &item_id.to_string(),
                    ],
                ).map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
            }
        }

        // Recalculate cart totals
        self.carts().recalculate(cart_id)?;

        // Record promotion usage for tracking
        for applied in &result.applied_promotions {
            // Look up coupon_id if a coupon code was used
            let coupon_id = if let Some(ref code) = applied.coupon_code {
                self.promotions().get_coupon_by_code(code)?
                    .map(|c| c.id)
            } else {
                None
            };

            let _ = self.promotions().record_usage(
                applied.promotion_id,
                coupon_id,
                cart.customer_id,
                None, // order_id - will be set when order is created
                Some(cart_id),
                applied.discount_amount,
                &cart.currency,
            );
        }

        Ok(result)
    }

    /// Access the event system for pub/sub and webhook management.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use futures::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let commerce = Commerce::new("./store.db")?;
    ///
    ///     // Subscribe to all events
    ///     let mut subscription = commerce.events().subscribe();
    ///
    ///     // Process events in background
    ///     tokio::spawn(async move {
    ///         while let Some(event) = subscription.next().await {
    ///             println!("Event: {:?}", event);
    ///         }
    ///     });
    ///
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "events")]
    pub fn events(&self) -> &EventSystem {
        &self.event_system
    }

    /// Subscribe to commerce events.
    ///
    /// This is a convenience method that returns an event subscription
    /// for receiving real-time commerce events.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use futures::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let commerce = Commerce::new("./store.db")?;
    ///
    ///     let mut subscription = commerce.subscribe_events();
    ///
    ///     while let Some(event) = subscription.next().await {
    ///         match event {
    ///             stateset_core::CommerceEvent::OrderCreated { order_id, .. } => {
    ///                 println!("New order: {}", order_id);
    ///             }
    ///             _ => {}
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "events")]
    pub fn subscribe_events(&self) -> EventSubscription {
        self.event_system.subscribe()
    }

    /// Register a webhook endpoint for event delivery.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, Webhook};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let webhook = Webhook::new(
    ///     "My Webhook",
    ///     "https://my-app.com/webhooks/stateset",
    /// ).with_secret("my-webhook-secret");
    ///
    /// if let Some(id) = commerce.register_webhook(webhook) {
    ///     println!("Webhook registered: {}", id);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "events")]
    pub fn register_webhook(&self, webhook: Webhook) -> Option<uuid::Uuid> {
        self.event_system.register_webhook(webhook)
    }

    /// Unregister a webhook endpoint.
    #[cfg(feature = "events")]
    pub fn unregister_webhook(&self, id: uuid::Uuid) -> bool {
        self.event_system.unregister_webhook(id)
    }

    /// List all registered webhooks.
    #[cfg(feature = "events")]
    pub fn list_webhooks(&self) -> Vec<Webhook> {
        self.event_system.list_webhooks()
    }

    /// Emit a commerce event manually.
    ///
    /// Events are typically emitted automatically by commerce operations,
    /// but you can also emit custom events using this method.
    #[cfg(feature = "events")]
    pub fn emit_event(&self, event: stateset_core::CommerceEvent) {
        self.event_system.emit(event);
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
    #[cfg(feature = "events")]
    event_config: Option<EventConfig>,
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

    /// Configure the event system.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, EventConfig};
    ///
    /// let commerce = Commerce::builder()
    ///     .database(":memory:")
    ///     .event_config(EventConfig {
    ///         channel_capacity: 2048,
    ///         enable_webhooks: true,
    ///         ..Default::default()
    ///     })
    ///     .build()?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "events")]
    pub fn event_config(mut self, config: EventConfig) -> Self {
        self.event_config = Some(config);
        self
    }

    /// Build the Commerce instance.
    pub fn build(self) -> Result<Commerce, CommerceError> {
        // Create event system if events feature is enabled
        #[cfg(feature = "events")]
        let event_system = Arc::new(
            self.event_config
                .map(EventSystem::with_config)
                .unwrap_or_else(EventSystem::new)
        );

        // Check if PostgreSQL URL is set
        #[cfg(feature = "postgres")]
        if let Some(url) = self.postgres_url {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {}", e)))?;

            let db = rt.block_on(PostgresDatabase::connect_with_options(
                &url,
                self.max_connections.unwrap_or(10),
                self.acquire_timeout_secs.unwrap_or(30),
            ))?;

            return Ok(Commerce {
                db: Arc::new(db),
                #[cfg(feature = "sqlite")]
                sqlite_db: None,
                #[cfg(feature = "events")]
                event_system,
            });
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

            let db = Arc::new(SqliteDatabase::new(&config)?);
            return Ok(Commerce {
                db: db.clone(),
                sqlite_db: Some(db),
                #[cfg(feature = "events")]
                event_system,
            });
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

    #[test]
    #[cfg(all(feature = "sqlite", feature = "events"))]
    fn test_event_system_basic() {
        let commerce = Commerce::new(":memory:").unwrap();

        // Verify event system is accessible
        let event_system = commerce.events();
        assert_eq!(event_system.subscriber_count(), 0);

        // Subscribe to events
        let _sub = commerce.subscribe_events();
        assert_eq!(commerce.events().subscriber_count(), 1);
    }

    #[test]
    #[cfg(all(feature = "sqlite", feature = "events"))]
    fn test_event_system_builder() {
        use crate::events::EventConfig;

        let commerce = Commerce::builder()
            .database(":memory:")
            .event_config(EventConfig {
                channel_capacity: 512,
                enable_webhooks: false,
                ..Default::default()
            })
            .build()
            .unwrap();

        // Verify custom config is applied
        assert_eq!(commerce.events().config().channel_capacity, 512);
        assert!(!commerce.events().config().enable_webhooks);
    }

    #[tokio::test]
    #[cfg(all(feature = "sqlite", feature = "events"))]
    async fn test_event_subscription() {
        use stateset_core::CommerceEvent;
        use chrono::Utc;
        use uuid::Uuid;

        let commerce = Commerce::new(":memory:").unwrap();
        let mut subscription = commerce.subscribe_events();

        // Emit a test event
        let event = CommerceEvent::CustomerCreated {
            customer_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            timestamp: Utc::now(),
        };

        commerce.emit_event(event.clone());

        // Receive the event
        let received = subscription.try_recv();
        assert!(received.is_some());

        if let Some(CommerceEvent::CustomerCreated { email, .. }) = received {
            assert_eq!(email, "test@example.com");
        } else {
            panic!("Expected CustomerCreated event");
        }
    }

    #[test]
    #[cfg(all(feature = "sqlite", feature = "events"))]
    fn test_webhook_registration() {
        use crate::events::Webhook;

        let commerce = Commerce::new(":memory:").unwrap();

        let webhook = Webhook::new(
            "Test Webhook",
            "https://example.com/webhook",
        );

        // Register webhook
        let id = commerce.register_webhook(webhook).unwrap();

        // Verify it's registered
        let webhooks = commerce.list_webhooks();
        assert_eq!(webhooks.len(), 1);
        assert_eq!(webhooks[0].id, id);

        // Unregister
        assert!(commerce.unregister_webhook(id));
        assert!(commerce.list_webhooks().is_empty());
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_promotions_create_and_list() {
        use stateset_core::{CreatePromotion, PromotionType, PromotionStatus, PromotionTrigger, PromotionTarget, StackingBehavior};
        use rust_decimal_macros::dec;

        let commerce = Commerce::new(":memory:").unwrap();

        // Create a percentage off promotion
        let promo = commerce.promotions().create(CreatePromotion {
            code: None,
            name: "Summer Sale".into(),
            description: Some("Get 20% off your order".into()),
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.20)),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        }).unwrap();

        assert_eq!(promo.name, "Summer Sale");
        assert_eq!(promo.promotion_type, PromotionType::PercentageOff);
        assert_eq!(promo.percentage_off, Some(dec!(0.20)));
        assert_eq!(promo.status, PromotionStatus::Draft);

        // Activate the promotion
        let promo = commerce.promotions().activate(promo.id).unwrap();
        assert_eq!(promo.status, PromotionStatus::Active);

        // List active promotions
        let active = commerce.promotions().get_active().unwrap();
        assert!(!active.is_empty());

        // Deactivate
        let promo = commerce.promotions().deactivate(promo.id).unwrap();
        assert_eq!(promo.status, PromotionStatus::Paused);
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_promotions_coupon_codes() {
        use stateset_core::{CreatePromotion, CreateCouponCode, PromotionType, CouponStatus, PromotionTrigger, PromotionTarget, StackingBehavior};
        use rust_decimal_macros::dec;

        let commerce = Commerce::new(":memory:").unwrap();

        // Create a promotion
        let promo = commerce.promotions().create(CreatePromotion {
            code: None,
            name: "VIP Discount".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.15)),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        }).unwrap();

        commerce.promotions().activate(promo.id).unwrap();

        // Create a coupon code
        let coupon = commerce.promotions().create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "VIP15".into(),
            usage_limit: Some(100),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        }).unwrap();

        assert_eq!(coupon.code, "VIP15");
        assert_eq!(coupon.status, CouponStatus::Active);
        assert_eq!(coupon.usage_limit, Some(100));
        assert_eq!(coupon.usage_count, 0);

        // Validate the coupon
        let validated = commerce.promotions().validate_coupon("VIP15").unwrap();
        assert!(validated.is_some());

        // Invalid coupon
        let invalid = commerce.promotions().validate_coupon("INVALID").unwrap();
        assert!(invalid.is_none());
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_promotions_fixed_amount() {
        use stateset_core::{CreatePromotion, PromotionType, ApplyPromotionsRequest, PromotionLineItem, PromotionTrigger, PromotionTarget, StackingBehavior};
        use rust_decimal_macros::dec;

        let commerce = Commerce::new(":memory:").unwrap();

        // Create $10 off automatic promotion
        let promo = commerce.promotions().create(CreatePromotion {
            code: None,
            name: "$10 Off".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: None,
            fixed_amount_off: Some(dec!(10.00)),
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        }).unwrap();

        commerce.promotions().activate(promo.id).unwrap();

        // Apply to a $100 order
        let result = commerce.promotions().apply(ApplyPromotionsRequest {
            cart_id: None,
            customer_id: None,
            subtotal: dec!(100.00),
            shipping_amount: dec!(5.00),
            shipping_country: None,
            shipping_state: None,
            currency: "USD".into(),
            coupon_codes: vec![],
            line_items: vec![PromotionLineItem {
                id: "item-1".into(),
                product_id: None,
                variant_id: None,
                sku: Some("SKU-001".into()),
                category_ids: vec![],
                quantity: 2,
                unit_price: dec!(50.00),
                line_total: dec!(100.00),
            }],
            is_first_order: false,
        }).unwrap();

        assert_eq!(result.total_discount, dec!(10.00));
        assert_eq!(result.discounted_subtotal, dec!(90.00));
        assert!(!result.applied_promotions.is_empty());
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_cart_promotions_integration() {
        use stateset_core::{CreateCart, AddCartItem, CreatePromotion, CreateCouponCode, PromotionType, PromotionTrigger, PromotionTarget, StackingBehavior};
        use rust_decimal_macros::dec;

        let commerce = Commerce::new(":memory:").unwrap();

        // Create a 25% off promotion with coupon
        let promo = commerce.promotions().create(CreatePromotion {
            code: None,
            name: "25% Off Everything".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.25)),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        }).unwrap();

        commerce.promotions().activate(promo.id).unwrap();

        commerce.promotions().create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "SAVE25".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        }).unwrap();

        // Create a cart with items
        let cart = commerce.carts().create(CreateCart {
            customer_email: Some("test@example.com".into()),
            ..Default::default()
        }).unwrap();

        commerce.carts().add_item(cart.id, AddCartItem {
            sku: "ITEM-001".into(),
            name: "Test Product".into(),
            quantity: 2,
            unit_price: dec!(50.00),
            ..Default::default()
        }).unwrap();

        // Apply coupon code
        commerce.carts().apply_discount(cart.id, "SAVE25").unwrap();

        // Calculate promotions
        let result = commerce.apply_cart_promotions(cart.id).unwrap();

        // 25% off of $100 = $25 discount
        assert_eq!(result.total_discount, dec!(25.00));
        assert_eq!(result.applied_promotions.len(), 1);
        assert_eq!(result.applied_promotions[0].promotion_name, "25% Off Everything");

        // Verify cart was updated
        let updated_cart = commerce.carts().get(cart.id).unwrap().unwrap();
        assert_eq!(updated_cart.discount_amount, dec!(25.00));
        assert_eq!(updated_cart.grand_total, dec!(75.00)); // $100 - $25
    }
}
