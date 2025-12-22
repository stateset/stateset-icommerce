// StateSet iCommerce - Go Example
//
// Demonstrates the full commerce workflow:
// - Customer creation
// - Product catalog management
// - Inventory tracking
// - Order processing
// - Analytics
//
// Run with: go run basic_usage.go
package main

import (
	"fmt"
	"log"

	"github.com/stateset/stateset-icommerce/bindings/go/stateset"
)

func main() {
	fmt.Println("=== StateSet iCommerce - Go Example ===")
	fmt.Println()

	// Initialize commerce with in-memory database
	commerce, err := stateset.New(":memory:")
	if err != nil {
		log.Fatalf("Failed to initialize commerce: %v", err)
	}
	defer commerce.Close()
	fmt.Println("✓ Commerce initialized")
	fmt.Println()

	// 1. Create a customer
	fmt.Println("1. Creating customer...")
	customer, err := commerce.Customers().Create(
		"alice@example.com",
		"Alice",
		"Smith",
		"+1-555-0123",
	)
	if err != nil {
		log.Fatalf("Failed to create customer: %v", err)
	}
	fmt.Printf("   Created customer: %s %s (%s)\n", customer.FirstName, customer.LastName, customer.Email)

	// 2. Create products
	fmt.Println()
	fmt.Println("2. Creating products...")
	widget, err := commerce.Products().Create(
		"Premium Widget",
		"WIDGET-001",
		29.99,
		"A high-quality widget for all your needs",
	)
	if err != nil {
		log.Fatalf("Failed to create widget: %v", err)
	}
	fmt.Printf("   Created product: %s (%s)\n", widget.Name, widget.ID)

	gadget, err := commerce.Products().Create(
		"Super Gadget",
		"GADGET-001",
		49.99,
		"An amazing gadget",
	)
	if err != nil {
		log.Fatalf("Failed to create gadget: %v", err)
	}
	fmt.Printf("   Created product: %s (%s)\n", gadget.Name, gadget.ID)

	// 3. Create inventory
	fmt.Println()
	fmt.Println("3. Setting up inventory...")
	_, err = commerce.Inventory().CreateItem("WIDGET-001", "Premium Widget", 100)
	if err != nil {
		log.Fatalf("Failed to create widget inventory: %v", err)
	}
	fmt.Println("   Created inventory for WIDGET-001 (100 units)")

	_, err = commerce.Inventory().CreateItem("GADGET-001", "Super Gadget", 50)
	if err != nil {
		log.Fatalf("Failed to create gadget inventory: %v", err)
	}
	fmt.Println("   Created inventory for GADGET-001 (50 units)")

	// Check stock
	widgetStock, err := commerce.Inventory().GetLevel("WIDGET-001")
	if err != nil {
		log.Fatalf("Failed to get widget stock: %v", err)
	}
	if widgetStock != nil {
		fmt.Printf("   Stock check WIDGET-001: %s available\n", widgetStock.TotalAvailable)
	}

	// 4. Create an order
	fmt.Println()
	fmt.Println("4. Creating order...")
	items := []stateset.OrderItem{
		{
			ProductID: widget.ID,
			SKU:       "WIDGET-001",
			Name:      "Premium Widget",
			Quantity:  2,
			UnitPrice: "29.99",
		},
		{
			ProductID: gadget.ID,
			SKU:       "GADGET-001",
			Name:      "Super Gadget",
			Quantity:  1,
			UnitPrice: "49.99",
		},
	}
	order, err := commerce.Orders().Create(customer.ID, items, "USD")
	if err != nil {
		log.Fatalf("Failed to create order: %v", err)
	}
	fmt.Printf("   Created order %s (total: $%s)\n", order.OrderNumber, order.TotalAmount)

	// 5. Process the order
	fmt.Println()
	fmt.Println("5. Processing order...")

	// Update order status
	order, err = commerce.Orders().UpdateStatus(order.ID, stateset.OrderStatusConfirmed)
	if err != nil {
		log.Fatalf("Failed to update order status: %v", err)
	}
	fmt.Printf("   Order status: %s\n", order.Status)

	// Adjust inventory (fulfill)
	if !commerce.Inventory().Adjust("WIDGET-001", -2, "Order fulfillment") {
		log.Fatalf("Failed to adjust widget inventory")
	}
	if !commerce.Inventory().Adjust("GADGET-001", -1, "Order fulfillment") {
		log.Fatalf("Failed to adjust gadget inventory")
	}
	fmt.Println("   Inventory adjusted")

	// Ship the order
	order, err = commerce.Orders().UpdateStatus(order.ID, stateset.OrderStatusShipped)
	if err != nil {
		log.Fatalf("Failed to ship order: %v", err)
	}
	fmt.Printf("   Order shipped (status: %s)\n", order.Status)

	// 6. Check final inventory
	fmt.Println()
	fmt.Println("6. Final inventory check...")
	finalWidgetStock, err := commerce.Inventory().GetLevel("WIDGET-001")
	if err != nil {
		log.Fatalf("Failed to get final widget stock: %v", err)
	}
	if finalWidgetStock != nil {
		fmt.Printf("   WIDGET-001: %s available (was 100)\n", finalWidgetStock.TotalAvailable)
	}

	finalGadgetStock, err := commerce.Inventory().GetLevel("GADGET-001")
	if err != nil {
		log.Fatalf("Failed to get final gadget stock: %v", err)
	}
	if finalGadgetStock != nil {
		fmt.Printf("   GADGET-001: %s available (was 50)\n", finalGadgetStock.TotalAvailable)
	}

	// 7. Analytics
	fmt.Println()
	fmt.Println("7. Analytics...")
	salesSummary, err := commerce.Analytics().GetSalesSummary(stateset.TimePeriodToday)
	if err != nil {
		log.Fatalf("Failed to get sales summary: %v", err)
	}
	fmt.Printf("   Revenue: $%s\n", salesSummary.TotalRevenue)
	fmt.Printf("   Orders: %d\n", salesSummary.OrderCount)
	fmt.Printf("   AOV: $%s\n", salesSummary.AverageOrderValue)

	// 8. Summary
	fmt.Println()
	fmt.Println("=== Summary ===")
	customers, _ := commerce.Customers().List()
	products, _ := commerce.Products().List()
	orders, _ := commerce.Orders().List()
	fmt.Printf("Customers: %d\n", len(customers))
	fmt.Printf("Products: %d\n", len(products))
	fmt.Printf("Orders: %d\n", len(orders))

	fmt.Println()
	fmt.Println("✓ Example completed successfully!")
}
