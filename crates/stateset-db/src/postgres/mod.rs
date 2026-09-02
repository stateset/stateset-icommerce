//! PostgreSQL database implementation using sqlx
//!
//! This module provides async PostgreSQL support for production deployments.

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
mod orders;
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
pub use vendor_credits::*;
pub use vendor_returns::*;
pub use warehouse::*;
pub use warranties::*;
pub use wishlists::*;
pub use work_orders::*;
pub use x402_credits::*;
pub use x402_payment_intents::*;
pub use zone_shipping_methods::*;

use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgSslMode};
use stateset_core::CommerceError;

/// Default page size applied when a list filter does not specify a limit.
pub(crate) const DEFAULT_LIST_LIMIT: u32 = 500;

/// Hard server-side ceiling on requested page sizes.
pub(crate) const MAX_LIST_LIMIT: u32 = 1000;

/// Clamp a requested page size to the server-side pagination policy:
/// `None` becomes [`DEFAULT_LIST_LIMIT`], and anything above
/// [`MAX_LIST_LIMIT`] is capped to it. Returned as `i64` for sqlx binding.
pub(crate) const fn effective_limit(limit: Option<u32>) -> i64 {
    let limit = match limit {
        Some(limit) if limit > MAX_LIST_LIMIT => MAX_LIST_LIMIT,
        Some(limit) => limit,
        None => DEFAULT_LIST_LIMIT,
    };
    limit as i64
}

/// Parse a keyset `(sort_key, id)` cursor into typed Postgres bind values.
///
/// The sort key must be an RFC 3339 timestamp and the id a UUID, matching how
/// cursors are encoded at the HTTP layer.
pub(crate) fn parse_after_cursor(
    after_cursor: Option<&(String, String)>,
) -> Result<Option<(chrono::DateTime<chrono::Utc>, uuid::Uuid)>, CommerceError> {
    match after_cursor {
        Some((sort_key, id)) => {
            let ts = chrono::DateTime::parse_from_rfc3339(sort_key)
                .map_err(|e| {
                    CommerceError::ValidationError(format!("invalid cursor timestamp: {e}"))
                })?
                .with_timezone(&chrono::Utc);
            let id = uuid::Uuid::parse_str(id)
                .map_err(|e| CommerceError::ValidationError(format!("invalid cursor id: {e}")))?;
            Ok(Some((ts, id)))
        }
        None => Ok(None),
    }
}

use std::future::Future;
use std::str::FromStr;
use std::time::Duration;

/// PostgreSQL database connection pool
#[derive(Debug, Clone)]
pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
    /// Connect to PostgreSQL database with URL
    pub async fn connect(url: impl Into<String>) -> Result<Self, CommerceError> {
        Self::connect_with_options(url, 10, 30).await
    }

    /// Connect with custom options
    pub async fn connect_with_options(
        url: impl Into<String>,
        max_connections: u32,
        acquire_timeout_secs: u64,
    ) -> Result<Self, CommerceError> {
        let url = url.into();
        let connect_options = parse_secure_connect_options(&url)?;
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
            .connect_with(connect_options)
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Run migrations
        Self::run_migrations(&pool).await?;

        Ok(Self { pool })
    }

    /// Run database migrations, serialized across concurrent callers.
    ///
    /// Acquires a session-level Postgres advisory lock for the duration of the
    /// run. Without it, two runners (multiple app instances booting against a
    /// fresh database, or parallel integration tests) can each observe a
    /// migration as unapplied and both execute its `CREATE TYPE`, which fails
    /// with a duplicate-key error on `pg_type`. The second runner now blocks on
    /// the lock, then finds every migration already applied and does nothing.
    async fn run_migrations(pool: &PgPool) -> Result<(), CommerceError> {
        // Arbitrary fixed key identifying the migration lock for this app.
        const MIGRATION_LOCK_KEY: i64 = 0x5354_4154_4553_4554;

        let mut lock_conn =
            pool.acquire().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(MIGRATION_LOCK_KEY)
            .execute(&mut *lock_conn)
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = Self::apply_migrations(pool).await;

        // Release before the connection returns to the pool — a session-level
        // advisory lock outlives the borrow on a pooled (not closed) connection.
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MIGRATION_LOCK_KEY)
            .execute(&mut *lock_conn)
            .await;

        result
    }

    /// The number of embedded migrations for the active feature set.
    ///
    /// Exposed so integration tests can assert "all embedded migrations
    /// applied" without hardcoding a count that rots every time a migration
    /// lands (a stale hardcoded count kept the Postgres parity CI lane red).
    #[must_use]
    pub fn embedded_migration_count() -> usize {
        Self::embedded_migrations().len()
    }

    /// The ordered list of embedded migrations for the active feature set.
    fn embedded_migrations() -> Vec<(&'static str, &'static str)> {
        // Get list of migrations
        let mut migrations = vec![
            ("001_initial_schema", include_str!("migrations/001_initial_schema.sql")),
            ("001a_pgcrypto", include_str!("migrations/001a_pgcrypto.sql")),
            ("002_inventory", include_str!("migrations/002_inventory.sql")),
            ("003_returns", include_str!("migrations/003_returns.sql")),
            ("004_manufacturing", include_str!("migrations/004_manufacturing.sql")),
            ("005_currency", include_str!("migrations/005_currency.sql")),
            ("006_shipments", include_str!("migrations/006_shipments.sql")),
            ("007_payments", include_str!("migrations/007_payments.sql")),
            ("008_warranties", include_str!("migrations/008_warranties.sql")),
            ("009_purchase_orders", include_str!("migrations/009_purchase_orders.sql")),
            ("010_invoices", include_str!("migrations/010_invoices.sql")),
            ("011_carts", include_str!("migrations/011_carts.sql")),
            ("012_versioning", include_str!("migrations/012_versioning.sql")),
            ("013_versioning_catalog", include_str!("migrations/013_versioning_catalog.sql")),
            ("014_tax", include_str!("migrations/014_tax.sql")),
            ("015_promotions", include_str!("migrations/015_promotions.sql")),
            ("016_subscriptions", include_str!("migrations/016_subscriptions.sql")),
            ("017_quality", include_str!("migrations/017_quality.sql")),
            ("018_lots", include_str!("migrations/018_lots.sql")),
            ("019_serials", include_str!("migrations/019_serials.sql")),
            ("020_warehouse", include_str!("migrations/020_warehouse.sql")),
            ("021_receiving", include_str!("migrations/021_receiving.sql")),
            ("022_fulfillment", include_str!("migrations/022_fulfillment.sql")),
            ("023_accounts_payable", include_str!("migrations/023_accounts_payable.sql")),
            ("024_cost_accounting", include_str!("migrations/024_cost_accounting.sql")),
            ("025_credit", include_str!("migrations/025_credit.sql")),
            ("026_backorder", include_str!("migrations/026_backorder.sql")),
            ("027_accounts_receivable", include_str!("migrations/027_accounts_receivable.sql")),
            ("028_general_ledger", include_str!("migrations/028_general_ledger.sql")),
            ("029_performance_indexes", include_str!("migrations/029_performance_indexes.sql")),
            ("030_idempotency_keys", include_str!("migrations/030_idempotency_keys.sql")),
            ("031_x402_credits", include_str!("migrations/031_x402_credits.sql")),
            ("032_erc8004", include_str!("migrations/032_erc8004.sql")),
            ("033_x402_a2a", include_str!("migrations/033_x402_a2a.sql")),
            ("034_custom_objects", include_str!("migrations/034_custom_objects.sql")),
        ];

        // Optional, experimental migrations.
        #[cfg(feature = "saga")]
        {
            migrations.push(("035_sagas", include_str!("migrations/035_sagas.sql")));
        }

        migrations.push(("036_orders_cart_id", include_str!("migrations/036_orders_cart_id.sql")));
        migrations.push((
            "037_x402_nonce_integrity",
            include_str!("migrations/037_x402_nonce_integrity.sql"),
        ));
        migrations.push(("038_gift_cards", include_str!("migrations/038_gift_cards.sql")));
        migrations.push(("040_store_credits", include_str!("migrations/040_store_credits.sql")));
        migrations.push(("041_reviews", include_str!("migrations/041_reviews.sql")));
        migrations.push(("042_wishlists", include_str!("migrations/042_wishlists.sql")));
        migrations.push(("043_segments", include_str!("migrations/043_segments.sql")));
        migrations.push(("044_shipping_zones", include_str!("migrations/044_shipping_zones.sql")));
        migrations.push(("045_rewards", include_str!("migrations/045_rewards.sql")));
        migrations.push(("046_search_configs", include_str!("migrations/046_search_configs.sql")));
        migrations.push((
            "047_zone_shipping_methods",
            include_str!("migrations/047_zone_shipping_methods.sql"),
        ));
        migrations.push(("048_fraud", include_str!("migrations/048_fraud.sql")));
        migrations.push(("049_loyalty", include_str!("migrations/049_loyalty.sql")));
        migrations.push(("050_x402_pqc", include_str!("migrations/050_x402_pqc.sql")));
        migrations.push(("051_loyalty_tiers", include_str!("migrations/051_loyalty_tiers.sql")));
        migrations.push((
            "052_wishlist_item_quantity",
            include_str!("migrations/052_wishlist_item_quantity.sql"),
        ));
        migrations.push((
            "053_remove_seeded_exchange_rates",
            include_str!("migrations/053_remove_seeded_exchange_rates.sql"),
        ));
        migrations.push(("054_wms_entities", include_str!("migrations/054_wms_entities.sql")));
        migrations.push((
            "054_supply_chain_entities",
            include_str!("migrations/054_supply_chain_entities.sql"),
        ));
        migrations.push((
            "055_fixed_assets_revrec",
            include_str!("migrations/055_fixed_assets_revrec.sql"),
        ));
        migrations.push((
            "056_gl_auto_posting_flags",
            include_str!("migrations/056_gl_auto_posting_flags.sql"),
        ));
        migrations.push(("057_cycle_counts", include_str!("migrations/057_cycle_counts.sql")));
        migrations
            .push(("058_gl_fx_revaluation", include_str!("migrations/058_gl_fx_revaluation.sql")));
        migrations
            .push(("059_http_idempotency", include_str!("migrations/059_http_idempotency.sql")));
        migrations.push(("060_channels", include_str!("migrations/060_channels.sql")));
        migrations.push(("061_companies", include_str!("migrations/061_companies.sql")));
        migrations.push(("062_activity_logs", include_str!("migrations/062_activity_logs.sql")));
        migrations.push((
            "063_payment_obligations",
            include_str!("migrations/063_payment_obligations.sql"),
        ));
        migrations.push(("064_prepayments", include_str!("migrations/064_prepayments.sql")));
        migrations.push(("065_price_levels", include_str!("migrations/065_price_levels.sql")));
        migrations
            .push(("066_price_schedules", include_str!("migrations/066_price_schedules.sql")));
        migrations.push(("067_purgatory", include_str!("migrations/067_purgatory.sql")));
        migrations.push(("068_edi_documents", include_str!("migrations/068_edi_documents.sql")));
        migrations.push((
            "069_integration_mappings",
            include_str!("migrations/069_integration_mappings.sql"),
        ));
        migrations.push((
            "070_integration_field_mappings",
            include_str!("migrations/070_integration_field_mappings.sql"),
        ));
        migrations.push((
            "071_topology_snapshots",
            include_str!("migrations/071_topology_snapshots.sql"),
        ));
        migrations.push((
            "072_order_item_shipped_quantity",
            include_str!("migrations/072_order_item_shipped_quantity.sql"),
        ));
        // Warehouse bins + return item disposition.
        migrations.push((
            "073_warehouse_bins_return_disposition",
            include_str!("migrations/073_warehouse_bins_return_disposition.sql"),
        ));
        migrations.push(("074_kernel_outbox", include_str!("migrations/074_kernel_outbox.sql")));
        migrations.push((
            "075_kernel_outbox_delivery",
            include_str!("migrations/075_kernel_outbox_delivery.sql"),
        ));
        migrations.push((
            "076_kernel_receipt_audit_chain",
            include_str!("migrations/076_kernel_receipt_audit_chain.sql"),
        ));
        migrations
            .push(("077_a2a_escrow_kernel", include_str!("migrations/077_a2a_escrow_kernel.sql")));
        migrations.push((
            "078_a2a_dispute_kernel",
            include_str!("migrations/078_a2a_dispute_kernel.sql"),
        ));
        migrations.push((
            "079_product_exact_money",
            include_str!("migrations/079_product_exact_money.sql"),
        ));
        migrations.push((
            "080_inventory_exact_quantity",
            include_str!("migrations/080_inventory_exact_quantity.sql"),
        ));
        // Bookkeeping column so direct payments (record_payment) survive the
        // AR recalculation instead of being replaced by application sums.
        migrations.push((
            "081_invoice_direct_amount_paid",
            include_str!("migrations/081_invoice_direct_amount_paid.sql"),
        ));
        // Database-enforced auto-post idempotency: unique key per source
        // document for the single-entry journal families.
        migrations.push((
            "082_gl_source_document_key",
            include_str!("migrations/082_gl_source_document_key.sql"),
        ));
        // Idempotency ledger so a retried direct invoice payment (a caller
        // supplying `RecordInvoicePayment.payment_id`) applies exactly once.
        migrations.push((
            "083_invoice_payment_idempotency",
            include_str!("migrations/083_invoice_payment_idempotency.sql"),
        ));
        // Legacy-safe uniqueness for subscription billing cycles.
        migrations.push((
            "084_billing_cycle_uniqueness",
            include_str!("migrations/084_billing_cycle_uniqueness.sql"),
        ));
        // Order-level tax/shipping/discount so checkout can carry what the
        // customer is actually charged.
        migrations.push((
            "085_order_money_breakdown",
            include_str!("migrations/085_order_money_breakdown.sql"),
        ));
        // Legacy-safe "one open reservation per serial" backstop.
        migrations.push((
            "086_serial_reservation_uniqueness",
            include_str!("migrations/086_serial_reservation_uniqueness.sql"),
        ));
        // Nullable `order_item_id` on inventory_reservations so a removed order
        // line releases ITS reservation, not the oldest one for the same SKU.
        migrations.push((
            "087_reservation_order_line",
            include_str!("migrations/087_reservation_order_line.sql"),
        ));
        // Lot/serial traceability columns on return_items.
        migrations.push((
            "088_return_idempotency_and_traceability",
            include_str!("migrations/088_return_idempotency_and_traceability.sql"),
        ));
        // Legacy-safe uniqueness for x402 settlement tx hashes.
        migrations.push((
            "089_x402_tx_hash_uniqueness",
            include_str!("migrations/089_x402_tx_hash_uniqueness.sql"),
        ));
        // Non-negative CHECK on inventory balances (NOT VALID → legacy-safe)
        // and `reservation_id` on backorder allocations.
        migrations.push((
            "090_inventory_balance_guards",
            include_str!("migrations/090_inventory_balance_guards.sql"),
        ));
        // Nullable billing-worker lease columns on subscriptions so due
        // subscriptions are claimed atomically before they are billed.
        migrations.push((
            "091_billing_claim_lease",
            include_str!("migrations/091_billing_claim_lease.sql"),
        ));
        // Legacy-safe, case-insensitive e-mail uniqueness for live customers
        // (keyed column; deleted accounts release their address).
        migrations.push((
            "092_customer_email_key",
            include_str!("migrations/092_customer_email_key.sql"),
        ));
        // Durable, tenant-scoped A2A credit terms and agent messaging
        // (previously process-local state in the HTTP routes).
        migrations.push((
            "093_a2a_credit_terms_and_messaging",
            include_str!("migrations/093_a2a_credit_terms_and_messaging.sql"),
        ));

        migrations
    }

    /// Apply pending migrations. Callers must hold the migration advisory lock
    /// (see [`Self::run_migrations`]).
    async fn apply_migrations(pool: &PgPool) -> Result<(), CommerceError> {
        // Create migrations table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                checksum TEXT,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        for (name, sql) in Self::embedded_migrations() {
            let mut tx =
                pool.begin().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            let checksum = Self::compute_migration_checksum(sql);

            // Check if migration already applied (inside tx so apply+record is atomic per migration).
            let existing_checksum: Option<Option<String>> =
                sqlx::query_scalar("SELECT checksum FROM _migrations WHERE name = $1")
                    .bind(name)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            if let Some(existing) = existing_checksum {
                let existing = existing.unwrap_or_default();
                if !existing.is_empty() && existing != checksum {
                    return Err(CommerceError::DatabaseError(format!(
                        "migration checksum mismatch for {name}: expected {checksum}, found {existing}",
                    )));
                }
                if existing.is_empty() {
                    sqlx::query("UPDATE _migrations SET checksum = $1 WHERE name = $2")
                        .bind(&checksum)
                        .bind(name)
                        .execute(tx.as_mut())
                        .await
                        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                }
            } else {
                sqlx::raw_sql(sql).execute(tx.as_mut()).await.map_err(|e| {
                    CommerceError::DatabaseError(format!("Migration {} failed: {}", name, e))
                })?;

                sqlx::query("INSERT INTO _migrations (name, checksum) VALUES ($1, $2)")
                    .bind(name)
                    .bind(&checksum)
                    .execute(tx.as_mut())
                    .await
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            }

            tx.commit().await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    fn compute_migration_checksum(sql: &str) -> String {
        let digest = Sha256::digest(sql.as_bytes());
        digest.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Get order repository
    pub fn orders(&self) -> PgOrderRepository {
        PgOrderRepository::new(self.pool.clone())
    }

    /// Get inventory repository
    pub fn inventory(&self) -> PgInventoryRepository {
        PgInventoryRepository::new(self.pool.clone())
    }

    /// Get customer repository
    pub fn customers(&self) -> PgCustomerRepository {
        PgCustomerRepository::new(self.pool.clone())
    }

    /// Get product repository
    pub fn products(&self) -> PgProductRepository {
        PgProductRepository::new(self.pool.clone())
    }

    /// Get the durable HTTP idempotency repository
    #[must_use]
    pub fn http_idempotency(&self) -> PgHttpIdempotencyRepository {
        PgHttpIdempotencyRepository::new(self.pool.clone())
    }

    /// Get custom objects repository (custom states / metaobjects)
    pub fn custom_objects(&self) -> PgCustomObjectRepository {
        PgCustomObjectRepository::new(self.pool.clone())
    }

    /// Get production batch repository
    pub fn production_batches(&self) -> PgProductionBatchRepository {
        PgProductionBatchRepository::new(self.pool.clone())
    }

    /// Get supplier SKU repository
    pub fn supplier_skus(&self) -> PgSupplierSkuRepository {
        PgSupplierSkuRepository::new(self.pool.clone())
    }

    /// Get fixed asset repository
    pub fn fixed_assets(&self) -> PgFixedAssetRepository {
        PgFixedAssetRepository::new(self.pool.clone())
    }

    /// Get revenue recognition repository
    pub fn revenue_recognition(&self) -> PgRevenueRecognitionRepository {
        PgRevenueRecognitionRepository::new(self.pool.clone())
    }

    /// Get vendor return repository
    pub fn vendor_returns(&self) -> PgVendorReturnRepository {
        PgVendorReturnRepository::new(self.pool.clone())
    }

    /// Get vendor credit repository
    pub fn vendor_credits(&self) -> PgVendorCreditRepository {
        PgVendorCreditRepository::new(self.pool.clone())
    }

    /// Get return repository
    pub fn returns(&self) -> PgReturnRepository {
        PgReturnRepository::new(self.pool.clone())
    }

    /// Get BOM repository
    pub fn bom(&self) -> PgBomRepository {
        PgBomRepository::new(self.pool.clone())
    }

    /// Get work order repository
    pub fn work_orders(&self) -> PgWorkOrderRepository {
        PgWorkOrderRepository::new(self.pool.clone())
    }

    /// Get currency repository
    pub fn currency(&self) -> PgCurrencyRepository {
        PgCurrencyRepository::new(self.pool.clone())
    }

    /// Get shipment repository
    pub fn shipments(&self) -> PgShipmentRepository {
        PgShipmentRepository::new(self.pool.clone())
    }

    /// Get payment repository
    pub fn payments(&self) -> PgPaymentRepository {
        PgPaymentRepository::new(self.pool.clone())
    }

    /// Get the durable kernel outbox consumer API.
    pub fn kernel_outbox(&self) -> PgKernelOutboxRepository {
        PgKernelOutboxRepository::new(self.pool.clone())
    }

    /// Create an envelope-aware executor governed by the supplied policy revision.
    pub fn kernel_executor(&self, policy: stateset_core::KernelPolicy) -> PgKernelExecutor {
        PgKernelExecutor::new(self.pool.clone(), policy)
    }

    /// Get warranty repository
    pub fn warranties(&self) -> PgWarrantyRepository {
        PgWarrantyRepository::new(self.pool.clone())
    }

    /// Get purchase order repository
    pub fn purchase_orders(&self) -> PgPurchaseOrderRepository {
        PgPurchaseOrderRepository::new(self.pool.clone())
    }

    /// Get invoice repository
    pub fn invoices(&self) -> PgInvoiceRepository {
        PgInvoiceRepository::new(self.pool.clone())
    }

    /// Get cart repository
    pub fn carts(&self) -> PgCartRepository {
        PgCartRepository::new(self.pool.clone())
    }

    /// Get analytics repository
    pub fn analytics(&self) -> PgAnalyticsRepository {
        PgAnalyticsRepository::new(self.pool.clone())
    }

    /// Get tax repository
    pub fn tax(&self) -> PgTaxRepository {
        PgTaxRepository::new(self.pool.clone())
    }

    /// Get promotions repository
    pub fn promotions(&self) -> PgPromotionRepository {
        PgPromotionRepository::new(self.pool.clone())
    }

    /// Get subscriptions repository
    pub fn subscriptions(&self) -> PgSubscriptionRepository {
        PgSubscriptionRepository::new(self.pool.clone())
    }

    /// Get quality repository
    pub fn quality(&self) -> PgQualityRepository {
        PgQualityRepository::new(self.pool.clone())
    }

    /// Get lots repository
    pub fn lots(&self) -> PgLotRepository {
        PgLotRepository::new(self.pool.clone())
    }

    /// Get serials repository
    pub fn serials(&self) -> PgSerialRepository {
        PgSerialRepository::new(self.pool.clone())
    }

    /// Get warehouse bin repository
    pub fn bins(&self) -> PgBinRepository {
        PgBinRepository::new(self.pool.clone())
    }

    /// Get warehouse repository
    pub fn warehouse(&self) -> PgWarehouseRepository {
        PgWarehouseRepository::new(self.pool.clone())
    }

    /// Get receiving repository
    pub fn receiving(&self) -> PgReceivingRepository {
        PgReceivingRepository::new(self.pool.clone())
    }

    /// Get fulfillment repository
    pub fn fulfillment(&self) -> PgFulfillmentRepository {
        PgFulfillmentRepository::new(self.pool.clone())
    }

    /// Get accounts payable repository
    pub fn accounts_payable(&self) -> PgAccountsPayableRepository {
        PgAccountsPayableRepository::new(self.pool.clone())
    }

    /// Get cost accounting repository
    pub fn cost_accounting(&self) -> PgCostAccountingRepository {
        PgCostAccountingRepository::new(self.pool.clone())
    }

    /// Get credit repository
    pub fn credit(&self) -> PgCreditRepository {
        PgCreditRepository::new(self.pool.clone())
    }

    /// Get backorder repository
    pub fn backorder(&self) -> PgBackorderRepository {
        PgBackorderRepository::new(self.pool.clone())
    }

    /// Get accounts receivable repository
    pub fn accounts_receivable(&self) -> PgAccountsReceivableRepository {
        PgAccountsReceivableRepository::new(self.pool.clone())
    }

    /// Get general ledger repository
    pub fn general_ledger(&self) -> PgGeneralLedgerRepository {
        PgGeneralLedgerRepository::new(self.pool.clone())
    }

    /// Get x402 payment intent repository
    pub fn x402_payment_intents(&self) -> PgX402PaymentIntentRepository {
        PgX402PaymentIntentRepository::new(self.pool.clone())
    }

    /// Get x402 credit ledger repository
    pub fn x402_credits(&self) -> PgX402CreditRepository {
        PgX402CreditRepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    pub fn a2a_quotes(&self) -> PgA2ARepository {
        PgA2ARepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    pub fn a2a_purchases(&self) -> PgA2ARepository {
        PgA2ARepository::new(self.pool.clone())
    }

    /// Get durable A2A credit terms repository
    pub fn a2a_credit_terms(&self) -> PgA2ACreditTermsRepository {
        PgA2ACreditTermsRepository::new(self.pool.clone())
    }

    /// Get durable A2A agent messaging repository
    pub fn a2a_messages(&self) -> PgA2AMessagingRepository {
        PgA2AMessagingRepository::new(self.pool.clone())
    }

    /// Get agent card repository
    pub fn agent_cards(&self) -> PgAgentCardRepository {
        PgAgentCardRepository::new(self.pool.clone())
    }

    /// Get agent identity repository (ERC-8004)
    pub fn agent_identities(&self) -> PgAgentIdentityRepository {
        PgAgentIdentityRepository::new(self.pool.clone())
    }

    /// Get agent reputation repository (ERC-8004)
    pub fn agent_reputation(&self) -> PgAgentReputationRepository {
        PgAgentReputationRepository::new(self.pool.clone())
    }

    /// Get agent validation repository (ERC-8004)
    pub fn agent_validation(&self) -> PgAgentValidationRepository {
        PgAgentValidationRepository::new(self.pool.clone())
    }

    /// Get segment repository
    pub fn segments(&self) -> PgSegmentRepository {
        PgSegmentRepository::new(self.pool.clone())
    }

    /// Get shipping zone repository
    pub fn shipping_zones(&self) -> PgShippingZoneRepository {
        PgShippingZoneRepository::new(self.pool.clone())
    }

    /// Get fraud repository
    pub fn fraud(&self) -> PgFraudRepository {
        PgFraudRepository::new(self.pool.clone())
    }

    /// Get loyalty program repository
    pub fn loyalty(&self) -> PgLoyaltyProgramRepository {
        PgLoyaltyProgramRepository::new(self.pool.clone())
    }

    /// Get gift card repository
    pub fn gift_cards(&self) -> PgGiftCardRepository {
        PgGiftCardRepository::new(self.pool.clone())
    }

    /// Get store credit repository
    pub fn store_credits(&self) -> PgStoreCreditRepository {
        PgStoreCreditRepository::new(self.pool.clone())
    }

    /// Get reward repository
    pub fn rewards(&self) -> PgRewardRepository {
        PgRewardRepository::new(self.pool.clone())
    }

    /// Get search config repository
    pub fn search_configs(&self) -> PgSearchConfigRepository {
        PgSearchConfigRepository::new(self.pool.clone())
    }

    /// Get zone shipping method repository
    pub fn zone_shipping_methods(&self) -> PgZoneShippingMethodRepository {
        PgZoneShippingMethodRepository::new(self.pool.clone())
    }

    /// Get transfer order repository
    pub fn transfer_orders(&self) -> PgTransferOrderRepository {
        PgTransferOrderRepository::new(self.pool.clone())
    }

    /// Get unit of measure repository
    pub fn units_of_measure(&self) -> PgUnitOfMeasureRepository {
        PgUnitOfMeasureRepository::new(self.pool.clone())
    }

    /// Get inbound shipment repository
    pub fn inbound_shipments(&self) -> PgInboundShipmentRepository {
        PgInboundShipmentRepository::new(self.pool.clone())
    }

    /// Get print station repository
    pub fn print_stations(&self) -> PgPrintStationRepository {
        PgPrintStationRepository::new(self.pool.clone())
    }

    /// Get stock snapshot repository
    pub fn stock_snapshots(&self) -> PgStockSnapshotRepository {
        PgStockSnapshotRepository::new(self.pool.clone())
    }

    /// Get channel repository
    pub fn channels(&self) -> PgChannelRepository {
        PgChannelRepository::new(self.pool.clone())
    }

    /// Get company repository
    pub fn companies(&self) -> PgCompanyRepository {
        PgCompanyRepository::new(self.pool.clone())
    }

    /// Get payment obligation repository
    pub fn payment_obligations(&self) -> PgPaymentObligationRepository {
        PgPaymentObligationRepository::new(self.pool.clone())
    }

    /// Get price level repository
    pub fn price_levels(&self) -> PgPriceLevelRepository {
        PgPriceLevelRepository::new(self.pool.clone())
    }

    /// Get prepayment repository
    pub fn prepayments(&self) -> PgPrepaymentRepository {
        PgPrepaymentRepository::new(self.pool.clone())
    }

    /// Get price schedule repository
    pub fn price_schedules(&self) -> PgPriceScheduleRepository {
        PgPriceScheduleRepository::new(self.pool.clone())
    }

    /// Get activity log repository
    pub fn activity_logs(&self) -> PgActivityLogRepository {
        PgActivityLogRepository::new(self.pool.clone())
    }

    /// Get integration mapping repository
    pub fn integration_mappings(&self) -> PgIntegrationMappingRepository {
        PgIntegrationMappingRepository::new(self.pool.clone())
    }

    /// Get integration field mapping repository
    pub fn integration_field_mappings(&self) -> PgIntegrationFieldMappingRepository {
        PgIntegrationFieldMappingRepository::new(self.pool.clone())
    }

    /// Get purgatory repository
    pub fn purgatory(&self) -> PgPurgatoryRepository {
        PgPurgatoryRepository::new(self.pool.clone())
    }

    /// Get EDI document repository
    pub fn edi_documents(&self) -> PgEdiDocumentRepository {
        PgEdiDocumentRepository::new(self.pool.clone())
    }

    /// Get topology snapshot repository
    pub fn topology_snapshots(&self) -> PgTopologySnapshotRepository {
        PgTopologySnapshotRepository::new(self.pool.clone())
    }

    /// Get underlying pool (for advanced use)
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }
}

fn parse_secure_connect_options(url: &str) -> Result<PgConnectOptions, CommerceError> {
    let options = PgConnectOptions::from_str(url)
        .map_err(|e| CommerceError::DatabaseError(format!("invalid postgres URL: {e}")))?;

    match options.get_ssl_mode() {
        PgSslMode::Disable | PgSslMode::Allow | PgSslMode::Prefer
            if is_local_postgres_host(options.get_host()) =>
        {
            Ok(options)
        }
        PgSslMode::Disable | PgSslMode::Allow | PgSslMode::Prefer => {
            Err(CommerceError::DatabaseError(
                "postgres sslmode must be require, verify-ca, or verify-full for non-local hosts"
                    .to_string(),
            ))
        }
        PgSslMode::Require | PgSslMode::VerifyCa | PgSslMode::VerifyFull => Ok(options),
    }
}

fn is_local_postgres_host(host: &str) -> bool {
    let normalized = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    normalized == "localhost"
        || normalized == "127.0.0.1"
        || normalized == "::1"
        || normalized.starts_with('/')
}

/// Helper function to convert sqlx errors to `CommerceError`
pub(crate) fn map_db_error(e: sqlx::Error) -> CommerceError {
    match e {
        sqlx::Error::RowNotFound => CommerceError::NotFound,
        sqlx::Error::Database(db_err) => {
            let message = db_err.message().to_string();
            if let Some(code) = db_err.code() {
                match code.as_ref() {
                    // 23505: unique_violation
                    "23505" => {
                        let constraint = db_err.constraint().unwrap_or("unknown");
                        if let Some(conflict) = map_unique_constraint_error(constraint, &message) {
                            return conflict;
                        }

                        CommerceError::Conflict(format!("{constraint}: {message}"))
                    }
                    // 23514: check_violation
                    "23514" => CommerceError::ValidationError(message),
                    // 23502: not_null_violation
                    "23502" => CommerceError::ValidationError(message),
                    _ => CommerceError::DatabaseError(message),
                }
            } else {
                CommerceError::DatabaseError(message)
            }
        }
        _ => CommerceError::DatabaseError(e.to_string()),
    }
}

fn map_unique_constraint_error(constraint: &str, message: &str) -> Option<CommerceError> {
    let lower = constraint.to_ascii_lowercase();
    if lower.contains("email") {
        return Some(CommerceError::EmailAlreadyExists(duplicate_key_value(message)));
    }

    if lower.contains("slug") {
        return Some(CommerceError::DuplicateSlug(duplicate_key_value(message)));
    }

    if lower.contains("sku") {
        return Some(CommerceError::DuplicateSku(duplicate_key_value(message)));
    }

    None
}

fn duplicate_key_value(message: &str) -> String {
    if let Some(start) = message.find(")=(") {
        let value_and_rest = &message[start + 3..];
        if let Some(end) = value_and_rest.find(')') {
            return value_and_rest[..end].trim_matches(|c| c == '\'' || c == '"').to_string();
        }
    }

    message.to_string()
}

#[cfg(test)]
mod unique_constraint_tests {
    use super::{duplicate_key_value, map_unique_constraint_error};
    use stateset_core::CommerceError;

    #[test]
    fn test_duplicate_key_value_extracts_first_value() {
        let message = "duplicate key value violates unique constraint \"products_sku_key\" Detail: Key (sku)=(DUP-001) already exists.";
        assert_eq!(duplicate_key_value(message), "DUP-001");
    }

    #[test]
    fn test_duplicate_key_value_falls_back_to_message() {
        let message = "duplicate key value violates unique constraint \"products_sku_key\"";
        assert_eq!(duplicate_key_value(message), message);
    }

    #[test]
    fn test_map_unique_constraint_error_recognizes_email() {
        let error = map_unique_constraint_error(
            "users_email_key",
            "duplicate key value violates unique constraint \"users_email_key\"",
        )
        .expect("expected email error");
        assert!(matches!(error, CommerceError::EmailAlreadyExists(_)));
    }

    #[test]
    fn test_map_unique_constraint_error_recognizes_slug() {
        let error = map_unique_constraint_error(
            "products_slug_key",
            "duplicate key value violates unique constraint \"products_slug_key\"",
        )
        .expect("expected slug error");
        assert!(matches!(error, CommerceError::DuplicateSlug(_)));
    }

    #[test]
    fn test_map_unique_constraint_error_is_case_insensitive() {
        let error = map_unique_constraint_error(
            "Products_Sku_Key",
            "duplicate key value violates unique constraint \"Products_Sku_Key\"",
        )
        .expect("expected sku error");
        assert!(matches!(error, CommerceError::DuplicateSku(_)));
    }

    #[test]
    fn test_map_unique_constraint_error_recognizes_sku() {
        let error = map_unique_constraint_error(
            "products_variants_sku_key",
            "duplicate key value violates unique constraint \"products_variants_sku_key\"",
        )
        .expect("expected sku error");
        assert!(matches!(error, CommerceError::DuplicateSku(_)));
    }
}

#[cfg(feature = "postgres")]
const PG_INITIAL_BACKOFF_MS: u64 = 1;
#[cfg(feature = "postgres")]
const PG_MAX_BACKOFF_MS: u64 = 200;

#[cfg(feature = "postgres")]
const fn pg_transaction_isolation_sql(isolation: crate::TransactionIsolation) -> &'static str {
    match isolation {
        crate::TransactionIsolation::ReadUncommitted => "READ UNCOMMITTED",
        crate::TransactionIsolation::ReadCommitted => "READ COMMITTED",
        crate::TransactionIsolation::RepeatableRead => "REPEATABLE READ",
        crate::TransactionIsolation::Serializable => "SERIALIZABLE",
    }
}

#[cfg(feature = "postgres")]
fn is_retryable_postgres_error(error: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_error) = error {
        let code = db_error.code().unwrap_or_default();
        matches!(code.as_ref(), "40001" | "40P01" | "55P03")
    } else {
        false
    }
}

#[cfg(feature = "postgres")]
fn normalize_statement_timeout_ms(timeout_ms: Option<u64>) -> i32 {
    timeout_ms.unwrap_or(crate::DEFAULT_TRANSACTION_TIMEOUT_MS).min(i32::MAX as u64) as i32
}

#[cfg(feature = "postgres")]
async fn maybe_retry_with_backoff(
    retries: &mut u32,
    backoff_ms: &mut u64,
    error: &sqlx::Error,
) -> bool {
    if !is_retryable_postgres_error(error) || *retries == 0 {
        return false;
    }

    *retries -= 1;
    let delay_ms = (*backoff_ms).min(PG_MAX_BACKOFF_MS);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    *backoff_ms = (*backoff_ms * 2).min(PG_MAX_BACKOFF_MS);
    true
}

#[cfg(feature = "postgres")]
impl crate::AsyncDatabaseExt for PostgresDatabase {
    async fn with_transaction_async_opts<'a, F, T, Fut>(
        &'a self,
        opts: crate::TransactionOptions,
        mut f: F,
    ) -> stateset_core::Result<T>
    where
        F: FnMut(&mut sqlx::Transaction<'a, sqlx::Postgres>) -> Fut + Send,
        Fut: std::future::Future<Output = std::result::Result<T, sqlx::Error>> + Send,
        T: Send,
    {
        let mut retries = if opts.retry_on_conflict { opts.max_retries } else { 0 };
        let mut backoff_ms = PG_INITIAL_BACKOFF_MS;
        let statement_timeout = normalize_statement_timeout_ms(opts.timeout_ms);
        let isolation_sql = pg_transaction_isolation_sql(opts.isolation);

        loop {
            let mut tx = match self.pool.begin().await {
                Ok(tx) => tx,
                Err(error) => {
                    if maybe_retry_with_backoff(&mut retries, &mut backoff_ms, &error).await {
                        continue;
                    }
                    return Err(map_db_error(error));
                }
            };

            if let Err(error) =
                sqlx::query(&format!("SET TRANSACTION ISOLATION LEVEL {}", isolation_sql))
                    .execute(tx.as_mut())
                    .await
            {
                if maybe_retry_with_backoff(&mut retries, &mut backoff_ms, &error).await {
                    continue;
                }
                return Err(map_db_error(error));
            }

            if let Err(error) = sqlx::query("SET LOCAL statement_timeout = $1")
                .bind(statement_timeout)
                .execute(tx.as_mut())
                .await
            {
                if maybe_retry_with_backoff(&mut retries, &mut backoff_ms, &error).await {
                    continue;
                }
                return Err(map_db_error(error));
            }

            match f(&mut tx).await {
                Ok(output) => {
                    if let Err(error) = tx.commit().await {
                        if maybe_retry_with_backoff(&mut retries, &mut backoff_ms, &error).await {
                            continue;
                        }
                        return Err(map_db_error(error));
                    }
                    return Ok(output);
                }
                Err(error) => {
                    if maybe_retry_with_backoff(&mut retries, &mut backoff_ms, &error).await {
                        continue;
                    }
                    return Err(map_db_error(error));
                }
            }
        }
    }
}

/// Shared multi-threaded Tokio runtime used to drive blocking Postgres calls.
///
/// Constructing a [`tokio::runtime::Runtime`] is expensive (it spawns a worker
/// thread pool, an I/O reactor and a timer driver), so building one per
/// repository call — as the original implementation did — was both slow and
/// wasteful. Instead we lazily build a single multi-threaded runtime on first
/// use and reuse it for every subsequent blocking call.
///
/// A *multi-threaded* runtime is required: [`block_on`] parks the calling
/// thread while the future runs, and `sqlx` drives its connection pool on the
/// runtime's worker threads. A current-thread runtime would deadlock under
/// concurrent blocking callers.
static SHARED_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

/// Return a reference to the shared runtime, building it on first use.
///
/// Runtime construction can fail (e.g. the OS refuses to spawn threads). In that
/// case nothing is cached and the error is propagated so a later call may retry.
fn shared_runtime() -> stateset_core::Result<&'static tokio::runtime::Runtime> {
    if let Some(rt) = SHARED_RUNTIME.get() {
        return Ok(rt);
    }
    // Build outside `get_or_init` so a construction failure is not swallowed by
    // the `OnceLock` (which can only ever store a successful value).
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {e}")))?;
    Ok(SHARED_RUNTIME.get_or_init(|| rt))
}

/// Drive an async Postgres future to completion on a dedicated blocking runtime.
///
/// This bridges the synchronous [`stateset_core::traits`] repository API to the
/// async `sqlx` backend. Calling it from *within* an existing async runtime is
/// rejected (rather than silently nesting runtimes, which would panic or
/// deadlock); async callers must use the async repository methods directly.
pub(crate) fn block_on<F, T>(fut: F) -> stateset_core::Result<T>
where
    F: Future<Output = stateset_core::Result<T>>,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(CommerceError::NotPermitted(
            "Blocking Postgres call inside an async runtime; use AsyncCommerce instead".into(),
        ));
    }

    shared_runtime()?.block_on(fut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::error::{DatabaseError, ErrorKind};
    use std::borrow::Cow;
    use std::fmt::{self, Display, Formatter};

    #[derive(Debug)]
    struct MockDbError {
        code: Option<String>,
        message: String,
    }

    impl Display for MockDbError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl std::error::Error for MockDbError {}

    impl DatabaseError for MockDbError {
        fn message(&self) -> &str {
            &self.message
        }

        fn code(&self) -> Option<Cow<'_, str>> {
            self.code.as_ref().map(|code| Cow::Owned(code.clone()))
        }

        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }

        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    fn mock_db_error(code: Option<&str>, message: &str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(MockDbError {
            code: code.map(str::to_string),
            message: message.to_string(),
        }))
    }

    #[test]
    fn pg_transaction_isolation_sql_is_stable() {
        assert_eq!(
            pg_transaction_isolation_sql(crate::TransactionIsolation::ReadUncommitted),
            "READ UNCOMMITTED"
        );
        assert_eq!(
            pg_transaction_isolation_sql(crate::TransactionIsolation::ReadCommitted),
            "READ COMMITTED"
        );
        assert_eq!(
            pg_transaction_isolation_sql(crate::TransactionIsolation::RepeatableRead),
            "REPEATABLE READ"
        );
        assert_eq!(
            pg_transaction_isolation_sql(crate::TransactionIsolation::Serializable),
            "SERIALIZABLE"
        );
    }

    #[test]
    fn parse_secure_connect_options_allows_local_ci_postgres() {
        assert!(
            parse_secure_connect_options(
                "postgres://postgres:postgres@localhost:5432/stateset_test"
            )
            .is_ok()
        );
        assert!(
            parse_secure_connect_options(
                "postgres://postgres:postgres@127.0.0.1:5432/stateset_test"
            )
            .is_ok()
        );
    }

    #[test]
    fn parse_secure_connect_options_rejects_insecure_remote_postgres() {
        let err = parse_secure_connect_options(
            "postgres://postgres:postgres@db.example.com:5432/stateset_test",
        )
        .expect_err("remote postgres URLs must opt into TLS");

        assert!(format!("{err}").contains("sslmode must be require"));
    }

    #[test]
    fn is_retryable_postgres_error_matches_transient_codes() {
        assert!(is_retryable_postgres_error(&mock_db_error(
            Some("40001"),
            "serialization_failure"
        )));
        assert!(is_retryable_postgres_error(&mock_db_error(Some("40P01"), "deadlock_detected")));
        assert!(is_retryable_postgres_error(&mock_db_error(Some("55P03"), "lock_not_available")));
        assert!(!is_retryable_postgres_error(&mock_db_error(Some("23505"), "unique_violation")));
        assert!(!is_retryable_postgres_error(&mock_db_error(None, "no_code")));
        assert!(!is_retryable_postgres_error(&sqlx::Error::RowNotFound));
    }

    #[test]
    fn normalize_statement_timeout_ms_uses_default_and_caps_max_i32() {
        assert_eq!(
            normalize_statement_timeout_ms(None),
            crate::DEFAULT_TRANSACTION_TIMEOUT_MS as i32
        );
        assert_eq!(normalize_statement_timeout_ms(Some(1500)), 1500);
        assert_eq!(normalize_statement_timeout_ms(Some(i32::MAX as u64 + 1)), i32::MAX,);
    }

    #[tokio::test]
    async fn maybe_retry_with_backoff_respects_retry_budget_and_backoff_growth() {
        let mut retries = 1;
        let mut backoff = 4;
        let err = mock_db_error(Some("40001"), "serialization_failure");

        assert!(maybe_retry_with_backoff(&mut retries, &mut backoff, &err).await);
        assert_eq!(retries, 0);
        assert_eq!(backoff, 8);
    }

    #[tokio::test]
    async fn maybe_retry_with_backoff_skips_non_retryable_errors() {
        let mut retries = 3;
        let mut backoff = 4;
        let err = mock_db_error(Some("23505"), "unique_violation");

        assert!(!maybe_retry_with_backoff(&mut retries, &mut backoff, &err).await);
        assert_eq!(retries, 3);
        assert_eq!(backoff, 4);
    }
}
