//! Review endpoints.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

// ============================================================================
// DTOs
// ============================================================================

/// Request body for `POST /api/v1/reviews`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateReviewRequest {
    /// Product being reviewed.
    #[schema(value_type = String, format = "uuid")]
    pub product_id: String,
    /// Customer who wrote the review.
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: String,
    /// Star rating (1-5).
    pub rating: u8,
    /// Review headline.
    pub title: Option<String>,
    /// Review body text.
    pub body: Option<String>,
    /// Whether the reviewer actually purchased the product.
    pub verified_purchase: Option<bool>,
}

/// Query parameters for `GET /api/v1/reviews` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct ReviewFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by product ID (UUID).
    pub product_id: Option<String>,
    /// Filter by review status (pending, approved, rejected).
    pub status: Option<String>,
    /// Filter by minimum rating (1-5).
    pub min_rating: Option<u8>,
}

impl ReviewFilterParams {
    /// Default page size.
    const DEFAULT_LIMIT: u32 = 50;
    /// Maximum allowed page size.
    const MAX_LIMIT: u32 = 200;

    /// Resolved limit with bounds checking.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        self.limit
            .unwrap_or(Self::DEFAULT_LIMIT)
            .clamp(1, Self::MAX_LIMIT)
    }

    /// Resolved offset.
    #[must_use]
    pub fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Response body for a single review.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ReviewResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: String,
    #[schema(value_type = String, format = "uuid")]
    pub product_id: String,
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: String,
    pub rating: u8,
    pub title: String,
    pub body: String,
    pub status: String,
    pub verified_purchase: bool,
    pub helpful_count: i64,
    pub created_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/reviews` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct ReviewListResponse {
    pub reviews: Vec<ReviewResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Router
// ============================================================================

/// Build the reviews sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/reviews", post(create_review).get(list_reviews))
        .route("/reviews/{id}", get(get_review).delete(delete_review))
}

// ============================================================================
// Handlers
// ============================================================================

/// `POST /api/v1/reviews`
#[utoipa::path(
    post,
    path = "/api/v1/reviews",
    tag = "reviews",
    request_body = CreateReviewRequest,
    responses(
        (status = 201, description = "Review created", body = ReviewResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateReviewRequest>,
) -> Result<(StatusCode, Json<ReviewResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    if req.rating < 1 || req.rating > 5 {
        return Err(HttpError::BadRequest(
            "Rating must be between 1 and 5".to_string(),
        ));
    }

    let review = commerce.reviews().create(stateset_core::CreateReview {
        product_id: req
            .product_id
            .parse()
            .map_err(|e| HttpError::BadRequest(format!("Invalid product_id: {e}")))?,
        customer_id: req
            .customer_id
            .parse()
            .map_err(|e| HttpError::BadRequest(format!("Invalid customer_id: {e}")))?,
        rating: req.rating,
        title: req.title,
        body: req.body,
        verified_purchase: req.verified_purchase.unwrap_or(false),
    })?;

    Ok((StatusCode::CREATED, Json(review_to_response(review))))
}

/// `GET /api/v1/reviews/{id}`
#[utoipa::path(
    get,
    path = "/api/v1/reviews/{id}",
    tag = "reviews",
    params(("id" = String, Path, description = "Review ID (UUID)")),
    responses(
        (status = 200, description = "Review details", body = ReviewResponse),
        (status = 404, description = "Review not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::ReviewId>,
) -> Result<Json<ReviewResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let review = commerce
        .reviews()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Review {id} not found")))?;
    Ok(Json(review_to_response(review)))
}

/// `GET /api/v1/reviews`
#[utoipa::path(
    get,
    path = "/api/v1/reviews",
    tag = "reviews",
    params(ReviewFilterParams),
    responses(
        (status = 200, description = "List of reviews", body = ReviewListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_reviews(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ReviewFilterParams>,
) -> Result<Json<ReviewListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    let product_id = params
        .product_id
        .map(|s| s.parse::<stateset_primitives::ProductId>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid product_id: {e}")))?;

    let status = params
        .status
        .map(|s| s.parse::<stateset_core::ReviewStatus>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid status: {e}")))?;

    // Count total matching records (without pagination)
    let count_filter = stateset_core::ReviewFilter {
        product_id,
        customer_id: None,
        status,
        min_rating: params.min_rating,
        verified_only: None,
        limit: None,
        offset: None,
    };
    let total = commerce.reviews().list(count_filter)?.len();

    // Fetch the requested page
    let filter = stateset_core::ReviewFilter {
        product_id,
        customer_id: None,
        status,
        min_rating: params.min_rating,
        verified_only: None,
        limit: Some(limit.saturating_add(1)),
        offset: Some(offset),
    };
    let mut reviews = commerce.reviews().list(filter)?;
    let has_more = reviews.len() > limit as usize;
    if has_more {
        reviews.truncate(limit as usize);
    }

    Ok(Json(ReviewListResponse {
        reviews: reviews.into_iter().map(review_to_response).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `DELETE /api/v1/reviews/{id}`
#[utoipa::path(
    delete,
    path = "/api/v1/reviews/{id}",
    tag = "reviews",
    params(("id" = String, Path, description = "Review ID (UUID)")),
    responses(
        (status = 204, description = "Review deleted"),
        (status = 404, description = "Review not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::ReviewId>,
) -> Result<StatusCode, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    commerce.reviews().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Helpers
// ============================================================================

fn review_to_response(r: stateset_core::Review) -> ReviewResponse {
    ReviewResponse {
        id: r.id.to_string(),
        product_id: r.product_id.to_string(),
        customer_id: r.customer_id.to_string(),
        rating: r.rating,
        title: r.title.unwrap_or_default(),
        body: r.body.unwrap_or_default(),
        status: r.status.to_string(),
        verified_purchase: r.verified_purchase,
        helpful_count: i64::from(r.helpful_count),
        created_at: r.created_at,
    }
}

#[cfg(test)]
mod tests {}
