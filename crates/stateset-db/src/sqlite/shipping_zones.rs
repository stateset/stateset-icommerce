//! SQLite implementation of shipping zone repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateShippingZone, Result, ShippingZone, ShippingZoneFilter, ShippingZoneId,
    ShippingZoneRepository, UpdateShippingZone,
};

#[derive(Debug)]
pub struct SqliteShippingZoneRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteShippingZoneRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl ShippingZoneRepository for SqliteShippingZoneRepository {
    fn create(&self, _input: CreateShippingZone) -> Result<ShippingZone> {
        Err(CommerceError::DatabaseError(
            "SQLite shipping zone create not yet implemented".to_string(),
        ))
    }

    fn get(&self, _id: ShippingZoneId) -> Result<Option<ShippingZone>> {
        Err(CommerceError::DatabaseError(
            "SQLite shipping zone get not yet implemented".to_string(),
        ))
    }

    fn update(&self, _id: ShippingZoneId, _input: UpdateShippingZone) -> Result<ShippingZone> {
        Err(CommerceError::DatabaseError(
            "SQLite shipping zone update not yet implemented".to_string(),
        ))
    }

    fn list(&self, _filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        Err(CommerceError::DatabaseError(
            "SQLite shipping zone list not yet implemented".to_string(),
        ))
    }

    fn delete(&self, _id: ShippingZoneId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite shipping zone delete not yet implemented".to_string(),
        ))
    }

    fn find_matching_zones(
        &self,
        _country: &str,
        _region: Option<&str>,
        _postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        Err(CommerceError::DatabaseError(
            "SQLite shipping zone find_matching_zones not yet implemented".to_string(),
        ))
    }
}
