//! Error handling patterns for stateset-icommerce
//!
//! This example demonstrates:
//! - Error categorization and matching
//! - Validation error handling
//! - Database error handling
//! - Retry patterns for retryable errors
//!
//! Run with: cargo run --example error_handling

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    Validate, ValidationBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== StateSet Error Handling Example ===\n");

    let commerce = Commerce::in_memory()?;

    // ========================================================================
    // 1. Not Found Errors
    // ========================================================================
    println!("1. Handling 'Not Found' errors...");

    let non_existent_id = uuid::Uuid::new_v4();
    match commerce.orders().get(non_existent_id) {
        Ok(Some(order)) => println!("   Found order: {}", order.order_number),
        Ok(None) => println!("   Order not found (returned None)"),
        Err(e) if e.is_not_found() => println!("   Error: Not found - {}", e),
        Err(e) => println!("   Unexpected error: {}", e),
    }

    // Try to update a non-existent order
    match commerce.orders().update_status(non_existent_id, stateset_embedded::OrderStatus::Confirmed) {
        Ok(_) => println!("   Order updated"),
        Err(e) if e.is_not_found() => println!("   Expected error: {}", e),
        Err(e) => println!("   Unexpected error: {}", e),
    }
    println!();

    // ========================================================================
    // 2. Validation Errors
    // ========================================================================
    println!("2. Handling validation errors...");

    // Try to create customer with invalid email
    match commerce.customers().create(CreateCustomer {
        email: "".into(), // Empty email
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    }) {
        Ok(customer) => println!("   Created customer: {}", customer.email),
        Err(e) if e.is_validation() => println!("   Validation error (expected): {}", e),
        Err(e) => println!("   Other error: {}", e),
    }

    // Using the ValidationBuilder for custom validation
    println!("\n   Using ValidationBuilder for custom validation:");

    let email = "test@example.com";
    let quantity = -5;
    let sku = "SKU WITH SPACES"; // Invalid SKU

    let validation_result = ValidationBuilder::new()
        .email("email", email)
        .positive_i32("quantity", quantity)
        .sku("sku", sku)
        .build_all();

    match validation_result {
        Ok(()) => println!("   All validations passed"),
        Err(CommerceError::ValidationError(msg)) => {
            println!("   Multiple validation errors:");
            for error in msg.split("; ") {
                println!("     - {}", error);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // ========================================================================
    // 3. Conflict Errors (Duplicates)
    // ========================================================================
    println!("3. Handling conflict errors...");

    // Create a customer
    commerce.customers().create(CreateCustomer {
        email: "duplicate@example.com".into(),
        first_name: "First".into(),
        last_name: "User".into(),
        ..Default::default()
    })?;
    println!("   Created first customer");

    // Try to create another customer with same email
    match commerce.customers().create(CreateCustomer {
        email: "duplicate@example.com".into(),
        first_name: "Second".into(),
        last_name: "User".into(),
        ..Default::default()
    }) {
        Ok(_) => println!("   Created second customer (unexpected)"),
        Err(e) if e.is_conflict() => println!("   Conflict error (expected): {}", e),
        Err(e) => println!("   Other error: {}", e),
    }

    // Create inventory item
    commerce.inventory().create_item(CreateInventoryItem {
        sku: "DUPLICATE-SKU".into(),
        name: "First Item".into(),
        initial_quantity: Some(dec!(10)),
        ..Default::default()
    })?;
    println!("   Created first inventory item");

    // Try to create item with same SKU
    match commerce.inventory().create_item(CreateInventoryItem {
        sku: "DUPLICATE-SKU".into(),
        name: "Second Item".into(),
        initial_quantity: Some(dec!(20)),
        ..Default::default()
    }) {
        Ok(_) => println!("   Created second item (unexpected)"),
        Err(e) if e.is_conflict() => println!("   Conflict error (expected): {}", e),
        Err(e) => println!("   Other error: {}", e),
    }
    println!();

    // ========================================================================
    // 4. Database Errors
    // ========================================================================
    println!("4. Handling database errors...");

    // Database errors are typically handled at a higher level
    // Here's how you might check for database-specific errors:

    fn handle_database_error(err: &CommerceError) {
        if let Some(db_err) = err.as_db_error() {
            match db_err {
                stateset_embedded::DbError::ConnectionFailed { url, message } => {
                    println!("   Connection failed to {}: {}", url, message);
                }
                stateset_embedded::DbError::QueryFailed { table, operation, message } => {
                    println!("   Query failed on {} ({}): {}", table, operation, message);
                }
                stateset_embedded::DbError::ConstraintViolation { table, constraint, .. } => {
                    println!("   Constraint violation on {}: {}", table, constraint);
                }
                stateset_embedded::DbError::TransactionFailed { message } => {
                    println!("   Transaction failed: {}", message);
                }
                stateset_embedded::DbError::PoolExhausted { timeout_ms } => {
                    println!("   Connection pool exhausted after {}ms", timeout_ms);
                }
                _ => println!("   Other database error: {}", db_err),
            }
        } else if err.is_database() {
            println!("   Legacy database error: {}", err);
        }
    }

    // Simulate checking a database error (in practice, this would come from an operation)
    let simulated_err = CommerceError::query_failed("orders", "insert", "simulated failure");
    handle_database_error(&simulated_err);
    println!();

    // ========================================================================
    // 5. Retry Pattern for Retryable Errors
    // ========================================================================
    println!("5. Retry pattern for retryable errors...");

    fn retry_operation<T, F>(mut operation: F, max_retries: u32) -> Result<T, CommerceError>
    where
        F: FnMut() -> Result<T, CommerceError>,
    {
        let mut attempts = 0;
        loop {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retryable() && attempts < max_retries => {
                    attempts += 1;
                    println!("   Retry {}/{}: {}", attempts, max_retries, e);
                    std::thread::sleep(std::time::Duration::from_millis(100 * attempts as u64));
                }
                Err(e) => return Err(e),
            }
        }
    }

    // Example usage (will succeed on first try since there's no real error)
    let result = retry_operation(
        || {
            commerce.inventory().get_stock("DUPLICATE-SKU")
        },
        3,
    );

    match result {
        Ok(Some(stock)) => println!("   Got stock: {} available", stock.total_available),
        Ok(None) => println!("   Stock not found"),
        Err(e) => println!("   Failed after retries: {}", e),
    }
    println!();

    // ========================================================================
    // 6. Error Matching Patterns
    // ========================================================================
    println!("6. Comprehensive error matching...");

    fn classify_error(err: &CommerceError) -> &'static str {
        if err.is_not_found() {
            "NOT_FOUND"
        } else if err.is_validation() {
            "VALIDATION_ERROR"
        } else if err.is_conflict() {
            "CONFLICT"
        } else if err.is_database() {
            "DATABASE_ERROR"
        } else if err.is_external_service() {
            "EXTERNAL_SERVICE"
        } else if err.is_retryable() {
            "RETRYABLE"
        } else {
            "UNKNOWN"
        }
    }

    // Test various errors
    let test_errors = vec![
        CommerceError::OrderNotFound(uuid::Uuid::new_v4()),
        CommerceError::ValidationError("test".to_string()),
        CommerceError::DuplicateSku("SKU-001".to_string()),
        CommerceError::DatabaseError("connection lost".to_string()),
        CommerceError::ExternalServiceError("payment gateway timeout".to_string()),
        CommerceError::OptimisticLockFailure,
    ];

    for err in &test_errors {
        println!("   {} -> {}", err, classify_error(err));
    }
    println!();

    // ========================================================================
    // 7. Result Combinators
    // ========================================================================
    println!("7. Using Result combinators...");

    // Chain operations with proper error handling
    let result = commerce
        .customers()
        .get_by_email("duplicate@example.com")?
        .ok_or(CommerceError::CustomerNotFound(uuid::Uuid::nil()))
        .map(|customer| {
            println!("   Found customer: {} {}", customer.first_name, customer.last_name);
            customer
        });

    match result {
        Ok(customer) => println!("   Customer ID: {}", customer.id),
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
