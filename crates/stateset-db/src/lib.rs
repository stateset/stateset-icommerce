#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet DB
//!
//! Database implementations for StateSet iCommerce.
//!
//! ## Features
//!
//! - `sqlite` (default): SQLite database support via rusqlite
//! - `postgres`: PostgreSQL database support via sqlx (async)
//! - `vector`: Vector search support via sqlite-vec extension
//! - `saga`: Experimental persisted saga coordinator (PostgreSQL-only)
//!
//! ## Usage
//!
//! ### SQLite (default)
//! ```ignore
//! use stateset_db::{SqliteDatabase, DatabaseConfig};
//! let db = SqliteDatabase::new(&DatabaseConfig::sqlite("./store.db"))?;
//! ```
//!
//! ### PostgreSQL
//! ```ignore
//! use stateset_db::{PostgresDatabase, DatabaseConfig};
//! let db = PostgresDatabase::connect(&DatabaseConfig::postgres("postgres://localhost/stateset")).await?;
//! ```
//!
//! ## Error Handling
//!
//! This crate uses typed errors via `stateset_core::DbError` for better
//! debugging and error categorization. Use the error helper functions
//! in the `error_helpers` module for converting backend-specific errors.

pub mod error_helpers;
pub mod http_idempotency;
pub use http_idempotency::{HttpIdempotencyRecord, HttpIdempotencyRepository};

#[cfg(feature = "sqlite")]
pub mod audit;
#[cfg(feature = "sqlite")]
pub mod backup;
#[cfg(feature = "sqlite")]
pub mod maintenance;
#[cfg(feature = "sqlite")]
pub mod migrations;
pub mod portability;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(all(feature = "postgres", feature = "saga"))]
pub mod saga;

pub mod transactions;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDatabase;

#[cfg(feature = "postgres")]
pub use postgres::PostgresDatabase;

use stateset_core::{
    A2ACommerceRepository, AccountsPayableRepository, AccountsReceivableRepository,
    ActivityLogRepository, AgentCardRepository, AgentIdentityRepository, AgentReputationRepository,
    AgentValidationRepository, AnalyticsRepository, BackorderRepository, BomRepository,
    CartRepository, ChannelRepository, CommerceError, CompanyRepository, CostAccountingRepository,
    CreditRepository, CurrencyRepository, CustomObjectRepository, CustomerRepository,
    EdiDocumentRepository, FixedAssetRepository, FraudRepository, FulfillmentRepository,
    GeneralLedgerRepository, GiftCardRepository, InboundShipmentRepository,
    IntegrationFieldMappingRepository, IntegrationMappingRepository, InventoryRepository,
    InvoiceRepository, LotRepository, LoyaltyProgramRepository, OrderRepository,
    PaymentObligationRepository, PaymentRepository, PrepaymentRepository, PriceLevelRepository,
    PriceScheduleRepository, PrintStationRepository, ProductRepository, ProductionBatchRepository,
    PromotionRepository, PurchaseOrderRepository, PurgatoryRepository, QualityRepository,
    ReceivingRepository, Result, ReturnRepository, RevenueRecognitionRepository, ReviewRepository,
    RewardRepository, SearchConfigRepository, SegmentRepository, SerialRepository,
    ShipmentRepository, ShippingZoneRepository, StockSnapshotRepository, StoreCreditRepository,
    SubscriptionRepository, SupplierSkuRepository, TaxRepository, TopologySnapshotRepository,
    TransferOrderRepository, UnitOfMeasureRepository, VendorCreditRepository,
    VendorReturnRepository, WarehouseRepository, WarrantyRepository, WishlistRepository,
    WorkOrderRepository, X402CreditRepository, X402PaymentIntentRepository,
    ZoneShippingMethodRepository,
};

// ============================================================================
// Transaction Support
// ============================================================================

/// Context for operations within a transaction
///
/// This trait provides access to repositories within a transaction scope.
/// All operations performed through the context are part of the same transaction.
pub trait TransactionContext: Send + Sync {
    /// Get the order repository within this transaction
    fn orders(&self) -> Box<dyn OrderRepository + '_>;
    /// Get the inventory repository within this transaction
    fn inventory(&self) -> Box<dyn InventoryRepository + '_>;
    /// Get the customer repository within this transaction
    fn customers(&self) -> Box<dyn CustomerRepository + '_>;
    /// Get the product repository within this transaction
    fn products(&self) -> Box<dyn ProductRepository + '_>;
}

/// Options for transaction execution
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct TransactionOptions {
    /// Timeout for the transaction in milliseconds (default: 30000)
    pub timeout_ms: Option<u64>,
    /// Isolation level for the transaction
    pub isolation: TransactionIsolation,
    /// Whether to retry on transient failures
    pub retry_on_conflict: bool,
    /// Maximum number of retries
    pub max_retries: u32,
}

const DEFAULT_TRANSACTION_TIMEOUT_MS: u64 = 30_000;

impl TransactionOptions {
    /// Create options with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timeout
    pub const fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Set the isolation level
    pub const fn isolation(mut self, level: TransactionIsolation) -> Self {
        self.isolation = level;
        self
    }

    /// Enable retry on conflict
    ///
    /// If enabled, the transaction closure may re-run on transient database failures.
    /// Ensure the closure body is idempotent (or safely handles retry) when enabling this option.
    pub const fn with_retries(mut self, max_retries: u32) -> Self {
        self.retry_on_conflict = true;
        self.max_retries = max_retries;
        self
    }
}

/// Transaction isolation levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransactionIsolation {
    /// Read uncommitted.
    ReadUncommitted,
    /// Read committed.
    ReadCommitted,
    /// Repeatable read.
    RepeatableRead,
    /// Serializable.
    #[default]
    Serializable,
}

/// Optional domain capability exposed by a database backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseCapability {
    GiftCards,
    StoreCredits,
    Segments,
    ShippingZones,
    ZoneShippingMethods,
    Reviews,
    Wishlists,
    LoyaltyPrograms,
    Rewards,
    Fraud,
    SearchConfigs,
    Channels,
    Companies,
    TransferOrders,
    UnitsOfMeasure,
    ProductionBatches,
    SupplierSkus,
    VendorReturns,
    VendorCredits,
    PaymentObligations,
    PriceLevels,
    Prepayments,
    PriceSchedules,
    ActivityLogs,
    IntegrationMappings,
    InboundShipments,
    Purgatory,
    PrintStations,
    EdiDocuments,
    FixedAssets,
    RevenueRecognition,
    IntegrationFieldMappings,
    TopologySnapshots,
    StockSnapshots,
}

impl DatabaseCapability {
    const fn repository_name(self) -> &'static str {
        match self {
            Self::GiftCards => "gift_cards",
            Self::StoreCredits => "store_credits",
            Self::Segments => "segments",
            Self::ShippingZones => "shipping_zones",
            Self::ZoneShippingMethods => "zone_shipping_methods",
            Self::Reviews => "reviews",
            Self::Wishlists => "wishlists",
            Self::LoyaltyPrograms => "loyalty_programs",
            Self::Rewards => "rewards",
            Self::Fraud => "fraud",
            Self::SearchConfigs => "search_configs",
            Self::Channels => "channels",
            Self::Companies => "companies",
            Self::TransferOrders => "transfer_orders",
            Self::UnitsOfMeasure => "units_of_measure",
            Self::ProductionBatches => "production_batches",
            Self::SupplierSkus => "supplier_skus",
            Self::VendorReturns => "vendor_returns",
            Self::VendorCredits => "vendor_credits",
            Self::PaymentObligations => "payment_obligations",
            Self::PriceLevels => "price_levels",
            Self::Prepayments => "prepayments",
            Self::PriceSchedules => "price_schedules",
            Self::ActivityLogs => "activity_logs",
            Self::IntegrationMappings => "integration_mappings",
            Self::InboundShipments => "inbound_shipments",
            Self::Purgatory => "purgatory",
            Self::PrintStations => "print_stations",
            Self::EdiDocuments => "edi_documents",
            Self::FixedAssets => "fixed_assets",
            Self::RevenueRecognition => "revenue_recognition",
            Self::IntegrationFieldMappings => "integration_field_mappings",
            Self::TopologySnapshots => "topology_snapshots",
            Self::StockSnapshots => "stock_snapshots",
        }
    }
}

// ============================================================================
// Database Trait
// ============================================================================

/// Unified database trait that both SQLite and PostgreSQL implement.
/// This allows stateset-embedded to work with either backend.
pub trait Database: Send + Sync {
    /// Human-readable backend name.
    fn backend_name(&self) -> &'static str {
        "external"
    }

    /// Whether the backend supports a given optional repository domain.
    fn supports_capability(&self, _capability: DatabaseCapability) -> bool {
        true
    }

    /// Fail fast when a caller requests an unsupported optional repository domain.
    fn ensure_capability(&self, capability: DatabaseCapability) -> Result<()> {
        if self.supports_capability(capability) {
            Ok(())
        } else {
            Err(CommerceError::NotPermitted(format!(
                "{} repository is not implemented for {} backend",
                capability.repository_name(),
                self.backend_name()
            )))
        }
    }

    /// Get the order repository
    fn orders(&self) -> Box<dyn OrderRepository + '_>;
    /// Get the inventory repository
    fn inventory(&self) -> Box<dyn InventoryRepository + '_>;
    /// Get the customer repository
    fn customers(&self) -> Box<dyn CustomerRepository + '_>;
    /// Get the product repository
    fn products(&self) -> Box<dyn ProductRepository + '_>;
    /// Get the custom objects repository (custom states / metaobjects)
    fn custom_objects(&self) -> Box<dyn CustomObjectRepository + '_>;
    /// Get the return repository
    fn returns(&self) -> Box<dyn ReturnRepository + '_>;
    /// Get the BOM (Bill of Materials) repository
    fn bom(&self) -> Box<dyn BomRepository + '_>;
    /// Get the work order repository
    fn work_orders(&self) -> Box<dyn WorkOrderRepository + '_>;
    /// Get the shipment repository
    fn shipments(&self) -> Box<dyn ShipmentRepository + '_>;
    /// Get the payment repository
    fn payments(&self) -> Box<dyn PaymentRepository + '_>;
    /// Get the warranty repository
    fn warranties(&self) -> Box<dyn WarrantyRepository + '_>;
    /// Get the purchase order repository
    fn purchase_orders(&self) -> Box<dyn PurchaseOrderRepository + '_>;
    /// Get the invoice repository
    fn invoices(&self) -> Box<dyn InvoiceRepository + '_>;
    /// Get the cart/checkout repository
    fn carts(&self) -> Box<dyn CartRepository + '_>;
    /// Get the analytics repository
    fn analytics(&self) -> Box<dyn AnalyticsRepository + '_>;
    /// Get the currency repository
    fn currency(&self) -> Box<dyn CurrencyRepository + '_>;
    /// Get the tax repository
    fn tax(&self) -> Box<dyn TaxRepository + '_>;
    /// Get the promotions repository
    fn promotions(&self) -> Box<dyn PromotionRepository + '_>;
    /// Get the subscriptions repository
    fn subscriptions(&self) -> Box<dyn SubscriptionRepository + '_>;
    /// Get the quality repository
    fn quality(&self) -> Box<dyn QualityRepository + '_>;
    /// Get the lots repository
    fn lots(&self) -> Box<dyn LotRepository + '_>;
    /// Get the serials repository
    fn serials(&self) -> Box<dyn SerialRepository + '_>;
    /// Get the warehouse repository
    fn warehouse(&self) -> Box<dyn WarehouseRepository + '_>;
    /// Get the receiving repository
    fn receiving(&self) -> Box<dyn ReceivingRepository + '_>;
    /// Get the fulfillment repository
    fn fulfillment(&self) -> Box<dyn FulfillmentRepository + '_>;
    /// Get the accounts payable repository
    fn accounts_payable(&self) -> Box<dyn AccountsPayableRepository + '_>;
    /// Get the cost accounting repository
    fn cost_accounting(&self) -> Box<dyn CostAccountingRepository + '_>;
    /// Get the credit repository
    fn credit(&self) -> Box<dyn CreditRepository + '_>;
    /// Get the backorder repository
    fn backorder(&self) -> Box<dyn BackorderRepository + '_>;
    /// Get the accounts receivable repository
    fn accounts_receivable(&self) -> Box<dyn AccountsReceivableRepository + '_>;
    /// Get the general ledger repository
    fn general_ledger(&self) -> Box<dyn GeneralLedgerRepository + '_>;
    /// Get the x402 payment intent repository
    fn x402_payment_intents(&self) -> Box<dyn X402PaymentIntentRepository + '_>;
    /// Get the x402 credit ledger repository
    fn x402_credits(&self) -> Box<dyn X402CreditRepository + '_>;
    /// Get the agent-to-agent commerce repository (quotes and purchases)
    fn a2a_quotes(&self) -> Box<dyn A2ACommerceRepository + '_>;
    /// Get the agent-to-agent commerce repository (quotes and purchases)
    fn a2a_purchases(&self) -> Box<dyn A2ACommerceRepository + '_>;
    /// Get the agent card repository
    fn agent_cards(&self) -> Box<dyn AgentCardRepository + '_>;
    /// Get the agent identity registry repository (ERC-8004)
    fn agent_identities(&self) -> Box<dyn AgentIdentityRepository + '_>;
    /// Get the agent reputation registry repository (ERC-8004)
    fn agent_reputation(&self) -> Box<dyn AgentReputationRepository + '_>;
    /// Get the agent validation registry repository (ERC-8004)
    fn agent_validation(&self) -> Box<dyn AgentValidationRepository + '_>;

    // --- New domain repositories ---

    /// Get the gift card repository
    fn gift_cards(&self) -> Box<dyn GiftCardRepository + '_>;
    /// Get the store credit repository
    fn store_credits(&self) -> Box<dyn StoreCreditRepository + '_>;
    /// Get the customer segment repository
    fn segments(&self) -> Box<dyn SegmentRepository + '_>;
    /// Get the shipping zone repository
    fn shipping_zones(&self) -> Box<dyn ShippingZoneRepository + '_>;
    /// Get the zone shipping method repository
    fn zone_shipping_methods(&self) -> Box<dyn ZoneShippingMethodRepository + '_>;
    /// Get the product review repository
    fn reviews(&self) -> Box<dyn ReviewRepository + '_>;
    /// Get the wishlist repository
    fn wishlists(&self) -> Box<dyn WishlistRepository + '_>;
    /// Get the loyalty program repository
    fn loyalty_programs(&self) -> Box<dyn LoyaltyProgramRepository + '_>;
    /// Get the reward catalog repository
    fn rewards(&self) -> Box<dyn RewardRepository + '_>;
    /// Get the fraud detection repository
    fn fraud(&self) -> Box<dyn FraudRepository + '_>;
    /// Get the search configuration repository
    fn search_configs(&self) -> Box<dyn SearchConfigRepository + '_>;
    /// Get the sales/fulfillment channel repository
    fn channels(&self) -> Box<dyn ChannelRepository + '_>;
    /// Get the B2B company repository
    fn companies(&self) -> Box<dyn CompanyRepository + '_>;
    /// Get the transfer order repository
    fn transfer_orders(&self) -> Box<dyn TransferOrderRepository + '_>;
    /// Get the units-of-measure repository
    fn units_of_measure(&self) -> Box<dyn UnitOfMeasureRepository + '_>;
    /// Get the production batch repository
    fn production_batches(&self) -> Box<dyn ProductionBatchRepository + '_>;
    /// Get the supplier SKU repository
    fn supplier_skus(&self) -> Box<dyn SupplierSkuRepository + '_>;
    /// Get the vendor return repository
    fn vendor_returns(&self) -> Box<dyn VendorReturnRepository + '_>;
    /// Get the vendor credit repository
    fn vendor_credits(&self) -> Box<dyn VendorCreditRepository + '_>;
    /// Get the payment obligation repository
    fn payment_obligations(&self) -> Box<dyn PaymentObligationRepository + '_>;
    /// Get the price level repository
    fn price_levels(&self) -> Box<dyn PriceLevelRepository + '_>;
    /// Get the prepayment repository
    fn prepayments(&self) -> Box<dyn PrepaymentRepository + '_>;
    /// Get the price schedule repository
    fn price_schedules(&self) -> Box<dyn PriceScheduleRepository + '_>;
    /// Get the activity log repository
    fn activity_logs(&self) -> Box<dyn ActivityLogRepository + '_>;
    /// Get the integration mapping repository
    fn integration_mappings(&self) -> Box<dyn IntegrationMappingRepository + '_>;
    /// Get the inbound shipment repository
    fn inbound_shipments(&self) -> Box<dyn InboundShipmentRepository + '_>;
    /// Get the purgatory repository
    fn purgatory(&self) -> Box<dyn PurgatoryRepository + '_>;
    /// Get the print station repository
    fn print_stations(&self) -> Box<dyn PrintStationRepository + '_>;
    /// Get the EDI document repository
    fn edi_documents(&self) -> Box<dyn EdiDocumentRepository + '_>;
    /// Get the integration field-mapping repository
    fn integration_field_mappings(&self) -> Box<dyn IntegrationFieldMappingRepository + '_>;
    /// Get the topology snapshot repository
    fn topology_snapshots(&self) -> Box<dyn TopologySnapshotRepository + '_>;
    /// Get the stock snapshot repository
    fn stock_snapshots(&self) -> Box<dyn StockSnapshotRepository + '_>;
    /// Get the fixed asset repository
    fn fixed_assets(&self) -> Box<dyn FixedAssetRepository + '_>;
    /// Get the revenue recognition repository
    fn revenue_recognition(&self) -> Box<dyn RevenueRecognitionRepository + '_>;
    /// Get the durable HTTP idempotency repository, if the backend provides
    /// one. Backends without durable idempotency support return `None`, in
    /// which case callers fall back to in-memory behavior.
    fn http_idempotency(&self) -> Option<Box<dyn HttpIdempotencyRepository + '_>> {
        None
    }
}

/// Extension trait for database transaction support.
///
/// Provides closure-based transaction management with automatic commit/rollback.
/// Note: This is a simplified transaction API. For complex transactions spanning
/// multiple repositories, use the raw connection approach via `SqliteDatabase::conn()`.
///
/// # Example
/// ```ignore
/// use stateset_db::{SqliteDatabase, DatabaseExt, TransactionOptions};
///
/// let db = SqliteDatabase::in_memory()?;
///
/// // Simple transaction using raw SQL
/// db.with_transaction(|conn| {
///     conn.execute("UPDATE inventory_balances SET quantity_on_hand = 100 WHERE item_id = 1", [])?;
///     conn.execute("INSERT INTO inventory_transactions (...) VALUES (...)", [...])?;
///     Ok(())
/// })?;
///
/// // Transaction with options
/// db.with_transaction_opts(
///     TransactionOptions::new().with_retries(3),
///     |conn| {
///         conn.execute("UPDATE orders SET status = 'completed' WHERE id = ?", [&order_id])?;
///         Ok(())
///     },
/// )?;
/// ```
#[cfg(feature = "sqlite")]
pub trait DatabaseExt {
    /// Execute a closure within a database transaction.
    ///
    /// The transaction is automatically committed if the closure returns `Ok`,
    /// and rolled back if it returns `Err` or panics.
    fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnMut(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>;

    /// Execute a closure within a database transaction with custom options.
    ///
    /// This method allows setting transaction options like timeout and retry behavior.
    fn with_transaction_opts<F, T>(&self, _opts: TransactionOptions, f: F) -> Result<T>
    where
        F: FnMut(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>,
    {
        // Default implementation ignores options and delegates to with_transaction
        self.with_transaction(f)
    }
}

/// Async extension trait for PostgreSQL transaction support.
///
/// # Example
/// ```ignore
/// use stateset_db::{PostgresDatabase, AsyncDatabaseExt, TransactionOptions};
///
/// let db = PostgresDatabase::connect("postgres://localhost/db").await?;
///
/// db.with_transaction_async(|tx| async move {
///     sqlx::query("UPDATE orders SET status = 'completed' WHERE id = $1")
///         .bind(order_id)
///         .execute(&mut *tx)
///         .await?;
///     Ok(())
/// }).await?;
/// ```
#[cfg(feature = "postgres")]
#[allow(async_fn_in_trait)]
pub trait AsyncDatabaseExt {
    /// Execute an async closure within a database transaction.
    ///
    /// The transaction is automatically committed if the closure returns `Ok`,
    /// and rolled back if it returns `Err` or panics.
    /// If `retry_on_conflict` is enabled, the closure may run more than once.
    async fn with_transaction_async<'a, F, T, Fut>(&'a self, f: F) -> Result<T>
    where
        F: FnMut(&mut sqlx::Transaction<'a, sqlx::Postgres>) -> Fut + Send,
        Fut: std::future::Future<Output = std::result::Result<T, sqlx::Error>> + Send,
        T: Send,
    {
        self.with_transaction_async_opts(crate::TransactionOptions::new(), f).await
    }

    /// Execute an async closure within a transaction with custom options.
    ///
    /// If `opts.retry_on_conflict` is enabled, the closure may run multiple times.
    async fn with_transaction_async_opts<'a, F, T, Fut>(
        &'a self,
        opts: TransactionOptions,
        f: F,
    ) -> Result<T>
    where
        F: FnMut(&mut sqlx::Transaction<'a, sqlx::Postgres>) -> Fut + Send,
        Fut: std::future::Future<Output = std::result::Result<T, sqlx::Error>> + Send,
        T: Send;
}

trait NewDomainRepositoryFactory {
    fn gift_cards_repo(&self) -> Box<dyn GiftCardRepository + '_>;
    fn store_credits_repo(&self) -> Box<dyn StoreCreditRepository + '_>;
    fn segments_repo(&self) -> Box<dyn SegmentRepository + '_>;
    fn shipping_zones_repo(&self) -> Box<dyn ShippingZoneRepository + '_>;
    fn zone_shipping_methods_repo(&self) -> Box<dyn ZoneShippingMethodRepository + '_>;
    fn reviews_repo(&self) -> Box<dyn ReviewRepository + '_>;
    fn wishlists_repo(&self) -> Box<dyn WishlistRepository + '_>;
    fn loyalty_programs_repo(&self) -> Box<dyn LoyaltyProgramRepository + '_>;
    fn rewards_repo(&self) -> Box<dyn RewardRepository + '_>;
    fn fraud_repo(&self) -> Box<dyn FraudRepository + '_>;
    fn search_configs_repo(&self) -> Box<dyn SearchConfigRepository + '_>;
    fn channels_repo(&self) -> Box<dyn ChannelRepository + '_>;
    fn companies_repo(&self) -> Box<dyn CompanyRepository + '_>;
    fn transfer_orders_repo(&self) -> Box<dyn TransferOrderRepository + '_>;
    fn units_of_measure_repo(&self) -> Box<dyn UnitOfMeasureRepository + '_>;
    fn production_batches_repo(&self) -> Box<dyn ProductionBatchRepository + '_>;
    fn supplier_skus_repo(&self) -> Box<dyn SupplierSkuRepository + '_>;
    fn vendor_returns_repo(&self) -> Box<dyn VendorReturnRepository + '_>;
    fn vendor_credits_repo(&self) -> Box<dyn VendorCreditRepository + '_>;
    fn payment_obligations_repo(&self) -> Box<dyn PaymentObligationRepository + '_>;
    fn price_levels_repo(&self) -> Box<dyn PriceLevelRepository + '_>;
    fn prepayments_repo(&self) -> Box<dyn PrepaymentRepository + '_>;
    fn price_schedules_repo(&self) -> Box<dyn PriceScheduleRepository + '_>;
    fn activity_logs_repo(&self) -> Box<dyn ActivityLogRepository + '_>;
    fn integration_mappings_repo(&self) -> Box<dyn IntegrationMappingRepository + '_>;
    fn inbound_shipments_repo(&self) -> Box<dyn InboundShipmentRepository + '_>;
    fn purgatory_repo(&self) -> Box<dyn PurgatoryRepository + '_>;
    fn print_stations_repo(&self) -> Box<dyn PrintStationRepository + '_>;
    fn edi_documents_repo(&self) -> Box<dyn EdiDocumentRepository + '_>;
    fn integration_field_mappings_repo(&self) -> Box<dyn IntegrationFieldMappingRepository + '_>;
    fn topology_snapshots_repo(&self) -> Box<dyn TopologySnapshotRepository + '_>;
    fn stock_snapshots_repo(&self) -> Box<dyn StockSnapshotRepository + '_>;
    fn fixed_assets_repo(&self) -> Box<dyn FixedAssetRepository + '_>;
    fn revenue_recognition_repo(&self) -> Box<dyn RevenueRecognitionRepository + '_>;
}

#[cfg(feature = "sqlite")]
impl NewDomainRepositoryFactory for SqliteDatabase {
    fn gift_cards_repo(&self) -> Box<dyn GiftCardRepository + '_> {
        Box::new(self.gift_cards())
    }

    fn store_credits_repo(&self) -> Box<dyn StoreCreditRepository + '_> {
        Box::new(self.store_credits())
    }

    fn segments_repo(&self) -> Box<dyn SegmentRepository + '_> {
        Box::new(self.segments())
    }

    fn shipping_zones_repo(&self) -> Box<dyn ShippingZoneRepository + '_> {
        Box::new(self.shipping_zones())
    }

    fn zone_shipping_methods_repo(&self) -> Box<dyn ZoneShippingMethodRepository + '_> {
        Box::new(self.zone_shipping_methods())
    }

    fn reviews_repo(&self) -> Box<dyn ReviewRepository + '_> {
        Box::new(self.reviews())
    }

    fn wishlists_repo(&self) -> Box<dyn WishlistRepository + '_> {
        Box::new(self.wishlists())
    }

    fn loyalty_programs_repo(&self) -> Box<dyn LoyaltyProgramRepository + '_> {
        Box::new(self.loyalty_programs())
    }

    fn rewards_repo(&self) -> Box<dyn RewardRepository + '_> {
        Box::new(self.rewards())
    }

    fn fraud_repo(&self) -> Box<dyn FraudRepository + '_> {
        Box::new(self.fraud())
    }

    fn search_configs_repo(&self) -> Box<dyn SearchConfigRepository + '_> {
        Box::new(self.search_configs())
    }

    fn channels_repo(&self) -> Box<dyn ChannelRepository + '_> {
        Box::new(self.channels())
    }

    fn companies_repo(&self) -> Box<dyn CompanyRepository + '_> {
        Box::new(self.companies())
    }

    fn transfer_orders_repo(&self) -> Box<dyn TransferOrderRepository + '_> {
        Box::new(self.transfer_orders())
    }

    fn units_of_measure_repo(&self) -> Box<dyn UnitOfMeasureRepository + '_> {
        Box::new(self.units_of_measure())
    }

    fn production_batches_repo(&self) -> Box<dyn ProductionBatchRepository + '_> {
        Box::new(self.production_batches())
    }

    fn supplier_skus_repo(&self) -> Box<dyn SupplierSkuRepository + '_> {
        Box::new(self.supplier_skus())
    }

    fn vendor_returns_repo(&self) -> Box<dyn VendorReturnRepository + '_> {
        Box::new(self.vendor_returns())
    }

    fn vendor_credits_repo(&self) -> Box<dyn VendorCreditRepository + '_> {
        Box::new(self.vendor_credits())
    }

    fn payment_obligations_repo(&self) -> Box<dyn PaymentObligationRepository + '_> {
        Box::new(self.payment_obligations())
    }

    fn price_levels_repo(&self) -> Box<dyn PriceLevelRepository + '_> {
        Box::new(self.price_levels())
    }

    fn prepayments_repo(&self) -> Box<dyn PrepaymentRepository + '_> {
        Box::new(self.prepayments())
    }

    fn price_schedules_repo(&self) -> Box<dyn PriceScheduleRepository + '_> {
        Box::new(self.price_schedules())
    }

    fn activity_logs_repo(&self) -> Box<dyn ActivityLogRepository + '_> {
        Box::new(self.activity_logs())
    }

    fn integration_mappings_repo(&self) -> Box<dyn IntegrationMappingRepository + '_> {
        Box::new(self.integration_mappings())
    }

    fn inbound_shipments_repo(&self) -> Box<dyn InboundShipmentRepository + '_> {
        Box::new(self.inbound_shipments())
    }

    fn purgatory_repo(&self) -> Box<dyn PurgatoryRepository + '_> {
        Box::new(self.purgatory())
    }

    fn print_stations_repo(&self) -> Box<dyn PrintStationRepository + '_> {
        Box::new(self.print_stations())
    }

    fn edi_documents_repo(&self) -> Box<dyn EdiDocumentRepository + '_> {
        Box::new(self.edi_documents())
    }

    fn integration_field_mappings_repo(&self) -> Box<dyn IntegrationFieldMappingRepository + '_> {
        Box::new(self.integration_field_mappings())
    }

    fn topology_snapshots_repo(&self) -> Box<dyn TopologySnapshotRepository + '_> {
        Box::new(self.topology_snapshots())
    }

    fn stock_snapshots_repo(&self) -> Box<dyn StockSnapshotRepository + '_> {
        Box::new(self.stock_snapshots())
    }

    fn fixed_assets_repo(&self) -> Box<dyn FixedAssetRepository + '_> {
        Box::new(self.fixed_assets())
    }

    fn revenue_recognition_repo(&self) -> Box<dyn RevenueRecognitionRepository + '_> {
        Box::new(self.revenue_recognition())
    }
}

#[cfg(feature = "postgres")]
impl NewDomainRepositoryFactory for PostgresDatabase {
    fn gift_cards_repo(&self) -> Box<dyn GiftCardRepository + '_> {
        Box::new(postgres::PgGiftCardRepository::new(self.pool().clone()))
    }

    fn store_credits_repo(&self) -> Box<dyn StoreCreditRepository + '_> {
        Box::new(postgres::PgStoreCreditRepository::new(self.pool().clone()))
    }

    fn segments_repo(&self) -> Box<dyn SegmentRepository + '_> {
        Box::new(self.segments())
    }

    fn shipping_zones_repo(&self) -> Box<dyn ShippingZoneRepository + '_> {
        Box::new(self.shipping_zones())
    }

    fn zone_shipping_methods_repo(&self) -> Box<dyn ZoneShippingMethodRepository + '_> {
        Box::new(self.zone_shipping_methods())
    }

    fn reviews_repo(&self) -> Box<dyn ReviewRepository + '_> {
        Box::new(postgres::PgReviewRepository::new(self.pool().clone()))
    }

    fn wishlists_repo(&self) -> Box<dyn WishlistRepository + '_> {
        Box::new(postgres::PgWishlistRepository::new(self.pool().clone()))
    }

    fn loyalty_programs_repo(&self) -> Box<dyn LoyaltyProgramRepository + '_> {
        Box::new(self.loyalty())
    }

    fn rewards_repo(&self) -> Box<dyn RewardRepository + '_> {
        Box::new(self.rewards())
    }

    fn fraud_repo(&self) -> Box<dyn FraudRepository + '_> {
        Box::new(self.fraud())
    }

    fn search_configs_repo(&self) -> Box<dyn SearchConfigRepository + '_> {
        Box::new(self.search_configs())
    }

    fn channels_repo(&self) -> Box<dyn ChannelRepository + '_> {
        Box::new(self.channels())
    }

    fn companies_repo(&self) -> Box<dyn CompanyRepository + '_> {
        Box::new(self.companies())
    }

    fn transfer_orders_repo(&self) -> Box<dyn TransferOrderRepository + '_> {
        Box::new(self.transfer_orders())
    }

    fn units_of_measure_repo(&self) -> Box<dyn UnitOfMeasureRepository + '_> {
        Box::new(self.units_of_measure())
    }

    fn production_batches_repo(&self) -> Box<dyn ProductionBatchRepository + '_> {
        Box::new(self.production_batches())
    }

    fn supplier_skus_repo(&self) -> Box<dyn SupplierSkuRepository + '_> {
        Box::new(self.supplier_skus())
    }

    fn vendor_returns_repo(&self) -> Box<dyn VendorReturnRepository + '_> {
        Box::new(self.vendor_returns())
    }

    fn vendor_credits_repo(&self) -> Box<dyn VendorCreditRepository + '_> {
        Box::new(self.vendor_credits())
    }

    fn payment_obligations_repo(&self) -> Box<dyn PaymentObligationRepository + '_> {
        Box::new(self.payment_obligations())
    }

    fn price_levels_repo(&self) -> Box<dyn PriceLevelRepository + '_> {
        Box::new(self.price_levels())
    }

    fn prepayments_repo(&self) -> Box<dyn PrepaymentRepository + '_> {
        Box::new(self.prepayments())
    }

    fn price_schedules_repo(&self) -> Box<dyn PriceScheduleRepository + '_> {
        Box::new(self.price_schedules())
    }

    fn activity_logs_repo(&self) -> Box<dyn ActivityLogRepository + '_> {
        Box::new(self.activity_logs())
    }

    fn integration_mappings_repo(&self) -> Box<dyn IntegrationMappingRepository + '_> {
        Box::new(self.integration_mappings())
    }

    fn inbound_shipments_repo(&self) -> Box<dyn InboundShipmentRepository + '_> {
        Box::new(self.inbound_shipments())
    }

    fn purgatory_repo(&self) -> Box<dyn PurgatoryRepository + '_> {
        Box::new(self.purgatory())
    }

    fn print_stations_repo(&self) -> Box<dyn PrintStationRepository + '_> {
        Box::new(self.print_stations())
    }

    fn edi_documents_repo(&self) -> Box<dyn EdiDocumentRepository + '_> {
        Box::new(self.edi_documents())
    }

    fn integration_field_mappings_repo(&self) -> Box<dyn IntegrationFieldMappingRepository + '_> {
        Box::new(self.integration_field_mappings())
    }

    fn topology_snapshots_repo(&self) -> Box<dyn TopologySnapshotRepository + '_> {
        Box::new(self.topology_snapshots())
    }

    fn stock_snapshots_repo(&self) -> Box<dyn StockSnapshotRepository + '_> {
        Box::new(self.stock_snapshots())
    }

    fn fixed_assets_repo(&self) -> Box<dyn FixedAssetRepository + '_> {
        Box::new(self.fixed_assets())
    }

    fn revenue_recognition_repo(&self) -> Box<dyn RevenueRecognitionRepository + '_> {
        Box::new(self.revenue_recognition())
    }
}

/// Macro to eliminate duplicate Database implementations
/// Generates all 32 repository accessor methods for any concrete Database type
macro_rules! impl_database_accessors {
    ($db_type:ident) => {
        impl Database for $db_type {
            fn backend_name(&self) -> &'static str {
                impl_database_accessors!(@backend_name $db_type)
            }

            fn supports_capability(&self, capability: DatabaseCapability) -> bool {
                impl_database_accessors!(@supports_capability $db_type, capability)
            }

            fn orders(&self) -> Box<dyn OrderRepository + '_> {
                Box::new(<$db_type>::orders(self))
            }

            fn inventory(&self) -> Box<dyn InventoryRepository + '_> {
                Box::new(<$db_type>::inventory(self))
            }

            fn customers(&self) -> Box<dyn CustomerRepository + '_> {
                Box::new(<$db_type>::customers(self))
            }

            fn products(&self) -> Box<dyn ProductRepository + '_> {
                Box::new(<$db_type>::products(self))
            }

            fn custom_objects(&self) -> Box<dyn CustomObjectRepository + '_> {
                Box::new(<$db_type>::custom_objects(self))
            }

            fn returns(&self) -> Box<dyn ReturnRepository + '_> {
                Box::new(<$db_type>::returns(self))
            }

            fn bom(&self) -> Box<dyn BomRepository + '_> {
                Box::new(<$db_type>::bom(self))
            }

            fn work_orders(&self) -> Box<dyn WorkOrderRepository + '_> {
                Box::new(<$db_type>::work_orders(self))
            }

            fn shipments(&self) -> Box<dyn ShipmentRepository + '_> {
                Box::new(<$db_type>::shipments(self))
            }

            fn payments(&self) -> Box<dyn PaymentRepository + '_> {
                Box::new(<$db_type>::payments(self))
            }

            fn warranties(&self) -> Box<dyn WarrantyRepository + '_> {
                Box::new(<$db_type>::warranties(self))
            }

            fn purchase_orders(&self) -> Box<dyn PurchaseOrderRepository + '_> {
                Box::new(<$db_type>::purchase_orders(self))
            }

            fn invoices(&self) -> Box<dyn InvoiceRepository + '_> {
                Box::new(<$db_type>::invoices(self))
            }

            fn carts(&self) -> Box<dyn CartRepository + '_> {
                Box::new(<$db_type>::carts(self))
            }

            fn analytics(&self) -> Box<dyn AnalyticsRepository + '_> {
                Box::new(<$db_type>::analytics(self))
            }

            fn currency(&self) -> Box<dyn CurrencyRepository + '_> {
                Box::new(<$db_type>::currency(self))
            }

            fn tax(&self) -> Box<dyn TaxRepository + '_> {
                Box::new(<$db_type>::tax(self))
            }

            fn promotions(&self) -> Box<dyn PromotionRepository + '_> {
                Box::new(<$db_type>::promotions(self))
            }

            fn subscriptions(&self) -> Box<dyn SubscriptionRepository + '_> {
                Box::new(<$db_type>::subscriptions(self))
            }

            fn quality(&self) -> Box<dyn QualityRepository + '_> {
                Box::new(<$db_type>::quality(self))
            }

            fn lots(&self) -> Box<dyn LotRepository + '_> {
                Box::new(<$db_type>::lots(self))
            }

            fn serials(&self) -> Box<dyn SerialRepository + '_> {
                Box::new(<$db_type>::serials(self))
            }

            fn warehouse(&self) -> Box<dyn WarehouseRepository + '_> {
                Box::new(<$db_type>::warehouse(self))
            }

            fn receiving(&self) -> Box<dyn ReceivingRepository + '_> {
                Box::new(<$db_type>::receiving(self))
            }

            fn fulfillment(&self) -> Box<dyn FulfillmentRepository + '_> {
                Box::new(<$db_type>::fulfillment(self))
            }

            fn accounts_payable(&self) -> Box<dyn AccountsPayableRepository + '_> {
                Box::new(<$db_type>::accounts_payable(self))
            }

            fn cost_accounting(&self) -> Box<dyn CostAccountingRepository + '_> {
                Box::new(<$db_type>::cost_accounting(self))
            }

            fn credit(&self) -> Box<dyn CreditRepository + '_> {
                Box::new(<$db_type>::credit(self))
            }

            fn backorder(&self) -> Box<dyn BackorderRepository + '_> {
                Box::new(<$db_type>::backorder(self))
            }

            fn accounts_receivable(&self) -> Box<dyn AccountsReceivableRepository + '_> {
                Box::new(<$db_type>::accounts_receivable(self))
            }

            fn general_ledger(&self) -> Box<dyn GeneralLedgerRepository + '_> {
                Box::new(<$db_type>::general_ledger(self))
            }

            fn x402_payment_intents(&self) -> Box<dyn X402PaymentIntentRepository + '_> {
                Box::new(<$db_type>::x402_payment_intents(self))
            }

            fn x402_credits(&self) -> Box<dyn X402CreditRepository + '_> {
                Box::new(<$db_type>::x402_credits(self))
            }

            fn a2a_quotes(&self) -> Box<dyn A2ACommerceRepository + '_> {
                Box::new(<$db_type>::a2a_quotes(self))
            }

            fn a2a_purchases(&self) -> Box<dyn A2ACommerceRepository + '_> {
                Box::new(<$db_type>::a2a_purchases(self))
            }

            fn agent_cards(&self) -> Box<dyn AgentCardRepository + '_> {
                Box::new(<$db_type>::agent_cards(self))
            }

            fn agent_identities(&self) -> Box<dyn AgentIdentityRepository + '_> {
                Box::new(<$db_type>::agent_identities(self))
            }

            fn agent_reputation(&self) -> Box<dyn AgentReputationRepository + '_> {
                Box::new(<$db_type>::agent_reputation(self))
            }

            fn agent_validation(&self) -> Box<dyn AgentValidationRepository + '_> {
                Box::new(<$db_type>::agent_validation(self))
            }

            // --- New domain repositories ---

            fn gift_cards(&self) -> Box<dyn GiftCardRepository + '_> {
                crate::NewDomainRepositoryFactory::gift_cards_repo(self)
            }

            fn store_credits(&self) -> Box<dyn StoreCreditRepository + '_> {
                crate::NewDomainRepositoryFactory::store_credits_repo(self)
            }

            fn segments(&self) -> Box<dyn SegmentRepository + '_> {
                crate::NewDomainRepositoryFactory::segments_repo(self)
            }

            fn shipping_zones(&self) -> Box<dyn ShippingZoneRepository + '_> {
                crate::NewDomainRepositoryFactory::shipping_zones_repo(self)
            }

            fn zone_shipping_methods(&self) -> Box<dyn ZoneShippingMethodRepository + '_> {
                crate::NewDomainRepositoryFactory::zone_shipping_methods_repo(self)
            }

            fn reviews(&self) -> Box<dyn ReviewRepository + '_> {
                crate::NewDomainRepositoryFactory::reviews_repo(self)
            }

            fn wishlists(&self) -> Box<dyn WishlistRepository + '_> {
                crate::NewDomainRepositoryFactory::wishlists_repo(self)
            }

            fn loyalty_programs(&self) -> Box<dyn LoyaltyProgramRepository + '_> {
                crate::NewDomainRepositoryFactory::loyalty_programs_repo(self)
            }

            fn rewards(&self) -> Box<dyn RewardRepository + '_> {
                crate::NewDomainRepositoryFactory::rewards_repo(self)
            }

            fn fraud(&self) -> Box<dyn FraudRepository + '_> {
                crate::NewDomainRepositoryFactory::fraud_repo(self)
            }

            fn search_configs(&self) -> Box<dyn SearchConfigRepository + '_> {
                crate::NewDomainRepositoryFactory::search_configs_repo(self)
            }

            fn channels(&self) -> Box<dyn ChannelRepository + '_> {
                crate::NewDomainRepositoryFactory::channels_repo(self)
            }

            fn companies(&self) -> Box<dyn CompanyRepository + '_> {
                crate::NewDomainRepositoryFactory::companies_repo(self)
            }

            fn transfer_orders(&self) -> Box<dyn TransferOrderRepository + '_> {
                crate::NewDomainRepositoryFactory::transfer_orders_repo(self)
            }

            fn units_of_measure(&self) -> Box<dyn UnitOfMeasureRepository + '_> {
                crate::NewDomainRepositoryFactory::units_of_measure_repo(self)
            }

            fn production_batches(&self) -> Box<dyn ProductionBatchRepository + '_> {
                crate::NewDomainRepositoryFactory::production_batches_repo(self)
            }

            fn supplier_skus(&self) -> Box<dyn SupplierSkuRepository + '_> {
                crate::NewDomainRepositoryFactory::supplier_skus_repo(self)
            }

            fn vendor_returns(&self) -> Box<dyn VendorReturnRepository + '_> {
                crate::NewDomainRepositoryFactory::vendor_returns_repo(self)
            }

            fn vendor_credits(&self) -> Box<dyn VendorCreditRepository + '_> {
                crate::NewDomainRepositoryFactory::vendor_credits_repo(self)
            }

            fn payment_obligations(&self) -> Box<dyn PaymentObligationRepository + '_> {
                crate::NewDomainRepositoryFactory::payment_obligations_repo(self)
            }

            fn price_levels(&self) -> Box<dyn PriceLevelRepository + '_> {
                crate::NewDomainRepositoryFactory::price_levels_repo(self)
            }

            fn prepayments(&self) -> Box<dyn PrepaymentRepository + '_> {
                crate::NewDomainRepositoryFactory::prepayments_repo(self)
            }

            fn price_schedules(&self) -> Box<dyn PriceScheduleRepository + '_> {
                crate::NewDomainRepositoryFactory::price_schedules_repo(self)
            }

            fn activity_logs(&self) -> Box<dyn ActivityLogRepository + '_> {
                crate::NewDomainRepositoryFactory::activity_logs_repo(self)
            }

            fn integration_mappings(&self) -> Box<dyn IntegrationMappingRepository + '_> {
                crate::NewDomainRepositoryFactory::integration_mappings_repo(self)
            }

            fn inbound_shipments(&self) -> Box<dyn InboundShipmentRepository + '_> {
                crate::NewDomainRepositoryFactory::inbound_shipments_repo(self)
            }

            fn purgatory(&self) -> Box<dyn PurgatoryRepository + '_> {
                crate::NewDomainRepositoryFactory::purgatory_repo(self)
            }

            fn print_stations(&self) -> Box<dyn PrintStationRepository + '_> {
                crate::NewDomainRepositoryFactory::print_stations_repo(self)
            }

            fn edi_documents(&self) -> Box<dyn EdiDocumentRepository + '_> {
                crate::NewDomainRepositoryFactory::edi_documents_repo(self)
            }

            fn integration_field_mappings(&self) -> Box<dyn IntegrationFieldMappingRepository + '_> {
                crate::NewDomainRepositoryFactory::integration_field_mappings_repo(self)
            }

            fn topology_snapshots(&self) -> Box<dyn TopologySnapshotRepository + '_> {
                crate::NewDomainRepositoryFactory::topology_snapshots_repo(self)
            }

            fn stock_snapshots(&self) -> Box<dyn StockSnapshotRepository + '_> {
                crate::NewDomainRepositoryFactory::stock_snapshots_repo(self)
            }

            fn fixed_assets(&self) -> Box<dyn FixedAssetRepository + '_> {
                crate::NewDomainRepositoryFactory::fixed_assets_repo(self)
            }

            fn revenue_recognition(&self) -> Box<dyn RevenueRecognitionRepository + '_> {
                crate::NewDomainRepositoryFactory::revenue_recognition_repo(self)
            }

            fn http_idempotency(&self) -> Option<Box<dyn HttpIdempotencyRepository + '_>> {
                Some(Box::new(<$db_type>::http_idempotency(self)))
            }
        }
    };
    (@backend_name SqliteDatabase) => {
        "sqlite"
    };
    (@backend_name PostgresDatabase) => {
        "postgres"
    };
    (@supports_capability SqliteDatabase, $capability:expr) => {{
        let _ = $capability;
        true
    }};
    (@supports_capability PostgresDatabase, $capability:expr) => {{
        let _ = $capability;
        true
    }};
}

// Apply the macro to generate Database implementations
#[cfg(feature = "sqlite")]
impl_database_accessors!(SqliteDatabase);

#[cfg(feature = "postgres")]
impl_database_accessors!(PostgresDatabase);

/// Default PostgreSQL pool size when `STATESET_DB_MAX_CONNECTIONS` is unset.
pub const DEFAULT_POSTGRES_POOL_SIZE: u32 = 10;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Path to database file (SQLite) or connection string (PostgreSQL)
    pub url: String,
    /// Maximum number of connections in pool
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self { url: "stateset.db".to_string(), max_connections: 5 }
    }
}

impl DatabaseConfig {
    /// Create config for SQLite with path
    #[must_use]
    pub fn sqlite(path: &str) -> Self {
        Self { url: path.to_string(), max_connections: 5 }
    }

    /// Create config for in-memory SQLite (useful for testing)
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            url: ":memory:".to_string(),
            // Use multiple connections with FULL_MUTEX mode for serialized access.
            // This avoids connection pool exhaustion in flows that legitimately need more
            // than one connection, such as nested repository calls in tests.
            max_connections: 4,
        }
    }

    /// Create config for PostgreSQL connection
    ///
    /// # Example
    /// ```ignore
    /// let config = DatabaseConfig::postgres("postgres://user:pass@localhost/stateset");
    /// ```
    #[must_use]
    pub fn postgres(connection_string: &str) -> Self {
        Self {
            url: connection_string.to_string(),
            max_connections: Self::pool_size_from_env(DEFAULT_POSTGRES_POOL_SIZE),
        }
    }

    /// Resolve a pool size from `STATESET_DB_MAX_CONNECTIONS`, falling back to
    /// `default` when the variable is unset or unparseable.
    ///
    /// The compiled-in defaults suit embedded and single-node use; server
    /// deployments serving concurrent traffic will usually need a larger pool
    /// than the default, hence the environment override.
    fn pool_size_from_env(default: u32) -> u32 {
        Self::parse_pool_size(std::env::var("STATESET_DB_MAX_CONNECTIONS").ok().as_deref(), default)
    }

    /// Pure half of [`Self::pool_size_from_env`], split out so the parsing
    /// rules are testable without mutating process-global environment state.
    fn parse_pool_size(raw: Option<&str>, default: u32) -> u32 {
        raw.and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|size| *size > 0)
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod config_tests {
    use super::{DEFAULT_POSTGRES_POOL_SIZE, DatabaseConfig};

    #[test]
    fn pool_size_falls_back_on_absent_or_invalid_values() {
        for raw in [None, Some(""), Some("   "), Some("not-a-number"), Some("0"), Some("-4")] {
            assert_eq!(
                DatabaseConfig::parse_pool_size(raw, DEFAULT_POSTGRES_POOL_SIZE),
                DEFAULT_POSTGRES_POOL_SIZE,
                "{raw:?} must fall back to the default pool size"
            );
        }
    }

    #[test]
    fn pool_size_honors_valid_override() {
        assert_eq!(DatabaseConfig::parse_pool_size(Some("64"), DEFAULT_POSTGRES_POOL_SIZE), 64);
        assert_eq!(DatabaseConfig::parse_pool_size(Some(" 32 "), DEFAULT_POSTGRES_POOL_SIZE), 32);
    }

    #[test]
    fn postgres_config_defaults_to_documented_pool_size() {
        // No override set in the test environment.
        assert_eq!(
            DatabaseConfig::postgres("postgres://localhost/x").max_connections,
            DEFAULT_POSTGRES_POOL_SIZE
        );
    }
}
