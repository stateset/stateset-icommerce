# Intelligent Commerce Protocol (ICP)

Open protocol for autonomous agents to conduct multi-step commerce
transactions across organizational boundaries: **negotiate, escrow,
fulfill, dispute, settle.** Composes with — does not compete with —
agentic-AI's checkout protocols (ACP, AP2), tool protocols (MCP, A2A),
and payment rails (x402, USDC, Stripe Treasury).

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

## Status

| Layer | Artifact | Tests |
|---|---|---|
| **Spec** | [`icp-spec/ICP-1.0-DRAFT.md`](./icp-spec/ICP-1.0-DRAFT.md) — normative protocol; canonicalization rules; 60+ error codes | — |
| **Wire** | **All 7 core intent verbs shipping** (`purchase.create`, `subscription.create`, `subscription.cancel`, `purchase.return`, `inventory.query`, `quote.request`, `payout.request` — see ICPIP-0003/0004), plus the `channel.register` extension verb (ICPIP-0005) | — |
| **Conformance** | [`icp-conformance/`](./icp-conformance/) — vector-driven, language-agnostic | 3 vectors × **4 IUTs (JS · Rust · Go · Python)** = **12 byte-identical PASS** |
| **Reference contract** | [`icp-spec/contracts/usdc-base/ICPEscrow.sol`](./icp-spec/contracts/usdc-base/) — production-quality Solidity | **15/15 Foundry PASS** |
| **HTTP handler** | [`icp-handler/`](./icp-handler/) — zero-dep merchant reference + [`openapi.yaml`](./icp-handler/openapi.yaml) for codegen; ICPIP-0005 register + signed emit + state-transition publish + recovery API | **39/39 Node-test PASS** |
| **MCP server** | [`icp-mcp/`](./icp-mcp/) — drops into Claude Desktop | **6/6 Node-test PASS** |
| **Settler daemon** | [`services/settler-stateset/`](./services/settler-stateset/) — Settler-side reference | **9/9 Node-test PASS** |
| **Demo** | [`icp-spec/examples/02-end-to-end-flow/`](./icp-spec/examples/02-end-to-end-flow/) — 9-step transcript | — |
| **Governance** | [`icp-spec/governance/`](./icp-spec/governance/) — Foundation charter, LOI, ICPIPs, risk register | — |
| **Distribution** | [`icp-spec/outreach/`](./icp-spec/outreach/) — 8 partner-specific drafts | — |
| **Deployment** | [`icp-docker/`](./icp-docker/) — one-command Docker Compose stack | **17/17 integration PASS** against the live stack |
| **SDKs** | [`packages/icp-client/`](./packages/icp-client/) (npm), [`packages/icp-python-client/`](./packages/icp-python-client/) (PyPI), [`crates/stateset-icp-client/`](./crates/stateset-icp-client/) (cargo) — 3 first-party clients, byte-identical wire bytes; **all 3 ship `registerWebhook` + `verifyWebhook` + `fetchChannelEvents` + `verifySettlementReceipt`**; JS has full `.d.ts` types | **JS 33/33 · Python 33/33 · Rust 29/29** |
| **Quickstart** | [`icp-spec/guides/icpip-0005-quickstart.md`](./icp-spec/guides/icpip-0005-quickstart.md) — side-by-side JS / Python / Rust integration in 5 minutes | — |

Cumulative protocol-layer tests on every CI run: **60+ PASS, 0 FAIL** across
HTTP, MCP, Settler, contract, conformance (**4 languages**), demos, and
the **Docker Compose integration suite** (17/17 against the live stack).

## Reproduce in 5 minutes

On a fresh checkout:

```sh
# (1) Conformance: both languages, byte-identical
cd icp-conformance
node runner/run.mjs --iut reference-demo      # JS IUT
node runner/run.mjs --iut stateset-rust       # Rust IUT (build first if needed)
cd ..

# (2) On-chain contract: 15 Foundry tests
cd icp-spec/contracts/usdc-base
forge install OpenZeppelin/openzeppelin-contracts foundry-rs/forge-std --no-git
forge test
cd ../../..

# (3) HTTP handler: full lifecycle roundtrip
cd icp-handler && PORT=0 node --test test/roundtrip.test.mjs && cd ..

# (4) MCP server: LLM-agent-shape tools
cd icp-mcp && node --test test/mcp.test.mjs && cd ..

# (5) Settler daemon: signed EscrowEvents + Receipts
cd services/settler-stateset && PORT=0 node --test test/settler.test.mjs && cd ../..

# (6) Flagship transcript demo
cd icp-spec/examples/02-end-to-end-flow && node demo.mjs && cd ../../..
```

Every line green. Every signature real Ed25519. Every state transition
verifiable. Zero mocks at the protocol layer.

## What ICP is

A protocol for the **operational lifecycle of a commerce transaction**
when the parties are autonomous AI agents:

- **purchase.create** — buyer Agent requests goods/services, signs an
  Intent with a price ceiling. Merchant Agent responds with a signed
  Quote. Buyer Accepts; Settler escrows funds. Merchant delivers;
  Settler verifies + releases. A co-signed SettlementReceipt is the
  canonical proof for tax, accounting, and audit.

- **subscription.create** — buyer Agent authorizes recurring purchases
  on a cadence (`30d`, `1y`, etc.) with a per-period cap. Each cycle
  triggers an automatic purchase.create occurrence. The per-period cap
  protects the buyer from auto-billing exploits — protocol-level
  guarantee, not policy.

- **purchase.return** — buyer Agent requests return / refund / replacement
  for a prior settled transaction, referencing the original
  `settlement_id`. Merchant signs a ReturnAuthorization with refund
  instructions or returns a typed `policy.return.*` error. The
  `max_refund` ceiling protects the buyer the same way `max_total`
  does on forward purchases.

- **inventory.query** — buyer Agent's signed read-only query for
  availability + pricing. Merchant signs an InventorySnapshot with
  `valid_until` validity window. Highest-volume verb by call count
  in B2B agentic commerce. Doesn't trigger an escrow; serves as the
  discovery primitive that precedes every value-transferring Intent.

- **subscription.cancel** — buyer Agent's signed cancellation of an
  existing subscription. Merchant returns a CancellationAuthorization
  with `effective_at` (immediate or end-of-period per merchant policy)
  and an optional `pro_rated_refund`. Idempotent. Without this verb,
  the only way out of a subscription is out-of-band — which produces
  no audit-grade record.

- **quote.request** — buyer Agent requests pricing without commitment;
  the B2B wholesale RFQ primitive. Merchant signs a non-binding
  PriceProposal with a `valid_until` window. The buyer commits later
  with a `purchase.create` referencing `from_proposal_id`, and the
  merchant must honor proposal prices while the proposal is valid.
  (ICPIP-0003)

- **payout.request** — seller Agent requests release of platform-held
  funds; the marketplace payout primitive and the only verb with
  inverted signing direction (the recipient signs the Intent). Platform
  signs a PayoutAuthorization with itemized binding fees and the rail
  finalization timing. (ICPIP-0004)

All seven core verbs ship in ICP-1.0. ICPIP-0005 adds an eighth, the
`channel.register` extension verb, for registering webhook/SSE push
channels.

## What ICP is NOT

- **Not a payments network.** ICP doesn't move money; it coordinates
  the parties that do.
- **Not a wallet.** Agents bring their own keys.
- **Not a marketplace.** Discovery is out of scope for 1.0.
- **Not a stablecoin issuer.** USDC, EURC, ACH USD, ETH — all are
  valid Settlers if they meet §11.1 trust criteria.
- **Not a chain.** Rail-agnostic. Base, Ethereum, Solana, ACH,
  Lightning, Fedwire — all valid.
- **Not StateSet's product.** Spec is CC-BY-4.0 + Apache-2.0 with RF
  patent grant. ICP Foundation transfers stewardship at incorporation
  (see [Foundation Charter](./icp-spec/governance/FOUNDATION-CHARTER.md)).

## Why now

Three forces converging in 2026:

1. **Tool protocols matured.** MCP and A2A landed in production.
   Agents now have a real tool surface.
2. **Checkout protocols emerged.** ACP and AP2 launched. Agents can
   buy.
3. **Payment rails for agents shipped.** x402 + USDC/CCTP. Agents
   can pay.

What's missing is the **operational lifecycle** between
checkout-acceptance and settlement — quote-binding, escrow,
fulfillment evidence, dispute, signed settlement receipt. Without it,
every agent-commerce stack reinvents this layer privately, with all
the mistakes that entails. With it, the layer is a shared open
protocol.

This is the same gap that PCI-DSS, EMV, ISO 20022, and SWIFT MT/MX
filled for the prior generation of commerce. The agentic generation
needs its equivalent. The window to define it is 12–18 months. After
that, big tech defines it unilaterally.

## Plug into Claude Desktop

`~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "icp": {
      "command": "node",
      "args": ["/abs/path/to/stateset-icommerce/icp-mcp/src/server.mjs"]
    }
  }
}
```

Restart Claude Desktop. Then ask:

> "Use the icp tools to subscribe me to a monthly $29.99 SaaS service
> from merchant aid:v1:zMcpMerchant.... Settle in USDC on Base
> Sepolia. My budget is $30/month. Walk me through every step."

Claude calls `icp_capabilities`, `icp_keypair_generate`,
`icp_intent_build_and_sign({verb: 'subscription.create', ...})`,
`icp_intent_submit`, and inspects the resulting signed
SubscriptionAuthorization. Every signature is real Ed25519. The
transaction works end-to-end on stock Node 20+.

## For partners

If you're at Anthropic, OpenAI, Stripe, Coinbase, Circle, Google,
Shopify — or you're building anything in agentic commerce — start
here:

| Read | Time | Outcome |
|---|---|---|
| **[PACKET.md](./icp-spec/PACKET.md)** | 8 min | Decision-grade summary; the document for your CEO |
| [ICP-1.0-DRAFT.md](./icp-spec/ICP-1.0-DRAFT.md) | 30 min | Full normative spec |
| [PROTOCOL-RFC.md](./icp-spec/PROTOCOL-RFC.md) | 10 min | How ICP composes with your protocol |
| [FOUNDATION-CHARTER.md](./icp-spec/governance/FOUNDATION-CHARTER.md) | 20 min | What founding membership commits and protects |
| [RISKS.md](./icp-spec/governance/RISKS.md) | 5 min | Honest risk register |

We're explicitly seeking review, criticism, and partnership before
ICP-1.0 ratification. Structural feedback now is much cheaper than
after.

**Contact:** `dom@stateset.com` · spec repo: `github.com/stateset/icp-spec`

## Path to billions

| Phase | Target | When |
|---|---|---|
| ICP-1.0 Final | Spec ratification | 2026-Q4 |
| First testnet volume | 100 txn on Base Sepolia | 2026-Q3 |
| First mainnet volume | $1M annualized | 2027-Q1 |
| $100M annualized | 10k merchants × $10k/yr | 2027-Q4 |
| $1B annualized | Stripe Treasury rail live, Foundation steady-state | 2028-Q4 |
| $10B annualized | 2+ Tier-1 commerce stacks embed ICP | 2029+ |

Detailed reasoning + per-phase preconditions in
[PACKET.md](./icp-spec/PACKET.md) §"Path to billions."

## Repository surfaces

```
stateset-icommerce/
├── icp-spec/                    # the protocol
│   ├── ICP-1.0-DRAFT.md         #   normative spec
│   ├── PACKET.md                #   partnership pitch
│   ├── PROTOCOL-RFC.md          #   cross-protocol composition
│   ├── SETTLERS.md              #   Settler interface
│   ├── handler-design.md        #   handler architecture
│   ├── schemas/                 #   JSON Schemas + canonicalization + error codes
│   ├── contracts/usdc-base/     #   ICPEscrow.sol (audit-ready)
│   ├── settlers/usdc-base.md    #   first reference Settler
│   ├── governance/              #   Foundation charter, LOI, ICPIPs, risks
│   ├── outreach/                #   partner-specific drafts
│   ├── examples/                #   runnable demos (incl. 9-step transcript)
│   └── STATUS.md                #   live build status
├── icp-conformance/             # cross-IUT determinism test harness
│   ├── runner/                  #   the runner (~250 LOC, no deps)
│   ├── vectors/icp-1.0/         #   01-aid-derivation, 02-canonical-json
│   ├── iut-adapters/            #   reference-demo (JS), stateset-rust
│   └── profiles/                #   icp-1.0-core, etc.
├── icp-handler/                 # merchant Backend reference (HTTP)
├── icp-mcp/                     # MCP transport reference
├── services/settler-stateset/   # Settler operator reference
├── crates/stateset-icp-iut/     # Rust IUT for conformance
└── (rest of the repo: stateset-icommerce engine — orthogonal to ICP)
```

The protocol-layer surfaces are intentionally small. Each is
readable end-to-end in under an hour. Each is replaceable: a team can
write their own handler, their own Settler, their own IUT, and as
long as conformance passes, they interoperate.

## License

- **Specification prose** (markdown): CC-BY-4.0
- **Schemas, test vectors, conformance suite, code**: Apache-2.0
- **Reference contracts** (Solidity): Apache-2.0
- **Patent policy**: contributors grant a royalty-free, irrevocable
  patent license for any patents reading on necessary ICP
  implementation. See [FOUNDATION-CHARTER.md §5.2](./icp-spec/governance/FOUNDATION-CHARTER.md).
