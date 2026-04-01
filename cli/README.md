# @stateset/cli

AI-powered command-line interface for autonomous commerce operations.

**Version:** 0.9.2

[![npm version](https://img.shields.io/npm/v/@stateset/cli.svg)](https://www.npmjs.com/package/@stateset/cli)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Highlights

- **Verifiable Event Sync (VES) Protocol** - Cryptographically signed event sourcing with Ed25519
- **gRPC Bidirectional Streaming** - Real-time sync with the StateSet Sequencer
- **Autonomous Business Engine** - Scheduled jobs, workflows, policies, approvals (`stateset-autonomous`)
- **Multi-Chain Stablecoin Payments** - Native crypto payments on Solana, Base, Ethereum, SET Chain, Zcash, and Bitcoin (`stateset-pay`)
- **x402 Payments** - Config + MCP server for paid API calls (`stateset-x402`, `stateset-x402-mcp`)

## Philosophy

The StateSet CLI is built on the premise that commerce infrastructure should be designed for AI agents, not just humans. Think of it as **"The SQLite of Commerce"** — an embedded, zero-dependency commerce engine that:

- **Runs locally** without cloud dependencies
- **Deterministic operations** for agent reliability
- **Agentic Commerce Protocol (ACP)** for standardized agent interactions
- **Safety-first architecture** — read-only by default, explicit `--apply` for writes

## Features

### Core
- **Natural Language Interface** - Ask Claude to perform commerce operations
- **Multi-Agent System** - 17 specialized agents auto-route to the best handler
- **90+ MCP Tools** - Full commerce API exposed to Claude
- **Hybrid Vector Search** - Semantic + BM25 search for products, customers, orders, and inventory (requires `OPENAI_API_KEY`)
- **Multi-turn Sessions** - Resume conversations for complex workflows
- **Preview Mode** - See what would happen before making changes
- **Direct Commands** - Fast, non-AI mode for scripting
- **Interactive Chat** - REPL for exploratory work

### Sync & Streaming
- **VES Protocol v1.0** - Verifiable Event Sync with Ed25519 signatures
- **gRPC Streaming** - Real-time bidirectional sync with sequencer
- **Event Outbox** - SQLite-backed event sourcing with conflict resolution
- **Optimistic Concurrency** - Base version tracking for safe multi-agent updates

### Payments
- **Multi-Chain Stablecoins** - USDC on Solana, Base, Ethereum, Arbitrum
- **SET Chain ssUSD** - Yield-bearing stablecoin on StateSet L2
- **Bitcoin & Zcash** - Native BTC and ZEC support
- **VES Key Derivation** - Deterministic wallet addresses per agent

### Infrastructure
- **Batch Processing** - Sequential or parallel request processing
- **Interactive Tutorials** - Guided onboarding for new users
- **SQLite/PostgreSQL** - Flexible storage backends
- **Rich Output** - ASCII tables, progress bars, formatted displays
- **Telemetry** - Distributed tracing with `--verbose` and `--stats`

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

Tip: `ss` is a shorthand alias for `stateset`.

### Run the Tutorial

New to StateSet CLI? Start with the interactive tutorial:

```bash
stateset-tutorial quickstart
```

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
> /status
> /exit
```

### Direct Commands (No AI)

```bash
# Customer operations
stateset-direct customers list
stateset-direct customers get alice@example.com

# Order operations
stateset-direct orders list
stateset-direct --apply orders ship <order-id> TRACK123

# Inventory operations
stateset-direct inventory stock WIDGET-001
stateset-direct --apply inventory adjust WIDGET-001 -5 "Sold 5 units"

# Vector search (hybrid semantic + BM25)
stateset-direct vector search products "wireless earbuds" 5
```

### Autonomous Engine

Initialize the default templates, then start the runtime:

```bash
stateset-autonomous init --db ./.stateset/commerce.db --store ./.stateset/autonomous
stateset-autonomous start --db ./.stateset/commerce.db --store ./.stateset/autonomous
```

Check status and manage scheduled jobs:

```bash
stateset-autonomous status
stateset-autonomous status --json --output autonomous-status.json

stateset-autonomous jobs list
stateset-autonomous jobs enable <job-id>
stateset-autonomous jobs run <job-id>
stateset-autonomous jobs --json | jq '.jobs[] | {id, name, schedule}'
stateset-autonomous jobs list --enabled
stateset-autonomous jobs list --status failed
```

Re-initialize templates (overwrites the autonomous store path):

```bash
stateset-autonomous init --force
```

### Stablecoin Payments

```bash
# Show wallets and balances
stateset-pay --wallet
stateset-pay --balance --chain solana

# Send a payment (safe by default, requires --apply)
stateset-pay --apply --to 9WzD...WWWM --amount 50 --chain solana

# Machine-readable output
stateset-pay --balance --chain solana --json --output balance.json
```

### Treasury (Funding + Token Purchases)

```bash
# Initialize treasury storage
stateset-treasury init

# Show an agent wallet to receive funds
stateset-treasury wallet --agent default --chain base

# Record a USDC deposit (use --apply to commit)
stateset-treasury deposit --agent default --chain base --token USDC --amount 250 --apply

# Enable LLM billing from treasury (USDC)
export TREASURY_BILLING=true
export TREASURY_CHAIN=base
export TREASURY_TOKEN=USDC
export TREASURY_AGENT=default
export TREASURY_ERC8004_REGISTRY=https://registry.example

# Register ERC-8004 identity (once)
stateset-treasury identity register --registry https://registry.example --agent-id default --uri https://agents.example/default --apply

# Run an agent call (LLM usage will debit USDC)
stateset "summarize today's orders"

# Or pass treasury config via CLI flags
stateset --treasury --treasury-chain base --treasury-token USDC --treasury-agent default \
  --treasury-erc8004-registry https://registry.example \
  "summarize today's orders"

# Optional: register a custom agent token (price used for mock swaps)
stateset-treasury token add --symbol AGENTX --chain set_chain --decimals 18 --price 0.25 --apply

# Optional: buy tokens using treasury funds
stateset-treasury buy --agent default --chain set_chain --to AGENTX --amount 100 --apply

# Check balances and ledger
stateset-treasury balance --agent default --chain base
stateset-treasury ledger --agent default --limit 10
stateset-treasury ledger --agent default --task <tool-call-id>

# Price tools in tokens (auto-debit on tool calls when --apply is set)
stateset-treasury pricing set --tool list_orders --chain base --token USDC --amount 0.05 --apply
stateset-treasury pricing list

# Link ERC-8004 identity to agent wallet
stateset-treasury identity register --registry https://registry.example --agent-id agent-001 --uri https://agents.example/agent-001 --apply
stateset-treasury identity link-wallet --registry https://registry.example --agent-id agent-001 --chain set_chain --apply
```

### Install Gateway Service

```bash
# Preview changes
stateset-install-service --dry-run

# Install system service (Linux systemd / macOS launchd)
sudo stateset-install-service

# Remove system service
sudo stateset-install-service --uninstall

# Machine-readable plan
stateset-install-service --dry-run --json --output service-plan.json
```

### Batch Processing

```bash
# Process multiple requests sequentially (maintains session context)
echo "list customers" | stateset --stdin --json

# Process requests from file
stateset --batch requests.txt

# Parallel processing (faster, independent requests)
stateset --batch requests.txt --parallel 4 --json

# Parallel with write operations
stateset --apply --batch orders.txt --parallel 3
```

## Commands

### Primary Commands

| Command | Description |
|---------|-------------|
| `stateset "<request>"` | AI-powered interface (auto-routes to best agent) |
| `stateset-chat` | Multi-turn interactive REPL |
| `stateset-direct <resource> <action>` | Direct CLI (no AI required) |

### Specialized Agent Commands

| Command | Agent | Description |
|---------|-------|-------------|
| `stateset-checkout` | checkout | Shopping cart & checkout flow (ACP) |
| `stateset-orders` | orders | Order lifecycle management |
| `stateset-inventory` | inventory | Stock & reservation management |
| `stateset-returns` | returns | RMA & refund processing |
| `stateset-analytics` | analytics | Sales metrics & forecasting |
| `stateset-promotions` | promotions | Promotions, discounts & coupons |
| `stateset-subscriptions` | subscriptions | Subscription plans & recurring billing |
| `stateset-create` | storefront | Scaffold e-commerce storefronts |
| `stateset-manufacturing` | manufacturing | BOM & work order management |
| `stateset-payments` | payments | Payment processing & refunds |
| `stateset-shipments` | shipments | Shipment tracking & delivery |
| `stateset-suppliers` | suppliers | Supplier & purchase order management |
| `stateset-invoices` | invoices | B2B invoice management |
| `stateset-warranties` | warranties | Product warranty & claims |
| `stateset-currency` | currency | Multi-currency & exchange rates |
| `stateset-tax` | tax | Tax calculation & compliance |
| `stateset-pay` | stablecoin | Native crypto payments (USDC, ssUSD, BTC, ZEC) |

### Utility Commands

| Command | Description |
|---------|-------------|
| `stateset-channels` | Multi-channel gateway runtime |
| `stateset-autonomous` | Autonomous engine (scheduler, workflows, policies, approvals) |
| `stateset-config` | Profile and configuration management |
| `stateset-daemon` | Systemd service manager for the gateway |
| `stateset-install-service` | Install gateway as a system service (systemd/launchd) |
| `stateset-doctor` | Health check and diagnostics |
| `stateset-events` | Event management and webhooks |
| `stateset-mcp-events` | MCP event gateway (SSE + history + subscriptions) |
| `stateset-sync` | Verifiable Event Sync with sequencer |
| `stateset-tutorial` | Interactive tutorials and onboarding |
| `stateset-completion` | Shell completion scripts (bash/zsh/fish) |
| `stateset-x402` | x402 config + key setup |
| `stateset-x402-mcp` | MCP server for x402 paid API calls |

## x402 MCP Server

Initialize configuration (generates signing key + derives wallet):

```bash
stateset-x402 init \
  --sequencer-url "https://api.sequencer.stateset.app" \
  --tenant-id "your-tenant-id" \
  --store-id "your-store-id" \
  --agent-id "your-agent-id" \
  --network "set_chain"
```

Expose paid API calls over MCP (stdio transport):

```bash
export X402_SEQUENCER_URL="https://api.sequencer.stateset.app"
export X402_TENANT_ID="your-tenant-id"
export X402_STORE_ID="your-store-id"
export X402_AGENT_ID="your-agent-id"
export X402_PAYER_ADDRESS="your-wallet-address"

# Optional budget guardrails
export X402_BUDGET_PER_CALL="1000000"
export X402_BUDGET_DAILY="50000000"

stateset-x402-mcp
```

You can also point the server at a config file:

```bash
export X402_CONFIG_FILE=".stateset/x402.json"
stateset-x402-mcp
```

Tools exposed:
- `x402_call`
- `x402_budget_status`
- `x402_history`
- `x402_receipt`
- `x402_balance`

## Architecture

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/       # Pure domain models (254 types, 18 modules)
│   ├── stateset-db/         # SQLite + PostgreSQL (53 tables)
│   └── stateset-embedded/   # High-level unified API (671+ methods)
├── bindings/
│   ├── node/                # @stateset/embedded (NAPI)
│   ├── python/              # stateset-embedded (PyO3)
│   └── wasm/                # WebAssembly for browsers
└── cli/
    ├── bin/                 # 26 CLI programs
    ├── src/
    │   ├── claude-harness.js    # Multi-agent SDK integration
    │   ├── mcp-server.js        # 90+ MCP tools for Claude
    │   ├── sync/                # VES Protocol & gRPC streaming
    │   │   ├── grpc-client.js   # Bidirectional gRPC client
    │   │   ├── engine.js        # Sync orchestration
    │   │   ├── outbox.js        # Event outbox (SQLite)
    │   │   ├── crypto.js        # Ed25519 signing & hashing
    │   │   └── proto/           # Protocol buffer definitions
    │   ├── chains/              # Multi-chain payment support
    │   │   ├── config.js        # Chain configurations
    │   │   ├── wallet.js        # VES key → wallet derivation
    │   │   ├── stablecoin.js    # Payment operations
    │   │   └── validation.js    # Address validation
    │   ├── permissions.js       # Fine-grained access control
    │   ├── telemetry.js         # Observability & tracing
    │   ├── errors.js            # Structured error handling
    │   ├── session.js           # Session persistence
    │   └── database.js          # Connection pooling
    └── .claude/
        ├── agents/          # 17 specialized agent definitions
        └── skills/          # Domain knowledge documents
```

### Technology Stack

- **Rust Core** - Pure domain logic with deterministic execution
- **@stateset/embedded** - Native Node.js bindings via NAPI
- **Claude Agent SDK** - AI agent framework with MCP tools
- **SQLite/PostgreSQL** - Flexible database backends

## Capabilities

### Commerce Operations

| Domain | Operations |
|--------|------------|
| **Orders** | Create, confirm, process, ship, deliver, cancel |
| **Customers** | Profiles, addresses, preferences, history |
| **Products** | Catalog with variants and attributes |
| **Inventory** | Multi-location stock, reservations, adjustments |
| **Carts** | Shopping cart with ACP checkout flow |
| **Returns** | RMA processing with refund management |
| **Shipments** | Fulfillment tracking with carrier integration |

### Manufacturing Operations

| Domain | Operations |
|--------|------------|
| **Bill of Materials** | Create, manage, activate BOMs |
| **Work Orders** | Production job management |
| **Components** | Track component requirements |

### Financial Operations

| Domain | Operations |
|--------|------------|
| **Multi-Currency** | 35+ currencies (USD, EUR, GBP, JPY, BTC, ETH, USDC) |
| **Payments** | Credit card, PayPal, cryptocurrency |
| **Refunds** | Multiple payout methods |
| **Invoices** | B2B invoice management with payment terms |
| **Purchase Orders** | Supplier management and procurement |
| **Tax** | Multi-jurisdiction tax calculation (US, EU, CA) |

### Promotions & Discounts

```bash
# Create promotions
stateset --apply "create a 20% off promotion called Summer Sale"
stateset --apply "create a buy 2 get 1 free promotion"
stateset --apply "create a free shipping promotion for orders over $50"

# Manage coupons
stateset --apply "create coupon SAVE20 with 100 use limit"
stateset "is coupon SAVE20 valid?"

# Apply to cart
stateset --apply "apply promotions to cart <cart-id>"
```

### Subscriptions & Recurring Billing

```bash
# Create plans
stateset --apply "create a monthly plan called Coffee Club at $29.99 with 14 day trial"
stateset --apply "create an annual plan called Pro at $99.99"

# Manage subscriptions
stateset --apply "subscribe customer <id> to the Coffee Club plan"
stateset --apply "pause subscription <id>"
stateset --apply "skip next billing for subscription <id>"
stateset --apply "cancel subscription <id>"

# View history
stateset "show billing history for subscription <id>"
```

### Manufacturing

```bash
# Bill of Materials
stateset-manufacturing "list all BOMs"
stateset-manufacturing --apply "create a BOM for product ASSEMBLY-001"
stateset-manufacturing --apply "add component PART-A qty 2 to BOM BOM-123"

# Work Orders
stateset-manufacturing "list pending work orders"
stateset-manufacturing --apply "create work order from BOM BOM-123 for 100 units"
stateset-manufacturing --apply "start work order WO-456"
stateset-manufacturing --apply "complete work order WO-456 with 98 units produced"
```

### Tax Management

```bash
# Calculate tax
stateset "calculate tax for an order shipping to California"
stateset "what's the tax rate for New York?"
stateset "calculate tax for cart CART-123456"

# Tax jurisdictions
stateset "show me all tax jurisdictions"
stateset "what are the EU VAT rates?"
stateset "get tax info for Texas"

# Exemptions
stateset --apply "create tax exemption for customer abc123 - resale certificate"
```

### Analytics & Forecasting

```bash
# Sales analytics
stateset "what's my total revenue this month?"
stateset "show me my best sellers"
stateset "who are my top customers?"

# Inventory health
stateset "what inventory needs attention?"
stateset "show me low stock items"

# Forecasting
stateset "predict inventory needs for next month"
stateset "forecast revenue for next quarter"
```

## Safety Architecture

### Read-Only by Default

All write operations are **blocked by default**. The CLI shows what would happen without making changes.

```bash
# Preview what would happen
stateset "create a customer named Bob"
# Output: "Would create customer: {email: ..., name: Bob}"

# Actually create the customer
stateset --apply "create a customer named Bob"
# Output: "Created customer: abc-123-def"

# Direct mode writes also require --apply
stateset-direct --apply inventory adjust WIDGET-001 -5 "Sold 5 units"
```

### Permission Controls

| Feature | Description |
|---------|-------------|
| **Preview Mode** | Default read-only operation |
| **Confirmation Thresholds** | High-value operations (>$1000) prompt for confirmation |
| **Permission Gating** | Five levels: none, read, preview, write, admin |
| **Spending Limits** | Max order value, daily totals |
| **Rate Limiting** | Tool calls/minute, write ops/minute |
| **Audit Logging** | Complete operation history |

## Observability

```bash
# Verbose output with tracing
stateset --verbose "show me pending orders"

# Execution statistics
stateset --stats "process all returns"

# JSON output for integration
stateset --json "list customers"
```

## Workflows

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

### Shopping Cart Checkout (ACP)

```bash
# Create a cart
stateset --apply "create a cart for alice@example.com"

# Add items (multi-turn)
stateset --apply --resume <session> "add 2 Premium Widgets at $29.99"
stateset --apply --resume <session> "add 1 Deluxe Widget at $49.99"

# Set shipping address
stateset --apply --resume <session> "set shipping to Alice Smith, 123 Main St, Anytown, CA 90210"

# Apply discount
stateset --apply --resume <session> "apply discount code SAVE10"

# Check shipping options
stateset --resume <session> "what shipping options are available?"

# Complete checkout
stateset --apply --resume <session> "pay with credit card and complete checkout"
```

### Cart Recovery

```bash
stateset "show me abandoned carts"
stateset "what items are in cart CART-123456?"
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

### Multi-Currency Support

```bash
stateset "what's the exchange rate from USD to EUR?"
stateset "convert $100 USD to EUR"
stateset "list all exchange rates"
stateset --apply "set exchange rate USD to EUR at 0.92"
stateset --apply "enable currencies USD, EUR, GBP, JPY"
```

### Stablecoin Payments

Send native cryptocurrency payments across multiple blockchains:

```bash
# List supported blockchains
stateset pay --chains

# Show agent wallet addresses
stateset pay --wallet                     # All chains
stateset pay --wallet --chain solana      # Specific chain

# Check stablecoin balance
stateset pay --balance --chain solana
stateset pay --balance --chain set_chain

# Send payment (preview mode - no actual transaction)
stateset pay --to 9WzDXwBb...WWWM --amount 50.00 --chain solana

# Execute real payment (requires --apply)
stateset pay --apply --to 9WzDXwBb...WWWM --amount 50.00 --chain solana

# Pay with ssUSD on SET Chain (yield-bearing stablecoin)
stateset pay --apply --to 0x1234...5678 --amount 100 --chain set_chain

# Include order metadata for audit trail
stateset pay --apply --to <addr> --amount 50 --chain solana --order ORD-123 --memo "Widget purchase"

# Write JSON output to file
stateset pay --wallet --output wallets.json

# AI-powered payments
stateset --apply "pay 50 USDC to 9WzDXwBb...WWWM on Solana"
stateset "check my wallet balance on Base"
```

**Supported Chains:**
| Chain | Token | Description |
|-------|-------|-------------|
| `solana` | USDC | Fast, cheap, proven liquidity |
| `solana_devnet` | USDC | Testing |
| `set_chain` | ssUSD | StateSet L2, yield-bearing |
| `base` | USDC | Coinbase L2, low fees |
| `ethereum` | USDC/USDT/DAI | Maximum security |
| `arbitrum` | USDC | Fast L2 |
| `zcash` | ZEC | Privacy-focused (t-addresses) |
| `bitcoin` | BTC | Original cryptocurrency |

### Supplier Management

```bash
# Manage suppliers
stateset-suppliers "list all suppliers"
stateset-suppliers --apply "create supplier Acme Corp"

# Purchase orders
stateset-suppliers --apply "create PO for 100 WIDGET-001 from Acme Corp"
stateset-suppliers --apply "approve purchase order PO-123"
stateset-suppliers --apply "send purchase order PO-123 to supplier"
```

### B2B Invoicing

```bash
# Create and manage invoices
stateset-invoices "list all invoices"
stateset-invoices --apply "create invoice for order ORD-456"
stateset-invoices --apply "send invoice INV-123"
stateset-invoices "show overdue invoices"
stateset-invoices --apply "record $500 payment for invoice INV-123"
```

### Storefront Creation

```bash
# Preview what would be created
stateset-create "create a store called Urban Thread"

# Create the project
stateset-create --apply "create a nextjs storefront for my coffee shop"

# Create in specific directory
stateset-create --apply --dir ~/projects "build an online bookstore"
```

Available templates: `nextjs`, `nextjs-minimal`, `vite-react`, `astro`

### Event Sync (VES Protocol)

Synchronize events between agents and the StateSet Sequencer using the Verifiable Event Sync protocol:

```bash
# Check sync status
stateset sync status

# Push local events to sequencer
stateset sync push

# Pull new events from sequencer
stateset sync pull

# Start real-time streaming sync
stateset sync stream

# Subscribe to specific entities
stateset sync stream --entity-type order --entity-id ORD-123
```

**gRPC Streaming** for real-time sync:

```javascript
import { GrpcSequencerClient } from '@stateset/cli/sync';

const client = new GrpcSequencerClient({
  url: 'sequencer.stateset.io:8081',
  tenantId: 'your-tenant-id',
  storeId: 'your-store-id',
  agentId: 'your-agent-id',
});

await client.connect();

// Push events
await client.pushEvents([{
  entityType: 'order',
  entityId: 'ORD-123',
  eventType: 'OrderCreated',
  payload: { status: 'pending', amount: 99.99 },
  baseVersion: 0,
}]);

// Subscribe to real-time updates
client.onEvent((event) => {
  console.log('Received:', event.eventType, event.entityId);
});

await client.startSyncStream();
```

**VES Protocol Features:**
- Ed25519 cryptographic signatures for event integrity
- Domain-separated hashing (`VES_PAYLOAD_PLAIN_V1`)
- Optimistic concurrency with base version tracking
- Automatic conflict detection and resolution
- Global sequence numbers for ordering

## Agent System

17 specialized agents handle different commerce domains:

| Agent | Tools | Purpose |
|-------|-------|---------|
| **checkout** | 14 ACP tools | Shopping cart & payment flows |
| **orders** | 6 tools | Order lifecycle & fulfillment |
| **inventory** | 6 tools | Stock management & reservations |
| **returns** | 5 tools | RMA & refund processing |
| **analytics** | 10 tools | Business intelligence & forecasting |
| **promotions** | 10 tools | Campaigns, discounts, coupons |
| **subscriptions** | 15 tools | Subscription plans & recurring billing |
| **manufacturing** | 11 tools | BOM & work order management |
| **payments** | 5 tools | Payment processing & refunds |
| **shipments** | 5 tools | Shipment tracking & delivery |
| **suppliers** | 8 tools | Supplier & purchase order management |
| **invoices** | 7 tools | B2B invoice management |
| **warranties** | 6 tools | Product warranty & claims |
| **currency** | 8 tools | Multi-currency & exchange rates |
| **tax** | 9 tools | Tax calculation & compliance |
| **storefront** | 12 tools | E-commerce site scaffolding |
| **customer-service** | All 87+ tools | Full-service fallback agent |

### Auto-Routing

The main `stateset` command automatically routes requests to the best agent based on:
- Request content analysis
- Confidence scoring
- Domain keyword matching
- Ambiguity detection

## MCP Tools (90+ Total)

| Domain | Count | Examples |
|--------|-------|----------|
| **Customers** | 3 | list, get, create |
| **Orders** | 6 | list, get, create, update_status, ship, cancel |
| **Products** | 4 | list, get, get_variant, create |
| **Inventory** | 6 | get_stock, create_item, adjust, reserve, confirm, release |
| **Returns** | 5 | list, get, create, approve, reject |
| **Carts/Checkout** | 14 | create, add_item, set_address, set_payment, complete_checkout |
| **Analytics** | 10 | sales_summary, top_products, demand_forecast, revenue_forecast |
| **Currency** | 8 | get_rate, convert, set_rate, format |
| **Tax** | 9 | calculate_tax, calculate_cart_tax, get_rate, list_jurisdictions |
| **Promotions** | 10 | list, create, activate, create_coupon, validate_coupon, apply |
| **Subscriptions** | 15 | list_plans, create_plan, create_subscription, pause, resume, cancel |
| **Manufacturing** | 11 | list_boms, create_bom, create_work_order, complete_work_order |
| **Payments** | 5 | list, get, create, complete, create_refund |
| **Shipments** | 3 | list, create, deliver |
| **Suppliers/POs** | 6 | list_suppliers, create_supplier, create_purchase_order |
| **Invoices** | 5 | list, create, send, record_payment, get_overdue |
| **Warranties** | 4 | list, create, create_claim, approve_claim |
| **Stablecoin** | 4 | get_agent_wallet, get_wallet_balance, create_payment, list_chains |

## Configuration

### Environment Variables

```bash
# Claude API key (required for AI mode)
export ANTHROPIC_API_KEY="sk-ant-..."
```

### Database

```bash
# Default: ./store.db (SQLite)
stateset --db /path/to/mystore.db "list customers"

# In-memory database (testing)
stateset --db :memory: "list customers"

# PostgreSQL (enterprise)
# Configure via environment or config file
```

### Flags Reference

| Flag | Description |
|------|-------------|
| `--db <path>` | Database path |
| `--apply` | Enable write operations |
| `--model <name>` | Claude model to use |
| `--resume <id>` | Resume previous session |
| `--json` | JSON output |
| `--verbose` | Real-time telemetry |
| `--stats` | Execution statistics |
| `--parallel <n>` | Process batch requests in parallel |
| `--batch <file>` | Read requests from file |
| `--stdin` | Read requests from stdin |
| `--help` | Show help |

## Session Management

Sessions enable multi-turn conversations with context preservation:

```bash
# First request returns session ID
stateset --apply "create a cart for alice@example.com"
# Output includes: Session ID: abc-123-def

# Resume to continue context
stateset --apply --resume abc-123-def "add 2 widgets at $29.99"
stateset --apply --resume abc-123-def "complete checkout"
```

## Tutorials

Learn the CLI with interactive tutorials:

```bash
# List available tutorials
stateset-tutorial

# Run specific tutorials
stateset-tutorial quickstart    # Learn basics in 5 minutes
stateset-tutorial orders        # Order management
stateset-tutorial inventory     # Stock management
stateset-tutorial checkout      # Shopping cart flow
stateset-tutorial analytics     # Business intelligence
```

## Shell Completions

Generate shell completion scripts:

```bash
# Bash
stateset-completion bash >> ~/.bashrc

# Zsh
stateset-completion zsh >> ~/.zshrc

# Fish
stateset-completion fish > ~/.config/fish/completions/stateset.fish
```

## Command Reference

### `stateset` - AI Agent

```
stateset [options] "<request>"

Options:
  --db <path>       Database path (default: ./store.db)
  --apply           Enable write operations
  --model <name>    Model name
  --provider <name> Model provider (default: claude)
  --think <level>   Extended thinking: off, low, medium, high
  --stream          Stream partial responses
  --budget <usd>    Maximum spend per query in USD
  --memory          Enable memory
  --no-memory       Disable memory
  --x402            Enable x402 MCP tools
  --resume <id>     Resume previous session
  --json            JSON output
  --format <fmt>    Output format: table, json, csv, yaml
  --output <file>   Write output to file
  --yes             Skip confirmation prompts
  --verbose         Show telemetry
  --stats           Show execution stats
  --parallel <n>    Parallel batch processing
  --batch <file>    Read requests from file
  --stdin           Read requests from stdin
  --help            Show help
```

### `stateset-chat` - Interactive Mode

```
stateset-chat [options]

Options:
  --db <path>       Database path
  --apply           Start with write enabled
  --model <name>    Model name
  --provider <name> Model provider (default: claude)
  --think <level>   Extended thinking: off, low, medium, high
  --stream          Stream partial responses
  --budget <usd>    Maximum spend per query in USD
  --memory          Enable memory
  --no-memory       Disable memory
  --x402            Enable x402 MCP tools

In-chat commands:
  /help           Show commands
  /status         Current settings
  /apply on|off   Toggle write mode
  /db <path>      Switch database
  /new            Start new session
  /exit           Exit
```

### `stateset-direct` - Direct Commands

```
stateset-direct [options] <resource> <action> [args]

Options:
  --db <path>     Database path
  --apply         Enable write operations
  --json          JSON output
  --format <fmt>  Output format: table, json
  --output <file> Write output to file
  --yes           Skip confirmation prompts
  --help          Show help

Resources:
  customers       Customer management
  orders          Order management
  products        Product catalog
  inventory       Stock management
  returns         Return processing
```

### `stateset-config` - Configuration

```
stateset-config <command> [options]

Options:
  --profile, -p <name>  Target a specific profile
  --json                JSON output
  --output <file>       Write output to file (implies --json)
  --help                Show help
```

### `stateset-events` - Event Streaming

```
stateset-events [options]

Options:
  --db <path>      Database path
  --filter <type>  Filter events (orders, inventory, customers, products, returns)
  --json           JSON output (one per line)
  --output <file>  Write output to file (implies --json)
  --quiet          Only output event data, no headers
  --help           Show help
```

### `stateset-mcp-events` - MCP Event Stream Gateway

```
stateset-mcp-events [options]

Options:
  --db <path>           Database path
  --host <host>         HTTP bind host (default: 127.0.0.1)
  --port <n>            HTTP port (default: 8081, 0 picks random)
  --history-limit <n>    Maximum events kept in memory (default: 500)
  --stream-name <name>   Stream name for emitted events
  --structured-tool-results Include machine-readable metadata in MCP tool responses
  --help                Show help

Endpoints:
  GET /events?session=<id>&types=success,error   SSE stream
  GET /history?session=<id>&types=success&since=<iso>&limit=<n>  Event history
  GET /subscriptions?session=<id>                          Active subscriptions
  GET /health                                              Service health
```

```bash
stateset-mcp-events --db ./store.db --port 8081

# Watch events for a session
curl -N "http://127.0.0.1:8081/events?session=session-1"

# Replay event history
curl "http://127.0.0.1:8081/history?types=success,error&limit=25"
```

### `stateset-channels` - Multi-Channel Gateway

```
stateset-channels --config <path> [options]

Options:
  --config <path>  Path to YAML or JSON config file (required)
  --verbose        Enable verbose logging
  --json           Output status as JSON
  --output <file>  Write JSON output to file (implies --json)
  --help           Show help
```

### `stateset-daemon` - Gateway Service Manager

```
stateset-daemon <command> [options]

Options:
  --config <path>  Config file path
  --port <n>       HTTP gateway port (default: 8080)
  --user           Manage as user service (no sudo)
  --follow, -f     Follow logs in real-time
  --json           JSON output (supported commands only)
  --output <file>  Write JSON output to file (implies --json)
  --reverse        Reverse SSH tunnel
  --persistent     Persistent SSH tunnel via systemd
  --name <n>       Name for persistent tunnel
  --help           Show help
```

### `stateset-autonomous` - Autonomous Engine

```
stateset-autonomous <command> [options]

Commands:
  start            Start the autonomous engine
  status           Show engine status
  init             Initialize default templates
  jobs             Manage scheduled jobs
  jobs list        List scheduled jobs
  jobs enable      Enable a job
  jobs disable     Disable a job
  jobs run         Run a job immediately

Options:
  --db <path>      Database path
  --store <path>   Engine data path
  --port <port>    Webhook server port (start)
  --no-webhooks    Disable webhook server
  --no-scheduler   Disable job scheduler
  --no-workflows   Disable workflow engine
  --no-policies    Disable policy engine
  --no-approvals   Disable approvals
  --init-defaults  Initialize with default templates (start)
  --notify-config  Notification routing config
  --force          Overwrite existing autonomous data (init)
  --status <s>     Filter jobs by status (jobs list)
  --enabled        Only enabled jobs (jobs list)
  --disabled       Only disabled jobs (jobs list)
  --json           JSON output (status/init/jobs)
  --output <file>  Write JSON output to file (implies --json)
  --verbose        Verbose output
  --help           Show help

Notes:
  Legacy flags `jobs --enable/--disable/--run` are still supported.
```

### `stateset-install-service` - Install Gateway Service

```
stateset-install-service [options]

Options:
  --dry-run       Preview actions without changes
  --uninstall     Remove the service
  --json          JSON output
  --output <file> Write JSON output to file (implies --json)
  --help          Show help
```

### Service Management Flow

```
# Install system service and load defaults
sudo stateset-install-service

# Validate config, then start
stateset-daemon validate
sudo stateset-daemon start

# Check status and tail logs
stateset-daemon status --json --output daemon-status.json
stateset-daemon logs -f
```

### `stateset-pay` - Stablecoin Payments

```
stateset-pay [options]

Options:
  --to <address>   Recipient wallet address
  --amount <amt>   Amount to send
  --chain <id>     Blockchain network
  --token <sym>    Token symbol
  --wallet         Show agent wallet address(es)
  --balance        Check wallet balance
  --chains         List supported blockchains
  --apply          Execute payment (default: simulate)
  --agent <id>     Agent ID
  --json           JSON output
  --output <file>  Write JSON output to file (implies --json)
  --yes            Skip confirmations
  --help           Show help
```

### `stateset-sync` - Verifiable Event Sync

```
stateset-sync <command> [options]

Options:
  --db <path>      Database path
  --json           JSON output
  --output <file>  Write JSON output to file (implies --json)
  --help           Show help
```

### `stateset-skills` - Skills Marketplace

```
stateset-skills <command> [options]

Options:
  --json           JSON output
  --output <file>  Write JSON output to file (implies --json)
  --category <cat> Filter by category
  --origin <name>  Filter by origin (bundled, installed, workspace)
  --force          Overwrite on install
  --help           Show help
```

### `stateset-x402` - x402 Configuration

```
stateset-x402 init [options]

Options:
  --sequencer-url <url>  Sequencer URL (required)
  --tenant-id <uuid>     Tenant UUID (required)
  --store-id <uuid>      Store UUID (required)
  --agent-id <id>        Agent ID (required)
  --network <network>    Preferred network (default: set_chain)
  --config-dir <path>    Config directory (default: .stateset)
  --config-file <path>   Override config file path
  --json                 JSON output
  --output <file>        Write JSON output to file (implies --json)
  --force                Overwrite existing config
  --help                 Show help
```

### `stateset-x402-mcp` - x402 MCP Server

```
stateset-x402-mcp [options]

Options:
  --config-dir <path>   Config directory (default: .stateset)
  --policy-dir <path>   Policy directory (default: STATESET_POLICY_DIR or .stateset)
  --help                Show help
  --version             Show version
```

## Error Handling

The CLI provides structured error handling with helpful suggestions:

```bash
# Permission errors
Error: Permission denied: 'create_customer' requires --apply flag

Suggestions:
  • Add --apply flag to enable write operations
  • Example: stateset --apply "your command here"
  • Run without --apply first to preview what would happen

# Not found errors
Error: Customer 'unknown@example.com' not found

Suggestions:
  • Check that the customer ID is correct
  • List available customers: stateset-direct customers list
  • Use a partial ID (like git) - only a few characters needed if unique
```

## Diagnostics

Run health checks:

```bash
stateset-doctor

# Check specific components
stateset-doctor --checks api,db,permissions

# JSON output for automation
stateset-doctor --json --output health.json
```

## Development

```bash
cd cli
npm install
npm link

# Test
npm test                    # All tests
npm run test:unit          # Unit tests only
npm run test:integration   # Integration tests

# Run diagnostics
stateset-doctor
```

## Programmatic Usage

The CLI modules can be used programmatically:

```javascript
import {
  runAgentLoop,
  createErrorHandler,
  createSessionManager,
  createDatabaseManager,
  withContext
} from '@stateset/cli';

// Run an agent request
const result = await runAgentLoop({
  request: 'list all customers',
  dbPath: './store.db',
  allowApply: false
});

// Use with request context
await withContext({ agent: 'orders' }, async (ctx) => {
  ctx.logEvent('processing_order');
  // ... your code
});
```

## License

MIT OR Apache-2.0
