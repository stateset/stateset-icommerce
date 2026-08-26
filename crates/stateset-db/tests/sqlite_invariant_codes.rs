#![cfg(feature = "sqlite")]
//! The commerce invariants catalogued in `docs/src/advanced/invariants.md` must
//! be rejected with a **typed** `CommerceError` carrying the stable code from
//! `icp-conformance/vectors/icp-1.0/10-commerce-invariants/` — not with a
//! stringly `ValidationError` an agent cannot branch on.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CreateRefund, CreateReturn, CreateReturnItem, CustomerId, CustomerRepository,
    InventoryRepository, Order, OrderRepository, OrderStatus, PaymentMethodType, PaymentRepository,
    ProductId, ReturnReason, ReturnRepository, UpdateOrder,
};
use stateset_db::SqliteDatabase;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

fn customer(db: &SqliteDatabase, email: &str) -> CustomerId {
    db.customers()
        .create(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

fn stock(db: &SqliteDatabase, sku: &str, qty: Decimal) {
    match db.inventory().create_item(CreateInventoryItem {
        sku: sku.to_string(),
        name: sku.to_string(),
        initial_quantity: Some(qty),
        ..Default::default()
    }) {
        Ok(_) | Err(CommerceError::DuplicateSku(_)) => {}
        Err(e) => panic!("create inventory item: {e:?}"),
    }
}

/// A single-line order for 2 x $25.00 (total $50.00), left in `Pending`.
fn order(db: &SqliteDatabase, email: &str, sku: &str) -> Order {
    let customer_id = customer(db, email);
    stock(db, sku, dec!(10));
    db.orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.to_string(),
                name: sku.to_string(),
                quantity: 2,
                unit_price: dec!(25.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
}

#[test]
fn over_refund_returns_refund_exceeds_captured() {
    let db = db();
    let payment = db
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("create payment");
    let payment = db.payments().mark_completed(payment.id).expect("complete payment");

    let err = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(150.00)),
            ..Default::default()
        })
        .expect_err("over-refund must be rejected");

    match &err {
        CommerceError::RefundExceedsCaptured {
            payment_id,
            captured,
            already_refunded,
            requested,
        } => {
            assert_eq!(*payment_id, payment.id.into_uuid());
            assert_eq!(captured, "100.00");
            assert_eq!(requested, "150.00");
            assert_eq!(already_refunded.parse::<Decimal>().expect("decimal"), Decimal::ZERO);
        }
        other => panic!("expected RefundExceedsCaptured, got {other:?}"),
    }
    assert_eq!(err.invariant_code(), Some("commerce.refund.exceeds_captured"));
    // Rejected operations still write nothing (invariant A1).
    assert!(db.payments().get_refunds(payment.id).expect("list refunds").is_empty());
}

#[test]
fn over_capture_returns_capture_exceeds_order_total() {
    let db = db();
    let order = order(&db, "capture@example.com", "INV-CODE-CAP");

    // The order total is 50.00; a 60.00 capture cannot be justified by the books.
    let err = db
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(60.00),
            ..Default::default()
        })
        .expect_err("capture above the order total must be rejected");

    match &err {
        CommerceError::CaptureExceedsOrderTotal {
            order_id,
            order_total,
            already_captured,
            requested,
        } => {
            assert_eq!(*order_id, order.id.into_uuid());
            assert_eq!(order_total.parse::<Decimal>().expect("decimal"), order.total_amount);
            assert_eq!(already_captured.parse::<Decimal>().expect("decimal"), Decimal::ZERO);
            assert_eq!(requested, "60.00");
        }
        other => panic!("expected CaptureExceedsOrderTotal, got {other:?}"),
    }
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"));
    // The message text is unchanged from the untyped guard it replaced.
    assert!(err.to_string().contains("already captured or in flight"), "{err}");
}

#[test]
fn return_against_unshipped_order_returns_return_order_not_shipped() {
    let db = db();
    let order = order(&db, "notshipped@example.com", "INV-CODE-RET");

    let err = db
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
        .expect_err("a return against an unshipped order must be rejected");

    match &err {
        CommerceError::ReturnOrderNotShipped { order_id, status } => {
            assert_eq!(*order_id, order.id.into_uuid());
            assert_eq!(status, &OrderStatus::Pending.to_string());
        }
        other => panic!("expected ReturnOrderNotShipped, got {other:?}"),
    }
    assert_eq!(err.invariant_code(), Some("commerce.return.order_not_shipped"));
    assert!(err.to_string().contains("must be shipped or delivered"), "{err}");
}

#[test]
fn over_return_returns_return_exceeds_returnable() {
    let db = db();
    let order = order(&db, "overreturn@example.com", "INV-CODE-OVR");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .expect("advance status");
    }

    // 2 units shipped; 3 cannot come back.
    let err = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 3,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect_err("returning more than shipped must be rejected");

    match &err {
        CommerceError::ReturnExceedsReturnable {
            order_item_id,
            basis,
            returnable,
            already_returned,
            requested,
        } => {
            assert_eq!(*order_item_id, order.items[0].id.into_uuid());
            assert_eq!(*basis, "shipped");
            assert_eq!(*returnable, 2);
            assert_eq!(*already_returned, 0);
            assert_eq!(*requested, 3);
        }
        other => panic!("expected ReturnExceedsReturnable, got {other:?}"),
    }
    assert_eq!(err.invariant_code(), Some("commerce.return.exceeds_shipped"));
}
