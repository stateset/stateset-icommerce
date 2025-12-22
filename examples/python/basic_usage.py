#!/usr/bin/env python3
"""
StateSet iCommerce - Python Example

Demonstrates the full commerce workflow:
- Customer creation
- Product catalog management
- Inventory tracking
- Order processing
- Analytics

Run with: python basic_usage.py
"""

from stateset import Commerce


def main():
    print("=== StateSet iCommerce - Python Example ===\n")

    # Initialize commerce with in-memory database
    commerce = Commerce(":memory:")
    print("✓ Commerce initialized\n")

    # 1. Create a customer
    print("1. Creating customer...")
    customer = commerce.customers.create(
        email="alice@example.com",
        first_name="Alice",
        last_name="Smith",
        phone="+1-555-0123"
    )
    print(f"   Created customer: {customer.first_name} {customer.last_name} ({customer.email})")

    # 2. Create products
    print("\n2. Creating products...")
    widget = commerce.products.create(
        name="Premium Widget",
        sku="WIDGET-001",
        price=29.99,
        description="A high-quality widget for all your needs"
    )
    print(f"   Created product: {widget.name} ({widget.slug})")

    gadget = commerce.products.create(
        name="Super Gadget",
        sku="GADGET-001",
        price=49.99,
        description="An amazing gadget"
    )
    print(f"   Created product: {gadget.name} ({gadget.slug})")

    # 3. Create inventory
    print("\n3. Setting up inventory...")
    commerce.inventory.create_item(
        sku="WIDGET-001",
        name="Premium Widget",
        initial_quantity=100
    )
    print("   Created inventory for WIDGET-001 (100 units)")

    commerce.inventory.create_item(
        sku="GADGET-001",
        name="Super Gadget",
        initial_quantity=50
    )
    print("   Created inventory for GADGET-001 (50 units)")

    # Check stock
    widget_stock = commerce.inventory.get_stock("WIDGET-001")
    print(f"   Stock check WIDGET-001: {widget_stock.total_available} available")

    # 4. Create an order
    print("\n4. Creating order...")
    order = commerce.orders.create(
        customer_id=customer.id,
        items=[
            {
                "product_id": widget.id,
                "sku": "WIDGET-001",
                "name": "Premium Widget",
                "quantity": 2,
                "unit_price": "29.99"
            },
            {
                "product_id": gadget.id,
                "sku": "GADGET-001",
                "name": "Super Gadget",
                "quantity": 1,
                "unit_price": "49.99"
            }
        ],
        currency="USD"
    )
    print(f"   Created order {order.order_number} (total: ${order.total_amount})")

    # 5. Process the order
    print("\n5. Processing order...")

    # Update order status
    order = commerce.orders.update_status(order.id, "confirmed")
    print(f"   Order status: {order.status}")

    # Adjust inventory (fulfill)
    commerce.inventory.adjust("WIDGET-001", -2, "Order fulfillment")
    commerce.inventory.adjust("GADGET-001", -1, "Order fulfillment")
    print("   Inventory adjusted")

    # Ship the order
    order = commerce.orders.ship(order.id, "TRACK123456")
    print(f"   Order shipped with tracking: {order.tracking_number}")

    # 6. Check final inventory
    print("\n6. Final inventory check...")
    final_widget_stock = commerce.inventory.get_stock("WIDGET-001")
    print(f"   WIDGET-001: {final_widget_stock.total_available} available (was 100)")

    final_gadget_stock = commerce.inventory.get_stock("GADGET-001")
    print(f"   GADGET-001: {final_gadget_stock.total_available} available (was 50)")

    # 7. Analytics
    print("\n7. Analytics...")
    sales_summary = commerce.analytics.sales_summary(period="today")
    print(f"   Revenue: ${sales_summary.total_revenue}")
    print(f"   Orders: {sales_summary.order_count}")
    print(f"   AOV: ${sales_summary.average_order_value}")

    # Get top products
    top_products = commerce.analytics.top_products(limit=5)
    print(f"   Top products: {len(top_products)}")

    # 8. Demand forecasting
    print("\n8. Demand forecasting...")
    forecasts = commerce.analytics.demand_forecast(days_ahead=30)
    for forecast in forecasts:
        if forecast.days_until_stockout and forecast.days_until_stockout < 14:
            print(f"   WARNING: {forecast.sku} will stock out in {forecast.days_until_stockout} days")
        else:
            print(f"   {forecast.sku}: {forecast.predicted_demand} units predicted")

    # 9. Summary
    print("\n=== Summary ===")
    customers_list = commerce.customers.list()
    products_list = commerce.products.list()
    orders_list = commerce.orders.list()
    print(f"Customers: {len(customers_list)}")
    print(f"Products: {len(products_list)}")
    print(f"Orders: {len(orders_list)}")

    print("\n✓ Example completed successfully!")


if __name__ == "__main__":
    main()
