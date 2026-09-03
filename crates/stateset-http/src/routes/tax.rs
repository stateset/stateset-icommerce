//! Tax exemption endpoints.
//!
//! Tax calculation honours only **verified** exemptions, and exemptions are
//! created unverified. Without a verify endpoint an exemption could be created
//! over the API and never take effect, so the whole lifecycle — create, read,
//! list by customer, verify/revoke — lives here.

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateTaxExemptionRequest {
    pub customer_id: Uuid,
    /// `resale`, `non_profit`, `government`, `diplomatic`, `export`, `other`.
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    /// Jurisdictions the exemption is scoped to (empty/absent = all).
    pub jurisdiction_ids: Option<Vec<Uuid>>,
    /// Product categories the exemption covers (empty/absent = all).
    pub exempt_categories: Option<Vec<String>>,
    pub effective_from: Option<NaiveDate>,
    pub expires_at: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct VerifyTaxExemptionRequest {
    /// `true` verifies the certificate, `false` revokes the verification.
    /// Absent means verify.
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TaxExemptionFilterParams {
    /// Customer whose exemptions to list (required).
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TaxExemptionResponse {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub jurisdiction_ids: Vec<Uuid>,
    pub exempt_categories: Vec<String>,
    pub effective_from: String,
    pub expires_at: Option<String>,
    /// Only verified exemptions are honoured by tax calculation.
    pub verified: bool,
    pub verified_at: Option<String>,
    pub active: bool,
    /// Whether this exemption is in force today — active, verified and inside
    /// its validity window. This is the test the tax engine applies.
    pub in_force: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TaxExemptionListResponse {
    pub exemptions: Vec<TaxExemptionResponse>,
    pub total: usize,
}

fn exemption_to_resp(e: &stateset_core::TaxExemption) -> TaxExemptionResponse {
    TaxExemptionResponse {
        id: e.id,
        customer_id: e.customer_id,
        exemption_type: e.exemption_type.to_string(),
        certificate_number: e.certificate_number.clone(),
        issuing_authority: e.issuing_authority.clone(),
        jurisdiction_ids: e.jurisdiction_ids.clone(),
        exempt_categories: e.exempt_categories.iter().map(ToString::to_string).collect(),
        effective_from: e.effective_from.to_string(),
        expires_at: e.expires_at.map(|d| d.to_string()),
        verified: e.verified,
        verified_at: e.verified_at.map(|d| d.to_rfc3339()),
        active: e.active,
        in_force: e.is_effective_on(chrono::Utc::now().date_naive()),
        notes: e.notes.clone(),
        created_at: e.created_at.to_rfc3339(),
        updated_at: e.updated_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tax/exemptions", post(create_exemption).get(list_exemptions))
        .route("/tax/exemptions/{id}", get(get_exemption))
        .route("/tax/exemptions/{id}/verify", post(verify_exemption))
}

#[utoipa::path(post, path = "/api/v1/tax/exemptions", tag = "tax",
    request_body = CreateTaxExemptionRequest,
    responses((status = 201, body = TaxExemptionResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_exemption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTaxExemptionRequest>,
) -> Result<(StatusCode, Json<TaxExemptionResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;

    let exemption_type: stateset_core::ExemptionType =
        req.exemption_type.parse().map_err(|_| {
            HttpError::BadRequest(format!("Unknown exemption type: {}", req.exemption_type))
        })?;
    let exempt_categories = req
        .exempt_categories
        .unwrap_or_default()
        .iter()
        .map(|value| {
            value.parse::<stateset_core::ProductTaxCategory>().map_err(|_| {
                HttpError::BadRequest(format!("Unknown product tax category: {value}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let input = stateset_core::CreateTaxExemption {
        customer_id: req.customer_id,
        exemption_type,
        certificate_number: req.certificate_number,
        issuing_authority: req.issuing_authority,
        jurisdiction_ids: req.jurisdiction_ids.unwrap_or_default(),
        exempt_categories,
        effective_from: req.effective_from.unwrap_or_else(|| chrono::Utc::now().date_naive()),
        expires_at: req.expires_at,
        notes: req.notes,
    };
    let exemption = c.tax().create_exemption(input)?;
    Ok((StatusCode::CREATED, Json(exemption_to_resp(&exemption))))
}

#[utoipa::path(get, path = "/api/v1/tax/exemptions", tag = "tax",
    params(TaxExemptionFilterParams),
    responses((status = 200, body = TaxExemptionListResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_exemptions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<TaxExemptionFilterParams>,
) -> Result<Json<TaxExemptionListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let customer_id = params
        .customer_id
        .ok_or_else(|| HttpError::BadRequest("customer_id is required".into()))?;
    let exemptions = c.tax().get_customer_exemptions(customer_id)?;
    let total = exemptions.len();
    Ok(Json(TaxExemptionListResponse {
        exemptions: exemptions.iter().map(exemption_to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/tax/exemptions/{id}", tag = "tax",
    params(("id" = String, Path, description = "Tax exemption ID")),
    responses((status = 200, body = TaxExemptionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_exemption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<TaxExemptionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let exemption = c
        .tax()
        .get_exemption(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Tax exemption {id} not found")))?;
    Ok(Json(exemption_to_resp(&exemption)))
}

#[utoipa::path(post, path = "/api/v1/tax/exemptions/{id}/verify", tag = "tax",
    params(("id" = String, Path, description = "Tax exemption ID")),
    request_body = VerifyTaxExemptionRequest,
    responses((status = 200, body = TaxExemptionResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn verify_exemption(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<VerifyTaxExemptionRequest>,
) -> Result<Json<TaxExemptionResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let exemption = c.tax().verify_exemption(id, req.verified.unwrap_or(true))?;
    Ok(Json(exemption_to_resp(&exemption)))
}
