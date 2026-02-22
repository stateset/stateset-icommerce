//! Event streaming infrastructure
//!
//! This module provides real-time event streaming capabilities:
//! - In-process pub/sub via broadcast channels
//! - Event persistence via `EventStore`
//! - Webhook delivery to external endpoints
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CommerceEvent};
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let commerce = Commerce::new(":memory:")?;
//!
//!     // Subscribe to all events
//!     let mut stream = commerce.events().subscribe();
//!
//!     // Process events in background
//!     tokio::spawn(async move {
//!         while let Some(event) = stream.next().await {
//!             println!("Event: {:?}", event);
//!         }
//!     });
//!
//!     // Events are emitted automatically when operations occur
//!     commerce.orders().create(...)?;  // Emits OrderCreated event
//!
//!     Ok(())
//! }
//! ```

mod bus;
mod emitter;
mod store;
mod webhook;

pub use bus::{EventBus, EventReceiver, EventSubscription};
pub use emitter::EventEmitter;
pub use store::InMemoryEventStore;
pub use webhook::{
    Webhook, WebhookConfig, WebhookDelivery, WebhookManager, WebhookRegistrationError,
};

#[cfg(feature = "sqlite-events")]
pub use store::SqliteEventStore;

#[cfg(feature = "postgres")]
pub use store::PostgresEventStore;

use stateset_core::{CommerceEvent, EventStore};
use std::sync::Arc;

/// Configuration for the event system
#[derive(Clone)]
pub struct EventConfig {
    /// Channel buffer size for broadcast
    pub channel_capacity: usize,
    /// Whether to persist events to the store
    pub persist_events: bool,
    /// Optional event store for persistence
    pub event_store: Option<Arc<dyn EventStore + Send + Sync>>,
    /// Maximum events to keep in the default in-memory store
    pub max_in_memory_events: usize,
    /// Whether to enable webhook delivery
    pub enable_webhooks: bool,
    /// Maximum retry attempts for webhook delivery
    pub webhook_max_retries: u32,
    /// Webhook timeout in seconds
    pub webhook_timeout_secs: u64,
    /// Maximum number of webhook requests in flight concurrently
    /// (values <= 1 are treated as 1 at runtime)
    pub webhook_max_in_flight: usize,
    /// Base delay in milliseconds for webhook retry backoff.
    /// Each retry uses exponential backoff up to a hard cap.
    pub webhook_retry_delay_ms: u64,
    /// Maximum webhook delivery records retained per webhook.
    /// Zero disables history retention for that webhook.
    pub webhook_max_delivery_history: usize,
}


impl Default for EventConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1024,
            persist_events: true,
            event_store: None,
            max_in_memory_events: 10_000,
            enable_webhooks: true,
            webhook_max_retries: 3,
            webhook_timeout_secs: 30,
            webhook_max_in_flight: 8,
            webhook_retry_delay_ms: 1000,
            webhook_max_delivery_history: 1_000,
        }
    }
}

impl std::fmt::Debug for EventConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventConfig")
            .field("channel_capacity", &self.channel_capacity)
            .field("persist_events", &self.persist_events)
            .field("event_store", &self.event_store.as_ref().map(|_| "<custom>"))
            .field("max_in_memory_events", &self.max_in_memory_events)
            .field("enable_webhooks", &self.enable_webhooks)
            .field("webhook_max_retries", &self.webhook_max_retries)
            .field("webhook_timeout_secs", &self.webhook_timeout_secs)
            .field("webhook_max_in_flight", &self.webhook_max_in_flight)
            .field("webhook_retry_delay_ms", &self.webhook_retry_delay_ms)
            .field("webhook_max_delivery_history", &self.webhook_max_delivery_history)
            .finish()
    }
}

/// Central event system that coordinates all event delivery mechanisms
pub struct EventSystem {
    bus: Arc<EventBus>,
    emitter: EventEmitter,
    webhook_manager: Option<WebhookManager>,
    event_store: Option<Arc<dyn EventStore + Send + Sync>>,
    config: EventConfig,
}

impl std::fmt::Debug for EventSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSystem")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl EventSystem {
    /// Create a new event system with default configuration
    pub fn new() -> Self {
        Self::with_config(EventConfig::default())
    }

    /// Create a new event system with custom configuration
    pub fn with_config(config: EventConfig) -> Self {
        let config = EventConfig {
            channel_capacity: config.channel_capacity.max(1),
            max_in_memory_events: config.max_in_memory_events.max(1),
            webhook_max_in_flight: config.webhook_max_in_flight.max(1),
            webhook_timeout_secs: config.webhook_timeout_secs.max(1),
            webhook_retry_delay_ms: config.webhook_retry_delay_ms.max(1),
            ..config
        };

        let bus = Arc::new(EventBus::new(config.channel_capacity));
        let emitter = EventEmitter::new(bus.clone());
        let webhook_manager = if config.enable_webhooks {
            Some(WebhookManager::with_config(WebhookConfig {
                max_retries: config.webhook_max_retries,
                timeout_secs: config.webhook_timeout_secs,
                max_in_flight: config.webhook_max_in_flight,
                retry_delay_ms: config.webhook_retry_delay_ms,
                max_delivery_history: config.webhook_max_delivery_history,
            }))
        } else {
            None
        };
        let event_store =
            if config.persist_events {
                Some(config.event_store.clone().unwrap_or_else(|| {
                    Arc::new(InMemoryEventStore::new(config.max_in_memory_events))
                }))
            } else {
                None
            };

        Self { bus, emitter, webhook_manager, event_store, config }
    }

    #[cfg(test)]
    fn is_webhooks_enabled(&self) -> bool {
        self.webhook_manager.is_some()
    }
}

impl EventSystem {
    /// Get the event emitter for publishing events
    pub const fn emitter(&self) -> &EventEmitter {
        &self.emitter
    }

    /// Subscribe to all events
    pub fn subscribe(&self) -> EventSubscription {
        self.bus.subscribe()
    }

    /// Subscribe to events matching a filter
    pub fn subscribe_filtered<F>(&self, filter: F) -> FilteredSubscription<F>
    where
        F: Fn(&CommerceEvent) -> bool + Send + 'static,
    {
        FilteredSubscription { inner: self.bus.subscribe(), filter }
    }

    /// Register a webhook endpoint.
    ///
    /// This is a legacy API that preserves the previous `Uuid`-returning
    /// contract. Prefer [`register_webhook_strict`](Self::register_webhook_strict) or
    /// [`try_register_webhook`](Self::try_register_webhook) when you need explicit
    /// failure handling.
    pub fn register_webhook(&self, webhook: Webhook) -> uuid::Uuid {
        self.register_webhook_strict(webhook).unwrap_or_else(|error| {
            tracing::warn!(error = %error, "Webhook registration fallback returned nil");
            uuid::Uuid::nil()
        })
    }

    /// Register a webhook endpoint with explicit failure semantics.
    pub fn register_webhook_strict(
        &self,
        webhook: Webhook,
    ) -> Result<uuid::Uuid, WebhookRegistrationError> {
        self.webhook_manager
            .as_ref()
            .ok_or(WebhookRegistrationError::WebhooksDisabled)
            .and_then(|wm| wm.register_strict(webhook))
    }

    /// Register a webhook endpoint.
    /// Returns `None` when registration is rejected (e.g. unsafe URL),
    /// or when webhooks are disabled.
    pub fn try_register_webhook(&self, webhook: Webhook) -> Option<uuid::Uuid> {
        self.webhook_manager.as_ref().and_then(|wm| wm.try_register(webhook))
    }

    /// Unregister a webhook
    pub fn unregister_webhook(&self, id: uuid::Uuid) -> bool {
        self.webhook_manager.as_ref().map(|wm| wm.unregister(id)).unwrap_or(false)
    }

    /// List all registered webhooks
    pub fn list_webhooks(&self) -> Vec<Webhook> {
        self.webhook_manager.as_ref().map(|wm| wm.list()).unwrap_or_default()
    }

    /// Get delivery history for a webhook (newest-first).
    pub fn webhook_deliveries(&self, webhook_id: uuid::Uuid) -> Vec<WebhookDelivery> {
        self.webhook_manager
            .as_ref()
            .map(|wm| wm.deliveries(webhook_id))
            .unwrap_or_default()
    }

    /// Get the event bus for advanced usage
    pub const fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }

    /// Get configuration
    pub const fn config(&self) -> &EventConfig {
        &self.config
    }

    /// Access the configured event store, if enabled.
    pub fn event_store(&self) -> Option<&(dyn EventStore + Send + Sync)> {
        self.event_store.as_deref()
    }

    /// Emit an event.
    ///
    /// Broadcast delivery is non-blocking. Webhook delivery is dispatched to a background task.
    /// Event persistence (if enabled) is best-effort and may block depending on the configured store.
    pub fn emit(&self, event: CommerceEvent) {
        if let Some(store) = &self.event_store {
            if let Err(err) = store.append(&event) {
                tracing::error!(
                    error = %err,
                    event_type = event.event_type(),
                    "Failed to persist event"
                );
            }
        }

        // Send to broadcast channel (non-blocking)
        self.emitter.emit(event.clone());

        // Send to webhooks (spawns async task)
        if let Some(ref wm) = self.webhook_manager {
            wm.deliver(event);
        }
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.bus.receiver_count()
    }

    /// Get total publish failures on the event bus
    pub fn bus_publish_failures(&self) -> u64 {
        self.emitter.total_publish_failures()
    }
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_system_config_normalizes_zero_limits() {
        let event_system = EventSystem::with_config(EventConfig {
            channel_capacity: 0,
            max_in_memory_events: 0,
            webhook_max_in_flight: 0,
            webhook_timeout_secs: 0,
            webhook_retry_delay_ms: 0,
            persist_events: false,
            enable_webhooks: false,
            ..Default::default()
        });

        let config = event_system.config();
        assert_eq!(config.channel_capacity, 1);
        assert_eq!(config.max_in_memory_events, 1);
        assert_eq!(config.webhook_max_in_flight, 1);
        assert_eq!(config.webhook_timeout_secs, 1);
        assert_eq!(config.webhook_retry_delay_ms, 1);
        assert!(!event_system.is_webhooks_enabled());
        assert!(event_system.event_store().is_none());
    }

    #[test]
    fn test_event_system_config_keeps_webhook_disabled() {
        let event_system = EventSystem::with_config(EventConfig {
            enable_webhooks: false,
            persist_events: false,
            ..Default::default()
        });

        assert!(!event_system.is_webhooks_enabled());
        assert!(event_system.event_store().is_none());
    }
}

/// A filtered event subscription
pub struct FilteredSubscription<F> {
    inner: EventSubscription,
    filter: F,
}

impl<F> std::fmt::Debug for FilteredSubscription<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilteredSubscription").finish_non_exhaustive()
    }
}

impl<F> FilteredSubscription<F>
where
    F: Fn(&CommerceEvent) -> bool,
{
    /// Receive the next event matching the filter
    pub async fn recv(&mut self) -> Option<CommerceEvent> {
        loop {
            match self.inner.recv().await {
                Some(event) if (self.filter)(&event) => return Some(event),
                Some(_) => continue,
                None => return None,
            }
        }
    }
}

/// Event type filter helpers
pub mod filters {
    use stateset_core::CommerceEvent;

    /// Filter for order events only
    pub const fn orders_only(event: &CommerceEvent) -> bool {
        matches!(
            event,
            CommerceEvent::OrderCreated { .. }
                | CommerceEvent::OrderStatusChanged { .. }
                | CommerceEvent::OrderPaymentStatusChanged { .. }
                | CommerceEvent::OrderFulfillmentStatusChanged { .. }
                | CommerceEvent::OrderCancelled { .. }
                | CommerceEvent::OrderItemAdded { .. }
                | CommerceEvent::OrderItemRemoved { .. }
        )
    }

    /// Filter for inventory events only
    pub const fn inventory_only(event: &CommerceEvent) -> bool {
        matches!(
            event,
            CommerceEvent::InventoryItemCreated { .. }
                | CommerceEvent::InventoryAdjusted { .. }
                | CommerceEvent::InventoryReserved { .. }
                | CommerceEvent::InventoryReservationReleased { .. }
                | CommerceEvent::InventoryReservationConfirmed { .. }
                | CommerceEvent::LowStockAlert { .. }
        )
    }

    /// Filter for customer events only
    pub const fn customers_only(event: &CommerceEvent) -> bool {
        matches!(
            event,
            CommerceEvent::CustomerCreated { .. }
                | CommerceEvent::CustomerUpdated { .. }
                | CommerceEvent::CustomerStatusChanged { .. }
                | CommerceEvent::CustomerAddressAdded { .. }
        )
    }

    /// Filter for product events only
    pub const fn products_only(event: &CommerceEvent) -> bool {
        matches!(
            event,
            CommerceEvent::ProductCreated { .. }
                | CommerceEvent::ProductUpdated { .. }
                | CommerceEvent::ProductStatusChanged { .. }
                | CommerceEvent::ProductVariantAdded { .. }
                | CommerceEvent::ProductVariantUpdated { .. }
        )
    }

    /// Filter for return events only
    pub const fn returns_only(event: &CommerceEvent) -> bool {
        matches!(
            event,
            CommerceEvent::ReturnRequested { .. }
                | CommerceEvent::ReturnStatusChanged { .. }
                | CommerceEvent::ReturnApproved { .. }
                | CommerceEvent::ReturnRejected { .. }
                | CommerceEvent::ReturnCompleted { .. }
                | CommerceEvent::RefundIssued { .. }
        )
    }

    /// Filter for low stock alerts
    pub const fn low_stock_alerts(event: &CommerceEvent) -> bool {
        matches!(event, CommerceEvent::LowStockAlert { .. })
    }

    /// Create a filter for events matching specific types
    pub fn event_types(types: &'static [&'static str]) -> impl Fn(&CommerceEvent) -> bool {
        move |event| types.contains(&event.event_type())
    }
}
