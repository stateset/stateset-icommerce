//! Vendor return endpoints (return-to-supplier).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_core::{ProductId, VendorReturnId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateVendorReturnItemRequest {
    pub product_id: String,
    /// Decimal quantity as a string.
    pub quantity: String,
    /// Decimal unit cost as a string.
    pub unit_cost: String,
    /// One of `defective`, `overage`, `wrong_item`, `other`.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateVendorReturnRequest {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub items: Vec<CreateVendorReturnItemRequest>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ProcessVendorReturnRequest {
    /// Whether to generate a vendor credit on processing.
    #[serde(default)]
    pub generate_credit: bool,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct VendorReturnFilterParams {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct VendorReturnResponse {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub status: String,
    pub total_credit: String,
    pub credit_generated: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct VendorReturnListResponse {
    pub vendor_returns: Vec<VendorReturnResponse>,
    pub total: usize,
}

fn to_resp(r: &stateset_core::VendorReturn) -> VendorReturnResponse {
    VendorReturnResponse {
        id: r.id.to_string(),
        number: r.number.clone(),
        supplier_id: r.supplier_id.to_string(),
        status: r.status.to_string(),
        total_credit: r.total_credit().to_string(),
        credit_generated: r.credit_generated,
        created_at: r.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/vendor-returns", post(create).get(list))
        .route("/vendor-returns/{id}", get(get_one))
        .route("/vendor-returns/{id}/submit", post(submit))
        .route("/vendor-returns/{id}/process", post(process))
        .route("/vendor-returns/{id}/cancel", post(cancel))
}

#[utoipa::path(post, path = "/api/v1/vendor-returns", tag = "vendor_returns",
    request_body = CreateVendorReturnRequest,
    responses((status = 201, body = VendorReturnResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateVendorReturnRequest>,
) -> Result<(StatusCode, Json<VendorReturnResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let purchase_order_id = match req.purchase_order_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "purchase_order_id")?),
        None => None,
    };
    let mut items = Vec::with_capacity(req.items.len());
    for i in req.items {
        let reason = match i.reason.as_deref() {
            Some(s) => parse_id(s, "reason")?,
            None => stateset_core::VendorReturnReason::default(),
        };
        items.push(stateset_core::CreateVendorReturnItem {
            product_id: parse_id::<ProductId>(&i.product_id, "product_id")?,
            quantity: parse_decimal(&i.quantity, "quantity")?,
            unit_cost: parse_decimal(&i.unit_cost, "unit_cost")?,
            reason,
        });
    }
    let input = stateset_core::CreateVendorReturn {
        supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
        purchase_order_id,
        currency: None,
        items,
        notes: req.notes,
    };
    let r = c.vendor_returns().create(input)?;
    Ok((StatusCode::CREATED, Json(to_resp(&r))))
}

#[utoipa::path(get, path = "/api/v1/vendor-returns", tag = "vendor_returns",
    params(VendorReturnFilterParams),
    responses((status = 200, body = VendorReturnListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<VendorReturnFilterParams>,
) -> Result<Json<VendorReturnListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let supplier_id = match params.supplier_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "supplier_id")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let total = c
        .vendor_returns()
        .list(stateset_core::VendorReturnFilter { supplier_id, status, ..Default::default() })?
        .len();
    let filter = stateset_core::VendorReturnFilter {
        supplier_id,
        status,
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
    };
    let returns = c.vendor_returns().list(filter)?;
    Ok(Json(VendorReturnListResponse {
        vendor_returns: returns.iter().map(to_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/vendor-returns/{id}", tag = "vendor_returns",
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let r = c
        .vendor_returns()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Vendor return {id} not found")))?;
    Ok(Json(to_resp(&r)))
}

#[utoipa::path(post, path = "/api/v1/vendor-returns/{id}/submit", tag = "vendor_returns",
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.vendor_returns().submit(id)?)))
}

#[utoipa::path(post, path = "/api/v1/vendor-returns/{id}/process", tag = "vendor_returns",
    request_body = ProcessVendorReturnRequest,
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn process(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
    Json(req): Json<ProcessVendorReturnRequest>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.vendor_returns().process(id, req.generate_credit)?)))
}

#[utoipa::path(post, path = "/api/v1/vendor-returns/{id}/cancel", tag = "vendor_returns",
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_resp(&c.vendor_returns().cancel(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_submit_process_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let app = router().with_state(state);
        let body = serde_json::json!({
            "supplier_id": uuid::Uuid::new_v4().to_string(),
            "items": [{
                "product_id": uuid::Uuid::new_v4().to_string(),
                "quantity": "3",
                "unit_cost": "10",
                "reason": "defective"
            }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::post("/vendor-returns")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total_credit"], "30");
        let id = json["id"].as_str().unwrap().to_string();

        let resp = app
            .oneshot(
                Request::post(format!("/vendor-returns/{id}/process"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"generate_credit": true}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "processed");
        assert_eq!(json["credit_generated"], true);
    }
}
