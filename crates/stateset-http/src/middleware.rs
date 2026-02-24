//! HTTP middleware — request ID, logging, CORS.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderName, Request, header::AUTHORIZATION},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
};
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::error::HttpError;

/// Header name for request IDs.
pub(crate) static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Debug)]
struct BearerAuthToken(Arc<str>);

impl BearerAuthToken {
    fn new(token: String) -> Self {
        Self(Arc::<str>::from(token))
    }
}

fn bearer_token_from_header(value: &str) -> Option<&str> {
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?;
    let token = parts.next()?.trim();
    if scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() {
        Some(token)
    } else {
        None
    }
}

async fn require_bearer_auth(
    State(token): State<BearerAuthToken>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/v1") {
        return next.run(request).await;
    }

    let provided = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bearer_token_from_header);

    match provided {
        Some(provided) if provided == token.0.as_ref() => next.run(request).await,
        _ => HttpError::Unauthorized("missing or invalid bearer token".to_string()).into_response(),
    }
}

/// Build the standard CORS middleware for development.
///
/// Allows any origin, method, and header.
pub(crate) fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

/// Build the request-ID middleware layers.
///
/// - Assigns a `x-request-id` UUID if the incoming request lacks one.
/// - Propagates the `x-request-id` into the response.
pub(crate) fn request_id_layers() -> (
    SetRequestIdLayer<MakeRequestUuid>,
    PropagateRequestIdLayer,
) {
    (
        SetRequestIdLayer::new(X_REQUEST_ID.clone(), MakeRequestUuid),
        PropagateRequestIdLayer::new(X_REQUEST_ID.clone()),
    )
}

/// Apply all standard middleware to a router.
pub(crate) fn apply_middleware(
    router: Router,
    with_cors: bool,
    with_request_id: bool,
    auth_token: Option<String>,
) -> Router {
    let mut router = router.layer(TraceLayer::new_for_http());

    if let Some(token) = auth_token {
        router = router.layer(from_fn_with_state(
            BearerAuthToken::new(token),
            require_bearer_auth,
        ));
    }

    if with_cors {
        router = router.layer(cors_layer());
    }

    if with_request_id {
        let (set_id, propagate_id) = request_id_layers();
        router = router.layer(propagate_id).layer(set_id);
    }

    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    #[test]
    fn x_request_id_header_name() {
        assert_eq!(X_REQUEST_ID.as_str(), "x-request-id");
    }

    #[test]
    fn cors_layer_builds() {
        let _layer = cors_layer();
    }

    #[test]
    fn request_id_layers_build() {
        let (_set, _propagate) = request_id_layers();
    }

    #[test]
    fn apply_middleware_no_extras() {
        let router = Router::new();
        let _router = apply_middleware(router, false, false, None);
    }

    #[test]
    fn apply_middleware_all() {
        let router = Router::new();
        let _router = apply_middleware(router, true, true, Some("token".to_string()));
    }

    #[test]
    fn apply_middleware_cors_only() {
        let router = Router::new();
        let _router = apply_middleware(router, true, false, None);
    }

    #[test]
    fn apply_middleware_request_id_only() {
        let router = Router::new();
        let _router = apply_middleware(router, false, true, None);
    }

    #[tokio::test]
    async fn auth_blocks_unauthorized_api_requests() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some("secret".to_string()));

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_allows_authorized_api_requests() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some("secret".to_string()));

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_skips_non_api_routes() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some("secret".to_string()));

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
