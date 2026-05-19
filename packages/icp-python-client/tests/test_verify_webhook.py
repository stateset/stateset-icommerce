"""Tests for `verify_webhook` — Python mirror of the JS SDK's `verifyWebhook`.

Validates the four ICPIP-0005 §6 checks in isolation: timestamp window,
HTTP-layer signature, body shape, envelope signature. End-to-end interop
against the live handler is already proven on the handler side
(`icp-handler/test/channel-publish.test.mjs`), so we don't duplicate
that here.
"""

from __future__ import annotations

import json
import time
import unittest

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
from cryptography.hazmat.primitives import serialization

from icp_client import ICPError, canonical_json, identity_from_seeds, sign_ed25519, verify_webhook


# Fixed merchant keypair so tests are deterministic. Reuse the same seed
# for both Ed25519 and X25519 — these tests only exercise Ed25519 signing.
_MERCHANT_SEED = bytes.fromhex(
    "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"
)
_merchant_identity = identity_from_seeds(_MERCHANT_SEED, _MERCHANT_SEED)
_merchant_pubkey_raw = _merchant_identity.ed25519_pubkey


def _sample_envelope():
    return {
        "v": "icp-1.0",
        "event_id": "icp_evt_test001",
        "event_type": "settlement.released",
        "channel_id": "icp_ch_test001",
        "sequence": 1,
        "originated_at": "2026-05-12T15:22:09.000Z",
        "source": "aid:v1:zMerchantTest",
        "target": "aid:v1:zAgentTest",
        "payload": {
            "settlement_id": "icp_set_abc",
            "escrow_id": "0xabc",
            "amount": {"amount": "29.99", "currency": "USDC"},
            "final_state": "released",
        },
        "previous_event_id": None,
        "delivery_attempt": 1,
    }


def _forge_post(
    envelope: dict,
    *,
    identity: Identity = _merchant_identity,
    method: str = "POST",
    path: str = "/icp/events",
    now_seconds: int | None = None,
) -> dict:
    envelope_canonical = canonical_json(envelope)
    envelope_sig = sign_ed25519(envelope_canonical, identity)
    body = json.dumps(
        {
            "envelope": envelope,
            "signature": {"alg": "ed25519", "kid": envelope["source"], "sig": envelope_sig},
        },
        separators=(",", ":"),
    )
    ts = str(int(time.time()) if now_seconds is None else now_seconds)
    material = f"{ts}.{method}.{path}.{body}"
    http_sig = sign_ed25519(material, identity)
    return {
        "body": body,
        "method": method,
        "path": path,
        "headers": {
            "content-type": "application/json",
            "x-icp-timestamp": ts,
            "x-icp-signature": f"ed25519={http_sig}",
            "x-icp-channel-id": envelope["channel_id"],
            "x-icp-event-id": envelope["event_id"],
            "x-icp-sequence": str(envelope["sequence"]),
        },
    }


class TestVerifyWebhook(unittest.TestCase):
    def test_happy_path_returns_parsed_envelope(self) -> None:
        forged = _forge_post(_sample_envelope())
        env = verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(env["event_id"], "icp_evt_test001")
        self.assertEqual(env["event_type"], "settlement.released")
        self.assertEqual(env["payload"]["final_state"], "released")

    def test_tampered_body_rejected(self) -> None:
        forged = _forge_post(_sample_envelope())
        forged["body"] = forged["body"].replace("29.99", "99.99")
        with self.assertRaises(ICPError) as cm:
            verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(cm.exception.code, "channel.signature_invalid")

    def test_flipped_envelope_signature_rejected(self) -> None:
        forged = _forge_post(_sample_envelope())
        parsed = json.loads(forged["body"])
        sig = parsed["signature"]["sig"]
        # Flip last hex char.
        parsed["signature"]["sig"] = sig[:-1] + ("0" if sig[-1] != "0" else "1")
        forged["body"] = json.dumps(parsed, separators=(",", ":"))
        # Re-sign the HTTP layer so the HTTP check passes; only envelope sig differs.
        ts = forged["headers"]["x-icp-timestamp"]
        material = f"{ts}.{forged['method']}.{forged['path']}.{forged['body']}"
        forged["headers"]["x-icp-signature"] = f"ed25519={sign_ed25519(material, _merchant_identity)}"
        with self.assertRaises(ICPError) as cm:
            verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(cm.exception.code, "channel.signature_invalid")

    def test_stale_timestamp_rejected_with_channel_replay(self) -> None:
        stale = int(time.time()) - 600
        forged = _forge_post(_sample_envelope(), now_seconds=stale)
        with self.assertRaises(ICPError) as cm:
            verify_webhook(
                **forged,
                merchant_pubkey_raw=_merchant_pubkey_raw,
                tolerance_seconds=300,
            )
        self.assertEqual(cm.exception.code, "channel.replay")

    def test_missing_timestamp_rejected(self) -> None:
        forged = _forge_post(_sample_envelope())
        forged["headers"].pop("x-icp-timestamp")
        with self.assertRaises(ICPError) as cm:
            verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(cm.exception.code, "channel.signature_invalid")

    def test_missing_signature_header_rejected(self) -> None:
        forged = _forge_post(_sample_envelope())
        forged["headers"].pop("x-icp-signature")
        with self.assertRaises(ICPError) as cm:
            verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(cm.exception.code, "channel.signature_invalid")

    def test_malformed_signature_header_rejected(self) -> None:
        forged = _forge_post(_sample_envelope())
        forged["headers"]["x-icp-signature"] = "hmac-sha256=deadbeef"  # algo not supported here
        with self.assertRaises(ICPError) as cm:
            verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(cm.exception.code, "channel.signature_invalid")

    def test_wrong_pubkey_rejected(self) -> None:
        forged = _forge_post(_sample_envelope())
        other_seed = bytes.fromhex(
            "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
        )
        other_pub = (
            Ed25519PrivateKey.from_private_bytes(other_seed)
            .public_key()
            .public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
        )
        with self.assertRaises(ICPError) as cm:
            verify_webhook(**forged, merchant_pubkey_raw=other_pub)
        self.assertEqual(cm.exception.code, "channel.signature_invalid")

    def test_case_insensitive_header_lookup(self) -> None:
        forged = _forge_post(_sample_envelope())
        # Upper-case the timestamp + signature headers. Lookup must still work.
        forged["headers"] = {
            "X-ICP-Timestamp": forged["headers"]["x-icp-timestamp"],
            "X-ICP-Signature": forged["headers"]["x-icp-signature"],
        }
        env = verify_webhook(**forged, merchant_pubkey_raw=_merchant_pubkey_raw)
        self.assertEqual(env["event_id"], "icp_evt_test001")


if __name__ == "__main__":
    unittest.main()
