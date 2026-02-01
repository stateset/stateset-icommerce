//! Validation patterns for stateset-icommerce
//!
//! This example demonstrates:
//! - Using the Validate trait for domain models
//! - Using ValidationBuilder for composable validations
//! - Custom validation rules
//! - Validation in create/update workflows
//!
//! Run with: cargo run --example validation

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, Result, Validate, ValidationBuilder,
    CreateCustomer, CreateProduct, CreateProductVariant, CreateOrder, CreateOrderItem,
};

// ============================================================================
// Custom Domain Model with Validation
// ============================================================================

/// A custom order request that implements validation
struct OrderRequest {
    customer_email: String,
    shipping_address: String,
    items: Vec<OrderItemRequest>,
    discount_code: Option<String>,
}

struct OrderItemRequest {
    sku: String,
    quantity: i32,
    unit_price: Decimal,
}

impl Validate for OrderRequest {
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .email("customer_email", &self.customer_email)
            .required("shipping_address", &self.shipping_address)
            .min_length("shipping_address", &self.shipping_address, 10)
            .non_empty_list("items", &self.items)
            .max_items("items", &self.items, 100)
            .build()?;

        // Validate each item
        for (i, item) in self.items.iter().enumerate() {
            item.validate_with_index(i)?;
        }

        // Custom validation: discount code format if present
        if let Some(ref code) = self.discount_code {
            if !code.is_empty() && (code.len() < 4 || code.len() > 20) {
                return Err(CommerceError::InvalidInput {
                    field: "discount_code".to_string(),
                    message: "must be 4-20 characters".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl OrderItemRequest {
    fn validate_with_index(&self, index: usize) -> Result<()> {
        let field_prefix = format!("items[{}]", index);

        ValidationBuilder::new()
            .sku(&format!("{}.sku", field_prefix), &self.sku)
            .positive_i32(&format!("{}.quantity", field_prefix), self.quantity)
            .non_negative(&format!("{}.unit_price", field_prefix), self.unit_price)
            .build()
    }
}

// ============================================================================
// Validation Examples
// ============================================================================

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("=== StateSet Validation Example ===\n");

    let commerce = Commerce::in_memory()?;

    // ========================================================================
    // 1. Basic Validation with ValidationBuilder
    // ========================================================================
    println!("1. Basic validation with ValidationBuilder...\n");

    // Valid data
    let valid_result = ValidationBuilder::new()
        .required("name", "Alice Smith")
        .email("email", "alice@example.com")
        .positive("price", dec!(29.99))
        .currency_code("currency", "USD")
        .build();

    println!("   Valid data: {:?}", valid_result.is_ok());

    // Invalid data - multiple errors
    let invalid_result = ValidationBuilder::new()
        .required("name", "")
        .email("email", "not-an-email")
        .positive("price", dec!(-10))
        .currency_code("currency", "usd") // lowercase
        .build_all(); // Get all errors

    match invalid_result {
        Ok(()) => println!("   Unexpectedly valid"),
        Err(CommerceError::ValidationError(msg)) => {
            println!("   Multiple errors found:");
            for error in msg.split("; ") {
                println!("     - {}", error);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // ========================================================================
    // 2. Validate Trait Usage
    // ========================================================================
    println!("2. Using the Validate trait...\n");

    // Valid order request
    let valid_order = OrderRequest {
        customer_email: "customer@example.com".to_string(),
        shipping_address: "123 Main Street, Anytown, ST 12345".to_string(),
        items: vec![
            OrderItemRequest {
                sku: "SKU-001".to_string(),
                quantity: 2,
                unit_price: dec!(29.99),
            },
            OrderItemRequest {
                sku: "SKU-002".to_string(),
                quantity: 1,
                unit_price: dec!(49.99),
            },
        ],
        discount_code: Some("SAVE10".to_string()),
    };

    match valid_order.validate() {
        Ok(()) => println!("   Valid order request passes validation"),
        Err(e) => println!("   Validation failed: {}", e),
    }

    // Check if valid without error
    println!("   Is valid: {}", valid_order.is_valid());

    // Invalid order request
    let invalid_order = OrderRequest {
        customer_email: "bad-email".to_string(),
        shipping_address: "Short".to_string(),
        items: vec![
            OrderItemRequest {
                sku: "SKU WITH SPACES".to_string(),
                quantity: -1,
                unit_price: dec!(-5.00),
            },
        ],
        discount_code: Some("AB".to_string()), // Too short
    };

    match invalid_order.validate() {
        Ok(()) => println!("   Unexpectedly valid"),
        Err(e) => println!("   Validation failed (expected): {}", e),
    }
    println!();

    // ========================================================================
    // 3. Using validated() for Method Chaining
    // ========================================================================
    println!("3. Method chaining with validated()...\n");

    fn process_order(order: OrderRequest) -> Result<String> {
        // Validate and process in one chain
        let order = order.validated()?;
        Ok(format!("Processing order with {} items", order.items.len()))
    }

    match process_order(valid_order) {
        Ok(msg) => println!("   Success: {}", msg),
        Err(e) => println!("   Failed: {}", e),
    }
    println!();

    // ========================================================================
    // 4. Validation in Commerce Operations
    // ========================================================================
    println!("4. Validation in commerce operations...\n");

    // Create a valid customer
    let customer = commerce.customers().create(CreateCustomer {
        email: "valid@example.com".into(),
        first_name: "Valid".into(),
        last_name: "User".into(),
        phone: Some("+1-555-123-4567".into()),
        ..Default::default()
    })?;
    println!("   Created customer: {} <{}>", customer.full_name(), customer.email);

    // Create a valid product
    let product = commerce.products().create(CreateProduct {
        name: "Test Product".into(),
        description: Some("A product for validation testing".into()),
        variants: Some(vec![CreateProductVariant {
            sku: "TEST-SKU-001".into(),
            price: dec!(99.99),
            ..Default::default()
        }]),
        ..Default::default()
    })?;
    println!("   Created product: {} ({})", product.name, product.slug);

    // Create a valid order
    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            sku: "TEST-SKU-001".into(),
            name: "Test Product".into(),
            quantity: 2,
            unit_price: dec!(99.99),
            ..Default::default()
        }],
        ..Default::default()
    })?;
    println!("   Created order: {} (total: ${})", order.order_number, order.total_amount);
    println!();

    // ========================================================================
    // 5. Custom Validation Helpers
    // ========================================================================
    println!("5. Custom validation helpers...\n");

    // You can extend ValidationBuilder with custom validations
    fn validate_order_total(items: &[OrderItemRequest], max_total: Decimal) -> Result<()> {
        let total: Decimal = items.iter()
            .map(|item| item.unit_price * Decimal::from(item.quantity))
            .sum();

        if total > max_total {
            return Err(CommerceError::ValidationError(
                format!("Order total ${} exceeds maximum ${}", total, max_total)
            ));
        }
        Ok(())
    }

    let items = vec![
        OrderItemRequest {
            sku: "ITEM-1".to_string(),
            quantity: 10,
            unit_price: dec!(100.00),
        },
    ];

    match validate_order_total(&items, dec!(500.00)) {
        Ok(()) => println!("   Order total is within limits"),
        Err(e) => println!("   Validation failed: {}", e),
    }
    println!();

    // ========================================================================
    // 6. Validation with Context
    // ========================================================================
    println!("6. Contextual validation...\n");

    // Sometimes validation depends on context (e.g., existing data)
    fn validate_unique_email(commerce: &Commerce, email: &str) -> Result<()> {
        if let Some(_) = commerce.customers().get_by_email(email)? {
            return Err(CommerceError::EmailAlreadyExists(email.to_string()));
        }
        Ok(())
    }

    // This should fail - email already exists
    match validate_unique_email(&commerce, "valid@example.com") {
        Ok(()) => println!("   Email is unique"),
        Err(e) => println!("   Email validation failed (expected): {}", e),
    }

    // This should succeed - email is new
    match validate_unique_email(&commerce, "new@example.com") {
        Ok(()) => println!("   Email is unique (new email)"),
        Err(e) => println!("   Unexpected error: {}", e),
    }
    println!();

    // ========================================================================
    // 7. Batch Validation
    // ========================================================================
    println!("7. Batch validation...\n");

    let emails = vec![
        "valid1@example.com",
        "invalid-email",
        "valid2@example.com",
        "",
        "valid3@example.com",
    ];

    let validation_results: Vec<(&str, bool)> = emails
        .iter()
        .map(|email| {
            let is_valid = ValidationBuilder::new()
                .email("email", email)
                .build()
                .is_ok();
            (*email, is_valid)
        })
        .collect();

    println!("   Batch email validation results:");
    for (email, is_valid) in validation_results {
        let status = if is_valid { "valid" } else { "invalid" };
        let display_email = if email.is_empty() { "(empty)" } else { email };
        println!("     {} -> {}", display_email, status);
    }

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
