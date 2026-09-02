#[cfg(feature = "postgres")]
use chrono::{Duration, Utc};
#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{
    A2APurchaseFilter, A2ASkill, CreateA2APurchase, CreateA2AQuote, CreateAgentCard,
    CreateX402PaymentIntent, CurrencyCode, ItemAvailability, PurchaseStatus, QuoteStatus,
    QuotedItem, SignX402PaymentIntent, SkillQuoteFilter, TrustLevel, X402_DEFAULT_SIGNATURE_SCHEME,
    X402Asset, X402CreditDirection, X402CreditTransactionFilter, X402IntentStatus, X402Network,
};
#[cfg(feature = "postgres")]
use stateset_crypto::pqc::generate_hybrid_signing_keypair;
#[cfg(feature = "postgres")]
use stateset_embedded::AsyncCommerce;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_async_x402_payment_intent_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres async x402 payment test"
            );
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");
    let x402 = commerce.x402();

    let payer = format!("0xpayer-{}", Uuid::new_v4().as_simple());
    let payee = format!("0xpayee-{}", Uuid::new_v4().as_simple());
    // Intents for a cart must match the cart's grand total, so use a real cart.
    let cart = commerce
        .carts()
        .create(stateset_core::CreateCart {
            currency: Some(stateset_core::CurrencyCode::USD),
            items: Some(vec![stateset_core::AddCartItem {
                sku: "SKU-X402-SMOKE".to_string(),
                name: "x402 smoke item".to_string(),
                quantity: 1,
                unit_price: rust_decimal::Decimal::new(200, 2),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .await
        .expect("create cart");
    let cart_id: Uuid = cart.id.into();

    let intent = x402
        .create_intent(CreateX402PaymentIntent {
            payer_address: payer.clone(),
            payee_address: payee.clone(),
            amount: 2_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            cart_id: Some(cart_id),
            ..Default::default()
        })
        .await
        .expect("create payment intent");

    assert_eq!(intent.status, X402IntentStatus::Created);
    assert_eq!(intent.payer_address, payer);
    assert_eq!(intent.payee_address, payee);
    assert_eq!(intent.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));

    let nonce = x402.get_next_nonce(&intent.payer_address).await.expect("get next nonce");
    assert_eq!(nonce, 1);

    let intents_for_cart = x402.intents_for_cart(cart_id).await.expect("list intents by cart");
    assert!(!intents_for_cart.is_empty(), "cart should have at least one intent");

    let mut to_sign =
        x402.get_intent(intent.id).await.expect("get intent").expect("intent should exist");
    let keypair = generate_hybrid_signing_keypair().expect("generate hybrid signing keypair");
    to_sign.sign_with_hybrid(&keypair).expect("locally sign intent");

    let signed = x402
        .sign_intent(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature_scheme: None,
                signature: to_sign.payer_signature.clone().expect("signature"),
                public_key: to_sign.payer_public_key.clone().expect("public key"),
                signature_bundle: to_sign.payer_signature_bundle.clone(),
                public_key_bundle: to_sign.payer_public_key_bundle.clone(),
            },
        )
        .await
        .expect("sign intent");

    assert_eq!(signed.status, X402IntentStatus::Signed);
    assert_eq!(signed.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));
    assert!(x402.is_ready_for_settlement(intent.id).await.expect("check settlement readiness"));
    assert!(
        x402.has_valid_signature(intent.id).await.expect("check intent signature"),
        "expected valid signature on signed intent"
    );

    let signed_intents = x402.signed_intents().await.expect("list signed intents");
    assert!(signed_intents.iter().any(|i| i.id == intent.id));

    // The signed intent still claims the cart: a second full-amount intent
    // is a double charge and is refused, naming the open intent.
    let err = x402
        .create_cart_payment(
            cart_id,
            &payer,
            &payee,
            dec!(2.00),
            X402Network::SetChain,
            X402Asset::Usdc,
        )
        .await
        .expect_err("second intent for a claimed cart must be refused");
    match &err {
        stateset_core::CommerceError::Conflict(message) => {
            assert!(message.contains(&intent.id.to_string()), "{message}");
        }
        other => panic!("expected Conflict, got {other:?}"),
    }

    let active = x402.active_intent_for_cart(cart_id).await.expect("find active intent");
    assert_eq!(active.expect("active intent for cart").id, intent.id);
    assert_eq!(x402.intents_for_cart(cart_id).await.expect("list intents by cart").len(), 1);

    let sequenced =
        x402.mark_sequenced(intent.id, 42, Uuid::new_v4()).await.expect("mark intent sequenced");
    assert_eq!(sequenced.status, X402IntentStatus::Sequenced);

    let batched = x402
        .mark_batched(intent.id, "0xroot", vec!["0xproof".into()])
        .await
        .expect("mark intent batched");
    assert_eq!(batched.status, X402IntentStatus::Batched);
    assert_eq!(batched.batch_merkle_root.as_deref(), Some("0xroot"));

    let settled = x402
        .mark_settled(intent.id, &format!("0xsettled-{}", Uuid::new_v4().as_simple()), 123_456)
        .await
        .expect("mark intent settled");
    assert_eq!(settled.status, X402IntentStatus::Settled);

    // A settled intent keeps the cart claimed for good.
    let err = x402
        .create_cart_payment(
            cart_id,
            &payer,
            &payee,
            dec!(2.00),
            X402Network::SetChain,
            X402Asset::Usdc,
        )
        .await
        .expect_err("paid cart must not accept another intent");
    assert!(matches!(err, stateset_core::CommerceError::Conflict(_)), "{err:?}");
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_async_x402_agents_a2a_credit_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres async x402 A2A/agent test"
            );
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");
    let x402 = commerce.x402();

    let network = X402Network::SetChain;
    let asset = X402Asset::Usdc;
    let seller_id = Uuid::new_v4();
    let seller_wallet = format!("0xagent-seller-{}", seller_id.as_simple());

    let card = x402
        .register_agent(CreateAgentCard {
            name: "Test Seller Agent".to_string(),
            description: Some("A2A seller for smoke testing".into()),
            wallet_address: seller_wallet.clone(),
            public_key: "seller-public-key".to_string(),
            supported_networks: Some(vec![network]),
            supported_assets: Some(vec![asset]),
            a2a_skills: Some(vec![A2ASkill::Sell]),
            trust_level: Some(TrustLevel::Sandbox),
            endpoint_url: Some("https://example.com/agent".to_string()),
            endpoint_protocol: Some("https".to_string()),
            merchant_id: Some(format!("merchant-{}", Uuid::new_v4().as_simple())),
            merchant_name: Some("Test Merchant".to_string()),
            business_category: Some("automation".to_string()),
            max_transaction_amount: Some(10_000_000),
            daily_volume_limit: Some(100_000_000),
            requires_kyc: Some(false),
            metadata: Some("{\"source\":\"async-smoke\"}".to_string()),
        })
        .await
        .expect("register agent");

    assert_eq!(card.wallet_address, seller_wallet);
    assert!(card.active);

    let sellers = x402
        .discover_agents(
            Some(network),
            Some(asset),
            Some(A2ASkill::Sell),
            Some(TrustLevel::Sandbox),
        )
        .await
        .expect("discover agents");
    assert!(
        sellers.iter().any(|seller| seller.id == card.id),
        "discover should return registered seller"
    );

    let active_agents = x402.active_agents().await.expect("list active agents");
    assert!(!active_agents.is_empty(), "expected at least one active agent");

    let verified = x402.verify_agent(card.id).await.expect("verify agent");
    assert_eq!(verified.trust_level, TrustLevel::Verified);

    let reactivated = x402.suspend_agent(card.id, "test").await.expect("suspend agent");
    assert!(!reactivated.active, "agent should be suspended");
    let reactivated = x402.reactivate_agent(card.id).await.expect("reactivate agent");
    assert!(reactivated.active, "agent should be reactivated");

    let buyer_id = Uuid::new_v4();
    let now = Utc::now();
    let quote = x402
        .create_quote(CreateA2AQuote {
            buyer_agent_id: buyer_id,
            seller_agent_id: card.id,
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-X42".to_string()),
                name: "Test Widget".to_string(),
                quantity: 2,
                unit_price: dec!(12.50),
                total: dec!(25.00),
                availability: ItemAvailability::InStock,
                lead_time_days: Some(1),
            }],
            subtotal: dec!(25.00),
            tax_amount: Some(dec!(1.00)),
            shipping_amount: Some(dec!(4.00)),
            discount_amount: None,
            total: dec!(30.00),
            currency: Some(CurrencyCode::USD),
            payment_network: Some(network),
            payment_asset: Some(asset),
            shipping_address: None,
            valid_until: now + Duration::hours(12),
            notes: Some("smoke quote".to_string()),
            metadata: Some("{\"channel\":\"integration\"}".to_string()),
        })
        .await
        .expect("create A2A quote");

    assert_eq!(quote.status, QuoteStatus::Pending);

    let quoted = x402
        .update_quote_status(quote.id, QuoteStatus::Quoted)
        .await
        .expect("mark quote as quoted");
    assert_eq!(quoted.status, QuoteStatus::Quoted);

    let no_op_quote = x402
        .update_quote_status(quote.id, QuoteStatus::Quoted)
        .await
        .expect("no-op quote status update");
    assert_eq!(no_op_quote.status, QuoteStatus::Quoted);

    let purchase = x402
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: card.id,
            quote_id: Some(quoted.id),
            payment_intent_id: None,
            items: quote.items.clone(),
            total: quoted.total,
            currency: Some(quoted.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("smoke test purchase".to_string()),
            metadata: Some("{\"channel\":\"integration\"}".to_string()),
        })
        .await
        .expect("create A2A purchase");

    assert_eq!(purchase.status, PurchaseStatus::Initiated);
    assert_eq!(purchase.quote_id, Some(quoted.id));

    let payment_pending = x402
        .update_purchase_status(purchase.id, PurchaseStatus::PaymentPending)
        .await
        .expect("mark purchase as payment pending");
    assert_eq!(payment_pending.status, PurchaseStatus::PaymentPending);

    let shipped = x402
        .update_purchase_status(purchase.id, PurchaseStatus::Shipped)
        .await
        .expect("mark purchase as shipped");
    assert_eq!(shipped.status, PurchaseStatus::Shipped);

    let listed_quotes = x402
        .list_quotes(SkillQuoteFilter {
            buyer_agent_id: Some(buyer_id),
            seller_agent_id: Some(card.id),
            ..Default::default()
        })
        .await
        .expect("list quotes");
    assert!(listed_quotes.iter().any(|q| q.id == quoted.id));

    let quote_by_number = x402
        .get_quote_by_number(&quoted.quote_number)
        .await
        .expect("get quote by number")
        .expect("quote by number should exist");
    assert_eq!(quote_by_number.id, quoted.id);

    let counted_quotes = x402
        .count_quotes(SkillQuoteFilter { seller_agent_id: Some(card.id), ..Default::default() })
        .await
        .expect("count quotes");
    assert!(counted_quotes >= 1, "expected at least one quote");

    let listed_purchases = x402
        .list_purchases(A2APurchaseFilter { buyer_agent_id: Some(buyer_id), ..Default::default() })
        .await
        .expect("list purchases");
    assert!(listed_purchases.iter().any(|p| p.id == purchase.id));

    // Orders are linked while the purchase is live; relinking to a different
    // order is refused and a completed purchase cannot be linked at all.
    let order_id = Uuid::new_v4();
    let updated =
        x402.link_purchase_to_order(purchase.id, order_id).await.expect("link purchase to order");
    assert_eq!(updated.order_id, Some(order_id));
    let err = x402
        .link_purchase_to_order(purchase.id, Uuid::new_v4())
        .await
        .expect_err("relinking to another order is refused");
    assert!(matches!(err, stateset_core::CommerceError::Conflict(_)), "{err:?}");

    let confirmed = x402
        .confirm_delivery(purchase.id, "signature-abc", Some(5), Some("Smooth delivery"))
        .await
        .expect("confirm purchase delivery");
    assert_eq!(confirmed.status, PurchaseStatus::Completed);

    let no_op_purchase = x402
        .update_purchase_status(purchase.id, PurchaseStatus::Completed)
        .await
        .expect("no-op purchase status update");
    assert_eq!(no_op_purchase.status, PurchaseStatus::Completed);

    let err = x402
        .link_purchase_to_order(purchase.id, Uuid::new_v4())
        .await
        .expect_err("completed purchase cannot be linked to another order");
    assert!(
        matches!(
            err,
            stateset_core::CommerceError::ValidationError(_)
                | stateset_core::CommerceError::Conflict(_)
        ),
        "{err:?}"
    );
    let unchanged = x402.get_purchase(purchase.id).await.expect("get").expect("exists");
    assert_eq!(unchanged.order_id, Some(order_id));

    let counted_purchases = x402
        .count_purchases(A2APurchaseFilter { buyer_agent_id: Some(buyer_id), ..Default::default() })
        .await
        .expect("count purchases");
    assert!(counted_purchases >= 1, "expected at least one purchase");

    let payer = format!("0xcredits-{}", Uuid::new_v4().as_simple());
    let initial_balance =
        x402.get_credit_balance(&payer, asset, network).await.expect("get initial credit balance");
    assert_eq!(initial_balance, 0);

    let credit_tx = x402
        .credit_account(
            &payer,
            asset,
            network,
            10_000,
            Some("test credit".to_string()),
            Some("tx-in".to_string()),
            Some("{\"reason\":\"smoke\"}".to_string()),
        )
        .await
        .expect("credit account");
    assert_eq!(credit_tx.balance_after, 10_000);
    assert_eq!(credit_tx.direction, X402CreditDirection::Credit);

    let debit_tx = x402
        .debit_account(
            &payer,
            asset,
            network,
            4_000,
            Some("test debit".to_string()),
            Some("tx-out".to_string()),
            None,
        )
        .await
        .expect("debit account");
    assert_eq!(debit_tx.balance_after, 6_000);
    assert_eq!(debit_tx.direction, X402CreditDirection::Debit);

    let after_balance =
        x402.get_credit_balance(&payer, asset, network).await.expect("get final credit balance");
    assert_eq!(after_balance, 6_000);

    let account = x402
        .get_or_create_credit_account(&payer, asset, network)
        .await
        .expect("get or create account");
    assert_eq!(account.balance, 6_000);

    let tx_history = x402
        .list_credit_transactions(X402CreditTransactionFilter {
            payer_address: Some(payer.clone()),
            asset: Some(asset),
            network: Some(network),
            direction: None,
            limit: Some(10),
            offset: Some(0),
        })
        .await
        .expect("list credit transactions");
    assert!(tx_history.len() >= 2, "expected at least two ledger entries");
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_async_a2a_state_guards() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres async a2a state guard test"
            );
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");
    let x402 = commerce.x402();
    let network = X402Network::SetChain;
    let asset = X402Asset::Usdc;

    let seller = x402
        .register_agent(CreateAgentCard {
            name: "A2A Guard Seller".to_string(),
            wallet_address: format!("0xagent-guard-seller-{}", Uuid::new_v4().as_simple()),
            public_key: "seller-public-key-guard".to_string(),
            supported_networks: Some(vec![network]),
            supported_assets: Some(vec![asset]),
            a2a_skills: Some(vec![A2ASkill::Sell]),
            trust_level: Some(TrustLevel::Sandbox),
            endpoint_url: Some("https://example.com/agent-guard".to_string()),
            endpoint_protocol: Some("https".to_string()),
            ..Default::default()
        })
        .await
        .expect("register guard seller");

    let other_seller = x402
        .register_agent(CreateAgentCard {
            name: "A2A Wrong Seller".to_string(),
            wallet_address: format!("0xagent-guard-seller-2-{}", Uuid::new_v4().as_simple()),
            public_key: "seller-public-key-guard-2".to_string(),
            supported_networks: Some(vec![network]),
            supported_assets: Some(vec![asset]),
            a2a_skills: Some(vec![A2ASkill::Sell]),
            trust_level: Some(TrustLevel::Sandbox),
            endpoint_url: Some("https://example.com/agent-guard-2".to_string()),
            endpoint_protocol: Some("https".to_string()),
            ..Default::default()
        })
        .await
        .expect("register wrong seller");

    let buyer_id = Uuid::new_v4();
    let now = Utc::now();
    let quote = x402
        .create_quote(CreateA2AQuote {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-GUARD".to_string()),
                name: "Guarded Item".to_string(),
                quantity: 1,
                unit_price: dec!(30.00),
                total: dec!(30.00),
                availability: ItemAvailability::InStock,
                lead_time_days: Some(1),
            }],
            subtotal: dec!(30.00),
            tax_amount: Some(dec!(1.00)),
            shipping_amount: Some(dec!(2.00)),
            discount_amount: None,
            total: dec!(33.00),
            currency: Some(CurrencyCode::USD),
            payment_network: Some(network),
            payment_asset: Some(asset),
            shipping_address: None,
            valid_until: now + Duration::hours(6),
            notes: Some("state guard quote".to_string()),
            metadata: Some("{\"purpose\":\"guard\"}".to_string()),
        })
        .await
        .expect("create guard quote");

    assert!(x402.update_quote_status(quote.id, QuoteStatus::Accepted).await.is_err());

    let quoted = x402
        .update_quote_status(quote.id, QuoteStatus::Quoted)
        .await
        .expect("mark quote as quoted");

    assert!(
        x402.create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: other_seller.id,
            quote_id: Some(quoted.id),
            payment_intent_id: None,
            items: quoted.items.clone(),
            total: quoted.total,
            currency: Some(quoted.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("wrong seller".to_string()),
            metadata: None,
        })
        .await
        .is_err()
    );

    assert!(
        x402.create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            quote_id: Some(quoted.id),
            payment_intent_id: None,
            items: quoted.items.clone(),
            total: quoted.total,
            currency: Some(CurrencyCode::EUR),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("wrong currency".to_string()),
            metadata: None,
        })
        .await
        .is_err()
    );

    assert!(
        x402.create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            quote_id: Some(quoted.id),
            payment_intent_id: None,
            items: quoted.items.clone(),
            total: quoted.total + dec!(1.00),
            currency: Some(quoted.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("wrong total".to_string()),
            metadata: None,
        })
        .await
        .is_err()
    );

    let purchase = x402
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            quote_id: Some(quoted.id),
            payment_intent_id: None,
            items: quoted.items.clone(),
            total: quoted.total,
            currency: Some(quoted.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("valid purchase".to_string()),
            metadata: None,
        })
        .await
        .expect("create guarded purchase");

    assert_eq!(purchase.status, PurchaseStatus::Initiated);

    assert!(x402.update_purchase_status(purchase.id, PurchaseStatus::Completed).await.is_err());

    assert!(
        x402.confirm_delivery(purchase.id, "signature", Some(5), Some("not shipped yet"))
            .await
            .is_err()
    );

    assert!(x402.update_quote_status(quote.id, QuoteStatus::Accepted).await.is_err());

    assert!(
        x402.create_quote(CreateA2AQuote {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-EXPIRED".to_string()),
                name: "Expired service".to_string(),
                quantity: 1,
                unit_price: dec!(15.00),
                total: dec!(15.00),
                availability: ItemAvailability::InStock,
                lead_time_days: Some(1),
            }],
            subtotal: dec!(15.00),
            tax_amount: Some(dec!(0.00)),
            shipping_amount: Some(dec!(0.00)),
            discount_amount: None,
            total: dec!(15.00),
            currency: Some(CurrencyCode::USD),
            payment_network: Some(network),
            payment_asset: Some(asset),
            shipping_address: None,
            valid_until: now - Duration::hours(1),
            notes: Some("expired quote".to_string()),
            metadata: None,
        })
        .await
        .is_err()
    );

    let quote = x402
        .create_quote(CreateA2AQuote {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-EXPIRED".to_string()),
                name: "Expired service".to_string(),
                quantity: 1,
                unit_price: dec!(15.00),
                total: dec!(15.00),
                availability: ItemAvailability::InStock,
                lead_time_days: Some(1),
            }],
            subtotal: dec!(15.00),
            tax_amount: Some(dec!(0.00)),
            shipping_amount: Some(dec!(0.00)),
            discount_amount: None,
            total: dec!(15.00),
            currency: Some(CurrencyCode::USD),
            payment_network: Some(network),
            payment_asset: Some(asset),
            shipping_address: None,
            valid_until: now + Duration::hours(1),
            notes: Some("expired quote".to_string()),
            metadata: None,
        })
        .await
        .expect("create valid quote for expired status transition");

    let quoted = x402
        .update_quote_status(quote.id, QuoteStatus::Quoted)
        .await
        .expect("mark quote as quoted");

    let expired = x402
        .update_quote_status(quoted.id, QuoteStatus::Expired)
        .await
        .expect("mark quote as expired");

    assert_eq!(expired.status, QuoteStatus::Expired);

    assert!(
        x402.create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            quote_id: Some(expired.id),
            payment_intent_id: None,
            items: expired.items,
            total: expired.total,
            currency: Some(expired.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("expired quote blocked".to_string()),
            metadata: None,
        })
        .await
        .is_err()
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_async_a2a_state_lifecycle_controls() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres async A2A lifecycle test"
            );
            return;
        }
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");
    let x402 = commerce.x402();
    let now = Utc::now();

    let seller = x402
        .register_agent(CreateAgentCard {
            name: "A2A Lifecycle Seller Async".into(),
            wallet_address: format!("0xagent-lifecycle-async-{}", Uuid::new_v4().as_simple()),
            public_key: "lifecycle-async-pub".into(),
            supported_networks: Some(vec![X402Network::SetChain]),
            supported_assets: Some(vec![X402Asset::Usdc]),
            a2a_skills: Some(vec![A2ASkill::Sell]),
            trust_level: Some(TrustLevel::Verified),
            endpoint_url: Some("https://agent.example.com/lifecycle-async".to_string()),
            endpoint_protocol: Some("https".to_string()),
            ..Default::default()
        })
        .await
        .expect("register lifecycle async seller");

    let buyer_id = Uuid::new_v4();

    let make_quote = |buyer_id: Uuid, seller_id: Uuid| CreateA2AQuote {
        buyer_agent_id: buyer_id,
        seller_agent_id: seller_id,
        items: vec![QuotedItem {
            line_number: 1,
            sku: Some("SKU-LC-1".to_string()),
            name: "Lifecycle service".to_string(),
            quantity: 1,
            unit_price: dec!(12.00),
            total: dec!(12.00),
            availability: ItemAvailability::InStock,
            lead_time_days: Some(1),
        }],
        subtotal: dec!(12.00),
        tax_amount: Some(rust_decimal::Decimal::ZERO),
        shipping_amount: Some(rust_decimal::Decimal::ZERO),
        discount_amount: Some(rust_decimal::Decimal::ZERO),
        total: dec!(12.00),
        currency: Some(CurrencyCode::USD),
        payment_network: Some(X402Network::SetChain),
        payment_asset: Some(X402Asset::Usdc),
        shipping_address: None,
        valid_until: now + Duration::hours(1),
        notes: Some("lifecycle quote async".to_string()),
        metadata: None,
    };

    let cancelled_quote = x402
        .create_quote(make_quote(buyer_id, seller.id))
        .await
        .expect("create async cancelled quote");
    let cancelled_quote = x402
        .update_quote_status(cancelled_quote.id, QuoteStatus::Quoted)
        .await
        .expect("update async quote to quoted");

    let cancelled_purchase = x402
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller.id,
            quote_id: Some(cancelled_quote.id),
            items: cancelled_quote.items.clone(),
            total: cancelled_quote.total,
            currency: Some(cancelled_quote.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("cancel path".into()),
            metadata: None,
            payment_intent_id: None,
        })
        .await
        .expect("create async cancelled purchase");

    let cancelled = x402
        .update_purchase_status(cancelled_purchase.id, PurchaseStatus::Cancelled)
        .await
        .expect("mark async purchase as cancelled");
    assert_eq!(cancelled.status, PurchaseStatus::Cancelled);

    let no_op = x402
        .update_purchase_status(cancelled_purchase.id, PurchaseStatus::Cancelled)
        .await
        .expect("no-op async purchase status update");
    assert_eq!(no_op.status, PurchaseStatus::Cancelled);

    assert!(
        x402.update_purchase_status(cancelled_purchase.id, PurchaseStatus::PaymentPending)
            .await
            .is_err()
    );
    assert!(
        x402.confirm_delivery(cancelled_purchase.id, "sig", Some(4), Some("should fail"))
            .await
            .is_err()
    );

    let disputed_quote = x402
        .create_quote(make_quote(Uuid::new_v4(), seller.id))
        .await
        .expect("create async disputed quote");
    let disputed_quote = x402
        .update_quote_status(disputed_quote.id, QuoteStatus::Quoted)
        .await
        .expect("update async disputed quote to quoted");

    let disputed_purchase = x402
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: disputed_quote.buyer_agent_id,
            seller_agent_id: seller.id,
            quote_id: Some(disputed_quote.id),
            items: disputed_quote.items.clone(),
            total: disputed_quote.total,
            currency: Some(disputed_quote.currency),
            fulfillment_type: Some("digital".to_string()),
            notes: Some("dispute path".into()),
            metadata: None,
            payment_intent_id: None,
        })
        .await
        .expect("create async disputed purchase");

    let disputed = x402
        .update_purchase_status(disputed_purchase.id, PurchaseStatus::Disputed)
        .await
        .expect("mark async purchase as disputed");
    assert_eq!(disputed.status, PurchaseStatus::Disputed);

    assert!(
        x402.update_purchase_status(disputed_purchase.id, PurchaseStatus::Shipped).await.is_err()
    );
    assert!(
        x402.confirm_delivery(disputed_purchase.id, "sig", Some(4), Some("blocked")).await.is_err()
    );
    assert!(
        x402.update_purchase_status(disputed_purchase.id, PurchaseStatus::Disputed).await.is_ok()
    );
}
