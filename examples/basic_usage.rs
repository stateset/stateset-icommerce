//! Basic usage example for stateset-icommerce
//!
//! Run with: cargo run --example basic_usage

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreateProduct, CreateProductVariant, OrderStatus,
};

fn main() -> Result<(), CommerceError> {
    println!("=== StateSet iCommerce Example ===\n");

    // Initialize commerce with in-memory database
    let commerce = Commerce::new(":memory:")?;
    println!("✓ Commerce initialized\n");

    // 1. Create a customer
    println!("1. Creating customer...");
    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        phone: Some("+1-555-0123".into()),
        accepts_marketing: Some(true),
        ..Default::default()
    })?;
    println!("   Created customer: {} ({})", customer.full_name(), customer.email);

    // 2. Create products
    println!("\n2. Creating products...");
    let widget = commerce.products().create(CreateProduct {
        name: "Premium Widget".into(),
        description: Some("A high-quality widget for all your needs".into()),
        variants: Some(vec![CreateProductVariant {
            sku: "WIDGET-001".into(),
            name: Some("Standard Widget".into()),
            price: dec!(29.99),
            compare_at_price: Some(dec!(39.99)),
            ..Default::default()
        }]),
        ..Default::default()
    })?;
    println!("   Created product: {} (slug: {})", widget.name, widget.slug);

    let gadget = commerce.products().create(CreateProduct {
        name: "Super Gadget".into(),
        description: Some("An amazing gadget".into()),
        variants: Some(vec![CreateProductVariant {
            sku: "GADGET-001".into(),
            price: dec!(49.99),
            ..Default::default()
        }]),
        ..Default::default()
    })?;
    println!("   Created product: {} (slug: {})", gadget.name, gadget.slug);

    // 3. Create inventory
    println!("\n3. Setting up inventory...");
    commerce.inventory().create_item(CreateInventoryItem {
        sku: "WIDGET-001".into(),
        name: "Premium Widget".into(),
        initial_quantity: Some(dec!(100)),
        reorder_point: Some(dec!(10)),
        ..Default::default()
    })?;
    println!("   Created inventory for WIDGET-001 (100 units)");

    commerce.inventory().create_item(CreateInventoryItem {
        sku: "GADGET-001".into(),
        name: "Super Gadget".into(),
        initial_quantity: Some(dec!(50)),
        reorder_point: Some(dec!(5)),
        ..Default::default()
    })?;
    println!("   Created inventory for GADGET-001 (50 units)");

    // Check stock
    if let Some(stock) = commerce.inventory().get_stock("WIDGET-001")? {
        println!("   Stock check WIDGET-001: {} available", stock.total_available);
    }

    // 4. Create an order
    println!("\n4. Creating order...");
    let widget_variant = commerce.products().get_variant_by_sku("WIDGET-001")?.unwrap();
    let gadget_variant = commerce.products().get_variant_by_sku("GADGET-001")?.unwrap();

    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![
            CreateOrderItem {
                product_id: widget.id,
                variant_id: Some(widget_variant.id),
                sku: "WIDGET-001".into(),
                name: "Premium Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
            CreateOrderItem {
                product_id: gadget.id,
                variant_id: Some(gadget_variant.id),
                sku: "GADGET-001".into(),
                name: "Super Gadget".into(),
                quantity: 1,
                unit_price: dec!(49.99),
                ..Default::default()
            },
        ],
        ..Default::default()
    })?;
    println!(
        "   Created order {} (total: ${})",
        order.order_number, order.total_amount
    );

    // 5. Process the order
    println!("\n5. Processing order...");

    // Reserve inventory
    let reservation = commerce.inventory().reserve(
        "WIDGET-001",
        dec!(2),
        "order",
        &order.id.to_string(),
        Some(3600),
    )?;
    println!("   Reserved 2x WIDGET-001 for order");

    // Update order status
    let order = commerce.orders().update_status(order.id, OrderStatus::Confirmed)?;
    println!("   Order status: {:?}", order.status);

    // Adjust inventory (fulfill)
    commerce.inventory().adjust("WIDGET-001", dec!(-2), "Order fulfillment")?;
    commerce.inventory().adjust("GADGET-001", dec!(-1), "Order fulfillment")?;
    println!("   Inventory adjusted");

    // Confirm reservation
    commerce.inventory().confirm_reservation(reservation.id)?;
    println!("   Reservation confirmed");

    // Ship the order
    let order = commerce.orders().ship(order.id, Some("TRACK123456"))?;
    println!("   Order shipped with tracking: {:?}", order.tracking_number);

    // 6. Check final inventory
    println!("\n6. Final inventory check...");
    if let Some(stock) = commerce.inventory().get_stock("WIDGET-001")? {
        println!("   WIDGET-001: {} available (was 100)", stock.total_available);
    }
    if let Some(stock) = commerce.inventory().get_stock("GADGET-001")? {
        println!("   GADGET-001: {} available (was 50)", stock.total_available);
    }

    // 7. Summary
    println!("\n=== Summary ===");
    println!("Customers: {}", commerce.customers().count(Default::default())?);
    println!("Products: {}", commerce.products().count(Default::default())?);
    println!("Orders: {}", commerce.orders().count(Default::default())?);

    println!("\n✓ Example completed successfully!");

    Ok(())
}
