//! SQLite implementation of reward catalog repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateReward, Result, Reward, RewardFilter, RewardId, RewardRepository,
};

#[derive(Debug)]
pub struct SqliteRewardRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteRewardRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl RewardRepository for SqliteRewardRepository {
    fn create(&self, _input: CreateReward) -> Result<Reward> {
        Err(CommerceError::DatabaseError("SQLite reward create not yet implemented".to_string()))
    }

    fn get(&self, _id: RewardId) -> Result<Option<Reward>> {
        Err(CommerceError::DatabaseError("SQLite reward get not yet implemented".to_string()))
    }

    fn list(&self, _filter: RewardFilter) -> Result<Vec<Reward>> {
        Err(CommerceError::DatabaseError("SQLite reward list not yet implemented".to_string()))
    }

    fn delete(&self, _id: RewardId) -> Result<()> {
        Err(CommerceError::DatabaseError("SQLite reward delete not yet implemented".to_string()))
    }
}
