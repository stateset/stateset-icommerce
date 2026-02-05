# ADR-0004: CLI Safety Model (`--apply`)

- Status: Accepted
- Date: 2026-02-05

## Context

The CLI accepts natural language instructions and can perform state‑changing operations (create, update, delete). Without safeguards, it is easy to run destructive actions by accident, especially during exploration or in automation.

## Decision

All write operations require an explicit `--apply` flag. The default mode is read‑only preview, and write intents are surfaced clearly in output. Agents are expected to validate and request confirmation for risky actions unless `--apply` is present and confirmation is skipped.

## Consequences

- Safer day‑to‑day usage and fewer accidental mutations.
- Clear separation between read‑only exploration and write execution.
- Automation must opt in to writes, which improves auditability but adds a small amount of friction.
