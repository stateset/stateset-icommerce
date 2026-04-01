//! x402 Protocol Payment Types
//!
//! Implementation of the x402 HTTP-native payment protocol for AI agents.
//! Enables off-chain payment signing with on-chain settlement via Set Chain L2.
//!
//! ## x402 Protocol Overview
//!
//! The x402 protocol uses HTTP 402 (Payment Required) status codes to enable
//! instant stablecoin micropayments. When a server requires payment, it returns
//! a 402 response with payment details. The client signs a payment intent off-chain
//! and includes it in the retry request.
//!
//! ## Flow
//!
//! 1. Client requests resource
//! 2. Server returns HTTP 402 with `X402PaymentRequired` header
//! 3. Client creates `X402PaymentIntent`, signs it with Ed25519
//! 4. Client syncs intent to sequencer for batching
//! 5. Batched payments are settled on Set Chain L2
//! 6. Server verifies payment via inclusion proof
//!
//! ## Example
//!
//! ```rust
//! use stateset_core::models::x402::{X402PaymentIntent, X402Network, X402Asset};
//!
//! let intent = X402PaymentIntent::new(
//!     "0x1234abcd1234abcd1234abcd1234abcd1234abcd",
//!     "0x5678efab5678efab5678efab5678efab5678efab",
//!     1_000_000, // 1 USDC (6 decimals)
//!     X402Asset::Usdc,
//!     X402Network::SetChain,
//! );
//! assert_eq!(intent.amount, 1_000_000);
//! ```

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rs_merkle::{MerkleProof, algorithms::Sha256 as MerkleSha256};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stateset_crypto::pqc::{
    HybridSignatureBundle as PqcHybridSignatureBundle, HybridSigningKeypair,
    HybridSigningPublicKey, StrictSigningKeypair, StrictSigningPublicKey, hybrid_sign_event_hash,
    hybrid_verify_event_signature, strict_sign_event_hash, strict_verify_event_signature,
};
use strum::{Display, EnumString};
use thiserror::Error;
use uuid::Uuid;

// =============================================================================
// x402 Protocol Constants
// =============================================================================

/// x402 protocol version
pub const X402_VERSION: &str = "1.0";

/// Domain separator for x402 payment signing (per EIP-712 style)
pub const X402_DOMAIN_SEPARATOR: &str = "X402_PAYMENT_V1";

/// Maximum payment validity window (24 hours in seconds)
pub const X402_MAX_VALIDITY_SECONDS: u64 = 86400;

/// Default payment validity window (1 hour in seconds)
pub const X402_DEFAULT_VALIDITY_SECONDS: u64 = 3600;

// =============================================================================
// x402 Network & Asset Types
// =============================================================================

/// Supported blockchain networks for x402 payments
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum X402Network {
    /// Set Chain L2 (StateSet native) - primary network
    #[default]
    #[strum(serialize = "set_chain", serialize = "set", serialize = "ssc")]
    SetChain,
    /// Set Chain testnet
    #[strum(serialize = "set_chain_testnet", serialize = "set_testnet")]
    SetChainTestnet,
    /// Base L2 (Coinbase)
    Base,
    /// Arc L2 (Circle stablecoin-native)
    Arc,
    /// Arc testnet
    #[strum(serialize = "arc_testnet", serialize = "arc-testnet")]
    ArcTestnet,
    /// Base Sepolia testnet
    BaseSepolia,
    /// Ethereum mainnet
    #[strum(serialize = "ethereum", serialize = "eth", serialize = "mainnet")]
    Ethereum,
    /// Ethereum Sepolia testnet
    #[strum(serialize = "ethereum_sepolia", serialize = "sepolia")]
    EthereumSepolia,
    /// Arbitrum One
    #[strum(serialize = "arbitrum", serialize = "arb")]
    Arbitrum,
    /// Optimism
    #[strum(serialize = "optimism", serialize = "op")]
    Optimism,
}

impl X402Network {
    /// Get the chain ID for this network
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        match self {
            Self::SetChain => 84532001,        // Set Chain mainnet
            Self::SetChainTestnet => 84532002, // Set Chain testnet
            Self::Arc => 5042001,              // Arc mainnet
            Self::ArcTestnet => 5042002,       // Arc testnet
            Self::Base => 8453,
            Self::BaseSepolia => 84532,
            Self::Ethereum => 1,
            Self::EthereumSepolia => 11155111,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
        }
    }

    /// Check if this is a testnet
    #[must_use]
    pub const fn is_testnet(&self) -> bool {
        matches!(
            self,
            Self::SetChainTestnet | Self::ArcTestnet | Self::BaseSepolia | Self::EthereumSepolia
        )
    }
}

/// Supported payment assets for x402
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum X402Asset {
    /// USD Coin (USDC) - primary stablecoin
    #[default]
    #[strum(serialize = "USDC")]
    Usdc,
    /// Tether (USDT)
    #[strum(serialize = "USDT", serialize = "TETHER")]
    Usdt,
    /// StateSet USD (ssUSD) - yield-bearing stablecoin
    #[serde(rename = "ssusd", alias = "ss_usd")]
    #[strum(serialize = "ssUSD", serialize = "SSUSD", serialize = "SS_USD")]
    SsUsd,
    /// Wrapped StateSet USD (ERC-4626)
    #[serde(rename = "wssusd", alias = "wss_usd")]
    #[strum(serialize = "wssUSD", serialize = "WSSUSD", serialize = "WSS_USD")]
    WssUsd,
    /// DAI stablecoin
    #[strum(serialize = "DAI")]
    Dai,
    /// Native ETH (for gas)
    #[strum(serialize = "ETH", serialize = "ETHER")]
    Eth,
}

impl X402Asset {
    /// Get the number of decimals for this asset
    #[must_use]
    pub const fn decimals(&self) -> u8 {
        match self {
            Self::Usdc | Self::Usdt | Self::SsUsd | Self::WssUsd => 6,
            Self::Dai | Self::Eth => 18,
        }
    }

    /// Get the token contract address for a given network
    #[must_use]
    pub const fn contract_address(&self, network: X402Network) -> Option<&'static str> {
        match (self, network) {
            // Set Chain addresses
            (Self::Usdc, X402Network::SetChain) => {
                Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")
            }
            (Self::SsUsd, X402Network::SetChain) => {
                Some("0x0000000000000000000000000000000000001001")
            }
            // Arc addresses
            (Self::Usdc, X402Network::Arc | X402Network::ArcTestnet) => {
                Some("0x3600000000000000000000000000000000000000")
            }
            // Base addresses
            (Self::Usdc, X402Network::Base) => Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
            // Ethereum addresses
            (Self::Usdc, X402Network::Ethereum) => {
                Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
            }
            (Self::Usdt, X402Network::Ethereum) => {
                Some("0xdAC17F958D2ee523a2206206994597C13D831ec7")
            }
            (Self::Dai, X402Network::Ethereum) => Some("0x6B175474E89094C44Da98b954Ee4606eB48"),
            // Native ETH has no contract
            (Self::Eth, _) => None,
            _ => None,
        }
    }
}

// =============================================================================
// x402 Payment Intent (Off-Chain Signed Payment Request)
// =============================================================================

/// Status of an x402 payment intent
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum X402IntentStatus {
    /// Intent created, not yet signed
    #[default]
    Created,
    /// Intent signed by payer
    Signed,
    /// Intent submitted to sequencer
    Sequenced,
    /// Intent included in batch commitment
    Batched,
    /// Intent settled on-chain
    Settled,
    /// Intent expired (validity window passed)
    Expired,
    /// Intent failed to settle
    Failed,
    /// Intent cancelled by payer
    Cancelled,
}

impl std::str::FromStr for X402IntentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "created" => Ok(Self::Created),
            "signed" => Ok(Self::Signed),
            "sequenced" => Ok(Self::Sequenced),
            "batched" => Ok(Self::Batched),
            "settled" => Ok(Self::Settled),
            "expired" => Ok(Self::Expired),
            "failed" => Ok(Self::Failed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown x402 intent status: {s}")),
        }
    }
}

/// Direction of x402 credit ledger entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum X402CreditDirection {
    #[strum(serialize = "credit", serialize = "cr")]
    Credit,
    #[strum(serialize = "debit", serialize = "dr")]
    Debit,
}

/// Supported x402 signature schemes for off-chain payment intents.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum X402SignatureScheme {
    /// Legacy Ed25519 only.
    #[default]
    Ed25519,
    /// PQC-strict ML-DSA-65 only.
    MlDsa65,
    /// Hybrid Ed25519 + ML-DSA-65.
    Ed25519MlDsa65,
}

/// Additional PQC signature material for hybrid and strict x402 signatures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402SignatureBundle {
    /// ML-DSA-65 signature bytes.
    #[serde(with = "hex")]
    pub ml_dsa_65_signature: Vec<u8>,
}

/// Additional PQC public-key material for hybrid and strict x402 signatures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402PublicKeyBundle {
    /// ML-DSA-65 public key bytes.
    #[serde(with = "hex")]
    pub ml_dsa_65_public_key: Vec<u8>,
}

/// x402 Payment Intent - A signed off-chain payment request
///
/// This is the core data structure for x402 payments. It contains all the
/// information needed to authorize a payment, signed by the payer's key.
/// Intents are batched by the sequencer and settled on Set Chain L2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402PaymentIntent {
    /// Unique intent ID (UUID v4)
    pub id: Uuid,

    /// x402 protocol version
    pub version: String,

    /// Current status
    pub status: X402IntentStatus,

    // =========================================================================
    // Payment Parameters (signed fields)
    // =========================================================================
    /// Payer wallet address (sender)
    pub payer_address: String,

    /// Payee wallet address (recipient)
    pub payee_address: String,

    /// Payment amount in smallest unit (e.g., 1000000 = 1 USDC)
    pub amount: u64,

    /// Human-readable amount for display
    pub amount_decimal: Decimal,

    /// Payment asset (USDC, ssUSD, etc.)
    pub asset: X402Asset,

    /// Target blockchain network
    pub network: X402Network,

    /// Chain ID for EIP-712 domain
    pub chain_id: u64,

    /// Token contract address (None for native ETH)
    pub token_address: Option<String>,

    // =========================================================================
    // Validity & Replay Protection
    // =========================================================================
    /// Unix timestamp when intent was created
    pub created_at_unix: u64,

    /// Unix timestamp when intent expires (validity window)
    pub valid_until: u64,

    /// Unique nonce for replay protection (per payer)
    pub nonce: u64,

    /// Idempotency key for deduplication
    pub idempotency_key: Option<String>,

    // =========================================================================
    // Resource & Context
    // =========================================================================
    /// Resource URI this payment unlocks (e.g., API endpoint)
    pub resource_uri: Option<String>,

    /// HTTP method for resource (GET, POST, etc.)
    pub resource_method: Option<String>,

    /// Description of what the payment is for
    pub description: Option<String>,

    /// Associated cart ID (if applicable)
    pub cart_id: Option<Uuid>,

    /// Associated order ID (if applicable)
    pub order_id: Option<Uuid>,

    /// Associated invoice ID (if applicable)
    pub invoice_id: Option<Uuid>,

    /// Merchant/payee identifier
    pub merchant_id: Option<String>,

    // =========================================================================
    // Cryptographic Fields
    // =========================================================================
    /// Signing hash (SHA-256 of canonical payment data)
    /// Format: `SHA256(X402_DOMAIN_SEPARATOR` || `canonical_json`)
    pub signing_hash: Option<String>,

    /// Signature scheme used to authorize this intent.
    pub payer_signature_scheme: Option<X402SignatureScheme>,

    /// Payer's Ed25519 signature over `signing_hash` (hex-encoded)
    pub payer_signature: Option<String>,

    /// Payer's public key (hex-encoded, 32 bytes)
    pub payer_public_key: Option<String>,

    /// Additional PQC signature material for hybrid or strict schemes.
    pub payer_signature_bundle: Option<X402SignatureBundle>,

    /// Additional PQC public-key material for hybrid or strict schemes.
    pub payer_public_key_bundle: Option<X402PublicKeyBundle>,

    // =========================================================================
    // Sequencer Fields (set after submission)
    // =========================================================================
    /// Sequence number assigned by sequencer
    pub sequence_number: Option<u64>,

    /// Timestamp when sequenced
    pub sequenced_at: Option<DateTime<Utc>>,

    /// Batch ID containing this intent
    pub batch_id: Option<Uuid>,

    /// Merkle root of the batch
    pub batch_merkle_root: Option<String>,

    /// Merkle inclusion proof (for verification)
    pub inclusion_proof: Option<Vec<String>>,

    // =========================================================================
    // Settlement Fields (set after on-chain execution)
    // =========================================================================
    /// On-chain transaction hash
    pub tx_hash: Option<String>,

    /// Block number where settled
    pub block_number: Option<u64>,

    /// Gas used for settlement
    pub gas_used: Option<u64>,

    /// Timestamp when settled on-chain
    pub settled_at: Option<DateTime<Utc>>,

    // =========================================================================
    // Metadata
    // =========================================================================
    /// Additional metadata (JSON)
    pub metadata: Option<String>,

    /// When the intent record was created
    pub created_at: DateTime<Utc>,

    /// When the intent was last updated
    pub updated_at: DateTime<Utc>,
}

impl X402PaymentIntent {
    /// Create a new payment intent
    pub fn new(
        payer_address: impl Into<String>,
        payee_address: impl Into<String>,
        amount: u64,
        asset: X402Asset,
        network: X402Network,
    ) -> Self {
        let now = Utc::now();
        let now_unix = now.timestamp() as u64;

        // Calculate decimal amount
        let decimals = asset.decimals();
        let divisor = 10u64.pow(u32::from(decimals));
        let amount_decimal = Decimal::from(amount) / Decimal::from(divisor);

        Self {
            id: Uuid::new_v4(),
            version: X402_VERSION.to_string(),
            status: X402IntentStatus::Created,
            payer_address: payer_address.into(),
            payee_address: payee_address.into(),
            amount,
            amount_decimal,
            asset,
            network,
            chain_id: network.chain_id(),
            token_address: asset.contract_address(network).map(String::from),
            created_at_unix: now_unix,
            valid_until: now_unix + X402_DEFAULT_VALIDITY_SECONDS,
            nonce: 0,
            idempotency_key: None,
            resource_uri: None,
            resource_method: None,
            description: None,
            cart_id: None,
            order_id: None,
            invoice_id: None,
            merchant_id: None,
            signing_hash: None,
            payer_signature_scheme: None,
            payer_signature: None,
            payer_public_key: None,
            payer_signature_bundle: None,
            payer_public_key_bundle: None,
            sequence_number: None,
            sequenced_at: None,
            batch_id: None,
            batch_merkle_root: None,
            inclusion_proof: None,
            tx_hash: None,
            block_number: None,
            gas_used: None,
            settled_at: None,
            metadata: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Set the validity window in seconds
    #[must_use]
    pub fn with_validity(mut self, seconds: u64) -> Self {
        self.valid_until = self.created_at_unix + seconds.min(X402_MAX_VALIDITY_SECONDS);
        self
    }

    /// Set the nonce for replay protection
    #[must_use]
    pub const fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }

    /// Set the resource URI this payment unlocks
    pub fn with_resource(mut self, uri: impl Into<String>, method: impl Into<String>) -> Self {
        self.resource_uri = Some(uri.into());
        self.resource_method = Some(method.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the associated order ID
    #[must_use]
    pub const fn with_order(mut self, order_id: Uuid) -> Self {
        self.order_id = Some(order_id);
        self
    }

    /// Set the idempotency key
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Check if the intent has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp() as u64;
        now > self.valid_until
    }

    /// Check if the intent is signed
    #[must_use]
    pub fn is_signed(&self) -> bool {
        let has_signing_hash =
            self.signing_hash.as_ref().is_some_and(|signing_hash| !signing_hash.is_empty());
        if !has_signing_hash {
            return false;
        }

        match self.signature_scheme() {
            X402SignatureScheme::Ed25519 => {
                self.payer_signature.as_ref().is_some_and(|signature| !signature.is_empty())
                    && self
                        .payer_public_key
                        .as_ref()
                        .is_some_and(|public_key| !public_key.is_empty())
            }
            X402SignatureScheme::MlDsa65 => {
                self.payer_signature_bundle.is_some() && self.payer_public_key_bundle.is_some()
            }
            X402SignatureScheme::Ed25519MlDsa65 => {
                self.payer_signature.as_ref().is_some_and(|signature| !signature.is_empty())
                    && self
                        .payer_public_key
                        .as_ref()
                        .is_some_and(|public_key| !public_key.is_empty())
                    && self.payer_signature_bundle.is_some()
                    && self.payer_public_key_bundle.is_some()
            }
        }
    }

    /// Check if the intent is settled
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.status == X402IntentStatus::Settled && self.tx_hash.is_some()
    }

    /// Try to get canonical JSON for signing (JCS - RFC 8785).
    pub fn try_canonical_signing_data(&self) -> Result<String, X402CryptoError> {
        // Per x402 spec, only signed fields are included
        let payload = serde_json::json!({
            "version": self.version,
            "payer": self.payer_address,
            "payee": self.payee_address,
            "amount": self.amount.to_string(),
            "asset": self.asset.to_string(),
            "chainId": self.chain_id,
            "tokenAddress": self.token_address,
            "nonce": self.nonce,
            "validUntil": self.valid_until,
            "resourceUri": self.resource_uri,
            "resourceMethod": self.resource_method,
        });
        serde_jcs::to_string(&payload).map_err(|e| X402CryptoError::Serialization(e.to_string()))
    }

    /// Get canonical JSON for signing (JCS - RFC 8785).
    pub fn canonical_signing_data(&self) -> Result<String, X402CryptoError> {
        self.try_canonical_signing_data()
    }

    /// Compute sequencer-compatible signing hash (`X402_PAYMENT_V1`)
    #[must_use]
    pub fn sequencer_signing_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        hasher.update(X402_DOMAIN_SEPARATOR.as_bytes());
        hasher.update(self.payer_address.as_bytes());
        hasher.update(self.payee_address.as_bytes());
        hasher.update(self.amount.to_be_bytes());
        hasher.update(format!("{:?}", self.asset).to_lowercase().as_bytes());
        hasher.update(self.network.to_string().as_bytes());
        hasher.update(self.chain_id.to_be_bytes());
        hasher.update(self.valid_until.to_be_bytes());
        hasher.update(self.nonce.to_be_bytes());
        // Bind signatures to the protected resource and method to prevent replay
        // across endpoints with identical payment parameters.
        match &self.resource_uri {
            Some(uri) => {
                hasher.update([1u8]);
                hasher.update((uri.len() as u64).to_be_bytes());
                hasher.update(uri.as_bytes());
            }
            None => hasher.update([0u8]),
        }
        match &self.resource_method {
            Some(method) => {
                hasher.update([1u8]);
                hasher.update((method.len() as u64).to_be_bytes());
                hasher.update(method.as_bytes());
            }
            None => hasher.update([0u8]),
        }

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Return the effective signature scheme, defaulting legacy rows to Ed25519.
    #[must_use]
    pub fn signature_scheme(&self) -> X402SignatureScheme {
        self.payer_signature_scheme.unwrap_or(X402SignatureScheme::Ed25519)
    }

    /// Sign the intent using Ed25519 (sequencer-compatible hash)
    pub fn sign_with_ed25519(&mut self, private_key: &[u8; 32]) -> Result<(), X402CryptoError> {
        let signing_hash = self.sequencer_signing_hash();
        let signing_key = SigningKey::from_bytes(private_key);
        let signature = signing_key.sign(&signing_hash);
        let public_key = signing_key.verifying_key();

        self.signing_hash = Some(hex0x(signing_hash));
        self.payer_signature_scheme = Some(X402SignatureScheme::Ed25519);
        self.payer_signature = Some(hex0x(signature.to_bytes()));
        self.payer_public_key = Some(hex0x(public_key.to_bytes()));
        self.payer_signature_bundle = None;
        self.payer_public_key_bundle = None;
        self.status = X402IntentStatus::Signed;
        Ok(())
    }

    /// Sign the intent using hybrid Ed25519 + ML-DSA-65.
    pub fn sign_with_hybrid(
        &mut self,
        keypair: &HybridSigningKeypair,
    ) -> Result<(), X402CryptoError> {
        let signing_hash = self.sequencer_signing_hash();
        let signature = hybrid_sign_event_hash(&signing_hash, &keypair.private)
            .map_err(|e| X402CryptoError::InvalidKey(e.to_string()))?;

        self.signing_hash = Some(hex0x(signing_hash));
        self.payer_signature_scheme = Some(X402SignatureScheme::Ed25519MlDsa65);
        self.payer_signature = Some(hex0x(signature.ed25519_signature));
        self.payer_public_key = Some(hex0x(keypair.public.ed25519_public_key));
        self.payer_signature_bundle =
            Some(X402SignatureBundle { ml_dsa_65_signature: signature.ml_dsa_65_signature });
        self.payer_public_key_bundle = Some(X402PublicKeyBundle {
            ml_dsa_65_public_key: keypair.public.ml_dsa_65_public_key.clone(),
        });
        self.status = X402IntentStatus::Signed;
        Ok(())
    }

    /// Sign the intent using PQC-strict ML-DSA-65.
    pub fn sign_with_strict(
        &mut self,
        keypair: &StrictSigningKeypair,
    ) -> Result<(), X402CryptoError> {
        let signing_hash = self.sequencer_signing_hash();
        let signature = strict_sign_event_hash(&signing_hash, &keypair.private)
            .map_err(|e| X402CryptoError::InvalidKey(e.to_string()))?;

        self.signing_hash = Some(hex0x(signing_hash));
        self.payer_signature_scheme = Some(X402SignatureScheme::MlDsa65);
        self.payer_signature = None;
        self.payer_public_key = None;
        self.payer_signature_bundle = Some(X402SignatureBundle { ml_dsa_65_signature: signature });
        self.payer_public_key_bundle = Some(X402PublicKeyBundle {
            ml_dsa_65_public_key: keypair.public.ml_dsa_65_public_key.clone(),
        });
        self.status = X402IntentStatus::Signed;
        Ok(())
    }

    /// Verify the configured x402 signature against the sequencer-compatible hash.
    pub fn verify_signature(&self) -> Result<bool, X402CryptoError> {
        let signing_hash = self.sequencer_signing_hash();

        let stored_hash =
            self.signing_hash.as_deref().ok_or(X402CryptoError::MissingField("signing_hash"))?;
        if decode_hex_array::<32>(stored_hash)? != signing_hash {
            return Ok(false);
        }

        match self.signature_scheme() {
            X402SignatureScheme::Ed25519 => {
                let signature_hex = self
                    .payer_signature
                    .as_deref()
                    .ok_or(X402CryptoError::MissingField("payer_signature"))?;
                let public_key_hex = self
                    .payer_public_key
                    .as_deref()
                    .ok_or(X402CryptoError::MissingField("payer_public_key"))?;

                let signature = Signature::from_bytes(&decode_hex_array::<64>(signature_hex)?);
                let public_key = VerifyingKey::from_bytes(&decode_hex_array::<32>(public_key_hex)?)
                    .map_err(|e| X402CryptoError::InvalidKey(e.to_string()))?;

                Ok(public_key.verify(&signing_hash, &signature).is_ok())
            }
            X402SignatureScheme::MlDsa65 => {
                let signature_bundle = self
                    .payer_signature_bundle
                    .as_ref()
                    .ok_or(X402CryptoError::MissingField("payer_signature_bundle"))?;
                let public_key_bundle = self
                    .payer_public_key_bundle
                    .as_ref()
                    .ok_or(X402CryptoError::MissingField("payer_public_key_bundle"))?;
                let public_key = StrictSigningPublicKey {
                    ml_dsa_65_public_key: public_key_bundle.ml_dsa_65_public_key.clone(),
                };
                Ok(strict_verify_event_signature(
                    &signing_hash,
                    &signature_bundle.ml_dsa_65_signature,
                    &public_key,
                ))
            }
            X402SignatureScheme::Ed25519MlDsa65 => {
                let signature_hex = self
                    .payer_signature
                    .as_deref()
                    .ok_or(X402CryptoError::MissingField("payer_signature"))?;
                let public_key_hex = self
                    .payer_public_key
                    .as_deref()
                    .ok_or(X402CryptoError::MissingField("payer_public_key"))?;
                let signature_bundle = self
                    .payer_signature_bundle
                    .as_ref()
                    .ok_or(X402CryptoError::MissingField("payer_signature_bundle"))?;
                let public_key_bundle = self
                    .payer_public_key_bundle
                    .as_ref()
                    .ok_or(X402CryptoError::MissingField("payer_public_key_bundle"))?;
                let signature = PqcHybridSignatureBundle {
                    ed25519_signature: decode_hex_array::<64>(signature_hex)?,
                    ml_dsa_65_signature: signature_bundle.ml_dsa_65_signature.clone(),
                };
                let public_key = HybridSigningPublicKey {
                    ed25519_public_key: decode_hex_array::<32>(public_key_hex)?,
                    ml_dsa_65_public_key: public_key_bundle.ml_dsa_65_public_key.clone(),
                };

                Ok(hybrid_verify_event_signature(&signing_hash, &signature, &public_key))
            }
        }
    }
}

// =============================================================================
// x402 Payment Response (Server's 402 Response)
// =============================================================================

/// x402 Payment Required Response
///
/// This is returned by servers in the `X-Payment-Required` header when
/// payment is needed to access a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402PaymentRequired {
    /// x402 protocol version
    pub version: String,

    /// Payee (merchant) wallet address
    pub payee_address: String,

    /// Required payment amount in smallest unit
    pub amount: u64,

    /// Human-readable amount
    pub amount_display: String,

    /// Required payment asset
    pub asset: X402Asset,

    /// Accepted networks (in order of preference)
    pub networks: Vec<X402Network>,

    /// Resource being accessed
    pub resource_uri: String,

    /// HTTP method for resource
    pub resource_method: String,

    /// Human-readable description of what payment unlocks
    pub description: Option<String>,

    /// Validity window for payment (seconds from now)
    pub validity_seconds: u64,

    /// Merchant identifier
    pub merchant_id: Option<String>,

    /// Merchant name for display
    pub merchant_name: Option<String>,

    /// Additional terms or conditions
    pub terms: Option<String>,

    /// Timestamp when this response was generated
    pub generated_at: DateTime<Utc>,
}

impl X402PaymentRequired {
    /// Create a new payment required response
    pub fn new(
        payee_address: impl Into<String>,
        amount: u64,
        asset: X402Asset,
        resource_uri: impl Into<String>,
        resource_method: impl Into<String>,
    ) -> Self {
        let decimals = asset.decimals();
        let divisor = 10u64.pow(u32::from(decimals));
        let decimal_amount = Decimal::from(amount) / Decimal::from(divisor);
        let amount_display = format!("{decimal_amount:.6} {asset}");

        Self {
            version: X402_VERSION.to_string(),
            payee_address: payee_address.into(),
            amount,
            amount_display,
            asset,
            networks: vec![X402Network::SetChain, X402Network::Base],
            resource_uri: resource_uri.into(),
            resource_method: resource_method.into(),
            description: None,
            validity_seconds: X402_DEFAULT_VALIDITY_SECONDS,
            merchant_id: None,
            merchant_name: None,
            terms: None,
            generated_at: Utc::now(),
        }
    }

    /// Set accepted networks
    #[must_use]
    pub fn with_networks(mut self, networks: Vec<X402Network>) -> Self {
        self.networks = networks;
        self
    }

    /// Set merchant info
    pub fn with_merchant(mut self, id: impl Into<String>, name: impl Into<String>) -> Self {
        self.merchant_id = Some(id.into());
        self.merchant_name = Some(name.into());
        self
    }

    /// Try encoding as base64 for HTTP header.
    pub fn try_to_header_value(&self) -> std::result::Result<String, serde_json::Error> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let json = serde_json::to_string(self)?;
        Ok(STANDARD.encode(json.as_bytes()))
    }

    /// Encode as base64 for HTTP header.
    pub fn to_header_value(&self) -> std::result::Result<String, serde_json::Error> {
        self.try_to_header_value()
    }

    /// Decode from HTTP header value
    pub fn from_header_value(value: &str) -> Result<Self, String> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let bytes = STANDARD.decode(value).map_err(|e| format!("Invalid base64: {e}"))?;
        let json = String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Invalid JSON: {e}"))
    }
}

// =============================================================================
// x402 Payment Receipt (Proof of Payment)
// =============================================================================

/// x402 Payment Receipt - Proof that payment was made
///
/// This is returned after successful payment and can be used to verify
/// the payment via Merkle inclusion proofs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402PaymentReceipt {
    /// Receipt ID
    #[serde(alias = "id")]
    pub receipt_id: Uuid,

    /// Original payment intent ID
    pub intent_id: Uuid,

    /// Sequence number from sequencer
    pub sequence_number: u64,

    /// Batch ID containing this payment
    pub batch_id: Uuid,

    /// Merkle root of the batch
    pub merkle_root: String,

    /// Merkle inclusion proof (list of sibling hashes)
    pub inclusion_proof: Vec<String>,

    /// Leaf index in the Merkle tree
    pub leaf_index: u32,

    /// Total leaves in the Merkle tree
    pub total_leaves: u32,

    /// On-chain transaction hash (if settled)
    pub tx_hash: Option<String>,

    /// Block number (if settled)
    pub block_number: Option<u64>,

    /// Payment details for verification
    pub payer_address: String,
    pub payee_address: String,
    pub amount: u64,
    pub asset: X402Asset,
    pub network: X402Network,
    pub chain_id: u64,
    pub nonce: u64,
    pub valid_until: u64,
    /// Sequencer signing hash (hex-encoded, 32 bytes)
    pub signing_hash: String,
    /// Signature scheme used for the original payer authorization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer_signature_scheme: Option<X402SignatureScheme>,
    /// Legacy Ed25519 signature (hex-encoded, 64 bytes).
    pub payer_signature: String,
    /// Additional PQC signature material for hybrid or strict signatures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer_signature_bundle: Option<X402SignatureBundle>,

    /// Timestamp
    pub created_at: DateTime<Utc>,
}

impl X402PaymentReceipt {
    /// Verify the inclusion proof against the merkle root
    #[must_use]
    pub fn verify_inclusion(&self) -> bool {
        if self.merkle_root.is_empty() {
            return false;
        }

        if self.total_leaves == 0 || self.leaf_index >= self.total_leaves {
            return false;
        }

        let root = match decode_hex_array::<32>(&self.merkle_root) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let leaf = match payment_leaf_hash(self) {
            Ok(hash) => hash,
            Err(_) => return false,
        };

        let mut proof_hashes = Vec::with_capacity(self.inclusion_proof.len());
        for proof_hash in &self.inclusion_proof {
            match decode_hex_array::<32>(proof_hash) {
                Ok(hash) => proof_hashes.push(hash),
                Err(_) => return false,
            }
        }

        let proof = MerkleProof::<MerkleSha256>::new(proof_hashes);
        proof.verify(root, &[self.leaf_index as usize], &[leaf], self.total_leaves as usize)
    }
}

// =============================================================================
// x402 Credit Ledger (Metered Billing)
// =============================================================================

/// x402 Credit Account - tracks prepaid balances for metered usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402CreditAccount {
    pub id: Uuid,
    pub payer_address: String,
    pub asset: X402Asset,
    pub network: X402Network,
    pub balance: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create a new x402 credit account
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateX402CreditAccount {
    pub payer_address: String,
    pub asset: X402Asset,
    pub network: X402Network,
    pub initial_balance: Option<u64>,
}

/// Credit ledger adjustment (credit or debit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402CreditAdjustment {
    pub payer_address: String,
    pub asset: X402Asset,
    pub network: X402Network,
    pub direction: X402CreditDirection,
    pub amount: u64,
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub metadata: Option<String>,
}

/// x402 credit transaction (ledger entry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402CreditTransaction {
    pub id: Uuid,
    pub account_id: Uuid,
    pub payer_address: String,
    pub asset: X402Asset,
    pub network: X402Network,
    pub direction: X402CreditDirection,
    pub amount: u64,
    pub balance_after: u64,
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Filter for listing credit ledger transactions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct X402CreditTransactionFilter {
    pub payer_address: Option<String>,
    pub asset: Option<X402Asset>,
    pub network: Option<X402Network>,
    pub direction: Option<X402CreditDirection>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// =============================================================================
// x402 Payment Batch (For Sequencer)
// =============================================================================

/// Status of a payment batch
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum X402BatchStatus {
    /// Batch is being assembled
    #[default]
    Pending,
    /// Batch is committed (Merkle root computed)
    Committed,
    /// Batch is being settled on-chain
    Settling,
    /// Batch is fully settled
    Settled,
    /// Batch settlement failed
    Failed,
}

/// x402 Payment Batch - A collection of payment intents for batch settlement
///
/// Multiple payment intents are batched together for gas-efficient
/// settlement on Set Chain L2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402PaymentBatch {
    /// Batch ID
    pub id: Uuid,

    /// Batch status
    pub status: X402BatchStatus,

    /// Target network for settlement
    pub network: X402Network,

    /// Number of payments in batch
    pub payment_count: u32,

    /// Total amount across all payments (by asset)
    pub total_amounts: Vec<(X402Asset, u64)>,

    /// Merkle root of payment hashes
    pub merkle_root: Option<String>,

    /// Previous state root (for chaining)
    pub prev_state_root: Option<String>,

    /// New state root after this batch
    pub new_state_root: Option<String>,

    /// Sequence range [start, end]
    pub sequence_start: u64,
    pub sequence_end: u64,

    /// On-chain settlement details
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub gas_used: Option<u64>,

    /// Timestamps
    pub created_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
}

impl X402PaymentBatch {
    /// Create a new empty batch
    #[must_use]
    pub fn new(network: X402Network) -> Self {
        Self {
            id: Uuid::new_v4(),
            status: X402BatchStatus::Pending,
            network,
            payment_count: 0,
            total_amounts: Vec::new(),
            merkle_root: None,
            prev_state_root: None,
            new_state_root: None,
            sequence_start: 0,
            sequence_end: 0,
            tx_hash: None,
            block_number: None,
            gas_used: None,
            created_at: Utc::now(),
            committed_at: None,
            settled_at: None,
        }
    }
}

// =============================================================================
// Input/Filter Types for API
// =============================================================================

/// Input for creating an x402 payment intent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateX402PaymentIntent {
    /// Payer wallet address
    pub payer_address: String,
    /// Payee wallet address
    pub payee_address: String,
    /// Amount in smallest unit
    pub amount: u64,
    /// Payment asset
    pub asset: X402Asset,
    /// Target network
    pub network: X402Network,
    /// Nonce for replay protection
    pub nonce: Option<u64>,
    /// Validity window in seconds
    pub validity_seconds: Option<u64>,
    /// Resource URI this payment unlocks
    pub resource_uri: Option<String>,
    /// HTTP method for resource
    pub resource_method: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Associated cart ID (for checkout flows)
    pub cart_id: Option<Uuid>,
    /// Associated order ID
    pub order_id: Option<Uuid>,
    /// Associated invoice ID
    pub invoice_id: Option<Uuid>,
    /// Merchant ID
    pub merchant_id: Option<String>,
    /// Idempotency key
    pub idempotency_key: Option<String>,
    /// Additional metadata
    pub metadata: Option<String>,
}

/// Input for signing an x402 payment intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignX402PaymentIntent {
    /// Intent ID to sign
    pub intent_id: Uuid,
    /// Signature scheme used to authorize the intent. Absent = legacy Ed25519.
    pub signature_scheme: Option<X402SignatureScheme>,
    /// Legacy Ed25519 signature (hex-encoded, 64 bytes).
    pub signature: String,
    /// Payer's Ed25519 public key (hex-encoded, 32 bytes).
    pub public_key: String,
    /// Additional PQC signature material for hybrid or strict signatures.
    pub signature_bundle: Option<X402SignatureBundle>,
    /// Additional PQC public-key material for hybrid or strict signatures.
    pub public_key_bundle: Option<X402PublicKeyBundle>,
}

/// Filter for listing x402 payment intents
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct X402PaymentIntentFilter {
    /// Filter by payer address
    pub payer_address: Option<String>,
    /// Filter by payee address
    pub payee_address: Option<String>,
    /// Filter by status
    pub status: Option<X402IntentStatus>,
    /// Filter by network
    pub network: Option<X402Network>,
    /// Filter by asset
    pub asset: Option<X402Asset>,
    /// Filter by order ID
    pub order_id: Option<Uuid>,
    /// Filter by batch ID
    pub batch_id: Option<Uuid>,
    /// Filter by date range
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    /// Pagination
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Generate a unique x402 intent ID
#[must_use]
pub fn generate_x402_intent_id() -> Uuid {
    Uuid::new_v4()
}

/// Calculate amount in smallest unit from decimal
#[must_use]
pub fn to_smallest_unit(amount: Decimal, asset: X402Asset) -> u64 {
    if amount <= Decimal::ZERO {
        return 0;
    }

    let decimals = asset.decimals();
    let multiplier = Decimal::from(10u64.pow(u32::from(decimals)));
    let scaled = amount * multiplier;
    if scaled.fract() != Decimal::ZERO {
        // Prevent positive sub-unit values from silently turning into zero.
        return scaled.ceil().to_u64().unwrap_or(u64::MAX);
    }
    scaled.to_u64().unwrap_or(0)
}

/// Calculate decimal amount from smallest unit
#[must_use]
pub fn from_smallest_unit(amount: u64, asset: X402Asset) -> Decimal {
    let decimals = asset.decimals();
    let divisor = Decimal::from(10u64.pow(u32::from(decimals)));
    Decimal::from(amount) / divisor
}

// =============================================================================
// x402 Crypto Helpers
// =============================================================================

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum X402CryptoError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("invalid hex: {0}")]
    InvalidHex(String),
    #[error("invalid length: expected {expected}, got {got}")]
    InvalidLength { expected: usize, got: usize },
    #[error("invalid key: {0}")]
    InvalidKey(String),
}

fn hex0x(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn decode_hex_array<const N: usize>(value: &str) -> Result<[u8; N], X402CryptoError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).map_err(|e| X402CryptoError::InvalidHex(e.to_string()))?;
    if bytes.len() != N {
        return Err(X402CryptoError::InvalidLength { expected: N, got: bytes.len() });
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn decode_hex_bytes(value: &str) -> Result<Vec<u8>, X402CryptoError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    hex::decode(trimmed).map_err(|e| X402CryptoError::InvalidHex(e.to_string()))
}

fn normalize_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn update_optional_leaf_bytes(
    hasher: &mut Sha256,
    bytes: Option<&[u8]>,
) -> Result<(), X402CryptoError> {
    match bytes {
        Some(bytes) => {
            let len = u64::try_from(bytes.len())
                .map_err(|_| X402CryptoError::Serialization("byte slice too large".to_string()))?;
            hasher.update([1u8]);
            hasher.update(len.to_be_bytes());
            hasher.update(bytes);
        }
        None => hasher.update([0u8]),
    }
    Ok(())
}

fn payment_leaf_hash(receipt: &X402PaymentReceipt) -> Result<[u8; 32], X402CryptoError> {
    let mut hasher = Sha256::new();

    hasher.update(receipt.intent_id.as_bytes());
    hasher.update(receipt.sequence_number.to_be_bytes());

    hasher.update(receipt.payer_address.as_bytes());
    hasher.update(receipt.payee_address.as_bytes());
    hasher.update(receipt.amount.to_be_bytes());
    hasher.update(receipt.asset.to_string().to_lowercase().as_bytes());
    hasher.update(receipt.network.to_string().as_bytes());
    hasher.update(receipt.chain_id.to_be_bytes());
    hasher.update(receipt.nonce.to_be_bytes());
    hasher.update(receipt.valid_until.to_be_bytes());

    let signing_hash = decode_hex_array::<32>(&receipt.signing_hash)?;
    let legacy_signature =
        normalize_optional_string(&receipt.payer_signature).map(|sig| decode_hex_bytes(&sig));
    let legacy_signature = legacy_signature.transpose()?;
    hasher.update(signing_hash);
    hasher.update(
        receipt
            .payer_signature_scheme
            .unwrap_or(X402SignatureScheme::Ed25519)
            .to_string()
            .as_bytes(),
    );
    update_optional_leaf_bytes(&mut hasher, legacy_signature.as_deref())?;
    update_optional_leaf_bytes(
        &mut hasher,
        receipt.payer_signature_bundle.as_ref().map(|bundle| bundle.ml_dsa_65_signature.as_slice()),
    )?;

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_crypto::pqc::{generate_hybrid_signing_keypair, generate_strict_signing_keypair};

    #[test]
    fn test_x402_payment_intent_creation() {
        let intent = X402PaymentIntent::new(
            "0x1234567890abcdef1234567890abcdef12345678",
            "0xabcdef1234567890abcdef1234567890abcdef12",
            1_000_000, // 1 USDC
            X402Asset::Usdc,
            X402Network::SetChain,
        );

        assert_eq!(intent.amount, 1_000_000);
        assert_eq!(intent.amount_decimal, Decimal::from(1));
        assert_eq!(intent.asset, X402Asset::Usdc);
        assert_eq!(intent.network, X402Network::SetChain);
        assert_eq!(intent.chain_id, 84532001);
        assert!(!intent.is_expired());
        assert!(!intent.is_signed());
    }

    #[test]
    fn test_x402_network_chain_ids() {
        assert_eq!(X402Network::SetChain.chain_id(), 84532001);
        assert_eq!(X402Network::Base.chain_id(), 8453);
        assert_eq!(X402Network::Ethereum.chain_id(), 1);
    }

    #[test]
    fn test_x402_asset_decimals() {
        assert_eq!(X402Asset::Usdc.decimals(), 6);
        assert_eq!(X402Asset::Dai.decimals(), 18);
        assert_eq!(X402Asset::Eth.decimals(), 18);
    }

    #[test]
    fn test_amount_conversion() {
        let decimal = Decimal::from(100);
        let smallest = to_smallest_unit(decimal, X402Asset::Usdc);
        assert_eq!(smallest, 100_000_000); // 100 USDC = 100,000,000 (6 decimals)

        let back = from_smallest_unit(smallest, X402Asset::Usdc);
        assert_eq!(back, decimal);
    }

    #[test]
    fn test_amount_conversion_rounds_up_sub_precision() {
        let decimal = Decimal::new(1, 7); // 0.0000001
        let smallest = to_smallest_unit(decimal, X402Asset::Usdc);
        assert_eq!(smallest, 1);
    }

    #[test]
    fn test_amount_conversion_non_positive_maps_to_zero() {
        assert_eq!(to_smallest_unit(Decimal::ZERO, X402Asset::Usdc), 0);
        assert_eq!(to_smallest_unit(Decimal::new(-1, 0), X402Asset::Usdc), 0);
    }

    #[test]
    fn test_signature_fails_when_resource_binding_changes() {
        let mut intent = X402PaymentIntent::new(
            "0x1234567890abcdef1234567890abcdef12345678",
            "0xabcdef1234567890abcdef1234567890abcdef12",
            1_000_000,
            X402Asset::Usdc,
            X402Network::SetChain,
        )
        .with_resource("/a", "GET")
        .with_nonce(42);

        intent.sign_with_ed25519(&[7u8; 32]).unwrap();
        assert!(intent.verify_signature().unwrap());

        let mut replayed = intent.clone();
        replayed.resource_uri = Some("/premium".to_string());
        assert!(!replayed.verify_signature().unwrap());

        let mut method_changed = intent;
        method_changed.resource_method = Some("POST".to_string());
        assert!(!method_changed.verify_signature().unwrap());
    }

    #[test]
    fn test_hybrid_signature_verifies() {
        let mut intent = X402PaymentIntent::new(
            "0x1234567890abcdef1234567890abcdef12345678",
            "0xabcdef1234567890abcdef1234567890abcdef12",
            1_000_000,
            X402Asset::Usdc,
            X402Network::SetChain,
        )
        .with_resource("/hybrid", "POST")
        .with_nonce(7);
        let keypair = generate_hybrid_signing_keypair().unwrap();

        intent.sign_with_hybrid(&keypair).unwrap();

        assert_eq!(intent.payer_signature_scheme, Some(X402SignatureScheme::Ed25519MlDsa65));
        assert!(intent.payer_signature_bundle.is_some());
        assert!(intent.payer_public_key_bundle.is_some());
        assert!(intent.verify_signature().unwrap());
    }

    #[test]
    fn test_strict_signature_verifies() {
        let mut intent = X402PaymentIntent::new(
            "0x1234567890abcdef1234567890abcdef12345678",
            "0xabcdef1234567890abcdef1234567890abcdef12",
            1_000_000,
            X402Asset::Usdc,
            X402Network::SetChain,
        )
        .with_resource("/strict", "POST")
        .with_nonce(9);
        let keypair = generate_strict_signing_keypair().unwrap();

        intent.sign_with_strict(&keypair).unwrap();

        assert_eq!(intent.payer_signature_scheme, Some(X402SignatureScheme::MlDsa65));
        assert!(intent.payer_signature.is_none());
        assert!(intent.payer_public_key.is_none());
        assert!(intent.verify_signature().unwrap());
    }

    #[test]
    fn test_x402_payment_required_header() {
        let req =
            X402PaymentRequired::new("0xpayee", 1_000_000, X402Asset::Usdc, "/api/resource", "GET");

        let header = req.to_header_value().unwrap();
        let decoded = X402PaymentRequired::from_header_value(&header).unwrap();

        assert_eq!(decoded.payee_address, "0xpayee");
        assert_eq!(decoded.amount, 1_000_000);
    }

    #[test]
    fn test_x402_merkle_inclusion_verification() {
        let mut receipt = X402PaymentReceipt {
            receipt_id: Uuid::new_v4(),
            intent_id: Uuid::new_v4(),
            sequence_number: 42,
            batch_id: Uuid::new_v4(),
            merkle_root: String::new(),
            inclusion_proof: vec![],
            leaf_index: 0,
            total_leaves: 0,
            tx_hash: None,
            block_number: None,
            payer_address: "0xpayer".to_string(),
            payee_address: "0xpayee".to_string(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            chain_id: X402Network::SetChain.chain_id(),
            nonce: 7,
            valid_until: 1_705_320_000,
            signing_hash: format!("0x{}", "11".repeat(32)),
            payer_signature_scheme: Some(X402SignatureScheme::Ed25519),
            payer_signature: format!("0x{}", "22".repeat(64)),
            payer_signature_bundle: None,
            created_at: Utc::now(),
        };

        let mut other = receipt.clone();
        other.intent_id = Uuid::new_v4();
        other.sequence_number = 43;
        other.nonce = 8;
        other.signing_hash = format!("0x{}", "33".repeat(32));
        other.payer_signature = format!("0x{}", "44".repeat(64));

        let leaf = payment_leaf_hash(&receipt).unwrap();
        let other_leaf = payment_leaf_hash(&other).unwrap();

        let leaves = vec![leaf, other_leaf];
        let tree = rs_merkle::MerkleTree::<MerkleSha256>::from_leaves(&leaves);
        let root = tree.root().expect("merkle root");
        let proof = tree.proof(&[0]);

        receipt.inclusion_proof =
            proof.proof_hashes().iter().map(|h| format!("0x{}", hex::encode(h))).collect();
        receipt.merkle_root = format!("0x{}", hex::encode(root));
        receipt.total_leaves = leaves.len() as u32;
        receipt.leaf_index = 0;

        assert!(receipt.verify_inclusion());
    }
}
