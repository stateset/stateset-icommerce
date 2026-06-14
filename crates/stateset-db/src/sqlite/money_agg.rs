//! Exact-decimal SQL aggregate for money columns.
//!
//! Money is stored in SQLite as TEXT (migration 006), so the built-in
//! `SUM()`/`AVG()` aggregates silently coerce each value to an IEEE-754 float
//! before accumulating — e.g. `SUM` over `'0.10'` and `'0.20'` yields
//! `0.30000000000000004`. For merchant-facing revenue/refund totals that get
//! reconciled against a ledger, that drift is unacceptable.
//!
//! [`register`] installs a `decimal_sum(x)` aggregate that accumulates with
//! [`rust_decimal::Decimal`] instead, so the total is exact while remaining a
//! streaming O(1)-memory SQL aggregate (no need to pull every row into Rust).
//! It returns the sum as a TEXT decimal string (`"0"` when there are no rows),
//! matching the shape the call sites already parse back into `Decimal`.

use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, Result as SqlResult};
use rust_decimal::Decimal;

/// Aggregate that sums money-typed values exactly using `Decimal`.
struct DecimalSum;

impl Aggregate<Option<Decimal>, String> for DecimalSum {
    fn init(&self, _: &mut Context<'_>) -> SqlResult<Option<Decimal>> {
        Ok(None)
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Option<Decimal>) -> SqlResult<()> {
        let value = match ctx.get_raw(0) {
            // NULLs are skipped, mirroring SQL `SUM` semantics.
            ValueRef::Null => return Ok(()),
            ValueRef::Text(bytes) => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?;
                text.trim()
                    .parse::<Decimal>()
                    .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?
            }
            ValueRef::Integer(i) => Decimal::from(i),
            ValueRef::Real(r) => {
                Decimal::try_from(r).map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?
            }
            ValueRef::Blob(_) => {
                return Err(rusqlite::Error::UserFunctionError(
                    "decimal_sum: BLOB is not a valid money value".into(),
                ));
            }
        };
        *acc = Some(acc.map_or(value, |running| running + value));
        Ok(())
    }

    fn finalize(&self, _: &mut Context<'_>, acc: Option<Option<Decimal>>) -> SqlResult<String> {
        // Outer `None` => no rows stepped; inner `None` => only NULLs seen.
        Ok(acc.flatten().unwrap_or(Decimal::ZERO).to_string())
    }
}

/// Register the `decimal_sum` aggregate on a connection.
///
/// Called from the pool's `with_init` so every pooled connection (including the
/// one migrations run on, and in-memory test databases) can use it.
pub(super) fn register(conn: &Connection) -> SqlResult<()> {
    conn.create_aggregate_function(
        "decimal_sum",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        DecimalSum,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Build a connection with `decimal_sum` registered and a TEXT money column
    /// seeded with the given string amounts.
    fn seeded_conn(amounts: &[&str]) -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory");
        register(&conn).expect("register decimal_sum");
        conn.execute("CREATE TABLE m (amount TEXT)", []).expect("create table");
        for amount in amounts {
            conn.execute("INSERT INTO m (amount) VALUES (?1)", [amount]).expect("insert");
        }
        conn
    }

    fn decimal_sum_of(conn: &Connection) -> Decimal {
        let text: String = conn
            .query_row("SELECT decimal_sum(amount) FROM m", [], |row| row.get(0))
            .expect("decimal_sum query");
        text.parse().expect("parse decimal")
    }

    #[test]
    fn decimal_sum_is_exact_where_builtin_sum_drifts() {
        let conn = seeded_conn(&["0.10", "0.20"]);

        // The built-in SUM coerces TEXT to f64 and accumulates with float error.
        let builtin: f64 =
            conn.query_row("SELECT SUM(amount) FROM m", [], |row| row.get(0)).expect("builtin sum");
        assert_ne!(builtin, 0.30_f64, "precondition: built-in SUM is expected to drift");

        // decimal_sum accumulates with Decimal and is exact.
        assert_eq!(decimal_sum_of(&conn), dec!(0.30));
    }

    #[test]
    fn decimal_sum_handles_many_cents_without_drift() {
        // 100 x 0.01 is a classic float-accumulation trap; the exact total is 1.00.
        let cents = vec!["0.01"; 100];
        let conn = seeded_conn(&cents);
        assert_eq!(decimal_sum_of(&conn), dec!(1.00));
    }

    #[test]
    fn decimal_sum_skips_nulls_and_is_zero_when_empty() {
        let empty = seeded_conn(&[]);
        assert_eq!(decimal_sum_of(&empty), Decimal::ZERO);

        let conn = Connection::open_in_memory().expect("open");
        register(&conn).expect("register");
        conn.execute("CREATE TABLE m (amount TEXT)", []).expect("create");
        conn.execute("INSERT INTO m (amount) VALUES (NULL)", []).expect("insert null");
        conn.execute("INSERT INTO m (amount) VALUES ('12.34')", []).expect("insert");
        assert_eq!(decimal_sum_of(&conn), dec!(12.34));
    }
}
