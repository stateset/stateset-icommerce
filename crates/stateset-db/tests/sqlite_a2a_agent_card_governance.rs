//! Agent cards govern who may buy and sell in A2A commerce.
//!
//! `create_quote` / `create_purchase` used to accept any `buyer_agent_id` /
//! `seller_agent_id` UUIDs with no link to `agent_cards`, so
//! `AgentCard::can_buy` / `can_sell` were never enforced. The buyer must now
//! resolve to an active card that can buy and the seller to one that can sell,
//! checked inside the insert's write transaction.
#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2ACommerceRepository, A2APurchaseFilter, A2ASkill, AgentCardRepository, CommerceError,
    CreateA2APurchase, CreateA2AQuote, CreateAgentCard, ItemAvailability, QuotedItem,
    SkillQuoteFilter,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn agent(db: &SqliteDatabase, skills: Vec<A2ASkill>) -> Uuid {
    db.agent_cards()
        .create(CreateAgentCard {
            name: "governance agent".into(),
            wallet_address: format!("0xagent-{}", Uuid::new_v4().as_simple()),
            public_key: "test-public-key".into(),
            a2a_skills: Some(skills),
            ..Default::default()
        })
        .expect("register agent card")
        .id
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
fn assert_refused(db: &SqliteDatabase, buyer: Uuid, seller: Uuid, side: &str, why: &str) {
    for result in [
        db.a2a_quotes().create_quote(quote(buyer, seller)).map(|_| ()),
        db.a2a_purchases().create_purchase(purchase(buyer, seller)).map(|_| ()),
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
        .count_quotes(SkillQuoteFilter { buyer_agent_id: Some(buyer), ..Default::default() })
        .expect("count quotes");
    let purchases = db
        .a2a_purchases()
        .count_purchases(A2APurchaseFilter { buyer_agent_id: Some(buyer), ..Default::default() })
        .expect("count purchases");
    assert_eq!((quotes, purchases), (0, 0), "{why}: nothing may be written");
}

#[test]
fn sqlite_a2a_unregistered_agents_cannot_trade() {
    let db = SqliteDatabase::in_memory().expect("db");
    let buyer = agent(&db, vec![A2ASkill::Buy]);
    let seller = agent(&db, vec![A2ASkill::Sell]);
    assert_refused(&db, Uuid::new_v4(), seller, "buyer_agent_id", "unknown buyer");
    assert_refused(&db, buyer, Uuid::new_v4(), "seller_agent_id", "unknown seller");
}

#[test]
fn sqlite_a2a_buyer_card_must_be_able_to_buy() {
    let db = SqliteDatabase::in_memory().expect("db");
    let seller = agent(&db, vec![A2ASkill::Sell]);
    let seller_only = agent(&db, vec![A2ASkill::Sell, A2ASkill::Quote]);
    assert_refused(&db, seller_only, seller, "buyer_agent_id", "seller-only card as buyer");
    let no_skills = agent(&db, vec![]);
    assert_refused(&db, no_skills, seller, "buyer_agent_id", "card without skills as buyer");
}

#[test]
fn sqlite_a2a_seller_card_must_be_able_to_sell() {
    let db = SqliteDatabase::in_memory().expect("db");
    let buyer = agent(&db, vec![A2ASkill::Buy]);
    let buyer_only = agent(&db, vec![A2ASkill::Buy, A2ASkill::RequestQuote]);
    assert_refused(&db, buyer, buyer_only, "seller_agent_id", "buyer-only card as seller");
}

#[test]
fn sqlite_a2a_suspended_seller_cannot_trade() {
    let db = SqliteDatabase::in_memory().expect("db");
    let buyer = agent(&db, vec![A2ASkill::Buy]);
    let seller = agent(&db, vec![A2ASkill::Sell]);
    db.agent_cards().suspend(seller, "fraud review").expect("suspend");
    assert_refused(&db, buyer, seller, "seller_agent_id", "suspended seller");
}

#[test]
fn sqlite_a2a_permitted_agents_can_quote_and_purchase() {
    let db = SqliteDatabase::in_memory().expect("db");
    // request_quote / quote count as buy / sell.
    let buyer = agent(&db, vec![A2ASkill::RequestQuote]);
    let seller = agent(&db, vec![A2ASkill::Quote]);
    let created = db.a2a_quotes().create_quote(quote(buyer, seller)).expect("quote");
    assert_eq!((created.buyer_agent_id, created.seller_agent_id), (buyer, seller));
    let bought = db.a2a_purchases().create_purchase(purchase(buyer, seller)).expect("purchase");
    assert_eq!((bought.buyer_agent_id, bought.seller_agent_id), (buyer, seller));
}
