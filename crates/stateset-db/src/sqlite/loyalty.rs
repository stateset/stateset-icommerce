//! SQLite implementation of loyalty program repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AdjustPoints, CommerceError, CreateLoyaltyProgram, CustomerId, EnrollCustomer,
    LoyaltyAccount, LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyProgramRepository, LoyaltyTransaction, Result,
};

#[derive(Debug)]
pub struct SqliteLoyaltyProgramRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteLoyaltyProgramRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl LoyaltyProgramRepository for SqliteLoyaltyProgramRepository {
    fn create(&self, _input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        todo!("SQLite loyalty program create")
    }

    fn get(&self, _id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        todo!("SQLite loyalty program get")
    }

    fn list(&self) -> Result<Vec<LoyaltyProgram>> {
        todo!("SQLite loyalty program list")
    }

    fn enroll(&self, _input: EnrollCustomer) -> Result<LoyaltyAccount> {
        todo!("SQLite loyalty program enroll")
    }

    fn get_account(&self, _id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        todo!("SQLite loyalty program get_account")
    }

    fn get_account_by_customer(
        &self,
        _customer_id: CustomerId,
        _program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        todo!("SQLite loyalty program get_account_by_customer")
    }

    fn list_accounts(&self, _filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        todo!("SQLite loyalty program list_accounts")
    }

    fn adjust_points(&self, _input: AdjustPoints) -> Result<LoyaltyTransaction> {
        todo!("SQLite loyalty program adjust_points")
    }

    fn get_transactions(
        &self,
        _account_id: LoyaltyAccountId,
        _limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        todo!("SQLite loyalty program get_transactions")
    }
}
