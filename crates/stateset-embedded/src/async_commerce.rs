//! Async Commerce API for PostgreSQL
//!
//! This module provides async access to commerce operations when using PostgreSQL.
//! All methods are truly async (no blocking).
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{AsyncCommerce, CreateOrder, CreateOrderItem, CreateX402PaymentIntent, X402Asset, X402Network};
//! use rust_decimal_macros::dec;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let commerce = AsyncCommerce::connect("postgres://localhost/stateset").await?;
//!
//!     let cart_id = uuid::Uuid::new_v4();
//!
//!     // Ecommerce orders
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
//!     // Agentic commerce payment intent
//!     let intent = commerce
//!         .x402()
//!         .create_intent(CreateX402PaymentIntent {
//!             payer_address: "0xBuyer...".into(),
//!             payee_address: "0xSeller...".into(),
//!             amount: 100_000_000,
//!             asset: X402Asset::Usdc,
//!             network: X402Network::SetChain,
//!             cart_id: Some(cart_id),
//!             ..Default::default()
//!         })
//!         .await?;
//!
//!     let _active = commerce.x402().active_intent_for_cart(cart_id).await?;
//!
//!     Ok(())
//! }
//! ```

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use stateset_core::{
    // Cart types
    AddCartItem,
    // Shipment types
    AddShipmentEvent,
    // Work Order types
    AddWorkOrderMaterial,
    // Inventory types
    AdjustInventory,
    // Analytics types
    AnalyticsQuery,
    // BOM types
    BillOfMaterials,
    BomComponent,
    BomFilter,
    Cart,
    CartAddress,
    CartFilter,
    CartItem,
    CheckoutResult,
    // Warranty types
    ClaimResolution,
    // Currency types
    ConversionResult,
    ConvertCurrency,
    CreateBom,
    CreateBomComponent,
    CreateCart,
    // Customer types
    CreateCustomer,
    CreateCustomerAddress,
    CreateInventoryItem,
    // Invoice types
    CreateInvoice,
    CreateInvoiceItem,
    // Order types
    CreateOrder,
    CreateOrderItem,
    // Payment types
    CreatePayment,
    CreatePaymentMethod,
    // Product types
    CreateProduct,
    CreateProductVariant,
    // Purchase Order types
    CreatePurchaseOrder,
    CreatePurchaseOrderItem,
    CreateRefund,
    // Return types
    CreateReturn,
    CreateShipment,
    CreateShipmentItem,
    CreateSupplier,
    CreateWarranty,
    CreateWarrantyClaim,
    CreateWorkOrder,
    CreateWorkOrderTask,
    Currency,
    Customer,
    CustomerAddress,
    CustomerFilter,
    CustomerMetrics,
    DemandForecast,
    ExchangeRate,
    ExchangeRateFilter,
    FulfillmentMetrics,
    InventoryBalance,
    InventoryFilter,
    InventoryHealth,
    InventoryItem,
    InventoryMovement,
    InventoryReservation,
    InventoryTransaction,
    Invoice,
    InvoiceFilter,
    InvoiceItem,
    LowStockItem,
    Order,
    OrderFilter,
    OrderItem,
    OrderStatus,
    OrderStatusBreakdown,
    Payment,
    PaymentFilter,
    PaymentMethod,
    PaymentStatus,
    Product,
    ProductFilter,
    ProductPerformance,
    ProductVariant,
    PurchaseOrder,
    PurchaseOrderFilter,
    PurchaseOrderItem,
    ReceivePurchaseOrderItems,
    RecordInvoicePayment,
    Refund,
    ReserveInventory,
    Result,
    Return,
    ReturnFilter,
    ReturnMetrics,
    RevenueByPeriod,
    RevenueForecast,
    SalesSummary,
    SetCartPayment,
    SetCartShipping,
    SetExchangeRate,
    Shipment,
    ShipmentEvent,
    ShipmentFilter,
    ShipmentItem,
    ShippingRate,
    StockLevel,
    StoreCurrencySettings,
    Supplier,
    SupplierFilter,
    TimeGranularity,
    TopCustomer,
    TopProduct,
    UpdateBom,
    UpdateCart,
    UpdateCartItem,
    UpdateCustomer,
    UpdateInvoice,
    UpdateOrder,
    UpdatePayment,
    UpdateProduct,
    UpdatePurchaseOrder,
    UpdateReturn,
    UpdateShipment,
    UpdateSupplier,
    UpdateWarranty,
    UpdateWarrantyClaim,
    UpdateWorkOrder,
    UpdateWorkOrderTask,
    Warranty,
    WarrantyClaim,
    WarrantyClaimFilter,
    WarrantyFilter,
    WorkOrder,
    WorkOrderFilter,
    WorkOrderMaterial,
    WorkOrderTask,
};
use stateset_core::{
    AddCarton,
    AddCartonItem,
    AddLotCertificate,
    AdjustLocationInventory,
    AdjustLot,
    AllocateBackorder,
    ApAgingSummary,
    ApplyCreditMemo,
    ApplyPaymentToInvoices,
    ApplyPromotionsRequest,
    ApplyPromotionsResult,
    ArAgingFilter,
    ArAgingSummary,
    ArPaymentApplication,
    AutoPostingConfig,
    Backorder,
    BackorderAllocation,
    BackorderFilter,
    BackorderFulfillment,
    BackorderSummary,
    BalanceSheet,
    // Batch types
    BatchResult,
    Bill,
    BillFilter,
    BillItem,
    BillPayment,
    BillPaymentFilter,
    BillingCycle,
    BillingCycleFilter,
    BillingCycleStatus,
    CancelSubscription,
    Carton,
    CartonItem,
    ChangeSerialStatus,
    CollectionActivity,
    CollectionActivityFilter,
    CollectionStatus,
    CompletePick,
    CompletePutAway,
    CompleteShip,
    ConsumeLot,
    CostAdjustment,
    CostAdjustmentFilter,
    CostLayer,
    CostLayerFilter,
    CostMethod,
    CostRollup,
    CostTransaction,
    CostTransactionFilter,
    CostVariance,
    CostVarianceFilter,
    CouponCode,
    CouponFilter,
    CreateAutoPostingConfig,
    // Backorder types
    CreateBackorder,
    // Accounts Payable types
    CreateBill,
    CreateBillItem,
    CreateBillPayment,
    CreateBillingCycle,
    // Accounts Receivable types
    CreateCollectionActivity,
    CreateCostAdjustment,
    CreateCostLayer,
    CreateCouponCode,
    // Credit types
    CreateCreditAccount,
    CreateCreditMemo,
    CreateDefectCode,
    // General Ledger types
    CreateGlAccount,
    CreateGlPeriod,
    // Quality types
    CreateInspection,
    CreateJournalEntry,
    CreateLocation,
    // Lot types
    CreateLot,
    CreateNonConformance,
    CreatePackTask,
    CreatePaymentRun,
    CreatePickTask,
    // Promotion types
    CreatePromotion,
    CreatePutAway,
    CreateQualityHold,
    // Receiving types
    CreateReceipt,
    // Serial types
    CreateSerialNumber,
    CreateSerialNumbersBulk,
    CreateShipTask,
    CreateSubscription,
    // Subscription types
    CreateSubscriptionPlan,
    CreateTaxExemption,
    // Tax types
    CreateTaxJurisdiction,
    CreateTaxRate,
    // Warehouse types
    CreateWarehouse,
    // Fulfillment types
    CreateWave,
    CreateWriteOff,
    CreateZone,
    CreditAccount,
    CreditAccountFilter,
    CreditAgingBucket,
    CreditApplication,
    CreditApplicationFilter,
    CreditCheckResult,
    CreditHold,
    CreditHoldFilter,
    CreditMemo,
    CreditMemoFilter,
    CreditTransaction,
    CreditTransactionFilter,
    CustomerArAging,
    CustomerArSummary,
    CustomerCreditSummary,
    CustomerStatement,
    DefectCode,
    DunningLetterType,
    FulfillBackorder,
    GenerateStatementRequest,
    GlAccount,
    GlAccountFilter,
    GlPeriod,
    GlPeriodFilter,
    IncomeStatement,
    Inspection,
    InspectionFilter,
    InspectionItem,
    InventoryValuation,
    IssueCostLayers,
    // Cost Accounting types
    ItemCost,
    ItemCostFilter,
    JournalEntry,
    JournalEntryFilter,
    JournalEntryLine,
    Location,
    LocationFilter,
    LocationInventory,
    LocationInventoryFilter,
    LocationMovement,
    Lot,
    LotCertificate,
    LotFilter,
    LotLocation,
    LotTransaction,
    MergeLots,
    MoveInventory,
    MoveSerial,
    MovementFilter,
    NonConformance,
    NonConformanceFilter,
    PackTask,
    PackTaskFilter,
    PauseSubscription,
    PaymentAllocation,
    PaymentRun,
    PaymentRunFilter,
    PickTask,
    PickTaskFilter,
    PlaceCreditHold,
    ProductTaxCategory,
    Promotion,
    PromotionFilter,
    PromotionUsage,
    PutAway,
    PutAwayFilter,
    QualityHold,
    QualityHoldFilter,
    Receipt,
    ReceiptFilter,
    ReceiptItem,
    ReceiveItems,
    RecordCostVariance,
    RecordCreditTransaction,
    RecordInspectionResult,
    ReleaseCreditHold,
    ReleaseQualityHold,
    ReserveLot,
    ReserveSerialNumber,
    ReviewCreditApplication,
    SerialFilter,
    SerialHistory,
    SerialHistoryFilter,
    SerialLookupResult,
    SerialNumber,
    SerialReservation,
    SerialValidation,
    SetItemCost,
    ShipTask,
    ShipTaskFilter,
    SkipBillingCycle,
    SkuBackorderSummary,
    SkuCostSummary,
    SplitLot,
    SubmitCreditApplication,
    Subscription,
    SubscriptionEvent,
    SubscriptionEventType,
    SubscriptionFilter,
    SubscriptionPlan,
    SubscriptionPlanFilter,
    SupplierApSummary,
    TaxAddress,
    TaxCalculationRequest,
    TaxCalculationResult,
    TaxExemption,
    TaxJurisdiction,
    TaxJurisdictionFilter,
    TaxRate,
    TaxRateFilter,
    TaxSettings,
    TraceabilityResult,
    TransferLot,
    TransferSerialOwnership,
    TrialBalance,
    UpdateBackorder,
    UpdateBill,
    UpdateCreditAccount,
    UpdateGlAccount,
    UpdateInspection,
    UpdateLocation,
    UpdateLot,
    UpdateNonConformance,
    UpdatePromotion,
    UpdateReceipt,
    UpdateSerialNumber,
    UpdateSubscription,
    UpdateSubscriptionPlan,
    UpdateWarehouse,
    UpdateZone,
    Warehouse,
    WarehouseFilter,
    Wave,
    WaveFilter,
    WriteOff,
    WriteOffFilter,
    Zone,
};
use stateset_core::{
    // Cycle count types
    CreateCycleCount,
    // Fixed asset types
    CreateFixedAsset,
    // Revenue recognition types
    CreateRevenueContract,
    CycleCount,
    CycleCountFilter,
    DepreciationSchedule,
    FixedAsset,
    FixedAssetFilter,
    PerformanceObligation,
    RecordCycleCountLine,
    RevenueContract,
    RevenueContractFilter,
    RevenueSchedule,
    UpdateFixedAsset,
    UpdateRevenueContract,
};
use stateset_db::PostgresDatabase;
use stateset_observability::{Metrics, MetricsConfig, MetricsSnapshot, init_metrics};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

use stateset_core::{
    A2APurchase, A2APurchaseFilter, A2ASkill, AgentCard, AgentCardFilter, CreateA2APurchase,
    CreateA2AQuote, CreateAgentCard, CreateX402PaymentIntent, PurchaseStatus, QuoteStatus,
    SignX402PaymentIntent, SkillQuote, SkillQuoteFilter, TrustLevel, UpdateAgentCard, X402Asset,
    X402CreditAccount, X402CreditAdjustment, X402CreditDirection, X402CreditTransaction,
    X402CreditTransactionFilter, X402IntentStatus, X402Network, X402PaymentIntent,
    X402PaymentIntentFilter, to_smallest_unit,
};
use stateset_core::{
    CreateCustomObject, CreateCustomObjectType, CustomObject, CustomObjectFilter, CustomObjectType,
    CustomObjectTypeFilter, UpdateCustomObject, UpdateCustomObjectType,
};

macro_rules! impl_opaque_debug {
    ($($name:ident),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(stringify!($name)).finish_non_exhaustive()
                }
            }
        )+
    };
}

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
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl AsyncCommerce {
    /// Connect to PostgreSQL and create an async commerce instance.
    ///
    /// # Arguments
    ///
    /// * `url` - PostgreSQL connection string (e.g., `<postgres://user:pass@localhost/db>`)
    pub async fn connect(url: &str) -> Result<Self> {
        let db = PostgresDatabase::connect(url).await?;
        Ok(Self {
            db: Arc::new(db),
            metrics: init_metrics(MetricsConfig::default()),
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        })
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
        Ok(Self {
            db: Arc::new(db),
            metrics: init_metrics(MetricsConfig::default()),
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        })
    }

    /// Create from an existing `PostgresDatabase` instance.
    pub fn from_database(db: Arc<PostgresDatabase>) -> Self {
        Self {
            db,
            metrics: init_metrics(MetricsConfig::default()),
            #[cfg(feature = "events")]
            event_system: Arc::new(EventSystem::new()),
        }
    }

    /// Access async order operations.
    pub fn orders(&self) -> AsyncOrders {
        AsyncOrders::new(
            self.db.clone(),
            self.metrics.clone(),
            #[cfg(feature = "events")]
            self.event_system.clone(),
        )
    }

    /// Access the event system for pub/sub and webhook management.
    ///
    /// Mirrors [`Commerce::events`](crate::Commerce::events) so that async
    /// PostgreSQL users can subscribe to the same [`CommerceEvent`] stream that
    /// the sync facade emits.
    #[cfg(feature = "events")]
    #[must_use]
    pub fn events(&self) -> &EventSystem {
        &self.event_system
    }

    /// Access async inventory operations.
    pub fn inventory(&self) -> AsyncInventory {
        AsyncInventory::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async customer operations.
    pub fn customers(&self) -> AsyncCustomers {
        AsyncCustomers::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async product operations.
    pub fn products(&self) -> AsyncProducts {
        AsyncProducts::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async custom objects operations (custom states / metaobjects).
    pub fn custom_objects(&self) -> AsyncCustomObjects {
        AsyncCustomObjects::new(self.db.clone())
    }

    /// Alias for `custom_objects` (for users who prefer the "custom states" name).
    pub fn custom_states(&self) -> AsyncCustomObjects {
        self.custom_objects()
    }

    /// Access async return operations.
    pub fn returns(&self) -> AsyncReturns {
        AsyncReturns::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async shipment operations.
    pub fn shipments(&self) -> AsyncShipments {
        AsyncShipments::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async payment operations.
    pub fn payments(&self) -> AsyncPayments {
        AsyncPayments::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async warranty operations.
    pub fn warranties(&self) -> AsyncWarranties {
        AsyncWarranties::new(self.db.clone())
    }

    /// Access async gift card operations.
    pub fn gift_cards(&self) -> AsyncGiftCards {
        AsyncGiftCards::new(self.db.clone())
    }

    /// Access async store credit operations.
    pub fn store_credits(&self) -> AsyncStoreCredits {
        AsyncStoreCredits::new(self.db.clone())
    }

    /// Access async loyalty operations.
    pub fn loyalty(&self) -> AsyncLoyalty {
        AsyncLoyalty::new(self.db.clone())
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
        AsyncCarts::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async analytics operations.
    pub fn analytics(&self) -> AsyncAnalytics {
        AsyncAnalytics::new(self.db.clone())
    }

    /// Access async currency operations.
    pub fn currency(&self) -> AsyncCurrency {
        AsyncCurrency::new(self.db.clone())
    }

    /// Access async tax operations.
    pub fn tax(&self) -> AsyncTax {
        AsyncTax::new(self.db.clone())
    }

    /// Access async promotions operations.
    pub fn promotions(&self) -> AsyncPromotions {
        AsyncPromotions::new(self.db.clone())
    }

    /// Access async subscriptions operations.
    pub fn subscriptions(&self) -> AsyncSubscriptions {
        AsyncSubscriptions::new(self.db.clone(), self.metrics.clone())
    }

    /// Access async quality operations.
    pub fn quality(&self) -> AsyncQuality {
        AsyncQuality::new(self.db.clone())
    }

    /// Access async lot operations.
    pub fn lots(&self) -> AsyncLots {
        AsyncLots::new(self.db.clone())
    }

    /// Access async serial operations.
    pub fn serials(&self) -> AsyncSerials {
        AsyncSerials::new(self.db.clone())
    }

    /// Access async warehouse operations.
    pub fn warehouse(&self) -> AsyncWarehouse {
        AsyncWarehouse::new(self.db.clone())
    }

    /// Access async receiving operations.
    pub fn receiving(&self) -> AsyncReceiving {
        AsyncReceiving::new(self.db.clone())
    }

    /// Access async fulfillment operations.
    pub fn fulfillment(&self) -> AsyncFulfillment {
        AsyncFulfillment::new(self.db.clone())
    }

    /// Access async accounts payable operations.
    pub fn accounts_payable(&self) -> AsyncAccountsPayable {
        AsyncAccountsPayable::new(self.db.clone())
    }

    /// Access async cost accounting operations.
    pub fn cost_accounting(&self) -> AsyncCostAccounting {
        AsyncCostAccounting::new(self.db.clone())
    }

    /// Access async credit operations.
    pub fn credit(&self) -> AsyncCredit {
        AsyncCredit::new(self.db.clone())
    }

    /// Access async backorder operations.
    pub fn backorder(&self) -> AsyncBackorder {
        AsyncBackorder::new(self.db.clone())
    }

    /// Access async accounts receivable operations.
    pub fn accounts_receivable(&self) -> AsyncAccountsReceivable {
        AsyncAccountsReceivable::new(self.db.clone())
    }

    /// Access async general ledger operations.
    pub fn general_ledger(&self) -> AsyncGeneralLedger {
        AsyncGeneralLedger::new(self.db.clone())
    }

    /// Access async fixed asset register operations.
    pub fn fixed_assets(&self) -> AsyncFixedAssets {
        AsyncFixedAssets::new(self.db.clone())
    }

    /// Access async revenue recognition operations.
    pub fn revenue_recognition(&self) -> AsyncRevenueRecognition {
        AsyncRevenueRecognition::new(self.db.clone())
    }

    /// Access async x402 and A2A operations.
    pub fn x402(&self) -> AsyncX402 {
        AsyncX402::new(self.db.clone())
    }

    /// Get the underlying database for advanced operations.
    pub fn database(&self) -> &PostgresDatabase {
        &self.db
    }

    /// Access async metrics handle.
    pub const fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Return a point-in-time metrics snapshot.
    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }
}

// ============================================================================
// Async Orders
// ============================================================================

/// Async order operations.
pub struct AsyncOrders {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl AsyncOrders {
    pub(crate) const fn new(
        db: Arc<PostgresDatabase>,
        metrics: Metrics,
        #[cfg(feature = "events")] event_system: Arc<EventSystem>,
    ) -> Self {
        Self {
            db,
            metrics,
            #[cfg(feature = "events")]
            event_system,
        }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    /// Create a new order.
    ///
    /// Mirrors the sync [`Orders::create`](crate::Orders::create) behaviour:
    /// it computes the expected order total via `stateset_pricing`, warns on
    /// pricing drift between the persisted total and the engine total, records
    /// metrics, and emits a [`CommerceEvent::OrderCreated`] event.
    pub async fn create(&self, input: CreateOrder) -> Result<Order> {
        // Validate pricing: compute expected total via pricing engine.
        let pricing_items: Vec<stateset_pricing::LineItem> = input
            .items
            .iter()
            .map(|item| stateset_pricing::LineItem {
                sku: item.sku.clone(),
                name: item.name.clone(),
                unit_price: item.unit_price,
                quantity: item.quantity as u32,
                discount: item.discount.map(stateset_pricing::LineDiscount::FixedAmount),
                tax_rate: None,
            })
            .collect();
        let pricing_input = stateset_pricing::OrderTotalInput {
            items: pricing_items,
            shipping_cost: Decimal::ZERO,
            shipping_tax_rate: None,
            order_discount: None,
            fees: vec![],
            rounding: {
                let minor = stateset_pricing::minor_units_for_currency(
                    &input.currency.unwrap_or_default().to_string(),
                );
                stateset_pricing::RoundingPolicy::new(stateset_pricing::RoundingMode::HalfUp, minor)
            },
        };
        let pricing_total = stateset_pricing::try_compute_order_total(&pricing_input).ok();
        if let Some(ref computed) = pricing_total {
            tracing::debug!(
                computed_total = %computed.grand_total,
                subtotal = %computed.subtotal,
                discount = %computed.total_discount,
                "Pricing engine computed order total"
            );
        }

        let order = self.db.orders().create_async(input).await?;

        // Detect pricing drift: warn if DB total diverges from pricing engine.
        if let Some(computed) = pricing_total {
            let diff = (order.total_amount - computed.grand_total).abs();
            if diff > Decimal::new(1, 2) {
                // Divergence > $0.01
                tracing::warn!(
                    order_id = %order.id,
                    db_total = %order.total_amount,
                    engine_total = %computed.grand_total,
                    diff = %diff,
                    "Pricing drift detected: DB total differs from pricing engine"
                );
            }
        }

        self.metrics.record_order_created(
            &order.customer_id.to_string(),
            order.total_amount.to_f64().unwrap_or(0.0),
        );

        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::OrderCreated {
                order_id: order.id,
                customer_id: order.customer_id,
                total_amount: order.total_amount,
                item_count: order.items.len(),
                timestamp: order.created_at,
            });
        }
        Ok(order)
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
        let mut tracking_number = None;
        let mut payment_status = None;
        if status == OrderStatus::Shipped {
            if let Some(order) = self.get(id).await? {
                if order.tracking_number.is_none() {
                    tracking_number = Some(format!("AUTO-{}", id));
                }
            }
        }
        if status == OrderStatus::Refunded {
            payment_status = Some(PaymentStatus::Refunded);
        }
        self.update(
            id,
            UpdateOrder {
                status: Some(status),
                payment_status,
                tracking_number,
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
            .list_async(OrderFilter { customer_id: Some(customer_id.into()), ..Default::default() })
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
        if let Some(order) = self.get(id).await? {
            match order.status {
                OrderStatus::Pending => {
                    self.update_status(id, OrderStatus::Confirmed).await?;
                    self.update_status(id, OrderStatus::Processing).await?;
                }
                OrderStatus::Confirmed => {
                    self.update_status(id, OrderStatus::Processing).await?;
                }
                _ => {}
            }
        }
        self.update(
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
    metrics: Metrics,
}

impl AsyncInventory {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
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
    pub async fn get_balance(
        &self,
        item_id: i64,
        location_id: i32,
    ) -> Result<Option<InventoryBalance>> {
        self.db.inventory().get_balance_async(item_id, location_id).await
    }

    /// Adjust inventory quantity.
    pub async fn adjust_inventory(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        let sku = input.sku.clone();
        let delta = input.quantity.to_f64().unwrap_or(0.0);
        let transaction = self.db.inventory().adjust_async(input).await?;
        self.metrics.record_inventory_adjusted(&sku, delta);
        Ok(transaction)
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

    /// List reservations by reference (e.g., order id).
    pub async fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>> {
        self.db.inventory().list_reservations_by_reference_async(reference_type, reference_id).await
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
    pub async fn get_transactions(
        &self,
        item_id: i64,
        limit: u32,
    ) -> Result<Vec<InventoryTransaction>> {
        self.db.inventory().get_transactions_async(item_id, limit).await
    }
}

// ============================================================================
// Async Customers
// ============================================================================

/// Async customer operations.
pub struct AsyncCustomers {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncCustomers {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new customer.
    pub async fn create(&self, input: CreateCustomer) -> Result<Customer> {
        let customer = self.db.customers().create_async(input).await?;
        self.metrics.record_customer_created(&customer.id.to_string());
        Ok(customer)
    }

    /// Get customer by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Customer>> {
        self.db.customers().get_async(id.into()).await
    }

    /// Get customer by email.
    pub async fn get_by_email(&self, email: &str) -> Result<Option<Customer>> {
        self.db.customers().get_by_email_async(email).await
    }

    /// Update a customer.
    pub async fn update(&self, id: Uuid, input: UpdateCustomer) -> Result<Customer> {
        self.db.customers().update_async(id.into(), input).await
    }

    /// List customers.
    pub async fn list(&self, filter: CustomerFilter) -> Result<Vec<Customer>> {
        self.db.customers().list_async(filter).await
    }

    /// Delete a customer.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.customers().delete_async(id.into()).await
    }

    /// Add an address to a customer.
    pub async fn add_address(&self, input: CreateCustomerAddress) -> Result<CustomerAddress> {
        self.db.customers().add_address_async(input).await
    }

    /// Get addresses for a customer.
    pub async fn get_addresses(&self, customer_id: Uuid) -> Result<Vec<CustomerAddress>> {
        self.db.customers().get_addresses_async(customer_id.into()).await
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
    metrics: Metrics,
}

impl AsyncProducts {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new product.
    pub async fn create(&self, input: CreateProduct) -> Result<Product> {
        let product = self.db.products().create_async(input).await?;
        self.metrics.record_product_created(&product.id.to_string());
        Ok(product)
    }

    /// Get product by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Product>> {
        self.db.products().get_async(id.into()).await
    }

    /// Get product by slug.
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        self.db.products().get_by_slug_async(slug).await
    }

    /// Update a product.
    pub async fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        self.db.products().update_async(id.into(), input).await
    }

    /// List products.
    pub async fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        self.db.products().list_async(filter).await
    }

    /// Delete a product.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.products().delete_async(id.into()).await
    }

    /// Add a variant to a product.
    pub async fn add_variant(
        &self,
        product_id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        self.db.products().add_variant_public_async(product_id.into(), variant).await
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
        self.db.products().get_variants_async(product_id.into()).await
    }

    /// Count products.
    pub async fn count(&self, filter: ProductFilter) -> Result<u64> {
        self.db.products().count_async(filter).await
    }
}

// ============================================================================
// Async Custom Objects (Custom States / Metaobjects)
// ============================================================================

/// Async custom object operations.
pub struct AsyncCustomObjects {
    db: Arc<PostgresDatabase>,
}

impl AsyncCustomObjects {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    // ------------------------------------------------------------------------
    // Types (schemas)
    // ------------------------------------------------------------------------

    /// Create a new custom object type (schema).
    pub async fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType> {
        self.db.custom_objects().create_type_async(input).await
    }

    /// Get a custom object type by ID.
    pub async fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>> {
        self.db.custom_objects().get_type_async(id).await
    }

    /// Get a custom object type by handle.
    pub async fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>> {
        self.db.custom_objects().get_type_by_handle_async(handle).await
    }

    /// Update a custom object type.
    pub async fn update_type(
        &self,
        id: Uuid,
        input: UpdateCustomObjectType,
    ) -> Result<CustomObjectType> {
        self.db.custom_objects().update_type_async(id, input).await
    }

    /// List custom object types.
    pub async fn list_types(
        &self,
        filter: CustomObjectTypeFilter,
    ) -> Result<Vec<CustomObjectType>> {
        self.db.custom_objects().list_types_async(filter).await
    }

    /// Delete a custom object type.
    pub async fn delete_type(&self, id: Uuid) -> Result<()> {
        self.db.custom_objects().delete_type_async(id).await
    }

    // ------------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------------

    /// Create a new custom object record.
    pub async fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject> {
        self.db.custom_objects().create_object_async(input).await
    }

    /// Get a custom object record by ID.
    pub async fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>> {
        self.db.custom_objects().get_object_async(id).await
    }

    /// Get a custom object record by type handle and object handle.
    pub async fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>> {
        self.db.custom_objects().get_object_by_handle_async(type_handle, object_handle).await
    }

    /// Update a custom object record.
    pub async fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject> {
        self.db.custom_objects().update_object_async(id, input).await
    }

    /// List custom object records.
    pub async fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>> {
        self.db.custom_objects().list_objects_async(filter).await
    }

    /// Delete a custom object record.
    pub async fn delete_object(&self, id: Uuid) -> Result<()> {
        self.db.custom_objects().delete_object_async(id).await
    }
}

// ============================================================================
// Async Returns
// ============================================================================

/// Async return operations.
pub struct AsyncReturns {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncReturns {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new return.
    pub async fn create(&self, input: CreateReturn) -> Result<Return> {
        let ret = self.db.returns().create_async(input).await?;
        self.metrics.record_return_requested(&ret.id.to_string());
        Ok(ret)
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
    metrics: Metrics,
}

impl AsyncShipments {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new shipment.
    pub async fn create(&self, input: CreateShipment) -> Result<Shipment> {
        let shipment = self.db.shipments().create_async(input).await?;
        self.metrics.record_shipment_created(&shipment.id.to_string());
        Ok(shipment)
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
        let shipment = self.db.shipments().mark_delivered_async(id).await?;
        self.metrics.record_shipment_delivered(&shipment.id.to_string());
        Ok(shipment)
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
    pub async fn add_item(
        &self,
        shipment_id: Uuid,
        item: CreateShipmentItem,
    ) -> Result<ShipmentItem> {
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
    pub async fn add_event(
        &self,
        shipment_id: Uuid,
        event: AddShipmentEvent,
    ) -> Result<ShipmentEvent> {
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
    metrics: Metrics,
}

impl AsyncPayments {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
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
        let payment = self.db.payments().mark_completed_async(id).await?;
        self.metrics.record_payment_completed(
            &payment.id.to_string(),
            payment.amount.to_f64().unwrap_or(0.0),
        );
        Ok(payment)
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
    pub async fn set_default_payment_method(
        &self,
        customer_id: Uuid,
        method_id: Uuid,
    ) -> Result<()> {
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new warranty.
    pub async fn create(&self, input: CreateWarranty) -> Result<Warranty> {
        self.db.warranties().create_async(input).await
    }

    /// Get warranty by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Warranty>> {
        self.db.warranties().get_async(id.into()).await
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
        self.db.warranties().update_async(id.into(), input).await
    }

    /// List warranties.
    pub async fn list(&self, filter: WarrantyFilter) -> Result<Vec<Warranty>> {
        self.db.warranties().list_async(filter).await
    }

    /// Get warranties for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_customer_async(customer_id.into()).await
    }

    /// Get warranties for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Warranty>> {
        self.db.warranties().for_order_async(order_id.into()).await
    }

    /// Void a warranty.
    pub async fn void(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().void_async(id.into()).await
    }

    /// Expire a warranty.
    pub async fn expire(&self, id: Uuid) -> Result<Warranty> {
        self.db.warranties().expire_async(id.into()).await
    }

    /// Transfer warranty to new owner.
    pub async fn transfer(&self, id: Uuid, new_customer_id: Uuid) -> Result<Warranty> {
        self.db.warranties().transfer_async(id.into(), new_customer_id.into()).await
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
    pub async fn update_claim(
        &self,
        id: Uuid,
        input: UpdateWarrantyClaim,
    ) -> Result<WarrantyClaim> {
        self.db.warranties().update_claim_async(id, input).await
    }

    /// List claims.
    pub async fn list_claims(&self, filter: WarrantyClaimFilter) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().list_claims_async(filter).await
    }

    /// Get claims for a warranty.
    pub async fn get_claims(&self, warranty_id: Uuid) -> Result<Vec<WarrantyClaim>> {
        self.db.warranties().get_claims_async(warranty_id.into()).await
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
    pub async fn complete_claim(
        &self,
        id: Uuid,
        resolution: ClaimResolution,
    ) -> Result<WarrantyClaim> {
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
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
    pub async fn add_component(
        &self,
        bom_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        self.db.bom().add_component_async(bom_id, component).await
    }

    /// Update a component.
    pub async fn update_component(
        &self,
        component_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
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
    pub async fn add_task(
        &self,
        work_order_id: Uuid,
        task: CreateWorkOrderTask,
    ) -> Result<WorkOrderTask> {
        self.db.work_orders().add_task_async(work_order_id, task).await
    }

    /// Update a task.
    pub async fn update_task(
        &self,
        task_id: Uuid,
        task: UpdateWorkOrderTask,
    ) -> Result<WorkOrderTask> {
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
    pub async fn complete_task(
        &self,
        task_id: Uuid,
        actual_hours: Option<Decimal>,
    ) -> Result<WorkOrderTask> {
        self.db.work_orders().complete_task_async(task_id, actual_hours).await
    }

    /// Add material to work order.
    pub async fn add_material(
        &self,
        work_order_id: Uuid,
        material: AddWorkOrderMaterial,
    ) -> Result<WorkOrderMaterial> {
        self.db.work_orders().add_material_async(work_order_id, material).await
    }

    /// Consume material.
    pub async fn consume_material(
        &self,
        material_id: Uuid,
        quantity: Decimal,
    ) -> Result<WorkOrderMaterial> {
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
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
    pub async fn acknowledge(
        &self,
        id: Uuid,
        supplier_reference: Option<&str>,
    ) -> Result<PurchaseOrder> {
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
    pub async fn receive(
        &self,
        id: Uuid,
        items: ReceivePurchaseOrderItems,
    ) -> Result<PurchaseOrder> {
        self.db.purchase_orders().receive_async(id, items).await
    }

    /// Complete purchase order.
    pub async fn complete(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().complete_async(id).await
    }

    /// Add item to purchase order.
    pub async fn add_item(
        &self,
        po_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().add_item_async(po_id, item).await
    }

    /// Update a PO item.
    pub async fn update_item(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
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
    metrics: Metrics,
}

impl AsyncCarts {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new cart.
    pub async fn create(&self, input: CreateCart) -> Result<Cart> {
        let cart = self.db.carts().create_async(input).await?;
        self.metrics.record_cart_created(&cart.id.to_string());
        Ok(cart)
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

    /// Complete checkout (creates order). The minted order is `Confirmed`
    /// with payment left `Pending`; record the payment through the payments
    /// API, or use [`complete_settled_externally`](Self::complete_settled_externally)
    /// when settlement genuinely happened outside the engine.
    pub async fn complete(&self, id: Uuid) -> Result<CheckoutResult> {
        let result = self.db.carts().complete_async(id).await?;
        self.metrics.record_cart_checkout_completed(
            &result.cart_id.to_string(),
            &result.order_id.to_string(),
        );
        Ok(result)
    }

    /// Complete checkout for a cart settled outside the engine (ACP, external
    /// PSP): explicit opt-in to mint a `Confirmed` + `Paid` order with no
    /// engine-side payment record.
    pub async fn complete_settled_externally(&self, id: Uuid) -> Result<CheckoutResult> {
        let result = self.db.carts().complete_settled_externally_async(id).await?;
        self.metrics.record_cart_checkout_completed(
            &result.cart_id.to_string(),
            &result.order_id.to_string(),
        );
        Ok(result)
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
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
    pub async fn product_performance(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<ProductPerformance>> {
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
    pub async fn inventory_movement(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<InventoryMovement>> {
        self.db.analytics().get_inventory_movement_async(query).await
    }

    /// Get order status breakdown.
    pub async fn order_status_breakdown(
        &self,
        query: AnalyticsQuery,
    ) -> Result<OrderStatusBreakdown> {
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
    pub async fn demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        self.db.analytics().get_demand_forecast_async(skus, days_ahead).await
    }

    /// Get revenue forecast.
    pub async fn revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
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
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
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
    pub async fn update_settings(
        &self,
        settings: StoreCurrencySettings,
    ) -> Result<StoreCurrencySettings> {
        self.db.currency().update_settings_async(settings).await
    }
}

// ============================================================================
// Async Tax
// ============================================================================

/// Async tax operations.
pub struct AsyncTax {
    db: Arc<PostgresDatabase>,
}

impl AsyncTax {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_jurisdiction(
        &self,
        input: CreateTaxJurisdiction,
    ) -> Result<TaxJurisdiction> {
        self.db.tax().create_jurisdiction_async(input).await
    }

    pub async fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        self.db.tax().get_jurisdiction_async(id).await
    }

    pub async fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        self.db.tax().get_jurisdiction_by_code_async(code).await
    }

    pub async fn list_jurisdictions(
        &self,
        filter: TaxJurisdictionFilter,
    ) -> Result<Vec<TaxJurisdiction>> {
        self.db.tax().list_jurisdictions_async(filter).await
    }

    pub async fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        self.db.tax().create_rate_async(input).await
    }

    pub async fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        self.db.tax().get_rate_async(id).await
    }

    pub async fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        self.db.tax().list_rates_async(filter).await
    }

    pub async fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        self.db.tax().get_rates_for_address_async(address, category, date).await
    }

    pub async fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        self.db.tax().create_exemption_async(input).await
    }

    pub async fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        self.db.tax().get_exemption_async(id).await
    }

    pub async fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        self.db.tax().get_customer_exemptions_async(customer_id).await
    }

    pub async fn get_settings(&self) -> Result<TaxSettings> {
        self.db.tax().get_settings_async().await
    }

    pub async fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        self.db.tax().update_settings_async(settings).await
    }

    pub async fn calculate_tax(
        &self,
        request: TaxCalculationRequest,
    ) -> Result<TaxCalculationResult> {
        self.db.tax().calculate_tax_async(request).await
    }

    pub async fn save_calculation(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()> {
        self.db
            .tax()
            .save_calculation_async(result, order_id, cart_id, customer_id, address, currency)
            .await
    }
}

// ============================================================================
// Async Promotions
// ============================================================================

/// Async promotions operations.
pub struct AsyncPromotions {
    db: Arc<PostgresDatabase>,
}

impl AsyncPromotions {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreatePromotion) -> Result<Promotion> {
        self.db.promotions().create_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Promotion>> {
        self.db.promotions().get_async(id.into()).await
    }

    pub async fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        self.db.promotions().get_by_code_async(code).await
    }

    pub async fn list(&self, filter: PromotionFilter) -> Result<Vec<Promotion>> {
        self.db.promotions().list_async(filter).await
    }

    pub async fn update(&self, id: Uuid, input: UpdatePromotion) -> Result<Promotion> {
        self.db.promotions().update_async(id, input).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.promotions().delete_async(id).await
    }

    pub async fn activate(&self, id: Uuid) -> Result<Promotion> {
        self.db.promotions().activate_async(id).await
    }

    pub async fn deactivate(&self, id: Uuid) -> Result<Promotion> {
        self.db.promotions().deactivate_async(id).await
    }

    pub async fn create_coupon(&self, input: CreateCouponCode) -> Result<CouponCode> {
        self.db.promotions().create_coupon_async(input).await
    }

    pub async fn get_coupon(&self, id: Uuid) -> Result<Option<CouponCode>> {
        self.db.promotions().get_coupon_async(id).await
    }

    pub async fn get_coupon_by_code(&self, code: &str) -> Result<Option<CouponCode>> {
        self.db.promotions().get_coupon_by_code_async(code).await
    }

    pub async fn list_coupons(&self, filter: CouponFilter) -> Result<Vec<CouponCode>> {
        self.db.promotions().list_coupons_async(filter).await
    }

    pub async fn apply_promotions(
        &self,
        request: ApplyPromotionsRequest,
    ) -> Result<ApplyPromotionsResult> {
        self.db.promotions().apply_promotions_async(request).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        promotion_id: Uuid,
        coupon_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        discount_amount: Decimal,
        currency: &str,
    ) -> Result<PromotionUsage> {
        self.db
            .promotions()
            .record_usage_async(
                promotion_id.into(),
                coupon_id,
                customer_id.map(Into::into),
                order_id.map(Into::into),
                cart_id.map(Into::into),
                discount_amount,
                currency,
            )
            .await
    }
}

// ============================================================================
// Async Subscriptions
// ============================================================================

/// Async subscriptions operations.
pub struct AsyncSubscriptions {
    db: Arc<PostgresDatabase>,
    metrics: Metrics,
}

impl AsyncSubscriptions {
    pub(crate) const fn new(db: Arc<PostgresDatabase>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    pub async fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        self.db.subscriptions().create_plan_async(input).await
    }

    pub async fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        self.db.subscriptions().get_plan_async(id).await
    }

    pub async fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        self.db.subscriptions().get_plan_by_code_async(code).await
    }

    pub async fn list_plans(
        &self,
        filter: SubscriptionPlanFilter,
    ) -> Result<Vec<SubscriptionPlan>> {
        self.db.subscriptions().list_plans_async(filter).await
    }

    pub async fn update_plan(
        &self,
        id: Uuid,
        input: UpdateSubscriptionPlan,
    ) -> Result<SubscriptionPlan> {
        self.db.subscriptions().update_plan_async(id, input).await
    }

    pub async fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.db.subscriptions().activate_plan_async(id).await
    }

    pub async fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.db.subscriptions().archive_plan_async(id).await
    }

    pub async fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription> {
        let subscription = self.db.subscriptions().create_subscription_async(input).await?;
        self.metrics.record_subscription_created(&subscription.id.to_string());
        Ok(subscription)
    }

    pub async fn get_subscription(&self, id: Uuid) -> Result<Option<Subscription>> {
        self.db.subscriptions().get_subscription_async(id.into()).await
    }

    pub async fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        self.db.subscriptions().get_subscription_by_number_async(number).await
    }

    pub async fn list_subscriptions(
        &self,
        filter: SubscriptionFilter,
    ) -> Result<Vec<Subscription>> {
        self.db.subscriptions().list_subscriptions_async(filter).await
    }

    pub async fn update_subscription(
        &self,
        id: Uuid,
        input: UpdateSubscription,
    ) -> Result<Subscription> {
        self.db.subscriptions().update_subscription_async(id.into(), input).await
    }

    pub async fn cancel_subscription(
        &self,
        id: Uuid,
        input: CancelSubscription,
    ) -> Result<Subscription> {
        self.db.subscriptions().cancel_subscription_async(id.into(), input).await
    }

    pub async fn pause_subscription(
        &self,
        id: Uuid,
        input: PauseSubscription,
    ) -> Result<Subscription> {
        self.db.subscriptions().pause_subscription_async(id.into(), input).await
    }

    pub async fn resume_subscription(&self, id: Uuid) -> Result<Subscription> {
        self.db.subscriptions().resume_subscription_async(id.into()).await
    }

    pub async fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle> {
        self.db.subscriptions().create_billing_cycle_async(input).await
    }

    pub async fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        self.db.subscriptions().get_billing_cycle_async(id).await
    }

    pub async fn list_billing_cycles(
        &self,
        filter: BillingCycleFilter,
    ) -> Result<Vec<BillingCycle>> {
        self.db.subscriptions().list_billing_cycles_async(filter).await
    }

    pub async fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle> {
        self.db.subscriptions().update_billing_cycle_status_async(id, status).await
    }

    pub async fn skip_billing_cycle(
        &self,
        id: Uuid,
        input: SkipBillingCycle,
    ) -> Result<Subscription> {
        self.db.subscriptions().skip_billing_cycle_async(id.into(), input).await
    }

    pub async fn record_event(
        &self,
        subscription_id: Uuid,
        event_type: SubscriptionEventType,
        notes: Option<String>,
    ) -> Result<SubscriptionEvent> {
        let description = notes.unwrap_or_else(|| "Event".to_string());
        self.db
            .subscriptions()
            .record_event_async(subscription_id.into(), event_type, &description, None, None)
            .await
    }

    pub async fn get_subscription_events(
        &self,
        subscription_id: Uuid,
    ) -> Result<Vec<SubscriptionEvent>> {
        self.db.subscriptions().get_subscription_events_async(subscription_id.into()).await
    }
}

// ============================================================================
// Async Quality
// ============================================================================

/// Async quality operations.
pub struct AsyncQuality {
    db: Arc<PostgresDatabase>,
}

impl AsyncQuality {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        self.db.quality().create_inspection_async(input).await
    }

    pub async fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>> {
        self.db.quality().get_inspection_async(id).await
    }

    pub async fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>> {
        self.db.quality().get_inspection_by_number_async(number).await
    }

    pub async fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection> {
        self.db.quality().update_inspection_async(id, input).await
    }

    pub async fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>> {
        self.db.quality().list_inspections_async(filter).await
    }

    pub async fn delete_inspection(&self, id: Uuid) -> Result<()> {
        self.db.quality().delete_inspection_async(id).await
    }

    pub async fn start_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.db.quality().start_inspection_async(id).await
    }

    pub async fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.db.quality().complete_inspection_async(id).await
    }

    pub async fn record_inspection_result(
        &self,
        input: RecordInspectionResult,
    ) -> Result<InspectionItem> {
        self.db.quality().record_inspection_result_async(input).await
    }

    pub async fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>> {
        self.db.quality().get_inspection_items_async(inspection_id).await
    }

    pub async fn count_inspections(&self, filter: InspectionFilter) -> Result<u64> {
        self.db.quality().count_inspections_async(filter).await
    }

    pub async fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance> {
        self.db.quality().create_ncr_async(input).await
    }

    pub async fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>> {
        self.db.quality().get_ncr_async(id).await
    }

    pub async fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>> {
        self.db.quality().get_ncr_by_number_async(number).await
    }

    pub async fn update_ncr(
        &self,
        id: Uuid,
        input: UpdateNonConformance,
    ) -> Result<NonConformance> {
        self.db.quality().update_ncr_async(id, input).await
    }

    pub async fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>> {
        self.db.quality().list_ncrs_async(filter).await
    }

    pub async fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.db.quality().close_ncr_async(id).await
    }

    pub async fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.db.quality().cancel_ncr_async(id).await
    }

    pub async fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64> {
        self.db.quality().count_ncrs_async(filter).await
    }

    pub async fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold> {
        self.db.quality().create_hold_async(input).await
    }

    pub async fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>> {
        self.db.quality().get_hold_async(id).await
    }

    pub async fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
        self.db.quality().list_holds_async(filter).await
    }

    pub async fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold> {
        self.db.quality().release_hold_async(id, input).await
    }

    pub async fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>> {
        self.db.quality().get_active_holds_for_sku_async(sku).await
    }

    pub async fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>> {
        self.db.quality().get_active_holds_for_lot_async(lot_number).await
    }

    pub async fn count_active_holds(&self) -> Result<u64> {
        self.db.quality().count_active_holds_async().await
    }

    pub async fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode> {
        self.db.quality().create_defect_code_async(input).await
    }

    pub async fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>> {
        self.db.quality().get_defect_code_async(code).await
    }

    pub async fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        self.db.quality().list_defect_codes_async(category).await
    }

    pub async fn deactivate_defect_code(&self, id: Uuid) -> Result<()> {
        self.db.quality().deactivate_defect_code_async(id).await
    }
}

// ============================================================================
// Async Lots
// ============================================================================

/// Async lot operations.
pub struct AsyncLots {
    db: Arc<PostgresDatabase>,
}

impl AsyncLots {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateLot) -> Result<Lot> {
        self.db.lots().create_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        self.db.lots().get_async(id).await
    }

    pub async fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        self.db.lots().get_by_number_async(lot_number).await
    }

    pub async fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        self.db.lots().update_async(id, input).await
    }

    pub async fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        self.db.lots().list_async(filter).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.lots().delete_async(id).await
    }

    pub async fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        self.db.lots().adjust_async(input).await
    }

    pub async fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        self.db.lots().consume_async(input).await
    }

    pub async fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        self.db.lots().reserve_async(input).await
    }

    pub async fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.lots().release_reservation_async(reservation_id).await
    }

    pub async fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        self.db.lots().confirm_reservation_async(reservation_id).await
    }

    pub async fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        self.db.lots().transfer_async(input).await
    }

    pub async fn split(&self, input: SplitLot) -> Result<Lot> {
        self.db.lots().split_async(input).await
    }

    pub async fn merge(&self, input: MergeLots) -> Result<Lot> {
        self.db.lots().merge_async(input).await
    }

    pub async fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        self.db.lots().quarantine_async(id, reason).await
    }

    pub async fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        self.db.lots().release_quarantine_async(id).await
    }

    pub async fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        self.db.lots().get_transactions_async(lot_id, limit).await
    }

    pub async fn get_quantity_at_location(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<Decimal>> {
        self.db.lots().get_quantity_at_location_async(lot_id, location_id).await
    }

    pub async fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        self.db.lots().get_lot_locations_async(lot_id).await
    }

    pub async fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        self.db.lots().add_certificate_async(input).await
    }

    pub async fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        self.db.lots().get_certificates_async(lot_id).await
    }

    pub async fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        self.db.lots().delete_certificate_async(certificate_id).await
    }

    pub async fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        self.db.lots().get_expiring_lots_async(days).await
    }

    pub async fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        self.db.lots().get_expired_lots_async().await
    }

    pub async fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        self.db.lots().get_available_lots_for_sku_async(sku).await
    }

    pub async fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        self.db.lots().trace_async(lot_id).await
    }

    pub async fn count(&self, filter: LotFilter) -> Result<u64> {
        self.db.lots().count_async(filter).await
    }

    pub async fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        self.db.lots().create_batch_async(inputs).await
    }

    pub async fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        self.db.lots().get_batch_async(ids).await
    }
}

// ============================================================================
// Async Serials
// ============================================================================

/// Async serial operations.
pub struct AsyncSerials {
    db: Arc<PostgresDatabase>,
}

impl AsyncSerials {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber> {
        self.db.serials().create_async(input).await
    }

    pub async fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>> {
        self.db.serials().create_bulk_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<SerialNumber>> {
        self.db.serials().get_async(id).await
    }

    pub async fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>> {
        self.db.serials().get_by_serial_async(serial).await
    }

    pub async fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber> {
        self.db.serials().update_async(id, input).await
    }

    pub async fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>> {
        self.db.serials().list_async(filter).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.serials().delete_async(id).await
    }

    pub async fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber> {
        self.db.serials().change_status_async(input).await
    }

    pub async fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation> {
        self.db.serials().reserve_async(input).await
    }

    pub async fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.serials().release_reservation_async(reservation_id).await
    }

    pub async fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.serials().confirm_reservation_async(reservation_id).await
    }

    pub async fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber> {
        self.db.serials().move_serial_async(input).await
    }

    pub async fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber> {
        self.db.serials().transfer_ownership_async(input).await
    }

    pub async fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber> {
        self.db.serials().mark_sold_async(id, customer_id, order_id).await
    }

    pub async fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber> {
        self.db.serials().mark_shipped_async(id, shipment_id).await
    }

    pub async fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber> {
        self.db.serials().mark_returned_async(id, return_id).await
    }

    pub async fn activate(&self, id: Uuid) -> Result<SerialNumber> {
        self.db.serials().activate_async(id).await
    }

    pub async fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.db.serials().quarantine_async(id, reason).await
    }

    pub async fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber> {
        self.db.serials().release_quarantine_async(id).await
    }

    pub async fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.db.serials().scrap_async(id, reason).await
    }

    pub async fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>> {
        self.db.serials().get_history_async(serial_id, filter).await
    }

    pub async fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>> {
        self.db.serials().lookup_async(serial).await
    }

    pub async fn validate(&self, serial: &str) -> Result<SerialValidation> {
        self.db.serials().validate_async(serial).await
    }

    pub async fn get_available_for_sku(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_available_for_sku_async(sku, limit).await
    }

    pub async fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_for_lot_async(lot_id).await
    }

    pub async fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_for_customer_async(customer_id).await
    }

    pub async fn count(&self, filter: SerialFilter) -> Result<u64> {
        self.db.serials().count_async(filter).await
    }

    pub async fn create_batch(
        &self,
        inputs: Vec<CreateSerialNumber>,
    ) -> Result<BatchResult<SerialNumber>> {
        self.db.serials().create_batch_async(inputs).await
    }

    pub async fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_batch_async(ids).await
    }
}

// ============================================================================
// Async Warehouse
// ============================================================================

/// Async warehouse operations.
pub struct AsyncWarehouse {
    db: Arc<PostgresDatabase>,
}

impl AsyncWarehouse {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse> {
        self.db.warehouse().create_warehouse_async(input).await
    }

    pub async fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>> {
        self.db.warehouse().get_warehouse_async(id).await
    }

    pub async fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        self.db.warehouse().get_warehouse_by_code_async(code).await
    }

    pub async fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse> {
        self.db.warehouse().update_warehouse_async(id, input).await
    }

    pub async fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        self.db.warehouse().list_warehouses_async(filter).await
    }

    pub async fn delete_warehouse(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_warehouse_async(id).await
    }

    pub async fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64> {
        self.db.warehouse().count_warehouses_async(filter).await
    }

    pub async fn create_zone(&self, input: CreateZone) -> Result<Zone> {
        self.db.warehouse().create_zone_async(input).await
    }

    pub async fn get_zone(&self, id: i32) -> Result<Option<Zone>> {
        self.db.warehouse().get_zone_async(id).await
    }

    pub async fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        self.db.warehouse().get_zones_async(warehouse_id).await
    }

    pub async fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        self.db.warehouse().update_zone_async(id, input).await
    }

    pub async fn delete_zone(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_zone_async(id).await
    }

    pub async fn create_location(&self, input: CreateLocation) -> Result<Location> {
        self.db.warehouse().create_location_async(input).await
    }

    pub async fn get_location(&self, id: i32) -> Result<Option<Location>> {
        self.db.warehouse().get_location_async(id).await
    }

    pub async fn get_location_by_code(
        &self,
        warehouse_id: i32,
        code: &str,
    ) -> Result<Option<Location>> {
        self.db.warehouse().get_location_by_code_async(warehouse_id, code).await
    }

    pub async fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        self.db.warehouse().update_location_async(id, input).await
    }

    pub async fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        self.db.warehouse().list_locations_async(filter).await
    }

    pub async fn delete_location(&self, id: i32) -> Result<()> {
        self.db.warehouse().delete_location_async(id).await
    }

    pub async fn count_locations(&self, filter: LocationFilter) -> Result<u64> {
        self.db.warehouse().count_locations_async(filter).await
    }

    pub async fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.db.warehouse().get_locations_for_warehouse_async(warehouse_id).await
    }

    pub async fn get_pickable_locations(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<Location>> {
        self.db.warehouse().get_pickable_locations_async(warehouse_id, sku).await
    }

    pub async fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.db.warehouse().get_receivable_locations_async(warehouse_id).await
    }

    pub async fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().get_location_inventory_async(location_id).await
    }

    pub async fn get_inventory_for_sku(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().get_inventory_for_sku_async(warehouse_id, sku).await
    }

    pub async fn adjust_inventory(
        &self,
        input: AdjustLocationInventory,
    ) -> Result<LocationInventory> {
        self.db.warehouse().adjust_inventory_async(input).await
    }

    pub async fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement> {
        self.db.warehouse().move_inventory_async(input).await
    }

    pub async fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        self.db.warehouse().list_location_inventory_async(filter).await
    }

    pub async fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>> {
        self.db.warehouse().get_movements_async(filter).await
    }

    pub async fn count_movements(&self, filter: MovementFilter) -> Result<u64> {
        self.db.warehouse().count_movements_async(filter).await
    }

    pub async fn create_locations_batch(
        &self,
        inputs: Vec<CreateLocation>,
    ) -> Result<BatchResult<Location>> {
        self.db.warehouse().create_locations_batch_async(inputs).await
    }

    pub async fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        self.db.warehouse().get_locations_batch_async(ids).await
    }

    /// Create a cycle count (draft) with its expected lines.
    pub async fn create_cycle_count(&self, input: CreateCycleCount) -> Result<CycleCount> {
        self.db.warehouse().create_cycle_count_async(input).await
    }

    /// Get a cycle count (with lines) by ID.
    pub async fn get_cycle_count(&self, id: Uuid) -> Result<Option<CycleCount>> {
        self.db.warehouse().get_cycle_count_async(id).await
    }

    /// List cycle counts matching the filter.
    pub async fn list_cycle_counts(&self, filter: CycleCountFilter) -> Result<Vec<CycleCount>> {
        self.db.warehouse().list_cycle_counts_async(filter).await
    }

    /// Start a draft cycle count (`draft` → `in_progress`).
    pub async fn start_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.db.warehouse().start_cycle_count_async(id).await
    }

    /// Record physical counts against an in-progress cycle count.
    pub async fn record_cycle_counts(
        &self,
        id: Uuid,
        counts: Vec<RecordCycleCountLine>,
    ) -> Result<CycleCount> {
        self.db.warehouse().record_cycle_counts_async(id, counts).await
    }

    /// Complete an in-progress cycle count, applying variance adjustments
    /// to `location_inventory`.
    pub async fn complete_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.db.warehouse().complete_cycle_count_async(id).await
    }

    /// Cancel a draft or in-progress cycle count. No adjustments are applied.
    pub async fn cancel_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.db.warehouse().cancel_cycle_count_async(id).await
    }
}

// ============================================================================
// Async Receiving
// ============================================================================

/// Async receiving operations.
pub struct AsyncReceiving {
    db: Arc<PostgresDatabase>,
}

impl AsyncReceiving {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        self.db.receiving().create_receipt_async(input).await
    }

    pub async fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        self.db.receiving().get_receipt_async(id).await
    }

    pub async fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>> {
        self.db.receiving().get_receipt_by_number_async(number).await
    }

    pub async fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        self.db.receiving().update_receipt_async(id, input).await
    }

    pub async fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        self.db.receiving().list_receipts_async(filter).await
    }

    pub async fn delete_receipt(&self, id: Uuid) -> Result<()> {
        self.db.receiving().delete_receipt_async(id).await
    }

    pub async fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().start_receiving_async(id).await
    }

    pub async fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        self.db.receiving().receive_items_async(input).await
    }

    pub async fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().complete_receiving_async(id).await
    }

    pub async fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().cancel_receipt_async(id).await
    }

    pub async fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        self.db.receiving().get_receipt_items_async(receipt_id).await
    }

    pub async fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        self.db.receiving().count_receipts_async(filter).await
    }

    pub async fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway> {
        self.db.receiving().create_put_away_async(input).await
    }

    pub async fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>> {
        self.db.receiving().get_put_away_async(id).await
    }

    pub async fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        self.db.receiving().list_put_aways_async(filter).await
    }

    pub async fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        self.db.receiving().assign_put_away_async(id, assigned_to).await
    }

    pub async fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.db.receiving().start_put_away_async(id).await
    }

    pub async fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        self.db.receiving().complete_put_away_async(input).await
    }

    pub async fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.db.receiving().cancel_put_away_async(id).await
    }

    pub async fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.db.receiving().get_pending_put_aways_async(receipt_id).await
    }

    pub async fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64> {
        self.db.receiving().count_put_aways_async(filter).await
    }

    pub async fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        self.db.receiving().create_receipt_from_po_async(po_id, warehouse_id).await
    }

    pub async fn create_receipts_batch(
        &self,
        inputs: Vec<CreateReceipt>,
    ) -> Result<BatchResult<Receipt>> {
        self.db.receiving().create_receipts_batch_async(inputs).await
    }

    pub async fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        self.db.receiving().get_receipts_batch_async(ids).await
    }
}

// ============================================================================
// Async Fulfillment
// ============================================================================

/// Async fulfillment operations.
pub struct AsyncFulfillment {
    db: Arc<PostgresDatabase>,
}

impl AsyncFulfillment {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        self.db.fulfillment().create_wave_async(input).await
    }

    pub async fn get_wave(&self, id: Uuid) -> Result<Option<Wave>> {
        self.db.fulfillment().get_wave_async(id).await
    }

    pub async fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        self.db.fulfillment().get_wave_by_number_async(number).await
    }

    pub async fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        self.db.fulfillment().list_waves_async(filter).await
    }

    pub async fn release_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().release_wave_async(id).await
    }

    pub async fn complete_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().complete_wave_async(id).await
    }

    pub async fn cancel_wave(&self, id: Uuid) -> Result<Wave> {
        self.db.fulfillment().cancel_wave_async(id).await
    }

    pub async fn get_wave_orders(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        self.db.fulfillment().get_wave_orders_async(wave_id).await
    }

    pub async fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        self.db.fulfillment().count_waves_async(filter).await
    }

    pub async fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        self.db.fulfillment().create_pick_async(input).await
    }

    pub async fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>> {
        self.db.fulfillment().get_pick_async(id).await
    }

    pub async fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        self.db.fulfillment().list_picks_async(filter).await
    }

    pub async fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        self.db.fulfillment().assign_pick_async(id, assigned_to).await
    }

    pub async fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        self.db.fulfillment().start_pick_async(id).await
    }

    pub async fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        self.db.fulfillment().complete_pick_async(input).await
    }

    pub async fn report_short(
        &self,
        id: Uuid,
        short_qty: Decimal,
        reason: &str,
    ) -> Result<PickTask> {
        self.db.fulfillment().report_short_async(id, short_qty, reason).await
    }

    pub async fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        self.db.fulfillment().cancel_pick_async(id).await
    }

    pub async fn get_picks_for_order(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_for_order_async(order_id).await
    }

    pub async fn get_picks_for_wave(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_for_wave_async(wave_id).await
    }

    pub async fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_picks_async(filter).await
    }

    pub async fn create_pack(&self, input: CreatePackTask) -> Result<PackTask> {
        self.db.fulfillment().create_pack_async(input).await
    }

    pub async fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>> {
        self.db.fulfillment().get_pack_async(id).await
    }

    pub async fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        self.db.fulfillment().list_packs_async(filter).await
    }

    pub async fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        self.db.fulfillment().assign_pack_async(id, assigned_to).await
    }

    pub async fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().start_pack_async(id).await
    }

    pub async fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().complete_pack_async(id).await
    }

    pub async fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        self.db.fulfillment().add_carton_async(input).await
    }

    pub async fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        self.db.fulfillment().add_carton_item_async(input).await
    }

    pub async fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        self.db.fulfillment().get_cartons_async(pack_task_id).await
    }

    pub async fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        self.db.fulfillment().get_carton_items_async(carton_id).await
    }

    pub async fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton> {
        self.db.fulfillment().mark_label_printed_async(carton_id).await
    }

    pub async fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        self.db.fulfillment().cancel_pack_async(id).await
    }

    pub async fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_packs_async(filter).await
    }

    pub async fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask> {
        self.db.fulfillment().create_ship_async(input).await
    }

    pub async fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>> {
        self.db.fulfillment().get_ship_async(id).await
    }

    pub async fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        self.db.fulfillment().list_ships_async(filter).await
    }

    pub async fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        self.db.fulfillment().assign_ship_async(id, assigned_to).await
    }

    pub async fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        self.db.fulfillment().print_label_async(id, label_url).await
    }

    pub async fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        self.db.fulfillment().complete_ship_async(input).await
    }

    pub async fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        self.db.fulfillment().cancel_ship_async(id).await
    }

    pub async fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        self.db.fulfillment().count_ships_async(filter).await
    }

    pub async fn create_picks_for_order(
        &self,
        order_id: Uuid,
        warehouse_id: i32,
    ) -> Result<Vec<PickTask>> {
        self.db.fulfillment().create_picks_for_order_async(order_id, warehouse_id).await
    }

    pub async fn is_order_ready_to_pack(&self, order_id: Uuid) -> Result<bool> {
        self.db.fulfillment().is_order_ready_to_pack_async(order_id).await
    }

    pub async fn is_order_ready_to_ship(&self, order_id: Uuid) -> Result<bool> {
        self.db.fulfillment().is_order_ready_to_ship_async(order_id).await
    }

    pub async fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        self.db.fulfillment().create_waves_batch_async(inputs).await
    }

    pub async fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        self.db.fulfillment().get_picks_batch_async(ids).await
    }
}

// ============================================================================
// Async Accounts Payable
// ============================================================================

/// Async accounts payable operations.
pub struct AsyncAccountsPayable {
    db: Arc<PostgresDatabase>,
}

impl AsyncAccountsPayable {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_bill(&self, input: CreateBill) -> Result<Bill> {
        self.db.accounts_payable().create_bill_async(input).await
    }

    pub async fn get_bill(&self, id: Uuid) -> Result<Option<Bill>> {
        self.db.accounts_payable().get_bill_async(id).await
    }

    pub async fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>> {
        self.db.accounts_payable().get_bill_by_number_async(number).await
    }

    pub async fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        self.db.accounts_payable().update_bill_async(id, input).await
    }

    pub async fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        self.db.accounts_payable().list_bills_async(filter).await
    }

    pub async fn delete_bill(&self, id: Uuid) -> Result<()> {
        self.db.accounts_payable().delete_bill_async(id).await
    }

    pub async fn approve_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().approve_bill_async(id).await
    }

    pub async fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().cancel_bill_async(id).await
    }

    pub async fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().dispute_bill_async(id).await
    }

    pub async fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        self.db.accounts_payable().get_bill_items_async(bill_id).await
    }

    pub async fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem> {
        self.db.accounts_payable().add_bill_item_async(bill_id, item).await
    }

    pub async fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        self.db.accounts_payable().remove_bill_item_async(item_id).await
    }

    pub async fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        self.db.accounts_payable().count_bills_async(filter).await
    }

    pub async fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_overdue_bills_async().await
    }

    pub async fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_bills_due_soon_async(days).await
    }

    pub async fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment> {
        self.db.accounts_payable().create_payment_async(input).await
    }

    pub async fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>> {
        self.db.accounts_payable().get_payment_async(id).await
    }

    pub async fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>> {
        self.db.accounts_payable().get_payment_by_number_async(number).await
    }

    pub async fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        self.db.accounts_payable().list_payments_async(filter).await
    }

    pub async fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.db.accounts_payable().void_payment_async(id).await
    }

    pub async fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.db.accounts_payable().clear_payment_async(id).await
    }

    pub async fn get_payment_allocations(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<PaymentAllocation>> {
        self.db.accounts_payable().get_payment_allocations_async(payment_id).await
    }

    pub async fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        self.db.accounts_payable().get_payments_for_bill_async(bill_id).await
    }

    pub async fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64> {
        self.db.accounts_payable().count_payments_async(filter).await
    }

    pub async fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        self.db.accounts_payable().create_payment_run_async(input).await
    }

    pub async fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        self.db.accounts_payable().get_payment_run_async(id).await
    }

    pub async fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        self.db.accounts_payable().list_payment_runs_async(filter).await
    }

    pub async fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        self.db.accounts_payable().approve_payment_run_async(id, approved_by).await
    }

    pub async fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.db.accounts_payable().process_payment_run_async(id).await
    }

    pub async fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.db.accounts_payable().cancel_payment_run_async(id).await
    }

    pub async fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_payment_run_bills_async(run_id).await
    }

    pub async fn get_aging_summary(&self) -> Result<ApAgingSummary> {
        self.db.accounts_payable().get_aging_summary_async().await
    }

    pub async fn get_supplier_summary(
        &self,
        supplier_id: Uuid,
    ) -> Result<Option<SupplierApSummary>> {
        self.db.accounts_payable().get_supplier_summary_async(supplier_id).await
    }

    pub async fn get_total_outstanding(&self) -> Result<Decimal> {
        self.db.accounts_payable().get_total_outstanding_async().await
    }

    pub async fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>> {
        self.db.accounts_payable().create_bills_batch_async(inputs).await
    }

    pub async fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_bills_batch_async(ids).await
    }
}

// ============================================================================
// Async Cost Accounting
// ============================================================================

/// Async cost accounting operations.
pub struct AsyncCostAccounting {
    db: Arc<PostgresDatabase>,
}

impl AsyncCostAccounting {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>> {
        self.db.cost_accounting().get_item_cost_async(sku).await
    }

    pub async fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost> {
        self.db.cost_accounting().set_item_cost_async(input).await
    }

    pub async fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        self.db.cost_accounting().list_item_costs_async(filter).await
    }

    pub async fn update_average_cost(
        &self,
        sku: &str,
        quantity: Decimal,
        unit_cost: Decimal,
    ) -> Result<ItemCost> {
        self.db.cost_accounting().update_average_cost_async(sku, quantity, unit_cost).await
    }

    pub async fn update_last_cost(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        self.db.cost_accounting().update_last_cost_async(sku, unit_cost).await
    }

    pub async fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        self.db.cost_accounting().create_cost_layer_async(input).await
    }

    pub async fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>> {
        self.db.cost_accounting().get_cost_layer_async(id).await
    }

    pub async fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        self.db.cost_accounting().list_cost_layers_async(filter).await
    }

    pub async fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().issue_fifo_async(input).await
    }

    pub async fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().issue_lifo_async(input).await
    }

    pub async fn get_layers_remaining(&self, sku: &str) -> Result<Decimal> {
        self.db.cost_accounting().get_layers_remaining_async(sku).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_cost_transaction(
        &self,
        sku: &str,
        transaction_type: stateset_core::CostTransactionType,
        quantity: Decimal,
        unit_cost: Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction> {
        self.db
            .cost_accounting()
            .record_cost_transaction_async(
                sku,
                transaction_type,
                quantity,
                unit_cost,
                layer_id,
                reference_type,
                reference_id,
                notes,
            )
            .await
    }

    pub async fn list_cost_transactions(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().list_cost_transactions_async(filter).await
    }

    pub async fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance> {
        self.db.cost_accounting().record_variance_async(input).await
    }

    pub async fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>> {
        self.db.cost_accounting().list_variances_async(filter).await
    }

    pub async fn get_variance_summary(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Decimal> {
        self.db.cost_accounting().get_variance_summary_async(from, to).await
    }

    pub async fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        self.db.cost_accounting().create_adjustment_async(input).await
    }

    pub async fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        self.db.cost_accounting().get_adjustment_async(id).await
    }

    pub async fn list_adjustments(
        &self,
        filter: CostAdjustmentFilter,
    ) -> Result<Vec<CostAdjustment>> {
        self.db.cost_accounting().list_adjustments_async(filter).await
    }

    pub async fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment> {
        self.db.cost_accounting().approve_adjustment_async(id, approved_by).await
    }

    pub async fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.db.cost_accounting().apply_adjustment_async(id).await
    }

    pub async fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.db.cost_accounting().reject_adjustment_async(id).await
    }

    pub async fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        self.db.cost_accounting().calculate_rollup_async(sku, bom_id).await
    }

    pub async fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>> {
        self.db.cost_accounting().get_rollup_async(sku).await
    }

    pub async fn get_inventory_valuation(
        &self,
        cost_method: CostMethod,
    ) -> Result<InventoryValuation> {
        self.db.cost_accounting().get_inventory_valuation_async(cost_method).await
    }

    pub async fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        self.db.cost_accounting().get_sku_cost_summary_async(sku).await
    }

    pub async fn get_total_inventory_value(&self) -> Result<Decimal> {
        self.db.cost_accounting().get_total_inventory_value_async().await
    }
}

// ============================================================================
// Async Credit
// ============================================================================

/// Async credit operations.
pub struct AsyncCredit {
    db: Arc<PostgresDatabase>,
}

impl AsyncCredit {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount> {
        self.db.credit().create_credit_account_async(input).await
    }

    pub async fn get_credit_account(&self, id: Uuid) -> Result<Option<CreditAccount>> {
        self.db.credit().get_credit_account_async(id).await
    }

    pub async fn get_credit_account_by_customer(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CreditAccount>> {
        self.db.credit().get_credit_account_by_customer_async(customer_id).await
    }

    pub async fn update_credit_account(
        &self,
        id: Uuid,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount> {
        self.db.credit().update_credit_account_async(id, input).await
    }

    pub async fn list_credit_accounts(
        &self,
        filter: CreditAccountFilter,
    ) -> Result<Vec<CreditAccount>> {
        self.db.credit().list_credit_accounts_async(filter).await
    }

    pub async fn adjust_credit_limit(
        &self,
        customer_id: Uuid,
        new_limit: Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.db.credit().adjust_credit_limit_async(customer_id, new_limit, reason).await
    }

    pub async fn suspend_credit_account(
        &self,
        customer_id: Uuid,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.db.credit().suspend_credit_account_async(customer_id, reason).await
    }

    pub async fn reactivate_credit_account(&self, customer_id: Uuid) -> Result<CreditAccount> {
        self.db.credit().reactivate_credit_account_async(customer_id).await
    }

    pub async fn check_credit(
        &self,
        customer_id: Uuid,
        order_amount: Decimal,
    ) -> Result<CreditCheckResult> {
        self.db.credit().check_credit_async(customer_id, order_amount).await
    }

    pub async fn reserve_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        self.db.credit().reserve_credit_async(customer_id, order_id, amount).await
    }

    pub async fn release_credit_reservation(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
    ) -> Result<CreditAccount> {
        self.db.credit().release_credit_reservation_async(customer_id, order_id).await
    }

    pub async fn charge_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        self.db.credit().charge_credit_async(customer_id, order_id, amount).await
    }

    pub async fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        self.db.credit().place_hold_async(input).await
    }

    pub async fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>> {
        self.db.credit().get_hold_async(id).await
    }

    pub async fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        self.db.credit().list_holds_async(filter).await
    }

    pub async fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        self.db.credit().release_hold_async(input).await
    }

    pub async fn get_active_holds(&self, customer_id: Uuid) -> Result<Vec<CreditHold>> {
        self.db.credit().get_active_holds_async(customer_id).await
    }

    pub async fn get_holds_for_order(&self, order_id: Uuid) -> Result<Vec<CreditHold>> {
        self.db.credit().get_holds_for_order_async(order_id).await
    }

    pub async fn submit_application(
        &self,
        input: SubmitCreditApplication,
    ) -> Result<CreditApplication> {
        self.db.credit().submit_application_async(input).await
    }

    pub async fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        self.db.credit().get_application_async(id).await
    }

    pub async fn list_applications(
        &self,
        filter: CreditApplicationFilter,
    ) -> Result<Vec<CreditApplication>> {
        self.db.credit().list_applications_async(filter).await
    }

    pub async fn review_application(
        &self,
        input: ReviewCreditApplication,
    ) -> Result<CreditApplication> {
        self.db.credit().review_application_async(input).await
    }

    pub async fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication> {
        self.db.credit().withdraw_application_async(id).await
    }

    pub async fn record_transaction(
        &self,
        input: RecordCreditTransaction,
    ) -> Result<CreditTransaction> {
        self.db.credit().record_transaction_async(input).await
    }

    pub async fn list_transactions(
        &self,
        filter: CreditTransactionFilter,
    ) -> Result<Vec<CreditTransaction>> {
        self.db.credit().list_transactions_async(filter).await
    }

    pub async fn apply_payment(
        &self,
        customer_id: Uuid,
        amount: Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        self.db.credit().apply_payment_async(customer_id, amount, reference_id).await
    }

    pub async fn get_customer_summary(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerCreditSummary>> {
        self.db.credit().get_customer_summary_async(customer_id).await
    }

    pub async fn get_aging_report(&self) -> Result<Vec<(Uuid, CreditAgingBucket)>> {
        Ok(self
            .db
            .credit()
            .get_aging_report_async()
            .await?
            .into_iter()
            .map(|(customer_id, bucket)| (customer_id.into_uuid(), bucket))
            .collect())
    }

    pub async fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>> {
        self.db.credit().get_over_limit_customers_async().await
    }
}

// ============================================================================
// Async Backorder
// ============================================================================

/// Async backorder operations.
pub struct AsyncBackorder {
    db: Arc<PostgresDatabase>,
}

impl AsyncBackorder {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        self.db.backorder().create_backorder_async(input).await
    }

    pub async fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        self.db.backorder().get_backorder_async(id).await
    }

    pub async fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        self.db.backorder().get_backorder_by_number_async(number).await
    }

    pub async fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        self.db.backorder().update_backorder_async(id, input).await
    }

    pub async fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        self.db.backorder().list_backorders_async(filter).await
    }

    pub async fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        self.db.backorder().cancel_backorder_async(id).await
    }

    pub async fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_order_async(order_id).await
    }

    pub async fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_customer_async(customer_id).await
    }

    pub async fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_sku_async(sku).await
    }

    pub async fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        self.db.backorder().fulfill_backorder_async(input).await
    }

    pub async fn get_fulfillment_history(
        &self,
        backorder_id: Uuid,
    ) -> Result<Vec<BackorderFulfillment>> {
        self.db.backorder().get_fulfillment_history_async(backorder_id).await
    }

    pub async fn allocate_backorder(
        &self,
        input: AllocateBackorder,
    ) -> Result<BackorderAllocation> {
        self.db.backorder().allocate_backorder_async(input).await
    }

    pub async fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        self.db.backorder().get_allocations_async(backorder_id).await
    }

    pub async fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.db.backorder().release_allocation_async(allocation_id).await
    }

    pub async fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.db.backorder().confirm_allocation_async(allocation_id).await
    }

    pub async fn expire_allocations(&self) -> Result<u32> {
        self.db.backorder().expire_allocations_async().await
    }

    pub async fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        self.db.backorder().auto_allocate_inventory_async(sku).await
    }

    pub async fn get_summary(&self) -> Result<BackorderSummary> {
        self.db.backorder().get_summary_async().await
    }

    pub async fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        self.db.backorder().get_sku_summary_async(sku).await
    }

    pub async fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        self.db.backorder().get_overdue_backorders_async().await
    }

    pub async fn count_pending(&self) -> Result<u64> {
        self.db.backorder().count_pending_async().await
    }
}

// ============================================================================
// Async Accounts Receivable
// ============================================================================

/// Async accounts receivable operations.
pub struct AsyncAccountsReceivable {
    db: Arc<PostgresDatabase>,
}

impl AsyncAccountsReceivable {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        self.db.accounts_receivable().get_aging_summary_async().await
    }

    pub async fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>> {
        self.db.accounts_receivable().get_customer_aging_async(customer_id).await
    }

    pub async fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        self.db.accounts_receivable().get_aging_report_async(filter).await
    }

    pub async fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        self.db.accounts_receivable().log_collection_activity_async(input).await
    }

    pub async fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        self.db.accounts_receivable().list_collection_activities_async(filter).await
    }

    pub async fn update_collection_status(
        &self,
        invoice_id: Uuid,
        status: CollectionStatus,
    ) -> Result<()> {
        self.db.accounts_receivable().update_collection_status_async(invoice_id, status).await
    }

    pub async fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        self.db.accounts_receivable().get_invoices_due_for_dunning_async().await
    }

    pub async fn send_dunning_letter(
        &self,
        invoice_id: Uuid,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        self.db
            .accounts_receivable()
            .send_dunning_letter_async(invoice_id, letter_type, sent_by)
            .await
    }

    pub async fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        self.db.accounts_receivable().create_write_off_async(input).await
    }

    pub async fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>> {
        self.db.accounts_receivable().get_write_off_async(id).await
    }

    pub async fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        self.db.accounts_receivable().list_write_offs_async(filter).await
    }

    pub async fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        self.db.accounts_receivable().reverse_write_off_async(id).await
    }

    pub async fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        self.db.accounts_receivable().create_credit_memo_async(input).await
    }

    pub async fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        self.db.accounts_receivable().get_credit_memo_async(id).await
    }

    pub async fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>> {
        self.db.accounts_receivable().get_credit_memo_by_number_async(number).await
    }

    pub async fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>> {
        self.db.accounts_receivable().list_credit_memos_async(filter).await
    }

    pub async fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        self.db.accounts_receivable().apply_credit_memo_async(input).await
    }

    pub async fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        self.db.accounts_receivable().void_credit_memo_async(id).await
    }

    pub async fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.db.accounts_receivable().get_unapplied_credits_async(customer_id).await
    }

    pub async fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        self.db.accounts_receivable().apply_payment_to_invoices_async(input).await
    }

    pub async fn get_payment_applications(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<ArPaymentApplication>> {
        self.db.accounts_receivable().get_payment_applications_async(payment_id).await
    }

    pub async fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        self.db.accounts_receivable().unapply_payment_async(application_id).await
    }

    pub async fn get_customer_summary(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerArSummary>> {
        self.db.accounts_receivable().get_customer_summary_async(customer_id).await
    }

    pub async fn generate_statement(
        &self,
        request: GenerateStatementRequest,
    ) -> Result<CustomerStatement> {
        self.db.accounts_receivable().generate_statement_async(request).await
    }

    pub async fn get_total_outstanding(&self) -> Result<Decimal> {
        self.db.accounts_receivable().get_total_outstanding_async().await
    }

    pub async fn get_dso(&self, days: i32) -> Result<Decimal> {
        self.db.accounts_receivable().get_dso_async(days).await
    }

    pub async fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        self.db.accounts_receivable().get_average_days_to_pay_async(customer_id).await
    }

    pub async fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>> {
        self.db.accounts_receivable().get_customers_batch_async(ids).await
    }
}

// ============================================================================
// Async General Ledger
// ============================================================================

/// Async general ledger operations.
pub struct AsyncGeneralLedger {
    db: Arc<PostgresDatabase>,
}

impl AsyncGeneralLedger {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        self.db.general_ledger().create_account_async(input).await
    }

    pub async fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        self.db.general_ledger().get_account_async(id).await
    }

    pub async fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>> {
        self.db.general_ledger().get_account_by_number_async(account_number).await
    }

    pub async fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount> {
        self.db.general_ledger().update_account_async(id, input).await
    }

    pub async fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().list_accounts_async(filter).await
    }

    pub async fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_account_hierarchy_async().await
    }

    pub async fn delete_account(&self, id: Uuid) -> Result<()> {
        self.db.general_ledger().delete_account_async(id).await
    }

    pub async fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().initialize_chart_of_accounts_async().await
    }

    pub async fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        self.db.general_ledger().create_period_async(input).await
    }

    pub async fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_period_async(id).await
    }

    pub async fn get_current_period(&self) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_current_period_async().await
    }

    pub async fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_period_for_date_async(date).await
    }

    pub async fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        self.db.general_ledger().list_periods_async(filter).await
    }

    pub async fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.db.general_ledger().open_period_async(id).await
    }

    pub async fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        self.db.general_ledger().close_period_async(id, closed_by).await
    }

    pub async fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        self.db.general_ledger().lock_period_async(id, locked_by).await
    }

    pub async fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.db.general_ledger().reopen_period_async(id).await
    }

    pub async fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        self.db.general_ledger().create_journal_entry_async(input).await
    }

    pub async fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        self.db.general_ledger().get_journal_entry_async(id).await
    }

    pub async fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        self.db.general_ledger().get_journal_entry_by_number_async(number).await
    }

    pub async fn list_journal_entries(
        &self,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntry>> {
        self.db.general_ledger().list_journal_entries_async(filter).await
    }

    pub async fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        self.db.general_ledger().post_journal_entry_async(id, posted_by).await
    }

    pub async fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().void_journal_entry_async(id).await
    }

    pub async fn reverse_journal_entry(
        &self,
        id: Uuid,
        reversal_date: NaiveDate,
    ) -> Result<JournalEntry> {
        self.db.general_ledger().reverse_journal_entry_async(id, reversal_date).await
    }

    pub async fn get_journal_entry_lines(
        &self,
        journal_entry_id: Uuid,
    ) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_journal_entry_lines_async(journal_entry_id).await
    }

    pub async fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>> {
        self.db.general_ledger().get_auto_posting_config_async().await
    }

    pub async fn set_auto_posting_config(
        &self,
        input: CreateAutoPostingConfig,
    ) -> Result<AutoPostingConfig> {
        self.db.general_ledger().set_auto_posting_config_async(input).await
    }

    pub async fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_invoice_async(invoice_id).await
    }

    pub async fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_payment_received_async(payment_id).await
    }

    pub async fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_bill_async(bill_id).await
    }

    pub async fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_bill_payment_async(payment_id).await
    }

    pub async fn auto_post_inventory_cost(
        &self,
        cost_transaction_id: Uuid,
    ) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_inventory_cost_async(cost_transaction_id).await
    }

    pub async fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_write_off_async(write_off_id).await
    }

    pub async fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        self.db.general_ledger().get_trial_balance_async(as_of_date).await
    }

    pub async fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        self.db.general_ledger().get_balance_sheet_async(as_of_date).await
    }

    pub async fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        self.db.general_ledger().get_income_statement_async(start_date, end_date).await
    }

    pub async fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<Decimal>> {
        self.db.general_ledger().get_account_balance_async(account_id, as_of_date).await
    }

    pub async fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_account_transactions_async(account_id, filter).await
    }

    pub async fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        self.db.general_ledger().run_period_close_async(period_id, closed_by).await
    }

    pub async fn create_accounts_batch(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
        self.db.general_ledger().create_accounts_batch_async(inputs).await
    }

    pub async fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_accounts_batch_async(ids).await
    }
}

// ============================================================================
// Async X402
// ============================================================================

/// Async x402 and A2A operations.
pub struct AsyncX402 {
    db: Arc<PostgresDatabase>,
}

impl AsyncX402 {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Payment Intent Operations
    // ========================================================================

    /// Create a new x402 payment intent.
    pub async fn create_intent(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().create_async(input).await
    }

    /// Get a payment intent by ID.
    pub async fn get_intent(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        self.db.x402_payment_intents().get_async(id).await
    }

    /// Sign a payment intent with its configured signature scheme.
    pub async fn sign_intent(
        &self,
        id: Uuid,
        input: SignX402PaymentIntent,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().sign_async(id, input).await
    }

    /// Mark an intent as sequenced in a settlement batch.
    pub async fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_sequenced_async(id, sequence_number, batch_id).await
    }

    /// Mark an intent as settled on-chain.
    pub async fn mark_settled(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_settled_async(id, tx_hash, block_number).await
    }

    /// Mark an intent as failed.
    pub async fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_failed_async(id, reason).await
    }

    /// Mark an intent as expired.
    pub async fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_expired_async(id).await
    }

    /// Cancel a payment intent.
    pub async fn cancel_intent(&self, id: Uuid) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().cancel_async(id).await
    }

    /// Get all payment intents for a cart.
    pub async fn intents_for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().for_cart_async(cart_id).await
    }

    /// Get all payment intents for an order.
    pub async fn intents_for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().for_order_async(order_id).await
    }

    /// Get the next nonce for a payer address.
    pub async fn get_next_nonce(&self, payer_address: &str) -> Result<u64> {
        self.db.x402_payment_intents().get_next_nonce_async(payer_address).await
    }

    /// List payment intents with optional filtering.
    pub async fn list_intents(
        &self,
        filter: X402PaymentIntentFilter,
    ) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().list_async(filter).await
    }

    /// Count payment intents matching a filter.
    pub async fn count_intents(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        self.db.x402_payment_intents().count_async(filter).await
    }

    /// Expire all stale intents that have passed their validity period.
    pub async fn expire_stale_intents(&self) -> Result<u64> {
        self.db.x402_payment_intents().expire_stale_intents_async().await
    }

    /// Get intents by status.
    pub async fn intents_by_status(
        &self,
        status: X402IntentStatus,
    ) -> Result<Vec<X402PaymentIntent>> {
        self.list_intents(X402PaymentIntentFilter { status: Some(status), ..Default::default() })
            .await
    }

    /// Get pending intents.
    pub async fn pending_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Created).await
    }

    /// Get signed intents awaiting settlement.
    pub async fn signed_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Signed).await
    }

    /// Get settled intents.
    pub async fn settled_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Settled).await
    }

    // ========================================================================
    // Credit Ledger Operations
    // ========================================================================

    /// Get a credit account for a payer/asset/network.
    pub async fn get_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        self.db.x402_credits().get_account_async(payer_address, asset, network).await
    }

    /// Get or create a credit account (balance default = 0).
    pub async fn get_or_create_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        self.db.x402_credits().get_or_create_account_async(payer_address, asset, network).await
    }

    /// Get current credit balance for a payer/asset/network.
    pub async fn get_credit_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64> {
        self.db.x402_credits().get_balance_async(payer_address, asset, network).await
    }

    /// Apply a credit or debit adjustment.
    pub async fn adjust_credit_balance(
        &self,
        input: X402CreditAdjustment,
    ) -> Result<X402CreditTransaction> {
        self.db.x402_credits().adjust_balance_async(input).await
    }

    /// Credit an account (increase balance).
    #[allow(clippy::too_many_arguments)]
    pub async fn credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
        amount: u64,
        reason: Option<String>,
        reference_id: Option<String>,
        metadata: Option<String>,
    ) -> Result<X402CreditTransaction> {
        self.adjust_credit_balance(X402CreditAdjustment {
            payer_address: payer_address.to_string(),
            asset,
            network,
            direction: X402CreditDirection::Credit,
            amount,
            reason,
            reference_id,
            metadata,
        })
        .await
    }

    /// Debit an account (decrease balance).
    #[allow(clippy::too_many_arguments)]
    pub async fn debit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
        amount: u64,
        reason: Option<String>,
        reference_id: Option<String>,
        metadata: Option<String>,
    ) -> Result<X402CreditTransaction> {
        self.adjust_credit_balance(X402CreditAdjustment {
            payer_address: payer_address.to_string(),
            asset,
            network,
            direction: X402CreditDirection::Debit,
            amount,
            reason,
            reference_id,
            metadata,
        })
        .await
    }

    /// List credit ledger transactions.
    pub async fn list_credit_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        self.db.x402_credits().list_transactions_async(filter).await
    }

    // ========================================================================
    // Agent Card Operations
    // ========================================================================

    /// Register a new agent card.
    pub async fn register_agent(&self, input: CreateAgentCard) -> Result<AgentCard> {
        self.db.agent_cards().create_async(input).await
    }

    /// Get an agent card by ID.
    pub async fn get_agent(&self, id: Uuid) -> Result<Option<AgentCard>> {
        self.db.agent_cards().get_async(id).await
    }

    /// Get an agent card by wallet address.
    pub async fn get_agent_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>> {
        self.db.agent_cards().get_by_wallet_async(wallet_address).await
    }

    /// Update an agent card.
    pub async fn update_agent(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard> {
        self.db.agent_cards().update_async(id, input).await
    }

    /// Delete an agent card.
    pub async fn delete_agent(&self, id: Uuid) -> Result<()> {
        self.db.agent_cards().delete_async(id).await
    }

    /// List agent cards with optional filtering.
    pub async fn list_agents(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        self.db.agent_cards().list_async(filter).await
    }

    /// Count agent cards matching a filter.
    pub async fn count_agents(&self, filter: AgentCardFilter) -> Result<u64> {
        self.db.agent_cards().count_async(filter).await
    }

    /// Verify an agent card.
    pub async fn verify_agent(&self, id: Uuid) -> Result<AgentCard> {
        self.db.agent_cards().verify_async(id, TrustLevel::Verified, "system").await
    }

    /// Suspend an agent card.
    pub async fn suspend_agent(&self, id: Uuid, reason: &str) -> Result<AgentCard> {
        self.db.agent_cards().suspend_async(id, reason).await
    }

    /// Reactivate an agent card.
    pub async fn reactivate_agent(&self, id: Uuid) -> Result<AgentCard> {
        self.db.agent_cards().reactivate_async(id).await
    }

    /// Discover agents with specific capabilities.
    pub async fn discover_agents(
        &self,
        network: Option<X402Network>,
        asset: Option<X402Asset>,
        skill: Option<A2ASkill>,
        min_trust_level: Option<TrustLevel>,
    ) -> Result<Vec<AgentCard>> {
        self.db
            .agent_cards()
            .discover_async(AgentCardFilter {
                network,
                asset,
                skill,
                trust_level: None,
                min_trust_level,
                active: Some(true),
                ..Default::default()
            })
            .await
    }

    /// Get all active agents.
    pub async fn active_agents(&self) -> Result<Vec<AgentCard>> {
        self.list_agents(AgentCardFilter { active: Some(true), ..Default::default() }).await
    }

    /// Get agents by trust level.
    pub async fn agents_by_trust_level(&self, level: TrustLevel) -> Result<Vec<AgentCard>> {
        self.list_agents(AgentCardFilter {
            trust_level: Some(level),
            active: Some(true),
            ..Default::default()
        })
        .await
    }

    /// Get verified agents.
    pub async fn verified_agents(&self) -> Result<Vec<AgentCard>> {
        self.agents_by_trust_level(TrustLevel::Verified).await
    }

    /// Create a payment intent for a cart.
    pub async fn create_cart_payment(
        &self,
        cart_id: Uuid,
        payer_address: &str,
        payee_address: &str,
        amount: rust_decimal::Decimal,
        network: X402Network,
        asset: X402Asset,
    ) -> Result<X402PaymentIntent> {
        self.create_intent(CreateX402PaymentIntent {
            payer_address: payer_address.to_string(),
            payee_address: payee_address.to_string(),
            amount: to_smallest_unit(amount, asset),
            asset,
            network,
            cart_id: Some(cart_id),
            ..Default::default()
        })
        .await
    }

    /// Get the active payment intent for a cart.
    pub async fn active_intent_for_cart(&self, cart_id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let intents = self.intents_for_cart(cart_id).await?;
        Ok(intents.into_iter().find(|intent| {
            matches!(
                intent.status,
                X402IntentStatus::Created
                    | X402IntentStatus::Signed
                    | X402IntentStatus::Sequenced
                    | X402IntentStatus::Settled
            )
        }))
    }

    /// Check if an intent is ready for settlement.
    pub async fn is_ready_for_settlement(&self, id: Uuid) -> Result<bool> {
        if let Some(intent) = self.get_intent(id).await? {
            let now = Utc::now().timestamp() as u64;
            Ok(intent.status == X402IntentStatus::Signed && intent.valid_until > now)
        } else {
            Ok(false)
        }
    }

    /// Verify an intent's configured signature against its canonical signing hash.
    pub async fn has_valid_signature(&self, id: Uuid) -> Result<bool> {
        if let Some(intent) = self.get_intent(id).await? {
            if !intent.is_signed() {
                return Ok(false);
            }
            Ok(intent.verify_signature().unwrap_or(false))
        } else {
            Ok(false)
        }
    }

    // ========================================================================
    // A2A Commerce Operations
    // ========================================================================

    /// Create a new A2A quote.
    pub async fn create_quote(&self, input: CreateA2AQuote) -> Result<SkillQuote> {
        self.db.a2a_quotes().create_quote_async(input).await
    }

    /// Get an A2A quote by ID.
    pub async fn get_quote(&self, id: Uuid) -> Result<Option<SkillQuote>> {
        self.db.a2a_quotes().get_quote_async(id).await
    }

    /// Get an A2A quote by quote number.
    pub async fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<SkillQuote>> {
        self.db.a2a_quotes().get_quote_by_number_async(quote_number).await
    }

    /// Update A2A quote status.
    pub async fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<SkillQuote> {
        self.db.a2a_quotes().update_quote_status_async(id, status).await
    }

    /// List A2A quotes with filter.
    pub async fn list_quotes(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>> {
        self.db.a2a_quotes().list_quotes_async(filter).await
    }

    /// Count A2A quotes matching filter.
    pub async fn count_quotes(&self, filter: SkillQuoteFilter) -> Result<u64> {
        self.db.a2a_quotes().count_quotes_async(filter).await
    }

    /// Create a new A2A purchase.
    pub async fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase> {
        self.db.a2a_purchases().create_purchase_async(input).await
    }

    /// Get an A2A purchase by ID.
    pub async fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>> {
        self.db.a2a_purchases().get_purchase_async(id).await
    }

    /// Get an A2A purchase by purchase number.
    pub async fn get_purchase_by_number(
        &self,
        purchase_number: &str,
    ) -> Result<Option<A2APurchase>> {
        self.db.a2a_purchases().get_purchase_by_number_async(purchase_number).await
    }

    /// Update A2A purchase status.
    pub async fn update_purchase_status(
        &self,
        id: Uuid,
        status: PurchaseStatus,
    ) -> Result<A2APurchase> {
        self.db.a2a_purchases().update_purchase_status_async(id, status).await
    }

    /// Link an A2A purchase to an order.
    pub async fn link_purchase_to_order(
        &self,
        purchase_id: Uuid,
        order_id: Uuid,
    ) -> Result<A2APurchase> {
        self.db.a2a_purchases().link_purchase_to_order_async(purchase_id, order_id).await
    }

    /// Confirm delivery for an A2A purchase.
    pub async fn confirm_delivery(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase> {
        self.db
            .a2a_purchases()
            .confirm_delivery_async(purchase_id, signature, rating, feedback)
            .await
    }

    /// List A2A purchases with filter.
    pub async fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>> {
        self.db.a2a_purchases().list_purchases_async(filter).await
    }

    /// Count A2A purchases matching filter.
    pub async fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64> {
        self.db.a2a_purchases().count_purchases_async(filter).await
    }
}

use stateset_core::{
    AdjustPoints, AdjustStoreCredit, CreateGiftCard, CreateLoyaltyProgram, CreateStoreCredit,
    CustomerId, EnrollCustomer, GiftCard, GiftCardFilter, GiftCardId, GiftCardTransaction,
    LoyaltyAccount, LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyTransaction, StoreCredit, StoreCreditFilter, StoreCreditTransaction, UpdateGiftCard,
};

/// Async gift card operations (create, spend, refund, disable).
pub struct AsyncGiftCards {
    db: Arc<PostgresDatabase>,
}

impl AsyncGiftCards {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateGiftCard) -> Result<GiftCard> {
        self.db.gift_cards().create_async(input).await
    }

    pub async fn get(&self, id: GiftCardId) -> Result<Option<GiftCard>> {
        self.db.gift_cards().get_async(id).await
    }

    pub async fn get_by_code(&self, code: &str) -> Result<Option<GiftCard>> {
        self.db.gift_cards().get_by_code_async(code).await
    }

    pub async fn update(&self, id: GiftCardId, input: UpdateGiftCard) -> Result<GiftCard> {
        self.db.gift_cards().update_async(id, input).await
    }

    pub async fn list(&self, filter: GiftCardFilter) -> Result<Vec<GiftCard>> {
        self.db.gift_cards().list_async(filter).await
    }

    /// Charge (debit) a gift card. Rejected for non-positive amounts,
    /// inactive or expired cards, and insufficient balance.
    pub async fn charge(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        self.db.gift_cards().charge_async(id, amount, reference_id).await
    }

    /// Refund (credit) to a gift card. Rejected for non-positive amounts and
    /// disabled cards.
    pub async fn refund(
        &self,
        id: GiftCardId,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransaction> {
        self.db.gift_cards().refund_async(id, amount, reference_id).await
    }

    pub async fn disable(&self, id: GiftCardId) -> Result<GiftCard> {
        self.db.gift_cards().disable_async(id).await
    }

    pub async fn get_transactions(&self, id: GiftCardId) -> Result<Vec<GiftCardTransaction>> {
        self.db.gift_cards().get_transactions_async(id).await
    }
}

/// Async store credit operations (issue, adjust, apply).
pub struct AsyncStoreCredits {
    db: Arc<PostgresDatabase>,
}

impl AsyncStoreCredits {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateStoreCredit) -> Result<StoreCredit> {
        self.db.store_credits().create_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<StoreCredit>> {
        self.db.store_credits().get_async(id).await
    }

    pub async fn list(&self, filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        self.db.store_credits().list_async(filter).await
    }

    /// Adjust a store credit balance. Rejected for voided or expired credits.
    pub async fn adjust(&self, id: Uuid, input: AdjustStoreCredit) -> Result<StoreCredit> {
        self.db.store_credits().adjust_async(id, input).await
    }

    /// Apply (debit) store credit, e.g. against an order. Rejected for
    /// non-positive amounts, non-active or expired credits, and insufficient
    /// balance.
    pub async fn apply(
        &self,
        id: Uuid,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        self.db.store_credits().apply_async(id, amount, reference_id).await
    }

    pub async fn get_transactions(&self, id: Uuid) -> Result<Vec<StoreCreditTransaction>> {
        self.db.store_credits().get_transactions_async(id).await
    }
}

/// Async loyalty program operations (programs, accounts, points).
pub struct AsyncLoyalty {
    db: Arc<PostgresDatabase>,
}

impl AsyncLoyalty {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_program(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        self.db.loyalty().create_async(input).await
    }

    pub async fn get_program(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        self.db.loyalty().get_async(id).await
    }

    pub async fn list_programs(&self) -> Result<Vec<LoyaltyProgram>> {
        self.db.loyalty().list_async().await
    }

    pub async fn enroll(&self, input: EnrollCustomer) -> Result<LoyaltyAccount> {
        self.db.loyalty().enroll_async(input).await
    }

    pub async fn get_account(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        self.db.loyalty().get_account_async(id).await
    }

    pub async fn get_account_by_customer(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        self.db.loyalty().get_account_by_customer_async(customer_id, program_id).await
    }

    pub async fn list_accounts(&self, filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        self.db.loyalty().list_accounts_async(filter).await
    }

    /// Adjust points on an account (earn, redeem, expire, ...). Redemptions
    /// cannot overdraw the balance; unknown accounts error.
    pub async fn adjust_points(&self, input: AdjustPoints) -> Result<LoyaltyTransaction> {
        self.db.loyalty().adjust_points_async(input).await
    }

    pub async fn get_transactions(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        self.db.loyalty().get_transactions_async(account_id, limit).await
    }
}

// ============================================================================
// Async Fixed Assets
// ============================================================================

/// Async fixed asset register operations.
pub struct AsyncFixedAssets {
    db: Arc<PostgresDatabase>,
}

impl AsyncFixedAssets {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new fixed asset.
    pub async fn create(&self, input: CreateFixedAsset) -> Result<FixedAsset> {
        self.db.fixed_assets().create_async(input).await
    }

    /// Get a fixed asset by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        self.db.fixed_assets().get_async(id).await
    }

    /// List fixed assets with optional filtering.
    pub async fn list(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>> {
        self.db.fixed_assets().list_async(filter).await
    }

    /// Update a fixed asset (partial).
    pub async fn update(&self, id: Uuid, input: UpdateFixedAsset) -> Result<FixedAsset> {
        self.db.fixed_assets().update_async(id, input).await
    }

    /// Place a draft asset in service.
    pub async fn place_in_service(&self, id: Uuid, date: NaiveDate) -> Result<FixedAsset> {
        self.db.fixed_assets().place_in_service_async(id, date).await
    }

    /// Dispose of an asset for the given proceeds, recording gain/loss.
    pub async fn dispose(
        &self,
        id: Uuid,
        date: NaiveDate,
        proceeds: Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.db.fixed_assets().dispose_async(id, date, proceeds, notes).await
    }

    /// Write off an asset (disposal with zero proceeds).
    pub async fn write_off(
        &self,
        id: Uuid,
        date: NaiveDate,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.db.fixed_assets().write_off_async(id, date, notes).await
    }

    /// Generate and persist the depreciation schedule for an asset.
    pub async fn generate_schedule(&self, id: Uuid) -> Result<DepreciationSchedule> {
        self.db.fixed_assets().generate_schedule_async(id).await
    }

    /// Get the persisted depreciation schedule for an asset, if generated.
    pub async fn get_schedule(&self, id: Uuid) -> Result<Option<DepreciationSchedule>> {
        self.db.fixed_assets().get_schedule_async(id).await
    }

    /// Post the next `periods` scheduled depreciation entries.
    pub async fn post_depreciation(&self, id: Uuid, periods: u32) -> Result<FixedAsset> {
        self.db.fixed_assets().post_depreciation_async(id, periods).await
    }
}

// ============================================================================
// Async Revenue Recognition
// ============================================================================

/// Async revenue recognition (ASC 606 style) operations.
pub struct AsyncRevenueRecognition {
    db: Arc<PostgresDatabase>,
}

impl AsyncRevenueRecognition {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a revenue contract with its performance obligations.
    pub async fn create_contract(&self, input: CreateRevenueContract) -> Result<RevenueContract> {
        self.db.revenue_recognition().create_contract_async(input).await
    }

    /// Get a revenue contract (with obligations) by ID.
    pub async fn get_contract(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        self.db.revenue_recognition().get_contract_async(id).await
    }

    /// List revenue contracts matching the filter.
    pub async fn list_contracts(
        &self,
        filter: RevenueContractFilter,
    ) -> Result<Vec<RevenueContract>> {
        self.db.revenue_recognition().list_contracts_async(filter).await
    }

    /// Update a revenue contract (partial).
    pub async fn update_contract(
        &self,
        id: Uuid,
        input: UpdateRevenueContract,
    ) -> Result<RevenueContract> {
        self.db.revenue_recognition().update_contract_async(id, input).await
    }

    /// List the performance obligations under a contract.
    pub async fn list_obligations(&self, contract_id: Uuid) -> Result<Vec<PerformanceObligation>> {
        self.db.revenue_recognition().list_obligations_async(contract_id).await
    }

    /// Generate and persist the recognition schedule for an obligation.
    pub async fn generate_schedule(&self, obligation_id: Uuid) -> Result<RevenueSchedule> {
        self.db.revenue_recognition().generate_schedule_async(obligation_id).await
    }

    /// Get the persisted recognition schedule for an obligation, if generated.
    pub async fn get_schedule(&self, obligation_id: Uuid) -> Result<Option<RevenueSchedule>> {
        self.db.revenue_recognition().get_schedule_async(obligation_id).await
    }

    /// Recognize all scheduled revenue through the given date.
    pub async fn recognize_period(
        &self,
        obligation_id: Uuid,
        through: NaiveDate,
    ) -> Result<RevenueSchedule> {
        self.db.revenue_recognition().recognize_period_async(obligation_id, through).await
    }
}

impl_opaque_debug!(
    AsyncCommerce,
    AsyncFixedAssets,
    AsyncRevenueRecognition,
    AsyncGiftCards,
    AsyncStoreCredits,
    AsyncLoyalty,
    AsyncOrders,
    AsyncInventory,
    AsyncCustomers,
    AsyncProducts,
    AsyncCustomObjects,
    AsyncReturns,
    AsyncShipments,
    AsyncPayments,
    AsyncWarranties,
    AsyncBom,
    AsyncWorkOrders,
    AsyncPurchaseOrders,
    AsyncInvoices,
    AsyncCarts,
    AsyncAnalytics,
    AsyncCurrency,
    AsyncTax,
    AsyncPromotions,
    AsyncSubscriptions,
    AsyncQuality,
    AsyncLots,
    AsyncSerials,
    AsyncWarehouse,
    AsyncReceiving,
    AsyncFulfillment,
    AsyncAccountsPayable,
    AsyncCostAccounting,
    AsyncCredit,
    AsyncBackorder,
    AsyncAccountsReceivable,
    AsyncGeneralLedger,
    AsyncX402,
);
