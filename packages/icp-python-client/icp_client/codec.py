"""ICP-1.0 wire codec: canonical JSON, Ed25519 signing, AID derivation."""

import hashlib
import json
import secrets
from dataclasses import dataclass
from typing import Any

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


def sign_ed25519(canonical: str, identity: Identity) -> str:
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
