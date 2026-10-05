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

## Shipment contents and audit

Shipment items require a positive quantity and non-empty SKU and name. This
validation applies to individual creation, batch creation, and later item additions.
Contents may be changed while a shipment is pending, processing, or on hold; they
are frozen once ready to ship and in all subsequent or cancelled states.

Adding or removing an item locks the parent shipment, increments its version, and
records `shipments.item_added.v1` or `shipments.item_removed.v1` in the transactional
outbox. Each fact includes the item contents and old/new shipment versions.
Creation facts include the initial items. If writing the fact fails, both the item
change and parent update roll back. Removing a missing item returns `NotFound`.
An item change also invalidates a previously read `expected_version` for a shipment
update, allowing packing completion to detect concurrent content changes.

Every new item is resolved to a line of the shipment's order. A missing
`order_item_id` is accepted only for an unambiguous SKU; supplied product IDs must
match that line. Creation and additions enforce a shared per-line manifest budget
across all non-cancelled shipments. PostgreSQL serializes allocations on the order
row; SQLite uses an immediate transaction. Atomic batches lock orders in a stable
order. Cancelled manifests release capacity while retaining their history.

Legacy items without references count against the budget when their SKU is
unambiguous. Ambiguous or inconsistent legacy data requires reconciliation before
new allocation. Order deletion is refused while shipment history exists, and line
removal is refused while a shipment item references that line, including cancelled
manifests.

These are tracking records. Their budget is the ordered quantity; fulfilled units
are not subtracted a second time. Allocations do not reserve stock or update
physical fulfillment. Automatic partial-shipment recovery still requires atomic
reconciliation with those records and durable request idempotency.

Tracking event appends also lock and version the parent. The event, parent update,
and `shipments.event_added.v1` outbox fact commit together; audit failure or version
exhaustion rolls back all three. The fact contains the full stored event and both
shipment versions. Previously read `expected_version` values become stale after
an append. Event types must contain non-whitespace text and fit in 100 characters;
locations fit in 255 characters. All event text rejects NUL characters. Both
backends count Unicode characters rather than UTF-8 bytes. Missing shipments
return `NotFound`.

Late observations may be appended to delivered or cancelled shipments. Their
historical `event_time` never changes lifecycle status, shipment milestones, or
order fulfillment; `updated_at` records ingestion time. New event timestamps use
microsecond precision on both backends, including returned values and audit facts.
Each call appends a distinct observation, so provider delivery deduplication must
be handled by the caller. This native tracking API does not certify a carrier's
claims or provide governed command receipts.

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
