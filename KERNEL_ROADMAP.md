# The Kernel Roadmap

**Mission: make StateSet iCommerce the kernel for how agents on the internet
buy, sell, and participate in global commerce together.**

*Successor to [PRD_2026.md](./docs/archive/PRD_2026.md). Written 2026-07-14, against
v1.7.0. This is a campaign plan, not a feature list: a kernel wins by
adoption, and adoption is earned in a specific order.*

---

## Thesis

Commerce infrastructure for AI agents needs three things no incumbent
platform provides together:

1. **An embedded engine** an agent can carry in-process — exact-decimal
   money, oversell-proof inventory, idempotent checkout, signed event log —
   with no SaaS dependency between the agent and its own commerce state.
2. **An open lifecycle protocol** (ICP) whose escrow semantics are defined
   mechanically, so implementations that have never met can agree about who
   holds the money.
3. **Payment rails agents can actually use** (x402/stablecoins) with
   settlement receipts that accounting and tax systems treat as
   authoritative.

The engine exists and is hardened (v1.7.0: A-grade correctness, live-PG
verified atomicity, per-client rate limiting, fail-closed replay guards).
The protocol exists and is now conformance-tested down to its state machine.
What remains is the campaign: completion, trust, compliance, and adoption.

---

## Workstream 1 — Protocol completion & second-party adoption

*A protocol with one implementer is a library. The kernel moment is when a
party we don't control passes conformance.*

**State today:** 4 vector families × 4 IUTs (JS · Rust · Go · Python), all
byte-identical in CI, including the full §8 escrow transition matrix and
event-replay reconstruction (`icp-conformance/vectors/icp-1.0/04-escrow-lifecycle`).

| # | Deliverable | Exit criterion |
|---|---|---|
| 1.1 | `05-intent-verbs` vector family: request/response conformance for all 7 verbs | 4 IUTs byte-identical on every verb's happy path + every normative error |
| 1.2 | `06-settlement-receipts` family: receipt construction + `verify_settlement_receipt` cross-checks | The three SDK helpers and all IUTs agree on valid/invalid receipts |
| 1.3 | Dispute-window timing vectors (deadline arithmetic, §8 expiry trigger) | Deterministic clock-injection cases pass on all IUTs |
| 1.4 | ICP-1.0 **Last Call**: freeze the spec, version the conformance suite 1.0.0 | No open normative ambiguities; conformance = the spec |
| 1.5 | **Second-party implementation** (founder-led): take `icp-spec/PACKET.md` to 3 candidate partners; offer white-glove conformance support | One external codebase passes `icp-1.0-core` in their CI, not ours |

The order matters: 1.1–1.4 make 1.5 credible. Nobody implements a protocol
whose reference suite tests hashing but not money movement — that objection
died this week; the remaining families close it completely.

## Workstream 2 — Trust graduation

*"Agents that have never met exchange real money" requires the trust layer
to graduate from reference-grade to audited.*

**State today:** Settler survives restarts with a persistent signing
identity and fail-closed corrupt-state handling; KEK rotation exists
(`stateset_crypto::rewrap_payload`, no plaintext exposure); signing keys are
zeroized. Custody contract has 15 Foundry tests, no external audit.

| # | Deliverable | Exit criterion |
|---|---|---|
| 2.1 | Settler on durable storage (SQLite via the engine, not JSON files) + multi-instance story | Kill -9 at any point loses zero escrow events |
| 2.2 | Signing-key rotation for Settler + agents (rotate + publish overlap window in discovery doc) | Observers verify events across a rotation boundary |
| 2.3 | External audit of the custody contract + Settler (founder: budget + firm selection) | Published report; findings closed |
| 2.4 | Reputation with teeth: stake-weighted agent reputation, slashing conditions specced in ICP | A bad actor's cost of betrayal exceeds the value of the median intent |
| 2.5 | Graduate PQC deps off release candidates when upstream stabilizes | `ml-kem`/`ml-dsa` at 1.0; hybrid envelope re-verified against test vectors |

## Workstream 3 — Compliance as first-class primitives

*The kernel that wins will be the one that made compliance hooks native
before regulators forced retrofits. Nobody wants to bolt KYC onto a
settlement rail after the fact — this is a moat precisely because it is
unglamorous.*

**State today:** Designed nowhere. This workstream starts at zero and that
is the opportunity.

| # | Deliverable | Exit criterion |
|---|---|---|
| 3.1 | **Compliance hooks RFC**: pre-settlement checkpoint API in ICP — sanctions screening, jurisdiction rules, KYC attestation references — as intent fields + Settler obligations, not middleware | RFC in `icp-spec/`, reviewed by outside counsel |
| 3.2 | SettlementReceipt tax extensions: jurisdiction, tax-collected fields, per-rail reporting identifiers | Receipts satisfy a real accountant for one US state + one EU country |
| 3.3 | Per-jurisdiction Settler capability declarations in the discovery doc (`limits`, allowed corridors) | An agent can determine *before* quoting whether a trade is servable |
| 3.4 | Founder track: money-transmission legal analysis per launch corridor | Written opinion for the first two corridors |

## Workstream 4 — Engineering debt with a schedule

*The B-grade architecture items, scheduled rather than deferred.*

| # | Deliverable | Exit criterion |
|---|---|---|
| 4.1 | **The dual-backend decision** (choose one): query/row abstraction, codegen, or officially demote Postgres to a documented capability subset | 23 SQLite-only domains either ported or documented out of scope; parity drift class closed |
| 4.2 | Binding parity: gift_cards + loyalty in Node & Python; Go/.NET/Swift to core-commerce completeness or honestly re-tiered in docs | `binding-api-inventory.md` gaps are choices, not accidents |
| 4.3 | Build the binding generator `bindings/generator/spec.yaml` describes, or delete it | One binding generated end-to-end, or the spec removed |
| 4.4 | db coverage floor ratchet: 50% → 65% → 80% as db-crate tests grow | Floor raised twice without exemptions |
| 4.5 | Sequencer/VES: publish the federation story (who else can run one?) | A second sequencer operator runbook exists |

## Distribution wedge

ACP/ChatGPT checkout (`stateset-acp-handler`, `npx create-acp-commerce`) is
the adoption channel that doesn't wait for the protocol campaign: every
merchant who wants agentic checkout today is a future ICP endpoint. Keep it
first-class; migrate it to `complete_settled_externally()` (v1.7.0 breaking
change) immediately.

## Sequencing

```
Q3 2026  ██ 1.1–1.3 conformance completion   ██ 2.1–2.2 settler durability + rotation
         ██ 3.1 compliance RFC draft          ██ 4.2 binding parity (Node/Python)
Q4 2026  ██ 1.4 Last Call                     ██ 2.3 audits begin
         ██ 3.2–3.3 receipts + capabilities   ██ 4.1 dual-backend decision executed
Q1 2027  ██ 1.5 second implementer passes     ██ 2.4 reputation with stake
         ██ 3.4 first two corridors legal     ██ 4.5 sequencer federation
```

**The single metric that matters** by Q1 2027: *an agent operated by someone
we've never spoken to completes a purchase from a merchant we've never
spoken to, through a Settler one of them chose, and both sides' accountants
accept the receipt.* Everything above serves that sentence.

---

*Maintenance: this document is reviewed at each release; items move to the
CHANGELOG as they land. Engineering items (1.1–1.4, 2.1–2.2, 2.5, 3.2–3.3,
4.x) are buildable in-repo; items marked "founder" (1.5, 2.3, 3.4) need
decisions and relationships no codebase can produce.*
