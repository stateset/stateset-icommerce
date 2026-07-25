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
| Domain tool modules | 87 |
| Domain tools | 923 |
| Commerce getters | 67 |
| Mapped getters | 67 |
| Audited classes | 32 |
| Audited methods | 386 |
| Mapped audited methods | 386 |
| Fully covered | yes |

## Commerce Getter Coverage

| Getter | Mapped module | Tools |
| --- | --- | --- |
| `accountsPayable` | `accounts-payable` | 11 |
| `accountsReceivable` | `accounts-receivable` | 8 |
| `activityLogs` | `activity-logs` | 5 |
| `analytics` | `analytics` | 14 |
| `backorder` | `backorders` | 9 |
| `bom` | `manufacturing` | 11 |
| `carts` | `carts` | 30 |
| `channels` | `channels` | 8 |
| `companies` | `companies` | 9 |
| `costAccounting` | `cost-accounting` | 5 |
| `credit` | `credit` | 8 |
| `currency` | `currency` | 12 |
| `customObjects` | `custom-objects` | 12 |
| `customers` | `customers` | 11 |
| `cycleCounts` | `cycle-counts` | 7 |
| `ediDocuments` | `edi-documents` | 5 |
| `erc8004` | `erc8004` | 5 |
| `fixedAssets` | `fixed-assets` | 9 |
| `fraud` | `fraud` | 6 |
| `fulfillment` | `fulfillment` | 14 |
| `generalLedger` | `general-ledger` | 17 |
| `giftCards` | `gift-cards` | 7 |
| `inboundShipments` | `inbound-shipments` | 8 |
| `integrationFieldMappings` | `integration-field-mappings` | 8 |
| `integrationMappings` | `integration-mappings` | 7 |
| `inventory` | `inventory` | 6 |
| `invoices` | `invoices` | 7 |
| `lots` | `lots` | 11 |
| `loyalty` | `loyalty` | 8 |
| `maintenance` | `maintenance` | 5 |
| `orders` | `orders` | 6 |
| `paymentObligations` | `payment-obligations` | 7 |
| `payments` | `payments` | 19 |
| `prepayments` | `prepayments` | 8 |
| `priceLevels` | `price-levels` | 9 |
| `priceSchedules` | `price-schedules` | 10 |
| `printStations` | `print-stations` | 8 |
| `productionBatches` | `production-batches` | 8 |
| `products` | `products` | 14 |
| `promotions` | `promotions` | 15 |
| `purchaseOrders` | `suppliers` | 10 |
| `purgatory` | `purgatory` | 6 |
| `quality` | `quality` | 15 |
| `receiving` | `receiving` | 8 |
| `returns` | `returns` | 12 |
| `revenueRecognition` | `revenue-recognition` | 6 |
| `reviews` | `reviews` | 7 |
| `searchConfig` | `search-config` | 7 |
| `segments` | `segments` | 6 |
| `serials` | `serials` | 8 |
| `shipments` | `shipments` | 14 |
| `shippingZones` | `shipping-zones` | 7 |
| `stockSnapshots` | `stock-snapshots` | 5 |
| `storeCredits` | `store-credits` | 5 |
| `subscriptions` | `subscriptions` | 17 |
| `supplierSkus` | `supplier-skus` | 7 |
| `tax` | `tax` | 29 |
| `topologySnapshots` | `topology-snapshots` | 5 |
| `transferOrders` | `transfer-orders` | 7 |
| `unitsOfMeasure` | `units-of-measure` | 10 |
| `vendorCredits` | `vendor-credits` | 8 |
| `vendorReturns` | `vendor-returns` | 6 |
| `warehouse` | `warehouse` | 9 |
| `warranties` | `warranties` | 7 |
| `wishlists` | `wishlists` | 6 |
| `workOrders` | `manufacturing` | 11 |
| `x402` | `x402` | 14 |

## Audited Class Coverage

| Class | Methods | Mapped | Uncovered | Stale mappings | Invalid tool refs |
| --- | --- | --- | --- | --- | --- |
| `AccountsPayable` | 12 | 12 | 0 | 0 | 0 |
| `AccountsReceivable` | 8 | 8 | 0 | 0 | 0 |
| `Analytics` | 14 | 14 | 0 | 0 | 0 |
| `Backorders` | 10 | 10 | 0 | 0 | 0 |
| `Bom` | 7 | 7 | 0 | 0 | 0 |
| `Carts` | 32 | 32 | 0 | 0 | 0 |
| `CostAccounting` | 5 | 5 | 0 | 0 | 0 |
| `Credit` | 9 | 9 | 0 | 0 | 0 |
| `CurrencyOperations` | 15 | 15 | 0 | 0 | 0 |
| `Customers` | 13 | 13 | 0 | 0 | 0 |
| `CustomObjects` | 12 | 12 | 0 | 0 | 0 |
| `Fulfillment` | 14 | 14 | 0 | 0 | 0 |
| `GeneralLedger` | 18 | 18 | 0 | 0 | 0 |
| `Inventory` | 6 | 6 | 0 | 0 | 0 |
| `Invoices` | 8 | 8 | 0 | 0 | 0 |
| `Lots` | 12 | 12 | 0 | 0 | 0 |
| `Orders` | 7 | 7 | 0 | 0 | 0 |
| `Payments` | 8 | 8 | 0 | 0 | 0 |
| `Products` | 16 | 16 | 0 | 0 | 0 |
| `Promotions` | 17 | 17 | 0 | 0 | 0 |
| `PurchaseOrders` | 11 | 11 | 0 | 0 | 0 |
| `Quality` | 15 | 15 | 0 | 0 | 0 |
| `Receiving` | 9 | 9 | 0 | 0 | 0 |
| `Returns` | 13 | 13 | 0 | 0 | 0 |
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

