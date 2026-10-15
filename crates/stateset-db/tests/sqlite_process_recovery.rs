//! Real process termination and contention against a file-backed store.
//! These checks cover process death, not host power loss or external providers.
#![cfg(feature = "sqlite")]

use rust_decimal::Decimal;
use stateset_core::{
    CommandEnvelope, CreateInventoryItem, ExecutionMode, ExecutionStatus, InventoryRepository,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, PrincipalKind, ReserveInventory,
};
use stateset_db::{DatabaseConfig, SqliteDatabase};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const STORE_ENV: &str = "STATESET_RECOVERY_CHILD_STORE";
const SKU: &str = "RECOVERY-FRACTIONAL";

fn database(path: &Path) -> SqliteDatabase {
    SqliteDatabase::new(&DatabaseConfig {
        url: path.to_str().expect("UTF-8 temporary path").into(),
        max_connections: 2,
    })
    .expect("open durable database")
}

fn policy() -> KernelPolicy {
    KernelPolicy::new("process-recovery-v1")
        .allow("inventory.reserve", KernelCommandPolicy::requiring(["inventory.reserve"]))
}

fn command(key: &str) -> CommandEnvelope<ReserveInventory> {
    let mut command = CommandEnvelope::preview(
        "inventory.reserve",
        key,
        KernelPrincipal {
            id: "agent:process-recovery".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant:process-recovery".into()),
            delegated_by: Some("operator:process-recovery".into()),
            capabilities: vec!["inventory.reserve".into()],
        },
        ReserveInventory {
            sku: SKU.into(),
            quantity: Decimal::new(125, 3),
            reference_type: "recovery-drill".into(),
            reference_id: key.into(),
            location_id: None,
            expires_in_seconds: None,
        },
    );
    command.mode = ExecutionMode::Apply;
    command.store_id = Some("store:process-recovery".into());
    command.policy_version = Some("process-recovery-v1".into());
    command
}

fn seed(path: &Path, quantity: Decimal) {
    database(path)
        .inventory()
        .create_item(CreateInventoryItem {
            sku: SKU.into(),
            name: "Exact fractional stock".into(),
            initial_quantity: Some(quantity),
            ..Default::default()
        })
        .expect("seed stock");
}

struct Worker(Child);

impl Drop for Worker {
    fn drop(&mut self) {
        // Also reap on a parent assertion failure or handshake timeout.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn(test: &str, path: &Path) -> Worker {
    Worker(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(STORE_ENV, path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn isolated worker"),
    )
}

fn kill_at_boundary(worker: &mut Worker) {
    let stdout = worker.0.stdout.take().expect("child stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let line = line.expect("worker output");
            if line.ends_with("process-boundary-ready") {
                let _ = sender.send(());
                break;
            }
        }
    });
    receiver.recv_timeout(Duration::from_secs(60)).expect("worker reached requested boundary");
    worker.0.kill().expect("terminate worker without cleanup");
    assert!(!worker.0.wait().expect("reap worker").success());
    reader.join().expect("join output reader");
}

fn await_termination() -> ! {
    println!("process-boundary-ready");
    std::io::stdout().flush().expect("flush boundary marker");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).expect("wait for parent");
    panic!("worker must be killed at the boundary, without returning");
}

fn integrity(db: &SqliteDatabase) {
    let conn = db.conn().expect("connection");
    let check: String =
        conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(check, "ok");
    let violations: usize = conn
        .prepare("PRAGMA foreign_key_check")
        .expect("foreign key check")
        .query_map([], |_| Ok(()))
        .expect("foreign keys")
        .count();
    assert_eq!(violations, 0);
    assert!(db.kernel_outbox().verify_audit_chain().expect("verify receipt chain").valid);
}

#[test]
fn process_death_before_commit_discards_uncommitted_wal_pages() {
    if let Some(path) = std::env::var_os(STORE_ENV) {
        let db = database(Path::new(&path));
        let mut conn = db.conn().expect("connection");
        conn.execute_batch("PRAGMA cache_size = 1; PRAGMA cache_spill = ON;").expect("force spill");
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("begin");
        tx.execute("UPDATE inventory_balances SET quantity_allocated = '0.125', quantity_available = '9.875'", []).expect("uncommitted stock");
        // Force uncommitted dirty pages into the WAL, rather than merely
        // killing a process with changes still in its application cache.
        tx.execute_batch("CREATE TABLE crash_payload (payload BLOB); INSERT INTO crash_payload VALUES (zeroblob(2097152));").expect("spill pages");
        let mut wal = path;
        wal.push("-wal");
        assert!(std::fs::metadata(wal).expect("uncommitted WAL").len() > 1024 * 1024);
        await_termination();
    }
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("store.db");
    seed(&path, Decimal::TEN);
    kill_at_boundary(&mut spawn(
        "process_death_before_commit_discards_uncommitted_wal_pages",
        &path,
    ));
    let db = database(&path);
    let stock = db.inventory().get_stock(SKU).expect("stock").expect("item");
    assert_eq!(stock.total_on_hand, Decimal::TEN);
    assert_eq!(stock.total_allocated, Decimal::ZERO);
    assert_eq!(stock.total_available, Decimal::TEN);
    let table_count: i64 = db
        .conn()
        .expect("connection")
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'crash_payload'", [], |row| {
            row.get(0)
        })
        .expect("table count");
    assert_eq!(table_count, 0);
    integrity(&db);
    println!(
        "process-recovery-proof boundary=before_commit stock=unchanged uncommitted=discarded integrity=ok"
    );
}

#[test]
fn process_death_after_commit_replays_receipt_without_duplicate_effects() {
    const KEY: &str = "lost-response-reservation";
    if let Some(path) = std::env::var_os(STORE_ENV) {
        let db = database(Path::new(&path));
        let receipt =
            db.kernel_executor(policy()).execute_reserve_inventory(&command(KEY)).expect("commit");
        assert_eq!(receipt.status, ExecutionStatus::Succeeded);
        // The parent receives a synchronization marker, not the receipt. The
        // caller therefore has an ambiguous result when this worker dies.
        await_termination();
    }
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("store.db");
    seed(&path, Decimal::TEN);
    kill_at_boundary(&mut spawn(
        "process_death_after_commit_replays_receipt_without_duplicate_effects",
        &path,
    ));
    let db = database(&path);
    let original = db
        .kernel_outbox()
        .receipt_by_idempotency_key(KEY)
        .expect("lookup")
        .expect("durable receipt");
    let replay =
        db.kernel_executor(policy()).execute_reserve_inventory(&command(KEY)).expect("retry");
    assert_eq!(serde_json::to_value(&replay).expect("receipt JSON"), original.receipt);
    let stock = db.inventory().get_stock(SKU).expect("stock").expect("item");
    assert_eq!(stock.total_allocated, Decimal::new(125, 3));
    assert_eq!(stock.total_available, Decimal::new(9875, 3));
    let reservations =
        db.inventory().list_reservations_by_reference("recovery-drill", KEY).expect("reservations");
    assert_eq!(reservations.len(), 1);
    let governed_events: i64 = db
        .conn()
        .expect("connection")
        .query_row("SELECT COUNT(*) FROM kernel_outbox WHERE idempotency_key = ?", [KEY], |row| {
            row.get(0)
        })
        .expect("outbox count");
    assert_eq!(governed_events, 1);
    integrity(&db);
    println!(
        "process-recovery-proof boundary=after_commit receipt=replayed reservations=1 events=1 integrity=ok"
    );
}

#[test]
fn multiple_processes_preserve_exact_stock_and_durable_retry_identity() {
    let keys = std::env::var("STATESET_RECOVERY_KEYS")
        .map(|value| value.parse::<u32>().expect("integer key count"))
        .unwrap_or(64);
    assert!((1..=100_000).contains(&keys), "key count must be 1..100000");
    if let Some(path) = std::env::var_os(STORE_ENV) {
        let mut start = String::new();
        std::io::stdin().read_line(&mut start).expect("start barrier");
        assert_eq!(start.trim(), "go");
        let db = database(Path::new(&path));
        let executor = db.kernel_executor(policy());
        for index in 0..keys {
            let result = executor
                .execute_reserve_inventory(&command(&format!("load-reservation-{index}")))
                .expect("reserve/replay");
            assert_eq!(result.status, ExecutionStatus::Succeeded);
        }
        return;
    }
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("store.db");
    let expected = Decimal::from(keys) * Decimal::new(125, 3);
    seed(&path, expected);
    let started = Instant::now();
    let mut workers: Vec<_> = (0..4)
        .map(|_| spawn("multiple_processes_preserve_exact_stock_and_durable_retry_identity", &path))
        .collect();
    for worker in &mut workers {
        worker.0.stdin.as_mut().expect("worker stdin").write_all(b"go\n").expect("release barrier");
    }
    for worker in &mut workers {
        let stdout = worker.0.stdout.take().expect("stdout");
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let output: Vec<_> = BufReader::new(stdout).lines().collect();
            let _ = sender.send(output);
        });
        let output =
            receiver.recv_timeout(Duration::from_secs(180)).expect("bounded workload duration");
        assert!(worker.0.wait().expect("worker status").success(), "worker failed: {output:?}");
        reader.join().expect("reader");
    }
    let db = database(&path);
    let stock = db.inventory().get_stock(SKU).expect("stock").expect("item");
    assert_eq!(stock.total_on_hand, expected);
    assert_eq!(stock.total_allocated, expected);
    assert_eq!(stock.total_available, Decimal::ZERO);
    let conn = db.conn().expect("connection");
    for table in ["inventory_reservations", "kernel_receipts", "kernel_receipt_audit_log"] {
        let count: u32 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, keys, "{table}");
    }
    let events: u32 = conn
        .query_row("SELECT COUNT(*) FROM kernel_outbox WHERE tier = 'governed'", [], |row| {
            row.get(0)
        })
        .expect("events");
    assert_eq!(events, keys);
    drop(conn);
    integrity(&db);
    println!(
        "process-workload-proof workers=4 keys={keys} attempts={} effects={keys} replays={} elapsed_ms={} integrity=ok audit=valid",
        keys * 4,
        keys * 3,
        started.elapsed().as_millis()
    );
}
