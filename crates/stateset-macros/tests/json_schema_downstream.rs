//! Integration tests for the `JsonSchema` derive macro.
//!
//! These tests verify that the generated schema functions compile and produce
//! correct JSON Schema output at runtime.

use stateset_macros::JsonSchema;

#[derive(Debug, JsonSchema, serde::Serialize)]
pub struct ExactMoney {
    pub amount: rust_decimal::Decimal,
}

#[test]
fn decimal_schema_matches_wire_serialization() {
    let value = ExactMoney { amount: "12345678901234567890.12".parse().unwrap() };
    let wire = serde_json::to_value(&value).unwrap();
    assert!(wire["amount"].is_string());
    assert_eq!(exact_money_json_schema()["properties"]["amount"]["type"], "string");
}

#[derive(Debug, JsonSchema)]
pub struct CreateOrder {
    pub customer_id: String,
    pub amount: f64,
    pub items: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, JsonSchema)]
pub struct WithUuids {
    pub id: uuid::Uuid,
    pub related_ids: Vec<uuid::Uuid>,
}

#[derive(Debug, JsonSchema)]
pub struct AllTypes {
    pub name: String,
    pub count: i32,
    pub big_count: i64,
    pub small_count: u8,
    pub price: f64,
    pub active: bool,
    pub id: uuid::Uuid,
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, JsonSchema)]
pub struct EmptySchema {}

#[derive(Debug, JsonSchema)]
pub struct AllOptional {
    pub name: Option<String>,
    pub count: Option<i32>,
}

#[test]
fn schema_is_object_type() {
    let schema = create_order_json_schema();
    assert_eq!(schema["type"], "object");
}

#[test]
fn schema_has_properties() {
    let schema = create_order_json_schema();
    let props = schema["properties"].as_object().expect("properties should be an object");
    assert!(props.contains_key("customer_id"));
    assert!(props.contains_key("amount"));
    assert!(props.contains_key("items"));
    assert!(props.contains_key("note"));
}

#[test]
fn string_field_schema() {
    let schema = create_order_json_schema();
    assert_eq!(schema["properties"]["customer_id"]["type"], "string");
}

#[test]
fn number_field_schema() {
    let schema = create_order_json_schema();
    assert_eq!(schema["properties"]["amount"]["type"], "number");
}

#[test]
fn array_field_schema() {
    let schema = create_order_json_schema();
    let items_schema = &schema["properties"]["items"];
    assert_eq!(items_schema["type"], "array");
    assert_eq!(items_schema["items"]["type"], "string");
}

#[test]
fn optional_field_in_properties_but_not_required() {
    let schema = create_order_json_schema();
    // note should be in properties
    assert_eq!(schema["properties"]["note"]["anyOf"][0]["type"], "string");
    assert_eq!(schema["properties"]["note"]["anyOf"][1]["type"], "null");
    // note should NOT be in required
    let required = schema["required"].as_array().expect("required should be an array");
    let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(!required_names.contains(&"note"), "optional field should not be in required");
}

#[test]
fn required_fields_listed() {
    let schema = create_order_json_schema();
    let required = schema["required"].as_array().expect("required should be an array");
    let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_names.contains(&"customer_id"));
    assert!(required_names.contains(&"amount"));
    assert!(required_names.contains(&"items"));
}

#[test]
fn uuid_field_has_format() {
    let schema = with_uuids_json_schema();
    assert_eq!(schema["properties"]["id"]["type"], "string");
    assert_eq!(schema["properties"]["id"]["format"], "uuid");
}

#[test]
fn vec_of_uuid_schema() {
    let schema = with_uuids_json_schema();
    let related = &schema["properties"]["related_ids"];
    assert_eq!(related["type"], "array");
    assert_eq!(related["items"]["type"], "string");
    assert_eq!(related["items"]["format"], "uuid");
}

#[test]
fn all_types_correct_mapping() {
    let schema = all_types_json_schema();
    assert_eq!(schema["properties"]["name"]["type"], "string");
    assert_eq!(schema["properties"]["count"]["type"], "integer");
    assert_eq!(schema["properties"]["big_count"]["type"], "integer");
    assert_eq!(schema["properties"]["small_count"]["type"], "integer");
    assert_eq!(schema["properties"]["price"]["type"], "number");
    assert_eq!(schema["properties"]["active"]["type"], "boolean");
    assert_eq!(schema["properties"]["id"]["type"], "string");
    assert_eq!(schema["properties"]["id"]["format"], "uuid");
    assert_eq!(schema["properties"]["tags"]["type"], "array");
    // note is Option<String>, should still have string type
    assert_eq!(schema["properties"]["note"]["anyOf"][0]["type"], "string");
}

#[test]
fn all_types_required_excludes_optional() {
    let schema = all_types_json_schema();
    let required = schema["required"].as_array().expect("required array");
    let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_names.contains(&"name"));
    assert!(required_names.contains(&"count"));
    assert!(required_names.contains(&"active"));
    assert!(!required_names.contains(&"note"), "note is Optional, should not be required");
}

#[test]
fn empty_schema_has_no_properties() {
    let schema = empty_schema_json_schema();
    assert_eq!(schema["type"], "object");
    let props = schema["properties"].as_object().expect("properties");
    assert!(props.is_empty(), "empty struct should have no properties");
    let required = schema["required"].as_array().expect("required");
    assert!(required.is_empty(), "empty struct should have no required fields");
}

#[test]
fn all_optional_schema_has_empty_required() {
    let schema = all_optional_json_schema();
    let required = schema["required"].as_array().expect("required");
    assert!(required.is_empty(), "all optional fields means empty required array");
    let props = schema["properties"].as_object().expect("properties");
    assert_eq!(props.len(), 2, "should have 2 properties");
}

#[test]
fn schema_is_valid_json() {
    let schema = create_order_json_schema();
    // Re-serialize to ensure it's valid JSON
    let json_str = serde_json::to_string_pretty(&schema).expect("should serialize to valid JSON");
    let _reparsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("should re-parse as valid JSON");
}

#[derive(Debug, JsonSchema, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireContract {
    pub first_name: String,
    #[serde(rename = "display-name")]
    pub display_name: String,
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_note: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip)]
    pub internal_only: String,
    pub r#type: String,
}

#[test]
fn schema_preserves_serialized_names_nullability_and_optional_fields() {
    let value = WireContract {
        first_name: "Ada".into(),
        display_name: "A".into(),
        note: None,
        extra_note: None,
        enabled: false,
        internal_only: "secret".into(),
        r#type: "buyer".into(),
    };
    let wire = serde_json::to_value(value).unwrap();
    let schema = wire_contract_json_schema();
    let properties = schema["properties"].as_object().unwrap();
    for name in wire.as_object().unwrap().keys() {
        assert!(properties.contains_key(name), "wire field {name} is missing from schema");
    }
    assert!(!properties.contains_key("first_name"));
    assert!(!properties.contains_key("internalOnly"));
    assert!(!properties.contains_key("r#type"));
    assert_eq!(wire["note"], serde_json::Value::Null);
    assert_eq!(properties["note"]["anyOf"][1]["type"], "null");
    assert_eq!(schema["additionalProperties"], false);
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("firstName")));
    assert!(required.contains(&serde_json::json!("display-name")));
    assert!(!required.contains(&serde_json::json!("enabled")));
    assert!(!required.contains(&serde_json::json!("extraNote")));
}

#[derive(Debug, JsonSchema, serde::Serialize)]
pub struct NestedWire {
    pub name: String,
}

#[derive(Debug, JsonSchema, serde::Serialize)]
pub struct WithNestedWire {
    #[schema(with = "nested_wire_json_schema")]
    pub nested: NestedWire,
}

#[test]
fn explicit_nested_schema_preserves_object_shape() {
    let value = WithNestedWire { nested: NestedWire { name: "nested".into() } };
    let wire = serde_json::to_value(value).unwrap();
    let schema = with_nested_wire_json_schema();
    assert!(wire["nested"].is_object());
    assert_eq!(schema["properties"]["nested"]["type"], "object");
    assert_eq!(schema["properties"]["nested"]["properties"]["name"]["type"], "string");
}
