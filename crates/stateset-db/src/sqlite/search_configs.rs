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
        todo!("SQLite search config create")
    }

    fn get(&self, _id: SearchConfigId) -> Result<Option<SearchConfig>> {
        todo!("SQLite search config get")
    }

    fn update(&self, _id: SearchConfigId, _input: UpdateSearchConfig) -> Result<SearchConfig> {
        todo!("SQLite search config update")
    }

    fn list(&self, _filter: SearchConfigFilter) -> Result<Vec<SearchConfig>> {
        todo!("SQLite search config list")
    }

    fn delete(&self, _id: SearchConfigId) -> Result<()> {
        todo!("SQLite search config delete")
    }

    fn get_active(&self) -> Result<Option<SearchConfig>> {
        todo!("SQLite search config get_active")
    }

    fn set_active(&self, _id: SearchConfigId) -> Result<SearchConfig> {
        todo!("SQLite search config set_active")
    }
}
