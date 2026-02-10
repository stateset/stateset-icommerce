//! PostgreSQL database implementation using sqlx
//!
//! This module provides async PostgreSQL support for production deployments.

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
mod fulfillment;
mod general_ledger;
mod inventory;
mod invoices;
mod lots;
mod orders;
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
mod unsupported;
mod warehouse;
mod warranties;
mod work_orders;
mod x402_credits;
mod x402_payment_intents;

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
pub use fulfillment::*;
pub use general_ledger::*;
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
pub use unsupported::*;
pub use warehouse::*;
pub use warranties::*;
pub use work_orders::*;
pub use x402_credits::*;
pub use x402_payment_intents::*;

use sqlx::postgres::{PgPool, PgPoolOptions};
use stateset_core::CommerceError;
use std::future::Future;
use std::time::Duration;

/// PostgreSQL database connection pool
#[derive(Clone)]
pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
    /// Connect to PostgreSQL database with URL
    pub async fn connect(url: &str) -> Result<Self, CommerceError> {
        Self::connect_with_options(url, 10, 30).await
    }

    /// Connect with custom options
    pub async fn connect_with_options(
        url: &str,
        max_connections: u32,
        acquire_timeout_secs: u64,
    ) -> Result<Self, CommerceError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
            .connect(url)
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Run migrations
        Self::run_migrations(&pool).await?;

        Ok(Self { pool })
    }

    /// Run database migrations
    async fn run_migrations(pool: &PgPool) -> Result<(), CommerceError> {
        // Create migrations table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Get list of migrations
        let mut migrations = vec![
            (
                "001_initial_schema",
                include_str!("migrations/001_initial_schema.sql"),
            ),
            (
                "001a_pgcrypto",
                include_str!("migrations/001a_pgcrypto.sql"),
            ),
            (
                "002_inventory",
                include_str!("migrations/002_inventory.sql"),
            ),
            ("003_returns", include_str!("migrations/003_returns.sql")),
            (
                "004_manufacturing",
                include_str!("migrations/004_manufacturing.sql"),
            ),
            ("005_currency", include_str!("migrations/005_currency.sql")),
            (
                "006_shipments",
                include_str!("migrations/006_shipments.sql"),
            ),
            ("007_payments", include_str!("migrations/007_payments.sql")),
            (
                "008_warranties",
                include_str!("migrations/008_warranties.sql"),
            ),
            (
                "009_purchase_orders",
                include_str!("migrations/009_purchase_orders.sql"),
            ),
            ("010_invoices", include_str!("migrations/010_invoices.sql")),
            ("011_carts", include_str!("migrations/011_carts.sql")),
            (
                "012_versioning",
                include_str!("migrations/012_versioning.sql"),
            ),
            (
                "013_versioning_catalog",
                include_str!("migrations/013_versioning_catalog.sql"),
            ),
            ("014_tax", include_str!("migrations/014_tax.sql")),
            (
                "015_promotions",
                include_str!("migrations/015_promotions.sql"),
            ),
            (
                "016_subscriptions",
                include_str!("migrations/016_subscriptions.sql"),
            ),
            ("017_quality", include_str!("migrations/017_quality.sql")),
            ("018_lots", include_str!("migrations/018_lots.sql")),
            ("019_serials", include_str!("migrations/019_serials.sql")),
            (
                "020_warehouse",
                include_str!("migrations/020_warehouse.sql"),
            ),
            (
                "021_receiving",
                include_str!("migrations/021_receiving.sql"),
            ),
            (
                "022_fulfillment",
                include_str!("migrations/022_fulfillment.sql"),
            ),
            (
                "023_accounts_payable",
                include_str!("migrations/023_accounts_payable.sql"),
            ),
            (
                "024_cost_accounting",
                include_str!("migrations/024_cost_accounting.sql"),
            ),
            ("025_credit", include_str!("migrations/025_credit.sql")),
            (
                "026_backorder",
                include_str!("migrations/026_backorder.sql"),
            ),
            (
                "027_accounts_receivable",
                include_str!("migrations/027_accounts_receivable.sql"),
            ),
            (
                "028_general_ledger",
                include_str!("migrations/028_general_ledger.sql"),
            ),
            (
                "029_performance_indexes",
                include_str!("migrations/029_performance_indexes.sql"),
            ),
            (
                "030_idempotency_keys",
                include_str!("migrations/030_idempotency_keys.sql"),
            ),
            (
                "031_x402_credits",
                include_str!("migrations/031_x402_credits.sql"),
            ),
            ("032_erc8004", include_str!("migrations/032_erc8004.sql")),
            ("033_x402_a2a", include_str!("migrations/033_x402_a2a.sql")),
            (
                "034_custom_objects",
                include_str!("migrations/034_custom_objects.sql"),
            ),
        ];

        // Optional, experimental migrations.
        #[cfg(feature = "saga")]
        {
            migrations.push(("035_sagas", include_str!("migrations/035_sagas.sql")));
        }

        migrations.push((
            "036_orders_cart_id",
            include_str!("migrations/036_orders_cart_id.sql"),
        ));

        for (name, sql) in migrations {
            // Check if migration already applied
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _migrations WHERE name = $1")
                .bind(name)
                .fetch_one(pool)
                .await
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            if count.0 == 0 {
                // Run migration
                sqlx::raw_sql(sql).execute(pool).await.map_err(|e| {
                    CommerceError::DatabaseError(format!("Migration {} failed: {}", name, e))
                })?;

                // Record migration
                sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
                    .bind(name)
                    .execute(pool)
                    .await
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            }
        }

        Ok(())
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

    /// Get custom objects repository (custom states / metaobjects)
    pub fn custom_objects(&self) -> PgCustomObjectRepository {
        PgCustomObjectRepository::new(self.pool.clone())
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

    /// Get underlying pool (for advanced use)
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Helper function to convert sqlx errors to CommerceError
pub(crate) fn map_db_error(e: sqlx::Error) -> CommerceError {
    match e {
        sqlx::Error::RowNotFound => CommerceError::NotFound,
        _ => CommerceError::DatabaseError(e.to_string()),
    }
}

pub(crate) fn block_on<F, T>(fut: F) -> stateset_core::Result<T>
where
    F: Future<Output = stateset_core::Result<T>>,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(CommerceError::NotPermitted(
            "Blocking Postgres call inside an async runtime; use AsyncCommerce instead".into(),
        ));
    }

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CommerceError::Internal(format!("Failed to create runtime: {}", e)))?;
    rt.block_on(fut)
}
