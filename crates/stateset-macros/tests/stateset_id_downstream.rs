use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use stateset_macros::StateSetId;

#[derive(StateSetId)]
pub struct DownstreamInvoiceId(uuid::Uuid);

#[derive(StateSetId)]
pub struct OrderId(uuid::Uuid);

#[derive(StateSetId)]
pub struct CustomerId(uuid::Uuid);

#[derive(StateSetId)]
struct PrivateId(uuid::Uuid);

const fn assert_id_traits<T: Copy + Eq + Ord + std::hash::Hash + serde::Serialize>() {}

#[test]
fn downstream_stateset_id_behaves_like_uuid_newtype() {
    assert_id_traits::<DownstreamInvoiceId>();

    let id = DownstreamInvoiceId::new();
    let as_uuid = id.as_uuid();
    assert_eq!(id.into_uuid(), *as_uuid);

    let parsed = DownstreamInvoiceId::from_str(&id.to_string()).expect("id should parse");
    assert_eq!(parsed, id);

    let encoded = serde_json::to_string(&id).expect("serialize");
    let decoded: DownstreamInvoiceId = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(decoded, id);
}

#[test]
fn multiple_id_types_are_distinct() {
    // Ensure different ID types are distinct types (no cross-assignment)
    let order_id = OrderId::new();
    let customer_id = CustomerId::new();

    // They should have different UUIDs (with extremely high probability)
    assert_ne!(order_id.into_uuid(), customer_id.into_uuid());
}

#[test]
fn nil_id_is_all_zeros() {
    let nil = DownstreamInvoiceId::nil();
    assert!(nil.is_nil());
    assert_eq!(nil.to_string(), "00000000-0000-0000-0000-000000000000");
}

#[test]
fn from_uuid_roundtrip() {
    let raw = uuid::Uuid::new_v4();
    let id = DownstreamInvoiceId::from(raw);
    let back: uuid::Uuid = id.into();
    assert_eq!(raw, back);
}

#[test]
fn from_uuid_method_roundtrip() {
    let raw = uuid::Uuid::new_v4();
    let id = DownstreamInvoiceId::from_uuid(raw);
    assert_eq!(*id.as_uuid(), raw);
    assert_eq!(id.into_uuid(), raw);
}

#[test]
fn as_ref_returns_inner_uuid() {
    let id = OrderId::new();
    let uuid_ref: &uuid::Uuid = id.as_ref();
    assert_eq!(*uuid_ref, id.into_uuid());
}

#[test]
fn display_format_is_uuid_string() {
    let raw = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("parse");
    let id = OrderId::from(raw);
    assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
}

#[test]
fn debug_format_includes_type_name() {
    let raw = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("parse");
    let id = OrderId::from(raw);
    let debug_str = format!("{:?}", id);
    assert!(debug_str.starts_with("OrderId("), "Debug should start with type name");
    assert!(debug_str.contains("550e8400"), "Debug should contain UUID");
}

#[test]
fn from_str_invalid_uuid_returns_error() {
    let result = OrderId::from_str("not-a-uuid");
    assert!(result.is_err(), "invalid UUID string should produce error");
}

#[test]
fn copy_semantics_work() {
    let id = CustomerId::new();
    let copy = id; // Copy
    let another = id; // still usable after copy
    assert_eq!(copy, another);
}

#[test]
fn hash_works_in_hashset() {
    let id1 = OrderId::new();
    let id2 = OrderId::new();
    let mut set = HashSet::new();
    set.insert(id1);
    set.insert(id2);
    set.insert(id1); // duplicate
    assert_eq!(set.len(), 2);
    assert!(set.contains(&id1));
    assert!(set.contains(&id2));
}

#[test]
fn hash_works_in_hashmap() {
    let id = CustomerId::new();
    let mut map = HashMap::new();
    map.insert(id, "Alice");
    assert_eq!(map.get(&id), Some(&"Alice"));
}

#[test]
fn ordering_is_consistent() {
    let id1 = OrderId::nil();
    let id2 = OrderId::from(uuid::Uuid::max());
    assert!(id1 < id2);
    assert!(id2 > id1);
    assert_eq!(id1.cmp(&id1), std::cmp::Ordering::Equal);
}

#[test]
fn default_generates_non_nil_id() {
    let id = OrderId::default();
    assert!(!id.is_nil(), "Default should generate a non-nil UUID via new()");
}

#[test]
fn serde_json_roundtrip_with_struct() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Wrapper {
        id: OrderId,
        name: String,
    }

    let w = Wrapper { id: OrderId::new(), name: "test".to_string() };

    let json = serde_json::to_string(&w).expect("serialize");
    let decoded: Wrapper = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.id, w.id);
    assert_eq!(decoded.name, w.name);
}

#[test]
fn private_id_type_works() {
    let id = PrivateId::new();
    assert!(!id.is_nil());
    let _ = id.to_string();
}
