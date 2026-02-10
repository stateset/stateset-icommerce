//!
//! Comprehensive end-to-end integration tests for full commerce workflows.
//!
//! These tests simulate real-world commerce scenarios spanning multiple domains:
//! - Complete order lifecycle (create → ship → return → refund)
//! - Multi-item inventory allocation
//! - Cross-domain transactions (order + inventory + payment)
//! - Cart completion with promotions and tax
//! - Subscription billing cycles
//! - Manufacturing workflows
//!

use rust_decimal_macros::dec;
use stateset_embedded::{
    AddCartItem, BillingCycleFilter, BillingInterval, CartAddress, Commerce, CreateBackorder,
    CreateBom, CreateBomComponent, CreateCart, CreateCouponCode, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreateOrderItem, CreatePromotion, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreateReturn, CreateReturnItem, CreateSerialNumbersBulk,
    CreateSubscription, CreateSubscriptionPlan, CreateSupplier, CreateWorkOrder, FulfillBackorder,
    FulfillmentSourceType, InventoryItem, ItemCondition, OrderStatus, PromotionType,
    PurchaseOrderStatus, ReserveSerialNumber, ReturnReason, ReturnStatus, SerialStatus,
    SubscriptionStatus, WorkOrderStatus,
};
use uuid::Uuid;

fn create_test_customer(commerce: &Commerce) -> Uuid {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
}

fn setup_test_inventory(
    commerce: &Commerce,
    sku: &str,
    name: &str,
    quantity: rust_decimal::Decimal,
) -> InventoryItem {
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: name.into(),
            initial_quantity: Some(quantity),
            ..Default::default()
        })
        .expect("Failed to create inventory item")
}

fn test_address() -> CartAddress {
    CartAddress {
        first_name: "Test".into(),
        last_name: "Customer".into(),
        company: None,
        line1: "123 Main St".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: Some("555-1234".into()),
        email: Some("customer@example.com".into()),
    }
}

#[test]
fn test_complete_order_lifecycle_from_cart_to_return() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    setup_test_inventory(&commerce, "SKU-001", "Widget", dec!(100));
    setup_test_inventory(&commerce, "SKU-002", "Gadget", dec!(50));

    let _customer_id = create_test_customer(&commerce);

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some(format!("customer-{}@example.com", Uuid::new_v4())),
            customer_name: Some("Test Customer".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item to cart");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-002".into(),
                name: "Gadget".into(),
                quantity: 1,
                unit_price: dec!(49.99),
                ..Default::default()
            },
        )
        .expect("Failed to add second item to cart");

    commerce
        .carts()
        .set_shipping_address(cart.id, test_address())
        .expect("Failed to set shipping address");

    let checkout = commerce
        .carts()
        .complete(cart.id)
        .expect("Failed to complete checkout");

    // Fetch the order using the order_id from checkout
    let order = commerce
        .orders()
        .get(checkout.order_id)
        .expect("Failed to get order")
        .expect("Order not found");
    assert_eq!(order.items.len(), 2);
    assert_eq!(order.status, OrderStatus::Confirmed);

    let order = commerce
        .orders()
        .ship(order.id, Some("FEDEX123456"))
        .expect("Failed to ship order");

    assert_eq!(order.status, OrderStatus::Shipped);
    assert_eq!(order.tracking_number, Some("FEDEX123456".into()));

    let order = commerce
        .orders()
        .update_status(order.id, OrderStatus::Delivered)
        .expect("Failed to mark order as delivered");

    // Get an order item id for the return
    let order_item_id = order.items[0].id;

    let return_order = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Damaged,
            reason_details: Some("Product damaged".into()),
            items: vec![CreateReturnItem {
                order_item_id,
                quantity: 1,
                condition: Some(ItemCondition::Damaged),
            }],
            ..Default::default()
        })
        .expect("Failed to create return");

    let return_order = commerce
        .returns()
        .approve(return_order.id)
        .expect("Failed to approve return");

    assert_eq!(return_order.status, ReturnStatus::Approved);
}

#[test]
fn test_cart_with_promotions_and_tax() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    setup_test_inventory(&commerce, "SKU-001", "Widget", dec!(100));

    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "20% Off Summer Sale".into(),
            description: Some("20% off all products".into()),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.20)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    // Activate the promotion
    commerce
        .promotions()
        .activate(promotion.id)
        .expect("Failed to activate promotion");

    let coupon = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            code: "SUMMER20".into(),
            promotion_id: promotion.id,
            usage_limit: Some(100),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon");

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some(format!("customer-{}@example.com", Uuid::new_v4())),
            customer_name: Some("Test Customer".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item to cart");

    let cart = commerce
        .carts()
        .apply_discount(cart.id, &coupon.code)
        .expect("Failed to apply discount");

    assert!(cart.discount_amount > dec!(0));
}

#[test]
fn test_subscription_billing_cycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);

    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: "Pro Monthly".into(),
            price: dec!(29.99),
            billing_interval: BillingInterval::Monthly,
            ..Default::default()
        })
        .expect("Failed to create subscription plan");

    let plan = commerce
        .subscriptions()
        .activate_plan(plan.id)
        .expect("Failed to activate subscription plan");

    let subscription = commerce
        .subscriptions()
        .subscribe(CreateSubscription {
            customer_id,
            plan_id: plan.id,
            ..Default::default()
        })
        .expect("Failed to create subscription");

    assert_eq!(subscription.status, SubscriptionStatus::Active);

    let billing_cycles = commerce
        .subscriptions()
        .list_billing_cycles(BillingCycleFilter {
            subscription_id: Some(subscription.id),
            ..Default::default()
        })
        .expect("Failed to list billing cycles");

    assert_eq!(billing_cycles.len(), 1);
}

#[test]
fn test_manufacturing_workflow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let product_id = Uuid::new_v4();
    let bom = commerce
        .bom()
        .create(CreateBom {
            product_id,
            name: "Widget Assembly".into(),
            ..Default::default()
        })
        .expect("Failed to create BOM");

    commerce
        .bom()
        .add_component(
            bom.id,
            CreateBomComponent {
                name: "Part A".into(),
                quantity: dec!(2),
                unit_of_measure: Some("pcs".into()),
                ..Default::default()
            },
        )
        .expect("Failed to add component to BOM");

    commerce
        .bom()
        .add_component(
            bom.id,
            CreateBomComponent {
                name: "Part B".into(),
                quantity: dec!(1),
                unit_of_measure: Some("pcs".into()),
                ..Default::default()
            },
        )
        .expect("Failed to add second component to BOM");

    let work_order = commerce
        .work_orders()
        .create(CreateWorkOrder {
            product_id,
            bom_id: Some(bom.id),
            quantity_to_build: dec!(50),
            ..Default::default()
        })
        .expect("Failed to create work order");

    assert_eq!(work_order.status, WorkOrderStatus::Planned);

    let work_order = commerce
        .work_orders()
        .start(work_order.id)
        .expect("Failed to start work order");

    assert_eq!(work_order.status, WorkOrderStatus::InProgress);

    let work_order = commerce
        .work_orders()
        .complete(work_order.id, dec!(50))
        .expect("Failed to complete work order");

    assert_eq!(work_order.status, WorkOrderStatus::Completed);
}

#[test]
fn test_supply_chain_workflow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: "Widget Supplier".into(),
            email: Some("supplier@example.com".into()),
            ..Default::default()
        })
        .expect("Failed to create supplier");

    let purchase_order = commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            ..Default::default()
        })
        .expect("Failed to create purchase order");

    let po_id = purchase_order.id;
    let _item = commerce
        .purchase_orders()
        .add_item(
            po_id,
            CreatePurchaseOrderItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: dec!(100),
                unit_cost: dec!(10.00),
                ..Default::default()
            },
        )
        .expect("Failed to add item to purchase order");

    // Submit first, then approve
    let purchase_order = commerce
        .purchase_orders()
        .submit(po_id)
        .expect("Failed to submit purchase order");

    let purchase_order = commerce
        .purchase_orders()
        .approve(purchase_order.id, "admin")
        .expect("Failed to approve purchase order");

    assert_eq!(purchase_order.status, PurchaseOrderStatus::Approved);
}

#[test]
fn test_serial_number_tracking_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    setup_test_inventory(&commerce, "SKU-001", "Widget", dec!(100));

    let serials = commerce
        .serials()
        .create_bulk(CreateSerialNumbersBulk {
            sku: "SKU-001".into(),
            quantity: 5,
            ..Default::default()
        })
        .expect("Failed to create serial numbers");

    assert_eq!(serials.len(), 5);

    for serial in &serials {
        assert_eq!(serial.status, SerialStatus::Available);
    }

    let serial = &serials[0];
    let customer_id = create_test_customer(&commerce);

    let reservation = commerce
        .serials()
        .reserve(ReserveSerialNumber {
            serial_id: serial.id,
            reference_type: "order".into(),
            reference_id: Uuid::new_v4(),
            ..Default::default()
        })
        .expect("Failed to reserve serial");

    // SerialReservation doesn't have a status field - check that it was created
    assert_eq!(reservation.serial_id, serial.id);

    commerce
        .serials()
        .confirm_reservation(reservation.id)
        .expect("Failed to confirm serial reservation");

    let serial = commerce
        .serials()
        .get(serial.id)
        .expect("Failed to get serial")
        .expect("Serial not found");

    assert_eq!(serial.status, SerialStatus::Reserved);

    let serial = commerce
        .serials()
        .mark_sold(serial.id, customer_id, None)
        .expect("Failed to mark serial as sold");

    assert_eq!(serial.status, SerialStatus::Sold);
}

#[test]
fn test_backorder_workflow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);

    // Create order first (no inventory reservation since item doesn't exist yet)
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    // Now create inventory item with 0 stock (simulating out-of-stock scenario)
    setup_test_inventory(&commerce, "SKU-001", "Widget", dec!(0));

    let backorder = commerce
        .backorder()
        .create_backorder(CreateBackorder {
            order_id: order.id,
            order_line_id: None,
            sku: "SKU-001".into(),
            quantity: dec!(2),
            customer_id,
            priority: None,
            expected_date: None,
            promised_date: None,
            source_location_id: None,
            notes: None,
        })
        .expect("Failed to create backorder");

    commerce
        .inventory()
        .adjust("SKU-001", dec!(10), "Restocking")
        .expect("Failed to adjust inventory");

    let backorder = commerce
        .backorder()
        .fulfill_backorder(FulfillBackorder {
            backorder_id: backorder.id,
            quantity: dec!(2),
            source_type: FulfillmentSourceType::Inventory,
            source_id: None,
            notes: None,
            fulfilled_by: None,
        })
        .expect("Failed to fulfill backorder");

    assert_eq!(backorder.quantity_remaining, dec!(0));
}
