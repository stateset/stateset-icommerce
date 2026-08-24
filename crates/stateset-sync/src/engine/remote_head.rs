//! Canonical remote head refresh and snapshotting.

use super::*;

impl SyncEngine {
    pub(super) fn remote_head_snapshot(&self) -> RemoteHead {
        let mut head = RemoteHead::new(self.state.remote_head);
        if let Some(state_root) = self.state.remote_state_root.clone() {
            head = head.with_state_root(state_root);
        }
        if let Some(commitment_id) = self.state.last_commitment_id.clone() {
            head = head.with_last_commitment_id(commitment_id);
        }
        if let Some(manifest) = self
            .state
            .last_commitment_id
            .as_deref()
            .and_then(|commitment_id| self.verified_commitment_manifest(commitment_id))
            .cloned()
        {
            head = head.with_commitment_manifest(CommitmentManifest {
                commitment_id: manifest.commitment_id,
                previous_commitment_id: manifest.previous_commitment_id,
                state_root: manifest.state_root,
                remote_head: manifest.remote_head,
                signer_id: manifest.signer_id,
                signature_scheme: manifest.signature_scheme,
                signer_public_key: Some(manifest.signer_public_key),
                signature: Some(manifest.signature),
                issued_at: manifest.issued_at,
            });
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
    /// head state, [`SyncError::Trust`] if an included commitment manifest
    /// fails signature verification or the configured
    /// [`crate::CommitmentTrustPolicy`] (by default, manifests are rejected
    /// until signer keys are pinned), or [`SyncError::Storage`] if persisting
    /// the updated runtime state fails.
    pub async fn refresh_remote_head(
        &mut self,
        transport: &dyn Transport,
    ) -> Result<RemoteHead, SyncError> {
        let observed = transport.fetch_head().await?;
        let previous_state = self.state.clone();
        let previous_manifests = self.manifests.clone();
        let previous_tofu_signer_pins = self.tofu_signer_pins.clone();
        let mut next_state = self.state.clone();

        match observed.remote_head.cmp(&previous_state.remote_head) {
            std::cmp::Ordering::Greater => {
                next_state.remote_head = observed.remote_head;
                next_state.remote_state_root = observed.state_root.clone();
                next_state.last_commitment_id = observed.last_commitment_id.clone();
            }
            std::cmp::Ordering::Equal => {
                if let Some(state_root) = observed.state_root.clone() {
                    next_state.remote_state_root = Some(state_root);
                }
                if let Some(commitment_id) = observed.last_commitment_id.clone() {
                    next_state.last_commitment_id = Some(commitment_id);
                }
            }
            std::cmp::Ordering::Less => {}
        }

        let metadata_updated = observed.remote_head >= previous_state.remote_head
            && (observed.state_root.is_some()
                || observed.last_commitment_id.is_some()
                || observed.commitment_manifest.is_some());
        if metadata_updated {
            match observed.commitment_manifest.as_ref() {
                Some(manifest) => {
                    let verified =
                        self.verify_remote_manifest_against_state(manifest, &next_state)?;
                    self.upsert_verified_commitment_manifest(verified);
                }
                None if self.config.commitment_trust.require_manifest => {
                    return Err(SyncError::Trust(format!(
                        "remote head {} included commitment metadata (state_root / commitment id) \
                         but no signed manifest; the default trust policy fails closed because an \
                         unsigned state root has no authenticity. Configure the sequencer to \
                         publish a signed manifest, or explicitly opt out with \
                         `SyncConfig::with_unauthenticated_remote_head_allowed()` in a trusted \
                         environment",
                        observed.remote_head
                    )));
                }
                None => {
                    // Opt-out is active: the operator accepted unauthenticated
                    // remote metadata. Warn loudly on every refresh so this is
                    // never silently forgotten in production.
                    eprintln!(
                        "WARN stateset_sync: recording UNAUTHENTICATED remote head {} \
                         (state_root / commitment id with no signed manifest); commitment-signer \
                         pinning is bypassed for this head because \
                         `with_unauthenticated_remote_head_allowed()` is set",
                        observed.remote_head
                    );
                }
            }
        }

        self.state = next_state;
        let head = self.remote_head_snapshot();
        if let Err(error) = self.persist_runtime_state() {
            self.state = previous_state;
            self.manifests = previous_manifests;
            self.tofu_signer_pins = previous_tofu_signer_pins;
            return Err(error);
        }
        Ok(head)
    }
}
