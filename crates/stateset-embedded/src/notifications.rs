//! Transactional email notification service.
//!
//! Maps [`CommerceEvent`]s to structured email payloads and delivers them via
//! configurable backends. The default backend POSTs a JSON payload to a
//! webhook URL, making it trivial to integrate with any email provider
//! (`SendGrid`, Mailgun, Postmark, etc.) through a thin relay.
//!
//! # Architecture
//!
//! ```text
//! CommerceEvent ──► NotificationService ──► EmailBackend ──► Webhook / Log
//!                    (event→email map)       (delivery)
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::notifications::{NotificationConfig, NotificationService, WebhookEmailBackend};
//!
//! let backend = WebhookEmailBackend::new("https://relay.example.com/email", Some("hmac-secret"));
//! let config = NotificationConfig {
//!     from_name: "Acme Store".into(),
//!     from_email: "orders@acme.com".into(),
//!     ..Default::default()
//! };
//! let service = NotificationService::new(config, Box::new(backend));
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_core::CommerceEvent;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// A function that resolves a commerce event to a recipient email address.
pub type RecipientResolver = Arc<dyn Fn(&CommerceEvent) -> Option<String> + Send + Sync>;

// ---------------------------------------------------------------------------
// Email template
// ---------------------------------------------------------------------------

/// Typed email template identifiers for commerce transactional emails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EmailTemplate {
    /// Confirmation sent when an order is placed.
    OrderConfirmation,
    /// Notification when an order ships.
    ShippingNotification,
    /// Update when a return request changes status.
    ReturnStatusUpdate,
    /// Notification when an order is cancelled.
    OrderCancellation,
    /// Notification when a refund is issued.
    RefundConfirmation,
    /// Alert when inventory drops below reorder point.
    LowStockAlert,
    /// Welcome email when a customer account is created.
    CustomerWelcome,
}

impl fmt::Display for EmailTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::OrderConfirmation => "order_confirmation",
            Self::ShippingNotification => "shipping_notification",
            Self::ReturnStatusUpdate => "return_status_update",
            Self::OrderCancellation => "order_cancellation",
            Self::RefundConfirmation => "refund_confirmation",
            Self::LowStockAlert => "low_stock_alert",
            Self::CustomerWelcome => "customer_welcome",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// Transactional email payload
// ---------------------------------------------------------------------------

/// A structured transactional email payload ready for delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionalEmail {
    /// Recipient email address.
    pub to: String,
    /// Sender display name.
    pub from_name: String,
    /// Sender email address.
    pub from_email: String,
    /// Email subject line.
    pub subject: String,
    /// Template identifier.
    pub template: EmailTemplate,
    /// Template variables (key-value pairs injected into the template).
    pub template_data: HashMap<String, serde_json::Value>,
    /// The source commerce event type that triggered this email.
    pub event_type: String,
    /// Timestamp of the originating event.
    pub event_timestamp: DateTime<Utc>,
    /// Unique message ID for idempotency / deduplication.
    pub message_id: uuid::Uuid,
}

// ---------------------------------------------------------------------------
// Delivery backend trait
// ---------------------------------------------------------------------------

/// Trait for email delivery backends.
///
/// Implementations receive a fully-formed [`TransactionalEmail`] and are
/// responsible for delivering it (or enqueuing it for delivery).
pub trait EmailBackend: Send + Sync + fmt::Debug {
    /// Deliver a transactional email.
    ///
    /// Returns `Ok(())` on successful delivery/enqueue, or an error message.
    fn deliver(&self, email: &TransactionalEmail) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Webhook backend
// ---------------------------------------------------------------------------

/// Delivers email payloads by posting JSON to a webhook URL.
///
/// The payload is signed with HMAC-SHA256 when a secret is configured,
/// using the `X-Signature-256` header (same scheme as the event webhook
/// system).
#[derive(Clone)]
pub struct WebhookEmailBackend {
    url: String,
    secret: Option<String>,
    #[cfg(feature = "events")]
    client: reqwest::blocking::Client,
}

impl fmt::Debug for WebhookEmailBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookEmailBackend")
            .field("url", &self.url)
            .field("has_secret", &self.secret.is_some())
            .finish()
    }
}

impl WebhookEmailBackend {
    /// Create a new webhook email backend.
    #[must_use]
    pub fn new(url: impl Into<String>, secret: Option<String>) -> Self {
        #[cfg(feature = "events")]
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            url: url.into(),
            secret,
            #[cfg(feature = "events")]
            client,
        }
    }

    /// Compute HMAC-SHA256 signature for a payload.
    #[cfg(feature = "events")]
    fn sign(&self, payload: &[u8]) -> Option<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let secret = self.secret.as_deref()?;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
        mac.update(payload);
        let result = mac.finalize();
        Some(format!("sha256={}", hex::encode(result.into_bytes())))
    }
}

impl EmailBackend for WebhookEmailBackend {
    fn deliver(&self, email: &TransactionalEmail) -> Result<(), String> {
        let payload =
            serde_json::to_vec(email).map_err(|e| format!("Failed to serialize email: {e}"))?;

        #[cfg(feature = "events")]
        {
            let mut request = self
                .client
                .post(&self.url)
                .header("Content-Type", "application/json")
                .header("User-Agent", "stateset-notifications/1.0");

            if let Some(sig) = self.sign(&payload) {
                request = request.header("X-Signature-256", sig);
            }

            let response = request
                .body(payload)
                .send()
                .map_err(|e| format!("Webhook delivery failed: {e}"))?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Webhook returned HTTP {}", response.status().as_u16()))
            }
        }

        #[cfg(not(feature = "events"))]
        {
            let _ = payload;
            Err("Webhook backend requires the `events` feature".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Log backend (for testing / development)
// ---------------------------------------------------------------------------

/// A backend that logs emails via `tracing` instead of delivering them.
///
/// Useful for development, testing, and dry-run scenarios.
#[derive(Debug, Clone, Default)]
pub struct LogEmailBackend {
    /// Collected emails (for test assertions).
    emails: Arc<std::sync::Mutex<Vec<TransactionalEmail>>>,
}

impl LogEmailBackend {
    /// Create a new log backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all emails that have been "delivered" (logged).
    #[must_use]
    pub fn emails(&self) -> Vec<TransactionalEmail> {
        self.emails.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }
}

impl EmailBackend for LogEmailBackend {
    fn deliver(&self, email: &TransactionalEmail) -> Result<(), String> {
        tracing::info!(
            template = %email.template,
            to = %email.to,
            subject = %email.subject,
            event_type = %email.event_type,
            message_id = %email.message_id,
            "Notification email (log backend)"
        );
        self.emails.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(email.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the notification service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Default sender display name.
    pub from_name: String,
    /// Default sender email address.
    pub from_email: String,
    /// Whether notifications are enabled.
    pub enabled: bool,
    /// Email templates that are enabled. Empty means all templates are enabled.
    pub enabled_templates: Vec<EmailTemplate>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            from_name: "StateSet Commerce".into(),
            from_email: "noreply@stateset.io".into(),
            enabled: true,
            enabled_templates: Vec::new(),
        }
    }
}

impl NotificationConfig {
    /// Check if a template is enabled.
    fn is_template_enabled(&self, template: EmailTemplate) -> bool {
        if !self.enabled {
            return false;
        }
        if self.enabled_templates.is_empty() {
            return true;
        }
        self.enabled_templates.contains(&template)
    }
}

// ---------------------------------------------------------------------------
// Notification service
// ---------------------------------------------------------------------------

/// Maps commerce events to transactional emails and delivers them via a
/// configurable backend.
pub struct NotificationService {
    config: NotificationConfig,
    backend: Box<dyn EmailBackend>,
    /// Optional recipient resolver: given an event, returns the email address.
    /// When `None`, only events that carry an email directly are deliverable.
    recipient_resolver: Option<RecipientResolver>,
}

impl fmt::Debug for NotificationService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationService")
            .field("config", &self.config)
            .field("backend", &self.backend)
            .field("has_resolver", &self.recipient_resolver.is_some())
            .finish()
    }
}

impl NotificationService {
    /// Create a new notification service.
    #[must_use]
    pub fn new(config: NotificationConfig, backend: Box<dyn EmailBackend>) -> Self {
        Self { config, backend, recipient_resolver: None }
    }

    /// Set a custom recipient resolver function.
    ///
    /// The resolver receives a `CommerceEvent` and should return the
    /// recipient email address, or `None` to skip sending.
    #[must_use]
    pub fn with_recipient_resolver(mut self, resolver: RecipientResolver) -> Self {
        self.recipient_resolver = Some(resolver);
        self
    }

    /// Get the service configuration.
    #[must_use]
    pub const fn config(&self) -> &NotificationConfig {
        &self.config
    }

    /// Process a commerce event, potentially sending an email notification.
    ///
    /// Returns `Ok(Some(message_id))` if an email was sent, `Ok(None)` if
    /// the event does not map to an email template or the template is
    /// disabled, and `Err` if delivery failed.
    pub fn process_event(&self, event: &CommerceEvent) -> Result<Option<uuid::Uuid>, String> {
        if !self.config.enabled {
            return Ok(None);
        }

        let Some(email) = self.event_to_email(event) else {
            return Ok(None);
        };

        self.backend.deliver(&email)?;

        tracing::debug!(
            template = %email.template,
            to = %email.to,
            message_id = %email.message_id,
            "Notification delivered"
        );

        Ok(Some(email.message_id))
    }

    /// Map a commerce event to a transactional email.
    ///
    /// Returns `None` if the event does not map to any email template, if
    /// the template is disabled, or if no recipient can be resolved.
    #[must_use]
    fn event_to_email(&self, event: &CommerceEvent) -> Option<TransactionalEmail> {
        let (template, subject, data) = self.extract_template_info(event)?;

        if !self.config.is_template_enabled(template) {
            return None;
        }

        let to = self.resolve_recipient(event)?;

        Some(TransactionalEmail {
            to,
            from_name: self.config.from_name.clone(),
            from_email: self.config.from_email.clone(),
            subject,
            template,
            template_data: data,
            event_type: event.event_type().to_string(),
            event_timestamp: event.timestamp(),
            message_id: uuid::Uuid::new_v4(),
        })
    }

    /// Extract template info from a commerce event.
    fn extract_template_info(
        &self,
        event: &CommerceEvent,
    ) -> Option<(EmailTemplate, String, HashMap<String, serde_json::Value>)> {
        match event {
            CommerceEvent::OrderCreated { order_id, total_amount, item_count, .. } => {
                let mut data = HashMap::new();
                data.insert("order_id".into(), serde_json::json!(order_id.to_string()));
                data.insert("total_amount".into(), serde_json::json!(total_amount.to_string()));
                data.insert("item_count".into(), serde_json::json!(item_count));
                Some((
                    EmailTemplate::OrderConfirmation,
                    format!("Order Confirmation — {order_id}"),
                    data,
                ))
            }
            CommerceEvent::OrderCancelled { order_id, reason, .. } => {
                let mut data = HashMap::new();
                data.insert("order_id".into(), serde_json::json!(order_id.to_string()));
                if let Some(reason) = reason {
                    data.insert("reason".into(), serde_json::json!(reason));
                }
                Some((
                    EmailTemplate::OrderCancellation,
                    format!("Order Cancelled — {order_id}"),
                    data,
                ))
            }
            CommerceEvent::OrderFulfillmentStatusChanged { order_id, to_status, .. } => {
                use stateset_core::FulfillmentStatus;
                if *to_status != FulfillmentStatus::Shipped {
                    return None;
                }
                let mut data = HashMap::new();
                data.insert("order_id".into(), serde_json::json!(order_id.to_string()));
                data.insert("status".into(), serde_json::json!(to_status.to_string()));
                Some((
                    EmailTemplate::ShippingNotification,
                    format!("Your Order Has Shipped — {order_id}"),
                    data,
                ))
            }
            CommerceEvent::ReturnStatusChanged { return_id, from_status, to_status, .. } => {
                let mut data = HashMap::new();
                data.insert("return_id".into(), serde_json::json!(return_id.to_string()));
                data.insert("from_status".into(), serde_json::json!(from_status.to_string()));
                data.insert("to_status".into(), serde_json::json!(to_status.to_string()));
                Some((
                    EmailTemplate::ReturnStatusUpdate,
                    format!("Return Update — {return_id}"),
                    data,
                ))
            }
            CommerceEvent::ReturnApproved { return_id, order_id, .. } => {
                let mut data = HashMap::new();
                data.insert("return_id".into(), serde_json::json!(return_id.to_string()));
                data.insert("order_id".into(), serde_json::json!(order_id.to_string()));
                Some((
                    EmailTemplate::ReturnStatusUpdate,
                    format!("Return Approved — {return_id}"),
                    data,
                ))
            }
            CommerceEvent::RefundIssued { return_id, order_id, amount, method, .. } => {
                let mut data = HashMap::new();
                data.insert("return_id".into(), serde_json::json!(return_id.to_string()));
                data.insert("order_id".into(), serde_json::json!(order_id.to_string()));
                data.insert("amount".into(), serde_json::json!(amount.to_string()));
                data.insert("method".into(), serde_json::json!(method));
                Some((
                    EmailTemplate::RefundConfirmation,
                    format!("Refund Issued — {order_id}"),
                    data,
                ))
            }
            CommerceEvent::LowStockAlert { sku, current_quantity, reorder_point, .. } => {
                let mut data = HashMap::new();
                data.insert("sku".into(), serde_json::json!(sku));
                data.insert(
                    "current_quantity".into(),
                    serde_json::json!(current_quantity.to_string()),
                );
                data.insert("reorder_point".into(), serde_json::json!(reorder_point.to_string()));
                Some((EmailTemplate::LowStockAlert, format!("Low Stock Alert — {sku}"), data))
            }
            CommerceEvent::CustomerCreated { customer_id, email, .. } => {
                let mut data = HashMap::new();
                data.insert("customer_id".into(), serde_json::json!(customer_id.to_string()));
                data.insert("email".into(), serde_json::json!(email));
                Some((EmailTemplate::CustomerWelcome, "Welcome to Our Store!".into(), data))
            }
            _ => None,
        }
    }

    /// Resolve the recipient email for an event.
    fn resolve_recipient(&self, event: &CommerceEvent) -> Option<String> {
        // First try the custom resolver
        if let Some(ref resolver) = self.recipient_resolver {
            if let Some(email) = resolver(event) {
                return Some(email);
            }
        }

        // Fall back to extracting email directly from event data
        match event {
            CommerceEvent::CustomerCreated { email, .. } => Some(email.clone()),
            // Other events don't carry emails directly — resolver is needed
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use stateset_core::{
        CustomerId, FulfillmentStatus, OrderId, ProductId, ReturnId, ReturnReason, ReturnStatus,
    };

    fn default_service() -> (NotificationService, Arc<std::sync::Mutex<Vec<TransactionalEmail>>>) {
        let backend = LogEmailBackend::new();
        let emails = backend.emails.clone();
        let config = NotificationConfig {
            from_name: "Test Store".into(),
            from_email: "test@example.com".into(),
            ..Default::default()
        };
        let service = NotificationService::new(config, Box::new(backend));
        (service, emails)
    }

    fn service_with_resolver()
    -> (NotificationService, Arc<std::sync::Mutex<Vec<TransactionalEmail>>>) {
        let (service, emails) = default_service();
        let service =
            service.with_recipient_resolver(Arc::new(|_| Some("customer@example.com".into())));
        (service, emails)
    }

    #[test]
    fn order_created_generates_confirmation() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: Decimal::new(9999, 2),
            item_count: 3,
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.is_ok());
        assert!(result.as_ref().ok().unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].template, EmailTemplate::OrderConfirmation);
        assert_eq!(sent[0].to, "customer@example.com");
        assert_eq!(sent[0].from_name, "Test Store");
        assert!(sent[0].subject.contains("Order Confirmation"));
        assert!(sent[0].template_data.contains_key("order_id"));
        assert!(sent[0].template_data.contains_key("total_amount"));
        assert!(sent[0].template_data.contains_key("item_count"));
    }

    #[test]
    fn order_cancelled_generates_cancellation() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::OrderCancelled {
            order_id: OrderId::new(),
            reason: Some("Customer request".into()),
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::OrderCancellation);
        assert_eq!(sent[0].template_data["reason"], "Customer request");
    }

    #[test]
    fn shipped_generates_shipping_notification() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::OrderFulfillmentStatusChanged {
            order_id: OrderId::new(),
            from_status: FulfillmentStatus::Unfulfilled,
            to_status: FulfillmentStatus::Shipped,
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::ShippingNotification);
        assert!(sent[0].subject.contains("Shipped"));
    }

    #[test]
    fn non_shipped_fulfillment_skipped() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::OrderFulfillmentStatusChanged {
            order_id: OrderId::new(),
            from_status: FulfillmentStatus::Unfulfilled,
            to_status: FulfillmentStatus::Fulfilled,
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_none());
        assert!(emails.lock().unwrap().is_empty());
    }

    #[test]
    fn return_status_change_generates_update() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::ReturnStatusChanged {
            return_id: ReturnId::new(),
            from_status: ReturnStatus::Requested,
            to_status: ReturnStatus::Approved,
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::ReturnStatusUpdate);
    }

    #[test]
    fn return_approved_generates_update() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::ReturnApproved {
            return_id: ReturnId::new(),
            order_id: OrderId::new(),
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::ReturnStatusUpdate);
        assert!(sent[0].subject.contains("Approved"));
    }

    #[test]
    fn refund_issued_generates_confirmation() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::RefundIssued {
            return_id: ReturnId::new(),
            order_id: OrderId::new(),
            amount: Decimal::new(2500, 2),
            method: "credit_card".into(),
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::RefundConfirmation);
        assert_eq!(sent[0].template_data["method"], "credit_card");
    }

    #[test]
    fn low_stock_alert_generates_email() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::LowStockAlert {
            sku: "WIDGET-001".into(),
            location_id: 1,
            current_quantity: Decimal::new(5, 0),
            reorder_point: Decimal::new(10, 0),
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::LowStockAlert);
        assert!(sent[0].subject.contains("WIDGET-001"));
    }

    #[test]
    fn customer_created_generates_welcome() {
        let (service, emails) = default_service();
        // CustomerCreated carries email directly — no resolver needed
        let event = CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: "alice@example.com".into(),
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_some());

        let sent = emails.lock().unwrap();
        assert_eq!(sent[0].template, EmailTemplate::CustomerWelcome);
        assert_eq!(sent[0].to, "alice@example.com");
        assert!(sent[0].subject.contains("Welcome"));
    }

    #[test]
    fn unhandled_event_returns_none() {
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::ProductCreated {
            product_id: ProductId::new(),
            name: "Widget".into(),
            slug: "widget".into(),
            timestamp: Utc::now(),
        };

        let result = service.process_event(&event);
        assert!(result.unwrap().is_none());
        assert!(emails.lock().unwrap().is_empty());
    }

    #[test]
    fn disabled_service_skips_all() {
        let backend = LogEmailBackend::new();
        let emails = backend.emails.clone();
        let config = NotificationConfig { enabled: false, ..Default::default() };
        let service = NotificationService::new(config, Box::new(backend))
            .with_recipient_resolver(Arc::new(|_| Some("a@b.com".into())));

        let event = CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: Decimal::new(100, 0),
            item_count: 1,
            timestamp: Utc::now(),
        };

        assert!(service.process_event(&event).unwrap().is_none());
        assert!(emails.lock().unwrap().is_empty());
    }

    #[test]
    fn template_filtering_works() {
        let backend = LogEmailBackend::new();
        let emails = backend.emails.clone();
        let config = NotificationConfig {
            enabled_templates: vec![EmailTemplate::LowStockAlert],
            ..Default::default()
        };
        let service = NotificationService::new(config, Box::new(backend))
            .with_recipient_resolver(Arc::new(|_| Some("a@b.com".into())));

        // OrderCreated is not in enabled list → skipped
        let event = CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: Decimal::new(100, 0),
            item_count: 1,
            timestamp: Utc::now(),
        };
        assert!(service.process_event(&event).unwrap().is_none());

        // LowStockAlert IS in enabled list → sent
        let event = CommerceEvent::LowStockAlert {
            sku: "SKU-001".into(),
            location_id: 1,
            current_quantity: Decimal::new(2, 0),
            reorder_point: Decimal::new(10, 0),
            timestamp: Utc::now(),
        };
        assert!(service.process_event(&event).unwrap().is_some());
        assert_eq!(emails.lock().unwrap().len(), 1);
    }

    #[test]
    fn no_resolver_no_email_for_order_events() {
        let (service, emails) = default_service();
        // No resolver set — OrderCreated has no email in the event itself
        let event = CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: Decimal::new(100, 0),
            item_count: 1,
            timestamp: Utc::now(),
        };

        assert!(service.process_event(&event).unwrap().is_none());
        assert!(emails.lock().unwrap().is_empty());
    }

    #[test]
    fn email_template_display() {
        assert_eq!(EmailTemplate::OrderConfirmation.to_string(), "order_confirmation");
        assert_eq!(EmailTemplate::ShippingNotification.to_string(), "shipping_notification");
        assert_eq!(EmailTemplate::ReturnStatusUpdate.to_string(), "return_status_update");
        assert_eq!(EmailTemplate::OrderCancellation.to_string(), "order_cancellation");
        assert_eq!(EmailTemplate::RefundConfirmation.to_string(), "refund_confirmation");
        assert_eq!(EmailTemplate::LowStockAlert.to_string(), "low_stock_alert");
        assert_eq!(EmailTemplate::CustomerWelcome.to_string(), "customer_welcome");
    }

    #[test]
    fn transactional_email_serializes() {
        let email = TransactionalEmail {
            to: "test@example.com".into(),
            from_name: "Store".into(),
            from_email: "noreply@store.com".into(),
            subject: "Test".into(),
            template: EmailTemplate::OrderConfirmation,
            template_data: HashMap::new(),
            event_type: "order_created".into(),
            event_timestamp: Utc::now(),
            message_id: uuid::Uuid::new_v4(),
        };

        let json = serde_json::to_string(&email).unwrap();
        assert!(json.contains("order_confirmation"));
        assert!(json.contains("test@example.com"));

        let roundtrip: TransactionalEmail = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.template, EmailTemplate::OrderConfirmation);
        assert_eq!(roundtrip.to, "test@example.com");
    }

    #[test]
    fn notification_config_default() {
        let config = NotificationConfig::default();
        assert!(config.enabled);
        assert!(config.enabled_templates.is_empty());
        assert_eq!(config.from_name, "StateSet Commerce");
        assert_eq!(config.from_email, "noreply@stateset.io");
    }

    #[test]
    fn webhook_backend_debug_hides_secret() {
        let backend =
            WebhookEmailBackend::new("https://example.com/email", Some("my_token_123".into()));
        let debug = format!("{backend:?}");
        assert!(debug.contains("has_secret: true"));
        // The actual secret value must not appear in debug output
        assert!(!debug.contains("my_token_123"));
    }

    #[test]
    fn log_backend_collects_emails() {
        let backend = LogEmailBackend::new();
        let email = TransactionalEmail {
            to: "a@b.com".into(),
            from_name: "S".into(),
            from_email: "n@s.com".into(),
            subject: "Hi".into(),
            template: EmailTemplate::CustomerWelcome,
            template_data: HashMap::new(),
            event_type: "customer_created".into(),
            event_timestamp: Utc::now(),
            message_id: uuid::Uuid::new_v4(),
        };

        backend.deliver(&email).unwrap();
        backend.deliver(&email).unwrap();

        let collected = backend.emails();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn return_requested_not_mapped() {
        // ReturnRequested is not directly mapped (ReturnStatusChanged is)
        let (service, emails) = service_with_resolver();
        let event = CommerceEvent::ReturnRequested {
            return_id: ReturnId::new(),
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            reason: ReturnReason::Defective,
            item_count: 1,
            timestamp: Utc::now(),
        };

        assert!(service.process_event(&event).unwrap().is_none());
        assert!(emails.lock().unwrap().is_empty());
    }

    #[test]
    fn message_id_is_unique() {
        let (service, emails) = service_with_resolver();

        for _ in 0..3 {
            let event = CommerceEvent::OrderCreated {
                order_id: OrderId::new(),
                customer_id: CustomerId::new(),
                total_amount: Decimal::new(100, 0),
                item_count: 1,
                timestamp: Utc::now(),
            };
            service.process_event(&event).unwrap();
        }

        let sent = emails.lock().unwrap();
        let ids: Vec<_> = sent.iter().map(|e| e.message_id).collect();
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
    }
}
