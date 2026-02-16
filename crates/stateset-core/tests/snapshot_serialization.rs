//! Snapshot tests for JSON serialization of core domain models.
//!
//! These tests use [`insta`] to capture the JSON representation of key domain
//! objects. When the serialization format changes (field renames, new fields,
//! enum variant changes), these tests will fail and require an explicit
//! `cargo insta review` to accept the new snapshots.
//!
//! This guards against accidental API/serialization breaking changes.

use chrono::{TimeZone, Utc};
use insta::assert_json_snapshot;
use rust_decimal_macros::dec;
use uuid::Uuid;

use stateset_core::models::customer::CustomerStatus;
use stateset_core::models::inventory::TransactionType;
use stateset_core::models::order::{
    Address, CreateOrder, CreateOrderItem, FulfillmentStatus, Order, OrderItem, OrderStatus,
    PaymentStatus,
};
use stateset_core::models::product::ProductStatus;
use stateset_core::models::returns::ReturnStatus;
use stateset_core::{CustomerId, OrderId, OrderItemId, ProductId};

use stateset_test_utils::fixtures;

// Fixed UUIDs for deterministic snapshots.
const ORDER_ID: OrderId =
    OrderId::from_uuid(Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef));
const CUSTOMER_ID: CustomerId =
    CustomerId::from_uuid(Uuid::from_u128(0xfede_dcba_9876_5432_fede_dcba_9876_5432));
const PRODUCT_ID: ProductId =
    ProductId::from_uuid(Uuid::from_u128(0xaaaa_bbbb_cccc_dddd_eeee_ffff_0000_1111));
const ITEM_ID: OrderItemId =
    OrderItemId::from_uuid(Uuid::from_u128(0x1111_2222_3333_4444_5555_6666_7777_8888));

fn fixed_timestamp() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()
}

#[test]
fn snapshot_order_status_variants() {
    let statuses = vec![
        OrderStatus::Pending,
        OrderStatus::Confirmed,
        OrderStatus::Processing,
        OrderStatus::Shipped,
        OrderStatus::Delivered,
        OrderStatus::Cancelled,
        OrderStatus::Refunded,
    ];
    assert_json_snapshot!("order_status_variants", statuses);
}

#[test]
fn snapshot_payment_status_variants() {
    let statuses = vec![
        PaymentStatus::Pending,
        PaymentStatus::Authorized,
        PaymentStatus::Paid,
        PaymentStatus::PartiallyPaid,
        PaymentStatus::Refunded,
        PaymentStatus::PartiallyRefunded,
        PaymentStatus::Failed,
    ];
    assert_json_snapshot!("payment_status_variants", statuses);
}

#[test]
fn snapshot_create_order_input() {
    let input = CreateOrder {
        customer_id: CUSTOMER_ID,
        items: vec![CreateOrderItem {
            product_id: PRODUCT_ID,
            variant_id: None,
            sku: "TEST-SKU-001".into(),
            name: "Test Widget".into(),
            quantity: 2,
            unit_price: dec!(29.99),
            discount: Some(dec!(5.00)),
            tax_amount: Some(dec!(4.50)),
        }],
        currency: Some("USD".into()),
        shipping_address: Some(Address {
            line1: "123 Main St".into(),
            line2: Some("Apt 4B".into()),
            city: "San Francisco".into(),
            state: Some("CA".into()),
            postal_code: "94102".into(),
            country: "US".into(),
        }),
        billing_address: None,
        notes: Some("Please gift wrap".into()),
        payment_method: Some("credit_card".into()),
        shipping_method: Some("standard".into()),
    };
    assert_json_snapshot!("create_order_input", input);
}

#[test]
fn snapshot_order() {
    let ts = fixed_timestamp();
    let order = Order {
        id: ORDER_ID,
        order_number: "ORD-2025-001".into(),
        customer_id: CUSTOMER_ID,
        status: OrderStatus::Confirmed,
        order_date: ts,
        total_amount: dec!(65.47),
        currency: "USD".into(),
        payment_status: PaymentStatus::Paid,
        fulfillment_status: FulfillmentStatus::Unfulfilled,
        payment_method: Some("credit_card".into()),
        shipping_method: Some("standard".into()),
        tracking_number: None,
        notes: None,
        shipping_address: Some(fixtures::test_address()),
        billing_address: None,
        items: vec![OrderItem {
            id: ITEM_ID,
            order_id: ORDER_ID,
            product_id: PRODUCT_ID,
            variant_id: None,
            sku: "TEST-SKU-001".into(),
            name: "Test Widget".into(),
            quantity: 2,
            unit_price: dec!(29.99),
            discount: dec!(5.00),
            tax_amount: dec!(4.50),
            total: dec!(54.98),
        }],
        version: 1,
        created_at: ts,
        updated_at: ts,
    };
    assert_json_snapshot!("order", order);
}

#[test]
fn snapshot_customer_status_variants() {
    let statuses = vec![
        CustomerStatus::Active,
        CustomerStatus::Inactive,
        CustomerStatus::Suspended,
    ];
    assert_json_snapshot!("customer_status_variants", statuses);
}

#[test]
fn snapshot_return_status_variants() {
    let statuses = vec![
        ReturnStatus::Requested,
        ReturnStatus::Approved,
        ReturnStatus::Rejected,
        ReturnStatus::InTransit,
        ReturnStatus::Received,
        ReturnStatus::Inspecting,
        ReturnStatus::Completed,
        ReturnStatus::Cancelled,
    ];
    assert_json_snapshot!("return_status_variants", statuses);
}

#[test]
fn snapshot_product_status_variants() {
    let statuses = vec![
        ProductStatus::Draft,
        ProductStatus::Active,
        ProductStatus::Archived,
    ];
    assert_json_snapshot!("product_status_variants", statuses);
}

#[test]
fn snapshot_inventory_transaction_types() {
    let types = vec![
        TransactionType::Receipt,
        TransactionType::Shipment,
        TransactionType::Adjustment,
        TransactionType::Transfer,
        TransactionType::Return,
        TransactionType::Allocation,
        TransactionType::Deallocation,
        TransactionType::CycleCount,
    ];
    assert_json_snapshot!("inventory_transaction_types", types);
}
