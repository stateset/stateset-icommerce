# stateset-sdk

[![crates.io](https://img.shields.io/crates/v/stateset-sdk.svg)](https://crates.io/crates/stateset-sdk)
[![docs.rs](https://docs.rs/stateset-sdk/badge.svg)](https://docs.rs/stateset-sdk)

**Start here.** The Rust facade for StateSet iCommerce — one dependency, one prelude,
feature-gated access to the commerce engine plus optional sync, crypto, policy, and
macro surfaces.

If you're writing Rust against StateSet, add this crate rather than picking sibling
crates by hand; it pins compatible versions of the whole set for you.

```toml
[dependencies]
stateset-sdk = { version = "1.37.0", features = ["full"] }
```

Or: `cargo add stateset-sdk --features full`

## Usage

```rust,no_run
# #[cfg(feature = "core")]
use stateset_sdk::prelude::*;

# #[cfg(feature = "core")]
# fn main() -> Result<()> {
let commerce = Commerce::new("store.db")?;

let customer = commerce.customers().create(CreateCustomer {
    email: "alice@example.com".into(),
    first_name: "Alice".into(),
    last_name: "Smith".into(),
    ..Default::default()
})?;
# let _ = customer;
# Ok(())
# }
# #[cfg(not(feature = "core"))]
# fn main() {}
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `core` | Primitives + Core + DB + Embedded + Observability | Yes |
| `crypto` | VES v1.0 cryptographic operations | No |
| `policy` | Declarative policy engine | No |
| `macros` | Proc macros (`StateSetId`, `GenerateDto`, `JsonSchema`) | No |
| `sync` | Outbox-driven sync engine and sequencer transport | No |
| `full` | Everything above | No |

## Sync Runtime

With `sync` enabled, `SyncRuntime` and `SyncRuntimeConfig` bundle the sync engine,
sequencer HTTP transport, and runtime auth into a single surface: config loading,
sync operations, JSON-ready snapshots, kernel receipt queries, and
confirmation/dead-letter inspection. `SyncRuntimeConfig` loads from a file, a JSON
string, or the environment via `from_file`, `from_json_str`, and `from_env`.

For durable incoming events, configure a state snapshot path, read the inbox,
commit application changes, then acknowledge the processed ids. Application
updates must deduplicate event ids because a crash before acknowledgement can
replay an already-applied event. A full inbox stops new pulls with `BufferFull`;
acknowledge processed events and resume from the saved cursor.

```rust,no_run
# #[cfg(feature = "sync")]
# mod example {
use stateset_sdk::{SyncRuntime, sync::SyncEvent};
use std::error::Error;

fn apply_pending(
    runtime: &mut SyncRuntime,
    commit: impl FnOnce(&[SyncEvent]) -> Result<(), Box<dyn Error>>,
) -> Result<usize, Box<dyn Error>> {
    let events = runtime.buffered_events();
    commit(&events)?; // Commit idempotent application updates before acknowledgement.
    let ids: Vec<_> = events.iter().map(|event| event.id).collect();
    Ok(runtime.acknowledge_buffered_events(&ids)?)
}
# }
```

## What's Underneath

| Crate | Role |
|-------|------|
| [`stateset-embedded`](https://crates.io/crates/stateset-embedded) | The engine — commerce operations over SQLite or PostgreSQL |
| [`stateset-core`](https://crates.io/crates/stateset-core) | Domain models, repository traits, validation, errors |
| [`stateset-primitives`](https://crates.io/crates/stateset-primitives) | Typed IDs, `Money`, `CurrencyCode`, `Sku` |
| [`stateset-db`](https://crates.io/crates/stateset-db) | SQLite and PostgreSQL implementations |
| [`stateset-observability`](https://crates.io/crates/stateset-observability) | Metrics and tracing bootstrap |
| [`stateset-crypto`](https://crates.io/crates/stateset-crypto) | Canonical JSON, signing, hybrid PQ crypto |
| [`stateset-policy`](https://crates.io/crates/stateset-policy) | Declarative rule engine |
| [`stateset-sync`](https://crates.io/crates/stateset-sync) | Event-sourced outbox and sync engine |

Serving HTTP instead of embedding? See
[`stateset-http`](https://crates.io/crates/stateset-http).

## License

MIT OR Apache-2.0
