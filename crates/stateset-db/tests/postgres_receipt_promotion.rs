#![cfg(feature = "postgres")]
#[path = "support/isolated_pg.rs"]
mod isolated_pg;
#[path = "support/receipt_promotion.rs"]
mod support;

use rust_decimal::Decimal;
use stateset_core::{ExecutionMode, ExecutionStatus, ReservationStatus};
use uuid::Uuid;

/// A private database: these tests verify the whole store-wide audit chain,
/// which concurrently running suites in a shared database can break.
async fn database() -> Option<isolated_pg::IsolatedPostgres> {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live receipt promotion");
        return None;
    };
    Some(isolated_pg::isolated_postgres(&url).await)
}

#[tokio::test]
async fn concurrent_preview_promotion_splits_exactly_once() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let sku = format!("PROMOTE-RACE-{}", Uuid::new_v4());
    let item = db.inventory().create_item_async(support::item(&sku)).await.unwrap();
    let source = db.inventory().reserve_async(support::reservation(&sku, 4)).await.unwrap();
    let executor = db.kernel_executor(support::policy(true));
    let mut command = support::command(source.id);
    assert_eq!(
        executor.execute_confirm_inventory_reservation_async(&command).await.unwrap().status,
        ExecutionStatus::Previewed
    );
    support::promote(&mut command);
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let executor = executor.clone();
            let mut command = command.clone();
            command.command_id = Uuid::new_v4();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                executor
                    .execute_confirm_inventory_reservation_async(&command)
                    .await
                    .expect("promote")
            })
        })
        .collect();
    barrier.wait().await;
    let mut receipts = Vec::new();
    for handle in handles {
        receipts.push(handle.await.unwrap());
    }
    assert_eq!(receipts[0].status, ExecutionStatus::Succeeded);
    assert_eq!(
        serde_json::to_value(&receipts[0]).unwrap(),
        serde_json::to_value(&receipts[1]).unwrap(),
        "both callers must receive the same sealed receipt"
    );
    assert_eq!(receipts[0].event_ids.len(), 1);
    let row: (String, Decimal) =
        sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
            .bind(source.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(row, ("pending".into(), Decimal::from(3)));
    let children: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_reservations WHERE item_id = $1 AND status = 'confirmed'",
    )
    .bind(item.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1' AND payload->>'source_reservation_id' = $1").bind(source.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!((children, events), (1, 1));
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        Decimal::from(4)
    );
    assert!(db.kernel_outbox().verify_audit_chain_async().await.unwrap().valid);
    isolated.drop_database().await;
}

#[tokio::test]
async fn promotion_rechecks_current_policy_and_version_and_keeps_rejections_durable() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    for scenario in ["policy", "version"] {
        let sku = format!("PROMOTE-{scenario}-{}", Uuid::new_v4());
        db.inventory().create_item_async(support::item(&sku)).await.unwrap();
        let source = db.inventory().reserve_async(support::reservation(&sku, 4)).await.unwrap();
        let allowed = db.kernel_executor(support::policy(true));
        let mut command = support::command(source.id);
        assert_eq!(
            allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap().status,
            ExecutionStatus::Previewed
        );
        if scenario == "version" {
            db.inventory().reserve_async(support::reservation(&sku, 1)).await.unwrap();
        }
        support::promote(&mut command);
        let receipt = db
            .kernel_executor(support::policy(scenario != "policy"))
            .execute_confirm_inventory_reservation_async(&command)
            .await
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
        let replay = allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap();
        assert_eq!(
            replay.receipt_id, receipt.receipt_id,
            "restored policy must not resurrect a rejected key"
        );
        let quantity: Decimal =
            sqlx::query_scalar("SELECT quantity FROM inventory_reservations WHERE id = $1")
                .bind(source.id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(quantity, Decimal::from(4));
        assert!(receipt.event_ids.is_empty());
        if scenario == "version" {
            command.expected_version = Some(3);
            let conflict =
                allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap();
            assert_eq!(conflict.error_code.as_deref(), Some("kernel.idempotency_conflict"));
        }
        command.idempotency_key = Uuid::new_v4().to_string();
        support::promote(&mut command);
        assert_eq!(
            allowed.execute_confirm_inventory_reservation_async(&command).await.unwrap().status,
            ExecutionStatus::Succeeded
        );
        let quantity: Decimal =
            sqlx::query_scalar("SELECT quantity FROM inventory_reservations WHERE id = $1")
                .bind(source.id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(quantity, Decimal::from(3));
        assert!(db.kernel_outbox().verify_audit_chain_async().await.unwrap().valid);
    }
    isolated.drop_database().await;
}

#[tokio::test]
async fn promotion_rechecks_expiry_without_replaying_the_preview_result() {
    let Some(isolated) = database().await else { return };
    let db = &isolated.db;
    let sku = format!("PROMOTE-EXPIRY-{}", Uuid::new_v4());
    db.inventory().create_item_async(support::item(&sku)).await.unwrap();
    let source = db.inventory().reserve_async(support::reservation(&sku, 4)).await.unwrap();
    let executor = db.kernel_executor(support::policy(true));
    let mut command = support::command(source.id);
    let preview = executor.execute_confirm_inventory_reservation_async(&command).await.unwrap();
    assert_eq!(preview.status, ExecutionStatus::Previewed);
    assert_eq!(preview.version_after, Some(2));
    // Move the deadline into the past deterministically while the balance's
    // optimistic version stays unchanged; apply must re-evaluate expiry.
    sqlx::query(
        "UPDATE inventory_reservations SET expires_at = '2000-01-01T00:00:00Z' WHERE id = $1",
    )
    .bind(source.id)
    .execute(db.pool())
    .await
    .unwrap();
    support::promote(&mut command);
    // Scope the injected fault to this fixture; other tests share the outbox.
    let trigger = format!("formal_expiry_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected expiry outbox failure'; END $$")).execute(db.pool()).await.unwrap();
    sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON kernel_outbox FOR EACH ROW WHEN (NEW.event_type = 'inventory.reservation_expired.v1' AND NEW.aggregate_id = '{}') EXECUTE FUNCTION {trigger}()", source.id)).execute(db.pool()).await.unwrap();
    let failed = executor.execute_confirm_inventory_reservation_async(&command).await;
    sqlx::query(&format!("DROP TRIGGER {trigger} ON kernel_outbox"))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    assert!(failed.is_err(), "expiry must roll back when its event cannot be recorded");
    let status: String =
        sqlx::query_scalar("SELECT status FROM inventory_reservations WHERE id = $1")
            .bind(source.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        Decimal::from(4)
    );
    let mut retained_preview = command.clone();
    retained_preview.mode = ExecutionMode::Preview;
    assert_eq!(
        executor
            .execute_confirm_inventory_reservation_async(&retained_preview)
            .await
            .unwrap()
            .receipt_id,
        preview.receipt_id
    );
    let applied = executor.execute_confirm_inventory_reservation_async(&command).await.unwrap();
    assert_eq!(applied.status, ExecutionStatus::Succeeded);
    assert_eq!(applied.result.as_ref().unwrap().status, ReservationStatus::Expired);
    assert_eq!(applied.version_after, Some(3));
    support::promote(&mut command);
    assert_eq!(
        executor.execute_confirm_inventory_reservation_async(&command).await.unwrap().receipt_id,
        applied.receipt_id
    );
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        Decimal::ZERO
    );
    let confirmations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_confirmed.v1' AND (aggregate_id = $1 OR payload->>'source_reservation_id' = $1)").bind(source.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(confirmations, 0);
    assert_eq!(applied.event_ids.len(), 1);
    let event: (String, Uuid, String, serde_json::Value) = sqlx::query_as(
        "SELECT event_type, command_id, idempotency_key, payload FROM kernel_outbox WHERE id = $1",
    )
    .bind(applied.event_ids[0])
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(event.0, "inventory.reservation_expired.v1");
    assert_eq!(event.1, applied.command_id);
    assert_eq!(event.2, command.idempotency_key);
    assert_eq!(event.3["reservation_id"], source.id.to_string());
    assert_eq!(event.3["quantity"], "4");
    assert_eq!(event.3["balance_version"], 3);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_expired.v1' AND aggregate_id = $1").bind(source.id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 1, "replay must not emit a second expiry event");
    isolated.drop_database().await;
}
