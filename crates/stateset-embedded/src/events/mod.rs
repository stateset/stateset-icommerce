//! Event streaming infrastructure
//!
//! This module provides real-time event streaming capabilities:
//! - In-process pub/sub via broadcast channels
//! - Event persistence via EventStore
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
pub use webhook::{Webhook, WebhookConfig, WebhookDelivery, WebhookManager};

#[cfg(feature = "sqlite-events")]
pub use store::SqliteEventStore;

#[cfg(feature = "postgres")]
pub use store::PostgresEventStore;

use stateset_core::CommerceEvent;
use std::sync::Arc;

/// Configuration for the event system
#[derive(Debug, Clone)]
pub struct EventConfig {
    /// Channel buffer size for broadcast
    pub channel_capacity: usize,
    /// Whether to persist events to the store
    pub persist_events: bool,
    /// Whether to enable webhook delivery
    pub enable_webhooks: bool,
    /// Maximum retry attempts for webhook delivery
    pub webhook_max_retries: u32,
    /// Webhook timeout in seconds
    pub webhook_timeout_secs: u64,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1024,
            persist_events: true,
            enable_webhooks: true,
            webhook_max_retries: 3,
            webhook_timeout_secs: 30,
        }
    }
}

/// Central event system that coordinates all event delivery mechanisms
pub struct EventSystem {
    bus: Arc<EventBus>,
    emitter: EventEmitter,
    webhook_manager: Option<WebhookManager>,
    config: EventConfig,
}

impl EventSystem {
    /// Create a new event system with default configuration
    pub fn new() -> Self {
        Self::with_config(EventConfig::default())
    }

    /// Create a new event system with custom configuration
    pub fn with_config(config: EventConfig) -> Self {
        let bus = Arc::new(EventBus::new(config.channel_capacity));
        let emitter = EventEmitter::new(bus.clone());
        let webhook_manager = if config.enable_webhooks {
            Some(WebhookManager::new(
                config.webhook_max_retries,
                config.webhook_timeout_secs,
            ))
        } else {
            None
        };

        Self {
            bus,
            emitter,
            webhook_manager,
            config,
        }
    }

    /// Get the event emitter for publishing events
    pub fn emitter(&self) -> &EventEmitter {
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
        FilteredSubscription {
            inner: self.bus.subscribe(),
            filter,
        }
    }

    /// Register a webhook endpoint
    pub fn register_webhook(&self, webhook: Webhook) -> Option<uuid::Uuid> {
        self.webhook_manager.as_ref().map(|wm| wm.register(webhook))
    }

    /// Unregister a webhook
    pub fn unregister_webhook(&self, id: uuid::Uuid) -> bool {
        self.webhook_manager
            .as_ref()
            .map(|wm| wm.unregister(id))
            .unwrap_or(false)
    }

    /// List all registered webhooks
    pub fn list_webhooks(&self) -> Vec<Webhook> {
        self.webhook_manager
            .as_ref()
            .map(|wm| wm.list())
            .unwrap_or_default()
    }

    /// Get the event bus for advanced usage
    pub fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }

    /// Get configuration
    pub fn config(&self) -> &EventConfig {
        &self.config
    }

    /// Emit an event (async, non-blocking)
    pub fn emit(&self, event: CommerceEvent) {
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
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// A filtered event subscription
pub struct FilteredSubscription<F> {
    inner: EventSubscription,
    filter: F,
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
    pub fn orders_only(event: &CommerceEvent) -> bool {
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
    pub fn inventory_only(event: &CommerceEvent) -> bool {
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
    pub fn customers_only(event: &CommerceEvent) -> bool {
        matches!(
            event,
            CommerceEvent::CustomerCreated { .. }
                | CommerceEvent::CustomerUpdated { .. }
                | CommerceEvent::CustomerStatusChanged { .. }
                | CommerceEvent::CustomerAddressAdded { .. }
        )
    }

    /// Filter for product events only
    pub fn products_only(event: &CommerceEvent) -> bool {
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
    pub fn returns_only(event: &CommerceEvent) -> bool {
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
    pub fn low_stock_alerts(event: &CommerceEvent) -> bool {
        matches!(event, CommerceEvent::LowStockAlert { .. })
    }

    /// Create a filter for events matching specific types
    pub fn event_types(types: &'static [&'static str]) -> impl Fn(&CommerceEvent) -> bool {
        move |event| types.contains(&event.event_type())
    }
}
