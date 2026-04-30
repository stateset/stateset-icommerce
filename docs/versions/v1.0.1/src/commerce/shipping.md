# Shipping & Fulfillment

iCommerce provides configurable shipping zones, rate calculation, carrier mapping, and shipment tracking to support domestic and international fulfillment.

## Shipping Zones

Define geographic zones with countries, regions, and postal code ranges:

```javascript
// Create a domestic zone
await toolkit.executeTool('create_shipping_zone', {
    name: 'US Continental',
    countries: ['US'],
    excludeRegions: ['HI', 'AK'],
});

// Create an international zone
await toolkit.executeTool('create_shipping_zone', {
    name: 'EU',
    countries: ['DE', 'FR', 'IT', 'ES', 'NL', 'BE', 'AT', 'PT', 'IE', 'FI', 'SE'],
});

// Zone with postal code ranges
await toolkit.executeTool('create_shipping_zone', {
    name: 'NYC Metro',
    countries: ['US'],
    postalCodeRanges: [
        { from: '10001', to: '10299' },
        { from: '11201', to: '11256' },
    ],
});
```

## Shipping Methods & Rates

Attach methods and rates to zones:

```javascript
// Add shipping methods to a zone
await toolkit.executeTool('create_shipping_method', {
    zoneId: 'zone-us-continental',
    name: 'Standard Shipping',
    carrier: 'USPS',
    minDays: 5,
    maxDays: 7,
    rates: [
        { minWeight: 0, maxWeight: 1, price: 5.99 },
        { minWeight: 1, maxWeight: 5, price: 8.99 },
        { minWeight: 5, maxWeight: 20, price: 12.99 },
    ],
});

await toolkit.executeTool('create_shipping_method', {
    zoneId: 'zone-us-continental',
    name: 'Express Shipping',
    carrier: 'FedEx',
    minDays: 1,
    maxDays: 3,
    rates: [
        { minWeight: 0, maxWeight: 1, price: 12.99 },
        { minWeight: 1, maxWeight: 5, price: 18.99 },
        { minWeight: 5, maxWeight: 20, price: 29.99 },
    ],
});
```

## Rate Calculation

At checkout, rates are calculated based on the shipping address zone match:

```javascript
const rates = await toolkit.executeTool('calculate_shipping_rates', {
    orderId: 'ord-123',
    shippingAddress: {
        country: 'US',
        region: 'CA',
        postalCode: '90210',
    },
});
// → [
//     { method: 'Standard Shipping', carrier: 'USPS', price: 8.99, minDays: 5, maxDays: 7 },
//     { method: 'Express Shipping', carrier: 'FedEx', price: 18.99, minDays: 1, maxDays: 3 },
// ]
```

Zone matching priority: postal code range > region > country. The most specific match wins.

## Shipment Creation

```javascript
// Ship an order
await toolkit.executeTool('ship_order', {
    orderId: 'ord-123',
    carrier: 'FedEx',
    trackingNumber: 'FEDEX-789456',
    items: [
        { sku: 'WIDGET-001', quantity: 2 },
        { sku: 'GADGET-002', quantity: 1 },
    ],
});
```

### Shipment Lifecycle

```
pending → shipped → in_transit → delivered
                               → exception (delay, damage)
                               → returned_to_sender
```

## Shipment Tracking

```javascript
// Get tracking status
const shipment = await toolkit.executeTool('get_shipment', {
    shipmentId: 'ship-001',
});
// → {
//     status: 'in_transit',
//     carrier: 'FedEx',
//     trackingNumber: 'FEDEX-789456',
//     estimatedDelivery: '2026-03-19',
//     events: [
//         { timestamp: '2026-03-17T10:00:00Z', status: 'shipped', location: 'Warehouse A' },
//         { timestamp: '2026-03-17T18:00:00Z', status: 'in_transit', location: 'Distribution Center' },
//     ],
// }

// Update tracking event
await toolkit.executeTool('update_shipment', {
    shipmentId: 'ship-001',
    status: 'delivered',
    deliveredAt: '2026-03-19T14:30:00Z',
});
```

## Split Shipments

When an order ships from multiple warehouses:

```javascript
// First shipment (warehouse A)
await toolkit.executeTool('ship_order', {
    orderId: 'ord-123',
    carrier: 'USPS',
    trackingNumber: 'USPS-111',
    items: [{ sku: 'WIDGET-001', quantity: 2 }],
    partial: true,
});

// Second shipment (warehouse B)
await toolkit.executeTool('ship_order', {
    orderId: 'ord-123',
    carrier: 'FedEx',
    trackingNumber: 'FEDEX-222',
    items: [{ sku: 'GADGET-002', quantity: 1 }],
    partial: true,
});
```

## Free Shipping Rules

Use the policy engine to implement free shipping thresholds:

```yaml
# policies/shipping.yaml
name: Free Shipping
domain: shipping
rules:
  - name: free-over-75
    conditions:
      - field: order_subtotal
        operator: greater_than_or_equal
        value: 75
    actions:
      - type: transform
        field: shipping_price
        value: 0
        reason: "Free shipping on orders $75+"
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `create_shipping_zone` | Define a geographic shipping zone |
| `list_shipping_zones` | List all zones |
| `create_shipping_method` | Add a method with rates to a zone |
| `calculate_shipping_rates` | Get available rates for an address |
| `ship_order` | Create a shipment with tracking |
| `get_shipment` | Get shipment details and tracking |
| `update_shipment` | Update shipment status |
| `list_shipments` | List shipments with filters |
