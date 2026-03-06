//! Server builder for configuring and running the HTTP service.

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use axum::Router;
use stateset_embedded::Commerce;
use uuid::Uuid;

use crate::error::HttpError;
use crate::middleware::{self, RateLimitConfig};
use crate::routes;
use crate::state::{AppState, IpCidr, MetricsHeaderLimits};

/// Default bind address.
const DEFAULT_ADDR: ([u8; 4], u16) = ([127, 0, 0, 1], 3000);
const METRICS_IP_ALLOWLIST_ENV: &str = "STATESET_HTTP_METRICS_IP_ALLOWLIST";
const METRICS_IP_CIDR_ALLOWLIST_ENV: &str = "STATESET_HTTP_METRICS_IP_CIDR_ALLOWLIST";
const METRICS_TRUSTED_PROXIES_ENV: &str = "STATESET_HTTP_METRICS_TRUSTED_PROXIES";
const METRICS_FORWARDED_HEADER_MAX_BYTES_ENV: &str = "STATESET_HTTP_METRICS_FORWARDED_MAX_BYTES";
const METRICS_X_FORWARDED_FOR_HEADER_MAX_BYTES_ENV: &str =
    "STATESET_HTTP_METRICS_X_FORWARDED_FOR_MAX_BYTES";
const METRICS_X_REAL_IP_HEADER_MAX_BYTES_ENV: &str = "STATESET_HTTP_METRICS_X_REAL_IP_MAX_BYTES";
const METRICS_AUTHORIZATION_HEADER_MAX_BYTES_ENV: &str =
    "STATESET_HTTP_METRICS_AUTHORIZATION_MAX_BYTES";

fn parse_ip_allowlist_csv(env_var: &str, value: &str) -> Result<Vec<IpAddr>, HttpError> {
    let mut ips = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry.parse::<IpAddr>().map_err(|error| {
                HttpError::BadRequest(format!("invalid IP '{entry}' in {env_var}: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    ips.sort_unstable();
    ips.dedup();
    Ok(ips)
}

fn parse_ip_cidr_allowlist_csv(env_var: &str, value: &str) -> Result<Vec<IpCidr>, HttpError> {
    let mut cidrs = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry.parse::<IpCidr>().map_err(|error| {
                HttpError::BadRequest(format!("invalid CIDR '{entry}' in {env_var}: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    cidrs.sort_unstable();
    cidrs.dedup();
    Ok(cidrs)
}

fn parse_positive_usize_env(env_var: &str, value: &str) -> Result<usize, HttpError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HttpError::BadRequest(format!(
            "{env_var} must be a positive integer greater than zero"
        )));
    }
    let parsed = trimmed.parse::<usize>().map_err(|error| {
        HttpError::BadRequest(format!("invalid value '{trimmed}' in {env_var}: {error}"))
    })?;
    if parsed == 0 {
        return Err(HttpError::BadRequest(format!("{env_var} must be greater than zero")));
    }
    Ok(parsed)
}

/// Builder for configuring and launching the HTTP server.
///
/// # Example
///
/// ```rust,ignore
/// use stateset_embedded::Commerce;
/// use stateset_http::ServerBuilder;
///
/// let commerce = Commerce::new(":memory:")?;
///
/// ServerBuilder::new_from_env(commerce)?
///     .bind("0.0.0.0:8080".parse()?)
///     .with_cors()
///     .with_request_id()
///     .with_bearer_auth("replace-me-with-a-secret")
///     .serve()
///     .await?;
/// ```
pub struct ServerBuilder {
    state: AppState,
    addr: SocketAddr,
    enable_cors: bool,
    enable_request_id: bool,
    api_bearer_token: Option<String>,
    bound_tenant_id: Option<String>,
    generated_default_token: bool,
    rate_limit: Option<RateLimitConfig>,
}

impl fmt::Debug for ServerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerBuilder")
            .field("state", &"AppState { .. }")
            .field("addr", &self.addr)
            .field("enable_cors", &self.enable_cors)
            .field("enable_request_id", &self.enable_request_id)
            .field("api_bearer_token", &self.api_bearer_token.as_ref().map(|_| "<redacted>"))
            .field(
                "metrics_bearer_token",
                &self.state.metrics_bearer_auth_token().map(|_| "<redacted>"),
            )
            .field("bound_tenant_id", &self.bound_tenant_id.as_ref().map(|_| "<redacted>"))
            .field("generated_default_token", &self.generated_default_token)
            .field("rate_limit", &self.rate_limit)
            .finish()
    }
}

impl ServerBuilder {
    /// Create a new server builder wrapping a [`Commerce`] instance.
    #[must_use]
    pub fn new(commerce: Commerce) -> Self {
        let generated_token = Uuid::new_v4().to_string();
        Self {
            state: AppState::new(commerce).with_metrics_bearer_auth(generated_token.clone()),
            addr: SocketAddr::from(DEFAULT_ADDR),
            enable_cors: false,
            enable_request_id: false,
            // Secure-by-default: API routes require an auth token unless
            // explicitly disabled.
            api_bearer_token: Some(generated_token),
            bound_tenant_id: None,
            generated_default_token: true,
            rate_limit: None,
        }
    }

    /// Create a new server builder and apply `/metrics` network policy from environment.
    ///
    /// This is useful for startup wiring where operators configure network policy via:
    /// - `STATESET_HTTP_METRICS_IP_ALLOWLIST`
    /// - `STATESET_HTTP_METRICS_IP_CIDR_ALLOWLIST`
    /// - `STATESET_HTTP_METRICS_TRUSTED_PROXIES`
    /// - `STATESET_HTTP_METRICS_FORWARDED_MAX_BYTES`
    /// - `STATESET_HTTP_METRICS_X_FORWARDED_FOR_MAX_BYTES`
    /// - `STATESET_HTTP_METRICS_X_REAL_IP_MAX_BYTES`
    /// - `STATESET_HTTP_METRICS_AUTHORIZATION_MAX_BYTES`
    pub fn new_from_env(commerce: Commerce) -> Result<Self, HttpError> {
        Self::new(commerce)
            .with_metrics_network_policy_from_env()?
            .with_metrics_header_limits_from_env()
    }

    /// Set the bind address.
    #[must_use]
    pub const fn bind(mut self, addr: SocketAddr) -> Self {
        self.addr = addr;
        self
    }

    /// Enable CORS middleware with explicit defaults and optional env override.
    ///
    /// Set `STATESET_HTTP_ALLOWED_ORIGINS` to a comma-separated list of origins
    /// to override the localhost-only defaults.
    #[must_use]
    pub const fn with_cors(mut self) -> Self {
        self.enable_cors = true;
        self
    }

    /// Enable request-ID middleware (generates UUID, propagates in headers).
    #[must_use]
    pub const fn with_request_id(mut self) -> Self {
        self.enable_request_id = true;
        self
    }

    /// Configure bearer authentication for `/api/v1/*` endpoints.
    ///
    /// Requests to API routes must include `Authorization: Bearer <token>`.
    #[must_use]
    pub fn with_bearer_auth(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.api_bearer_token = Some(token.clone());
        self.state = self.state.with_metrics_bearer_auth(token);
        self.generated_default_token = false;
        self
    }

    /// Configure bearer authentication for `/metrics`.
    #[must_use]
    pub fn with_metrics_bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.state = self.state.with_metrics_bearer_auth(token);
        self
    }

    /// Disable authentication for `/metrics`.
    #[must_use]
    pub fn without_metrics_auth(mut self) -> Self {
        self.state = self.state.without_metrics_auth();
        self
    }

    /// Configure a client IP allowlist for `/metrics`.
    #[must_use]
    pub fn with_metrics_ip_allowlist<I>(mut self, ips: I) -> Self
    where
        I: IntoIterator<Item = IpAddr>,
    {
        self.state = self.state.with_metrics_ip_allowlist(ips);
        self
    }

    /// Disable `/metrics` IP allowlist checks.
    #[must_use]
    pub fn without_metrics_ip_allowlist(mut self) -> Self {
        self.state = self.state.without_metrics_ip_allowlist();
        self
    }

    /// Configure a CIDR-based client IP allowlist for `/metrics`.
    #[must_use]
    pub fn with_metrics_ip_cidr_allowlist<I>(mut self, cidrs: I) -> Self
    where
        I: IntoIterator<Item = IpCidr>,
    {
        self.state = self.state.with_metrics_ip_cidr_allowlist(cidrs);
        self
    }

    /// Disable CIDR-based `/metrics` IP allowlist checks.
    #[must_use]
    pub fn without_metrics_ip_cidr_allowlist(mut self) -> Self {
        self.state = self.state.without_metrics_ip_cidr_allowlist();
        self
    }

    /// Configure trusted proxy CIDRs for `/metrics`.
    #[must_use]
    pub fn with_metrics_trusted_proxies<I>(mut self, cidrs: I) -> Self
    where
        I: IntoIterator<Item = IpCidr>,
    {
        self.state = self.state.with_metrics_trusted_proxies(cidrs);
        self
    }

    /// Configure max accepted forwarding header lengths for `/metrics`.
    #[must_use]
    pub fn with_metrics_header_limits(mut self, limits: MetricsHeaderLimits) -> Self {
        self.state = self.state.with_metrics_header_limits(limits);
        self
    }

    /// Apply `/metrics` network policy from environment variables.
    ///
    /// Supported variables (comma-separated entries):
    /// - `STATESET_HTTP_METRICS_IP_ALLOWLIST` (exact IPs)
    /// - `STATESET_HTTP_METRICS_IP_CIDR_ALLOWLIST` (CIDRs and/or IPs)
    /// - `STATESET_HTTP_METRICS_TRUSTED_PROXIES` (CIDRs and/or IPs)
    ///
    /// If a variable is present but empty, the corresponding config is disabled.
    pub fn with_metrics_network_policy_from_env(self) -> Result<Self, HttpError> {
        let ip_allowlist = std::env::var(METRICS_IP_ALLOWLIST_ENV).ok();
        let ip_cidr_allowlist = std::env::var(METRICS_IP_CIDR_ALLOWLIST_ENV).ok();
        let trusted_proxies = std::env::var(METRICS_TRUSTED_PROXIES_ENV).ok();
        self.with_metrics_network_policy_from_values(
            ip_allowlist.as_deref(),
            ip_cidr_allowlist.as_deref(),
            trusted_proxies.as_deref(),
        )
    }

    /// Apply `/metrics` forwarding header limits from environment variables.
    ///
    /// Supported variables (positive integers in bytes):
    /// - `STATESET_HTTP_METRICS_FORWARDED_MAX_BYTES`
    /// - `STATESET_HTTP_METRICS_X_FORWARDED_FOR_MAX_BYTES`
    /// - `STATESET_HTTP_METRICS_X_REAL_IP_MAX_BYTES`
    /// - `STATESET_HTTP_METRICS_AUTHORIZATION_MAX_BYTES`
    pub fn with_metrics_header_limits_from_env(self) -> Result<Self, HttpError> {
        let forwarded = std::env::var(METRICS_FORWARDED_HEADER_MAX_BYTES_ENV).ok();
        let x_forwarded_for = std::env::var(METRICS_X_FORWARDED_FOR_HEADER_MAX_BYTES_ENV).ok();
        let x_real_ip = std::env::var(METRICS_X_REAL_IP_HEADER_MAX_BYTES_ENV).ok();
        let authorization = std::env::var(METRICS_AUTHORIZATION_HEADER_MAX_BYTES_ENV).ok();
        self.with_metrics_header_limits_from_values(
            forwarded.as_deref(),
            x_forwarded_for.as_deref(),
            x_real_ip.as_deref(),
            authorization.as_deref(),
        )
    }

    fn with_metrics_network_policy_from_values(
        mut self,
        ip_allowlist: Option<&str>,
        ip_cidr_allowlist: Option<&str>,
        trusted_proxies: Option<&str>,
    ) -> Result<Self, HttpError> {
        if let Some(raw) = ip_allowlist {
            let ips = parse_ip_allowlist_csv(METRICS_IP_ALLOWLIST_ENV, raw)?;
            self = if ips.is_empty() {
                self.without_metrics_ip_allowlist()
            } else {
                self.with_metrics_ip_allowlist(ips)
            };
        }

        if let Some(raw) = ip_cidr_allowlist {
            let cidrs = parse_ip_cidr_allowlist_csv(METRICS_IP_CIDR_ALLOWLIST_ENV, raw)?;
            self = if cidrs.is_empty() {
                self.without_metrics_ip_cidr_allowlist()
            } else {
                self.with_metrics_ip_cidr_allowlist(cidrs)
            };
        }

        if let Some(raw) = trusted_proxies {
            let cidrs = parse_ip_cidr_allowlist_csv(METRICS_TRUSTED_PROXIES_ENV, raw)?;
            self = if cidrs.is_empty() {
                self.without_metrics_trusted_proxies()
            } else {
                self.with_metrics_trusted_proxies(cidrs)
            };
        }

        Ok(self)
    }

    fn with_metrics_header_limits_from_values(
        self,
        forwarded: Option<&str>,
        x_forwarded_for: Option<&str>,
        x_real_ip: Option<&str>,
        authorization: Option<&str>,
    ) -> Result<Self, HttpError> {
        let current = self.metrics_header_limits();
        let forwarded = if let Some(raw) = forwarded {
            parse_positive_usize_env(METRICS_FORWARDED_HEADER_MAX_BYTES_ENV, raw)?
        } else {
            current.forwarded_header_value_bytes()
        };
        let x_forwarded_for = if let Some(raw) = x_forwarded_for {
            parse_positive_usize_env(METRICS_X_FORWARDED_FOR_HEADER_MAX_BYTES_ENV, raw)?
        } else {
            current.x_forwarded_for_header_value_bytes()
        };
        let x_real_ip = if let Some(raw) = x_real_ip {
            parse_positive_usize_env(METRICS_X_REAL_IP_HEADER_MAX_BYTES_ENV, raw)?
        } else {
            current.x_real_ip_header_value_bytes()
        };
        let authorization = if let Some(raw) = authorization {
            parse_positive_usize_env(METRICS_AUTHORIZATION_HEADER_MAX_BYTES_ENV, raw)?
        } else {
            current.authorization_header_value_bytes()
        };
        let limits = MetricsHeaderLimits::new_with_authorization(
            forwarded,
            x_forwarded_for,
            x_real_ip,
            authorization,
        )?;
        Ok(self.with_metrics_header_limits(limits))
    }

    /// Disable trusted proxy checks for `/metrics`.
    #[must_use]
    pub fn without_metrics_trusted_proxies(mut self) -> Self {
        self.state = self.state.without_metrics_trusted_proxies();
        self
    }

    /// Bind the configured bearer token to a single tenant.
    ///
    /// Requests using this token must present the same `x-tenant-id`.
    #[must_use]
    pub fn bind_auth_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.bound_tenant_id = Some(tenant_id.into());
        self
    }

    /// Configure bearer authentication and bind it to a tenant in one call.
    #[must_use]
    pub fn with_bearer_auth_for_tenant(
        self,
        token: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        self.with_bearer_auth(token).bind_auth_tenant(tenant_id)
    }

    /// Enable per-tenant storage using `<base_dir>/<tenant>.db`.
    #[must_use]
    pub fn with_tenant_db_dir(mut self, base_dir: impl Into<PathBuf>) -> Self {
        self.state = self.state.with_tenant_db_dir(base_dir);
        self
    }

    /// Set the maximum number of lazily created tenant databases kept in-memory.
    #[must_use]
    pub fn with_max_tenant_dbs(mut self, max_tenant_dbs: usize) -> Self {
        self.state = self.state.with_max_tenant_dbs(max_tenant_dbs);
        self
    }

    /// Disable API authentication (not recommended for untrusted networks).
    #[must_use]
    pub fn without_auth(mut self) -> Self {
        self.api_bearer_token = None;
        self.bound_tenant_id = None;
        self.state = self.state.without_metrics_auth();
        self.generated_default_token = false;
        self
    }

    /// Enable global rate limiting with a token bucket.
    ///
    /// `requests_per_second` controls steady-state throughput; `burst_size`
    /// controls how many requests can be served in a burst before throttling.
    /// Requests exceeding the limit receive HTTP 429.
    #[must_use]
    pub const fn with_rate_limit(mut self, requests_per_second: u64, burst_size: u64) -> Self {
        self.rate_limit = Some(RateLimitConfig { requests_per_second, burst_size });
        self
    }

    /// Return the configured bearer token, if auth is enabled.
    #[must_use]
    pub fn bearer_auth_token(&self) -> Option<&str> {
        self.api_bearer_token.as_deref()
    }

    /// Return the configured bearer token for `/metrics`, if enabled.
    #[must_use]
    pub fn metrics_bearer_auth_token(&self) -> Option<&str> {
        self.state.metrics_bearer_auth_token()
    }

    /// Return configured metrics IP allowlist entries, if any.
    #[must_use]
    pub fn metrics_ip_allowlist(&self) -> Option<Vec<IpAddr>> {
        self.state.metrics_ip_allowlist()
    }

    /// Return configured CIDR-based metrics IP allowlist entries, if any.
    #[must_use]
    pub fn metrics_ip_cidr_allowlist(&self) -> Option<Vec<IpCidr>> {
        self.state.metrics_ip_cidr_allowlist()
    }

    /// Return configured trusted proxy CIDRs for `/metrics`, if any.
    #[must_use]
    pub fn metrics_trusted_proxies(&self) -> Option<Vec<IpCidr>> {
        self.state.metrics_trusted_proxies()
    }

    /// Return configured max accepted forwarding header lengths for `/metrics`.
    #[must_use]
    pub const fn metrics_header_limits(&self) -> MetricsHeaderLimits {
        self.state.metrics_header_limits()
    }

    /// Build the axum [`Router`] without starting the server.
    ///
    /// Useful for testing or embedding in a larger application.
    pub fn build(self) -> Router {
        let auth_config = self.api_bearer_token.map(|token| (token, self.bound_tenant_id));
        let router = routes::api_router().with_state(self.state);
        middleware::apply_middleware(
            router,
            self.enable_cors,
            self.enable_request_id,
            auth_config,
            self.rate_limit,
        )
    }

    /// Build the router and start serving HTTP requests.
    ///
    /// This method will block until the server is shut down.
    pub async fn serve(self) -> Result<(), HttpError> {
        let token = self.api_bearer_token.clone();
        let metrics_token = self.state.metrics_bearer_auth_token().map(ToOwned::to_owned);
        let trusted_proxy_count = self.state.metrics_trusted_proxies().map_or(0, |v| v.len());
        let metrics_header_limits = self.state.metrics_header_limits();
        let bound_tenant_id = self.bound_tenant_id.clone();
        let generated_default_token = self.generated_default_token;
        let addr = self.addr;

        if token.is_none() && !addr.ip().is_loopback() {
            return Err(HttpError::BadRequest(
                "Refusing to start without API auth on a non-loopback address".to_string(),
            ));
        }

        let app = self.build();

        tracing::info!("StateSet HTTP listening on {addr}");
        if let Some(token) = token.as_deref() {
            tracing::info!("API bearer authentication is enabled for /api/v1/*");
            if let Some(bound_tenant_id) = bound_tenant_id.as_deref() {
                tracing::info!(
                    tenant_id = %bound_tenant_id,
                    "API token is bound to a specific tenant"
                );
            }
            if generated_default_token {
                tracing::warn!(
                    "Using generated bearer token. Persist it and rotate for production deployments."
                );
                let preview: String = token.chars().take(8).collect();
                tracing::info!(
                    token_preview = %preview,
                    token_length = token.len(),
                    "Generated API bearer token (redacted preview)"
                );
            }
        } else {
            tracing::warn!("API authentication is disabled for /api/v1/*");
        }

        if metrics_token.is_some() {
            tracing::info!("Metrics bearer authentication is enabled for /metrics");
        } else if !addr.ip().is_loopback() {
            tracing::warn!(
                "Metrics authentication is disabled for /metrics on a non-loopback bind"
            );
        }
        if trusted_proxy_count > 0 {
            tracing::info!(
                trusted_proxy_count,
                "Metrics forwarded headers are trusted only for configured proxy CIDRs"
            );
        }
        tracing::info!(
            forwarded_header_max_bytes = metrics_header_limits.forwarded_header_value_bytes(),
            x_forwarded_for_header_max_bytes =
                metrics_header_limits.x_forwarded_for_header_value_bytes(),
            x_real_ip_header_max_bytes = metrics_header_limits.x_real_ip_header_value_bytes(),
            authorization_header_max_bytes =
                metrics_header_limits.authorization_header_value_bytes(),
            "Metrics header byte limits configured"
        );

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| HttpError::InternalError(format!("Failed to bind: {e}")))?;

        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|e| HttpError::InternalError(format!("Server error: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use uuid::Uuid;

    fn test_commerce() -> Commerce {
        Commerce::new(":memory:").expect("in-memory Commerce")
    }

    #[test]
    fn builder_default_addr() {
        let builder = ServerBuilder::new(test_commerce());
        assert_eq!(builder.addr, SocketAddr::from(DEFAULT_ADDR));
        assert!(!builder.enable_cors);
        assert!(!builder.enable_request_id);
        assert!(builder.api_bearer_token.is_some());
        assert!(builder.metrics_bearer_auth_token().is_some());
        assert_eq!(builder.metrics_header_limits(), MetricsHeaderLimits::default());
        assert!(builder.bound_tenant_id.is_none());
    }

    #[test]
    fn builder_with_bind() {
        let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        let builder = ServerBuilder::new(test_commerce()).bind(addr);
        assert_eq!(builder.addr, addr);
    }

    #[test]
    fn builder_with_cors() {
        let builder = ServerBuilder::new(test_commerce()).with_cors();
        assert!(builder.enable_cors);
    }

    #[test]
    fn builder_with_request_id() {
        let builder = ServerBuilder::new(test_commerce()).with_request_id();
        assert!(builder.enable_request_id);
    }

    #[test]
    fn builder_with_bearer_auth() {
        let builder = ServerBuilder::new(test_commerce()).with_bearer_auth("test-token");
        assert_eq!(builder.bearer_auth_token(), Some("test-token"));
        assert_eq!(builder.metrics_bearer_auth_token(), Some("test-token"));
        assert!(builder.bound_tenant_id.is_none());
    }

    #[test]
    fn builder_with_metrics_bearer_auth() {
        let builder = ServerBuilder::new(test_commerce()).with_metrics_bearer_auth("metrics-token");
        assert_eq!(builder.metrics_bearer_auth_token(), Some("metrics-token"));
    }

    #[test]
    fn builder_with_metrics_ip_allowlist() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap(), "10.0.0.1".parse().unwrap()]);

        let allowlist = builder.metrics_ip_allowlist().unwrap();
        assert_eq!(allowlist.len(), 2);
    }

    #[test]
    fn parse_ip_allowlist_csv_parses_and_sorts_unique_values() {
        let parsed =
            parse_ip_allowlist_csv(METRICS_IP_ALLOWLIST_ENV, "127.0.0.1, 10.0.0.1,127.0.0.1")
                .unwrap();
        assert_eq!(
            parsed,
            vec!["10.0.0.1".parse::<IpAddr>().unwrap(), "127.0.0.1".parse::<IpAddr>().unwrap()]
        );
    }

    #[test]
    fn parse_ip_cidr_allowlist_csv_rejects_invalid_entries() {
        let err = parse_ip_cidr_allowlist_csv(METRICS_IP_CIDR_ALLOWLIST_ENV, "10.0.0.0/8,bogus")
            .expect_err("should reject invalid CIDR");
        assert!(err.to_string().contains(METRICS_IP_CIDR_ALLOWLIST_ENV));
    }

    #[test]
    fn builder_with_metrics_ip_cidr_allowlist() {
        let builder = ServerBuilder::new(test_commerce()).with_metrics_ip_cidr_allowlist([
            "10.0.0.0/8".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
        ]);

        let cidrs = builder.metrics_ip_cidr_allowlist().unwrap();
        assert_eq!(cidrs, vec!["10.0.0.0/8".parse().unwrap(), "127.0.0.1/32".parse().unwrap()]);
    }

    #[test]
    fn builder_with_metrics_trusted_proxies() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()]);
        let proxies = builder.metrics_trusted_proxies().unwrap();
        assert_eq!(proxies, vec!["10.0.0.0/8".parse().unwrap()]);
    }

    #[test]
    fn builder_with_metrics_header_limits() {
        let limits = MetricsHeaderLimits::new(1024, 1536, 256).unwrap();
        let builder = ServerBuilder::new(test_commerce()).with_metrics_header_limits(limits);
        assert_eq!(builder.metrics_header_limits(), limits);
    }

    #[test]
    fn builder_with_metrics_network_policy_from_env() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_network_policy_from_values(
                Some("127.0.0.1"),
                Some("10.0.0.0/8"),
                Some("10.1.0.0/16"),
            )
            .expect("policy should parse");

        assert_eq!(builder.metrics_ip_allowlist(), Some(vec!["127.0.0.1".parse().unwrap()]));
        assert_eq!(builder.metrics_ip_cidr_allowlist(), Some(vec!["10.0.0.0/8".parse().unwrap()]));
        assert_eq!(builder.metrics_trusted_proxies(), Some(vec!["10.1.0.0/16".parse().unwrap()]));
    }

    #[test]
    fn builder_with_metrics_network_policy_from_env_can_disable_lists_with_empty_values() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
            .with_metrics_ip_cidr_allowlist(["10.0.0.0/8".parse().unwrap()])
            .with_metrics_trusted_proxies(["10.1.0.0/16".parse().unwrap()])
            .with_metrics_network_policy_from_values(Some("   "), Some(","), Some("  ,  "))
            .expect("empty policy values should disable lists");

        assert!(builder.metrics_ip_allowlist().is_none());
        assert!(builder.metrics_ip_cidr_allowlist().is_none());
        assert!(builder.metrics_trusted_proxies().is_none());
    }

    #[test]
    fn parse_positive_usize_env_rejects_zero_and_empty_values() {
        let zero =
            parse_positive_usize_env(METRICS_X_REAL_IP_HEADER_MAX_BYTES_ENV, "0").unwrap_err();
        assert!(zero.to_string().contains("greater than zero"));

        let empty =
            parse_positive_usize_env(METRICS_X_REAL_IP_HEADER_MAX_BYTES_ENV, "   ").unwrap_err();
        assert!(empty.to_string().contains("positive integer"));
    }

    #[test]
    fn builder_with_metrics_header_limits_from_values() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_header_limits_from_values(
                Some("1024"),
                Some("1536"),
                Some("256"),
                Some("768"),
            )
            .expect("limits should parse");
        let limits = builder.metrics_header_limits();
        assert_eq!(limits.forwarded_header_value_bytes(), 1024);
        assert_eq!(limits.x_forwarded_for_header_value_bytes(), 1536);
        assert_eq!(limits.x_real_ip_header_value_bytes(), 256);
        assert_eq!(limits.authorization_header_value_bytes(), 768);
    }

    #[test]
    fn builder_with_metrics_header_limits_from_values_applies_partial_overrides() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_header_limits_from_values(None, Some("4096"), None, None)
            .expect("partial limits should parse");
        let limits = builder.metrics_header_limits();
        assert_eq!(
            limits.forwarded_header_value_bytes(),
            MetricsHeaderLimits::DEFAULT_FORWARDED_HEADER_VALUE_BYTES
        );
        assert_eq!(limits.x_forwarded_for_header_value_bytes(), 4096);
        assert_eq!(
            limits.x_real_ip_header_value_bytes(),
            MetricsHeaderLimits::DEFAULT_X_REAL_IP_HEADER_VALUE_BYTES
        );
        assert_eq!(
            limits.authorization_header_value_bytes(),
            MetricsHeaderLimits::DEFAULT_AUTHORIZATION_HEADER_VALUE_BYTES
        );
    }

    #[test]
    fn builder_with_bearer_auth_for_tenant() {
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("tenant-token", "tenant-1");
        assert_eq!(builder.bearer_auth_token(), Some("tenant-token"));
        assert_eq!(builder.bound_tenant_id.as_deref(), Some("tenant-1"));
    }

    #[test]
    fn builder_without_auth() {
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("token", "tenant-1")
            .without_auth();
        assert!(builder.bearer_auth_token().is_none());
        assert!(builder.metrics_bearer_auth_token().is_none());
        assert!(builder.bound_tenant_id.is_none());
    }

    #[test]
    fn builder_without_metrics_auth() {
        let builder = ServerBuilder::new(test_commerce()).without_metrics_auth();
        assert!(builder.bearer_auth_token().is_some());
        assert!(builder.metrics_bearer_auth_token().is_none());
    }

    #[test]
    fn builder_without_metrics_ip_allowlist() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap()])
            .without_metrics_ip_allowlist();
        assert!(builder.metrics_ip_allowlist().is_none());
    }

    #[test]
    fn builder_without_metrics_ip_cidr_allowlist() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_ip_cidr_allowlist(["10.0.0.0/8".parse().unwrap()])
            .without_metrics_ip_cidr_allowlist();
        assert!(builder.metrics_ip_cidr_allowlist().is_none());
    }

    #[test]
    fn builder_without_metrics_trusted_proxies() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_trusted_proxies(["10.0.0.0/8".parse().unwrap()])
            .without_metrics_trusted_proxies();
        assert!(builder.metrics_trusted_proxies().is_none());
    }

    #[test]
    fn builder_chaining() {
        let addr: SocketAddr = "0.0.0.0:9090".parse().unwrap();
        let builder = ServerBuilder::new(test_commerce())
            .bind(addr)
            .with_cors()
            .with_request_id()
            .with_bearer_auth("chain-token")
            .bind_auth_tenant("chain-tenant");
        assert_eq!(builder.addr, addr);
        assert!(builder.enable_cors);
        assert!(builder.enable_request_id);
        assert_eq!(builder.bearer_auth_token(), Some("chain-token"));
        assert_eq!(builder.bound_tenant_id.as_deref(), Some("chain-tenant"));
    }

    #[test]
    fn builder_with_max_tenant_dbs() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-builder-{}", Uuid::new_v4()));
        let builder = ServerBuilder::new(test_commerce())
            .with_tenant_db_dir(tenant_dir.clone())
            .with_max_tenant_dbs(1);

        let tenant_a = builder.state.commerce_for_tenant(Some("tenant-a")).unwrap();
        let second_while_in_use = builder.state.commerce_for_tenant(Some("tenant-b"));
        assert!(matches!(second_while_in_use, Err(HttpError::TooManyRequests(_))));

        drop(tenant_a);
        let second_after_release = builder.state.commerce_for_tenant(Some("tenant-b"));
        assert!(second_after_release.is_ok());

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[test]
    fn builder_builds_router() {
        let _router = ServerBuilder::new(test_commerce()).build();
    }

    #[tokio::test]
    async fn built_router_serves_health() {
        let router = ServerBuilder::new(test_commerce()).with_cors().with_request_id().build();

        let resp =
            router.oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_serves_api_orders() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_serves_api_customers() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/customers").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_serves_api_products() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/products").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_404_for_unknown_path() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp = router
            .oneshot(Request::get("/api/v1/nonexistent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn builder_debug_impl() {
        let builder = ServerBuilder::new(test_commerce());
        let dbg = format!("{builder:?}");
        assert!(dbg.contains("ServerBuilder"));
        assert!(dbg.contains("<redacted>"));
    }

    #[tokio::test]
    async fn built_router_blocks_api_without_token_by_default() {
        let router = ServerBuilder::new(test_commerce()).build();

        let resp = router
            .oneshot(Request::get("/api/v1/orders").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn built_router_allows_api_with_token() {
        let builder = ServerBuilder::new(test_commerce());
        let token =
            builder.bearer_auth_token().expect("default auth token should be present").to_string();
        let router = builder.build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_blocks_metrics_without_token_by_default() {
        let router = ServerBuilder::new(test_commerce()).build();

        let resp =
            router.oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn built_router_allows_metrics_with_token() {
        let builder = ServerBuilder::new(test_commerce());
        let token = builder
            .metrics_bearer_auth_token()
            .expect("default metrics token should be present")
            .to_string();
        let router = builder.build();

        let resp = router
            .oneshot(
                Request::get("/metrics")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_allows_metrics_without_token_when_disabled() {
        let router = ServerBuilder::new(test_commerce()).without_metrics_auth().build();

        let resp =
            router.oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_router_rejects_mismatched_tenant_for_bound_token() {
        let router = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("bound-token", "tenant-1")
            .build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer bound-token")
                    .header("x-tenant-id", "tenant-2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn serve_refuses_public_bind_without_auth() {
        let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let err = ServerBuilder::new(test_commerce())
            .bind(addr)
            .without_auth()
            .serve()
            .await
            .expect_err("should reject public bind without auth");

        assert!(err.to_string().contains("Refusing to start without API auth"));
    }
}
