//! Audit log for tracking critical mutations.
//!
//! Provides a lightweight `record_audit` function that inserts into the
//! `audit_log` table (canonical engine migration 101, legacy standalone V8).
//! Callers opt into this helper; it is not automatically attached to every
//! repository mutation. Governed receipt audit chains and domain outbox facts
//! are separate mechanisms. Operator receipt reconciliation requires an audit
//! insert in the same transaction as its state change.

use std::sync::atomic::{AtomicU64, Ordering};

static AUDIT_WRITE_FAILURES: AtomicU64 = AtomicU64::new(0);

/// Number of failed audit inserts in this process, across all SQLite stores.
/// Monotonic, without a reset API; export as a counter and alert on increases.
/// A zero count is not evidence that every business operation was audited.
pub fn audit_write_failures() -> u64 {
    AUDIT_WRITE_FAILURES.load(Ordering::Relaxed)
}

/// Insert an audit entry and propagate failure. Call this inside the business
/// transaction when an operation requires an audit record to commit.
pub fn record_audit_required(
    conn: &rusqlite::Connection,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    actor: Option<&str>,
    details: Option<&str>,
) -> rusqlite::Result<()> {
    let result = conn.execute(
        "INSERT INTO audit_log (action, entity_type, entity_id, actor, details) VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![action, entity_type, entity_id, actor, details],
    );
    result
        .and_then(
            |rows| {
                if rows == 1 { Ok(()) } else { Err(rusqlite::Error::StatementChangedRows(rows)) }
            },
        )
        .inspect_err(|_| {
            AUDIT_WRITE_FAILURES.fetch_add(1, Ordering::Relaxed);
        })
}

/// Record an audit log entry for a mutation.
///
/// This is a best-effort operation — failures are logged but do not
/// prevent the original operation from succeeding.
pub fn record_audit(
    conn: &rusqlite::Connection,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    actor: Option<&str>,
    details: Option<&str>,
) {
    let result = record_audit_required(conn, action, entity_type, entity_id, actor, details);
    if let Err(e) = result {
        tracing::warn!(
            action,
            entity_type,
            entity_id,
            error = %e,
            "Failed to write audit log entry"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_audit_succeeds_on_valid_table() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                actor TEXT,
                details TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .unwrap();

        record_audit(&conn, "create", "order", "ord-123", Some("admin"), Some("{\"total\":99.99}"));

        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);

        let action: String =
            conn.query_row("SELECT action FROM audit_log WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(action, "create");
    }

    #[test]
    fn record_audit_does_not_panic_on_missing_table() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // No table created — should log warning but not panic
        let before = audit_write_failures();
        record_audit(&conn, "delete", "customer", "cust-456", None, None);
        assert!(audit_write_failures() > before);
    }
}
