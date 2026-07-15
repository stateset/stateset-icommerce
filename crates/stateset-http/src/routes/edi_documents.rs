//! EDI document endpoints (trading-partner document tracking + reporting).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::EdiDocumentId;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateEdiDocumentRequest {
    pub document_type: String,
    /// `inbound` or `outbound`.
    pub direction: Option<String>,
    pub partner: Option<String>,
    pub reference: Option<String>,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct SetStatusRequest {
    /// `pending`, `sent`, `acknowledged`, `processed`, `error`.
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct EdiDocumentFilterParams {
    pub document_type: Option<String>,
    pub direction: Option<String>,
    pub status: Option<String>,
    pub partner: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct EdiDocumentResponse {
    pub id: String,
    pub document_type: String,
    pub direction: String,
    pub status: String,
    pub partner: Option<String>,
    pub reference: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct EdiDocumentListResponse {
    pub documents: Vec<EdiDocumentResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct EdiCountResponse {
    pub key: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct EdiSummaryResponse {
    pub total: u64,
    pub by_status: Vec<EdiCountResponse>,
    pub by_type: Vec<EdiCountResponse>,
}

fn to_resp(d: &stateset_core::EdiDocument) -> EdiDocumentResponse {
    EdiDocumentResponse {
        id: d.id.to_string(),
        document_type: d.document_type.clone(),
        direction: d.direction.to_string(),
        status: d.status.to_string(),
        partner: d.partner.clone(),
        reference: d.reference.clone(),
        error_message: d.error_message.clone(),
        created_at: d.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/edi-documents", post(create).get(list))
        .route("/edi-documents/summary", get(summary))
        .route("/edi-documents/{id}", get(get_one))
        .route("/edi-documents/{id}/status", post(set_status))
}

#[utoipa::path(post, operation_id = "edi_documents_create", path = "/api/v1/edi-documents", tag = "edi_documents",
    request_body = CreateEdiDocumentRequest,
    responses((status = 201, body = EdiDocumentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEdiDocumentRequest>,
) -> Result<(StatusCode, Json<EdiDocumentResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let direction = match req.direction.as_deref() {
        Some(s) => parse_id(s, "direction")?,
        None => stateset_core::EdiDirection::default(),
    };
    let input = stateset_core::CreateEdiDocument {
        document_type: req.document_type,
        direction,
        partner: req.partner,
        reference: req.reference,
        payload: req.payload,
    };
    let d = c.edi_documents().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&d))))
}

#[utoipa::path(get, operation_id = "edi_documents_list", path = "/api/v1/edi-documents", tag = "edi_documents",
    params(EdiDocumentFilterParams),
    responses((status = 200, body = EdiDocumentListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<EdiDocumentFilterParams>,
) -> Result<Json<EdiDocumentListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let direction = match params.direction.as_deref() {
        Some(s) => Some(parse_id(s, "direction")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::EdiDocumentFilter {
        document_type: params.document_type.clone(),
        direction,
        status,
        partner: params.partner.clone(),
        ..Default::default()
    };
    let total = c.edi_documents().list(base.clone())?.len();
    let filter = stateset_core::EdiDocumentFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let documents = c.edi_documents().list(filter)?;
    Ok(Json(EdiDocumentListResponse { documents: documents.iter().map(to_resp).collect(), total }))
}

#[utoipa::path(get, operation_id = "edi_documents_summary", path = "/api/v1/edi-documents/summary", tag = "edi_documents",
    responses((status = 200, body = EdiSummaryResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EdiSummaryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let s = c.edi_documents().summary()?;
    let map = |v: Vec<stateset_core::EdiCount>| -> Vec<EdiCountResponse> {
        v.into_iter().map(|c| EdiCountResponse { key: c.key, count: c.count }).collect()
    };
    Ok(Json(EdiSummaryResponse {
        total: s.total,
        by_status: map(s.by_status),
        by_type: map(s.by_type),
    }))
}

#[utoipa::path(get, operation_id = "edi_documents_get_one", path = "/api/v1/edi-documents/{id}", tag = "edi_documents",
    params(("id" = String, Path, description = "EDI document ID")),
    responses((status = 200, body = EdiDocumentResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<EdiDocumentId>,
) -> Result<Json<EdiDocumentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let d = c
        .edi_documents()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("EDI document {id} not found")))?;
    Ok(Json(to_resp(&d)))
}

#[utoipa::path(post, operation_id = "edi_documents_set_status", path = "/api/v1/edi-documents/{id}/status", tag = "edi_documents",
    request_body = SetStatusRequest,
    params(("id" = String, Path, description = "EDI document ID")),
    responses((status = 200, body = EdiDocumentResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn set_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<EdiDocumentId>,
    Json(req): Json<SetStatusRequest>,
) -> Result<Json<EdiDocumentResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = parse_id(&req.status, "status")?;
    Ok(Json(to_resp(&c.edi_documents().set_status(id, status, req.error_message)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_and_summary() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "document_type": "850",
            "direction": "inbound",
            "partner": "ACME-EDI"
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/edi-documents")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = app
            .oneshot(Request::get("/edi-documents/summary").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"], 1);
    }
}
