//! Product review operations for managing customer reviews and ratings
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateReview, ProductId, CustomerId};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let review = commerce.reviews().create(CreateReview {
//!     product_id: ProductId::new(),
//!     customer_id: CustomerId::new(),
//!     rating: 5,
//!     title: Some("Excellent product!".into()),
//!     body: Some("Works exactly as described.".into()),
//!     ..Default::default()
//! })?;
//!
//! println!("Review created with rating: {}", review.rating);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    CreateReview, ProductId, Result, Review, ReviewFilter, ReviewId, ReviewSummary, UpdateReview,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Product review operations.
pub struct Reviews {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Reviews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reviews").finish_non_exhaustive()
    }
}

impl Reviews {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether reviews are supported by the active backend.
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Reviews)
    }

    fn ensure_supported(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Reviews)
    }

    /// Create a new product review.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateReview, ProductId, CustomerId};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let review = commerce.reviews().create(CreateReview {
    ///     product_id: ProductId::new(),
    ///     customer_id: CustomerId::new(),
    ///     rating: 4,
    ///     title: Some("Good value".into()),
    ///     body: Some("Decent quality for the price.".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateReview) -> Result<Review> {
        self.ensure_supported()?;
        self.db.reviews().create(input)
    }

    /// Get a review by ID.
    pub fn get(&self, id: ReviewId) -> Result<Option<Review>> {
        self.ensure_supported()?;
        self.db.reviews().get(id)
    }

    /// Update a review.
    pub fn update(&self, id: ReviewId, input: UpdateReview) -> Result<Review> {
        self.ensure_supported()?;
        self.db.reviews().update(id, input)
    }

    /// List reviews with optional filtering.
    pub fn list(&self, filter: ReviewFilter) -> Result<Vec<Review>> {
        self.ensure_supported()?;
        self.db.reviews().list(filter)
    }

    /// Delete a review.
    pub fn delete(&self, id: ReviewId) -> Result<()> {
        self.ensure_supported()?;
        self.db.reviews().delete(id)
    }

    /// Get aggregate review summary for a product.
    ///
    /// Returns average rating, total count, and rating distribution.
    pub fn get_summary(&self, product_id: ProductId) -> Result<ReviewSummary> {
        self.ensure_supported()?;
        self.db.reviews().get_summary(product_id)
    }

    /// Mark a review as helpful (increment helpful count).
    pub fn mark_helpful(&self, id: ReviewId) -> Result<()> {
        self.ensure_supported()?;
        self.db.reviews().mark_helpful(id)
    }

    /// Mark a review as reported (increment reported count).
    pub fn mark_reported(&self, id: ReviewId) -> Result<()> {
        self.ensure_supported()?;
        self.db.reviews().mark_reported(id)
    }
}
