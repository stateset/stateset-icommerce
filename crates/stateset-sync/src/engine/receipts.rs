//! Unified kernel receipts and command-level convergence views.

use super::*;

impl SyncEngine {
    /// Return a unified receipt view spanning pending, confirmed, and rejected local events.
    #[must_use]
    pub fn kernel_receipts(&self) -> Vec<KernelReceipt> {
        let mut receipts: Vec<_> =
            self.pending_events().into_iter().map(KernelReceipt::from_pending).collect();
        receipts.extend(self.confirmations.iter().map(KernelReceipt::from_confirmation));
        receipts.extend(self.dead_letters.iter().map(KernelReceipt::from_dead_letter));
        receipts.sort_by_key(KernelReceipt::ordering_key);
        receipts
    }

    /// Return the unified receipt for a local event id, if known.
    #[must_use]
    pub fn kernel_receipt_for_event(&self, event_id: Uuid) -> Option<KernelReceipt> {
        self.kernel_receipts().into_iter().find(|receipt| receipt.event_id == event_id)
    }

    /// Return all unified receipts associated with a command id.
    #[must_use]
    pub fn kernel_receipts_for_command(&self, command_id: &str) -> Vec<KernelReceipt> {
        self.kernel_receipts()
            .into_iter()
            .filter(|receipt| receipt.command_id.as_deref() == Some(command_id))
            .collect()
    }

    /// Return all unified receipts for an entity identity.
    #[must_use]
    pub fn kernel_receipts_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Vec<KernelReceipt> {
        self.kernel_receipts()
            .into_iter()
            .filter(|receipt| receipt.entity_type == entity_type && receipt.entity_id == entity_id)
            .collect()
    }

    /// Return the latest unified receipt associated with a command id.
    #[must_use]
    pub fn latest_kernel_receipt_for_command(&self, command_id: &str) -> Option<KernelReceipt> {
        self.kernel_receipts_for_command(command_id).into_iter().last()
    }

    /// Return the latest unified receipt for an entity identity.
    #[must_use]
    pub fn latest_kernel_receipt_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<KernelReceipt> {
        self.kernel_receipts_for_entity(entity_type, entity_id).into_iter().last()
    }

    /// Return command-level convergence snapshots for every command currently retained in the kernel.
    #[must_use]
    pub fn command_convergences(&self) -> Vec<CommandConvergence> {
        let mut receipts_by_command: HashMap<String, Vec<KernelReceipt>> = HashMap::new();
        for receipt in self.kernel_receipts() {
            let Some(command_id) = receipt.command_id.clone() else {
                continue;
            };
            receipts_by_command.entry(command_id).or_default().push(receipt);
        }
        let verified_manifest = self
            .state
            .last_commitment_id
            .as_deref()
            .and_then(|commitment_id| self.verified_commitment_manifest(commitment_id));

        let mut convergences: Vec<_> = receipts_by_command
            .into_iter()
            .map(|(command_id, receipts)| {
                CommandConvergence::from_receipts(
                    command_id,
                    receipts,
                    &self.state,
                    verified_manifest,
                )
            })
            .collect();
        convergences.sort_by(|left, right| left.command_id.cmp(&right.command_id));
        convergences
    }

    /// Return the command-level convergence snapshot for a specific command id, if retained.
    #[must_use]
    pub fn command_convergence(&self, command_id: &str) -> Option<CommandConvergence> {
        let receipts = self.kernel_receipts_for_command(command_id);
        if receipts.is_empty() {
            None
        } else {
            let verified_manifest = self
                .state
                .last_commitment_id
                .as_deref()
                .and_then(|commitment_id| self.verified_commitment_manifest(commitment_id));
            Some(CommandConvergence::from_receipts(
                command_id,
                receipts,
                &self.state,
                verified_manifest,
            ))
        }
    }
}
