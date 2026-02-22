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

impl ZoneShippingMethodRepository for SqliteZoneShippingMethodRepository {
    fn create(&self, _input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod> {
        todo!("SQLite zone shipping method create")
    }

    fn get(&self, _id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        todo!("SQLite zone shipping method get")
    }

    fn list(&self, _filter: ZoneShippingMethodFilter) -> Result<Vec<ZoneShippingMethod>> {
        todo!("SQLite zone shipping method list")
    }

    fn delete(&self, _id: ShippingMethodId) -> Result<()> {
        todo!("SQLite zone shipping method delete")
    }

    fn calculate_rates(
        &self,
        _request: ZoneShippingRateRequest,
    ) -> Result<Vec<ZoneShippingRate>> {
        todo!("SQLite zone shipping method calculate_rates")
    }
}
