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
use stateset_core::models::fraud::{CreateFraudRule, FraudDecision, FraudSignalType};
use stateset_core::models::gift_card::CreateGiftCard;
use stateset_core::models::inventory::CreateInventoryItem;
use stateset_core::models::loyalty::{CreateLoyaltyProgram, LoyaltyTier};
use stateset_core::models::order::{Address, CreateOrder, CreateOrderItem};
use stateset_core::models::product::{CreateProduct, CreateProductVariant};
use stateset_core::models::review::CreateReview;
use stateset_core::models::segment::{CreateSegment, SegmentType};
use stateset_core::models::shipping_zone::CreateShippingZone;
use stateset_core::models::store_credit::{CreateStoreCredit, StoreCreditReason};
use stateset_core::models::wishlist::CreateWishlist;
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

// ============================================================================
// Gift Card Fixtures
// ============================================================================

/// Create a [`CreateGiftCard`] with a $100 USD balance.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_gift_card_input();
/// assert_eq!(input.currency, "USD");
/// ```
pub fn create_gift_card_input() -> CreateGiftCard {
    CreateGiftCard {
        code: None,
        initial_balance: dec!(100.00),
        currency: "USD".to_string(),
        recipient_email: Some("recipient@example.com".to_string()),
        sender_name: Some("Test Sender".to_string()),
        message: Some("Happy Birthday!".to_string()),
        expires_at: None,
    }
}

// ============================================================================
// Store Credit Fixtures
// ============================================================================

/// Create a [`CreateStoreCredit`] for the given customer with a $50 USD balance.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_store_credit_input(stateset_core::CustomerId::new());
/// assert_eq!(input.currency, "USD");
/// ```
pub fn create_store_credit_input(customer_id: CustomerId) -> CreateStoreCredit {
    CreateStoreCredit {
        customer_id,
        amount: dec!(50.00),
        currency: "USD".to_string(),
        reason: StoreCreditReason::Return,
        reference_id: Some("RET-001".to_string()),
        note: Some("Refund for returned item".to_string()),
        expires_at: None,
    }
}

// ============================================================================
// Review Fixtures
// ============================================================================

/// Create a [`CreateReview`] for the given product and customer.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_review_input(
///     stateset_core::ProductId::new(),
///     stateset_core::CustomerId::new(),
/// );
/// assert_eq!(input.rating, 5);
/// ```
pub fn create_review_input(product_id: ProductId, customer_id: CustomerId) -> CreateReview {
    CreateReview {
        product_id,
        customer_id,
        rating: 5,
        title: Some("Great product!".to_string()),
        body: Some("Really happy with my purchase. Would buy again.".to_string()),
        verified_purchase: true,
    }
}

// ============================================================================
// Segment Fixtures
// ============================================================================

/// Create a static [`CreateSegment`] with no rules.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_segment_input();
/// assert_eq!(input.name, "Test Segment");
/// ```
pub fn create_segment_input() -> CreateSegment {
    CreateSegment {
        name: "Test Segment".to_string(),
        description: Some("A segment for testing".to_string()),
        segment_type: SegmentType::Static,
        rules: vec![],
    }
}

// ============================================================================
// Shipping Zone Fixtures
// ============================================================================

/// Create a [`CreateShippingZone`] covering the United States.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_shipping_zone_input();
/// assert!(input.countries.contains(&"US".to_string()));
/// ```
pub fn create_shipping_zone_input() -> CreateShippingZone {
    CreateShippingZone {
        name: "Domestic".to_string(),
        countries: vec!["US".to_string()],
        regions: vec![],
        postal_codes: vec![],
        priority: Some(0),
    }
}

// ============================================================================
// Wishlist Fixtures
// ============================================================================

/// Create a private [`CreateWishlist`] for the given customer.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_wishlist_input(stateset_core::CustomerId::new());
/// assert_eq!(input.name, "My Wishlist");
/// assert!(!input.is_public);
/// ```
pub fn create_wishlist_input(customer_id: CustomerId) -> CreateWishlist {
    CreateWishlist {
        customer_id,
        name: "My Wishlist".to_string(),
        is_public: false,
    }
}

// ============================================================================
// Loyalty Program Fixtures
// ============================================================================

/// Create a [`CreateLoyaltyProgram`] with Bronze/Silver/Gold tiers.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_loyalty_program_input();
/// assert_eq!(input.points_per_dollar, 1);
/// assert_eq!(input.tiers.len(), 3);
/// ```
pub fn create_loyalty_program_input() -> CreateLoyaltyProgram {
    CreateLoyaltyProgram {
        name: "Test Loyalty Program".to_string(),
        description: Some("Earn points on every purchase".to_string()),
        points_per_dollar: 1,
        tiers: vec![
            LoyaltyTier {
                name: "Bronze".to_string(),
                min_points: 0,
                multiplier: 1.0,
                perks: vec!["Early access to sales".to_string()],
            },
            LoyaltyTier {
                name: "Silver".to_string(),
                min_points: 500,
                multiplier: 1.5,
                perks: vec!["Free standard shipping".to_string()],
            },
            LoyaltyTier {
                name: "Gold".to_string(),
                min_points: 2000,
                multiplier: 2.0,
                perks: vec!["Free express shipping".to_string(), "Dedicated support".to_string()],
            },
        ],
    }
}

// ============================================================================
// Fraud Rule Fixtures
// ============================================================================

/// Create a [`CreateFraudRule`] that flags high-value first orders for review.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_fraud_rule_input();
/// assert_eq!(input.threshold, 0.8);
/// ```
pub fn create_fraud_rule_input() -> CreateFraudRule {
    CreateFraudRule {
        name: "High Value First Order Review".to_string(),
        description: Some("Flag unusually high first orders for manual review".to_string()),
        signal_type: FraudSignalType::HighValueFirstOrder,
        threshold: 0.8,
        action: FraudDecision::Review,
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

    #[test]
    fn gift_card_input_is_valid() {
        let gc = create_gift_card_input();
        assert_eq!(gc.currency, "USD");
        assert_eq!(gc.initial_balance, dec!(100.00));
        assert_eq!(gc.recipient_email.as_deref(), Some("recipient@example.com"));
        assert!(gc.code.is_none(), "code should be auto-generated");
    }

    #[test]
    fn store_credit_input_is_valid() {
        let customer_id = CustomerId::new();
        let sc = create_store_credit_input(customer_id);
        assert_eq!(sc.customer_id, customer_id);
        assert_eq!(sc.currency, "USD");
        assert_eq!(sc.amount, dec!(50.00));
    }

    #[test]
    fn review_input_is_valid() {
        let product_id = ProductId::new();
        let customer_id = CustomerId::new();
        let review = create_review_input(product_id, customer_id);
        assert_eq!(review.product_id, product_id);
        assert_eq!(review.customer_id, customer_id);
        assert_eq!(review.rating, 5);
        assert!(review.verified_purchase);
    }

    #[test]
    fn segment_input_is_valid() {
        use stateset_core::models::segment::SegmentType;
        let segment = create_segment_input();
        assert_eq!(segment.name, "Test Segment");
        assert_eq!(segment.segment_type, SegmentType::Static);
        assert!(segment.rules.is_empty());
    }

    #[test]
    fn shipping_zone_input_is_valid() {
        let zone = create_shipping_zone_input();
        assert_eq!(zone.name, "Domestic");
        assert!(zone.countries.contains(&"US".to_string()));
        assert_eq!(zone.priority, Some(0));
    }

    #[test]
    fn wishlist_input_is_valid() {
        let customer_id = CustomerId::new();
        let wl = create_wishlist_input(customer_id);
        assert_eq!(wl.customer_id, customer_id);
        assert_eq!(wl.name, "My Wishlist");
        assert!(!wl.is_public);
    }

    #[test]
    fn loyalty_program_input_is_valid() {
        let program = create_loyalty_program_input();
        assert_eq!(program.name, "Test Loyalty Program");
        assert_eq!(program.points_per_dollar, 1);
        assert_eq!(program.tiers.len(), 3);
        assert_eq!(program.tiers[0].name, "Bronze");
        assert_eq!(program.tiers[1].name, "Silver");
        assert_eq!(program.tiers[2].name, "Gold");
        assert_eq!(program.tiers[2].multiplier, 2.0);
    }

    #[test]
    fn fraud_rule_input_is_valid() {
        use stateset_core::models::fraud::{FraudDecision, FraudSignalType};
        let rule = create_fraud_rule_input();
        assert_eq!(rule.name, "High Value First Order Review");
        assert_eq!(rule.signal_type, FraudSignalType::HighValueFirstOrder);
        assert!((rule.threshold - 0.8).abs() < f64::EPSILON);
        assert_eq!(rule.action, FraudDecision::Review);
    }
}
