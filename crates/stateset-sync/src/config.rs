use serde::{Deserialize, Serialize};

/// Default buffer capacity for the event buffer.
const DEFAULT_BUFFER_CAPACITY: usize = 1000;

/// Default batch size for push/pull operations.
const DEFAULT_BATCH_SIZE: usize = 100;

/// Configuration for the sync engine.
///
/// Maps to the JS `SyncConfig` (`agent_id`, `tenant_id`, `store_id`) plus
/// tuning knobs for buffer capacity and batch sizes.
///
/// # Examples
///
/// ```
/// use stateset_sync::SyncConfig;
///
/// let config = SyncConfig::new("agent-1", "tenant-1", "store-1");
/// assert_eq!(config.agent_id, "agent-1");
/// assert_eq!(config.buffer_capacity, 1000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Unique identifier for this agent.
    pub agent_id: String,
    /// Tenant identifier for multi-tenancy.
    pub tenant_id: String,
    /// Store identifier within the tenant.
    pub store_id: String,
    /// Maximum number of events the in-memory buffer can hold.
    pub buffer_capacity: usize,
    /// Maximum events per push/pull batch.
    pub batch_size: usize,
}

impl SyncConfig {
    /// Create a new `SyncConfig` with sensible defaults.
    #[must_use]
    pub fn new(
        agent_id: impl Into<String>,
        tenant_id: impl Into<String>,
        store_id: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            tenant_id: tenant_id.into(),
            store_id: store_id.into(),
            buffer_capacity: DEFAULT_BUFFER_CAPACITY,
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// Set the buffer capacity.
    #[must_use]
    pub const fn with_buffer_capacity(mut self, capacity: usize) -> Self {
        self.buffer_capacity = capacity;
        self
    }

    /// Set the batch size.
    #[must_use]
    pub const fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_config_defaults() {
        let config = SyncConfig::new("a", "t", "s");
        assert_eq!(config.agent_id, "a");
        assert_eq!(config.tenant_id, "t");
        assert_eq!(config.store_id, "s");
        assert_eq!(config.buffer_capacity, DEFAULT_BUFFER_CAPACITY);
        assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn config_builder_pattern() {
        let config = SyncConfig::new("a", "t", "s").with_buffer_capacity(500).with_batch_size(50);
        assert_eq!(config.buffer_capacity, 500);
        assert_eq!(config.batch_size, 50);
    }

    #[test]
    fn config_serde_roundtrip() {
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1");
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SyncConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_id, config.agent_id);
        assert_eq!(deserialized.tenant_id, config.tenant_id);
        assert_eq!(deserialized.store_id, config.store_id);
        assert_eq!(deserialized.buffer_capacity, config.buffer_capacity);
        assert_eq!(deserialized.batch_size, config.batch_size);
    }

    #[test]
    fn config_clone() {
        let config = SyncConfig::new("a", "t", "s");
        let cloned = config.clone();
        assert_eq!(cloned.agent_id, config.agent_id);
    }

    #[test]
    fn config_debug() {
        let config = SyncConfig::new("a", "t", "s");
        let debug = format!("{config:?}");
        assert!(debug.contains("SyncConfig"));
        assert!(debug.contains("agent_id"));
    }
}
