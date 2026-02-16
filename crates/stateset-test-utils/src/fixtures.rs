//! Reusable test fixture builders for domain objects.
//!
//! These builders produce valid [`stateset_core`] domain inputs with sensible
//! defaults. Each function returns a *create input* struct ready to pass to
//! a repository or the `Commerce` facade.
//!
//! All generated data is deterministic where possible (fixed strings) but uses
//! `Uuid::new_v4()` for identifiers so fixtures never collide in a test database.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::CustomerId;
use uuid::Uuid;

use stateset_core::models::customer::{CreateCustomer, CreateCustomerAddress};
use stateset_core::models::inventory::CreateInventoryItem;
use stateset_core::models::order::{Address, CreateOrder, CreateOrderItem};
use stateset_core::models::product::{CreateProduct, CreateProductVariant};
use stateset_core::ProductId;

// ============================================================================
// Customer Fixtures
// ============================================================================

/// Create a [`CreateCustomer`] input with a unique email.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_customer_input();
/// assert!(input.email.contains("@example.com"));
/// ```
pub fn create_customer_input() -> CreateCustomer {
    CreateCustomer {
        email: format!("test-{}@example.com", Uuid::new_v4()),
        first_name: "Test".into(),
        last_name: "User".into(),
        phone: Some("+1-555-0100".into()),
        accepts_marketing: Some(false),
        tags: None,
        metadata: None,
    }
}

/// Create a [`CreateCustomer`] with a specific email.
pub fn create_customer_with_email(email: impl Into<String>) -> CreateCustomer {
    CreateCustomer {
        email: email.into(),
        ..create_customer_input()
    }
}

/// Create a [`CreateCustomerAddress`] for a given customer.
pub fn create_address_input(customer_id: CustomerId) -> CreateCustomerAddress {
    CreateCustomerAddress {
        customer_id,
        address_type: None,
        first_name: "Test".into(),
        last_name: "User".into(),
        company: None,
        line1: "123 Main St".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: Some("+1-555-0100".into()),
        is_default: Some(true),
    }
}

// ============================================================================
// Order Fixtures
// ============================================================================

/// Create a shipping/billing [`Address`].
pub fn test_address() -> Address {
    Address {
        line1: "123 Main St".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
    }
}

/// Create a [`CreateOrderItem`] with sensible defaults.
///
/// ```rust
/// let item = stateset_test_utils::fixtures::create_order_item_input();
/// assert_eq!(item.quantity, 2);
/// ```
pub fn create_order_item_input() -> CreateOrderItem {
    CreateOrderItem {
        product_id: ProductId::new(),
        variant_id: None,
        sku: "TEST-SKU-001".into(),
        name: "Test Product".into(),
        quantity: 2,
        unit_price: dec!(29.99),
        discount: None,
        tax_amount: None,
    }
}

/// Create a [`CreateOrderItem`] with a specific SKU and price.
pub fn create_order_item_with(sku: &str, quantity: i32, unit_price: Decimal) -> CreateOrderItem {
    CreateOrderItem {
        product_id: ProductId::new(),
        variant_id: None,
        sku: sku.into(),
        name: format!("Product {sku}"),
        quantity,
        unit_price,
        discount: None,
        tax_amount: None,
    }
}

/// Create a [`CreateOrder`] with one default item for the given customer.
///
/// ```rust
/// let order = stateset_test_utils::fixtures::create_order_input(stateset_core::CustomerId::new());
/// assert_eq!(order.items.len(), 1);
/// assert_eq!(order.currency, Some("USD".to_string()));
/// ```
pub fn create_order_input(customer_id: CustomerId) -> CreateOrder {
    CreateOrder {
        customer_id,
        items: vec![create_order_item_input()],
        currency: Some("USD".into()),
        shipping_address: Some(test_address()),
        billing_address: None,
        notes: None,
        payment_method: None,
        shipping_method: None,
    }
}

/// Create a [`CreateOrder`] with specific items.
pub fn create_order_with_items(customer_id: CustomerId, items: Vec<CreateOrderItem>) -> CreateOrder {
    CreateOrder {
        customer_id,
        items,
        currency: Some("USD".into()),
        shipping_address: Some(test_address()),
        billing_address: None,
        notes: None,
        payment_method: None,
        shipping_method: None,
    }
}

// ============================================================================
// Product Fixtures
// ============================================================================

/// Create a [`CreateProduct`] with one default variant.
///
/// ```rust
/// let product = stateset_test_utils::fixtures::create_product_input();
/// assert_eq!(product.name, "Test Product");
/// ```
pub fn create_product_input() -> CreateProduct {
    CreateProduct {
        name: "Test Product".into(),
        slug: Some("test-product".into()),
        description: Some("A product for testing".into()),
        product_type: None,
        attributes: None,
        seo: None,
        variants: Some(vec![CreateProductVariant {
            sku: format!("SKU-{}", Uuid::new_v4().as_simple()),
            name: Some("Default".into()),
            price: dec!(29.99),
            compare_at_price: None,
            cost: Some(dec!(10.00)),
            barcode: None,
            weight: None,
            weight_unit: None,
            options: None,
            is_default: Some(true),
        }]),
    }
}

/// Create a [`CreateProduct`] with a specific name.
pub fn create_product_with_name(name: &str) -> CreateProduct {
    CreateProduct {
        name: name.into(),
        slug: Some(name.to_lowercase().replace(' ', "-")),
        ..create_product_input()
    }
}

// ============================================================================
// Inventory Fixtures
// ============================================================================

/// Create a [`CreateInventoryItem`] with a unique SKU.
///
/// ```rust
/// let item = stateset_test_utils::fixtures::create_inventory_input();
/// assert!(item.sku.starts_with("INV-"));
/// ```
pub fn create_inventory_input() -> CreateInventoryItem {
    CreateInventoryItem {
        sku: format!("INV-{}", Uuid::new_v4().as_simple()),
        name: "Test Inventory Item".into(),
        description: Some("Test item for unit tests".into()),
        unit_of_measure: Some("each".into()),
        initial_quantity: Some(dec!(100)),
        location_id: None,
        reorder_point: Some(dec!(10)),
        safety_stock: Some(dec!(5)),
    }
}

/// Create a [`CreateInventoryItem`] with a specific SKU and quantity.
pub fn create_inventory_with(sku: &str, quantity: Decimal) -> CreateInventoryItem {
    CreateInventoryItem {
        sku: sku.into(),
        name: format!("Item {sku}"),
        description: None,
        unit_of_measure: Some("each".into()),
        initial_quantity: Some(quantity),
        location_id: None,
        reorder_point: Some(dec!(10)),
        safety_stock: Some(dec!(5)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_input_has_unique_email() {
        let a = create_customer_input();
        let b = create_customer_input();
        assert_ne!(a.email, b.email);
        assert!(a.email.contains("@example.com"));
    }

    #[test]
    fn order_input_has_items() {
        let order = create_order_input(CustomerId::new());
        assert_eq!(order.items.len(), 1);
        assert_eq!(order.items[0].sku, "TEST-SKU-001");
        assert_eq!(order.items[0].quantity, 2);
    }

    #[test]
    fn product_input_has_variant() {
        let product = create_product_input();
        let variants = product.variants.unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].price, dec!(29.99));
    }

    #[test]
    fn inventory_input_has_quantity() {
        let inv = create_inventory_input();
        assert_eq!(inv.initial_quantity, Some(dec!(100)));
        assert!(inv.sku.starts_with("INV-"));
    }

    #[test]
    fn address_input_complete() {
        let addr = create_address_input(CustomerId::new());
        assert_eq!(addr.city, "San Francisco");
        assert_eq!(addr.country, "US");
    }

    #[test]
    fn custom_order_items() {
        let items = vec![
            create_order_item_with("SKU-A", 1, dec!(10.00)),
            create_order_item_with("SKU-B", 3, dec!(5.50)),
        ];
        let order = create_order_with_items(CustomerId::new(), items);
        assert_eq!(order.items.len(), 2);
        assert_eq!(order.items[1].sku, "SKU-B");
    }
}
