//! Property-based tests for stateset-core models
//!
//! Run with: cargo test --test `proptest_models`

use proptest::prelude::*;
use rust_decimal::Decimal;
use uuid::Uuid;

use stateset_core::models::{CartStatus, OrderStatus, PaymentStatus};

// ============================================================================
// Decimal Arithmetic Properties
// ============================================================================

proptest! {
    /// Order total calculation: subtotal + tax - discount should equal total
    #[test]
    fn order_total_calculation_is_correct(
        subtotal_cents in 0i64..1_000_000,
        tax_rate_pct in 0i64..30,      // 0-30%
        discount_cents in 0i64..100_000,
    ) {
        let subtotal = Decimal::new(subtotal_cents, 2);
        let tax_rate = Decimal::new(tax_rate_pct, 2);
        // Cap discount at subtotal
        let discount = Decimal::new(discount_cents.min(subtotal_cents), 2);

        let tax = (subtotal * tax_rate).round_dp(2);
        let total = subtotal + tax - discount;

        // Total should never be negative
        prop_assert!(total >= Decimal::ZERO || discount > subtotal + tax);

        // Total should equal the sum of its parts
        prop_assert_eq!(total, subtotal + tax - discount);
    }

    /// Quantity * unit_price should equal line total (before tax/discount)
    #[test]
    fn line_item_subtotal_calculation(
        quantity in 1i32..1000,
        unit_price_cents in 1i64..100_000,
    ) {
        let unit_price = Decimal::new(unit_price_cents, 2);
        let qty_decimal = Decimal::from(quantity);

        let line_subtotal = unit_price * qty_decimal;

        // Line subtotal should be positive
        prop_assert!(line_subtotal > Decimal::ZERO);

        // Should be commutative
        prop_assert_eq!(line_subtotal, qty_decimal * unit_price);
    }

    /// Tax calculation should round to 2 decimal places
    #[test]
    fn tax_calculation_rounds_correctly(
        amount_cents in 1i64..10_000_000,
        tax_rate_bps in 0i64..5000, // 0-50% in basis points
    ) {
        let amount = Decimal::new(amount_cents, 2);
        let tax_rate = Decimal::new(tax_rate_bps, 4); // basis points to decimal

        let tax = (amount * tax_rate).round_dp(2);

        // Tax should have at most 2 decimal places
        prop_assert!(tax.scale() <= 2);

        // Tax should be non-negative
        prop_assert!(tax >= Decimal::ZERO);
    }

    /// Discount should never exceed subtotal
    #[test]
    fn discount_capped_at_subtotal(
        subtotal_cents in 1i64..1_000_000,
        discount_percent in 0i64..100,
    ) {
        let subtotal = Decimal::new(subtotal_cents, 2);
        let discount_rate = Decimal::new(discount_percent, 2);

        let discount = (subtotal * discount_rate).round_dp(2);

        // Discount should not exceed subtotal
        prop_assert!(discount <= subtotal);
    }
}

// ============================================================================
// Inventory Quantity Properties
// ============================================================================

proptest! {
    /// Inventory quantity after allocation should be non-negative
    #[test]
    fn inventory_allocation_non_negative(
        available in 0i32..10_000,
        requested in 0i32..1000,
    ) {
        let allocated = requested.min(available);
        let remaining = available - allocated;

        // Remaining should never be negative
        prop_assert!(remaining >= 0);

        // Allocated should not exceed available
        prop_assert!(allocated <= available);

        // Allocated should not exceed requested
        prop_assert!(allocated <= requested);
    }

    /// Reorder point calculation
    #[test]
    fn reorder_point_calculation(
        daily_sales in 1i32..1000,
        lead_time_days in 1i32..60,
        safety_stock in 0i32..500,
    ) {
        let reorder_point = (daily_sales * lead_time_days) + safety_stock;

        // Reorder point should be at least safety stock
        prop_assert!(reorder_point >= safety_stock);

        // Reorder point should be positive
        prop_assert!(reorder_point > 0);
    }
}

// ============================================================================
// Status Transition Properties
// ============================================================================

fn valid_order_status_transitions(from: OrderStatus) -> Vec<OrderStatus> {
    match from {
        OrderStatus::Pending => vec![OrderStatus::Confirmed, OrderStatus::Cancelled],
        OrderStatus::Confirmed => vec![OrderStatus::Processing, OrderStatus::Cancelled],
        OrderStatus::Processing => vec![OrderStatus::Shipped, OrderStatus::Cancelled],
        OrderStatus::Shipped => vec![OrderStatus::Delivered],
        OrderStatus::Delivered => vec![OrderStatus::Refunded],
        OrderStatus::Cancelled => vec![],
        OrderStatus::Refunded => vec![],
        _ => vec![],
    }
}

proptest! {
    /// Order status transitions should only go to valid next states
    #[test]
    fn order_status_transition_validity(
        from_idx in 0usize..7,
        to_idx in 0usize..7,
    ) {
        let statuses = [
            OrderStatus::Pending,
            OrderStatus::Confirmed,
            OrderStatus::Processing,
            OrderStatus::Shipped,
            OrderStatus::Delivered,
            OrderStatus::Cancelled,
            OrderStatus::Refunded,
        ];

        let from = statuses[from_idx];
        let to = statuses[to_idx];
        let valid_transitions = valid_order_status_transitions(from);

        let is_valid = from.can_transition_to(to);

        if from == to {
            prop_assert!(is_valid);
        } else {
            prop_assert_eq!(is_valid, valid_transitions.contains(&to));
        }
    }
}

// ============================================================================
// Serialization Roundtrip Properties
// ============================================================================

fn arb_order_status() -> impl Strategy<Value = OrderStatus> {
    prop_oneof![
        Just(OrderStatus::Pending),
        Just(OrderStatus::Confirmed),
        Just(OrderStatus::Processing),
        Just(OrderStatus::Shipped),
        Just(OrderStatus::Delivered),
        Just(OrderStatus::Cancelled),
        Just(OrderStatus::Refunded),
    ]
}

fn arb_payment_status() -> impl Strategy<Value = PaymentStatus> {
    prop_oneof![
        Just(PaymentStatus::Pending),
        Just(PaymentStatus::Authorized),
        Just(PaymentStatus::Paid),
        Just(PaymentStatus::PartiallyPaid),
        Just(PaymentStatus::Refunded),
        Just(PaymentStatus::PartiallyRefunded),
        Just(PaymentStatus::Failed),
    ]
}

fn arb_cart_status() -> impl Strategy<Value = CartStatus> {
    prop_oneof![
        Just(CartStatus::Active),
        Just(CartStatus::ReadyForPayment),
        Just(CartStatus::PaymentPending),
        Just(CartStatus::Completed),
        Just(CartStatus::Abandoned),
        Just(CartStatus::Cancelled),
        Just(CartStatus::Expired),
    ]
}

proptest! {
    /// OrderStatus serialization roundtrip
    #[test]
    fn order_status_serde_roundtrip(status in arb_order_status()) {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: OrderStatus = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(status, deserialized);
    }

    /// PaymentStatus serialization roundtrip
    #[test]
    fn payment_status_serde_roundtrip(status in arb_payment_status()) {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: PaymentStatus = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(status, deserialized);
    }

    /// CartStatus serialization roundtrip
    #[test]
    fn cart_status_serde_roundtrip(status in arb_cart_status()) {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: CartStatus = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(status, deserialized);
    }

    /// String serialization should produce valid JSON
    #[test]
    fn string_fields_produce_valid_json(
        s in "[a-zA-Z0-9 ]{1,100}"
    ) {
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: String = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(s, deserialized);
    }

    /// UUID serialization roundtrip
    #[test]
    fn uuid_serde_roundtrip(_seed in any::<u64>()) {
        let uuid = Uuid::new_v4();
        let json = serde_json::to_string(&uuid).unwrap();
        let deserialized: Uuid = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(uuid, deserialized);
    }

    /// Decimal serialization roundtrip
    #[test]
    fn decimal_serde_roundtrip(
        mantissa in -999_999_999i64..999_999_999i64,
        scale in 0u32..4,
    ) {
        let decimal = Decimal::new(mantissa, scale);
        let json = serde_json::to_string(&decimal).unwrap();
        let deserialized: Decimal = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(decimal, deserialized);
    }
}

// ============================================================================
// Currency Conversion Properties
// ============================================================================

proptest! {
    /// Currency conversion should be reversible (within rounding tolerance)
    #[test]
    fn currency_conversion_reversible(
        amount_cents in 100i64..10_000_000,
        rate_bps in 5000i64..200_000, // 0.5x to 20x exchange rate
    ) {
        let amount = Decimal::new(amount_cents, 2);
        let rate = Decimal::new(rate_bps, 4);
        let inverse_rate = Decimal::ONE / rate;

        // Convert to foreign currency
        let converted = (amount * rate).round_dp(2);

        // Convert back
        let back = (converted * inverse_rate).round_dp(2);

        // Should be within 1 cent due to rounding
        let diff = (amount - back).abs();
        prop_assert!(diff <= Decimal::new(1, 2),
            "Conversion roundtrip error too large: {} -> {} -> {}, diff = {}",
            amount, converted, back, diff);
    }

    /// Exchange rate multiplication should be associative for multi-currency
    #[test]
    fn exchange_rate_associativity(
        amount_cents in 100i64..1_000_000,
        rate1_bps in 8000i64..12000, // ~0.8x to 1.2x
        rate2_bps in 8000i64..12000,
    ) {
        let amount = Decimal::new(amount_cents, 2);
        let rate1 = Decimal::new(rate1_bps, 4);
        let rate2 = Decimal::new(rate2_bps, 4);

        // (amount * rate1) * rate2
        let path1 = ((amount * rate1).round_dp(4) * rate2).round_dp(2);

        // amount * (rate1 * rate2)
        let combined_rate = (rate1 * rate2).round_dp(8);
        let path2 = (amount * combined_rate).round_dp(2);

        // Should be within 1 cent
        let diff = (path1 - path2).abs();
        prop_assert!(diff <= Decimal::new(1, 2),
            "Associativity error: path1={}, path2={}, diff={}",
            path1, path2, diff);
    }
}

// ============================================================================
// Order Number Generation Properties
// ============================================================================

proptest! {
    /// Order numbers should be unique for different timestamps
    #[test]
    fn order_number_uniqueness(
        year in 2020i32..2030,
        seq1 in 1u32..999999,
        seq2 in 1u32..999999,
    ) {
        let order_num1 = format!("ORD-{}-{:06}", year, seq1);
        let order_num2 = format!("ORD-{}-{:06}", year, seq2);

        if seq1 != seq2 {
            prop_assert_ne!(order_num1, order_num2);
        } else {
            prop_assert_eq!(order_num1, order_num2);
        }
    }

    /// Order numbers should have consistent format
    #[test]
    fn order_number_format(
        year in 2020i32..2030,
        seq in 1u32..999999,
    ) {
        let order_num = format!("ORD-{}-{:06}", year, seq);

        // Should start with ORD-
        prop_assert!(order_num.starts_with("ORD-"));

        // Should have consistent length
        prop_assert_eq!(order_num.len(), 15); // ORD-YYYY-NNNNNN

        // Should be parseable
        let parts: Vec<&str> = order_num.split('-').collect();
        prop_assert_eq!(parts.len(), 3);
        prop_assert_eq!(parts[0], "ORD");
    }
}

// ============================================================================
// Money Type Properties
// ============================================================================

use stateset_primitives::{CurrencyCode, Money};

proptest! {
    #[test]
    fn money_add_rejects_currency_mismatch(
        a_cents in -999_999i64..999_999,
        b_cents in -999_999i64..999_999,
    ) {
        let usd = Money::new(Decimal::new(a_cents, 2), CurrencyCode::USD);
        let eur = Money::new(Decimal::new(b_cents, 2), CurrencyCode::EUR);
        prop_assert!(usd.checked_add(eur).is_none());
    }

    #[test]
    fn money_add_commutative(
        a_cents in -999_999i64..999_999,
        b_cents in -999_999i64..999_999,
    ) {
        let a = Money::new(Decimal::new(a_cents, 2), CurrencyCode::USD);
        let b = Money::new(Decimal::new(b_cents, 2), CurrencyCode::USD);
        let ab = a.checked_add(b).unwrap();
        let ba = b.checked_add(a).unwrap();
        prop_assert_eq!(ab.amount(), ba.amount());
        prop_assert_eq!(ab.currency(), CurrencyCode::USD);
    }

    #[test]
    fn money_zero_is_identity(amount_cents in -999_999i64..999_999) {
        let m = Money::new(Decimal::new(amount_cents, 2), CurrencyCode::USD);
        let zero = Money::zero(CurrencyCode::USD);
        let result = m.checked_add(zero).unwrap();
        prop_assert_eq!(result.amount(), m.amount());
    }

    #[test]
    fn money_negate_is_self_inverse(amount_cents in -999_999i64..999_999) {
        let m = Money::new(Decimal::new(amount_cents, 2), CurrencyCode::USD);
        let double_negated = m.negate().negate();
        prop_assert_eq!(double_negated.amount(), m.amount());
    }

    #[test]
    fn money_abs_always_non_negative(amount_cents in -999_999i64..999_999) {
        let m = Money::new(Decimal::new(amount_cents, 2), CurrencyCode::EUR);
        prop_assert!(m.abs().amount() >= Decimal::ZERO);
    }

    #[test]
    fn money_div_zero_returns_none(amount_cents in -999_999i64..999_999) {
        let m = Money::new(Decimal::new(amount_cents, 2), CurrencyCode::USD);
        prop_assert!(m.checked_div_scalar(Decimal::ZERO).is_none());
    }

    #[test]
    fn money_serde_roundtrip(amount_cents in -999_999i64..999_999) {
        let m = Money::new(Decimal::new(amount_cents, 2), CurrencyCode::USD);
        let json = serde_json::to_string(&m).unwrap();
        let deserialized: Money = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(m, deserialized);
    }
}

// ============================================================================
// Return Status Transition Properties
// ============================================================================

use stateset_core::models::ReturnStatus;

fn valid_return_transitions(from: ReturnStatus) -> Vec<ReturnStatus> {
    match from {
        ReturnStatus::Requested => {
            vec![ReturnStatus::Approved, ReturnStatus::Rejected, ReturnStatus::Cancelled]
        }
        ReturnStatus::Approved => vec![ReturnStatus::InTransit, ReturnStatus::Cancelled],
        ReturnStatus::InTransit => vec![ReturnStatus::Received],
        ReturnStatus::Received => vec![ReturnStatus::Inspecting],
        ReturnStatus::Inspecting => vec![ReturnStatus::Completed, ReturnStatus::Rejected],
        ReturnStatus::Rejected | ReturnStatus::Completed | ReturnStatus::Cancelled => vec![],
        _ => vec![],
    }
}

proptest! {
    #[test]
    fn return_status_transition_validity(from_idx in 0usize..8, to_idx in 0usize..8) {
        let statuses = [
            ReturnStatus::Requested, ReturnStatus::Approved, ReturnStatus::Rejected,
            ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting,
            ReturnStatus::Completed, ReturnStatus::Cancelled,
        ];
        let from = statuses[from_idx];
        let to = statuses[to_idx];
        if from == to {
            prop_assert!(from.can_transition_to(to));
        } else {
            prop_assert_eq!(from.can_transition_to(to), valid_return_transitions(from).contains(&to));
        }
    }
}

// ============================================================================
// Subscription Status Transition Properties
// ============================================================================

use stateset_core::models::SubscriptionStatus;

fn valid_subscription_transitions(from: SubscriptionStatus) -> Vec<SubscriptionStatus> {
    match from {
        SubscriptionStatus::Pending => vec![
            SubscriptionStatus::Trial,
            SubscriptionStatus::Active,
            SubscriptionStatus::Cancelled,
        ],
        SubscriptionStatus::Trial => vec![
            SubscriptionStatus::Active,
            SubscriptionStatus::Cancelled,
            SubscriptionStatus::Expired,
        ],
        SubscriptionStatus::Active => vec![
            SubscriptionStatus::Paused,
            SubscriptionStatus::PastDue,
            SubscriptionStatus::Cancelled,
            SubscriptionStatus::Expired,
        ],
        SubscriptionStatus::Paused => {
            vec![SubscriptionStatus::Active, SubscriptionStatus::Cancelled]
        }
        SubscriptionStatus::PastDue => vec![
            SubscriptionStatus::Active,
            SubscriptionStatus::Cancelled,
            SubscriptionStatus::Expired,
        ],
        SubscriptionStatus::Cancelled | SubscriptionStatus::Expired => vec![],
        _ => vec![],
    }
}

proptest! {
    #[test]
    fn subscription_status_transition_validity(from_idx in 0usize..7, to_idx in 0usize..7) {
        let statuses = [
            SubscriptionStatus::Pending, SubscriptionStatus::Trial, SubscriptionStatus::Active,
            SubscriptionStatus::Paused, SubscriptionStatus::PastDue,
            SubscriptionStatus::Cancelled, SubscriptionStatus::Expired,
        ];
        let from = statuses[from_idx];
        let to = statuses[to_idx];
        if from == to {
            prop_assert!(from.can_transition_to(to));
        } else {
            prop_assert_eq!(from.can_transition_to(to), valid_subscription_transitions(from).contains(&to));
        }
    }

    #[test]
    fn subscription_status_serde_roundtrip(idx in 0usize..7) {
        let statuses = [
            SubscriptionStatus::Pending, SubscriptionStatus::Trial,
            SubscriptionStatus::Active, SubscriptionStatus::Paused,
            SubscriptionStatus::PastDue, SubscriptionStatus::Cancelled,
            SubscriptionStatus::Expired,
        ];
        let status = statuses[idx];
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: SubscriptionStatus = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(status, deserialized);
    }

    #[test]
    fn return_status_serde_roundtrip(idx in 0usize..8) {
        let statuses = [
            ReturnStatus::Requested, ReturnStatus::Approved, ReturnStatus::Rejected,
            ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting,
            ReturnStatus::Completed, ReturnStatus::Cancelled,
        ];
        let status = statuses[idx];
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ReturnStatus = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(status, deserialized);
    }

    #[test]
    fn currency_code_roundtrip(idx in 0usize..8) {
        let codes = ["USD", "EUR", "GBP", "JPY", "CAD", "AUD", "CHF", "CNY"];
        let code_str = codes[idx];
        let parsed: CurrencyCode = code_str.parse().unwrap();
        prop_assert_eq!(parsed.as_str(), code_str);
    }

    #[test]
    fn invalid_currency_code_rejected(s in "[a-z]{1,2}|[A-Z]{4,}|[0-9]{3}") {
        let result: Result<CurrencyCode, _> = s.parse();
        prop_assert!(result.is_err());
    }

    #[test]
    fn line_totals_sum_to_subtotal(
        qty1 in 1i32..100, price1_cents in 1i64..100_000,
        qty2 in 1i32..100, price2_cents in 1i64..100_000,
    ) {
        let p1 = Decimal::new(price1_cents, 2);
        let p2 = Decimal::new(price2_cents, 2);
        let line1 = p1 * Decimal::from(qty1);
        let line2 = p2 * Decimal::from(qty2);
        let subtotal = line1 + line2;
        prop_assert!(subtotal > Decimal::ZERO);
        prop_assert!(subtotal >= line1);
        prop_assert!(subtotal >= line2);
    }
}

// ============================================================================
// Event Serde Roundtrip Tests (non-proptest, but important coverage)
// ============================================================================

#[cfg(test)]
mod event_roundtrip_tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use stateset_core::events::CommerceEvent;
    use stateset_primitives::*;

    macro_rules! roundtrip {
        ($name:ident, $event:expr) => {
            #[test]
            fn $name() {
                let event = $event;
                let json = serde_json::to_string(&event).unwrap();
                let back: CommerceEvent = serde_json::from_str(&json).unwrap();
                assert_eq!(event.event_type(), back.event_type());
                assert_eq!(event.timestamp(), back.timestamp());
            }
        };
    }

    roundtrip!(
        cart_created,
        CommerceEvent::CartCreated {
            cart_id: CartId::new(),
            customer_id: Some(CustomerId::new()),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        cart_item_added,
        CommerceEvent::CartItemAdded {
            cart_id: CartId::new(),
            product_id: ProductId::new(),
            sku: "SKU-001".into(),
            quantity: 2,
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        payment_created,
        CommerceEvent::PaymentCreated {
            payment_id: PaymentId::new(),
            order_id: OrderId::new(),
            amount: dec!(99.99),
            currency: CurrencyCode::USD,
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        payment_completed,
        CommerceEvent::PaymentCompleted {
            payment_id: PaymentId::new(),
            order_id: OrderId::new(),
            amount: dec!(99.99),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        shipment_created,
        CommerceEvent::ShipmentCreated {
            shipment_id: ShipmentId::new(),
            order_id: OrderId::new(),
            carrier: "USPS".into(),
            tracking_number: Some("1Z999".into()),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        shipment_delivered,
        CommerceEvent::ShipmentDelivered {
            shipment_id: ShipmentId::new(),
            order_id: OrderId::new(),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        invoice_created,
        CommerceEvent::InvoiceCreated {
            invoice_id: InvoiceId::new(),
            customer_id: CustomerId::new(),
            total_amount: dec!(500.00),
            currency: CurrencyCode::EUR,
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        subscription_created,
        CommerceEvent::SubscriptionCreated {
            subscription_id: SubscriptionId::new(),
            customer_id: CustomerId::new(),
            plan_name: "Pro".into(),
            price: dec!(29.99),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        subscription_cancelled,
        CommerceEvent::SubscriptionCancelled {
            subscription_id: SubscriptionId::new(),
            reason: Some("Too expensive".into()),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        subscription_renewed,
        CommerceEvent::SubscriptionRenewed {
            subscription_id: SubscriptionId::new(),
            billing_amount: dec!(29.99),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        gift_card_created,
        CommerceEvent::GiftCardCreated {
            gift_card_id: GiftCardId::new(),
            initial_balance: dec!(100.00),
            currency: CurrencyCode::USD,
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        gift_card_redeemed,
        CommerceEvent::GiftCardRedeemed {
            gift_card_id: GiftCardId::new(),
            amount: dec!(25.00),
            order_id: Some(OrderId::new()),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        store_credit_issued,
        CommerceEvent::StoreCreditIssued {
            store_credit_id: StoreCreditId::new(),
            customer_id: CustomerId::new(),
            amount: dec!(15.00),
            reason: "Return refund".into(),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        store_credit_applied,
        CommerceEvent::StoreCreditApplied {
            store_credit_id: StoreCreditId::new(),
            order_id: OrderId::new(),
            amount: dec!(15.00),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        loyalty_points_earned,
        CommerceEvent::LoyaltyPointsEarned {
            program_id: LoyaltyProgramId::new(),
            customer_id: CustomerId::new(),
            points: 500,
            source: "purchase".into(),
            timestamp: Utc::now()
        }
    );
    roundtrip!(
        loyalty_points_redeemed,
        CommerceEvent::LoyaltyPointsRedeemed {
            program_id: LoyaltyProgramId::new(),
            customer_id: CustomerId::new(),
            points: 200,
            timestamp: Utc::now()
        }
    );
}
