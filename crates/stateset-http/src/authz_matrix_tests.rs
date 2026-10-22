//! Table-driven denial tests over the whole route inventory.
//!
//! For every row of [`ROUTE_POLICIES`] this drives a real request through the
//! full [`ServerBuilder`] stack (bearer auth, role engine, ownership layer,
//! handler) as customer **A**, using customer **B**'s record ids, and checks
//! the outcome the row's access class promises. Owned rows are also checked
//! for A's own records and for the operator.

use std::collections::HashMap;

use axum::{Router, body::Body, http::Request, http::StatusCode};
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use stateset_authz::{AuthzEngineBuilder, Role};
use stateset_core::{
    BillingInterval, CreateCart, CreateCustomer, CreateInvoice, CreateLoyaltyProgram, CreateOrder,
    CreateOrderItem, CreatePayment, CreateProduct, CreateReturn, CreateReturnItem, CreateReview,
    CreateShipment, CreateStoreCredit, CreateSubscription, CreateSubscriptionPlan, CreateWarranty,
    CreateWishlist, EnrollCustomer, OrderStatus, PaymentMethodType, ReturnReason,
    StoreCreditReason, UpdateOrder,
};
use stateset_embedded::Commerce;
use tower::ServiceExt;
use uuid::Uuid;

use crate::ServerBuilder;
use crate::route_policy::{Access, BodyOwner, OwnerKind, ROUTE_POLICIES, RoutePolicy};

pub(crate) const OPERATOR_TOKEN: &str = "operator-token";
pub(crate) const CUSTOMER_A_TOKEN: &str = "customer-a-token";
pub(crate) const CUSTOMER_B_TOKEN: &str = "customer-b-token";

/// One customer's seeded records, keyed by owner kind.
pub(crate) struct Seeds {
    pub(crate) customer: Uuid,
    pub(crate) ids: HashMap<OwnerKind, Uuid>,
}

impl Seeds {
    pub(crate) fn id(&self, kind: OwnerKind) -> Uuid {
        self.ids[&kind]
    }
}

fn seed_customer(commerce: &Commerce, label: &str, product: stateset_core::ProductId) -> Seeds {
    let unique = Uuid::new_v4().simple().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("{label}-{unique}@example.com"),
            first_name: label.into(),
            last_name: "Owner".into(),
            ..Default::default()
        })
        .unwrap();
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product,
                sku: format!("SKU-{unique}"),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    // A second, shipped order carries the return and the shipment so the first
    // stays cancellable.
    let shipped = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product,
                sku: format!("SKU-S-{unique}"),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        commerce
            .orders()
            .update(shipped.id, UpdateOrder { status: Some(status), ..Default::default() })
            .unwrap();
    }
    let shipped = commerce.orders().get(shipped.id).unwrap().unwrap();
    let ret = commerce
        .returns()
        .create(CreateReturn {
            order_id: shipped.id,
            reason: ReturnReason::Defective,
            reason_details: None,
            idempotency_key: None,
            items: vec![CreateReturnItem {
                order_item_id: shipped.items[0].id,
                quantity: 1,
                condition: None,
            }],
            notes: None,
        })
        .unwrap();
    let shipment = commerce
        .shipments()
        .create(CreateShipment { order_id: shipped.id, ..Default::default() })
        .unwrap();
    let cart = commerce
        .carts()
        .create(CreateCart { customer_id: Some(customer.id), ..Default::default() })
        .unwrap();
    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            customer_id: Some(customer.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(20),
            ..Default::default()
        })
        .unwrap();
    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: format!("Plan {unique}"),
            billing_interval: BillingInterval::Monthly,
            price: dec!(9),
            ..Default::default()
        })
        .unwrap();
    commerce.subscriptions().activate_plan(plan.id).unwrap();
    let subscription = commerce
        .subscriptions()
        .subscribe(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            ..Default::default()
        })
        .unwrap();
    let review = commerce
        .reviews()
        .create(CreateReview {
            product_id: product,
            customer_id: customer.id,
            rating: 5,
            title: None,
            body: None,
            verified_purchase: false,
        })
        .unwrap();
    let wishlist = commerce
        .wishlists()
        .create(CreateWishlist { customer_id: customer.id, name: "Mine".into(), is_public: false })
        .unwrap();
    let store_credit = commerce
        .store_credits()
        .create(CreateStoreCredit {
            customer_id: customer.id,
            amount: dec!(5),
            currency: stateset_core::CurrencyCode::USD,
            reason: StoreCreditReason::Manual,
            reference_id: None,
            note: None,
            expires_at: None,
        })
        .unwrap();
    let invoice = commerce
        .invoices()
        .create(CreateInvoice { customer_id: customer.id, ..Default::default() })
        .unwrap();
    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id: customer.id,
            order_id: Some(order.id),
            product_id: Some(product),
            ..Default::default()
        })
        .unwrap();
    let program = commerce
        .loyalty()
        .create_program(CreateLoyaltyProgram {
            name: format!("Program {unique}"),
            description: None,
            points_per_dollar: 1,
            tiers: vec![],
        })
        .unwrap();
    let loyalty = commerce
        .loyalty()
        .enroll(EnrollCustomer { customer_id: customer.id, program_id: program.id })
        .unwrap();

    let ids = HashMap::from([
        (OwnerKind::Customer, customer.id.into_uuid()),
        (OwnerKind::Order, order.id.into_uuid()),
        (OwnerKind::Cart, cart.id.into_uuid()),
        (OwnerKind::Payment, payment.id.into_uuid()),
        (OwnerKind::Return, ret.id.into_uuid()),
        (OwnerKind::Shipment, shipment.id.into_uuid()),
        (OwnerKind::Subscription, subscription.id.into_uuid()),
        (OwnerKind::Review, review.id.into_uuid()),
        (OwnerKind::Wishlist, wishlist.id.into_uuid()),
        (OwnerKind::StoreCredit, store_credit.id.into_uuid()),
        (OwnerKind::Invoice, invoice.id.into()),
        (OwnerKind::Warranty, warranty.id.into_uuid()),
        (OwnerKind::LoyaltyAccount, loyalty.id.into_uuid()),
    ]);
    Seeds { customer: customer.id.into_uuid(), ids }
}

/// A server with an operator, customer A and customer B, plus their seeds.
pub(crate) struct Fixture {
    pub(crate) app: Router,
    pub(crate) a: Seeds,
    pub(crate) b: Seeds,
}

/// Build the fixture. With `with_engine`, customers hold the full `operator`
/// role, so every denial below comes from the ownership layer, not the role.
pub(crate) fn fixture(with_engine: bool) -> Fixture {
    let commerce = Commerce::new(":memory:").unwrap();
    let product = commerce
        .products()
        .create(CreateProduct { name: "Shared widget".into(), ..Default::default() })
        .unwrap()
        .id;
    let a = seed_customer(&commerce, "a", product);
    let b = seed_customer(&commerce, "b", product);

    let mut builder = ServerBuilder::new(commerce)
        .without_auth()
        .without_background_sweeps()
        .with_ignore_tenant_header()
        .add_bearer_auth_for_actor(OPERATOR_TOKEN, "op-1")
        .add_bearer_auth_for_customer(CUSTOMER_A_TOKEN, "cust-a", a.customer.to_string())
        .add_bearer_auth_for_customer(CUSTOMER_B_TOKEN, "cust-b", b.customer.to_string());
    if with_engine {
        builder = builder.with_authz_engine(
            AuthzEngineBuilder::new()
                .add_role(Role::admin())
                .add_role(Role::operator())
                .assign_role("op-1", "admin")
                .assign_role("cust-a", "operator")
                .assign_role("cust-b", "operator")
                .build(),
        );
    }
    Fixture { app: builder.build(), a, b }
}

pub(crate) async fn call(
    app: &Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-tenant-id", "default")
        .header("idempotency-key", Uuid::new_v4().to_string());
    let body = match body {
        Some(value) => {
            request = request.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Fill a route template: the owned parameter gets `owned`, everything else a
/// random UUID.
fn concrete(policy: &RoutePolicy, owned: Option<(&str, Uuid)>) -> String {
    policy
        .path
        .split('/')
        .map(|segment| match segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            Some(name) => match owned {
                Some((param, id)) if param == name => id.to_string(),
                _ => Uuid::new_v4().to_string(),
            },
            None => segment.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The record kind a customer-filtered list returns.
fn list_kind(path: &str) -> OwnerKind {
    match path {
        "/api/v1/orders" => OwnerKind::Order,
        "/api/v1/carts" => OwnerKind::Cart,
        "/api/v1/payments" => OwnerKind::Payment,
        "/api/v1/returns" => OwnerKind::Return,
        "/api/v1/subscriptions" => OwnerKind::Subscription,
        "/api/v1/wishlists" => OwnerKind::Wishlist,
        "/api/v1/store-credits" => OwnerKind::StoreCredit,
        "/api/v1/invoices" => OwnerKind::Invoice,
        "/api/v1/warranties" => OwnerKind::Warranty,
        other => panic!("owned-list route {other} has no list_kind mapping"),
    }
}

fn error_code(body: &Value) -> &str {
    body["error"]["code"].as_str().or_else(|| body["code"].as_str()).unwrap_or_default()
}

#[tokio::test]
async fn customer_principals_are_confined_on_every_classified_route() {
    for with_engine in [true, false] {
        let fx = fixture(with_engine);
        let mut checked = HashMap::<&str, usize>::new();
        for policy in ROUTE_POLICIES {
            let label = format!("[engine={with_engine}] {} {}", policy.method, policy.path);
            match policy.access {
                Access::Infra => continue,
                Access::Operator => {
                    let (status, body) = call(
                        &fx.app,
                        CUSTOMER_A_TOKEN,
                        policy.method,
                        &concrete(policy, None),
                        (policy.method != "GET").then(|| json!({})),
                    )
                    .await;
                    assert_eq!(status, StatusCode::FORBIDDEN, "{label}: {body}");
                }
                Access::Public => {
                    let (status, body) = call(
                        &fx.app,
                        CUSTOMER_A_TOKEN,
                        policy.method,
                        &concrete(policy, None),
                        None,
                    )
                    .await;
                    assert_ne!(status, StatusCode::FORBIDDEN, "{label}: {body}");
                    assert_ne!(status, StatusCode::UNAUTHORIZED, "{label}: {body}");
                }
                Access::Owned { kind, param } => {
                    let others = concrete(policy, Some((param, fx.b.id(kind))));
                    let body =
                        (policy.method != "GET" && policy.method != "DELETE").then(|| json!({}));
                    let (status, response) =
                        call(&fx.app, CUSTOMER_A_TOKEN, policy.method, &others, body.clone()).await;
                    assert_eq!(status, StatusCode::NOT_FOUND, "{label}: {response}");
                    // Indistinguishable from a missing record.
                    let missing = concrete(policy, Some((param, Uuid::new_v4())));
                    let (missing_status, missing_body) =
                        call(&fx.app, CUSTOMER_A_TOKEN, policy.method, &missing, body).await;
                    assert_eq!(missing_status, StatusCode::NOT_FOUND, "{label}");
                    assert_eq!(error_code(&response), error_code(&missing_body), "{label}");

                    if policy.method == "GET" {
                        let own = concrete(policy, Some((param, fx.a.id(kind))));
                        let (status, body) =
                            call(&fx.app, CUSTOMER_A_TOKEN, "GET", &own, None).await;
                        assert_eq!(status, StatusCode::OK, "{label} own record: {body}");
                        let (status, body) =
                            call(&fx.app, OPERATOR_TOKEN, "GET", &others, None).await;
                        assert_eq!(status, StatusCode::OK, "{label} operator: {body}");
                    }
                }
                Access::OwnedList { filter } => {
                    let tampered = format!("{}?{filter}={}&limit=200", policy.path, fx.b.customer);
                    let (status, body) =
                        call(&fx.app, CUSTOMER_A_TOKEN, "GET", &tampered, None).await;
                    assert_eq!(status, StatusCode::OK, "{label}: {body}");
                    let kind = list_kind(policy.path);
                    let text = body.to_string();
                    for foreign in [fx.b.customer, fx.b.id(kind)] {
                        assert!(
                            !text.contains(&foreign.to_string()),
                            "{label}: customer A saw customer B's records: {text}"
                        );
                    }
                    assert!(
                        text.contains(&fx.a.id(kind).to_string()),
                        "{label}: customer A's own record missing: {text}"
                    );
                    let (status, body) = call(
                        &fx.app,
                        OPERATOR_TOKEN,
                        "GET",
                        &format!("{}?limit=200", policy.path),
                        None,
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK, "{label} operator: {body}");
                }
                Access::OwnedCreate { body } => {
                    let (field, payload) = match body {
                        BodyOwner::Customer(field) => {
                            (field, json!({ field: fx.b.customer.to_string() }))
                        }
                        BodyOwner::Reference { field, kind, .. } => {
                            (field, json!({ field: fx.b.id(kind).to_string() }))
                        }
                    };
                    let (status, response) =
                        call(&fx.app, CUSTOMER_A_TOKEN, "POST", policy.path, Some(payload)).await;
                    let expected = match body {
                        BodyOwner::Customer(_) => StatusCode::FORBIDDEN,
                        BodyOwner::Reference { .. } => StatusCode::NOT_FOUND,
                    };
                    assert_eq!(status, expected, "{label} ({field}): {response}");
                }
            }
            *checked.entry(policy.access.label()).or_default() += 1;
        }
        assert!(checked["operator"] > 300, "{checked:?}");
        assert!(checked["owned"] >= 30, "{checked:?}");
        assert!(checked["owned-list"] >= 9, "{checked:?}");
        assert!(checked["owned-create"] >= 9, "{checked:?}");
    }
}
