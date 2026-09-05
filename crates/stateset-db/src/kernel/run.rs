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
        let mut policy_decision = policy.evaluate(command, started_at);
        if policy.commands.get(&command.command_type).is_some_and(|rule| {
            (rule.requires_budget || rule.max_amount.is_some() || rule.approval_above.is_some())
                && !supports_observed_money_binding(&command.command_type)
        }) {
            policy_decision.allowed = false;
            policy_decision.reason_codes.push("policy.money_binding_unsupported".to_string());
        }
        if policy.commands.get(&command.command_type).is_some_and(|rule| {
            !rule.allowed_counterparty_ids.is_empty()
                && !supports_observed_counterparty_binding(&command.command_type)
        }) {
            policy_decision.allowed = false;
            policy_decision
                .reason_codes
                .push("policy.counterparty_binding_unsupported".to_string());
        }
        if policy.commands.get(&command.command_type).is_some_and(|rule| {
            (rule.max_asset_amount.is_some() || rule.approval_above_asset.is_some())
                && !supports_observed_asset_binding(&command.command_type)
        }) {
            policy_decision.allowed = false;
            policy_decision.reason_codes.push("policy.asset_binding_unsupported".to_string());
        }
        let guard = guard.evaluate(command, &policy_decision, started_at);
        Ok(Self {
            command,
            request_hash,
            started_at,
            policy: policy_decision,
            guard,
            aggregate_type,
        })
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

/// Whether the executor binds the signed, declared monetary commitment to
/// the amount observed in the domain payload before allowing execution.
///
/// Monetary policy must fail closed for every command not listed here. A
/// model-provided commitment is authorization intent, not proof of the value
/// the underlying mutation will actually commit.
fn supports_observed_money_binding(command_type: &str) -> bool {
    matches!(
        command_type,
        "checkout.commit" | "payments.create" | "payments.create_refund" | "subscriptions.charge"
    )
}

/// Whether the executor maps its domain target to a canonical economic ID and
/// checks that identity against the signed commitment before mutation.
fn supports_observed_counterparty_binding(command_type: &str) -> bool {
    matches!(command_type, "payments.create" | "payments.create_refund" | "subscriptions.charge")
}

/// Whether the executor binds a declared exact asset amount to domain state
/// while the kernel can still prevent the custody transition.
fn supports_observed_asset_binding(command_type: &str) -> bool {
    matches!(command_type, "a2a.escrow.create" | "a2a.escrow.fund")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use stateset_core::{EconomicCommitment, KernelCommandPolicy, KernelPrincipal, PrincipalKind};

    fn principal() -> KernelPrincipal {
        KernelPrincipal {
            id: "operator".into(),
            kind: PrincipalKind::Human,
            tenant_id: Some("tenant".into()),
            delegated_by: None,
            capabilities: vec![],
        }
    }

    #[test]
    fn monetary_rules_fail_closed_without_observed_payload_binding() {
        let mut command = CommandEnvelope::preview(
            "orders.transition",
            "money-binding-guard",
            principal(),
            serde_json::json!({}),
        );
        command.store_id = Some("store".into());
        let policy = KernelPolicy::new("p1").allow(
            "orders.transition",
            KernelCommandPolicy::requiring([] as [&str; 0]).with_budget(),
        );

        let run = CommandRun::prepare(
            &command,
            &command.payload,
            &policy,
            EnvelopeGuard::aggregate("orders.transition"),
            "order",
        )
        .expect("prepare");

        assert!(!run.policy.allowed);
        assert!(run.policy.reason_codes.iter().any(|code| code == "policy.budget_required"));
        assert!(
            run.policy.reason_codes.iter().any(|code| code == "policy.money_binding_unsupported")
        );
        assert_eq!(run.guard.as_ref().map(|guard| guard.code), Some("kernel.policy_denied"));
    }

    #[test]
    fn asset_rules_fail_closed_after_external_settlement() {
        let mut command = CommandEnvelope::preview(
            "x402.settle",
            "asset-binding-guard",
            principal(),
            serde_json::json!({}),
        );
        command.store_id = Some("store".into());
        command.commitment = Some(EconomicCommitment::for_asset(Decimal::ONE, "USDC"));
        let policy = KernelPolicy::new("p1").allow(
            "x402.settle",
            KernelCommandPolicy::requiring([] as [&str; 0])
                .with_max_asset_amount(Decimal::new(100, 0), "USDC"),
        );

        let run = CommandRun::prepare(
            &command,
            &command.payload,
            &policy,
            EnvelopeGuard::aggregate("x402.settle"),
            "x402_intent",
        )
        .expect("prepare");

        assert!(!run.policy.allowed);
        assert!(
            run.policy.reason_codes.iter().any(|code| code == "policy.asset_binding_unsupported")
        );
    }

    #[test]
    fn escrow_asset_rules_reach_the_observed_binding_layer() {
        let mut command = CommandEnvelope::preview(
            "a2a.escrow.create",
            "asset-binding-supported",
            principal(),
            serde_json::json!({}),
        );
        command.store_id = Some("store".into());
        command.commitment = Some(EconomicCommitment::for_asset(Decimal::ONE, "USDC"));
        let policy = KernelPolicy::new("p1").allow(
            "a2a.escrow.create",
            KernelCommandPolicy::requiring([] as [&str; 0])
                .with_max_asset_amount(Decimal::new(100, 0), "USDC"),
        );

        let run = CommandRun::prepare(
            &command,
            &command.payload,
            &policy,
            EnvelopeGuard::create("a2a.escrow.create"),
            "a2a_escrow",
        )
        .expect("prepare");

        assert!(run.policy.allowed);
        assert!(
            !run.policy.reason_codes.iter().any(|code| code == "policy.asset_binding_unsupported")
        );
    }

    #[test]
    fn payment_monetary_rules_reach_the_observed_binding_layer() {
        let mut command = CommandEnvelope::preview(
            "payments.create",
            "money-binding-supported",
            principal(),
            serde_json::json!({}),
        );
        command.store_id = Some("store".into());
        let policy = KernelPolicy::new("p1")
            .allow("payments.create", KernelCommandPolicy::requiring([] as [&str; 0]));

        let run = CommandRun::prepare(
            &command,
            &command.payload,
            &policy,
            EnvelopeGuard::create("payments.create"),
            "payment",
        )
        .expect("prepare");

        assert!(run.policy.allowed);
        assert!(
            !run.policy.reason_codes.iter().any(|code| code == "policy.money_binding_unsupported")
        );
    }

    #[test]
    fn counterparty_allowlists_reach_supported_observed_target_binding() {
        let mut command = CommandEnvelope::preview(
            "payments.create",
            "counterparty-binding-guard",
            principal(),
            serde_json::json!({}),
        );
        command.store_id = Some("store".into());
        let policy = KernelPolicy::new("p1").allow(
            "payments.create",
            KernelCommandPolicy::requiring([] as [&str; 0])
                .for_counterparties(["counterparty:allowed"]),
        );

        let run = CommandRun::prepare(
            &command,
            &command.payload,
            &policy,
            EnvelopeGuard::create("payments.create"),
            "payment",
        )
        .expect("prepare");

        assert!(!run.policy.allowed);
        assert!(run.policy.reason_codes.iter().any(|code| code == "policy.counterparty_required"));
        assert!(
            !run.policy
                .reason_codes
                .iter()
                .any(|code| code == "policy.counterparty_binding_unsupported")
        );
    }

    #[test]
    fn counterparty_allowlists_fail_closed_for_unbound_commands() {
        let mut command = CommandEnvelope::preview(
            "orders.transition",
            "counterparty-binding-unsupported",
            principal(),
            serde_json::json!({}),
        );
        command.store_id = Some("store".into());
        let policy = KernelPolicy::new("p1").allow(
            "orders.transition",
            KernelCommandPolicy::requiring([] as [&str; 0])
                .for_counterparties(["counterparty:allowed"]),
        );

        let run = CommandRun::prepare(
            &command,
            &command.payload,
            &policy,
            EnvelopeGuard::aggregate("orders.transition"),
            "order",
        )
        .expect("prepare");

        assert!(!run.policy.allowed);
        assert!(
            run.policy
                .reason_codes
                .iter()
                .any(|code| code == "policy.counterparty_binding_unsupported")
        );
        assert_eq!(run.guard.as_ref().map(|guard| guard.code), Some("kernel.policy_denied"));
    }
}
