//! Vendor return endpoints (return-to-supplier).

use crate::error::{ErrorBody, HttpError};
use crate::parsing::{parse_decimal, parse_id};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use stateset_core::{ProductId, VendorReturnId};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateVendorReturnItemRequest {
    pub product_id: String,
    pub sku: String,
    pub source_location_id: i32,
    pub lot_id: Option<String>,
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
    pub warehouse_id: i32,
    pub currency: Option<String>,
    pub purchase_order_id: Option<String>,
    pub items: Vec<CreateVendorReturnItemRequest>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct ProcessVendorReturnRequest {
    /// Required for processing; false is refused.
    #[serde(default = "credit_required")]
    pub generate_credit: bool,
}

const fn credit_required() -> bool {
    true
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
    pub warehouse_id: Option<i32>,
    pub currency: String,
    pub items: Vec<VendorReturnItemResponse>,
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
        warehouse_id: r.warehouse_id,
        currency: r.currency.to_string(),
        items: r
            .items
            .iter()
            .map(|item| VendorReturnItemResponse {
                id: item.id.to_string(),
                product_id: item.product_id.to_string(),
                sku: item.sku.clone(),
                source_location_id: item.source_location_id,
                lot_id: item.lot_id,
                quantity: item.quantity.to_string(),
                unit_cost: item.unit_cost.to_string(),
                reason: item.reason.to_string(),
            })
            .collect(),
        number: r.number.clone(),
        supplier_id: r.supplier_id.to_string(),
        status: r.status.to_string(),
        total_credit: r.total_credit().to_string(),
        credit_generated: r.credit_generated,
        created_at: r.created_at.to_rfc3339(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/vendor-returns", post(create).get(list))
        .route("/vendor-returns/{id}", get(get_one))
        .route("/vendor-returns/{id}/submit", post(submit))
        .route("/vendor-returns/{id}/process", post(process))
        .route("/vendor-returns/{id}/cancel", post(cancel))
}

#[utoipa::path(post, operation_id = "vendor_returns_create", path = "/api/v1/vendor-returns", tag = "vendor_returns",
    request_body = CreateVendorReturnRequest,
    responses((status = 201, body = VendorReturnResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateVendorReturnRequest>,
) -> Result<(StatusCode, Json<VendorReturnResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
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
                    sku: i.sku,
                    source_location_id: i.source_location_id,
                    lot_id: i
                        .lot_id
                        .as_deref()
                        .map(|id| parse_id::<Uuid>(id, "lot_id"))
                        .transpose()?,
                    quantity: parse_decimal(&i.quantity, "quantity")?,
                    unit_cost: parse_decimal(&i.unit_cost, "unit_cost")?,
                    reason,
                });
            }
            let input = stateset_core::CreateVendorReturn {
                supplier_id: parse_id::<Uuid>(&req.supplier_id, "supplier_id")?,
                warehouse_id: req.warehouse_id,
                purchase_order_id,
                currency: req
                    .currency
                    .as_deref()
                    .map(|value| parse_id(value, "currency"))
                    .transpose()?,
                items,
                notes: req.notes,
            };
            let r = c.vendor_returns().create(input)?;
            Ok((StatusCode::CREATED, Json(to_resp(&r))))
        })
        .await
}

#[utoipa::path(get, operation_id = "vendor_returns_list", path = "/api/v1/vendor-returns", tag = "vendor_returns",
    params(VendorReturnFilterParams),
    responses((status = 200, body = VendorReturnListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<VendorReturnFilterParams>,
) -> Result<Json<VendorReturnListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
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
                .list(stateset_core::VendorReturnFilter {
                    supplier_id,
                    status,
                    ..Default::default()
                })?
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
        })
        .await
}

#[utoipa::path(get, operation_id = "vendor_returns_get_one", path = "/api/v1/vendor-returns/{id}", tag = "vendor_returns",
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| {
            let r = c
                .vendor_returns()
                .get(id)?
                .ok_or_else(|| HttpError::NotFound(format!("Vendor return {id} not found")))?;
            Ok(Json(to_resp(&r)))
        })
        .await
}

#[utoipa::path(post, operation_id = "vendor_returns_submit", path = "/api/v1/vendor-returns/{id}/submit", tag = "vendor_returns",
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| Ok(Json(to_resp(&c.vendor_returns().submit(id)?))))
        .await
}

#[utoipa::path(post, operation_id = "vendor_returns_process", path = "/api/v1/vendor-returns/{id}/process", tag = "vendor_returns",
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
    state
        .run_blocking(tid.as_deref(), move |c| {
            Ok(Json(to_resp(&c.vendor_returns().process(id, req.generate_credit)?)))
        })
        .await
}

#[utoipa::path(post, operation_id = "vendor_returns_cancel", path = "/api/v1/vendor-returns/{id}/cancel", tag = "vendor_returns",
    params(("id" = String, Path, description = "Vendor return ID")),
    responses((status = 200, body = VendorReturnResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<VendorReturnId>,
) -> Result<Json<VendorReturnResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    state
        .run_blocking(tid.as_deref(), move |c| Ok(Json(to_resp(&c.vendor_returns().cancel(id)?))))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use rust_decimal::Decimal;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_submit_process_flow() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let c = state.commerce_for_tenant(None).unwrap();
        let supplier = c
            .purchase_orders()
            .create_supplier(stateset_core::CreateSupplier {
                name: "Returns".into(),
                ..Default::default()
            })
            .unwrap();
        let product = c
            .products()
            .create(stateset_core::CreateProduct {
                name: "Returned item".into(),
                variants: Some(vec![stateset_core::CreateProductVariant {
                    sku: "RETURNED".into(),
                    price: Decimal::TEN,
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();
        let warehouse = c
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: "VR".into(),
                name: "Returns".into(),
                ..Default::default()
            })
            .unwrap();
        let location = c
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id: warehouse.id,
                ..Default::default()
            })
            .unwrap();
        c.warehouse()
            .adjust_inventory(stateset_core::AdjustLocationInventory {
                location_id: location.id,
                sku: "RETURNED".into(),
                lot_id: None,
                quantity: Decimal::TEN,
                reason: "receipt".into(),
                reference_type: None,
                reference_id: None,
                performed_by: None,
            })
            .unwrap();
        let app = router().with_state(state);
        let body = serde_json::json!({
            "supplier_id": supplier.id.to_string(),
            "warehouse_id": warehouse.id,
            "items": [{
                "product_id": product.id.to_string(),
                "sku": "RETURNED",
                "source_location_id": location.id,
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
        assert_eq!(
            json["total_credit"].as_str().unwrap().parse::<Decimal>().unwrap(),
            Decimal::from(30)
        );
        let id = json["id"].as_str().unwrap().to_string();

        let submitted = app
            .clone()
            .oneshot(
                Request::post(format!("/vendor-returns/{id}/submit")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(submitted.status(), StatusCode::OK);
        let resp = app
            .oneshot(
                Request::post(format!("/vendor-returns/{id}/process"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "processed");
        assert_eq!(json["credit_generated"], true);
        assert_eq!(
            c.inventory().get_stock("RETURNED").unwrap().unwrap().total_on_hand,
            Decimal::from(7)
        );
        let credits = c
            .vendor_credits()
            .list(stateset_core::VendorCreditFilter {
                supplier_id: Some(supplier.id),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(credits.len(), 1);
        assert_eq!(credits[0].amount, Decimal::from(30));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct VendorReturnItemResponse {
    pub id: String,
    pub product_id: String,
    pub sku: String,
    pub source_location_id: Option<i32>,
    pub lot_id: Option<Uuid>,
    pub quantity: String,
    pub unit_cost: String,
    pub reason: String,
}
