# stateset-db

Database implementations for StateSet iCommerce. Provides SQLite and PostgreSQL backends that implement the repository traits from `stateset-core`.

## Backend Support

### SQLite (Primary — Full Coverage)

The default and recommended backend. All 52 entity modules are fully implemented with 165+ tests. Includes:

- Core commerce: orders, customers, products, inventory, returns, payments, invoices, shipments
- V4 entities: reviews, wishlists, gift cards, loyalty, fraud, segments, store credits, shipping zones, rewards, search configs
- A2A: agent cards, quotes, purchases, reputation, validation, identities
- Financial: accounts payable/receivable, general ledger, cost accounting, credit
- Advanced: vector search, BOM/manufacturing, subscriptions, promotions, tax, warranties

### PostgreSQL (Async — Partial Coverage)

Async backend for high-concurrency deployments. Covers core V1-V3 entities (39 modules). V4+ entities (reviews, wishlists, gift cards, loyalty, fraud, segments, etc.) are not yet implemented for Postgres — use SQLite for full feature coverage.

## Feature Flags

- `sqlite` (default) — SQLite via rusqlite (bundled). Full entity coverage.
- `postgres` — PostgreSQL via sqlx. Async. Partial entity coverage (V1-V3).
- `vector` — Vector embedding storage (SQLite only)
- `saga` — Experimental saga/workflow support

## License

MIT OR Apache-2.0
