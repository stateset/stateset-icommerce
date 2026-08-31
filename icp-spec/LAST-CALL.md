# ICP-1.0 Last Call

**Entered:** 2026-08-31
**Review window closes:** 2026-09-14 (14 days, per `governance/ICPIP-process.md`)
**Editor:** StateSet, Inc. (interim steward) — dom@stateset.com

ICP-1.0 has entered Last Call. The normative surface is frozen: only
editorial corrections and on-record resolutions of material objections
filed during the window may change `ICP-1.0-DRAFT.md`, `SETTLERS.md`,
`schemas/canonicalization.md`, or `schemas/error-codes.md` before the
spec is promoted to Final. Everything else is ICP-1.1 material and goes
through the ICPIP process.

## Why the exit criteria are met

The ICPIP process requires at least two independent implementations
passing the conformance vectors before Final. The evidence at Last Call
entry exceeds that:

- **Ten normative vector families** in `icp-conformance/vectors/icp-1.0/`:
  aid-derivation, canonical-json (22 sub-cases), signature-verification,
  escrow-lifecycle (full §8 transition matrix + event replay),
  intent-validation, quote-binding, settlement-receipts, timing,
  ceilings, and commerce-invariants.
- **Four reference IUTs** (JavaScript, Rust, Go, Python) pass every
  vector with **byte-identical outputs**, enforced as a blocking CI gate.
- **Conformance suite released as `icp-conformance 1.0.0`** covering
  ICP-1.0 (`icp-conformance/README.md` §Versioning). The suite is the
  conformance definition: an implementation is ICP-1.0 conformant iff it
  passes 100% of the `icp-1.0` vectors (`ICP-1.0-DRAFT.md` §2).
- **No open normative ambiguities.** The tracked open questions are all
  explicitly deferred to ICP-1.1: atomic cross-Settler swap,
  settler-of-last-resort, subsidized fees (`SETTLERS.md` §Open
  questions), floats in metadata, streaming canonicalization, CBOR tags
  (`schemas/canonicalization.md` §5).

## Filing an objection

Open a GitHub issue on the spec repository titled `[last-call] <topic>`,
or email the editor. A **material objection** identifies a normative
statement that is ambiguous, unimplementable, or unsafe, with a concrete
failure scenario. Material objections and their resolutions are recorded
below; editorial nits are fixed silently.

## Objection log

| # | Filed | Objection | Resolution |
|---|-------|-----------|------------|
| — | — | none filed yet | — |

## After the window

If the window closes with all material objections resolved, the editors
promote ICP-1.0 to **Final**: the status line flips, the document is
renamed `ICP-1.0.md`, and conformance certification opens. If a material
objection requires a normative change, the change is applied, the
conformance suite is patched in the same commit, and the window restarts.
