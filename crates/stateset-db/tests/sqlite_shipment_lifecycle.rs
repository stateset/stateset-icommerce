#![cfg(feature = "sqlite")]

use stateset_core::{
    AddShipmentEvent, CommerceError, CreateShipment, CreateShipmentItem, Shipment,
    ShipmentRepository, ShipmentStatus, ShippingCarrier, UpdateShipment,
};
use stateset_core::{
    CreateCustomer, CreateOrder, CreateOrderItem, CustomerRepository, OrderRepository,
};
use stateset_db::SqliteDatabase;
use std::sync::{Arc, Barrier};

fn order(db: &SqliteDatabase) -> stateset_core::Order {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{}@example.com", uuid::Uuid::new_v4()),
            first_name: "Ada".into(),
            last_name: "L".into(),
            ..Default::default()
        })
        .unwrap();
    db.orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: stateset_core::ProductId::new(),
                sku: "W-1".into(),
                name: "Widget".into(),
                quantity: 5,
                unit_price: rust_decimal::Decimal::ONE,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
}

fn shipment(db: &SqliteDatabase) -> Shipment {
    db.shipments()
        .create(CreateShipment {
            order_id: order(&db).id,
            recipient_name: "Ada".into(),
            shipping_address: "1 Main".into(),
            carrier: Some(ShippingCarrier::Ups),
            ..Default::default()
        })
        .unwrap()
}

fn item_input() -> CreateShipmentItem {
    CreateShipmentItem {
        sku: "W-1".into(),
        name: "Widget".into(),
        quantity: 1,
        ..Default::default()
    }
}

fn allocation(order_id: stateset_core::OrderId, quantity: i32) -> CreateShipment {
    CreateShipment {
        order_id,
        recipient_name: "Ada".into(),
        shipping_address: "1 Main".into(),
        items: Some(vec![CreateShipmentItem { quantity, ..item_input() }]),
        ..Default::default()
    }
}

#[test]
fn allocations_bind_identity_enforce_order_budget_and_preserve_history() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let s = shipment(&db);
    let source = db.orders().get(s.order_id).unwrap().unwrap();
    let foreign = order(&db);
    let line = &source.items[0];
    for invalid in [
        CreateShipmentItem { order_item_id: Some(foreign.items[0].id.into_uuid()), ..item_input() },
        CreateShipmentItem {
            order_item_id: Some(line.id.into_uuid()),
            sku: "OTHER".into(),
            ..item_input()
        },
        CreateShipmentItem { product_id: Some(stateset_core::ProductId::new()), ..item_input() },
    ] {
        assert!(repo.add_item(s.id, invalid.clone()).is_err());
        assert!(
            repo.create(CreateShipment { items: Some(vec![invalid]), ..allocation(s.order_id, 1) })
                .is_err()
        );
    }
    let first = repo.add_item(s.id, CreateShipmentItem { quantity: 2, ..item_input() }).unwrap();
    assert_eq!(first.order_item_id, Some(line.id.into_uuid()));
    assert_eq!(first.product_id, Some(line.product_id));
    let second = repo.create(allocation(s.order_id, 3)).unwrap();
    assert!(repo.add_item(s.id, item_input()).is_err());
    assert!(repo.create(allocation(s.order_id, 1)).is_err());
    assert!(repo.create_batch_atomic(vec![allocation(s.order_id, 1)]).is_err());
    assert!(
        db.orders()
            .remove_item(s.order_id, line.id)
            .unwrap_err()
            .to_string()
            .contains("shipment history")
    );
    assert!(db.orders().delete(s.order_id).unwrap_err().to_string().contains("shipment history"));
    repo.cancel(second.id).unwrap();
    let replacement = repo.create(allocation(s.order_id, 3)).unwrap();
    assert_eq!(replacement.items[0].order_item_id, Some(line.id.into_uuid()));
    repo.cancel(s.id).unwrap();
    repo.cancel(replacement.id).unwrap();
    assert!(db.orders().remove_item(s.order_id, line.id).is_err());
    assert!(db.orders().delete(s.order_id).is_err());
    assert_eq!(db.orders().get(s.order_id).unwrap().unwrap().items[0].shipped_quantity, 0);
}

#[test]
fn allocation_counts_legacy_items_and_refuses_ambiguous_skus() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let s = repo.create(allocation(order(&db).id, 4)).unwrap();
    db.conn().unwrap().execute("UPDATE shipment_items SET order_item_id = NULL, product_id = NULL WHERE shipment_id = ?", [s.id.to_string()]).unwrap();
    assert!(repo.add_item(s.id, CreateShipmentItem { quantity: 2, ..item_input() }).is_err());
    let last = repo.add_item(s.id, item_input()).unwrap();
    assert!(last.order_item_id.is_some());
    assert_eq!(repo.get(s.id).unwrap().unwrap().items.iter().map(|i| i.quantity).sum::<i32>(), 5);
    let added_line = db
        .orders()
        .add_item(
            s.order_id,
            CreateOrderItem {
                product_id: stateset_core::ProductId::new(),
                sku: "W-1".into(),
                name: "Another line".into(),
                quantity: 5,
                unit_price: rust_decimal::Decimal::ONE,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(repo.add_item(s.id, item_input()).unwrap_err().to_string().contains("Ambiguous"));
    // Even an explicit new reference cannot guess which line owns the legacy units.
    assert!(
        repo.add_item(
            s.id,
            CreateShipmentItem { order_item_id: Some(added_line.id.into_uuid()), ..item_input() }
        )
        .is_err()
    );
    let clean = shipment(&db);
    let second_line = db
        .orders()
        .add_item(
            clean.order_id,
            CreateOrderItem {
                product_id: stateset_core::ProductId::new(),
                sku: "W-1".into(),
                name: "Another line".into(),
                quantity: 5,
                unit_price: rust_decimal::Decimal::ONE,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(repo.add_item(clean.id, item_input()).is_err());
    assert_eq!(
        repo.add_item(
            clean.id,
            CreateShipmentItem { order_item_id: Some(second_line.id.into_uuid()), ..item_input() }
        )
        .unwrap()
        .order_item_id,
        Some(second_line.id.into_uuid())
    );
}

#[test]
fn competing_allocation_batches_are_atomic_and_cannot_double_assign_units() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    let a = order(&db);
    let b = order(&db);
    assert!(
        db.shipments().create_batch_atomic(vec![allocation(a.id, 3), allocation(a.id, 3)]).is_err()
    );
    assert!(db.shipments().for_order(a.id).unwrap().is_empty());
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = [vec![a.id, b.id], vec![b.id, a.id]]
        .into_iter()
        .map(|ids| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                db.shipments()
                    .create_batch_atomic(ids.into_iter().map(|id| allocation(id, 3)).collect())
            })
        })
        .collect();
    let results: Vec<_> = tasks.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    for id in [a.id, b.id] {
        let shipments = db.shipments().for_order(id).unwrap();
        assert_eq!(shipments.len(), 1);
        assert_eq!(shipments[0].items[0].quantity, 3);
        assert_eq!(events(&db, shipments[0].id), 1);
    }
}

#[test]
fn concurrent_order_line_removal_and_item_allocation_have_one_winner() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    let s = shipment(&db);
    let line = db.orders().get(s.order_id).unwrap().unwrap().items[0].id;
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = [true, false]
        .into_iter()
        .map(|add| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                if add {
                    db.shipments().add_item(s.id, item_input()).map(|_| ())
                } else {
                    db.orders().remove_item(s.order_id, line)
                }
            })
        })
        .collect();
    let results: Vec<_> = tasks.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let items = db.shipments().get_items(s.id).unwrap();
    let lines = db.orders().get(s.order_id).unwrap().unwrap().items;
    assert_eq!(items.len(), lines.len());
    if let Some(item) = items.first() {
        assert_eq!(item.order_item_id, Some(lines[0].id.into_uuid()));
    }
}

#[test]
fn item_mutations_advance_version_and_preserve_audit_contents() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let s = shipment(&db);
    let item = repo.add_item(s.id, item_input()).unwrap();
    let added = repo.get(s.id).unwrap().unwrap();
    assert_eq!(added.version, 2);
    assert_eq!(added.updated_at, item.created_at);
    assert_eq!(added.items[0].id, item.id);
    assert!(matches!(
        repo.update(
            s.id,
            UpdateShipment {
                expected_version: Some(1),
                notes: Some("stale".into()),
                ..Default::default()
            }
        ),
        Err(CommerceError::VersionConflict { .. })
    ));
    repo.remove_item(item.id).unwrap();
    let removed = repo.get(s.id).unwrap().unwrap();
    assert_eq!(removed.version, 3);
    assert!(removed.items.is_empty());
    assert!(matches!(repo.remove_item(item.id), Err(CommerceError::NotFound)));
    assert!(matches!(
        repo.add_item(stateset_core::ShipmentId::new(), item_input()),
        Err(CommerceError::NotFound)
    ));
    let facts = db.kernel_outbox().pending(100).unwrap();
    for (event_type, version) in [("shipments.item_added.v1", 2), ("shipments.item_removed.v1", 3)]
    {
        let fact = facts.iter().find(|e| e.event_type == event_type).unwrap();
        assert_eq!(fact.aggregate_id, s.id.to_string());
        assert_eq!(fact.payload["version"], version);
        assert_eq!(fact.payload["previous_version"], version - 1);
        assert_eq!(fact.payload["item"]["id"], item.id.to_string());
        assert_eq!(fact.payload["item"]["quantity"], 1);
        assert_eq!(fact.payload["item"]["sku"], "W-1");
    }
    assert_eq!(events(&db, s.id), 3);
}

#[test]
fn item_contents_are_frozen_from_ready_through_terminal_states() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    for status in [
        ShipmentStatus::ReadyToShip,
        ShipmentStatus::Shipped,
        ShipmentStatus::InTransit,
        ShipmentStatus::OutForDelivery,
        ShipmentStatus::Delivered,
        ShipmentStatus::Failed,
        ShipmentStatus::Returned,
        ShipmentStatus::Cancelled,
    ] {
        let s = shipment(&db);
        let item = repo.add_item(s.id, item_input()).unwrap();
        // Seed each lifecycle state to test item guards independently of transition routing.
        db.conn()
            .unwrap()
            .execute(
                "UPDATE shipments SET status = ? WHERE id = ?",
                [status.to_string(), s.id.to_string()],
            )
            .unwrap();
        let before = repo.get(s.id).unwrap().unwrap();
        assert!(repo.add_item(s.id, item_input()).is_err(), "{status}");
        assert!(repo.remove_item(item.id).is_err(), "{status}");
        assert_eq!(
            serde_json::to_value(repo.get(s.id).unwrap().unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert_eq!(events(&db, s.id), 2);
    }
    for status in [ShipmentStatus::Processing, ShipmentStatus::OnHold] {
        let s = shipment(&db);
        repo.update(s.id, UpdateShipment { status: Some(status), ..Default::default() }).unwrap();
        let item = repo.add_item(s.id, item_input()).unwrap();
        repo.remove_item(item.id).unwrap();
        assert_eq!(repo.get(s.id).unwrap().unwrap().version, 4);
    }
}

#[test]
fn invalid_items_are_rejected_in_single_and_batch_creation_and_initial_items_are_audited() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let input = CreateShipment {
        order_id: order(&db).id,
        recipient_name: "Ada".into(),
        shipping_address: "1 Main".into(),
        items: Some(vec![item_input()]),
        ..Default::default()
    };
    for invalid in [
        CreateShipmentItem { quantity: 0, ..item_input() },
        CreateShipmentItem { quantity: -1, ..item_input() },
        CreateShipmentItem { sku: " ".into(), ..item_input() },
        CreateShipmentItem { name: "\t".into(), ..item_input() },
    ] {
        let bad = CreateShipment { items: Some(vec![invalid]), ..input.clone() };
        assert!(repo.create(bad.clone()).is_err());
        assert!(repo.create_batch_atomic(vec![input.clone(), bad]).is_err());
        assert_eq!(repo.count(Default::default()).unwrap(), 0);
        assert!(
            db.kernel_outbox()
                .pending(100)
                .unwrap()
                .iter()
                .all(|fact| fact.aggregate_type != "shipment")
        );
    }
    let s = repo.create(input.clone()).unwrap();
    assert!(repo.add_item(s.id, CreateShipmentItem { quantity: 0, ..item_input() }).is_err());
    let batch = repo.create_batch_atomic(vec![input]).unwrap();
    for created in std::iter::once(s).chain(batch) {
        let fact = db
            .kernel_outbox()
            .pending(100)
            .unwrap()
            .into_iter()
            .find(|e| e.aggregate_id == created.id.to_string())
            .unwrap();
        assert_eq!(fact.payload["items"][0]["id"], created.items[0].id.to_string());
        assert_eq!(fact.payload["items"][0]["quantity"], 1);
    }
}

#[test]
fn item_audit_failure_and_version_overflow_roll_back_items_and_parent() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let s = shipment(&db);
    let item = repo.add_item(s.id, item_input()).unwrap();
    let before = repo.get(s.id).unwrap().unwrap();
    db.conn().unwrap().execute_batch("CREATE TRIGGER reject_item_audit BEFORE INSERT ON kernel_outbox WHEN NEW.event_type IN ('shipments.item_added.v1', 'shipments.item_removed.v1') BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;").unwrap();
    assert!(repo.add_item(s.id, item_input()).is_err());
    assert!(repo.remove_item(item.id).is_err());
    assert_eq!(
        serde_json::to_value(repo.get(s.id).unwrap().unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert_eq!(events(&db, s.id), 2);
    db.conn().unwrap().execute_batch("DROP TRIGGER reject_item_audit;").unwrap();
    db.conn()
        .unwrap()
        .execute(
            "UPDATE shipments SET version = ? WHERE id = ?",
            rusqlite::params![i32::MAX, s.id.to_string()],
        )
        .unwrap();
    assert!(repo.add_item(s.id, item_input()).is_err());
    assert!(repo.remove_item(item.id).is_err());
    let stored = repo.get(s.id).unwrap().unwrap();
    assert_eq!(stored.version, i32::MAX);
    assert_eq!(stored.items.len(), 1);
    assert_eq!(events(&db, s.id), 2);
}

#[test]
fn concurrent_item_additions_and_duplicate_removals_serialize_parent_versions() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    let s = shipment(&db);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                db.shipments().add_item(s.id, item_input()).unwrap()
            })
        })
        .collect();
    let items: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(db.shipments().get(s.id).unwrap().unwrap().version, 3);
    assert_eq!(events(&db, s.id), 3);
    let item_id = items[0].id;
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                db.shipments().remove_item(item_id)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results.iter().any(|r| matches!(r, Err(CommerceError::NotFound))));
    let stored = db.shipments().get(s.id).unwrap().unwrap();
    assert_eq!(stored.version, 4);
    assert_eq!(stored.items.len(), 1);
    assert_eq!(events(&db, s.id), 4);
}

#[test]
fn concurrent_item_addition_and_versioned_ready_transition_have_one_winner() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    let s = shipment(&db);
    db.shipments().mark_processing(s.id).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = [true, false]
        .into_iter()
        .map(|add| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                if add {
                    db.shipments().add_item(s.id, item_input()).map(|_| ())
                } else {
                    db.shipments()
                        .update(
                            s.id,
                            UpdateShipment {
                                status: Some(ShipmentStatus::ReadyToShip),
                                expected_version: Some(2),
                                ..Default::default()
                            },
                        )
                        .map(|_| ())
                }
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let stored = db.shipments().get(s.id).unwrap().unwrap();
    assert_eq!(stored.version, 3);
    assert_eq!(stored.items.len(), usize::from(stored.status == ShipmentStatus::Processing));
    assert_eq!(events(&db, s.id), 3);
}

fn events(db: &SqliteDatabase, id: stateset_core::ShipmentId) -> usize {
    db.kernel_outbox()
        .pending(1000)
        .unwrap()
        .iter()
        .filter(|e| e.aggregate_id == id.to_string())
        .count()
}

fn ready(db: &SqliteDatabase, id: stateset_core::ShipmentId) {
    db.shipments().mark_processing(id).unwrap();
    db.shipments().mark_ready(id).unwrap();
}

#[test]
fn lifecycle_versions_timestamps_and_outbox_are_consistent() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let s = shipment(&db);
    assert_eq!(repo.get(s.id).unwrap().unwrap().version, 1);
    assert_eq!(events(&db, s.id), 1);
    ready(&db, s.id);
    let shipped = repo.ship(s.id, Some("TRACK".into())).unwrap();
    assert_eq!(shipped.version, 4);
    assert!(shipped.shipped_at.is_some());
    let count = events(&db, s.id);
    let replay = repo.ship(s.id, Some("TRACK".into())).unwrap();
    assert_eq!(replay.version, shipped.version);
    assert_eq!(replay.shipped_at, shipped.shipped_at);
    assert_eq!(replay.updated_at, shipped.updated_at);
    assert_eq!(events(&db, s.id), count);
    repo.mark_in_transit(s.id).unwrap();
    repo.mark_out_for_delivery(s.id).unwrap();
    let delivered = repo.mark_delivered(s.id).unwrap();
    assert_eq!(delivered.version, 7);
    assert!(delivered.delivered_at.is_some());
    assert_eq!(repo.get_batch(vec![s.id]).unwrap()[0].version, 7);
    let outbox = db.kernel_outbox().pending(100).unwrap();
    let event = outbox.iter().find(|event| event.payload["version"] == 7).unwrap();
    assert_eq!(event.event_type, "shipments.updated.v1");
    assert_eq!(event.payload["status"], "delivered");
    assert_eq!(event.payload["previous_status"], "out_for_delivery");
    assert!(repo.cancel(s.id).is_err());
    assert!(repo.delete(s.id).is_err());
    assert!(repo.mark_processing(s.id).is_err());
    assert_eq!(repo.get(s.id).unwrap().unwrap().version, 7);
}

#[test]
fn every_update_route_enforces_transitions_and_missing_rows() {
    let db = SqliteDatabase::in_memory().unwrap();
    let s = shipment(&db);
    let repo = db.shipments();
    assert!(repo.ship(s.id, None).is_err());
    assert!(repo.mark_delivered(s.id).is_err());
    assert!(
        repo.update(
            s.id,
            UpdateShipment { status: Some(ShipmentStatus::Delivered), ..Default::default() }
        )
        .is_err()
    );
    assert!(
        repo.update_batch_atomic(vec![(
            s.id,
            UpdateShipment { status: Some(ShipmentStatus::Delivered), ..Default::default() }
        )])
        .is_err()
    );
    let batch = repo
        .update_batch(vec![(
            s.id,
            UpdateShipment { status: Some(ShipmentStatus::Delivered), ..Default::default() },
        )])
        .unwrap();
    assert_eq!(batch.success_count, 0);
    assert!(repo.delete(stateset_core::ShipmentId::new()).is_err());
    assert_eq!(repo.get(s.id).unwrap().unwrap().version, 1);
}

#[test]
fn batch_failure_rolls_back_state_and_audit_and_cancel_preserves_history() {
    let db = SqliteDatabase::in_memory().unwrap();
    let repo = db.shipments();
    let a = shipment(&db);
    let b = shipment(&db);
    repo.add_item(
        a.id,
        CreateShipmentItem {
            sku: "W-1".into(),
            name: "Widget".into(),
            quantity: 1,
            ..Default::default()
        },
    )
    .unwrap();
    repo.add_event(
        a.id,
        AddShipmentEvent {
            event_type: "label_created".into(),
            location: None,
            description: None,
            event_time: None,
        },
    )
    .unwrap();
    let count = events(&db, a.id);
    assert!(
        repo.update_batch_atomic(vec![
            (a.id, UpdateShipment { notes: Some("should roll back".into()), ..Default::default() }),
            (
                b.id,
                UpdateShipment { status: Some(ShipmentStatus::Delivered), ..Default::default() }
            ),
        ])
        .is_err()
    );
    assert_eq!(repo.get(a.id).unwrap().unwrap().notes, None);
    assert_eq!(events(&db, a.id), count);
    repo.delete_batch_atomic(vec![a.id]).unwrap();
    let cancelled = repo.get(a.id).unwrap().unwrap();
    assert_eq!(cancelled.status, ShipmentStatus::Cancelled);
    assert_eq!(cancelled.items.len(), 1);
    assert_eq!(cancelled.events.len(), 1);
    ready(&db, b.id);
    repo.ship(b.id, None).unwrap();
    let c = shipment(&db);
    assert!(repo.delete_batch_atomic(vec![c.id, b.id]).is_err());
    assert_eq!(repo.get(c.id).unwrap().unwrap().status, ShipmentStatus::Pending);
}

#[test]
fn outbox_failure_rolls_back_the_mutation() {
    let db = SqliteDatabase::in_memory().unwrap();
    let s = shipment(&db);
    db.conn().unwrap().execute_batch("CREATE TRIGGER reject_shipment_audit BEFORE INSERT ON kernel_outbox WHEN NEW.aggregate_type = 'shipment' BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;").unwrap();
    assert!(db.shipments().mark_processing(s.id).is_err());
    let after = db.shipments().get(s.id).unwrap().unwrap();
    assert_eq!(after.status, ShipmentStatus::Pending);
    assert_eq!(after.version, 1);
}

#[test]
fn concurrent_distinct_field_patches_are_not_lost() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    let s = shipment(&db);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [
        UpdateShipment { notes: Some("packing note".into()), ..Default::default() },
        UpdateShipment { tracking_number: Some("TRACK".into()), ..Default::default() },
    ]
    .into_iter()
    .map(|patch| {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            db.shipments().update(s.id, patch)
        })
    })
    .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    let after = db.shipments().get(s.id).unwrap().unwrap();
    assert_eq!(after.notes.as_deref(), Some("packing note"));
    assert_eq!(after.tracking_number.as_deref(), Some("TRACK"));
    assert_eq!(after.version, 3);
}

#[test]
fn concurrent_expected_version_writers_have_one_winner() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    let s = shipment(&db);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|note| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                db.shipments().update(
                    s.id,
                    UpdateShipment {
                        expected_version: Some(1),
                        notes: Some(note.into()),
                        ..Default::default()
                    },
                )
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.iter().filter(|r| matches!(r, Err(CommerceError::VersionConflict { .. }))).count(),
        1
    );
    assert_eq!(db.shipments().get(s.id).unwrap().unwrap().version, 2);
}

#[test]
fn creation_and_atomic_creation_roll_back_when_audit_is_unavailable() {
    let db = SqliteDatabase::in_memory().unwrap();
    db.conn().unwrap().execute_batch("CREATE TRIGGER reject_shipment_create BEFORE INSERT ON kernel_outbox WHEN NEW.aggregate_type = 'shipment' BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;").unwrap();
    let input = CreateShipment {
        order_id: order(&db).id,
        recipient_name: "Ada".into(),
        shipping_address: "1 Main".into(),
        items: Some(vec![CreateShipmentItem {
            sku: "W-1".into(),
            name: "Widget".into(),
            quantity: 1,
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert!(db.shipments().create(input.clone()).is_err());
    assert!(db.shipments().create_batch_atomic(vec![input]).is_err());
    assert_eq!(db.shipments().count(Default::default()).unwrap(), 0);
    let items: i64 = db
        .conn()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM shipment_items", [], |row| row.get(0))
        .unwrap();
    assert_eq!(items, 0);
}

#[test]
fn shipment_version_migration_preserves_existing_records_and_can_be_rerun() {
    let db = SqliteDatabase::in_memory().unwrap();
    let s = shipment(&db);
    {
        let mut conn = db.conn().unwrap();
        conn.execute_batch("ALTER TABLE shipments DROP COLUMN version; DELETE FROM _migrations WHERE name = '097_shipment_version';").unwrap();
        stateset_db::migrations::run_migrations(&mut conn).unwrap();
        stateset_db::migrations::run_migrations(&mut conn).unwrap();
    }
    let stored = db.shipments().get(s.id).unwrap().unwrap();
    assert_eq!(stored.version, 1);
    assert_eq!(stored.recipient_name, s.recipient_name);
    assert_eq!(stored.created_at, s.created_at);
    assert_eq!(db.shipments().mark_processing(s.id).unwrap().version, 2);
}
