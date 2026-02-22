# Commerce Concepts

## Core Entities

### Orders
An order is a confirmed request to purchase one or more products. Orders have a lifecycle from
creation through fulfillment to delivery. Each order has a unique ID (e.g. ORD-12345) and
tracks line items, quantities, pricing, shipping address, and payment.

### Carts
A cart is a temporary collection of items a customer intends to purchase. Carts precede orders
in the checkout flow. A cart becomes an order when `complete_checkout` is called.

### Checkout
The checkout process converts a cart into a confirmed order. Steps: add items, set shipping
address, apply discounts, calculate tax, select shipping rate, capture payment.

### Fulfillment
Fulfillment is the process of picking, packing, and shipping ordered items to the customer.
A fulfillment may cover the whole order or a subset (partial fulfillment).

## Key Metrics & Terminology

- **SKU** (Stock Keeping Unit) — unique identifier for a distinct product variant
- **AOV** (Average Order Value) — total revenue divided by number of orders in a period
- **LTV** (Lifetime Value) — total revenue attributed to a customer over their relationship
- **Churn Rate** — percentage of subscribers who cancel in a given period
- **Conversion Rate** — percentage of carts or sessions that result in a completed order
- **GMV** (Gross Merchandise Value) — total sales volume before fees/refunds
- **RMA** (Return Merchandise Authorization) — authorization number for a return
- **COGS** (Cost of Goods Sold) — direct cost of producing the items sold

## Common Workflows

### Browse → Cart → Checkout → Order → Fulfillment → Delivery
1. Customer browses products (`list_products`, `get_product`)
2. Customer adds items to cart (`create_cart`, `add_cart_item`)
3. Customer applies discount codes (`apply_cart_discount`)
4. Shipping address set, tax calculated (`set_cart_shipping_address`, `calculate_cart_tax`)
5. Payment method set (`set_cart_payment`)
6. Checkout completed, order created (`complete_checkout`)
7. Warehouse picks and packs items, shipment created (`create_shipment`)
8. Carrier scans package; tracking number provided
9. Package delivered (`deliver_shipment`)

### Subscription Commerce
Recurring billing plans allow customers to subscribe to products or services. Billing cycles
run automatically. Trials, pauses, and cancellations are managed through the subscriptions API.

### B2B Commerce
Business buyers use purchase orders and invoices. Suppliers receive POs, fulfill them, and
merchants record invoice payments. Credit terms (net-30, net-60) are common.
