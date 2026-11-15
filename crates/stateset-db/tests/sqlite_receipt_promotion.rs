#![cfg(feature = "sqlite")]
#[path = "support/audit_correspondence.rs"]
mod audit;
#[path = "support/receipt_promotion.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::{ExecutionMode, ExecutionStatus, InventoryRepository, ReservationStatus};
use stateset_db::SqliteDatabase;
use stateset_db::kernel_outbox::{KernelAuditFailure, KernelAuditField, KernelAuditVerification};
use uuid::Uuid;

#[test]
fn concurrent_preview_promotion_splits_exactly_once() {
    let db = SqliteDatabase::in_memory().expect("database");
    db.inventory().create_item(support::item("PROMOTE-RACE")).expect("item");
    let source = db.inventory().reserve(support::reservation("PROMOTE-RACE", 4)).expect("hold");
    let executor = db.kernel_executor(support::policy(true));
    let mut command = support::command(source.id);
    assert_eq!(
        executor.execute_confirm_inventory_reservation(&command).unwrap().status,
        ExecutionStatus::Previewed
    );
    support::promote(&mut command);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let executor = executor.clone();
            let mut command = command.clone();
            command.command_id = Uuid::new_v4();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                executor.execute_confirm_inventory_reservation(&command).expect("promote")
            })
        })
        .collect();
    barrier.wait();
    let receipts: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(receipts[0].status, ExecutionStatus::Succeeded);
    assert_eq!(
        serde_json::to_value(&receipts[0]).unwrap(),
        serde_json::to_value(&receipts[1]).unwrap(),
        "both callers must receive the same sealed receipt"
    );
    assert_eq!(receipts[0].event_ids.len(), 1);
    let row = db.inventory().get_reservation(source.id).unwrap().unwrap();
    assert_eq!((row.status, row.quantity), (ReservationStatus::Pending, Decimal::from(3)));
    let conn = db.pool().get().unwrap();
    let (children, events): (i64, i64) = conn.query_row("SELECT (SELECT COUNT(*) FROM inventory_reservations WHERE status = 'confirmed'), (SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1')", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((children, events), (1, 1));
    assert_eq!(
        db.inventory().get_stock("PROMOTE-RACE").unwrap().unwrap().total_allocated,
        Decimal::from(4)
    );
    assert!(db.kernel_outbox().verify_audit_chain().unwrap().valid);
}

#[test]
fn promotion_rechecks_current_policy_and_version_and_keeps_rejections_durable() {
    for scenario in ["policy", "version"] {
        let db = SqliteDatabase::in_memory().expect("database");
        db.inventory().create_item(support::item(scenario)).unwrap();
        let source = db.inventory().reserve(support::reservation(scenario, 4)).unwrap();
        let allowed = db.kernel_executor(support::policy(true));
        let mut command = support::command(source.id);
        assert_eq!(
            allowed.execute_confirm_inventory_reservation(&command).unwrap().status,
            ExecutionStatus::Previewed
        );
        if scenario == "version" {
            db.inventory().reserve(support::reservation(scenario, 1)).unwrap();
        }
        support::promote(&mut command);
        let receipt = db
            .kernel_executor(support::policy(scenario != "policy"))
            .execute_confirm_inventory_reservation(&command)
            .unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Rejected, "{scenario}");
        assert_eq!(
            receipt.error_code.as_deref(),
            Some(if scenario == "policy" {
                "kernel.policy_denied"
            } else {
                "kernel.version_conflict"
            })
        );
        support::promote(&mut command);
        let replay = allowed.execute_confirm_inventory_reservation(&command).unwrap();
        assert_eq!(
            replay.receipt_id, receipt.receipt_id,
            "restored policy must not resurrect a rejected key"
        );
        assert_eq!(
            db.inventory().get_reservation(source.id).unwrap().unwrap().quantity,
            Decimal::from(4)
        );
        assert!(receipt.event_ids.is_empty());
        if scenario == "version" {
            command.expected_version = Some(3);
            let conflict = allowed.execute_confirm_inventory_reservation(&command).unwrap();
            assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));
        }
        command.idempotency_key = Uuid::new_v4().to_string();
        support::promote(&mut command);
        assert_eq!(
            allowed.execute_confirm_inventory_reservation(&command).unwrap().status,
            ExecutionStatus::Succeeded
        );
        assert_eq!(
            db.inventory().get_reservation(source.id).unwrap().unwrap().quantity,
            Decimal::from(3)
        );
        assert!(db.kernel_outbox().verify_audit_chain().unwrap().valid);
    }
}

#[test]
fn promotion_rechecks_expiry_without_replaying_the_preview_result() {
    let db = SqliteDatabase::in_memory().expect("database");
    db.inventory().create_item(support::item("EXPIRY")).unwrap();
    let source = db.inventory().reserve(support::reservation("EXPIRY", 4)).unwrap();
    let executor = db.kernel_executor(support::policy(true));
    let mut command = support::command(source.id);
    let preview = executor.execute_confirm_inventory_reservation(&command).unwrap();
    assert_eq!(preview.status, ExecutionStatus::Previewed);
    assert_eq!(preview.version_after, Some(2));
    // Deterministic expiry boundary: change only the timestamp, without a sleep
    // or a balance version change. Apply must re-read it under the transaction.
    db.pool()
        .get()
        .unwrap()
        .execute(
            "UPDATE inventory_reservations SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?",
            [source.id.to_string()],
        )
        .unwrap();
    support::promote(&mut command);
    db.pool()
        .get()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_expiry_event BEFORE INSERT ON kernel_outbox
         WHEN NEW.event_type = 'inventory.reservation_expired.v1'
         BEGIN SELECT RAISE(ABORT, 'injected expiry outbox failure'); END;",
        )
        .unwrap();
    let failed = executor.execute_confirm_inventory_reservation(&command);
    db.pool().get().unwrap().execute_batch("DROP TRIGGER reject_expiry_event").unwrap();
    assert!(failed.is_err(), "expiry must roll back when its event cannot be recorded");
    assert_eq!(
        db.inventory().get_reservation(source.id).unwrap().unwrap().status,
        ReservationStatus::Pending
    );
    assert_eq!(
        db.inventory().get_stock("EXPIRY").unwrap().unwrap().total_allocated,
        Decimal::from(4)
    );
    let mut retained_preview = command.clone();
    retained_preview.mode = ExecutionMode::Preview;
    assert_eq!(
        executor.execute_confirm_inventory_reservation(&retained_preview).unwrap().receipt_id,
        preview.receipt_id
    );
    let applied = executor.execute_confirm_inventory_reservation(&command).unwrap();
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().unwrap().status, ReservationStatus::Expired);
    assert_eq!(applied.version_after, Some(3));
    support::promote(&mut command);
    assert_eq!(
        executor.execute_confirm_inventory_reservation(&command).unwrap().receipt_id,
        applied.receipt_id
    );
    assert_eq!(db.inventory().get_stock("EXPIRY").unwrap().unwrap().total_allocated, Decimal::ZERO);
    let conn = db.pool().get().unwrap();
    let confirmations: i64 = conn.query_row("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1'", [], |r| r.get(0)).unwrap();
    assert_eq!(confirmations, 0);
    assert_eq!(applied.event_ids.len(), 1);
    let event: (String, String, String, String) = conn.query_row(
        "SELECT event_type, command_id, idempotency_key, payload FROM kernel_outbox WHERE id = ?",
        [applied.event_ids[0].to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).unwrap();
    assert_eq!(event.0, "inventory.reservation_expired.v1");
    assert_eq!(event.1, applied.command_id.to_string());
    assert_eq!(event.2, command.idempotency_key);
    let payload: serde_json::Value = serde_json::from_str(&event.3).unwrap();
    assert_eq!(payload["reservation_id"], source.id.to_string());
    assert_eq!(payload["quantity"], "4");
    assert_eq!(payload["balance_version"], 3);
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_expired.v1'", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1, "replay must not emit a second expiry event");
}

// --- Receipt audit correspondence -------------------------------------------
//
// `kernel_receipts` is the idempotent-replay index: deleting, re-keying or
// rolling back a row lets a retry under the original key execute again. The
// chain verifier must therefore check the index against the sealed entries,
// not only the surviving rows against the chain.

/// Run the shared scenario (see `support/audit_correspondence.rs`).
fn audited_store() -> (SqliteDatabase, audit::Keys) {
    let db = SqliteDatabase::in_memory().expect("database");
    let keys = audit::Keys::new();
    let executor = db.kernel_executor(audit::allowing());
    let preview = executor.execute_create_cart(&audit::preview(&keys.promoted)).unwrap();
    assert_eq!(preview.status, ExecutionStatus::Previewed);
    let promoted = executor.execute_create_cart(&audit::apply(&keys.promoted)).unwrap();
    assert_eq!(promoted.status, ExecutionStatus::Succeeded);
    let replay = executor.execute_create_cart(&audit::apply(&keys.promoted)).unwrap();
    assert_eq!(replay.receipt_id, promoted.receipt_id, "a replay appends nothing");
    let preview_only = executor.execute_create_cart(&audit::preview(&keys.preview_only)).unwrap();
    assert_eq!(preview_only.status, ExecutionStatus::Previewed);
    let rejected = db
        .kernel_executor(audit::denying())
        .execute_create_cart(&audit::apply(&keys.rejected))
        .unwrap();
    assert_eq!(rejected.status, ExecutionStatus::Rejected);
    let applied = executor.execute_create_cart(&audit::apply(&keys.applied)).unwrap();
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    let conflict = executor.execute_create_cart(&audit::conflicting(&keys.applied)).unwrap();
    assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));
    (db, keys)
}

fn carts_for(db: &SqliteDatabase, key: &str) -> i64 {
    db.pool()
        .get()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM carts WHERE customer_email = ?",
            [audit::email(key)],
            |row| row.get(0),
        )
        .unwrap()
}

/// Audit sequences sealed under `key`, oldest first.
fn sequences(db: &SqliteDatabase, key: &str) -> Vec<i64> {
    let conn = db.pool().get().unwrap();
    let mut statement = conn
        .prepare(
            "SELECT sequence FROM kernel_receipt_audit_log WHERE idempotency_key = ?
             ORDER BY sequence",
        )
        .unwrap();
    statement.query_map([key], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
}

fn execute(db: &SqliteDatabase, sql: &str, params: impl rusqlite::Params) {
    assert_eq!(db.pool().get().unwrap().execute(sql, params).unwrap(), 1, "{sql}");
}

fn verify(db: &SqliteDatabase) -> KernelAuditVerification {
    db.kernel_outbox().verify_audit_chain().expect("verify chain")
}

/// Assert `expected` is the reported failure and the chain cannot checkpoint.
fn assert_failure(db: &SqliteDatabase, expected: KernelAuditFailure) {
    let verification = verify(db);
    assert!(!verification.valid, "{verification:?}");
    assert_eq!(verification.failure.as_ref(), Some(&expected));
    assert_eq!(verification.first_invalid_sequence, Some(expected.sequence().unwrap_or(0)));
    assert!(db.kernel_outbox().audit_checkpoint().is_err(), "an invalid chain must not checkpoint");
}

#[test]
fn audit_correspondence_holds_across_preview_promotion_rejection_and_replay() {
    let (db, keys) = audited_store();
    let verification = verify(&db);
    assert!(verification.valid, "{verification:?}");
    assert_eq!(verification.entries, audit::SCENARIO_ENTRIES);
    assert_eq!(verification.failure, None);
    assert_eq!(verification.first_invalid_sequence, None);
    let rows: i64 = db
        .pool()
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM kernel_receipts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, audit::SCENARIO_ROWS, "the promoted preview is superseded, not materialized");
    assert_eq!(sequences(&db, &keys.promoted).len(), 2);
    for key in [&keys.preview_only, &keys.rejected, &keys.applied] {
        assert_eq!(sequences(&db, key).len(), 1, "{key}");
    }
    let carts: Vec<i64> = [&keys.promoted, &keys.preview_only, &keys.rejected, &keys.applied]
        .into_iter()
        .map(|key| carts_for(&db, key))
        .collect();
    assert_eq!(carts, [1, 0, 0, 1], "only applied receipts created carts");
    let checkpoint = db.kernel_outbox().audit_checkpoint().expect("checkpoint");
    assert!(db.kernel_outbox().verify_audit_checkpoint(&checkpoint).unwrap());
}

#[test]
fn audit_detects_a_deleted_materialized_receipt_and_the_re_execution_it_allows() {
    let (db, keys) = audited_store();
    let sealed = sequences(&db, &keys.applied)[0];
    execute(&db, "DELETE FROM kernel_receipts WHERE idempotency_key = ?", [&keys.applied]);
    assert_failure(
        &db,
        KernelAuditFailure::MissingMaterializedReceipt {
            sequence: sealed,
            idempotency_key: keys.applied.clone(),
        },
    );

    // The deleted row was the replay index, so the same key executes again —
    // a second cart. The chain now records that re-execution.
    let retry = db
        .kernel_executor(audit::allowing())
        .execute_create_cart(&audit::apply(&keys.applied))
        .unwrap();
    assert_eq!(retry.status, ExecutionStatus::Succeeded);
    assert_eq!(carts_for(&db, &keys.applied), 2, "the hazard this check exists for");
    let resealed = sequences(&db, &keys.applied);
    assert_eq!(resealed.len(), 2);
    assert_failure(
        &db,
        KernelAuditFailure::InvalidSupersession {
            sequence: resealed[1],
            superseded_sequence: sealed,
            idempotency_key: keys.applied,
        },
    );
}

#[test]
fn audit_detects_a_re_keyed_materialized_receipt() {
    let (db, keys) = audited_store();
    let sealed = sequences(&db, &keys.applied)[0];
    execute(
        &db,
        "UPDATE kernel_receipts SET idempotency_key = ? WHERE idempotency_key = ?",
        [format!("{}-rekeyed", keys.applied), keys.applied.clone()],
    );
    assert_failure(
        &db,
        KernelAuditFailure::MaterializedReceiptMismatch {
            sequence: sealed,
            idempotency_key: keys.applied,
            field: KernelAuditField::IdempotencyKey,
        },
    );
}

/// A tamper of one materialized receipt and the field it must be caught on.
type ReceiptTamper = (&'static str, fn(&audit::Keys) -> &String, KernelAuditField);

#[test]
fn audit_detects_an_altered_materialized_receipt() {
    let cases: [ReceiptTamper; 6] = [
        (
            "UPDATE kernel_receipts SET status = 'succeeded' WHERE idempotency_key = ?",
            |keys| &keys.rejected,
            KernelAuditField::Status,
        ),
        (
            "UPDATE kernel_receipts SET receipt = json_set(receipt, '$.error_code', 'kernel.none')
             WHERE idempotency_key = ?",
            |keys| &keys.rejected,
            KernelAuditField::Receipt,
        ),
        (
            "UPDATE kernel_receipts SET receipt = json_set(receipt, '$.status', 'rejected')
             WHERE idempotency_key = ?",
            |keys| &keys.applied,
            KernelAuditField::Receipt,
        ),
        (
            "UPDATE kernel_receipts SET request_hash = 'rebound' WHERE idempotency_key = ?",
            |keys| &keys.applied,
            KernelAuditField::RequestHash,
        ),
        (
            "UPDATE kernel_receipts SET command_type = 'carts.item.add'
             WHERE idempotency_key = ?",
            |keys| &keys.applied,
            KernelAuditField::CommandType,
        ),
        (
            "UPDATE kernel_receipts SET command_id = lower(hex(randomblob(16)))
             WHERE idempotency_key = ?",
            |keys| &keys.applied,
            KernelAuditField::CommandId,
        ),
    ];
    for (sql, key, field) in cases {
        let (db, keys) = audited_store();
        let key = key(&keys).clone();
        let sealed = *sequences(&db, &key).last().unwrap();
        execute(&db, sql, [&key]);
        assert_failure(
            &db,
            KernelAuditFailure::MaterializedReceiptMismatch {
                sequence: sealed,
                idempotency_key: key,
                field,
            },
        );
    }

    // Restoring the superseded preview as the materialized receipt would let
    // the promotion run again.
    let (db, keys) = audited_store();
    let [preview, promoted] = sequences(&db, &keys.promoted)[..] else {
        panic!("a promoted key seals two entries")
    };
    let (audit_hash, receipt): (String, String) = db
        .pool()
        .get()
        .unwrap()
        .query_row(
            "SELECT audit_hash, receipt FROM kernel_receipt_audit_log WHERE sequence = ?",
            [preview],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let mut sealed: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    sealed["audit_hash"] = serde_json::Value::String(audit_hash);
    execute(
        &db,
        "UPDATE kernel_receipts SET status = 'previewed', command_id = ?, receipt = ?
         WHERE idempotency_key = ?",
        [
            sealed["command_id"].as_str().unwrap().to_owned(),
            sealed.to_string(),
            keys.promoted.clone(),
        ],
    );
    assert_failure(
        &db,
        KernelAuditFailure::StaleMaterializedReceipt {
            sequence: promoted,
            materialized_sequence: preview,
            idempotency_key: keys.promoted,
        },
    );
}

#[test]
fn audit_detects_orphan_materialized_receipts() {
    let (db, _) = audited_store();
    let now = chrono::Utc::now().to_rfc3339();
    execute(
        &db,
        "INSERT INTO kernel_receipts (command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at)
         VALUES (?, 'audit-orphan', 'payments.create', '1.0', 'h', 'succeeded',
                 '{\"audit_hash\":\"0000\"}', ?, ?)",
        [Uuid::new_v4().to_string(), now.clone(), now],
    );
    assert_failure(
        &db,
        KernelAuditFailure::OrphanMaterializedReceipt {
            idempotency_key: "audit-orphan".into(),
            claimed_audit_hash: Some("0000".into()),
        },
    );

    // A copy of a real receipt filed under a second key: the sealed entry now
    // has two materialized receipts.
    let (db, keys) = audited_store();
    let sealed = sequences(&db, &keys.applied)[0];
    execute(
        &db,
        "INSERT INTO kernel_receipts (command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at)
         SELECT ?, 'audit-copy', command_type, contract_version, request_hash, status, receipt,
                created_at, completed_at
         FROM kernel_receipts WHERE idempotency_key = ?",
        [Uuid::new_v4().to_string(), keys.applied.clone()],
    );
    assert_failure(
        &db,
        KernelAuditFailure::MaterializedReceiptMismatch {
            sequence: sealed,
            idempotency_key: keys.applied,
            field: KernelAuditField::IdempotencyKey,
        },
    );
}

#[test]
fn audit_detects_a_rewritten_unhashed_audit_column() {
    // `idempotency_key`, `command_id` and `receipt_id` on the audit log are
    // outside the hash, so the links still verify; they are checked against
    // the receipt they index instead.
    let (db, keys) = audited_store();
    let sealed = sequences(&db, &keys.applied)[0];
    execute(
        &db,
        "UPDATE kernel_receipt_audit_log SET idempotency_key = 'elsewhere' WHERE sequence = ?",
        [sealed],
    );
    assert_failure(
        &db,
        KernelAuditFailure::InvalidEntryField {
            sequence: sealed,
            field: KernelAuditField::IdempotencyKey,
        },
    );
    assert_eq!(verify(&db).entries, audit::SCENARIO_ENTRIES, "every link still verifies");
}
