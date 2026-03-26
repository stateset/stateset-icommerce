//! Application state shared across all route handlers.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    net::IpAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::http::HeaderMap;
use stateset_embedded::Commerce;

use crate::{error::HttpError, middleware::X_TENANT_ID};

/// Default cap for lazily created per-tenant databases.
pub(crate) const DEFAULT_MAX_TENANT_DBS: usize = 256;

#[derive(Debug)]
struct TenantCacheEntry {
    commerce: Arc<Commerce>,
    last_access_tick: u64,
}

/// Snapshot of per-tenant cache behavior used by health endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantCacheMetrics {
    /// Whether per-tenant routing is enabled.
    pub enabled: bool,
    /// Maximum active tenant databases kept in the cache.
    pub max_cached_dbs: usize,
    /// Number of tenant databases currently cached.
    pub cached_dbs: usize,
    /// Number of cached tenant databases currently in use by requests.
    pub in_use_cached_dbs: usize,
    /// Number of cache hits for tenant resolution.
    pub hits: u64,
    /// Number of cache misses for tenant resolution.
    pub misses: u64,
    /// Number of idle tenant cache entries evicted due to capacity.
    pub evictions: u64,
    /// Number of rejections when all cache entries were in use at capacity.
    pub rejections: u64,
}

/// Snapshot of `/metrics` access outcomes for observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MetricsAccessMetrics {
    pub(crate) requests_total: u64,
    pub(crate) allowed_total: u64,
    pub(crate) allowed_peer_total: u64,
    pub(crate) allowed_forwarded_trusted_proxy_total: u64,
    pub(crate) allowed_forwarded_without_peer_total: u64,
    pub(crate) allowed_unavailable_total: u64,
    pub(crate) denied_ip_total: u64,
    pub(crate) denied_ip_not_allowed_total: u64,
    pub(crate) denied_missing_peer_ip_with_trusted_proxies_total: u64,
    pub(crate) denied_auth_total: u64,
    pub(crate) denied_auth_header_missing_total: u64,
    pub(crate) denied_auth_header_invalid_total: u64,
    pub(crate) denied_auth_header_invalid_encoding_total: u64,
    pub(crate) denied_auth_header_invalid_scheme_total: u64,
    pub(crate) denied_auth_header_malformed_total: u64,
    pub(crate) denied_auth_header_multiple_total: u64,
    pub(crate) denied_auth_header_oversized_total: u64,
    pub(crate) denied_auth_token_mismatch_total: u64,
    pub(crate) denied_forwarded_missing_total: u64,
    pub(crate) denied_forwarded_invalid_total: u64,
    pub(crate) denied_forwarded_oversized_total: u64,
}

/// Maximum accepted byte lengths for metrics-related forwarding headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsHeaderLimits {
    forwarded_header_value_bytes: usize,
    x_forwarded_for_header_value_bytes: usize,
    x_real_ip_header_value_bytes: usize,
    authorization_header_value_bytes: usize,
}

impl MetricsHeaderLimits {
    /// Default max byte length for the standard `Forwarded` header.
    pub const DEFAULT_FORWARDED_HEADER_VALUE_BYTES: usize = 2048;
    /// Default max byte length for `X-Forwarded-For`.
    pub const DEFAULT_X_FORWARDED_FOR_HEADER_VALUE_BYTES: usize = 2048;
    /// Default max byte length for `X-Real-IP`.
    pub const DEFAULT_X_REAL_IP_HEADER_VALUE_BYTES: usize = 512;
    /// Default max byte length for `Authorization`.
    pub const DEFAULT_AUTHORIZATION_HEADER_VALUE_BYTES: usize = 2048;

    /// Create a custom header limit set. All values must be greater than zero.
    pub fn new(
        forwarded_header_value_bytes: usize,
        x_forwarded_for_header_value_bytes: usize,
        x_real_ip_header_value_bytes: usize,
    ) -> Result<Self, HttpError> {
        Self::new_with_authorization(
            forwarded_header_value_bytes,
            x_forwarded_for_header_value_bytes,
            x_real_ip_header_value_bytes,
            Self::DEFAULT_AUTHORIZATION_HEADER_VALUE_BYTES,
        )
    }

    /// Create a custom header limit set including Authorization.
    ///
    /// All values must be greater than zero.
    pub fn new_with_authorization(
        forwarded_header_value_bytes: usize,
        x_forwarded_for_header_value_bytes: usize,
        x_real_ip_header_value_bytes: usize,
        authorization_header_value_bytes: usize,
    ) -> Result<Self, HttpError> {
        if forwarded_header_value_bytes == 0 {
            return Err(HttpError::BadRequest(
                "metrics forwarded header limit must be greater than zero".to_string(),
            ));
        }
        if x_forwarded_for_header_value_bytes == 0 {
            return Err(HttpError::BadRequest(
                "metrics x-forwarded-for header limit must be greater than zero".to_string(),
            ));
        }
        if x_real_ip_header_value_bytes == 0 {
            return Err(HttpError::BadRequest(
                "metrics x-real-ip header limit must be greater than zero".to_string(),
            ));
        }
        if authorization_header_value_bytes == 0 {
            return Err(HttpError::BadRequest(
                "metrics authorization header limit must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            forwarded_header_value_bytes,
            x_forwarded_for_header_value_bytes,
            x_real_ip_header_value_bytes,
            authorization_header_value_bytes,
        })
    }

    /// Max byte length for `Forwarded`.
    #[must_use]
    pub const fn forwarded_header_value_bytes(self) -> usize {
        self.forwarded_header_value_bytes
    }

    /// Max byte length for `X-Forwarded-For`.
    #[must_use]
    pub const fn x_forwarded_for_header_value_bytes(self) -> usize {
        self.x_forwarded_for_header_value_bytes
    }

    /// Max byte length for `X-Real-IP`.
    #[must_use]
    pub const fn x_real_ip_header_value_bytes(self) -> usize {
        self.x_real_ip_header_value_bytes
    }

    /// Max byte length for `Authorization`.
    #[must_use]
    pub const fn authorization_header_value_bytes(self) -> usize {
        self.authorization_header_value_bytes
    }
}

impl Default for MetricsHeaderLimits {
    fn default() -> Self {
        Self {
            forwarded_header_value_bytes: Self::DEFAULT_FORWARDED_HEADER_VALUE_BYTES,
            x_forwarded_for_header_value_bytes: Self::DEFAULT_X_FORWARDED_FOR_HEADER_VALUE_BYTES,
            x_real_ip_header_value_bytes: Self::DEFAULT_X_REAL_IP_HEADER_VALUE_BYTES,
            authorization_header_value_bytes: Self::DEFAULT_AUTHORIZATION_HEADER_VALUE_BYTES,
        }
    }
}

/// CIDR block used for trusted proxy checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IpCidr {
    network: IpAddr,
    prefix_len: u8,
}

impl IpCidr {
    /// Creates a CIDR network from an IP and prefix length.
    pub fn new(network: IpAddr, prefix_len: u8) -> Result<Self, HttpError> {
        let max_prefix = match network {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix_len > max_prefix {
            return Err(HttpError::BadRequest(format!(
                "invalid CIDR prefix length {prefix_len} for {network}"
            )));
        }
        Ok(Self { network, prefix_len })
    }

    /// Returns true if `ip` is contained in this CIDR range.
    #[must_use]
    pub fn contains(self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                let mask = if self.prefix_len == 0 {
                    0
                } else {
                    u32::MAX << (32 - u32::from(self.prefix_len))
                };
                (u32::from(network) & mask) == (u32::from(ip) & mask)
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                let mask = if self.prefix_len == 0 {
                    0
                } else {
                    u128::MAX << (128 - u32::from(self.prefix_len))
                };
                (u128::from(network) & mask) == (u128::from(ip) & mask)
            }
            _ => false,
        }
    }

    /// Returns the CIDR prefix length.
    #[must_use]
    pub const fn prefix_len(self) -> u8 {
        self.prefix_len
    }

    /// Returns the CIDR network base IP.
    #[must_use]
    pub const fn network(self) -> IpAddr {
        self.network
    }
}

impl fmt::Display for IpCidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix_len)
    }
}

impl FromStr for IpCidr {
    type Err = HttpError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Err(HttpError::BadRequest("CIDR cannot be empty".to_string()));
        }

        if let Some((ip, prefix)) = value.split_once('/') {
            let network = ip.trim().parse::<IpAddr>().map_err(|error| {
                HttpError::BadRequest(format!("invalid CIDR address '{}': {error}", ip.trim()))
            })?;
            let prefix_len = prefix.trim().parse::<u8>().map_err(|error| {
                HttpError::BadRequest(format!(
                    "invalid CIDR prefix '{}' in '{}': {error}",
                    prefix.trim(),
                    value
                ))
            })?;
            Self::new(network, prefix_len)
        } else {
            let ip = value.parse::<IpAddr>().map_err(|error| {
                HttpError::BadRequest(format!("invalid IP address '{value}': {error}"))
            })?;
            let prefix = match ip {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            };
            Self::new(ip, prefix)
        }
    }
}

/// Shared application state injected into every route handler via
/// [`axum::extract::State`].
#[derive(Clone)]
pub struct AppState {
    commerce: Arc<Commerce>,
    tenant_db_dir: Option<Arc<PathBuf>>,
    tenant_cache: Arc<RwLock<HashMap<String, TenantCacheEntry>>>,
    tenant_access_clock: Arc<AtomicU64>,
    tenant_cache_hits: Arc<AtomicU64>,
    tenant_cache_misses: Arc<AtomicU64>,
    tenant_cache_evictions: Arc<AtomicU64>,
    tenant_cache_rejections: Arc<AtomicU64>,
    metrics_scrape_requests: Arc<AtomicU64>,
    metrics_scrape_allowed: Arc<AtomicU64>,
    metrics_scrape_allowed_peer: Arc<AtomicU64>,
    metrics_scrape_allowed_forwarded_trusted_proxy: Arc<AtomicU64>,
    metrics_scrape_allowed_forwarded_without_peer: Arc<AtomicU64>,
    metrics_scrape_allowed_unavailable: Arc<AtomicU64>,
    metrics_scrape_denied_ip: Arc<AtomicU64>,
    metrics_scrape_denied_ip_not_allowed: Arc<AtomicU64>,
    metrics_scrape_denied_missing_peer_ip_with_trusted_proxies: Arc<AtomicU64>,
    metrics_scrape_denied_auth: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_missing: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_invalid: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_invalid_encoding: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_invalid_scheme: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_malformed: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_multiple: Arc<AtomicU64>,
    metrics_scrape_denied_auth_header_oversized: Arc<AtomicU64>,
    metrics_scrape_denied_auth_token_mismatch: Arc<AtomicU64>,
    metrics_scrape_denied_forwarded_missing: Arc<AtomicU64>,
    metrics_scrape_denied_forwarded_invalid: Arc<AtomicU64>,
    metrics_scrape_denied_forwarded_oversized: Arc<AtomicU64>,
    metrics_bearer_token: Option<Arc<str>>,
    metrics_ip_allowlist: Option<Arc<HashSet<IpAddr>>>,
    metrics_ip_cidr_allowlist: Option<Arc<Vec<IpCidr>>>,
    metrics_trusted_proxies: Option<Arc<Vec<IpCidr>>>,
    metrics_header_limits: MetricsHeaderLimits,
    max_tenant_dbs: usize,
}

impl fmt::Debug for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState")
            .field("commerce", &"Commerce { .. }")
            .field(
                "tenant_db_dir",
                &self.tenant_db_dir.as_deref().map(|path| path.display().to_string()),
            )
            .field(
                "metrics_bearer_token",
                &self.metrics_bearer_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "metrics_ip_allowlist_size",
                &self.metrics_ip_allowlist.as_ref().map(|allowlist| allowlist.len()),
            )
            .field(
                "metrics_ip_cidr_allowlist_size",
                &self.metrics_ip_cidr_allowlist.as_ref().map(|allowlist| allowlist.len()),
            )
            .field(
                "metrics_trusted_proxies_size",
                &self.metrics_trusted_proxies.as_ref().map(|proxies| proxies.len()),
            )
            .field("metrics_scrape_requests", &self.metrics_scrape_requests.load(Ordering::Relaxed))
            .field("metrics_scrape_allowed", &self.metrics_scrape_allowed.load(Ordering::Relaxed))
            .field(
                "metrics_scrape_allowed_peer",
                &self.metrics_scrape_allowed_peer.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_allowed_forwarded_trusted_proxy",
                &self.metrics_scrape_allowed_forwarded_trusted_proxy.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_allowed_forwarded_without_peer",
                &self.metrics_scrape_allowed_forwarded_without_peer.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_allowed_unavailable",
                &self.metrics_scrape_allowed_unavailable.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_ip",
                &self.metrics_scrape_denied_ip.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_ip_not_allowed",
                &self.metrics_scrape_denied_ip_not_allowed.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_missing_peer_ip_with_trusted_proxies",
                &self
                    .metrics_scrape_denied_missing_peer_ip_with_trusted_proxies
                    .load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth",
                &self.metrics_scrape_denied_auth.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_missing",
                &self.metrics_scrape_denied_auth_header_missing.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_invalid",
                &self.metrics_scrape_denied_auth_header_invalid.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_invalid_encoding",
                &self.metrics_scrape_denied_auth_header_invalid_encoding.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_invalid_scheme",
                &self.metrics_scrape_denied_auth_header_invalid_scheme.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_malformed",
                &self.metrics_scrape_denied_auth_header_malformed.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_multiple",
                &self.metrics_scrape_denied_auth_header_multiple.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_header_oversized",
                &self.metrics_scrape_denied_auth_header_oversized.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_auth_token_mismatch",
                &self.metrics_scrape_denied_auth_token_mismatch.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_forwarded_missing",
                &self.metrics_scrape_denied_forwarded_missing.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_forwarded_invalid",
                &self.metrics_scrape_denied_forwarded_invalid.load(Ordering::Relaxed),
            )
            .field(
                "metrics_scrape_denied_forwarded_oversized",
                &self.metrics_scrape_denied_forwarded_oversized.load(Ordering::Relaxed),
            )
            .field("metrics_header_limits", &self.metrics_header_limits)
            .field("max_tenant_dbs", &self.max_tenant_dbs)
            .finish()
    }
}

fn is_valid_tenant_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return false;
    }
    trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

/// Parse `x-tenant-id` from request headers.
#[must_use]
pub(crate) fn tenant_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(&X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

impl AppState {
    /// Create a new `AppState` wrapping a [`Commerce`] instance.
    #[must_use]
    pub fn new(commerce: Commerce) -> Self {
        Self::new_with_tenant_db_dir(commerce, None)
    }

    /// Create a new `AppState` with optional per-tenant database directory.
    #[must_use]
    pub fn new_with_tenant_db_dir(commerce: Commerce, tenant_db_dir: Option<PathBuf>) -> Self {
        Self {
            commerce: Arc::new(commerce),
            tenant_db_dir: tenant_db_dir.map(Arc::new),
            tenant_cache: Arc::new(RwLock::new(HashMap::new())),
            tenant_access_clock: Arc::new(AtomicU64::new(0)),
            tenant_cache_hits: Arc::new(AtomicU64::new(0)),
            tenant_cache_misses: Arc::new(AtomicU64::new(0)),
            tenant_cache_evictions: Arc::new(AtomicU64::new(0)),
            tenant_cache_rejections: Arc::new(AtomicU64::new(0)),
            metrics_scrape_requests: Arc::new(AtomicU64::new(0)),
            metrics_scrape_allowed: Arc::new(AtomicU64::new(0)),
            metrics_scrape_allowed_peer: Arc::new(AtomicU64::new(0)),
            metrics_scrape_allowed_forwarded_trusted_proxy: Arc::new(AtomicU64::new(0)),
            metrics_scrape_allowed_forwarded_without_peer: Arc::new(AtomicU64::new(0)),
            metrics_scrape_allowed_unavailable: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_ip: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_ip_not_allowed: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_missing_peer_ip_with_trusted_proxies: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_missing: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_invalid: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_invalid_encoding: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_invalid_scheme: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_malformed: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_multiple: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_header_oversized: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_auth_token_mismatch: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_forwarded_missing: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_forwarded_invalid: Arc::new(AtomicU64::new(0)),
            metrics_scrape_denied_forwarded_oversized: Arc::new(AtomicU64::new(0)),
            metrics_bearer_token: None,
            metrics_ip_allowlist: None,
            metrics_ip_cidr_allowlist: None,
            metrics_trusted_proxies: None,
            metrics_header_limits: MetricsHeaderLimits::default(),
            max_tenant_dbs: DEFAULT_MAX_TENANT_DBS,
        }
    }

    /// Enable per-tenant SQLite routing under a base directory.
    #[must_use]
    pub fn with_tenant_db_dir(mut self, tenant_db_dir: impl Into<PathBuf>) -> Self {
        self.tenant_db_dir = Some(Arc::new(tenant_db_dir.into()));
        self
    }

    /// Set the maximum number of per-tenant databases kept in the active cache.
    ///
    /// When capacity is reached, the least-recently-used idle tenant engine is
    /// evicted to make room for a new tenant. If every cached tenant engine is
    /// currently in use, creation of an additional tenant database is rejected.
    #[must_use]
    pub fn with_max_tenant_dbs(mut self, max_tenant_dbs: usize) -> Self {
        self.max_tenant_dbs = max_tenant_dbs.max(1);
        self
    }

    /// Configure bearer authentication for the `/metrics` endpoint.
    #[must_use]
    pub fn with_metrics_bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.metrics_bearer_token = Some(Arc::<str>::from(token.into()));
        self
    }

    /// Disable authentication for `/metrics`.
    #[must_use]
    pub fn without_metrics_auth(mut self) -> Self {
        self.metrics_bearer_token = None;
        self
    }

    /// Configure a client IP allowlist for `/metrics`.
    ///
    /// Empty lists disable allowlist enforcement.
    #[must_use]
    pub fn with_metrics_ip_allowlist<I>(mut self, ips: I) -> Self
    where
        I: IntoIterator<Item = IpAddr>,
    {
        let set: HashSet<IpAddr> = ips.into_iter().collect();
        self.metrics_ip_allowlist = if set.is_empty() { None } else { Some(Arc::new(set)) };
        self
    }

    /// Disable IP allowlist checks for `/metrics`.
    #[must_use]
    pub fn without_metrics_ip_allowlist(mut self) -> Self {
        self.metrics_ip_allowlist = None;
        self
    }

    /// Configure a CIDR-based client IP allowlist for `/metrics`.
    ///
    /// Empty lists disable CIDR allowlist enforcement.
    #[must_use]
    pub fn with_metrics_ip_cidr_allowlist<I>(mut self, cidrs: I) -> Self
    where
        I: IntoIterator<Item = IpCidr>,
    {
        let mut cidrs: Vec<IpCidr> = cidrs.into_iter().collect();
        cidrs.sort_unstable();
        cidrs.dedup();
        self.metrics_ip_cidr_allowlist =
            if cidrs.is_empty() { None } else { Some(Arc::new(cidrs)) };
        self
    }

    /// Disable CIDR-based IP allowlist checks for `/metrics`.
    #[must_use]
    pub fn without_metrics_ip_cidr_allowlist(mut self) -> Self {
        self.metrics_ip_cidr_allowlist = None;
        self
    }

    /// Configure trusted proxy CIDRs for `/metrics`.
    ///
    /// When configured, forwarded headers are only trusted when the request
    /// peer IP matches one of these networks.
    #[must_use]
    pub fn with_metrics_trusted_proxies<I>(mut self, cidrs: I) -> Self
    where
        I: IntoIterator<Item = IpCidr>,
    {
        let mut proxies: Vec<IpCidr> = cidrs.into_iter().collect();
        proxies.sort_unstable();
        proxies.dedup();
        self.metrics_trusted_proxies =
            if proxies.is_empty() { None } else { Some(Arc::new(proxies)) };
        self
    }

    /// Disable trusted proxy checks for `/metrics`.
    #[must_use]
    pub fn without_metrics_trusted_proxies(mut self) -> Self {
        self.metrics_trusted_proxies = None;
        self
    }

    /// Configure max accepted forwarding header lengths for `/metrics`.
    #[must_use]
    pub const fn with_metrics_header_limits(mut self, limits: MetricsHeaderLimits) -> Self {
        self.metrics_header_limits = limits;
        self
    }

    /// Returns the configured tenant DB directory when per-tenant routing is enabled.
    #[must_use]
    pub fn tenant_db_dir(&self) -> Option<&Path> {
        self.tenant_db_dir.as_deref().map(std::path::PathBuf::as_path)
    }

    /// Access the underlying default [`Commerce`] engine.
    #[must_use]
    pub fn commerce(&self) -> &Commerce {
        &self.commerce
    }

    /// Return the configured bearer token for `/metrics`, if set.
    #[must_use]
    pub fn metrics_bearer_auth_token(&self) -> Option<&str> {
        self.metrics_bearer_token.as_deref()
    }

    /// Return configured metrics IP allowlist entries.
    #[must_use]
    pub fn metrics_ip_allowlist(&self) -> Option<Vec<IpAddr>> {
        self.metrics_ip_allowlist.as_ref().map(|allowlist| {
            let mut ips: Vec<IpAddr> = allowlist.iter().copied().collect();
            ips.sort_unstable();
            ips
        })
    }

    /// Return configured CIDR-based metrics IP allowlist entries.
    #[must_use]
    pub fn metrics_ip_cidr_allowlist(&self) -> Option<Vec<IpCidr>> {
        self.metrics_ip_cidr_allowlist.as_ref().map(|cidrs| cidrs.as_ref().clone())
    }

    /// Return number of configured exact-IP metrics allowlist entries.
    #[must_use]
    pub fn metrics_ip_allowlist_len(&self) -> usize {
        self.metrics_ip_allowlist.as_ref().map_or(0, |allowlist| allowlist.len())
    }

    /// Return number of configured CIDR metrics allowlist entries.
    #[must_use]
    pub fn metrics_ip_cidr_allowlist_len(&self) -> usize {
        self.metrics_ip_cidr_allowlist.as_ref().map_or(0, |cidrs| cidrs.len())
    }

    /// Returns whether `/metrics` IP allowlist checks are enabled.
    #[must_use]
    pub const fn has_metrics_ip_allowlist(&self) -> bool {
        self.metrics_ip_allowlist.is_some() || self.metrics_ip_cidr_allowlist.is_some()
    }

    /// Returns configured trusted proxy CIDRs.
    #[must_use]
    pub fn metrics_trusted_proxies(&self) -> Option<Vec<IpCidr>> {
        self.metrics_trusted_proxies.as_ref().map(|proxies| proxies.as_ref().clone())
    }

    /// Return number of configured trusted proxy CIDR entries.
    #[must_use]
    pub fn metrics_trusted_proxies_len(&self) -> usize {
        self.metrics_trusted_proxies.as_ref().map_or(0, |proxies| proxies.len())
    }

    /// Record a `/metrics` scrape attempt.
    pub(crate) fn record_metrics_scrape_attempt(&self) {
        self.metrics_scrape_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful `/metrics` scrape.
    pub(crate) fn record_metrics_scrape_allowed(&self) {
        self.metrics_scrape_allowed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful `/metrics` scrape attributed to direct peer IP.
    pub(crate) fn record_metrics_scrape_allowed_peer(&self) {
        self.metrics_scrape_allowed_peer.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful `/metrics` scrape attributed to forwarded IP from a trusted proxy.
    pub(crate) fn record_metrics_scrape_allowed_forwarded_trusted_proxy(&self) {
        self.metrics_scrape_allowed_forwarded_trusted_proxy.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful `/metrics` scrape attributed to forwarded headers without a peer IP.
    pub(crate) fn record_metrics_scrape_allowed_forwarded_without_peer(&self) {
        self.metrics_scrape_allowed_forwarded_without_peer.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful `/metrics` scrape where no client IP source was resolved.
    pub(crate) fn record_metrics_scrape_allowed_unavailable(&self) {
        self.metrics_scrape_allowed_unavailable.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied by network policy.
    pub(crate) fn record_metrics_scrape_denied_ip(&self) {
        self.metrics_scrape_denied_ip.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied because the resolved client IP was not allowed.
    pub(crate) fn record_metrics_scrape_denied_ip_not_allowed(&self) {
        self.metrics_scrape_denied_ip_not_allowed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied because trusted proxy mode had no peer IP metadata.
    pub(crate) fn record_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies(&self) {
        self.metrics_scrape_denied_missing_peer_ip_with_trusted_proxies
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied by authentication.
    pub(crate) fn record_metrics_scrape_denied_auth(&self) {
        self.metrics_scrape_denied_auth.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to a missing Authorization header.
    pub(crate) fn record_metrics_scrape_denied_auth_header_missing(&self) {
        self.metrics_scrape_denied_auth_header_missing.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to an invalid Authorization header format.
    pub(crate) fn record_metrics_scrape_denied_auth_header_invalid(&self) {
        self.metrics_scrape_denied_auth_header_invalid.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to non-UTF8 Authorization header bytes.
    pub(crate) fn record_metrics_scrape_denied_auth_header_invalid_encoding(&self) {
        self.metrics_scrape_denied_auth_header_invalid_encoding.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to non-Bearer Authorization scheme.
    pub(crate) fn record_metrics_scrape_denied_auth_header_invalid_scheme(&self) {
        self.metrics_scrape_denied_auth_header_invalid_scheme.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to malformed Authorization header structure.
    pub(crate) fn record_metrics_scrape_denied_auth_header_malformed(&self) {
        self.metrics_scrape_denied_auth_header_malformed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to multiple Authorization headers.
    pub(crate) fn record_metrics_scrape_denied_auth_header_multiple(&self) {
        self.metrics_scrape_denied_auth_header_multiple.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to oversized Authorization header value.
    pub(crate) fn record_metrics_scrape_denied_auth_header_oversized(&self) {
        self.metrics_scrape_denied_auth_header_oversized.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied due to bearer token mismatch.
    pub(crate) fn record_metrics_scrape_denied_auth_token_mismatch(&self) {
        self.metrics_scrape_denied_auth_token_mismatch.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied because forwarded headers were missing.
    pub(crate) fn record_metrics_scrape_denied_forwarded_missing(&self) {
        self.metrics_scrape_denied_forwarded_missing.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied because forwarded headers were invalid.
    pub(crate) fn record_metrics_scrape_denied_forwarded_invalid(&self) {
        self.metrics_scrape_denied_forwarded_invalid.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a `/metrics` scrape denied because forwarded headers were oversized.
    pub(crate) fn record_metrics_scrape_denied_forwarded_oversized(&self) {
        self.metrics_scrape_denied_forwarded_oversized.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot `/metrics` access outcome counters.
    #[must_use]
    pub(crate) fn metrics_access_metrics(&self) -> MetricsAccessMetrics {
        MetricsAccessMetrics {
            requests_total: self.metrics_scrape_requests.load(Ordering::Relaxed),
            allowed_total: self.metrics_scrape_allowed.load(Ordering::Relaxed),
            allowed_peer_total: self.metrics_scrape_allowed_peer.load(Ordering::Relaxed),
            allowed_forwarded_trusted_proxy_total: self
                .metrics_scrape_allowed_forwarded_trusted_proxy
                .load(Ordering::Relaxed),
            allowed_forwarded_without_peer_total: self
                .metrics_scrape_allowed_forwarded_without_peer
                .load(Ordering::Relaxed),
            allowed_unavailable_total: self
                .metrics_scrape_allowed_unavailable
                .load(Ordering::Relaxed),
            denied_ip_total: self.metrics_scrape_denied_ip.load(Ordering::Relaxed),
            denied_ip_not_allowed_total: self
                .metrics_scrape_denied_ip_not_allowed
                .load(Ordering::Relaxed),
            denied_missing_peer_ip_with_trusted_proxies_total: self
                .metrics_scrape_denied_missing_peer_ip_with_trusted_proxies
                .load(Ordering::Relaxed),
            denied_auth_total: self.metrics_scrape_denied_auth.load(Ordering::Relaxed),
            denied_auth_header_missing_total: self
                .metrics_scrape_denied_auth_header_missing
                .load(Ordering::Relaxed),
            denied_auth_header_invalid_total: self
                .metrics_scrape_denied_auth_header_invalid
                .load(Ordering::Relaxed),
            denied_auth_header_invalid_encoding_total: self
                .metrics_scrape_denied_auth_header_invalid_encoding
                .load(Ordering::Relaxed),
            denied_auth_header_invalid_scheme_total: self
                .metrics_scrape_denied_auth_header_invalid_scheme
                .load(Ordering::Relaxed),
            denied_auth_header_malformed_total: self
                .metrics_scrape_denied_auth_header_malformed
                .load(Ordering::Relaxed),
            denied_auth_header_multiple_total: self
                .metrics_scrape_denied_auth_header_multiple
                .load(Ordering::Relaxed),
            denied_auth_header_oversized_total: self
                .metrics_scrape_denied_auth_header_oversized
                .load(Ordering::Relaxed),
            denied_auth_token_mismatch_total: self
                .metrics_scrape_denied_auth_token_mismatch
                .load(Ordering::Relaxed),
            denied_forwarded_missing_total: self
                .metrics_scrape_denied_forwarded_missing
                .load(Ordering::Relaxed),
            denied_forwarded_invalid_total: self
                .metrics_scrape_denied_forwarded_invalid
                .load(Ordering::Relaxed),
            denied_forwarded_oversized_total: self
                .metrics_scrape_denied_forwarded_oversized
                .load(Ordering::Relaxed),
        }
    }

    /// Returns max accepted forwarding header lengths for `/metrics`.
    #[must_use]
    pub const fn metrics_header_limits(&self) -> MetricsHeaderLimits {
        self.metrics_header_limits
    }

    /// Returns whether trusted proxy CIDRs are configured.
    #[must_use]
    pub const fn has_metrics_trusted_proxies(&self) -> bool {
        self.metrics_trusted_proxies.is_some()
    }

    /// Check whether a peer IP is within configured trusted proxy CIDRs.
    #[must_use]
    pub fn is_metrics_trusted_proxy(&self, ip: IpAddr) -> bool {
        self.metrics_trusted_proxies
            .as_ref()
            .is_some_and(|proxies| proxies.iter().any(|proxy| proxy.contains(ip)))
    }

    /// Check whether a metrics client IP is allowed by the configured allowlist.
    #[must_use]
    pub fn is_metrics_ip_allowed(&self, ip: IpAddr) -> bool {
        if !self.has_metrics_ip_allowlist() {
            return true;
        }

        self.metrics_ip_allowlist.as_ref().is_some_and(|allowlist| allowlist.contains(&ip))
            || self
                .metrics_ip_cidr_allowlist
                .as_ref()
                .is_some_and(|cidrs| cidrs.iter().any(|cidr| cidr.contains(ip)))
    }

    /// Snapshot tenant cache runtime metrics.
    #[must_use]
    pub fn tenant_cache_metrics(&self) -> TenantCacheMetrics {
        let (cached_dbs, in_use_cached_dbs) = self
            .tenant_cache
            .read()
            .map(|cache| {
                let cached = cache.len();
                let in_use =
                    cache.values().filter(|entry| Arc::strong_count(&entry.commerce) > 1).count();
                (cached, in_use)
            })
            .unwrap_or((0, 0));

        TenantCacheMetrics {
            enabled: self.tenant_db_dir.is_some(),
            max_cached_dbs: self.max_tenant_dbs,
            cached_dbs,
            in_use_cached_dbs,
            hits: self.tenant_cache_hits.load(Ordering::Relaxed),
            misses: self.tenant_cache_misses.load(Ordering::Relaxed),
            evictions: self.tenant_cache_evictions.load(Ordering::Relaxed),
            rejections: self.tenant_cache_rejections.load(Ordering::Relaxed),
        }
    }

    /// Resolve the [`Commerce`] engine for a tenant.
    ///
    /// When per-tenant routing is disabled, this always returns the default engine.
    pub fn commerce_for_tenant(&self, tenant_id: Option<&str>) -> Result<Arc<Commerce>, HttpError> {
        let Some(base_dir) = self.tenant_db_dir.as_deref() else {
            return Ok(Arc::clone(&self.commerce));
        };

        let tenant_id = tenant_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| HttpError::BadRequest("missing x-tenant-id header".to_string()))?;

        if !is_valid_tenant_id(tenant_id) {
            return Err(HttpError::BadRequest("invalid x-tenant-id header".to_string()));
        }

        let access_tick = self.next_tenant_access_tick();
        let mut cache = self.tenant_cache.write().map_err(|_| {
            HttpError::InternalError("tenant cache lock poisoned while writing".to_string())
        })?;
        if let Some(existing) = cache.get_mut(tenant_id) {
            existing.last_access_tick = access_tick;
            self.tenant_cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::clone(&existing.commerce));
        }
        self.tenant_cache_misses.fetch_add(1, Ordering::Relaxed);
        drop(cache);

        std::fs::create_dir_all(base_dir).map_err(|error| {
            HttpError::InternalError(format!(
                "failed to create tenant database directory {}: {error}",
                base_dir.display()
            ))
        })?;

        let db_path = base_dir.join(format!("{tenant_id}.db"));
        let db_path_str = db_path.to_string_lossy().into_owned();
        let created = Arc::new(Commerce::new(&db_path_str).map_err(|error| {
            HttpError::InternalError(format!(
                "failed to initialize tenant database for '{tenant_id}': {error}"
            ))
        })?);

        let mut cache = self.tenant_cache.write().map_err(|_| {
            HttpError::InternalError("tenant cache lock poisoned while writing".to_string())
        })?;
        let access_tick = self.next_tenant_access_tick();
        if let Some(existing) = cache.get_mut(tenant_id) {
            existing.last_access_tick = access_tick;
            self.tenant_cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::clone(&existing.commerce));
        }
        if cache.len() >= self.max_tenant_dbs {
            self.evict_lru_idle_tenant(&mut cache)?;
        }
        let entry = cache.entry(tenant_id.to_string()).or_insert_with(|| TenantCacheEntry {
            commerce: Arc::clone(&created),
            last_access_tick: access_tick,
        });
        Ok(Arc::clone(&entry.commerce))
    }

    fn next_tenant_access_tick(&self) -> u64 {
        self.tenant_access_clock.fetch_add(1, Ordering::Relaxed)
    }

    fn evict_lru_idle_tenant(
        &self,
        cache: &mut HashMap<String, TenantCacheEntry>,
    ) -> Result<(), HttpError> {
        let Some((tenant_id, _)) = cache
            .iter()
            .filter(|(_, entry)| Arc::strong_count(&entry.commerce) == 1)
            .min_by_key(|(_, entry)| entry.last_access_tick)
            .map(|(tenant_id, entry)| (tenant_id.clone(), entry.last_access_tick))
        else {
            self.tenant_cache_rejections.fetch_add(1, Ordering::Relaxed);
            return Err(HttpError::TooManyRequests(format!(
                "tenant database limit reached (max: {}); all cached tenant engines are currently in use",
                self.max_tenant_dbs
            )));
        };

        cache.remove(&tenant_id);
        self.tenant_cache_evictions.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::CreateCustomer;
    use uuid::Uuid;

    fn test_commerce() -> Commerce {
        Commerce::new(":memory:").expect("in-memory Commerce")
    }

    #[test]
    fn state_debug_impl() {
        let state = AppState::new(test_commerce());
        let dbg = format!("{state:?}");
        assert!(dbg.contains("AppState"));
    }

    #[test]
    fn state_clone() {
        let state = AppState::new(test_commerce());
        let cloned = state.clone();
        // Both point to the same Arc
        assert!(Arc::ptr_eq(&state.commerce, &cloned.commerce));
    }

    #[test]
    fn state_accessor() {
        let state = AppState::new(test_commerce());
        // Just verify it doesn't panic
        let _commerce = state.commerce();
    }

    #[test]
    fn metrics_auth_configuration_round_trip() {
        let state = AppState::new(test_commerce()).with_metrics_bearer_auth("metrics-token");
        assert_eq!(state.metrics_bearer_auth_token(), Some("metrics-token"));

        let disabled = state.without_metrics_auth();
        assert_eq!(disabled.metrics_bearer_auth_token(), None);
    }

    #[test]
    fn metrics_ip_allowlist_configuration_round_trip() {
        let state = AppState::new(test_commerce())
            .with_metrics_ip_allowlist(["127.0.0.1".parse().unwrap(), "10.0.0.1".parse().unwrap()]);
        assert_eq!(
            state.metrics_ip_allowlist().unwrap(),
            vec!["10.0.0.1".parse::<IpAddr>().unwrap(), "127.0.0.1".parse::<IpAddr>().unwrap()]
        );
        assert_eq!(state.metrics_ip_allowlist_len(), 2);
        assert_eq!(state.metrics_ip_cidr_allowlist_len(), 0);
        assert!(state.metrics_ip_cidr_allowlist().is_none());
        assert!(state.has_metrics_ip_allowlist());
        assert!(state.is_metrics_ip_allowed("127.0.0.1".parse().unwrap()));
        assert!(!state.is_metrics_ip_allowed("192.168.1.10".parse().unwrap()));

        let disabled = state.without_metrics_ip_allowlist();
        assert!(disabled.metrics_ip_allowlist().is_none());
        assert_eq!(disabled.metrics_ip_allowlist_len(), 0);
        assert!(!disabled.has_metrics_ip_allowlist());
        assert!(disabled.is_metrics_ip_allowed("192.168.1.10".parse().unwrap()));
    }

    #[test]
    fn metrics_ip_cidr_allowlist_configuration_round_trip() {
        let state = AppState::new(test_commerce()).with_metrics_ip_cidr_allowlist([
            "10.0.0.0/8".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
        ]);
        assert_eq!(
            state.metrics_ip_cidr_allowlist().unwrap(),
            vec!["10.0.0.0/8".parse().unwrap(), "127.0.0.1/32".parse().unwrap()]
        );
        assert_eq!(state.metrics_ip_allowlist_len(), 0);
        assert_eq!(state.metrics_ip_cidr_allowlist_len(), 2);
        assert!(state.metrics_ip_allowlist().is_none());
        assert!(state.has_metrics_ip_allowlist());
        assert!(state.is_metrics_ip_allowed("10.5.6.7".parse().unwrap()));
        assert!(state.is_metrics_ip_allowed("127.0.0.1".parse().unwrap()));
        assert!(!state.is_metrics_ip_allowed("203.0.113.10".parse().unwrap()));

        let disabled = state.without_metrics_ip_cidr_allowlist();
        assert!(disabled.metrics_ip_cidr_allowlist().is_none());
        assert_eq!(disabled.metrics_ip_cidr_allowlist_len(), 0);
        assert!(!disabled.has_metrics_ip_allowlist());
        assert!(disabled.is_metrics_ip_allowed("203.0.113.10".parse().unwrap()));
    }

    #[test]
    fn metrics_ip_allowlists_are_union_when_both_configured() {
        let state = AppState::new(test_commerce())
            .with_metrics_ip_allowlist(["203.0.113.10".parse().unwrap()])
            .with_metrics_ip_cidr_allowlist(["10.0.0.0/8".parse().unwrap()]);

        assert!(state.is_metrics_ip_allowed("203.0.113.10".parse().unwrap()));
        assert!(state.is_metrics_ip_allowed("10.1.2.3".parse().unwrap()));
        assert!(!state.is_metrics_ip_allowed("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn ip_cidr_parsing_and_contains() {
        let cidr = "10.0.0.0/8".parse::<IpCidr>().unwrap();
        assert!(cidr.contains("10.1.2.3".parse().unwrap()));
        assert!(!cidr.contains("11.1.2.3".parse().unwrap()));

        let host = "127.0.0.1".parse::<IpCidr>().unwrap();
        assert_eq!(host.to_string(), "127.0.0.1/32");
        assert!(host.contains("127.0.0.1".parse().unwrap()));
        assert!(!host.contains("127.0.0.2".parse().unwrap()));
    }

    #[test]
    fn metrics_trusted_proxies_configuration_round_trip() {
        let state = AppState::new(test_commerce()).with_metrics_trusted_proxies([
            "10.0.0.0/8".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
        ]);

        let proxies = state.metrics_trusted_proxies().unwrap();
        assert_eq!(proxies.len(), 2);
        assert_eq!(state.metrics_trusted_proxies_len(), 2);
        assert!(state.has_metrics_trusted_proxies());
        assert!(state.is_metrics_trusted_proxy("10.9.8.7".parse().unwrap()));
        assert!(state.is_metrics_trusted_proxy("127.0.0.1".parse().unwrap()));
        assert!(!state.is_metrics_trusted_proxy("192.168.1.10".parse().unwrap()));

        let disabled = state.without_metrics_trusted_proxies();
        assert!(disabled.metrics_trusted_proxies().is_none());
        assert_eq!(disabled.metrics_trusted_proxies_len(), 0);
        assert!(!disabled.has_metrics_trusted_proxies());
        assert!(!disabled.is_metrics_trusted_proxy("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn metrics_header_limits_configuration_round_trip() {
        let limits = MetricsHeaderLimits::new_with_authorization(1024, 1536, 256, 768).unwrap();
        let state = AppState::new(test_commerce()).with_metrics_header_limits(limits);
        assert_eq!(state.metrics_header_limits(), limits);
        assert_eq!(state.metrics_header_limits().authorization_header_value_bytes(), 768);
    }

    #[test]
    fn metrics_header_limits_reject_zero_values() {
        assert!(MetricsHeaderLimits::new(0, 1024, 256).is_err());
        assert!(MetricsHeaderLimits::new(1024, 0, 256).is_err());
        assert!(MetricsHeaderLimits::new(1024, 1024, 0).is_err());
        assert!(MetricsHeaderLimits::new_with_authorization(1024, 1024, 256, 0).is_err());
    }

    #[test]
    fn metrics_access_metrics_track_counters() {
        let state = AppState::new(test_commerce());
        let initial = state.metrics_access_metrics();
        assert_eq!(initial.requests_total, 0);
        assert_eq!(initial.allowed_total, 0);
        assert_eq!(initial.allowed_peer_total, 0);
        assert_eq!(initial.allowed_forwarded_trusted_proxy_total, 0);
        assert_eq!(initial.allowed_forwarded_without_peer_total, 0);
        assert_eq!(initial.allowed_unavailable_total, 0);
        assert_eq!(initial.denied_ip_total, 0);
        assert_eq!(initial.denied_ip_not_allowed_total, 0);
        assert_eq!(initial.denied_missing_peer_ip_with_trusted_proxies_total, 0);
        assert_eq!(initial.denied_auth_total, 0);
        assert_eq!(initial.denied_auth_header_missing_total, 0);
        assert_eq!(initial.denied_auth_header_invalid_total, 0);
        assert_eq!(initial.denied_auth_header_invalid_encoding_total, 0);
        assert_eq!(initial.denied_auth_header_invalid_scheme_total, 0);
        assert_eq!(initial.denied_auth_header_malformed_total, 0);
        assert_eq!(initial.denied_auth_header_multiple_total, 0);
        assert_eq!(initial.denied_auth_header_oversized_total, 0);
        assert_eq!(initial.denied_auth_token_mismatch_total, 0);
        assert_eq!(initial.denied_forwarded_missing_total, 0);
        assert_eq!(initial.denied_forwarded_invalid_total, 0);
        assert_eq!(initial.denied_forwarded_oversized_total, 0);

        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_auth();
        state.record_metrics_scrape_denied_auth_header_missing();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_ip();
        state.record_metrics_scrape_denied_ip_not_allowed();
        state.record_metrics_scrape_denied_forwarded_missing();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_auth();
        state.record_metrics_scrape_denied_auth_header_invalid();
        state.record_metrics_scrape_denied_auth_header_invalid_encoding();
        state.record_metrics_scrape_denied_auth_header_invalid();
        state.record_metrics_scrape_denied_auth_header_invalid_scheme();
        state.record_metrics_scrape_denied_auth_header_invalid();
        state.record_metrics_scrape_denied_auth_header_malformed();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_auth();
        state.record_metrics_scrape_denied_auth_header_multiple();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_auth();
        state.record_metrics_scrape_denied_auth_header_oversized();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_auth();
        state.record_metrics_scrape_denied_auth_token_mismatch();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_ip();
        state.record_metrics_scrape_denied_forwarded_invalid();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_denied_ip();
        state.record_metrics_scrape_denied_missing_peer_ip_with_trusted_proxies();
        state.record_metrics_scrape_denied_forwarded_oversized();
        state.record_metrics_scrape_attempt();
        state.record_metrics_scrape_allowed();
        state.record_metrics_scrape_allowed_peer();
        state.record_metrics_scrape_allowed_forwarded_trusted_proxy();
        state.record_metrics_scrape_allowed_forwarded_without_peer();
        state.record_metrics_scrape_allowed_unavailable();

        let snapshot = state.metrics_access_metrics();
        assert_eq!(snapshot.requests_total, 9);
        assert_eq!(snapshot.allowed_total, 1);
        assert_eq!(snapshot.allowed_peer_total, 1);
        assert_eq!(snapshot.allowed_forwarded_trusted_proxy_total, 1);
        assert_eq!(snapshot.allowed_forwarded_without_peer_total, 1);
        assert_eq!(snapshot.allowed_unavailable_total, 1);
        assert_eq!(snapshot.denied_ip_total, 3);
        assert_eq!(snapshot.denied_ip_not_allowed_total, 1);
        assert_eq!(snapshot.denied_missing_peer_ip_with_trusted_proxies_total, 1);
        assert_eq!(snapshot.denied_auth_total, 5);
        assert_eq!(snapshot.denied_auth_header_missing_total, 1);
        assert_eq!(snapshot.denied_auth_header_invalid_total, 3);
        assert_eq!(snapshot.denied_auth_header_invalid_encoding_total, 1);
        assert_eq!(snapshot.denied_auth_header_invalid_scheme_total, 1);
        assert_eq!(snapshot.denied_auth_header_malformed_total, 1);
        assert_eq!(snapshot.denied_auth_header_multiple_total, 1);
        assert_eq!(snapshot.denied_auth_header_oversized_total, 1);
        assert_eq!(snapshot.denied_auth_token_mismatch_total, 1);
        assert_eq!(snapshot.denied_forwarded_missing_total, 1);
        assert_eq!(snapshot.denied_forwarded_invalid_total, 1);
        assert_eq!(snapshot.denied_forwarded_oversized_total, 1);
    }

    #[test]
    fn tenant_header_parser_reads_valid_value() {
        let mut headers = HeaderMap::new();
        headers.insert(X_TENANT_ID.clone(), "tenant-1".parse().unwrap());
        assert_eq!(tenant_id_from_headers(&headers).as_deref(), Some("tenant-1"));
    }

    #[test]
    fn tenant_header_parser_rejects_empty_values() {
        let mut headers = HeaderMap::new();
        headers.insert(X_TENANT_ID.clone(), " ".parse().unwrap());
        assert!(tenant_id_from_headers(&headers).is_none());
    }

    #[test]
    fn tenant_routing_isolated_when_configured() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()));

        let tenant_a = state.commerce_for_tenant(Some("tenant-a")).expect("tenant-a commerce");
        let tenant_b = state.commerce_for_tenant(Some("tenant-b")).expect("tenant-b commerce");

        tenant_a
            .customers()
            .create(CreateCustomer {
                email: "tenant-a@example.com".into(),
                first_name: "Tenant".into(),
                last_name: "A".into(),
                ..Default::default()
            })
            .unwrap();

        let a_count = tenant_a.customers().list(Default::default()).unwrap().len();
        let b_count = tenant_b.customers().list(Default::default()).unwrap().len();
        assert_eq!(a_count, 1);
        assert_eq!(b_count, 0);

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[test]
    fn tenant_routing_requires_header_when_enabled() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()));

        let err = state.commerce_for_tenant(None).expect_err("expected missing tenant error");
        assert!(matches!(err, HttpError::BadRequest(_)));

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[test]
    fn tenant_routing_enforces_database_limit() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()))
            .with_max_tenant_dbs(1);

        let first = state.commerce_for_tenant(Some("tenant-a"));
        assert!(first.is_ok());

        let second = state.commerce_for_tenant(Some("tenant-b"));
        assert!(matches!(second, Err(HttpError::TooManyRequests(_))));

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[test]
    fn tenant_routing_evicts_idle_lru_database_and_reopens_from_disk() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()))
            .with_max_tenant_dbs(1);

        let tenant_a = state.commerce_for_tenant(Some("tenant-a")).unwrap();
        tenant_a
            .customers()
            .create(CreateCustomer {
                email: "tenant-a@example.com".into(),
                first_name: "Tenant".into(),
                last_name: "A".into(),
                ..Default::default()
            })
            .unwrap();
        drop(tenant_a);

        assert!(state.commerce_for_tenant(Some("tenant-b")).is_ok());

        let tenant_a_reloaded = state.commerce_for_tenant(Some("tenant-a")).unwrap();
        let tenant_a_customer_count =
            tenant_a_reloaded.customers().list(Default::default()).unwrap().len();
        assert_eq!(tenant_a_customer_count, 1);

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[test]
    fn tenant_cache_metrics_track_hits_misses_evictions_and_rejections() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()))
            .with_max_tenant_dbs(1);

        let tenant_a = state.commerce_for_tenant(Some("tenant-a")).unwrap();
        let tenant_a_again = state.commerce_for_tenant(Some("tenant-a")).unwrap();
        drop(tenant_a_again);

        let tenant_b_while_a_in_use = state.commerce_for_tenant(Some("tenant-b"));
        assert!(matches!(tenant_b_while_a_in_use, Err(HttpError::TooManyRequests(_))));

        drop(tenant_a);
        let tenant_b_after_release = state.commerce_for_tenant(Some("tenant-b"));
        assert!(tenant_b_after_release.is_ok());

        let metrics = state.tenant_cache_metrics();
        assert!(metrics.enabled);
        assert_eq!(metrics.max_cached_dbs, 1);
        assert_eq!(metrics.cached_dbs, 1);
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 3);
        assert_eq!(metrics.evictions, 1);
        assert_eq!(metrics.rejections, 1);

        let _ = std::fs::remove_dir_all(tenant_dir);
    }
}
