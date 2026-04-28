//! Server builder for configuring and running the HTTP service.

use std::{
    collections::HashSet,
    fmt,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use axum::Router;
use stateset_authz::AuthzEngine;
use stateset_embedded::Commerce;
use uuid::Uuid;

use crate::error::HttpError;
use crate::middleware::{self, AuthzConfig, BearerAuthBinding, RateLimitConfig};
use crate::routes::{self, DEFAULT_REQUEST_BODY_LIMIT_BYTES};
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
const REQUEST_BODY_MAX_BYTES_ENV: &str = "STATESET_HTTP_MAX_BODY_BYTES";

#[derive(Clone, Debug, PartialEq, Eq)]
struct AdditionalApiBearerBinding {
    token: String,
    bound_tenant_id: Option<String>,
    bound_actor_id: Option<String>,
}

impl AdditionalApiBearerBinding {
    const fn new(
        token: String,
        bound_tenant_id: Option<String>,
        bound_actor_id: Option<String>,
    ) -> Self {
        Self { token, bound_tenant_id, bound_actor_id }
    }
}

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
    bound_actor_id: Option<String>,
    additional_api_bearer_bindings: Vec<AdditionalApiBearerBinding>,
    generated_default_token: bool,
    max_request_body_bytes: usize,
    authz_config: Option<AuthzConfig>,
    trust_actor_headers_for_authz: bool,
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
            .field("bound_actor_id", &self.bound_actor_id.as_ref().map(|_| "<redacted>"))
            .field("additional_api_bearer_bindings", &self.additional_api_bearer_bindings.len())
            .field("generated_default_token", &self.generated_default_token)
            .field("max_request_body_bytes", &self.max_request_body_bytes)
            .field("authz_enabled", &self.authz_config.is_some())
            .field("trust_actor_headers_for_authz", &self.trust_actor_headers_for_authz)
            .field("rate_limit", &self.rate_limit)
            .finish()
    }
}

impl ServerBuilder {
    fn api_bearer_bindings(&self) -> Vec<AdditionalApiBearerBinding> {
        let mut bindings = Vec::with_capacity(
            usize::from(self.api_bearer_token.is_some())
                + self.additional_api_bearer_bindings.len(),
        );
        if let Some(token) = self.api_bearer_token.clone() {
            bindings.push(AdditionalApiBearerBinding::new(
                token,
                self.bound_tenant_id.clone(),
                self.bound_actor_id.clone(),
            ));
        }
        bindings.extend(self.additional_api_bearer_bindings.iter().cloned());
        bindings
    }

    fn tenant_routing_auth_error(&self) -> Option<&'static str> {
        self.state.tenant_db_dir()?;
        let bindings = self.api_bearer_bindings();
        if bindings.is_empty() {
            return Some("per-tenant database routing requires API auth to remain enabled");
        }
        if bindings.iter().any(|binding| binding.bound_tenant_id.is_none()) {
            return Some(
                "per-tenant database routing requires binding every API bearer token to a single tenant",
            );
        }
        None
    }

    fn api_auth_error(&self) -> Option<String> {
        if let Some(message) = self.tenant_routing_auth_error() {
            return Some(message.to_string());
        }
        let bindings = self.api_bearer_bindings();
        if bindings.is_empty() {
            if self.authz_config.is_some() && !self.trust_actor_headers_for_authz {
                return Some(
                    "request authorization requires actor-bound API authentication or explicitly trusted x-actor-id headers"
                        .to_string(),
                );
            }
            if self.bound_actor_id.is_some() {
                return Some(
                    "actor-bound API authentication requires API auth to remain enabled"
                        .to_string(),
                );
            }
            return None;
        }

        let mut seen_tokens = HashSet::with_capacity(bindings.len());
        for binding in &bindings {
            if !seen_tokens.insert(binding.token.clone()) {
                return Some("duplicate API bearer tokens are not allowed".to_string());
            }
            if let Some(bound_actor_id) = binding.bound_actor_id.as_deref() {
                if !middleware::is_valid_actor_id(bound_actor_id) {
                    return Some("bound actor ID is invalid".to_string());
                }
            }
        }
        if self.authz_config.is_some()
            && !self.trust_actor_headers_for_authz
            && bindings.iter().any(|binding| binding.bound_actor_id.is_none())
        {
            return Some(
                "request authorization requires binding every API bearer token to a single actor or explicitly trusting x-actor-id headers"
                    .to_string(),
            );
        }
        None
    }

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
            bound_actor_id: None,
            additional_api_bearer_bindings: Vec::new(),
            generated_default_token: true,
            max_request_body_bytes: DEFAULT_REQUEST_BODY_LIMIT_BYTES,
            authz_config: None,
            trust_actor_headers_for_authz: false,
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
            .with_metrics_header_limits_from_env()?
            .with_request_body_limit_from_env()
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
        self.api_bearer_token = Some(token.into());
        self.generated_default_token = false;
        self
    }

    fn push_bearer_auth_binding(
        &mut self,
        token: impl Into<String>,
        bound_tenant_id: Option<String>,
        bound_actor_id: Option<String>,
    ) {
        self.additional_api_bearer_bindings.push(AdditionalApiBearerBinding::new(
            token.into(),
            bound_tenant_id,
            bound_actor_id,
        ));
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

    /// Apply the request body size limit from `STATESET_HTTP_MAX_BODY_BYTES`.
    pub fn with_request_body_limit_from_env(self) -> Result<Self, HttpError> {
        let max_body_bytes = std::env::var(REQUEST_BODY_MAX_BYTES_ENV).ok();
        self.with_request_body_limit_from_value(max_body_bytes.as_deref())
    }

    fn with_request_body_limit_from_value(
        mut self,
        max_body_bytes: Option<&str>,
    ) -> Result<Self, HttpError> {
        if let Some(raw) = max_body_bytes {
            self.max_request_body_bytes =
                parse_positive_usize_env(REQUEST_BODY_MAX_BYTES_ENV, raw)?;
        }
        Ok(self)
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

    /// Bind the configured bearer token to a single actor.
    ///
    /// When set, authenticated API requests are treated as this actor during
    /// authorization checks, and any conflicting `x-actor-id` header is rejected.
    #[must_use]
    pub fn bind_auth_actor(mut self, actor_id: impl Into<String>) -> Self {
        self.bound_actor_id = Some(actor_id.into());
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

    /// Configure bearer authentication and bind it to an actor in one call.
    #[must_use]
    pub fn with_bearer_auth_for_actor(
        self,
        token: impl Into<String>,
        actor_id: impl Into<String>,
    ) -> Self {
        self.with_bearer_auth(token).bind_auth_actor(actor_id)
    }

    /// Add an additional bearer token for `/api/v1/*` endpoints.
    ///
    /// This leaves the primary configured API token unchanged.
    #[must_use]
    pub fn add_bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.push_bearer_auth_binding(token, None, None);
        self
    }

    /// Add an additional bearer token bound to a tenant.
    #[must_use]
    pub fn add_bearer_auth_for_tenant(
        mut self,
        token: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        self.push_bearer_auth_binding(token, Some(tenant_id.into()), None);
        self
    }

    /// Add an additional bearer token bound to an actor.
    #[must_use]
    pub fn add_bearer_auth_for_actor(
        mut self,
        token: impl Into<String>,
        actor_id: impl Into<String>,
    ) -> Self {
        self.push_bearer_auth_binding(token, None, Some(actor_id.into()));
        self
    }

    /// Add an additional bearer token bound to both an actor and a tenant.
    #[must_use]
    pub fn add_bearer_auth_for_actor_and_tenant(
        mut self,
        token: impl Into<String>,
        actor_id: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        self.push_bearer_auth_binding(token, Some(tenant_id.into()), Some(actor_id.into()));
        self
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

    /// Set the maximum accepted request body size for extractor-based endpoints.
    #[must_use]
    pub const fn with_max_request_body_bytes(mut self, max_request_body_bytes: usize) -> Self {
        self.max_request_body_bytes =
            if max_request_body_bytes == 0 { 1 } else { max_request_body_bytes };
        self
    }

    /// Enable request authorization for `/api/v1/*` using a provided authz engine.
    ///
    /// By default, authorization expects actor identity to come from actor-bound
    /// bearer tokens. If you want to trust `x-actor-id` request headers instead,
    /// also call [`Self::trust_actor_headers_for_authz`].
    #[must_use]
    pub fn with_authz_engine(mut self, engine: AuthzEngine) -> Self {
        self.authz_config = Some(AuthzConfig::new(engine));
        self
    }

    /// Explicitly trust `x-actor-id` request headers for authorization.
    ///
    /// Use this only behind a trusted upstream that authenticates callers and
    /// strips or overwrites any client-supplied actor header.
    #[must_use]
    pub const fn trust_actor_headers_for_authz(mut self) -> Self {
        self.trust_actor_headers_for_authz = true;
        self
    }

    /// Disable API authentication (not recommended for untrusted networks).
    #[must_use]
    pub fn without_auth(mut self) -> Self {
        self.api_bearer_token = None;
        self.bound_tenant_id = None;
        self.bound_actor_id = None;
        self.additional_api_bearer_bindings.clear();
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
        let misconfigured_auth_message = self.api_auth_error();
        let auth_bindings = self
            .api_bearer_bindings()
            .into_iter()
            .map(|binding| {
                BearerAuthBinding::new(
                    binding.token,
                    binding.bound_tenant_id,
                    binding.bound_actor_id,
                )
            })
            .collect::<Vec<_>>();
        let auth_config = if auth_bindings.is_empty() { None } else { Some(auth_bindings) };
        let trust_actor_headers_for_authz = self.trust_actor_headers_for_authz;
        let authz_config = self.authz_config.map(|config| {
            if trust_actor_headers_for_authz { config.with_trusted_actor_headers() } else { config }
        });
        let router =
            routes::api_router_with_body_limit(self.max_request_body_bytes).with_state(self.state);
        middleware::apply_middleware(
            router,
            self.enable_cors,
            self.enable_request_id,
            auth_config,
            authz_config,
            misconfigured_auth_message,
            self.rate_limit,
        )
    }

    /// Build the router and start serving HTTP requests.
    ///
    /// This method will block until the server is shut down.
    pub async fn serve(self) -> Result<(), HttpError> {
        let token = self.api_bearer_token.clone();
        let api_token_count = self.api_bearer_bindings().len();
        let metrics_token = self.state.metrics_bearer_auth_token().map(ToOwned::to_owned);
        let trusted_proxy_count = self.state.metrics_trusted_proxies().map_or(0, |v| v.len());
        let metrics_header_limits = self.state.metrics_header_limits();
        let bound_tenant_id = self.bound_tenant_id.clone();
        let bound_actor_id = self.bound_actor_id.clone();
        let generated_default_token = self.generated_default_token;
        let authz_enabled = self.authz_config.is_some();
        let trust_actor_headers_for_authz = self.trust_actor_headers_for_authz;
        let addr = self.addr;

        if api_token_count == 0 && !addr.ip().is_loopback() {
            return Err(HttpError::BadRequest(
                "Refusing to start without API auth on a non-loopback address".to_string(),
            ));
        }

        if let Some(message) = self.api_auth_error() {
            return Err(HttpError::BadRequest(format!("Refusing to start: {message}")));
        }

        let app = self.build();

        tracing::info!("StateSet HTTP listening on {addr}");
        if let Some(token) = token.as_deref() {
            tracing::info!("API bearer authentication is enabled for /api/v1/*");
            if api_token_count > 1 {
                tracing::info!(api_token_count, "Multiple API bearer tokens are configured");
            }
            if let Some(bound_tenant_id) = bound_tenant_id.as_deref() {
                tracing::info!(
                    tenant_id = %bound_tenant_id,
                    "API token is bound to a specific tenant"
                );
            }
            if let Some(bound_actor_id) = bound_actor_id.as_deref() {
                tracing::info!(
                    actor_id = %bound_actor_id,
                    "API token is bound to a specific actor"
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
        } else if api_token_count > 0 {
            tracing::info!("API bearer authentication is enabled for /api/v1/*");
            tracing::info!(api_token_count, "Multiple API bearer tokens are configured");
        } else {
            tracing::warn!("API authentication is disabled for /api/v1/*");
        }
        if authz_enabled && bound_actor_id.is_none() && trust_actor_headers_for_authz {
            tracing::warn!(
                "Request authorization is enabled for /api/v1/*; ensure x-actor-id is set by a trusted upstream"
            );
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
    use stateset_authz::{AuthzEngineBuilder, Role};
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
        assert!(builder.bound_actor_id.is_none());
        assert!(builder.additional_api_bearer_bindings.is_empty());
        assert_eq!(builder.max_request_body_bytes, DEFAULT_REQUEST_BODY_LIMIT_BYTES);
        assert!(builder.authz_config.is_none());
        assert!(!builder.trust_actor_headers_for_authz);
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
        let builder = ServerBuilder::new(test_commerce());
        let default_metrics_token = builder
            .metrics_bearer_auth_token()
            .expect("builder should start with metrics auth")
            .to_string();
        let builder = builder.with_bearer_auth("test-token");
        assert_eq!(builder.bearer_auth_token(), Some("test-token"));
        assert_eq!(builder.metrics_bearer_auth_token(), Some(default_metrics_token.as_str()));
        assert!(builder.bound_tenant_id.is_none());
        assert!(builder.bound_actor_id.is_none());
    }

    #[test]
    fn builder_with_metrics_bearer_auth() {
        let builder = ServerBuilder::new(test_commerce()).with_metrics_bearer_auth("metrics-token");
        assert_eq!(builder.metrics_bearer_auth_token(), Some("metrics-token"));
    }

    #[test]
    fn builder_with_bearer_auth_keeps_explicit_metrics_token() {
        let builder = ServerBuilder::new(test_commerce())
            .with_metrics_bearer_auth("metrics-token")
            .with_bearer_auth("api-token");
        assert_eq!(builder.bearer_auth_token(), Some("api-token"));
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
    fn builder_with_request_body_limit() {
        let builder = ServerBuilder::new(test_commerce()).with_max_request_body_bytes(4096);
        assert_eq!(builder.max_request_body_bytes, 4096);
    }

    #[test]
    fn builder_with_request_body_limit_from_value() {
        let builder = ServerBuilder::new(test_commerce())
            .with_request_body_limit_from_value(Some("8192"))
            .expect("request body limit should parse");
        assert_eq!(builder.max_request_body_bytes, 8192);
    }

    #[test]
    fn builder_with_authz_engine() {
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::viewer())
            .assign_role("viewer-1", "viewer")
            .build();
        let builder = ServerBuilder::new(test_commerce()).with_authz_engine(engine);
        assert!(builder.authz_config.is_some());
    }

    #[test]
    fn builder_trusts_actor_headers_for_authz() {
        let builder = ServerBuilder::new(test_commerce()).trust_actor_headers_for_authz();
        assert!(builder.trust_actor_headers_for_authz);
    }

    #[test]
    fn builder_with_bearer_auth_for_actor() {
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_actor("actor-token", "admin-1");
        assert_eq!(builder.bearer_auth_token(), Some("actor-token"));
        assert_eq!(builder.bound_actor_id.as_deref(), Some("admin-1"));
    }

    #[test]
    fn builder_adds_additional_actor_bound_token() {
        let builder = ServerBuilder::new(test_commerce())
            .without_auth()
            .add_bearer_auth_for_actor("viewer-token", "viewer-1");
        assert!(builder.bearer_auth_token().is_none());
        assert_eq!(builder.additional_api_bearer_bindings.len(), 1);
        assert_eq!(builder.additional_api_bearer_bindings[0].token, "viewer-token");
        assert_eq!(
            builder.additional_api_bearer_bindings[0].bound_actor_id.as_deref(),
            Some("viewer-1")
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
            .bind_auth_actor("admin-1")
            .without_auth();
        assert!(builder.bearer_auth_token().is_none());
        assert!(builder.metrics_bearer_auth_token().is_some());
        assert!(builder.bound_tenant_id.is_none());
        assert!(builder.bound_actor_id.is_none());
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
            .bind_auth_tenant("chain-tenant")
            .bind_auth_actor("chain-actor");
        assert_eq!(builder.addr, addr);
        assert!(builder.enable_cors);
        assert!(builder.enable_request_id);
        assert_eq!(builder.bearer_auth_token(), Some("chain-token"));
        assert_eq!(builder.bound_tenant_id.as_deref(), Some("chain-tenant"));
        assert_eq!(builder.bound_actor_id.as_deref(), Some("chain-actor"));
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
    async fn built_router_fails_closed_for_unbound_tenant_routing() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-misconfig-{}", Uuid::new_v4()));
        let builder = ServerBuilder::new(test_commerce()).with_tenant_db_dir(tenant_dir.clone());
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
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[tokio::test]
    async fn built_router_fails_closed_for_invalid_bound_actor() {
        let builder = ServerBuilder::new(test_commerce()).bind_auth_actor("invalid actor");
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
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn built_router_fails_closed_for_actor_binding_without_auth() {
        let router =
            ServerBuilder::new(test_commerce()).without_auth().bind_auth_actor("admin-1").build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn built_router_fails_closed_for_duplicate_api_tokens() {
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth("duplicate-token")
            .add_bearer_auth_for_actor("duplicate-token", "viewer-1");
        let router = builder.build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer duplicate-token")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn built_router_fails_closed_for_authz_without_actor_bound_tokens_or_trusted_headers() {
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::viewer())
            .assign_role("viewer-1", "viewer")
            .build();
        let builder = ServerBuilder::new(test_commerce()).with_authz_engine(engine);
        let token =
            builder.bearer_auth_token().expect("default auth token should be present").to_string();
        let router = builder.build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-tenant-id", "tenant-1")
                    .header("x-actor-id", "viewer-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn built_router_fails_closed_for_tenant_routing_with_unbound_additional_token() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-extra-token-{}", Uuid::new_v4()));
        let builder = ServerBuilder::new(test_commerce())
            .with_bearer_auth_for_tenant("primary-token", "tenant-1")
            .add_bearer_auth_for_actor("viewer-token", "viewer-1")
            .with_tenant_db_dir(tenant_dir.clone());
        let router = builder.build();

        let resp = router
            .oneshot(
                Request::get("/api/v1/orders")
                    .header("authorization", "Bearer viewer-token")
                    .header("x-tenant-id", "tenant-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let _ = std::fs::remove_dir_all(tenant_dir);
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
    async fn built_router_keeps_metrics_protected_after_without_auth() {
        let router = ServerBuilder::new(test_commerce()).without_auth().build();

        let resp =
            router.oneshot(Request::get("/metrics").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
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

    #[tokio::test]
    async fn serve_refuses_tenant_routing_without_tenant_binding() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-serve-misconfig-{}", Uuid::new_v4()));
        let err = ServerBuilder::new(test_commerce())
            .with_tenant_db_dir(tenant_dir.clone())
            .serve()
            .await
            .expect_err("should reject unbound tenant routing");

        assert!(err.to_string().contains("binding every API bearer token"));
        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[tokio::test]
    async fn serve_refuses_tenant_routing_without_auth() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-serve-no-auth-{}", Uuid::new_v4()));
        let err = ServerBuilder::new(test_commerce())
            .with_tenant_db_dir(tenant_dir.clone())
            .without_auth()
            .serve()
            .await
            .expect_err("should reject tenant routing without auth");

        assert!(err.to_string().contains("requires API auth"));
        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[tokio::test]
    async fn serve_refuses_authz_without_actor_bound_tokens_or_trusted_headers() {
        let engine = AuthzEngineBuilder::new()
            .add_role(Role::viewer())
            .assign_role("viewer-1", "viewer")
            .build();
        let err = ServerBuilder::new(test_commerce())
            .with_authz_engine(engine)
            .serve()
            .await
            .expect_err("should reject authz configuration without actor identity source");

        assert!(err.to_string().contains("binding every API bearer token"));
    }
}
