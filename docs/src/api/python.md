# Python API Reference

The Python binding provides a `Commerce` class through the `stateset-embedded` package.

## Installation

```bash
pip install stateset-embedded
# or
poetry add stateset-embedded
# or
uv add stateset-embedded
```

## Quick Start

```python
from stateset_embedded import Commerce

# Initialize with SQLite database
commerce = Commerce("commerce.db")

# Or use in-memory database for testing
commerce = Commerce(":memory:")

# Create a customer
customer = commerce.customers.create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith",
    phone="+1-555-0123"
)

# Create a product
product = commerce.products.create(
    name="Premium Widget",
    sku="WIDGET-001",
    price=29.99,
    description="High-quality widget"
)

# Create inventory
commerce.inventory.create_item(
    sku="WIDGET-001",
    name="Premium Widget",
    initial_quantity=100
)

# Create an order
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        {"sku": "WIDGET-001", "name": "Widget", "quantity": 2, "unit_price": 29.99}
    ],
    currency="USD"
)

# Ship the order
shipped = commerce.orders.ship(order.id)
print(f"Order {shipped.order_number} shipped!")
```

## Common Operations

### Customer Management

```python
# Create customer
customer = commerce.customers.create(
    email="test@example.com",
    first_name="Test",
    last_name="User"
)

# Get customer by ID
customer = commerce.customers.get(customer_id)

# List all customers
customers = commerce.customers.list()

# Delete customer
deleted = commerce.customers.delete(customer_id)
```

### Inventory Management

```python
# Create inventory item
item = commerce.inventory.create_item(
    sku="SKU-001",
    name="Widget",
    initial_quantity=100
)

# Adjust inventory
commerce.inventory.adjust("SKU-001", 50, "Received shipment")

# Reserve inventory
reservation = commerce.inventory.reserve("SKU-001", 10)

# Release reservation
commerce.inventory.release(reservation.id)

# Get stock level
level = commerce.inventory.get_level("SKU-001")
print(f"Available: {level.available}")
```

### Order Processing

```python
# Create order
order = commerce.orders.create(
    customer_id=customer.id,
    items=[
        {"sku": "SKU-001", "name": "Widget", "quantity": 2, "unit_price": 29.99}
    ]
)

# Update status
commerce.orders.update_status(order.id, "processing")

# Ship order
shipped = commerce.orders.ship(order.id)

# Cancel order
cancelled = commerce.orders.cancel(order.id)

# List orders by status
pending = commerce.orders.list_by_status("pending")
```

### Subscriptions

```python
# Create a subscription plan
plan = commerce.subscriptions.create_plan(
    code="PREMIUM",
    name="Premium Plan",
    interval="month",
    interval_count=1,
    price=19.99,
    currency="USD"
)

# Subscribe a customer
subscription = commerce.subscriptions.subscribe(customer.id, plan.id)

# Pause subscription
commerce.subscriptions.pause(subscription.id)

# Resume subscription
commerce.subscriptions.resume(subscription.id)

# Cancel subscription
commerce.subscriptions.cancel(subscription.id)
```

### Promotions

```python
# Create a promotion
promo = commerce.promotions.create(
    code="SUMMER20",
    name="Summer Sale",
    discount_type="percentage",
    discount_value=20.0
)

# Activate promotion
commerce.promotions.activate(promo.id)

# Create a coupon
coupon = commerce.promotions.create_coupon(promo.id, "SAVE20NOW", max_uses=100)

# Validate coupon
valid = commerce.promotions.validate_coupon("SAVE20NOW")
```

### Analytics

```python
# Get sales summary
summary = commerce.analytics.sales_summary()
print(f"Total revenue: {summary.total_revenue}")
print(f"Order count: {summary.order_count}")

# Get top products
top_products = commerce.analytics.top_products(limit=10)

# Get top customers
top_customers = commerce.analytics.top_customers(limit=10)
```

## Error Handling

```python
from stateset_embedded import Commerce, StateSetError

try:
    order = commerce.orders.ship(order_id)
except StateSetError as e:
    print(f"StateSet error: {e}")
except Exception as e:
    raise e
```

## Context Manager

```python
from stateset_embedded import Commerce

# Use as context manager for automatic cleanup
with Commerce("commerce.db") as commerce:
    customer = commerce.customers.create(
        email="context@example.com",
        first_name="Context",
        last_name="User"
    )
    # ...
# Automatically closed
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers` | Customer management |
| `products` | Product catalog |
| `orders` | Order lifecycle |
| `inventory` | Stock management |
| `carts` | Shopping carts |
| `returns` | Return processing |
| `payments` | Payment operations |
| `shipments` | Shipping management |
| `warranties` | Warranty tracking |
| `suppliers` | Supplier management |
| `purchase_orders` | Purchase orders |
| `invoices` | B2B invoicing |
| `bom` | Bills of Materials |
| `work_orders` | Manufacturing |
| `currency` | Multi-currency |
| `subscriptions` | Recurring billing |
| `promotions` | Discounts & coupons |
| `tax` | Tax calculations |
| `quality` | Quality control |
| `lots` | Lot tracking |
| `serials` | Serial numbers |
| `warehouse` | Warehouse ops |
| `receiving` | Receiving |
| `fulfillment` | Picking & packing |
| `accounts_payable` | A/P management |
| `accounts_receivable` | A/R management |
| `cost_accounting` | Cost tracking |
| `credit` | Credit management |
| `backorders` | Backorder tracking |
| `general_ledger` | GL accounting |
| `analytics` | Reporting & forecasts |

## Source Files

- Entry point: `Commerce`
- Type stubs: `bindings/python/python/stateset_embedded/__init__.pyi`
- Module root: `bindings/python/python/stateset_embedded/__init__.py`

## Examples

- `examples/python/basic_usage.py`
- `examples/python/subscriptions.py`
