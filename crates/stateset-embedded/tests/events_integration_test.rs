#![cfg(feature = "sqlite")]

//! Integration tests for the event infrastructure:
//! `EventBus`, `EventEmitter`, `EventStore`, `EventSystem`, filtered subscriptions,
//! notification service integration, and webhook registration edge cases.

use chrono::Utc;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceEvent, CustomerId, EventStore, OrderId, OrderStatus, ProductId, ReturnId, ReturnReason,
};
use stateset_embedded::events::{
    EventBus, EventConfig, EventEmitter, EventSystem, InMemoryEventStore, Webhook,
    WebhookRegistrationError,
};
use stateset_embedded::notifications::{
    EmailTemplate, LogEmailBackend, NotificationConfig, NotificationService,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// EventBus: advanced scenarios
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bus_multiple_event_types_delivered() {
    let bus = EventBus::new(32);
    let mut sub = bus.subscribe();

    let order_event = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(50.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    let customer_event = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "multi@test.com".into(),
        timestamp: Utc::now(),
    };

    bus.publish(order_event);
    bus.publish(customer_event);

    let e1 = sub.try_recv().expect("first event");
    assert_eq!(e1.event_type(), "order_created");
    let e2 = sub.try_recv().expect("second event");
    assert_eq!(e2.event_type(), "customer_created");
    assert!(sub.try_recv().is_none());
}

#[test]
fn bus_subscriber_dropped_reduces_count() {
    let bus = EventBus::new(8);
    assert_eq!(bus.receiver_count(), 0);

    let sub1 = bus.subscribe();
    assert_eq!(bus.receiver_count(), 1);

    let sub2 = bus.subscribe();
    assert_eq!(bus.receiver_count(), 2);

    drop(sub1);
    assert_eq!(bus.receiver_count(), 1);

    drop(sub2);
    assert_eq!(bus.receiver_count(), 0);
}

#[test]
fn bus_publish_counters_accumulate() {
    let bus = EventBus::new(4);
    assert_eq!(bus.events_published(), 0);
    assert_eq!(bus.events_publish_failures(), 0);

    // No subscribers => failure path
    for _ in 0..3 {
        bus.publish(CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: "counter@test.com".into(),
            timestamp: Utc::now(),
        });
    }
    assert_eq!(bus.events_published(), 3);
    assert_eq!(bus.events_publish_failures(), 3);

    // With subscriber => success path
    let _sub = bus.subscribe();
    bus.publish(CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "counter2@test.com".into(),
        timestamp: Utc::now(),
    });
    assert_eq!(bus.events_published(), 4);
    assert_eq!(bus.events_publish_failures(), 3); // unchanged
}

#[test]
fn bus_debug_format() {
    let bus = EventBus::new(16);
    let debug = format!("{bus:?}");
    assert!(debug.contains("EventBus"));
    assert!(debug.contains("events_published"));
}

// ---------------------------------------------------------------------------
// EventEmitter: convenience methods
// ---------------------------------------------------------------------------

#[test]
fn emitter_order_created_convenience() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let _sub = bus.subscribe();

    emitter.order_created(OrderId::new(), CustomerId::new(), dec!(99.99), 2);

    assert_eq!(emitter.total_events(), 1);
    assert_eq!(emitter.subscriber_count(), 1);
}

#[test]
fn emitter_order_status_changed() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.order_status_changed(OrderId::new(), OrderStatus::Pending, OrderStatus::Confirmed);

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "order_status_changed");
}

#[test]
fn emitter_order_cancelled() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.order_cancelled(OrderId::new(), Some("Customer request".into()));

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "order_cancelled");
}

#[test]
fn emitter_customer_created() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.customer_created(CustomerId::new(), "test@test.com".into());

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "customer_created");
}

#[test]
fn emitter_product_created() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.product_created(ProductId::new(), "Widget".into(), "widget".into());

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "product_created");
}

#[test]
fn emitter_inventory_adjusted() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.inventory_adjusted(1, "SKU-001".into(), 1, dec!(-5), dec!(95), "sale".into());

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "inventory_adjusted");
}

#[test]
fn emitter_low_stock_alert() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.low_stock_alert("SKU-LOW".into(), 1, dec!(3), dec!(10));

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "low_stock_alert");
}

#[test]
fn emitter_return_requested() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.return_requested(
        ReturnId::new(),
        OrderId::new(),
        CustomerId::new(),
        ReturnReason::Defective,
        2,
    );

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "return_requested");
}

#[test]
fn emitter_return_approved() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.return_approved(ReturnId::new(), OrderId::new());

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "return_approved");
}

#[test]
fn emitter_refund_issued() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    emitter.refund_issued(ReturnId::new(), OrderId::new(), dec!(25.50), "credit_card".into());

    let event = sub.try_recv().expect("event");
    assert_eq!(event.event_type(), "refund_issued");
}

#[test]
fn emitter_emit_all_batch() {
    let bus = Arc::new(EventBus::new(32));
    let emitter = EventEmitter::new(bus.clone());
    let mut sub = bus.subscribe();

    let events = vec![
        CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: "a@test.com".into(),
            timestamp: Utc::now(),
        },
        CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: "b@test.com".into(),
            timestamp: Utc::now(),
        },
        CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: dec!(10.00),
            item_count: 1,
            timestamp: Utc::now(),
        },
    ];
    emitter.emit_all(events);

    assert_eq!(emitter.total_events(), 3);
    assert!(sub.try_recv().is_some());
    assert!(sub.try_recv().is_some());
    assert!(sub.try_recv().is_some());
    assert!(sub.try_recv().is_none());
}

#[test]
fn emitter_no_subscribers_tracks_failures() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(Arc::clone(&bus));
    // No subscribers

    emitter.emit(CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "no-sub@test.com".into(),
        timestamp: Utc::now(),
    });

    assert_eq!(emitter.total_events(), 1);
    assert_eq!(emitter.total_publish_failures(), 1);
}

#[test]
fn emitter_debug_format() {
    let bus = Arc::new(EventBus::new(16));
    let emitter = EventEmitter::new(bus);
    let debug = format!("{emitter:?}");
    assert!(debug.contains("EventEmitter"));
}

// ---------------------------------------------------------------------------
// InMemoryEventStore: advanced scenarios
// ---------------------------------------------------------------------------

#[test]
fn store_sequence_increments() {
    let store = InMemoryEventStore::new(100);
    assert_eq!(store.latest_sequence().unwrap(), 0);

    for i in 1..=5 {
        let seq = store
            .append(&CommerceEvent::CustomerCreated {
                customer_id: CustomerId::new(),
                email: format!("seq{i}@test.com"),
                timestamp: Utc::now(),
            })
            .unwrap();
        assert_eq!(seq, i);
    }

    assert_eq!(store.latest_sequence().unwrap(), 5);
}

#[test]
fn store_get_events_since_filters_correctly() {
    let store = InMemoryEventStore::new(100);

    for i in 0..10 {
        store
            .append(&CommerceEvent::CustomerCreated {
                customer_id: CustomerId::new(),
                email: format!("since{i}@test.com"),
                timestamp: Utc::now(),
            })
            .unwrap();
    }

    // Get events after sequence 5
    let events = store.get_events_since(5, 100).unwrap();
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].0, 6); // first event has sequence 6
}

#[test]
fn store_get_events_since_respects_limit() {
    let store = InMemoryEventStore::new(100);

    for _ in 0..10 {
        store
            .append(&CommerceEvent::CustomerCreated {
                customer_id: CustomerId::new(),
                email: "limit@test.com".into(),
                timestamp: Utc::now(),
            })
            .unwrap();
    }

    let events = store.get_events_since(0, 3).unwrap();
    assert_eq!(events.len(), 3);
}

#[test]
fn store_aggregate_query_order() {
    let store = InMemoryEventStore::new(100);
    let order_id = OrderId::new();

    // Create order event
    store
        .append(&CommerceEvent::OrderCreated {
            order_id,
            customer_id: CustomerId::new(),
            total_amount: dec!(100.00),
            item_count: 2,
            timestamp: Utc::now(),
        })
        .unwrap();

    // Status change event for same order
    store
        .append(&CommerceEvent::OrderStatusChanged {
            order_id,
            from_status: OrderStatus::Pending,
            to_status: OrderStatus::Confirmed,
            timestamp: Utc::now(),
        })
        .unwrap();

    // Different order
    store
        .append(&CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: dec!(50.00),
            item_count: 1,
            timestamp: Utc::now(),
        })
        .unwrap();

    let events = store.get_events_for_aggregate("order", &order_id.to_string()).unwrap();
    assert_eq!(events.len(), 2);
}

#[test]
fn store_aggregate_query_customer() {
    let store = InMemoryEventStore::new(100);
    let customer_id = CustomerId::new();

    store
        .append(&CommerceEvent::CustomerCreated {
            customer_id,
            email: "agg@test.com".into(),
            timestamp: Utc::now(),
        })
        .unwrap();

    store
        .append(&CommerceEvent::CustomerUpdated {
            customer_id,
            fields_changed: vec!["email".into()],
            timestamp: Utc::now(),
        })
        .unwrap();

    let events = store.get_events_for_aggregate("customer", &customer_id.to_string()).unwrap();
    assert_eq!(events.len(), 2);
}

#[test]
fn store_aggregate_query_empty_result() {
    let store = InMemoryEventStore::new(100);
    let events = store.get_events_for_aggregate("order", "nonexistent-id").unwrap();
    assert!(events.is_empty());
}

#[test]
fn store_eviction_keeps_newest() {
    let store = InMemoryEventStore::new(3);

    for i in 0..5 {
        store
            .append(&CommerceEvent::CustomerCreated {
                customer_id: CustomerId::new(),
                email: format!("evict{i}@test.com"),
                timestamp: Utc::now(),
            })
            .unwrap();
    }

    let events = store.get_events_since(0, 100).unwrap();
    assert_eq!(events.len(), 3);
    // Should have sequences 3, 4, 5 (oldest evicted)
    assert_eq!(events[0].0, 3);
    assert_eq!(events[2].0, 5);
}

#[test]
fn store_debug_format() {
    let store = InMemoryEventStore::new(100);
    let debug = format!("{store:?}");
    assert!(debug.contains("InMemoryEventStore"));
}

// ---------------------------------------------------------------------------
// EventSystem: integration
// ---------------------------------------------------------------------------

#[test]
fn event_system_default_has_webhooks_and_store() {
    let system = EventSystem::new();
    assert!(system.event_store().is_some());
    assert_eq!(system.subscriber_count(), 0);
}

#[test]
fn event_system_emit_persists_to_store() {
    let system = EventSystem::new();

    system.emit(CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "persist@test.com".into(),
        timestamp: Utc::now(),
    });

    let store = system.event_store().expect("store enabled");
    assert_eq!(store.latest_sequence().unwrap(), 1);
}

#[test]
fn event_system_emit_broadcasts_to_subscriber() {
    let system = EventSystem::new();
    let mut sub = system.subscribe();

    system.emit(CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(42.00),
        item_count: 1,
        timestamp: Utc::now(),
    });

    let event = sub.try_recv().expect("received event");
    assert_eq!(event.event_type(), "order_created");
}

#[test]
fn event_system_emitter_accessor() {
    let system = EventSystem::new();
    let _sub = system.subscribe();

    system.emitter().emit(CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "emitter-acc@test.com".into(),
        timestamp: Utc::now(),
    });

    // emitter tracks stats on bus
    assert_eq!(system.emitter().total_events(), 1);
}

#[test]
fn event_system_subscriber_count() {
    let system = EventSystem::new();
    assert_eq!(system.subscriber_count(), 0);

    let _s1 = system.subscribe();
    assert_eq!(system.subscriber_count(), 1);

    let _s2 = system.subscribe();
    assert_eq!(system.subscriber_count(), 2);

    drop(_s1);
    assert_eq!(system.subscriber_count(), 1);
}

#[test]
fn event_system_bus_publish_failures_tracks() {
    let system = EventSystem::with_config(EventConfig {
        enable_webhooks: false,
        persist_events: false,
        ..Default::default()
    });

    // No subscribers => publish failure tracked
    system.emit(CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "fail@test.com".into(),
        timestamp: Utc::now(),
    });

    assert_eq!(system.bus_publish_failures(), 1);
}

#[test]
fn event_system_no_persistence_when_disabled() {
    let system = EventSystem::with_config(EventConfig {
        persist_events: false,
        enable_webhooks: false,
        ..Default::default()
    });

    assert!(system.event_store().is_none());
}

#[test]
fn event_system_config_accessor() {
    let system = EventSystem::with_config(EventConfig {
        channel_capacity: 256,
        webhook_max_retries: 5,
        ..Default::default()
    });
    assert_eq!(system.config().channel_capacity, 256);
    assert_eq!(system.config().webhook_max_retries, 5);
}

#[test]
fn event_system_bus_accessor() {
    let system = EventSystem::new();
    let _sub = system.bus().subscribe();
    assert_eq!(system.subscriber_count(), 1);
}

// ---------------------------------------------------------------------------
// EventSystem: webhook registration
// ---------------------------------------------------------------------------

#[test]
fn event_system_register_webhook_public_url() {
    let system = EventSystem::new();
    let wh = Webhook::new("test", "https://hooks.example.com/callback");
    let id = system.register_webhook(wh);
    assert!(!id.is_nil());

    let hooks = system.list_webhooks();
    assert_eq!(hooks.len(), 1);
}

#[test]
fn event_system_register_webhook_rejects_localhost() {
    let system = EventSystem::new();
    let wh = Webhook::new("bad", "http://localhost:8080/hook");
    let result = system.register_webhook_strict(wh);
    assert_eq!(result, Err(WebhookRegistrationError::UnsafeUrl));
}

#[test]
fn event_system_register_webhook_rejects_private_ip() {
    let system = EventSystem::new();
    let wh = Webhook::new("bad", "http://192.168.1.1/hook");
    let result = system.register_webhook_strict(wh);
    assert_eq!(result, Err(WebhookRegistrationError::UnsafeUrl));
}

#[test]
fn event_system_try_register_returns_none_for_bad_url() {
    let system = EventSystem::new();
    let wh = Webhook::new("bad", "ftp://files.example.com/hook");
    assert!(system.try_register_webhook(wh).is_none());
}

#[test]
fn event_system_unregister_webhook() {
    let system = EventSystem::new();
    let wh = Webhook::new("test", "https://hooks.example.com/callback");
    let id = system.register_webhook(wh);

    assert!(system.unregister_webhook(id));
    assert!(system.list_webhooks().is_empty());
}

#[test]
fn event_system_unregister_nonexistent_returns_false() {
    let system = EventSystem::new();
    assert!(!system.unregister_webhook(uuid::Uuid::new_v4()));
}

#[test]
fn event_system_webhooks_disabled_rejects_registration() {
    let system =
        EventSystem::with_config(EventConfig { enable_webhooks: false, ..Default::default() });
    let wh = Webhook::new("test", "https://hooks.example.com/callback");
    let result = system.register_webhook_strict(wh);
    assert_eq!(result, Err(WebhookRegistrationError::WebhooksDisabled));
}

#[test]
fn event_system_webhook_deliveries_empty_for_unknown() {
    let system = EventSystem::new();
    let deliveries = system.webhook_deliveries(uuid::Uuid::new_v4());
    assert!(deliveries.is_empty());
}

// ---------------------------------------------------------------------------
// EventSystem: filtered subscriptions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn filtered_subscription_orders_only() {
    let system = EventSystem::new();
    let mut filtered = system.subscribe_filtered(stateset_embedded::events::filters::orders_only);

    // Emit a customer event (should be filtered)
    system.emit(CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "filter@test.com".into(),
        timestamp: Utc::now(),
    });

    // Emit an order event (should pass filter)
    system.emit(CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(50.00),
        item_count: 1,
        timestamp: Utc::now(),
    });

    let got = tokio::time::timeout(std::time::Duration::from_millis(200), filtered.recv())
        .await
        .expect("timeout")
        .expect("event");

    assert_eq!(got.event_type(), "order_created");
}

// ---------------------------------------------------------------------------
// Event filter functions
// ---------------------------------------------------------------------------

#[test]
fn filter_orders_only() {
    use stateset_embedded::events::filters;

    let order = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    let customer = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "f@t.com".into(),
        timestamp: Utc::now(),
    };
    assert!(filters::orders_only(&order));
    assert!(!filters::orders_only(&customer));
}

#[test]
fn filter_customers_only() {
    use stateset_embedded::events::filters;

    let customer = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "f@t.com".into(),
        timestamp: Utc::now(),
    };
    let order = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(filters::customers_only(&customer));
    assert!(!filters::customers_only(&order));
}

#[test]
fn filter_returns_only() {
    use stateset_embedded::events::filters;

    let ret = CommerceEvent::ReturnRequested {
        return_id: ReturnId::new(),
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        reason: ReturnReason::Defective,
        item_count: 1,
        timestamp: Utc::now(),
    };
    let order = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(filters::returns_only(&ret));
    assert!(!filters::returns_only(&order));
}

#[test]
fn filter_products_only() {
    use stateset_embedded::events::filters;

    let product = CommerceEvent::ProductCreated {
        product_id: ProductId::new(),
        name: "Widget".into(),
        slug: "widget".into(),
        timestamp: Utc::now(),
    };
    assert!(filters::products_only(&product));

    let customer = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "f@t.com".into(),
        timestamp: Utc::now(),
    };
    assert!(!filters::products_only(&customer));
}

#[test]
fn filter_inventory_only() {
    use stateset_embedded::events::filters;

    let inv = CommerceEvent::InventoryAdjusted {
        item_id: 1,
        sku: "SKU".into(),
        location_id: 1,
        quantity_change: dec!(-5),
        new_quantity: dec!(95),
        reason: "sale".into(),
        timestamp: Utc::now(),
    };
    assert!(filters::inventory_only(&inv));

    let order = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(!filters::inventory_only(&order));
}

#[test]
fn filter_low_stock_alerts() {
    use stateset_embedded::events::filters;

    let alert = CommerceEvent::LowStockAlert {
        sku: "LOW".into(),
        location_id: 1,
        current_quantity: dec!(2),
        reorder_point: dec!(10),
        timestamp: Utc::now(),
    };
    assert!(filters::low_stock_alerts(&alert));

    let inv = CommerceEvent::InventoryAdjusted {
        item_id: 1,
        sku: "SKU".into(),
        location_id: 1,
        quantity_change: dec!(-5),
        new_quantity: dec!(95),
        reason: "sale".into(),
        timestamp: Utc::now(),
    };
    assert!(!filters::low_stock_alerts(&inv));
}

#[test]
fn filter_event_types() {
    use stateset_embedded::events::filters;

    let filter = filters::event_types(&["order_created", "customer_created"]);

    let order = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(filter(&order));

    let product = CommerceEvent::ProductCreated {
        product_id: ProductId::new(),
        name: "Widget".into(),
        slug: "widget".into(),
        timestamp: Utc::now(),
    };
    assert!(!filter(&product));
}

// ---------------------------------------------------------------------------
// Webhook struct builder methods
// ---------------------------------------------------------------------------

#[test]
fn webhook_builder_with_secret() {
    let wh = Webhook::new("test", "https://hooks.example.com").with_secret("my-secret");
    assert_eq!(wh.secret.as_deref(), Some("my-secret"));
}

#[test]
fn webhook_builder_with_events() {
    let wh = Webhook::new("test", "https://hooks.example.com")
        .with_events(vec!["order_created".into(), "customer_created".into()]);
    assert_eq!(wh.event_types.len(), 2);
}

#[test]
fn webhook_builder_with_header() {
    let wh = Webhook::new("test", "https://hooks.example.com")
        .with_header("X-Custom", "value1")
        .with_header("X-Another", "value2");
    assert_eq!(wh.headers.len(), 2);
    assert_eq!(wh.headers.get("X-Custom").unwrap(), "value1");
}

#[test]
fn webhook_should_receive_all_when_no_filter() {
    let wh = Webhook::new("test", "https://hooks.example.com");
    let event = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(wh.should_receive(&event));
}

#[test]
fn webhook_should_receive_respects_event_filter() {
    let wh =
        Webhook::new("test", "https://hooks.example.com").with_events(vec!["order_created".into()]);

    let order = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(wh.should_receive(&order));

    let customer = CommerceEvent::CustomerCreated {
        customer_id: CustomerId::new(),
        email: "f@t.com".into(),
        timestamp: Utc::now(),
    };
    assert!(!wh.should_receive(&customer));
}

#[test]
fn webhook_inactive_receives_nothing() {
    let mut wh = Webhook::new("test", "https://hooks.example.com");
    wh.active = false;

    let event = CommerceEvent::OrderCreated {
        order_id: OrderId::new(),
        customer_id: CustomerId::new(),
        total_amount: dec!(10.00),
        item_count: 1,
        timestamp: Utc::now(),
    };
    assert!(!wh.should_receive(&event));
}

// ---------------------------------------------------------------------------
// WebhookRegistrationError display
// ---------------------------------------------------------------------------

#[test]
fn webhook_registration_error_display() {
    assert!(WebhookRegistrationError::UnsafeUrl.to_string().contains("URL validation"),);
    assert!(WebhookRegistrationError::DuplicateId.to_string().contains("duplicate"),);
    assert!(WebhookRegistrationError::WebhooksDisabled.to_string().contains("disabled"),);
}

// ---------------------------------------------------------------------------
// Multi-tenant isolation: separate Commerce instances
// ---------------------------------------------------------------------------

#[test]
fn multi_tenant_isolation() {
    use stateset_core::{CreateCustomer, CustomerFilter};
    use stateset_embedded::Commerce;

    let tenant_a = Commerce::new(":memory:").expect("tenant a");
    let tenant_b = Commerce::new(":memory:").expect("tenant b");

    tenant_a
        .customers()
        .create(CreateCustomer {
            email: "alice@tenant-a.com".into(),
            first_name: "Alice".into(),
            last_name: "A".into(),
            ..Default::default()
        })
        .expect("create customer");

    tenant_b
        .customers()
        .create(CreateCustomer {
            email: "bob@tenant-b.com".into(),
            first_name: "Bob".into(),
            last_name: "B".into(),
            ..Default::default()
        })
        .expect("create customer");

    let a_customers = tenant_a.customers().list(CustomerFilter::default()).expect("list a");
    let b_customers = tenant_b.customers().list(CustomerFilter::default()).expect("list b");

    assert_eq!(a_customers.len(), 1);
    assert_eq!(b_customers.len(), 1);
    assert_eq!(a_customers[0].email, "alice@tenant-a.com");
    assert_eq!(b_customers[0].email, "bob@tenant-b.com");
}

// ---------------------------------------------------------------------------
// Notification service: event pipeline integration
// ---------------------------------------------------------------------------

#[test]
fn notification_pipeline_multiple_events() {
    let backend = LogEmailBackend::new();
    let backend_ref = backend.clone();
    let config = NotificationConfig::default();
    let service = NotificationService::new(config, Box::new(backend))
        .with_recipient_resolver(Arc::new(|_| Some("all@test.com".into())));

    // OrderCreated => email
    service
        .process_event(&CommerceEvent::OrderCreated {
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            total_amount: dec!(100.00),
            item_count: 2,
            timestamp: Utc::now(),
        })
        .unwrap();

    // LowStockAlert => email
    service
        .process_event(&CommerceEvent::LowStockAlert {
            sku: "LOW-001".into(),
            location_id: 1,
            current_quantity: dec!(3),
            reorder_point: dec!(10),
            timestamp: Utc::now(),
        })
        .unwrap();

    // ProductCreated => no email (not mapped)
    service
        .process_event(&CommerceEvent::ProductCreated {
            product_id: ProductId::new(),
            name: "Widget".into(),
            slug: "widget".into(),
            timestamp: Utc::now(),
        })
        .unwrap();

    let sent = backend_ref.emails();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].template, EmailTemplate::OrderConfirmation);
    assert_eq!(sent[1].template, EmailTemplate::LowStockAlert);
}

// ---------------------------------------------------------------------------
// EventConfig default values
// ---------------------------------------------------------------------------

#[test]
fn event_config_default_values() {
    let config = EventConfig::default();
    assert_eq!(config.channel_capacity, 1024);
    assert!(config.persist_events);
    assert!(config.event_store.is_none());
    assert_eq!(config.max_in_memory_events, 10_000);
    assert!(config.enable_webhooks);
    assert_eq!(config.webhook_max_retries, 3);
    assert_eq!(config.webhook_timeout_secs, 30);
    assert_eq!(config.webhook_max_in_flight, 8);
    assert_eq!(config.webhook_retry_delay_ms, 1000);
    assert_eq!(config.webhook_max_delivery_history, 1_000);
    assert!(config.webhook_outbound_allowlist.is_empty());
}

#[test]
fn event_config_debug() {
    let config = EventConfig::default();
    let debug = format!("{config:?}");
    assert!(debug.contains("channel_capacity"));
    assert!(debug.contains("webhook_max_retries"));
}

// ---------------------------------------------------------------------------
// EventSystem debug
// ---------------------------------------------------------------------------

#[test]
fn event_system_debug() {
    let system = EventSystem::new();
    let debug = format!("{system:?}");
    assert!(debug.contains("EventSystem"));
}
