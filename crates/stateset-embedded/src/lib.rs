#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet iCommerce
//!
//! The SQLite of commerce operations. An embeddable commerce library
//! that runs anywhere with zero external dependencies.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateCustomer, CreateOrder, CreateOrderItem, CreateInventoryItem};
//! use rust_decimal_macros::dec;
//!
//! // Initialize with a database file (creates if not exists)
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a customer
//! let customer = commerce.customers().create(CreateCustomer {
//!     email: "alice@example.com".into(),
//!     first_name: "Alice".into(),
//!     last_name: "Smith".into(),
//!     ..Default::default()
//! })?;
//!
//! // Create inventory
//! commerce.inventory().create_item(CreateInventoryItem {
//!     sku: "SKU-001".into(),
//!     name: "Widget".into(),
//!     initial_quantity: Some(dec!(100)),
//!     ..Default::default()
//! })?;
//!
//! // Create an order
//! let order = commerce.orders().create(CreateOrder {
//!     customer_id: customer.id,
//!     items: vec![CreateOrderItem {
//!         sku: "SKU-001".into(),
//!         name: "Widget".into(),
//!         quantity: 2,
//!         unit_price: dec!(29.99),
//!         ..Default::default()
//!     }],
//!     ..Default::default()
//! })?;
//!
//! // Adjust inventory
//! commerce.inventory().adjust("SKU-001", dec!(-2), "Order fulfillment")?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```
//!
//! ## Features
//!
//! - **Zero configuration** - Just point to a file and go
//! - **Embedded SQLite** - No external database server needed (default)
//! - **PostgreSQL support** - Scale to production with `postgres` feature
//! - **Full commerce stack** - Orders, inventory, customers, products, returns
//! - **Sync API** - Simple blocking operations
//! - **Async API** - True async for PostgreSQL with `AsyncCommerce`
//! - **Event-driven** - Subscribe to commerce events for side effects
//!
//! ## Database Backends
//!
//! ### SQLite (default)
//! ```rust,ignore
//! let commerce = Commerce::new("./store.db")?;
//! // or in-memory for testing
//! let commerce = Commerce::new(":memory:")?;
//! ```
//!
//! ### PostgreSQL (requires `postgres` feature)
//! ```rust,ignore
//! let commerce = Commerce::with_postgres("postgres://user:pass@localhost/db")?;
//! // or via builder
//! let commerce = Commerce::builder()
//!     .postgres("postgres://localhost/stateset")
//!     .max_connections(20)
//!     .build()?;
//! ```
//!
//! ### Async PostgreSQL API
//!
//! For true async operations with PostgreSQL, use `AsyncCommerce`:
//!
//! ```rust,ignore
//! use stateset_embedded::{AsyncCommerce, CreateOrder, CreateOrderItem};
//! use rust_decimal_macros::dec;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let commerce = AsyncCommerce::connect("postgres://localhost/stateset").await?;
//!
//!     // All operations are truly async
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
//!     let customers = commerce.customers().list(Default::default()).await?;
//!     let inventory = commerce.inventory().get_stock("SKU-001").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │           Your Application              │
//! │  ┌───────────────────────────────────┐  │
//! │  │     Commerce (this crate)         │  │
//! │  │  ┌─────────────────────────────┐  │  │
//! │  │  │  SQLite or PostgreSQL       │  │  │
//! │  │  └─────────────────────────────┘  │  │
//! │  └───────────────────────────────────┘  │
//! └─────────────────────────────────────────┘
//! ```

#[cfg(feature = "async")]
use futures as _;
#[cfg(feature = "postgres")]
use sqlx as _;

mod accounts_payable;
mod accounts_receivable;
mod activity_logs;
mod analytics;
mod backorder;
mod bom;
mod carts;
mod channels;
mod commerce;
mod companies;
mod cost_accounting;
mod credit;
mod currency;
mod custom_objects;
mod customers;
mod edi_documents;
mod erc8004;
mod fixed_assets;
mod fraud;
mod fulfillment;
mod general_ledger;
mod gift_cards;
mod inbound_shipments;
mod integration_field_mappings;
mod integration_mappings;
mod inventory;
mod invoices;
mod lots;
mod loyalty;
pub mod maintenance;
mod orders;
mod payment_obligations;
mod payments;
mod prepayments;
mod price_levels;
mod price_schedules;
mod print_stations;
mod production_batches;
mod products;
mod promotions;
mod purchase_orders;
mod purgatory;
mod quality;
mod receiving;
mod returns;
mod revenue_recognition;
mod reviews;
mod search_config;
mod segments;
mod serials;
mod shipments;
mod shipping_zones;
mod stock_snapshots;
mod store_credits;
mod subscriptions;
mod supplier_skus;
mod tax;
mod topology_snapshots;
mod transfer_orders;
mod units_of_measure;
mod vendor_credits;
mod vendor_returns;
mod warehouse;
mod warranties;
mod wishlists;
mod work_orders;
mod x402;

pub mod notifications;

#[cfg(feature = "vector")]
mod vector;

#[cfg(feature = "postgres")]
mod async_commerce;

#[cfg(feature = "events")]
pub mod events;

/// Curated stable subset of the public API — see [`prelude`] for what
/// `stateset-embedded` intends to commit to at 1.0.
pub mod prelude;

// Event system types (feature-gated)
#[cfg(feature = "events")]
pub use events::{
    EventBus, EventConfig, EventReceiver, EventSubscription, EventSystem, FilteredSubscription,
    InMemoryEventStore, Webhook, WebhookConfig, WebhookDelivery, WebhookManager,
    WebhookRegistrationError, filters,
};

#[cfg(all(feature = "events", feature = "sqlite-events"))]
pub use events::SqliteEventStore;

pub use accounts_payable::AccountsPayable;
pub use accounts_receivable::AccountsReceivable;
pub use activity_logs::ActivityLogs;
pub use analytics::Analytics;
pub use backorder::Backorders;
pub use bom::Bom;
pub use carts::Carts;
pub use channels::Channels;
pub use commerce::{Commerce, CommerceBackend, CommerceBuilder, CommerceHealth};
pub use companies::Companies;
pub use cost_accounting::CostAccounting;
pub use credit::Credit;
pub use currency::CurrencyOps;
pub use custom_objects::CustomObjects;
pub use customers::Customers;
pub use edi_documents::EdiDocuments;
pub use erc8004::Erc8004;
pub use fixed_assets::FixedAssets;
pub use fraud::Fraud;
pub use fulfillment::Fulfillment;
pub use general_ledger::GeneralLedger;
pub use gift_cards::GiftCards;
pub use inbound_shipments::InboundShipments;
pub use integration_field_mappings::IntegrationFieldMappings;
pub use integration_mappings::IntegrationMappings;
pub use inventory::Inventory;
pub use invoices::Invoices;
pub use lots::Lots;
pub use loyalty::Loyalty;
pub use maintenance::Maintenance;
pub use orders::Orders;
pub use payment_obligations::PaymentObligations;
pub use payments::Payments;
pub use prepayments::Prepayments;
pub use price_levels::PriceLevels;
pub use price_schedules::PriceSchedules;
pub use print_stations::PrintStations;
pub use production_batches::ProductionBatches;
pub use products::Products;
pub use promotions::Promotions;
pub use purchase_orders::PurchaseOrders;
pub use purgatory::Purgatory;
pub use quality::Quality;
pub use receiving::Receiving;
pub use returns::Returns;
pub use revenue_recognition::RevenueRecognition;
pub use reviews::Reviews;
pub use search_config::SearchConfigs;
pub use segments::Segments;
pub use serials::Serials;
pub use shipments::Shipments;
pub use shipping_zones::ShippingZones;
pub use stock_snapshots::StockSnapshots;
pub use store_credits::StoreCredits;
pub use subscriptions::Subscriptions;
pub use supplier_skus::SupplierSkus;
pub use tax::Tax;
pub use topology_snapshots::TopologySnapshots;
pub use transfer_orders::TransferOrders;
pub use units_of_measure::UnitsOfMeasure;
pub use vendor_credits::VendorCredits;
pub use vendor_returns::VendorReturns;
pub use warehouse::WarehouseOps;
pub use warranties::Warranties;
pub use wishlists::Wishlists;
pub use work_orders::WorkOrders;
pub use x402::X402;

pub use notifications::{
    EmailBackend, EmailTemplate, LogEmailBackend, NotificationConfig, NotificationService,
    RecipientResolver, TransactionalEmail, WebhookEmailBackend,
};

#[cfg(feature = "vector")]
pub use vector::Vector;

// Async API for PostgreSQL (feature-gated)
#[cfg(feature = "postgres")]
pub use async_commerce::{
    AsyncAnalytics, AsyncBom, AsyncCarts, AsyncCommerce, AsyncCurrency, AsyncCustomObjects,
    AsyncCustomers, AsyncFixedAssets, AsyncGiftCards, AsyncInventory, AsyncInvoices, AsyncLoyalty,
    AsyncOrders, AsyncPayments, AsyncProducts, AsyncPurchaseOrders, AsyncReturns,
    AsyncRevenueRecognition, AsyncShipments, AsyncStoreCredits, AsyncWarranties, AsyncWorkOrders,
    AsyncX402,
};

// Re-export Database trait for advanced users who want to bring their own database
pub use stateset_db::Database;
// Re-export the durable HTTP idempotency store types for the HTTP layer
pub use stateset_db::{HttpIdempotencyRecord, HttpIdempotencyRepository};
// Re-export observability primitives used by Commerce diagnostics/metrics APIs
pub use stateset_observability::{Metrics, MetricsConfig, MetricsSnapshot};

// Re-export core types for convenience
pub use stateset_core::{
    A2APurchase,
    A2APurchaseFilter,
    A2AQuote,
    A2AQuoteFilter,
    // A2A Skill types
    A2ASkill,
    // General Ledger types
    AccountStatus,
    AccountSubType,
    AccountType,
    // Cart/Checkout types
    AddCartItem,
    // Fulfillment types
    AddCarton,
    AddCartonItem,
    // Lot/Batch Tracking types
    AddLotCertificate,
    // Shipment types
    AddShipmentEvent,
    // Manufacturing - Work Order types
    AddWorkOrderMaterial,
    // Order types
    Address,
    // Customer types
    AddressType,
    // Inventory types
    AdjustInventory,
    // Warehouse & Location types
    AdjustLocationInventory,
    AdjustLot,
    // Agent Card types
    AgentCard,
    AgentCardFilter,
    AgentFeedback,
    AgentFeedbackFilter,
    AgentFeedbackResponse,
    // ERC-8004 types
    AgentIdentity,
    AgentIdentityFilter,
    AgentMetadataEntry,
    AgentRegistrationFile,
    AgentRegistrationRef,
    AgentServiceEndpoint,
    AgentValidationFilter,
    AgentValidationRequest,
    AgentValidationResponse,
    AgentValidationStatus,
    AgentWalletProofType,
    AgingBucket,
    // Backorder types
    AllocateBackorder,
    AllocationStatus,
    // Analytics types
    AnalyticsQuery,
    // Accounts Payable types
    ApAgingSummary,
    // Promotion types
    AppliedPromotion,
    ApplyCartDiscount,
    // Accounts Receivable types
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
    BackorderPriority,
    BackorderStatus,
    BackorderSummary,
    BalanceSheet,
    BalanceSheetLine,
    BalanceSide,
    BatchResult,
    Bill,
    BillFilter,
    BillItem,
    // Manufacturing - BOM types
    BillOfMaterials,
    BillPayment,
    BillPaymentFilter,
    BillStatus,
    // Subscription types
    BillingCycle,
    BillingCycleFilter,
    BillingCycleStatus,
    BillingInterval,
    BomComponent,
    BomFilter,
    BomStatus,
    // Tax types
    CanadianTaxInfo,
    CancelSubscription,
    // Payment types
    CardBrand,
    Cart,
    CartAddress,
    CartFilter,
    CartItem,
    CartPaymentStatus,
    CartStatus,
    Carton,
    CartonItem,
    CertificateType,
    // Serial Number types
    ChangeSerialStatus,
    CheckoutResult,
    // Warranty types
    ClaimResolution,
    ClaimStatus,
    // Month-end close orchestration types
    CloseMonthOptions,
    CloseMonthReport,
    CloseMonthStepReport,
    CloseMonthStepStatus,
    CollectionActivity,
    CollectionActivityFilter,
    CollectionActivityType,
    CollectionStatus,
    // Errors
    CommerceError,
    // Events
    CommerceEvent,
    // Quality Control types
    CompleteInspection,
    CompletePick,
    // Receiving types
    CompletePutAway,
    CompleteShip,
    ConditionOperator,
    ConditionType,
    ConfirmDeliveryInput,
    ConfirmDeliveryOutput,
    ConsumeLot,
    // Currency types
    ConversionResult,
    ConvertCurrency,
    // Cost Accounting types
    CostAdjustment,
    CostAdjustmentFilter,
    CostAdjustmentStatus,
    CostAdjustmentType,
    CostLayer,
    CostLayerFilter,
    CostLayerSource,
    CostMethod,
    CostRollup,
    CostTransaction,
    CostTransactionFilter,
    CostTransactionType,
    CostVariance,
    CostVarianceFilter,
    CouponCode,
    CouponFilter,
    CouponStatus,
    CreateA2APurchase,
    CreateA2AQuote,
    CreateAgentCard,
    CreateAgentFeedback,
    CreateAgentFeedbackResponse,
    CreateAgentIdentity,
    CreateAgentValidationRequest,
    CreateAgentValidationResponse,
    CreateAutoPostingConfig,
    CreateBackorder,
    CreateBill,
    CreateBillItem,
    CreateBillPayment,
    CreateBillingCycle,
    CreateBom,
    CreateBomComponent,
    CreateCart,
    CreateCollectionActivity,
    CreateCostAdjustment,
    CreateCostLayer,
    CreateCouponCode,
    // Credit types
    CreateCreditAccount,
    CreateCreditMemo,
    CreateCustomObject,
    CreateCustomObjectType,
    CreateCustomer,
    CreateCustomerAddress,
    // Cycle count types
    CreateCycleCount,
    CreateCycleCountLine,
    CreateDefectCode,
    // Fraud types
    CreateFraudAssessment,
    // Gift Card types
    CreateGiftCard,
    CreateGlAccount,
    CreateGlPeriod,
    CreateInspection,
    CreateInspectionItem,
    CreateInventoryItem,
    // Invoice types
    CreateInvoice,
    CreateInvoiceItem,
    CreateJournalEntry,
    CreateJournalEntryLine,
    CreateLocation,
    CreateLot,
    // Loyalty types
    CreateLoyaltyProgram,
    CreateNcr,
    CreateNonConformance,
    CreateOrder,
    CreateOrderItem,
    CreatePackTask,
    CreatePayment,
    CreatePaymentMethod,
    CreatePaymentRun,
    CreatePickTask,
    // Product types
    CreateProduct,
    CreateProductVariant,
    CreatePromotion,
    CreatePromotionCondition,
    // Purchase Order types
    CreatePurchaseOrder,
    CreatePurchaseOrderItem,
    CreatePutAway,
    CreateQualityHold,
    CreateReceipt,
    CreateReceiptItem,
    CreateReceiptLine,
    CreateRefund,
    // Return types
    CreateReturn,
    CreateReturnItem,
    // Review types
    CreateReview,
    // Search Config types
    CreateSearchConfig,
    // Segment types
    CreateSegment,
    CreateSerial,
    CreateSerialNumber,
    CreateSerialNumbersBulk,
    CreateShipTask,
    CreateShipment,
    CreateShipmentItem,
    // Shipping Zone types
    CreateShippingZone,
    // Store Credit types
    CreateStoreCredit,
    CreateSubscription,
    CreateSubscriptionItem,
    CreateSubscriptionPlan,
    CreateSubscriptionPlanItem,
    CreateSupplier,
    CreateTaxExemption,
    CreateTaxJurisdiction,
    CreateTaxRate,
    CreateWarehouse,
    CreateWarehouseLocation,
    CreateWarranty,
    CreateWarrantyClaim,
    CreateWave,
    // Wishlist types
    CreateWishlist,
    CreateWorkOrder,
    CreateWorkOrderTask,
    CreateWriteOff,
    CreateX402CreditAccount,
    // x402 Payment Protocol types
    CreateX402PaymentIntent,
    CreateZone,
    CreditAccount,
    CreditAccountFilter,
    CreditAccountStatus,
    CreditAgingBucket,
    CreditApplication,
    CreditApplicationFilter,
    CreditApplicationStatus,
    CreditCheckResult,
    CreditHold,
    CreditHoldFilter,
    CreditHoldStatus,
    CreditHoldType,
    CreditMemo,
    CreditMemoApplication,
    CreditMemoFilter,
    CreditMemoReason,
    CreditMemoStatus,
    CreditTransaction,
    CreditTransactionFilter,
    CreditTransactionType,
    Currency,
    CurrencyCode,
    // Custom Objects (custom states / metaobjects)
    CustomFieldDefinition,
    CustomFieldType,
    CustomObject,
    CustomObjectFilter,
    CustomObjectType,
    CustomObjectTypeFilter,
    Customer,
    CustomerAddress,
    CustomerArAging,
    CustomerArSummary,
    CustomerCreditSummary,
    CustomerFilter,
    CustomerId,
    CustomerMetrics,
    CustomerStatement,
    CustomerStatus,
    CycleCount,
    CycleCountFilter,
    CycleCountLine,
    CycleCountStatus,
    DateRange,
    DefectCode,
    DemandForecast,
    DiscountTier,
    DiscoverSellersInput,
    DiscoverSellersOutput,
    Disposition,
    DunningLetterType,
    ERC8004_REGISTRATION_V1,
    EuVatInfo,
    ExchangeRate,
    ExchangeRateFilter,
    ExemptionType,
    FeedbackSummary,
    FulfillBackorder,
    // Typed IDs
    FulfillmentId,
    FulfillmentMetrics,
    FulfillmentSourceType,
    FulfillmentStatus,
    FulfillmentType,
    GenerateStatementRequest,
    GlAccount,
    GlAccountFilter,
    GlPeriod,
    GlPeriodFilter,
    HoldType,
    IncomeStatement,
    IncomeStatementLine,
    InitiatePurchaseInput,
    InitiatePurchaseOutput,
    Inspection,
    InspectionFilter,
    InspectionItem,
    InspectionResult,
    InspectionStatus,
    InspectionType,
    InventoryBalance,
    InventoryFilter,
    InventoryHealth,
    InventoryItem,
    InventoryMovement,
    InventoryReservation,
    InventoryTransaction,
    InventoryValuation,
    Invoice,
    InvoiceFilter,
    InvoiceItem,
    InvoiceStatus,
    InvoiceType,
    IssueCostLayers,
    ItemAvailability,
    ItemCondition,
    ItemCost,
    ItemCostFilter,
    JournalEntry,
    JournalEntryFilter,
    JournalEntryLine,
    JournalEntrySource,
    JournalEntryStatus,
    JournalEntryType,
    JurisdictionLevel,
    JurisdictionSummary,
    LineItemDiscount,
    LineItemTax,
    Location,
    LocationFilter,
    LocationInventory,
    LocationInventoryFilter,
    LocationMovement,
    LocationStock,
    LocationType,
    Lot,
    LotCertificate,
    LotFilter,
    LotLocation,
    LotStatus,
    LotTransaction,
    LotTransactionType,
    LowStockItem,
    MergeLots,
    Money,
    MoveInventory,
    MoveSerial,
    MovementFilter,
    MovementType,
    NcrStatus,
    NonConformance,
    NonConformanceFilter,
    NonConformanceSource,
    Order,
    OrderFilter,
    OrderId,
    OrderItem,
    OrderItemId,
    OrderStatus,
    OrderStatusBreakdown,
    PackStatus,
    PackTask,
    PackTaskFilter,
    PackageType,
    PauseSubscription,
    PayBill,
    Payment,
    PaymentAllocation,
    PaymentAllocationInput,
    PaymentApplicationLine,
    PaymentFilter,
    PaymentId,
    PaymentMethod,
    PaymentMethodAP,
    PaymentMethodType,
    PaymentRun,
    PaymentRunFilter,
    PaymentRunStatus,
    PaymentStatus,
    PaymentStatusAP,
    PaymentTerms,
    PaymentTransactionStatus,
    PeriodStatus,
    PickStatus,
    PickTask,
    PickTaskFilter,
    PlaceCreditHold,
    PlanStatus,
    Product,
    ProductAttribute,
    ProductFilter,
    ProductId,
    ProductPerformance,
    ProductStatus,
    ProductTaxCategory,
    ProductType,
    ProductVariant,
    Promotion,
    PromotionCondition,
    PromotionFilter,
    PromotionLineItem,
    PromotionStatus,
    PromotionTarget,
    PromotionTrigger,
    PromotionType,
    PromotionUsage,
    PurchaseOrder,
    PurchaseOrderFilter,
    PurchaseOrderItem,
    PurchaseOrderStatus,
    PurchaseStatus,
    PutAway,
    PutAwayFilter,
    PutAwayStatus,
    QualityHold,
    QualityHoldFilter,
    QuoteItem,
    QuoteStatus,
    QuotedItem,
    Receipt,
    ReceiptFilter,
    ReceiptItem,
    ReceiptItemStatus,
    ReceiptStatus,
    ReceiptType,
    ReceiveItemLine,
    ReceiveItems,
    ReceivePurchaseOrderItems,
    RecordCostVariance,
    RecordCreditTransaction,
    RecordCycleCountLine,
    RecordInspectionResult,
    RecordInvoicePayment,
    Refund,
    RefundStatus,
    RejectedPromotion,
    RejectionReason,
    ReleaseCreditHold,
    ReleaseQualityHold,
    RequestQuoteInput,
    RequestQuoteOutput,
    ReservationStatus,
    ReserveInventory,
    ReserveLot,
    ReserveSerialNumber,
    Result,
    Return,
    ReturnFilter,
    ReturnId,
    ReturnItem,
    ReturnMetrics,
    ReturnReason,
    ReturnReasonCount,
    ReturnStatus,
    RevenueByPeriod,
    RevenueForecast,
    ReviewCreditApplication,
    RiskRating,
    RoundingMode,
    SalesSummary,
    SegmentType,
    SellerInfo,
    SeoMetadata,
    SerialEventType,
    SerialFilter,
    SerialHistory,
    SerialHistoryFilter,
    SerialLookupResult,
    SerialNumber,
    SerialReservation,
    SerialStatus,
    SerialValidation,
    SetCartPayment,
    SetCartShipping,
    SetExchangeRate,
    SetItemCost,
    Severity,
    ShipStatus,
    ShipTask,
    ShipTaskFilter,
    Shipment,
    ShipmentEvent,
    ShipmentFilter,
    ShipmentId,
    ShipmentItem,
    ShipmentStatus,
    ShippingCarrier,
    ShippingMethod,
    ShippingRate,
    SignX402PaymentIntent,
    SkillQuote,
    SkillQuoteFilter,
    SkipBillingCycle,
    SkuBackorderSummary,
    SkuCostSummary,
    SplitLot,
    StackingBehavior,
    StatementLineItem,
    StatementTransactionType,
    StockLevel,
    StoreCreditReason,
    StoreCurrencySettings,
    SubmitCreditApplication,
    Subscription,
    SubscriptionEvent,
    SubscriptionEventType,
    SubscriptionFilter,
    SubscriptionItem,
    SubscriptionPlan,
    SubscriptionPlanFilter,
    SubscriptionPlanItem,
    SubscriptionStatus,
    Supplier,
    SupplierApSummary,
    SupplierFilter,
    TaskStatus,
    TaxAddress,
    TaxBreakdown,
    TaxCalculationMethod,
    TaxCalculationRequest,
    TaxCalculationResult,
    TaxCompoundMethod,
    TaxDetail,
    TaxExemption,
    TaxJurisdiction,
    TaxJurisdictionFilter,
    TaxLineItem,
    TaxRate,
    TaxRateFilter,
    TaxSettings,
    TaxType,
    TimeGranularity,
    TimePeriod,
    TopCustomer,
    TopProduct,
    TopReturnedProduct,
    TraceNode,
    TraceNodeType,
    TraceabilityResult,
    TransactionType,
    TransferLot,
    TransferSerialOwnership,
    Trend,
    TrialBalance,
    TrialBalanceLine,
    TrustLevel,
    UpdateAgentCard,
    UpdateAgentIdentity,
    UpdateBackorder,
    UpdateBill,
    UpdateBom,
    UpdateCart,
    UpdateCartItem,
    UpdateCreditAccount,
    UpdateCustomObject,
    UpdateCustomObjectType,
    UpdateCustomer,
    UpdateGlAccount,
    UpdateInspection,
    UpdateInvoice,
    UpdateLocation,
    UpdateLot,
    UpdateNonConformance,
    UpdateOrder,
    UpdatePayment,
    UpdateProduct,
    UpdatePromotion,
    UpdatePurchaseOrder,
    UpdateReceipt,
    UpdateReturn,
    UpdateSerialNumber,
    UpdateShipment,
    UpdateSubscription,
    UpdateSubscriptionPlan,
    UpdateSupplier,
    UpdateWarehouse,
    UpdateWarranty,
    UpdateWarrantyClaim,
    UpdateWorkOrder,
    UpdateWorkOrderTask,
    UpdateZone,
    UsStateTaxInfo,
    ValidationSummary,
    VarianceType,
    VariantOption,
    Warehouse,
    WarehouseAddress,
    WarehouseFilter,
    WarehouseType,
    Warranty,
    WarrantyClaim,
    WarrantyClaimFilter,
    WarrantyFilter,
    WarrantyLookupStatus,
    WarrantyStatus,
    WarrantyType,
    Wave,
    WaveFilter,
    WaveStatus,
    WaveType,
    WorkOrder,
    WorkOrderFilter,
    WorkOrderMaterial,
    WorkOrderPriority,
    WorkOrderStatus,
    WorkOrderTask,
    WriteOff,
    WriteOffFilter,
    WriteOffReason,
    X402_DEFAULT_VALIDITY_SECONDS,
    X402_DOMAIN_SEPARATOR,
    X402_MAX_VALIDITY_SECONDS,
    X402_VERSION,
    X402Asset,
    X402BatchStatus,
    X402CreditAccount,
    X402CreditAdjustment,
    X402CreditDirection,
    X402CreditTransaction,
    X402CreditTransactionFilter,
    X402CryptoError,
    X402IntentStatus,
    X402Network,
    X402PaymentBatch,
    X402PaymentIntent,
    X402PaymentIntentFilter,
    X402PaymentReceipt,
    X402PaymentRequired,
    Zone,
    from_smallest_unit,
    generate_ap_payment_number,
    generate_backorder_number,
    generate_bill_number,
    generate_claim_number,
    generate_cost_adjustment_number,
    generate_coupon_code,
    generate_credit_application_number,
    generate_credit_memo_number,
    generate_invoice_number,
    generate_journal_entry_number,
    generate_payment_number,
    generate_payment_run_number,
    generate_plan_code,
    generate_po_number,
    generate_promotion_code,
    generate_refund_number,
    generate_subscription_number,
    generate_warranty_number,
    generate_write_off_number,
    generate_x402_intent_id,
    get_canadian_tax_info,
    get_eu_vat_info,
    get_us_state_tax_info,
    is_eu_member,
    to_smallest_unit,
    validate_currency_code,
    validate_custom_object_type_input,
    // Validation utilities
    validate_email,
    validate_phone,
    validate_postal_code,
    validate_price,
    validate_quantity,
    validate_sku,
};

// Vector search types (feature-gated)
#[cfg(feature = "vector")]
pub use stateset_core::{
    EmbeddingConfig, EmbeddingMetadata, EmbeddingStats, EntityType, VectorSearchQuery,
    VectorSearchResult,
};

/// Compiles the code examples in `README.md` as doctests, so the crates.io
/// landing page can never drift from the real API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
