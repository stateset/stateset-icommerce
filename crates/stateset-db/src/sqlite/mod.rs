//! SQLite database implementation

mod a2a;
mod a2a_credit_terms;
mod a2a_messaging;
mod accounts_payable;
mod accounts_receivable;
mod activity_logs;
mod agent_cards;
mod agent_identities;
mod agent_reputation;
mod agent_validation;
mod analytics;
mod backorder;
mod bins;
mod bom;
mod carts;
mod channels;
mod companies;
mod cost_accounting;
mod credit;
mod currency;
mod custom_objects;
mod customers;
mod edi_documents;
mod fixed_assets;
mod fraud;
mod fulfillment;
mod general_ledger;
mod gift_cards;
mod http_idempotency;
mod inbound_shipments;
mod integration_field_mappings;
mod integration_mappings;
mod inventory;
mod invoices;
mod kernel_executor;
mod kernel_outbox;
mod lots;
mod loyalty;
mod money_agg;
mod orders;
pub(crate) mod parse_helpers;
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
mod rewards;
mod search_configs;
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
mod x402_credits;
mod x402_payment_intents;
mod zone_shipping_methods;

#[cfg(feature = "vector")]
mod vector;

pub use a2a::*;
pub use a2a_credit_terms::*;
pub use a2a_messaging::*;
pub use accounts_payable::*;
pub use accounts_receivable::*;
pub use activity_logs::*;
pub use agent_cards::*;
pub use agent_identities::*;
pub use agent_reputation::*;
pub use agent_validation::*;
pub use analytics::*;
pub use backorder::*;
pub use bins::*;
pub use bom::*;
pub use carts::*;
pub use channels::*;
pub use companies::*;
pub use cost_accounting::*;
pub use credit::*;
pub use currency::*;
pub use custom_objects::*;
pub use customers::*;
pub use edi_documents::*;
pub use fixed_assets::*;
pub use fraud::*;
pub use fulfillment::*;
pub use general_ledger::*;
pub use gift_cards::*;
pub use http_idempotency::*;
pub use inbound_shipments::*;
pub use integration_field_mappings::*;
pub use integration_mappings::*;
pub use inventory::*;
pub use invoices::*;
pub use kernel_executor::*;
pub use kernel_outbox::*;
pub use lots::*;
pub use loyalty::*;
pub use orders::*;
pub use payment_obligations::*;
pub use payments::*;
pub use prepayments::*;
pub use price_levels::*;
pub use price_schedules::*;
pub use print_stations::*;
pub use production_batches::*;
pub use products::*;
pub use promotions::*;
pub use purchase_orders::*;
pub use purgatory::*;
pub use quality::*;
pub use receiving::*;
pub use returns::*;
pub use revenue_recognition::*;
pub use reviews::*;
pub use rewards::*;
pub use search_configs::*;
pub use segments::*;
pub use serials::*;
pub use shipments::*;
pub use shipping_zones::*;
pub use stock_snapshots::*;
pub use store_credits::*;
pub use subscriptions::*;
pub use supplier_skus::*;
pub use tax::*;
pub use topology_snapshots::*;
pub use transfer_orders::*;
pub use units_of_measure::*;
#[cfg(feature = "vector")]
pub use vector::*;
pub use vendor_credits::*;
pub use vendor_returns::*;
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

/// Best-effort cleanup for `:memory:` databases, which are backed by private
/// temp files (see the pool construction below for why).
#[derive(Debug)]
struct EphemeralDbGuard {
    path: std::path::PathBuf,
}

impl Drop for EphemeralDbGuard {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = self.path.as_os_str().to_owned();
            p.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(p));
        }
    }
}

/// Database health status returned by [`SqliteDatabase::health_check`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct DbHealthStatus {
    /// Round-trip latency to DB in milliseconds
    pub latency_ms: u64,
    /// Total connections in pool
    pub pool_size: u32,
    /// Idle connections available
    pub pool_idle: u32,
    /// Total database file size in bytes
    pub db_size_bytes: u64,
    /// Free pages available for reuse
    pub freelist_pages: u64,
}

#[derive(Debug)]
struct PragmaScope {
    conn: PooledConnection<SqliteConnectionManager>,
    previous_timeout: u64,
    previous_read_uncommitted: Option<bool>,
}

impl PragmaScope {
    fn new(
        conn: PooledConnection<SqliteConnectionManager>,
        timeout_ms: u64,
        set_read_uncommitted: bool,
        fallback_timeout_ms: u64,
    ) -> Result<Self, rusqlite::Error> {
        let previous_timeout: u64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, u64>(0))
            .unwrap_or(fallback_timeout_ms);

        conn.execute_batch(&format!("PRAGMA busy_timeout = {timeout_ms}"))?;

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

        Ok(Self { conn, previous_timeout, previous_read_uncommitted })
    }
}

impl std::ops::Deref for PragmaScope {
    type Target = PooledConnection<SqliteConnectionManager>;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl std::ops::DerefMut for PragmaScope {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.conn
    }
}

impl Drop for PragmaScope {
    fn drop(&mut self) {
        let _ =
            self.conn.execute_batch(&format!("PRAGMA busy_timeout = {}", self.previous_timeout));
        if let Some(previous_read_uncommitted) = self.previous_read_uncommitted {
            let _ = self.conn.execute_batch(&format!(
                "PRAGMA read_uncommitted = {}",
                if previous_read_uncommitted { "true" } else { "false" }
            ));
        }
    }
}

impl SqliteDatabase {
    /// Create a new SQLite database connection
    pub fn new(config: &DatabaseConfig) -> Result<Self, CommerceError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::Duration;

        // Counter to generate unique database names for each in-memory instance
        static MEMORY_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

        // `:memory:` databases are backed by a PRIVATE TEMP FILE, not SQLite
        // shared-cache. Shared-cache (the only way a connection pool can see
        // one true in-memory database) uses table-level locks whose semantics
        // differ from file databases — BEGIN IMMEDIATE does not fully
        // serialize read-modify-write transactions there, which surfaced as a
        // rare lost update in the weighted-average-cost concurrency test. A
        // temp file gets the exact production locking model (WAL, IMMEDIATE
        // serialization, full pool concurrency); on Linux, tmp is typically
        // RAM-backed anyway. The files are deleted when the last handle drops.
        let is_memory = config.url == ":memory:";
        let mut ephemeral = None;
        let (manager, max_connections) = if is_memory {
            let db_id = MEMORY_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("stateset_memdb_{}_{db_id}.db", std::process::id()));
            ephemeral = Some(std::sync::Arc::new(EphemeralDbGuard { path: path.clone() }));
            let manager = SqliteConnectionManager::file(&path).with_flags(
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
            );
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
            // Keep the ephemeral-file guard alive as long as the POOL does
            // (the init closure lives inside the connection manager). Tying
            // it to `SqliteDatabase` instead deleted the temp file while
            // accessor repos still held pool clones — lazily opened
            // connections then recreated an EMPTY database ("no such table").
            let _keep_ephemeral_alive = &ephemeral;
            // Use longer busy_timeout for high concurrency scenarios
            conn.execute_batch(&format!(
                "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = {};",
                crate::DEFAULT_TRANSACTION_TIMEOUT_MS
            ))?;
            conn.execute_batch("PRAGMA journal_mode = WAL;")?;
            // Performance tuning: reduce fsync overhead and increase cache
            conn.execute_batch(
                "PRAGMA synchronous = NORMAL;\
                 PRAGMA cache_size = -16000;\
                 PRAGMA temp_store = MEMORY;\
                 PRAGMA mmap_size = 268435456;\
                 PRAGMA wal_autocheckpoint = 10000;",
            )?;
            // Exact-decimal money aggregate (decimal_sum) for analytics totals;
            // see money_agg for why SQL's float SUM() is unsafe on TEXT money.
            money_agg::register(conn)?;
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

    /// Run a health check: verify DB connectivity, return pool and storage stats.
    pub fn health_check(&self) -> Result<DbHealthStatus, CommerceError> {
        let start = std::time::Instant::now();
        let conn = self.conn()?;
        // Verify DB is responsive with a trivial query
        let _: i64 = conn
            .query_row("SELECT 1", [], |row| row.get(0))
            .map_err(|e| CommerceError::DatabaseError(format!("Health check query failed: {e}")))?;

        // Get storage stats (page_count * page_size = total DB size)
        let page_count: i64 =
            conn.query_row("PRAGMA page_count", [], |row| row.get(0)).unwrap_or(0);
        let page_size: i64 =
            conn.query_row("PRAGMA page_size", [], |row| row.get(0)).unwrap_or(4096);
        let freelist_count: i64 =
            conn.query_row("PRAGMA freelist_count", [], |row| row.get(0)).unwrap_or(0);

        let pool_state = self.pool.state();

        Ok(DbHealthStatus {
            latency_ms: start.elapsed().as_millis() as u64,
            pool_size: pool_state.connections,
            pool_idle: pool_state.idle_connections,
            db_size_bytes: (page_count * page_size) as u64,
            freelist_pages: freelist_count as u64,
        })
    }

    /// Graceful shutdown: checkpoint WAL and optimize before closing.
    ///
    /// Call this before dropping the database to ensure all WAL data is
    /// flushed to the main database file. Safe to call multiple times.
    pub fn shutdown(&self) -> Result<(), CommerceError> {
        let conn = self.conn()?;
        // Checkpoint WAL to main database file
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .map_err(|e| CommerceError::DatabaseError(format!("WAL checkpoint failed: {e}")))?;
        // Run optimize to update query planner statistics
        conn.execute_batch("PRAGMA optimize")
            .map_err(|e| CommerceError::DatabaseError(format!("PRAGMA optimize failed: {e}")))?;
        tracing::info!("Database shutdown: WAL checkpointed and optimized");
        Ok(())
    }

    /// Get order repository
    #[must_use]
    pub fn orders(&self) -> SqliteOrderRepository {
        SqliteOrderRepository::new(self.pool.clone())
    }

    /// Get inventory repository
    #[must_use]
    pub fn inventory(&self) -> SqliteInventoryRepository {
        SqliteInventoryRepository::new(self.pool.clone())
    }

    /// Get customer repository
    #[must_use]
    pub fn customers(&self) -> SqliteCustomerRepository {
        SqliteCustomerRepository::new(self.pool.clone())
    }

    /// Get product repository
    #[must_use]
    pub fn products(&self) -> SqliteProductRepository {
        SqliteProductRepository::new(self.pool.clone())
    }

    /// Get custom objects repository (custom states / metaobjects)
    #[must_use]
    pub fn custom_objects(&self) -> SqliteCustomObjectRepository {
        SqliteCustomObjectRepository::new(self.pool.clone())
    }

    /// Get return repository
    #[must_use]
    pub fn returns(&self) -> SqliteReturnRepository {
        SqliteReturnRepository::new(self.pool.clone())
    }

    /// Get BOM (Bill of Materials) repository
    #[must_use]
    pub fn bom(&self) -> SqliteBomRepository {
        SqliteBomRepository::new(self.pool.clone())
    }

    /// Get work order repository
    #[must_use]
    pub fn work_orders(&self) -> SqliteWorkOrderRepository {
        SqliteWorkOrderRepository::new(self.pool.clone())
    }

    /// Get shipment repository
    #[must_use]
    pub fn shipments(&self) -> SqliteShipmentRepository {
        SqliteShipmentRepository::new(self.pool.clone())
    }

    /// Get payment repository
    #[must_use]
    pub fn payments(&self) -> SqlitePaymentRepository {
        SqlitePaymentRepository::new(self.pool.clone())
    }

    /// Get the durable kernel outbox consumer API.
    #[must_use]
    pub fn kernel_outbox(&self) -> SqliteKernelOutboxRepository {
        SqliteKernelOutboxRepository::new(self.pool.clone())
    }

    /// Create an envelope-aware executor governed by the supplied policy revision.
    #[must_use]
    pub fn kernel_executor(&self, policy: stateset_core::KernelPolicy) -> SqliteKernelExecutor {
        SqliteKernelExecutor::new(self.pool.clone(), policy)
    }

    /// Get warranty repository
    #[must_use]
    pub fn warranties(&self) -> SqliteWarrantyRepository {
        SqliteWarrantyRepository::new(self.pool.clone())
    }

    /// Get purchase order repository
    #[must_use]
    pub fn purchase_orders(&self) -> SqlitePurchaseOrderRepository {
        SqlitePurchaseOrderRepository::new(self.pool.clone())
    }

    /// Get invoice repository
    #[must_use]
    pub fn invoices(&self) -> SqliteInvoiceRepository {
        SqliteInvoiceRepository::new(self.pool.clone())
    }

    /// Get cart repository
    #[must_use]
    pub fn carts(&self) -> SqliteCartRepository {
        SqliteCartRepository::new(self.pool.clone())
    }

    /// Get analytics repository
    #[must_use]
    pub fn analytics(&self) -> SqliteAnalyticsRepository {
        SqliteAnalyticsRepository::new(self.pool.clone())
    }

    /// Get currency repository
    #[must_use]
    pub fn currency(&self) -> SqliteCurrencyRepository {
        SqliteCurrencyRepository::new(self.pool.clone())
    }

    /// Get tax repository
    #[must_use]
    pub fn tax(&self) -> SqliteTaxRepository {
        SqliteTaxRepository::new(self.pool.clone())
    }

    /// Get promotions repository
    #[must_use]
    pub fn promotions(&self) -> SqlitePromotionRepository {
        SqlitePromotionRepository::new(self.pool.clone())
    }

    /// Get subscriptions repository
    #[must_use]
    pub fn subscriptions(&self) -> SqliteSubscriptionRepository {
        SqliteSubscriptionRepository::new(self.pool.clone())
    }

    /// Get quality repository
    #[must_use]
    pub fn quality(&self) -> SqliteQualityRepository {
        SqliteQualityRepository::new(self.pool.clone())
    }

    /// Get lots repository
    #[must_use]
    pub fn lots(&self) -> SqliteLotRepository {
        SqliteLotRepository::new(self.pool.clone())
    }

    /// Get serials repository
    #[must_use]
    pub fn serials(&self) -> SqliteSerialRepository {
        SqliteSerialRepository::new(self.pool.clone())
    }

    /// Get warehouse bin repository
    #[must_use]
    pub fn bins(&self) -> SqliteBinRepository {
        SqliteBinRepository::new(self.pool.clone())
    }

    /// Get warehouse repository
    #[must_use]
    pub fn warehouse(&self) -> SqliteWarehouseRepository {
        SqliteWarehouseRepository::new(self.pool.clone())
    }

    /// Get receiving repository
    #[must_use]
    pub fn receiving(&self) -> SqliteReceivingRepository {
        SqliteReceivingRepository::new(self.pool.clone())
    }

    /// Get fulfillment repository
    #[must_use]
    pub fn fulfillment(&self) -> SqliteFulfillmentRepository {
        SqliteFulfillmentRepository::new(self.pool.clone())
    }

    /// Get accounts payable repository
    #[must_use]
    pub fn accounts_payable(&self) -> SqliteAccountsPayableRepository {
        SqliteAccountsPayableRepository::new(self.pool.clone())
    }

    /// Get cost accounting repository
    #[must_use]
    pub fn cost_accounting(&self) -> SqliteCostAccountingRepository {
        SqliteCostAccountingRepository::new(self.pool.clone())
    }

    /// Get credit repository
    #[must_use]
    pub fn credit(&self) -> SqliteCreditRepository {
        SqliteCreditRepository::new(self.pool.clone())
    }

    /// Get backorder repository
    #[must_use]
    pub fn backorder(&self) -> SqliteBackorderRepository {
        SqliteBackorderRepository::new(self.pool.clone())
    }

    /// Get accounts receivable repository
    #[must_use]
    pub fn accounts_receivable(&self) -> SqliteAccountsReceivableRepository {
        SqliteAccountsReceivableRepository::new(self.pool.clone())
    }

    /// Get general ledger repository
    #[must_use]
    pub fn general_ledger(&self) -> SqliteGeneralLedgerRepository {
        SqliteGeneralLedgerRepository::new(self.pool.clone())
    }

    /// Get vector search repository (requires `vector` feature)
    #[cfg(feature = "vector")]
    pub fn vector(&self) -> SqliteVectorRepository {
        SqliteVectorRepository::new(self.pool.clone())
    }

    /// Get x402 payment intent repository
    #[must_use]
    pub fn x402_payment_intents(&self) -> SqliteX402PaymentIntentRepository {
        SqliteX402PaymentIntentRepository::new(self.pool.clone())
    }

    /// Get x402 credit ledger repository
    #[must_use]
    pub fn x402_credits(&self) -> SqliteX402CreditRepository {
        SqliteX402CreditRepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    #[must_use]
    pub fn a2a_quotes(&self) -> SqliteA2ARepository {
        SqliteA2ARepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    #[must_use]
    pub fn a2a_purchases(&self) -> SqliteA2ARepository {
        SqliteA2ARepository::new(self.pool.clone())
    }

    /// Get durable A2A credit terms repository
    #[must_use]
    pub fn a2a_credit_terms(&self) -> SqliteA2ACreditTermsRepository {
        SqliteA2ACreditTermsRepository::new(self.pool.clone())
    }

    /// Get durable A2A agent messaging repository
    #[must_use]
    pub fn a2a_messages(&self) -> SqliteA2AMessagingRepository {
        SqliteA2AMessagingRepository::new(self.pool.clone())
    }

    /// Get agent card repository
    #[must_use]
    pub fn agent_cards(&self) -> SqliteAgentCardRepository {
        SqliteAgentCardRepository::new(self.pool.clone())
    }

    /// Get agent identity repository (ERC-8004)
    #[must_use]
    pub fn agent_identities(&self) -> SqliteAgentIdentityRepository {
        SqliteAgentIdentityRepository::new(self.pool.clone())
    }

    /// Get agent reputation repository (ERC-8004)
    #[must_use]
    pub fn agent_reputation(&self) -> SqliteAgentReputationRepository {
        SqliteAgentReputationRepository::new(self.pool.clone())
    }

    /// Get agent validation repository (ERC-8004)
    #[must_use]
    pub fn agent_validation(&self) -> SqliteAgentValidationRepository {
        SqliteAgentValidationRepository::new(self.pool.clone())
    }

    /// Get gift card repository
    #[must_use]
    pub fn gift_cards(&self) -> SqliteGiftCardRepository {
        SqliteGiftCardRepository::new(self.pool.clone())
    }

    /// Get store credit repository
    #[must_use]
    pub fn store_credits(&self) -> SqliteStoreCreditRepository {
        SqliteStoreCreditRepository::new(self.pool.clone())
    }

    /// Get customer segment repository
    #[must_use]
    pub fn segments(&self) -> SqliteSegmentRepository {
        SqliteSegmentRepository::new(self.pool.clone())
    }

    /// Get shipping zone repository
    #[must_use]
    pub fn shipping_zones(&self) -> SqliteShippingZoneRepository {
        SqliteShippingZoneRepository::new(self.pool.clone())
    }

    /// Get channel repository
    #[must_use]
    pub fn channels(&self) -> SqliteChannelRepository {
        SqliteChannelRepository::new(self.pool.clone())
    }

    /// Get company (B2B account) repository
    #[must_use]
    pub fn companies(&self) -> SqliteCompanyRepository {
        SqliteCompanyRepository::new(self.pool.clone())
    }

    /// Get transfer order repository
    #[must_use]
    pub fn transfer_orders(&self) -> SqliteTransferOrderRepository {
        SqliteTransferOrderRepository::new(self.pool.clone())
    }

    /// Get units-of-measure repository
    #[must_use]
    pub fn units_of_measure(&self) -> SqliteUnitOfMeasureRepository {
        SqliteUnitOfMeasureRepository::new(self.pool.clone())
    }

    /// Get production batch repository
    #[must_use]
    pub fn production_batches(&self) -> SqliteProductionBatchRepository {
        SqliteProductionBatchRepository::new(self.pool.clone())
    }

    /// Get supplier SKU repository
    #[must_use]
    pub fn supplier_skus(&self) -> SqliteSupplierSkuRepository {
        SqliteSupplierSkuRepository::new(self.pool.clone())
    }

    /// Get fixed asset repository
    #[must_use]
    pub fn fixed_assets(&self) -> SqliteFixedAssetRepository {
        SqliteFixedAssetRepository::new(self.pool.clone())
    }

    /// Get revenue recognition repository
    #[must_use]
    pub fn revenue_recognition(&self) -> SqliteRevenueRecognitionRepository {
        SqliteRevenueRecognitionRepository::new(self.pool.clone())
    }

    /// Get vendor return repository
    #[must_use]
    pub fn vendor_returns(&self) -> SqliteVendorReturnRepository {
        SqliteVendorReturnRepository::new(self.pool.clone())
    }

    /// Get vendor credit repository
    #[must_use]
    pub fn vendor_credits(&self) -> SqliteVendorCreditRepository {
        SqliteVendorCreditRepository::new(self.pool.clone())
    }

    /// Get payment obligation repository
    #[must_use]
    pub fn payment_obligations(&self) -> SqlitePaymentObligationRepository {
        SqlitePaymentObligationRepository::new(self.pool.clone())
    }

    /// Get price level repository
    #[must_use]
    pub fn price_levels(&self) -> SqlitePriceLevelRepository {
        SqlitePriceLevelRepository::new(self.pool.clone())
    }

    /// Get prepayment repository
    #[must_use]
    pub fn prepayments(&self) -> SqlitePrepaymentRepository {
        SqlitePrepaymentRepository::new(self.pool.clone())
    }

    /// Get price schedule repository
    #[must_use]
    pub fn price_schedules(&self) -> SqlitePriceScheduleRepository {
        SqlitePriceScheduleRepository::new(self.pool.clone())
    }

    /// Get the durable HTTP idempotency repository
    #[must_use]
    pub fn http_idempotency(&self) -> SqliteHttpIdempotencyRepository {
        SqliteHttpIdempotencyRepository::new(self.pool.clone())
    }

    /// Get activity log repository
    #[must_use]
    pub fn activity_logs(&self) -> SqliteActivityLogRepository {
        SqliteActivityLogRepository::new(self.pool.clone())
    }

    /// Get integration mapping repository
    #[must_use]
    pub fn integration_mappings(&self) -> SqliteIntegrationMappingRepository {
        SqliteIntegrationMappingRepository::new(self.pool.clone())
    }

    /// Get inbound shipment repository
    #[must_use]
    pub fn inbound_shipments(&self) -> SqliteInboundShipmentRepository {
        SqliteInboundShipmentRepository::new(self.pool.clone())
    }

    /// Get purgatory (order ingestion staging) repository
    #[must_use]
    pub fn purgatory(&self) -> SqlitePurgatoryRepository {
        SqlitePurgatoryRepository::new(self.pool.clone())
    }

    /// Get print station repository
    #[must_use]
    pub fn print_stations(&self) -> SqlitePrintStationRepository {
        SqlitePrintStationRepository::new(self.pool.clone())
    }

    /// Get EDI document repository
    #[must_use]
    pub fn edi_documents(&self) -> SqliteEdiDocumentRepository {
        SqliteEdiDocumentRepository::new(self.pool.clone())
    }

    /// Get integration field-mapping repository
    #[must_use]
    pub fn integration_field_mappings(&self) -> SqliteIntegrationFieldMappingRepository {
        SqliteIntegrationFieldMappingRepository::new(self.pool.clone())
    }

    /// Get topology snapshot repository
    #[must_use]
    pub fn topology_snapshots(&self) -> SqliteTopologySnapshotRepository {
        SqliteTopologySnapshotRepository::new(self.pool.clone())
    }

    /// Get stock snapshot repository
    #[must_use]
    pub fn stock_snapshots(&self) -> SqliteStockSnapshotRepository {
        SqliteStockSnapshotRepository::new(self.pool.clone())
    }

    /// Get zone shipping method repository
    #[must_use]
    pub fn zone_shipping_methods(&self) -> SqliteZoneShippingMethodRepository {
        SqliteZoneShippingMethodRepository::new(self.pool.clone())
    }

    /// Get product review repository
    #[must_use]
    pub fn reviews(&self) -> SqliteReviewRepository {
        SqliteReviewRepository::new(self.pool.clone())
    }

    /// Get wishlist repository
    #[must_use]
    pub fn wishlists(&self) -> SqliteWishlistRepository {
        SqliteWishlistRepository::new(self.pool.clone())
    }

    /// Get loyalty program repository
    #[must_use]
    pub fn loyalty_programs(&self) -> SqliteLoyaltyProgramRepository {
        SqliteLoyaltyProgramRepository::new(self.pool.clone())
    }

    /// Get reward catalog repository
    #[must_use]
    pub fn rewards(&self) -> SqliteRewardRepository {
        SqliteRewardRepository::new(self.pool.clone())
    }

    /// Get fraud detection repository
    #[must_use]
    pub fn fraud(&self) -> SqliteFraudRepository {
        SqliteFraudRepository::new(self.pool.clone())
    }

    /// Get search configuration repository
    #[must_use]
    pub fn search_configs(&self) -> SqliteSearchConfigRepository {
        SqliteSearchConfigRepository::new(self.pool.clone())
    }

    /// Get underlying pool (for advanced use)
    #[must_use]
    pub const fn pool(&self) -> &Pool<SqliteConnectionManager> {
        &self.pool
    }
}

/// The values a write is carrying, so a unique-constraint violation can be
/// reported with the offending slug / SKU / e-mail instead of the column name.
///
/// SQLite's `UNIQUE constraint failed: <table>.<column>` message never
/// contains the value, unlike Postgres' `Key (sku)=(…) already exists`, so the
/// call site has to supply it. Fields left `None` fall back to the column name.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ConflictValues<'a> {
    /// Value written to `products.slug`.
    pub slug: Option<&'a str>,
    /// Value written to `product_variants.sku`.
    pub sku: Option<&'a str>,
    /// Value written to `customers.email` / `customers.email_key`.
    pub email: Option<&'a str>,
}

impl<'a> ConflictValues<'a> {
    /// A context naming only the slug in flight.
    pub(crate) const fn slug(slug: &'a str) -> Self {
        Self { slug: Some(slug), sku: None, email: None }
    }

    /// A context naming only the SKU in flight.
    pub(crate) const fn sku(sku: &'a str) -> Self {
        Self { slug: None, sku: Some(sku), email: None }
    }

    /// A context naming only the e-mail in flight.
    pub(crate) const fn email(email: &'a str) -> Self {
        Self { slug: None, sku: None, email: Some(email) }
    }
}

/// Helper function to convert rusqlite errors to `CommerceError`
pub(crate) fn map_db_error(e: rusqlite::Error) -> CommerceError {
    map_db_error_with(e, ConflictValues::default())
}

/// [`map_db_error`] with the offending values for typed conflicts.
pub(crate) fn map_db_error_with(e: rusqlite::Error, values: ConflictValues<'_>) -> CommerceError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
        rusqlite::Error::ToSqlConversionFailure(boxed) => {
            // Extract CommerceError if it was wrapped for transaction propagation
            match boxed.downcast::<CommerceError>() {
                Ok(commerce_error) => *commerce_error,
                Err(other) => CommerceError::DatabaseError(other.to_string()),
            }
        }
        rusqlite::Error::SqliteFailure(err, _) => {
            match err.code {
                // SQLITE_CONSTRAINT (19): unique/fk/check violation
                rusqlite::ErrorCode::ConstraintViolation => {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        map_unique_constraint_message(&msg, values)
                    } else {
                        CommerceError::ValidationError(msg)
                    }
                }
                _ => {
                    // SQLITE_FULL = 13
                    if err.extended_code == rusqlite::ffi::SQLITE_FULL {
                        CommerceError::Internal("Storage full: database disk is full".into())
                    } else {
                        CommerceError::DatabaseError(e.to_string())
                    }
                }
            }
        }
        _ => CommerceError::DatabaseError(e.to_string()),
    }
}

/// Map a SQLite `UNIQUE constraint failed: <table>.<column>` message onto the
/// typed conflict the repositories promise for that column.
///
/// The pre-checks in the product / customer repositories normally surface
/// these conflicts before the INSERT; this mapping is the backstop for the
/// window between check and write (and for writers that skip the check) so a
/// raced duplicate is still a `DuplicateSlug` / `DuplicateSku` /
/// `EmailAlreadyExists` rather than an untyped `Conflict`.
///
/// `values` carries the offending slug / SKU / e-mail from the call site so
/// the error names the value the caller tried to write (SQLite's message only
/// names the column); it falls back to the column name when unset.
pub(crate) fn map_unique_constraint_message(
    msg: &str,
    values: ConflictValues<'_>,
) -> CommerceError {
    let column =
        msg.rsplit_once("UNIQUE constraint failed:").map(|(_, rest)| rest.trim()).unwrap_or("");
    let payload = |value: Option<&str>| value.unwrap_or(column).to_string();
    if column.starts_with("products.slug") {
        CommerceError::DuplicateSlug(payload(values.slug))
    } else if column.starts_with("product_variants.sku") {
        CommerceError::DuplicateSku(payload(values.sku))
    } else if column.starts_with("customers.email") {
        CommerceError::EmailAlreadyExists(payload(values.email))
    } else {
        CommerceError::Conflict(msg.to_string())
    }
}

#[cfg(test)]
mod unique_constraint_mapping_tests {
    use super::{ConflictValues, map_unique_constraint_message};
    use stateset_core::CommerceError;

    #[test]
    fn maps_product_slug_to_duplicate_slug() {
        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: products.slug",
            ConflictValues::default(),
        );
        assert!(matches!(err, CommerceError::DuplicateSlug(_)), "{err:?}");
    }

    #[test]
    fn maps_variant_sku_to_duplicate_sku() {
        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: product_variants.sku",
            ConflictValues::default(),
        );
        assert!(matches!(err, CommerceError::DuplicateSku(_)), "{err:?}");
    }

    #[test]
    fn maps_customer_email_and_email_key_to_email_already_exists() {
        for msg in [
            "UNIQUE constraint failed: customers.email",
            "UNIQUE constraint failed: customers.email_key",
        ] {
            let err = map_unique_constraint_message(msg, ConflictValues::default());
            assert!(matches!(err, CommerceError::EmailAlreadyExists(_)), "{err:?}");
        }
    }

    #[test]
    fn unknown_unique_stays_untyped_conflict() {
        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: orders.order_number",
            ConflictValues::default(),
        );
        assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    }

    #[test]
    fn typed_conflicts_carry_the_offending_value_when_supplied() {
        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: products.slug",
            ConflictValues::slug("premium-widget"),
        );
        assert!(
            matches!(&err, CommerceError::DuplicateSlug(value) if value == "premium-widget"),
            "{err:?}"
        );

        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: product_variants.sku",
            ConflictValues::sku("WIDGET-001"),
        );
        assert!(
            matches!(&err, CommerceError::DuplicateSku(value) if value == "WIDGET-001"),
            "{err:?}"
        );

        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: customers.email_key",
            ConflictValues::email("ada@example.com"),
        );
        assert!(
            matches!(&err, CommerceError::EmailAlreadyExists(value) if value == "ada@example.com"),
            "{err:?}"
        );
    }

    #[test]
    fn typed_conflicts_fall_back_to_the_column_name_without_a_value() {
        let err = map_unique_constraint_message(
            "UNIQUE constraint failed: products.slug",
            // A SKU in flight says nothing about a slug collision.
            ConflictValues::sku("WIDGET-001"),
        );
        assert!(
            matches!(&err, CommerceError::DuplicateSlug(value) if value == "products.slug"),
            "{err:?}"
        );
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

/// Escape LIKE wildcard characters (%, _, \) in a search string.
///
/// Prevents user-supplied search terms from acting as LIKE metacharacters.
/// Use with `LIKE ? ESCAPE '\'` in the SQL query.
#[must_use]
pub(crate) fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

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

/// Default page size applied when a list filter does not specify a limit.
pub(crate) const DEFAULT_LIST_LIMIT: u32 = 500;

/// Hard server-side ceiling on requested page sizes.
pub(crate) const MAX_LIST_LIMIT: u32 = 1000;

/// Clamp a requested page size to the server-side pagination policy:
/// `None` becomes [`DEFAULT_LIST_LIMIT`], and anything above
/// [`MAX_LIST_LIMIT`] is capped to it.
pub(crate) const fn effective_limit(limit: Option<u32>) -> u32 {
    match limit {
        Some(limit) if limit > MAX_LIST_LIMIT => MAX_LIST_LIMIT,
        Some(limit) => limit,
        None => DEFAULT_LIST_LIMIT,
    }
}

/// Append a `LIMIT`/`OFFSET` pagination clause to a query string.
///
/// A `LIMIT` is always emitted: when the filter supplies no limit the
/// server-side default ([`DEFAULT_LIST_LIMIT`]) applies, and requested limits
/// are capped at [`MAX_LIST_LIMIT`] — unbounded scans are never produced.
pub(crate) fn append_limit_offset<O: std::fmt::Display>(
    sql: &mut String,
    limit: Option<u32>,
    offset: Option<O>,
) {
    let limit = effective_limit(limit);
    match offset {
        Some(offset) => sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}")),
        None => sql.push_str(&format!(" LIMIT {limit}")),
    }
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
    params.iter().map(std::convert::AsRef::as_ref).collect()
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
/// Per-retry backoff jitter in milliseconds (0..50), from a thread-local
/// xorshift PRNG. Each thread seeds from `RandomState` so concurrent
/// contenders draw decorrelated sequences instead of backing off in lockstep
/// and re-colliding every round.
pub(crate) fn retry_jitter(retries: u32) -> u64 {
    use std::cell::Cell;
    use std::hash::{BuildHasher, Hasher, RandomState};

    thread_local! {
        static SEED: Cell<u64> = Cell::new({
            // RandomState carries fresh per-instance entropy, giving every
            // thread its own seed. Avoid 0, the xorshift fixed point.
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u64(0xa076_1d64_78bd_642f);
            hasher.finish().max(1)
        });
    }

    SEED.with(|seed| {
        let mut s = seed.get().wrapping_add(u64::from(retries));
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        seed.set(s);
        s % 50
    })
}

pub(crate) fn with_retry<T, F>(mut f: F, max_retries: u32) -> Result<T, rusqlite::Error>
where
    F: FnMut() -> Result<T, rusqlite::Error>,
{
    let mut retries = 0;
    let mut backoff_ms = INITIAL_BACKOFF_MS;

    loop {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if is_retryable_error(&e) && retries < max_retries => {
                retries += 1;
                let jitter = retry_jitter(retries);
                let delay = backoff_ms.min(MAX_BACKOFF_MS) + jitter;
                thread::sleep(Duration::from_millis(delay));
                // Exponential backoff with cap
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Threshold above which transactions are logged as slow (milliseconds).
const SLOW_QUERY_THRESHOLD_MS: u128 = 500;

/// Begin an IMMEDIATE-mode write transaction on an existing pooled connection.
///
/// Thin wrapper used by store write paths whose control flow does not fit the
/// closure shape of [`with_immediate_transaction`]. IMMEDIATE mode acquires the
/// write lock up front, avoiding `SQLITE_BUSY` lock-upgrade failures and lost
/// updates that DEFERRED transactions risk under write concurrency.
pub(crate) fn begin_immediate(
    conn: &mut rusqlite::Connection,
) -> rusqlite::Result<rusqlite::Transaction<'_>> {
    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
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
    let start = std::time::Instant::now();
    let result = with_retry(
        || {
            let mut conn = pool.get().map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    Some(e.to_string()),
                )
            })?;

            // Defer FK checks until commit for better batch performance
            conn.execute_batch("PRAGMA defer_foreign_keys = ON")?;

            // Use IMMEDIATE transaction to acquire write lock immediately
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        },
        MAX_RETRIES,
    )
    .map_err(map_db_error);

    let elapsed = start.elapsed();
    if elapsed.as_millis() > SLOW_QUERY_THRESHOLD_MS {
        tracing::warn!(
            duration_ms = elapsed.as_millis() as u64,
            "Slow transaction detected (>{SLOW_QUERY_THRESHOLD_MS}ms)"
        );
    }

    result
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
                let conn = self.pool.get().map_err(|e| {
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                        Some(e.to_string()),
                    )
                })?;
                let mut conn =
                    PragmaScope::new(conn, timeout_ms, set_read_uncommitted, fallback_timeout_ms)?;

                {
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
                }
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

    #[test]
    fn append_limit_offset_covers_all_cases() {
        let case = |limit: Option<u32>, offset: Option<u32>| {
            let mut sql = "SELECT * FROM t".to_string();
            append_limit_offset(&mut sql, limit, offset);
            sql
        };
        assert_eq!(case(Some(10), Some(5)), "SELECT * FROM t LIMIT 10 OFFSET 5");
        assert_eq!(case(Some(10), None), "SELECT * FROM t LIMIT 10");
        // No limit → server-side default applies (never an unbounded scan).
        assert_eq!(case(None, None), "SELECT * FROM t LIMIT 500");
        // A bare `OFFSET` is invalid on SQLite, so a LIMIT (the default) must
        // always precede it.
        assert_eq!(case(None, Some(5)), "SELECT * FROM t LIMIT 500 OFFSET 5");
        // Requested limits are capped at the server-side maximum.
        assert_eq!(case(Some(5000), None), "SELECT * FROM t LIMIT 1000");
    }

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
    fn with_transaction_restores_pragmas_after_panic() {
        let db = SqliteDatabase::new(&DatabaseConfig {
            url: ":memory:".to_string(),
            max_connections: 1,
        })
        .expect("db should initialize");

        {
            let conn = db.conn().expect("connection should open");
            conn.execute_batch("PRAGMA busy_timeout = 25; PRAGMA read_uncommitted = false;")
                .expect("initial pragmas should be set");
        }

        let panic_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = db.with_transaction_opts(
                crate::TransactionOptions::new()
                    .timeout_ms(250)
                    .isolation(crate::TransactionIsolation::ReadUncommitted),
                |_conn| -> std::result::Result<(), rusqlite::Error> {
                    panic!("boom");
                },
            );
        }));
        assert!(panic_result.is_err());

        let conn = db.conn().expect("connection should reopen");
        let busy_timeout: u64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, u64>(0))
            .expect("busy_timeout pragma should be readable");
        let read_uncommitted: i64 = conn
            .query_row("PRAGMA read_uncommitted", [], |row| row.get::<_, i64>(0))
            .expect("read_uncommitted pragma should be readable");
        assert_eq!(busy_timeout, 25);
        assert_eq!(read_uncommitted, 0);
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

    #[test]
    fn retry_jitter_is_decorrelated_across_threads() {
        // Concurrent writers hitting SQLITE_BUSY must not back off in lockstep:
        // each thread's jitter sequence has to differ, or contenders re-collide
        // every round and the jitter is decorative.
        let sequences: Vec<Vec<u64>> = (0..4)
            .map(|_| {
                std::thread::spawn(|| (1..=8).map(retry_jitter).collect::<Vec<u64>>())
                    .join()
                    .expect("jitter thread panicked")
            })
            .collect();

        let distinct: std::collections::HashSet<&Vec<u64>> = sequences.iter().collect();
        assert!(
            distinct.len() > 1,
            "all {} threads drew the identical jitter sequence {:?}",
            sequences.len(),
            sequences[0]
        );
    }
}
