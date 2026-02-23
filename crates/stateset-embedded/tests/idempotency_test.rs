//! Idempotency tests for payments, refunds, and returns

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund,
    CreateReturn, CreateReturnItem, PaymentMethodType, ReturnReason,
};
use uuid::Uuid;

fn create_order_with_item(
    commerce: &Commerce,
) -> (stateset_embedded::OrderId, stateset_embedded::OrderItemId) {
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("idem-{}@example.com", Uuid::new_v4()),
            first_name: "Idem".into(),
            last_name: "Potent".into(),
            ..Default::default()
        })
        .expect("Failed to create customer");

    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "IDEM-001".into(),
                name: "Idempotent Widget".into(),
                quantity: 1,
                unit_price: dec!(9.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order");

    let item_id = order.items.first().expect("Order item missing").id;
    (order.id, item_id)
}

#[test]
fn test_payment_idempotency_key() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            amount: dec!(25.00),
            payment_method: PaymentMethodType::CreditCard,
            idempotency_key: Some("pay-idem-1".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");

    let retry = commerce
        .payments()
        .create(CreatePayment {
            amount: dec!(50.00),
            payment_method: PaymentMethodType::DebitCard,
            idempotency_key: Some("pay-idem-1".into()),
            ..Default::default()
        })
        .expect("Failed to retry payment");

    assert_eq!(payment.id, retry.id);
    assert_eq!(commerce.payments().count(Default::default()).unwrap(), 1);
}

#[test]
fn test_refund_idempotency_key() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            amount: dec!(75.00),
            payment_method: PaymentMethodType::CreditCard,
            ..Default::default()
        })
        .expect("Failed to create payment");

    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            reason: Some("Test refund".into()),
            idempotency_key: Some("refund-idem-1".into()),
            ..Default::default()
        })
        .expect("Failed to create refund");

    let retry = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(15.00)),
            reason: Some("Retry refund".into()),
            idempotency_key: Some("refund-idem-1".into()),
            ..Default::default()
        })
        .expect("Failed to retry refund");

    assert_eq!(refund.id, retry.id);
    assert_eq!(commerce.payments().get_refunds(payment.id).unwrap().len(), 1);
}

#[test]
fn test_return_idempotency_key() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let (order_id, order_item_id) = create_order_with_item(&commerce);

    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id,
            reason: ReturnReason::Other,
            items: vec![CreateReturnItem { order_item_id, quantity: 1, ..Default::default() }],
            idempotency_key: Some("return-idem-1".into()),
            ..Default::default()
        })
        .expect("Failed to create return");

    let retry = commerce
        .returns()
        .create(CreateReturn {
            order_id,
            reason: ReturnReason::Damaged,
            items: vec![CreateReturnItem { order_item_id, quantity: 1, ..Default::default() }],
            idempotency_key: Some("return-idem-1".into()),
            ..Default::default()
        })
        .expect("Failed to retry return");

    assert_eq!(ret.id, retry.id);
    assert_eq!(commerce.returns().count(Default::default()).unwrap(), 1);
}
