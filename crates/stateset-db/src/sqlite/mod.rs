//! SQLite database implementation

mod analytics;
mod bom;
mod carts;
mod currency;
mod customers;
mod inventory;
mod invoices;
mod orders;
mod payments;
mod products;
mod promotions;
mod purchase_orders;
mod returns;
mod shipments;
mod subscriptions;
mod tax;
mod warranties;
mod work_orders;

pub use analytics::*;
pub use bom::*;
pub use carts::*;
pub use currency::*;
pub use customers::*;
pub use inventory::*;
pub use invoices::*;
pub use orders::*;
pub use payments::*;
pub use products::*;
pub use promotions::*;
pub use purchase_orders::*;
pub use returns::*;
pub use shipments::*;
pub use subscriptions::*;
pub use tax::*;
pub use warranties::*;
pub use work_orders::*;

use crate::migrations;
use crate::DatabaseConfig;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;
use stateset_core::CommerceError;

/// SQLite database connection pool
pub struct SqliteDatabase {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteDatabase {
    /// Create a new SQLite database connection
    pub fn new(config: &DatabaseConfig) -> Result<Self, CommerceError> {
        use std::time::Duration;

        // For in-memory databases, use SqliteConnectionManager::memory() with pool size 1
        // This ensures all operations use the same in-memory database
        let (manager, max_connections) = if config.url == ":memory:" {
            (SqliteConnectionManager::memory(), 1)
        } else {
            let manager = SqliteConnectionManager::file(&config.url).with_flags(
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
            );
            (manager, config.max_connections)
        };

        let pool = Pool::builder()
            .max_size(max_connections)
            .connection_timeout(Duration::from_secs(30))
            .build(manager)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Run migrations
        let conn = pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        migrations::run_migrations(&conn)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Create an in-memory database (useful for testing)
    pub fn in_memory() -> Result<Self, CommerceError> {
        Self::new(&DatabaseConfig::in_memory())
    }

    /// Get a connection from the pool
    pub fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, CommerceError> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// Get order repository
    pub fn orders(&self) -> SqliteOrderRepository {
        SqliteOrderRepository::new(self.pool.clone())
    }

    /// Get inventory repository
    pub fn inventory(&self) -> SqliteInventoryRepository {
        SqliteInventoryRepository::new(self.pool.clone())
    }

    /// Get customer repository
    pub fn customers(&self) -> SqliteCustomerRepository {
        SqliteCustomerRepository::new(self.pool.clone())
    }

    /// Get product repository
    pub fn products(&self) -> SqliteProductRepository {
        SqliteProductRepository::new(self.pool.clone())
    }

    /// Get return repository
    pub fn returns(&self) -> SqliteReturnRepository {
        SqliteReturnRepository::new(self.pool.clone())
    }

    /// Get BOM (Bill of Materials) repository
    pub fn bom(&self) -> SqliteBomRepository {
        SqliteBomRepository::new(self.pool.clone())
    }

    /// Get work order repository
    pub fn work_orders(&self) -> SqliteWorkOrderRepository {
        SqliteWorkOrderRepository::new(self.pool.clone())
    }

    /// Get shipment repository
    pub fn shipments(&self) -> SqliteShipmentRepository {
        SqliteShipmentRepository::new(self.pool.clone())
    }

    /// Get payment repository
    pub fn payments(&self) -> SqlitePaymentRepository {
        SqlitePaymentRepository::new(self.pool.clone())
    }

    /// Get warranty repository
    pub fn warranties(&self) -> SqliteWarrantyRepository {
        SqliteWarrantyRepository::new(self.pool.clone())
    }

    /// Get purchase order repository
    pub fn purchase_orders(&self) -> SqlitePurchaseOrderRepository {
        SqlitePurchaseOrderRepository::new(self.pool.clone())
    }

    /// Get invoice repository
    pub fn invoices(&self) -> SqliteInvoiceRepository {
        SqliteInvoiceRepository::new(self.pool.clone())
    }

    /// Get cart repository
    pub fn carts(&self) -> SqliteCartRepository {
        SqliteCartRepository::new(self.pool.clone())
    }

    /// Get analytics repository
    pub fn analytics(&self) -> SqliteAnalyticsRepository {
        SqliteAnalyticsRepository::new(self.pool.clone())
    }

    /// Get currency repository
    pub fn currency(&self) -> SqliteCurrencyRepository {
        SqliteCurrencyRepository::new(self.pool.clone())
    }

    /// Get tax repository
    pub fn tax(&self) -> SqliteTaxRepository {
        SqliteTaxRepository::new(self.pool.clone())
    }

    /// Get promotions repository
    pub fn promotions(&self) -> SqlitePromotionRepository {
        SqlitePromotionRepository::new(self.pool.clone())
    }

    /// Get subscriptions repository
    pub fn subscriptions(&self) -> SqliteSubscriptionRepository {
        SqliteSubscriptionRepository::new(self.pool.clone())
    }

    /// Get underlying pool (for advanced use)
    pub fn pool(&self) -> &Pool<SqliteConnectionManager> {
        &self.pool
    }
}

/// Helper function to convert rusqlite errors to CommerceError
pub(crate) fn map_db_error(e: rusqlite::Error) -> CommerceError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
        _ => CommerceError::DatabaseError(e.to_string()),
    }
}

/// Helper to parse decimal from string
pub(crate) fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or_default()
}

// Transaction support implementation
use crate::DatabaseExt;

impl DatabaseExt for SqliteDatabase {
    fn with_transaction<F, T>(&self, f: F) -> stateset_core::Result<T>
    where
        F: FnOnce(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>,
    {
        let mut conn = self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;

        match f(&tx) {
            Ok(result) => {
                tx.commit().map_err(map_db_error)?;
                Ok(result)
            }
            Err(e) => {
                // Transaction is automatically rolled back on drop
                Err(map_db_error(e))
            }
        }
    }
}
