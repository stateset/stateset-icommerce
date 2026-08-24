//! Commitment-manifest trust policy, verification, and command attestations.

use super::*;

impl SyncEngine {
    pub(super) fn trim_attestations_to_capacity(&mut self) {
        let capacity = self.config.resolved_confirmation_capacity();
        if self.attestations.len() > capacity {
            let overflow = self.attestations.len() - capacity;
            self.attestations.drain(0..overflow);
        }
    }

    pub(super) fn trim_manifests_to_capacity(&mut self) {
        let capacity = self.config.resolved_confirmation_capacity();
        if self.manifests.len() > capacity {
            let overflow = self.manifests.len() - capacity;
            self.manifests.drain(0..overflow);
        }
    }

    pub(super) fn upsert_verified_commitment_manifest(
        &mut self,
        verified: VerifiedCommitmentManifest,
    ) {
        self.manifests.retain(|existing| existing.commitment_id != verified.commitment_id);
        self.manifests.push(verified);
        self.trim_manifests_to_capacity();
    }

    /// Normalize a hex-encoded signer key for comparison: trimmed, lowercase,
    /// without a `0x` prefix.
    pub(super) fn normalize_signer_key(key: &str) -> String {
        let lowered = key.trim().to_ascii_lowercase();
        lowered.strip_prefix("0x").map_or(lowered.clone(), ToOwned::to_owned)
    }

    /// Enforce the configured [`crate::CommitmentTrustPolicy`] against a
    /// signature-verified manifest.
    ///
    /// Signature verification alone is self-certifying (the manifest supplies
    /// its own public key), so this gate is what binds the manifest signer to
    /// an operator trust decision:
    ///
    /// - the optional `trusted_signer_ids` allowlist is checked in all modes;
    /// - [`SignerTrustMode::PinnedKeys`] (default) requires the signer key to
    ///   be pinned in `trusted_signer_public_keys`; with no pinned keys the
    ///   manifest is rejected (fail closed);
    /// - [`SignerTrustMode::TrustOnFirstUse`] durably pins the first verified
    ///   signer key (persisted with the sync-state snapshot) and rejects any
    ///   other key thereafter;
    /// - [`SignerTrustMode::AllowAnySigner`] skips the key check entirely and
    ///   must be opted into explicitly.
    ///
    /// May record a new trust-on-first-use pin in memory; callers persist it
    /// via `persist_runtime_state` and roll it back on persistence failure.
    pub(super) fn enforce_commitment_trust_policy(
        &mut self,
        verified: &VerifiedCommitmentManifest,
    ) -> Result<(), SyncError> {
        let trusted_signer_ids = &self.config.commitment_trust.trusted_signer_ids;
        if !trusted_signer_ids.is_empty()
            && !trusted_signer_ids.iter().any(|signer_id| signer_id == &verified.signer_id)
        {
            return Err(SyncError::Trust(format!(
                "commitment manifest `{}` signer `{}` is not in the trusted signer allowlist",
                verified.commitment_id, verified.signer_id
            )));
        }

        let manifest_key = Self::normalize_signer_key(&verified.signer_public_key);
        let has_pinned_keys = !self.config.commitment_trust.trusted_signer_public_keys.is_empty();
        let key_is_pinned = self
            .config
            .commitment_trust
            .trusted_signer_public_keys
            .iter()
            .any(|public_key| Self::normalize_signer_key(public_key) == manifest_key);

        match self.config.commitment_trust.signer_trust {
            SignerTrustMode::AllowAnySigner => Ok(()),
            SignerTrustMode::PinnedKeys => {
                if key_is_pinned {
                    Ok(())
                } else if has_pinned_keys {
                    Err(SyncError::Trust(format!(
                        "commitment manifest `{}` public key is not in the trusted key allowlist",
                        verified.commitment_id
                    )))
                } else {
                    Err(SyncError::Trust(format!(
                        "commitment manifest `{}` rejected: no commitment signer keys are pinned, \
                         and the default trust policy fails closed because the manifest supplies \
                         its own public key; pin the sequencer key with \
                         `SyncConfig::with_trusted_commitment_signer_public_key(..)`, or \
                         explicitly opt into `with_commitment_trust_on_first_use()` or \
                         `with_allow_any_commitment_signer()`",
                        verified.commitment_id
                    )))
                }
            }
            SignerTrustMode::TrustOnFirstUse => {
                if key_is_pinned {
                    return Ok(());
                }
                if let Some(pinned_key) = self.tofu_signer_pins.get(&verified.signer_id) {
                    if *pinned_key == manifest_key {
                        return Ok(());
                    }
                    return Err(SyncError::Trust(format!(
                        "commitment manifest `{}` signer `{}` presented a key that differs from \
                         the key pinned on first use; refusing signer key rotation — pin the new \
                         key explicitly with \
                         `SyncConfig::with_trusted_commitment_signer_public_key(..)` if the \
                         rotation is legitimate",
                        verified.commitment_id, verified.signer_id
                    )));
                }
                if self.tofu_signer_pins.is_empty() {
                    self.tofu_signer_pins.insert(verified.signer_id.clone(), manifest_key);
                    return Ok(());
                }
                Err(SyncError::Trust(format!(
                    "commitment manifest `{}` signer `{}` is unknown and a first-use signer key \
                     is already pinned; pin additional keys explicitly with \
                     `SyncConfig::with_trusted_commitment_signer_public_key(..)`",
                    verified.commitment_id, verified.signer_id
                )))
            }
        }
    }

    pub(super) fn verify_remote_manifest_against_state(
        &mut self,
        manifest: &CommitmentManifest,
        state: &SyncState,
    ) -> Result<VerifiedCommitmentManifest, SyncError> {
        let verified =
            verify_commitment_manifest_against_state(manifest, state).map_err(|error| {
                SyncError::Trust(format!(
                    "remote commitment manifest `{}` failed verification: {error}",
                    manifest.commitment_id
                ))
            })?;
        self.enforce_commitment_trust_policy(&verified)?;
        Ok(verified)
    }

    /// Return all verified commitment manifests retained by the engine.
    #[must_use]
    pub fn verified_commitment_manifests(&self) -> &[VerifiedCommitmentManifest] {
        &self.manifests
    }

    /// Return the verified commitment manifest for a specific commitment id, if retained.
    #[must_use]
    pub fn verified_commitment_manifest(
        &self,
        commitment_id: &str,
    ) -> Option<&VerifiedCommitmentManifest> {
        self.manifests.iter().find(|manifest| manifest.commitment_id == commitment_id)
    }

    /// Verify and retain a signed commitment manifest against the current remote state.
    ///
    /// # Trust model
    ///
    /// The manifest signature is checked against the manifest's own embedded
    /// signer key, which alone proves nothing about who produced it. The
    /// signer key is therefore additionally checked against the configured
    /// [`crate::CommitmentTrustPolicy`]. By default
    /// ([`SignerTrustMode::PinnedKeys`] with no pinned keys) every manifest is
    /// rejected with [`ManifestVerificationError::TrustPolicyViolation`] until
    /// the operator pins signer keys or explicitly opts into
    /// trust-on-first-use or allow-any-signer.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestVerificationError`] if the manifest signature, trust
    /// policy, state binding, or durable persistence fails.
    pub fn verify_commitment_manifest(
        &mut self,
        manifest: CommitmentManifest,
    ) -> Result<VerifiedCommitmentManifest, ManifestVerificationError> {
        let verified = verify_commitment_manifest_against_state(&manifest, &self.state)?;
        let previous_tofu_signer_pins = self.tofu_signer_pins.clone();
        if let Err(error) = self.enforce_commitment_trust_policy(&verified) {
            return Err(ManifestVerificationError::TrustPolicyViolation {
                commitment_id: manifest.commitment_id,
                reason: error.to_string(),
            });
        }

        let previous = self.manifests.clone();
        self.upsert_verified_commitment_manifest(verified.clone());

        if let Err(error) = self.persist_runtime_state() {
            self.manifests = previous;
            self.tofu_signer_pins = previous_tofu_signer_pins;
            return Err(ManifestVerificationError::PersistenceFailed {
                commitment_id: manifest.commitment_id,
                reason: error.to_string(),
            });
        }

        Ok(verified)
    }

    /// Return all verified command attestations retained by the engine.
    #[must_use]
    pub fn command_attestations(&self) -> &[CommandAttestation] {
        &self.attestations
    }

    /// Return the verified command attestation for a specific command id, if retained.
    #[must_use]
    pub fn command_attestation(&self, command_id: &str) -> Option<&CommandAttestation> {
        self.attestations.iter().find(|attestation| attestation.command_id == command_id)
    }

    /// Verify and retain a command inclusion proof against current kernel receipts and remote state.
    ///
    /// # Errors
    ///
    /// Returns [`AttestationError`] if the proof is inconsistent with retained receipts or the
    /// currently known remote commitment metadata.
    pub fn attest_command(
        &mut self,
        proof: CommandInclusionProof,
    ) -> Result<CommandAttestation, AttestationError> {
        let receipts = self.kernel_receipts_for_command(&proof.command_id);
        let mut attestation = verify_command_inclusion_proof(&proof, &receipts, &self.state)?;
        if let Some(commitment_id) = proof.commitment_id.as_deref() {
            if let Some(manifest) = self.verified_commitment_manifest(commitment_id) {
                attestation.manifest_signer_id = Some(manifest.signer_id.clone());
                attestation.manifest_verified_at = Some(manifest.verified_at);
            }
        }

        let previous = self.attestations.clone();
        self.attestations.retain(|existing| existing.command_id != attestation.command_id);
        self.attestations.push(attestation.clone());
        self.trim_attestations_to_capacity();
        if let Err(error) = self.persist_runtime_state() {
            self.attestations = previous;
            return Err(AttestationError::InvalidProofShape {
                command_id: proof.command_id,
                reason: format!("persist verified attestation failed: {error}"),
            });
        }

        Ok(attestation)
    }
}
