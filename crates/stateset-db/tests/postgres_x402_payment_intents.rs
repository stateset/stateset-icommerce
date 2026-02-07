#[cfg(feature = "postgres")]
use stateset_core::{
    CreateX402PaymentIntent, SignX402PaymentIntent, X402Asset, X402IntentStatus, X402Network,
    X402PaymentIntentFilter,
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
async fn postgres_x402_payment_intent_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres x402 intent test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url)
        .await
        .expect("connect to postgres and run migrations");

    let repo = db.x402_payment_intents();
    let payer = format!("0xtest{}", Uuid::new_v4().to_string().replace('-', ""));
    let payee = format!("0xpayee{}", Uuid::new_v4().to_string().replace('-', ""));
    let cart_id = Uuid::new_v4();
    let idempotency = format!("idem-{}", Uuid::new_v4());

    let intent = repo
        .create_async(CreateX402PaymentIntent {
            payer_address: payer.clone(),
            payee_address: payee.clone(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            nonce: None,
            validity_seconds: Some(3600),
            resource_uri: Some("/x402/test".to_string()),
            resource_method: Some("POST".to_string()),
            description: Some("test intent".to_string()),
            cart_id: Some(cart_id),
            order_id: None,
            invoice_id: None,
            merchant_id: Some("merchant-test".to_string()),
            idempotency_key: Some(idempotency.clone()),
            metadata: Some("{\"source\":\"test\"}".to_string()),
        })
        .await
        .expect("create intent");

    assert_eq!(intent.status, X402IntentStatus::Created);
    assert_eq!(intent.cart_id, Some(cart_id));

    let by_key = repo
        .get_by_idempotency_key_async(&idempotency)
        .await
        .expect("get by idempotency")
        .expect("intent for idempotency");
    assert_eq!(by_key.id, intent.id);

    let signed = repo
        .sign_async(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature: "deadbeef".to_string(),
                public_key: "cafebabe".to_string(),
            },
        )
        .await
        .expect("sign intent");
    assert_eq!(signed.status, X402IntentStatus::Signed);

    let batch_id = Uuid::new_v4();
    let sequenced = repo
        .mark_sequenced_async(intent.id, 42, batch_id)
        .await
        .expect("sequence intent");
    assert_eq!(sequenced.status, X402IntentStatus::Sequenced);

    let settled = repo
        .mark_settled_async(intent.id, "0xtxhash", 123)
        .await
        .expect("settle intent");
    assert_eq!(settled.status, X402IntentStatus::Settled);

    let for_cart = repo.for_cart_async(cart_id).await.expect("for_cart");
    assert!(for_cart.iter().any(|i| i.id == intent.id));

    let list = repo
        .list_async(X402PaymentIntentFilter {
            payer_address: Some(payer),
            payee_address: None,
            status: None,
            network: Some(X402Network::SetChain),
            asset: Some(X402Asset::Usdc),
            order_id: None,
            batch_id: None,
            from_date: None,
            to_date: None,
            limit: Some(10),
            offset: Some(0),
        })
        .await
        .expect("list intents");

    assert!(!list.is_empty(), "expected at least one intent in list");
}
