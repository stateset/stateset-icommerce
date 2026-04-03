# Trust Foundation

This page summarizes the current trust posture of the repo.

The canonical repo-native inventory lives in `TRUST_FOUNDATION.md` at the repository root.

## Current Posture

- Current workspace release: `0.9.5`
- Current release status: pre-`1.0`
- This repo is one layer in a larger documented stack with adjacent `stateset-sequencer`, `stateset-stark`, and `set` repos

## What Is Implemented Here

- embedded commerce engine crates, bindings, CLI, MCP, and A2A primitives
- observability primitives and deployment assets
- benchmark harnesses and perf gates
- versioning and deprecation policy
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
- `1.0` stability and LTS guarantees
- standalone A2A conformance suite
- public benchmark and SLO evidence

## Operating Rules

- Treat only code-discoverable or runtime-discoverable interfaces as shipped.
- Treat hand-maintained tool counts as provisional unless generated from code.
- Treat pre-`1.0` compatibility as a policy commitment, not a final freeze.

## See Also

- [Versioning](versioning.md)
- [Security Architecture](security/architecture.md)
- [The StateSet Trilogy](trilogy/overview.md)
- [Sequencer](trilogy/sequencer.md)
- [STARK Compliance Proofs](trilogy/stark-proofs.md)
- [Observability & Telemetry](guides/observability.md)
- [Compliance & Audit](advanced/compliance.md)
