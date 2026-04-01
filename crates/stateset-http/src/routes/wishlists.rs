//! Wishlist endpoints.

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

/// Request body for `POST /api/v1/wishlists`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateWishlistRequest {
    /// Customer who owns the wishlist.
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: String,
    /// Human-readable name for the wishlist.
    pub name: Option<String>,
    /// Whether the wishlist is visible to others.
    pub is_public: Option<bool>,
}

/// Request body for `POST /api/v1/wishlists/:id/items`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct AddWishlistItemRequest {
    /// Product to add to the wishlist.
    #[schema(value_type = String, format = "uuid")]
    pub product_id: String,
    /// Optional note about the item.
    pub note: Option<String>,
}

/// Query parameters for `GET /api/v1/wishlists` with filtering.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct WishlistFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
    /// Filter by customer ID (UUID).
    pub customer_id: Option<String>,
}

impl WishlistFilterParams {
    /// Default page size.
    const DEFAULT_LIMIT: u32 = 50;
    /// Maximum allowed page size.
    const MAX_LIMIT: u32 = 200;

    /// Resolved limit with bounds checking.
    #[must_use]
    pub(crate) fn resolved_limit(&self) -> u32 {
        self.limit.unwrap_or(Self::DEFAULT_LIMIT).clamp(1, Self::MAX_LIMIT)
    }

    /// Resolved offset.
    #[must_use]
    pub(crate) fn resolved_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// A single item in a wishlist.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WishlistItemResponse {
    #[schema(value_type = String, format = "uuid")]
    pub product_id: String,
    pub note: Option<String>,
    pub added_at: DateTime<Utc>,
}

/// Response body for a single wishlist.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct WishlistResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: String,
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: String,
    pub name: String,
    pub is_public: bool,
    pub items: Vec<WishlistItemResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/wishlists` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct WishlistListResponse {
    pub wishlists: Vec<WishlistResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

// ============================================================================
// Router
// ============================================================================

/// Build the wishlists sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/wishlists", post(create_wishlist).get(list_wishlists))
        .route("/wishlists/{id}", get(get_wishlist).delete(delete_wishlist))
        .route("/wishlists/{id}/items", post(add_item))
        .route("/wishlists/{id}/items/{product_id}", axum::routing::delete(remove_item))
}

// ============================================================================
// Handlers
// ============================================================================

/// `POST /api/v1/wishlists`
#[utoipa::path(
    post,
    path = "/api/v1/wishlists",
    tag = "wishlists",
    request_body = CreateWishlistRequest,
    responses(
        (status = 201, description = "Wishlist created", body = WishlistResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_wishlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateWishlistRequest>,
) -> Result<(StatusCode, Json<WishlistResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let wishlist = commerce.wishlists().create(stateset_core::CreateWishlist {
        customer_id: req
            .customer_id
            .parse()
            .map_err(|e| HttpError::BadRequest(format!("Invalid customer_id: {e}")))?,
        name: req.name.unwrap_or_else(|| "My Wishlist".to_string()),
        is_public: req.is_public.unwrap_or(false),
    })?;

    Ok((StatusCode::CREATED, Json(wishlist_to_response(wishlist))))
}

/// `GET /api/v1/wishlists/{id}`
#[utoipa::path(
    get,
    path = "/api/v1/wishlists/{id}",
    tag = "wishlists",
    params(("id" = String, Path, description = "Wishlist ID (UUID)")),
    responses(
        (status = 200, description = "Wishlist details", body = WishlistResponse),
        (status = 404, description = "Wishlist not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_wishlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::WishlistId>,
) -> Result<Json<WishlistResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let wishlist = commerce
        .wishlists()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Wishlist {id} not found")))?;
    Ok(Json(wishlist_to_response(wishlist)))
}

/// `GET /api/v1/wishlists`
#[utoipa::path(
    get,
    path = "/api/v1/wishlists",
    tag = "wishlists",
    params(WishlistFilterParams),
    responses(
        (status = 200, description = "List of wishlists", body = WishlistListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_wishlists(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<WishlistFilterParams>,
) -> Result<Json<WishlistListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    let customer_id = params
        .customer_id
        .map(|s| s.parse::<stateset_primitives::CustomerId>())
        .transpose()
        .map_err(|e| HttpError::BadRequest(format!("Invalid customer_id: {e}")))?;

    // Count total matching records (without pagination)
    let count_filter =
        stateset_core::WishlistFilter { customer_id, is_public: None, limit: None, offset: None };
    let total = commerce.wishlists().list(count_filter)?.len();

    // Fetch the requested page
    let filter = stateset_core::WishlistFilter {
        customer_id,
        is_public: None,
        limit: Some(limit.saturating_add(1)),
        offset: Some(offset),
    };
    let mut wishlists = commerce.wishlists().list(filter)?;
    let has_more = wishlists.len() > limit as usize;
    if has_more {
        wishlists.truncate(limit as usize);
    }

    Ok(Json(WishlistListResponse {
        wishlists: wishlists.into_iter().map(wishlist_to_response).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `DELETE /api/v1/wishlists/{id}`
#[utoipa::path(
    delete,
    path = "/api/v1/wishlists/{id}",
    tag = "wishlists",
    params(("id" = String, Path, description = "Wishlist ID (UUID)")),
    responses(
        (status = 204, description = "Wishlist deleted"),
        (status = 404, description = "Wishlist not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn delete_wishlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::WishlistId>,
) -> Result<StatusCode, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    commerce.wishlists().delete(id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/wishlists/{id}/items`
#[utoipa::path(
    post,
    path = "/api/v1/wishlists/{id}/items",
    tag = "wishlists",
    params(("id" = String, Path, description = "Wishlist ID (UUID)")),
    request_body = AddWishlistItemRequest,
    responses(
        (status = 200, description = "Item added to wishlist", body = WishlistResponse),
        (status = 404, description = "Wishlist not found", body = ErrorBody),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn add_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::WishlistId>,
    Json(req): Json<AddWishlistItemRequest>,
) -> Result<Json<WishlistResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let product_id: stateset_primitives::ProductId = req
        .product_id
        .parse()
        .map_err(|e| HttpError::BadRequest(format!("Invalid product_id: {e}")))?;

    let _item = commerce.wishlists().add_item(
        id,
        stateset_core::AddWishlistItem {
            product_id,
            variant_id: None,
            note: req.note,
            quantity: None,
            priority: None,
        },
    )?;
    let wishlist = commerce
        .wishlists()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Wishlist {id} not found")))?;
    Ok(Json(wishlist_to_response(wishlist)))
}

/// `DELETE /api/v1/wishlists/{id}/items/{product_id}`
#[utoipa::path(
    delete,
    path = "/api/v1/wishlists/{id}/items/{product_id}",
    tag = "wishlists",
    params(
        ("id" = String, Path, description = "Wishlist ID (UUID)"),
        ("product_id" = String, Path, description = "Product ID (UUID) to remove"),
    ),
    responses(
        (status = 200, description = "Item removed from wishlist", body = WishlistResponse),
        (status = 404, description = "Wishlist or item not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn remove_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, product_id)): Path<(stateset_primitives::WishlistId, stateset_primitives::ProductId)>,
) -> Result<Json<WishlistResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    commerce.wishlists().remove_item(id, product_id)?;
    let wishlist = commerce
        .wishlists()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Wishlist {id} not found")))?;
    Ok(Json(wishlist_to_response(wishlist)))
}

// ============================================================================
// Helpers
// ============================================================================

fn wishlist_to_response(w: stateset_core::Wishlist) -> WishlistResponse {
    WishlistResponse {
        id: w.id.to_string(),
        customer_id: w.customer_id.to_string(),
        name: w.name,
        is_public: w.is_public,
        items: w
            .items
            .into_iter()
            .map(|i| WishlistItemResponse {
                product_id: i.product_id.to_string(),
                note: i.note,
                added_at: i.added_at,
            })
            .collect(),
        created_at: w.created_at,
        updated_at: w.updated_at,
    }
}

#[cfg(test)]
mod tests {}
