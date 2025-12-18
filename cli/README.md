# @stateset/cli

AI-powered command-line interface for StateSet Commerce operations.

## Features

- **Natural Language Interface** - Ask Claude to perform commerce operations
- **Multi-turn Conversations** - Resume sessions for complex workflows
- **Preview Mode** - See what would happen before making changes
- **Direct Commands** - Fast, non-AI mode for simple operations
- **Interactive Chat** - REPL for exploratory work
- **SQLite Backend** - All data stored locally

## Installation

```bash
npm install -g @stateset/cli
```

Or run locally:

```bash
cd cli
npm install
npm link
```

## Quick Start

### AI-Powered Mode

```bash
# List customers (read-only by default)
stateset "show me all customers"

# Check inventory
stateset "how much stock do we have of WIDGET-001?"

# Create a customer (requires --apply)
stateset --apply "create a customer named Alice with email alice@example.com"

# Multi-turn workflow
stateset --apply "create an order for that customer with 2 widgets at $29.99"
stateset --apply --resume <session-id> "ship that order with tracking ABC123"
```

### Interactive Chat

```bash
stateset-chat

# In chat:
> show me all orders
> /apply on
> create a product called Premium Widget with SKU WIDGET-001 at $29.99
> /exit
```

### Direct Commands (No AI)

```bash
# Customer operations
stateset-direct customers list
stateset-direct customers get alice@example.com
stateset-direct customers create bob@example.com Bob Smith

# Order operations
stateset-direct orders list
stateset-direct orders get <order-id>
stateset-direct orders ship <order-id> TRACK123

# Inventory operations
stateset-direct inventory stock WIDGET-001
stateset-direct inventory adjust WIDGET-001 -5 "Sold 5 units"
stateset-direct inventory create WIDGET-002 "Large Widget" 50

# Product operations
stateset-direct products list
stateset-direct products variant WIDGET-001

# Return operations
stateset-direct returns list
stateset-direct returns approve <return-id>
```

## Commands

### `stateset` - AI Agent

The main AI-powered interface that understands natural language.

```
stateset [options] "<request>"

Options:
  --db <path>     Database path (default: ./store.db)
  --apply         Enable write operations
  --model <name>  Claude model (default: see src/config.js)
  --resume <id>   Resume previous session
  --json          JSON output
  --help          Show help
```

### `stateset-chat` - Interactive Mode

Multi-turn conversational interface.

```
stateset-chat [options]

Options:
  --db <path>     Database path
  --apply         Start with write enabled
  --model <name>  Claude model

In-chat commands:
  /help           Show commands
  /status         Current settings
  /apply on|off   Toggle write mode
  /db <path>      Switch database
  /new            Start new session
  /exit           Exit
```

### `stateset-direct` - Direct Commands

Fast, non-AI interface for common operations.

```
stateset-direct [options] <resource> <action> [args]

Options:
  --db <path>     Database path
  --json          JSON output
  --help          Show help

Resources:
  customers       Customer management
  orders          Order management
  products        Product catalog
  inventory       Stock management
  returns         Return processing
```

## Safety

By default, all write operations are **blocked**. The CLI will show you what would happen, but won't make changes.

To enable writes, use the `--apply` flag:

```bash
# Preview what would happen
stateset "create a customer named Bob"
# Output: "Would create customer: {email: ..., name: Bob}"

# Actually create the customer
stateset --apply "create a customer named Bob"
# Output: "Created customer: abc-123-def"
```

## Available Operations

### Customers
- List all customers
- Get customer by ID or email
- Create customer
- Count customers

### Orders
- List all orders
- Get order details with line items
- Create order
- Update order status
- Ship order with tracking
- Cancel order

### Products
- List products
- Get product details
- Get variant by SKU
- Create product with variants

### Inventory
- Get stock levels
- Create inventory items
- Adjust stock (add/remove)
- Reserve stock for orders
- Confirm/release reservations

### Returns
- List returns
- Get return details
- Create return request
- Approve/reject returns

### Carts/Checkout (Agentic Commerce Protocol)
- List shopping carts
- Get cart details with items
- Create cart (guest or authenticated)
- Add/update/remove cart items
- Set shipping address
- Set payment method
- Apply discount codes
- Get shipping rates
- Complete checkout (creates order)
- Cancel/abandon carts
- Get abandoned carts for recovery

## Examples

### E-commerce Workflow

```bash
# Set up a product
stateset --apply "create a product called 'Premium Widget' with SKU WIDGET-001 at $29.99"

# Add inventory
stateset --apply "create inventory for WIDGET-001 with 100 units"

# Create a customer
stateset --apply "create customer alice@example.com named Alice Smith"

# Create an order
stateset --apply "create an order for alice@example.com: 2x WIDGET-001"

# Ship it
stateset --apply --resume <session> "ship that order with tracking FEDEX123"
```

### Inventory Management

```bash
# Check stock
stateset "how much WIDGET-001 do we have?"

# Restock
stateset --apply "add 50 units to WIDGET-001 - received shipment"

# Adjust for damage
stateset --apply "remove 3 units from WIDGET-001 - damaged in warehouse"
```

### Processing Returns

```bash
# Create return
stateset --apply "create a return for order #12345 - item defective"

# Review and approve
stateset "show me pending returns"
stateset --apply "approve return <return-id>"
```

### Shopping Cart Checkout (Agentic Commerce Protocol)

```bash
# Create a cart for guest checkout
stateset --apply "create a cart for alice@example.com"

# Add items to cart (multi-turn)
stateset --apply --resume <session> "add 2 Premium Widgets at $29.99"
stateset --apply --resume <session> "add 1 Deluxe Widget at $49.99"

# Set shipping address
stateset --apply --resume <session> "set shipping to Alice Smith, 123 Main St, Anytown, CA 90210"

# Apply discount
stateset --apply --resume <session> "apply discount code SAVE10"

# Check shipping options
stateset --resume <session> "what shipping options are available?"

# Set payment and complete
stateset --apply --resume <session> "pay with credit card and complete checkout"
```

### Cart Recovery

```bash
# Find abandoned carts
stateset "show me abandoned carts"

# Get cart details for follow-up
stateset "show me cart CART-123456789"
```

## Configuration

### Environment Variables

```bash
# Claude API key (required for AI mode)
export ANTHROPIC_API_KEY="sk-ant-..."
```

### Database

The CLI uses SQLite for local storage. Default path is `./store.db`.

```bash
# Use a different database
stateset --db /path/to/mystore.db "list customers"

# Use in-memory database (testing)
stateset --db :memory: "list customers"
```

## Architecture

This CLI is built on:

- **@stateset/embedded** - Native Rust commerce library
- **Claude Agent SDK** - AI agent framework with MCP tools
- **SQLite** - Local database storage

The AI mode uses Claude with MCP (Model Context Protocol) tools to understand natural language requests and translate them into commerce operations.

## License

MIT OR Apache-2.0
