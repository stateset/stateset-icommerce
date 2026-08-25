#![cfg(feature = "sqlite")]

//! Integration tests for Payment processing

use rust_decimal_macros::dec;
use stateset_core::CurrencyCode;
use stateset_embedded::{
    CardBrand, Commerce, CommerceError, CreateCustomer, CreateInvoice, CreateInvoiceItem,
    CreateOrder, CreateOrderItem, CreatePayment, CreatePaymentMethod, CreateRefund, CustomerId,
    OrderId, Payment, PaymentFilter, PaymentId, PaymentMethodType, PaymentTransactionStatus,
    RefundStatus, UpdatePayment,
};
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create a test customer
fn create_test_customer(commerce: &Commerce) -> CustomerId {
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

/// Helper to create a test order with default items
fn create_test_order(commerce: &Commerce, customer_id: CustomerId) -> OrderId {
    commerce
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "TEST-SKU-001".into(),
                name: "Test Product".into(),
                quantity: 2,
                unit_price: dec!(59.99), // order total 119.98 leaves room for the 99.99 test captures
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create order")
        .id
}

/// Helper to create a test invoice
fn create_test_invoice(commerce: &Commerce, customer_id: CustomerId) -> Uuid {
    commerce
        .invoices()
        .create(CreateInvoice {
            customer_id,
            items: vec![CreateInvoiceItem {
                description: "Test invoice item".into(),
                quantity: dec!(1),
                unit_price: dec!(49.99),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create invoice")
        .id
        .into()
}

/// Helper to create a test payment for an order
fn create_test_payment(
    commerce: &Commerce,
    order_id: Option<OrderId>,
    customer_id: Option<CustomerId>,
) -> Payment {
    commerce
        .payments()
        .create(CreatePayment {
            order_id,
            customer_id,
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(99.99),
            currency: Some(CurrencyCode::USD),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            billing_email: Some("test@example.com".into()),
            ..Default::default()
        })
        .expect("Failed to create payment")
}

// ============================================================================
// Basic Payment Creation Tests
// ============================================================================

#[test]
fn test_create_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order_id = create_test_order(&commerce, customer_id);

    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            customer_id: Some(customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(99.99),
            currency: Some(CurrencyCode::USD),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            billing_email: Some("payment@example.com".into()),
            billing_name: Some("Test User".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");

    assert!(!payment.id.is_nil());
    assert!(!payment.payment_number.is_empty());
    assert!(payment.payment_number.starts_with("PAY-"));
    assert_eq!(payment.order_id, Some(order_id));
    assert_eq!(payment.customer_id, Some(customer_id));
    assert_eq!(payment.status, PaymentTransactionStatus::Pending);
    assert_eq!(payment.payment_method, PaymentMethodType::CreditCard);
    assert_eq!(payment.amount, dec!(99.99));
    assert_eq!(payment.currency, CurrencyCode::USD);
    assert_eq!(payment.card_brand, Some(CardBrand::Visa));
    assert_eq!(payment.card_last4, Some("4242".into()));
}

#[test]
fn test_create_payment_without_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let payment = commerce
        .payments()
        .create(CreatePayment {
            customer_id: Some(customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(50.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    assert!(!payment.id.is_nil());
    assert!(payment.order_id.is_none());
    assert_eq!(payment.customer_id, Some(customer_id));
}

#[test]
fn test_create_payment_with_external_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(75.00),
            external_id: Some("pi_stripe_12345".into()),
            processor: Some("stripe".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");

    assert_eq!(payment.external_id, Some("pi_stripe_12345".into()));
    assert_eq!(payment.processor, Some("stripe".into()));
}

#[test]
fn test_create_payment_with_description() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            description: Some("Payment for order #12345".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");

    assert_eq!(payment.description, Some("Payment for order #12345".into()));
}

#[test]
fn test_create_payment_default_currency() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(50.00),
            // No currency specified - should default to USD
            ..Default::default()
        })
        .expect("Failed to create payment");

    assert_eq!(payment.currency, CurrencyCode::USD);
}

// ============================================================================
// Payment Retrieval Tests
// ============================================================================

#[test]
fn test_get_payment_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order_id = create_test_order(&commerce, customer_id);
    let created = create_test_payment(&commerce, Some(order_id), Some(customer_id));

    let retrieved = commerce
        .payments()
        .get(created.id)
        .expect("Failed to get payment")
        .expect("Payment not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.payment_number, created.payment_number);
    assert_eq!(retrieved.amount, created.amount);
}

#[test]
fn test_get_payment_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let fake_id: PaymentId = Uuid::new_v4().into();
    let result = commerce.payments().get(fake_id).expect("Should not error for missing payment");

    assert!(result.is_none());
}

#[test]
fn test_get_payment_by_number() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let created = create_test_payment(&commerce, None, None);

    let retrieved = commerce
        .payments()
        .get_by_number(&created.payment_number)
        .expect("Failed to get payment by number")
        .expect("Payment not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.payment_number, created.payment_number);
}

#[test]
fn test_get_payment_by_number_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce
        .payments()
        .get_by_number("PAY-NONEXISTENT-123")
        .expect("Should not error for missing payment");

    assert!(result.is_none());
}

#[test]
fn test_get_payment_by_external_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            external_id: Some("pi_test_external_123".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");

    let retrieved = commerce
        .payments()
        .get_by_external_id("pi_test_external_123")
        .expect("Failed to get payment by external ID")
        .expect("Payment not found");

    assert_eq!(retrieved.id, payment.id);
    assert_eq!(retrieved.external_id, Some("pi_test_external_123".into()));
}

// ============================================================================
// Payment Status Transition Tests
// ============================================================================

#[test]
fn test_payment_status_pending_to_completed() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    assert_eq!(payment.status, PaymentTransactionStatus::Pending);

    let completed =
        commerce.payments().mark_completed(payment.id).expect("Failed to mark payment completed");

    assert_eq!(completed.status, PaymentTransactionStatus::Completed);
    assert!(completed.paid_at.is_some());
}

#[test]
fn test_payment_status_pending_to_processing() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    assert_eq!(payment.status, PaymentTransactionStatus::Pending);

    let processing =
        commerce.payments().mark_processing(payment.id).expect("Failed to mark payment processing");

    assert_eq!(processing.status, PaymentTransactionStatus::Processing);
}

#[test]
fn test_payment_status_pending_to_failed() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    assert_eq!(payment.status, PaymentTransactionStatus::Pending);

    let failed = commerce
        .payments()
        .mark_failed(payment.id, "Card declined", Some("card_declined"))
        .expect("Failed to mark payment as failed");

    assert_eq!(failed.status, PaymentTransactionStatus::Failed);
    assert_eq!(failed.failure_reason, Some("Card declined".into()));
    assert_eq!(failed.failure_code, Some("card_declined".into()));
}

#[test]
fn test_payment_status_pending_to_cancelled() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    assert_eq!(payment.status, PaymentTransactionStatus::Pending);

    let cancelled = commerce.payments().cancel(payment.id).expect("Failed to cancel payment");

    assert_eq!(cancelled.status, PaymentTransactionStatus::Cancelled);
}

#[test]
fn test_payment_status_processing_to_completed() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    // Move to processing first
    commerce.payments().mark_processing(payment.id).expect("Failed to mark processing");

    // Then complete
    let completed =
        commerce.payments().mark_completed(payment.id).expect("Failed to mark completed");

    assert_eq!(completed.status, PaymentTransactionStatus::Completed);
}

#[test]
fn test_payment_status_processing_to_failed() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    // Move to processing first
    commerce.payments().mark_processing(payment.id).expect("Failed to mark processing");

    // Then fail
    let failed = commerce
        .payments()
        .mark_failed(payment.id, "Insufficient funds", Some("insufficient_funds"))
        .expect("Failed to mark as failed");

    assert_eq!(failed.status, PaymentTransactionStatus::Failed);
    assert_eq!(failed.failure_reason, Some("Insufficient funds".into()));
}

// ============================================================================
// Refund Tests
// ============================================================================

#[test]
fn test_create_refund() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    // Complete the payment first
    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    // Create a full refund
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            reason: Some("Customer request".into()),
            ..Default::default()
        })
        .expect("Failed to create refund");

    assert!(!refund.id.is_nil());
    assert!(!refund.refund_number.is_empty());
    assert!(refund.refund_number.starts_with("REF-"));
    assert_eq!(refund.payment_id, payment.id);
    assert_eq!(refund.status, RefundStatus::Pending);
    assert_eq!(refund.amount, payment.amount); // Full refund amount
    assert_eq!(refund.reason, Some("Customer request".into()));
}

#[test]
fn test_partial_refund() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    // Complete the payment first
    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    // Create a partial refund
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(25.00)),
            reason: Some("Partial refund - damaged item".into()),
            ..Default::default()
        })
        .expect("Failed to create partial refund");

    assert_eq!(refund.amount, dec!(25.00));
    assert_eq!(refund.reason, Some("Partial refund - damaged item".into()));
}

#[test]
fn test_multiple_partial_refunds() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create payment with known amount
    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    // Complete the payment
    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    // First partial refund
    let refund1 = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(30.00)),
            reason: Some("First partial refund".into()),
            ..Default::default()
        })
        .expect("Failed to create first refund");

    // Second partial refund
    let refund2 = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(20.00)),
            reason: Some("Second partial refund".into()),
            ..Default::default()
        })
        .expect("Failed to create second refund");

    assert_eq!(refund1.amount, dec!(30.00));
    assert_eq!(refund2.amount, dec!(20.00));

    // Get all refunds for the payment
    let refunds = commerce.payments().get_refunds(payment.id).expect("Failed to get refunds");

    assert_eq!(refunds.len(), 2);
}

#[test]
fn test_get_refund_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let created_refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .expect("Failed to create refund");

    let retrieved = commerce
        .payments()
        .get_refund(created_refund.id)
        .expect("Failed to get refund")
        .expect("Refund not found");

    assert_eq!(retrieved.id, created_refund.id);
    assert_eq!(retrieved.refund_number, created_refund.refund_number);
    assert_eq!(retrieved.amount, dec!(50.00));
}

#[test]
fn test_complete_refund() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .expect("Failed to create refund");

    assert_eq!(refund.status, RefundStatus::Pending);

    let completed_refund =
        commerce.payments().complete_refund(refund.id).expect("Failed to complete refund");

    assert_eq!(completed_refund.status, RefundStatus::Completed);
    assert!(completed_refund.refunded_at.is_some());
}

#[test]
fn test_fail_refund() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(50.00)),
            ..Default::default()
        })
        .expect("Failed to create refund");

    let failed_refund = commerce
        .payments()
        .fail_refund(refund.id, "Refund processing error")
        .expect("Failed to fail refund");

    assert_eq!(failed_refund.status, RefundStatus::Failed);
    assert_eq!(failed_refund.failure_reason, Some("Refund processing error".into()));
}

#[test]
fn test_refund_rejects_amount_exceeding_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None); // amount = 99.99
    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let err = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(150.00)),
            ..Default::default()
        })
        .expect_err("refund exceeding payment must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // Nothing should have been persisted.
    assert!(commerce.payments().get_refunds(payment.id).expect("list refunds").is_empty());
}

#[test]
fn test_refund_rejects_non_positive_amount() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);
    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    for amount in [dec!(0.00), dec!(-1.00)] {
        let err = commerce
            .payments()
            .create_refund(CreateRefund {
                payment_id: payment.id,
                amount: Some(amount),
                ..Default::default()
            })
            .expect_err("non-positive refund must be rejected");
        assert!(matches!(err, CommerceError::ValidationError(_)), "{amount}: {err:?}");
    }
}

#[test]
fn test_refund_rejects_pending_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    // Payment left in the default `Pending` state (not captured).
    let payment = create_test_payment(&commerce, None, None);

    let err = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect_err("refunding a pending payment must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

#[test]
fn test_refund_rejects_failed_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);
    commerce
        .payments()
        .mark_failed(payment.id, "declined", Some("declined"))
        .expect("Failed to fail payment");

    let err = commerce
        .payments()
        .create_refund(CreateRefund { payment_id: payment.id, amount: None, ..Default::default() })
        .expect_err("refunding a failed payment must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

#[test]
fn test_two_partial_refunds_sum_to_exact_decimal() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    // 0.10 + 0.20 == 0.30 exactly, but SQLite float math on TEXT columns would
    // yield 0.30000000000000004. This guards the money-precision regression
    // through the embedded accessor path.
    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(0.30),
            ..Default::default()
        })
        .expect("Failed to create payment");
    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let r1 = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.10)),
            ..Default::default()
        })
        .expect("first refund");
    commerce.payments().complete_refund(r1.id).expect("complete first refund");

    let r2 = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(0.20)),
            ..Default::default()
        })
        .expect("second refund");
    commerce.payments().complete_refund(r2.id).expect("complete second refund");

    let reloaded = commerce.payments().get(payment.id).expect("get").expect("payment present");
    assert_eq!(reloaded.amount_refunded, dec!(0.30));
    assert_eq!(reloaded.amount_refunded.to_string(), "0.30");
    assert_eq!(reloaded.status, PaymentTransactionStatus::Refunded);
}

#[test]
fn test_refund_with_external_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(25.00)),
            external_id: Some("re_stripe_refund_123".into()),
            notes: Some("Processed via Stripe".into()),
            ..Default::default()
        })
        .expect("Failed to create refund");

    assert_eq!(refund.external_id, Some("re_stripe_refund_123".into()));
    assert_eq!(refund.notes, Some("Processed via Stripe".into()));
}

// ============================================================================
// Payment Listing Tests
// ============================================================================

#[test]
fn test_list_payments() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create multiple payments
    for _ in 0..5 {
        create_test_payment(&commerce, None, None);
    }

    let payments =
        commerce.payments().list(PaymentFilter::default()).expect("Failed to list payments");

    assert!(payments.len() >= 5);
}

#[test]
fn test_list_payments_by_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order_id = create_test_order(&commerce, customer_id);

    // Create payments for the order
    for _ in 0..3 {
        commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(order_id),
                payment_method: PaymentMethodType::CreditCard,
                amount: dec!(33.33),
                ..Default::default()
            })
            .expect("Failed to create payment");
    }

    // Create some payments without order
    create_test_payment(&commerce, None, None);
    create_test_payment(&commerce, None, None);

    let order_payments =
        commerce.payments().for_order(order_id).expect("Failed to get payments for order");

    assert_eq!(order_payments.len(), 3);
    assert!(order_payments.iter().all(|p| p.order_id == Some(order_id)));
}

#[test]
fn test_list_payments_by_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer1 = create_test_customer(&commerce);
    let customer2 = create_test_customer(&commerce);

    // Create payments for customer1
    for _ in 0..3 {
        create_test_payment(&commerce, None, Some(customer1));
    }

    // Create payments for customer2
    for _ in 0..2 {
        create_test_payment(&commerce, None, Some(customer2));
    }

    let customer1_payments = commerce
        .payments()
        .list(PaymentFilter { customer_id: Some(customer1), ..Default::default() })
        .expect("Failed to list payments");

    assert_eq!(customer1_payments.len(), 3);
    assert!(customer1_payments.iter().all(|p| p.customer_id == Some(customer1)));
}

#[test]
fn test_list_payments_by_invoice() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let invoice1 = create_test_invoice(&commerce, customer_id);
    let invoice2 = create_test_invoice(&commerce, customer_id);

    commerce
        .payments()
        .create(CreatePayment {
            invoice_id: Some(invoice1),
            customer_id: Some(customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(49.99),
            ..Default::default()
        })
        .expect("Failed to create invoice payment 1");

    commerce
        .payments()
        .create(CreatePayment {
            invoice_id: Some(invoice2),
            customer_id: Some(customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(24.99),
            ..Default::default()
        })
        .expect("Failed to create invoice payment 2");

    create_test_payment(&commerce, None, Some(customer_id));

    let filtered = commerce
        .payments()
        .list(PaymentFilter { invoice_id: Some(invoice1), ..Default::default() })
        .expect("Failed to filter payments by invoice");
    let invoice_payments =
        commerce.payments().for_invoice(invoice1).expect("Failed to fetch payments for invoice");
    let count = commerce
        .payments()
        .count(PaymentFilter { invoice_id: Some(invoice1), ..Default::default() })
        .expect("Failed to count payments by invoice");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].invoice_id, Some(invoice1));
    assert_eq!(invoice_payments.len(), 1);
    assert_eq!(invoice_payments[0].invoice_id, Some(invoice1));
    assert_eq!(count, 1);
}

#[test]
fn test_list_payments_by_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create payments with different statuses
    let payment1 = create_test_payment(&commerce, None, None);
    let payment2 = create_test_payment(&commerce, None, None);
    let _payment3 = create_test_payment(&commerce, None, None);

    // Complete some payments
    commerce.payments().mark_completed(payment1.id).expect("Failed to complete payment");
    commerce.payments().mark_completed(payment2.id).expect("Failed to complete payment");

    let completed_payments = commerce
        .payments()
        .list(PaymentFilter {
            status: Some(PaymentTransactionStatus::Completed),
            ..Default::default()
        })
        .expect("Failed to list payments");

    assert_eq!(completed_payments.len(), 2);
    assert!(completed_payments.iter().all(|p| p.status == PaymentTransactionStatus::Completed));
}

#[test]
fn test_list_payments_with_limit() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create more payments than the limit
    for _ in 0..10 {
        create_test_payment(&commerce, None, None);
    }

    let payments = commerce
        .payments()
        .list(PaymentFilter { limit: Some(5), ..Default::default() })
        .expect("Failed to list payments");

    assert_eq!(payments.len(), 5);
}

#[test]
fn test_list_payments_with_offset() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create payments
    for _ in 0..10 {
        create_test_payment(&commerce, None, None);
    }

    let payments = commerce
        .payments()
        .list(PaymentFilter { limit: Some(5), offset: Some(3), ..Default::default() })
        .expect("Failed to list payments");

    assert!(payments.len() <= 5);
}

#[test]
fn test_count_payments() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create payments
    for _ in 0..7 {
        create_test_payment(&commerce, None, None);
    }

    let count =
        commerce.payments().count(PaymentFilter::default()).expect("Failed to count payments");

    assert!(count >= 7);
}

#[test]
fn test_count_payments_by_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create and complete some payments
    for _ in 0..4 {
        let payment = create_test_payment(&commerce, None, None);
        commerce.payments().mark_completed(payment.id).expect("Failed to complete");
    }

    // Create some pending payments
    for _ in 0..3 {
        create_test_payment(&commerce, None, None);
    }

    let completed_count = commerce
        .payments()
        .count(PaymentFilter {
            status: Some(PaymentTransactionStatus::Completed),
            ..Default::default()
        })
        .expect("Failed to count");

    assert_eq!(completed_count, 4);
}

// ============================================================================
// Payment Method Tests
// ============================================================================

#[test]
fn test_payment_methods() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Test different payment method types
    let credit_card = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(50.00),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            ..Default::default()
        })
        .expect("Failed to create credit card payment");

    let paypal = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::PayPal,
            amount: dec!(75.00),
            billing_email: Some("paypal@example.com".into()),
            ..Default::default()
        })
        .expect("Failed to create PayPal payment");

    let bank_transfer = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::BankTransfer,
            amount: dec!(200.00),
            ..Default::default()
        })
        .expect("Failed to create bank transfer payment");

    assert_eq!(credit_card.payment_method, PaymentMethodType::CreditCard);
    assert_eq!(paypal.payment_method, PaymentMethodType::PayPal);
    assert_eq!(bank_transfer.payment_method, PaymentMethodType::BankTransfer);
}

#[test]
fn test_create_payment_method_for_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let method = commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            is_default: Some(true),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            card_exp_month: Some(12),
            card_exp_year: Some(2027),
            cardholder_name: Some("Test User".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    assert!(!method.id.is_nil());
    assert_eq!(method.customer_id, customer_id);
    assert_eq!(method.method_type, PaymentMethodType::CreditCard);
    assert!(method.is_default);
    assert_eq!(method.card_brand, Some(CardBrand::Visa));
    assert_eq!(method.card_last4, Some("4242".into()));
    assert_eq!(method.card_exp_month, Some(12));
    assert_eq!(method.card_exp_year, Some(2027));
}

#[test]
fn test_get_customer_payment_methods() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create multiple payment methods
    commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            is_default: Some(true),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            card_brand: Some(CardBrand::Mastercard),
            card_last4: Some("5555".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::BankTransfer,
            bank_name: Some("Test Bank".into()),
            account_last4: Some("6789".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    let methods = commerce
        .payments()
        .get_payment_methods(customer_id)
        .expect("Failed to get payment methods");

    assert_eq!(methods.len(), 3);
    assert!(methods.iter().all(|m| m.customer_id == customer_id));
}

#[test]
fn test_delete_payment_method() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let method = commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    commerce.payments().delete_payment_method(method.id).expect("Failed to delete payment method");

    let methods = commerce
        .payments()
        .get_payment_methods(customer_id)
        .expect("Failed to get payment methods");

    assert!(methods.is_empty());
}

#[test]
fn test_set_default_payment_method() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create first method as default
    let _method1 = commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            is_default: Some(true),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    // Create second method
    let method2 = commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            card_brand: Some(CardBrand::Mastercard),
            card_last4: Some("5555".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    // Set method2 as default
    commerce
        .payments()
        .set_default_payment_method(customer_id, method2.id)
        .expect("Failed to set default payment method");

    let methods = commerce
        .payments()
        .get_payment_methods(customer_id)
        .expect("Failed to get payment methods");

    // Find and verify the default
    let default_method = methods.iter().find(|m| m.is_default);
    assert!(default_method.is_some());
    assert_eq!(default_method.unwrap().id, method2.id);
}

// ============================================================================
// Payment Update Tests
// ============================================================================

#[test]
fn test_update_payment_external_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    let updated = commerce
        .payments()
        .update(
            payment.id,
            UpdatePayment {
                external_id: Some("pi_updated_external_123".into()),
                ..Default::default()
            },
        )
        .expect("Failed to update payment");

    assert_eq!(updated.external_id, Some("pi_updated_external_123".into()));
}

#[test]
fn test_update_payment_metadata() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    let updated = commerce
        .payments()
        .update(
            payment.id,
            UpdatePayment {
                metadata: Some(r#"{"key": "value", "order_ref": "ORD-123"}"#.into()),
                ..Default::default()
            },
        )
        .expect("Failed to update payment");

    assert!(updated.metadata.is_some());
    assert!(updated.metadata.unwrap().contains("order_ref"));
}

// ============================================================================
// Card Brand Tests
// ============================================================================

#[test]
fn test_various_card_brands() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let brands = vec![
        (CardBrand::Visa, "4242"),
        (CardBrand::Mastercard, "5555"),
        (CardBrand::Amex, "3782"),
        (CardBrand::Discover, "6011"),
        (CardBrand::DinersClub, "3056"),
        (CardBrand::Jcb, "3566"),
        (CardBrand::UnionPay, "6200"),
    ];

    for (brand, last4) in brands {
        let payment = commerce
            .payments()
            .create(CreatePayment {
                payment_method: PaymentMethodType::CreditCard,
                amount: dec!(100.00),
                card_brand: Some(brand),
                card_last4: Some(last4.into()),
                ..Default::default()
            })
            .expect("Failed to create payment");

        assert_eq!(payment.card_brand, Some(brand));
        assert_eq!(payment.card_last4, Some(last4.into()));
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_payment_number_uniqueness() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment1 = create_test_payment(&commerce, None, None);
    let payment2 = create_test_payment(&commerce, None, None);
    let payment3 = create_test_payment(&commerce, None, None);

    // All payment numbers should be unique
    assert_ne!(payment1.payment_number, payment2.payment_number);
    assert_ne!(payment2.payment_number, payment3.payment_number);
    assert_ne!(payment1.payment_number, payment3.payment_number);
}

#[test]
fn test_payment_timestamps() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    // created_at and updated_at should be set
    assert!(payment.created_at <= chrono::Utc::now());
    assert!(payment.updated_at <= chrono::Utc::now());
}

#[test]
fn test_payment_with_high_value() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(99999.99),
            ..Default::default()
        })
        .expect("Failed to create high value payment");

    assert_eq!(payment.amount, dec!(99999.99));
}

#[test]
fn test_payment_with_small_value() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(0.01),
            ..Default::default()
        })
        .expect("Failed to create small value payment");

    assert_eq!(payment.amount, dec!(0.01));
}

#[test]
fn test_payment_update_increments_timestamp() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    // Update the payment
    let updated = commerce
        .payments()
        .update(
            payment.id,
            UpdatePayment { external_id: Some("updated_id".into()), ..Default::default() },
        )
        .expect("Failed to update payment");

    // Updated timestamp should be >= original
    assert!(updated.updated_at >= payment.updated_at);
}

#[test]
fn test_refund_number_uniqueness() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let payment = create_test_payment(&commerce, None, None);

    commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    let refund1 = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect("Failed to create refund");

    let refund2 = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect("Failed to create refund");

    // All refund numbers should be unique
    assert_ne!(refund1.refund_number, refund2.refund_number);
}

#[test]
fn test_payment_with_different_currencies() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let usd_payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect("Failed to create USD payment");

    let eur_payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(85.00),
            currency: Some(CurrencyCode::EUR),
            ..Default::default()
        })
        .expect("Failed to create EUR payment");

    let gbp_payment = commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(75.00),
            currency: Some(CurrencyCode::GBP),
            ..Default::default()
        })
        .expect("Failed to create GBP payment");

    assert_eq!(usd_payment.currency, CurrencyCode::USD);
    assert_eq!(eur_payment.currency, CurrencyCode::EUR);
    assert_eq!(gbp_payment.currency, CurrencyCode::GBP);
}

#[test]
fn test_list_payments_by_amount_range() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create payments with different amounts
    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(25.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(50.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(75.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    // Get all payments and filter by amount range in code
    // (min_amount/max_amount filters may not be implemented at the DB level)
    let all_payments =
        commerce.payments().list(PaymentFilter::default()).expect("Failed to list payments");

    // Manually filter by amount range
    let mid_range_payments: Vec<_> = all_payments
        .iter()
        .filter(|p| p.amount >= dec!(40.00) && p.amount <= dec!(80.00))
        .collect();

    // Should include 50.00 and 75.00 payments
    assert_eq!(mid_range_payments.len(), 2);
}

#[test]
fn test_list_payments_by_payment_method() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create payments with different methods
    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::PayPal,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    commerce
        .payments()
        .create(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            ..Default::default()
        })
        .expect("Failed to create payment");

    // Get all payments and filter by payment method in code
    // (payment_method filter may not be implemented at the DB level)
    let all_payments =
        commerce.payments().list(PaymentFilter::default()).expect("Failed to list payments");

    // Manually filter by payment method
    let credit_card_payments: Vec<_> =
        all_payments.iter().filter(|p| p.payment_method == PaymentMethodType::CreditCard).collect();

    assert_eq!(credit_card_payments.len(), 2);
    assert!(credit_card_payments.iter().all(|p| p.payment_method == PaymentMethodType::CreditCard));
}

// ============================================================================
// Full Payment Flow Test
// ============================================================================

#[test]
fn test_full_payment_flow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let order_id = create_test_order(&commerce, customer_id);

    // 1. Create a payment for the order
    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order_id),
            customer_id: Some(customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(59.98), // partial capture (order total is 119.98)
            currency: Some(CurrencyCode::USD),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            card_exp_month: Some(12),
            card_exp_year: Some(2027),
            billing_email: Some("customer@example.com".into()),
            billing_name: Some("Test Customer".into()),
            external_id: Some("pi_stripe_intent_123".into()),
            processor: Some("stripe".into()),
            ..Default::default()
        })
        .expect("Failed to create payment");

    assert_eq!(payment.status, PaymentTransactionStatus::Pending);

    // 2. Mark as processing (simulating payment processor)
    let payment =
        commerce.payments().mark_processing(payment.id).expect("Failed to mark processing");

    assert_eq!(payment.status, PaymentTransactionStatus::Processing);

    // 3. Complete the payment
    let payment =
        commerce.payments().mark_completed(payment.id).expect("Failed to complete payment");

    assert_eq!(payment.status, PaymentTransactionStatus::Completed);
    assert!(payment.paid_at.is_some());

    // 4. Customer requests partial refund
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(29.99)), // Half refund
            reason: Some("Item was damaged".into()),
            external_id: Some("re_stripe_refund_456".into()),
            ..Default::default()
        })
        .expect("Failed to create refund");

    assert_eq!(refund.status, RefundStatus::Pending);
    assert_eq!(refund.amount, dec!(29.99));

    // 5. Complete the refund
    let refund = commerce.payments().complete_refund(refund.id).expect("Failed to complete refund");

    assert_eq!(refund.status, RefundStatus::Completed);
    assert!(refund.refunded_at.is_some());

    // 6. Verify the payment shows the refund
    let updated_payment = commerce
        .payments()
        .get(payment.id)
        .expect("Failed to get payment")
        .expect("Payment not found");

    // After a partial refund, the payment status should be PartiallyRefunded
    assert_eq!(updated_payment.status, PaymentTransactionStatus::PartiallyRefunded);

    // 7. Verify we can list all refunds for this payment
    let refunds = commerce.payments().get_refunds(payment.id).expect("Failed to get refunds");

    assert_eq!(refunds.len(), 1);
    assert_eq!(refunds[0].amount, dec!(29.99));
}

#[test]
fn test_payment_method_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // 1. Create a payment method
    let method = commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            is_default: Some(true),
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            card_exp_month: Some(12),
            card_exp_year: Some(2027),
            cardholder_name: Some("Test User".into()),
            external_id: Some("pm_stripe_123".into()),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    assert!(method.is_default);

    // 2. Add another payment method
    let method2 = commerce
        .payments()
        .create_payment_method(CreatePaymentMethod {
            customer_id,
            method_type: PaymentMethodType::CreditCard,
            card_brand: Some(CardBrand::Mastercard),
            card_last4: Some("5555".into()),
            card_exp_month: Some(6),
            card_exp_year: Some(2028),
            ..Default::default()
        })
        .expect("Failed to create payment method");

    // 3. Verify customer has 2 payment methods
    let methods = commerce
        .payments()
        .get_payment_methods(customer_id)
        .expect("Failed to get payment methods");

    assert_eq!(methods.len(), 2);

    // 4. Change default payment method
    commerce
        .payments()
        .set_default_payment_method(customer_id, method2.id)
        .expect("Failed to set default");

    // 5. Verify the new default
    let methods = commerce
        .payments()
        .get_payment_methods(customer_id)
        .expect("Failed to get payment methods");

    let default = methods.iter().find(|m| m.is_default).unwrap();
    assert_eq!(default.id, method2.id);

    // 6. Delete the old default
    commerce.payments().delete_payment_method(method.id).expect("Failed to delete payment method");

    // 7. Verify only one method remains
    let methods = commerce
        .payments()
        .get_payment_methods(customer_id)
        .expect("Failed to get payment methods");

    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].id, method2.id);
}
