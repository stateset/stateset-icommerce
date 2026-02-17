#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AddressType, CommerceError, CreateCustomer, CreateCustomerAddress, CreateOrder, CreateOrderItem,
    CreateProduct, ProductId, UpdateCustomer, UpdateProduct,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
async fn setup_db() -> Option<PostgresDatabase> {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres validation test");
            return None;
        }
    };

    Some(
        PostgresDatabase::connect(&url)
            .await
            .expect("connect to postgres and run migrations"),
    )
}

#[cfg(feature = "postgres")]
async fn create_customer(db: &PostgresDatabase, email: &str) -> stateset_core::Customer {
    db.customers()
        .create_async(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_order_rejects_invalid_sku() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("valid-{}@example.com", Uuid::new_v4())).await;

    let result = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "BAD SKU".to_string(),
                name: "Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await;

    assert!(matches!(
        result,
        Err(CommerceError::ValidationError(_)) | Err(CommerceError::InvalidInput { .. })
    ));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_order_rejects_discount_exceeding_subtotal() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("discount-{}@example.com", Uuid::new_v4())).await;

    let result = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SKU-100".to_string(),
                name: "Widget".to_string(),
                quantity: 1,
                unit_price: dec!(10.00),
                discount: Some(dec!(15.00)),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await;

    assert!(matches!(result, Err(CommerceError::ValidationError(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_customer_address_rejects_invalid_postal_code() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("address-{}@example.com", Uuid::new_v4())).await;

    let result = db
        .customers()
        .add_address_async(CreateCustomerAddress {
            customer_id: customer.id,
            address_type: None,
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            company: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "Springfield".to_string(),
            state: Some("CA".to_string()),
            postal_code: "X".to_string(),
            country: "US".to_string(),
            phone: None,
            is_default: Some(true),
        })
        .await;

    assert!(matches!(result, Err(CommerceError::ValidationError(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_customer_update_rejects_duplicate_email() {
    let Some(db) = setup_db().await else {
        return;
    };

    let first = create_customer(&db, &format!("first-{}@example.com", Uuid::new_v4())).await;
    let second = create_customer(&db, &format!("second-{}@example.com", Uuid::new_v4())).await;

    let result = db
        .customers()
        .update_async(
            second.id,
            UpdateCustomer { email: Some(first.email), ..Default::default() },
        )
        .await;

    assert!(matches!(result, Err(CommerceError::EmailAlreadyExists(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_customer_set_default_address_rejects_wrong_owner() {
    let Some(db) = setup_db().await else {
        return;
    };

    let owner = create_customer(&db, &format!("owner-{}@example.com", Uuid::new_v4())).await;
    let other = create_customer(&db, &format!("other-{}@example.com", Uuid::new_v4())).await;

    let address = db
        .customers()
        .add_address_async(CreateCustomerAddress {
            customer_id: owner.id,
            address_type: None,
            first_name: "Owner".to_string(),
            last_name: "User".to_string(),
            company: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "Springfield".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94105".to_string(),
            country: "US".to_string(),
            phone: None,
            is_default: Some(true),
        })
        .await
        .expect("create owner address");

    let result = db
        .customers()
        .set_default_address_async(other.id, address.id, AddressType::Shipping)
        .await;

    assert!(matches!(result, Err(CommerceError::ValidationError(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_add_address_sets_default_ids() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("default-{}@example.com", Uuid::new_v4())).await;

    let billing = db
        .customers()
        .add_address_async(CreateCustomerAddress {
            customer_id: customer.id,
            address_type: Some(AddressType::Billing),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            company: None,
            line1: "456 Market St".to_string(),
            line2: None,
            city: "Springfield".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94105".to_string(),
            country: "US".to_string(),
            phone: None,
            is_default: Some(true),
        })
        .await
        .expect("create billing address");

    let shipping = db
        .customers()
        .add_address_async(CreateCustomerAddress {
            customer_id: customer.id,
            address_type: Some(AddressType::Shipping),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            company: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "Springfield".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94105".to_string(),
            country: "US".to_string(),
            phone: None,
            is_default: Some(true),
        })
        .await
        .expect("create shipping address");

    let updated = db.customers().get_async(customer.id).await.expect("get customer").expect("customer exists");

    assert_eq!(updated.default_billing_address_id, Some(billing.id));
    assert_eq!(updated.default_shipping_address_id, Some(shipping.id));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_delete_default_address_clears_customer_default() {
    let Some(db) = setup_db().await else {
        return;
    };

    let customer = create_customer(&db, &format!("delete-default-{}@example.com", Uuid::new_v4())).await;

    let shipping = db
        .customers()
        .add_address_async(CreateCustomerAddress {
            customer_id: customer.id,
            address_type: Some(AddressType::Shipping),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            company: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "Springfield".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94105".to_string(),
            country: "US".to_string(),
            phone: None,
            is_default: Some(true),
        })
        .await
        .expect("create shipping address");

    db.customers()
        .delete_address_async(shipping.id)
        .await
        .expect("delete shipping address");

    let updated = db.customers().get_async(customer.id).await.expect("get customer").expect("customer exists");
    assert_eq!(updated.default_shipping_address_id, None);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_product_update_rejects_duplicate_slug() {
    let Some(db) = setup_db().await else {
        return;
    };

    let first = db
        .products()
        .create_async(CreateProduct {
            name: "First".into(),
            slug: Some("first".into()),
            ..Default::default()
        })
        .await
        .expect("create first product");

    let second = db
        .products()
        .create_async(CreateProduct {
            name: "Second".into(),
            slug: Some("second".into()),
            ..Default::default()
        })
        .await
        .expect("create second product");

    let result = db
        .products()
        .update_async(second.id, UpdateProduct { slug: Some(first.slug.clone()), ..Default::default() })
        .await;

    assert!(matches!(result, Err(CommerceError::DuplicateSlug(_))));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_product_update_batch_atomic_rejects_duplicate_slug() {
    let Some(db) = setup_db().await else {
        return;
    };

    let first = db
        .products()
        .create_async(CreateProduct {
            name: "First".into(),
            slug: Some("batch-first".into()),
            ..Default::default()
        })
        .await
        .expect("create first product");

    let second = db
        .products()
        .create_async(CreateProduct {
            name: "Second".into(),
            slug: Some("batch-second".into()),
            ..Default::default()
        })
        .await
        .expect("create second product");

    let result = db
        .products()
        .update_batch_atomic_async(vec![(
            second.id,
            UpdateProduct { slug: Some(first.slug.clone()), ..Default::default() },
        )])
        .await;

    match result {
        Err(CommerceError::DuplicateSlug(_)) => {}
        Err(other) => panic!("expected DuplicateSlug, got {other:?}"),
        Ok(_) => panic!("expected DuplicateSlug, got success"),
    }
}
