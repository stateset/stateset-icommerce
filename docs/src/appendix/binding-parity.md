# Binding Parity

This page is generated from the engine's public API (`crates/stateset-embedded/src`) and the
binding sources under `bindings/`. Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_binding_parity.mjs
```

Machine-readable output lives at `artifacts/compatibility/binding-parity.json`. The package-level
inventory this builds on is [Binding API Inventory](binding-api-inventory.md).

## How exposure is measured

- **Domains** are the `Commerce` accessors that return a handle (`commerce.orders()`,
  `commerce.promotions()`, ...) plus the root `commerce` methods. Each domain's engine methods are
  the `pub fn` methods with a `self` receiver on the returned type. The engine exposes
  70 domains and 1227 methods.
- **Traced bindings** (Node, Python, Go): an engine method is *exposed* only when an exported
  binding method (or a helper it calls) actually calls it — `commerce.promotions().get_by_code(`.
  Names do not matter, so renames (`GetLevel` → `get_stock`) are credited correctly. A class
  counts for its primary domain; calls into other domains are recorded as cross-domain reach in the
  JSON but not counted.
- **Native reach** (the other FFI bindings): any engine call in the binding's Rust layer. The host
  language layer is not traced, so this is an upper bound on what users of that binding can reach.
- **Name-matched surfaces** (.NET, Swift, WASM host APIs): host method names mapped to engine
  names across conventions (`getByCode` / `GetByCode` / `get_by_code`) plus the alias map
  below. A match is a claim about the surface, not evidence of engine backing.
- **Exempt** methods are engine APIs that are not meant to cross a language boundary; they are
  listed with reasons in `bindings/parity-baseline.json`.

## Coverage summary

| Binding | Evidence | Gated | Exposed | Coverage | Parity vs Node |
| --- | --- | --- | --- | --- | --- |
| Node.js | traced: #[napi] methods -> engine calls | yes | 742/1219 | 60.9% | reference |
| Python | traced: #[pymethods] methods -> engine calls | yes | 593/1219 | 48.6% | 79.2% |
| Go | traced: Go methods -> C.stateset_* -> Rust FFI -> engine calls | yes | 80/1219 | 6.6% | 10.8% |
| .NET | native reach: engine calls anywhere in the Rust layer | no | 48/1219 | 3.9% | 6.3% |
| Java | native reach: engine calls anywhere in the Rust layer | no | 77/1219 | 6.3% | 9.6% |
| Kotlin | native reach: engine calls anywhere in the Rust layer | no | 61/1219 | 5% | 8% |
| PHP | native reach: engine calls anywhere in the Rust layer | no | 178/1219 | 14.6% | 21.7% |
| Ruby | native reach: engine calls anywhere in the Rust layer | no | 0/1219 | 0% | 0% |
| Swift | native reach: engine calls anywhere in the Rust layer | no | 51/1219 | 4.2% | 6.9% |
| WASM | native reach: engine calls anywhere in the Rust layer | no | 0/1219 | 0% | 0% |

Coverage is exposed / (engine methods − exempt). Parity vs Node is the share of Node-exposed engine
methods the binding also exposes.

## Name-matched host surfaces

| Binding | Host methods | Name-matched | Coverage | Unmatched host methods |
| --- | --- | --- | --- | --- |
| .NET | 245 | 220/1219 | 18% | 7 |
| Swift | 71 | 71/1219 | 5.8% | 0 |
| WASM | 148 | 134/1219 | 11% | 14 |

## Per-domain matrix

Cells are exposed / applicable engine methods.

| Domain | Engine methods | Exempt | Node.js | Python | Go | .NET | Java | Kotlin | PHP | Ruby | Swift | WASM |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `commerce` | 21 | 4 | 8/17 | 4/17 | 0/17 | 0/17 | 0/17 | 0/17 | 0/17 | 0/17 | 0/17 | 0/17 |
| `accounts_payable` | 38 | 0 | 13/38 | 8/38 | 0/38 | 2/38 | 3/38 | 3/38 | 4/38 | 0/38 | 3/38 | 0/38 |
| `accounts_receivable` | 28 | 0 | 8/28 | 3/28 | 0/28 | 2/28 | 3/28 | 2/28 | 3/28 | 0/28 | 2/28 | 0/28 |
| `activity_logs` | 5 | 0 | 5/5 | 5/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 | 0/5 |
| `agent` | 4 | 4 | exempt | exempt | exempt | exempt | exempt | exempt | exempt | exempt | exempt | exempt |
| `analytics` | 14 | 0 | 14/14 | 14/14 | 3/14 | 3/14 | 1/14 | 3/14 | 1/14 | 0/14 | 3/14 | 0/14 |
| `backorder` | 21 | 0 | 21/21 | 7/21 | 0/21 | 2/21 | 4/21 | 2/21 | 5/21 | 0/21 | 2/21 | 0/21 |
| `bom` | 13 | 0 | 7/13 | 7/13 | 6/13 | 0/13 | 0/13 | 0/13 | 9/13 | 0/13 | 0/13 | 0/13 |
| `carts` | 33 | 0 | 32/33 | 32/33 | 3/33 | 3/33 | 4/33 | 3/33 | 5/33 | 0/33 | 3/33 | 0/33 |
| `channels` | 9 | 0 | 9/9 | 9/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 |
| `companies` | 11 | 0 | 11/11 | 11/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 |
| `cost_accounting` | 27 | 0 | 5/27 | 5/27 | 0/27 | 2/27 | 3/27 | 2/27 | 3/27 | 0/27 | 2/27 | 0/27 |
| `credit` | 29 | 0 | 9/29 | 5/29 | 0/29 | 1/29 | 3/29 | 2/29 | 3/29 | 0/29 | 2/29 | 0/29 |
| `currency` | 16 | 0 | 15/16 | 15/16 | 4/16 | 0/16 | 0/16 | 0/16 | 7/16 | 0/16 | 0/16 | 0/16 |
| `custom_objects` | 12 | 0 | 12/12 | 12/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 |
| `customers` | 14 | 0 | 13/14 | 5/14 | 4/14 | 4/14 | 5/14 | 4/14 | 5/14 | 0/14 | 4/14 | 0/14 |
| `edi_documents` | 6 | 0 | 5/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 |
| `erc8004` | 26 | 0 | 17/26 | 17/26 | 0/26 | 0/26 | 0/26 | 0/26 | 0/26 | 0/26 | 0/26 | 0/26 |
| `fixed_assets` | 11 | 0 | 11/11 | 11/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 |
| `fraud` | 11 | 0 | 11/11 | 11/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 |
| `fulfillment` | 46 | 0 | 14/46 | 6/46 | 0/46 | 0/46 | 3/46 | 0/46 | 4/46 | 0/46 | 0/46 | 0/46 |
| `general_ledger` | 44 | 0 | 18/44 | 12/44 | 0/44 | 2/44 | 3/44 | 2/44 | 5/44 | 0/44 | 2/44 | 0/44 |
| `gift_cards` | 10 | 0 | 10/10 | 10/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 |
| `inbound_shipments` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `integration_field_mappings` | 9 | 0 | 9/9 | 9/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 |
| `integration_mappings` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `inventory` | 16 | 0 | 6/16 | 6/16 | 3/16 | 3/16 | 8/16 | 3/16 | 8/16 | 0/16 | 3/16 | 0/16 |
| `invoices` | 21 | 0 | 8/21 | 8/21 | 7/21 | 0/21 | 0/21 | 0/21 | 10/21 | 0/21 | 0/21 | 0/21 |
| `kernel_audit` | 3 | 0 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 | 0/3 |
| `lots` | 35 | 0 | 12/35 | 6/35 | 0/35 | 2/35 | 4/35 | 2/35 | 5/35 | 0/35 | 2/35 | 0/35 |
| `loyalty` | 14 | 0 | 14/14 | 14/14 | 0/14 | 0/14 | 0/14 | 0/14 | 0/14 | 0/14 | 0/14 | 0/14 |
| `maintenance` | 10 | 0 | 7/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 |
| `orders` | 18 | 0 | 10/18 | 8/18 | 5/18 | 4/18 | 8/18 | 6/18 | 8/18 | 0/18 | 4/18 | 0/18 |
| `payment_obligations` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `payments` | 23 | 0 | 12/23 | 11/23 | 6/23 | 1/23 | 1/23 | 3/23 | 1/23 | 0/23 | 1/23 | 0/23 |
| `prepayments` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `price_levels` | 9 | 0 | 9/9 | 9/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 |
| `price_schedules` | 10 | 0 | 10/10 | 10/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 | 0/10 |
| `print_stations` | 9 | 0 | 9/9 | 9/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 |
| `production_batches` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `products` | 17 | 0 | 16/17 | 6/17 | 4/17 | 3/17 | 4/17 | 3/17 | 5/17 | 0/17 | 3/17 | 0/17 |
| `promotions` | 19 | 0 | 19/19 | 17/19 | 0/19 | 0/19 | 0/19 | 0/19 | 9/19 | 0/19 | 0/19 | 0/19 |
| `purchase_orders` | 26 | 0 | 11/26 | 11/26 | 10/26 | 0/26 | 0/26 | 0/26 | 11/26 | 0/26 | 0/26 | 0/26 |
| `purgatory` | 7 | 0 | 7/7 | 7/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 |
| `quality` | 33 | 0 | 17/33 | 10/33 | 0/33 | 1/33 | 4/33 | 2/33 | 4/33 | 0/33 | 2/33 | 0/33 |
| `receiving` | 24 | 0 | 10/24 | 5/24 | 0/24 | 0/24 | 4/24 | 0/24 | 4/24 | 0/24 | 0/24 | 0/24 |
| `returns` | 15 | 0 | 13/15 | 6/15 | 6/15 | 2/15 | 6/15 | 6/15 | 5/15 | 0/15 | 2/15 | 0/15 |
| `revenue_recognition` | 9 | 0 | 9/9 | 9/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 |
| `reviews` | 9 | 0 | 9/9 | 9/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 | 0/9 |
| `search_config` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `segments` | 11 | 0 | 10/11 | 10/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 |
| `serials` | 36 | 0 | 9/36 | 4/36 | 0/36 | 2/36 | 3/36 | 2/36 | 4/36 | 0/36 | 2/36 | 0/36 |
| `shipments` | 24 | 0 | 10/24 | 7/24 | 6/24 | 6/24 | 0/24 | 8/24 | 9/24 | 0/24 | 6/24 | 0/24 |
| `shipping_zones` | 12 | 0 | 12/12 | 12/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 | 0/12 |
| `stock_snapshots` | 6 | 0 | 6/6 | 6/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 |
| `store_credits` | 7 | 0 | 7/7 | 7/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 |
| `subscriptions` | 33 | 0 | 19/33 | 16/33 | 0/33 | 0/33 | 0/33 | 0/33 | 11/33 | 0/33 | 0/33 | 0/33 |
| `supplier_skus` | 7 | 0 | 7/7 | 7/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 |
| `tax` | 21 | 0 | 18/21 | 14/21 | 0/21 | 0/21 | 0/21 | 0/21 | 8/21 | 0/21 | 0/21 | 0/21 |
| `topology_snapshots` | 6 | 0 | 6/6 | 6/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 | 0/6 |
| `transfer_orders` | 7 | 0 | 7/7 | 7/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 |
| `units_of_measure` | 11 | 0 | 11/11 | 11/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 | 0/11 |
| `vector` | 22 | 0 | 15/22 | 9/22 | 0/22 | 0/22 | 0/22 | 0/22 | 0/22 | 0/22 | 0/22 | 0/22 |
| `vendor_credits` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `vendor_returns` | 7 | 0 | 7/7 | 7/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 | 0/7 |
| `warehouse` | 52 | 0 | 17/52 | 12/52 | 0/52 | 3/52 | 3/52 | 3/52 | 4/52 | 0/52 | 3/52 | 0/52 |
| `warranties` | 24 | 0 | 8/24 | 8/24 | 7/24 | 0/24 | 0/24 | 0/24 | 9/24 | 0/24 | 0/24 | 0/24 |
| `wishlists` | 8 | 0 | 8/8 | 8/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 | 0/8 |
| `work_orders` | 23 | 0 | 7/23 | 7/23 | 6/23 | 0/23 | 0/23 | 0/23 | 9/23 | 0/23 | 0/23 | 0/23 |
| `x402` | 69 | 0 | 17/69 | 0/69 | 0/69 | 0/69 | 0/69 | 0/69 | 0/69 | 0/69 | 0/69 | 0/69 |

## Gate

`scripts/ci/check_release_hygiene.sh` runs `node ./scripts/ci/generate_binding_parity.mjs --check`,
which fails when this page or the JSON artifact is stale, or when the current state disagrees with
`bindings/parity-baseline.json`:

1. a gated binding **loses** an engine method it exposed;
2. Node exposes a method that a gated binding lacks and the baseline does **not list the gap** —
   new gaps must be explicit;
3. a listed gap has been **closed** (stale entry — the gap list only shrinks);
4. a binding **gains** a method the baseline has not recorded (so a later loss is caught).

`--update-baseline` rewrites the baseline from the current state and refuses to record losses
unless `--allow-loss` is also passed. Gated bindings: `node`, `python`, `go`
(reference: `node`).

| Binding | Known gaps vs Node (baseline) |
| --- | --- |
| python | 154 |
| go | 662 |

## Exemptions

| Scope | Engine method | Reason |
| --- | --- | --- |
| all | `commerce.database` | returns a borrowed &dyn Database trait object; no language boundary can carry it |
| all | `commerce.events` | returns a borrowed &EventSystem; bindings expose subscribe_events / webhooks instead |
| all | `commerce.metrics` | returns a borrowed &Metrics handle; metrics_snapshot is the portable form |
| all | `commerce.kernel_executor` | returns the typed SqliteKernelExecutor; bindings use execute_kernel_command |
| all | `agent.command` | generic over a Rust payload type (T: Serialize); bindings use execute_kernel_command |
| all | `agent.executor` | returns a borrowed &SqliteKernelExecutor |
| all | `agent.identity` | returns a borrowed &EconomicAgent |
| all | `agent.transactions` | returns a borrowed &CanonicalTransactionApi |

## Name mapping

Host methods whose name differs from the engine method they call. These do not affect coverage
(exposure is traced), but undocumented renames are where binding docs drift from the engine docs.
The documented alias map lives under `aliases` in `bindings/parity-baseline.json` and is also used for the
name-matched surfaces.

| Alias (domain.host_name) | Engine method |
| --- | --- |
| `analytics.get_sales_summary` | `analytics.sales_summary` |
| `analytics.get_top_customers` | `analytics.top_customers` |
| `analytics.get_top_products` | `analytics.top_products` |
| `currency.get_base_currency` | `currency.base_currency` |
| `currency.get_enabled_currencies` | `currency.enabled_currencies` |
| `inventory.get_level` | `inventory.get_stock` |
| `lots.create_lot` | `lots.create` |
| `lots.get_lot` | `lots.get` |
| `lots.get_lots_by_sku` | `lots.get_available_lots_for_sku` |
| `lots.list_lots` | `lots.list` |
| `lots.quarantine_lot` | `lots.quarantine` |
| `maintenance.backup` | `maintenance.backup_to` |
| `maintenance.import` | `maintenance.import_from_file` |
| `maintenance.is_supported` | `maintenance.supports_backup` |
| `maintenance.restore` | `maintenance.restore_from` |
| `payments.complete` | `payments.mark_completed` |
| `payments.fail` | `payments.mark_failed` |
| `payments.refund` | `payments.create_refund` |
| `promotions.activate_promotion` | `promotions.activate` |
| `promotions.apply_promotions` | `promotions.apply` |
| `promotions.create_promotion` | `promotions.create` |
| `promotions.deactivate_promotion` | `promotions.deactivate` |
| `promotions.delete_promotion` | `promotions.delete` |
| `promotions.get_active_promotions` | `promotions.get_active` |
| `promotions.get_promotion` | `promotions.get` |
| `promotions.get_promotion_by_code` | `promotions.get_by_code` |
| `promotions.list_promotions` | `promotions.list` |
| `promotions.update_promotion` | `promotions.update` |
| `revenue_recognition.recognize` | `revenue_recognition.recognize_period` |
| `shipments.deliver` | `shipments.mark_delivered` |
| `subscriptions.skip_billing` | `subscriptions.skip_next_cycle` |
| `vector.index_all_customers` | `vector.index_customers` |
| `vector.index_all_inventory` | `vector.index_inventory_items` |
| `vector.index_all_orders` | `vector.index_orders` |
| `vector.index_all_products` | `vector.index_products` |

| Binding | Host method | Engine method(s) called | Documented |
| --- | --- | --- | --- |
| go | `AnalyticsAPI.GetSalesSummary` | `analytics.sales_summary` | yes |
| go | `AnalyticsAPI.GetTopCustomers` | `analytics.top_customers` | yes |
| go | `AnalyticsAPI.GetTopProducts` | `analytics.top_products` | yes |
| go | `InventoryAPI.GetLevel` | `inventory.get_stock` | yes |
| go | `PaymentsAPI.Complete` | `payments.mark_completed` | yes |
| go | `PaymentsAPI.Fail` | `payments.mark_failed` | yes |
| go | `PaymentsAPI.Refund` | `payments.create_refund` | yes |
| go | `ProductsAPI.Publish` | `products.update` | no |
| go | `ShipmentsAPI.Deliver` | `shipments.mark_delivered` | yes |
| go | `SuppliersAPI.Create` | `purchase_orders.create_supplier` | no |
| go | `SuppliersAPI.Get` | `purchase_orders.get_supplier` | no |
| go | `SuppliersAPI.List` | `purchase_orders.list_suppliers` | no |
| node | `Carts.addItemExact` | `carts.add_item` | no |
| node | `CurrencyOperations.getBaseCurrency` | `currency.base_currency` | yes |
| node | `CurrencyOperations.getEnabledCurrencies` | `currency.enabled_currencies` | yes |
| node | `CycleCounts.cancel` | `warehouse.cancel_cycle_count` | no |
| node | `CycleCounts.complete` | `warehouse.complete_cycle_count` | no |
| node | `CycleCounts.create` | `warehouse.create_cycle_count` | no |
| node | `CycleCounts.get` | `warehouse.get_cycle_count` | no |
| node | `CycleCounts.list` | `warehouse.list_cycle_counts` | no |
| node | `CycleCounts.recordCounts` | `warehouse.record_cycle_counts` | no |
| node | `CycleCounts.start` | `warehouse.start_cycle_count` | no |
| node | `Events.subscribe` | `commerce.subscribe_events` | no |
| node | `Events.subscribeFiltered` | `commerce.subscribe_events` | no |
| node | `Maintenance.backup` | `maintenance.backup_to` | yes |
| node | `Maintenance.export` | `maintenance.export_to_file_with` | no |
| node | `Maintenance.exportToFile` | `maintenance.export_to_file_with` | no |
| node | `Maintenance.import` | `maintenance.import_from_file` | yes |
| node | `Maintenance.isSupported` | `maintenance.supports_backup` | yes |
| node | `Maintenance.listPortableDomains` | `maintenance.exportable_domains`, `maintenance.importable_domains` | no |
| node | `Maintenance.restore` | `maintenance.restore_from` | yes |
| node | `Orders.createExact` | `orders.create`, `orders.create_from_cart` | no |
| node | `Payments.createExact` | `payments.create` | no |
| node | `Payments.createRefundExact` | `payments.create_refund`, `payments.get` | no |
| node | `Promotions.applyToCart` | `commerce.apply_cart_promotions` | no |
| node | `RevenueRecognition.recognize` | `revenue_recognition.recognize_period` | yes |
| node | `Shipments.addItem` | `shipments.add_item_with_version` | no |
| node | `Shipments.deliver` | `shipments.mark_delivered`, `shipments.update` | yes |
| node | `Shipments.removeItem` | `shipments.remove_item_with_version` | no |
| node | `Subscriptions.skipBilling` | `subscriptions.skip_next_cycle` | yes |
| node | `VectorSearch.indexAllCustomers` | `vector.index_customers` | yes |
| node | `VectorSearch.indexAllInventory` | `vector.index_inventory_items` | yes |
| node | `VectorSearch.indexAllOrders` | `vector.index_orders` | yes |
| node | `VectorSearch.indexAllProducts` | `vector.index_products` | yes |
| python | `CycleCounts.cancel` | `warehouse.cancel_cycle_count` | no |
| python | `CycleCounts.complete` | `warehouse.complete_cycle_count` | no |
| python | `CycleCounts.create` | `warehouse.create_cycle_count` | no |
| python | `CycleCounts.get` | `warehouse.get_cycle_count` | no |
| python | `CycleCounts.list` | `warehouse.list_cycle_counts` | no |
| python | `CycleCounts.record_counts` | `warehouse.record_cycle_counts` | no |
| python | `CycleCounts.start` | `warehouse.start_cycle_count` | no |
| python | `LotsApi.create_lot` | `lots.create` | yes |
| python | `LotsApi.get_lot` | `lots.get` | yes |
| python | `LotsApi.get_lots_by_sku` | `lots.get_available_lots_for_sku` | yes |
| python | `LotsApi.list_lots` | `lots.list` | yes |
| python | `LotsApi.quarantine_lot` | `lots.quarantine` | yes |
| python | `Payments.complete` | `payments.mark_completed` | yes |
| python | `Payments.create_exact` | `payments.create` | no |
| python | `Payments.create_refund_exact` | `payments.create_refund`, `payments.get` | no |
| python | `PromotionsApi.apply_to_cart` | `commerce.apply_cart_promotions` | no |
| python | `RevenueRecognition.recognize` | `revenue_recognition.recognize_period` | yes |
| python | `VectorSearch.index_all_customers` | `vector.index_customers` | yes |
| python | `VectorSearch.index_all_products` | `vector.index_products` | yes |

## Hollow signals (report only)

Struct literals in engine-backed bindings that hand the engine an always-empty collection
(`items: vec![]` and similar). Each is a place where a binding may accept a call that can never
carry the data the engine supports — the defect class behind Python's line-item-less invoices,
purchase orders and promotions. They are not a CI failure; review each one.

None found.

## Unmatched host methods (name-matched surfaces)

| Binding | Host method |
| --- | --- |
| .NET | `AccountsReceivableApi.ListReceivables` |
| .NET | `CostAccountingApi.ListCostEntries` |
| .NET | `CreditApi.GetCreditLimit` |
| .NET | `CreditApi.SetCreditLimit` |
| .NET | `FulfillmentApi.ListPickLists` |
| .NET | `SerialsApi.ListSerials` |
| .NET | `SerialsApi.RegisterSerial` |
| WASM | `Orders.getItems` |
| WASM | `Promotions.countCoupons` |
| WASM | `Promotions.countPromotions` |
| WASM | `Promotions.new` |
| WASM | `Subscriptions.countPlans` |
| WASM | `Subscriptions.countSubscriptions` |
| WASM | `Tax.countExemptions` |
| WASM | `Tax.countJurisdictions` |
| WASM | `Tax.countRates` |
| WASM | `Tax.getCanadianTaxInfo` |
| WASM | `Tax.getEuVatInfo` |
| WASM | `Tax.getUsStateInfo` |
| WASM | `Tax.isEuCountry` |
| WASM | `WorkOrders.recordOutput` |

## Per-domain detail (traced bindings)

### `commerce` (root methods)

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `apply_cart_promotions` | yes | yes | — |
| `backend` | — | — | — |
| `calculate_cart_tax` | — | — | — |
| `database` | exempt | exempt | exempt |
| `economic_budget_status` | yes | yes | — |
| `emit_event` | — | — | — |
| `events` | exempt | exempt | exempt |
| `execute_kernel_command` | yes | yes | — |
| `health_check` | — | — | — |
| `kernel_executor` | exempt | exempt | exempt |
| `list_webhooks` | yes | — | — |
| `metrics` | exempt | exempt | exempt |
| `metrics_snapshot` | — | — | — |
| `provision_economic_budget` | yes | yes | — |
| `register_webhook` | yes | — | — |
| `register_webhook_strict` | — | — | — |
| `subscribe_events` | yes | — | — |
| `transactions` | — | — | — |
| `try_register_webhook` | — | — | — |
| `unregister_webhook` | yes | — | — |
| `webhook_deliveries` | — | — | — |

### `commerce.accounts_payable()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_bill_item` | — | — | — |
| `approve_bill` | yes | yes | — |
| `approve_payment_run` | — | — | — |
| `cancel_bill` | yes | — | — |
| `cancel_payment_run` | — | — | — |
| `clear_payment` | — | — | — |
| `count_bills` | yes | — | — |
| `count_payments` | — | — | — |
| `create_bill` | yes | yes | — |
| `create_bills_batch` | — | — | — |
| `create_payment` | — | — | — |
| `create_payment_run` | — | — | — |
| `delete_bill` | — | — | — |
| `dispute_bill` | — | — | — |
| `get_aging_summary` | yes | yes | — |
| `get_bill` | yes | yes | — |
| `get_bill_by_number` | yes | — | — |
| `get_bill_items` | yes | — | — |
| `get_bills_batch` | — | — | — |
| `get_bills_due_soon` | yes | — | — |
| `get_overdue_bills` | yes | yes | — |
| `get_payment` | — | — | — |
| `get_payment_allocations` | — | — | — |
| `get_payment_by_number` | — | — | — |
| `get_payment_run` | — | — | — |
| `get_payment_run_bills` | — | — | — |
| `get_payments_for_bill` | — | — | — |
| `get_supplier_summary` | — | — | — |
| `get_total_outstanding` | yes | — | — |
| `list_bills` | yes | yes | — |
| `list_payment_runs` | — | — | — |
| `list_payments` | — | — | — |
| `pay_bill` | — | yes | — |
| `process_payment_run` | — | — | — |
| `remove_bill_item` | — | — | — |
| `three_way_match` | yes | yes | — |
| `update_bill` | — | — | — |
| `void_payment` | — | — | — |

### `commerce.accounts_receivable()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `apply_credit_memo` | — | — | — |
| `apply_payment_to_invoices` | — | — | — |
| `create_credit_memo` | yes | yes | — |
| `create_write_off` | — | — | — |
| `generate_statement` | — | — | — |
| `get_aging_report` | — | — | — |
| `get_aging_summary` | yes | yes | — |
| `get_average_days_to_pay` | — | — | — |
| `get_credit_memo` | yes | — | — |
| `get_credit_memo_by_number` | — | — | — |
| `get_customer_aging` | — | — | — |
| `get_customer_summary` | — | — | — |
| `get_customers_batch` | — | — | — |
| `get_dso` | yes | yes | — |
| `get_invoices_due_for_dunning` | — | — | — |
| `get_payment_applications` | — | — | — |
| `get_total_outstanding` | yes | — | — |
| `get_unapplied_credits` | yes | — | — |
| `get_write_off` | — | — | — |
| `list_collection_activities` | — | — | — |
| `list_credit_memos` | yes | — | — |
| `list_write_offs` | — | — | — |
| `log_collection_activity` | — | — | — |
| `reverse_write_off` | — | — | — |
| `send_dunning_letter` | — | — | — |
| `unapply_payment` | — | — | — |
| `update_collection_status` | — | — | — |
| `void_credit_memo` | yes | — | — |

### `commerce.activity_logs()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `get` | yes | yes | — |
| `history_for_subject` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `record` | yes | yes | — |

### `commerce.agent()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `command` | exempt | exempt | exempt |
| `executor` | exempt | exempt | exempt |
| `identity` | exempt | exempt | exempt |
| `transactions` | exempt | exempt | exempt |

### `commerce.analytics()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `customer_metrics` | yes | yes | — |
| `demand_forecast` | yes | yes | — |
| `fulfillment_metrics` | yes | yes | — |
| `inventory_health` | yes | yes | — |
| `inventory_movement` | yes | yes | — |
| `low_stock_items` | yes | yes | — |
| `order_status_breakdown` | yes | yes | — |
| `product_performance` | yes | yes | — |
| `return_metrics` | yes | yes | — |
| `revenue_by_period` | yes | yes | — |
| `revenue_forecast` | yes | yes | — |
| `sales_summary` | yes | yes | yes |
| `top_customers` | yes | yes | yes |
| `top_products` | yes | yes | yes |

### `commerce.backorder()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `allocate_backorder` | yes | — | — |
| `auto_allocate_inventory` | yes | — | — |
| `cancel_backorder` | yes | yes | — |
| `confirm_allocation` | yes | — | — |
| `count_pending` | yes | — | — |
| `create_backorder` | yes | yes | — |
| `expire_allocations` | yes | — | — |
| `fulfill_backorder` | yes | yes | — |
| `get_allocations` | yes | — | — |
| `get_backorder` | yes | yes | — |
| `get_backorder_by_number` | yes | — | — |
| `get_backorders_for_customer` | yes | — | — |
| `get_backorders_for_order` | yes | — | — |
| `get_backorders_for_sku` | yes | — | — |
| `get_fulfillment_history` | yes | — | — |
| `get_overdue_backorders` | yes | yes | — |
| `get_sku_summary` | yes | — | — |
| `get_summary` | yes | yes | — |
| `list_backorders` | yes | yes | — |
| `release_allocation` | yes | — | — |
| `update_backorder` | yes | — | — |

### `commerce.bom()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `activate` | yes | yes | yes |
| `add_component` | yes | yes | yes |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `delete` | — | — | — |
| `for_product` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | — | — | — |
| `get_components` | yes | yes | yes |
| `list` | yes | yes | yes |
| `remove_component` | — | — | — |
| `update` | — | — | — |
| `update_component` | — | — | — |

### `commerce.carts()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `abandon` | yes | yes | — |
| `add_item` | yes | yes | yes |
| `apply_discount` | yes | yes | — |
| `begin_checkout` | yes | yes | — |
| `cancel` | yes | yes | — |
| `clear_items` | yes | yes | — |
| `complete` | yes | yes | — |
| `complete_settled_externally` | — | — | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `delete` | yes | yes | — |
| `expire` | yes | yes | — |
| `for_customer` | yes | yes | — |
| `get` | yes | yes | yes |
| `get_abandoned` | yes | yes | — |
| `get_by_number` | yes | yes | — |
| `get_expired` | yes | yes | — |
| `get_items` | yes | yes | — |
| `get_shipping_rates` | yes | yes | — |
| `list` | yes | yes | — |
| `mark_ready_for_payment` | yes | yes | — |
| `recalculate` | yes | yes | — |
| `release_inventory` | yes | yes | — |
| `remove_discount` | yes | yes | — |
| `remove_item` | yes | yes | — |
| `reserve_inventory` | yes | yes | — |
| `set_billing_address` | yes | yes | — |
| `set_payment` | yes | yes | — |
| `set_shipping` | yes | yes | — |
| `set_shipping_address` | yes | yes | — |
| `set_tax` | yes | yes | — |
| `update` | yes | yes | — |
| `update_item` | yes | yes | — |

### `commerce.channels()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_product_mappings` | yes | yes | — |
| `set_lock` | yes | yes | — |
| `sync_products` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.companies()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `create_contact` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `get_contact` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_addresses` | yes | yes | — |
| `list_contacts` | yes | yes | — |
| `list_price_overrides` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.cost_accounting()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `apply_adjustment` | — | — | — |
| `approve_adjustment` | — | — | — |
| `calculate_rollup` | — | — | — |
| `create_adjustment` | — | — | — |
| `create_cost_layer` | — | — | — |
| `get_adjustment` | — | — | — |
| `get_cost_layer` | — | — | — |
| `get_inventory_valuation` | — | yes | — |
| `get_item_cost` | yes | yes | — |
| `get_layers_remaining` | — | — | — |
| `get_rollup` | — | — | — |
| `get_sku_cost_summary` | — | — | — |
| `get_total_inventory_value` | yes | yes | — |
| `get_variance_summary` | — | — | — |
| `issue_fifo` | — | — | — |
| `issue_lifo` | — | — | — |
| `list_adjustments` | — | — | — |
| `list_cost_layers` | — | — | — |
| `list_cost_transactions` | — | — | — |
| `list_item_costs` | yes | — | — |
| `list_variances` | — | — | — |
| `record_cost_transaction` | — | — | — |
| `record_variance` | — | — | — |
| `reject_adjustment` | — | — | — |
| `set_item_cost` | yes | yes | — |
| `update_average_cost` | yes | yes | — |
| `update_last_cost` | — | — | — |

### `commerce.credit()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `adjust_credit_limit` | yes | yes | — |
| `apply_payment` | — | — | — |
| `charge_credit` | — | — | — |
| `check_credit` | yes | yes | — |
| `create_credit_account` | yes | yes | — |
| `get_active_holds` | — | — | — |
| `get_aging_report` | — | — | — |
| `get_application` | — | — | — |
| `get_credit_account` | yes | — | — |
| `get_credit_account_by_customer` | yes | yes | — |
| `get_customer_summary` | — | — | — |
| `get_hold` | — | — | — |
| `get_holds_for_order` | — | — | — |
| `get_over_limit_customers` | yes | yes | — |
| `list_applications` | — | — | — |
| `list_credit_accounts` | yes | — | — |
| `list_holds` | — | — | — |
| `list_transactions` | — | — | — |
| `place_hold` | — | — | — |
| `reactivate_credit_account` | yes | — | — |
| `record_transaction` | — | — | — |
| `release_credit_reservation` | — | — | — |
| `release_hold` | — | — | — |
| `reserve_credit` | — | — | — |
| `review_application` | — | — | — |
| `submit_application` | — | — | — |
| `suspend_credit_account` | yes | — | — |
| `update_credit_account` | — | — | — |
| `withdraw_application` | — | — | — |

### `commerce.currency()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `base_currency` | yes | yes | — |
| `convert` | yes | yes | yes |
| `convert_amount` | — | — | — |
| `delete_rate` | yes | yes | — |
| `enable_currencies` | yes | yes | — |
| `enabled_currencies` | yes | yes | — |
| `format` | yes | yes | — |
| `get_rate` | yes | yes | yes |
| `get_rates_for` | yes | yes | — |
| `get_settings` | yes | yes | yes |
| `is_enabled` | yes | yes | — |
| `list_rates` | yes | yes | — |
| `set_base_currency` | yes | yes | — |
| `set_rate` | yes | yes | yes |
| `set_rates` | yes | yes | — |
| `update_settings` | yes | yes | — |

### `commerce.custom_objects()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create_object` | yes | yes | — |
| `create_type` | yes | yes | — |
| `delete_object` | yes | yes | — |
| `delete_type` | yes | yes | — |
| `get_object` | yes | yes | — |
| `get_object_by_handle` | yes | yes | — |
| `get_type` | yes | yes | — |
| `get_type_by_handle` | yes | yes | — |
| `list_objects` | yes | yes | — |
| `list_types` | yes | yes | — |
| `update_object` | yes | yes | — |
| `update_type` | yes | yes | — |

### `commerce.customers()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_address` | yes | — | — |
| `anonymize` | — | — | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `delete` | yes | — | yes |
| `delete_address` | yes | — | — |
| `find_or_create` | yes | — | — |
| `get` | yes | yes | yes |
| `get_addresses` | yes | — | — |
| `get_by_email` | yes | yes | — |
| `list` | yes | yes | yes |
| `set_default_address` | yes | — | — |
| `update` | yes | — | — |
| `update_address` | yes | — | — |

### `commerce.edi_documents()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | — | — |
| `get` | yes | — | — |
| `is_supported` | — | — | — |
| `list` | yes | — | — |
| `set_status` | yes | — | — |
| `summary` | yes | — | — |

### `commerce.erc8004()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `agent_validations` | — | — | — |
| `append_feedback_response` | — | — | — |
| `clear_agent_wallet` | yes | yes | — |
| `count_identities` | yes | yes | — |
| `delete_identity_metadata` | — | — | — |
| `feedback_clients` | — | — | — |
| `feedback_response_count` | — | — | — |
| `feedback_summary` | yes | yes | — |
| `get_identity` | yes | yes | — |
| `get_identity_by_wallet` | yes | yes | — |
| `get_identity_metadata` | — | — | — |
| `give_feedback` | yes | yes | — |
| `last_feedback_index` | — | — | — |
| `list_identities` | yes | yes | — |
| `read_all_feedback` | yes | yes | — |
| `read_feedback` | yes | yes | — |
| `register_identity` | yes | yes | — |
| `request_validation` | yes | yes | — |
| `respond_validation` | yes | yes | — |
| `revoke_feedback` | yes | yes | — |
| `set_agent_wallet` | yes | yes | — |
| `set_identity_metadata` | — | — | — |
| `update_identity` | yes | yes | — |
| `validation_status` | yes | yes | — |
| `validation_summary` | yes | yes | — |
| `validator_requests` | — | — | — |

### `commerce.fixed_assets()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `dispose` | yes | yes | — |
| `generate_schedule` | yes | yes | — |
| `get` | yes | yes | — |
| `get_schedule` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `place_in_service` | yes | yes | — |
| `post_depreciation` | yes | yes | — |
| `update` | yes | yes | — |
| `write_off` | yes | yes | — |

### `commerce.fraud()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create_assessment` | yes | yes | — |
| `create_rule` | yes | yes | — |
| `delete_rule` | yes | yes | — |
| `get_active_rules` | yes | yes | — |
| `get_assessment` | yes | yes | — |
| `get_rule` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list_assessments` | yes | yes | — |
| `list_rules` | yes | yes | — |
| `review_assessment` | yes | yes | — |
| `update_rule` | yes | yes | — |

### `commerce.fulfillment()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_carton` | — | — | — |
| `add_carton_item` | — | — | — |
| `assign_pack` | — | — | — |
| `assign_pick` | yes | — | — |
| `assign_ship` | — | — | — |
| `cancel_pack` | — | — | — |
| `cancel_pick` | yes | — | — |
| `cancel_ship` | — | — | — |
| `cancel_wave` | yes | — | — |
| `complete_pack` | — | — | — |
| `complete_pick` | — | yes | — |
| `complete_ship` | — | — | — |
| `complete_wave` | yes | — | — |
| `count_packs` | — | — | — |
| `count_picks` | — | — | — |
| `count_ships` | — | — | — |
| `count_waves` | yes | — | — |
| `create_pack` | — | — | — |
| `create_pick` | — | — | — |
| `create_picks_for_order` | — | — | — |
| `create_ship` | — | — | — |
| `create_wave` | yes | yes | — |
| `create_waves_batch` | — | — | — |
| `get_carton_items` | — | — | — |
| `get_cartons` | — | — | — |
| `get_pack` | — | — | — |
| `get_pick` | yes | — | — |
| `get_picks_batch` | — | — | — |
| `get_picks_for_order` | — | — | — |
| `get_picks_for_wave` | — | — | — |
| `get_ship` | — | — | — |
| `get_wave` | yes | yes | — |
| `get_wave_by_number` | — | — | — |
| `get_wave_orders` | — | — | — |
| `is_order_ready_to_pack` | yes | — | — |
| `is_order_ready_to_ship` | yes | — | — |
| `list_packs` | — | — | — |
| `list_picks` | yes | yes | — |
| `list_ships` | — | — | — |
| `list_waves` | yes | yes | — |
| `mark_label_printed` | — | — | — |
| `print_label` | — | — | — |
| `release_wave` | yes | yes | — |
| `report_short` | — | — | — |
| `start_pack` | — | — | — |
| `start_pick` | yes | — | — |

### `commerce.general_ledger()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `auto_post_bill` | — | — | — |
| `auto_post_bill_payment` | — | — | — |
| `auto_post_inventory_cost` | — | — | — |
| `auto_post_invoice` | — | — | — |
| `auto_post_payment_received` | — | — | — |
| `auto_post_write_off` | — | — | — |
| `close_month` | yes | yes | — |
| `close_period` | — | — | — |
| `create_account` | yes | yes | — |
| `create_accounts_batch` | — | — | — |
| `create_journal_entry` | — | — | — |
| `create_period` | yes | yes | — |
| `delete_account` | — | — | — |
| `get_account` | yes | yes | — |
| `get_account_balance` | yes | — | — |
| `get_account_by_number` | yes | yes | — |
| `get_account_hierarchy` | — | — | — |
| `get_account_transactions` | — | — | — |
| `get_accounts_batch` | — | — | — |
| `get_auto_posting_config` | — | — | — |
| `get_balance_sheet` | yes | — | — |
| `get_current_period` | — | — | — |
| `get_income_statement` | yes | — | — |
| `get_journal_entry` | yes | yes | — |
| `get_journal_entry_by_number` | — | — | — |
| `get_journal_entry_lines` | — | — | — |
| `get_period` | — | — | — |
| `get_period_for_date` | — | — | — |
| `get_trial_balance` | yes | yes | — |
| `initialize_chart_of_accounts` | yes | yes | — |
| `list_accounts` | yes | yes | — |
| `list_journal_entries` | yes | — | — |
| `list_periods` | yes | — | — |
| `lock_period` | — | — | — |
| `open_period` | yes | yes | — |
| `post_journal_entry` | yes | yes | — |
| `reclose_period` | — | — | — |
| `reopen_period` | — | — | — |
| `revalue` | yes | yes | — |
| `reverse_journal_entry` | — | — | — |
| `run_period_close` | — | — | — |
| `set_auto_posting_config` | — | — | — |
| `update_account` | — | — | — |
| `void_journal_entry` | yes | — | — |

### `commerce.gift_cards()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `charge` | yes | yes | — |
| `create` | yes | yes | — |
| `disable` | yes | yes | — |
| `get` | yes | yes | — |
| `get_by_code` | yes | yes | — |
| `get_transactions` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `refund` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.inbound_shipments()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `cancel` | yes | yes | — |
| `create` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `mark_arrived` | yes | yes | — |
| `mark_in_transit` | yes | yes | — |
| `receive_line` | yes | yes | — |

### `commerce.integration_field_mappings()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `bulk_create` | yes | yes | — |
| `bulk_delete` | yes | yes | — |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `distinct_groups` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.integration_mappings()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `bulk_upsert` | yes | yes | — |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `resolve` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.inventory()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `adjust` | yes | yes | yes |
| `adjust_at_location` | — | — | — |
| `confirm_reservation` | yes | yes | — |
| `create_item` | yes | yes | yes |
| `expire_reservations` | — | — | — |
| `get_item` | — | — | — |
| `get_item_by_sku` | — | — | — |
| `get_reorder_needed` | — | — | — |
| `get_reservation` | — | — | — |
| `get_stock` | yes | yes | yes |
| `get_transactions` | — | — | — |
| `has_stock` | — | — | — |
| `list` | — | — | — |
| `list_reservations_by_reference` | — | — | — |
| `release_reservation` | yes | yes | — |
| `reserve` | yes | yes | — |

### `commerce.invoices()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_item` | — | — | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `customer_balance` | — | — | — |
| `dispute` | — | — | — |
| `for_customer` | — | — | — |
| `for_order` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | — | — | — |
| `get_items` | — | — | — |
| `get_overdue` | yes | yes | yes |
| `list` | yes | yes | yes |
| `mark_viewed` | — | — | — |
| `recalculate` | — | — | — |
| `record_payment` | yes | yes | yes |
| `remove_item` | — | — | — |
| `send` | yes | yes | yes |
| `update` | — | — | — |
| `update_item` | — | — | — |
| `void` | yes | yes | yes |
| `write_off` | — | — | — |

### `commerce.kernel_audit()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `checkpoint` | — | — | — |
| `verify_chain` | — | — | — |
| `verify_checkpoint` | — | — | — |

### `commerce.lots()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_certificate` | — | — | — |
| `adjust` | — | — | — |
| `confirm_reservation` | — | — | — |
| `consume` | — | — | — |
| `count` | yes | — | — |
| `create` | yes | yes | — |
| `create_batch` | — | — | — |
| `delete` | — | — | — |
| `delete_certificate` | — | — | — |
| `expire_lots` | — | — | — |
| `get` | yes | yes | — |
| `get_active_lots` | yes | — | — |
| `get_available_lots_for_sku` | yes | yes | — |
| `get_batch` | — | — | — |
| `get_by_number` | yes | — | — |
| `get_certificates` | — | — | — |
| `get_children` | — | — | — |
| `get_expired_lots` | yes | — | — |
| `get_expiring_lots` | yes | yes | — |
| `get_locations` | — | — | — |
| `get_parents` | — | — | — |
| `get_quantity_at_location` | — | — | — |
| `get_quarantined` | yes | — | — |
| `get_transactions` | — | — | — |
| `list` | yes | yes | — |
| `merge` | — | — | — |
| `quarantine` | yes | yes | — |
| `release_expired_reservations` | — | — | — |
| `release_quarantine` | yes | — | — |
| `release_reservation` | — | — | — |
| `reserve` | — | — | — |
| `split` | — | — | — |
| `trace` | — | — | — |
| `transfer` | — | — | — |
| `update` | — | — | — |

### `commerce.loyalty()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `adjust_points` | yes | yes | — |
| `create_program` | yes | yes | — |
| `create_reward` | yes | yes | — |
| `delete_reward` | yes | yes | — |
| `enroll` | yes | yes | — |
| `get_account` | yes | yes | — |
| `get_account_by_customer` | yes | yes | — |
| `get_program` | yes | yes | — |
| `get_reward` | yes | yes | — |
| `get_transactions` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list_accounts` | yes | yes | — |
| `list_programs` | yes | yes | — |
| `list_rewards` | yes | yes | — |

### `commerce.maintenance()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `backup_to` | yes | — | — |
| `export_all` | — | — | — |
| `export_to_file` | — | — | — |
| `export_to_file_with` | yes | — | — |
| `exportable_domains` | yes | — | — |
| `import_all` | — | — | — |
| `import_from_file` | yes | — | — |
| `importable_domains` | yes | — | — |
| `restore_from` | yes | — | — |
| `supports_backup` | yes | — | — |

### `commerce.orders()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_item` | — | — | — |
| `cancel` | yes | yes | yes |
| `cancel_with` | — | — | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `create_from_cart` | yes | — | — |
| `delete` | — | — | — |
| `deliver` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | yes | yes | — |
| `list` | yes | yes | yes |
| `list_for_customer` | — | — | — |
| `remove_item` | — | — | — |
| `remove_item_with` | — | — | — |
| `ship` | yes | yes | — |
| `ship_lines` | — | — | — |
| `update` | yes | — | — |
| `update_status` | yes | yes | yes |

### `commerce.payment_obligations()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `dashboard` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `link_bill` | yes | yes | — |
| `list` | yes | yes | — |
| `record_payment` | yes | yes | — |
| `set_status` | yes | yes | — |

### `commerce.payments()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `cancel` | yes | — | — |
| `complete_refund` | yes | yes | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `create_payment_method` | — | — | — |
| `create_refund` | yes | yes | yes |
| `delete_payment_method` | — | — | — |
| `fail_refund` | yes | yes | — |
| `for_invoice` | — | — | — |
| `for_order` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_external_id` | — | — | — |
| `get_by_number` | — | — | — |
| `get_payment_methods` | — | — | — |
| `get_refund` | yes | yes | — |
| `get_refunds` | yes | yes | — |
| `list` | yes | yes | yes |
| `mark_completed` | yes | yes | yes |
| `mark_failed` | yes | yes | yes |
| `mark_processing` | — | — | — |
| `open_captures_for_order` | — | — | — |
| `set_default_payment_method` | — | — | — |
| `update` | — | — | — |

### `commerce.prepayments()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `apply` | yes | yes | — |
| `create` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_applications` | yes | yes | — |
| `refund` | yes | yes | — |
| `reverse_application` | yes | yes | — |

### `commerce.price_levels()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `delete_entry` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_entries` | yes | yes | — |
| `set_entry` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.price_schedules()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `delete_entry` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_entries` | yes | yes | — |
| `resolve_price` | yes | yes | — |
| `set_entry` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.print_stations()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `complete_job` | yes | yes | — |
| `enqueue_job` | yes | yes | — |
| `get_station` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list_jobs` | yes | yes | — |
| `list_stations` | yes | yes | — |
| `next_job` | yes | yes | — |
| `pair` | yes | yes | — |
| `revoke_station` | yes | yes | — |

### `commerce.production_batches()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_work_orders` | yes | yes | — |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `remove_work_order` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.products()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `activate` | yes | — | — |
| `add_variant` | yes | — | — |
| `archive` | yes | — | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `delete` | yes | — | — |
| `delete_variant` | yes | — | — |
| `get` | yes | yes | yes |
| `get_by_slug` | yes | — | — |
| `get_variant` | yes | — | — |
| `get_variant_by_sku` | yes | yes | — |
| `get_variants` | yes | — | — |
| `list` | yes | yes | yes |
| `list_active` | — | — | — |
| `search` | yes | — | — |
| `update` | yes | yes | yes |
| `update_variant` | yes | — | — |

### `commerce.promotions()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `activate` | yes | yes | — |
| `add_condition` | yes | yes | — |
| `apply` | yes | yes | — |
| `create` | yes | yes | — |
| `create_coupon` | yes | yes | — |
| `deactivate` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `get_active` | yes | yes | — |
| `get_by_code` | yes | yes | — |
| `get_coupon` | yes | yes | — |
| `get_coupon_by_code` | yes | yes | — |
| `is_valid` | yes | — | — |
| `list` | yes | yes | — |
| `list_coupons` | yes | yes | — |
| `list_usage` | yes | yes | — |
| `record_usage` | yes | yes | — |
| `update` | yes | — | — |
| `validate_coupon` | yes | yes | — |

### `commerce.purchase_orders()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `acknowledge` | — | — | — |
| `add_item` | — | — | — |
| `approve` | yes | yes | yes |
| `cancel` | yes | yes | yes |
| `complete` | — | — | — |
| `count` | yes | yes | — |
| `count_suppliers` | — | — | — |
| `create` | yes | yes | yes |
| `create_supplier` | yes | yes | yes |
| `delete_supplier` | — | — | — |
| `for_supplier` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | — | — | — |
| `get_items` | — | — | — |
| `get_supplier` | yes | yes | yes |
| `get_supplier_by_code` | — | — | — |
| `hold` | — | — | — |
| `list` | yes | yes | yes |
| `list_suppliers` | yes | yes | yes |
| `receive` | — | — | — |
| `remove_item` | — | — | — |
| `send` | yes | yes | yes |
| `submit` | yes | yes | yes |
| `update` | — | — | — |
| `update_item` | — | — | — |
| `update_supplier` | — | — | — |

### `commerce.purgatory()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `ingest` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `map_line` | yes | yes | — |
| `post` | yes | yes | — |

### `commerce.quality()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `cancel_ncr` | — | — | — |
| `close_ncr` | yes | yes | — |
| `complete_inspection` | yes | yes | — |
| `count_active_holds` | yes | — | — |
| `count_inspections` | — | — | — |
| `count_ncrs` | — | — | — |
| `create_defect_code` | — | — | — |
| `create_hold` | yes | yes | — |
| `create_inspection` | yes | yes | — |
| `create_ncr` | yes | yes | — |
| `deactivate_defect_code` | — | — | — |
| `delete_inspection` | — | — | — |
| `get_active_holds` | yes | — | — |
| `get_active_holds_for_lot` | — | — | — |
| `get_active_holds_for_sku` | — | — | — |
| `get_defect_code` | — | — | — |
| `get_hold` | yes | — | — |
| `get_inspection` | yes | yes | — |
| `get_inspection_by_number` | — | — | — |
| `get_inspection_items` | yes | — | — |
| `get_ncr` | yes | — | — |
| `get_ncr_by_number` | — | — | — |
| `get_open_ncrs` | — | — | — |
| `get_pending_inspections` | — | — | — |
| `list_defect_codes` | — | — | — |
| `list_holds` | yes | — | — |
| `list_inspections` | yes | yes | — |
| `list_ncrs` | yes | yes | — |
| `record_inspection_result` | — | — | — |
| `release_hold` | yes | yes | — |
| `start_inspection` | yes | — | — |
| `update_inspection` | — | — | — |
| `update_ncr` | yes | yes | — |

### `commerce.receiving()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `assign_put_away` | — | — | — |
| `cancel_put_away` | — | — | — |
| `cancel_receipt` | yes | — | — |
| `complete_put_away` | — | — | — |
| `complete_receiving` | yes | yes | — |
| `count_put_aways` | — | — | — |
| `count_receipts` | yes | — | — |
| `create_put_away` | — | — | — |
| `create_receipt` | yes | yes | — |
| `create_receipt_from_po` | yes | — | — |
| `create_receipts_batch` | — | — | — |
| `delete_receipt` | — | — | — |
| `get_pending_put_aways` | — | — | — |
| `get_put_away` | — | — | — |
| `get_receipt` | yes | yes | — |
| `get_receipt_by_number` | yes | — | — |
| `get_receipt_items` | yes | yes | — |
| `get_receipts_batch` | — | — | — |
| `list_put_aways` | — | — | — |
| `list_receipts` | yes | yes | — |
| `receive_items` | — | — | — |
| `start_put_away` | — | — | — |
| `start_receiving` | yes | — | — |
| `update_receipt` | — | — | — |

### `commerce.returns()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_tracking` | yes | — | — |
| `approve` | yes | yes | yes |
| `cancel` | yes | — | — |
| `complete` | yes | — | yes |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `get` | yes | yes | yes |
| `list` | yes | yes | yes |
| `list_for_customer` | yes | — | — |
| `list_for_order` | yes | — | — |
| `list_pending` | yes | — | — |
| `mark_received` | yes | — | — |
| `reject` | yes | yes | yes |
| `set_item_disposition` | — | — | — |
| `update` | — | — | — |

### `commerce.revenue_recognition()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create_contract` | yes | yes | — |
| `generate_schedule` | yes | yes | — |
| `get_contract` | yes | yes | — |
| `get_schedule` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list_contracts` | yes | yes | — |
| `list_obligations` | yes | yes | — |
| `recognize_period` | yes | yes | — |
| `update_contract` | yes | yes | — |

### `commerce.reviews()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `get_summary` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `mark_helpful` | yes | yes | — |
| `mark_reported` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.search_config()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `get_active` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `set_active` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.segments()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_member` | yes | yes | — |
| `count_members` | — | — | — |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_member` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_members` | yes | yes | — |
| `remove_member` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.serials()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `activate` | — | — | — |
| `can_ship` | — | — | — |
| `change_status` | — | yes | — |
| `confirm_reservation` | — | — | — |
| `count` | yes | — | — |
| `create` | yes | yes | — |
| `create_batch` | — | — | — |
| `create_bulk` | — | — | — |
| `delete` | — | — | — |
| `get` | yes | — | — |
| `get_available` | yes | — | — |
| `get_batch` | — | — | — |
| `get_batch_by_serial` | — | — | — |
| `get_by_serial` | yes | yes | — |
| `get_for_customer` | — | — | — |
| `get_for_lot` | — | — | — |
| `get_history` | — | — | — |
| `get_reservation` | — | — | — |
| `is_available` | yes | — | — |
| `list` | yes | yes | — |
| `lookup` | — | — | — |
| `mark_returned` | — | — | — |
| `mark_shipped` | — | — | — |
| `mark_sold` | yes | — | — |
| `move_serial` | — | — | — |
| `quarantine` | yes | — | — |
| `quarantine_for_lot` | — | — | — |
| `release_expired_reservations` | — | — | — |
| `release_quarantine` | — | — | — |
| `release_quarantine_for_lot` | — | — | — |
| `release_reservation` | — | — | — |
| `reserve` | — | — | — |
| `scrap` | — | — | — |
| `transfer_ownership` | — | — | — |
| `update` | — | — | — |
| `validate` | — | — | — |

### `commerce.shipments()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_event` | — | — | — |
| `add_item` | — | — | — |
| `add_item_with_version` | yes | — | — |
| `cancel` | yes | yes | yes |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `for_order` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | — | — | — |
| `get_by_tracking` | — | — | — |
| `get_events` | — | — | — |
| `get_items` | — | — | — |
| `hold` | — | — | — |
| `list` | yes | yes | yes |
| `mark_delivered` | yes | yes | yes |
| `mark_failed` | — | — | — |
| `mark_in_transit` | — | — | — |
| `mark_out_for_delivery` | — | — | — |
| `mark_processing` | — | — | — |
| `mark_ready` | — | — | — |
| `remove_item` | — | — | — |
| `remove_item_with_version` | yes | — | — |
| `ship` | yes | yes | yes |
| `update` | yes | — | — |

### `commerce.shipping_zones()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `calculate_rates` | yes | yes | — |
| `create` | yes | yes | — |
| `create_method` | yes | yes | — |
| `delete` | yes | yes | — |
| `delete_method` | yes | yes | — |
| `find_matching_zones` | yes | yes | — |
| `get` | yes | yes | — |
| `get_method` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_methods` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.stock_snapshots()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `capture` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `latest` | yes | yes | — |
| `list` | yes | yes | — |

### `commerce.store_credits()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `adjust` | yes | yes | — |
| `apply` | yes | yes | — |
| `create` | yes | yes | — |
| `get` | yes | yes | — |
| `get_transactions` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |

### `commerce.subscriptions()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `activate_plan` | yes | yes | — |
| `archive_plan` | yes | yes | — |
| `cancel` | yes | yes | — |
| `claim_due_for_billing` | — | — | — |
| `create_billing_cycle` | — | — | — |
| `create_claimed_billing_cycle` | — | — | — |
| `create_plan` | yes | yes | — |
| `get` | yes | yes | — |
| `get_active_customer_subscriptions` | — | — | — |
| `get_active_plans` | — | — | — |
| `get_billing_cycle` | yes | yes | — |
| `get_by_number` | yes | yes | — |
| `get_customer_subscriptions` | — | — | — |
| `get_due_for_billing` | — | — | — |
| `get_events` | yes | yes | — |
| `get_plan` | yes | yes | — |
| `get_plan_by_code` | yes | — | — |
| `get_trials_ending` | — | — | — |
| `is_active` | — | — | — |
| `is_in_trial` | — | — | — |
| `list` | yes | yes | — |
| `list_billing_cycles` | yes | yes | — |
| `list_plans` | yes | yes | — |
| `mark_cycle_failed` | — | — | — |
| `mark_cycle_paid` | — | — | — |
| `pause` | yes | yes | — |
| `release_billing_claim` | — | — | — |
| `resume` | yes | yes | — |
| `skip_next_cycle` | yes | yes | — |
| `subscribe` | yes | yes | — |
| `update` | yes | — | — |
| `update_billing_cycle_status` | — | — | — |
| `update_plan` | yes | — | — |

### `commerce.supplier_skus()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `bulk_upsert` | yes | yes | — |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.tax()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `calculate` | yes | — | — |
| `calculate_for_item` | yes | — | — |
| `create_exemption` | yes | yes | — |
| `create_jurisdiction` | yes | yes | — |
| `create_rate` | yes | yes | — |
| `customer_is_exempt` | yes | yes | — |
| `customer_is_exempt_on` | — | — | — |
| `get_customer_exemptions` | yes | yes | — |
| `get_effective_rate` | yes | — | — |
| `get_exemption` | yes | yes | — |
| `get_jurisdiction` | yes | yes | — |
| `get_jurisdiction_by_code` | yes | yes | — |
| `get_rate` | yes | yes | — |
| `get_rates_for_address` | — | — | — |
| `get_settings` | yes | yes | — |
| `is_enabled` | yes | yes | — |
| `list_jurisdictions` | yes | yes | — |
| `list_rates` | yes | yes | — |
| `set_enabled` | yes | yes | — |
| `update_settings` | yes | — | — |
| `verify_exemption` | — | — | — |

### `commerce.topology_snapshots()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `capture` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `latest` | yes | yes | — |
| `list` | yes | yes | — |

### `commerce.transfer_orders()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `cancel` | yes | yes | — |
| `create` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `receive_line` | yes | yes | — |
| `ship` | yes | yes | — |

### `commerce.units_of_measure()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `create_class` | yes | yes | — |
| `create_rule` | yes | yes | — |
| `create_uom` | yes | yes | — |
| `delete_class` | yes | yes | — |
| `delete_rule` | yes | yes | — |
| `delete_uom` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list_classes` | yes | yes | — |
| `list_rules` | yes | yes | — |
| `list_uoms` | yes | yes | — |
| `set_base_uom` | yes | yes | — |

### `commerce.vector()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `clear` | yes | yes | — |
| `clear_all` | yes | yes | — |
| `embed` | — | — | — |
| `index_customer` | yes | yes | — |
| `index_customers` | yes | yes | — |
| `index_inventory_item` | yes | — | — |
| `index_inventory_items` | yes | — | — |
| `index_order` | yes | — | — |
| `index_orders` | yes | — | — |
| `index_product` | yes | yes | — |
| `index_products` | yes | yes | — |
| `is_indexed` | — | — | — |
| `search_customers` | yes | yes | — |
| `search_inventory` | yes | — | — |
| `search_orders` | yes | — | — |
| `search_products` | yes | yes | — |
| `search_products_by_embedding` | — | — | — |
| `stats` | yes | yes | — |
| `unindex_customer` | — | — | — |
| `unindex_inventory_item` | — | — | — |
| `unindex_order` | — | — | — |
| `unindex_product` | — | — | — |

### `commerce.vendor_credits()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `apply` | yes | yes | — |
| `cancel` | yes | yes | — |
| `create` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `list_applications` | yes | yes | — |
| `reverse_application` | yes | yes | — |

### `commerce.vendor_returns()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `cancel` | yes | yes | — |
| `create` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `process` | yes | yes | — |
| `submit` | yes | yes | — |

### `commerce.warehouse()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `adjust_bin_level` | — | — | — |
| `adjust_inventory` | — | — | — |
| `cancel_cycle_count` | yes | yes | — |
| `complete_cycle_count` | yes | yes | — |
| `count_bins` | — | — | — |
| `count_locations` | — | — | — |
| `count_movements` | — | — | — |
| `count_warehouses` | yes | — | — |
| `create_bin` | — | — | — |
| `create_cycle_count` | yes | yes | — |
| `create_location` | yes | yes | — |
| `create_locations_batch` | — | — | — |
| `create_warehouse` | yes | yes | — |
| `create_zone` | — | — | — |
| `delete_bin` | — | — | — |
| `delete_location` | — | — | — |
| `delete_warehouse` | — | — | — |
| `delete_zone` | — | — | — |
| `get_bin` | — | — | — |
| `get_bin_by_code` | — | — | — |
| `get_bin_levels` | — | — | — |
| `get_bin_levels_for_sku` | — | — | — |
| `get_cycle_count` | yes | yes | — |
| `get_inventory_for_sku` | — | — | — |
| `get_location` | yes | — | — |
| `get_location_by_code` | — | — | — |
| `get_location_inventory` | — | — | — |
| `get_locations_batch` | — | — | — |
| `get_locations_for_warehouse` | — | yes | — |
| `get_movements` | — | — | — |
| `get_pickable_locations` | yes | — | — |
| `get_receivable_locations` | — | — | — |
| `get_total_available` | yes | — | — |
| `get_total_on_hand` | — | — | — |
| `get_warehouse` | yes | yes | — |
| `get_warehouse_by_code` | yes | — | — |
| `get_zone` | — | — | — |
| `get_zones` | — | — | — |
| `list_bins` | — | — | — |
| `list_cycle_counts` | yes | yes | — |
| `list_location_inventory` | — | — | — |
| `list_locations` | yes | — | — |
| `list_warehouses` | yes | yes | — |
| `move_between_bins` | — | — | — |
| `move_inventory` | — | — | — |
| `reconcile_bins` | — | — | — |
| `record_cycle_counts` | yes | yes | — |
| `start_cycle_count` | yes | yes | — |
| `update_bin` | — | — | — |
| `update_location` | — | — | — |
| `update_warehouse` | — | — | — |
| `update_zone` | — | — | — |

### `commerce.warranties()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `approve_claim` | yes | yes | yes |
| `cancel_claim` | — | — | — |
| `complete_claim` | yes | yes | yes |
| `count` | yes | yes | — |
| `count_claims` | — | — | — |
| `create` | yes | yes | yes |
| `create_claim` | yes | yes | yes |
| `deny_claim` | yes | yes | yes |
| `expire` | — | — | — |
| `for_customer` | — | — | — |
| `for_order` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | — | — | — |
| `get_by_serial` | — | — | — |
| `get_claim` | — | — | — |
| `get_claim_by_number` | — | — | — |
| `get_claims` | — | — | — |
| `is_valid` | — | — | — |
| `list` | yes | yes | yes |
| `list_claims` | — | — | — |
| `transfer` | — | — | — |
| `update` | — | — | — |
| `update_claim` | — | — | — |
| `void` | — | — | — |

### `commerce.wishlists()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_item` | yes | yes | — |
| `create` | yes | yes | — |
| `delete` | yes | yes | — |
| `get` | yes | yes | — |
| `is_supported` | yes | yes | — |
| `list` | yes | yes | — |
| `remove_item` | yes | yes | — |
| `update` | yes | yes | — |

### `commerce.work_orders()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `add_material` | — | — | — |
| `add_task` | — | — | — |
| `cancel` | yes | yes | yes |
| `complete` | yes | yes | yes |
| `complete_task` | — | — | — |
| `consume_material` | — | — | — |
| `count` | yes | yes | — |
| `create` | yes | yes | yes |
| `delete` | — | — | — |
| `for_bom` | — | — | — |
| `for_product` | — | — | — |
| `get` | yes | yes | yes |
| `get_by_number` | — | — | — |
| `get_materials` | — | — | — |
| `get_tasks` | — | — | — |
| `hold` | — | — | — |
| `list` | yes | yes | yes |
| `remove_task` | — | — | — |
| `resume` | — | — | — |
| `start` | yes | yes | yes |
| `start_task` | — | — | — |
| `update` | — | — | — |
| `update_task` | — | — | — |

### `commerce.x402()`

| Engine method | Node.js | Python | Go |
| --- | --- | --- | --- |
| `acknowledge_agent_message` | — | — | — |
| `active_agents` | — | — | — |
| `active_intent_for_cart` | — | — | — |
| `adjust_credit_balance` | — | — | — |
| `agents_by_trust_level` | — | — | — |
| `cancel_intent` | — | — | — |
| `charge_credit_terms` | — | — | — |
| `confirm_delivery` | — | — | — |
| `count_agents` | — | — | — |
| `count_intents` | — | — | — |
| `count_purchases` | — | — | — |
| `count_quotes` | — | — | — |
| `create_cart_payment` | — | — | — |
| `create_credit_terms` | — | — | — |
| `create_intent` | yes | — | — |
| `create_purchase` | — | — | — |
| `create_quote` | — | — | — |
| `credit_account` | yes | — | — |
| `debit_account` | yes | — | — |
| `delete_agent` | — | — | — |
| `discover_agents` | yes | — | — |
| `expire_stale_intents` | — | — | — |
| `fail_agent_message` | — | — | — |
| `get_agent` | yes | — | — |
| `get_agent_by_wallet` | yes | — | — |
| `get_agent_message` | — | — | — |
| `get_credit_account` | yes | — | — |
| `get_credit_balance` | yes | — | — |
| `get_credit_terms` | — | — | — |
| `get_intent` | yes | — | — |
| `get_next_nonce` | yes | — | — |
| `get_or_create_credit_account` | — | — | — |
| `get_purchase` | — | — | — |
| `get_purchase_by_number` | — | — | — |
| `get_quote` | — | — | — |
| `get_quote_by_number` | — | — | — |
| `has_valid_signature` | — | — | — |
| `intents_by_status` | — | — | — |
| `intents_for_cart` | — | — | — |
| `intents_for_order` | — | — | — |
| `is_ready_for_settlement` | — | — | — |
| `link_purchase_to_order` | — | — | — |
| `list_agent_messages` | — | — | — |
| `list_agents` | yes | — | — |
| `list_credit_terms` | — | — | — |
| `list_credit_terms_entries` | — | — | — |
| `list_credit_transactions` | yes | — | — |
| `list_intents` | yes | — | — |
| `list_purchases` | — | — | — |
| `list_quotes` | — | — | — |
| `mark_batched` | — | — | — |
| `mark_expired` | — | — | — |
| `mark_failed` | — | — | — |
| `mark_sequenced` | — | — | — |
| `mark_settled` | yes | — | — |
| `pending_intents` | — | — | — |
| `reactivate_agent` | — | — | — |
| `record_credit_terms_payment` | — | — | — |
| `register_agent` | yes | — | — |
| `send_agent_message` | — | — | — |
| `settled_intents` | — | — | — |
| `sign_intent` | yes | — | — |
| `signed_intents` | — | — | — |
| `suspend_agent` | — | — | — |
| `update_agent` | — | — | — |
| `update_purchase_status` | — | — | — |
| `update_quote_status` | — | — | — |
| `verified_agents` | — | — | — |
| `verify_agent` | yes | — | — |
