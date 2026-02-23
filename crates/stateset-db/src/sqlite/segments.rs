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

impl SegmentRepository for SqliteSegmentRepository {
    fn create(&self, _input: CreateSegment) -> Result<Segment> {
        todo!("SQLite segment create")
    }

    fn get(&self, _id: SegmentId) -> Result<Option<Segment>> {
        todo!("SQLite segment get")
    }

    fn update(&self, _id: SegmentId, _input: UpdateSegment) -> Result<Segment> {
        todo!("SQLite segment update")
    }

    fn list(&self, _filter: SegmentFilter) -> Result<Vec<Segment>> {
        todo!("SQLite segment list")
    }

    fn delete(&self, _id: SegmentId) -> Result<()> {
        todo!("SQLite segment delete")
    }

    fn add_member(
        &self,
        _segment_id: SegmentId,
        _customer_id: CustomerId,
    ) -> Result<SegmentMembership> {
        todo!("SQLite segment add_member")
    }

    fn remove_member(&self, _segment_id: SegmentId, _customer_id: CustomerId) -> Result<()> {
        todo!("SQLite segment remove_member")
    }

    fn list_members(
        &self,
        _segment_id: SegmentId,
        _limit: Option<u32>,
        _offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        todo!("SQLite segment list_members")
    }

    fn is_member(&self, _segment_id: SegmentId, _customer_id: CustomerId) -> Result<bool> {
        todo!("SQLite segment is_member")
    }

    fn count_members(&self, _segment_id: SegmentId) -> Result<u64> {
        todo!("SQLite segment count_members")
    }
}
