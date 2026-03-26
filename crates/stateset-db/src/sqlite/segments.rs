//! SQLite implementation of customer segment repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateSegment, CustomerId, Result, Segment, SegmentFilter, SegmentId,
    SegmentMembership, SegmentRepository, UpdateSegment,
};

#[derive(Debug)]
pub struct SqliteSegmentRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSegmentRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl SegmentRepository for SqliteSegmentRepository {
    fn create(&self, _input: CreateSegment) -> Result<Segment> {
        Err(CommerceError::DatabaseError("SQLite segment create not yet implemented".to_string()))
    }

    fn get(&self, _id: SegmentId) -> Result<Option<Segment>> {
        Err(CommerceError::DatabaseError("SQLite segment get not yet implemented".to_string()))
    }

    fn update(&self, _id: SegmentId, _input: UpdateSegment) -> Result<Segment> {
        Err(CommerceError::DatabaseError("SQLite segment update not yet implemented".to_string()))
    }

    fn list(&self, _filter: SegmentFilter) -> Result<Vec<Segment>> {
        Err(CommerceError::DatabaseError("SQLite segment list not yet implemented".to_string()))
    }

    fn delete(&self, _id: SegmentId) -> Result<()> {
        Err(CommerceError::DatabaseError("SQLite segment delete not yet implemented".to_string()))
    }

    fn add_member(
        &self,
        _segment_id: SegmentId,
        _customer_id: CustomerId,
    ) -> Result<SegmentMembership> {
        Err(CommerceError::DatabaseError(
            "SQLite segment add_member not yet implemented".to_string(),
        ))
    }

    fn remove_member(&self, _segment_id: SegmentId, _customer_id: CustomerId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite segment remove_member not yet implemented".to_string(),
        ))
    }

    fn list_members(
        &self,
        _segment_id: SegmentId,
        _limit: Option<u32>,
        _offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        Err(CommerceError::DatabaseError(
            "SQLite segment list_members not yet implemented".to_string(),
        ))
    }

    fn is_member(&self, _segment_id: SegmentId, _customer_id: CustomerId) -> Result<bool> {
        Err(CommerceError::DatabaseError(
            "SQLite segment is_member not yet implemented".to_string(),
        ))
    }

    fn count_members(&self, _segment_id: SegmentId) -> Result<u64> {
        Err(CommerceError::DatabaseError(
            "SQLite segment count_members not yet implemented".to_string(),
        ))
    }
}
