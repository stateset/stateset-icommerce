# ICP × Claude Agent Demo

End-to-end demonstration of an Anthropic Claude agent driving a
complete ICP-1.0 commerce transaction through tool calls.

This is the artifact that converts "ICP works in tests" → "Claude
actually buys things via ICP." Send the resulting transcript on
Twitter / HN / partnership emails for the visceral proof.

## Run

```sh
pip install -r requirements.txt   # anthropic + cryptography
export ANTHROPIC_API_KEY=sk-ant-...
python3 claude_agent.py
```

Output: `transcript.md` (also printed to stdout). Captures Claude's
reasoning at each turn + every ICP tool call + the final co-signed
SettlementReceipt.

**Without an API key**, the script automatically falls back to a
deterministic simulator that walks the same tool sequence without
LLM reasoning. Useful for CI and for environments without API access.
The simulator proves the architecture works; the real Claude path
proves Claude actually uses it.

## What this proves

1. **The Python SDK works in a real LLM agent loop.** `icp-client`
   imports cleanly into a Python agent, every method returns Python
   dicts, the tool-result serialization round-trips correctly.
2. **The 7 ICP verbs are LLM-tractable.** Claude can read the tool
   descriptions, infer which to call, parameterize them correctly,
   and chain them into a complete purchase flow.
3. **Independent signature verification survives the agent loop.**
   Every merchant response that flows back through the tool dispatcher
   is verified against `.well-known/icp` pubkeys by the SDK — Claude
   doesn't need to know how Ed25519 works.
4. **Errors are recoverable.** When Claude submits an Intent that
   fails policy (`policy.settler.not_allowed`, `policy.quote.exceeds_max_total`,
   etc.), the typed `ICPError` surfaces as a `tool_result` with
   `is_error=true`. Claude reads the error code and retries or asks
   the user for guidance.

## Tools exposed to Claude

Five tools that wrap the 7 ICP verbs (some verbs map to one tool,
others compose):

| Tool | ICP verb(s) | Purpose |
|---|---|---|
| `icp_capabilities` | (discovery) | Discover merchant + Settler allowlist |
| `icp_inventory` | `inventory.query` | Read prices and availability |
| `icp_purchase` | `purchase.create` | Submit a signed Intent → get Quote |
| `icp_accept` | (continuation) | Accept Quote → trigger escrow + fulfillment |
| `icp_observe` | (SSE) | Watch escrow state transitions |

The other ICP verbs (`subscription.create`, `subscription.cancel`,
`purchase.return`, `quote.request`, `payout.request`) follow the same
pattern — wrap the SDK method, define an Anthropic schema, route via
the dispatcher. Adding them is ~30 LOC each.

## Sample transcript (simulator output)

```
## Simulated agent (no ANTHROPIC_API_KEY set)

### Agent → icp_capabilities
{
  "merchant_aid": "aid:v1:zMcpMerchant...",
  "verbs": ["purchase.create", "subscription.create", ..., "payout.request"]
}

### Agent → icp_inventory
[
  { "sku": "WIDGET-001", "available_quantity": 47, "unit_price": {"amount": "29.99", "currency": "USDC"} },
  ...
]

### Agent → icp_purchase
{ "quote_total": {"amount": "62.98", "currency": "USDC"}, "quote_id": "icp_qt_..." }

### Agent → icp_accept
{ "escrow_id": "0x...", "has_settlement": true }

### Final SettlementReceipt (co-signed)
{ "settlement_id": "icp_set_...", "amount": {"amount": "62.98", "currency": "USDC"}, "final_state": "released" }
```

When run with `ANTHROPIC_API_KEY` set, the transcript includes Claude's
**reasoning between each tool call**: explanations of what it's
deciding, why it picked specific parameter values, how it interpreted
each response. That's the artifact that converts "the tools exist"
into "watch an LLM agent actually decide to use them."

## Cost (real Claude path)

Approximate Anthropic API cost per demo run with claude-sonnet-4-5
(Nov 2024 pricing):
- ~5 turns × ~2k tokens average = ~10k tokens total
- Input: ~$0.03 / Output: ~$0.05
- **Cost per full demo: ~$0.08**

Cheap enough to run for every demo at a partner meeting.

## Why this matters for ICP

Tier-1 partners (Anthropic, OpenAI, Stripe, etc.) reviewing ICP have
two reactions to consider:

1. *"This is interesting but our agents don't speak it yet"* — until
   tick 30 (v1.5.0), partially true. The Python SDK existed but
   nobody had wired Anthropic SDK to it.

2. *"Show me an LLM agent doing this end-to-end"* — until this demo,
   not possible. Now: 60 seconds of Claude reasoning + ICP tool calls
   produces a co-signed SettlementReceipt.

The demo bridges the gap between "the protocol exists" and "AI agents
use the protocol." It's the same role that the Stripe Connect "first
$1 transferred" demo played in 2012 — proof-of-life that the
abstraction maps to real value flow.

## What's not yet here

- **Multi-merchant comparison shopping**: agent queries inventory
  across N merchants, picks the cheapest. Trivial to add (Claude
  loops the `icp_inventory` tool); not in this demo to keep the
  transcript short.
- **Subscription + cancel** flow: same pattern, extend the
  dispatcher.
- **B2B RFQ + commit** flow (`quote.request` → `purchase.create
  with from_proposal_id`): same pattern.
- **Marketplace payout** (inverted-direction): same pattern, but the
  agent is the SELLER.
- **Compounding agents**: an agent that uses ICP to fund another
  agent's ICP transaction (e.g. one agent pays another for compute).
  Requires a small DAG runner on top of the SDK.

All five are ~50 LOC additions on top of the existing dispatcher.

## License

CC-BY-4.0 for the prose; Apache-2.0 for the script. Reuse freely.
