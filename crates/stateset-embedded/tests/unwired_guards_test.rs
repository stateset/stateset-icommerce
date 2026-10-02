#![cfg(feature = "sqlite")]

//! Guard predicates in `stateset-core` that no write path consulted.
//!
//! Each test does the forbidden thing through the public API and expects the
//! write to be refused (and nothing to be written).
//!
//! - `Channel::is_mutation_blocked`: `api_locked` is documented to reject
//!   `update`, `delete` AND product sync, but `sync_products` never checked
//!   it, so a locked channel's SKU mappings could be rewritten or deleted.
//! - `Channel::can_ingest`: purgatory ingest accepted a `channel_id` naming a
//!   fulfillment-only channel (or no channel at all).

use rust_decimal_macros::dec;
use stateset_core::{
    ChannelId, ChannelProductSyncItem, ChannelType, CommerceError, CreateChannel, IngestLineItem,
    IngestOrder, ProductId, PurgatoryFilter,
};
use stateset_embedded::Commerce;

fn channel(commerce: &Commerce, channel_type: ChannelType) -> stateset_core::Channel {
    commerce
        .channels()
        .create(CreateChannel {
            name: format!("chan-{}", uuid::Uuid::new_v4().simple()),
            channel_type,
            integration: Some("shopify".into()),
            default_warehouse_id: None,
            tags: vec![],
            metadata: serde_json::Value::Null,
        })
        .expect("create channel")
}

fn upsert(sku: &str) -> ChannelProductSyncItem {
    ChannelProductSyncItem {
        channel_sku: sku.into(),
        product_id: Some(ProductId::new()),
        internal_sku: Some(format!("INT-{sku}")),
        delete: false,
    }
}

#[test]
fn locked_channel_rejects_product_sync() {
    let commerce = Commerce::in_memory().unwrap();
    let ch = channel(&commerce, ChannelType::SalesChannel);
    assert_eq!(commerce.channels().sync_products(ch.id, vec![upsert("A")]).unwrap(), 1);

    commerce.channels().set_lock(ch.id, true).unwrap();

    let err = commerce
        .channels()
        .sync_products(ch.id, vec![upsert("B")])
        .expect_err("a locked channel must refuse an upsert sync");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let delete = ChannelProductSyncItem {
        channel_sku: "A".into(),
        product_id: None,
        internal_sku: None,
        delete: true,
    };
    let err = commerce
        .channels()
        .sync_products(ch.id, vec![delete])
        .expect_err("a locked channel must refuse a delete sync");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let mappings = commerce.channels().list_product_mappings(ch.id).unwrap();
    assert_eq!(mappings.len(), 1, "the locked channel's mappings are untouched");
    assert_eq!(mappings[0].channel_sku, "A");

    // Unlocking restores the write path.
    commerce.channels().set_lock(ch.id, false).unwrap();
    assert_eq!(commerce.channels().sync_products(ch.id, vec![upsert("B")]).unwrap(), 1);
}

#[test]
fn product_sync_on_unknown_channel_is_not_found() {
    let commerce = Commerce::in_memory().unwrap();
    let err = commerce
        .channels()
        .sync_products(ChannelId::new(), vec![upsert("A")])
        .expect_err("no channel, no mappings");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

fn ingest(channel_id: Option<ChannelId>) -> IngestOrder {
    IngestOrder {
        channel_id,
        external_order_id: format!("EXT-{}", uuid::Uuid::new_v4().simple()),
        external_status: Some("paid".into()),
        metadata: serde_json::Value::Null,
        items: vec![IngestLineItem {
            external_sku: "X".into(),
            quantity: dec!(1),
            product_id: None,
        }],
    }
}

#[test]
fn purgatory_ingest_requires_an_ingesting_channel() {
    let commerce = Commerce::in_memory().unwrap();

    let fulfillment = channel(&commerce, ChannelType::FulfillmentChannel);
    let err = commerce
        .purgatory()
        .ingest(ingest(Some(fulfillment.id)))
        .expect_err("a fulfillment-only channel cannot ingest orders");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let err = commerce
        .purgatory()
        .ingest(ingest(Some(ChannelId::new())))
        .expect_err("an unknown channel cannot ingest orders");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    assert!(
        commerce.purgatory().list(PurgatoryFilter::default()).unwrap().is_empty(),
        "refused ingests write nothing"
    );

    // Sales, end-to-end and channel-less ingests still work.
    let sales = channel(&commerce, ChannelType::SalesChannel);
    let e2e = channel(&commerce, ChannelType::EndToEndChannel);
    commerce.purgatory().ingest(ingest(Some(sales.id))).expect("sales channel ingests");
    commerce.purgatory().ingest(ingest(Some(e2e.id))).expect("end-to-end channel ingests");
    commerce.purgatory().ingest(ingest(None)).expect("channel-less ingest");
}
