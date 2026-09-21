//! x402 Payment Protocol API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]
// `#[napi]` free functions and their object types are reached only through
// napi's registration, which rustc cannot see from inside a private module.
#![allow(dead_code)]

use super::*;

// ============================================================================
// x402 Payment Protocol API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreateIntentInput {
    pub payer_address: String,
    pub payee_address: String,
    pub amount: i64,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: Option<String>,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: Option<String>,
    #[napi(ts_type = "X402SignatureScheme")]
    pub signature_scheme: Option<String>,
    pub nonce: Option<i64>,
    pub validity_seconds: Option<i64>,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
    pub description: Option<String>,
    pub cart_id: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub merchant_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SigningHashInput {
    pub payer_address: String,
    pub payee_address: String,
    pub amount: i64,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: String,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: String,
    pub chain_id: i64,
    pub valid_until: i64,
    pub nonce: i64,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SignatureBundleInput {
    pub ml_dsa_65_signature: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402PublicKeyBundleInput {
    pub ml_dsa_65_public_key: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SignatureBundleOutput {
    pub ml_dsa_65_signature: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402PublicKeyBundleOutput {
    pub ml_dsa_65_public_key: Buffer,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402SignIntentInput {
    pub intent_id: Option<String>,
    #[napi(ts_type = "X402SignatureScheme")]
    pub signature_scheme: Option<String>,
    pub signature: String,
    pub public_key: String,
    pub signature_bundle: Option<X402SignatureBundleInput>,
    pub public_key_bundle: Option<X402PublicKeyBundleInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct X402IntentFilterInput {
    pub payer_address: Option<String>,
    pub payee_address: Option<String>,
    #[napi(ts_type = "X402IntentStatusInput")]
    pub status: Option<String>,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: Option<String>,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: Option<String>,
    pub order_id: Option<String>,
    pub batch_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Clone)]
pub struct X402IntentOutput {
    pub id: String,
    pub version: String,
    #[napi(ts_type = "X402IntentStatus")]
    pub status: String,
    pub payer_address: String,
    pub payee_address: String,
    pub amount: i64,
    /// @deprecated Use the `amountDecimalExact` twin; float money will be removed in 2.0.
    pub amount_decimal: f64,
    /// Exact base-10 amount decimal, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_decimal_exact: String,
    #[napi(ts_type = "X402Asset")]
    pub asset: String,
    #[napi(ts_type = "X402Network")]
    pub network: String,
    pub chain_id: i64,
    pub token_address: Option<String>,
    pub created_at_unix: i64,
    pub valid_until: i64,
    pub nonce: i64,
    pub idempotency_key: Option<String>,
    pub resource_uri: Option<String>,
    pub resource_method: Option<String>,
    pub description: Option<String>,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub merchant_id: Option<String>,
    pub signing_hash: Option<String>,
    #[napi(ts_type = "X402SignatureScheme")]
    pub payer_signature_scheme: Option<String>,
    pub payer_signature: Option<String>,
    pub payer_public_key: Option<String>,
    pub payer_signature_bundle: Option<X402SignatureBundleOutput>,
    pub payer_public_key_bundle: Option<X402PublicKeyBundleOutput>,
    pub sequence_number: Option<i64>,
    pub sequenced_at: Option<String>,
    pub batch_id: Option<String>,
    pub batch_merkle_root: Option<String>,
    pub inclusion_proof: Option<Vec<String>>,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub gas_used: Option<i64>,
    pub settled_at: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::X402PaymentIntent> for X402IntentOutput {
    type Error = Error;

    fn try_from(intent: stateset_core::X402PaymentIntent) -> Result<Self> {
        let (amount_decimal, amount_decimal_exact) =
            money_pair(intent.amount_decimal, "x402 intent amount")?;
        Ok(Self {
            id: intent.id.to_string(),
            version: intent.version,
            status: intent.status.to_string(),
            payer_address: intent.payer_address,
            payee_address: intent.payee_address,
            amount: intent.amount as i64,
            amount_decimal,
            amount_decimal_exact,
            asset: intent.asset.to_string().to_lowercase(),
            network: intent.network.to_string(),
            chain_id: intent.chain_id as i64,
            token_address: intent.token_address,
            created_at_unix: intent.created_at_unix as i64,
            valid_until: intent.valid_until as i64,
            nonce: intent.nonce as i64,
            idempotency_key: intent.idempotency_key,
            resource_uri: intent.resource_uri,
            resource_method: intent.resource_method,
            description: intent.description,
            order_id: intent.order_id.map(|id| id.to_string()),
            invoice_id: intent.invoice_id.map(|id| id.to_string()),
            merchant_id: intent.merchant_id,
            signing_hash: intent.signing_hash,
            payer_signature_scheme: intent.payer_signature_scheme.map(|scheme| scheme.to_string()),
            payer_signature: intent.payer_signature,
            payer_public_key: intent.payer_public_key,
            payer_signature_bundle: intent.payer_signature_bundle.map(|bundle| {
                X402SignatureBundleOutput {
                    ml_dsa_65_signature: Buffer::from(bundle.ml_dsa_65_signature.as_slice()),
                }
            }),
            payer_public_key_bundle: intent.payer_public_key_bundle.map(|bundle| {
                X402PublicKeyBundleOutput {
                    ml_dsa_65_public_key: Buffer::from(bundle.ml_dsa_65_public_key.as_slice()),
                }
            }),
            sequence_number: intent.sequence_number.map(|n| n as i64),
            sequenced_at: intent.sequenced_at.map(|d| d.to_rfc3339()),
            batch_id: intent.batch_id.map(|id| id.to_string()),
            batch_merkle_root: intent.batch_merkle_root,
            inclusion_proof: intent.inclusion_proof,
            tx_hash: intent.tx_hash,
            block_number: intent.block_number.map(|n| n as i64),
            gas_used: intent.gas_used.map(|n| n as i64),
            settled_at: intent.settled_at.map(|d| d.to_rfc3339()),
            metadata: intent.metadata,
            created_at: intent.created_at.to_rfc3339(),
            updated_at: intent.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402AgentCardInput {
    pub name: String,
    pub description: Option<String>,
    pub wallet_address: String,
    pub public_key: String,
    #[napi(ts_type = "X402NetworkInput[]")]
    pub supported_networks: Option<Vec<String>>,
    #[napi(ts_type = "X402AssetInput[]")]
    pub supported_assets: Option<Vec<String>>,
    #[napi(ts_type = "X402A2ASkillInput[]")]
    pub a2a_skills: Option<Vec<String>>,
    #[napi(ts_type = "X402TrustLevelInput")]
    pub trust_level: Option<String>,
    pub endpoint_url: Option<String>,
    pub endpoint_protocol: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
    pub business_category: Option<String>,
    pub max_transaction_amount: Option<i64>,
    pub daily_volume_limit: Option<i64>,
    pub requires_kyc: Option<bool>,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct X402AgentCardFilterInput {
    pub wallet_address: Option<String>,
    #[napi(ts_type = "X402TrustLevelInput")]
    pub trust_level: Option<String>,
    #[napi(ts_type = "X402TrustLevelInput")]
    pub min_trust_level: Option<String>,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: Option<String>,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: Option<String>,
    #[napi(ts_type = "X402A2ASkillInput")]
    pub skill: Option<String>,
    pub active: Option<bool>,
    pub merchant_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402AgentCardOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub wallet_address: String,
    pub public_key: String,
    #[napi(ts_type = "X402Network[]")]
    pub supported_networks: Vec<String>,
    #[napi(ts_type = "X402Asset[]")]
    pub supported_assets: Vec<String>,
    #[napi(ts_type = "X402A2ASkill[]")]
    pub a2a_skills: Vec<String>,
    #[napi(ts_type = "X402TrustLevel")]
    pub trust_level: String,
    pub verified_at: Option<String>,
    pub verification_method: Option<String>,
    pub endpoint_url: Option<String>,
    pub endpoint_protocol: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
    pub business_category: Option<String>,
    pub max_transaction_amount: Option<i64>,
    pub daily_volume_limit: Option<i64>,
    pub requires_kyc: bool,
    pub active: bool,
    pub suspended_at: Option<String>,
    pub suspension_reason: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::AgentCard> for X402AgentCardOutput {
    fn from(card: stateset_core::AgentCard) -> Self {
        Self {
            id: card.id.to_string(),
            name: card.name,
            description: card.description,
            wallet_address: card.wallet_address,
            public_key: card.public_key,
            supported_networks: card
                .supported_networks
                .into_iter()
                .map(|n| n.to_string())
                .collect(),
            supported_assets: card
                .supported_assets
                .into_iter()
                .map(|a| a.to_string().to_lowercase())
                .collect(),
            a2a_skills: card.a2a_skills.into_iter().map(|s| s.to_string()).collect(),
            trust_level: card.trust_level.to_string(),
            verified_at: card.verified_at.map(|d| d.to_rfc3339()),
            verification_method: card.verification_method,
            endpoint_url: card.endpoint_url,
            endpoint_protocol: card.endpoint_protocol,
            merchant_id: card.merchant_id,
            merchant_name: card.merchant_name,
            business_category: card.business_category,
            max_transaction_amount: card.max_transaction_amount.map(|v| v as i64),
            daily_volume_limit: card.daily_volume_limit.map(|v| v as i64),
            requires_kyc: card.requires_kyc,
            active: card.active,
            suspended_at: card.suspended_at.map(|d| d.to_rfc3339()),
            suspension_reason: card.suspension_reason,
            metadata: card.metadata,
            created_at: card.created_at.to_rfc3339(),
            updated_at: card.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditBalanceInput {
    pub payer_address: String,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: Option<String>,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditAdjustmentInput {
    pub payer_address: String,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: Option<String>,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: Option<String>,
    pub amount: i64,
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct X402CreditTransactionFilterInput {
    pub payer_address: Option<String>,
    #[napi(ts_type = "X402AssetInput")]
    pub asset: Option<String>,
    #[napi(ts_type = "X402NetworkInput")]
    pub network: Option<String>,
    #[napi(ts_type = "X402CreditDirectionInput")]
    pub direction: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditAccountOutput {
    pub id: String,
    pub payer_address: String,
    #[napi(ts_type = "X402Asset")]
    pub asset: String,
    #[napi(ts_type = "X402Network")]
    pub network: String,
    pub balance: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::X402CreditAccount> for X402CreditAccountOutput {
    fn from(account: stateset_core::X402CreditAccount) -> Self {
        Self {
            id: account.id.to_string(),
            payer_address: account.payer_address,
            asset: account.asset.to_string().to_lowercase(),
            network: account.network.to_string(),
            balance: account.balance as i64,
            created_at: account.created_at.to_rfc3339(),
            updated_at: account.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct X402CreditTransactionOutput {
    pub id: String,
    pub account_id: String,
    pub payer_address: String,
    #[napi(ts_type = "X402Asset")]
    pub asset: String,
    #[napi(ts_type = "X402Network")]
    pub network: String,
    #[napi(ts_type = "X402CreditDirection")]
    pub direction: String,
    pub amount: i64,
    pub balance_after: i64,
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::X402CreditTransaction> for X402CreditTransactionOutput {
    fn from(txn: stateset_core::X402CreditTransaction) -> Self {
        Self {
            id: txn.id.to_string(),
            account_id: txn.account_id.to_string(),
            payer_address: txn.payer_address,
            asset: txn.asset.to_string().to_lowercase(),
            network: txn.network.to_string(),
            direction: txn.direction.to_string(),
            amount: txn.amount as i64,
            balance_after: txn.balance_after as i64,
            reason: txn.reason,
            reference_id: txn.reference_id,
            metadata: txn.metadata,
            created_at: txn.created_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_x402_asset(s: &str) -> Result<stateset_core::X402Asset> {
    s.parse::<stateset_core::X402Asset>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid x402 asset", e))
}

pub(crate) fn parse_x402_network(s: &str) -> Result<stateset_core::X402Network> {
    s.parse::<stateset_core::X402Network>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid x402 network", e))
}

pub(crate) fn parse_x402_status(s: &str) -> Result<stateset_core::X402IntentStatus> {
    s.parse::<stateset_core::X402IntentStatus>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid x402 status", e))
}

pub(crate) fn parse_x402_signature_scheme(s: &str) -> Result<stateset_core::X402SignatureScheme> {
    s.parse::<stateset_core::X402SignatureScheme>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid x402 signature scheme", e))
}

pub(crate) fn parse_trust_level(s: &str) -> Result<stateset_core::TrustLevel> {
    s.parse::<stateset_core::TrustLevel>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid trust level", e))
}

pub(crate) fn parse_a2a_skill(s: &str) -> Result<stateset_core::A2ASkill> {
    s.parse::<stateset_core::A2ASkill>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid A2A skill", e))
}

pub(crate) fn parse_credit_direction(s: &str) -> Result<stateset_core::X402CreditDirection> {
    s.parse::<stateset_core::X402CreditDirection>()
        .map_err(|e| wrap(ErrCode::Validation, "Invalid credit direction", e))
}

pub(crate) fn parse_uuid_opt(value: Option<String>) -> Result<Option<uuid::Uuid>> {
    match value {
        Some(id) => Ok(Some(id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?)),
        None => Ok(None),
    }
}

pub(crate) fn parse_amount(value: i64) -> Result<u64> {
    if value < 0 {
        return Err(coded(ErrCode::Validation, "Amount must be >= 0"));
    }
    Ok(value as u64)
}

pub(crate) fn parse_u64_field(field: &str, value: i64) -> Result<u64> {
    if value < 0 {
        return Err(coded(ErrCode::Validation, format!("{} must be >= 0", field)));
    }
    Ok(value as u64)
}

pub(crate) fn parse_u64_opt(field: &str, value: Option<i64>) -> Result<Option<u64>> {
    match value {
        Some(val) => Ok(Some(parse_u64_field(field, val)?)),
        None => Ok(None),
    }
}

/// Compute the sequencer-compatible x402 signing hash for a payment intent shape.
#[napi]
pub fn ves_x402_compute_signing_hash(input: X402SigningHashInput) -> Result<Buffer> {
    guard(|| {
        use sha2::{Digest, Sha256};

        let amount = parse_amount(input.amount)?;
        let chain_id = parse_u64_field("chain_id", input.chain_id)?;
        let valid_until = parse_u64_field("valid_until", input.valid_until)?;
        let nonce = parse_u64_field("nonce", input.nonce)?;
        let asset = parse_x402_asset(&input.asset)?;
        let network = parse_x402_network(&input.network)?;

        let mut hasher = Sha256::new();
        hasher.update(stateset_core::X402_DOMAIN_SEPARATOR.as_bytes());
        hasher.update(input.payer_address.as_bytes());
        hasher.update(input.payee_address.as_bytes());
        hasher.update(amount.to_be_bytes());
        hasher.update(format!("{:?}", asset).to_lowercase().as_bytes());
        hasher.update(network.to_string().as_bytes());
        hasher.update(chain_id.to_be_bytes());
        hasher.update(valid_until.to_be_bytes());
        hasher.update(nonce.to_be_bytes());

        match input.resource_uri {
            Some(uri) => {
                hasher.update([1u8]);
                hasher.update((uri.len() as u64).to_be_bytes());
                hasher.update(uri.as_bytes());
            }
            None => hasher.update([0u8]),
        }

        match input.resource_method {
            Some(method) => {
                hasher.update([1u8]);
                hasher.update((method.len() as u64).to_be_bytes());
                hasher.update(method.as_bytes());
            }
            None => hasher.update([0u8]),
        }

        let result: [u8; 32] = hasher.finalize().into();
        Ok(Buffer::from(result.as_slice()))
    })
}

#[napi]
pub struct X402 {
    pub(crate) commerce: Handle,
}

#[napi]
impl X402 {
    #[napi]
    pub async fn create_intent(&self, input: X402CreateIntentInput) -> Result<X402IntentOutput> {
        let commerce = self.commerce.get()?;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let amount = parse_amount(input.amount)?;

        let intent = commerce
            .x402()
            .create_intent(stateset_core::CreateX402PaymentIntent {
                payer_address: input.payer_address,
                payee_address: input.payee_address,
                amount,
                asset,
                network,
                signature_scheme: input
                    .signature_scheme
                    .as_deref()
                    .map(parse_x402_signature_scheme)
                    .transpose()?,
                nonce: parse_u64_opt("nonce", input.nonce)?,
                validity_seconds: parse_u64_opt("validity_seconds", input.validity_seconds)?,
                resource_uri: input.resource_uri,
                resource_method: input.resource_method,
                description: input.description,
                cart_id: parse_uuid_opt(input.cart_id)?,
                order_id: parse_uuid_opt(input.order_id)?,
                invoice_id: parse_uuid_opt(input.invoice_id)?,
                merchant_id: input.merchant_id,
                idempotency_key: input.idempotency_key,
                metadata: input.metadata,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create x402 intent", e))?;

        convert_output(intent)
    }

    #[napi]
    pub async fn sign_intent(
        &self,
        intent_id: String,
        input: X402SignIntentInput,
    ) -> Result<X402IntentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = intent_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let signed = commerce
            .x402()
            .sign_intent(
                uuid,
                stateset_core::SignX402PaymentIntent {
                    intent_id: uuid,
                    signature_scheme: input
                        .signature_scheme
                        .as_deref()
                        .map(parse_x402_signature_scheme)
                        .transpose()?,
                    signature: input.signature,
                    public_key: input.public_key,
                    signature_bundle: input.signature_bundle.map(|bundle| {
                        stateset_core::X402SignatureBundle {
                            ml_dsa_65_signature: bundle.ml_dsa_65_signature.as_ref().to_vec(),
                        }
                    }),
                    public_key_bundle: input.public_key_bundle.map(|bundle| {
                        stateset_core::X402PublicKeyBundle {
                            ml_dsa_65_public_key: bundle.ml_dsa_65_public_key.as_ref().to_vec(),
                        }
                    }),
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to sign x402 intent", e))?;

        convert_output(signed)
    }

    #[napi]
    pub async fn get_intent(&self, id: String) -> Result<Option<X402IntentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let intent = commerce
            .x402()
            .get_intent(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get x402 intent", e))?;
        convert_optional_output(intent)
    }

    #[napi]
    pub async fn list_intents(
        &self,
        filter: X402IntentFilterInput,
    ) -> Result<Vec<X402IntentOutput>> {
        let commerce = self.commerce.get()?;
        let intents = commerce
            .x402()
            .list_intents(stateset_core::X402PaymentIntentFilter {
                payer_address: filter.payer_address,
                payee_address: filter.payee_address,
                status: match filter.status {
                    Some(val) => Some(parse_x402_status(&val)?),
                    None => None,
                },
                network: match filter.network {
                    Some(val) => Some(parse_x402_network(&val)?),
                    None => None,
                },
                asset: match filter.asset {
                    Some(val) => Some(parse_x402_asset(&val)?),
                    None => None,
                },
                order_id: parse_uuid_opt(filter.order_id)?,
                batch_id: parse_uuid_opt(filter.batch_id)?,
                limit: filter.limit,
                offset: filter.offset,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list x402 intents", e))?;

        convert_outputs(intents)
    }

    #[napi]
    pub async fn mark_settled(
        &self,
        intent_id: String,
        tx_hash: String,
        block_number: i64,
    ) -> Result<X402IntentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = intent_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let block_number = parse_u64_field("block_number", block_number)?;
        let intent = commerce
            .x402()
            .mark_settled(uuid, &tx_hash, block_number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark settled", e))?;
        convert_output(intent)
    }

    #[napi]
    pub async fn get_next_nonce(&self, payer_address: String) -> Result<i64> {
        let commerce = self.commerce.get()?;
        let nonce = commerce
            .x402()
            .get_next_nonce(&payer_address)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get nonce", e))?;
        i64::try_from(nonce)
            .map_err(|_| coded(ErrCode::Validation, "Nonce too large to fit in i64"))
    }

    #[napi]
    pub async fn register_agent(&self, input: X402AgentCardInput) -> Result<X402AgentCardOutput> {
        let commerce = self.commerce.get()?;
        let supported_networks = match input.supported_networks {
            Some(list) => {
                let mut parsed = Vec::with_capacity(list.len());
                for item in list {
                    parsed.push(parse_x402_network(&item)?);
                }
                Some(parsed)
            }
            None => None,
        };
        let supported_assets = match input.supported_assets {
            Some(list) => {
                let mut parsed = Vec::with_capacity(list.len());
                for item in list {
                    parsed.push(parse_x402_asset(&item)?);
                }
                Some(parsed)
            }
            None => None,
        };
        let a2a_skills = match input.a2a_skills {
            Some(list) => {
                let mut parsed = Vec::with_capacity(list.len());
                for item in list {
                    parsed.push(parse_a2a_skill(&item)?);
                }
                Some(parsed)
            }
            None => None,
        };
        let trust_level = match input.trust_level {
            Some(val) => Some(parse_trust_level(&val)?),
            None => None,
        };
        let max_transaction_amount = match input.max_transaction_amount {
            Some(val) => Some(parse_amount(val)?),
            None => None,
        };
        let daily_volume_limit = match input.daily_volume_limit {
            Some(val) => Some(parse_amount(val)?),
            None => None,
        };

        let card = commerce
            .x402()
            .register_agent(stateset_core::CreateAgentCard {
                name: input.name,
                description: input.description,
                wallet_address: input.wallet_address,
                public_key: input.public_key,
                supported_networks,
                supported_assets,
                a2a_skills,
                trust_level,
                endpoint_url: input.endpoint_url,
                endpoint_protocol: input.endpoint_protocol,
                merchant_id: input.merchant_id,
                merchant_name: input.merchant_name,
                business_category: input.business_category,
                max_transaction_amount,
                daily_volume_limit,
                requires_kyc: input.requires_kyc,
                metadata: input.metadata,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to register agent", e))?;

        Ok(card.into())
    }

    #[napi(
        ts_args_type = "network?: X402NetworkInput, asset?: X402AssetInput, skill?: X402A2ASkillInput, trustLevel?: X402TrustLevelInput"
    )]
    pub async fn discover_agents(
        &self,
        network: Option<String>,
        asset: Option<String>,
        skill: Option<String>,
        trust_level: Option<String>,
    ) -> Result<Vec<X402AgentCardOutput>> {
        let commerce = self.commerce.get()?;
        let network = match network {
            Some(val) => Some(parse_x402_network(&val)?),
            None => None,
        };
        let asset = match asset {
            Some(val) => Some(parse_x402_asset(&val)?),
            None => None,
        };
        let skill = match skill {
            Some(val) => Some(parse_a2a_skill(&val)?),
            None => None,
        };
        let trust_level = match trust_level {
            Some(val) => Some(parse_trust_level(&val)?),
            None => None,
        };

        let agents = commerce
            .x402()
            .discover_agents(network, asset, skill, trust_level)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to discover agents", e))?;

        Ok(agents.into_iter().map(|a| a.into()).collect())
    }

    #[napi]
    pub async fn get_agent(&self, id: String) -> Result<Option<X402AgentCardOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let agent = commerce
            .x402()
            .get_agent(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get agent", e))?;
        Ok(agent.map(|a| a.into()))
    }

    #[napi]
    pub async fn get_agent_by_wallet(
        &self,
        wallet_address: String,
    ) -> Result<Option<X402AgentCardOutput>> {
        let commerce = self.commerce.get()?;
        let agent = commerce
            .x402()
            .get_agent_by_wallet(&wallet_address)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get agent", e))?;
        Ok(agent.map(|a| a.into()))
    }

    #[napi]
    pub async fn verify_agent(&self, id: String) -> Result<X402AgentCardOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let agent = commerce
            .x402()
            .verify_agent(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to verify agent", e))?;
        Ok(agent.into())
    }

    #[napi]
    pub async fn list_agents(
        &self,
        filter: X402AgentCardFilterInput,
    ) -> Result<Vec<X402AgentCardOutput>> {
        let commerce = self.commerce.get()?;
        let agents = commerce
            .x402()
            .list_agents(stateset_core::AgentCardFilter {
                wallet_address: filter.wallet_address,
                trust_level: match filter.trust_level {
                    Some(val) => Some(parse_trust_level(&val)?),
                    None => None,
                },
                min_trust_level: match filter.min_trust_level {
                    Some(val) => Some(parse_trust_level(&val)?),
                    None => None,
                },
                network: match filter.network {
                    Some(val) => Some(parse_x402_network(&val)?),
                    None => None,
                },
                asset: match filter.asset {
                    Some(val) => Some(parse_x402_asset(&val)?),
                    None => None,
                },
                skill: match filter.skill {
                    Some(val) => Some(parse_a2a_skill(&val)?),
                    None => None,
                },
                active: filter.active,
                merchant_id: filter.merchant_id,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list agents", e))?;

        Ok(agents.into_iter().map(|a| a.into()).collect())
    }

    #[napi]
    pub async fn get_credit_balance(&self, input: X402CreditBalanceInput) -> Result<i64> {
        let commerce = self.commerce.get()?;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let balance = commerce
            .x402()
            .get_credit_balance(&input.payer_address, asset, network)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get credit balance", e))?;
        Ok(balance as i64)
    }

    #[napi]
    pub async fn get_credit_account(
        &self,
        input: X402CreditBalanceInput,
    ) -> Result<Option<X402CreditAccountOutput>> {
        let commerce = self.commerce.get()?;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let account = commerce
            .x402()
            .get_credit_account(&input.payer_address, asset, network)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get credit account", e))?;
        Ok(account.map(|a| a.into()))
    }

    #[napi]
    pub async fn credit_account(
        &self,
        input: X402CreditAdjustmentInput,
    ) -> Result<X402CreditTransactionOutput> {
        let commerce = self.commerce.get()?;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let amount = parse_amount(input.amount)?;
        let txn = commerce
            .x402()
            .credit_account(
                &input.payer_address,
                asset,
                network,
                amount,
                input.reason,
                input.reference_id,
                input.metadata,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to credit account", e))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn debit_account(
        &self,
        input: X402CreditAdjustmentInput,
    ) -> Result<X402CreditTransactionOutput> {
        let commerce = self.commerce.get()?;
        let asset = match input.asset {
            Some(val) => parse_x402_asset(&val)?,
            None => stateset_core::X402Asset::Usdc,
        };
        let network = match input.network {
            Some(val) => parse_x402_network(&val)?,
            None => stateset_core::X402Network::SetChain,
        };
        let amount = parse_amount(input.amount)?;
        let txn = commerce
            .x402()
            .debit_account(
                &input.payer_address,
                asset,
                network,
                amount,
                input.reason,
                input.reference_id,
                input.metadata,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to debit account", e))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn list_credit_transactions(
        &self,
        filter: X402CreditTransactionFilterInput,
    ) -> Result<Vec<X402CreditTransactionOutput>> {
        let commerce = self.commerce.get()?;
        let transactions = commerce
            .x402()
            .list_credit_transactions(stateset_core::X402CreditTransactionFilter {
                payer_address: filter.payer_address,
                asset: match filter.asset {
                    Some(val) => Some(parse_x402_asset(&val)?),
                    None => None,
                },
                network: match filter.network {
                    Some(val) => Some(parse_x402_network(&val)?),
                    None => None,
                },
                direction: match filter.direction {
                    Some(val) => Some(parse_credit_direction(&val)?),
                    None => None,
                },
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list credit transactions", e))?;

        Ok(transactions.into_iter().map(|t| t.into()).collect())
    }
}
