#![cfg(feature = "postgres")]

use stateset_core::{
    AddShipmentEvent, CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreateProduct,
    CreateShipment, CreateShipmentItem, Shipment, ShipmentStatus, UpdateShipment,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

async fn database() -> Option<PostgresDatabase> {
    let url = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")).ok()?;
    Some(PostgresDatabase::connect(url).await.expect("connect and migrate"))
}

fn item_input() -> CreateShipmentItem {
    CreateShipmentItem {
        sku: "W-1".into(),
        name: "Widget".into(),
        quantity: 1,
        ..Default::default()
    }
}

fn tracking_input() -> AddShipmentEvent {
    AddShipmentEvent {
        event_type: "arrived_at_hub".into(),
        location: Some("Vancouver".into()),
        description: Some("Carrier observation".into()),
        event_time: Some("2024-02-03T04:05:06.123456789Z".parse().unwrap()),
    }
}

#[tokio::test]
async fn postgres_tracking_append_versions_parent_and_binds_persisted_event_to_outbox() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let event = repo.add_event_async(id, tracking_input()).await.unwrap();
    let stored = repo.get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, s.version + 1);
    assert_eq!(stored.status, s.status);
    assert_eq!(stored.updated_at, event.created_at);
    assert_eq!(event.event_time.timestamp_subsec_nanos(), 123_456_000);
    assert_eq!(event.created_at.timestamp_subsec_nanos() % 1_000, 0);
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(serde_json::to_value(&stored.events[0]).unwrap(), value);
    assert_eq!(serde_json::to_value(&repo.get_events_async(id).await.unwrap()[0]).unwrap(), value);
    let fact: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = 'shipments.event_added.v1'"
    ).bind(id.to_string()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(fact["event"], value);
    assert_eq!(fact["previous_version"], s.version);
    assert_eq!(fact["version"], stored.version);
    assert_eq!(fact["changed_fields"], serde_json::json!(["events"]));
    assert!(matches!(
        repo.update_async(
            id,
            UpdateShipment {
                expected_version: Some(s.version),
                notes: Some("stale".into()),
                ..Default::default()
            }
        )
        .await,
        Err(CommerceError::VersionConflict { .. })
    ));
    assert_eq!(event_count(&db, id).await, 2);
}

#[tokio::test]
async fn postgres_tracking_rejects_empty_types_and_missing_parent_without_effects() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let before = serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap();
    for event_type in ["", " \t\n"] {
        assert!(matches!(
            repo.add_event_async(
                id,
                AddShipmentEvent { event_type: event_type.into(), ..tracking_input() }
            )
            .await,
            Err(CommerceError::ValidationError(_))
        ));
    }
    assert!(matches!(
        repo.add_event_async(Uuid::new_v4(), tracking_input()).await,
        Err(CommerceError::NotFound)
    ));
    assert_eq!(serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap(), before);
    assert_eq!(event_count(&db, id).await, 1);
}

#[tokio::test]
async fn postgres_tracking_text_limits_count_unicode_characters() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let before = serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap();
    for input in [
        AddShipmentEvent { event_type: "é".repeat(101), ..tracking_input() },
        AddShipmentEvent { location: Some("港".repeat(256)), ..tracking_input() },
        AddShipmentEvent { event_type: "scan\0".into(), ..tracking_input() },
        AddShipmentEvent { location: Some("hub\0".into()), ..tracking_input() },
        AddShipmentEvent { description: Some("note\0".into()), ..tracking_input() },
    ] {
        assert!(matches!(
            repo.add_event_async(id, input).await,
            Err(CommerceError::ValidationError(_))
        ));
    }
    assert_eq!(serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap(), before);
    assert_eq!(event_count(&db, id).await, 1);
    let event = repo
        .add_event_async(
            id,
            AddShipmentEvent {
                event_type: "é".repeat(100),
                location: Some("港".repeat(255)),
                ..tracking_input()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&repo.get_events_async(id).await.unwrap()[0]).unwrap(),
        serde_json::to_value(event).unwrap()
    );
    assert_eq!(repo.get_async(id).await.unwrap().unwrap().version, 2);
}

#[tokio::test]
async fn postgres_late_tracking_observations_preserve_terminal_status_and_fulfillment() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    for status in [ShipmentStatus::Cancelled, ShipmentStatus::Delivered] {
        let s = shipment(&db).await;
        let id = s.id.into_uuid();
        if status == ShipmentStatus::Cancelled {
            repo.cancel_async(id).await.unwrap();
        } else {
            ready(&db, id).await;
            repo.ship_async(id, None).await.unwrap();
            repo.mark_in_transit_async(id).await.unwrap();
            repo.mark_out_for_delivery_async(id).await.unwrap();
            repo.mark_delivered_async(id).await.unwrap();
        }
        let before = repo.get_async(id).await.unwrap().unwrap();
        let order_before =
            serde_json::to_value(db.orders().get_async(s.order_id.into_uuid()).await.unwrap())
                .unwrap();
        let first = repo.add_event_async(id, tracking_input()).await.unwrap();
        let second = repo.add_event_async(id, tracking_input()).await.unwrap();
        assert_ne!(first.id, second.id);
        let after = repo.get_async(id).await.unwrap().unwrap();
        assert_eq!(after.status, status);
        assert_eq!(after.version, before.version + 2);
        assert_eq!(after.shipped_at, before.shipped_at);
        assert_eq!(after.delivered_at, before.delivered_at);
        assert_eq!(after.events.len(), 2);
        assert_eq!(
            serde_json::to_value(db.orders().get_async(s.order_id.into_uuid()).await.unwrap())
                .unwrap(),
            order_before
        );
    }
}

#[tokio::test]
async fn postgres_tracking_audit_failure_and_version_overflow_roll_back_event_and_parent() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let before = repo.get_async(id).await.unwrap().unwrap();
    let trigger = format!("shipment_tracking_{}", id.simple());
    sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'audit unavailable'; END $$")).execute(db.pool()).await.unwrap();
    sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON kernel_outbox FOR EACH ROW WHEN (NEW.aggregate_id = '{id}' AND NEW.event_type = 'shipments.event_added.v1') EXECUTE FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    let result = repo.add_event_async(id, tracking_input()).await;
    sqlx::query(&format!("DROP TRIGGER {trigger} ON kernel_outbox"))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    assert!(result.is_err());
    assert_eq!(
        serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(event_count(&db, id).await, 1);
    sqlx::query("UPDATE shipments SET version = $1 WHERE id = $2")
        .bind(i32::MAX)
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        repo.add_event_async(id, tracking_input())
            .await
            .unwrap_err()
            .to_string()
            .contains("exhausted")
    );
    let stored = repo.get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, i32::MAX);
    assert_eq!(stored.updated_at, before.updated_at);
    assert!(stored.events.is_empty());
    assert_eq!(event_count(&db, id).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_tracking_appends_and_cancellation_serialize_versions() {
    let Some(db) = database().await else {
        return;
    };
    let db = Arc::new(db);
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let barrier = Arc::new(Barrier::new(5));
    let mut tasks = Vec::new();
    for index in 0..5 {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            if index == 4 {
                db.shipments().cancel_async(id).await.unwrap();
            } else {
                db.shipments().add_event_async(id, tracking_input()).await.unwrap();
            }
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.status, ShipmentStatus::Cancelled);
    assert_eq!(stored.version, 6);
    assert_eq!(stored.events.len(), 4);
    let versions: Vec<i32> = sqlx::query_scalar(
        "SELECT (payload->>'version')::integer FROM kernel_outbox WHERE aggregate_id = $1 ORDER BY (payload->>'version')::integer"
    ).bind(id.to_string()).fetch_all(db.pool()).await.unwrap();
    assert_eq!(versions, vec![1, 2, 3, 4, 5, 6]);
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

#[tokio::test]
async fn postgres_allocations_bind_identity_enforce_order_budget_and_preserve_history() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let source = db.orders().get_async(s.order_id.into_uuid()).await.unwrap().unwrap();
    let foreign = shipment(&db).await;
    let foreign_order = db.orders().get_async(foreign.order_id.into_uuid()).await.unwrap().unwrap();
    let id = s.id.into_uuid();
    let line = &source.items[0];
    for invalid in [
        CreateShipmentItem {
            order_item_id: Some(foreign_order.items[0].id.into_uuid()),
            ..item_input()
        },
        CreateShipmentItem {
            order_item_id: Some(line.id.into_uuid()),
            sku: "OTHER".into(),
            ..item_input()
        },
        CreateShipmentItem { product_id: Some(stateset_core::ProductId::new()), ..item_input() },
    ] {
        assert!(repo.add_item_async(id, invalid.clone()).await.is_err());
        assert!(
            repo.create_async(CreateShipment {
                items: Some(vec![invalid]),
                ..allocation(s.order_id, 1)
            })
            .await
            .is_err()
        );
    }
    let first =
        repo.add_item_async(id, CreateShipmentItem { quantity: 2, ..item_input() }).await.unwrap();
    assert_eq!(first.order_item_id, Some(line.id.into_uuid()));
    assert_eq!(first.product_id, Some(line.product_id));
    let second = repo.create_async(allocation(s.order_id, 3)).await.unwrap();
    assert!(repo.add_item_async(id, item_input()).await.is_err());
    assert!(repo.create_async(allocation(s.order_id, 1)).await.is_err());
    assert!(repo.create_batch_atomic_async(vec![allocation(s.order_id, 1)]).await.is_err());
    assert!(
        db.orders()
            .remove_item_async(s.order_id.into_uuid(), line.id.into_uuid())
            .await
            .unwrap_err()
            .to_string()
            .contains("shipment history")
    );
    assert!(
        db.orders()
            .delete_async(s.order_id.into_uuid())
            .await
            .unwrap_err()
            .to_string()
            .contains("shipment history")
    );
    repo.cancel_async(second.id.into_uuid()).await.unwrap();
    let replacement = repo.create_async(allocation(s.order_id, 3)).await.unwrap();
    assert_eq!(replacement.items[0].order_item_id, Some(line.id.into_uuid()));
    repo.cancel_async(id).await.unwrap();
    repo.cancel_async(replacement.id.into_uuid()).await.unwrap();
    assert!(
        db.orders().remove_item_async(s.order_id.into_uuid(), line.id.into_uuid()).await.is_err()
    );
    assert!(db.orders().delete_async(s.order_id.into_uuid()).await.is_err());
    assert_eq!(
        db.orders().get_async(s.order_id.into_uuid()).await.unwrap().unwrap().items[0]
            .shipped_quantity,
        0
    );
}

#[tokio::test]
async fn postgres_allocation_counts_legacy_items_and_refuses_ambiguous_skus() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    repo.add_item_async(id, CreateShipmentItem { quantity: 4, ..item_input() }).await.unwrap();
    sqlx::query(
        "UPDATE shipment_items SET order_item_id = NULL, product_id = NULL WHERE shipment_id = $1",
    )
    .bind(id)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        repo.add_item_async(id, CreateShipmentItem { quantity: 2, ..item_input() }).await.is_err()
    );
    let last = repo.add_item_async(id, item_input()).await.unwrap();
    assert!(last.order_item_id.is_some());
    assert_eq!(
        repo.get_async(id).await.unwrap().unwrap().items.iter().map(|i| i.quantity).sum::<i32>(),
        5
    );
    let source = db.orders().get_async(s.order_id.into_uuid()).await.unwrap().unwrap();
    let extra = CreateOrderItem {
        product_id: source.items[0].product_id,
        sku: "W-1".into(),
        name: "Another line".into(),
        quantity: 5,
        unit_price: rust_decimal::Decimal::ONE,
        ..Default::default()
    };
    let added_line = db.orders().add_item_async(s.order_id.into_uuid(), extra).await.unwrap();
    assert!(
        repo.add_item_async(id, item_input()).await.unwrap_err().to_string().contains("Ambiguous")
    );
    assert!(
        repo.add_item_async(
            id,
            CreateShipmentItem { order_item_id: Some(added_line.id.into_uuid()), ..item_input() }
        )
        .await
        .is_err()
    );
    let clean = shipment(&db).await;
    let clean_order = db.orders().get_async(clean.order_id.into_uuid()).await.unwrap().unwrap();
    let extra = CreateOrderItem {
        product_id: clean_order.items[0].product_id,
        sku: "W-1".into(),
        name: "Another line".into(),
        quantity: 5,
        unit_price: rust_decimal::Decimal::ONE,
        ..Default::default()
    };
    let second_line = db.orders().add_item_async(clean.order_id.into_uuid(), extra).await.unwrap();
    assert!(repo.add_item_async(clean.id.into_uuid(), item_input()).await.is_err());
    assert_eq!(
        repo.add_item_async(
            clean.id.into_uuid(),
            CreateShipmentItem { order_item_id: Some(second_line.id.into_uuid()), ..item_input() }
        )
        .await
        .unwrap()
        .order_item_id,
        Some(second_line.id.into_uuid())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_competing_allocation_batches_are_atomic_and_cannot_double_assign_units() {
    let Some(db) = database().await else {
        return;
    };
    let db = Arc::new(db);
    let a = shipment(&db).await;
    let b = shipment(&db).await;
    assert!(
        db.shipments()
            .create_batch_atomic_async(vec![allocation(a.order_id, 3), allocation(a.order_id, 3)])
            .await
            .is_err()
    );
    assert_eq!(db.shipments().for_order_async(a.order_id.into_uuid()).await.unwrap().len(), 1);
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = [vec![a.order_id, b.order_id], vec![b.order_id, a.order_id]]
        .into_iter()
        .map(|ids| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.shipments()
                    .create_batch_atomic_async(
                        ids.into_iter().map(|id| allocation(id, 3)).collect(),
                    )
                    .await
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    for id in [a.order_id, b.order_id] {
        let shipments = db.shipments().for_order_async(id.into_uuid()).await.unwrap();
        assert_eq!(shipments.len(), 2); // Initial empty manifest plus exactly one allocation.
        let assigned: Vec<_> = shipments.into_iter().filter(|s| !s.items.is_empty()).collect();
        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].items[0].quantity, 3);
        assert_eq!(event_count(&db, assigned[0].id.into_uuid()).await, 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_order_line_removal_and_item_allocation_have_one_winner() {
    let Some(db) = database().await else {
        return;
    };
    let db = Arc::new(db);
    let s = shipment(&db).await;
    let line = db.orders().get_async(s.order_id.into_uuid()).await.unwrap().unwrap().items[0]
        .id
        .into_uuid();
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = [true, false]
        .into_iter()
        .map(|add| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                if add {
                    db.shipments().add_item_async(s.id.into_uuid(), item_input()).await.map(|_| ())
                } else {
                    db.orders().remove_item_async(s.order_id.into_uuid(), line).await
                }
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let items = db.shipments().get_items_async(s.id.into_uuid()).await.unwrap();
    let lines = db.orders().get_async(s.order_id.into_uuid()).await.unwrap().unwrap().items;
    assert_eq!(items.len(), lines.len());
    if let Some(item) = items.first() {
        assert_eq!(item.order_item_id, Some(lines[0].id.into_uuid()));
    }
}

#[tokio::test]
async fn postgres_item_mutations_advance_version_and_preserve_audit_contents() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let item = repo.add_item_async(id, item_input()).await.unwrap();
    let added = repo.get_async(id).await.unwrap().unwrap();
    assert_eq!(added.version, 2);
    assert_eq!(added.updated_at, item.created_at);
    assert_eq!(
        serde_json::to_value(&added.items[0]).unwrap(),
        serde_json::to_value(&item).unwrap()
    );
    assert!(matches!(
        repo.update_async(
            id,
            UpdateShipment {
                expected_version: Some(1),
                notes: Some("stale".into()),
                ..Default::default()
            }
        )
        .await,
        Err(CommerceError::VersionConflict { .. })
    ));
    repo.remove_item_async(item.id).await.unwrap();
    let removed = repo.get_async(id).await.unwrap().unwrap();
    assert_eq!(removed.version, 3);
    assert!(removed.items.is_empty());
    assert!(matches!(repo.remove_item_async(item.id).await, Err(CommerceError::NotFound)));
    assert!(matches!(
        repo.add_item_async(Uuid::new_v4(), item_input()).await,
        Err(CommerceError::NotFound)
    ));
    for (event_type, version) in [("shipments.item_added.v1", 2), ("shipments.item_removed.v1", 3)]
    {
        let payload: serde_json::Value = sqlx::query_scalar(
            "SELECT payload FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = $2",
        )
        .bind(id.to_string())
        .bind(event_type)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(payload["version"], version);
        assert_eq!(payload["previous_version"], version - 1);
        assert_eq!(payload["item"]["id"], item.id.to_string());
        assert_eq!(payload["item"]["quantity"], 1);
        assert_eq!(payload["item"]["sku"], "W-1");
    }
    assert_eq!(event_count(&db, id).await, 3);
}

#[tokio::test]
async fn postgres_item_contents_are_frozen_from_ready_through_terminal_states() {
    let Some(db) = database().await else {
        return;
    };
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
        let s = shipment(&db).await;
        let id = s.id.into_uuid();
        let item = repo.add_item_async(id, item_input()).await.unwrap();
        sqlx::query("UPDATE shipments SET status = $1 WHERE id = $2")
            .bind(status.to_string())
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
        let before = repo.get_async(id).await.unwrap().unwrap();
        assert!(repo.add_item_async(id, item_input()).await.is_err(), "{status}");
        assert!(repo.remove_item_async(item.id).await.is_err(), "{status}");
        assert_eq!(
            serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert_eq!(event_count(&db, id).await, 2);
    }
    for status in [ShipmentStatus::Processing, ShipmentStatus::OnHold] {
        let s = shipment(&db).await;
        let id = s.id.into_uuid();
        repo.update_async(id, UpdateShipment { status: Some(status), ..Default::default() })
            .await
            .unwrap();
        let item = repo.add_item_async(id, item_input()).await.unwrap();
        repo.remove_item_async(item.id).await.unwrap();
        assert_eq!(repo.get_async(id).await.unwrap().unwrap().version, 4);
    }
}

#[tokio::test]
async fn postgres_invalid_initial_items_roll_back_batches_and_valid_items_are_audited() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let input = CreateShipment {
        order_id: s.order_id,
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
        let bad = CreateShipment { items: Some(vec![invalid.clone()]), ..input.clone() };
        assert!(repo.create_async(bad.clone()).await.is_err());
        assert!(repo.create_batch_atomic_async(vec![input.clone(), bad]).await.is_err());
        assert!(repo.add_item_async(s.id.into_uuid(), invalid).await.is_err());
        assert_eq!(repo.for_order_async(s.order_id.into_uuid()).await.unwrap().len(), 1);
        let facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_type = 'shipment' AND payload->>'order_id' = $1").bind(s.order_id.to_string()).fetch_one(db.pool()).await.unwrap();
        assert_eq!(facts, 1);
    }
    let single = repo.create_async(input.clone()).await.unwrap();
    let batch = repo.create_batch_atomic_async(vec![input]).await.unwrap();
    for created in std::iter::once(single).chain(batch) {
        let payload: serde_json::Value = sqlx::query_scalar("SELECT payload FROM kernel_outbox WHERE aggregate_id = $1 AND event_type = 'shipments.created.v1'").bind(created.id.to_string()).fetch_one(db.pool()).await.unwrap();
        assert_eq!(payload["items"][0]["id"], created.items[0].id.to_string());
        assert_eq!(payload["items"][0]["quantity"], 1);
    }
}

#[tokio::test]
async fn postgres_item_audit_failure_and_version_overflow_roll_back_items_and_parent() {
    let Some(db) = database().await else {
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let item = repo.add_item_async(id, item_input()).await.unwrap();
    let before = repo.get_async(id).await.unwrap().unwrap();
    let trigger = format!("shipment_items_{}", id.simple());
    sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'audit unavailable'; END $$")).execute(db.pool()).await.unwrap();
    // Other lifecycle tests share this database and may run concurrently.
    sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON kernel_outbox FOR EACH ROW WHEN (NEW.aggregate_id = '{id}') EXECUTE FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    let add = repo.add_item_async(id, item_input()).await;
    let remove = repo.remove_item_async(item.id).await;
    sqlx::query(&format!("DROP TRIGGER {trigger} ON kernel_outbox"))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    assert!(add.is_err());
    assert!(remove.is_err());
    assert_eq!(
        serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert_eq!(event_count(&db, id).await, 2);
    sqlx::query("UPDATE shipments SET version = $1 WHERE id = $2")
        .bind(i32::MAX)
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(repo.add_item_async(id, item_input()).await.is_err());
    assert!(repo.remove_item_async(item.id).await.is_err());
    let stored = repo.get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, i32::MAX);
    assert_eq!(stored.items.len(), 1);
    assert_eq!(event_count(&db, id).await, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_item_additions_and_duplicate_removals_serialize_parent_versions() {
    let Some(db) = database().await else {
        return;
    };
    let db = Arc::new(db);
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.shipments().add_item_async(id, item_input()).await.unwrap()
            })
        })
        .collect();
    let mut items = Vec::new();
    for task in tasks {
        items.push(task.await.unwrap());
    }
    assert_eq!(db.shipments().get_async(id).await.unwrap().unwrap().version, 3);
    assert_eq!(event_count(&db, id).await, 3);
    let item_id = items[0].id;
    let tasks: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.shipments().remove_item_async(item_id).await
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results.iter().any(|r| matches!(r, Err(CommerceError::NotFound))));
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, 4);
    assert_eq!(stored.items.len(), 1);
    assert_eq!(event_count(&db, id).await, 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_item_addition_and_versioned_ready_transition_have_one_winner() {
    let Some(db) = database().await else {
        return;
    };
    let db = Arc::new(db);
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    db.shipments().mark_processing_async(id).await.unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = [true, false]
        .into_iter()
        .map(|add| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                if add {
                    db.shipments().add_item_async(id, item_input()).await.map(|_| ())
                } else {
                    db.shipments()
                        .update_async(
                            id,
                            UpdateShipment {
                                status: Some(ShipmentStatus::ReadyToShip),
                                expected_version: Some(2),
                                ..Default::default()
                            },
                        )
                        .await
                        .map(|_| ())
                }
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, 3);
    assert_eq!(stored.items.len(), usize::from(stored.status == ShipmentStatus::Processing));
    assert_eq!(event_count(&db, id).await, 3);
}

async fn shipment(db: &PostgresDatabase) -> Shipment {
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("{}@example.com", Uuid::new_v4()),
            first_name: "Ada".into(),
            last_name: "L".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let product = db
        .products()
        .create_async(CreateProduct {
            name: format!("Widget {}", Uuid::new_v4()),
            ..Default::default()
        })
        .await
        .unwrap();
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: "W-1".into(),
                name: "Widget".into(),
                quantity: 5,
                unit_price: rust_decimal::Decimal::ONE,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    db.shipments()
        .create_async(CreateShipment {
            order_id: order.id,
            recipient_name: "Ada".into(),
            shipping_address: "1 Main".into(),
            ..Default::default()
        })
        .await
        .unwrap()
}

async fn event_count(db: &PostgresDatabase, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_type = 'shipment' AND aggregate_id = $1")
        .bind(id.to_string()).fetch_one(db.pool()).await.unwrap()
}

async fn ready(db: &PostgresDatabase, id: Uuid) {
    db.shipments().mark_processing_async(id).await.unwrap();
    db.shipments().mark_ready_async(id).await.unwrap();
}

#[tokio::test]
async fn postgres_shipment_lifecycle_versions_replay_and_invalid_routes() {
    let Some(db) = database().await else {
        eprintln!("POSTGRES_URL not set; skipped");
        return;
    };
    let repo = db.shipments();
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    assert_eq!(event_count(&db, id).await, 1);
    assert!(repo.ship_async(id, None).await.is_err());
    assert!(repo.mark_delivered_async(id).await.is_err());
    ready(&db, id).await;
    let shipped = repo.ship_async(id, Some("TRACK".into())).await.unwrap();
    assert_eq!(shipped.version, 4);
    assert!(shipped.shipped_at.is_some());
    let count = event_count(&db, id).await;
    let replay = repo.ship_async(id, Some("TRACK".into())).await.unwrap();
    assert_eq!(replay.version, 4);
    assert_eq!(replay.updated_at, shipped.updated_at);
    assert_eq!(replay.shipped_at, shipped.shipped_at);
    assert_eq!(event_count(&db, id).await, count);
    repo.mark_in_transit_async(id).await.unwrap();
    repo.mark_out_for_delivery_async(id).await.unwrap();
    let delivered = repo.mark_delivered_async(id).await.unwrap();
    assert_eq!(delivered.version, 7);
    assert!(delivered.delivered_at.is_some());
    assert!(repo.cancel_async(id).await.is_err());
    assert!(repo.delete_async(id).await.is_err());
    assert!(
        repo.update_async(
            id,
            UpdateShipment { status: Some(ShipmentStatus::Pending), ..Default::default() }
        )
        .await
        .is_err()
    );
    assert!(repo.delete_async(Uuid::new_v4()).await.is_err());
    assert_eq!(repo.get_batch_async(vec![s.id]).await.unwrap()[0].version, 7);
}

#[tokio::test]
async fn postgres_shipment_atomic_batches_roll_back_writes_and_facts() {
    let Some(db) = database().await else {
        eprintln!("POSTGRES_URL not set; skipped");
        return;
    };
    let a = shipment(&db).await;
    let b = shipment(&db).await;
    let count = event_count(&db, a.id.into_uuid()).await;
    assert!(
        db.shipments()
            .update_batch_atomic_async(vec![
                (a.id, UpdateShipment { notes: Some("rollback".into()), ..Default::default() }),
                (
                    b.id,
                    UpdateShipment {
                        status: Some(ShipmentStatus::Delivered),
                        ..Default::default()
                    }
                ),
            ])
            .await
            .is_err()
    );
    assert_eq!(db.shipments().get_async(a.id.into_uuid()).await.unwrap().unwrap().notes, None);
    assert_eq!(event_count(&db, a.id.into_uuid()).await, count);
    db.shipments()
        .add_item_async(
            a.id.into_uuid(),
            CreateShipmentItem {
                sku: "W-1".into(),
                name: "Widget".into(),
                quantity: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    db.shipments()
        .add_event_async(
            a.id.into_uuid(),
            AddShipmentEvent {
                event_type: "label_created".into(),
                location: None,
                description: None,
                event_time: None,
            },
        )
        .await
        .unwrap();
    ready(&db, b.id.into_uuid()).await;
    db.shipments().ship_async(b.id.into_uuid(), None).await.unwrap();
    assert!(db.shipments().delete_batch_atomic_async(vec![a.id, b.id]).await.is_err());
    assert_eq!(
        db.shipments().get_async(a.id.into_uuid()).await.unwrap().unwrap().status,
        ShipmentStatus::Pending
    );
    db.shipments().delete_batch_atomic_async(vec![a.id]).await.unwrap();
    let cancelled = db.shipments().get_async(a.id.into_uuid()).await.unwrap().unwrap();
    assert_eq!(cancelled.status, ShipmentStatus::Cancelled);
    assert_eq!(cancelled.items.len(), 1);
    assert_eq!(cancelled.events.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_shipment_concurrent_patches_and_expected_version() {
    let Some(db) = database().await else {
        eprintln!("POSTGRES_URL not set; skipped");
        return;
    };
    let db = Arc::new(db);
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = Vec::new();
    for patch in [
        UpdateShipment { notes: Some("packing note".into()), ..Default::default() },
        UpdateShipment { tracking_number: Some("TRACK".into()), ..Default::default() },
    ] {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            db.shipments().update_async(id, patch).await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, 3);
    assert_eq!(stored.notes.as_deref(), Some("packing note"));
    assert_eq!(stored.tracking_number.as_deref(), Some("TRACK"));
    let mut tasks = Vec::new();
    for note in ["first", "second"] {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            db.shipments()
                .update_async(
                    id,
                    UpdateShipment {
                        expected_version: Some(3),
                        notes: Some(note.into()),
                        ..Default::default()
                    },
                )
                .await
        }));
    }
    let mut wins = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => wins += 1,
            Err(CommerceError::VersionConflict { .. }) => {}
            Err(error) => panic!("unexpected {error}"),
        }
    }
    assert_eq!(wins, 1);
    assert_eq!(db.shipments().get_async(id).await.unwrap().unwrap().version, 4);
}

#[tokio::test]
async fn postgres_shipment_outbox_failure_rolls_back_update() {
    let Some(db) = database().await else {
        eprintln!("POSTGRES_URL not set; skipped");
        return;
    };
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let trigger = format!("shipment_audit_{}", id.simple());
    sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'audit unavailable'; END $$" )).execute(db.pool()).await.unwrap();
    // Restrict failure injection to this shipment even with parallel test workers.
    sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON kernel_outbox FOR EACH ROW WHEN (NEW.aggregate_id = '{id}') EXECUTE FUNCTION {trigger}()" )).execute(db.pool()).await.unwrap();
    let result = db.shipments().mark_processing_async(id).await;
    sqlx::query(&format!("DROP TRIGGER {trigger} ON kernel_outbox"))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    assert!(result.is_err());
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.status, ShipmentStatus::Pending);
    assert_eq!(stored.version, 1);
}

#[tokio::test]
async fn postgres_shipment_creation_rolls_back_when_audit_is_unavailable() {
    let Some(db) = database().await else {
        eprintln!("POSTGRES_URL not set; skipped");
        return;
    };
    let s = shipment(&db).await;
    let trigger = format!("shipment_create_{}", s.id.into_uuid().simple());
    sqlx::query(&format!("CREATE FUNCTION {trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'audit unavailable'; END $$")).execute(db.pool()).await.unwrap();
    sqlx::query(&format!("CREATE TRIGGER {trigger} BEFORE INSERT ON kernel_outbox FOR EACH ROW WHEN (NEW.event_type = 'shipments.created.v1' AND NEW.payload->>'order_id' = '{}') EXECUTE FUNCTION {trigger}()", s.order_id)).execute(db.pool()).await.unwrap();
    let input = CreateShipment {
        order_id: s.order_id,
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
    let single = db.shipments().create_async(input.clone()).await;
    let batch = db.shipments().create_batch_atomic_async(vec![input]).await;
    sqlx::query(&format!("DROP TRIGGER {trigger} ON kernel_outbox"))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(&format!("DROP FUNCTION {trigger}()")).execute(db.pool()).await.unwrap();
    assert!(single.is_err());
    assert!(batch.is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shipments WHERE order_id = $1")
        .bind(s.order_id.into_uuid())
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn postgres_versioned_packing_edits_refuse_stale_reads_without_mutation() {
    let Some(db) = database().await else {
        return;
    };
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let repo = db.shipments();
    let item = repo.add_item_with_version_async(id, item_input(), Some(s.version)).await.unwrap();
    let current = repo.get_async(id).await.unwrap().unwrap();
    for version in [s.version, -1, i32::MAX] {
        assert!(
            matches!(repo.add_item_with_version_async(id, CreateShipmentItem { quantity: 5, ..item_input() }, Some(version)).await,
            Err(CommerceError::VersionConflict { expected_version, .. }) if expected_version == version)
        );
        assert!(matches!(repo.remove_item_with_version_async(item.id, Some(version)).await,
            Err(CommerceError::VersionConflict { expected_version, .. }) if expected_version == version));
        assert_eq!(
            serde_json::to_value(repo.get_async(id).await.unwrap().unwrap()).unwrap(),
            serde_json::to_value(&current).unwrap()
        );
        assert_eq!(event_count(&db, id).await, 2);
    }
    repo.remove_item_with_version_async(item.id, Some(current.version)).await.unwrap();
    let stored = repo.get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, current.version + 1);
    assert!(stored.items.is_empty());
    assert_eq!(event_count(&db, id).await, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_versioned_packing_edits_have_one_winner() {
    let Some(db) = database().await else {
        return;
    };
    let db = Arc::new(db);
    let s = shipment(&db).await;
    let id = s.id.into_uuid();
    let barrier = Arc::new(Barrier::new(2));
    let tasks: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                db.shipments().add_item_with_version_async(id, item_input(), Some(s.version)).await
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.iter().filter(|r| matches!(r, Err(CommerceError::VersionConflict { .. }))).count(),
        1
    );
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, 2);
    assert_eq!(stored.items.len(), 1);
    assert_eq!(event_count(&db, id).await, 2);
    let item_id = stored.items[0].id;
    let tasks: Vec<_> = (0..2)
        .map(|index| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                if index == 0 {
                    db.shipments()
                        .add_item_with_version_async(id, item_input(), Some(2))
                        .await
                        .map(|_| ())
                } else {
                    db.shipments().remove_item_with_version_async(item_id, Some(2)).await
                }
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.iter().filter(|r| matches!(r, Err(CommerceError::VersionConflict { .. }))).count(),
        1
    );
    let stored = db.shipments().get_async(id).await.unwrap().unwrap();
    assert_eq!(stored.version, 3);
    assert!(matches!(stored.items.len(), 0 | 2));
    assert_eq!(event_count(&db, id).await, 3);
}
