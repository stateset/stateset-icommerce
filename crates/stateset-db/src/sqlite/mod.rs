//! SQLite database implementation

mod analytics;
mod bom;
mod carts;
mod currency;
mod customers;
mod fulfillment;
mod inventory;
mod invoices;
mod lots;
mod orders;
pub(crate) mod parse_helpers;
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

pub use accounts_payable::*;
pub use accounts_receivable::*;
pub use backorder::*;
pub use cost_accounting::*;
pub use credit::*;
pub use general_ledger::*;
pub use analytics::*;
pub use bom::*;
pub use carts::*;
pub use currency::*;
pub use customers::*;
pub use fulfillment::*;
pub use inventory::*;
pub use invoices::*;
pub use lots::*;
pub use orders::*;
pub use payments::*;
pub use products::*;
pub use promotions::*;
pub use purchase_orders::*;
pub use quality::*;
pub use receiving::*;
pub use returns::*;
pub use serials::*;
pub use shipments::*;
pub use subscriptions::*;
pub use tax::*;
pub use warehouse::*;
pub use warranties::*;
pub use work_orders::*;

use crate::migrations;
use crate::DatabaseConfig;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::OpenFlags;
use stateset_core::CommerceError;

/// SQLite database connection pool
pub struct SqliteDatabase {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteDatabase {
    /// Create a new SQLite database connection
    pub fn new(config: &DatabaseConfig) -> Result<Self, CommerceError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::Duration;

        // Counter to generate unique database names for each in-memory instance
        static MEMORY_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

        // For in-memory databases, use shared-cache mode with a unique URI to allow multiple
        // connections to access the same in-memory database. Each Commerce instance gets its
        // own unique database name to ensure test isolation.
        let is_memory = config.url == ":memory:";
        let (manager, max_connections) = if is_memory {
            // Generate unique database name for this instance
            let db_id = MEMORY_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
            let uri = format!("file:memdb_{}?mode=memory&cache=shared", db_id);
            let manager = SqliteConnectionManager::file(&uri).with_flags(
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            );
            // Respect configured pool size; ensure at least one connection.
            (manager, config.max_connections.max(1))
        } else {
            let manager = SqliteConnectionManager::file(&config.url).with_flags(
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
            );
            (manager, config.max_connections)
        };
        let manager = manager.with_init(move |conn| {
            conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
            if !is_memory {
                conn.execute_batch("PRAGMA journal_mode = WAL;")?;
            }
            Ok(())
        });

        let pool = Pool::builder()
            .max_size(max_connections)
            .connection_timeout(Duration::from_secs(30))
            .build(manager)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Run migrations
        let conn = pool
            .get()
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

    /// Get quality repository
    pub fn quality(&self) -> SqliteQualityRepository {
        SqliteQualityRepository::new(self.pool.clone())
    }

    /// Get lots repository
    pub fn lots(&self) -> SqliteLotRepository {
        SqliteLotRepository::new(self.pool.clone())
    }

    /// Get serials repository
    pub fn serials(&self) -> SqliteSerialRepository {
        SqliteSerialRepository::new(self.pool.clone())
    }

    /// Get warehouse repository
    pub fn warehouse(&self) -> SqliteWarehouseRepository {
        SqliteWarehouseRepository::new(self.pool.clone())
    }

    /// Get receiving repository
    pub fn receiving(&self) -> SqliteReceivingRepository {
        SqliteReceivingRepository::new(self.pool.clone())
    }

    /// Get fulfillment repository
    pub fn fulfillment(&self) -> SqliteFulfillmentRepository {
        SqliteFulfillmentRepository::new(self.pool.clone())
    }

    /// Get accounts payable repository
    pub fn accounts_payable(&self) -> SqliteAccountsPayableRepository {
        SqliteAccountsPayableRepository::new(self.pool.clone())
    }

    /// Get cost accounting repository
    pub fn cost_accounting(&self) -> SqliteCostAccountingRepository {
        SqliteCostAccountingRepository::new(self.pool.clone())
    }

    /// Get credit repository
    pub fn credit(&self) -> SqliteCreditRepository {
        SqliteCreditRepository::new(self.pool.clone())
    }

    /// Get backorder repository
    pub fn backorder(&self) -> SqliteBackorderRepository {
        SqliteBackorderRepository::new(self.pool.clone())
    }

    /// Get accounts receivable repository
    pub fn accounts_receivable(&self) -> SqliteAccountsReceivableRepository {
        SqliteAccountsReceivableRepository::new(self.pool.clone())
    }

    /// Get general ledger repository
    pub fn general_ledger(&self) -> SqliteGeneralLedgerRepository {
        SqliteGeneralLedgerRepository::new(self.pool.clone())
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

// Re-export parse helpers for use in submodules
pub(crate) use parse_helpers::{
    parse_uuid_row, parse_uuid_opt_row,
    parse_datetime_row, parse_datetime_opt_row,
    parse_decimal_row, parse_decimal_opt_row,
    parse_json_row, parse_json_opt_row,
    parse_date_row,
    parse_enum_row,
    // Non-row variants for use outside rusqlite closures
    parse_uuid, parse_uuid_opt, parse_datetime, parse_datetime_opt,
    parse_decimal as parse_decimal_strict, parse_decimal_opt,
    parse_enum,
};

// ============================================================================
// Batch Operation Helpers
// ============================================================================

/// Build SQL IN clause with placeholders for the given count
/// Example: build_in_clause(3) returns "?, ?, ?"
pub(crate) fn build_in_clause(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Sum a single decimal column from a query using exact Decimal parsing.
pub(crate) fn sum_decimal_query(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    entity: &str,
    field: &str,
) -> stateset_core::Result<Decimal> {
    let mut stmt = conn.prepare(sql).map_err(map_db_error)?;
    let mut rows = stmt.query(params).map_err(map_db_error)?;
    let mut total = Decimal::ZERO;

    while let Some(row) = rows.next().map_err(map_db_error)? {
        let raw: Option<String> = row.get(0).map_err(map_db_error)?;
        if let Some(raw) = raw {
            if !raw.is_empty() {
                total += parse_decimal_strict(&raw, entity, field)?;
            }
        }
    }

    Ok(total)
}

/// Convert a slice of UUIDs to boxed parameter vector for rusqlite
pub(crate) fn uuid_params(ids: &[uuid::Uuid]) -> Vec<Box<dyn rusqlite::ToSql>> {
    ids.iter()
        .map(|id| Box::new(id.to_string()) as Box<dyn rusqlite::ToSql>)
        .collect()
}

/// Convert boxed params to references for rusqlite execution
pub(crate) fn params_refs(params: &[Box<dyn rusqlite::ToSql>]) -> Vec<&dyn rusqlite::ToSql> {
    params.iter().map(|p| p.as_ref()).collect()
}

/// Convert a slice of i64 IDs to boxed parameter vector for rusqlite
pub(crate) fn i64_params(ids: &[i64]) -> Vec<Box<dyn rusqlite::ToSql>> {
    ids.iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>)
        .collect()
}

/// Convert a slice of strings to boxed parameter vector for rusqlite
pub(crate) fn string_params(strings: &[String]) -> Vec<Box<dyn rusqlite::ToSql>> {
    strings
        .iter()
        .map(|s| Box::new(s.clone()) as Box<dyn rusqlite::ToSql>)
        .collect()
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
