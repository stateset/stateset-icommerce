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
| Paths | 164 |
| Operations | 236 |
| Schemas | 214 |
| Tags | 45 |

## Method Counts

| Method | Operations |
| --- | --- |
| DELETE | 20 |
| GET | 101 |
| PATCH | 10 |
| POST | 101 |
| PUT | 4 |

## Tag Counts

| Tag | Operations | Description |
| --- | --- | --- |
| `a2a` | 8 | Agent-to-agent messaging and credit terms |
| `activity_logs` | 4 | Record-level activity history |
| `channels` | 6 | Sales channel management |
| `companies` | 6 | B2B company account management |
| `currency` | 3 | Exchange rates and currency conversion |
| `customers` | 5 | Customer management |
| `edi_documents` | 5 | EDI document exchange |
| `events` | 1 | Real-time event streaming |
| `gift_cards` | 6 | Gift card management |
| `health` | 5 | Health check endpoints |
| `inbound_shipments` | 7 | Inbound shipment receiving |
| `integration_field_mappings` | 8 | Integration field-level mappings |
| `integration_mappings` | 7 | External integration record mappings |
| `inventory` | 3 | Stock and inventory management |
| `invoices` | 5 | Invoice management |
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
| `purgatory` | 6 | Quarantined record review |
| `reports` | 5 | Computed business reports |
| `returns` | 4 | Return request processing |
| `reviews` | 4 | Product review management |
| `segments` | 6 | Customer segment management |
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
| `warranties` | 3 | Product warranty management |
| `wishlists` | 6 | Customer wishlist management |

## Operations

| Method | Path | Tags | Operation ID | Summary |
| --- | --- | --- | --- | --- |
| `GET` | `/api/v1/a2a/credit` | `a2a` | `list_terms` | `GET /api/v1/a2a/credit` |
| `POST` | `/api/v1/a2a/credit` | `a2a` | `create_terms` | `POST /api/v1/a2a/credit` |
| `GET` | `/api/v1/a2a/credit/{id}` | `a2a` | `get_terms` | `GET /api/v1/a2a/credit/:id` |
| `POST` | `/api/v1/a2a/credit/{id}/charge` | `a2a` | `charge_credit` | `POST /api/v1/a2a/credit/:id/charge` |
| `POST` | `/api/v1/a2a/credit/{id}/payment` | `a2a` | `record_payment` | `POST /api/v1/a2a/credit/:id/payment` |
| `GET` | `/api/v1/a2a/messages` | `a2a` | `list_messages` | `GET /api/v1/a2a/messages` |
| `POST` | `/api/v1/a2a/messages` | `a2a` | `send_message` | `POST /api/v1/a2a/messages` |
| `POST` | `/api/v1/a2a/messages/{id}/acknowledge` | `a2a` | `acknowledge_message` | `POST /api/v1/a2a/messages/:id/acknowledge` |
| `GET` | `/api/v1/activity-logs` | `activity_logs` | `activity_logs_list` | — |
| `POST` | `/api/v1/activity-logs` | `activity_logs` | `activity_logs_record` | — |
| `GET` | `/api/v1/activity-logs/{id}` | `activity_logs` | `activity_logs_get_one` | — |
| `GET` | `/api/v1/activity-logs/{subject_type}/{subject_id}` | `activity_logs` | `activity_logs_history` | — |
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
| `GET` | `/api/v1/edi-documents` | `edi_documents` | `edi_documents_list` | — |
| `POST` | `/api/v1/edi-documents` | `edi_documents` | `edi_documents_create` | — |
| `GET` | `/api/v1/edi-documents/{id}` | `edi_documents` | `edi_documents_get_one` | — |
| `POST` | `/api/v1/edi-documents/{id}/status` | `edi_documents` | `edi_documents_set_status` | — |
| `GET` | `/api/v1/edi-documents/summary` | `edi_documents` | `edi_documents_summary` | — |
| `GET` | `/api/v1/events/stream` | `events` | `event_stream` | `GET /api/v1/events/stream` — SSE endpoint. |
| `GET` | `/api/v1/gift-cards` | `gift_cards` | `list_gift_cards` | `GET /api/v1/gift-cards` |
| `POST` | `/api/v1/gift-cards` | `gift_cards` | `create_gift_card` | `POST /api/v1/gift-cards` |
| `GET` | `/api/v1/gift-cards/{id}` | `gift_cards` | `get_gift_card` | `GET /api/v1/gift-cards/{id}` |
| `POST` | `/api/v1/gift-cards/{id}/charge` | `gift_cards` | `charge_gift_card` | `POST /api/v1/gift-cards/{id}/charge` |
| `POST` | `/api/v1/gift-cards/{id}/disable` | `gift_cards` | `disable_gift_card` | `POST /api/v1/gift-cards/{id}/disable` |
| `POST` | `/api/v1/gift-cards/{id}/refund` | `gift_cards` | `refund_gift_card` | `POST /api/v1/gift-cards/{id}/refund` |
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
| `GET` | `/api/v1/purgatory/orders` | `purgatory` | `purgatory_list` | — |
| `POST` | `/api/v1/purgatory/orders` | `purgatory` | `purgatory_ingest` | — |
| `DELETE` | `/api/v1/purgatory/orders/{id}` | `purgatory` | `purgatory_delete_one` | — |
| `GET` | `/api/v1/purgatory/orders/{id}` | `purgatory` | `purgatory_get_one` | — |
| `POST` | `/api/v1/purgatory/orders/{id}/lines/{line_id}` | `purgatory` | `purgatory_map_line` | — |
| `POST` | `/api/v1/purgatory/orders/{id}/post` | `purgatory` | `purgatory_post_order` | — |
| `POST` | `/api/v1/reports/close-the-books` | `reports` | `reports_close_the_books` | — |
| `POST` | `/api/v1/reports/consumption` | `reports` | `reports_consumption` | — |
| `POST` | `/api/v1/reports/inventory-aging` | `reports` | `reports_inventory_aging` | — |
| `POST` | `/api/v1/reports/sales-by-channel` | `reports` | `reports_sales_by_channel` | — |
| `POST` | `/api/v1/reports/transaction-cogs` | `reports` | `reports_transaction_cogs` | — |
| `GET` | `/api/v1/returns` | `returns` | `list_returns` | `GET /api/v1/returns` |
| `POST` | `/api/v1/returns` | `returns` | `create_return` | `POST /api/v1/returns` |
| `GET` | `/api/v1/returns/{id}` | `returns` | `get_return` | `GET /api/v1/returns/:id` |
| `PATCH` | `/api/v1/returns/{id}/approve` | `returns` | `approve_return` | `PATCH /api/v1/returns/:id/approve` |
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
| `GET` | `/api/v1/warranties` | `warranties` | `list_warranties` | — |
| `POST` | `/api/v1/warranties` | `warranties` | `create_warranty` | — |
| `GET` | `/api/v1/warranties/{id}` | `warranties` | `get_warranty` | — |
| `GET` | `/api/v1/wishlists` | `wishlists` | `list_wishlists` | `GET /api/v1/wishlists` |
| `POST` | `/api/v1/wishlists` | `wishlists` | `create_wishlist` | `POST /api/v1/wishlists` |
| `DELETE` | `/api/v1/wishlists/{id}` | `wishlists` | `delete_wishlist` | `DELETE /api/v1/wishlists/{id}` |
| `GET` | `/api/v1/wishlists/{id}` | `wishlists` | `get_wishlist` | `GET /api/v1/wishlists/{id}` |
| `POST` | `/api/v1/wishlists/{id}/items` | `wishlists` | `add_item` | `POST /api/v1/wishlists/{id}/items` |
| `DELETE` | `/api/v1/wishlists/{id}/items/{product_id}` | `wishlists` | `remove_item` | `DELETE /api/v1/wishlists/{id}/items/{product_id}` |
| `GET` | `/health` | `health` | `health` | `GET /health` — simple liveness probe. |
| `GET` | `/health/deep` | `health` | `deep_health` | `GET /health/deep` — deep health check with DB connectivity and metrics. |
| `GET` | `/health/ready` | `health` | `readiness` | `GET /health/ready` — readiness probe that checks DB connectivity. |
| `GET` | `/metrics` | `health` | `metrics` | `GET /metrics` — Prometheus-compatible operational metrics. |
| `GET` | `/version` | `health` | `version` | `GET /version` — build & release metadata. |
