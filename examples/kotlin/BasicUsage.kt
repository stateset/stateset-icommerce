/**
 * StateSet iCommerce - Kotlin Example
 *
 * Demonstrates the full commerce workflow:
 * - Customer creation
 * - Product catalog management
 * - Inventory tracking
 * - Order processing
 * - Analytics
 *
 * Run with: ./gradlew run
 * Or build a jar: ./gradlew jar && java -jar build/libs/kotlin-1.30.0.jar
 */
package com.stateset.examples

import com.stateset.embedded.*

fun main() {
    println("=== StateSet iCommerce - Kotlin Example ===")
    println()

    // Initialize commerce with in-memory database
    val commerce = StateSetCommerce(":memory:")
    println("✓ Commerce initialized")
    println()

    try {
        // 1. Create a customer
        println("1. Creating customer...")
        val customer = commerce.customers.create(
            email = "alice@example.com",
            firstName = "Alice",
            lastName = "Smith",
            phone = "+1-555-0123"
        )
        println("   Created customer: ${customer.firstName} ${customer.lastName} (${customer.email})")

        // 2. Create products
        println()
        println("2. Creating products...")
        val widget = commerce.products.create(
            name = "Premium Widget",
            sku = "WIDGET-001",
            price = 29.99,
            description = "A high-quality widget for all your needs"
        )
        println("   Created product: ${widget.name} (${widget.id})")

        val gadget = commerce.products.create(
            name = "Super Gadget",
            sku = "GADGET-001",
            price = 49.99,
            description = "An amazing gadget"
        )
        println("   Created product: ${gadget.name} (${gadget.id})")

        // 3. Create inventory
        println()
        println("3. Setting up inventory...")
        commerce.inventory.createItem(
            sku = "WIDGET-001",
            name = "Premium Widget",
            initialQuantity = 100.0
        )
        println("   Created inventory for WIDGET-001 (100 units)")

        commerce.inventory.createItem(
            sku = "GADGET-001",
            name = "Super Gadget",
            initialQuantity = 50.0
        )
        println("   Created inventory for GADGET-001 (50 units)")

        // Check stock
        val widgetStock = commerce.inventory.getStock("WIDGET-001")
        println("   Stock check WIDGET-001: ${widgetStock?.totalAvailable} available")

        // 4. Create an order
        println()
        println("4. Creating order...")
        val order = commerce.orders.create(
            customerId = customer.id,
            items = listOf(
                OrderItem(
                    productId = widget.id,
                    sku = "WIDGET-001",
                    name = "Premium Widget",
                    quantity = 2,
                    unitPrice = "29.99"
                ),
                OrderItem(
                    productId = gadget.id,
                    sku = "GADGET-001",
                    name = "Super Gadget",
                    quantity = 1,
                    unitPrice = "49.99"
                )
            ),
            currency = "USD"
        )
        println("   Created order ${order.orderNumber} (total: $${order.totalAmount})")

        // 5. Process the order
        println()
        println("5. Processing order...")

        // Update order status
        var updatedOrder = commerce.orders.updateStatus(order.id, OrderStatus.CONFIRMED)
        println("   Order status: ${updatedOrder.status}")

        // Adjust inventory (fulfill)
        commerce.inventory.adjust("WIDGET-001", -2.0, "Order fulfillment")
        commerce.inventory.adjust("GADGET-001", -1.0, "Order fulfillment")
        println("   Inventory adjusted")

        // Ship the order
        updatedOrder = commerce.orders.updateStatus(order.id, OrderStatus.SHIPPED)
        println("   Order shipped (status: ${updatedOrder.status})")

        // 6. Check final inventory
        println()
        println("6. Final inventory check...")
        val finalWidgetStock = commerce.inventory.getStock("WIDGET-001")
        println("   WIDGET-001: ${finalWidgetStock?.totalAvailable} available (was 100)")

        val finalGadgetStock = commerce.inventory.getStock("GADGET-001")
        println("   GADGET-001: ${finalGadgetStock?.totalAvailable} available (was 50)")

        // 7. Analytics
        println()
        println("7. Analytics...")
        val salesSummary = commerce.analytics.getSalesSummary(TimePeriod.TODAY)
        println("   Revenue: $${salesSummary.totalRevenue}")
        println("   Orders: ${salesSummary.orderCount}")
        println("   AOV: $${salesSummary.averageOrderValue}")

        // Get top products
        val topProducts = commerce.analytics.getTopProducts(TimePeriod.MONTH, 5)
        println("   Top products: ${topProducts.size}")

        // 8. Summary
        println()
        println("=== Summary ===")
        val customers = commerce.customers.list()
        val products = commerce.products.list()
        val orders = commerce.orders.list()
        println("Customers: ${customers.size}")
        println("Products: ${products.size}")
        println("Orders: ${orders.size}")

        println()
        println("✓ Example completed successfully!")

    } finally {
        commerce.close()
    }
}
