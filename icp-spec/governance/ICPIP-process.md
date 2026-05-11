# ICPIP — Intelligent Commerce Protocol Improvement Proposal Process

Modeled on Ethereum's EIP and Bitcoin's BIP processes. ICPIPs are the only
mechanism for normative changes to the protocol after ICP-1.0 ratification.

## Types

- **Standards Track** — changes affecting the wire format, signature scheme,
  state machine, or any normative behavior. Two sub-tracks:
  - **Core** — protocol semantics (intent verbs, escrow states, settlement).
  - **Networking** — transport bindings (HTTP, gRPC, MCP, libp2p).
- **Meta** — process changes (this doc, governance, IP policy).
- **Informational** — design notes, best practices, security analyses.

## Statuses

`Draft` → `Review` → `Last Call` → `Final`
                          │
                          └─▶ `Withdrawn` / `Stagnant`

A Standards Track ICPIP **MUST** spend at least 14 days in `Last Call` and
**MUST** have at least two independent implementations passing the new
conformance vectors before promotion to `Final`.

## Lifecycle

1. **Pre-discussion** on the icp-spec repo Discussions board. Idea sketch.
2. **Draft PR** in `icp-spec/icpips/icpip-NNNN-<short-name>.md` using the
   template at `icpips/icpip-template.md`.
3. **Review** — ICPIP Editors confirm format compliance and assign a number.
4. **Last Call** — 14-day public review window. Material objections must be
   resolved on-record.
5. **Final** — locked text. Becomes part of the next protocol minor or major.

## Editors

Until the ICP Foundation is incorporated, ICPIP Editors are appointed by
StateSet, Inc. with public nomination via PR. After Foundation incorporation,
Editors are confirmed by Foundation board majority.

Current editors: _(to be appointed at ICP-1.0 ratification)_.

## ICPIP-0001 placeholder

ICPIP-0001 is reserved for the ICPIP Process itself (this document) once
ratified. After ratification, this file becomes `icpips/icpip-0001.md`.
