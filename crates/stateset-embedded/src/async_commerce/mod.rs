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

// Shared imports for every accessor module in this directory —
// `pub(crate)` so submodules can pull the whole prelude via `use super::*`.
pub(crate) use chrono::{DateTime, NaiveDate, Utc};
pub(crate) use rust_decimal::{Decimal, prelude::ToPrimitive};
pub(crate) use stateset_core::{
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
    ShipOrder,
    Shipment,
    ShipmentEvent,
    ShipmentFilter,
    ShipmentItem,
    ShipmentLineInput,
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
pub(crate) use stateset_core::{
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
    LotGenealogyLink,
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
pub(crate) use stateset_core::{
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
pub(crate) use stateset_db::PostgresDatabase;
pub(crate) use stateset_observability::{Metrics, MetricsConfig, MetricsSnapshot, init_metrics};
pub(crate) use std::sync::Arc;
pub(crate) use uuid::Uuid;

#[cfg(feature = "events")]
pub(crate) use crate::events::EventSystem;
#[cfg(feature = "events")]
pub(crate) use stateset_core::CommerceEvent;

pub(crate) use stateset_core::{
    A2APurchase, A2APurchaseFilter, A2ASkill, AgentCard, AgentCardFilter, CreateA2APurchase,
    CreateA2AQuote, CreateAgentCard, CreateX402PaymentIntent, PurchaseStatus, QuoteStatus,
    SignX402PaymentIntent, SkillQuote, SkillQuoteFilter, TrustLevel, UpdateAgentCard, X402Asset,
    X402CreditAccount, X402CreditAdjustment, X402CreditDirection, X402CreditTransaction,
    X402CreditTransactionFilter, X402IntentStatus, X402Network, X402PaymentIntent,
    X402PaymentIntentFilter, to_smallest_unit,
};
pub(crate) use stateset_core::{
    CreateCustomObject, CreateCustomObjectType, CustomObject, CustomObjectFilter, CustomObjectType,
    CustomObjectTypeFilter, UpdateCustomObject, UpdateCustomObjectType,
};

pub(crate) use stateset_core::{
    AdjustPoints, AdjustStoreCredit, CreateGiftCard, CreateLoyaltyProgram, CreateStoreCredit,
    CustomerId, EnrollCustomer, GiftCard, GiftCardFilter, GiftCardId, GiftCardTransaction,
    LoyaltyAccount, LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyTransaction, StoreCredit, StoreCreditFilter, StoreCreditTransaction, UpdateGiftCard,
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

mod b2b;
mod core;
mod finance;
mod growth;
mod manufacturing;
mod storefront;
mod traceability;
mod warehouse;
mod x402;

pub use b2b::*;
pub use core::*;
pub use finance::*;
pub use growth::*;
pub use manufacturing::*;
pub use storefront::*;
pub use traceability::*;
pub use warehouse::*;
pub use x402::*;

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
