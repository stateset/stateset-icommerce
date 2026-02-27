use serde::{Deserialize, Serialize};

/// Default buffer capacity for the event buffer.
const DEFAULT_BUFFER_CAPACITY: usize = 1000;

/// Default batch size for push/pull operations.
const DEFAULT_BATCH_SIZE: usize = 100;
/// Default outbox capacity for pending local events.
const DEFAULT_OUTBOX_CAPACITY: usize = 10_000;

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
    /// Maximum pending local events in the outbox.
    pub outbox_capacity: usize,
    /// Optional durable outbox snapshot path.
    pub outbox_path: Option<String>,
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
            outbox_capacity: DEFAULT_OUTBOX_CAPACITY,
            outbox_path: None,
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

    /// Set the outbox capacity.
    #[must_use]
    pub const fn with_outbox_capacity(mut self, capacity: usize) -> Self {
        self.outbox_capacity = capacity;
        self
    }

    /// Set the durable outbox path.
    #[must_use]
    pub fn with_outbox_path(mut self, path: impl Into<String>) -> Self {
        self.outbox_path = Some(path.into());
        self
    }

    /// Resolve a valid buffer capacity.
    #[must_use]
    pub fn resolved_buffer_capacity(&self) -> usize {
        self.buffer_capacity.max(1)
    }

    /// Resolve a valid batch size.
    #[must_use]
    pub fn resolved_batch_size(&self) -> usize {
        self.batch_size.max(1)
    }

    /// Resolve a valid outbox capacity.
    #[must_use]
    pub fn resolved_outbox_capacity(&self) -> usize {
        self.outbox_capacity.max(1)
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns [`crate::SyncError::InvalidConfig`] when required fields are invalid.
    pub fn validate(&self) -> Result<(), crate::SyncError> {
        if self.agent_id.trim().is_empty() {
            return Err(crate::SyncError::InvalidConfig("agent_id must not be empty".into()));
        }
        if self.tenant_id.trim().is_empty() {
            return Err(crate::SyncError::InvalidConfig("tenant_id must not be empty".into()));
        }
        if self.store_id.trim().is_empty() {
            return Err(crate::SyncError::InvalidConfig("store_id must not be empty".into()));
        }
        if self.buffer_capacity == 0 {
            return Err(crate::SyncError::InvalidConfig(
                "buffer_capacity must be greater than 0".into(),
            ));
        }
        if self.batch_size == 0 {
            return Err(crate::SyncError::InvalidConfig(
                "batch_size must be greater than 0".into(),
            ));
        }
        if self.outbox_capacity == 0 {
            return Err(crate::SyncError::InvalidConfig(
                "outbox_capacity must be greater than 0".into(),
            ));
        }
        if self.outbox_path.as_ref().is_some_and(|path| path.trim().is_empty()) {
            return Err(crate::SyncError::InvalidConfig(
                "outbox_path must not be empty when provided".into(),
            ));
        }
        Ok(())
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
        assert_eq!(config.outbox_capacity, DEFAULT_OUTBOX_CAPACITY);
        assert!(config.outbox_path.is_none());
    }

    #[test]
    fn config_builder_pattern() {
        let config = SyncConfig::new("a", "t", "s")
            .with_buffer_capacity(500)
            .with_batch_size(50)
            .with_outbox_capacity(900)
            .with_outbox_path("/tmp/sync-outbox.json");
        assert_eq!(config.buffer_capacity, 500);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.outbox_capacity, 900);
        assert_eq!(config.outbox_path.as_deref(), Some("/tmp/sync-outbox.json"));
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
        assert_eq!(deserialized.outbox_capacity, config.outbox_capacity);
        assert_eq!(deserialized.outbox_path, config.outbox_path);
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

    #[test]
    fn validate_rejects_empty_ids_and_zero_caps() {
        let bad = SyncConfig::new("", "tenant", "store");
        assert!(bad.validate().is_err());

        let bad = SyncConfig::new("agent", "", "store");
        assert!(bad.validate().is_err());

        let bad = SyncConfig::new("agent", "tenant", "")
            .with_batch_size(0)
            .with_buffer_capacity(0)
            .with_outbox_capacity(0);
        assert!(bad.validate().is_err());
    }

    #[test]
    fn validate_accepts_good_config() {
        let ok = SyncConfig::new("agent", "tenant", "store")
            .with_buffer_capacity(100)
            .with_batch_size(10)
            .with_outbox_capacity(1000)
            .with_outbox_path("/tmp/outbox.json");
        assert!(ok.validate().is_ok());
    }
}
