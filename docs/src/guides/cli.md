# CLI

The `stateset` CLI is a natural-language interface to the embedded commerce engine and 520+ MCP tools. Tip: `ss` is a shorthand alias for `stateset`.

## Safety Model

- **Read-only by default.** Every command shows what would happen without changing anything.
- **Writes require `--apply`.** Mutations must be explicitly opted into.
- **20 high-risk tools require approval.** Even with `--apply`, destructive operations prompt for confirmation.

This is designed for autonomous agents: an LLM can safely explore your commerce data without accidentally shipping orders or issuing refunds.

## Common Commands

### Querying (Read-Only)

```bash
stateset "show me pending orders"
stateset "what products are low on stock?"
stateset "what is my revenue this month?"
stateset "find customers who ordered in the last 30 days"
stateset "list past-due subscriptions"
```

### Writing (Requires --apply)

```bash
stateset --apply "create a customer named Alice with email alice@example.com"
stateset --apply "ship order #12345 with tracking FEDEX123"
stateset --apply "adjust inventory for WIDGET-001, add 50 units, reason: shipment received"
stateset --apply "approve return RMA-789"
```

### Vector Search (Requires OPENAI_API_KEY)

Hybrid semantic + BM25 search when `OPENAI_API_KEY` is set:

```bash
export OPENAI_API_KEY=sk-...
stateset "find products similar to wireless earbuds"
stateset "search customers like enterprise retail buyers"
stateset "find orders mentioning backorder or late shipment"
```

## Domain-Specific Agents

Each commerce domain has a dedicated CLI command backed by a specialized agent:

```bash
stateset-orders "show pending orders ready to ship"
stateset-inventory "what SKUs are below reorder point?"
stateset-returns "process return for order ORD-001"
stateset-analytics "forecast revenue for next quarter"
stateset-checkout "create a cart for alice@example.com and add 2 widgets"
stateset-payments "show me refunds issued this month"
stateset-subscriptions "list customers with past-due subscriptions"
stateset-manufacturing "show work orders in progress"
stateset-tax "what is the effective tax rate for California?"
stateset-suppliers "list purchase orders pending approval"
```

## Direct CLI (No AI)

For scripting and automation, bypass AI routing with direct commands:

```bash
stateset-direct customers list
stateset-direct orders get ORD-001
stateset-direct inventory adjust SKU-001 --quantity 50 --reason "shipment received"
```

## All CLI Commands

47 binary commands are available:

| Category | Commands |
|----------|----------|
| **Core** | `stateset`, `stateset-direct`, `stateset-init`, `stateset-setup`, `stateset-config`, `stateset-update` |
| **Commerce** | `stateset-orders`, `stateset-inventory`, `stateset-payments`, `stateset-returns`, `stateset-checkout`, `stateset-subscriptions`, `stateset-shipments`, `stateset-invoices`, `stateset-warranties`, `stateset-promotions`, `stateset-tax`, `stateset-currency`, `stateset-manufacturing`, `stateset-suppliers`, `stateset-treasury` |
| **AI & Agents** | `stateset-agents`, `stateset-chat`, `stateset-autonomous`, `stateset-simulate`, `stateset-skills`, `stateset-create`, `stateset-tutorial` |
| **Analytics** | `stateset-analytics`, `stateset-events`, `stateset-mcp-events` |
| **Integration** | `stateset-webhooks`, `stateset-import`, `stateset-sync` |
| **Notifications** | `stateset-telegram`, `stateset-discord`, `stateset-slack`, `stateset-whatsapp`, `stateset-signal`, `stateset-google-chat`, `stateset-channels` |
| **Payments** | `stateset-pay`, `stateset-x402`, `stateset-x402-mcp` |
| **Operations** | `stateset-daemon`, `stateset-doctor`, `stateset-install-service`, `stateset-completion` |

## Setup & Configuration

```bash
# One-line quickstart
stateset-init --quickstart

# Full setup with demo data
stateset-init --demo

# Run the doctor to check your environment
stateset-doctor

# Fix detected issues automatically
stateset-doctor --fix
```

## Shell Completion

```bash
# Generate completion for your shell
stateset-completion bash >> ~/.bashrc
stateset-completion zsh >> ~/.zshrc
stateset-completion fish >> ~/.config/fish/completions/stateset.fish
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | Enable semantic search and AI agent features |
| `STATESET_DATA_DIR` | Custom data directory (default: `.stateset/`) |
| `STATESET_DB_PATH` | Custom SQLite database path |
| `STATESET_LOG_LEVEL` | Logging level: `error`, `warn`, `info`, `debug` |
| `STATESET_POLICIES_DIR` | Policy YAML directory (default: `./policies/`) |
| `STRIPE_API_KEY` | Enable Stripe adapter |
| `STRIPE_WEBHOOK_SECRET` | Stripe webhook signature verification |

## Error Messages

Common errors and how to resolve them:

| Error | Cause | Fix |
|-------|-------|-----|
| `--apply required` | Write operation without `--apply` flag | Add `--apply` to the command |
| `Policy denied: ...` | Policy rule blocked the operation | Read the denial reason and remediation |
| `Entity not found` | Invalid ID | Check the entity ID and try `list_*` first |
| `Permission denied` | Insufficient permission level | Use an API key with the required role |
| `OPENAI_API_KEY not set` | Semantic search without API key | Export `OPENAI_API_KEY=sk-...` |

## Scripting & Automation

For non-interactive batch scripts, use `stateset-direct`:

```bash
#!/bin/bash
# Batch create customers from a file
while IFS=, read -r name email; do
    stateset-direct customers create --name "$name" --email "$email" --apply
done < customers.csv

# Export orders to JSON
stateset-direct orders list --format json > orders.json

# Check inventory levels
stateset-direct inventory list --below-reorder | jq '.[] | .sku'
```

## Daemon Mode

Run iCommerce as a background service:

```bash
# Start as daemon (webhook server + autonomous engine)
stateset-daemon start

# Check daemon status
stateset-daemon status

# Stop daemon
stateset-daemon stop
```

The daemon runs the webhook server, billing executor, dispute resolver, and heartbeat monitor.

## Diagnostics

```bash
# Run environment diagnostics
stateset-doctor

# Auto-fix detected issues
stateset-doctor --fix

# Check database integrity
stateset-direct db check

# Show version and configuration
stateset --version
stateset-config show
```

## Full Reference

For CLI workflow patterns, see [Operations](operations.md). For all MCP tools exposed by the CLI, see [MCP Tools](mcp-tools.md). For data import, see [Data Migration](data-migration.md).
