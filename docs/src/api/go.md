# Go API Reference

The Go binding provides `stateset.New` and the `Commerce` handle for building commerce applications.

## Installation

```bash
go get github.com/stateset/stateset-icommerce/bindings/go/stateset@v1.30.0
```

## Quick Start

```go
package main

import (
    "fmt"
    "log"

    "github.com/stateset/stateset-icommerce/bindings/go/stateset"
)

func main() {
    // Initialize with SQLite database
    commerce, err := stateset.New("commerce.db")
    if err != nil {
        log.Fatal(err)
    }
    defer commerce.Close()

    // Create a customer
    customer, err := commerce.Customers().Create(stateset.CreateCustomer{
        Email:     "alice@example.com",
        FirstName: "Alice",
        LastName:  "Smith",
        Phone:     "+1-555-0123",
    })
    if err != nil {
        log.Fatal(err)
    }

    // Create a product
    product, err := commerce.Products().Create(stateset.CreateProduct{
        Name:        "Premium Widget",
        SKU:         "WIDGET-001",
        Price:       29.99,
        Description: "High-quality widget",
    })
    if err != nil {
        log.Fatal(err)
    }

    // Create inventory
    item, err := commerce.Inventory().CreateItem(stateset.CreateInventoryItem{
        SKU:             "WIDGET-001",
        Name:            "Premium Widget",
        InitialQuantity: 100,
    })
    if err != nil {
        log.Fatal(err)
    }

    // Create an order
    order, err := commerce.Orders().Create(stateset.CreateOrder{
        CustomerID: customer.ID,
        Items: []stateset.OrderItem{
            {SKU: "WIDGET-001", Name: "Widget", Quantity: 2, UnitPrice: 29.99},
        },
        Currency: "USD",
    })
    if err != nil {
        log.Fatal(err)
    }

    // Ship the order
    shipped, err := commerce.Orders().Ship(order.ID)
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Order %s shipped!\n", shipped.OrderNumber)
}
```

## Common Operations

### Customer Management

```go
// Create customer
customer, err := commerce.Customers().Create(stateset.CreateCustomer{
    Email:     "test@example.com",
    FirstName: "Test",
    LastName:  "User",
})

// Get customer by ID
found, err := commerce.Customers().Get(customerID)

// List all customers
customers, err := commerce.Customers().List()

// Delete customer
deleted, err := commerce.Customers().Delete(customerID)
```

### Inventory Management

```go
// Create inventory item
item, err := commerce.Inventory().CreateItem(stateset.CreateInventoryItem{
    SKU:             "SKU-001",
    Name:            "Widget",
    InitialQuantity: 100,
})

// Adjust inventory
err = commerce.Inventory().Adjust("SKU-001", 50, "Received shipment")

// Reserve inventory
reservation, err := commerce.Inventory().Reserve("SKU-001", 10, nil)

// Release reservation
err = commerce.Inventory().Release(reservation.ID)

// Get stock level
level, err := commerce.Inventory().GetLevel("SKU-001")
fmt.Printf("Available: %d\n", level.Available)
```

### Order Processing

```go
// Create order
order, err := commerce.Orders().Create(stateset.CreateOrder{
    CustomerID: customer.ID,
    Items: []stateset.OrderItem{
        {SKU: "SKU-001", Name: "Widget", Quantity: 2, UnitPrice: 29.99},
    },
})

// Update status
err = commerce.Orders().UpdateStatus(order.ID, "processing")

// Ship order
shipped, err := commerce.Orders().Ship(order.ID)

// Cancel order
cancelled, err := commerce.Orders().Cancel(order.ID)

// List orders by status
pending, err := commerce.Orders().ListByStatus("pending")
```

### Subscriptions

```go
// Create a subscription plan
plan, err := commerce.Subscriptions().CreatePlan(stateset.CreatePlan{
    Code:          "PREMIUM",
    Name:          "Premium Plan",
    Interval:      "month",
    IntervalCount: 1,
    Price:         19.99,
    Currency:      "USD",
})

// Subscribe a customer
subscription, err := commerce.Subscriptions().Subscribe(customer.ID, plan.ID)

// Pause/Resume/Cancel
paused, err := commerce.Subscriptions().Pause(subscription.ID)
resumed, err := commerce.Subscriptions().Resume(subscription.ID)
cancelled, err := commerce.Subscriptions().Cancel(subscription.ID)
```

### Analytics

```go
// Get sales summary
summary, err := commerce.Analytics().SalesSummary()
fmt.Printf("Total revenue: %s\n", summary.TotalRevenue)

// Get top products
topProducts, err := commerce.Analytics().TopProducts(10)

// Get top customers
topCustomers, err := commerce.Analytics().TopCustomers(10)
```

## Error Handling

```go
order, err := commerce.Orders().Ship(orderID)
if err != nil {
    if errors.Is(err, stateset.ErrNotFound) {
        fmt.Println("Order not found")
    } else if errors.Is(err, stateset.ErrInvalidState) {
        fmt.Println("Cannot ship order in current state")
    } else {
        return fmt.Errorf("failed to ship order: %w", err)
    }
}
```

## Available APIs

| API | Description |
|-----|-------------|
| `Customers()` | Customer management |
| `Products()` | Product catalog |
| `Orders()` | Order lifecycle |
| `Inventory()` | Stock management |
| `Carts()` | Shopping carts |
| `Returns()` | Return processing |
| `Payments()` | Payment operations |
| `Shipments()` | Shipping management |
| `Warranties()` | Warranty tracking |
| `Suppliers()` | Supplier management |
| `PurchaseOrders()` | Purchase orders |
| `Invoices()` | B2B invoicing |
| `BOM()` | Bills of Materials |
| `WorkOrders()` | Manufacturing |
| `Currency()` | Multi-currency |
| `Subscriptions()` | Recurring billing |
| `Promotions()` | Discounts & coupons |
| `Tax()` | Tax calculations |
| `Quality()` | Quality control |
| `Lots()` | Lot tracking |
| `Serials()` | Serial numbers |
| `Warehouse()` | Warehouse ops |
| `Receiving()` | Receiving |
| `Fulfillment()` | Picking & packing |
| `AccountsPayable()` | A/P management |
| `AccountsReceivable()` | A/R management |
| `CostAccounting()` | Cost tracking |
| `Credit()` | Credit management |
| `Backorders()` | Backorder tracking |
| `GeneralLedger()` | GL accounting |
| `Analytics()` | Reporting & forecasts |

## Platform Support

| Platform | Architectures | Status |
|----------|---------------|--------|
| Linux | x64, arm64 | Supported |
| macOS | x64, arm64 | Supported |
| Windows | x64 | Supported |

## Source Files

- Entry point: `stateset.New`
- Sources: `bindings/go/stateset/`

## Examples

- `examples/go/basic_usage.go`
