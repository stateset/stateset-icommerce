"""ICPIP-0005 receiver helpers — `verify_webhook` is the Stripe-style
one-call validator for inbound webhook events.

Hand it the raw HTTP body string, the request headers (case-insensitive),
the HTTP method/path, and the merchant's raw 32-byte Ed25519 pubkey from
`.well-known/icp`. Get back the parsed `EventEnvelope` dict, OR an
`ICPError` with a `channel.*` code that maps directly to HTTP status.

Performs every check ICPIP-0005 §6 mandates:
  1. HTTP timestamp within ±tolerance (default 300s) → `channel.replay` on miss.
  2. HTTP-layer `X-ICP-Signature: ed25519=<hex>` verifies against
     `<timestamp>.<method>.<path>.<body>`.
  3. Body parses as `{envelope, signature}`.
  4. Envelope signature verifies against the merchant pubkey over the
     envelope's canonical JSON bytes.

This mirrors the JavaScript SDK's `verifyWebhook` function byte-for-byte.
"""

from __future__ import annotations

import json
import re
import time
from typing import Any, Mapping

from .client import ICPError
from .codec import canonical_json, verify_ed25519


_HTTP_SIG_PATTERN = re.compile(r"^ed25519=([0-9a-fA-F]+)$")


def _normalized_header_lookup(headers: Mapping[str, str] | None, name: str) -> str | None:
    """Look up `name` (case-insensitive) in a header mapping.

    Accepts dict, multidict, requests.structures.CaseInsensitiveDict, or
    any object that exposes either case-insensitive `__getitem__` or
    standard `dict.get` semantics.
    """
    if not headers:
        return None
    # 1. Try exact key.
    if hasattr(headers, "get"):
        v = headers.get(name)
        if v is not None:
            return v
        v = headers.get(name.lower())
        if v is not None:
            return v
        v = headers.get(name.upper())
        if v is not None:
            return v
    # 2. Fall back to scan for case-insensitive match.
    target = name.lower()
    try:
        items = headers.items()  # type: ignore[union-attr]
    except AttributeError:
        return None
    for k, v in items:
        if k.lower() == target:
            return v
    return None


def verify_webhook(
    *,
    body: str,
    headers: Mapping[str, str],
    method: str,
    path: str,
    merchant_pubkey_raw: bytes,
    tolerance_seconds: int = 300,
    now_seconds: int | None = None,
) -> dict[str, Any]:
    """Verify an inbound webhook and return its parsed `EventEnvelope`.

    Raises `ICPError` (a typed exception with a `channel.*` code) on any
    verification failure.

    :param body: Raw HTTP body string. Do NOT pre-parse — JSON re-encoding
                 would break the HTTP-layer signature.
    :param headers: HTTP request headers (case-insensitive lookup).
    :param method: HTTP method (e.g. "POST").
    :param path: HTTP path (including query string if the original request
                 had one).
    :param merchant_pubkey_raw: Raw 32-byte Ed25519 pubkey from the
                                merchant's `.well-known/icp` discovery.
    :param tolerance_seconds: Replay window (default 300s).
    :param now_seconds: Override "now" for testing.
    :returns: The verified envelope dict.
    """
    if now_seconds is None:
        now_seconds = int(time.time())

    # 1. Timestamp window.
    ts_header = _normalized_header_lookup(headers, "x-icp-timestamp")
    if ts_header is None:
        raise ICPError("channel.signature_invalid", "missing X-ICP-Timestamp header")
    try:
        ts = int(ts_header)
    except (TypeError, ValueError) as e:
        raise ICPError(
            "channel.signature_invalid",
            f"invalid X-ICP-Timestamp: {ts_header}",
        ) from e
    if abs(now_seconds - ts) > tolerance_seconds:
        raise ICPError(
            "channel.replay",
            f"timestamp {ts} outside ±{tolerance_seconds}s of {now_seconds}",
        )

    # 2. HTTP-layer signature.
    sig_header = _normalized_header_lookup(headers, "x-icp-signature")
    if sig_header is None:
        raise ICPError("channel.signature_invalid", "missing X-ICP-Signature header")
    m = _HTTP_SIG_PATTERN.match(sig_header)
    if not m:
        raise ICPError(
            "channel.signature_invalid",
            "X-ICP-Signature must be ed25519=<hex>",
        )
    http_sig_hex = m.group(1)
    http_material = f"{ts_header}.{method}.{path}.{body}"
    if not verify_ed25519(http_material, http_sig_hex, merchant_pubkey_raw):
        raise ICPError(
            "channel.signature_invalid",
            "HTTP-layer signature verification failed",
        )

    # 3. Body shape.
    try:
        parsed = json.loads(body)
    except (json.JSONDecodeError, TypeError) as e:
        raise ICPError(
            "channel.signature_invalid",
            f"body is not JSON: {e}",
        ) from e
    envelope = parsed.get("envelope") if isinstance(parsed, dict) else None
    signature = parsed.get("signature") if isinstance(parsed, dict) else None
    if not isinstance(envelope, dict) or not isinstance(signature, dict):
        raise ICPError(
            "channel.signature_invalid",
            "body missing {envelope, signature} object pair",
        )
    sig = signature.get("sig")
    if not isinstance(sig, str):
        raise ICPError(
            "channel.signature_invalid",
            "body.signature.sig must be a hex string",
        )

    # 4. Envelope signature over canonical bytes.
    envelope_canonical = canonical_json(envelope)
    if not verify_ed25519(envelope_canonical, sig, merchant_pubkey_raw):
        raise ICPError(
            "channel.signature_invalid",
            "envelope signature verification failed",
        )

    return envelope
