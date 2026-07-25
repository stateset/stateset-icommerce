# stateset-db

[![crates.io](https://img.shields.io/crates/v/stateset-db.svg)](https://crates.io/crates/stateset-db)
[![docs.rs](https://docs.rs/stateset-db/badge.svg)](https://docs.rs/stateset-db)

SQLite and PostgreSQL implementations of the repository traits from
[`stateset-core`](https://crates.io/crates/stateset-core).

Most users don't depend on this crate directly —
[`stateset-embedded`](https://crates.io/crates/stateset-embedded) picks the backend
for you. Reach for it when you want the storage layer without the engine's
convenience API, or to implement the same traits over your own store.

## Backends

### SQLite (default)

The reference backend, via bundled `rusqlite`. Zero external setup: the database is a
file, migrations run on open, and the whole domain surface is implemented — core
commerce, A2A, finance, manufacturing, and the V4 entities (reviews, wishlists, gift
cards, loyalty, fraud, segments, store credits, shipping zones, rewards, search
configs).

### PostgreSQL

Async backend via `sqlx`, for concurrent deployments. As of v1.17.0 it implements the
same domain surface as SQLite — every repository capability returns a real store,
verified by parity tests against a live PostgreSQL instance. The exception is vector
search, which is SQLite-only (see the `vector` feature).

One behavioral note: the synchronous repository API bridges to `sqlx` by blocking, so
calling it from inside an existing async runtime is **rejected** rather than silently
nesting runtimes and deadlocking. Async callers should use `AsyncCommerce` and the
async repository methods directly.

## Usage

```rust,no_run
use stateset_db::{SqliteDatabase, DatabaseConfig};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let db = SqliteDatabase::new(&DatabaseConfig::sqlite("./store.db"))?;
# let _ = db;
# Ok(())
# }
```

```rust,ignore
use stateset_db::{PostgresDatabase, DatabaseConfig};

let db = PostgresDatabase::connect(
    &DatabaseConfig::postgres("postgres://localhost/stateset")
).await?;
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `sqlite` | SQLite via bundled rusqlite | Yes |
| `postgres` | Async PostgreSQL via sqlx | No |
| `vector` | Vector search via the sqlite-vec extension (SQLite only) | No |
| `saga` | Experimental persisted saga coordinator (PostgreSQL only) | No |

## Error Handling

Errors are typed as `stateset_core::DbError` rather than backend-specific types, so
callers can categorize failures (not found, conflict, retryable) without knowing which
backend is underneath. The `error_helpers` module converts backend errors into that
taxonomy.

## Part of StateSet iCommerce

Sits under [`stateset-embedded`](https://crates.io/crates/stateset-embedded) and
applies the schema from
[`stateset-migrations`](https://crates.io/crates/stateset-migrations). Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
