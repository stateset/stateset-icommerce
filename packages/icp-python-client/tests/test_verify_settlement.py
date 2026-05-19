"""Tests for ``verify_settlement_receipt`` — mirror of the JS suite.

The receipt is signed by BOTH the merchant AND the Settler over the
canonical bytes of the receipt body minus the two signature fields.
Both signatures must verify for the receipt to be considered final.
"""

from __future__ import annotations

import unittest

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

from icp_client import (
    ICPError,
    canonical_json,
    identity_from_seeds,
    sign_ed25519,
    verify_settlement_receipt,
)


def _pubkey_raw(seed: bytes) -> bytes:
    """Derive raw 32-byte Ed25519 public key from a 32-byte seed."""
    priv = Ed25519PrivateKey.from_private_bytes(seed)
    return priv.public_key().public_bytes(
        serialization.Encoding.Raw,
        serialization.PublicFormat.Raw,
    )


_MERCHANT_SEED = bytes.fromhex("aa" * 32)
_SETTLER_SEED = bytes.fromhex("bb" * 32)
_MERCHANT_PUB = _pubkey_raw(_MERCHANT_SEED)
_SETTLER_PUB = _pubkey_raw(_SETTLER_SEED)
# `sign_ed25519` consumes an Identity; we just need ed25519_seed.
_MERCHANT_ID = identity_from_seeds(_MERCHANT_SEED, _MERCHANT_SEED)  # x-seed unused
_SETTLER_ID = identity_from_seeds(_SETTLER_SEED, _SETTLER_SEED)


def _build_signed_receipt(**overrides: object) -> dict:
    unsigned = {
        "type": "icp.settlement.receipt",
        "v": "icp-1.0",
        "settlement_id": "icp_set_TEST",
        "escrow_id": "0xabcdef",
        "intent_id": "icp_int_TEST",
        "final_state": "released",
        "amount": {"amount": "29.99", "currency": "USDC"},
        "rail": "demo-mock",
        "rail_txid": "0xcafe",
        "settled_at": "2026-05-12T18:00:00.000Z",
        "released_to": "0xMerchantPayout",
        **overrides,
    }
    canonical = canonical_json(unsigned)
    merchant_sig = sign_ed25519(canonical, _MERCHANT_ID)
    settler_sig = sign_ed25519(canonical, _SETTLER_ID)
    return {
        **unsigned,
        "merchant_signature": {"alg": "ed25519", "kid": "aid:v1:zMerchant", "sig": merchant_sig},
        "settler_signature": {"alg": "ed25519", "kid": "aid:v1:zSettler", "sig": settler_sig},
    }


class TestVerifySettlementReceipt(unittest.TestCase):
    def test_happy_path_returns_receipt_unchanged(self) -> None:
        receipt = _build_signed_receipt()
        out = verify_settlement_receipt(
            receipt=receipt,
            merchant_pubkey_raw=_MERCHANT_PUB,
            settler_pubkey_raw=_SETTLER_PUB,
        )
        self.assertIs(out, receipt)
        self.assertEqual(out["final_state"], "released")

    def test_tampered_amount_raises_merchant_signature_invalid(self) -> None:
        receipt = _build_signed_receipt()
        receipt["amount"] = {"amount": "999.99", "currency": "USDC"}  # mutate post-sign
        with self.assertRaises(ICPError) as ctx:
            verify_settlement_receipt(
                receipt=receipt,
                merchant_pubkey_raw=_MERCHANT_PUB,
                settler_pubkey_raw=_SETTLER_PUB,
            )
        self.assertEqual(ctx.exception.code, "signature.invalid")

    def test_wrong_settler_pubkey_raises_typed_code(self) -> None:
        receipt = _build_signed_receipt()
        other_pub = _pubkey_raw(bytes.fromhex("cc" * 32))
        with self.assertRaises(ICPError) as ctx:
            verify_settlement_receipt(
                receipt=receipt,
                merchant_pubkey_raw=_MERCHANT_PUB,
                settler_pubkey_raw=other_pub,
            )
        self.assertEqual(ctx.exception.code, "settlement.settler_signature_invalid")

    def test_missing_merchant_signature_raises_format_error(self) -> None:
        receipt = _build_signed_receipt()
        del receipt["merchant_signature"]
        with self.assertRaises(ICPError) as ctx:
            verify_settlement_receipt(
                receipt=receipt,
                merchant_pubkey_raw=_MERCHANT_PUB,
                settler_pubkey_raw=_SETTLER_PUB,
            )
        self.assertEqual(ctx.exception.code, "format.missing_field")

    def test_missing_settler_signature_raises_format_error(self) -> None:
        receipt = _build_signed_receipt()
        del receipt["settler_signature"]
        with self.assertRaises(ICPError) as ctx:
            verify_settlement_receipt(
                receipt=receipt,
                merchant_pubkey_raw=_MERCHANT_PUB,
                settler_pubkey_raw=_SETTLER_PUB,
            )
        self.assertEqual(ctx.exception.code, "format.missing_field")

    def test_require_settler_false_skips_settler_check(self) -> None:
        receipt = _build_signed_receipt()
        del receipt["settler_signature"]
        # Should NOT raise; settler pubkey is unused on this path.
        out = verify_settlement_receipt(
            receipt=receipt,
            merchant_pubkey_raw=_MERCHANT_PUB,
            settler_pubkey_raw=b"\x00" * 32,
            require_settler=False,
        )
        self.assertIs(out, receipt)

    def test_both_signatures_cover_same_canonical_bytes(self) -> None:
        receipt = _build_signed_receipt()
        unsigned = {
            k: v
            for k, v in receipt.items()
            if k not in ("merchant_signature", "settler_signature")
        }
        canonical = canonical_json(unsigned)
        expected_merchant = sign_ed25519(canonical, _MERCHANT_ID)
        expected_settler = sign_ed25519(canonical, _SETTLER_ID)
        self.assertEqual(receipt["merchant_signature"]["sig"], expected_merchant)
        self.assertEqual(receipt["settler_signature"]["sig"], expected_settler)


if __name__ == "__main__":
    unittest.main()
