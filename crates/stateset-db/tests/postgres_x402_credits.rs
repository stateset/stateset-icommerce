#[cfg(feature = "postgres")]
use stateset_core::{
    CommerceError, X402Asset, X402CreditAdjustment, X402CreditDirection,
    X402CreditTransactionFilter, X402Network,
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
async fn postgres_x402_credit_ledger_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres x402 credit test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url)
        .await
        .expect("connect to postgres and run migrations");

    let repo = db.x402_credits();
    let payer = format!("0xtest{}", Uuid::new_v4().to_string().replace('-', ""));
    let asset = X402Asset::Usdc;
    let network = X402Network::SetChain;

    let balance = repo
        .get_balance_async(&payer, asset, network)
        .await
        .expect("initial balance");
    assert_eq!(balance, 0);

    let credit_tx = repo
        .adjust_balance_async(X402CreditAdjustment {
            payer_address: payer.clone(),
            asset,
            network,
            direction: X402CreditDirection::Credit,
            amount: 1_000,
            reason: Some("test credit".into()),
            reference_id: None,
            metadata: None,
        })
        .await
        .expect("credit balance");
    assert_eq!(credit_tx.balance_after, 1_000);

    let debit_tx = repo
        .adjust_balance_async(X402CreditAdjustment {
            payer_address: payer.clone(),
            asset,
            network,
            direction: X402CreditDirection::Debit,
            amount: 400,
            reason: Some("test debit".into()),
            reference_id: None,
            metadata: None,
        })
        .await
        .expect("debit balance");
    assert_eq!(debit_tx.balance_after, 600);

    let balance = repo
        .get_balance_async(&payer, asset, network)
        .await
        .expect("balance after debit");
    assert_eq!(balance, 600);

    let err = repo
        .adjust_balance_async(X402CreditAdjustment {
            payer_address: payer.clone(),
            asset,
            network,
            direction: X402CreditDirection::Debit,
            amount: 10_000,
            reason: Some("overspend".into()),
            reference_id: None,
            metadata: None,
        })
        .await
        .expect_err("should reject insufficient balance");

    match err {
        CommerceError::NotPermitted(_) => {}
        other => panic!("expected NotPermitted error, got {other:?}"),
    }

    let history = repo
        .list_transactions_async(X402CreditTransactionFilter {
            payer_address: Some(payer.clone()),
            asset: Some(asset),
            network: Some(network),
            direction: None,
            limit: None,
            offset: None,
        })
        .await
        .expect("list transactions");

    assert!(history.len() >= 2, "expected at least two ledger entries");
}
