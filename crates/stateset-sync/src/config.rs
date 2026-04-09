use serde::{Deserialize, Serialize};

/// Default buffer capacity for the event buffer.
const DEFAULT_BUFFER_CAPACITY: usize = 1000;

/// Default batch size for push/pull operations.
const DEFAULT_BATCH_SIZE: usize = 100;
/// Default outbox capacity for pending local events.
const DEFAULT_OUTBOX_CAPACITY: usize = 10_000;
/// Default number of retained push confirmations persisted in sync state.
const DEFAULT_CONFIRMATION_CAPACITY: usize = 1000;

const fn default_confirmation_capacity() -> usize {
    DEFAULT_CONFIRMATION_CAPACITY
}

/// Policy controlling which remote commitment manifests are accepted automatically.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitmentTrustPolicy {
    /// Whether remote commitment metadata must include a signed manifest.
    #[serde(default)]
    pub require_manifest: bool,
    /// Optional allowlist of signer ids that may publish trusted manifests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_signer_ids: Vec<String>,
    /// Optional allowlist of signer public keys that may publish trusted manifests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_signer_public_keys: Vec<String>,
}

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
    /// Optional durable sync-state snapshot path.
    pub state_path: Option<String>,
    /// Maximum number of sequencer push confirmations to retain durably.
    #[serde(default = "default_confirmation_capacity")]
    pub confirmation_capacity: usize,
    /// Trust policy used when importing signed remote commitment manifests.
    #[serde(default)]
    pub commitment_trust: CommitmentTrustPolicy,
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
            state_path: None,
            confirmation_capacity: DEFAULT_CONFIRMATION_CAPACITY,
            commitment_trust: CommitmentTrustPolicy::default(),
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

    /// Set the durable sync-state snapshot path.
    #[must_use]
    pub fn with_state_path(mut self, path: impl Into<String>) -> Self {
        self.state_path = Some(path.into());
        self
    }

    /// Set the retained confirmation capacity.
    #[must_use]
    pub const fn with_confirmation_capacity(mut self, capacity: usize) -> Self {
        self.confirmation_capacity = capacity;
        self
    }

    /// Require remote heads with commitment metadata to include a signed manifest.
    #[must_use]
    pub const fn with_require_commitment_manifest(mut self, require_manifest: bool) -> Self {
        self.commitment_trust.require_manifest = require_manifest;
        self
    }

    /// Allow a specific signer id to publish trusted commitment manifests.
    #[must_use]
    pub fn with_trusted_commitment_signer(mut self, signer_id: impl Into<String>) -> Self {
        self.commitment_trust.trusted_signer_ids.push(signer_id.into());
        self
    }

    /// Allow a specific signer public key to publish trusted commitment manifests.
    #[must_use]
    pub fn with_trusted_commitment_signer_public_key(
        mut self,
        signer_public_key: impl Into<String>,
    ) -> Self {
        self.commitment_trust.trusted_signer_public_keys.push(signer_public_key.into());
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

    /// Resolve a valid confirmation retention capacity.
    #[must_use]
    pub fn resolved_confirmation_capacity(&self) -> usize {
        self.confirmation_capacity.max(1)
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
        if self.confirmation_capacity == 0 {
            return Err(crate::SyncError::InvalidConfig(
                "confirmation_capacity must be greater than 0".into(),
            ));
        }
        if self.outbox_path.as_ref().is_some_and(|path| path.trim().is_empty()) {
            return Err(crate::SyncError::InvalidConfig(
                "outbox_path must not be empty when provided".into(),
            ));
        }
        if self.state_path.as_ref().is_some_and(|path| path.trim().is_empty()) {
            return Err(crate::SyncError::InvalidConfig(
                "state_path must not be empty when provided".into(),
            ));
        }
        if self
            .commitment_trust
            .trusted_signer_ids
            .iter()
            .any(|signer_id| signer_id.trim().is_empty())
        {
            return Err(crate::SyncError::InvalidConfig(
                "trusted commitment signer ids must not be empty".into(),
            ));
        }
        if self
            .commitment_trust
            .trusted_signer_public_keys
            .iter()
            .any(|signer_public_key| signer_public_key.trim().is_empty())
        {
            return Err(crate::SyncError::InvalidConfig(
                "trusted commitment signer public keys must not be empty".into(),
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
        assert_eq!(config.confirmation_capacity, DEFAULT_CONFIRMATION_CAPACITY);
        assert!(config.outbox_path.is_none());
        assert!(config.state_path.is_none());
        assert!(!config.commitment_trust.require_manifest);
        assert!(config.commitment_trust.trusted_signer_ids.is_empty());
        assert!(config.commitment_trust.trusted_signer_public_keys.is_empty());
    }

    #[test]
    fn config_builder_pattern() {
        let config = SyncConfig::new("a", "t", "s")
            .with_buffer_capacity(500)
            .with_batch_size(50)
            .with_outbox_capacity(900)
            .with_outbox_path("/tmp/sync-outbox.json")
            .with_state_path("/tmp/sync-state.json")
            .with_confirmation_capacity(128)
            .with_require_commitment_manifest(true)
            .with_trusted_commitment_signer("sequencer-a")
            .with_trusted_commitment_signer_public_key("aa".repeat(32));
        assert_eq!(config.buffer_capacity, 500);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.outbox_capacity, 900);
        assert_eq!(config.confirmation_capacity, 128);
        assert_eq!(config.outbox_path.as_deref(), Some("/tmp/sync-outbox.json"));
        assert_eq!(config.state_path.as_deref(), Some("/tmp/sync-state.json"));
        assert!(config.commitment_trust.require_manifest);
        assert_eq!(config.commitment_trust.trusted_signer_ids, vec!["sequencer-a"]);
        assert_eq!(config.commitment_trust.trusted_signer_public_keys, vec!["aa".repeat(32)]);
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
        assert_eq!(deserialized.confirmation_capacity, config.confirmation_capacity);
        assert_eq!(deserialized.outbox_path, config.outbox_path);
        assert_eq!(deserialized.state_path, config.state_path);
        assert_eq!(deserialized.commitment_trust, config.commitment_trust);
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
            .with_outbox_capacity(0)
            .with_confirmation_capacity(0);
        assert!(bad.validate().is_err());
    }

    #[test]
    fn validate_accepts_good_config() {
        let ok = SyncConfig::new("agent", "tenant", "store")
            .with_buffer_capacity(100)
            .with_batch_size(10)
            .with_outbox_capacity(1000)
            .with_outbox_path("/tmp/outbox.json")
            .with_state_path("/tmp/state.json")
            .with_confirmation_capacity(64)
            .with_require_commitment_manifest(true)
            .with_trusted_commitment_signer("sequencer-a")
            .with_trusted_commitment_signer_public_key("bb".repeat(32));
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_trusted_signer_entries() {
        assert!(
            SyncConfig::new("agent", "tenant", "store")
                .with_trusted_commitment_signer(" ")
                .validate()
                .is_err()
        );
        assert!(
            SyncConfig::new("agent", "tenant", "store")
                .with_trusted_commitment_signer_public_key("")
                .validate()
                .is_err()
        );
    }
}
