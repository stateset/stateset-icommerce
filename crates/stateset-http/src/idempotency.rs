//! Idempotency-Key middleware for REST mutation endpoints.
//!
//! Honors an `Idempotency-Key` request header on `POST` create endpoints
//! (orders, payments, refunds, returns, shipment items, …) and shipment-item
//! `DELETE` endpoints. The first request for a
//! `(tenant, key)` pair runs the handler and stores the response (status code,
//! body bytes, content type, and a fingerprint of the request). Subsequent
//! requests with the same key:
//!
//! - **identical request fingerprint** → the stored response is replayed
//!   verbatim with an `Idempotency-Replayed: true` header and **no duplicate
//!   resource** is created;
//! - **different fingerprint** (method + path/query + body hash) → the request is
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
//! database. Before calling a handler, the layer atomically reserves the key
//! in the shared database. Only the winning request executes. A reservation
//! without a saved response returns HTTP 409 on retries and never expires:
//! after a crash the outcome is uncertain and must be reconciled by an
//! operator, rather than risking a second mutation. Completed responses expire
//! normally. Store failures fail closed with HTTP 503.
//! This provides at-most-once execution, not automatic recovery of a response
//! lost between the business commit and response persistence.
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
use std::sync::{Arc, Mutex, Weak};
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
type CacheKey = (String, String);
type KeyLock = tokio::sync::Mutex<()>;
type InflightRegistry = Arc<Mutex<HashMap<CacheKey, Weak<KeyLock>>>>;

/// A cached idempotent response plus the fingerprint of the originating request.
#[derive(Clone, Debug)]
struct CachedResponse {
    status: StatusCode,
    content_type: Option<HeaderValue>,
    body: Vec<u8>,
    /// Hex SHA-256 of method + path including query + request body.
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
            self.order.retain(|queued| queued != key);
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
    /// Serialize lookups and handler execution for a key within this process.
    inflight: InflightRegistry,
    /// Commerce handle whose database backs the durable store, if any.
    durable: Option<Arc<Commerce>>,
    /// Resolve the same database as the route, including tenant routing.
    routing_state: Option<crate::AppState>,
    /// Counts durable writes to schedule opportunistic bulk expiry sweeps.
    write_count: Arc<AtomicU64>,
    /// Reject guarded money-moving creates that omit `Idempotency-Key` (428).
    require_keys: bool,
    ttl: Duration,
}

impl std::fmt::Debug for IdempotencyLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdempotencyLayer")
            .field("durable", &(self.durable.is_some() || self.routing_state.is_some()))
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
            inflight: Arc::new(Mutex::new(HashMap::new())),
            durable: None,
            routing_state: None,
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
        self.routing_state = None;
        self
    }

    /// Keep reservations beside the business data, including tenant stores.
    pub(crate) fn with_app_state(mut self, state: crate::AppState) -> Self {
        self.routing_state = Some(state);
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

    async fn lock_key(&self, key: &CacheKey) -> InflightKeyGuard {
        let key_lock = {
            let mut registry =
                self.inflight.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            // Cancelled waiters can leave weak entries behind. Sweep stale
            // entries periodically without scanning all active keys on every
            // request during a concurrency spike.
            if registry.len() >= 1024 && registry.len() % 512 == 0 {
                registry.retain(|_, lock| lock.strong_count() > 0);
            }
            if let Some(existing) = registry.get(key).and_then(Weak::upgrade) {
                existing
            } else {
                let lock = Arc::new(KeyLock::new(()));
                registry.insert(key.clone(), Arc::downgrade(&lock));
                lock
            }
        };
        let guard = key_lock.clone().lock_owned().await;
        InflightKeyGuard {
            _guard: Some(guard),
            key_lock,
            key: key.clone(),
            registry: self.inflight.clone(),
        }
    }
}

struct InflightKeyGuard {
    _guard: Option<tokio::sync::OwnedMutexGuard<()>>,
    key_lock: Arc<KeyLock>,
    key: CacheKey,
    registry: InflightRegistry,
}

impl Drop for InflightKeyGuard {
    fn drop(&mut self) {
        // Release the async lock before looking for another holder. Waiters
        // own an Arc, so the key remains registered while any one waits.
        drop(self._guard.take());
        let mut registry = self.registry.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if Arc::strong_count(&self.key_lock) == 1
            && registry
                .get(&self.key)
                .is_some_and(|entry| entry.ptr_eq(&Arc::downgrade(&self.key_lock)))
        {
            registry.remove(&self.key);
        }
    }
}

impl Default for IdempotencyLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Collision-resistant fingerprint of the request: hex SHA-256 over
/// method, path including query, and body, used to detect when an `Idempotency-Key` is replayed
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
        ["payments", _id, "refund"] | ["shipments", _id, "items"] => true,
        _ => false,
    }
}

fn is_idempotent_endpoint(method: &Method, path: &str) -> bool {
    if *method == Method::POST {
        return is_idempotent_post_path(path);
    }
    if *method != Method::DELETE {
        return false;
    }
    let Some(rest) = path.strip_prefix("/api/v1/") else {
        return false;
    };
    let segments = rest.split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    matches!(segments.as_slice(), ["shipments", _, "items", _])
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

fn unavailable_response() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, axum::Json(serde_json::json!({
        "error": { "code": "idempotency_unavailable", "message": "durable idempotency store unavailable; retry with the same key" }
    }))).into_response()
}

fn pending_response() -> Response {
    (StatusCode::CONFLICT, axum::Json(serde_json::json!({
        "error": { "code": "idempotency_in_progress", "message": "request is in progress or its outcome requires reconciliation; do not retry with a new key" }
    }))).into_response()
}

async fn durable_get(
    commerce: Arc<Commerce>,
    tenant: String,
    key: String,
    cutoff: DateTime<Utc>,
) -> Result<Option<HttpIdempotencyRecord>, String> {
    tokio::task::spawn_blocking(move || {
        let db = commerce.database();
        let repo =
            db.http_idempotency().ok_or_else(|| "backend has no idempotency store".to_string())?;
        repo.get(&tenant, &key, cutoff).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn durable_claim(
    commerce: Arc<Commerce>,
    record: HttpIdempotencyRecord,
) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        let db = commerce.database();
        let repo =
            db.http_idempotency().ok_or_else(|| "backend has no idempotency store".to_string())?;
        repo.put(&record).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Complete the previously acquired reservation before populating the cache.
async fn durable_put(
    layer: &IdempotencyLayer,
    record: HttpIdempotencyRecord,
) -> Result<(), String> {
    let Some(commerce) = layer.durable.clone() else {
        return Ok(());
    };
    let sweep_cutoff =
        ((layer.write_count.fetch_add(1, Ordering::Relaxed) + 1) % SWEEP_EVERY_N_WRITES == 0)
            .then(|| layer.expiry_cutoff(Utc::now()));
    tokio::task::spawn_blocking(move || {
        let db = commerce.database();
        let repo =
            db.http_idempotency().ok_or_else(|| "backend has no idempotency store".to_string())?;
        if !repo.complete(&record).map_err(|e| e.to_string())? {
            return Err("idempotency reservation was not completed".into());
        }
        if let Some(cutoff) = sweep_cutoff {
            if let Err(error) = repo.purge_expired(cutoff) {
                tracing::warn!(%error, "idempotency expiry sweep failed");
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
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

/// Axum middleware enforcing `Idempotency-Key` semantics on selected mutations.
pub(crate) async fn idempotency(
    State(mut layer): State<IdempotencyLayer>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !is_idempotent_endpoint(request.method(), request.uri().path()) {
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

    // Authentication has already bound the tenant header. Resolve it with the
    // same rules as route handlers before consulting even the memory cache.
    // A tenant deployment has no default tenant: resolving None at startup
    // would silently downgrade every request to memory-only idempotency.
    if let Some(state) = layer.routing_state.take() {
        let tenant_id = (!tenant.is_empty()).then(|| tenant.clone());
        match tokio::task::spawn_blocking(move || state.commerce_for_tenant(tenant_id.as_deref()))
            .await
        {
            Ok(Ok(commerce)) => layer.durable = Some(commerce),
            Ok(Err(error)) => return error.into_response(),
            Err(error) => {
                tracing::warn!(%error, "idempotency tenant resolution failed");
                return unavailable_response();
            }
        }
    }

    // Buffer the request body so we can hash it and replay the handler with it.
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return HttpError::BadRequest("request body too large for idempotency".to_string())
                .into_response();
        }
    };
    // Query parameters can carry mutation preconditions. Bind their exact spelling
    // as well as the path; changing a precondition must never replay old success.
    let target = parts.uri.path_and_query().map_or(path.as_str(), |value| value.as_str());
    let fingerprint = request_fingerprint(&parts.method, target, &body_bytes);
    // A second request must re-check the cache after the first finishes. The
    // lock spans durable lookup, handler execution, and response persistence.
    let _inflight_guard = layer.lock_key(&cache_key).await;
    let now = Utc::now();

    // Read-through lookup: memory first, then the durable store.
    let mut cached = {
        let mut store = layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.get(&cache_key, now)
    };
    if cached.is_none()
        && let Some(commerce) = layer.durable.clone()
    {
        let record = match durable_get(
            commerce,
            tenant.clone(),
            key.clone(),
            layer.expiry_cutoff(now),
        )
        .await
        {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!(%error, "idempotency lookup failed");
                return unavailable_response();
            }
        };
        if let Some(record) = record {
            if record.request_fingerprint != fingerprint {
                return conflict_response();
            }
            if record.response_status == 0 {
                return pending_response();
            }
            let entry = record_to_cached(record);
            layer
                .store
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(cache_key.clone(), entry.clone());
            cached = Some(entry);
        }
    }
    if let Some(cached) = cached {
        if cached.request_fingerprint == fingerprint {
            return replay_response(&cached);
        }
        return conflict_response();
    }

    if let Some(commerce) = layer.durable.clone() {
        let reservation = HttpIdempotencyRecord {
            tenant: tenant.clone(),
            idempotency_key: key.clone(),
            request_fingerprint: fingerprint.clone(),
            response_status: 0,
            content_type: None,
            response_body: Vec::new(),
            created_at: now,
        };
        match durable_claim(commerce.clone(), reservation).await {
            Ok(true) => {}
            Ok(false) => {
                // Another replica owns this key. Never execute a second handler.
                return match durable_get(
                    commerce,
                    tenant.clone(),
                    key.clone(),
                    layer.expiry_cutoff(now),
                )
                .await
                {
                    Ok(Some(record)) if record.request_fingerprint != fingerprint => {
                        conflict_response()
                    }
                    Ok(Some(record)) if record.response_status != 0 => {
                        replay_response(&record_to_cached(record))
                    }
                    Ok(_) => pending_response(),
                    Err(_) => unavailable_response(),
                };
            }
            Err(error) => {
                tracing::warn!(%error, "idempotency reservation failed");
                return unavailable_response();
            }
        }
    }

    // Miss: run the inner handler with the buffered body restored.
    let request = Request::from_parts(parts, Body::from(body_bytes));
    let response = next.run(request).await;

    // Only cache deterministic, successfully-produced responses. Server errors
    // (5xx) may follow a committed mutation. Durable reservations remain
    // unresolved in that case; only memory-only mode permits another attempt.
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
    let now = Utc::now();
    let cached = CachedResponse {
        status: resp_parts.status,
        content_type,
        body: resp_bytes.to_vec(),
        request_fingerprint: fingerprint.clone(),
        created_at: now,
    };
    if let Err(error) = durable_put(
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
            response_body: cached.body.clone(),
            created_at: now,
        },
    )
    .await
    {
        tracing::warn!(%error, "idempotency completion failed; reservation retained");
        return unavailable_response();
    }
    layer.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(cache_key, cached);

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
    fn expired_cache_entries_do_not_accumulate_eviction_metadata() {
        let mut store = IdempotencyStore::new(Duration::ZERO, 1);
        let key = (String::new(), "reused".into());
        let now = Utc::now();
        for _ in 0..100 {
            store.insert(
                key.clone(),
                CachedResponse {
                    status: StatusCode::CREATED,
                    content_type: None,
                    body: Vec::new(),
                    request_fingerprint: "request".into(),
                    created_at: now,
                },
            );
            assert!(store.get(&key, now).is_none());
            assert!(store.order.is_empty());
        }
    }

    #[tokio::test]
    async fn independent_replicas_execute_a_key_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("replicas.db");
        let first = Arc::new(Commerce::new(path.to_str().unwrap()).unwrap());
        let second = Arc::new(Commerce::new(path.to_str().unwrap()).unwrap());
        let counter = Arc::new(AtomicU64::new(0));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let make_app = |commerce: &Arc<Commerce>| {
            let counter = counter.clone();
            let started = started.clone();
            let release = release.clone();
            Router::new()
                .route(
                    "/api/v1/orders",
                    post(move || {
                        let counter = counter.clone();
                        let started = started.clone();
                        let release = release.clone();
                        async move {
                            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                            if n == 1 {
                                started.notify_one();
                                release.notified().await;
                            }
                            (StatusCode::CREATED, format!("order-{n}"))
                        }
                    }),
                )
                .layer(axum::middleware::from_fn_with_state(durable_layer(commerce), idempotency))
        };
        let a = make_app(&first);
        let b = make_app(&second);
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/api/v1/orders")
                .header("idempotency-key", "replica-race")
                .body(Body::from("{}"))
                .unwrap()
        };
        // Hold the first handler before response persistence. Without a durable
        // reservation, the other replica would execute another mutation here.
        let first_request = tokio::spawn(a.oneshot(request()));
        tokio::time::timeout(Duration::from_secs(30), started.notified()).await.unwrap();
        let second_response = b.clone().oneshot(request()).await.unwrap();
        release.notify_one();
        assert_eq!(first_request.await.unwrap().unwrap().status(), StatusCode::CREATED);
        assert_eq!(second_response.status(), StatusCode::CONFLICT);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        let replay = b.oneshot(request()).await.unwrap();
        assert!(replay.status().is_success());
        assert_eq!(replay.headers()["idempotency-replayed"], "true");
    }

    #[tokio::test]
    async fn abandoned_claim_blocks_reexecution_even_after_ttl() {
        let commerce = Arc::new(Commerce::new(":memory:").unwrap());
        commerce
            .database()
            .http_idempotency()
            .unwrap()
            .put(&HttpIdempotencyRecord {
                tenant: String::new(),
                idempotency_key: "crashed".into(),
                request_fingerprint: request_fingerprint(&Method::POST, "/api/v1/orders", b"{}"),
                response_status: 0,
                content_type: None,
                response_body: vec![],
                created_at: Utc::now() - chrono::Duration::days(30),
            })
            .unwrap();
        let counter = Arc::new(AtomicU64::new(0));
        let app = app(durable_layer(&commerce), counter.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/orders")
                    .header("idempotency-key", "crashed")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unavailable_store_prevents_handler_execution() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unavailable.db");
        let commerce = Arc::new(Commerce::new(path.to_str().unwrap()).unwrap());
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch("DROP TABLE http_idempotency_keys")
            .unwrap();
        let counter = Arc::new(AtomicU64::new(0));
        let response = app(durable_layer(&commerce), counter.clone())
            .oneshot(
                Request::post("/api/v1/orders")
                    .header("idempotency-key", "unavailable")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn response_persistence_failure_retains_reservation_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("completion-failure.db");
        let commerce = Arc::new(Commerce::new(path.to_str().unwrap()).unwrap());
        // Simulate a failure after the handler has performed its side effect.
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_completion BEFORE UPDATE ON http_idempotency_keys
             BEGIN SELECT RAISE(ABORT, 'injected completion failure'); END;",
            )
            .unwrap();
        let counter = Arc::new(AtomicU64::new(0));
        let request = || {
            Request::post("/api/v1/orders")
                .header("idempotency-key", "uncertain")
                .body(Body::from("{}"))
                .unwrap()
        };
        let response =
            app(durable_layer(&commerce), counter.clone()).oneshot(request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        drop(commerce);
        let restarted = Arc::new(Commerce::new(path.to_str().unwrap()).unwrap());
        let response =
            app(durable_layer(&restarted), counter.clone()).oneshot(request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn is_idempotent_post_path_matches_create_endpoints() {
        assert!(is_idempotent_post_path("/api/v1/orders"));
        assert!(is_idempotent_post_path("/api/v1/payments"));
        assert!(is_idempotent_post_path("/api/v1/returns"));
        assert!(is_idempotent_post_path("/api/v1/payments/abc/refund"));
        assert!(is_idempotent_post_path("/api/v1/ap/payments"));
        assert!(is_idempotent_endpoint(&Method::POST, "/api/v1/shipments/abc/items"));
        assert!(is_idempotent_endpoint(&Method::DELETE, "/api/v1/shipments/abc/items/def"));
        assert!(!is_idempotent_endpoint(&Method::GET, "/api/v1/shipments/abc/items/def"));
        assert!(!is_idempotent_endpoint(&Method::DELETE, "/api/v1/shipments/abc"));
        assert!(!is_idempotent_endpoint(&Method::DELETE, "/api/v1/orders/abc/items/def"));
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_requests_for_one_key_run_handler_once() {
        let counter = Arc::new(AtomicU64::new(0));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let app = Router::new()
            .route(
                "/api/v1/orders",
                post({
                    let counter = counter.clone();
                    let started = started.clone();
                    let release = release.clone();
                    move || {
                        let counter = counter.clone();
                        let started = started.clone();
                        let release = release.clone();
                        async move {
                            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                            started.notify_one();
                            release.notified().await;
                            (StatusCode::CREATED, format!("order-{n}"))
                        }
                    }
                }),
            )
            .layer(axum::middleware::from_fn_with_state(IdempotencyLayer::new(), idempotency));
        let request = || {
            Request::post("/api/v1/orders")
                .header("idempotency-key", "same-key")
                .body(Body::from("{}"))
                .unwrap()
        };
        let first = tokio::spawn(app.clone().oneshot(request()));
        tokio::time::timeout(Duration::from_secs(5), started.notified()).await.unwrap();
        let second = tokio::spawn(app.oneshot(request()));
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(!second.is_finished());
        release.notify_waiters();
        let first =
            tokio::time::timeout(Duration::from_secs(5), first).await.unwrap().unwrap().unwrap();
        let second =
            tokio::time::timeout(Duration::from_secs(5), second).await.unwrap().unwrap().unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(first.headers()["idempotency-replayed"], "false");
        assert_eq!(second.headers()["idempotency-replayed"], "true");
        assert_eq!(
            to_bytes(first.into_body(), usize::MAX).await.unwrap(),
            to_bytes(second.into_body(), usize::MAX).await.unwrap()
        );
    }

    #[tokio::test]
    async fn distinct_keys_have_independent_locks_and_cleanup() {
        let layer = IdempotencyLayer::new();
        let first_key = ("tenant".to_string(), "first".to_string());
        let second_key = ("tenant".to_string(), "second".to_string());
        let first = layer.lock_key(&first_key).await;
        let second = tokio::time::timeout(Duration::from_millis(100), layer.lock_key(&second_key))
            .await
            .expect("a distinct key must not wait for the first handler");
        assert_eq!(layer.inflight.lock().unwrap().len(), 2);
        drop(first);
        drop(second);
        assert!(layer.inflight.lock().unwrap().is_empty());
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
