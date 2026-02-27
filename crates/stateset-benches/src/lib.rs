#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

//! Shared benchmark helpers for StateSet iCommerce.
//!
//! Provides fixture generators and convenience functions used across all
//! criterion benchmark suites in this crate, plus optional perf-gate checks
//! (`STATESET_PERF_GATE=1`) for CI threshold enforcement.

pub mod perf_gate;

// Dependencies used only in bench binaries — suppress unused_crate_dependencies.
use chrono as _;
use criterion as _;
use rust_decimal as _;
use stateset_crypto as _;
use stateset_db as _;
use stateset_primitives as _;
use tokio as _;

use rust_decimal_macros::dec;
use serde_json::json;
use stateset_core::models::customer::CreateCustomer;
use stateset_core::models::order::{Address, CreateOrder, CreateOrderItem};
use stateset_core::{CustomerId, ProductId};
use stateset_embedded::Commerce;
use uuid::Uuid;

/// Generate `n` [`CreateOrder`] inputs with unique IDs and a single line item each.
///
/// Each order targets a distinct (random) customer and contains one item
/// with a deterministic SKU derived from its index.
#[must_use]
pub fn create_test_orders(n: usize) -> Vec<CreateOrder> {
    (0..n)
        .map(|i| CreateOrder {
            customer_id: CustomerId::new(),
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                variant_id: None,
                sku: format!("BENCH-SKU-{i:06}"),
                name: format!("Bench Product {i}"),
                quantity: 2,
                unit_price: dec!(29.99),
                discount: None,
                tax_amount: None,
            }],
            currency: Some("USD".into()),
            shipping_address: Some(Address {
                line1: "123 Bench St".into(),
                line2: None,
                city: "San Francisco".into(),
                state: Some("CA".into()),
                postal_code: "94102".into(),
                country: "US".into(),
            }),
            billing_address: None,
            notes: None,
            payment_method: None,
            shipping_method: None,
        })
        .collect()
}

/// Generate `n` [`CreateCustomer`] inputs with unique emails.
#[must_use]
pub fn create_test_customers(n: usize) -> Vec<CreateCustomer> {
    (0..n)
        .map(|i| CreateCustomer {
            email: format!("bench-{}-{}@example.com", i, Uuid::new_v4()),
            first_name: "Bench".into(),
            last_name: format!("User{i}"),
            phone: Some("+1-555-0100".into()),
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .collect()
}

/// Generate `n` JSON payloads of varying complexity for JCS benchmarking.
///
/// - Index `i % 3 == 0`: small payload (5 fields)
/// - Index `i % 3 == 1`: medium payload (nested object, ~20 fields)
/// - Index `i % 3 == 2`: large payload (deeply nested, 100+ fields)
#[must_use]
pub fn create_test_json_payloads(n: usize) -> Vec<serde_json::Value> {
    (0..n)
        .map(|i| match i % 3 {
            0 => json!({
                "id": Uuid::new_v4().to_string(),
                "name": format!("item-{i}"),
                "price": 29.99,
                "currency": "USD",
                "active": true,
            }),
            1 => {
                let mut obj = serde_json::Map::new();
                for j in 0..20 {
                    obj.insert(
                        format!("field_{j:02}"),
                        json!({
                            "value": j,
                            "label": format!("Label {j}"),
                            "nested": { "depth": 1, "index": j }
                        }),
                    );
                }
                serde_json::Value::Object(obj)
            }
            _ => {
                let mut obj = serde_json::Map::new();
                for j in 0..50 {
                    let mut inner = serde_json::Map::new();
                    for k in 0..3 {
                        inner.insert(
                            format!("sub_{k}"),
                            json!({
                                "id": format!("{i}-{j}-{k}"),
                                "tags": ["alpha", "beta", "gamma"],
                                "metadata": {
                                    "created": "2026-01-01T00:00:00Z",
                                    "version": k
                                }
                            }),
                        );
                    }
                    obj.insert(format!("section_{j:03}"), serde_json::Value::Object(inner));
                }
                serde_json::Value::Object(obj)
            }
        })
        .collect()
}

/// Create an in-memory [`Commerce`] instance suitable for benchmarking.
///
/// Returns the `Commerce` handle together with the [`tempfile::TempDir`] that
/// backs it. The directory (and database) is cleaned up when the `TempDir` is
/// dropped.
///
/// # Panics
///
/// Panics if the Commerce instance cannot be created.
#[must_use]
pub fn create_temp_commerce() -> (Commerce, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("bench.db");
    let commerce = Commerce::new(db_path.to_str().expect("non-UTF-8 temp path"))
        .expect("failed to create Commerce instance");
    (commerce, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_orders() {
        let orders = create_test_orders(5);
        assert_eq!(orders.len(), 5);
        for (i, order) in orders.iter().enumerate() {
            assert_eq!(order.items.len(), 1);
            assert_eq!(order.items[0].sku, format!("BENCH-SKU-{i:06}"));
        }
    }

    #[test]
    fn test_create_test_customers() {
        let customers = create_test_customers(3);
        assert_eq!(customers.len(), 3);
        for c in &customers {
            assert!(c.email.contains("@example.com"));
        }
    }

    #[test]
    fn test_create_test_json_payloads() {
        let payloads = create_test_json_payloads(6);
        assert_eq!(payloads.len(), 6);
        // Check small payloads have ~5 fields
        assert!(payloads[0].as_object().unwrap().len() == 5);
        // Check medium payloads have 20 fields
        assert!(payloads[1].as_object().unwrap().len() == 20);
        // Check large payloads have 50 sections
        assert!(payloads[2].as_object().unwrap().len() == 50);
    }

    #[test]
    fn test_create_temp_commerce() {
        let (commerce, _dir) = create_temp_commerce();
        // Smoke test: check that we can access orders without panicking
        let _orders = commerce.orders();
    }
}
