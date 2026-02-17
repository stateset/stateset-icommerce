use super::Commerce;

#[cfg(feature = "events")]
use crate::events::{EventSubscription, EventSystem, Webhook, WebhookDelivery, WebhookRegistrationError};

impl Commerce {
    /// Access the event system for pub/sub and webhook management.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use futures::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let commerce = Commerce::new("./store.db")?;
    ///
    ///     // Subscribe to all events
    ///     let mut subscription = commerce.events().subscribe();
    ///
    ///     // Process events in background
    ///     tokio::spawn(async move {
    ///         while let Some(event) = subscription.next().await {
    ///             println!("Event: {:?}", event);
    ///         }
    ///     });
    ///
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "events")]
    pub fn events(&self) -> &EventSystem {
        &self.event_system
    }

    /// Subscribe to commerce events.
    ///
    /// This is a convenience method that returns an event subscription
    /// for receiving real-time commerce events.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use futures::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let commerce = Commerce::new("./store.db")?;
    ///
    ///     let mut subscription = commerce.subscribe_events();
    ///
    ///     while let Some(event) = subscription.next().await {
    ///         match event {
    ///             stateset_core::CommerceEvent::OrderCreated { order_id, .. } => {
    ///                 println!("New order: {}", order_id);
    ///             }
    ///             _ => {}
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "events")]
    pub fn subscribe_events(&self) -> EventSubscription {
        self.event_system.subscribe()
    }

    /// Register a webhook endpoint for event delivery.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, Webhook};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let webhook = Webhook::new(
    ///     "My Webhook",
    ///     "https://my-app.com/webhooks/stateset",
    /// ).with_secret("my-webhook-secret");
    ///
    /// if let Some(id) = commerce.try_register_webhook(webhook) {
    ///     println!("Webhook registered: {}", id);
    /// } else {
    ///     println!("Webhook registration failed");
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[cfg(feature = "events")]
    pub fn register_webhook(&self, webhook: Webhook) -> uuid::Uuid {
        self.event_system.register_webhook(webhook)
    }

    /// Register a webhook endpoint with explicit failure semantics.
    #[cfg(feature = "events")]
    pub fn register_webhook_strict(
        &self,
        webhook: Webhook,
    ) -> Result<uuid::Uuid, WebhookRegistrationError> {
        self.event_system.register_webhook_strict(webhook)
    }

    /// Register a webhook endpoint, returning `None` when registration fails validation.
    ///
    /// This is the safer API when you need to distinguish between a successful
    /// registration ID and a blocked/invalid webhook definition.
    #[cfg(feature = "events")]
    pub fn try_register_webhook(&self, webhook: Webhook) -> Option<uuid::Uuid> {
        self.event_system.try_register_webhook(webhook)
    }

    /// Unregister a webhook endpoint.
    #[cfg(feature = "events")]
    pub fn unregister_webhook(&self, id: uuid::Uuid) -> bool {
        self.event_system.unregister_webhook(id)
    }

    /// List all registered webhooks.
    #[cfg(feature = "events")]
    pub fn list_webhooks(&self) -> Vec<Webhook> {
        self.event_system.list_webhooks()
    }

    /// Get delivery history for a webhook (newest-first).
    #[cfg(feature = "events")]
    pub fn webhook_deliveries(&self, id: uuid::Uuid) -> Vec<WebhookDelivery> {
        self.event_system.webhook_deliveries(id)
    }

    /// Emit a commerce event manually.
    ///
    /// Events are typically emitted automatically by commerce operations,
    /// but you can also emit custom events using this method.
    #[cfg(feature = "events")]
    pub fn emit_event(&self, event: stateset_core::CommerceEvent) {
        self.event_system.emit(event);
    }
}
