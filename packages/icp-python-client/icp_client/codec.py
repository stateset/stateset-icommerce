"""ICP-1.0 wire codec: canonical JSON, Ed25519 signing, AID derivation."""

from __future__ import annotations

import datetime
import hashlib
import json
import secrets
from dataclasses import dataclass
from typing import Any, Optional, Sequence, Union

try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import (
        Ed25519PrivateKey,
        Ed25519PublicKey,
    )
    from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
    from cryptography.exceptions import InvalidSignature
except ImportError as exc:
    raise ImportError(
        "icp_client requires the `cryptography` package. Install with: pip install cryptography"
    ) from exc

BASE58BTC_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


@dataclass
class Identity:
    """An ICP Agent identity: Ed25519 + X25519 keypairs + derived AID."""
    ed25519_seed: bytes      # 32 bytes
    x25519_seed: bytes       # 32 bytes
    ed25519_pubkey: bytes    # 32 bytes
    x25519_pubkey: bytes     # 32 bytes
    aid: str                 # aid:v1:z<base58btc>


def generate_identity() -> Identity:
    """Generate a fresh Agent identity. For production, persist + reuse the seeds."""
    ed_priv = Ed25519PrivateKey.generate()
    x_priv = X25519PrivateKey.generate()
    ed_seed = ed_priv.private_bytes_raw()
    x_seed = x_priv.private_bytes_raw()
    return identity_from_seeds(ed_seed, x_seed)


def identity_from_seeds(ed_seed: bytes, x_seed: bytes) -> Identity:
    """Reconstruct an Agent identity from 32-byte seeds."""
    if len(ed_seed) != 32:
        raise ValueError("ed25519_seed must be 32 bytes")
    if len(x_seed) != 32:
        raise ValueError("x25519_seed must be 32 bytes")

    ed_priv = Ed25519PrivateKey.from_private_bytes(ed_seed)
    x_priv = X25519PrivateKey.from_private_bytes(x_seed)
    ed_pub = ed_priv.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    x_pub = x_priv.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    aid_digest = hashlib.sha256(ed_pub + b"\x00" + x_pub).digest()
    aid = "aid:v1:z" + base58btc_encode(aid_digest)
    return Identity(
        ed25519_seed=ed_seed,
        x25519_seed=x_seed,
        ed25519_pubkey=ed_pub,
        x25519_pubkey=x_pub,
        aid=aid,
    )


def base58btc_encode(data: bytes) -> str:
    """Bitcoin Base58btc with leading-zero preservation. Matches JS/Rust/Go/JS IUTs."""
    if not data:
        return ""
    n = int.from_bytes(data, "big")
    out = ""
    while n > 0:
        n, r = divmod(n, 58)
        out = BASE58BTC_ALPHABET[r] + out
    leading_ones = ""
    for b in data:
        if b == 0:
            leading_ones += "1"
        else:
            break
    return leading_ones + out


def canonical_json(value: Any) -> str:
    """RFC-8785-compatible canonical JSON.

    Lexicographic key ordering, no whitespace, standard JSON escapes.
    Produces byte-identical output to the JavaScript SDK and the
    conformance suite's Python IUT (stateset-python).
    """
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sign_ed25519(canonical: str, identity: "Union[Identity, PrincipalIdentity]") -> str:
    """Sign canonical bytes with the identity's Ed25519 key. Returns hex."""
    priv = Ed25519PrivateKey.from_private_bytes(identity.ed25519_seed)
    sig = priv.sign(canonical.encode("utf-8"))
    return sig.hex()


def verify_ed25519(canonical: str, signature_hex: str, ed_pubkey_raw: bytes) -> bool:
    """Verify an Ed25519 signature against a raw 32-byte public key."""
    try:
        sig = bytes.fromhex(signature_hex)
        if len(sig) != 64:
            return False
        pub = Ed25519PublicKey.from_public_bytes(ed_pubkey_raw)
        pub.verify(sig, canonical.encode("utf-8"))
        return True
    except (InvalidSignature, ValueError):
        return False


# ---------------------------------------------------------------------------
# PrincipalBinding (§4.4) — the delegation an Agent carries on every Intent
# ---------------------------------------------------------------------------

#: Verbs an Agent is delegated by default.
DEFAULT_VERBS = [
    "purchase.create",
    "subscription.create",
    "subscription.cancel",
    "purchase.return",
    "inventory.query",
    "quote.request",
    "payout.request",
]
#: Default per-Intent spend ceiling carried in the PrincipalBinding.
DEFAULT_MAX_PER_INTENT = {"amount": "10000", "currency": "USDC"}
#: Default PrincipalBinding lifetime: 24h.
DEFAULT_BINDING_TTL = datetime.timedelta(days=1)


@dataclass
class PrincipalIdentity:
    """A principal's Ed25519 signing key.

    A principal is an organization (``did:web:…``), not an Agent: no AID and
    no X25519 half. The public key is what an operator registers with the
    handler (``ICP_PRINCIPAL_KEYS_JSON``); the seed belongs in a KMS.
    """

    ed25519_seed: bytes   # 32 bytes
    ed25519_pubkey: bytes  # 32 bytes


def generate_principal_identity() -> PrincipalIdentity:
    """Generate a fresh principal signing key. Persist ``ed25519_seed``."""
    return principal_identity_from_seed(Ed25519PrivateKey.generate().private_bytes_raw())


def principal_identity_from_seed(ed_seed: bytes) -> PrincipalIdentity:
    """Restore a principal signing key from its 32-byte Ed25519 seed."""
    if len(ed_seed) != 32:
        raise ValueError("principal ed25519_seed must be 32 bytes")
    priv = Ed25519PrivateKey.from_private_bytes(ed_seed)
    return PrincipalIdentity(
        ed25519_seed=ed_seed,
        ed25519_pubkey=priv.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw),
    )


def _binding_expiry(expires_at: Union[datetime.datetime, str, None]) -> str:
    if expires_at is None:
        expires_at = datetime.datetime.now(datetime.timezone.utc) + DEFAULT_BINDING_TTL
    if isinstance(expires_at, str):
        # Validate rather than trust: an unparseable expiry is a binding no
        # handler can accept, and the failure would surface as a signature
        # error hours later.
        try:
            datetime.datetime.fromisoformat(expires_at.replace("Z", "+00:00"))
        except ValueError:
            raise ValueError("principal_binding expires_at is not RFC 3339") from None
        return expires_at
    if not isinstance(expires_at, datetime.datetime):
        raise ValueError("principal_binding expires_at must be a datetime or RFC 3339 string")
    if expires_at.tzinfo is None:
        expires_at = expires_at.replace(tzinfo=datetime.timezone.utc)
    return (
        expires_at.astimezone(datetime.timezone.utc)
        .isoformat(timespec="milliseconds")
        .replace("+00:00", "Z")
    )


def sign_principal_binding(
    principal: str,
    agent: str,
    principal_identity: PrincipalIdentity,
    expires_at: Union[datetime.datetime, str, None] = None,
    verbs: Optional[Sequence[str]] = None,
    max_per_intent: Optional[dict] = None,
    revocation: Optional[str] = None,
    **extra: Any,
) -> dict:
    """Sign a PrincipalBinding: the principal's statement that this Agent may
    act for it, over these verbs, up to this ceiling, until this expiry.

    This is the artifact a handler checks before it will quote anything. The
    signing input is ``canonical_json(binding)`` with the ``signature`` field
    removed — the rule the reference handler's ``checkDelegation`` applies — so
    every other field is covered and mutating one after signing invalidates it.

    Byte-identical to the JavaScript SDK's ``signPrincipalBinding``.
    """
    if not isinstance(principal, str) or not principal or principal.strip() != principal:
        raise ValueError("principal_binding.principal is required")
    if not isinstance(agent, str) or not agent:
        raise ValueError("principal_binding.agent is required")
    authorized_verbs = list(DEFAULT_VERBS if verbs is None else verbs)
    if not authorized_verbs:
        raise ValueError("principal_binding.authority.verbs must be non-empty")
    if not isinstance(principal_identity, PrincipalIdentity) and not hasattr(
        principal_identity, "ed25519_seed"
    ):
        raise ValueError("principal_identity must hold a 32-byte ed25519_seed")
    body = {
        **extra,
        "principal": principal,
        "agent": agent,
        "authority": {
            "max_per_intent": dict(max_per_intent or DEFAULT_MAX_PER_INTENT),
            "verbs": authorized_verbs,
        },
        "expiry": _binding_expiry(expires_at),
        "revocation": revocation or f"https://example.com/icp-revocation/{agent}",
    }
    return {
        **body,
        "signature": {
            "alg": "ed25519",
            "kid": principal,
            "sig": sign_ed25519(canonical_json(body), principal_identity),
        },
    }


def new_id(prefix: str) -> str:
    """ULID-shaped 26-char Crockford-base32 identifier."""
    alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
    bits = int.from_bytes(secrets.token_bytes(16), "big")
    chars = []
    for _ in range(26):
        chars.append(alphabet[bits & 31])
        bits >>= 5
    return f"{prefix}_{''.join(reversed(chars))}"


def new_nonce_hex() -> str:
    """16 random bytes hex-encoded for the Intent nonce field."""
    return secrets.token_bytes(16).hex()
