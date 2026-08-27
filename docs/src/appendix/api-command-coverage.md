# API Command Coverage

This page is generated from the live tool modules in `cli/src/tools` and the CLI command registry in
`cli/src/commands/index.js`. Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_api_command_coverage.mjs
```

Machine-readable output lives at `artifacts/compatibility/api-command-coverage.json`.

## Summary

| Metric | Value |
| --- | --- |
| Tool modules | 87 |
| Command modules on disk | 87 |
| Command modules in registry | 87 |
| Tool-backed command modules | 8 |
| Uncovered tool modules | 0 |
| Uncovered tool-backed actions | 0 |
| Command-only modules | 0 |
| Registry mismatches | 0 |
| Fully covered | yes |

## Module Coverage

| Module | Coverage style | Actions | Aliases | Tool-backed action coverage |
| --- | --- | --- | --- | --- |
| `a2a` | tool-backed | 59 | 1 | 59/59 |
| `a2a-automation` | tool-backed | 32 | 2 | 32/32 |
| `a2a-intelligence` | tool-backed | 17 | 2 | 17/17 |
| `a2a-observability` | tool-backed | 15 | 2 | 15/15 |
| `a2a-platform` | tool-backed | 16 | 2 | 16/16 |
| `accounts-payable` | custom | 10 | 2 | - |
| `accounts-receivable` | custom | 8 | 2 | - |
| `activity-logs` | tool-backed | 5 | 0 | - |
| `agent-cards` | tool-backed | 5 | 2 | 5/5 |
| `agent-receipt` | tool-backed | 11 | 0 | - |
| `agent-runtime` | tool-backed | 29 | 2 | 29/29 |
| `analytics` | custom | 9 | 2 | - |
| `audit` | custom | 4 | 2 | - |
| `backorders` | custom | 9 | 2 | - |
| `carts` | custom | 27 | 2 | - |
| `catalog` | custom | 6 | 2 | - |
| `channels` | tool-backed | 8 | 0 | - |
| `checkout` | custom | 8 | 2 | - |
| `circuit-breaker` | custom | 8 | 2 | - |
| `companies` | tool-backed | 9 | 0 | - |
| `compliance` | custom | 6 | 2 | - |
| `connectors` | custom | 11 | 2 | - |
| `cost-accounting` | custom | 5 | 2 | - |
| `credit` | custom | 8 | 2 | - |
| `currency` | custom | 7 | 2 | - |
| `custom-objects` | custom | 12 | 2 | - |
| `customers` | custom | 5 | 2 | - |
| `cycle-counts` | tool-backed | 7 | 0 | - |
| `edi-documents` | tool-backed | 5 | 0 | - |
| `erc8004` | custom | 5 | 2 | - |
| `fixed-assets` | tool-backed | 9 | 0 | - |
| `fraud` | custom | 6 | 2 | - |
| `fulfillment` | custom | 14 | 2 | - |
| `general-ledger` | custom | 12 | 2 | - |
| `gift-cards` | custom | 7 | 2 | - |
| `import` | custom | 10 | 2 | - |
| `inbound-shipments` | tool-backed | 8 | 0 | - |
| `integration-field-mappings` | tool-backed | 8 | 0 | - |
| `integration-mappings` | tool-backed | 7 | 0 | - |
| `inventory` | custom | 7 | 3 | - |
| `invoices` | custom | 7 | 2 | - |
| `lots` | custom | 11 | 2 | - |
| `loyalty` | custom | 8 | 2 | - |
| `maintenance` | tool-backed | 5 | 0 | - |
| `manufacturing` | custom | 11 | 2 | - |
| `orders` | custom | 8 | 2 | - |
| `payment-obligations` | tool-backed | 7 | 0 | - |
| `payments` | custom | 19 | 2 | - |
| `policies` | custom | 5 | 2 | - |
| `prepayments` | tool-backed | 8 | 0 | - |
| `price-levels` | tool-backed | 9 | 0 | - |
| `price-schedules` | tool-backed | 10 | 0 | - |
| `print-stations` | tool-backed | 8 | 0 | - |
| `production-batches` | tool-backed | 8 | 0 | - |
| `products` | custom | 6 | 2 | - |
| `promotions` | custom | 9 | 1 | - |
| `proofs` | custom | 7 | 1 | - |
| `purgatory` | tool-backed | 6 | 0 | - |
| `quality` | custom | 15 | 2 | - |
| `receiving` | custom | 8 | 2 | - |
| `returns` | custom | 8 | 2 | - |
| `revenue-recognition` | tool-backed | 6 | 0 | - |
| `reviews` | custom | 8 | 2 | - |
| `search-config` | tool-backed | 7 | 0 | - |
| `segments` | custom | 7 | 2 | - |
| `serials` | custom | 8 | 2 | - |
| `shipments` | custom | 13 | 3 | - |
| `shipping-zones` | custom | 7 | 2 | - |
| `stablecoin` | custom | 4 | 2 | - |
| `stock-snapshots` | tool-backed | 5 | 0 | - |
| `store-credits` | custom | 5 | 2 | - |
| `subscriptions` | custom | 10 | 1 | - |
| `supplier-skus` | tool-backed | 7 | 0 | - |
| `suppliers` | custom | 10 | 2 | - |
| `sync` | custom | 20 | 2 | - |
| `tax` | custom | 8 | 2 | - |
| `topology-snapshots` | tool-backed | 5 | 0 | - |
| `transfer-orders` | tool-backed | 7 | 0 | - |
| `treasury` | custom | 6 | 2 | - |
| `units-of-measure` | tool-backed | 10 | 0 | - |
| `vector` | custom | 12 | 2 | - |
| `vendor-credits` | tool-backed | 8 | 0 | - |
| `vendor-returns` | tool-backed | 6 | 0 | - |
| `warehouse` | custom | 9 | 2 | - |
| `warranties` | custom | 7 | 2 | - |
| `wishlists` | custom | 6 | 2 | - |
| `x402` | tool-backed | 14 | 1 | 14/14 |

## Uncovered Tool Modules

| Module | Status | Actions | Aliases |
| --- | --- | --- | --- |
| None | - | - | - |

## Uncovered Tool-Backed Actions

| Module | Tool | Status |
| --- | --- | --- |
| None | - | - |
