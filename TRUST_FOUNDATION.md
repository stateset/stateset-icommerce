# Trust Foundation

Status: active repo policy
Last reviewed: 2026-09-04
Applies to: `stateset-icommerce` and the adjacent trilogy repos referenced from this workspace

This document is the canonical trust inventory for this repository.

It separates claims into three buckets:

- `implemented in this repo`
- `documented for adjacent repos`
- `planned / open gap`

If a capability does not fit one of those buckets, it must not be marketed as generally available.

## Scope

This workspace is the application-layer repo in a larger documented stack:

- `stateset-icommerce`: commerce engine, bindings, CLI, MCP, A2A
- `stateset-sequencer`: ordering, receipts, commitments, x402 batch processing
- `stateset-stark`: STARK proving and verification
- `set`: settlement chain, registry contracts, anchor service

The current workspace release line is `1.31.0`.

## Trust Levels

| Level | Meaning | Dependency | Status |
|-------|---------|------------|--------|
| `local acceptance` | The local runtime accepted the operation and persisted it | application policy, local DB, local runtime | implemented in this repo |
| `pq soft finality` | Hybrid or PQ receipt plus local proof and payload-binding validation | PQC migration, signed receipts, verifier discipline | documented migration target, partially implemented |
| `classical hard finality` | A batch commitment is anchored on an EVM-compatible chain and can be independently checked | sequencer, anchor service, SET Chain / EVM substrate | documented for adjacent repos |
| `pq hard finality` | Hard finality with quantum-resistant anchoring or attestations | new PQ anchoring substrate, PQ transparency log, or PQ bridge | open gap |

The current PQC specification is explicit about this boundary:

- `pq soft finality` is defined.
- `classical hard finality` is defined.
- `pq hard finality` is not yet defined.

That means the stack can make a truthful PQ migration claim only up to the soft-finality boundary today.

## Residual Classical Dependencies

The repo and its companion specs still carry classical dependencies that must be stated plainly:

- EVM anchoring remains a classical hard-finality mechanism.
- The exact-EVM x402 path remains shaped around classical/EVM settlement semantics.
- The STARK proof plane is PQ-friendly, but it does not replace signature, KEM, receipt, or chain-finality layers.

Do not collapse those into a single "post-quantum" claim.

## Compatibility Contract

The current workspace release line is `1.31.0`, so the published artifacts are
on the first stable `v1.x` compatibility line.

The `v1.0.0` compatibility contract is frozen and remains active:

- patch releases in `v1.x` are non-breaking bug, security, performance, and
  documentation updates;
- minor releases in `v1.x` are additive for the documented stable surfaces;
- stable surfaces for `v1.x` are the curated Rust SDK and embedded preludes,
  language binding version line, MCP tool names and schemas, CLI flags, policy
  YAML, and additive SQLite migrations;
- deprecations require runtime warnings and documentation updates, and remain
  supported for at least two minor releases and 90 days before removal in the
  next major;
- `v1.0.x` is the initial stabilization/LTS line: critical regressions and
  security fixes are eligible for backport there until `v1.31.0` ships;
- after `v1.31.0`, the latest `v1.y` and previous `v1.(y-1)` lines receive
  security and release-blocking bug backports.

## Evidence Matrix

| Area | Status | Evidence in or from this repo | What is still missing |
|------|--------|-------------------------------|-----------------------|
| PQC control-plane migration | partially implemented / documented | `docs/PQC_INITIAL_SPEC.md` defines hybrid and strict PQ profiles and rollout phases | public hard-finality design, x402 PQ authorization path, public implementation evidence across all components |
| Finality model | documented | `docs/PQC_INITIAL_SPEC.md` and trilogy docs distinguish soft vs hard finality | `pq hard finality` definition and deployment plan |
| Security disclosure process | implemented in this repo | `SECURITY.md` defines reporting channel and response targets | public advisory index, CVE process, bug bounty |
| Independent security audits | open gap | no public third-party audit reports are linked from this repo | published audit reports and remediation tracking |
| Formal verification | open gap | no Lean, Coq, or machine-checked proof artifacts were located in this workspace | AIR proofs, ordering proofs, PQ composition proofs |
| Versioning and deprecation policy | implemented policy | `docs/src/versioning.md` and `RELEASING.md` define the `v1.0` contract, deprecation window, and backport rules | GitHub branch protection and release permissions live outside this repo |
| Observability primitives | implemented in this repo | `crates/stateset-observability`, `deploy/grafana`, `deploy/prometheus` | published SLOs, runbooks, correlated production dashboards, chaos results |
| Bench harness and perf gates | implemented in this repo | `crates/stateset-benches`, `perf-gates.json` | published benchmark report with hardware, workload, and repeatable methodology |
| A2A discovery and reputation data model | implemented in this repo | agent-card, identity, and reputation traits and models exist in core crates | standalone normative spec, conformance suite, multi-language reference agents |
| Economic agent identity and authority | implemented for operator-owned runtime configuration | `EconomicAgent` binds principal, role, scope, capabilities, credentials, keys, and budgets; `EconomicAuthority` compiles autonomous/approval/deny tiers into kernel policy | external organizational identity resolution, revocation distribution, hosted credential lifecycle |
| Portable economic receipts | implemented for local execution and reference ICP co-signing | `EconomicReceipt` binds canonical result hashes, policy decisions, commitments, audit anchors, settlement evidence, and independent Ed25519 signatures; the two-sided demo verifies merchant and Settler signatures | hosted key registry, public transparency service, hard-finality anchor integration |
| Canonical transaction vocabulary | implemented as a framework-neutral intent facade | `quote/buy/sell/pay/fulfill/return/refund/subscribe` create scoped `EconomicIntent` values; governed adapters execute the currently mapped domain commands | governed executor coverage for every verb as one atomic orchestration |
| Two-agent purchase demonstration | executable reference implementation | `icp-spec/examples/03-two-sided-flow` exercises delegated intent, quote ceiling, inventory reservation, order state, mock rail lifecycle, verified co-signing, and tamper rejection | replace in-memory merchant state and mock chain injection with embedded production state and a live rail |
| Regulatory and compliance posture | partially documented | compliance, GDPR, AML, and audit guidance exist in docs | jurisdiction-by-jurisdiction mapping, legal opinions, hosted-service controls, code-verified tool inventory |
| Hosted control environment | open gap in this repo | no SOC 2 report, DPA, retention schedule, or control matrix is linked from this workspace | SOC 2 Type II, retention and deletion map, evidence collection policy |

## Documentation Integrity Findings

This repo had documentation drift that directly affected trust, and some of it still exists in historical materials:

1. Hard-coded MCP tool counts diverged across top-level docs.
   The live README and mdBook surfaces have been normalized in this pass, but historical documents such as `docs/whitepaper.md` still contain older counts like `365+`.
2. Some mdBook pages name operational or compliance tools that were not located in a code search of this workspace on 2026-04-02.
   Examples include `a2a_health_status`, `a2a_agent_introspection`, `export_soc2_evidence`, `audit_compliance_cert`, `export_gdpr_subject_data`, `request_gdpr_erasure`, and `generate_compliance_package`.
3. Release and support docs had stale version markers relative to the workspace release line before this pass.

The operating rule from this point forward is:

- do not hard-code MCP tool counts unless they are generated from the codebase or runtime registry;
- do not present a tool as shipped unless it is discoverable in the target runtime or clearly marked as tier-specific or planned;
- keep support/version docs updated in the same change that updates the release line.

## What Can Be Claimed Today

These are defensible claims today:

- strong embedded commerce engineering in a multi-crate Rust workspace with broad bindings coverage;
- documented PQ migration strategy with explicit residual classical dependencies;
- observability primitives, benchmark harnesses, and deployment assets present in-repo;
- A2A identity, discovery, and reputation primitives in code;
- operator-owned economic agent identities and tiered authority compiled into
  fail-closed kernel policy;
- result-bound, independently co-signable economic receipts and an executable
  two-agent reference flow using a simulated rail;
- a frozen `v1.0` OSS compatibility contract with documented deprecation and backport rules;
- a defined vulnerability-reporting process.

These are not yet defensible as shipped, globally trustworthy claims:

- PQ hard finality
- formal verification
- public third-party audit coverage
- SOC 2 readiness or hosted control assurance
- exact, stable MCP tool counts across all surfaces
- universal availability of every tool named in the docs

## Immediate Trust Priorities

1. Publish third-party audit scope, reports, and remediation history.
2. Turn benchmark and observability scaffolding into public operating evidence.
3. Split A2A into a standalone normative spec with conformance tests.
4. Define and publish the `pq hard finality` design before making institutional-grade PQ claims.
5. Replace hand-maintained tool-count marketing with generated inventories.

## Non-Negotiable Selling Rules

- Do not claim `pq hard finality`.
- Do not claim formal verification.
- Do not claim public audit coverage until reports are linked.
- Do not claim SOC 2 posture unless a hosted control environment and evidence package exist.
- Do not claim exact MCP tool counts unless they come from a generated inventory.
- Do not market doc-only compliance or observability interfaces as shipped GA features.
