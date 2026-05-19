//! ICP-1.0 wire types matching the `JavaScript` reference SDK
//! (`packages/icp-client/src/index.mjs`).
//!
//! Field naming and serialization follows the wire format the merchant
//! handler validates — not the documented spec (those are still being
//! reconciled). All amounts are decimal strings to preserve canonical
//! bytes through canonicalization.

use serde::{Deserialize, Serialize};

/// Agent Identifier per ICP-1.0 §4.2.
pub type AID = String;

/// Monetary amount + currency. The `amount` is a canonical decimal string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Decimal amount as a string.
    pub amount: String,
    /// ISO 4217 fiat code or registered stablecoin code.
    pub currency: String,
}

/// Inner `authority` object inside `principal_binding`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authority {
    /// Max amount per single Intent.
    pub max_per_intent: Money,
    /// Verbs the Agent is authorized to call.
    pub verbs: Vec<String>,
    /// Optional cap on payout requests (used by `payout.request`, per ICPIP-0004).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per_payout: Option<Money>,
}

/// Principal-to-Agent binding. Wire shape matches the JS SDK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipalBinding {
    /// Principal identifier (typically a DID).
    pub principal: String,
    /// Agent's AID.
    pub agent: AID,
    /// Authority caps.
    pub authority: Authority,
    /// Binding expiry (RFC 3339).
    pub expiry: String,
    /// Revocation list URL.
    pub revocation: String,
    /// Principal-issued signature attesting to this binding.
    pub signature: Signature,
}

/// Signature envelope `{alg, kid, sig}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// Signing algorithm (e.g. "ed25519").
    pub alg: String,
    /// Key identifier (AID).
    pub kid: String,
    /// Hex-encoded signature bytes.
    pub sig: String,
}

/// Fields common to every Intent verb. Wire shape per the JS SDK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentBase {
    /// Protocol version (e.g. "icp-1.0").
    pub v: String,
    /// Verb name (e.g. "purchase.create").
    pub verb: String,
    /// Unique Intent identifier (random).
    pub intent_id: String,
    /// Buyer AID.
    pub buyer: AID,
    /// Merchant AID (must match the handler's published merchant AID).
    pub merchant: AID,
    /// Settler identifier (must be in merchant allowlist).
    pub settler: String,
    /// Convenience copy of `exp` (RFC 3339).
    pub expiry: String,
    /// Principal-Agent binding.
    pub principal_binding: PrincipalBinding,
    /// 16-byte random nonce as hex (32 hex chars).
    pub nonce: String,
    /// Issuance timestamp (RFC 3339).
    pub iat: String,
    /// Expiry timestamp (RFC 3339, must be ≤ iat + 600s).
    pub exp: String,
}

/// Line item in a purchase request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    /// SKU identifier.
    pub sku: String,
    /// Quantity (must be ≥ 1).
    pub quantity: u32,
    /// Unit price.
    pub unit_price: Money,
}
