#![cfg(feature = "sqlite")]
#[path = "support/receipt_promotion.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::{ExecutionMode, ExecutionStatus, InventoryRepository, ReservationStatus};
use stateset_db::SqliteDatabase;
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
