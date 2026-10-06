# HTTP Authorization and Ownership

`stateset-http` authorizes every `/api/v1` request in three layers:

1. **Authentication** (bearer token). A token can be bound to a tenant, to an
   actor, and (for customer-scoped principals) to a customer. A token bound to
   an actor overwrites `x-actor-id`. If a client sends a different actor, the
   request gets 403.
2. **Role check** (`stateset-authz`). The actor's role must allow the route's
   *action class* on its resource type. The action class comes from the table
   below, not from a guess based on the verb.
3. **Ownership** (object level). This layer applies only to
   **customer-scoped principals**. They may read and change only records that
   belong to their customer.

## Principals

The deployment is single-tenant per database, and there are two classes of
principal:

| Class | How it is established | What it may do |
|---|---|---|
| Operator / staff | Any credential not bound to a customer | Whatever its role allows (unchanged) |
| Customer-scoped (a customer, or an agent acting for one) | `ServerBuilder::add_bearer_auth_for_customer(token, actor, customer_uuid)`, or `with_customer_principal(actor, customer_uuid)` for an actor that a gateway or another actor-bound token establishes | Only records it owns, public catalog reads, and nothing marked `operator` or `destructive` |

The customer binding comes **only** from server-side configuration. The
ownership layer reads `x-actor-id` only in two cases: when an actor-bound
credential has overwritten it, or when the operator has called
`trust_actor_headers_for_authz()`. A header sent by the client on an unbound
token can never select a principal. A non-loopback `serve()` refuses trusted
actor headers unless `with_trusted_gateway_controls()` acknowledges a gateway
that strips them. So a client-supplied header never grants operator rights on
a public bind.

When customer principals are configured, enforcement is **always on** for
them, and there is no opt-out. If principals are configured but nothing can
establish an actor (no actor-bound token, and actor headers are not trusted),
the configuration is invalid. In that case every API call fails with 500
instead of running unenforced.

## Access classes

| Access | Customer-scoped behaviour |
|---|---|
| `public` | Allowed. Catalog and reference reads: products, reviews, loyalty programs, currency rates and conversion, the OpenAPI document. |
| `operator` | **403**. This covers finance (GL, AP, AR, fixed assets, revenue recognition), inventory and warehouse operations, promotions, pricing, purchasing, manufacturing, integrations, events and audit, gift cards, and every list that cannot be filtered by customer. |
| `owned` | The record named by the path must belong to the customer. Otherwise the response is **404**. |
| `owned-list` | The list's `customer_id` query filter is forced to the customer (any client value, in any encoding, is replaced) **before** the handler counts and paginates. A page therefore never contains, and is never sized by, another customer's records. |
| `owned-create` | The body's customer field must be the customer. A different customer gets **403**, and a missing field is set to the customer. A record the body references (for example `order_id` on a return) must belong to the customer, or the response is **404**. |
| `infra` | Outside `/api/v1` (health, metrics, version), so the layer does not apply. |

Whatever the access class, the **`destructive`** action class (void, reverse,
write-off, charge, settle, refund, dispose, scrap, unapply, period
close/lock/reopen, month-end close, payment-run processing, store-credit
adjust/apply) needs the `Delete` permission level, which means the built-in
`operator` role or above. Customer-scoped principals are always refused it.

### Not-found versus forbidden

For an `owned` route, a customer asking for another customer's record gets
**404** with exactly the message the handler uses for a missing id (for
example `Order <id> not found`). That makes "exists but not yours" look the
same as "does not exist", so the API cannot be used to find out which ids
exist. For `operator` routes and destructive actions the response is **403**:
the refusal depends only on the route, not on any record, so it reveals
nothing about the data.

### How ownership is resolved

| Record | Owner |
|---|---|
| customer | itself |
| order, return, subscription, review, wishlist, store credit, invoice, warranty, loyalty account | its `customer_id` |
| cart | its `customer_id` (a guest cart has no owner, so customer principals cannot reach it) |
| payment | its `customer_id`, or failing that, its order's customer |
| shipment | its order's customer |

## Keeping the inventory complete

The table is `ROUTE_POLICIES` in `crates/stateset-http/src/route_policy.rs`.
Tests in that module fail when any of the following happens:

- a route in `src/routes/*.rs` has no row (so **new routes must be
  classified**),
- a row names a route that is not mounted,
- an OpenAPI operation has no row,
- a destructive route is not `operator`,
- this page drifts from the table.

At runtime, a matched route that has no row fails closed with 403.
Regenerate the table below with:

```sh
STATESET_BLESS_HTTP_AUTHZ_DOC=1 cargo test -p stateset-http --lib route_policy
```

## Route inventory

<!-- BEGIN GENERATED ROUTE TABLE (route_policy.rs) -->

477 routes: 30 owned, 9 owned-list, 9 owned-create, 9 public, 415 operator, 5 infra (23 destructive).

| Method | Path | Action | Access | Ownership rule |
|---|---|---|---|---|
| POST | `/api/v1/a2a/credit` | create | operator | customer principals refused (403) |
| GET | `/api/v1/a2a/credit` | list | operator | customer principals refused (403) |
| GET | `/api/v1/a2a/credit/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/a2a/credit/{id}/charge` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/a2a/credit/{id}/payment` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/a2a/credit/{id}/entries` | list | operator | customer principals refused (403) |
| POST | `/api/v1/a2a/messages` | create | operator | customer principals refused (403) |
| GET | `/api/v1/a2a/messages` | list | operator | customer principals refused (403) |
| GET | `/api/v1/a2a/messages/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/a2a/messages/{id}/acknowledge` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/a2a/messages/{id}/fail` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/ap/bills` | create | operator | customer principals refused (403) |
| GET | `/api/v1/ap/bills` | list | operator | customer principals refused (403) |
| GET | `/api/v1/ap/bills/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/ap/bills/{id}/approve` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/ap/bills/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/ap/bills/{id}/dispute` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/ap/bills/{id}/three-way-match` | list | operator | customer principals refused (403) |
| POST | `/api/v1/ap/payments` | create | operator | customer principals refused (403) |
| POST | `/api/v1/ap/payments/{id}/void` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/ap/payment-runs` | create | operator | customer principals refused (403) |
| POST | `/api/v1/ap/payment-runs/{id}/approve` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/ap/payment-runs/{id}/process` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/ap/payment-runs/{id}/cancel` | delete | operator | customer principals refused (403) |
| GET | `/api/v1/ap/aging` | list | operator | customer principals refused (403) |
| GET | `/api/v1/ar/aging` | list | operator | customer principals refused (403) |
| GET | `/api/v1/ar/aging/customers` | list | operator | customer principals refused (403) |
| GET | `/api/v1/ar/aging/customers/{customer_id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/ar/payment-applications` | create | operator | customer principals refused (403) |
| POST | `/api/v1/ar/payment-applications/{id}/unapply` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/ar/credit-memos` | create | operator | customer principals refused (403) |
| GET | `/api/v1/ar/credit-memos` | list | operator | customer principals refused (403) |
| POST | `/api/v1/ar/credit-memos/{id}/apply` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/ar/write-offs` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/ar/write-offs/{id}/reverse` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/ar/collection-activities` | create | operator | customer principals refused (403) |
| GET | `/api/v1/ar/collection-activities` | list | operator | customer principals refused (403) |
| POST | `/api/v1/ar/invoices/{invoice_id}/dunning` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/ar/dunning/due` | list | operator | customer principals refused (403) |
| GET | `/api/v1/ar/customers/{customer_id}/statement` | list | operator | customer principals refused (403) |
| POST | `/api/v1/activity-logs` | create | operator | customer principals refused (403) |
| GET | `/api/v1/activity-logs` | list | operator | customer principals refused (403) |
| GET | `/api/v1/activity-logs/{id}` | read | operator | customer principals refused (403) |
| GET | `/api/v1/activity-logs/{subject_type}/{subject_id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/backorders` | create | operator | customer principals refused (403) |
| GET | `/api/v1/backorders` | list | operator | customer principals refused (403) |
| GET | `/api/v1/backorders/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/backorders/{id}/fulfill` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/backorders/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/boms` | create | operator | customer principals refused (403) |
| GET | `/api/v1/boms` | list | operator | customer principals refused (403) |
| GET | `/api/v1/boms/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/boms/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/boms/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/boms/{id}/activate` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/boms/{id}/components` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/boms/{id}/components` | list | operator | customer principals refused (403) |
| POST | `/api/v1/carts` | create | owned-create | body `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/carts` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/carts/{id}` | read | owned | `{id}` is a cart the customer owns |
| POST | `/api/v1/carts/{id}/items` | execute | owned | `{id}` is a cart the customer owns |
| PUT | `/api/v1/carts/{id}/items/{item_id}` | update | owned | `{id}` is a cart the customer owns |
| DELETE | `/api/v1/carts/{id}/items/{item_id}` | delete | owned | `{id}` is a cart the customer owns |
| POST | `/api/v1/carts/{id}/shipping` | execute | owned | `{id}` is a cart the customer owns |
| POST | `/api/v1/carts/{id}/payment` | execute | owned | `{id}` is a cart the customer owns |
| POST | `/api/v1/carts/{id}/complete` | execute | owned | `{id}` is a cart the customer owns |
| POST | `/api/v1/carts/{id}/cancel` | delete | owned | `{id}` is a cart the customer owns |
| POST | `/api/v1/channels` | create | operator | customer principals refused (403) |
| GET | `/api/v1/channels` | list | operator | customer principals refused (403) |
| GET | `/api/v1/channels/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/channels/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/channels/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/channels/{id}/lock` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/companies` | create | operator | customer principals refused (403) |
| GET | `/api/v1/companies` | list | operator | customer principals refused (403) |
| GET | `/api/v1/companies/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/companies/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/companies/{id}/contacts` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/companies/{id}/contacts` | list | operator | customer principals refused (403) |
| GET | `/api/v1/currencies/rates` | list | public | any authenticated principal |
| POST | `/api/v1/currencies/rates` | create | operator | customer principals refused (403) |
| POST | `/api/v1/currencies/convert` | execute | public | any authenticated principal |
| POST | `/api/v1/customers` | create | operator | customer principals refused (403) |
| GET | `/api/v1/customers` | list | operator | customer principals refused (403) |
| GET | `/api/v1/customers/{id}` | read | owned | `{id}` is a customer the customer owns |
| PATCH | `/api/v1/customers/{id}` | update | owned | `{id}` is a customer the customer owns |
| DELETE | `/api/v1/customers/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/edi-documents` | create | operator | customer principals refused (403) |
| GET | `/api/v1/edi-documents` | list | operator | customer principals refused (403) |
| GET | `/api/v1/edi-documents/summary` | list | operator | customer principals refused (403) |
| GET | `/api/v1/edi-documents/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/edi-documents/{id}/status` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/events/stream` | list | operator | customer principals refused (403) |
| POST | `/api/v1/fixed-assets` | create | operator | customer principals refused (403) |
| GET | `/api/v1/fixed-assets` | list | operator | customer principals refused (403) |
| GET | `/api/v1/fixed-assets/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/fixed-assets/{id}` | update | operator | customer principals refused (403) |
| POST | `/api/v1/fixed-assets/{id}/place-in-service` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/fixed-assets/{id}/dispose` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/fixed-assets/{id}/write-off` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/fixed-assets/{id}/schedule` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/fixed-assets/{id}/schedule` | list | operator | customer principals refused (403) |
| POST | `/api/v1/fixed-assets/{id}/post-depreciation` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/waves` | create | operator | customer principals refused (403) |
| GET | `/api/v1/fulfillment/waves` | list | operator | customer principals refused (403) |
| GET | `/api/v1/fulfillment/waves/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/waves/{id}/release` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/fulfillment/picks` | list | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/picks/{id}/assign` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/picks/{id}/complete` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/fulfillment/packs` | list | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/packs/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/packs/{id}/cartons` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/fulfillment/packs/{id}/cartons` | list | operator | customer principals refused (403) |
| GET | `/api/v1/fulfillment/ships` | list | operator | customer principals refused (403) |
| POST | `/api/v1/fulfillment/ships/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/gl/accounts` | create | operator | customer principals refused (403) |
| GET | `/api/v1/gl/accounts` | list | operator | customer principals refused (403) |
| GET | `/api/v1/gl/accounts/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/gl/journal-entries` | create | operator | customer principals refused (403) |
| GET | `/api/v1/gl/journal-entries` | list | operator | customer principals refused (403) |
| GET | `/api/v1/gl/journal-entries/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/gl/journal-entries/{id}/post` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/gl/journal-entries/{id}/void` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gl/journal-entries/{id}/reverse` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gl/revalue` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/gl/close-month` | destructive | operator | customer principals refused (403) |
| GET | `/api/v1/gl/trial-balance` | list | operator | customer principals refused (403) |
| GET | `/api/v1/gl/balance-sheet` | list | operator | customer principals refused (403) |
| GET | `/api/v1/gl/income-statement` | list | operator | customer principals refused (403) |
| POST | `/api/v1/gl/periods` | create | operator | customer principals refused (403) |
| GET | `/api/v1/gl/periods` | list | operator | customer principals refused (403) |
| POST | `/api/v1/gl/periods/{id}/open` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/gl/periods/{id}/close` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gl/periods/{id}/lock` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gl/periods/{id}/reopen` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gift-cards` | create | operator | customer principals refused (403) |
| GET | `/api/v1/gift-cards` | list | operator | customer principals refused (403) |
| GET | `/api/v1/gift-cards/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/gift-cards/{id}/charge` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gift-cards/{id}/refund` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/gift-cards/{id}/disable` | delete | operator | customer principals refused (403) |
| GET | `/health` | read | infra | outside `/api/v1` |
| GET | `/health/ready` | read | infra | outside `/api/v1` |
| GET | `/health/deep` | read | infra | outside `/api/v1` |
| GET | `/metrics` | read | infra | outside `/api/v1` |
| GET | `/version` | read | infra | outside `/api/v1` |
| POST | `/api/v1/inbound-shipments` | create | operator | customer principals refused (403) |
| GET | `/api/v1/inbound-shipments` | list | operator | customer principals refused (403) |
| GET | `/api/v1/inbound-shipments/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/inbound-shipments/{id}/in-transit` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/inbound-shipments/{id}/arrived` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/inbound-shipments/{id}/receive` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/inbound-shipments/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/integration-field-mappings` | create | operator | customer principals refused (403) |
| GET | `/api/v1/integration-field-mappings` | list | operator | customer principals refused (403) |
| POST | `/api/v1/integration-field-mappings/bulk` | create | operator | customer principals refused (403) |
| DELETE | `/api/v1/integration-field-mappings/bulk` | delete | operator | customer principals refused (403) |
| GET | `/api/v1/integration-field-mappings/groups` | list | operator | customer principals refused (403) |
| GET | `/api/v1/integration-field-mappings/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/integration-field-mappings/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/integration-field-mappings/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/integration-mappings` | create | operator | customer principals refused (403) |
| GET | `/api/v1/integration-mappings` | list | operator | customer principals refused (403) |
| POST | `/api/v1/integration-mappings/bulk` | create | operator | customer principals refused (403) |
| GET | `/api/v1/integration-mappings/resolve` | list | operator | customer principals refused (403) |
| GET | `/api/v1/integration-mappings/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/integration-mappings/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/integration-mappings/{id}` | delete | operator | customer principals refused (403) |
| GET | `/api/v1/inventory` | list | operator | customer principals refused (403) |
| GET | `/api/v1/inventory/reservations` | list | operator | customer principals refused (403) |
| POST | `/api/v1/inventory/reservations/expire` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/inventory/reservations/{reservation_id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/inventory/reservations/{reservation_id}/release` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/inventory/reservations/{reservation_id}/confirm` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/inventory/sweeps/run` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/inventory/{sku}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/inventory/{sku}/adjust` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/inventory/{sku}/reservations` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/invoices` | create | operator | customer principals refused (403) |
| GET | `/api/v1/invoices` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/invoices/{id}` | read | owned | `{id}` is a invoice the customer owns |
| POST | `/api/v1/invoices/{id}/send` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/invoices/{id}/payments` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/kernel/audit` | list | operator | customer principals refused (403) |
| GET | `/api/v1/kernel/audit/checkpoint` | list | operator | customer principals refused (403) |
| POST | `/api/v1/lots` | create | operator | customer principals refused (403) |
| GET | `/api/v1/lots` | list | operator | customer principals refused (403) |
| GET | `/api/v1/lots/expiring` | list | operator | customer principals refused (403) |
| GET | `/api/v1/lots/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/lots/{id}/consume` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/lots/{id}/reserve` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/lots/reservations/{reservation_id}/release` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/lots/{id}/quarantine` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/lots/{id}/release-quarantine` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/lots/{id}/genealogy` | list | operator | customer principals refused (403) |
| POST | `/api/v1/loyalty/programs` | create | operator | customer principals refused (403) |
| GET | `/api/v1/loyalty/programs` | list | public | any authenticated principal |
| POST | `/api/v1/loyalty/enroll` | execute | owned-create | body `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/loyalty/accounts/{id}` | read | owned | `{id}` is a loyalty account the customer owns |
| POST | `/api/v1/negotiations` | create | operator | customer principals refused (403) |
| GET | `/api/v1/negotiations/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/negotiations/{id}/counter-offer` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/negotiations/{id}/accept` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/negotiations/{id}/reject` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/orders` | create | owned-create | body `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/orders` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/orders/{id}` | read | owned | `{id}` is a order the customer owns |
| PATCH | `/api/v1/orders/{id}/cancel` | delete | owned | `{id}` is a order the customer owns |
| PATCH | `/api/v1/orders/{id}/ship` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/payment-obligations` | create | operator | customer principals refused (403) |
| GET | `/api/v1/payment-obligations` | list | operator | customer principals refused (403) |
| GET | `/api/v1/payment-obligations/dashboard` | list | operator | customer principals refused (403) |
| GET | `/api/v1/payment-obligations/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/payment-obligations/{id}/payments` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/payment-obligations/{id}/status` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/payment-obligations/{id}/bills` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/payments` | create | owned-create | body `order_id` must be a order the customer owns; `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/payments` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/payments/{id}` | read | owned | `{id}` is a payment the customer owns |
| POST | `/api/v1/payments/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/payments/{id}/refund` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/prepayments` | create | operator | customer principals refused (403) |
| GET | `/api/v1/prepayments` | list | operator | customer principals refused (403) |
| GET | `/api/v1/prepayments/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/prepayments/{id}/apply` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/prepayments/{id}/refund` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/price-levels` | create | operator | customer principals refused (403) |
| GET | `/api/v1/price-levels` | list | operator | customer principals refused (403) |
| GET | `/api/v1/price-levels/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/price-levels/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/price-levels/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/price-levels/{id}/entries` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/price-levels/{id}/entries` | list | operator | customer principals refused (403) |
| POST | `/api/v1/price-schedules` | create | operator | customer principals refused (403) |
| GET | `/api/v1/price-schedules` | list | operator | customer principals refused (403) |
| GET | `/api/v1/price-schedules/resolve` | list | operator | customer principals refused (403) |
| GET | `/api/v1/price-schedules/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/price-schedules/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/price-schedules/{id}/entries` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/price-schedules/{id}/entries` | list | operator | customer principals refused (403) |
| POST | `/api/v1/print-stations` | create | operator | customer principals refused (403) |
| GET | `/api/v1/print-stations` | list | operator | customer principals refused (403) |
| POST | `/api/v1/print-stations/{id}/revoke` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/print-stations/{id}/jobs` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/print-stations/{id}/jobs` | list | operator | customer principals refused (403) |
| POST | `/api/v1/print-stations/{id}/jobs/next` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/print-jobs/{job_id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/production-batches` | create | operator | customer principals refused (403) |
| GET | `/api/v1/production-batches` | list | operator | customer principals refused (403) |
| GET | `/api/v1/production-batches/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/production-batches/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/production-batches/{id}/work-orders` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/products` | create | operator | customer principals refused (403) |
| GET | `/api/v1/products` | list | public | any authenticated principal |
| GET | `/api/v1/products/{id}` | read | public | any authenticated principal |
| PATCH | `/api/v1/products/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/products/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/promotions` | create | operator | customer principals refused (403) |
| GET | `/api/v1/promotions` | list | operator | customer principals refused (403) |
| GET | `/api/v1/promotions/{id}` | read | operator | customer principals refused (403) |
| PATCH | `/api/v1/promotions/{id}/activate` | execute | operator | customer principals refused (403) |
| PATCH | `/api/v1/promotions/{id}/deactivate` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders` | create | operator | customer principals refused (403) |
| GET | `/api/v1/purchase-orders` | list | operator | customer principals refused (403) |
| GET | `/api/v1/purchase-orders/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/purchase-orders/{id}` | update | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/submit` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/approve` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/send` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/acknowledge` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/hold` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/purchase-orders/{id}/receive` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/purchase-orders/{id}/items` | list | operator | customer principals refused (403) |
| POST | `/api/v1/suppliers` | create | operator | customer principals refused (403) |
| GET | `/api/v1/suppliers` | list | operator | customer principals refused (403) |
| GET | `/api/v1/suppliers/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/suppliers/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/suppliers/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/purgatory/orders` | create | operator | customer principals refused (403) |
| GET | `/api/v1/purgatory/orders` | list | operator | customer principals refused (403) |
| GET | `/api/v1/purgatory/orders/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/purgatory/orders/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/purgatory/orders/{id}/post` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/purgatory/orders/{id}/lines/{line_id}` | create | operator | customer principals refused (403) |
| POST | `/api/v1/quality/inspections` | create | operator | customer principals refused (403) |
| GET | `/api/v1/quality/inspections` | list | operator | customer principals refused (403) |
| GET | `/api/v1/quality/inspections/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/quality/inspections/{id}/start` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/quality/inspections/{id}/results` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/quality/inspections/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/quality/ncrs` | create | operator | customer principals refused (403) |
| GET | `/api/v1/quality/ncrs` | list | operator | customer principals refused (403) |
| GET | `/api/v1/quality/ncrs/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/quality/ncrs/{id}/disposition` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/quality/ncrs/{id}/close` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/quality/holds` | create | operator | customer principals refused (403) |
| GET | `/api/v1/quality/holds` | list | operator | customer principals refused (403) |
| GET | `/api/v1/quality/holds/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/quality/holds/{id}/release` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/receipts` | create | operator | customer principals refused (403) |
| GET | `/api/v1/receipts` | list | operator | customer principals refused (403) |
| GET | `/api/v1/receipts/{id}` | read | operator | customer principals refused (403) |
| GET | `/api/v1/receipts/{id}/items` | list | operator | customer principals refused (403) |
| POST | `/api/v1/receipts/{id}/start` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/receipts/{id}/receive` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/receipts/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/receipts/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/put-aways` | create | operator | customer principals refused (403) |
| GET | `/api/v1/put-aways` | list | operator | customer principals refused (403) |
| GET | `/api/v1/put-aways/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/put-aways/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/reports/inventory-aging` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/reports/sales-by-channel` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/reports/transaction-cogs` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/reports/close-the-books` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/reports/consumption` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/returns` | create | owned-create | body `order_id` must be a order the customer owns |
| GET | `/api/v1/returns` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/returns/{id}` | read | owned | `{id}` is a return the customer owns |
| PATCH | `/api/v1/returns/{id}` | update | operator | customer principals refused (403) |
| PATCH | `/api/v1/returns/{id}/approve` | execute | operator | customer principals refused (403) |
| PATCH | `/api/v1/returns/{id}/reject` | execute | operator | customer principals refused (403) |
| PATCH | `/api/v1/returns/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/returns/{id}/items/{item_id}/disposition` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/revenue-contracts` | create | operator | customer principals refused (403) |
| GET | `/api/v1/revenue-contracts` | list | operator | customer principals refused (403) |
| GET | `/api/v1/revenue-contracts/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/revenue-contracts/{id}` | update | operator | customer principals refused (403) |
| GET | `/api/v1/revenue-contracts/{id}/obligations` | list | operator | customer principals refused (403) |
| POST | `/api/v1/revenue-obligations/{id}/schedule` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/revenue-obligations/{id}/schedule` | list | operator | customer principals refused (403) |
| POST | `/api/v1/revenue-obligations/{id}/recognize` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/reviews` | create | owned-create | body `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/reviews` | list | public | any authenticated principal |
| GET | `/api/v1/reviews/{id}` | read | public | any authenticated principal |
| DELETE | `/api/v1/reviews/{id}` | delete | owned | `{id}` is a review the customer owns |
| POST | `/api/v1/segments` | create | operator | customer principals refused (403) |
| GET | `/api/v1/segments` | list | operator | customer principals refused (403) |
| GET | `/api/v1/segments/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/segments/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/segments/{id}/members/{customer_id}` | create | operator | customer principals refused (403) |
| DELETE | `/api/v1/segments/{id}/members/{customer_id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/serials` | create | operator | customer principals refused (403) |
| GET | `/api/v1/serials` | list | operator | customer principals refused (403) |
| GET | `/api/v1/serials/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/serials/{id}/reserve` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/serials/reservations/{reservation_id}/release` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/serials/{id}/ship` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/serials/{id}/return` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/serials/{id}/scrap` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/shipments` | create | operator | customer principals refused (403) |
| GET | `/api/v1/shipments` | list | operator | customer principals refused (403) |
| GET | `/api/v1/shipments/{id}` | read | owned | `{id}` is a shipment the customer owns |
| PATCH | `/api/v1/shipments/{id}` | update | operator | customer principals refused (403) |
| POST | `/api/v1/shipments/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/shipments/{id}/deliver` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/shipments/{id}/items` | execute | operator | customer principals refused (403) |
| DELETE | `/api/v1/shipments/{id}/items/{item_id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/shipping-zones` | create | operator | customer principals refused (403) |
| GET | `/api/v1/shipping-zones` | list | operator | customer principals refused (403) |
| GET | `/api/v1/shipping-zones/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/shipping-zones/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/stock-snapshots` | create | operator | customer principals refused (403) |
| GET | `/api/v1/stock-snapshots` | list | operator | customer principals refused (403) |
| GET | `/api/v1/stock-snapshots/latest` | list | operator | customer principals refused (403) |
| GET | `/api/v1/stock-snapshots/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/stock-snapshots/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/store-credits` | create | operator | customer principals refused (403) |
| GET | `/api/v1/store-credits` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/store-credits/{id}` | read | owned | `{id}` is a store credit the customer owns |
| POST | `/api/v1/store-credits/{id}/adjust` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/store-credits/{id}/apply` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/subscriptions` | create | owned-create | body `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/subscriptions` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/subscriptions/{id}` | read | owned | `{id}` is a subscription the customer owns |
| PATCH | `/api/v1/subscriptions/{id}/pause` | execute | owned | `{id}` is a subscription the customer owns |
| PATCH | `/api/v1/subscriptions/{id}/resume` | execute | owned | `{id}` is a subscription the customer owns |
| PATCH | `/api/v1/subscriptions/{id}/cancel` | delete | owned | `{id}` is a subscription the customer owns |
| POST | `/api/v1/supplier-skus` | create | operator | customer principals refused (403) |
| GET | `/api/v1/supplier-skus` | list | operator | customer principals refused (403) |
| GET | `/api/v1/supplier-skus/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/supplier-skus/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/tax/exemptions` | create | operator | customer principals refused (403) |
| GET | `/api/v1/tax/exemptions` | list | operator | customer principals refused (403) |
| GET | `/api/v1/tax/exemptions/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/tax/exemptions/{id}/verify` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/topology-snapshots` | create | operator | customer principals refused (403) |
| GET | `/api/v1/topology-snapshots` | list | operator | customer principals refused (403) |
| GET | `/api/v1/topology-snapshots/latest` | list | operator | customer principals refused (403) |
| GET | `/api/v1/topology-snapshots/{id}` | read | operator | customer principals refused (403) |
| DELETE | `/api/v1/topology-snapshots/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/transfer-orders` | create | operator | customer principals refused (403) |
| GET | `/api/v1/transfer-orders` | list | operator | customer principals refused (403) |
| GET | `/api/v1/transfer-orders/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/transfer-orders/{id}/ship` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/transfer-orders/{id}/receive` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/transfer-orders/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/unit-classes` | create | operator | customer principals refused (403) |
| GET | `/api/v1/unit-classes` | list | operator | customer principals refused (403) |
| POST | `/api/v1/units-of-measure` | create | operator | customer principals refused (403) |
| GET | `/api/v1/units-of-measure` | list | operator | customer principals refused (403) |
| POST | `/api/v1/unit-conversion-rules` | create | operator | customer principals refused (403) |
| GET | `/api/v1/unit-conversion-rules` | list | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-credits` | create | operator | customer principals refused (403) |
| GET | `/api/v1/vendor-credits` | list | operator | customer principals refused (403) |
| GET | `/api/v1/vendor-credits/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-credits/{id}/apply` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-credits/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-returns` | create | operator | customer principals refused (403) |
| GET | `/api/v1/vendor-returns` | list | operator | customer principals refused (403) |
| GET | `/api/v1/vendor-returns/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-returns/{id}/submit` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-returns/{id}/process` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/vendor-returns/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/warehouses` | create | operator | customer principals refused (403) |
| GET | `/api/v1/warehouses` | list | operator | customer principals refused (403) |
| GET | `/api/v1/warehouses/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/warehouses/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/warehouses/{id}` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/warehouse-locations` | create | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-locations` | list | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-locations/{id}` | read | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-locations/{id}/inventory` | list | operator | customer principals refused (403) |
| POST | `/api/v1/warehouse-inventory/adjust` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/warehouse-inventory/move` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/warehouse-bins` | create | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-bins` | list | operator | customer principals refused (403) |
| POST | `/api/v1/warehouse-bins/adjust` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/warehouse-bins/move` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-bins/reconcile` | list | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-bins/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/warehouse-bins/{id}` | update | operator | customer principals refused (403) |
| DELETE | `/api/v1/warehouse-bins/{id}` | delete | operator | customer principals refused (403) |
| GET | `/api/v1/warehouse-bins/{id}/levels` | list | operator | customer principals refused (403) |
| POST | `/api/v1/cycle-counts` | create | operator | customer principals refused (403) |
| GET | `/api/v1/cycle-counts` | list | operator | customer principals refused (403) |
| GET | `/api/v1/cycle-counts/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/cycle-counts/{id}/start` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/cycle-counts/{id}/counts` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/cycle-counts/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/cycle-counts/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/warranties` | create | owned-create | body `order_id` must be a order the customer owns |
| GET | `/api/v1/warranties` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/warranties/{id}` | read | owned | `{id}` is a warranty the customer owns |
| POST | `/api/v1/wishlists` | create | owned-create | body `customer_id` must be (or is set to) the customer |
| GET | `/api/v1/wishlists` | list | owned-list | `customer_id` forced to the customer |
| GET | `/api/v1/wishlists/{id}` | read | owned | `{id}` is a wishlist the customer owns |
| DELETE | `/api/v1/wishlists/{id}` | delete | owned | `{id}` is a wishlist the customer owns |
| POST | `/api/v1/wishlists/{id}/items` | execute | owned | `{id}` is a wishlist the customer owns |
| DELETE | `/api/v1/wishlists/{id}/items/{product_id}` | delete | owned | `{id}` is a wishlist the customer owns |
| POST | `/api/v1/work-orders` | create | operator | customer principals refused (403) |
| GET | `/api/v1/work-orders` | list | operator | customer principals refused (403) |
| GET | `/api/v1/work-orders/{id}` | read | operator | customer principals refused (403) |
| PUT | `/api/v1/work-orders/{id}` | update | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/{id}/start` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/{id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/{id}/hold` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/{id}/resume` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/{id}/cancel` | delete | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/{id}/tasks` | execute | operator | customer principals refused (403) |
| GET | `/api/v1/work-orders/{id}/tasks` | list | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/tasks/{task_id}/start` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/work-orders/tasks/{task_id}/complete` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/x402/intents` | create | operator | customer principals refused (403) |
| GET | `/api/v1/x402/intents` | list | operator | customer principals refused (403) |
| GET | `/api/v1/x402/intents/{id}` | read | operator | customer principals refused (403) |
| POST | `/api/v1/x402/intents/{id}/sign` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/x402/intents/{id}/settle` | destructive | operator | customer principals refused (403) |
| POST | `/api/v1/x402/intents/{id}/fail` | execute | operator | customer principals refused (403) |
| POST | `/api/v1/x402/intents/{id}/cancel` | delete | operator | customer principals refused (403) |
| GET | `/api/v1/x402/carts/{cart_id}/intents` | list | owned | `{cart_id}` is a cart the customer owns |
| GET | `/api/v1/x402/orders/{order_id}/intents` | list | owned | `{order_id}` is a order the customer owns |
| GET | `/api/v1/openapi.json` | list | public | any authenticated principal |
| GET | `/api/v1/docs` | list | public | any authenticated principal |

<!-- END GENERATED ROUTE TABLE -->
