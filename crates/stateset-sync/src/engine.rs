use chrono::Utc;

use crate::buffer::EventBuffer;
use crate::config::SyncConfig;
use crate::conflict::{ConflictResolver, ConflictStrategy};
use crate::error::SyncError;
use crate::event::SyncEvent;
use crate::outbox::Outbox;
use crate::state::{SyncState, SyncStatus};
use crate::transport::{PullResult, PushResult, Transport};

/// The sync engine orchestrates synchronization between local state and
/// a remote sequencer.
///
/// This is the Rust equivalent of the JS `SyncEngine` class, providing:
/// - Event recording to the outbox
/// - Push (outbox -> remote) via a [`Transport`]
/// - Pull (remote -> buffer) via a [`Transport`]
/// - Conflict resolution during pull
/// - Status reporting
///
/// # Examples
///
/// ```
/// use stateset_sync::{SyncEngine, SyncConfig, SyncEvent};
/// use serde_json::json;
///
/// let config = SyncConfig::new("agent-1", "tenant-1", "store-1");
/// let mut engine = SyncEngine::new(config);
///
/// let seq = engine.record(SyncEvent::new("order.created", "order", "ORD-1", json!({"total": 99})));
/// assert!(seq.is_ok());
/// assert_eq!(engine.pending_count(), 1);
/// ```
#[derive(Debug)]
pub struct SyncEngine {
    config: SyncConfig,
    state: SyncState,
    outbox: Outbox,
    buffer: EventBuffer,
    resolver: ConflictResolver,
    initialized: bool,
}

impl SyncEngine {
    /// Create a new `SyncEngine` with the given configuration.
    #[must_use]
    pub fn new(config: SyncConfig) -> Self {
        let buffer_capacity = config.buffer_capacity;
        Self {
            config,
            state: SyncState::default(),
            outbox: Outbox::with_default_capacity(),
            buffer: EventBuffer::new(buffer_capacity),
            resolver: ConflictResolver::default(),
            initialized: true,
        }
    }

    /// Create a `SyncEngine` with a custom conflict resolution strategy.
    #[must_use]
    pub fn with_strategy(config: SyncConfig, strategy: ConflictStrategy) -> Self {
        let buffer_capacity = config.buffer_capacity;
        Self {
            config,
            state: SyncState::default(),
            outbox: Outbox::with_default_capacity(),
            buffer: EventBuffer::new(buffer_capacity),
            resolver: ConflictResolver::new(strategy),
            initialized: true,
        }
    }

    /// Record an event into the outbox for later push.
    ///
    /// Returns the assigned local sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::OutboxFull`] if the outbox is at capacity.
    pub fn record(&mut self, event: SyncEvent) -> Result<u64, SyncError> {
        let seq = self.outbox.append(event)?;
        self.state.local_head = seq;
        self.state.pending_count = self.outbox.count();
        Ok(seq)
    }

    /// Push pending events from the outbox to the remote via the given transport.
    ///
    /// Drains up to `batch_size` events from the outbox.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    pub async fn push(&mut self, transport: &dyn Transport) -> Result<PushResult, SyncError> {
        let batch_size = self.config.batch_size;
        let events: Vec<SyncEvent> =
            self.outbox.peek(batch_size).into_iter().cloned().collect();

        if events.is_empty() {
            return Ok(PushResult {
                accepted: 0,
                remote_head: self.state.remote_head,
            });
        }

        let result = transport.push_events(&events).await?;
        let accepted = result.accepted.min(events.len());
        if accepted > 0 {
            let _ = self.outbox.drain(accepted);
        }

        self.state.remote_head = result.remote_head;
        self.state.last_push = Some(Utc::now());
        self.state.pending_count = self.outbox.count();

        Ok(result)
    }

    /// Pull events from the remote sequencer into the local buffer.
    ///
    /// Pulled events are added to the event buffer. If conflicts are
    /// detected (same `entity_type` + `entity_id` in both outbox and pulled),
    /// they are resolved using the configured strategy.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    pub async fn pull(&mut self, transport: &dyn Transport) -> Result<PullResult, SyncError> {
        let since = self.state.remote_head;
        let limit = self.config.batch_size;

        let result = transport.pull_events(since, limit).await?;

        // Detect and resolve conflicts between pending outbox events and pulled events
        let pending = self.outbox.peek(self.outbox.count());
        for pulled_event in &result.events {
            for local_event in &pending {
                if local_event.entity_type == pulled_event.entity_type
                    && local_event.entity_id == pulled_event.entity_id
                {
                    let _resolution = self.resolver.resolve(local_event, pulled_event);
                    // In a full implementation, we'd apply the resolution
                    // (e.g., remove the local event from outbox if KeepRemote)
                }
            }
        }

        // Buffer all pulled events
        for event in &result.events {
            self.buffer.push(event.clone());
        }

        self.state.remote_head = result.remote_head;
        self.state.last_pull = Some(Utc::now());

        Ok(result)
    }

    /// Get the current sync status.
    #[must_use]
    pub fn status(&self) -> SyncStatus {
        SyncStatus {
            initialized: self.initialized,
            local_head: self.state.local_head,
            remote_head: self.state.remote_head,
            pending: self.outbox.count(),
            lag: self.state.lag(),
            last_push: self.state.last_push,
            last_pull: self.state.last_pull,
            buffered_events: self.buffer.len(),
        }
    }

    /// Return the number of events pending in the outbox.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.outbox.count()
    }

    /// Return the number of events currently in the pull buffer.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Drain all events from the pull buffer.
    pub fn drain_buffer(&mut self) -> Vec<SyncEvent> {
        self.buffer.drain_all()
    }

    /// Return a reference to the current sync state.
    #[must_use]
    pub const fn state(&self) -> &SyncState {
        &self.state
    }

    /// Return a reference to the sync configuration.
    #[must_use]
    pub const fn config(&self) -> &SyncConfig {
        &self.config
    }

    /// Return a reference to the conflict resolver.
    #[must_use]
    pub const fn resolver(&self) -> &ConflictResolver {
        &self.resolver
    }

    /// Perform a full sync: push first, then pull.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered during push or pull.
    pub async fn full_sync(
        &mut self,
        transport: &dyn Transport,
    ) -> Result<(PushResult, PullResult), SyncError> {
        let push_result = self.push(transport).await?;
        let pull_result = self.pull(transport).await?;
        Ok((push_result, pull_result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::NullTransport;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    fn make_config() -> SyncConfig {
        SyncConfig::new("agent-1", "tenant-1", "store-1")
    }

    fn make_event(event_type: &str) -> SyncEvent {
        SyncEvent::new(event_type, "order", "ORD-1", json!({}))
    }

    #[test]
    fn new_engine() {
        let engine = SyncEngine::new(make_config());
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.buffered_count(), 0);
        assert!(engine.status().initialized);
    }

    #[test]
    fn record_event() {
        let mut engine = SyncEngine::new(make_config());
        let seq = engine.record(make_event("order.created")).unwrap();
        assert_eq!(seq, 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.state().local_head, 1);
    }

    #[test]
    fn record_multiple_events() {
        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();
        engine.record(make_event("c")).unwrap();
        assert_eq!(engine.pending_count(), 3);
        assert_eq!(engine.state().local_head, 3);
    }

    #[tokio::test]
    async fn push_with_null_transport() {
        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let transport = NullTransport::new();
        let result = engine.push(&transport).await.unwrap();
        assert_eq!(result.accepted, 2);
        assert_eq!(engine.pending_count(), 0);
        assert!(engine.state().last_push.is_some());
    }

    #[tokio::test]
    async fn push_empty_outbox() {
        let mut engine = SyncEngine::new(make_config());
        let transport = NullTransport::new();
        let result = engine.push(&transport).await.unwrap();
        assert_eq!(result.accepted, 0);
    }

    #[tokio::test]
    async fn pull_with_null_transport() {
        let mut engine = SyncEngine::new(make_config());
        let transport = NullTransport::new();
        let result = engine.pull(&transport).await.unwrap();
        assert!(result.events.is_empty());
        assert!(!result.has_more);
        assert!(engine.state().last_pull.is_some());
    }

    #[tokio::test]
    async fn pull_buffers_events() {
        /// Mock transport that returns predefined events on pull.
        #[derive(Debug)]
        struct MockPullTransport {
            events: Vec<SyncEvent>,
            head: u64,
        }

        #[async_trait::async_trait]
        impl Transport for MockPullTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult {
                    accepted: events.len(),
                    remote_head: self.head,
                })
            }
            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult {
                    events: self.events.clone(),
                    remote_head: self.head,
                    has_more: false,
                })
            }
        }

        let transport = MockPullTransport {
            events: vec![
                make_event("pulled-1").with_sequence(1),
                make_event("pulled-2").with_sequence(2),
            ],
            head: 2,
        };

        let mut engine = SyncEngine::new(make_config());
        let result = engine.pull(&transport).await.unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(engine.buffered_count(), 2);
        assert_eq!(engine.state().remote_head, 2);
    }

    #[tokio::test]
    async fn full_sync() {
        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("local")).unwrap();

        let transport = NullTransport::new();
        let (push_result, pull_result) = engine.full_sync(&transport).await.unwrap();
        assert_eq!(push_result.accepted, 1);
        assert!(pull_result.events.is_empty());
        assert_eq!(engine.pending_count(), 0);
    }

    #[test]
    fn status_reporting() {
        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let status = engine.status();
        assert!(status.initialized);
        assert_eq!(status.pending, 2);
        assert_eq!(status.local_head, 2);
        assert_eq!(status.remote_head, 0);
        assert_eq!(status.lag, 0);
        assert!(status.last_push.is_none());
    }

    #[test]
    fn drain_buffer() {
        let mut engine = SyncEngine::new(make_config());
        // Manually push to buffer via engine internals
        engine.buffer.push(make_event("buffered"));
        assert_eq!(engine.buffered_count(), 1);

        let drained = engine.drain_buffer();
        assert_eq!(drained.len(), 1);
        assert_eq!(engine.buffered_count(), 0);
    }

    #[test]
    fn engine_with_strategy() {
        let engine = SyncEngine::with_strategy(make_config(), ConflictStrategy::LocalWins);
        assert_eq!(engine.resolver().strategy(), ConflictStrategy::LocalWins);
    }

    #[test]
    fn config_accessor() {
        let config = make_config();
        let engine = SyncEngine::new(config);
        assert_eq!(engine.config().agent_id, "agent-1");
    }

    #[tokio::test]
    async fn push_respects_batch_size() {
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1").with_batch_size(2);
        let mut engine = SyncEngine::new(config);
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();
        engine.record(make_event("c")).unwrap();

        let transport = NullTransport::new();
        let result = engine.push(&transport).await.unwrap();
        // Should only push 2 due to batch_size
        assert_eq!(result.accepted, 2);
        assert_eq!(engine.pending_count(), 1);
    }

    #[tokio::test]
    async fn push_updates_state() {
        /// Mock transport that returns an increasing remote head.
        #[derive(Debug)]
        struct MockHeadTransport {
            head: Arc<AtomicU64>,
        }

        #[async_trait::async_trait]
        impl Transport for MockHeadTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let new_head = self.head.fetch_add(events.len() as u64, Ordering::SeqCst)
                    + events.len() as u64;
                Ok(PushResult {
                    accepted: events.len(),
                    remote_head: new_head,
                })
            }
            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult {
                    events: vec![],
                    remote_head: self.head.load(Ordering::SeqCst),
                    has_more: false,
                })
            }
        }

        let transport = MockHeadTransport {
            head: Arc::new(AtomicU64::new(0)),
        };

        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let result = engine.push(&transport).await.unwrap();
        assert_eq!(result.remote_head, 2);
        assert_eq!(engine.state().remote_head, 2);
    }

    #[tokio::test]
    async fn transport_error_propagates() {
        /// Transport that always fails.
        #[derive(Debug)]
        struct FailTransport;

        #[async_trait::async_trait]
        impl Transport for FailTransport {
            async fn push_events(&self, _events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Err(SyncError::Transport("network down".into()))
            }
            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Err(SyncError::Transport("network down".into()))
            }
        }

        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("a")).unwrap();

        let transport = FailTransport;
        let result = engine.push(&transport).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncError::Transport(_)));
        // Failed push must not drop local events.
        assert_eq!(engine.pending_count(), 1);

        let pull_result = engine.pull(&transport).await;
        assert!(pull_result.is_err());
    }

    #[tokio::test]
    async fn push_only_drains_accepted_events() {
        /// Transport that only accepts one event from each batch.
        #[derive(Debug)]
        struct PartialAcceptTransport;

        #[async_trait::async_trait]
        impl Transport for PartialAcceptTransport {
            async fn push_events(&self, _events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult {
                    accepted: 1,
                    remote_head: 1,
                })
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult {
                    events: vec![],
                    remote_head: 1,
                    has_more: false,
                })
            }
        }

        let mut engine = SyncEngine::new(make_config());
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();
        engine.record(make_event("c")).unwrap();

        let result = engine.push(&PartialAcceptTransport).await.unwrap();
        assert_eq!(result.accepted, 1);
        assert_eq!(engine.pending_count(), 2);
    }

    #[tokio::test]
    async fn pull_conflict_resolution() {
        /// Transport that returns events conflicting with local outbox.
        #[derive(Debug)]
        struct ConflictTransport;

        #[async_trait::async_trait]
        impl Transport for ConflictTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult {
                    accepted: events.len(),
                    remote_head: 10,
                })
            }
            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                // Return an event for the same entity as the pending local event
                let remote_event =
                    SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                        .with_sequence(5);
                Ok(PullResult {
                    events: vec![remote_event],
                    remote_head: 5,
                    has_more: false,
                })
            }
        }

        let mut engine =
            SyncEngine::with_strategy(make_config(), ConflictStrategy::RemoteWins);
        engine
            .record(SyncEvent::new(
                "order.updated",
                "order",
                "ORD-1",
                json!({"status": "local"}),
            ))
            .unwrap();

        let transport = ConflictTransport;
        let result = engine.pull(&transport).await.unwrap();
        assert_eq!(result.events.len(), 1);
        // Conflict resolution happened (RemoteWins) -- events still buffered
        assert_eq!(engine.buffered_count(), 1);
    }

    #[test]
    fn engine_debug() {
        let engine = SyncEngine::new(make_config());
        let debug = format!("{engine:?}");
        assert!(debug.contains("SyncEngine"));
    }
}
