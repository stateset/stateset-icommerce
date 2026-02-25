//! SQLite implementation of product review repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateReview, ProductId, Result, Review, ReviewFilter, ReviewId,
    ReviewRepository, ReviewSummary, UpdateReview,
};

#[derive(Debug)]
pub struct SqliteReviewRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteReviewRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl ReviewRepository for SqliteReviewRepository {
    fn create(&self, _input: CreateReview) -> Result<Review> {
        Err(CommerceError::DatabaseError("SQLite review create not yet implemented".to_string()))
    }

    fn get(&self, _id: ReviewId) -> Result<Option<Review>> {
        Err(CommerceError::DatabaseError("SQLite review get not yet implemented".to_string()))
    }

    fn update(&self, _id: ReviewId, _input: UpdateReview) -> Result<Review> {
        Err(CommerceError::DatabaseError("SQLite review update not yet implemented".to_string()))
    }

    fn list(&self, _filter: ReviewFilter) -> Result<Vec<Review>> {
        Err(CommerceError::DatabaseError("SQLite review list not yet implemented".to_string()))
    }

    fn delete(&self, _id: ReviewId) -> Result<()> {
        Err(CommerceError::DatabaseError("SQLite review delete not yet implemented".to_string()))
    }

    fn get_summary(&self, _product_id: ProductId) -> Result<ReviewSummary> {
        Err(CommerceError::DatabaseError(
            "SQLite review get_summary not yet implemented".to_string(),
        ))
    }

    fn mark_helpful(&self, _id: ReviewId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite review mark_helpful not yet implemented".to_string(),
        ))
    }

    fn mark_reported(&self, _id: ReviewId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite review mark_reported not yet implemented".to_string(),
        ))
    }
}
