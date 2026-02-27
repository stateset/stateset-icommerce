#![deny(unsafe_code)]
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

#[cfg(feature = "sqlite")]
pub mod migrations;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(all(feature = "postgres", feature = "saga"))]
pub mod saga;

pub mod transactions;
#[cfg(feature = "postgres")]
mod unsupported_repositories;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDatabase;

#[cfg(feature = "postgres")]
pub use postgres::PostgresDatabase;

use stateset_core::{
    A2ACommerceRepository, AccountsPayableRepository, AccountsReceivableRepository,
    AgentCardRepository, AgentIdentityRepository, AgentReputationRepository,
    AgentValidationRepository, AnalyticsRepository, BackorderRepository, BomRepository,
    CartRepository, CostAccountingRepository, CreditRepository, CurrencyRepository,
    CustomObjectRepository, CustomerRepository, FraudRepository, FulfillmentRepository,
    GeneralLedgerRepository, GiftCardRepository, InventoryRepository, InvoiceRepository,
    LotRepository, LoyaltyProgramRepository, OrderRepository, PaymentRepository, ProductRepository,
    PromotionRepository, PurchaseOrderRepository, QualityRepository, ReceivingRepository, Result,
    ReturnRepository, ReviewRepository, RewardRepository, SearchConfigRepository,
    SegmentRepository, SerialRepository, ShipmentRepository, ShippingZoneRepository,
    StoreCreditRepository, SubscriptionRepository, TaxRepository, WarehouseRepository,
    WarrantyRepository, WishlistRepository, WorkOrderRepository, X402CreditRepository,
    X402PaymentIntentRepository, ZoneShippingMethodRepository,
};
#[cfg(feature = "postgres")]
use unsupported_repositories::{
    UnsupportedFraudRepository, UnsupportedGiftCardRepository, UnsupportedLoyaltyProgramRepository,
    UnsupportedReviewRepository, UnsupportedRewardRepository, UnsupportedSearchConfigRepository,
    UnsupportedSegmentRepository, UnsupportedShippingZoneRepository,
    UnsupportedStoreCreditRepository, UnsupportedWishlistRepository,
    UnsupportedZoneShippingMethodRepository,
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

// ============================================================================
// Database Trait
// ============================================================================

/// Unified database trait that both SQLite and PostgreSQL implement.
/// This allows stateset-embedded to work with either backend.
pub trait Database: Send + Sync {
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
}

#[cfg(feature = "postgres")]
impl NewDomainRepositoryFactory for PostgresDatabase {
    fn gift_cards_repo(&self) -> Box<dyn GiftCardRepository + '_> {
        Box::new(UnsupportedGiftCardRepository::new("postgres"))
    }

    fn store_credits_repo(&self) -> Box<dyn StoreCreditRepository + '_> {
        Box::new(UnsupportedStoreCreditRepository::new("postgres"))
    }

    fn segments_repo(&self) -> Box<dyn SegmentRepository + '_> {
        Box::new(UnsupportedSegmentRepository::new("postgres"))
    }

    fn shipping_zones_repo(&self) -> Box<dyn ShippingZoneRepository + '_> {
        Box::new(UnsupportedShippingZoneRepository::new("postgres"))
    }

    fn zone_shipping_methods_repo(&self) -> Box<dyn ZoneShippingMethodRepository + '_> {
        Box::new(UnsupportedZoneShippingMethodRepository::new("postgres"))
    }

    fn reviews_repo(&self) -> Box<dyn ReviewRepository + '_> {
        Box::new(UnsupportedReviewRepository::new("postgres"))
    }

    fn wishlists_repo(&self) -> Box<dyn WishlistRepository + '_> {
        Box::new(UnsupportedWishlistRepository::new("postgres"))
    }

    fn loyalty_programs_repo(&self) -> Box<dyn LoyaltyProgramRepository + '_> {
        Box::new(UnsupportedLoyaltyProgramRepository::new("postgres"))
    }

    fn rewards_repo(&self) -> Box<dyn RewardRepository + '_> {
        Box::new(UnsupportedRewardRepository::new("postgres"))
    }

    fn fraud_repo(&self) -> Box<dyn FraudRepository + '_> {
        Box::new(UnsupportedFraudRepository::new("postgres"))
    }

    fn search_configs_repo(&self) -> Box<dyn SearchConfigRepository + '_> {
        Box::new(UnsupportedSearchConfigRepository::new("postgres"))
    }
}

/// Macro to eliminate duplicate Database implementations
/// Generates all 32 repository accessor methods for any concrete Database type
macro_rules! impl_database_accessors {
    ($db_type:ty) => {
        impl Database for $db_type {
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
        }
    };
}

// Apply the macro to generate Database implementations
#[cfg(feature = "sqlite")]
impl_database_accessors!(SqliteDatabase);

#[cfg(feature = "postgres")]
impl_database_accessors!(PostgresDatabase);

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
    pub fn sqlite(path: &str) -> Self {
        Self { url: path.to_string(), max_connections: 5 }
    }

    /// Create config for in-memory SQLite (useful for testing)
    pub fn in_memory() -> Self {
        Self {
            url: ":memory:".to_string(),
            // Use multiple connections with FULL_MUTEX mode for serialized access.
            // This avoids connection pool exhaustion while preventing SQLITE_LOCKED errors.
            max_connections: 4,
        }
    }

    /// Create config for PostgreSQL connection
    ///
    /// # Example
    /// ```ignore
    /// let config = DatabaseConfig::postgres("postgres://user:pass@localhost/stateset");
    /// ```
    pub fn postgres(connection_string: &str) -> Self {
        Self { url: connection_string.to_string(), max_connections: 10 }
    }
}
