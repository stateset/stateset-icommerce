//! Durable sync-state snapshot: path resolution, load, and atomic persist.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SyncEngineSnapshot {
    pub(super) state: SyncState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) next_pull_cursor: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) dead_letters: Vec<DeadLetter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) confirmations: Vec<PushConfirmation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) attestations: Vec<CommandAttestation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) manifests: Vec<VerifiedCommitmentManifest>,
    /// Signer keys pinned by trust-on-first-use, keyed by signer id
    /// (normalized lowercase hex, no `0x` prefix).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) tofu_signer_pins: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) pending_outbox_removals: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) buffered_events: Vec<SyncEvent>,
}

impl SyncEngine {
    pub(super) fn resolved_state_path(config: &SyncConfig) -> Option<PathBuf> {
        if let Some(path) = config.state_path.as_deref() {
            return Some(PathBuf::from(path));
        }
        config
            .outbox_path
            .as_deref()
            .map(|path| Self::default_state_path_for_outbox(Path::new(path)))
    }

    pub(super) fn default_state_path_for_outbox(path: &Path) -> PathBuf {
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

    pub(super) fn load_state_snapshot(
        path: &Path,
    ) -> Result<Option<SyncEngineSnapshot>, SyncError> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(SyncError::Storage(format!(
                    "read sync-state snapshot failed: {error}"
                )));
            }
        };
        let snapshot = serde_json::from_str(&contents)?;
        Ok(Some(snapshot))
    }

    pub(super) fn persist_runtime_state(&self) -> Result<(), SyncError> {
        let Some(path) = self.state_path.as_deref() else {
            return Ok(());
        };

        let snapshot = SyncEngineSnapshot {
            state: self.state.clone(),
            next_pull_cursor: self.next_pull_cursor,
            dead_letters: self.dead_letters.clone(),
            confirmations: self.confirmations.clone(),
            attestations: self.attestations.clone(),
            manifests: self.manifests.clone(),
            tofu_signer_pins: self.tofu_signer_pins.clone(),
            pending_outbox_removals: self.pending_outbox_removals.clone(),
            buffered_events: self.buffer.snapshot(),
        };
        crate::snapshot::write(path, &snapshot)
    }

    pub(super) fn finish_outbox_cleanup(&mut self) -> Result<(), SyncError> {
        if self.pending_outbox_removals.is_empty() {
            return Ok(());
        }
        // Persist again when retrying: a failed cleanup may have left the
        // visible state snapshot without a journal while memory retains it.
        self.persist_runtime_state()?;
        if let Some(path) = &self.state_path {
            crate::snapshot::sync_parent(path)?;
        }
        let removals: HashSet<_> = self.pending_outbox_removals.iter().copied().collect();
        let result = self.outbox.try_retain(|event| !removals.contains(&event.id));
        self.state.pending_count = self.outbox.count();
        result?;
        self.outbox.sync_persistence()?;

        let journal = std::mem::take(&mut self.pending_outbox_removals);
        let result = self
            .persist_runtime_state()
            .and_then(|()| self.state_path.as_deref().map_or(Ok(()), crate::snapshot::sync_parent));
        if result.is_err() {
            // Reapplying exact-id removals is safe. Keep them until cleanup is
            // durably acknowledged, including failures after file replacement.
            self.pending_outbox_removals = journal;
        }
        result
    }
}
