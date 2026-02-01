//! Events and webhooks example for stateset-icommerce
//!
//! This example demonstrates:
//! - Subscribing to commerce events
//! - Registering webhooks for event delivery
//! - Event-driven workflows
//!
//! Run with: cargo run --example events_webhooks --features events

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CommerceEvent, EventConfig, Webhook,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn main() -> Result<(), CommerceError> {
    println!("=== StateSet Events & Webhooks Example ===\n");

    // Initialize commerce with custom event configuration
    let commerce = Commerce::builder()
        .in_memory()
        .event_config(EventConfig {
            channel_capacity: 512,
            persist_events: false, // Don't persist for this example
            enable_webhooks: true,
            webhook_max_retries: 3,
            webhook_timeout_secs: 10,
            ..Default::default()
        })
        .build()?;

    println!("Commerce initialized with events enabled\n");

    // ========================================================================
    // 1. Subscribe to Events
    // ========================================================================
    println!("1. Setting up event subscription...");

    // Track event counts
    let customer_events = Arc::new(AtomicU32::new(0));
    let order_events = Arc::new(AtomicU32::new(0));
    let inventory_events = Arc::new(AtomicU32::new(0));

    // Subscribe to all events
    let mut subscription = commerce.subscribe_events();

    // Clone counters for the processing thread
    let customer_events_clone = customer_events.clone();
    let order_events_clone = order_events.clone();
    let inventory_events_clone = inventory_events.clone();

    // Process events in a separate thread
    std::thread::spawn(move || {
        while let Some(event) = subscription.try_recv() {
            match event {
                CommerceEvent::CustomerCreated { email, .. } => {
                    println!("   [EVENT] Customer created: {}", email);
                    customer_events_clone.fetch_add(1, Ordering::SeqCst);
                }
                CommerceEvent::OrderCreated { order_id, total, .. } => {
                    println!("   [EVENT] Order created: {} (total: ${})", order_id, total);
                    order_events_clone.fetch_add(1, Ordering::SeqCst);
                }
                CommerceEvent::InventoryAdjusted { sku, quantity_change, .. } => {
                    println!("   [EVENT] Inventory adjusted: {} ({:+})", sku, quantity_change);
                    inventory_events_clone.fetch_add(1, Ordering::SeqCst);
                }
                CommerceEvent::OrderStatusChanged { order_id, new_status, .. } => {
                    println!("   [EVENT] Order {} status changed to: {:?}", order_id, new_status);
                    order_events_clone.fetch_add(1, Ordering::SeqCst);
                }
                _ => {
                    println!("   [EVENT] Other event received");
                }
            }
        }
    });

    println!("   Event subscription active\n");

    // ========================================================================
    // 2. Register Webhooks
    // ========================================================================
    println!("2. Registering webhooks...");

    // Register a webhook for order events
    let order_webhook = Webhook::new(
        "Order Notifications",
        "https://example.com/webhooks/orders",
    )
    .with_secret("my-secret-key")
    .for_events(vec![
        "order.created".to_string(),
        "order.status_changed".to_string(),
        "order.shipped".to_string(),
    ]);

    if let Some(webhook_id) = commerce.register_webhook(order_webhook) {
        println!("   Registered order webhook: {}", webhook_id);
    }

    // Register a webhook for inventory events
    let inventory_webhook = Webhook::new(
        "Inventory Alerts",
        "https://example.com/webhooks/inventory",
    )
    .with_secret("inventory-secret")
    .for_events(vec![
        "inventory.low_stock".to_string(),
        "inventory.adjusted".to_string(),
    ]);

    if let Some(webhook_id) = commerce.register_webhook(inventory_webhook) {
        println!("   Registered inventory webhook: {}", webhook_id);
    }

    // List all webhooks
    let webhooks = commerce.list_webhooks();
    println!("   Total webhooks registered: {}\n", webhooks.len());

    // ========================================================================
    // 3. Generate Events through Normal Operations
    // ========================================================================
    println!("3. Performing operations (will generate events)...\n");

    // Create a customer (generates CustomerCreated event)
    let customer = commerce.customers().create(CreateCustomer {
        email: "events-demo@example.com".into(),
        first_name: "Demo".into(),
        last_name: "User".into(),
        ..Default::default()
    })?;
    println!("   Created customer: {}", customer.email);

    // Create inventory (generates InventoryCreated event)
    commerce.inventory().create_item(CreateInventoryItem {
        sku: "EVENT-DEMO-001".into(),
        name: "Event Demo Product".into(),
        initial_quantity: Some(dec!(50)),
        reorder_point: Some(dec!(10)),
        ..Default::default()
    })?;
    println!("   Created inventory item: EVENT-DEMO-001");

    // Create an order (generates OrderCreated event)
    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            sku: "EVENT-DEMO-001".into(),
            name: "Event Demo Product".into(),
            quantity: 3,
            unit_price: dec!(19.99),
            ..Default::default()
        }],
        ..Default::default()
    })?;
    println!("   Created order: {}", order.order_number);

    // Adjust inventory (generates InventoryAdjusted event)
    commerce.inventory().adjust("EVENT-DEMO-001", dec!(-3), "Order fulfillment")?;
    println!("   Adjusted inventory: -3 units");

    // Ship order (generates OrderStatusChanged event)
    let order = commerce.orders().ship(order.id, Some("TRACK-EVENTS-123"))?;
    println!("   Shipped order: tracking {}", order.tracking_number.unwrap_or_default());

    // Give events time to propagate
    std::thread::sleep(std::time::Duration::from_millis(100));

    // ========================================================================
    // 4. Emit Custom Events
    // ========================================================================
    println!("\n4. Emitting custom events...");

    // You can also emit events manually
    commerce.emit_event(CommerceEvent::InventoryAdjusted {
        item_id: 999,
        sku: "MANUAL-EVENT".to_string(),
        quantity_change: dec!(-100),
        new_quantity: dec!(0),
        reason: "Manual event for demo".to_string(),
        timestamp: chrono::Utc::now(),
    });
    println!("   Emitted manual inventory event\n");

    // ========================================================================
    // 5. Summary
    // ========================================================================
    std::thread::sleep(std::time::Duration::from_millis(100));

    println!("=== Event Summary ===");
    println!("Customer events received: {}", customer_events.load(Ordering::SeqCst));
    println!("Order events received: {}", order_events.load(Ordering::SeqCst));
    println!("Inventory events received: {}", inventory_events.load(Ordering::SeqCst));
    println!("Webhooks registered: {}", commerce.list_webhooks().len());

    println!("\nExample completed successfully!");

    Ok(())
}
