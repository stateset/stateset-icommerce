# stateset-db

Database implementations for StateSet iCommerce. Provides SQLite and PostgreSQL backends that implement the repository traits from `stateset-core`.

## Features

- **SQLite** (default): Embedded database via `rusqlite` with `r2d2` connection pooling
- **PostgreSQL** (optional): Async backend via `sqlx`
- Automatic migrations and schema management
- Event store for append-only commerce events

## Feature Flags

- `sqlite` (default) — SQLite via rusqlite (bundled)
- `postgres` — PostgreSQL via sqlx
- `vector` — Vector embedding storage
- `saga` — Experimental saga/workflow support

## License

MIT OR Apache-2.0
