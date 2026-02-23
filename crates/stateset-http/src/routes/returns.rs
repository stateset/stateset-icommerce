//! Return endpoints.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch, post},
};

use crate::dto::{CreateReturnRequest, ReturnResponse};
use crate::error::HttpError;
use crate::state::AppState;
use stateset_core::{CreateReturn, CreateReturnItem, ItemCondition, OrderItemId, ReturnId, ReturnReason};
use std::str::FromStr;

/// Build the returns sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/returns", post(create_return))
        .route("/returns/{id}", get(get_return))
        .route("/returns/{id}/approve", patch(approve_return))
}

/// `POST /api/v1/returns`
async fn create_return(
    State(state): State<AppState>,
    Json(req): Json<CreateReturnRequest>,
) -> Result<(axum::http::StatusCode, Json<ReturnResponse>), HttpError> {
    let reason = ReturnReason::from_str(&req.reason)
        .map_err(|e| HttpError::BadRequest(format!("Invalid reason: {e}")))?;

    let items: Vec<CreateReturnItem> = req
        .items
        .into_iter()
        .map(|item| {
            let condition = item
                .condition
                .as_deref()
                .map(ItemCondition::from_str)
                .transpose()
                .map_err(|e| HttpError::BadRequest(format!("Invalid condition: {e}")));
            condition.map(|c| CreateReturnItem {
                order_item_id: OrderItemId::from_uuid(item.order_item_id),
                quantity: item.quantity,
                condition: c,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let input = CreateReturn {
        order_id: req.order_id,
        reason,
        reason_details: req.reason_details,
        items,
        notes: req.notes,
        ..Default::default()
    };
    let ret = state.commerce().returns().create(input)?;
    Ok((axum::http::StatusCode::CREATED, Json(ReturnResponse::from(ret))))
}

/// `GET /api/v1/returns/:id`
async fn get_return(
    State(state): State<AppState>,
    Path(id): Path<ReturnId>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let ret = state
        .commerce()
        .returns()
        .get(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Return {id} not found")))?;
    Ok(Json(ReturnResponse::from(ret)))
}

/// `PATCH /api/v1/returns/:id/approve`
async fn approve_return(
    State(state): State<AppState>,
    Path(id): Path<ReturnId>,
) -> Result<Json<ReturnResponse>, HttpError> {
    let ret = state.commerce().returns().approve(id)?;
    Ok(Json(ReturnResponse::from(ret)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(
            Commerce::new(":memory:").expect("in-memory Commerce"),
        ))
    }

    #[tokio::test]
    async fn get_return_not_found() {
        let id = ReturnId::new();
        let resp = app()
            .oneshot(
                Request::get(format!("/returns/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn approve_nonexistent_return() {
        let id = ReturnId::new();
        let resp = app()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/returns/{id}/approve"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(resp.status().is_client_error());
    }

    #[tokio::test]
    async fn create_return_invalid_reason() {
        let body = serde_json::json!({
            "order_id": uuid::Uuid::new_v4(),
            "reason": "unicorn_dust",
            "items": []
        });
        let resp = app()
            .oneshot(
                Request::post("/returns")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
