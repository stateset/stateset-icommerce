#![cfg(feature = "sqlite")]

use stateset_core::{CommerceError, CreateProduct, ProductRepository, UpdateProduct};
use stateset_db::SqliteDatabase;

fn setup_db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("failed to create in-memory db")
}

#[test]
fn product_update_rejects_duplicate_slug() {
    let db = setup_db();
    let first = db
        .products()
        .create(CreateProduct {
            name: "First".into(),
            slug: Some("first".into()),
            ..Default::default()
        })
        .expect("failed to create product");
    let second = db
        .products()
        .create(CreateProduct {
            name: "Second".into(),
            slug: Some("second".into()),
            ..Default::default()
        })
        .expect("failed to create product");

    let result = db
        .products()
        .update(second.id, UpdateProduct { slug: Some(first.slug.clone()), ..Default::default() });

    match result {
        Err(CommerceError::DuplicateSlug(_)) => {}
        other => panic!("expected DuplicateSlug, got {other:?}"),
    }
}

#[test]
fn product_update_batch_atomic_rejects_duplicate_slug() {
    let db = setup_db();
    let first = db
        .products()
        .create(CreateProduct {
            name: "First".into(),
            slug: Some("first".into()),
            ..Default::default()
        })
        .expect("failed to create product");
    let second = db
        .products()
        .create(CreateProduct {
            name: "Second".into(),
            slug: Some("second".into()),
            ..Default::default()
        })
        .expect("failed to create product");

    let result = db.products().update_batch_atomic(vec![(
        second.id,
        UpdateProduct { slug: Some(first.slug.clone()), ..Default::default() },
    )]);

    match result {
        Err(CommerceError::DuplicateSlug(_)) => {}
        other => panic!("expected DuplicateSlug, got {other:?}"),
    }
}
