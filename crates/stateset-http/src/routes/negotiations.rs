//! Negotiation endpoints for autonomous agent-to-agent price negotiation.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_a2a::negotiation::NegotiationEngine;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

/// Store key: negotiations are scoped per tenant (`None` = default tenant) so
/// one tenant can never read or mutate another tenant's negotiations.
type StoreKey = (Option<String>, Uuid);

/// In-memory negotiation store.
///
/// The V9 tables `a2a_negotiations` and `a2a_negotiation_offers` exist in
/// `stateset-migrations`, but no repository exposes them yet —
/// [`stateset_core::traits::A2ACommerceRepository`] only covers quotes and
/// purchases. Wiring DB persistence requires new negotiation repository
/// methods on both the SQLite and Postgres backends; until then this store is
/// process-local and negotiations do not survive a restart.
type NegotiationStore = Arc<RwLock<HashMap<StoreKey, stateset_a2a::negotiation::Negotiation>>>;

/// Build the negotiations sub-router.
pub fn router() -> Router<AppState> {
    let store: NegotiationStore = Arc::new(RwLock::new(HashMap::new()));
    Router::new()
        .route(
            "/negotiations",
            post({
                let store = store.clone();
                move |state, headers, body| create_negotiation(state, headers, body, store)
            }),
        )
        .route(
            "/negotiations/{id}",
            get({
                let store = store.clone();
                move |state, headers, path| get_negotiation(state, headers, path, store)
            }),
        )
        .route(
            "/negotiations/{id}/counter-offer",
            post({
                let store = store.clone();
                move |state, headers, path, body| counter_offer(state, headers, path, body, store)
            }),
        )
        .route(
            "/negotiations/{id}/accept",
            post({
                let store = store.clone();
                move |state, headers, path| accept_negotiation(state, headers, path, store)
            }),
        )
        .route(
            "/negotiations/{id}/reject",
            post(move |state, headers, path| reject_negotiation(state, headers, path, store)),
        )
}

/// Request body for creating a negotiation.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateNegotiationRequest {
    pub buyer_agent_id: String,
    pub seller_agent_id: String,
    #[schema(value_type = String)]
    pub initial_offer: Decimal,
    pub currency: Option<String>,
    pub max_rounds: Option<u32>,
    #[schema(value_type = Option<String>)]
    pub auto_accept_below: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub auto_reject_above: Option<Decimal>,
    pub expires_in_hours: Option<u32>,
}

/// Request body for a counter-offer.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CounterOfferRequest {
    pub from_agent_id: String,
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub message: Option<String>,
}

/// Response body for a negotiation.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NegotiationResponse {
    pub id: String,
    pub buyer_agent_id: String,
    pub seller_agent_id: String,
    pub status: String,
    #[schema(value_type = String)]
    pub current_offer: Decimal,
    pub currency: String,
    pub rounds: u32,
    pub max_rounds: u32,
    pub offer_count: usize,
    pub created_at: String,
}

fn neg_to_response(n: &stateset_a2a::negotiation::Negotiation) -> NegotiationResponse {
    NegotiationResponse {
        id: n.id.to_string(),
        buyer_agent_id: n.buyer_agent_id.clone(),
        seller_agent_id: n.seller_agent_id.clone(),
        status: n.status.to_string(),
        current_offer: n.current_offer,
        currency: n.currency.clone(),
        rounds: n.rounds,
        max_rounds: n.max_rounds,
        offer_count: n.offers.len(),
        created_at: n.created_at.to_rfc3339(),
    }
}

/// `POST /api/v1/negotiations`
#[utoipa::path(post, path = "/api/v1/negotiations", tag = "negotiations",
    request_body = CreateNegotiationRequest,
    responses((status = 201, body = NegotiationResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn create_negotiation(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateNegotiationRequest>,
    store: NegotiationStore,
) -> Result<(StatusCode, Json<NegotiationResponse>), HttpError> {
    if req.initial_offer <= Decimal::ZERO {
        return Err(HttpError::BadRequest("Invalid amount: initial_offer must be positive".into()));
    }
    let tenant_id = tenant_id_from_headers(&headers);
    let expires = Utc::now() + Duration::hours(i64::from(req.expires_in_hours.unwrap_or(24)));

    let neg = NegotiationEngine::create(
        req.buyer_agent_id,
        req.seller_agent_id,
        req.initial_offer,
        req.currency.as_deref().unwrap_or("USD"),
        req.max_rounds.unwrap_or(10),
        expires,
        req.auto_accept_below,
        req.auto_reject_above,
    );

    let resp = neg_to_response(&neg);
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    guard.insert((tenant_id, neg.id), neg);

    Ok((StatusCode::CREATED, Json(resp)))
}

/// `GET /api/v1/negotiations/:id`
#[utoipa::path(get, path = "/api/v1/negotiations/{id}", tag = "negotiations",
    params(("id" = String, Path, description = "Negotiation ID (UUID)")),
    responses((status = 200, body = NegotiationResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn get_negotiation(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let guard = store.read().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let neg = guard
        .get(&(tenant_id, id))
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;
    Ok(Json(neg_to_response(neg)))
}

/// `POST /api/v1/negotiations/:id/counter-offer`
#[utoipa::path(post, path = "/api/v1/negotiations/{id}/counter-offer", tag = "negotiations",
    params(("id" = String, Path, description = "Negotiation ID (UUID)")),
    request_body = CounterOfferRequest,
    responses((status = 200, body = NegotiationResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn counter_offer(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<CounterOfferRequest>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    if req.amount <= Decimal::ZERO {
        return Err(HttpError::BadRequest("Invalid amount: amount must be positive".into()));
    }
    let tenant_id = tenant_id_from_headers(&headers);

    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let key = (tenant_id, id);
    let neg = guard
        .remove(&key)
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;

    let neg = NegotiationEngine::counter_offer(neg, req.from_agent_id, req.amount, req.message)
        .map_err(|e| HttpError::BadRequest(format!("Counter-offer failed: {e}")))?;

    let resp = neg_to_response(&neg);
    guard.insert(key, neg);
    Ok(Json(resp))
}

/// `POST /api/v1/negotiations/:id/accept`
#[utoipa::path(post, path = "/api/v1/negotiations/{id}/accept", tag = "negotiations",
    params(("id" = String, Path, description = "Negotiation ID (UUID)")),
    responses((status = 200, body = NegotiationResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn accept_negotiation(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let key = (tenant_id, id);
    let neg = guard
        .remove(&key)
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;

    let neg = NegotiationEngine::accept(neg)
        .map_err(|e| HttpError::BadRequest(format!("Accept failed: {e}")))?;

    let resp = neg_to_response(&neg);
    guard.insert(key, neg);
    Ok(Json(resp))
}

/// `POST /api/v1/negotiations/:id/reject`
#[utoipa::path(post, path = "/api/v1/negotiations/{id}/reject", tag = "negotiations",
    params(("id" = String, Path, description = "Negotiation ID (UUID)")),
    responses((status = 200, body = NegotiationResponse), (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip_all)]
pub(crate) async fn reject_negotiation(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    store: NegotiationStore,
) -> Result<Json<NegotiationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let mut guard = store.write().map_err(|_| HttpError::InternalError("Lock poisoned".into()))?;
    let key = (tenant_id, id);
    let neg = guard
        .remove(&key)
        .ok_or_else(|| HttpError::NotFound(format!("Negotiation {id} not found")))?;

    let neg = NegotiationEngine::reject(neg, None)
        .map_err(|e| HttpError::BadRequest(format!("Reject failed: {e}")))?;

    let resp = neg_to_response(&neg);
    guard.insert(key, neg);
    Ok(Json(resp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn post_json(uri: &str, tenant: Option<&str>, body: &serde_json::Value) -> Request<Body> {
        let mut builder = Request::post(uri).header("content-type", "application/json");
        if let Some(tenant) = tenant {
            builder = builder.header("x-tenant-id", tenant);
        }
        builder.body(Body::from(serde_json::to_vec(body).unwrap())).unwrap()
    }

    fn get_request(uri: &str, tenant: Option<&str>) -> Request<Body> {
        let mut builder = Request::get(uri);
        if let Some(tenant) = tenant {
            builder = builder.header("x-tenant-id", tenant);
        }
        builder.body(Body::empty()).unwrap()
    }

    async fn create_negotiation(app: &Router, tenant: Option<&str>, offer: &str) -> String {
        let body = serde_json::json!({
            "buyer_agent_id": "agent-buyer",
            "seller_agent_id": "agent-seller",
            "initial_offer": offer,
        });
        let resp = app.clone().oneshot(post_json("/negotiations", tenant, &body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = json_body(resp).await;
        json["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn create_negotiation_serializes_offer_as_decimal_string() {
        let app = app();
        let body = serde_json::json!({
            "buyer_agent_id": "agent-buyer",
            "seller_agent_id": "agent-seller",
            "initial_offer": "100.50",
        });

        let resp = app.oneshot(post_json("/negotiations", None, &body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = json_body(resp).await;
        assert_eq!(json["current_offer"], "100.50");
        assert_eq!(json["currency"], "USD");
        assert_eq!(json["status"], "open");
        assert_eq!(json["rounds"], 1);
    }

    #[tokio::test]
    async fn create_negotiation_accepts_numeric_amounts() {
        let app = app();
        let body = serde_json::json!({
            "buyer_agent_id": "agent-buyer",
            "seller_agent_id": "agent-seller",
            "initial_offer": 49.99,
        });

        let resp = app.oneshot(post_json("/negotiations", None, &body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = json_body(resp).await;
        assert_eq!(json["current_offer"], "49.99");
    }

    #[tokio::test]
    async fn create_negotiation_preserves_high_precision_amounts() {
        // A value f64 cannot represent exactly must round-trip unchanged.
        let app = app();
        let body = serde_json::json!({
            "buyer_agent_id": "agent-buyer",
            "seller_agent_id": "agent-seller",
            "initial_offer": "1234567890.123456789",
        });

        let resp = app.oneshot(post_json("/negotiations", None, &body)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = json_body(resp).await;
        assert_eq!(json["current_offer"], "1234567890.123456789");
    }

    #[tokio::test]
    async fn create_negotiation_rejects_non_positive_offer() {
        let app = app();
        for offer in ["0", "-5.00"] {
            let body = serde_json::json!({
                "buyer_agent_id": "agent-buyer",
                "seller_agent_id": "agent-seller",
                "initial_offer": offer,
            });
            let resp = app.clone().oneshot(post_json("/negotiations", None, &body)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn counter_offer_uses_decimal_amount() {
        let app = app();
        let id = create_negotiation(&app, None, "100.00").await;

        let body = serde_json::json!({
            "from_agent_id": "agent-seller",
            "amount": "95.25",
        });
        let resp = app
            .oneshot(post_json(&format!("/negotiations/{id}/counter-offer"), None, &body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = json_body(resp).await;
        assert_eq!(json["current_offer"], "95.25");
        assert_eq!(json["status"], "counter_offered");
        assert_eq!(json["rounds"], 2);
        assert_eq!(json["offer_count"], 2);
    }

    #[tokio::test]
    async fn counter_offer_rejects_non_positive_amount() {
        let app = app();
        let id = create_negotiation(&app, None, "100.00").await;

        let body = serde_json::json!({
            "from_agent_id": "agent-seller",
            "amount": "-1",
        });
        let resp = app
            .oneshot(post_json(&format!("/negotiations/{id}/counter-offer"), None, &body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn accept_negotiation_returns_accepted_status() {
        let app = app();
        let id = create_negotiation(&app, None, "100.00").await;

        let body = serde_json::json!({});
        let resp = app
            .oneshot(post_json(&format!("/negotiations/{id}/accept"), None, &body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = json_body(resp).await;
        assert_eq!(json["status"], "accepted");
    }

    #[tokio::test]
    async fn reject_negotiation_returns_rejected_status() {
        let app = app();
        let id = create_negotiation(&app, None, "100.00").await;

        let body = serde_json::json!({});
        let resp = app
            .oneshot(post_json(&format!("/negotiations/{id}/reject"), None, &body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = json_body(resp).await;
        assert_eq!(json["status"], "rejected");
    }

    #[tokio::test]
    async fn get_negotiation_unknown_id_returns_404() {
        let app = app();
        let resp = app
            .oneshot(get_request(&format!("/negotiations/{}", Uuid::new_v4()), None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn tenant_cannot_read_other_tenants_negotiation() {
        let app = app();
        let id = create_negotiation(&app, Some("tenant-a"), "100.00").await;

        // Tenant B must not see tenant A's negotiation.
        let resp = app
            .clone()
            .oneshot(get_request(&format!("/negotiations/{id}"), Some("tenant-b")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // Tenant A still sees it.
        let resp = app
            .oneshot(get_request(&format!("/negotiations/{id}"), Some("tenant-a")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn tenant_cannot_mutate_other_tenants_negotiation() {
        let app = app();
        let id = create_negotiation(&app, Some("tenant-a"), "100.00").await;

        let counter = serde_json::json!({
            "from_agent_id": "agent-seller",
            "amount": "95.00",
        });
        for uri in [
            format!("/negotiations/{id}/counter-offer"),
            format!("/negotiations/{id}/accept"),
            format!("/negotiations/{id}/reject"),
        ] {
            let resp =
                app.clone().oneshot(post_json(&uri, Some("tenant-b"), &counter)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} must 404 for tenant-b");
        }

        // The negotiation is untouched for tenant A.
        let resp = app
            .oneshot(get_request(&format!("/negotiations/{id}"), Some("tenant-a")))
            .await
            .unwrap();
        let json = json_body(resp).await;
        assert_eq!(json["status"], "open");
        assert_eq!(json["rounds"], 1);
    }

    #[tokio::test]
    async fn default_tenant_is_isolated_from_named_tenants() {
        let app = app();
        let id = create_negotiation(&app, None, "100.00").await;

        let resp = app
            .clone()
            .oneshot(get_request(&format!("/negotiations/{id}"), Some("tenant-a")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = app.oneshot(get_request(&format!("/negotiations/{id}"), None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
