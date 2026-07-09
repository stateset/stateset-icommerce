//! Exact-decimal SQL aggregate for money columns.
//!
//! Money is stored in SQLite as TEXT (migration 006), so the built-in
//! `SUM()`/`AVG()` aggregates silently coerce each value to an IEEE-754 float
//! before accumulating — e.g. `SUM` over `'0.10'` and `'0.20'` yields
//! `0.30000000000000004`. For merchant-facing revenue/refund totals that get
//! reconciled against a ledger, that drift is unacceptable.
//!
//! [`register`] installs `decimal_sum(x)` and `decimal_sum_product(x, y)`
//! aggregates that accumulate with [`rust_decimal::Decimal`] instead, so the
//! totals are exact while remaining streaming O(1)-memory SQL aggregates (no
//! need to pull every row into Rust). Both return the result as a TEXT decimal
//! string (`"0"` when there are no rows), matching the shape the call sites
//! already parse back into `Decimal`.

use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, Result as SqlResult};
use rust_decimal::Decimal;

/// Parse one SQL value into a `Decimal`, or `None` for SQL NULL.
fn decimal_arg(fn_name: &str, value: ValueRef<'_>) -> SqlResult<Option<Decimal>> {
    match value {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => {
            let text = std::str::from_utf8(bytes)
                .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?;
            text.trim()
                .parse::<Decimal>()
                .map(Some)
                .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))
        }
        ValueRef::Integer(i) => Ok(Some(Decimal::from(i))),
        ValueRef::Real(r) => Decimal::try_from(r)
            .map(Some)
            .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e))),
        ValueRef::Blob(_) => Err(rusqlite::Error::UserFunctionError(
            format!("{fn_name}: BLOB is not a valid money value").into(),
        )),
    }
}

fn overflow_err(fn_name: &str) -> rusqlite::Error {
    rusqlite::Error::UserFunctionError(format!("{fn_name}: decimal overflow").into())
}

/// Aggregate that sums money-typed values exactly using `Decimal`.
struct DecimalSum;

impl Aggregate<Option<Decimal>, String> for DecimalSum {
    fn init(&self, _: &mut Context<'_>) -> SqlResult<Option<Decimal>> {
        Ok(None)
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Option<Decimal>) -> SqlResult<()> {
        // NULLs are skipped, mirroring SQL `SUM` semantics.
        let Some(value) = decimal_arg("decimal_sum", ctx.get_raw(0))? else {
            return Ok(());
        };
        let running = acc.unwrap_or(Decimal::ZERO);
        *acc = Some(running.checked_add(value).ok_or_else(|| overflow_err("decimal_sum"))?);
        Ok(())
    }

    fn finalize(&self, _: &mut Context<'_>, acc: Option<Option<Decimal>>) -> SqlResult<String> {
        // Outer `None` => no rows stepped; inner `None` => only NULLs seen.
        Ok(acc.flatten().unwrap_or(Decimal::ZERO).to_string())
    }
}

/// Aggregate that sums `x * y` exactly using `Decimal` — the exact-decimal
/// counterpart of `SUM(x * y)` (e.g. quantity × unit cost for inventory
/// valuation). A row where either argument is NULL is skipped, mirroring the
/// built-in `SUM`'s NULL-product semantics.
struct DecimalSumProduct;

impl Aggregate<Option<Decimal>, String> for DecimalSumProduct {
    fn init(&self, _: &mut Context<'_>) -> SqlResult<Option<Decimal>> {
        Ok(None)
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Option<Decimal>) -> SqlResult<()> {
        let (Some(x), Some(y)) = (
            decimal_arg("decimal_sum_product", ctx.get_raw(0))?,
            decimal_arg("decimal_sum_product", ctx.get_raw(1))?,
        ) else {
            return Ok(());
        };
        let product = x.checked_mul(y).ok_or_else(|| overflow_err("decimal_sum_product"))?;
        let running = acc.unwrap_or(Decimal::ZERO);
        *acc =
            Some(running.checked_add(product).ok_or_else(|| overflow_err("decimal_sum_product"))?);
        Ok(())
    }

    fn finalize(&self, _: &mut Context<'_>, acc: Option<Option<Decimal>>) -> SqlResult<String> {
        Ok(acc.flatten().unwrap_or(Decimal::ZERO).to_string())
    }
}

/// Register the `decimal_sum` and `decimal_sum_product` aggregates on a
/// connection.
///
/// Called from the pool's `with_init` so every pooled connection (including the
/// one migrations run on, and in-memory test databases) can use it.
pub(super) fn register(conn: &Connection) -> SqlResult<()> {
    conn.create_aggregate_function(
        "decimal_sum",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        DecimalSum,
    )?;
    conn.create_aggregate_function(
        "decimal_sum_product",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        DecimalSumProduct,
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
    fn decimal_sum_product_is_exact_where_builtin_drifts() {
        let conn = Connection::open_in_memory().expect("open");
        register(&conn).expect("register");
        conn.execute("CREATE TABLE b (qty INTEGER, cost TEXT)", []).expect("create");
        // 3 x 0.10 accumulated as floats drifts; exact total is 0.70.
        for (qty, cost) in [(1, "0.10"), (2, "0.10"), (4, "0.10")] {
            conn.execute("INSERT INTO b (qty, cost) VALUES (?1, ?2)", (qty, cost)).expect("insert");
        }

        let builtin: f64 =
            conn.query_row("SELECT SUM(qty * cost) FROM b", [], |row| row.get(0)).expect("builtin");
        assert_ne!(builtin, 0.70_f64, "precondition: built-in SUM(x*y) is expected to drift");

        let text: String = conn
            .query_row("SELECT decimal_sum_product(qty, cost) FROM b", [], |row| row.get(0))
            .expect("exact");
        assert_eq!(text.parse::<Decimal>().expect("parse"), dec!(0.70));
    }

    #[test]
    fn decimal_sum_product_skips_null_rows_and_is_zero_when_empty() {
        let conn = Connection::open_in_memory().expect("open");
        register(&conn).expect("register");
        conn.execute("CREATE TABLE b (qty INTEGER, cost TEXT)", []).expect("create");

        let empty: String = conn
            .query_row("SELECT decimal_sum_product(qty, cost) FROM b", [], |row| row.get(0))
            .expect("empty");
        assert_eq!(empty, "0");

        conn.execute("INSERT INTO b (qty, cost) VALUES (NULL, '9.99')", []).expect("null qty");
        conn.execute("INSERT INTO b (qty, cost) VALUES (3, NULL)", []).expect("null cost");
        conn.execute("INSERT INTO b (qty, cost) VALUES (2, '1.25')", []).expect("row");

        let text: String = conn
            .query_row("SELECT decimal_sum_product(qty, cost) FROM b", [], |row| row.get(0))
            .expect("sum");
        assert_eq!(text.parse::<Decimal>().expect("parse"), dec!(2.50));
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
