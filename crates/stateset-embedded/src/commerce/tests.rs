use super::*;

#[cfg(feature = "sqlite")]
fn commerce_with_temp_db() -> (Commerce, tempfile::NamedTempFile) {
    let db_file = tempfile::NamedTempFile::new().expect("create temp sqlite db");
    let db_path = db_file.path().to_str().expect("temp db path utf-8");
    let commerce =
        Commerce::builder().sqlite(db_path).max_connections(1).build().expect("create commerce");

    (commerce, db_file)
}

#[test]
#[cfg(feature = "sqlite")]
fn test_create_commerce() {
    let commerce = Commerce::new(":memory:").unwrap();
    assert!(commerce.orders().list(Default::default()).unwrap().is_empty());
}

#[test]
#[cfg(feature = "sqlite")]
fn test_builder() {
    let commerce = Commerce::builder().database(":memory:").max_connections(1).build().unwrap();

    assert!(commerce.customers().list(Default::default()).unwrap().is_empty());
}

#[test]
#[cfg(feature = "sqlite")]
fn test_builder_can_disable_metrics() {
    let commerce = Commerce::builder().database(":memory:").disable_metrics().build().unwrap();

    assert!(!commerce.metrics().is_enabled());
}

#[test]
#[cfg(feature = "sqlite")]
fn test_metrics_record_key_engine_operations() {
    use rust_decimal_macros::dec;
    use stateset_core::{
        AddCartItem, BillingInterval, CreateCart, CreateCustomer, CreateInventoryItem, CreateOrder,
        CreateOrderItem, CreatePayment, CreateProduct, CreateProductVariant, CreateReturn,
        CreateReturnItem, CreateShipment, CreateShipmentItem, CreateSubscription,
        CreateSubscriptionPlan, PaymentMethodType, ReturnReason, ShippingCarrier,
    };

    let commerce = Commerce::new(":memory:").unwrap();

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "metrics@example.com".into(),
            first_name: "Metric".into(),
            last_name: "Tester".into(),
            ..Default::default()
        })
        .unwrap();

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id.into(),
            items: vec![CreateOrderItem {
                product_id: stateset_core::ProductId::new(),
                sku: "SKU-METRIC".into(),
                name: "Metric Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();

    commerce
        .products()
        .create(CreateProduct {
            name: "Metric Widget".into(),
            variants: Some(vec![CreateProductVariant {
                sku: "SKU-METRIC".into(),
                price: dec!(29.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();

    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "SKU-METRIC".into(),
            name: "Metric Widget".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .unwrap();

    commerce.inventory().adjust("SKU-METRIC", dec!(-2), "order allocation").unwrap();

    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(59.98),
            ..Default::default()
        })
        .unwrap();
    commerce.payments().mark_completed(payment.id).unwrap();

    commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer.id.into()),
            customer_email: Some(customer.email.clone()),
            items: Some(vec![AddCartItem {
                sku: "SKU-METRIC-CART".into(),
                name: "Cart Metric Item".into(),
                quantity: 1,
                unit_price: dec!(9.99),
                requires_shipping: Some(false),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    commerce.carts().complete(cart.id).unwrap();

    let shipment = commerce
        .shipments()
        .create(CreateShipment {
            order_id: order.id,
            carrier: Some(ShippingCarrier::Ups),
            recipient_name: "Metric Tester".into(),
            shipping_address: "123 Metric Way, Testville, ST 12345".into(),
            items: Some(vec![CreateShipmentItem {
                sku: "SKU-METRIC".into(),
                name: "Metric Widget".into(),
                quantity: 1,
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    commerce.shipments().mark_delivered(shipment.id).unwrap();

    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: "Metrics Monthly Plan".into(),
            billing_interval: BillingInterval::Monthly,
            price: dec!(19.99),
            ..Default::default()
        })
        .unwrap();
    commerce.subscriptions().activate_plan(plan.id).unwrap();
    commerce
        .subscriptions()
        .subscribe(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            ..Default::default()
        })
        .unwrap();

    let snapshot = commerce.metrics_snapshot();
    assert_eq!(snapshot.orders_created, 1);
    assert_eq!(snapshot.customers_created, 1);
    assert_eq!(snapshot.products_created, 1);
    assert_eq!(snapshot.carts_created, 1);
    assert_eq!(snapshot.cart_checkouts_completed, 1);
    assert_eq!(snapshot.returns_requested, 1);
    assert_eq!(snapshot.shipments_created, 1);
    assert_eq!(snapshot.shipments_delivered, 1);
    assert_eq!(snapshot.subscriptions_created, 1);
    assert_eq!(snapshot.payments_completed, 1);
    assert_eq!(snapshot.inventory_adjustments, 1);
    assert!((snapshot.order_amount_total - 59.98).abs() < 1e-9);
    assert!((snapshot.payment_amount_total - 59.98).abs() < 1e-9);
    assert!((snapshot.inventory_delta_total - -2.0).abs() < 1e-9);
}

#[test]
#[cfg(feature = "sqlite")]
fn test_health_check_reports_engine_state() {
    let commerce = Commerce::new(":memory:").unwrap();
    let health = commerce.health_check();

    assert!(health.healthy);
    assert_eq!(health.backend, CommerceBackend::Sqlite);
    assert!(health.database_reachable);
    assert!(health.database_error.is_none());
    #[cfg(feature = "events")]
    assert_eq!(health.event_subscribers, 0);
}

#[test]
#[cfg(feature = "sqlite")]
fn test_bom_operations() {
    use rust_decimal_macros::dec;
    use stateset_core::{BomStatus, CreateBom, CreateBomComponent};

    let commerce = Commerce::new(":memory:").unwrap();
    let product_id = stateset_core::ProductId::new();

    // Create a BOM
    let bom = commerce
        .bom()
        .create(CreateBom {
            product_id,
            name: "Test BOM".into(),
            description: Some("Test description".into()),
            components: Some(vec![CreateBomComponent {
                name: "Component A".into(),
                component_sku: Some("COMP-A".into()),
                quantity: dec!(2),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(bom.name, "Test BOM");
    assert_eq!(bom.status, BomStatus::Draft);
    assert!(bom.bom_number.starts_with("BOM-"));

    // Get components
    let components = commerce.bom().get_components(bom.id).unwrap();
    assert_eq!(components.len(), 1);
    assert_eq!(components[0].name, "Component A");

    // Activate
    let bom = commerce.bom().activate(bom.id).unwrap();
    assert_eq!(bom.status, BomStatus::Active);
}

#[test]
#[cfg(feature = "sqlite")]
fn test_work_order_operations() {
    use rust_decimal_macros::dec;
    use stateset_core::{CreateWorkOrder, WorkOrderStatus};

    let commerce = Commerce::new(":memory:").unwrap();
    let product_id = stateset_core::ProductId::new();

    // Create work order
    let wo = commerce
        .work_orders()
        .create(CreateWorkOrder {
            product_id,
            quantity_to_build: dec!(100),
            notes: Some("Test work order".into()),
            ..Default::default()
        })
        .unwrap();

    assert!(wo.work_order_number.starts_with("WO-"));
    assert_eq!(wo.status, WorkOrderStatus::Planned);
    assert_eq!(wo.quantity_to_build, dec!(100));

    // Start work order
    let wo = commerce.work_orders().start(wo.id).unwrap();
    assert_eq!(wo.status, WorkOrderStatus::InProgress);

    // Complete work order
    let wo = commerce.work_orders().complete(wo.id, dec!(100)).unwrap();
    assert_eq!(wo.status, WorkOrderStatus::Completed);
    assert_eq!(wo.quantity_completed, dec!(100));
}

#[test]
#[cfg(feature = "sqlite")]
fn test_shipment_operations() {
    use stateset_core::{CreateShipment, CreateShipmentItem, ShipmentStatus, ShippingCarrier};

    let commerce = Commerce::new(":memory:").unwrap();
    let order_id: crate::OrderId = uuid::Uuid::new_v4().into();

    // Create shipment
    let shipment = commerce
        .shipments()
        .create(CreateShipment {
            order_id,
            carrier: Some(ShippingCarrier::Ups),
            recipient_name: "Alice Smith".into(),
            shipping_address: "123 Main St, City, ST 12345".into(),
            items: Some(vec![CreateShipmentItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();

    assert!(shipment.shipment_number.starts_with("SHP-"));
    assert_eq!(shipment.status, ShipmentStatus::Pending);
    assert_eq!(shipment.carrier, ShippingCarrier::Ups);

    // Get items
    let items = commerce.shipments().get_items(shipment.id).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].sku, "SKU-001");

    // Mark as processing
    let shipment = commerce.shipments().mark_processing(shipment.id).unwrap();
    assert_eq!(shipment.status, ShipmentStatus::Processing);

    // Ship with tracking number
    let shipment =
        commerce.shipments().ship(shipment.id, Some("1Z999AA10123456784".into())).unwrap();
    assert_eq!(shipment.status, ShipmentStatus::Shipped);
    assert_eq!(shipment.tracking_number, Some("1Z999AA10123456784".to_string()));
    assert!(shipment.tracking_url.is_some());

    // Mark in transit
    let shipment = commerce.shipments().mark_in_transit(shipment.id).unwrap();
    assert_eq!(shipment.status, ShipmentStatus::InTransit);

    // Mark delivered
    let shipment = commerce.shipments().mark_delivered(shipment.id).unwrap();
    assert_eq!(shipment.status, ShipmentStatus::Delivered);
    assert!(shipment.delivered_at.is_some());
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_event_system_basic() {
    let commerce = Commerce::new(":memory:").unwrap();

    // Verify event system is accessible
    let event_system = commerce.events();
    assert_eq!(event_system.subscriber_count(), 0);

    // Subscribe to events
    let _sub = commerce.subscribe_events();
    assert_eq!(commerce.events().subscriber_count(), 1);
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_event_system_builder() {
    use crate::events::EventConfig;

    let commerce = Commerce::builder()
        .database(":memory:")
        .event_config(EventConfig {
            channel_capacity: 512,
            webhook_max_retries: 5,
            webhook_timeout_secs: 12,
            webhook_max_in_flight: 4,
            webhook_retry_delay_ms: 750,
            webhook_max_delivery_history: 42,
            enable_webhooks: false,
            ..Default::default()
        })
        .build()
        .unwrap();

    // Verify custom config is applied
    assert_eq!(commerce.events().config().channel_capacity, 512);
    assert!(!commerce.events().config().enable_webhooks);
    assert_eq!(commerce.events().config().webhook_max_retries, 5);
    assert_eq!(commerce.events().config().webhook_timeout_secs, 12);
    assert_eq!(commerce.events().config().webhook_max_in_flight, 4);
    assert_eq!(commerce.events().config().webhook_retry_delay_ms, 750);
    assert_eq!(commerce.events().config().webhook_max_delivery_history, 42);
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_event_system_builder_normalizes_config() {
    use crate::events::EventConfig;

    let commerce = Commerce::builder()
        .database(":memory:")
        .event_config(EventConfig {
            channel_capacity: 0,
            max_in_memory_events: 0,
            webhook_max_in_flight: 0,
            webhook_timeout_secs: 0,
            webhook_retry_delay_ms: 0,
            persist_events: false,
            enable_webhooks: false,
            ..Default::default()
        })
        .build()
        .unwrap();

    let config = commerce.events().config();
    assert_eq!(config.channel_capacity, 1);
    assert_eq!(config.max_in_memory_events, 1);
    assert_eq!(config.webhook_max_in_flight, 1);
    assert_eq!(config.webhook_timeout_secs, 1);
    assert_eq!(config.webhook_retry_delay_ms, 1);
}

#[tokio::test]
#[cfg(all(feature = "sqlite", feature = "events"))]
async fn test_event_subscription() {
    use chrono::Utc;
    use stateset_core::CommerceEvent;

    let commerce = Commerce::new(":memory:").unwrap();
    let mut subscription = commerce.subscribe_events();

    // Emit a test event
    let event = CommerceEvent::CustomerCreated {
        customer_id: stateset_core::CustomerId::new(),
        email: "test@example.com".to_string(),
        timestamp: Utc::now(),
    };

    commerce.emit_event(event.clone());

    // Receive the event
    let received = subscription.try_recv();
    assert!(received.is_some());

    if let Some(CommerceEvent::CustomerCreated { email, .. }) = received {
        assert_eq!(email, "test@example.com");
    } else {
        panic!("Expected CustomerCreated event");
    }
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_webhook_registration() {
    use crate::events::Webhook;
    use uuid::Uuid;

    let commerce = Commerce::new(":memory:").unwrap();

    let webhook = Webhook::new("Test Webhook", "https://example.com/webhook");

    // Register webhook
    let id = commerce.register_webhook(webhook);
    assert_ne!(id, Uuid::nil());

    // Verify it's registered
    let webhooks = commerce.list_webhooks();
    assert_eq!(webhooks.len(), 1);
    assert_eq!(webhooks[0].id, id);

    // Unregister
    assert!(commerce.unregister_webhook(id));
    assert!(commerce.list_webhooks().is_empty());
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_webhook_registration_disabled() {
    use crate::events::{EventConfig, Webhook, WebhookRegistrationError};
    use uuid::Uuid;

    let commerce = Commerce::builder()
        .database(":memory:")
        .event_config(EventConfig {
            enable_webhooks: false,
            persist_events: false,
            ..Default::default()
        })
        .build()
        .unwrap();

    let webhook = Webhook::new("Disabled Webhook", "https://example.com/webhook");
    assert_eq!(
        commerce.register_webhook_strict(webhook.clone()),
        Err(WebhookRegistrationError::WebhooksDisabled)
    );
    let id = commerce.register_webhook(webhook);
    assert_eq!(id, Uuid::nil());
    assert_eq!(commerce.try_register_webhook(Webhook::new("Disabled Webhook", "https://example.com/webhook")), None);
    assert!(commerce.list_webhooks().is_empty());
    assert!(commerce.webhook_deliveries(id).is_empty());
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_webhook_registration_rejects_unsafe_url() {
    use crate::events::{Webhook, WebhookRegistrationError};
    use uuid::Uuid;

    let commerce = Commerce::new(":memory:").unwrap();

    let webhook = Webhook::new("Unsafe Webhook", "http://localhost:8080/webhook");

    assert_eq!(commerce.try_register_webhook(webhook), None);
    assert_eq!(
        commerce.register_webhook_strict(Webhook::new("Unsafe Webhook", "http://localhost:8080/webhook")),
        Err(WebhookRegistrationError::UnsafeUrl)
    );
    let fallback_id = commerce.register_webhook(Webhook::new("Fallback Webhook", "http://localhost:8080/webhook"));
    assert_eq!(fallback_id, Uuid::nil());
    assert!(commerce.list_webhooks().is_empty());
}

#[test]
#[cfg(all(feature = "sqlite", feature = "events"))]
fn test_webhook_delivery_history_api() {
    use crate::events::Webhook;

    let commerce = Commerce::new(":memory:").unwrap();
    let webhook = Webhook::new("History API", "https://example.com/webhook");

    let id = commerce.register_webhook(webhook);

    assert!(commerce.webhook_deliveries(id).is_empty());
    assert!(commerce.unregister_webhook(id));
    assert!(commerce.webhook_deliveries(id).is_empty());
}

#[test]
#[cfg(feature = "sqlite")]
fn test_promotions_create_and_list() {
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreatePromotion, PromotionStatus, PromotionTarget, PromotionTrigger, PromotionType,
        StackingBehavior,
    };

    let (commerce, _db_file) = commerce_with_temp_db();

    // Create a percentage off promotion
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: None,
            name: "Summer Sale".into(),
            description: Some("Get 20% off your order".into()),
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.20)),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        })
        .unwrap();

    assert_eq!(promo.name, "Summer Sale");
    assert_eq!(promo.promotion_type, PromotionType::PercentageOff);
    assert_eq!(promo.percentage_off, Some(dec!(0.20)));
    assert_eq!(promo.status, PromotionStatus::Draft);

    // Activate the promotion
    let promo = commerce.promotions().activate(promo.id).unwrap();
    assert_eq!(promo.status, PromotionStatus::Active);

    // List active promotions
    let active = commerce.promotions().get_active().unwrap();
    assert!(!active.is_empty());

    // Deactivate
    let promo = commerce.promotions().deactivate(promo.id).unwrap();
    assert_eq!(promo.status, PromotionStatus::Paused);
}

#[test]
#[cfg(feature = "sqlite")]
fn test_promotions_coupon_codes() {
    use rust_decimal_macros::dec;
    use stateset_core::{
        CouponStatus, CreateCouponCode, CreatePromotion, PromotionTarget, PromotionTrigger,
        PromotionType, StackingBehavior,
    };

    let (commerce, _db_file) = commerce_with_temp_db();

    // Create a promotion
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: None,
            name: "VIP Discount".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.15)),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        })
        .unwrap();

    commerce.promotions().activate(promo.id).unwrap();

    // Create a coupon code
    let coupon = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "VIP15".into(),
            usage_limit: Some(100),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .unwrap();

    assert_eq!(coupon.code, "VIP15");
    assert_eq!(coupon.status, CouponStatus::Active);
    assert_eq!(coupon.usage_limit, Some(100));
    assert_eq!(coupon.usage_count, 0);

    // Validate the coupon
    let validated = commerce.promotions().validate_coupon("VIP15").unwrap();
    assert!(validated.is_some());

    // Invalid coupon
    let invalid = commerce.promotions().validate_coupon("INVALID").unwrap();
    assert!(invalid.is_none());
}

#[test]
#[cfg(feature = "sqlite")]
fn test_promotions_fixed_amount() {
    use rust_decimal_macros::dec;
    use stateset_core::{
        ApplyPromotionsRequest, CreatePromotion, PromotionLineItem, PromotionTarget,
        PromotionTrigger, PromotionType, StackingBehavior,
    };

    let (commerce, _db_file) = commerce_with_temp_db();

    // Create $10 off automatic promotion
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: None,
            name: "$10 Off".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: None,
            fixed_amount_off: Some(dec!(10.00)),
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        })
        .unwrap();

    commerce.promotions().activate(promo.id).unwrap();

    // Apply to a $100 order
    let result = commerce
        .promotions()
        .apply(ApplyPromotionsRequest {
            cart_id: None,
            customer_id: None,
            subtotal: dec!(100.00),
            shipping_amount: dec!(5.00),
            shipping_country: None,
            shipping_state: None,
            currency: "USD".into(),
            coupon_codes: vec![],
            line_items: vec![PromotionLineItem {
                id: "item-1".into(),
                product_id: None,
                variant_id: None,
                sku: Some("SKU-001".into()),
                category_ids: vec![],
                quantity: 2,
                unit_price: dec!(50.00),
                line_total: dec!(100.00),
            }],
            is_first_order: false,
        })
        .unwrap();

    assert_eq!(result.total_discount, dec!(10.00));
    assert_eq!(result.discounted_subtotal, dec!(90.00));
    assert!(!result.applied_promotions.is_empty());
}

#[test]
#[cfg(feature = "sqlite")]
fn test_cart_promotions_integration() {
    use rust_decimal_macros::dec;
    use stateset_core::{
        AddCartItem, CreateCart, CreateCouponCode, CreatePromotion, PromotionTarget,
        PromotionTrigger, PromotionType, StackingBehavior,
    };

    let (commerce, _db_file) = commerce_with_temp_db();

    // Create a 25% off promotion with coupon
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: None,
            name: "25% Off Everything".into(),
            description: None,
            internal_notes: None,
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.25)),
            fixed_amount_off: None,
            max_discount_amount: None,
            buy_quantity: None,
            get_quantity: None,
            get_discount_percent: None,
            tiers: None,
            bundle_product_ids: None,
            bundle_discount: None,
            starts_at: None,
            ends_at: None,
            total_usage_limit: None,
            per_customer_limit: None,
            priority: Some(1),
            conditions: None,
            applicable_product_ids: None,
            applicable_category_ids: None,
            applicable_skus: None,
            excluded_product_ids: None,
            excluded_category_ids: None,
            eligible_customer_ids: None,
            eligible_customer_groups: None,
            currency: None,
            metadata: None,
        })
        .unwrap();

    commerce.promotions().activate(promo.id).unwrap();

    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "SAVE25".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .unwrap();

    // Create a cart with items
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("test@example.com".into()),
            ..Default::default()
        })
        .unwrap();

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "ITEM-001".into(),
                name: "Test Product".into(),
                quantity: 2,
                unit_price: dec!(50.00),
                ..Default::default()
            },
        )
        .unwrap();

    // Apply coupon code
    commerce.carts().apply_discount(cart.id, "SAVE25").unwrap();

    // Calculate promotions
    let cart_id: uuid::Uuid = cart.id.into();
    let result = commerce.apply_cart_promotions(cart_id).unwrap();

    // 25% off of $100 = $25 discount
    assert_eq!(result.total_discount, dec!(25.00));
    assert_eq!(result.applied_promotions.len(), 1);
    assert_eq!(result.applied_promotions[0].promotion_name, "25% Off Everything");

    // Verify cart was updated
    let updated_cart = commerce.carts().get(cart_id.into()).unwrap().unwrap();
    assert_eq!(updated_cart.discount_amount, dec!(25.00));
    assert_eq!(updated_cart.grand_total, dec!(75.00)); // $100 - $25
}
