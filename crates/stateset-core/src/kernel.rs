//! Versioned command and receipt contracts for agent-safe commerce execution.
//!
//! Domain repositories remain usable directly. This module supplies the
//! stable envelope an AI runtime can place around any domain command so
//! identity, intent, authorization evidence, retries, and outcomes are
//! explicit and machine-verifiable.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stateset_primitives::{CurrencyCode, Money, MoneyWire};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use uuid::Uuid;

/// Current version of the kernel command/receipt wire contract.
pub const KERNEL_CONTRACT_VERSION: &str = "1.0";

/// Identity class responsible for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrincipalKind {
    /// An authenticated person.
    Human,
    /// An autonomous or delegated software agent.
    Agent,
    /// An internal system process.
    System,
    /// An external integration.
    Integration,
}

/// Authenticated identity and delegation context for a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelPrincipal {
    /// Stable subject identifier.
    pub id: String,
    /// Subject category.
    pub kind: PrincipalKind,
    /// Tenant boundary, when the store is multi-tenant.
    pub tenant_id: Option<String>,
    /// Principal that delegated authority to this subject.
    pub delegated_by: Option<String>,
    /// Capabilities asserted by the caller and checked by policy.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Whether execution is a non-mutating preview or an authorized mutation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Validate and describe effects without committing them.
    #[default]
    Preview,
    /// Commit the command if all guards pass.
    Apply,
}

/// Evidence for an approval required by policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalEvidence {
    /// Stable approval identifier.
    pub approval_id: String,
    /// Principal that granted approval.
    pub approved_by: String,
    /// Policy-defined scope of the approval.
    pub scope: String,
    /// Tenant this approval authorizes.
    pub tenant_id: Option<String>,
    /// Store this approval authorizes.
    pub store_id: Option<String>,
    /// Semantic retry key this approval authorizes.
    pub idempotency_key: Option<String>,
    /// When the approval was granted.
    pub approved_at: DateTime<Utc>,
    /// Optional expiry after which this evidence must be rejected.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Cryptographic proof that a trusted issuer authorized the semantic command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityEvidence {
    pub issuer: String,
    pub key_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// Hex-encoded Ed25519 signature over [`authority_signing_hash`]. Attach
    /// the authority claim with an empty signature before computing the hash;
    /// the digest binds every claim field except this signature value.
    pub signature: String,
}

/// A principal-issued objective under which an agent may take economic action.
///
/// A mandate is evidence, not ambient authority. Policy still decides whether
/// the command is permitted, and deployments that require non-repudiation
/// should combine it with [`AuthorityEvidence`]. Because the complete mandate
/// is included in the authority and idempotency hashes, it cannot be swapped
/// between signing, preview, apply, or retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicMandate {
    /// Stable mandate identifier shared by every command pursuing the objective.
    pub mandate_id: String,
    /// Agent or other principal to which the mandate was issued.
    pub subject_id: String,
    /// Principal that issued the mandate.
    pub issued_by: String,
    /// Human-readable economic objective retained for audit and explanation.
    pub objective: String,
    /// Commands this mandate permits. Empty is never interpreted as wildcard.
    #[serde(default)]
    pub allowed_commands: BTreeSet<String>,
    /// Tenant boundary to which the mandate is confined.
    pub tenant_id: Option<String>,
    /// Store boundary to which the mandate is confined.
    pub store_id: Option<String>,
    /// Beginning of the mandate validity window.
    pub issued_at: DateTime<Utc>,
    /// End of the mandate validity window.
    pub expires_at: DateTime<Utc>,
}

impl EconomicMandate {
    /// Create a mandate; bind tenant and store with [`Self::for_scope`].
    pub fn new(
        mandate_id: impl Into<String>,
        subject_id: impl Into<String>,
        issued_by: impl Into<String>,
        objective: impl Into<String>,
        allowed_commands: impl IntoIterator<Item = impl Into<String>>,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            mandate_id: mandate_id.into(),
            subject_id: subject_id.into(),
            issued_by: issued_by.into(),
            objective: objective.into(),
            allowed_commands: allowed_commands.into_iter().map(Into::into).collect(),
            tenant_id: None,
            store_id: None,
            issued_at,
            expires_at,
        }
    }

    /// Bind this mandate to one tenant and logical store.
    #[must_use]
    pub fn for_scope(mut self, tenant_id: impl Into<String>, store_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self.store_id = Some(store_id.into());
        self
    }
}

/// Exact non-fiat amount identified by an asset symbol or canonical token ID.
///
/// Unlike [`MoneyWire`], the denomination is not restricted to three-letter
/// ISO currency codes. The amount remains a decimal string and the asset ID is
/// case-sensitive so signatures can bind chain-qualified identifiers without
/// lossy normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAmountWire {
    pub amount: String,
    pub asset: String,
}

impl AssetAmountWire {
    pub fn new(amount: Decimal, asset: impl Into<String>) -> Self {
        Self { amount: amount.to_string(), asset: asset.into() }
    }

    pub fn validate(&self) -> Result<Decimal, KernelContractError> {
        if self.asset.trim().is_empty() {
            return Err(KernelContractError::MissingField("asset_amount.asset"));
        }
        if self.asset != self.asset.trim() || self.asset.len() > 256 {
            return Err(KernelContractError::InvalidField(
                "asset_amount.asset",
                "asset identifiers must be trimmed and at most 256 bytes".into(),
            ));
        }
        let amount = self.amount.parse::<Decimal>().map_err(|error| {
            KernelContractError::InvalidField("asset_amount.amount", error.to_string())
        })?;
        if amount.is_sign_negative() {
            return Err(KernelContractError::InvalidField(
                "asset_amount.amount",
                "asset commitments cannot be negative".into(),
            ));
        }
        Ok(amount)
    }
}

/// Resources an economic command proposes to commit.
///
/// Money uses the decimal-string wire type; floating point never enters the
/// authorization boundary. `quantity` is also a decimal string because units
/// may be fractional. Domain executors must bind these declared values to the
/// command payload before mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicCommitment {
    /// Budget or wallet against which this commitment should be accounted.
    pub budget_id: Option<String>,
    /// Exact money moved or placed at risk by the command.
    pub amount: Option<MoneyWire>,
    /// Exact amount of a non-fiat or chain-qualified asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_amount: Option<AssetAmountWire>,
    /// Stable identity of the other economic party, when one exists.
    pub counterparty_id: Option<String>,
    /// Exact inventory, capacity, or other unit commitment.
    pub quantity: Option<String>,
    /// Evidence identifiers (quotes, tickets, contracts, etc.) supporting it.
    #[serde(default)]
    pub evidence: Vec<String>,
}

impl EconomicCommitment {
    /// Declare an exact monetary commitment against a named budget.
    pub fn for_money(budget_id: impl Into<String>, amount: Money) -> Self {
        Self {
            budget_id: Some(budget_id.into()),
            amount: Some(amount.to_wire()),
            asset_amount: None,
            counterparty_id: None,
            quantity: None,
            evidence: Vec::new(),
        }
    }

    /// Declare an exact non-fiat or token-denominated commitment.
    pub fn for_asset(amount: Decimal, asset: impl Into<String>) -> Self {
        Self {
            budget_id: None,
            amount: None,
            asset_amount: Some(AssetAmountWire::new(amount, asset)),
            counterparty_id: None,
            quantity: None,
            evidence: Vec::new(),
        }
    }

    /// Bind the commitment to a counterparty.
    #[must_use]
    pub fn with_counterparty(mut self, counterparty_id: impl Into<String>) -> Self {
        self.counterparty_id = Some(counterparty_id.into());
        self
    }

    /// Attach exact non-monetary units to the commitment.
    #[must_use]
    pub fn with_quantity(mut self, quantity: impl Into<String>) -> Self {
        self.quantity = Some(quantity.into());
        self
    }

    /// Attach stable evidence identifiers supporting the action.
    #[must_use]
    pub fn with_evidence(mut self, evidence: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.evidence = evidence.into_iter().map(Into::into).collect();
        self
    }

    /// Return the declared money after exact decimal and currency-scale validation.
    pub fn money(&self) -> Result<Option<Money>, KernelContractError> {
        self.amount.clone().map(Money::try_from).transpose().map_err(|error| {
            KernelContractError::InvalidField("commitment.amount", error.to_string())
        })
    }

    /// Return the exact declared asset amount and its case-sensitive asset ID.
    pub fn asset(&self) -> Result<Option<(Decimal, &str)>, KernelContractError> {
        self.asset_amount
            .as_ref()
            .map(|asset| asset.validate().map(|amount| (amount, asset.asset.as_str())))
            .transpose()
    }

    /// Return the declared quantity after exact decimal parsing.
    pub fn parsed_quantity(&self) -> Result<Option<Decimal>, KernelContractError> {
        self.quantity.as_deref().map(str::parse::<Decimal>).transpose().map_err(|error| {
            KernelContractError::InvalidField("commitment.quantity", error.to_string())
        })
    }

    /// Check that a money-moving executor observed exactly the amount declared
    /// at the authorization boundary.
    pub fn binds_money(&self, amount: Decimal, currency: CurrencyCode) -> bool {
        self.money()
            .ok()
            .flatten()
            .is_some_and(|declared| declared.currency() == currency && declared.amount() == amount)
    }

    /// Check an executor-observed token or non-fiat amount against the signed declaration.
    pub fn binds_asset(&self, amount: Decimal, asset: &str) -> bool {
        self.asset()
            .ok()
            .flatten()
            .is_some_and(|(declared, denomination)| declared == amount && denomination == asset)
    }
}

/// Operator-provisioned monetary authority for one principal.
///
/// The definition is immutable after creation. Executors maintain committed
/// and available balances separately so changing a policy file cannot reset
/// spend already reserved by successful commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicBudget {
    /// Stable identifier referenced by [`EconomicCommitment::budget_id`].
    pub budget_id: String,
    /// Principal permitted to commit this budget.
    pub principal_id: String,
    /// Optional tenant boundary.
    pub tenant_id: Option<String>,
    /// Optional logical store boundary.
    pub store_id: Option<String>,
    /// Total exact amount available over the budget lifetime.
    pub limit: MoneyWire,
    /// Beginning of the validity window.
    pub valid_from: DateTime<Utc>,
    /// End of the validity window.
    pub expires_at: DateTime<Utc>,
}

impl EconomicBudget {
    /// Construct an unscoped budget definition.
    pub fn new(
        budget_id: impl Into<String>,
        principal_id: impl Into<String>,
        limit: Money,
        valid_from: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            budget_id: budget_id.into(),
            principal_id: principal_id.into(),
            tenant_id: None,
            store_id: None,
            limit: limit.to_wire(),
            valid_from,
            expires_at,
        }
    }

    /// Bind this budget to one tenant and logical store.
    #[must_use]
    pub fn for_scope(mut self, tenant_id: impl Into<String>, store_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self.store_id = Some(store_id.into());
        self
    }

    /// Validate identifiers, exact money, and the validity window.
    pub fn validate(&self) -> Result<Money, KernelContractError> {
        if self.budget_id.trim().is_empty() {
            return Err(KernelContractError::MissingField("budget.budget_id"));
        }
        if self.principal_id.trim().is_empty() {
            return Err(KernelContractError::MissingField("budget.principal_id"));
        }
        if self.tenant_id.as_deref().is_some_and(|value| value.trim().is_empty()) {
            return Err(KernelContractError::MissingField("budget.tenant_id"));
        }
        if self.store_id.as_deref().is_some_and(|value| value.trim().is_empty()) {
            return Err(KernelContractError::MissingField("budget.store_id"));
        }
        if self.expires_at <= self.valid_from {
            return Err(KernelContractError::InvalidField(
                "budget.expires_at",
                "budget expiry must be after the beginning of its validity window".into(),
            ));
        }
        let limit = Money::try_from(self.limit.clone()).map_err(|error| {
            KernelContractError::InvalidField("budget.limit", error.to_string())
        })?;
        if limit.is_negative() {
            return Err(KernelContractError::InvalidField(
                "budget.limit",
                "budget limits cannot be negative".into(),
            ));
        }
        Ok(limit)
    }
}

/// Current exact balance of an operator-provisioned economic budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicBudgetStatus {
    /// Immutable budget definition.
    pub budget: EconomicBudget,
    /// Amount reserved by successfully committed commands.
    pub committed: MoneyWire,
    /// Amount still available for new commitments.
    pub available: MoneyWire,
}

/// Versioned execution request shared by every commerce command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope<T> {
    /// Wire-contract version.
    pub contract_version: String,
    /// Unique identity for this invocation.
    pub command_id: Uuid,
    /// Stable retry key. Retries must reuse this value.
    pub idempotency_key: String,
    /// Stable namespaced command name, such as `payments.create`.
    pub command_type: String,
    /// Authenticated actor and delegation context.
    pub principal: KernelPrincipal,
    /// Logical store boundary.
    pub store_id: Option<String>,
    /// Root workflow identifier.
    pub correlation_id: Option<Uuid>,
    /// Command or event that caused this command.
    pub causation_id: Option<Uuid>,
    /// Optimistic concurrency version expected by the caller.
    pub expected_version: Option<i32>,
    /// Policy revision the caller expects to govern execution.
    pub policy_version: Option<String>,
    /// Human or automated approval evidence.
    pub approval: Option<ApprovalEvidence>,
    /// Optional signed authority, required when the command policy enables it.
    pub authority: Option<AuthorityEvidence>,
    /// Objective and delegation scope under which this command is being pursued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandate: Option<EconomicMandate>,
    /// Exact resources this command proposes to commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<EconomicCommitment>,
    /// Time after which execution should not begin.
    pub deadline: Option<DateTime<Utc>>,
    /// Distributed tracing identifier.
    pub trace_id: Option<String>,
    /// Preview or apply posture.
    pub mode: ExecutionMode,
    /// Domain-specific request.
    pub payload: T,
    /// Time the command was issued.
    pub issued_at: DateTime<Utc>,
}

impl<T> CommandEnvelope<T> {
    /// Construct a safe-by-default preview command.
    pub fn preview(
        command_type: impl Into<String>,
        idempotency_key: impl Into<String>,
        principal: KernelPrincipal,
        payload: T,
    ) -> Self {
        Self {
            contract_version: KERNEL_CONTRACT_VERSION.into(),
            command_id: Uuid::new_v4(),
            idempotency_key: idempotency_key.into(),
            command_type: command_type.into(),
            principal,
            store_id: None,
            correlation_id: None,
            causation_id: None,
            expected_version: None,
            policy_version: None,
            approval: None,
            authority: None,
            mandate: None,
            commitment: None,
            deadline: None,
            trace_id: None,
            mode: ExecutionMode::Preview,
            payload,
            issued_at: Utc::now(),
        }
    }

    /// Explicitly opt this command into mutation after preview/authorization.
    #[must_use]
    pub const fn into_apply(mut self) -> Self {
        self.mode = ExecutionMode::Apply;
        self
    }

    /// Attach the economic objective under which this command is issued.
    #[must_use]
    pub fn with_mandate(mut self, mandate: EconomicMandate) -> Self {
        self.mandate = Some(mandate);
        self
    }

    /// Attach the exact resources this command proposes to commit.
    #[must_use]
    pub fn with_commitment(mut self, commitment: EconomicCommitment) -> Self {
        self.commitment = Some(commitment);
        self
    }

    /// Validate the cross-domain kernel contract.
    pub fn validate_contract(&self) -> Result<(), KernelContractError> {
        if self.contract_version != KERNEL_CONTRACT_VERSION {
            return Err(KernelContractError::UnsupportedVersion(self.contract_version.clone()));
        }
        if self.command_id.is_nil() {
            return Err(KernelContractError::MissingField("command_id"));
        }
        if self.command_type.trim().is_empty() {
            return Err(KernelContractError::MissingField("command_type"));
        }
        if self.idempotency_key.trim().is_empty() {
            return Err(KernelContractError::MissingField("idempotency_key"));
        }
        if self.principal.id.trim().is_empty() {
            return Err(KernelContractError::MissingField("principal.id"));
        }
        if let Some(mandate) = &self.mandate {
            if mandate.mandate_id.trim().is_empty() {
                return Err(KernelContractError::MissingField("mandate.mandate_id"));
            }
            if mandate.subject_id.trim().is_empty() {
                return Err(KernelContractError::MissingField("mandate.subject_id"));
            }
            if mandate.issued_by.trim().is_empty() {
                return Err(KernelContractError::MissingField("mandate.issued_by"));
            }
            if mandate.objective.trim().is_empty() {
                return Err(KernelContractError::MissingField("mandate.objective"));
            }
            if mandate.expires_at <= mandate.issued_at {
                return Err(KernelContractError::InvalidField(
                    "mandate.expires_at",
                    "mandate expiry must be after its issue time".into(),
                ));
            }
        }
        if let Some(commitment) = &self.commitment {
            if commitment.budget_id.as_deref().is_some_and(|value| value.trim().is_empty()) {
                return Err(KernelContractError::MissingField("commitment.budget_id"));
            }
            if commitment.counterparty_id.as_deref().is_some_and(|value| value.trim().is_empty()) {
                return Err(KernelContractError::MissingField("commitment.counterparty_id"));
            }
            if commitment.evidence.iter().any(|evidence| evidence.trim().is_empty()) {
                return Err(KernelContractError::InvalidField(
                    "commitment.evidence",
                    "evidence identifiers cannot be empty".into(),
                ));
            }
            if commitment.amount.is_some() {
                let money = commitment.money()?;
                if money.is_some_and(|money| money.is_negative()) {
                    return Err(KernelContractError::InvalidField(
                        "commitment.amount",
                        "economic commitments cannot be negative".into(),
                    ));
                }
            }
            if commitment.amount.is_some() && commitment.asset_amount.is_some() {
                return Err(KernelContractError::InvalidField(
                    "commitment.asset_amount",
                    "a commitment cannot declare both fiat money and a non-fiat asset".into(),
                ));
            }
            if commitment.asset_amount.is_some() {
                commitment.asset()?;
                if commitment.budget_id.is_some() {
                    return Err(KernelContractError::InvalidField(
                        "commitment.budget_id",
                        "asset commitments cannot name a fiat budget".into(),
                    ));
                }
            }
            if commitment.parsed_quantity()?.is_some_and(|quantity| quantity.is_sign_negative()) {
                return Err(KernelContractError::InvalidField(
                    "commitment.quantity",
                    "economic commitments cannot be negative".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Result category recorded in an execution receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Guards passed and predicted effects were returned without mutation.
    Previewed,
    /// Mutation committed.
    Succeeded,
    /// Policy, approval, validation, or concurrency guard rejected execution.
    Rejected,
    /// Execution began but failed.
    Failed,
}

/// Machine-readable guidance for retrying a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDisposition {
    /// Retrying cannot resolve this outcome.
    Never,
    /// Retry using exactly the same idempotency key.
    SameKey,
    /// Reload state and retry after resolving an optimistic conflict.
    AfterConflict,
    /// Retry later using exactly the same idempotency key.
    AfterDelay,
}

/// Policy decision captured with a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionEvidence {
    /// Policy revision evaluated.
    pub policy_version: String,
    /// Stable decision identifier.
    pub decision_id: String,
    /// Whether policy allowed execution.
    pub allowed: bool,
    /// Stable reason codes suitable for automation.
    #[serde(default)]
    pub reason_codes: Vec<String>,
}

/// Accountability data copied from a command into its durable receipt.
///
/// This makes a receipt independently useful to an auditor: it identifies
/// who acted, for whom, under which objective, against which budget and
/// counterparty, and what approval or signed authority supported the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicReceiptContext {
    /// Authenticated actor and principal delegation chain.
    pub principal: KernelPrincipal,
    /// Logical store in which the action occurred.
    pub store_id: Option<String>,
    /// Durable workstream/objective correlation identifier.
    pub correlation_id: Option<Uuid>,
    /// Mandate supplied with the command.
    pub mandate: Option<EconomicMandate>,
    /// Resources declared and bound at authorization time.
    pub commitment: Option<EconomicCommitment>,
    /// Approval used to cross an authority tier, if any.
    pub approval_id: Option<String>,
    /// Issuer of cryptographically signed authority, if any.
    pub authority_issuer: Option<String>,
}

impl EconomicReceiptContext {
    /// Capture the accountability context of a command.
    #[must_use]
    pub fn from_command<T>(command: &CommandEnvelope<T>) -> Self {
        Self {
            principal: command.principal.clone(),
            store_id: command.store_id.clone(),
            correlation_id: command.correlation_id,
            mandate: command.mandate.clone(),
            commitment: command.commitment.clone(),
            approval_id: command.approval.as_ref().map(|approval| approval.approval_id.clone()),
            authority_issuer: command.authority.as_ref().map(|authority| authority.issuer.clone()),
        }
    }
}

/// Policy requirements for one namespaced kernel command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCommandPolicy {
    /// Capabilities the authenticated principal must hold.
    #[serde(default)]
    pub required_capabilities: BTreeSet<String>,
    /// Whether unexpired approval evidence scoped to this command is required.
    #[serde(default)]
    pub requires_approval: bool,
    /// Require a non-empty tenant boundary on the principal.
    #[serde(default = "default_true")]
    pub requires_tenant: bool,
    /// Require a non-empty logical store boundary on the command.
    #[serde(default = "default_true")]
    pub requires_store: bool,
    /// Optional tenant allowlist. Empty permits any non-empty tenant when
    /// `requires_tenant` is enabled.
    #[serde(default)]
    pub allowed_tenant_ids: BTreeSet<String>,
    /// Optional logical-store allowlist. Empty permits any non-empty store
    /// when `requires_store` is enabled.
    #[serde(default)]
    pub allowed_store_ids: BTreeSet<String>,
    /// Require autonomous agents to identify their delegating principal.
    #[serde(default = "default_true")]
    pub requires_agent_delegation: bool,
    /// Require a valid signature from a key trusted by the policy.
    #[serde(default)]
    pub requires_signed_authority: bool,
    /// Require a live mandate bound to subject, issuer, command, tenant, and store.
    #[serde(default)]
    pub requires_mandate: bool,
    /// Require a named, durable budget for every monetary commitment.
    #[serde(default)]
    pub requires_budget: bool,
    /// Maximum exact monetary commitment permitted for one command.
    #[serde(default)]
    pub max_amount: Option<MoneyWire>,
    /// Require approval only when the commitment exceeds this exact amount.
    #[serde(default)]
    pub approval_above: Option<MoneyWire>,
    /// Maximum exact non-fiat asset commitment permitted for one command.
    #[serde(default)]
    pub max_asset_amount: Option<AssetAmountWire>,
    /// Require approval when the asset commitment exceeds this exact amount.
    #[serde(default)]
    pub approval_above_asset: Option<AssetAmountWire>,
    /// Maximum exact inventory, capacity, or other unit commitment.
    #[serde(default)]
    pub max_quantity: Option<String>,
    /// Counterparties permitted by this command rule; empty permits any.
    #[serde(default)]
    pub allowed_counterparty_ids: BTreeSet<String>,
}

const fn default_true() -> bool {
    true
}

impl KernelCommandPolicy {
    /// Require the supplied capabilities without requiring separate approval.
    pub fn requiring(capabilities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            required_capabilities: capabilities.into_iter().map(Into::into).collect(),
            requires_approval: false,
            requires_tenant: true,
            requires_store: true,
            allowed_tenant_ids: BTreeSet::new(),
            allowed_store_ids: BTreeSet::new(),
            requires_agent_delegation: true,
            requires_signed_authority: false,
            requires_mandate: false,
            requires_budget: false,
            max_amount: None,
            approval_above: None,
            max_asset_amount: None,
            approval_above_asset: None,
            max_quantity: None,
            allowed_counterparty_ids: BTreeSet::new(),
        }
    }

    /// Require explicit approval in addition to the configured capabilities.
    #[must_use]
    pub const fn with_approval(mut self) -> Self {
        self.requires_approval = true;
        self
    }

    /// Require cryptographic authorization of the semantic command.
    #[must_use]
    pub const fn with_signed_authority(mut self) -> Self {
        self.requires_signed_authority = true;
        self
    }

    /// Require a live, correctly scoped economic mandate.
    #[must_use]
    pub const fn with_mandate(mut self) -> Self {
        self.requires_mandate = true;
        self
    }

    /// Require a durable budget identifier on the economic commitment.
    #[must_use]
    pub const fn with_budget(mut self) -> Self {
        self.requires_budget = true;
        self
    }

    /// Cap the money one command may commit.
    #[must_use]
    pub fn with_max_amount(mut self, amount: Money) -> Self {
        self.max_amount = Some(amount.to_wire());
        self
    }

    /// Require explicit approval only above the supplied transaction amount.
    #[must_use]
    pub fn with_approval_above(mut self, amount: Money) -> Self {
        self.approval_above = Some(amount.to_wire());
        self
    }

    /// Cap the non-fiat asset amount one command may commit.
    #[must_use]
    pub fn with_max_asset_amount(mut self, amount: Decimal, asset: impl Into<String>) -> Self {
        self.max_asset_amount = Some(AssetAmountWire::new(amount, asset));
        self
    }

    /// Require explicit approval only above the supplied asset amount.
    #[must_use]
    pub fn with_asset_approval_above(mut self, amount: Decimal, asset: impl Into<String>) -> Self {
        self.approval_above_asset = Some(AssetAmountWire::new(amount, asset));
        self
    }

    /// Bound the exact non-monetary units one command may commit.
    #[must_use]
    pub fn with_max_quantity(mut self, quantity: Decimal) -> Self {
        self.max_quantity = Some(quantity.normalize().to_string());
        self
    }

    /// Restrict commitments to named counterparties.
    #[must_use]
    pub fn for_counterparties(
        mut self,
        counterparties: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_counterparty_ids = counterparties.into_iter().map(Into::into).collect();
        self
    }

    /// Restrict this command to explicitly configured tenant identities.
    #[must_use]
    pub fn for_tenants(mut self, tenants: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_tenant_ids = tenants.into_iter().map(Into::into).collect();
        self
    }

    /// Restrict this command to explicitly configured logical stores.
    #[must_use]
    pub fn for_stores(mut self, stores: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_store_ids = stores.into_iter().map(Into::into).collect();
        self
    }
}

/// Deterministic, versioned allow-list evaluated before kernel execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelPolicy {
    /// Stable policy revision included in every decision receipt.
    pub version: String,
    /// Command-specific rules. Commands absent from this map are denied.
    #[serde(default)]
    pub commands: BTreeMap<String, KernelCommandPolicy>,
    /// Trusted Ed25519 verifying keys, hex encoded and addressed by key ID.
    #[serde(default)]
    pub trusted_authority_keys: BTreeMap<String, String>,
}

impl KernelPolicy {
    /// Create a deny-by-default policy revision.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            commands: BTreeMap::new(),
            trusted_authority_keys: BTreeMap::new(),
        }
    }

    /// Add or replace a command rule.
    #[must_use]
    pub fn allow(mut self, command_type: impl Into<String>, rule: KernelCommandPolicy) -> Self {
        self.commands.insert(command_type.into(), rule);
        self
    }

    /// Trust an Ed25519 authority key under a stable key ID.
    #[must_use]
    pub fn with_trusted_authority_key(
        mut self,
        key_id: impl Into<String>,
        public_key: [u8; 32],
    ) -> Self {
        self.trusted_authority_keys.insert(key_id.into(), hex::encode(public_key));
        self
    }

    /// Evaluate a command without side effects.
    #[must_use]
    pub fn evaluate<T: Serialize>(
        &self,
        command: &CommandEnvelope<T>,
        now: DateTime<Utc>,
    ) -> PolicyDecisionEvidence {
        let mut reasons = Vec::new();
        if command.policy_version.as_deref().is_some_and(|version| version != self.version) {
            reasons.push("policy.version_conflict".to_string());
        }

        match self.commands.get(&command.command_type) {
            None => reasons.push("policy.command_not_allowed".to_string()),
            Some(rule) => {
                if rule.requires_tenant
                    && command.principal.tenant_id.as_deref().is_none_or(str::is_empty)
                {
                    reasons.push("policy.tenant_required".to_string());
                }
                if rule.requires_store && command.store_id.as_deref().is_none_or(str::is_empty) {
                    reasons.push("policy.store_required".to_string());
                }
                if !rule.allowed_tenant_ids.is_empty()
                    && command
                        .principal
                        .tenant_id
                        .as_ref()
                        .is_none_or(|tenant| !rule.allowed_tenant_ids.contains(tenant))
                {
                    reasons.push("policy.tenant_not_allowed".to_string());
                }
                if !rule.allowed_store_ids.is_empty()
                    && command
                        .store_id
                        .as_ref()
                        .is_none_or(|store| !rule.allowed_store_ids.contains(store))
                {
                    reasons.push("policy.store_not_allowed".to_string());
                }
                if rule.requires_agent_delegation
                    && command.principal.kind == PrincipalKind::Agent
                    && command.principal.delegated_by.as_deref().is_none_or(str::is_empty)
                {
                    reasons.push("policy.agent_delegation_required".to_string());
                }
                match &command.mandate {
                    None if rule.requires_mandate => {
                        reasons.push("policy.mandate_required".to_string());
                    }
                    Some(mandate) => {
                        if mandate.subject_id != command.principal.id {
                            reasons.push("policy.mandate_subject_mismatch".to_string());
                        }
                        if command.principal.kind == PrincipalKind::Agent
                            && command.principal.delegated_by.as_deref()
                                != Some(mandate.issued_by.as_str())
                        {
                            reasons.push("policy.mandate_issuer_mismatch".to_string());
                        }
                        if !mandate.allowed_commands.contains(&command.command_type) {
                            reasons.push("policy.mandate_command_not_allowed".to_string());
                        }
                        if mandate.tenant_id != command.principal.tenant_id {
                            reasons.push("policy.mandate_tenant_mismatch".to_string());
                        }
                        if mandate.store_id != command.store_id {
                            reasons.push("policy.mandate_store_mismatch".to_string());
                        }
                        if mandate.issued_at > now {
                            reasons.push("policy.mandate_not_yet_valid".to_string());
                        }
                        if mandate.expires_at <= now {
                            reasons.push("policy.mandate_expired".to_string());
                        }
                    }
                    None => {}
                }
                let commitment_money = command
                    .commitment
                    .as_ref()
                    .and_then(|commitment| commitment.money().ok().flatten());
                let commitment_asset = command
                    .commitment
                    .as_ref()
                    .and_then(|commitment| commitment.asset().ok().flatten());
                let commitment_quantity = command
                    .commitment
                    .as_ref()
                    .and_then(|commitment| commitment.parsed_quantity().ok().flatten());
                if rule.requires_budget
                    && command
                        .commitment
                        .as_ref()
                        .and_then(|commitment| commitment.budget_id.as_deref())
                        .is_none_or(str::is_empty)
                {
                    reasons.push("policy.budget_required".to_string());
                }
                if command.commitment.as_ref().is_some_and(|commitment| commitment.amount.is_some())
                    && commitment_money.is_none()
                {
                    reasons.push("policy.commitment_amount_invalid".to_string());
                }
                if command
                    .commitment
                    .as_ref()
                    .is_some_and(|commitment| commitment.asset_amount.is_some())
                    && commitment_asset.is_none()
                {
                    reasons.push("policy.commitment_asset_amount_invalid".to_string());
                }
                if command.commitment.as_ref().is_some_and(|commitment| {
                    commitment.quantity.is_some() && commitment_quantity.is_none()
                }) {
                    reasons.push("policy.commitment_quantity_invalid".to_string());
                }
                if let Some(limit_wire) = &rule.max_amount {
                    match (Money::try_from(limit_wire.clone()), commitment_money) {
                        (Err(_), _) => reasons.push("policy.max_amount_invalid".to_string()),
                        (Ok(limit), _) if limit.is_negative() => {
                            reasons.push("policy.max_amount_invalid".to_string());
                        }
                        (Ok(_), None) => {
                            reasons.push("policy.commitment_amount_required".to_string());
                        }
                        (Ok(limit), Some(amount)) if limit.currency() != amount.currency() => {
                            reasons.push("policy.commitment_currency_mismatch".to_string());
                        }
                        (Ok(limit), Some(amount)) if amount.amount() > limit.amount() => {
                            reasons.push("policy.max_amount_exceeded".to_string());
                        }
                        _ => {}
                    }
                }
                if let Some(limit_wire) = &rule.max_asset_amount {
                    match (limit_wire.validate(), commitment_asset) {
                        (Err(_), _) => reasons.push("policy.max_asset_amount_invalid".to_string()),
                        (Ok(_), None) => {
                            reasons.push("policy.commitment_asset_amount_required".to_string());
                        }
                        (Ok(_), Some((_, asset))) if limit_wire.asset != asset => {
                            reasons.push("policy.commitment_asset_mismatch".to_string());
                        }
                        (Ok(limit), Some((amount, _))) if amount > limit => {
                            reasons.push("policy.max_asset_amount_exceeded".to_string());
                        }
                        _ => {}
                    }
                }
                if let Some(limit_wire) = &rule.max_quantity {
                    match (limit_wire.parse::<Decimal>(), commitment_quantity) {
                        (Err(_), _) => reasons.push("policy.max_quantity_invalid".to_string()),
                        (Ok(limit), _) if limit.is_sign_negative() => {
                            reasons.push("policy.max_quantity_invalid".to_string());
                        }
                        (Ok(_), None) => {
                            reasons.push("policy.commitment_quantity_required".to_string());
                        }
                        (Ok(limit), Some(quantity)) if quantity > limit => {
                            reasons.push("policy.max_quantity_exceeded".to_string());
                        }
                        _ => {}
                    }
                }
                if !rule.allowed_counterparty_ids.is_empty() {
                    match command
                        .commitment
                        .as_ref()
                        .and_then(|commitment| commitment.counterparty_id.as_ref())
                    {
                        None => reasons.push("policy.counterparty_required".to_string()),
                        Some(counterparty)
                            if !rule.allowed_counterparty_ids.contains(counterparty) =>
                        {
                            reasons.push("policy.counterparty_not_allowed".to_string());
                        }
                        Some(_) => {}
                    }
                }
                if rule.requires_signed_authority {
                    match &command.authority {
                        None => reasons.push("policy.signed_authority_required".to_string()),
                        Some(authority) => {
                            if authority.issued_at > now {
                                reasons.push("policy.authority_not_yet_valid".to_string());
                            }
                            if authority.expires_at <= now {
                                reasons.push("policy.authority_expired".to_string());
                            }
                            if command.principal.kind == PrincipalKind::Agent
                                && command.principal.delegated_by.as_deref()
                                    != Some(authority.issuer.as_str())
                            {
                                reasons.push("policy.authority_issuer_mismatch".to_string());
                            }
                            match self.trusted_authority_keys.get(&authority.key_id) {
                                None => reasons.push("policy.authority_key_untrusted".to_string()),
                                Some(public_key) => {
                                    if !verify_authority(command, authority, public_key) {
                                        reasons
                                            .push("policy.authority_signature_invalid".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                let held: BTreeSet<&str> =
                    command.principal.capabilities.iter().map(String::as_str).collect();
                for capability in &rule.required_capabilities {
                    if !held.contains(capability.as_str()) {
                        reasons.push(format!("policy.capability_missing:{capability}"));
                    }
                }
                let approval_threshold_exceeded = match &rule.approval_above {
                    None => false,
                    Some(threshold_wire) => match Money::try_from(threshold_wire.clone()) {
                        Err(_) => {
                            reasons.push("policy.approval_threshold_invalid".to_string());
                            false
                        }
                        Ok(threshold) if threshold.is_negative() => {
                            reasons.push("policy.approval_threshold_invalid".to_string());
                            false
                        }
                        Ok(_) if commitment_money.is_none() => {
                            if !reasons.iter().any(|reason| {
                                reason.as_str() == "policy.commitment_amount_required"
                            }) {
                                reasons.push("policy.commitment_amount_required".to_string());
                            }
                            false
                        }
                        Ok(threshold)
                            if commitment_money.is_some_and(|amount| {
                                threshold.currency() != amount.currency()
                            }) =>
                        {
                            if !reasons.iter().any(|reason| {
                                reason.as_str() == "policy.commitment_currency_mismatch"
                            }) {
                                reasons.push("policy.commitment_currency_mismatch".to_string());
                            }
                            false
                        }
                        Ok(threshold) => commitment_money
                            .is_some_and(|amount| amount.amount() > threshold.amount()),
                    },
                };
                let asset_approval_threshold_exceeded = match &rule.approval_above_asset {
                    None => false,
                    Some(threshold_wire) => match threshold_wire.validate() {
                        Err(_) => {
                            reasons.push("policy.asset_approval_threshold_invalid".to_string());
                            false
                        }
                        Ok(_) if commitment_asset.is_none() => {
                            if !reasons.iter().any(|reason| {
                                reason.as_str() == "policy.commitment_asset_amount_required"
                            }) {
                                reasons.push("policy.commitment_asset_amount_required".to_string());
                            }
                            false
                        }
                        Ok(_)
                            if commitment_asset
                                .is_some_and(|(_, asset)| threshold_wire.asset != asset) =>
                        {
                            if !reasons
                                .iter()
                                .any(|reason| reason.as_str() == "policy.commitment_asset_mismatch")
                            {
                                reasons.push("policy.commitment_asset_mismatch".to_string());
                            }
                            false
                        }
                        Ok(threshold) => {
                            commitment_asset.is_some_and(|(amount, _)| amount > threshold)
                        }
                    },
                };
                if rule.requires_approval
                    || approval_threshold_exceeded
                    || asset_approval_threshold_exceeded
                {
                    match &command.approval {
                        None => reasons.push("policy.approval_required".to_string()),
                        Some(approval) => {
                            if approval.approval_id.trim().is_empty() {
                                reasons.push("policy.approval_id_missing".to_string());
                            }
                            if approval.approved_by.trim().is_empty() {
                                reasons.push("policy.approver_missing".to_string());
                            }
                            if approval.scope != command.command_type {
                                reasons.push("policy.approval_scope_mismatch".to_string());
                            }
                            if approval.tenant_id != command.principal.tenant_id {
                                reasons.push("policy.approval_tenant_mismatch".to_string());
                            }
                            if approval.store_id != command.store_id {
                                reasons.push("policy.approval_store_mismatch".to_string());
                            }
                            if approval.idempotency_key.as_deref()
                                != Some(command.idempotency_key.as_str())
                            {
                                reasons.push("policy.approval_intent_mismatch".to_string());
                            }
                            if approval.approved_at > now {
                                reasons.push("policy.approval_not_yet_valid".to_string());
                            }
                            if approval.expires_at.is_some_and(|expires_at| expires_at <= now) {
                                reasons.push("policy.approval_expired".to_string());
                            }
                        }
                    }
                }
            }
        }

        PolicyDecisionEvidence {
            policy_version: self.version.clone(),
            decision_id: Uuid::new_v4().to_string(),
            allowed: reasons.is_empty(),
            reason_codes: reasons,
        }
    }
}

/// Canonical SHA-256 digest signed by command authorities.
///
/// The command must already contain the authority claim (issuer, key ID, and
/// validity window), normally with an empty `signature`. Those fields are part
/// of the digest; only the signature bytes themselves are excluded.
pub fn authority_signing_hash<T: Serialize>(
    command: &CommandEnvelope<T>,
) -> Result<[u8; 32], KernelContractError> {
    let authority = command.authority.as_ref().map(|authority| {
        serde_json::json!({
            "issuer": authority.issuer,
            "key_id": authority.key_id,
            "issued_at": authority.issued_at,
            "expires_at": authority.expires_at,
        })
    });
    let mut value = serde_json::json!({
        "contract_version": command.contract_version,
        "idempotency_key": command.idempotency_key,
        "command_type": command.command_type,
        "principal": command.principal,
        "store_id": command.store_id,
        "correlation_id": command.correlation_id,
        "causation_id": command.causation_id,
        "expected_version": command.expected_version,
        "policy_version": command.policy_version,
        "approval": command.approval,
        "authority": authority,
        "deadline": command.deadline,
        "payload": command.payload,
        "issued_at": command.issued_at,
    });
    if let serde_json::Value::Object(fields) = &mut value {
        if let Some(mandate) = &command.mandate {
            fields.insert(
                "mandate".into(),
                serde_json::to_value(mandate)
                    .map_err(|error| KernelContractError::Serialization(error.to_string()))?,
            );
        }
        if let Some(commitment) = &command.commitment {
            fields.insert(
                "commitment".into(),
                serde_json::to_value(commitment)
                    .map_err(|error| KernelContractError::Serialization(error.to_string()))?,
            );
        }
    }
    let canonical = stateset_crypto::canonicalize::canonicalize_json(&value)
        .map_err(|error| KernelContractError::Serialization(error.to_string()))?;
    Ok(Sha256::digest(canonical.as_bytes()).into())
}

fn verify_authority<T: Serialize>(
    command: &CommandEnvelope<T>,
    authority: &AuthorityEvidence,
    public_key_hex: &str,
) -> bool {
    let Ok(hash) = authority_signing_hash(command) else { return false };
    let Ok(public_key) = hex::decode(public_key_hex) else { return false };
    let Ok(public_key): Result<[u8; 32], _> = public_key.try_into() else { return false };
    let Ok(signature) = hex::decode(&authority.signature) else { return false };
    let Ok(signature): Result<[u8; 64], _> = signature.try_into() else { return false };
    stateset_crypto::sign::verify_event_signature(&hash, &signature, &public_key)
}

/// Durable, machine-readable outcome of a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt<T> {
    /// Wire-contract version.
    pub contract_version: String,
    /// Unique receipt identity.
    pub receipt_id: Uuid,
    /// Command this receipt answers.
    pub command_id: Uuid,
    /// Retry key copied from the command.
    pub idempotency_key: String,
    /// Stable namespaced command name.
    pub command_type: String,
    /// Outcome category.
    pub status: ExecutionStatus,
    /// Applied domain result. A preview may omit it when no aggregate exists yet.
    pub result: Option<T>,
    /// Stable error code, never parsed from prose.
    pub error_code: Option<String>,
    /// Human-readable diagnostic.
    pub error_message: Option<String>,
    /// Machine-readable retry instruction.
    pub retry: RetryDisposition,
    /// Affected aggregate category.
    pub aggregate_type: Option<String>,
    /// Affected aggregate identity.
    pub aggregate_id: Option<String>,
    /// Aggregate version observed before execution.
    pub version_before: Option<i32>,
    /// Aggregate version after execution.
    pub version_after: Option<i32>,
    /// Durable events committed atomically with the mutation.
    #[serde(default)]
    pub event_ids: Vec<Uuid>,
    /// Policy evidence used for this outcome.
    pub policy: Option<PolicyDecisionEvidence>,
    /// Identity, mandate, authority, and resource commitment behind the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economic_context: Option<EconomicReceiptContext>,
    /// Optional hash anchoring the corresponding audit record.
    pub audit_hash: Option<String>,
    /// Time execution began.
    pub started_at: DateTime<Utc>,
    /// Time the outcome became final.
    pub completed_at: DateTime<Utc>,
}

impl<T> ExecutionReceipt<T> {
    /// Build a successful receipt from an applied command.
    pub fn succeeded(command: &CommandEnvelope<impl Sized>, result: T) -> Self {
        let now = Utc::now();
        Self {
            contract_version: KERNEL_CONTRACT_VERSION.into(),
            receipt_id: Uuid::new_v4(),
            command_id: command.command_id,
            idempotency_key: command.idempotency_key.clone(),
            command_type: command.command_type.clone(),
            status: ExecutionStatus::Succeeded,
            result: Some(result),
            error_code: None,
            error_message: None,
            retry: RetryDisposition::SameKey,
            aggregate_type: None,
            aggregate_id: None,
            version_before: None,
            version_after: None,
            event_ids: Vec::new(),
            policy: None,
            economic_context: Some(EconomicReceiptContext::from_command(command)),
            audit_hash: None,
            started_at: now,
            completed_at: now,
        }
    }
}

/// Invalid kernel envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KernelContractError {
    /// A required string/id was empty.
    MissingField(&'static str),
    /// A field was present but violated its exact wire contract.
    InvalidField(&'static str, String),
    /// The consumer does not support this wire version.
    UnsupportedVersion(String),
    /// Semantic command canonicalization failed.
    Serialization(String),
}

impl fmt::Display for KernelContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "kernel command is missing {field}"),
            Self::InvalidField(field, message) => {
                write!(f, "kernel command has invalid {field}: {message}")
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported kernel contract version: {version}")
            }
            Self::Serialization(message) => {
                write!(f, "kernel command serialization failed: {message}")
            }
        }
    }
}

impl std::error::Error for KernelContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> KernelPrincipal {
        KernelPrincipal {
            id: "agent:buyer-7".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-42".into()),
            capabilities: vec!["payments.create".into()],
        }
    }

    #[test]
    fn commands_default_to_preview_and_require_a_retry_key() {
        let mut command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        assert_eq!(command.mode, ExecutionMode::Preview);
        assert!(command.validate_contract().is_ok());

        command.idempotency_key.clear();
        assert_eq!(
            command.validate_contract(),
            Err(KernelContractError::MissingField("idempotency_key"))
        );
    }

    #[test]
    fn receipt_preserves_command_identity_across_json() {
        let command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        let receipt = ExecutionReceipt::succeeded(&command, "pay_123".to_string());
        let json = serde_json::to_string(&receipt).expect("receipt should serialize");
        let decoded: ExecutionReceipt<String> =
            serde_json::from_str(&json).expect("receipt should deserialize");

        assert_eq!(decoded.command_id, command.command_id);
        assert_eq!(decoded.idempotency_key, command.idempotency_key);
        assert_eq!(decoded.status, ExecutionStatus::Succeeded);
    }

    #[test]
    fn policy_is_deny_by_default_and_checks_capability_version_and_approval() {
        let policy = KernelPolicy::new("policy-2").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"]).with_approval(),
        );
        let now = Utc::now();
        let mut command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        command.policy_version = Some("policy-1".into());
        let denied = policy.evaluate(&command, now);
        assert!(!denied.allowed);
        assert!(denied.reason_codes.contains(&"policy.version_conflict".to_string()));
        assert!(denied.reason_codes.contains(&"policy.approval_required".to_string()));

        command.policy_version = Some("policy-2".into());
        command.approval = Some(ApprovalEvidence {
            approval_id: "approval-1".into(),
            approved_by: "user-42".into(),
            scope: "payments.create".into(),
            tenant_id: Some("tenant-1".into()),
            store_id: Some("store-1".into()),
            idempotency_key: Some("retry-1".into()),
            approved_at: now,
            expires_at: None,
        });
        assert!(policy.evaluate(&command, now).allowed);

        let mut wrong_store = command.clone();
        wrong_store.store_id = Some("store-2".into());
        assert!(
            policy
                .evaluate(&wrong_store, now)
                .reason_codes
                .contains(&"policy.approval_store_mismatch".to_string())
        );

        let mut unscoped = command.clone();
        unscoped.store_id = None;
        unscoped.principal.tenant_id = None;
        unscoped.principal.delegated_by = None;
        let denied = policy.evaluate(&unscoped, now);
        assert!(denied.reason_codes.contains(&"policy.store_required".to_string()));
        assert!(denied.reason_codes.contains(&"policy.tenant_required".to_string()));
        assert!(denied.reason_codes.contains(&"policy.agent_delegation_required".to_string()));

        command.command_type = "ledger.post".into();
        assert!(
            policy
                .evaluate(&command, now)
                .reason_codes
                .contains(&"policy.command_not_allowed".to_string())
        );
    }

    #[test]
    fn policy_binds_commands_to_explicit_tenant_and_store_allowlists() {
        let policy = KernelPolicy::new("policy-tenant-scope").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"])
                .for_tenants(["tenant-1"])
                .for_stores(["store-1"]),
        );
        let now = Utc::now();
        let mut command = CommandEnvelope::preview("payments.create", "retry-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        assert!(policy.evaluate(&command, now).allowed);

        command.principal.tenant_id = Some("tenant-2".into());
        command.store_id = Some("store-2".into());
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.tenant_not_allowed".to_string()));
        assert!(denied.reason_codes.contains(&"policy.store_not_allowed".to_string()));
    }

    #[test]
    fn policy_enforces_mandates_counterparties_and_exact_money_tiers() {
        let now = Utc::now();
        let policy = KernelPolicy::new("economic-policy-v1").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"])
                .with_mandate()
                .with_budget()
                .with_max_amount(Money::new(Decimal::new(50000, 2), CurrencyCode::USD))
                .with_approval_above(Money::new(Decimal::new(10000, 2), CurrencyCode::USD))
                .with_max_quantity(Decimal::ONE)
                .for_counterparties(["merchant:trusted"]),
        );
        let mut command = CommandEnvelope::preview("payments.create", "economic-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        command.mandate = Some(EconomicMandate {
            mandate_id: "mandate:buy-bike".into(),
            subject_id: "agent:buyer-7".into(),
            issued_by: "user-42".into(),
            objective: "Buy the best road bike within budget".into(),
            allowed_commands: BTreeSet::from(["payments.create".into()]),
            tenant_id: Some("tenant-1".into()),
            store_id: Some("store-1".into()),
            issued_at: now - chrono::Duration::minutes(1),
            expires_at: now + chrono::Duration::hours(1),
        });
        command.commitment = Some(EconomicCommitment {
            budget_id: Some("budget:bike".into()),
            amount: Some(Money::new(Decimal::new(9900, 2), CurrencyCode::USD).to_wire()),
            asset_amount: None,
            counterparty_id: Some("merchant:trusted".into()),
            quantity: Some("1".into()),
            evidence: vec!["quote:q-17".into()],
        });
        assert!(policy.evaluate(&command, now).allowed);

        command.commitment.as_mut().expect("commitment").budget_id = None;
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.budget_required".to_string()));
        command.commitment.as_mut().expect("commitment").budget_id = Some("budget:bike".into());

        command.commitment.as_mut().expect("commitment").amount =
            Some(Money::new(Decimal::new(25000, 2), CurrencyCode::USD).to_wire());
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.approval_required".to_string()));

        command.commitment.as_mut().expect("commitment").amount =
            Some(Money::new(Decimal::new(50100, 2), CurrencyCode::USD).to_wire());
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.max_amount_exceeded".to_string()));

        command.commitment.as_mut().expect("commitment").counterparty_id =
            Some("merchant:unknown".into());
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.counterparty_not_allowed".to_string()));

        command.commitment.as_mut().expect("commitment").quantity = Some("2".into());
        let denied = policy.evaluate(&command, now);
        assert!(denied.reason_codes.contains(&"policy.max_quantity_exceeded".to_string()));
    }

    #[test]
    fn policy_enforces_exact_asset_commitments_and_approval_tiers() {
        let now = Utc::now();
        let policy = KernelPolicy::new("asset-policy-v1").allow(
            "a2a.escrow.create",
            KernelCommandPolicy::requiring([] as [&str; 0])
                .with_max_asset_amount(Decimal::new(10000, 2), "USDC")
                .with_asset_approval_above(Decimal::new(2500, 2), "USDC"),
        );
        let mut command = CommandEnvelope::preview(
            "a2a.escrow.create",
            "asset-1",
            agent(),
            serde_json::json!({}),
        );
        command.store_id = Some("store-1".into());
        command.commitment = Some(EconomicCommitment::for_asset(Decimal::new(2000, 2), "USDC"));
        assert!(policy.evaluate(&command, now).allowed);

        command.commitment = Some(EconomicCommitment::for_asset(Decimal::new(3000, 2), "USDC"));
        assert!(
            policy
                .evaluate(&command, now)
                .reason_codes
                .contains(&"policy.approval_required".to_string())
        );

        command.commitment = Some(EconomicCommitment::for_asset(Decimal::new(10100, 2), "USDC"));
        assert!(
            policy
                .evaluate(&command, now)
                .reason_codes
                .contains(&"policy.max_asset_amount_exceeded".to_string())
        );

        command.commitment = Some(EconomicCommitment::for_asset(Decimal::new(2000, 2), "ETH"));
        assert!(
            policy
                .evaluate(&command, now)
                .reason_codes
                .contains(&"policy.commitment_asset_mismatch".to_string())
        );
    }

    #[test]
    fn contract_rejects_ambiguous_asset_and_fiat_commitments() {
        let mut command = CommandEnvelope::preview(
            "a2a.escrow.create",
            "asset-ambiguous",
            agent(),
            serde_json::json!({}),
        );
        command.store_id = Some("store-1".into());
        let mut commitment = EconomicCommitment::for_asset(Decimal::ONE, "USDC");
        commitment.amount = Some(Money::new(Decimal::ONE, CurrencyCode::USD).to_wire());
        command.commitment = Some(commitment);
        assert!(matches!(
            command.validate_contract(),
            Err(KernelContractError::InvalidField("commitment.asset_amount", _))
        ));
    }

    #[test]
    fn receipt_carries_portable_economic_accountability_context() {
        let now = Utc::now();
        let mut command = CommandEnvelope::preview("payments.create", "receipt-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        command.correlation_id = Some(Uuid::new_v4());
        command.commitment = Some(EconomicCommitment {
            budget_id: Some("budget:ops".into()),
            amount: Some(Money::new(Decimal::new(4200, 2), CurrencyCode::USD).to_wire()),
            asset_amount: None,
            counterparty_id: Some("merchant:one".into()),
            quantity: None,
            evidence: vec![],
        });
        command.approval = Some(ApprovalEvidence {
            approval_id: "approval:42".into(),
            approved_by: "user-42".into(),
            scope: "payments.create".into(),
            tenant_id: Some("tenant-1".into()),
            store_id: Some("store-1".into()),
            idempotency_key: Some("receipt-1".into()),
            approved_at: now,
            expires_at: None,
        });

        let receipt = ExecutionReceipt::succeeded(&command, "payment:1");
        let context = receipt.economic_context.expect("economic context");
        assert_eq!(context.principal.id, "agent:buyer-7");
        assert_eq!(context.approval_id.as_deref(), Some("approval:42"));
        assert_eq!(context.commitment.and_then(|value| value.budget_id), Some("budget:ops".into()));
    }

    #[test]
    fn signed_authority_is_bound_to_the_semantic_intent() {
        let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
        let policy = KernelPolicy::new("policy-1")
            .allow(
                "payments.create",
                KernelCommandPolicy::requiring(["payments.create"]).with_signed_authority(),
            )
            .with_trusted_authority_key("delegator-key-1", public_key);
        let now = Utc::now();
        let mut command =
            CommandEnvelope::preview("payments.create", "retry-signed-1", agent(), 42_u8);
        command.store_id = Some("store-1".into());
        command.policy_version = Some("policy-1".into());
        command.approval = Some(ApprovalEvidence {
            approval_id: "approval-signed-1".into(),
            approved_by: "user-42".into(),
            scope: "payments.create".into(),
            tenant_id: Some("tenant-1".into()),
            store_id: Some("store-1".into()),
            idempotency_key: Some("retry-signed-1".into()),
            approved_at: now,
            expires_at: Some(now + chrono::Duration::minutes(5)),
        });
        command.authority = Some(AuthorityEvidence {
            issuer: "user-42".into(),
            key_id: "delegator-key-1".into(),
            issued_at: now,
            expires_at: now + chrono::Duration::minutes(5),
            signature: String::new(),
        });
        command.commitment = Some(EconomicCommitment {
            budget_id: Some("budget:signed".into()),
            amount: Some(Money::new(Decimal::new(4200, 2), CurrencyCode::USD).to_wire()),
            asset_amount: None,
            counterparty_id: Some("merchant:signed".into()),
            quantity: None,
            evidence: vec![],
        });
        let hash = authority_signing_hash(&command).expect("canonical intent");
        let signature = stateset_crypto::sign::sign_event_hash(&hash, &private_key).expect("sign");
        command.authority.as_mut().expect("authority claim").signature = hex::encode(signature);
        assert!(policy.evaluate(&command, now).allowed);

        let assert_signature_invalid = |changed: &CommandEnvelope<u8>| {
            let denied = policy.evaluate(changed, now);
            assert!(
                denied.reason_codes.contains(&"policy.authority_signature_invalid".to_string()),
                "expected signature rejection, got {:?}",
                denied.reason_codes
            );
        };

        let mut changed = command.clone();
        changed.payload = 43;
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.principal.tenant_id = Some("tenant-2".into());
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.store_id = Some("store-2".into());
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.expected_version = Some(2);
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.approval.as_mut().expect("approval").approval_id = "approval-substitute".into();
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.deadline = Some(now + chrono::Duration::hours(1));
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.commitment.as_mut().expect("commitment").budget_id = Some("budget:other".into());
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.authority.as_mut().expect("authority").issued_at -= chrono::Duration::minutes(1);
        assert_signature_invalid(&changed);
        let mut changed = command.clone();
        changed.authority.as_mut().expect("authority").expires_at += chrono::Duration::hours(1);
        assert_signature_invalid(&changed);

        let mut absent = command.clone();
        absent.authority = None;
        assert!(
            policy
                .evaluate(&absent, now)
                .reason_codes
                .contains(&"policy.signed_authority_required".to_string())
        );
        let mut untrusted = command.clone();
        untrusted.authority.as_mut().expect("authority").key_id = "unknown-key".into();
        assert!(
            policy
                .evaluate(&untrusted, now)
                .reason_codes
                .contains(&"policy.authority_key_untrusted".to_string())
        );
        let mut wrong_issuer = command.clone();
        wrong_issuer.authority.as_mut().expect("authority").issuer = "user:other".into();
        assert!(
            policy
                .evaluate(&wrong_issuer, now)
                .reason_codes
                .contains(&"policy.authority_issuer_mismatch".to_string())
        );
        let mut expired = command.clone();
        {
            let authority = expired.authority.as_mut().expect("authority");
            authority.issued_at = now - chrono::Duration::minutes(10);
            authority.expires_at = now;
            authority.signature.clear();
        }
        let hash = authority_signing_hash(&expired).expect("expired canonical intent");
        let signature = stateset_crypto::sign::sign_event_hash(&hash, &private_key).expect("sign");
        expired.authority.as_mut().expect("authority").signature = hex::encode(signature);
        assert!(
            policy
                .evaluate(&expired, now)
                .reason_codes
                .contains(&"policy.authority_expired".to_string())
        );

        let mut future = command.clone();
        {
            let authority = future.authority.as_mut().expect("authority");
            authority.issued_at = now + chrono::Duration::minutes(1);
            authority.expires_at = now + chrono::Duration::minutes(10);
            authority.signature.clear();
        }
        let hash = authority_signing_hash(&future).expect("future canonical intent");
        let signature = stateset_crypto::sign::sign_event_hash(&hash, &private_key).expect("sign");
        future.authority.as_mut().expect("authority").signature = hex::encode(signature);
        assert!(
            policy
                .evaluate(&future, now)
                .reason_codes
                .contains(&"policy.authority_not_yet_valid".to_string())
        );
    }
}
