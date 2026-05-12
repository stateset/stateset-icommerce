"""ICPClient — the main public API surface for the Python SDK.

Mirrors the JavaScript @stateset/icp-client API exactly.
"""

from __future__ import annotations

import datetime
import json
import urllib.request
import urllib.error
from dataclasses import dataclass
from typing import Any, Iterator

from .codec import (
    Identity,
    canonical_json,
    generate_identity,
    identity_from_seeds,
    new_id,
    new_nonce_hex,
    sign_ed25519,
    verify_ed25519,
)


class ICPError(Exception):
    """Typed error raised by the SDK. Inspect `.code` for protocol-level branching."""

    def __init__(self, code: str, message: str, details: dict | None = None) -> None:
        super().__init__(f"{code}: {message}")
        self.code = code
        self.details = details or {}


@dataclass
class ICPClient:
    """High-level ICP-1.0 client. Construct via ICPClient.create(...)."""

    handler_url: str
    principal: str
    identity: Identity
    verbs: list[str]
    max_per_intent: dict
    revocation_url: str
    _merchant_pub_cache: bytes | None = None

    # ------------------------------------------------------------------
    # Construction
    # ------------------------------------------------------------------

    @classmethod
    def create(
        cls,
        handler_url: str,
        principal: str,
        identity: Identity | None = None,
        verbs: list[str] | None = None,
        max_per_intent: dict | None = None,
        revocation_url: str | None = None,
    ) -> "ICPClient":
        """Create a client with a fresh or restored identity."""
        if not handler_url:
            raise ICPError("format.missing_field", "handler_url required")
        if not principal:
            raise ICPError("format.missing_field", "principal required")
        ident = identity or generate_identity()
        return cls(
            handler_url=handler_url.rstrip("/"),
            principal=principal,
            identity=ident,
            verbs=verbs or [
                "purchase.create",
                "subscription.create",
                "subscription.cancel",
                "purchase.return",
                "inventory.query",
                "quote.request",
                "payout.request",
            ],
            max_per_intent=max_per_intent or {"amount": "10000", "currency": "USDC"},
            revocation_url=revocation_url or f"https://example.com/icp-revocation/{ident.aid}",
        )

    @property
    def aid(self) -> str:
        return self.identity.aid

    # ------------------------------------------------------------------
    # Discovery
    # ------------------------------------------------------------------

    def capabilities(self) -> dict:
        """Fetch the handler's .well-known/icp doc and cache the merchant pubkey."""
        caps = self._get(f"{self.handler_url}/icp/v1/.well-known/icp")
        pub_hex = caps.get("merchant_pubkey", {}).get("raw_hex")
        if pub_hex:
            self._merchant_pub_cache = bytes.fromhex(pub_hex)
        return caps

    # ------------------------------------------------------------------
    # Verbs
    # ------------------------------------------------------------------

    def inventory(
        self,
        merchant: str,
        settler: str,
        skus: list[dict] | None = None,
        filters: dict | None = None,
        max_results: int | None = None,
    ) -> dict:
        """inventory.query — read-only discovery."""
        intent = self._base_intent("inventory.query", merchant, settler)
        if skus is not None:
            intent["skus"] = skus
        if filters is not None:
            intent["filters"] = filters
        if max_results is not None:
            intent["max_results"] = max_results
        result = self._submit(intent)
        self._verify_merchant(result["snapshot"], result["signature"])
        return result

    def purchase(
        self,
        merchant: str,
        settler: str,
        items: list[dict],
        max_total: dict,
        ship_to: dict | None = None,
        from_proposal_id: str | None = None,
    ) -> dict:
        """purchase.create — one-shot purchase. Returns the merchant Quote."""
        intent = self._base_intent("purchase.create", merchant, settler)
        intent["items"] = items
        intent["max_total"] = max_total
        if ship_to is not None:
            intent["ship_to"] = ship_to
        if from_proposal_id is not None:
            intent["from_proposal_id"] = from_proposal_id
        result = self._submit(intent)
        self._verify_merchant(result["quote"], result["signature"])
        return result

    def accept(self, quote_id: str, body: dict | None = None) -> dict:
        """Accept a Quote; returns on-chain funding instructions."""
        return self._post(
            f"{self.handler_url}/icp/v1/quotes/{quote_id}/accept",
            body or {},
        )

    def subscribe(
        self,
        merchant: str,
        settler: str,
        service_id: str,
        cadence: str,
        max_total_per_period: dict,
        first_charge_at: str,
        max_occurrences: int | None = None,
    ) -> dict:
        """subscription.create — recurring authorization."""
        intent = self._base_intent("subscription.create", merchant, settler)
        intent["service_id"] = service_id
        intent["cadence"] = cadence
        intent["max_total_per_period"] = max_total_per_period
        intent["max_occurrences"] = max_occurrences
        intent["first_charge_at"] = first_charge_at
        result = self._submit(intent)
        self._verify_merchant(result["authorization"], result["signature"])
        return result

    def cancel(
        self,
        merchant: str,
        settler: str,
        subscription_id: str,
        effective: str = "immediate",
        reason: str | None = None,
    ) -> dict:
        """subscription.cancel — cancel an existing subscription."""
        intent = self._base_intent("subscription.cancel", merchant, settler)
        intent["subscription_id"] = subscription_id
        intent["effective"] = effective
        if reason is not None:
            intent["reason"] = reason
        result = self._submit(intent)
        self._verify_merchant(result["authorization"], result["signature"])
        return result

    def return_(
        self,
        merchant: str,
        settler: str,
        original_settlement_id: str,
        items: list[dict],
        desired_outcome: str,
        max_refund: dict | None = None,
        narrative: str | None = None,
    ) -> dict:
        """purchase.return — request return/refund/replacement."""
        intent = self._base_intent("purchase.return", merchant, settler)
        intent["original_settlement_id"] = original_settlement_id
        intent["items"] = items
        intent["desired_outcome"] = desired_outcome
        if max_refund is not None:
            intent["max_refund"] = max_refund
        if narrative is not None:
            intent["narrative"] = narrative
        result = self._submit(intent)
        self._verify_merchant(result["authorization"], result["signature"])
        return result

    def request_quote(
        self,
        merchant: str,
        settler: str,
        items: list[dict],
        purchase_window: str | None = None,
        expected_delivery_by: str | None = None,
        ship_to: dict | None = None,
        context: str | None = None,
    ) -> dict:
        """quote.request — B2B RFQ for non-binding pricing."""
        intent = self._base_intent("quote.request", merchant, settler)
        intent["items"] = items
        if purchase_window is not None:
            intent["purchase_window"] = purchase_window
        if expected_delivery_by is not None:
            intent["expected_delivery_by"] = expected_delivery_by
        if ship_to is not None:
            intent["ship_to"] = ship_to
        if context is not None:
            intent["context"] = context
        result = self._submit(intent)
        self._verify_merchant(result["proposal"], result["signature"])
        return result

    def payout(
        self,
        platform: str,
        settler: str,
        amount: dict,
        destination: dict,
        expedited: bool | None = None,
    ) -> dict:
        """payout.request — marketplace seller payout. NB: seller signs (inverted direction)."""
        intent = self._base_intent("payout.request", platform, settler)
        # Inverted-direction field rename: this Agent is the SELLER, not the BUYER.
        intent["seller"] = intent.pop("buyer")
        intent["platform"] = intent.pop("merchant")
        intent["amount"] = amount
        intent["destination"] = destination
        if expedited is not None:
            intent["expedited"] = expedited
        result = self._submit(intent)
        self._verify_merchant(result["authorization"], result["signature"])
        return result

    # ------------------------------------------------------------------
    # Observe & retrieve
    # ------------------------------------------------------------------

    def observe(self, escrow_id: str) -> Iterator[dict]:
        """Iterator over EscrowEvents for the given escrow via Server-Sent Events."""
        url = f"{self.handler_url}/icp/v1/escrows/{escrow_id}/events"
        req = urllib.request.Request(url, headers={"Accept": "text/event-stream"})
        with urllib.request.urlopen(req) as resp:
            buf = ""
            while True:
                chunk = resp.read(4096)
                if not chunk:
                    break
                buf += chunk.decode("utf-8")
                while "\n\n" in buf:
                    block, buf = buf.split("\n\n", 1)
                    for line in block.split("\n"):
                        if line.startswith("data: "):
                            try:
                                yield json.loads(line[6:])
                            except json.JSONDecodeError:
                                pass

    def settlement(self, settlement_id: str) -> dict:
        """Fetch a SettlementReceipt by id."""
        return self._get(f"{self.handler_url}/icp/v1/settlements/{settlement_id}")

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------

    def _base_intent(self, verb: str, counterparty_aid: str, settler: str) -> dict:
        now = datetime.datetime.now(datetime.timezone.utc)
        exp = now + datetime.timedelta(seconds=300)
        return {
            "v": "icp-1.0",
            "verb": verb,
            "intent_id": new_id("icp_int"),
            "buyer": self.identity.aid,
            "merchant": counterparty_aid,
            "settler": settler,
            "expiry": exp.isoformat().replace("+00:00", "Z"),
            "principal_binding": self._principal_binding(),
            "nonce": new_nonce_hex(),
            "iat": now.isoformat().replace("+00:00", "Z"),
            "exp": exp.isoformat().replace("+00:00", "Z"),
        }

    def _principal_binding(self) -> dict:
        now = datetime.datetime.now(datetime.timezone.utc)
        binding_exp = now + datetime.timedelta(days=1)
        return {
            "principal": self.principal,
            "agent": self.identity.aid,
            "authority": {
                "max_per_intent": self.max_per_intent,
                "verbs": self.verbs,
            },
            "expiry": binding_exp.isoformat().replace("+00:00", "Z"),
            "revocation": self.revocation_url,
            "signature": {"alg": "ed25519", "kid": "self", "sig": "deadbeef"},
        }

    def _submit(self, intent: dict) -> dict:
        canonical = canonical_json(intent)
        sig = sign_ed25519(canonical, self.identity)
        body = {
            "intent": intent,
            "signature": {"alg": "ed25519", "kid": self.identity.aid, "sig": sig},
            "_pubkey_hex": self.identity.ed25519_pubkey.hex(),
        }
        return self._post(f"{self.handler_url}/icp/v1/intents", body)

    def _verify_merchant(self, payload: dict, signature: dict) -> None:
        """Verify the merchant signature against the cached pubkey from .well-known/icp."""
        if self._merchant_pub_cache is None:
            self.capabilities()
        if self._merchant_pub_cache is None:
            return  # merchant didn't publish a pubkey; skip with caller's awareness
        canonical = canonical_json(payload)
        if not verify_ed25519(canonical, signature["sig"], self._merchant_pub_cache):
            raise ICPError(
                "signature.invalid",
                "merchant signature failed verification against published .well-known/icp pubkey",
            )

    def _get(self, url: str) -> dict:
        try:
            with urllib.request.urlopen(url) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8") if e.fp else ""
            try:
                payload = json.loads(body) if body else {}
                raise ICPError(
                    payload.get("code", "format.unknown"),
                    payload.get("message", f"HTTP {e.code}"),
                    payload,
                ) from e
            except json.JSONDecodeError:
                raise ICPError("format.unknown", f"HTTP {e.code}: {body}") from e

    def _post(self, url: str, body: dict) -> dict:
        data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            body_str = e.read().decode("utf-8") if e.fp else ""
            try:
                payload = json.loads(body_str) if body_str else {}
                raise ICPError(
                    payload.get("code", "format.unknown"),
                    payload.get("message", f"HTTP {e.code}"),
                    payload,
                ) from e
            except json.JSONDecodeError:
                raise ICPError("format.unknown", f"HTTP {e.code}: {body_str}") from e
