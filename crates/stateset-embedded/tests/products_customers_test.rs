#![cfg(feature = "sqlite")]

//! Embedded-API coverage for products and customers (previously untested at
//! this layer): validation at the façade, the product / customer state
//! machines, live-reference guards, e-mail normalisation, deletion and
//! anonymisation, and address default invariants.

use rust_decimal_macros::dec;
use stateset_embedded::prelude::*;
use stateset_embedded::{
    AddressType, Commerce, CommerceError, CreateCustomer, CreateCustomerAddress, CreateOrder,
    CreateOrderItem, CreateProduct, CreateProductVariant, CustomerFilter, CustomerId,
    CustomerStatus, ProductFilter, ProductId, ProductStatus, UpdateCustomer,
};

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("in-memory commerce")
}

fn product(commerce: &Commerce, slug: &str, sku: &str) -> Product {
    commerce
        .products()
        .create(CreateProduct {
            name: format!("Product {slug}"),
            slug: Some(slug.into()),
            variants: Some(vec![CreateProductVariant {
                sku: sku.into(),
                price: dec!(25.00),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect("create product")
}

fn customer(commerce: &Commerce, email: &str) -> Customer {
    commerce
        .customers()
        .create(CreateCustomer {
            email: email.into(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            ..Default::default()
        })
        .expect("create customer")
}

fn address(
    customer_id: CustomerId,
    address_type: AddressType,
    is_default: bool,
) -> CreateCustomerAddress {
    CreateCustomerAddress {
        customer_id,
        address_type: Some(address_type),
        first_name: "Ada".into(),
        last_name: "Lovelace".into(),
        company: None,
        line1: "1 Analytical Way".into(),
        line2: None,
        city: "London".into(),
        state: None,
        postal_code: "SW1A 1AA".into(),
        country: "GB".into(),
        phone: None,
        is_default: Some(is_default),
    }
}

// ---------------------------------------------------------------------------
// Products
// ---------------------------------------------------------------------------

#[test]
fn product_create_validates_variant_prices_at_the_facade() {
    let c = commerce();
    let err = c
        .products()
        .create(CreateProduct {
            name: "Bad money".into(),
            variants: Some(vec![CreateProductVariant {
                sku: "BAD-MONEY".into(),
                price: dec!(10.00),
                compare_at_price: Some(dec!(5.00)),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect_err("compare_at below price is rejected");
    assert!(
        matches!(err, CommerceError::InvalidInput { ref field, .. } if field == "compare_at_price")
    );

    let err = c
        .products()
        .create(CreateProduct {
            name: "Too fine".into(),
            variants: Some(vec![CreateProductVariant {
                sku: "FINE".into(),
                price: dec!(1.00001),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect_err("sub-0.0001 price is rejected");
    assert!(matches!(err, CommerceError::InvalidInput { ref field, .. } if field == "price"));
    assert!(c.products().get_by_slug("too-fine").expect("ok").is_none(), "nothing persisted");
}

#[test]
fn product_lifecycle_activate_archive_and_terminal_state() {
    let c = commerce();
    let p = product(&c, "widget", "WIDGET-1");
    assert_eq!(p.status, ProductStatus::Draft);

    let active = c.products().activate(p.id).expect("activate");
    assert!(active.is_purchasable());
    assert_eq!(c.products().list_active().expect("ok").len(), 1);

    let archived = c.products().archive(p.id).expect("archive");
    assert_eq!(archived.status, ProductStatus::Archived);
    assert!(
        c.products().list(ProductFilter::default()).expect("ok").is_empty(),
        "archived hidden by default"
    );

    let err = c.products().activate(p.id).expect_err("Archived is terminal");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    c.products().delete(p.id).expect("delete of archived product is idempotent");
}

#[test]
fn product_archive_is_refused_while_an_open_order_references_its_sku() {
    let c = commerce();
    let p = product(&c, "ordered", "ORDERED-1");
    c.products().activate(p.id).expect("activate");
    let cust = customer(&c, "buyer@example.com");
    c.orders()
        .create(CreateOrder {
            customer_id: cust.id,
            items: vec![CreateOrderItem {
                product_id: p.id,
                sku: "ORDERED-1".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(25.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("order");

    let err = c.products().archive(p.id).expect_err("open order blocks archive");
    match err {
        CommerceError::Conflict(msg) => assert!(msg.contains("open order line"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    let err = c.products().delete(p.id).expect_err("delete is refused too");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    let v = c.products().get_variants(p.id).expect("ok").remove(0);
    let err = c.products().delete_variant(v.id).expect_err("variant delete is refused too");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(c.products().get(p.id).expect("ok").expect("found").status, ProductStatus::Active);

    // The customer also cannot be deleted while that order is open.
    let err = c.customers().delete(cust.id).expect_err("open order blocks customer delete");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
}

#[test]
fn product_variant_guards_at_the_facade() {
    let c = commerce();
    let p = product(&c, "guarded", "GUARD-1");

    let err = c
        .products()
        .add_variant(
            ProductId::new(),
            CreateProductVariant { sku: "ORPHAN".into(), price: dec!(1), ..Default::default() },
        )
        .expect_err("unknown product");
    assert!(matches!(err, CommerceError::ProductNotFound(_)), "{err:?}");

    let err = c
        .products()
        .add_variant(
            p.id,
            CreateProductVariant { sku: "GUARD-1".into(), price: dec!(1), ..Default::default() },
        )
        .expect_err("duplicate sku");
    assert!(matches!(err, CommerceError::DuplicateSku(_)), "{err:?}");

    let v2 = c
        .products()
        .add_variant(
            p.id,
            CreateProductVariant { sku: "GUARD-2".into(), price: dec!(2), ..Default::default() },
        )
        .expect("second variant");
    assert_eq!(v2.name, "GUARD-2");
    c.products().delete_variant(v2.id).expect("soft delete");
    assert_eq!(
        c.products().get_variants(p.id).expect("ok").len(),
        1,
        "inactive variants are hidden"
    );
    assert!(c.products().get_variant(v2.id).expect("ok").is_some_and(|v| !v.is_active));
}

// ---------------------------------------------------------------------------
// Customers
// ---------------------------------------------------------------------------

#[test]
fn customer_email_is_case_insensitive_and_find_or_create_never_duplicates() {
    let c = commerce();
    let first = customer(&c, "Ada@Example.com");
    assert_eq!(first.email, "ada@example.com");

    let found = c
        .customers()
        .find_or_create(CreateCustomer {
            email: "ADA@example.COM".into(),
            first_name: "Other".into(),
            last_name: "Person".into(),
            ..Default::default()
        })
        .expect("find_or_create");
    assert_eq!(found.id, first.id);
    assert_eq!(c.customers().count(CustomerFilter::default()).expect("count"), 1);

    let err = c
        .customers()
        .create(CreateCustomer {
            email: "ada@example.com ".into(),
            first_name: "Dup".into(),
            last_name: "Licate".into(),
            ..Default::default()
        })
        .expect_err("duplicate");
    assert!(matches!(err, CommerceError::EmailAlreadyExists(_)), "{err:?}");
}

#[test]
fn customer_delete_releases_email_and_is_terminal() {
    let c = commerce();
    let cust = customer(&c, "gone@example.com");
    c.customers().delete(cust.id).expect("delete");

    let deleted = c.customers().get(cust.id).expect("ok").expect("row kept");
    assert_eq!(deleted.status, CustomerStatus::Deleted);
    assert!(Customer::is_tombstone_email(&deleted.email));
    assert!(
        c.customers().list(CustomerFilter::default()).expect("ok").is_empty(),
        "hidden by default"
    );

    let again = customer(&c, "gone@example.com");
    assert_ne!(again.id, cust.id, "re-registration creates a fresh account");

    let err = c
        .customers()
        .update(
            cust.id,
            UpdateCustomer { status: Some(CustomerStatus::Active), ..Default::default() },
        )
        .expect_err("resurrection refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");

    let err = c
        .customers()
        .update(
            again.id,
            UpdateCustomer { status: Some(CustomerStatus::Deleted), ..Default::default() },
        )
        .expect_err("deleted must go through delete()");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[test]
fn customer_anonymize_scrubs_pii() {
    let c = commerce();
    let cust = c
        .customers()
        .create(CreateCustomer {
            email: "pii@example.com".into(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            phone: Some("+44 20 7946 0958".into()),
            tags: Some(vec!["vip".into()]),
            metadata: Some(serde_json::json!({"dob": "1815-12-10"})),
            ..Default::default()
        })
        .expect("create");
    c.customers().add_address(address(cust.id, AddressType::Both, true)).expect("address");

    let scrubbed = c.customers().anonymize(cust.id).expect("anonymize");
    assert_eq!(scrubbed.status, CustomerStatus::Deleted);
    assert_eq!(scrubbed.first_name, "Deleted");
    assert!(scrubbed.phone.is_none() && scrubbed.tags.is_empty() && scrubbed.metadata.is_none());
    assert!(
        scrubbed.default_shipping_address_id.is_none()
            && scrubbed.default_billing_address_id.is_none()
    );
    assert!(Customer::is_tombstone_email(&scrubbed.email));
    assert!(c.customers().get_addresses(cust.id).expect("ok").is_empty());
}

#[test]
fn customer_address_defaults_stay_consistent() {
    let c = commerce();
    let cust = customer(&c, "addr@example.com");
    let both = c.customers().add_address(address(cust.id, AddressType::Both, true)).expect("both");
    let ship =
        c.customers().add_address(address(cust.id, AddressType::Shipping, false)).expect("ship");

    c.customers().set_default_address(cust.id, ship.id, AddressType::Shipping).expect("set");
    let after = c.customers().get(cust.id).expect("ok").expect("found");
    assert_eq!(after.default_shipping_address_id, Some(ship.id));
    assert_eq!(after.default_billing_address_id, Some(both.id));
    let addresses = c.customers().get_addresses(cust.id).expect("ok");
    assert!(addresses.iter().all(|a| a.is_default), "both rows are a default for some role");

    let err = c
        .customers()
        .set_default_address(cust.id, ship.id, AddressType::Billing)
        .expect_err("shipping-only address cannot be the billing default");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    c.customers().delete_address(both.id).expect("delete");
    let after = c.customers().get(cust.id).expect("ok").expect("found");
    assert_eq!(after.default_billing_address_id, None);
    assert_eq!(after.default_shipping_address_id, Some(ship.id));
}
