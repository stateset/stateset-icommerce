#!/bin/bash
# curl demo of the full ICP-1.0 flow against a running icp-handler.
#
# Prerequisites:
#   1. In one terminal: cd icp-handler && node src/server.mjs
#   2. In another: bash curl-demo.sh
#
# This script does NOT build a signed Intent (signing requires crypto code).
# For a full signed-Intent demo, see test/roundtrip.test.mjs.
# This script demonstrates the read-side and error-path endpoints.

set -e
HOST="${HOST:-http://127.0.0.1:8787}"

echo "=== GET /healthz ==="
curl -s "$HOST/healthz" | jq .

echo ""
echo "=== GET /icp/v1/.well-known/icp ==="
curl -s "$HOST/icp/v1/.well-known/icp" | jq .

echo ""
echo "=== GET /icp/v1/settlers ==="
curl -s "$HOST/icp/v1/settlers" | jq .

echo ""
echo "=== POST /icp/v1/intents (unsigned, expect signature.invalid) ==="
# The signature `kid` ("demo") is not a spec `aid:v1:z…` AID, so the handler
# cannot re-derive a binding from it; it falls through to Ed25519 verification,
# which fails against the all-zero key. A real client supplies `kid` = its AID
# plus `_pubkey_hex` AND `_x_pubkey_hex` so the handler can verify the §4.2
# binding (see test/aid-binding.test.mjs).
curl -s -X POST "$HOST/icp/v1/intents" \
  -H 'content-type: application/json' \
  -d '{
    "intent": {
      "v": "icp-1.0",
      "verb": "purchase.create",
      "intent_id": "icp_int_demo",
      "buyer": "aid:v1:zDemo",
      "merchant": "aid:v1:zMerchantDemo",
      "settler": "settler:stateset.usdc.base-sepolia",
      "items": [{"sku":"X","quantity":1,"unit_price":{"amount":"1","currency":"USDC"}}],
      "max_total": {"amount":"2","currency":"USDC"},
      "expiry": "2026-12-31T00:00:00Z",
      "principal_binding": {},
      "nonce": "0123456789abcdef0123456789abcdef",
      "iat": "2026-05-09T00:00:00Z",
      "exp": "2026-05-09T00:09:59Z"
    },
    "signature": {"alg":"ed25519","kid":"demo","sig":"00"},
    "_pubkey_hex": "0000000000000000000000000000000000000000000000000000000000000000"
  }' | jq .

echo ""
echo "For a complete signed-flow demo, run: PORT=0 node --test test/roundtrip.test.mjs"
