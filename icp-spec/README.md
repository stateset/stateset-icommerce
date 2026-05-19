# Intelligent Commerce Protocol (ICP)

ICP is an open protocol for autonomous agents to negotiate, escrow, settle, and
dispute commerce transactions across organizational boundaries. It is the
operational layer underneath agentic checkout (ACP, AP2) and agent-native
payment rails (x402, USDC).

This directory contains the **normative protocol specification**, kept
deliberately separate from any implementation. The reference implementation
lives in this repository (`crates/`, `cli/`); other implementations are
encouraged and certified via the conformance suite.

## Status

**ICP-1.0 — DRAFT.** Wire format is not yet frozen. Breaking changes possible
until ICP-1.0 ratification. After 1.0, wire format is frozen for the major
version per the versioning policy below.

## Layout

| Path | Purpose |
|---|---|
| `PACKET.md` | Partnership packet — 8-min read, decision-grade summary |
| `ICP-1.0-DRAFT.md` | The normative spec document |
| `SETTLERS.md` | Normative Settler interface specification |
| `PROTOCOL-RFC.md` | Composition with AP2 / ACP / x402 / MCP |
| `handler-design.md` | Design for the sibling `stateset-icp-handler` repo |
| `schemas/` | JSON Schemas, canonicalization rules, error codes |
| `settlers/` | Reference Settler bindings (first: USDC on Base) |
| `contracts/` | On-chain custody contracts (Solidity, audit-ready) |
| `examples/` | Runnable demos including the 9-step transcript |
| `guides/` | Operator-facing integration walkthroughs (merchant, settler, push channels) |
| `outreach/` | Partner-specific outreach drafts (Coinbase, Anthropic, Stripe, ...) |
| `governance/` | Foundation Charter, LOI template, ICPIP process, Risk register |

## Versioning

ICP follows semantic versioning at the protocol level:

- **MAJOR** — breaking change to the wire format, signature scheme, or state
  machine. Implementations MUST NOT silently downgrade.
- **MINOR** — additive changes (new optional fields, new error codes, new
  intent verbs). Implementations MUST ignore unknown fields and unknown verbs
  with a defined error.
- **PATCH** — clarifications, typo fixes, test vector additions. No code
  changes required.

Wire format is frozen for the lifetime of a MAJOR version. The conformance
suite is versioned separately and patches are backward-compatible within a
MAJOR.

## Governance

Until the ICP Foundation is incorporated, **StateSet, Inc.** acts as
specification steward. Issues and pull requests are accepted at
`github.com/stateset/icp-spec`. Material changes are proposed as ICPIPs (see
`governance/ICPIP-process.md`).

The intent is to transfer stewardship to a vendor-neutral foundation
(Delaware 501(c)(6) trade association or Swiss Verein) once 5+ independent
implementations and 3+ Tier-1 commerce platforms have signed an MOU.

## Implementations

| Name | Language | Status | Conformance |
|---|---|---|---|
| `stateset-icommerce` | Rust | Reference | _(pending conformance suite)_ |

Any team building a new implementation: start with `ICP-1.0-DRAFT.md`,
implement against `schemas/`, then run `icp-conformance` (forthcoming).
Conformant implementations are listed here.

## Intellectual property

The specification text is licensed under
**Creative Commons Attribution 4.0 (CC-BY-4.0)**.
The schemas and test vectors are licensed under **Apache-2.0**.
Contributors agree that any patents reading on a contribution are licensed
royalty-free to all implementers under terms compatible with the
W3C Patent Policy (CC-BY-RF or equivalent).

## Why the name

"Intelligent Commerce Protocol" because the parties to the protocol are AI
agents acting on behalf of principals (humans, businesses, other agents).
The protocol does not require AI; a deterministic program may speak ICP.
But it is designed for autonomous parties: every action is signed,
every state transition is verifiable, and every settlement is reconstructable
from the event log. No party need trust any other party's word.
