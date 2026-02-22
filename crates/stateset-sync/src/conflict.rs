use serde::{Deserialize, Serialize};

use crate::event::SyncEvent;

/// Strategy for resolving conflicts between local and remote events.
///
/// Maps to the JS `ResolutionStrategy` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ConflictStrategy {
    /// Accept the remote event, discard local.
    RemoteWins,
    /// Keep the local event, ignore remote.
    LocalWins,
    /// Compare timestamps and keep the most recent.
    LastWriterWins,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        Self::RemoteWins
    }
}

/// The outcome of resolving a conflict between two events.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Resolution {
    /// Keep the local event.
    KeepLocal,
    /// Keep the remote event.
    KeepRemote,
    /// Merge both events into a new event.
    Merge(SyncEvent),
}

/// Resolves conflicts between a local and a remote `SyncEvent`.
///
/// This is a pure, stateless resolver. The JS `ConflictResolver` is
/// SQLite-backed and more complex; this Rust version provides the core
/// resolution logic used by `SyncEngine`.
///
/// # Examples
///
/// ```
/// use stateset_sync::{ConflictResolver, ConflictStrategy, SyncEvent, Resolution};
/// use serde_json::json;
///
/// let resolver = ConflictResolver::new(ConflictStrategy::RemoteWins);
/// let local = SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "shipped"}));
/// let remote = SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "cancelled"}));
///
/// let resolution = resolver.resolve(&local, &remote);
/// assert!(matches!(resolution, Resolution::KeepRemote));
/// ```
#[derive(Debug, Clone)]
pub struct ConflictResolver {
    strategy: ConflictStrategy,
}

impl ConflictResolver {
    /// Create a new resolver with the given strategy.
    #[must_use]
    pub const fn new(strategy: ConflictStrategy) -> Self {
        Self { strategy }
    }

    /// Return the configured strategy.
    #[must_use]
    pub const fn strategy(&self) -> ConflictStrategy {
        self.strategy
    }

    /// Resolve a conflict between a local event and a remote event.
    #[must_use]
    pub fn resolve(&self, local: &SyncEvent, remote: &SyncEvent) -> Resolution {
        match self.strategy {
            ConflictStrategy::RemoteWins => Resolution::KeepRemote,
            ConflictStrategy::LocalWins => Resolution::KeepLocal,
            ConflictStrategy::LastWriterWins => {
                if local.timestamp >= remote.timestamp {
                    Resolution::KeepLocal
                } else {
                    Resolution::KeepRemote
                }
            }
        }
    }

    /// Resolve a batch of conflicts, returning one resolution per pair.
    ///
    /// Each tuple is `(local, remote)`.
    #[must_use]
    pub fn resolve_batch(&self, pairs: &[(&SyncEvent, &SyncEvent)]) -> Vec<Resolution> {
        pairs.iter().map(|(l, r)| self.resolve(l, r)).collect()
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new(ConflictStrategy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use serde_json::json;
    use uuid::Uuid;

    fn make_event_at(name: &str, ts_offset_secs: i64) -> SyncEvent {
        let ts = Utc::now() + Duration::seconds(ts_offset_secs);
        SyncEvent::with_id(
            Uuid::new_v4(),
            0,
            name,
            "order",
            "ORD-1",
            json!({"action": name}),
            ts,
        )
    }

    #[test]
    fn remote_wins_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::RemoteWins);
        let local = make_event_at("local", 0);
        let remote = make_event_at("remote", 0);
        assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepRemote);
    }

    #[test]
    fn local_wins_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::LocalWins);
        let local = make_event_at("local", 0);
        let remote = make_event_at("remote", 0);
        assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);
    }

    #[test]
    fn last_writer_wins_local_newer() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let local = make_event_at("local", 10); // 10 seconds in the future
        let remote = make_event_at("remote", -10); // 10 seconds in the past
        assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);
    }

    #[test]
    fn last_writer_wins_remote_newer() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let local = make_event_at("local", -10);
        let remote = make_event_at("remote", 10);
        assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepRemote);
    }

    #[test]
    fn last_writer_wins_equal_timestamps_keeps_local() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let ts = Utc::now();
        let local = SyncEvent::with_id(Uuid::new_v4(), 1, "local", "o", "1", json!({}), ts);
        let remote = SyncEvent::with_id(Uuid::new_v4(), 2, "remote", "o", "1", json!({}), ts);
        // local.timestamp >= remote.timestamp, so KeepLocal
        assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);
    }

    #[test]
    fn default_strategy_is_remote_wins() {
        let resolver = ConflictResolver::default();
        assert_eq!(resolver.strategy(), ConflictStrategy::RemoteWins);
    }

    #[test]
    fn resolve_batch() {
        let resolver = ConflictResolver::new(ConflictStrategy::RemoteWins);
        let l1 = make_event_at("l1", 0);
        let r1 = make_event_at("r1", 0);
        let l2 = make_event_at("l2", 0);
        let r2 = make_event_at("r2", 0);

        let pairs = vec![(&l1, &r1), (&l2, &r2)];
        let resolutions = resolver.resolve_batch(&pairs);
        assert_eq!(resolutions.len(), 2);
        assert!(resolutions.iter().all(|r| *r == Resolution::KeepRemote));
    }

    #[test]
    fn conflict_strategy_serde_roundtrip() {
        let strategy = ConflictStrategy::LastWriterWins;
        let json = serde_json::to_string(&strategy).unwrap();
        let deserialized: ConflictStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, strategy);
    }

    #[test]
    fn conflict_strategy_debug() {
        let strategy = ConflictStrategy::LocalWins;
        let debug = format!("{strategy:?}");
        assert!(debug.contains("LocalWins"));
    }

    #[test]
    fn resolver_clone() {
        let resolver = ConflictResolver::new(ConflictStrategy::LocalWins);
        let cloned = resolver.clone();
        assert_eq!(cloned.strategy(), ConflictStrategy::LocalWins);
    }

    #[test]
    fn resolution_debug() {
        let resolution = Resolution::KeepLocal;
        let debug = format!("{resolution:?}");
        assert!(debug.contains("KeepLocal"));
    }

    #[test]
    fn resolution_clone_eq() {
        let r1 = Resolution::KeepRemote;
        let r2 = r1.clone();
        assert_eq!(r1, r2);
    }

    #[test]
    fn resolution_merge_variant() {
        let event = make_event_at("merged", 0);
        let resolution = Resolution::Merge(event.clone());
        if let Resolution::Merge(merged) = resolution {
            assert_eq!(merged.event_type, "merged");
        } else {
            panic!("Expected Merge variant");
        }
    }
}
