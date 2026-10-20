#![cfg(feature = "postgres")]
#[path = "support/reservation_preview.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::{
    ConfirmInventoryReservation, CreateInventoryItem, ExecutionMode, ReleaseInventoryReservation,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

#[tokio::test]
async fn reservation_preview_matches_lean_and_apply() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live preview conformance");
        return;
    };
    let db = PostgresDatabase::connect(url).await.expect("connect and migrate");
    let executor = db.kernel_executor(support::policy());
    for case in support::cases() {
        let id = Uuid::new_v4();
        let item = db
            .inventory()
            .create_item_async(CreateInventoryItem {
                sku: format!("PREVIEW-{id}"),
                name: "Formal preview".into(),
                initial_quantity: Some(Decimal::from(6)),
                ..Default::default()
            })
            .await
            .expect("item");
        let expiry: chrono::DateTime<chrono::Utc> =
            if case.due { "2000-01-01T00:00:00Z" } else { "2100-01-01T00:00:00Z" }
                .parse()
                .expect("expiry");
        sqlx::query("INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at) VALUES ($1, $2, 1, 2, $3, 'formal', 'preview', $4)")
            .bind(id).bind(item.id).bind(case.status.to_string()).bind(expiry).execute(db.pool()).await.expect("reservation");
        sqlx::query("UPDATE inventory_balances SET quantity_allocated = $1, quantity_available = $2, version = 10 WHERE item_id = $3")
            .bind(case.allocated()).bind(Decimal::from(6) - case.allocated()).bind(item.id).execute(db.pool()).await.expect("balance");
        let mut receipts = Vec::new();
        for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
            let receipt = if case.operation == "release" {
                executor
                    .execute_release_inventory_reservation_async(&support::command(
                        "inventory.reservation.release",
                        ReleaseInventoryReservation { reservation_id: id },
                        mode,
                    ))
                    .await
                    .expect("release")
            } else {
                executor
                    .execute_confirm_inventory_reservation_async(&support::command(
                        "inventory.reservation.confirm",
                        ConfirmInventoryReservation {
                            reservation_id: id,
                            quantity: (case.operation == "split").then_some(Decimal::ONE),
                        },
                        mode,
                    ))
                    .await
                    .expect("confirm")
            };
            let balance: (i32, Decimal) = sqlx::query_as(
                "SELECT version, quantity_allocated FROM inventory_balances WHERE item_id = $1",
            )
            .bind(item.id)
            .fetch_one(db.pool())
            .await
            .expect("balance");
            if mode == ExecutionMode::Preview {
                assert_eq!(balance, (10, case.allocated()), "preview wrote stock: {case:?}");
                let row: (String, Decimal) = sqlx::query_as(
                    "SELECT status, quantity FROM inventory_reservations WHERE id = $1",
                )
                .bind(id)
                .fetch_one(db.pool())
                .await
                .expect("source");
                assert_eq!(
                    row,
                    (case.status.to_string(), Decimal::from(2)),
                    "preview wrote reservation: {case:?}"
                );
            } else {
                assert_eq!(
                    balance,
                    (10 + case.delta, case.allocated() - Decimal::from(2 * case.delta)),
                    "{case:?}"
                );
            }
            receipts.push(receipt);
        }
        case.check(&receipts[0], &receipts[1]);
    }
}
