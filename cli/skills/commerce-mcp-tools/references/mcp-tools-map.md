# MCP Tool Map

## Table of Contents

- [StateSet Commerce MCP Tools](#stateset-commerce-mcp-tools)
  - [Customer Tools](#customer-tools)
  - [Order Tools](#order-tools)
  - [Product Tools](#product-tools)
  - [Inventory Tools](#inventory-tools)
  - [Returns Tools](#returns-tools)
  - [Cart/Checkout Tools (Agentic Commerce Protocol)](#cart-checkout-tools-agentic-commerce-protocol)
  - [Analytics & Forecasting Tools](#analytics-forecasting-tools)
  - [Currency & Exchange Rate Tools](#currency-exchange-rate-tools)
  - [Tax Calculation Tools](#tax-calculation-tools)
  - [Promotions & Discounts Tools](#promotions-discounts-tools)
  - [Subscription Tools](#subscription-tools)
  - [Sync Tools (Verifiable Event Sync)](#sync-tools-verifiable-event-sync)
  - [Sync Conflict Resolution Tools](#sync-conflict-resolution-tools)
  - [Manufacturing Tools - BOM & Work Orders](#manufacturing-tools-bom-work-orders)
  - [Payment Tools](#payment-tools)
  - [Stablecoin Payment Tools (Native Crypto Payments)](#stablecoin-payment-tools-native-crypto-payments)
  - [Shipment Tools](#shipment-tools)
  - [Supplier & Purchase Order Tools](#supplier-purchase-order-tools)
  - [Invoice Tools](#invoice-tools)
  - [Warranty Tools](#warranty-tools)
- [Autonomous Engine MCP Tools](#autonomous-engine-mcp-tools)
  - [Scheduler Tools](#scheduler-tools)
  - [Workflow Tools](#workflow-tools)
  - [Policy Tools](#policy-tools)
  - [Approval Tools](#approval-tools)
  - [Webhook Tools](#webhook-tools)
  - [Engine-Level Tools](#engine-level-tools)

## StateSet Commerce MCP Tools

### Customer Tools

#### list_customers

List all customers in the database. Returns customer details including email, name, and status.

Params: none

Example:
```json
{}
```

#### get_customer

Get a specific customer by ID or email address.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| identifier | string | yes | Customer ID (UUID) or email address |

Example:
```json
{
  "identifier": "<string>"
}
```

#### create_customer

Create a new customer. Requires email, first name, and last name.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| email | string | yes | Customer email address |
| firstName | string | yes | Customer first name |
| lastName | string | yes | Customer last name |
| phone | string | no | Customer phone number |
| acceptsMarketing | boolean | no | Whether customer accepts marketing |

Example:
```json
{
  "email": "name@example.com",
  "firstName": "<string>",
  "lastName": "<string>"
}
```

### Order Tools

#### list_orders

List all orders. Shows order number, status, customer, total amount, and item count.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| limit | number | no | Maximum number of orders to return |

Example:
```json
{}
```

#### get_order

Get a specific order by ID or order number. Returns full order details including line items.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| identifier | string | yes | Order ID (UUID) or order number |

Example:
```json
{
  "identifier": "<string>"
}
```

#### create_order

Create a new order for a customer with line items.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | yes | Customer ID (UUID) |
| items | string | yes | Product SKU |
| currency | string | no | Currency code |
| notes | string | no | Order notes |

Example:
```json
{
  "customerId": "<uuid>",
  "items": "<string>"
}
```

#### update_order_status

Update the status of an order. Valid statuses: pending, confirmed, processing, shipped, delivered, cancelled, refunded.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| orderId | string | yes | Order ID (UUID) |
| status | enum (pending, confirmed, processing, shipped, delivered, cancelled, refunded) | yes | New order status |

Example:
```json
{
  "orderId": "<uuid>",
  "status": "pending"
}
```

#### ship_order

Mark an order as shipped with optional tracking number.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| orderId | string | yes | Order ID (UUID) |
| trackingNumber | string | no | Shipping tracking number |

Example:
```json
{
  "orderId": "<uuid>"
}
```

#### cancel_order

Cancel an order. Only pending or confirmed orders can be cancelled.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| orderId | string | yes | Order ID (UUID) |

Example:
```json
{
  "orderId": "<uuid>"
}
```

### Product Tools

#### list_products

List all products in the catalog.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| limit | number | no | Maximum number of products to return |

Example:
```json
{}
```

#### get_product

Get a specific product by ID.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| productId | string | yes | Product ID (UUID) |

Example:
```json
{
  "productId": "<uuid>"
}
```

#### get_product_variant

Get a product variant by SKU.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| sku | string | yes | Product variant SKU |

Example:
```json
{
  "sku": "SKU-001"
}
```

#### create_product

Create a new product with optional variants.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| name | string | yes | Product name |
| description | string | no | Product description |
| variants | string | no | Variant SKU |

Example:
```json
{
  "name": "<string>"
}
```

### Inventory Tools

#### get_stock

Get current stock level for a SKU. Shows on-hand, allocated, and available quantities.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| sku | string | yes | Product SKU |

Example:
```json
{
  "sku": "SKU-001"
}
```

#### create_inventory_item

Create a new inventory item for a SKU.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| sku | string | yes | Product SKU |
| name | string | yes | Item name |
| description | string | no | Item description |
| initialQuantity | number | no | Initial stock quantity |
| reorderPoint | number | no | Reorder point threshold |

Example:
```json
{
  "sku": "SKU-001",
  "name": "<string>"
}
```

#### adjust_inventory

Adjust inventory quantity for a SKU. Use positive numbers to add stock, negative to remove.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| sku | string | yes | Product SKU |
| quantity | number | yes | Quantity adjustment (positive to add, negative to subtract) |
| reason | string | yes | Reason for adjustment (e.g., "Received shipment", "Damaged goods") |

Example:
```json
{
  "sku": "SKU-001",
  "quantity": 1,
  "reason": "<string>"
}
```

#### reserve_inventory

Reserve inventory for an order. Reserved stock is allocated but not yet deducted.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| sku | string | yes | Product SKU |
| quantity | number | yes | Quantity to reserve |
| referenceType | string | yes | Reference type (e.g., "order", "transfer") |
| referenceId | string | yes | Reference ID (e.g., order ID) |
| expiresInSeconds | number | no | Reservation expiry in seconds |

Example:
```json
{
  "sku": "SKU-001",
  "quantity": 1,
  "referenceType": "<string>",
  "referenceId": "<uuid>"
}
```

#### confirm_reservation

Confirm an inventory reservation, deducting the reserved quantity from stock.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| reservationId | string | yes | Reservation ID |

Example:
```json
{
  "reservationId": "<uuid>"
}
```

#### release_reservation

Release an inventory reservation, returning the reserved quantity to available stock.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| reservationId | string | yes | Reservation ID |

Example:
```json
{
  "reservationId": "<uuid>"
}
```

### Returns Tools

#### list_returns

List all returns. Shows return status, order, and reason.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| limit | number | no | Maximum number of returns to show |

Example:
```json
{}
```

#### get_return

Get a specific return by ID.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| returnId | string | yes | Return ID (UUID) |

Example:
```json
{
  "returnId": "<uuid>"
}
```

#### create_return

Create a return request for an order.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| orderId | string | yes | Order ID (UUID) |
| reason | enum (defective, wrong_item, not_as_described, changed_mind, better_price_found, no_longer_needed, damaged, other) | yes | Return reason |
| reasonDetails | string | no | Additional details about the return reason |
| items | string | yes | Order item ID to return |

Example:
```json
{
  "orderId": "<uuid>",
  "reason": "defective",
  "items": "<string>"
}
```

#### approve_return

Approve a return request.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| returnId | string | yes | Return ID (UUID) |

Example:
```json
{
  "returnId": "<uuid>"
}
```

#### reject_return

Reject a return request with a reason.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| returnId | string | yes | Return ID (UUID) |
| reason | string | yes | Reason for rejection |

Example:
```json
{
  "returnId": "<uuid>",
  "reason": "<string>"
}
```

### Cart/Checkout Tools (Agentic Commerce Protocol)

#### list_carts

List all shopping carts. Shows cart status, customer, totals, and item count.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| limit | number | no | Maximum number of carts to return |

Example:
```json
{}
```

#### get_cart

Get a specific cart by ID or cart number. Returns full cart details including items.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| identifier | string | yes | Cart ID (UUID) or cart number |

Example:
```json
{
  "identifier": "<string>"
}
```

#### create_cart

Create a new shopping cart. Can be for a guest or authenticated customer.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | no | Customer ID (UUID) for authenticated checkout |
| customerEmail | string | no | Customer email for guest checkout |
| customerName | string | no | Customer name |
| currency | string | no | Currency code |
| expiresInMinutes | number | no | Cart expiration time in minutes |

Example:
```json
{}
```

#### add_cart_item

Add an item to a shopping cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |
| sku | string | yes | Product SKU |
| name | string | yes | Product name |
| quantity | number | yes | Quantity to add |
| unitPrice | number | yes | Unit price |
| description | string | no | Item description |
| imageUrl | string | no | Product image URL |

Example:
```json
{
  "cartId": "<uuid>",
  "sku": "SKU-001",
  "name": "<string>",
  "quantity": 1,
  "unitPrice": 0
}
```

#### update_cart_item

Update the quantity of an item in the cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| itemId | string | yes | Cart item ID (UUID) |
| quantity | number | yes | New quantity |

Example:
```json
{
  "itemId": "<uuid>",
  "quantity": 1
}
```

#### remove_cart_item

Remove an item from the cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| itemId | string | yes | Cart item ID (UUID) |

Example:
```json
{
  "itemId": "<uuid>"
}
```

#### set_cart_shipping_address

Set the shipping address for a cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |
| firstName | string | yes | First name |
| lastName | string | yes | Last name |
| line1 | string | yes | Address line 1 |
| line2 | string | no | Address line 2 |
| city | string | yes | City |
| state | string | no | State/Province |
| postalCode | string | yes | Postal/ZIP code |
| country | string | yes | Country code (e.g., US) |
| phone | string | no | Phone number |
| email | string | no | Email address |

Example:
```json
{
  "cartId": "<uuid>",
  "firstName": "<string>",
  "lastName": "<string>",
  "line1": "<string>",
  "city": "<string>",
  "postalCode": "<string>",
  "country": "<string>"
}
```

#### set_cart_payment

Set the payment method for a cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |
| paymentMethod | string | yes | Payment method (e.g., credit_card, paypal, crypto) |
| paymentToken | string | no | Payment token from payment provider |

Example:
```json
{
  "cartId": "<uuid>",
  "paymentMethod": "<string>"
}
```

#### apply_cart_discount

Apply a coupon/discount code to the cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |
| couponCode | string | yes | Coupon or discount code |

Example:
```json
{
  "cartId": "<uuid>",
  "couponCode": "<string>"
}
```

#### get_shipping_rates

Get available shipping rates for a cart based on contents and address.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |

Example:
```json
{
  "cartId": "<uuid>"
}
```

#### complete_checkout

Complete the checkout process and convert the cart to an order. This is the final step in the checkout flow.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |

Example:
```json
{
  "cartId": "<uuid>"
}
```

#### cancel_cart

Cancel a shopping cart.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |

Example:
```json
{
  "cartId": "<uuid>"
}
```

#### abandon_cart

Mark a cart as abandoned (for recovery campaigns).

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID (UUID) |

Example:
```json
{
  "cartId": "<uuid>"
}
```

#### get_abandoned_carts

Get all abandoned carts for recovery campaigns.

Params: none

Example:
```json
{}
```

### Analytics & Forecasting Tools

#### get_sales_summary

Get sales summary for a time period. Returns total revenue, order count, average order value, items sold, and unique customers.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| period | enum (today, last7days, last30days, this_month, last_month, this_year, all_time) | no | Time period for the summary |

Example:
```json
{}
```

#### get_top_products

Get top selling products by revenue or units sold.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| period | enum (today, last7days, last30days, this_month, last_month, this_year, all_time) | no | Time period |
| limit | number | no | Maximum number of products to return |

Example:
```json
{}
```

#### get_customer_metrics

Get customer metrics including total customers, new customers, returning customers, and average lifetime value.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| period | enum (today, last7days, last30days, this_month, last_month, this_year, all_time) | no | Time period |

Example:
```json
{}
```

#### get_top_customers

Get top customers by total spend.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| period | enum (today, last7days, last30days, this_month, last_month, this_year, all_time) | no | Time period |
| limit | number | no | Maximum number of customers to return |

Example:
```json
{}
```

#### get_inventory_health

Get inventory health summary showing total SKUs, in-stock, low stock, and out of stock counts.

Params: none

Example:
```json
{}
```

#### get_low_stock_items

Get items that are low in stock or approaching reorder point.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| threshold | number | no | Stock threshold to consider as low (default: 10) |

Example:
```json
{}
```

#### get_demand_forecast

Get demand forecast for inventory items based on historical sales. Predicts future demand and days until stockout.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| skus | string | no | List of SKUs to forecast (all items if not specified) |
| daysAhead | number | no | Number of days to forecast ahead |

Example:
```json
{}
```

#### get_revenue_forecast

Get revenue forecast based on historical trends.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| periodsAhead | number | no | Number of periods to forecast |
| granularity | enum (day, week, month) | no | Time granularity |

Example:
```json
{}
```

#### get_order_status_breakdown

Get breakdown of orders by status.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| period | enum (today, last7days, last30days, this_month, last_month, this_year, all_time) | no | Time period |

Example:
```json
{}
```

#### get_return_metrics

Get return metrics including return rate and total refunds.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| period | enum (today, last7days, last30days, this_month, last_month, this_year, all_time) | no | Time period |

Example:
```json
{}
```

### Currency & Exchange Rate Tools

#### get_exchange_rate

Get the exchange rate between two currencies.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| from | string | yes | Source currency code (e.g., USD, EUR, GBP) |
| to | string | yes | Target currency code (e.g., EUR, USD, GBP) |

Example:
```json
{
  "from": "<string>",
  "to": "<string>"
}
```

#### list_exchange_rates

List all available exchange rates, optionally filtered by base currency.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| baseCurrency | string | no | Filter by base currency code (e.g., USD) |

Example:
```json
{}
```

#### convert_currency

Convert an amount from one currency to another using current exchange rates.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| from | string | yes | Source currency code (e.g., USD) |
| to | string | yes | Target currency code (e.g., EUR) |
| amount | number | yes | Amount to convert |

Example:
```json
{
  "from": "<string>",
  "to": "<string>",
  "amount": 0
}
```

#### set_exchange_rate

Set or update an exchange rate between two currencies.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| baseCurrency | string | yes | Base currency code (e.g., USD) |
| quoteCurrency | string | yes | Quote currency code (e.g., EUR) |
| rate | number | yes | Exchange rate (e.g., 0.92 for USD to EUR) |
| source | string | no | Source of the rate (e.g., manual, api) |

Example:
```json
{
  "baseCurrency": "USD",
  "quoteCurrency": "USD",
  "rate": 0
}
```

#### get_currency_settings

Get the store currency settings including base currency and enabled currencies.

Params: none

Example:
```json
{}
```

#### set_base_currency

Set the store's base currency.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| currency | string | yes | Currency code to set as base (e.g., USD, EUR) |

Example:
```json
{
  "currency": "USD"
}
```

#### enable_currencies

Enable currencies for the store.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| currencies | string | yes | List of currency codes to enable (e.g., ["USD", "EUR", "GBP"]) |

Example:
```json
{
  "currencies": "<string>"
}
```

#### format_currency

Format an amount with currency symbol.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| amount | number | yes | Amount to format |
| currency | string | yes | Currency code (e.g., USD, EUR) |

Example:
```json
{
  "amount": 0,
  "currency": "USD"
}
```

### Tax Calculation Tools

#### calculate_tax

Calculate tax for a transaction based on shipping address and line items. Supports US sales tax, EU VAT, and Canadian GST/HST/PST.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| items | string | no | Line item identifier |
| shippingAddress | string | no | Country code (e.g., US, DE, CA) |
| shippingAmount | number | no | Shipping amount (may be taxable) |
| customerId | string | no | Customer ID for exemption lookup |

Example:
```json
{}
```

#### get_tax_rate

Get the effective tax rate for a shipping address and product category.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| country | string | yes | Country code (e.g., US, DE, CA) |
| state | string | no | State/Province code (e.g., CA, TX, ON) |
| city | string | no | City name |
| taxCategory | string | no | Product tax category: standard, reduced, exempt, digital, food, clothing, medical |

Example:
```json
{
  "country": "<string>"
}
```

#### list_tax_jurisdictions

List tax jurisdictions with optional filtering by country or level.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| countryCode | string | no | Filter by country code (e.g., US, DE, CA) |
| level | string | no | Filter by level: country, state, county, city, district |

Example:
```json
{}
```

#### list_tax_rates

List tax rates for a jurisdiction or all active rates.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| jurisdictionId | string | no | Filter by jurisdiction ID |
| taxType | string | no | Filter by tax type: sales_tax, vat, gst, hst, pst, qst |
| productCategory | string | no | Filter by product category: standard, reduced, exempt, digital |

Example:
```json
{}
```

#### get_tax_settings

Get the store tax calculation settings.

Params: none

Example:
```json
{}
```

#### get_us_state_tax_info

Get pre-configured US state sales tax information including rates and rules.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| stateCode | string | yes | US state code (e.g., CA, TX, NY) |

Example:
```json
{
  "stateCode": "<string>"
}
```

#### get_customer_tax_exemptions

Get active tax exemptions for a customer.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | yes | Customer ID |

Example:
```json
{
  "customerId": "<uuid>"
}
```

#### create_tax_exemption

Create a tax exemption certificate for a customer.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | yes | Customer ID |
| exemptionType | string | yes | Type: resale, non_profit, government, educational, religious, medical, manufacturing, agricultural, export, diplomatic |
| certificateNumber | string | no | Exemption certificate number |
| issuingAuthority | string | no | Issuing authority (e.g., state name) |
| expiresAt | string | no | Expiration date (YYYY-MM-DD) |

Example:
```json
{
  "customerId": "<uuid>",
  "exemptionType": "<string>"
}
```

#### calculate_cart_tax

Calculate and apply tax to a cart based on its shipping address. Must set shipping address first. Returns tax breakdown and updates cart totals.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID to calculate tax for |

Example:
```json
{
  "cartId": "<uuid>"
}
```

### Promotions & Discounts Tools

#### list_promotions

List all promotions. Shows active, paused, and scheduled promotions with their discount details.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| status | enum (active, paused, draft, expired, scheduled) | no | Filter by promotion status |
| type | enum (percentage_off, fixed_amount_off, buy_x_get_y, free_shipping, tiered_discount) | no | Filter by promotion type |

Example:
```json
{}
```

#### get_promotion

Get a promotion by ID or internal code.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| identifier | string | yes | Promotion ID (UUID) or internal code |

Example:
```json
{
  "identifier": "<string>"
}
```

#### create_promotion

Create a new promotion. Supports percentage off, fixed amount off, BOGO, free shipping, and tiered discounts.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| name | string | yes | Promotion name (e.g., "Summer Sale") |
| type | enum (percentage_off, fixed_amount_off, buy_x_get_y, free_shipping, tiered_discount) | yes | Type of discount |
| trigger | enum (automatic, coupon_code, both) | no | How the promotion is triggered |
| percentageOff | number | no | Percentage discount (0.20 = 20% off) |
| fixedAmountOff | number | no | Fixed amount discount in dollars |
| maxDiscountAmount | number | no | Maximum discount cap |
| description | string | no | Public description |
| startsAt | string | no | Start date (ISO 8601) |
| endsAt | string | no | End date (ISO 8601) |

Example:
```json
{
  "name": "<string>",
  "type": "percentage_off"
}
```

#### activate_promotion

Activate a promotion to make it available for use.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| promotionId | string | yes | Promotion ID to activate |

Example:
```json
{
  "promotionId": "<uuid>"
}
```

#### deactivate_promotion

Pause/deactivate a promotion.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| promotionId | string | yes | Promotion ID to deactivate |

Example:
```json
{
  "promotionId": "<uuid>"
}
```

#### create_coupon

Create a coupon code for a promotion.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| promotionId | string | yes | Promotion ID to create coupon for |
| code | string | yes | Coupon code (e.g., "SUMMER25") |
| usageLimit | number | no | Maximum number of times this coupon can be used |
| perCustomerLimit | number | no | Max uses per customer |
| startsAt | string | no | Coupon valid from (ISO 8601) |
| endsAt | string | no | Coupon valid until (ISO 8601) |

Example:
```json
{
  "promotionId": "<uuid>",
  "code": "<string>"
}
```

#### validate_coupon

Check if a coupon code is valid and can be used.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| code | string | yes | Coupon code to validate |

Example:
```json
{
  "code": "<string>"
}
```

#### list_coupons

List coupon codes with optional filters.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| promotionId | string | no | Filter by promotion ID |
| status | enum (active, expired, depleted, disabled) | no | Filter by status |

Example:
```json
{}
```

#### get_active_promotions

Get all currently active promotions.

Params: none

Example:
```json
{}
```

#### apply_cart_promotions

Calculate and apply all applicable promotions to a cart. Uses coupon codes on the cart and automatic promotions.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cartId | string | yes | Cart ID to apply promotions to |

Example:
```json
{
  "cartId": "<uuid>"
}
```

### Subscription Tools

#### list_subscription_plans

List all subscription plans. Filter by status (draft, active, archived) or billing interval.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| status | enum (draft, active, archived) | no | Filter by plan status |
| billingInterval | enum (weekly, biweekly, monthly, bimonthly, quarterly, semiannual, annual) | no | Filter by billing interval |

Example:
```json
{}
```

#### get_subscription_plan

Get details for a specific subscription plan.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| planId | string | yes | Plan ID or code |

Example:
```json
{
  "planId": "<uuid>"
}
```

#### create_subscription_plan

Create a new subscription plan. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| name | string | yes | Plan name |
| billingInterval | enum (weekly, biweekly, monthly, bimonthly, quarterly, semiannual, annual) | yes | Billing interval |
| price | number | yes | Price per billing cycle |
| currency | string | no | Currency code (default: USD) |
| trialDays | number | no | Trial period in days |
| description | string | no | Plan description |
| setupFee | number | no | One-time setup fee |

Example:
```json
{
  "name": "<string>",
  "billingInterval": "weekly",
  "price": 0
}
```

#### activate_subscription_plan

Activate a subscription plan (make it available for new subscriptions). Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| planId | string | yes | Plan ID to activate |

Example:
```json
{
  "planId": "<uuid>"
}
```

#### archive_subscription_plan

Archive a subscription plan (no new subscriptions, existing ones continue). Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| planId | string | yes | Plan ID to archive |

Example:
```json
{
  "planId": "<uuid>"
}
```

#### list_subscriptions

List subscriptions. Filter by customer, plan, or status.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | no | Filter by customer ID |
| planId | string | no | Filter by plan ID |
| status | enum (trial, active, paused, past_due, cancelled, expired, pending) | no | Filter by status |

Example:
```json
{}
```

#### get_subscription

Get details for a specific subscription.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID or number |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

#### create_subscription

Create a new subscription for a customer. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | yes | Customer ID |
| planId | string | yes | Plan ID |
| paymentMethodId | string | no | Payment method ID from payment provider |
| skipTrial | boolean | no | Skip trial period |
| couponCode | string | no | Coupon code to apply |

Example:
```json
{
  "customerId": "<uuid>",
  "planId": "<uuid>"
}
```

#### pause_subscription

Pause a subscription (stops billing, can resume later). Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID |
| resumeAt | string | no | ISO date when to auto-resume |
| reason | string | no | Reason for pausing |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

#### resume_subscription

Resume a paused subscription. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

#### cancel_subscription

Cancel a subscription. By default cancels at end of period. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID |
| immediate | boolean | no | Cancel immediately (default: false, cancels at period end) |
| reason | string | no | Reason for cancellation |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

#### skip_billing_cycle

Skip the next billing cycle for a subscription. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID |
| reason | string | no | Reason for skipping |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

#### list_billing_cycles

List billing cycles for a subscription.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID |
| status | enum (scheduled, processing, paid, failed, skipped, refunded, voided) | no | Filter by status |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

#### get_billing_cycle

Get details for a specific billing cycle.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| cycleId | string | yes | Billing cycle ID |

Example:
```json
{
  "cycleId": "<uuid>"
}
```

#### get_subscription_events

Get event history (audit log) for a subscription.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| subscriptionId | string | yes | Subscription ID |
| limit | number | no | Maximum events to return |

Example:
```json
{
  "subscriptionId": "<uuid>"
}
```

### Sync Tools (Verifiable Event Sync)

#### sync_status

Get the current sync status between local database and remote sequencer. Shows pending events, sync lag, and connection status.

Params: none

Example:
```json
{}
```

#### sync_push

Push pending local events to the remote sequencer. Requires --apply flag for actual push.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| batchSize | number | no | Maximum events to push in one batch (default: 100) |
| dryRun | boolean | no | Show what would be pushed without actually pushing |

Example:
```json
{}
```

#### sync_pull

Pull events from the remote sequencer and store them locally.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| fromSequence | number | no | Start pulling from this sequence number |
| limit | number | no | Maximum events to pull (default: 1000) |

Example:
```json
{}
```

#### sync_outbox

List events in the local outbox. Shows pending, synced, failed, and rejected events.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| status | enum (pending, synced, failed, rejected, all) | no | Filter by status (default: all) |
| limit | number | no | Maximum events to return (default: 20) |

Example:
```json
{}
```

#### sync_retry_failed

Reset failed events to pending status so they can be retried. Requires --apply flag.

Params: none

Example:
```json
{}
```

#### sync_entity_history

Get the event history for a specific entity from the remote sequencer.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| entityType | string | yes | Entity type (order, customer, product, inventory, return, cart) |
| entityId | string | yes | Entity ID |

Example:
```json
{
  "entityType": "<string>",
  "entityId": "<uuid>"
}
```

#### sync_full

Perform a full sync: push pending events then pull new events. Requires --apply flag for push.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| pushBatchSize | number | no | Maximum events to push (default: 100) |
| pullLimit | number | no | Maximum events to pull (default: 1000) |

Example:
```json
{}
```

### Sync Conflict Resolution Tools

#### sync_conflicts

List unresolved sync conflicts. Conflicts occur when local and remote events modify the same entity concurrently.

Params: none

Example:
```json
{}
```

#### sync_resolve

Resolve a specific sync conflict using a resolution strategy. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| conflictId | string | yes | The conflict ID to resolve |
| strategy | enum (remote-wins, local-wins, merge) | no | Resolution strategy (default: uses suggested strategy) |

Example:
```json
{
  "conflictId": "<uuid>"
}
```

#### sync_rebase

Resolve all sync conflicts using a resolution strategy. Requires --apply flag.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| strategy | enum (remote-wins, local-wins, merge) | no | Resolution strategy for all conflicts (default: remote-wins) |

Example:
```json
{}
```

### Manufacturing Tools - BOM & Work Orders

#### list_boms

List all Bills of Materials (BOMs). BOMs define the components/ingredients needed to manufacture a product.

Params: none

Example:
```json
{}
```

#### get_bom

Get a Bill of Materials by ID, including all components/ingredients.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| bomId | string | yes | BOM ID or BOM number |

Example:
```json
{
  "bomId": "<uuid>"
}
```

#### create_bom

Create a new Bill of Materials for a product. Defines what components/ingredients are needed.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| name | string | yes | BOM name (e.g., "Classic Pickled Onions Recipe") |
| productId | string | yes | Product ID this BOM is for |
| description | string | no | Description of this BOM |
| revision | string | no | Revision number (default: A) |

Example:
```json
{
  "name": "<string>",
  "productId": "<uuid>"
}
```

#### add_bom_component

Add a component/ingredient to a Bill of Materials.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| bomId | string | yes | BOM ID to add component to |
| name | string | yes | Component name (e.g., "Yellow Onions") |
| sku | string | no | Component SKU if from inventory |
| quantity | number | yes | Quantity needed per unit produced |
| unitOfMeasure | string | no | Unit (e.g., "kg", "lbs", "each", "ml") |
| notes | string | no | Notes about this component |

Example:
```json
{
  "bomId": "<uuid>",
  "name": "<string>",
  "quantity": 1
}
```

#### activate_bom

Activate a BOM to make it available for work orders.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| bomId | string | yes | BOM ID to activate |

Example:
```json
{
  "bomId": "<uuid>"
}
```

#### list_work_orders

List all manufacturing work orders. Work orders track production runs.

Params: none

Example:
```json
{}
```

#### get_work_order

Get a work order by ID with full details.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| workOrderId | string | yes | Work order ID or number |

Example:
```json
{
  "workOrderId": "<uuid>"
}
```

#### create_work_order

Create a manufacturing work order to produce a quantity of product.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| productId | string | yes | Product ID to manufacture |
| bomId | string | no | BOM ID to use (optional) |
| quantityToBuild | number | yes | Number of units to produce |
| priority | enum (low, normal, high, urgent) | no | Priority level |
| scheduledStart | string | no | Scheduled start date (ISO format) |
| scheduledEnd | string | no | Scheduled end date (ISO format) |
| notes | string | no | Production notes |

Example:
```json
{
  "productId": "<uuid>",
  "quantityToBuild": 1
}
```

#### start_work_order

Start a work order (begin production).

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| workOrderId | string | yes | Work order ID to start |

Example:
```json
{
  "workOrderId": "<uuid>"
}
```

#### complete_work_order

Complete a work order with the quantity produced.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| workOrderId | string | yes | Work order ID to complete |
| quantityCompleted | number | yes | Number of units actually produced |

Example:
```json
{
  "workOrderId": "<uuid>",
  "quantityCompleted": 1
}
```

#### cancel_work_order

Cancel a work order.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| workOrderId | string | yes | Work order ID to cancel |

Example:
```json
{
  "workOrderId": "<uuid>"
}
```

### Payment Tools

#### list_payments

List all payments in the system.

Params: none

Example:
```json
{}
```

#### get_payment

Get a payment by ID.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| paymentId | string | yes | Payment ID |

Example:
```json
{
  "paymentId": "<uuid>"
}
```

#### create_payment

Create a payment for an order.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| orderId | string | yes | Order ID |
| amount | number | yes | Payment amount |
| currency | string | no | Currency (default: USD) |
| method | string | no | Payment method: credit_card, paypal, bank_transfer, crypto |

Example:
```json
{
  "orderId": "<uuid>",
  "amount": 0
}
```

#### complete_payment

Mark a payment as completed.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| paymentId | string | yes | Payment ID |

Example:
```json
{
  "paymentId": "<uuid>"
}
```

#### create_refund

Create a refund for a payment.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| paymentId | string | yes | Payment ID to refund |
| amount | number | yes | Refund amount |
| reason | string | no | Refund reason |

Example:
```json
{
  "paymentId": "<uuid>",
  "amount": 0
}
```

### Stablecoin Payment Tools (Native Crypto Payments)

#### get_agent_wallet

Get the agent wallet address for a specific blockchain. Returns the wallet address derived from VES keys.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| chain | string | no | Blockchain: solana, solana_devnet, set_chain, base, ethereum, arbitrum (default: solana) |

Example:
```json
{}
```

#### get_wallet_balance

Check the stablecoin balance of the agent wallet on a blockchain.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| chain | string | no | Blockchain: solana, set_chain, base (default: solana) |
| token | string | no | Token symbol: USDC, ssUSD, USDT (default: chain stablecoin) |

Example:
```json
{}
```

#### create_stablecoin_payment

Create and execute a stablecoin payment to a wallet address. Supports USDC on Solana, ssUSD on SET Chain, etc.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| toAddress | string | yes | Recipient wallet address |
| amount | number | yes | Amount to send (e.g., 50.00) |
| chain | string | no | Blockchain: solana, set_chain, base (default: solana) |
| token | string | no | Token: USDC, ssUSD (default: chain stablecoin) |
| orderId | string | no | Order ID for audit trail |
| customerId | string | no | Customer ID for audit trail |
| memo | string | no | Payment memo |

Example:
```json
{
  "toAddress": "<string>",
  "amount": 0
}
```

#### list_supported_chains

List all supported blockchain networks for stablecoin payments.

Params: none

Example:
```json
{}
```

### Shipment Tools

#### list_shipments

List all shipments.

Params: none

Example:
```json
{}
```

#### create_shipment

Create a shipment for an order.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| orderId | string | yes | Order ID |
| carrier | string | no | Carrier: USPS, UPS, FedEx, DHL |
| service | string | no | Service level |

Example:
```json
{
  "orderId": "<uuid>"
}
```

#### deliver_shipment

Mark a shipment as delivered.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| shipmentId | string | yes | Shipment ID |

Example:
```json
{
  "shipmentId": "<uuid>"
}
```

### Supplier & Purchase Order Tools

#### list_suppliers

List all suppliers.

Params: none

Example:
```json
{}
```

#### create_supplier

Create a new supplier.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| name | string | yes | Supplier name |
| email | string | no | Contact email |
| phone | string | no | Phone number |
| address | string | no | Address |

Example:
```json
{
  "name": "<string>"
}
```

#### list_purchase_orders

List all purchase orders.

Params: none

Example:
```json
{}
```

#### create_purchase_order

Create a purchase order to a supplier.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| supplierId | string | yes | Supplier ID |
| items | string | yes | JSON array: [{"sku":"X","name":"Y","quantity":10,"unitPrice":5.00}] |
| notes | string | no | Notes |

Example:
```json
{
  "supplierId": "<uuid>",
  "items": "<string>"
}
```

#### approve_purchase_order

Approve a purchase order.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| purchaseOrderId | string | yes | PO ID |
| approvedBy | string | yes | Approver name |

Example:
```json
{
  "purchaseOrderId": "<uuid>",
  "approvedBy": "<string>"
}
```

#### send_purchase_order

Send a PO to the supplier.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| purchaseOrderId | string | yes | PO ID |

Example:
```json
{
  "purchaseOrderId": "<uuid>"
}
```

### Invoice Tools

#### list_invoices

List all invoices.

Params: none

Example:
```json
{}
```

#### create_invoice

Create an invoice for a customer.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | yes | Customer ID |
| orderId | string | no | Order ID |
| items | string | yes | JSON array: [{"description":"X","quantity":1,"unitPrice":10.00}] |
| dueDate | string | no | Due date ISO |
| notes | string | no | Notes |

Example:
```json
{
  "customerId": "<uuid>",
  "items": "<string>"
}
```

#### send_invoice

Send an invoice to the customer.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| invoiceId | string | yes | Invoice ID |

Example:
```json
{
  "invoiceId": "<uuid>"
}
```

#### record_invoice_payment

Record payment on an invoice.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| invoiceId | string | yes | Invoice ID |
| amount | number | yes | Amount paid |
| paymentMethod | string | no | Payment method |
| reference | string | no | Check/reference number |

Example:
```json
{
  "invoiceId": "<uuid>",
  "amount": 0
}
```

#### get_overdue_invoices

Get all overdue invoices.

Params: none

Example:
```json
{}
```

### Warranty Tools

#### list_warranties

List all warranties.

Params: none

Example:
```json
{}
```

#### create_warranty

Create a warranty for a product.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| customerId | string | yes | Customer ID (required) |
| orderId | string | no | Order ID |
| productId | string | no | Product ID |
| warrantyType | string | no | Type: standard, extended, lifetime |
| durationMonths | number | no | Duration in months |

Example:
```json
{
  "customerId": "<uuid>"
}
```

#### create_warranty_claim

File a warranty claim.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| warrantyId | string | yes | Warranty ID |
| description | string | yes | Issue description |
| claimType | string | no | Type: repair, replacement, refund |

Example:
```json
{
  "warrantyId": "<uuid>",
  "description": "<string>"
}
```

#### approve_warranty_claim

Approve a warranty claim.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| claimId | string | yes | Claim ID |

Example:
```json
{
  "claimId": "<uuid>"
}
```


## Autonomous Engine MCP Tools

### Scheduler Tools

#### list_scheduled_jobs

List all scheduled jobs. Returns job details including schedule, next run time, and status.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| status | enum (all, enabled, disabled) | no | Filter by status |

Example:
```json
{}
```

#### create_scheduled_job

Create a new scheduled job. Supports cron expressions, intervals, and one-time execution.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| name | string | yes | Job name |
| description | string | no | Job description |
| type | enum (cron, interval, once) | yes | Schedule type |
| schedule | string | yes | Cron expression (e.g., "0 * * * *"), interval in ms, or ISO date for once |
| agent | string | yes | Agent to execute (e.g., "inventory", "analytics") |
| request | string | yes | Request to send to the agent |
| enabled | boolean | no | Whether job is enabled |

Example:
```json
{
  "name": "<string>",
  "type": "cron",
  "schedule": "<string>",
  "agent": "<string>",
  "request": "<string>"
}
```

#### run_job_now

Manually trigger a scheduled job to run immediately.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| jobId | string | yes | Job ID to run |

Example:
```json
{
  "jobId": "<uuid>"
}
```

#### toggle_job

Enable or disable a scheduled job.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| jobId | string | yes | Job ID |
| enabled | boolean | yes | Whether to enable the job |

Example:
```json
{
  "jobId": "<uuid>",
  "enabled": true
}
```

#### get_scheduler_status

Get scheduler status including running jobs and recent history.

Params: none

Example:
```json
{}
```

### Workflow Tools

#### list_workflows

List all registered workflow definitions.

Params: none

Example:
```json
{}
```

#### start_workflow

Start a new workflow instance.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| workflowId | string | yes | Workflow definition ID |
| context | record | no | Initial context data |

Example:
```json
{
  "workflowId": "<uuid>"
}
```

#### transition_workflow

Transition a workflow instance to a new state.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| instanceId | string | yes | Workflow instance ID |
| targetState | string | yes | Target state name |
| context | record | no | Additional context for transition |

Example:
```json
{
  "instanceId": "<uuid>",
  "targetState": "<string>"
}
```

#### trigger_workflow_event

Trigger an event on a workflow instance.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| instanceId | string | yes | Workflow instance ID |
| event | string | yes | Event/transition name |
| context | record | no | Event context |

Example:
```json
{
  "instanceId": "<uuid>",
  "event": "<string>"
}
```

#### list_workflow_instances

List workflow instances.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| workflowId | string | no | Filter by workflow ID |
| status | enum (running, completed, failed, paused, cancelled) | no | Filter by status |

Example:
```json
{}
```

#### get_workflow_status

Get overall workflow engine status.

Params: none

Example:
```json
{}
```

### Policy Tools

#### list_policies

List all registered policy sets.

Params: none

Example:
```json
{}
```

#### evaluate_policy

Evaluate policies for a domain with given context.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| domain | string | yes | Policy domain (e.g., "orders", "returns", "inventory") |
| context | record | yes | Context for evaluation |

Example:
```json
{
  "domain": "<string>",
  "context": {}
}
```

#### get_policy_status

Get policy engine status.

Params: none

Example:
```json
{}
```

### Approval Tools

#### list_pending_approvals

List pending approval requests.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| domain | string | no | Filter by domain |

Example:
```json
{}
```

#### create_approval_request

Create a new approval request for an operation.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| domain | string | yes | Domain (e.g., "orders", "returns", "purchase_orders") |
| entityType | string | yes | Entity type (e.g., "order", "return") |
| entityId | string | yes | Entity ID |
| title | string | yes | Request title |
| description | string | no | Request description |
| amount | number | no | Amount for threshold-based routing |
| requestedBy | string | yes | Requester ID |

Example:
```json
{
  "domain": "<string>",
  "entityType": "<string>",
  "entityId": "<uuid>",
  "title": "<string>",
  "requestedBy": "<string>"
}
```

#### approve_request

Approve an approval request.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| requestId | string | yes | Approval request ID |
| approverId | string | yes | Approver ID |
| reason | string | no | Approval reason |

Example:
```json
{
  "requestId": "<uuid>",
  "approverId": "<uuid>"
}
```

#### reject_request

Reject an approval request.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| requestId | string | yes | Approval request ID |
| approverId | string | yes | Approver ID |
| reason | string | no | Rejection reason |

Example:
```json
{
  "requestId": "<uuid>",
  "approverId": "<uuid>"
}
```

#### get_approval_status

Get approval queue status.

Params: none

Example:
```json
{}
```

### Webhook Tools

#### list_webhook_sources

List registered webhook sources.

Params: none

Example:
```json
{}
```

#### list_webhook_handlers

List registered webhook handlers.

Params: none

Example:
```json
{}
```

#### get_webhook_history

Get recent webhook event history.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| limit | number | no | Number of events to return |

Example:
```json
{}
```

#### get_webhook_status

Get webhook server status.

Params: none

Example:
```json
{}
```

### Engine-Level Tools

#### get_autonomous_status

Get comprehensive status of the autonomous business engine.

Params: none

Example:
```json
{}
```

#### pre_operation_check

Run policy and approval check before an operation.

Params:

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| domain | string | yes | Operation domain |
| context | record | yes | Operation context |

Example:
```json
{
  "domain": "<string>",
  "context": {}
}
```
