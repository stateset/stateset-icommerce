# ADR-0003: Event-Driven Extensions and Auditability

- Status: Accepted
- Date: 2026-02-05

## Context

Commerce systems need reliable integration points for downstream systems (analytics, fulfillment, notifications) and an audit trail for regulatory and operational requirements. Direct synchronous hooks increase latency and couple core workflows to external systems.

## Decision

Provide an event system that emits structured commerce events and supports subscriptions and webhooks. The VES (Verifiable Event Sync) layer builds on these events to provide ordered replication and cryptographic proofs for auditability.

## Consequences

- Integrations can consume events asynchronously without blocking core writes.
- The system supports verifiable, replayable histories for compliance and debugging.
- Extra operational considerations exist for event delivery and backpressure management.
