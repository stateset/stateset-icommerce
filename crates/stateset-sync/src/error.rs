use thiserror::Error;

/// Errors that can occur during sync operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SyncError {
    /// The outbox has reached its maximum capacity.
    #[error("outbox full: capacity {capacity}, current {current}")]
    OutboxFull {
        /// Maximum allowed events.
        capacity: usize,
        /// Current event count.
        current: usize,
    },

    /// The event buffer has reached its maximum capacity (informational; old events are evicted).
    #[error("buffer full: capacity {0}")]
    BufferFull(usize),

    /// A transport-level error occurred during push or pull.
    #[error("transport error: {0}")]
    Transport(String),

    /// A conflict was detected between local and remote events.
    #[error("conflict on entity {entity_type}/{entity_id}: {description}")]
    Conflict {
        /// The type of entity involved.
        entity_type: String,
        /// The identifier of the entity.
        entity_id: String,
        /// Human-readable conflict description.
        description: String,
    },

    /// The sync engine has not been initialized.
    #[error("sync engine not initialized")]
    NotInitialized,

    /// A serialization or deserialization error occurred.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// An invalid configuration value was provided.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// An event with a duplicate ID was detected.
    #[error("duplicate event id: {0}")]
    DuplicateEvent(String),

    /// The requested sequence number is out of range.
    #[error("sequence {requested} is out of range (head: {head})")]
    SequenceOutOfRange {
        /// The sequence that was requested.
        requested: u64,
        /// The current head sequence.
        head: u64,
    },
}

impl From<serde_json::Error> for SyncError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_outbox_full() {
        let err = SyncError::OutboxFull {
            capacity: 1000,
            current: 1000,
        };
        assert_eq!(err.to_string(), "outbox full: capacity 1000, current 1000");
    }

    #[test]
    fn error_display_transport() {
        let err = SyncError::Transport("connection refused".into());
        assert_eq!(err.to_string(), "transport error: connection refused");
    }

    #[test]
    fn error_display_conflict() {
        let err = SyncError::Conflict {
            entity_type: "order".into(),
            entity_id: "ORD-123".into(),
            description: "version mismatch".into(),
        };
        assert!(err.to_string().contains("order/ORD-123"));
    }

    #[test]
    fn error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let sync_err: SyncError = json_err.into();
        assert!(matches!(sync_err, SyncError::Serialization(_)));
    }

    #[test]
    fn error_display_sequence_out_of_range() {
        let err = SyncError::SequenceOutOfRange {
            requested: 500,
            head: 100,
        };
        assert!(err.to_string().contains("500"));
        assert!(err.to_string().contains("100"));
    }
}
