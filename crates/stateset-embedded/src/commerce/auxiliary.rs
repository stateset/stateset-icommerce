//! Auxiliary SQLite connections for side tables the engine does not own.

use super::Commerce;

use stateset_core::CommerceError;
use stateset_db::sqlite::rusqlite;

impl Commerce {
    /// Open a connection on this engine's own SQLite file, outside the pool,
    /// for auxiliary tables the engine does not own (a sync outbox, an import
    /// id map, a bridge inbox).
    ///
    /// Host code that keeps such tables in the engine's file must reach them
    /// through this connection, never through a second SQLite library loaded
    /// into the same process. SQLite tracks its POSIX locks per library copy
    /// while the kernel tracks them per process, so a second copy that opens
    /// and closes the file drops locks this engine still relies on and can
    /// checkpoint and unlink the WAL beneath it — later commits then vanish
    /// and the file reports "database disk image is malformed". This
    /// connection is opened by the same linked SQLite as the engine's pool,
    /// so closing it never releases the engine's locks.
    ///
    /// Works for `:memory:` too: the connection reaches the engine's private
    /// temp file. See [`stateset_db::SqliteDatabase::open_auxiliary_connection`]
    /// for the setup it shares with pooled connections.
    ///
    /// Writing commerce tables through it bypasses every engine invariant;
    /// use the domain APIs for those.
    ///
    /// # Errors
    ///
    /// Returns [`CommerceError::NotPermitted`] for a PostgreSQL or
    /// externally supplied backend, which have no SQLite file to share, and
    /// [`CommerceError::DatabaseError`] when the connection cannot be opened.
    pub fn open_auxiliary_sqlite_connection(&self) -> Result<rusqlite::Connection, CommerceError> {
        match &self.sqlite_db {
            Some(db) => db.open_auxiliary_connection(),
            None => Err(CommerceError::NotPermitted(format!(
                "auxiliary SQLite connections need a SQLite-backed Commerce; this instance uses the {:?} backend",
                self.backend
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    fn create_customer(commerce: &Commerce, email: &str) {
        commerce
            .customers()
            .create(stateset_core::CreateCustomer {
                email: email.to_owned(),
                first_name: "Side".to_owned(),
                last_name: "Store".to_owned(),
                ..Default::default()
            })
            .expect("create customer");
    }

    fn count(conn: &rusqlite::Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))
            .expect("count rows")
    }

    #[test]
    fn memory_engine_connection_reaches_the_private_temp_file() {
        let commerce = Commerce::new(":memory:").expect("commerce");
        create_customer(&commerce, "memory@example.com");
        let conn = commerce.open_auxiliary_sqlite_connection().expect("auxiliary connection");
        assert_eq!(count(&conn, "customers"), 1, "sees the engine's committed rows");

        conn.execute_batch(
            "CREATE TABLE side_notes (body TEXT); INSERT INTO side_notes VALUES ('x');",
        )
        .expect("write a side table");
        let other = commerce.open_auxiliary_sqlite_connection().expect("second connection");
        assert_eq!(count(&other, "side_notes"), 1, "every auxiliary connection shares the file");

        // Set up exactly as a pooled connection.
        let mode: String =
            conn.query_row("PRAGMA journal_mode", [], |row| row.get(0)).expect("journal_mode");
        assert_eq!(mode, "wal");
        let foreign_keys: i64 =
            conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).expect("foreign_keys");
        assert_eq!(foreign_keys, 1);
        let total: String = conn
            .query_row(
                "SELECT decimal_sum(x) FROM (SELECT '0.10' AS x UNION ALL SELECT '0.20')",
                [],
                |row| row.get(0),
            )
            .expect("decimal_sum is registered");
        assert_eq!(total, "0.30");

        // Another `:memory:` engine is another database.
        let unrelated = Commerce::new(":memory:").expect("unrelated commerce");
        let elsewhere = unrelated.open_auxiliary_sqlite_connection().expect("unrelated connection");
        assert_eq!(count(&elsewhere, "customers"), 0);
    }

    #[test]
    fn close_releases_the_file_after_concurrent_writes_grew_the_pool() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("store.db");
        // A plain drop leaves the WAL behind in about 15% of these rounds,
        // so 10 rounds catch a regression about 80% of the time; the
        // deterministic check is in `stateset_db`'s `file_release` test.
        for round in 0..10 {
            let commerce = Commerce::new(path.to_str().expect("utf-8 path")).expect("commerce");
            // Concurrent writers make r2d2 open more connections in the
            // background; one may still be opening when the writers finish.
            std::thread::scope(|scope| {
                for writer in 0..8 {
                    let commerce = &commerce;
                    scope.spawn(move || {
                        create_customer(commerce, &format!("r{round}w{writer}@example.com"));
                    });
                }
            });
            commerce.close().expect("close");
            assert!(
                !dir.path().join("store.db-wal").exists(),
                "round {round}: the WAL is gone when close() returns"
            );
        }
        let reopened = Commerce::new(path.to_str().expect("utf-8 path")).expect("reopen");
        let conn = reopened.open_auxiliary_sqlite_connection().expect("auxiliary connection");
        assert_eq!(count(&conn, "customers"), 80);
    }

    #[test]
    fn close_reports_a_file_still_held_by_an_auxiliary_connection() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("store.db");
        let commerce = Commerce::new(path.to_str().expect("utf-8 path")).expect("commerce");
        let conn = commerce.open_auxiliary_sqlite_connection().expect("auxiliary connection");
        let error = commerce
            .close_within(std::time::Duration::from_millis(50))
            .expect_err("the auxiliary connection still holds the file");
        assert!(matches!(error, CommerceError::DatabaseError(_)), "{error:?}");
        assert!(error.to_string().contains("still in use"), "{error}");
        drop(conn);
        assert!(!dir.path().join("store.db-wal").exists(), "the last connection removed the WAL");
    }

    #[test]
    fn memory_engine_file_outlives_the_engine_while_a_connection_is_open() {
        let commerce = Commerce::new(":memory:").expect("commerce");
        create_customer(&commerce, "kept@example.com");
        let conn = commerce.open_auxiliary_sqlite_connection().expect("auxiliary connection");
        drop(commerce);
        assert_eq!(count(&conn, "customers"), 1, "the private directory is still alive");
    }

    #[test]
    fn closing_an_auxiliary_connection_keeps_the_engine_wal() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("store.db");
        let commerce = Commerce::new(path.to_str().expect("utf-8 path")).expect("commerce");
        create_customer(&commerce, "before@example.com");
        let wal = dir.path().join("store.db-wal");
        assert!(wal.exists(), "the engine runs in WAL mode");

        let conn = commerce.open_auxiliary_sqlite_connection().expect("auxiliary connection");
        conn.execute_batch("CREATE TABLE side_notes (body TEXT)").expect("write a side table");
        drop(conn);
        assert!(wal.exists(), "closing it must not checkpoint and unlink the engine's WAL");

        create_customer(&commerce, "after@example.com");
        let reader = commerce.open_auxiliary_sqlite_connection().expect("reader");
        assert_eq!(count(&reader, "customers"), 2);
    }

    #[test]
    fn a_backend_without_a_sqlite_file_refuses() {
        let database = Arc::new(stateset_db::SqliteDatabase::in_memory().expect("database"));
        let commerce = Commerce::with_database(database);
        let error =
            commerce.open_auxiliary_sqlite_connection().expect_err("an external backend refuses");
        assert!(
            matches!(&error, CommerceError::NotPermitted(message) if message.contains("External")),
            "{error}"
        );
    }
}
