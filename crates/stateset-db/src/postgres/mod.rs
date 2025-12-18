//! PostgreSQL database implementation using sqlx
//!
//! This module provides async PostgreSQL support for production deployments.

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
mod purchase_orders;
mod returns;
mod shipments;
mod unsupported;
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
pub use purchase_orders::*;
pub use returns::*;
pub use shipments::*;
pub use unsupported::*;
pub use warranties::*;
pub use work_orders::*;

use sqlx::postgres::{PgPool, PgPoolOptions};
use stateset_core::CommerceError;
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
        let migrations = vec![
            ("001_initial_schema", include_str!("migrations/001_initial_schema.sql")),
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
        ];

        for (name, sql) in migrations {
            // Check if migration already applied
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM _migrations WHERE name = $1"
            )
            .bind(name)
            .fetch_one(pool)
            .await
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            if count.0 == 0 {
                // Run migration
                sqlx::raw_sql(sql)
                    .execute(pool)
                    .await
                    .map_err(|e| CommerceError::DatabaseError(format!("Migration {} failed: {}", name, e)))?;

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
