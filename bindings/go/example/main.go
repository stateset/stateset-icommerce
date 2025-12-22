// Example usage of StateSet Commerce Go bindings
package main

import (
	"fmt"
	"log"

	"github.com/stateset/stateset-icommerce/bindings/go/stateset"
)

func main() {
	// Create an in-memory commerce instance
	commerce, err := stateset.New(":memory:")
	if err != nil {
		log.Fatalf("Failed to create commerce instance: %v", err)
	}
	defer commerce.Close()

	fmt.Println("StateSet Commerce initialized successfully!")

	// Create a customer
	customer, err := commerce.Customers().Create(
		"alice@example.com",
		"Alice",
		"Smith",
		"+1-555-0123",
	)
	if err != nil {
		log.Fatalf("Failed to create customer: %v", err)
	}
	fmt.Printf("Created customer: %s (%s %s)\n", customer.ID, customer.FirstName, customer.LastName)

	// Create a product
	product, err := commerce.Products().Create(
		"Premium Widget",
		"WIDGET-001",
		99.99,
		"A premium quality widget",
	)
	if err != nil {
		log.Fatalf("Failed to create product: %v", err)
	}
	fmt.Printf("Created product: %s (%s) - $%.2f\n", product.ID, product.Name, 99.99)

	// Create inventory for the product
	invItem, err := commerce.Inventory().CreateItem(
		"WIDGET-001",
		"Premium Widget",
		100.0,
	)
	if err != nil {
		log.Fatalf("Failed to create inventory item: %v", err)
	}
	fmt.Printf("Created inventory item: %d (%s)\n", invItem.ID, invItem.SKU)

	// Create an order
	items := []stateset.OrderItem{
		{
			ProductID: product.ID,
			SKU:       "WIDGET-001",
			Name:      "Premium Widget",
			Quantity:  2,
			UnitPrice: "99.99",
		},
	}
	order, err := commerce.Orders().Create(customer.ID, items, "USD")
	if err != nil {
		log.Fatalf("Failed to create order: %v", err)
	}
	fmt.Printf("Created order: %s (Total: %s %s)\n", order.OrderNumber, order.TotalAmount, order.Currency)

	// Update order status
	order, err = commerce.Orders().UpdateStatus(order.ID, stateset.OrderStatusConfirmed)
	if err != nil {
		log.Fatalf("Failed to update order status: %v", err)
	}
	fmt.Printf("Order status updated to: %s\n", order.Status)

	// Note: Payment creation requires additional setup in the core
	// Skipping payment demo for basic example
	fmt.Printf("Order ready for payment processing\n")

	// Get sales summary
	summary, err := commerce.Analytics().GetSalesSummary(stateset.TimePeriodToday)
	if err != nil {
		log.Fatalf("Failed to get sales summary: %v", err)
	}
	fmt.Printf("Sales summary - Revenue: %s, Orders: %d, AOV: %s\n",
		summary.TotalRevenue, summary.OrderCount, summary.AverageOrderValue)

	// List all customers
	customers, err := commerce.Customers().List()
	if err != nil {
		log.Fatalf("Failed to list customers: %v", err)
	}
	fmt.Printf("Total customers: %d\n", len(customers))

	// List all orders
	orders, err := commerce.Orders().List()
	if err != nil {
		log.Fatalf("Failed to list orders: %v", err)
	}
	fmt.Printf("Total orders: %d\n", len(orders))

	fmt.Println("\nStateSet Commerce Go bindings working correctly!")
}
