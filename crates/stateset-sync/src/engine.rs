use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::buffer::EventBuffer;
use crate::config::SyncConfig;
use crate::conflict::{ConflictResolver, ConflictStrategy, Resolution};
use crate::error::SyncError;
use crate::event::SyncEvent;
use crate::outbox::Outbox;
use crate::state::{SyncState, SyncStatus};
use crate::transport::{
    PullPage, PullResult, PushAcknowledgement, PushRejection, PushResult, RemoteHead, Transport,
    derive_next_cursor,
};

/// Safety stop for paginated pull loops in `full_sync`.
const MAX_PULL_PAGES: usize = 10_000;

/// A non-retryable pushed event that the sequencer explicitly rejected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeadLetter {
    /// The local event that was rejected.
    pub event: SyncEvent,
    /// The rejection metadata reported by the remote.
    pub rejection: PushRejection,
    /// Timestamp when the event was moved out of the outbox.
    pub rejected_at: DateTime<Utc>,
}

impl DeadLetter {
    /// Create a new dead-letter entry for a rejected local event.
    #[must_use]
    pub fn new(event: SyncEvent, rejection: PushRejection) -> Self {
        Self { event, rejection, rejected_at: Utc::now() }
    }
}

/// Durable record that a local event received a canonical remote sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushConfirmation {
    /// Local event id that was accepted by the sequencer.
    pub event_id: Uuid,
    /// Optional upstream command identifier associated with the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// Event type confirmed by the sequencer.
    pub event_type: String,
    /// Entity type confirmed by the sequencer.
    pub entity_type: String,
    /// Entity id confirmed by the sequencer.
    pub entity_id: String,
    /// Provisional local outbox sequence that originally carried the event, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_sequence: Option<u64>,
    /// Canonical remote sequence assigned by the sequencer.
    pub remote_sequence: u64,
    /// VES payload hash for the confirmed event.
    pub hash: String,
    /// Optional sequencer receipt handle or hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    /// Timestamp when the confirmation was retained locally.
    pub confirmed_at: DateTime<Utc>,
}

impl PushConfirmation {
    /// Create a retained confirmation from a local event and sequencer acknowledgement.
    #[must_use]
    pub fn from_ack(event: &SyncEvent, acknowledgement: &PushAcknowledgement) -> Self {
        Self {
            event_id: event.id,
            command_id: event.command_id.clone(),
            event_type: event.event_type.clone(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            local_sequence: event.local_sequence(),
            remote_sequence: acknowledgement.remote_sequence,
            hash: event.hash.clone(),
            receipt: acknowledgement.receipt.clone(),
            confirmed_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncEngineSnapshot {
    state: SyncState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next_pull_cursor: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dead_letters: Vec<DeadLetter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    confirmations: Vec<PushConfirmation>,
}

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
/// let mut engine = SyncEngine::new(config).expect("valid sync config");
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
    state_path: Option<PathBuf>,
    next_pull_cursor: Option<u64>,
    dead_letters: Vec<DeadLetter>,
    confirmations: Vec<PushConfirmation>,
    initialized: bool,
}

impl SyncEngine {
    /// Create a new `SyncEngine` with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] for invalid settings or
    /// [`SyncError::Storage`] when durable outbox initialization fails.
    pub fn new(config: SyncConfig) -> Result<Self, SyncError> {
        Self::try_new(config)
    }

    /// Create a `SyncEngine` with a custom conflict resolution strategy.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] for invalid settings or
    /// [`SyncError::Storage`] when durable outbox initialization fails.
    pub fn with_strategy(
        config: SyncConfig,
        strategy: ConflictStrategy,
    ) -> Result<Self, SyncError> {
        Self::try_with_strategy(config, strategy)
    }

    /// Fallible constructor that validates config and initializes persistence.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] for invalid settings or
    /// [`SyncError::Storage`] when durable outbox initialization fails.
    pub fn try_new(config: SyncConfig) -> Result<Self, SyncError> {
        Self::try_with_strategy(config, ConflictStrategy::default())
    }

    /// Fallible constructor with explicit conflict strategy.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] for invalid settings or
    /// [`SyncError::Storage`] when durable outbox initialization fails.
    pub fn try_with_strategy(
        config: SyncConfig,
        strategy: ConflictStrategy,
    ) -> Result<Self, SyncError> {
        config.validate()?;
        let buffer_capacity = config.resolved_buffer_capacity();
        let outbox = if let Some(path) = config.outbox_path.as_deref() {
            Outbox::with_persistence(config.resolved_outbox_capacity(), path)?
        } else {
            Outbox::new(config.resolved_outbox_capacity())
        };
        let state_path = Self::resolved_state_path(&config);
        let snapshot = if let Some(path) = state_path.as_deref() {
            Self::load_state_snapshot(path)?
        } else {
            None
        };
        let (mut state, next_pull_cursor, dead_letters, mut confirmations) =
            if let Some(snapshot) = snapshot {
                (
                    snapshot.state,
                    snapshot.next_pull_cursor,
                    snapshot.dead_letters,
                    snapshot.confirmations,
                )
            } else {
                (SyncState::default(), None, Vec::new(), Vec::new())
            };
        state.local_head = state.local_head.max(outbox.next_sequence().saturating_sub(1));
        state.pending_count = outbox.count();
        let confirmation_capacity = config.resolved_confirmation_capacity();
        if confirmations.len() > confirmation_capacity {
            let overflow = confirmations.len() - confirmation_capacity;
            confirmations.drain(0..overflow);
        }

        Ok(Self {
            config,
            state,
            outbox,
            buffer: EventBuffer::new(buffer_capacity),
            resolver: ConflictResolver::new(strategy),
            state_path,
            next_pull_cursor,
            dead_letters,
            confirmations,
            initialized: true,
        })
    }

    fn resolved_state_path(config: &SyncConfig) -> Option<PathBuf> {
        if let Some(path) = config.state_path.as_deref() {
            return Some(PathBuf::from(path));
        }
        config
            .outbox_path
            .as_deref()
            .map(|path| Self::default_state_path_for_outbox(Path::new(path)))
    }

    fn default_state_path_for_outbox(path: &Path) -> PathBuf {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return path.with_file_name("sync-state.json");
        };

        let derived_name = if let Some((stem, ext)) = file_name.rsplit_once('.') {
            format!("{stem}.state.{ext}")
        } else {
            format!("{file_name}.state.json")
        };
        path.with_file_name(derived_name)
    }

    fn load_state_snapshot(path: &Path) -> Result<Option<SyncEngineSnapshot>, SyncError> {
        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(path).map_err(|error| {
            SyncError::Storage(format!("read sync-state snapshot failed: {error}"))
        })?;
        if contents.trim().is_empty() {
            return Ok(None);
        }
        let snapshot = serde_json::from_str(&contents)?;
        Ok(Some(snapshot))
    }

    fn persist_runtime_state(&self) -> Result<(), SyncError> {
        let Some(path) = self.state_path.as_deref() else {
            return Ok(());
        };

        let snapshot = SyncEngineSnapshot {
            state: self.state.clone(),
            next_pull_cursor: self.next_pull_cursor,
            dead_letters: self.dead_letters.clone(),
            confirmations: self.confirmations.clone(),
        };
        let serialized = serde_json::to_string_pretty(&snapshot)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                SyncError::Storage(format!("create sync-state snapshot directory failed: {error}"))
            })?;
        }

        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, serialized).map_err(|error| {
            SyncError::Storage(format!("write sync-state snapshot failed: {error}"))
        })?;
        fs::rename(&tmp_path, path).map_err(|error| {
            SyncError::Storage(format!("replace sync-state snapshot failed: {error}"))
        })?;
        Ok(())
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

    fn dead_letter_index(&self, event_id: Uuid) -> Option<usize> {
        self.dead_letters.iter().position(|dead_letter| dead_letter.event.id == event_id)
    }

    /// Push pending events from the outbox to the remote via the given transport.
    ///
    /// Drains up to `batch_size` events from the outbox.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    pub async fn push(&mut self, transport: &dyn Transport) -> Result<PushResult, SyncError> {
        let batch_size = self.config.resolved_batch_size();
        let events: Vec<SyncEvent> = self.outbox.peek(batch_size).into_iter().cloned().collect();

        if events.is_empty() {
            return Ok(PushResult::accepted_only(0, self.state.remote_head));
        }

        let result = transport.push_events(&events).await?;
        Self::validate_push_result(&events, &result)?;

        let dead_letters = Self::collect_dead_letters(&events, &result.rejections);
        let dead_letter_ids: HashSet<_> = dead_letters.iter().map(|entry| entry.event.id).collect();
        let removable_ids: HashSet<_> = if result.acknowledgements.is_empty() {
            events
                .iter()
                .take(result.accepted)
                .map(|event| event.id)
                .chain(dead_letter_ids.iter().copied())
                .collect()
        } else {
            result
                .acknowledgements
                .iter()
                .map(|ack| ack.event_id)
                .chain(dead_letter_ids.iter().copied())
                .collect()
        };

        if !removable_ids.is_empty() {
            if let Err(err) = self.outbox.try_retain(|event| !removable_ids.contains(&event.id)) {
                self.state.pending_count = self.outbox.count();
                return Err(err);
            }
        }

        if !result.acknowledgements.is_empty() {
            self.retain_push_confirmations(&events, &result.acknowledgements);
        }

        if !dead_letters.is_empty() {
            self.dead_letters.extend(dead_letters);
        }

        self.state.remote_head = self.state.remote_head.max(result.remote_head);
        if let Some(acknowledged_head) = result.acknowledged_head() {
            self.state.last_acknowledged_remote_sequence = Some(
                self.state
                    .last_acknowledged_remote_sequence
                    .map_or(acknowledged_head, |current| current.max(acknowledged_head)),
            );
        }
        self.state.last_push = Some(Utc::now());
        self.state.pending_count = self.outbox.count();
        self.persist_runtime_state()?;

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
        let since = self.next_pull_cursor.unwrap_or(self.state.remote_cursor);
        let (result, next_cursor) = self.pull_since(transport, since).await?;
        self.next_pull_cursor = next_cursor;
        self.persist_runtime_state()?;
        Ok(result)
    }

    async fn pull_since(
        &mut self,
        transport: &dyn Transport,
        since: u64,
    ) -> Result<(PullResult, Option<u64>), SyncError> {
        let limit = self.config.resolved_batch_size();
        let PullPage {
            result,
            next_cursor: transport_next_cursor,
            observed_cursor: transport_observed_cursor,
        } = transport.pull_events_page(since, limit).await?;

        // Detect and resolve conflicts between pending outbox events and pulled events
        let pending: Vec<SyncEvent> =
            self.outbox.peek(self.outbox.count()).into_iter().cloned().collect();
        let mut drop_local_ids = HashSet::new();
        let mut events_to_buffer = Vec::with_capacity(result.events.len());

        for pulled_event in &result.events {
            let mut keep_remote = true;

            if let Some(local_event) = pending.iter().rev().find(|local_event| {
                local_event.entity_type == pulled_event.entity_type
                    && local_event.entity_id == pulled_event.entity_id
            }) {
                match self.resolver.resolve(local_event, pulled_event) {
                    Resolution::KeepLocal => {
                        keep_remote = false;
                    }
                    Resolution::KeepRemote => {
                        drop_local_ids.insert(local_event.id);
                    }
                    Resolution::Merge(merged) => {
                        drop_local_ids.insert(local_event.id);
                        events_to_buffer.push(merged);
                        keep_remote = false;
                    }
                }
            }

            if keep_remote {
                events_to_buffer.push(pulled_event.clone());
            }
        }

        if !drop_local_ids.is_empty() {
            self.outbox.try_retain(|event| !drop_local_ids.contains(&event.id))?;
            self.state.pending_count = self.outbox.count();
        }

        // Buffer resolved events
        for event in events_to_buffer {
            self.buffer.push(event);
        }

        self.state.remote_head = result.remote_head;
        self.state.last_pull = Some(Utc::now());
        let observed_cursor = transport_observed_cursor
            .filter(|cursor| *cursor > since)
            .or_else(|| derive_next_cursor(since, &result.events))
            .unwrap_or(since);
        self.state.remote_cursor = self.state.remote_cursor.max(observed_cursor);

        let next_cursor = Self::resolve_next_cursor(
            since,
            &result.events,
            result.has_more,
            transport_next_cursor,
        )?;

        Ok((result, next_cursor))
    }

    fn collect_dead_letters(events: &[SyncEvent], rejections: &[PushRejection]) -> Vec<DeadLetter> {
        let events_by_id: HashMap<_, _> = events.iter().map(|event| (event.id, event)).collect();
        rejections
            .iter()
            .filter(|rejection| rejection.retryable != Some(true))
            .filter_map(|rejection| {
                events_by_id
                    .get(&rejection.event_id)
                    .map(|event| DeadLetter::new((*event).clone(), rejection.clone()))
            })
            .collect()
    }

    fn collect_push_confirmations(
        events: &[SyncEvent],
        acknowledgements: &[PushAcknowledgement],
    ) -> Vec<PushConfirmation> {
        let events_by_id: HashMap<_, _> = events.iter().map(|event| (event.id, event)).collect();
        acknowledgements
            .iter()
            .filter_map(|acknowledgement| {
                events_by_id
                    .get(&acknowledgement.event_id)
                    .map(|event| PushConfirmation::from_ack(event, acknowledgement))
            })
            .collect()
    }

    fn retain_push_confirmations(
        &mut self,
        events: &[SyncEvent],
        acknowledgements: &[PushAcknowledgement],
    ) {
        for confirmation in Self::collect_push_confirmations(events, acknowledgements) {
            self.confirmations.retain(|existing| {
                existing.event_id != confirmation.event_id
                    && existing.remote_sequence != confirmation.remote_sequence
            });
            self.confirmations.push(confirmation);
        }
        self.trim_confirmations_to_capacity();
    }

    fn trim_confirmations_to_capacity(&mut self) {
        let capacity = self.config.resolved_confirmation_capacity();
        if self.confirmations.len() > capacity {
            let overflow = self.confirmations.len() - capacity;
            self.confirmations.drain(0..overflow);
        }
    }

    fn validate_push_result(events: &[SyncEvent], result: &PushResult) -> Result<(), SyncError> {
        if result.accepted > events.len() {
            return Err(SyncError::Transport(format!(
                "push acknowledged more events than were sent (sent={}, accepted={})",
                events.len(),
                result.accepted
            )));
        }

        if result.accepted + result.rejections.len() > events.len() {
            return Err(SyncError::Transport(format!(
                "push reported more terminal outcomes than were sent (sent={}, accepted={}, rejected={})",
                events.len(),
                result.accepted,
                result.rejections.len()
            )));
        }

        if !result.acknowledgements.is_empty() && result.acknowledgements.len() != result.accepted {
            return Err(SyncError::Transport(format!(
                "push acknowledgements did not match accepted count (accepted={}, acknowledgements={})",
                result.accepted,
                result.acknowledgements.len()
            )));
        }

        let expected_ids: HashSet<_> = events.iter().map(|event| event.id).collect();
        let mut seen_ids =
            HashSet::with_capacity(result.acknowledgements.len() + result.rejections.len());

        for acknowledgement in &result.acknowledgements {
            if acknowledgement.remote_sequence == 0 {
                return Err(SyncError::Transport(
                    "push acknowledgement contained remote_sequence=0".to_string(),
                ));
            }

            if !expected_ids.contains(&acknowledgement.event_id) {
                return Err(SyncError::Transport(
                    "push acknowledgement did not match a sent local event".to_string(),
                ));
            }

            if !seen_ids.insert(acknowledgement.event_id) {
                return Err(SyncError::Transport(
                    "push acknowledgement contained duplicate event ids".to_string(),
                ));
            }
        }

        for rejection in &result.rejections {
            if !expected_ids.contains(&rejection.event_id) {
                return Err(SyncError::Transport(
                    "push rejection did not match a sent local event".to_string(),
                ));
            }

            if !seen_ids.insert(rejection.event_id) {
                return Err(SyncError::Transport(
                    "push result contained duplicate event ids across acknowledgements/rejections"
                        .to_string(),
                ));
            }
        }

        if result.acknowledgements.is_empty() && !result.rejections.is_empty() {
            let assumed_accepted_prefix: HashSet<_> =
                events.iter().take(result.accepted).map(|event| event.id).collect();
            if result
                .rejections
                .iter()
                .any(|rejection| assumed_accepted_prefix.contains(&rejection.event_id))
            {
                return Err(SyncError::Transport(
                    "push rejection overlapped the accepted prefix without per-event acknowledgements"
                        .to_string(),
                ));
            }
        }

        if let Some(acknowledged_head) = result.acknowledged_head() {
            if acknowledged_head > result.remote_head {
                return Err(SyncError::Transport(format!(
                    "push acknowledgement head exceeded remote head (ack_head={}, remote_head={})",
                    acknowledged_head, result.remote_head
                )));
            }
        }

        Ok(())
    }

    fn resolve_next_cursor(
        since: u64,
        events: &[SyncEvent],
        has_more: bool,
        transport_next_cursor: Option<u64>,
    ) -> Result<Option<u64>, SyncError> {
        if !has_more {
            return Ok(None);
        }

        let next_cursor = transport_next_cursor.or_else(|| derive_next_cursor(since, events));
        let Some(next_cursor) = next_cursor else {
            return Err(SyncError::Transport(
                "pull pagination stalled: has_more=true but no advancing cursor available"
                    .to_string(),
            ));
        };
        if next_cursor <= since {
            return Err(SyncError::Transport(format!(
                "pull pagination cursor did not advance (since={since}, next_cursor={next_cursor})"
            )));
        }
        Ok(Some(next_cursor))
    }

    fn remote_head_snapshot(&self) -> RemoteHead {
        let mut head = RemoteHead::new(self.state.remote_head);
        if let Some(state_root) = self.state.remote_state_root.clone() {
            head = head.with_state_root(state_root);
        }
        if let Some(commitment_id) = self.state.last_commitment_id.clone() {
            head = head.with_last_commitment_id(commitment_id);
        }
        head
    }

    /// Refresh the known canonical remote head without pulling events.
    ///
    /// This updates [`SyncState::remote_head`] but does not advance the local
    /// pull cursor.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport cannot fetch remote
    /// head state, or [`SyncError::Storage`] if persisting the updated runtime
    /// state fails.
    pub async fn refresh_remote_head(
        &mut self,
        transport: &dyn Transport,
    ) -> Result<RemoteHead, SyncError> {
        let observed = transport.fetch_head().await?;

        match observed.remote_head.cmp(&self.state.remote_head) {
            std::cmp::Ordering::Greater => {
                self.state.remote_head = observed.remote_head;
                self.state.remote_state_root = observed.state_root.clone();
                self.state.last_commitment_id = observed.last_commitment_id.clone();
            }
            std::cmp::Ordering::Equal => {
                if let Some(state_root) = observed.state_root.clone() {
                    self.state.remote_state_root = Some(state_root);
                }
                if let Some(commitment_id) = observed.last_commitment_id.clone() {
                    self.state.last_commitment_id = Some(commitment_id);
                }
            }
            std::cmp::Ordering::Less => {}
        }

        let head = self.remote_head_snapshot();
        self.persist_runtime_state()?;
        Ok(head)
    }

    /// Get the current sync status.
    #[must_use]
    pub fn status(&self) -> SyncStatus {
        SyncStatus {
            initialized: self.initialized,
            local_head: self.state.local_head,
            remote_head: self.state.remote_head,
            remote_state_root: self.state.remote_state_root.clone(),
            last_commitment_id: self.state.last_commitment_id.clone(),
            remote_cursor: self.state.remote_cursor,
            next_pull_cursor: self.next_pull_cursor,
            last_acknowledged_remote_sequence: self.state.last_acknowledged_remote_sequence,
            pending: self.outbox.count(),
            dead_letters: self.dead_letters.len(),
            retained_confirmations: self.confirmations.len(),
            lag: self.state.lag(),
            caught_up: self.state.is_synced(),
            last_push: self.state.last_push,
            last_pull: self.state.last_pull,
            buffered_events: self.buffer.len(),
        }
    }

    /// Return the number of retained push confirmations available for inspection.
    #[must_use]
    pub fn confirmation_count(&self) -> usize {
        self.confirmations.len()
    }

    /// Return the retained push confirmations.
    #[must_use]
    pub fn confirmations(&self) -> &[PushConfirmation] {
        &self.confirmations
    }

    /// Return the retained confirmation for a local event id, if known.
    #[must_use]
    pub fn confirmation_for_event(&self, event_id: Uuid) -> Option<&PushConfirmation> {
        self.confirmations.iter().find(|confirmation| confirmation.event_id == event_id)
    }

    /// Return the retained confirmation for a canonical remote sequence, if known.
    #[must_use]
    pub fn confirmation_for_remote_sequence(
        &self,
        remote_sequence: u64,
    ) -> Option<&PushConfirmation> {
        self.confirmations
            .iter()
            .find(|confirmation| confirmation.remote_sequence == remote_sequence)
    }

    /// Return all retained confirmations that share a receipt handle.
    #[must_use]
    pub fn confirmations_for_receipt(&self, receipt: &str) -> Vec<&PushConfirmation> {
        self.confirmations
            .iter()
            .filter(|confirmation| confirmation.receipt.as_deref() == Some(receipt))
            .collect()
    }

    /// Return all retained confirmations associated with a command id.
    #[must_use]
    pub fn confirmations_for_command(&self, command_id: &str) -> Vec<&PushConfirmation> {
        self.confirmations
            .iter()
            .filter(|confirmation| confirmation.command_id.as_deref() == Some(command_id))
            .collect()
    }

    /// Return all retained confirmations for an entity identity.
    #[must_use]
    pub fn confirmations_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Vec<&PushConfirmation> {
        self.confirmations
            .iter()
            .filter(|confirmation| {
                confirmation.entity_type == entity_type && confirmation.entity_id == entity_id
            })
            .collect()
    }

    /// Return the latest retained confirmation associated with a command id.
    #[must_use]
    pub fn latest_confirmation_for_command(&self, command_id: &str) -> Option<&PushConfirmation> {
        self.confirmations
            .iter()
            .filter(|confirmation| confirmation.command_id.as_deref() == Some(command_id))
            .max_by_key(|confirmation| confirmation.remote_sequence)
    }

    /// Return the latest retained confirmation for an entity identity.
    #[must_use]
    pub fn latest_confirmation_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<&PushConfirmation> {
        self.confirmations
            .iter()
            .filter(|confirmation| {
                confirmation.entity_type == entity_type && confirmation.entity_id == entity_id
            })
            .max_by_key(|confirmation| confirmation.remote_sequence)
    }

    /// Drain all retained push confirmations and persist the updated state.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if the runtime state snapshot cannot be updated.
    pub fn drain_confirmations(&mut self) -> Result<Vec<PushConfirmation>, SyncError> {
        let confirmations = std::mem::take(&mut self.confirmations);
        if let Err(err) = self.persist_runtime_state() {
            self.confirmations = confirmations;
            return Err(err);
        }
        Ok(confirmations)
    }

    /// Return the number of events pending in the outbox.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.outbox.count()
    }

    /// Return the number of dead-lettered events retained by the engine.
    #[must_use]
    pub fn dead_letter_count(&self) -> usize {
        self.dead_letters.len()
    }

    /// Return the current dead-letter queue.
    #[must_use]
    pub fn dead_letters(&self) -> &[DeadLetter] {
        &self.dead_letters
    }

    /// Return the retained dead-letter entry for a local event id, if known.
    #[must_use]
    pub fn dead_letter_for_event(&self, event_id: Uuid) -> Option<&DeadLetter> {
        self.dead_letters.iter().find(|dead_letter| dead_letter.event.id == event_id)
    }

    /// Return all retained dead-letter entries associated with a command id.
    #[must_use]
    pub fn dead_letters_for_command(&self, command_id: &str) -> Vec<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| dead_letter.event.command_id.as_deref() == Some(command_id))
            .collect()
    }

    /// Return all retained dead-letter entries for an entity identity.
    #[must_use]
    pub fn dead_letters_for_entity(&self, entity_type: &str, entity_id: &str) -> Vec<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| {
                dead_letter.event.entity_type == entity_type
                    && dead_letter.event.entity_id == entity_id
            })
            .collect()
    }

    /// Return the latest retained dead-letter entry associated with a command id.
    #[must_use]
    pub fn latest_dead_letter_for_command(&self, command_id: &str) -> Option<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| dead_letter.event.command_id.as_deref() == Some(command_id))
            .max_by_key(|dead_letter| dead_letter.rejected_at)
    }

    /// Return the latest retained dead-letter entry for an entity identity.
    #[must_use]
    pub fn latest_dead_letter_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| {
                dead_letter.event.entity_type == entity_type
                    && dead_letter.event.entity_id == entity_id
            })
            .max_by_key(|dead_letter| dead_letter.rejected_at)
    }

    /// Requeue a dead-lettered event back into the local outbox.
    ///
    /// Returns the newly assigned local outbox sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::NotFound`] if the dead-letter entry is missing,
    /// [`SyncError::DuplicateEvent`] if the event is already pending in the
    /// outbox, or [`SyncError::Storage`] if persistence fails.
    pub fn requeue_dead_letter(&mut self, event_id: Uuid) -> Result<u64, SyncError> {
        let Some(index) = self.dead_letter_index(event_id) else {
            return Err(SyncError::NotFound(format!("dead-letter event {event_id}")));
        };
        if self.outbox.contains_event_id(event_id) {
            return Err(SyncError::DuplicateEvent(event_id.to_string()));
        }

        let dead_letter = self.dead_letters[index].clone();
        let sequence = self.outbox.append(dead_letter.event.clone())?;
        let removed = self.dead_letters.remove(index);
        self.state.local_head = self.state.local_head.max(sequence);
        self.state.pending_count = self.outbox.count();

        if let Err(err) = self.persist_runtime_state() {
            self.dead_letters.insert(index, removed);
            if let Err(rollback_err) = self.outbox.try_retain(|event| event.id != event_id) {
                self.state.pending_count = self.outbox.count();
                return Err(SyncError::Storage(format!(
                    "requeue dead-letter rollback failed after snapshot error: snapshot={err}; outbox={rollback_err}"
                )));
            }
            self.state.pending_count = self.outbox.count();
            return Err(err);
        }

        Ok(sequence)
    }

    /// Permanently discard a dead-letter entry.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::NotFound`] if the dead-letter entry is missing or
    /// [`SyncError::Storage`] if persistence fails.
    pub fn discard_dead_letter(&mut self, event_id: Uuid) -> Result<DeadLetter, SyncError> {
        let Some(index) = self.dead_letter_index(event_id) else {
            return Err(SyncError::NotFound(format!("dead-letter event {event_id}")));
        };

        let dead_letter = self.dead_letters.remove(index);
        if let Err(err) = self.persist_runtime_state() {
            self.dead_letters.insert(index, dead_letter.clone());
            return Err(err);
        }
        Ok(dead_letter)
    }

    /// Drain all retained dead-letter entries and persist the updated state.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if the runtime state snapshot cannot be updated.
    pub fn drain_dead_letters(&mut self) -> Result<Vec<DeadLetter>, SyncError> {
        let dead_letters = std::mem::take(&mut self.dead_letters);
        if let Err(err) = self.persist_runtime_state() {
            self.dead_letters = dead_letters;
            return Err(err);
        }
        Ok(dead_letters)
    }

    /// Return the number of events currently in the pull buffer.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Snapshot all buffered pulled events without draining them.
    #[must_use]
    pub fn buffered_events(&self) -> Vec<SyncEvent> {
        self.buffer.snapshot()
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
        let mut since = self.next_pull_cursor.unwrap_or(self.state.remote_cursor);
        let (mut pull_result, next_cursor) = self.pull_since(transport, since).await?;
        self.next_pull_cursor = next_cursor;
        self.persist_runtime_state()?;
        let mut pull_pages = 1;

        while pull_result.has_more {
            if pull_pages >= MAX_PULL_PAGES {
                return Err(SyncError::Transport(
                    "pull pagination exceeded safety limit".to_string(),
                ));
            }

            since = self.next_pull_cursor.ok_or_else(|| {
                SyncError::Transport(
                    "pull pagination stalled: has_more=true but no continuation cursor".to_string(),
                )
            })?;

            let (next_page, page_cursor) = self.pull_since(transport, since).await?;
            self.next_pull_cursor = page_cursor;
            self.persist_runtime_state()?;

            pull_result.events.extend(next_page.events);
            pull_result.remote_head = next_page.remote_head;
            pull_result.has_more = next_page.has_more;
            pull_pages += 1;
        }

        self.next_pull_cursor = None;
        self.persist_runtime_state()?;
        Ok((push_result, pull_result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::NullTransport;
    use proptest::prelude::*;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tempfile::tempdir;

    fn make_config() -> SyncConfig {
        SyncConfig::new("agent-1", "tenant-1", "store-1")
    }

    fn make_event(event_type: &str) -> SyncEvent {
        SyncEvent::new(event_type, "order", "ORD-1", json!({}))
    }

    #[test]
    fn new_engine() {
        let engine = SyncEngine::new(make_config()).unwrap();
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.buffered_count(), 0);
        assert!(engine.status().initialized);
    }

    #[test]
    fn record_event() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let seq = engine.record(make_event("order.created")).unwrap();
        assert_eq!(seq, 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.state().local_head, 1);
    }

    #[test]
    fn record_multiple_events() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();
        engine.record(make_event("c")).unwrap();
        assert_eq!(engine.pending_count(), 3);
        assert_eq!(engine.state().local_head, 3);
    }

    #[tokio::test]
    async fn push_with_null_transport() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
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
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let transport = NullTransport::new();
        let result = engine.push(&transport).await.unwrap();
        assert_eq!(result.accepted, 0);
    }

    #[tokio::test]
    async fn pull_with_null_transport() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
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
                Ok(PushResult::accepted_only(events.len(), self.head))
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
                make_event("pulled-1").with_remote_sequence(1),
                make_event("pulled-2").with_remote_sequence(2),
            ],
            head: 2,
        };

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let result = engine.pull(&transport).await.unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(engine.buffered_count(), 2);
        assert_eq!(engine.state().remote_head, 2);
        assert_eq!(engine.state().remote_cursor, 2);
    }

    #[tokio::test]
    async fn full_sync() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("local")).unwrap();

        let transport = NullTransport::new();
        let (push_result, pull_result) = engine.full_sync(&transport).await.unwrap();
        assert_eq!(push_result.accepted, 1);
        assert!(pull_result.events.is_empty());
        assert_eq!(engine.pending_count(), 0);
    }

    #[tokio::test]
    async fn refresh_remote_head_updates_known_head_without_advancing_cursor() {
        #[derive(Debug)]
        struct HeadTransport;

        #[async_trait::async_trait]
        impl Transport for HeadTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 0))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
            }

            async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
                Ok(RemoteHead::new(15)
                    .with_state_root("root-15")
                    .with_last_commitment_id("BATCH-15"))
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();

        let head = engine.refresh_remote_head(&HeadTransport).await.unwrap();
        assert_eq!(head.remote_head, 15);
        assert_eq!(head.state_root.as_deref(), Some("root-15"));
        assert_eq!(head.last_commitment_id.as_deref(), Some("BATCH-15"));
        assert_eq!(engine.state().remote_head, 15);
        assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-15"));
        assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-15"));
        assert_eq!(engine.state().remote_cursor, 0);
        assert_eq!(engine.state().lag(), 15);
    }

    #[tokio::test]
    async fn refresh_remote_head_ignores_stale_metadata_from_lower_head() {
        #[derive(Debug)]
        struct StaleHeadTransport;

        #[async_trait::async_trait]
        impl Transport for StaleHeadTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 0))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
            }

            async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
                Ok(RemoteHead::new(10)
                    .with_state_root("stale-root")
                    .with_last_commitment_id("STALE-BATCH"))
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.state.remote_head = 12;
        engine.state.remote_state_root = Some("root-12".into());
        engine.state.last_commitment_id = Some("BATCH-12".into());

        let head = engine.refresh_remote_head(&StaleHeadTransport).await.unwrap();
        assert_eq!(head.remote_head, 12);
        assert_eq!(head.state_root.as_deref(), Some("root-12"));
        assert_eq!(head.last_commitment_id.as_deref(), Some("BATCH-12"));
        assert_eq!(engine.state().remote_head, 12);
        assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-12"));
        assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-12"));
    }

    #[tokio::test]
    async fn refresh_remote_head_persists_state_snapshot() {
        #[derive(Debug)]
        struct HeadTransport;

        #[async_trait::async_trait]
        impl Transport for HeadTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 0))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
            }

            async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
                Ok(RemoteHead::new(21)
                    .with_state_root("root-21")
                    .with_last_commitment_id("BATCH-21"))
            }
        }

        let dir = tempdir().unwrap();
        let state_path = dir.path().join("sync-state.json");
        let config = make_config().with_state_path(state_path.to_string_lossy().into_owned());

        {
            let mut engine = SyncEngine::new(config.clone()).unwrap();
            engine.refresh_remote_head(&HeadTransport).await.unwrap();
            assert_eq!(engine.state().remote_head, 21);
            assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-21"));
            assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-21"));
        }

        let engine = SyncEngine::new(config).unwrap();
        assert_eq!(engine.state().remote_head, 21);
        assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-21"));
        assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-21"));
    }

    #[test]
    fn status_reporting() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let status = engine.status();
        assert!(status.initialized);
        assert_eq!(status.pending, 2);
        assert_eq!(status.dead_letters, 0);
        assert_eq!(status.retained_confirmations, 0);
        assert_eq!(status.local_head, 2);
        assert_eq!(status.remote_head, 0);
        assert_eq!(status.remote_state_root, None);
        assert_eq!(status.last_commitment_id, None);
        assert_eq!(status.remote_cursor, 0);
        assert_eq!(status.next_pull_cursor, None);
        assert_eq!(status.last_acknowledged_remote_sequence, None);
        assert_eq!(status.lag, 0);
        assert!(!status.caught_up);
        assert!(status.last_push.is_none());
    }

    #[test]
    fn status_reports_continuation_cursor_when_pull_is_in_progress() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.state.remote_head = 10;
        engine.state.remote_cursor = 8;
        engine.next_pull_cursor = Some(9);

        let status = engine.status();
        assert_eq!(status.next_pull_cursor, Some(9));
        assert!(!status.caught_up);

        engine.state.remote_cursor = 10;
        engine.next_pull_cursor = None;
        let status = engine.status();
        assert!(status.caught_up);
    }

    #[test]
    fn drain_confirmations_clears_engine_log() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = make_event("confirmed").with_command_id("cmd-1");
        let acknowledgement =
            crate::transport::PushAcknowledgement::new(event.id, 42).with_receipt("receipt-42");
        engine.confirmations.push(PushConfirmation::from_ack(&event, &acknowledgement));

        let drained = engine.drain_confirmations().unwrap();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].event_id, event.id);
        assert_eq!(drained[0].receipt.as_deref(), Some("receipt-42"));
        assert_eq!(engine.confirmation_count(), 0);
    }

    #[test]
    fn confirmation_lookup_supports_event_sequence_and_receipt_queries() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let first = make_event("confirmed-a").with_command_id("cmd-a").with_local_sequence(1);
        let second = make_event("confirmed-b").with_command_id("cmd-a").with_local_sequence(2);
        let first_ack =
            crate::transport::PushAcknowledgement::new(first.id, 42).with_receipt("batch-1");
        let second_ack =
            crate::transport::PushAcknowledgement::new(second.id, 43).with_receipt("batch-1");
        engine.confirmations.push(PushConfirmation::from_ack(&first, &first_ack));
        engine.confirmations.push(PushConfirmation::from_ack(&second, &second_ack));

        let by_event = engine.confirmation_for_event(first.id).unwrap();
        assert_eq!(by_event.command_id.as_deref(), Some("cmd-a"));
        assert_eq!(by_event.remote_sequence, 42);

        let by_sequence = engine.confirmation_for_remote_sequence(43).unwrap();
        assert_eq!(by_sequence.event_id, second.id);

        let by_receipt = engine.confirmations_for_receipt("batch-1");
        assert_eq!(by_receipt.len(), 2);

        let by_command = engine.confirmations_for_command("cmd-a");
        assert_eq!(by_command.len(), 2);
        assert_eq!(by_command[0].event_id, first.id);
        assert_eq!(by_command[1].event_id, second.id);

        let latest_by_command = engine.latest_confirmation_for_command("cmd-a").unwrap();
        assert_eq!(latest_by_command.event_id, second.id);
        assert_eq!(latest_by_command.remote_sequence, 43);

        let by_entity = engine.confirmations_for_entity("order", "ORD-1");
        assert_eq!(by_entity.len(), 2);

        let latest_by_entity = engine.latest_confirmation_for_entity("order", "ORD-1").unwrap();
        assert_eq!(latest_by_entity.event_id, second.id);
        assert_eq!(latest_by_entity.remote_sequence, 43);

        assert!(engine.confirmation_for_event(uuid::Uuid::new_v4()).is_none());
        assert!(engine.confirmation_for_remote_sequence(99).is_none());
        assert!(engine.confirmations_for_command("missing").is_empty());
        assert!(engine.confirmations_for_entity("order", "missing").is_empty());
        assert!(engine.latest_confirmation_for_command("missing").is_none());
        assert!(engine.latest_confirmation_for_entity("order", "missing").is_none());
    }

    #[test]
    fn drain_dead_letters_clears_engine_queue() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let dead_letter = DeadLetter::new(
            make_event("dead-letter"),
            crate::transport::PushRejection::new(uuid::Uuid::new_v4())
                .with_reason("invalid signature"),
        );
        engine.dead_letters.push(dead_letter.clone());

        let drained = engine.drain_dead_letters().unwrap();
        assert_eq!(drained, vec![dead_letter]);
        assert_eq!(engine.dead_letter_count(), 0);
    }

    #[test]
    fn dead_letter_lookup_supports_event_command_and_entity_queries() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let base = Utc::now();

        let first_event = make_event("dead-a").with_command_id("cmd-a");
        let first_id = first_event.id;
        let mut first = DeadLetter::new(
            first_event,
            crate::transport::PushRejection::new(first_id).with_reason("invalid signature"),
        );
        first.rejected_at = base;

        let second_event = make_event("dead-b").with_command_id("cmd-a");
        let second_id = second_event.id;
        let mut second = DeadLetter::new(
            second_event,
            crate::transport::PushRejection::new(second_id).with_reason("invalid schema"),
        );
        second.rejected_at = base + chrono::Duration::seconds(1);

        let third_event =
            SyncEvent::new("dead-c", "order", "ORD-2", json!({})).with_command_id("cmd-b");
        let third_id = third_event.id;
        let mut third = DeadLetter::new(
            third_event,
            crate::transport::PushRejection::new(third_id).with_reason("tenant mismatch"),
        );
        third.rejected_at = base + chrono::Duration::seconds(2);

        engine.dead_letters.push(first);
        engine.dead_letters.push(second);
        engine.dead_letters.push(third);

        let by_event = engine.dead_letter_for_event(first_id).unwrap();
        assert_eq!(by_event.event.command_id.as_deref(), Some("cmd-a"));
        assert_eq!(by_event.event.entity_id, "ORD-1");

        let by_command = engine.dead_letters_for_command("cmd-a");
        assert_eq!(by_command.len(), 2);
        assert_eq!(by_command[0].event.id, first_id);
        assert_eq!(by_command[1].event.id, second_id);

        let latest_by_command = engine.latest_dead_letter_for_command("cmd-a").unwrap();
        assert_eq!(latest_by_command.event.id, second_id);
        assert_eq!(latest_by_command.rejection.reason.as_deref(), Some("invalid schema"));

        let by_entity = engine.dead_letters_for_entity("order", "ORD-1");
        assert_eq!(by_entity.len(), 2);

        let latest_by_entity = engine.latest_dead_letter_for_entity("order", "ORD-1").unwrap();
        assert_eq!(latest_by_entity.event.id, second_id);

        assert!(engine.dead_letter_for_event(uuid::Uuid::new_v4()).is_none());
        assert!(engine.dead_letters_for_command("missing").is_empty());
        assert!(engine.dead_letters_for_entity("order", "missing").is_empty());
        assert!(engine.latest_dead_letter_for_command("missing").is_none());
        assert!(engine.latest_dead_letter_for_entity("order", "missing").is_none());
    }

    #[test]
    fn requeue_dead_letter_moves_event_back_to_outbox() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = make_event("dead-letter");
        let event_id = event.id;
        engine.dead_letters.push(DeadLetter::new(
            event,
            crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
        ));

        let sequence = engine.requeue_dead_letter(event_id).unwrap();
        assert_eq!(sequence, 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.dead_letter_count(), 0);
        let pending = engine.outbox.peek(10);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, event_id);
        assert_eq!(pending[0].local_sequence(), Some(1));
    }

    #[test]
    fn requeue_dead_letter_rejects_duplicate_pending_event() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = make_event("duplicate");
        let event_id = event.id;
        engine.record(event.clone()).unwrap();
        engine.dead_letters.push(DeadLetter::new(
            event,
            crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
        ));

        let err = engine.requeue_dead_letter(event_id).unwrap_err();
        assert!(matches!(err, SyncError::DuplicateEvent(_)));
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.dead_letter_count(), 1);
    }

    #[test]
    fn requeue_dead_letter_returns_not_found_for_unknown_id() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let err = engine.requeue_dead_letter(uuid::Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, SyncError::NotFound(_)));
    }

    #[test]
    fn discard_dead_letter_removes_entry() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = make_event("discarded");
        let event_id = event.id;
        engine.dead_letters.push(DeadLetter::new(
            event,
            crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
        ));

        let discarded = engine.discard_dead_letter(event_id).unwrap();
        assert_eq!(discarded.event.id, event_id);
        assert_eq!(engine.dead_letter_count(), 0);
    }

    #[test]
    fn drain_buffer() {
        let mut engine = SyncEngine::new(make_config()).unwrap();
        // Manually push to buffer via engine internals
        engine.buffer.push(make_event("buffered"));
        assert_eq!(engine.buffered_count(), 1);

        let drained = engine.drain_buffer();
        assert_eq!(drained.len(), 1);
        assert_eq!(engine.buffered_count(), 0);
    }

    #[test]
    fn engine_with_strategy() {
        let engine = SyncEngine::with_strategy(make_config(), ConflictStrategy::LocalWins).unwrap();
        assert_eq!(engine.resolver().strategy(), ConflictStrategy::LocalWins);
    }

    #[test]
    fn config_accessor() {
        let config = make_config();
        let engine = SyncEngine::new(config).unwrap();
        assert_eq!(engine.config().agent_id, "agent-1");
    }

    #[test]
    fn try_new_rejects_invalid_config() {
        let bad = SyncConfig::new("", "tenant", "store");
        assert!(SyncEngine::try_new(bad).is_err());
    }

    #[test]
    fn try_new_with_persistent_outbox_restores_pending_events() {
        let dir = tempdir().unwrap();
        let outbox_path = dir.path().join("sync-outbox.json");
        let path_str = outbox_path.to_string_lossy().to_string();

        {
            let mut engine = SyncEngine::try_new(
                SyncConfig::new("agent-1", "tenant-1", "store-1")
                    .with_outbox_path(path_str.clone()),
            )
            .unwrap();
            engine.record(make_event("persisted-a")).unwrap();
            engine.record(make_event("persisted-b")).unwrap();
            assert_eq!(engine.pending_count(), 2);
        }

        let engine = SyncEngine::try_new(
            SyncConfig::new("agent-1", "tenant-1", "store-1").with_outbox_path(path_str),
        )
        .unwrap();
        assert_eq!(engine.pending_count(), 2);
    }

    #[test]
    fn default_state_path_for_outbox_derives_sibling_file() {
        let derived = SyncEngine::default_state_path_for_outbox(std::path::Path::new(
            "/tmp/sync-outbox.json",
        ));
        assert_eq!(derived, std::path::PathBuf::from("/tmp/sync-outbox.state.json"));
    }

    #[tokio::test]
    async fn try_new_with_persistent_sync_state_restores_remote_progress() {
        #[derive(Debug)]
        struct PersistedStateTransport;

        #[async_trait::async_trait]
        impl Transport for PersistedStateTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| {
                        crate::transport::PushAcknowledgement::new(event.id, 7 + index as u64)
                    })
                    .collect();
                Ok(PushResult::accepted_only(events.len(), 9)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                if since == 0 {
                    Ok(PullResult {
                        events: vec![make_event("remote").with_remote_sequence(10)],
                        remote_head: 12,
                        has_more: true,
                    })
                } else {
                    Ok(PullResult { events: vec![], remote_head: 12, has_more: false })
                }
            }
        }

        let dir = tempdir().unwrap();
        let outbox_path = dir.path().join("sync-outbox.json");
        let state_path = dir.path().join("sync-state.json");
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1")
            .with_outbox_path(outbox_path.to_string_lossy().into_owned())
            .with_state_path(state_path.to_string_lossy().into_owned());

        {
            let mut engine = SyncEngine::try_new(config.clone()).unwrap();
            engine.record(make_event("local")).unwrap();
            engine.push(&PersistedStateTransport).await.unwrap();
            let result = engine.pull(&PersistedStateTransport).await.unwrap();
            assert_eq!(result.events.len(), 1);
            assert_eq!(engine.state().remote_head, 12);
            assert_eq!(engine.state().remote_cursor, 10);
            assert_eq!(engine.state().last_acknowledged_remote_sequence, Some(7));
            assert_eq!(engine.next_pull_cursor, Some(10));
        }

        let restored = SyncEngine::try_new(config).unwrap();
        assert_eq!(restored.state().local_head, 1);
        assert_eq!(restored.state().remote_head, 12);
        assert_eq!(restored.state().remote_cursor, 10);
        assert_eq!(restored.state().last_acknowledged_remote_sequence, Some(7));
        assert_eq!(restored.next_pull_cursor, Some(10));
        assert_eq!(restored.pending_count(), 0);
    }

    #[tokio::test]
    async fn try_new_with_persistent_sync_state_restores_dead_letters() {
        #[derive(Debug)]
        struct RejectingTransport;

        #[async_trait::async_trait]
        impl Transport for RejectingTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(0, 0).with_rejections(vec![
                    crate::transport::PushRejection::new(events[0].id)
                        .with_code("invalid_signature")
                        .with_reason("signature verification failed")
                        .with_retryable(false),
                ]))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
            }
        }

        let dir = tempdir().unwrap();
        let outbox_path = dir.path().join("sync-outbox.json");
        let state_path = dir.path().join("sync-state.json");
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1")
            .with_outbox_path(outbox_path.to_string_lossy().into_owned())
            .with_state_path(state_path.to_string_lossy().into_owned());

        let event_id = {
            let mut engine = SyncEngine::try_new(config.clone()).unwrap();
            let event = make_event("dead-letter");
            let event_id = event.id;
            engine.record(event).unwrap();
            engine.push(&RejectingTransport).await.unwrap();
            assert_eq!(engine.pending_count(), 0);
            assert_eq!(engine.dead_letter_count(), 1);
            event_id
        };

        let restored = SyncEngine::try_new(config).unwrap();
        assert_eq!(restored.pending_count(), 0);
        assert_eq!(restored.dead_letter_count(), 1);
        assert_eq!(restored.dead_letters()[0].event.id, event_id);
        assert_eq!(restored.dead_letters()[0].rejection.code.as_deref(), Some("invalid_signature"));
    }

    #[tokio::test]
    async fn requeue_dead_letter_persists_across_restart() {
        let dir = tempdir().unwrap();
        let outbox_path = dir.path().join("sync-outbox.json");
        let state_path = dir.path().join("sync-state.json");
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1")
            .with_outbox_path(outbox_path.to_string_lossy().into_owned())
            .with_state_path(state_path.to_string_lossy().into_owned());

        let event_id = {
            let mut engine = SyncEngine::try_new(config.clone()).unwrap();
            let event = make_event("requeued");
            let event_id = event.id;
            engine.dead_letters.push(DeadLetter::new(
                event,
                crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
            ));
            engine.persist_runtime_state().unwrap();

            let sequence = engine.requeue_dead_letter(event_id).unwrap();
            assert_eq!(sequence, 1);
            assert_eq!(engine.pending_count(), 1);
            assert_eq!(engine.dead_letter_count(), 0);
            event_id
        };

        let restored = SyncEngine::try_new(config).unwrap();
        assert_eq!(restored.pending_count(), 1);
        assert_eq!(restored.dead_letter_count(), 0);
        let pending = restored.outbox.peek(10);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, event_id);
    }

    #[tokio::test]
    async fn push_confirmations_persist_across_restart() {
        #[derive(Debug)]
        struct ReceiptAckTransport;

        #[async_trait::async_trait]
        impl Transport for ReceiptAckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| {
                        crate::transport::PushAcknowledgement::new(event.id, 30 + index as u64)
                            .with_receipt(format!("receipt-{}", 30 + index as u64))
                    })
                    .collect();
                Ok(PushResult::accepted_only(events.len(), 31)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 31, has_more: false })
            }
        }

        let dir = tempdir().unwrap();
        let state_path = dir.path().join("sync-state.json");
        let config = make_config().with_state_path(state_path.to_string_lossy().into_owned());

        let first_id = {
            let mut engine = SyncEngine::try_new(config.clone()).unwrap();
            let first = make_event("persisted-a").with_command_id("cmd-persist");
            let second = make_event("persisted-b");
            let first_id = first.id;
            engine.record(first).unwrap();
            engine.record(second).unwrap();
            engine.push(&ReceiptAckTransport).await.unwrap();
            assert_eq!(engine.confirmation_count(), 2);
            first_id
        };

        let restored = SyncEngine::try_new(config).unwrap();
        assert_eq!(restored.confirmation_count(), 2);
        assert_eq!(restored.confirmations()[0].event_id, first_id);
        assert_eq!(restored.confirmations()[0].command_id.as_deref(), Some("cmd-persist"));
        assert_eq!(restored.confirmations()[0].remote_sequence, 30);
        assert_eq!(restored.confirmations()[0].receipt.as_deref(), Some("receipt-30"));
    }

    #[tokio::test]
    async fn push_confirmation_retention_respects_capacity() {
        #[derive(Debug)]
        struct ReceiptAckTransport;

        #[async_trait::async_trait]
        impl Transport for ReceiptAckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| {
                        crate::transport::PushAcknowledgement::new(event.id, 40 + index as u64)
                    })
                    .collect();
                Ok(PushResult::accepted_only(events.len(), 42)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 42, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config().with_confirmation_capacity(2)).unwrap();
        let first = make_event("a");
        let second = make_event("b");
        let third = make_event("c");
        let second_id = second.id;
        let third_id = third.id;
        engine.record(first).unwrap();
        engine.record(second).unwrap();
        engine.record(third).unwrap();

        engine.push(&ReceiptAckTransport).await.unwrap();

        assert_eq!(engine.confirmation_count(), 2);
        assert_eq!(engine.confirmations()[0].event_id, second_id);
        assert_eq!(engine.confirmations()[0].remote_sequence, 41);
        assert_eq!(engine.confirmations()[1].event_id, third_id);
        assert_eq!(engine.confirmations()[1].remote_sequence, 42);
    }

    #[tokio::test]
    async fn push_confirmations_replace_stale_entries_for_matching_ids() {
        #[derive(Debug)]
        struct ReplacementTransport;

        #[async_trait::async_trait]
        impl Transport for ReplacementTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = vec![
                    crate::transport::PushAcknowledgement::new(events[0].id, 55)
                        .with_receipt("receipt-55"),
                ];
                Ok(PushResult::accepted_only(events.len(), 55)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 55, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = make_event("replace-me");
        let event_id = event.id;
        engine.confirmations.push(PushConfirmation::from_ack(
            &event.clone().with_local_sequence(1),
            &crate::transport::PushAcknowledgement::new(event_id, 11).with_receipt("receipt-11"),
        ));
        engine.record(event).unwrap();

        engine.push(&ReplacementTransport).await.unwrap();

        assert_eq!(engine.confirmation_count(), 1);
        let confirmation = engine.confirmation_for_event(event_id).unwrap();
        assert_eq!(confirmation.remote_sequence, 55);
        assert_eq!(confirmation.receipt.as_deref(), Some("receipt-55"));
    }

    #[tokio::test]
    async fn drain_confirmations_persists_clear_state() {
        #[derive(Debug)]
        struct ReceiptAckTransport;

        #[async_trait::async_trait]
        impl Transport for ReceiptAckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = events
                    .iter()
                    .map(|event| crate::transport::PushAcknowledgement::new(event.id, 50))
                    .collect();
                Ok(PushResult::accepted_only(events.len(), 50)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 50, has_more: false })
            }
        }

        let dir = tempdir().unwrap();
        let state_path = dir.path().join("sync-state.json");
        let config = make_config().with_state_path(state_path.to_string_lossy().into_owned());

        {
            let mut engine = SyncEngine::try_new(config.clone()).unwrap();
            engine.record(make_event("drained")).unwrap();
            engine.push(&ReceiptAckTransport).await.unwrap();
            assert_eq!(engine.confirmation_count(), 1);

            let drained = engine.drain_confirmations().unwrap();
            assert_eq!(drained.len(), 1);
            assert_eq!(engine.confirmation_count(), 0);
        }

        let restored = SyncEngine::try_new(config).unwrap();
        assert_eq!(restored.confirmation_count(), 0);
    }

    #[tokio::test]
    async fn push_respects_batch_size() {
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1").with_batch_size(2);
        let mut engine = SyncEngine::new(config).unwrap();
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
                Ok(PushResult::accepted_only(events.len(), new_head))
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

        let transport = MockHeadTransport { head: Arc::new(AtomicU64::new(0)) };

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let result = engine.push(&transport).await.unwrap();
        assert_eq!(result.remote_head, 2);
        assert_eq!(engine.state().remote_head, 2);
    }

    #[tokio::test]
    async fn push_tracks_acknowledged_remote_sequence() {
        #[derive(Debug)]
        struct AckTransport;

        #[async_trait::async_trait]
        impl Transport for AckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| {
                        crate::transport::PushAcknowledgement::new(event.id, 10 + index as u64)
                    })
                    .collect();
                Ok(PushResult::accepted_only(events.len(), 11)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 11, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let result = engine.push(&AckTransport).await.unwrap();
        assert_eq!(result.acknowledged_head(), Some(11));
        assert_eq!(engine.state().last_acknowledged_remote_sequence, Some(11));
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.confirmation_count(), 2);
        assert_eq!(engine.status().retained_confirmations, 2);
        assert_eq!(engine.confirmations()[0].local_sequence, Some(1));
        assert_eq!(engine.confirmations()[1].local_sequence, Some(2));
    }

    #[tokio::test]
    async fn push_retains_exact_confirmations_with_receipts() {
        #[derive(Debug)]
        struct ReceiptAckTransport;

        #[async_trait::async_trait]
        impl Transport for ReceiptAckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                let acknowledgements = events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| {
                        crate::transport::PushAcknowledgement::new(event.id, 20 + index as u64)
                            .with_receipt(format!("receipt-{}", 20 + index as u64))
                    })
                    .collect();
                Ok(PushResult::accepted_only(events.len(), 21)
                    .with_acknowledgements(acknowledgements))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 21, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let first = make_event("a").with_command_id("cmd-a");
        let second = make_event("b");
        let first_id = first.id;
        let second_id = second.id;
        engine.record(first).unwrap();
        engine.record(second).unwrap();

        engine.push(&ReceiptAckTransport).await.unwrap();

        let confirmations = engine.confirmations();
        assert_eq!(confirmations.len(), 2);
        assert_eq!(confirmations[0].event_id, first_id);
        assert_eq!(confirmations[0].command_id.as_deref(), Some("cmd-a"));
        assert_eq!(confirmations[0].local_sequence, Some(1));
        assert_eq!(confirmations[0].remote_sequence, 20);
        assert_eq!(confirmations[0].receipt.as_deref(), Some("receipt-20"));
        assert_eq!(confirmations[1].event_id, second_id);
        assert_eq!(confirmations[1].local_sequence, Some(2));
        assert_eq!(confirmations[1].remote_sequence, 21);
        assert_eq!(confirmations[1].receipt.as_deref(), Some("receipt-21"));
    }

    #[tokio::test]
    async fn push_acknowledgements_remove_non_prefix_events() {
        #[derive(Debug)]
        struct SparseAckTransport;

        #[async_trait::async_trait]
        impl Transport for SparseAckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(2, 12).with_acknowledgements(vec![
                    crate::transport::PushAcknowledgement::new(events[0].id, 10),
                    crate::transport::PushAcknowledgement::new(events[2].id, 12),
                ]))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 12, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let first = make_event("a");
        let second = make_event("b");
        let third = make_event("c");
        engine.record(first.clone()).unwrap();
        engine.record(second.clone()).unwrap();
        engine.record(third.clone()).unwrap();

        let result = engine.push(&SparseAckTransport).await.unwrap();
        assert_eq!(result.accepted, 2);
        assert_eq!(engine.pending_count(), 1);
        let remaining = engine.outbox.peek(10);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
        assert_eq!(engine.state().last_acknowledged_remote_sequence, Some(12));
    }

    #[tokio::test]
    async fn push_dead_letters_non_retryable_rejections() {
        #[derive(Debug)]
        struct RejectingTransport;

        #[async_trait::async_trait]
        impl Transport for RejectingTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(1, 20).with_rejections(vec![
                    crate::transport::PushRejection::new(events[2].id)
                        .with_code("invalid_signature")
                        .with_reason("signature verification failed")
                        .with_retryable(false),
                ]))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 20, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let first = make_event("a");
        let second = make_event("b");
        let third = make_event("c");
        engine.record(first.clone()).unwrap();
        engine.record(second.clone()).unwrap();
        engine.record(third.clone()).unwrap();

        let result = engine.push(&RejectingTransport).await.unwrap();
        assert_eq!(result.accepted, 1);
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.dead_letter_count(), 1);
        let remaining = engine.outbox.peek(10);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
        let dead_letter = &engine.dead_letters()[0];
        assert_eq!(dead_letter.event.id, third.id);
        assert_eq!(dead_letter.rejection.code.as_deref(), Some("invalid_signature"));
        assert_eq!(engine.status().dead_letters, 1);
    }

    #[tokio::test]
    async fn push_retryable_rejections_stay_pending() {
        #[derive(Debug)]
        struct RetryableRejectTransport;

        #[async_trait::async_trait]
        impl Transport for RetryableRejectTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(0, 0).with_rejections(vec![
                    crate::transport::PushRejection::new(events[0].id)
                        .with_reason("sequencer is rebalancing")
                        .with_retryable(true),
                ]))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let event = make_event("retryable");
        let event_id = event.id;
        engine.record(event).unwrap();

        let result = engine.push(&RetryableRejectTransport).await.unwrap();
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.dead_letter_count(), 0);
        assert_eq!(engine.outbox.peek(10)[0].id, event_id);
    }

    #[tokio::test]
    async fn push_rejects_prefix_overlap_without_acknowledgements() {
        #[derive(Debug)]
        struct InvalidRejectionTransport;

        #[async_trait::async_trait]
        impl Transport for InvalidRejectionTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(1, 10).with_rejections(vec![
                    crate::transport::PushRejection::new(events[0].id)
                        .with_reason("cannot overlap accepted prefix"),
                ]))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 10, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();

        let err = engine.push(&InvalidRejectionTransport).await.unwrap_err();
        assert!(matches!(err, SyncError::Transport(_)));
        assert_eq!(engine.pending_count(), 2);
        assert_eq!(engine.dead_letter_count(), 0);
    }

    #[tokio::test]
    async fn push_rejects_acknowledgement_for_unknown_event() {
        #[derive(Debug)]
        struct InvalidAckTransport;

        #[async_trait::async_trait]
        impl Transport for InvalidAckTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 99).with_acknowledgements(vec![
                    crate::transport::PushAcknowledgement::new(uuid::Uuid::new_v4(), 99),
                ]))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 99, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();

        let err = engine.push(&InvalidAckTransport).await.unwrap_err();
        assert!(matches!(err, SyncError::Transport(_)));
        assert_eq!(engine.pending_count(), 1);
        assert!(engine.state().last_push.is_none());
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

        let mut engine = SyncEngine::new(make_config()).unwrap();
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
                Ok(PushResult::accepted_only(1, 1))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 1, has_more: false })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        engine.record(make_event("a")).unwrap();
        engine.record(make_event("b")).unwrap();
        engine.record(make_event("c")).unwrap();

        let result = engine.push(&PartialAcceptTransport).await.unwrap();
        assert_eq!(result.accepted, 1);
        assert_eq!(engine.pending_count(), 2);
    }

    #[tokio::test]
    async fn push_returns_storage_error_when_ack_persist_fails() {
        #[derive(Debug)]
        struct AcceptAllTransport;

        #[async_trait::async_trait]
        impl Transport for AcceptAllTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), events.len() as u64))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.json");
        let config = make_config().with_outbox_path(path.to_string_lossy().into_owned());
        let mut engine = SyncEngine::new(config).unwrap();
        engine.record(make_event("a")).unwrap();

        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();

        let err = engine.push(&AcceptAllTransport).await.unwrap_err();
        assert!(matches!(err, SyncError::Storage(_)));
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.state().remote_head, 0);
        assert!(engine.state().last_push.is_none());
    }

    #[tokio::test]
    async fn pull_conflict_resolution() {
        /// Transport that returns events conflicting with local outbox.
        #[derive(Debug)]
        struct ConflictTransport;

        #[async_trait::async_trait]
        impl Transport for ConflictTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 10))
            }
            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                // Return an event for the same entity as the pending local event
                let remote_event =
                    SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                        .with_remote_sequence(5);
                Ok(PullResult { events: vec![remote_event], remote_head: 5, has_more: false })
            }
        }

        let mut engine =
            SyncEngine::with_strategy(make_config(), ConflictStrategy::RemoteWins).unwrap();
        engine
            .record(SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "local"})))
            .unwrap();

        let transport = ConflictTransport;
        let result = engine.pull(&transport).await.unwrap();
        assert_eq!(result.events.len(), 1);
        // RemoteWins removes conflicting local outbox events and keeps pulled event.
        assert_eq!(engine.buffered_count(), 1);
        assert_eq!(engine.pending_count(), 0);
    }

    #[tokio::test]
    async fn pull_conflict_remote_wins_only_drops_latest_pending_event() {
        #[derive(Debug)]
        struct ConflictTransport;

        #[async_trait::async_trait]
        impl Transport for ConflictTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 10))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                let remote_event =
                    SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                        .with_remote_sequence(5);
                Ok(PullResult { events: vec![remote_event], remote_head: 5, has_more: false })
            }
        }

        let mut engine =
            SyncEngine::with_strategy(make_config(), ConflictStrategy::RemoteWins).unwrap();
        engine
            .record(SyncEvent::new("order.note_added", "order", "ORD-1", json!({"note": "a"})))
            .unwrap();
        engine
            .record(SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "local"})))
            .unwrap();

        engine.pull(&ConflictTransport).await.unwrap();

        let pending: Vec<_> = engine.outbox.peek(10).into_iter().cloned().collect();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].event_type, "order.note_added");
        assert_eq!(engine.buffered_count(), 1);
    }

    #[tokio::test]
    async fn pull_conflict_local_wins_keeps_pending_and_skips_remote() {
        #[derive(Debug)]
        struct ConflictTransport;

        #[async_trait::async_trait]
        impl Transport for ConflictTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 10))
            }
            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                let remote_event =
                    SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                        .with_remote_sequence(5);
                Ok(PullResult { events: vec![remote_event], remote_head: 5, has_more: false })
            }
        }

        let mut engine =
            SyncEngine::with_strategy(make_config(), ConflictStrategy::LocalWins).unwrap();
        engine
            .record(SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "local"})))
            .unwrap();

        let result = engine.pull(&ConflictTransport).await.unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.buffered_count(), 0);
    }

    #[tokio::test]
    async fn full_sync_paginates_pull_until_complete() {
        #[derive(Debug)]
        struct PagingTransport {
            pulls: Arc<AtomicU64>,
            since_args: Arc<Mutex<Vec<u64>>>,
        }

        #[async_trait::async_trait]
        impl Transport for PagingTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 0))
            }

            async fn pull_events(
                &self,
                since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                self.since_args.lock().unwrap().push(since);
                let call = self.pulls.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    Ok(PullResult {
                        events: vec![
                            SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                                .with_remote_sequence(1),
                        ],
                        // Simulate remote_head as global head watermark, not page cursor.
                        remote_head: 999,
                        has_more: true,
                    })
                } else {
                    Ok(PullResult {
                        events: vec![
                            SyncEvent::new("order.updated", "order", "ORD-2", json!({}))
                                .with_remote_sequence(2),
                        ],
                        remote_head: 999,
                        has_more: false,
                    })
                }
            }
        }

        let since_args = Arc::new(Mutex::new(Vec::new()));
        let transport = PagingTransport {
            pulls: Arc::new(AtomicU64::new(0)),
            since_args: Arc::clone(&since_args),
        };
        let mut engine = SyncEngine::new(make_config()).unwrap();
        let (_push_result, pull_result) = engine.full_sync(&transport).await.unwrap();

        assert_eq!(pull_result.events.len(), 2);
        assert!(!pull_result.has_more);
        assert_eq!(engine.buffered_count(), 2);
        assert_eq!(engine.state().remote_head, 999);
        assert_eq!(transport.pulls.load(Ordering::SeqCst), 2);
        assert_eq!(&*since_args.lock().unwrap(), &[0, 1]);
    }

    #[tokio::test]
    async fn pull_cursor_does_not_advance_from_push_remote_head() {
        #[derive(Debug)]
        struct HeadSkewTransport {
            since_args: Arc<Mutex<Vec<u64>>>,
        }

        #[async_trait::async_trait]
        impl Transport for HeadSkewTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 1000))
            }

            async fn pull_events(
                &self,
                since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                self.since_args.lock().unwrap().push(since);
                Ok(PullResult {
                    events: vec![
                        SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                            .with_remote_sequence(7),
                    ],
                    remote_head: 1000,
                    has_more: false,
                })
            }
        }

        let since_args = Arc::new(Mutex::new(Vec::new()));
        let transport = HeadSkewTransport { since_args: Arc::clone(&since_args) };
        let mut engine = SyncEngine::new(make_config()).unwrap();

        engine.record(make_event("local.pending")).unwrap();
        let (_push, pull) = engine.full_sync(&transport).await.unwrap();

        assert_eq!(pull.events.len(), 1);
        assert_eq!(&*since_args.lock().unwrap(), &[0]);
    }

    #[tokio::test]
    async fn pull_tracks_observed_cursor_separately_from_continuation_cursor() {
        #[derive(Debug)]
        struct ContinuationCursorTransport;

        #[async_trait::async_trait]
        impl Transport for ContinuationCursorTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 0))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                Ok(PullResult { events: vec![], remote_head: 12, has_more: true })
            }

            async fn pull_events_page(
                &self,
                since: u64,
                _limit: usize,
            ) -> Result<PullPage, SyncError> {
                assert_eq!(since, 0);
                Ok(PullPage {
                    result: PullResult {
                        events: vec![
                            SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                                .with_remote_sequence(10),
                        ],
                        remote_head: 12,
                        has_more: true,
                    },
                    next_cursor: Some(11),
                    observed_cursor: Some(10),
                })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let result = engine.pull(&ContinuationCursorTransport).await.unwrap();
        assert_eq!(result.remote_head, 12);
        assert_eq!(engine.state().remote_cursor, 10);
        assert_eq!(engine.next_pull_cursor, Some(11));
    }

    #[tokio::test]
    async fn pull_errors_when_has_more_but_cursor_cannot_advance() {
        #[derive(Debug)]
        struct StalledPagingTransport;

        #[async_trait::async_trait]
        impl Transport for StalledPagingTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 0))
            }

            async fn pull_events(
                &self,
                _since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                // has_more=true but no event sequence progress, so default cursor
                // derivation cannot safely continue.
                Ok(PullResult {
                    events: vec![
                        SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                            .with_remote_sequence(0),
                    ],
                    remote_head: 100,
                    has_more: true,
                })
            }
        }

        let mut engine = SyncEngine::new(make_config()).unwrap();
        let err = engine.pull(&StalledPagingTransport).await.unwrap_err();
        assert!(matches!(err, SyncError::Transport(_)));
    }

    proptest! {
        #[test]
        fn resolve_next_cursor_enforces_monotonic_progress(
            since in 0u64..20_000,
            transport_cursor in prop::option::of(0u64..20_000),
            sequences in prop::collection::vec(0u64..20_000, 0..64),
        ) {
            let events: Vec<SyncEvent> = sequences
                .iter()
                .enumerate()
                .map(|(i, seq)| {
                    SyncEvent::new(
                        format!("evt-{i}"),
                        "entity",
                        format!("id-{i}"),
                        json!({ "s": seq }),
                    )
                    .with_remote_sequence(*seq)
                })
                .collect();

            let result = SyncEngine::resolve_next_cursor(since, &events, true, transport_cursor);
            if let Some(cursor) = transport_cursor {
                if cursor > since {
                    prop_assert_eq!(result.unwrap(), Some(cursor));
                } else {
                    prop_assert!(matches!(result, Err(SyncError::Transport(_))));
                }
            } else if let Some(expected) = derive_next_cursor(since, &events) {
                prop_assert_eq!(result.unwrap(), Some(expected));
            } else {
                prop_assert!(matches!(result, Err(SyncError::Transport(_))));
            }
        }
    }

    proptest! {
        #[test]
        fn resolve_next_cursor_returns_none_when_transport_signals_no_more(
            since in 0u64..20_000,
            transport_cursor in prop::option::of(0u64..20_000),
            sequences in prop::collection::vec(0u64..20_000, 0..64),
        ) {
            let events: Vec<SyncEvent> = sequences
                .iter()
                .enumerate()
                .map(|(i, seq)| {
                    SyncEvent::new(
                        format!("evt-{i}"),
                        "entity",
                        format!("id-{i}"),
                        json!({}),
                    )
                    .with_remote_sequence(*seq)
                })
                .collect();

            let result = SyncEngine::resolve_next_cursor(since, &events, false, transport_cursor)
                .unwrap();
            prop_assert_eq!(result, None);
        }
    }

    #[test]
    fn engine_debug() {
        let engine = SyncEngine::new(make_config()).unwrap();
        let debug = format!("{engine:?}");
        assert!(debug.contains("SyncEngine"));
    }
}
