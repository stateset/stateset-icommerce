//! First-class identities, authority tiers, intents, and portable receipts for
//! autonomous economic actors.

use crate::{
    AssetAmountWire, CommandEnvelope, EconomicCommitment, ExecutionReceipt, ExecutionStatus,
    KernelCommandPolicy, KernelContractError, KernelPolicy, KernelPrincipal, Money, MoneyWire,
    PolicyDecisionEvidence, PrincipalKind,
};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// Credential retained with an economic agent's operator-owned identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCredential {
    pub id: String,
    pub credential_type: String,
    pub issuer: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub claims: BTreeMap<String, String>,
}

/// An autonomous actor and the principal from which its authority derives.
///
/// This is trusted runtime configuration, not a model-provided command field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicAgent {
    pub id: String,
    pub principal_id: String,
    pub role: String,
    pub tenant_id: String,
    pub store_id: String,
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
    #[serde(default)]
    pub budget_ids: BTreeSet<String>,
    #[serde(default)]
    pub credentials: Vec<AgentCredential>,
    /// Hex-encoded Ed25519 public key used to verify this agent's actions.
    pub public_key: Option<String>,
}

impl EconomicAgent {
    /// Create an agent and bind it to one principal, role, tenant, and store.
    pub fn new(
        id: impl Into<String>,
        principal_id: impl Into<String>,
        role: impl Into<String>,
        tenant_id: impl Into<String>,
        store_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            principal_id: principal_id.into(),
            role: role.into(),
            tenant_id: tenant_id.into(),
            store_id: store_id.into(),
            capabilities: BTreeSet::new(),
            budget_ids: BTreeSet::new(),
            credentials: Vec::new(),
            public_key: None,
        }
    }

    #[must_use]
    pub fn with_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.capabilities = capabilities.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_budgets(mut self, budget_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.budget_ids = budget_ids.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_credentials(mut self, credentials: Vec<AgentCredential>) -> Self {
        self.credentials = credentials;
        self
    }

    #[must_use]
    pub fn with_public_key(mut self, public_key: impl Into<String>) -> Self {
        self.public_key = Some(public_key.into());
        self
    }

    /// Validate the complete operator-owned identity document.
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), KernelContractError> {
        for (field, value) in [
            ("agent.id", self.id.as_str()),
            ("agent.principal_id", self.principal_id.as_str()),
            ("agent.role", self.role.as_str()),
            ("agent.tenant_id", self.tenant_id.as_str()),
            ("agent.store_id", self.store_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(KernelContractError::MissingField(field));
            }
        }
        if self.id == self.principal_id {
            return Err(KernelContractError::InvalidField(
                "agent.principal_id",
                "an autonomous agent must be delegated by a distinct principal".into(),
            ));
        }
        if self.capabilities.iter().any(|value| value.trim().is_empty())
            || self.budget_ids.iter().any(|value| value.trim().is_empty())
        {
            return Err(KernelContractError::InvalidField(
                "agent.capabilities",
                "capability and budget identifiers cannot be empty".into(),
            ));
        }
        if let Some(key) = &self.public_key {
            let decoded = hex::decode(key).map_err(|error| {
                KernelContractError::InvalidField("agent.public_key", error.to_string())
            })?;
            if decoded.len() != 32 {
                return Err(KernelContractError::InvalidField(
                    "agent.public_key",
                    "Ed25519 public keys must contain exactly 32 bytes".into(),
                ));
            }
        }
        for credential in &self.credentials {
            if credential.id.trim().is_empty()
                || credential.credential_type.trim().is_empty()
                || credential.issuer.trim().is_empty()
            {
                return Err(KernelContractError::InvalidField(
                    "agent.credentials",
                    "credential id, type, and issuer are required".into(),
                ));
            }
            if credential.issued_at > now
                || credential.expires_at.is_some_and(|expires_at| expires_at <= now)
            {
                return Err(KernelContractError::InvalidField(
                    "agent.credentials",
                    "credentials must be currently valid".into(),
                ));
            }
        }
        Ok(())
    }

    /// Convert trusted agent configuration into a kernel principal.
    #[must_use]
    pub fn kernel_principal(&self) -> KernelPrincipal {
        KernelPrincipal {
            id: self.id.clone(),
            kind: PrincipalKind::Agent,
            tenant_id: Some(self.tenant_id.clone()),
            delegated_by: Some(self.principal_id.clone()),
            capabilities: self.capabilities.iter().cloned().collect(),
        }
    }

    /// Create a scoped, preview-by-default command for this agent.
    #[must_use]
    pub fn command<T>(
        &self,
        command_type: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload: T,
    ) -> CommandEnvelope<T> {
        let mut command = CommandEnvelope::preview(
            command_type,
            idempotency_key,
            self.kernel_principal(),
            payload,
        );
        command.store_id = Some(self.store_id.clone());
        command
    }
}

/// A fiat or non-fiat exact value used by an authority tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AuthorityAmount {
    Money(MoneyWire),
    Asset(AssetAmountWire),
}

/// Bounded authority for one kernel command.
///
/// Values through `autonomous_up_to` need no human approval. Higher values up
/// to `approval_up_to` require approval. Anything higher is denied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicAuthorityRule {
    pub capability: String,
    pub autonomous_up_to: AuthorityAmount,
    pub approval_up_to: AuthorityAmount,
    #[serde(default)]
    pub requires_budget: bool,
    #[serde(default)]
    pub allowed_counterparty_ids: BTreeSet<String>,
    /// Optional ceiling for inventory, capacity, or other exact units.
    #[serde(default)]
    pub max_quantity: Option<String>,
}

impl EconomicAuthorityRule {
    #[must_use]
    pub fn money(
        capability: impl Into<String>,
        autonomous_up_to: Money,
        approval_up_to: Money,
    ) -> Self {
        Self {
            capability: capability.into(),
            autonomous_up_to: AuthorityAmount::Money(autonomous_up_to.to_wire()),
            approval_up_to: AuthorityAmount::Money(approval_up_to.to_wire()),
            requires_budget: false,
            allowed_counterparty_ids: BTreeSet::new(),
            max_quantity: None,
        }
    }

    #[must_use]
    pub fn asset(
        capability: impl Into<String>,
        autonomous_up_to: Decimal,
        approval_up_to: Decimal,
        asset: impl Into<String>,
    ) -> Self {
        let asset = asset.into();
        Self {
            capability: capability.into(),
            autonomous_up_to: AuthorityAmount::Asset(AssetAmountWire::new(
                autonomous_up_to,
                asset.clone(),
            )),
            approval_up_to: AuthorityAmount::Asset(AssetAmountWire::new(approval_up_to, asset)),
            requires_budget: false,
            allowed_counterparty_ids: BTreeSet::new(),
            max_quantity: None,
        }
    }

    #[must_use]
    pub const fn with_budget(mut self) -> Self {
        self.requires_budget = true;
        self
    }

    #[must_use]
    pub fn for_counterparties(
        mut self,
        counterparties: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_counterparty_ids = counterparties.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_max_quantity(mut self, quantity: Decimal) -> Self {
        self.max_quantity = Some(quantity.normalize().to_string());
        self
    }

    fn compile(&self, agent: &EconomicAgent) -> Result<KernelCommandPolicy, KernelContractError> {
        if self.capability.trim().is_empty() {
            return Err(KernelContractError::MissingField("authority.capability"));
        }
        if !agent.capabilities.contains(&self.capability) {
            return Err(KernelContractError::InvalidField(
                "authority.capability",
                format!("agent does not hold capability {}", self.capability),
            ));
        }
        let mut rule = KernelCommandPolicy::requiring([self.capability.clone()])
            .for_tenants([agent.tenant_id.clone()])
            .for_stores([agent.store_id.clone()])
            .for_counterparties(self.allowed_counterparty_ids.iter().cloned());
        if self.requires_budget {
            rule = rule.with_budget();
        }
        match (&self.autonomous_up_to, &self.approval_up_to) {
            (AuthorityAmount::Money(autonomous), AuthorityAmount::Money(maximum)) => {
                let autonomous = Money::try_from(autonomous.clone()).map_err(|error| {
                    KernelContractError::InvalidField(
                        "authority.autonomous_up_to",
                        error.to_string(),
                    )
                })?;
                let maximum = Money::try_from(maximum.clone()).map_err(|error| {
                    KernelContractError::InvalidField("authority.approval_up_to", error.to_string())
                })?;
                if autonomous.is_negative()
                    || maximum.is_negative()
                    || autonomous.currency() != maximum.currency()
                    || autonomous.amount() > maximum.amount()
                {
                    return Err(KernelContractError::InvalidField(
                        "authority.approval_up_to",
                        "authority tiers must be non-negative, use one currency, and increase"
                            .into(),
                    ));
                }
                rule = rule.with_max_amount(maximum).with_approval_above(autonomous);
            }
            (AuthorityAmount::Asset(autonomous), AuthorityAmount::Asset(maximum)) => {
                let autonomous_amount = autonomous.validate()?;
                let maximum_amount = maximum.validate()?;
                if autonomous.asset != maximum.asset || autonomous_amount > maximum_amount {
                    return Err(KernelContractError::InvalidField(
                        "authority.approval_up_to",
                        "authority tiers must use one asset and increase".into(),
                    ));
                }
                if self.requires_budget {
                    return Err(KernelContractError::InvalidField(
                        "authority.requires_budget",
                        "durable asset budgets require reservation and release semantics".into(),
                    ));
                }
                rule = rule
                    .with_max_asset_amount(maximum_amount, maximum.asset.clone())
                    .with_asset_approval_above(autonomous_amount, autonomous.asset.clone());
            }
            _ => {
                return Err(KernelContractError::InvalidField(
                    "authority.approval_up_to",
                    "authority tiers cannot mix fiat money and non-fiat assets".into(),
                ));
            }
        }
        if let Some(quantity) = &self.max_quantity {
            let quantity = quantity.parse::<Decimal>().map_err(|error| {
                KernelContractError::InvalidField("authority.max_quantity", error.to_string())
            })?;
            if quantity.is_sign_negative() {
                return Err(KernelContractError::InvalidField(
                    "authority.max_quantity",
                    "maximum quantity cannot be negative".into(),
                ));
            }
            rule = rule.with_max_quantity(quantity);
        }
        Ok(rule)
    }
}

/// Operator-owned authority document compiled into deny-by-default policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicAuthority {
    pub version: String,
    #[serde(default)]
    pub commands: BTreeMap<String, EconomicAuthorityRule>,
    #[serde(default)]
    pub requires_mandate: bool,
    #[serde(default)]
    pub requires_signed_authority: bool,
    /// Operator-trusted issuer keys used to verify signed commands.
    #[serde(default)]
    pub trusted_authority_keys: BTreeMap<String, String>,
}

impl EconomicAuthority {
    #[must_use]
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            commands: BTreeMap::new(),
            requires_mandate: false,
            requires_signed_authority: false,
            trusted_authority_keys: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn allow(mut self, command: impl Into<String>, rule: EconomicAuthorityRule) -> Self {
        self.commands.insert(command.into(), rule);
        self
    }

    #[must_use]
    pub const fn with_mandates(mut self) -> Self {
        self.requires_mandate = true;
        self
    }

    #[must_use]
    pub const fn with_signed_commands(mut self) -> Self {
        self.requires_signed_authority = true;
        self
    }

    #[must_use]
    pub fn with_trusted_authority_key(
        mut self,
        key_id: impl Into<String>,
        public_key: [u8; 32],
    ) -> Self {
        self.trusted_authority_keys.insert(key_id.into(), hex::encode(public_key));
        self
    }

    /// Compile the ergonomic authority document into the kernel's canonical
    /// command policy. Commands absent from the result remain denied.
    pub fn compile(&self, agent: &EconomicAgent) -> Result<KernelPolicy, KernelContractError> {
        agent.validate(Utc::now())?;
        if self.version.trim().is_empty() {
            return Err(KernelContractError::MissingField("authority.version"));
        }
        let mut policy = KernelPolicy::new(&self.version);
        for (key_id, encoded) in &self.trusted_authority_keys {
            let decoded = hex::decode(encoded).map_err(|error| {
                KernelContractError::InvalidField(
                    "authority.trusted_authority_keys",
                    error.to_string(),
                )
            })?;
            let key = <[u8; 32]>::try_from(decoded.as_slice()).map_err(|_| {
                KernelContractError::InvalidField(
                    "authority.trusted_authority_keys",
                    "Ed25519 public keys must contain exactly 32 bytes".into(),
                )
            })?;
            policy = policy.with_trusted_authority_key(key_id, key);
        }
        for (command, authority) in &self.commands {
            if command.trim().is_empty() {
                return Err(KernelContractError::MissingField("authority.commands"));
            }
            let mut rule = authority.compile(agent)?;
            if self.requires_mandate {
                rule = rule.with_mandate();
            }
            if self.requires_signed_authority {
                rule = rule.with_signed_authority();
            }
            policy = policy.allow(command, rule);
        }
        Ok(policy)
    }
}

/// The intentionally small public vocabulary for economic action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EconomicVerb {
    Quote,
    Buy,
    Sell,
    Pay,
    Fulfill,
    Return,
    Refund,
    Subscribe,
}

impl EconomicVerb {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Pay => "pay",
            Self::Fulfill => "fulfill",
            Self::Return => "return",
            Self::Refund => "refund",
            Self::Subscribe => "subscribe",
        }
    }
}

/// Framework-neutral intent produced by one of the eight canonical verbs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicIntent {
    pub intent_id: Uuid,
    pub idempotency_key: String,
    pub verb: EconomicVerb,
    pub agent_id: String,
    pub principal_id: String,
    pub tenant_id: String,
    pub store_id: String,
    pub commitment: Option<EconomicCommitment>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

/// Small intent facade; protocol and domain adapters execute these intents.
#[derive(Debug, Clone)]
pub struct CanonicalTransactionApi {
    agent: EconomicAgent,
}

impl CanonicalTransactionApi {
    #[must_use]
    pub const fn new(agent: EconomicAgent) -> Self {
        Self { agent }
    }

    fn intent(
        &self,
        verb: EconomicVerb,
        idempotency_key: impl Into<String>,
        commitment: Option<EconomicCommitment>,
        payload: Value,
    ) -> EconomicIntent {
        EconomicIntent {
            intent_id: Uuid::new_v4(),
            idempotency_key: idempotency_key.into(),
            verb,
            agent_id: self.agent.id.clone(),
            principal_id: self.agent.principal_id.clone(),
            tenant_id: self.agent.tenant_id.clone(),
            store_id: self.agent.store_id.clone(),
            commitment,
            payload,
            created_at: Utc::now(),
        }
    }

    pub fn quote(&self, key: impl Into<String>, payload: Value) -> EconomicIntent {
        self.intent(EconomicVerb::Quote, key, None, payload)
    }

    pub fn buy(
        &self,
        key: impl Into<String>,
        commitment: EconomicCommitment,
        payload: Value,
    ) -> EconomicIntent {
        self.intent(EconomicVerb::Buy, key, Some(commitment), payload)
    }

    pub fn sell(
        &self,
        key: impl Into<String>,
        commitment: EconomicCommitment,
        payload: Value,
    ) -> EconomicIntent {
        self.intent(EconomicVerb::Sell, key, Some(commitment), payload)
    }

    pub fn pay(
        &self,
        key: impl Into<String>,
        commitment: EconomicCommitment,
        payload: Value,
    ) -> EconomicIntent {
        self.intent(EconomicVerb::Pay, key, Some(commitment), payload)
    }

    pub fn fulfill(&self, key: impl Into<String>, payload: Value) -> EconomicIntent {
        self.intent(EconomicVerb::Fulfill, key, None, payload)
    }

    pub fn return_order(&self, key: impl Into<String>, payload: Value) -> EconomicIntent {
        self.intent(EconomicVerb::Return, key, None, payload)
    }

    pub fn refund(
        &self,
        key: impl Into<String>,
        commitment: EconomicCommitment,
        payload: Value,
    ) -> EconomicIntent {
        self.intent(EconomicVerb::Refund, key, Some(commitment), payload)
    }

    pub fn subscribe(
        &self,
        key: impl Into<String>,
        commitment: EconomicCommitment,
        payload: Value,
    ) -> EconomicIntent {
        self.intent(EconomicVerb::Subscribe, key, Some(commitment), payload)
    }
}

/// Settlement evidence attached to a portable economic receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicSettlement {
    pub rail: String,
    pub amount: AuthorityAmount,
    pub transaction_id: String,
    pub status: String,
}

/// One independently verifiable signature over an economic receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicReceiptSignature {
    pub signer_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub signature: String,
}

/// Canonical, result-bound proof of an economic action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicReceipt {
    pub contract_version: String,
    pub receipt_id: Uuid,
    pub command_id: Uuid,
    pub command_type: String,
    pub status: ExecutionStatus,
    pub agent_id: String,
    pub principal_id: Option<String>,
    pub intent: String,
    pub commitment: Option<EconomicCommitment>,
    pub decision: Option<PolicyDecisionEvidence>,
    pub result_hash: Option<String>,
    pub settlement: Option<EconomicSettlement>,
    pub audit_hash: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub signatures: Vec<EconomicReceiptSignature>,
}

impl EconomicReceipt {
    /// Project a domain receipt into a compact, portable proof. The domain
    /// result is represented by its canonical SHA-256 digest.
    pub fn from_execution<T: Serialize>(
        receipt: &ExecutionReceipt<T>,
    ) -> Result<Self, KernelContractError> {
        let context = receipt
            .economic_context
            .as_ref()
            .ok_or(KernelContractError::MissingField("receipt.economic_context"))?;
        let result_hash = receipt.result.as_ref().map(canonical_hash).transpose()?;
        Ok(Self {
            contract_version: receipt.contract_version.clone(),
            receipt_id: receipt.receipt_id,
            command_id: receipt.command_id,
            command_type: receipt.command_type.clone(),
            status: receipt.status,
            agent_id: context.principal.id.clone(),
            principal_id: context.principal.delegated_by.clone(),
            intent: context
                .mandate
                .as_ref()
                .map_or_else(|| receipt.command_type.clone(), |value| value.objective.clone()),
            commitment: context.commitment.clone(),
            decision: receipt.policy.clone(),
            result_hash,
            settlement: None,
            audit_hash: receipt.audit_hash.clone(),
            timestamp: receipt.completed_at,
            signatures: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_settlement(mut self, settlement: EconomicSettlement) -> Self {
        self.settlement = Some(settlement);
        self
    }

    /// Canonical digest signed by every receipt party. Signature records are
    /// excluded so agent, merchant, and settler can co-sign the same bytes.
    pub fn signing_hash(&self) -> Result<[u8; 32], KernelContractError> {
        let mut unsigned = self.clone();
        unsigned.signatures.clear();
        let bytes = serde_jcs::to_vec(&unsigned)
            .map_err(|error| KernelContractError::Serialization(error.to_string()))?;
        Ok(Sha256::digest(bytes).into())
    }

    /// Add or replace one Ed25519 co-signature.
    pub fn sign(
        &mut self,
        signer_id: impl Into<String>,
        key_id: impl Into<String>,
        key: &SigningKey,
    ) -> Result<(), KernelContractError> {
        self.validate()?;
        let signer_id = signer_id.into();
        let key_id = key_id.into();
        let signature = key.sign(&self.signing_hash()?);
        self.signatures
            .retain(|existing| existing.signer_id != signer_id || existing.key_id != key_id);
        self.signatures.push(EconomicReceiptSignature {
            signer_id,
            key_id,
            algorithm: "ed25519".into(),
            signature: hex::encode(signature.to_bytes()),
        });
        Ok(())
    }

    /// Verify all signatures against a trusted key registry indexed by key ID.
    #[must_use]
    pub fn verify_signatures(&self, trusted_keys: &BTreeMap<String, [u8; 32]>) -> bool {
        if self.signatures.is_empty() || self.validate().is_err() {
            return false;
        }
        let Ok(hash) = self.signing_hash() else {
            return false;
        };
        self.signatures.iter().all(|record| {
            if record.algorithm != "ed25519" {
                return false;
            }
            let Some(key) = trusted_keys.get(&record.key_id) else {
                return false;
            };
            let Ok(signature_bytes) = hex::decode(&record.signature) else {
                return false;
            };
            let Ok(signature_bytes) = <[u8; 64]>::try_from(signature_bytes.as_slice()) else {
                return false;
            };
            VerifyingKey::from_bytes(key).is_ok_and(|verifying_key| {
                verifying_key.verify(&hash, &Signature::from_bytes(&signature_bytes)).is_ok()
            })
        })
    }

    fn validate(&self) -> Result<(), KernelContractError> {
        for (field, value) in [
            ("receipt.command_type", self.command_type.as_str()),
            ("receipt.agent_id", self.agent_id.as_str()),
            ("receipt.intent", self.intent.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(KernelContractError::MissingField(field));
            }
        }
        if let Some(settlement) = &self.settlement {
            if settlement.rail.trim().is_empty()
                || settlement.transaction_id.trim().is_empty()
                || settlement.status.trim().is_empty()
            {
                return Err(KernelContractError::InvalidField(
                    "receipt.settlement",
                    "rail, transaction_id, and status are required".into(),
                ));
            }
            match &settlement.amount {
                AuthorityAmount::Money(wire) => {
                    let money = Money::try_from(wire.clone()).map_err(|error| {
                        KernelContractError::InvalidField(
                            "receipt.settlement.amount",
                            error.to_string(),
                        )
                    })?;
                    if money.is_negative() {
                        return Err(KernelContractError::InvalidField(
                            "receipt.settlement.amount",
                            "settlement amounts cannot be negative".into(),
                        ));
                    }
                }
                AuthorityAmount::Asset(wire) => {
                    wire.validate()?;
                }
            }
        }
        Ok(())
    }
}

fn canonical_hash<T: Serialize>(value: &T) -> Result<String, KernelContractError> {
    let bytes = serde_jcs::to_vec(value)
        .map_err(|error| KernelContractError::Serialization(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurrencyCode, ExecutionReceipt};

    fn agent() -> EconomicAgent {
        EconomicAgent::new(
            "agent:acme:procurement:7",
            "company:acme",
            "procurement",
            "tenant:acme",
            "store:production",
        )
        .with_capabilities(["payments.create", "a2a.escrow.create"])
        .with_budgets(["budget:procurement:monthly"])
    }

    #[test]
    fn agent_identity_produces_scoped_delegated_commands() {
        let agent = agent();
        agent.validate(Utc::now()).expect("valid identity");
        let command = agent.command("payments.create", "pay-1", Value::Null);
        assert_eq!(command.principal.id, agent.id);
        assert_eq!(command.principal.delegated_by.as_deref(), Some("company:acme"));
        assert_eq!(command.store_id.as_deref(), Some("store:production"));
    }

    #[test]
    fn authority_compiles_autonomous_approval_and_deny_tiers() {
        let agent = agent();
        let authority = EconomicAuthority::new("procurement-v4").allow(
            "payments.create",
            EconomicAuthorityRule::money(
                "payments.create",
                Money::new(Decimal::new(250000, 2), CurrencyCode::USD),
                Money::new(Decimal::new(2500000, 2), CurrencyCode::USD),
            )
            .with_budget(),
        );
        let policy = authority.compile(&agent).expect("compile authority");
        let rule = policy.commands.get("payments.create").expect("command rule");
        assert_eq!(rule.approval_above.as_ref().expect("threshold").amount, "2500.00");
        assert_eq!(rule.max_amount.as_ref().expect("maximum").amount, "25000.00");
        assert!(rule.requires_budget);
    }

    #[test]
    fn canonical_api_exposes_only_the_small_verb_vocabulary() {
        let api = CanonicalTransactionApi::new(agent());
        let intent = api.buy(
            "buy-1",
            EconomicCommitment::for_asset(Decimal::new(465000, 2), "USDC"),
            serde_json::json!({"sku": "SKU-100", "quantity": "50"}),
        );
        assert_eq!(intent.verb, EconomicVerb::Buy);
        assert_eq!(intent.principal_id, "company:acme");
    }

    #[test]
    fn economic_receipt_is_result_bound_and_co_signable() {
        let agent = agent();
        let mut command = agent.command("a2a.escrow.create", "buy-1", Value::Null);
        command.commitment = Some(EconomicCommitment::for_asset(Decimal::new(465000, 2), "USDC"));
        let execution = ExecutionReceipt::succeeded(&command, serde_json::json!({"id": "e-1"}));
        let mut receipt = EconomicReceipt::from_execution(&execution)
            .expect("economic receipt")
            .with_settlement(EconomicSettlement {
                rail: "x402".into(),
                amount: AuthorityAmount::Asset(AssetAmountWire::new(
                    Decimal::new(465000, 2),
                    "USDC",
                )),
                transaction_id: "0x82".into(),
                status: "settled".into(),
            });
        let agent_key = SigningKey::from_bytes(&[7_u8; 32]);
        let merchant_key = SigningKey::from_bytes(&[9_u8; 32]);
        receipt.sign("agent:buyer", "buyer-key", &agent_key).expect("buyer signature");
        receipt.sign("agent:merchant", "merchant-key", &merchant_key).expect("merchant signature");
        let keys = BTreeMap::from([
            ("buyer-key".into(), agent_key.verifying_key().to_bytes()),
            ("merchant-key".into(), merchant_key.verifying_key().to_bytes()),
        ]);
        assert!(receipt.verify_signatures(&keys));

        receipt.settlement.as_mut().expect("settlement").transaction_id = "0xtampered".into();
        assert!(!receipt.verify_signatures(&keys));
    }

    #[test]
    fn operator_examples_deserialize_and_compile_together() {
        let agent: EconomicAgent =
            serde_json::from_str(include_str!("../../../kernel/examples/economic-agent.json"))
                .expect("economic agent example");
        let authority: EconomicAuthority =
            serde_json::from_str(include_str!("../../../kernel/examples/economic-authority.json"))
                .expect("economic authority example");
        let policy = authority.compile(&agent).expect("compile example authority");
        assert_eq!(policy.commands.len(), 3);
        assert!(policy.commands.values().all(|rule| rule.requires_mandate));
        assert!(policy.commands.values().all(|rule| rule.requires_signed_authority));
    }
}
