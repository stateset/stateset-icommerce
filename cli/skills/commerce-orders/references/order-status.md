# Order Status and Transitions

## Status Flow

- pending -> confirmed -> processing -> shipped -> delivered
- cancelled and refunded are terminal outcomes

## Allowed Transitions

- pending: confirmed, cancelled
- confirmed: processing, cancelled
- processing: shipped
- shipped: delivered

## Common Commands

- `stateset --apply "confirm order ORD-123"`
- `stateset --apply "ship order ORD-123 with tracking TRACK123"`
- `stateset --apply "cancel order ORD-123"`

## Related Fields

- paymentStatus: pending, paid, failed, refunded
- fulfillmentStatus: unfulfilled, partially_fulfilled, fulfilled, returned
