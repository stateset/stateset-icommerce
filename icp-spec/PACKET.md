# ICP Partnership Packet

*Read time: 8 minutes. Decision-grade.*

## What ICP is

The **Intelligent Commerce Protocol** is an open spec for autonomous
agents to conduct multi-step commerce transactions across organizational
boundaries: negotiation, escrow, fulfillment, dispute, settlement.

It composes with — does not compete with — the agentic-commerce stack
that's already in flight:

```
┌─────────────────────────────────────────────────────────────────┐
│  Agent runtime: LangGraph · OpenAI Agents · Anthropic SDK · ...  │
├─────────────────────────────────────────────────────────────────┤
│  Tool surface:  MCP (Anthropic) · A2A (Google)                  │
├─────────────────────────────────────────────────────────────────┤
│  Checkout:      ACP (OpenAI/Stripe) · AP2 (Google)              │
├─────────────────────────────────────────────────────────────────┤
│  ⟶ ICP — quote · escrow · fulfillment · dispute · settlement ⟵  │
├─────────────────────────────────────────────────────────────────┤
│  Payment rail:  x402 (Coinbase) · USDC/CCTP · Stripe Treasury    │
└─────────────────────────────────────────────────────────────────┘
```

ICP fills the operational seam between checkout-acceptance and final
settlement. No other public protocol covers this layer.

## Status (verifiable, not pitch-deck)

| Asset | Status | Where |
|---|---|---|
| Open spec, ICP-1.0 draft | Normative, frozen surface | `icp-spec/ICP-1.0-DRAFT.md` |
| Canonical serialization rules | Normative (CBOR + JSON) | `icp-spec/schemas/canonicalization.md` |
| Error code enumeration | 60+ codes, 13 namespaces, frozen | `icp-spec/schemas/error-codes.md` |
| Settler interface | Normative spec | `icp-spec/SETTLERS.md` |
| First reference Settler binding | USDC on Base | `icp-spec/settlers/usdc-base.md` |
| On-chain custody contract | Audit-ready Solidity, **15/15 Foundry tests PASS** | `icp-spec/contracts/usdc-base/` |
| Conformance suite | 2 vectors, both pass, CI-gated | `icp-conformance/` |
| Two independent IUTs (JS + Rust) | Byte-identical wire bytes, CI-enforced | `icp-conformance/iut-adapters/` |
| HTTP handler reference | **6/6 roundtrip tests PASS** | `icp-handler/` |
| MCP server reference | **6/6 roundtrip tests PASS** | `icp-mcp/` |
| End-to-end demo + transcript | ~5 second run, runnable now | `icp-spec/examples/02-end-to-end-flow/` |
| Governance charter | Draft, ready to incorporate | `icp-spec/governance/FOUNDATION-CHARTER.md` |
| Outreach packet | 8 partner-specific emails drafted | `icp-spec/outreach/` |

Reproduce all of this in **under 5 minutes** on a fresh machine:

```sh
git clone github.com/stateset/stateset-icommerce
cd stateset-icommerce
# Conformance (~10 sec)
cd icp-conformance && node runner/run.mjs && cd ..
# Contracts (~30 sec)
cd icp-spec/contracts/usdc-base && forge install OpenZeppelin/openzeppelin-contracts foundry-rs/forge-std --no-git && forge test && cd ../../..
# HTTP handler (~3 sec)
cd icp-handler && PORT=0 node --test test/roundtrip.test.mjs && cd ..
# MCP server (~5 sec)
cd icp-mcp && node --test test/mcp.test.mjs && cd ..
# Flagship demo (~5 sec)
cd icp-spec/examples/02-end-to-end-flow && node demo.mjs && cd ../../..
```

Every line green. Every signature real Ed25519. Every state transition
verifiable. Zero mocks at the protocol layer.

## Why now

Three forces converging in 2026:

1. **Tool protocols matured.** MCP and A2A landed in production. Agents
   now have a real tool surface.
2. **Checkout protocols emerged.** ACP and AP2 launched. Agents can buy.
3. **Payment rails for agents shipped.** x402 + USDC/CCTP. Agents can
   pay.

What's missing is the **operational lifecycle** between
checkout-acceptance and settlement — quote-binding, escrow, fulfillment
evidence, dispute, signed settlement receipt. Without it, every
agent-commerce stack reinvents this layer privately, with all the
mistakes that entails. With it, the layer is a shared open protocol.

This is the same gap that PCI-DSS, EMV, ISO 20022, and SWIFT MT/MX
filled for the prior generation of commerce. The agentic generation
needs its equivalent. The window to define it is 12–18 months. After
that, big tech defines it unilaterally.

## What we're asking

Three concrete asks, one Tier-1 partner at a time:

### Ask 1 — Review the spec

Read `ICP-1.0-DRAFT.md` (≤500 lines) and tell us where it overlaps your
roadmap, where it duplicates something you've already specced, and
where it's wrong. Structural feedback now is much cheaper than after
ICP-1.0 ratification.

**Effort on your side: ~2 hours.** No commitments attached.

### Ask 2 — Co-author a composition document

One paired protocol (ICP × ACP, ICP × AP2, ICP × x402, or ICP × MCP),
one 1-pager showing how they fit. We do the writing; we need a named
reviewer who can speak for your spec's design intent.

**Effort: 1 working session + light async review.** Output: a joint
publication carrying both organizations' names.

### Ask 3 — Foundation founding membership

Join the ICP Foundation (Delaware 501(c)(6) trade association,
incorporation Q3 2026 contingent on 5+ founding members). One board
seat. Membership dues tiered $25k–$250k/year. RF patent grant for any
patents reading on a member contribution. No equity, no IP transfer.

**Effort: legal review, sign LOI, allocate one engineering reviewer.**
See `governance/LOI-TEMPLATE.md` and `governance/FOUNDATION-CHARTER.md`.

## Path to billions

| Phase | Target | What it requires | When |
|---|---|---|---|
| Spec frozen | ICP-1.0 Final ratification | 5 founding members + audit + 2nd impl in production | 2026-Q4 |
| First testnet volume | 100 transactions on Base Sepolia | Settler signing daemon + 1 design partner merchant | 2026-Q3 |
| First mainnet volume | $1M aggregated annualized | Circle operating `settler:circle.usdc.base`, 1 Tier-1 partner integrating | 2027-Q1 |
| $100M annualized | 10k merchants × $10k/yr average OR 100 large operators × $1M/yr | Multi-rail Settler diversity, real KYB/KYC, foundation operating | 2027-Q4 |
| $1B annualized | 100k merchants OR 1000 large operators | Stripe Treasury rail live, Foundation governance steady-state, ICPIPs flowing | 2028-Q4 |
| $10B annualized | Embedded in 2+ Tier-1 commerce stacks | Travel Rule compliance, regulator engagement, multi-jurisdiction Settlers | 2029+ |

**Comparable trajectories:** Stripe took 5 years to $1B GPV. Lightning
Network took 5 years to $700M/yr. ACH took 50 years to reach today's
scale. ICP's market window is shaped by the agentic-AI transition, which
is moving faster than any commerce shift in history.

## What this is NOT

- **Not a payments network.** ICP doesn't move money; it coordinates
  the parties that do.
- **Not a wallet.** Agents bring their own keys.
- **Not a marketplace.** Discovery is out of scope for ICP-1.0.
- **Not a stablecoin issuer.** USDC, EURC, Stripe USD, ACH USD, ETH —
  all are valid Settlers if they meet §11.1 trust criteria.
- **Not a chain.** ICP is rail-agnostic. SET Chain is one Settler;
  Base, Ethereum, Solana, ACH, Lightning, Fedwire are all valid.
- **Not StateSet's product.** Spec is CC-BY-4.0 + schemas Apache-2.0
  with RF patent grant. Foundation transfers stewardship at incorporation.

## Risk register (honest)

Full register at `governance/RISKS.md`. The five we're most aware of:

1. **Big-tech capture.** AP2 or ACP could absorb the lifecycle layer.
   Mitigation: aggressive composition framing (we *complement*, not
   compete). If they do absorb it, ICP becomes the conformance bridge.
2. **Single-author bus factor.** Most spec authorship traces to one
   person. Foundation incorporation + ICPIP editor diversity is the
   structural fix.
3. **Regulatory drift.** Travel Rule, MTL requirements, Stable-coin
   Genius Act — all shifting. Mitigation: Settler interface abstracts
   the regulatory boundary; non-compliant Settlers are revoked from
   the allowlist without protocol changes.
4. **Adoption frost.** First-mover discount: until 3+ Tier-1 partners
   adopt, ICP is "interesting but observed." Mitigation: design partner
   program with discounted dues for the first 5 commerce platforms.
5. **Quantum migration.** Ed25519 will be broken by CRQCs eventually.
   Mitigation: spec already allows hybrid `ed25519+ml-dsa-65` signatures;
   migration window is 5–10 years.

## Contact

- **Editor:** `dom@stateset.com`
- **Spec repo:** `github.com/stateset/icp-spec` (public Q3 2026; private
  preview available on request)
- **Reference implementation:** `github.com/stateset/stateset-icommerce`

If after reading this packet you want a 30-minute working session, reply
to whichever outreach email you received with three time-slots. We'll
take the first one.

If the answer is no, we'd really like to know why. "Wrong layer," "scope
collision with X," "too crypto-heavy," "not crypto enough" — each
answer materially improves the next conversation.
