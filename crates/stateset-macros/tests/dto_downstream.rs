//! Integration tests for the `GenerateDto` derive macro.
//!
//! These tests verify that the generated DTO structs compile and behave
//! correctly when used from a downstream crate.

use stateset_macros::GenerateDto;

#[derive(Debug, GenerateDto)]
#[dto(create, update, filter)]
pub struct Product {
    #[dto(skip_create)]
    pub id: String,
    pub name: String,
    pub sku: String,
    pub price: f64,
    #[dto(skip_update)]
    pub created_at: String,
}

#[derive(Debug, GenerateDto)]
#[dto(create)]
pub struct SimpleItem {
    pub title: String,
    pub quantity: i32,
}

#[derive(Debug, GenerateDto)]
#[dto(create, update)]
pub struct WithOptional {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, GenerateDto)]
#[dto(filter)]
pub struct Warehouse {
    pub location: String,
    pub capacity: i64,
    #[dto(skip_filter)]
    pub internal_code: String,
}

#[test]
fn create_product_has_correct_fields() {
    let dto = CreateProduct {
        name: "Widget".to_string(),
        sku: "WDG-001".to_string(),
        price: 9.99,
        created_at: "2026-01-01".to_string(),
    };
    assert_eq!(dto.name, "Widget");
    assert_eq!(dto.sku, "WDG-001");
    assert!((dto.price - 9.99).abs() < f64::EPSILON);
    assert_eq!(dto.created_at, "2026-01-01");
}

#[test]
fn create_product_skips_id() {
    // CreateProduct should not have an `id` field — this is a compile-time check.
    // If `id` were present, the struct literal above would need it.
    let dto = CreateProduct {
        name: String::new(),
        sku: String::new(),
        price: 0.0,
        created_at: String::new(),
    };
    let _ = dto;
}

#[test]
fn update_product_all_fields_optional() {
    let dto = UpdateProduct {
        name: Some("New name".to_string()),
        sku: None,
        price: Some(19.99),
        id: None,
    };
    assert_eq!(dto.name, Some("New name".to_string()));
    assert_eq!(dto.sku, None);
}

#[test]
fn update_product_skips_created_at() {
    // UpdateProduct should not have `created_at` — compile-time check.
    let dto = UpdateProduct { name: None, sku: None, price: None, id: None };
    let _ = dto;
}

#[test]
fn filter_product_has_limit_offset() {
    let filter = ProductFilter {
        id: Some("abc".to_string()),
        name: None,
        sku: None,
        price: None,
        created_at: None,
        limit: Some(10),
        offset: Some(0),
    };
    assert_eq!(filter.limit, Some(10));
    assert_eq!(filter.offset, Some(0));
}

#[test]
fn create_simple_item_compiles() {
    let item = CreateSimpleItem { title: "Test".to_string(), quantity: 5 };
    assert_eq!(item.title, "Test");
    assert_eq!(item.quantity, 5);
}

#[test]
fn dto_with_optional_fields_in_create() {
    let dto = CreateWithOptional {
        name: "Test".to_string(),
        description: Some("A description".to_string()),
        tags: vec!["a".to_string(), "b".to_string()],
    };
    assert_eq!(dto.description, Some("A description".to_string()));
    assert_eq!(dto.tags.len(), 2);
}

#[test]
fn dto_with_vec_field_in_update() {
    // Update wraps everything in Option, so Vec becomes Option<Vec<String>>
    let dto = UpdateWithOptional {
        name: Some("Updated".to_string()),
        description: None,
        tags: Some(vec!["new-tag".to_string()]),
    };
    assert_eq!(dto.tags, Some(vec!["new-tag".to_string()]));
}

#[test]
fn filter_skip_filter_excludes_field() {
    // WarehouseFilter should not have `internal_code`
    let filter = WarehouseFilter {
        location: Some("NYC".to_string()),
        capacity: Some(1000),
        limit: None,
        offset: None,
    };
    assert_eq!(filter.location, Some("NYC".to_string()));
}

#[test]
fn dto_structs_implement_debug() {
    let dto = CreateSimpleItem { title: "Test".to_string(), quantity: 1 };
    let debug = format!("{:?}", dto);
    assert!(debug.contains("Test"), "Debug output should contain field values");
}

#[test]
fn dto_structs_implement_clone() {
    let dto = CreateSimpleItem { title: "Original".to_string(), quantity: 42 };
    #[allow(clippy::redundant_clone)]
    let cloned = dto.clone();
    assert_eq!(cloned.title, "Original");
    assert_eq!(cloned.quantity, 42);
}

#[test]
fn dto_structs_implement_default() {
    let dto = CreateSimpleItem::default();
    assert_eq!(dto.title, "");
    assert_eq!(dto.quantity, 0);
}

#[test]
fn dto_structs_implement_serialize() {
    let dto = CreateSimpleItem { title: "Widget".to_string(), quantity: 10 };
    let json = serde_json::to_string(&dto).expect("serialize");
    assert!(json.contains("Widget"));
    assert!(json.contains("10"));
}

#[test]
fn dto_structs_implement_deserialize() {
    let json = r#"{"title":"Gadget","quantity":5}"#;
    let dto: CreateSimpleItem = serde_json::from_str(json).expect("deserialize");
    assert_eq!(dto.title, "Gadget");
    assert_eq!(dto.quantity, 5);
}

#[test]
fn filter_default_has_none_limit_offset() {
    let filter = ProductFilter::default();
    assert_eq!(filter.limit, None);
    assert_eq!(filter.offset, None);
    assert_eq!(filter.name, None);
}
