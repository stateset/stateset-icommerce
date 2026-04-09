use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::sync::{
    AttestationError, CommandAttestation, CommandConvergence, CommandInclusionProof,
    CommitmentManifest, DeadLetter, KernelExecutionError, KernelReceipt, KernelTransaction,
    ManifestVerificationError, PullResult, PushConfirmation, PushResult, RemoteHead,
    SequencerHttpTransport, SyncConfig, SyncEngine, SyncError, SyncEvent, SyncStatus,
    VerifiedCommitmentManifest,
};
use uuid::Uuid;

const DEFAULT_SYNC_RUNTIME_ENV_PREFIX: &str = "STATESET_SYNC_";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum SyncRuntimeAuth {
    ApiKey(String),
    BearerToken(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRuntimeConfig {
    pub engine: SyncConfig,
    pub sequencer_base_url: String,
    pub auth: Option<SyncRuntimeAuth>,
    pub agent_key_id: u32,
}

impl SyncRuntimeConfig {
    /// Default environment-variable prefix used by [`Self::from_env`].
    pub const DEFAULT_ENV_PREFIX: &str = DEFAULT_SYNC_RUNTIME_ENV_PREFIX;

    #[must_use]
    pub fn new(sequencer_base_url: impl Into<String>, engine: SyncConfig) -> Self {
        Self { engine, sequencer_base_url: sequencer_base_url.into(), auth: None, agent_key_id: 0 }
    }

    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.auth = Some(SyncRuntimeAuth::ApiKey(api_key.into()));
        self
    }

    #[must_use]
    pub fn with_bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.auth = Some(SyncRuntimeAuth::BearerToken(bearer_token.into()));
        self
    }

    #[must_use]
    pub const fn with_agent_key_id(mut self, agent_key_id: u32) -> Self {
        self.agent_key_id = agent_key_id;
        self
    }

    /// Validate the runtime config before building a runtime.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] when required fields are empty.
    pub fn validate(&self) -> Result<(), SyncError> {
        self.engine.validate()?;
        if self.sequencer_base_url.trim().is_empty() {
            return Err(SyncError::InvalidConfig("sequencer_base_url must not be empty".into()));
        }
        match self.auth.as_ref() {
            Some(SyncRuntimeAuth::ApiKey(api_key)) if api_key.trim().is_empty() => {
                Err(SyncError::InvalidConfig("sync runtime api key must not be empty".into()))
            }
            Some(SyncRuntimeAuth::BearerToken(bearer_token)) if bearer_token.trim().is_empty() => {
                Err(SyncError::InvalidConfig("sync runtime bearer token must not be empty".into()))
            }
            _ => Ok(()),
        }
    }

    /// Parse a runtime config from a JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Serialization`] for invalid JSON and
    /// [`SyncError::InvalidConfig`] for invalid values.
    pub fn from_json_str(json: &str) -> Result<Self, SyncError> {
        let config: Self = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Load a runtime config from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] when the file cannot be read and
    /// [`SyncError::Serialization`] or [`SyncError::InvalidConfig`] when the
    /// file contents are invalid.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, SyncError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| {
            SyncError::Storage(format!(
                "failed to read sync runtime config `{}`: {error}",
                path.display()
            ))
        })?;
        Self::from_json_str(&contents)
    }

    /// Load a runtime config from environment variables using the default
    /// `STATESET_SYNC_` prefix.
    ///
    /// Required variables:
    /// `STATESET_SYNC_SEQUENCER_BASE_URL`, `STATESET_SYNC_AGENT_ID`,
    /// `STATESET_SYNC_TENANT_ID`, and `STATESET_SYNC_STORE_ID`.
    ///
    /// Optional variables:
    /// `STATESET_SYNC_API_KEY`, `STATESET_SYNC_BEARER_TOKEN`,
    /// `STATESET_SYNC_AGENT_KEY_ID`, `STATESET_SYNC_BUFFER_CAPACITY`,
    /// `STATESET_SYNC_BATCH_SIZE`, `STATESET_SYNC_OUTBOX_CAPACITY`,
    /// `STATESET_SYNC_OUTBOX_PATH`, `STATESET_SYNC_STATE_PATH`,
    /// `STATESET_SYNC_CONFIRMATION_CAPACITY`,
    /// `STATESET_SYNC_REQUIRE_COMMITMENT_MANIFEST`,
    /// `STATESET_SYNC_TRUSTED_COMMITMENT_SIGNERS`, and
    /// `STATESET_SYNC_TRUSTED_COMMITMENT_PUBLIC_KEYS`.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] when required variables are missing,
    /// mutually exclusive auth variables are both set, or numeric values do not
    /// parse.
    pub fn from_env() -> Result<Self, SyncError> {
        Self::from_env_prefixed(Self::DEFAULT_ENV_PREFIX)
    }

    /// Load a runtime config from environment variables using a custom prefix.
    ///
    /// The prefix is concatenated directly with each variable name suffix. For
    /// example, `prefix = "STATESET_SYNC_"` reads `STATESET_SYNC_AGENT_ID`.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] using the same rules as
    /// [`Self::from_env`].
    pub fn from_env_prefixed(prefix: &str) -> Result<Self, SyncError> {
        let vars: HashMap<String, String> = std::env::vars().collect();
        load_runtime_config_from_env_map(prefix, &vars)
    }

    pub fn build(self) -> Result<SyncRuntime, SyncError> {
        self.validate()?;
        SyncRuntime::from_runtime_config(self)
    }
}

/// Serializable runtime inspection snapshot for CLI, FFI, and debug surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRuntimeSnapshot {
    /// Current aggregate sync status.
    pub status: SyncStatus,
    /// Verified commitment manifests retained by the runtime.
    #[serde(default)]
    pub verified_commitment_manifests: Vec<VerifiedCommitmentManifest>,
    /// Verified command settlement attestations retained by the runtime.
    #[serde(default)]
    pub command_attestations: Vec<CommandAttestation>,
    /// Command-level counterparty convergence derived from retained kernel receipts.
    #[serde(default)]
    pub command_convergences: Vec<CommandConvergence>,
    /// Unified kernel receipts spanning pending, confirmed, and rejected local events.
    #[serde(default)]
    pub kernel_receipts: Vec<KernelReceipt>,
    /// Retained push confirmations.
    #[serde(default)]
    pub confirmations: Vec<PushConfirmation>,
    /// Retained dead-letter entries.
    #[serde(default)]
    pub dead_letters: Vec<DeadLetter>,
    /// Buffered pulled events waiting for consumption.
    #[serde(default)]
    pub buffered_events: Vec<SyncEvent>,
}

impl SyncRuntimeSnapshot {
    /// Serialize the snapshot as compact JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Serialization`] if JSON encoding fails.
    pub fn to_json(&self) -> Result<String, SyncError> {
        serde_json::to_string(self).map_err(Into::into)
    }

    /// Serialize the snapshot as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Serialization`] if JSON encoding fails.
    pub fn to_json_pretty(&self) -> Result<String, SyncError> {
        serde_json::to_string_pretty(self).map_err(Into::into)
    }
}

/// Convenience runtime that bundles a [`SyncEngine`] with a concrete
/// [`SequencerHttpTransport`].
///
/// This gives Rust SDK consumers a single object for local recording, health
/// checks, remote head refresh, and push/pull/full-sync operations without
/// manually threading the transport through every engine call.
#[derive(Debug)]
pub struct SyncRuntime {
    engine: SyncEngine,
    transport: SequencerHttpTransport,
}

impl SyncRuntime {
    /// Create a sync runtime from a composed runtime config.
    ///
    /// # Errors
    ///
    /// Returns the same config and transport validation errors as [`Self::new`].
    pub fn from_runtime_config(config: SyncRuntimeConfig) -> Result<Self, SyncError> {
        config.validate()?;
        let SyncRuntimeConfig { engine, sequencer_base_url, auth, agent_key_id } = config;
        let mut runtime = Self::new(sequencer_base_url, engine)?;
        runtime = runtime.with_agent_key_id(agent_key_id);
        if let Some(auth) = auth {
            runtime = match auth {
                SyncRuntimeAuth::ApiKey(api_key) => runtime.with_api_key(api_key),
                SyncRuntimeAuth::BearerToken(bearer_token) => {
                    runtime.with_bearer_token(bearer_token)
                }
            };
        }
        Ok(runtime)
    }

    /// Create a sync runtime from a sequencer base URL and engine config.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] if either the engine config or
    /// sequencer transport configuration is invalid.
    pub fn new(
        sequencer_base_url: impl Into<String>,
        config: SyncConfig,
    ) -> Result<Self, SyncError> {
        let transport = SequencerHttpTransport::from_config(sequencer_base_url, &config)?;
        let engine = SyncEngine::new(config)?;
        Ok(Self { engine, transport })
    }

    /// Create a runtime from an existing engine and transport.
    #[must_use]
    pub const fn from_parts(engine: SyncEngine, transport: SequencerHttpTransport) -> Self {
        Self { engine, transport }
    }

    /// Consume the runtime and return the owned engine and transport.
    #[must_use]
    pub fn into_parts(self) -> (SyncEngine, SequencerHttpTransport) {
        (self.engine, self.transport)
    }

    /// Configure an API key on the underlying sequencer transport.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.transport = self.transport.clone().with_api_key(api_key);
        self
    }

    /// Configure a bearer token on the underlying sequencer transport.
    #[must_use]
    pub fn with_bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.transport = self.transport.clone().with_bearer_token(bearer_token);
        self
    }

    /// Configure the agent key id used for signed VES envelopes.
    #[must_use]
    pub fn with_agent_key_id(mut self, agent_key_id: u32) -> Self {
        self.transport = self.transport.clone().with_agent_key_id(agent_key_id);
        self
    }

    /// Return the owned sync engine.
    #[must_use]
    pub const fn engine(&self) -> &SyncEngine {
        &self.engine
    }

    /// Return a mutable reference to the owned sync engine.
    pub const fn engine_mut(&mut self) -> &mut SyncEngine {
        &mut self.engine
    }

    /// Return the configured HTTP transport.
    #[must_use]
    pub const fn transport(&self) -> &SequencerHttpTransport {
        &self.transport
    }

    /// Record a local event into the outbox.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::OutboxFull`] if the engine outbox is at capacity.
    pub fn record(&mut self, event: SyncEvent) -> Result<u64, SyncError> {
        self.engine.record(event)
    }

    /// Execute a local transaction-kernel request and record the resulting event.
    ///
    /// # Errors
    ///
    /// Returns the same policy, budget, and outbox errors as
    /// [`SyncEngine::record_kernel_transaction`].
    pub fn record_kernel_transaction(
        &mut self,
        transaction: KernelTransaction,
    ) -> Result<KernelReceipt, KernelExecutionError> {
        self.engine.record_kernel_transaction(transaction)
    }

    /// Return the current sync status snapshot.
    #[must_use]
    pub fn status(&self) -> SyncStatus {
        self.engine.status()
    }

    /// Return command-level counterparty convergence snapshots for retained commands.
    #[must_use]
    pub fn command_convergences(&self) -> Vec<CommandConvergence> {
        self.engine.command_convergences()
    }

    /// Return the command-level counterparty convergence snapshot for a command id, if known.
    #[must_use]
    pub fn command_convergence(&self, command_id: &str) -> Option<CommandConvergence> {
        self.engine.command_convergence(command_id)
    }

    /// Return all verified commitment manifests retained by the runtime.
    #[must_use]
    pub fn verified_commitment_manifests(&self) -> &[VerifiedCommitmentManifest] {
        self.engine.verified_commitment_manifests()
    }

    /// Return the verified commitment manifest for a specific commitment id, if known.
    #[must_use]
    pub fn verified_commitment_manifest(
        &self,
        commitment_id: &str,
    ) -> Option<&VerifiedCommitmentManifest> {
        self.engine.verified_commitment_manifest(commitment_id)
    }

    /// Verify and retain a signed commitment manifest against the current remote state.
    ///
    /// # Errors
    ///
    /// Returns the same manifest verification errors as
    /// [`SyncEngine::verify_commitment_manifest`].
    pub fn verify_commitment_manifest(
        &mut self,
        manifest: CommitmentManifest,
    ) -> Result<VerifiedCommitmentManifest, ManifestVerificationError> {
        self.engine.verify_commitment_manifest(manifest)
    }

    /// Return all verified command settlement attestations retained by the runtime.
    #[must_use]
    pub fn command_attestations(&self) -> &[CommandAttestation] {
        self.engine.command_attestations()
    }

    /// Return the verified command settlement attestation for a command id, if known.
    #[must_use]
    pub fn command_attestation(&self, command_id: &str) -> Option<&CommandAttestation> {
        self.engine.command_attestation(command_id)
    }

    /// Verify and retain a command inclusion proof against current kernel receipts and remote state.
    ///
    /// # Errors
    ///
    /// Returns the same attestation errors as [`SyncEngine::attest_command`].
    pub fn attest_command(
        &mut self,
        proof: CommandInclusionProof,
    ) -> Result<CommandAttestation, AttestationError> {
        self.engine.attest_command(proof)
    }

    /// Return the number of retained push confirmations available for inspection.
    #[must_use]
    pub fn confirmation_count(&self) -> usize {
        self.engine.confirmation_count()
    }

    /// Return the retained push confirmations.
    #[must_use]
    pub fn confirmations(&self) -> &[PushConfirmation] {
        self.engine.confirmations()
    }

    /// Return unified kernel receipts spanning pending, confirmed, and rejected local events.
    #[must_use]
    pub fn kernel_receipts(&self) -> Vec<KernelReceipt> {
        self.engine.kernel_receipts()
    }

    /// Return the unified kernel receipt for a local event id, if known.
    #[must_use]
    pub fn kernel_receipt_for_event(&self, event_id: Uuid) -> Option<KernelReceipt> {
        self.engine.kernel_receipt_for_event(event_id)
    }

    /// Return all unified kernel receipts associated with a command id.
    #[must_use]
    pub fn kernel_receipts_for_command(&self, command_id: &str) -> Vec<KernelReceipt> {
        self.engine.kernel_receipts_for_command(command_id)
    }

    /// Return all unified kernel receipts for an entity identity.
    #[must_use]
    pub fn kernel_receipts_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Vec<KernelReceipt> {
        self.engine.kernel_receipts_for_entity(entity_type, entity_id)
    }

    /// Return the latest unified kernel receipt associated with a command id.
    #[must_use]
    pub fn latest_kernel_receipt_for_command(&self, command_id: &str) -> Option<KernelReceipt> {
        self.engine.latest_kernel_receipt_for_command(command_id)
    }

    /// Return the latest unified kernel receipt for an entity identity.
    #[must_use]
    pub fn latest_kernel_receipt_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<KernelReceipt> {
        self.engine.latest_kernel_receipt_for_entity(entity_type, entity_id)
    }

    /// Return the retained confirmation for a local event id, if known.
    #[must_use]
    pub fn confirmation_for_event(&self, event_id: Uuid) -> Option<&PushConfirmation> {
        self.engine.confirmation_for_event(event_id)
    }

    /// Return the retained confirmation for a canonical remote sequence, if known.
    #[must_use]
    pub fn confirmation_for_remote_sequence(
        &self,
        remote_sequence: u64,
    ) -> Option<&PushConfirmation> {
        self.engine.confirmation_for_remote_sequence(remote_sequence)
    }

    /// Return all retained confirmations that share a receipt handle.
    #[must_use]
    pub fn confirmations_for_receipt(&self, receipt: &str) -> Vec<&PushConfirmation> {
        self.engine.confirmations_for_receipt(receipt)
    }

    /// Return all retained confirmations associated with a command id.
    #[must_use]
    pub fn confirmations_for_command(&self, command_id: &str) -> Vec<&PushConfirmation> {
        self.engine.confirmations_for_command(command_id)
    }

    /// Return all retained confirmations for an entity identity.
    #[must_use]
    pub fn confirmations_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Vec<&PushConfirmation> {
        self.engine.confirmations_for_entity(entity_type, entity_id)
    }

    /// Return the latest retained confirmation associated with a command id.
    #[must_use]
    pub fn latest_confirmation_for_command(&self, command_id: &str) -> Option<&PushConfirmation> {
        self.engine.latest_confirmation_for_command(command_id)
    }

    /// Return the latest retained confirmation for an entity identity.
    #[must_use]
    pub fn latest_confirmation_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<&PushConfirmation> {
        self.engine.latest_confirmation_for_entity(entity_type, entity_id)
    }

    /// Drain all retained push confirmations and persist the updated state.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if the runtime state snapshot cannot be updated.
    pub fn drain_confirmations(&mut self) -> Result<Vec<PushConfirmation>, SyncError> {
        self.engine.drain_confirmations()
    }

    /// Return the number of events pending in the outbox.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.engine.pending_count()
    }

    /// Return the number of dead-lettered events retained by the runtime.
    #[must_use]
    pub fn dead_letter_count(&self) -> usize {
        self.engine.dead_letter_count()
    }

    /// Return the current dead-letter queue.
    #[must_use]
    pub fn dead_letters(&self) -> &[DeadLetter] {
        self.engine.dead_letters()
    }

    /// Return the retained dead-letter entry for a local event id, if known.
    #[must_use]
    pub fn dead_letter_for_event(&self, event_id: Uuid) -> Option<&DeadLetter> {
        self.engine.dead_letter_for_event(event_id)
    }

    /// Return all retained dead-letter entries associated with a command id.
    #[must_use]
    pub fn dead_letters_for_command(&self, command_id: &str) -> Vec<&DeadLetter> {
        self.engine.dead_letters_for_command(command_id)
    }

    /// Return all retained dead-letter entries for an entity identity.
    #[must_use]
    pub fn dead_letters_for_entity(&self, entity_type: &str, entity_id: &str) -> Vec<&DeadLetter> {
        self.engine.dead_letters_for_entity(entity_type, entity_id)
    }

    /// Return the latest retained dead-letter entry associated with a command id.
    #[must_use]
    pub fn latest_dead_letter_for_command(&self, command_id: &str) -> Option<&DeadLetter> {
        self.engine.latest_dead_letter_for_command(command_id)
    }

    /// Return the latest retained dead-letter entry for an entity identity.
    #[must_use]
    pub fn latest_dead_letter_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<&DeadLetter> {
        self.engine.latest_dead_letter_for_entity(entity_type, entity_id)
    }

    /// Requeue a dead-lettered event back into the local outbox.
    ///
    /// Returns the newly assigned local outbox sequence number.
    ///
    /// # Errors
    ///
    /// Returns the same requeue errors as [`SyncEngine::requeue_dead_letter`].
    pub fn requeue_dead_letter(&mut self, event_id: Uuid) -> Result<u64, SyncError> {
        self.engine.requeue_dead_letter(event_id)
    }

    /// Permanently discard a dead-letter entry.
    ///
    /// # Errors
    ///
    /// Returns the same discard errors as [`SyncEngine::discard_dead_letter`].
    pub fn discard_dead_letter(&mut self, event_id: Uuid) -> Result<DeadLetter, SyncError> {
        self.engine.discard_dead_letter(event_id)
    }

    /// Drain all retained dead-letter entries and persist the updated state.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if the runtime state snapshot cannot be updated.
    pub fn drain_dead_letters(&mut self) -> Result<Vec<DeadLetter>, SyncError> {
        self.engine.drain_dead_letters()
    }

    /// Return the number of events currently in the pull buffer.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.engine.buffered_count()
    }

    /// Drain all buffered pulled events.
    #[must_use]
    pub fn drain_buffer(&mut self) -> Vec<SyncEvent> {
        self.engine.drain_buffer()
    }

    /// Snapshot runtime status, confirmations, dead letters, and buffered events.
    #[must_use]
    pub fn snapshot(&self) -> SyncRuntimeSnapshot {
        SyncRuntimeSnapshot {
            status: self.status(),
            verified_commitment_manifests: self.verified_commitment_manifests().to_vec(),
            command_attestations: self.command_attestations().to_vec(),
            command_convergences: self.command_convergences(),
            kernel_receipts: self.kernel_receipts(),
            confirmations: self.confirmations().to_vec(),
            dead_letters: self.dead_letters().to_vec(),
            buffered_events: self.engine.buffered_events(),
        }
    }

    /// Serialize a runtime snapshot as compact JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Serialization`] if JSON encoding fails.
    pub fn snapshot_json(&self) -> Result<String, SyncError> {
        self.snapshot().to_json()
    }

    /// Serialize a runtime snapshot as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Serialization`] if JSON encoding fails.
    pub fn snapshot_json_pretty(&self) -> Result<String, SyncError> {
        self.snapshot().to_json_pretty()
    }

    /// Probe the remote sequencer health endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the sequencer is unreachable.
    pub async fn healthcheck(&self) -> Result<(), SyncError> {
        self.transport.healthcheck().await
    }

    /// Refresh the known canonical remote head without pulling events.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] or [`SyncError::Storage`] if the
    /// underlying engine refresh fails.
    pub async fn refresh_remote_head(&mut self) -> Result<RemoteHead, SyncError> {
        self.engine.refresh_remote_head(&self.transport).await
    }

    /// Push pending local events through the configured transport.
    ///
    /// # Errors
    ///
    /// Returns the same push errors as [`SyncEngine::push`].
    pub async fn push(&mut self) -> Result<PushResult, SyncError> {
        self.engine.push(&self.transport).await
    }

    /// Pull remote events through the configured transport.
    ///
    /// # Errors
    ///
    /// Returns the same pull errors as [`SyncEngine::pull`].
    pub async fn pull(&mut self) -> Result<PullResult, SyncError> {
        self.engine.pull(&self.transport).await
    }

    /// Perform a full sync using the configured transport.
    ///
    /// # Errors
    ///
    /// Returns the first push or pull error encountered.
    pub async fn full_sync(&mut self) -> Result<(PushResult, PullResult), SyncError> {
        self.engine.full_sync(&self.transport).await
    }
}

fn load_runtime_config_from_env_map(
    prefix: &str,
    vars: &HashMap<String, String>,
) -> Result<SyncRuntimeConfig, SyncError> {
    let sequencer_base_url = required_env_string(vars, prefix, "SEQUENCER_BASE_URL")?;
    let agent_id = required_env_string(vars, prefix, "AGENT_ID")?;
    let tenant_id = required_env_string(vars, prefix, "TENANT_ID")?;
    let store_id = required_env_string(vars, prefix, "STORE_ID")?;

    let mut engine = SyncConfig::new(agent_id, tenant_id, store_id);
    if let Some(buffer_capacity) = optional_env_parsed::<usize>(vars, prefix, "BUFFER_CAPACITY")? {
        engine = engine.with_buffer_capacity(buffer_capacity);
    }
    if let Some(batch_size) = optional_env_parsed::<usize>(vars, prefix, "BATCH_SIZE")? {
        engine = engine.with_batch_size(batch_size);
    }
    if let Some(outbox_capacity) = optional_env_parsed::<usize>(vars, prefix, "OUTBOX_CAPACITY")? {
        engine = engine.with_outbox_capacity(outbox_capacity);
    }
    if let Some(outbox_path) = optional_env_string(vars, prefix, "OUTBOX_PATH")? {
        engine = engine.with_outbox_path(outbox_path);
    }
    if let Some(state_path) = optional_env_string(vars, prefix, "STATE_PATH")? {
        engine = engine.with_state_path(state_path);
    }
    if let Some(confirmation_capacity) =
        optional_env_parsed::<usize>(vars, prefix, "CONFIRMATION_CAPACITY")?
    {
        engine = engine.with_confirmation_capacity(confirmation_capacity);
    }
    if let Some(require_manifest) =
        optional_env_parsed::<bool>(vars, prefix, "REQUIRE_COMMITMENT_MANIFEST")?
    {
        engine = engine.with_require_commitment_manifest(require_manifest);
    }
    if let Some(trusted_signers) = optional_env_csv(vars, prefix, "TRUSTED_COMMITMENT_SIGNERS")? {
        for signer in trusted_signers {
            engine = engine.with_trusted_commitment_signer(signer);
        }
    }
    if let Some(trusted_public_keys) =
        optional_env_csv(vars, prefix, "TRUSTED_COMMITMENT_PUBLIC_KEYS")?
    {
        for public_key in trusted_public_keys {
            engine = engine.with_trusted_commitment_signer_public_key(public_key);
        }
    }

    let api_key_var = env_var_name(prefix, "API_KEY");
    let bearer_token_var = env_var_name(prefix, "BEARER_TOKEN");
    let auth = match (
        optional_env_string(vars, prefix, "API_KEY")?,
        optional_env_string(vars, prefix, "BEARER_TOKEN")?,
    ) {
        (Some(_), Some(_)) => {
            return Err(SyncError::InvalidConfig(format!(
                "environment variables `{api_key_var}` and `{bearer_token_var}` are mutually exclusive"
            )));
        }
        (Some(api_key), None) => Some(SyncRuntimeAuth::ApiKey(api_key)),
        (None, Some(bearer_token)) => Some(SyncRuntimeAuth::BearerToken(bearer_token)),
        (None, None) => None,
    };

    let agent_key_id = optional_env_parsed::<u32>(vars, prefix, "AGENT_KEY_ID")?.unwrap_or(0);

    let config = SyncRuntimeConfig { engine, sequencer_base_url, auth, agent_key_id };
    config.validate()?;
    Ok(config)
}

fn env_var_name(prefix: &str, suffix: &str) -> String {
    format!("{prefix}{suffix}")
}

fn required_env_string(
    vars: &HashMap<String, String>,
    prefix: &str,
    suffix: &str,
) -> Result<String, SyncError> {
    let var_name = env_var_name(prefix, suffix);
    let Some(value) = vars.get(&var_name) else {
        return Err(SyncError::InvalidConfig(format!(
            "missing required environment variable `{var_name}`"
        )));
    };
    normalize_env_string(&var_name, value)
}

fn optional_env_string(
    vars: &HashMap<String, String>,
    prefix: &str,
    suffix: &str,
) -> Result<Option<String>, SyncError> {
    let var_name = env_var_name(prefix, suffix);
    match vars.get(&var_name) {
        Some(value) => normalize_env_string(&var_name, value).map(Some),
        None => Ok(None),
    }
}

fn normalize_env_string(var_name: &str, value: &str) -> Result<String, SyncError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SyncError::InvalidConfig(format!(
            "environment variable `{var_name}` must not be empty"
        )));
    }
    Ok(trimmed.to_owned())
}

fn optional_env_parsed<T>(
    vars: &HashMap<String, String>,
    prefix: &str,
    suffix: &str,
) -> Result<Option<T>, SyncError>
where
    T: FromStr,
    T::Err: Display,
{
    let var_name = env_var_name(prefix, suffix);
    let Some(value) = optional_env_string(vars, prefix, suffix)? else {
        return Ok(None);
    };
    value.parse::<T>().map(Some).map_err(|error| {
        SyncError::InvalidConfig(format!("environment variable `{var_name}` is invalid: {error}"))
    })
}

fn optional_env_csv(
    vars: &HashMap<String, String>,
    prefix: &str,
    suffix: &str,
) -> Result<Option<Vec<String>>, SyncError> {
    let Some(value) = optional_env_string(vars, prefix, suffix)? else {
        return Ok(None);
    };
    let entries = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    Ok(Some(entries))
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use async_trait::async_trait;
    use serde_json::json;

    use crate::sync::{
        BudgetAuthorization, BudgetCheckpoint, CommandInclusionProof,
        CounterpartyConvergenceStatus, KernelReceiptStatus, KernelTransaction, PolicyCheckpoint,
        PolicyDecision, PushAcknowledgement, PushRejection, Transport,
    };

    use super::*;

    fn make_config() -> SyncConfig {
        SyncConfig::new("agent-1", "tenant-1", "store-1")
    }

    fn temp_config_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        temp_dir()
            .join(format!("stateset-sync-runtime-config-{name}-{}-{nanos}.json", process::id()))
    }

    fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn make_runtime_transport() -> SequencerHttpTransport {
        SequencerHttpTransport::from_config("https://sequencer.stateset.com", &make_config())
            .unwrap()
    }

    #[derive(Debug, Clone, Default)]
    struct AckTransport;

    #[async_trait]
    impl Transport for AckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    PushAcknowledgement::new(event.id, (index + 1) as u64)
                        .with_receipt(format!("receipt-{}", index + 1))
                })
                .collect();
            Ok(PushResult::accepted_only(events.len(), events.len() as u64)
                .with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: Vec::new(), remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(0))
        }
    }

    #[derive(Debug, Clone, Default)]
    struct RejectingTransport;

    #[async_trait]
    impl Transport for RejectingTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let rejections = events
                .iter()
                .map(|event| {
                    PushRejection::new(event.id)
                        .with_code("invalid_event")
                        .with_reason("event rejected")
                        .with_retryable(false)
                })
                .collect();
            Ok(PushResult::accepted_only(0, 0).with_rejections(rejections))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: Vec::new(), remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(0))
        }
    }

    #[derive(Debug, Clone, Default)]
    struct PullTransport;

    #[async_trait]
    impl Transport for PullTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            let events = if since >= 7 {
                Vec::new()
            } else {
                vec![
                    SyncEvent::new("inventory.adjusted", "inventory", "SKU-1", json!({"delta": 2}))
                        .with_command_id("cmd-buffer")
                        .with_remote_sequence(7),
                ]
            };
            Ok(PullResult { events, remote_head: 7, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(7))
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MixedPushTransport;

    #[async_trait]
    impl Transport for MixedPushTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let mut acknowledgements = Vec::new();
            let mut rejections = Vec::new();
            if let Some(event) = events.first() {
                acknowledgements
                    .push(PushAcknowledgement::new(event.id, 11).with_receipt("receipt-mixed"));
            }
            if let Some(event) = events.get(1) {
                rejections.push(
                    PushRejection::new(event.id)
                        .with_code("invalid_event")
                        .with_reason("event rejected")
                        .with_retryable(false),
                );
            }
            Ok(PushResult::accepted_only(acknowledgements.len(), 11)
                .with_acknowledgements(acknowledgements)
                .with_rejections(rejections))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: Vec::new(), remote_head: 11, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(11))
        }
    }

    #[test]
    fn runtime_new_bundles_engine_and_transport() {
        let runtime = SyncRuntime::new("https://sequencer.stateset.com/", make_config()).unwrap();

        assert_eq!(runtime.engine().config().agent_id, "agent-1");
        assert_eq!(runtime.transport().base_url(), "https://sequencer.stateset.com");
        assert_eq!(runtime.transport().tenant_id(), "tenant-1");
        assert_eq!(runtime.status().pending, 0);
    }

    #[test]
    fn runtime_builder_methods_update_transport_configuration() {
        let runtime = SyncRuntime::new("https://sequencer.stateset.com", make_config())
            .unwrap()
            .with_agent_key_id(9)
            .with_api_key("ss_example_key")
            .with_bearer_token("token");

        assert_eq!(runtime.transport().agent_key_id(), 9);
        assert!(runtime.transport().has_api_key());
        assert!(runtime.transport().has_bearer_token());
    }

    #[test]
    fn runtime_config_builds_runtime_with_api_key_auth() {
        let runtime = SyncRuntimeConfig::new("https://sequencer.stateset.com", make_config())
            .with_api_key("ss_example_key")
            .with_agent_key_id(7)
            .build()
            .unwrap();

        assert_eq!(runtime.transport().agent_key_id(), 7);
        assert!(runtime.transport().has_api_key());
        assert!(!runtime.transport().has_bearer_token());
    }

    #[test]
    fn runtime_config_builds_runtime_with_bearer_auth() {
        let runtime = SyncRuntime::from_runtime_config(
            SyncRuntimeConfig::new("https://sequencer.stateset.com", make_config())
                .with_bearer_token("bearer-token"),
        )
        .unwrap();

        assert!(!runtime.transport().has_api_key());
        assert!(runtime.transport().has_bearer_token());
    }

    #[test]
    fn runtime_config_round_trips_from_json() {
        let json = serde_json::to_string(
            &SyncRuntimeConfig::new("https://sequencer.stateset.com", make_config())
                .with_api_key("ss_example_key")
                .with_agent_key_id(42),
        )
        .unwrap();

        let config = SyncRuntimeConfig::from_json_str(&json).unwrap();

        assert_eq!(config.sequencer_base_url, "https://sequencer.stateset.com");
        assert_eq!(config.agent_key_id, 42);
        assert_eq!(config.engine.agent_id, "agent-1");
        assert!(
            matches!(config.auth, Some(SyncRuntimeAuth::ApiKey(ref key)) if key == "ss_example_key")
        );
    }

    #[test]
    fn runtime_config_loads_from_file() {
        let path = temp_config_path("from-file");
        let payload = serde_json::to_string(
            &SyncRuntimeConfig::new(
                "https://sequencer.stateset.com",
                make_config()
                    .with_batch_size(64)
                    .with_outbox_path("/tmp/outbox.json")
                    .with_state_path("/tmp/state.json"),
            )
            .with_bearer_token("bearer-token")
            .with_agent_key_id(11),
        )
        .unwrap();
        fs::write(&path, payload).unwrap();

        let config = SyncRuntimeConfig::from_file(&path).unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(config.engine.batch_size, 64);
        assert_eq!(config.engine.outbox_path.as_deref(), Some("/tmp/outbox.json"));
        assert_eq!(config.engine.state_path.as_deref(), Some("/tmp/state.json"));
        assert_eq!(config.agent_key_id, 11);
        assert!(
            matches!(config.auth, Some(SyncRuntimeAuth::BearerToken(ref token)) if token == "bearer-token")
        );
    }

    #[test]
    fn runtime_config_loads_from_env_map() {
        let prefix = "TEST_SYNC_";
        let vars = HashMap::from([
            (format!("{prefix}SEQUENCER_BASE_URL"), "https://sequencer.stateset.com".to_owned()),
            (format!("{prefix}AGENT_ID"), "agent-from-env".to_owned()),
            (format!("{prefix}TENANT_ID"), "tenant-from-env".to_owned()),
            (format!("{prefix}STORE_ID"), "store-from-env".to_owned()),
            (format!("{prefix}API_KEY"), "ss_env_key".to_owned()),
            (format!("{prefix}AGENT_KEY_ID"), "27".to_owned()),
            (format!("{prefix}BUFFER_CAPACITY"), "2048".to_owned()),
            (format!("{prefix}BATCH_SIZE"), "32".to_owned()),
            (format!("{prefix}OUTBOX_CAPACITY"), "4096".to_owned()),
            (format!("{prefix}OUTBOX_PATH"), "/tmp/env-outbox.json".to_owned()),
            (format!("{prefix}STATE_PATH"), "/tmp/env-state.json".to_owned()),
            (format!("{prefix}CONFIRMATION_CAPACITY"), "88".to_owned()),
            (format!("{prefix}REQUIRE_COMMITMENT_MANIFEST"), "true".to_owned()),
            (format!("{prefix}TRUSTED_COMMITMENT_SIGNERS"), "sequencer-a,sequencer-b".to_owned()),
            (format!("{prefix}TRUSTED_COMMITMENT_PUBLIC_KEYS"), "aa,bb".to_owned()),
        ]);

        let config = load_runtime_config_from_env_map(prefix, &vars).unwrap();

        assert_eq!(config.sequencer_base_url, "https://sequencer.stateset.com");
        assert_eq!(config.engine.agent_id, "agent-from-env");
        assert_eq!(config.engine.tenant_id, "tenant-from-env");
        assert_eq!(config.engine.store_id, "store-from-env");
        assert_eq!(config.engine.buffer_capacity, 2048);
        assert_eq!(config.engine.batch_size, 32);
        assert_eq!(config.engine.outbox_capacity, 4096);
        assert_eq!(config.engine.outbox_path.as_deref(), Some("/tmp/env-outbox.json"));
        assert_eq!(config.engine.state_path.as_deref(), Some("/tmp/env-state.json"));
        assert_eq!(config.engine.confirmation_capacity, 88);
        assert!(config.engine.commitment_trust.require_manifest);
        assert_eq!(
            config.engine.commitment_trust.trusted_signer_ids,
            vec!["sequencer-a".to_string(), "sequencer-b".to_string()]
        );
        assert_eq!(
            config.engine.commitment_trust.trusted_signer_public_keys,
            vec!["aa".to_string(), "bb".to_string()]
        );
        assert_eq!(config.agent_key_id, 27);
        assert!(
            matches!(config.auth, Some(SyncRuntimeAuth::ApiKey(ref key)) if key == "ss_env_key")
        );
    }

    #[test]
    fn runtime_config_rejects_conflicting_env_auth() {
        let prefix = "TEST_SYNC_";
        let vars = HashMap::from([
            (format!("{prefix}SEQUENCER_BASE_URL"), "https://sequencer.stateset.com".to_owned()),
            (format!("{prefix}AGENT_ID"), "agent-from-env".to_owned()),
            (format!("{prefix}TENANT_ID"), "tenant-from-env".to_owned()),
            (format!("{prefix}STORE_ID"), "store-from-env".to_owned()),
            (format!("{prefix}API_KEY"), "ss_env_key".to_owned()),
            (format!("{prefix}BEARER_TOKEN"), "token".to_owned()),
        ]);

        let error = load_runtime_config_from_env_map(prefix, &vars).unwrap_err();

        assert!(matches!(error, SyncError::InvalidConfig(_)));
        assert!(error.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn runtime_record_forwards_to_engine() {
        let mut runtime =
            SyncRuntime::new("https://sequencer.stateset.com", make_config()).unwrap();

        let sequence = runtime
            .record(SyncEvent::new("order.created", "order", "ORD-1", json!({"total": 99})))
            .unwrap();

        assert_eq!(sequence, 1);
        assert_eq!(runtime.pending_count(), 1);
        assert_eq!(runtime.engine().pending_count(), 1);
        assert_eq!(runtime.status().pending, 1);
    }

    #[test]
    fn runtime_record_kernel_transaction_returns_pending_receipt() {
        let mut runtime =
            SyncRuntime::new("https://sequencer.stateset.com", make_config()).unwrap();

        let receipt = runtime
            .record_kernel_transaction(
                KernelTransaction::new(SyncEvent::new(
                    "order.created",
                    "order",
                    "ORD-9",
                    json!({"total": 45}),
                ))
                .with_policy_checkpoint(PolicyCheckpoint::new("orders", PolicyDecision::Allowed))
                .with_budget_authorization(BudgetAuthorization::new("budget-9", 4500, 5000, "USD")),
            )
            .unwrap();

        assert_eq!(receipt.status, KernelReceiptStatus::LocalPending);
        assert_eq!(receipt.local_sequence, Some(1));
        assert_eq!(runtime.pending_count(), 1);
    }

    #[tokio::test]
    async fn runtime_exposes_confirmation_queries_and_drain() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let first = SyncEvent::new("order.created", "order", "ORD-1", json!({"total": 99}))
            .with_command_id("cmd-1")
            .with_policy_checkpoint(
                PolicyCheckpoint::new("orders", PolicyDecision::Allowed)
                    .with_reason("within threshold"),
            );
        let second = SyncEvent::new("order.updated", "order", "ORD-1", json!({"total": 109}))
            .with_command_id("cmd-1")
            .with_budget_checkpoint(
                BudgetCheckpoint::new("budget-1", 10900, "USD").with_remaining_amount_minor(900),
            );
        let first_id = first.id;
        let second_id = second.id;

        engine.record(first).unwrap();
        engine.record(second).unwrap();
        let push = engine.push(&AckTransport).await.unwrap();
        assert_eq!(push.accepted, 2);

        let mut runtime = SyncRuntime::from_parts(engine, make_runtime_transport());

        assert_eq!(runtime.command_convergences().len(), 1);
        assert_eq!(
            runtime.command_convergence("cmd-1").unwrap().status,
            CounterpartyConvergenceStatus::ConfirmedRemote
        );
        assert_eq!(runtime.kernel_receipts().len(), 2);
        assert_eq!(
            runtime.kernel_receipt_for_event(first_id).unwrap().status,
            KernelReceiptStatus::ConfirmedRemote
        );
        assert_eq!(runtime.kernel_receipts_for_command("cmd-1").len(), 2);
        assert_eq!(runtime.kernel_receipts_for_entity("order", "ORD-1").len(), 2);
        assert_eq!(runtime.latest_kernel_receipt_for_command("cmd-1").unwrap().event_id, second_id);
        assert_eq!(
            runtime.latest_kernel_receipt_for_entity("order", "ORD-1").unwrap().event_id,
            second_id
        );
        assert_eq!(runtime.confirmation_count(), 2);
        assert_eq!(runtime.confirmations().len(), 2);
        assert_eq!(runtime.pending_count(), 0);
        assert_eq!(runtime.confirmation_for_event(first_id).unwrap().remote_sequence, 1);
        assert_eq!(runtime.confirmation_for_remote_sequence(2).unwrap().event_id, second_id);
        assert_eq!(runtime.confirmations_for_receipt("receipt-1").len(), 1);
        assert_eq!(runtime.confirmations_for_command("cmd-1").len(), 2);
        assert_eq!(runtime.confirmations_for_entity("order", "ORD-1").len(), 2);
        assert_eq!(runtime.latest_confirmation_for_command("cmd-1").unwrap().remote_sequence, 2);
        assert_eq!(
            runtime.latest_confirmation_for_entity("order", "ORD-1").unwrap().remote_sequence,
            2
        );

        let drained = runtime.drain_confirmations().unwrap();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].event_id, first_id);
        assert_eq!(runtime.confirmation_count(), 0);
    }

    #[tokio::test]
    async fn runtime_exposes_dead_letter_queries_and_recovery() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event =
            SyncEvent::new("payment.failed", "payment", "PAY-1", json!({"status": "failed"}))
                .with_command_id("cmd-reject")
                .with_policy_checkpoint(PolicyCheckpoint::new("payments", PolicyDecision::Denied));
        let event_id = event.id;

        engine.record(event).unwrap();
        let push = engine.push(&RejectingTransport).await.unwrap();
        assert_eq!(push.rejections.len(), 1);

        let mut runtime = SyncRuntime::from_parts(engine, make_runtime_transport());

        assert_eq!(
            runtime.command_convergence("cmd-reject").unwrap().status,
            CounterpartyConvergenceStatus::RejectedRemote
        );
        assert_eq!(runtime.kernel_receipts().len(), 1);
        assert_eq!(
            runtime.kernel_receipt_for_event(event_id).unwrap().status,
            KernelReceiptStatus::RejectedRemote
        );
        assert_eq!(runtime.dead_letter_count(), 1);
        assert_eq!(runtime.dead_letters().len(), 1);
        assert_eq!(runtime.dead_letter_for_event(event_id).unwrap().event.id, event_id);
        assert_eq!(runtime.dead_letters_for_command("cmd-reject").len(), 1);
        assert_eq!(runtime.dead_letters_for_entity("payment", "PAY-1").len(), 1);
        assert_eq!(
            runtime.latest_dead_letter_for_command("cmd-reject").unwrap().event.id,
            event_id
        );
        assert_eq!(
            runtime.latest_dead_letter_for_entity("payment", "PAY-1").unwrap().event.id,
            event_id
        );

        let sequence = runtime.requeue_dead_letter(event_id).unwrap();
        assert_eq!(sequence, 2);
        assert_eq!(runtime.dead_letter_count(), 0);
        assert_eq!(runtime.pending_count(), 1);

        let (mut engine, transport) = runtime.into_parts();
        let push = engine.push(&RejectingTransport).await.unwrap();
        assert_eq!(push.rejections.len(), 1);

        let mut runtime = SyncRuntime::from_parts(engine, transport);
        let discarded = runtime.discard_dead_letter(event_id).unwrap();
        assert_eq!(discarded.event.id, event_id);
        assert_eq!(runtime.dead_letter_count(), 0);
    }

    #[tokio::test]
    async fn runtime_attests_command_against_remote_commitment() {
        let (private_key, public_key) = stateset_crypto::sign::generate_keypair();

        #[derive(Debug)]
        struct HeadTransport {
            remote_head: RemoteHead,
        }

        #[async_trait]
        impl Transport for HeadTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), self.remote_head.remote_head))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult {
                    events: Vec::new(),
                    remote_head: self.remote_head.remote_head,
                    has_more: false,
                })
            }

            async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
                Ok(self.remote_head.clone())
            }

            async fn pull_events_page(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<crate::sync::PullPage, SyncError> {
                Ok(crate::sync::PullPage {
                    result: PullResult {
                        events: Vec::new(),
                        remote_head: self.remote_head.remote_head,
                        has_more: false,
                    },
                    next_cursor: None,
                    observed_cursor: Some(self.remote_head.remote_head),
                })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = SyncEvent::new("order.created", "order", "ORD-4", json!({"total": 44}))
            .with_command_id("cmd-attest-runtime");
        engine.record(event.clone()).unwrap();
        let push = engine.push(&AckTransport).await.unwrap();
        assert_eq!(push.accepted, 1);

        let receipts = engine.kernel_receipts_for_command("cmd-attest-runtime");
        let leaf_hash =
            crate::sync::compute_command_settlement_leaf("cmd-attest-runtime", &receipts).unwrap();
        let root = hex_encode(leaf_hash);
        let head_transport = HeadTransport {
            remote_head: RemoteHead::new(1)
                .with_state_root(root.clone())
                .with_last_commitment_id("BATCH-1"),
        };
        engine.refresh_remote_head(&head_transport).await.unwrap();
        engine.pull(&head_transport).await.unwrap();

        let mut runtime = SyncRuntime::from_parts(engine, make_runtime_transport());
        let verified_manifest = runtime
            .verify_commitment_manifest(
                crate::sync::sign_commitment_manifest(
                    crate::sync::CommitmentManifest::new(
                        "BATCH-1",
                        root.clone(),
                        1,
                        "sequencer-runtime",
                    ),
                    &private_key,
                    &public_key,
                )
                .unwrap(),
            )
            .unwrap();
        let attestation = runtime
            .attest_command(
                CommandInclusionProof::new("cmd-attest-runtime", root, 0, 1)
                    .with_commitment_id("BATCH-1"),
            )
            .unwrap();

        assert_eq!(verified_manifest.signer_id, "sequencer-runtime");
        assert_eq!(attestation.max_remote_sequence, 1);
        assert!(attestation.settled);
        assert_eq!(attestation.manifest_signer_id.as_deref(), Some("sequencer-runtime"));
        assert!(attestation.manifest_verified_at.is_some());
        assert_eq!(runtime.verified_commitment_manifests().len(), 1);
        assert_eq!(runtime.command_attestations().len(), 1);
        assert_eq!(
            runtime
                .command_attestation("cmd-attest-runtime")
                .and_then(|stored| stored.commitment_id.as_deref()),
            Some("BATCH-1")
        );
        assert_eq!(
            runtime
                .verified_commitment_manifest("BATCH-1")
                .map(|manifest| manifest.signer_id.as_str()),
            Some("sequencer-runtime")
        );
        assert_eq!(runtime.snapshot().verified_commitment_manifests.len(), 1);
    }

    #[tokio::test]
    async fn runtime_exposes_buffer_drain_helpers() {
        let mut engine = SyncEngine::new(make_config()).unwrap();

        let pull = engine.pull(&PullTransport).await.unwrap();
        assert_eq!(pull.events.len(), 1);

        let mut runtime = SyncRuntime::from_parts(engine, make_runtime_transport());
        assert_eq!(runtime.buffered_count(), 1);

        let drained = runtime.drain_buffer();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].entity_type, "inventory");
        assert_eq!(drained[0].entity_id, "SKU-1");
        assert_eq!(drained[0].canonical_sequence(), Some(7));
        assert_eq!(runtime.buffered_count(), 0);
    }

    #[tokio::test]
    async fn runtime_snapshot_round_trips_with_confirmations_dead_letters_and_buffer() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine
            .record(
                SyncEvent::new("order.created", "order", "ORD-2", json!({"total": 150}))
                    .with_command_id("cmd-snapshot-confirmed")
                    .with_policy_checkpoint(PolicyCheckpoint::new(
                        "orders",
                        PolicyDecision::Allowed,
                    )),
            )
            .unwrap();
        engine
            .record(
                SyncEvent::new("order.failed", "order", "ORD-3", json!({"reason": "invalid"}))
                    .with_command_id("cmd-snapshot-rejected")
                    .with_budget_checkpoint(BudgetCheckpoint::new("budget-2", 150, "USD")),
            )
            .unwrap();

        let push = engine.push(&MixedPushTransport).await.unwrap();
        assert_eq!(push.accepted, 1);
        assert_eq!(push.rejections.len(), 1);

        let pull = engine.pull(&PullTransport).await.unwrap();
        assert_eq!(pull.events.len(), 1);

        let runtime = SyncRuntime::from_parts(engine, make_runtime_transport());
        let snapshot = runtime.snapshot();

        assert_eq!(snapshot.status.pending, 0);
        assert_eq!(snapshot.status.retained_confirmations, 1);
        assert_eq!(snapshot.status.dead_letters, 1);
        assert_eq!(snapshot.status.buffered_events, 1);
        assert_eq!(snapshot.verified_commitment_manifests.len(), 0);
        assert_eq!(snapshot.command_attestations.len(), 0);
        assert_eq!(snapshot.command_convergences.len(), 2);
        assert_eq!(snapshot.kernel_receipts.len(), 2);
        assert_eq!(snapshot.kernel_receipts[0].status, KernelReceiptStatus::ConfirmedRemote);
        assert_eq!(snapshot.kernel_receipts[1].status, KernelReceiptStatus::RejectedRemote);
        assert_eq!(snapshot.confirmations.len(), 1);
        assert_eq!(snapshot.confirmations[0].entity_id, "ORD-2");
        assert_eq!(snapshot.dead_letters.len(), 1);
        assert_eq!(snapshot.dead_letters[0].event.entity_id, "ORD-3");
        assert_eq!(snapshot.buffered_events.len(), 1);
        assert_eq!(snapshot.buffered_events[0].entity_id, "SKU-1");

        let json = runtime.snapshot_json_pretty().unwrap();
        let decoded: SyncRuntimeSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status.dead_letters, 1);
        assert_eq!(decoded.verified_commitment_manifests.len(), 0);
        assert_eq!(decoded.command_attestations.len(), 0);
        assert_eq!(decoded.command_convergences.len(), 2);
        assert_eq!(decoded.kernel_receipts.len(), 2);
        assert_eq!(decoded.confirmations.len(), 1);
        assert_eq!(decoded.dead_letters.len(), 1);
        assert_eq!(decoded.buffered_events.len(), 1);
        assert_eq!(decoded.buffered_events[0].canonical_sequence(), Some(7));
    }
}
