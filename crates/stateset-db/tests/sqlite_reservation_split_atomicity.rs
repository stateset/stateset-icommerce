#![cfg(feature = "sqlite")]
//! Fault injection for the atomic split assumed by Lean and ReservationSplits.tla.
use rust_decimal_macros::dec;
use stateset_core::{
    CommandEnvelope, ConfirmInventoryReservation, CreateInventoryItem, ExecutionMode,
    ExecutionStatus, InventoryRepository, KernelCommandPolicy, KernelPolicy, KernelPrincipal,
    PrincipalKind, ReserveInventory,
};
use stateset_db::SqliteDatabase;

#[test]
fn split_confirmation_rolls_back_on_child_or_receipt_failure() {
    for failure_table in ["inventory_reservations", "kernel_receipts"] {
        let db = SqliteDatabase::in_memory().expect("database");
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: "FORMAL-SPLIT".into(),
                name: "Atomic split".into(),
                initial_quantity: Some(dec!(5)),
                ..Default::default()
            })
            .expect("stock");
        let source = db
            .inventory()
            .reserve(ReserveInventory {
                sku: "FORMAL-SPLIT".into(),
                quantity: dec!(3),
                location_id: Some(1),
                reference_type: "order".into(),
                reference_id: "formal-source".into(),
                expires_in_seconds: None,
            })
            .expect("reserve");
        let policy = KernelPolicy::new("formal-v1").allow(
            "inventory.reservation.confirm",
            KernelCommandPolicy::requiring(["inventory.reservation.confirm"]),
        );
        let mut command = CommandEnvelope::preview(
            "inventory.reservation.confirm",
            "formal-split-1",
            KernelPrincipal {
                id: "agent:formal-test".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("formal-tenant".into()),
                delegated_by: Some("user:formal-test".into()),
                capabilities: vec!["inventory.reservation.confirm".into()],
            },
            ConfirmInventoryReservation { reservation_id: source.id, quantity: Some(dec!(1)) },
        );
        command.mode = ExecutionMode::Apply;
        command.store_id = Some("formal-store".into());
        command.policy_version = Some("formal-v1".into());
        {
            let conn = db.pool().get().expect("connection");
            // Table names are fixed test constants, never external input.
            conn.execute_batch(&format!(
                "CREATE TRIGGER formal_abort BEFORE INSERT ON {failure_table}
                 BEGIN SELECT RAISE(ABORT, 'injected formal split failure'); END;"
            ))
            .expect("failure trigger");
        }
        let failed =
            db.kernel_executor(policy.clone()).execute_confirm_inventory_reservation(&command);
        assert!(failed.is_err(), "{failure_table} failure must abort: {failed:?}");
        let retained = db.inventory().get_reservation(source.id).expect("row").expect("source");
        assert_eq!(retained.quantity, dec!(3));
        assert_eq!(retained.status, source.status);
        let stock = db.inventory().get_stock("FORMAL-SPLIT").expect("stock").expect("item");
        assert_eq!(stock.total_allocated, dec!(3));
        assert_eq!(stock.total_available, dec!(2));
        {
            let conn = db.pool().get().expect("connection");
            let children: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM inventory_reservations WHERE status = 'confirmed'",
                    [],
                    |r| r.get(0),
                )
                .expect("children");
            let events: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1'", [], |r| r.get(0),
            ).expect("events");
            let receipts: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM kernel_receipts WHERE idempotency_key = 'formal-split-1'",
                    [],
                    |r| r.get(0),
                )
                .expect("receipts");
            assert_eq!((children, events, receipts), (0, 0, 0));
            conn.execute_batch("DROP TRIGGER formal_abort").expect("remove fault");
        }
        let receipt = db
            .kernel_executor(policy)
            .execute_confirm_inventory_reservation(&command)
            .expect("same command succeeds after rollback");
        assert_eq!(receipt.status, ExecutionStatus::Succeeded);
        assert_eq!(receipt.result.expect("child").quantity, dec!(1));
        assert_eq!(
            db.inventory().get_reservation(source.id).expect("row").expect("source").quantity,
            dec!(2)
        );
    }
}
