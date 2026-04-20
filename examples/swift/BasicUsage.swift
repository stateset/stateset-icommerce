/**
 * StateSet iCommerce - Swift Example
 *
 * Demonstrates the full commerce workflow:
 * - Customer creation
 * - Product catalog management
 * - Inventory tracking
 * - Order processing
 * - Analytics
 *
 * Run with: swift run
 */

import Foundation
import StateSet

func main() throws {
    print("=== StateSet iCommerce - Swift Example ===")
    print()

    // Initialize commerce with in-memory database
    let commerce = try StateSetCommerce(path: ":memory:")
    defer { commerce.close() }
    print("✓ Commerce initialized")
    print()

    // 1. Create a customer
    print("1. Creating customer...")
    let customer = try commerce.customers.create(
        email: "alice@example.com",
        firstName: "Alice",
        lastName: "Smith",
        phone: "+1-555-0123"
    )
    print("   Created customer: \(customer.firstName) \(customer.lastName) (\(customer.email))")

    // 2. Create products
    print()
    print("2. Creating products...")
    let widget = try commerce.products.create(
        name: "Premium Widget",
        sku: "WIDGET-001",
        price: 29.99,
        description: "A high-quality widget for all your needs"
    )
    print("   Created product: \(widget.name) (\(widget.id))")

    let gadget = try commerce.products.create(
        name: "Super Gadget",
        sku: "GADGET-001",
        price: 49.99,
        description: "An amazing gadget"
    )
    print("   Created product: \(gadget.name) (\(gadget.id))")

    // 3. Create inventory
    print()
    print("3. Setting up inventory...")
    try commerce.inventory.createItem(
        sku: "WIDGET-001",
        name: "Premium Widget",
        initialQuantity: 100
    )
    print("   Created inventory for WIDGET-001 (100 units)")

    try commerce.inventory.createItem(
        sku: "GADGET-001",
        name: "Super Gadget",
        initialQuantity: 50
    )
    print("   Created inventory for GADGET-001 (50 units)")

    // Check stock
    if let widgetStock = try commerce.inventory.getStock(sku: "WIDGET-001") {
        print("   Stock check WIDGET-001: \(widgetStock.totalAvailable) available")
    }

    // 4. Create an order
    print()
    print("4. Creating order...")
    let order = try commerce.orders.create(
        customerId: customer.id,
        items: [
            OrderItem(
                productId: widget.id,
                sku: "WIDGET-001",
                name: "Premium Widget",
                quantity: 2,
                unitPrice: "29.99"
            ),
            OrderItem(
                productId: gadget.id,
                sku: "GADGET-001",
                name: "Super Gadget",
                quantity: 1,
                unitPrice: "49.99"
            )
        ],
        currency: "USD"
    )
    print("   Created order \(order.orderNumber) (total: $\(order.totalAmount))")

    // 5. Process the order
    print()
    print("5. Processing order...")

    // Update order status
    var updatedOrder = try commerce.orders.updateStatus(id: order.id, status: .confirmed)
    print("   Order status: \(updatedOrder.status)")

    // Adjust inventory (fulfill)
    try commerce.inventory.adjust(sku: "WIDGET-001", delta: -2, reason: "Order fulfillment")
    try commerce.inventory.adjust(sku: "GADGET-001", delta: -1, reason: "Order fulfillment")
    print("   Inventory adjusted")

    // Ship the order
    updatedOrder = try commerce.orders.updateStatus(id: order.id, status: .shipped)
    print("   Order shipped (status: \(updatedOrder.status))")

    // 6. Check final inventory
    print()
    print("6. Final inventory check...")
    if let finalWidgetStock = try commerce.inventory.getStock(sku: "WIDGET-001") {
        print("   WIDGET-001: \(finalWidgetStock.totalAvailable) available (was 100)")
    }
    if let finalGadgetStock = try commerce.inventory.getStock(sku: "GADGET-001") {
        print("   GADGET-001: \(finalGadgetStock.totalAvailable) available (was 50)")
    }

    // 7. Analytics
    print()
    print("7. Analytics...")
    let salesSummary = try commerce.analytics.getSalesSummary(period: .today)
    print("   Revenue: $\(salesSummary.totalRevenue)")
    print("   Orders: \(salesSummary.orderCount)")
    print("   AOV: $\(salesSummary.averageOrderValue)")

    // Get top products
    let topProducts = try commerce.analytics.getTopProducts(period: .month, limit: 5)
    print("   Top products: \(topProducts.count)")

    // 8. Summary
    print()
    print("=== Summary ===")
    let customers = try commerce.customers.list()
    let products = try commerce.products.list()
    let orders = try commerce.orders.list()
    print("Customers: \(customers.count)")
    print("Products: \(products.count)")
    print("Orders: \(orders.count)")

    print()
    print("✓ Example completed successfully!")
}

// Run the example
do {
    try main()
} catch {
    print("Error: \(error)")
    exit(1)
}
