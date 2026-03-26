//! Webhook delivery system for external event notifications

use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use stateset_core::CommerceEvent;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::sync::{Arc, RwLock};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// Webhook endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    /// Unique identifier
    pub id: Uuid,
    /// Display name
    pub name: String,
    /// Target URL for POST requests
    pub url: String,
    /// Optional secret for HMAC signature
    pub secret: Option<String>,
    /// Event types to receive (empty = all events)
    pub event_types: Vec<String>,
    /// Whether the webhook is active
    pub active: bool,
    /// Custom headers to include
    pub headers: HashMap<String, String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Reasons webhook registration failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebhookRegistrationError {
    /// The webhook URL is considered unsafe (e.g. localhost, private network, invalid scheme).
    UnsafeUrl,
    /// A webhook with this ID already exists.
    DuplicateId,
    /// Webhooks are disabled in the event system configuration.
    WebhooksDisabled,
}

impl fmt::Display for WebhookRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeUrl => {
                write!(f, "webhook registration rejected: URL validation failed")
            }
            Self::DuplicateId => {
                write!(f, "webhook registration rejected: duplicate webhook id")
            }
            Self::WebhooksDisabled => {
                write!(f, "webhook registration rejected: webhook subsystem is disabled")
            }
        }
    }
}

impl Webhook {
    /// Create a new webhook
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            url: url.into(),
            secret: None,
            event_types: Vec::new(),
            active: true,
            headers: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Set the secret for HMAC signature
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Filter to specific event types
    #[must_use] 
    pub fn with_events(mut self, events: Vec<String>) -> Self {
        self.event_types = events;
        self
    }

    /// Add a custom header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Check if this webhook should receive an event
    #[must_use] 
    pub fn should_receive(&self, event: &CommerceEvent) -> bool {
        if !self.active {
            return false;
        }
        if self.event_types.is_empty() {
            return true;
        }
        self.event_types.contains(&event.event_type().to_string())
    }
}

/// Configuration for webhook delivery
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub max_retries: u32,
    pub timeout_secs: u64,
    pub retry_delay_ms: u64,
    pub max_in_flight: usize,
    /// Maximum number of delivery records to keep per webhook
    pub max_delivery_history: usize,
    /// Optional outbound host allowlist.
    ///
    /// When empty, current strict public-host defaults apply.
    /// When non-empty, hosts must match one of these entries in addition
    /// to existing SSRF/public-network checks.
    ///
    /// Supported entry forms:
    /// - `example.com` (exact host)
    /// - `*.example.com` (subdomain match only)
    pub outbound_allowlist: Vec<String>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout_secs: 30,
            retry_delay_ms: 1000,
            max_in_flight: 8,
            max_delivery_history: 1_000,
            outbound_allowlist: Vec::new(),
        }
    }
}

impl WebhookConfig {
    /// Replace outbound allowlist entries with normalized host rules.
    pub fn with_outbound_allowlist<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.outbound_allowlist = normalize_allowlist(entries);
        self
    }
}

enum WebhookRuntime {
    Handle(tokio::runtime::Handle),
    Owned(tokio::runtime::Runtime),
    Disabled(String),
}

impl WebhookRuntime {
    fn new() -> Self {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            Self::Handle(handle)
        } else {
            match tokio::runtime::Runtime::new() {
                Ok(runtime) => Self::Owned(runtime),
                Err(err) => {
                    let fallback =
                        tokio::runtime::Builder::new_current_thread().enable_all().build();
                    match fallback {
                        Ok(runtime) => Self::Owned(runtime),
                        Err(fallback_err) => {
                            let message = format!(
                                "Failed to create webhook runtime: {err}; fallback failed: {fallback_err}"
                            );
                            tracing::error!("{}", message);
                            Self::Disabled(message)
                        }
                    }
                }
            }
        }
    }

    fn spawn<F>(&self, fut: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        match self {
            Self::Handle(handle) => {
                handle.spawn(fut);
            }
            Self::Owned(runtime) => {
                runtime.spawn(fut);
            }
            Self::Disabled(message) => {
                tracing::error!("Webhook runtime unavailable: {}", message);
            }
        }
    }
}

/// Webhook delivery attempt record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: String,
    pub event_id: Uuid,
    pub status: DeliveryStatus,
    pub attempts: u32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub response_status: Option<u16>,
    pub response_body: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Delivery status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Retrying,
}

/// Webhook manager handles registration and delivery
pub struct WebhookManager {
    webhooks: Arc<RwLock<HashMap<Uuid, Webhook>>>,
    delivery_history: Arc<RwLock<HashMap<Uuid, VecDeque<WebhookDelivery>>>>,
    config: WebhookConfig,
    client: reqwest::Client,
    runtime: WebhookRuntime,
    delivery_task_limit: Arc<Semaphore>,
}

impl std::fmt::Debug for WebhookManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookManager").field("config", &self.config).finish_non_exhaustive()
    }
}

impl WebhookManager {
    /// Create a new webhook manager
    #[must_use] 
    pub fn new(max_retries: u32, timeout_secs: u64) -> Self {
        Self::with_config(WebhookConfig { max_retries, timeout_secs, ..Default::default() })
    }

    /// Create a new webhook manager with full configuration.
    #[must_use] 
    pub fn with_config(config: WebhookConfig) -> Self {
        let mut config = WebhookConfig {
            max_in_flight: config.max_in_flight.max(1),
            timeout_secs: config.timeout_secs.max(1),
            retry_delay_ms: config.retry_delay_ms.max(1),
            ..config
        };
        config.outbound_allowlist = normalize_allowlist(config.outbound_allowlist.iter());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|err| {
                tracing::error!("Failed to create HTTP client: {}", err);
                reqwest::Client::new()
            });
        let runtime = WebhookRuntime::new();
        let delivery_task_limit = Arc::new(Semaphore::new(config.max_in_flight.max(1)));

        Self {
            webhooks: Arc::new(RwLock::new(HashMap::new())),
            delivery_history: Arc::new(RwLock::new(HashMap::new())),
            config,
            client,
            runtime,
            delivery_task_limit,
        }
    }

    /// Register a new webhook using the legacy API.
    ///
    /// Returns `Uuid::nil()` on failure to preserve compatibility with
    /// older call sites. Use [`register_strict`](Self::register_strict) or
    /// [`try_register`](Self::try_register) for explicit handling.
    pub fn register(&self, webhook: Webhook) -> Uuid {
        match self.register_strict(webhook) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(error = %err, "Webhook registration fallback returned nil");
                Uuid::nil()
            }
        }
    }

    /// Register a new webhook with explicit failure semantics.
    ///
    /// Returns a registration error when the URL is rejected.
    pub fn register_strict(&self, webhook: Webhook) -> Result<Uuid, WebhookRegistrationError> {
        if !is_safe_webhook_url(&webhook.url, &self.config.outbound_allowlist) {
            tracing::warn!(
                webhook_id = %webhook.id,
                webhook_url = %webhook.url,
                "Rejected webhook registration: unsafe URL"
            );
            return Err(WebhookRegistrationError::UnsafeUrl);
        }

        let id = webhook.id;
        let mut webhooks = match self.webhooks.write() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (write); recovering");
                poison.into_inner()
            }
        };
        if webhooks.contains_key(&id) {
            tracing::warn!(webhook_id = %id, "Rejected webhook registration: duplicate id");
            return Err(WebhookRegistrationError::DuplicateId);
        }
        webhooks.insert(id, webhook);
        tracing::info!(webhook_id = %id, "Webhook registered");
        Ok(id)
    }

    /// Register a new webhook, returning `None` when URL validation fails.
    pub fn try_register(&self, webhook: Webhook) -> Option<Uuid> {
        self.register_strict(webhook).ok()
    }

    /// Unregister a webhook
    pub fn unregister(&self, id: Uuid) -> bool {
        let mut webhooks = match self.webhooks.write() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (write); recovering");
                poison.into_inner()
            }
        };
        let removed = webhooks.remove(&id).is_some();
        if removed {
            let mut delivery_history = match self.delivery_history.write() {
                Ok(guard) => guard,
                Err(poison) => {
                    tracing::error!(
                        "WebhookManager delivery history lock poisoned (write); recovering"
                    );
                    poison.into_inner()
                }
            };
            delivery_history.remove(&id);
            tracing::info!(webhook_id = %id, "Webhook unregistered");
        }
        removed
    }

    /// Get a webhook by ID
    pub fn get(&self, id: Uuid) -> Option<Webhook> {
        let webhooks = match self.webhooks.read() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (read); recovering");
                poison.into_inner()
            }
        };
        webhooks.get(&id).cloned()
    }

    /// List all webhooks
    pub fn list(&self) -> Vec<Webhook> {
        let webhooks = match self.webhooks.read() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (read); recovering");
                poison.into_inner()
            }
        };
        webhooks.values().cloned().collect()
    }

    /// Get delivery history for a webhook (newest-first)
    pub fn deliveries(&self, webhook_id: Uuid) -> Vec<WebhookDelivery> {
        let history = match self.delivery_history.read() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager delivery history lock poisoned (read); recovering");
                poison.into_inner()
            }
        };
        history
            .get(&webhook_id)
            .map(|entries| entries.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    /// Update a webhook
    pub fn update(&self, webhook: Webhook) -> bool {
        let id = webhook.id;
        let mut webhooks = match self.webhooks.write() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (write); recovering");
                poison.into_inner()
            }
        };
        if !is_safe_webhook_url(&webhook.url, &self.config.outbound_allowlist) {
            tracing::warn!(webhook_id = %webhook.id, webhook_url = %webhook.url, "Rejected webhook update: unsafe URL");
            return false;
        }

        if let std::collections::hash_map::Entry::Occupied(mut e) = webhooks.entry(id) {
            e.insert(webhook);
            true
        } else {
            false
        }
    }

    /// Enable/disable a webhook
    pub fn set_active(&self, id: Uuid, active: bool) -> bool {
        let mut webhooks = match self.webhooks.write() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (write); recovering");
                poison.into_inner()
            }
        };
        if let Some(webhook) = webhooks.get_mut(&id) {
            webhook.active = active;
            true
        } else {
            false
        }
    }

    /// Deliver an event to all matching webhooks (spawns async tasks)
    pub fn deliver(&self, event: CommerceEvent) {
        use futures::stream::{self, StreamExt};

        let webhooks_guard = match self.webhooks.read() {
            Ok(guard) => guard,
            Err(poison) => {
                tracing::error!("WebhookManager webhooks lock poisoned (read); recovering");
                poison.into_inner()
            }
        };
        let webhooks: Vec<Webhook> =
            webhooks_guard.values().filter(|w| w.should_receive(&event)).cloned().collect();

        if webhooks.is_empty() {
            return;
        }

        let client = self.client.clone();
        let config = self.config.clone();
        let delivery_history = self.delivery_history.clone();
        let max_in_flight = config.max_in_flight.max(1);
        let Some(delivery_permit) = self.delivery_task_limit.clone().try_acquire_owned().ok()
        else {
            tracing::warn!(
                event_type = event.event_type(),
                "Dropping webhook delivery due to saturated delivery workers"
            );
            return;
        };

        self.runtime.spawn(async move {
            let _delivery_permit = delivery_permit;
            let deliveries = stream::iter(webhooks.into_iter().map(|webhook| {
                let client = client.clone();
                let event = event.clone();
                let config = config.clone();
                let delivery_history = delivery_history.clone();
                async move {
                    deliver_to_webhook(&client, &webhook, &event, &config, &delivery_history).await;
                }
            }));

            deliveries.for_each_concurrent(max_in_flight, |fut| fut).await;
        });
    }
}

/// Deliver an event to a specific webhook with retries
async fn deliver_to_webhook(
    client: &reqwest::Client,
    webhook: &Webhook,
    event: &CommerceEvent,
    config: &WebhookConfig,
    delivery_history: &Arc<RwLock<HashMap<Uuid, VecDeque<WebhookDelivery>>>>,
) {
    let mut delivery = WebhookDelivery {
        id: Uuid::new_v4(),
        webhook_id: webhook.id,
        event_type: event.event_type().to_string(),
        event_id: Uuid::new_v4(),
        status: DeliveryStatus::Pending,
        attempts: 0,
        last_attempt_at: None,
        response_status: None,
        response_body: None,
        error: None,
        created_at: Utc::now(),
    };

    if !is_safe_webhook_url_for_delivery(&webhook.url, &config.outbound_allowlist) {
        tracing::error!(
            webhook_id = %webhook.id,
            webhook_url = %webhook.url,
            "Skipping webhook delivery: unsafe URL"
        );
        delivery.status = DeliveryStatus::Failed;
        delivery.error = Some("unsafe webhook URL".to_string());
        append_delivery_record(delivery_history, webhook.id, delivery, config.max_delivery_history);
        return;
    }

    let payload = WebhookPayload {
        id: Uuid::new_v4(),
        event_type: event.event_type().to_string(),
        timestamp: event.timestamp(),
        data: event.clone(),
    };
    delivery.event_id = payload.id;

    let body = match serde_json::to_string(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize webhook payload");
            delivery.status = DeliveryStatus::Failed;
            delivery.error = Some(format!("failed to serialize payload: {e}"));
            append_delivery_record(
                delivery_history,
                webhook.id,
                delivery,
                config.max_delivery_history,
            );
            return;
        }
    };

    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            // Exponential backoff with saturation + cap to avoid overflows or ridiculous delays.
            const MAX_BACKOFF_MS: u64 = 60_000;
            let exp = attempt.saturating_sub(1);
            let shift = exp.min(16);
            let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
            let delay_ms = config.retry_delay_ms.saturating_mul(factor).min(MAX_BACKOFF_MS);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        begin_delivery_attempt(&mut delivery, attempt + 1);

        let mut request = client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-ID", webhook.id.to_string())
            .header("X-Event-Type", event.event_type())
            .header("X-Delivery-ID", payload.id.to_string())
            .header("X-Delivery-Attempt", (attempt + 1).to_string());

        // Add HMAC signature if secret is configured
        if let Some(ref secret) = webhook.secret {
            if let Some(signature) = compute_signature(secret, &body) {
                request = request.header("X-Signature", signature);
            }
        }

        // Add custom headers
        for (key, value) in &webhook.headers {
            request = request.header(key, value);
        }

        match request.body(body.clone()).send().await {
            Ok(response) => {
                let status = response.status();
                delivery.response_status = Some(status.as_u16());
                if status.is_success() {
                    delivery.status = DeliveryStatus::Delivered;
                    tracing::debug!(
                        webhook_id = %webhook.id,
                        event_type = event.event_type(),
                        status = %status,
                        attempt = attempt + 1,
                        "Webhook delivered successfully"
                    );
                    append_delivery_record(
                        delivery_history,
                        webhook.id,
                        delivery,
                        config.max_delivery_history,
                    );
                    return;
                } else {
                    let response_body = response.text().await.unwrap_or_default();
                    delivery.response_body = Some(response_body.clone());
                    if attempt >= config.max_retries {
                        delivery.status = DeliveryStatus::Failed;
                    } else {
                        delivery.status = DeliveryStatus::Retrying;
                    }
                    tracing::warn!(
                        webhook_id = %webhook.id,
                        event_type = event.event_type(),
                        status = %status,
                        attempt = attempt + 1,
                        response = %response_body,
                        "Webhook delivery failed with non-success status"
                    );
                }
            }
            Err(e) => {
                delivery.error = Some(e.to_string());
                if attempt >= config.max_retries {
                    delivery.status = DeliveryStatus::Failed;
                } else {
                    delivery.status = DeliveryStatus::Retrying;
                }
                tracing::warn!(
                    webhook_id = %webhook.id,
                    event_type = event.event_type(),
                    attempt = attempt + 1,
                    error = %e,
                    "Webhook delivery failed"
                );
            }
        }

        if delivery.status == DeliveryStatus::Failed && attempt >= config.max_retries {
            break;
        }
    }

    append_delivery_record(delivery_history, webhook.id, delivery, config.max_delivery_history);
    tracing::error!(
        webhook_id = %webhook.id,
        event_type = event.event_type(),
        max_retries = config.max_retries,
        "Webhook delivery exhausted all retries"
    );
}

fn begin_delivery_attempt(delivery: &mut WebhookDelivery, attempt: u32) {
    delivery.attempts = attempt;
    delivery.last_attempt_at = Some(Utc::now());
    // Keep only metadata from the active attempt.
    delivery.response_status = None;
    delivery.response_body = None;
    delivery.error = None;
}

fn append_delivery_record(
    delivery_history: &Arc<RwLock<HashMap<Uuid, VecDeque<WebhookDelivery>>>>,
    webhook_id: Uuid,
    delivery: WebhookDelivery,
    max_delivery_history: usize,
) {
    let mut history = match delivery_history.write() {
        Ok(guard) => guard,
        Err(poison) => {
            tracing::error!("WebhookManager delivery history lock poisoned (write); recovering");
            poison.into_inner()
        }
    };
    let entries = history.entry(webhook_id).or_insert_with(VecDeque::new);
    entries.push_back(delivery);

    while entries.len() > max_delivery_history {
        entries.pop_front();
    }
}

fn normalize_allowlist<I, S>(entries: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = Vec::new();
    for entry in entries {
        let candidate = normalize_host_match_value(entry.as_ref());
        if candidate.is_empty() {
            continue;
        }
        if !normalized.contains(&candidate) {
            normalized.push(candidate);
        }
    }
    normalized
}

fn normalize_host_match_value(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(suffix) = trimmed.strip_prefix("*.") {
        let suffix = suffix.trim_end_matches('.').to_ascii_lowercase();
        if suffix.is_empty() {
            return String::new();
        }
        return format!("*.{suffix}");
    }
    trimmed.trim_end_matches('.').to_ascii_lowercase()
}

fn host_matches_allowlist(host: &str, allowlist: &[String]) -> bool {
    let normalized_host = normalize_host_match_value(host);

    allowlist.iter().any(|entry| allowlist_entry_matches(normalized_host.as_str(), entry.as_str()))
}

fn allowlist_entry_matches(host: &str, entry: &str) -> bool {
    if let Some(suffix) = entry.strip_prefix("*.") {
        // Subdomain-only wildcard. `*.example.com` does not match `example.com`.
        return host.len() > suffix.len()
            && host.ends_with(suffix)
            && host
                .as_bytes()
                .get(host.len().saturating_sub(suffix.len() + 1))
                .is_some_and(|b| *b == b'.');
    }

    host == entry
}

fn is_safe_webhook_url(url: &str, outbound_allowlist: &[String]) -> bool {
    // Registration should not depend on live DNS resolution because that creates
    // environment-dependent false negatives (offline CI/dev shells). We still
    // enforce strict scheme/host/IP checks here and apply DNS resolution in the
    // delivery path.
    is_safe_webhook_url_with_dns_check(url, false, outbound_allowlist)
}

fn is_safe_webhook_url_for_delivery(url: &str, outbound_allowlist: &[String]) -> bool {
    is_safe_webhook_url_with_dns_check(url, true, outbound_allowlist)
}

fn is_safe_webhook_url_with_dns_check(
    url: &str,
    resolve_dns: bool,
    outbound_allowlist: &[String],
) -> bool {
    is_safe_webhook_url_with_dns_resolver(url, resolve_dns, outbound_allowlist, &resolve_host_ips)
}

fn is_safe_webhook_url_with_dns_resolver<F>(
    url: &str,
    resolve_dns: bool,
    outbound_allowlist: &[String],
    dns_resolver: &F,
) -> bool
where
    F: Fn(&str, u16) -> Result<Vec<IpAddr>, String>,
{
    let parsed = match Url::parse(url) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(webhook_url = %url, error = %err, "Webhook URL parse failed");
            return false;
        }
    };

    if !matches!(parsed.scheme(), "http" | "https") {
        tracing::warn!(webhook_url = %url, scheme = %parsed.scheme(), "Webhook URL scheme rejected");
        return false;
    }

    if parsed.password().is_some() || !parsed.username().is_empty() {
        tracing::warn!(webhook_url = %url, "Webhook URL userinfo rejected");
        return false;
    }

    let host = if let Some(host) = parsed.host_str() { host } else {
        tracing::warn!(webhook_url = %url, "Webhook URL host missing");
        return false;
    };

    if is_localhostish_host(host) {
        tracing::warn!(webhook_url = %url, "Webhook URL host rejected (localhost-like)");
        return false;
    }

    // Reject all IPv6-mapped IPv4 literals up front (`::ffff:...`). Even when
    // syntactically valid, these are unnecessary for webhook endpoints and are
    // a common bypass vector for localhost/private filtering logic.
    let normalized_host = host.trim().trim_matches('[').trim_matches(']').to_ascii_lowercase();
    if normalized_host.contains("::ffff:") {
        tracing::warn!(webhook_url = %url, "Webhook URL host rejected (IPv6-mapped IPv4 literal)");
        return false;
    }

    // Some `host_str` forms of IPv6-mapped IPv4 literals can evade strict
    // `IpAddr` parsing depending on representation. Handle dotted suffix forms
    // explicitly to keep registration checks deterministic.
    if let Some(mapped_v4) = parse_ipv6_mapped_ipv4_host(host) {
        if is_public_ipv4(mapped_v4) {
            return true;
        }
        tracing::warn!(
            webhook_url = %url,
            mapped_ipv4 = %mapped_v4,
            "Webhook URL host rejected (IPv6-mapped non-public IPv4)"
        );
        return false;
    }

    if parse_decimal_ipv4_literal(host).is_some() || is_ambiguous_ipv4_encoding_host(host) {
        tracing::warn!(webhook_url = %url, "Webhook URL host rejected (ambiguous IPv4 encoding)");
        return false;
    }

    if !outbound_allowlist.is_empty() && !host_matches_allowlist(host, outbound_allowlist) {
        tracing::warn!(webhook_url = %url, "Webhook URL host rejected (not in outbound allowlist)");
        return false;
    }

    let host_for_ip_parse = host.trim().trim_matches('[').trim_matches(']');
    if let Ok(ipv6) = host_for_ip_parse.parse::<Ipv6Addr>() {
        if is_public_ipv6(ipv6) {
            return true;
        }
        tracing::warn!(webhook_url = %url, ip = %ipv6, "Webhook URL host IP rejected");
        return false;
    }
    if let Ok(ipv4) = host_for_ip_parse.parse::<Ipv4Addr>() {
        if is_public_ipv4(ipv4) {
            return true;
        }
        tracing::warn!(webhook_url = %url, ip = %ipv4, "Webhook URL host IP rejected");
        return false;
    }

    if resolve_dns {
        is_public_hostname_with_resolver(
            host,
            parsed.port_or_known_default().unwrap_or(443),
            dns_resolver,
        )
    } else {
        true
    }
}

fn parse_ipv6_mapped_ipv4_host(host: &str) -> Option<Ipv4Addr> {
    let normalized = host.trim().trim_matches('[').trim_matches(']').to_ascii_lowercase();
    let mapped = normalized.strip_prefix("::ffff:")?;
    mapped.parse::<Ipv4Addr>().ok()
}

fn is_localhostish_host(host: &str) -> bool {
    let normalized = host.trim().trim_end_matches('.').to_ascii_lowercase();

    normalized == "localhost"
        || normalized == "localhost.localdomain"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".local")
}

fn is_public_hostname_with_resolver<F>(host: &str, port: u16, dns_resolver: &F) -> bool
where
    F: Fn(&str, u16) -> Result<Vec<IpAddr>, String>,
{
    let resolved = match dns_resolver(host, port) {
        Ok(ips) => ips,
        Err(err) => {
            tracing::warn!(webhook_host = %host, error = %err, "Webhook hostname DNS resolution failed");
            return false;
        }
    };

    if resolved.is_empty() {
        tracing::warn!(webhook_host = %host, "Webhook hostname did not resolve to any address");
        return false;
    }

    for ip in &resolved {
        if !is_public_ip(*ip) {
            tracing::warn!(webhook_host = %host, ip = %ip, "Webhook hostname resolves to private or local network addresses");
            return false;
        }
    }

    true
}

fn resolve_host_ips(host: &str, port: u16) -> Result<Vec<IpAddr>, String> {
    match (host, port).to_socket_addrs() {
        Ok(addrs) => {
            let mut ips = Vec::new();
            for socket_addr in addrs {
                let ip = socket_addr.ip();
                if !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
            Ok(ips)
        }
        Err(err) => Err(err.to_string()),
    }
}

fn parse_decimal_ipv4_literal(host: &str) -> Option<Ipv4Addr> {
    if !host.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = host.parse::<u32>().ok()?;
    Some(Ipv4Addr::from(n))
}

fn is_ambiguous_ipv4_encoding_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.is_empty() || labels.len() > 4 {
        return false;
    }

    let mut saw_hex = false;
    for label in &labels {
        if label.is_empty() {
            return false;
        }

        if let Some(hex_digits) = label.strip_prefix("0x") {
            if hex_digits.is_empty() || !hex_digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                return false;
            }
            saw_hex = true;
            continue;
        }

        if !label.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }

    if labels.len() == 1 {
        return saw_hex;
    }

    saw_hex
        || labels.len() < 4
        || labels.iter().any(|label| label.len() > 1 && label.starts_with('0'))
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) => is_public_ipv4(addr),
        IpAddr::V6(addr) => is_public_ipv6(addr),
    }
}

fn is_public_ipv4(addr: Ipv4Addr) -> bool {
    let octets = addr.octets();

    // Reject all non-public, local, and special-use ranges.
    if addr.is_loopback()
        || addr.is_private()
        || addr.is_link_local()
        || addr.is_multicast()
        || addr.is_unspecified()
        || addr.is_broadcast()
        || addr.is_documentation()
    {
        return false;
    }

    // 0.0.0.0/8 ("this" network)
    if octets[0] == 0 {
        return false;
    }

    // 100.64.0.0/10 (carrier-grade NAT)
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return false;
    }

    // 192.0.0.0/24 (IETF protocol assignments)
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        return false;
    }

    // 198.18.0.0/15 (benchmarking)
    if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
        return false;
    }

    // 240.0.0.0/4 (future use / reserved)
    if octets[0] >= 240 {
        return false;
    }

    true
}

const fn is_public_ipv6(addr: Ipv6Addr) -> bool {
    if addr.to_ipv4_mapped().is_some() {
        return false;
    }

    let segments = addr.segments();
    let first = segments[0];

    // fc00::/7 unique-local, fe80::/10 link-local, fec0::/10 site-local (deprecated)
    let is_unique_local = (first & 0xfe00) == 0xfc00;
    let is_link_local = (first & 0xffc0) == 0xfe80;
    let is_site_local = (first & 0xffc0) == 0xfec0;
    // 2001:db8::/32 documentation
    let is_documentation = first == 0x2001 && segments[1] == 0x0db8;

    !addr.is_loopback()
        && !addr.is_unspecified()
        && !addr.is_multicast()
        && !is_unique_local
        && !is_link_local
        && !is_site_local
        && !is_documentation
}

/// Webhook payload wrapper
#[derive(Debug, Serialize, Deserialize)]
struct WebhookPayload {
    id: Uuid,
    event_type: String,
    timestamp: DateTime<Utc>,
    data: CommerceEvent,
}

/// Compute HMAC-SHA256 signature for webhook payload
fn compute_signature(secret: &str, body: &str) -> Option<String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(err) => {
            tracing::error!(error = %err, "Failed to initialize webhook HMAC");
            return None;
        }
    };
    mac.update(body.as_bytes());
    let result = mac.finalize();

    Some(format!("sha256={}", hex::encode(result.into_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_creation() {
        let webhook = Webhook::new("Test Hook", "https://example.com/webhook")
            .with_secret("mysecret")
            .with_events(vec!["order_created".to_string()])
            .with_header("X-Custom", "value");

        assert_eq!(webhook.name, "Test Hook");
        assert_eq!(webhook.url, "https://example.com/webhook");
        assert_eq!(webhook.secret, Some("mysecret".to_string()));
        assert_eq!(webhook.event_types, vec!["order_created"]);
        assert_eq!(webhook.headers.get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_webhook_should_receive() {
        use chrono::Utc;
        use rust_decimal_macros::dec;

        let webhook = Webhook::new("Test", "https://example.com")
            .with_events(vec!["order_created".to_string()]);

        let order_event = CommerceEvent::OrderCreated {
            order_id: stateset_core::OrderId::new(),
            customer_id: stateset_core::CustomerId::new(),
            total_amount: dec!(100),
            item_count: 1,
            timestamp: Utc::now(),
        };

        let customer_event = CommerceEvent::CustomerCreated {
            customer_id: stateset_core::CustomerId::new(),
            email: "test@example.com".to_string(),
            timestamp: Utc::now(),
        };

        assert!(webhook.should_receive(&order_event));
        assert!(!webhook.should_receive(&customer_event));
    }

    #[test]
    fn test_compute_signature() {
        let signature = compute_signature("secret", "test body").expect("signature");
        assert!(signature.starts_with("sha256="));
    }

    #[test]
    fn test_webhook_register_rejects_localhost() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Localhost Hook", "http://localhost:8080/webhook");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
        assert!(manager.list().is_empty());
    }

    #[test]
    fn test_webhook_register_rejects_private_ip() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Private IP Hook", "https://10.0.0.1/callback");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
        assert!(manager.list().is_empty());
    }

    #[test]
    fn test_webhook_register_rejects_ipv6_mapped_ipv4_loopback() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("IPv6 mapped loopback hook", "https://[::ffff:127.0.0.1]/cb");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
    }

    #[test]
    fn test_webhook_register_rejects_ipv6_mapped_ipv4_private() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("IPv6 mapped private hook", "https://[::ffff:10.0.0.1]/cb");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
    }

    #[test]
    fn test_webhook_register_rejects_shared_ipv4_space() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("CGNAT hook", "https://100.64.0.1/cb");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
    }

    #[test]
    fn test_webhook_register_accepts_public_ipv6() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Public IPv6 Hook", "https://[2606:4700:4700::1111]/callback");
        let id = manager.try_register(webhook).expect("expected public IPv6 to be accepted");
        assert_ne!(id, Uuid::nil());
    }

    #[test]
    fn test_webhook_config_normalizes_runtime_limits() {
        let manager = WebhookManager::with_config(WebhookConfig {
            max_retries: 4,
            timeout_secs: 0,
            max_in_flight: 0,
            retry_delay_ms: 0,
            max_delivery_history: 7,
            outbound_allowlist: vec![
                "  API.EXAMPLE.COM. ".to_string(),
                "*.Example.com.".to_string(),
            ],
        });

        assert_eq!(manager.config.max_retries, 4);
        assert_eq!(manager.config.timeout_secs, 1);
        assert_eq!(manager.config.max_in_flight, 1);
        assert_eq!(manager.config.retry_delay_ms, 1);
        assert_eq!(manager.config.max_delivery_history, 7);
        assert_eq!(
            manager.config.outbound_allowlist,
            vec!["api.example.com".to_string(), "*.example.com".to_string()]
        );
    }

    #[test]
    fn test_webhook_register_accepts_public_ip() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Public IP Hook", "https://1.1.1.1/callback");

        let id = manager.try_register(webhook).expect("expected public IP to be accepted");
        let registered = manager.get(id).expect("registered webhook should exist");
        assert_eq!(registered.url, "https://1.1.1.1/callback");
    }

    #[test]
    fn test_webhook_register_rejects_non_http_scheme() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("FTP Hook", "ftp://example.com/webhook");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
    }

    #[test]
    fn test_webhook_register_rejects_userinfo() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Userinfo Hook", "https://user:pass@example.com/webhook");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
    }

    #[test]
    fn test_webhook_register_rejects_invalid_url() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Bad URL Hook", "not-a-url");

        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        assert!(manager.try_register(webhook).is_none());
    }

    #[test]
    fn test_webhook_deliveries_keeps_newest_first_and_respects_limit() {
        let manager = WebhookManager::with_config(WebhookConfig {
            max_delivery_history: 2,
            ..Default::default()
        });
        let webhook = Webhook::new("History Hook", "https://example.com/webhook");
        let webhook_id = webhook.id;

        let mk_delivery = |record_id: Uuid, status: DeliveryStatus| WebhookDelivery {
            id: record_id,
            webhook_id,
            event_type: "order_created".to_string(),
            event_id: Uuid::new_v4(),
            status,
            attempts: 1,
            last_attempt_at: None,
            response_status: None,
            response_body: None,
            error: None,
            created_at: Utc::now(),
        };

        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();

        append_delivery_record(
            &manager.delivery_history,
            webhook_id,
            mk_delivery(first, DeliveryStatus::Pending),
            2,
        );
        append_delivery_record(
            &manager.delivery_history,
            webhook_id,
            mk_delivery(second, DeliveryStatus::Delivered),
            2,
        );
        append_delivery_record(
            &manager.delivery_history,
            webhook_id,
            mk_delivery(third, DeliveryStatus::Failed),
            2,
        );

        let deliveries = manager.deliveries(webhook_id);
        assert_eq!(deliveries.len(), 2);
        assert_eq!(deliveries[0].id, third);
        assert_eq!(deliveries[1].id, second);
    }

    #[test]
    fn test_webhook_deliveries_respects_zero_cap() {
        let manager = WebhookManager::with_config(WebhookConfig {
            max_delivery_history: 0,
            ..Default::default()
        });
        let webhook = Webhook::new("No History Hook", "https://example.com/webhook");
        let webhook_id = webhook.id;

        append_delivery_record(
            &manager.delivery_history,
            webhook_id,
            WebhookDelivery {
                id: Uuid::new_v4(),
                webhook_id,
                event_type: "order_created".to_string(),
                event_id: Uuid::new_v4(),
                status: DeliveryStatus::Delivered,
                attempts: 1,
                last_attempt_at: None,
                response_status: None,
                response_body: None,
                error: None,
                created_at: Utc::now(),
            },
            0,
        );

        assert_eq!(manager.deliveries(webhook_id).len(), 0);
    }

    #[test]
    fn test_webhook_unregister_clears_delivery_history() {
        let manager = WebhookManager::with_config(WebhookConfig {
            max_delivery_history: 3,
            ..Default::default()
        });
        let webhook = Webhook::new("History Hook", "https://example.com/webhook");
        let webhook_id = webhook.id;
        manager.try_register(webhook).expect("webhook should register");

        append_delivery_record(
            &manager.delivery_history,
            webhook_id,
            WebhookDelivery {
                id: Uuid::new_v4(),
                webhook_id,
                event_type: "order_created".to_string(),
                event_id: Uuid::new_v4(),
                status: DeliveryStatus::Delivered,
                attempts: 1,
                last_attempt_at: None,
                response_status: None,
                response_body: None,
                error: None,
                created_at: Utc::now(),
            },
            3,
        );

        assert_eq!(manager.deliveries(webhook_id).len(), 1);

        assert!(manager.unregister(webhook_id));
        assert_eq!(manager.deliveries(webhook_id).len(), 0);
        assert!(manager.list().is_empty());
    }

    #[test]
    fn test_webhook_register_fallback_returns_nil() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Bad URL Hook", "gopher://127.0.0.1/webhook");
        assert_eq!(
            manager.register_strict(webhook.clone()),
            Err(WebhookRegistrationError::UnsafeUrl)
        );
        let id = manager.register(webhook);

        assert_eq!(id, Uuid::nil());
        assert!(manager.list().is_empty());
    }

    #[test]
    fn test_webhook_register_rejects_duplicate_id() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Primary", "https://example.com/webhook");
        let duplicate = webhook.clone();

        let first_id = manager.register_strict(webhook).unwrap();
        assert_eq!(first_id, duplicate.id);
        assert_eq!(manager.register_strict(duplicate), Err(WebhookRegistrationError::DuplicateId));
        assert_eq!(manager.list().len(), 1);
    }

    #[test]
    fn test_begin_delivery_attempt_clears_stale_metadata() {
        let mut delivery = WebhookDelivery {
            id: Uuid::new_v4(),
            webhook_id: Uuid::new_v4(),
            event_type: "order_created".to_string(),
            event_id: Uuid::new_v4(),
            status: DeliveryStatus::Retrying,
            attempts: 1,
            last_attempt_at: Some(Utc::now()),
            response_status: Some(500),
            response_body: Some("bad gateway".to_string()),
            error: Some("connection refused".to_string()),
            created_at: Utc::now(),
        };

        begin_delivery_attempt(&mut delivery, 2);

        assert_eq!(delivery.attempts, 2);
        assert!(delivery.last_attempt_at.is_some());
        assert_eq!(delivery.response_status, None);
        assert_eq!(delivery.response_body, None);
        assert_eq!(delivery.error, None);
    }

    #[test]
    fn test_webhook_register_rejects_decimal_ipv4_literal_encoding() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Decimal Literal", "https://2130706433/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_hex_ipv4_literal_encoding() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Hex Literal", "https://0x7f000001/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_octal_ipv4_literal_encoding() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Octal Literal", "https://0177.0.0.1/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_short_ipv4_literal_encoding() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Short Literal", "https://127.1/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_link_local_ipv6() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Link Local", "https://[fe80::1]/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_reserved_documentation_ipv4() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Doc IPv4", "https://203.0.113.9/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_userinfo_confusion_variant() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Userinfo Confusion", "https://safe.example.com@127.0.0.1/cb");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_register_rejects_malformed_authority() {
        let manager = WebhookManager::new(3, 30);
        let webhook = Webhook::new("Malformed", "https://[::1/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_allowlist_allows_exact_host() {
        let manager = WebhookManager::with_config(
            WebhookConfig::default().with_outbound_allowlist(["api.example.com"]),
        );
        let webhook = Webhook::new("Allowlisted", "https://api.example.com/callback");
        assert!(manager.try_register(webhook).is_some());
    }

    #[test]
    fn test_webhook_allowlist_blocks_non_listed_host() {
        let manager = WebhookManager::with_config(
            WebhookConfig::default().with_outbound_allowlist(["api.example.com"]),
        );
        let webhook = Webhook::new("Blocked", "https://other.example.com/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_allowlist_supports_wildcard_subdomain_only() {
        let manager = WebhookManager::with_config(
            WebhookConfig::default().with_outbound_allowlist(["*.example.com"]),
        );
        assert!(
            manager
                .try_register(Webhook::new("Subdomain", "https://hooks.example.com/callback"))
                .is_some()
        );
        assert_eq!(
            manager.try_register(Webhook::new("Apex", "https://example.com/callback")),
            None
        );
    }

    #[test]
    fn test_webhook_allowlist_cannot_override_private_ip_rejection() {
        let manager = WebhookManager::with_config(
            WebhookConfig::default().with_outbound_allowlist(["127.0.0.1"]),
        );
        let webhook = Webhook::new("Private", "https://127.0.0.1/callback");
        assert_eq!(manager.try_register(webhook), None);
    }

    #[test]
    fn test_webhook_dns_rebinding_style_resolution_mixed_ips_is_rejected() {
        let allowlist = vec!["api.example.com".to_string()];
        let is_safe = is_safe_webhook_url_with_dns_resolver(
            "https://api.example.com/callback",
            true,
            &allowlist,
            &|_host, _port| {
                Ok(vec![
                    IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                ])
            },
        );
        assert!(!is_safe);
    }

    #[test]
    fn test_webhook_dns_rebinding_style_resolution_all_public_is_allowed() {
        let allowlist = vec!["api.example.com".to_string()];
        let is_safe = is_safe_webhook_url_with_dns_resolver(
            "https://api.example.com/callback",
            true,
            &allowlist,
            &|_host, _port| {
                Ok(vec![
                    IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                ])
            },
        );
        assert!(is_safe);
    }
}
