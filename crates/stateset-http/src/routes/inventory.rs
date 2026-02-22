//! Inventory endpoints.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};

use crate::dto::{InventoryAdjustRequest, InventoryResponse};
use crate::error::HttpError;
use crate::state::AppState;

/// Build the inventory sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/inventory/{sku}", get(get_stock))
        .route("/inventory/{sku}/adjust", post(adjust_stock))
}

/// `GET /api/v1/inventory/:sku`
async fn get_stock(
    State(state): State<AppState>,
    Path(sku): Path<String>,
) -> Result<Json<InventoryResponse>, HttpError> {
    let stock = state
        .commerce()
        .inventory()
        .get_stock(&sku)?
        .ok_or_else(|| HttpError::NotFound(format!("Inventory item {sku} not found")))?;
    Ok(Json(InventoryResponse::from(stock)))
}

/// `POST /api/v1/inventory/:sku/adjust`
async fn adjust_stock(
    State(state): State<AppState>,
    Path(sku): Path<String>,
    Json(req): Json<InventoryAdjustRequest>,
) -> Result<Json<InventoryResponse>, HttpError> {
    // Perform the adjustment
    state
        .commerce()
        .inventory()
        .adjust(&sku, req.quantity, &req.reason)?;

    // Fetch updated stock levels
    let stock = state
        .commerce()
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
            .oneshot(
                Request::get("/inventory/NONEXISTENT")
                    .body(Body::empty())
                    .unwrap(),
            )
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
            .oneshot(
                Request::get("/inventory/WIDGET-001")
                    .body(Body::empty())
                    .unwrap(),
            )
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
}
