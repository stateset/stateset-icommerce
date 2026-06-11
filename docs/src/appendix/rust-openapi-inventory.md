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
| Paths | 75 |
| Operations | 104 |
| Schemas | 87 |
| Tags | 23 |

## Method Counts

| Method | Operations |
| --- | --- |
| DELETE | 8 |
| GET | 47 |
| PATCH | 10 |
| POST | 39 |

## Tag Counts

| Tag | Operations | Description |
| --- | --- | --- |
| `a2a` | 8 | Agent-to-agent messaging and credit terms |
| `currency` | 3 | Exchange rates and currency conversion |
| `customers` | 5 | Customer management |
| `events` | 1 | Real-time event streaming |
| `gift_cards` | 4 | Gift card management |
| `health` | 5 | Health check endpoints |
| `inventory` | 3 | Stock and inventory management |
| `invoices` | 5 | Invoice management |
| `loyalty` | 4 | Loyalty program management |
| `negotiations` | 5 | Agent-to-agent price negotiation |
| `orders` | 5 | Order lifecycle management |
| `payments` | 5 | Payment transaction management |
| `products` | 5 | Product catalog |
| `promotions` | 5 | Promotion and discount management |
| `returns` | 4 | Return request processing |
| `reviews` | 4 | Product review management |
| `segments` | 6 | Customer segment management |
| `shipments` | 4 | Shipment tracking and management |
| `shipping` | 4 | Shipping zone management |
| `store_credits` | 4 | Store credit management |
| `subscriptions` | 6 | Recurring subscription management |
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
| `POST` | `/api/v1/currencies/convert` | `currency` | `convert_currency` | — |
| `GET` | `/api/v1/currencies/rates` | `currency` | `list_rates` | — |
| `POST` | `/api/v1/currencies/rates` | `currency` | `set_rate` | — |
| `GET` | `/api/v1/customers` | `customers` | `list_customers` | `GET /api/v1/customers` |
| `POST` | `/api/v1/customers` | `customers` | `create_customer` | `POST /api/v1/customers` |
| `DELETE` | `/api/v1/customers/{id}` | `customers` | `delete_customer` | `DELETE /api/v1/customers/:id` |
| `GET` | `/api/v1/customers/{id}` | `customers` | `get_customer` | `GET /api/v1/customers/:id` |
| `PATCH` | `/api/v1/customers/{id}` | `customers` | `update_customer` | `PATCH /api/v1/customers/:id` |
| `GET` | `/api/v1/events/stream` | `events` | `event_stream` | `GET /api/v1/events/stream` — SSE endpoint. |
| `GET` | `/api/v1/gift-cards` | `gift_cards` | `list_gift_cards` | `GET /api/v1/gift-cards` |
| `POST` | `/api/v1/gift-cards` | `gift_cards` | `create_gift_card` | `POST /api/v1/gift-cards` |
| `GET` | `/api/v1/gift-cards/{id}` | `gift_cards` | `get_gift_card` | `GET /api/v1/gift-cards/{id}` |
| `POST` | `/api/v1/gift-cards/{id}/disable` | `gift_cards` | `disable_gift_card` | `POST /api/v1/gift-cards/{id}/disable` |
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
| `GET` | `/api/v1/payments` | `payments` | `list_payments` | `GET /api/v1/payments` |
| `POST` | `/api/v1/payments` | `payments` | `create_payment` | `POST /api/v1/payments` |
| `GET` | `/api/v1/payments/{id}` | `payments` | `get_payment` | `GET /api/v1/payments/:id` |
| `POST` | `/api/v1/payments/{id}/complete` | `payments` | `complete_payment` | `POST /api/v1/payments/:id/complete` |
| `POST` | `/api/v1/payments/{id}/refund` | `payments` | `create_refund` | `POST /api/v1/payments/:id/refund` |
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
| `GET` | `/api/v1/store-credits` | `store_credits` | `list_store_credits` | — |
| `POST` | `/api/v1/store-credits` | `store_credits` | `create_store_credit` | — |
| `GET` | `/api/v1/store-credits/{id}` | `store_credits` | `get_store_credit` | — |
| `POST` | `/api/v1/store-credits/{id}/adjust` | `store_credits` | `adjust_store_credit` | — |
| `GET` | `/api/v1/subscriptions` | `subscriptions` | `list_subscriptions` | — |
| `POST` | `/api/v1/subscriptions` | `subscriptions` | `create_subscription` | — |
| `GET` | `/api/v1/subscriptions/{id}` | `subscriptions` | `get_subscription` | — |
| `PATCH` | `/api/v1/subscriptions/{id}/cancel` | `subscriptions` | `cancel_subscription` | — |
| `PATCH` | `/api/v1/subscriptions/{id}/pause` | `subscriptions` | `pause_subscription` | — |
| `PATCH` | `/api/v1/subscriptions/{id}/resume` | `subscriptions` | `resume_subscription` | — |
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
