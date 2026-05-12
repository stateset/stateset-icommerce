# icp-client (Python)

Pip-installable Python SDK for the Intelligent Commerce Protocol
(ICP-1.0). Mirrors the JavaScript [`@stateset/icp-client`](../icp-client/)
API surface.

Designed for the **agent-developer ecosystem**: Anthropic SDK, OpenAI
Agents, LangChain, LangGraph — all Python-first. Drop ICP into a
Python agent in 5 lines.

## Install

```sh
pip install icp-client
```

Requires Python 3.8+ and the [`cryptography`](https://cryptography.io/)
package (auto-installed).

## Use

```python
from icp_client import ICPClient

client = ICPClient.create(
    handler_url="https://merchant.example/icp",
    principal="did:web:my-store.example",
)

caps = client.capabilities()

# Discovery — browse inventory
snapshot = client.inventory(
    merchant=caps["merchant_aid"],
    settler="settler:circle.usdc.base",
    skus=[{"sku": "WIDGET-001", "quantity": 1}],
    filters={"in_stock_only": True},
)

# Retail purchase
order = client.purchase(
    merchant=caps["merchant_aid"],
    settler="settler:circle.usdc.base",
    items=[{
        "sku": "WIDGET-001", "quantity": 1,
        "unit_price": {"amount": "29.99", "currency": "USDC"},
    }],
    max_total={"amount": "35.00", "currency": "USDC"},
)
funding = client.accept(order["quote"]["quote_id"])
# → buyer wallet signs + broadcasts funding.funding.args

# Observe escrow state via SSE
for event in client.observe(funding["funding"]["escrow_id"]):
    print(event["from_state"], "→", event["to_state"])

# Subscription
sub = client.subscribe(
    merchant=caps["merchant_aid"],
    settler="settler:circle.usdc.base",
    service_id="premium-monthly",
    cadence="30d",
    max_total_per_period={"amount": "29.99", "currency": "USDC"},
    first_charge_at="2026-06-01T00:00:00Z",
)
# Cancel later:
client.cancel(merchant=..., settler=..., subscription_id=sub["authorization"]["subscription_id"])

# Returns
client.return_(
    merchant=..., settler=...,
    original_settlement_id="icp_set_...",
    items=[{"sku": "WIDGET-001", "quantity": 1, "reason": "defective"}],
    desired_outcome="refund",
    max_refund={"amount": "30.00", "currency": "USDC"},
)

# B2B RFQ
proposal = client.request_quote(
    merchant=..., settler=...,
    items=[{"sku": "FASTENER-M6X20", "quantity": 500}],
    purchase_window="30d",
)
# Then commit to it:
client.purchase(
    merchant=..., settler=...,
    items=[...],
    max_total=proposal["proposal"]["total"],
    from_proposal_id=proposal["proposal"]["proposal_id"],
)

# Marketplace payout (seller-signed; inverted direction handled internally)
client.payout(
    platform=...,
    settler=...,
    amount={"amount": "1000", "currency": "USDC"},
    destination={"type": "wallet", "wallet_address": "0x..."},
)

# Audit replay
receipt = client.settlement("icp_set_...")
```

## API

### `ICPClient.create(handler_url, principal, **kwargs) → ICPClient`

| Argument | Type | Description |
|---|---|---|
| `handler_url` | str | Base URL of the ICP HTTP handler |
| `principal` | str | DID/LEI of the principal authorizing this agent |
| `identity` | `Identity?` | Pre-existing identity. Default: generate fresh. |
| `verbs` | `list[str]?` | PrincipalBinding `authority.verbs`. Default: all 7 ICP-1.0 verbs. |
| `max_per_intent` | `dict?` | Authority cap. Default: $10,000 USDC. |
| `revocation_url` | `str?` | Where revocation can be checked. |

### Methods (all 7 ICP verbs)

| Method | Returns | Verb |
|---|---|---|
| `client.capabilities()` | merchant's `.well-known/icp` doc | (discovery) |
| `client.inventory(merchant, settler, ...)` | signed InventorySnapshot | inventory.query |
| `client.purchase(merchant, settler, items, max_total, ...)` | signed Quote | purchase.create |
| `client.accept(quote_id)` | EscrowFunding instructions | (continuation) |
| `client.subscribe(merchant, settler, service_id, cadence, ...)` | signed SubscriptionAuthorization | subscription.create |
| `client.cancel(merchant, settler, subscription_id, effective="immediate")` | signed CancellationAuthorization | subscription.cancel |
| `client.return_(merchant, settler, original_settlement_id, items, desired_outcome, ...)` | signed ReturnAuthorization | purchase.return |
| `client.request_quote(merchant, settler, items, ...)` | signed PriceProposal | quote.request |
| `client.payout(platform, settler, amount, destination, ...)` | signed PayoutAuthorization | payout.request |
| `client.observe(escrow_id)` | iterator over EscrowEvents (SSE) | (real-time) |
| `client.settlement(settlement_id)` | SettlementReceipt | (audit) |

**Every merchant response is independently signature-verified** against
the public key from the merchant's `.well-known/icp` discovery
document. A verification failure raises `ICPError("signature.invalid", ...)`.

### Errors

```python
from icp_client import ICPError

try:
    client.purchase(...)
except ICPError as err:
    if err.code == "policy.settler.not_allowed":
        # Re-route through a different Settler
        ...
    elif err.code == "policy.quote.exceeds_max_total":
        # Renegotiate or reject
        ...
    elif err.code == "signature.invalid":
        # Merchant signature didn't verify — protocol fraud or misconfig
        ...
    else:
        raise
```

Full error code enumeration: `icp-spec/schemas/error-codes.md` in the
[main repo](https://github.com/stateset/stateset-icommerce).

### Identity persistence

For production, persist the identity seed instead of generating fresh:

```python
import json
from pathlib import Path
from icp_client import generate_identity, identity_from_seeds

KEYFILE = Path("keypair.json")

if KEYFILE.exists():
    saved = json.loads(KEYFILE.read_text())
    identity = identity_from_seeds(
        bytes.fromhex(saved["ed25519_seed"]),
        bytes.fromhex(saved["x25519_seed"]),
    )
else:
    identity = generate_identity()
    KEYFILE.write_text(json.dumps({
        "ed25519_seed": identity.ed25519_seed.hex(),
        "x25519_seed": identity.x25519_seed.hex(),
        "aid": identity.aid,
    }))

client = ICPClient.create(handler_url=..., principal=..., identity=identity)
```

In production, seeds live in a KMS or HSM, not on disk.

## Use with Anthropic SDK

The Python SDK plays naturally with the Anthropic API for agent loops:

```python
import anthropic
from icp_client import ICPClient

icp = ICPClient.create(handler_url=..., principal="did:web:my-store")
caps = icp.capabilities()

claude = anthropic.Anthropic()

# Expose ICP as Anthropic SDK tools
tools = [
    {
        "name": "icp_purchase",
        "description": "Buy something via ICP-1.0",
        "input_schema": {
            "type": "object",
            "properties": {
                "items": {"type": "array"},
                "max_total": {"type": "object"},
            },
            "required": ["items", "max_total"],
        },
    },
]

response = claude.messages.create(
    model="claude-sonnet-4-6",
    max_tokens=1024,
    tools=tools,
    messages=[{"role": "user", "content": "Buy 2 widgets for under $70"}],
)

for block in response.content:
    if block.type == "tool_use" and block.name == "icp_purchase":
        result = icp.purchase(
            merchant=caps["merchant_aid"],
            settler="settler:circle.usdc.base",
            items=block.input["items"],
            max_total=block.input["max_total"],
        )
        # Feed `result` back into Claude as a tool_result block
```

The SDK takes care of signing, canonicalization, and merchant
signature verification; the agent just describes what it wants to buy.

## Test

From the `packages/icp-python-client/` directory:

```sh
python3 tests/test_client.py
```

The test spawns a live `icp-handler` server (Node.js), exercises every
public method, asserts merchant signatures verify, and exercises both
forward (purchase) and inverted-direction (payout) flows. **12/12 PASS.**

## License

MIT OR Apache-2.0 (your choice).
