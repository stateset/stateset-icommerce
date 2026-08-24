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
        ) = if let Some(snapshot) = snapshot {
            (
                snapshot.state,
                snapshot.next_pull_cursor,
                snapshot.dead_letters,
                snapshot.confirmations,
                snapshot.attestations,
                snapshot.manifests,
                snapshot.tofu_signer_pins,
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
            attestations,
            manifests,
            tofu_signer_pins,
            initialized: true,
        })
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
}
