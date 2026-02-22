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

impl ReviewRepository for SqliteReviewRepository {
    fn create(&self, _input: CreateReview) -> Result<Review> {
        todo!("SQLite review create")
    }

    fn get(&self, _id: ReviewId) -> Result<Option<Review>> {
        todo!("SQLite review get")
    }

    fn update(&self, _id: ReviewId, _input: UpdateReview) -> Result<Review> {
        todo!("SQLite review update")
    }

    fn list(&self, _filter: ReviewFilter) -> Result<Vec<Review>> {
        todo!("SQLite review list")
    }

    fn delete(&self, _id: ReviewId) -> Result<()> {
        todo!("SQLite review delete")
    }

    fn get_summary(&self, _product_id: ProductId) -> Result<ReviewSummary> {
        todo!("SQLite review get_summary")
    }

    fn mark_helpful(&self, _id: ReviewId) -> Result<()> {
        todo!("SQLite review mark_helpful")
    }

    fn mark_reported(&self, _id: ReviewId) -> Result<()> {
        todo!("SQLite review mark_reported")
    }
}
