//! Event bus for in-process pub/sub using tokio broadcast channels

use stateset_core::CommerceEvent;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

/// Event bus for broadcasting events to multiple subscribers
pub struct EventBus {
    sender: broadcast::Sender<CommerceEvent>,
    events_published: AtomicU64,
}

impl EventBus {
    /// Create a new event bus with the specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            events_published: AtomicU64::new(0),
        }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: CommerceEvent) -> usize {
        self.events_published.fetch_add(1, Ordering::Relaxed);
        // Returns number of receivers that received the message
        // If no receivers, this returns an error but we ignore it
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribe to events from this bus
    pub fn subscribe(&self) -> EventSubscription {
        EventSubscription {
            receiver: EventReceiver::new(self.sender.subscribe()),
        }
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get total number of events published
    pub fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
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
        match self.inner.try_recv() {
            Ok(event) => Some(event),
            Err(_) => None,
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
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::future::Future;
        use std::task::Poll;

        // Create a future for recv and poll it
        let recv_future = self.receiver.recv();
        tokio::pin!(recv_future);

        match recv_future.poll(cx) {
            Poll::Ready(result) => Poll::Ready(result),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new(16);

        let mut sub1 = bus.subscribe();
        let mut sub2 = bus.subscribe();

        let event = CommerceEvent::OrderCreated {
            order_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
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
            customer_id: Uuid::new_v4(),
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
}
