//! SQLite implementation of zone shipping method repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateZoneShippingMethod, Result, ShippingMethodId, ZoneShippingMethod,
    ZoneShippingMethodFilter, ZoneShippingMethodRepository, ZoneShippingRate,
    ZoneShippingRateRequest,
};

#[derive(Debug)]
pub struct SqliteZoneShippingMethodRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteZoneShippingMethodRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl ZoneShippingMethodRepository for SqliteZoneShippingMethodRepository {
    fn create(&self, _input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod> {
        Err(CommerceError::DatabaseError(
            "SQLite zone shipping method create not yet implemented".to_string(),
        ))
    }

    fn get(&self, _id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        Err(CommerceError::DatabaseError(
            "SQLite zone shipping method get not yet implemented".to_string(),
        ))
    }

    fn list(&self, _filter: ZoneShippingMethodFilter) -> Result<Vec<ZoneShippingMethod>> {
        Err(CommerceError::DatabaseError(
            "SQLite zone shipping method list not yet implemented".to_string(),
        ))
    }

    fn delete(&self, _id: ShippingMethodId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite zone shipping method delete not yet implemented".to_string(),
        ))
    }

    fn calculate_rates(&self, _request: ZoneShippingRateRequest) -> Result<Vec<ZoneShippingRate>> {
        Err(CommerceError::DatabaseError(
            "SQLite zone shipping method calculate_rates not yet implemented".to_string(),
        ))
    }
}
