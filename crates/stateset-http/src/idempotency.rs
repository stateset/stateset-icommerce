//! Idempotency-Key middleware for REST mutation endpoints.
//!
//! Honors an `Idempotency-Key` request header on `POST` create endpoints
//! (orders, payments, refunds, returns, …). The first request for a
//! `(tenant, key)` pair runs the handler and stores the response (status code,
//! body bytes, content type, and a fingerprint of the request). Subsequent
//! requests with the same key:
//!
//! - **identical request fingerprint** → the stored response is replayed
//!   verbatim with an `Idempotency-Replayed: true` header and **no duplicate
//!   resource** is created;
//! - **different fingerprint** (method + path + body hash) → the request is
//!   rejected with HTTP 422 ([`HttpError::ValidationError`]) carrying the
//!   standard API error envelope.
//!
//! Keys are scoped per tenant (`x-tenant-id`), so the same key used by two
//! tenants resolves to two independent entries.
//!
//! # Durability
//!
//! Entries are persisted to the commerce database (`http_idempotency_keys`
//! table) when the layer is built with [`IdempotencyLayer::with_durable_store`]
//! — the [`crate::server::ServerBuilder`] wires this automatically — so
//! replays survive process restarts and work across replicas sharing a
//! database. The in-process map acts as a bounded read-through cache in front
//! of the durable store: lookups consult memory first, fall back to the
//! database, and populate memory on a durable hit. Durable-store failures
//! degrade gracefully to memory-only behavior (logged, never request-fatal).
//!
//! TTL cleanup is enforced lazily on read (expired rows are deleted when
//! touched) plus an opportunistic bulk sweep every
//! [`SWEEP_EVERY_N_WRITES`] durable writes.
//!
//! # Required keys (HTTP 428)
//!
//! When [`IdempotencyLayer::with_required_keys`] is enabled (the
//! `ServerBuilder` default, opt-out via `require_idempotency_keys(false)` or
//! `STATESET_HTTP_REQUIRE_IDEMPOTENCY_KEYS=false`), money-moving create
//! endpoints — `POST /orders`, `/payments`, `/payments/{id}/refund`, and
//! `/ap/payments` — reject requests without an `Idempotency-Key` header with
//! HTTP 428 (Precondition Required).

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderName, HeaderValue, Method, Request, StatusCode, header::CONTENT_TYPE},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use http_body_util::BodyExt as _;
use stateset_embedded::{Commerce, HttpIdempotencyRecord};

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
/// Default maximum number of in-memory cached idempotency entries.
const DEFAULT_MAX_ENTRIES: usize = 10_000;
/// Maximum request body size buffered for idempotency hashing (1 `MiB`).
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Run a bulk expiry sweep of the durable store every N durable writes.
const SWEEP_EVERY_N_WRITES: u64 = 512;

/// A cached idempotent response plus the fingerprint of the originating request.
#[derive(Clone, Debug)]
struct CachedResponse {
    status: StatusCode,
    content_type: Option<HeaderValue>,
    body: Vec<u8>,
    /// Hex SHA-256 of method + path + request body.
    request_fingerprint: String,
    created_at: DateTime<Utc>,
}

/// Bounded, TTL-scoped in-memory cache of idempotent responses keyed by
/// `(tenant, key)`. Acts as a read-through cache in front of the durable store.
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

    fn is_expired(&self, entry: &CachedResponse, now: DateTime<Utc>) -> bool {
        let age = now.signed_duration_since(entry.created_at);
        age >= chrono::Duration::from_std(self.ttl).unwrap_or(chrono::Duration::MAX)
    }

    /// Fetch a non-expired entry, evicting it if it has expired.
    fn get(&mut self, key: &(String, String), now: DateTime<Utc>) -> Option<CachedResponse> {
        let expired = self.entries.get(key).is_some_and(|entry| self.is_expired(entry, now));
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

/// Shared idempotency state wrapped for use as axum middleware state.
#[derive(Clone)]
pub struct IdempotencyLayer {
    store: Arc<Mutex<IdempotencyStore>>,
    /// Commerce handle whose database backs the durable store, if any.
    durable: Option<Arc<Commerce>>,
    /// Counts durable writes to schedule opportunistic bulk expiry sweeps.
    write_count: Arc<AtomicU64>,
    /// Reject guarded money-moving creates that omit `Idempotency-Key` (428).
    require_keys: bool,
    ttl: Duration,
}

impl std::fmt::Debug for IdempotencyLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdempotencyLayer")
            .field("durable", &self.durable.is_some())
            .field("require_keys", &self.require_keys)
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl IdempotencyLayer {
    /// Create a memory-only layer with the default TTL and capacity.
    ///
    /// Used by the bare routers; [`crate::server::ServerBuilder`] upgrades this
    /// with a durable store and the required-key gate.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DEFAULT_TTL, DEFAULT_MAX_ENTRIES)
    }

    /// Create a layer with explicit TTL and in-memory capacity.
    fn with_config(ttl: Duration, max_entries: usize) -> Self {
        Self {
            store: Arc::new(Mutex::new(IdempotencyStore::new(ttl, max_entries))),
            durable: None,
            write_count: Arc::new(AtomicU64::new(0)),
            require_keys: false,
            ttl,
        }
    }

    /// Back the layer with the durable `http_idempotency_keys` store of the
    /// given commerce instance's database. The in-memory map becomes a
    /// read-through cache in front of it.
    #[must_use]
    pub fn with_durable_store(mut self, commerce: Arc<Commerce>) -> Self {
        self.durable = Some(commerce);
        self
    }

    /// Require `Idempotency-Key` on guarded money-moving create endpoints,
    /// rejecting bare requests with HTTP 428 (Precondition Required).
    #[must_use]
    pub const fn with_required_keys(mut self, require: bool) -> Self {
        self.require_keys = require;
        self
    }

    /// Earliest `created_at` still considered live at `now`.
    fn expiry_cutoff(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        chrono::Duration::from_std(self.ttl)
            .ok()
            .and_then(|ttl| now.checked_sub_signed(ttl))
            .unwrap_or(DateTime::<Utc>::MIN_UTC)
    }
}

impl Default for IdempotencyLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Collision-resistant fingerprint of the request: hex SHA-256 over
/// method, path, and body, used to detect when an `Idempotency-Key` is replayed
/// with a *different* request (a client conflict).
///
/// A non-cryptographic hash (e.g. FNV-1a) is unsuitable here: an attacker who
/// reuses a victim's key could grind a body that collides with the original and
/// thereby slip a different request past the "same key, different request"
/// guard (or have a cached response replayed for a request it never matched).
/// SHA-256 makes such a collision computationally infeasible.
fn request_fingerprint(method: &Method, path: &str, body: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(method.as_str().as_bytes());
    hasher.update(b"\n");
    hasher.update(path.as_bytes());
    hasher.update(b"\n");
    hasher.update(body);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
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
        | ["products"]
        // AP payment creates a new financial record.
        | ["ap", "payments"] => true,
        // Payment refund creates a new refund record.
        ["payments", _id, "refund"] => true,
        _ => false,
    }
}

/// Whether a `POST` path is a money-moving create that *requires* an
/// `Idempotency-Key` when the required-key gate is enabled.
fn requires_idempotency_key(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/api/v1/") else {
        return false;
    };
    let segments = rest.split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    matches!(
        segments.as_slice(),
        ["orders"] | ["payments"] | ["payments", _, "refund"] | ["ap", "payments"]
    )
}

/// HTTP 428 (Precondition Required) response in the standard error envelope.
fn precondition_required_response(path: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "code": "precondition_required",
            "message": format!(
                "Idempotency-Key header is required for POST {path}: supply a unique key per \
                 logical operation so safe retries can be replayed without duplicate side effects \
                 (server opt-out: require_idempotency_keys(false) / \
                 STATESET_HTTP_REQUIRE_IDEMPOTENCY_KEYS=false)"
            ),
        }
    });
    (StatusCode::PRECONDITION_REQUIRED, axum::Json(body)).into_response()
}

fn conflict_response() -> Response {
    HttpError::ValidationError(
        "idempotency-key already used with a different request body".to_string(),
    )
    .into_response()
}

/// Load a durable entry (off the async runtime); errors degrade to `None`.
async fn durable_get(
    commerce: Arc<Commerce>,
    tenant: String,
    key: String,
    cutoff: DateTime<Utc>,
) -> Option<HttpIdempotencyRecord> {
    let result = tokio::task::spawn_blocking(move || {
        commerce
            .database()
            .http_idempotency()
            .map(|repo| repo.get(&tenant, &key, cutoff))
            .transpose()
            .map(Option::flatten)
    })
    .await;
    match result {
        Ok(Ok(record)) => record,
        Ok(Err(error)) => {
            tracing::warn!(%error, "durable idempotency lookup failed; falling back to memory");
            None
        }
        Err(error) => {
            tracing::warn!(%error, "durable idempotency lookup task failed");
            None
        }
    }
}

/// Persist a durable entry and run an opportunistic expiry sweep.
async fn durable_put(layer: &IdempotencyLayer, record: HttpIdempotencyRecord) {
    let Some(commerce) = layer.durable.clone() else {
        return;
    };
    let sweep_cutoff =
        ((layer.write_count.fetch_add(1, Ordering::Relaxed) + 1) % SWEEP_EVERY_N_WRITES == 0)
            .then(|| layer.expiry_cutoff(Utc::now()));
    let result = tokio::task::spawn_blocking(move || {
        let db = commerce.database();
        let Some(repo) = db.http_idempotency() else {
            return Ok(());
        };
        repo.put(&record)?;
        if let Some(cutoff) = sweep_cutoff {
            repo.purge_expired(cutoff)?;
        }
        Ok::<_, stateset_core::CommerceError>(())
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::warn!(%error, "durable idempotency write failed; entry is memory-only");
        }
        Err(error) => {
            tracing::warn!(%error, "durable idempotency write task failed");
        }
    }
}

fn record_to_cached(record: HttpIdempotencyRecord) -> CachedResponse {
    CachedResponse {
        status: StatusCode::from_u16(record.response_status)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        content_type: record
            .content_type
            .as_deref()
            .and_then(|value| HeaderValue::from_str(value).ok()),
        body: record.response_body,
        request_fingerprint: record.request_fingerprint,
        created_at: record.created_at,
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
    let path = request.uri().path().to_owned();

    // Extract the idempotency key; absent key disables caching for this request
    // unless the endpoint is money-moving and required keys are enabled.
    let key = request
        .headers()
        .get(&IDEMPOTENCY_KEY)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let Some(key) = key else {
        if layer.require_keys && requires_idempotency_key(&path) {
            return precondition_required_response(&path);
        }
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
    let cache_key = (tenant.clone(), key.clone());

    // Buffer the request body so we can hash it and replay the handler with it.
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return HttpError::BadRequest("request body too large for idempotency".to_string())
                .into_response();
        }
    };
    let fingerprint = request_fingerprint(&parts.method, &path, &body_bytes);
    let now = Utc::now();

    // Read-through lookup: memory first, then the durable store.
    let mut cached = {
        let mut store = layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.get(&cache_key, now)
    };
    if cached.is_none()
        && let Some(commerce) = layer.durable.clone()
    {
        let record =
            durable_get(commerce, tenant.clone(), key.clone(), layer.expiry_cutoff(now)).await;
        if let Some(record) = record {
            let entry = record_to_cached(record);
            let mut store = layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            store.insert(cache_key.clone(), entry.clone());
            cached = Some(entry);
        }
    }
    if let Some(cached) = cached {
        if cached.request_fingerprint == fingerprint {
            return replay_response(&cached);
        }
        return conflict_response();
    }

    // Miss: run the inner handler with the buffered body restored.
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
        request_fingerprint: fingerprint.clone(),
        created_at: now,
    };
    {
        let mut store = layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.insert(cache_key, cached.clone());
    }
    durable_put(
        &layer,
        HttpIdempotencyRecord {
            tenant,
            idempotency_key: key,
            request_fingerprint: fingerprint,
            response_status: cached.status.as_u16(),
            content_type: cached
                .content_type
                .as_ref()
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            response_body: cached.body,
            created_at: now,
        },
    )
    .await;

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
        app_at("/api/v1/orders", layer, counter)
    }

    fn app_at(path: &str, layer: IdempotencyLayer, counter: Arc<AtomicU64>) -> Router {
        Router::new()
            .route(
                path,
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

    fn durable_layer(commerce: &Arc<Commerce>) -> IdempotencyLayer {
        IdempotencyLayer::new().with_durable_store(commerce.clone())
    }

    #[test]
    fn is_idempotent_post_path_matches_create_endpoints() {
        assert!(is_idempotent_post_path("/api/v1/orders"));
        assert!(is_idempotent_post_path("/api/v1/payments"));
        assert!(is_idempotent_post_path("/api/v1/returns"));
        assert!(is_idempotent_post_path("/api/v1/payments/abc/refund"));
        assert!(is_idempotent_post_path("/api/v1/ap/payments"));
        // Action routes that mutate existing resources are excluded.
        assert!(!is_idempotent_post_path("/api/v1/payments/abc/complete"));
        assert!(!is_idempotent_post_path("/api/v1/orders/abc/cancel"));
        // Non-API paths never participate.
        assert!(!is_idempotent_post_path("/health"));
    }

    #[test]
    fn requires_idempotency_key_matches_money_moving_creates() {
        assert!(requires_idempotency_key("/api/v1/orders"));
        assert!(requires_idempotency_key("/api/v1/payments"));
        assert!(requires_idempotency_key("/api/v1/payments/abc/refund"));
        assert!(requires_idempotency_key("/api/v1/ap/payments"));
        // Non-money creates stay optional.
        assert!(!requires_idempotency_key("/api/v1/customers"));
        assert!(!requires_idempotency_key("/api/v1/products"));
        assert!(!requires_idempotency_key("/api/v1/returns"));
    }

    #[test]
    fn store_evicts_oldest_when_over_capacity() {
        let mut store = IdempotencyStore::new(DEFAULT_TTL, 2);
        let now = Utc::now();
        let make = |seed: u8| CachedResponse {
            status: StatusCode::CREATED,
            content_type: None,
            body: Vec::new(),
            request_fingerprint: format!("{seed:064}"),
            created_at: now,
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
        let created_at = Utc::now();
        store.insert(
            ("t".into(), "a".into()),
            CachedResponse {
                status: StatusCode::CREATED,
                content_type: None,
                body: Vec::new(),
                request_fingerprint: "f".repeat(64),
                created_at,
            },
        );
        let later = created_at + chrono::Duration::seconds(11);
        assert!(store.get(&("t".into(), "a".into()), later).is_none());
    }

    #[test]
    fn fingerprint_covers_method_path_and_body() {
        let base = request_fingerprint(&Method::POST, "/api/v1/orders", b"{}");
        assert_ne!(base, request_fingerprint(&Method::PUT, "/api/v1/orders", b"{}"));
        assert_ne!(base, request_fingerprint(&Method::POST, "/api/v1/payments", b"{}"));
        assert_ne!(base, request_fingerprint(&Method::POST, "/api/v1/orders", b"{ }"));
        assert_eq!(base, request_fingerprint(&Method::POST, "/api/v1/orders", b"{}"));
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

    #[tokio::test]
    async fn missing_key_on_guarded_route_returns_428_when_required() {
        for path in [
            "/api/v1/orders",
            "/api/v1/payments",
            "/api/v1/payments/abc/refund",
            "/api/v1/ap/payments",
        ] {
            let counter = Arc::new(AtomicU64::new(0));
            let layer = IdempotencyLayer::new().with_required_keys(true);
            let app = app_at(path, layer, counter.clone());

            let resp = app
                .clone()
                .oneshot(Request::post(path).body(Body::from("{}")).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::PRECONDITION_REQUIRED, "path {path}");
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(json["error"]["code"], "precondition_required");
            // The handler never ran.
            assert_eq!(counter.load(Ordering::SeqCst), 0);

            // With a key the request proceeds normally.
            let ok = app
                .oneshot(
                    Request::post(path)
                        .header("idempotency-key", "k1")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(ok.status(), StatusCode::CREATED, "path {path}");
        }
    }

    #[tokio::test]
    async fn missing_key_allowed_on_guarded_route_when_requirement_disabled() {
        let counter = Arc::new(AtomicU64::new(0));
        let layer = IdempotencyLayer::new().with_required_keys(false);
        let app = app_at("/api/v1/payments", layer, counter.clone());

        let resp = app
            .oneshot(Request::post("/api/v1/payments").body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn missing_key_on_unguarded_create_never_428s() {
        let counter = Arc::new(AtomicU64::new(0));
        let layer = IdempotencyLayer::new().with_required_keys(true);
        let app = app_at("/api/v1/customers", layer, counter.clone());

        let resp = app
            .oneshot(Request::post("/api/v1/customers").body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn durable_replay_survives_restart_with_same_database() {
        // Two layers over the same database file simulate a process restart
        // (fresh in-memory cache, same durable store).
        let db_path =
            std::env::temp_dir().join(format!("stateset-idem-restart-{}.db", uuid::Uuid::new_v4()));
        let db_path_str = db_path.to_str().unwrap().to_owned();

        let body = "{\"customer\":\"c1\"}";
        let first_body_bytes;
        {
            let commerce = Arc::new(Commerce::new(&db_path_str).unwrap());
            let counter = Arc::new(AtomicU64::new(0));
            let app = app(durable_layer(&commerce), counter.clone());
            let first = app
                .oneshot(
                    Request::post("/api/v1/orders")
                        .header("idempotency-key", "restart-key")
                        .header("x-tenant-id", "tenant-a")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(first.status(), StatusCode::CREATED);
            assert_eq!(counter.load(Ordering::SeqCst), 1);
            first_body_bytes = axum::body::to_bytes(first.into_body(), usize::MAX).await.unwrap();
        }

        // "Restart": a brand-new Commerce + layer over the same file.
        let commerce = Arc::new(Commerce::new(&db_path_str).unwrap());
        let counter = Arc::new(AtomicU64::new(0));
        let app = app(durable_layer(&commerce), counter.clone());

        let replay = app
            .clone()
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "restart-key")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::CREATED);
        assert_eq!(
            replay.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
            Some("true"),
            "replay after restart must come from the durable store"
        );
        let replay_body = axum::body::to_bytes(replay.into_body(), usize::MAX).await.unwrap();
        assert_eq!(first_body_bytes, replay_body);
        // The handler never ran in the "restarted" process.
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Same key, different body after restart → 422 from the durable record.
        let conflict = app
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "restart-key")
                    .header("x-tenant-id", "tenant-a")
                    .body(Body::from("{\"customer\":\"DIFFERENT\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn durable_entries_expire_after_ttl() {
        let commerce = Arc::new(Commerce::in_memory().unwrap());
        // Zero TTL: every stored entry is immediately expired, so both memory
        // and durable lookups must treat it as absent and rerun the handler.
        let layer = IdempotencyLayer::with_config(Duration::ZERO, DEFAULT_MAX_ENTRIES)
            .with_durable_store(commerce.clone());
        let counter = Arc::new(AtomicU64::new(0));
        let app = app(layer, counter.clone());

        for _ in 0..2 {
            let resp = app
                .clone()
                .oneshot(
                    Request::post("/api/v1/orders")
                        .header("idempotency-key", "ttl-key")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
            assert_eq!(
                resp.headers().get("idempotency-replayed").and_then(|v| v.to_str().ok()),
                Some("false"),
                "expired entries must never replay"
            );
        }
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
