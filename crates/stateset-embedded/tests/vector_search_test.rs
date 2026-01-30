//! Vector search integration tests
//!
//! Tests the full embed → store → search round-trip using the embedded commerce engine.
//! Requires OPENAI_API_KEY environment variable. Run with:
//!   OPENAI_API_KEY=sk-... cargo test --test vector_search_test -- --ignored
//!
//! All tests are #[ignore] by default so they don't run in CI without the API key.

use rust_decimal_macros::dec;
use stateset_embedded::{Commerce, CreateCustomer, CreateProduct};
use uuid::Uuid;

// ============================================================================
// Helpers
// ============================================================================

fn api_key() -> String {
    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY required for vector tests")
}

fn test_commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to initialize commerce engine")
}

// ============================================================================
// Round-Trip: Create → Index → Search → Verify
// ============================================================================

#[test]
#[ignore]
fn test_product_embed_store_search_roundtrip() {
    let commerce = test_commerce();
    let vector = commerce.vector(api_key()).expect("vector init");

    // Create two semantically distinct products
    let headphones = commerce
        .products()
        .create(CreateProduct {
            name: "Wireless Bluetooth Headphones".into(),
            description: Some("Noise cancelling over-ear headphones with 30h battery".into()),
            sku: Some(format!("VEC-HP-{}", Uuid::new_v4().simple())),
            price: Some(dec!(149.99)),
            ..Default::default()
        })
        .expect("create headphones");

    let tea = commerce
        .products()
        .create(CreateProduct {
            name: "Organic Green Tea".into(),
            description: Some("Premium loose leaf green tea from Japanese highlands".into()),
            sku: Some(format!("VEC-TEA-{}", Uuid::new_v4().simple())),
            price: Some(dec!(24.99)),
            ..Default::default()
        })
        .expect("create tea");

    // Index both products
    vector.index_product(&headphones).expect("index headphones");
    vector.index_product(&tea).expect("index tea");

    // Search for something similar to headphones
    let results = vector
        .search_products("bluetooth audio headset noise cancelling", 5)
        .expect("search products");

    assert!(!results.is_empty(), "Should return at least one result");
    assert_eq!(
        results[0].entity.id, headphones.id,
        "Headphones should rank first for audio query"
    );
    assert!(
        results[0].score > 0.5,
        "Top result should have high similarity score"
    );

    // Search for something similar to tea
    let results = vector
        .search_products("green tea leaves organic", 5)
        .expect("search for tea");

    assert!(!results.is_empty(), "Should return at least one result");
    assert_eq!(
        results[0].entity.id, tea.id,
        "Tea should rank first for tea query"
    );
}

// ============================================================================
// Batch Indexing
// ============================================================================

#[test]
#[ignore]
fn test_batch_index_products() {
    let commerce = test_commerce();
    let vector = commerce.vector(api_key()).expect("vector init");

    // Create 5 products
    let mut products = Vec::new();
    let names = [
        ("Running Shoes", "Lightweight running shoes for marathons"),
        ("Hiking Boots", "Waterproof hiking boots for trails"),
        ("Dress Shoes", "Leather dress shoes for formal events"),
        ("Sandals", "Open-toe summer sandals"),
        ("Sneakers", "Casual everyday sneakers"),
    ];

    for (name, desc) in &names {
        let p = commerce
            .products()
            .create(CreateProduct {
                name: name.to_string(),
                description: Some(desc.to_string()),
                sku: Some(format!("BATCH-{}", Uuid::new_v4().simple())),
                price: Some(dec!(89.99)),
                ..Default::default()
            })
            .expect("create product");
        products.push(p);
    }

    // Batch index
    let indexed = vector
        .index_products(&products)
        .expect("batch index");
    assert_eq!(indexed, 5, "All 5 products should be indexed");

    // Search should find all of them
    let results = vector
        .search_products("footwear shoes", 10)
        .expect("search");
    assert!(results.len() >= 3, "Should find multiple shoe-related products");
}

// ============================================================================
// Cross-Entity Isolation
// ============================================================================

#[test]
#[ignore]
fn test_cross_entity_search_isolation() {
    let commerce = test_commerce();
    let vector = commerce.vector(api_key()).expect("vector init");

    // Create a product and a customer with similar text
    let product = commerce
        .products()
        .create(CreateProduct {
            name: "Coffee Machine Deluxe".into(),
            description: Some("Premium espresso coffee machine".into()),
            sku: Some(format!("ISO-{}", Uuid::new_v4().simple())),
            ..Default::default()
        })
        .expect("create product");

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("coffee-lover-{}@test.com", Uuid::new_v4()),
            first_name: "Coffee".into(),
            last_name: "Lover".into(),
            ..Default::default()
        })
        .expect("create customer");

    // Index both
    vector.index_product(&product).expect("index product");
    vector.index_customer(&customer).expect("index customer");

    // Search products should only return products
    let product_results = vector
        .search_products("coffee", 10)
        .expect("search products");
    for r in &product_results {
        assert_eq!(r.entity.id, product.id, "Product search should only return products");
    }

    // Search customers should only return customers
    let customer_results = vector
        .search_customers("coffee", 10)
        .expect("search customers");
    for r in &customer_results {
        assert_eq!(r.entity.id, customer.id, "Customer search should only return customers");
    }
}

// ============================================================================
// Stats and Cleanup
// ============================================================================

#[test]
#[ignore]
fn test_embedding_stats_and_cleanup() {
    let commerce = test_commerce();
    let vector = commerce.vector(api_key()).expect("vector init");

    let product = commerce
        .products()
        .create(CreateProduct {
            name: "Stats Test Product".into(),
            sku: Some(format!("STAT-{}", Uuid::new_v4().simple())),
            ..Default::default()
        })
        .expect("create product");

    // Initially empty
    let stats = vector.stats().expect("stats");
    assert_eq!(stats.product_count, 0);

    // Index and verify stats
    vector.index_product(&product).expect("index");
    let stats = vector.stats().expect("stats");
    assert_eq!(stats.product_count, 1);

    // Check is_indexed
    let indexed = vector
        .is_indexed(stateset_embedded::EntityType::Product, &product.id.to_string())
        .expect("is_indexed");
    assert!(indexed, "Product should be indexed");

    // Unindex and verify
    vector
        .unindex_product(&product.id.to_string())
        .expect("unindex");
    let indexed = vector
        .is_indexed(stateset_embedded::EntityType::Product, &product.id.to_string())
        .expect("is_indexed");
    assert!(!indexed, "Product should no longer be indexed");

    let stats = vector.stats().expect("stats");
    assert_eq!(stats.product_count, 0);
}

// ============================================================================
// Clear All
// ============================================================================

#[test]
#[ignore]
fn test_clear_all_embeddings() {
    let commerce = test_commerce();
    let vector = commerce.vector(api_key()).expect("vector init");

    // Index a product and a customer
    let product = commerce
        .products()
        .create(CreateProduct {
            name: "Clear Test".into(),
            sku: Some(format!("CLR-{}", Uuid::new_v4().simple())),
            ..Default::default()
        })
        .expect("create");
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("clear-{}@test.com", Uuid::new_v4()),
            first_name: "Clear".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("create");

    vector.index_product(&product).expect("index product");
    vector.index_customer(&customer).expect("index customer");

    let stats = vector.stats().expect("stats");
    assert!(stats.product_count > 0);
    assert!(stats.customer_count > 0);

    // Clear all
    vector.clear_all().expect("clear all");

    let stats = vector.stats().expect("stats after clear");
    assert_eq!(stats.product_count, 0);
    assert_eq!(stats.customer_count, 0);
}
