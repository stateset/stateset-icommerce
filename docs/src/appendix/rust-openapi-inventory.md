# Rust OpenAPI Inventory

This page is generated from the live Rust OpenAPI spec exported by `stateset-http`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_rust_openapi_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/rust-openapi-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| OpenAPI version | `3.1.0` |
| API title | StateSet Commerce API |
| API version | `1.0.4` |
| Paths | 329 |
| Operations | 445 |
| Schemas | 432 |
| Tags | 61 |

## Method Counts

| Method | Operations |
| --- | --- |
| DELETE | 25 |
| GET | 177 |
| PATCH | 10 |
| POST | 220 |
| PUT | 13 |

## Tag Counts

| Tag | Operations | Description |
| --- | --- | --- |
| `a2a` | 11 | Agent-to-agent messaging and credit terms |
| `accounts_payable` | 14 | Supplier bills, AP payments and allocations, payment runs, and AP aging |
| `accounts_receivable` | 15 | AR aging, payment application, credit memos, write-offs, dunning, and customer statements |
| `activity_logs` | 4 | Record-level activity history |
| `backorders` | 5 | Backorder creation, fulfillment, and cancellation |
| `bom` | 8 | Bills of materials: BOM revisions, components, and status control |
| `carts` | 10 | Cart and checkout sessions: items, shipping, payment, completion |
| `channels` | 6 | Sales channel management |
| `companies` | 6 | B2B company account management |
| `currency` | 3 | Exchange rates and currency conversion |
| `customers` | 5 | Customer management |
| `edi_documents` | 5 | EDI document exchange |
| `events` | 1 | Real-time event streaming |
| `fixed_assets` | 10 | Fixed-asset register: acquisition, depreciation schedules, disposal, and write-off |
| `fulfillment` | 13 | Outbound fulfillment: waves, pick tasks, pack tasks and cartons, ship tasks |
| `general_ledger` | 20 | Chart of accounts, journal entries, accounting periods, and financial reports (trial balance, balance sheet, income statement) |
| `gift_cards` | 6 | Gift card management |
| `health` | 5 | Health check endpoints |
| `inbound_shipments` | 7 | Inbound shipment receiving |
| `integration_field_mappings` | 8 | Integration field-level mappings |
| `integration_mappings` | 7 | External integration record mappings |
| `inventory` | 3 | Stock and inventory management |
| `invoices` | 5 | Invoice management |
| `lots` | 9 | Lot/batch tracking: creation, consumption, reservations, quarantine, and expiry queries |
| `loyalty` | 4 | Loyalty program management |
| `negotiations` | 5 | Agent-to-agent price negotiation |
| `orders` | 5 | Order lifecycle management |
| `payment_obligations` | 7 | Payment obligation tracking |
| `payments` | 5 | Payment transaction management |
| `prepayments` | 5 | Customer and vendor prepayments |
| `price_levels` | 7 | Customer price level management |
| `price_schedules` | 7 | Scheduled pricing management |
| `print_stations` | 7 | Warehouse print station management |
| `production_batches` | 5 | Production batch tracking |
| `products` | 5 | Product catalog |
| `promotions` | 5 | Promotion and discount management |
| `purchase_orders` | 18 | Supplier procurement: purchase order lifecycle (draft → approval → send → acknowledge → receive → complete) and supplier management |
| `purgatory` | 6 | Quarantined record review |
| `quality` | 15 | Quality control: inspections, non-conformance reports, and quality holds |
| `receiving` | 12 | Inbound receiving: goods receipts, item receipt, and put-away tasks |
| `reports` | 5 | Computed business reports |
| `returns` | 5 | Return request processing |
| `revenue_recognition` | 8 | Revenue contracts, performance obligations, and recognition schedules (ASC 606) |
| `reviews` | 4 | Product review management |
| `segments` | 6 | Customer segment management |
| `serials` | 8 | Serial number tracking: creation, lookup, reservations, and lifecycle transitions |
| `shipments` | 4 | Shipment tracking and management |
| `shipping` | 4 | Shipping zone management |
| `stock_snapshots` | 5 | Point-in-time inventory snapshots |
| `store_credits` | 5 | Store credit management |
| `subscriptions` | 6 | Recurring subscription management |
| `supplier_skus` | 4 | Supplier SKU catalog |
| `topology_snapshots` | 5 | Network topology snapshots |
| `transfer_orders` | 6 | Inter-warehouse transfer orders |
| `units_of_measure` | 6 | Unit of measure definitions |
| `vendor_credits` | 5 | Vendor credit management |
| `vendor_returns` | 6 | Return-to-vendor processing |
| `warehouse` | 27 | Warehouses, storage locations, and location-level inventory (adjust/move) |
| `warranties` | 3 | Product warranty management |
| `wishlists` | 6 | Customer wishlist management |
| `work_orders` | 13 | Manufacturing work order lifecycle and shop-floor tasks |

## Operations

| Method | Path | Tags | Operation ID | Summary |
| --- | --- | --- | --- | --- |
| `GET` | `/api/v1/a2a/credit` | `a2a` | `list_terms` | `GET /api/v1/a2a/credit` |
| `POST` | `/api/v1/a2a/credit` | `a2a` | `create_terms` | `POST /api/v1/a2a/credit` |
| `GET` | `/api/v1/a2a/credit/{id}` | `a2a` | `get_terms` | `GET /api/v1/a2a/credit/:id` |
| `POST` | `/api/v1/a2a/credit/{id}/charge` | `a2a` | `charge_credit` | `POST /api/v1/a2a/credit/:id/charge` |
| `GET` | `/api/v1/a2a/credit/{id}/entries` | `a2a` | `list_entries` | `GET /api/v1/a2a/credit/:id/entries` |
| `POST` | `/api/v1/a2a/credit/{id}/payment` | `a2a` | `record_payment` | `POST /api/v1/a2a/credit/:id/payment` |
| `GET` | `/api/v1/a2a/messages` | `a2a` | `list_messages` | `GET /api/v1/a2a/messages` |
| `POST` | `/api/v1/a2a/messages` | `a2a` | `send_message` | `POST /api/v1/a2a/messages` |
| `GET` | `/api/v1/a2a/messages/{id}` | `a2a` | `get_message` | `GET /api/v1/a2a/messages/:id` |
| `POST` | `/api/v1/a2a/messages/{id}/acknowledge` | `a2a` | `acknowledge_message` | `POST /api/v1/a2a/messages/:id/acknowledge` |
| `POST` | `/api/v1/a2a/messages/{id}/fail` | `a2a` | `fail_message` | `POST /api/v1/a2a/messages/:id/fail` |
| `GET` | `/api/v1/activity-logs` | `activity_logs` | `activity_logs_list` | — |
| `POST` | `/api/v1/activity-logs` | `activity_logs` | `activity_logs_record` | — |
| `GET` | `/api/v1/activity-logs/{id}` | `activity_logs` | `activity_logs_get_one` | — |
| `GET` | `/api/v1/activity-logs/{subject_type}/{subject_id}` | `activity_logs` | `activity_logs_history` | — |
| `GET` | `/api/v1/ap/aging` | `accounts_payable` | `ap_aging` | — |
| `GET` | `/api/v1/ap/bills` | `accounts_payable` | `ap_list_bills` | — |
| `POST` | `/api/v1/ap/bills` | `accounts_payable` | `ap_create_bill` | — |
| `GET` | `/api/v1/ap/bills/{id}` | `accounts_payable` | `ap_get_bill` | — |
| `POST` | `/api/v1/ap/bills/{id}/approve` | `accounts_payable` | `ap_approve_bill` | — |
| `POST` | `/api/v1/ap/bills/{id}/cancel` | `accounts_payable` | `ap_cancel_bill` | — |
| `POST` | `/api/v1/ap/bills/{id}/dispute` | `accounts_payable` | `ap_dispute_bill` | — |
| `GET` | `/api/v1/ap/bills/{id}/three-way-match` | `accounts_payable` | `ap_three_way_match_bill` | — |
| `POST` | `/api/v1/ap/payment-runs` | `accounts_payable` | `ap_create_payment_run` | — |
| `POST` | `/api/v1/ap/payment-runs/{id}/approve` | `accounts_payable` | `ap_approve_payment_run` | — |
| `POST` | `/api/v1/ap/payment-runs/{id}/cancel` | `accounts_payable` | `ap_cancel_payment_run` | — |
| `POST` | `/api/v1/ap/payment-runs/{id}/process` | `accounts_payable` | `ap_process_payment_run` | — |
| `POST` | `/api/v1/ap/payments` | `accounts_payable` | `ap_create_payment` | — |
| `POST` | `/api/v1/ap/payments/{id}/void` | `accounts_payable` | `ap_void_payment` | — |
| `GET` | `/api/v1/ar/aging` | `accounts_receivable` | `ar_aging_summary` | — |
| `GET` | `/api/v1/ar/aging/customers` | `accounts_receivable` | `ar_aging_report` | — |
| `GET` | `/api/v1/ar/aging/customers/{customer_id}` | `accounts_receivable` | `ar_customer_aging` | — |
| `GET` | `/api/v1/ar/collection-activities` | `accounts_receivable` | `ar_list_collection_activities` | — |
| `POST` | `/api/v1/ar/collection-activities` | `accounts_receivable` | `ar_record_collection_activity` | — |
| `GET` | `/api/v1/ar/credit-memos` | `accounts_receivable` | `ar_list_credit_memos` | — |
| `POST` | `/api/v1/ar/credit-memos` | `accounts_receivable` | `ar_create_credit_memo` | — |
| `POST` | `/api/v1/ar/credit-memos/{id}/apply` | `accounts_receivable` | `ar_apply_credit_memo` | — |
| `GET` | `/api/v1/ar/customers/{customer_id}/statement` | `accounts_receivable` | `ar_customer_statement` | — |
| `GET` | `/api/v1/ar/dunning/due` | `accounts_receivable` | `ar_invoices_due_for_dunning` | — |
| `POST` | `/api/v1/ar/invoices/{invoice_id}/dunning` | `accounts_receivable` | `ar_send_dunning` | — |
| `POST` | `/api/v1/ar/payment-applications` | `accounts_receivable` | `ar_apply_payment` | — |
| `POST` | `/api/v1/ar/payment-applications/{id}/unapply` | `accounts_receivable` | `ar_unapply_payment` | — |
| `POST` | `/api/v1/ar/write-offs` | `accounts_receivable` | `ar_create_write_off` | — |
| `POST` | `/api/v1/ar/write-offs/{id}/reverse` | `accounts_receivable` | `ar_reverse_write_off` | — |
| `GET` | `/api/v1/backorders` | `backorders` | `backorders_list` | — |
| `POST` | `/api/v1/backorders` | `backorders` | `backorders_create` | — |
| `GET` | `/api/v1/backorders/{id}` | `backorders` | `backorders_get_one` | — |
| `POST` | `/api/v1/backorders/{id}/cancel` | `backorders` | `backorders_cancel` | — |
| `POST` | `/api/v1/backorders/{id}/fulfill` | `backorders` | `backorders_fulfill` | — |
| `GET` | `/api/v1/boms` | `bom` | `bom_list` | — |
| `POST` | `/api/v1/boms` | `bom` | `bom_create` | — |
| `DELETE` | `/api/v1/boms/{id}` | `bom` | `bom_delete` | — |
| `GET` | `/api/v1/boms/{id}` | `bom` | `bom_get_one` | — |
| `PUT` | `/api/v1/boms/{id}` | `bom` | `bom_update` | — |
| `POST` | `/api/v1/boms/{id}/activate` | `bom` | `bom_activate` | — |
| `GET` | `/api/v1/boms/{id}/components` | `bom` | `bom_list_components` | — |
| `POST` | `/api/v1/boms/{id}/components` | `bom` | `bom_add_component` | — |
| `GET` | `/api/v1/carts` | `carts` | `carts_list` | — |
| `POST` | `/api/v1/carts` | `carts` | `carts_create` | — |
| `GET` | `/api/v1/carts/{id}` | `carts` | `carts_get_one` | — |
| `POST` | `/api/v1/carts/{id}/cancel` | `carts` | `carts_cancel` | — |
| `POST` | `/api/v1/carts/{id}/complete` | `carts` | `carts_complete` | — |
| `POST` | `/api/v1/carts/{id}/items` | `carts` | `carts_add_item` | — |
| `DELETE` | `/api/v1/carts/{id}/items/{item_id}` | `carts` | `carts_remove_item` | — |
| `PUT` | `/api/v1/carts/{id}/items/{item_id}` | `carts` | `carts_update_item` | — |
| `POST` | `/api/v1/carts/{id}/payment` | `carts` | `carts_set_payment` | — |
| `POST` | `/api/v1/carts/{id}/shipping` | `carts` | `carts_set_shipping` | — |
| `GET` | `/api/v1/channels` | `channels` | `channels_list` | — |
| `POST` | `/api/v1/channels` | `channels` | `channels_create` | — |
| `DELETE` | `/api/v1/channels/{id}` | `channels` | `channels_delete_one` | — |
| `GET` | `/api/v1/channels/{id}` | `channels` | `channels_get_one` | — |
| `PUT` | `/api/v1/channels/{id}` | `channels` | `channels_update` | — |
| `POST` | `/api/v1/channels/{id}/lock` | `channels` | `channels_set_lock` | — |
| `GET` | `/api/v1/companies` | `companies` | `companies_list` | — |
| `POST` | `/api/v1/companies` | `companies` | `companies_create` | — |
| `DELETE` | `/api/v1/companies/{id}` | `companies` | `companies_delete_one` | — |
| `GET` | `/api/v1/companies/{id}` | `companies` | `companies_get_one` | — |
| `GET` | `/api/v1/companies/{id}/contacts` | `companies` | `companies_list_contacts` | — |
| `POST` | `/api/v1/companies/{id}/contacts` | `companies` | `companies_create_contact` | — |
| `POST` | `/api/v1/currencies/convert` | `currency` | `convert_currency` | — |
| `GET` | `/api/v1/currencies/rates` | `currency` | `list_rates` | — |
| `POST` | `/api/v1/currencies/rates` | `currency` | `set_rate` | — |
| `GET` | `/api/v1/customers` | `customers` | `list_customers` | `GET /api/v1/customers` |
| `POST` | `/api/v1/customers` | `customers` | `create_customer` | `POST /api/v1/customers` |
| `DELETE` | `/api/v1/customers/{id}` | `customers` | `delete_customer` | `DELETE /api/v1/customers/:id` |
| `GET` | `/api/v1/customers/{id}` | `customers` | `get_customer` | `GET /api/v1/customers/:id` |
| `PATCH` | `/api/v1/customers/{id}` | `customers` | `update_customer` | `PATCH /api/v1/customers/:id` |
| `GET` | `/api/v1/cycle-counts` | `warehouse` | `cycle_count_list` | — |
| `POST` | `/api/v1/cycle-counts` | `warehouse` | `cycle_count_create` | — |
| `GET` | `/api/v1/cycle-counts/{id}` | `warehouse` | `cycle_count_get_one` | — |
| `POST` | `/api/v1/cycle-counts/{id}/cancel` | `warehouse` | `cycle_count_cancel` | — |
| `POST` | `/api/v1/cycle-counts/{id}/complete` | `warehouse` | `cycle_count_complete` | — |
| `POST` | `/api/v1/cycle-counts/{id}/counts` | `warehouse` | `cycle_count_record_counts` | — |
| `POST` | `/api/v1/cycle-counts/{id}/start` | `warehouse` | `cycle_count_start` | — |
| `GET` | `/api/v1/edi-documents` | `edi_documents` | `edi_documents_list` | — |
| `POST` | `/api/v1/edi-documents` | `edi_documents` | `edi_documents_create` | — |
| `GET` | `/api/v1/edi-documents/{id}` | `edi_documents` | `edi_documents_get_one` | — |
| `POST` | `/api/v1/edi-documents/{id}/status` | `edi_documents` | `edi_documents_set_status` | — |
| `GET` | `/api/v1/edi-documents/summary` | `edi_documents` | `edi_documents_summary` | — |
| `GET` | `/api/v1/events/stream` | `events` | `event_stream` | `GET /api/v1/events/stream` — SSE endpoint. |
| `GET` | `/api/v1/fixed-assets` | `fixed_assets` | `fixed_assets_list` | — |
| `POST` | `/api/v1/fixed-assets` | `fixed_assets` | `fixed_assets_create` | — |
| `GET` | `/api/v1/fixed-assets/{id}` | `fixed_assets` | `fixed_assets_get_one` | — |
| `PUT` | `/api/v1/fixed-assets/{id}` | `fixed_assets` | `fixed_assets_update` | — |
| `POST` | `/api/v1/fixed-assets/{id}/dispose` | `fixed_assets` | `fixed_assets_dispose` | — |
| `POST` | `/api/v1/fixed-assets/{id}/place-in-service` | `fixed_assets` | `fixed_assets_place_in_service` | — |
| `POST` | `/api/v1/fixed-assets/{id}/post-depreciation` | `fixed_assets` | `fixed_assets_post_depreciation` | — |
| `GET` | `/api/v1/fixed-assets/{id}/schedule` | `fixed_assets` | `fixed_assets_get_schedule` | — |
| `POST` | `/api/v1/fixed-assets/{id}/schedule` | `fixed_assets` | `fixed_assets_generate_schedule` | — |
| `POST` | `/api/v1/fixed-assets/{id}/write-off` | `fixed_assets` | `fixed_assets_write_off` | — |
| `GET` | `/api/v1/fulfillment/packs` | `fulfillment` | `fulfillment_pack_list` | — |
| `GET` | `/api/v1/fulfillment/packs/{id}/cartons` | `fulfillment` | `fulfillment_carton_list` | — |
| `POST` | `/api/v1/fulfillment/packs/{id}/cartons` | `fulfillment` | `fulfillment_carton_add` | — |
| `POST` | `/api/v1/fulfillment/packs/{id}/complete` | `fulfillment` | `fulfillment_pack_complete` | — |
| `GET` | `/api/v1/fulfillment/picks` | `fulfillment` | `fulfillment_pick_list` | — |
| `POST` | `/api/v1/fulfillment/picks/{id}/assign` | `fulfillment` | `fulfillment_pick_assign` | — |
| `POST` | `/api/v1/fulfillment/picks/{id}/complete` | `fulfillment` | `fulfillment_pick_complete` | — |
| `GET` | `/api/v1/fulfillment/ships` | `fulfillment` | `fulfillment_ship_list` | — |
| `POST` | `/api/v1/fulfillment/ships/{id}/complete` | `fulfillment` | `fulfillment_ship_complete` | — |
| `GET` | `/api/v1/fulfillment/waves` | `fulfillment` | `fulfillment_wave_list` | — |
| `POST` | `/api/v1/fulfillment/waves` | `fulfillment` | `fulfillment_wave_create` | — |
| `GET` | `/api/v1/fulfillment/waves/{id}` | `fulfillment` | `fulfillment_wave_get_one` | — |
| `POST` | `/api/v1/fulfillment/waves/{id}/release` | `fulfillment` | `fulfillment_wave_release` | — |
| `GET` | `/api/v1/gift-cards` | `gift_cards` | `list_gift_cards` | `GET /api/v1/gift-cards` |
| `POST` | `/api/v1/gift-cards` | `gift_cards` | `create_gift_card` | `POST /api/v1/gift-cards` |
| `GET` | `/api/v1/gift-cards/{id}` | `gift_cards` | `get_gift_card` | `GET /api/v1/gift-cards/{id}` |
| `POST` | `/api/v1/gift-cards/{id}/charge` | `gift_cards` | `charge_gift_card` | `POST /api/v1/gift-cards/{id}/charge` |
| `POST` | `/api/v1/gift-cards/{id}/disable` | `gift_cards` | `disable_gift_card` | `POST /api/v1/gift-cards/{id}/disable` |
| `POST` | `/api/v1/gift-cards/{id}/refund` | `gift_cards` | `refund_gift_card` | `POST /api/v1/gift-cards/{id}/refund` |
| `GET` | `/api/v1/gl/accounts` | `general_ledger` | `general_ledger_list_accounts` | — |
| `POST` | `/api/v1/gl/accounts` | `general_ledger` | `general_ledger_create_account` | — |
| `GET` | `/api/v1/gl/accounts/{id}` | `general_ledger` | `general_ledger_get_account` | — |
| `GET` | `/api/v1/gl/balance-sheet` | `general_ledger` | `general_ledger_balance_sheet` | — |
| `POST` | `/api/v1/gl/close-month` | `general_ledger` | `general_ledger_close_month` | — |
| `GET` | `/api/v1/gl/income-statement` | `general_ledger` | `general_ledger_income_statement` | — |
| `GET` | `/api/v1/gl/journal-entries` | `general_ledger` | `general_ledger_list_journal_entries` | — |
| `POST` | `/api/v1/gl/journal-entries` | `general_ledger` | `general_ledger_create_journal_entry` | — |
| `GET` | `/api/v1/gl/journal-entries/{id}` | `general_ledger` | `general_ledger_get_journal_entry` | — |
| `POST` | `/api/v1/gl/journal-entries/{id}/post` | `general_ledger` | `general_ledger_post_journal_entry` | — |
| `POST` | `/api/v1/gl/journal-entries/{id}/reverse` | `general_ledger` | `general_ledger_reverse_journal_entry` | — |
| `POST` | `/api/v1/gl/journal-entries/{id}/void` | `general_ledger` | `general_ledger_void_journal_entry` | — |
| `GET` | `/api/v1/gl/periods` | `general_ledger` | `general_ledger_list_periods` | — |
| `POST` | `/api/v1/gl/periods` | `general_ledger` | `general_ledger_create_period` | — |
| `POST` | `/api/v1/gl/periods/{id}/close` | `general_ledger` | `general_ledger_close_period` | — |
| `POST` | `/api/v1/gl/periods/{id}/lock` | `general_ledger` | `general_ledger_lock_period` | — |
| `POST` | `/api/v1/gl/periods/{id}/open` | `general_ledger` | `general_ledger_open_period` | — |
| `POST` | `/api/v1/gl/periods/{id}/reopen` | `general_ledger` | `general_ledger_reopen_period` | — |
| `POST` | `/api/v1/gl/revalue` | `general_ledger` | `general_ledger_revalue` | — |
| `GET` | `/api/v1/gl/trial-balance` | `general_ledger` | `general_ledger_trial_balance` | — |
| `GET` | `/api/v1/inbound-shipments` | `inbound_shipments` | `inbound_shipments_list` | — |
| `POST` | `/api/v1/inbound-shipments` | `inbound_shipments` | `inbound_shipments_create` | — |
| `GET` | `/api/v1/inbound-shipments/{id}` | `inbound_shipments` | `inbound_shipments_get_one` | — |
| `POST` | `/api/v1/inbound-shipments/{id}/arrived` | `inbound_shipments` | `inbound_shipments_mark_arrived` | — |
| `POST` | `/api/v1/inbound-shipments/{id}/cancel` | `inbound_shipments` | `inbound_shipments_cancel` | — |
| `POST` | `/api/v1/inbound-shipments/{id}/in-transit` | `inbound_shipments` | `inbound_shipments_mark_in_transit` | — |
| `POST` | `/api/v1/inbound-shipments/{id}/receive` | `inbound_shipments` | `inbound_shipments_receive` | — |
| `GET` | `/api/v1/integration-field-mappings` | `integration_field_mappings` | `integration_field_mappings_list` | — |
| `POST` | `/api/v1/integration-field-mappings` | `integration_field_mappings` | `integration_field_mappings_create` | — |
| `DELETE` | `/api/v1/integration-field-mappings/{id}` | `integration_field_mappings` | `integration_field_mappings_delete_one` | — |
| `GET` | `/api/v1/integration-field-mappings/{id}` | `integration_field_mappings` | `integration_field_mappings_get_one` | — |
| `PUT` | `/api/v1/integration-field-mappings/{id}` | `integration_field_mappings` | `integration_field_mappings_update` | — |
| `DELETE` | `/api/v1/integration-field-mappings/bulk` | `integration_field_mappings` | `integration_field_mappings_bulk_delete` | — |
| `POST` | `/api/v1/integration-field-mappings/bulk` | `integration_field_mappings` | `integration_field_mappings_bulk_create` | — |
| `GET` | `/api/v1/integration-field-mappings/groups` | `integration_field_mappings` | `integration_field_mappings_groups` | — |
| `GET` | `/api/v1/integration-mappings` | `integration_mappings` | `integration_mappings_list` | — |
| `POST` | `/api/v1/integration-mappings` | `integration_mappings` | `integration_mappings_create` | — |
| `DELETE` | `/api/v1/integration-mappings/{id}` | `integration_mappings` | `integration_mappings_delete_one` | — |
| `GET` | `/api/v1/integration-mappings/{id}` | `integration_mappings` | `integration_mappings_get_one` | — |
| `PUT` | `/api/v1/integration-mappings/{id}` | `integration_mappings` | `integration_mappings_update` | — |
| `POST` | `/api/v1/integration-mappings/bulk` | `integration_mappings` | `integration_mappings_bulk_create` | — |
| `GET` | `/api/v1/integration-mappings/resolve` | `integration_mappings` | `integration_mappings_resolve` | — |
| `GET` | `/api/v1/inventory` | `inventory` | `list_inventory` | `GET /api/v1/inventory` |
| `GET` | `/api/v1/inventory/{sku}` | `inventory` | `get_stock` | `GET /api/v1/inventory/:sku` |
| `POST` | `/api/v1/inventory/{sku}/adjust` | `inventory` | `adjust_stock` | `POST /api/v1/inventory/:sku/adjust` |
| `GET` | `/api/v1/invoices` | `invoices` | `list_invoices` | `GET /api/v1/invoices` |
| `POST` | `/api/v1/invoices` | `invoices` | `create_invoice` | `POST /api/v1/invoices` |
| `GET` | `/api/v1/invoices/{id}` | `invoices` | `get_invoice` | `GET /api/v1/invoices/:id` |
| `POST` | `/api/v1/invoices/{id}/payments` | `invoices` | `record_invoice_payment` | `POST /api/v1/invoices/:id/payments` |
| `POST` | `/api/v1/invoices/{id}/send` | `invoices` | `send_invoice` | `POST /api/v1/invoices/:id/send` |
| `GET` | `/api/v1/lots` | `lots` | `lots_list` | — |
| `POST` | `/api/v1/lots` | `lots` | `lots_create` | — |
| `GET` | `/api/v1/lots/{id}` | `lots` | `lots_get_one` | — |
| `POST` | `/api/v1/lots/{id}/consume` | `lots` | `lots_consume` | — |
| `POST` | `/api/v1/lots/{id}/quarantine` | `lots` | `lots_quarantine` | — |
| `POST` | `/api/v1/lots/{id}/release-quarantine` | `lots` | `lots_release_quarantine` | — |
| `POST` | `/api/v1/lots/{id}/reserve` | `lots` | `lots_reserve` | — |
| `GET` | `/api/v1/lots/expiring` | `lots` | `lots_expiring` | — |
| `POST` | `/api/v1/lots/reservations/{reservation_id}/release` | `lots` | `lots_release_reservation` | — |
| `GET` | `/api/v1/loyalty/accounts/{id}` | `loyalty` | `get_account` | `GET /api/v1/loyalty/accounts/{id}` |
| `POST` | `/api/v1/loyalty/enroll` | `loyalty` | `enroll_customer` | `POST /api/v1/loyalty/enroll` |
| `GET` | `/api/v1/loyalty/programs` | `loyalty` | `list_programs` | `GET /api/v1/loyalty/programs` |
| `POST` | `/api/v1/loyalty/programs` | `loyalty` | `create_program` | `POST /api/v1/loyalty/programs` |
| `POST` | `/api/v1/negotiations` | `negotiations` | `create_negotiation` | `POST /api/v1/negotiations` |
| `GET` | `/api/v1/negotiations/{id}` | `negotiations` | `get_negotiation` | `GET /api/v1/negotiations/:id` |
| `POST` | `/api/v1/negotiations/{id}/accept` | `negotiations` | `accept_negotiation` | `POST /api/v1/negotiations/:id/accept` |
| `POST` | `/api/v1/negotiations/{id}/counter-offer` | `negotiations` | `counter_offer` | `POST /api/v1/negotiations/:id/counter-offer` |
| `POST` | `/api/v1/negotiations/{id}/reject` | `negotiations` | `reject_negotiation` | `POST /api/v1/negotiations/:id/reject` |
| `GET` | `/api/v1/orders` | `orders` | `list_orders` | `GET /api/v1/orders` |
| `POST` | `/api/v1/orders` | `orders` | `create_order` | `POST /api/v1/orders` |
| `GET` | `/api/v1/orders/{id}` | `orders` | `get_order` | `GET /api/v1/orders/:id` |
| `PATCH` | `/api/v1/orders/{id}/cancel` | `orders` | `cancel_order` | `PATCH /api/v1/orders/:id/cancel` |
| `PATCH` | `/api/v1/orders/{id}/ship` | `orders` | `ship_order` | `PATCH /api/v1/orders/:id/ship` |
| `GET` | `/api/v1/payment-obligations` | `payment_obligations` | `payment_obligations_list` | — |
| `POST` | `/api/v1/payment-obligations` | `payment_obligations` | `payment_obligations_create` | — |
| `GET` | `/api/v1/payment-obligations/{id}` | `payment_obligations` | `payment_obligations_get_one` | — |
| `POST` | `/api/v1/payment-obligations/{id}/bills` | `payment_obligations` | `payment_obligations_link_bill` | — |
| `POST` | `/api/v1/payment-obligations/{id}/payments` | `payment_obligations` | `payment_obligations_record_payment` | — |
| `POST` | `/api/v1/payment-obligations/{id}/status` | `payment_obligations` | `payment_obligations_set_status` | — |
| `GET` | `/api/v1/payment-obligations/dashboard` | `payment_obligations` | `payment_obligations_dashboard` | — |
| `GET` | `/api/v1/payments` | `payments` | `list_payments` | `GET /api/v1/payments` |
| `POST` | `/api/v1/payments` | `payments` | `create_payment` | `POST /api/v1/payments` |
| `GET` | `/api/v1/payments/{id}` | `payments` | `get_payment` | `GET /api/v1/payments/:id` |
| `POST` | `/api/v1/payments/{id}/complete` | `payments` | `complete_payment` | `POST /api/v1/payments/:id/complete` |
| `POST` | `/api/v1/payments/{id}/refund` | `payments` | `create_refund` | `POST /api/v1/payments/:id/refund` |
| `GET` | `/api/v1/prepayments` | `prepayments` | `prepayments_list` | — |
| `POST` | `/api/v1/prepayments` | `prepayments` | `prepayments_create` | — |
| `GET` | `/api/v1/prepayments/{id}` | `prepayments` | `prepayments_get_one` | — |
| `POST` | `/api/v1/prepayments/{id}/apply` | `prepayments` | `prepayments_apply` | — |
| `POST` | `/api/v1/prepayments/{id}/refund` | `prepayments` | `prepayments_refund` | — |
| `GET` | `/api/v1/price-levels` | `price_levels` | `price_levels_list` | — |
| `POST` | `/api/v1/price-levels` | `price_levels` | `price_levels_create` | — |
| `DELETE` | `/api/v1/price-levels/{id}` | `price_levels` | `price_levels_delete_one` | — |
| `GET` | `/api/v1/price-levels/{id}` | `price_levels` | `price_levels_get_one` | — |
| `PUT` | `/api/v1/price-levels/{id}` | `price_levels` | `price_levels_update` | — |
| `GET` | `/api/v1/price-levels/{id}/entries` | `price_levels` | `price_levels_list_entries` | — |
| `POST` | `/api/v1/price-levels/{id}/entries` | `price_levels` | `price_levels_set_entry` | — |
| `GET` | `/api/v1/price-schedules` | `price_schedules` | `price_schedules_list` | — |
| `POST` | `/api/v1/price-schedules` | `price_schedules` | `price_schedules_create` | — |
| `DELETE` | `/api/v1/price-schedules/{id}` | `price_schedules` | `price_schedules_delete_one` | — |
| `GET` | `/api/v1/price-schedules/{id}` | `price_schedules` | `price_schedules_get_one` | — |
| `GET` | `/api/v1/price-schedules/{id}/entries` | `price_schedules` | `price_schedules_list_entries` | — |
| `POST` | `/api/v1/price-schedules/{id}/entries` | `price_schedules` | `price_schedules_set_entry` | — |
| `GET` | `/api/v1/price-schedules/resolve` | `price_schedules` | `price_schedules_resolve` | — |
| `POST` | `/api/v1/print-jobs/{job_id}/complete` | `print_stations` | `print_stations_complete_job` | — |
| `GET` | `/api/v1/print-stations` | `print_stations` | `print_stations_list_stations` | — |
| `POST` | `/api/v1/print-stations` | `print_stations` | `print_stations_pair` | — |
| `GET` | `/api/v1/print-stations/{id}/jobs` | `print_stations` | `print_stations_list_jobs` | — |
| `POST` | `/api/v1/print-stations/{id}/jobs` | `print_stations` | `print_stations_enqueue` | — |
| `POST` | `/api/v1/print-stations/{id}/jobs/next` | `print_stations` | `print_stations_next_job` | — |
| `POST` | `/api/v1/print-stations/{id}/revoke` | `print_stations` | `print_stations_revoke` | — |
| `GET` | `/api/v1/production-batches` | `production_batches` | `production_batches_list` | — |
| `POST` | `/api/v1/production-batches` | `production_batches` | `production_batches_create` | — |
| `DELETE` | `/api/v1/production-batches/{id}` | `production_batches` | `production_batches_delete_one` | — |
| `GET` | `/api/v1/production-batches/{id}` | `production_batches` | `production_batches_get_one` | — |
| `POST` | `/api/v1/production-batches/{id}/work-orders` | `production_batches` | `production_batches_add_work_orders` | — |
| `GET` | `/api/v1/products` | `products` | `list_products` | `GET /api/v1/products` |
| `POST` | `/api/v1/products` | `products` | `create_product` | `POST /api/v1/products` |
| `DELETE` | `/api/v1/products/{id}` | `products` | `delete_product` | `DELETE /api/v1/products/:id` |
| `GET` | `/api/v1/products/{id}` | `products` | `get_product` | `GET /api/v1/products/:id` |
| `PATCH` | `/api/v1/products/{id}` | `products` | `update_product` | `PATCH /api/v1/products/:id` |
| `GET` | `/api/v1/promotions` | `promotions` | `list_promotions` | — |
| `POST` | `/api/v1/promotions` | `promotions` | `create_promotion` | — |
| `GET` | `/api/v1/promotions/{id}` | `promotions` | `get_promotion` | — |
| `PATCH` | `/api/v1/promotions/{id}/activate` | `promotions` | `activate_promotion` | — |
| `PATCH` | `/api/v1/promotions/{id}/deactivate` | `promotions` | `deactivate_promotion` | — |
| `GET` | `/api/v1/purchase-orders` | `purchase_orders` | `purchase_orders_list` | — |
| `POST` | `/api/v1/purchase-orders` | `purchase_orders` | `purchase_orders_create` | — |
| `GET` | `/api/v1/purchase-orders/{id}` | `purchase_orders` | `purchase_orders_get_one` | — |
| `PUT` | `/api/v1/purchase-orders/{id}` | `purchase_orders` | `purchase_orders_update` | — |
| `POST` | `/api/v1/purchase-orders/{id}/acknowledge` | `purchase_orders` | `purchase_orders_acknowledge` | — |
| `POST` | `/api/v1/purchase-orders/{id}/approve` | `purchase_orders` | `purchase_orders_approve` | — |
| `POST` | `/api/v1/purchase-orders/{id}/cancel` | `purchase_orders` | `purchase_orders_cancel` | — |
| `POST` | `/api/v1/purchase-orders/{id}/complete` | `purchase_orders` | `purchase_orders_complete` | — |
| `POST` | `/api/v1/purchase-orders/{id}/hold` | `purchase_orders` | `purchase_orders_hold` | — |
| `GET` | `/api/v1/purchase-orders/{id}/items` | `purchase_orders` | `purchase_orders_get_items` | — |
| `POST` | `/api/v1/purchase-orders/{id}/receive` | `purchase_orders` | `purchase_orders_receive` | — |
| `POST` | `/api/v1/purchase-orders/{id}/send` | `purchase_orders` | `purchase_orders_send` | — |
| `POST` | `/api/v1/purchase-orders/{id}/submit` | `purchase_orders` | `purchase_orders_submit` | — |
| `GET` | `/api/v1/purgatory/orders` | `purgatory` | `purgatory_list` | — |
| `POST` | `/api/v1/purgatory/orders` | `purgatory` | `purgatory_ingest` | — |
| `DELETE` | `/api/v1/purgatory/orders/{id}` | `purgatory` | `purgatory_delete_one` | — |
| `GET` | `/api/v1/purgatory/orders/{id}` | `purgatory` | `purgatory_get_one` | — |
| `POST` | `/api/v1/purgatory/orders/{id}/lines/{line_id}` | `purgatory` | `purgatory_map_line` | — |
| `POST` | `/api/v1/purgatory/orders/{id}/post` | `purgatory` | `purgatory_post_order` | — |
| `GET` | `/api/v1/put-aways` | `receiving` | `receiving_put_away_list` | — |
| `POST` | `/api/v1/put-aways` | `receiving` | `receiving_put_away_create` | — |
| `GET` | `/api/v1/put-aways/{id}` | `receiving` | `receiving_put_away_get_one` | — |
| `POST` | `/api/v1/put-aways/{id}/complete` | `receiving` | `receiving_put_away_complete` | — |
| `GET` | `/api/v1/quality/holds` | `quality` | `quality_list_holds` | — |
| `POST` | `/api/v1/quality/holds` | `quality` | `quality_create_hold` | — |
| `GET` | `/api/v1/quality/holds/{id}` | `quality` | `quality_get_hold` | — |
| `POST` | `/api/v1/quality/holds/{id}/release` | `quality` | `quality_release_hold` | — |
| `GET` | `/api/v1/quality/inspections` | `quality` | `quality_list_inspections` | — |
| `POST` | `/api/v1/quality/inspections` | `quality` | `quality_create_inspection` | — |
| `GET` | `/api/v1/quality/inspections/{id}` | `quality` | `quality_get_inspection` | — |
| `POST` | `/api/v1/quality/inspections/{id}/complete` | `quality` | `quality_complete_inspection` | — |
| `POST` | `/api/v1/quality/inspections/{id}/results` | `quality` | `quality_record_inspection_result` | — |
| `POST` | `/api/v1/quality/inspections/{id}/start` | `quality` | `quality_start_inspection` | — |
| `GET` | `/api/v1/quality/ncrs` | `quality` | `quality_list_ncrs` | — |
| `POST` | `/api/v1/quality/ncrs` | `quality` | `quality_create_ncr` | — |
| `GET` | `/api/v1/quality/ncrs/{id}` | `quality` | `quality_get_ncr` | — |
| `POST` | `/api/v1/quality/ncrs/{id}/close` | `quality` | `quality_close_ncr` | — |
| `POST` | `/api/v1/quality/ncrs/{id}/disposition` | `quality` | `quality_disposition_ncr` | — |
| `GET` | `/api/v1/receipts` | `receiving` | `receiving_receipt_list` | — |
| `POST` | `/api/v1/receipts` | `receiving` | `receiving_receipt_create` | — |
| `GET` | `/api/v1/receipts/{id}` | `receiving` | `receiving_receipt_get_one` | — |
| `POST` | `/api/v1/receipts/{id}/cancel` | `receiving` | `receiving_receipt_cancel` | — |
| `POST` | `/api/v1/receipts/{id}/complete` | `receiving` | `receiving_receipt_complete` | — |
| `GET` | `/api/v1/receipts/{id}/items` | `receiving` | `receiving_receipt_items` | — |
| `POST` | `/api/v1/receipts/{id}/receive` | `receiving` | `receiving_receipt_receive_items` | — |
| `POST` | `/api/v1/receipts/{id}/start` | `receiving` | `receiving_receipt_start` | — |
| `POST` | `/api/v1/reports/close-the-books` | `reports` | `reports_close_the_books` | — |
| `POST` | `/api/v1/reports/consumption` | `reports` | `reports_consumption` | — |
| `POST` | `/api/v1/reports/inventory-aging` | `reports` | `reports_inventory_aging` | — |
| `POST` | `/api/v1/reports/sales-by-channel` | `reports` | `reports_sales_by_channel` | — |
| `POST` | `/api/v1/reports/transaction-cogs` | `reports` | `reports_transaction_cogs` | — |
| `GET` | `/api/v1/returns` | `returns` | `list_returns` | `GET /api/v1/returns` |
| `POST` | `/api/v1/returns` | `returns` | `create_return` | `POST /api/v1/returns` |
| `GET` | `/api/v1/returns/{id}` | `returns` | `get_return` | `GET /api/v1/returns/:id` |
| `PATCH` | `/api/v1/returns/{id}/approve` | `returns` | `approve_return` | `PATCH /api/v1/returns/:id/approve` |
| `POST` | `/api/v1/returns/{id}/items/{item_id}/disposition` | `returns` | `return_item_set_disposition` | `POST /api/v1/returns/:id/items/:item_id/disposition` |
| `GET` | `/api/v1/revenue-contracts` | `revenue_recognition` | `revenue_recognition_list_contracts` | — |
| `POST` | `/api/v1/revenue-contracts` | `revenue_recognition` | `revenue_recognition_create_contract` | — |
| `GET` | `/api/v1/revenue-contracts/{id}` | `revenue_recognition` | `revenue_recognition_get_contract` | — |
| `PUT` | `/api/v1/revenue-contracts/{id}` | `revenue_recognition` | `revenue_recognition_update_contract` | — |
| `GET` | `/api/v1/revenue-contracts/{id}/obligations` | `revenue_recognition` | `revenue_recognition_list_obligations` | — |
| `POST` | `/api/v1/revenue-obligations/{id}/recognize` | `revenue_recognition` | `revenue_recognition_recognize` | — |
| `GET` | `/api/v1/revenue-obligations/{id}/schedule` | `revenue_recognition` | `revenue_recognition_get_schedule` | — |
| `POST` | `/api/v1/revenue-obligations/{id}/schedule` | `revenue_recognition` | `revenue_recognition_generate_schedule` | — |
| `GET` | `/api/v1/reviews` | `reviews` | `list_reviews` | `GET /api/v1/reviews` |
| `POST` | `/api/v1/reviews` | `reviews` | `create_review` | `POST /api/v1/reviews` |
| `DELETE` | `/api/v1/reviews/{id}` | `reviews` | `delete_review` | `DELETE /api/v1/reviews/{id}` |
| `GET` | `/api/v1/reviews/{id}` | `reviews` | `get_review` | `GET /api/v1/reviews/{id}` |
| `GET` | `/api/v1/segments` | `segments` | `list_segments` | — |
| `POST` | `/api/v1/segments` | `segments` | `create_segment` | — |
| `DELETE` | `/api/v1/segments/{id}` | `segments` | `delete_segment` | — |
| `GET` | `/api/v1/segments/{id}` | `segments` | `get_segment` | — |
| `DELETE` | `/api/v1/segments/{id}/members/{customer_id}` | `segments` | `remove_member` | — |
| `POST` | `/api/v1/segments/{id}/members/{customer_id}` | `segments` | `add_member` | — |
| `GET` | `/api/v1/serials` | `serials` | `serials_list` | — |
| `POST` | `/api/v1/serials` | `serials` | `serials_create` | — |
| `GET` | `/api/v1/serials/{id}` | `serials` | `serials_get_one` | — |
| `POST` | `/api/v1/serials/{id}/reserve` | `serials` | `serials_reserve` | — |
| `POST` | `/api/v1/serials/{id}/return` | `serials` | `serials_return` | — |
| `POST` | `/api/v1/serials/{id}/scrap` | `serials` | `serials_scrap` | — |
| `POST` | `/api/v1/serials/{id}/ship` | `serials` | `serials_ship` | — |
| `POST` | `/api/v1/serials/reservations/{reservation_id}/release` | `serials` | `serials_release_reservation` | — |
| `GET` | `/api/v1/shipments` | `shipments` | `list_shipments` | `GET /api/v1/shipments` |
| `POST` | `/api/v1/shipments` | `shipments` | `create_shipment` | `POST /api/v1/shipments` |
| `GET` | `/api/v1/shipments/{id}` | `shipments` | `get_shipment` | `GET /api/v1/shipments/:id` |
| `POST` | `/api/v1/shipments/{id}/deliver` | `shipments` | `deliver_shipment` | `POST /api/v1/shipments/:id/deliver` |
| `GET` | `/api/v1/shipping-zones` | `shipping` | `list_zones` | — |
| `POST` | `/api/v1/shipping-zones` | `shipping` | `create_zone` | — |
| `DELETE` | `/api/v1/shipping-zones/{id}` | `shipping` | `delete_zone` | — |
| `GET` | `/api/v1/shipping-zones/{id}` | `shipping` | `get_zone` | — |
| `GET` | `/api/v1/stock-snapshots` | `stock_snapshots` | `stock_snapshots_list` | — |
| `POST` | `/api/v1/stock-snapshots` | `stock_snapshots` | `stock_snapshots_capture` | — |
| `DELETE` | `/api/v1/stock-snapshots/{id}` | `stock_snapshots` | `stock_snapshots_delete_one` | — |
| `GET` | `/api/v1/stock-snapshots/{id}` | `stock_snapshots` | `stock_snapshots_get_one` | — |
| `GET` | `/api/v1/stock-snapshots/latest` | `stock_snapshots` | `stock_snapshots_latest` | — |
| `GET` | `/api/v1/store-credits` | `store_credits` | `list_store_credits` | — |
| `POST` | `/api/v1/store-credits` | `store_credits` | `create_store_credit` | — |
| `GET` | `/api/v1/store-credits/{id}` | `store_credits` | `get_store_credit` | — |
| `POST` | `/api/v1/store-credits/{id}/adjust` | `store_credits` | `adjust_store_credit` | — |
| `POST` | `/api/v1/store-credits/{id}/apply` | `store_credits` | `apply_store_credit` | `POST /api/v1/store-credits/{id}/apply` — debit a store credit (e.g.
against an order). Rejected for non-positive amounts, non-active or
expired credits, and insufficient balance. |
| `GET` | `/api/v1/subscriptions` | `subscriptions` | `list_subscriptions` | — |
| `POST` | `/api/v1/subscriptions` | `subscriptions` | `create_subscription` | — |
| `GET` | `/api/v1/subscriptions/{id}` | `subscriptions` | `get_subscription` | — |
| `PATCH` | `/api/v1/subscriptions/{id}/cancel` | `subscriptions` | `cancel_subscription` | — |
| `PATCH` | `/api/v1/subscriptions/{id}/pause` | `subscriptions` | `pause_subscription` | — |
| `PATCH` | `/api/v1/subscriptions/{id}/resume` | `subscriptions` | `resume_subscription` | — |
| `GET` | `/api/v1/supplier-skus` | `supplier_skus` | `supplier_skus_list` | — |
| `POST` | `/api/v1/supplier-skus` | `supplier_skus` | `supplier_skus_create` | — |
| `DELETE` | `/api/v1/supplier-skus/{id}` | `supplier_skus` | `supplier_skus_delete_one` | — |
| `GET` | `/api/v1/supplier-skus/{id}` | `supplier_skus` | `supplier_skus_get_one` | — |
| `GET` | `/api/v1/suppliers` | `purchase_orders` | `purchase_orders_list_suppliers` | — |
| `POST` | `/api/v1/suppliers` | `purchase_orders` | `purchase_orders_create_supplier` | — |
| `DELETE` | `/api/v1/suppliers/{id}` | `purchase_orders` | `purchase_orders_delete_supplier` | — |
| `GET` | `/api/v1/suppliers/{id}` | `purchase_orders` | `purchase_orders_get_supplier` | — |
| `PUT` | `/api/v1/suppliers/{id}` | `purchase_orders` | `purchase_orders_update_supplier` | — |
| `GET` | `/api/v1/topology-snapshots` | `topology_snapshots` | `topology_snapshots_list` | — |
| `POST` | `/api/v1/topology-snapshots` | `topology_snapshots` | `topology_snapshots_capture` | — |
| `DELETE` | `/api/v1/topology-snapshots/{id}` | `topology_snapshots` | `topology_snapshots_delete_one` | — |
| `GET` | `/api/v1/topology-snapshots/{id}` | `topology_snapshots` | `topology_snapshots_get_one` | — |
| `GET` | `/api/v1/topology-snapshots/latest` | `topology_snapshots` | `topology_snapshots_latest` | — |
| `GET` | `/api/v1/transfer-orders` | `transfer_orders` | `transfer_orders_list` | — |
| `POST` | `/api/v1/transfer-orders` | `transfer_orders` | `transfer_orders_create` | — |
| `GET` | `/api/v1/transfer-orders/{id}` | `transfer_orders` | `transfer_orders_get_one` | — |
| `POST` | `/api/v1/transfer-orders/{id}/cancel` | `transfer_orders` | `transfer_orders_cancel` | — |
| `POST` | `/api/v1/transfer-orders/{id}/receive` | `transfer_orders` | `transfer_orders_receive` | — |
| `POST` | `/api/v1/transfer-orders/{id}/ship` | `transfer_orders` | `transfer_orders_ship` | — |
| `GET` | `/api/v1/unit-classes` | `units_of_measure` | `units_of_measure_list_classes` | — |
| `POST` | `/api/v1/unit-classes` | `units_of_measure` | `units_of_measure_create_class` | — |
| `GET` | `/api/v1/unit-conversion-rules` | `units_of_measure` | `units_of_measure_list_rules` | — |
| `POST` | `/api/v1/unit-conversion-rules` | `units_of_measure` | `units_of_measure_create_rule` | — |
| `GET` | `/api/v1/units-of-measure` | `units_of_measure` | `units_of_measure_list_uoms` | — |
| `POST` | `/api/v1/units-of-measure` | `units_of_measure` | `units_of_measure_create_uom` | — |
| `GET` | `/api/v1/vendor-credits` | `vendor_credits` | `vendor_credits_list` | — |
| `POST` | `/api/v1/vendor-credits` | `vendor_credits` | `vendor_credits_create` | — |
| `GET` | `/api/v1/vendor-credits/{id}` | `vendor_credits` | `vendor_credits_get_one` | — |
| `POST` | `/api/v1/vendor-credits/{id}/apply` | `vendor_credits` | `vendor_credits_apply` | — |
| `POST` | `/api/v1/vendor-credits/{id}/cancel` | `vendor_credits` | `vendor_credits_cancel` | — |
| `GET` | `/api/v1/vendor-returns` | `vendor_returns` | `vendor_returns_list` | — |
| `POST` | `/api/v1/vendor-returns` | `vendor_returns` | `vendor_returns_create` | — |
| `GET` | `/api/v1/vendor-returns/{id}` | `vendor_returns` | `vendor_returns_get_one` | — |
| `POST` | `/api/v1/vendor-returns/{id}/cancel` | `vendor_returns` | `vendor_returns_cancel` | — |
| `POST` | `/api/v1/vendor-returns/{id}/process` | `vendor_returns` | `vendor_returns_process` | — |
| `POST` | `/api/v1/vendor-returns/{id}/submit` | `vendor_returns` | `vendor_returns_submit` | — |
| `GET` | `/api/v1/warehouse-bins` | `warehouse` | `warehouse_bin_list` | — |
| `POST` | `/api/v1/warehouse-bins` | `warehouse` | `warehouse_bin_create` | — |
| `DELETE` | `/api/v1/warehouse-bins/{id}` | `warehouse` | `warehouse_bin_delete` | — |
| `GET` | `/api/v1/warehouse-bins/{id}` | `warehouse` | `warehouse_bin_get_one` | — |
| `PUT` | `/api/v1/warehouse-bins/{id}` | `warehouse` | `warehouse_bin_update` | — |
| `GET` | `/api/v1/warehouse-bins/{id}/levels` | `warehouse` | `warehouse_bin_levels` | — |
| `POST` | `/api/v1/warehouse-bins/adjust` | `warehouse` | `warehouse_bin_adjust` | — |
| `POST` | `/api/v1/warehouse-bins/move` | `warehouse` | `warehouse_bin_move` | — |
| `GET` | `/api/v1/warehouse-bins/reconcile` | `warehouse` | `warehouse_bin_reconcile` | — |
| `POST` | `/api/v1/warehouse-inventory/adjust` | `warehouse` | `warehouse_inventory_adjust` | — |
| `POST` | `/api/v1/warehouse-inventory/move` | `warehouse` | `warehouse_inventory_move` | — |
| `GET` | `/api/v1/warehouse-locations` | `warehouse` | `warehouse_location_list` | — |
| `POST` | `/api/v1/warehouse-locations` | `warehouse` | `warehouse_location_create` | — |
| `GET` | `/api/v1/warehouse-locations/{id}` | `warehouse` | `warehouse_location_get_one` | — |
| `GET` | `/api/v1/warehouse-locations/{id}/inventory` | `warehouse` | `warehouse_location_inventory` | — |
| `GET` | `/api/v1/warehouses` | `warehouse` | `warehouse_list` | — |
| `POST` | `/api/v1/warehouses` | `warehouse` | `warehouse_create` | — |
| `DELETE` | `/api/v1/warehouses/{id}` | `warehouse` | `warehouse_delete` | — |
| `GET` | `/api/v1/warehouses/{id}` | `warehouse` | `warehouse_get_one` | — |
| `PUT` | `/api/v1/warehouses/{id}` | `warehouse` | `warehouse_update` | — |
| `GET` | `/api/v1/warranties` | `warranties` | `list_warranties` | — |
| `POST` | `/api/v1/warranties` | `warranties` | `create_warranty` | — |
| `GET` | `/api/v1/warranties/{id}` | `warranties` | `get_warranty` | — |
| `GET` | `/api/v1/wishlists` | `wishlists` | `list_wishlists` | `GET /api/v1/wishlists` |
| `POST` | `/api/v1/wishlists` | `wishlists` | `create_wishlist` | `POST /api/v1/wishlists` |
| `DELETE` | `/api/v1/wishlists/{id}` | `wishlists` | `delete_wishlist` | `DELETE /api/v1/wishlists/{id}` |
| `GET` | `/api/v1/wishlists/{id}` | `wishlists` | `get_wishlist` | `GET /api/v1/wishlists/{id}` |
| `POST` | `/api/v1/wishlists/{id}/items` | `wishlists` | `add_item` | `POST /api/v1/wishlists/{id}/items` |
| `DELETE` | `/api/v1/wishlists/{id}/items/{product_id}` | `wishlists` | `remove_item` | `DELETE /api/v1/wishlists/{id}/items/{product_id}` |
| `GET` | `/api/v1/work-orders` | `work_orders` | `work_orders_list` | — |
| `POST` | `/api/v1/work-orders` | `work_orders` | `work_orders_create` | — |
| `GET` | `/api/v1/work-orders/{id}` | `work_orders` | `work_orders_get_one` | — |
| `PUT` | `/api/v1/work-orders/{id}` | `work_orders` | `work_orders_update` | — |
| `POST` | `/api/v1/work-orders/{id}/cancel` | `work_orders` | `work_orders_cancel` | — |
| `POST` | `/api/v1/work-orders/{id}/complete` | `work_orders` | `work_orders_complete` | — |
| `POST` | `/api/v1/work-orders/{id}/hold` | `work_orders` | `work_orders_hold` | — |
| `POST` | `/api/v1/work-orders/{id}/resume` | `work_orders` | `work_orders_resume` | — |
| `POST` | `/api/v1/work-orders/{id}/start` | `work_orders` | `work_orders_start` | — |
| `GET` | `/api/v1/work-orders/{id}/tasks` | `work_orders` | `work_orders_list_tasks` | — |
| `POST` | `/api/v1/work-orders/{id}/tasks` | `work_orders` | `work_orders_add_task` | — |
| `POST` | `/api/v1/work-orders/tasks/{task_id}/complete` | `work_orders` | `work_orders_complete_task` | — |
| `POST` | `/api/v1/work-orders/tasks/{task_id}/start` | `work_orders` | `work_orders_start_task` | — |
| `GET` | `/health` | `health` | `health` | `GET /health` — simple liveness probe. |
| `GET` | `/health/deep` | `health` | `deep_health` | `GET /health/deep` — deep health check with DB connectivity and metrics. |
| `GET` | `/health/ready` | `health` | `readiness` | `GET /health/ready` — readiness probe that checks DB connectivity. |
| `GET` | `/metrics` | `health` | `metrics` | `GET /metrics` — Prometheus-compatible operational metrics. |
| `GET` | `/version` | `health` | `version` | `GET /version` — build & release metadata. |
