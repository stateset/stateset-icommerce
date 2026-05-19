"""SettlementReceipt verification.

The ``SettlementReceipt`` is the single most load-bearing artifact in
ICP-1.0: it proves payment to the merchant, to any downstream auditor,
and to KYC/AML pipelines. It's co-signed by BOTH the merchant AND the
Settler over the canonical bytes of the receipt body minus the two
signature fields themselves.

Partners MUST verify both signatures before treating settlement as
final. :func:`verify_settlement_receipt` is the Stripe-style one-call
helper that does this correctly.
"""

from __future__ import annotations

from typing import Any

from .client import ICPError
from .codec import canonical_json, verify_ed25519


def verify_settlement_receipt(
    *,
    receipt: dict[str, Any],
    merchant_pubkey_raw: bytes,
    settler_pubkey_raw: bytes,
    require_settler: bool = True,
) -> dict[str, Any]:
    """Verify a co-signed ``SettlementReceipt`` and return it on success.

    The receipt's ``merchant_signature`` and ``settler_signature`` fields
    are stripped, the remainder is canonicalized via RFC 8785 JCS, and
    each signature is verified against the corresponding raw 32-byte
    Ed25519 public key.

    Raises :class:`ICPError`:

      * ``format.missing_field`` — receipt missing
        ``merchant_signature`` or ``settler_signature`` (the latter only
        when ``require_settler=True``).
      * ``signature.invalid`` — the merchant signature failed.
      * ``settlement.settler_signature_invalid`` — the settler signature
        failed.

    :param receipt: The receipt dict (as returned by the handler's
                    fulfill or settlement endpoints).
    :param merchant_pubkey_raw: Raw 32-byte Ed25519 pubkey from the
                                merchant's ``.well-known/icp``.
    :param settler_pubkey_raw: Raw 32-byte Ed25519 pubkey from the
                               Settler's ``.well-known/icp`` (or
                               wherever the Settler publishes its
                               verifying key).
    :param require_settler: When ``False``, skip the settler-signature
                            check entirely. Default ``True``.
    :returns: The receipt dict unchanged.
    """
    if not isinstance(receipt, dict):
        raise ICPError("format.missing_field", "receipt must be a dict")

    merchant_sig = receipt.get("merchant_signature")
    if not isinstance(merchant_sig, dict) or not merchant_sig.get("sig"):
        raise ICPError(
            "format.missing_field",
            "receipt.merchant_signature.sig required",
        )

    settler_sig = receipt.get("settler_signature")
    if require_settler:
        if not isinstance(settler_sig, dict) or not settler_sig.get("sig"):
            raise ICPError(
                "format.missing_field",
                "receipt.settler_signature.sig required",
            )

    # Strip BOTH signature fields and re-canonicalize. The signing path:
    #   canonical = canonical_json(receipt without signatures)
    #   merchant_signature = sign(canonical)
    #   settler_signature  = sign(canonical)   # same canonical bytes
    unsigned = {
        k: v
        for k, v in receipt.items()
        if k not in ("merchant_signature", "settler_signature")
    }
    canonical = canonical_json(unsigned)

    if not verify_ed25519(canonical, merchant_sig["sig"], merchant_pubkey_raw):
        kid = merchant_sig.get("kid", "<unknown>")
        raise ICPError(
            "signature.invalid",
            f"merchant signature verification failed (kid={kid})",
        )

    if require_settler:
        if not verify_ed25519(canonical, settler_sig["sig"], settler_pubkey_raw):
            kid = settler_sig.get("kid", "<unknown>")
            raise ICPError(
                "settlement.settler_signature_invalid",
                f"settler signature verification failed (kid={kid})",
            )

    return receipt
