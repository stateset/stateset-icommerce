//! One receipt factory for both backends.
//!
//! Receipts differ across backends only in ids and timestamps; every other
//! field is derived here from the command, the policy decision, and the
//! outcome so the two executors cannot drift.

use crate::{KernelOutboxEvent, KernelReceiptRecord};
use chrono::{DateTime, Utc};
use serde::Serialize;
use stateset_core::{
    CommandEnvelope, CommerceError, EconomicReceiptContext, ExecutionReceipt, ExecutionStatus,
    PolicyDecisionEvidence, RetryDisposition,
};
use uuid::Uuid;

/// Stable status label persisted on `kernel_receipts.status`.
#[must_use]
pub const fn status_name(status: ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Previewed => "previewed",
        ExecutionStatus::Succeeded => "succeeded",
        ExecutionStatus::Rejected => "rejected",
        ExecutionStatus::Failed => "failed",
    }
}

fn base<C, T>(
    command: &CommandEnvelope<C>,
    status: ExecutionStatus,
    policy: Option<PolicyDecisionEvidence>,
    aggregate_type: &str,
    started_at: DateTime<Utc>,
) -> ExecutionReceipt<T> {
    ExecutionReceipt {
        contract_version: stateset_core::KERNEL_CONTRACT_VERSION.into(),
        receipt_id: Uuid::new_v4(),
        command_id: command.command_id,
        idempotency_key: command.idempotency_key.clone(),
        command_type: command.command_type.clone(),
        status,
        result: None,
        error_code: None,
        error_message: None,
        retry: RetryDisposition::Never,
        aggregate_type: Some(aggregate_type.into()),
        aggregate_id: None,
        version_before: None,
        version_after: None,
        event_ids: Vec::new(),
        policy,
        economic_context: Some(EconomicReceiptContext::from_command(command)),
        audit_hash: None,
        started_at,
        completed_at: Utc::now(),
    }
}

/// A sealed rejection. `retry` is always explicit — there is no default.
#[must_use]
pub fn rejected_receipt<C, T>(
    command: &CommandEnvelope<C>,
    policy: Option<PolicyDecisionEvidence>,
    code: &str,
    message: &str,
    retry: RetryDisposition,
    aggregate_type: &str,
) -> ExecutionReceipt<T> {
    let mut receipt = base(command, ExecutionStatus::Rejected, policy, aggregate_type, Utc::now());
    receipt.error_code = Some(code.into());
    receipt.error_message = Some(message.into());
    receipt.retry = retry;
    receipt
}

/// A durable, non-mutating preview.
#[must_use]
pub fn preview_receipt<C, T>(
    command: &CommandEnvelope<C>,
    policy: PolicyDecisionEvidence,
    aggregate_type: &str,
) -> ExecutionReceipt<T> {
    base(command, ExecutionStatus::Previewed, Some(policy), aggregate_type, Utc::now())
}

/// A committed mutation. Retrying with the same key replays this receipt.
///
/// The parameters are the full description of what was committed; grouping
/// them into a struct would only move the same fields behind another name.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn succeeded_receipt<C, T>(
    command: &CommandEnvelope<C>,
    policy: PolicyDecisionEvidence,
    result: T,
    aggregate_type: &str,
    aggregate_id: Option<String>,
    version_before: Option<i32>,
    version_after: Option<i32>,
    event_ids: Vec<Uuid>,
    started_at: DateTime<Utc>,
) -> ExecutionReceipt<T> {
    let mut receipt =
        base(command, ExecutionStatus::Succeeded, Some(policy), aggregate_type, started_at);
    receipt.result = Some(result);
    receipt.retry = RetryDisposition::SameKey;
    receipt.aggregate_id = aggregate_id;
    receipt.version_before = version_before;
    receipt.version_after = version_after;
    receipt.event_ids = event_ids;
    receipt
}

/// Durable record for a receipt about to be sealed into the audit chain.
pub fn receipt_record<T: Serialize>(
    request_hash: &str,
    receipt: &ExecutionReceipt<T>,
) -> Result<KernelReceiptRecord, CommerceError> {
    let value = serde_json::to_value(receipt)
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    Ok(KernelReceiptRecord {
        command_id: receipt.command_id,
        idempotency_key: receipt.idempotency_key.clone(),
        command_type: receipt.command_type.clone(),
        contract_version: receipt.contract_version.clone(),
        request_hash: request_hash.into(),
        status: status_name(receipt.status).into(),
        receipt: value,
        created_at: receipt.started_at,
        completed_at: receipt.completed_at,
    })
}

/// Serialized principal kind as persisted on outbox rows.
#[must_use]
pub fn principal_kind_name<C>(command: &CommandEnvelope<C>) -> String {
    serde_json::to_value(command.principal.kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

/// Attach command, principal, and causal context to an outbox event.
pub fn attach_command_context<C>(event: &mut KernelOutboxEvent, command: &CommandEnvelope<C>) {
    event.command_id = Some(command.command_id);
    event.principal_type = Some(principal_kind_name(command));
    event.principal_id = Some(command.principal.id.clone());
    event.correlation_id = command.correlation_id;
    event.causation_id = command.causation_id;
}

/// Build a domain event already carrying the command context.
#[must_use]
pub fn command_event<C>(
    command: &CommandEnvelope<C>,
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: impl Into<String>,
    payload: serde_json::Value,
) -> KernelOutboxEvent {
    let mut event = KernelOutboxEvent::domain(
        event_type,
        aggregate_type,
        aggregate_id,
        payload,
        Some(command.idempotency_key.clone()),
    );
    attach_command_context(&mut event, command);
    event
}

/// Stable receipt code for a checkout commit failure.
#[must_use]
pub fn checkout_error_code(error: &CommerceError) -> &'static str {
    error.invariant_code().unwrap_or(match error {
        CommerceError::NotFound => "commerce.checkout.cart_not_found",
        CommerceError::ValidationError(_) => "commerce.checkout.validation_failed",
        CommerceError::Conflict(_) => "commerce.checkout.conflict",
        _ => "commerce.checkout.rejected",
    })
}
