//! Durable, tenant-scoped A2A credit terms and agent messaging on SQLite.
#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2AAgentMessageFilter, A2AAgentMessageStatus, A2ACreditEntryType, A2ACreditMovement,
    A2ACreditPaymentTerms, A2ACreditTermsFilter, A2ACreditTermsRepository, A2AMessagingRepository,
    CommerceError, CreateA2ACreditTerms, SendA2AAgentMessage,
};
use stateset_db::SqliteDatabase;
use std::sync::{Arc, Barrier};
use uuid::Uuid;

fn terms_input(tenant: &str, limit: rust_decimal::Decimal) -> CreateA2ACreditTerms {
    CreateA2ACreditTerms {
        tenant_id: tenant.into(),
        creditor_agent_id: "agent-creditor".into(),
        debtor_agent_id: "agent-debtor".into(),
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

#[test]
fn sqlite_a2a_credit_terms_persist_charge_pay_and_journal() {
    let db = SqliteDatabase::in_memory().expect("db");
    let repo = db.a2a_credit_terms();
    let terms = repo.create_terms(terms_input("t1", dec!(1000))).expect("create");
    assert_eq!(terms.outstanding_balance, dec!(0));
    assert_eq!(terms.currency, "USD");
    assert_eq!(terms.payment_terms, A2ACreditPaymentTerms::Net60);

    // Survives a fresh repository handle (it is in the database, not memory).
    let fetched = db.a2a_credit_terms().get_terms("t1", terms.id).unwrap().expect("exists");
    assert_eq!(fetched, terms);

    let (after_charge, entry) =
        repo.charge(movement("t1", terms.id, dec!(250.50))).expect("charge");
    assert_eq!(after_charge.outstanding_balance, dec!(250.50));
    assert_eq!(after_charge.available_credit(), dec!(749.50));
    assert_eq!(entry.entry_type, A2ACreditEntryType::Charge);
    assert_eq!(entry.balance_after, dec!(250.50));
    let due = entry.due_date.expect("charges carry a due date");
    assert!(due > chrono::Utc::now() + chrono::Duration::days(59));

    let err = repo.charge(movement("t1", terms.id, dec!(749.51))).expect_err("over limit");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "{err:?}");
    let err = repo.charge(movement("t1", terms.id, dec!(0))).expect_err("zero");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let err = repo.record_payment(movement("t1", terms.id, dec!(300))).expect_err("overpay");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let (after_pay, pay) = repo.record_payment(movement("t1", terms.id, dec!(50.50))).expect("pay");
    assert_eq!(after_pay.outstanding_balance, dec!(200));
    assert_eq!(pay.entry_type, A2ACreditEntryType::Payment);
    assert!(pay.due_date.is_none());

    let entries = repo.list_entries("t1", terms.id).expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, entry.id);
    assert_eq!(entries[1].id, pay.id);

    let listed = repo
        .list_terms(A2ACreditTermsFilter {
            tenant_id: "t1".into(),
            debtor_agent_id: Some("agent-debtor".into()),
            ..Default::default()
        })
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].outstanding_balance, dec!(200));
}

#[test]
fn sqlite_a2a_credit_terms_are_tenant_scoped() {
    let db = SqliteDatabase::in_memory().expect("db");
    let repo = db.a2a_credit_terms();
    let terms = repo.create_terms(terms_input("tenant-a", dec!(100))).expect("create");
    assert!(repo.get_terms("tenant-b", terms.id).unwrap().is_none());
    let err = repo.charge(movement("tenant-b", terms.id, dec!(1))).expect_err("cross-tenant");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
    assert!(
        repo.list_terms(A2ACreditTermsFilter {
            tenant_id: "tenant-b".into(),
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(repo.get_terms("tenant-a", terms.id).unwrap().unwrap().outstanding_balance, dec!(0));
}

#[test]
fn sqlite_a2a_credit_terms_validation() {
    let db = SqliteDatabase::in_memory().expect("db");
    let repo = db.a2a_credit_terms();
    let mut bad = terms_input("t", dec!(0));
    assert!(matches!(repo.create_terms(bad.clone()), Err(CommerceError::ValidationError(_))));
    bad.credit_limit = dec!(10);
    bad.debtor_agent_id = bad.creditor_agent_id.clone();
    assert!(matches!(repo.create_terms(bad.clone()), Err(CommerceError::ValidationError(_))));
    bad.debtor_agent_id = "other".into();
    bad.tenant_id = " ".into();
    assert!(matches!(repo.create_terms(bad), Err(CommerceError::ValidationError(_))));
}

#[test]
fn sqlite_a2a_credit_concurrent_charges_never_exceed_limit() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("db"));
    for round in 0..10 {
        // Limit covers exactly 3 charges of 100; 8 contenders race.
        let terms = db.a2a_credit_terms().create_terms(terms_input("t", dec!(300))).unwrap();
        let contenders = 8;
        let barrier = Arc::new(Barrier::new(contenders));
        let handles: Vec<_> = (0..contenders)
            .map(|_| {
                let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
                std::thread::spawn(move || {
                    barrier.wait();
                    db.a2a_credit_terms().charge(movement("t", terms.id, dec!(100)))
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let ok = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 3, "round {round}: exactly the affordable charges succeed: {results:?}");
        let stored = db.a2a_credit_terms().get_terms("t", terms.id).unwrap().unwrap();
        assert_eq!(stored.outstanding_balance, dec!(300));
        assert_eq!(db.a2a_credit_terms().list_entries("t", terms.id).unwrap().len(), 3);
    }
}

#[test]
fn sqlite_a2a_messages_persist_sequence_and_acknowledge() {
    let db = SqliteDatabase::in_memory().expect("db");
    let repo = db.a2a_messages();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let first = repo
        .send_message(SendA2AAgentMessage {
            tenant_id: "t1".into(),
            conversation_id: None,
            from_agent_id: a,
            to_agent_id: b,
            message_type: "quote_request".into(),
            payload: serde_json::json!({"sku": "X", "qty": 2}),
            max_attempts: Some(2),
        })
        .expect("send");
    assert_eq!(first.sequence_number, 1);
    assert_eq!(first.status, A2AAgentMessageStatus::Pending);
    let second = repo
        .send_message(SendA2AAgentMessage {
            tenant_id: "t1".into(),
            conversation_id: Some(first.conversation_id),
            from_agent_id: b,
            to_agent_id: a,
            message_type: "quote_response".into(),
            payload: serde_json::json!({"price": "10.00"}),
            max_attempts: None,
        })
        .expect("reply");
    assert_eq!(second.sequence_number, 2);
    assert_eq!(second.conversation_id, first.conversation_id);

    // Durable: a fresh handle sees the same rows, payload intact.
    let stored = db.a2a_messages().get_message("t1", first.id).unwrap().expect("exists");
    assert_eq!(stored.payload["qty"], 2);

    // Pending listing for b contains only the first; tenant-scoped.
    let pending = repo
        .list_messages(A2AAgentMessageFilter {
            tenant_id: "t1".into(),
            to_agent_id: Some(b),
            status: Some(A2AAgentMessageStatus::Pending),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(pending.iter().map(|m| m.id).collect::<Vec<_>>(), vec![first.id]);
    assert!(repo.get_message("t2", first.id).unwrap().is_none());
    assert!(matches!(repo.acknowledge_message("t2", first.id), Err(CommerceError::NotFound)));

    let acked = repo.acknowledge_message("t1", first.id).expect("ack");
    assert_eq!(acked.status, A2AAgentMessageStatus::Acknowledged);
    assert!(acked.acknowledged_at.is_some());
    let err = repo.acknowledge_message("t1", first.id).expect_err("double ack");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    // Failure bookkeeping: retry once, then Failed at max_attempts.
    let conv = repo
        .send_message(SendA2AAgentMessage {
            tenant_id: "t1".into(),
            conversation_id: Some(first.conversation_id),
            from_agent_id: a,
            to_agent_id: b,
            message_type: "custom".into(),
            payload: serde_json::json!({}),
            max_attempts: Some(2),
        })
        .unwrap();
    let retry = repo.fail_message("t1", conv.id, "timeout").expect("first failure");
    assert_eq!(retry.status, A2AAgentMessageStatus::Pending);
    assert_eq!(retry.attempts, 1);
    assert!(retry.next_retry_at.is_some());
    let dead = repo.fail_message("t1", conv.id, "timeout again").expect("second failure");
    assert_eq!(dead.status, A2AAgentMessageStatus::Failed);
    assert_eq!(dead.error.as_deref(), Some("timeout again"));
    assert!(matches!(
        repo.fail_message("t1", conv.id, "x"),
        Err(CommerceError::ValidationError(_))
    ));

    // Sender validation.
    let err = repo
        .send_message(SendA2AAgentMessage {
            tenant_id: "t1".into(),
            conversation_id: None,
            from_agent_id: a,
            to_agent_id: a,
            message_type: "x".into(),
            payload: serde_json::json!({}),
            max_attempts: None,
        })
        .expect_err("self-message");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[test]
fn sqlite_a2a_messages_concurrent_senders_get_unique_sequence_numbers() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("db"));
    let conversation = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let contenders = 8;
    let barrier = Arc::new(Barrier::new(contenders));
    let handles: Vec<_> = (0..contenders)
        .map(|_| {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.a2a_messages()
                    .send_message(SendA2AAgentMessage {
                        tenant_id: "t".into(),
                        conversation_id: Some(conversation),
                        from_agent_id: a,
                        to_agent_id: b,
                        message_type: "ping".into(),
                        payload: serde_json::json!({}),
                        max_attempts: None,
                    })
                    .expect("send")
                    .sequence_number
            })
        })
        .collect();
    let mut seqs: Vec<u64> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
    seqs.sort_unstable();
    assert_eq!(seqs, (1..=contenders as u64).collect::<Vec<_>>());
}
