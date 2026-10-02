//! Product reviews.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Product reviews
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReviewInput {
    pub product_id: String,
    pub customer_id: String,
    /// Star rating 1–5
    pub rating: u32,
    pub title: Option<String>,
    pub body: Option<String>,
    pub verified_purchase: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateReviewInput {
    pub rating: Option<u32>,
    pub title: Option<String>,
    pub body: Option<String>,
    /// Moderation status: pending, approved, rejected, flagged
    #[napi(ts_type = "ReviewStatus")]
    pub status: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ReviewFilterInput {
    pub product_id: Option<String>,
    pub customer_id: Option<String>,
    #[napi(ts_type = "ReviewStatus")]
    pub status: Option<String>,
    pub min_rating: Option<u32>,
    pub verified_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewOutput {
    pub id: String,
    pub product_id: String,
    pub customer_id: String,
    pub rating: u32,
    pub title: Option<String>,
    pub body: Option<String>,
    #[napi(ts_type = "ReviewStatus")]
    pub status: String,
    pub verified_purchase: bool,
    pub helpful_count: u32,
    pub reported_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Review> for ReviewOutput {
    fn from(r: stateset_core::Review) -> Self {
        Self {
            id: r.id.to_string(),
            product_id: r.product_id.to_string(),
            customer_id: r.customer_id.to_string(),
            rating: u32::from(r.rating),
            title: r.title,
            body: r.body,
            status: format!("{}", r.status),
            verified_purchase: r.verified_purchase,
            helpful_count: r.helpful_count,
            reported_count: r.reported_count,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewSummaryOutput {
    pub product_id: String,
    pub average_rating: f64,
    pub total_reviews: i64,
    /// Counts for 1★, 2★, 3★, 4★, 5★ (index 0 = 1 star)
    pub rating_distribution: Vec<u32>,
}

impl From<stateset_core::ReviewSummary> for ReviewSummaryOutput {
    fn from(s: stateset_core::ReviewSummary) -> Self {
        Self {
            product_id: s.product_id.to_string(),
            average_rating: s.average_rating,
            total_reviews: s.total_reviews as i64,
            rating_distribution: s.rating_distribution.to_vec(),
        }
    }
}

#[napi]
pub struct Reviews {
    pub(crate) commerce: Handle,
}

#[napi]
impl Reviews {
    /// Whether the reviews backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.reviews().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateReviewInput) -> Result<ReviewOutput> {
        let commerce = self.commerce.get()?;
        let product_id: uuid::Uuid = input
            .product_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;
        let customer_id: uuid::Uuid = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let rating = u8::try_from(input.rating)
            .map_err(|_| coded(ErrCode::Validation, "rating must be between 1 and 5"))?;
        let review = commerce
            .reviews()
            .create(stateset_core::CreateReview {
                product_id: product_id.into(),
                customer_id: customer_id.into(),
                rating,
                title: input.title,
                body: input.body,
                verified_purchase: input.verified_purchase.unwrap_or(false),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create review", e))?;
        Ok(review.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ReviewOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let review = commerce
            .reviews()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get review", e))?;
        Ok(review.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateReviewInput) -> Result<ReviewOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let rating = match input.rating {
            Some(r) => Some(
                u8::try_from(r)
                    .map_err(|_| coded(ErrCode::Validation, "rating must be between 1 and 5"))?,
            ),
            None => None,
        };
        let status = match input.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::ReviewStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid review status"))?,
            ),
            None => None,
        };
        let review = commerce
            .reviews()
            .update(
                uuid.into(),
                stateset_core::UpdateReview {
                    rating,
                    title: input.title.map(Some),
                    body: input.body.map(Some),
                    status,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update review", e))?;
        Ok(review.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<ReviewFilterInput>) -> Result<Vec<ReviewOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let product_id = match filter.product_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?
                    .into(),
            ),
            None => None,
        };
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let status = match filter.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::ReviewStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid review status"))?,
            ),
            None => None,
        };
        let min_rating = match filter.min_rating {
            Some(r) => Some(
                u8::try_from(r)
                    .map_err(|_| coded(ErrCode::Validation, "min_rating out of range"))?,
            ),
            None => None,
        };
        let reviews = commerce
            .reviews()
            .list(stateset_core::ReviewFilter {
                product_id,
                customer_id,
                status,
                min_rating,
                verified_only: filter.verified_only,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list reviews", e))?;
        Ok(reviews.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .reviews()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete review", e))?;
        Ok(())
    }

    /// Aggregate rating summary for a product (average, total, star distribution).
    #[napi]
    pub async fn get_summary(&self, product_id: String) -> Result<ReviewSummaryOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;
        let summary = commerce
            .reviews()
            .get_summary(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get review summary", e))?;
        Ok(summary.into())
    }

    #[napi]
    pub async fn mark_helpful(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .reviews()
            .mark_helpful(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark review helpful", e))?;
        Ok(())
    }

    #[napi]
    pub async fn mark_reported(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .reviews()
            .mark_reported(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark review reported", e))?;
        Ok(())
    }
}
