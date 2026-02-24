//! SQLite database implementation

mod a2a;
mod accounts_payable;
mod accounts_receivable;
mod agent_cards;
mod agent_identities;
mod agent_reputation;
mod agent_validation;
mod analytics;
mod backorder;
mod bom;
mod carts;
mod cost_accounting;
mod credit;
mod currency;
mod custom_objects;
mod customers;
mod fraud;
mod fulfillment;
mod general_ledger;
mod gift_cards;
mod inventory;
mod invoices;
mod lots;
mod loyalty;
mod orders;
pub(crate) mod parse_helpers;
mod payments;
mod products;
mod promotions;
mod purchase_orders;
mod quality;
mod receiving;
mod returns;
mod reviews;
mod rewards;
mod search_configs;
mod segments;
mod serials;
mod shipments;
mod shipping_zones;
mod store_credits;
mod subscriptions;
mod tax;
mod warehouse;
mod warranties;
mod wishlists;
mod work_orders;
mod x402_credits;
mod x402_payment_intents;
mod zone_shipping_methods;

#[cfg(feature = "vector")]
mod vector;

pub use a2a::*;
pub use accounts_payable::*;
pub use accounts_receivable::*;
pub use agent_cards::*;
pub use agent_identities::*;
pub use agent_reputation::*;
pub use agent_validation::*;
pub use analytics::*;
pub use backorder::*;
pub use bom::*;
pub use carts::*;
pub use cost_accounting::*;
pub use credit::*;
pub use currency::*;
pub use custom_objects::*;
pub use customers::*;
pub use fraud::*;
pub use fulfillment::*;
pub use general_ledger::*;
pub use gift_cards::*;
pub use inventory::*;
pub use invoices::*;
pub use lots::*;
pub use loyalty::*;
pub use orders::*;
pub use payments::*;
pub use products::*;
pub use promotions::*;
pub use purchase_orders::*;
pub use quality::*;
pub use receiving::*;
pub use returns::*;
pub use reviews::*;
pub use rewards::*;
pub use search_configs::*;
pub use segments::*;
pub use serials::*;
pub use shipments::*;
pub use shipping_zones::*;
pub use store_credits::*;
pub use subscriptions::*;
pub use tax::*;
#[cfg(feature = "vector")]
pub use vector::*;
pub use warehouse::*;
pub use warranties::*;
pub use wishlists::*;
pub use work_orders::*;
pub use x402_credits::*;
pub use x402_payment_intents::*;
pub use zone_shipping_methods::*;

use crate::DatabaseConfig;
use crate::migrations;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;
use rust_decimal::Decimal;
use stateset_core::CommerceError;
use std::panic::{self, AssertUnwindSafe};
use std::thread;
use std::time::Duration;

/// SQLite database connection pool
#[derive(Debug)]
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
            // Use longer busy_timeout for high concurrency scenarios
            conn.execute_batch(&format!(
                "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = {};",
                crate::DEFAULT_TRANSACTION_TIMEOUT_MS
            ))?;
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

        // Get connection for setup
        let mut conn = pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Run migrations
        migrations::run_migrations(&mut conn)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Create an in-memory database (useful for testing)
    pub fn in_memory() -> Result<Self, CommerceError> {
        Self::new(&DatabaseConfig::in_memory())
    }

    /// Get a connection from the pool
    pub fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, CommerceError> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
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

    /// Get custom objects repository (custom states / metaobjects)
    pub fn custom_objects(&self) -> SqliteCustomObjectRepository {
        SqliteCustomObjectRepository::new(self.pool.clone())
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

    /// Get vector search repository (requires `vector` feature)
    #[cfg(feature = "vector")]
    pub fn vector(&self) -> SqliteVectorRepository {
        SqliteVectorRepository::new(self.pool.clone())
    }

    /// Get x402 payment intent repository
    pub fn x402_payment_intents(&self) -> SqliteX402PaymentIntentRepository {
        SqliteX402PaymentIntentRepository::new(self.pool.clone())
    }

    /// Get x402 credit ledger repository
    pub fn x402_credits(&self) -> SqliteX402CreditRepository {
        SqliteX402CreditRepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    pub fn a2a_quotes(&self) -> SqliteA2ARepository {
        SqliteA2ARepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    pub fn a2a_purchases(&self) -> SqliteA2ARepository {
        SqliteA2ARepository::new(self.pool.clone())
    }

    /// Get agent card repository
    pub fn agent_cards(&self) -> SqliteAgentCardRepository {
        SqliteAgentCardRepository::new(self.pool.clone())
    }

    /// Get agent identity repository (ERC-8004)
    pub fn agent_identities(&self) -> SqliteAgentIdentityRepository {
        SqliteAgentIdentityRepository::new(self.pool.clone())
    }

    /// Get agent reputation repository (ERC-8004)
    pub fn agent_reputation(&self) -> SqliteAgentReputationRepository {
        SqliteAgentReputationRepository::new(self.pool.clone())
    }

    /// Get agent validation repository (ERC-8004)
    pub fn agent_validation(&self) -> SqliteAgentValidationRepository {
        SqliteAgentValidationRepository::new(self.pool.clone())
    }

    /// Get gift card repository
    pub fn gift_cards(&self) -> SqliteGiftCardRepository {
        SqliteGiftCardRepository::new(self.pool.clone())
    }

    /// Get store credit repository
    pub fn store_credits(&self) -> SqliteStoreCreditRepository {
        SqliteStoreCreditRepository::new(self.pool.clone())
    }

    /// Get customer segment repository
    pub fn segments(&self) -> SqliteSegmentRepository {
        SqliteSegmentRepository::new(self.pool.clone())
    }

    /// Get shipping zone repository
    pub fn shipping_zones(&self) -> SqliteShippingZoneRepository {
        SqliteShippingZoneRepository::new(self.pool.clone())
    }

    /// Get zone shipping method repository
    pub fn zone_shipping_methods(&self) -> SqliteZoneShippingMethodRepository {
        SqliteZoneShippingMethodRepository::new(self.pool.clone())
    }

    /// Get product review repository
    pub fn reviews(&self) -> SqliteReviewRepository {
        SqliteReviewRepository::new(self.pool.clone())
    }

    /// Get wishlist repository
    pub fn wishlists(&self) -> SqliteWishlistRepository {
        SqliteWishlistRepository::new(self.pool.clone())
    }

    /// Get loyalty program repository
    pub fn loyalty_programs(&self) -> SqliteLoyaltyProgramRepository {
        SqliteLoyaltyProgramRepository::new(self.pool.clone())
    }

    /// Get reward catalog repository
    pub fn rewards(&self) -> SqliteRewardRepository {
        SqliteRewardRepository::new(self.pool.clone())
    }

    /// Get fraud detection repository
    pub fn fraud(&self) -> SqliteFraudRepository {
        SqliteFraudRepository::new(self.pool.clone())
    }

    /// Get search configuration repository
    pub fn search_configs(&self) -> SqliteSearchConfigRepository {
        SqliteSearchConfigRepository::new(self.pool.clone())
    }

    /// Get underlying pool (for advanced use)
    pub const fn pool(&self) -> &Pool<SqliteConnectionManager> {
        &self.pool
    }
}

/// Helper function to convert rusqlite errors to `CommerceError`
pub(crate) fn map_db_error(e: rusqlite::Error) -> CommerceError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
        rusqlite::Error::ToSqlConversionFailure(boxed) => {
            // Extract CommerceError if it was wrapped for transaction propagation
            match boxed.downcast::<CommerceError>() {
                Ok(commerce_error) => *commerce_error,
                Err(other) => CommerceError::DatabaseError(other.to_string()),
            }
        }
        _ => CommerceError::DatabaseError(e.to_string()),
    }
}

// Re-export parse helpers for use in submodules
pub(crate) use parse_helpers::{
    parse_date_row,
    parse_datetime,
    parse_datetime_opt,
    parse_datetime_opt_row,
    parse_datetime_row,
    parse_decimal as parse_decimal_strict,
    parse_decimal_opt,
    parse_decimal_opt_row,
    parse_decimal_row,
    parse_enum,
    parse_enum_row,
    parse_json_opt_row,
    parse_json_row,
    // Non-row variants for use outside rusqlite closures
    parse_uuid,
    parse_uuid_opt,
    parse_uuid_opt_row,
    parse_uuid_row,
};

// ============================================================================
// Batch Operation Helpers
// ============================================================================

/// Build SQL IN clause with placeholders for the given count
/// Example: `build_in_clause(3)` returns "?, ?, ?"
pub(crate) fn build_in_clause(count: usize) -> String {
    if count == 0 {
        return "NULL".to_string();
    }

    std::iter::repeat_n("?", count).collect::<Vec<_>>().join(", ")
}

/// Check if SQLite JSON1 functions are available.
pub(crate) fn json1_available(conn: &rusqlite::Connection) -> bool {
    conn.query_row("SELECT json_valid('[]')", [], |row| row.get::<_, i32>(0))
        .map(|value| value == 1)
        .unwrap_or(false)
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
    ids.iter().map(|id| Box::new(id.to_string()) as Box<dyn rusqlite::ToSql>).collect()
}

/// Convert boxed params to references for rusqlite execution
pub(crate) fn params_refs(params: &[Box<dyn rusqlite::ToSql>]) -> Vec<&dyn rusqlite::ToSql> {
    params.iter().map(|p| p.as_ref()).collect()
}

/// Convert a slice of i64 IDs to boxed parameter vector for rusqlite
pub(crate) fn i64_params(ids: &[i64]) -> Vec<Box<dyn rusqlite::ToSql>> {
    ids.iter().map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>).collect()
}

/// Convert a slice of strings to boxed parameter vector for rusqlite
pub(crate) fn string_params(strings: &[String]) -> Vec<Box<dyn rusqlite::ToSql>> {
    strings.iter().map(|s| Box::new(s.clone()) as Box<dyn rusqlite::ToSql>).collect()
}

// ============================================================================
// Retry Helpers for SQLite Concurrency
// ============================================================================

/// Maximum number of retries for transient database errors
const MAX_RETRIES: u32 = 50;

/// Initial backoff delay in milliseconds
const INITIAL_BACKOFF_MS: u64 = 1;

/// Maximum backoff delay in milliseconds
const MAX_BACKOFF_MS: u64 = 200;

/// Check if a rusqlite error is a transient lock error that can be retried
pub(crate) fn is_retryable_error(e: &rusqlite::Error) -> bool {
    match e {
        rusqlite::Error::SqliteFailure(ffi_err, msg) => {
            matches!(
                ffi_err.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) || msg.as_ref().is_some_and(|m| {
                m.contains("database is locked") || m.contains("database table is locked")
            })
        }
        _ => false,
    }
}

/// Execute a database operation with retry logic for transient lock errors.
/// Uses exponential backoff with jitter to avoid thundering herd.
pub(crate) fn with_retry<T, F>(mut f: F, max_retries: u32) -> Result<T, rusqlite::Error>
where
    F: FnMut() -> Result<T, rusqlite::Error>,
{
    use std::cell::Cell;
    use std::time::Instant;

    // Thread-local simple PRNG for jitter
    thread_local! {
        static SEED: Cell<u64> = Cell::new(
            Instant::now().elapsed().as_nanos() as u64
        );
    }

    let mut retries = 0;
    let mut backoff_ms = INITIAL_BACKOFF_MS;

    loop {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if is_retryable_error(&e) && retries < max_retries => {
                retries += 1;
                // Simple xorshift for pseudo-random jitter
                let jitter = SEED.with(|seed| {
                    let mut s = seed.get().wrapping_add(retries as u64);
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    seed.set(s);
                    s % 50
                });
                let delay = backoff_ms.min(MAX_BACKOFF_MS) + jitter;
                thread::sleep(Duration::from_millis(delay));
                // Exponential backoff with cap
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Execute a transactional database operation with IMMEDIATE transaction mode
/// and retry logic. IMMEDIATE transactions acquire write locks immediately,
/// avoiding deadlocks caused by lock upgrade failures in DEFERRED mode.
pub(crate) fn with_immediate_transaction<T, F>(
    pool: &Pool<SqliteConnectionManager>,
    f: F,
) -> stateset_core::Result<T>
where
    F: Fn(&rusqlite::Transaction<'_>) -> Result<T, rusqlite::Error>,
{
    with_retry(
        || {
            let mut conn = pool.get().map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    Some(e.to_string()),
                )
            })?;

            // Use IMMEDIATE transaction to acquire write lock immediately
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        },
        MAX_RETRIES,
    )
    .map_err(map_db_error)
}

// Transaction support implementation
use crate::DatabaseExt;

impl DatabaseExt for SqliteDatabase {
    fn with_transaction<F, T>(&self, f: F) -> stateset_core::Result<T>
    where
        F: FnMut(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>,
    {
        self.with_transaction_opts(crate::TransactionOptions::new(), f)
    }

    fn with_transaction_opts<F, T>(
        &self,
        opts: crate::TransactionOptions,
        mut f: F,
    ) -> stateset_core::Result<T>
    where
        F: FnMut(&rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>,
    {
        let retries = if opts.retry_on_conflict { opts.max_retries } else { 0 };
        let timeout_ms = opts.timeout_ms.unwrap_or(crate::DEFAULT_TRANSACTION_TIMEOUT_MS);
        let set_read_uncommitted =
            matches!(opts.isolation, crate::TransactionIsolation::ReadUncommitted);
        let fallback_timeout_ms = timeout_ms;

        crate::sqlite::with_retry(
            || {
                let mut conn = self.pool.get().map_err(|e| {
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                        Some(e.to_string()),
                    )
                })?;
                let previous_timeout: u64 = conn
                    .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, u64>(0))
                    .unwrap_or(fallback_timeout_ms);

                conn.execute_batch(&format!("PRAGMA busy_timeout = {}", timeout_ms))?;

                let previous_read_uncommitted = if set_read_uncommitted {
                    let previous: i64 = conn
                        .query_row("PRAGMA read_uncommitted", [], |row| row.get::<_, i64>(0))
                        .unwrap_or(0);
                    let previous_read_uncommitted = previous == 1;
                    conn.execute_batch("PRAGMA read_uncommitted = true")?;
                    Some(previous_read_uncommitted)
                } else {
                    None
                };

                let result = {
                    let tx =
                        conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                    let result = panic::catch_unwind(AssertUnwindSafe(|| f(&tx)));

                    match result {
                        Ok(result) => {
                            let result = result?;
                            tx.commit()?;
                            Ok(result)
                        }
                        Err(panic_payload) => {
                            panic::resume_unwind(panic_payload);
                        }
                    }
                };

                let _ = conn.execute_batch(&format!("PRAGMA busy_timeout = {}", previous_timeout));
                if let Some(previous_read_uncommitted) = previous_read_uncommitted {
                    let _ = conn.execute_batch(&format!(
                        "PRAGMA read_uncommitted = {}",
                        if previous_read_uncommitted { "true" } else { "false" }
                    ));
                }

                result
            },
            retries,
        )
        .map_err(map_db_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn retryable_error() -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".to_string()),
        )
    }

    #[test]
    fn build_in_clause_empty_is_null() {
        assert_eq!(build_in_clause(0), "NULL".to_string());
    }

    #[test]
    fn build_in_clause_uses_question_mark_placeholders() {
        assert_eq!(build_in_clause(3), "?, ?, ?".to_string());
    }

    #[test]
    fn with_transaction_restores_read_uncommitted_pragma() {
        let db = SqliteDatabase::new(&DatabaseConfig {
            url: ":memory:".to_string(),
            max_connections: 1,
        })
        .expect("db should initialize");

        {
            let conn = db.conn().expect("connection should open");
            conn.execute_batch("PRAGMA read_uncommitted = true")
                .expect("read_uncommitted pragma should be set");
            let before: i64 = conn
                .query_row("PRAGMA read_uncommitted", [], |row| row.get::<_, i64>(0))
                .expect("read_uncommitted pragma should be readable");
            assert_eq!(before, 1);
        }

        db.with_transaction_opts(
            crate::TransactionOptions::new()
                .isolation(crate::TransactionIsolation::ReadUncommitted),
            |conn| {
                conn.execute_batch("PRAGMA read_uncommitted = true")?;
                Ok(())
            },
        )
        .expect("transaction should succeed");

        let conn = db.conn().expect("connection should reopen");
        let after: i64 = conn
            .query_row("PRAGMA read_uncommitted", [], |row| row.get::<_, i64>(0))
            .expect("read_uncommitted pragma should be readable");
        assert_eq!(after, 1);
    }

    #[test]
    fn with_retry_respects_zero_retries() {
        let attempts = Cell::new(0u32);
        let err: std::result::Result<(), rusqlite::Error> = with_retry(
            || {
                attempts.set(attempts.get() + 1);
                Err(retryable_error())
            },
            0,
        );

        assert!(err.is_err());
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn with_retry_retries_until_success() {
        let attempts = Cell::new(0u32);
        let result = with_retry(
            || {
                let attempt = attempts.get() + 1;
                attempts.set(attempt);
                if attempt < 3 { Err(retryable_error()) } else { Ok(attempt) }
            },
            5,
        )
        .expect("operation should succeed after retries");

        assert_eq!(result, 3);
        assert_eq!(attempts.get(), 3);
    }
}
