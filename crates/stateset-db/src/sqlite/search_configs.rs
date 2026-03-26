//! SQLite implementation of search configuration repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateSearchConfig, Result, SearchConfig, SearchConfigFilter, SearchConfigId,
    SearchConfigRepository, UpdateSearchConfig,
};

#[derive(Debug)]
pub struct SqliteSearchConfigRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSearchConfigRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl SearchConfigRepository for SqliteSearchConfigRepository {
    fn create(&self, _input: CreateSearchConfig) -> Result<SearchConfig> {
        Err(CommerceError::DatabaseError(
            "SQLite search config create not yet implemented".to_string(),
        ))
    }

    fn get(&self, _id: SearchConfigId) -> Result<Option<SearchConfig>> {
        Err(CommerceError::DatabaseError(
            "SQLite search config get not yet implemented".to_string(),
        ))
    }

    fn update(&self, _id: SearchConfigId, _input: UpdateSearchConfig) -> Result<SearchConfig> {
        Err(CommerceError::DatabaseError(
            "SQLite search config update not yet implemented".to_string(),
        ))
    }

    fn list(&self, _filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        Err(CommerceError::DatabaseError(
            "SQLite search config list not yet implemented".to_string(),
        ))
    }

    fn delete(&self, _id: SearchConfigId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite search config delete not yet implemented".to_string(),
        ))
    }

    fn get_active(&self) -> Result<Option<SearchConfig>> {
        Err(CommerceError::DatabaseError(
            "SQLite search config get_active not yet implemented".to_string(),
        ))
    }

    fn set_active(&self, _id: SearchConfigId) -> Result<SearchConfig> {
        Err(CommerceError::DatabaseError(
            "SQLite search config set_active not yet implemented".to_string(),
        ))
    }
}
