//! Round-6 product / customer hardening on the SQLite backend.
//!
//! Covers the defects the round-6 audit raised against these repositories:
//! the missing live-reference guard on `Active -> Draft`, money validation
//! that only existed in the embedded façade, price filtering and cursor
//! pagination that disagreed with Postgres, the non-atomic find-or-create, and
//! typed conflicts that named the column instead of the offending value.
#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateProduct, CreateProductVariant, CustomerFilter,
    CustomerRepository, ProductFilter, ProductId, ProductRepository, ProductStatus, UpdateProduct,
};
use stateset_db::SqliteDatabase;
use std::sync::Arc;
use uuid::Uuid;

/// Both shapes a failed input check can take (`ValidationBuilder` reports
/// per-field `InvalidInput`, ad-hoc checks report `ValidationError`).
const fn is_rejected_input(err: &CommerceError) -> bool {
    matches!(err, CommerceError::ValidationError(_) | CommerceError::InvalidInput { .. })
}

fn product_input(slug: &str, sku: &str, price: rust_decimal::Decimal) -> CreateProduct {
    CreateProduct {
        name: format!("Product {slug}"),
        slug: Some(slug.to_string()),
        variants: Some(vec![CreateProductVariant {
            sku: sku.to_string(),
            price,
            ..Default::default()
        }]),
        ..Default::default()
    }
}

fn seed_active_cart_line(db: &SqliteDatabase, sku: &str) {
    let conn = db.pool().get().expect("connection");
    let cart_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO carts (id, cart_number, status, created_at, updated_at)
         VALUES (?, ?, 'active', datetime('now'), datetime('now'))",
        rusqlite::params![&cart_id, format!("C-{cart_id}")],
    )
    .expect("cart");
    conn.execute(
        "INSERT INTO cart_items (id, cart_id, sku, name, quantity, unit_price, total, created_at, updated_at)
         VALUES (?, ?, ?, 'Item', 1, '1', '1', datetime('now'), datetime('now'))",
        rusqlite::params![Uuid::new_v4().to_string(), &cart_id, sku],
    )
    .expect("cart item");
}

// ---------------------------------------------------------------------------
// Defect 2 — unpublishing had no live-reference guard
// ---------------------------------------------------------------------------

#[test]
fn unpublishing_a_product_is_refused_while_a_cart_holds_its_sku() {
    let db = SqliteDatabase::in_memory().expect("db");
    let products = db.products();
    let product =
        products.create(product_input("live-widget", "SKU-LIVE", dec!(19.99))).expect("create");
    products
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .expect("publish");

    seed_active_cart_line(&db, "SKU-LIVE");

    let err = products
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Draft), ..Default::default() },
        )
        .expect_err("unpublish must be refused");
    match err {
        CommerceError::Conflict(message) => {
            assert!(message.contains("unpublish"), "{message}");
            assert!(message.contains("1 active cart line"), "{message}");
        }
        other => panic!("expected Conflict, got {other:?}"),
    }

    // The SKU is still sellable, so the cart the guard protected is coherent.
    assert_eq!(products.get(product.id).expect("ok").expect("found").status, ProductStatus::Active);
}

#[test]
fn unpublishing_is_allowed_once_nothing_references_the_sku() {
    let db = SqliteDatabase::in_memory().expect("db");
    let products = db.products();
    let product =
        products.create(product_input("quiet-widget", "SKU-QUIET", dec!(5))).expect("create");
    products
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .expect("publish");
    let unpublished = products
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Draft), ..Default::default() },
        )
        .expect("unpublish");
    assert_eq!(unpublished.status, ProductStatus::Draft);
}

// ---------------------------------------------------------------------------
// Defect 3 — money validation was façade-only
// ---------------------------------------------------------------------------

#[test]
fn the_repository_rejects_variant_money_it_cannot_honour() {
    let db = SqliteDatabase::in_memory().expect("db");
    let products = db.products();

    let bad_amounts = [
        (
            "negative price",
            CreateProductVariant { sku: "SKU-NEG".into(), price: dec!(-1), ..Default::default() },
        ),
        (
            "compare-at below price",
            CreateProductVariant {
                sku: "SKU-CMP".into(),
                price: dec!(10),
                compare_at_price: Some(dec!(5)),
                ..Default::default()
            },
        ),
        (
            "scale beyond storage",
            CreateProductVariant {
                sku: "SKU-SCALE".into(),
                price: dec!(1.234567),
                ..Default::default()
            },
        ),
        (
            "negative cost",
            CreateProductVariant {
                sku: "SKU-COST".into(),
                price: dec!(1),
                cost: Some(dec!(-2)),
                ..Default::default()
            },
        ),
    ];

    // ... on create (inline variants) ...
    for (what, variant) in &bad_amounts {
        let err = products
            .create(CreateProduct {
                name: "Bad".into(),
                slug: Some(format!("bad-{}", variant.sku.to_lowercase())),
                variants: Some(vec![variant.clone()]),
                ..Default::default()
            })
            .expect_err(what);
        assert!(is_rejected_input(&err), "{what}: {err:?}");
        assert!(
            products
                .get_by_slug(&format!("bad-{}", variant.sku.to_lowercase()))
                .expect("ok")
                .is_none(),
            "{what}: nothing may be written"
        );
    }

    // ... on add_variant and update_variant.
    let product =
        products.create(product_input("good-widget", "SKU-GOOD", dec!(9.99))).expect("create");
    let existing = products.get_variants(product.id).expect("variants").remove(0);
    for (what, variant) in &bad_amounts {
        let err = products.add_variant(product.id, variant.clone()).expect_err(what);
        assert!(is_rejected_input(&err), "add_variant {what}: {err:?}");
        let err = products.update_variant(existing.id, variant.clone()).expect_err(what);
        assert!(is_rejected_input(&err), "update_variant {what}: {err:?}");
    }

    // The variant that was already there is untouched.
    let reread = products.get_variant(existing.id).expect("ok").expect("found");
    assert_eq!(reread.price, dec!(9.99));
    assert_eq!(reread.sku, "SKU-GOOD");
}

// ---------------------------------------------------------------------------
// Defect 4 — price filtering, cursor pagination and atomic find-or-create
// ---------------------------------------------------------------------------

fn seeded_catalogue() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().expect("db");
    let products = db.products();
    for (i, price) in [dec!(5), dec!(15), dec!(25), dec!(35)].into_iter().enumerate() {
        products
            .create(CreateProduct {
                name: format!("Page {i}"),
                slug: Some(format!("page-{i}")),
                variants: Some(vec![CreateProductVariant {
                    sku: format!("SKU-P{i}"),
                    price,
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .expect("create");
    }
    db
}

#[test]
fn price_filtering_paginates_the_filtered_set_not_a_thinned_page() {
    let db = seeded_catalogue();
    let products = db.products();

    // Three products are >= 15. A limit of 2 must return the first two of
    // those, not "the first two products, then filtered".
    let page = products
        .list(ProductFilter { min_price: Some(dec!(15)), limit: Some(2), ..Default::default() })
        .expect("list");
    assert_eq!(page.len(), 2, "{page:?}");
    assert_eq!(page[0].slug, "page-1");
    assert_eq!(page[1].slug, "page-2");

    // The bound itself is inclusive on both ends.
    let banded = products
        .list(ProductFilter {
            min_price: Some(dec!(15)),
            max_price: Some(dec!(25)),
            ..Default::default()
        })
        .expect("list");
    assert_eq!(banded.iter().map(|p| p.slug.as_str()).collect::<Vec<_>>(), ["page-1", "page-2"]);

    // `count` agrees with `list` — both now run the same SQL predicates.
    assert_eq!(
        products
            .count(ProductFilter { min_price: Some(dec!(15)), ..Default::default() })
            .expect("count"),
        3
    );
    assert_eq!(
        products
            .count(ProductFilter {
                min_price: Some(dec!(15)),
                max_price: Some(dec!(25)),
                ..Default::default()
            })
            .expect("count"),
        2
    );

    // Offset still applies in non-cursor mode.
    let offset_page = products
        .list(ProductFilter { min_price: Some(dec!(15)), offset: Some(1), ..Default::default() })
        .expect("list");
    assert_eq!(
        offset_page.iter().map(|p| p.slug.as_str()).collect::<Vec<_>>(),
        ["page-2", "page-3"]
    );
}

#[test]
fn cursor_pagination_ignores_offset_and_covers_every_row_exactly_once() {
    let db = seeded_catalogue();
    let products = db.products();

    let page = |after: Option<(String, String)>, offset: Option<u32>| {
        products
            .list(ProductFilter {
                limit: Some(2),
                offset,
                after_cursor: after,
                ..Default::default()
            })
            .expect("list")
    };

    let first = page(None, None);
    assert_eq!(first.len(), 2);
    let last = first.last().expect("two");
    // A stale offset must NOT be applied on top of the cursor: doing so
    // silently skipped a whole page (Postgres already suppressed it).
    let second = page(Some((last.name.clone(), last.id.to_string())), Some(2));
    assert_eq!(second.len(), 2);
    assert_eq!(
        second,
        page(Some((last.name.clone(), last.id.to_string())), None),
        "the cursor page must not depend on a leftover offset"
    );

    let mut seen: Vec<ProductId> = first.iter().chain(second.iter()).map(|p| p.id).collect();
    seen.sort();
    let mut expected: Vec<ProductId> =
        products.list(ProductFilter::default()).expect("all").into_iter().map(|p| p.id).collect();
    expected.sort();
    assert_eq!(seen, expected, "two cursor pages must cover all four rows without overlap");

    let last = second.last().expect("two");
    assert!(page(Some((last.name.clone(), last.id.to_string())), Some(2)).is_empty());

    // Offset still applies when there is no cursor.
    assert_eq!(
        page(None, Some(2)).iter().map(|p| p.slug.as_str()).collect::<Vec<_>>(),
        ["page-2", "page-3"]
    );
}

#[test]
fn find_or_create_under_a_real_thread_race_yields_exactly_one_customer() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("db"));
    let email = "race@example.com";

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let db = Arc::clone(&db);
            std::thread::spawn(move || {
                db.customers().get_or_create_by_email(CreateCustomer {
                    // Casing must not matter to the race either.
                    email: if i % 2 == 0 { "Race@Example.COM".into() } else { email.to_string() },
                    first_name: "R".into(),
                    last_name: "A".into(),
                    ..Default::default()
                })
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
    for result in &results {
        assert!(result.is_ok(), "no caller may lose with an error: {result:?}");
    }
    let created = results.iter().filter(|r| r.as_ref().expect("ok").1).count();
    assert_eq!(created, 1, "exactly one caller may report a creation");

    let ids: std::collections::HashSet<_> =
        results.iter().map(|r| r.as_ref().expect("ok").0.id).collect();
    assert_eq!(ids.len(), 1, "every caller must get the same customer");
    assert_eq!(
        db.customers()
            .count(CustomerFilter { email: Some(email.into()), ..Default::default() })
            .expect("count"),
        1
    );
}

// ---------------------------------------------------------------------------
// Defect 5 — typed conflicts must carry the offending value
// ---------------------------------------------------------------------------

#[test]
fn typed_conflicts_name_the_offending_slug_sku_and_email() {
    let db = SqliteDatabase::in_memory().expect("db");
    let products = db.products();
    products.create(product_input("taken-slug", "SKU-TAKEN", dec!(1))).expect("create");

    let err = products
        .create(product_input("taken-slug", "SKU-OTHER", dec!(1)))
        .expect_err("duplicate slug");
    match err {
        CommerceError::DuplicateSlug(value) => assert_eq!(value, "taken-slug"),
        other => panic!("expected DuplicateSlug, got {other:?}"),
    }

    let err = products
        .create(product_input("other-slug", "SKU-TAKEN", dec!(1)))
        .expect_err("duplicate sku");
    match err {
        CommerceError::DuplicateSku(value) => assert_eq!(value, "SKU-TAKEN"),
        other => panic!("expected DuplicateSku, got {other:?}"),
    }

    let customers = db.customers();
    let registration = |email: &str| CreateCustomer {
        email: email.to_string(),
        first_name: "A".into(),
        last_name: "B".into(),
        ..Default::default()
    };
    customers.create(registration("taken@example.com")).expect("create");
    let err = customers.create(registration("TAKEN@Example.com")).expect_err("duplicate email");
    match err {
        // Normalised, so the caller can echo it back safely.
        CommerceError::EmailAlreadyExists(value) => assert_eq!(value, "taken@example.com"),
        other => panic!("expected EmailAlreadyExists, got {other:?}"),
    }
}
