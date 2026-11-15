//! Shared fixtures for the receipt audit-correspondence proofs.
//!
//! Both backends run the same scenario with `carts.create` — a command with
//! no domain-level idempotency, so the materialized receipt is the only thing
//! standing between a retry and a second cart: one key previewed, promoted
//! and replayed; one key only previewed; one key rejected; one key applied
//! directly, then hit with an idempotency conflict that must not persist.
//! That is five sealed audit entries and four materialized receipts — the
//! promoted preview is superseded under its key.

use stateset_core::{
    CommandEnvelope, CreateCart, CurrencyCode, ExecutionMode, KernelCommandPolicy, KernelPolicy,
    KernelPrincipal, PrincipalKind,
};
use uuid::Uuid;

const POLICY: &str = "audit-correspondence-v1";

/// Audit entries the scenario seals.
pub(crate) const SCENARIO_ENTRIES: u64 = 5;
/// Materialized receipts the scenario leaves.
pub(crate) const SCENARIO_ROWS: i64 = 4;

pub(crate) fn allowing() -> KernelPolicy {
    KernelPolicy::new(POLICY)
        .allow("carts.create", KernelCommandPolicy::requiring(["carts.create"]))
}

/// The same policy version granting nothing: every command is a durable
/// `kernel.policy_denied` rejection.
pub(crate) fn denying() -> KernelPolicy {
    KernelPolicy::new(POLICY)
}

/// The cart email a key's command creates, to count carts per key.
pub(crate) fn email(key: &str) -> String {
    format!("{key}@example.com")
}

/// A fresh invocation (new command id) of the one cart a key is bound to.
pub(crate) fn preview(key: &str) -> CommandEnvelope<CreateCart> {
    let mut command = CommandEnvelope::preview(
        "carts.create",
        key,
        KernelPrincipal {
            id: "agent:audit-correspondence".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-audit".into()),
            delegated_by: Some("user-audit".into()),
            capabilities: vec!["carts.create".into()],
        },
        CreateCart {
            customer_email: Some(email(key)),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        },
    );
    command.store_id = Some("store-audit".into());
    command.policy_version = Some(POLICY.into());
    command
}

pub(crate) fn apply(key: &str) -> CommandEnvelope<CreateCart> {
    let mut command = preview(key);
    command.mode = ExecutionMode::Apply;
    command
}

/// The same key bound to a different cart: an idempotency conflict.
pub(crate) fn conflicting(key: &str) -> CommandEnvelope<CreateCart> {
    let mut command = apply(key);
    command.payload.notes = Some("a different request".into());
    command
}

/// Distinct idempotency keys for one scenario run.
pub(crate) struct Keys {
    pub(crate) promoted: String,
    pub(crate) preview_only: String,
    pub(crate) rejected: String,
    pub(crate) applied: String,
}

impl Keys {
    pub(crate) fn new() -> Self {
        let run = Uuid::new_v4().simple().to_string();
        Self {
            promoted: format!("audit-promoted-{run}"),
            preview_only: format!("audit-preview-only-{run}"),
            rejected: format!("audit-rejected-{run}"),
            applied: format!("audit-applied-{run}"),
        }
    }
}
