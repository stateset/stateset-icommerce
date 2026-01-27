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
//! ```rust,ignore
//! use stateset_core::x402::{X402PaymentIntent, X402Network, X402Asset};
//!
//! let intent = X402PaymentIntent::new(
//!     "0x1234...sender",
//!     "0x5678...recipient",
//!     1_000_000, // 1 USDC (6 decimals)
//!     X402Asset::Usdc,
//!     X402Network::SetChain,
//! );
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use thiserror::Error;
use ed25519_dalek::{Signer, Verifier, SigningKey, VerifyingKey, Signature};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum X402Network {
    /// Set Chain L2 (StateSet native) - primary network
    #[default]
    SetChain,
    /// Set Chain testnet
    SetChainTestnet,
    /// Base L2 (Coinbase)
    Base,
    /// Arc L2 (Circle stablecoin-native)
    Arc,
    /// Arc testnet
    ArcTestnet,
    /// Base Sepolia testnet
    BaseSepolia,
    /// Ethereum mainnet
    Ethereum,
    /// Ethereum Sepolia testnet
    EthereumSepolia,
    /// Arbitrum One
    Arbitrum,
    /// Optimism
    Optimism,
}

impl X402Network {
    /// Get the chain ID for this network
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::SetChain => 84532001,        // Set Chain mainnet
            Self::SetChainTestnet => 84532002, // Set Chain testnet
            Self::Arc => 5042001,        // Arc mainnet
            Self::ArcTestnet => 5042002, // Arc testnet
            Self::Base => 8453,
            Self::BaseSepolia => 84532,
            Self::Ethereum => 1,
            Self::EthereumSepolia => 11155111,
            Self::Arbitrum => 42161,
            Self::Optimism => 10,
        }
    }

    /// Check if this is a testnet
    pub fn is_testnet(&self) -> bool {
        matches!(
            self,
            Self::SetChainTestnet | Self::ArcTestnet | Self::BaseSepolia | Self::EthereumSepolia
        )
    }
}

impl std::fmt::Display for X402Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetChain => write!(f, "set_chain"),
            Self::SetChainTestnet => write!(f, "set_chain_testnet"),
            Self::Arc => write!(f, "arc"),
            Self::ArcTestnet => write!(f, "arc_testnet"),
            Self::Base => write!(f, "base"),
            Self::BaseSepolia => write!(f, "base_sepolia"),
            Self::Ethereum => write!(f, "ethereum"),
            Self::EthereumSepolia => write!(f, "ethereum_sepolia"),
            Self::Arbitrum => write!(f, "arbitrum"),
            Self::Optimism => write!(f, "optimism"),
        }
    }
}

impl std::str::FromStr for X402Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "set_chain" | "set" | "ssc" => Ok(Self::SetChain),
            "set_chain_testnet" | "set_testnet" => Ok(Self::SetChainTestnet),
            "arc" => Ok(Self::Arc),
            "arc_testnet" | "arc-testnet" => Ok(Self::ArcTestnet),
            "base" => Ok(Self::Base),
            "base_sepolia" => Ok(Self::BaseSepolia),
            "ethereum" | "eth" | "mainnet" => Ok(Self::Ethereum),
            "ethereum_sepolia" | "sepolia" => Ok(Self::EthereumSepolia),
            "arbitrum" | "arb" => Ok(Self::Arbitrum),
            "optimism" | "op" => Ok(Self::Optimism),
            _ => Err(format!("Unknown x402 network: {}", s)),
        }
    }
}

/// Supported payment assets for x402
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum X402Asset {
    /// USD Coin (USDC) - primary stablecoin
    #[default]
    Usdc,
    /// Tether (USDT)
    Usdt,
    /// StateSet USD (ssUSD) - yield-bearing stablecoin
    SsUsd,
    /// Wrapped StateSet USD (ERC-4626)
    WssUsd,
    /// DAI stablecoin
    Dai,
    /// Native ETH (for gas)
    Eth,
}

impl X402Asset {
    /// Get the number of decimals for this asset
    pub fn decimals(&self) -> u8 {
        match self {
            Self::Usdc | Self::Usdt | Self::SsUsd | Self::WssUsd => 6,
            Self::Dai | Self::Eth => 18,
        }
    }

    /// Get the token contract address for a given network
    pub fn contract_address(&self, network: X402Network) -> Option<&'static str> {
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
            (Self::Usdc, X402Network::Base) => {
                Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")
            }
            // Ethereum addresses
            (Self::Usdc, X402Network::Ethereum) => {
                Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
            }
            (Self::Usdt, X402Network::Ethereum) => {
                Some("0xdAC17F958D2ee523a2206206994597C13D831ec7")
            }
            (Self::Dai, X402Network::Ethereum) => {
                Some("0x6B175474E89094C44Da98b954Ee4606eB48")
            }
            // Native ETH has no contract
            (Self::Eth, _) => None,
            _ => None,
        }
    }
}

impl std::fmt::Display for X402Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usdc => write!(f, "USDC"),
            Self::Usdt => write!(f, "USDT"),
            Self::SsUsd => write!(f, "ssUSD"),
            Self::WssUsd => write!(f, "wssUSD"),
            Self::Dai => write!(f, "DAI"),
            Self::Eth => write!(f, "ETH"),
        }
    }
}

impl std::str::FromStr for X402Asset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "USDC" => Ok(Self::Usdc),
            "USDT" | "TETHER" => Ok(Self::Usdt),
            "SSUSD" | "SS_USD" => Ok(Self::SsUsd),
            "WSSUSD" | "WSS_USD" => Ok(Self::WssUsd),
            "DAI" => Ok(Self::Dai),
            "ETH" | "ETHER" => Ok(Self::Eth),
            _ => Err(format!("Unknown x402 asset: {}", s)),
        }
    }
}

// =============================================================================
// x402 Payment Intent (Off-Chain Signed Payment Request)
// =============================================================================

/// Status of an x402 payment intent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for X402IntentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Signed => write!(f, "signed"),
            Self::Sequenced => write!(f, "sequenced"),
            Self::Batched => write!(f, "batched"),
            Self::Settled => write!(f, "settled"),
            Self::Expired => write!(f, "expired"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
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
    /// Format: SHA256(X402_DOMAIN_SEPARATOR || canonical_json)
    pub signing_hash: Option<String>,

    /// Payer's Ed25519 signature over signing_hash (hex-encoded)
    pub payer_signature: Option<String>,

    /// Payer's public key (hex-encoded, 32 bytes)
    pub payer_public_key: Option<String>,

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
        let divisor = 10u64.pow(decimals as u32);
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
            order_id: None,
            invoice_id: None,
            merchant_id: None,
            signing_hash: None,
            payer_signature: None,
            payer_public_key: None,
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
    pub fn with_validity(mut self, seconds: u64) -> Self {
        self.valid_until = self.created_at_unix + seconds.min(X402_MAX_VALIDITY_SECONDS);
        self
    }

    /// Set the nonce for replay protection
    pub fn with_nonce(mut self, nonce: u64) -> Self {
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
    pub fn with_order(mut self, order_id: Uuid) -> Self {
        self.order_id = Some(order_id);
        self
    }

    /// Set the idempotency key
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Check if the intent has expired
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp() as u64;
        now > self.valid_until
    }

    /// Check if the intent is signed
    pub fn is_signed(&self) -> bool {
        matches!(
            (
                self.payer_signature.as_ref(),
                self.payer_public_key.as_ref(),
                self.signing_hash.as_ref()
            ),
            (Some(signature), Some(public_key), Some(signing_hash))
                if !signature.is_empty() && !public_key.is_empty() && !signing_hash.is_empty()
        )
    }

    /// Check if the intent is settled
    pub fn is_settled(&self) -> bool {
        self.status == X402IntentStatus::Settled && self.tx_hash.is_some()
    }

    /// Get the canonical JSON for signing (JCS - RFC 8785)
    pub fn canonical_signing_data(&self) -> String {
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
        });
        serde_jcs::to_string(&payload).unwrap_or_default()
    }

    /// Compute sequencer-compatible signing hash (X402_PAYMENT_V1)
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

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Sign the intent using Ed25519 (sequencer-compatible hash)
    pub fn sign_with_ed25519(&mut self, private_key: &[u8; 32]) -> Result<(), X402CryptoError> {
        let signing_hash = self.sequencer_signing_hash();
        let signing_key = SigningKey::from_bytes(private_key);
        let signature = signing_key.sign(&signing_hash);
        let public_key = signing_key.verifying_key();

        self.signing_hash = Some(hex0x(&signing_hash));
        self.payer_signature = Some(hex0x(signature.to_bytes()));
        self.payer_public_key = Some(hex0x(public_key.to_bytes()));
        self.status = X402IntentStatus::Signed;
        Ok(())
    }

    /// Verify Ed25519 signature against sequencer-compatible hash
    pub fn verify_signature(&self) -> Result<bool, X402CryptoError> {
        let signing_hash = self.sequencer_signing_hash();

        let stored_hash = self
            .signing_hash
            .as_deref()
            .ok_or(X402CryptoError::MissingField("signing_hash"))?;
        if decode_hex_array::<32>(stored_hash)? != signing_hash {
            return Ok(false);
        }

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
        let divisor = 10u64.pow(decimals as u32);
        let decimal_amount = Decimal::from(amount) / Decimal::from(divisor);
        let amount_display = format!("{:.6} {}", decimal_amount, asset);

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

    /// Encode as base64 for HTTP header
    pub fn to_header_value(&self) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let json = serde_json::to_string(self).unwrap_or_default();
        STANDARD.encode(json.as_bytes())
    }

    /// Decode from HTTP header value
    pub fn from_header_value(value: &str) -> Result<Self, String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let bytes = STANDARD
            .decode(value)
            .map_err(|e| format!("Invalid base64: {}", e))?;
        let json = String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {}", e))?;
        serde_json::from_str(&json).map_err(|e| format!("Invalid JSON: {}", e))
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
    pub id: Uuid,

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

    /// Timestamp
    pub created_at: DateTime<Utc>,
}

impl X402PaymentReceipt {
    /// Verify the inclusion proof against the merkle root
    pub fn verify_inclusion(&self) -> bool {
        if self.merkle_root.is_empty() {
            return false;
        }

        let root_bytes = match decode_hex_bytes(&self.merkle_root) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let mut hash = payment_leaf_hash(self);
        for proof_hash in &self.inclusion_proof {
            let sibling = match decode_hex_bytes(proof_hash) {
                Ok(bytes) => bytes,
                Err(_) => return false,
            };
            hash = hash_pair(&hash, &sibling);
        }

        hash == root_bytes
    }
}

// =============================================================================
// x402 Payment Batch (For Sequencer)
// =============================================================================

/// Status of a payment batch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    /// Ed25519 signature (hex-encoded, 64 bytes)
    pub signature: String,
    /// Payer's public key (hex-encoded, 32 bytes)
    pub public_key: String,
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
pub fn generate_x402_intent_id() -> Uuid {
    Uuid::new_v4()
}

/// Calculate amount in smallest unit from decimal
pub fn to_smallest_unit(amount: Decimal, asset: X402Asset) -> u64 {
    let decimals = asset.decimals();
    let multiplier = Decimal::from(10u64.pow(decimals as u32));
    let scaled = amount * multiplier;
    if scaled.fract() != Decimal::ZERO {
        return 0;
    }
    scaled.to_u64().unwrap_or(0)
}

/// Calculate decimal amount from smallest unit
pub fn from_smallest_unit(amount: u64, asset: X402Asset) -> Decimal {
    let decimals = asset.decimals();
    let divisor = Decimal::from(10u64.pow(decimals as u32));
    Decimal::from(amount) / divisor
}


// =============================================================================
// x402 Crypto Helpers
// =============================================================================

#[derive(Debug, Error)]
pub enum X402CryptoError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_x402_payment_required_header() {
        let req = X402PaymentRequired::new(
            "0xpayee",
            1_000_000,
            X402Asset::Usdc,
            "/api/resource",
            "GET",
        );

        let header = req.to_header_value();
        let decoded = X402PaymentRequired::from_header_value(&header).unwrap();

        assert_eq!(decoded.payee_address, "0xpayee");
        assert_eq!(decoded.amount, 1_000_000);
    }

    #[test]
    fn test_x402_merkle_inclusion_verification() {
        let receipt = X402PaymentReceipt {
            id: Uuid::new_v4(),
            intent_id: Uuid::new_v4(),
            sequence_number: 42,
            batch_id: Uuid::new_v4(),
            merkle_root: String::new(),
            inclusion_proof: vec![],
            leaf_index: 0,
            tx_hash: None,
            block_number: None,
            payer_address: "0xpayer".to_string(),
            payee_address: "0xpayee".to_string(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            created_at: Utc::now(),
        };

        let leaf = payment_leaf_hash(&receipt);
        let sibling = sha256_bytes(b"dummy");
        let root = hash_pair(&leaf, &sibling);

        let mut updated = receipt;
        updated.inclusion_proof = vec![hex::encode(sibling)];
        updated.merkle_root = hex::encode(root);

        assert!(updated.verify_inclusion());
    }
}

fn sha256_bytes(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn hash_pair(left: &[u8], right: &[u8]) -> Vec<u8> {
    if left <= right {
        sha256_bytes(&[left, right].concat())
    } else {
        sha256_bytes(&[right, left].concat())
    }
}

fn decode_hex_bytes(value: &str) -> Result<Vec<u8>, hex::FromHexError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    hex::decode(trimmed)
}

fn payment_leaf_hash(receipt: &X402PaymentReceipt) -> Vec<u8> {
    let payload = serde_json::json!({
        "intentId": receipt.intent_id,
        "sequenceNumber": receipt.sequence_number,
        "payer": receipt.payer_address,
        "payee": receipt.payee_address,
        "amount": receipt.amount.to_string(),
        "asset": receipt.asset.to_string(),
        "network": receipt.network.to_string(),
    });
    let canonical = serde_jcs::to_string(&payload).unwrap_or_default();
    sha256_bytes(canonical.as_bytes())
}
