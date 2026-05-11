# ICP — Risk Register

Honest assessment of what could prevent ICP from achieving its mission.
Maintained in the public spec repo because partners need to see we've
thought about these, and an opaque risk register would itself be a red
flag.

## Risk taxonomy

Each risk has:
- **Severity (1–5)** if realized, on a "annoying"→"protocol-killing" scale.
- **Likelihood (1–5)** of materializing in the 36-month horizon.
- **Score** = Severity × Likelihood.
- **Mitigation** — concrete steps either taken or planned.
- **Tripwire** — observable signal that the risk is materializing.

## 1. Strategic risks

### 1.1 Big-tech absorption of the lifecycle layer

| | |
|---|---|
| Severity | 5 — ICP becomes redundant |
| Likelihood | 3 — AP2 and ACP have roadmap latitude to expand |
| Score | 15 |
| Mitigation | Aggressive composition framing in PROTOCOL-RFC.md; co-authored 1-pagers per protocol pair; open spec under permissive licenses prevents proprietary forks from gaining ground |
| Tripwire | AP2 v2 or ACP v2 announcement includes escrow + dispute + settlement-receipt primitives |
| Status | Active; monitoring |

### 1.2 Spec capture by a single member

| | |
|---|---|
| Severity | 4 — Loses neutrality, becomes a single-vendor protocol |
| Likelihood | 2 — Charter anti-capture provisions are strong |
| Score | 8 |
| Mitigation | One board seat per parent org; supermajority for spec changes; sunset on common-control concentration; conflict-of-interest recusal |
| Tripwire | Any founding member tries to negotiate >1 board seat OR proposes a charter amendment weakening anti-capture |
| Status | Designed-out |

### 1.3 Adoption frost

| | |
|---|---|
| Severity | 4 — Spec is correct but unused |
| Likelihood | 4 — Default outcome for any new protocol |
| Score | 16 |
| Mitigation | Design-partner program (discounted dues for first 5 commerce platforms); pre-built Saleor/Medusa plugins; reference implementations in 3+ languages; aggressive transcript-quality demos |
| Tripwire | After Foundation incorporation, no Tier-1 commerce platform integrates within 12 months |
| Status | Highest unmitigated risk |

## 2. Technical risks

### 2.1 Signature scheme break

| | |
|---|---|
| Severity | 5 — Existing volume becomes unverifiable |
| Likelihood | 2 — Ed25519 has held for a decade; pre-quantum |
| Score | 10 |
| Mitigation | Spec already allows hybrid `ed25519+ml-dsa-65`; migration is a minor bump; PQC keys carriable from day 1 |
| Tripwire | Practical attack on Ed25519 announced; cryptanalytic break of EdDSA |
| Status | Mitigated by hybrid signature provision |

### 2.2 Canonicalization divergence

| | |
|---|---|
| Severity | 4 — Cross-impl signatures fail |
| Likelihood | 2 — Conformance vector 02 catches this |
| Score | 8 |
| Mitigation | Canonicalization.md normative; conformance vector 02 with 11 sub-cases; CI gate on cross-IUT determinism |
| Tripwire | New impl reports inability to match expected canonical bytes |
| Status | Mitigated |

### 2.3 Settler-side critical bug or compromise

| | |
|---|---|
| Severity | 5 — Loss of escrowed funds |
| Likelihood | 3 — Custodial software is hard |
| Score | 15 |
| Mitigation | Multi-sig contract admin; 48-hour timelock; counterparty exposure caps; proof-of-reserves attestation; auditor review before allowlist inclusion; non-upgradeable contracts (current version) |
| Tripwire | POR attestation arithmetic fails; auditor finds critical pre-allowlist; on-chain anomaly |
| Status | Mitigated by Settler interface design + audit gate |

### 2.4 Quote-binding bypass

| | |
|---|---|
| Severity | 4 — Buyer agents could be over-charged |
| Likelihood | 2 — Spec §11.4 + conformance suite covers this |
| Score | 8 |
| Mitigation | `max_total` is a MUST NOT ceiling; reference handler enforces; conformance suite will add a vector for this in future ticks |
| Tripwire | A non-conformant Settler ignores `max_total`; reported via conformance dashboard |
| Status | Designed-out at protocol level |

## 3. Regulatory risks

### 3.1 Travel Rule expansion to agent transactions

| | |
|---|---|
| Severity | 3 — Compliance overhead at the Settler edge |
| Likelihood | 4 — FATF guidance is trending this way |
| Score | 12 |
| Mitigation | Settler interface abstracts compliance; non-compliant Settlers removed from allowlist; KYB at the Principal level (not per-Intent) |
| Tripwire | FATF or US Treasury issues agent-specific Travel Rule guidance |
| Status | Architected for; ready to comply |

### 3.2 SEC or CFTC reclassification of agentic value transfer

| | |
|---|---|
| Severity | 4 — Could ban certain Settler types in the US |
| Likelihood | 3 — Agentic-AI regulation will arrive eventually |
| Score | 12 |
| Mitigation | Foundation is non-profit, non-revenue; Settlers are independent legal entities responsible for their own compliance; charter allows quick allowlist updates |
| Tripwire | Enforcement action against any Settler operating ICP-conformant rails |
| Status | Monitored; Settler diversity mitigates |

### 3.3 EU/UK MiCA-style restrictions on agent-native commerce

| | |
|---|---|
| Severity | 3 — Limits EU adoption |
| Likelihood | 3 — Regulatory frameworks are mid-evolution |
| Score | 9 |
| Mitigation | Swiss Verein form for the Foundation if needed; EU-native Settlers can implement local Travel-Rule patterns |
| Tripwire | MiCA II or EU AI Act guidance explicitly restricting protocol-level agent commerce |
| Status | Monitored |

### 3.4 OFAC sanctions exposure via Settler

| | |
|---|---|
| Severity | 3 — Aggravated case: Foundation entity itself sanctioned |
| Likelihood | 2 — Foundation never holds value |
| Score | 6 |
| Mitigation | Foundation never custodies; Settlers responsible for OFAC screening at their boundary; allowlist excludes any Settler that fails OFAC criteria |
| Tripwire | Any Settler in the allowlist appears on OFAC SDN list |
| Status | Architected-out at Foundation level |

## 4. Operational risks

### 4.1 Single-author bus factor

| | |
|---|---|
| Severity | 4 — Spec stewardship paralysis |
| Likelihood | 2 — Healthy; addressable by Foundation incorporation |
| Score | 8 |
| Mitigation | ICPIP Editor diversification; Foundation board appointments; reference impl maintenance grants |
| Tripwire | StateSet steward seat unfilled for >30 days |
| Status | Active; planned Foundation transition |

### 4.2 Reference impl maintenance lapse

| | |
|---|---|
| Severity | 2 — Bad for adoption signal |
| Likelihood | 2 — Foundation funding covers this |
| Score | 4 |
| Mitigation | $1M of 24-month budget allocated to reference impl maintenance grants |
| Tripwire | CI red for >30 days on any impl |
| Status | Mitigated |

### 4.3 Foundation insolvency

| | |
|---|---|
| Severity | 3 — Spec stewardship gap |
| Likelihood | 1 — Conservative $6M / $10M plan |
| Score | 3 |
| Mitigation | 12-month reserves; treasurer+chair joint signature for large disbursements; annual audit |
| Tripwire | Reserves dip below 6 months runway |
| Status | Mitigated by financial controls |

## 5. Competitive risks

### 5.1 Coinbase / Stripe / Circle launches their own competing protocol

| | |
|---|---|
| Severity | 4 — Splits adoption surface |
| Likelihood | 3 — Plausible if our outreach lands too late |
| Score | 12 |
| Mitigation | Outreach playbook prioritizes these three; composition framing gives them adoption-without-fork path; Settler interface lets them be Settler operators without owning the spec |
| Tripwire | Any of these three announces an "agent commerce protocol" without first reviewing ICP |
| Status | Active; 90-day outreach window |

### 5.2 Crypto-native protocol (anchored on a specific chain) gains traction first

| | |
|---|---|
| Severity | 3 — Chain-locked alternative emerges |
| Likelihood | 3 — Solana / Base ecosystems are fast |
| Score | 9 |
| Mitigation | ICP is rail-agnostic by design; a chain-anchored competitor is strictly less general; multi-rail Settler interface is competitive moat |
| Tripwire | A Solana or Base team launches an "agent commerce L2" within 6 months |
| Status | Monitored |

## 6. Total exposure summary

| Severity tier | Count | Notes |
|---|---|---|
| Score 12+ (red) | 4 | All actively mitigated; tripwires set |
| Score 8–11 (yellow) | 6 | Designed-out or under monitoring |
| Score 4–7 (green) | 2 | Tolerated |
| Score 1–3 (low) | 1 | Tolerated |

The 4 red-tier risks share a structural mitigation: **a credible Foundation
with active outreach.** If the Foundation incorporates with 5+ Tier-1
founding members in 2026-Q3/Q4, three of the four red risks drop
substantially.

## 7. Review cadence

This register is reviewed and updated at every ICPIP Final ratification
and at least annually by the Foundation Risk Committee (a subcommittee
of the board to be formed post-incorporation).

Members and partners who identify a risk not on this list are encouraged
to submit it via PR to this document.
