# Architecture

StateSet iCommerce is split into a small set of Rust crates with language bindings on top:

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/       # Domain models and business logic
│   ├── stateset-db/         # SQLite and PostgreSQL persistence
│   └── stateset-embedded/   # Unified high-level API
├── bindings/                # Language bindings (Node, Python, Ruby, etc.)
└── cli/                     # MCP server + natural language CLI
```

## Core layers

- **stateset-core** defines orders, products, inventory, and other domain types.
- **stateset-db** implements storage backends for SQLite and PostgreSQL.
- **stateset-embedded** is the primary API surface exposed to bindings and the CLI.

## Agent and CLI layer

The CLI exposes MCP tools and applies a safety model where writes require `--apply`. The agent layer builds deterministic workflows on top of the same core APIs.

## Sync and auditability

The VES sync system provides ordered event replication, conflict handling, and cryptographic proofs for auditability.
