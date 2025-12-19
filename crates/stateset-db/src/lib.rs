//! # StateSet DB
//!
//! Database implementations for StateSet iCommerce.
//!
//! ## Features
//!
//! - `sqlite` (default): SQLite database support via rusqlite
//! - `postgres`: PostgreSQL database support via sqlx (async)
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

#[cfg(feature = "sqlite")]
pub mod migrations;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDatabase;

#[cfg(feature = "postgres")]
pub use postgres::PostgresDatabase;


use stateset_core::{
    AnalyticsRepository, BomRepository, CartRepository, CurrencyRepository,
    CustomerRepository, InventoryRepository, InvoiceRepository, OrderRepository, PaymentRepository,
    ProductRepository, PurchaseOrderRepository, Result, ReturnRepository, ShipmentRepository,
    WarrantyRepository, WorkOrderRepository,
};

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
}

/// Extension trait for database transaction support.
///
/// Provides closure-based transaction management with automatic commit/rollback.
/// Note: This is a simplified transaction API. For complex transactions spanning
/// multiple repositories, use the raw connection approach via `SqliteDatabase::conn()`.
///
/// # Example
/// ```ignore
/// use stateset_db::{SqliteDatabase, DatabaseExt};
///
/// let db = SqliteDatabase::in_memory()?;
///
/// // Simple transaction using raw SQL
/// db.with_transaction(|conn| {
///     conn.execute("UPDATE inventory_balances SET quantity_on_hand = 100 WHERE item_id = 1", [])?;
///     conn.execute("INSERT INTO inventory_transactions (...) VALUES (...)", [...])?;
///     Ok(())
/// })?;
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
}

#[cfg(feature = "sqlite")]
impl Database for SqliteDatabase {
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
}

#[cfg(feature = "postgres")]
impl Database for PostgresDatabase {
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
}

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
            max_connections: 1,
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
