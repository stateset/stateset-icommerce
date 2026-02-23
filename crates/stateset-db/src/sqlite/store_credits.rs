//! SQLite implementation of store credit repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AdjustStoreCredit, CommerceError, CreateStoreCredit, Result, StoreCredit,
    StoreCreditFilter, StoreCreditId, StoreCreditRepository, StoreCreditTransaction,
};

#[derive(Debug)]
pub struct SqliteStoreCreditRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteStoreCreditRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl StoreCreditRepository for SqliteStoreCreditRepository {
    fn create(&self, _input: CreateStoreCredit) -> Result<StoreCredit> {
        todo!("SQLite store credit create")
    }

    fn get(&self, _id: StoreCreditId) -> Result<Option<StoreCredit>> {
        todo!("SQLite store credit get")
    }

    fn list(&self, _filter: StoreCreditFilter) -> Result<Vec<StoreCredit>> {
        todo!("SQLite store credit list")
    }

    fn adjust(&self, _id: StoreCreditId, _input: AdjustStoreCredit) -> Result<StoreCredit> {
        todo!("SQLite store credit adjust")
    }

    fn apply(
        &self,
        _id: StoreCreditId,
        _amount: Decimal,
        _reference_id: Option<String>,
    ) -> Result<StoreCreditTransaction> {
        todo!("SQLite store credit apply")
    }

    fn get_transactions(
        &self,
        _store_credit_id: StoreCreditId,
    ) -> Result<Vec<StoreCreditTransaction>> {
        todo!("SQLite store credit get_transactions")
    }
}
