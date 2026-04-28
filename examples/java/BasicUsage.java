/**
 * StateSet iCommerce - Java Example
 *
 * Demonstrates the full commerce workflow:
 * - Customer creation
 * - Product catalog management
 * - Inventory tracking
 * - Order processing
 * - Analytics
 *
 * Compile with: javac -d . -cp stateset-embedded-1.0.0.jar BasicUsage.java
 * Run with: java -cp .:stateset-embedded-1.0.0.jar com.stateset.examples.BasicUsage
 */

package com.stateset.examples;

import com.stateset.embedded.*;
import java.util.Arrays;
import java.util.List;

public class BasicUsage {
    public static void main(String[] args) {
        System.out.println("=== StateSet iCommerce - Java Example ===");
        System.out.println();

        // Initialize commerce with in-memory database
        try (Commerce commerce = new Commerce(":memory:")) {
            System.out.println("✓ Commerce initialized");
            System.out.println();

            // 1. Create a customer
            System.out.println("1. Creating customer...");
            Customer customer = commerce.customers().create(
                "alice@example.com",
                "Alice",
                "Smith",
                "+1-555-0123"
            );
            System.out.printf("   Created customer: %s %s (%s)%n",
                customer.getFirstName(), customer.getLastName(), customer.getEmail());

            // 2. Create products
            System.out.println();
            System.out.println("2. Creating products...");
            Product widget = commerce.products().create(
                "Premium Widget",
                "WIDGET-001",
                29.99,
                "A high-quality widget for all your needs"
            );
            System.out.printf("   Created product: %s (%s)%n", widget.getName(), widget.getId());

            Product gadget = commerce.products().create(
                "Super Gadget",
                "GADGET-001",
                49.99,
                "An amazing gadget"
            );
            System.out.printf("   Created product: %s (%s)%n", gadget.getName(), gadget.getId());

            // 3. Create inventory
            System.out.println();
            System.out.println("3. Setting up inventory...");
            commerce.inventory().createItem("WIDGET-001", "Premium Widget", 100);
            System.out.println("   Created inventory for WIDGET-001 (100 units)");

            commerce.inventory().createItem("GADGET-001", "Super Gadget", 50);
            System.out.println("   Created inventory for GADGET-001 (50 units)");

            // Check stock
            StockLevel widgetStock = commerce.inventory().getStock("WIDGET-001");
            System.out.printf("   Stock check WIDGET-001: %s available%n", widgetStock.getTotalAvailable());

            // 4. Create an order
            System.out.println();
            System.out.println("4. Creating order...");
            List<OrderItem> items = Arrays.asList(
                new OrderItem(widget.getId(), "WIDGET-001", "Premium Widget", 2, "29.99"),
                new OrderItem(gadget.getId(), "GADGET-001", "Super Gadget", 1, "49.99")
            );
            Order order = commerce.orders().create(customer.getId(), items, "USD");
            System.out.printf("   Created order %s (total: $%s)%n",
                order.getOrderNumber(), order.getTotalAmount());

            // 5. Process the order
            System.out.println();
            System.out.println("5. Processing order...");

            // Update order status
            order = commerce.orders().updateStatus(order.getId(), OrderStatus.CONFIRMED);
            System.out.printf("   Order status: %s%n", order.getStatus());

            // Adjust inventory (fulfill)
            commerce.inventory().adjust("WIDGET-001", -2, "Order fulfillment");
            commerce.inventory().adjust("GADGET-001", -1, "Order fulfillment");
            System.out.println("   Inventory adjusted");

            // Ship the order
            order = commerce.orders().ship(order.getId(), "TRACK123456");
            System.out.printf("   Order shipped with tracking: %s%n", order.getTrackingNumber());

            // 6. Check final inventory
            System.out.println();
            System.out.println("6. Final inventory check...");
            StockLevel finalWidgetStock = commerce.inventory().getStock("WIDGET-001");
            System.out.printf("   WIDGET-001: %s available (was 100)%n", finalWidgetStock.getTotalAvailable());

            StockLevel finalGadgetStock = commerce.inventory().getStock("GADGET-001");
            System.out.printf("   GADGET-001: %s available (was 50)%n", finalGadgetStock.getTotalAvailable());

            // 7. Analytics
            System.out.println();
            System.out.println("7. Analytics...");
            SalesSummary salesSummary = commerce.analytics().salesSummary(30);
            System.out.printf("   Revenue: $%s%n", salesSummary.getTotalRevenue());
            System.out.printf("   Orders: %d%n", salesSummary.getOrderCount());
            System.out.printf("   AOV: $%s%n", salesSummary.getAverageOrderValue());

            // Get top products
            List<TopProduct> topProducts = commerce.analytics().topProducts(5);
            System.out.printf("   Top products: %d%n", topProducts.size());

            // 8. Summary
            System.out.println();
            System.out.println("=== Summary ===");
            List<Customer> customers = commerce.customers().list();
            List<Product> products = commerce.products().list();
            List<Order> orders = commerce.orders().list();
            System.out.printf("Customers: %d%n", customers.size());
            System.out.printf("Products: %d%n", products.size());
            System.out.printf("Orders: %d%n", orders.size());

            System.out.println();
            System.out.println("✓ Example completed successfully!");

        } catch (Exception e) {
            System.err.println("Error: " + e.getMessage());
            e.printStackTrace();
        }
    }
}
