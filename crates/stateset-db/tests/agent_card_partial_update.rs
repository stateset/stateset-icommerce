//! `agent_cards::update` must be a partial write, not a read-merge-write.
//!
//! Both backends used to load the whole card on one connection, fold the
//! caller's `Option` fields over that snapshot in Rust, and then write every
//! column back. Two agents editing different fields of the same card at once
//! therefore each wrote the OTHER field's stale value, and whichever committed
//! last silently reverted its neighbour's edit — a lost update with both
//! callers told they had succeeded.
//!
//! The repair sends one `SET col = COALESCE($n, col)` statement, so a field the
//! caller did not name is never read and never rewritten. `a2a_skills` keeps
//! its "an explicit empty list clears the column" semantics through a
//! CASE/flag pair rather than COALESCE.
//!
//! The SQLite half runs everywhere; the Postgres half needs a live instance
//! (`POSTGRES_URL` / `DATABASE_URL`) and is skipped otherwise.

use stateset_core::{
    A2ASkill, CreateAgentCard, TrustLevel, UpdateAgentCard, X402Asset, X402Network,
};
use uuid::Uuid;

fn new_card(name: &str) -> CreateAgentCard {
    CreateAgentCard {
        name: name.to_string(),
        description: Some("original description".to_string()),
        wallet_address: format!("0xwallet{}", Uuid::new_v4().to_string().replace('-', "")),
        public_key: "pubkey-partial-update".to_string(),
        supported_networks: Some(vec![X402Network::SetChain]),
        supported_assets: Some(vec![X402Asset::Usdc]),
        a2a_skills: Some(vec![A2ASkill::Sell]),
        trust_level: Some(TrustLevel::Sandbox),
        endpoint_url: Some("https://example.com/a2a".to_string()),
        endpoint_protocol: Some("https".to_string()),
        merchant_id: Some("merchant-original".to_string()),
        merchant_name: Some("Original Merchant".to_string()),
        business_category: Some("testing".to_string()),
        max_transaction_amount: Some(1_000_000),
        daily_volume_limit: Some(5_000_000),
        requires_kyc: Some(false),
        metadata: Some("{\"env\":\"test\"}".to_string()),
    }
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use super::{A2ASkill, UpdateAgentCard, Uuid, new_card};
    use stateset_core::{AgentCardRepository, CommerceError};
    use stateset_db::{DatabaseConfig, SqliteDatabase};
    use std::sync::{Arc, Barrier};

    fn test_db() -> Arc<SqliteDatabase> {
        Arc::new(
            SqliteDatabase::new(&DatabaseConfig { url: ":memory:".into(), max_connections: 8 })
                .expect("in-memory db"),
        )
    }

    /// Two agents editing different fields of one card at the same time: both
    /// edits must survive.
    #[test]
    fn sqlite_concurrent_partial_updates_both_land() {
        let db = test_db();

        for trial in 0..12 {
            let card = db.agent_cards().create(new_card("Original Name")).expect("create card");
            let barrier = Arc::new(Barrier::new(2));

            let renamer = {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                let id = card.id;
                std::thread::spawn(move || {
                    barrier.wait();
                    db.agent_cards().update(
                        id,
                        UpdateAgentCard {
                            name: Some("Renamed Agent".into()),
                            ..Default::default()
                        },
                    )
                })
            };
            let rebrander = {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                let id = card.id;
                std::thread::spawn(move || {
                    barrier.wait();
                    db.agent_cards().update(
                        id,
                        UpdateAgentCard {
                            merchant_name: Some("Rebranded Merchant".into()),
                            ..Default::default()
                        },
                    )
                })
            };

            renamer.join().expect("renamer thread").expect("rename");
            rebrander.join().expect("rebrander thread").expect("rebrand");

            let after = db.agent_cards().get(card.id).expect("get").expect("exists");
            assert_eq!(after.name, "Renamed Agent", "trial {trial}: the rename was lost");
            assert_eq!(
                after.merchant_name.as_deref(),
                Some("Rebranded Merchant"),
                "trial {trial}: the rebrand was lost"
            );
            // Fields nobody touched are untouched.
            assert_eq!(after.description.as_deref(), Some("original description"));
            assert_eq!(after.merchant_id.as_deref(), Some("merchant-original"));
            assert_eq!(after.max_transaction_amount, Some(1_000_000));
        }
    }

    /// A partial update leaves everything it did not name exactly as it was,
    /// and an explicit empty skill list still clears the column.
    #[test]
    fn sqlite_partial_update_preserves_unnamed_fields() {
        let db = test_db();
        let card = db.agent_cards().create(new_card("Keeper")).expect("create card");

        let updated = db
            .agent_cards()
            .update(
                card.id,
                UpdateAgentCard { description: Some("edited".into()), ..Default::default() },
            )
            .expect("update");
        assert_eq!(updated.name, "Keeper");
        assert_eq!(updated.description.as_deref(), Some("edited"));
        assert_eq!(updated.a2a_skills, vec![A2ASkill::Sell]);
        assert_eq!(updated.supported_networks, card.supported_networks);
        assert_eq!(updated.supported_assets, card.supported_assets);
        assert_eq!(updated.trust_level, card.trust_level);
        assert_eq!(updated.endpoint_url, card.endpoint_url);
        assert_eq!(updated.endpoint_protocol, card.endpoint_protocol);
        assert_eq!(updated.merchant_id, card.merchant_id);
        assert_eq!(updated.merchant_name, card.merchant_name);
        assert_eq!(updated.business_category, card.business_category);
        assert_eq!(updated.max_transaction_amount, card.max_transaction_amount);
        assert_eq!(updated.daily_volume_limit, card.daily_volume_limit);
        assert_eq!(updated.requires_kyc, card.requires_kyc);
        assert_eq!(updated.active, card.active);
        assert_eq!(updated.metadata, card.metadata);

        let cleared = db
            .agent_cards()
            .update(card.id, UpdateAgentCard { a2a_skills: Some(vec![]), ..Default::default() })
            .expect("clear skills");
        assert!(cleared.a2a_skills.is_empty(), "an explicit empty list clears the column");
        assert_eq!(cleared.description.as_deref(), Some("edited"), "the earlier edit survives");
    }

    /// Updating a card that does not exist is still `NotFound`.
    #[test]
    fn sqlite_update_of_a_missing_card_is_not_found() {
        let db = test_db();
        let err = db
            .agent_cards()
            .update(
                Uuid::new_v4(),
                UpdateAgentCard { name: Some("ghost".into()), ..Default::default() },
            )
            .expect_err("missing card");
        assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    }
}

#[cfg(feature = "postgres")]
mod postgres {
    use super::{A2ASkill, UpdateAgentCard, Uuid, new_card};
    use stateset_core::CommerceError;
    use stateset_db::PostgresDatabase;
    use std::sync::Arc;
    use tokio::sync::Barrier;

    fn postgres_url() -> Option<String> {
        std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
    }

    async fn connect() -> Option<Arc<PostgresDatabase>> {
        let url = postgres_url()?;
        Some(Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate")))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn postgres_concurrent_partial_updates_both_land() {
        let Some(db) = connect().await else {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        };

        for trial in 0..8 {
            let card =
                db.agent_cards().create_async(new_card("Original Name")).await.expect("create");
            let barrier = Arc::new(Barrier::new(2));

            let renamer = {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                let id = card.id;
                tokio::spawn(async move {
                    barrier.wait().await;
                    db.agent_cards()
                        .update_async(
                            id,
                            UpdateAgentCard {
                                name: Some("Renamed Agent".into()),
                                ..Default::default()
                            },
                        )
                        .await
                })
            };
            let rebrander = {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                let id = card.id;
                tokio::spawn(async move {
                    barrier.wait().await;
                    db.agent_cards()
                        .update_async(
                            id,
                            UpdateAgentCard {
                                merchant_name: Some("Rebranded Merchant".into()),
                                ..Default::default()
                            },
                        )
                        .await
                })
            };

            renamer.await.expect("renamer task").expect("rename");
            rebrander.await.expect("rebrander task").expect("rebrand");

            let after = db.agent_cards().get_async(card.id).await.expect("get").expect("exists");
            assert_eq!(after.name, "Renamed Agent", "trial {trial}: the rename was lost");
            assert_eq!(
                after.merchant_name.as_deref(),
                Some("Rebranded Merchant"),
                "trial {trial}: the rebrand was lost"
            );
            assert_eq!(after.description.as_deref(), Some("original description"));
            assert_eq!(after.merchant_id.as_deref(), Some("merchant-original"));
            assert_eq!(after.max_transaction_amount, Some(1_000_000));
        }
    }

    #[tokio::test]
    async fn postgres_partial_update_preserves_unnamed_fields() {
        let Some(db) = connect().await else {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        };
        let card = db.agent_cards().create_async(new_card("Keeper")).await.expect("create");

        let updated = db
            .agent_cards()
            .update_async(
                card.id,
                UpdateAgentCard { description: Some("edited".into()), ..Default::default() },
            )
            .await
            .expect("update");
        assert_eq!(updated.name, "Keeper");
        assert_eq!(updated.description.as_deref(), Some("edited"));
        assert_eq!(updated.a2a_skills, vec![A2ASkill::Sell]);
        assert_eq!(updated.supported_networks, card.supported_networks);
        assert_eq!(updated.supported_assets, card.supported_assets);
        assert_eq!(updated.trust_level, card.trust_level);
        assert_eq!(updated.endpoint_url, card.endpoint_url);
        assert_eq!(updated.endpoint_protocol, card.endpoint_protocol);
        assert_eq!(updated.merchant_id, card.merchant_id);
        assert_eq!(updated.merchant_name, card.merchant_name);
        assert_eq!(updated.business_category, card.business_category);
        assert_eq!(updated.max_transaction_amount, card.max_transaction_amount);
        assert_eq!(updated.daily_volume_limit, card.daily_volume_limit);
        assert_eq!(updated.requires_kyc, card.requires_kyc);
        assert_eq!(updated.active, card.active);
        assert_eq!(updated.metadata, card.metadata);

        let cleared = db
            .agent_cards()
            .update_async(
                card.id,
                UpdateAgentCard { a2a_skills: Some(vec![]), ..Default::default() },
            )
            .await
            .expect("clear skills");
        assert!(cleared.a2a_skills.is_empty(), "an explicit empty list clears the column");
        assert_eq!(cleared.description.as_deref(), Some("edited"), "the earlier edit survives");
    }

    #[tokio::test]
    async fn postgres_update_of_a_missing_card_is_not_found() {
        let Some(db) = connect().await else {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        };
        let err = db
            .agent_cards()
            .update_async(
                Uuid::new_v4(),
                UpdateAgentCard { name: Some("ghost".into()), ..Default::default() },
            )
            .await
            .expect_err("missing card");
        assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    }
}
