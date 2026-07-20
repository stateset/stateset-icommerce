//! Postgres `update_type` for custom object types must succeed.
//!
//! The optimistic-locking UPDATE used `RETURNING 1` (an INT4 literal) decoded as
//! `(i64,)`, which fails on Postgres with a type mismatch — so updating a custom
//! object type errored even for a valid update (SQLite, being dynamically typed,
//! worked). Decoding as `i32` fixes it.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{
    CreateCustomObjectType, CustomFieldDefinition, CustomFieldType, UpdateCustomObjectType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_update_custom_object_type_succeeds() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping custom object type update test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let co = commerce.custom_objects();

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let ty = co
        .create_type(CreateCustomObjectType {
            handle: format!("thing_{}__c", &unique[..8]),
            display_name: "Thing".into(),
            description: None,
            fields: vec![CustomFieldDefinition {
                key: "name".into(),
                field_type: CustomFieldType::String,
                required: false,
                list: false,
                description: None,
            }],
        })
        .await
        .expect("create type");

    let updated = co
        .update_type(
            ty.id,
            UpdateCustomObjectType {
                display_name: Some("Thing Renamed".into()),
                description: None,
                fields: None,
            },
        )
        .await
        .expect("update type must succeed");
    assert_eq!(updated.display_name, "Thing Renamed");
}
