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

mod analytics;
mod bom;
mod carts;
mod commerce;
mod currency;
mod customers;
mod fulfillment;
mod inventory;
mod invoices;
mod lots;
mod orders;
mod payments;
mod products;
mod promotions;
mod purchase_orders;
mod quality;
mod receiving;
mod returns;
mod serials;
mod shipments;
mod subscriptions;
mod tax;
mod warehouse;
mod warranties;
mod work_orders;
mod accounts_payable;
mod accounts_receivable;
mod cost_accounting;
mod credit;
mod backorder;
mod general_ledger;

#[cfg(feature = "postgres")]
mod async_commerce;

#[cfg(feature = "events")]
pub mod events;

// Event system types (feature-gated)
#[cfg(feature = "events")]
pub use events::{
    EventBus, EventConfig, EventReceiver, EventSubscription, EventSystem,
    Webhook, WebhookConfig, WebhookDelivery, WebhookManager,
    FilteredSubscription, InMemoryEventStore, filters,
};

#[cfg(all(feature = "events", feature = "sqlite-events"))]
pub use events::SqliteEventStore;

pub use analytics::Analytics;
pub use bom::Bom;
pub use carts::Carts;
pub use commerce::{Commerce, CommerceBuilder};
pub use currency::CurrencyOps;
pub use customers::Customers;
pub use fulfillment::Fulfillment;
pub use inventory::Inventory;
pub use invoices::Invoices;
pub use lots::Lots;
pub use orders::Orders;
pub use payments::Payments;
pub use products::Products;
pub use promotions::Promotions;
pub use purchase_orders::PurchaseOrders;
pub use quality::Quality;
pub use receiving::Receiving;
pub use returns::Returns;
pub use serials::Serials;
pub use shipments::Shipments;
pub use subscriptions::Subscriptions;
pub use tax::Tax;
pub use warehouse::WarehouseOps;
pub use warranties::Warranties;
pub use work_orders::WorkOrders;
pub use accounts_payable::AccountsPayable;
pub use accounts_receivable::AccountsReceivable;
pub use cost_accounting::CostAccounting;
pub use credit::Credit;
pub use backorder::Backorders;
pub use general_ledger::GeneralLedger;

// Async API for PostgreSQL (feature-gated)
#[cfg(feature = "postgres")]
pub use async_commerce::{
    AsyncAnalytics, AsyncBom, AsyncCarts, AsyncCommerce, AsyncCurrency, AsyncCustomers,
    AsyncInventory, AsyncInvoices, AsyncOrders, AsyncPayments, AsyncProducts, AsyncPurchaseOrders,
    AsyncReturns, AsyncShipments, AsyncWarranties, AsyncWorkOrders,
};

// Re-export Database trait for advanced users who want to bring their own database
pub use stateset_db::Database;

// Re-export core types for convenience
pub use stateset_core::{
    // Errors
    CommerceError,
    Result,
    // Order types
    Address,
    CreateOrder,
    CreateOrderItem,
    FulfillmentStatus,
    Order,
    OrderFilter,
    OrderItem,
    OrderStatus,
    PaymentStatus,
    UpdateOrder,
    // Inventory types
    AdjustInventory,
    CreateInventoryItem,
    InventoryBalance,
    InventoryFilter,
    InventoryItem,
    InventoryReservation,
    InventoryTransaction,
    LocationStock,
    ReservationStatus,
    ReserveInventory,
    StockLevel,
    TransactionType,
    // Customer types
    AddressType,
    CreateCustomer,
    CreateCustomerAddress,
    Customer,
    CustomerAddress,
    CustomerFilter,
    CustomerStatus,
    UpdateCustomer,
    // Product types
    CreateProduct,
    CreateProductVariant,
    Product,
    ProductAttribute,
    ProductFilter,
    ProductStatus,
    ProductType,
    ProductVariant,
    SeoMetadata,
    UpdateProduct,
    VariantOption,
    // Return types
    CreateReturn,
    CreateReturnItem,
    ItemCondition,
    Return,
    ReturnFilter,
    ReturnItem,
    ReturnReason,
    ReturnStatus,
    UpdateReturn,
    // Manufacturing - BOM types
    BillOfMaterials,
    BomComponent,
    BomFilter,
    BomStatus,
    CreateBom,
    CreateBomComponent,
    UpdateBom,
    // Manufacturing - Work Order types
    AddWorkOrderMaterial,
    CreateWorkOrder,
    CreateWorkOrderTask,
    TaskStatus,
    UpdateWorkOrder,
    UpdateWorkOrderTask,
    WorkOrder,
    WorkOrderFilter,
    WorkOrderMaterial,
    WorkOrderPriority,
    WorkOrderStatus,
    WorkOrderTask,
    // Shipment types
    AddShipmentEvent,
    CreateShipment,
    CreateShipmentItem,
    Shipment,
    ShipmentEvent,
    ShipmentFilter,
    ShipmentItem,
    ShipmentStatus,
    ShippingCarrier,
    ShippingMethod,
    UpdateShipment,
    // Payment types
    CardBrand,
    CreatePayment,
    CreatePaymentMethod,
    CreateRefund,
    Payment,
    PaymentFilter,
    PaymentMethod,
    PaymentMethodType,
    PaymentTransactionStatus,
    Refund,
    RefundStatus,
    UpdatePayment,
    generate_payment_number,
    generate_refund_number,
    // Warranty types
    ClaimResolution,
    ClaimStatus,
    CreateWarranty,
    CreateWarrantyClaim,
    UpdateWarranty,
    UpdateWarrantyClaim,
    Warranty,
    WarrantyClaim,
    WarrantyClaimFilter,
    WarrantyFilter,
    WarrantyStatus,
    WarrantyType,
    generate_warranty_number,
    generate_claim_number,
    // Purchase Order types
    CreatePurchaseOrder,
    CreatePurchaseOrderItem,
    CreateSupplier,
    PaymentTerms,
    PurchaseOrder,
    PurchaseOrderFilter,
    PurchaseOrderItem,
    PurchaseOrderStatus,
    ReceivePurchaseOrderItems,
    Supplier,
    SupplierFilter,
    UpdatePurchaseOrder,
    UpdateSupplier,
    generate_po_number,
    // Invoice types
    CreateInvoice,
    CreateInvoiceItem,
    Invoice,
    InvoiceFilter,
    InvoiceItem,
    InvoiceStatus,
    InvoiceType,
    RecordInvoicePayment,
    UpdateInvoice,
    generate_invoice_number,
    // Cart/Checkout types
    AddCartItem,
    ApplyCartDiscount,
    Cart,
    CartAddress,
    CartFilter,
    CartItem,
    CartPaymentStatus,
    CartStatus,
    CheckoutResult,
    CreateCart,
    FulfillmentType,
    SetCartPayment,
    SetCartShipping,
    ShippingRate,
    UpdateCart,
    UpdateCartItem,
    // Events
    CommerceEvent,
    // Analytics types
    AnalyticsQuery,
    CustomerMetrics,
    DateRange,
    DemandForecast,
    FulfillmentMetrics,
    InventoryHealth,
    InventoryMovement,
    LowStockItem,
    OrderStatusBreakdown,
    ProductPerformance,
    ReturnMetrics,
    ReturnReasonCount,
    RevenueByPeriod,
    RevenueForecast,
    SalesSummary,
    TimeGranularity,
    TimePeriod,
    TopCustomer,
    TopProduct,
    TopReturnedProduct,
    Trend,
    // Currency types
    ConversionResult,
    ConvertCurrency,
    Currency,
    ExchangeRate,
    ExchangeRateFilter,
    Money,
    RoundingMode,
    SetExchangeRate,
    StoreCurrencySettings,
    // Tax types
    CanadianTaxInfo,
    CreateTaxExemption,
    CreateTaxJurisdiction,
    CreateTaxRate,
    EuVatInfo,
    ExemptionType,
    JurisdictionLevel,
    JurisdictionSummary,
    LineItemTax,
    ProductTaxCategory,
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
    UsStateTaxInfo,
    get_canadian_tax_info,
    get_eu_vat_info,
    get_us_state_tax_info,
    is_eu_member,
    // Promotion types
    AppliedPromotion,
    ApplyPromotionsRequest,
    ApplyPromotionsResult,
    ConditionOperator,
    ConditionType,
    CouponCode,
    CouponFilter,
    CouponStatus,
    CreateCouponCode,
    CreatePromotion,
    CreatePromotionCondition,
    DiscountTier,
    LineItemDiscount,
    Promotion,
    PromotionCondition,
    PromotionFilter,
    PromotionLineItem,
    PromotionStatus,
    PromotionTarget,
    PromotionTrigger,
    PromotionType,
    PromotionUsage,
    RejectedPromotion,
    RejectionReason,
    StackingBehavior,
    UpdatePromotion,
    generate_coupon_code,
    generate_promotion_code,
    // Subscription types
    BillingCycle,
    BillingCycleFilter,
    BillingCycleStatus,
    BillingInterval,
    CancelSubscription,
    CreateBillingCycle,
    CreateSubscription,
    CreateSubscriptionItem,
    CreateSubscriptionPlan,
    CreateSubscriptionPlanItem,
    PauseSubscription,
    PlanStatus,
    SkipBillingCycle,
    Subscription,
    SubscriptionEvent,
    SubscriptionEventType,
    SubscriptionFilter,
    SubscriptionItem,
    SubscriptionPlan,
    SubscriptionPlanFilter,
    SubscriptionPlanItem,
    SubscriptionStatus,
    UpdateSubscription,
    UpdateSubscriptionPlan,
    generate_plan_code,
    generate_subscription_number,
    // Quality Control types
    CompleteInspection,
    CreateDefectCode,
    CreateInspection,
    CreateInspectionItem,
    CreateNcr,
    CreateNonConformance,
    CreateQualityHold,
    DefectCode,
    Disposition,
    HoldType,
    Inspection,
    InspectionFilter,
    InspectionItem,
    InspectionResult,
    InspectionStatus,
    InspectionType,
    NonConformance,
    NonConformanceFilter,
    NonConformanceSource,
    NcrStatus,
    QualityHold,
    QualityHoldFilter,
    RecordInspectionResult,
    ReleaseQualityHold,
    Severity,
    UpdateInspection,
    UpdateNonConformance,
    // Lot/Batch Tracking types
    AddLotCertificate,
    AdjustLot,
    CertificateType,
    ConsumeLot,
    CreateLot,
    Lot,
    LotCertificate,
    LotFilter,
    LotLocation,
    LotStatus,
    LotTransaction,
    LotTransactionType,
    MergeLots,
    ReserveLot,
    SplitLot,
    TraceabilityResult,
    TraceNode,
    TraceNodeType,
    TransferLot,
    UpdateLot,
    // Serial Number types
    ChangeSerialStatus,
    CreateSerial,
    CreateSerialNumber,
    CreateSerialNumbersBulk,
    MoveSerial,
    ReserveSerialNumber,
    SerialEventType,
    SerialFilter,
    SerialHistory,
    SerialHistoryFilter,
    SerialLookupResult,
    SerialNumber,
    SerialReservation,
    SerialStatus,
    SerialValidation,
    TransferSerialOwnership,
    UpdateSerialNumber,
    WarrantyLookupStatus,
    // Warehouse & Location types
    AdjustLocationInventory,
    CreateLocation,
    CreateWarehouse,
    CreateWarehouseLocation,
    CreateZone,
    Location,
    LocationFilter,
    LocationInventory,
    LocationInventoryFilter,
    LocationMovement,
    LocationType,
    MoveInventory,
    MovementFilter,
    MovementType,
    UpdateLocation,
    UpdateWarehouse,
    UpdateZone,
    Warehouse,
    WarehouseAddress,
    WarehouseFilter,
    WarehouseType,
    Zone,
    // Receiving types
    CompletePutAway,
    CreatePutAway,
    CreateReceipt,
    CreateReceiptItem,
    CreateReceiptLine,
    PutAway,
    PutAwayFilter,
    PutAwayStatus,
    Receipt,
    ReceiptFilter,
    ReceiptItem,
    ReceiptItemStatus,
    ReceiptStatus,
    ReceiptType,
    ReceiveItems,
    ReceiveItemLine,
    UpdateReceipt,
    // Fulfillment types
    AddCarton,
    AddCartonItem,
    Carton,
    CartonItem,
    CompletePick,
    CompleteShip,
    CreatePackTask,
    CreatePickTask,
    CreateShipTask,
    CreateWave,
    PackStatus,
    PackTask,
    PackTaskFilter,
    PackageType,
    PickStatus,
    PickTask,
    PickTaskFilter,
    ShipStatus,
    ShipTask,
    ShipTaskFilter,
    Wave,
    WaveFilter,
    WaveStatus,
    WaveType,
    // Accounts Payable types
    ApAgingSummary,
    Bill,
    BillFilter,
    BillItem,
    BillPayment,
    BillPaymentFilter,
    BillStatus,
    CreateBill,
    CreateBillItem,
    CreateBillPayment,
    CreatePaymentRun,
    PayBill,
    PaymentAllocation,
    PaymentAllocationInput,
    PaymentMethodAP,
    PaymentRun,
    PaymentRunFilter,
    PaymentRunStatus,
    PaymentStatusAP,
    SupplierApSummary,
    UpdateBill,
    generate_bill_number,
    generate_ap_payment_number,
    generate_payment_run_number,
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
    CreateCostAdjustment,
    CreateCostLayer,
    InventoryValuation,
    IssueCostLayers,
    ItemCost,
    ItemCostFilter,
    RecordCostVariance,
    SetItemCost,
    SkuCostSummary,
    VarianceType,
    generate_cost_adjustment_number,
    // Credit types
    CreateCreditAccount,
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
    CreditTransaction,
    CreditTransactionFilter,
    CreditTransactionType,
    CustomerCreditSummary,
    PlaceCreditHold,
    RecordCreditTransaction,
    ReleaseCreditHold,
    ReviewCreditApplication,
    RiskRating,
    SubmitCreditApplication,
    UpdateCreditAccount,
    generate_credit_application_number,
    // Backorder types
    AllocateBackorder,
    AllocationStatus,
    Backorder,
    BackorderAllocation,
    BackorderFilter,
    BackorderFulfillment,
    BackorderPriority,
    BackorderStatus,
    BackorderSummary,
    CreateBackorder,
    FulfillBackorder,
    FulfillmentSourceType,
    SkuBackorderSummary,
    UpdateBackorder,
    generate_backorder_number,
    // Accounts Receivable types
    ApplyCreditMemo,
    ApplyPaymentToInvoices,
    ArAgingFilter,
    ArAgingSummary,
    ArPaymentApplication,
    CollectionActivity,
    CollectionActivityFilter,
    CollectionActivityType,
    CollectionStatus,
    CreateCollectionActivity,
    CreateCreditMemo,
    CreateWriteOff,
    CreditMemo,
    CreditMemoFilter,
    CreditMemoReason,
    CreditMemoStatus,
    CustomerArAging,
    CustomerArSummary,
    CustomerStatement,
    DunningLetterType,
    GenerateStatementRequest,
    PaymentApplicationLine,
    AgingBucket,
    CreditMemoApplication,
    StatementLineItem,
    StatementTransactionType,
    WriteOff,
    WriteOffFilter,
    WriteOffReason,
    generate_credit_memo_number,
    generate_write_off_number,
    // General Ledger types
    AccountStatus,
    AccountSubType,
    AccountType,
    AutoPostingConfig,
    BalanceSheet,
    BalanceSheetLine,
    BalanceSide,
    BatchResult,
    CreateAutoPostingConfig,
    CreateGlAccount,
    CreateGlPeriod,
    CreateJournalEntry,
    CreateJournalEntryLine,
    GlAccount,
    GlAccountFilter,
    GlPeriod,
    GlPeriodFilter,
    IncomeStatement,
    IncomeStatementLine,
    JournalEntry,
    JournalEntryFilter,
    JournalEntryLine,
    JournalEntrySource,
    JournalEntryStatus,
    JournalEntryType,
    PeriodStatus,
    TrialBalance,
    TrialBalanceLine,
    UpdateGlAccount,
    generate_journal_entry_number,
    // Validation utilities
    validate_email,
    validate_sku,
    validate_phone,
    validate_currency_code,
    validate_postal_code,
    validate_quantity,
    validate_price,
    // x402 Payment Protocol types
    CreateX402PaymentIntent,
    SignX402PaymentIntent,
    X402Asset,
    X402BatchStatus,
    X402IntentStatus,
    X402Network,
    X402PaymentBatch,
    X402PaymentIntent,
    X402PaymentIntentFilter,
    X402PaymentReceipt,
    X402PaymentRequired,
    X402_DEFAULT_VALIDITY_SECONDS,
    X402_DOMAIN_SEPARATOR,
    X402_MAX_VALIDITY_SECONDS,
    X402_VERSION,
    from_smallest_unit,
    generate_x402_intent_id,
    to_smallest_unit,
};
