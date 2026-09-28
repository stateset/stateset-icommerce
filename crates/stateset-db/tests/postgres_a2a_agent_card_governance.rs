//! Agent cards govern who may buy and sell in A2A commerce.
//!
//! `create_quote` / `create_purchase` used to accept any `buyer_agent_id` /
//! `seller_agent_id` UUIDs with no link to `agent_cards`, so
//! `AgentCard::can_buy` / `can_sell` were never enforced. The buyer must now
//! resolve to an active card that can buy and the seller to one that can sell,
//! checked inside the insert's write transaction.
//!
//! Postgres twin of `sqlite_a2a_agent_card_governance.rs` (cards are read
//! `FOR SHARE`). Requires a live Postgres instance (`POSTGRES_URL` /
//! `DATABASE_URL`); skipped otherwise.
#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2APurchaseFilter, A2ASkill, CommerceError, CreateA2APurchase, CreateA2AQuote, CreateAgentCard,
    ItemAvailability, QuotedItem, SkillQuoteFilter,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

async fn agent(db: &PostgresDatabase, skills: Vec<A2ASkill>) -> Uuid {
    db.agent_cards()
        .create_async(CreateAgentCard {
            name: "governance agent".into(),
            wallet_address: format!("0xagent-{}", Uuid::new_v4().as_simple()),
            public_key: "test-public-key".into(),
            a2a_skills: Some(skills),
            ..Default::default()
        })
        .await
        .expect("register agent card")
        .id
}

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) =
        std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
    else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
}

fn item() -> QuotedItem {
    QuotedItem {
        line_number: 1,
        sku: Some("SKU-1".into()),
        name: "Widget".into(),
        quantity: 1,
        unit_price: dec!(10),
        total: dec!(10),
        availability: ItemAvailability::InStock,
        lead_time_days: None,
    }
}

fn quote(buyer: Uuid, seller: Uuid) -> CreateA2AQuote {
    CreateA2AQuote {
        buyer_agent_id: buyer,
        seller_agent_id: seller,
        items: vec![item()],
        subtotal: dec!(10),
        total: dec!(10),
        valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
        ..Default::default()
    }
}

fn purchase(buyer: Uuid, seller: Uuid) -> CreateA2APurchase {
    CreateA2APurchase {
        buyer_agent_id: buyer,
        seller_agent_id: seller,
        items: vec![item()],
        total: dec!(10),
        ..Default::default()
    }
}

/// Refused with a validation error naming `side`, and nothing was written.
async fn assert_refused(db: &PostgresDatabase, buyer: Uuid, seller: Uuid, side: &str, why: &str) {
    for result in [
        db.a2a_quotes().create_quote_async(quote(buyer, seller)).await.map(|_| ()),
        db.a2a_purchases().create_purchase_async(purchase(buyer, seller)).await.map(|_| ()),
    ] {
        match result {
            Err(CommerceError::ValidationError(message)) => {
                assert!(message.contains(side), "{why}: message must name {side}: {message}");
            }
            other => panic!("{why}: expected ValidationError naming {side}, got {other:?}"),
        }
    }
    let quotes = db
        .a2a_quotes()
        .count_quotes_async(SkillQuoteFilter { buyer_agent_id: Some(buyer), ..Default::default() })
        .await
        .expect("count quotes");
    let purchases = db
        .a2a_purchases()
        .count_purchases_async(A2APurchaseFilter {
            buyer_agent_id: Some(buyer),
            ..Default::default()
        })
        .await
        .expect("count purchases");
    assert_eq!((quotes, purchases), (0, 0), "{why}: nothing may be written");
}

#[tokio::test]
async fn postgres_a2a_unregistered_agents_cannot_trade() {
    let Some(db) = connect().await else { return };
    let buyer = agent(&db, vec![A2ASkill::Buy]).await;
    let seller = agent(&db, vec![A2ASkill::Sell]).await;
    assert_refused(&db, Uuid::new_v4(), seller, "buyer_agent_id", "unknown buyer").await;
    assert_refused(&db, buyer, Uuid::new_v4(), "seller_agent_id", "unknown seller").await;
}

#[tokio::test]
async fn postgres_a2a_buyer_card_must_be_able_to_buy() {
    let Some(db) = connect().await else { return };
    let seller = agent(&db, vec![A2ASkill::Sell]).await;
    let seller_only = agent(&db, vec![A2ASkill::Sell, A2ASkill::Quote]).await;
    assert_refused(&db, seller_only, seller, "buyer_agent_id", "seller-only card as buyer").await;
    let no_skills = agent(&db, vec![]).await;
    assert_refused(&db, no_skills, seller, "buyer_agent_id", "card without skills as buyer").await;
}

#[tokio::test]
async fn postgres_a2a_seller_card_must_be_able_to_sell() {
    let Some(db) = connect().await else { return };
    let buyer = agent(&db, vec![A2ASkill::Buy]).await;
    let buyer_only = agent(&db, vec![A2ASkill::Buy, A2ASkill::RequestQuote]).await;
    assert_refused(&db, buyer, buyer_only, "seller_agent_id", "buyer-only card as seller").await;
}

#[tokio::test]
async fn postgres_a2a_suspended_seller_cannot_trade() {
    let Some(db) = connect().await else { return };
    let buyer = agent(&db, vec![A2ASkill::Buy]).await;
    let seller = agent(&db, vec![A2ASkill::Sell]).await;
    db.agent_cards().suspend_async(seller, "fraud review").await.expect("suspend");
    assert_refused(&db, buyer, seller, "seller_agent_id", "suspended seller").await;
}

#[tokio::test]
async fn postgres_a2a_permitted_agents_can_quote_and_purchase() {
    let Some(db) = connect().await else { return };
    // request_quote / quote count as buy / sell.
    let buyer = agent(&db, vec![A2ASkill::RequestQuote]).await;
    let seller = agent(&db, vec![A2ASkill::Quote]).await;
    let created = db.a2a_quotes().create_quote_async(quote(buyer, seller)).await.expect("quote");
    assert_eq!((created.buyer_agent_id, created.seller_agent_id), (buyer, seller));
    let bought =
        db.a2a_purchases().create_purchase_async(purchase(buyer, seller)).await.expect("purchase");
    assert_eq!((bought.buyer_agent_id, bought.seller_agent_id), (buyer, seller));
}
