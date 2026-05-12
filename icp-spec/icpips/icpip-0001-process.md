# ICPIP-0001: The ICPIP Process

```
ICPIP:        0001
Title:        The ICPIP Process
Author:       Dom Steil <dom@stateset.com> (interim editor)
Discussions:  github.com/stateset/icp-spec/discussions (forthcoming)
Status:       Draft
Type:         Meta
Category:     Process
Created:      2026-05-12
Requires:     —
Supersedes:   —
```

## Abstract

This ICPIP describes the formal process by which the Intelligent Commerce
Protocol (ICP) is evolved post-ICP-1.0 ratification: how proposals are
authored, reviewed, advanced through statuses, and ratified into the
protocol. It is itself the first ICPIP, and is intended to be the
authoritative document for the process going forward.

The process is modeled on the **Ethereum EIP process** (EIP-1) and the
**Bitcoin BIP process** (BIP-2), adapted for ICP's specific governance
structure (the ICP Foundation, Charter §3 board, ICPIP Editors).

## Motivation

ICP-1.0 ratifies a specific wire format, error vocabulary, escrow state
machine, and verb set. After ratification, the spec must be able to
evolve without ad-hoc decisions by any single contributor — including
the original spec stewards. Without a formal process, future protocol
changes will either: (a) bottleneck on a single decision-maker, or (b)
fork into incompatible implementations. Both kill cross-implementation
interoperability, which is the load-bearing property of any commerce
protocol.

This ICPIP fixes the process. It is intentionally **slow-by-default**:
every Standards Track ICPIP must spend at least 14 days in Last Call
and must have at least two independent implementations passing the new
conformance vectors before promotion to Final. Slow ratification
protects the protocol from premature freeze.

## Specification

### Types

Every ICPIP has exactly one **Type**:

- **Standards Track** — changes affecting the wire format, signature
  scheme, state machine, or any normative behavior of an ICP
  implementation. Two sub-categories:
  - **Core** — protocol semantics (intent verbs, escrow states,
    settlement, error codes).
  - **Networking** — transport bindings (HTTP, gRPC, MCP, libp2p).
- **Meta** — process changes (this ICPIP, governance, IP policy).
- **Informational** — design notes, best practices, security analyses.
  Informational ICPIPs are NOT normative; implementations need not
  follow them. They exist to document community consensus on questions
  that don't require protocol changes.

### Statuses

The ICPIP status flow:

```
Draft ──▶ Review ──▶ Last Call ──▶ Final
                          │
                          └────▶ Withdrawn / Stagnant
```

**Draft** — Proposal authored, may evolve freely. No promises of
stability.

**Review** — Editor-confirmed format compliance; number assigned; open
for substantive community feedback.

**Last Call** — Final 14-day public comment window before ratification.
Material objections raised during Last Call MUST be resolved on-record
before promotion to Final.

**Final** — Ratified. Locked text. Becomes part of the next protocol
minor (Standards Track) or is published as accepted process / guidance
(Meta / Informational). Final ICPIPs MAY be amended only by a
superseding ICPIP that explicitly references them.

**Withdrawn** — Author retracted. Withdrawn ICPIPs MAY be reopened by
any author with permission of the ICPIP Editors and the original author
(if reachable).

**Stagnant** — No activity for 6 months. Auto-marked by editors. May be
reopened.

### Lifecycle

1. **Pre-discussion.** Idea raised on the discussions board. No format
   requirements. Editors may indicate likelihood of acceptance.
2. **Draft.** Author opens a PR adding `icpips/icpip-NNNN-<short-name>.md`.
   For an unassigned number, use `xxxx`; editors will renumber on
   acceptance into Review.
3. **Editor review.** Editors confirm format compliance (template
   conformance, scope, no duplication). Either:
   - Accepted into **Review** with a number, OR
   - Rejected with a written rationale (format, scope, duplication).
4. **Community review.** Open-ended duration. Author iterates based on
   feedback. Editors call **Last Call** when consensus appears stable
   AND (for Standards Track) at least one prototype implementation
   exists.
5. **Last Call.** 14-day fixed window. New material objections push
   the ICPIP back to Review. Trivial editorial fixes do not.
6. **Final.** Editor merges with status `Final`. For Standards Track,
   the change ships in the next minor (e.g. ICP-1.1).

### Editors

ICPIP Editors are appointed by the ICP Foundation board (Charter §3).
Editors do NOT have veto power on substantive grounds — their role is
strictly format compliance, numbering, and lifecycle management.
Editors who feel an ICPIP is technically wrong MAY register that as a
public comment; they MUST NOT block format-compliant ICPIPs from
entering Review.

Until the ICP Foundation is incorporated, editors are appointed by
StateSet, Inc. with public nomination via the discussions board.

**Spec stewardship seat veto (interim).** The spec stewardship seat
(currently StateSet) retains a 30-day suspensive veto on Final
promotion of Standards Track ICPIPs, reversible by 7-of-9 board override
(per Charter §3.4). This veto expires at the 24-month mark or by
7-of-9 board override.

### Numbering

ICPIPs are numbered in **monotonic creation order**. Numbers are
permanent — withdrawn or rejected ICPIPs do not free their number.
A new author who wants to revisit a withdrawn proposal opens a new
ICPIP that references the prior number.

Numbering ranges (advisory, not enforced):
- 1–99: bootstrap / charter / process
- 100–999: ICP-1.x evolution
- 1000–9999: ICP-2.x evolution
- ≥10000: reserved for future use

### Patent and IP policy

Every ICPIP author MUST sign the standard IP Policy (Charter §5.2):
royalty-free, irrevocable patent license for any claims that read on
necessary implementation. Co-authors and substantive contributors
MUST do the same. ICPIPs may not advance to Final until all
contributors are signed.

### Process changes

Material changes to this process require a **Meta ICPIP** that
explicitly supersedes this one (`Supersedes: 0001`). Meta ICPIPs
follow the same lifecycle.

## Rationale

### Why slow-by-default

Commerce protocols at billion-dollar scale fail catastrophically on
premature changes. Bitcoin's history is full of contentious hard
forks; SWIFT's MX migration took two decades. ICP's process is
intentionally designed to err on the side of slowness:

- 14-day Last Call: enough time for the global engineering community
  to notice and comment.
- Two independent implementations required: forces real-world
  validation before normative status.
- 30-day suspensive veto from spec stewardship: catches obviously
  wrong changes during the bootstrap period.

### Why Standards Track requires implementation

Specs without implementations encode aspirational behavior that may
not be technically achievable. Requiring a prototype implementation
before Final ensures every Final ICPIP has been built at least once.
The conformance suite (`icp-conformance/`) extends this: vectors
defined in the ICPIP must pass against ≥2 IUTs before ratification.

### Why Meta and Informational tracks exist

Some changes don't affect the wire format — they're process changes
(this ICPIP), guidance documents (security analyses, best practices),
or community consensus on non-normative questions. Conflating them
with Standards Track ICPIPs would slow them down for no benefit and
muddle the conformance discipline.

### Comparison to EIP-1 / BIP-2

The ICPIP process borrows heavily from EIP-1 and BIP-2. Key differences:

- **Mandatory implementation gate**: ICPIP-0001 requires ≥2
  implementations before Final; EIP-1 has a "Last Call" but
  implementation count is informal.
- **Spec stewardship veto**: temporary, sunset at 24 months. Neither
  EIP nor BIP has an equivalent — they're decentralized from day 1.
  ICP is launching with a single steward; the veto is a transition
  mechanism, not a permanent feature.
- **IP grant required at author level**, not just contributor level.
  This is stricter than EIP-1 and matches W3C process style.

## Backwards Compatibility

This ICPIP is the first ICPIP. It has no prior process to be
compatible with. All future Standards Track ICPIPs MUST follow this
process; future Meta or Informational ICPIPs MAY supersede it via the
mechanism in §"Process changes."

## Security Considerations

The ICPIP process itself has security implications:

1. **Stewardship veto abuse**: a steward with veto authority can
   indefinitely block valid changes by repeatedly invoking the
   30-day suspensive veto. **Mitigation**: 7-of-9 board override,
   24-month sunset.
2. **Sybil author concentration**: a single legal entity could
   author many ICPIPs under different names. **Mitigation**:
   IP policy requires identifiable signed grants; co-author
   diversity not currently mandated but tracked by editors.
3. **Race-to-Final for breaking changes**: a malicious author
   could rush a breaking ICPIP through Last Call before the
   community engages. **Mitigation**: 14-day Last Call MUST be
   honored; editors monitor for substantive late-arriving
   objections.

## Test Vectors

This ICPIP has no test vectors. Process ICPIPs don't ship
conformance tests; their conformance is procedural (i.e. did the
editors follow the lifecycle for this ICPIP).

## Reference Implementation

This document itself is the reference. There is no executable
component.

## References

- Ethereum EIP-1 (process): https://eips.ethereum.org/EIPS/eip-1
- Bitcoin BIP-2 (process): https://github.com/bitcoin/bips/blob/master/bip-0002.mediawiki
- W3C Process: https://www.w3.org/policies/process/
- ICP Foundation Charter: `../governance/FOUNDATION-CHARTER.md`
- ICPIP Process pre-1.0 sketch: `../governance/ICPIP-process.md`

## Copyright

This ICPIP is licensed under CC-BY-4.0 (the same license as the ICP
specification prose).
