#![cfg(feature = "sqlite")]
//! SQLite parity for the stale-candidate race modeled in LazyExpiry.tla.
use rust_decimal::Decimal;
use stateset_core::{CreateInventoryItem, InventoryRepository, ReserveInventory};
use stateset_db::SqliteDatabase;
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

#[test]
fn concurrent_lazy_expiry_preserves_live_holds_and_emits_once() {
    let db = std::sync::Arc::new(SqliteDatabase::in_memory().unwrap());
    for scale in [0, 6] {
        let unit = Decimal::new(1, scale);
        let sku = format!("LAZY-EXPIRY-{scale}");
        let item = db
            .inventory()
            .create_item(CreateInventoryItem {
                sku: sku.clone(),
                name: "Concurrent lazy expiry".into(),
                initial_quantity: Some(unit * Decimal::from(10)),
                ..Default::default()
            })
            .unwrap();
        let expired = db.inventory().reserve(reserve(&sku, unit * Decimal::from(2))).unwrap();
        let live = db.inventory().reserve(reserve(&sku, unit * Decimal::from(3))).unwrap();
        db.pool().get().unwrap().execute(
            "UPDATE inventory_reservations SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?",
            [expired.id.to_string()],
        ).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let db = db.clone();
                let barrier = barrier.clone();
                let input = reserve(&sku, unit);
                std::thread::spawn(move || {
                    barrier.wait();
                    db.inventory().reserve(input)
                })
            })
            .collect();
        barrier.wait();
        for worker in workers {
            worker.join().unwrap().expect("reservation succeeded");
        }
        let stock = db.inventory().get_stock(&sku).unwrap().unwrap();
        let conn = db.pool().get().unwrap();
        let mut statement = conn.prepare("SELECT quantity FROM inventory_reservations WHERE item_id = ? AND status IN ('pending', 'confirmed', 'allocated')").unwrap();
        let held: Decimal = statement
            .query_map([item.id], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap().parse::<Decimal>().unwrap())
            .sum();
        let events: i64 = conn.query_row("SELECT COUNT(*) FROM kernel_outbox WHERE event_type = 'inventory.reservation_expired.v1' AND aggregate_id = ?", [expired.id.to_string()], |r| r.get(0)).unwrap();
        assert_eq!(
            (stock.total_allocated, held, events),
            (unit * Decimal::from(5), unit * Decimal::from(5), 1),
            "scale={scale}"
        );
        let retained = db.inventory().get_reservation(live.id).unwrap().unwrap();
        assert_eq!(retained.quantity, unit * Decimal::from(3));
        assert!(retained.status.holds_stock());
    }
}
