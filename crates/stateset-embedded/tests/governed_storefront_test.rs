//! The governed storefront catalog through the embedded JSON dispatcher —
//! the exact wire path the Node binding (`executeKernelCommand`) and the MCP
//! strict endpoint use. Every payload below is shaped the way
//! `cli/src/kernel-tool-execution.js` builds it, so a drift between the CLI
//! and the Rust contract fails here rather than in a live agent session.

use serde_json::{Value, json};
use stateset_core::{
    CouponCode, CreateCouponCode, CreatePromotion, KernelCommandPolicy, KernelPolicy,
    PromotionTrigger, PromotionType,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

const STOREFRONT: [&str; 17] = [
    "customers.create",
    "carts.create",
    "carts.item.add",
    "carts.shipping_address.set",
    "carts.payment_method.set",
    "carts.coupon.apply",
    "carts.tax.calculate",
    "checkout.commit",
    "payments.create",
    "payments.complete",
    "orders.transition",
    "shipments.create",
    "orders.ship",
    "returns.create",
    "returns.transition",
    "returns.tracking.add",
    "payments.create_refund",
];

fn policy() -> KernelPolicy {
    STOREFRONT.iter().fold(KernelPolicy::new("storefront-v1"), |policy, command| {
        policy.allow(*command, KernelCommandPolicy::requiring([*command]))
    })
}

struct Agent {
    commerce: Commerce,
    seq: u32,
}

impl Agent {
    fn envelope(&mut self, command_type: &str, mode: &str, payload: Value) -> Value {
        self.seq += 1;
        json!({
            "contract_version": "1.0",
            "command_id": Uuid::new_v4().to_string(),
            "idempotency_key": format!("storefront-{command_type}-{}", self.seq),
            "command_type": command_type,
            "principal": {
                "id": "agent:storefront",
                "kind": "agent",
                "tenant_id": "tenant:storefront",
                "delegated_by": "user:operator",
                "capabilities": STOREFRONT,
            },
            "store_id": "store:storefront",
            "correlation_id": null,
            "causation_id": null,
            "expected_version": null,
            "policy_version": "storefront-v1",
            "approval": null,
            "authority": null,
            "mandate": null,
            "commitment": null,
            "deadline": null,
            "trace_id": null,
            "mode": mode,
            "payload": payload,
            "issued_at": chrono::Utc::now().to_rfc3339(),
        })
    }

    fn run(&mut self, command_type: &str, mode: &str, payload: Value) -> Value {
        let envelope = self.envelope(command_type, mode, payload);
        self.commerce.execute_kernel_command(envelope, policy()).expect(command_type)
    }

    /// Apply and require a sealed success; returns the receipt's result.
    fn apply(&mut self, command_type: &str, payload: Value) -> Value {
        let receipt = self.run(command_type, "apply", payload);
        assert_eq!(receipt["status"], "succeeded", "{command_type}: {receipt}");
        assert!(receipt["audit_hash"].is_string(), "{command_type} receipt is sealed");
        assert_eq!(receipt["event_ids"].as_array().map(Vec::len), Some(1), "{command_type}");
        receipt["result"].clone()
    }

    fn rejected(&mut self, command_type: &str, payload: Value) -> String {
        let receipt = self.run(command_type, "apply", payload);
        assert_eq!(receipt["status"], "rejected", "{command_type}: {receipt}");
        receipt["error_code"].as_str().unwrap_or_default().to_owned()
    }
}

fn address() -> Value {
    json!({
        "first_name": "Ada", "last_name": "Lovelace", "company": null,
        "line1": "1 Main St", "line2": null, "city": "Los Angeles", "state": "CA",
        "postal_code": "90001", "country": "US", "phone": null, "email": null,
    })
}

#[test]
#[allow(clippy::too_many_lines)]
fn governed_storefront_runs_a_complete_checkout_through_the_json_dispatcher() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    // Operator provisioning, outside the agent surface.
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Welcome 10%".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            percentage_off: Some(rust_decimal_macros::dec!(0.10)),
            ..Default::default()
        })
        .expect("promotion");
    commerce.promotions().activate(promotion.id).expect("activate");
    let _: CouponCode = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promotion.id,
            code: "WELCOME10".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("coupon");
    let mut agent = Agent { commerce, seq: 0 };

    let customer = agent.apply(
        "customers.create",
        json!({"email": "ada@example.com", "first_name": "Ada", "last_name": "Lovelace",
               "phone": null, "accepts_marketing": false, "tags": null, "metadata": null}),
    );
    let customer_id = customer["id"].as_str().expect("customer id").to_owned();

    let cart = agent.apply(
        "carts.create",
        json!({"customer_id": customer_id, "customer_email": null, "customer_name": null,
               "currency": "USD", "items": null, "shipping_address": null,
               "billing_address": null, "notes": null, "metadata": null,
               "expires_in_minutes": null}),
    );
    let cart_id = cart["id"].as_str().expect("cart id").to_owned();

    let item = json!({"product_id": null, "variant_id": null, "sku": "W-1", "name": "Widget",
                      "description": null, "image_url": null, "quantity": 2,
                      "unit_price": "50.00", "original_price": null, "weight": null,
                      "requires_shipping": null, "metadata": null});
    let preview = agent.run("carts.item.add", "preview", json!({"cart_id": cart_id, "item": item}));
    assert_eq!(preview["status"], "previewed", "{preview}");
    let with_item = agent.apply("carts.item.add", json!({"cart_id": cart_id, "item": item}));
    assert_eq!(with_item["subtotal"], "100.00", "{with_item}");

    agent.apply("carts.shipping_address.set", json!({"cart_id": cart_id, "address": address()}));
    let taxed = agent.apply("carts.tax.calculate", json!({"cart_id": cart_id}));
    assert_eq!(taxed["cart"]["tax_amount"], taxed["calculation"]["total_tax"], "{taxed}");

    assert_eq!(
        agent.rejected("carts.coupon.apply", json!({"cart_id": cart_id, "coupon_code": "NOPE"})),
        "commerce.validation_failed"
    );
    let discounted =
        agent.apply("carts.coupon.apply", json!({"cart_id": cart_id, "coupon_code": "WELCOME10"}));
    assert_eq!(discounted["discount_amount"], "10.00", "{discounted}");

    let with_payment = agent.apply(
        "carts.payment_method.set",
        json!({"cart_id": cart_id, "payment": {"payment_method": "credit_card",
               "payment_token": "tok_embedded_secret", "billing_address": null}}),
    );
    assert_eq!(with_payment["payment_token"], "[redacted]");

    let checkout = agent.apply("checkout.commit", json!({"cart_id": cart_id}));
    let order_id = checkout["order_id"].as_str().expect("order id").to_owned();
    let total = checkout["total_charged"].as_str().expect("total").to_owned();
    assert_eq!(total, discounted["grand_total"].as_str().expect("grand total"));

    let payment = agent.apply(
        "payments.create",
        json!({"order_id": order_id, "amount": total, "currency": "USD",
               "payment_method": "credit_card", "idempotency_key": null}),
    );
    let payment_id = payment["id"].as_str().expect("payment id").to_owned();
    let captured = agent.apply("payments.complete", json!({"payment_id": payment_id}));
    assert_eq!(captured["status"], "completed");
    assert_eq!(
        agent.rejected("payments.complete", json!({"payment_id": Uuid::new_v4().to_string()})),
        "commerce.payment_not_found"
    );

    agent.apply(
        "orders.transition",
        json!({"order_id": order_id, "status": "processing", "payment_status": null}),
    );
    let shipment = json!({
        "order_id": order_id, "carrier": "ups", "shipping_method": "ground",
        "tracking_number": null, "recipient_name": "Ada Lovelace", "recipient_email": null,
        "recipient_phone": null, "shipping_address": "1 Main St, Los Angeles, CA 90001, US",
        "weight_kg": null, "dimensions": null, "shipping_cost": null, "insurance_amount": null,
        "signature_required": null, "estimated_delivery": null, "notes": null, "items": null,
    });
    let created = agent.apply("shipments.create", shipment);
    assert_eq!(created["carrier"], "ups");
    agent.apply(
        "orders.ship",
        json!({"order_id": order_id, "tracking_number": "1Z999", "lines": null}),
    );

    let order = agent
        .commerce
        .orders()
        .get(order_id.parse::<Uuid>().expect("uuid").into())
        .expect("order")
        .expect("order");
    let line = order.items[0].id.to_string();
    let return_payload = |quantity: i32| {
        json!({"order_id": order_id, "reason": "defective", "reason_details": null,
               "idempotency_key": null,
               "items": [{"order_item_id": line, "quantity": quantity, "condition": null}],
               "notes": null})
    };
    assert_eq!(
        agent.rejected("returns.create", return_payload(9)),
        "commerce.return.exceeds_shipped"
    );
    let returned = agent.apply("returns.create", return_payload(1));
    let return_id = returned["id"].as_str().expect("return id").to_owned();
    agent.apply("returns.transition", json!({"return_id": return_id, "status": "approved"}));
    let tracked = agent.apply(
        "returns.tracking.add",
        json!({"return_id": return_id, "tracking_number": "RET-1Z"}),
    );
    assert_eq!(tracked["status"], "in_transit");
    agent.apply("returns.transition", json!({"return_id": return_id, "status": "received"}));
    agent.apply("returns.transition", json!({"return_id": return_id, "status": "completed"}));

    let chain = agent.commerce.kernel_audit().expect("audit").verify_chain().expect("verify");
    assert!(chain.valid, "{chain:?}");
}

#[test]
fn the_storefront_catalog_refuses_an_ungranted_command() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let mut agent = Agent { commerce, seq: 0 };
    let mut envelope = agent.envelope(
        "customers.create",
        "apply",
        json!({"email": "no@example.com", "first_name": "No", "last_name": "Grant",
               "phone": null, "accepts_marketing": null, "tags": null, "metadata": null}),
    );
    envelope["principal"]["capabilities"] = json!(["carts.create"]);
    let receipt = agent.commerce.execute_kernel_command(envelope, policy()).expect("dispatch");
    assert_eq!(receipt["status"], "rejected");
    assert_eq!(receipt["error_code"], "kernel.policy_denied");
    assert!(agent.commerce.customers().get_by_email("no@example.com").expect("lookup").is_none());
}
