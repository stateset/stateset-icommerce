#![deny(unsafe_code)]

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

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDatabase;

#[cfg(feature = "postgres")]
pub use postgres::PostgresDatabase;

use stateset_core::{
    AccountsPayableRepository, AccountsReceivableRepository, AgentCardRepository,
    AgentIdentityRepository, AgentReputationRepository, AgentValidationRepository,
    AnalyticsRepository, BackorderRepository, BomRepository, CartRepository,
    CostAccountingRepository, CreditRepository, CurrencyRepository, CustomObjectRepository,
    CustomerRepository, FulfillmentRepository, GeneralLedgerRepository, InventoryRepository,
    InvoiceRepository, LotRepository, OrderRepository, PaymentRepository, ProductRepository,
    PromotionRepository, PurchaseOrderRepository, QualityRepository, ReceivingRepository, Result,
    ReturnRepository, SerialRepository, ShipmentRepository, SubscriptionRepository, TaxRepository,
    WarehouseRepository, WarrantyRepository, WorkOrderRepository, X402CreditRepository,
    X402PaymentIntentRepository,
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

impl TransactionOptions {
    /// Create options with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timeout
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Set the isolation level
    pub fn isolation(mut self, level: TransactionIsolation) -> Self {
        self.isolation = level;
        self
    }

    /// Enable retry on conflict
    pub fn with_retries(mut self, max_retries: u32) -> Self {
        self.retry_on_conflict = true;
        self.max_retries = max_retries;
        self
    }
}

/// Transaction isolation levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransactionIsolation {
    /// Read uncommitted (SQLite ignores this, uses serializable)
    ReadUncommitted,
    /// Read committed
    ReadCommitted,
    /// Repeatable read
    RepeatableRead,
    /// Serializable (default for SQLite)
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
    /// Get the agent card repository
    fn agent_cards(&self) -> Box<dyn AgentCardRepository + '_>;
    /// Get the agent identity registry repository (ERC-8004)
    fn agent_identities(&self) -> Box<dyn AgentIdentityRepository + '_>;
    /// Get the agent reputation registry repository (ERC-8004)
    fn agent_reputation(&self) -> Box<dyn AgentReputationRepository + '_>;
    /// Get the agent validation registry repository (ERC-8004)
    fn agent_validation(&self) -> Box<dyn AgentValidationRepository + '_>;
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
        F: FnOnce(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>;

    /// Execute a closure within a database transaction with custom options.
    ///
    /// This method allows setting transaction options like timeout and retry behavior.
    fn with_transaction_opts<F, T>(&self, _opts: TransactionOptions, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>,
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
    async fn with_transaction_async<F, T, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(sqlx::Transaction<'static, sqlx::Postgres>) -> Fut + Send,
        Fut: std::future::Future<Output = std::result::Result<T, sqlx::Error>> + Send,
        T: Send;

    /// Execute an async closure within a transaction with custom options.
    async fn with_transaction_async_opts<F, T, Fut>(
        &self,
        opts: TransactionOptions,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(sqlx::Transaction<'static, sqlx::Postgres>) -> Fut + Send,
        Fut: std::future::Future<Output = std::result::Result<T, sqlx::Error>> + Send,
        T: Send;
}

/// Macro to eliminate duplicate Database implementations
/// Generates all 32 repository accessor methods for any concrete Database type
macro_rules! impl_database_accessors {
    ($db_type:ty) => {
        impl Database for $db_type {
            fn orders(&self) -> Box<dyn OrderRepository + '_> {
                Box::new(self.orders())
            }

            fn inventory(&self) -> Box<dyn InventoryRepository + '_> {
                Box::new(self.inventory())
            }

            fn customers(&self) -> Box<dyn CustomerRepository + '_> {
                Box::new(self.customers())
            }

            fn products(&self) -> Box<dyn ProductRepository + '_> {
                Box::new(self.products())
            }

            fn custom_objects(&self) -> Box<dyn CustomObjectRepository + '_> {
                Box::new(self.custom_objects())
            }

            fn returns(&self) -> Box<dyn ReturnRepository + '_> {
                Box::new(self.returns())
            }

            fn bom(&self) -> Box<dyn BomRepository + '_> {
                Box::new(self.bom())
            }

            fn work_orders(&self) -> Box<dyn WorkOrderRepository + '_> {
                Box::new(self.work_orders())
            }

            fn shipments(&self) -> Box<dyn ShipmentRepository + '_> {
                Box::new(self.shipments())
            }

            fn payments(&self) -> Box<dyn PaymentRepository + '_> {
                Box::new(self.payments())
            }

            fn warranties(&self) -> Box<dyn WarrantyRepository + '_> {
                Box::new(self.warranties())
            }

            fn purchase_orders(&self) -> Box<dyn PurchaseOrderRepository + '_> {
                Box::new(self.purchase_orders())
            }

            fn invoices(&self) -> Box<dyn InvoiceRepository + '_> {
                Box::new(self.invoices())
            }

            fn carts(&self) -> Box<dyn CartRepository + '_> {
                Box::new(self.carts())
            }

            fn analytics(&self) -> Box<dyn AnalyticsRepository + '_> {
                Box::new(self.analytics())
            }

            fn currency(&self) -> Box<dyn CurrencyRepository + '_> {
                Box::new(self.currency())
            }

            fn tax(&self) -> Box<dyn TaxRepository + '_> {
                Box::new(self.tax())
            }

            fn promotions(&self) -> Box<dyn PromotionRepository + '_> {
                Box::new(self.promotions())
            }

            fn subscriptions(&self) -> Box<dyn SubscriptionRepository + '_> {
                Box::new(self.subscriptions())
            }

            fn quality(&self) -> Box<dyn QualityRepository + '_> {
                Box::new(self.quality())
            }

            fn lots(&self) -> Box<dyn LotRepository + '_> {
                Box::new(self.lots())
            }

            fn serials(&self) -> Box<dyn SerialRepository + '_> {
                Box::new(self.serials())
            }

            fn warehouse(&self) -> Box<dyn WarehouseRepository + '_> {
                Box::new(self.warehouse())
            }

            fn receiving(&self) -> Box<dyn ReceivingRepository + '_> {
                Box::new(self.receiving())
            }

            fn fulfillment(&self) -> Box<dyn FulfillmentRepository + '_> {
                Box::new(self.fulfillment())
            }

            fn accounts_payable(&self) -> Box<dyn AccountsPayableRepository + '_> {
                Box::new(self.accounts_payable())
            }

            fn cost_accounting(&self) -> Box<dyn CostAccountingRepository + '_> {
                Box::new(self.cost_accounting())
            }

            fn credit(&self) -> Box<dyn CreditRepository + '_> {
                Box::new(self.credit())
            }

            fn backorder(&self) -> Box<dyn BackorderRepository + '_> {
                Box::new(self.backorder())
            }

            fn accounts_receivable(&self) -> Box<dyn AccountsReceivableRepository + '_> {
                Box::new(self.accounts_receivable())
            }

            fn general_ledger(&self) -> Box<dyn GeneralLedgerRepository + '_> {
                Box::new(self.general_ledger())
            }

            fn x402_payment_intents(&self) -> Box<dyn X402PaymentIntentRepository + '_> {
                Box::new(self.x402_payment_intents())
            }

            fn x402_credits(&self) -> Box<dyn X402CreditRepository + '_> {
                Box::new(self.x402_credits())
            }

            fn agent_cards(&self) -> Box<dyn AgentCardRepository + '_> {
                Box::new(self.agent_cards())
            }

            fn agent_identities(&self) -> Box<dyn AgentIdentityRepository + '_> {
                Box::new(self.agent_identities())
            }

            fn agent_reputation(&self) -> Box<dyn AgentReputationRepository + '_> {
                Box::new(self.agent_reputation())
            }

            fn agent_validation(&self) -> Box<dyn AgentValidationRepository + '_> {
                Box::new(self.agent_validation())
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
        Self {
            url: "stateset.db".to_string(),
            max_connections: 5,
        }
    }
}

impl DatabaseConfig {
    /// Create config for SQLite with path
    pub fn sqlite(path: &str) -> Self {
        Self {
            url: path.to_string(),
            max_connections: 5,
        }
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
        Self {
            url: connection_string.to_string(),
            max_connections: 10,
        }
    }
}
