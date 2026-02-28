//! HTTP middleware — request ID, logging, CORS.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{
        HeaderName, HeaderValue, Method, Request,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::error::HttpError;

/// Header name for request IDs.
pub(crate) static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub(crate) static X_TENANT_ID: HeaderName = HeaderName::from_static("x-tenant-id");
const CORS_ORIGINS_ENV: &str = "STATESET_HTTP_ALLOWED_ORIGINS";
const DEFAULT_CORS_ORIGINS: [&str; 2] = ["http://localhost:3000", "http://127.0.0.1:3000"];

#[derive(Clone, Debug)]
struct BearerAuthConfig {
    token: Arc<str>,
    bound_tenant_id: Option<Arc<str>>,
}

impl BearerAuthConfig {
    fn new(token: String, bound_tenant_id: Option<String>) -> Self {
        Self {
            token: Arc::<str>::from(token),
            bound_tenant_id: bound_tenant_id.map(Arc::<str>::from),
        }
    }
}

fn bearer_token_from_header(value: &str) -> Option<&str> {
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?;
    let token = parts.next()?.trim();
    if scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() { Some(token) } else { None }
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }

    let mut diff = 0u8;
    for (&left, &right) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn is_valid_tenant_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return false;
    }
    trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

async fn require_bearer_auth(
    State(auth): State<BearerAuthConfig>,
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
        Some(provided) if constant_time_eq(provided, auth.token.as_ref()) => {}
        _ => {
            return HttpError::Unauthorized("missing or invalid bearer token".to_string())
                .into_response();
        }
    }

    let tenant_id = request
        .headers()
        .get(&X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match tenant_id {
        Some(value) if is_valid_tenant_id(value) => {
            if let Some(bound_tenant_id) = auth.bound_tenant_id.as_deref() {
                if !constant_time_eq(value, bound_tenant_id) {
                    return HttpError::Forbidden(
                        "tenant is not authorized for supplied bearer token".to_string(),
                    )
                    .into_response();
                }
            }
            next.run(request).await
        }
        Some(_) => HttpError::BadRequest("invalid x-tenant-id header".to_string()).into_response(),
        None => HttpError::BadRequest("missing x-tenant-id header".to_string()).into_response(),
    }
}

/// Build the standard CORS middleware for development.
///
/// Allows a configurable set of origins with explicit methods and headers.
pub(crate) fn cors_layer() -> CorsLayer {
    let configured = std::env::var(CORS_ORIGINS_ENV).ok();
    let allowed_origins = configured
        .as_deref()
        .map(|value| value.split(',').map(str::trim).filter(|origin| !origin.is_empty()))
        .into_iter()
        .flatten()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect::<Vec<_>>();
    let origins = if allowed_origins.is_empty() {
        DEFAULT_CORS_ORIGINS
            .iter()
            .filter_map(|origin| HeaderValue::from_str(origin).ok())
            .collect::<Vec<_>>()
    } else {
        allowed_origins
    };

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, X_TENANT_ID.clone()])
}

/// Build the request-ID middleware layers.
///
/// - Assigns a `x-request-id` UUID if the incoming request lacks one.
/// - Propagates the `x-request-id` into the response.
pub(crate) fn request_id_layers() -> (SetRequestIdLayer<MakeRequestUuid>, PropagateRequestIdLayer) {
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
    auth_config: Option<(String, Option<String>)>,
) -> Router {
    let mut router = router.layer(TraceLayer::new_for_http());

    if let Some((token, bound_tenant_id)) = auth_config {
        router = router.layer(from_fn_with_state(
            BearerAuthConfig::new(token, bound_tenant_id),
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
    use axum::http::{Method, Request, StatusCode};
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
        let _router = apply_middleware(router, true, true, Some(("token".to_string(), None)));
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

    #[test]
    fn constant_time_eq_behaves_like_string_equality() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secreu"));
        assert!(!constant_time_eq("secret", "secret1"));
        assert!(!constant_time_eq("", "a"));
    }

    #[test]
    fn tenant_id_validation() {
        assert!(is_valid_tenant_id("tenant-1"));
        assert!(is_valid_tenant_id("tenant.alpha_01"));
        assert!(!is_valid_tenant_id(""));
        assert!(!is_valid_tenant_id("  "));
        assert!(!is_valid_tenant_id("../etc/passwd"));
        assert!(!is_valid_tenant_id("tenant/one"));
    }

    #[tokio::test]
    async fn auth_blocks_unauthorized_api_requests() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some(("secret".to_string(), None)));

        let response =
            app.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_allows_authorized_api_requests() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some(("secret".to_string(), None)));

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_blocks_missing_tenant_header() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some(("secret".to_string(), None)));

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn auth_blocks_tenant_mismatch_when_token_is_bound() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(
            router,
            false,
            false,
            Some(("secret".to_string(), Some("tenant-a".to_string()))),
        );

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .header("x-tenant-id", "tenant-b")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn auth_skips_non_api_routes() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let app = apply_middleware(router, false, false, Some(("secret".to_string(), None)));

        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cors_allows_localhost_origin_by_default() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let app = apply_middleware(router, true, false, None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/health")
                    .header("origin", "http://localhost:3000")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|value| value.to_str().ok()),
            Some("http://localhost:3000")
        );
    }

    #[tokio::test]
    async fn cors_rejects_unconfigured_origin_by_default() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let app = apply_middleware(router, true, false, None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/health")
                    .header("origin", "https://evil.example")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("access-control-allow-origin").is_none());
    }
}
