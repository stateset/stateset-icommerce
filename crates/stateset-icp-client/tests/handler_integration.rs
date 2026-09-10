//! Live integration test: spawn the JS icp-handler, drive it from the
//! Rust SDK over the wire, assert every verb roundtrips.
//!
//! Proves byte-identical interop between the Rust SDK and the JS
//! reference implementation. If this test passes, any Rust service
//! can talk to any ICP-1.0 handler in any language.
//!
//! Skipped automatically (test marked ignored) if `node` is not on
//! PATH or the handler's `package.json` is missing.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use stateset_icp_client::{
    Client, Identity, LineItem, Money, PrincipalBindingParams, PrincipalIdentity,
};

struct Handler {
    child: Child,
    port: u16,
}

impl Drop for Handler {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn workspace_root() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR = .../crates/stateset-icp-client
    manifest.parent().unwrap().parent().unwrap().to_path_buf()
}

fn maybe_spawn_handler() -> Option<Handler> {
    maybe_spawn_handler_with_env(&[])
}

/// Spawn the reference handler with extra environment.
///
/// `ICP_TRUST_MODE=enforce` plus the two key registries turn the handler into
/// the thing a real deployment runs: one that verifies every delegation
/// against operator-configured keys. Mirrors the JS suite's
/// `icp-handler/test/client-binding.test.mjs` setup.
fn maybe_spawn_handler_with_env(env: &[(&str, String)]) -> Option<Handler> {
    let root = workspace_root();
    let handler_dir = root.join("icp-handler");
    if !handler_dir.join("package.json").exists() {
        eprintln!("icp-handler/package.json not found — skipping");
        return None;
    }
    if Command::new("node").arg("--version").stdout(Stdio::null()).status().ok()?.code() != Some(0)
    {
        eprintln!("node not available — skipping");
        return None;
    }

    // Spawn handler on an ephemeral port. PORT=0 lets the OS pick.
    // We have to read stderr to discover the chosen port (handler logs it).
    let mut cmd = Command::new("node");
    cmd.arg("src/server.mjs")
        .current_dir(&handler_dir)
        .env("PORT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let mut child = cmd.spawn().ok()?;

    // Read stderr until we see "listening on http://127.0.0.1:PORT", then
    // hand the reader off to a drain thread so the handler's stderr pipe
    // doesn't fill up and block subsequent verb requests.
    use std::io::{BufRead, BufReader};
    let stderr = child.stderr.take().expect("stderr piped");
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    let mut port: Option<u16> = None;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(idx) = line.find("127.0.0.1:") {
                    let tail = &line[idx + "127.0.0.1:".len()..];
                    let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(p) = num.parse::<u16>() {
                        port = Some(p);
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }

    let p = port?;
    // Drain remaining stderr in a daemon thread so the pipe buffer never fills.
    std::thread::spawn(move || {
        let mut buf = String::new();
        while reader.read_line(&mut buf).map_or(0, |n| n) > 0 {
            buf.clear();
        }
    });
    // Brief warmup so the listener is fully accepting.
    std::thread::sleep(Duration::from_millis(50));
    Some(Handler { child, port: p })
}

#[test]
fn rust_sdk_roundtrips_against_js_handler() {
    let Some(handler) = maybe_spawn_handler() else {
        eprintln!("skipping: handler unavailable");
        return;
    };

    let id = Identity::generate();
    let url = format!("http://127.0.0.1:{}", handler.port);
    let client = Client::new(&url, id);

    // 1. Discovery: handler returns merchant_aid + capabilities.verbs + settlers.
    //    well_known() caches merchant pubkey internally.
    let well_known = client.well_known().expect("well_known");
    let merchant_aid =
        well_known["merchant_aid"].as_str().expect("merchant_aid string").to_string();
    assert!(merchant_aid.starts_with("aid:v1:z"));
    let verbs = well_known["capabilities"]["verbs"].as_array().expect("verbs array");
    assert!(
        verbs.iter().any(|v| v.as_str() == Some("purchase.create")),
        "well_known: {well_known}"
    );
    let settlers = well_known["settler_allowlist"].as_array().expect("settler_allowlist");
    let settler = settlers[0].as_str().expect("settler string").to_string();
    assert!(
        client.merchant_pubkey_hex().is_some(),
        "well_known should have cached the merchant pubkey"
    );

    // 2. inventory.query — verify merchant signature on the response.
    let inv = client.inventory(&merchant_aid, &settler, &["WIDGET-001"]).expect("inventory");
    assert!(inv.payload.is_object(), "inventory payload: {:?}", inv.payload);
    assert_eq!(inv.signature.alg, "ed25519");
    client.verify_signed_response(&inv).expect("inventory signature must verify");

    // 3. purchase.create → Quote (verified).
    let items = vec![LineItem {
        sku: "WIDGET-001".to_string(),
        quantity: 2,
        unit_price: Money { amount: "29.99".to_string(), currency: "USDC".to_string() },
    }];
    let max_total = Money { amount: "200.00".to_string(), currency: "USDC".to_string() };
    let quote = client.purchase(&merchant_aid, &settler, items, max_total).expect("quote");
    let quote_id = quote.payload["quote_id"].as_str().expect("quote_id").to_string();
    assert!(!quote_id.is_empty());
    assert_eq!(quote.payload["total"]["currency"], "USDC");
    client.verify_signed_response(&quote).expect("quote signature must verify");

    // 4. subscription.create → authorization (verified).
    let sub = client
        .subscribe(
            &merchant_aid,
            &settler,
            "pro-tier",
            "monthly",
            Money { amount: "20.00".to_string(), currency: "USDC".to_string() },
            Some(12),
            "2026-06-01T00:00:00.000Z",
        )
        .expect("subscribe");
    assert!(sub.payload.is_object());
    client.verify_signed_response(&sub).expect("subscribe signature must verify");

    // 5. subscription.cancel → authorization (verified).
    let cancel = client
        .cancel(
            &merchant_aid,
            &settler,
            sub.payload["subscription_id"].as_str().unwrap_or("sub_unknown"),
            "end-of-period",
            Some("integration-test"),
        )
        .expect("cancel");
    client.verify_signed_response(&cancel).expect("cancel signature must verify");

    // 6. quote.request → proposal (verified). Use the JS-SDK-shaped items array.
    let rfq_items = serde_json::json!([
        { "sku": "WIDGET-001", "quantity": 50 }
    ]);
    let proposal = client.request_quote(&merchant_aid, &settler, rfq_items).expect("quote.request");
    assert!(proposal.payload.is_object());
    client.verify_signed_response(&proposal).expect("proposal signature must verify");

    // 7. purchase.return → authorization (verified).
    let ret_items = serde_json::json!([
        { "sku": "WIDGET-001", "quantity": 1, "reason": "damaged" }
    ]);
    let ret = client
        .return_purchase(&merchant_aid, &settler, "icp_set_demo", ret_items, "refund")
        .expect("return");
    client.verify_signed_response(&ret).expect("return signature must verify");

    // 8. payout.request → authorization (verified). A payout must be signed by the
    //    seller it names — the handler binds the intent signer to the acting party
    //    on every verb — so a second identity acts as the seller and submits its own
    //    payout. The stub may still reject for policy reasons depending on platform
    //    state; tolerate that path explicitly while exercising the signed request.
    let seller_identity = Identity::generate();
    let seller_aid = seller_identity.aid().to_string();
    let seller_client = Client::new(&url, seller_identity);
    seller_client.well_known().expect("seller well_known");
    match seller_client.payout(
        &merchant_aid,
        &settler,
        &seller_aid,
        &merchant_aid, // platform = merchant for the demo stub
        Money { amount: "10.00".to_string(), currency: "USDC".to_string() },
    ) {
        Ok(payout) => {
            seller_client.verify_signed_response(&payout).expect("payout signature must verify");
        }
        Err(stateset_icp_client::Error::Icp { code, .. }) => {
            // Policy rejection is a valid handler outcome for an unknown seller; we
            // still proved the signed request roundtrips through verification gates.
            assert!(code.starts_with("policy."), "unexpected error code: {code}");
        }
        Err(other) => panic!("payout failed unexpectedly: {other:?}"),
    }

    // 9. channel.register (webhook) — ICPIP-0005 registration. Use https://
    //    so the production URL validator accepts it.
    let reg = client
        .register_webhook(
            &merchant_aid,
            &settler,
            "webhook",
            Some("https://agent.example.com/icp/events"),
            &["settlement.released", "escrow.refunded"],
        )
        .expect("channel.register (webhook)");
    assert_eq!(reg.payload["channel_type"], "webhook");
    assert_eq!(reg.payload["webhook_url"], "https://agent.example.com/icp/events");
    assert!(reg.payload["channel_id"].as_str().map(|s| s.starts_with("icp_ch_")).unwrap_or(false));
    client.verify_signed_response(&reg).expect("channel.register signature must verify");

    // 10. channel.register (sse) — mints subscription token.
    let sse = client
        .register_webhook(&merchant_aid, &settler, "sse", None, &["dispute.opened"])
        .expect("channel.register (sse)");
    assert_eq!(sse.payload["channel_type"], "sse");
    assert!(sse.payload["subscription_token"].is_string());
    assert_eq!(sse.payload["token_ttl_seconds"], 3600);

    // 11. channel.register with http:// non-loopback URL → typed channel.url_unverified.
    match client.register_webhook(
        &merchant_aid,
        &settler,
        "webhook",
        Some("http://insecure.example.com/events"),
        &["settlement.released"],
    ) {
        Ok(other) => panic!("expected error, got: {other:?}"),
        Err(stateset_icp_client::Error::Icp { code, .. }) => {
            assert_eq!(code, "channel.url_unverified");
        }
        Err(other) => panic!("unexpected error variant: {other:?}"),
    }

    // 12. Recovery API roundtrip. Register a webhook with an unreachable
    //     loopback URL so the live POST fails, run a fulfill cycle to
    //     trigger publish, then fetch the recovered envelope.
    let recov_reg = client
        .register_webhook(
            &merchant_aid,
            &settler,
            "webhook",
            Some("http://127.0.0.1:1/icp/events"), // unreachable on purpose
            &["settlement.released"],
        )
        .expect("recovery channel register");
    let recov_channel_id =
        recov_reg.payload["channel_id"].as_str().expect("channel_id").to_string();

    // Drive a purchase → accept → fulfill cycle.
    let purchase = client
        .purchase(
            &merchant_aid,
            &settler,
            vec![LineItem {
                // Use a real catalog item: quote acceptance now performs an
                // inventory reservation and must reject synthetic SKUs.
                sku: "WIDGET-003".to_string(),
                quantity: 1,
                unit_price: Money { amount: "12.00".to_string(), currency: "USDC".to_string() },
            }],
            Money { amount: "20.00".to_string(), currency: "USDC".to_string() },
        )
        .expect("purchase");
    let quote_id = purchase.payload["quote_id"].as_str().unwrap().to_string();
    let accepted = client.accept_quote(&quote_id).expect("accept");
    let escrow_id = accepted["funding"]["escrow_id"]
        .as_str()
        .or_else(|| accepted["escrow_id"].as_str())
        .or_else(|| accepted["escrow"].as_str())
        .expect("escrow_id")
        .to_string();
    // Trigger fulfill directly via the ureq agent.
    let fulfill_url = format!("{}/icp/v1/escrows/{}/fulfill", url, escrow_id);
    let _ = ureq::post(&fulfill_url)
        .send_json(serde_json::json!({"evidence_id": "icp_ful_RUST_RECOV"}));

    // Settle window so the fire-and-forget publish lands in the recovery log.
    std::thread::sleep(Duration::from_millis(200));

    let events = client.fetch_channel_events(&recov_channel_id, 0).expect("fetch_channel_events");
    assert!(!events.is_empty(), "expected at least one recovered event, got: {events:?}");
    let evt = events
        .iter()
        .find(|e| e["event_type"] == "settlement.released")
        .expect("must include settlement.released");
    assert_eq!(evt["channel_id"], recov_channel_id);
    assert_eq!(evt["payload"]["final_state"], "released");

    // since=<latest sequence> → empty.
    let latest_seq = evt["sequence"].as_u64().expect("sequence");
    let tail = client.fetch_channel_events(&recov_channel_id, latest_seq).expect("tail fetch");
    assert!(tail.is_empty(), "tail must be empty after since=latest");

    // 13. Recovery on unknown channel → typed channel.not_found.
    match client.fetch_channel_events("icp_ch_does_not_exist_xyz", 0) {
        Ok(other) => panic!("expected error, got: {other:?}"),
        Err(stateset_icp_client::Error::Icp { code, .. }) => {
            assert_eq!(code, "channel.not_found");
        }
        Err(other) => panic!("unexpected error variant: {other:?}"),
    }
}

/// The enforcing handler is the one a real deployment runs: it verifies every
/// delegation against operator-configured principal keys. Until this test
/// existed the Rust SDK could not transact with one at all — it shipped a
/// placeholder binding (`kid: "self"`, `sig: "deadbeef"`) that looked like a
/// delegation and proved nothing, so the only way through was to turn
/// delegation checking off.
///
/// This is the contract between the two halves: the binding
/// `sign_principal_binding` produces verifies against the principal key an
/// operator registered, every verb the client can emit is covered by the
/// default delegation, and each way of getting it wrong fails closed with the
/// code the handler's delegation suite pins.
#[test]
fn enforcing_handler_accepts_a_real_binding_and_rejects_the_alternatives() {
    const PRINCIPAL: &str = "did:web:rust-sdk-binding.example";
    const OTHER_PRINCIPAL: &str = "did:web:not-registered.example";

    // Both identities must exist before the handler starts: an operator-keyed
    // handler admits only signer AIDs it was configured with.
    let identity = Identity::generate();
    let seller_identity = Identity::generate();
    let principal_key = PrincipalIdentity::generate();

    let agent_keys = serde_json::json!({
        identity.aid(): hex::encode(identity.ed_pubkey()),
        seller_identity.aid(): hex::encode(seller_identity.ed_pubkey()),
    })
    .to_string();
    let principal_keys =
        serde_json::json!({ PRINCIPAL: principal_key.ed_pubkey_hex() }).to_string();

    let Some(handler) = maybe_spawn_handler_with_env(&[
        ("ICP_TRUST_MODE", "enforce".to_string()),
        ("ICP_PRINCIPAL_KEYS_JSON", principal_keys),
        ("ICP_AGENT_KEYS_JSON", agent_keys),
    ]) else {
        // A new test that skips itself is a test that never ran. CI sets
        // ICP_HANDLER_REQUIRED=1 so a missing handler is a hard failure
        // rather than a green run for the wrong reason.
        assert!(
            std::env::var("ICP_HANDLER_REQUIRED").as_deref() != Ok("1"),
            "ICP_HANDLER_REQUIRED=1 but the JS handler could not be spawned \
             (node missing or icp-handler/package.json not found)"
        );
        eprintln!("skipping: handler unavailable");
        return;
    };
    let url = format!("http://127.0.0.1:{}", handler.port);

    let delegated = |id: Identity| -> Client {
        Client::new(&url, id)
            .with_principal_identity(PRINCIPAL, principal_key.clone())
            .expect("principal identity")
    };

    let client = delegated(identity.clone());
    let well_known = client.well_known().expect("well_known");
    assert_eq!(well_known["trust_mode"], "enforce", "handler must be enforcing: {well_known}");
    let merchant = well_known["merchant_aid"].as_str().expect("merchant_aid").to_string();
    let settler = well_known["settler_allowlist"][0].as_str().expect("settler").to_string();

    let purchase_items = || {
        vec![LineItem {
            sku: "WIDGET-001".to_string(),
            quantity: 1,
            unit_price: Money { amount: "29.99".to_string(), currency: "USDC".to_string() },
        }]
    };
    let max_total = || Money { amount: "200.00".to_string(), currency: "USDC".to_string() };

    // 1. The happy path: a binding this SDK signed clears checkDelegation.
    let quote = client
        .purchase(&merchant, &settler, purchase_items(), max_total())
        .expect("a signed binding must be accepted by an enforcing handler");
    assert!(quote.payload["quote_id"].as_str().is_some_and(|q| !q.is_empty()));
    client.verify_signed_response(&quote).expect("quote signature must verify");

    // 2. The DEFAULT delegation must cover this client's whole verb surface —
    //    a client whose own methods answer delegation.scope_mismatch is not
    //    usable, and the enforcing handler is the only judge of that.
    client.inventory(&merchant, &settler, &["WIDGET-001"]).expect("inventory.query");
    let sub = client
        .subscribe(
            &merchant,
            &settler,
            "pro-tier",
            "monthly",
            Money { amount: "20.00".to_string(), currency: "USDC".to_string() },
            Some(12),
            "2026-06-01T00:00:00.000Z",
        )
        .expect("subscription.create");
    client
        .cancel(
            &merchant,
            &settler,
            sub.payload["subscription_id"].as_str().unwrap_or("icp_sub_ENFORCE0000000000000001"),
            "end-of-period",
            Some("enforce-mode-test"),
        )
        .expect("subscription.cancel");
    client
        .return_purchase(
            &merchant,
            &settler,
            "icp_set_ENFORCE0000000000000001",
            serde_json::json!([{ "sku": "WIDGET-001", "quantity": 1, "reason": "damaged" }]),
            "refund",
        )
        .expect("purchase.return");
    client
        .request_quote(
            &merchant,
            &settler,
            serde_json::json!([{ "sku": "WIDGET-001", "quantity": 50 }]),
        )
        .expect("quote.request");
    client
        .register_webhook(
            &merchant,
            &settler,
            "webhook",
            Some("https://agent.example.com/icp/events"),
            &["settlement.released"],
        )
        .expect("channel.register");

    // payout.request binds the delegation to the *seller*, not the buyer, so
    // the seller signs its own. A policy rejection is still a valid handler
    // outcome for an unknown seller — what must not happen is a delegation
    // failure.
    let seller_aid = seller_identity.aid().to_string();
    let seller_client = delegated(seller_identity);
    seller_client.well_known().expect("seller well_known");
    match seller_client.payout(
        &merchant,
        &settler,
        &seller_aid,
        &merchant,
        Money { amount: "10.00".to_string(), currency: "USDC".to_string() },
    ) {
        Ok(_) => {}
        Err(stateset_icp_client::Error::Icp { code, .. }) => {
            assert!(code.starts_with("policy."), "payout.request delegation failed: {code}");
        }
        Err(other) => panic!("payout failed unexpectedly: {other:?}"),
    }

    // 3. No delegation at all → delegation.required. This is the honest
    //    outcome for an undelegated Agent, and the one the placeholder used
    //    to hide.
    let undelegated = Client::new(&url, identity.clone());
    assert_eq!(
        icp_code(undelegated.purchase(&merchant, &settler, purchase_items(), max_total())),
        "delegation.required"
    );

    // 4. Authority widened after signing → delegation.signature_invalid. The
    //    classic escalation attempt: every field but `signature` is covered.
    let mut tampered =
        PrincipalBindingParams::new(PRINCIPAL, identity.aid()).sign(&principal_key).expect("sign");
    tampered.authority.max_per_intent =
        Money { amount: "1000000".to_string(), currency: "USDC".to_string() };
    let tampering = Client::new(&url, identity.clone())
        .with_principal_binding(tampered)
        .expect("binding delegates this agent");
    assert_eq!(
        icp_code(tampering.purchase(&merchant, &settler, purchase_items(), max_total())),
        "delegation.signature_invalid"
    );

    // 5. Signed by a key that is not the principal's → same failure.
    let impostor = Client::new(&url, identity.clone())
        .with_principal_identity(PRINCIPAL, PrincipalIdentity::generate())
        .expect("principal identity");
    assert_eq!(
        icp_code(impostor.purchase(&merchant, &settler, purchase_items(), max_total())),
        "delegation.signature_invalid"
    );

    // 6. A binding that does not cover the verb → scope_mismatch, not a
    //    silent pass.
    let narrow = Client::new(&url, identity.clone())
        .with_principal_identity(PRINCIPAL, principal_key.clone())
        .expect("principal identity")
        .with_verbs(["inventory.query"]);
    assert_eq!(
        icp_code(narrow.purchase(&merchant, &settler, purchase_items(), max_total())),
        "delegation.scope_mismatch"
    );

    // 7. A principal the operator never configured → principal_unknown.
    let stranger = Client::new(&url, identity.clone())
        .with_principal_identity(OTHER_PRINCIPAL, principal_key.clone())
        .expect("principal identity");
    assert_eq!(
        icp_code(stranger.purchase(&merchant, &settler, purchase_items(), max_total())),
        "delegation.principal_unknown"
    );

    // 8. An expired binding is refused even though its signature is good.
    let expired = PrincipalBindingParams::new(PRINCIPAL, identity.aid())
        .expires_at("2020-01-01T00:00:00Z")
        .sign(&principal_key)
        .expect("sign");
    let stale = Client::new(&url, identity)
        .with_principal_binding(expired)
        .expect("binding delegates this agent");
    assert_eq!(
        icp_code(stale.purchase(&merchant, &settler, purchase_items(), max_total())),
        "delegation.expired"
    );
}

/// The typed ICP error code from a call that must not have succeeded.
fn icp_code<T: std::fmt::Debug>(result: Result<T, stateset_icp_client::Error>) -> String {
    match result {
        Ok(value) => panic!("expected an ICP error, got: {value:?}"),
        Err(stateset_icp_client::Error::Icp { code, .. }) => code,
        Err(other) => panic!("expected a typed ICP error, got: {other:?}"),
    }
}

/// A client can hold a principal key or a binding signed elsewhere — never
/// both, since accepting both would silently pick one. And a binding that
/// delegates somebody else is caught here rather than by the handler.
#[test]
fn miswired_delegation_configuration_is_refused_locally() {
    let identity = Identity::generate();
    let principal_key = PrincipalIdentity::generate();
    let binding = PrincipalBindingParams::new("did:web:x.example", identity.aid())
        .sign(&principal_key)
        .expect("sign");

    let err = Client::new("http://127.0.0.1:1", identity)
        .with_principal_identity("did:web:x.example", principal_key)
        .expect("principal identity")
        .with_principal_binding(binding.clone())
        .expect_err("both must be refused");
    assert!(format!("{err}").contains("not both"), "got: {err}");

    let other = Identity::generate();
    let err = Client::new("http://127.0.0.1:1", other)
        .with_principal_binding(binding)
        .expect_err("a binding for a different agent must be refused");
    assert!(format!("{err}").contains("delegates"), "got: {err}");
}
