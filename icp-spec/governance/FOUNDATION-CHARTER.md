# ICP Foundation — Charter (Draft)

**Status:** Draft for circulation among prospective founding members.
**Date:** 2026-05-11
**Form:** Delaware 501(c)(6) trade association (proposed) OR Swiss Verein
(alternative; see §10).

This charter defines the governance, IP, finance, and operational
structure of the Intelligent Commerce Protocol Foundation ("the
Foundation"). It is the legal vehicle through which the Intelligent
Commerce Protocol (ICP) spec is maintained as a vendor-neutral standard
after ICP-1.0 ratification.

The charter is intentionally **explicit about anti-capture provisions**.
Founding members are signing on for a vendor-neutral standard, not a
StateSet product wrapped in a foundation. The provisions below are the
structural guarantee of that.

## 1. Purpose

The Foundation exists to:

1. **Steward the ICP specification.** Approve ICPIPs, manage the
   spec-version pipeline, publish authoritative specifications.
2. **Operate the conformance program.** Maintain `icp-conformance`,
   certify implementations, publish the public conformance dashboard.
3. **Maintain the Settler allowlist.** Define operational standards for
   value-custody operators, certify Settlers, manage removals.
4. **Convene the ecosystem.** Annual ICP Summit, working groups,
   public mailing lists, security disclosures.

The Foundation does **NOT**:

- Operate any production infrastructure (no Settler, no handler, no RPC).
- Hold or transfer value.
- Issue tokens, securities, or any financial instruments.
- Compete with members.
- Earn revenue beyond the sources in §6.

## 2. Membership

### 2.1 Tiers

| Tier | Annual dues | Voting | Notes |
|---|---|---|---|
| **Founding** | $250,000 | 1 board seat | First 9 entities; dues credit toward Foundation operations and audit grants |
| **Steward** | $150,000 | Working-group seat | Up to 18 organizations |
| **Adopter** | $25,000 | Mailing list, ICPIP comment rights | No cap |
| **Implementer** | $0 (in-kind) | Mailing list, conformance submission | Independent ICP implementations, academic, OSS |

Tier eligibility is by entity type and contribution, not by employee
count or revenue. A small commerce platform implementing ICP can be a
Founding member at the same dues level as a large one.

Dues are pro-rated for the first 12 months. Members may upgrade tier at
any time on the new tier's dues schedule.

### 2.2 Founding-member criteria

A founding member is an entity that:

1. Pays Founding dues for at least 2 years up-front (refundable if
   Foundation dissolves within 24 months for any reason).
2. Names one technical reviewer to the ICP Core Working Group.
3. Signs the standard IP Policy (§5).
4. Commits to good-faith review of ICPIPs published during membership.

The Foundation accepts up to 9 founding members, then closes the tier.
Subsequent members join as Stewards regardless of dues willingness.

### 2.3 Anti-capture provisions

These are **load-bearing** clauses, not boilerplate:

1. **One board seat per parent organization.** Subsidiaries and
   affiliates count as one. A holding company's 8 subsidiaries cannot
   take 8 seats.
2. **Conflict-of-interest recusal.** A board member MUST recuse from any
   vote in which their organization has direct material interest
   (e.g. their Settler being voted onto the allowlist).
3. **Spec changes require supermajority.** Any change to ICP wire
   format, signature scheme, or state machine requires **5-of-9**
   founding-board votes AND **no veto from the spec stewardship seat**
   (see §3.1).
4. **No member > 25% of dues revenue.** If any single member's dues
   exceed 25% of annual revenue, the Foundation MUST publicly disclose
   and seek diversification.
5. **Sunset clause for any single-org control.** If 6+ board members
   represent organizations subject to common control or merger, the
   Foundation MUST publicly notice and call a re-election within 90
   days.

## 3. Board structure

### 3.1 Composition

- **9 board seats** in steady state. Below 9 during bootstrap.
- **1 spec stewardship seat** held by StateSet, Inc. for the first 24
  months post-incorporation, then put to election. This seat has:
  - One vote on all board matters (NOT supervoting).
  - A 30-day suspensive veto on any ICPIP merging into a Final state
    (see §3.4), reversible by 7-of-9 board override.
- **8 elected seats**, one per founding member (or appointee).
- Elections held annually; seats are 2-year terms staggered.

### 3.2 Officers

- **Chair** — elected by board from board, 1-year term.
- **Treasurer** — elected by board, 1-year term, signs financials.
- **Secretary** — elected by board, 1-year term, maintains records.
- **Spec Editor-in-Chief** — appointed by board from the ICPIP Editor
  pool, manages ICPIP process, no vote.

### 3.3 Quorum and voting

- Quorum: 5 of 9.
- Default vote: simple majority of present.
- Spec changes (§2.3 #3): 5-of-9 affirmative.
- Charter changes: 7-of-9 affirmative AND 30-day public notice.
- Settler allowlist additions/removals: 5-of-9.
- Foundation dissolution: 7-of-9 AND 90-day public notice.

### 3.4 Spec stewardship veto

The spec stewardship seat (currently StateSet) holds a 30-day suspensive
veto on any ICPIP advancing to **Final** state. This is a check against
spec damage during the bootstrap period, not a control mechanism. It
expires at the 24-month mark or earlier by 7-of-9 board override.

## 4. Working groups

Standing working groups, each chaired by a board member, open membership
to all member tiers:

- **Core** — protocol semantics (intent verbs, escrow, settlement)
- **Settlers** — Settler interface, allowlist, POR standards
- **Compliance** — KYB/KYC/AML/Travel Rule integration patterns
- **Cryptography** — signature schemes, PQC migration, replay protection
- **Ecosystem** — SDKs, conformance, dashboards, developer relations
- **Security** — disclosure process, audit coordination

Working groups produce ICPIPs for board approval. Quorum: 3 active members.

## 5. Intellectual property policy

### 5.1 Spec & schemas

- **Specification prose** (markdown, ICP-1.0-DRAFT.md and successors):
  CC-BY-4.0
- **Schemas, test vectors, conformance suite, code**: Apache-2.0
- All contributions to Foundation-owned artifacts are made under these
  licenses.

### 5.2 Patents

Each member grants a **royalty-free, irrevocable, worldwide patent
license** to all other implementers for any patent claims that read on
necessary implementation of an ICP specification at the version
adopted at the time of grant.

A member that withdraws revokes only the patent license for ICP versions
adopted **after** the withdrawal date. Versions in production at the
time of withdrawal retain their license.

This is the standard W3C-style RF patent grant. The Foundation does
NOT take ownership of member patents.

### 5.3 Trademarks

The Foundation holds:
- "Intelligent Commerce Protocol" wordmark
- "ICP-N.M Conformant" certification mark
- Logo (TBD)

Use of "ICP" alone is reserved for the Foundation. Members may use
"ICP-N.M Conformant" only with valid conformance certification.

## 6. Finances

### 6.1 Sources of revenue

- Member dues (§2.1)
- Strategic grants from members (no preferential governance attached)
- Ethereum/Solana ecosystem grants for open-source spec development
- Conformance certification fees (members: included; non-members: $5k
  per certification cycle)
- Subsidies for ICP Summit (sponsorships)
- **Excluded:** product revenue, infrastructure operation, value
  capture, token issuance

### 6.2 Use of funds (24-month plan)

| Category | $ allocation | Rationale |
|---|---|---|
| Foundation operations (legal, admin, accounting) | $1,000,000 | Recurring; non-discretionary |
| Spec maintenance + Editor compensation | $800,000 | Two full-time editors + working-group ops |
| Conformance program | $400,000 | Dashboard, test infrastructure, certification ops |
| Reference impl maintenance grants | $1,000,000 | Distributed to top 2-3 conformant implementations |
| Audit grants (Trail of Bits, OZ Diligence) | $600,000 | First contract + first crypto-protocol audits |
| Ecosystem grants (SDKs, integrations) | $1,500,000 | Saleor, Medusa, plugin developers |
| Reserves (12 months runway) | $700,000 | Bank-account buffer |
| **Total** | **$6,000,000** | |

### 6.3 Sources (24-month plan)

| Source | $ expected | Notes |
|---|---|---|
| 6 founding members × $250k × 2 years = $3M | $3,000,000 | Below the 9-member cap; minimum threshold |
| 12 steward members × $150k × 2 years = $3.6M | $3,600,000 | Conservative |
| 50 adopters × $25k × 2 years = $2.5M | $2,500,000 | Optional but achievable |
| Foundation/ecosystem grants | $1,000,000 | Probabilistic |
| **Total inflow** | **$10,100,000** | Surplus reserve |

The math is "$6M needed, $10M plausible inflow." Headroom for delayed
member commitments and unexpected costs.

### 6.4 Financial controls

- Annual independent audit (Big-4 or equivalent).
- Quarterly financial statements to members.
- Treasurer + Chair joint signature for any disbursement > $50k.
- Board approval for any disbursement > $200k.
- Reserves invested only in US Treasury bills or insured money-market
  accounts.

## 7. ICPIP process

The ICPIP process is defined in `governance/ICPIP-process.md`. The
Foundation:

1. Confirms ICPIP Editor appointments (board vote).
2. Provides the public review forum.
3. Maintains the canonical ICPIP repository.
4. Promotes ICPIPs through Draft → Review → Last Call → Final.

Material changes to the ICPIP process itself require a Meta-ICPIP and
a 7-of-9 board vote.

## 8. Conformance program

Defined in `icp-conformance/README.md`. The Foundation:

1. Operates the public conformance dashboard.
2. Reviews implementation submissions on a rolling basis.
3. Issues certifications for the current ICP-N.M version.
4. Revokes certifications upon failed re-test (annual + after spec minor).
5. Publishes a clear deprecation timeline for older spec versions.

Non-member implementations may submit. Certification fees are $5k for
non-members.

## 9. Dissolution

The Foundation MAY dissolve by 7-of-9 board vote with 90-day public
notice. On dissolution:

1. Specifications and conformance suite transfer to a successor
   non-profit selected by 7-of-9 board, or to the Software Freedom
   Conservancy by default.
2. Trademarks transfer to the same successor.
3. Reserves first cover wind-down costs, then refund unfulfilled member
   dues pro-rated, then donate to the Apache Software Foundation.
4. No member receives any residual asset.

## 10. Alternative form: Swiss Verein

If Delaware 501(c)(6) status is delayed by IRS review, the Foundation
incorporates as a **Swiss Verein** (association under Swiss Civil Code
Art. 60ff). All charter provisions apply equivalently; tax filings
shift to Switzerland.

The Verein option also better suits a foundation with members in
multiple jurisdictions, as it avoids the US-tax-resident-board
constraint.

A subset of the bootstrap board will determine which form to pursue at
T-30 days from incorporation, based on member jurisdictional mix and
IRS responsiveness.

## 11. Signatures

This document is a **draft** circulated for legal review and feedback.
It becomes the **founding charter** upon execution by 5+ founding
members and filing with the chosen jurisdiction.

| Founding member | Authorized signatory | Date |
|---|---|---|
| StateSet, Inc. | _____________________ | _____ |
| _____________________ | _____________________ | _____ |
| _____________________ | _____________________ | _____ |
| _____________________ | _____________________ | _____ |
| _____________________ | _____________________ | _____ |

---

*Prepared by StateSet, Inc. as interim spec steward. Legal counsel:
TBD (founding members may co-counsel). This document is a draft and is
not legally binding until executed.*
