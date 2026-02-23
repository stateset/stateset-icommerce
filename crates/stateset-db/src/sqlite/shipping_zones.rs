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

impl ShippingZoneRepository for SqliteShippingZoneRepository {
    fn create(&self, _input: CreateShippingZone) -> Result<ShippingZone> {
        todo!("SQLite shipping zone create")
    }

    fn get(&self, _id: ShippingZoneId) -> Result<Option<ShippingZone>> {
        todo!("SQLite shipping zone get")
    }

    fn update(&self, _id: ShippingZoneId, _input: UpdateShippingZone) -> Result<ShippingZone> {
        todo!("SQLite shipping zone update")
    }

    fn list(&self, _filter: ShippingZoneFilter) -> Result<Vec<ShippingZone>> {
        todo!("SQLite shipping zone list")
    }

    fn delete(&self, _id: ShippingZoneId) -> Result<()> {
        todo!("SQLite shipping zone delete")
    }

    fn find_matching_zones(
        &self,
        _country: &str,
        _region: Option<&str>,
        _postal_code: Option<&str>,
    ) -> Result<Vec<ShippingZone>> {
        todo!("SQLite shipping zone find_matching_zones")
    }
}
