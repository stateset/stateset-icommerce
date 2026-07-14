# Trust Foundation

This page summarizes the current trust posture of the repo.

The canonical repo-native inventory lives in `TRUST_FOUNDATION.md` at the repository root.

## Current Posture

- Current workspace release: `1.7.0`
- Current release status: stable `v1.x`; the `v1.0.0` compatibility contract remains active
- This repo is one layer in a larger documented stack with adjacent `stateset-sequencer`, `stateset-stark`, and `set` repos

## What Is Implemented Here

- embedded commerce engine crates, bindings, CLI, MCP, and A2A primitives
- observability primitives and deployment assets
- benchmark harnesses and perf gates
- versioning, deprecation, and `v1.0` backport policy
- vulnerability reporting process

## What Is Documented But Not Fully Closed

- PQ migration strategy and finality taxonomy
- sequencer and chain-level hard-finality story
- regulatory and compliance posture
- operational control surfaces for enterprise deployments

## What Remains Open

- `pq hard finality`
- public third-party audit reports
- formal verification artifacts
- standalone A2A conformance suite
- public benchmark and SLO evidence

## Operating Rules

- Treat only code-discoverable or runtime-discoverable interfaces as shipped.
- Treat hand-maintained tool counts as provisional unless generated from code.
- Treat `v1.0.0` as a stable OSS API contract, not as shorthand for audit, PQ hard-finality, or hosted-control claims.

## See Also

- [Versioning](versioning.md)
- [Security Architecture](security/architecture.md)
- [The StateSet Trilogy](trilogy/overview.md)
- [Sequencer](trilogy/sequencer.md)
- [STARK Compliance Proofs](trilogy/stark-proofs.md)
- [Observability & Telemetry](guides/observability.md)
- [Compliance & Audit](advanced/compliance.md)
