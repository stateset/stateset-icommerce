#![cfg(feature = "postgres")]
//! PostgreSQL twins of two SQLite promotion tests: the usage ledger is
//! readable by order / promotion / customer (with pagination), and a failed
//! condition is reported with the rejection code of its class.

use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, ConditionOperator, ConditionType, CreateCustomer, CreateOrder,
    CreateOrderItem, CreatePromotion, CreatePromotionCondition, CurrencyCode, CustomerId, Order,
    OrderStatus, ProductId, PromotionId, PromotionLineItem, PromotionTrigger, PromotionType,
    PromotionUsageFilter, RejectionReason, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let url = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

macro_rules! require_db {
    () => {
        match connect().await {
            Some(db) => db,
            None => {
                eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

async fn customer(db: &PostgresDatabase) -> CustomerId {
    db.customers()
        .create_async(CreateCustomer {
            email: format!("pg-follow-{}@example.com", Uuid::new_v4()),
            first_name: "Pg".into(),
            last_name: "Follow".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
        .id
}

/// A two-unit, single-line order advanced to `Processing`.
async fn processing_order(db: &PostgresDatabase) -> Order {
    let customer_id = customer(db).await;
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("PG-FOLLOW-{}", Uuid::new_v4()),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update_async(
                order.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .expect("advance status");
    }
    order
}

#[tokio::test]
async fn postgres_list_usage_filters_the_ledger() {
    let db = require_db!();
    let promos = db.promotions();
    let tag = Uuid::new_v4().simple().to_string();
    let make = |code: String| CreatePromotion {
        code: Some(code),
        name: "Ledger".into(),
        promotion_type: PromotionType::PercentageOff,
        percentage_off: Some(dec!(0.10)),
        ..Default::default()
    };
    let ten = promos.create_async(make(format!("LEDGER10-{tag}"))).await.expect("promo 1");
    let five = promos.create_async(make(format!("LEDGER5-{tag}"))).await.expect("promo 2");
    let order_a = processing_order(&db).await;
    let order_b = processing_order(&db).await;
    let (alice, bob) = (order_a.customer_id, order_b.customer_id);

    promos
        .record_usage_async(ten.id, None, Some(alice), Some(order_a.id), None, dec!(2.00), "USD")
        .await
        .expect("use 1");
    promos
        .record_usage_async(five.id, None, Some(alice), Some(order_a.id), None, dec!(1.00), "USD")
        .await
        .expect("use 2");
    promos
        .record_usage_async(ten.id, None, Some(bob), Some(order_b.id), None, dec!(3.00), "USD")
        .await
        .expect("use 3");

    let for_a = promos
        .list_usage_async(PromotionUsageFilter { order_id: Some(order_a.id), ..Default::default() })
        .await
        .expect("by order");
    assert_eq!(for_a.len(), 2);
    let mut amounts: Vec<_> = for_a.iter().map(|u| u.discount_amount).collect();
    amounts.sort();
    assert_eq!(amounts, vec![dec!(1.00), dec!(2.00)]);

    let ten_uses = promos
        .list_usage_async(PromotionUsageFilter { promotion_id: Some(ten.id), ..Default::default() })
        .await
        .expect("by promotion");
    assert_eq!(ten_uses.len(), 2);

    let ten_bob = promos
        .list_usage_async(PromotionUsageFilter {
            promotion_id: Some(ten.id),
            customer_id: Some(bob),
            ..Default::default()
        })
        .await
        .expect("by promotion and customer");
    assert_eq!(ten_bob.len(), 1);
    assert_eq!(ten_bob[0].order_id, Some(order_b.id));

    let page = promos
        .list_usage_async(PromotionUsageFilter {
            promotion_id: Some(ten.id),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .expect("page");
    let rest = promos
        .list_usage_async(PromotionUsageFilter {
            promotion_id: Some(ten.id),
            offset: Some(1),
            ..Default::default()
        })
        .await
        .expect("rest");
    assert_eq!((page.len(), rest.len()), (1, 1));
    assert_ne!(page[0].id, rest[0].id);
}

#[tokio::test]
async fn postgres_failed_conditions_carry_a_class_specific_rejection_code() {
    let db = require_db!();
    let promos = db.promotions();
    let tag = Uuid::new_v4().simple().to_string();
    let mut ids = Vec::new();
    for (name, condition_type, operator, value) in [
        ("FIRST", ConditionType::FirstOrder, ConditionOperator::Equals, "true"),
        ("SKU", ConditionType::SkuInCart, ConditionOperator::In, "OTHER"),
        ("MIN", ConditionType::MinimumSubtotal, ConditionOperator::GreaterThanOrEqual, "1000"),
    ] {
        let promo = promos
            .create_async(CreatePromotion {
                code: Some(format!("RC-{name}-{tag}")),
                name: name.into(),
                promotion_type: PromotionType::PercentageOff,
                trigger: PromotionTrigger::Automatic,
                percentage_off: Some(dec!(0.10)),
                conditions: Some(vec![CreatePromotionCondition {
                    condition_type,
                    operator,
                    value: value.into(),
                    is_required: true,
                }]),
                // Evaluated before any promotion another test left active in
                // the shared database (an exclusive one would otherwise
                // refuse these as NotStackable first).
                priority: Some(i32::MIN),
                ..Default::default()
            })
            .await
            .expect("create promo");
        promos.activate_async(promo.id.into_uuid()).await.expect("activate");
        ids.push(promo.id);
    }
    let result = promos
        .apply_promotions_async(ApplyPromotionsRequest {
            line_items: vec![PromotionLineItem {
                id: "MINE".into(),
                product_id: None,
                variant_id: None,
                sku: Some("MINE".into()),
                category_ids: vec![],
                quantity: 1,
                unit_price: dec!(100.00),
                line_total: dec!(100.00),
            }],
            subtotal: dec!(100.00),
            currency: CurrencyCode::USD,
            ..Default::default()
        })
        .await
        .expect("eval");
    let code_for = |id: PromotionId| {
        result
            .rejected_promotions
            .iter()
            .find(|r| r.promotion_id == Some(id))
            .map(|r| r.reason_code)
            .unwrap_or_else(|| panic!("{id} not rejected"))
    };
    assert_eq!(code_for(ids[0]), RejectionReason::CustomerNotEligible);
    assert_eq!(code_for(ids[1]), RejectionReason::ProductNotEligible);
    assert_eq!(code_for(ids[2]), RejectionReason::MinimumNotMet);
}
