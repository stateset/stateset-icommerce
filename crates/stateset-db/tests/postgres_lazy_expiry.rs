#![cfg(feature = "postgres")]
//! Force both callers into the lazy-expiry window using observable row locks,
//! rather than hoping a scheduler interleaving reproduces the race.

use rust_decimal::Decimal;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use stateset_core::{CreateInventoryItem, ReserveInventory};
use stateset_db::{PostgresDatabase, postgres::PgInventoryRepository};
use std::{str::FromStr, time::Duration};
use uuid::Uuid;

fn reserve(sku: &str, quantity: Decimal) -> ReserveInventory {
    ReserveInventory {
        sku: sku.into(),
        quantity,
        location_id: Some(1),
        reference_type: "formal".into(),
        reference_id: Uuid::new_v4().to_string(),
        expires_in_seconds: None,
    }
}

/// Expiry timestamps intentionally disagree with reservation identity order.
/// A shipment holding the smaller ID must be able to acquire the larger ID
/// while lazy expiry waits; otherwise the two operations can form a cycle.
#[tokio::test]
async fn lazy_expiry_locks_reservations_in_shipment_identity_order() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping lazy-expiry lock order");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let name = format!("lazy_order_{}", Uuid::new_v4().simple());
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(PgConnectOptions::from_str(&url).unwrap().application_name(&name))
        .await
        .unwrap();
    for scale in [0, 6] {
        let unit = Decimal::new(1, scale);
        let sku = format!("LAZY-ORDER-{}", Uuid::new_v4());
        let item = db
            .inventory()
            .create_item_async(CreateInventoryItem {
                sku: sku.clone(),
                name: "Lazy expiry lock order".into(),
                initial_quantity: Some(unit * Decimal::from(10)),
                ..Default::default()
            })
            .await
            .unwrap();
        let mut ids = Vec::new();
        for _ in 0..2 {
            ids.push(
                db.inventory()
                    .reserve_async(reserve(&sku, unit * Decimal::from(2)))
                    .await
                    .unwrap()
                    .id,
            );
        }
        let live =
            db.inventory().reserve_async(reserve(&sku, unit * Decimal::from(3))).await.unwrap();
        // Compare the persisted row so timestamp storage precision is not
        // mistaken for a mutation of the live sibling.
        let live = db.inventory().get_reservation_async(live.id).await.unwrap().unwrap();
        ids.sort();
        sqlx::query("UPDATE inventory_reservations SET expires_at = CASE WHEN id = $1 THEN '2000-01-01'::timestamptz ELSE '1999-01-01'::timestamptz END WHERE id = ANY($2)")
            .bind(ids[0]).bind(&ids).execute(db.pool()).await.unwrap();
        let mut gate = db.pool().begin().await.unwrap();
        sqlx::query("SELECT id FROM inventory_reservations WHERE id = $1 FOR UPDATE")
            .bind(ids[0])
            .fetch_one(gate.as_mut())
            .await
            .unwrap();
        let input = reserve(&sku, unit);
        let inventory = PgInventoryRepository::new(pool.clone());
        let worker = tokio::spawn(async move { inventory.reserve_async(input).await });
        let waiting = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let blocked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock')")
                    .bind(&name).fetch_one(db.pool()).await.unwrap();
                if blocked { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await;
        let next =
            sqlx::query("SELECT id FROM inventory_reservations WHERE id = $1 FOR UPDATE NOWAIT")
                .bind(ids[1])
                .fetch_one(gate.as_mut())
                .await;
        gate.rollback().await.unwrap();
        let created =
            tokio::time::timeout(Duration::from_secs(30), worker).await.unwrap().unwrap().unwrap();
        waiting.expect("lazy expiry reached the first reservation lock");
        assert!(
            next.is_ok(),
            "scale={scale}: lazy expiry held the higher ID while waiting for the lower: {next:?}"
        );
        assert_eq!(created.quantity, unit);
        assert!(created.status.holds_stock());
        let stock = db.inventory().get_stock_async(&sku).await.unwrap().unwrap();
        assert_eq!(
            (stock.total_on_hand, stock.total_allocated, stock.total_available),
            (unit * Decimal::from(10), unit * Decimal::from(4), unit * Decimal::from(6))
        );
        let held: Decimal = sqlx::query_scalar("SELECT SUM(quantity) FROM inventory_reservations WHERE item_id = $1 AND status IN ('pending', 'confirmed', 'allocated')")
            .bind(item.id).fetch_one(db.pool()).await.unwrap();
        assert_eq!(held, stock.total_allocated);
        for id in ids {
            let expired = db.inventory().get_reservation_async(id).await.unwrap().unwrap();
            assert_eq!(expired.status.to_string(), "expired");
            assert_eq!(expired.quantity, unit * Decimal::from(2));
            let payloads: Vec<serde_json::Value> = sqlx::query_scalar("SELECT payload FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = 'inventory.reservation_expired.v1'")
                .bind(id.to_string()).fetch_all(db.pool()).await.unwrap();
            assert_eq!(payloads.len(), 1);
            assert_eq!(
                payloads[0]["quantity"].as_str().unwrap().parse::<Decimal>().unwrap(),
                unit * Decimal::from(2)
            );
        }
        assert_eq!(db.inventory().get_reservation_async(live.id).await.unwrap().unwrap(), live);
    }
    pool.close().await;
}

#[tokio::test]
async fn concurrent_lazy_expiry_preserves_live_holds_and_emits_once() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live lazy-expiry race");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect and migrate");
    for scale in [0, 6] {
        let unit = Decimal::new(1, scale);
        let sku = format!("LAZY-EXPIRY-{}", Uuid::new_v4());
        let item = db
            .inventory()
            .create_item_async(CreateInventoryItem {
                sku: sku.clone(),
                name: "Concurrent lazy expiry".into(),
                initial_quantity: Some(unit * Decimal::from(10)),
                ..Default::default()
            })
            .await
            .unwrap();
        let expired =
            db.inventory().reserve_async(reserve(&sku, unit * Decimal::from(2))).await.unwrap();
        let live =
            db.inventory().reserve_async(reserve(&sku, unit * Decimal::from(3))).await.unwrap();
        sqlx::query(
            "UPDATE inventory_reservations SET expires_at = '2000-01-01T00:00:00Z' WHERE id = $1",
        )
        .bind(expired.id)
        .execute(db.pool())
        .await
        .unwrap();

        // Hold the balance so neither worker can finish expiry before the
        // other has entered it. Each dedicated pool has an observable name.
        let mut gate = db.pool().begin().await.unwrap();
        sqlx::query(
            "SELECT id FROM inventory_balances WHERE item_id = $1 AND location_id = 1 FOR UPDATE",
        )
        .bind(item.id)
        .fetch_one(gate.as_mut())
        .await
        .unwrap();
        let names: Vec<String> =
            (0..2).map(|i| format!("expiry_{}_{}", Uuid::new_v4().simple(), i)).collect();
        let mut workers = Vec::new();
        for name in &names {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect_with(PgConnectOptions::from_str(&url).unwrap().application_name(name))
                .await
                .unwrap();
            let input = reserve(&sku, unit);
            workers.push(tokio::spawn(async move {
                let result = PgInventoryRepository::new(pool.clone()).reserve_async(input).await;
                pool.close().await;
                result
            }));
        }
        let waiting = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM pg_stat_activity WHERE application_name = ANY($1) AND wait_event_type = 'Lock'"
                ).bind(&names).fetch_one(db.pool()).await.unwrap();
                if count == 2 { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await;
        // Always open the gate and join the workers, even if synchronization
        // failed, so a failing test leaves no live blocked transactions behind.
        gate.rollback().await.unwrap();
        for worker in workers {
            tokio::time::timeout(Duration::from_secs(30), worker)
                .await
                .expect("worker completed")
                .expect("worker did not panic")
                .expect("reservation succeeded");
        }
        waiting.expect("both workers reached a real database lock before opening the gate");

        let allocated: Decimal = sqlx::query_scalar("SELECT quantity_allocated FROM inventory_balances WHERE item_id = $1 AND location_id = 1")
            .bind(item.id).fetch_one(db.pool()).await.unwrap();
        let held: Decimal = sqlx::query_scalar("SELECT SUM(quantity) FROM inventory_reservations WHERE item_id = $1 AND status IN ('pending', 'confirmed', 'allocated')")
            .bind(item.id).fetch_one(db.pool()).await.unwrap();
        let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_expired.v1' AND aggregate_id = $1")
            .bind(expired.id.to_string()).fetch_one(db.pool()).await.unwrap();
        assert_eq!(
            (allocated, held, events),
            (unit * Decimal::from(5), unit * Decimal::from(5), 1),
            "scale={scale}: expiry must release its own units once, without consuming the live hold"
        );
        let retained = db.inventory().get_reservation_async(live.id).await.unwrap().unwrap();
        assert_eq!(retained.quantity, unit * Decimal::from(3));
        assert!(retained.status.holds_stock());
    }
}
