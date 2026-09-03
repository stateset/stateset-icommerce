//! Per-invocation driver state shared by both backends.

use crate::kernel::envelope::{EnvelopeGuard, GuardRejection};
use crate::kernel::receipt::{
    command_event, preview_receipt, receipt_record, rejected_receipt, succeeded_receipt,
};
use crate::kernel_outbox::semantic_request_hash;
use crate::{KernelOutboxEvent, KernelReceiptRecord};
use chrono::{DateTime, Utc};
use serde::Serialize;
use stateset_core::{
    CommandEnvelope, CommerceError, ExecutionMode, ExecutionReceipt, KernelPolicy,
    PolicyDecisionEvidence, RetryDisposition,
};
use uuid::Uuid;

/// Everything a backend needs to run one governed command: the semantic
/// request hash, the policy decision, the static guard verdict, and a
/// receipt factory bound to the command.
#[derive(Debug)]
pub struct CommandRun<'a, C> {
    /// The envelope being executed.
    pub command: &'a CommandEnvelope<C>,
    /// Canonical hash bound to the idempotency key.
    pub request_hash: String,
    /// Instant execution began; recorded on every receipt.
    pub started_at: DateTime<Utc>,
    /// Policy decision evidence recorded on every receipt.
    pub policy: PolicyDecisionEvidence,
    /// First envelope or domain guard rejection, if any.
    pub guard: Option<GuardRejection>,
    /// Aggregate type recorded on every receipt.
    pub aggregate_type: &'static str,
}

impl<'a, C: Serialize> CommandRun<'a, C> {
    /// Validate the contract, hash the semantic request, evaluate policy, and
    /// run the envelope guard chain. Nothing here touches a database.
    pub fn prepare<H: Serialize>(
        command: &'a CommandEnvelope<C>,
        hash_payload: &H,
        policy: &KernelPolicy,
        guard: EnvelopeGuard<'_>,
        aggregate_type: &'static str,
    ) -> Result<Self, CommerceError> {
        command
            .validate_contract()
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let request_hash = semantic_request_hash(command, hash_payload)?;
        let started_at = Utc::now();
        let policy = policy.evaluate(command, started_at);
        let guard = guard.evaluate(command, &policy, started_at);
        Ok(Self { command, request_hash, started_at, policy, guard, aggregate_type })
    }

    /// Append domain-level static checks after the envelope chain. Runs only
    /// when no envelope rejection is already pending.
    #[must_use]
    pub fn then_guard(mut self, check: impl FnOnce(&Self) -> Option<GuardRejection>) -> Self {
        if self.guard.is_none() {
            self.guard = check(&self);
        }
        self
    }

    /// Whether the caller asked for a non-mutating preview.
    #[must_use]
    pub fn is_preview(&self) -> bool {
        self.command.mode == ExecutionMode::Preview
    }

    /// The pending guard rejection as a sealed receipt, if any.
    #[must_use]
    pub fn guard_receipt<T>(&self) -> Option<ExecutionReceipt<T>> {
        self.guard.as_ref().map(|guard| self.rejected(guard.code, &guard.message, guard.retry))
    }

    /// A rejection receipt carrying this run's policy evidence.
    #[must_use]
    pub fn rejected<T>(
        &self,
        code: &str,
        message: &str,
        retry: RetryDisposition,
    ) -> ExecutionReceipt<T> {
        let mut receipt = rejected_receipt(
            self.command,
            Some(self.policy.clone()),
            code,
            message,
            retry,
            self.aggregate_type,
        );
        receipt.started_at = self.started_at;
        receipt
    }

    /// A rejection receipt from a plan verdict.
    #[must_use]
    pub fn rejected_by<T>(&self, rejection: &GuardRejection) -> ExecutionReceipt<T> {
        self.rejected(rejection.code, &rejection.message, rejection.retry)
    }

    /// A preview receipt carrying this run's policy evidence.
    #[must_use]
    pub fn previewed<T>(&self) -> ExecutionReceipt<T> {
        let mut receipt = preview_receipt(self.command, self.policy.clone(), self.aggregate_type);
        receipt.started_at = self.started_at;
        receipt
    }

    /// A success receipt for a committed mutation.
    #[must_use]
    pub fn succeeded<T>(
        &self,
        result: T,
        aggregate_id: Option<String>,
        version_before: Option<i32>,
        version_after: Option<i32>,
        event_ids: Vec<Uuid>,
    ) -> ExecutionReceipt<T> {
        succeeded_receipt(
            self.command,
            self.policy.clone(),
            result,
            self.aggregate_type,
            aggregate_id,
            version_before,
            version_after,
            event_ids,
            self.started_at,
        )
    }

    /// Durable record for sealing `receipt` under this run's request hash.
    pub fn record<T: Serialize>(
        &self,
        receipt: &ExecutionReceipt<T>,
    ) -> Result<KernelReceiptRecord, CommerceError> {
        receipt_record(&self.request_hash, receipt)
    }

    /// A domain event carrying this command's context.
    #[must_use]
    pub fn event(
        &self,
        event_type: &str,
        aggregate_type: &str,
        aggregate_id: impl Into<String>,
        payload: serde_json::Value,
    ) -> KernelOutboxEvent {
        command_event(self.command, event_type, aggregate_type, aggregate_id, payload)
    }
}
