//! Postgres mirrors of the round-5 product / customer hardening tests
//! (state machines, live-reference guards, atomic creates, e-mail
//! normalisation, address default invariants, keyset pagination, races).
//!
//! Requires `POSTGRES_URL` / `DATABASE_URL`; skips silently otherwise.
#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AddressType, CommerceError, CreateCustomer, CreateCustomerAddress, CreateOrder,
    CreateOrderItem, CreateProduct, CreateProductVariant, Customer, CustomerFilter, CustomerId,
    CustomerStatus, ProductFilter, ProductId, ProductStatus, UpdateCustomer, UpdateProduct,
    VariantPurchasability,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn setup_db() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

fn tag() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

fn product_input(tag: &str, skus: &[&str]) -> CreateProduct {
    CreateProduct {
        name: format!("Product {tag}"),
        slug: Some(format!("product-{tag}")),
        variants: Some(
            skus.iter()
                .map(|sku| CreateProductVariant {
                    sku: (*sku).to_string(),
                    price: dec!(19.99),
                    ..Default::default()
                })
                .collect(),
        ),
        ..Default::default()
    }
}

async fn make_customer(db: &PostgresDatabase, email: &str) -> Customer {
    db.customers()
        .create_async(CreateCustomer {
            email: email.to_string(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            ..Default::default()
        })
        .await
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

#[tokio::test]
async fn pg_create_product_is_atomic_when_a_later_variant_collides() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let taken = format!("SKU-{t}-A");
    db.products()
        .create_async(product_input(&format!("{t}-first"), &[&taken]))
        .await
        .expect("first");

    let fresh = format!("SKU-{t}-B");
    let err = db
        .products()
        .create_async(product_input(&format!("{t}-second"), &[&fresh, &taken]))
        .await
        .expect_err("second variant collides");
    assert!(matches!(err, CommerceError::DuplicateSku(_)), "{err:?}");
    assert!(
        db.products()
            .get_by_slug_async(&format!("product-{t}-second"))
            .await
            .expect("ok")
            .is_none(),
        "half-product must be rolled back"
    );
    assert!(db.products().get_variant_by_sku_async(&fresh).await.expect("ok").is_none());

    // Invalid inline SKU is rejected before anything is written.
    let err = db
        .products()
        .create_async(product_input(&format!("{t}-third"), &["bad sku"]))
        .await
        .expect_err("invalid sku");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert!(
        db.products().get_by_slug_async(&format!("product-{t}-third")).await.expect("ok").is_none()
    );
}

#[tokio::test]
async fn pg_delete_variant_is_soft_and_get_variants_filters_inactive() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let sku = format!("SKU-{t}");
    let p = db.products().create_async(product_input(&t, &[&sku])).await.expect("create");
    let v = db.products().get_variants_async(p.id).await.expect("ok").remove(0);
    assert_eq!(v.name, sku, "name falls back to the SKU");

    db.products().delete_variant_async(v.id).await.expect("delete");
    assert!(db.products().get_variants_async(p.id).await.expect("ok").is_empty());
    let all = db.products().get_variants_including_inactive_async(p.id).await.expect("ok");
    assert_eq!(all.len(), 1);
    assert!(!all[0].is_active);
    assert!(
        db.products().get_variant_async(v.id).await.expect("ok").is_some(),
        "row is kept so cart_items.variant_id never dangles"
    );
    assert_eq!(
        db.products().variant_purchasability_async(&sku).await.expect("ok"),
        VariantPurchasability::VariantInactive
    );
    db.products().delete_variant_async(v.id).await.expect("idempotent");
}

#[tokio::test]
async fn pg_add_variant_guards() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let err = db
        .products()
        .add_variant_public_async(
            ProductId::new(),
            CreateProductVariant {
                sku: format!("ORPHAN-{t}"),
                price: dec!(1),
                ..Default::default()
            },
        )
        .await
        .expect_err("missing product");
    assert!(matches!(err, CommerceError::ProductNotFound(_)), "{err:?}");

    let p = db
        .products()
        .create_async(product_input(&t, &[&format!("SKU-{t}")]))
        .await
        .expect("create");
    let err = db
        .products()
        .add_variant_public_async(
            p.id,
            CreateProductVariant { sku: "not a sku".into(), price: dec!(1), ..Default::default() },
        )
        .await
        .expect_err("invalid sku");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let err = db
        .products()
        .add_variant_public_async(
            p.id,
            CreateProductVariant { sku: format!("SKU-{t}"), price: dec!(1), ..Default::default() },
        )
        .await
        .expect_err("duplicate sku");
    assert!(matches!(err, CommerceError::DuplicateSku(_)), "{err:?}");

    let added = db
        .products()
        .add_variant_public_async(
            p.id,
            CreateProductVariant {
                sku: format!("SKU-{t}-2"),
                price: dec!(2),
                ..Default::default()
            },
        )
        .await
        .expect("add");
    assert_eq!(added.name, format!("SKU-{t}-2"), "returned name must not be empty");
}

#[tokio::test]
async fn pg_archive_refuses_while_active_cart_references_sku() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let sku = format!("SKU-{t}");
    let p = db.products().create_async(product_input(&t, &[&sku])).await.expect("create");

    let cart_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO carts (id, cart_number, status, created_at, updated_at) VALUES ($1, $2, 'active', NOW(), NOW())",
    )
    .bind(cart_id)
    .bind(format!("C-{t}"))
    .execute(db.pool())
    .await
    .expect("cart");
    sqlx::query(
        "INSERT INTO cart_items (id, cart_id, sku, name, quantity, unit_price, total, created_at, updated_at)
         VALUES ($1, $2, $3, 'x', 1, 1, 1, NOW(), NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(cart_id)
    .bind(&sku)
    .execute(db.pool())
    .await
    .expect("cart item");

    let err = db.products().delete_async(p.id).await.expect_err("archive refused");
    match err {
        CommerceError::Conflict(msg) => assert!(msg.contains("1 active cart line"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    let err = db
        .products()
        .update_async(
            p.id,
            UpdateProduct { status: Some(ProductStatus::Archived), ..Default::default() },
        )
        .await
        .expect_err("status update refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    let v = db.products().get_variants_async(p.id).await.expect("ok").remove(0);
    let err = db.products().delete_variant_async(v.id).await.expect_err("variant delete refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert_eq!(
        db.products().get_async(p.id).await.expect("ok").expect("found").status,
        ProductStatus::Draft
    );

    sqlx::query("UPDATE carts SET status = 'abandoned' WHERE id = $1")
        .bind(cart_id)
        .execute(db.pool())
        .await
        .expect("abandon");
    db.products().delete_async(p.id).await.expect("archive after abandon");
}

#[tokio::test]
async fn pg_archived_product_cannot_be_reactivated_and_purchasability_tracks_status() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let sku = format!("SKU-{t}");
    let p = db.products().create_async(product_input(&t, &[&sku])).await.expect("create");
    assert_eq!(
        db.products().variant_purchasability_async(&sku).await.expect("ok"),
        VariantPurchasability::ProductNotActive(ProductStatus::Draft)
    );
    db.products()
        .update_async(
            p.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .await
        .expect("activate");
    assert_eq!(
        db.products().variant_purchasability_async(&sku).await.expect("ok"),
        VariantPurchasability::Purchasable
    );
    assert_eq!(
        db.products().variant_purchasability_async(&format!("NOPE-{t}")).await.expect("ok"),
        VariantPurchasability::NotInCatalog
    );

    db.products().delete_async(p.id).await.expect("archive");
    let err = db
        .products()
        .update_async(
            p.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .await
        .expect_err("Archived -> Active refused");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    db.products().delete_async(p.id).await.expect("re-archive is a no-op");
}

#[tokio::test]
async fn pg_update_writes_only_supplied_fields() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let p = db
        .products()
        .create_async(CreateProduct {
            seo: Some(stateset_core::SeoMetadata {
                title: Some("SEO".into()),
                description: None,
                keywords: vec![],
            }),
            ..product_input(&t, &[&format!("SKU-{t}")])
        })
        .await
        .expect("create");
    let updated = db
        .products()
        .update_async(p.id, UpdateProduct { name: Some("Renamed".into()), ..Default::default() })
        .await
        .expect("update");
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.seo.as_ref().and_then(|s| s.title.clone()).as_deref(), Some("SEO"));
    assert_eq!(updated.slug, p.slug);
}

#[tokio::test]
async fn pg_product_keyset_cursor_pagination_matches_sqlite_semantics() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    // Same-named products force the (name, id) tie-break to matter.
    let mut ids = Vec::new();
    for i in 0..4 {
        let p = db
            .products()
            .create_async(CreateProduct {
                name: format!("Cursor {t}"),
                slug: Some(format!("cursor-{t}-{i}")),
                ..Default::default()
            })
            .await
            .expect("create");
        ids.push(p.id);
    }
    let name = format!("Cursor {t}");
    let page = |after: Option<(String, String)>| {
        let db = &db;
        let name = name.clone();
        async move {
            db.products()
                .list_async(ProductFilter {
                    search: Some(name),
                    limit: Some(2),
                    after_cursor: after,
                    ..Default::default()
                })
                .await
                .expect("list")
        }
    };
    let first = page(None).await;
    assert_eq!(first.len(), 2);
    let last = first.last().expect("two");
    let second = page(Some((last.name.clone(), last.id.to_string()))).await;
    assert_eq!(second.len(), 2);
    let mut seen: Vec<ProductId> = first.iter().chain(second.iter()).map(|p| p.id).collect();
    let mut expected = ids.clone();
    seen.sort();
    expected.sort();
    assert_eq!(seen, expected, "two cursor pages must cover all four rows without overlap");
    let last = second.last().expect("two");
    assert!(page(Some((last.name.clone(), last.id.to_string()))).await.is_empty());

    let err = db
        .products()
        .list_async(ProductFilter {
            after_cursor: Some((name, "not-a-uuid".into())),
            ..Default::default()
        })
        .await
        .expect_err("bad cursor");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[tokio::test]
async fn pg_concurrent_slug_and_sku_creation_yields_exactly_one_winner() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let db = std::sync::Arc::new(db);
    let mut handles = Vec::new();
    for _ in 0..8 {
        let db = std::sync::Arc::clone(&db);
        let t = t.clone();
        handles.push(tokio::spawn(async move {
            db.products().create_async(product_input(&t, &[&format!("RACE-{t}")])).await
        }));
    }
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await.expect("task"));
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
    for r in results.iter().filter(|r| r.is_err()) {
        assert!(
            matches!(r, Err(CommerceError::DuplicateSlug(_) | CommerceError::DuplicateSku(_))),
            "losers must get a typed conflict: {r:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Customers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pg_email_is_normalised_on_create_lookup_and_update() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let c = make_customer(&db, &format!("  Ada-{t}@Example.COM ")).await;
    assert_eq!(c.email, format!("ada-{t}@example.com"));
    assert_eq!(
        db.customers()
            .get_by_email_async(&format!("ADA-{t}@example.com"))
            .await
            .expect("ok")
            .expect("found")
            .id,
        c.id
    );
    let err = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("ada-{t}@EXAMPLE.com"),
            first_name: "A".into(),
            last_name: "B".into(),
            ..Default::default()
        })
        .await
        .expect_err("case collision");
    assert!(matches!(err, CommerceError::EmailAlreadyExists(_)), "{err:?}");

    let other = make_customer(&db, &format!("other-{t}@example.com")).await;
    let err = db
        .customers()
        .update_async(
            other.id,
            UpdateCustomer { email: Some(format!("Ada-{t}@Example.com")), ..Default::default() },
        )
        .await
        .expect_err("update collision");
    assert!(matches!(err, CommerceError::EmailAlreadyExists(_)), "{err:?}");

    // get_or_create is case-insensitive too.
    let same = db
        .customers()
        .get_or_create_by_email_async(CreateCustomer {
            email: format!("ADA-{t}@EXAMPLE.COM"),
            first_name: "X".into(),
            last_name: "Y".into(),
            ..Default::default()
        })
        .await
        .expect("get_or_create");
    assert_eq!(same.id, c.id);
}

#[tokio::test]
async fn pg_deleted_customer_releases_email_and_cannot_be_resurrected() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let email = format!("gone-{t}@example.com");
    let c = make_customer(&db, &email).await;
    db.customers().delete_async(c.id).await.expect("delete");
    let deleted = db.customers().get_async(c.id).await.expect("ok").expect("row kept");
    assert_eq!(deleted.status, CustomerStatus::Deleted);
    assert!(Customer::is_tombstone_email(&deleted.email), "{}", deleted.email);
    assert!(db.customers().get_by_email_async(&email).await.expect("ok").is_none());

    let again = make_customer(&db, &format!("Gone-{t}@Example.com")).await;
    assert_ne!(again.id, c.id);

    let err = db
        .customers()
        .update_async(
            c.id,
            UpdateCustomer { status: Some(CustomerStatus::Active), ..Default::default() },
        )
        .await
        .expect_err("resurrection refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    db.customers().delete_async(c.id).await.expect("idempotent");
}

#[tokio::test]
async fn pg_delete_refuses_while_open_orders_exist_and_anonymize_scrubs() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let c = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("buyer-{t}@example.com"),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            phone: Some("+44 20 7946 0958".into()),
            tags: Some(vec!["vip".into()]),
            metadata: Some(serde_json::json!({"dob": "1815-12-10"})),
            ..Default::default()
        })
        .await
        .expect("create");
    db.customers()
        .add_address_async(address(c.id, AddressType::Both, true))
        .await
        .expect("address");

    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: c.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("ORD-{t}"),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("order");

    let err = db.customers().delete_async(c.id).await.expect_err("open order blocks delete");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    let err = db.customers().anonymize_async(c.id).await.expect_err("open order blocks anonymize");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");

    sqlx::query("UPDATE orders SET status = 'cancelled' WHERE id = $1")
        .bind(order.id.into_uuid())
        .execute(db.pool())
        .await
        .expect("cancel");

    let scrubbed = db.customers().anonymize_async(c.id).await.expect("anonymize");
    assert_eq!(scrubbed.status, CustomerStatus::Deleted);
    assert_eq!(scrubbed.first_name, "Deleted");
    assert!(scrubbed.phone.is_none());
    assert!(scrubbed.tags.is_empty());
    assert!(scrubbed.metadata.is_none());
    assert!(scrubbed.default_shipping_address_id.is_none());
    assert!(Customer::is_tombstone_email(&scrubbed.email));
    assert!(db.customers().get_addresses_async(c.id).await.expect("ok").is_empty());
    assert!(matches!(
        db.customers().anonymize_async(CustomerId::new()).await,
        Err(CommerceError::CustomerNotFound(_))
    ));
}

async fn assert_default_invariant(db: &PostgresDatabase, customer_id: CustomerId) {
    let c = db.customers().get_async(customer_id).await.expect("ok").expect("found");
    let addresses = db.customers().get_addresses_async(customer_id).await.expect("ok");
    let pointed: std::collections::HashSet<Uuid> =
        [c.default_shipping_address_id, c.default_billing_address_id]
            .into_iter()
            .flatten()
            .collect();
    let flagged: std::collections::HashSet<Uuid> =
        addresses.iter().filter(|a| a.is_default).map(|a| a.id).collect();
    assert_eq!(pointed, flagged, "flagged rows must be exactly the pointed-at rows");
    if let Some(s) = c.default_shipping_address_id {
        assert!(
            addresses.iter().find(|a| a.id == s).expect("exists").address_type.covers_shipping()
        );
    }
    if let Some(b) = c.default_billing_address_id {
        assert!(
            addresses.iter().find(|a| a.id == b).expect("exists").address_type.covers_billing()
        );
    }
}

#[tokio::test]
async fn pg_address_defaults_keep_pointer_and_flag_invariant() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let c = make_customer(&db, &format!("addr-{t}@example.com")).await;
    let both = db
        .customers()
        .add_address_async(address(c.id, AddressType::Both, true))
        .await
        .expect("both");
    let ship = db
        .customers()
        .add_address_async(address(c.id, AddressType::Shipping, false))
        .await
        .expect("ship");

    db.customers()
        .set_default_address_async(c.id, ship.id, AddressType::Shipping)
        .await
        .expect("set");
    let cust = db.customers().get_async(c.id).await.expect("ok").expect("found");
    assert_eq!(cust.default_shipping_address_id, Some(ship.id));
    assert_eq!(cust.default_billing_address_id, Some(both.id), "billing default must survive");
    assert_default_invariant(&db, c.id).await;

    let err = db
        .customers()
        .set_default_address_async(c.id, ship.id, AddressType::Billing)
        .await
        .expect_err("type mismatch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    // Re-type `both` to shipping-only via update_address: it drops billing.
    let updated = db
        .customers()
        .update_address_async(both.id, address(c.id, AddressType::Shipping, true))
        .await
        .expect("update");
    assert_eq!(updated.address_type, AddressType::Shipping);
    assert!(updated.is_default);
    let cust = db.customers().get_async(c.id).await.expect("ok").expect("found");
    assert_eq!(cust.default_shipping_address_id, Some(both.id));
    assert_eq!(cust.default_billing_address_id, None);
    assert_default_invariant(&db, c.id).await;

    db.customers().delete_address_async(both.id).await.expect("delete");
    let cust = db.customers().get_async(c.id).await.expect("ok").expect("found");
    assert_eq!(cust.default_shipping_address_id, None);
    assert_default_invariant(&db, c.id).await;
}

#[tokio::test]
async fn pg_customer_keyset_cursor_pagination() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let mut ids = Vec::new();
    for i in 0..4 {
        ids.push(make_customer(&db, &format!("cursor-{t}-{i}@example.com")).await.id);
    }
    let filter = |after: Option<(String, String)>| CustomerFilter {
        email: Some(format!("cursor-{t}-")),
        limit: Some(2),
        after_cursor: after,
        ..Default::default()
    };
    let first = db.customers().list_async(filter(None)).await.expect("list");
    assert_eq!(first.len(), 2);
    let last = first.last().expect("two");
    let second = db
        .customers()
        .list_async(filter(Some((last.created_at.to_rfc3339(), last.id.to_string()))))
        .await
        .expect("page 2");
    assert_eq!(second.len(), 2);
    let mut seen: Vec<CustomerId> = first.iter().chain(second.iter()).map(|c| c.id).collect();
    seen.sort();
    ids.sort();
    assert_eq!(seen, ids, "two cursor pages must cover all four rows without overlap");
    let err = db
        .customers()
        .list_async(filter(Some(("yesterday".into(), Uuid::new_v4().to_string()))))
        .await
        .expect_err("bad cursor");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
}

#[tokio::test]
async fn pg_concurrent_case_variant_creation_yields_exactly_one_customer() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let db = std::sync::Arc::new(db);
    let mut handles = Vec::new();
    for i in 0..8 {
        let db = std::sync::Arc::clone(&db);
        let email = if i % 2 == 0 {
            format!("Race-{t}@Example.com")
        } else {
            format!("race-{t}@example.com")
        };
        handles.push(tokio::spawn(async move {
            db.customers()
                .create_async(CreateCustomer {
                    email,
                    first_name: "R".into(),
                    last_name: "A".into(),
                    ..Default::default()
                })
                .await
        }));
    }
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await.expect("task"));
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
    for r in results.iter().filter(|r| r.is_err()) {
        assert!(matches!(r, Err(CommerceError::EmailAlreadyExists(_))), "{r:?}");
    }
    let n = db
        .customers()
        .count_async(CustomerFilter { email: Some(format!("race-{t}@")), ..Default::default() })
        .await
        .expect("count");
    assert_eq!(n, 1);
}

/// Postgres twin of the SQLite `add_item_refuses_a_sku_withdrawn_from_the_catalogue`:
/// a cart line whose SKU resolves to the catalogue must still be sellable, while
/// a SKU the catalogue has never heard of stays addable as an ad-hoc line.
#[tokio::test]
async fn pg_add_item_refuses_a_sku_withdrawn_from_the_catalogue() {
    let Some(db) = setup_db().await else { return };
    let t = tag();
    let sku = format!("SKU-{t}");
    let product = db.products().create_async(product_input(&t, &[&sku])).await.expect("create");
    db.products()
        .update_async(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .await
        .expect("activate");

    let cart =
        db.carts().create_async(stateset_core::CreateCart::default()).await.expect("create cart");
    let line = |sku: &str| stateset_core::AddCartItem {
        sku: sku.to_string(),
        name: "Item".to_string(),
        quantity: 1,
        unit_price: dec!(19.99),
        ..Default::default()
    };

    db.carts()
        .add_item_async(cart.id.into_uuid(), line(&sku))
        .await
        .expect("an active catalogue SKU is sellable");
    let adhoc = format!("SKU-ADHOC-{t}");
    db.carts().add_item_async(cart.id.into_uuid(), line(&adhoc)).await.expect("ad-hoc line");

    // Withdraw a product nothing references (archiving the one above is refused
    // precisely because the cart holds it).
    let t2 = tag();
    let sku2 = format!("SKU-{t2}");
    let withdrawn = db.products().create_async(product_input(&t2, &[&sku2])).await.expect("create");
    db.products()
        .update_async(
            withdrawn.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .await
        .expect("activate");
    db.products()
        .update_async(
            withdrawn.id,
            UpdateProduct { status: Some(ProductStatus::Archived), ..Default::default() },
        )
        .await
        .expect("archive");

    let err = db
        .carts()
        .add_item_async(cart.id.into_uuid(), line(&sku2))
        .await
        .expect_err("an archived product must not be addable");
    match err {
        CommerceError::ValidationError(message) => {
            assert!(message.contains(&sku2), "{message}");
            assert!(message.contains("not purchasable"), "{message}");
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }

    db.carts().add_item_async(cart.id.into_uuid(), line(&adhoc)).await.expect("ad-hoc still fine");
}
