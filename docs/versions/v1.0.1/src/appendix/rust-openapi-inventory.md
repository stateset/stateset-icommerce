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
| API version | `1.0.1` |
| Paths | 40 |
| Operations | 57 |
| Schemas | 54 |
| Tags | 13 |

## Method Counts

| Method | Operations |
| --- | --- |
| DELETE | 5 |
| GET | 27 |
| PATCH | 5 |
| POST | 20 |

## Tag Counts

| Tag | Operations | Description |
| --- | --- | --- |
| `customers` | 5 | Customer management |
| `gift_cards` | 4 | Gift card management |
| `health` | 3 | Health check endpoints |
| `inventory` | 3 | Stock and inventory management |
| `invoices` | 5 | Invoice management |
| `loyalty` | 4 | Loyalty program management |
| `orders` | 5 | Order lifecycle management |
| `payments` | 5 | Payment transaction management |
| `products` | 5 | Product catalog |
| `returns` | 4 | Return request processing |
| `reviews` | 4 | Product review management |
| `shipments` | 4 | Shipment tracking and management |
| `wishlists` | 6 | Customer wishlist management |

## Operations

| Method | Path | Tags | Operation ID | Summary |
| --- | --- | --- | --- | --- |
| `GET` | `/api/v1/customers` | `customers` | `list_customers` | `GET /api/v1/customers` |
| `POST` | `/api/v1/customers` | `customers` | `create_customer` | `POST /api/v1/customers` |
| `DELETE` | `/api/v1/customers/{id}` | `customers` | `delete_customer` | `DELETE /api/v1/customers/:id` |
| `GET` | `/api/v1/customers/{id}` | `customers` | `get_customer` | `GET /api/v1/customers/:id` |
| `PATCH` | `/api/v1/customers/{id}` | `customers` | `update_customer` | `PATCH /api/v1/customers/:id` |
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
| `GET` | `/api/v1/returns` | `returns` | `list_returns` | `GET /api/v1/returns` |
| `POST` | `/api/v1/returns` | `returns` | `create_return` | `POST /api/v1/returns` |
| `GET` | `/api/v1/returns/{id}` | `returns` | `get_return` | `GET /api/v1/returns/:id` |
| `PATCH` | `/api/v1/returns/{id}/approve` | `returns` | `approve_return` | `PATCH /api/v1/returns/:id/approve` |
| `GET` | `/api/v1/reviews` | `reviews` | `list_reviews` | `GET /api/v1/reviews` |
| `POST` | `/api/v1/reviews` | `reviews` | `create_review` | `POST /api/v1/reviews` |
| `DELETE` | `/api/v1/reviews/{id}` | `reviews` | `delete_review` | `DELETE /api/v1/reviews/{id}` |
| `GET` | `/api/v1/reviews/{id}` | `reviews` | `get_review` | `GET /api/v1/reviews/{id}` |
| `GET` | `/api/v1/shipments` | `shipments` | `list_shipments` | `GET /api/v1/shipments` |
| `POST` | `/api/v1/shipments` | `shipments` | `create_shipment` | `POST /api/v1/shipments` |
| `GET` | `/api/v1/shipments/{id}` | `shipments` | `get_shipment` | `GET /api/v1/shipments/:id` |
| `POST` | `/api/v1/shipments/{id}/deliver` | `shipments` | `deliver_shipment` | `POST /api/v1/shipments/:id/deliver` |
| `GET` | `/api/v1/wishlists` | `wishlists` | `list_wishlists` | `GET /api/v1/wishlists` |
| `POST` | `/api/v1/wishlists` | `wishlists` | `create_wishlist` | `POST /api/v1/wishlists` |
| `DELETE` | `/api/v1/wishlists/{id}` | `wishlists` | `delete_wishlist` | `DELETE /api/v1/wishlists/{id}` |
| `GET` | `/api/v1/wishlists/{id}` | `wishlists` | `get_wishlist` | `GET /api/v1/wishlists/{id}` |
| `POST` | `/api/v1/wishlists/{id}/items` | `wishlists` | `add_item` | `POST /api/v1/wishlists/{id}/items` |
| `DELETE` | `/api/v1/wishlists/{id}/items/{product_id}` | `wishlists` | `remove_item` | `DELETE /api/v1/wishlists/{id}/items/{product_id}` |
| `GET` | `/health` | `health` | `health` | `GET /health` — simple liveness probe. |
| `GET` | `/health/ready` | `health` | `readiness` | `GET /health/ready` — readiness probe that checks DB connectivity. |
| `GET` | `/metrics` | `health` | `metrics` | `GET /metrics` — Prometheus-compatible operational metrics. |
