//! Postgres twin of `unwired_guards_test.rs`: `Channel::is_mutation_blocked`
//! on product sync and `Channel::can_ingest` on purgatory ingest.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    ChannelId, ChannelProductSyncItem, ChannelType, CommerceError, CreateChannel, IngestLineItem,
    IngestOrder, ProductId, PurgatoryFilter,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

macro_rules! require_pg {
    ($name:literal) => {
        match postgres_url() {
            Some(url) => AsyncCommerce::connect(&url).await.expect("connect + migrate"),
            None => {
                eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping {}", $name);
                return;
            }
        }
    };
}

fn create(channel_type: ChannelType) -> CreateChannel {
    CreateChannel {
        name: format!("chan-{}", uuid::Uuid::new_v4().simple()),
        channel_type,
        integration: Some("shopify".into()),
        default_warehouse_id: None,
        tags: vec![],
        metadata: serde_json::Value::Null,
    }
}

fn upsert(sku: &str) -> ChannelProductSyncItem {
    ChannelProductSyncItem {
        channel_sku: sku.into(),
        product_id: Some(ProductId::new()),
        internal_sku: Some(format!("INT-{sku}")),
        delete: false,
    }
}

#[tokio::test]
async fn postgres_locked_channel_rejects_product_sync() {
    let commerce = require_pg!("locked channel product sync");
    let store = commerce.database().channels();
    let ch = store.create_async(create(ChannelType::SalesChannel)).await.unwrap();
    assert_eq!(store.sync_products_async(ch.id, vec![upsert("A")]).await.unwrap(), 1);

    store.set_lock_async(ch.id, true).await.unwrap();
    let err = store
        .sync_products_async(ch.id, vec![upsert("B")])
        .await
        .expect_err("a locked channel must refuse an upsert sync");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let delete = ChannelProductSyncItem {
        channel_sku: "A".into(),
        product_id: None,
        internal_sku: None,
        delete: true,
    };
    let err = store
        .sync_products_async(ch.id, vec![delete])
        .await
        .expect_err("a locked channel must refuse a delete sync");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let mappings = store.list_product_mappings_async(ch.id).await.unwrap();
    assert_eq!(mappings.len(), 1, "the locked channel's mappings are untouched");

    store.set_lock_async(ch.id, false).await.unwrap();
    assert_eq!(store.sync_products_async(ch.id, vec![upsert("B")]).await.unwrap(), 1);

    let err = store
        .sync_products_async(ChannelId::new(), vec![upsert("A")])
        .await
        .expect_err("no channel, no mappings");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

fn ingest(channel_id: Option<ChannelId>, marker: &str) -> IngestOrder {
    IngestOrder {
        channel_id,
        external_order_id: format!("EXT-{marker}-{}", uuid::Uuid::new_v4().simple()),
        external_status: Some("paid".into()),
        metadata: serde_json::Value::Null,
        items: vec![IngestLineItem {
            external_sku: "X".into(),
            quantity: dec!(1),
            product_id: None,
        }],
    }
}

#[tokio::test]
async fn postgres_purgatory_ingest_requires_an_ingesting_channel() {
    let commerce = require_pg!("purgatory ingest channel guard");
    let channels = commerce.database().channels();
    let purgatory = commerce.database().purgatory();

    let fulfillment = channels.create_async(create(ChannelType::FulfillmentChannel)).await.unwrap();
    let err = purgatory
        .ingest_async(ingest(Some(fulfillment.id), "ful"))
        .await
        .expect_err("a fulfillment-only channel cannot ingest orders");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let unknown = ChannelId::new();
    let err = purgatory
        .ingest_async(ingest(Some(unknown), "unk"))
        .await
        .expect_err("an unknown channel cannot ingest orders");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    for id in [fulfillment.id, unknown] {
        let rows = purgatory
            .list_async(PurgatoryFilter { channel_id: Some(id), ..Default::default() })
            .await
            .unwrap();
        assert!(rows.is_empty(), "refused ingests write nothing");
    }

    let sales = channels.create_async(create(ChannelType::SalesChannel)).await.unwrap();
    let e2e = channels.create_async(create(ChannelType::EndToEndChannel)).await.unwrap();
    purgatory.ingest_async(ingest(Some(sales.id), "sales")).await.expect("sales ingests");
    purgatory.ingest_async(ingest(Some(e2e.id), "e2e")).await.expect("end-to-end ingests");
    purgatory.ingest_async(ingest(None, "none")).await.expect("channel-less ingest");
}
