# stateset-embedded

Local-first embedded commerce library for Python, powered by Rust.

## Installation

```bash
pip install stateset-embedded
```

Or build from source:

```bash
cd bindings/python
pip install maturin
maturin develop
```

## Quick Start

```python
from stateset_embedded import Commerce, CreateOrderItemInput

# Initialize with SQLite database
commerce = Commerce("./store.db")

# Or use in-memory database for testing
commerce = Commerce(":memory:")

# Create a customer
customer = commerce.customers.create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith"
)
print(f"Created customer: {customer.id}")

# Create a product with variant
product = commerce.products.create(
    name="Premium Widget",
    description="A high-quality widget"
)

# Create inventory
item = commerce.inventory.create_item(
    sku="WIDGET-001",
    name="Premium Widget",
    initial_quantity=100
)

# Check stock
stock = commerce.inventory.get_stock("WIDGET-001")
print(f"Available: {stock.total_available}")

# Create an order
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        CreateOrderItemInput(
            sku="WIDGET-001",
            name="Premium Widget",
            quantity=2,
            unit_price=29.99
        )
    ]
)
print(f"Order {order.order_number}: ${order.total_amount}")

# Ship the order
commerce.orders.ship(order.id, tracking_number="1Z999AA10123456784")

# Analytics
summary = commerce.analytics.sales_summary(period="last30days")
print(f"Revenue: ${summary.total_revenue}")

# Currency conversion (set a rate, then convert)
commerce.currency.set_rate("USD", "EUR", 0.92, source="manual")
conversion = commerce.currency.convert("USD", "EUR", 100.0)
print(f"$100 USD = €{conversion.converted_amount} EUR")
```

## Features

- **Local-First**: All data stored in SQLite, works offline
- **Zero Dependencies**: Single native extension, no external services
- **Type Safe**: Full type hints and IDE support
- **Fast**: Native Rust performance
- **Analytics + Currency**: Built-in reporting/forecasting and multi-currency operations

## API Reference

### Commerce

Main entry point for all operations.

```python
commerce = Commerce("./store.db")  # SQLite file
commerce = Commerce(":memory:")     # In-memory database
```

### Customers

```python
# Create
customer = commerce.customers.create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith",
    phone="+1234567890",
    accepts_marketing=True
)

# Get by ID or email
customer = commerce.customers.get(customer_id)
customer = commerce.customers.get_by_email("alice@example.com")

# List all
customers = commerce.customers.list()

# Count
count = commerce.customers.count()
```

### Orders

```python
# Create
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        CreateOrderItemInput(
            sku="SKU-001",
            name="Product Name",
            quantity=2,
            unit_price=29.99
        )
    ],
    currency="USD",
    notes="Gift wrap please"
)

# Get
order = commerce.orders.get(order_id)

# List all
orders = commerce.orders.list()

# Update status
order = commerce.orders.update_status(order_id, "processing")

# Ship with tracking
order = commerce.orders.ship(order_id, tracking_number="1Z123...")

# Cancel
order = commerce.orders.cancel(order_id)
```

### Products

```python
# Create with variants
from stateset_embedded import CreateProductVariantInput

product = commerce.products.create(
    name="Premium Widget",
    description="High-quality widget",
    variants=[
        CreateProductVariantInput(
            sku="WIDGET-SM",
            price=19.99,
            name="Small"
        ),
        CreateProductVariantInput(
            sku="WIDGET-LG",
            price=29.99,
            name="Large"
        )
    ]
)

# Get by ID
product = commerce.products.get(product_id)

# Get variant by SKU
variant = commerce.products.get_variant_by_sku("WIDGET-SM")

# List all
products = commerce.products.list()
```

### Inventory

```python
# Create inventory item
item = commerce.inventory.create_item(
    sku="WIDGET-001",
    name="Premium Widget",
    description="High-quality widget",
    initial_quantity=100,
    reorder_point=10
)

# Check stock levels
stock = commerce.inventory.get_stock("WIDGET-001")
print(f"On hand: {stock.total_on_hand}")
print(f"Allocated: {stock.total_allocated}")
print(f"Available: {stock.total_available}")

# Adjust stock
commerce.inventory.adjust("WIDGET-001", -5, "Sold 5 units")
commerce.inventory.adjust("WIDGET-001", 50, "Received shipment")

# Reserve for order
reservation = commerce.inventory.reserve(
    sku="WIDGET-001",
    quantity=2,
    reference_type="order",
    reference_id=order_id,
    expires_in_seconds=3600  # 1 hour
)

# Confirm reservation (deducts from on-hand)
commerce.inventory.confirm_reservation(reservation.id)

# Or release reservation (returns to available)
commerce.inventory.release_reservation(reservation.id)
```

### Returns

```python
from stateset_embedded import CreateReturnItemInput

# Create return request
ret = commerce.returns.create(
    order_id=order.id,
    reason="defective",
    items=[
        CreateReturnItemInput(
            order_item_id=order.items[0].id,
            quantity=1
        )
    ],
    reason_details="Product arrived damaged"
)

# Get return
ret = commerce.returns.get(return_id)

# Approve return
ret = commerce.returns.approve(return_id)

# Reject return
ret = commerce.returns.reject(return_id, "Item was used")

# List all returns
returns = commerce.returns.list()
```

### Carts / Checkout

```python
from stateset_embedded import CartAddress, AddCartItemInput

# Create a cart (guest checkout)
cart = commerce.carts.create(customer_email="alice@example.com", currency="USD")

# Add items
commerce.carts.add_item(
    cart_id=cart.id,
    item=AddCartItemInput(
        sku="SKU-001",
        name="Widget",
        quantity=2,
        unit_price=29.99,
    ),
)

# Set shipping (address + selection)
address = CartAddress(
    first_name="Alice",
    last_name="Smith",
    line1="123 Main St",
    city="San Francisco",
    postal_code="94105",
    country="US",
)
cart = commerce.carts.set_shipping(
    cart.id,
    address,
    shipping_method="standard",
    shipping_carrier="ups",
    shipping_amount=9.99,
)

# Reserve inventory for cart items (optional)
cart = commerce.carts.reserve_inventory(cart.id)

# Complete checkout (creates an order)
result = commerce.carts.complete(cart.id)
print(result.order_number)
```

### Analytics

```python
# Sales summary
summary = commerce.analytics.sales_summary(period="last30days")
print(summary.total_revenue, summary.order_count)

# Top products / customers
top_products = commerce.analytics.top_products(period="this_month", limit=10)
top_customers = commerce.analytics.top_customers(period="all_time", limit=10)

# Forecasting
forecasts = commerce.analytics.demand_forecast(days_ahead=30)
revenue = commerce.analytics.revenue_forecast(periods_ahead=3, granularity="month")
```

### Currency

```python
# Set an exchange rate
commerce.currency.set_rate("USD", "EUR", 0.92, source="manual")

# Convert currency
conversion = commerce.currency.convert("USD", "EUR", 100.0)
print(conversion.converted_amount)

# Store settings
settings = commerce.currency.get_settings()
settings = commerce.currency.enable_currencies(["USD", "EUR", "GBP"])
```

## Order Statuses

- `pending` - Order created, awaiting confirmation
- `confirmed` - Order confirmed
- `processing` - Order being processed
- `shipped` - Order shipped
- `delivered` - Order delivered
- `cancelled` - Order cancelled
- `refunded` - Order refunded

## Return Reasons

- `defective` - Product is defective
- `not_as_described` - Product not as described
- `wrong_item` - Wrong item received
- `no_longer_needed` - No longer needed
- `changed_mind` - Changed mind
- `better_price_found` - Found better price elsewhere
- `damaged` - Product arrived damaged
- `other` - Other reason

## Development

```bash
# Install dev dependencies
pip install maturin pytest

# Build in development mode
maturin develop

# Run tests
pytest tests/

# Build release wheel
maturin build --release
```

## License

MIT OR Apache-2.0
