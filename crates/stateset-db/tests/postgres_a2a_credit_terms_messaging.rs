//! Postgres twin of `sqlite_a2a_credit_terms_messaging.rs`: durable,
//! tenant-scoped A2A credit terms and agent messaging, including races.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.
#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2AAgentMessageFilter, A2AAgentMessageStatus, A2ACreditEntryType, A2ACreditMovement,
    A2ACreditPaymentTerms, A2ACreditTermsFilter, CommerceError, CreateA2ACreditTerms,
    SendA2AAgentMessage,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn terms_input(tenant: &str, limit: rust_decimal::Decimal) -> CreateA2ACreditTerms {
    CreateA2ACreditTerms {
        tenant_id: tenant.into(),
        creditor_agent_id: format!("creditor-{}", Uuid::new_v4().as_simple()),
        debtor_agent_id: format!("debtor-{}", Uuid::new_v4().as_simple()),
        credit_limit: limit,
        currency: None,
        payment_terms: Some(A2ACreditPaymentTerms::Net60),
        min_trust_tier: None,
    }
}

fn movement(tenant: &str, terms_id: Uuid, amount: rust_decimal::Decimal) -> A2ACreditMovement {
    A2ACreditMovement {
        tenant_id: tenant.into(),
        terms_id,
        amount,
        reference_id: None,
        notes: None,
    }
}

#[tokio::test]
async fn postgres_a2a_credit_terms_persist_charge_pay_journal_and_tenant_scope() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.a2a_credit_terms();
    let tenant = format!("tenant-{}", Uuid::new_v4().as_simple());
    let input = terms_input(&tenant, dec!(1000));
    let debtor = input.debtor_agent_id.clone();
    let terms = repo.create_terms_async(input).await.expect("create");
    assert_eq!(terms.outstanding_balance, dec!(0));
    // (timestamps round-trip at microsecond precision on Postgres)
    let fetched = repo.get_terms_async(&tenant, terms.id).await.unwrap().expect("exists");
    assert_eq!(
        (fetched.id, fetched.credit_limit, fetched.status),
        (terms.id, terms.credit_limit, terms.status)
    );

    let (after_charge, entry) =
        repo.charge_async(movement(&tenant, terms.id, dec!(250.50))).await.expect("charge");
    assert_eq!(after_charge.outstanding_balance, dec!(250.50));
    assert_eq!(entry.entry_type, A2ACreditEntryType::Charge);
    assert!(entry.due_date.is_some());
    let err =
        repo.charge_async(movement(&tenant, terms.id, dec!(749.51))).await.expect_err("limit");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "{err:?}");
    let err = repo
        .record_payment_async(movement(&tenant, terms.id, dec!(300)))
        .await
        .expect_err("overpay");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let (after_pay, _) =
        repo.record_payment_async(movement(&tenant, terms.id, dec!(50.50))).await.expect("pay");
    assert_eq!(after_pay.outstanding_balance, dec!(200));
    assert_eq!(repo.list_entries_async(&tenant, terms.id).await.unwrap().len(), 2);
    let listed = repo
        .list_terms_async(A2ACreditTermsFilter {
            tenant_id: tenant.clone(),
            debtor_agent_id: Some(debtor),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);

    // Tenant scoping.
    assert!(repo.get_terms_async("other-tenant", terms.id).await.unwrap().is_none());
    let err =
        repo.charge_async(movement("other-tenant", terms.id, dec!(1))).await.expect_err("x-tenant");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_a2a_credit_concurrent_charges_never_exceed_limit() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let tenant = format!("tenant-{}", Uuid::new_v4().as_simple());
    for round in 0..8 {
        let terms = db
            .a2a_credit_terms()
            .create_terms_async(terms_input(&tenant, dec!(300)))
            .await
            .unwrap();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let db = Arc::clone(&db);
                let tenant = tenant.clone();
                tokio::spawn(async move {
                    db.a2a_credit_terms().charge_async(movement(&tenant, terms.id, dec!(100))).await
                })
            })
            .collect();
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.expect("task"));
        }
        let ok = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 3, "round {round}: exactly the affordable charges succeed: {results:?}");
        let stored =
            db.a2a_credit_terms().get_terms_async(&tenant, terms.id).await.unwrap().unwrap();
        assert_eq!(stored.outstanding_balance, dec!(300));
        assert_eq!(
            db.a2a_credit_terms().list_entries_async(&tenant, terms.id).await.unwrap().len(),
            3
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_a2a_messages_persist_sequence_acknowledge_and_race() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let repo = db.a2a_messages();
    let tenant = format!("tenant-{}", Uuid::new_v4().as_simple());
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let first = repo
        .send_message_async(SendA2AAgentMessage {
            tenant_id: tenant.clone(),
            conversation_id: None,
            from_agent_id: a,
            to_agent_id: b,
            message_type: "quote_request".into(),
            payload: serde_json::json!({"sku": "X", "qty": 2}),
            max_attempts: Some(2),
        })
        .await
        .expect("send");
    assert_eq!(first.sequence_number, 1);
    let second = repo
        .send_message_async(SendA2AAgentMessage {
            tenant_id: tenant.clone(),
            conversation_id: Some(first.conversation_id),
            from_agent_id: b,
            to_agent_id: a,
            message_type: "quote_response".into(),
            payload: serde_json::json!({"price": "10.00"}),
            max_attempts: None,
        })
        .await
        .expect("reply");
    assert_eq!(second.sequence_number, 2);
    let stored = repo.get_message_async(&tenant, first.id).await.unwrap().expect("exists");
    assert_eq!(stored.payload["qty"], 2);
    let pending = repo
        .list_messages_async(A2AAgentMessageFilter {
            tenant_id: tenant.clone(),
            to_agent_id: Some(b),
            status: Some(A2AAgentMessageStatus::Pending),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(pending.iter().map(|m| m.id).collect::<Vec<_>>(), vec![first.id]);
    assert!(repo.get_message_async("other-tenant", first.id).await.unwrap().is_none());

    let acked = repo.acknowledge_message_async(&tenant, first.id).await.expect("ack");
    assert_eq!(acked.status, A2AAgentMessageStatus::Acknowledged);
    assert!(matches!(
        repo.acknowledge_message_async(&tenant, first.id).await,
        Err(CommerceError::ValidationError(_))
    ));
    let retry = repo.fail_message_async(&tenant, second.id, "timeout").await.expect("fail 1");
    assert_eq!(retry.status, A2AAgentMessageStatus::Pending);
    assert_eq!(retry.attempts, 1);

    // Concurrent senders in one conversation get unique, gap-free sequence numbers.
    let conversation = Uuid::new_v4();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let db = Arc::clone(&db);
            let tenant = tenant.clone();
            tokio::spawn(async move {
                db.a2a_messages()
                    .send_message_async(SendA2AAgentMessage {
                        tenant_id: tenant,
                        conversation_id: Some(conversation),
                        from_agent_id: a,
                        to_agent_id: b,
                        message_type: "ping".into(),
                        payload: serde_json::json!({}),
                        max_attempts: None,
                    })
                    .await
                    .expect("send")
                    .sequence_number
            })
        })
        .collect();
    let mut seqs = Vec::new();
    for h in handles {
        seqs.push(h.await.expect("task"));
    }
    seqs.sort_unstable();
    assert_eq!(seqs, (1..=8u64).collect::<Vec<_>>());
}
