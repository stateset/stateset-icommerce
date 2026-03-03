//! Inventory endpoints.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};

use crate::dto::{InventoryAdjustRequest, InventoryResponse};
use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

/// Build the inventory sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/inventory/{sku}", get(get_stock))
        .route("/inventory/{sku}/adjust", post(adjust_stock))
}

/// `GET /api/v1/inventory/:sku`
#[utoipa::path(
    get,
    path = "/api/v1/inventory/{sku}",
    tag = "inventory",
    params(("sku" = String, Path, description = "Product SKU")),
    responses(
        (status = 200, description = "Stock levels", body = InventoryResponse),
        (status = 404, description = "SKU not found", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_stock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
) -> Result<Json<InventoryResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;
    let stock = commerce
        .inventory()
        .get_stock(&sku)?
        .ok_or_else(|| HttpError::NotFound(format!("Inventory item {sku} not found")))?;
    Ok(Json(InventoryResponse::from(stock)))
}

/// `POST /api/v1/inventory/:sku/adjust`
#[utoipa::path(
    post,
    path = "/api/v1/inventory/{sku}/adjust",
    tag = "inventory",
    params(("sku" = String, Path, description = "Product SKU")),
    request_body = InventoryAdjustRequest,
    responses(
        (status = 200, description = "Stock adjusted", body = InventoryResponse),
        (status = 404, description = "SKU not found", body = ErrorBody),
        (status = 422, description = "Validation error", body = ErrorBody),
    )
)]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn adjust_stock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sku): Path<String>,
    Json(req): Json<InventoryAdjustRequest>,
) -> Result<Json<InventoryResponse>, HttpError> {
    if req.location_id.is_some() {
        return Err(HttpError::ValidationError(
            "location_id is not supported by /inventory/:sku/adjust; omit location_id".to_string(),
        ));
    }

    let tenant_id = tenant_id_from_headers(&headers);
    let commerce = state.commerce_for_tenant(tenant_id.as_deref())?;

    // Perform the adjustment
    commerce.inventory().adjust(&sku, req.quantity, &req.reason)?;

    // Fetch updated stock levels
    let stock = commerce
        .inventory()
        .get_stock(&sku)?
        .ok_or_else(|| HttpError::NotFound(format!("Inventory item {sku} not found")))?;
    Ok(Json(InventoryResponse::from(stock)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use rust_decimal_macros::dec;
    use stateset_core::CreateInventoryItem;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app_with_state() -> (Router, AppState) {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let router = router().with_state(state.clone());
        (router, state)
    }

    fn app() -> Router {
        let (router, _) = app_with_state();
        router
    }

    #[tokio::test]
    async fn get_stock_not_found() {
        let resp = app()
            .oneshot(Request::get("/inventory/NONEXISTENT").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_and_get_stock() {
        let (app, state) = app_with_state();

        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "WIDGET-001".into(),
                name: "Widget".into(),
                initial_quantity: Some(dec!(100)),
                ..Default::default()
            })
            .unwrap();

        let resp = app
            .oneshot(Request::get("/inventory/WIDGET-001").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["sku"], "WIDGET-001");
    }

    #[tokio::test]
    async fn adjust_stock_works() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));

        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "ADJ-001".into(),
                name: "Adjustable Widget".into(),
                initial_quantity: Some(dec!(50)),
                ..Default::default()
            })
            .unwrap();

        let app = router().with_state(state);

        let body = serde_json::json!({
            "quantity": "-10",
            "reason": "Damaged stock removal"
        });
        let resp = app
            .oneshot(
                Request::post("/inventory/ADJ-001/adjust")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json["sku"], "ADJ-001");
    }

    #[tokio::test]
    async fn adjust_stock_rejects_location_id() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        state
            .commerce()
            .inventory()
            .create_item(CreateInventoryItem {
                sku: "LOC-001".into(),
                name: "Location Item".into(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .unwrap();

        let app = router().with_state(state);

        let body = serde_json::json!({
            "quantity": "-1",
            "reason": "manual",
            "location_id": 42
        });
        let resp = app
            .oneshot(
                Request::post("/inventory/LOC-001/adjust")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert_eq!(json["error"]["code"], "validation_error");
    }
}
