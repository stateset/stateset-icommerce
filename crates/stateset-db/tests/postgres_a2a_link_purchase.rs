//! Postgres twin of `sqlite_a2a_link_purchase.rs`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.
#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2ASkill, CommerceError, CreateA2APurchase, CreateAgentCard, ItemAvailability, PurchaseStatus,
    QuotedItem,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

/// Register an agent card that may buy (A2A quotes/purchases require one).
async fn buyer(db: &PostgresDatabase) -> Uuid {
    agent(db, A2ASkill::Buy).await
}

/// Register an agent card that may sell.
async fn seller(db: &PostgresDatabase) -> Uuid {
    agent(db, A2ASkill::Sell).await
}

async fn agent(db: &PostgresDatabase, skill: A2ASkill) -> Uuid {
    db.agent_cards()
        .create_async(CreateAgentCard {
            name: format!("{skill} agent"),
            wallet_address: format!("0xagent-{}", Uuid::new_v4().as_simple()),
            public_key: "test-public-key".into(),
            a2a_skills: Some(vec![skill]),
            ..Default::default()
        })
        .await
        .expect("register agent card")
        .id
}

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn purchase(db: &PostgresDatabase) -> Uuid {
    db.a2a_purchases()
        .create_purchase_async(CreateA2APurchase {
            buyer_agent_id: buyer(db).await,
            seller_agent_id: seller(db).await,
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-1".into()),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(10),
                total: dec!(10),
                availability: ItemAvailability::InStock,
                lead_time_days: None,
            }],
            total: dec!(10),
            ..Default::default()
        })
        .await
        .expect("create purchase")
        .id
}

#[tokio::test]
async fn postgres_link_purchase_to_order_is_conditional() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.a2a_purchases();
    let id = purchase(&db).await;
    let order = Uuid::new_v4();

    let linked = repo.link_purchase_to_order_async(id, order).await.expect("link");
    assert_eq!(linked.order_id, Some(order));
    let again = repo.link_purchase_to_order_async(id, order).await.expect("idempotent");
    assert_eq!(again.order_id, Some(order));
    let err = repo.link_purchase_to_order_async(id, Uuid::new_v4()).await.expect_err("relink");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(repo.get_purchase_async(id).await.unwrap().unwrap().order_id, Some(order));
    let err = repo.link_purchase_to_order_async(Uuid::new_v4(), order).await.expect_err("missing");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");

    let cancelled = purchase(&db).await;
    repo.update_purchase_status_async(cancelled, PurchaseStatus::Cancelled).await.expect("cancel");
    let err = repo.link_purchase_to_order_async(cancelled, order).await.expect_err("cancelled");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(repo.get_purchase_async(cancelled).await.unwrap().unwrap().order_id, None);
}
