#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{
    AdjustInventory, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreateProduct, OrderStatus, UpdateCustomer, UpdateOrder, UpdateProduct,
};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_core_crud_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres CRUD test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("test-{}@example.com", unique),
            first_name: "Test".into(),
            last_name: "User".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let updated_customer = db
        .customers()
        .update_async(
            customer.id,
            UpdateCustomer { last_name: Some("Updated".into()), ..Default::default() },
        )
        .await
        .expect("update customer");
    assert_eq!(updated_customer.last_name, "Updated");

    let product = db
        .products()
        .create_async(CreateProduct {
            name: format!("Widget {}", unique),
            slug: Some(format!("widget-{}", unique)),
            description: Some("Test product".into()),
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        })
        .await
        .expect("create product");

    let updated_product = db
        .products()
        .update_async(
            product.id,
            UpdateProduct { description: Some("Updated".into()), ..Default::default() },
        )
        .await
        .expect("update product");
    assert_eq!(updated_product.description, "Updated");

    let inventory_item = db
        .inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: "Widget".into(),
            description: None,
            unit_of_measure: None,
            initial_quantity: Some(dec!(10)),
            location_id: None,
            reorder_point: None,
            safety_stock: None,
        })
        .await
        .expect("create inventory item");
    assert_eq!(inventory_item.sku, sku);

    let stock = db.inventory().get_stock_async(&sku).await.expect("get stock").expect("stock row");
    assert_eq!(stock.total_on_hand, dec!(10));

    db.inventory()
        .adjust_async(AdjustInventory {
            sku: sku.clone(),
            location_id: None,
            quantity: dec!(5),
            reason: "test adjust".into(),
            reference_type: None,
            reference_id: None,
        })
        .await
        .expect("adjust inventory");

    let stock = db
        .inventory()
        .get_stock_async(&sku)
        .await
        .expect("get stock after adjust")
        .expect("stock row");
    assert_eq!(stock.total_on_hand, dec!(15));

    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                variant_id: None,
                sku: sku.clone(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(9.99),
                discount: None,
                tax_amount: None,
            }],
            ..Default::default()
        })
        .await
        .expect("create order");

    let fetched =
        db.orders().get_async(order.id.into()).await.expect("get order").expect("order row");
    assert_eq!(fetched.items.len(), 1);

    let updated = db
        .orders()
        .update_async(
            order.id.into(),
            UpdateOrder {
                status: Some(OrderStatus::Confirmed),
                payment_status: None,
                fulfillment_status: None,
                tracking_number: None,
                notes: None,
                shipping_address: None,
                billing_address: None,
            },
        )
        .await
        .expect("update order");
    assert_eq!(updated.status, OrderStatus::Confirmed);

    db.orders().delete_async(order.id.into()).await.expect("delete order");
    db.products().delete_async(product.id).await.expect("delete product");
    db.customers().delete_async(customer.id).await.expect("delete customer");
}
