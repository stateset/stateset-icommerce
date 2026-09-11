//! Principal keys and `PrincipalBinding` signing.
//!
//! A *principal* is an organization (`did:web:…`), not an Agent: it has no
//! AID and no X25519 half, only an Ed25519 key whose public half an operator
//! registers with the handler (`ICP_PRINCIPAL_KEYS_JSON`). The separation is
//! the point — an Agent that could sign its own delegation proves nothing.

use crate::time::{format_rfc3339_millis, now_millis, parse_rfc3339_millis};
use crate::types::{Authority, Money, PrincipalBinding, Signature};
use crate::{Error, canonical_json};
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;

/// Default `authority.verbs`: every verb this client can emit, and nothing
/// wider. A binding is a capability grant, so a default that authorized verbs
/// the SDK cannot even send would hand out authority for no reason; a default
/// narrower than the method surface would make the client answer
/// `delegation.scope_mismatch` from its own methods.
///
/// Matches the Python SDK's `DEFAULT_VERBS` (both expose `payout.request`;
/// the `JavaScript` SDK does not, so its list is one shorter).
pub const DEFAULT_VERBS: [&str; 8] = [
    "channel.register",
    "inventory.query",
    "payout.request",
    "purchase.create",
    "purchase.return",
    "quote.request",
    "subscription.cancel",
    "subscription.create",
];

/// Default per-Intent spend ceiling carried in the `PrincipalBinding`.
pub fn default_max_per_intent() -> Money {
    Money { amount: "10000".to_string(), currency: "USDC".to_string() }
}

/// Default `PrincipalBinding` lifetime: 24h, the §5.3 ceiling for non-Intents.
const DEFAULT_BINDING_TTL_MS: i64 = 86_400_000;

/// A principal's Ed25519 signing key.
///
/// Persist [`PrincipalIdentity::seed`] in a KMS or secret store and restore
/// it with [`PrincipalIdentity::from_seed_hex`]; the public half
/// ([`PrincipalIdentity::ed_pubkey_hex`]) is what you hand the merchant for
/// their `ICP_PRINCIPAL_KEYS_JSON`.
#[derive(Clone)]
pub struct PrincipalIdentity {
    signing: SigningKey,
    pub_raw: [u8; 32],
}

impl std::fmt::Debug for PrincipalIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The seed never reaches a log line through this type.
        f.debug_struct("PrincipalIdentity")
            .field("ed_pubkey", &hex::encode(self.pub_raw))
            .finish_non_exhaustive()
    }
}

impl PrincipalIdentity {
    /// Generate a fresh principal signing key from the OS RNG.
    pub fn generate() -> Self {
        use rand_core::{OsRng, RngCore};
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self::from_seed(&seed)
    }

    /// Restore a principal key from its 32-byte Ed25519 seed.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing = SigningKey::from_bytes(seed);
        let pub_raw = signing.verifying_key().to_bytes();
        Self { signing, pub_raw }
    }

    /// Restore a principal key from a hex-encoded 32-byte Ed25519 seed.
    pub fn from_seed_hex(seed_hex: &str) -> Result<Self, Error> {
        let bytes = hex::decode(seed_hex.trim())
            .map_err(|e| Error::InvalidInput(format!("principal seed hex: {e}")))?;
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::InvalidInput("principal seed must be 32 bytes".to_string()))?;
        Ok(Self::from_seed(&seed))
    }

    /// The 32-byte seed, for persistence into a KMS or secret store.
    pub fn seed(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    /// Raw 32-byte Ed25519 public key.
    pub const fn ed_pubkey(&self) -> [u8; 32] {
        self.pub_raw
    }

    /// Hex-encoded Ed25519 public key — the value an operator registers in
    /// the handler's `ICP_PRINCIPAL_KEYS_JSON`.
    pub fn ed_pubkey_hex(&self) -> String {
        hex::encode(self.pub_raw)
    }

    /// Sign a message and hex-encode the 64-byte signature.
    pub fn sign_hex(&self, message: &[u8]) -> String {
        hex::encode(self.signing.sign(message).to_bytes())
    }
}

/// Inputs to [`sign_principal_binding`].
///
/// Every field except `principal` and `agent` has a default that mirrors the
/// `JavaScript` and Python SDKs, so the common case is
/// `PrincipalBindingParams::new(principal, agent).sign(&key)`.
#[derive(Debug, Clone)]
pub struct PrincipalBindingParams {
    principal: String,
    agent: String,
    expires_at: Option<String>,
    verbs: Option<Vec<String>>,
    max_per_intent: Option<Money>,
    max_per_payout: Option<Money>,
    revocation: Option<String>,
}

impl PrincipalBindingParams {
    /// Delegate `agent` to act for `principal`.
    pub fn new(principal: impl Into<String>, agent: impl Into<String>) -> Self {
        Self {
            principal: principal.into(),
            agent: agent.into(),
            expires_at: None,
            verbs: None,
            max_per_intent: None,
            max_per_payout: None,
            revocation: None,
        }
    }

    /// Binding expiry as an RFC 3339 string. Default: 24h from now.
    ///
    /// Normalized to the millisecond `…Z` form every SDK emits, so the same
    /// instant spelled three ways signs the same bytes.
    ///
    /// `Z` and a numeric `±HH:MM` / `±HHMM` offset are both read as written.
    /// A string carrying **no offset at all** (`2026-01-01T00:00:00`) is read
    /// as **UTC**, matching the Python SDK's naive-`datetime` handling — not
    /// as local time, which is what `JavaScript`'s `new Date()` would make of
    /// the same string. Pass an explicit offset if the distinction matters:
    /// an expiry that means a different instant in a different SDK is an
    /// expiry two parties disagree about.
    #[must_use]
    pub fn expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }

    /// Verbs the Agent may call. Default: [`DEFAULT_VERBS`].
    #[must_use]
    pub fn verbs<I, S>(mut self, verbs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.verbs = Some(verbs.into_iter().map(Into::into).collect());
        self
    }

    /// Per-Intent spend ceiling. Default: 10000 USDC.
    #[must_use]
    pub fn max_per_intent(mut self, max_per_intent: Money) -> Self {
        self.max_per_intent = Some(max_per_intent);
        self
    }

    /// Optional per-payout ceiling (ICPIP-0004). Omitted from the binding
    /// entirely when unset, which is what the handler treats as "uncapped".
    #[must_use]
    pub fn max_per_payout(mut self, max_per_payout: Money) -> Self {
        self.max_per_payout = Some(max_per_payout);
        self
    }

    /// Revocation list URL. Default: `https://example.com/icp-revocation/<agent>`.
    #[must_use]
    pub fn revocation(mut self, revocation: impl Into<String>) -> Self {
        self.revocation = Some(revocation.into());
        self
    }

    /// Sign these params — see [`sign_principal_binding`].
    pub fn sign(&self, principal_identity: &PrincipalIdentity) -> Result<PrincipalBinding, Error> {
        sign_principal_binding(self, principal_identity)
    }
}

/// The signing input: the binding with its own `signature` field absent.
///
/// A struct rather than a `Value` with a key deleted, so it is impossible to
/// sign over a shape the binding does not actually have.
#[derive(Serialize)]
struct BindingBody<'a> {
    principal: &'a str,
    agent: &'a str,
    authority: &'a Authority,
    expiry: &'a str,
    revocation: &'a str,
}

/// Sign a `PrincipalBinding`: the principal's statement that this Agent may
/// act for it, over these verbs, up to this ceiling, until this expiry.
///
/// This is the artifact a handler checks before it will quote anything. The
/// canonical bytes are `canonical_json(binding)` with the `signature` field
/// removed — the rule the reference handler's `checkDelegation` applies — so
/// every other field is covered, and mutating one after signing invalidates
/// the signature.
///
/// Given the same inputs, byte-identical to the `JavaScript` SDK's
/// `signPrincipalBinding` and the Python SDK's `sign_principal_binding`;
/// `tests/principal_binding_vector.rs` asserts that against the vector all
/// three suites read.
///
/// # Errors
/// [`Error::InvalidInput`] when `principal` or `agent` is empty (or the
/// principal carries surrounding whitespace, which the handler compares
/// exactly), when `verbs` is empty, or when `expires_at` is not RFC 3339.
pub fn sign_principal_binding(
    params: &PrincipalBindingParams,
    principal_identity: &PrincipalIdentity,
) -> Result<PrincipalBinding, Error> {
    if params.principal.is_empty() || params.principal.trim() != params.principal {
        return Err(Error::InvalidInput(
            "principal_binding.principal is required and must not be padded".to_string(),
        ));
    }
    if params.agent.is_empty() {
        return Err(Error::InvalidInput("principal_binding.agent is required".to_string()));
    }
    let verbs: Vec<String> = params
        .verbs
        .clone()
        .unwrap_or_else(|| DEFAULT_VERBS.iter().map(|v| (*v).to_string()).collect::<Vec<String>>());
    if verbs.is_empty() {
        return Err(Error::InvalidInput(
            "principal_binding.authority.verbs must be non-empty".to_string(),
        ));
    }
    let expiry = match params.expires_at.as_deref() {
        Some(raw) => format_rfc3339_millis(parse_rfc3339_millis(raw)?),
        None => format_rfc3339_millis(now_millis() + DEFAULT_BINDING_TTL_MS),
    };
    let authority = Authority {
        max_per_intent: params.max_per_intent.clone().unwrap_or_else(default_max_per_intent),
        verbs,
        max_per_payout: params.max_per_payout.clone(),
    };
    let revocation = params
        .revocation
        .clone()
        .unwrap_or_else(|| format!("https://example.com/icp-revocation/{}", params.agent));

    let body = BindingBody {
        principal: &params.principal,
        agent: &params.agent,
        authority: &authority,
        expiry: &expiry,
        revocation: &revocation,
    };
    let sig = principal_identity.sign_hex(canonical_json(&body)?.as_bytes());

    Ok(PrincipalBinding {
        principal: params.principal.clone(),
        agent: params.agent.clone(),
        authority,
        expiry,
        revocation,
        signature: Signature { alg: "ed25519".to_string(), kid: params.principal.clone(), sig },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTOR_SEED: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";

    #[test]
    fn debug_never_prints_the_seed() {
        let id = PrincipalIdentity::from_seed_hex(VECTOR_SEED).unwrap();
        let rendered = format!("{id:?}");
        assert!(!rendered.contains(VECTOR_SEED), "seed leaked: {rendered}");
        assert!(rendered.contains(&id.ed_pubkey_hex()));
    }

    #[test]
    fn seed_round_trips() {
        let id = PrincipalIdentity::generate();
        let restored = PrincipalIdentity::from_seed(&id.seed());
        assert_eq!(restored.ed_pubkey_hex(), id.ed_pubkey_hex());
    }

    #[test]
    fn bad_seed_hex_is_rejected() {
        assert!(PrincipalIdentity::from_seed_hex("zz").is_err());
        assert!(PrincipalIdentity::from_seed_hex("aabb").is_err());
    }

    #[test]
    fn max_per_payout_is_omitted_unless_asked_for() {
        let id = PrincipalIdentity::from_seed_hex(VECTOR_SEED).unwrap();
        let plain = PrincipalBindingParams::new("did:web:x", "aid:v1:zA").sign(&id).unwrap();
        let json = serde_json::to_value(&plain).unwrap();
        assert!(json["authority"].get("max_per_payout").is_none());

        let capped = PrincipalBindingParams::new("did:web:x", "aid:v1:zA")
            .max_per_payout(Money { amount: "5.00".to_string(), currency: "USDC".to_string() })
            .sign(&id)
            .unwrap();
        assert_eq!(capped.authority.max_per_payout.as_ref().unwrap().amount, "5.00");
        // …and the cap is covered by the signature, so widening it after the
        // fact cannot pass the handler.
        let widened = serde_json::to_value(&capped).unwrap();
        assert_eq!(widened["authority"]["max_per_payout"]["amount"], "5.00");
    }

    #[test]
    fn a_default_expiry_is_a_live_day_away() {
        let id = PrincipalIdentity::generate();
        let binding = PrincipalBindingParams::new("did:web:x", "aid:v1:zA").sign(&id).unwrap();
        let expiry = crate::time::parse_rfc3339_millis(&binding.expiry).unwrap();
        let delta = expiry - now_millis();
        assert!(
            delta > DEFAULT_BINDING_TTL_MS - 60_000 && delta <= DEFAULT_BINDING_TTL_MS,
            "expiry {delta}ms away"
        );
    }
}
