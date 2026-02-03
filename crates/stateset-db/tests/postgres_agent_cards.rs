#[cfg(feature = "postgres")]
use stateset_core::{
    A2ASkill, AgentCardFilter, CreateAgentCard, TrustLevel, X402Asset, X402Network,
};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL")
        .ok()
        .or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_agent_cards_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres agent card test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url)
        .await
        .expect("connect to postgres and run migrations");

    let repo = db.agent_cards();
    let wallet = format!("0xwallet{}", Uuid::new_v4().to_string().replace('-', ""));

    let card = repo
        .create_async(CreateAgentCard {
            name: "Test Agent".to_string(),
            description: Some("test agent card".to_string()),
            wallet_address: wallet.clone(),
            public_key: "pubkey-test".to_string(),
            supported_networks: Some(vec![X402Network::SetChain, X402Network::Base]),
            supported_assets: Some(vec![X402Asset::Usdc, X402Asset::Dai]),
            a2a_skills: Some(vec![A2ASkill::Sell, A2ASkill::Quote]),
            trust_level: Some(TrustLevel::Sandbox),
            endpoint_url: Some("https://example.com/a2a".to_string()),
            endpoint_protocol: Some("https".to_string()),
            merchant_id: Some("merchant-test".to_string()),
            merchant_name: Some("Merchant Test".to_string()),
            business_category: Some("testing".to_string()),
            max_transaction_amount: Some(1_000_000),
            daily_volume_limit: Some(5_000_000),
            requires_kyc: Some(false),
            metadata: Some("{\"env\":\"test\"}".to_string()),
        })
        .await
        .expect("create agent card");

    assert_eq!(card.wallet_address, wallet);
    assert!(card.active);

    let by_wallet = repo
        .get_by_wallet_async(&wallet)
        .await
        .expect("get by wallet")
        .expect("agent card for wallet");
    assert_eq!(by_wallet.id, card.id);

    let filtered = repo
        .list_async(AgentCardFilter {
            network: Some(X402Network::SetChain),
            asset: Some(X402Asset::Usdc),
            skill: Some(A2ASkill::Sell),
            active: Some(true),
            merchant_id: Some("merchant-test".to_string()),
            ..Default::default()
        })
        .await
        .expect("list agent cards");
    assert!(filtered.iter().any(|c| c.id == card.id));

    let count = repo
        .count_async(AgentCardFilter {
            network: Some(X402Network::SetChain),
            asset: Some(X402Asset::Usdc),
            skill: Some(A2ASkill::Sell),
            active: Some(true),
            merchant_id: Some("merchant-test".to_string()),
            ..Default::default()
        })
        .await
        .expect("count agent cards");
    assert!(count >= 1);

    let verified = repo
        .verify_async(card.id, TrustLevel::Verified, "test")
        .await
        .expect("verify agent card");
    assert_eq!(verified.trust_level, TrustLevel::Verified);
    assert!(verified.verified_at.is_some());

    let suspended = repo
        .suspend_async(card.id, "test")
        .await
        .expect("suspend agent card");
    assert!(!suspended.active);

    let reactivated = repo
        .reactivate_async(card.id)
        .await
        .expect("reactivate agent card");
    assert!(reactivated.active);
}
