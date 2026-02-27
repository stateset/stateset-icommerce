//! Health-check endpoints.

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};

use crate::dto::{HealthResponse, ReadyResponse};
use crate::state::AppState;

/// Build the health-check router.
pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health)).route("/health/ready", get(readiness))
}

/// `GET /health` — simple liveness probe.
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// `GET /health/ready` — readiness probe that checks DB connectivity.
async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadyResponse>) {
    // Try a lightweight operation to verify DB is reachable.
    let database_connected = state.commerce().orders().count(Default::default()).is_ok();
    let (status, body) = readiness_response(database_connected);
    (status, Json(body))
}

fn readiness_response(database_connected: bool) -> (StatusCode, ReadyResponse) {
    if database_connected {
        (StatusCode::OK, ReadyResponse { status: "ok", database: "connected" })
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            ReadyResponse { status: "not_ready", database: "disconnected" },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let resp =
            app().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn readiness_returns_connected() {
        let resp = app()
            .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["database"], "connected");
    }

    #[test]
    fn readiness_response_reports_not_ready_when_disconnected() {
        let (status, body) = readiness_response(false);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.database, "disconnected");
    }
}
