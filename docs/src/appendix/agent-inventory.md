# Agent Inventory

This page is generated from the live agent definitions in `cli/src/agent-definitions.js`
and validated against the MCP server registry in `cli/src/mcp-server-registry.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_agent_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/agent-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Total agents | 20 |
| Commerce MCP tools | 938 |
| Scaffold MCP tools | 13 |
| x402 MCP tools | 5 |
| Agents with full commerce access | 1 |
| Agents with scoped tool sets | 19 |
| Scoped tool references | 301 |

## Supported MCP Servers

| MCP server | Tools | Source |
| --- | --- | --- |
| stateset-commerce | 938 | `cli/src/mcp-server.js` |
| stateset-scaffold | 13 | `cli/src/scaffold-server.js` |
| stateset-x402 | 5 | `cli/src/x402-mcp-server.js` |

## Agent Registry

| Agent | Display name | Tool access | MCP servers | Description |
| --- | --- | --- | --- | --- |
| `agents` | Agents Agent | 53 named tools | `stateset-commerce` | Multi-agent runtime orchestration, A2A commerce, and agent lifecycle management |
| `analytics` | Analytics Agent | 14 named tools | `stateset-commerce` | Business intelligence and forecasting specialist |
| `checkout` | Checkout Agent | 32 named tools | `stateset-commerce` | Shopping cart and protocol-neutral checkout flow specialist |
| `currency` | Currency Agent | 12 named tools | `stateset-commerce` | Multi-currency support and exchange rate management specialist |
| `customer-service` | Customer Service | All 938 commerce MCP tools | `stateset-commerce` | Full-service agent with access to all commerce tools |
| `inventory` | Inventory Agent | 6 named tools | `stateset-commerce` | Stock and inventory management specialist |
| `invoices` | Invoices Agent | 9 named tools | `stateset-commerce` | B2B invoice management and accounts receivable specialist |
| `manufacturing` | Manufacturing Agent | 13 named tools | `stateset-commerce` | Bill of Materials (BOM) and work order management specialist |
| `orders` | Orders Agent | 8 named tools | `stateset-commerce` | Order lifecycle management specialist |
| `payments` | Payments Agent | 21 named tools | `stateset-commerce` | Payment processing and refund management specialist |
| `promotions` | Promotions Agent | 17 named tools | `stateset-commerce` | Promotions, discounts, and coupon code management specialist |
| `returns` | Returns Agent | 7 named tools | `stateset-commerce` | Return request processing specialist |
| `shipments` | Shipments Agent | 16 named tools | `stateset-commerce` | Shipment tracking and delivery management specialist |
| `stablecoin` | Stablecoin Agent | 4 named tools | `stateset-commerce` | Native stablecoin wallet, balance, and payment specialist |
| `storefront` | Storefront Agent | 13 scaffold tools | `stateset-scaffold` | Creates e-commerce storefront websites using StateSet iCommerce |
| `subscriptions` | Subscriptions Agent | 19 named tools | `stateset-commerce` | Subscription plans, recurring billing, and customer subscription lifecycle management |
| `suppliers` | Suppliers Agent | 12 named tools | `stateset-commerce` | Supplier management and purchase order specialist |
| `sync` | Sync Agent | 7 named tools | `stateset-commerce` | Verifiable Event Sync (VES) management - sync local state with production sequencer |
| `tax` | Tax Agent | 29 named tools | `stateset-commerce` | Tax calculation and compliance specialist |
| `warranties` | Warranties Agent | 9 named tools | `stateset-commerce` | Product warranty and claims management specialist |
