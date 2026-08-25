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
use stateset_primitives::CurrencyCode;
use uuid::Uuid;

use stateset_core::models::cart::{AddCartItem, CreateCart};
use stateset_core::models::customer::{CreateCustomer, CreateCustomerAddress};
use stateset_core::models::fraud::{CreateFraudRule, FraudDecision, FraudSignalType};
use stateset_core::models::gift_card::CreateGiftCard;
use stateset_core::models::inventory::CreateInventoryItem;
use stateset_core::models::loyalty::{CreateLoyaltyProgram, LoyaltyTier};
use stateset_core::models::order::{Address, CreateOrder, CreateOrderItem};
use stateset_core::models::payment::{CreatePayment, PaymentMethodType};
use stateset_core::models::product::{CreateProduct, CreateProductVariant};
use stateset_core::models::returns::{CreateReturn, CreateReturnItem, ReturnReason};
use stateset_core::models::review::CreateReview;
use stateset_core::models::segment::{CreateSegment, SegmentType};
use stateset_core::models::shipment::CreateShipment;
use stateset_core::models::shipping_zone::CreateShippingZone;
use stateset_core::models::store_credit::{CreateStoreCredit, StoreCreditReason};
use stateset_core::models::subscription::{
    BillingInterval, CreateSubscription, CreateSubscriptionPlan,
};
use stateset_core::models::warranty::CreateWarranty;
use stateset_core::models::wishlist::CreateWishlist;
use stateset_core::{OrderId, OrderItemId, ProductId};

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
    CreateCustomer { email: email.into(), ..create_customer_input() }
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
/// assert_eq!(order.currency, Some(stateset_primitives::CurrencyCode::USD));
/// ```
pub fn create_order_input(customer_id: CustomerId) -> CreateOrder {
    CreateOrder {
        customer_id,
        items: vec![create_order_item_input()],
        currency: Some(CurrencyCode::USD),
        shipping_address: Some(test_address()),
        billing_address: None,
        notes: None,
        payment_method: None,
        shipping_method: None,
        stock_policy: stateset_core::StockPolicy::default(),
    }
}

/// Create a [`CreateOrder`] with specific items.
pub fn create_order_with_items(
    customer_id: CustomerId,
    items: Vec<CreateOrderItem>,
) -> CreateOrder {
    CreateOrder {
        customer_id,
        items,
        currency: Some(CurrencyCode::USD),
        shipping_address: Some(test_address()),
        billing_address: None,
        notes: None,
        payment_method: None,
        shipping_method: None,
        stock_policy: stateset_core::StockPolicy::default(),
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
/// assert_eq!(input.currency, stateset_primitives::CurrencyCode::USD);
/// ```
pub fn create_gift_card_input() -> CreateGiftCard {
    CreateGiftCard {
        code: None,
        initial_balance: dec!(100.00),
        currency: CurrencyCode::USD,
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
/// assert_eq!(input.currency, stateset_primitives::CurrencyCode::USD);
/// ```
pub fn create_store_credit_input(customer_id: CustomerId) -> CreateStoreCredit {
    CreateStoreCredit {
        customer_id,
        amount: dec!(50.00),
        currency: CurrencyCode::USD,
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
    CreateWishlist { customer_id, name: "My Wishlist".to_string(), is_public: false }
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

// ============================================================================
// Payment Fixtures
// ============================================================================

/// Create a [`CreatePayment`] input for the given order with a $59.98 USD credit-card charge.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_payment_input(stateset_core::OrderId::new());
/// assert_eq!(input.amount, rust_decimal_macros::dec!(59.98));
/// ```
pub fn create_payment_input(order_id: OrderId) -> CreatePayment {
    CreatePayment {
        order_id: Some(order_id),
        customer_id: Some(CustomerId::new()),
        payment_method: PaymentMethodType::CreditCard,
        amount: dec!(59.98),
        currency: Some(CurrencyCode::USD),
        processor: Some("stripe".into()),
        description: Some("Test payment".into()),
        ..Default::default()
    }
}

// ============================================================================
// Shipment Fixtures
// ============================================================================

/// Create a [`CreateShipment`] input for the given order with UPS Standard shipping.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_shipment_input(stateset_core::OrderId::new());
/// assert_eq!(input.recipient_name, "Test User");
/// ```
pub fn create_shipment_input(order_id: OrderId) -> CreateShipment {
    use stateset_core::models::shipment::{CreateShipmentItem, ShippingCarrier, ShippingMethod};

    CreateShipment {
        order_id,
        carrier: Some(ShippingCarrier::Ups),
        shipping_method: Some(ShippingMethod::Standard),
        tracking_number: Some("1Z999AA10123456784".into()),
        recipient_name: "Test User".into(),
        recipient_email: Some("test@example.com".into()),
        recipient_phone: Some("+1-555-0100".into()),
        shipping_address: "123 Main St, San Francisco, CA 94102, US".into(),
        weight_kg: Some(dec!(1.5)),
        dimensions: Some("30x20x10 cm".into()),
        shipping_cost: Some(dec!(9.99)),
        insurance_amount: None,
        signature_required: Some(false),
        estimated_delivery: None,
        notes: None,
        items: Some(vec![CreateShipmentItem {
            order_item_id: Some(Uuid::new_v4()),
            product_id: Some(ProductId::new()),
            sku: "TEST-SKU-001".into(),
            name: "Test Product".into(),
            quantity: 2,
        }]),
    }
}

// ============================================================================
// Return Fixtures
// ============================================================================

/// Create a [`CreateReturn`] input for the given order with one defective item.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_return_input(stateset_core::OrderId::new());
/// assert_eq!(input.reason, stateset_core::models::returns::ReturnReason::Defective);
/// assert_eq!(input.items.len(), 1);
/// ```
pub fn create_return_input(order_id: OrderId) -> CreateReturn {
    CreateReturn {
        order_id,
        reason: ReturnReason::Defective,
        reason_details: Some("Item arrived with a cracked screen".into()),
        idempotency_key: Some(Uuid::new_v4().to_string()),
        items: vec![CreateReturnItem {
            order_item_id: OrderItemId::new(),
            quantity: 1,
            condition: Some(stateset_core::models::returns::ItemCondition::Defective),
        }],
        notes: None,
    }
}

// ============================================================================
// Subscription Plan Fixtures
// ============================================================================

/// Create a [`CreateSubscriptionPlan`] for a $29.99/month plan with a 14-day trial.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_subscription_plan_input();
/// assert_eq!(input.name, "Test Monthly Plan");
/// assert_eq!(input.price, rust_decimal_macros::dec!(29.99));
/// ```
pub fn create_subscription_plan_input() -> CreateSubscriptionPlan {
    CreateSubscriptionPlan {
        code: Some(format!("PLAN-{}", Uuid::new_v4().as_simple())),
        name: "Test Monthly Plan".into(),
        description: Some("A monthly subscription plan for testing".into()),
        billing_interval: BillingInterval::Monthly,
        custom_interval_days: None,
        price: dec!(29.99),
        setup_fee: None,
        currency: Some(CurrencyCode::USD),
        trial_days: Some(14),
        trial_requires_payment_method: Some(true),
        min_cycles: None,
        max_cycles: None,
        items: None,
        discount_percent: None,
        discount_amount: None,
        metadata: None,
    }
}

// ============================================================================
// Subscription Fixtures
// ============================================================================

/// Create a [`CreateSubscription`] linking a customer to a plan.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_subscription_input(
///     stateset_core::CustomerId::new(),
///     uuid::Uuid::new_v4(),
/// );
/// assert!(input.skip_trial.is_none());
/// ```
pub fn create_subscription_input(customer_id: CustomerId, plan_id: Uuid) -> CreateSubscription {
    CreateSubscription {
        customer_id,
        plan_id,
        items: None,
        price: None,
        payment_method_id: Some("pm_test_123".into()),
        shipping_address: Some(test_address()),
        billing_address: None,
        skip_trial: None,
        start_date: None,
        coupon_code: None,
        metadata: None,
    }
}

// ============================================================================
// Cart Fixtures
// ============================================================================

/// Create a [`CreateCart`] with one test item in USD.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_cart_input(None);
/// assert_eq!(input.currency, Some(stateset_primitives::CurrencyCode::USD));
/// ```
pub fn create_cart_input(customer_id: Option<CustomerId>) -> CreateCart {
    CreateCart {
        customer_id,
        customer_email: if customer_id.is_none() { Some("guest@example.com".into()) } else { None },
        customer_name: if customer_id.is_none() { Some("Guest User".into()) } else { None },
        currency: Some(CurrencyCode::USD),
        items: Some(vec![AddCartItem {
            product_id: Some(ProductId::new()),
            variant_id: None,
            sku: "CART-SKU-001".into(),
            name: "Cart Test Item".into(),
            description: Some("An item for cart testing".into()),
            image_url: None,
            quantity: 1,
            unit_price: dec!(19.99),
            original_price: None,
            weight: None,
            requires_shipping: Some(true),
            metadata: None,
        }]),
        shipping_address: None,
        billing_address: None,
        notes: None,
        metadata: None,
        expires_in_minutes: Some(60),
    }
}

// ============================================================================
// Warranty Fixtures
// ============================================================================

/// Create a [`CreateWarranty`] for the given product with a 12-month standard warranty.
///
/// ```rust
/// let input = stateset_test_utils::fixtures::create_warranty_input(stateset_core::ProductId::new());
/// assert_eq!(input.duration_months, Some(12));
/// ```
pub fn create_warranty_input(product_id: ProductId) -> CreateWarranty {
    CreateWarranty {
        customer_id: CustomerId::new(),
        order_id: None,
        order_item_id: None,
        product_id: Some(product_id),
        sku: Some("WRN-SKU-001".into()),
        serial_number: Some(format!("SN-{}", Uuid::new_v4().as_simple())),
        warranty_type: Some(stateset_core::models::warranty::WarrantyType::Standard),
        provider: Some("Manufacturer".into()),
        coverage_description: Some("Covers manufacturing defects".into()),
        purchase_date: None,
        start_date: None,
        end_date: None,
        duration_months: Some(12),
        max_coverage_amount: Some(dec!(500.00)),
        deductible: Some(dec!(25.00)),
        max_claims: Some(3),
        terms: None,
        notes: None,
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
        assert_eq!(gc.currency, stateset_primitives::CurrencyCode::USD);
        assert_eq!(gc.initial_balance, dec!(100.00));
        assert_eq!(gc.recipient_email.as_deref(), Some("recipient@example.com"));
        assert!(gc.code.is_none(), "code should be auto-generated");
    }

    #[test]
    fn store_credit_input_is_valid() {
        let customer_id = CustomerId::new();
        let sc = create_store_credit_input(customer_id);
        assert_eq!(sc.customer_id, customer_id);
        assert_eq!(sc.currency, stateset_primitives::CurrencyCode::USD);
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

    #[test]
    fn payment_input_is_valid() {
        let order_id = OrderId::new();
        let payment = create_payment_input(order_id);
        assert_eq!(payment.order_id, Some(order_id));
        assert_eq!(payment.amount, dec!(59.98));
        assert_eq!(payment.currency, Some(stateset_primitives::CurrencyCode::USD));
        assert_eq!(payment.payment_method, PaymentMethodType::CreditCard);
        assert!(payment.customer_id.is_some());
    }

    #[test]
    fn shipment_input_is_valid() {
        let order_id = OrderId::new();
        let shipment = create_shipment_input(order_id);
        assert_eq!(shipment.order_id, order_id);
        assert_eq!(shipment.recipient_name, "Test User");
        assert!(shipment.tracking_number.is_some());
        assert!(shipment.items.is_some());
        assert_eq!(shipment.items.as_ref().unwrap().len(), 1);
        assert_eq!(shipment.items.as_ref().unwrap()[0].quantity, 2);
    }

    #[test]
    fn return_input_is_valid() {
        let order_id = OrderId::new();
        let ret = create_return_input(order_id);
        assert_eq!(ret.order_id, order_id);
        assert_eq!(ret.reason, ReturnReason::Defective);
        assert_eq!(ret.items.len(), 1);
        assert_eq!(ret.items[0].quantity, 1);
        assert!(ret.idempotency_key.is_some());
    }

    #[test]
    fn subscription_plan_input_is_valid() {
        let plan = create_subscription_plan_input();
        assert_eq!(plan.name, "Test Monthly Plan");
        assert_eq!(plan.price, dec!(29.99));
        assert_eq!(plan.billing_interval, BillingInterval::Monthly);
        assert_eq!(plan.trial_days, Some(14));
        assert_eq!(plan.currency, Some(stateset_primitives::CurrencyCode::USD));
        assert!(plan.code.is_some());
    }

    #[test]
    fn subscription_input_is_valid() {
        let customer_id = CustomerId::new();
        let plan_id = Uuid::new_v4();
        let sub = create_subscription_input(customer_id, plan_id);
        assert_eq!(sub.customer_id, customer_id);
        assert_eq!(sub.plan_id, plan_id);
        assert_eq!(sub.payment_method_id, Some("pm_test_123".to_string()));
        assert!(sub.shipping_address.is_some());
    }

    #[test]
    fn cart_input_with_customer_is_valid() {
        let customer_id = CustomerId::new();
        let cart = create_cart_input(Some(customer_id));
        assert_eq!(cart.customer_id, Some(customer_id));
        assert_eq!(cart.currency, Some(stateset_primitives::CurrencyCode::USD));
        assert!(cart.items.is_some());
        assert_eq!(cart.items.as_ref().unwrap().len(), 1);
        assert_eq!(cart.items.as_ref().unwrap()[0].sku, "CART-SKU-001");
        assert!(cart.customer_email.is_none(), "should not set email when customer_id is provided");
    }

    #[test]
    fn cart_input_guest_has_email() {
        let cart = create_cart_input(None);
        assert!(cart.customer_id.is_none());
        assert_eq!(cart.customer_email.as_deref(), Some("guest@example.com"));
        assert_eq!(cart.customer_name.as_deref(), Some("Guest User"));
    }

    #[test]
    fn warranty_input_is_valid() {
        let product_id = ProductId::new();
        let warranty = create_warranty_input(product_id);
        assert_eq!(warranty.product_id, Some(product_id));
        assert_eq!(warranty.duration_months, Some(12));
        assert_eq!(warranty.max_coverage_amount, Some(dec!(500.00)));
        assert_eq!(warranty.max_claims, Some(3));
        assert!(warranty.serial_number.is_some());
        assert!(warranty.sku.is_some());
    }
}
