//! Webhook delivery system for external event notifications

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_core::CommerceEvent;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout_secs: 30,
            retry_delay_ms: 1000,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Retrying,
}

/// Webhook manager handles registration and delivery
pub struct WebhookManager {
    webhooks: Arc<RwLock<HashMap<Uuid, Webhook>>>,
    config: WebhookConfig,
    client: reqwest::Client,
}

impl WebhookManager {
    /// Create a new webhook manager
    pub fn new(max_retries: u32, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            webhooks: Arc::new(RwLock::new(HashMap::new())),
            config: WebhookConfig {
                max_retries,
                timeout_secs,
                ..Default::default()
            },
            client,
        }
    }

    /// Register a new webhook
    pub fn register(&self, webhook: Webhook) -> Uuid {
        let id = webhook.id;
        self.webhooks.write().unwrap().insert(id, webhook);
        tracing::info!(webhook_id = %id, "Webhook registered");
        id
    }

    /// Unregister a webhook
    pub fn unregister(&self, id: Uuid) -> bool {
        let removed = self.webhooks.write().unwrap().remove(&id).is_some();
        if removed {
            tracing::info!(webhook_id = %id, "Webhook unregistered");
        }
        removed
    }

    /// Get a webhook by ID
    pub fn get(&self, id: Uuid) -> Option<Webhook> {
        self.webhooks.read().unwrap().get(&id).cloned()
    }

    /// List all webhooks
    pub fn list(&self) -> Vec<Webhook> {
        self.webhooks.read().unwrap().values().cloned().collect()
    }

    /// Update a webhook
    pub fn update(&self, webhook: Webhook) -> bool {
        let id = webhook.id;
        let mut webhooks = self.webhooks.write().unwrap();
        if webhooks.contains_key(&id) {
            webhooks.insert(id, webhook);
            true
        } else {
            false
        }
    }

    /// Enable/disable a webhook
    pub fn set_active(&self, id: Uuid, active: bool) -> bool {
        let mut webhooks = self.webhooks.write().unwrap();
        if let Some(webhook) = webhooks.get_mut(&id) {
            webhook.active = active;
            true
        } else {
            false
        }
    }

    /// Deliver an event to all matching webhooks (spawns async tasks)
    pub fn deliver(&self, event: CommerceEvent) {
        let webhooks: Vec<Webhook> = self
            .webhooks
            .read()
            .unwrap()
            .values()
            .filter(|w| w.should_receive(&event))
            .cloned()
            .collect();

        if webhooks.is_empty() {
            return;
        }

        let client = self.client.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            for webhook in webhooks {
                let client = client.clone();
                let event = event.clone();
                let config = config.clone();

                tokio::spawn(async move {
                    deliver_to_webhook(&client, &webhook, &event, &config).await;
                });
            }
        });
    }
}

/// Deliver an event to a specific webhook with retries
async fn deliver_to_webhook(
    client: &reqwest::Client,
    webhook: &Webhook,
    event: &CommerceEvent,
    config: &WebhookConfig,
) {
    let payload = WebhookPayload {
        id: Uuid::new_v4(),
        event_type: event.event_type().to_string(),
        timestamp: event.timestamp(),
        data: event.clone(),
    };

    let body = match serde_json::to_string(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize webhook payload");
            return;
        }
    };

    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(
                config.retry_delay_ms * (1 << (attempt - 1)), // Exponential backoff
            ))
            .await;
        }

        let mut request = client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-ID", webhook.id.to_string())
            .header("X-Event-Type", event.event_type())
            .header("X-Delivery-ID", payload.id.to_string())
            .header("X-Delivery-Attempt", (attempt + 1).to_string());

        // Add HMAC signature if secret is configured
        if let Some(ref secret) = webhook.secret {
            let signature = compute_signature(secret, &body);
            request = request.header("X-Signature", signature);
        }

        // Add custom headers
        for (key, value) in &webhook.headers {
            request = request.header(key, value);
        }

        match request.body(body.clone()).send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    tracing::debug!(
                        webhook_id = %webhook.id,
                        event_type = event.event_type(),
                        status = %status,
                        attempt = attempt + 1,
                        "Webhook delivered successfully"
                    );
                    return;
                } else {
                    let body = response.text().await.unwrap_or_default();
                    tracing::warn!(
                        webhook_id = %webhook.id,
                        event_type = event.event_type(),
                        status = %status,
                        attempt = attempt + 1,
                        response = %body,
                        "Webhook delivery failed with non-success status"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    webhook_id = %webhook.id,
                    event_type = event.event_type(),
                    attempt = attempt + 1,
                    error = %e,
                    "Webhook delivery failed"
                );
            }
        }
    }

    tracing::error!(
        webhook_id = %webhook.id,
        event_type = event.event_type(),
        max_retries = config.max_retries,
        "Webhook delivery exhausted all retries"
    );
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
fn compute_signature(secret: &str, body: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(body.as_bytes());
    let result = mac.finalize();

    format!("sha256={}", hex::encode(result.into_bytes()))
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
            order_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            total_amount: dec!(100),
            item_count: 1,
            timestamp: Utc::now(),
        };

        let customer_event = CommerceEvent::CustomerCreated {
            customer_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            timestamp: Utc::now(),
        };

        assert!(webhook.should_receive(&order_event));
        assert!(!webhook.should_receive(&customer_event));
    }

    #[test]
    fn test_compute_signature() {
        let signature = compute_signature("secret", "test body");
        assert!(signature.starts_with("sha256="));
    }
}
