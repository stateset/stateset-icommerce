//! PostgreSQL database implementation using sqlx
//!
//! This module provides async PostgreSQL support for production deployments.

mod bom;
mod customers;
mod inventory;
mod orders;
mod products;
mod returns;
mod unsupported;
mod work_orders;

pub use bom::*;
pub use customers::*;
pub use inventory::*;
pub use orders::*;
pub use products::*;
pub use returns::*;
pub use unsupported::*;
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
