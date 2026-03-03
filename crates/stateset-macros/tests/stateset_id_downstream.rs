use std::str::FromStr;

use stateset_macros::StateSetId;

#[derive(StateSetId)]
pub struct DownstreamInvoiceId(uuid::Uuid);

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
