//! Agent credit terms endpoints for net 30/60/90 payment between trusted agents.
//!
//! Credit lines are durable and tenant-scoped: they live in the
//! `a2a_credit_terms` / `a2a_credit_entries` tables of the tenant's database
//! (resolved from `x-tenant-id`), so they survive restarts, are visible to
//! every replica, and every balance change is journaled and applied under a
//! write lock with a conditional UPDATE.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{
    A2ACreditEntry, A2ACreditMovement, A2ACreditPaymentTerms, A2ACreditTerms, A2ACreditTermsFilter,
    A2ACreditTermsStatus, CreateA2ACreditTerms,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

/// Tenant used when no `x-tenant-id` header is present (single-tenant
/// deployments); rows are still stamped so a later tenant split is clean.
pub(crate) const DEFAULT_TENANT: &str = "default";

pub(crate) fn tenant_scope(headers: &HeaderMap) -> String {
    tenant_id_from_headers(headers).unwrap_or_else(|| DEFAULT_TENANT.to_string())
}

/// Build the credit terms sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/a2a/credit", post(create_terms).get(list_terms))
        .route("/a2a/credit/{id}", get(get_terms))
        .route("/a2a/credit/{id}/charge", post(charge_credit))
        .route("/a2a/credit/{id}/payment", post(record_payment))
        .route("/a2a/credit/{id}/entries", get(list_entries))
}

/// Request body for creating credit terms.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateCreditTermsRequest {
    pub creditor_agent_id: String,
    pub debtor_agent_id: String,
    /// Credit limit (exact decimal; accepts a JSON string or number).
    #[schema(value_type = String)]
    pub credit_limit: Decimal,
    pub currency: Option<String>,
    /// One of `net_15`, `net_30` (default), `net_60`, `net_90`, `prepaid`.
    pub payment_terms: Option<String>,
}

/// Request body for charging or paying.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreditAmountRequest {
    /// Amount (exact decimal; accepts a JSON string or number).
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
}

/// Query params for listing credit terms.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CreditTermsFilterParams {
    pub creditor_agent_id: Option<String>,
    pub debtor_agent_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Response body for credit terms.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreditTermsResponse {
    pub id: String,
    pub tenant_id: String,
    pub creditor_agent_id: String,
    pub debtor_agent_id: String,
    pub credit_limit: String,
    pub outstanding_balance: String,
    pub available_credit: String,
    pub currency: String,
    pub payment_terms: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Response body for a credit journal entry.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreditEntryResponse {
    pub id: String,
    pub terms_id: String,
    pub entry_type: String,
    pub amount: String,
    pub balance_after: String,
    pub reference_id: Option<String>,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub created_at: String,
}

fn terms_to_response(t: &A2ACreditTerms) -> CreditTermsResponse {
    CreditTermsResponse {
        id: t.id.to_string(),
        tenant_id: t.tenant_id.clone(),
        creditor_agent_id: t.creditor_agent_id.clone(),
        debtor_agent_id: t.debtor_agent_id.clone(),
        credit_limit: t.credit_limit.to_string(),
        outstanding_balance: t.outstanding_balance.to_string(),
        available_credit: t.available_credit().to_string(),
        currency: t.currency.clone(),
        payment_terms: t.payment_terms.to_string(),
        status: t.status.to_string(),
        created_at: t.created_at.to_rfc3339(),
        updated_at: t.updated_at.to_rfc3339(),
    }
}

fn entry_to_response(e: &A2ACreditEntry) -> CreditEntryResponse {
    CreditEntryResponse {
        id: e.id.to_string(),
        terms_id: e.terms_id.to_string(),
        entry_type: e.entry_type.to_string(),
        amount: e.amount.to_string(),
        balance_after: e.balance_after.to_string(),
        reference_id: e.reference_id.clone(),
        notes: e.notes.clone(),
        due_date: e.due_date.map(|d| d.to_rfc3339()),
        created_at: e.created_at.to_rfc3339(),
    }
}

fn parse_payment_terms(value: Option<&str>) -> Result<Option<A2ACreditPaymentTerms>, HttpError> {
    match value {
        None => Ok(None),
        Some(raw) => raw.parse::<A2ACreditPaymentTerms>().map(Some).map_err(|_| {
            HttpError::BadRequest(format!(
                "Invalid payment_terms: {raw}. Valid values: net_15, net_30, net_60, net_90, prepaid"
            ))
        }),
    }
}

/// `POST /api/v1/a2a/credit`
#[utoipa::path(post, path = "/api/v1/a2a/credit", tag = "a2a",
    request_body = CreateCreditTermsRequest,
    responses((status = 201, body = CreditTermsResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn create_terms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCreditTermsRequest>,
) -> Result<(StatusCode, Json<CreditTermsResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let payment_terms = parse_payment_terms(req.payment_terms.as_deref())?;
    let terms = c.x402().create_credit_terms(CreateA2ACreditTerms {
        tenant_id: tenant_scope(&headers),
        creditor_agent_id: req.creditor_agent_id,
        debtor_agent_id: req.debtor_agent_id,
        credit_limit: req.credit_limit,
        currency: req.currency,
        payment_terms,
        min_trust_tier: None,
    })?;
    Ok((StatusCode::CREATED, Json(terms_to_response(&terms))))
}

/// `GET /api/v1/a2a/credit`
#[utoipa::path(get, path = "/api/v1/a2a/credit", tag = "a2a",
    params(CreditTermsFilterParams),
    responses((status = 200, body = Vec<CreditTermsResponse>)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn list_terms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CreditTermsFilterParams>,
) -> Result<Json<Vec<CreditTermsResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = params
        .status
        .as_deref()
        .map(|s| {
            s.parse::<A2ACreditTermsStatus>()
                .map_err(|_| HttpError::BadRequest(format!("Invalid status: {s}")))
        })
        .transpose()?;
    let terms = c.x402().list_credit_terms(A2ACreditTermsFilter {
        tenant_id: tenant_scope(&headers),
        creditor_agent_id: params.creditor_agent_id,
        debtor_agent_id: params.debtor_agent_id,
        status,
        limit: params.limit,
        offset: params.offset,
    })?;
    Ok(Json(terms.iter().map(terms_to_response).collect()))
}

/// `GET /api/v1/a2a/credit/:id`
#[utoipa::path(get, path = "/api/v1/a2a/credit/{id}", tag = "a2a",
    params(("id" = String, Path, description = "Credit terms ID (UUID)")),
    responses((status = 200, body = CreditTermsResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn get_terms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<CreditTermsResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let terms = c
        .x402()
        .get_credit_terms(&tenant_scope(&headers), id)?
        .ok_or_else(|| HttpError::NotFound(format!("Credit terms {id} not found")))?;
    Ok(Json(terms_to_response(&terms)))
}

/// `GET /api/v1/a2a/credit/:id/entries`
#[utoipa::path(get, path = "/api/v1/a2a/credit/{id}/entries", tag = "a2a",
    params(("id" = String, Path, description = "Credit terms ID (UUID)")),
    responses((status = 200, body = Vec<CreditEntryResponse>), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn list_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CreditEntryResponse>>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let tenant = tenant_scope(&headers);
    c.x402()
        .get_credit_terms(&tenant, id)?
        .ok_or_else(|| HttpError::NotFound(format!("Credit terms {id} not found")))?;
    let entries = c.x402().list_credit_terms_entries(&tenant, id)?;
    Ok(Json(entries.iter().map(entry_to_response).collect()))
}

/// `POST /api/v1/a2a/credit/:id/charge`
#[utoipa::path(post, path = "/api/v1/a2a/credit/{id}/charge", tag = "a2a",
    params(("id" = String, Path, description = "Credit terms ID (UUID)")),
    request_body = CreditAmountRequest,
    responses((status = 200, body = CreditTermsResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn charge_credit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<CreditTermsResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let (terms, _entry) = c
        .x402()
        .charge_credit_terms(A2ACreditMovement {
            tenant_id: tenant_scope(&headers),
            terms_id: id,
            amount: req.amount,
            reference_id: req.reference_id,
            notes: req.notes,
        })
        .map_err(|e| match e {
            stateset_core::CommerceError::NotFound => {
                HttpError::NotFound(format!("Credit terms {id} not found"))
            }
            stateset_core::CommerceError::NotPermitted(msg)
            | stateset_core::CommerceError::ValidationError(msg) => {
                HttpError::BadRequest(format!("Charge failed: {msg}"))
            }
            other => HttpError::from(other),
        })?;
    Ok(Json(terms_to_response(&terms)))
}

/// `POST /api/v1/a2a/credit/:id/payment`
#[utoipa::path(post, path = "/api/v1/a2a/credit/{id}/payment", tag = "a2a",
    params(("id" = String, Path, description = "Credit terms ID (UUID)")),
    request_body = CreditAmountRequest,
    responses((status = 200, body = CreditTermsResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn record_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CreditAmountRequest>,
) -> Result<Json<CreditTermsResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let (terms, _entry) = c
        .x402()
        .record_credit_terms_payment(A2ACreditMovement {
            tenant_id: tenant_scope(&headers),
            terms_id: id,
            amount: req.amount,
            reference_id: req.reference_id,
            notes: req.notes,
        })
        .map_err(|e| match e {
            stateset_core::CommerceError::NotFound => {
                HttpError::NotFound(format!("Credit terms {id} not found"))
            }
            stateset_core::CommerceError::ValidationError(msg) => {
                HttpError::BadRequest(format!("Payment failed: {msg}"))
            }
            other => HttpError::from(other),
        })?;
    Ok(Json(terms_to_response(&terms)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn credit_dtos_accept_string_and_number_amounts_exactly() {
        let from_str: CreditAmountRequest =
            serde_json::from_str(r#"{"amount":"0.10"}"#).expect("string amount");
        assert_eq!(from_str.amount, dec!(0.10));
        let from_num: CreditAmountRequest =
            serde_json::from_str(r#"{"amount":0.1}"#).expect("number amount");
        assert_eq!(from_num.amount, dec!(0.1));

        let terms: CreateCreditTermsRequest = serde_json::from_str(
            r#"{"creditor_agent_id":"a","debtor_agent_id":"b","credit_limit":"1000.005"}"#,
        )
        .expect("terms");
        assert_eq!(terms.credit_limit, dec!(1000.005));
    }

    #[test]
    fn credit_dtos_reject_non_numeric_amounts() {
        assert!(serde_json::from_str::<CreditAmountRequest>(r#"{"amount":"abc"}"#).is_err());
        assert!(serde_json::from_str::<CreditAmountRequest>(r#"{"amount":true}"#).is_err());
    }

    #[test]
    fn payment_terms_parse_all_documented_values() {
        for (raw, expected) in [
            ("net_15", A2ACreditPaymentTerms::Net15),
            ("net_30", A2ACreditPaymentTerms::Net30),
            ("net_60", A2ACreditPaymentTerms::Net60),
            ("net_90", A2ACreditPaymentTerms::Net90),
            ("prepaid", A2ACreditPaymentTerms::Prepaid),
        ] {
            assert_eq!(parse_payment_terms(Some(raw)).unwrap(), Some(expected));
        }
        assert_eq!(parse_payment_terms(None).unwrap(), None);
        assert!(parse_payment_terms(Some("net_45")).is_err());
    }
}
