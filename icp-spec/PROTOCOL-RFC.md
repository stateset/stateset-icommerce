# ICP and the Agentic-Commerce Protocol Stack

A short note for maintainers of **AP2** (Google), **ACP** (OpenAI / Stripe),
**x402** (Coinbase), **MCP** (Anthropic), and any team building agent-native
commerce infrastructure.

## TL;DR

ICP is not a competitor to AP2, ACP, x402, or MCP. ICP fills the operational
seam none of them currently address: the **multi-step lifecycle of a
commerce transaction across organizational boundaries** — quote, escrow,
fulfillment proof, dispute, settlement receipt — composing on top of
whatever payment rail and checkout protocol the parties have already chosen.

We're publishing ICP-1.0 as an open spec under CC-BY-4.0 / Apache-2.0 with a
royalty-free patent grant. We have a working reference implementation in
Rust (`stateset-icommerce`, ~250k LOC, 15.8k tests, v1.0.4 shipping). We are
explicitly seeking review, criticism, and partnership before ratification.

## The seam

Today's stack:

```
┌───────────────────────────────────────────────────────────────┐
│  Agent runtime (LangGraph, OpenAI Agents, Anthropic SDK, ...)  │
├───────────────────────────────────────────────────────────────┤
│  Tool surface                                                  │
│    MCP — Anthropic           A2A — Google                      │
├───────────────────────────────────────────────────────────────┤
│  Checkout protocol                                             │
│    ACP — OpenAI/Stripe       AP2 — Google                      │
├───────────────────────────────────────────────────────────────┤
│  ???  ← multi-step lifecycle, escrow, dispute, fulfillment     │
├───────────────────────────────────────────────────────────────┤
│  Payment rail                                                  │
│    x402 — Coinbase    USDC/CCTP    Stripe Treasury    ACH      │
└───────────────────────────────────────────────────────────────┘
```

ACP and AP2 are designed around a one-shot interaction: agent presents
mandate, merchant validates, payment fires. They are excellent at that.

What they don't address — by design — is what happens between order
acceptance and final settlement when the transaction is non-instantaneous:

- **Goods take days to ship.** Where does the value sit in the meantime?
  Who proves fulfillment? What's the dispute window?
- **Services accrue value over time** (a SaaS subscription, a long-running
  AI inference job). When is each release authorized?
- **The buyer is itself an agent with bounded authority.** What stops a
  merchant from returning an upsold quote and relying on the agent
  auto-accepting?
- **Two agents need to settle without trusting each other.** Without a
  protocol-defined escrow + dispute primitive, every pair of agent operators
  reinvents the wheel.

This is the seam ICP fills. It's the thing every B2B commerce stack has
needed for forty years (PO → ASN → invoice → payment) and that the
agentic stack does not yet have.

## How ICP composes

### With ACP

ACP defines `checkout.session` and the merchant-side acceptance flow. ICP
wraps an ACP session with an **EscrowEvent stream** so the agent (and its
principal) gets cryptographic proof of each state transition between
`payment_authorized` and `goods_delivered`. The ACP merchant endpoint can
remain unchanged; ICP adds a sibling endpoint that emits signed
EscrowEvents and a final SettlementReceipt.

Concretely: an ACP-conformant merchant becomes ICP-conformant by adding
one HTTP endpoint (`POST /icp/v1/escrow/events`) and one signing key.

### With AP2

AP2's IntentMandate defines the *authorization*. ICP defines the
*execution lifecycle* of that mandate. An AP2 IntentMandate becomes the
`principal_binding` field of an ICP Intent. The AP2 audit trail and ICP
escrow events are complementary — together they give a regulator-grade
end-to-end story for any Tier-1 commerce flow.

AP2 maintainers: ICP is what your spec implies needs to exist on the
merchant side of the seam. We'd rather extend AP2 than fork it.

### With x402

x402 is a payment-trigger protocol (HTTP 402 + signed payment intent). It
is the fastest path we know of for agent-native value transfer. ICP uses
x402 as one of its `Settler` rails: an ICP `purchase.create` Intent with
`settler: "settler:x402.usdc.base"` produces an x402 payment intent at the
`pending → funded` escrow transition.

We'd like to publish a joint x402 + ICP reference flow showing
deposit → escrow → release on Base, signed end-to-end. Coinbase team:
naming a co-author?

### With MCP

MCP is the tool-surface protocol. ICP is one of the resources an agent
exposes over MCP. The reference implementation ships an MCP server with
700+ commerce tools; the ICP-relevant subset (`icp_intent_create`,
`icp_quote_sign`, `icp_escrow_observe`, `icp_settlement_verify`) is being
factored out into a standalone `icp-mcp` server so any commerce backend
can adopt ICP without adopting the whole engine.

Anthropic: this is roughly a port of the Stripe MCP server pattern but for
the multi-step commerce lifecycle.

## What we're asking for

1. **Read the spec** — `ICP-1.0-DRAFT.md`. It's ~500 lines. The wire format
   has not yet been frozen; structural feedback now is much cheaper than
   after ratification.
2. **Tell us where it overlaps your spec** — if AP2/ACP/x402 already
   addresses something we've duplicated, we want to remove it. The goal is
   composition, not collision.
3. **Co-author a composition document** — a 1-pager per protocol pair
   (`ICP×AP2.md`, `ICP×ACP.md`, `ICP×x402.md`, `ICP×MCP.md`) so
   implementers see exactly how the pieces fit. We'll do the writing; we
   need a reviewer from each side.
4. **Consider ICP Foundation membership** — Delaware 501(c)(6) trade
   association we are standing up to govern the spec post-1.0. Founding
   members get spec-editor seats. The intent is explicit non-capture: no
   single vendor (including StateSet) has more than one board seat.

## What this is not

- **Not a payments network.** ICP doesn't move money; it coordinates the
  parties that do.
- **Not a wallet.** Agents bring their own keys; ICP defines how the keys
  are bound to a principal and how signatures are verified.
- **Not a marketplace.** Discovery and matchmaking are out of scope for
  1.0; we expect AP2's Agent Cards or A2A's directory to fill that.
- **Not a stablecoin.** Value rails are pluggable. USDC, USDT, fiat ACH,
  ETH, SET Chain — all are valid Settlers if they meet §11.1's trust
  criteria.

## Status and timeline

- **2026-Q2** (now): ICP-1.0 Draft published. RI shipping. RFC outreach to
  the four protocol teams above and to top-25 commerce platforms.
- **2026-Q3**: Conformance suite v0.1. Second independent implementation
  (target: TS + Saleor or Medusa plugin). First three SettlementReceipts
  on real USDC volume.
- **2026-Q4**: ICP Foundation incorporated. Founding members announced.
  ICP-1.0 Last Call.
- **2027-Q1**: ICP-1.0 Final. Conformance certification opens.

## Contact

- Spec repo: `github.com/stateset/icp-spec` (forthcoming public)
- Reference impl: `github.com/stateset/stateset-icommerce`
- Editor: `dom@stateset.com`

We are early. The wire format will change. That is the point of publishing
now: it's far cheaper to fix the protocol before it has 100 implementers
than after.

— StateSet, interim ICP spec steward
