//! Source lint: no SQLite arithmetic on TEXT money columns.
//!
//! Money is stored in TEXT columns on the SQLite backend and summed exactly in
//! Rust (`rust_decimal`) or via the registered `decimal_sum` aggregate. Doing
//! `SUM(col)` / `col + ?` in SQL coerces TEXT to IEEE-754 floats
//! (`'0.10'+'0.20' = 0.30000000000000004`), which corrupted refund state once
//! already (June 2026, payments.rs). This test walks the SQLite backend
//! sources and fails on new raw arithmetic over money-named columns, so the
//! convention cannot silently regress.
//!
//! Deliberate exceptions (averages where float approximation is documented and
//! immaterial) live in the allowlist below — extend it only with a comment at
//! the SQL site explaining why exactness is not required.

use std::fs;
use std::path::Path;

/// Column names that hold money as TEXT on the SQLite backend.
const MONEY_COLUMNS: &[&str] = &[
    "amount",
    "total_amount",
    "unit_price",
    "subtotal",
    "tax_amount",
    "discount_amount",
    "refund_amount",
    "amount_refunded",
    "cost_price",
    "credit_limit",
    "current_balance",
    "balance",
    "total_spent",
];

/// `(file suffix, needle)` pairs that are documented, deliberate float
/// approximations (averages) or the `decimal_sum` test fixture itself.
const ALLOWLIST: &[(&str, &str)] = &[
    // avg_order: average, float coercion immaterial; exact totals in the same
    // query use decimal_sum (see comment at the SQL site).
    ("analytics.rs", "SUM(total_amount) / NULLIF(COUNT(*), 0)"),
    // Average lifetime value: inner SUM feeds an AVG; documented approximate.
    ("analytics.rs", "SELECT customer_id, SUM(total_amount) as total"),
    // money_agg.rs tests intentionally compare builtin SUM against decimal_sum.
    ("money_agg.rs", "SUM(amount)"),
    ("money_agg.rs", "SUM(qty * cost)"),
];

fn is_allowlisted(file_name: &str, line: &str) -> bool {
    ALLOWLIST.iter().any(|(suffix, needle)| file_name.ends_with(suffix) && line.contains(needle))
}

#[test]
fn no_raw_sql_arithmetic_on_money_columns() {
    let sqlite_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sqlite");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&sqlite_dir).expect("read src/sqlite") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let source = fs::read_to_string(&path).expect("read source file");

        for (idx, line) in source.lines().enumerate() {
            // Comments about the rule are not violations of the rule.
            if line.trim_start().starts_with("//") {
                continue;
            }
            if is_allowlisted(&file_name, line) {
                continue;
            }
            for col in MONEY_COLUMNS {
                // Raw SQL aggregation over a money column.
                for agg in ["SUM(", "sum("] {
                    let needle = format!("{agg}{col})");
                    if line.contains(&needle) && !line.contains("decimal_sum") {
                        violations.push(format!(
                            "{file_name}:{}: raw SQL {agg}{col}) — use decimal_sum or sum in Rust",
                            idx + 1
                        ));
                    }
                }
                // In-SQL addition/subtraction on a money column
                // (`amount_refunded = amount_refunded + ?` style).
                for op in ['+', '-'] {
                    let needle = format!("{col} {op} ?");
                    if line.contains(&needle) {
                        violations.push(format!(
                            "{file_name}:{}: SQL `{col} {op} ?` coerces TEXT money to float — \
                             compute in Rust with Decimal and write the result",
                            idx + 1
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SQL arithmetic on TEXT money columns detected:\n{}",
        violations.join("\n")
    );
}
