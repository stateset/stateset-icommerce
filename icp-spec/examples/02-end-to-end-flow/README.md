# ICP-1.0 — End-to-End Flagship Demo

One self-contained script that walks the entire Intelligent Commerce
Protocol lifecycle and writes a clean transcript suitable for
**embedding in outreach, blog posts, demo videos, and partnership
pitches**.

## Run it

```sh
cd icp-spec/examples/02-end-to-end-flow
node demo.mjs
```

Zero dependencies. Stock Node 20+. Under 5 seconds end-to-end.

Output:
- A formatted markdown transcript printed to stdout
- The same transcript written to `transcript.md` (gitignored — each run
  produces fresh keys, identifiers, timestamps)

## What it does

| # | Step | Validated property |
|---|---|---|
| 1 | Discover counterparty via `icp_capabilities` | The merchant's accepted Settlers and supported verbs are protocol-introspectable |
| 2 | Provision buyer identity (Ed25519 + X25519 + AID) | AID derivation per ICP-1.0 §4.2 |
| 3 | Build + sign a `purchase.create` Intent | Real Ed25519 signature over canonical JSON |
| 4 | Submit Intent → signed Quote | Merchant's signature verification, replay-window check, Settler allowlist gate, `max_total` ceiling |
| 5 | Accept Quote → on-chain funding instructions | Funding calldata points at the deployable `ICPEscrow.sol` contract |
| 6 | Fulfill → 4 signed EscrowEvents in chain | State machine: `pending → funded → fulfilled → released`, each with monotonic `seq` and Settler signature |
| 7 | SettlementReceipt | Co-signed by merchant + Settler (§S.3 requires both for validity) |
| 8 | Audit replay | Anyone with `settlement_id` can re-fetch and verify |
| 9 | Summary | Total bytes signed, total time, what guarantees the protocol gave |

## What this proves

Every artifact in the transcript is **real** — not a mock-up, not a
diagram. Specifically:

- Real Ed25519 keypair generation via `node:crypto`
- Real AID derivation: `aid:v1:z<base58btc(SHA-256(ed_pk || 0x00 || x_pk))>`
- Real RFC 8785 JCS canonical JSON (the same `serde_jcs` produces in
  the Rust IUT — proven byte-identical via the conformance suite)
- Real 64-byte Ed25519 signatures
- Real signature verification at the merchant handler
- Real escrow state machine running through the same `icp-mcp` server
  that plugs into Claude Desktop

The output is markdown so it pastes directly into:
- The PROTOCOL-RFC outreach emails (`icp-spec/outreach/`)
- A GitHub README hero example
- A Hacker News / X / Mastodon launch post
- An internal sales deck

## Sample output (first three steps)

```
# ICP-1.0 End-to-End Demo

**Scenario.** A buyer Agent operating on behalf of a small-business
principal wants to purchase **2 widgets at $29.99 each** ($59.98 cart)
from a merchant Agent it has not transacted with before. The buyer is
willing to pay up to **$70 in USDC**, settled on **Base Sepolia**
(testnet bootstrap). Both parties speak the Intelligent Commerce
Protocol (ICP-1.0).

### Step 1 — Discover counterparty

The buyer's first MCP call is `icp_capabilities` — equivalent to GET
`/icp/v1/.well-known/icp` over HTTP. This tells the buyer who they're
dealing with, which Settlers the merchant accepts, and what spec
version is supported.

{
  "spec": "icp-1.0",
  "merchant_aid": "aid:v1:zMcpMerchant... ",
  "settler_allowlist": ["settler:stateset.usdc.base-sepolia", ...],
  "supported_verbs": ["purchase.create"]
}

> The merchant accepts `settler:stateset.usdc.base-sepolia` — the
> StateSet-operated Base Sepolia testnet bootstrap Settler. The
> buyer's policy permits this.

### Step 2 — Provision buyer identity
...
```

## Swap in a real LLM

The demo currently drives the MCP server with deterministic tool-call
logic. To run the same flow with **Claude actually making the decisions**,
add the `icp-mcp` server to Claude Desktop (`claude-desktop-config.example.json`
in the `icp-mcp/` directory) and prompt:

> "Use the icp tools to purchase 2 widgets at $29.99 each from
> merchant aid:v1:z<the merchant_aid you discover via icp_capabilities>.
> Settle in USDC on Base Sepolia. My budget is $70. Walk me through
> every step and verify each signature."

Claude (or any MCP-compatible agent) will execute the same sequence
because the tools are the same. The deterministic demo proves what's
possible; an LLM-driven session proves the affordance lands for actual
agentic use.

## Why a flagship demo matters

Three sentences in an outreach email don't communicate "this protocol
works." A 9-step transcript that ends in a co-signed SettlementReceipt
does. The demo is the proof-of-life that converts "interesting idea" to
"let's set up a call." It's the same role the Stripe `curl` demo played
in 2010 and the GraphQL playground played in 2015.
