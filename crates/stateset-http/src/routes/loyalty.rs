//! Loyalty program endpoints.

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

/// Request body for `POST /api/v1/loyalty/programs`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateLoyaltyProgramRequest {
    /// Program name.
    pub name: String,
    /// Points earned per dollar spent.
    pub points_per_dollar: Option<u32>,
    /// Human-readable description.
    pub description: Option<String>,
}

/// Request body for `POST /api/v1/loyalty/enroll`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct EnrollCustomerRequest {
    /// Loyalty program to enroll in.
    #[schema(value_type = String, format = "uuid")]
    pub program_id: String,
    /// Customer to enroll.
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: String,
}

/// Query parameters for `GET /api/v1/loyalty/programs`.
#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
pub(crate) struct LoyaltyProgramFilterParams {
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u32>,
    /// Number of results to skip (default: 0).
    pub offset: Option<u32>,
}

impl LoyaltyProgramFilterParams {
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

/// Response body for a single loyalty program.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LoyaltyProgramResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: String,
    pub name: String,
    pub description: String,
    pub points_per_dollar: u32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response body for `GET /api/v1/loyalty/programs` (list).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct LoyaltyProgramListResponse {
    pub programs: Vec<LoyaltyProgramResponse>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

/// Response body for a single loyalty account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct LoyaltyAccountResponse {
    #[schema(value_type = String, format = "uuid")]
    pub id: String,
    #[schema(value_type = String, format = "uuid")]
    pub program_id: String,
    #[schema(value_type = String, format = "uuid")]
    pub customer_id: String,
    pub points_balance: i64,
    pub tier: String,
    pub lifetime_points: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Router
// ============================================================================

/// Build the loyalty sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/loyalty/programs",
            post(create_program).get(list_programs),
        )
        .route("/loyalty/enroll", post(enroll_customer))
        .route("/loyalty/accounts/{id}", get(get_account))
}

// ============================================================================
// Handlers
// ============================================================================

/// `POST /api/v1/loyalty/programs`
#[utoipa::path(
    post,
    path = "/api/v1/loyalty/programs",
    tag = "loyalty",
    request_body = CreateLoyaltyProgramRequest,
    responses(
        (status = 201, description = "Loyalty program created", body = LoyaltyProgramResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_program(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateLoyaltyProgramRequest>,
) -> Result<(StatusCode, Json<LoyaltyProgramResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let program =
        commerce
            .loyalty()
            .create_program(stateset_core::CreateLoyaltyProgram {
                name: req.name,
                description: req.description,
                points_per_dollar: req.points_per_dollar.unwrap_or(1),
                tiers: vec![],
            })?;

    Ok((StatusCode::CREATED, Json(program_to_response(program))))
}

/// `GET /api/v1/loyalty/programs`
#[utoipa::path(
    get,
    path = "/api/v1/loyalty/programs",
    tag = "loyalty",
    params(LoyaltyProgramFilterParams),
    responses(
        (status = 200, description = "List of loyalty programs", body = LoyaltyProgramListResponse),
        (status = 400, description = "Invalid filter parameter", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, params))]
pub(crate) async fn list_programs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<LoyaltyProgramFilterParams>,
) -> Result<Json<LoyaltyProgramListResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let limit = params.resolved_limit();
    let offset = params.resolved_offset();

    // Fetch all programs (no filter struct exists) and paginate in-memory
    let all_programs = commerce.loyalty().list_programs()?;
    let total = all_programs.len();

    let mut programs: Vec<_> = all_programs
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize + 1)
        .collect();
    let has_more = programs.len() > limit as usize;
    if has_more {
        programs.truncate(limit as usize);
    }

    Ok(Json(LoyaltyProgramListResponse {
        programs: programs.into_iter().map(program_to_response).collect(),
        total,
        limit,
        offset,
        has_more,
    }))
}

/// `POST /api/v1/loyalty/enroll`
#[utoipa::path(
    post,
    path = "/api/v1/loyalty/enroll",
    tag = "loyalty",
    request_body = EnrollCustomerRequest,
    responses(
        (status = 201, description = "Customer enrolled", body = LoyaltyAccountResponse),
        (status = 400, description = "Invalid request", body = ErrorBody),
        (status = 404, description = "Program or customer not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn enroll_customer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<EnrollCustomerRequest>,
) -> Result<(StatusCode, Json<LoyaltyAccountResponse>), HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    let program_id: stateset_primitives::LoyaltyProgramId = req
        .program_id
        .parse()
        .map_err(|e| HttpError::BadRequest(format!("Invalid program_id: {e}")))?;
    let customer_id: stateset_primitives::CustomerId = req
        .customer_id
        .parse()
        .map_err(|e| HttpError::BadRequest(format!("Invalid customer_id: {e}")))?;

    let account = commerce.loyalty().enroll(stateset_core::EnrollCustomer {
        customer_id,
        program_id,
    })?;
    Ok((StatusCode::CREATED, Json(account_to_response(account))))
}

/// `GET /api/v1/loyalty/accounts/{id}`
#[utoipa::path(
    get,
    path = "/api/v1/loyalty/accounts/{id}",
    tag = "loyalty",
    params(("id" = String, Path, description = "Loyalty account ID (UUID)")),
    responses(
        (status = 200, description = "Loyalty account details", body = LoyaltyAccountResponse),
        (status = 404, description = "Loyalty account not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<stateset_primitives::LoyaltyAccountId>,
) -> Result<Json<LoyaltyAccountResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let account = commerce
        .loyalty()
        .get_account(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Loyalty account {id} not found")))?;
    Ok(Json(account_to_response(account)))
}

// ============================================================================
// Helpers
// ============================================================================

fn program_to_response(p: stateset_core::LoyaltyProgram) -> LoyaltyProgramResponse {
    let is_active = p.is_active();
    LoyaltyProgramResponse {
        id: p.id.to_string(),
        name: p.name,
        description: p.description.unwrap_or_default(),
        points_per_dollar: p.points_per_dollar,
        is_active,
        created_at: p.created_at,
        updated_at: p.updated_at,
    }
}

fn account_to_response(a: stateset_core::LoyaltyAccount) -> LoyaltyAccountResponse {
    LoyaltyAccountResponse {
        id: a.id.to_string(),
        program_id: a.program_id.to_string(),
        customer_id: a.customer_id.to_string(),
        points_balance: a.points_balance,
        tier: a.tier,
        lifetime_points: a.lifetime_points,
        created_at: a.created_at,
        updated_at: a.updated_at,
    }
}

#[cfg(test)]
mod tests {}
