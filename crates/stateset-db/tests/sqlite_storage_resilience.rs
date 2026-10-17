//! Bounded storage/locking failures on private stores; never fill the host disk.
#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{CreatePayment, CreateRefund, PaymentRepository, RefundStatus};
use stateset_db::{DatabaseConfig, SqliteDatabase, SqliteDurability};

#[test]
fn durability_applies_to_every_pooled_connection_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    for mode in [SqliteDurability::Normal, SqliteDurability::Full] {
        let config = DatabaseConfig {
            url: dir.path().join(format!("{mode:?}.db")).to_str().unwrap().into(),
            max_connections: 3,
        };
        let db = SqliteDatabase::new_with_durability(&config, mode).unwrap();
        let expected = if mode == SqliteDurability::Full { 2 } else { 1 };
        let connections: Vec<_> = (0..3).map(|_| db.conn().unwrap()).collect();
        for conn in &connections {
            let synchronous: i64 =
                conn.query_row("PRAGMA synchronous", [], |row| row.get(0)).unwrap();
            let foreign_keys: i64 =
                conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).unwrap();
            assert_eq!(synchronous, expected);
            assert_eq!(foreign_keys, 1);
        }
        drop(connections);
        assert_eq!(db.storage_status().unwrap().journal_mode, "wal");
        assert_eq!(db.storage_status().unwrap().synchronous, expected as u32);
        db.shutdown().unwrap();
        drop(db);
        let reopened = SqliteDatabase::new_with_durability(&config, mode).unwrap();
        assert_eq!(reopened.storage_status().unwrap().synchronous, expected as u32);
    }
}

#[test]
fn environment_policy_is_checked_before_opening_the_store() {
    const CHILD: &str = "STATESET_DURABILITY_TEST_CHILD";
    if let Some(path) = std::env::var_os(CHILD) {
        let config = DatabaseConfig::sqlite(path.to_str().unwrap());
        let setting = std::env::var(stateset_db::SQLITE_SYNCHRONOUS_ENV).unwrap();
        if setting == "FULL" {
            let db = SqliteDatabase::new(&config).unwrap();
            assert_eq!(db.storage_status().unwrap().synchronous, 2);
        } else {
            assert!(SqliteDatabase::new(&config).is_err());
            assert!(!std::path::Path::new(&path).exists());
        }
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for setting in ["FULL", "OFF", "typo"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "environment_policy_is_checked_before_opening_the_store",
                "--nocapture",
            ])
            .env(CHILD, dir.path().join(format!("{setting}.db")))
            .env(stateset_db::SQLITE_SYNCHRONOUS_ENV, setting)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn shutdown_reports_a_checkpoint_blocked_by_a_reader_and_can_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkpoint.db");
    let config = DatabaseConfig { url: path.to_str().unwrap().into(), max_connections: 1 };
    let db = SqliteDatabase::new_with_durability(&config, SqliteDurability::Full).unwrap();
    db.conn().unwrap().execute_batch("CREATE TABLE probe (value INTEGER); INSERT INTO probe VALUES (1); PRAGMA busy_timeout=25;").unwrap();
    let reader = rusqlite::Connection::open(&path).unwrap();
    reader.execute_batch("BEGIN; SELECT * FROM probe;").unwrap();
    db.conn().unwrap().execute("INSERT INTO probe VALUES (2)", []).unwrap();
    let error = db.shutdown().expect_err("active snapshot prevents a truncating checkpoint");
    assert!(error.to_string().contains("checkpoint blocked"), "{error}");
    reader.execute_batch("ROLLBACK").unwrap();
    db.shutdown().expect("checkpoint succeeds after the reader releases its snapshot");
    assert_eq!(std::fs::metadata(path.with_extension("db-wal")).unwrap().len(), 0);
}

#[test]
fn sqlite_full_during_refund_rolls_back_and_same_refund_can_retry() {
    let db = SqliteDatabase::new_with_durability(
        &DatabaseConfig { url: ":memory:".into(), max_connections: 1 },
        SqliteDurability::Full,
    )
    .unwrap();
    let payment =
        db.payments().create(CreatePayment { amount: dec!(0.30), ..Default::default() }).unwrap();
    db.payments().mark_completed(payment.id).unwrap();
    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .unwrap();
    let conn = db.conn().unwrap();
    conn.execute_batch(
        "CREATE TABLE storage_pressure (payload BLOB);
        CREATE TRIGGER exhaust_refund_storage AFTER UPDATE OF status ON refunds
        WHEN NEW.status = 'completed' BEGIN
            INSERT INTO storage_pressure VALUES (zeroblob(2097152)); END;",
    )
    .unwrap();
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0)).unwrap();
    let original_limit: i64 =
        conn.query_row("PRAGMA max_page_count", [], |row| row.get(0)).unwrap();
    conn.pragma_update(None, "max_page_count", pages + 1).unwrap();
    let events_before: i64 =
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get(0)).unwrap();
    drop(conn);
    let error =
        db.payments().complete_refund(refund.id).expect_err("bounded SQLite storage exhaustion");
    assert!(error.to_string().contains("full"), "{error}");
    assert_eq!(db.payments().get_refund(refund.id).unwrap().unwrap().status, RefundStatus::Pending);
    assert_eq!(db.payments().get(payment.id).unwrap().unwrap().amount_refunded, dec!(0));
    let conn = db.conn().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM storage_pressure", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM kernel_outbox", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        events_before
    );
    conn.pragma_update(None, "max_page_count", original_limit).unwrap();
    conn.execute_batch("DROP TRIGGER exhaust_refund_storage").unwrap();
    assert_eq!(
        conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)).unwrap(),
        "ok"
    );
    drop(conn);
    db.payments().complete_refund(refund.id).unwrap();
    db.payments().complete_refund(refund.id).unwrap();
    assert_eq!(db.payments().get(payment.id).unwrap().unwrap().amount_refunded, dec!(0.10));
    println!("storage-resilience-proof sqlite_full=rollback refund_retry=once integrity=ok");
}
