# Agent Inventory

This page is generated from the live agent definitions in `cli/src/agent-definitions.js`
and validated against the MCP server export in `cli/src/mcp-server.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_agent_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/agent-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Total agents | 20 |
| Commerce MCP tools | 535 |
| Scaffold MCP tools | 13 |
| Agents with full commerce access | 1 |
| Agents with scoped tool sets | 19 |
| Scoped tool references | 202 |

## Supported MCP Servers

| MCP server | Tools |
| --- | --- |
| stateset-commerce | 535 |
| stateset-scaffold | 13 |

## Agent Registry

| Agent | Display name | Tool access | MCP servers | Description |
| --- | --- | --- | --- | --- |
| `agents` | Agents Agent | 39 named tools | `stateset-commerce` | Multi-agent runtime orchestration, A2A commerce, and agent lifecycle management |
| `analytics` | Analytics Agent | 10 named tools | `stateset-commerce` | Business intelligence and forecasting specialist |
| `checkout` | Checkout Agent | 16 named tools | `stateset-commerce` | Shopping cart and checkout flow specialist (Agentic Commerce Protocol) |
| `currency` | Currency Agent | 8 named tools | `stateset-commerce` | Multi-currency support and exchange rate management specialist |
| `customer-service` | Customer Service | All 535 commerce MCP tools | `stateset-commerce` | Full-service agent with access to all commerce tools |
| `inventory` | Inventory Agent | 6 named tools | `stateset-commerce` | Stock and inventory management specialist |
| `invoices` | Invoices Agent | 7 named tools | `stateset-commerce` | B2B invoice management and accounts receivable specialist |
| `manufacturing` | Manufacturing Agent | 13 named tools | `stateset-commerce` | Bill of Materials (BOM) and work order management specialist |
| `orders` | Orders Agent | 8 named tools | `stateset-commerce` | Order lifecycle management specialist |
| `payments` | Payments Agent | 7 named tools | `stateset-commerce` | Payment processing and refund management specialist |
| `promotions` | Promotions Agent | 12 named tools | `stateset-commerce` | Promotions, discounts, and coupon code management specialist |
| `returns` | Returns Agent | 7 named tools | `stateset-commerce` | Return request processing specialist |
| `shipments` | Shipments Agent | 5 named tools | `stateset-commerce` | Shipment tracking and delivery management specialist |
| `stablecoin` | Stablecoin Agent | 4 named tools | `stateset-commerce` | Native stablecoin wallet, balance, and payment specialist |
| `storefront` | Storefront Agent | 13 scaffold tools | `stateset-scaffold` | Creates e-commerce storefront websites using StateSet iCommerce |
| `subscriptions` | Subscriptions Agent | 17 named tools | `stateset-commerce` | Subscription plans, recurring billing, and customer subscription lifecycle management |
| `suppliers` | Suppliers Agent | 8 named tools | `stateset-commerce` | Supplier management and purchase order specialist |
| `sync` | Sync Agent | 7 named tools | `stateset-commerce` | Verifiable Event Sync (VES) management - sync local state with production sequencer |
| `tax` | Tax Agent | 9 named tools | `stateset-commerce` | Tax calculation and compliance specialist |
| `warranties` | Warranties Agent | 6 named tools | `stateset-commerce` | Product warranty and claims management specialist |
