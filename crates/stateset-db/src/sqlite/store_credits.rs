//! SQLite implementation of store credit repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AdjustStoreCredit, CommerceError, CreateStoreCredit, Result, StoreCredit, StoreCreditFilter,
    StoreCreditId, StoreCreditRepository, StoreCreditTransaction,
};

#[derive(Debug)]
pub struct SqliteStoreCreditRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteStoreCreditRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl StoreCreditRepository for SqliteStoreCreditRepository {
    fn create(&self, _input: CreateStoreCredit) -> Result<StoreCredit> {
        Err(CommerceError::DatabaseError(
            "SQLite store credit create not yet implemented".to_string(),
        ))
    }

    fn get(&self, _id: StoreCreditId) -> Result<Option<StoreCredit>> {
        Err(CommerceError::DatabaseError("SQLite store credit get not yet implemented".to_string()))
    }

    fn list(&self, _filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        Err(CommerceError::DatabaseError(
            "SQLite store credit list not yet implemented".to_string(),
        ))
    }

    fn adjust(&self, _id: StoreCreditId, _input: AdjustStoreCredit) -> Result<StoreCredit> {
        Err(CommerceError::DatabaseError(
            "SQLite store credit adjust not yet implemented".to_string(),
        ))
    }

    fn apply(
        &self,
        _id: StoreCreditId,
        _amount: Decimal,
        _reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        Err(CommerceError::DatabaseError(
            "SQLite store credit apply not yet implemented".to_string(),
        ))
    }

    fn get_transactions(
        &self,
        _store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        Err(CommerceError::DatabaseError(
            "SQLite store credit get_transactions not yet implemented".to_string(),
        ))
    }
}
