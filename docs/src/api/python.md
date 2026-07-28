# Python API Reference

The Python binding provides a `Commerce` class and a native agent toolkit
through the `stateset-embedded` package.

## Installation

```bash
pip install stateset-embedded
# or
poetry add stateset-embedded
# or
uv add stateset-embedded

# optional framework extras
pip install "stateset-embedded[langchain]"
pip install "stateset-embedded[crewai]"
pip install "stateset-embedded[autogen]"
pip install "stateset-embedded[agents]"
```

Supported: CPython 3.9–3.13, with prebuilt wheels for Linux x64/aarch64,
macOS Intel/Apple Silicon, and Windows x64; other environments build
from the sdist wherever a Rust toolchain is available.

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

## Agent Toolkit

For Python agent runtimes, the binding also ships a native toolkit over the
embedded engine:

```python
from stateset_embedded import Commerce, create_embedded_agent_toolkit

commerce = Commerce(":memory:")
toolkit = create_embedded_agent_toolkit(commerce, allow_apply=False)

openai_tools = toolkit.get_tools(format="openai")
descriptors = toolkit.create_tool_descriptors(
    filter=["list_customers", "list_orders", "get_sales_summary"]
)
callable_registry = toolkit.create_callable_registry(filter=["list_customers"])
langchain_tools = toolkit.create_langchain_tools(filter=["list_customers"])

execution = toolkit.execute_openai_tool_call(
    {
        "call_id": "py_call_1",
        "function": {
            "name": "list_customers",
            "arguments": "{\"limit\": 5}",
        },
    }
)
```

This Python toolkit covers core embedded commerce operations such as customer,
order, product, inventory, and analytics workflows. For the full JS
registry-generated tool surface and policy/runtime helpers, use the JS toolkit
or MCP server.

Framework helper methods are also available for Python hosts:

- `create_callable_registry()` for simple callable maps
- `create_tool_descriptors()` for executable framework-neutral descriptors
- `execute_tool()` / `execute_tool_calls()` for direct generic execution
- `create_openai_tools()` and `execute_openai_tool_call()` for OpenAI-compatible hosts
- `create_langchain_tools()` for LangChain when `langchain-core` is installed
- `create_crewai_tools()` for CrewAI when `crewai` is installed
- `create_autogen_tools()` for AutoGen when `autogen-core` is installed

Each framework helper also accepts a `tool_factory` callback so you can build
framework objects yourself without adding those dependencies to the base package.

The same helpers are also exported as direct helper modules:

```python
from stateset_embedded.generic import create_tool_descriptors, create_callable_registry
from stateset_embedded.openai import create_openai_tools, execute_openai_tool_call
from stateset_embedded.langchain import create_langchain_tools
from stateset_embedded.crewai import create_crewai_tools
from stateset_embedded.autogen import create_autogen_tools

descriptors = create_tool_descriptors(commerce, filter=["list_customers"])
registry = create_callable_registry(commerce, filter=["list_customers"])
openai_tools = create_openai_tools(commerce, filter=["list_customers"])
langchain_tools = create_langchain_tools(commerce, filter=["list_customers"])
crewai_tools = create_crewai_tools(commerce, filter=["count_customers"])
autogen_tools = create_autogen_tools(commerce, filter=["get_sales_summary"])
```

See the runnable examples under:

- `examples/python/openai_tools.py`
- `examples/python/generic_tools.py`
- `examples/python/langchain_tools.py`
- `examples/python/crewai_tools.py`
- `examples/python/autogen_tools.py`
- `examples/python/framework_adapters.py`

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

# Get customer by email
customer = commerce.customers.get_by_email("test@example.com")

# List all customers
customers = commerce.customers.list()

# Count customers
count = commerce.customers.count()
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
reservation = commerce.inventory.reserve(
    "SKU-001",
    10,
    reference_type="order",
    reference_id="ord-123"
)

# Release reservation
commerce.inventory.release_reservation(reservation.id)

# Get stock level
level = commerce.inventory.get_stock("SKU-001")
print(f"Available: {level.total_available}")
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

# Count orders
count = commerce.orders.count()
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
try:
    order = commerce.orders.ship(order_id)
except Exception as e:
    print(f"Operation failed: {e}")
```

## Available APIs

| API | Description |
|-----|-------------|
| `customers` | Customer management |
| `products` | Product catalog |
| `custom_objects` | Custom schemas and custom object records |
| `orders` | Order lifecycle |
| `inventory` | Stock management |
| `carts` | Shopping carts |
| `returns` | Return processing |
| `payments` | Payment operations |
| `shipments` | Shipping management |
| `warranties` | Warranty tracking |
| `purchase_orders` | Purchase orders |
| `invoices` | B2B invoicing |
| `bom` | Bills of Materials |
| `work_orders` | Manufacturing |
| `currency` | Multi-currency |
| `analytics` | Reporting & forecasts |
| `vector(openai_api_key)` | Semantic search for products and customers |
| `SyncRuntime` | Local sync runtime for sequencer event recording and replication |

## Source Files

- Entry point: `Commerce`
- Type stubs: `bindings/python/python/stateset_embedded/__init__.pyi`
- Module root: `bindings/python/python/stateset_embedded/__init__.py`

## Examples

- `examples/python/basic_usage.py`
- `examples/python/agent_toolkit.py`
