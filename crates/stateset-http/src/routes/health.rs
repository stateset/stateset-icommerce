//! Health-check endpoints.

use std::{
    fmt::Write,
    net::{IpAddr, SocketAddr},
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Request, State},
    http::{
        HeaderMap, StatusCode,
        header::{self, AUTHORIZATION, HeaderName},
    },
    response::IntoResponse,
    routing::get,
};

use crate::dto::{HealthResponse, ReadyResponse, TenantCacheResponse};
use crate::error::HttpError;
use crate::state::{AppState, MetricsHeaderLimits, TenantCacheMetrics};

/// Build the health-check router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
        .route("/health/deep", get(deep_health))
        .route("/metrics", get(metrics))
}

/// `GET /health` — simple liveness probe.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is alive", body = HealthResponse),
    )
)]
#[tracing::instrument(skip(state))]
pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok", tenant_cache: tenant_cache_response(&state) })
}

/// `GET /health/ready` — readiness probe that checks DB connectivity.
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "Service is ready", body = ReadyResponse),
        (status = 503, description = "Service is not ready", body = ReadyResponse),
    )
)]
#[tracing::instrument(skip(state))]
pub(crate) async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadyResponse>) {
    // Try a lightweight operation to verify DB is reachable.
    let database_connected = state.commerce().orders().count(Default::default()).is_ok();
    let tenant_cache = tenant_cache_response(&state);
    let (status, body) = readiness_response(database_connected, tenant_cache);
    (status, Json(body))
}

/// `GET /health/deep` — deep health check with DB connectivity and metrics.
#[tracing::instrument(skip(state))]
pub(crate) async fn deep_health(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, HttpError> {
    // Verify DB connectivity by executing a trivial query
    let start = std::time::Instant::now();
    let db_ok = state.commerce().orders().count(Default::default()).is_ok();
    let db_latency_ms = start.elapsed().as_millis() as u64;

    let metrics = state.commerce().metrics_snapshot();

    if !db_ok {
        return Err(HttpError::InternalError("Database connectivity check failed".into()));
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "database": {
            "connected": true,
            "latency_ms": db_latency_ms,
        },
        "metrics": {
            "orders_created": metrics.orders_created,
            "customers_created": metrics.customers_created,
            "products_created": metrics.products_created,
            "payments_completed": metrics.payments_completed,
        }
    })))
}

/// `GET /metrics` — Prometheus-compatible operational metrics.
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "health",
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain"),
        (status = 401, description = "Missing or invalid metrics bearer token"),
        (status = 403, description = "Metrics request IP is not allowed"),
    )
)]
#[tracing::instrument(skip(state))]
pub(crate) async fn metrics(
    State(state): State<AppState>,
    request: Request,
) -> Result<impl IntoResponse, HttpError> {
    let peer_ip = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|info| info.0.ip());
    state.record_metrics_scrape_attempt();
    let access = match require_metrics_access(&state, request.headers(), peer_ip) {
        Ok(access) => access,
        Err(error) => {
            let reason = error.reason;
            match reason {
                MetricsAccessFailureReason::AuthHeaderMissing => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_header_missing();
                }
                MetricsAccessFailureReason::AuthHeaderInvalidEncoding => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_header_invalid();
                    state.record_metrics_scrape_denied_auth_header_invalid_encoding();
                }
                MetricsAccessFailureReason::AuthHeaderInvalidScheme => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_header_invalid();
                    state.record_metrics_scrape_denied_auth_header_invalid_scheme();
                }
                MetricsAccessFailureReason::AuthHeaderMalformed => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_header_invalid();
                    state.record_metrics_scrape_denied_auth_header_malformed();
                }
                MetricsAccessFailureReason::AuthHeaderMultiple => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_header_multiple();
                }
                MetricsAccessFailureReason::AuthHeaderOversized => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_header_oversized();
                }
                MetricsAccessFailureReason::AuthTokenMismatch => {
                    state.record_metrics_scrape_denied_auth();
                    state.record_metrics_scrape_denied_auth_token_mismatch();
                }
                MetricsAccessFailureReason::IpNotAllowed => {
                    state.record_metrics_scrape_denied_ip();
                    state.record_metrics_scrape_denied_ip_not_allowed();
                }
                MetricsAccessFailureReason::MissingPeerIpWithTrustedProxies => {
                    state.record_metrics_scrape_denied_ip();
                    state.record_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies();
                }
                MetricsAccessFailureReason::ForwardedHeadersMissing
                | MetricsAccessFailureReason::ForwardedHeadersInvalid
                | MetricsAccessFailureReason::ForwardedHeadersOversized => {
                    state.record_metrics_scrape_denied_ip();
                }
            }
            tracing::warn!(
                reason = reason.as_str(),
                peer_ip = ?peer_ip,
                trusted_proxy_mode = state.has_metrics_trusted_proxies(),
                ip_allowlist_enabled = state.has_metrics_ip_allowlist(),
                auth_enabled = state.metrics_bearer_auth_token().is_some(),
                "metrics access denied"
            );
            return Err(error.into_http_error());
        }
    };
    state.record_metrics_scrape_allowed();
    match access.client_ip_source {
        MetricsClientIpSource::Peer => state.record_metrics_scrape_allowed_peer(),
        MetricsClientIpSource::ForwardedTrustedProxy => {
            state.record_metrics_scrape_allowed_forwarded_trusted_proxy();
        }
        MetricsClientIpSource::ForwardedWithoutPeer => {
            state.record_metrics_scrape_allowed_forwarded_without_peer();
        }
        MetricsClientIpSource::Unavailable => state.record_metrics_scrape_allowed_unavailable(),
    }
    tracing::debug!(
        peer_ip = ?peer_ip,
        client_ip = ?access.client_ip,
        client_ip_source = access.client_ip_source.as_str(),
        trusted_proxy_mode = state.has_metrics_trusted_proxies(),
        ip_allowlist_enabled = state.has_metrics_ip_allowlist(),
        auth_enabled = state.metrics_bearer_auth_token().is_some(),
        "metrics access granted"
    );

    Ok((
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        prometheus_metrics(&state),
    ))
}

const fn readiness_response(
    database_connected: bool,
    tenant_cache: TenantCacheResponse,
) -> (StatusCode, ReadyResponse) {
    if database_connected {
        (StatusCode::OK, ReadyResponse { status: "ok", database: "connected", tenant_cache })
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            ReadyResponse { status: "not_ready", database: "disconnected", tenant_cache },
        )
    }
}

fn tenant_cache_response(state: &AppState) -> TenantCacheResponse {
    let metrics: TenantCacheMetrics = state.tenant_cache_metrics();
    TenantCacheResponse {
        enabled: metrics.enabled,
        max_cached_dbs: metrics.max_cached_dbs,
        cached_dbs: metrics.cached_dbs,
        in_use_cached_dbs: metrics.in_use_cached_dbs,
        hits: metrics.hits,
        misses: metrics.misses,
        evictions: metrics.evictions,
        rejections: metrics.rejections,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricsAccessFailureReason {
    IpNotAllowed,
    MissingPeerIpWithTrustedProxies,
    ForwardedHeadersMissing,
    ForwardedHeadersInvalid,
    ForwardedHeadersOversized,
    AuthHeaderMissing,
    AuthHeaderInvalidEncoding,
    AuthHeaderInvalidScheme,
    AuthHeaderMalformed,
    AuthHeaderMultiple,
    AuthHeaderOversized,
    AuthTokenMismatch,
}

impl MetricsAccessFailureReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::IpNotAllowed => "ip_not_allowed",
            Self::MissingPeerIpWithTrustedProxies => "missing_peer_ip_with_trusted_proxies",
            Self::ForwardedHeadersMissing => "forwarded_headers_missing",
            Self::ForwardedHeadersInvalid => "forwarded_headers_invalid",
            Self::ForwardedHeadersOversized => "forwarded_headers_oversized",
            Self::AuthHeaderMissing => "auth_header_missing",
            Self::AuthHeaderInvalidEncoding => "auth_header_invalid_encoding",
            Self::AuthHeaderInvalidScheme => "auth_header_invalid_scheme",
            Self::AuthHeaderMalformed => "auth_header_malformed",
            Self::AuthHeaderMultiple => "auth_header_multiple",
            Self::AuthHeaderOversized => "auth_header_oversized",
            Self::AuthTokenMismatch => "auth_token_mismatch",
        }
    }
}

#[derive(Debug)]
struct MetricsAccessError {
    reason: MetricsAccessFailureReason,
    http_error: HttpError,
}

impl MetricsAccessError {
    fn forbidden(reason: MetricsAccessFailureReason, message: impl Into<String>) -> Self {
        Self { reason, http_error: HttpError::Forbidden(message.into()) }
    }

    fn unauthorized(reason: MetricsAccessFailureReason, message: impl Into<String>) -> Self {
        Self { reason, http_error: HttpError::Unauthorized(message.into()) }
    }

    fn into_http_error(self) -> HttpError {
        self.http_error
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricsClientIpSource {
    Peer,
    ForwardedTrustedProxy,
    ForwardedWithoutPeer,
    Unavailable,
}

impl MetricsClientIpSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Peer => "peer",
            Self::ForwardedTrustedProxy => "forwarded_trusted_proxy",
            Self::ForwardedWithoutPeer => "forwarded_without_peer",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MetricsAccessContext {
    client_ip: Option<IpAddr>,
    client_ip_source: MetricsClientIpSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedClientIp {
    ip: IpAddr,
    source: MetricsClientIpSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricsAuthHeaderParseFailure {
    Missing,
    InvalidEncoding,
    InvalidScheme,
    Malformed,
    Multiple,
    Oversized,
}

fn require_metrics_access(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
) -> Result<MetricsAccessContext, MetricsAccessError> {
    let mut context = MetricsAccessContext {
        client_ip: None,
        client_ip_source: MetricsClientIpSource::Unavailable,
    };
    if state.has_metrics_ip_allowlist() {
        let client = resolve_metrics_client_ip(state, headers, peer_ip)?;
        context =
            MetricsAccessContext { client_ip: Some(client.ip), client_ip_source: client.source };
        if !state.is_metrics_ip_allowed(client.ip) {
            return Err(MetricsAccessError::forbidden(
                MetricsAccessFailureReason::IpNotAllowed,
                "metrics endpoint is not allowed from this client IP",
            ));
        }
    } else if let Some(peer_ip) = peer_ip {
        context = MetricsAccessContext {
            client_ip: Some(peer_ip),
            client_ip_source: MetricsClientIpSource::Peer,
        };
    }

    let Some(expected_token) = state.metrics_bearer_auth_token() else {
        return Ok(context);
    };

    let provided = parse_authorization_bearer_token(headers, state.metrics_header_limits())
        .map_err(|reason| {
            let failure_reason = match reason {
                MetricsAuthHeaderParseFailure::Missing => {
                    MetricsAccessFailureReason::AuthHeaderMissing
                }
                MetricsAuthHeaderParseFailure::InvalidEncoding => {
                    MetricsAccessFailureReason::AuthHeaderInvalidEncoding
                }
                MetricsAuthHeaderParseFailure::InvalidScheme => {
                    MetricsAccessFailureReason::AuthHeaderInvalidScheme
                }
                MetricsAuthHeaderParseFailure::Malformed => {
                    MetricsAccessFailureReason::AuthHeaderMalformed
                }
                MetricsAuthHeaderParseFailure::Multiple => {
                    MetricsAccessFailureReason::AuthHeaderMultiple
                }
                MetricsAuthHeaderParseFailure::Oversized => {
                    MetricsAccessFailureReason::AuthHeaderOversized
                }
            };
            MetricsAccessError::unauthorized(
                failure_reason,
                "missing or invalid metrics bearer token",
            )
        })?;

    if constant_time_eq(provided, expected_token) {
        Ok(context)
    } else {
        Err(MetricsAccessError::unauthorized(
            MetricsAccessFailureReason::AuthTokenMismatch,
            "missing or invalid metrics bearer token",
        ))
    }
}

fn parse_authorization_bearer_token(
    headers: &HeaderMap,
    limits: MetricsHeaderLimits,
) -> Result<&str, MetricsAuthHeaderParseFailure> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next().ok_or(MetricsAuthHeaderParseFailure::Missing)?;
    if values.next().is_some() {
        return Err(MetricsAuthHeaderParseFailure::Multiple);
    }
    let value = value.to_str().map_err(|_| MetricsAuthHeaderParseFailure::InvalidEncoding)?;
    if value.len() > limits.authorization_header_value_bytes() {
        return Err(MetricsAuthHeaderParseFailure::Oversized);
    }
    parse_bearer_token_from_header(value)
}

fn resolve_metrics_client_ip(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
) -> Result<ResolvedClientIp, MetricsAccessError> {
    const FORWARDED: HeaderName = HeaderName::from_static("forwarded");
    const X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
    const X_REAL_IP: HeaderName = HeaderName::from_static("x-real-ip");
    let forwarded_ip = parse_forwarded_client_ip_with_reason(
        headers,
        &FORWARDED,
        &X_FORWARDED_FOR,
        &X_REAL_IP,
        state.metrics_header_limits(),
    );

    if state.has_metrics_trusted_proxies() {
        let peer_ip = peer_ip.ok_or_else(|| {
            MetricsAccessError::forbidden(
                MetricsAccessFailureReason::MissingPeerIpWithTrustedProxies,
                "metrics endpoint requires a peer IP when trusted proxies are configured",
            )
        })?;
        if state.is_metrics_trusted_proxy(peer_ip) {
            forwarded_ip
                .map(|ip| ResolvedClientIp {
                    ip,
                    source: MetricsClientIpSource::ForwardedTrustedProxy,
                })
                .map_err(|reason| {
                    record_forwarded_parse_denial(state, reason);
                    MetricsAccessError::forbidden(
                        metrics_access_reason_from_forwarded_failure(reason),
                        "metrics endpoint requires forwarded, x-forwarded-for, or x-real-ip from trusted proxies"
                    )
                })
        } else {
            Ok(ResolvedClientIp { ip: peer_ip, source: MetricsClientIpSource::Peer })
        }
    } else if let Some(peer_ip) = peer_ip {
        Ok(ResolvedClientIp { ip: peer_ip, source: MetricsClientIpSource::Peer })
    } else {
        forwarded_ip
            .map(|ip| ResolvedClientIp {
                ip,
                source: MetricsClientIpSource::ForwardedWithoutPeer,
            })
            .map_err(|reason| {
                record_forwarded_parse_denial(state, reason);
                MetricsAccessError::forbidden(
                    metrics_access_reason_from_forwarded_failure(reason),
                    "metrics endpoint requires forwarded, x-forwarded-for, or x-real-ip when peer IP is unavailable"
                )
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForwardedClientIpFailureReason {
    Missing,
    Invalid,
    Oversized,
}

const fn metrics_access_reason_from_forwarded_failure(
    reason: ForwardedClientIpFailureReason,
) -> MetricsAccessFailureReason {
    match reason {
        ForwardedClientIpFailureReason::Missing => {
            MetricsAccessFailureReason::ForwardedHeadersMissing
        }
        ForwardedClientIpFailureReason::Invalid => {
            MetricsAccessFailureReason::ForwardedHeadersInvalid
        }
        ForwardedClientIpFailureReason::Oversized => {
            MetricsAccessFailureReason::ForwardedHeadersOversized
        }
    }
}

fn record_forwarded_parse_denial(state: &AppState, reason: ForwardedClientIpFailureReason) {
    match reason {
        ForwardedClientIpFailureReason::Missing => {
            state.record_metrics_scrape_denied_forwarded_missing();
        }
        ForwardedClientIpFailureReason::Invalid => {
            state.record_metrics_scrape_denied_forwarded_invalid();
        }
        ForwardedClientIpFailureReason::Oversized => {
            state.record_metrics_scrape_denied_forwarded_oversized();
        }
    }
}

fn parse_forwarded_client_ip_with_reason(
    headers: &HeaderMap,
    forwarded: &HeaderName,
    x_forwarded_for: &HeaderName,
    x_real_ip: &HeaderName,
    limits: MetricsHeaderLimits,
) -> Result<IpAddr, ForwardedClientIpFailureReason> {
    let mut saw_oversized = false;
    let mut saw_invalid = false;

    if let Some(value) = headers.get(forwarded) {
        match value.to_str() {
            Ok(value) if value.len() <= limits.forwarded_header_value_bytes() => {
                if let Some(ip) = parse_forwarded_header_for_ip(value) {
                    return Ok(ip);
                }
                saw_invalid = true;
            }
            Ok(_) => saw_oversized = true,
            Err(_) => saw_invalid = true,
        }
    }

    if let Some(value) = headers.get(x_forwarded_for) {
        match value.to_str() {
            Ok(value) if value.len() <= limits.x_forwarded_for_header_value_bytes() => {
                if let Some(ip) = parse_x_forwarded_for_ip(value) {
                    return Ok(ip);
                }
                saw_invalid = true;
            }
            Ok(_) => saw_oversized = true,
            Err(_) => saw_invalid = true,
        }
    }

    if let Some(value) = headers.get(x_real_ip) {
        match value.to_str() {
            Ok(value) if value.len() <= limits.x_real_ip_header_value_bytes() => {
                if let Some(ip) = parse_client_ip(value) {
                    return Ok(ip);
                }
                saw_invalid = true;
            }
            Ok(_) => saw_oversized = true,
            Err(_) => saw_invalid = true,
        }
    }

    if saw_oversized {
        Err(ForwardedClientIpFailureReason::Oversized)
    } else if saw_invalid {
        Err(ForwardedClientIpFailureReason::Invalid)
    } else {
        Err(ForwardedClientIpFailureReason::Missing)
    }
}

fn parse_forwarded_header_for_ip(value: &str) -> Option<IpAddr> {
    let first_hop = value.split(',').next()?.trim();
    for param in first_hop.split(';') {
        let (key, raw_value) = param.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("for") {
            continue;
        }

        let value = raw_value.trim().trim_matches('"').trim();
        if value.is_empty() || value.starts_with('_') {
            return None;
        }
        if let Some(inner) = value.strip_prefix('[').and_then(|v| v.split(']').next()) {
            return inner.parse::<IpAddr>().ok();
        }
        return parse_client_ip(value);
    }
    None
}

fn parse_x_forwarded_for_ip(value: &str) -> Option<IpAddr> {
    parse_client_ip(value.split(',').next()?.trim())
}

fn parse_client_ip(value: &str) -> Option<IpAddr> {
    value
        .trim()
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.trim().parse::<SocketAddr>().ok().map(|addr| addr.ip()))
}

fn parse_bearer_token_from_header(value: &str) -> Result<&str, MetricsAuthHeaderParseFailure> {
    let mut parts = value.split_ascii_whitespace();
    let scheme = parts.next().ok_or(MetricsAuthHeaderParseFailure::Malformed)?;
    let token = parts.next().ok_or(MetricsAuthHeaderParseFailure::Malformed)?;
    if parts.next().is_some() {
        return Err(MetricsAuthHeaderParseFailure::Malformed);
    }
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err(MetricsAuthHeaderParseFailure::InvalidScheme);
    }
    if token.is_empty() {
        return Err(MetricsAuthHeaderParseFailure::Malformed);
    }
    Ok(token)
}

#[cfg(test)]
fn bearer_token_from_header(value: &str) -> Option<&str> {
    parse_bearer_token_from_header(value).ok()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut diff = a_bytes.len() ^ b_bytes.len();
    let max_len = a_bytes.len().max(b_bytes.len());
    let mut idx = 0usize;
    while idx < max_len {
        let left = a_bytes.get(idx).copied().unwrap_or(0);
        let right = b_bytes.get(idx).copied().unwrap_or(0);
        diff |= usize::from(left ^ right);
        idx += 1;
    }
    diff == 0
}

fn prometheus_metrics(state: &AppState) -> String {
    let mut out = String::new();
    let tenant = state.tenant_cache_metrics();
    let access = state.metrics_access_metrics();
    let limits = state.metrics_header_limits();
    let engine = state.commerce().metrics_snapshot();

    write_prometheus_gauge(
        &mut out,
        "stateset_http_tenant_cache_enabled",
        "Whether per-tenant routing is enabled (1=true, 0=false).",
        if tenant.enabled { 1.0 } else { 0.0 },
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_tenant_cache_max_cached_dbs",
        "Maximum number of tenant databases cached in memory.",
        tenant.max_cached_dbs as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_tenant_cache_cached_dbs",
        "Number of tenant databases currently cached.",
        tenant.cached_dbs as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_tenant_cache_in_use_cached_dbs",
        "Number of cached tenant databases currently in use.",
        tenant.in_use_cached_dbs as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_tenant_cache_hits_total",
        "Total number of tenant cache hits.",
        tenant.hits as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_tenant_cache_misses_total",
        "Total number of tenant cache misses.",
        tenant.misses as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_tenant_cache_evictions_total",
        "Total number of tenant cache evictions.",
        tenant.evictions as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_tenant_cache_rejections_total",
        "Total number of tenant cache rejections when all cached tenants are in use.",
        tenant.rejections as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_requests_total",
        "Total /metrics scrape requests received.",
        access.requests_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_allowed_total",
        "Total /metrics scrape requests allowed.",
        access.allowed_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_allowed_peer_total",
        "Total /metrics scrape requests allowed using direct peer IP source.",
        access.allowed_peer_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total",
        "Total /metrics scrape requests allowed using forwarded client IP from a trusted proxy.",
        access.allowed_forwarded_trusted_proxy_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_allowed_forwarded_without_peer_total",
        "Total /metrics scrape requests allowed using forwarded client IP without peer metadata.",
        access.allowed_forwarded_without_peer_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_allowed_unavailable_total",
        "Total /metrics scrape requests allowed without a resolved client IP source.",
        access.allowed_unavailable_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_ip_total",
        "Total /metrics scrape requests denied by network policy.",
        access.denied_ip_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_ip_not_allowed_total",
        "Total /metrics scrape requests denied because resolved client IP was not allowlisted.",
        access.denied_ip_not_allowed_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total",
        "Total /metrics scrape requests denied because trusted proxy mode lacked peer IP metadata.",
        access.denied_missing_peer_ip_with_trusted_proxies_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_total",
        "Total /metrics scrape requests denied by authentication.",
        access.denied_auth_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_missing_total",
        "Total /metrics scrape requests denied because Authorization header was missing.",
        access.denied_auth_header_missing_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_invalid_total",
        "Total /metrics scrape requests denied because Authorization header was malformed.",
        access.denied_auth_header_invalid_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total",
        "Total /metrics scrape requests denied because Authorization header could not be decoded as UTF-8.",
        access.denied_auth_header_invalid_encoding_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total",
        "Total /metrics scrape requests denied because Authorization scheme was not Bearer.",
        access.denied_auth_header_invalid_scheme_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_malformed_total",
        "Total /metrics scrape requests denied because Authorization header structure was malformed.",
        access.denied_auth_header_malformed_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_multiple_total",
        "Total /metrics scrape requests denied because multiple Authorization headers were present.",
        access.denied_auth_header_multiple_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_header_oversized_total",
        "Total /metrics scrape requests denied because Authorization header value was oversized.",
        access.denied_auth_header_oversized_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_auth_token_mismatch_total",
        "Total /metrics scrape requests denied because bearer token did not match.",
        access.denied_auth_token_mismatch_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_forwarded_missing_total",
        "Total /metrics scrape requests denied because forwarding headers were required but missing.",
        access.denied_forwarded_missing_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_forwarded_invalid_total",
        "Total /metrics scrape requests denied because forwarding headers were invalid.",
        access.denied_forwarded_invalid_total as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_http_metrics_scrape_denied_forwarded_oversized_total",
        "Total /metrics scrape requests denied because forwarding headers were oversized.",
        access.denied_forwarded_oversized_total as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_auth_enabled",
        "Whether /metrics bearer auth is enabled (1=true, 0=false).",
        if state.metrics_bearer_auth_token().is_some() { 1.0 } else { 0.0 },
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_ip_allowlist_enabled",
        "Whether /metrics IP allowlist checks are enabled (1=true, 0=false).",
        if state.has_metrics_ip_allowlist() { 1.0 } else { 0.0 },
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_ip_allowlist_entries",
        "Number of exact-IP entries configured for /metrics allowlist.",
        state.metrics_ip_allowlist_len() as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_ip_cidr_allowlist_entries",
        "Number of CIDR entries configured for /metrics allowlist.",
        state.metrics_ip_cidr_allowlist_len() as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_trusted_proxies_enabled",
        "Whether /metrics trusted proxy CIDRs are configured (1=true, 0=false).",
        if state.has_metrics_trusted_proxies() { 1.0 } else { 0.0 },
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_trusted_proxy_cidr_entries",
        "Number of trusted proxy CIDR entries configured for /metrics.",
        state.metrics_trusted_proxies_len() as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_forwarded_header_limit_bytes",
        "Max accepted byte length for the Forwarded header on /metrics.",
        limits.forwarded_header_value_bytes() as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_x_forwarded_for_header_limit_bytes",
        "Max accepted byte length for X-Forwarded-For on /metrics.",
        limits.x_forwarded_for_header_value_bytes() as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_x_real_ip_header_limit_bytes",
        "Max accepted byte length for X-Real-IP on /metrics.",
        limits.x_real_ip_header_value_bytes() as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_http_metrics_authorization_header_limit_bytes",
        "Max accepted byte length for Authorization on /metrics.",
        limits.authorization_header_value_bytes() as f64,
    );

    write_prometheus_gauge(
        &mut out,
        "stateset_engine_metrics_default_engine_only",
        "Whether engine metrics below represent only the default engine (1=true in tenant mode, 0=false).",
        if tenant.enabled { 1.0 } else { 0.0 },
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_metrics_enabled",
        "Whether engine metrics collection is enabled (1=true, 0=false).",
        if engine.enabled { 1.0 } else { 0.0 },
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_orders_created_total",
        "Total orders created.",
        engine.orders_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_customers_created_total",
        "Total customers created.",
        engine.customers_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_products_created_total",
        "Total products created.",
        engine.products_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_returns_requested_total",
        "Total return requests created.",
        engine.returns_requested as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_carts_created_total",
        "Total carts created.",
        engine.carts_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_cart_checkouts_completed_total",
        "Total completed cart checkouts.",
        engine.cart_checkouts_completed as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_shipments_created_total",
        "Total shipments created.",
        engine.shipments_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_shipments_delivered_total",
        "Total shipments delivered.",
        engine.shipments_delivered as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_subscriptions_created_total",
        "Total subscriptions created.",
        engine.subscriptions_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_payments_completed_total",
        "Total completed payments.",
        engine.payments_completed as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_inventory_adjustments_total",
        "Total inventory adjustments.",
        engine.inventory_adjustments as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_a2a_quotes_created_total",
        "Total A2A quotes created.",
        engine.a2a_quotes_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_a2a_purchases_created_total",
        "Total A2A purchases created.",
        engine.a2a_purchases_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_x402_intents_created_total",
        "Total x402 intents created.",
        engine.x402_intents_created as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_x402_intents_settled_total",
        "Total x402 intents settled.",
        engine.x402_intents_settled as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_policy_evaluations_total",
        "Total policy evaluations.",
        engine.policy_evaluations as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_policy_denials_total",
        "Total policy denials.",
        engine.policy_denials as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_agent_registrations_total",
        "Total agent registrations.",
        engine.agent_registrations as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_webhook_deliveries_total",
        "Total webhook deliveries attempted.",
        engine.webhook_deliveries as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_webhook_failures_total",
        "Total webhook delivery failures.",
        engine.webhook_failures as f64,
    );

    write_prometheus_gauge(
        &mut out,
        "stateset_engine_order_amount_total",
        "Sum of recorded order amounts.",
        engine.order_amount_total,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_payment_amount_total",
        "Sum of recorded payment amounts.",
        engine.payment_amount_total,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_inventory_delta_total",
        "Sum of recorded inventory deltas.",
        engine.inventory_delta_total,
    );

    write_prometheus_counter(
        &mut out,
        "stateset_engine_red_requests_total",
        "Total recorded requests in global RED metrics.",
        engine.red_global.requests as f64,
    );
    write_prometheus_counter(
        &mut out,
        "stateset_engine_red_errors_total",
        "Total recorded request errors in global RED metrics.",
        engine.red_global.errors as f64,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_red_duration_total_ms",
        "Total recorded request duration in milliseconds (global RED).",
        engine.red_global.duration_total_ms,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_red_error_rate",
        "Global RED error rate.",
        engine.red_global.error_rate,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_red_avg_duration_ms",
        "Global RED average duration in milliseconds.",
        engine.red_global.avg_duration_ms,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_red_p50_ms",
        "Global RED p50 latency in milliseconds.",
        engine.red_global.p50_ms,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_red_p95_ms",
        "Global RED p95 latency in milliseconds.",
        engine.red_global.p95_ms,
    );
    write_prometheus_gauge(
        &mut out,
        "stateset_engine_red_p99_ms",
        "Global RED p99 latency in milliseconds.",
        engine.red_global.p99_ms,
    );

    out.push_str(
        "# HELP stateset_engine_red_operation_requests_total Requests by normalized operation label.\n",
    );
    out.push_str("# TYPE stateset_engine_red_operation_requests_total counter\n");
    for (operation, red) in &engine.red_by_operation {
        let _ = writeln!(
            out,
            "stateset_engine_red_operation_requests_total{{operation=\"{}\"}} {}",
            escape_prometheus_label_value(operation),
            red.requests
        );
    }

    out.push_str(
        "# HELP stateset_engine_red_operation_errors_total Errors by normalized operation label.\n",
    );
    out.push_str("# TYPE stateset_engine_red_operation_errors_total counter\n");
    for (operation, red) in &engine.red_by_operation {
        let _ = writeln!(
            out,
            "stateset_engine_red_operation_errors_total{{operation=\"{}\"}} {}",
            escape_prometheus_label_value(operation),
            red.errors
        );
    }

    out.push_str("# HELP stateset_engine_red_operation_avg_duration_ms Average request duration by normalized operation label.\n");
    out.push_str("# TYPE stateset_engine_red_operation_avg_duration_ms gauge\n");
    for (operation, red) in &engine.red_by_operation {
        let _ = writeln!(
            out,
            "stateset_engine_red_operation_avg_duration_ms{{operation=\"{}\"}} {}",
            escape_prometheus_label_value(operation),
            sanitize_prometheus_f64(red.avg_duration_ms)
        );
    }

    out
}

fn write_prometheus_counter(output: &mut String, name: &str, help: &str, value: f64) {
    let _ = writeln!(output, "# HELP {name} {help}");
    let _ = writeln!(output, "# TYPE {name} counter");
    let _ = writeln!(output, "{name} {}", sanitize_prometheus_f64(value));
}

fn write_prometheus_gauge(output: &mut String, name: &str, help: &str, value: f64) {
    let _ = writeln!(output, "# HELP {name} {help}");
    let _ = writeln!(output, "# TYPE {name} gauge");
    let _ = writeln!(output, "{name} {}", sanitize_prometheus_f64(value));
}

const fn sanitize_prometheus_f64(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn escape_prometheus_label_value(value: &str) -> String {
    value.replace('\\', r"\\").replace('\n', r"\n").replace('"', r#"\""#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
    use stateset_embedded::Commerce;
    use std::future::Future;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone)]
    struct SharedLogWriterFactory {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    struct SharedLogWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl<'a> MakeWriter<'a> for SharedLogWriterFactory {
        type Writer = SharedLogWriter;

        fn make_writer(&'a self) -> Self::Writer {
            SharedLogWriter { buffer: Arc::clone(&self.buffer) }
        }
    }

    impl Write for SharedLogWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut lock = self.buffer.lock().map_err(|_| io::Error::other("log buffer lock"))?;
            lock.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    async fn with_captured_logs<F, Fut>(f: F) -> String
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt::fmt()
            .with_ansi(false)
            .with_target(false)
            .without_time()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(SharedLogWriterFactory { buffer: Arc::clone(&buffer) })
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        f().await;
        String::from_utf8(buffer.lock().unwrap().clone()).unwrap()
    }

    fn app() -> Router {
        router().with_state(AppState::new(Commerce::new(":memory:").expect("in-memory Commerce")))
    }

    fn app_with_metrics_auth_token(token: &str) -> Router {
        router().with_state(
            AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                .with_metrics_bearer_auth(token),
        )
    }

    fn metrics_request(token: &str) -> Request<Body> {
        Request::get("/metrics")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    fn metrics_request_with_peer(token: &str, peer: &str) -> Request<Body> {
        let mut request = metrics_request(token);
        request.extensions_mut().insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        request
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let resp =
            app().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["tenant_cache"]["enabled"], false);
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
        assert_eq!(json["tenant_cache"]["cached_dbs"], 0);
    }

    #[tokio::test]
    async fn metrics_returns_prometheus_text() {
        let resp =
            app().oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/plain; version=0.0.4; charset=utf-8"
        );

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("stateset_http_tenant_cache_enabled 0"));
        assert!(text.contains("stateset_http_metrics_scrape_requests_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 0"));
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 0")
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_not_allowed_total 0"));
        assert!(text.contains(
            "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total 0"
        ));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 0"));
        assert!(
            text.contains(
                "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 0"
            )
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_invalid_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_auth_enabled 0"));
        assert!(text.contains("stateset_http_metrics_ip_allowlist_enabled 0"));
        assert!(text.contains("stateset_http_metrics_ip_allowlist_entries 0"));
        assert!(text.contains("stateset_http_metrics_ip_cidr_allowlist_entries 0"));
        assert!(text.contains("stateset_http_metrics_trusted_proxies_enabled 0"));
        assert!(text.contains("stateset_http_metrics_trusted_proxy_cidr_entries 0"));
        assert!(text.contains("stateset_http_metrics_forwarded_header_limit_bytes 2048"));
        assert!(text.contains("stateset_http_metrics_x_forwarded_for_header_limit_bytes 2048"));
        assert!(text.contains("stateset_http_metrics_x_real_ip_header_limit_bytes 512"));
        assert!(text.contains("stateset_http_metrics_authorization_header_limit_bytes 2048"));
        assert!(text.contains("stateset_engine_metrics_default_engine_only 0"));
        assert!(text.contains("stateset_engine_metrics_enabled 1"));
        assert!(text.contains("stateset_engine_orders_created_total"));
    }

    #[test]
    fn prometheus_metrics_reports_metrics_access_policy_configuration() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token")
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
            .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
            .with_metrics_header_limits(
                MetricsHeaderLimits::new_with_authorization(1024, 1536, 256, 768).unwrap(),
            );

        let text = prometheus_metrics(&state);
        assert!(text.contains("stateset_http_metrics_scrape_requests_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 0"));
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 0")
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_not_allowed_total 0"));
        assert!(text.contains(
            "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total 0"
        ));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 0"));
        assert!(
            text.contains(
                "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 0"
            )
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_invalid_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_auth_enabled 1"));
        assert!(text.contains("stateset_http_metrics_ip_allowlist_enabled 1"));
        assert!(text.contains("stateset_http_metrics_ip_allowlist_entries 1"));
        assert!(text.contains("stateset_http_metrics_ip_cidr_allowlist_entries 0"));
        assert!(text.contains("stateset_http_metrics_trusted_proxies_enabled 1"));
        assert!(text.contains("stateset_http_metrics_trusted_proxy_cidr_entries 1"));
        assert!(text.contains("stateset_http_metrics_forwarded_header_limit_bytes 1024"));
        assert!(text.contains("stateset_http_metrics_x_forwarded_for_header_limit_bytes 1536"));
        assert!(text.contains("stateset_http_metrics_x_real_ip_header_limit_bytes 256"));
        assert!(text.contains("stateset_http_metrics_authorization_header_limit_bytes 768"));
    }

    #[tokio::test]
    async fn metrics_access_counters_track_denials_and_successes() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token")
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()]);
        let router = router().with_state(state);

        let unauthorized = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let forbidden = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-forwarded-for", "203.0.113.10")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let allowed = router
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);

        let body = axum::body::to_bytes(allowed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("stateset_http_metrics_scrape_requests_total 3"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 0"));
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 0")
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 1")
        );
        assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_not_allowed_total 1"));
        assert!(text.contains(
            "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total 0"
        ));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 0"));
        assert!(
            text.contains(
                "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 0"
            )
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_invalid_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_oversized_total 0"));
    }

    #[tokio::test]
    async fn metrics_access_counters_track_forwarded_parse_failure_reasons() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token")
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
            .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()]);
        let router = router().with_state(state);

        let missing = router
            .clone()
            .oneshot(metrics_request_with_peer("metrics-token", "10.1.2.3:8080"))
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);

        let mut invalid_request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
        invalid_request.headers_mut().insert("x-forwarded-for", "invalid".parse().unwrap());
        let invalid = router.clone().oneshot(invalid_request).await.unwrap();
        assert_eq!(invalid.status(), StatusCode::FORBIDDEN);

        let mut oversized_request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
        oversized_request
            .headers_mut()
            .insert("x-forwarded-for", format!("127.0.0.1,{}", "a".repeat(4096)).parse().unwrap());
        let oversized = router.clone().oneshot(oversized_request).await.unwrap();
        assert_eq!(oversized.status(), StatusCode::FORBIDDEN);

        let mut allowed_request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
        allowed_request.headers_mut().insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
        let allowed = router.oneshot(allowed_request).await.unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);

        let body = axum::body::to_bytes(allowed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("stateset_http_metrics_scrape_requests_total 4"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 0"));
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 1")
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 3"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_not_allowed_total 0"));
        assert!(text.contains(
            "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total 0"
        ));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 0"));
        assert!(
            text.contains(
                "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 0"
            )
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_missing_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_invalid_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_oversized_total 1"));
    }

    #[tokio::test]
    async fn metrics_access_counters_track_missing_peer_with_trusted_proxies_reason() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token")
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
            .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()]);
        let router = router().with_state(state);

        let missing_peer = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_peer.status(), StatusCode::FORBIDDEN);

        let mut allowed_request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
        allowed_request.headers_mut().insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
        let allowed = router.oneshot(allowed_request).await.unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);

        let body = axum::body::to_bytes(allowed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("stateset_http_metrics_scrape_requests_total 2"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 0"));
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 1")
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_not_allowed_total 0"));
        assert!(text.contains(
            "stateset_http_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies_total 1"
        ));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 0"));
        assert!(
            text.contains(
                "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 0"
            )
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_missing_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_invalid_total 0"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_forwarded_oversized_total 0"));
    }

    #[tokio::test]
    async fn metrics_access_counters_track_auth_failure_reasons() {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token");
        let router = router().with_state(state);

        let missing = router
            .clone()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let invalid_scheme = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Token metrics-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid_scheme.status(), StatusCode::UNAUTHORIZED);

        let malformed = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token extra")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(malformed.status(), StatusCode::UNAUTHORIZED);

        let mut invalid_encoding_request = Request::get("/metrics").body(Body::empty()).unwrap();
        invalid_encoding_request
            .headers_mut()
            .insert(AUTHORIZATION, HeaderValue::from_bytes(b"Bearer \xFF\xFE").unwrap());
        let invalid_encoding = router.clone().oneshot(invalid_encoding_request).await.unwrap();
        assert_eq!(invalid_encoding.status(), StatusCode::UNAUTHORIZED);

        let mismatch = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(mismatch.status(), StatusCode::UNAUTHORIZED);

        let multiple = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("authorization", "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(multiple.status(), StatusCode::UNAUTHORIZED);

        let oversized = router
            .clone()
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", format!("Bearer {}", "x".repeat(3000)))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized.status(), StatusCode::UNAUTHORIZED);

        let allowed = router
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);

        let body = axum::body::to_bytes(allowed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("stateset_http_metrics_scrape_requests_total 8"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_total 7"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_missing_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_total 3"));
        assert!(
            text.contains(
                "stateset_http_metrics_scrape_denied_auth_header_invalid_encoding_total 1"
            )
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_denied_auth_header_invalid_scheme_total 1")
        );
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_malformed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_multiple_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_header_oversized_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_auth_token_mismatch_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_denied_ip_total 0"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn metrics_access_denied_log_includes_reason_context() {
        let logs = with_captured_logs(|| async {
            let response = app_with_metrics_auth_token("metrics-token")
                .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        })
        .await;

        assert!(logs.contains("metrics access denied"), "logs were: {logs}");
        assert!(logs.contains("auth_header_missing"), "logs were: {logs}");
        assert!(logs.contains("trusted_proxy_mode=false"), "logs were: {logs}");
        assert!(logs.contains("ip_allowlist_enabled=false"), "logs were: {logs}");
        assert!(logs.contains("auth_enabled=true"), "logs were: {logs}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn metrics_access_granted_log_includes_source_context() {
        let logs = with_captured_logs(|| async {
            let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                .with_metrics_bearer_auth("metrics-token")
                .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()]);
            let router = router().with_state(state);

            let mut request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
            request.headers_mut().insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
            let response = router.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        })
        .await;

        assert!(logs.contains("metrics access granted"), "logs were: {logs}");
        assert!(logs.contains("forwarded_trusted_proxy"), "logs were: {logs}");
        assert!(logs.contains("trusted_proxy_mode=true"), "logs were: {logs}");
        assert!(logs.contains("ip_allowlist_enabled=true"), "logs were: {logs}");
        assert!(logs.contains("auth_enabled=true"), "logs were: {logs}");
    }

    #[test]
    fn require_metrics_access_reports_client_ip_source_context() {
        let mut peer_headers = HeaderMap::new();
        peer_headers.insert("authorization", "Bearer metrics-token".parse().unwrap());
        let peer_state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token");
        let peer_context = require_metrics_access(
            &peer_state,
            &peer_headers,
            Some("203.0.113.10".parse().unwrap()),
        )
        .expect("peer access should succeed");
        assert_eq!(peer_context.client_ip, Some("203.0.113.10".parse().unwrap()));
        assert_eq!(peer_context.client_ip_source, MetricsClientIpSource::Peer);

        let mut trusted_headers = HeaderMap::new();
        trusted_headers.insert("authorization", "Bearer metrics-token".parse().unwrap());
        trusted_headers.insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
        let trusted_state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token")
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
            .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()]);
        let trusted_context = require_metrics_access(
            &trusted_state,
            &trusted_headers,
            Some("10.1.2.3".parse().unwrap()),
        )
        .expect("trusted forwarded access should succeed");
        assert_eq!(trusted_context.client_ip, Some("127.0.0.1".parse().unwrap()));
        assert_eq!(trusted_context.client_ip_source, MetricsClientIpSource::ForwardedTrustedProxy);

        let mut forwarded_headers = HeaderMap::new();
        forwarded_headers.insert("authorization", "Bearer metrics-token".parse().unwrap());
        forwarded_headers.insert("x-real-ip", "127.0.0.1:443".parse().unwrap());
        let forwarded_state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
            .with_metrics_bearer_auth("metrics-token")
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()]);
        let forwarded_context = require_metrics_access(&forwarded_state, &forwarded_headers, None)
            .expect("forwarded-without-peer access should succeed");
        assert_eq!(forwarded_context.client_ip, Some("127.0.0.1".parse().unwrap()));
        assert_eq!(forwarded_context.client_ip_source, MetricsClientIpSource::ForwardedWithoutPeer);
    }

    #[tokio::test]
    async fn metrics_requires_token_when_configured() {
        let resp = app_with_metrics_auth_token("metrics-token")
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn metrics_allows_valid_token_when_configured() {
        let resp = app_with_metrics_auth_token("metrics-token")
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn parse_x_forwarded_for_ip_reads_first_hop() {
        assert_eq!(
            parse_x_forwarded_for_ip("203.0.113.10, 10.0.0.1"),
            Some("203.0.113.10".parse().unwrap())
        );
        assert_eq!(
            parse_x_forwarded_for_ip("203.0.113.10:443, 10.0.0.1"),
            Some("203.0.113.10".parse().unwrap())
        );
        assert_eq!(parse_x_forwarded_for_ip("invalid"), None);
    }

    #[test]
    fn parse_client_ip_accepts_ip_and_socket_addr_forms() {
        assert_eq!(parse_client_ip("2001:db8::1"), Some("2001:db8::1".parse().unwrap()));
        assert_eq!(parse_client_ip("[2001:db8::1]:8443"), Some("2001:db8::1".parse().unwrap()));
        assert_eq!(parse_client_ip("203.0.113.42:443"), Some("203.0.113.42".parse().unwrap()));
        assert_eq!(parse_client_ip("not-an-ip"), None);
    }

    #[test]
    fn parse_forwarded_header_for_ip_reads_for_parameter() {
        assert_eq!(
            parse_forwarded_header_for_ip("for=203.0.113.60;proto=https"),
            Some("203.0.113.60".parse().unwrap())
        );
        assert_eq!(
            parse_forwarded_header_for_ip("for=\"[2001:db8::1]:8443\";by=198.51.100.2"),
            Some("2001:db8::1".parse().unwrap())
        );
        assert_eq!(parse_forwarded_header_for_ip("for=_hidden"), None);
    }

    #[test]
    fn parse_forwarded_client_ip_ignores_oversized_forwarded_header() {
        let mut headers = HeaderMap::new();
        let limits = MetricsHeaderLimits::default();
        headers.insert(
            "forwarded",
            format!(
                "for=127.0.0.1;extra={}",
                "a".repeat(limits.forwarded_header_value_bytes() + 1)
            )
            .parse()
            .unwrap(),
        );
        headers.insert("x-forwarded-for", "127.0.0.1".parse().unwrap());

        let resolved = parse_forwarded_client_ip_with_reason(
            &headers,
            &HeaderName::from_static("forwarded"),
            &HeaderName::from_static("x-forwarded-for"),
            &HeaderName::from_static("x-real-ip"),
            limits,
        )
        .ok();
        assert_eq!(resolved, Some("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn parse_forwarded_client_ip_rejects_oversized_x_forwarded_for_header() {
        let mut headers = HeaderMap::new();
        let limits = MetricsHeaderLimits::default();
        headers.insert(
            "x-forwarded-for",
            format!("127.0.0.1,{}", "a".repeat(limits.x_forwarded_for_header_value_bytes() + 1))
                .parse()
                .unwrap(),
        );

        let resolved = parse_forwarded_client_ip_with_reason(
            &headers,
            &HeaderName::from_static("forwarded"),
            &HeaderName::from_static("x-forwarded-for"),
            &HeaderName::from_static("x-real-ip"),
            limits,
        )
        .ok();
        assert_eq!(resolved, None);
    }

    #[test]
    fn parse_forwarded_client_ip_respects_custom_x_real_ip_limit() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "127.0.0.1:8080".parse().unwrap());

        let rejected = parse_forwarded_client_ip_with_reason(
            &headers,
            &HeaderName::from_static("forwarded"),
            &HeaderName::from_static("x-forwarded-for"),
            &HeaderName::from_static("x-real-ip"),
            MetricsHeaderLimits::new(2048, 2048, 8).unwrap(),
        )
        .ok();
        assert_eq!(rejected, None);

        let accepted = parse_forwarded_client_ip_with_reason(
            &headers,
            &HeaderName::from_static("forwarded"),
            &HeaderName::from_static("x-forwarded-for"),
            &HeaderName::from_static("x-real-ip"),
            MetricsHeaderLimits::new(2048, 2048, 32).unwrap(),
        )
        .ok();
        assert_eq!(accepted, Some("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn parse_forwarded_client_ip_with_reason_reports_failure_reasons() {
        let limits = MetricsHeaderLimits::default();
        let forwarded = HeaderName::from_static("forwarded");
        let x_forwarded_for = HeaderName::from_static("x-forwarded-for");
        let x_real_ip = HeaderName::from_static("x-real-ip");

        let headers = HeaderMap::new();
        assert_eq!(
            parse_forwarded_client_ip_with_reason(
                &headers,
                &forwarded,
                &x_forwarded_for,
                &x_real_ip,
                limits
            ),
            Err(ForwardedClientIpFailureReason::Missing)
        );

        let mut invalid_headers = HeaderMap::new();
        invalid_headers.insert("x-forwarded-for", "invalid".parse().unwrap());
        assert_eq!(
            parse_forwarded_client_ip_with_reason(
                &invalid_headers,
                &forwarded,
                &x_forwarded_for,
                &x_real_ip,
                limits
            ),
            Err(ForwardedClientIpFailureReason::Invalid)
        );

        let mut oversized_headers = HeaderMap::new();
        oversized_headers.insert(
            "x-forwarded-for",
            format!("127.0.0.1,{}", "a".repeat(limits.x_forwarded_for_header_value_bytes() + 1))
                .parse()
                .unwrap(),
        );
        assert_eq!(
            parse_forwarded_client_ip_with_reason(
                &oversized_headers,
                &forwarded,
                &x_forwarded_for,
                &x_real_ip,
                limits
            ),
            Err(ForwardedClientIpFailureReason::Oversized)
        );
    }

    #[test]
    fn parse_authorization_bearer_token_respects_custom_limit() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer short-token".parse().unwrap());
        let accepted_limits =
            MetricsHeaderLimits::new_with_authorization(2048, 2048, 512, 32).unwrap();
        let rejected_limits =
            MetricsHeaderLimits::new_with_authorization(2048, 2048, 512, 12).unwrap();

        assert_eq!(parse_authorization_bearer_token(&headers, accepted_limits), Ok("short-token"));
        assert_eq!(
            parse_authorization_bearer_token(&headers, rejected_limits),
            Err(MetricsAuthHeaderParseFailure::Oversized)
        );
    }

    #[test]
    fn parse_authorization_bearer_token_rejects_extra_parts() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer short-token extra".parse().unwrap());

        assert_eq!(
            parse_authorization_bearer_token(&headers, MetricsHeaderLimits::default()),
            Err(MetricsAuthHeaderParseFailure::Malformed)
        );
    }

    #[test]
    fn parse_authorization_bearer_token_rejects_invalid_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Token short-token".parse().unwrap());

        assert_eq!(
            parse_authorization_bearer_token(&headers, MetricsHeaderLimits::default()),
            Err(MetricsAuthHeaderParseFailure::InvalidScheme)
        );
    }

    #[test]
    fn parse_authorization_bearer_token_rejects_non_utf8_bytes() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_bytes(b"Bearer \xFF\xFE").unwrap());

        assert_eq!(
            parse_authorization_bearer_token(&headers, MetricsHeaderLimits::default()),
            Err(MetricsAuthHeaderParseFailure::InvalidEncoding)
        );
    }

    #[test]
    fn bearer_token_from_header_requires_exactly_two_parts() {
        assert_eq!(bearer_token_from_header("Bearer metrics-token"), Some("metrics-token"));
        assert_eq!(bearer_token_from_header("Bearer metrics-token extra"), None);
        assert_eq!(bearer_token_from_header("Bearer"), None);
        assert_eq!(bearer_token_from_header("Token metrics-token"), None);
    }

    #[test]
    fn constant_time_eq_behaves_like_string_equality() {
        assert!(constant_time_eq("metrics-token", "metrics-token"));
        assert!(!constant_time_eq("metrics-token", "metrics-token-2"));
        assert!(!constant_time_eq("metrics-token", "metrics-t0ken"));
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("", "non-empty"));
    }

    #[tokio::test]
    async fn metrics_blocks_disallowed_ip_when_allowlist_configured() {
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-forwarded-for", "203.0.113.10")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn metrics_allows_allowed_ip_when_allowlist_configured() {
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_allows_allowed_x_real_ip_when_allowlist_configured() {
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-real-ip", "127.0.0.1:443")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_requires_client_ip_header_when_allowlist_configured() {
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn metrics_allows_peer_ip_when_allowlist_configured_and_peer_present() {
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(metrics_request_with_peer("metrics-token", "127.0.0.1:8080"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("stateset_http_metrics_scrape_allowed_total 1"));
        assert!(text.contains("stateset_http_metrics_scrape_allowed_peer_total 1"));
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_trusted_proxy_total 0")
        );
        assert!(
            text.contains("stateset_http_metrics_scrape_allowed_forwarded_without_peer_total 0")
        );
        assert!(text.contains("stateset_http_metrics_scrape_allowed_unavailable_total 0"));
    }

    #[tokio::test]
    async fn metrics_ignores_forwarded_ip_from_untrusted_peer_when_trusted_proxies_configured() {
        let mut request = metrics_request_with_peer("metrics-token", "203.0.113.10:8080");
        request.headers_mut().insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn metrics_uses_forwarded_ip_from_trusted_proxy_when_configured() {
        let mut request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
        request.headers_mut().insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_uses_forwarded_header_from_trusted_proxy_when_configured() {
        let mut request = metrics_request_with_peer("metrics-token", "10.1.2.3:8080");
        request.headers_mut().insert("forwarded", "for=127.0.0.1;proto=https".parse().unwrap());
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_requires_peer_ip_when_trusted_proxies_configured() {
        let resp = router()
            .with_state(
                AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"))
                    .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
                    .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
                    .with_metrics_bearer_auth("metrics-token"),
            )
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", "Bearer metrics-token")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn readiness_response_reports_not_ready_when_disconnected() {
        let (status, body) = readiness_response(
            false,
            TenantCacheResponse {
                enabled: false,
                max_cached_dbs: 256,
                cached_dbs: 0,
                in_use_cached_dbs: 0,
                hits: 0,
                misses: 0,
                evictions: 0,
                rejections: 0,
            },
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.database, "disconnected");
        assert_eq!(body.tenant_cache.misses, 0);
    }

    #[test]
    fn escape_prometheus_label_escapes_control_characters() {
        let escaped = escape_prometheus_label_value("checkout\"v2\\canary\n");
        assert_eq!(escaped, "checkout\\\"v2\\\\canary\\n");
    }
}
