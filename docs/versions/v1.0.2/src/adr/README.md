# Architecture Decision Records

Architecture Decision Records (ADRs) capture the "why" behind major structural choices in StateSet iCommerce. Each ADR records the context that motivated a decision, the alternatives considered, and the consequences — positive and negative — so future changes can be evaluated against the original intent.

## How to Read ADRs

Each ADR follows the format:
- **Context**: The problem or situation that required a decision
- **Decision**: What was chosen and why
- **Consequences**: What changed as a result, including trade-offs

## Index

| # | Decision | Status | Date |
|---|----------|--------|------|
| [0001](0001-layered-architecture.md) | Layered crate architecture | Accepted | 2026-02-05 |
| [0002](0002-embedded-sqlite-default.md) | SQLite as default backend | Accepted | 2026-02-05 |
| [0003](0003-event-driven-extensions.md) | Event-driven extensions and VES | Accepted | 2026-02-05 |
| [0004](0004-cli-safety-model.md) | CLI safety model (`--apply`) | Accepted | 2026-02-05 |
| [0005](0005-binding-generation.md) | Single-spec binding generation | Accepted | 2026-02-05 |

These five decisions form the architectural foundation. They are unlikely to be superseded — deviations should be evaluated against the original rationale.
