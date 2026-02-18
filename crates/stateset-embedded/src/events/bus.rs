//! Event bus for in-process pub/sub using tokio broadcast channels

use futures::future::FutureExt;
use stateset_core::CommerceEvent;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use tokio::sync::broadcast;

/// Event bus for broadcasting events to multiple subscribers
pub struct EventBus {
    sender: broadcast::Sender<CommerceEvent>,
    events_published: AtomicU64,
    events_publish_failures: AtomicU64,
}

impl EventBus {
    /// Create a new event bus with the specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            events_published: AtomicU64::new(0),
            events_publish_failures: AtomicU64::new(0),
        }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: CommerceEvent) -> usize {
        self.events_published.fetch_add(1, Ordering::Relaxed);
        // Returns number of receivers that received the message
        // If no active receivers (for example before any subscribe), this is surfaced as a debug log.
        let event_type = event.event_type().to_string();
        let receiver_count = self.sender.receiver_count();
        match self.sender.send(event) {
            Ok(receivers) => receivers,
            Err(error) => {
                self.events_publish_failures.fetch_add(1, Ordering::Relaxed);
                if receiver_count == 0 {
                    tracing::debug!(
                        event_type,
                        error = %error,
                        receiver_count,
                        "Dropped event publish: no active subscribers"
                    );
                } else {
                    tracing::warn!(
                        event_type,
                        error = %error,
                        receiver_count,
                        "Failed to publish event to in-process subscribers"
                    );
                }
                0
            }
        }
    }

    /// Subscribe to events from this bus
    pub fn subscribe(&self) -> EventSubscription {
        EventSubscription { receiver: EventReceiver::new(self.sender.subscribe()) }
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get total number of events published
    pub fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    /// Get the number of events that failed to publish to the in-process bus
    pub fn events_publish_failures(&self) -> u64 {
        self.events_publish_failures.load(Ordering::Relaxed)
    }
}

/// Wrapper around broadcast receiver with convenience methods
pub struct EventReceiver {
    inner: broadcast::Receiver<CommerceEvent>,
}

impl EventReceiver {
    fn new(receiver: broadcast::Receiver<CommerceEvent>) -> Self {
        Self { inner: receiver }
    }

    /// Receive the next event, waiting if necessary
    pub async fn recv(&mut self) -> Option<CommerceEvent> {
        loop {
            match self.inner.recv().await {
                Ok(event) => return Some(event),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    // Log that we skipped some events due to slow consumer
                    tracing::warn!(skipped, "Event receiver lagged, skipped events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    /// Try to receive an event without waiting
    pub fn try_recv(&mut self) -> Option<CommerceEvent> {
        self.inner.try_recv().ok()
    }

    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<CommerceEvent>> {
        loop {
            let mut recv = std::pin::pin!(self.inner.recv());
            match std::future::Future::poll(recv.as_mut(), cx) {
                Poll::Ready(Ok(event)) => return Poll::Ready(Some(event)),
                Poll::Ready(Err(broadcast::error::RecvError::Lagged(skipped))) => {
                    tracing::warn!(skipped, "Event receiver lagged, skipped events");
                    continue;
                }
                Poll::Ready(Err(broadcast::error::RecvError::Closed)) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// An event subscription that can be used to receive events
pub struct EventSubscription {
    receiver: EventReceiver,
}

impl EventSubscription {
    /// Receive the next event
    pub async fn recv(&mut self) -> Option<CommerceEvent> {
        self.receiver.recv().await
    }

    /// Try to receive without waiting
    pub fn try_recv(&mut self) -> Option<CommerceEvent> {
        self.receiver.try_recv()
    }
}

// Implement Stream trait for use with StreamExt
impl futures::Stream for EventSubscription {
    type Item = CommerceEvent;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new(16);

        let mut sub1 = bus.subscribe();
        let mut sub2 = bus.subscribe();

        let event = CommerceEvent::OrderCreated {
            order_id: stateset_core::OrderId::new(),
            customer_id: stateset_core::CustomerId::new(),
            total_amount: dec!(100.00),
            item_count: 2,
            timestamp: Utc::now(),
        };

        // Publish should reach both subscribers
        let receivers = bus.publish(event.clone());
        assert_eq!(receivers, 2);

        // Both subscribers should receive the event
        let received1 = sub1.try_recv();
        let received2 = sub2.try_recv();

        assert!(received1.is_some());
        assert!(received2.is_some());
    }

    #[tokio::test]
    async fn test_event_bus_no_subscribers() {
        let bus = EventBus::new(16);

        let event = CommerceEvent::CustomerCreated {
            customer_id: stateset_core::CustomerId::new(),
            email: "test@example.com".to_string(),
            timestamp: Utc::now(),
        };

        // Should not panic even with no subscribers
        let receivers = bus.publish(event);
        assert_eq!(receivers, 0);
    }

    #[test]
    fn test_receiver_count() {
        let bus = EventBus::new(16);
        assert_eq!(bus.receiver_count(), 0);

        let _sub1 = bus.subscribe();
        assert_eq!(bus.receiver_count(), 1);

        let _sub2 = bus.subscribe();
        assert_eq!(bus.receiver_count(), 2);
    }

    #[test]
    fn test_event_bus_publish_failure_tracking() {
        let bus = EventBus::new(16);

        let event = CommerceEvent::CustomerCreated {
            customer_id: stateset_core::CustomerId::new(),
            email: "test@example.com".to_string(),
            timestamp: Utc::now(),
        };

        let receivers = bus.publish(event);
        assert_eq!(receivers, 0);
        assert_eq!(bus.events_published(), 1);
        assert_eq!(bus.events_publish_failures(), 1);
    }

    #[tokio::test]
    async fn test_event_subscription_stream_next_wakes_on_publish() {
        use futures::StreamExt;

        let bus = EventBus::new(16);
        let mut sub = bus.subscribe();
        let event = CommerceEvent::OrderCreated {
            order_id: stateset_core::OrderId::new(),
            customer_id: stateset_core::CustomerId::new(),
            total_amount: dec!(100.00),
            item_count: 2,
            timestamp: Utc::now(),
        };

        let bus_for_publish = bus;
        let publish_task = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            bus_for_publish.publish(event.clone());
        });

        let got = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            sub.next(),
        )
        .await
        .expect("timed out while waiting for published event");

        publish_task.await.expect("publisher task failed");
        assert!(got.is_some());
    }
}
