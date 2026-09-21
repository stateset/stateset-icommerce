//! Regression tests for the Postgres gift-card / store-credit / loyalty money
//! guards (mirrors the SQLite in-module tests).
//!
//! Guards under test:
//! - charge/apply/refund reject non-positive amounts (a negative charge or
//!   apply previously *minted* balance)
//! - date-expired instruments whose status is still `active` cannot spend
//! - refunds cannot resurrect disabled gift cards
//! - store-credit apply requires `Active` status; adjust rejects voided credits
//! - loyalty redemptions cannot overdraw an account, and unknown accounts error
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` /
//! `DATABASE_URL`) and are skipped otherwise, so they run only in CI with a
//! provisioned database (the Postgres Parity job).

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AdjustPoints, AdjustStoreCredit, CommerceError, CreateGiftCard, CreateLoyaltyProgram,
    CreateStoreCredit, CurrencyCode, CustomerId, EnrollCustomer, GiftCardStatus, LoyaltyAccountId,
    LoyaltyTransactionType, StoreCreditReason,
};
use stateset_db::PostgresDatabase;
use stateset_db::postgres::{
    PgGiftCardRepository, PgLoyaltyProgramRepository, PgStoreCreditRepository,
};
use std::env;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

fn assert_validation(err: CommerceError) {
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

async fn create_card(
    repo: &PgGiftCardRepository,
    balance: Decimal,
    expires_at: Option<chrono::DateTime<Utc>>,
) -> stateset_core::GiftCard {
    repo.create_async(CreateGiftCard {
        code: None,
        initial_balance: balance,
        currency: CurrencyCode::USD,
        recipient_email: None,
        sender_name: None,
        message: None,
        expires_at,
    })
    .await
    .expect("create gift card")
}

#[tokio::test]
async fn postgres_gift_card_charge_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgGiftCardRepository::new(db.pool().clone());
    let gc = create_card(&repo, dec!(50.00), None).await;

    // Non-positive amounts must not mint balance.
    assert_validation(repo.charge_async(gc.id, dec!(-10.00), None).await.unwrap_err());
    assert_validation(repo.charge_async(gc.id, Decimal::ZERO, None).await.unwrap_err());

    // Overdraft rejected; exact spend allowed.
    assert_validation(repo.charge_async(gc.id, dec!(60.00), None).await.unwrap_err());
    let txn = repo.charge_async(gc.id, dec!(30.00), None).await.expect("charge 30");
    assert_eq!(txn.balance_after, dec!(20.00));

    // A date-expired card whose status is still 'active' cannot spend.
    let expired = create_card(&repo, dec!(50.00), Some(Utc::now() - Duration::days(1))).await;
    assert_eq!(expired.status, GiftCardStatus::Active);
    assert_validation(repo.charge_async(expired.id, dec!(10.00), None).await.unwrap_err());
}

#[tokio::test]
async fn postgres_gift_card_refund_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgGiftCardRepository::new(db.pool().clone());
    let gc = create_card(&repo, dec!(50.00), None).await;

    assert_validation(repo.refund_async(gc.id, dec!(-5.00), None).await.unwrap_err());

    // Refunding a disabled card must not resurrect it.
    repo.disable_async(gc.id).await.expect("disable");
    assert_validation(repo.refund_async(gc.id, dec!(10.00), None).await.unwrap_err());
    let fetched = repo.get_async(gc.id).await.expect("get").expect("found");
    assert_eq!(fetched.status, GiftCardStatus::Disabled);
    assert_eq!(fetched.current_balance, dec!(50.00));
}

/// Postgres enforces `store_credits.customer_id` as a foreign key (unlike the
/// SQLite test schema), so the credit needs a real customer row.
async fn create_customer(db: &PostgresDatabase) -> CustomerId {
    let unique = uuid::Uuid::new_v4();
    db.customers()
        .create_async(stateset_core::CreateCustomer {
            email: format!("guard-{unique}@example.com"),
            first_name: "Guard".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer")
        .id
}

async fn create_credit(
    repo: &PgStoreCreditRepository,
    customer_id: CustomerId,
    amount: Decimal,
) -> stateset_core::StoreCredit {
    repo.create_async(CreateStoreCredit {
        customer_id,
        amount,
        currency: CurrencyCode::USD,
        reason: StoreCreditReason::Return,
        reference_id: None,
        note: None,
        expires_at: None,
    })
    .await
    .expect("create store credit")
}

#[tokio::test]
async fn postgres_store_credit_apply_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgStoreCreditRepository::new(db.pool().clone());
    let customer_id = create_customer(&db).await;
    let sc = create_credit(&repo, customer_id, dec!(50.00)).await;
    let sc_uuid = sc.id.into_uuid();

    // Negative apply must not mint balance; overdraft rejected.
    assert_validation(repo.apply_async(sc_uuid, dec!(-10.00), None).await.unwrap_err());
    assert_validation(repo.apply_async(sc_uuid, dec!(99.00), None).await.unwrap_err());

    let txn = repo.apply_async(sc_uuid, dec!(30.00), None).await.expect("apply 30");
    assert_eq!(txn.balance_after, dec!(20.00));

    // Voided credits can be neither applied nor adjusted back to life.
    sqlx::query("UPDATE store_credits SET status = 'voided' WHERE id = $1")
        .bind(sc_uuid)
        .execute(db.pool())
        .await
        .expect("void credit");
    assert_validation(repo.apply_async(sc_uuid, dec!(5.00), None).await.unwrap_err());
    assert_validation(
        repo.adjust_async(
            sc_uuid,
            AdjustStoreCredit { amount: dec!(10.00), note: None, reference_id: None },
        )
        .await
        .unwrap_err(),
    );

    let fetched = repo.get_async(sc_uuid).await.expect("get").expect("found");
    assert_eq!(fetched.current_balance, dec!(20.00));
}

#[tokio::test]
async fn postgres_credit_reserve_and_charge_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = stateset_db::postgres::PgCreditRepository::new(db.pool().clone());
    let customer_id = create_customer(&db).await;
    repo.create_credit_account_async(stateset_core::CreateCreditAccount {
        customer_id,
        credit_limit: dec!(100.00),
        currency: None,
        payment_terms: Some("NET30".into()),
        risk_rating: None,
        notes: None,
    })
    .await
    .expect("create credit account");
    let cust = customer_id.into_uuid();

    // Non-positive and over-line reservations rejected.
    assert_validation(
        repo.reserve_credit_async(cust, uuid::Uuid::new_v4(), dec!(-10)).await.unwrap_err(),
    );
    assert_validation(
        repo.reserve_credit_async(cust, uuid::Uuid::new_v4(), dec!(150)).await.unwrap_err(),
    );

    // Reservations respect existing holds.
    let order = uuid::Uuid::new_v4();
    repo.reserve_credit_async(cust, order, dec!(60)).await.expect("reserve 60");
    assert_validation(
        repo.reserve_credit_async(cust, uuid::Uuid::new_v4(), dec!(50)).await.unwrap_err(),
    );

    // Charges respect the limit; a rejected charge keeps its reservation.
    repo.charge_credit_async(cust, order, dec!(60)).await.expect("charge 60");
    assert_validation(
        repo.charge_credit_async(cust, uuid::Uuid::new_v4(), dec!(50)).await.unwrap_err(),
    );
    let acct = repo.get_credit_account_by_customer_async(cust).await.expect("get").expect("found");
    assert_eq!(acct.current_balance, dec!(60.00));
}

#[tokio::test]
async fn postgres_lot_transfer_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = stateset_db::postgres::PgLotRepository::new(db.pool().clone());
    let lot = repo
        .create_async(stateset_core::CreateLot {
            sku: format!("XFER-{}", uuid::Uuid::new_v4()),
            quantity: dec!(5),
            initial_location_id: Some(1),
            ..Default::default()
        })
        .await
        .expect("create lot");

    let base = stateset_core::TransferLot {
        lot_id: lot.id,
        quantity: dec!(10),
        from_location_id: 99,
        to_location_id: 2,
        reason: None,
        performed_by: None,
    };

    // Missing source location must not mint quantity at the destination.
    assert_validation(
        repo.transfer_async(stateset_core::TransferLot { ..base.clone() }).await.unwrap_err(),
    );
    // Short source rejected; non-positive quantity rejected.
    assert_validation(
        repo.transfer_async(stateset_core::TransferLot { from_location_id: 1, ..base.clone() })
            .await
            .unwrap_err(),
    );
    assert_validation(
        repo.transfer_async(stateset_core::TransferLot {
            from_location_id: 1,
            quantity: dec!(-1),
            ..base.clone()
        })
        .await
        .unwrap_err(),
    );

    // A covered transfer still works.
    repo.transfer_async(stateset_core::TransferLot {
        from_location_id: 1,
        quantity: dec!(3),
        ..base
    })
    .await
    .expect("valid transfer");
}

#[tokio::test]
async fn postgres_loyalty_adjust_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = PgLoyaltyProgramRepository::new(db.pool().clone());
    let program = repo
        .create_async(CreateLoyaltyProgram {
            name: format!("Guard Program {}", uuid::Uuid::new_v4()),
            description: None,
            points_per_dollar: 1,
            tiers: vec![],
        })
        .await
        .expect("create program");
    let account = repo
        .enroll_async(EnrollCustomer { customer_id: CustomerId::new(), program_id: program.id })
        .await
        .expect("enroll");

    repo.adjust_points_async(AdjustPoints {
        account_id: account.id,
        points: 50,
        transaction_type: LoyaltyTransactionType::Earn,
        reference_id: None,
        description: None,
    })
    .await
    .expect("earn 50");

    // Overdraft redemption rejected.
    assert_validation(
        repo.adjust_points_async(AdjustPoints {
            account_id: account.id,
            points: -100,
            transaction_type: LoyaltyTransactionType::Redeem,
            reference_id: None,
            description: None,
        })
        .await
        .unwrap_err(),
    );

    // Unknown accounts error instead of minting orphaned transactions.
    let err = repo
        .adjust_points_async(AdjustPoints {
            account_id: LoyaltyAccountId::new(),
            points: 10,
            transaction_type: LoyaltyTransactionType::Earn,
            reference_id: None,
            description: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    let fetched = repo.get_account_async(account.id).await.expect("get account").expect("found");
    assert_eq!(fetched.points_balance, 50);
}

#[tokio::test]
async fn postgres_promotion_per_customer_limit_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = stateset_db::postgres::PgPromotionRepository::new(db.pool().clone());
    let promo = repo
        .create_async(stateset_core::CreatePromotion {
            code: Some(format!("PER-CUST-{}", uuid::Uuid::new_v4())),
            name: "Per customer".into(),
            promotion_type: stateset_core::PromotionType::PercentageOff,
            trigger: stateset_core::PromotionTrigger::CouponCode,
            target: stateset_core::PromotionTarget::Order,
            stacking: stateset_core::StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            per_customer_limit: Some(1),
            ..Default::default()
        })
        .await
        .expect("create promotion");

    let alice = create_customer(&db).await;
    repo.record_usage_async(promo.id, None, Some(alice), None, None, dec!(5.00), "USD")
        .await
        .expect("first use");
    assert_validation(
        repo.record_usage_async(promo.id, None, Some(alice), None, None, dec!(5.00), "USD")
            .await
            .unwrap_err(),
    );

    // A different customer is unaffected.
    let bob = create_customer(&db).await;
    repo.record_usage_async(promo.id, None, Some(bob), None, None, dec!(5.00), "USD")
        .await
        .expect("bob first use");
}

/// End-to-end evaluation gates on the Postgres promotions path: inactive
/// promotions, coupon limits, customer eligibility, and product scoping.
#[tokio::test]
async fn postgres_promotion_evaluation_gates() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = stateset_db::postgres::PgPromotionRepository::new(db.pool().clone());
    let alice = create_customer(&db).await;
    let bob = create_customer(&db).await;

    let request =
        |code: &str, customer: Option<CustomerId>| stateset_core::ApplyPromotionsRequest {
            cart_id: None,
            customer_id: customer,
            coupon_codes: vec![code.to_string()],
            line_items: vec![
                stateset_core::PromotionLineItem {
                    id: "w".into(),
                    product_id: None,
                    variant_id: None,
                    sku: Some("WIDGET".into()),
                    category_ids: vec![],
                    quantity: 1,
                    unit_price: dec!(40.00),
                    line_total: dec!(40.00),
                },
                stateset_core::PromotionLineItem {
                    id: "g".into(),
                    product_id: None,
                    variant_id: None,
                    sku: Some("GADGET".into()),
                    category_ids: vec![],
                    quantity: 1,
                    unit_price: dec!(60.00),
                    line_total: dec!(60.00),
                },
            ],
            subtotal: dec!(100.00),
            shipping_amount: dec!(10.00),
            shipping_country: None,
            shipping_state: None,
            currency: CurrencyCode::USD,
            is_first_order: false,
        };

    // A draft promotion must not apply via its coupon; activation with
    // eligibility + product scoping then behaves exactly like SQLite.
    let code = format!("PG-EVAL-{}", uuid::Uuid::new_v4());
    let promo = repo
        .create_async(stateset_core::CreatePromotion {
            code: Some(code.clone()),
            name: "PG eval gates".into(),
            promotion_type: stateset_core::PromotionType::PercentageOff,
            trigger: stateset_core::PromotionTrigger::CouponCode,
            target: stateset_core::PromotionTarget::Order,
            stacking: stateset_core::StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            applicable_skus: Some(vec!["WIDGET".into()]),
            eligible_customer_ids: Some(vec![alice]),
            ..Default::default()
        })
        .await
        .expect("create promotion");
    let coupon_code = format!("CP-{}", uuid::Uuid::new_v4());
    repo.create_coupon_async(stateset_core::CreateCouponCode {
        promotion_id: promo.id,
        code: coupon_code.clone(),
        usage_limit: None,
        per_customer_limit: None,
        starts_at: None,
        ends_at: None,
        metadata: None,
    })
    .await
    .expect("create coupon");

    // Draft: rejected.
    let result =
        repo.apply_promotions_async(request(&coupon_code, Some(alice))).await.expect("eval");
    assert!(result.applied_promotions.is_empty(), "draft promo must not apply: {result:?}");

    repo.activate_async(promo.id.into_uuid()).await.expect("activate");

    // Ineligible customer: rejected with CustomerNotEligible.
    let result = repo.apply_promotions_async(request(&coupon_code, Some(bob))).await.expect("eval");
    assert!(
        result
            .rejected_promotions
            .iter()
            .any(|r| r.reason_code == stateset_core::RejectionReason::CustomerNotEligible),
        "bob must be rejected: {result:?}"
    );

    // Eligible customer: discount scoped to the WIDGET line (10% of 40.00).
    let result =
        repo.apply_promotions_async(request(&coupon_code, Some(alice))).await.expect("eval");
    assert_eq!(result.applied_promotions.len(), 1, "{result:?}");
    assert_eq!(result.total_discount, dec!(4.00), "scoped to widgets: {result:?}");
}

/// Negative credit limits are rejected on both the open and adjust paths,
/// nothing is written, and zero stays a legitimate (fully restricted) line.
#[tokio::test]
async fn postgres_credit_limit_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = db.credit();
    let customer_id = create_customer(&db).await;
    let cust = customer_id.into_uuid();
    let account = |limit: Decimal| stateset_core::CreateCreditAccount {
        customer_id,
        credit_limit: limit,
        currency: None,
        payment_terms: None,
        risk_rating: None,
        notes: None,
    };

    assert_validation(repo.create_credit_account_async(account(dec!(-100))).await.unwrap_err());
    assert!(
        repo.get_credit_account_by_customer_async(cust).await.expect("get").is_none(),
        "nothing may be written for a rejected account"
    );
    assert!(
        !repo
            .get_over_limit_customers_async()
            .await
            .expect("over limit")
            .iter()
            .any(|a| a.customer_id == customer_id),
        "a rejected account cannot be over limit"
    );

    let zero = repo.create_credit_account_async(account(Decimal::ZERO)).await.expect("zero limit");
    assert_eq!(zero.credit_limit, Decimal::ZERO);

    repo.adjust_credit_limit_async(cust, dec!(500), "approved").await.expect("raise");
    assert_validation(repo.adjust_credit_limit_async(cust, dec!(-1), "bad").await.unwrap_err());
    let acct = repo.get_credit_account_by_customer_async(cust).await.expect("get").expect("found");
    assert_eq!(acct.credit_limit, dec!(500), "limit must be unchanged");
    assert_validation(
        repo.update_credit_account_async(
            acct.id.into_uuid(),
            stateset_core::UpdateCreditAccount {
                credit_limit: Some(dec!(-5)),
                ..Default::default()
            },
        )
        .await
        .unwrap_err(),
    );
    let ledger = repo
        .list_transactions_async(stateset_core::CreditTransactionFilter {
            customer_id: Some(customer_id),
            transaction_type: Some(stateset_core::CreditTransactionType::LimitChange),
            ..Default::default()
        })
        .await
        .expect("ledger");
    assert_eq!(ledger.len(), 1, "only the approved raise may leave a limit_change row");
}

/// Negative cost fields and blank SKUs are rejected by `set_item_cost` and
/// `update_average_cost`; a rejected write leaves existing costs untouched.
#[tokio::test]
async fn postgres_item_cost_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = db.cost_accounting();
    let uniq = uuid::Uuid::new_v4().simple().to_string();
    let sku = |tag: &str| format!("{tag}-{}", &uniq[..8]);

    for input in [
        stateset_core::SetItemCost {
            sku: sku("NEG-STD"),
            standard_cost: Some(dec!(-1)),
            ..Default::default()
        },
        stateset_core::SetItemCost {
            sku: sku("NEG-MAT"),
            material_cost: Some(dec!(-0.01)),
            ..Default::default()
        },
        stateset_core::SetItemCost {
            sku: sku("NEG-LAB"),
            labor_cost: Some(dec!(-5)),
            ..Default::default()
        },
        stateset_core::SetItemCost {
            sku: sku("NEG-OVH"),
            overhead_cost: Some(dec!(-5)),
            ..Default::default()
        },
        stateset_core::SetItemCost {
            sku: String::new(),
            standard_cost: Some(dec!(1)),
            ..Default::default()
        },
        stateset_core::SetItemCost {
            sku: "   ".into(),
            standard_cost: Some(dec!(1)),
            ..Default::default()
        },
    ] {
        let s = input.sku.clone();
        assert_validation(repo.set_item_cost_async(input).await.unwrap_err());
        assert!(repo.get_item_cost_async(&s).await.expect("get").is_none(), "sku {s:?} written");
    }

    let good = sku("GOOD");
    repo.set_item_cost_async(stateset_core::SetItemCost {
        sku: good.clone(),
        standard_cost: Some(dec!(10)),
        ..Default::default()
    })
    .await
    .expect("good");
    assert_validation(
        repo.set_item_cost_async(stateset_core::SetItemCost {
            sku: good.clone(),
            standard_cost: Some(dec!(-10)),
            ..Default::default()
        })
        .await
        .unwrap_err(),
    );
    let kept = repo.get_item_cost_async(&good).await.expect("get").expect("found");
    assert_eq!(kept.standard_cost, dec!(10));

    let free = repo
        .set_item_cost_async(stateset_core::SetItemCost {
            sku: sku("FREE"),
            standard_cost: Some(Decimal::ZERO),
            ..Default::default()
        })
        .await
        .expect("zero is a legitimate cost");
    assert_eq!(free.standard_cost, Decimal::ZERO);

    // update_average_cost: the seed path must not create a record from a
    // negative cost, and an existing average/last cost must stay put.
    let fresh = sku("AVG-NEW");
    assert_validation(
        repo.update_average_cost_async(&fresh, dec!(10), dec!(-2)).await.unwrap_err(),
    );
    assert!(repo.get_item_cost_async(&fresh).await.expect("get").is_none());
    assert_validation(repo.update_average_cost_async(&good, dec!(10), dec!(-2)).await.unwrap_err());
    let kept = repo.get_item_cost_async(&good).await.expect("get").expect("found");
    assert_eq!(kept.average_cost, dec!(10));
    assert_eq!(kept.last_cost, dec!(10));
}

/// A credit memo needs a real customer, and a voided memo is no longer
/// applicable credit.
#[tokio::test]
async fn postgres_credit_memo_guards() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let repo = db.accounts_receivable();
    let memo = |customer_id: uuid::Uuid, amount: Decimal| stateset_core::CreateCreditMemo {
        customer_id,
        original_invoice_id: None,
        reason: stateset_core::CreditMemoReason::ReturnedGoods,
        amount,
        notes: None,
    };

    let ghost = uuid::Uuid::new_v4();
    let err = repo.create_credit_memo_async(memo(ghost, dec!(25))).await.unwrap_err();
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    let listed = repo
        .list_credit_memos_async(stateset_core::CreditMemoFilter {
            customer_id: Some(ghost),
            ..Default::default()
        })
        .await
        .expect("list");
    assert!(listed.is_empty(), "nothing may be written for a rejected memo");

    let cust = create_customer(&db).await.into_uuid();
    let voided = repo.create_credit_memo_async(memo(cust, dec!(30))).await.expect("memo");
    let open = repo.create_credit_memo_async(memo(cust, dec!(70))).await.expect("memo");
    repo.void_credit_memo_async(voided.id).await.expect("void");

    let unapplied = repo.get_unapplied_credits_async(cust).await.expect("unapplied");
    assert_eq!(
        unapplied.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![open.id],
        "a voided memo is not applicable credit"
    );
    let fetched = repo.get_credit_memo_async(voided.id).await.expect("get").expect("found");
    assert!(!fetched.can_apply());
}
