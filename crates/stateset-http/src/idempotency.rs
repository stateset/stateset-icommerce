//! Idempotency-Key middleware for REST mutation endpoints.
//!
//! Honors an optional `Idempotency-Key` request header on `POST` create
//! endpoints (orders, payments, refunds, returns, …). The first request for a
//! `(tenant, key)` pair runs the handler and caches the response (status code,
//! body bytes, content type, and a hash of the request body). Subsequent
//! requests with the same key:
//!
//! - **identical request body** → the cached response is replayed verbatim with
//!   an `Idempotency-Replayed: true` header and **no duplicate resource** is
//!   created;
//! - **different request body** → the request is rejected with HTTP 422
//!   ([`HttpError::ValidationError`]) carrying the standard API error envelope.
//!
//! Keys are scoped per tenant (`x-tenant-id`), so the same key used by two
//! tenants resolves to two independent cache entries.
//!
//! The cache is a bounded, time-to-live (TTL) store that mirrors the
//! shared-state pattern used by the rate limiter in [`crate::middleware`]: an
//! `Arc<Mutex<…>>` held for the lifetime of the router. Entries older than the
//! configured TTL are lazily evicted, and the store is capped at a maximum
//! entry count with FIFO eviction of the oldest keys to bound memory use.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderName, HeaderValue, Method, Request, StatusCode, header::CONTENT_TYPE},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt as _;

use crate::error::HttpError;

/// Request header carrying the client-chosen idempotency key.
pub(crate) static IDEMPOTENCY_KEY: HeaderName = HeaderName::from_static("idempotency-key");
/// Response header set to `true` when a cached response is replayed.
pub(crate) static IDEMPOTENCY_REPLAYED: HeaderName =
    HeaderName::from_static("idempotency-replayed");
/// Request header carrying the tenant id (mirrors `crate::middleware::X_TENANT_ID`).
static X_TENANT_ID: HeaderName = HeaderName::from_static("x-tenant-id");

/// Default time-to-live for cached idempotent responses (24 hours).
const DEFAULT_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Default maximum number of cached idempotency entries.
const DEFAULT_MAX_ENTRIES: usize = 10_000;
/// Maximum request body size buffered for idempotency hashing (1 `MiB`).
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// A cached idempotent response plus the hash of the originating request body.
#[derive(Clone, Debug)]
struct CachedResponse {
    status: StatusCode,
    content_type: Option<HeaderValue>,
    body: Vec<u8>,
    request_body_hash: [u8; 32],
    stored_at: Instant,
}

/// Bounded, TTL-scoped store of idempotent responses keyed by `(tenant, key)`.
#[derive(Debug)]
struct IdempotencyStore {
    entries: HashMap<(String, String), CachedResponse>,
    /// Insertion order for FIFO eviction when capacity is exceeded.
    order: VecDeque<(String, String)>,
    ttl: Duration,
    max_entries: usize,
}

impl IdempotencyStore {
    fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            ttl,
            max_entries: max_entries.max(1),
        }
    }

    /// Fetch a non-expired entry, evicting it if it has expired.
    fn get(&mut self, key: &(String, String), now: Instant) -> Option<CachedResponse> {
        let expired = self
            .entries
            .get(key)
            .is_some_and(|entry| now.duration_since(entry.stored_at) >= self.ttl);
        if expired {
            self.entries.remove(key);
            return None;
        }
        self.entries.get(key).cloned()
    }

    /// Insert a new entry, enforcing the entry cap with FIFO eviction.
    fn insert(&mut self, key: (String, String), value: CachedResponse) {
        if self.entries.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.entries.len() > self.max_entries {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

/// Shared idempotency cache wrapped for use as axum middleware state.
#[derive(Clone, Debug)]
pub(crate) struct IdempotencyLayer {
    store: Arc<Mutex<IdempotencyStore>>,
}

impl IdempotencyLayer {
    /// Create a layer with the default TTL and capacity.
    pub(crate) fn new() -> Self {
        Self::with_config(DEFAULT_TTL, DEFAULT_MAX_ENTRIES)
    }

    /// Create a layer with explicit TTL and capacity (used in tests).
    fn with_config(ttl: Duration, max_entries: usize) -> Self {
        Self { store: Arc::new(Mutex::new(IdempotencyStore::new(ttl, max_entries))) }
    }
}

impl Default for IdempotencyLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Collision-resistant SHA-256 digest of the request body, used to detect when
/// an `Idempotency-Key` is replayed with a *different* body (a client conflict).
///
/// A non-cryptographic hash (e.g. FNV-1a) is unsuitable here: an attacker who
/// reuses a victim's key could grind a body that collides with the original and
/// thereby slip a different request past the "same key, different body" guard
/// (or have a cached response replayed for a body it never matched). SHA-256
/// makes such a collision computationally infeasible.
fn hash_body(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Whether a path is a `POST` create endpoint that participates in idempotency.
///
/// The set covers resource-creating collection endpoints plus the mutation
/// sub-routes (`/refund`) that create new financial records. Action routes that
/// only transition existing resources (`/complete`, `/cancel`, …) are excluded.
fn is_idempotent_post_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/api/v1/") else {
        return false;
    };
    let segments = rest.split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    match segments.as_slice() {
        // Collection-create endpoints: POST /api/v1/<resource>
        ["orders"]
        | ["payments"]
        | ["returns"]
        | ["invoices"]
        | ["shipments"]
        | ["customers"]
        | ["products"] => true,
        // Payment refund creates a new refund record.
        ["payments", _id, "refund"] => true,
        _ => false,
    }
}

/// Axum middleware enforcing `Idempotency-Key` semantics on create endpoints.
pub(crate) async fn idempotency(
    State(layer): State<IdempotencyLayer>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Only POST create endpoints participate; everything else passes through.
    if request.method() != Method::POST || !is_idempotent_post_path(request.uri().path()) {
        return next.run(request).await;
    }

    // Extract the idempotency key; absent key disables caching for this request.
    let key = request
        .headers()
        .get(&IDEMPOTENCY_KEY)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let Some(key) = key else {
        return next.run(request).await;
    };
    if key.len() > 255 {
        return HttpError::BadRequest("idempotency-key exceeds 255 characters".to_string())
            .into_response();
    }

    // Tenant scoping: requests without a tenant share the "" namespace.
    let tenant = request
        .headers()
        .get(&X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_default();
    let cache_key = (tenant, key);

    // Buffer the request body so we can hash it and replay the handler with it.
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return HttpError::BadRequest("request body too large for idempotency".to_string())
                .into_response();
        }
    };
    let request_hash = hash_body(&body_bytes);
    let now = Instant::now();

    // Cache lookup: replay on identical body, reject on conflicting body.
    {
        let mut store = layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = store.get(&cache_key, now) {
            if cached.request_body_hash == request_hash {
                return replay_response(&cached);
            }
            return HttpError::ValidationError(
                "idempotency-key already used with a different request body".to_string(),
            )
            .into_response();
        }
    }

    // Cache miss: run the inner handler with the buffered body restored.
    let request = Request::from_parts(parts, Body::from(body_bytes));
    let response = next.run(request).await;

    // Only cache deterministic, successfully-produced responses. Server errors
    // (5xx) and rate-limit/conflict (429) are transient and must be retryable.
    let status = response.status();
    let cacheable = status.is_success()
        || status == StatusCode::BAD_REQUEST
        || status == StatusCode::UNPROCESSABLE_ENTITY
        || status == StatusCode::CONFLICT
        || status == StatusCode::NOT_FOUND;
    if !cacheable {
        return response;
    }

    let (mut resp_parts, resp_body) = response.into_parts();
    let resp_bytes = match resp_body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return HttpError::InternalError("failed to buffer response".to_string())
                .into_response();
        }
    };

    let content_type = resp_parts.headers.get(CONTENT_TYPE).cloned();
    let cached = CachedResponse {
        status: resp_parts.status,
        content_type,
        body: resp_bytes.to_vec(),
        request_body_hash: request_hash,
        stored_at: now,
    };
    {
        let mut store = layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.insert(cache_key, cached);
    }

    // Annotate the freshly-stored response so callers can observe first-write.
    resp_parts.headers.insert(IDEMPOTENCY_REPLAYED.clone(), HeaderValue::from_static("false"));
    Response::from_parts(resp_parts, Body::from(resp_bytes))
}

/// Reconstruct a response from a cached entry.
fn replay_response(cached: &CachedResponse) -> Response {
    let mut response = Response::new(Body::from(cached.body.clone()));
    *response.status_mut() = cached.status;
    if let Some(content_type) = &cached.content_type {
        response.headers_mut().insert(CONTENT_TYPE, content_type.clone());
    }
    response.headers_mut().insert(IDEMPOTENCY_REPLAYED.clone(), HeaderValue::from_static("true"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::http::Request;
    use axum::routing::post;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tower::ServiceExt as _;

    fn app(layer: IdempotencyLayer, counter: Arc<AtomicU64>) -> Router {
        Router::new()
            .route(
                "/api/v1/orders",
                post(move || {
                    let counter = counter.clone();
                    async move {
                        let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                        (
                            StatusCode::CREATED,
                            [(CONTENT_TYPE, "application/json")],
                            format!("{{\"id\":\"order-{n}\"}}"),
                        )
                    }
                }),
            )
            .layer(axum::middleware::from_fn_with_state(layer, idempotency))
    }

    #[test]
    fn is_idempotent_post_path_matches_create_endpoints() {
        assert!(is_idempotent_post_path("/api/v1/orders"));
        assert!(is_idempotent_post_path("/api/v1/payments"));
        assert!(is_idempotent_post_path("/api/v1/returns"));
        assert!(is_idempotent_post_path("/api/v1/payments/abc/refund"));
        // Action routes that mutate existing resources are excluded.
        assert!(!is_idempotent_post_path("/api/v1/payments/abc/complete"));
        assert!(!is_idempotent_post_path("/api/v1/orders/abc/cancel"));
        // Non-API paths never participate.
        assert!(!is_idempotent_post_path("/health"));
    }

    #[test]
    fn store_evicts_oldest_when_over_capacity() {
        let mut store = IdempotencyStore::new(DEFAULT_TTL, 2);
        let now = Instant::now();
        let make = |seed: u8| CachedResponse {
            status: StatusCode::CREATED,
            content_type: None,
            body: Vec::new(),
            request_body_hash: [seed; 32],
            stored_at: now,
        };
        store.insert(("t".into(), "a".into()), make(1));
        store.insert(("t".into(), "b".into()), make(2));
        store.insert(("t".into(), "c".into()), make(3));
        assert_eq!(store.entries.len(), 2);
        // "a" was the oldest and should have been evicted.
        assert!(store.get(&("t".into(), "a".into()), now).is_none());
        assert!(store.get(&("t".into(), "c".into()), now).is_some());
    }

    #[test]
    fn store_expires_entries_after_ttl() {
        let mut store = IdempotencyStore::new(Duration::from_secs(10), 100);
        let stored_at = Instant::now();
        store.insert(
            ("t".into(), "a".into()),
            CachedResponse {
                status: StatusCode::CREATED,
                content_type: None,
                body: Vec::new(),
                request_body_hash: [1u8; 32],
                stored_at,
            },
        );
        let later = stored_at + Duration::from_secs(11);
        assert!(store.get(&("t".into(), "a".into()), later).is_none());
    }

    #[tokio::test]
    async fn replays_identical_request_without_rerunning_handler() {
        let counter = Arc::new(AtomicU64::new(0));
        let layer = IdempotencyLayer::new();
        let app = app(layer, counter.clone());

        let body = "{\"customer\":\"c1\"}";
        let first = app
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "key-1")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        assert_eq!(
            first.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
            Some("false")
        );
        let first_body = axum::body::to_bytes(first.into_body(), usize::MAX).await.unwrap();

        let second = app
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "key-1")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::CREATED);
        assert_eq!(
            second.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
            Some("true")
        );
        let second_body = axum::body::to_bytes(second.into_body(), usize::MAX).await.unwrap();

        // Handler ran exactly once; both responses are byte-identical.
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(first_body, second_body);
    }

    #[tokio::test]
    async fn conflicting_body_for_same_key_is_rejected() {
        let counter = Arc::new(AtomicU64::new(0));
        let app = app(IdempotencyLayer::new(), counter.clone());

        let first = app
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "key-2")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from("{\"customer\":\"c1\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);

        let conflict = app
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "key-2")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from("{\"customer\":\"DIFFERENT\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::UNPROCESSABLE_ENTITY);
        // The conflicting request never reaches the handler.
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn same_key_is_isolated_per_tenant() {
        let counter = Arc::new(AtomicU64::new(0));
        let app = app(IdempotencyLayer::new(), counter.clone());

        let body = "{\"customer\":\"shared\"}";
        for tenant in ["tenant-a", "tenant-b"] {
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/api/v1/orders")
                        .header("idempotency-key", "shared-key")
                        .header("x-tenant-id", tenant)
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
            assert_eq!(
                resp.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
                Some("false"),
                "each tenant's first use of the key must be a fresh write"
            );
        }
        // Distinct tenants → two independent handler invocations.
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn missing_key_disables_caching() {
        let counter = Arc::new(AtomicU64::new(0));
        let app = app(IdempotencyLayer::new(), counter.clone());

        for _ in 0..2 {
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/api/v1/orders")
                        .header("x-tenant-id", "tenant-a")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
            assert!(resp.headers().get("idempotency-replayed").is_none());
        }
        // No key → handler runs every time.
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
