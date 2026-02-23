//! Product review and ratings domain models
//!
//! Handles customer reviews, moderation workflow, and aggregate rating summaries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::{CustomerId, ProductId, ReviewId};
use strum::{Display, EnumString};

/// Review moderation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ReviewStatus {
    /// Awaiting moderation
    #[default]
    Pending,
    /// Approved and visible
    Approved,
    /// Rejected by moderator
    Rejected,
    /// Flagged for further review
    Flagged,
}

/// A product review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    /// Unique review ID
    pub id: ReviewId,
    /// Product being reviewed
    pub product_id: ProductId,
    /// Customer who wrote the review
    pub customer_id: CustomerId,
    /// Rating (1-5 stars)
    pub rating: u8,
    /// Review title/headline
    pub title: Option<String>,
    /// Review body text
    pub body: Option<String>,
    /// Moderation status
    pub status: ReviewStatus,
    /// Whether the reviewer purchased the product
    pub verified_purchase: bool,
    /// Number of "helpful" votes
    pub helpful_count: u32,
    /// Number of times reported as inappropriate
    pub reported_count: u32,
    /// When the review was created
    pub created_at: DateTime<Utc>,
    /// When the review was last updated
    pub updated_at: DateTime<Utc>,
}

/// Aggregate rating summary for a product
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSummary {
    /// Product ID
    pub product_id: ProductId,
    /// Average rating (1.0-5.0)
    pub average_rating: f64,
    /// Total number of reviews
    pub total_reviews: u64,
    /// Distribution of ratings: index 0 = 1-star count, index 4 = 5-star count
    pub rating_distribution: [u32; 5],
}

/// Input for creating a review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReview {
    /// Product being reviewed
    pub product_id: ProductId,
    /// Customer writing the review
    pub customer_id: CustomerId,
    /// Rating (1-5)
    pub rating: u8,
    /// Review title
    pub title: Option<String>,
    /// Review body
    pub body: Option<String>,
    /// Whether purchase was verified
    pub verified_purchase: bool,
}

/// Input for updating a review
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateReview {
    /// Updated rating
    pub rating: Option<u8>,
    /// Updated title
    pub title: Option<Option<String>>,
    /// Updated body
    pub body: Option<Option<String>>,
    /// Updated status
    pub status: Option<ReviewStatus>,
}

/// Filter for listing reviews
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewFilter {
    /// Filter by product
    pub product_id: Option<ProductId>,
    /// Filter by customer
    pub customer_id: Option<CustomerId>,
    /// Filter by status
    pub status: Option<ReviewStatus>,
    /// Filter by minimum rating
    pub min_rating: Option<u8>,
    /// Only verified purchases
    pub verified_only: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl Review {
    /// Whether this review is publicly visible
    pub fn is_visible(&self) -> bool {
        self.status == ReviewStatus::Approved
    }

    /// Validate that the rating is in the valid range (1-5)
    pub fn is_valid_rating(rating: u8) -> bool {
        (1..=5).contains(&rating)
    }
}

impl ReviewSummary {
    /// Create an empty summary for a product with no reviews
    pub const fn empty(product_id: ProductId) -> Self {
        Self {
            product_id,
            average_rating: 0.0,
            total_reviews: 0,
            rating_distribution: [0; 5],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_primitives::{CustomerId, ProductId, ReviewId};

    fn make_review(status: ReviewStatus, rating: u8) -> Review {
        Review {
            id: ReviewId::new(),
            product_id: ProductId::new(),
            customer_id: CustomerId::new(),
            rating,
            title: Some("Great product".to_string()),
            body: Some("Really happy with this.".to_string()),
            status,
            verified_purchase: true,
            helpful_count: 0,
            reported_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- is_visible ----

    #[test]
    fn is_visible_returns_true_when_approved() {
        let review = make_review(ReviewStatus::Approved, 5);
        assert!(review.is_visible());
    }

    #[test]
    fn is_visible_returns_false_when_pending() {
        let review = make_review(ReviewStatus::Pending, 5);
        assert!(!review.is_visible());
    }

    #[test]
    fn is_visible_returns_false_when_rejected() {
        let review = make_review(ReviewStatus::Rejected, 5);
        assert!(!review.is_visible());
    }

    #[test]
    fn is_visible_returns_false_when_flagged() {
        let review = make_review(ReviewStatus::Flagged, 5);
        assert!(!review.is_visible());
    }

    // ---- is_valid_rating ----

    #[test]
    fn is_valid_rating_accepts_one_through_five() {
        for r in 1u8..=5 {
            assert!(Review::is_valid_rating(r), "rating {r} should be valid");
        }
    }

    #[test]
    fn is_valid_rating_rejects_zero() {
        assert!(!Review::is_valid_rating(0));
    }

    #[test]
    fn is_valid_rating_rejects_six() {
        assert!(!Review::is_valid_rating(6));
    }

    // ---- ReviewSummary::empty ----

    #[test]
    fn review_summary_empty_has_zero_totals() {
        let product_id = ProductId::new();
        let summary = ReviewSummary::empty(product_id);
        assert_eq!(summary.total_reviews, 0);
        assert_eq!(summary.average_rating, 0.0);
        assert_eq!(summary.rating_distribution, [0u32; 5]);
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn review_status_display_fromstr_roundtrip() {
        for status in [
            ReviewStatus::Pending,
            ReviewStatus::Approved,
            ReviewStatus::Rejected,
            ReviewStatus::Flagged,
        ] {
            let s = status.to_string();
            let parsed: ReviewStatus = s.parse().unwrap();
            assert_eq!(parsed, status, "round-trip failed for {s}");
        }
    }

    // ---- Default ----

    #[test]
    fn review_status_default_is_pending() {
        assert_eq!(ReviewStatus::default(), ReviewStatus::Pending);
    }
}
