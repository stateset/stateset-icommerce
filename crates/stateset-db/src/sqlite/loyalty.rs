//! SQLite implementation of loyalty program repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AdjustPoints, CommerceError, CreateLoyaltyProgram, CustomerId, EnrollCustomer, LoyaltyAccount,
    LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyProgramRepository, LoyaltyTransaction, Result,
};

#[derive(Debug)]
pub struct SqliteLoyaltyProgramRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteLoyaltyProgramRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl LoyaltyProgramRepository for SqliteLoyaltyProgramRepository {
    fn create(&self, _input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program create not yet implemented".to_string(),
        ))
    }

    fn get(&self, _id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program get not yet implemented".to_string(),
        ))
    }

    fn list(&self) -> Result<Vec<LoyaltyProgram>> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program list not yet implemented".to_string(),
        ))
    }

    fn enroll(&self, _input: EnrollCustomer) -> Result<LoyaltyAccount> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program enroll not yet implemented".to_string(),
        ))
    }

    fn get_account(&self, _id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program get_account not yet implemented".to_string(),
        ))
    }

    fn get_account_by_customer(
        &self,
        _customer_id: CustomerId,
        _program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program get_account_by_customer not yet implemented".to_string(),
        ))
    }

    fn list_accounts(&self, _filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program list_accounts not yet implemented".to_string(),
        ))
    }

    fn adjust_points(&self, _input: AdjustPoints) -> Result<LoyaltyTransaction> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program adjust_points not yet implemented".to_string(),
        ))
    }

    fn get_transactions(
        &self,
        _account_id: LoyaltyAccountId,
        _limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        Err(CommerceError::DatabaseError(
            "SQLite loyalty program get_transactions not yet implemented".to_string(),
        ))
    }
}
