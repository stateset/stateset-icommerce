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
}
