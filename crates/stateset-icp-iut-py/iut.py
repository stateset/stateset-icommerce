#!/usr/bin/env python3
"""ICP-1.0 conformance IUT — Python reference.

Reads one JSON object from stdin, dispatches on the test name passed in
argv[1], writes one JSON object to stdout. Protocol: see
icp-conformance/iut-adapters/iut.protocol.md.

Uses the `cryptography` library (industry standard, widely available) for
Ed25519 + X25519. Everything else (canonical JSON, Base58btc, AID derivation)
is Python stdlib.

This adapter doubles as a reference Python implementation of the ICP-1.0
canonical wire format that agent developers can vendor. The agent-developer
ecosystem (Anthropic SDK, OpenAI SDK, LangChain, LangGraph) is Python-first;
having a working Python implementation that passes the same conformance
vectors as Rust and Go means an agent developer's Intent will produce
byte-identical bytes regardless of where the merchant or settler runs.
"""

import sys
import json
import hashlib

try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import (
        Ed25519PrivateKey,
        Ed25519PublicKey,
    )
    from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
    from cryptography.exceptions import InvalidSignature
except ImportError as e:
    # Per iut.protocol.md: exit 2 + JSON on stderr signals SKIP.
    print(
        json.dumps(
            {"error": "unsupported", "reason": f"cryptography library not installed: {e}"}
        ),
        file=sys.stderr,
    )
    sys.exit(2)


BASE58BTC_ALPHABET = (
    "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
)


def base58btc_encode(data: bytes) -> str:
    """Bitcoin Base58btc with leading-zero preservation. Mirrors JS/Rust/Go IUTs."""
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


def canonical_json(value) -> str:
    """RFC-8785-compatible canonical JSON for ICP-1.0 payload shapes.

    Same rule as JS IUT: lexicographic key ordering, no whitespace, standard
    JSON escapes. Python's json.dumps with sort_keys=True + separators is
    sufficient for the value shapes ICP-1.0 uses (objects, arrays, strings,
    integers, decimals, booleans, null — no floats in monetary fields).
    """
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )


# ---------------------------------------------------------------------------
# Test 01: AID derivation and Intent signing
# ---------------------------------------------------------------------------


def run_01_aid_derivation(inp):
    agent = inp.get("agent")
    if not isinstance(agent, dict):
        raise ValueError("missing 'agent' in input")
    ed_seed_hex = agent.get("ed25519_seed_hex")
    x_seed_hex = agent.get("x25519_seed_hex")
    if not isinstance(ed_seed_hex, str) or not isinstance(x_seed_hex, str):
        raise ValueError("agent.ed25519_seed_hex and agent.x25519_seed_hex required")
    ed_seed = bytes.fromhex(ed_seed_hex)
    x_seed = bytes.fromhex(x_seed_hex)
    if len(ed_seed) != 32:
        raise ValueError("ed25519_seed must be 32 bytes")
    if len(x_seed) != 32:
        raise ValueError("x25519_seed must be 32 bytes")

    # Keypairs
    ed_priv = Ed25519PrivateKey.from_private_bytes(ed_seed)
    ed_pub = ed_priv.public_key()
    ed_pub_raw = ed_pub.public_bytes(Encoding.Raw, PublicFormat.Raw)

    x_priv = X25519PrivateKey.from_private_bytes(x_seed)
    x_pub_raw = x_priv.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)

    # AID per ICP-1.0 §4.2
    aid_payload = ed_pub_raw + b"\x00" + x_pub_raw
    digest = hashlib.sha256(aid_payload).digest()
    aid = "aid:v1:z" + base58btc_encode(digest)

    # Build Intent: fill buyer + principal_binding.agent
    intent_in = inp.get("intent")
    if not isinstance(intent_in, dict):
        raise ValueError("missing 'intent' in input")
    # Deep-copy so we don't mutate the input.
    intent = json.loads(json.dumps(intent_in))
    intent["buyer"] = aid
    if isinstance(intent.get("principal_binding"), dict):
        intent["principal_binding"]["agent"] = aid

    # Canonicalize and sign
    canonical = canonical_json(intent)
    sig = ed_priv.sign(canonical.encode("utf-8"))

    out = {
        "ed25519_pubkey_hex": ed_pub_raw.hex(),
        "x25519_pubkey_hex": x_pub_raw.hex(),
        "aid": aid,
        "intent_canonical_string": canonical,
        "intent_canonical_bytes_hex": canonical.encode("utf-8").hex(),
        "intent_signature_hex": sig.hex(),
    }

    # Optional negative-case: tampered payload MUST fail verification
    params = inp.get("params") or {}
    if params.get("verify_tamper_rejected"):
        tampered = canonical.replace("29.99", "0.01", 1)
        try:
            ed_pub.verify(sig, tampered.encode("utf-8"))
            out["tamper_rejected"] = False  # bug — should reject
        except InvalidSignature:
            out["tamper_rejected"] = True

    return out


# ---------------------------------------------------------------------------
# Test 02: Canonical JSON
# ---------------------------------------------------------------------------


def run_02_canonical_json(inp):
    cases = inp.get("cases")
    if not isinstance(cases, list):
        raise ValueError("input.cases must be an array")
    canonical_strings = []
    names = []
    for i, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ValueError(f"case {i} not an object")
        names.append(case.get("name", ""))
        canonical_strings.append(canonical_json(case.get("value")))
    return {"canonical_strings": canonical_strings, "names": names}


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    if len(sys.argv) < 2:
        print("FATAL: missing test name argument", file=sys.stderr)
        sys.exit(2)
    test_name = sys.argv[1]

    try:
        inp = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(f"FATAL: invalid JSON on stdin: {e}", file=sys.stderr)
        sys.exit(2)

    try:
        if test_name == "01-aid-derivation":
            output = run_01_aid_derivation(inp)
        elif test_name == "02-canonical-json":
            output = run_02_canonical_json(inp)
        else:
            print(
                json.dumps(
                    {"error": "unsupported", "reason": f"no handler for {test_name}"}
                ),
                file=sys.stderr,
            )
            sys.exit(2)
    except Exception as e:
        print(f"FATAL: adapter error: {e}", file=sys.stderr)
        sys.exit(1)

    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
