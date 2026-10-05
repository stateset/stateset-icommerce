# stateset-migrations

[![crates.io](https://img.shields.io/crates/v/stateset-migrations.svg)](https://crates.io/crates/stateset-migrations)
[![docs.rs](https://docs.rs/stateset-migrations/badge.svg)](https://docs.rs/stateset-migrations)

A schema migration framework with first-class SQLite support: ordered, versioned
migrations with checksum validation, rollback, and status reporting — plus the full
built-in StateSet iCommerce schema.

Checksums are the reason this exists rather than a hand-rolled `PRAGMA user_version`
bump. If a migration that has already been applied is edited, the registry refuses to
run instead of silently diverging your schema from everyone else's.

## Features

- **`Migration`** and **`MigrationRecord`** — define and track schema changes
- **`MigrationRegistry`** — ordered, versioned migrations with checksum validation
- **`SqliteMigrator`** — apply and roll back against SQLite
- **Built-in migrations** for the complete StateSet iCommerce schema
- **`SchemaVersion`** and **`MigrationStatus`** for reporting

## Usage

```rust
use stateset_migrations::{builtin_registry, SqliteMigrator};

let registry = builtin_registry().unwrap();
let migrator = SqliteMigrator::new(registry);

let conn = rusqlite::Connection::open_in_memory().unwrap();
let applied = migrator.migrate(&conn).unwrap();
println!("Applied {} migrations", applied.len());

let status = migrator.status(&conn).unwrap();
println!("Schema: {}", status.schema_version);
```

Application migrations use a separate `_stateset_custom_migrations` ledger and
can run on the same connection as the engine. Engine migrations are immutable;
define application extensions in a separate registry:

```rust
use stateset_migrations::{Migration, MigrationRegistry};

let registry = MigrationRegistry::builder()
    .add(Migration::new(1, "create_users", "CREATE TABLE users (id TEXT PRIMARY KEY);"))
    .add(Migration::with_down(
        2,
        "add_email",
        "ALTER TABLE users ADD COLUMN email TEXT;",
        "SELECT 1; -- SQLite cannot drop columns easily",
    ))
    .build()
    .unwrap();

assert_eq!(registry.len(), 2);
```

## Rollback

The canonical engine migrations are forward-only and share the exact SQL,
checksums, ledger, and legacy data upgrades used by `stateset-db`. Historical
`v1_*` through `v9_*` constructors remain available for compatibility through
`builtins::legacy_registry`; they are not the current engine schema.
Existing stores created with the old standalone built-ins must continue using
`legacy_registry()` until their data is migrated into a fresh engine store.
The engine refuses the old standalone ledger instead of applying incompatible
SQL to it. Older *engine* ledgers without checksums are upgraded automatically.

Only migrations declared with `Migration::with_down` can be rolled back; a
forward-only `Migration::new` has no inverse and the migrator will say so rather than
guess one. Roll back deliberately — the built-in schema is additive by policy, which
is what keeps `v1.x` upgrades non-breaking.

## Part of StateSet iCommerce

Applied automatically by [`stateset-db`](https://crates.io/crates/stateset-db) and
[`stateset-embedded`](https://crates.io/crates/stateset-embedded) on open, so most
users never call it directly. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
