//! HTTP middleware — request ID, logging, CORS, rate limiting, caching.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    Router,
    body::Body,
    extract::{MatchedPath, State},
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

/// Process-wide RED metrics registry for the HTTP surface.
///
/// Populated by [`track_http_metrics`] and rendered by the `/metrics`
/// exposition endpoint. A single registry per process mirrors the standard
/// Prometheus default-registry model.
static HTTP_METRICS: std::sync::LazyLock<stateset_observability::Metrics> =
    std::sync::LazyLock::new(|| {
        stateset_observability::init_metrics(stateset_observability::MetricsConfig::default())
    });

/// Access the process-wide HTTP RED metrics registry.
pub(crate) fn http_metrics() -> &'static stateset_observability::Metrics {
    &HTTP_METRICS
}

/// Route label used when a request does not match any registered route.
///
/// Keeps label cardinality bounded: raw (potentially attacker-controlled)
/// paths are never used as metric labels.
const UNMATCHED_ROUTE_LABEL: &str = "unmatched";

/// RED-metrics middleware: records request count, 4xx/5xx error counts, and a
/// latency histogram per `(method, route-pattern)`.
///
/// The route label comes from axum's [`MatchedPath`] (e.g.
/// `/api/v1/orders/{id}`), never the raw request path, so path parameters do
/// not explode label cardinality.
pub(crate) async fn track_http_metrics(request: Request<Body>, next: Next) -> Response {
    let method = request.method().as_str().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| UNMATCHED_ROUTE_LABEL.to_owned(), |path| path.as_str().to_owned());
    let start = Instant::now();
    let response = next.run(request).await;
    http_metrics().record_http_request(
        &method,
        &route,
        response.status().as_u16(),
        start.elapsed(),
    );
    response
}

/// Header name for request IDs.
pub(crate) static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub(crate) static X_TENANT_ID: HeaderName = HeaderName::from_static("x-tenant-id");
pub(crate) static X_ACTOR_ID: HeaderName = HeaderName::from_static("x-actor-id");
const CORS_ORIGINS_ENV: &str = "STATESET_HTTP_ALLOWED_ORIGINS";
const DEFAULT_CORS_ORIGINS: [&str; 2] = ["http://localhost:3000", "http://127.0.0.1:3000"];

#[derive(Clone)]
pub(crate) struct BearerAuthBinding {
    token: Arc<str>,
    bound_tenant_id: Option<Arc<str>>,
    bound_actor_id: Option<Arc<str>>,
}

impl std::fmt::Debug for BearerAuthBinding {
    /// The bearer token is a credential: it is never rendered, even in debug
    /// output, so accidental `{:?}` logging cannot leak it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BearerAuthBinding")
            .field("token", &"[REDACTED]")
            .field("bound_tenant_id", &self.bound_tenant_id)
            .field("bound_actor_id", &self.bound_actor_id)
            .finish()
    }
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
    strict_path_mapping: bool,
}

impl AuthzConfig {
    pub(crate) fn new(engine: AuthzEngine) -> Self {
        Self {
            engine: Arc::new(Mutex::new(engine)),
            trust_actor_headers: false,
            strict_path_mapping: false,
        }
    }

    pub(crate) const fn with_trusted_actor_headers(mut self) -> Self {
        self.trust_actor_headers = true;
        self
    }

    /// Deny `/api/v1` requests that cannot be mapped to a resource/action pair
    /// instead of letting them bypass authorization.
    pub(crate) const fn with_strict_path_mapping(mut self) -> Self {
        self.strict_path_mapping = true;
        self
    }
}

fn bearer_token_from_header(value: &str) -> Option<&str> {
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?;
    let token = parts.next()?.trim();
    if scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() { Some(token) } else { None }
}

/// Constant-time string equality for credentials.
///
/// Both inputs are hashed to fixed-length SHA-256 digests and the digests are
/// compared with [`subtle::ConstantTimeEq`], so neither the length of the
/// secret nor the position of the first mismatch is observable through timing.
pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    use sha2::{Digest, Sha256};
    use subtle::ConstantTimeEq;

    let a_digest = Sha256::digest(a.as_bytes());
    let b_digest = Sha256::digest(b.as_bytes());
    bool::from(a_digest.ct_eq(&b_digest))
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
            .rev()
            .find(|(_, binding)| constant_time_eq(provided, binding.token.as_ref()))
            .map(|(_, binding)| binding),
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
        if config.strict_path_mapping && request.uri().path().starts_with("/api/v1") {
            return HttpError::Forbidden(
                "request path is not mapped to an authorization target".to_string(),
            )
            .into_response();
        }
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
    /// Key clients by the client IP reported in `X-Forwarded-For` /
    /// `Forwarded` instead of the TCP peer address.
    ///
    /// Only enable this when the server sits behind a proxy that overwrites or
    /// appends to those headers; otherwise any client can pick its own bucket.
    /// Defaults to `false`.
    pub trust_proxy_headers: bool,
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

    /// A bucket that would refill to capacity right now carries no throttling
    /// state — the client has been idle long enough to be forgotten.
    fn is_idle(&self) -> bool {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        (self.tokens + elapsed * self.rate) >= self.capacity
    }
}

/// Maximum number of client buckets tracked before eviction kicks in.
pub(crate) const RATE_LIMITER_MAX_KEYS: usize = 10_000;

/// Per-client rate limiter: independent token buckets keyed by peer IP, so
/// one abusive client exhausts its own budget without starving other tenants.
///
/// Memory is bounded by `max_keys`: when full, idle (fully refilled) buckets
/// are dropped first, then the least-recently-used bucket.
pub(crate) struct KeyedRateLimiter {
    buckets: Mutex<HashMap<IpAddr, TokenBucket>>,
    config: RateLimitConfig,
    max_keys: usize,
}

impl KeyedRateLimiter {
    pub(crate) fn new(config: &RateLimitConfig, max_keys: usize) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            config: config.clone(),
            max_keys: max_keys.max(1),
        }
    }

    pub(crate) fn try_acquire(&self, key: IpAddr) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if !buckets.contains_key(&key) && buckets.len() >= self.max_keys {
            Self::evict(&mut buckets);
        }
        buckets
            .entry(key)
            .or_insert_with(|| {
                TokenBucket::new(self.config.requests_per_second, self.config.burst_size)
            })
            .try_acquire()
    }

    /// Drop idle buckets (a fully refilled bucket carries no throttling
    /// state); if none are idle, drop the least-recently-used bucket so a new
    /// client can always be admitted.
    fn evict(buckets: &mut HashMap<IpAddr, TokenBucket>) {
        let before = buckets.len();
        buckets.retain(|_, bucket| !bucket.is_idle());
        if buckets.len() == before {
            if let Some(oldest) =
                buckets.iter().min_by_key(|(_, b)| b.last_refill).map(|(ip, _)| *ip)
            {
                buckets.remove(&oldest);
            }
        }
    }

    #[cfg(test)]
    fn tracked_keys(&self) -> usize {
        self.buckets.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()
    }
}

/// Extract the client IP a trusted proxy reported in `X-Forwarded-For` or
/// `Forwarded`.
///
/// The *last* entry is used: it is the one appended by the nearest (trusted)
/// proxy, whereas earlier entries are client-controlled.
fn forwarded_client_ip(request: &Request<Body>) -> Option<IpAddr> {
    fn parse_ip(raw: &str) -> Option<IpAddr> {
        let value = raw.trim().trim_matches('"');
        if let Ok(ip) = value.parse::<IpAddr>() {
            return Some(ip);
        }
        // "[v6]:port", "v4:port" or "[v6]" forms.
        if let Ok(addr) = value.parse::<SocketAddr>() {
            return Some(addr.ip());
        }
        value.strip_prefix('[').and_then(|v| v.strip_suffix(']')).and_then(|v| v.parse().ok())
    }

    let headers = request.headers();
    if let Some(ip) = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(parse_ip)
        .next_back()
    {
        return Some(ip);
    }
    headers
        .get_all("forwarded")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .flat_map(|element| element.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            name.eq_ignore_ascii_case("for").then_some(value)
        })
        .filter_map(parse_ip)
        .next_back()
}

/// Resolve the bucket key for a request.
///
/// Order of precedence: trusted proxy headers (only when
/// [`RateLimitConfig::trust_proxy_headers`] is set), then the TCP peer from
/// `ConnectInfo`. Requests with neither share one fallback bucket, and a
/// one-time warning is logged because the limiter then throttles the whole
/// process rather than individual clients.
fn rate_limit_key(request: &Request<Body>, trust_proxy_headers: bool) -> IpAddr {
    static FALLBACK_WARNED: std::sync::Once = std::sync::Once::new();

    if trust_proxy_headers {
        if let Some(ip) = forwarded_client_ip(request) {
            return ip;
        }
    }
    if let Some(ci) = request.extensions().get::<axum::extract::ConnectInfo<SocketAddr>>() {
        return ci.0.ip();
    }
    FALLBACK_WARNED.call_once(|| {
        tracing::warn!(
            "rate limiter could not determine the client address (no ConnectInfo and no \
             trusted proxy headers); all clients share one bucket, so rate limiting is \
             per-process rather than per-client. Serve with `into_make_service_with_connect_info` \
             or enable `trust_proxy_headers` behind a trusted proxy."
        );
    });
    IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
}

/// Middleware that enforces a per-client request rate limit.
///
/// Clients are keyed by [`rate_limit_key`]. Returns HTTP 429 with a
/// `Retry-After` header when the client's bucket is empty.
async fn rate_limit(
    State(limiter): State<Arc<KeyedRateLimiter>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let key = rate_limit_key(&request, limiter.config.trust_proxy_headers);
    if limiter.try_acquire(key) {
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
        let limiter = Arc::new(KeyedRateLimiter::new(&config, RATE_LIMITER_MAX_KEYS));
        router = router.layer(from_fn_with_state(limiter, rate_limit));
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
    fn keyed_rate_limiter_isolates_clients() {
        let limiter = KeyedRateLimiter::new(
            &RateLimitConfig { requests_per_second: 1, burst_size: 2, trust_proxy_headers: false },
            100,
        );
        let a: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        let b: std::net::IpAddr = "10.0.0.2".parse().unwrap();
        assert!(limiter.try_acquire(a));
        assert!(limiter.try_acquire(a));
        assert!(!limiter.try_acquire(a), "client A must be throttled after its burst");
        assert!(limiter.try_acquire(b), "client B must not be starved by client A");
    }

    #[test]
    fn keyed_rate_limiter_bounds_tracked_keys() {
        let limiter = KeyedRateLimiter::new(
            &RateLimitConfig { requests_per_second: 1, burst_size: 1, trust_proxy_headers: false },
            4,
        );
        for i in 0..50u8 {
            let ip: std::net::IpAddr = format!("10.1.0.{i}").parse().unwrap();
            let _ = limiter.try_acquire(ip);
        }
        assert!(
            limiter.tracked_keys() <= 4,
            "tracked keys must stay bounded, got {}",
            limiter.tracked_keys()
        );
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
    fn bearer_auth_binding_debug_redacts_token() {
        let binding = BearerAuthBinding::new(
            "super-secret-token-value".to_string(),
            Some("tenant-1".to_string()),
            Some("actor-1".to_string()),
        );
        let rendered = format!("{binding:?}");
        assert!(!rendered.contains("super-secret-token-value"), "token leaked: {rendered}");
        assert!(rendered.contains("[REDACTED]"));
        assert!(rendered.contains("tenant-1"));

        let config = BearerAuthConfig::new(vec![binding]);
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("super-secret-token-value"), "token leaked: {rendered}");
    }

    #[test]
    fn rate_limit_key_ignores_proxy_headers_unless_trusted() {
        let peer: SocketAddr = "10.0.0.7:4000".parse().unwrap();
        let mut request = Request::get("/health")
            .header("x-forwarded-for", "203.0.113.9, 198.51.100.4")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(axum::extract::ConnectInfo(peer));

        assert_eq!(rate_limit_key(&request, false), peer.ip());
        assert_eq!(rate_limit_key(&request, true), "198.51.100.4".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn rate_limit_key_parses_forwarded_header_forms() {
        let request = Request::get("/health")
            .header("forwarded", "for=192.0.2.60;proto=http, for=\"[2001:db8::1]:8080\"")
            .body(Body::empty())
            .unwrap();
        assert_eq!(rate_limit_key(&request, true), "2001:db8::1".parse::<IpAddr>().unwrap());

        // Garbage headers fall through to the peer address / fallback bucket.
        let request = Request::get("/health")
            .header("x-forwarded-for", "not-an-ip")
            .body(Body::empty())
            .unwrap();
        assert_eq!(rate_limit_key(&request, true), IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    }

    #[test]
    fn constant_time_eq_behaves_like_string_equality() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secreu"));
        assert!(!constant_time_eq("secret", "secret1"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("a", ""));
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
        let config = RateLimitConfig {
            requests_per_second: 100,
            burst_size: 200,
            trust_proxy_headers: false,
        };
        let _limiter = KeyedRateLimiter::new(&config, RATE_LIMITER_MAX_KEYS);
    }

    #[tokio::test]
    async fn rate_limit_allows_requests_within_limit() {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let config = Some(RateLimitConfig {
            requests_per_second: 100,
            burst_size: 10,
            trust_proxy_headers: false,
        });
        let app = apply_middleware(router, false, false, None, None, None, config);

        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rate_limit_returns_429_when_exceeded() {
        let config =
            RateLimitConfig { requests_per_second: 1, burst_size: 1, trust_proxy_headers: false };
        let limiter = Arc::new(KeyedRateLimiter::new(&config, RATE_LIMITER_MAX_KEYS));

        // Exhaust the fallback bucket (oneshot requests carry no ConnectInfo).
        assert!(limiter.try_acquire(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)));

        // Build the middleware with a pre-exhausted bucket
        let app: Router<()> = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(from_fn_with_state(limiter, rate_limit));

        let response =
            app.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("retry-after").and_then(|v| v.to_str().ok()), Some("1"));
    }

    #[tokio::test]
    async fn rate_limit_keys_by_connect_info_peer_ip() {
        let config =
            RateLimitConfig { requests_per_second: 1, burst_size: 1, trust_proxy_headers: false };
        let limiter = Arc::new(KeyedRateLimiter::new(&config, RATE_LIMITER_MAX_KEYS));
        let app: Router<()> = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(from_fn_with_state(limiter, rate_limit));

        let request_from = |ip: &str| {
            let mut request = Request::get("/health").body(Body::empty()).unwrap();
            let addr: SocketAddr = format!("{ip}:12345").parse().unwrap();
            request.extensions_mut().insert(axum::extract::ConnectInfo(addr));
            request
        };

        // Client A uses its single-token burst, then gets throttled.
        let ok = app.clone().oneshot(request_from("10.9.0.1")).await.unwrap();
        assert_eq!(ok.status(), StatusCode::OK);
        let throttled = app.clone().oneshot(request_from("10.9.0.1")).await.unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);

        // Client B is unaffected by A's exhaustion.
        let other = app.clone().oneshot(request_from("10.9.0.2")).await.unwrap();
        assert_eq!(other.status(), StatusCode::OK);
    }

    #[test]
    fn apply_middleware_with_rate_limit() {
        let router = Router::new();
        let config = Some(RateLimitConfig {
            requests_per_second: 50,
            burst_size: 100,
            trust_proxy_headers: false,
        });
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
    async fn strict_authz_denies_unmapped_api_paths() {
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
            Some(AuthzConfig::new(engine).with_strict_path_mapping()),
            None,
            None,
        );

        // HEAD requests have no authz_target mapping, so strict mode denies them.
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::HEAD)
                    .uri("/api/v1/orders")
                    .header("authorization", "Bearer secret")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn non_strict_authz_allows_unmapped_api_paths() {
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
                Request::builder()
                    .method(Method::HEAD)
                    .uri("/api/v1/orders")
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

    // ── RED metrics middleware ─────────────────────────────────────────

    fn red_test_router(route: &str) -> Router {
        Router::new()
            .route(route, get(|| async { (StatusCode::OK, "ok") }))
            .layer(from_fn(track_http_metrics))
    }

    async fn red_test_request(router: Router, path: &str) -> StatusCode {
        router
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn red_middleware_records_success_error_classes() {
        let router = Router::new()
            .route("/red-classes/ok/{id}", get(|| async { (StatusCode::OK, "ok") }))
            .route(
                "/red-classes/missing/{id}",
                get(|| async { (StatusCode::NOT_FOUND, "missing") }),
            )
            .route(
                "/red-classes/broken/{id}",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
            )
            .layer(from_fn(track_http_metrics));

        assert_eq!(red_test_request(router.clone(), "/red-classes/ok/1").await, StatusCode::OK);
        assert_eq!(
            red_test_request(router.clone(), "/red-classes/missing/2").await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            red_test_request(router, "/red-classes/broken/3").await,
            StatusCode::INTERNAL_SERVER_ERROR
        );

        let snap = http_metrics().snapshot();

        let ok = snap
            .http_by_route
            .get(&("GET".to_owned(), "/red-classes/ok/{id}".to_owned()))
            .expect("200 route recorded");
        assert_eq!(ok.requests, 1);
        assert_eq!(ok.errors_4xx, 0);
        assert_eq!(ok.errors_5xx, 0);
        let (last_bound, last_count) = ok.latency_buckets[ok.latency_buckets.len() - 1];
        assert!(last_bound.is_infinite());
        assert_eq!(last_count, 1);

        let missing = snap
            .http_by_route
            .get(&("GET".to_owned(), "/red-classes/missing/{id}".to_owned()))
            .expect("404 route recorded");
        assert_eq!(missing.requests, 1);
        assert_eq!(missing.errors_4xx, 1);
        assert_eq!(missing.errors_5xx, 0);

        let broken = snap
            .http_by_route
            .get(&("GET".to_owned(), "/red-classes/broken/{id}".to_owned()))
            .expect("500 route recorded");
        assert_eq!(broken.requests, 1);
        assert_eq!(broken.errors_4xx, 0);
        assert_eq!(broken.errors_5xx, 1);
    }

    #[tokio::test]
    async fn red_middleware_uses_matched_path_not_raw_path() {
        let router = red_test_router("/red-cardinality/{id}");
        for id in ["1", "2", "abc-def"] {
            let status = red_test_request(router.clone(), &format!("/red-cardinality/{id}")).await;
            assert_eq!(status, StatusCode::OK);
        }

        let snap = http_metrics().snapshot();
        let pattern = snap
            .http_by_route
            .get(&("GET".to_owned(), "/red-cardinality/{id}".to_owned()))
            .expect("route pattern key present");
        assert_eq!(pattern.requests, 3, "all raw paths collapse onto the route pattern");
        for raw in ["/red-cardinality/1", "/red-cardinality/2", "/red-cardinality/abc-def"] {
            assert!(
                !snap.http_by_route.contains_key(&("GET".to_owned(), raw.to_owned())),
                "raw path {raw} must not appear as a metric label"
            );
        }
    }

    #[tokio::test]
    async fn red_middleware_labels_unmatched_routes_safely() {
        let router = red_test_router("/red-unmatched-known");
        let status =
            red_test_request(router, "/red-unmatched-nonexistent/secret-cardinality-bomb").await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let snap = http_metrics().snapshot();
        let unmatched = snap
            .http_by_route
            .get(&("GET".to_owned(), UNMATCHED_ROUTE_LABEL.to_owned()))
            .expect("unmatched requests recorded under the fixed label");
        assert!(unmatched.requests >= 1);
        assert!(unmatched.errors_4xx >= 1);
        assert!(!snap.http_by_route.contains_key(&(
            "GET".to_owned(),
            "/red-unmatched-nonexistent/secret-cardinality-bomb".to_owned()
        )));
    }
}
