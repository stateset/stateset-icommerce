# StateSet Embedded Commerce - Java

Local-first commerce engine for Java applications. Native JNI bindings to the Rust commerce core.

## Installation

### Maven

```xml
<dependency>
    <groupId>com.stateset</groupId>
    <artifactId>embedded</artifactId>
    <version>0.1.6</version>
</dependency>
```

### Gradle

```groovy
implementation 'com.stateset:embedded:0.1.6'
```

### Gradle (Kotlin DSL)

```kotlin
implementation("com.stateset:embedded:0.1.6")
```

## Quick Start

```java
import com.stateset.embedded.*;

public class Example {
    public static void main(String[] args) {
        // Initialize commerce engine (creates/opens SQLite database)
        try (Commerce commerce = new Commerce("commerce.db")) {

            // Create a customer
            Customer customer = commerce.customers().create(
                "alice@example.com",
                "Alice",
                "Smith"
            );
            System.out.println("Created customer: " + customer.getId());

            // Create a product
            Product product = commerce.products().create(
                "WIDGET-001",
                "Premium Widget",
                29.99
            );
            System.out.println("Created product: " + product.getSku());

            // Add inventory
            InventoryItem item = commerce.inventory().adjust(
                product.getSku(),
                100,
                "Initial stock"
            );
            System.out.println("Stock level: " + item.getQuantityAvailable());

            // Create a shopping cart
            Cart cart = commerce.carts().create(customer.getId(), "USD");
            cart = commerce.carts().addItem(
                cart.getId(),
                product.getSku(),
                product.getName(),
                2,
                product.getBasePrice()
            );

            // Checkout
            Order order = commerce.carts().checkout(cart.getId());
            System.out.println("Order created: " + order.getId());

            // Process payment
            commerce.payments().recordPayment(
                order.getId(),
                order.getTotalAmount(),
                "card",
                "txn_123456"
            );

            // Get sales analytics
            SalesSummary summary = commerce.analytics().salesSummary(30);
            System.out.println("Revenue (30 days): $" + summary.getTotalRevenue());

        } catch (StateSetException e) {
            System.err.println("Commerce error: " + e.getMessage());
        }
    }
}
```

## API Reference

### Commerce

The main entry point. Implements `AutoCloseable` for automatic resource management.

```java
// Initialize with database path
Commerce commerce = new Commerce("commerce.db");

// Access APIs
commerce.customers()   // Customer management
commerce.products()    // Product catalog
commerce.inventory()   // Inventory tracking
commerce.orders()      // Order management
commerce.carts()       // Shopping carts
commerce.payments()    // Payment processing
commerce.returns()     // Returns/refunds
commerce.analytics()   // Sales analytics

// Always close when done
commerce.close();
```

### Customers

```java
// Create a customer
Customer customer = commerce.customers().create(
    "email@example.com",
    "FirstName",
    "LastName"
);

// Get by ID
Optional<Customer> found = commerce.customers().get(customer.getId());

// List all customers
List<Customer> customers = commerce.customers().list();
```

### Products

```java
// Create a product
Product product = commerce.products().create(
    "SKU-001",
    "Product Name",
    99.99
);

// Get by SKU
Optional<Product> found = commerce.products().get("SKU-001");

// List all products
List<Product> products = commerce.products().list();
```

### Inventory

```java
// Adjust inventory (positive or negative)
InventoryItem item = commerce.inventory().adjust(
    "SKU-001",
    50,      // quantity change
    "Restock from supplier"
);

// Check stock level
Optional<InventoryItem> stock = commerce.inventory().get("SKU-001");
stock.ifPresent(i -> {
    System.out.println("Available: " + i.getQuantityAvailable());
    System.out.println("Reserved: " + i.getQuantityReserved());
});

// Reserve stock (for pending orders)
commerce.inventory().reserve("SKU-001", 5);

// Release reserved stock
commerce.inventory().release("SKU-001", 5);
```

### Orders

```java
// Create an order directly
Order order = commerce.orders().create(customer.getId(), "USD");

// Get by ID
Optional<Order> found = commerce.orders().get(order.getId());

// List all orders
List<Order> orders = commerce.orders().list();

// Update order status
commerce.orders().updateStatus(order.getId(), "shipped");
```

### Shopping Carts

```java
// Create a cart (with optional customer ID and currency)
Cart cart = commerce.carts().create(customerId, "USD");

// Add items
cart = commerce.carts().addItem(
    cart.getId(),
    "SKU-001",
    "Product Name",
    2,          // quantity
    29.99       // unit price
);

// Get cart
Optional<Cart> found = commerce.carts().get(cart.getId());

// Checkout (converts cart to order)
Order order = commerce.carts().checkout(cart.getId());
```

### Payments

```java
// Record a payment
commerce.payments().recordPayment(
    orderId,
    amount,
    "card",         // payment method
    "txn_123456"    // external reference
);
```

### Returns

```java
// Create a return request
ReturnRequest returnReq = commerce.returns().create(
    orderId,
    "Product arrived damaged",
    Arrays.asList("item1", "item2")  // item IDs
);

// Process the return
commerce.returns().process(returnReq.getId());

// Issue refund
commerce.returns().refund(returnReq.getId(), 59.98);
```

### Analytics

```java
// Get sales summary for time period
SalesSummary summary = commerce.analytics().salesSummary(30);  // last 30 days
SalesSummary allTime = commerce.analytics().salesSummary(0);   // all time

System.out.println("Orders: " + summary.getTotalOrders());
System.out.println("Revenue: $" + summary.getTotalRevenue());
System.out.println("AOV: $" + summary.getAverageOrderValue());
```

## Framework Integration

### Spring Boot

```java
@Configuration
public class CommerceConfig {

    @Bean(destroyMethod = "close")
    public Commerce commerce() {
        return new Commerce("commerce.db");
    }
}

@Service
public class ProductService {

    private final Commerce commerce;

    public ProductService(Commerce commerce) {
        this.commerce = commerce;
    }

    public Product createProduct(String sku, String name, double price) {
        return commerce.products().create(sku, name, price);
    }

    public List<Product> getAllProducts() {
        return commerce.products().list();
    }
}

@RestController
@RequestMapping("/api/products")
public class ProductController {

    private final ProductService productService;

    @PostMapping
    public Product create(@RequestBody CreateProductRequest req) {
        return productService.createProduct(
            req.getSku(),
            req.getName(),
            req.getPrice()
        );
    }

    @GetMapping
    public List<Product> list() {
        return productService.getAllProducts();
    }
}
```

### Micronaut

```java
@Factory
public class CommerceFactory {

    @Singleton
    @Bean(preDestroy = "close")
    public Commerce commerce() {
        return new Commerce("commerce.db");
    }
}

@Controller("/api/orders")
public class OrderController {

    private final Commerce commerce;

    public OrderController(Commerce commerce) {
        this.commerce = commerce;
    }

    @Get("/{id}")
    public Optional<Order> get(String id) {
        return commerce.orders().get(id);
    }

    @Get
    public List<Order> list() {
        return commerce.orders().list();
    }
}
```

### Quarkus

```java
@ApplicationScoped
public class CommerceProducer {

    private Commerce commerce;

    @PostConstruct
    void init() {
        commerce = new Commerce("commerce.db");
    }

    @PreDestroy
    void cleanup() {
        if (commerce != null) {
            commerce.close();
        }
    }

    @Produces
    @ApplicationScoped
    public Commerce commerce() {
        return commerce;
    }
}
```

## Enterprise E-commerce Integration

### SAP Hybris / SAP Commerce Cloud

```java
// Custom facade implementation
public class StateSetProductFacade implements ProductFacade {

    private final Commerce commerce;

    @Override
    public ProductData getProductForCode(String code) {
        return commerce.products().get(code)
            .map(this::toProductData)
            .orElseThrow(() -> new UnknownIdentifierException(code));
    }

    private ProductData toProductData(Product product) {
        ProductData data = new ProductData();
        data.setCode(product.getSku());
        data.setName(product.getName());
        data.setPrice(createPrice(product.getBasePrice()));
        return data;
    }
}
```

### Broadleaf Commerce

```java
@Component
public class StateSetInventoryService implements InventoryService {

    private final Commerce commerce;

    @Override
    public boolean isAvailable(Sku sku, int quantity) {
        return commerce.inventory().get(sku.getId())
            .map(item -> item.getQuantityAvailable() >= quantity)
            .orElse(false);
    }

    @Override
    public void decrementInventory(Sku sku, int quantity) {
        commerce.inventory().adjust(
            sku.getId(),
            -quantity,
            "Order fulfillment"
        );
    }
}
```

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|--------|
| Linux    | x86_64      | ✅     |
| Linux    | arm64       | ✅     |
| macOS    | x86_64      | ✅     |
| macOS    | arm64 (M1+) | ✅     |
| Windows  | x86_64      | ✅     |

## Building from Source

### Prerequisites

- Rust 1.70+
- JDK 11+
- Gradle 8.0+

### Build

```bash
# Build native library
cd bindings/java
cargo build --release

# Build Java JAR
cd java
./gradlew build
```

### Run Tests

```bash
./gradlew test
```

## Thread Safety

The `Commerce` class and all its APIs are thread-safe. The underlying Rust engine uses proper synchronization. You can safely share a single `Commerce` instance across multiple threads.

```java
// Safe to use from multiple threads
ExecutorService executor = Executors.newFixedThreadPool(10);
Commerce commerce = new Commerce("commerce.db");

for (int i = 0; i < 100; i++) {
    final int idx = i;
    executor.submit(() -> {
        commerce.products().create(
            "SKU-" + idx,
            "Product " + idx,
            9.99
        );
    });
}
```

## Error Handling

All errors are thrown as `StateSetException`:

```java
try {
    commerce.inventory().adjust("NONEXISTENT", -100, "Test");
} catch (StateSetException e) {
    System.err.println("Error: " + e.getMessage());
    // Handle specific error types based on message
}
```

## Performance

- **Embedded SQLite**: No network latency, sub-millisecond queries
- **Native JNI**: Direct memory access, minimal marshalling overhead
- **Thread-safe**: Concurrent access with proper locking
- **Lazy loading**: APIs are instantiated on first access

## License

MIT License - see LICENSE file for details.

## Links

- [GitHub Repository](https://github.com/stateset/stateset-icommerce)
- [Maven Central](https://central.sonatype.com/artifact/com.stateset/embedded)
- [Documentation](https://docs.stateset.com)
