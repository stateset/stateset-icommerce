//! Additional ERP workflow endpoints. Quantities and money use decimal strings.
use crate::{
    error::HttpError,
    middleware::FinanceActor,
    state::{AppState, tenant_id_from_headers},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stateset_core::*;
use std::result::Result;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
fn parse<T: std::str::FromStr>(value: &str, field: &str) -> Result<T, HttpError> {
    value.parse().map_err(|_| HttpError::BadRequest(format!("invalid {field}")))
}
fn optional<T: std::str::FromStr>(
    value: Option<String>,
    field: &str,
) -> Result<Option<T>, HttpError> {
    value.map(|v| parse(&v, field)).transpose()
}
fn json(value: impl Serialize) -> Result<Json<Value>, HttpError> {
    serde_json::to_value(value).map(Json).map_err(|e| HttpError::InternalError(e.to_string()))
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct ReceiptFromPoRequest {
    pub purchase_order_id: String,
    pub warehouse_id: i32,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct AssignPutAwayRequest {
    pub assigned_to: String,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct AllocateBackorderRequest {
    pub quantity: String,
    pub location_id: Option<i32>,
    pub lot_id: Option<String>,
    pub expires_at: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct LotTransferRequest {
    pub quantity: String,
    pub from_location_id: i32,
    pub to_location_id: i32,
    pub reason: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct LotAdjustmentRequest {
    pub quantity_change: String,
    pub reason: String,
    pub location_id: Option<i32>,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct LotSplitRequest {
    pub quantity: String,
    pub new_lot_number: Option<String>,
    pub reason: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct WorkOrderMaterialRequest {
    pub component_id: Option<String>,
    pub component_sku: String,
    pub component_name: String,
    pub quantity: String,
}
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct ConsumeMaterialRequest {
    pub quantity: String,
}
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in=Query)]
pub(crate) struct AccountBalanceQuery {
    pub as_of_date: Option<String>,
}
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/receipts/from-po", post(receipt_from_po))
        .route("/put-aways/{id}/assign", post(assign_put_away))
        .route("/put-aways/{id}/start", post(start_put_away))
        .route("/put-aways/{id}/cancel", post(cancel_put_away))
        .route("/backorders/{id}/allocations", post(allocate_backorder).get(backorder_allocations))
        .route("/backorders/allocations/{id}/release", post(release_allocation))
        .route("/backorders/allocations/{id}/confirm", post(confirm_allocation))
        .route("/lots/{id}/transfer", post(transfer_lot))
        .route("/lots/{id}/adjust", post(adjust_lot))
        .route("/lots/{id}/split", post(split_lot))
        .route("/work-orders/{id}/materials", post(add_material).get(list_materials))
        .route("/work-orders/materials/{id}/consume", post(consume_material))
        .route("/gl/accounts/{id}/balance", get(account_balance))
        .route("/gl/periods/{id}/reclose", post(reclose_period))
}
#[utoipa::path(post,operation_id="erp_receipt_from_po",path="/api/v1/receipts/from-po",tag="erp_operations",request_body=ReceiptFromPoRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn receipt_from_po(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReceiptFromPoRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.receiving().create_receipt_from_po(
                parse(&req.purchase_order_id, "purchase_order_id")?,
                req.warehouse_id,
            )?)
        })
        .await
}

#[utoipa::path(post,operation_id="erp_assign_put_away",path="/api/v1/put-aways/{id}/assign",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=AssignPutAwayRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn assign_put_away(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignPutAwayRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.receiving().assign_put_away(id, &req.assigned_to)?)
        })
        .await
}

#[utoipa::path(post,operation_id="erp_start_put_away",path="/api/v1/put-aways/{id}/start",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn start_put_away(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state.run_blocking(tenant.as_deref(), move |c| json(c.receiving().start_put_away(id)?)).await
}

#[utoipa::path(post,operation_id="erp_cancel_put_away",path="/api/v1/put-aways/{id}/cancel",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn cancel_put_away(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state.run_blocking(tenant.as_deref(), move |c| json(c.receiving().cancel_put_away(id)?)).await
}

#[utoipa::path(post,operation_id="erp_allocate_backorder",path="/api/v1/backorders/{id}/allocations",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=AllocateBackorderRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn allocate_backorder(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<AllocateBackorderRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.backorder().allocate_backorder(AllocateBackorder {
                backorder_id: id,
                quantity: parse(&req.quantity, "quantity")?,
                location_id: req.location_id,
                lot_id: optional(req.lot_id, "lot_id")?,
                expires_at: optional(req.expires_at, "expires_at")?,
            })?)
        })
        .await
}

#[utoipa::path(get,operation_id="erp_backorder_allocations",path="/api/v1/backorders/{id}/allocations",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Vec<Object>),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn backorder_allocations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state.run_blocking(tenant.as_deref(), move |c| json(c.backorder().get_allocations(id)?)).await
}

#[utoipa::path(post,operation_id="erp_release_allocation",path="/api/v1/backorders/allocations/{id}/release",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn release_allocation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| json(c.backorder().release_allocation(id)?))
        .await
}

#[utoipa::path(post,operation_id="erp_confirm_allocation",path="/api/v1/backorders/allocations/{id}/confirm",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn confirm_allocation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| json(c.backorder().confirm_allocation(id)?))
        .await
}

#[utoipa::path(post,operation_id="erp_transfer_lot",path="/api/v1/lots/{id}/transfer",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=LotTransferRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn transfer_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    actor: FinanceActor,
    Json(req): Json<LotTransferRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.lots().transfer(TransferLot {
                lot_id: id,
                quantity: parse(&req.quantity, "quantity")?,
                from_location_id: req.from_location_id,
                to_location_id: req.to_location_id,
                reason: req.reason,
                performed_by: Some(actor.0),
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="erp_adjust_lot",path="/api/v1/lots/{id}/adjust",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=LotAdjustmentRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn adjust_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    actor: FinanceActor,
    Json(req): Json<LotAdjustmentRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.lots().adjust(AdjustLot {
                lot_id: id,
                quantity_change: parse(&req.quantity_change, "quantity_change")?,
                reason: req.reason,
                location_id: req.location_id,
                reference_type: req.reference_type,
                reference_id: optional(req.reference_id, "reference_id")?,
                performed_by: Some(actor.0),
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="erp_split_lot",path="/api/v1/lots/{id}/split",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=LotSplitRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn split_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<LotSplitRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.lots().split(SplitLot {
                lot_id: id,
                quantity: parse(&req.quantity, "quantity")?,
                new_lot_number: req.new_lot_number,
                reason: req.reason,
            })?)
        })
        .await
}

#[utoipa::path(post,operation_id="erp_add_material",path="/api/v1/work-orders/{id}/materials",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=WorkOrderMaterialRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn add_material(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<WorkOrderMaterialRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.work_orders().add_material(
                id,
                AddWorkOrderMaterial {
                    component_id: optional(req.component_id, "component_id")?,
                    component_sku: req.component_sku,
                    component_name: req.component_name,
                    quantity: parse(&req.quantity, "quantity")?,
                },
            )?)
        })
        .await
}

#[utoipa::path(get,operation_id="erp_list_materials",path="/api/v1/work-orders/{id}/materials",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Vec<Object>),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn list_materials(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state.run_blocking(tenant.as_deref(), move |c| json(c.work_orders().get_materials(id)?)).await
}

#[utoipa::path(post,operation_id="erp_consume_material",path="/api/v1/work-orders/materials/{id}/consume",tag="erp_operations",params(("id"=String,Path,description="Record ID")), request_body=ConsumeMaterialRequest, responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn consume_material(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ConsumeMaterialRequest>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.work_orders().consume_material(id, parse(&req.quantity, "quantity")?)?)
        })
        .await
}

#[utoipa::path(get,operation_id="erp_account_balance",path="/api/v1/gl/accounts/{id}/balance",tag="erp_operations",params(AccountBalanceQuery, ("id"=String,Path,description="Account ID")), responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn account_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(req): Query<AccountBalanceQuery>,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            let balance = c
                .general_ledger()
                .get_account_balance(id, optional(req.as_of_date, "as_of_date")?)?
                .ok_or_else(|| HttpError::NotFound("Account not found".into()))?;
            json(serde_json::json!({"account_id":id,"balance":balance.to_string()}))
        })
        .await
}

#[utoipa::path(post,operation_id="erp_reclose_period",path="/api/v1/gl/periods/{id}/reclose",tag="erp_operations",params(("id"=String,Path,description="Record ID")), responses((status=200,body=Object),(status=400,body=crate::error::ErrorBody),(status=409,body=crate::error::ErrorBody)))]
pub(crate) async fn reclose_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    actor: FinanceActor,
) -> Result<Json<Value>, HttpError> {
    let tenant = tenant_id_from_headers(&headers);
    state
        .run_blocking(tenant.as_deref(), move |c| {
            json(c.general_ledger().reclose_period(id, &actor.0)?)
        })
        .await
}
