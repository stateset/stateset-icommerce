# stateset-go

Go bindings for StateSet Embedded Commerce via CGo FFI.

## Prerequisites

```bash
# Build the native library first
cargo build -p stateset-go --release
```

## Installation

```go
import "github.com/stateset/stateset-icommerce/bindings/go/stateset"
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
    commerce, err := stateset.New(":memory:")
    if err != nil {
        log.Fatal(err)
    }
    defer commerce.Close()

    // Create a customer
    customer, err := commerce.Customers().Create(
        "alice@example.com", "Alice", "Smith", "+1-555-0123",
    )
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Created customer: %s\n", customer.ID)

    // Create a product
    product, err := commerce.Products().Create(
        "Widget", "WIDGET-001", 29.99, "A quality widget",
    )
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Created product: %s ($%.2f)\n", product.Name, 29.99)

    // Create inventory
    _, err = commerce.Inventory().Create(product.ID, "WIDGET-001", 100)
    if err != nil {
        log.Fatal(err)
    }

    // Create an order
    order, err := commerce.Orders().Create(customer.ID, "USD")
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Created order: %s\n", order.ID)
}
```

## API

### Commerce

```go
commerce, err := stateset.New("store.db")  // SQLite file
commerce, err := stateset.New(":memory:")   // In-memory
defer commerce.Close()
```

### Customers

```go
customer, err := commerce.Customers().Create(email, firstName, lastName, phone)
customer, err := commerce.Customers().Get(id)
customers, err := commerce.Customers().List(limit, offset)
```

### Products

```go
product, err := commerce.Products().Create(name, sku, price, description)
product, err := commerce.Products().Get(id)
products, err := commerce.Products().List(limit, offset)
```

### Orders

```go
order, err := commerce.Orders().Create(customerID, currency)
order, err := commerce.Orders().Get(id)
orders, err := commerce.Orders().List(limit, offset)
```

### Inventory

```go
item, err := commerce.Inventory().Create(productID, sku, quantity)
item, err := commerce.Inventory().Get(id)
err = commerce.Inventory().Adjust(id, delta, reason)
```

### Returns

```go
ret, err := commerce.Returns().Create(orderID, reason)
ret, err := commerce.Returns().Get(id)
err = commerce.Returns().Approve(id)
```

## Running the Example

```bash
# Build native library
cargo build -p stateset-go --release

# Run example
cd bindings/go/example
go run ./
```

## Architecture

The Go binding uses CGo to call the StateSet FFI layer (`libstateset_go`). All commerce data is stored in a local SQLite database, making it zero-dependency and embeddable in any Go application.

```
Go Application
  └── stateset (Go package)
        └── CGo FFI
              └── libstateset_go.so/dylib
                    └── stateset-embedded (Rust)
                          └── SQLite
```

## License

MIT OR Apache-2.0
