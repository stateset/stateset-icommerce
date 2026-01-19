#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    AddressType, CommerceError, CreateCustomer, CreateCustomerAddress, CreateOrder,
    CreateOrderItem, CustomerRepository, OrderRepository, UpdateCustomer,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn create_customer(db: &SqliteDatabase, email: &str) -> stateset_core::Customer {
    db.customers()
        .create(CreateCustomer {
            email: email.to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            ..Default::default()
        })
        .expect("create customer")
}

#[test]
fn sqlite_order_rejects_invalid_sku() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "valid@example.com");

    let result = db.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            product_id: Uuid::new_v4(),
            sku: "BAD SKU".to_string(),
            name: "Widget".to_string(),
            quantity: 1,
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        ..Default::default()
    });

    assert!(matches!(
        result,
        Err(CommerceError::ValidationError(_)) | Err(CommerceError::InvalidInput { .. })
    ));
}

#[test]
fn sqlite_order_rejects_discount_exceeding_subtotal() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "discount@example.com");

    let result = db.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            product_id: Uuid::new_v4(),
            sku: "SKU-100".to_string(),
            name: "Widget".to_string(),
            quantity: 1,
            unit_price: dec!(10.00),
            discount: Some(dec!(15.00)),
            ..Default::default()
        }],
        ..Default::default()
    });

    assert!(matches!(result, Err(CommerceError::ValidationError(_))));
}

#[test]
fn sqlite_customer_address_rejects_invalid_postal_code() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "address@example.com");

    let result = db.customers().add_address(CreateCustomerAddress {
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
    });

    assert!(matches!(result, Err(CommerceError::ValidationError(_))));
}

#[test]
fn sqlite_customer_update_rejects_duplicate_email() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let first = create_customer(&db, "first@example.com");
    let second = create_customer(&db, "second@example.com");

    let result = db.customers().update(
        second.id,
        UpdateCustomer {
            email: Some(first.email),
            ..Default::default()
        },
    );

    assert!(matches!(result, Err(CommerceError::EmailAlreadyExists(_))));
}

#[test]
fn sqlite_customer_set_default_address_rejects_wrong_owner() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let owner = create_customer(&db, "owner@example.com");
    let other = create_customer(&db, "other@example.com");

    let address = db
        .customers()
        .add_address(CreateCustomerAddress {
            customer_id: owner.id,
            address_type: None,
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
        .expect("create address");

    let result = db
        .customers()
        .set_default_address(other.id, address.id, AddressType::Shipping);

    assert!(matches!(result, Err(CommerceError::ValidationError(_))));
}

#[test]
fn sqlite_add_address_sets_default_ids() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "default@example.com");

    let billing = db
        .customers()
        .add_address(CreateCustomerAddress {
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
        .expect("create address");

    let shipping = db
        .customers()
        .add_address(CreateCustomerAddress {
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
        .expect("create address");

    let updated = db
        .customers()
        .get(customer.id)
        .expect("get customer")
        .expect("customer exists");

    assert_eq!(updated.default_billing_address_id, Some(billing.id));
    assert_eq!(updated.default_shipping_address_id, Some(shipping.id));
}

#[test]
fn sqlite_delete_default_address_clears_customer_default() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = create_customer(&db, "delete-default@example.com");

    let shipping = db
        .customers()
        .add_address(CreateCustomerAddress {
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
        .expect("create address");

    db.customers()
        .delete_address(shipping.id)
        .expect("delete address");

    let updated = db
        .customers()
        .get(customer.id)
        .expect("get customer")
        .expect("customer exists");

    assert_eq!(updated.default_shipping_address_id, None);
}
