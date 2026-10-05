//! Construction, status reporting, and simple accessors for [`SyncEngine`].

use super::*;

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
        let (
            mut state,
            next_pull_cursor,
            dead_letters,
            mut confirmations,
            mut attestations,
            mut manifests,
            tofu_signer_pins,
            pending_outbox_removals,
            buffered_events,
        ) = if let Some(snapshot) = snapshot {
            (
                snapshot.state,
                snapshot.next_pull_cursor,
                snapshot.dead_letters,
                snapshot.confirmations,
                snapshot.attestations,
                snapshot.manifests,
                snapshot.tofu_signer_pins,
                snapshot.pending_outbox_removals,
                snapshot.buffered_events,
            )
        } else {
            (
                SyncState::default(),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                BTreeMap::new(),
                Vec::new(),
                Vec::new(),
            )
        };
        state.local_head = state.local_head.max(outbox.next_sequence().saturating_sub(1));
        state.pending_count = outbox.count();
        let confirmation_capacity = config.resolved_confirmation_capacity();
        if confirmations.len() > confirmation_capacity {
            let overflow = confirmations.len() - confirmation_capacity;
            confirmations.drain(0..overflow);
        }
        if attestations.len() > confirmation_capacity {
            let overflow = attestations.len() - confirmation_capacity;
            attestations.drain(0..overflow);
        }
        if manifests.len() > confirmation_capacity {
            let overflow = manifests.len() - confirmation_capacity;
            manifests.drain(0..overflow);
        }

        let mut engine = Self {
            config,
            state,
            outbox,
            buffer: EventBuffer::from_snapshot(buffer_capacity, buffered_events),
            resolver: ConflictResolver::new(strategy),
            state_path,
            next_pull_cursor,
            dead_letters,
            confirmations,
            attestations,
            manifests,
            tofu_signer_pins,
            pending_outbox_removals,
            initialized: true,
        };
        engine.finish_outbox_cleanup()?;
        Ok(engine)
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

    /// Snapshot all buffered pulled events without draining them.
    #[must_use]
    pub fn buffered_events(&self) -> Vec<SyncEvent> {
        self.buffer.snapshot()
    }

    /// Compatibility helper to drain the pull buffer. On persistence failure,
    /// returns no events and preserves the buffer. Prefer [`Self::try_drain_buffer`]
    /// to observe failures, or acknowledge events only after applying them.
    pub fn drain_buffer(&mut self) -> Vec<SyncEvent> {
        self.try_drain_buffer().unwrap_or_default()
    }

    /// Remove and return all buffered events, persisting the removal first.
    /// This transfers responsibility to the caller; for crash-safe application
    /// processing, read [`Self::buffered_events`] and then call
    /// [`Self::acknowledge_buffered_events`] after committing application changes.
    ///
    /// # Errors
    /// Returns a storage error without draining events if snapshot writing fails.
    pub fn try_drain_buffer(&mut self) -> Result<Vec<SyncEvent>, SyncError> {
        let previous = self.buffer.clone();
        let events = self.buffer.drain_all();
        if let Err(error) = self.persist_runtime_state() {
            self.buffer = previous;
            return Err(error);
        }
        Ok(events)
    }

    /// Acknowledge events only after their application changes have committed.
    /// Removes matching ids durably and returns their count. Unknown or repeated
    /// ids are harmless, making acknowledgement retries idempotent.
    ///
    /// # Errors
    /// Returns a storage error if acknowledgement persistence fails. Applications
    /// must deduplicate event ids: a crash between application commit and this
    /// acknowledgement can replay an event.
    pub fn acknowledge_buffered_events(&mut self, event_ids: &[Uuid]) -> Result<usize, SyncError> {
        let ids: HashSet<_> = event_ids.iter().copied().collect();
        let previous = self.buffer.clone();
        let remaining =
            self.buffer.snapshot().into_iter().filter(|event| !ids.contains(&event.id)).collect();
        self.buffer = EventBuffer::from_snapshot(self.buffer.capacity(), remaining);
        let removed = previous.len() - self.buffer.len();
        if let Err(error) = self.persist_runtime_state() {
            self.buffer = previous;
            return Err(error);
        }
        if let Some(path) = &self.state_path {
            crate::snapshot::sync_parent(path)?;
        }
        Ok(removed)
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
}
