"""End-to-end test of the Python SDK against a spawned icp-handler.

Runs with pytest. Spawns the Node-based icp-handler as a subprocess,
waits for the listening port, then drives the SDK through every public
method.
"""

import json
import os
import re
import subprocess
import sys
import time
import unittest
from pathlib import Path

# Make the package importable without install
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from icp_client import (  # noqa: E402
    ICPClient,
    ICPError,
    canonical_json,
    generate_identity,
    sign_ed25519,
)


HANDLER_SCRIPT = (
    Path(__file__).resolve().parent.parent.parent.parent
    / "icp-handler"
    / "src"
    / "server.mjs"
)


class HandlerProc:
    """Spawn the icp-handler and capture its listening port."""

    def __init__(self) -> None:
        self.proc = subprocess.Popen(
            ["node", str(HANDLER_SCRIPT)],
            env={**os.environ, "PORT": "0"},
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.base_url = self._wait_for_listen()

    def _wait_for_listen(self) -> str:
        deadline = time.time() + 5.0
        buf = ""
        while time.time() < deadline:
            chunk = os.read(self.proc.stderr.fileno(), 4096)
            if not chunk:
                continue
            buf += chunk.decode("utf-8")
            m = re.search(r"listening on (http://127\.0\.0\.1:\d+)", buf)
            if m:
                return m.group(1)
        raise RuntimeError(f"handler did not start in 5s. stderr:\n{buf}")

    def close(self) -> None:
        self.proc.terminate()
        try:
            self.proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.proc.kill()


class TestICPClient(unittest.TestCase):
    handler: HandlerProc
    client: ICPClient

    @classmethod
    def setUpClass(cls) -> None:
        cls.handler = HandlerProc()
        cls.client = ICPClient.create(
            handler_url=cls.handler.base_url,
            principal="did:web:py-sdk-test.example",
        )

    @classmethod
    def tearDownClass(cls) -> None:
        cls.handler.close()

    def test_identity_aid_is_valid(self) -> None:
        self.assertTrue(self.client.aid.startswith("aid:v1:z"))
        self.assertEqual(len(self.client.identity.ed25519_pubkey), 32)
        self.assertEqual(len(self.client.identity.x25519_pubkey), 32)

    def test_capabilities_advertises_all_commerce_verbs(self) -> None:
        caps = self.client.capabilities()
        self.assertEqual(caps["spec"], "icp-1.0")
        verbs = caps["capabilities"]["verbs"]
        # All 7 commerce verbs are required; operational verbs like
        # `channel.register` (ICPIP-0005) may be advertised alongside.
        for expected in [
            "purchase.create",
            "subscription.create",
            "subscription.cancel",
            "purchase.return",
            "inventory.query",
            "quote.request",
            "payout.request",
        ]:
            self.assertIn(expected, verbs)

    def test_inventory_returns_signed_snapshot(self) -> None:
        caps = self.client.capabilities()
        result = self.client.inventory(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
        )
        self.assertEqual(result["snapshot"]["type"], "inventory.snapshot")
        self.assertGreater(len(result["snapshot"]["items"]), 0)

    def test_purchase_returns_signed_quote(self) -> None:
        caps = self.client.capabilities()
        result = self.client.purchase(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            items=[
                {
                    "sku": "WIDGET-001",
                    "quantity": 1,
                    "unit_price": {"amount": "29.99", "currency": "USDC"},
                }
            ],
            max_total={"amount": "35.00", "currency": "USDC"},
        )
        self.assertEqual(result["quote"]["v"], "icp-1.0")
        # 29.99 × 1.05 = 31.49
        self.assertEqual(result["quote"]["total"]["amount"], "31.49")

    def test_subscribe_returns_signed_authorization(self) -> None:
        caps = self.client.capabilities()
        import datetime
        first_charge = (
            datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=1)
        ).isoformat().replace("+00:00", "Z")
        result = self.client.subscribe(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            service_id="premium-monthly",
            cadence="30d",
            max_total_per_period={"amount": "29.99", "currency": "USDC"},
            max_occurrences=12,
            first_charge_at=first_charge,
        )
        self.assertEqual(result["authorization"]["type"], "subscription.authorization")
        self.assertEqual(result["authorization"]["cadence"], "30d")

    def test_cancel_returns_signed_cancellation(self) -> None:
        caps = self.client.capabilities()
        result = self.client.cancel(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            subscription_id="icp_sub_01HXYZPYTHONTEST0000000001",
            effective="immediate",
        )
        self.assertEqual(result["authorization"]["type"], "subscription.cancellation")

    def test_return_returns_signed_authorization(self) -> None:
        caps = self.client.capabilities()
        result = self.client.return_(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            original_settlement_id="icp_set_01HXYZPYTHONTEST0000000001",
            items=[{"sku": "WIDGET-001", "quantity": 1, "reason": "defective"}],
            desired_outcome="refund",
            max_refund={"amount": "30.00", "currency": "USDC"},
        )
        self.assertEqual(result["authorization"]["type"], "return.authorization")

    def test_request_quote_returns_signed_proposal(self) -> None:
        caps = self.client.capabilities()
        result = self.client.request_quote(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            items=[{"sku": "WIDGET-001", "quantity": 500}],
            purchase_window="30d",
        )
        self.assertEqual(result["proposal"]["type"], "price.proposal")
        # 500 × $29.99 × 0.80 (20% volume discount) = $11996.00
        self.assertEqual(result["proposal"]["total"]["amount"], "11996.00")

    def test_payout_returns_signed_authorization(self) -> None:
        caps = self.client.capabilities()
        result = self.client.payout(
            platform=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            amount={"amount": "1000.00", "currency": "USDC"},
            destination={
                "type": "wallet",
                "wallet_address": "0x1111111111111111111111111111111111111111",
            },
        )
        self.assertEqual(result["authorization"]["type"], "payout.authorization")
        # 3% commission + 1% reserve on $1000 = $40 fees; approved = $960
        self.assertEqual(result["authorization"]["approved_amount"]["amount"], "960.00")

    def test_disallowed_settler_raises_typed_icp_error(self) -> None:
        caps = self.client.capabilities()
        with self.assertRaises(ICPError) as ctx:
            self.client.purchase(
                merchant=caps["merchant_aid"],
                settler="settler:evil.fake.network",
                items=[{"sku": "X", "quantity": 1, "unit_price": {"amount": "1", "currency": "USDC"}}],
                max_total={"amount": "2", "currency": "USDC"},
            )
        self.assertEqual(ctx.exception.code, "policy.settler.not_allowed")

    def test_canonical_json_byte_identical_to_iut(self) -> None:
        # Same case as the JS SDK test for cross-language proof
        value = {"b": 2, "a": 1, "nested": {"y": [3, 1, 2], "x": None}}
        self.assertEqual(
            canonical_json(value),
            '{"a":1,"b":2,"nested":{"x":null,"y":[3,1,2]}}',
        )

    def test_sign_ed25519_produces_64_byte_hex(self) -> None:
        ident = generate_identity()
        sig = sign_ed25519('{"hello":"world"}', ident)
        self.assertEqual(len(bytes.fromhex(sig)), 64)

    def test_register_webhook_returns_signed_channel_registration(self) -> None:
        caps = self.client.capabilities()
        result = self.client.register_webhook(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            url="https://agent.example.com/icp/events",
            event_filters=["settlement.released", "escrow.refunded"],
        )
        self.assertEqual(result["channel"]["type"], "channel.registration")
        self.assertEqual(result["channel"]["channel_type"], "webhook")
        self.assertEqual(
            result["channel"]["webhook_url"], "https://agent.example.com/icp/events"
        )
        self.assertEqual(
            result["channel"]["events_registered"],
            ["settlement.released", "escrow.refunded"],
        )
        self.assertTrue(result["channel"]["channel_id"].startswith("icp_ch_"))
        self.assertEqual(result["signature"]["alg"], "ed25519")

    def test_register_webhook_sse_mints_subscription_token(self) -> None:
        caps = self.client.capabilities()
        result = self.client.register_webhook(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            type="sse",
            event_filters=["dispute.opened"],
        )
        self.assertEqual(result["channel"]["channel_type"], "sse")
        self.assertTrue(result["channel"]["subscription_token"])
        self.assertEqual(result["channel"]["token_ttl_seconds"], 3600)

    def test_register_webhook_http_nonloopback_url_rejected(self) -> None:
        caps = self.client.capabilities()
        with self.assertRaises(ICPError) as ctx:
            self.client.register_webhook(
                merchant=caps["merchant_aid"],
                settler="settler:stateset.usdc.base-sepolia",
                url="http://insecure.example.com/events",
                event_filters=["settlement.released"],
            )
        self.assertEqual(ctx.exception.code, "channel.url_unverified")

    def test_fetch_channel_events_returns_verified_envelopes(self) -> None:
        caps = self.client.capabilities()
        # Register a webhook subscribed to settlement.released, pointed at
        # an unreachable loopback URL so the live POST fails — the recovery
        # log still captures the signed envelope.
        reg = self.client.register_webhook(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            url="http://127.0.0.1:1/icp/events",  # deliberately unreachable
            event_filters=["settlement.released"],
        )
        channel_id = reg["channel"]["channel_id"]

        # Run a full purchase → accept → fulfill cycle to trigger the publish.
        purchase = self.client.purchase(
            merchant=caps["merchant_aid"],
            settler="settler:stateset.usdc.base-sepolia",
            items=[{"sku": "WIDGET-003", "quantity": 1, "unit_price": {"amount": "12.00", "currency": "USDC"}}],
            max_total={"amount": "15.00", "currency": "USDC"},
        )
        accepted = self.client.accept(purchase["quote"]["quote_id"])
        # Trigger fulfill via raw HTTP since the SDK doesn't expose it.
        import urllib.request
        req = urllib.request.Request(
            f"{self.handler.base_url}/icp/v1/escrows/{accepted['funding']['escrow_id']}/fulfill",
            data=json.dumps({"evidence_id": "icp_ful_PY_RECOV"}).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req) as _:
            pass

        # Fire-and-forget publish settle window.
        time.sleep(0.15)

        events = self.client.fetch_channel_events(channel_id, 0)
        self.assertGreaterEqual(len(events), 1, f"expected ≥1 event, got: {events}")
        evt = next((e for e in events if e["event_type"] == "settlement.released"), None)
        self.assertIsNotNone(evt, "must include settlement.released")
        self.assertEqual(evt["channel_id"], channel_id)
        self.assertEqual(evt["payload"]["final_state"], "released")

        # since=evt.sequence returns empty (no events with sequence > that).
        tail = self.client.fetch_channel_events(channel_id, evt["sequence"])
        self.assertEqual(tail, [])

    def test_fetch_channel_events_unknown_channel_raises_typed_error(self) -> None:
        with self.assertRaises(ICPError) as ctx:
            self.client.fetch_channel_events("icp_ch_does_not_exist", 0)
        self.assertEqual(ctx.exception.code, "channel.not_found")


if __name__ == "__main__":
    unittest.main()
