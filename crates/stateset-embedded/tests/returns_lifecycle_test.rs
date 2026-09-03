#![cfg(feature = "sqlite")]

//! Integration tests for Returns/RMA lifecycle
//!
//! Tests the full return lifecycle including:
//! - Create → Approve → Receive → Complete
//! - Create → Reject
//! - Create → Cancel
//! - Tracking number management
//! - Invalid state transitions
//! - Filtering by status, order, and customer

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateOrder, CreateOrderItem, CreateReturn, CreateReturnItem,
    ItemCondition, OrderStatus, Return, ReturnFilter, ReturnReason, ReturnStatus, UpdateReturn,
};
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_customer(commerce: &Commerce) -> Uuid {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("return-test-{}@example.com", Uuid::new_v4()),
            first_name: "Return".into(),
            last_name: "Tester".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
        .into_uuid()
}

fn create_delivered_order(commerce: &Commerce, customer_id: Uuid) -> stateset_embedded::Order {
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer_id.into(),
            items: vec![
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "RET-SKU-001".into(),
                    name: "Widget Alpha".into(),
                    quantity: 3,
                    unit_price: dec!(49.99),
                    ..Default::default()
                },
                CreateOrderItem {
                    product_id: Uuid::new_v4().into(),
                    sku: "RET-SKU-002".into(),
                    name: "Widget Beta".into(),
                    quantity: 1,
                    unit_price: dec!(99.99),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .expect("Failed to create order");

    // Move order to delivered
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Confirmed)
        .expect("Failed to confirm order");
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Processing)
        .expect("Failed to process order");
    commerce.orders().update_status(order.id, OrderStatus::Shipped).expect("Failed to ship order");
    commerce
        .orders()
        .update_status(order.id, OrderStatus::Delivered)
        .expect("Failed to deliver order");

    commerce.orders().get(order.id).expect("Failed to get order").expect("Order not found")
}

fn create_test_return(commerce: &Commerce, order: &stateset_embedded::Order) -> Return {
    let items: Vec<CreateReturnItem> = order
        .items
        .iter()
        .take(1)
        .map(|item| CreateReturnItem {
            order_item_id: item.id,
            quantity: 1,
            condition: Some(ItemCondition::Defective),
        })
        .collect();

    commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            reason_details: Some("Product stopped working after 2 days".into()),
            items,
            notes: Some("Customer reports intermittent failure".into()),
            ..Default::default()
        })
        .expect("Failed to create return")
}

// ============================================================================
// Full Lifecycle: Create → Approve → Receive → Complete
// ============================================================================

#[test]
fn test_return_full_approve_complete_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    // 1. Create return
    let ret = create_test_return(&commerce, &order);
    assert_eq!(ret.status, ReturnStatus::Requested);
    assert_eq!(ret.order_id, order.id);
    assert_eq!(ret.reason, ReturnReason::Defective);
    assert!(ret.reason_details.is_some());
    assert!(!ret.items.is_empty());

    // 2. Approve
    let ret = commerce.returns().approve(ret.id).expect("Failed to approve");
    assert_eq!(ret.status, ReturnStatus::Approved);

    // 3. Add tracking and mark in-transit
    let ret =
        commerce.returns().add_tracking(ret.id, "RMA-TRACK-12345").expect("Failed to add tracking");
    assert_eq!(ret.status, ReturnStatus::InTransit);
    assert_eq!(ret.tracking_number.as_deref(), Some("RMA-TRACK-12345"));

    // 4. Mark received
    let ret = commerce.returns().mark_received(ret.id).expect("Failed to mark received");
    assert_eq!(ret.status, ReturnStatus::Received);

    // 5. Complete with refund info
    let ret = commerce
        .returns()
        .update(
            ret.id,
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                // No warehouse disposition in this flow: write the received
                // units off explicitly (completion refuses silently vanishing
                // stock otherwise).
                write_off_undispositioned: true,
                refund_amount: Some(dec!(49.99)),
                refund_method: Some("original_payment".into()),
                ..Default::default()
            },
        )
        .expect("Failed to complete return");
    assert_eq!(ret.status, ReturnStatus::Completed);
    assert_eq!(ret.refund_amount, Some(dec!(49.99)));
    assert_eq!(ret.refund_method.as_deref(), Some("original_payment"));
}

// ============================================================================
// Rejection Workflow
// ============================================================================

#[test]
fn test_return_reject_workflow() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    assert_eq!(ret.status, ReturnStatus::Requested);

    let ret = commerce
        .returns()
        .reject(ret.id, "Outside 30-day return window")
        .expect("Failed to reject return");
    assert_eq!(ret.status, ReturnStatus::Rejected);
}

// ============================================================================
// Cancellation Workflow
// ============================================================================

#[test]
fn test_return_cancel_workflow() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    let ret = commerce.returns().cancel(ret.id).expect("Failed to cancel return");
    assert_eq!(ret.status, ReturnStatus::Cancelled);
}

#[test]
fn test_return_cancel_after_approval() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    let ret = commerce.returns().approve(ret.id).expect("approve");
    assert_eq!(ret.status, ReturnStatus::Approved);

    let ret = commerce.returns().cancel(ret.id).expect("cancel");
    assert_eq!(ret.status, ReturnStatus::Cancelled);
}

// ============================================================================
// Return Retrieval and Filtering
// ============================================================================

#[test]
fn test_return_get_by_id() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let created = create_test_return(&commerce, &order);

    let fetched = commerce
        .returns()
        .get(created.id)
        .expect("Failed to get return")
        .expect("Return not found");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.order_id, order.id);
}

#[test]
fn test_return_get_nonexistent() {
    let commerce = Commerce::new(":memory:").expect("init");
    let result = commerce.returns().get(Uuid::new_v4().into()).expect("Failed to query");
    assert!(result.is_none());
}

#[test]
fn test_return_list_for_order() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    // Create two returns for same order
    create_test_return(&commerce, &order);
    create_test_return(&commerce, &order);

    let returns = commerce.returns().list_for_order(order.id).expect("Failed to list returns");
    assert_eq!(returns.len(), 2);
}

#[test]
fn test_return_list_for_customer() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order1 = create_delivered_order(&commerce, customer_id);
    let order2 = create_delivered_order(&commerce, customer_id);

    create_test_return(&commerce, &order1);
    create_test_return(&commerce, &order2);

    let returns = commerce
        .returns()
        .list_for_customer(customer_id.into())
        .expect("Failed to list customer returns");
    assert_eq!(returns.len(), 2);
}

#[test]
fn test_return_list_pending() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    let ret1 = create_test_return(&commerce, &order);
    let ret2 = create_test_return(&commerce, &order);

    // Approve one, leave the other pending
    commerce.returns().approve(ret1.id).expect("approve");

    let pending = commerce.returns().list_pending().expect("list pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, ret2.id);
}

#[test]
fn test_return_filter_by_status() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    let ret1 = create_test_return(&commerce, &order);
    let _ret2 = create_test_return(&commerce, &order);

    commerce.returns().approve(ret1.id).expect("approve");

    let approved = commerce
        .returns()
        .list(ReturnFilter { status: Some(ReturnStatus::Approved), ..Default::default() })
        .expect("Failed to filter");
    assert_eq!(approved.len(), 1);
    assert_eq!(approved[0].id, ret1.id);
}

#[test]
fn test_return_filter_by_reason() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    // Create a return with a different reason
    commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::ChangedMind,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create");

    create_test_return(&commerce, &order); // Defective reason

    let defective_returns = commerce
        .returns()
        .list(ReturnFilter { reason: Some(ReturnReason::Defective), ..Default::default() })
        .expect("filter by reason");
    assert_eq!(defective_returns.len(), 1);
}

#[test]
fn test_return_count() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    create_test_return(&commerce, &order);
    create_test_return(&commerce, &order);
    create_test_return(&commerce, &order);

    let count = commerce.returns().count(ReturnFilter::default()).expect("count");
    assert_eq!(count, 3);
}

// ============================================================================
// Return Reasons Coverage
// ============================================================================

#[test]
fn test_return_all_reasons() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);

    let reasons = vec![
        ReturnReason::Defective,
        ReturnReason::WrongItem,
        ReturnReason::NotAsDescribed,
        ReturnReason::ChangedMind,
        ReturnReason::BetterPriceFound,
        ReturnReason::NoLongerNeeded,
        ReturnReason::Damaged,
        ReturnReason::Other,
    ];

    for reason in reasons {
        // A fresh order per reason so each return stays within the purchased
        // quantity (the engine now rejects returning more than was ordered).
        let order = create_delivered_order(&commerce, customer_id);
        let ret = commerce
            .returns()
            .create(CreateReturn {
                order_id: order.id,
                reason,
                items: vec![CreateReturnItem {
                    order_item_id: order.items[0].id,
                    quantity: 1,
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap_or_else(|e| panic!("Failed to create return with reason {:?}: {}", reason, e));
        assert_eq!(ret.reason, reason);
    }
}

// ============================================================================
// Tracking Number
// ============================================================================

#[test]
fn test_return_add_tracking_sets_in_transit() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    let ret = commerce.returns().approve(ret.id).expect("approve");
    let ret = commerce.returns().add_tracking(ret.id, "FEDEX-999888777").expect("add tracking");

    assert_eq!(ret.status, ReturnStatus::InTransit);
    assert_eq!(ret.tracking_number.as_deref(), Some("FEDEX-999888777"));
}

// ============================================================================
// Idempotency
// ============================================================================

#[test]
fn test_return_idempotency_key() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    let key = format!("idem-{}", Uuid::new_v4());

    let ret1 = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            idempotency_key: Some(key.clone()),
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("first create");

    let ret2 = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            idempotency_key: Some(key),
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("second create");

    assert_eq!(ret1.id, ret2.id, "Idempotent creates should return same return");
}

// ============================================================================
// Notes and Updates
// ============================================================================

#[test]
fn test_return_update_notes() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    let ret = commerce
        .returns()
        .update(
            ret.id,
            UpdateReturn {
                notes: Some("Customer called, expedite this return".into()),
                ..Default::default()
            },
        )
        .expect("update notes");

    assert_eq!(ret.notes.as_deref(), Some("Customer called, expedite this return"));
}

// ============================================================================
// Version Tracking
// ============================================================================

#[test]
fn test_return_version_increments() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    let v0 = ret.version;

    let ret = commerce.returns().approve(ret.id).expect("approve");
    assert!(ret.version > v0, "Version should increment on approve");

    let v1 = ret.version;
    let ret = commerce
        .returns()
        .update(
            ret.id,
            UpdateReturn { status: Some(ReturnStatus::InTransit), ..Default::default() },
        )
        .expect("mark in transit");
    assert!(ret.version > v1, "Version should increment on status change");

    let v2 = ret.version;
    let ret = commerce.returns().mark_received(ret.id).expect("mark received");
    assert!(ret.version > v2, "Version should increment on status change");
}

// ============================================================================
// Timestamps
// ============================================================================

#[test]
fn test_return_timestamps_populated() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let ret = create_test_return(&commerce, &order);

    assert!(ret.created_at <= ret.updated_at);

    let ret = commerce.returns().approve(ret.id).expect("approve");
    assert!(ret.updated_at >= ret.created_at);
}

// ============================================================================
// Multiple Items Return
// ============================================================================

#[test]
fn test_return_multiple_items() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);

    let items: Vec<CreateReturnItem> = order
        .items
        .iter()
        .map(|item| CreateReturnItem {
            order_item_id: item.id,
            quantity: 1,
            condition: Some(ItemCondition::Damaged),
        })
        .collect();

    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Damaged,
            items,
            ..Default::default()
        })
        .expect("create multi-item return");

    assert_eq!(ret.items.len(), 2, "Return should have 2 items");
}

// ============================================================================
// Over-return integrity: cannot return more than was purchased
// ============================================================================

#[test]
fn test_return_rejects_more_than_ordered_quantity() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    // item[0] was ordered with quantity 3.
    let item = &order.items[0];

    let err = commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: item.id,
                quantity: 5, // more than the 3 ordered
                condition: Some(ItemCondition::Defective),
            }],
            ..Default::default()
        })
        .expect_err("returning more than ordered must be rejected");
    let msg = format!("{err}").to_lowercase();
    assert!(msg.contains("returnable") || msg.contains("return"), "unexpected error: {err}");
}

#[test]
fn test_return_rejects_cumulative_over_return() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order = create_delivered_order(&commerce, customer_id);
    let item_id = order.items[0].id; // ordered quantity 3

    // Return all 3 units first.
    commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: item_id,
                quantity: 3,
                condition: Some(ItemCondition::Defective),
            }],
            ..Default::default()
        })
        .expect("first return of all 3 units");

    // A second return of even 1 more unit must be rejected — nothing remains.
    commerce
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: item_id,
                quantity: 1,
                condition: Some(ItemCondition::Defective),
            }],
            ..Default::default()
        })
        .expect_err("returning past the ordered quantity across returns must be rejected");
}

#[test]
fn test_return_rejects_item_from_another_order() {
    let commerce = Commerce::new(":memory:").expect("init");
    let customer_id = create_test_customer(&commerce);
    let order_a = create_delivered_order(&commerce, customer_id);
    let order_b = create_delivered_order(&commerce, customer_id);

    // Try to return order B's item against order A.
    let foreign_item_id = order_b.items[0].id;
    commerce
        .returns()
        .create(CreateReturn {
            order_id: order_a.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: foreign_item_id,
                quantity: 1,
                condition: Some(ItemCondition::Defective),
            }],
            ..Default::default()
        })
        .expect_err("returning an item that belongs to another order must be rejected");
}
