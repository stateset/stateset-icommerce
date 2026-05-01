# MCP API Coverage

This page is generated from the Node binding surface in `bindings/node/index.d.ts`
and the shared MCP coverage model in `cli/src/coverage/mcp-api-coverage.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_mcp_api_coverage.mjs
```

Machine-readable output lives at `artifacts/compatibility/mcp-api-coverage.json`.

## Summary

| Metric | Value |
| --- | --- |
| Domain tool modules | 60 |
| Domain tools | 693 |
| Commerce getters | 32 |
| Mapped getters | 32 |
| Audited classes | 32 |
| Audited methods | 354 |
| Mapped audited methods | 354 |
| Fully covered | yes |

## Commerce Getter Coverage

| Getter | Mapped module | Tools |
| --- | --- | --- |
| `accountsPayable` | `accounts-payable` | 10 |
| `accountsReceivable` | `accounts-receivable` | 8 |
| `analytics` | `analytics` | 14 |
| `backorder` | `backorders` | 9 |
| `bom` | `manufacturing` | 11 |
| `carts` | `carts` | 30 |
| `costAccounting` | `cost-accounting` | 5 |
| `credit` | `credit` | 8 |
| `currency` | `currency` | 12 |
| `customObjects` | `custom-objects` | 12 |
| `customers` | `customers` | 3 |
| `fulfillment` | `fulfillment` | 14 |
| `generalLedger` | `general-ledger` | 12 |
| `inventory` | `inventory` | 6 |
| `invoices` | `invoices` | 7 |
| `lots` | `lots` | 11 |
| `orders` | `orders` | 6 |
| `payments` | `payments` | 19 |
| `products` | `products` | 4 |
| `promotions` | `promotions` | 15 |
| `purchaseOrders` | `suppliers` | 10 |
| `quality` | `quality` | 15 |
| `receiving` | `receiving` | 8 |
| `returns` | `returns` | 5 |
| `serials` | `serials` | 8 |
| `shipments` | `shipments` | 14 |
| `subscriptions` | `subscriptions` | 17 |
| `tax` | `tax` | 29 |
| `warehouse` | `warehouse` | 9 |
| `warranties` | `warranties` | 7 |
| `workOrders` | `manufacturing` | 11 |
| `x402` | `x402` | 14 |

## Audited Class Coverage

| Class | Methods | Mapped | Uncovered | Stale mappings | Invalid tool refs |
| --- | --- | --- | --- | --- | --- |
| `AccountsPayable` | 11 | 11 | 0 | 0 | 0 |
| `AccountsReceivable` | 8 | 8 | 0 | 0 | 0 |
| `Analytics` | 14 | 14 | 0 | 0 | 0 |
| `Backorders` | 10 | 10 | 0 | 0 | 0 |
| `Bom` | 7 | 7 | 0 | 0 | 0 |
| `Carts` | 32 | 32 | 0 | 0 | 0 |
| `CostAccounting` | 5 | 5 | 0 | 0 | 0 |
| `Credit` | 9 | 9 | 0 | 0 | 0 |
| `CurrencyOperations` | 15 | 15 | 0 | 0 | 0 |
| `Customers` | 5 | 5 | 0 | 0 | 0 |
| `CustomObjects` | 12 | 12 | 0 | 0 | 0 |
| `Fulfillment` | 14 | 14 | 0 | 0 | 0 |
| `GeneralLedger` | 13 | 13 | 0 | 0 | 0 |
| `Inventory` | 6 | 6 | 0 | 0 | 0 |
| `Invoices` | 8 | 8 | 0 | 0 | 0 |
| `Lots` | 12 | 12 | 0 | 0 | 0 |
| `Orders` | 7 | 7 | 0 | 0 | 0 |
| `Payments` | 8 | 8 | 0 | 0 | 0 |
| `Products` | 5 | 5 | 0 | 0 | 0 |
| `Promotions` | 17 | 17 | 0 | 0 | 0 |
| `PurchaseOrders` | 11 | 11 | 0 | 0 | 0 |
| `Quality` | 15 | 15 | 0 | 0 | 0 |
| `Receiving` | 9 | 9 | 0 | 0 | 0 |
| `Returns` | 6 | 6 | 0 | 0 | 0 |
| `Serials` | 9 | 9 | 0 | 0 | 0 |
| `Shipments` | 7 | 7 | 0 | 0 | 0 |
| `Subscriptions` | 19 | 19 | 0 | 0 | 0 |
| `Tax` | 18 | 18 | 0 | 0 | 0 |
| `Warehouse` | 10 | 10 | 0 | 0 | 0 |
| `Warranties` | 8 | 8 | 0 | 0 | 0 |
| `WorkOrders` | 7 | 7 | 0 | 0 | 0 |
| `X402` | 17 | 17 | 0 | 0 | 0 |

## Uncovered Commerce Getters

None.

## Stale Getter Mappings

None.

## Uncovered Audited Methods

None.

## Stale Audited Method Mappings

None.

## Invalid Audited Tool References

None.
