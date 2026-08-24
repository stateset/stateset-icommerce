//! Query and drain retained push confirmations.

use super::*;

impl SyncEngine {
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
}
