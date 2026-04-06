//! HTTP middleware — request ID, logging, CORS, rate limiting, caching.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{
        HeaderName, HeaderValue, Method, Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware::{Next, from_fn, from_fn_with_state},
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use stateset_authz::{AccessDecision, Action, AuthzEngine, Resource};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::error::HttpError;

/// Header name for request IDs.
pub(crate) static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub(crate) static X_TENANT_ID: HeaderName = HeaderName::from_static("x-tenant-id");
pub(crate) static X_ACTOR_ID: HeaderName = HeaderName::from_static("x-actor-id");
const CORS_ORIGINS_ENV: &str = "STATESET_HTTP_ALLOWED_ORIGINS";
const DEFAULT_CORS_ORIGINS: [&str; 2] = ["http://localhost:3000", "http://127.0.0.1:3000"];

#[derive(Clone, Debug)]
pub(crate) struct BearerAuthBinding {
    token: Arc<str>,
    bound_tenant_id: Option<Arc<str>>,
    bound_actor_id: Option<Arc<str>>,
}

impl BearerAuthBinding {
    pub(crate) fn new(
        token: String,
        bound_tenant_id: Option<String>,
        bound_actor_id: Option<String>,
    ) -> Self {
        Self {
            token: Arc::<str>::from(token),
            bound_tenant_id: bound_tenant_id.map(Arc::<str>::from),
            bound_actor_id: bound_actor_id.map(Arc::<str>::from),
        }
    }
}

#[derive(Clone, Debug)]
struct BearerAuthConfig {
    bindings: Arc<[BearerAuthBinding]>,
}

impl BearerAuthConfig {
    fn new(bindings: Vec<BearerAuthBinding>) -> Self {
        Self { bindings: Arc::<[BearerAuthBinding]>::from(bindings) }
    }
}

#[derive(Clone, Debug)]
struct MisconfiguredApiAuthConfig {
    message: Arc<str>,
}

impl MisconfiguredApiAuthConfig {
    fn new(message: String) -> Self {
        Self { message: Arc::<str>::from(message) }
    }
}

#[derive(Clone, Debug)]
struct AuthenticatedActorIdentity;

#[derive(Clone, Debug)]
pub(crate) struct AuthzConfig {
    engine: Arc<Mutex<AuthzEngine>>,
    trust_actor_headers: bool,
}

impl AuthzConfig {
    pub(crate) fn new(engine: AuthzEngine) -> Self {
        Self { engine: Arc::new(Mutex::new(engine)), trust_actor_headers: false }
    }

    pub(crate) fn with_trusted_actor_headers(mut self) -> Self {
        self.trust_actor_headers = true;
        self
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

fn requires_bearer_auth(path: &str) -> bool {
    path.starts_with("/api/v1") || path == "/health/deep"
}

fn requires_tenant_header(path: &str) -> bool {
    path.starts_with("/api/v1")
}

fn authz_target(request: &Request<Body>) -> Option<(Resource, Action)> {
    let path = request.uri().path();
    let stripped = path.strip_prefix("/api/v1/")?;
    let segments = stripped.split('/').filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }

    let resource_type = match segments.as_slice() {
        ["openapi.json"] => "openapi".to_string(),
        [first, ..] => (*first).to_string(),
        [] => return None,
    };
    let resource = if is_list_like_route(&segments) || is_execute_route(&segments) {
        Resource::new(resource_type)
    } else if segments.len() >= 2 {
        Resource::with_id(resource_type, segments[1])
    } else {
        Resource::new(resource_type)
    };

    let terminal_segment = segments.last().copied();
    let action = match *request.method() {
        Method::GET => {
            if is_list_like_route(&segments) {
                Action::List
            } else {
                Action::Read
            }
        }
        Method::POST => {
            if let Some(segment) = terminal_segment.filter(|_| is_execute_route(&segments)) {
                action_for_route_segment(segment)
            } else {
                Action::Create
            }
        }
        Method::PATCH | Method::PUT => {
            if let Some(segment) = terminal_segment.filter(|_| is_execute_route(&segments)) {
                action_for_route_segment(segment)
            } else {
                Action::Update
            }
        }
        Method::DELETE => Action::Delete,
        _ => return None,
    };

    Some((resource, action))
}

fn is_list_like_route(segments: &[&str]) -> bool {
    matches!(
        segments,
        [_] | ["loyalty", "programs"]
            | ["currencies", "rates"]
            | ["tax", "rates"]
            | ["tax", "jurisdictions"]
            | ["events", "stream"]
    )
}

fn is_execute_route(segments: &[&str]) -> bool {
    segments.last().is_some_and(|segment| is_action_segment(segment))
}

fn action_for_route_segment(segment: &str) -> Action {
    if is_delete_like_action_segment(segment) { Action::Delete } else { Action::Execute }
}

fn is_action_segment(segment: &str) -> bool {
    matches!(
        segment,
        "cancel"
            | "ship"
            | "disable"
            | "complete"
            | "refund"
            | "calculate"
            | "deliver"
            | "enroll"
            | "send"
            | "pause"
            | "resume"
            | "activate"
            | "deactivate"
            | "adjust"
            | "convert"
            | "approve"
            | "start"
    )
}

fn is_delete_like_action_segment(segment: &str) -> bool {
    matches!(segment, "cancel" | "disable" | "refund" | "deactivate")
}

pub(crate) fn is_valid_actor_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return false;
    }
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '@' | '+'))
}

async fn require_bearer_auth(
    State(auth): State<BearerAuthConfig>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if !requires_bearer_auth(path) {
        return next.run(request).await;
    }

    let provided = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bearer_token_from_header);

    let binding = match provided {
        Some(provided) => auth
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| constant_time_eq(provided, binding.token.as_ref()))
            .map(|(index, _)| index)
            .last()
            .and_then(|index| auth.bindings.get(index)),
        _ => {
            return HttpError::Unauthorized("missing or invalid bearer token".to_string())
                .into_response();
        }
    };
    let Some(binding) = binding else {
        return HttpError::Unauthorized("missing or invalid bearer token".to_string())
            .into_response();
    };

    if !requires_tenant_header(path) {
        return next.run(request).await;
    }

    if let Some(bound_actor_id) = binding.bound_actor_id.as_deref() {
        if !is_valid_actor_id(bound_actor_id) {
            return HttpError::InternalError("configured actor binding is invalid".to_string())
                .into_response();
        }

        if let Some(provided_actor_id) = request
            .headers()
            .get(&X_ACTOR_ID)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !constant_time_eq(provided_actor_id, bound_actor_id) {
                return HttpError::Forbidden(
                    "actor is not authorized for supplied bearer token".to_string(),
                )
                .into_response();
            }
        }

        let actor_header_value = match HeaderValue::from_str(bound_actor_id) {
            Ok(value) => value,
            Err(_) => {
                return HttpError::InternalError("configured actor binding is invalid".to_string())
                    .into_response();
            }
        };
        request.headers_mut().insert(X_ACTOR_ID.clone(), actor_header_value);
        request.extensions_mut().insert(AuthenticatedActorIdentity);
    }

    let tenant_id = request
        .headers()
        .get(&X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match tenant_id {
        Some(value) if is_valid_tenant_id(value) => {
            if let Some(bound_tenant_id) = binding.bound_tenant_id.as_deref() {
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

async fn reject_misconfigured_api_auth(
    State(config): State<MisconfiguredApiAuthConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if requires_bearer_auth(request.uri().path()) {
        return HttpError::InternalError(config.message.to_string()).into_response();
    }
    next.run(request).await
}

async fn require_authorization(
    State(config): State<AuthzConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some((resource, action)) = authz_target(&request) else {
        return next.run(request).await;
    };

    let actor_identity_established =
        request.extensions().get::<AuthenticatedActorIdentity>().is_some();
    if !config.trust_actor_headers && !actor_identity_established {
        return HttpError::Unauthorized(
            "actor identity was not established by authentication".to_string(),
        )
        .into_response();
    }

    let actor_id = match request
        .headers()
        .get(&X_ACTOR_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && is_valid_actor_id(value))
    {
        Some(actor_id) => actor_id,
        None => {
            return HttpError::Unauthorized("missing or invalid x-actor-id header".to_string())
                .into_response();
        }
    };

    let decision = {
        let mut engine = config.engine.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        engine.authorize(actor_id, &resource, &action)
    };

    match decision {
        AccessDecision::Allowed => next.run(request).await,
        AccessDecision::Denied { reason } => HttpError::Forbidden(reason).into_response(),
        AccessDecision::RequiresApproval { reason } => HttpError::Forbidden(reason).into_response(),
        _ => HttpError::Forbidden("authorization denied".to_string()).into_response(),
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
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, X_TENANT_ID.clone(), X_ACTOR_ID.clone()])
}

// ============================================================================
// Rate Limiting
// ============================================================================

/// Configuration for the token bucket rate limiter.
#[derive(Clone, Debug)]
pub(crate) struct RateLimitConfig {
    /// Sustained requests per second.
    pub requests_per_second: u64,
    /// Maximum burst size (token bucket capacity).
    pub burst_size: u64,
}

/// Token bucket for rate limiting.
///
/// Tokens refill at `rate` per second up to `capacity`. Each request consumes
/// one token; if none are available the request is rejected with HTTP 429.
pub(crate) struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    rate: f64,
    capacity: f64,
}

impl TokenBucket {
    fn new(rate: u64, capacity: u64) -> Self {
        Self {
            tokens: capacity as f64,
            last_refill: Instant::now(),
            rate: rate as f64,
            capacity: capacity as f64,
        }
    }

    fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Create a shared token bucket wrapped in an `Arc<Mutex<_>>`.
pub(crate) fn create_rate_limiter(config: &RateLimitConfig) -> Arc<Mutex<TokenBucket>> {
    Arc::new(Mutex::new(TokenBucket::new(config.requests_per_second, config.burst_size)))
}

/// Middleware that enforces a global request rate limit.
///
/// Returns HTTP 429 with a `Retry-After` header when the bucket is empty.
async fn rate_limit(
    State(bucket): State<Arc<Mutex<TokenBucket>>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let allowed = {
        let mut guard = bucket.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.try_acquire()
    };
    if allowed {
        next.run(request).await
    } else {
        let mut response =
            HttpError::TooManyRequests("rate limit exceeded".to_string()).into_response();
        response.headers_mut().insert("retry-after", HeaderValue::from_static("1"));
        response
    }
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

// ============================================================================
// HTTP Caching (ETag + Cache-Control)
// ============================================================================

/// Compute a weak `ETag` from response body bytes using FNV-1a for speed.
fn compute_etag(body: &[u8]) -> String {
    // FNV-1a 64-bit — fast, non-cryptographic hash ideal for ETags.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in body {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("W/\"{hash:016x}\"")
}

/// Determine the `Cache-Control` value based on the request path.
///
/// - Health endpoints: `no-cache` (always revalidate)
/// - List endpoints (`/api/v1/<resource>`): short TTL (10s) + revalidation
/// - Single resource (`/api/v1/<resource>/<id>`): moderate TTL (60s) + revalidation
/// - `OpenAPI` spec: long TTL (1 hour, public)
fn cache_control_for_path(path: &str) -> &'static str {
    if path.starts_with("/health") || path.ends_with("/events/stream") {
        // Health endpoints always revalidate; SSE streams must not be cached.
        "no-cache"
    } else if path.contains("/openapi.json") {
        "public, max-age=3600"
    } else if let Some(rest) = path.strip_prefix("/api/v1/") {
        // Count path segments after /api/v1/
        let segments = rest.split('/').filter(|s| !s.is_empty()).count();
        if segments <= 1 {
            // List endpoint (e.g. /api/v1/orders)
            "private, max-age=10, must-revalidate"
        } else {
            // Single resource (e.g. /api/v1/orders/{id})
            "private, max-age=60, must-revalidate"
        }
    } else {
        "no-cache"
    }
}

/// Middleware that adds `ETag` and `Cache-Control` headers to GET responses,
/// and handles `If-None-Match` conditional requests (returns 304 Not Modified).
///
/// Only applied to successful GET responses with a response body.
async fn http_cache(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();

    // Only cache GET requests
    if method != Method::GET {
        return next.run(request).await;
    }

    // Extract If-None-Match header before passing the request
    let if_none_match =
        request.headers().get("if-none-match").and_then(|v| v.to_str().ok()).map(String::from);

    let response = next.run(request).await;

    // Only cache successful responses
    if !response.status().is_success() {
        return response;
    }

    let (mut parts, body) = response.into_parts();

    // Do not buffer indefinitely streaming responses such as SSE.
    let is_sse = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));
    if is_sse {
        parts.headers.insert("cache-control", HeaderValue::from_static("no-cache"));
        return Response::from_parts(parts, body);
    }

    // Collect the body bytes
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    // Compute ETag
    let etag = compute_etag(&body_bytes);

    // Check If-None-Match
    if let Some(client_etag) = if_none_match {
        // Strip quotes and W/ prefix for comparison
        let normalize = |s: &str| s.replace("W/", "").replace('"', "");
        if normalize(&client_etag) == normalize(&etag) {
            let mut not_modified = Response::new(Body::empty());
            *not_modified.status_mut() = StatusCode::NOT_MODIFIED;
            not_modified.headers_mut().insert(
                "etag",
                HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("")),
            );
            not_modified
                .headers_mut()
                .insert("cache-control", HeaderValue::from_static(cache_control_for_path(&path)));
            return not_modified;
        }
    }

    // Add caching headers
    parts
        .headers
        .insert("etag", HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("")));
    parts.headers.insert("cache-control", HeaderValue::from_static(cache_control_for_path(&path)));

    Response::from_parts(parts, Body::from(body_bytes))
}

/// Apply all standard middleware to a router.
///
/// Layers are applied inside-out in this order:
///
/// 1. **Tracing** (outermost after our layers — logs every request)
/// 2. **HTTP caching** — `ETag` generation + `Cache-Control` headers for GET responses
/// 3. **Rate limiting** — global token-bucket limiter (HTTP 429 on exhaustion)
/// 4. **Config guard** — fail closed for invalid tenant-routing auth configuration
/// 5. **Bearer auth** — constant-time token comparison + optional tenant binding
/// 6. **CORS** — explicit origin allowlist from `STATESET_HTTP_ALLOWED_ORIGINS`
/// 7. **Request ID** — `X-Request-ID` propagation for distributed tracing
pub(crate) fn apply_middleware(
    router: Router,
    with_cors: bool,
    with_request_id: bool,
    auth_config: Option<Vec<BearerAuthBinding>>,
    authz_config: Option<AuthzConfig>,
    misconfigured_auth_message: Option<String>,
    rate_limit_config: Option<RateLimitConfig>,
) -> Router {
    let mut router = router.layer(TraceLayer::new_for_http());

    // HTTP caching (ETag + Cache-Control) — innermost layer for GET responses
    router = router.layer(from_fn(http_cache));

    if let Some(config) = rate_limit_config {
        let bucket = create_rate_limiter(&config);
        router = router.layer(from_fn_with_state(bucket, rate_limit));
    }

    if let Some(config) = authz_config {
        router = router.layer(from_fn_with_state(config, require_authorization));
    }

    if let Some(bindings) = auth_config.filter(|bindings| !bindings.is_empty()) {
        router =
            router.layer(from_fn_with_state(BearerAuthConfig::new(bindings), require_bearer_auth));
    }

    if let Some(message) = misconfigured_auth_message {
        router = router.layer(from_fn_with_state(
            MisconfiguredApiAuthConfig::new(message),
            reject_misconfigured_api_auth,
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
    use axum::response::sse::{Event, KeepAlive, Sse};
    use axum::routing::{get, patch, post};
    use stateset_authz::{AuthzEngineBuilder, PermissionLevel, Role, RoleBuilder};
    use std::{convert::Infallible, time::Duration};
    use tokio_stream::{StreamExt as _, wrappers::IntervalStream};
    use tower::ServiceExt;

    fn auth_bindings(token: &str) -> Vec<BearerAuthBinding> {
        vec![BearerAuthBinding::new(token.to_string(), None, None)]
    }

    fn tenant_bound_auth_bindings(token: &str, tenant_id: &str) -> Vec<BearerAuthBinding> {
        vec![BearerAuthBinding::new(token.to_string(), Some(tenant_id.to_string()), None)]
    }

    fn actor_bound_auth_bindings(token: &str, actor_id: &str) -> Vec<BearerAuthBinding> {
        vec![BearerAuthBinding::new(token.to_string(), None, Some(actor_id.to_string()))]
    }

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
        let _router = apply_middleware(router, false, false, None, None, None, None);
    }

    #[test]
    fn apply_middleware_all() {
        let router = Router::new();
        let _router =
            apply_middleware(router, true, true, Some(auth_bindings("token")), None, None, None);
    }

    #[test]
    fn apply_middleware_cors_only() {
        let router = Router::new();
        let _router = apply_middleware(router, true, false, None, None, None, None);
    }

    #[test]
    fn apply_middleware_request_id_only() {
        let router = Router::new();
        let _router = apply_middleware(router, false, true, None, None, None, None);
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
        let app =
            apply_middleware(router, false, false, Some(auth_bindings("secret")), None, None, None);

        let response =
            app.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_allows_authorized_api_requests() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app =
            apply_middleware(router, false, false, Some(auth_bindings("secret")), None, None, None);

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
        let app =
            apply_middleware(router, false, false, Some(auth_bindings("secret")), None, None, None);

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
            Some(tenant_bound_auth_bindings("secret", "tenant-a")),
            None,
            None,
            None,
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
        let app =
            apply_middleware(router, false, false, Some(auth_bindings("secret")), None, None, None);

        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_blocks_unauthorized_deep_health_requests() {
        let router = Router::new().route("/health/deep", get(|| async { "ok" }));
        let app =
            apply_middleware(router, false, false, Some(auth_bindings("secret")), None, None, None);

        let response =
            app.oneshot(Request::get("/health/deep").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_allows_deep_health_without_tenant_header() {
        let router = Router::new().route("/health/deep", get(|| async { "ok" }));
        let app =
            apply_middleware(router, false, false, Some(auth_bindings("secret")), None, None, None);

        let response = app
            .oneshot(
                Request::get("/health/deep")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn misconfigured_tenant_routing_blocks_protected_routes() {
        let router = Router::new()
            .route("/api/v1/orders", get(|| async { "ok" }))
            .route("/health/deep", get(|| async { "ok" }))
            .route("/health", get(|| async { "ok" }));
        let app = apply_middleware(
            router,
            false,
            false,
            Some(auth_bindings("secret")),
            None,
            Some("tenant routing requires a bound bearer token".to_string()),
            None,
        );

        let api_response = app
            .clone()
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(api_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let deep_response = app
            .clone()
            .oneshot(
                Request::get("/health/deep")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(deep_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let health_response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cors_allows_localhost_origin_by_default() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let app = apply_middleware(router, true, false, None, None, None, None);

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
        let app = apply_middleware(router, true, false, None, None, None, None);

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

    #[test]
    fn token_bucket_allows_within_capacity() {
        let mut bucket = TokenBucket::new(10, 5);
        for _ in 0..5 {
            assert!(bucket.try_acquire(), "should allow requests within burst capacity");
        }
    }

    #[test]
    fn token_bucket_rejects_over_capacity() {
        let mut bucket = TokenBucket::new(10, 2);
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire(), "should reject requests beyond burst");
    }

    #[test]
    fn token_bucket_refills_over_time() {
        let mut bucket = TokenBucket::new(1000, 1);
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());
        // Manually advance the last_refill to simulate time passing
        bucket.last_refill -= std::time::Duration::from_millis(10);
        assert!(bucket.try_acquire(), "should refill tokens over time");
    }

    #[test]
    fn rate_limit_config_builds() {
        let config = RateLimitConfig { requests_per_second: 100, burst_size: 200 };
        let _limiter = create_rate_limiter(&config);
    }

    #[tokio::test]
    async fn rate_limit_allows_requests_within_limit() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let config = Some(RateLimitConfig { requests_per_second: 100, burst_size: 10 });
        let app = apply_middleware(router, false, false, None, None, None, config);

        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rate_limit_returns_429_when_exceeded() {
        let config = RateLimitConfig { requests_per_second: 1, burst_size: 1 };
        let bucket = create_rate_limiter(&config);

        // Exhaust the bucket
        {
            let mut guard = bucket.lock().unwrap();
            guard.try_acquire();
        }

        // Build the middleware with a pre-exhausted bucket
        let app: Router<()> = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(from_fn_with_state(bucket, rate_limit));

        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("retry-after").and_then(|v| v.to_str().ok()), Some("1"));
    }

    #[test]
    fn apply_middleware_with_rate_limit() {
        let router = Router::new();
        let config = Some(RateLimitConfig { requests_per_second: 50, burst_size: 100 });
        let _router = apply_middleware(router, false, false, None, None, None, config);
    }

    #[test]
    fn authz_target_maps_list_and_destructive_routes() {
        let list_request = Request::get("/api/v1/orders").body(Body::empty()).unwrap();
        let cancel_request =
            Request::patch("/api/v1/orders/ord_1/cancel").body(Body::empty()).unwrap();

        let list_target = authz_target(&list_request).expect("list target should map");
        assert_eq!(list_target, (Resource::new("orders"), Action::List));

        let cancel_target = authz_target(&cancel_request).expect("cancel target should map");
        assert_eq!(cancel_target, (Resource::new("orders"), Action::Delete));
    }

    #[tokio::test]
    async fn authz_requires_actor_header() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::viewer())
            .assign_role("viewer-1", "viewer")
            .build();
        let app = apply_middleware(
            router,
            false,
            false,
            None,
            Some(AuthzConfig::new(engine)),
            None,
            None,
        );

        let response =
            app.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn authz_allows_viewer_reads() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::viewer())
            .assign_role("viewer-1", "viewer")
            .build();
        let app = apply_middleware(
            router,
            false,
            false,
            None,
            Some(AuthzConfig::new(engine).with_trusted_actor_headers()),
            None,
            None,
        );

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("x-actor-id", "viewer-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn bound_actor_token_populates_authz_actor_header() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::viewer())
            .assign_role("viewer-1", "viewer")
            .build();
        let app = apply_middleware(
            router,
            false,
            false,
            Some(actor_bound_auth_bindings("secret", "viewer-1")),
            Some(AuthzConfig::new(engine)),
            None,
            None,
        );

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
    async fn bound_actor_token_rejects_conflicting_actor_header() {
        let router = Router::new().route("/api/v1/orders", get(|| async { "ok" }));
        let app = apply_middleware(
            router,
            false,
            false,
            Some(actor_bound_auth_bindings("secret", "viewer-1")),
            None,
            None,
            None,
        );

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .header("x-tenant-id", "tenant-1")
                    .header("x-actor-id", "admin-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn authz_denies_destructive_order_action_for_write_only_role() {
        let router = Router::new().route("/api/v1/orders/{id}/cancel", patch(|| async { "ok" }));
        let engine = AuthzEngineBuilder::new()
            .add_role(RoleBuilder::new("writer").default_level(PermissionLevel::Write).build())
            .assign_role("writer-1", "writer")
            .build();
        let app = apply_middleware(
            router,
            false,
            false,
            None,
            Some(AuthzConfig::new(engine).with_trusted_actor_headers()),
            None,
            None,
        );

        let response = app
            .oneshot(
                Request::patch("/api/v1/orders/ord_1/cancel")
                    .header("x-actor-id", "writer-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn auth_runs_before_authz_when_both_are_enabled() {
        let router = Router::new().route("/api/v1/orders", post(|| async { "ok" }));
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .assign_role("admin-1", "admin")
            .build();
        let app = apply_middleware(
            router,
            false,
            false,
            Some(auth_bindings("secret")),
            Some(AuthzConfig::new(engine)),
            None,
            None,
        );

        let response = app
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("x-actor-id", "admin-1")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // ========================================================================
    // HTTP Caching Tests
    // ========================================================================

    #[test]
    fn compute_etag_deterministic() {
        let etag1 = compute_etag(b"hello world");
        let etag2 = compute_etag(b"hello world");
        assert_eq!(etag1, etag2, "same input should produce same ETag");
    }

    #[test]
    fn compute_etag_differs_for_different_input() {
        let etag1 = compute_etag(b"hello");
        let etag2 = compute_etag(b"world");
        assert_ne!(etag1, etag2, "different input should produce different ETag");
    }

    #[test]
    fn compute_etag_is_weak() {
        let etag = compute_etag(b"test");
        assert!(etag.starts_with("W/\""), "ETag should be a weak validator");
    }

    #[test]
    fn cache_control_health_is_no_cache() {
        assert_eq!(cache_control_for_path("/health"), "no-cache");
        assert_eq!(cache_control_for_path("/health/ready"), "no-cache");
    }

    #[test]
    fn cache_control_openapi_is_public() {
        assert_eq!(cache_control_for_path("/api/v1/openapi.json"), "public, max-age=3600");
    }

    #[test]
    fn cache_control_list_endpoints() {
        assert_eq!(
            cache_control_for_path("/api/v1/orders"),
            "private, max-age=10, must-revalidate"
        );
        assert_eq!(
            cache_control_for_path("/api/v1/customers"),
            "private, max-age=10, must-revalidate"
        );
    }

    #[test]
    fn cache_control_single_resource() {
        assert_eq!(
            cache_control_for_path("/api/v1/orders/abc-123"),
            "private, max-age=60, must-revalidate"
        );
        assert_eq!(
            cache_control_for_path("/api/v1/customers/cust-1"),
            "private, max-age=60, must-revalidate"
        );
    }

    #[test]
    fn cache_control_event_stream_is_no_cache() {
        assert_eq!(cache_control_for_path("/api/v1/events/stream"), "no-cache");
    }

    #[tokio::test]
    async fn http_cache_adds_etag_to_get_response() {
        let app: Router<()> = Router::new()
            .route("/api/v1/orders", get(|| async { "[]" }))
            .layer(from_fn(http_cache));

        let response =
            app.oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap()).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("etag").is_some(), "should have ETag header");
        assert!(
            response.headers().get("cache-control").is_some(),
            "should have Cache-Control header"
        );
    }

    #[tokio::test]
    async fn http_cache_returns_304_on_matching_etag() {
        let app: Router<()> = Router::new()
            .route("/api/v1/orders", get(|| async { "[]" }))
            .layer(from_fn(http_cache));

        // First request to get the ETag
        let response = app
            .clone()
            .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let etag = response.headers().get("etag").unwrap().to_str().unwrap().to_string();

        // Second request with If-None-Match
        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("if-none-match", &etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }

    #[tokio::test]
    async fn http_cache_skips_post_requests() {
        use axum::routing::post;

        let app: Router<()> = Router::new()
            .route("/api/v1/orders", post(|| async { "created" }))
            .layer(from_fn(http_cache));

        let response = app
            .oneshot(Request::post("/api/v1/orders").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("etag").is_none(), "POST should not have ETag");
    }

    #[tokio::test]
    async fn http_cache_does_not_cache_error_responses() {
        async fn handler() -> (StatusCode, &'static str) {
            (StatusCode::NOT_FOUND, "not found")
        }

        let app: Router<()> =
            Router::new().route("/api/v1/orders/missing", get(handler)).layer(from_fn(http_cache));

        let response = app
            .oneshot(Request::get("/api/v1/orders/missing").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().get("etag").is_none(), "error responses should not have ETag");
    }

    #[tokio::test]
    async fn http_cache_returns_200_on_mismatched_etag() {
        let app: Router<()> = Router::new()
            .route("/api/v1/orders", get(|| async { "[]" }))
            .layer(from_fn(http_cache));

        let response = app
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("if-none-match", "W/\"stale-etag\"")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("etag").is_some());
    }

    #[tokio::test]
    async fn http_cache_skips_sse_responses() {
        let app: Router<()> = Router::new()
            .route(
                "/api/v1/events/stream",
                get(|| async {
                    let stream =
                        IntervalStream::new(tokio::time::interval(Duration::from_secs(60)))
                            .map(|_| Ok::<_, Infallible>(Event::default().data("tick")));
                    Sse::new(stream).keep_alive(KeepAlive::default())
                }),
            )
            .layer(from_fn(http_cache));

        let response = tokio::time::timeout(
            Duration::from_millis(200),
            app.oneshot(Request::get("/api/v1/events/stream").body(Body::empty()).unwrap()),
        )
        .await
        .expect("SSE response should not be buffered by cache middleware")
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        assert_eq!(
            response.headers().get("cache-control").and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
        assert!(
            response.headers().get("etag").is_none(),
            "streaming responses should not have ETags"
        );
    }
}
