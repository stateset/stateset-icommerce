#![cfg(feature = "sqlite")]
#[path = "support/reservation_preview.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::{
    ConfirmInventoryReservation, CreateInventoryItem, ExecutionMode, InventoryRepository,
    ReleaseInventoryReservation,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

#[test]
fn reservation_preview_matches_lean_and_apply() {
    let db = SqliteDatabase::in_memory().expect("database");
    let executor = db.kernel_executor(support::policy());
    for case in support::cases() {
        let id = Uuid::new_v4();
        let item = db
            .inventory()
            .create_item(CreateInventoryItem {
                sku: format!("PREVIEW-{id}"),
                name: "Formal preview".into(),
                initial_quantity: Some(Decimal::from(6)),
                ..Default::default()
            })
            .expect("item");
        let expiry = if case.due { "2000-01-01T00:00:00Z" } else { "2100-01-01T00:00:00Z" };
        {
            let conn = db.pool().get().expect("connection");
            conn.execute("INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at) VALUES (?, ?, 1, '2', ?, 'formal', 'preview', ?)", rusqlite::params![id.to_string(), item.id, case.status.to_string(), expiry]).expect("reservation");
            conn.execute("UPDATE inventory_balances SET quantity_allocated = ?, quantity_available = ?, version = 10 WHERE item_id = ?", rusqlite::params![case.allocated().to_string(), (Decimal::from(6) - case.allocated()).to_string(), item.id]).expect("balance");
        }
        let balance = || {
            db.pool()
                .get()
                .expect("connection")
                .query_row(
                    "SELECT version, quantity_allocated FROM inventory_balances WHERE item_id = ?",
                    [item.id],
                    |r| {
                        Ok((
                            r.get::<_, i32>(0)?,
                            r.get::<_, String>(1)?.parse::<Decimal>().expect("decimal"),
                        ))
                    },
                )
                .expect("balance")
        };
        let execute = |mode| {
            if case.operation == "release" {
                executor
                    .execute_release_inventory_reservation(&support::command(
                        "inventory.reservation.release",
                        ReleaseInventoryReservation { reservation_id: id },
                        mode,
                    ))
                    .expect("release")
            } else {
                executor
                    .execute_confirm_inventory_reservation(&support::command(
                        "inventory.reservation.confirm",
                        ConfirmInventoryReservation {
                            reservation_id: id,
                            quantity: (case.operation == "split").then_some(Decimal::ONE),
                        },
                        mode,
                    ))
                    .expect("confirm")
            }
        };
        let preview = execute(ExecutionMode::Preview);
        assert_eq!(balance(), (10, case.allocated()), "preview wrote stock: {case:?}");
        let row = db.inventory().get_reservation(id).expect("row").expect("source");
        assert_eq!(
            (row.status, row.quantity),
            (case.status, Decimal::from(2)),
            "preview wrote reservation: {case:?}"
        );
        let applied = execute(ExecutionMode::Apply);
        case.check(&preview, &applied);
        assert_eq!(
            balance(),
            (10 + case.delta, case.allocated() - Decimal::from(2 * case.delta)),
            "{case:?}"
        );
    }
}
