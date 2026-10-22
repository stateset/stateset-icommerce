//! SQLite timestamp columns are written as RFC 3339 (`2026-10-22T18:00:00+00:00`)
//! while `datetime('now')` renders `2026-10-22 18:00:00`. Comparing a raw
//! column with it is a text comparison in which `'T' > ' '`, so on the current
//! UTC day the answer is wrong: a lot that expired this morning was neither
//! expired nor excluded from "expiring". Compare with
//! `strftime('%Y-%m-%dT%H:%M:%f', 'now')` instead, or normalise both sides
//! with `datetime(column)`.
//!
//! This gate fails on any `column <op> datetime('now' ...)` in the SQLite
//! backend unless the column is wrapped in a function call or allowlisted.

use std::path::Path;

/// `(file, column)` pairs that compare against `datetime('now')` on purpose.
const ALLOWED: &[(&str, &str)] = &[
    // Stored at midnight UTC; the lexical quirk makes a bill due today not yet
    // overdue, matching Postgres's `due_date < CURRENT_DATE`.
    ("accounts_payable.rs", "due_date"),
    // Written with `datetime('now')`, so both sides share one format.
    ("accounts_receivable.rs", "last_dunning_date"),
];

const OPERATORS: &[&str] = &["<=", ">=", "<", ">", "="];

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read sqlite source dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The column compared against `datetime('now'` at byte `at`, or `None` when
/// the left operand is itself a function call such as `datetime(col)`.
fn compared_column(line: &str, at: usize) -> Option<String> {
    let before = line[..at].trim_end();
    let op = OPERATORS.iter().find(|op| before.ends_with(**op))?;
    let operand = before[..before.len() - op.len()].trim_end();
    if operand.ends_with(')') {
        return None;
    }
    let column: String = operand
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (!column.is_empty()).then_some(column)
}

#[test]
fn sqlite_backend_never_compares_raw_columns_with_datetime_now() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sqlite");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(!files.is_empty(), "no SQLite sources found under {}", root.display());

    let mut violations = Vec::new();
    for path in files {
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let text = std::fs::read_to_string(&path).expect("read source");
        for (number, line) in text.lines().enumerate() {
            for (at, _) in line.match_indices("datetime('now'") {
                let Some(column) = compared_column(line, at) else { continue };
                let bare = column.rsplit('.').next().unwrap_or(&column);
                if ALLOWED.iter().any(|(f, c)| *f == file && *c == bare) {
                    continue;
                }
                violations.push(format!("{}:{}: {}", file, number + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "compare RFC 3339 columns with strftime('%Y-%m-%dT%H:%M:%f', 'now'), \
         not datetime('now'):\n{}",
        violations.join("\n")
    );
}

#[test]
fn gate_detects_a_raw_comparison_and_skips_a_wrapped_one() {
    let raw = "AND expiration_date <= datetime('now') ORDER BY";
    assert_eq!(
        compared_column(raw, raw.find("datetime('now'").unwrap()).as_deref(),
        Some("expiration_date")
    );
    let wrapped = "AND datetime(ends_at) >= datetime('now')";
    assert_eq!(compared_column(wrapped, wrapped.rfind("datetime('now'").unwrap()), None);
}
