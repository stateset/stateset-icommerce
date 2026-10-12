#![cfg(feature = "postgres")]
#[path = "support/shipment_atomicity.rs"]
mod support;

use rust_decimal::Decimal;
use serde_json::Value;
use stateset_core::*;
use stateset_db::PostgresDatabase;
use uuid::Uuid;

async fn wait_for_lock(
    db: &PostgresDatabase,
    name: &str,
) -> std::result::Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock')",
            ).bind(name).fetch_one(db.pool()).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await
}

/// A holder of the later reservation must still be able to acquire its
/// balance while shipment waits for that row. NOWAIT detects the closing edge
/// of a possible deadlock without depending on PostgreSQL's victim selection.
#[tokio::test]
async fn shipment_locks_all_reservations_before_balances() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping shipment lock-order check");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let name = format!("lock_order_{}", Uuid::new_v4().simple());
    let separator = if url.contains('?') { '&' } else { '?' };
    let ship_db = PostgresDatabase::connect_with_options(
        format!("{url}{separator}application_name={name}"),
        1,
        30,
    )
    .await
    .unwrap();
    let sku = format!("LOCK-ORDER-{}", Uuid::new_v4());
    let item = db.inventory().create_item_async(support::stock(&sku)).await.unwrap();
    let customer = db.customers().create_async(support::customer()).await.unwrap();
    let order = db
        .orders()
        .create_async(support::order(customer.id, &[sku.clone(), sku.clone()]))
        .await
        .unwrap();
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update_async(
                order.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .unwrap();
    }
    let payment = db.payments().create_async(support::payment(&order)).await.unwrap();
    db.payments().mark_completed_async(payment.id.into_uuid()).await.unwrap();
    let order = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
    let last: Uuid = sqlx::query_scalar("SELECT id FROM inventory_reservations WHERE reference_id = $1 ORDER BY item_id DESC, location_id DESC, id DESC LIMIT 1")
        .bind(order.id.to_string()).fetch_one(db.pool()).await.unwrap();
    let mut gate = db.pool().begin().await.unwrap();
    sqlx::query("SELECT id FROM inventory_reservations WHERE id = $1 FOR UPDATE")
        .bind(last)
        .fetch_one(gate.as_mut())
        .await
        .unwrap();
    let kernel = ship_db.kernel_executor(support::policy());
    let mut command = support::command(&order, false);
    command.mode = ExecutionMode::Apply;
    let worker =
        tokio::spawn(async move { kernel.execute_ship_reserved_order_async(&command).await });
    let waiting = wait_for_lock(&db, &name).await;
    let balance = sqlx::query("SELECT id FROM inventory_balances WHERE item_id = $1 AND location_id = 1 FOR UPDATE NOWAIT")
        .bind(item.id).fetch_one(gate.as_mut()).await;
    gate.rollback().await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), worker).await;
    ship_db.pool().close().await;
    waiting.expect("shipment reached the later reservation lock");
    let receipt = result.unwrap().unwrap().unwrap();
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{:?}", receipt.error_message);
    assert!(
        balance.is_ok(),
        "shipment held the balance while waiting for its reservation owner: {balance:?}"
    );
    let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
    assert_eq!(
        (stock.total_on_hand, stock.total_allocated, stock.total_available),
        (Decimal::from(5), Decimal::ZERO, Decimal::from(5))
    );
}

/// Sweep time is an explicit API argument: move its cutoff past the target's
/// deadline after shipment admission, without relying on wall-clock sleeps.
/// All assertions are scoped to this fixture, since the sweep itself is global.
#[tokio::test]
async fn shipment_expiry_interleavings_preserve_stock_and_recheck_deadlines() {
    use chrono::{Duration, Utc};
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping shipment/expiry races");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let names = [
        format!("expiry_ship_{}", Uuid::new_v4().simple()),
        format!("sweep_{}", Uuid::new_v4().simple()),
    ];
    let separator = if url.contains('?') { '&' } else { '?' };
    let ship_db = PostgresDatabase::connect_with_options(
        format!("{url}{separator}application_name={}", names[0]),
        1,
        30,
    )
    .await
    .unwrap();
    let sweep_db = PostgresDatabase::connect_with_options(
        format!("{url}{separator}application_name={}", names[1]),
        1,
        30,
    )
    .await
    .unwrap();
    for scenario in ["ship_first", "sweep_first", "deadline_changed"] {
        for partial in [true, false] {
            let suffix = Uuid::new_v4().simple().to_string();
            let sku = format!("SHIP-EXPIRY-{suffix}");
            let item = db.inventory().create_item_async(support::stock(&sku)).await.unwrap();
            let customer = db.customers().create_async(support::customer()).await.unwrap();
            let order = db
                .orders()
                .create_async(support::order(customer.id, &[sku.clone(), sku.clone()]))
                .await
                .unwrap();
            for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
                db.orders()
                    .update_async(
                        order.id.into_uuid(),
                        UpdateOrder { status: Some(status), ..Default::default() },
                    )
                    .await
                    .unwrap();
            }
            let payment = db.payments().create_async(support::payment(&order)).await.unwrap();
            db.payments().mark_completed_async(payment.id.into_uuid()).await.unwrap();
            let order = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
            let (reservation, line): (Uuid, Uuid) = sqlx::query_as("SELECT id, order_item_id FROM inventory_reservations WHERE reference_id = $1 ORDER BY item_id, location_id, id LIMIT 1")
                .bind(order.id.to_string()).fetch_one(db.pool()).await.unwrap();
            let target = order.items.iter().find(|i| i.id.into_uuid() == line).unwrap();
            let sibling = order.items.iter().find(|i| i.id != target.id).unwrap();
            let quantity = if partial { 1 } else { target.quantity };
            let deadline = Utc::now() + Duration::days(1);
            let cutoff = deadline + Duration::seconds(1);
            sqlx::query("UPDATE inventory_reservations SET expires_at = $1 WHERE id = $2")
                .bind(deadline)
                .bind(reservation)
                .execute(db.pool())
                .await
                .unwrap();
            let mut command = support::command(&order, false);
            command.mode = ExecutionMode::Apply;
            command.payload.lines =
                Some(vec![ShipmentLineInput { order_item_id: target.id, quantity }]);
            let kernel = ship_db.kernel_executor(support::policy());
            let ship_command = command.clone();
            let ship = async move { kernel.execute_ship_reserved_order_async(&ship_command).await };
            let mut gate = db.pool().begin().await.unwrap();
            let trigger = format!("expiry_gate_{suffix}");
            let key = i64::from_be_bytes(Uuid::new_v4().as_bytes()[..8].try_into().unwrap());
            if scenario == "ship_first" {
                // Pause after the shipment has passed admission, locked the
                // hold, and started writing, before its transaction commits.
                sqlx::query("SELECT pg_advisory_xact_lock($1)")
                    .bind(key)
                    .execute(gate.as_mut())
                    .await
                    .unwrap();
                sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({key}::bigint); RETURN NEW; END $$")).execute(db.pool()).await.unwrap();
                sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON inventory_transactions FOR EACH ROW WHEN (NEW.item_id = {} AND NEW.transaction_type = 'shipment') EXECUTE FUNCTION {trigger}()", item.id)).execute(db.pool()).await.unwrap();
            } else if scenario == "sweep_first" {
                // The sweep locks the reservation, then waits for its balance.
                sqlx::query("SELECT id FROM inventory_balances WHERE item_id = $1 AND location_id = 1 FOR UPDATE").bind(item.id).fetch_one(gate.as_mut()).await.unwrap();
            } else {
                // Keep the old live deadline visible until the shipment has
                // started and reached its locked reservation read.
                sqlx::query("SELECT id FROM inventory_reservations WHERE id = $1 FOR UPDATE")
                    .bind(reservation)
                    .fetch_one(gate.as_mut())
                    .await
                    .unwrap();
            }
            let sweep_worker = if scenario == "sweep_first" {
                let inventory = sweep_db.inventory();
                let worker = tokio::spawn(async move {
                    inventory.expire_reservations_async(cutoff, 10_000).await
                });
                let waiting = wait_for_lock(&db, &names[1]).await;
                if waiting.is_err() {
                    gate.rollback().await.unwrap();
                    worker.abort();
                    panic!("sweeper did not reach the balance lock");
                }
                Some(worker)
            } else {
                None
            };
            let worker = tokio::spawn(ship);
            let waiting = wait_for_lock(&db, &names[0]).await;
            if scenario == "deadline_changed" && waiting.is_ok() {
                // This deadline is later than command start but earlier than
                // resumed admission. No wall-clock sleep is necessary.
                sqlx::query("UPDATE inventory_reservations SET expires_at = clock_timestamp() WHERE id = $1")
                    .bind(reservation).execute(gate.as_mut()).await.unwrap();
            }
            let skip_result = if scenario != "sweep_first" && waiting.is_ok() {
                Some(
                    tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        sweep_db.inventory().expire_reservations_async(cutoff, 10_000),
                    )
                    .await,
                )
            } else {
                None
            };
            // Read-only MVCC observations while the gate is held: the sweeper
            // must neither close nor emit an event for the locked target.
            let before_unlock: (String, Decimal, i64) = sqlx::query_as("SELECT status, quantity, (SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = $1::text AND event_type = 'inventory.reservation_expired.v1') FROM inventory_reservations WHERE id = $2")
                .bind(reservation.to_string()).bind(reservation).fetch_one(db.pool()).await.unwrap();
            gate.commit().await.unwrap();
            let result = tokio::time::timeout(std::time::Duration::from_secs(30), worker).await;
            let swept = if let Some(worker) = sweep_worker {
                Some(tokio::time::timeout(std::time::Duration::from_secs(30), worker).await)
            } else {
                None
            };
            if scenario == "ship_first" {
                sqlx::query(&format!("DROP TRIGGER {trigger} ON inventory_transactions"))
                    .execute(db.pool())
                    .await
                    .unwrap();
                sqlx::query(&format!("DROP FUNCTION {trigger}()"))
                    .execute(db.pool())
                    .await
                    .unwrap();
            }
            waiting.expect("shipment reached the forced lock wait");
            if let Some(result) = skip_result {
                result.expect("SKIP LOCKED must finish before unlock").unwrap();
            }
            if let Some(result) = swept {
                result.unwrap().unwrap().unwrap();
            }
            assert_eq!(before_unlock, ("pending".into(), Decimal::from(target.quantity), 0));
            let receipt = result.unwrap().unwrap().unwrap();
            let shipped = if scenario == "ship_first" { quantity } else { 0 };
            assert_eq!(
                receipt.status,
                if scenario == "ship_first" {
                    ExecutionStatus::Succeeded
                } else {
                    ExecutionStatus::Rejected
                },
                "{scenario} partial={partial}: {:?}",
                receipt.error_message
            );
            if scenario != "ship_first" {
                assert_eq!(
                    receipt.error_code.as_deref(),
                    Some(if scenario == "deadline_changed" {
                        "commerce.reservation_expired"
                    } else {
                        "commerce.shipment_inventory_invalid"
                    })
                );
                assert!(receipt.event_ids.is_empty());
                assert_eq!(
                    db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap(),
                    order
                );
            }
            let updated = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
            assert_eq!(
                updated.items.iter().find(|i| i.id == target.id).unwrap().shipped_quantity,
                shipped
            );
            assert_eq!(
                updated.items.iter().find(|i| i.id == sibling.id).unwrap().shipped_quantity,
                0
            );
            let fulfilled_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = 'inventory.reservation_fulfilled.v1'")
                .bind(reservation.to_string()).fetch_one(db.pool()).await.unwrap();
            assert_eq!(fulfilled_events, i64::from(scenario == "ship_first"));
            // Revisit skipped rows, then prove repeated sweeps cannot expire
            // the same units or emit the target's expiry twice.
            db.inventory().expire_reservations_async(cutoff, 10_000).await.unwrap();
            let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
            assert_eq!(
                (stock.total_on_hand, stock.total_allocated, stock.total_available),
                (
                    Decimal::from(10 - shipped),
                    Decimal::from(sibling.quantity),
                    Decimal::from(10 - shipped - sibling.quantity)
                )
            );
            let row: (String, Decimal) =
                sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
                    .bind(reservation)
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            assert_eq!(
                row,
                if shipped == target.quantity {
                    ("fulfilled".into(), Decimal::from(target.quantity))
                } else {
                    ("expired".into(), Decimal::from(target.quantity - shipped))
                }
            );
            let sibling_row: (String, Decimal) = sqlx::query_as(
                "SELECT status, quantity FROM inventory_reservations WHERE order_item_id = $1",
            )
            .bind(sibling.id.into_uuid())
            .fetch_one(db.pool())
            .await
            .unwrap();
            assert_eq!(sibling_row, ("pending".into(), Decimal::from(sibling.quantity)));
            let expiry_payloads: Vec<Value> = sqlx::query_scalar("SELECT payload FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = 'inventory.reservation_expired.v1'").bind(reservation.to_string()).fetch_all(db.pool()).await.unwrap();
            assert_eq!(expiry_payloads.len(), usize::from(shipped != target.quantity));
            if let Some(payload) = expiry_payloads.first() {
                let expired: Decimal = payload["quantity"].as_str().unwrap().parse().unwrap();
                assert_eq!(expired, Decimal::from(target.quantity - shipped));
            }
            let ledger: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(quantity), 0) FROM inventory_transactions WHERE item_id = $1 AND transaction_type = 'shipment'").bind(item.id).fetch_one(db.pool()).await.unwrap();
            assert_eq!(ledger, -Decimal::from(shipped));
            let aggregates = [reservation.to_string(), order.id.to_string()];
            let before =
                snapshot(&db, &[item.id], &aggregates, order.id, &command.idempotency_key).await;
            db.inventory().expire_reservations_async(cutoff, 10_000).await.unwrap();
            command.command_id = Uuid::new_v4();
            let replay = db
                .kernel_executor(support::policy())
                .execute_ship_reserved_order_async(&command)
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(replay).unwrap(),
                serde_json::to_value(receipt).unwrap()
            );
            assert_eq!(
                snapshot(&db, &[item.id], &aggregates, order.id, &command.idempotency_key).await,
                before
            );
        }
    }
    ship_db.pool().close().await;
    sweep_db.pool().close().await;
}

/// Queue both operations at the same reservation lock, in each order. A
/// same-SKU sibling makes an erroneous second allocation decrement visible
/// even when the aggregate balance would remain nonnegative.
#[tokio::test]
async fn shipment_racing_release_uses_current_hold_and_preserves_sibling() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping shipment/release races");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let names = [
        format!("ship_{}", Uuid::new_v4().simple()),
        format!("release_{}", Uuid::new_v4().simple()),
    ];
    let separator = if url.contains('?') { '&' } else { '?' };
    let ship_db = PostgresDatabase::connect_with_options(
        format!("{url}{separator}application_name={}", names[0]),
        1,
        30,
    )
    .await
    .unwrap();
    let release_db = PostgresDatabase::connect_with_options(
        format!("{url}{separator}application_name={}", names[1]),
        1,
        30,
    )
    .await
    .unwrap();
    for ship_first in [true, false] {
        for partial in [true, false] {
            let sku = format!("SHIP-RELEASE-{}", Uuid::new_v4());
            let item = db.inventory().create_item_async(support::stock(&sku)).await.unwrap();
            let customer = db.customers().create_async(support::customer()).await.unwrap();
            let order = db
                .orders()
                .create_async(support::order(customer.id, &[sku.clone(), sku.clone()]))
                .await
                .unwrap();
            for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
                db.orders()
                    .update_async(
                        order.id.into_uuid(),
                        UpdateOrder { status: Some(status), ..Default::default() },
                    )
                    .await
                    .unwrap();
            }
            let payment = db.payments().create_async(support::payment(&order)).await.unwrap();
            db.payments().mark_completed_async(payment.id.into_uuid()).await.unwrap();
            let order = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
            // Gate the first reservation in the executor's lock order.
            let (reservation, line): (Uuid, Uuid) = sqlx::query_as("SELECT id, order_item_id FROM inventory_reservations WHERE reference_id = $1 ORDER BY item_id, location_id, id LIMIT 1")
                .bind(order.id.to_string()).fetch_one(db.pool()).await.unwrap();
            let target = order.items.iter().find(|i| i.id.into_uuid() == line).unwrap();
            let sibling = order.items.iter().find(|i| i.id != target.id).unwrap();
            let quantity = if partial { 1 } else { target.quantity };
            let mut command = support::command(&order, false);
            command.mode = ExecutionMode::Apply;
            command.payload.lines =
                Some(vec![ShipmentLineInput { order_item_id: target.id, quantity }]);
            let kernel = ship_db.kernel_executor(support::policy());
            let mut gate = db.pool().begin().await.unwrap();
            sqlx::query("SELECT id FROM inventory_reservations WHERE id = $1 FOR UPDATE")
                .bind(reservation)
                .fetch_one(gate.as_mut())
                .await
                .unwrap();
            let ship_command = command.clone();
            let ship = async move { kernel.execute_ship_reserved_order_async(&ship_command).await };
            let inventory = release_db.inventory();
            let release = async move { inventory.release_reservation_async(reservation).await };
            let (ship_worker, release_worker, first_wait, second_wait) = if ship_first {
                let a = tokio::spawn(ship);
                let first = wait_for_lock(&db, &names[0]).await;
                let b = tokio::spawn(release);
                let second = wait_for_lock(&db, &names[1]).await;
                (a, b, first, second)
            } else {
                let b = tokio::spawn(release);
                let first = wait_for_lock(&db, &names[1]).await;
                let a = tokio::spawn(ship);
                let second = wait_for_lock(&db, &names[0]).await;
                (a, b, first, second)
            };
            gate.rollback().await.unwrap();
            let receipt = tokio::time::timeout(std::time::Duration::from_secs(30), ship_worker)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(30), release_worker)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            first_wait.expect("first worker reached reservation lock");
            second_wait.expect("second worker reached reservation lock");
            let shipped = if ship_first { quantity } else { 0 };
            assert_eq!(
                receipt.status,
                if ship_first { ExecutionStatus::Succeeded } else { ExecutionStatus::Rejected },
                "ship_first={ship_first} partial={partial}: {:?}",
                receipt.error_message
            );
            if !ship_first {
                assert_eq!(
                    receipt.error_code.as_deref(),
                    Some("commerce.shipment_inventory_invalid")
                );
            }
            let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
            assert_eq!(
                (stock.total_on_hand, stock.total_allocated, stock.total_available),
                (
                    Decimal::from(10 - shipped),
                    Decimal::from(sibling.quantity),
                    Decimal::from(10 - shipped - sibling.quantity)
                )
            );
            let held: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(quantity), 0) FROM inventory_reservations WHERE item_id = $1 AND status IN ('pending', 'confirmed', 'allocated')").bind(item.id).fetch_one(db.pool()).await.unwrap();
            assert_eq!(held, stock.total_allocated);
            let sibling_hold: (String, Decimal) = sqlx::query_as(
                "SELECT status, quantity FROM inventory_reservations WHERE order_item_id = $1",
            )
            .bind(sibling.id.into_uuid())
            .fetch_one(db.pool())
            .await
            .unwrap();
            assert_eq!(sibling_hold, ("pending".into(), Decimal::from(sibling.quantity)));
            let target_hold: (String, Decimal) =
                sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
                    .bind(reservation)
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            assert_eq!(
                target_hold,
                if ship_first && !partial {
                    ("fulfilled".into(), Decimal::from(target.quantity))
                } else {
                    ("released".into(), Decimal::from(target.quantity - shipped))
                }
            );
            let updated = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
            assert_eq!(
                updated.items.iter().find(|i| i.id == target.id).unwrap().shipped_quantity,
                shipped
            );
            assert_eq!(
                updated.items.iter().find(|i| i.id == sibling.id).unwrap().shipped_quantity,
                0
            );
            let effects: (i64, i64) = sqlx::query_as("SELECT COUNT(*) FILTER (WHERE event_type = 'inventory.reservation_fulfilled.v1'), COUNT(*) FILTER (WHERE event_type = 'inventory.reservation_released.v1') FROM kernel_outbox WHERE aggregate_id = $1").bind(reservation.to_string()).fetch_one(db.pool()).await.unwrap();
            assert_eq!(effects, (i64::from(ship_first), i64::from(!ship_first || partial)));
            let ledger: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(quantity), 0) FROM inventory_transactions WHERE item_id = $1 AND transaction_type = 'shipment'").bind(item.id).fetch_one(db.pool()).await.unwrap();
            assert_eq!(ledger, -Decimal::from(shipped));
            // Replay after the competing release must retain the original
            // durable decision and produce no further stock/order effects.
            let before = snapshot(
                &db,
                &[item.id],
                &[reservation.to_string(), order.id.to_string()],
                order.id,
                &command.idempotency_key,
            )
            .await;
            command.command_id = Uuid::new_v4();
            let replay = db
                .kernel_executor(support::policy())
                .execute_ship_reserved_order_async(&command)
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(replay).unwrap(),
                serde_json::to_value(receipt).unwrap()
            );
            assert_eq!(
                snapshot(
                    &db,
                    &[item.id],
                    &[reservation.to_string(), order.id.to_string()],
                    order.id,
                    &command.idempotency_key
                )
                .await,
                before
            );
        }
    }
    ship_db.pool().close().await;
    release_db.pool().close().await;
}

async fn snapshot(
    db: &PostgresDatabase,
    items: &[i64],
    aggregates: &[String],
    order: OrderId,
    key: &str,
) -> Value {
    let mut result = serde_json::Map::new();
    for table in ["inventory_balances", "inventory_reservations", "inventory_transactions"] {
        let rows: Value = sqlx::query_scalar(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY id), '[]'::jsonb) FROM {table} t WHERE item_id = ANY($1)"))
            .bind(items).fetch_one(db.pool()).await.unwrap();
        result.insert(table.into(), rows);
    }
    for (table, column) in [("orders", "id"), ("order_items", "order_id")] {
        let rows: Value = sqlx::query_scalar(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY id), '[]'::jsonb) FROM {table} t WHERE {column} = $1"))
            .bind(order.into_uuid()).fetch_one(db.pool()).await.unwrap();
        result.insert(table.into(), rows);
    }
    let rows: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY id), '[]'::jsonb) FROM kernel_outbox t WHERE aggregate_id = ANY($1) OR idempotency_key = $2")
        .bind(aggregates).bind(key).fetch_one(db.pool()).await.unwrap();
    result.insert("kernel_outbox".into(), rows);
    let rows: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY command_id), '[]'::jsonb) FROM kernel_receipts t WHERE idempotency_key = $1")
        .bind(key).fetch_one(db.pool()).await.unwrap();
    result.insert("kernel_receipts".into(), rows);
    Value::Object(result)
}

#[tokio::test]
async fn shipment_faults_rollback_every_effect_and_allow_same_key_retry() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live shipment atomicity");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    for fault in support::FAULTS {
        for preview in [false, true] {
            for partial in [false, true] {
                let suffix = Uuid::new_v4().simple().to_string();
                let skus = [format!("ATOMIC-A-{suffix}"), format!("ATOMIC-B-{suffix}")];
                let mut items = Vec::new();
                for sku in &skus {
                    items.push(
                        db.inventory().create_item_async(support::stock(sku)).await.unwrap().id,
                    );
                }
                let customer = db.customers().create_async(support::customer()).await.unwrap();
                let order =
                    db.orders().create_async(support::order(customer.id, &skus)).await.unwrap();
                for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
                    db.orders()
                        .update_async(
                            order.id.into_uuid(),
                            UpdateOrder { status: Some(status), ..Default::default() },
                        )
                        .await
                        .unwrap();
                }
                let payment = db.payments().create_async(support::payment(&order)).await.unwrap();
                db.payments().mark_completed_async(payment.id.into_uuid()).await.unwrap();
                let order = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
                let kernel = db.kernel_executor(support::policy());
                let mut command = support::command(&order, partial);
                if preview {
                    assert_eq!(
                        kernel.execute_ship_reserved_order_async(&command).await.unwrap().status,
                        ExecutionStatus::Previewed
                    );
                    command.command_id = Uuid::new_v4();
                }
                command.mode = ExecutionMode::Apply;
                let reservations: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM inventory_reservations WHERE reference_id = $1 ORDER BY item_id, location_id, id")
                    .bind(order.id.to_string()).fetch_all(db.pool()).await.unwrap();
                assert_eq!(reservations.len(), 2);
                let mut aggregates: Vec<String> =
                    reservations.iter().map(ToString::to_string).collect();
                aggregates.push(order.id.to_string());
                let before =
                    snapshot(&db, &items, &aggregates, order.id, &command.idempotency_key).await;
                let (table, operation, condition) = support::fault(
                    fault,
                    items[1],
                    reservations[1],
                    order.id,
                    &command.idempotency_key,
                );
                let trigger = format!("shipment_fault_{suffix}");
                let first_item = items[0];
                sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                    IF (SELECT COUNT(*) FROM inventory_transactions WHERE item_id = {first_item} AND transaction_type = 'shipment') <> 1
                        THEN RAISE EXCEPTION 'earlier shipment write missing'; END IF;
                    RAISE EXCEPTION 'injected shipment atomicity failure'; END $$")).execute(db.pool()).await.unwrap();
                sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE {operation} ON {table} FOR EACH ROW WHEN ({condition}) EXECUTE FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
                let failed = kernel.execute_ship_reserved_order_async(&command).await;
                sqlx::query(&format!("DROP TRIGGER {trigger} ON {table}"))
                    .execute(db.pool())
                    .await
                    .unwrap();
                sqlx::query(&format!("DROP FUNCTION {trigger}()"))
                    .execute(db.pool())
                    .await
                    .unwrap();
                let error = failed.expect_err("the selected fault must be reached").to_string();
                assert!(
                    error.contains("injected shipment atomicity failure"),
                    "{fault} preview={preview} partial={partial}: {error}"
                );
                assert_eq!(
                    snapshot(&db, &items, &aggregates, order.id, &command.idempotency_key).await,
                    before,
                    "{fault} preview={preview} partial={partial}: all durable rows must roll back"
                );

                let receipt = kernel.execute_ship_reserved_order_async(&command).await.unwrap();
                support::check_receipt(&receipt, &order, partial);
                for (index, sku) in skus.iter().enumerate() {
                    let stock = db.inventory().get_stock_async(sku).await.unwrap().unwrap();
                    let reserved = Decimal::from(index as i32 + 2);
                    let shipped = if partial { Decimal::ONE } else { reserved };
                    assert_eq!(
                        (stock.total_on_hand, stock.total_allocated, stock.total_available),
                        (
                            Decimal::from(10) - shipped,
                            reserved - shipped,
                            Decimal::from(10) - reserved
                        )
                    );
                }
                let linked: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE command_id = $1")
                        .bind(command.command_id)
                        .fetch_one(db.pool())
                        .await
                        .unwrap();
                assert_eq!(linked, 3);
                let shipped: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_transactions WHERE item_id = ANY($1) AND transaction_type = 'shipment'").bind(&items).fetch_one(db.pool()).await.unwrap();
                assert_eq!(shipped, 2);
                let committed =
                    snapshot(&db, &items, &aggregates, order.id, &command.idempotency_key).await;
                command.command_id = Uuid::new_v4();
                let replay = kernel.execute_ship_reserved_order_async(&command).await.unwrap();
                assert_eq!(
                    serde_json::to_value(replay).unwrap(),
                    serde_json::to_value(receipt).unwrap()
                );
                assert_eq!(
                    snapshot(&db, &items, &aggregates, order.id, &command.idempotency_key).await,
                    committed,
                    "replay must not write again"
                );
            }
        }
    }
}

#[tokio::test]
async fn line_owned_fractional_splits_preserve_same_sku_sibling() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live ownership check");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let sku = format!("OWNERSHIP-{}", Uuid::new_v4());
    db.inventory().create_item_async(support::stock(&sku)).await.unwrap();
    let customer = db.customers().create_async(support::customer()).await.unwrap();
    let order = db
        .orders()
        .create_async(support::order(customer.id, &[sku.clone(), sku.clone()]))
        .await
        .unwrap();
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update_async(
                order.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .unwrap();
    }
    let payment = db.payments().create_async(support::payment(&order)).await.unwrap();
    db.payments().mark_completed_async(payment.id.into_uuid()).await.unwrap();
    let order = db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap();
    let target = &order.items[0];
    let sibling = &order.items[1];
    let micro = Decimal::new(1, 6);
    let source: Uuid =
        sqlx::query_scalar("SELECT id FROM inventory_reservations WHERE order_item_id = $1")
            .bind(target.id.into_uuid())
            .fetch_one(db.pool())
            .await
            .unwrap();
    let child = Uuid::from_u128(source.as_u128() + 1);
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO inventory_reservations (id,item_id,location_id,quantity,status,reference_type,reference_id,expires_at,created_at,order_item_id)
        SELECT $1,item_id,location_id,$2,status,reference_type,reference_id,expires_at,created_at,order_item_id FROM inventory_reservations WHERE id = $3")
        .bind(child).bind(Decimal::from(target.quantity) - micro).bind(source).execute(tx.as_mut()).await.unwrap();
    sqlx::query("UPDATE inventory_reservations SET quantity = $1 WHERE id = $2")
        .bind(micro)
        .bind(source)
        .execute(tx.as_mut())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut command = support::command(&order, false);
    command.mode = ExecutionMode::Apply;
    command.payload.lines = Some(vec![ShipmentLineInput { order_item_id: target.id, quantity: 1 }]);
    let kernel = db.kernel_executor(support::policy());
    let receipt = kernel.execute_ship_reserved_order_async(&command).await.unwrap();
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{:?}", receipt.error_message);
    assert_eq!(receipt.event_ids.len(), 3);
    let shipped = receipt.result.as_ref().unwrap();
    assert_eq!(shipped.items.iter().find(|i| i.id == target.id).unwrap().shipped_quantity, 1);
    assert_eq!(shipped.items.iter().find(|i| i.id == sibling.id).unwrap().shipped_quantity, 0);
    let original: (String, Decimal) =
        sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
            .bind(source)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(original, ("fulfilled".into(), micro));
    let remainder: Decimal =
        sqlx::query_scalar("SELECT quantity FROM inventory_reservations WHERE id = $1")
            .bind(child)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(remainder, Decimal::from(target.quantity - 1));
    let sibling_row: (String, Decimal) = sqlx::query_as(
        "SELECT status, quantity FROM inventory_reservations WHERE order_item_id = $1",
    )
    .bind(sibling.id.into_uuid())
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(sibling_row, ("pending".into(), Decimal::from(sibling.quantity)));
    let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
    assert_eq!(
        (stock.total_on_hand, stock.total_allocated, stock.total_available),
        (Decimal::from(9), Decimal::from(4), Decimal::from(5))
    );
    command.command_id = Uuid::new_v4();
    assert_eq!(
        serde_json::to_value(kernel.execute_ship_reserved_order_async(&command).await.unwrap())
            .unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );

    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE inventory_reservations SET quantity = $1 WHERE id = $2")
        .bind(Decimal::from(target.quantity - 1) - micro)
        .bind(child)
        .execute(tx.as_mut())
        .await
        .unwrap();
    sqlx::query("UPDATE inventory_reservations SET quantity = $1 WHERE order_item_id = $2")
        .bind(Decimal::from(sibling.quantity) + micro)
        .bind(sibling.id.into_uuid())
        .execute(tx.as_mut())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut next = support::command(shipped, false);
    next.mode = ExecutionMode::Apply;
    next.payload.lines = command.payload.lines;
    let refused = kernel.execute_ship_reserved_order_async(&next).await.unwrap();
    assert_eq!(refused.status, ExecutionStatus::Rejected);
    assert_eq!(refused.error_code.as_deref(), Some("commerce.shipment_inventory_invalid"));
    assert!(refused.event_ids.is_empty());
    assert_eq!(db.orders().get_async(order.id.into_uuid()).await.unwrap().unwrap(), *shipped);
    assert_eq!(
        serde_json::to_value(db.inventory().get_stock_async(&sku).await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(stock).unwrap()
    );
}
