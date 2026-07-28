# Money: Storage and Arithmetic Contract

Money in StateSet iCommerce is **exact decimal, end to end**. This page is the
authoritative statement of how that is achieved on each backend, why the
storage choices look the way they do, and which invariants are enforced by
gates rather than convention.

## The contract

1. **All money arithmetic happens in Rust**, using
   [`rust_decimal::Decimal`](https://docs.rs/rust_decimal) — 96-bit exact
   decimal. No money value is ever computed in SQL on the SQLite backend.
2. **Amounts are rounded to the currency minor unit at the write boundary**
   (`MONEY_SCALE`, currently 2), so stored aggregates foot exactly to their
   line items.
3. **Overflow-safe operations** (`checked_add`, `checked_mul`) guard the
   arithmetic paths; caps, limits, and balances are enforced inside the same
   transaction that writes them.

## SQLite: TEXT columns are deliberate

Money columns on the SQLite backend are `TEXT`, storing canonical decimal
strings (`"29.99"`). This is a considered choice, not an accident:

- SQLite has no exact-decimal type. `NUMERIC`/`REAL` affinity coerces to
  IEEE-754 floats — `'0.10' + '0.20'` becomes `0.30000000000000004` — which
  is precisely the corruption class this design avoids. `INTEGER` minor
  units would be exact but bake a per-currency scale into every value.
- TEXT round-trips `Decimal` losslessly and sorts/compares correctly when
  comparisons happen in Rust, where they belong.

The hazard of TEXT money is doing arithmetic on it **in SQL**, where SQLite
silently coerces to floats. Two mechanisms make that hazard structural
rather than a convention:

- **`decimal_sum`** — a registered application-defined aggregate
  (`crates/stateset-db/src/sqlite/money_agg.rs`) that accumulates exactly in
  `Decimal`. All reconciled totals (revenue, refunds, balances) use it.
- **A source lint** (`crates/stateset-db/tests/money_sql_lint.rs`) fails the
  build on any raw `SUM(<money column>)` or `<money column> + ?` in the
  SQLite backend sources. Deliberate exceptions (analytics *averages*,
  where float approximation is documented and immaterial) live in an
  explicit allowlist. The lint is verified to catch injected violations.

## PostgreSQL: native DECIMAL

The PostgreSQL backend stores money as `DECIMAL(12,2)` and may aggregate in
SQL, since Postgres numerics are exact. One decode rule applies: `numeric`
values must be read as `Decimal` (or explicitly cast, e.g.
`::double precision` for the analytics averages) — sqlx refuses implicit
`numeric` → `f64` decodes, which is a feature.

## Cross-backend parity

Both backends run the same guard suites: over-refund prevention, credit
limits enforced in-transaction (`FOR UPDATE` on Postgres), gift-card /
store-credit / loyalty overdraft rejection, and rounding tests that pin
totals to the cent. The parity CI lanes run these against a live Postgres.

## What is intentionally approximate

Analytics *averages* (average order value, average lifetime value) use SQL
`AVG`/`SUM` with float coercion because exactness is immaterial for a
statistical read model and the values are never written back. Each such site
carries a comment and a lint allowlist entry; adding a new one requires both.
