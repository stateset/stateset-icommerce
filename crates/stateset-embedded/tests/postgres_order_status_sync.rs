//! Postgres twin of `order_status_sync_test.rs`: an order's `payment_status`
//! / `fulfillment_status` follow its payment ledger and its shipments.
//!
//! Skipped (passes vacuously) without `POSTGRES_URL` / `DATABASE_URL`.

#[cfg(feature = "postgres")]
mod pg {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AddCartItem, CartAddress, CreateCart, CreateCustomer, CreatePayment, CreateRefund,
        FulfillmentStatus, Order, OrderStatus, PaymentMethodType, PaymentStatus, SetCartPayment,
        ShipmentLineInput,
    };
    use stateset_embedded::AsyncCommerce;
    use uuid::Uuid;

    fn postgres_url() -> Option<String> {
        std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
    }

    async fn connect() -> Option<AsyncCommerce> {
        let Some(url) = postgres_url() else {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres order status sync");
            return None;
        };
        Some(AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations"))
    }

    fn address() -> CartAddress {
        CartAddress {
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            company: None,
            line1: "1 Analytical Way".into(),
            line2: None,
            city: "San Francisco".into(),
            state: Some("CA".into()),
            postal_code: "94102".into(),
            country: "US".into(),
            phone: None,
            email: Some("ada@example.com".into()),
        }
    }

    async fn get(commerce: &AsyncCommerce, order: &Order) -> Order {
        commerce.orders().get(order.id.into_uuid()).await.expect("get order").expect("order exists")
    }

    async fn checkout(commerce: &AsyncCommerce) -> Order {
        let unique = Uuid::new_v4().simple().to_string();
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: format!("sync-{unique}@example.com"),
                first_name: "Ada".into(),
                last_name: "Lovelace".into(),
                ..Default::default()
            })
            .await
            .expect("create customer");
        let cart = commerce
            .carts()
            .create(CreateCart {
                customer_id: Some(customer.id),
                customer_email: Some(customer.email.clone()),
                customer_name: Some("Ada Lovelace".into()),
                ..Default::default()
            })
            .await
            .expect("create cart");
        for (sku, qty, price) in [
            (format!("SYNC-A-{unique}"), 2, dec!(29.99)),
            (format!("SYNC-B-{unique}"), 1, dec!(10.03)),
        ] {
            commerce
                .carts()
                .add_item(
                    cart.id.into_uuid(),
                    AddCartItem {
                        sku: sku.clone(),
                        name: sku,
                        quantity: qty,
                        unit_price: price,
                        ..Default::default()
                    },
                )
                .await
                .expect("add item");
        }
        commerce
            .carts()
            .set_shipping_address(cart.id.into_uuid(), address())
            .await
            .expect("shipping address");
        commerce
            .carts()
            .set_payment(
                cart.id.into_uuid(),
                SetCartPayment {
                    payment_method: "credit_card".into(),
                    payment_token: Some("tok_test".into()),
                    billing_address: None,
                },
            )
            .await
            .expect("set payment");
        let result = commerce.carts().complete(cart.id.into_uuid()).await.expect("checkout");
        let order = commerce
            .orders()
            .get(result.order_id.into_uuid())
            .await
            .expect("get order")
            .expect("order exists");
        assert_eq!(order.payment_status, PaymentStatus::Pending);
        assert_eq!(order.fulfillment_status, FulfillmentStatus::Unfulfilled);
        assert!(order.total_amount > Decimal::ZERO);
        order
    }

    async fn pay(commerce: &AsyncCommerce, order: &Order, amount: Decimal) -> Uuid {
        let payment = commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(order.id),
                customer_id: Some(order.customer_id),
                payment_method: PaymentMethodType::CreditCard,
                amount,
                currency: Some(order.currency),
                ..Default::default()
            })
            .await
            .expect("create payment");
        let id = payment.id.into_uuid();
        commerce.payments().mark_completed(id).await.expect("complete payment");
        id
    }

    async fn refund(commerce: &AsyncCommerce, payment_id: Uuid, amount: Decimal) {
        let refund = commerce
            .payments()
            .create_refund(CreateRefund {
                payment_id: payment_id.into(),
                amount: Some(amount),
                reason: Some("customer return".into()),
                ..Default::default()
            })
            .await
            .expect("create refund");
        commerce.payments().complete_refund(refund.id).await.expect("complete refund");
    }

    #[tokio::test]
    async fn postgres_paid_shipped_then_refunded_order_reports_each_state() {
        let Some(commerce) = connect().await else { return };
        let order = checkout(&commerce).await;

        let payment_id = pay(&commerce, &order, order.total_amount).await;
        let paid = get(&commerce, &order).await;
        assert_eq!(paid.payment_status, PaymentStatus::Paid);
        assert!(paid.version > order.version, "the status write bumps the order version");

        let shipped =
            commerce.orders().ship(order.id.into_uuid(), Some("1Z999")).await.expect("ship");
        assert_eq!(shipped.status, OrderStatus::Shipped);
        assert_eq!(shipped.fulfillment_status, FulfillmentStatus::Shipped);
        assert_eq!(shipped.payment_status, PaymentStatus::Paid);

        refund(&commerce, payment_id, dec!(10.03)).await;
        let partially = get(&commerce, &order).await;
        assert_eq!(partially.payment_status, PaymentStatus::PartiallyRefunded);
        assert_eq!(partially.fulfillment_status, FulfillmentStatus::Shipped);

        refund(&commerce, payment_id, order.total_amount - dec!(10.03)).await;
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::Refunded);

        let delivered = commerce.orders().deliver(order.id.into_uuid()).await.expect("deliver");
        assert_eq!(delivered.fulfillment_status, FulfillmentStatus::Delivered);
        assert_eq!(delivered.payment_status, PaymentStatus::Refunded);
    }

    #[tokio::test]
    async fn postgres_partial_payment_then_top_up_is_partially_paid_then_paid() {
        let Some(commerce) = connect().await else { return };
        let order = checkout(&commerce).await;

        pay(&commerce, &order, order.total_amount - dec!(0.01)).await;
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::PartiallyPaid);

        pay(&commerce, &order, dec!(0.01)).await;
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::Paid);
    }

    #[tokio::test]
    async fn postgres_processing_payment_authorizes_and_failed_payment_fails_the_order() {
        let Some(commerce) = connect().await else { return };
        let order = checkout(&commerce).await;

        let payment = commerce
            .payments()
            .create(CreatePayment {
                order_id: Some(order.id),
                payment_method: PaymentMethodType::CreditCard,
                amount: order.total_amount,
                currency: Some(order.currency),
                ..Default::default()
            })
            .await
            .expect("create payment");
        let id = payment.id.into_uuid();
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::Pending);

        commerce.payments().mark_processing(id).await.expect("processing");
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::Authorized);

        commerce.payments().mark_failed(id, "card declined", None).await.expect("fail");
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::Failed);

        pay(&commerce, &order, order.total_amount).await;
        assert_eq!(get(&commerce, &order).await.payment_status, PaymentStatus::Paid);
    }

    #[tokio::test]
    async fn postgres_partial_shipment_is_partially_fulfilled() {
        let Some(commerce) = connect().await else { return };
        let order = checkout(&commerce).await;

        let line = order.items.iter().find(|item| item.quantity == 2).expect("two-unit line");
        let partial = commerce
            .orders()
            .ship_lines(
                order.id.into_uuid(),
                None,
                Some(vec![ShipmentLineInput { order_item_id: line.id, quantity: 1 }]),
            )
            .await
            .expect("partial ship");
        assert_eq!(partial.status, OrderStatus::PartiallyShipped);
        assert_eq!(partial.fulfillment_status, FulfillmentStatus::PartiallyFulfilled);

        let rest = commerce.orders().ship(order.id.into_uuid(), None).await.expect("ship the rest");
        assert_eq!(rest.status, OrderStatus::Shipped);
        assert_eq!(rest.fulfillment_status, FulfillmentStatus::Shipped);
    }

    /// Twin of `refused_ship_leaves_a_confirmed_order_untouched`: a refused
    /// ship must not advance a confirmed order to `processing`.
    #[tokio::test]
    async fn postgres_refused_ship_leaves_a_confirmed_order_untouched() {
        let Some(commerce) = connect().await else { return };
        let order = checkout(&commerce).await;
        let before = get(&commerce, &order).await;
        assert_eq!(before.status, OrderStatus::Confirmed);

        let line = order.items.iter().find(|item| item.quantity == 2).expect("two-unit line");
        let err = commerce
            .orders()
            .ship_lines(
                order.id.into_uuid(),
                None,
                Some(vec![ShipmentLineInput { order_item_id: line.id, quantity: 3 }]),
            )
            .await
            .expect_err("3 of 2 units must be refused");
        assert!(
            matches!(err, stateset_core::CommerceError::ShipmentExceedsOrdered { .. }),
            "{err:?}"
        );
        let after = get(&commerce, &order).await;
        assert_eq!(after.status, OrderStatus::Confirmed, "a refused ship moved the order");
        assert_eq!(after.version, before.version, "a refused ship bumped the order version");

        let partial = commerce
            .orders()
            .ship_lines(
                order.id.into_uuid(),
                None,
                Some(vec![ShipmentLineInput { order_item_id: line.id, quantity: 1 }]),
            )
            .await
            .expect("a valid ship of a confirmed order succeeds");
        assert_eq!(partial.status, OrderStatus::PartiallyShipped);
    }
}
