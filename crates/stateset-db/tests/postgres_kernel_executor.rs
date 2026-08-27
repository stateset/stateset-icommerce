#![cfg(feature = "postgres")]

//! Live PostgreSQL proofs for the governed AI-commerce command boundary.

use rust_decimal_macros::dec;
use stateset_core::{
    A2ADisputeResolutionType, AccountType, AddCartItem, BillingCycleFilter, BillingCycleStatus,
    BillingInterval, CartAddress, CartStatus, ChargeSubscription, CommandEnvelope, CommitCheckout,
    ConfirmInventoryReservation, CreateA2AEscrow, CreateCart, CreateCustomer, CreateGlAccount,
    CreateGlPeriod, CreateInventoryItem, CreateJournalEntry, CreateJournalEntryLine, CreateOrder,
    CreateOrderItem, CreatePayment, CreateProduct, CreateProductVariant, CreateRefund,
    CreateReturn, CreateReturnItem, CreateSubscription, CreateSubscriptionPlan,
    CreateX402PaymentIntent, CurrencyCode, DisputeA2AEscrow, ExecutionMode, ExecutionStatus,
    FileA2ADispute, FundA2AEscrow, JournalEntryStatus, KernelCommandPolicy, KernelPolicy,
    KernelPrincipal, OrderStatus, PaymentMethodType, PaymentStatus, PostJournalEntry,
    PrincipalKind, ProductId, RefundA2AEscrow, ReleaseA2AEscrow, ReleaseInventoryReservation,
    ReservationStatus, ReserveInventory, ResolveA2ADispute, ReturnStatus, SetCartPayment,
    SettleX402Intent, ShipOrderCommand, ShipmentLineInput, SubmitA2ADisputeEvidence,
    TransitionOrder, TransitionReturn, UpdateOrder, X402Asset, X402IntentStatus, X402Network,
};
use stateset_db::PostgresDatabase;
use std::{env, sync::Arc};
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

fn policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("products.create", KernelCommandPolicy::requiring(["products.create"]))
        .allow("inventory.item.create", KernelCommandPolicy::requiring(["inventory.item.create"]))
        .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]))
        .allow("payments.create_refund", KernelCommandPolicy::requiring(["payments.create_refund"]))
        .allow("checkout.commit", KernelCommandPolicy::requiring(["checkout.commit"]))
        .allow("subscriptions.charge", KernelCommandPolicy::requiring(["subscriptions.charge"]))
        .allow("a2a.escrow.create", KernelCommandPolicy::requiring(["a2a.escrow.create"]))
        .allow("a2a.escrow.dispute", KernelCommandPolicy::requiring(["a2a.escrow.dispute"]))
        .allow("a2a.escrow.fund", KernelCommandPolicy::requiring(["a2a.escrow.fund"]))
        .allow("a2a.escrow.release", KernelCommandPolicy::requiring(["a2a.escrow.release"]))
        .allow("a2a.escrow.refund", KernelCommandPolicy::requiring(["a2a.escrow.refund"]))
        .allow("a2a.dispute.file", KernelCommandPolicy::requiring(["a2a.dispute.file"]))
        .allow(
            "a2a.dispute.evidence.submit",
            KernelCommandPolicy::requiring(["a2a.dispute.evidence.submit"]),
        )
        .allow("a2a.dispute.resolve", KernelCommandPolicy::requiring(["a2a.dispute.resolve"]))
        .allow("inventory.reserve", KernelCommandPolicy::requiring(["inventory.reserve"]))
        .allow(
            "inventory.reservation.confirm",
            KernelCommandPolicy::requiring(["inventory.reservation.confirm"]),
        )
        .allow(
            "inventory.reservation.release",
            KernelCommandPolicy::requiring(["inventory.reservation.release"]),
        )
        .allow("orders.transition", KernelCommandPolicy::requiring(["orders.transition"]))
        .allow("orders.ship", KernelCommandPolicy::requiring(["orders.ship"]))
        .allow("returns.transition", KernelCommandPolicy::requiring(["returns.transition"]))
        .allow("ledger.post", KernelCommandPolicy::requiring(["ledger.post"]))
        .allow("x402.settle", KernelCommandPolicy::requiring(["x402.settle"]))
}

#[tokio::test]
async fn postgres_kernel_inventory_create_serializes_sku_and_preserves_exact_quantity() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping inventory proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let suffix = Uuid::new_v4();
    let sku = format!("PG-INV-{suffix}");
    let payload = CreateInventoryItem {
        sku: sku.clone(),
        name: "PostgreSQL fractional inventory".into(),
        initial_quantity: Some(dec!(9007199254740993.125)),
        reorder_point: Some(dec!(0.125)),
        safety_stock: Some(dec!(0.025)),
        ..Default::default()
    };
    let preview = kernel_command(
        "inventory.item.create",
        format!("pg-inventory-preview-{suffix}"),
        "inventory.item.create",
        payload.clone(),
    );
    let previewed = db
        .kernel_executor(policy())
        .execute_create_inventory_item_async(&preview)
        .await
        .expect("preview inventory item");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);

    let mut tasks = Vec::new();
    for index in 0..8 {
        let db = Arc::clone(&db);
        let mut command = kernel_command(
            "inventory.item.create",
            format!("pg-inventory-apply-{suffix}-{index}"),
            "inventory.item.create",
            payload.clone(),
        );
        command.mode = ExecutionMode::Apply;
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_create_inventory_item_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute inventory create"));
    }
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Succeeded).count(),
        1
    );
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Rejected).count(),
        7
    );
    assert!(receipts.iter().all(|receipt| receipt.audit_hash.is_some()));
    let stored: rust_decimal::Decimal = sqlx::query_scalar(
        "SELECT b.quantity_on_hand FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = $1",
    )
    .bind(&sku)
    .fetch_one(db.pool())
    .await
    .expect("load exact inventory quantity");
    assert_eq!(stored, dec!(9007199254740993.125));
    let transactions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_transactions t JOIN inventory_items i ON i.id = t.item_id WHERE i.sku = $1 AND t.transaction_type = 'receipt'",
    )
    .bind(&sku)
    .fetch_one(db.pool())
    .await
    .expect("count initial receipts");
    assert_eq!(transactions, 1);
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_product_create_serializes_semantic_uniqueness_and_exact_money() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping product proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let suffix = Uuid::new_v4();
    let sku = format!("PG-AGENT-{suffix}");
    let slug = format!("pg-autonomous-offer-{suffix}");
    let payload = CreateProduct {
        name: "PostgreSQL Autonomous Offer".into(),
        slug: Some(slug.clone()),
        variants: Some(vec![CreateProductVariant {
            sku: sku.clone(),
            name: Some("Default".into()),
            price: dec!(9007199254740993.25),
            compare_at_price: Some(dec!(9007199254740994.25)),
            is_default: Some(true),
            ..Default::default()
        }]),
        ..Default::default()
    };

    let preview = kernel_command(
        "products.create",
        format!("pg-product-preview-{suffix}"),
        "products.create",
        payload.clone(),
    );
    let previewed = db
        .kernel_executor(policy())
        .execute_create_product_async(&preview)
        .await
        .expect("preview product");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);

    let mut tasks = Vec::new();
    for index in 0..8 {
        let db = Arc::clone(&db);
        let mut command = kernel_command(
            "products.create",
            format!("pg-product-apply-{suffix}-{index}"),
            "products.create",
            payload.clone(),
        );
        command.mode = ExecutionMode::Apply;
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_create_product_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute product"));
    }
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Succeeded).count(),
        1
    );
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Rejected).count(),
        7
    );
    assert!(receipts.iter().all(|receipt| receipt.audit_hash.is_some()));
    let stored_price: rust_decimal::Decimal =
        sqlx::query_scalar("SELECT price FROM product_variants WHERE sku = $1")
            .bind(&sku)
            .fetch_one(db.pool())
            .await
            .expect("load exact variant price");
    assert_eq!(stored_price, dec!(9007199254740993.25));
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

fn kernel_command<C>(
    command_type: &str,
    key: String,
    capability: &str,
    payload: C,
) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(
        command_type,
        key,
        KernelPrincipal {
            id: "agent:postgres-parity".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec![capability.into()],
        },
        payload,
    );
    command.store_id = Some("store-postgres".into());
    command.policy_version = Some("commerce-policy-1".into());
    command
}

fn payment_command(key: String) -> CommandEnvelope<CreatePayment> {
    let mut command = CommandEnvelope::preview(
        "payments.create",
        key,
        KernelPrincipal {
            id: "agent:postgres-checkout".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec!["payments.create".into()],
        },
        CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        },
    );
    command.store_id = Some("store-postgres".into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

fn refund_command(
    key: String,
    payment_id: stateset_core::PaymentId,
) -> CommandEnvelope<CreateRefund> {
    let mut command = CommandEnvelope::preview(
        "payments.create_refund",
        key,
        KernelPrincipal {
            id: "agent:postgres-refunds".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec!["payments.create_refund".into()],
        },
        CreateRefund {
            payment_id,
            amount: Some(dec!(20.00)),
            reason: Some("kernel concurrency proof".into()),
            ..Default::default()
        },
    );
    command.store_id = Some("store-postgres".into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

#[tokio::test]
async fn postgres_kernel_same_key_concurrency_executes_once_and_replays_one_receipt() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping kernel concurrency proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let command = payment_command(format!("pg-kernel-payment-{}", Uuid::new_v4()));
    let mut tasks = Vec::new();
    for _ in 0..12 {
        let db = Arc::clone(&db);
        let command = command.clone();
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_create_payment_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute or replay"));
    }
    assert!(receipts.iter().all(|receipt| receipt.status == ExecutionStatus::Succeeded));
    assert!(receipts.iter().all(|receipt| receipt.receipt_id == receipts[0].receipt_id));
    assert!(receipts.iter().all(|receipt| receipt.audit_hash == receipts[0].audit_hash));
    assert!(receipts[0].audit_hash.is_some());
    let payment_id = receipts[0].result.as_ref().expect("payment result").id;
    assert!(db.payments().get_async(payment_id.into_uuid()).await.expect("load").is_some());
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
    let checkpoint = db.kernel_outbox().audit_checkpoint_async().await.expect("checkpoint");
    assert!(
        db.kernel_outbox()
            .verify_audit_checkpoint_async(&checkpoint)
            .await
            .expect("verify checkpoint")
    );
}

#[tokio::test]
async fn postgres_kernel_concurrent_refunds_preserve_money_and_receipt_invariants() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping kernel refund proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let payment = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(100.00),
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("complete payment");

    let mut tasks = Vec::new();
    for contender in 0..10 {
        let db = Arc::clone(&db);
        let command =
            refund_command(format!("pg-kernel-refund-{}-{contender}", Uuid::new_v4()), payment.id);
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_create_refund_async(&command).await
        }));
    }
    let mut succeeded = 0;
    let mut rejected = 0;
    for task in tasks {
        let receipt = task.await.expect("join").expect("durable outcome");
        assert!(receipt.audit_hash.is_some());
        match receipt.status {
            ExecutionStatus::Succeeded => succeeded += 1,
            ExecutionStatus::Rejected => {
                rejected += 1;
                assert_eq!(receipt.error_code.as_deref(), Some("commerce.refund.exceeds_captured"));
            }
            status => panic!("unexpected outcome {status:?}"),
        }
    }
    assert_eq!((succeeded, rejected), (5, 5));
    let refunds = db.payments().get_refunds_async(payment.id.into_uuid()).await.expect("refunds");
    assert_eq!(
        refunds.iter().map(|refund| refund.amount).sum::<rust_decimal::Decimal>(),
        dec!(100)
    );
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_checkout_preview_and_concurrent_apply_commit_once() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping kernel checkout proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let suffix = Uuid::new_v4();
    let email = format!("pg-kernel-checkout-{suffix}@example.com");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: email.clone(),
            first_name: "Postgres".into(),
            last_name: "Kernel".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let cart = db
        .carts()
        .create_async(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(email.clone()),
            customer_name: Some("Postgres Kernel".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");
    db.carts()
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: None,
                sku: format!("PG-KERNEL-CHECKOUT-{suffix}"),
                name: "Postgres Kernel Item".into(),
                quantity: 2,
                unit_price: dec!(12.34),
                ..Default::default()
            },
        )
        .await
        .expect("add item");
    db.carts()
        .set_shipping_address_async(
            cart.id.into_uuid(),
            CartAddress {
                first_name: "Postgres".into(),
                last_name: "Kernel".into(),
                company: None,
                line1: "1 Transaction Way".into(),
                line2: None,
                city: "Vancouver".into(),
                state: Some("BC".into()),
                postal_code: "V6B 1A1".into(),
                country: "CA".into(),
                phone: None,
                email: Some(email),
            },
        )
        .await
        .expect("set shipping");
    db.carts()
        .set_payment_async(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_pg_kernel".into()),
                ..Default::default()
            },
        )
        .await
        .expect("set payment");

    let key = format!("pg-kernel-checkout-{suffix}");
    let mut preview = CommandEnvelope::preview(
        "checkout.commit",
        key,
        KernelPrincipal {
            id: "agent:postgres-checkout".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec!["checkout.commit".into()],
        },
        CommitCheckout { cart_id: cart.id },
    );
    preview.store_id = Some("store-postgres".into());
    preview.policy_version = Some("commerce-policy-1".into());
    let previewed = db
        .kernel_executor(policy())
        .execute_commit_checkout_async(&preview)
        .await
        .expect("preview checkout");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(
        db.carts().get_async(cart.id.into_uuid()).await.expect("load cart").expect("cart").status,
        CartStatus::Active
    );

    preview.command_id = Uuid::new_v4();
    preview.mode = ExecutionMode::Apply;
    let command = preview;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let db = Arc::clone(&db);
        let command = command.clone();
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_commit_checkout_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute or replay"));
    }
    assert!(receipts.iter().all(|receipt| receipt.status == ExecutionStatus::Succeeded));
    assert!(receipts.iter().all(|receipt| receipt.receipt_id == receipts[0].receipt_id));
    let checkout = receipts[0].result.as_ref().expect("checkout result");
    let order = db
        .orders()
        .get_async(checkout.order_id.into_uuid())
        .await
        .expect("load order")
        .expect("order");
    assert_eq!(order.status, OrderStatus::Confirmed);
    assert_eq!(order.payment_status, PaymentStatus::Pending);
    let completed =
        db.carts().get_async(cart.id.into_uuid()).await.expect("load cart").expect("cart");
    assert_eq!(completed.status, CartStatus::Completed);
    assert_eq!(completed.order_id, Some(order.id));
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_subscription_charge_converges_on_one_pending_payment() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping subscription charge proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let suffix = Uuid::new_v4();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("pg-kernel-subscription-{suffix}@example.com"),
            first_name: "Subscription".into(),
            last_name: "Kernel".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let subscriptions = db.subscriptions();
    let plan = subscriptions
        .create_plan_async(CreateSubscriptionPlan {
            code: Some(format!("kernel-{suffix}")),
            name: "Postgres Kernel Monthly".into(),
            description: None,
            billing_interval: BillingInterval::Monthly,
            custom_interval_days: None,
            price: dec!(41.25),
            setup_fee: None,
            currency: Some(CurrencyCode::USD),
            trial_days: Some(0),
            trial_requires_payment_method: Some(true),
            min_cycles: None,
            max_cycles: None,
            items: None,
            discount_percent: None,
            discount_amount: None,
            metadata: None,
        })
        .await
        .expect("create plan");
    subscriptions.activate_plan_async(plan.id).await.expect("activate plan");
    let subscription = subscriptions
        .create_subscription_async(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            payment_method_id: Some("pm_pg_kernel".into()),
            skip_trial: Some(true),
            ..Default::default()
        })
        .await
        .expect("create subscription");
    let cycle = subscriptions
        .list_billing_cycles_async(BillingCycleFilter {
            subscription_id: Some(subscription.id),
            ..Default::default()
        })
        .await
        .expect("list cycles")
        .into_iter()
        .next()
        .expect("initial cycle");
    let key = format!("pg-kernel-subscription-charge-{suffix}");
    let mut preview = CommandEnvelope::preview(
        "subscriptions.charge",
        key,
        KernelPrincipal {
            id: "agent:postgres-subscriptions".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec!["subscriptions.charge".into()],
        },
        ChargeSubscription {
            billing_cycle_id: cycle.id,
            payment_method: PaymentMethodType::CreditCard,
            processor: Some("test-processor".into()),
        },
    );
    preview.store_id = Some("store-postgres".into());
    preview.policy_version = Some("commerce-policy-1".into());
    let previewed = db
        .kernel_executor(policy())
        .execute_charge_subscription_async(&preview)
        .await
        .expect("preview charge");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    assert_eq!(
        db.subscriptions()
            .get_billing_cycle_async(cycle.id)
            .await
            .expect("load cycle")
            .expect("cycle")
            .status,
        BillingCycleStatus::Scheduled
    );

    preview.command_id = Uuid::new_v4();
    preview.mode = ExecutionMode::Apply;
    let command = preview;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let db = Arc::clone(&db);
        let command = command.clone();
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_charge_subscription_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute or replay"));
    }
    assert!(receipts.iter().all(|receipt| receipt.status == ExecutionStatus::Succeeded));
    assert!(receipts.iter().all(|receipt| receipt.receipt_id == receipts[0].receipt_id));
    let charge = receipts[0].result.as_ref().expect("charge result");
    assert_eq!(charge.billing_cycle.status, BillingCycleStatus::Processing);
    assert_eq!(charge.payment.status, stateset_core::PaymentTransactionStatus::Pending);
    assert_eq!(charge.payment.amount, dec!(41.25));
    assert_eq!(charge.billing_cycle.payment_id, Some(charge.payment.id.to_string()));
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_a2a_escrow_create_fund_and_refund_preserve_lifecycle_parity() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping A2A lifecycle proof");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect and migrate");
    let suffix = Uuid::new_v4();
    let mut create = kernel_command(
        "a2a.escrow.create",
        format!("pg-a2a-create-{suffix}"),
        "a2a.escrow.create",
        CreateA2AEscrow {
            quote_id: None,
            payment_id: None,
            buyer_address: format!("did:key:buyer:{suffix}"),
            seller_address: format!("did:key:seller:{suffix}"),
            amount: dec!(0.123456),
            asset: "USDC".into(),
            network: "set_chain".into(),
            release_conditions: vec![],
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
            auto_release_after: None,
            metadata: Some(serde_json::json!({"test": "postgres-parity"})),
        },
    );
    let previewed = db
        .kernel_executor(policy())
        .execute_create_a2a_escrow_async(&create)
        .await
        .expect("preview create");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    create.command_id = Uuid::new_v4();
    create.mode = ExecutionMode::Apply;
    let created = db
        .kernel_executor(policy())
        .execute_create_a2a_escrow_async(&create)
        .await
        .expect("apply create");
    assert_eq!(created.status, ExecutionStatus::Succeeded);
    assert!(created.audit_hash.is_some());
    let escrow = created.result.expect("created escrow");
    assert_eq!(escrow.amount_decimal, dec!(0.123456));

    let mut fund = kernel_command(
        "a2a.escrow.fund",
        format!("pg-a2a-fund-{suffix}"),
        "a2a.escrow.fund",
        FundA2AEscrow { escrow_id: escrow.id.clone() },
    );
    fund.mode = ExecutionMode::Apply;
    let funded = db
        .kernel_executor(policy())
        .execute_fund_a2a_escrow_async(&fund)
        .await
        .expect("fund escrow");
    assert_eq!(
        funded.result.expect("funded escrow").status,
        stateset_core::A2AEscrowStatus::Active
    );

    let mut dispute = kernel_command(
        "a2a.escrow.dispute",
        format!("pg-a2a-dispute-{suffix}"),
        "a2a.escrow.dispute",
        DisputeA2AEscrow {
            escrow_id: escrow.id.clone(),
            reason: "seller did not provide delivery evidence".into(),
            category: Some("non_delivery".into()),
        },
    );
    dispute.mode = ExecutionMode::Apply;
    let disputed = db
        .kernel_executor(policy())
        .execute_dispute_a2a_escrow_async(&dispute)
        .await
        .expect("dispute escrow");
    assert_eq!(
        disputed.result.expect("disputed escrow").status,
        stateset_core::A2AEscrowStatus::Disputed
    );

    let mut refund = kernel_command(
        "a2a.escrow.refund",
        format!("pg-a2a-refund-{suffix}"),
        "a2a.escrow.refund",
        RefundA2AEscrow {
            escrow_id: escrow.id.clone(),
            reason: Some("postgres lifecycle parity".into()),
        },
    );
    refund.mode = ExecutionMode::Apply;
    let command = refund;
    let db = Arc::new(db);
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let db = Arc::clone(&db);
        let command = command.clone();
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_refund_a2a_escrow_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute or replay"));
    }
    assert!(receipts.iter().all(|receipt| receipt.status == ExecutionStatus::Succeeded));
    assert!(receipts.iter().all(|receipt| receipt.receipt_id == receipts[0].receipt_id));
    assert_eq!(
        receipts[0].result.as_ref().expect("refunded escrow").status,
        stateset_core::A2AEscrowStatus::Refunded
    );
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM kernel_outbox WHERE aggregate_type = 'a2a_escrow' AND aggregate_id = $1",
    )
    .bind(&escrow.id)
    .fetch_one(db.pool())
    .await
    .expect("count lifecycle events");
    assert_eq!(events, 4);
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_a2a_formal_dispute_is_scoped_exact_atomic_and_convergent() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping formal dispute parity proof");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect and migrate");
    let suffix = Uuid::new_v4();
    let buyer = format!("did:key:buyer:{suffix}");
    let seller = format!("did:key:seller:{suffix}");
    let mut create = kernel_command(
        "a2a.escrow.create",
        format!("pg-formal-create-{suffix}"),
        "a2a.escrow.create",
        CreateA2AEscrow {
            quote_id: None,
            payment_id: None,
            buyer_address: buyer.clone(),
            seller_address: seller.clone(),
            amount: dec!(123.456789),
            asset: "USDC".into(),
            network: "set_chain".into(),
            release_conditions: vec![],
            expires_at: chrono::Utc::now() + chrono::Duration::days(30),
            auto_release_after: None,
            metadata: None,
        },
    );
    create.mode = ExecutionMode::Apply;
    let escrow = db
        .kernel_executor(policy())
        .execute_create_a2a_escrow_async(&create)
        .await
        .expect("create escrow")
        .result
        .expect("escrow result");
    assert_eq!(escrow.tenant_id, "tenant-postgres");
    assert_eq!(escrow.store_id, "store-postgres");

    let mut fund = kernel_command(
        "a2a.escrow.fund",
        format!("pg-formal-fund-{suffix}"),
        "a2a.escrow.fund",
        FundA2AEscrow { escrow_id: escrow.id.clone() },
    );
    fund.mode = ExecutionMode::Apply;
    db.kernel_executor(policy()).execute_fund_a2a_escrow_async(&fund).await.expect("fund escrow");

    let now = chrono::Utc::now();
    let mut file = kernel_command(
        "a2a.dispute.file",
        format!("pg-formal-file-{suffix}"),
        "a2a.dispute.file",
        FileA2ADispute {
            escrow_id: escrow.id.clone(),
            claimant_address: buyer.clone(),
            reason: "delivery evidence missing".into(),
            category: "non_delivery".into(),
            evidence_deadline: now + chrono::Duration::days(7),
            review_deadline: now + chrono::Duration::days(14),
            metadata: None,
        },
    );
    file.principal.id = buyer.clone();
    let previewed = db
        .kernel_executor(policy())
        .execute_file_a2a_dispute_async(&file)
        .await
        .expect("preview dispute");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    file.command_id = Uuid::new_v4();
    file.mode = ExecutionMode::Apply;
    let dispute = db
        .kernel_executor(policy())
        .execute_file_a2a_dispute_async(&file)
        .await
        .expect("file dispute")
        .result
        .expect("dispute result");
    assert_eq!(dispute.amount, dec!(123.456789));
    assert_eq!(dispute.respondent_address, seller);

    let mut evidence = kernel_command(
        "a2a.dispute.evidence.submit",
        format!("pg-formal-evidence-{suffix}"),
        "a2a.dispute.evidence.submit",
        SubmitA2ADisputeEvidence {
            dispute_id: dispute.id.clone(),
            submitted_by: buyer.clone(),
            evidence_type: "communication".into(),
            title: "Seller conversation".into(),
            description: None,
            content: "seller acknowledged that delivery did not occur".into(),
        },
    );
    evidence.principal.id = buyer;
    evidence.mode = ExecutionMode::Apply;
    let submitted = db
        .kernel_executor(policy())
        .execute_submit_a2a_dispute_evidence_async(&evidence)
        .await
        .expect("submit evidence");
    assert!(submitted.result.expect("evidence result").content_hash.starts_with("sha256:"));

    let mut cross_tenant = kernel_command(
        "a2a.dispute.resolve",
        format!("pg-formal-cross-tenant-{suffix}"),
        "a2a.dispute.resolve",
        ResolveA2ADispute {
            dispute_id: dispute.id.clone(),
            resolution_type: A2ADisputeResolutionType::FullRefund,
            buyer_amount: None,
            seller_amount: None,
            note: None,
        },
    );
    cross_tenant.principal.tenant_id = Some("tenant-other".into());
    cross_tenant.mode = ExecutionMode::Apply;
    let denied = db
        .kernel_executor(policy())
        .execute_resolve_a2a_dispute_async(&cross_tenant)
        .await
        .expect("durable cross-tenant denial");
    assert_eq!(denied.status, ExecutionStatus::Rejected);
    assert_eq!(denied.error_code.as_deref(), Some("commerce.a2a.dispute_not_found"));

    let mut resolve = kernel_command(
        "a2a.dispute.resolve",
        format!("pg-formal-resolve-{suffix}"),
        "a2a.dispute.resolve",
        ResolveA2ADispute {
            dispute_id: dispute.id.clone(),
            resolution_type: A2ADisputeResolutionType::Split,
            buyer_amount: Some(dec!(40.000001)),
            seller_amount: Some(dec!(83.456788)),
            note: Some("evidence supports an exact proportional split".into()),
        },
    );
    resolve.mode = ExecutionMode::Apply;
    let command = resolve;
    let db = Arc::new(db);
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let db = Arc::clone(&db);
        let command = command.clone();
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_resolve_a2a_dispute_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute or replay"));
    }
    assert!(receipts.iter().all(|receipt| receipt.status == ExecutionStatus::Succeeded));
    assert!(receipts.iter().all(|receipt| receipt.receipt_id == receipts[0].receipt_id));
    let result = receipts[0].result.as_ref().expect("resolution result");
    assert_eq!(result.dispute.buyer_amount, Some(dec!(40.000001)));
    assert_eq!(result.dispute.seller_amount, Some(dec!(83.456788)));
    assert_eq!(result.escrow.status, stateset_core::A2AEscrowStatus::Resolved);
    assert_eq!(receipts[0].event_ids.len(), 2);
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_a2a_escrow_release_converges_after_all_conditions() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping A2A escrow proof");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect and migrate"));
    let suffix = Uuid::new_v4();
    let quote_id = Uuid::new_v4();
    let buyer_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO a2a_quotes (
            id, quote_number, status, buyer_agent_id, seller_agent_id, items,
            subtotal, tax_amount, shipping_amount, discount_amount, total, currency,
            valid_until, created_at, updated_at
         ) VALUES ($1, $2, 'fulfilled', $3, $4, '[]'::jsonb,
                   55.50, 0, 0, 0, 55.50, 'USD', $5, $6, $6)",
    )
    .bind(quote_id)
    .bind(format!("QUOTE-{suffix}"))
    .bind(buyer_id)
    .bind(seller_id)
    .bind(now + chrono::Duration::hours(24))
    .bind(now)
    .execute(db.pool())
    .await
    .expect("seed quote");
    let escrow_id = format!("escrow-{suffix}");
    let conditions = serde_json::json!([
        {"type": "seller_fulfilled", "quoteId": quote_id.to_string()},
        {"type": "buyer_confirmed", "completed": true},
        {"type": "time_lock", "releaseAfter": (now - chrono::Duration::minutes(1)).to_rfc3339()},
        {"type": "milestone", "completed": true}
    ]);
    sqlx::query(
        "INSERT INTO a2a_escrows (
            id, status, quote_id, buyer_address, seller_address, amount, amount_decimal,
            asset, network, release_conditions, funded_at, expires_at, created_at, updated_at,
            tenant_id, store_id
         ) VALUES ($1, 'active', $2, '0xbuyer', '0xseller', 55500000, 55.50,
                   'USDC', 'set_chain', $3, $4, $5, $4, $4,
                   'tenant-postgres', 'store-postgres')",
    )
    .bind(&escrow_id)
    .bind(quote_id.to_string())
    .bind(&conditions)
    .bind(now)
    .bind(now + chrono::Duration::hours(24))
    .execute(db.pool())
    .await
    .expect("seed escrow");
    let key = format!("pg-kernel-escrow-release-{suffix}");
    let mut preview = CommandEnvelope::preview(
        "a2a.escrow.release",
        key,
        KernelPrincipal {
            id: "agent:postgres-escrow".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec!["a2a.escrow.release".into()],
        },
        ReleaseA2AEscrow { escrow_id: escrow_id.clone() },
    );
    preview.store_id = Some("store-postgres".into());
    preview.policy_version = Some("commerce-policy-1".into());
    let previewed = db
        .kernel_executor(policy())
        .execute_release_a2a_escrow_async(&preview)
        .await
        .expect("preview release");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    let stored_status: String = sqlx::query_scalar("SELECT status FROM a2a_escrows WHERE id = $1")
        .bind(&escrow_id)
        .fetch_one(db.pool())
        .await
        .expect("load status");
    assert_eq!(stored_status, "active");

    preview.command_id = Uuid::new_v4();
    preview.mode = ExecutionMode::Apply;
    let command = preview;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let db = Arc::clone(&db);
        let command = command.clone();
        tasks.push(tokio::spawn(async move {
            db.kernel_executor(policy()).execute_release_a2a_escrow_async(&command).await
        }));
    }
    let mut receipts = Vec::new();
    for task in tasks {
        receipts.push(task.await.expect("join").expect("execute or replay"));
    }
    assert!(receipts.iter().all(|receipt| receipt.status == ExecutionStatus::Succeeded));
    assert!(receipts.iter().all(|receipt| receipt.receipt_id == receipts[0].receipt_id));
    assert_eq!(receipts[0].result.as_ref().expect("escrow").amount_decimal, dec!(55.50));
    let released: (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status, released_at FROM a2a_escrows WHERE id = $1")
            .bind(&escrow_id)
            .fetch_one(db.pool())
            .await
            .expect("load released escrow");
    assert_eq!(released.0, "released");
    assert!(released.1.is_some());
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}

#[tokio::test]
async fn postgres_kernel_inventory_commands_preserve_exact_lifecycle_parity() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping inventory parity proof");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect and migrate");
    let suffix = Uuid::new_v4();
    let sku = format!("PG-KERNEL-INVENTORY-{suffix}");
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: "Postgres kernel stock".into(),
            initial_quantity: Some(dec!(10.000)),
            ..Default::default()
        })
        .await
        .expect("create stock");
    let mut reserve = kernel_command(
        "inventory.reserve",
        format!("pg-reserve-{suffix}"),
        "inventory.reserve",
        ReserveInventory {
            sku: sku.clone(),
            location_id: Some(1),
            quantity: dec!(3.500),
            reference_type: "order".into(),
            reference_id: format!("order-{suffix}"),
            expires_in_seconds: Some(900),
        },
    );
    let previewed = db
        .kernel_executor(policy())
        .execute_reserve_inventory_async(&reserve)
        .await
        .expect("preview reserve");
    assert_eq!(previewed.status, ExecutionStatus::Previewed);
    reserve.command_id = Uuid::new_v4();
    reserve.mode = ExecutionMode::Apply;
    let reserved = db
        .kernel_executor(policy())
        .execute_reserve_inventory_async(&reserve)
        .await
        .expect("reserve");
    assert_eq!(reserved.status, ExecutionStatus::Succeeded);
    let reservation = reserved.result.as_ref().expect("reservation");
    assert_eq!(reservation.quantity, dec!(3.500));

    let mut confirm = kernel_command(
        "inventory.reservation.confirm",
        format!("pg-confirm-{suffix}"),
        "inventory.reservation.confirm",
        ConfirmInventoryReservation { reservation_id: reservation.id, quantity: Some(dec!(2.000)) },
    );
    confirm.mode = ExecutionMode::Apply;
    let confirmed = db
        .kernel_executor(policy())
        .execute_confirm_inventory_reservation_async(&confirm)
        .await
        .expect("confirm reservation");
    assert_eq!(confirmed.status, ExecutionStatus::Succeeded);
    assert_eq!(confirmed.result.as_ref().expect("confirmed").quantity, dec!(2.000));
    assert_eq!(confirmed.result.as_ref().expect("confirmed").status, ReservationStatus::Confirmed);

    let mut release = kernel_command(
        "inventory.reservation.release",
        format!("pg-release-{suffix}"),
        "inventory.reservation.release",
        ReleaseInventoryReservation { reservation_id: reservation.id },
    );
    release.mode = ExecutionMode::Apply;
    let released = db
        .kernel_executor(policy())
        .execute_release_inventory_reservation_async(&release)
        .await
        .expect("release remainder");
    assert_eq!(released.status, ExecutionStatus::Succeeded);
    assert_eq!(released.result.as_ref().expect("released").status, ReservationStatus::Released);
    let balance: (rust_decimal::Decimal, rust_decimal::Decimal) = sqlx::query_as(
        "SELECT b.quantity_available, b.quantity_allocated
         FROM inventory_balances b JOIN inventory_items i ON i.id = b.item_id
         WHERE i.sku = $1 AND b.location_id = 1",
    )
    .bind(&sku)
    .fetch_one(db.pool())
    .await
    .expect("load balance");
    assert_eq!(balance, (dec!(8.000), dec!(2.000)));
}

#[tokio::test]
async fn postgres_kernel_order_shipment_and_return_commands_share_state_machine_parity() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping lifecycle parity proof");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect and migrate");
    let suffix = Uuid::new_v4();
    let sku = format!("PG-KERNEL-LIFECYCLE-{suffix}");
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: "Lifecycle stock".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .await
        .expect("create stock");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("pg-kernel-lifecycle-{suffix}@example.com"),
            first_name: "Lifecycle".into(),
            last_name: "Kernel".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.clone(),
                name: "Lifecycle item".into(),
                quantity: 2,
                unit_price: dec!(15.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    let mut transition = kernel_command(
        "orders.transition",
        format!("pg-order-transition-{suffix}"),
        "orders.transition",
        TransitionOrder {
            order_id: order.id,
            status: OrderStatus::Confirmed,
            payment_status: None,
        },
    );
    transition.expected_version = Some(order.version);
    transition.mode = ExecutionMode::Apply;
    let confirmed = db
        .kernel_executor(policy())
        .execute_transition_order_async(&transition)
        .await
        .expect("confirm order");
    assert_eq!(confirmed.status, ExecutionStatus::Succeeded);
    assert_eq!(confirmed.result.as_ref().expect("order").status, OrderStatus::Confirmed);
    let processing = db
        .orders()
        .update_async(
            order.id.into_uuid(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("process order");
    let line_id = processing.items[0].id;
    let mut ship_partial = kernel_command(
        "orders.ship",
        format!("pg-order-ship-partial-{suffix}"),
        "orders.ship",
        ShipOrderCommand {
            order_id: order.id,
            tracking_number: Some("PG-KERNEL-TRACK".into()),
            lines: Some(vec![ShipmentLineInput { order_item_id: line_id, quantity: 1 }]),
        },
    );
    ship_partial.expected_version = Some(processing.version);
    ship_partial.mode = ExecutionMode::Apply;
    let partial = db
        .kernel_executor(policy())
        .execute_ship_order_async(&ship_partial)
        .await
        .expect("partial shipment");
    assert_eq!(partial.status, ExecutionStatus::Succeeded);
    assert_eq!(partial.result.as_ref().expect("order").status, OrderStatus::PartiallyShipped);
    let partial_order = partial.result.as_ref().expect("order");
    let mut ship_rest = kernel_command(
        "orders.ship",
        format!("pg-order-ship-rest-{suffix}"),
        "orders.ship",
        ShipOrderCommand {
            order_id: order.id,
            tracking_number: Some("PG-KERNEL-TRACK".into()),
            lines: None,
        },
    );
    ship_rest.expected_version = Some(partial_order.version);
    ship_rest.mode = ExecutionMode::Apply;
    let shipped = db
        .kernel_executor(policy())
        .execute_ship_order_async(&ship_rest)
        .await
        .expect("complete shipment");
    assert_eq!(shipped.result.as_ref().expect("order").status, OrderStatus::Shipped);
    assert_eq!(shipped.result.as_ref().expect("order").items[0].shipped_quantity, 2);

    let returned = db
        .returns()
        .create_async(CreateReturn {
            order_id: order.id,
            items: vec![CreateReturnItem { order_item_id: line_id, quantity: 1, condition: None }],
            ..Default::default()
        })
        .await
        .expect("create return");
    let mut approve = kernel_command(
        "returns.transition",
        format!("pg-return-approve-{suffix}"),
        "returns.transition",
        TransitionReturn { return_id: returned.id, status: ReturnStatus::Approved },
    );
    approve.expected_version = Some(returned.version);
    approve.mode = ExecutionMode::Apply;
    let approved = db
        .kernel_executor(policy())
        .execute_transition_return_async(&approve)
        .await
        .expect("approve return");
    assert_eq!(approved.status, ExecutionStatus::Succeeded);
    assert_eq!(approved.result.as_ref().expect("return").status, ReturnStatus::Approved);
}

#[tokio::test]
async fn postgres_kernel_ledger_and_x402_commands_preserve_exact_fact_parity() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping finance parity proof");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect and migrate");
    let suffix = Uuid::new_v4();
    let bytes = suffix.as_bytes();
    let fiscal_year = 2200 + i32::from(bytes[0]);
    let period_number = 1 + i32::from(bytes[1] % 12);
    let gl = db.general_ledger();
    let period = gl
        .create_period_async(CreateGlPeriod {
            period_name: format!("FY{fiscal_year}-{period_number}-{suffix}"),
            fiscal_year,
            period_number,
            start_date: chrono::NaiveDate::from_ymd_opt(fiscal_year, 1, 1).expect("date"),
            end_date: chrono::NaiveDate::from_ymd_opt(fiscal_year, 12, 31).expect("date"),
        })
        .await
        .expect("create period");
    gl.open_period_async(period.id).await.expect("open period");
    let cash = gl
        .create_account_async(CreateGlAccount {
            account_number: format!("1000-{suffix}"),
            name: "Kernel Cash".into(),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .await
        .expect("create cash");
    let revenue = gl
        .create_account_async(CreateGlAccount {
            account_number: format!("4000-{suffix}"),
            name: "Kernel Revenue".into(),
            description: None,
            account_type: AccountType::Revenue,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .await
        .expect("create revenue");
    let entry = gl
        .create_journal_entry_async(CreateJournalEntry {
            entry_date: chrono::NaiveDate::from_ymd_opt(fiscal_year, 8, 1).expect("date"),
            entry_type: None,
            description: "PostgreSQL kernel parity".into(),
            lines: vec![
                CreateJournalEntryLine::debit(cash.id, dec!(77.77), None),
                CreateJournalEntryLine::credit(revenue.id, dec!(77.77), None),
            ],
            source_document_type: Some("kernel_command".into()),
            source_document_id: None,
            auto_post: Some(false),
        })
        .await
        .expect("create journal");
    let mut post = kernel_command(
        "ledger.post",
        format!("pg-ledger-post-{suffix}"),
        "ledger.post",
        PostJournalEntry { journal_entry_id: entry.id, posted_by: "agent:postgres-parity".into() },
    );
    post.mode = ExecutionMode::Apply;
    let posted = db
        .kernel_executor(policy())
        .execute_post_journal_entry_async(&post)
        .await
        .expect("post journal");
    assert_eq!(posted.status, ExecutionStatus::Succeeded);
    assert_eq!(posted.result.as_ref().expect("entry").status, JournalEntryStatus::Posted);
    assert_eq!(posted.result.as_ref().expect("entry").total_debits, dec!(77.77));

    let intent = db
        .x402_payment_intents()
        .create_async(CreateX402PaymentIntent {
            payer_address: format!("0xpayer{suffix}"),
            payee_address: format!("0xpayee{suffix}"),
            amount: 2_500_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .await
        .expect("create x402 intent");
    sqlx::query("UPDATE x402_payment_intents SET status = 'sequenced' WHERE id = $1")
        .bind(intent.id)
        .execute(db.pool())
        .await
        .expect("sequence intent");
    let mut settle = kernel_command(
        "x402.settle",
        format!("pg-x402-settle-{suffix}"),
        "x402.settle",
        SettleX402Intent {
            intent_id: intent.id,
            tx_hash: format!("0xsettled{suffix}"),
            block_number: 4242,
        },
    );
    settle.mode = ExecutionMode::Apply;
    let settled = db
        .kernel_executor(policy())
        .execute_settle_x402_intent_async(&settle)
        .await
        .expect("settle intent");
    assert_eq!(settled.status, ExecutionStatus::Succeeded);
    assert_eq!(settled.result.as_ref().expect("intent").status, X402IntentStatus::Settled);
    assert_eq!(settled.result.as_ref().expect("intent").block_number, Some(4242));
    assert!(db.kernel_outbox().verify_audit_chain_async().await.expect("verify chain").valid);
}
