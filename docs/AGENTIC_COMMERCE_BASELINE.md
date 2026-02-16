# Agentic Commerce Baseline Review

## Objective

This document identifies what is already working for StateSet today and what is still missing to make it the reference platform for **first-class Agentic Commerce**.

---

## What is already in place

- Embedded commerce execution in-process (SQLite/Postgres first-class).
- Strong MCP tooling surface (`stateset` + `stateset-x402-mcp`) with explicit policy hooks.
- Policy runtime with evaluatable rule sets, deny/allow/transform actions, and history.
- A2A primitives: payments, disputes, subscriptions, splits, webhooks, evidence, and reputation.
- Verifiable sync primitives for eventually consistent multi-agent state.
- Multi-channel runtime and governance scaffolding (permissions, hooks, approvals, sessions).

---

## What is still missing for a viral baseline

1. **Deterministic replay + simulation**
   - Agents need to run proposals that can be replayed exactly before execution.
   - Current flow has `--apply` preview in CLI mode, but tool-level deterministic replay across MCP calls is not yet standardized.

2. **Execution governance package**
   - Policy currently blocks/permits actions, but missing composable “decision bundles” that can require:
     - explicit proof artifacts,
     - staged approvals,
     - auto-rollback instructions,
     - signed audit bundles.

3. **Global trust layer**
   - Trust is split across policy + telemetry + approvals, but there is no universal trust contract for:
     - reputation portability between agents,
     - on-chain or verifiable attestations of action intent,
     - standardized fraud/risk gates per merchant segment.

4. **Agent network layer**
   - ACP is positioned, but there is not yet a widely deployed marketplace of capabilities, intents, and routing policy to make discovery/rematching automatic.

5. **Payment intent + settlement as protocol-level primitive**
   - `x402` exists, but settlement and payment intent governance should be first-class in the same policy graph used by commerce operations.

6. **Cross-tool semantic identity**
   - Tool namespaces and domains are improving, but richer capability metadata (idempotency keys, compensation hooks, side-effect descriptors) is needed for safe autonomous orchestration at scale.

7. **Viral adoption UX**
   - Onboarding still assumes high context. Missing:
     - one-command demo environment,
     - opinionated starter policies,
     - benchmarked templates for B2B, DTC, marketplaces, and procurement.

---

## Why “agentic commerce” is still unresolved in the market

- Most platforms optimize for human dashboards, not machine-first autonomous loops.
- Existing AI integrations are shallow wrappers around existing SaaS APIs.
- Few stacks combine:
  - deterministic state transitions,
  - policy as mandatory control plane,
  - economic/payment primitives,
  - and cross-agent trust contracts.

StateSet has the right direction; the missing gap is “**enterprise-grade autonomous execution safety**” at protocol scale.

---

## Baseline we should lock down first

1. Build a standardized **Agentic Commerce Runtime Contract**:
   - deterministic tool contract (`input`, `idempotency`, `compensation`, `rollback`, `read-model impact`).
2. Add **policy-first execution envelopes**:
   - every high-risk tool must pass allow/deny/transform + required approvals before mutation.
3. Publish **governance tooling**:
   - policy editor, simulation mode, and signed audit export.
4. Add **intent → payment → fulfillment** closed-loop by default:
   - x402 policy binding + policy domain metadata.
5. Add **agent reputation + marketplace discovery** docs and sample runtime profiles.
6. Add **benchmark suites** for latency, safety, and policy-denial rate by domain.

---

## Immediate execution checklist (implemented today vs next)

- Implemented in this cycle:
  - Tool policy domain inference + x402 policy integration in MCP paths.
  - `stateset-x402-mcp` policy directory flag and propagation.
  - Improved x402 server formatting and stability.
- Next actions:
  - add deterministic simulation endpoint for MCP tool runs,
  - expose policy bundle simulation + approval traces,
  - add trust attestations and replay manifests.

