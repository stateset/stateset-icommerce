# ADR-0002: Embedded SQLite as the Default Backend

- Status: Accepted
- Date: 2026-02-05

## Context

The product targets edge devices, single-tenant deployments, and developers who want a "drop-in" commerce engine without external infrastructure. We needed to choose a default storage backend.

Options considered:

1. **PostgreSQL-first** — Production-grade, but requires a running server and connection configuration. Onboarding friction is high.
2. **In-memory only** — Zero setup, but no persistence. Useless for real commerce.
3. **SQLite-first** — Single file, zero configuration, included in the binary. Portable, fast, well-tested.
4. **DynamoDB / cloud-native** — Requires cloud credentials and internet connectivity. Incompatible with the embedded, local-first philosophy.

## Decision

Make SQLite the default backend with zero configuration. PostgreSQL remains available via a feature flag and builder configuration for production scale and multi-instance deployments.

```rust
// Zero config — just a file path
let commerce = Commerce::new("commerce.db")?;

// Or in-memory for testing
let commerce = Commerce::new(":memory:")?;

// PostgreSQL when you need it
let commerce = Commerce::with_postgres("postgres://localhost/stateset")?;
```

## Consequences

**Positive:**
- Fast local setup — `npm install` + `stateset-init --quickstart` creates a working commerce engine in seconds
- A single binary can run end-to-end without Docker, Kubernetes, or cloud services
- SQLite WAL mode provides excellent read concurrency
- Database is a single file — easy to backup, copy, version control, or embed in a container
- Latency is sub-millisecond for most operations (no network round-trip)

**Negative:**
- SQLite supports only one writer at a time — not suitable for high-concurrency web servers
- Some advanced PostgreSQL features (JSONB queries, full-text search with `tsvector`, materialized views) aren't available
- Switching from SQLite to PostgreSQL requires changing the initialization code (but no schema changes — the migration system handles both)

## Migration Path

The migration from SQLite to PostgreSQL is straightforward:

1. Change `Commerce::new("file.db")` to `AsyncCommerce::connect("postgres://...")`
2. Run migrations (automatic on first connection)
3. Export/import data if needed

The API surface is identical — all domain operations work the same way on both backends.

## When to Use PostgreSQL

Switch to PostgreSQL when you need:
- Multiple concurrent writers (web server with many requests)
- Multi-instance deployment behind a load balancer
- Advanced query capabilities (JSONB, full-text search)
- Managed database with automatic backups and point-in-time recovery

See [Async vs Sync](../guides/async-vs-sync.md) for a detailed comparison.
