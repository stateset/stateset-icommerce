//! Local event recording, including transaction-kernel policy/budget gating.

use super::*;

impl SyncEngine {
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

    /// Execute a local transaction-kernel request and record the resulting event.
    ///
    /// This applies policy and budget enforcement before the event reaches the
    /// local outbox. On success it returns the pending kernel receipt for the
    /// newly recorded event.
    ///
    /// # Errors
    ///
    /// Returns [`KernelExecutionError`] when local policy or budget checks fail
    /// or when the underlying outbox record operation fails.
    pub fn record_kernel_transaction(
        &mut self,
        transaction: KernelTransaction,
    ) -> Result<KernelReceipt, KernelExecutionError> {
        let KernelTransaction { mut event, policy, budget } = transaction;

        if let Some(policy) = policy {
            match policy.decision {
                PolicyDecision::Allowed => {
                    event = event.with_policy_checkpoint(policy);
                }
                PolicyDecision::Denied => {
                    return Err(KernelExecutionError::PolicyDenied {
                        domain: policy.domain,
                        reason: policy.reason,
                    });
                }
                PolicyDecision::RequiresApproval => {
                    return Err(KernelExecutionError::ApprovalRequired {
                        domain: policy.domain,
                        reason: policy.reason,
                    });
                }
            }
        }

        if let Some(budget) = budget {
            if budget.requested_amount_minor > budget.available_amount_minor {
                return Err(KernelExecutionError::BudgetExceeded {
                    budget_id: budget.budget_id,
                    requested_amount_minor: budget.requested_amount_minor,
                    available_amount_minor: budget.available_amount_minor,
                    currency: budget.currency,
                });
            }

            let remaining_amount_minor = budget.remaining_amount_minor();
            event = event.with_budget_checkpoint(
                BudgetCheckpoint::new(
                    budget.budget_id,
                    budget.requested_amount_minor,
                    budget.currency,
                )
                .with_remaining_amount_minor(remaining_amount_minor),
            );
        }

        let event_id = event.id;
        self.record(event).map_err(KernelExecutionError::from)?;
        self.kernel_receipt_for_event(event_id).ok_or_else(|| KernelExecutionError::Sync {
            message: format!("recorded kernel transaction {event_id} missing pending receipt"),
        })
    }

    pub(super) fn pending_events(&self) -> Vec<&SyncEvent> {
        self.outbox.peek(self.outbox.count())
    }
}
