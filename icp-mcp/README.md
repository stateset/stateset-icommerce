# icp-mcp — MCP server for ICP-1.0

Speaks the [Model Context Protocol](https://modelcontextprotocol.io) over
stdio so an LLM agent (Claude Desktop, Cursor, Windsurf, custom Anthropic
or OpenAI agent) can transact commerce via ICP-1.0 by calling tools — no
HTTP, no protocol details to memorize.

**Zero dependencies.** Pure Node.js, ~600 LOC.

## Why this exists

ICP-1.0 defines a multi-step commerce lifecycle (quote, escrow, fulfillment,
dispute, settlement) on top of agent-native payment rails. The MCP binding
turns that lifecycle into a tool surface an LLM already knows how to use.

For Anthropic's `PROTOCOL-RFC.md` "With MCP" composition: this is the
reference implementation of the binding.

## Plug into Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`
(macOS) or the equivalent on your platform:

```json
{
  "mcpServers": {
    "icp": {
      "command": "node",
      "args": ["/absolute/path/to/icp-mcp/src/server.mjs"]
    }
  }
}
```

Restart Claude Desktop. The 8 ICP tools become available in any
conversation. Ask Claude something like:

> "Use the icp tools to buy 1 widget at $100 from merchant
> aid:v1:zMerchantPlaceholder. Settle in USDC on Base Sepolia.
> Walk me through every step."

## Run standalone

```sh
node src/server.mjs
```

The server speaks JSON-RPC 2.0 on stdin/stdout. Logs go to stderr.

## Test

```sh
node --test test/mcp.test.mjs
```

The test suite drives the server as Claude Desktop would: spawns it as a
subprocess, performs the initialize handshake, walks the full ICP lifecycle
via tool calls. **6/6 PASS** including negative cases.

## Tool surface

| Tool | Purpose |
|---|---|
| `icp_capabilities` | Discover server: spec version, allowlisted Settlers, merchant identity |
| `icp_keypair_generate` | Fresh Ed25519 + X25519 keypair + derived AID (testing) |
| `icp_intent_build_and_sign` | Build + sign a `purchase.create` Intent |
| `icp_intent_submit` | Submit Intent → signed Quote |
| `icp_quote_accept` | Accept Quote → on-chain funding instructions |
| `icp_escrow_state` | Current escrow state + event log |
| `icp_fulfill` | Submit fulfillment evidence (demo auto-releases) |
| `icp_settlement_get` | Fetch co-signed SettlementReceipt |

Every tool returns either a structured result or a typed `error` object
with an ICP error code (e.g. `signature.invalid`, `policy.settler.not_allowed`,
`replay.expired`). LLMs can branch on the code.

## Example tool sequence (LLM-readable)

```text
1. agent → icp_capabilities()
   ← {merchant_aid, settler_allowlist, supported_verbs}

2. agent → icp_keypair_generate()
   ← {aid, ed25519_seed_hex, x25519_pubkey_hex}

3. agent → icp_intent_build_and_sign({
      ed25519_seed_hex, x25519_pubkey_hex,
      merchant_aid, settler,
      items: [{sku, quantity, unit_price}],
      max_total
   })
   ← {intent, signature, _pubkey_hex}

4. agent → icp_intent_submit({intent, signature, _pubkey_hex})
   ← {quote, signature} or {error: {code: "policy.quote.exceeds_max_total"}}

5. agent → icp_quote_accept({quote_id})
   ← {funding: {escrow_id, chain, contract, function, args}}

   (buyer wallet now signs and broadcasts the funding tx — out of MCP scope)

6. agent → icp_fulfill({escrow_id, evidence_id})  // merchant side
   ← {receipt: {settlement_id, final_state, amount, ...signatures}}

7. agent → icp_settlement_get({settlement_id})  // either side, audit
   ← {receipt}
```

## Composition with ACP / AP2 / x402

This server is **complementary** to the agentic-checkout protocols:

- **ACP / AP2**: agent presents authorization, merchant accepts at checkout.
- **ICP**: from that point until SettlementReceipt is signed, the lifecycle
  is recorded as a signed event chain so both parties (and any auditor)
  can verify the sequence later.
- **x402**: ICP names x402 as a Settler rail. The Settler-side EscrowEvent
  stream carries x402 payment confirmations.

An agent that already speaks MCP for tool surface, ACP/AP2 for checkout,
and x402 for payments can now also speak ICP for the lifecycle between
acceptance and settlement — without learning new protocols, because every
ICP operation is a tool call.

## What this is NOT yet

- **Resources/prompts**: the MCP spec supports resources and prompts in
  addition to tools. The next minor version of `icp-mcp` will expose the
  SettlementReceipt corpus as MCP resources (`icp://settlements/<id>`)
  and provide a `commerce-walkthrough` prompt for first-time agents.
- **Persistence**: state is in-memory per process. Restart loses
  everything. Production handlers persist via the real engine.
- **Real chain**: the `icp_fulfill` tool simulates funding and release
  for demo purposes. Production needs the Settler signing daemon
  (forthcoming, sibling to `icp-handler`).

## Status

**Reference implementation, ICP-1.0 partial conformance** (purchase.create
only, plus all read-side tools). Tracks the spec. Suitable for
Anthropic-team review of the MCP binding section in
`icp-spec/PROTOCOL-RFC.md`.
