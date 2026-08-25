//! Repository traits for data access abstraction.
//!
//! This module defines the interface for data persistence. Implementations
//! can be SQLite, PostgreSQL, in-memory, etc.
//!
//! # Generic Repository
//!
//! The [`Repository`] trait provides a generic CRUD interface using associated
//! types. Domain-specific traits (e.g. [`OrderRepository`]) extend it with
//! entity-specific operations.
//!
//! # Auto-impl
//!
//! Key traits use [`auto_impl`](https://docs.rs/auto_impl) so that `&T`,
//! `Box<T>`, and `Arc<T>` automatically implement the trait when `T` does.
//!
//! # Layout
//!
//! The domain-specific traits live in private thematic submodules and are re-exported
//! here, so `stateset_core::traits::OrderRepository` (and friends) remain the
//! canonical paths.

pub mod repository;

mod agentic;
mod analytics;
mod catalog;
mod customers;
mod events;
mod finance;
mod fulfillment;
mod inventory;
mod manufacturing;
mod orders;
mod payments;
mod platform;
mod purchasing;
mod returns;
mod warehouse;

// The `Transactional` trait is defined in `repository.rs` and re-exported
// from this module via `pub use repository::Transactional;`.
pub use repository::{Repository, Transactional};

pub use agentic::{
    A2ACommerceRepository, AgentCardRepository, AgentIdentityRepository, AgentReputationRepository,
    AgentValidationRepository, X402CreditRepository, X402PaymentIntentRepository,
};
pub use analytics::{AnalyticsRepository, VectorRepository};
pub use catalog::{
    ChannelRepository, PriceLevelRepository, PriceScheduleRepository, ProductRepository,
    PromotionRepository, SearchConfigRepository, UnitOfMeasureRepository,
};
pub use customers::{
    CompanyRepository, CustomerRepository, GiftCardRepository, LoyaltyProgramRepository,
    ReviewRepository, RewardRepository, SegmentRepository, StoreCreditRepository,
    WishlistRepository,
};
pub use events::EventHandler;
pub use finance::{
    AccountsPayableRepository, AccountsReceivableRepository, CostAccountingRepository,
    CreditRepository, FixedAssetRepository, GeneralLedgerRepository, InvoiceRepository,
    RevenueRecognitionRepository, VendorCreditRepository,
};
pub use fulfillment::{
    FulfillmentRepository, PrintStationRepository, ShipmentRepository, ShippingZoneRepository,
    ZoneShippingMethodRepository,
};
pub use inventory::{
    InventoryRepository, LotRepository, SerialRepository, StockSnapshotRepository,
};
pub use manufacturing::{
    BomRepository, ProductionBatchRepository, QualityRepository, WorkOrderRepository,
};
pub use orders::{BackorderRepository, CartRepository, OrderRepository};
pub use payments::{
    CurrencyRepository, FraudRepository, PaymentObligationRepository, PaymentRepository,
    PrepaymentRepository, SubscriptionRepository, TaxRepository,
};
pub use platform::{
    ActivityLogRepository, CustomObjectRepository, EdiDocumentRepository,
    IntegrationFieldMappingRepository, IntegrationMappingRepository, PurgatoryRepository,
    TopologySnapshotRepository,
};
pub use purchasing::{PurchaseOrderRepository, SupplierSkuRepository, VendorReturnRepository};
pub use returns::{ReturnRepository, WarrantyRepository};
pub use warehouse::{
    BinRepository, InboundShipmentRepository, ReceivingRepository, TransferOrderRepository,
    WarehouseRepository,
};

// Shared imports for every trait module in this directory —
// `pub(crate)` so submodules can pull the whole prelude via `use super::*`.

pub(crate) use crate::errors::{BatchResult, Result};
pub(crate) use crate::models::{
    A2APurchase, A2APurchaseFilter, AddCartItem, AddCarton, AddCartonItem, AddLotCertificate,
    AddShipmentEvent, AddWishlistItem, AddWorkOrderMaterial, AddressType, AdjustInventory,
    AdjustLocationInventory, AdjustLot, AdjustPoints, AdjustStoreCredit, AgentCard,
    AgentCardFilter, AgentFeedback, AgentFeedbackFilter, AgentFeedbackResponse, AgentIdentity,
    AgentIdentityFilter, AgentMetadataEntry, AgentValidationRequest, AgentValidationResponse,
    AgentValidationStatus, AgentWalletProofType, AllocateBackorder, AnalyticsQuery, ApAgingSummary,
    ApplyCreditMemo, ApplyPaymentToInvoices, ApplyPromotionsRequest, ApplyPromotionsResult,
    ArAgingFilter, ArAgingSummary, ArPaymentApplication, AutoPostingConfig, Backorder,
    BackorderAllocation, BackorderFilter, BackorderFulfillment, BackorderSummary, BalanceSheet,
    Bill, BillFilter, BillItem, BillOfMaterials, BillPayment, BillPaymentFilter, BillingCycle,
    BillingCycleFilter, BillingCycleStatus, BomComponent, BomFilter, CancelSubscription, Cart,
    CartAddress, CartFilter, CartItem, Carton, CartonItem, ChangeSerialStatus, CheckoutResult,
    ClaimResolution, CollectionActivity, CollectionActivityFilter, CollectionStatus, CompletePick,
    CompletePutAway, CompleteShip, ConsumeLot, ConversionResult, ConvertCurrency, CostAdjustment,
    CostAdjustmentFilter, CostLayer, CostLayerFilter, CostMethod, CostRollup, CostTransaction,
    CostTransactionFilter, CostTransactionType, CostVariance, CostVarianceFilter, CouponCode,
    CouponFilter, CreateA2APurchase, CreateA2AQuote, CreateAgentCard, CreateAgentFeedback,
    CreateAgentFeedbackResponse, CreateAgentIdentity, CreateAgentValidationRequest,
    CreateAgentValidationResponse, CreateAutoPostingConfig, CreateBackorder, CreateBill,
    CreateBillItem, CreateBillPayment, CreateBillingCycle, CreateBom, CreateBomComponent,
    CreateCart, CreateCollectionActivity, CreateCostAdjustment, CreateCostLayer, CreateCouponCode,
    CreateCreditAccount, CreateCreditMemo, CreateCustomObject, CreateCustomObjectType,
    CreateCustomer, CreateCustomerAddress, CreateCycleCount, CreateDefectCode,
    CreateFraudAssessment, CreateFraudRule, CreateGiftCard, CreateGlAccount, CreateGlPeriod,
    CreateInspection, CreateInventoryItem, CreateInvoice, CreateInvoiceItem, CreateJournalEntry,
    CreateLocation, CreateLot, CreateLoyaltyProgram, CreateNonConformance, CreateOrder,
    CreateOrderItem, CreatePackTask, CreatePayment, CreatePaymentMethod, CreatePaymentRun,
    CreatePickTask, CreateProduct, CreateProductVariant, CreatePromotion, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreatePutAway, CreateQualityHold, CreateReceipt, CreateRefund,
    CreateReturn, CreateReview, CreateReward, CreateSearchConfig, CreateSegment,
    CreateSerialNumber, CreateSerialNumbersBulk, CreateShipTask, CreateShipment,
    CreateShipmentItem, CreateShippingZone, CreateStoreCredit, CreateSubscription,
    CreateSubscriptionPlan, CreateSupplier, CreateTaxExemption, CreateTaxJurisdiction,
    CreateTaxRate, CreateWarehouse, CreateWarranty, CreateWarrantyClaim, CreateWave,
    CreateWishlist, CreateWorkOrder, CreateWorkOrderTask, CreateWriteOff, CreateX402PaymentIntent,
    CreateZone, CreateZoneShippingMethod, CreditAccount, CreditAccountFilter, CreditAgingBucket,
    CreditApplication, CreditApplicationFilter, CreditCheckResult, CreditHold, CreditHoldFilter,
    CreditMemo, CreditMemoFilter, CreditTransaction, CreditTransactionFilter, Currency,
    CustomObject, CustomObjectFilter, CustomObjectType, CustomObjectTypeFilter, Customer,
    CustomerAddress, CustomerArAging, CustomerArSummary, CustomerCreditSummary, CustomerFilter,
    CustomerMetrics, CustomerStatement, CycleCount, CycleCountFilter, DefectCode, DemandForecast,
    DunningLetterType, EmbeddingMetadata, EmbeddingStats, EnrollCustomer, EntityType, ExchangeRate,
    ExchangeRateFilter, FeedbackSummary, FraudAssessment, FraudAssessmentFilter, FraudDecision,
    FraudRule, FraudRuleFilter, FulfillBackorder, FulfillmentMetrics, GenerateStatementRequest,
    GiftCard, GiftCardFilter, GiftCardTransaction, GlAccount, GlAccountFilter, GlPeriod,
    GlPeriodFilter, IncomeStatement, Inspection, InspectionFilter, InspectionItem,
    InventoryBalance, InventoryFilter, InventoryHealth, InventoryItem, InventoryMovement,
    InventoryReservation, InventoryTransaction, InventoryValuation, Invoice, InvoiceFilter,
    InvoiceItem, IssueCostLayers, ItemCost, ItemCostFilter, JournalEntry, JournalEntryFilter,
    JournalEntryLine, Location, LocationFilter, LocationInventory, LocationInventoryFilter,
    LocationMovement, Lot, LotCertificate, LotFilter, LotLocation, LotTransaction, LowStockItem,
    LoyaltyAccount, LoyaltyAccountFilter, LoyaltyProgram, LoyaltyTransaction, MergeLots,
    MoveInventory, MoveSerial, MovementFilter, NonConformance, NonConformanceFilter, Order,
    OrderFilter, OrderItem, OrderStatusBreakdown, PackTask, PackTaskFilter, PauseSubscription,
    Payment, PaymentAllocation, PaymentFilter, PaymentMethod, PaymentRun, PaymentRunFilter,
    PickTask, PickTaskFilter, PlaceCreditHold, Product, ProductFilter, ProductPerformance,
    ProductTaxCategory, ProductVariant, Promotion, PromotionFilter, PromotionUsage, PurchaseOrder,
    PurchaseOrderFilter, PurchaseOrderItem, PurchaseStatus, PutAway, PutAwayFilter, QualityHold,
    QualityHoldFilter, QuoteStatus, Receipt, ReceiptFilter, ReceiptItem, ReceiveItems,
    ReceivePurchaseOrderItems, RecordCostVariance, RecordCreditTransaction, RecordCycleCountLine,
    RecordInspectionResult, RecordInvoicePayment, Refund, ReleaseCreditHold, ReleaseQualityHold,
    ReserveInventory, ReserveLot, ReserveSerialNumber, Return, ReturnFilter, ReturnMetrics,
    RevaluationResult, RevenueByPeriod, RevenueForecast, Review, ReviewCreditApplication,
    ReviewFilter, ReviewSummary, Reward, RewardFilter, SalesSummary, SearchConfig,
    SearchConfigFilter, Segment, SegmentFilter, SegmentMembership, SerialFilter, SerialHistory,
    SerialHistoryFilter, SerialLookupResult, SerialNumber, SerialReservation, SerialValidation,
    SetCartPayment, SetCartShipping, SetCartX402Payment, SetExchangeRate, SetItemCost, ShipOrder,
    ShipTask, ShipTaskFilter, Shipment, ShipmentEvent, ShipmentFilter, ShipmentItem, ShippingRate,
    ShippingZone, ShippingZoneFilter, SignX402PaymentIntent, SkillQuote, SkillQuoteFilter,
    SkipBillingCycle, SkuBackorderSummary, SkuCostSummary, SplitLot, StockLevel, StoreCredit,
    StoreCreditFilter, StoreCreditTransaction, StoreCurrencySettings, SubmitCreditApplication,
    Subscription, SubscriptionEvent, SubscriptionEventType, SubscriptionFilter, SubscriptionPlan,
    SubscriptionPlanFilter, Supplier, SupplierApSummary, SupplierFilter, TaxAddress,
    TaxCalculationRequest, TaxCalculationResult, TaxExemption, TaxJurisdiction,
    TaxJurisdictionFilter, TaxRate, TaxRateFilter, TaxSettings, TimeGranularity, TopCustomer,
    TopProduct, TraceabilityResult, TransferLot, TransferSerialOwnership, TrialBalance, TrustLevel,
    UpdateAgentCard, UpdateAgentIdentity, UpdateBackorder, UpdateBill, UpdateBom, UpdateCart,
    UpdateCartItem, UpdateCreditAccount, UpdateCustomObject, UpdateCustomObjectType,
    UpdateCustomer, UpdateFraudRule, UpdateGiftCard, UpdateGlAccount, UpdateInspection,
    UpdateInvoice, UpdateLocation, UpdateLot, UpdateNonConformance, UpdateOrder, UpdatePayment,
    UpdateProduct, UpdatePromotion, UpdatePurchaseOrder, UpdateReceipt, UpdateReturn, UpdateReview,
    UpdateSearchConfig, UpdateSegment, UpdateSerialNumber, UpdateShipment, UpdateShippingZone,
    UpdateSubscription, UpdateSubscriptionPlan, UpdateSupplier, UpdateWarehouse, UpdateWarranty,
    UpdateWarrantyClaim, UpdateWishlist, UpdateWorkOrder, UpdateWorkOrderTask, UpdateZone,
    ValidationSummary, VectorSearchResult, Warehouse, WarehouseFilter, Warranty, WarrantyClaim,
    WarrantyClaimFilter, WarrantyFilter, Wave, WaveFilter, Wishlist, WishlistFilter, WishlistItem,
    WorkOrder, WorkOrderFilter, WorkOrderMaterial, WorkOrderTask, WriteOff, WriteOffFilter,
    X402Asset, X402CheckoutResult, X402CreditAccount, X402CreditAdjustment, X402CreditTransaction,
    X402CreditTransactionFilter, X402Network, X402PaymentIntent, X402PaymentIntentFilter, Zone,
    ZoneShippingMethod, ZoneShippingMethodFilter, ZoneShippingRate, ZoneShippingRateRequest,
};
pub(crate) use crate::models::{
    AdjustBinLevel, BinLevel, BinMovement, BinReconciliation, CreateWarehouseBin, MoveBetweenBins,
    ReturnItem, SetReturnDisposition, UpdateWarehouseBin, WarehouseBin, WarehouseBinFilter,
};
// Newly-added B2B / ERP-ops entities (channels, companies, transfer orders,
// units of measure, production batches).
pub(crate) use crate::models::{
    ActivityLogEntry, ActivityLogFilter, ApplyPrepayment, ApplyVendorCredit, BulkSupplierSkuItem,
    CaptureStockSnapshot, CaptureTopologySnapshot, CreateEdiDocument, CreateInboundShipment,
    CreateIntegrationFieldMapping, CreateIntegrationMapping, CreatePaymentObligation,
    CreatePrepayment, CreatePriceLevel, CreatePriceSchedule, CreatePrintStation, CreateSupplierSku,
    CreateVendorCredit, CreateVendorReturn, EdiAggregateSummary, EdiDocument, EdiDocumentFilter,
    EdiStatus, EnqueuePrintJob, InboundShipment, InboundShipmentFilter, IngestOrder,
    IntegrationFieldMapping, IntegrationFieldMappingFilter, IntegrationMapping,
    IntegrationMappingFilter, MapPurgatoryLine, MappingLookup, PairStationResult,
    PaymentObligation, PaymentObligationDashboard, PaymentObligationFilter,
    PaymentObligationStatus, Prepayment, PrepaymentApplication, PrepaymentFilter, PriceLevel,
    PriceLevelEntry, PriceLevelFilter, PriceSchedule, PriceScheduleEntry, PriceScheduleFilter,
    PrintJob, PrintJobFilter, PrintStation, PurgatoryFilter, PurgatoryOrder, RecordActivity,
    StockSnapshot, StockSnapshotFilter, SupplierSku, SupplierSkuFilter, TopologySnapshot,
    TopologySnapshotFilter, UpdateIntegrationFieldMapping, UpdateIntegrationMapping,
    UpdatePriceLevel, UpdatePriceSchedule, UpdateSupplierSku, VendorCredit,
    VendorCreditApplication, VendorCreditFilter, VendorReturn, VendorReturnFilter,
};
pub(crate) use crate::models::{
    Channel, ChannelFilter, ChannelProductMapping, ChannelProductSyncItem, Company, CompanyFilter,
    CompanyPriceOverride, CompanyShippingAddress, Contact, CreateChannel, CreateCompany,
    CreateContact, CreateProductionBatch, CreateTransferOrder, CreateUnitClass,
    CreateUnitConversionRule, CreateUnitOfMeasure, ProductionBatch, ProductionBatchFilter,
    TransferOrder, TransferOrderFilter, UnitClass, UnitConversionRule, UnitOfMeasure,
    UnitOfMeasureFilter, UpdateChannel, UpdateCompany, UpdateProductionBatch,
};
pub(crate) use crate::models::{
    CreateFixedAsset, CreateRevenueContract, DepreciationSchedule, FixedAsset, FixedAssetFilter,
    PerformanceObligation, RevenueContract, RevenueContractFilter, RevenueSchedule,
    UpdateFixedAsset, UpdateRevenueContract,
};
pub(crate) use chrono::NaiveDate;
pub(crate) use chrono::{DateTime, Utc};
pub(crate) use stateset_primitives::{
    ActivityLogId, ChannelId, CompanyId, ContactId, EdiDocumentId, InboundShipmentId,
    InboundShipmentItemId, IntegrationFieldMappingId, IntegrationMappingId, PaymentObligationId,
    PrepaymentApplicationId, PrepaymentId, PriceLevelId, PriceScheduleId, PrintJobId,
    PrintStationId, ProductionBatchId, PurgatoryLineItemId, PurgatoryOrderId, StockSnapshotId,
    SupplierSkuId, TopologySnapshotId, TransferOrderId, TransferOrderItemId, UnitClassId,
    UnitConversionRuleId, UnitOfMeasureId, VendorCreditApplicationId, VendorCreditId,
    VendorReturnId,
};
pub(crate) use stateset_primitives::{
    CartId, CreditId, CustomerId, FraudRuleId, FulfillmentId, GiftCardId, InvoiceId,
    LoyaltyAccountId, LoyaltyProgramId, OrderId, OrderItemId, PaymentId, ProductId, PromotionId,
    PurchaseOrderId, ReturnId, ReviewId, RewardId, SearchConfigId, SegmentId, ShipmentId,
    ShippingMethodId, ShippingZoneId, StoreCreditId, SubscriptionId, WarrantyId, WishlistId,
};
pub(crate) use uuid::Uuid;
