//! PostgreSQL database implementation using sqlx
//!
//! This module provides async PostgreSQL support for production deployments.

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
mod warehouse;
mod warranties;
mod work_orders;
mod x402_credits;
mod x402_payment_intents;

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

    /// Get A2A quote/purchase repository
    pub fn a2a_quotes(&self) -> PgA2ARepository {
        PgA2ARepository::new(self.pool.clone())
    }

    /// Get A2A quote/purchase repository
    pub fn a2a_purchases(&self) -> PgA2ARepository {
        PgA2ARepository::new(self.pool.clone())
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
mod tests {
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
fn pg_transaction_isolation_sql(isolation: crate::TransactionIsolation) -> &'static str {
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
